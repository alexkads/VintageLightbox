//! 📚 Os painéis Camadas e Histórico do editor (vieram de `janela.rs`).
//!
//! Camadas: a mesclagem e a opacidade em controles compactos (a opacidade é
//! um campo numérico com o slider num popover), os cadeados, a pilha de cima
//! para baixo — que é o que rola — e o rodapé fixo com as ações. A camada
//! escolhida tem o fundo de destaque; o alvo (pixels ou máscara) a moldura na
//! miniatura dele.

use gpui_kit::component::input::Input;
use gpui_kit::component::menu::ContextMenuExt as _;
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::popover::Popover;
use gpui_kit::component::select::Select;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _};
use gpui_kit::{
    div, img, prelude::*, px, AnyElement, Context, MouseButton, MouseDownEvent, ObjectFit,
    SharedString,
};

use super::aparencia::{self, medida};
use super::ferramentas::na_plataforma;
use super::{mascara_e_cadeados, moldura, operacao_dos, EditorDeFoto, LADO_DA_MINIATURA, MOSTRAR};
use crate::recursos::Icone;
use editor_core::Modo;

impl EditorDeFoto {
    /// O "Fundo" do Photoshop, embaixo de todas: a fotografia base, que nunca
    /// muda (C28) — por isso com o cadeado e sem olho. Clique explica onde
    /// pintar; duplo clique (ou o cadeado) cria a camada da fotografia, como o
    /// "Fundo → Camada 0" de lá; ⌘/Ctrl + clique seleciona tudo.
    fn linha_do_fundo(&self, lado: gpui_kit::Pixels, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme().clone();
        let criando = self.criando_a_fotografia;
        div()
            .id("editor-camada-fundo")
            .debug_selector(|| "editor-camada-fundo".into())
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(4.))
            .py(px(2.))
            .mt(px(4.))
            .rounded(crate::tema::canto(4.))
            .cursor_pointer()
            .hover(|d| d.bg(tema.muted))
            // O lugar do olho (o mesmo botão do kit): o Fundo aparece sempre.
            .child(
                div()
                    .flex_shrink_0()
                    .w(px(crate::tema::medidas().botao_icone)),
            )
            .when_some(self.miniatura_do_fundo.clone(), |d, m| {
                d.child(
                    moldura(div(), false, tema.ring)
                        .child(img(m).object_fit(ObjectFit::Contain).w(lado).h(lado)),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .text_sm()
                    .italic()
                    .child("Fundo"),
            )
            .child(
                crate::estilo::botao_icone_pequeno("editor-fundo-cadeado", Icone::Lock)
                    .debug_selector(|| "editor-fundo-cadeado".into())
                    .tooltip("A fotografia base fica intacta. Clique para criar a camada da fotografia e retocá-la")
                    .disabled(criando)
                    .on_click(cx.listener(|ed, _, _, cx| ed.criar_camada_da_fotografia(cx))),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |ed, evento: &MouseDownEvent, window, cx| {
                    if evento.modifiers.secondary() {
                        ed.selecionar_tudo(cx);
                    } else if evento.click_count >= 2 {
                        ed.criar_camada_da_fotografia(cx);
                    } else {
                        ed.aviso = Some((
                            na_plataforma(
                                "O Fundo é a fotografia base e fica intacto — pinte numa camada (⇧⌘N) ou dê duplo clique para criar a camada da fotografia",
                            )
                            .into(),
                            false,
                        ));
                        cx.notify();
                    }
                    window.focus(&ed.foco, cx);
                }),
            )
            .into_any_element()
    }

    /// O painel Camadas do Photoshop: o modo e a opacidade da escolhida, a
    /// pilha de cima para baixo, e os botões embaixo.
    pub(super) fn painel_de_camadas(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme().clone();
        let cadeados = self.barra_de_cadeados(cx);
        let (camadas, ativa, pode_desfazer_alguma) = match self.sessao() {
            Some(s) => (
                s.documento()
                    .camadas
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        (
                            c.nome.clone(),
                            c.visivel,
                            c.modo,
                            c.mascara.as_ref().map(|m| m.ativa),
                            c.ajuste.is_some(),
                            s.documento().base_do_recorte(i).is_some(),
                            c.bloqueio.algum(),
                            c.mascara.as_ref().is_some_and(|m| m.vinculada),
                            c.mascara_vetorial.as_ref().map(|m| m.ativa),
                        )
                    })
                    .collect::<Vec<_>>(),
                s.ativa(),
                true,
            ),
            None => (Vec::new(), 0, false),
        };
        let na_mascara = self.na_mascara();
        let tem_mascara = camadas.get(ativa).is_some_and(|c| c.3.is_some());
        let quantas = camadas.len();
        let (ativa_recortada, ativa_pode_recortar, ativa_e_base) = self
            .sessao()
            .map(|s| {
                let doc = s.documento();
                (
                    doc.camadas.get(ativa).is_some_and(|c| c.recortada),
                    s.pode_recortar(ativa),
                    doc.camadas.get(ativa).is_some_and(|c| !c.recortada)
                        && doc.fim_do_conjunto(ativa) > ativa,
                )
            })
            .unwrap_or_default();
        let criando_a_fotografia = self.criando_a_fotografia;
        let alvo_vetorial = self.sessao().and_then(editor_core::Sessao::alvo_vetorial);
        let renomeando = self.renomeando.clone();
        let miniaturas = self.miniaturas.clone();
        let das_mascaras = self.miniaturas_das_mascaras.clone();
        let lado = px(LADO_DA_MINIATURA as f32 * 0.75);
        // A moldura do alvo na cor de destaque do tema: em volta de uma
        // máscara branca, a do texto sumia.
        let cor_da_moldura = tema.ring;
        let cor_do_icone_de_ajuste = tema.muted;
        let linhas = camadas
            .into_iter()
            .enumerate()
            .rev()
            .map(|(i, (nome, visivel, modo, mascara, de_ajuste, recortada, bloqueada, vinculada, vetorial))| {
                let escolhida = i == ativa;
                let nome_do_arrasto: SharedString = nome.clone().into();
                let id_do_olho: SharedString = format!("editor-olho-{i}").into();
                let olho = crate::estilo::botao_icone_pequeno(
                    id_do_olho.clone(),
                    if visivel { Icone::Eye } else { Icone::EyeOff },
                )
                .debug_selector(move || id_do_olho.to_string())
                .tooltip(if visivel {
                    format!("Esconder a camada (escolhida: {MOSTRAR})")
                } else {
                    format!("Mostrar a camada (escolhida: {MOSTRAR})")
                })
                .on_click(cx.listener(move |ed, _, _, cx| ed.alternar_visibilidade_de(i, cx)));
                let texto: AnyElement = match &renomeando {
                    Some((j, campo)) if *j == i => div()
                        .flex_1()
                        .child(crate::estilo::campo_pequeno(Input::new(campo).xsmall()))
                        .into_any_element(),
                    _ => div()
                        .flex_1()
                        .min_w(px(0.))
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_sm()
                        .when(!visivel, |d| d.text_color(tema.muted_foreground))
                        .child(nome)
                        .into_any_element(),
                };
                let linha = div()
                    .id(("editor-camada", i))
                    .debug_selector(move || format!("editor-camada-{i}"))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(4.))
                    .py(px(2.))
                    .rounded(crate::tema::canto(4.))
                    .cursor_pointer()
                    .when(escolhida, |d| {
                        d.bg(tema.accent).text_color(tema.accent_foreground)
                    })
                    .when(!escolhida, |d| d.hover(|d| d.bg(tema.muted)))
                    // O olho não escolhe a camada, como no Photoshop.
                    .child(
                        div()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(olho),
                    )
                    // Recortada: recuada, com a seta que desce para a base.
                    .when(recortada, |d| {
                        d.child(
                            div()
                                .id(("editor-recorte", i))
                                .debug_selector(move || format!("editor-recorte-{i}"))
                                .w(px(14.))
                                .text_sm()
                                .text_color(cor_da_moldura)
                                .child("↳")
                                .tooltip(|window, cx| {
                                    gpui_kit::component::tooltip::Tooltip::new(
                                        "Máscara de corte: aparece só onde a camada de baixo tem pixels (⌥ + clique na divisa libera)",
                                    )
                                    .build(window, cx)
                                }),
                        )
                    })
                    // A camada de ajuste não tem pixels: o ícone dela, como no
                    // Photoshop.
                    .when(de_ajuste, |d| {
                        d.child(
                            moldura(
                                div()
                                    .debug_selector(move || format!("editor-miniatura-{i}"))
                                    .size(lado)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(cor_do_icone_de_ajuste)
                                    .child(
                                        gpui_kit::component::Icon::new(Icone::Contrast)
                                            .size_4()
                                            .text_color(cor_da_moldura),
                                    ),
                                false,
                                cor_da_moldura,
                            ),
                        )
                    })
                    .when_some(
                        miniaturas.get(i).cloned().filter(|_| !de_ajuste),
                        |d, m| {
                            d.child(
                                moldura(
                                    div().debug_selector(move || format!("editor-miniatura-{i}")),
                                    escolhida && mascara.is_some() && !na_mascara,
                                    cor_da_moldura,
                                )
                                .child(img(m).object_fit(ObjectFit::Contain).w(lado).h(lado)),
                            )
                        },
                    )
                    // A corrente entre as duas miniaturas: clique solta ou
                    // vincula a máscara à camada.
                    .when(mascara.is_some() && !de_ajuste, |d| {
                        d.child(
                            div()
                                .id(("editor-corrente", i))
                                .debug_selector(move || format!("editor-corrente-{i}"))
                                .flex()
                                .items_center()
                                .justify_center()
                                .w(px(12.))
                                .child(
                                    gpui_kit::component::Icon::new(if vinculada {
                                        Icone::Link2
                                    } else {
                                        Icone::Link2Off
                                    })
                                    .size_3()
                                    .text_color(if vinculada {
                                        cor_da_moldura
                                    } else {
                                        tema.muted_foreground
                                    }),
                                )
                                .tooltip(move |window, cx| {
                                    gpui_kit::component::tooltip::Tooltip::new(if vinculada {
                                        "Vinculada: a máscara anda com a camada — clique para soltar"
                                    } else {
                                        "Solta: a máscara fica no lugar — clique para vincular"
                                    })
                                    .build(window, cx)
                                })
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |ed, _e: &MouseDownEvent, window, cx| {
                                        cx.stop_propagation();
                                        ed.alternar_vinculo_de(i, cx);
                                        window.focus(&ed.foco, cx);
                                    }),
                                ),
                        )
                    })
                    // A máscara: clique escolhe, ⇧ + clique liga e desliga,
                    // ⌥ + clique mostra só ela no palco.
                    .when_some(
                        mascara.zip(das_mascaras.get(i).cloned().flatten()),
                        |d, (ligada, m)| {
                            d.child(
                                moldura(
                                    div()
                                        .id(("editor-mascara", i))
                                        .debug_selector(move || format!("editor-mascara-{i}"))
                                        .relative()
                                        .tooltip(move |window, cx| {
                                            gpui_kit::component::tooltip::Tooltip::new(if ligada {
                                                "Máscara — clique para pintar nela; ⇧ + clique desliga; ⌥ + clique mostra só ela"
                                            } else {
                                                "Máscara desligada — ⇧ + clique liga"
                                            })
                                            .build(window, cx)
                                        }),
                                    escolhida && na_mascara,
                                    cor_da_moldura,
                                )
                                .child(img(m).object_fit(ObjectFit::Contain).w(lado).h(lado))
                                .when(!ligada, |d| {
                                    d.child(
                                        div()
                                            .absolute()
                                            .inset_0()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_color(gpui_kit::red())
                                            .text_lg()
                                            .child("✕"),
                                    )
                                })
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |ed, e: &MouseDownEvent, window, cx| {
                                        cx.stop_propagation();
                                        if e.modifiers.secondary() {
                                            ed.selecionar_da_camada(
                                                i,
                                                true,
                                                operacao_dos(e.modifiers),
                                                cx,
                                            );
                                        } else if e.modifiers.shift {
                                            ed.alternar_mascara_de(i, cx);
                                        } else if e.modifiers.alt {
                                            ed.alternar_so_a_mascara(i, cx);
                                        } else {
                                            ed.escolher_mascara(i, cx);
                                        }
                                        window.focus(&ed.foco, cx);
                                    }),
                                ),
                            )
                        },
                    )
                    // A máscara vetorial (a Caneta): clique escolhe o caminho
                    // dela para editar; ⇧ + clique liga e desliga.
                    .when_some(vetorial, |d, ligada| {
                        let escolhida = alvo_vetorial
                            == Some(editor_core::vetor::LugarDoCaminho::Mascara(i));
                        d.child(
                            moldura(
                                div()
                                    .id(("editor-mascara-vetorial", i))
                                    .debug_selector(move || format!("editor-mascara-vetorial-{i}"))
                                    .relative()
                                    .size(lado)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(gpui_kit::white())
                                    .child(
                                        gpui_kit::component::Icon::new(Icone::PenTool)
                                            .size_4()
                                            .text_color(gpui_kit::black()),
                                    )
                                    .when(!ligada, |d| {
                                        d.child(
                                            div()
                                                .absolute()
                                                .inset_0()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_color(gpui_kit::red())
                                                .text_lg()
                                                .child("✕"),
                                        )
                                    })
                                    .tooltip(move |window, cx| {
                                        gpui_kit::component::tooltip::Tooltip::new(if ligada {
                                            "Máscara vetorial — clique para editar o caminho dela; ⇧ + clique desliga"
                                        } else {
                                            "Máscara vetorial desligada — ⇧ + clique liga"
                                        })
                                        .build(window, cx)
                                    }),
                                escolhida,
                                cor_da_moldura,
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |ed, e: &MouseDownEvent, window, cx| {
                                    cx.stop_propagation();
                                    if e.modifiers.shift {
                                        ed.na_sessao(cx, |s| {
                                            s.alternar_mascara_vetorial(i);
                                        });
                                    } else {
                                        ed.escolher_camada(i, cx);
                                        ed.escolher_caminho_no_painel(
                                            Some(editor_core::vetor::LugarDoCaminho::Mascara(i)),
                                            cx,
                                        );
                                    }
                                    window.focus(&ed.foco, cx);
                                }),
                            ),
                        )
                    })
                    .child(texto)
                    .when(bloqueada, |d| {
                        d.child(
                            div()
                                .debug_selector(move || format!("editor-cadeado-da-camada-{i}"))
                                .child(
                                    gpui_kit::component::Icon::new(Icone::Lock)
                                        .size_3()
                                        .text_color(tema.muted_foreground),
                                ),
                        )
                    })
                    .when(modo != Modo::Normal, |d| {
                        d.child(
                            div()
                                .text_xs()
                                .text_color(tema.muted_foreground)
                                .child(modo.nome()),
                        )
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |ed, evento: &MouseDownEvent, window, cx| {
                            if evento.modifiers.secondary() {
                                // ⌘ + clique: a seleção do que a camada tem,
                                // sem trocar a escolhida (⇧ soma, ⌥ tira).
                                ed.selecionar_da_camada(i, false, operacao_dos(evento.modifiers), cx);
                            } else if evento.click_count >= 2 {
                                ed.comecar_a_renomear(i, window, cx);
                            } else {
                                if ed.renomeando.as_ref().is_some_and(|(j, _)| *j != i) {
                                    ed.terminar_de_renomear(true, window, cx);
                                }
                                ed.escolher_camada(i, cx);
                                ed.apertou_na_camada();
                                if ed.renomeando.is_none() {
                                    window.focus(&ed.foco, cx);
                                }
                            }
                        }),
                    );
                // Arrastar a linha para outra muda a camada de lugar (um passo
                // só; o conjunto de recorte anda inteiro).
                let cor_do_alvo = tema.muted;
                let linha = linha
                    .on_drag(
                        mascara_e_cadeados::ArrastoDeCamada {
                            nome: nome_do_arrasto.clone(),
                        },
                        |valor, _posicao, _window, cx| {
                            let nome = valor.nome.clone();
                            cx.new(|_| mascara_e_cadeados::FantasmaDaCamada { nome })
                        },
                    )
                    .drag_over::<mascara_e_cadeados::ArrastoDeCamada>(move |estilo, _, _, _| {
                        estilo.bg(cor_do_alvo)
                    })

                    // 🔑 Ao vivo, como as guias (`app/guias.rs`): passar sobre
                    // outra linha já leva a camada para lá — o soltar não
                    // precisa cair numa linha. O arrasto inteiro é um passo.
                    .on_drag_move(cx.listener(
                        move |ed,
                              evento: &gpui_kit::DragMoveEvent<
                            mascara_e_cadeados::ArrastoDeCamada,
                        >,
                              _window,
                              cx| {
                            if evento.bounds.contains(&evento.event.position) {
                                ed.arrastar_camada_ate(i, cx);
                            }
                        },
                    ));
                // O botão direito na linha: o menu da camada (ela passa a ser
                // a escolhida, como no Photoshop).
                let ed = cx.entity();
                let linha = linha.context_menu(move |menu, window, cx| {
                    let menu = match window.focused(cx) {
                        Some(antes) => menu.action_context(antes),
                        None => menu,
                    };
                    let (recortada, pode, quantas, com_mascara, de_pixels) = ed.update(cx, |ed, cx| {
                        ed.escolher_camada(i, cx);
                        ed.sessao()
                            .map(|s| {
                                let c = s.documento().camadas.get(i);
                                (
                                    c.is_some_and(|c| c.recortada),
                                    s.pode_recortar(i),
                                    s.documento().camadas.len(),
                                    c.is_some_and(|c| c.mascara.is_some()),
                                    c.is_some_and(|c| c.ajuste.is_none()),
                                )
                            })
                            .unwrap_or_default()
                    });
                    let item = |id: &'static str, rotulo: &'static str, ligado: bool, fazer: fn(&mut EditorDeFoto, &mut Context<EditorDeFoto>)| {
                        let ed = ed.clone();
                        crate::estilo::item_de_menu(id, na_plataforma(rotulo), None)
                            .disabled(!ligado)
                            .on_click(move |_ev, _window, cx| {
                                ed.update(cx, fazer);
                            })
                    };
                    let recorte = {
                        let ed = ed.clone();
                        crate::estilo::item_de_menu(
                            "editor-menu-recorte",
                            na_plataforma(if recortada {
                                "Liberar máscara de corte  ⌥⌘G"
                            } else {
                                "Criar máscara de corte  ⌥⌘G"
                            }),
                            None,
                        )
                        .disabled(!recortada && !pode)
                        .on_click(move |_ev, _window, cx| {
                            ed.update(cx, |ed, cx| ed.alternar_mascara_de_corte(i, cx));
                        })
                    };
                    menu.item(recorte)
                        .separator()
                        .item(item(
                            "editor-menu-aplicar-mascara",
                            "Aplicar máscara",
                            com_mascara && de_pixels,
                            |ed, cx| ed.aplicar_mascara(cx),
                        ))
                        .item(item(
                            "editor-menu-inverter-mascara",
                            "Inverter máscara",
                            com_mascara,
                            |ed, cx| ed.inverter_mascara(cx),
                        ))
                        .item(item(
                            "editor-menu-vinculo",
                            "Vincular ou soltar a máscara",
                            com_mascara && de_pixels,
                            move |ed, cx| ed.alternar_vinculo_de_ativa(cx),
                        ))
                        .separator()
                        .item(item(
                            "editor-menu-duplicar",
                            "Duplicar camada",
                            true,
                            |ed, cx| ed.duplicar_camada_inteira(cx),
                        ))
                        .item(item(
                            "editor-menu-fotografia",
                            "Criar camada da fotografia base",
                            true,
                            |ed, cx| ed.criar_camada_da_fotografia(cx),
                        ))
                        .item(item(
                            "editor-menu-mesclar",
                            "Mesclar para baixo  ⌘E",
                            i > 0 && quantas > 1,
                            |ed, cx| ed.mesclar_para_baixo(cx),
                        ))
                        .item(item(
                            "editor-menu-excluir",
                            "Excluir camada",
                            quantas > 1,
                            |ed, cx| ed.excluir_camada(cx),
                        ))
                });
                // A divisa com a de baixo: ⌥ + clique cria ou libera a máscara
                // de corte da de cima (a desta linha).
                let divisa = (i > 0).then(|| {
                    div()
                        .id(("editor-divisa", i))
                        .debug_selector(move || format!("editor-divisa-{i}"))
                        .h(px(4.))
                        .mx(px(4.))
                        .rounded(crate::tema::canto(2.))
                        .hover(|d| d.bg(tema.border))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |ed, e: &MouseDownEvent, window, cx| {
                                cx.stop_propagation();
                                if e.modifiers.alt {
                                    ed.alternar_mascara_de_corte(i, cx);
                                } else {
                                    ed.escolher_camada(i, cx);
                                }
                                window.focus(&ed.foco, cx);
                            }),
                        )
                });
                div().flex().flex_col().child(linha).children(divisa)
            });
        let botao = |id: &'static str, icone: Icone, dica: &'static str, ligado: bool| {
            crate::estilo::botao_icone_pequeno(id, icone)
                .debug_selector(move || id.into())
                .tooltip(na_plataforma(dica))
                .disabled(!ligado)
        };
        let c = aparencia::cores(cx);
        let opacidade = self.opacidade_da_camada.clone();
        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h(px(0.))
            // O topo: mesclagem e opacidade numa linha, e os cadeados.
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_shrink_0()
                    .gap(px(6.))
                    .p(px(6.))
                    .border_b_1()
                    .border_color(c.borda)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(medida::VAO_MIUDO))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .debug_selector(|| "editor-modo".into())
                                    .child(crate::estilo::campo_pequeno(
                                        Select::new(&self.modo)
                                            .xsmall()
                                            .disabled(!pode_desfazer_alguma),
                                    )),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_xs()
                                    .text_color(c.apagado)
                                    .child("Opacidade:"),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .w(px(medida::CAMPO_NUMERICO))
                                    .debug_selector(|| "editor-opacidade-da-camada".into())
                                    .child(crate::estilo::campo_pequeno(
                                        Input::new(&self.campo_da_opacidade_da_camada)
                                            .xsmall()
                                            .suffix(div().text_xs().child("%"))
                                            .disabled(!pode_desfazer_alguma),
                                    )),
                            )
                            .child(
                                Popover::new("editor-opacidade-da-camada-slider")
                                    .anchor(gpui_kit::Anchor::TopRight)
                                    .trigger(
                                        crate::estilo::botao_icone(
                                            "editor-opacidade-da-camada-slider",
                                            Icone::ChevronDown,
                                            20.,
                                            12.,
                                        )
                                        .tooltip("Opacidade da camada"),
                                    )
                                    .content(move |_, _window, _cx| {
                                        div()
                                            .w(px(180.))
                                            .h(px(20.))
                                            .child(crate::estilo::slider(&opacidade))
                                    }),
                            ),
                    )
                    .child(cadeados),
            )
            .child(
                div()
                    .id("editor-camadas")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .px(px(4.))
                    .py(px(2.))
                    .children(linhas)
                    .when(pode_desfazer_alguma, |d| d.child(self.linha_do_fundo(lado, cx))),
            )
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap(px(2.))
                    .px(px(4.))
                    .py(px(3.))
                    .border_t_1()
                    .border_color(c.borda)
                    .child({
                        let ed = cx.entity();
                        botao(
                            "editor-camada-ajuste",
                            Icone::Contrast,
                            "Nova camada de ajuste",
                            pode_desfazer_alguma,
                        )
                        .dropdown_menu_with_anchor(
                            gpui_kit::Anchor::BottomLeft,
                            move |mut menu, _window, _cx| {
                                for a in editor_core::ajuste::TODOS {
                                    let ed = ed.clone();
                                    let id: &'static str = match a.chave() {
                                        "brilho" => "editor-ajuste-novo-brilho",
                                        "niveis" => "editor-ajuste-novo-niveis",
                                        "curvas" => "editor-ajuste-novo-curvas",
                                        "matiz" => "editor-ajuste-novo-matiz",
                                        _ => "editor-ajuste-novo-inverter",
                                    };
                                    menu = menu.item(
                                        crate::estilo::item_de_menu(id, a.nome(), None).on_click(
                                            move |_ev, _window, cx| {
                                                ed.update(cx, |ed, cx| {
                                                    ed.nova_camada_de_ajuste(a, cx)
                                                });
                                            },
                                        ),
                                    );
                                }
                                menu
                            },
                        )
                    })
                    .child(
                        botao(
                            "editor-camada-mascara",
                            Icone::LayerMask,
                            "Adicionar máscara (⌥ esconde tudo; com seleção, nasce dela)",
                            pode_desfazer_alguma && !tem_mascara,
                        )
                        .on_click(cx.listener(
                            |ed, e: &gpui_kit::ClickEvent, _, cx| {
                                ed.adicionar_mascara(e.modifiers().alt, cx)
                            },
                        )),
                    )
                    .child(
                        botao(
                            "editor-camada-nova",
                            Icone::Plus,
                            "Nova camada (⇧⌘N)",
                            pode_desfazer_alguma,
                        )
                        .on_click(cx.listener(|ed, _, _, cx| ed.nova_camada(cx))),
                    )
                    // O resto do menu de camadas do Photoshop, com os atalhos.
                    .child({
                        let ed = cx.entity();
                        let (pode_subir, pode_descer) =
                            (ativa + 1 < quantas, ativa > 0 && quantas > 1);
                        botao(
                            "editor-camada-mais",
                            Icone::EllipsisVertical,
                            "Mais: duplicar, fotografia base, máscara de corte, subir, descer, mesclar",
                            pode_desfazer_alguma,
                        )
                        .dropdown_menu_with_anchor(
                            gpui_kit::Anchor::BottomLeft,
                            move |menu, _window, _cx| {
                                let item = |id: &'static str,
                                            rotulo: &'static str,
                                            ligado: bool,
                                            fazer: fn(
                                    &mut EditorDeFoto,
                                    &mut Context<EditorDeFoto>,
                                )| {
                                    let ed = ed.clone();
                                    crate::estilo::item_de_menu(id, na_plataforma(rotulo), None)
                                        .disabled(!ligado)
                                        .on_click(move |_ev, _window, cx| {
                                            ed.update(cx, fazer);
                                        })
                                };
                                let recorte = {
                                    let ed = ed.clone();
                                    crate::estilo::item_de_menu(
                                        "editor-camada-recorte",
                                        na_plataforma(if ativa_recortada {
                                            "Liberar máscara de corte  ⌥⌘G"
                                        } else {
                                            "Criar máscara de corte  ⌥⌘G"
                                        }),
                                        None,
                                    )
                                    .disabled(!ativa_recortada && !ativa_pode_recortar)
                                    .on_click(move |_ev, _window, cx| {
                                        ed.update(cx, |ed, cx| {
                                            ed.alternar_mascara_de_corte(ativa, cx)
                                        });
                                    })
                                };
                                menu.item(item(
                                    "editor-camada-duplicar",
                                    "Duplicar camada (exata)",
                                    true,
                                    |ed, cx| ed.duplicar_camada_inteira(cx),
                                ))
                                .item(item(
                                    "editor-camada-via-copia",
                                    "Camada via cópia  ⌘J",
                                    true,
                                    |ed, cx| ed.duplicar_camada(cx),
                                ))
                                .item(item(
                                    "editor-camada-fotografia",
                                    if criando_a_fotografia {
                                        "Criando a camada da fotografia…"
                                    } else {
                                        "Criar camada da fotografia base"
                                    },
                                    !criando_a_fotografia,
                                    |ed, cx| ed.criar_camada_da_fotografia(cx),
                                ))
                                .item(item(
                                    "editor-camada-carimbar",
                                    "Carimbar visível  ⇧⌥⌘E",
                                    true,
                                    |ed, cx| ed.carimbar_visivel(cx),
                                ))
                                .item(item(
                                    "editor-camada-importar",
                                    "Importar imagem como camada…",
                                    true,
                                    |ed, cx| ed.importar_imagem(cx),
                                ))
                                .separator()
                                .item(recorte)
                                .item(item(
                                    "editor-camada-aplicar-mascara",
                                    "Aplicar máscara",
                                    true,
                                    |ed, cx| ed.aplicar_mascara(cx),
                                ))
                                .separator()
                                .item(item(
                                    "editor-camada-subir",
                                    "Subir a camada  ⌘]",
                                    pode_subir,
                                    |ed, cx| ed.mover_camada(1, cx),
                                ))
                                .item(item(
                                    "editor-camada-descer",
                                    "Descer a camada  ⌘[",
                                    pode_descer,
                                    |ed, cx| ed.mover_camada(-1, cx),
                                ))
                                .item(item(
                                    "editor-camada-mesclar",
                                    if ativa_e_base {
                                        "Mesclar máscara de corte  ⌘E"
                                    } else {
                                        "Mesclar para baixo  ⌘E"
                                    },
                                    pode_descer || ativa_e_base,
                                    |ed, cx| ed.mesclar_para_baixo(cx),
                                ))
                            },
                        )
                    })
                    .child(div().flex_1())
                    .child(
                        botao(
                            "editor-camada-excluir",
                            Icone::Trash2,
                            if na_mascara {
                                "Excluir a máscara"
                            } else {
                                "Excluir a camada"
                            },
                            quantas > 1 || na_mascara,
                        )
                        .on_click(cx.listener(move |ed, _, _, cx| {
                            if na_mascara {
                                ed.excluir_mascara(cx)
                            } else {
                                ed.excluir_camada(cx)
                            }
                        })),
                    ),
            )
    }

    /// O painel Histórico do Photoshop: a abertura e cada passo, do mais
    /// antigo ao mais novo; o vigente realçado, os desfeitos apagados. Clicar
    /// num passo volta (ou avança) até ele.
    pub(super) fn painel_do_historico(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme().clone();
        let (nomes, posicao) = match self.sessao() {
            Some(s) => (
                std::iter::once("Abertura".to_string())
                    .chain(
                        s.historico()
                            .passos()
                            .iter()
                            .map(|p| p.descricao(s.documento())),
                    )
                    .collect::<Vec<_>>(),
                s.historico().posicao(),
            ),
            None => (Vec::new(), 0),
        };
        let linhas = nomes.into_iter().enumerate().map(|(i, nome)| {
            let vigente = i == posicao;
            let desfeito = i > posicao;
            div()
                .id(("editor-historico", i))
                .debug_selector(move || format!("editor-historico-{i}"))
                .px(px(6.))
                .py(px(2.))
                .rounded(crate::tema::canto(4.))
                .text_sm()
                .cursor_pointer()
                .when(vigente, |d| {
                    d.bg(tema.accent).text_color(tema.accent_foreground)
                })
                .when(desfeito, |d| {
                    d.text_color(tema.muted_foreground).opacity(0.6)
                })
                .when(!vigente, |d| d.hover(|d| d.bg(tema.muted)))
                .child(nome)
                .on_click(cx.listener(move |ed, _, _, cx| ed.ir_para_no_historico(i, cx)))
        });
        div()
            .id("editor-historico")
            .flex()
            .flex_col()
            .gap(px(1.))
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .children(linhas)
    }
}
