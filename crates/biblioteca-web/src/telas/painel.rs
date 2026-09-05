//! O painel ao lado da grade: o que a seleção é, e o que se faz com ela.
//!
//! Com uma foto, os controles dela (situação, faixa, negociação, preço de
//! venda, recibo, revelar, apagar). Com várias, as ações em lote. Sem seleção,
//! os atalhos que quem chega pela primeira vez não adivinha.

use biblioteca_core::acervo::permissoes;
use biblioteca_core::{dinheiro, negociacao, preco_de_venda};
use egui::{Color32, RichText};

use crate::tema;

use crate::app::{Acao, App, Comando, Dialogo, Pedido};
use crate::modelo::Foto;
use crate::telas::{etiqueta, suave};

impl App {
    pub(crate) fn ui_painel(&mut self, ctx: &egui::Context, comandos: &mut Vec<Comando>) {
        egui::SidePanel::right("painel")
            .resizable(false)
            .exact_width(300.0)
            .frame(egui::Frame::none().inner_margin(egui::Margin::symmetric(12.0, 10.0)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let selecionadas = self.ids_selecionados();
                        let foco = self.id_em_foco();
                        let unica = match selecionadas.len() {
                            1 => selecionadas.first().cloned(),
                            0 => foco,
                            _ => None,
                        };
                        match unica.and_then(|id| self.foto(&id).cloned()) {
                            Some(foto) => {
                                self.ui_uma_foto(ui, &foto, selecionadas.is_empty(), comandos)
                            }
                            None if selecionadas.is_empty() => ui_dicas(ui),
                            None => self.ui_lote(ui, &selecionadas, comandos),
                        }
                    });
            });
    }

    fn ui_uma_foto(
        &mut self,
        ui: &mut egui::Ui,
        f: &Foto,
        so_em_foco: bool,
        comandos: &mut Vec<Comando>,
    ) {
        let ocupado = self.pendentes > 0;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(suave(format!("{}.", f.ordem + 1)));
                ui.label(RichText::new(&f.arquivo).strong());
            });
            linha(
                ui,
                "Situação",
                &match &f.apagada_em {
                    Some(a) => format!("Apagada pela retenção em {a}"),
                    None => f.rotulo_do_estado().to_string(),
                },
            );
            linha(ui, "Tamanho", &format!("{} MB", f.tamanho_mb));
            linha(ui, "Downloads", &f.downloads.to_string());
            if let Some(r) = &f.revelada_em {
                linha(ui, "Revelada", r);
            }
            if let Some(e) = self.edicao_local(&f.id) {
                ui.label(
                    RichText::new(format!(
                        "Edição local de {} — ainda não salva na galeria",
                        e.atualizada_em
                    ))
                    .small()
                    .color(tema::AMBAR),
                );
            }

            if f.estado == "comprada" {
                if let Some(pedido) = &f.pedido_id {
                    if ui.small_button("Recibo do MercadoPago").clicked() {
                        comandos.push(Comando::Dialogo(Dialogo::Recibo {
                            arquivo: f.arquivo.clone(),
                            liberada_em: f.liberada_em.clone(),
                            downloads: f.downloads,
                            fotos_no_pedido: self.estado.contar_no_pedido(pedido),
                            resultado: None,
                        }));
                        comandos.push(Comando::Acao(Acao::Recibo {
                            pedido_id: pedido.clone(),
                        }));
                    }
                }
            }

            if ui.link(suave("Abrir a prévia ↗")).clicked() {
                comandos.push(Comando::Pedido(Pedido::AbrirUrl {
                    url: f.previa.clone(),
                }));
            }

            let editavel = f.editavel();
            let ids = vec![f.id.clone()];

            if editavel {
                ui.add_space(4.0);
                if ui
                    .add_enabled(!ocupado, egui::Button::new("Revelar"))
                    .clicked()
                {
                    comandos.push(Comando::Pedido(Pedido::Revelar {
                        foto_id: Some(f.id.clone()),
                    }));
                }
                ui.horizontal(|ui| {
                    let para_levada = f.estado == "disponivel";
                    let rotulo = if para_levada {
                        "Marcar como levada"
                    } else {
                        "Pôr à venda"
                    };
                    if ui
                        .add_enabled(!ocupado, egui::Button::new(rotulo))
                        .clicked()
                    {
                        comandos.push(Comando::Acao(Acao::Estado {
                            ids: ids.clone(),
                            estado: if para_levada {
                                "levada_no_balcao"
                            } else {
                                "disponivel"
                            }
                            .into(),
                        }));
                    }
                    if ui
                        .add_enabled(
                            !ocupado,
                            egui::Button::new(RichText::new("Apagar").color(tema::VERMELHO)),
                        )
                        .clicked()
                    {
                        comandos.push(Comando::Dialogo(Dialogo::ConfirmarApagar {
                            ids: ids.clone(),
                        }));
                    }
                });
            }

            // Faixa
            ui.add_space(4.0);
            if editavel {
                ui.label(suave("Quantidade de pessoas desta foto"));
                let atual = f.produto_id.clone().unwrap_or_default();
                let mut escolhido = atual.clone();
                let padrao = format!(
                    "Padrão da galeria ({})",
                    self.estado.nome_da_faixa(&self.estado.produto.id)
                );
                egui::ComboBox::from_id_salt(("faixa", &f.id))
                    .selected_text(if atual.is_empty() {
                        padrao.clone()
                    } else {
                        self.estado.nome_da_faixa(&atual)
                    })
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut escolhido, String::new(), padrao);
                        for fx in &self.estado.faixas {
                            ui.selectable_value(
                                &mut escolhido,
                                fx.id.clone(),
                                format!("{} — {}", fx.nome, dinheiro::formatar(fx.preco)),
                            );
                        }
                    });
                if escolhido != atual {
                    comandos.push(Comando::Acao(Acao::Produto {
                        ids: ids.clone(),
                        produto_id: (!escolhido.is_empty()).then_some(escolhido),
                    }));
                }
            } else {
                ui.label(suave(self.estado.nome_da_faixa(&f.produto_efetivo)))
                    .on_hover_text("A faixa pela qual esta foto foi paga");
            }

            // Negociação
            let existe = f.preco_negociado.is_some()
                || f.observacao.as_deref().is_some_and(|o| !o.is_empty());
            if editavel || existe {
                ui.add_space(4.0);
                let texto = if existe {
                    let e = negociacao::descrever(f.preco_negociado, f.observacao.as_deref());
                    match e.detalhe {
                        Some(d) => format!("{} — {d}", e.titulo),
                        None => e.titulo.clone(),
                    }
                } else {
                    "Negociação…".to_string()
                };
                if editavel {
                    let botao = if existe {
                        egui::Button::new(RichText::new(&texto).color(tema::AMBAR))
                            .fill(Color32::from_rgba_unmultiplied(0xfb, 0xbf, 0x24, 40))
                    } else {
                        egui::Button::new(suave(&texto)).frame(false)
                    };
                    if ui.add_enabled(!ocupado, botao).clicked() {
                        comandos.push(Comando::Dialogo(self.dialogo_de_negociacao(
                            "Negociação desta foto".into(),
                            ids.clone(),
                            Some(self.estado.preco_da_faixa(f)),
                            if existe {
                                negociacao::interpretar(f.preco_negociado, f.observacao.as_deref())
                            } else {
                                negociacao::Negociacao::default()
                            },
                            existe,
                        )));
                    }
                } else {
                    ui.label(suave(texto));
                }
            }

            // Preço de venda online
            ui.add_space(4.0);
            let preco_loja = self.estado.preco_online_da_faixa(f);
            if editavel {
                if self.painel.preco_de.as_deref() != Some(&f.id) {
                    self.painel.preco_de = Some(f.id.clone());
                    self.painel.preco_texto = f
                        .preco_de_venda
                        .map(dinheiro::formatar_campo)
                        .unwrap_or_default();
                    self.painel.preco_erro = None;
                }
                ui.label(suave("Preço de venda online"));
                let mut aplicar = false;
                ui.horizontal(|ui| {
                    ui.label(suave("R$"));
                    let campo = ui.add_enabled(
                        !ocupado,
                        egui::TextEdit::singleline(&mut self.painel.preco_texto)
                            .desired_width(80.0)
                            .hint_text(dinheiro::formatar_campo(preco_loja)),
                    );
                    if campo.changed() {
                        self.painel.preco_erro = None;
                    }
                    if campo.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        aplicar = true;
                    }
                    if ui
                        .add_enabled(!ocupado, egui::Button::new("Aplicar"))
                        .clicked()
                    {
                        aplicar = true;
                    }
                });
                if aplicar {
                    match preco_de_venda::ler(&self.painel.preco_texto) {
                        Ok(preco) => comandos.push(Comando::Acao(Acao::PrecoDeVenda {
                            ids: ids.clone(),
                            preco: Some(preco),
                        })),
                        Err(e) => self.painel.preco_erro = Some(e),
                    }
                }
                match f.preco_de_venda {
                    Some(p) => {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(suave(format!(
                                "Fixado em {}; a faixa cobraria {}.",
                                dinheiro::formatar(p),
                                dinheiro::formatar(preco_loja)
                            )));
                            if ui.link(suave("Voltar à faixa")).clicked() {
                                comandos.push(Comando::Acao(Acao::PrecoDeVenda {
                                    ids: ids.clone(),
                                    preco: None,
                                }));
                            }
                        });
                    }
                    None => {
                        ui.label(suave(format!(
                            "Sem valor fixado: o cliente paga {} (preço da loja).",
                            dinheiro::formatar(preco_loja)
                        )));
                    }
                }
                if let Some(e) = &self.painel.preco_erro {
                    ui.label(RichText::new(e).small().color(tema::VERMELHO));
                }
            } else if let Some(p) = f.preco_de_venda {
                ui.label(suave(format!("Vendida por {}", dinheiro::formatar(p))));
            }

            if so_em_foco {
                ui.add_space(6.0);
                ui.label(suave("Só em foco. Clique ou espaço para selecionar."));
            }
        });
    }

    fn ui_lote(&mut self, ui: &mut egui::Ui, selecionadas: &[String], comandos: &mut Vec<Comando>) {
        let ocupado = self.pendentes > 0;
        let escolhidas: Vec<&biblioteca_core::acervo::Foto> = self
            .selecao
            .marcadas()
            .filter_map(|n| self.acervo.visivel(n))
            .collect();
        let p = permissoes(escolhidas.into_iter());
        let n = selecionadas.len();
        let m = p.podem_mudar();
        let ids = p.ids_que_mudam.clone();

        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{n} selecionadas")).strong());
                if ocupado {
                    ui.spinner();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.link(suave("limpar")).clicked() {
                        comandos.push(Comando::LimparSelecao);
                    }
                });
            });
            if p.tem_intocaveis() {
                ui.label(suave(if m == 0 {
                    "Nenhuma delas pode mudar: comprada e apagada ficam como estão.".to_string()
                } else {
                    format!("{m} podem mudar; comprada e apagada ficam como estão.")
                }));
            }
            if m == 0 {
                return;
            }

            if ui
                .add_enabled(!ocupado, egui::Button::new("Marcar como levadas"))
                .clicked()
            {
                comandos.push(Comando::Acao(Acao::Estado {
                    ids: ids.clone(),
                    estado: "levada_no_balcao".into(),
                }));
            }
            if ui
                .add_enabled(!ocupado, egui::Button::new("Pôr à venda"))
                .clicked()
            {
                comandos.push(Comando::Acao(Acao::Estado {
                    ids: ids.clone(),
                    estado: "disponivel".into(),
                }));
            }

            ui.label(suave("Faixa"));
            let mut escolha = "__nada".to_string();
            let padrao = format!(
                "Padrão da galeria ({})",
                self.estado.nome_da_faixa(&self.estado.produto.id)
            );
            egui::ComboBox::from_id_salt("faixa-lote")
                .selected_text("mudar para…")
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut escolha, String::new(), padrao);
                    for fx in &self.estado.faixas {
                        ui.selectable_value(
                            &mut escolha,
                            fx.id.clone(),
                            format!("{} — {}", fx.nome, dinheiro::formatar(fx.preco)),
                        );
                    }
                });
            if escolha != "__nada" {
                comandos.push(Comando::Acao(Acao::Produto {
                    ids: ids.clone(),
                    produto_id: (!escolha.is_empty()).then_some(escolha),
                }));
            }

            if ui
                .add_enabled(!ocupado, egui::Button::new("Negociação…"))
                .clicked()
            {
                let precos: std::collections::BTreeSet<i64> = ids
                    .iter()
                    .filter_map(|id| self.foto(id))
                    .map(|f| self.estado.preco_da_faixa(f))
                    .collect();
                let existente = ids.iter().filter_map(|id| self.foto(id)).any(|f| {
                    f.preco_negociado.is_some()
                        || f.observacao.as_deref().is_some_and(|o| !o.is_empty())
                });
                comandos.push(Comando::Dialogo(self.dialogo_de_negociacao(
                    format!("Negociação de {m} foto{}", if m > 1 { "s" } else { "" }),
                    ids.clone(),
                    if precos.len() == 1 {
                        precos.iter().next().copied()
                    } else {
                        None
                    },
                    negociacao::Negociacao::default(),
                    existente,
                )));
            }

            // Preço de venda em lote
            ui.add_space(4.0);
            ui.label(suave("Preço de venda online"));
            let fixadas = ids
                .iter()
                .filter_map(|id| self.foto(id))
                .filter(|f| f.preco_de_venda.is_some())
                .count();
            let mut aplicar = false;
            ui.horizontal(|ui| {
                ui.label(suave("R$"));
                let campo = ui.add_enabled(
                    !ocupado,
                    egui::TextEdit::singleline(&mut self.painel.lote_preco_texto)
                        .desired_width(80.0)
                        .hint_text("19,90"),
                );
                if campo.changed() {
                    self.painel.lote_preco_erro = None;
                }
                if campo.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    aplicar = true;
                }
                if ui
                    .add_enabled(!ocupado, egui::Button::new("Aplicar"))
                    .clicked()
                {
                    aplicar = true;
                }
            });
            if aplicar {
                match preco_de_venda::ler(&self.painel.lote_preco_texto) {
                    Ok(preco) => comandos.push(Comando::Acao(Acao::PrecoDeVenda {
                        ids: ids.clone(),
                        preco: Some(preco),
                    })),
                    Err(e) => self.painel.lote_preco_erro = Some(e),
                }
            }
            if let Some(e) = &self.painel.lote_preco_erro {
                ui.label(RichText::new(e).small().color(tema::VERMELHO));
            }
            if fixadas > 0 {
                ui.horizontal_wrapped(|ui| {
                    ui.label(suave(format!("{fixadas} com valor fixado.")));
                    if ui.link(suave("Voltar todas à faixa")).clicked() {
                        comandos.push(Comando::Acao(Acao::PrecoDeVenda {
                            ids: ids.clone(),
                            preco: None,
                        }));
                    }
                });
            }

            ui.add_space(6.0);
            if ui
                .add_enabled(
                    !ocupado,
                    egui::Button::new(RichText::new("Apagar").color(tema::VERMELHO)).frame(false),
                )
                .clicked()
            {
                comandos.push(Comando::Dialogo(Dialogo::ConfirmarApagar {
                    ids: ids.clone(),
                }));
            }
        });
    }

    pub(crate) fn dialogo_de_negociacao(
        &self,
        titulo: String,
        ids: Vec<String>,
        preco_da_faixa: Option<i64>,
        n: negociacao::Negociacao,
        existente: bool,
    ) -> Dialogo {
        Dialogo::Negociacao {
            titulo,
            preco_texto: n.preco.map(dinheiro::formatar_campo).unwrap_or_default(),
            ids,
            n,
            preco_da_faixa,
            existente,
            erro: None,
        }
    }
}

fn linha(ui: &mut egui::Ui, rotulo: &str, valor: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(suave(rotulo));
        ui.label(RichText::new(valor).small());
    });
}

fn ui_dicas(ui: &mut egui::Ui) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(suave(
            "Clique numa foto para ver e mudar o que está marcado nela.",
        ));
        ui.add_space(4.0);
        ui.label(suave(
            "Shift + clique estende a seleção; Ctrl + clique acrescenta.",
        ));
        ui.label(suave(
            "Arraste sobre a grade para marcar várias de uma vez.",
        ));
        ui.label(suave("Duplo clique abre a prévia em tamanho maior."));
        ui.label(suave("Ctrl + roda do mouse muda o tamanho das miniaturas."));
        ui.add_space(4.0);
        etiqueta(
            ui,
            "setas · espaço · Enter · Ctrl+A · Esc",
            tema::N800,
            tema::N300,
        );
    });
}
