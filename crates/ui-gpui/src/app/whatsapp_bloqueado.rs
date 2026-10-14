//! 🚨 O aviso de que a Meta parou de entregar as mensagens da casa.
//!
//! Em 13/out/2026 a conta do WhatsApp Business ficou com cobrança em aberto:
//! a Meta recusou tudo o dia inteiro, o "Exportar › WhatsApp" dizia que as
//! fotos tinham ido, e o balcão reenviou as mesmas três sete vezes ao mesmo
//! cliente. O dono: *"tem que ter alguma área no sistema para informar esse
//! problema de pagamento […] inclusive um link para fazer o pagamento, pois
//! isso será recorrente daqui para frente"*.
//!
//! # O que este módulo faz
//!
//! Pergunta ao site de minuto em minuto (`GET /whatsapp/saude-do-envio`) e,
//! enquanto a Meta estiver recusando, acende um trecho vermelho no rodapé. O
//! clique abre o que parou, quantas mensagens se perderam, desde quando e o
//! que fazer — e, para quem administra, o botão que leva ao pagamento.
//!
//! 🔑 **A regra não mora aqui.** Quando acende e quando apaga é do site
//! (`domain::whatsapp::saude_do_envio`): a primeira mensagem que chega ao
//! cliente apaga o aviso sozinha, nas duas interfaces ao mesmo tempo.
//!
//! 🚨 **Leitura que falha não apaga nem acende.** Sem resposta não há o que
//! afirmar; o trecho fica como estava até o site responder.

use std::fmt::Display;
use std::time::Duration;

use chrono::{DateTime, Local, TimeZone, Utc};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon};
use gpui_kit::{div, prelude::*, px, AnyElement, Context, FontWeight, SharedString};
use serde_json::Value;

use super::Aplicativo;
use crate::estilo;
use crate::pos_venda::porta::{PedidoJson, Recado};
use crate::recursos::Icone;
use crate::segundo_plano::frases::plural;

const ROTULO: &str = "saude-do-whatsapp";

/// De quanto em quanto tempo o app pergunta. O bloqueio dura horas.
const INTERVALO: Duration = Duration::from_secs(60);

/// A espera enquanto não há conta a quem perguntar.
const SEM_CONTA: Duration = Duration::from_secs(3);

/// Cobrança em aberto na conta do WhatsApp Business (Cloud API).
const PAGAMENTO_PENDENTE: i64 = 131042;

/// O que o rodapé mostra enquanto a Meta recusa as mensagens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhatsAppBloqueado {
    /// "Pagamento pendente na conta do WhatsApp Business".
    pub titulo: String,
    /// "3 mensagens para 2 conversas não chegaram desde hoje às 15:40."
    pub resumo: String,
    pub o_que_fazer: String,
    /// Onde se resolve — só endereço da Meta, e só para quem administra.
    pub link: Option<String>,
    pub rotulo_do_link: &'static str,
}

impl WhatsAppBloqueado {
    /// A frase para quem está escolhendo o contato no "Exportar › WhatsApp".
    pub fn na_exportacao(&self) -> String {
        format!(
            "A Meta não está entregando mensagens: {}. O que for enviado agora não chega ao cliente.",
            minuscula_na_frente(&self.titulo)
        )
    }
}

/// Só a inicial: "WhatsApp Business" é nome próprio no meio da frase.
fn minuscula_na_frente(texto: &str) -> String {
    let mut letras = texto.chars();
    match letras.next() {
        Some(primeira) => primeira.to_lowercase().chain(letras).collect(),
        None => String::new(),
    }
}

/// Só `https` em `facebook.com` ou `meta.com`. O site já filtra; o botão leva
/// a uma página de pagamento, e conferir de novo custa uma função.
fn link_da_meta(link: &str) -> Option<String> {
    let resto = link.strip_prefix("https://")?;
    let host = resto.split(['/', '?', '#']).next().unwrap_or_default();
    ["facebook.com", "meta.com"]
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")))
        .then(|| link.to_string())
}

fn desde_quando<Tz: TimeZone>(desde: Option<&str>, agora: &DateTime<Tz>) -> String
where
    Tz::Offset: Display,
{
    let Some(quando) = desde.and_then(|d| DateTime::parse_from_rfc3339(d).ok()) else {
        return String::new();
    };
    let quando = quando.with_timezone(&Utc).with_timezone(&agora.timezone());
    if quando.date_naive() == agora.date_naive() {
        format!(" desde hoje às {}", quando.format("%H:%M"))
    } else {
        format!(" desde {}", quando.format("%d/%m às %H:%M"))
    }
}

/// O aviso na resposta do site, ou `None` quando está tudo chegando.
pub fn aviso_da_resposta<Tz: TimeZone>(
    valor: &Value,
    administra: bool,
    agora: &DateTime<Tz>,
) -> Option<WhatsAppBloqueado>
where
    Tz::Offset: Display,
{
    if valor.get("bloqueado")?.as_bool() != Some(true) {
        return None;
    }
    let motivo = valor.get("motivo")?;
    let texto = |campo: &str| {
        motivo
            .get(campo)
            .and_then(Value::as_str)
            .unwrap_or_default()
    };
    let conta = |campo: &str| valor.get(campo).and_then(Value::as_u64).unwrap_or(0) as usize;
    let perdidas = conta("mensagens_perdidas");
    let chegaram = if perdidas == 1 {
        "não chegou"
    } else {
        "não chegaram"
    };
    let mut o_que_fazer = texto("o_que_fazer").to_string();
    if !administra {
        o_que_fazer.push_str(" Avise quem administra a conta.");
    }
    let cobranca = motivo.get("codigo").and_then(Value::as_i64) == Some(PAGAMENTO_PENDENTE);
    Some(WhatsAppBloqueado {
        titulo: texto("titulo").to_string(),
        resumo: format!(
            "{} para {} {chegaram}{}. O que aparece como enviado não chega ao cliente.",
            plural(perdidas, "mensagem", "mensagens"),
            plural(conta("contatos_afetados"), "conversa", "conversas"),
            desde_quando(valor.get("desde").and_then(Value::as_str), agora),
        ),
        o_que_fazer,
        link: administra
            .then(|| motivo.get("link").and_then(Value::as_str))
            .flatten()
            .and_then(link_da_meta),
        rotulo_do_link: if cobranca {
            "Pagar na Meta"
        } else {
            "Resolver na Meta"
        },
    })
}

impl Aplicativo {
    /// Liga a pergunta de minuto em minuto. Uma vez por abertura: a conta
    /// entra e sai, e o laço olha a sessão a cada volta.
    pub(super) fn vigiar_o_whatsapp(&mut self, cx: &mut Context<Self>) {
        if self._whatsapp.is_some() {
            return;
        }
        self._whatsapp = Some(cx.spawn(async move |raiz, cx| loop {
            let Ok(perguntou) = raiz.update(cx, |raiz, cx| raiz.perguntar_do_whatsapp(cx)) else {
                return;
            };
            if !perguntou {
                // Sem conta (ou antes de o `/auth/me` responder): olha de novo
                // logo, para o aviso não esperar um minuto depois de entrar.
                cx.background_executor().timer(SEM_CONTA).await;
                continue;
            }
            // A resposta: até 20 s, sem segurar a interface.
            for _ in 0..100 {
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                let Ok(chegou) = raiz.update(cx, |raiz, cx| raiz.colher_o_whatsapp(cx)) else {
                    return;
                };
                if chegou {
                    break;
                }
            }
            cx.background_executor().timer(INTERVALO).await;
        }));
    }

    /// Pede a saúde do envio ao site. `false` quando não havia a quem pedir.
    fn perguntar_do_whatsapp(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(sessao) = self.sessao.clone() else {
            // Saiu da conta: o aviso era dela.
            if self.whatsapp_bloqueado.take().is_some() {
                self.vendo_o_whatsapp = false;
                self.exportacao
                    .update(cx, |tela, cx| tela.definir_bloqueio_do_whatsapp(None, cx));
                cx.notify();
            }
            return false;
        };
        // A rota é de quem tem as conversas ou opera o balcão. Antes de o
        // `/auth/me` responder não se sabe; a próxima volta pergunta.
        let pode = self.conta.as_ref().is_some_and(|conta| {
            conta.pode(biblioteca_core::acesso::CONVERSAS)
                || conta.pode(biblioteca_core::acesso::OPERAR_O_BALCAO)
        });
        if !pode {
            return false;
        }
        self.publicador.pedir_json(
            sessao,
            PedidoJson::ler(ROTULO, "/whatsapp/saude-do-envio"),
            self.recados_do_whatsapp.0.clone(),
        );
        true
    }

    /// Trata a resposta. `true` quando ela chegou.
    pub(crate) fn colher_o_whatsapp(&mut self, cx: &mut Context<Self>) -> bool {
        let mut chegou = false;
        while let Ok(recado) = self.recados_do_whatsapp.1.try_recv() {
            let Recado::Json {
                rotulo: ROTULO,
                resultado,
            } = recado
            else {
                continue;
            };
            chegou = true;
            // Site de antes da rota, conta sem a capacidade, rede fora: fica
            // como estava.
            let Ok(valor) = resultado else {
                continue;
            };
            let administra = self
                .conta
                .as_ref()
                .and_then(|conta| conta.acesso.as_ref())
                .is_some_and(|acesso| acesso.admin);
            let novo = aviso_da_resposta(&valor, administra, &Local::now());
            if novo != self.whatsapp_bloqueado {
                if novo.is_none() {
                    self.vendo_o_whatsapp = false;
                }
                // 💬 Quem está no "Exportar › WhatsApp" lê o mesmo aviso ali.
                let frase = novo.as_ref().map(WhatsAppBloqueado::na_exportacao);
                self.exportacao
                    .update(cx, |tela, cx| tela.definir_bloqueio_do_whatsapp(frase, cx));
                self.whatsapp_bloqueado = novo;
                cx.notify();
            }
        }
        chegou
    }

    /// O trecho do rodapé: vermelho, e o clique abre os detalhes.
    pub(super) fn trecho_do_whatsapp(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let aviso = self.whatsapp_bloqueado.as_ref()?;
        let perigo = cx.theme().danger;
        let raiz = cx.entity().downgrade();
        let do_conteudo = raiz.clone();
        let dica: SharedString = aviso.titulo.clone().into();
        Some(
            Popover::new("rodape-whatsapp-detalhes")
                .anchor(gpui_kit::Anchor::BottomLeft)
                .appearance(false)
                .open(self.vendo_o_whatsapp)
                .on_open_change(move |aberto, _window, cx| {
                    let aberto = *aberto;
                    let _ = raiz.update(cx, |raiz, cx| {
                        raiz.vendo_o_whatsapp = aberto;
                        cx.notify();
                    });
                })
                .content(move |_, _window, cx| {
                    do_conteudo
                        .upgrade()
                        .and_then(|raiz| raiz.update(cx, |raiz, cx| raiz.detalhes_do_whatsapp(cx)))
                        .unwrap_or_else(|| div().into_any_element())
                })
                .trigger(
                    estilo::botao_raso("rodape-whatsapp")
                        .debug_selector(|| "rodape-whatsapp".into())
                        .flex_none()
                        .text_color(perigo)
                        .font_weight(FontWeight::SEMIBOLD)
                        .bg(perigo.opacity(0.12))
                        .child(Icon::new(Icone::TriangleAlert).size(px(12.)))
                        .child("WhatsApp não está entregando")
                        // Aberto, o quadro já diz tudo — e a dica cairia em
                        // cima do canto dele.
                        .when(!self.vendo_o_whatsapp, |botao| botao.tooltip(dica)),
                )
                .into_any_element(),
        )
    }

    /// O conteúdo do `Popover`: o que parou, o que fazer e onde pagar.
    fn detalhes_do_whatsapp(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        use gpui_kit::component::button::ButtonVariants as _;
        use gpui_kit::component::Sizable as _;

        let aviso = self.whatsapp_bloqueado.clone()?;
        let tema = cx.theme();
        let (fundo, apagado, perigo) = (tema.popover, tema.muted_foreground, tema.danger);
        let botao = |id: &'static str, rotulo: &'static str| {
            gpui_kit::component::button::Button::new(id)
                .label(rotulo)
                .xsmall()
        };
        let pagar = aviso.link.clone().map(|link| {
            botao("whatsapp-resolver", aviso.rotulo_do_link)
                .danger()
                .on_click(move |_, _window, cx| cx.open_url(&link))
        });
        Some(
            v_flex()
                .id("detalhes-do-whatsapp")
                .debug_selector(|| "detalhes-do-whatsapp".into())
                .mb(px(6.))
                .w(px(440.))
                .p(px(12.))
                .gap(px(8.))
                .rounded(crate::tema::canto(10.))
                .border_1()
                .border_color(perigo.opacity(0.55))
                .bg(fundo)
                .shadow_lg()
                .child(
                    h_flex()
                        .items_start()
                        .gap(px(8.))
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(perigo)
                        .child(Icon::new(Icone::TriangleAlert).size(px(14.)))
                        .child(div().flex_1().min_w(px(0.)).child(format!(
                            "O WhatsApp não está entregando: {}",
                            minuscula_na_frente(&aviso.titulo)
                        ))),
                )
                .child(div().text_xs().child(aviso.resumo))
                .child(div().text_xs().text_color(apagado).child(aviso.o_que_fazer))
                .child(
                    h_flex()
                        .justify_end()
                        .gap(px(6.))
                        .child(
                            botao("whatsapp-fechar", "Fechar")
                                .ghost()
                                .on_click(cx.listener(|raiz, _, _window, cx| {
                                    raiz.vendo_o_whatsapp = false;
                                    cx.notify();
                                })),
                        )
                        .children(pagar),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use chrono::FixedOffset;
    use serde_json::json;

    const PAGAR: &str =
        "https://business.facebook.com/billing_hub/accounts/details/?business_id=1&asset_id=2&wizard_name=PAY_NOW";

    fn agora() -> DateTime<FixedOffset> {
        // 13/out/2026, 18:00 em Brasília.
        FixedOffset::west_opt(3 * 3600)
            .unwrap()
            .with_ymd_and_hms(2026, 10, 13, 18, 0, 0)
            .unwrap()
    }

    fn bloqueado() -> Value {
        json!({
            "bloqueado": true,
            "motivo": {
                "codigo": 131042,
                "titulo": "Pagamento pendente na conta do WhatsApp Business",
                "o_que_fazer": "A Meta só volta a entregar depois que a cobrança for paga.",
                "link": PAGAR,
                "da_meta": "131042: Business eligibility payment issue",
            },
            "desde": "2026-10-13T18:40:00Z",
            "mensagens_perdidas": 3,
            "contatos_afetados": 2,
        })
    }

    #[test]
    fn sem_bloqueio_o_rodape_nao_diz_nada() {
        let normal = json!({
            "bloqueado": false, "motivo": null, "desde": null,
            "mensagens_perdidas": 0, "contatos_afetados": 0,
        });
        assert_eq!(aviso_da_resposta(&normal, true, &agora()), None);
        // Resposta de outro formato também não acende.
        assert_eq!(aviso_da_resposta(&json!({}), true, &agora()), None);
        assert_eq!(aviso_da_resposta(&json!([]), true, &agora()), None);
    }

    #[test]
    fn quem_administra_ve_a_conta_do_que_se_perdeu_e_o_botao_de_pagar() {
        let aviso = aviso_da_resposta(&bloqueado(), true, &agora()).unwrap();

        assert_eq!(
            aviso.titulo,
            "Pagamento pendente na conta do WhatsApp Business"
        );
        // 18:40 UTC são 15:40 no balcão.
        assert_eq!(
            aviso.resumo,
            "3 mensagens para 2 conversas não chegaram desde hoje às 15:40. \
             O que aparece como enviado não chega ao cliente."
        );
        assert_eq!(aviso.link.as_deref(), Some(PAGAR));
        assert_eq!(aviso.rotulo_do_link, "Pagar na Meta");
    }

    #[test]
    fn quem_so_opera_o_balcao_e_mandado_avisar_sem_o_link_da_cobranca() {
        let aviso = aviso_da_resposta(&bloqueado(), false, &agora()).unwrap();

        assert_eq!(aviso.link, None);
        assert!(aviso
            .o_que_fazer
            .ends_with("Avise quem administra a conta."));
    }

    #[test]
    fn uma_mensagem_so_concorda_no_singular_e_outro_dia_leva_a_data() {
        let mut resposta = bloqueado();
        resposta["mensagens_perdidas"] = json!(1);
        resposta["contatos_afetados"] = json!(1);
        resposta["desde"] = json!("2026-10-12T18:40:00Z");

        let aviso = aviso_da_resposta(&resposta, true, &agora()).unwrap();

        assert!(
            aviso
                .resumo
                .starts_with("1 mensagem para 1 conversa não chegou desde 12/10 às 15:40."),
            "{}",
            aviso.resumo
        );
    }

    #[test]
    fn a_frase_da_exportacao_diz_o_motivo_sem_gritar_o_nome_proprio() {
        let aviso = aviso_da_resposta(&bloqueado(), false, &agora()).unwrap();

        assert_eq!(
            aviso.na_exportacao(),
            "A Meta não está entregando mensagens: pagamento pendente na conta do WhatsApp \
             Business. O que for enviado agora não chega ao cliente."
        );
    }

    #[test]
    fn bloqueio_que_nao_e_cobranca_nao_fala_em_pagar() {
        let mut resposta = bloqueado();
        resposta["motivo"]["codigo"] = json!(131031);

        let aviso = aviso_da_resposta(&resposta, true, &agora()).unwrap();

        assert_eq!(aviso.rotulo_do_link, "Resolver na Meta");
    }

    #[test]
    fn link_de_fora_da_meta_nao_vira_botao() {
        assert_eq!(link_da_meta(PAGAR).as_deref(), Some(PAGAR));
        assert!(link_da_meta("https://business.meta.com/billing").is_some());
        for falso in [
            "",
            "pagar",
            "http://business.facebook.com/billing_hub",
            "https://business.facebook.com.pagamento.example/pay",
            "https://example.com/?u=business.facebook.com",
        ] {
            assert_eq!(link_da_meta(falso), None, "{falso}");
        }

        let mut resposta = bloqueado();
        resposta["motivo"]["link"] = json!("https://example.com/pagar");
        assert_eq!(
            aviso_da_resposta(&resposta, true, &agora()).unwrap().link,
            None
        );
    }
}
