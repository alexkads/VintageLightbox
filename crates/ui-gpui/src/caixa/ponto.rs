//! 🖥️ **O caixa deste balcão** (CX1, CX2…) e o encerramento na troca de
//! operador (dono, 25/11/2026: *"se um outro usuário logar no mesmo caixa de
//! outro um encerramento deverá ser feito"*; perguntado, *fixo na máquina* e
//! *quem chega encerra*).
//!
//! - **A escolha é da máquina**, gravada ao lado do catálogo, por estúdio
//!   (`caixa-do-balcao.json`). Ela se faz no diálogo de abrir, entre os caixas
//!   que o estúdio cadastrou no site; sem escolha, o caixa abre como antes, só
//!   do operador.
//! - 🚨 **Nunca escolhe sozinha.** A migration criou um CX1 em cada estúdio, e o
//!   Canela tem mais de um balcão: fixar o CX1 por conta própria poria o
//!   segundo balcão em "caixa ocupado" logo cedo.
//! - **O ponto ocupado por outra pessoa** volta da API como 409 com a frase
//!   *"O CX1 está aberto por … Encerre-o antes de abrir o seu."*: o diálogo de
//!   abrir oferece o encerramento, que é o fechamento às cegas de sempre, com o
//!   motivo obrigatório, pelo `POST /pos-venda/caixa/encerrar`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme};
use gpui_kit::{div, prelude::*, px, ClickEvent, Context, Div, Window};
use serde_json::json;

use super::dados::{self, PontoDoCaixa};
use super::dialogos::{Dialogo, Encerramento, TipoDeDialogo};
use super::tela::{codificar, Caixa, TipoDeRecado};
use crate::estilo;

/// O pedaço da frase da API que diz "o ponto é de outra pessoa".
const PONTO_OCUPADO: &str = "Encerre-o antes de abrir o seu";

#[cfg(not(test))]
fn arquivo() -> PathBuf {
    infrastructure::paths::AppPaths::catalog_root().join("caixa-do-balcao.json")
}

/// 🚨 Nos testes, um arquivo por processo na pasta temporária: gravar no
/// catálogo do fotógrafo durante o `cargo test` é o que as outras telas evitam.
#[cfg(test)]
fn arquivo() -> PathBuf {
    std::env::temp_dir().join(format!("vlb-caixa-do-balcao-{}.json", std::process::id()))
}

fn escolhas() -> BTreeMap<String, String> {
    std::fs::read_to_string(arquivo())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// O id do caixa em que esta máquina foi fixada, naquele estúdio.
pub fn ponto_guardado(estudio: &str) -> Option<String> {
    escolhas().remove(estudio)
}

fn guardar_ponto(estudio: &str, ponto: Option<&str>) {
    let mut todas = escolhas();
    match ponto {
        Some(p) => {
            todas.insert(estudio.to_string(), p.to_string());
        }
        None => {
            todas.remove(estudio);
        }
    }
    if let Ok(texto) = serde_json::to_string_pretty(&todas) {
        let _ = std::fs::write(arquivo(), texto);
    }
}

/// A frase da API é a do ponto ocupado por outra pessoa?
pub fn e_ponto_ocupado(erro: &str) -> bool {
    dados::status_do_erro(erro) == Some(409) && erro.contains(PONTO_OCUPADO)
}

impl Caixa {
    /// Os caixas do estúdio, quando o diálogo de abrir aparece. Vão pelo canal
    /// das gravações para a colheita esperar a resposta.
    pub(super) fn pedir_os_pontos(&mut self, cx: &mut Context<Self>) {
        let (Some(estudio), Some(conta)) = (
            self.estudio_do_caixa().map(str::to_string),
            self.sessao.clone(),
        ) else {
            return;
        };
        self.gravando += 1;
        self.publicador.pedir_json(
            conta,
            crate::pos_venda::porta::PedidoJson::ler(
                "pontos",
                format!("/pos-venda/caixa/pontos?estudio_id={}", codificar(&estudio)),
            ),
            self.gravacoes.0.clone(),
        );
        self.acompanhar(cx);
    }

    /// Os caixas chegaram (ou a API de antes deles respondeu 404: nenhum).
    pub(super) fn receber_os_pontos(&mut self, resultado: Result<serde_json::Value, String>) {
        if let Some(v) = self.vista.as_mut() {
            v.pontos = resultado.and_then(dados::ler_pontos).unwrap_or_default();
        }
    }

    /// O caixa em que este balcão está fixado — só se ele continua ativo.
    pub(super) fn ponto_do_balcao(&self) -> Option<&PontoDoCaixa> {
        let v = self.vista.as_ref()?;
        let guardado = ponto_guardado(v.estudio_id.as_deref()?)?;
        v.pontos.iter().find(|p| p.id == guardado)
    }

    fn escolher_o_ponto(&mut self, ponto: Option<String>, cx: &mut Context<Self>) {
        if let Some(estudio) = self.estudio_do_caixa().map(str::to_string) {
            guardar_ponto(&estudio, ponto.as_deref());
        }
        if let Some(Dialogo::Abrir(form)) = self.dialogo.as_mut() {
            form.ponto_ocupado = None;
        }
        cx.notify();
    }

    /// "Caixa deste balcão": as fichas dos caixas do estúdio, e "Sem caixa
    /// fixo". Sem caixa cadastrado, a linha não aparece.
    pub(super) fn linha_do_ponto(&self, cx: &mut Context<Self>) -> Option<Div> {
        let pontos = self.vista.as_ref()?.pontos.clone();
        if pontos.is_empty() {
            return None;
        }
        let atual = self.ponto_do_balcao().map(|p| p.id.clone());
        let mut fichas = h_flex().gap(px(8.)).flex_wrap().child(
            estilo::chip("caixa-ponto-nenhum", atual.is_none())
                .child("Sem caixa fixo")
                .on_click(cx.listener(|t, _: &ClickEvent, _, cx| t.escolher_o_ponto(None, cx))),
        );
        for p in pontos {
            let id = p.id.clone();
            fichas = fichas.child(
                estilo::chip(
                    format!("caixa-ponto-{}", p.nome.to_lowercase()),
                    atual.as_deref() == Some(p.id.as_str()),
                )
                .child(p.nome.clone())
                .on_click(cx.listener(move |t, _: &ClickEvent, _, cx| {
                    t.escolher_o_ponto(Some(id.clone()), cx)
                })),
            );
        }
        Some(
            v_flex()
                .gap(px(6.))
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .child("Caixa deste balcão"),
                )
                .child(fichas)
                .child(div().text_xs().text_color(cx.theme().muted_foreground).child(
                    "Fica gravado neste computador. Quem entrar aqui com o caixa de outra pessoa aberto encerra o dela antes de abrir o seu.",
                )),
        )
    }

    /// "Encerrar este caixa": sai a abertura, entra o fechamento às cegas do
    /// caixa de quem estava no ponto.
    pub(super) fn abrir_o_encerramento(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let frase = match self.dialogo.as_ref() {
            Some(Dialogo::Abrir(form)) => form.ponto_ocupado.clone(),
            _ => None,
        };
        let (Some(frase), Some(ponto)) = (frase, self.ponto_do_balcao().map(|p| p.id.clone()))
        else {
            return;
        };
        self.dialogo = None;
        self.abrir_dialogo(TipoDeDialogo::Fechar, window, cx);
        if let Some(Dialogo::Fechar(form)) = self.dialogo.as_mut() {
            form.encerrar = Some(Encerramento { ponto, frase });
        }
        cx.notify();
    }

    /// `POST /pos-venda/caixa/encerrar`: a contagem às cegas e o motivo.
    pub(super) fn enviar_o_encerramento(&mut self, cx: &mut Context<Self>) {
        let local = self.local_do_caixa();
        let Some((lido, motivo, chave, ponto)) = self.ler_o_encerramento(cx) else {
            return;
        };
        let contado = match lido {
            Ok(c) => c,
            Err(erro) => {
                self.erro_no_fechamento(erro, cx);
                return;
            }
        };
        if motivo.is_empty() {
            self.erro_no_fechamento("Diga por que você está encerrando este caixa.".into(), cx);
            return;
        }
        self.marcar_o_fechamento_enviando();
        self.gravar(
            "encerrar",
            "POST",
            "/pos-venda/caixa/encerrar".into(),
            json!({
                "ponto_de_caixa_id": ponto,
                "contado": dados::contado_json(&contado),
                "motivo": motivo,
                // A API só grava (e só exige) o atraso se o caixa encerrado
                // passou do corte.
                "motivo_do_atraso": motivo,
                "chave": chave,
                "local": biblioteca_core::local::texto_do_caixa(local),
            }),
            cx,
        );
        cx.notify();
    }

    /// A resposta do encerramento: o resultado à vista, e o caixa relido —
    /// agora o ponto está livre para abrir o seu.
    pub(super) fn receber_o_encerramento(
        &mut self,
        resultado: Result<serde_json::Value, String>,
        cx: &mut Context<Self>,
    ) {
        match resultado {
            Ok(valor) => {
                self.mostrar_o_resultado_do_fechamento(dados::ler_caixa(valor).ok());
                self.avisar(
                    "Caixa encerrado. Agora abra o seu (F8).",
                    TipoDeRecado::Sucesso,
                    cx,
                );
                self.recarregar(cx);
            }
            Err(erro) => {
                self.erro_no_fechamento(
                    dados::mensagem_do_erro(&erro, "Não foi possível encerrar o caixa."),
                    cx,
                );
            }
        }
    }
}
