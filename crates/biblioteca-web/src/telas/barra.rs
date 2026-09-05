//! A barra da grade: o recorte por situação com contagens, o zoom e o botão
//! de marcar as visíveis. E o painel central inteiro, que é a barra em cima e
//! a grade rolando embaixo.

use biblioteca_core::acervo::{Estado, Filtro};
use biblioteca_core::grade::{ZOOM_MAX, ZOOM_MIN};

use crate::app::{App, Comando};
use crate::telas::suave;

const FILTROS: [(Filtro, &str); 5] = [
    (Filtro::Todas, "Todas"),
    (Filtro::Situacao(Estado::LevadaNoBalcao), "Levadas"),
    (Filtro::Situacao(Estado::Disponivel), "À venda"),
    (Filtro::Situacao(Estado::Comprada), "Compradas"),
    (Filtro::Apagadas, "Apagadas"),
];

impl App {
    pub(crate) fn ui_central(&mut self, ctx: &egui::Context, comandos: &mut Vec<Comando>) {
        egui::CentralPanel::default()
            .frame(egui::Frame::none().inner_margin(egui::Margin::symmetric(16.0, 8.0)))
            .show(ctx, |ui| {
                self.ui_envio(ui, comandos);
                ui.add_space(10.0);

                if self.estado.fotos.is_empty() {
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| {
                        ui.label(suave("Nenhuma foto ainda. Escolha o estado acima e arraste os originais exportados do Lightroom — a marca d'água é aplicada aqui, na cópia que o cliente vê."));
                    });
                    return;
                }

                self.ui_barra(ui, comandos);
                ui.add_space(6.0);

                if self.acervo.total_visivel() == 0 {
                    ui.label(suave("Nenhuma foto neste filtro."));
                    return;
                }

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.ui_grade(ui, comandos);
                    });
            });
    }

    fn ui_barra(&mut self, ui: &mut egui::Ui, comandos: &mut Vec<Comando>) {
        let contagens = self.acervo.contagens();
        ui.horizontal_wrapped(|ui| {
            for (filtro, rotulo) in FILTROS {
                let n = contagens.de(filtro);
                let ativo = self.filtro == filtro;
                if filtro != Filtro::Todas && n == 0 && !ativo {
                    continue;
                }
                if ui
                    .selectable_label(ativo, format!("{rotulo}  {n}"))
                    .clicked()
                    && !ativo
                {
                    comandos.push(Comando::Filtrar(filtro));
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let total = self.acervo.total_visivel();
                if total > 0 {
                    let todas = (0..total).all(|i| self.selecao.tem(i));
                    let rotulo = format!(
                        "{} as {total} visíveis",
                        if todas { "Desmarcar" } else { "Selecionar" }
                    );
                    if ui.small_button(rotulo).clicked() {
                        comandos.push(Comando::AlternarTodas);
                    }
                }
                let mut zoom = self.zoom;
                if ui
                    .add(
                        egui::Slider::new(&mut zoom, ZOOM_MIN..=ZOOM_MAX)
                            .show_value(false)
                            .step_by(10.0),
                    )
                    .on_hover_text("Tamanho das miniaturas (Ctrl + roda do mouse também)")
                    .changed()
                {
                    comandos.push(Comando::Zoom(zoom));
                }
                ui.label(suave("Zoom"));
            });
        });
    }
}
