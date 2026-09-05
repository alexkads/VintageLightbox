//! As telas — cada arquivo é um `impl App` com um pedaço do quadro.
//!
//! A ordem em que o quadro as chama é a do `App::ui`: cabeçalho (topo), painel
//! (direita), central (barra + envio + grade), diálogos (por cima), avisos.

mod barra;
mod cabecalho;
mod dialogos;
mod envio;
mod grade;
mod painel;

use egui::{Color32, RichText, Ui};

use crate::tema;

/// Texto secundário — o `text-muted-foreground` do site.
pub fn suave(texto: impl Into<String>) -> RichText {
    RichText::new(texto).color(tema::N400).small()
}

pub fn titulo(texto: impl Into<String>) -> RichText {
    RichText::new(texto).heading()
}

/// Um "chip" pequeno — a etiqueta de situação.
pub fn etiqueta(ui: &mut Ui, texto: &str, fundo: Color32, frente: Color32) {
    egui::Frame::none()
        .fill(fundo)
        .rounding(tema::RAIO)
        .inner_margin(egui::Margin::symmetric(6.0, 1.0))
        .show(ui, |ui| {
            ui.label(RichText::new(texto).size(10.5).color(frente));
        });
}
