//! 🎛️ A barra de opções do Photoshop: uma faixa fixa embaixo dos menus, com
//! as opções da ferramenta na mão — ou da transformação, da malha e do
//! Liquidificar, com Cancelar e Aplicar presos à direita.
//!
//! 🔑 **A altura não muda** ao trocar de ferramenta: o palco fica no lugar
//! (uma faixa que aparecia e sumia empurrava a foto, e as marcas da caixa
//! saíam desenhadas com a medida velha). Numa janela estreita o meio rola de
//! lado; a ferramenta à esquerda e Cancelar/Aplicar à direita não saem da
//! vista.
//!
//! 🔑 **Os controles são os estados que já existiam** (`SliderState`,
//! `SelectState`, os campos): a barra e o painel Pincel mostram o mesmo
//! valor, nunca cópias. Só aparece o que a ferramenta faz de verdade.

use gpui_kit::component::input::InputState;
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::popover::Popover;
use gpui_kit::component::select::Select;
use gpui_kit::component::slider::SliderState;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, Context, Entity, SharedString};

use super::aparencia::{self, medida};
use super::ferramentas::{def_de, na_plataforma, nome_com_letra};
use super::{nome_do_modo, Auxiliar, EditorDeFoto, Item, Modificacao, TipoDeEstilo, MODOS};
use crate::recursos::Icone;
use crate::revelacao::zoom::Nivel;
use editor_core::{Estilo, Ferramenta, Sessao};

/// Um texto apagado de rótulo.
fn rotulo(texto: impl Into<SharedString>, cx: &gpui_kit::App) -> gpui_kit::Div {
    div()
        .flex_shrink_0()
        .text_xs()
        .text_color(aparencia::cores(cx).apagado)
        .child(texto.into())
}

fn separador_da_faixa(cx: &gpui_kit::App) -> gpui_kit::Div {
    div()
        .flex_shrink_0()
        .w(px(1.))
        .h(px(18.))
        .bg(aparencia::cores(cx).borda)
}

/// A ajuda longa de um modo: um "?" com a dica, em vez de texto fixo na
/// faixa.
fn ajuda(id: &'static str, texto: &'static str) -> AnyElement {
    crate::estilo::botao_icone(id, Icone::Info, 22., 13.)
        .tooltip(na_plataforma(texto))
        .into_any_element()
}

impl EditorDeFoto {
    /// Um valor que abre um slider num popover ("Opacidade: 100% ▾") — o
    /// mesmo `SliderState` do painel Pincel.
    fn valor_com_slider(
        &self,
        id: &'static str,
        nome: &'static str,
        valor: String,
        estado: &Entity<SliderState>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let estado = estado.clone();
        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(medida::VAO_MIUDO))
            .child(rotulo(format!("{nome}:"), cx))
            .child(
                Popover::new(id)
                    .anchor(gpui_kit::Anchor::TopLeft)
                    .trigger(
                        crate::estilo::botao_contorno_pequeno(id, cx)
                            .label(format!("{valor} ▾"))
                            .tooltip(nome),
                    )
                    .content(move |_, _window, _cx| {
                        div()
                            .w(px(200.))
                            .h(px(20.))
                            .child(crate::estilo::slider(&estado))
                    }),
            )
            .into_any_element()
    }

    /// O pincel: o círculo com o diâmetro, e tamanho e dureza num popover
    /// (o "Brush Preset picker" do Photoshop).
    pub(super) fn pincel_em_popover(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.sessao().map(|s| s.pincel).unwrap_or_default();
        let (tamanho, dureza) = (self.tamanho.clone(), self.dureza.clone());
        let ed = cx.entity().downgrade();
        Popover::new("editor-tamanho-do-pincel")
            .anchor(gpui_kit::Anchor::TopLeft)
            .trigger(
                crate::estilo::botao_contorno_pequeno("editor-tamanho-do-pincel", cx)
                    .label(format!("● {:.0} px ▾", p.raio * 2.0))
                    .tooltip(na_plataforma(
                        "Tamanho e dureza do pincel — [ ] tamanho, { } dureza",
                    )),
            )
            .content(move |_, _window, cx| {
                let (raio, dureza_agora) = ed
                    .upgrade()
                    .and_then(|ed| {
                        ed.read(cx)
                            .sessao()
                            .map(|s| (s.pincel.raio, s.pincel.dureza))
                    })
                    .unwrap_or((10.0, 1.0));
                let apagado = aparencia::cores(cx).apagado;
                let linha = |nome: &'static str, valor: String| {
                    div()
                        .flex()
                        .justify_between()
                        .text_xs()
                        .text_color(apagado)
                        .child(nome)
                        .child(valor)
                };
                div()
                    .w(px(220.))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(linha("Tamanho  [ ]", format!("{:.0} px", raio * 2.0)))
                    .child(
                        div()
                            .h(px(20.))
                            .debug_selector(|| "editor-tamanho".into())
                            .child(crate::estilo::slider(&tamanho)),
                    )
                    .child(linha(
                        "Dureza  { }",
                        format!("{:.0}%", dureza_agora * 100.0),
                    ))
                    .child(
                        div()
                            .h(px(20.))
                            .debug_selector(|| "editor-dureza".into())
                            .child(crate::estilo::slider(&dureza)),
                    )
            })
            .into_any_element()
    }

    /// Um Select das opções, numa largura fixa.
    fn select_da_faixa(
        &self,
        seletor: &'static str,
        largura: f32,
        estado: &Entity<
            gpui_kit::component::select::SelectState<Vec<crate::sessoes::filtros_da_lista::Opcao>>,
        >,
    ) -> AnyElement {
        div()
            .flex_shrink_0()
            .w(px(largura))
            .debug_selector(move || seletor.into())
            .child(crate::estilo::campo_pequeno(Select::new(estado).xsmall()))
            .into_any_element()
    }

    fn caixa_de_marcar(
        &self,
        id: &'static str,
        nome: &'static str,
        ligada: bool,
        fazer: fn(&mut EditorDeFoto, &mut Context<EditorDeFoto>),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .flex_shrink_0()
            .debug_selector(move || id.into())
            .child(
                gpui_kit::component::checkbox::Checkbox::new(id)
                    .xsmall()
                    .label(nome)
                    .checked(ligada)
                    .on_click(cx.listener(move |ed, _: &bool, _, cx| fazer(ed, cx))),
            )
            .into_any_element()
    }

    /// As opções de quem pinta: pincel, borracha, carimbo, recuperação,
    /// correção, desfoque, nitidez, subexposição e superexposição.
    fn opcoes_de_pintura(&self, f: Option<Ferramenta>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let p = self.sessao().map(|s| s.pincel).unwrap_or_default();
        let mut v = vec![
            div()
                .flex_shrink_0()
                .w(px(150.))
                .debug_selector(|| "editor-predefinicao".into())
                .child(crate::estilo::campo_pequeno(
                    Select::new(&self.seletor_de_predefinicao)
                        .xsmall()
                        .placeholder("Predefinição"),
                ))
                .into_any_element(),
            self.pincel_em_popover(cx),
        ];
        if matches!(f, Some(Ferramenta::Pincel | Ferramenta::Carimbo)) {
            v.push(rotulo("Modo:", cx).into_any_element());
            v.push(self.select_da_faixa(
                "editor-modo-da-ferramenta",
                130.,
                &self.seletor_do_modo_da_ferramenta,
            ));
        }
        if matches!(
            f,
            Some(Ferramenta::Subexposicao(_) | Ferramenta::Superexposicao(_))
        ) {
            v.push(rotulo("Faixa:", cx).into_any_element());
            v.push(self.select_da_faixa("editor-faixa", 120., &self.seletor_de_faixa));
        }
        let opacidade = match f {
            Some(Ferramenta::Subexposicao(_) | Ferramenta::Superexposicao(_)) => Some("Exposição"),
            Some(Ferramenta::Desfoque | Ferramenta::Nitidez) => Some("Força"),
            Some(
                Ferramenta::Pincel
                | Ferramenta::Borracha
                | Ferramenta::Carimbo
                | Ferramenta::Misturador,
            ) => Some("Opacidade"),
            _ => None,
        };
        if let Some(nome) = opacidade {
            v.push(separador_da_faixa(cx).into_any_element());
            v.push(self.valor_com_slider(
                "editor-opacidade",
                nome,
                format!("{:.0}%", p.opacidade * 100.0),
                &self.opacidade,
                cx,
            ));
        }
        if f == Some(Ferramenta::Misturador) {
            v.extend(self.opcoes_do_misturador(cx));
        }
        if matches!(
            f,
            Some(
                Ferramenta::Pincel
                    | Ferramenta::Borracha
                    | Ferramenta::Carimbo
                    | Ferramenta::Misturador
            )
        ) {
            v.push(self.valor_com_slider(
                "editor-fluxo",
                "Fluxo",
                format!("{:.0}%", p.fluxo * 100.0),
                &self.fluxo,
                cx,
            ));
        }
        if matches!(f, Some(Ferramenta::Pincel | Ferramenta::Borracha)) {
            v.push(self.valor_com_slider(
                "editor-suavizacao",
                "Suavização",
                format!("{:.0}%", p.suavizacao * 100.0),
                &self.suavizacao_do_pincel,
                cx,
            ));
        }
        if f.is_some_and(Ferramenta::copia_da_origem) {
            let opcoes = self.sessao().map(|s| s.carimbo).unwrap_or_default();
            v.push(separador_da_faixa(cx).into_any_element());
            v.push(rotulo("Amostra:", cx).into_any_element());
            v.push(self.select_da_faixa(
                "editor-amostra-do-carimbo",
                170.,
                &self.seletor_da_amostra_do_carimbo,
            ));
            v.push(self.caixa_de_marcar(
                "editor-carimbo-alinhado",
                "Alinhado",
                opcoes.alinhado,
                |ed, cx| ed.alternar_carimbo_alinhado(cx),
                cx,
            ));
            v.push(self.caixa_de_marcar(
                "editor-carimbo-sobreposicao",
                "Mostrar a origem",
                self.sobreposicao_do_carimbo,
                |ed, cx| ed.alternar_sobreposicao_do_carimbo(cx),
                cx,
            ));
        }
        if f == Some(Ferramenta::Recuperacao) {
            v.push(self.valor_com_slider(
                "editor-difusao-da-recuperacao",
                "Difusão",
                format!("{}", p.difusao),
                &self.difusao_da_recuperacao,
                cx,
            ));
        }
        // As opções avançadas (espaçamento) moram no painel Pincel.
        v.push(separador_da_faixa(cx).into_any_element());
        v.push(
            crate::estilo::botao_icone(
                "editor-abrir-painel-pincel",
                Icone::SlidersHorizontal,
                24.,
                14.,
            )
            .tooltip("Configurações do pincel (painel Pincel)")
            .on_click(cx.listener(|ed, _, window, cx| {
                ed.mostrar_painel(super::area_de_trabalho::QualPainel::Pincel, window, cx)
            }))
            .into_any_element(),
        );
        v
    }

    /// O Pincel misturador: a ponta (o reservatório sobre a sujeira) com o
    /// menu do Photoshop (Carregar, Limpar, e "a cada traço"), umidade,
    /// carga e mistura, e "Todas as camadas".
    fn opcoes_do_misturador(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let (m, ponta) = self
            .sessao()
            .map(|s| (s.misturador, s.tinta.cor_da_ponta(s.misturador.mistura)))
            .unwrap_or_default();
        let cor = ponta.map(|c| gpui_kit::Rgba {
            r: c[0] as f32 / 255.0,
            g: c[1] as f32 / 255.0,
            b: c[2] as f32 / 255.0,
            a: 1.0,
        });
        let borda = aparencia::cores(cx).borda;
        let amostra = div()
            .size(px(14.))
            .rounded(px(3.))
            .border_1()
            .border_color(borda)
            .when_some(cor, |d, c| d.bg(c));
        let ed = cx.entity();
        let ponta = crate::estilo::botao_contorno_pequeno("editor-misturador-ponta", cx)
            .debug_selector(|| "editor-misturador-ponta".into())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(amostra)
                    .child("▾"),
            )
            .tooltip("A tinta do pincel: carregar, limpar e o que fazer a cada traço")
            .dropdown_menu_with_anchor(gpui_kit::Anchor::TopLeft, move |menu, _window, _cx| {
                let item =
                    |id: &'static str,
                     rotulo: &'static str,
                     fazer: fn(&mut EditorDeFoto, &mut Context<EditorDeFoto>)| {
                        let ed = ed.clone();
                        crate::estilo::item_de_menu(id, rotulo, None)
                            .on_click(move |_ev, _window, cx| ed.update(cx, fazer))
                    };
                menu.item(item(
                    "editor-misturador-carregar",
                    "Carregar o pincel (cor de frente)",
                    |ed, cx| ed.carregar_o_misturador(true, cx),
                ))
                .item(item(
                    "editor-misturador-limpar",
                    "Limpar o pincel",
                    |ed, cx| ed.carregar_o_misturador(false, cx),
                ))
                .separator()
                .item(
                    item(
                        "editor-misturador-carregar-apos",
                        "Carregar a cada traço",
                        |ed, cx| ed.alternar_opcao_do_misturador(0, cx),
                    )
                    .checked(m.carregar_apos),
                )
                .item(
                    item(
                        "editor-misturador-limpar-apos",
                        "Limpar a cada traço",
                        |ed, cx| ed.alternar_opcao_do_misturador(1, cx),
                    )
                    .checked(m.limpar_apos),
                )
            });
        vec![
            separador_da_faixa(cx).into_any_element(),
            ponta.into_any_element(),
            self.valor_com_slider(
                "editor-misturador-umidade",
                "Umidade",
                format!("{:.0}%", m.umidade * 100.0),
                &self.pele.umidade,
                cx,
            ),
            self.valor_com_slider(
                "editor-misturador-carga",
                "Carga",
                format!("{:.0}%", m.carga * 100.0),
                &self.pele.carga,
                cx,
            ),
            self.valor_com_slider(
                "editor-misturador-mistura",
                "Mistura",
                format!("{:.0}%", m.mistura * 100.0),
                &self.pele.mistura,
                cx,
            ),
            self.caixa_de_marcar(
                "editor-misturador-todas",
                "Todas as camadas",
                m.todas_as_camadas,
                |ed, cx| ed.alternar_opcao_do_misturador(2, cx),
                cx,
            ),
        ]
    }

    /// 100% e Encaixar — a Mão e a Lupa.
    pub(super) fn botoes_de_zoom(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        vec![
            crate::estilo::botao_contorno_pequeno("editor-opcoes-100", cx)
                .label("100%")
                .tooltip(na_plataforma("Um pixel da foto num pixel da tela (⌘⌥0)"))
                .on_click(cx.listener(|ed, _, _, cx| ed.ir_para_nivel(Nivel::Razao(1.0), None, cx)))
                .into_any_element(),
            crate::estilo::botao_contorno_pequeno("editor-opcoes-encaixar", cx)
                .label("Encaixar na tela")
                .tooltip(na_plataforma("A foto inteira na janela (⌘0)"))
                .on_click(cx.listener(|ed, _, _, cx| ed.ir_para_nivel(Nivel::Encaixar, None, cx)))
                .into_any_element(),
            crate::estilo::botao_contorno_pequeno("editor-opcoes-preencher", cx)
                .label("Preencher a tela")
                .tooltip("A foto cobre a janela inteira")
                .on_click(cx.listener(|ed, _, _, cx| ed.ir_para_nivel(Nivel::Preencher, None, cx)))
                .into_any_element(),
        ]
    }

    /// As opções da ferramenta na mão.
    fn opcoes_da_ferramenta(&self, item: Item, cx: &mut Context<Self>) -> Vec<AnyElement> {
        match item {
            Item::S(_) | Item::A(Auxiliar::Varinha) => vec![self.barra_de_opcoes_da_selecao(cx)],
            Item::F(f) => self.opcoes_de_pintura(Some(f), cx),
            Item::A(Auxiliar::Correcao) => self.opcoes_de_pintura(None, cx),
            Item::A(Auxiliar::Remendo) => {
                let difusao = self.sessao().map_or(5, |s| s.pincel.difusao);
                vec![
                    self.valor_com_slider(
                        "editor-difusao-do-remendo",
                        "Difusão",
                        format!("{difusao}"),
                        &self.difusao_da_recuperacao,
                        cx,
                    ),
                    ajuda(
                        "editor-ajuda-do-remendo",
                        "Contorne a área com defeito (ou use a seleção que já existe) e arraste-a até a pele limpa. A textura vem de lá; a cor e a luz se adaptam à borda daqui. Lê a foto até a camada escolhida e pinta nela.",
                    ),
                ]
            }
            Item::A(Auxiliar::Mover) => {
                vec![
                    crate::estilo::botao_contorno_pequeno("editor-opcoes-transformar", cx)
                        .label("Transformação livre")
                        .tooltip(na_plataforma("Caixa com alças na camada escolhida (⌘T)"))
                        .disabled(!self.pronta())
                        .on_click(cx.listener(|ed, _, _, cx| ed.transformar(cx)))
                        .into_any_element(),
                ]
            }
            Item::A(Auxiliar::Degrade) => {
                let (frente, fundo, na_mascara) = self
                    .sessao()
                    .map(|s| (s.pincel.cor, s.pincel.cor_de_fundo, s.na_mascara()))
                    .unwrap_or(([0; 3], [255; 3], false));
                let cor = |c: [u8; 3]| -> gpui_kit::Hsla {
                    gpui_kit::rgb((c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32).into()
                };
                let fim = if na_mascara {
                    cor(fundo)
                } else {
                    cor(frente).opacity(0.0)
                };
                vec![
                    rotulo(
                        if na_mascara {
                            "Frente → fundo"
                        } else {
                            "Frente → transparente"
                        },
                        cx,
                    )
                    .into_any_element(),
                    div()
                        .debug_selector(|| "editor-previa-do-degrade".into())
                        .w(px(120.))
                        .h(px(14.))
                        .border_1()
                        .border_color(aparencia::cores(cx).borda)
                        .bg(gpui_kit::linear_gradient(
                            90.,
                            gpui_kit::linear_color_stop(cor(frente), 0.),
                            gpui_kit::linear_color_stop(fim, 1.),
                        ))
                        .into_any_element(),
                ]
            }
            Item::A(Auxiliar::Mao) => self.botoes_de_zoom(cx),
            Item::A(Auxiliar::Zoom) => {
                use gpui_kit::component::button::ButtonGroup;
                let reduz = self.lupa_reduz;
                let grupo = ButtonGroup::new("editor-modo-da-lupa")
                    .xsmall()
                    .child(
                        if reduz {
                            crate::estilo::botao_contorno_pequeno("editor-lupa-ampliar", cx)
                        } else {
                            crate::estilo::botao_primario_pequeno("editor-lupa-ampliar", cx)
                        }
                        .child(gpui_kit::component::Icon::new(Icone::ZoomIn).size(px(14.)))
                        .tooltip("Ampliar")
                        .selected(!reduz),
                    )
                    .child(
                        if reduz {
                            crate::estilo::botao_primario_pequeno("editor-lupa-reduzir", cx)
                        } else {
                            crate::estilo::botao_contorno_pequeno("editor-lupa-reduzir", cx)
                        }
                        .child(gpui_kit::component::Icon::new(Icone::ZoomOut).size(px(14.)))
                        .tooltip(na_plataforma("Reduzir (⌥ inverte no clique)"))
                        .selected(reduz),
                    )
                    .on_click(cx.listener(|ed, cliques: &Vec<usize>, _, cx| {
                        if let Some(i) = cliques.first() {
                            ed.lupa_reduz = *i == 1;
                            cx.notify();
                        }
                    }));
                let mut v = vec![
                    grupo.into_any_element(),
                    separador_da_faixa(cx).into_any_element(),
                ];
                v.extend(self.botoes_de_zoom(cx));
                v
            }
            Item::A(Auxiliar::GirarVista) => {
                use gpui_kit::component::input::NumberInput;
                vec![
                    rotulo("Ângulo de rotação:", cx).into_any_element(),
                    div()
                        .flex_shrink_0()
                        .w(px(medida::CAMPO_NUMERICO + 16.))
                        .debug_selector(|| "editor-angulo-da-vista".into())
                        .child(crate::estilo::campo_pequeno(
                            NumberInput::new(&self.campo_do_giro)
                                .xsmall()
                                .suffix(div().text_xs().child("°")),
                        ))
                        .into_any_element(),
                    crate::estilo::botao_contorno_pequeno("editor-redefinir-vista", cx)
                        .label("Redefinir vista")
                        .tooltip("Volta a 0° (Esc com a Girar vista na mão)")
                        .disabled(self.giro == 0.0)
                        .on_click(cx.listener(|ed, _, _, cx| ed.girar_a_vista(0.0, cx)))
                        .into_any_element(),
                ]
            }
            Item::A(Auxiliar::ContaGotas | Auxiliar::Lata) => Vec::new(),
            Item::P(f) => self.opcoes_da_caneta(f, cx),
        }
    }

    /// A faixa inteira: a ferramenta, as opções e, durante uma transformação,
    /// Cancelar e Aplicar.
    pub(super) fn barra_de_opcoes(&self, cx: &mut Context<Self>) -> AnyElement {
        let c = aparencia::cores(cx);
        let transformando = self.transformando();
        let liquidificando = self.liquidificando();
        let (icone, titulo, conteudo): (Icone, String, Vec<AnyElement>) = if liquidificando {
            (
                Icone::Droplet,
                "Liquidificar".into(),
                vec![self.barra_do_liquidificar(cx)],
            )
        } else if transformando {
            let so_o_contorno = self.sessao().is_some_and(Sessao::transformando_a_selecao);
            (
                Icone::Move,
                if self.sessao().is_some_and(Sessao::transformando_o_caminho) {
                    "Transformar caminho"
                } else if so_o_contorno {
                    "Transformar seleção (só o contorno)"
                } else if self.deformando() {
                    "Deformar"
                } else {
                    "Transformação livre"
                }
                .into(),
                vec![self.barra_da_transformacao(cx)],
            )
        } else {
            match self.item_atual().and_then(|i| def_de(&i).map(|d| (i, d))) {
                Some((item, d)) => (
                    d.icone,
                    nome_com_letra(d),
                    self.opcoes_da_ferramenta(item, cx),
                ),
                None => (Icone::Paintbrush, String::new(), Vec::new()),
            }
        };
        let confirmar = transformando || liquidificando;
        div()
            .id("editor-barra-de-opcoes")
            .debug_selector(|| "editor-barra-de-opcoes".into())
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(medida::VAO))
            .h(px(medida::ALTURA_DAS_OPCOES))
            .pl(px(8.))
            .pr(px(6.))
            .bg(c.cromo)
            .border_b_1()
            .border_color(c.borda)
            .child(
                div()
                    .id("editor-ferramenta-nas-opcoes")
                    .debug_selector(|| "editor-ferramenta-nas-opcoes".into())
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(26.))
                    .rounded(px(medida::CANTO))
                    .border_1()
                    .border_color(c.borda)
                    .child(gpui_kit::component::Icon::new(icone).size(px(16.)))
                    .tooltip({
                        let titulo = titulo.clone();
                        move |window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(titulo.clone())
                                .build(window, cx)
                        }
                    }),
            )
            .when(confirmar, |d| {
                d.child(
                    div()
                        .flex_shrink_0()
                        .debug_selector(|| "editor-dica-da-transformacao".into())
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .child(titulo.clone()),
                )
            })
            .child(separador_da_faixa(cx))
            .child(
                div()
                    .id("editor-opcoes-rolagem")
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .overflow_x_scroll()
                    .flex()
                    .items_center()
                    .gap(px(medida::VAO))
                    .children(conteudo),
            )
            .when(confirmar, |d| {
                d.child(separador_da_faixa(cx))
                    .child(
                        crate::estilo::botao_contorno_pequeno("editor-cancelar-transformacao", cx)
                            .child(gpui_kit::component::Icon::new(Icone::X).size(px(14.)))
                            .tooltip("Cancelar (Esc)")
                            .on_click(cx.listener(|ed, _, _, cx| ed.cancelar_transformacao(cx))),
                    )
                    .child(
                        crate::estilo::botao_primario_pequeno("editor-aplicar-transformacao", cx)
                            .child(gpui_kit::component::Icon::new(Icone::Check).size(px(14.)))
                            .tooltip(na_plataforma("Aplicar (⏎)"))
                            .on_click(cx.listener(|ed, _, _, cx| ed.aplicar_transformacao(cx))),
                    )
            })
            .into_any_element()
    }

    /// O ponto de referência da transformação: os 9 pontos da caixa (o
    /// "Reference point location" do Photoshop). O centro é o padrão.
    fn referencia_de_nove_pontos(&self, cx: &mut Context<Self>) -> AnyElement {
        let c = aparencia::cores(cx);
        let atual = self.fracao_da_referencia();
        let mut grade = div()
            .id("editor-referencia-de-nove-pontos")
            .debug_selector(|| "editor-referencia-de-nove".into())
            .flex_shrink_0()
            .flex()
            .flex_wrap()
            .w(px(27.))
            .gap(px(1.));
        for (j, fy) in [0.0f32, 0.5, 1.0].into_iter().enumerate() {
            for (i, fx) in [0.0f32, 0.5, 1.0].into_iter().enumerate() {
                let escolhido =
                    atual.is_some_and(|(ax, ay)| (ax - fx).abs() < 0.01 && (ay - fy).abs() < 0.01);
                grade = grade.child(
                    div()
                        .id(("editor-referencia", j * 3 + i))
                        .debug_selector(move || format!("editor-referencia-{}", j * 3 + i))
                        .size(px(8.))
                        .border_1()
                        .border_color(c.apagado)
                        .when(escolhido, |d| d.bg(c.texto))
                        .cursor_pointer()
                        .on_click(
                            cx.listener(move |ed, _, _, cx| ed.escolher_referencia(fx, fy, cx)),
                        ),
                );
            }
        }
        grade
            .tooltip(|window, cx| {
                gpui_kit::component::tooltip::Tooltip::new("Ponto de referência").build(window, cx)
            })
            .into_any_element()
    }

    /// A referência como fração da caixa (para a grade de 9 pontos).
    fn fracao_da_referencia(&self) -> Option<(f32, f32)> {
        let (caixa, _) = self.sessao()?.transformacao()?;
        let (x, y) = self.referencia_da_caixa()?;
        Some((
            (x - caixa.x as f32) / (caixa.largura as f32).max(1.0),
            (y - caixa.y as f32) / (caixa.altura as f32).max(1.0),
        ))
    }

    /// Um dos 9 pontos vira a referência.
    pub fn escolher_referencia(&mut self, fx: f32, fy: f32, cx: &mut Context<Self>) {
        let Some((caixa, _)) = self.sessao().and_then(Sessao::transformacao) else {
            return;
        };
        self.referencia_da_caixa = Some((
            caixa.x as f32 + caixa.largura as f32 * fx,
            caixa.y as f32 + caixa.altura as f32 * fy,
        ));
        self.numeros_mostrados = None;
        cx.notify();
    }
}

// ------------------------------------------- as barras que vieram de janela.rs

impl EditorDeFoto {
    /// O botão "Modificar seleção ▾": Difundir…, Expandir…, Contrair… (cada um
    /// pede o valor em pixels) e, separado, "Transformar seleção" — só o
    /// contorno, ao contrário do ⌘T, que leva os pixels.
    fn menu_modificar_selecao(
        &self,
        id: &'static str,
        rotulo: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tem_selecao = self.sessao().and_then(Sessao::selecao).is_some();
        let ed = cx.entity();
        crate::estilo::botao_contorno_pequeno(id, cx)
            .debug_selector(move || id.into())
            .label(rotulo)
            .tooltip("Difundir, expandir, contrair (com o valor em pixels) e Transformar seleção")
            .disabled(!tem_selecao || self.transformando())
            .dropdown_menu_with_anchor(gpui_kit::Anchor::TopLeft, move |menu, _window, _cx| {
                let item = |item_id: &'static str, rotulo: &'static str, m: Modificacao| {
                    let ed = ed.clone();
                    crate::estilo::item_de_menu(item_id, rotulo, None).on_click(
                        move |_ev, window, cx| {
                            ed.update(cx, |ed, cx| ed.abrir_modificacao(m, window, cx));
                        },
                    )
                };
                let transformar = {
                    let ed = ed.clone();
                    crate::estilo::item_de_menu(
                        "editor-transformar-selecao",
                        "Transformar seleção",
                        None,
                    )
                    .on_click(move |_ev, _window, cx| {
                        ed.update(cx, |ed, cx| ed.transformar_selecao(cx));
                    })
                };
                menu.item(item(
                    "editor-modificar-difundir",
                    "Difundir…  ⇧F6",
                    Modificacao::Difundir,
                ))
                .item(item(
                    "editor-modificar-expandir",
                    "Expandir…",
                    Modificacao::Expandir,
                ))
                .item(item(
                    "editor-modificar-contrair",
                    "Contrair…",
                    Modificacao::Contrair,
                ))
                .separator()
                .item(transformar)
            })
            .into_any_element()
    }

    /// A barra de opções das ferramentas de seleção, embaixo da barra de cima
    /// (a do Photoshop): os quatro modos, e as opções de cada ferramenta —
    /// difusão e estilo na retangular e na elíptica, antisserrilhado na
    /// elíptica e nos laços, tolerância/contígua/amostra na varinha. Mudar uma
    /// opção não mexe na seleção que existe nem entra no desfazer.
    fn barra_de_opcoes_da_selecao(&self, cx: &mut Context<Self>) -> AnyElement {
        use gpui_kit::component::button::ButtonGroup;
        use gpui_kit::component::checkbox::Checkbox;
        use gpui_kit::component::input::NumberInput;
        let tema = cx.theme().clone();
        let tipo = self.selecionando;
        let rotulo = |texto: &'static str| {
            div()
                .text_xs()
                .text_color(tema.muted_foreground)
                .child(texto)
        };
        let separador = || div().w(px(1.)).h(px(18.)).bg(tema.border);
        let numero =
            |campo: &Entity<InputState>, seletor: &'static str, sufixo: Option<&'static str>| {
                div()
                    .w(px(84.))
                    .debug_selector(move || seletor.into())
                    .child(crate::estilo::campo_pequeno(
                        NumberInput::new(campo)
                            .xsmall()
                            .when_some(sufixo, |n, s| n.suffix(div().text_xs().child(s))),
                    ))
            };
        let realcado = self.modo_realcado();
        // O ativo no botão primário (a cor de destaque do tema): o realce do
        // contorno do kit é um cinza quase igual ao do fundo.
        let modos = ButtonGroup::new("editor-modos-de-selecao")
            .xsmall()
            .children(MODOS.iter().enumerate().map(|(i, modo)| {
                let id: &'static str = [
                    "editor-modo-nova",
                    "editor-modo-adicionar",
                    "editor-modo-subtrair",
                    "editor-modo-intersectar",
                ][i];
                let dica = [
                    "Nova seleção",
                    "Adicionar à seleção (⇧ no gesto)",
                    "Subtrair da seleção (⌥ no gesto)",
                    "Intersectar com a seleção (⇧⌥ no gesto)",
                ][i];
                if realcado == *modo {
                    crate::estilo::botao_primario_pequeno(id, cx)
                } else {
                    crate::estilo::botao_contorno_pequeno(id, cx)
                }
                .debug_selector(move || id.into())
                .label(nome_do_modo(*modo))
                .tooltip(na_plataforma(dica))
                .selected(realcado == *modo)
            }))
            .on_click(cx.listener(|ed, cliques: &Vec<usize>, _, cx| {
                if let Some(modo) = cliques.first().and_then(|i| MODOS.get(*i)) {
                    ed.escolher_modo_de_selecao(*modo, cx);
                }
            }));
        let mut barra = div()
            .id("editor-opcoes-da-selecao")
            .debug_selector(|| "editor-opcoes-da-selecao".into())
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(medida::VAO))
            .child(modos);

        if let Some(tipo) = tipo {
            let o = self.opcoes_da_forma(tipo);
            barra = barra
                .child(separador())
                .child(rotulo("Difusão"))
                .child(numero(&self.campo_da_difusao, "editor-difusao", Some("px")));
            if tipo.suaviza() {
                barra = barra.child(
                    div().debug_selector(|| "editor-suavizar".into()).child(
                        Checkbox::new("editor-suavizar")
                            .xsmall()
                            .label("Antisserrilhado")
                            .checked(o.acabamento.suavizar)
                            .on_click(cx.listener(|ed, _: &bool, _, cx| ed.alternar_suavizar(cx))),
                    ),
                );
            }
            if tipo.tem_estilo() {
                barra = barra.child(separador()).child(rotulo("Estilo")).child(
                    div()
                        .w(px(140.))
                        .debug_selector(|| "editor-estilo".into())
                        .child(crate::estilo::campo_pequeno(
                            Select::new(&self.seletor_de_estilo).xsmall(),
                        )),
                );
                if o.estilo != TipoDeEstilo::Normal {
                    let unidade = (o.estilo == TipoDeEstilo::Tamanho).then_some("px");
                    barra = barra
                        .child(rotulo("Largura"))
                        .child(numero(
                            &self.campo_da_largura,
                            "editor-estilo-largura",
                            unidade,
                        ))
                        .child(
                            crate::estilo::botao_fantasma_pequeno("editor-trocar-medidas", cx)
                                .debug_selector(|| "editor-trocar-medidas".into())
                                .label("⇄")
                                .tooltip("Trocar largura e altura")
                                .on_click(
                                    cx.listener(|ed, _, _, cx| ed.trocar_largura_e_altura(cx)),
                                ),
                        )
                        .child(rotulo("Altura"))
                        .child(numero(
                            &self.campo_da_altura,
                            "editor-estilo-altura",
                            unidade,
                        ));
                }
                if o.estilo == TipoDeEstilo::Proporcao {
                    let ed = cx.entity();
                    barra = barra.child(
                        crate::estilo::botao_fantasma_pequeno("editor-proporcoes", cx)
                            .debug_selector(|| "editor-proporcoes".into())
                            .label("Predefinições ▾")
                            .dropdown_menu_with_anchor(
                                gpui_kit::Anchor::TopLeft,
                                move |mut menu, _window, _cx| {
                                    for (l, a) in Estilo::PROPORCOES {
                                        let ed = ed.clone();
                                        menu =
                                            menu.item(
                                                crate::estilo::item_de_menu(
                                                    match (l, a) {
                                                        (1, 1) => "editor-proporcao-1-1",
                                                        (3, 2) => "editor-proporcao-3-2",
                                                        (4, 3) => "editor-proporcao-4-3",
                                                        _ => "editor-proporcao-16-9",
                                                    },
                                                    format!("{l}:{a}"),
                                                    None,
                                                )
                                                .on_click(move |_ev, _w, cx| {
                                                    ed.update(cx, |ed, cx| {
                                                        ed.usar_proporcao(l as f32, a as f32, cx);
                                                        ed.campos_de = None;
                                                    });
                                                }),
                                            );
                                    }
                                    menu
                                },
                            ),
                    );
                }
            }
        } else {
            let v = self.opcoes_da_varinha;
            barra = barra
                .child(separador())
                .child(rotulo("Tolerância"))
                .child(numero(&self.campo_da_tolerancia, "editor-tolerancia", None))
                .child(
                    div().debug_selector(|| "editor-suavizar".into()).child(
                        Checkbox::new("editor-suavizar")
                            .xsmall()
                            .label("Antisserrilhado")
                            .checked(v.suavizar)
                            .on_click(cx.listener(|ed, _: &bool, _, cx| ed.alternar_suavizar(cx))),
                    ),
                )
                .child(
                    div().debug_selector(|| "editor-contigua".into()).child(
                        Checkbox::new("editor-contigua")
                            .xsmall()
                            .label("Contígua")
                            .checked(v.contigua)
                            .on_click(
                                cx.listener(|ed, _: &bool, _, cx| ed.alternar_varinha_contigua(cx)),
                            ),
                    ),
                )
                .child(rotulo("Amostra"))
                .child(
                    div()
                        .w(px(150.))
                        .debug_selector(|| "editor-amostra".into())
                        .child(crate::estilo::campo_pequeno(
                            Select::new(&self.seletor_de_amostra).xsmall(),
                        )),
                );
        }
        barra
            .child(separador())
            .child(
                crate::estilo::botao_contorno_pequeno("editor-opcoes-preenchimento", cx)
                    .label("Preenchimento sensível ao conteúdo…")
                    .tooltip(na_plataforma(
                        "Remover o selecionado com prévia: PatchMatch ou IA local (Editar › Preenchimento sensível ao conteúdo) · ⇧⌫ preenche direto",
                    ))
                    .on_click(cx.listener(|ed, _, _, cx| ed.abrir_preenchimento(cx))),
            )
            .child(self.menu_modificar_selecao(
                "editor-modificar-selecao",
                "Modificar seleção ▾",
                cx,
            ))
            .into_any_element()
    }

    /// A barra de opções da transformação (⌘T e Transformar seleção): X e Y do
    /// ponto de referência, largura e altura em %, o ângulo, e a dica dos
    /// gestos — a barra do Photoshop durante a transformação livre.
    /// A barra do Liquidificar: a ferramenta, o tamanho (`[` `]`), a pressão
    /// e "Restaurar tudo".
    fn barra_do_liquidificar(&self, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme().clone();
        let (raio, forca) = self
            .sessao()
            .map(|s| (s.pincel.raio, s.forca_do_liquido))
            .unwrap_or((40.0, 0.5));
        div()
            .id("editor-opcoes-do-liquidificar")
            .debug_selector(|| "editor-opcoes-do-liquidificar".into())
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(medida::VAO))
            .child(div().text_xs().child("Deformação para a frente"))
            .child(
                div()
                    .text_xs()
                    .text_color(tema.muted_foreground)
                    .child(format!("Tamanho {:.0} px  [ ]", raio * 2.0)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(tema.muted_foreground)
                    .child(format!("Pressão {:.0}%", forca * 100.0)),
            )
            .child(
                div()
                    .w(px(140.))
                    .h(px(20.))
                    .debug_selector(|| "editor-pressao-do-liquido".into())
                    .child(crate::estilo::slider(&self.pressao_do_liquido)),
            )
            .child(
                crate::estilo::botao_contorno_pequeno("editor-restaurar-liquido", cx)
                    .debug_selector(|| "editor-restaurar-liquido".into())
                    .label("Restaurar tudo")
                    .tooltip("A camada volta a como estava, e o Liquidificar continua aberto")
                    .on_click(cx.listener(|ed, _, _, cx| ed.restaurar_liquidificacao(cx))),
            )
            .child(ajuda(
                "editor-ajuda-do-liquidificar",
                "Arraste para empurrar os pixels · [ ] tamanho · { } dureza · com seleção, o de fora fica parado · Enter aplica · Esc cancela",
            ))
            .into_any_element()
    }

    fn barra_da_transformacao(&self, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme().clone();
        let separador = || div().w(px(1.)).h(px(18.)).bg(tema.border);
        if self.deformando() {
            let intocada = self.sessao().is_some_and(Sessao::malha_intocada);
            return div()
                .id("editor-opcoes-do-deformar")
                .debug_selector(|| "editor-opcoes-do-deformar".into())
                .flex()
                .flex_shrink_0()
                .items_center()
                .gap(px(medida::VAO))
                .child(
                    div()
                        .text_xs()
                        .child("Deformar — 3 × 3 células"),
                )
                .child(
                    div().debug_selector(|| "editor-grade-do-deformar".into()).child(
                        gpui_kit::component::checkbox::Checkbox::new("editor-grade-do-deformar")
                            .xsmall()
                            .label("Mostrar a grade")
                            .checked(self.grade_visivel)
                            .on_click(cx.listener(|ed, _: &bool, _, cx| ed.alternar_grade(cx))),
                    ),
                )
                .child(
                    crate::estilo::botao_contorno_pequeno("editor-redefinir-malha", cx)
                        .debug_selector(|| "editor-redefinir-malha".into())
                        .label("Redefinir")
                        .tooltip("Volta à malha do começo, sem aplicar")
                        .disabled(intocada)
                        .on_click(cx.listener(|ed, _, _, cx| ed.redefinir_malha(cx))),
                )
                .child(
                    crate::estilo::botao_fantasma_pequeno("editor-voltar-a-transformacao", cx)
                        .debug_selector(|| "editor-voltar-a-transformacao".into())
                        .label("Transformação livre")
                        .tooltip(if intocada {
                            "Volta à caixa da transformação livre"
                        } else {
                            "Só com a malha intocada (Redefinir antes): uma malha deformada não cabe numa caixa"
                        })
                        .disabled(!intocada)
                        .on_click(cx.listener(|ed, _, _, cx| ed.voltar_a_transformacao_livre(cx))),
                )
                .child(ajuda(
                    "editor-ajuda-do-deformar",
                    "Arraste um ponto (o canto leva as alças junto) ou puxe por dentro da malha · Enter aplica · Esc cancela",
                ))
                .into_any_element();
        }
        let so_o_contorno = self.sessao().is_some_and(Sessao::transformando_a_selecao);
        div()
            .id("editor-opcoes-da-transformacao")
            .debug_selector(|| "editor-opcoes-da-transformacao".into())
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(medida::VAO))
            .child(self.referencia_de_nove_pontos(cx))
            .child(separador())
            .children({
                        use gpui_kit::component::input::NumberInput;
                        let rotulos = ["X", "Y", "L %", "A %", "Ângulo"];
                        let seletores = [
                            "editor-transformacao-x",
                            "editor-transformacao-y",
                            "editor-transformacao-l",
                            "editor-transformacao-a",
                            "editor-transformacao-angulo",
                        ];
                        let travada = self.proporcao_travada;
                        let corrente = crate::estilo::botao_icone(
                            "editor-proporcao-travada",
                            if travada { Icone::Link2 } else { Icone::Link2Off },
                            22.,
                            12.,
                        )
                        .tooltip(if travada {
                            "Largura e altura juntas — clique para soltar"
                        } else {
                            "Largura e altura soltas — clique para manter a proporção"
                        })
                        .on_click(cx.listener(|ed, _, _, cx| {
                            ed.proporcao_travada = !ed.proporcao_travada;
                            cx.notify();
                        }));
                        let mut corrente = Some(corrente.into_any_element());
                        self.campos_da_transformacao
                            .iter()
                            .zip(rotulos.into_iter().zip(seletores))
                            .enumerate()
                            .flat_map(|(i, (campo, (rotulo, seletor)))| {
                                let entre = if i == 3 { corrente.take() } else { None };
                                let campo = div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(tema.muted_foreground)
                                            .child(rotulo),
                                    )
                                    .child(
                                        div()
                                            .w(px(84.))
                                            .debug_selector(move || seletor.into())
                                            .child(crate::estilo::campo_pequeno(
                                                NumberInput::new(campo).xsmall(),
                                            )),
                                    )
                                    .into_any_element();
                                entre.into_iter().chain(std::iter::once(campo))
                            })
                            .collect::<Vec<_>>()
                    })
            .when(!so_o_contorno, |barra| {
                barra.child(
                    crate::estilo::botao_contorno_pequeno("editor-alternar-deformar", cx)
                        .debug_selector(|| "editor-alternar-deformar".into())
                        .label("Deformar")
                        .tooltip("Trocar a caixa pela malha de 3 × 3 células (Warp)")
                        .on_click(cx.listener(|ed, _, _, cx| ed.deformar(cx))),
                )
            })
            .child(ajuda(
                "editor-ajuda-da-transformacao",
                "Alças: tamanho (⇧ livre, ⌥ em volta da referência) · fora: girar (⇧ 15°) · arraste o alvo para mudar a referência · Enter aplica · Esc cancela",
            ))
            .into_any_element()
    }
}
