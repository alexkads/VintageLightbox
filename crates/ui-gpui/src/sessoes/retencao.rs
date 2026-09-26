//! A política de retenção do pós-venda — a rota
//! `/dashboard/sessoes-fotograficas/configuracoes` do site (`TelaDeRetencao` e
//! `FormularioDeRetencao`).
//!
//! Quanto tempo cada foto fica guardada, quando o cliente é avisado e o que
//! acontece com quem não leu o aviso. Quem aplica é o cron do servidor; a tela
//! só lê e grava `GET/PUT /pos-venda/configuracao`. As faixas e as frases de
//! erro são as do site, em [`biblioteca_core::retencao`].

use crate::campo::TrocarValor as _;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use biblioteca_core::retencao::{self, Politica, Prazo};
use domain::services::pos_venda::Sessao;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon};
use gpui_kit::{
    div, prelude::*, px, Context, Entity, EventEmitter, FontWeight, SharedString, Task, Window,
};

use crate::estilo;
use crate::pos_venda::porta::{PedidoJson, Publicador, Recado};
use crate::recursos::Icone;

const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(100);
const CAMINHO: &str = "/pos-venda/configuracao";

/// O que a tela pede à raiz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PedidoDaRetencao {
    /// "← Sessões fotográficas".
    Voltar,
}

impl EventEmitter<PedidoDaRetencao> for Retencao {}

#[derive(Debug, Clone, PartialEq)]
enum Estado {
    Carregando,
    /// A busca falhou: a falha ocupa o lugar do formulário, como no site.
    NaoCarregou,
    Pronta,
}

pub struct Retencao {
    publicador: Arc<dyn Publicador>,
    sessao: Option<Sessao>,
    estado: Estado,
    campos: [Entity<InputState>; 4],
    apagar_automaticamente: bool,
    apagar_liberada_sem_download: bool,
    atualizada_em: Option<String>,
    erros: [Option<&'static str>; 4],
    /// A frase do servidor quando ele recusa (`Alert` "Não foi possível salvar").
    recusa: Option<SharedString>,
    salvando: bool,
    salvou: bool,
    recados: (Sender<Recado>, Receiver<Recado>),
    colhendo: bool,
    _colheita: Option<Task<()>>,
}

impl Retencao {
    pub fn nova(
        publicador: Arc<dyn Publicador>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let campos = std::array::from_fn(|_| cx.new(|cx| InputState::new(window, cx)));
        Self {
            publicador,
            sessao: None,
            estado: Estado::Carregando,
            campos,
            apagar_automaticamente: false,
            apagar_liberada_sem_download: false,
            atualizada_em: None,
            erros: [None; 4],
            recusa: None,
            salvando: false,
            salvou: false,
            recados: channel(),
            colhendo: false,
            _colheita: None,
        }
    }

    pub fn definir_sessao(&mut self, sessao: Sessao) {
        self.sessao = Some(sessao);
    }

    /// A tela vai aparecer: relê a configuração, como o carregador do site.
    pub fn abrir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        self.estado = Estado::Carregando;
        self.erros = [None; 4];
        self.recusa = None;
        self.salvou = false;
        self.publicador.pedir_json(
            sessao,
            PedidoJson::ler("retencao", CAMINHO),
            self.recados.0.clone(),
        );
        self.acompanhar(window, cx);
        cx.notify();
    }

    /// "Salvar retenção".
    pub fn salvar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.salvando {
            return;
        }
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        let digitado: Vec<String> = self
            .campos
            .iter()
            .map(|c| c.read(cx).value().to_string())
            .collect();
        let conferido = retencao::conferir(
            [&digitado[0], &digitado[1], &digitado[2], &digitado[3]],
            self.apagar_liberada_sem_download,
            self.apagar_automaticamente,
        );
        self.recusa = None;
        self.salvou = false;
        match conferido {
            Err(erros) => self.erros = erros,
            Ok(politica) => {
                self.erros = [None; 4];
                self.salvando = true;
                self.publicador.pedir_json(
                    sessao,
                    PedidoJson::gravar("retencao-salva", "PUT", CAMINHO, corpo(&politica)),
                    self.recados.0.clone(),
                );
                self.acompanhar(window, cx);
            }
        }
        cx.notify();
    }

    /// Acorda a cada 100 ms enquanto houver resposta a esperar.
    fn acompanhar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.colhendo {
            return;
        }
        self.colhendo = true;
        self._colheita = Some(cx.spawn_in(window, async move |tela, cx| loop {
            cx.background_executor().timer(INTERVALO_DE_COLHEITA).await;
            let continua = tela
                .update_in(cx, |tela, window, cx| tela.colher(window, cx))
                .unwrap_or(false);
            if !continua {
                break;
            }
        }));
    }

    /// Drena o canal. Devolve se vale continuar acordando.
    pub fn colher(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;
        while let Ok(recado) = self.recados.1.try_recv() {
            let Recado::Json { rotulo, resultado } = recado else {
                continue;
            };
            mudou = true;
            match (rotulo, resultado) {
                ("retencao", Ok(valor)) => {
                    self.preencher(&valor, window, cx);
                    self.estado = Estado::Pronta;
                }
                ("retencao", Err(erro)) => {
                    crate::telemetria::avisar!("⚠️ [Retenção] {erro}");
                    self.estado = Estado::NaoCarregou;
                }
                ("retencao-salva", resultado) => {
                    self.salvando = false;
                    match resultado {
                        Ok(valor) => {
                            self.preencher(&valor, window, cx);
                            self.salvou = true;
                        }
                        Err(erro) => self.recusa = Some(explicar(&erro).into()),
                    }
                }
                _ => {}
            }
        }
        if mudou {
            cx.notify();
        }
        let continua = self.estado == Estado::Carregando || self.salvando;
        if !continua {
            self.colhendo = false;
        }
        continua
    }

    fn preencher(
        &mut self,
        valor: &serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for (i, prazo) in Prazo::TODOS.iter().enumerate() {
            let numero = valor
                .get(prazo.campo())
                .and_then(|v| v.as_i64())
                .map(|n| n.to_string())
                .unwrap_or_default();
            self.campos[i].update(cx, |campo, cx| campo.trocar_valor(numero, window, cx));
        }
        let marcado = |campo: &str| valor.get(campo).and_then(|v| v.as_bool()).unwrap_or(false);
        self.apagar_automaticamente = marcado("apagar_automaticamente");
        self.apagar_liberada_sem_download = marcado("apagar_liberada_sem_download");
        self.atualizada_em = valor
            .get("atualizada_em")
            .and_then(|v| v.as_str())
            .and_then(data_e_hora_br);
    }

    #[cfg(test)]
    pub(crate) fn erros(&self) -> [Option<&'static str>; 4] {
        self.erros
    }

    #[cfg(test)]
    pub(crate) fn digitar(
        &mut self,
        valores: [&str; 4],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for (campo, valor) in self.campos.iter().zip(valores) {
            let valor = valor.to_string();
            campo.update(cx, |c, cx| c.trocar_valor(valor, window, cx));
        }
    }
}

fn corpo(p: &Politica) -> serde_json::Value {
    serde_json::json!({
        "dias_a_venda": p.dias_a_venda,
        "dias_liberadas": p.dias_liberadas,
        "dias_de_aviso": p.dias_de_aviso,
        "prorrogacao_sem_leitura_dias": p.prorrogacao_sem_leitura_dias,
        "apagar_liberada_sem_download": p.apagar_liberada_sem_download,
        "apagar_automaticamente": p.apagar_automaticamente,
    })
}

/// As frases do `explicar` do site.
fn explicar(erro: &str) -> String {
    if erro.contains("403") {
        return "Sua conta não pode alterar a retenção.".into();
    }
    // O backend responde 400 dizendo qual campo saiu da faixa.
    if let Some(resto) = erro.split("400 Bad Request: ").nth(1) {
        return resto.to_string();
    }
    "Não foi possível salvar a configuração.".into()
}

/// `2026-09-02T11:04:00Z` → `02/09/2026, 08:04`, no fuso do estúdio.
fn data_e_hora_br(iso: &str) -> Option<String> {
    let instante = chrono::DateTime::parse_from_rfc3339(iso).ok()?;
    let brasilia = chrono::FixedOffset::west_opt(3 * 3600)?;
    Some(
        instante
            .with_timezone(&brasilia)
            .format("%d/%m/%Y, %H:%M")
            .to_string(),
    )
}

impl Render for Retencao {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let (apagado, texto) = (tema.muted_foreground, tema.foreground);

        let voltar = estilo::botao_contorno("retencao-voltar", cx)
            .child(Icon::new(Icone::ArrowLeft).size(px(16.)))
            .child("Sessões fotográficas")
            .on_click(cx.listener(|_, _, _, cx| cx.emit(PedidoDaRetencao::Voltar)));

        let cabecalho = estilo::cabecalho_da_pagina(
            "Retenção do pós-venda",
            Some(
                div()
                    .child(
                        "Por quanto tempo as fotos ficam guardadas, quando o cliente é avisado, \
                         e o que acontece com quem não leu o aviso. Armazenamento custa por foto \
                         guardada — e o cliente que não voltou em três meses não vai voltar.",
                    )
                    .into_any_element(),
            ),
            None,
            Some(voltar.into_any_element()),
            cx,
        );

        let corpo = match self.estado {
            Estado::Carregando => div()
                .text_sm()
                .text_color(apagado)
                .child("Carregando…")
                .into_any_element(),
            Estado::NaoCarregou => v_flex()
                .max_w(px(672.))
                .child(estilo::aviso(
                    "Não foi possível carregar a configuração. Tente de novo; se continuar, a API pode estar fora do ar.",
                    true,
                    cx,
                ))
                .into_any_element(),
            Estado::Pronta => self.formulario(cx).into_any_element(),
        };

        v_flex()
            .id("retencao")
            .size_full()
            .overflow_y_scroll()
            .p(px(24.))
            .gap(px(24.))
            .text_color(texto)
            .child(cabecalho)
            .child(corpo)
    }
}

impl Retencao {
    fn formulario(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let (apagado, perigo) = (tema.muted_foreground, tema.danger);
        let campos = Prazo::TODOS.iter().enumerate().map(|(i, prazo)| {
            v_flex()
                .gap(px(8.))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child(prazo.rotulo()),
                )
                .child(div().w(px(128.)).child(Input::new(&self.campos[i])))
                .child(div().text_xs().text_color(apagado).child(prazo.ajuda()))
                .when_some(self.erros[i], |d, erro| {
                    d.child(div().text_sm().text_color(perigo).child(erro))
                })
        });

        let caixa = |id: &'static str,
                     marcado: bool,
                     rotulo: &'static str,
                     ajuda: &'static str,
                     alternar: fn(&mut Retencao),
                     cx: &mut Context<Self>| {
            v_flex()
                .gap(px(2.))
                .child(
                    Checkbox::new(id)
                        .checked(marcado)
                        .label(rotulo)
                        .on_click(cx.listener(move |tela, _, _, cx| {
                            alternar(tela);
                            cx.notify();
                        })),
                )
                .child(div().ml(px(24.)).text_xs().text_color(apagado).child(ajuda))
        };

        v_flex()
            .max_w(px(672.))
            .gap(px(28.))
            .when_some(self.atualizada_em.clone(), |d, quando| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(apagado)
                        .child(format!("Última alteração em {quando}.")),
                )
            })
            .when_some(self.recusa.clone(), |d, recusa| {
                d.child(estilo::aviso(
                    format!("Não foi possível salvar. {recusa}"),
                    true,
                    cx,
                ))
            })
            .children(campos)
            .child(
                v_flex()
                    .gap(px(12.))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child("O que a retenção pode apagar"),
                    )
                    .child(caixa(
                        "retencao-apagar",
                        self.apagar_automaticamente,
                        "Apagar automaticamente o que venceu",
                        "Desmarcado, o cron só avisa e nunca apaga. É a trava geral.",
                        |t| t.apagar_automaticamente = !t.apagar_automaticamente,
                        cx,
                    ))
                    .child(caixa(
                        "retencao-sem-download",
                        self.apagar_liberada_sem_download,
                        "Apagar também a foto adquirida que o cliente nunca baixou",
                        "Desmarcado (o padrão), a foto paga que nunca saiu fica guardada mesmo \
                         vencida — é o compromisso de entrega. O download é contado a cada vez \
                         que o cliente baixa o original.",
                        |t| t.apagar_liberada_sem_download = !t.apagar_liberada_sem_download,
                        cx,
                    )),
            )
            .child(
                h_flex()
                    .gap(px(12.))
                    .child(estilo::desligado(
                        estilo::botao_primario("retencao-salvar", cx)
                            .child(if self.salvando {
                                "Salvando…"
                            } else {
                                "Salvar retenção"
                            })
                            .on_click(cx.listener(|tela, _, window, cx| tela.salvar(window, cx))),
                        self.salvando,
                    ))
                    .when(self.salvou, |d| {
                        d.child(
                            div()
                                .text_sm()
                                .text_color(crate::tema::cores::sucesso())
                                .child("Retenção salva."),
                        )
                    }),
            )
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::pos_venda::porta::mentira::PublicadorDeMentira;
    use gpui_kit::TestAppContext;

    fn sessao() -> Sessao {
        Sessao {
            access_token: "tok".into(),
            refresh_token: "ref".into(),
            access_vence_em: i64::MAX,
            refresh_vence_em: i64::MAX,
        }
    }

    fn janela(
        cx: &mut TestAppContext,
        publicador: Arc<PublicadorDeMentira>,
    ) -> gpui_kit::WindowHandle<Retencao> {
        cx.update(gpui_kit::init);
        cx.add_window(move |window, cx| {
            let mut tela = Retencao::nova(publicador, window, cx);
            tela.definir_sessao(sessao());
            tela.abrir(window, cx);
            tela
        })
    }

    fn colher(cx: &mut TestAppContext, janela: &gpui_kit::WindowHandle<Retencao>) {
        for _ in 0..5 {
            let _ = janela.update(cx, |tela, window, cx| tela.colher(window, cx));
        }
    }

    #[gpui_kit::test]
    fn abre_com_a_politica_do_servidor_e_so_grava_o_que_passa(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        publicador.responder_json(
            "retencao",
            Ok(serde_json::json!({
                "dias_a_venda": 90, "dias_liberadas": 365, "dias_de_aviso": 20,
                "prorrogacao_sem_leitura_dias": 30, "apagar_liberada_sem_download": false,
                "apagar_automaticamente": true, "atualizada_em": "2026-09-02T11:04:00Z"
            })),
        );
        let janela = janela(cx, publicador.clone());
        colher(cx, &janela);
        janela
            .update(cx, |tela, _window, cx| {
                assert_eq!(tela.estado, Estado::Pronta);
                assert!(tela.apagar_automaticamente);
                assert_eq!(tela.atualizada_em.as_deref(), Some("02/09/2026, 08:04"));
                assert_eq!(tela.campos[1].read(cx).value().as_ref(), "365");
            })
            .unwrap();

        // Um prazo fora da faixa não sai daqui.
        janela
            .update(cx, |tela, window, cx| {
                tela.digitar(["90", "365", "0", "30"], window, cx);
                tela.salvar(window, cx);
                assert_eq!(tela.erros()[2], Some("Ao menos 1 dia antes"));
            })
            .unwrap();
        assert_eq!(
            publicador.pedidos_json().len(),
            1,
            "só a leitura foi pedida"
        );

        // Corrigido, vai com o corpo inteiro.
        publicador.responder_json(
            "retencao-salva",
            Ok(serde_json::json!({"dias_a_venda": 60})),
        );
        janela
            .update(cx, |tela, window, cx| {
                tela.digitar(["60", "365", "20", "30"], window, cx);
                tela.salvar(window, cx);
            })
            .unwrap();
        colher(cx, &janela);
        let pedidos = publicador.pedidos_json();
        let gravado = pedidos.last().unwrap();
        assert_eq!(gravado.metodo, "PUT");
        assert_eq!(gravado.caminho, "/pos-venda/configuracao");
        assert_eq!(gravado.corpo.as_ref().unwrap()["dias_a_venda"], 60);
        janela
            .update(cx, |tela, _window, _cx| assert!(tela.salvou))
            .unwrap();
    }

    #[gpui_kit::test]
    fn a_falha_da_leitura_ocupa_o_lugar_do_formulario(cx: &mut TestAppContext) {
        let janela = janela(cx, Arc::new(PublicadorDeMentira::default()));
        colher(cx, &janela);
        janela
            .update(cx, |tela, _window, _cx| {
                assert_eq!(tela.estado, Estado::NaoCarregou)
            })
            .unwrap();
    }

    #[test]
    fn a_data_sai_no_fuso_do_estudio() {
        assert_eq!(
            data_e_hora_br("2026-09-02T11:04:00Z").as_deref(),
            Some("02/09/2026, 08:04")
        );
        assert_eq!(data_e_hora_br("ontem"), None);
    }

    #[test]
    fn o_corpo_leva_os_seis_campos_da_api() {
        let politica = retencao::conferir(["90", "365", "20", "30"], true, false).unwrap();
        let valor = corpo(&politica);
        assert_eq!(valor["dias_a_venda"], 90);
        assert_eq!(valor["prorrogacao_sem_leitura_dias"], 30);
        assert_eq!(valor["apagar_liberada_sem_download"], true);
        assert_eq!(valor["apagar_automaticamente"], false);
    }

    #[test]
    fn a_recusa_usa_as_frases_do_site() {
        assert_eq!(
            explicar("o site respondeu 403 Forbidden: sem permissão"),
            "Sua conta não pode alterar a retenção."
        );
        assert_eq!(
            explicar("o site respondeu 400 Bad Request: dias_de_aviso fora da faixa"),
            "dias_de_aviso fora da faixa"
        );
        assert_eq!(explicar("rede"), "Não foi possível salvar a configuração.");
    }
}
