//! O Google Authenticator da conta, nas Configurações (aba Conta).
//!
//! # Por que existe
//!
//! Dono, 2026-10-18: além do navegador, o operador entra no app com e-mail e
//! senha, com um link no e-mail **ou com os seis dígitos do Authenticator do
//! celular**. Esta é a tela de cadastrar o celular — a mesma de
//! `/dashboard/autenticador` no site (as duas interfaces cadastram; entrar com
//! o código é só a tela de entrada). Fala com o site pela porta JSON
//! (`/auth/autenticador`), sem regra própria: a regra mora no backend
//! (`application::auth::autenticador`).
//!
//! Os três estados são os do site: **desligado** → **cadastrando** (o QR e o
//! primeiro código) → **ligado** (os códigos de recuperação, uma vez só, e o
//! "desligar" com um código).

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use domain::services::pos_venda::Sessao;
use gpui_kit::component::input::{InputState, OtpEvent, OtpState};
use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    div, prelude::*, px, Context, Entity, RenderImage, SharedString, Subscription, Task, Window,
};

use crate::pos_venda::porta::{PedidoJson, Publicador, Recado};

const LER: &str = "autenticador:ler";
const INICIAR: &str = "autenticador:iniciar";
const CONFIRMAR: &str = "autenticador:confirmar";
const DESLIGAR: &str = "autenticador:desligar";

/// Em que pé está o cadastro.
#[derive(Clone)]
pub enum Situacao {
    Lendo,
    Desligado,
    Cadastrando {
        qr: Arc<RenderImage>,
        segredo: String,
    },
    Ligado {
        desde: Option<String>,
        restantes: u64,
    },
    /// Acabou de ligar: os códigos de recuperação, a única vez em que existem.
    RecemLigado {
        codigos: Vec<String>,
    },
    /// O site não respondeu ou o Authenticator está desligado lá.
    Indisponivel(String),
}

pub struct AutenticadorDaConta {
    publicador: Arc<dyn Publicador>,
    sessao: Option<Sessao>,
    situacao: Situacao,
    codigo: Entity<OtpState>,
    para_desligar: Entity<InputState>,
    ocupado: bool,
    erro: Option<SharedString>,
    recados: (Sender<Recado>, Receiver<Recado>),
    _colheita: Option<Task<()>>,
    _assinaturas: Vec<Subscription>,
}

impl AutenticadorDaConta {
    pub fn nova(
        publicador: Arc<dyn Publicador>,
        sessao: Option<Sessao>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let codigo = cx.new(|cx| OtpState::new(6, window, cx));
        let para_desligar = cx.new(|cx| {
            InputState::new(window, cx).placeholder("código do celular ou de recuperação")
        });
        let assinaturas =
            vec![
                cx.subscribe_in(&codigo, window, |tela, _, evento: &OtpEvent, window, cx| {
                    if matches!(evento, OtpEvent::Complete) {
                        tela.confirmar(window, cx);
                    }
                }),
            ];
        let mut tela = Self {
            publicador,
            sessao,
            situacao: Situacao::Lendo,
            codigo,
            para_desligar,
            ocupado: false,
            erro: None,
            recados: channel(),
            _colheita: None,
            _assinaturas: assinaturas,
        };
        tela.reler(cx);
        tela
    }

    /// A sessão mudou (outra conta entrou): relê do zero.
    pub fn trocar_sessao(&mut self, sessao: Option<Sessao>, cx: &mut Context<Self>) {
        self.sessao = sessao;
        self.situacao = Situacao::Lendo;
        self.erro = None;
        self.reler(cx);
    }

    pub fn situacao(&self) -> &Situacao {
        &self.situacao
    }

    pub fn erro(&self) -> Option<&SharedString> {
        self.erro.as_ref()
    }

    fn pedir(&mut self, pedido: PedidoJson, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            self.situacao =
                Situacao::Indisponivel("Entre na conta para cadastrar o celular.".into());
            cx.notify();
            return;
        };
        self.ocupado = true;
        self.publicador
            .pedir_json(sessao, pedido, self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    pub fn reler(&mut self, cx: &mut Context<Self>) {
        self.pedir(PedidoJson::ler(LER, "/auth/autenticador"), cx);
    }

    pub fn iniciar(&mut self, cx: &mut Context<Self>) {
        if self.ocupado {
            return;
        }
        self.erro = None;
        self.pedir(
            PedidoJson::gravar(INICIAR, "POST", "/auth/autenticador", serde_json::json!({})),
            cx,
        );
    }

    pub fn confirmar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let codigo = self.codigo.read(cx).value().to_string();
        if self.ocupado || codigo.len() != 6 {
            return;
        }
        self.erro = None;
        self.codigo.update(cx, |c, cx| c.set_value("", window, cx));
        self.pedir(
            PedidoJson::gravar(
                CONFIRMAR,
                "POST",
                "/auth/autenticador/confirmar",
                serde_json::json!({ "codigo": codigo }),
            ),
            cx,
        );
    }

    pub fn desligar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let codigo = self.para_desligar.read(cx).value().trim().to_string();
        if self.ocupado || codigo.is_empty() {
            return;
        }
        self.erro = None;
        self.para_desligar
            .update(cx, |c, cx| c.set_value("", window, cx));
        self.pedir(
            PedidoJson::gravar(
                DESLIGAR,
                "POST",
                "/auth/autenticador/desligar",
                serde_json::json!({ "codigo": codigo }),
            ),
            cx,
        );
    }

    pub fn cancelar_cadastro(&mut self, cx: &mut Context<Self>) {
        self.situacao = Situacao::Desligado;
        self.erro = None;
        cx.notify();
    }

    fn acompanhar(&mut self, cx: &mut Context<Self>) {
        if self._colheita.is_some() {
            return;
        }
        self._colheita = Some(cx.spawn(async move |esta, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let Ok(continua) = esta.update(cx, |tela, cx| tela.colher(cx)) else {
                break;
            };
            if !continua {
                break;
            }
        }));
    }

    /// Drena o canal. Devolve se vale continuar acordando.
    pub fn colher(&mut self, cx: &mut Context<Self>) -> bool {
        while let Ok(recado) = self.recados.1.try_recv() {
            let Recado::Json { rotulo, resultado } = recado else {
                continue;
            };
            self.ocupado = false;
            match (rotulo, resultado) {
                (LER, Ok(v)) => self.situacao = situacao_lida(&v),
                (LER, Err(e)) => self.situacao = Situacao::Indisponivel(legivel(&e)),
                (INICIAR, Ok(v)) => match cadastro(&v) {
                    Some(c) => self.situacao = c,
                    None => self.erro = Some("O site mandou um QR ilegível. Tente de novo.".into()),
                },
                (CONFIRMAR, Ok(v)) => {
                    let codigos = v["codigos_de_recuperacao"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|c| c.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default();
                    self.situacao = Situacao::RecemLigado { codigos };
                }
                (DESLIGAR, Ok(_)) => self.situacao = Situacao::Desligado,
                (_, Err(e)) => self.erro = Some(legivel(&e).into()),
                _ => {}
            }
            cx.notify();
        }
        let continua = self.ocupado;
        if !continua {
            self._colheita = None;
        }
        continua
    }
}

fn legivel(erro: &str) -> String {
    if erro.contains("422") || erro.to_lowercase().contains("codigo incorreto") {
        return "Código incorreto ou vencido. Confira o relógio do celular e use o código novo."
            .into();
    }
    crate::erro_da_api::legivel(erro)
}

fn situacao_lida(v: &serde_json::Value) -> Situacao {
    if v["ligado"].as_bool().unwrap_or(false) {
        Situacao::Ligado {
            // `2026-10-18T…` vira `18/10/2026`, como o resto do app escreve data.
            desde: v["desde"].as_str().and_then(|d| {
                let (ano, resto) = d.get(..10)?.split_once('-')?;
                let (mes, dia) = resto.split_once('-')?;
                Some(format!("{dia}/{mes}/{ano}"))
            }),
            restantes: v["recuperacoes_restantes"].as_u64().unwrap_or(0),
        }
    } else {
        Situacao::Desligado
    }
}

/// O QR em PNG/base64 virado imagem do GPUI.
fn cadastro(v: &serde_json::Value) -> Option<Situacao> {
    let png = base64::engine::general_purpose::STANDARD
        .decode(v["qr_png_base64"].as_str()?)
        .ok()?;
    let imagem = image::load_from_memory(&png).ok()?;
    Some(Situacao::Cadastrando {
        qr: crate::imagem::para_gpui(imagem),
        segredo: v["segredo"].as_str()?.to_string(),
    })
}

impl Render for AutenticadorDaConta {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui_kit::component::input::{Input, OtpInput};
        use gpui_kit::component::{v_flex, Disableable};
        use gpui_kit::img;

        let apagado = cx.theme().muted_foreground;
        let titulo = div()
            .text_sm()
            .font_weight(gpui_kit::FontWeight::MEDIUM)
            .child("Google Authenticator");
        let explicacao = |t: &str| div().text_xs().text_color(apagado).child(t.to_string());
        let ocupado = self.ocupado;

        let miolo = match self.situacao.clone() {
            Situacao::Lendo => v_flex().child(explicacao("Lendo…")),
            Situacao::Indisponivel(motivo) => v_flex().child(explicacao(&motivo)),
            Situacao::Desligado => v_flex()
                .gap(px(10.))
                .items_start()
                .child(explicacao(
                    "Com o Google Authenticator (ou Microsoft Authenticator, 1Password, Authy), \
                     a tela de entrada pede só o seu e-mail e os seis dígitos que o celular mostra.",
                ))
                .child(
                    crate::estilo::botao_primario("autenticador-ligar", cx)
                        .label(if ocupado { "Preparando…" } else { "Ligar o Google Authenticator" })
                        .disabled(ocupado)
                        .on_click(cx.listener(|tela, _, _, cx| tela.iniciar(cx))),
                ),
            Situacao::Cadastrando { qr, segredo } => v_flex()
                .gap(px(10.))
                .items_start()
                .child(explicacao(
                    "No Google Authenticator, toque em “+” e em “Ler QR code”. Depois digite o \
                     código de seis dígitos que aparecer.",
                ))
                .child(
                    div()
                        .size(px(200.))
                        .p(px(8.))
                        .rounded(crate::tema::canto(6.))
                        .bg(gpui_kit::white())
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(img(qr).max_w_full().max_h_full()),
                )
                .child(explicacao(&format!("Sem câmera? Digite a chave à mão: {segredo}")))
                .child(OtpInput::new(&self.codigo).groups(2).disabled(ocupado))
                .child(
                    gpui_kit::component::h_flex()
                        .gap(px(8.))
                        .child(
                            crate::estilo::botao_primario("autenticador-confirmar", cx)
                                .label(if ocupado { "Conferindo…" } else { "Confirmar" })
                                .disabled(ocupado)
                                .on_click(cx.listener(|tela, _, window, cx| tela.confirmar(window, cx))),
                        )
                        .child(
                            crate::estilo::botao_secundario("autenticador-cancelar", cx)
                                .label("Cancelar")
                                .on_click(cx.listener(|tela, _, _, cx| tela.cancelar_cadastro(cx))),
                        ),
                ),
            Situacao::RecemLigado { codigos } => v_flex()
                .gap(px(10.))
                .child(explicacao(
                    "Ligado. Guarde estes códigos num lugar seguro: cada um entra uma vez no lugar \
                     do código do celular, se ele se perder. Eles não aparecem de novo.",
                ))
                .child(
                    div()
                        .grid()
                        .grid_cols(2)
                        .gap(px(6.))
                        .font_family("Menlo")
                        .text_sm()
                        .children(codigos.into_iter().map(|c| {
                            div()
                                .px(px(10.))
                                .py(px(4.))
                                .rounded(crate::tema::canto(4.))
                                .border_1()
                                .border_color(cx.theme().border)
                                .child(c)
                        })),
                ),
            Situacao::Ligado { desde, restantes } => v_flex()
                .gap(px(10.))
                .items_start()
                .child(explicacao(&format!(
                    "Ligado{}. Restam {restantes} códigos de recuperação. Para desligar, digite um \
                     código do celular ou de recuperação.",
                    desde.map(|d| format!(" desde {d}")).unwrap_or_default()
                )))
                .child(
                    div().w(px(340.)).child(crate::estilo::campo(
                        Input::new(&self.para_desligar).disabled(ocupado),
                    )),
                )
                .child(
                    crate::estilo::botao_secundario("autenticador-desligar", cx)
                        .label(if ocupado { "Desligando…" } else { "Desligar o Authenticator" })
                        .disabled(ocupado)
                        .on_click(cx.listener(|tela, _, window, cx| tela.desligar(window, cx))),
                ),
        };

        v_flex()
            .gap(px(10.))
            .max_w(px(520.))
            .child(titulo)
            .child(miolo)
            .when_some(self.erro.clone(), |d, erro| {
                d.child(div().text_xs().text_color(cx.theme().danger).child(erro))
            })
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::pos_venda::porta::mentira::PublicadorDeMentira;
    use gpui_kit::TestAppContext;

    fn sessao() -> Sessao {
        Sessao {
            access_token: "t".into(),
            refresh_token: "r".into(),
            access_vence_em: i64::MAX,
            refresh_vence_em: i64::MAX,
        }
    }

    /// Um PNG 1×1 de verdade, para o QR decodificar.
    fn png_minimo() -> String {
        let mut bytes = Vec::new();
        image::DynamicImage::new_rgba8(1, 1)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    #[gpui_kit::test]
    fn do_desligado_ao_qr_e_aos_codigos_de_recuperacao(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let publicador = Arc::new(PublicadorDeMentira::default());
        publicador.responder_json(
            LER,
            Ok(serde_json::json!({ "ligado": false, "pendente": false })),
        );
        publicador.responder_json(
            INICIAR,
            Ok(serde_json::json!({
                "uri": "otpauth://totp/RecordarFotos:a@b?secret=X",
                "segredo": "ABCD EFGH",
                "qr_png_base64": png_minimo(),
            })),
        );
        publicador.responder_json(
            CONFIRMAR,
            Ok(serde_json::json!({ "codigos_de_recuperacao": ["abcd-efgh", "jkmn-pqrs"] })),
        );

        let janela = cx.add_window(|window, cx| {
            AutenticadorDaConta::nova(publicador.clone(), Some(sessao()), window, cx)
        });
        janela
            .update(cx, |tela, window, cx| {
                tela.colher(cx);
                assert!(matches!(tela.situacao(), Situacao::Desligado));

                tela.iniciar(cx);
                tela.colher(cx);
                assert!(
                    matches!(tela.situacao(), Situacao::Cadastrando { segredo, .. } if segredo == "ABCD EFGH"),
                    "o QR decodificado e a chave à mão"
                );

                tela.codigo.update(cx, |c, cx| c.set_value("123456", window, cx));
                tela.confirmar(window, cx);
                tela.colher(cx);
                assert!(
                    matches!(tela.situacao(), Situacao::RecemLigado { codigos } if codigos.len() == 2)
                );
            })
            .expect("a janela de teste");

        let caminhos: Vec<_> = publicador
            .pedidos_json()
            .into_iter()
            .map(|p| (p.metodo, p.caminho, p.corpo))
            .collect();
        assert_eq!(caminhos[2].1, "/auth/autenticador/confirmar");
        assert_eq!(
            caminhos[2].2,
            Some(serde_json::json!({ "codigo": "123456" }))
        );
    }

    #[gpui_kit::test]
    fn codigo_errado_avisa_sem_sair_do_cadastro(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let publicador = Arc::new(PublicadorDeMentira::default());
        publicador.responder_json(
            LER,
            Ok(serde_json::json!({ "ligado": true, "desde": "2026-10-18T10:00:00Z", "recuperacoes_restantes": 8 })),
        );
        publicador.responder_json(
            DESLIGAR,
            Err("o site respondeu 422 Unprocessable Entity: Codigo incorreto ou vencido".into()),
        );
        let janela = cx.add_window(|window, cx| {
            AutenticadorDaConta::nova(publicador.clone(), Some(sessao()), window, cx)
        });
        janela
            .update(cx, |tela, window, cx| {
                tela.colher(cx);
                assert!(matches!(
                    tela.situacao(),
                    Situacao::Ligado { restantes: 8, .. }
                ));
                tela.para_desligar
                    .update(cx, |c, cx| c.set_value("000000", window, cx));
                tela.desligar(window, cx);
                tela.colher(cx);
                assert!(
                    matches!(tela.situacao(), Situacao::Ligado { .. }),
                    "continua ligado"
                );
                assert!(tela.erro().is_some_and(|e| e.contains("Código incorreto")));
            })
            .expect("a janela de teste");
    }
}
