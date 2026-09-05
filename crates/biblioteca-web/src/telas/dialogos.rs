//! Os diálogos: negociação, confirmação de apagar, recibo e link.
//!
//! Um `egui::Modal` por cima de tudo. Quem decide o conteúdo é o core
//! (`negociacao::montar` valida antes de ir ao servidor); quem grava é o site.

use biblioteca_core::{dinheiro, negociacao};
use egui::{Color32, RichText};

use crate::tema;

use crate::app::{Acao, App, Comando, Dialogo, NegociacaoJson, Pedido};
use crate::telas::{etiqueta, suave};

impl App {
    pub(crate) fn ui_dialogos(&mut self, ctx: &egui::Context, comandos: &mut Vec<Comando>) {
        let Some(dialogo) = &mut self.dialogo else {
            return;
        };
        let ocupado = self.pendentes > 0;
        let mut fechar = false;

        let resposta = egui::Modal::new(egui::Id::new("dialogo")).show(ctx, |ui| {
            ui.set_width(440.0);
            match dialogo {
                Dialogo::ConfirmarApagar { ids } => {
                    ui.heading(format!("Apagar {} foto(s)?", ids.len()));
                    ui.label(suave("Os arquivos delas somem junto. Não há como desfazer."));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.add_enabled(!ocupado, egui::Button::new(RichText::new("Apagar").color(Color32::WHITE)).fill(Color32::from_rgb(0xdc, 0x26, 0x26))).clicked() {
                            comandos.push(Comando::Acao(Acao::Remover { ids: ids.clone() }));
                            fechar = true;
                        }
                        if ui.button("Cancelar").clicked() {
                            fechar = true;
                        }
                    });
                }
                Dialogo::Link { url, aviso } => {
                    ui.heading("Link da galeria");
                    if let Some(a) = aviso {
                        ui.label(suave(a.as_str()));
                    } else {
                        ui.label(suave("Copiado. Entra sem senha, cria a conta no primeiro clique e vale 7 dias."));
                    }
                    ui.add_space(4.0);
                    ui.add(egui::TextEdit::singleline(url).desired_width(f32::INFINITY));
                    ui.horizontal(|ui| {
                        if ui.button("Copiar de novo").clicked() {
                            comandos.push(Comando::Pedido(Pedido::Copiar { texto: url.clone() }));
                        }
                        if ui.button("Fechar").clicked() {
                            fechar = true;
                        }
                    });
                }
                Dialogo::Recibo { arquivo, liberada_em, downloads, fotos_no_pedido, resultado, .. } => {
                    ui.heading("Recibo do MercadoPago");
                    ui.label(suave(format!(
                        "{arquivo}{}{} · {downloads} download(s)",
                        liberada_em.as_ref().map(|l| format!(" · liberada em {l}")).unwrap_or_default(),
                        if *fotos_no_pedido > 1 { format!(" · {fotos_no_pedido} fotos desta galeria no mesmo pedido") } else { String::new() }
                    )));
                    ui.add_space(6.0);
                    match resultado {
                        None => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(suave("Consultando o gateway…"));
                            });
                        }
                        Some(Ok(pagamentos)) => {
                            for p in pagamentos.iter() {
                                egui::Frame::group(ui.style()).show(ui, |ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(RichText::new(dinheiro::formatar_reais(p.valor)).strong());
                                        let (rotulo, cor) = aparencia_do_status(&p.status);
                                        etiqueta(ui, rotulo, cor, Color32::WHITE);
                                        ui.label(suave(&p.meio));
                                    });
                                    let mut detalhes = vec![p.quando.clone()];
                                    if let Some(l) = p.liquido {
                                        detalhes.push(format!("líquido {}", dinheiro::formatar_reais(l)));
                                    }
                                    if let Some(e) = &p.pagador {
                                        detalhes.push(e.clone());
                                    }
                                    detalhes.push(format!("#{}", p.id));
                                    ui.label(suave(detalhes.join(" · ")));
                                });
                            }
                        }
                        Some(Err((motivo, detalhe))) => {
                            let (titulo, texto) = explicacao_da_ausencia(motivo);
                            ui.label(RichText::new(titulo).strong());
                            ui.label(suave(texto));
                            if let Some(d) = detalhe {
                                ui.label(suave(d.as_str()));
                            }
                        }
                    }
                    ui.add_space(6.0);
                    if ui.button("Fechar").clicked() {
                        fechar = true;
                    }
                }
                Dialogo::Negociacao { titulo, ids, n, preco_texto, preco_da_faixa, existente, erro } => {
                    ui.heading(titulo.as_str());
                    ui.label(suave("O que aconteceu no balcão. Isto não muda o preço de venda: a compra online continua cobrando a faixa."));
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        for tipo in negociacao::Tipo::TODOS {
                            if ui.selectable_label(n.tipo == tipo, tipo.rotulo()).on_hover_text(tipo.dica()).clicked() {
                                n.tipo = tipo;
                                *erro = None;
                            }
                        }
                    });
                    ui.label(suave(n.tipo.dica()));
                    ui.add_space(4.0);

                    match n.tipo {
                        negociacao::Tipo::Cortesia => {}
                        negociacao::Tipo::Desconto | negociacao::Tipo::Parceiro | negociacao::Tipo::Outro => {
                            ui.horizontal(|ui| {
                                ui.label(suave(if n.tipo == negociacao::Tipo::Desconto { "Quanto foi cobrado (R$)" } else { "Quanto o cliente pagou (R$, opcional)" }));
                                if ui.add(egui::TextEdit::singleline(preco_texto).desired_width(90.0).hint_text(preco_da_faixa.map(dinheiro::formatar_campo).unwrap_or_default())).changed() {
                                    *erro = None;
                                }
                            });
                            if let Some(p) = preco_da_faixa {
                                ui.label(suave(format!("A faixa vale {}.", dinheiro::formatar(*p))));
                            }
                        }
                    }
                    if n.tipo == negociacao::Tipo::Parceiro {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(suave("Site"));
                            for parceiro in negociacao::PARCEIROS {
                                if ui.selectable_label(n.parceiro == parceiro, parceiro).clicked() {
                                    n.parceiro = parceiro.to_string();
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(suave("Cupom"));
                            ui.add(egui::TextEdit::singleline(&mut n.cupom).desired_width(140.0));
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.label(suave(if n.tipo == negociacao::Tipo::Outro { "O que foi combinado" } else { "Motivo (opcional)" }));
                        ui.add(egui::TextEdit::singleline(&mut n.motivo).desired_width(f32::INFINITY));
                    });
                    if let Some(e) = erro {
                        ui.label(RichText::new(e.as_str()).small().color(tema::VERMELHO));
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.add_enabled(!ocupado, egui::Button::new("Salvar")).clicked() {
                            // O preço digitado entra no modelo antes de montar.
                            n.preco = if n.tipo == negociacao::Tipo::Cortesia {
                                Some(0)
                            } else if preco_texto.trim().is_empty() {
                                None
                            } else {
                                match dinheiro::ler_campo(preco_texto) {
                                    Some(p) => Some(p),
                                    None => {
                                        *erro = Some("Valor inválido. Use o formato 19,90.".into());
                                        Some(-1)
                                    }
                                }
                            };
                            if n.preco != Some(-1) {
                                match negociacao::montar(n) {
                                    Ok(g) => {
                                        comandos.push(Comando::Acao(Acao::Negociacao {
                                            ids: ids.clone(),
                                            valor: Some(NegociacaoJson { preco_negociado: g.preco_negociado, observacao: g.observacao }),
                                        }));
                                        fechar = true;
                                    }
                                    Err(e) => *erro = Some(e),
                                }
                            }
                        }
                        if *existente && ui.add_enabled(!ocupado, egui::Button::new("Remover negociação")).clicked() {
                            comandos.push(Comando::Acao(Acao::Negociacao { ids: ids.clone(), valor: None }));
                            fechar = true;
                        }
                        if ui.button("Cancelar").clicked() {
                            fechar = true;
                        }
                    });
                }
            }
        });

        if fechar || resposta.should_close() {
            comandos.push(Comando::FecharDialogo);
        }
    }
}

/// A mesma lista de `/dashboard/pagamentos` (`recibo.ts`): dois vocabulários
/// para o mesmo status seriam duas telas discordando sobre o mesmo pagamento.
fn aparencia_do_status(status: &str) -> (&'static str, Color32) {
    match status {
        "approved" => ("Aprovado", Color32::from_rgb(0x05, 0x96, 0x69)),
        "pending" => ("Pendente", tema::N600),
        "in_process" => ("Em análise", tema::N600),
        "in_mediation" => ("Em mediação", Color32::from_rgb(0xdc, 0x26, 0x26)),
        "authorized" => ("Autorizado", tema::N600),
        "rejected" => ("Rejeitado", tema::N700),
        "cancelled" => ("Cancelado", tema::N700),
        "refunded" => ("Estornado", Color32::from_rgb(0xdc, 0x26, 0x26)),
        "charged_back" => ("Chargeback", Color32::from_rgb(0xdc, 0x26, 0x26)),
        _ => ("?", tema::N700),
    }
}

fn explicacao_da_ausencia(motivo: &str) -> (&'static str, &'static str) {
    match motivo {
        "sem_pedido" => (
            "Esta foto não tem pedido",
            "Ela consta como comprada, mas sem pedido ligado — é o caso da foto liberada à mão ou de uma negociação registrada no balcão. Não há cobrança no MercadoPago para mostrar.",
        ),
        "nada_no_gateway" => (
            "Nenhum pagamento no MercadoPago para este pedido",
            "O pedido existe do nosso lado e o gateway não tem cobrança com esta referência. Costuma ser pagamento feito por fora (PIX direto, dinheiro no balcão) ou pedido liberado à mão.",
        ),
        _ => (
            "O MercadoPago não respondeu",
            "A consulta ao gateway falhou agora. O pagamento não some por isso — tente de novo em instantes.",
        ),
    }
}
