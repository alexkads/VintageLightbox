//! 💛 As favoritas da Revelação no perfil do usuário, na API.
//!
//! Dono, 2026-10-02: *"A favoritação de presets precisa gravar no banco de
//! dados no perfil do usuário da API e não só localmente."* A lista é a mesma
//! do site (`GET`/`PUT /revelacao/favoritas`): favoritar aqui aparece lá, e em
//! outro balcão com a mesma conta.
//!
//! A Revelação não fala com a API — ela pede à raiz
//! ([`PedidoDaRevelacao::GuardarFavoritas`]), como o "Salvar na galeria". O
//! JSON ao lado do catálogo (`ordem-dos-presets.json`) fica como a última lista
//! que veio: a coluna abre com ela, e a Nova sessão a lê sem rede.

use gpui_kit::Context;
use serde_json::{json, Value};

use super::Aplicativo;
use crate::pos_venda::porta::{PedidoJson, Recado};
use crate::revelacao::presets::ordem::FavoritasDoPerfil;

const CAMINHO: &str = "/revelacao/favoritas";
const LIDAS: &str = "favoritas-lidas";
const GRAVADAS: &str = "favoritas-gravadas";

/// As chaves de uma resposta `{ "chaves": [...] }`.
fn chaves_de(valor: &Value) -> Option<Vec<String>> {
    valor.get("chaves")?.as_array().map(|itens| {
        itens
            .iter()
            .filter_map(|c| c.as_str().map(str::to_string))
            .collect()
    })
}

impl Aplicativo {
    /// Pede as favoritas do perfil — ao entrar na conta.
    pub(super) fn carregar_favoritas(&mut self, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        self.favoritas_mexidas = false;
        self.publicador.pedir_json(
            sessao,
            PedidoJson::ler(LIDAS, CAMINHO),
            self.recados_das_favoritas.0.clone(),
        );
        self.esperar_favoritas(cx);
    }

    /// Manda ao perfil a lista que a Revelação tem agora.
    pub(super) fn guardar_favoritas(&mut self, cx: &mut Context<Self>) {
        // Mexeu antes de a leitura voltar: a leitura chega velha e não pode
        // desfazer o coração.
        self.favoritas_mexidas = true;
        let chaves = self.revelacao.read(cx).favoritas();
        self.enviar_favoritas(chaves, cx);
    }

    fn enviar_favoritas(&mut self, chaves: Vec<String>, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        self.publicador.pedir_json(
            sessao,
            PedidoJson::gravar(GRAVADAS, "PUT", CAMINHO, json!({ "chaves": chaves })),
            self.recados_das_favoritas.0.clone(),
        );
        self.esperar_favoritas(cx);
    }

    /// Colhe as respostas enquanto houver alguma a caminho (até 30 s).
    fn esperar_favoritas(&mut self, cx: &mut Context<Self>) {
        self.favoritas_a_caminho += 1;
        if self._favoritas.is_some() {
            return;
        }
        self._favoritas = Some(cx.spawn(async move |raiz, cx| {
            for _ in 0..300 {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
                let Ok(acabou) = raiz.update(cx, |raiz, cx| raiz.colher_favoritas(cx)) else {
                    return;
                };
                if acabou {
                    break;
                }
            }
            let _ = raiz.update(cx, |raiz, _| {
                raiz._favoritas = None;
                raiz.favoritas_a_caminho = 0;
            });
        }));
    }

    /// Trata o que chegou. `true` quando não há mais nada a caminho.
    pub(crate) fn colher_favoritas(&mut self, cx: &mut Context<Self>) -> bool {
        while let Ok(recado) = self.recados_das_favoritas.1.try_recv() {
            let Recado::Json { rotulo, resultado } = recado else {
                continue;
            };
            self.favoritas_a_caminho = self.favoritas_a_caminho.saturating_sub(1);
            match (rotulo, resultado) {
                (LIDAS, Ok(valor)) => {
                    let Some(do_perfil) = chaves_de(&valor) else {
                        continue;
                    };
                    if self.favoritas_mexidas {
                        continue;
                    }
                    let ordem = crate::revelacao::presets::ordem::ler();
                    match ordem.favoritas_do_perfil(do_perfil) {
                        FavoritasDoPerfil::Subir(chaves) => self.enviar_favoritas(chaves, cx),
                        FavoritasDoPerfil::Adotar(chaves) => self
                            .revelacao
                            .update(cx, |tela, cx| tela.receber_favoritas(chaves, cx)),
                    }
                }
                (GRAVADAS, Ok(valor)) => {
                    // A guardada é a que o servidor devolveu (sem repetidas):
                    // ela vale aqui também, e marca a migração como feita.
                    if let Some(chaves) = chaves_de(&valor) {
                        self.revelacao
                            .update(cx, |tela, cx| tela.receber_favoritas(chaves, cx));
                    }
                }
                (LIDAS, Err(erro)) => {
                    // Sem rede: fica a última lista que veio.
                    crate::telemetria::avisar!("⚠️ [Favoritas] {CAMINHO}: {erro}");
                }
                (_, Err(erro)) => {
                    crate::telemetria::avisar!("⚠️ [Favoritas] {CAMINHO}: {erro}");
                    self.avisar_falha(format!("As favoritas não foram guardadas: {erro}"), cx);
                }
                _ => {}
            }
        }
        self.favoritas_a_caminho == 0
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn le_as_chaves_da_resposta_e_recusa_o_que_nao_e_lista() {
        assert_eq!(
            chaves_de(&json!({ "chaves": ["sistema:sepia", 3, "abc"] })),
            Some(vec!["sistema:sepia".to_string(), "abc".to_string()])
        );
        assert_eq!(chaves_de(&json!({ "chaves": [] })), Some(vec![]));
        assert_eq!(chaves_de(&json!({ "outra": 1 })), None);
    }
}
