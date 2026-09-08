//! As cores da grade — e só as da grade.
//!
//! O wasm não desenha texto nem controle: o que ele pinta é o fundo do tile
//! enquanto a miniatura não chega, o anel de seleção, o anel do foco, a
//! caixinha de marcar, o véu da foto apagada e o laço do arrasto. Tudo o mais
//! é DOM do site, com os tokens do site.
//!
//! A grade **segue o tema do site** (claro ou escuro), como o resto do painel
//! — é o `bg-muted`/`border` de cada tema. O âmbar da seleção é o mesmo nos
//! dois: é a marca do editor de revelação (`amber-400`), e é o que liga as
//! duas telas.

use egui::Color32;

/// `amber-400` — a seleção, nos dois temas.
pub const AMBAR: Color32 = Color32::from_rgb(0xfb, 0xbf, 0x24);

#[derive(Debug, Clone, Copy)]
pub struct Cores {
    /// O que se vê antes de a miniatura chegar, e atrás de uma que não veio.
    pub fundo_do_tile: Color32,
    /// O anel da foto em foco (o cursor do teclado).
    pub foco: Color32,
    /// A caixinha de marcar em repouso (só no hover).
    pub caixa: Color32,
    pub borda_da_caixa: Color32,
    /// O véu sobre a foto apagada pela retenção.
    pub veu: Color32,
    /// O fundo do canvas — o mesmo do site, para o canvas sumir na página.
    pub fundo: Color32,
}

impl Cores {
    /// `neutral-100` de fundo de tile sobre o branco do site.
    pub fn claro() -> Self {
        Self {
            fundo_do_tile: Color32::from_rgb(0xe5, 0xe5, 0xe5),
            foco: Color32::from_rgb(0x0a, 0x0a, 0x0a),
            caixa: Color32::from_rgba_unmultiplied(0xff, 0xff, 0xff, 200),
            borda_da_caixa: Color32::from_rgb(0x73, 0x73, 0x73),
            veu: Color32::from_rgba_unmultiplied(0xff, 0xff, 0xff, 140),
            fundo: Color32::WHITE,
        }
    }

    /// `neutral-800` de fundo de tile sobre o `neutral-950` do site.
    pub fn escuro() -> Self {
        Self {
            fundo_do_tile: Color32::from_rgb(0x26, 0x26, 0x26),
            foco: Color32::from_rgb(0xf5, 0xf5, 0xf5),
            caixa: Color32::from_rgba_unmultiplied(0, 0, 0, 110),
            borda_da_caixa: Color32::WHITE,
            veu: Color32::from_rgba_unmultiplied(0x0a, 0x0a, 0x0a, 150),
            fundo: Color32::from_rgb(0x0a, 0x0a, 0x0a),
        }
    }

    pub fn do_tema(escuro: bool) -> Self {
        if escuro {
            Self::escuro()
        } else {
            Self::claro()
        }
    }
}
