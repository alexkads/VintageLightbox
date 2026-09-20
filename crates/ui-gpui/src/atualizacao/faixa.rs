//! A faixa que avisa da versão nova.
//!
//! 🔑 **Faixa, e não modal.** Quem está triando 200 fotos não pode ser
//! interrompido por uma janela que exige clique para sumir — o desfecho
//! conhecido disso é o operador aprender a fechar sem ler, e a próxima que
//! importar de verdade some junto. A faixa fica no rodapé, ocupa uma linha e
//! espera. Foi a escolha do dono: *avisa e pergunta*.
//!
//! ⚠️ **Ela não instala nada sozinha.** O botão é que instala, e o "Depois"
//! guarda a versão como dispensada até a próxima abertura — não para sempre:
//! uma correção que o fotógrafo dispensou uma vez precisa voltar a aparecer.

use std::sync::Arc;

use gpui::{div, prelude::*, px, AnyElement, SharedString, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, Sizable};

use super::porta::Aviso;

/// O que a raiz guarda enquanto a faixa está no ar.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Estado {
    /// O aviso corrente, se houver.
    pub aviso: Option<Aviso>,
    /// Verdadeiro entre o clique em "Atualizar" e a resposta.
    pub instalando: bool,
    /// A versão que o operador mandou esperar **nesta sessão**.
    pub dispensada: Option<String>,
}

impl Estado {
    /// O que a faixa mostra agora — `None` quando não há nada a dizer.
    pub fn visivel(&self) -> Option<&Aviso> {
        match self.aviso.as_ref()? {
            // Dispensada nesta abertura: some da vista, volta na próxima.
            Aviso::Disponivel(nova) if self.dispensada.as_deref() == Some(&nova.versao) => None,
            aviso => Some(aviso),
        }
    }

    /// O que a faixa diz, em uma linha.
    pub fn texto(&self) -> Option<String> {
        Some(match self.visivel()? {
            Aviso::Disponivel(nova) if self.instalando => {
                format!("Baixando a versão {}…", nova.versao)
            }
            Aviso::Disponivel(nova) => match &nova.notas {
                Some(notas) if !notas.trim().is_empty() => {
                    format!("Versão {} disponível — {}", nova.versao, notas.trim())
                }
                _ => format!("Versão {} disponível", nova.versao),
            },
            Aviso::Instalada(versao) => {
                format!("Versão {versao} instalada. Reabra para usá-la.")
            }
            Aviso::Falhou(motivo) => format!("Não consegui atualizar: {motivo}"),
        })
    }
}

/// O que os botões da faixa pedem à raiz.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pedido {
    Instalar,
    Reabrir,
    Dispensar,
}

/// Quem atende os botões da faixa.
///
/// 🔑 Um `Arc`, e não um genérico `Clone`: o que a raiz passa aqui é o
/// `cx.listener` do GPUI, que **não é `Clone`** — e cada botão precisa da sua
/// cópia. O `Arc` resolve os dois de uma vez.
pub type Agir = Arc<dyn Fn(Pedido, &mut Window, &mut gpui::App)>;

/// Desenha a faixa. `agir` recebe o [`Pedido`] de cada botão.
pub fn desenhar(estado: &Estado, cx: &gpui::App, agir: Agir) -> Option<AnyElement> {
    let texto: SharedString = estado.texto()?.into();
    let aviso = estado.visivel()?.clone();

    let mut botoes = div().flex().items_center().gap(px(6.));
    match &aviso {
        Aviso::Disponivel(_) if estado.instalando => {}
        Aviso::Disponivel(_) => {
            let instalar = agir.clone();
            let depois = agir.clone();
            botoes = botoes
                .child(
                    Button::new("atualizar-agora")
                        .label("Atualizar")
                        .xsmall()
                        .primary()
                        .on_click(move |_ev, w, cx| instalar(Pedido::Instalar, w, cx)),
                )
                .child(
                    Button::new("atualizar-depois")
                        .label("Depois")
                        .xsmall()
                        .ghost()
                        .on_click(move |_ev, w, cx| depois(Pedido::Dispensar, w, cx)),
                );
        }
        Aviso::Instalada(_) => {
            let reabrir = agir.clone();
            botoes = botoes.child(
                Button::new("atualizar-reabrir")
                    .label("Reabrir agora")
                    .xsmall()
                    .primary()
                    .on_click(move |_ev, w, cx| reabrir(Pedido::Reabrir, w, cx)),
            );
        }
        Aviso::Falhou(_) => {
            let fechar = agir.clone();
            botoes = botoes.child(
                Button::new("atualizar-fechar")
                    .label("Fechar")
                    .xsmall()
                    .ghost()
                    .on_click(move |_ev, w, cx| fechar(Pedido::Dispensar, w, cx)),
            );
        }
    }

    Some(
        div()
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .px(px(12.))
            .py(px(6.))
            .bg(cx.theme().background)
            .border_t_1()
            .border_color(cx.theme().border)
            .child(div().text_xs().child(texto))
            .child(botoes)
            .into_any_element(),
    )
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::atualizacao::porta::VersaoNova;

    fn nova(versao: &str) -> Aviso {
        Aviso::Disponivel(VersaoNova {
            versao: versao.into(),
            notas: None,
        })
    }

    #[test]
    fn sem_aviso_a_faixa_nao_diz_nada() {
        assert_eq!(Estado::default().texto(), None);
    }

    #[test]
    fn a_versao_dispensada_some_da_vista() {
        let estado = Estado {
            aviso: Some(nova("0.2.0")),
            dispensada: Some("0.2.0".into()),
            ..Default::default()
        };
        assert_eq!(estado.texto(), None);
    }

    /// 🔑 Dispensar a 0.2.0 não pode calar a 0.3.0 — senão uma correção
    /// urgente fica invisível para quem clicou "Depois" uma vez.
    #[test]
    fn dispensar_uma_versao_nao_cala_a_proxima() {
        let estado = Estado {
            aviso: Some(nova("0.3.0")),
            dispensada: Some("0.2.0".into()),
            ..Default::default()
        };
        assert_eq!(estado.texto().as_deref(), Some("Versão 0.3.0 disponível"));
    }

    #[test]
    fn as_notas_entram_na_linha_quando_existem() {
        let estado = Estado {
            aviso: Some(Aviso::Disponivel(VersaoNova {
                versao: "0.2.0".into(),
                notas: Some("  corrige o magenta da tonalização  ".into()),
            })),
            ..Default::default()
        };
        assert_eq!(
            estado.texto().as_deref(),
            Some("Versão 0.2.0 disponível — corrige o magenta da tonalização")
        );
    }

    /// Notas vazias não viram um travessão solto no fim da frase.
    #[test]
    fn notas_em_branco_nao_deixam_travessao() {
        let estado = Estado {
            aviso: Some(Aviso::Disponivel(VersaoNova {
                versao: "0.2.0".into(),
                notas: Some("   ".into()),
            })),
            ..Default::default()
        };
        assert_eq!(estado.texto().as_deref(), Some("Versão 0.2.0 disponível"));
    }

    #[test]
    fn instalando_troca_a_frase_e_tira_os_botoes() {
        let estado = Estado {
            aviso: Some(nova("0.2.0")),
            instalando: true,
            ..Default::default()
        };
        assert_eq!(estado.texto().as_deref(), Some("Baixando a versão 0.2.0…"));
    }

    #[test]
    fn instalada_pede_para_reabrir() {
        let estado = Estado {
            aviso: Some(Aviso::Instalada("0.2.0".into())),
            ..Default::default()
        };
        assert_eq!(
            estado.texto().as_deref(),
            Some("Versão 0.2.0 instalada. Reabra para usá-la.")
        );
    }

    /// A falha aparece: houve um clique esperando resposta.
    #[test]
    fn a_falha_da_instalacao_aparece_na_faixa() {
        let estado = Estado {
            aviso: Some(Aviso::Falhou("assinatura inválida".into())),
            ..Default::default()
        };
        assert_eq!(
            estado.texto().as_deref(),
            Some("Não consegui atualizar: assinatura inválida")
        );
    }
}
