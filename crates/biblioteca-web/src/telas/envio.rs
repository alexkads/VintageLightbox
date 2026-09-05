//! A área de envio: o que a próxima leva vira, os parâmetros da compressão e
//! a fila em andamento.
//!
//! # A fronteira com o site, aqui
//!
//! O arquivo em si nunca passa por este wasm: escolher arquivo é um
//! `<input type=file>`, soltar é um evento do DOM, e a fila que comprime e
//! sobe é um Worker do site (`importacao/worker.ts`). O que esta tela faz é
//! **decidir** — estado, faixa, parâmetros — e **mostrar** o que a fila está
//! fazendo. O hospedeiro lê a leva daqui (`leva_json`) na hora de enfileirar,
//! e empurra o andamento para cá (`definir_importacao`).

use biblioteca_core::dinheiro;
use egui::{Color32, RichText};

use crate::app::{App, Comando, Pedido};
use crate::telas::suave;
use crate::tema;

impl App {
    pub(crate) fn ui_envio(&mut self, ui: &mut egui::Ui, comandos: &mut Vec<Comando>) {
        let fundo = if self.arrastando_arquivos {
            Color32::from_rgba_unmultiplied(0xfb, 0xbf, 0x24, 25)
        } else {
            tema::N900
        };
        let borda = if self.arrastando_arquivos {
            tema::AMBAR
        } else {
            tema::N800
        };

        egui::Frame::none()
            .fill(fundo)
            .stroke(egui::Stroke::new(1.5_f32, borda))
            .rounding(tema::RAIO)
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("O cliente decidiu:");
                    if ui
                        .radio(self.leva.estado == "levada_no_balcao", "Levadas no balcão")
                        .clicked()
                    {
                        self.leva.estado = "levada_no_balcao".into();
                    }
                    if ui.radio(self.leva.estado == "disponivel", "À venda").clicked() {
                        self.leva.estado = "disponivel".into();
                    }

                    ui.add_space(12.0);
                    ui.label(suave("Faixa:"));
                    let padrao = format!(
                        "Padrão ({} — {})",
                        self.estado.produto.nome,
                        dinheiro::formatar(self.estado.produto.preco)
                    );
                    let nome = if self.leva.produto_id.is_empty() {
                        padrao.clone()
                    } else {
                        self.estado.nome_da_faixa(&self.leva.produto_id)
                    };
                    egui::ComboBox::from_id_salt("faixa-da-leva")
                        .selected_text(nome)
                        .width(240.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.leva.produto_id, String::new(), padrao);
                            for f in &self.estado.faixas {
                                ui.selectable_value(
                                    &mut self.leva.produto_id,
                                    f.id.clone(),
                                    format!("{} — {}", f.nome, dinheiro::formatar(f.preco)),
                                );
                            }
                        });

                    ui.add_space(12.0);
                    if ui.button("Escolher fotos").clicked() {
                        comandos.push(Comando::Pedido(Pedido::EscolherArquivos));
                    }

                    let p = &self.leva.parametros;
                    let resumo = if p.ligada {
                        format!("{} px · q{} · {} por vez", p.lado_maximo, p.qualidade, p.simultaneos)
                    } else {
                        "Sem comprimir".to_string()
                    };
                    if ui
                        .selectable_label(self.ajustes_abertos, format!("Ajustes: {resumo}"))
                        .on_hover_text("Tamanho, qualidade e quantos sobem ao mesmo tempo")
                        .clicked()
                    {
                        self.ajustes_abertos = !self.ajustes_abertos;
                    }
                });

                let rotulo = if self.leva.estado == "levada_no_balcao" {
                    "levadas no balcão"
                } else {
                    "à venda"
                };
                ui.label(suave(if self.arrastando_arquivos {
                    format!("Solte para importar como {rotulo}.")
                } else {
                    format!("Ou arraste os originais (JPEG/PNG) para cá — entram como {rotulo}. A fila continua se você sair desta tela, e cada foto aparece na grade assim que chega.")
                }));

                if self.ajustes_abertos {
                    self.ui_ajustes(ui, comandos);
                }
                if !self.importacao.is_empty() {
                    self.ui_fila(ui, comandos);
                }
            });
    }

    fn ui_ajustes(&mut self, ui: &mut egui::Ui, comandos: &mut Vec<Comando>) {
        ui.add_space(6.0);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            let p = &mut self.leva.parametros;
            let mut mudou = false;
            mudou |= ui
                .checkbox(&mut p.ligada, "Comprimir no navegador antes de enviar")
                .changed();
            ui.label(suave("O arquivo é recodificado aqui, pelo mesmo motor da revelação. Desligado, sobe o original da câmera — e é isso que o cliente baixa ao comprar."));
            ui.horizontal_wrapped(|ui| {
                ui.add_enabled_ui(p.ligada, |ui| {
                    ui.label(suave("Lado maior (px)"));
                    mudou |= ui
                        .add(egui::DragValue::new(&mut p.lado_maximo).range(1200..=12000).speed(50))
                        .changed();
                    ui.label(suave("Qualidade"));
                    mudou |= ui
                        .add(egui::DragValue::new(&mut p.qualidade).range(60..=100))
                        .changed();
                });
                ui.label(suave("Envios ao mesmo tempo"));
                mudou |= ui
                    .add(egui::DragValue::new(&mut p.simultaneos).range(1..=6))
                    .changed();
            });
            ui.label(suave("Vale para as próximas levas; o que já está na fila mantém o que foi escolhido quando entrou."));
            if mudou {
                comandos.push(Comando::GuardarLeva);
            }
        });
    }

    fn ui_fila(&self, ui: &mut egui::Ui, comandos: &mut Vec<Comando>) {
        let total = self.importacao.len();
        let prontas = self
            .importacao
            .iter()
            .filter(|i| i.estado == "pronta")
            .count();
        let com_erro = self
            .importacao
            .iter()
            .filter(|i| i.estado == "erro")
            .count();
        let pendentes = total - prontas - com_erro;
        let poupados: u64 = self
            .importacao
            .iter()
            .filter(|i| i.estado == "pronta")
            .filter_map(|i| i.bytes.map(|b| i.bytes_originais.saturating_sub(b)))
            .sum();

        ui.add_space(8.0);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(if pendentes == 0 {
                        "Importação concluída"
                    } else {
                        "Importando…"
                    })
                    .strong(),
                );
                let mut resumo = format!("{prontas} de {total}");
                if com_erro > 0 {
                    resumo.push_str(&format!(" — {com_erro} com erro"));
                }
                if poupados > 0 {
                    resumo.push_str(&format!(" · {} a menos de subida", mb(poupados)));
                }
                ui.label(suave(resumo));
                if pendentes == 0 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button("✕")
                            .on_hover_text("Fechar o resumo")
                            .clicked()
                        {
                            comandos.push(Comando::Pedido(Pedido::Importacao {
                                acao: "limpar".into(),
                                id: None,
                            }));
                        }
                    });
                }
            });
            let terminadas = (prontas + com_erro) as f32 / total.max(1) as f32;
            ui.add(egui::ProgressBar::new(terminadas).desired_height(6.0));

            egui::ScrollArea::vertical()
                .max_height(180.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for item in &self.importacao {
                        ui.horizontal(|ui| {
                            let icone = match item.estado.as_str() {
                                "pronta" => RichText::new("✔").color(tema::VERDE),
                                "erro" => RichText::new("!").color(tema::VERMELHO),
                                "comprimindo" | "enviando" => RichText::new("◌").color(tema::N400),
                                _ => RichText::new(" "),
                            };
                            ui.label(icone);
                            ui.label(RichText::new(&item.nome_original).small());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if (item.estado == "erro" || item.estado == "aguardando")
                                        && ui
                                            .small_button("✕")
                                            .on_hover_text("Tirar da fila")
                                            .clicked()
                                    {
                                        comandos.push(Comando::Pedido(Pedido::Importacao {
                                            acao: "retirar".into(),
                                            id: Some(item.id.clone()),
                                        }));
                                    }
                                    if item.estado == "erro"
                                        && ui
                                            .small_button("↻")
                                            .on_hover_text("Tentar de novo")
                                            .clicked()
                                    {
                                        comandos.push(Comando::Pedido(Pedido::Importacao {
                                            acao: "tentar_de_novo".into(),
                                            id: Some(item.id.clone()),
                                        }));
                                    }
                                    let rotulo = match item.estado.as_str() {
                                        "aguardando" => "na fila".to_string(),
                                        "comprimindo" => "comprimindo…".to_string(),
                                        "enviando" => format!("{}%", item.progresso),
                                        "erro" => "falhou".to_string(),
                                        _ => match item.bytes {
                                            Some(b) if b < item.bytes_originais => {
                                                format!("{} → {}", mb(item.bytes_originais), mb(b))
                                            }
                                            Some(b) => mb(b),
                                            None => mb(item.bytes_originais),
                                        },
                                    };
                                    ui.label(suave(rotulo));
                                },
                            );
                        });
                        if let Some(e) = &item.erro {
                            ui.label(RichText::new(e).small().color(tema::VERMELHO));
                        } else if let Some(a) = &item.aviso {
                            ui.label(suave(a));
                        }
                    }
                });
        });
    }
}

fn mb(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
}
