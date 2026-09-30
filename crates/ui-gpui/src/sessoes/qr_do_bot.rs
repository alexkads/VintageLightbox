//! 📸 O QR do bot da sessão — no canto inferior direito da sessão e da
//! segunda tela.
//!
//! # O pedido (dono, 30/set/2026)
//!
//! *"Durante a sessão de fotos nas interfaces web e desktop… exiba no canto
//! inferior direito da tela um QRCode onde o mesmo poderá levar o cliente para
//! um bot (WhatsApp, Instagram, Telegram, Messenger ou nosso botweb)… A
//! aplicação deverá acusar que o cliente iniciou a conversa no bot. A tecla i
//! deverá esconder o QRCode."*
//!
//! O QR leva à página `/s/{codigo}` do site, com as cinco portas; o bot faz as
//! perguntas (nome, telefone, e-mail, como conheceu) e grava na sessão. Quem
//! manda tudo isto é o site (`GET /pos-venda/galerias/{id}/bot`); o app só
//! desenha e acusa.
//!
//! 🔑 **A web é a referência**: as regras daqui são as de
//! `frontend/src/lib/biblioteca/qr-do-bot.ts`, com os mesmos nomes e os mesmos
//! textos — a mesma sessão aberta nas duas telas diz a mesma coisa.

use std::sync::Arc;

use gpui_kit::{canvas, div, point, prelude::*, px, size, Bounds, Hsla, IntoElement};
use serde::Deserialize;

/// O que `GET /pos-venda/galerias/{id}/bot` devolve.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SituacaoDoBot {
    pub convite: ConviteDoBot,
    /// O QR em módulos — `true` é escuro —, já com a margem clara.
    pub qr: Vec<Vec<bool>>,
    #[serde(default)]
    pub clientes: Vec<ClienteNoBot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ConviteDoBot {
    pub codigo: String,
    pub url: String,
}

/// Quem chegou pelo QR, por qual canal, e o que respondeu.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ClienteNoBot {
    pub canal: String,
    pub nome: Option<String>,
    pub passo: String,
    pub nome_completo: Option<String>,
    /// RFC 3339, como o site manda — só serve de chave.
    pub iniciado_em: String,
}

/// O caminho da situação no site.
pub fn caminho_da_situacao(galeria_id: &str) -> String {
    format!("/pos-venda/galerias/{galeria_id}/bot")
}

/// O caminho do tempo real da sessão.
pub fn caminho_dos_eventos(galeria_id: &str) -> String {
    format!("/pos-venda/galerias/{galeria_id}/eventos")
}

/// O rótulo com que a tela pede a situação — o que volta no `Recado::Json`.
pub const PEDIDO_DA_SITUACAO: &str = "bot-da-sessao";

/// O evento do site que muda o cartão.
pub fn evento_do_bot(dados: &serde_json::Value) -> bool {
    matches!(
        dados.get("tipo").and_then(|t| t.as_str()),
        Some("cliente_no_bot" | "dados_do_bot")
    )
}

pub fn nome_do_canal(canal: &str) -> &'static str {
    match canal {
        "whatsapp" => "WhatsApp",
        "instagram" => "Instagram",
        "telegram" => "Telegram",
        "messenger" => "Messenger",
        "web" => "chat do site",
        _ => "bot",
    }
}

pub fn rotulo_do_passo(passo: &str) -> &'static str {
    match passo {
        "nome" => "respondendo o nome",
        "telefone" => "respondendo o telefone",
        "email" => "respondendo o e-mail",
        "como_conheceu" | "como_conheceu_detalhe" => "respondendo como conheceu",
        "escolher_galeria" => "escolhendo a sessão",
        "concluido" => "cadastro concluído",
        _ => "conversando",
    }
}

/// O nome que deu, o do perfil, ou "Cliente".
pub fn quem_e(cliente: &ClienteNoBot) -> String {
    cliente
        .nome_completo
        .clone()
        .or_else(|| cliente.nome.clone())
        .unwrap_or_else(|| "Cliente".to_string())
}

/// Em que ponto a sessão está com o bot — ver o site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoDoBot {
    Aguardando,
    Conversando,
    Concluido,
}

pub fn estado_do_bot(clientes: &[ClienteNoBot]) -> EstadoDoBot {
    if clientes.is_empty() {
        EstadoDoBot::Aguardando
    } else if clientes.iter().any(|c| c.passo == "concluido") {
        EstadoDoBot::Concluido
    } else {
        EstadoDoBot::Conversando
    }
}

/// O que merece interromper o operador entre duas leituras. A primeira
/// leitura não é novidade: é o estado que já existia.
pub fn novidades_do_bot(antes: Option<&[ClienteNoBot]>, depois: &[ClienteNoBot]) -> Vec<String> {
    let Some(antes) = antes else {
        return Vec::new();
    };
    let chave = |c: &ClienteNoBot| (c.canal.clone(), c.iniciado_em.clone());
    depois
        .iter()
        .filter_map(|c| match antes.iter().find(|v| chave(v) == chave(c)) {
            None => Some(format!(
                "{} iniciou a conversa pelo {}",
                quem_e(c),
                nome_do_canal(&c.canal)
            )),
            Some(velho) if velho.passo != "concluido" && c.passo == "concluido" => Some(format!(
                "{} concluiu o cadastro pelo {}",
                quem_e(c),
                nome_do_canal(&c.canal)
            )),
            Some(_) => None,
        })
        .collect()
}

/// O que a sessão manda à segunda tela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConviteNaTela {
    pub codigo: String,
    pub qr: Arc<Vec<Vec<bool>>>,
    pub estado: EstadoDoBot,
    /// "Maria · WhatsApp", quando alguém já começou.
    pub quem: Option<String>,
}

pub fn convite_para_a_tela(situacao: &SituacaoDoBot) -> ConviteNaTela {
    ConviteNaTela {
        codigo: situacao.convite.codigo.clone(),
        qr: Arc::new(situacao.qr.clone()),
        estado: estado_do_bot(&situacao.clientes),
        quem: situacao
            .clientes
            .first()
            .map(|c| format!("{} · {}", quem_e(c), nome_do_canal(&c.canal))),
    }
}

/// O que o "Avisar cliente" diz depois de avisar — o `resumoDoAviso` do site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumoDoAviso {
    pub sucesso: String,
    pub falhas: Vec<String>,
}

pub fn resumo_do_aviso(
    email: &str,
    canais: &[domain::services::pos_venda::EntregaNoCanal],
) -> ResumoDoAviso {
    let mut por: Vec<&str> = vec!["e-mail"];
    por.extend(
        canais
            .iter()
            .filter(|c| c.enviado)
            .map(|c| nome_do_canal(&c.canal)),
    );
    let lista = match por.split_last() {
        Some((ultimo, antes)) if !antes.is_empty() => format!("{} e {ultimo}", antes.join(", ")),
        _ => por[0].to_string(),
    };
    let sucesso = if email.is_empty() {
        format!("Aviso enviado por {lista}.")
    } else {
        format!("Aviso enviado por {lista} ({email}).")
    };
    ResumoDoAviso {
        sucesso,
        falhas: canais
            .iter()
            .filter(|c| !c.enviado)
            .map(|c| {
                format!(
                    "{} ficou de fora: {}",
                    nome_do_canal(&c.canal),
                    c.motivo.as_deref().unwrap_or("sem motivo informado")
                )
            })
            .collect(),
    }
}

/// O QR desenhado módulo a módulo, preto sobre branco, em `lado` pontos.
///
/// 🔑 **Pintado, e não mil `div`**: são ~1.000 módulos, e um `canvas` com um
/// quadrado por módulo escuro é um elemento só no quadro. Preto sobre branco
/// em qualquer tema — a câmera não lê outra combinação com segurança.
pub fn desenho_do_qr(qr: Arc<Vec<Vec<bool>>>, lado: f32) -> impl IntoElement {
    let preto: Hsla = gpui_kit::black();
    let branco: Hsla = gpui_kit::white();
    div().size(px(lado)).bg(branco).child(
        canvas(
            |_, _, _| {},
            move |limites: Bounds<gpui_kit::Pixels>, _, window, _| {
                let n = qr.len().max(1) as f32;
                let modulo = f32::from(limites.size.width) / n;
                for (y, linha) in qr.iter().enumerate() {
                    for (x, escuro) in linha.iter().enumerate() {
                        if !*escuro {
                            continue;
                        }
                        let origem = point(
                            limites.origin.x + px(x as f32 * modulo),
                            limites.origin.y + px(y as f32 * modulo),
                        );
                        // Um décimo de ponto a mais: sem ele, o arredondamento
                        // deixa fios brancos entre módulos vizinhos.
                        window.paint_quad(gpui_kit::fill(
                            Bounds::new(origem, size(px(modulo + 0.1), px(modulo + 0.1))),
                            preto,
                        ));
                    }
                }
            },
        )
        .size_full(),
    )
}

/// O que a tela da sessão guarda sobre o bot.
#[derive(Default)]
pub struct EstadoDoQr {
    pub situacao: Option<SituacaoDoBot>,
    /// Os clientes da leitura anterior — `None` antes da primeira.
    pub anteriores: Option<Vec<ClienteNoBot>>,
    /// A tecla `I`.
    pub escondido: bool,
    /// Depois de concluído o QR recolhe; o operador o abre para outra pessoa.
    pub qr_aberto: bool,
    pub lendo: bool,
    pub releitura_pendente: bool,
    /// Enquanto viva, o tempo real desta sessão fica aberto.
    pub guarda: Option<crate::tempo_real::Guarda>,
    pub vigia: Option<gpui_kit::Task<()>>,
}

impl EstadoDoQr {
    /// O convite que a segunda tela deve mostrar agora — `None` escondido.
    pub fn para_a_tela_do_cliente(&self) -> Option<ConviteNaTela> {
        if self.escondido {
            return None;
        }
        self.situacao.as_ref().map(convite_para_a_tela)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use domain::services::pos_venda::EntregaNoCanal;

    fn cliente(canal: &str, passo: &str, nome_completo: Option<&str>) -> ClienteNoBot {
        ClienteNoBot {
            canal: canal.into(),
            nome: Some("Maria".into()),
            passo: passo.into(),
            nome_completo: nome_completo.map(str::to_string),
            iniciado_em: "2026-09-30T12:00:00Z".into(),
        }
    }

    #[test]
    fn a_situacao_do_site_e_lida_como_ele_manda() {
        let json = serde_json::json!({
            "convite": { "codigo": "RF-ABC234", "url": "https://x/s/RF-ABC234", "expira_em": "2026-10-02T03:00:00Z" },
            "qr": [[true, false], [false, true]],
            "clientes": [{
                "canal": "whatsapp", "nome": "Maria", "passo": "email", "nome_completo": "Maria Souza",
                "telefone": null, "email": null, "como_conheceu": null, "como_conheceu_detalhe": null,
                "iniciado_em": "2026-09-30T12:00:00Z", "atualizado_em": "2026-09-30T12:01:00Z"
            }]
        });
        let s: SituacaoDoBot = serde_json::from_value(json).unwrap();
        assert_eq!(s.convite.codigo, "RF-ABC234");
        assert_eq!(s.clientes[0].passo, "email");
        let tela = convite_para_a_tela(&s);
        assert_eq!(tela.estado, EstadoDoBot::Conversando);
        assert_eq!(tela.quem.as_deref(), Some("Maria Souza · WhatsApp"));
    }

    #[test]
    fn acusa_quem_chegou_e_quem_terminou_e_so_uma_vez() {
        assert!(novidades_do_bot(None, &[cliente("web", "nome", None)]).is_empty());
        assert_eq!(
            novidades_do_bot(Some(&[]), &[cliente("whatsapp", "nome", None)]),
            vec!["Maria iniciou a conversa pelo WhatsApp"]
        );
        let antes = [cliente("web", "como_conheceu", None)];
        let depois = [cliente("web", "concluido", Some("Maria Souza"))];
        assert_eq!(
            novidades_do_bot(Some(&antes), &depois),
            vec!["Maria Souza concluiu o cadastro pelo chat do site"]
        );
        assert!(novidades_do_bot(Some(&depois), &depois).is_empty());
    }

    #[test]
    fn so_os_eventos_do_bot_mudam_o_cartao() {
        assert!(evento_do_bot(
            &serde_json::json!({ "tipo": "cliente_no_bot" })
        ));
        assert!(evento_do_bot(
            &serde_json::json!({ "tipo": "dados_do_bot" })
        ));
        assert!(!evento_do_bot(
            &serde_json::json!({ "tipo": "foto_chegou" })
        ));
    }

    #[test]
    fn o_resumo_do_aviso_e_o_do_site() {
        let r = resumo_do_aviso(
            "a@x.com",
            &[
                EntregaNoCanal {
                    canal: "whatsapp".into(),
                    enviado: true,
                    motivo: None,
                },
                EntregaNoCanal {
                    canal: "telegram".into(),
                    enviado: true,
                    motivo: None,
                },
                EntregaNoCanal {
                    canal: "instagram".into(),
                    enviado: false,
                    motivo: Some("a janela de 24 horas fechou há 2 dias".into()),
                },
            ],
        );
        assert_eq!(
            r.sucesso,
            "Aviso enviado por e-mail, WhatsApp e Telegram (a@x.com)."
        );
        assert_eq!(
            r.falhas,
            vec!["Instagram ficou de fora: a janela de 24 horas fechou há 2 dias"]
        );
        assert_eq!(
            resumo_do_aviso("a@x.com", &[]).sucesso,
            "Aviso enviado por e-mail (a@x.com)."
        );
    }
}
