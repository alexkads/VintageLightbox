//! 📋 Os menus do editor: Arquivo, Editar, Camada, Selecionar, Filtro,
//! Visualizar, Janela e Ajuda — na faixa de cima da janela, que também é a
//! barra de título onde o app desenha a própria (arrasta, duplo clique
//! maximiza; no GNOME, os botões da janela no fim).
//!
//! 🔑 **Cada item chama o mesmo método do atalho e do botão**, e o atalho
//! escrito ao lado sai da própria ligação de teclas (`window` acha a tecla da
//! ação no contexto do editor), na escrita da plataforma — ⌘ no macOS, Ctrl
//! no Windows e no Linux. Só entra o que funciona; o que não vale agora fica
//! apagado.
//!
//! ⚠️ **Por que dentro da janela, e não no menu do macOS.** O menu do sistema
//! é um só para todas as janelas do app, e o dono quis nele só o do app (sem
//! Editar e sem Janela, 16/09). Nos menus da janela, o atalho de uma não
//! captura a outra nem o campo de texto: as teclas continuam as do contexto
//! `EditorDeFoto`.

use gpui_kit::component::menu::{DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_kit::{
    div, prelude::*, px, Action, AnyElement, AsKeystroke as _, Context, Entity, FocusHandle, Window,
};

use super::aparencia::{self, medida};
use super::area_de_trabalho::QualPainel;
use super::ferramentas::{na_plataforma, nome_com_letra, FERRAMENTAS};
use super::filtro::Tipo;
use super::{EditorDeFoto, Modificacao};
use crate::editor::*;
use editor_core::Sessao;

/// A tecla de uma ação no contexto do editor, como a plataforma escreve.
pub fn atalho_da_acao(acao: &dyn Action, foco: &FocusHandle, window: &Window) -> Option<String> {
    let ligacao = window.highest_precedence_binding_for_action_in(acao, foco)?;
    let tecla = ligacao.keystrokes().first()?;
    Some(gpui_kit::component::kbd::Kbd::format(tecla.as_keystroke()))
}

/// O que os menus precisam saber do editor na hora de abrir.
struct Estado {
    pronta: bool,
    pode_desfazer: bool,
    pode_refazer: bool,
    desfazer: String,
    refazer: String,
    tem_selecao: bool,
    camadas: usize,
    ativa: usize,
    com_mascara: bool,
    de_pixels: bool,
    recortada: bool,
    pode_recortar: bool,
    tem_copia: bool,
    giro: bool,
    antes: bool,
    pode_reselecionar: bool,
    reguas: bool,
    na_mascara: bool,
    guias_visiveis: bool,
    guias_travadas: bool,
    tem_guias: bool,
    ajustar: bool,
}

impl EditorDeFoto {
    fn estado_dos_menus(&self) -> Estado {
        let s = self.sessao();
        let passo = |p: Option<&editor_core::Comando>, verbo: &str| match (s, p) {
            (Some(s), Some(p)) => format!("{verbo} {}", p.descricao(s.documento())),
            _ => verbo.to_string(),
        };
        let camada = s.map(|s| s.camada_ativa());
        Estado {
            pronta: self.pronta(),
            pode_desfazer: s.is_some_and(|s| s.historico().pode_desfazer()),
            pode_refazer: s.is_some_and(|s| s.historico().pode_refazer()),
            desfazer: passo(s.and_then(|s| s.historico().a_desfazer()), "Desfazer"),
            refazer: passo(s.and_then(|s| s.historico().a_refazer()), "Refazer"),
            tem_selecao: s.and_then(Sessao::selecao).is_some(),
            camadas: s.map_or(0, |s| s.documento().camadas.len()),
            ativa: s.map_or(0, Sessao::ativa),
            com_mascara: camada.is_some_and(|c| c.mascara.is_some()),
            de_pixels: camada.is_some_and(|c| c.ajuste.is_none()),
            recortada: camada.is_some_and(|c| c.recortada),
            pode_recortar: s.is_some_and(|s| s.pode_recortar(s.ativa())),
            tem_copia: self.copiado.is_some(),
            giro: self.giro != 0.0,
            antes: self.mostrando_antes(),
            pode_reselecionar: s.is_some_and(Sessao::pode_reselecionar),
            reguas: self.reguas_ligadas(),
            na_mascara: self.na_mascara(),
            guias_visiveis: self.guias_visiveis(),
            guias_travadas: self.guias_travadas(),
            tem_guias: s.is_some_and(|s| !s.guias().is_empty()),
            ajustar: self.ajustar_ligado(),
        }
    }

    /// A faixa dos menus (e da barra de título, onde o app desenha a dele).
    pub(super) fn linha_de_menus(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let c = aparencia::cores(cx);
        let fraca = cx.entity().downgrade();
        let controles = crate::janela::controles_com_fechar(
            "janela-editor",
            c.texto,
            window,
            cx,
            move |window, cx| {
                let _ = fraca.update(cx, |ed, cx| ed.fechar(window, cx));
            },
        );
        let ed = cx.entity();
        let foco = self.foco.clone();
        let menu = |id: &'static str,
                    rotulo: &'static str,
                    montar: fn(
            PopupMenu,
            &Entity<EditorDeFoto>,
            &FocusHandle,
            &mut Window,
            &mut Context<PopupMenu>,
        ) -> PopupMenu,
                    cx: &mut Context<Self>| {
            let (ed, foco) = (ed.clone(), foco.clone());
            crate::estilo::botao_fantasma_pequeno(id, cx)
                .label(rotulo)
                .dropdown_menu_with_anchor(gpui_kit::Anchor::TopLeft, move |m, window, cx| {
                    montar(m.min_w(px(260.)), &ed, &foco, window, cx)
                })
        };
        crate::janela::como_barra_de_titulo(div(), "barra-do-editor", window, cx)
            .flex()
            .flex_shrink_0()
            .items_center()
            .h(px(medida::ALTURA_DOS_MENUS))
            .pl(px(4.))
            .pr(px(4.))
            .bg(c.cromo)
            .border_b_1()
            .border_color(c.borda)
            .child(menu("editor-menu-arquivo", "Arquivo", menu_arquivo, cx))
            .child(menu("editor-menu-editar", "Editar", menu_editar, cx))
            .child(menu("editor-menu-imagem", "Imagem", menu_imagem, cx))
            .child(menu("editor-menu-camada", "Camada", menu_camada, cx))
            .child(menu(
                "editor-menu-selecionar",
                "Selecionar",
                menu_selecionar,
                cx,
            ))
            .child(menu("editor-menu-filtro", "Filtro", menu_filtro, cx))
            .child(menu(
                "editor-menu-visualizar",
                "Visualizar",
                menu_visualizar,
                cx,
            ))
            .child(menu("editor-menu-janela", "Janela", menu_janela, cx))
            .child(menu("editor-menu-ajuda", "Ajuda", menu_ajuda, cx))
            .child(div().flex_1())
            .child(controles)
            .into_any_element()
    }
}

/// Um item que chama o editor; o atalho, quando há, sai da ação.
fn item(
    ed: &Entity<EditorDeFoto>,
    id: &'static str,
    rotulo: impl Into<gpui_kit::SharedString>,
    atalho: Option<String>,
    ligado: bool,
    fazer: impl Fn(&mut EditorDeFoto, &mut Window, &mut Context<EditorDeFoto>) + 'static,
) -> PopupMenuItem {
    let ed = ed.clone();
    crate::estilo::item_de_menu_com_atalho(id, rotulo, atalho)
        .disabled(!ligado)
        .on_click(move |_, window, cx| {
            ed.update(cx, |ed, cx| fazer(ed, window, cx));
        })
}

/// Um item de uma ação: dispara a ação no editor (o mesmo caminho da tecla).
fn acao(
    ed: &Entity<EditorDeFoto>,
    foco: &FocusHandle,
    window: &Window,
    id: &'static str,
    rotulo: impl Into<gpui_kit::SharedString>,
    acao: impl Action + Clone,
    ligado: bool,
) -> PopupMenuItem {
    let atalho = atalho_da_acao(&acao, foco, window);
    let foco = foco.clone();
    let _ = ed;
    crate::estilo::item_de_menu_com_atalho(id, rotulo, atalho)
        .disabled(!ligado)
        .on_click(move |_, window, cx| {
            // No nó do editor, como a tecla: o menu que fecha não está no
            // caminho do foco.
            let (foco, acao) = (foco.clone(), acao.clone());
            window.defer(cx, move |window, cx| {
                window.focus(&foco, cx);
                foco.dispatch_action(&acao, window, cx);
            });
        })
}

fn menu_arquivo(
    m: PopupMenu,
    ed: &Entity<EditorDeFoto>,
    foco: &FocusHandle,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let e = ed.read(cx).estado_dos_menus();
    let salvando = ed.read(cx).salvando();
    m.item(acao(
        ed,
        foco,
        window,
        "editor-menu-salvar",
        "Salvar",
        SalvarNoEditor,
        e.pronta && !salvando,
    ))
    .item(item(
        ed,
        "editor-menu-importar",
        "Importar imagem como camada…",
        None,
        e.pronta,
        |ed, _, cx| ed.importar_imagem(cx),
    ))
    .separator()
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-fechar",
        "Fechar",
        FecharEditor,
        true,
    ))
}

fn menu_editar(
    m: PopupMenu,
    ed: &Entity<EditorDeFoto>,
    foco: &FocusHandle,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let e = ed.read(cx).estado_dos_menus();
    let p = e.pronta;
    m.item(acao(
        ed,
        foco,
        window,
        "editor-menu-desfazer",
        e.desfazer.clone(),
        DesfazerNoEditor,
        e.pode_desfazer,
    ))
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-refazer",
        e.refazer.clone(),
        RefazerNoEditor,
        e.pode_refazer,
    ))
    .separator()
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-recortar",
        "Recortar",
        Recortar,
        p && e.tem_selecao,
    ))
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-copiar",
        "Copiar",
        Copiar,
        p,
    ))
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-copiar-mesclado",
        "Copiar mesclado",
        CopiarMesclado,
        p,
    ))
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-colar",
        "Colar",
        Colar,
        p,
    ))
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-colar-no-lugar",
        "Colar no lugar",
        ColarNoLugar,
        p && e.tem_copia,
    ))
    .separator()
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-preencher-conteudo",
        "Preencher…",
        PreencherPeloConteudo,
        p,
    ))
    .item(item(
        ed,
        "editor-abrir-preenchimento",
        "Preenchimento sensível ao conteúdo…",
        None,
        p,
        |ed, _, cx| ed.abrir_preenchimento(cx),
    ))
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-apagar",
        "Apagar",
        ApagarSelecao,
        p && e.tem_selecao,
    ))
    .separator()
    .item(acao(
        ed,
        foco,
        window,
        "editor-transformar-livre",
        "Transformação livre",
        TransformacaoLivre,
        p,
    ))
    .item(item(
        ed,
        "editor-transformar-deformar",
        "Transformar: Deformar",
        None,
        p,
        |ed, _, cx| ed.deformar(cx),
    ))
}

/// Imagem › Ajustes, com as teclas do Photoshop (⌘L, ⌘M, ⌘U, ⌘I). 🔑 Aqui
/// eles nascem como **camada de ajuste** — a foto embaixo fica intacta e a
/// seleção vira a máscara —, e o Inverter é o de sempre (cores da camada ou a
/// máscara). Girar e redimensionar a foto inteira não entram no editor (C31).
fn menu_imagem(
    m: PopupMenu,
    ed: &Entity<EditorDeFoto>,
    foco: &FocusHandle,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let p = ed.read(cx).estado_dos_menus().pronta;
    let (ed2, foco2) = (ed.clone(), foco.clone());
    m.submenu("Ajustes", window, cx, move |sub, window, _cx| {
        let (ed, foco) = (&ed2, &foco2);
        sub.item(item(
            ed,
            "editor-imagem-brilho",
            "Brilho/Contraste…",
            None,
            p,
            |ed, _, cx| ed.ajuste_pela_chave("brilho", cx),
        ))
        .separator()
        .item(acao(
            ed,
            foco,
            window,
            "editor-imagem-niveis",
            "Níveis…",
            AjusteNiveis,
            p,
        ))
        .item(acao(
            ed,
            foco,
            window,
            "editor-imagem-curvas",
            "Curvas…",
            AjusteCurvas,
            p,
        ))
        .separator()
        .item(acao(
            ed,
            foco,
            window,
            "editor-imagem-matiz",
            "Matiz/Saturação…",
            AjusteMatiz,
            p,
        ))
        .separator()
        .item(acao(
            ed,
            foco,
            window,
            "editor-imagem-inverter",
            "Inverter",
            Inverter,
            p,
        ))
    })
}

fn menu_camada(
    m: PopupMenu,
    ed: &Entity<EditorDeFoto>,
    foco: &FocusHandle,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let e = ed.read(cx).estado_dos_menus();
    let p = e.pronta;
    let ed_ajustes = ed.clone();
    let ed_mascara = ed.clone();
    let (com_mascara, de_pixels) = (e.com_mascara, e.de_pixels);
    m.item(acao(
        ed,
        foco,
        window,
        "editor-menu-nova-camada",
        "Nova camada",
        NovaCamada,
        p,
    ))
    .item(item(
        ed,
        "editor-menu-duplicar-camada",
        "Duplicar camada",
        None,
        p,
        |ed, _, cx| ed.duplicar_camada_inteira(cx),
    ))
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-via-copia",
        "Camada via cópia",
        DuplicarCamada,
        p,
    ))
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-via-recorte",
        "Camada via recorte",
        CamadaViaRecorte,
        p && e.tem_selecao,
    ))
    .item(item(
        ed,
        "editor-menu-excluir-camada",
        "Excluir camada",
        None,
        p && e.camadas > 1,
        |ed, _, cx| ed.excluir_camada(cx),
    ))
    .separator()
    .submenu(
        "Nova camada de ajuste",
        window,
        cx,
        move |mut sub, _window, _cx| {
            for a in editor_core::ajuste::TODOS {
                let ed = ed_ajustes.clone();
                sub = sub.item(PopupMenuItem::new(format!("{}…", a.nome())).on_click(
                    move |_, _, cx| {
                        ed.update(cx, |ed, cx| ed.nova_camada_de_ajuste(a, cx));
                    },
                ));
            }
            sub
        },
    )
    .submenu(
        "Máscara de camada",
        window,
        cx,
        move |sub, _window, _cx| {
            let ed = &ed_mascara;
            sub.item(item(
                ed,
                "editor-menu-mascara-revelar",
                "Revelar tudo",
                None,
                !com_mascara,
                |ed, _, cx| ed.adicionar_mascara(false, cx),
            ))
            .item(item(
                ed,
                "editor-menu-mascara-ocultar",
                "Ocultar tudo",
                None,
                !com_mascara,
                |ed, _, cx| ed.adicionar_mascara(true, cx),
            ))
            .separator()
            .item(item(
                ed,
                "editor-menu-mascara-excluir",
                "Excluir",
                None,
                com_mascara,
                |ed, _, cx| ed.excluir_mascara(cx),
            ))
            .item(item(
                ed,
                "editor-menu-mascara-aplicar",
                "Aplicar",
                None,
                com_mascara && de_pixels,
                |ed, _, cx| ed.aplicar_mascara(cx),
            ))
            .item(item(
                ed,
                "editor-menu-mascara-inverter",
                "Inverter",
                None,
                com_mascara,
                |ed, _, cx| ed.inverter_mascara(cx),
            ))
            .item(item(
                ed,
                "editor-menu-mascara-vinculo",
                "Vincular ou soltar",
                None,
                com_mascara && de_pixels,
                |ed, _, cx| ed.alternar_vinculo_de_ativa(cx),
            ))
        },
    )
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-mascara-de-corte",
        if e.recortada {
            "Liberar máscara de corte"
        } else {
            "Criar máscara de corte"
        },
        AlternarMascaraDeCorte,
        p && (e.recortada || e.pode_recortar),
    ))
    .separator()
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-subir",
        "Subir a camada",
        SubirCamada,
        p && e.ativa + 1 < e.camadas,
    ))
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-descer",
        "Descer a camada",
        DescerCamada,
        p && e.ativa > 0,
    ))
    .separator()
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-mesclar",
        "Mesclar para baixo",
        MesclarParaBaixo,
        p && e.camadas > 1,
    ))
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-carimbar",
        "Carimbar visível",
        CarimbarVisivel,
        p,
    ))
    .item(item(
        ed,
        "editor-menu-fotografia",
        "Criar camada da fotografia base",
        None,
        p && !ed.read(cx).criando_a_fotografia(),
        |ed, _, cx| ed.criar_camada_da_fotografia(cx),
    ))
    .separator()
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-mostrar-camada",
        "Mostrar ou ocultar a camada",
        AlternarCamada,
        p,
    ))
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-bloquear",
        "Bloquear transparência",
        BloquearTransparencia,
        p,
    ))
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-inverter",
        "Inverter (cores ou máscara)",
        Inverter,
        p,
    ))
}

fn menu_selecionar(
    m: PopupMenu,
    ed: &Entity<EditorDeFoto>,
    foco: &FocusHandle,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let e = ed.read(cx).estado_dos_menus();
    let (p, sel) = (e.pronta, e.tem_selecao);
    let modificar = |id: &'static str, rotulo: &'static str, md: Modificacao| {
        item(ed, id, rotulo, None, p && sel, move |ed, window, cx| {
            ed.abrir_modificacao(md, window, cx)
        })
    };
    m.item(acao(
        ed,
        foco,
        window,
        "editor-menu-tudo",
        "Tudo",
        SelecionarTudo,
        p,
    ))
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-desmarcar",
        "Desmarcar",
        Desmarcar,
        p && sel,
    ))
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-reselecionar",
        "Reselecionar",
        Reselecionar,
        p && e.pode_reselecionar,
    ))
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-inverter-selecao",
        "Inverter",
        InverterSelecao,
        p && sel,
    ))
    .separator()
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-difundir",
        "Difundir…",
        DifundirSelecao,
        p && sel,
    ))
    .item(modificar(
        "editor-menu-expandir",
        "Expandir…",
        Modificacao::Expandir,
    ))
    .item(modificar(
        "editor-menu-contrair",
        "Contrair…",
        Modificacao::Contrair,
    ))
    .separator()
    .item(item(
        ed,
        "editor-menu-transformar-selecao",
        "Transformar seleção",
        None,
        p && sel,
        |ed, _, cx| ed.transformar_selecao(cx),
    ))
}

fn menu_filtro(
    m: PopupMenu,
    ed: &Entity<EditorDeFoto>,
    foco: &FocusHandle,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let e = ed.read(cx).estado_dos_menus();
    let pode = e.pronta && (e.de_pixels || e.na_mascara);
    let (ed_d, ed_n, ed_r, ed_o) = (ed.clone(), ed.clone(), ed.clone(), ed.clone());
    m.item(acao(
        ed,
        foco,
        window,
        "editor-filtro-liquidificar",
        "Liquidificar…",
        Liquidificar,
        e.pronta && e.de_pixels,
    ))
    .separator()
    .submenu("Desfoque", window, cx, move |sub, _window, _cx| {
        sub.item(filtro(
            &ed_d,
            "editor-filtro-desfoque",
            Tipo::Desfoque,
            pode,
        ))
        .item(filtro(
            &ed_d,
            "editor-filtro-superficie",
            Tipo::Superficie,
            pode,
        ))
    })
    .submenu("Nitidez", window, cx, move |sub, _window, _cx| {
        sub.item(filtro(&ed_n, "editor-filtro-nitidez", Tipo::Nitidez, pode))
    })
    .submenu("Ruído", window, cx, move |sub, _window, _cx| {
        sub.item(filtro(&ed_r, "editor-filtro-ruido", Tipo::Ruido, pode))
            .item(filtro(&ed_r, "editor-filtro-mediana", Tipo::Mediana, pode))
    })
    .submenu("Outros", window, cx, move |sub, _window, _cx| {
        sub.item(filtro(
            &ed_o,
            "editor-filtro-alta-frequencia",
            Tipo::AltaFrequencia,
            pode,
        ))
    })
}

/// Um item de Filtro: abre o diálogo daquele filtro ("…" como no Photoshop).
fn filtro(ed: &Entity<EditorDeFoto>, id: &'static str, tipo: Tipo, pode: bool) -> PopupMenuItem {
    item(
        ed,
        id,
        format!("{}…", tipo.titulo()),
        None,
        pode,
        move |ed, window, cx| ed.abrir_filtro(tipo, window, cx),
    )
}

fn menu_visualizar(
    m: PopupMenu,
    ed: &Entity<EditorDeFoto>,
    foco: &FocusHandle,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let e = ed.read(cx).estado_dos_menus();
    let p = e.pronta;
    m.item(acao(
        ed,
        foco,
        window,
        "editor-menu-ampliar",
        "Ampliar",
        Aproximar,
        p,
    ))
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-reduzir",
        "Reduzir",
        Afastar,
        p,
    ))
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-encaixar",
        "Encaixar na tela",
        Encaixar,
        p,
    ))
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-100",
        "100%",
        UmPorUm,
        p,
    ))
    .separator()
    .item(
        acao(
            ed,
            foco,
            window,
            "editor-antes-depois",
            "Antes/Depois",
            AlternarAntesDepois,
            p,
        )
        .checked(e.antes),
    )
    .item(acao(
        ed,
        foco,
        window,
        "editor-menu-rubi",
        "Máscara em rubi",
        AlternarRubi,
        p && e.com_mascara,
    ))
    .separator()
    .item(
        acao(
            ed,
            foco,
            window,
            "editor-menu-reguas",
            "Réguas",
            AlternarReguas,
            true,
        )
        .checked(e.reguas),
    )
    .item(
        acao(
            ed,
            foco,
            window,
            "editor-menu-guias",
            "Guias",
            AlternarGuias,
            true,
        )
        .checked(e.guias_visiveis),
    )
    .item(
        acao(
            ed,
            foco,
            window,
            "editor-menu-travar-guias",
            "Travar guias",
            TravarGuias,
            true,
        )
        .checked(e.guias_travadas),
    )
    .item(
        acao(
            ed,
            foco,
            window,
            "editor-menu-ajustar",
            "Ajustar",
            AjustarAsGuias,
            true,
        )
        .checked(e.ajustar),
    )
    .item(item(
        ed,
        "editor-menu-limpar-guias",
        "Limpar guias",
        None,
        p && e.tem_guias,
        |ed, _, cx| ed.limpar_guias(cx),
    ))
    .separator()
    .item(item(
        ed,
        "editor-menu-redefinir-vista",
        "Redefinir a rotação da vista",
        None,
        e.giro,
        |ed, _, cx| ed.girar_a_vista(0.0, cx),
    ))
}

fn menu_janela(
    m: PopupMenu,
    ed: &Entity<EditorDeFoto>,
    foco: &FocusHandle,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let (duas, recolhido) = {
        let e = ed.read(cx);
        (e.barra_em_duas_colunas, e.arranjo().recolhido)
    };
    let mut m = m;
    for qual in QualPainel::TODOS {
        let visivel = ed.read(cx).painel_na_frente(qual, cx);
        let id: &'static str = match qual {
            QualPainel::Cor => "editor-janela-cor",
            QualPainel::Amostras => "editor-janela-amostras",
            QualPainel::Propriedades => "editor-janela-propriedades",
            QualPainel::Pincel => "editor-janela-pincel",
            QualPainel::Historico => "editor-janela-historico",
            QualPainel::Camadas => "editor-janela-camadas",
            QualPainel::Navegador => "editor-janela-navegador",
            QualPainel::Info => "editor-janela-info",
            QualPainel::Ajustes => "editor-janela-ajustes",
        };
        m = m.item(
            item(ed, id, qual.titulo(), None, true, move |ed, window, cx| {
                ed.alternar_painel(qual, window, cx)
            })
            .checked(visivel),
        );
    }
    m.separator()
        .item(
            item(
                ed,
                "editor-janela-duas-colunas",
                "Ferramentas em duas colunas",
                None,
                true,
                |ed, _, cx| ed.alternar_colunas_da_barra(cx),
            )
            .checked(duas),
        )
        .item(
            item(
                ed,
                "editor-janela-recolher",
                "Recolher painéis em ícones",
                None,
                true,
                |ed, _, cx| ed.alternar_recolhido(cx),
            )
            .checked(recolhido),
        )
        .item(acao(
            ed,
            foco,
            window,
            "editor-janela-ocultar-paineis",
            "Ocultar ou mostrar os painéis",
            AlternarPaineis,
            true,
        ))
        .item(acao(
            ed,
            foco,
            window,
            "editor-janela-ocultar-tudo",
            "Ocultar ou mostrar ferramentas e painéis",
            AlternarInterface,
            true,
        ))
        .separator()
        .item(item(
            ed,
            "editor-restaurar-area",
            "Restaurar a área de trabalho padrão",
            None,
            true,
            |ed, window, cx| ed.restaurar_area_de_trabalho(window, cx),
        ))
}

fn menu_ajuda(
    m: PopupMenu,
    ed: &Entity<EditorDeFoto>,
    _foco: &FocusHandle,
    _window: &mut Window,
    _cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    m.item(item(
        ed,
        "editor-ajuda-atalhos",
        "Atalhos de teclado do editor…",
        None,
        true,
        |ed, _, cx| {
            ed.mostrando_atalhos = true;
            cx.notify();
        },
    ))
}

impl EditorDeFoto {
    /// A Ajuda: as ferramentas com a letra e os comandos com a tecla, das
    /// mesmas fontes da barra e dos menus.
    pub(super) fn dialogo_dos_atalhos(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let c = aparencia::cores(cx);
        let foco = self.foco.clone();
        let linha = |nome: String, tecla: String| {
            div()
                .flex()
                .justify_between()
                .gap(px(12.))
                .text_xs()
                .child(nome)
                .child(div().text_color(c.apagado).child(tecla))
        };
        let ferramentas = FERRAMENTAS.iter().map(|d| {
            linha(
                nome_com_letra(d)
                    .split(" (")
                    .next()
                    .unwrap_or_default()
                    .to_string(),
                d.letra
                    .map(|l| l.to_ascii_uppercase().to_string())
                    .unwrap_or_else(|| "—".into()),
            )
        });
        let comandos: Vec<(&str, Box<dyn Action>)> = vec![
            ("Salvar", Box::new(SalvarNoEditor)),
            ("Desfazer", Box::new(DesfazerNoEditor)),
            ("Refazer", Box::new(RefazerNoEditor)),
            ("Transformação livre", Box::new(TransformacaoLivre)),
            ("Nova camada", Box::new(NovaCamada)),
            ("Camada via cópia", Box::new(DuplicarCamada)),
            ("Mesclar para baixo", Box::new(MesclarParaBaixo)),
            ("Selecionar tudo", Box::new(SelecionarTudo)),
            ("Desmarcar", Box::new(Desmarcar)),
            ("Inverter seleção", Box::new(InverterSelecao)),
            ("Reselecionar", Box::new(Reselecionar)),
            ("Níveis", Box::new(AjusteNiveis)),
            ("Curvas", Box::new(AjusteCurvas)),
            ("Matiz/Saturação", Box::new(AjusteMatiz)),
            ("Trocar frente e fundo", Box::new(TrocarCores)),
            ("Preto e branco", Box::new(CoresPadrao)),
            ("Ampliar", Box::new(Aproximar)),
            ("Reduzir", Box::new(Afastar)),
            ("Encaixar na tela", Box::new(Encaixar)),
            ("100%", Box::new(UmPorUm)),
            ("Réguas", Box::new(AlternarReguas)),
            ("Guias", Box::new(AlternarGuias)),
            ("Travar guias", Box::new(TravarGuias)),
            ("Ajustar", Box::new(AjustarAsGuias)),
            ("Ocultar ferramentas e painéis", Box::new(AlternarInterface)),
            ("Ocultar só os painéis", Box::new(AlternarPaineis)),
            ("Liquidificar", Box::new(Liquidificar)),
        ];
        let comandos: Vec<_> = comandos
            .into_iter()
            .filter_map(|(nome, a)| {
                atalho_da_acao(a.as_ref(), &foco, window).map(|t| linha(nome.to_string(), t))
            })
            .collect();
        Some(
            gpui_kit::component::v_flex()
                .gap(px(12.))
                .debug_selector(|| "editor-dialogo-atalhos".into())
                .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(crate::estilo::cabecalho_do_dialogo(
                    "Atalhos de teclado do editor",
                    na_plataforma("Espaço segurado: Mão · ⇧ + letra: a próxima do grupo · [ ] tamanho · { } dureza"),
                    None,
                    cx,
                ))
                .child(
                    div()
                        .flex()
                        .gap(px(24.))
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap(px(3.))
                                .child(div().text_sm().child("Ferramentas"))
                                .children(ferramentas),
                        )
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap(px(3.))
                                .child(div().text_sm().child("Comandos"))
                                .children(comandos),
                        ),
                )
                .child(
                    crate::estilo::rodape_do_dialogo().child(
                        crate::estilo::botao_primario("editor-fechar-atalhos", cx)
                            .child("Fechar")
                            .on_click(cx.listener(|ed, _, _, cx| {
                                ed.mostrando_atalhos = false;
                                cx.notify();
                            })),
                    ),
                )
                .into_any_element(),
        )
    }
}
