//! 🖱️ O menu do botão direito na tira da sessão — o par do menu da tira da
//! Revelação (`revelacao/tela/tira.rs`), com o que a sessão faz com as fotos.
//!
//! Dono, 08/10/2026: *"Eu quero também um menu como esse dentro na sessão
//! fotográfica na filmstrip com as opções de lá"*. As opções são as mesmas do
//! painel da direita e das teclas da tira (nota, `P`, `X`, Negociação,
//! Exportar, Excluir); o menu só as junta onde a mão já está.
//!
//! 🔑 **Sobre quem age**: o botão direito numa foto fora da seleção a seleciona
//! sozinha, como no Finder e no Lightroom; numa foto da seleção, o lote fica e
//! ela vira a do foco. Daí em diante cada item chama o mesmo método da tecla
//! ou do painel, que age sobre as marcadas — as regras (sem nota não vai ao
//! balcão, comprada não se rejeita…) são as de sempre, com os avisos de sempre.

use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::component::Icon;
use gpui_kit::{px, App, ClickEvent, Context, WeakEntity, Window};

use biblioteca_core::acervo;
use biblioteca_core::selecao::Modificadores;

use super::{Detalhe, Pedido};
use crate::recursos::Icone;

/// O que o menu precisa saber, lido uma vez ao abrir.
pub(super) struct DadosDoMenu {
    titulo: String,
    marcadas: usize,
    /// A clicada pode abrir na Revelação.
    revelavel: bool,
    /// A nota comum às marcadas (`None` se mista ou sem nota).
    nota: Option<u8>,
    /// Das marcadas, as que podem ir ao balcão ou voltar à venda.
    editaveis: usize,
    todas_levadas: bool,
    /// Das marcadas, as que a rejeição alcança (nem comprada, nem levada).
    rejeitaveis: usize,
    todas_rejeitadas: bool,
    negociaveis: usize,
    nao_salvas: usize,
    excluiveis: usize,
}

impl Detalhe {
    /// O botão direito numa miniatura da tira: ajeita a seleção e guarda a
    /// clicada para o menu, que é montado depois do evento.
    pub(super) fn apontar_o_menu(&mut self, posicao: usize, cx: &mut Context<Self>) {
        if self.selecao.tem(posicao) {
            self.selecao.focar(Some(posicao));
        } else {
            self.clicar(posicao, Modificadores::default(), cx);
        }
        self.menu_da_tira = Some(posicao);
        cx.notify();
    }

    /// Os números do menu, contados sobre as marcadas.
    pub(super) fn dados_do_menu(&mut self) -> Option<DadosDoMenu> {
        let posicao = self.menu_da_tira.take()?;
        let clicada = self.acervo.visivel(posicao)?.clone();
        let marcadas: Vec<acervo::Foto> = self
            .selecao
            .marcadas()
            .filter_map(|p| self.acervo.visivel(p))
            .cloned()
            .collect();
        let editaveis: Vec<&acervo::Foto> = marcadas.iter().filter(|f| f.editavel()).collect();
        let rejeitaveis: Vec<&acervo::Foto> = marcadas
            .iter()
            .filter(|f| {
                !matches!(
                    f.estado,
                    acervo::Estado::Comprada | acervo::Estado::LevadaNoBalcao
                )
            })
            .collect();
        let primeira_nota = marcadas.first().and_then(|f| f.nota);
        Some(DadosDoMenu {
            titulo: if marcadas.len() > 1 {
                format!("{} fotos selecionadas", marcadas.len())
            } else {
                clicada.arquivo.clone()
            },
            marcadas: marcadas.len(),
            revelavel: self.pedido_de_revelar(Some(&clicada.id)).is_some(),
            nota: primeira_nota
                .filter(|_| marcadas.iter().all(|f| f.nota == primeira_nota))
                .map(|n| n.min(5)),
            editaveis: editaveis.len(),
            todas_levadas: !editaveis.is_empty()
                && editaveis
                    .iter()
                    .all(|f| f.estado == acervo::Estado::LevadaNoBalcao),
            rejeitaveis: rejeitaveis.len(),
            todas_rejeitadas: !rejeitaveis.is_empty() && rejeitaveis.iter().all(|f| f.rejeitada),
            negociaveis: marcadas
                .iter()
                .filter(|f| !self.ids_locais.contains(&f.id) && f.negociavel())
                .count(),
            nao_salvas: self.nao_salvas_marcadas().len(),
            excluiveis: marcadas
                .iter()
                .filter(|f| f.pode_excluir() && !self.subindo_agora.contains(&f.id))
                .count(),
        })
    }
}

/// Um clique de item que chama um método do `Detalhe`.
fn com(
    tela: &WeakEntity<Detalhe>,
    f: impl Fn(&mut Detalhe, &mut Window, &mut Context<Detalhe>) + 'static,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let tela = tela.clone();
    move |_ev, window, cx| {
        let _ = tela.update(cx, |tela, cx| f(tela, window, cx));
    }
}

/// Uma tecla à direita do item, como o kit desenha atalho.
fn com_tecla(rotulo: impl Into<gpui_kit::SharedString>, tecla: &str) -> PopupMenuItem {
    crate::estilo::item_com_atalho(rotulo, tecla)
}

/// 🗂️ O menu, nos grupos do menu da Revelação: a foto · o balcão · a
/// seleção · o que não tem volta. O que não serve às marcadas some.
pub(super) fn montar(
    menu: PopupMenu,
    dados: DadosDoMenu,
    tela: WeakEntity<Detalhe>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let varias = dados.marcadas > 1;
    let mut menu = menu.min_w(px(240.)).label(dados.titulo);

    // ── A foto ────────────────────────────────────────────────────────
    menu = menu.separator();
    if dados.revelavel {
        menu = menu.item(
            PopupMenuItem::new("Revelar")
                .icon(Icon::new(Icone::SlidersHorizontal))
                .on_click(com(&tela, |tela, _w, cx| tela.revelar_a_do_foco(cx))),
        );
    }
    // A nota num submenu: seis itens soltos alongariam o menu à toa.
    let nota = dados.nota;
    let para_a_nota = tela.clone();
    menu = menu.submenu_with_icon(
        Some(Icon::new(Icone::Sparkles)),
        "Nota",
        window,
        cx,
        move |sub, _w, _cx| {
            let mut sub = sub;
            for estrela in (1..=5u8).rev() {
                sub = sub.item(
                    com_tecla("★".repeat(estrela as usize), &estrela.to_string())
                        .checked(nota == Some(estrela))
                        .on_click(com(&para_a_nota, move |tela, _w, cx| {
                            tela.dar_nota(estrela, cx)
                        })),
                );
            }
            sub.separator().item(
                com_tecla("Sem nota", "0")
                    .disabled(nota.is_none())
                    .on_click(com(&para_a_nota, |tela, _w, cx| tela.dar_nota(0, cx))),
            )
        },
    );
    menu = menu.item(
        PopupMenuItem::new(if varias {
            format!("Exportar… ({})", dados.marcadas)
        } else {
            "Exportar…".to_string()
        })
        .icon(Icon::new(Icone::FolderInput))
        .on_click(com(&tela, |_tela, _w, cx| cx.emit(Pedido::Exportar))),
    );

    // ── O balcão ──────────────────────────────────────────────────────
    let mut balcao: Vec<PopupMenuItem> = Vec::new();
    if dados.editaveis > 0 {
        balcao.push(
            com_tecla(
                if dados.todas_levadas {
                    "Devolver à venda"
                } else {
                    "Levada no balcão"
                },
                "p",
            )
            .icon(Icon::new(if dados.todas_levadas {
                Icone::Store
            } else {
                Icone::ShoppingBag
            }))
            .on_click(com(&tela, |tela, _w, cx| tela.alternar_levada(cx))),
        );
    }
    if dados.rejeitaveis > 0 {
        balcao.push(
            com_tecla(
                if dados.todas_rejeitadas {
                    "Tirar a rejeição"
                } else {
                    "Rejeitar"
                },
                "x",
            )
            .icon(Icon::new(if dados.todas_rejeitadas {
                Icone::Undo2
            } else {
                Icone::EyeOff
            }))
            .on_click(com(&tela, |tela, _w, cx| tela.alternar_rejeicao(cx))),
        );
    }
    if dados.negociaveis > 0 {
        balcao.push(
            PopupMenuItem::new("Negociação…")
                .icon(Icon::new(Icone::Handshake))
                .on_click(com(&tela, move |tela, _w, cx| {
                    let ids = tela.marcadas();
                    tela.pedir_negociacao(ids, !varias, cx)
                })),
        );
    }
    if !balcao.is_empty() {
        menu = menu.separator();
        for item in balcao {
            menu = menu.item(item);
        }
    }

    // ── A seleção ─────────────────────────────────────────────────────
    menu = menu
        .separator()
        .item(
            com_tecla("Selecionar todas", "secondary-a")
                .icon(Icon::new(Icone::SquareCheck))
                .on_click(com(&tela, |tela, _w, cx| tela.selecionar_tudo(cx))),
        )
        .item(
            com_tecla("Limpar a seleção", "secondary-d")
                .icon(Icon::new(Icone::Square))
                .on_click(com(&tela, |tela, _w, cx| tela.limpar_selecao(cx))),
        );

    // ── O que não tem volta ───────────────────────────────────────────
    let mut desfazer: Vec<PopupMenuItem> = Vec::new();
    if dados.nao_salvas > 0 {
        desfazer.push(
            PopupMenuItem::new(if dados.nao_salvas > 1 {
                format!("Descartar a revelação de {} fotos", dados.nao_salvas)
            } else {
                "Descartar a revelação".to_string()
            })
            .icon(Icon::new(Icone::RotateCcw))
            .on_click(com(&tela, |tela, _w, cx| tela.descartar_das_marcadas(cx))),
        );
    }
    if dados.excluiveis > 0 {
        desfazer.push(
            com_tecla(
                if dados.excluiveis > 1 {
                    format!("Excluir {} fotos…", dados.excluiveis)
                } else {
                    "Excluir…".to_string()
                },
                "backspace",
            )
            .icon(Icon::new(Icone::Trash2))
            .on_click(com(&tela, |tela, window, cx| {
                tela.excluir_pela_tecla(window, cx)
            })),
        );
    }
    if !desfazer.is_empty() {
        menu = menu.separator();
        for item in desfazer {
            menu = menu.item(item);
        }
    }
    menu
}

impl Detalhe {
    /// 🧪 `sessao_tira marcar N` · `menu N` · `fechar` — o roteiro de
    /// depuração. ⚠️ O `ContextMenu` não abre por fora: o `menu` monta **o
    /// mesmo** menu, com a seleção que o botão direito deixaria, e o desenha
    /// sobre a miniatura.
    pub(crate) fn seguir_o_roteiro_do_menu(
        &mut self,
        gesto: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut partes = gesto.split_whitespace();
        let nome = partes.next().unwrap_or("");
        let posicao: Option<usize> = partes.next().and_then(|n| n.parse().ok());
        match (nome, posicao) {
            ("marcar", Some(posicao)) => self.clicar(
                posicao,
                Modificadores {
                    aditivo: true,
                    faixa: false,
                },
                cx,
            ),
            ("menu", Some(posicao)) => {
                // O elemento conta a partir do primeiro desenhado, com o
                // espaçador na frente se houver.
                let primeira = self.tira_desenhada.0;
                let elemento = posicao
                    .checked_sub(primeira)
                    .map(|k| k + usize::from(primeira > 0));
                let Some(item) = elemento.and_then(|k| self.rolagem_da_tira.bounds_for_item(k))
                else {
                    eprintln!("[roteiro] a miniatura {posicao} não está desenhada na tira");
                    return;
                };
                self.apontar_o_menu(posicao, cx);
                let Some(dados) = self.dados_do_menu() else {
                    return;
                };
                let esta = cx.entity().downgrade();
                let menu = PopupMenu::build(window, cx, move |menu, window, cx| {
                    montar(menu, dados, esta, window, cx)
                });
                let ponto = item.center() + self.rolagem_da_tira.offset();
                self.menu_do_roteiro = Some((menu, ponto));
                cx.notify();
            }
            ("fechar", _) => {
                self.menu_do_roteiro = None;
                cx.notify();
            }
            _ => eprintln!("[roteiro] gesto desconhecido da tira da sessão: '{gesto}'"),
        }
    }
}
