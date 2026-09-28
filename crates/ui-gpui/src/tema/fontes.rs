//! A letra do template: as famílias do `/create` embutidas no binário.
//!
//! O `build.rs` embute `fontes/<slug>/*.ttf` da fonte do corpo e da dos
//! títulos (e só delas: as 26 juntas passariam de 10 MB), e [`registrar`] as
//! entrega ao GPUI antes da primeira janela. A pasta nasce de
//! `scripts/baixar-do-template.py --fonte <slug>`, que baixa os pesos 400,
//! 500, 600 e 700 do Google Fonts — a mesma origem do `@fontsource` do shadcn.
//!
//! Com `fonte = "sistema"`, nada é embutido e o texto usa a letra do sistema
//! operacional, como o app sempre fez.

use gpui_kit::{App, SharedString};

use super::preset::{FONTE_DO_SISTEMA, HERDAR};

/// O nome da família dentro do arquivo — o que o GPUI procura. `None` é a
/// letra do sistema.
pub fn familia(slug: &str) -> Option<&'static str> {
    Some(match slug {
        "inter" => "Inter",
        "noto-sans" => "Noto Sans",
        "nunito-sans" => "Nunito Sans",
        "figtree" => "Figtree",
        "roboto" => "Roboto",
        "raleway" => "Raleway",
        "dm-sans" => "DM Sans",
        "public-sans" => "Public Sans",
        "outfit" => "Outfit",
        "jetbrains-mono" => "JetBrains Mono",
        "geist" => "Geist",
        "geist-mono" => "Geist Mono",
        "lora" => "Lora",
        "merriweather" => "Merriweather",
        "playfair-display" => "Playfair Display",
        "noto-serif" => "Noto Serif",
        "roboto-slab" => "Roboto Slab",
        "oxanium" => "Oxanium",
        "manrope" => "Manrope",
        "space-grotesk" => "Space Grotesk",
        "montserrat" => "Montserrat",
        "ibm-plex-sans" => "IBM Plex Sans",
        "source-sans-3" => "Source Sans 3",
        "instrument-sans" => "Instrument Sans",
        "eb-garamond" => "EB Garamond",
        "instrument-serif" => "Instrument Serif",
        FONTE_DO_SISTEMA => return None,
        _ => return None,
    })
}

/// A família dos títulos (`--font-heading`): a do corpo quando o template diz
/// `inherit`. `None` é "não trocar" — o título herda a letra de quem o contém.
pub fn dos_titulos() -> Option<SharedString> {
    let template = super::template();
    match template.fonte_dos_titulos {
        HERDAR => None,
        slug => familia(slug).map(SharedString::from),
    }
}

/// Entrega ao GPUI as fontes que o `build.rs` embutiu. Chamar antes de
/// aplicar o tema: o `font.family` dele só acha a família depois disto.
pub fn registrar(cx: &mut App) {
    let arquivos: Vec<_> = crate::recursos::fontes()
        .map(std::borrow::Cow::Borrowed)
        .collect();
    if arquivos.is_empty() {
        return;
    }
    let quantos = arquivos.len();
    if let Err(erro) = cx.text_system().add_fonts(arquivos) {
        // A letra do sistema continua valendo; o app abre do mesmo jeito.
        eprintln!("🔤 [Template] as {quantos} fontes embutidas não carregaram: {erro:#}");
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::tema::preset::FONTES;

    #[test]
    fn toda_fonte_do_create_tem_familia() {
        for slug in FONTES {
            assert!(familia(slug).is_some(), "`{slug}` sem família");
        }
        assert_eq!(familia(FONTE_DO_SISTEMA), None);
    }
}
