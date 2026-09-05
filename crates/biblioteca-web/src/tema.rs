//! O tema — o **mesmo visual do editor de revelação**, e não o do egui.
//!
//! O dono viu a biblioteca em egui ao lado do editor e disse: *"biblioteca-web
//! deve seguir o mesmo visual de revelacao-web"*. O editor é escuro, em
//! `neutral` do Tailwind, com âmbar nos sliders e na seleção, cantos de 6 px e
//! a Geist Sans do site. Tudo isso vira números aqui, e nada de aparência fica
//! espalhado pelas telas: quem quiser mudar a cara da biblioteca mexe num
//! arquivo.
//!
//! # A fonte vai embutida
//!
//! O egui rasteriza texto sozinho e precisa do arquivo da fonte; a Geist do
//! site é self-hosted pelo `next/font`, e o wasm não a alcança. Os dois pesos
//! (`Regular` e `Medium`) entram no binário por `include_bytes!` — ~180 KB
//! cada, o preço de o nome de uma foto ter a mesma letra nos dois lados.
//!
//! # Sempre escuro
//!
//! O editor é escuro em qualquer tema do site — é um palco para a foto. A
//! biblioteca segue a mesma decisão, e por isso `tema_escuro` do hospedeiro
//! deixou de mudar o visual: ela é a antessala do mesmo palco.

use egui::{Color32, FontData, FontDefinitions, FontFamily, Rounding, Stroke, Visuals};

/// A paleta `neutral` do Tailwind, que é a do editor.
pub const N950: Color32 = Color32::from_rgb(0x0a, 0x0a, 0x0a);
pub const N900: Color32 = Color32::from_rgb(0x17, 0x17, 0x17);
pub const N800: Color32 = Color32::from_rgb(0x26, 0x26, 0x26);
pub const N700: Color32 = Color32::from_rgb(0x40, 0x40, 0x40);
pub const N600: Color32 = Color32::from_rgb(0x52, 0x52, 0x52);
pub const N500: Color32 = Color32::from_rgb(0x73, 0x73, 0x73);
pub const N400: Color32 = Color32::from_rgb(0xa3, 0xa3, 0xa3);
pub const N300: Color32 = Color32::from_rgb(0xd4, 0xd4, 0xd4);
pub const N200: Color32 = Color32::from_rgb(0xe5, 0xe5, 0xe5);
pub const N100: Color32 = Color32::from_rgb(0xf5, 0xf5, 0xf5);
/// O âmbar dos sliders e da seleção (`amber-400` / `amber-300`).
pub const AMBAR: Color32 = Color32::from_rgb(0xfb, 0xbf, 0x24);
pub const AMBAR_CLARO: Color32 = Color32::from_rgb(0xfc, 0xd3, 0x4d);
/// Erro e ação destrutiva (`red-400`).
pub const VERMELHO: Color32 = Color32::from_rgb(0xf8, 0x71, 0x71);
/// Sucesso (`emerald-400`).
pub const VERDE: Color32 = Color32::from_rgb(0x34, 0xd3, 0x99);

/// `rounded-md`.
pub const RAIO: f32 = 6.0;
/// `rounded-lg` — o tile da foto.
pub const RAIO_DO_TILE: f32 = 8.0;

/// Aplica fonte, cores e espaçamento ao contexto. Uma vez, ao abrir.
pub fn aplicar(ctx: &egui::Context) {
    ctx.set_fonts(fontes());
    ctx.set_visuals(visuais());

    let mut estilo = (*ctx.style()).clone();
    use egui::{FontId, TextStyle};
    estilo.text_styles = [
        (TextStyle::Small, FontId::proportional(11.0)),
        (TextStyle::Body, FontId::proportional(13.0)),
        (TextStyle::Button, FontId::proportional(13.0)),
        (
            TextStyle::Heading,
            FontId::new(17.0, FontFamily::Name("geist-medium".into())),
        ),
        (TextStyle::Monospace, FontId::monospace(12.0)),
    ]
    .into();
    estilo.spacing.item_spacing = egui::vec2(8.0, 6.0);
    estilo.spacing.button_padding = egui::vec2(10.0, 5.0);
    estilo.spacing.interact_size.y = 26.0;
    estilo.spacing.slider_width = 120.0;
    ctx.set_style(estilo);
}

fn fontes() -> FontDefinitions {
    let mut fontes = FontDefinitions::default();
    fontes.font_data.insert(
        "geist".into(),
        FontData::from_static(include_bytes!("../assets/Geist-Regular.ttf")).into(),
    );
    fontes.font_data.insert(
        "geist-medium".into(),
        FontData::from_static(include_bytes!("../assets/Geist-Medium.ttf")).into(),
    );
    // A Geist na frente da lista: ela resolve o texto, e as fontes do egui
    // ficam para os símbolos que ela não tem (✔, setas, ícones).
    fontes
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "geist".into());
    fontes.families.insert(
        FontFamily::Name("geist-medium".into()),
        vec!["geist-medium".into(), "geist".into()],
    );
    fontes
}

fn visuais() -> Visuals {
    let mut v = Visuals::dark();
    v.panel_fill = N950;
    v.window_fill = N900;
    v.extreme_bg_color = N900;
    v.faint_bg_color = N900;
    v.code_bg_color = N800;
    v.window_stroke = Stroke::new(1.0_f32, N800);
    v.window_rounding = Rounding::same(RAIO);
    v.menu_rounding = Rounding::same(RAIO);
    v.override_text_color = Some(N200);
    v.hyperlink_color = AMBAR_CLARO;
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(0xfb, 0xbf, 0x24, 60);
    v.selection.stroke = Stroke::new(1.0_f32, AMBAR);
    v.warn_fg_color = AMBAR;
    v.error_fg_color = VERMELHO;

    let w = &mut v.widgets;
    // Repouso: o fundo dos campos e botões é neutral-800 sobre o 950/900.
    w.noninteractive.bg_fill = N900;
    w.noninteractive.weak_bg_fill = N900;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, N800);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, N400);
    w.noninteractive.rounding = Rounding::same(RAIO);

    w.inactive.bg_fill = N800;
    w.inactive.weak_bg_fill = N800;
    w.inactive.bg_stroke = Stroke::new(1.0_f32, N700);
    w.inactive.fg_stroke = Stroke::new(1.0_f32, N300);
    w.inactive.rounding = Rounding::same(RAIO);

    w.hovered.bg_fill = N700;
    w.hovered.weak_bg_fill = N700;
    w.hovered.bg_stroke = Stroke::new(1.0_f32, N600);
    w.hovered.fg_stroke = Stroke::new(1.0_f32, N100);
    w.hovered.rounding = Rounding::same(RAIO);

    w.active.bg_fill = N600;
    w.active.weak_bg_fill = N600;
    w.active.bg_stroke = Stroke::new(1.0_f32, N500);
    w.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    w.active.rounding = Rounding::same(RAIO);

    // O que está "ligado" (recorte ativo, radio marcado, slider): âmbar sobre
    // preto, como o `bg-amber-400 text-black` do editor.
    w.open.bg_fill = N800;
    w.open.weak_bg_fill = N800;
    w.open.bg_stroke = Stroke::new(1.0_f32, N700);
    w.open.fg_stroke = Stroke::new(1.0_f32, N200);
    w.open.rounding = Rounding::same(RAIO);
    v
}
