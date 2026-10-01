//! 🎞️ O painel Básico no formato do Lightroom (dono, 01/10/2026, com o print
//! do painel do Lightroom em português: *"Eu quero o painel Básico com essa
//! configuração"*).
//!
//! De cima para baixo:
//!
//! - **Automático** e **P&B** — o tom automático e o tratamento em preto e
//!   branco. Com o P&B ligado, o painel HSL dá lugar ao **P&B** (a Mistura de
//!   preto e branco), e a Vibração e a Saturação ficam apagadas;
//! - **Perfil**: Cor ou Monocromático — o mesmo interruptor do P&B, como no
//!   Lightroom (escolher "Monocromático" liga o P&B, e "Cor" o desliga);
//! - o **conta-gotas** e o **EB** (Como fotografado, Automático,
//!   Personalizado), e a Temperatura e o Colorir;
//! - **Tom**: Exposição e Contraste; Realces, Sombras, Brancos e Pretos;
//! - **Presença**: Textura, Claridade e Desembaçar; Vibração e Saturação.
//!
//! Os números são os do Lightroom (−100 a 100), e não os do campo do motor —
//! a conversão está em `controles.rs` (`lightroom!`).
//!
//! ⚠️ **O HDR do Lightroom não está aqui**: o motor não revela em faixa
//! dinâmica alta, e um botão que não faz nada seria pior que nenhum. O mesmo
//! vale para o navegador de perfis (a grade ao lado do Perfil): os perfis
//! criativos da Adobe são tabelas de cor que não temos.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{h_flex, ActiveTheme, Disableable, Icon, Selectable as _, Sizable};
use gpui_kit::{div, prelude::*, px, AnyElement, Context, MouseDownEvent, SharedString, Window};

use super::super::{Aberta, Revelacao};
use super::titulo;
use crate::recursos::Icone;
use crate::revelacao::balanco;
use crate::revelacao::controles::Secao;
use crate::revelacao::processador::Ajustes;

/// Os perfis da lista, na ordem do `bw_ativo`: 0 é Cor, 1 é Monocromático.
pub(in crate::revelacao) const PERFIS: [&str; 2] = ["Cor", "Monocromático"];

/// O que a lista do EB mostra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::revelacao) enum Balanco {
    /// Temperatura e Colorir no neutro: a luz que a câmera gravou.
    ComoFotografado,
    /// O que o "Automático" da lista escolheu para esta foto, intocado.
    Automatico,
    /// Qualquer outro par — mexido à mão, pelo conta-gotas ou por preset.
    Personalizado,
}

impl Balanco {
    fn rotulo(self) -> &'static str {
        match self {
            Balanco::ComoFotografado => "Como fotografado",
            Balanco::Automatico => "Automático",
            Balanco::Personalizado => "Personalizado",
        }
    }
}

/// O balanço de agora, para o rótulo da lista. `automatico` é o par que o
/// "Automático" escolheu nesta foto, se escolheu.
pub(in crate::revelacao) fn balanco_de(
    ajustes: &Ajustes,
    automatico: Option<(f32, f32)>,
) -> Balanco {
    let par = (ajustes.temperature, ajustes.tint);
    if par == (0.0, 0.0) {
        Balanco::ComoFotografado
    } else if automatico == Some(par) {
        Balanco::Automatico
    } else {
        Balanco::Personalizado
    }
}

/// Os rótulos de cada bloco, na ordem do Lightroom. Os grupos de dentro de
/// "Tom" e "Presença" ganham um respiro entre si, como lá.
const BALANCO: [&str; 2] = ["Temperatura", "Colorir"];
const TOM: [&[&str]; 2] = [
    &["Exposição", "Contraste"],
    &["Realces", "Sombras", "Brancos", "Pretos"],
];
const PRESENCA: [&[&str]; 2] = [
    &["Textura", "Claridade", "Desembaçar"],
    &["Vibração", "Saturação"],
];

impl Revelacao {
    /// O conteúdo do painel Básico.
    pub(super) fn painel_basico(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut dentro = vec![
            self.tratamento(cx),
            self.perfil(cx),
            self.balanco_de_branco(cx),
        ];
        dentro.extend(BALANCO.iter().map(|r| self.linha_do_basico(r, cx)));
        for (nome, blocos) in [("Tom", TOM), ("Presença", PRESENCA)] {
            dentro.push(titulo(nome, true, cx));
            for (n, bloco) in blocos.iter().enumerate() {
                if n > 0 {
                    dentro.push(div().h(px(4.)).into_any_element());
                }
                dentro.extend(bloco.iter().map(|r| self.linha_do_basico(r, cx)));
            }
        }
        dentro
    }

    /// O índice do controle do Básico com este rótulo.
    fn indice_do_basico(&self, rotulo: &str) -> usize {
        self.controles
            .iter()
            .position(|c| c.definicao.secao == Secao::Basico && c.definicao.rotulo == rotulo)
            .unwrap_or_else(|| panic!("o Básico não tem `{rotulo}`"))
    }

    fn linha_do_basico(&self, rotulo: &str, cx: &mut Context<Self>) -> AnyElement {
        let i = self.indice_do_basico(rotulo);
        self.linha_em_linha(i, &self.controles[i], cx)
    }

    /// "Automático" e "P&B", encostados à direita como no Lightroom.
    fn tratamento(&self, cx: &mut Context<Self>) -> AnyElement {
        let ligado = self.controles_ligados();
        let pronto = matches!(self.aberta.as_ref(), Some(Aberta { bruta: Some(_), .. })) && ligado;
        let pb = self.ajustes.bw_ativo != 0.0;
        h_flex()
            .justify_end()
            .gap(px(4.))
            .child(
                Button::new("tom-automatico")
                    .label("Automático")
                    .ghost()
                    .xsmall()
                    .disabled(!pronto)
                    .on_click(cx.listener(|tela, _ev, window, cx| {
                        tela.tom_automatico(window, cx);
                    })),
            )
            .child(
                Button::new("tratamento-pb")
                    .label("P&B")
                    .xsmall()
                    .when(!pb, |b| b.ghost())
                    .selected(pb)
                    .disabled(!ligado)
                    .on_click(cx.listener(move |tela, _ev, window, cx| {
                        tela.definir_pb(!pb, window, cx);
                    })),
            )
            .into_any_element()
    }

    /// Liga ou desliga o P&B — o botão e o Perfil. Um gesto inteiro, como um
    /// preset: entra no histórico e grava na hora.
    pub(crate) fn definir_pb(&mut self, ligar: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.controles_ligados() || (self.ajustes.bw_ativo != 0.0) == ligar {
            return;
        }
        self.gesto_discreto(|a| a.bw_ativo = if ligar { 1.0 } else { 0.0 }, window, cx);
    }

    /// "Perfil:" e a lista Cor / Monocromático.
    fn perfil(&self, cx: &mut Context<Self>) -> AnyElement {
        let atual = PERFIS[usize::from(self.ajustes.bw_ativo != 0.0)];
        let tela = cx.entity().downgrade();
        let lista = lista_suspensa("perfil", atual, !self.controles_ligados()).dropdown_menu(
            move |menu, _window, _cx| {
                PERFIS.iter().enumerate().fold(menu, |menu, (i, nome)| {
                    let tela = tela.clone();
                    menu.item(PopupMenuItem::new(*nome).checked(*nome == atual).on_click(
                        move |_, window, cx| {
                            let _ = tela.update(cx, |tela, cx| tela.definir_pb(i == 1, window, cx));
                        },
                    ))
                })
            },
        );
        h_flex()
            .gap(px(8.))
            .text_xs()
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child("Perfil:"),
            )
            .child(lista)
            .into_any_element()
    }

    /// O conta-gotas, "EB:" e a lista do balanço.
    fn balanco_de_branco(&self, cx: &mut Context<Self>) -> AnyElement {
        let ligado = self.controles_ligados();
        let pronto = matches!(self.aberta.as_ref(), Some(Aberta { bruta: Some(_), .. })) && ligado;
        let atual = balanco_de(&self.ajustes, self.estado_do_painel.balanco_automatico);
        let tela = cx.entity().downgrade();
        let lista = lista_suspensa("balanco", atual.rotulo(), !ligado).dropdown_menu_with_anchor(
            gpui_kit::Anchor::TopRight,
            move |menu, _window, _cx| {
                [
                    Balanco::ComoFotografado,
                    Balanco::Automatico,
                    Balanco::Personalizado,
                ]
                .into_iter()
                .fold(menu, |menu, opcao| {
                    let tela = tela.clone();
                    menu.item(
                        PopupMenuItem::new(opcao.rotulo())
                            .checked(opcao == atual)
                            // "Personalizado" é o que sobra: escolhê-lo não
                            // tem para onde levar a foto.
                            .disabled(opcao == Balanco::Personalizado)
                            .on_click(move |_, window, cx| {
                                let _ = tela.update(cx, |tela, cx| {
                                    tela.escolher_balanco(opcao, window, cx)
                                });
                            }),
                    )
                })
            },
        );
        let armado = self.estado_do_painel.conta_gotas;
        h_flex()
            .gap(px(8.))
            .text_xs()
            .child(
                crate::estilo::botao_icone_pequeno("conta-gotas", Icone::Pipette)
                    .selected(armado)
                    .disabled(!pronto)
                    .tooltip("Clique num ponto neutro da foto (Esc cancela)")
                    .on_click(cx.listener(|tela, _ev, _window, cx| {
                        tela.armar_conta_gotas(!tela.estado_do_painel.conta_gotas, cx);
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .text_center()
                    .text_color(cx.theme().muted_foreground)
                    .child("EB:"),
            )
            .child(lista)
            .into_any_element()
    }

    /// O que a lista do EB faz.
    fn escolher_balanco(&mut self, opcao: Balanco, window: &mut Window, cx: &mut Context<Self>) {
        if !self.controles_ligados() {
            return;
        }
        let par = match opcao {
            Balanco::ComoFotografado => (0.0, 0.0),
            Balanco::Automatico => {
                let Some(Aberta {
                    bruta: Some(bruta), ..
                }) = self.aberta.as_ref()
                else {
                    return;
                };
                let Some(media) = balanco::media_neutra(bruta) else {
                    return;
                };
                let par = balanco::neutralizar(media, &self.ajustes);
                self.estado_do_painel.balanco_automatico = Some(par);
                par
            }
            Balanco::Personalizado => return,
        };
        self.gesto_discreto(|a| (a.temperature, a.tint) = par, window, cx);
    }

    /// Arma (ou desarma) o conta-gotas: o próximo clique na foto é dele.
    pub(crate) fn armar_conta_gotas(&mut self, armar: bool, cx: &mut Context<Self>) {
        self.estado_do_painel.conta_gotas = armar && self.controles_ligados();
        cx.notify();
    }

    /// O `Esc` com o conta-gotas armado: desarma, e diz se havia o que
    /// desarmar (senão o `Esc` segue adiante e sai da Revelação).
    pub fn esc_do_conta_gotas(&mut self, cx: &mut Context<Self>) -> bool {
        let armado = self.estado_do_painel.conta_gotas;
        if armado {
            self.armar_conta_gotas(false, cx);
        }
        armado
    }

    /// O clique do conta-gotas na foto: o ponto vira neutro, e o conta-gotas
    /// desarma — como no Lightroom com "Descartar automaticamente".
    pub(in crate::revelacao::tela) fn conta_gotas_apertar(
        &mut self,
        evento: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let p = self.ponto_na_area(evento.position);
        let Some(q) = self.foto_do_ponto(p) else {
            return;
        };
        self.pegar_o_branco(q, window, cx);
    }

    /// Neutraliza a cor da foto crua em `q` (0–1 da foto inteira).
    pub(crate) fn pegar_o_branco(
        &mut self,
        q: [f32; 2],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(Aberta {
            bruta: Some(bruta), ..
        }) = self.aberta.as_ref()
        else {
            return;
        };
        let Some(cor) = balanco::amostra(bruta, q, 5) else {
            return;
        };
        let par = balanco::neutralizar(cor, &self.ajustes);
        self.estado_do_painel.conta_gotas = false;
        self.gesto_discreto(|a| (a.temperature, a.tint) = par, window, cx);
    }
}

/// O botão de lista do Lightroom ("Monocromático ⇕"): o nome escolhido e as
/// setas, sem moldura.
fn lista_suspensa(id: &'static str, rotulo: &'static str, desligada: bool) -> Button {
    Button::new(id).ghost().xsmall().disabled(desligada).child(
        h_flex()
            .gap(px(4.))
            .child(SharedString::from(rotulo))
            .child(Icon::new(Icone::ChevronsUpDown).size(px(12.))),
    )
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_rotulo_do_eb_segue_o_par() {
        let mut a = Ajustes::default();
        assert_eq!(balanco_de(&a, None), Balanco::ComoFotografado);
        a.temperature = 1.5;
        assert_eq!(balanco_de(&a, None), Balanco::Personalizado);
        assert_eq!(balanco_de(&a, Some((1.5, 0.0))), Balanco::Automatico);
        a.tint = 0.2;
        assert_eq!(balanco_de(&a, Some((1.5, 0.0))), Balanco::Personalizado);
    }
}
