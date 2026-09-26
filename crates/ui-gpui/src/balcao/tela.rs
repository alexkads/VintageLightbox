//! O passo 6 do fluxo: **o cliente paga no balcão** — o `negociacao-dialog.tsx`
//! do site, no mesmo desenho.
//!
//! Primeiro o **tipo**, em cartões grandes: cortesia, desconto, já paga em
//! outro site, outro. O tipo decide o que falta perguntar, e só isso aparece —
//! cortesia não pede preço, parceiro pede o site (numa lista) e o cupom. Até
//! 26/set/2026 isto era uma faixa de botões miúdos com um "Registrar", e o dono
//! mandou o print da web: *"eu preciso que no vintagelightbox seja assim"*.
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

use crate::campo::TrocarValor as _;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use adapters::view_models::PhotoViewModel;
use biblioteca_core::dinheiro;
use biblioteca_core::negociacao::{self, Gravavel, Negociacao, Tipo, PARCEIROS};
use domain::services::pos_venda::{MudancaDaFoto, Sessao};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon};
use gpui_kit::{
    div, prelude::*, px, ClickEvent, Context, Entity, EventEmitter, FocusHandle, Focusable,
    FontWeight, SharedString, Subscription, Task, Window,
};

use crate::estilo;
use crate::pos_venda::porta::{Publicador, Recado};
use crate::recursos::Icone;

const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(100);

gpui_kit::actions!(balcao, [SalvarNegociacao, FecharNegociacao]);

/// O contexto de teclado do diálogo.
const CONTEXTO: &str = "Negociacao";

/// ⏎ é o "Salvar" (o `<form onSubmit>` do site) e Esc fecha.
///
/// 🚨 **O Enter tem de ser consumido por uma ação**, e não só ouvido pelo
/// `PressEnter` do campo: o `InputState` de uma linha emite o evento e
/// **propaga** a tecla, e sem ninguém que a pare ela volta ao campo como texto.
/// O `\n` dentro de um campo de uma linha derrubava o app no desenho seguinte
/// (`shape_line`, achado pelo roteiro em 26/set/2026). É o mesmo arranjo do
/// `ConfirmarDialogo` do caixa. Ligação repetida é inofensiva.
pub fn init(cx: &mut gpui_kit::App) {
    cx.bind_keys([
        gpui_kit::KeyBinding::new("enter", SalvarNegociacao, Some(CONTEXTO)),
        gpui_kit::KeyBinding::new("escape", FecharNegociacao, Some(CONTEXTO)),
    ]);
}

/// Com o que o diálogo abre — as quatro coisas que o `NegociacaoDialog` do site
/// recebe além das fotos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abertura {
    /// "Negociação desta foto", ou "Negociação de 3 fotos".
    pub titulo: String,
    /// O que já está gravado, lido de volta; o vazio quando não há nada.
    pub inicial: Negociacao,
    /// O preço da faixa, ao lado do valor. `None` em lote de faixas misturadas.
    pub preco_da_faixa: Option<i64>,
    /// Já há negociação gravada — mostra "Remover negociação".
    pub existente: bool,
}

impl Abertura {
    /// A de várias fotos: começa vazia, como o lote do site (`grade.tsx`).
    pub fn do_lote(quantas: usize) -> Self {
        Self {
            titulo: format!(
                "Negociação de {quantas} foto{}",
                if quantas > 1 { "s" } else { "" }
            ),
            inicial: Negociacao::default(),
            preco_da_faixa: None,
            existente: false,
        }
    }

    /// A de uma foto: já preenchida com o que está gravado (`foto-acoes.tsx`).
    pub fn da_foto(
        preco_negociado: Option<i64>,
        observacao: Option<&str>,
        preco_da_faixa: Option<i64>,
    ) -> Self {
        let existente =
            preco_negociado.is_some() || observacao.is_some_and(|o| !o.trim().is_empty());
        Self {
            titulo: "Negociação desta foto".into(),
            inicial: if existente {
                negociacao::interpretar(preco_negociado, observacao)
            } else {
                Negociacao::default()
            },
            preco_da_faixa,
            existente,
        }
    }
}

/// O que o diálogo pede a quem o mostra.
pub enum Evento {
    /// Fechar. `gravou` = o site confirmou tudo, e a grade precisa reler.
    Fechar { gravou: bool },
}

impl EventEmitter<Evento> for Balcao {}

pub struct Balcao {
    publicador: Arc<dyn Publicador>,
    sessao: Option<Sessao>,
    /// Os ids **do site** das fotos que recebem a negociação.
    alvos: Vec<String>,
    /// As da seleção que não estão no site, e por isso ficam de fora.
    fora: usize,
    titulo: String,
    preco_da_faixa: Option<i64>,
    existente: bool,
    tipo: Tipo,
    parceiro: String,
    lista_de_parceiros: Entity<SelectState<Vec<&'static str>>>,
    preco: Entity<InputState>,
    cupom: Entity<InputState>,
    motivo: Entity<InputState>,
    /// "Remover a negociação?" — o `useConfirmacao` do site, por cima do
    /// diálogo.
    confirmando: bool,
    /// O campo que ganha o cursor no próximo desenho.
    foco_pendente: Option<FocusHandle>,
    enviando: usize,
    /// Quantas o site já confirmou nesta rodada.
    gravadas: usize,
    /// Quantas o site recusou nesta rodada.
    recusadas: usize,
    erro: Option<SharedString>,
    recados: (Sender<Recado>, Receiver<Recado>),
    colhendo: bool,
    _colheita: Option<Task<()>>,
    _assinaturas: Vec<Subscription>,
}

impl Balcao {
    pub fn nova(
        publicador: Arc<dyn Publicador>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let preco = cx.new(|cx| InputState::new(window, cx).placeholder("0,00"));
        let cupom =
            cx.new(|cx| InputState::new(window, cx).placeholder("o código que o cliente trouxe"));
        let motivo =
            cx.new(|cx| InputState::new(window, cx).placeholder("aniversário, indicação, brinde…"));
        let lista_de_parceiros =
            cx.new(|cx| SelectState::new(PARCEIROS.to_vec(), None, window, cx));

        init(cx);
        let assinaturas = vec![cx.subscribe_in(
            &lista_de_parceiros,
            window,
            |tela, _, evento: &SelectEvent<Vec<&'static str>>, _, cx| {
                let SelectEvent::Confirm(Some(parceiro)) = evento else {
                    return;
                };
                tela.escolher_parceiro(parceiro, cx);
            },
        )];

        Self {
            publicador,
            sessao: None,
            alvos: Vec::new(),
            fora: 0,
            titulo: String::new(),
            preco_da_faixa: None,
            existente: false,
            tipo: Tipo::Cortesia,
            parceiro: PARCEIROS[0].to_string(),
            lista_de_parceiros,
            preco,
            cupom,
            motivo,
            confirmando: false,
            foco_pendente: None,
            enviando: 0,
            gravadas: 0,
            recusadas: 0,
            erro: None,
            recados: channel(),
            colhendo: false,
            _colheita: None,
            _assinaturas: assinaturas,
        }
    }

    pub fn definir_sessao(&mut self, sessao: Sessao, cx: &mut Context<Self>) {
        self.sessao = Some(sessao);
        cx.notify();
    }

    /// Abre para uma seleção da Biblioteca: entra só a que está no site.
    pub fn abrir_para(
        &mut self,
        fotos: Vec<PhotoViewModel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let alvos: Vec<String> = fotos
            .iter()
            .filter_map(|f| f.pos_venda_foto_id.clone())
            .collect();
        let fora = fotos.len() - alvos.len();
        let abertura = Abertura::do_lote(alvos.len());
        self.abrir(alvos, fora, abertura, window, cx);
    }

    /// Abre para fotos do site, pelo id de lá — o gesto da sessão.
    pub fn abrir(
        &mut self,
        alvos: Vec<String>,
        fora: usize,
        abertura: Abertura,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Abertura {
            titulo,
            inicial,
            preco_da_faixa,
            existente,
        } = abertura;
        self.alvos = alvos;
        self.fora = fora;
        self.titulo = titulo;
        self.preco_da_faixa = preco_da_faixa;
        self.existente = existente;
        self.confirmando = false;
        self.erro = None;
        self.gravadas = 0;
        self.recusadas = 0;

        let preco = match (inicial.preco, inicial.tipo) {
            (_, Tipo::Cortesia) | (None, _) => String::new(),
            (Some(p), _) => dinheiro::formatar_campo(p),
        };
        self.preco
            .update(cx, |c, cx| c.trocar_valor(preco, window, cx));
        self.cupom.update(cx, |c, cx| {
            c.trocar_valor(inicial.cupom.clone(), window, cx)
        });
        self.motivo.update(cx, |c, cx| {
            c.trocar_valor(inicial.motivo.clone(), window, cx)
        });
        let parceiro = PARCEIROS
            .into_iter()
            .find(|p| *p == inicial.parceiro)
            .unwrap_or(PARCEIROS[0]);
        self.parceiro = parceiro.to_string();
        self.lista_de_parceiros
            .update(cx, |l, cx| l.set_selected_value(&parceiro, window, cx));
        self.mudar_tipo(inicial.tipo, window, cx);
    }

    /// Os ids do site que recebem a negociação.
    pub fn negociaveis(&self) -> &[String] {
        &self.alvos
    }

    /// As que não têm linha no site, e por isso ficam de fora.
    pub fn fora(&self) -> usize {
        self.fora
    }

    pub fn titulo(&self) -> &str {
        &self.titulo
    }

    pub fn existente(&self) -> bool {
        self.existente
    }

    pub fn escolher_tipo(&mut self, tipo: Tipo, cx: &mut Context<Self>) {
        self.tipo = tipo;
        self.erro = None;
        cx.notify();
    }

    /// Troca o tipo e leva o cursor ao campo que ele pede — o `autoFocus` do
    /// site, e o marcador do motivo que muda com o tipo.
    pub fn mudar_tipo(&mut self, tipo: Tipo, window: &mut Window, cx: &mut Context<Self>) {
        self.escolher_tipo(tipo, cx);
        let dica = match tipo {
            Tipo::Cortesia => "aniversário, indicação, brinde…",
            Tipo::Outro => "ex.: troca por indicação de cliente",
            _ => "opcional",
        };
        self.motivo
            .update(cx, |c, cx| c.set_placeholder(dica, window, cx));
        let campo = match tipo {
            Tipo::Desconto => &self.preco,
            Tipo::Parceiro => &self.cupom,
            _ => &self.motivo,
        };
        // 🔑 O foco vai no próximo desenho, e não agora: o campo de texto do
        // `gpui-component` procura o `Root` da janela ao ganhar o foco — como
        // o `foco_pendente` do caixa.
        self.foco_pendente = Some(campo.read(cx).focus_handle(cx));
        cx.notify();
    }

    pub fn escolher_parceiro(&mut self, parceiro: &str, cx: &mut Context<Self>) {
        self.parceiro = parceiro.to_string();
        cx.notify();
    }

    /// 🧪 O cupom, como se digitado — para os e2e.
    #[cfg(test)]
    pub fn escrever_cupom(&mut self, cupom: &str, window: &mut Window, cx: &mut Context<Self>) {
        let cupom = cupom.to_string();
        self.cupom
            .update(cx, |c, cx| c.trocar_valor(cupom, window, cx));
    }

    pub fn tipo(&self) -> Tipo {
        self.tipo
    }

    pub fn parceiro(&self) -> &str {
        &self.parceiro
    }

    /// "Salvar": grava a negociação montada, ou a remoção se ela estiver
    /// sendo confirmada.
    pub fn salvar(&mut self, cx: &mut Context<Self>) {
        if self.confirmando {
            self.remover(cx);
        } else {
            self.registrar(cx);
        }
    }

    /// Grava a negociação nas fotos.
    ///
    /// 🔑 **O core decide se falta alguma coisa**, e a recusa aparece aqui sem
    /// ida ao servidor: é a mesma `montar` que a tela do site usa, então o que é
    /// recusado aqui é recusado lá.
    pub fn registrar(&mut self, cx: &mut Context<Self>) {
        if self.enviando > 0 {
            return;
        }
        let texto = self.preco.read(cx).value().to_string();
        let mut preco = None;
        if self.tipo != Tipo::Cortesia && !texto.trim().is_empty() {
            match dinheiro::ler_campo(&texto) {
                Some(p) => preco = Some(p),
                None => {
                    self.erro = Some("Valor inválido. Use o formato 15,00.".into());
                    cx.notify();
                    return;
                }
            }
        }
        let n = Negociacao {
            tipo: self.tipo,
            preco: if self.tipo == Tipo::Cortesia {
                Some(0)
            } else {
                preco
            },
            parceiro: self.parceiro.clone(),
            cupom: self.cupom.read(cx).value().to_string(),
            motivo: self.motivo.read(cx).value().to_string(),
        };
        match negociacao::montar(&n) {
            Ok(g) => self.gravar(Some(g), cx),
            Err(falta) => {
                self.erro = Some(falta.into());
                cx.notify();
            }
        }
    }

    /// "Remover negociação" pede a confirmação, dentro do diálogo.
    pub fn pedir_remocao(&mut self, cx: &mut Context<Self>) {
        if self.enviando == 0 {
            self.confirmando = true;
            cx.notify();
        }
    }

    /// Apaga o registro: os dois campos voltam a vazio.
    pub fn remover(&mut self, cx: &mut Context<Self>) {
        self.confirmando = false;
        self.gravar(None, cx);
    }

    /// `None` remove — o `salvar(null)` do site.
    fn gravar(&mut self, valor: Option<Gravavel>, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            self.erro = Some("esta ação precisa da conta do site".into());
            cx.notify();
            return;
        };
        if self.enviando > 0 {
            return;
        }
        if self.alvos.is_empty() {
            self.erro =
                Some("nenhuma das fotos escolhidas está no site — classifique-as primeiro".into());
            cx.notify();
            return;
        }

        // 🔑 O decimal em texto é o que a API fala, e a conversão é a mesma que
        // o campo de preço usa ao ler. `None` continua `None`: "voltar ao preço
        // da faixa" é diferente de "não mexer".
        let (preco_negociado, observacao) = match valor {
            Some(g) => (g.preco_negociado.map(centavos_em_decimal), g.observacao),
            None => (None, None),
        };
        let mudanca = MudancaDaFoto {
            preco_negociado: Some(preco_negociado),
            observacao_da_negociacao: Some(observacao),
            ..MudancaDaFoto::default()
        };

        self.erro = None;
        self.gravadas = 0;
        self.recusadas = 0;
        self.enviando = self.alvos.len();
        for foto_id in &self.alvos {
            self.publicador.negociar(
                sessao.clone(),
                foto_id.clone(),
                mudanca.clone(),
                self.recados.0.clone(),
            );
        }
        self.acompanhar(cx);
        cx.notify();
    }

    /// A pergunta de remover é filha do diálogo, como no site: `Esc` fecha só
    /// ela e devolve ao formulário aberto.
    fn fechar(&mut self, cx: &mut Context<Self>) {
        if self.confirmando {
            self.confirmando = false;
            cx.notify();
            return;
        }
        if self.enviando == 0 {
            cx.emit(Evento::Fechar { gravou: false });
        }
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

    /// Recolhe as respostas do site. Quando a última chega sem recusa, o
    /// diálogo fecha — como o do site, que fecha quando `salvar` dá certo.
    pub fn colher(&mut self, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;
        while let Ok(recado) = self.recados.1.try_recv() {
            mudou = true;
            match recado.sem_o_id() {
                Recado::Sincronizou => {
                    self.gravadas += 1;
                    self.enviando = self.enviando.saturating_sub(1);
                }
                Recado::Falhou(erro) => {
                    self.recusadas += 1;
                    self.enviando = self.enviando.saturating_sub(1);
                    self.erro = Some(erro.into());
                }
                _ => {}
            }
        }
        let continua = self.enviando > 0;
        if mudou && !continua {
            if self.recusadas == 0 {
                cx.emit(Evento::Fechar { gravou: true });
            } else if self.gravadas > 0 {
                // Parte foi: a grade relê, e o diálogo fica para dizer o resto.
                let erro = self.erro.clone().unwrap_or_default();
                self.erro = Some(format!("{} não mudaram: {erro}", self.recusadas).into());
            }
        }
        if mudou {
            cx.notify();
        }
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
}

/// Centavos no formato que a API recebe: decimal em texto, com dois dígitos.
///
/// ⚠️ **Não é `dinheiro::formatar`**, que escreve para gente ler (`"R$ 19,90"`).
/// O site espera `"19.90"`, com ponto e sem símbolo — mandar o formatado seria
/// um `400` com a mensagem certa e a causa escondida.
fn centavos_em_decimal(centavos: i64) -> String {
    format!("{}.{:02}", centavos / 100, (centavos % 100).abs())
}

/// `Label` em cima, campo embaixo (`grid gap-1.5`).
fn rotulado(rotulo: &str, campo: impl IntoElement) -> gpui_kit::Div {
    v_flex()
        .gap(px(6.))
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .child(rotulo.to_string()),
        )
        .child(campo)
}

impl Render for Balcao {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // ⚠️ Campo com foco, ao se desenhar, pede o `Root` da janela — que o
        // app sempre tem (`main.rs`) e as janelas de teste do balcão e do
        // `fluxo.rs` não. Sem ele o cursor só não vai sozinho ao campo.
        if let Some(foco) = self.foco_pendente.take() {
            if window
                .root::<gpui_kit::component::Root>()
                .flatten()
                .is_some()
            {
                window.focus(&foco, cx);
            }
        }
        let tema = cx.theme();
        let (primaria, borda, acento, realce, apagado, perigo, aviso) = (
            tema.primary,
            tema.border,
            tema.accent,
            tema.muted,
            tema.muted_foreground,
            tema.danger,
            tema.warning,
        );
        let tipo = self.tipo;
        let enviando = self.enviando > 0;

        let tipos = div()
            .grid()
            .grid_cols(2)
            .gap(px(8.))
            .children(Tipo::TODOS.into_iter().map(|t| {
                let ativo = tipo == t;
                // `rounded-lg border p-3 hover:bg-muted`; o escolhido ganha
                // `border-primary bg-primary/5 ring-1 ring-primary` — a borda de
                // 2 px, com 1 px a menos de respiro para o texto não pular.
                v_flex()
                    .id(SharedString::from(format!("balcao-tipo-{}", t.rotulo())))
                    .gap(px(2.))
                    .rounded(px(10.))
                    .cursor_pointer()
                    .map(|d| {
                        if ativo {
                            d.p(px(11.))
                                .border_2()
                                .border_color(primaria)
                                .bg(primaria.opacity(0.05))
                        } else {
                            d.p(px(12.))
                                .border_1()
                                .border_color(borda)
                                .hover(move |s| s.bg(realce))
                        }
                    })
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child(t.rotulo()),
                    )
                    .child(div().text_xs().text_color(apagado).child(t.dica()))
                    .on_click(cx.listener(move |tela, _: &ClickEvent, window, cx| {
                        tela.mudar_tipo(t, window, cx)
                    }))
            }));

        let parceiro = (tipo == Tipo::Parceiro).then(|| {
            div()
                .grid()
                .grid_cols(2)
                .gap(px(12.))
                .child(rotulado(
                    "Site",
                    div()
                        .debug_selector(|| "balcao-site".into())
                        .child(Select::new(&self.lista_de_parceiros)),
                ))
                .child(rotulado("Cupom", Input::new(&self.cupom)))
        });

        let preco = (tipo != Tipo::Cortesia).then(|| {
            let rotulo = match tipo {
                Tipo::Desconto => "Quanto foi cobrado",
                Tipo::Parceiro => "Quanto pagou lá (se souber)",
                _ => "Valor cobrado (se houver)",
            };
            rotulado(
                rotulo,
                h_flex()
                    .gap(px(8.))
                    .child(div().text_sm().text_color(apagado).child("R$"))
                    .child(div().w(px(128.)).child(Input::new(&self.preco)))
                    .children(self.preco_da_faixa.map(|p| {
                        div()
                            .text_xs()
                            .text_color(apagado)
                            .child(format!("preço da faixa: {}", dinheiro::formatar(p)))
                    })),
            )
        });

        let motivo = rotulado(
            if tipo == Tipo::Outro {
                "O que foi combinado"
            } else {
                "Motivo (opcional)"
            },
            Input::new(&self.motivo),
        );

        let fora = (self.fora > 0).then(|| {
            div().text_xs().text_color(aviso).child(format!(
                "{} foto(s) da seleção ainda não estão no site e ficam de fora — \
                 classifique-as para elas subirem.",
                self.fora
            ))
        });

        // "Remover a negociação?" — o `AlertDialog` do `useConfirmacao`, por
        // cima do diálogo: um segundo `Dialog` do gpui-kit, de 448 px, com o
        // rodapé cinza de ponta a ponta. O clique fora não fecha nada, como
        // no site; o `Esc` e o ⏎ são os do formulário de baixo (`fechar` e
        // `salvar` conferem `confirmando`).
        let pergunta = self.confirmando.then(|| {
            v_flex()
                .id("balcao-pergunta")
                .gap(px(6.))
                .child(
                    div()
                        .text_size(px(16.))
                        .font_weight(FontWeight::MEDIUM)
                        .child("Remover a negociação?"),
                )
                .child(div().text_sm().text_color(apagado).child(
                    "O registro do que foi combinado no balcão — tipo, valor e motivo — é apagado.",
                ))
                .into_any_element()
        });
        let rodape_da_pergunta = self.confirmando.then(|| {
            estilo::rodape_do_dialogo()
                .w_full()
                .p(px(16.))
                .border_t_1()
                .border_color(borda)
                .bg(realce.opacity(0.5))
                .rounded_b(px(14.))
                .child(
                    estilo::botao_contorno("balcao-nao-remover", cx)
                        .child("Cancelar")
                        .on_click(cx.listener(|tela, _: &ClickEvent, _, cx| {
                            tela.confirmando = false;
                            cx.notify();
                        })),
                )
                .child(
                    estilo::botao_perigo("balcao-remover-sim", cx)
                        .child("Remover")
                        .on_click(cx.listener(|tela, _: &ClickEvent, _, cx| tela.remover(cx))),
                )
                .into_any_element()
        });

        let botoes = h_flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .pt(px(4.))
            .child(
                estilo::desligado(estilo::botao_primario("balcao-registrar", cx), enviando)
                    .debug_selector(|| "balcao-registrar".into())
                    .child(if enviando { "Salvando…" } else { "Salvar" })
                    .on_click(cx.listener(|tela, _: &ClickEvent, _, cx| {
                        tela.confirmando = false;
                        tela.registrar(cx)
                    })),
            )
            .child(
                estilo::desligado(estilo::botao_fantasma("balcao-cancelar", cx), enviando)
                    .debug_selector(|| "balcao-cancelar".into())
                    .child("Cancelar")
                    .on_click(cx.listener(|tela, _: &ClickEvent, _, cx| tela.fechar(cx))),
            )
            .when(self.existente, |d| {
                d.child(div().flex_1()).child(
                    estilo::desligado(estilo::botao_fantasma("balcao-remover", cx), enviando)
                        .text_color(perigo)
                        .child("Remover negociação")
                        .on_click(
                            cx.listener(|tela, _: &ClickEvent, _, cx| tela.pedir_remocao(cx)),
                        ),
                )
            });

        // 🪟 O diálogo é o `Dialog` do gpui-kit (`crate::dialogo`): véu, caixa e
        // canto (`rounded-xl`) são dele. O `DialogHeader` do site — título em
        // `text-base font-medium`, 6 px até a descrição — e o X são daqui.
        // Com a pergunta à vista, o clique fora não fecha o de baixo.
        let formulario = estilo::conteudo_do_dialogo()
            .id("balcao-dialogo")
            .debug_selector(|| "balcao-dialogo".into())
            .relative()
            .key_context(CONTEXTO)
            .on_action(cx.listener(|tela, _: &SalvarNegociacao, _, cx| tela.salvar(cx)))
            .on_action(cx.listener(|tela, _: &FecharNegociacao, _, cx| tela.fechar(cx)))
            .child(
                v_flex()
                    .pr(px(24.))
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(self.titulo.clone()),
                    )
                    .child(div().text_sm().text_color(apagado).child(
                        "O que foi combinado no balcão. Não muda o preço da compra online.",
                    )),
            )
            .child(tipos)
            .children(parceiro)
            .children(preco)
            .child(motivo)
            .children(fora)
            .children(
                self.erro
                    .clone()
                    .map(|e| div().text_sm().text_color(perigo).child(e)),
            )
            .child(botoes)
            .child(
                div()
                    .id("balcao-fechar")
                    .debug_selector(|| "balcao-fechar".into())
                    .absolute()
                    .top(px(16.))
                    .right(px(16.))
                    .size(px(20.))
                    .rounded(px(4.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .opacity(0.7)
                    .cursor_pointer()
                    .hover(move |s| s.opacity(1.).bg(acento))
                    .child(Icon::new(Icone::X).size(px(16.)))
                    .on_click(cx.listener(|tela, _: &ClickEvent, _, cx| tela.fechar(cx))),
            )
            .into_any_element();
        let confirmando = self.confirmando;
        let dialogo = crate::dialogo::desenhar_conteudo(
            Some(formulario),
            None,
            crate::dialogo::Jeito {
                largura: 512.,
                esc: true,
                veu: !confirmando,
                x: false,
            },
            |tela, _, cx| tela.fechar(cx),
            window,
            cx,
        );
        let pergunta = crate::dialogo::desenhar_conteudo(
            pergunta,
            rodape_da_pergunta,
            crate::dialogo::Jeito::alerta(448.),
            |tela, _, cx| {
                tela.confirmando = false;
                cx.notify();
            },
            window,
            cx,
        );
        div().children(dialogo).children(pergunta)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::pos_venda::porta::mentira::PublicadorDeMentira;
    use gpui_kit::TestAppContext;

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
    ) -> gpui_kit::WindowHandle<Balcao> {
        cx.update(gpui_kit::init);
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

    fn colher(cx: &mut TestAppContext, janela: &gpui_kit::WindowHandle<Balcao>) {
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
    #[gpui_kit::test]
    fn so_negocia_o_que_ja_esta_no_site(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = janela(cx, publicador.clone());

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir_para(
                    vec![
                        foto("a", Some("remota-a")),
                        foto("b", None),
                        foto("c", Some("remota-c")),
                    ],
                    window,
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
    #[gpui_kit::test]
    fn o_que_falta_e_dito_antes_de_sair_daqui(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = janela(cx, publicador.clone());

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir_para(vec![foto("a", Some("remota-a"))], window, cx);
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

    /// Sem nenhuma foto no site não há o que registrar, e a tela diz isso.
    #[gpui_kit::test]
    fn selecao_toda_fora_do_site_nao_registra_nada(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = janela(cx, publicador.clone());

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir_para(vec![foto("a", None), foto("b", None)], window, cx);
                assert!(tela.negociaveis().is_empty());
                assert_eq!(tela.fora(), 2);
                tela.registrar(cx);
                assert!(tela.erro().is_some());
            })
            .expect("a janela deve estar aberta");
        assert!(publicador.negociadas().is_empty());
    }

    /// 🖼️ **Uma foto abre como a web abre**: "Negociação desta foto", já no
    /// tipo gravado, com o site, o cupom e o preço da faixa — o print do dono
    /// (26/set/2026), "Já paga em outro site", LançadorDeOfertas, AHEB82.
    #[gpui_kit::test]
    fn uma_foto_abre_com_o_que_esta_gravado(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = janela(cx, publicador.clone());

        janela
            .update(cx, |tela, window, cx| {
                let abertura =
                    Abertura::da_foto(None, Some("LançadorDeOfertas — cupom AHEB82"), Some(4000));
                assert!(abertura.existente);
                tela.abrir(vec!["remota-a".into()], 0, abertura, window, cx);

                assert_eq!(tela.titulo(), "Negociação desta foto");
                assert_eq!(tela.tipo(), Tipo::Parceiro);
                assert_eq!(tela.parceiro(), "LançadorDeOfertas");
                assert_eq!(tela.cupom.read(cx).value(), "AHEB82");
                assert!(tela.existente(), "com negociação, o \"Remover\" aparece");
            })
            .expect("a janela deve estar aberta");

        // E o lote começa vazio, com o título que conta.
        let lote = Abertura::do_lote(3);
        assert_eq!(lote.titulo, "Negociação de 3 fotos");
        assert_eq!(lote.inicial, Negociacao::default());
        assert_eq!(Abertura::do_lote(1).titulo, "Negociação de 1 foto");
        assert!(!Abertura::da_foto(None, Some("  "), None).existente);
    }

    /// 🗑️ "Remover negociação" pergunta antes, e a remoção grava os **dois
    /// campos vazios** — o `salvar(null)` do site, que não é "não mexer".
    #[gpui_kit::test]
    fn remover_pergunta_e_grava_vazio(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = janela(cx, publicador.clone());

        janela
            .update(cx, |tela, window, cx| {
                let abertura = Abertura::da_foto(Some(0), Some("Cortesia"), None);
                tela.abrir(vec!["remota-a".into()], 0, abertura, window, cx);
                tela.pedir_remocao(cx);
                assert!(publicador.negociadas().is_empty(), "só perguntou");
                // Esc fecha só a pergunta: o diálogo segue aberto, sem gravar.
                tela.fechar(cx);
                assert!(!tela.confirmando);
                tela.pedir_remocao(cx);
                // ⏎ com a pergunta à vista confirma a remoção.
                tela.salvar(cx);
            })
            .expect("a janela deve estar aberta");
        colher(cx, &janela);

        let negociadas = publicador.negociadas();
        assert_eq!(negociadas.len(), 1);
        assert_eq!(negociadas[0].1.preco_negociado, Some(None));
        assert_eq!(negociadas[0].1.observacao_da_negociacao, Some(None));
    }

    /// ✅ Quando o site confirma tudo, o diálogo pede para fechar — como o do
    /// site, que fecha quando `salvar` dá certo — e avisa que gravou, para a
    /// grade reler.
    #[gpui_kit::test]
    fn fecha_quando_o_site_confirma(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = janela(cx, publicador.clone());
        let fechou = Arc::new(std::sync::Mutex::new(None));

        janela
            .update(cx, |tela, window, cx| {
                let fechou = fechou.clone();
                cx.subscribe_in(&cx.entity(), window, move |_, _, ev: &Evento, _, _| {
                    let Evento::Fechar { gravou } = ev;
                    *fechou.lock().unwrap() = Some(*gravou);
                })
                .detach();
                tela.abrir(vec!["remota-a".into()], 0, Abertura::do_lote(1), window, cx);
                tela.registrar(cx);
            })
            .expect("a janela deve estar aberta");
        colher(cx, &janela);

        assert_eq!(*fechou.lock().unwrap(), Some(true));
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
