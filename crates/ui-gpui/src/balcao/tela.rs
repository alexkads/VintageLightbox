//! O passo 6 do fluxo: **o cliente paga no balcão**.
//!
//! # O que se registra aqui, e o que não
//!
//! 🔒 **Nada aqui muda o preço de venda.** É registro do que já aconteceu:
//! cortesia, desconto, ou "já pagou em site parceiro". O que a galeria online
//! cobra é outro campo, e não se toca nele por engano ao anotar um acerto de
//! balcão — a distinção é do próprio backend (`preco_negociado` versus
//! `preco_de_venda`) e está preservada aqui.
//!
//! # 🚨 Só há o que negociar em foto que está no site
//!
//! A negociação se grava na linha da foto no `recordarfotos.com.br`. Uma foto
//! que nunca subiu não tem essa linha — e o caminho para ela existir é
//! classificar (passo 3). A tela diz isso em vez de deixar o botão ligado para
//! dar erro depois.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use adapters::view_models::PhotoViewModel;
use biblioteca_core::dinheiro;
use biblioteca_core::negociacao::{self, Negociacao, Tipo, PARCEIROS};
use domain::services::pos_venda::{MudancaDaFoto, Sessao};
use gpui::{div, prelude::*, px, Context, SharedString, Task, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{ActiveTheme, Disableable, Selectable, Sizable};

use crate::pos_venda::porta::{Publicador, Recado};

const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(100);

pub struct Balcao {
    publicador: Arc<dyn Publicador>,
    sessao: Option<Sessao>,
    /// A seleção que veio da grade.
    fotos: Vec<PhotoViewModel>,
    tipo: Tipo,
    preco: gpui::Entity<InputState>,
    cupom: gpui::Entity<InputState>,
    motivo: gpui::Entity<InputState>,
    parceiro: String,
    enviando: usize,
    /// Quantas o site já confirmou nesta rodada.
    gravadas: usize,
    erro: Option<SharedString>,
    recados: (Sender<Recado>, Receiver<Recado>),
    colhendo: bool,
    _colheita: Option<Task<()>>,
}

impl Balcao {
    pub fn nova(
        publicador: Arc<dyn Publicador>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            publicador,
            sessao: None,
            fotos: Vec::new(),
            tipo: Tipo::Cortesia,
            preco: cx.new(|cx| InputState::new(window, cx).placeholder("19,90")),
            cupom: cx.new(|cx| InputState::new(window, cx).placeholder("cupom")),
            motivo: cx.new(|cx| InputState::new(window, cx).placeholder("aniversário")),
            parceiro: PARCEIROS[0].to_string(),
            enviando: 0,
            gravadas: 0,
            erro: None,
            recados: channel(),
            colhendo: false,
            _colheita: None,
        }
    }

    pub fn definir_sessao(&mut self, sessao: Sessao, cx: &mut Context<Self>) {
        self.sessao = Some(sessao);
        cx.notify();
    }

    /// Abre o balcão para uma seleção.
    pub fn abrir_para(&mut self, fotos: Vec<PhotoViewModel>, cx: &mut Context<Self>) {
        self.fotos = fotos;
        self.erro = None;
        self.gravadas = 0;
        cx.notify();
    }

    /// As que podem receber a negociação — as que já estão no site.
    pub fn negociaveis(&self) -> Vec<&PhotoViewModel> {
        self.fotos
            .iter()
            .filter(|f| f.pos_venda_foto_id.is_some())
            .collect()
    }

    /// As que não têm linha no site, e por isso ficam de fora.
    pub fn fora(&self) -> usize {
        self.fotos.len() - self.negociaveis().len()
    }

    pub fn escolher_tipo(&mut self, tipo: Tipo, cx: &mut Context<Self>) {
        self.tipo = tipo;
        self.erro = None;
        cx.notify();
    }

    pub fn escolher_parceiro(&mut self, parceiro: &str, cx: &mut Context<Self>) {
        self.parceiro = parceiro.to_string();
        cx.notify();
    }

    pub fn tipo(&self) -> Tipo {
        self.tipo
    }

    pub fn parceiro(&self) -> &str {
        &self.parceiro
    }

    /// O que a tela está pedindo, no formato do core.
    fn negociacao(&self, cx: &Context<Self>) -> Negociacao {
        Negociacao {
            tipo: self.tipo,
            preco: dinheiro::ler_campo(&self.preco.read(cx).value()),
            parceiro: self.parceiro.clone(),
            cupom: self.cupom.read(cx).value().to_string(),
            motivo: self.motivo.read(cx).value().to_string(),
        }
    }

    /// Grava a negociação na seleção.
    ///
    /// 🔑 **O core decide se falta alguma coisa**, e a recusa aparece aqui sem
    /// ida ao servidor: é a mesma `montar` que a tela do site usa, então o que é
    /// recusado aqui é recusado lá.
    pub fn registrar(&mut self, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            self.erro = Some("esta ação precisa da conta do site".into());
            cx.notify();
            return;
        };
        if self.enviando > 0 {
            return;
        }

        let gravavel = match negociacao::montar(&self.negociacao(cx)) {
            Ok(g) => g,
            Err(falta) => {
                self.erro = Some(falta.into());
                cx.notify();
                return;
            }
        };

        let alvos: Vec<String> = self
            .negociaveis()
            .iter()
            .filter_map(|f| f.pos_venda_foto_id.clone())
            .collect();
        if alvos.is_empty() {
            self.erro =
                Some("nenhuma das fotos escolhidas está no site — classifique-as primeiro".into());
            cx.notify();
            return;
        }

        // 🔑 O decimal em texto é o que a API fala, e a conversão é a mesma que
        // o campo de preço usa ao ler. `None` continua `None`: "voltar ao preço
        // da faixa" é diferente de "não mexer".
        let mudanca = MudancaDaFoto {
            preco_negociado: Some(gravavel.preco_negociado.map(centavos_em_decimal)),
            observacao_da_negociacao: Some(gravavel.observacao.clone()),
            ..MudancaDaFoto::default()
        };

        self.erro = None;
        self.gravadas = 0;
        self.enviando = alvos.len();
        for foto_id in alvos {
            self.publicador.negociar(
                sessao.clone(),
                foto_id,
                mudanca.clone(),
                self.recados.0.clone(),
            );
        }
        self.acompanhar(cx);
        cx.notify();
    }

    fn acompanhar(&mut self, cx: &mut Context<Self>) {
        if self.colhendo {
            return;
        }
        self.colhendo = true;
        self._colheita = Some(cx.spawn(async move |esta, cx| loop {
            cx.background_executor().timer(INTERVALO_DE_COLHEITA).await;
            let Ok(continua) = esta.update(cx, |tela, cx| tela.colher(cx)) else {
                break;
            };
            if !continua {
                break;
            }
        }));
    }

    pub fn colher(&mut self, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;
        while let Ok(recado) = self.recados.1.try_recv() {
            mudou = true;
            match recado {
                Recado::Sincronizou => {
                    self.gravadas += 1;
                    self.enviando = self.enviando.saturating_sub(1);
                }
                Recado::Falhou(erro) => {
                    self.enviando = self.enviando.saturating_sub(1);
                    self.erro = Some(erro.into());
                }
                _ => {}
            }
        }
        if mudou {
            cx.notify();
        }
        let continua = self.enviando > 0;
        if !continua {
            self.colhendo = false;
        }
        continua
    }

    pub fn gravadas(&self) -> usize {
        self.gravadas
    }

    pub fn erro(&self) -> Option<&SharedString> {
        self.erro.as_ref()
    }

    pub fn resumo(&self) -> String {
        let quantas = self.negociaveis().len();
        let fora = self.fora();
        match (self.enviando, self.gravadas) {
            (0, 0) if quantas == 0 => "nenhuma foto no site para registrar".to_string(),
            (0, gravadas) if gravadas > 0 => format!("{gravadas} registrada(s)"),
            (0, _) if fora > 0 => {
                format!("{quantas} no site · {fora} ainda não classificada(s)")
            }
            (0, _) => format!("{quantas} foto(s)"),
            (faltam, _) => format!("gravando… faltam {faltam}"),
        }
    }
}

/// Centavos no formato que a API recebe: decimal em texto, com dois dígitos.
///
/// ⚠️ **Não é `dinheiro::formatar`**, que escreve para gente ler (`"R$ 19,90"`).
/// O site espera `"19.90"`, com ponto e sem símbolo — mandar o formatado seria
/// um `400` com a mensagem certa e a causa escondida.
fn centavos_em_decimal(centavos: i64) -> String {
    format!("{}.{:02}", centavos / 100, (centavos % 100).abs())
}

impl Render for Balcao {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tipo = self.tipo;
        let campo = |rotulo: &str, estado: &gpui::Entity<InputState>| {
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .flex_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(rotulo.to_string()),
                )
                .child(Input::new(estado).xsmall())
        };

        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .p(px(16.))
            .min_w(px(460.))
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        "O que o cliente acertou ao levar estas fotos. \
                         🔒 Não muda o preço da galeria online — é registro do que já aconteceu.",
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(4.))
                    .children(Tipo::TODOS.into_iter().map(|t| {
                        Button::new(SharedString::from(format!("balcao-tipo-{}", t.rotulo())))
                            .label(t.rotulo())
                            .xsmall()
                            .selected(tipo == t)
                            .on_click(
                                cx.listener(move |tela, _ev, _window, cx| {
                                    tela.escolher_tipo(t, cx)
                                }),
                            )
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(tipo.dica()),
            )
            // Cada tipo pede o que precisa, e só isso: um formulário com os
            // cinco campos sempre visíveis faria procurar qual deles vale.
            .when(matches!(tipo, Tipo::Desconto | Tipo::Outro), |tela| {
                tela.child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(campo("quanto entrou", &self.preco))
                        .child(campo("motivo", &self.motivo)),
                )
            })
            .when(matches!(tipo, Tipo::Cortesia), |tela| {
                tela.child(campo("motivo", &self.motivo))
            })
            .when(matches!(tipo, Tipo::Parceiro), |tela| {
                tela.child(div().flex().flex_wrap().gap(px(4.)).children(
                    PARCEIROS.into_iter().map(|p| {
                        Button::new(SharedString::from(format!("balcao-parceiro-{p}")))
                            .label(p)
                            .xsmall()
                            .selected(self.parceiro == p)
                            .on_click(cx.listener(move |tela, _ev, _window, cx| {
                                tela.escolher_parceiro(p, cx)
                            }))
                    }),
                ))
                .child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(campo("cupom", &self.cupom))
                        .child(campo("quanto entrou", &self.preco)),
                )
            })
            .when_some(self.erro.clone(), |tela, erro| {
                tela.child(div().text_xs().text_color(cx.theme().danger).child(erro))
            })
            .when(self.fora() > 0, |tela| {
                let fora = self.fora();
                tela.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().warning)
                        .child(format!(
                            "{fora} foto(s) da seleção ainda não estão no site e ficam de fora — \
                             classifique-as para elas subirem."
                        )),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.resumo()),
                    )
                    .child(
                        Button::new("balcao-registrar")
                            .label("Registrar")
                            .xsmall()
                            .primary()
                            .disabled(self.enviando > 0 || self.negociaveis().is_empty())
                            .on_click(cx.listener(|tela, _ev, _window, cx| tela.registrar(cx))),
                    ),
            )
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::pos_venda::porta::mentira::PublicadorDeMentira;
    use gpui::TestAppContext;

    fn foto(id: &str, no_site: Option<&str>) -> PhotoViewModel {
        PhotoViewModel {
            id: id.into(),
            name: format!("{id}.jpg"),
            pos_venda_foto_id: no_site.map(str::to_string),
            ..Default::default()
        }
    }

    fn janela(
        cx: &mut TestAppContext,
        publicador: Arc<PublicadorDeMentira>,
    ) -> gpui::WindowHandle<Balcao> {
        cx.update(gpui_component::init);
        cx.add_window(move |window, cx| {
            let mut tela = Balcao::nova(publicador, window, cx);
            tela.sessao = Some(Sessao {
                access_token: "tok".into(),
                refresh_token: "ref".into(),
                // Prazos folgados: o que estes testes exercem é a tela, não a
                // renovação — que tem teste próprio em `pos_venda/http.rs`.
                access_vence_em: i64::MAX,
                refresh_vence_em: i64::MAX,
            });
            tela
        })
    }

    fn colher(cx: &mut TestAppContext, janela: &gpui::WindowHandle<Balcao>) {
        for _ in 0..10 {
            let _ = janela.update(cx, |tela, _window, cx| tela.colher(cx));
            cx.run_until_parked();
        }
    }

    /// 🚨 Só entra na negociação a foto que **está no site**.
    ///
    /// A negociação se grava na linha da foto no `recordarfotos.com.br`; uma
    /// foto que nunca subiu não tem essa linha. Mandar assim mesmo traria um
    /// `404` por foto e nenhuma explicação para quem está no balcão com o
    /// cliente na frente.
    #[gpui::test]
    fn so_negocia_o_que_ja_esta_no_site(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = janela(cx, publicador.clone());

        janela
            .update(cx, |tela, _window, cx| {
                tela.abrir_para(
                    vec![
                        foto("a", Some("remota-a")),
                        foto("b", None),
                        foto("c", Some("remota-c")),
                    ],
                    cx,
                );
                assert_eq!(tela.negociaveis().len(), 2);
                assert_eq!(tela.fora(), 1, "a que não subiu fica de fora, e a tela diz");

                tela.escolher_tipo(Tipo::Cortesia, cx);
                tela.registrar(cx);
            })
            .expect("a janela deve estar aberta");
        colher(cx, &janela);

        let negociadas = publicador.negociadas();
        assert_eq!(negociadas.len(), 2, "duas, e não três");
        let ids: Vec<&str> = negociadas.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, vec!["remota-a", "remota-c"]);

        // Cortesia grava zero e o motivo, e **não** toca no preço de venda.
        let (_, mudanca) = &negociadas[0];
        assert_eq!(mudanca.preco_negociado, Some(Some("0.00".into())));
        assert_eq!(
            mudanca.observacao_da_negociacao,
            Some(Some("Cortesia".into()))
        );
        assert_eq!(mudanca.nota, None, "negociar não mexe na classificação");
        assert_eq!(mudanca.estado, None, "nem no estado do balcão");
    }

    /// ⚠️ A recusa do core aparece **sem ir ao servidor**.
    ///
    /// É a mesma `montar` que a tela do site usa: o que é recusado aqui é
    /// recusado lá, e o operador não espera uma ida à rede para descobrir que
    /// faltou o valor do desconto.
    #[gpui::test]
    fn o_que_falta_e_dito_antes_de_sair_daqui(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = janela(cx, publicador.clone());

        janela
            .update(cx, |tela, _window, cx| {
                tela.abrir_para(vec![foto("a", Some("remota-a"))], cx);
                // Desconto sem quanto entrou: o core recusa.
                tela.escolher_tipo(Tipo::Desconto, cx);
                tela.registrar(cx);
                assert!(tela.erro().is_some(), "a falta tinha de ser dita");
            })
            .expect("a janela deve estar aberta");

        assert!(
            publicador.negociadas().is_empty(),
            "nada podia ter saído daqui"
        );
    }

    /// Sem nenhuma foto no site não há o que registrar, e o botão diz isso.
    #[gpui::test]
    fn selecao_toda_fora_do_site_nao_registra_nada(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = janela(cx, publicador.clone());

        janela
            .update(cx, |tela, _window, cx| {
                tela.abrir_para(vec![foto("a", None), foto("b", None)], cx);
                assert_eq!(tela.resumo(), "nenhuma foto no site para registrar");
                tela.registrar(cx);
                assert!(tela.erro().is_some());
            })
            .expect("a janela deve estar aberta");
        assert!(publicador.negociadas().is_empty());
    }

    /// 🚨 O decimal que vai para a API é `"19.90"`, e não `"R$ 19,90"`.
    ///
    /// Mandar o formatado para gente ler daria um `400` com a mensagem certa e a
    /// causa escondida — o tipo de defeito que só aparece com o cliente na
    /// frente.
    #[test]
    fn o_preco_vai_no_formato_que_a_api_fala() {
        assert_eq!(centavos_em_decimal(0), "0.00");
        assert_eq!(centavos_em_decimal(1990), "19.90");
        assert_eq!(centavos_em_decimal(150_000), "1500.00");
        assert_eq!(centavos_em_decimal(5), "0.05");
    }
}
