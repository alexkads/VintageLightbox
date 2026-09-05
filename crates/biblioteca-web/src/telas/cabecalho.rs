//! O cabeçalho: quem é, o que tem, até quando, e os botões de sair.
//!
//! O mesmo `<header>` da página em React, com o texto que o dono pediu em
//! 2026-09-02: uma linha por bloco, sem explicar o fluxo a quem já o sabe.

use biblioteca_core::dinheiro;
use egui::RichText;

use crate::tema;

use crate::app::{Acao, App, Comando, Pedido};
use crate::telas::{suave, titulo};

impl App {
    pub(crate) fn ui_cabecalho(&mut self, ctx: &egui::Context, comandos: &mut Vec<Comando>) {
        egui::TopBottomPanel::top("cabecalho")
            .frame(egui::Frame::none().inner_margin(egui::Margin::symmetric(16.0, 10.0)))
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui.link(suave("← Sessões fotográficas")).clicked() {
                        comandos.push(Comando::Pedido(Pedido::AbrirUrl {
                            url: "/dashboard/sessoes-fotograficas".into(),
                        }));
                    }
                });

                ui.vertical(|ui| {
                    ui.vertical(|ui| {
                        ui.label(titulo(&self.estado.galeria.titulo));
                        let mut contato = self
                            .estado
                            .galeria
                            .email
                            .clone()
                            .unwrap_or_else(|| "sem e-mail".into());
                        if let Some(w) = &self.estado.galeria.whatsapp {
                            contato.push_str(&format!(" · {w}"));
                        }
                        ui.horizontal(|ui| {
                            ui.label(suave(contato));
                            if self.estado.galeria.cliente_ja_abriu {
                                ui.label(
                                    RichText::new("· cliente já abriu")
                                        .small()
                                        .color(tema::VERDE),
                                );
                            }
                        });
                    });

                    // Os botões numa linha própria, da esquerda para a direita: o
                    // alinhamento à direita dentro de uma linha que quebra sobrepõe
                    // botão com título em tela estreita.
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        self.ui_botoes_do_cabecalho(ui, comandos);
                    });
                });

                ui.add_space(6.0);
                self.ui_dados(ui);
                ui.add_space(4.0);
                self.ui_ultimo_aviso(ui, comandos);
            });
    }

    fn ui_botoes_do_cabecalho(&mut self, ui: &mut egui::Ui, comandos: &mut Vec<Comando>) {
        let ocupado = self.pendentes > 0;
        let email = self.estado.galeria.email.clone();

        // Avisar cliente — só com e-mail; sem ele a frase explica.
        match &email {
            Some(email) => {
                let rotulo = if self.estado.ja_avisado {
                    format!("Reenviar \"fotos prontas\" para {email}")
                } else {
                    format!("Avisar {email}: fotos prontas")
                };
                if ui
                    .add_enabled(!ocupado, egui::Button::new(rotulo))
                    .on_hover_text(
                        "O e-mail com os prazos e um link que entra sem senha por 7 dias",
                    )
                    .clicked()
                {
                    comandos.push(Comando::Acao(Acao::Avisar));
                }
            }
            None => {
                ui.label(suave(
                    "Só WhatsApp: sem e-mail não há para onde mandar o aviso.",
                ));
            }
        }

        // Copiar link — o do backend (entra sem senha), ou o endereço cru.
        let rotulo_link = if email.is_some() {
            "Copiar link"
        } else {
            "Copiar endereço"
        };
        if ui
            .add_enabled(!ocupado, egui::Button::new(rotulo_link))
            .on_hover_text(if email.is_some() {
                "Link que entra sem senha, válido por 7 dias — o mesmo do e-mail"
            } else {
                "Sem e-mail não há link sem senha — este exige conta com o WhatsApp da galeria"
            })
            .clicked()
        {
            if email.is_some() {
                comandos.push(Comando::Acao(Acao::Link));
            } else {
                let url = format!("/meus-ensaios/{}", self.estado.galeria.id);
                comandos.push(Comando::Pedido(Pedido::Copiar { texto: url.clone() }));
                comandos.push(Comando::Dialogo(crate::app::Dialogo::Link {
                    url,
                    aviso: Some(
                        "Esta galeria só tem WhatsApp: o link pede login e só abre para uma conta que já tenha esse número no cadastro."
                            .into(),
                    ),
                }));
            }
        }

        // Revelar fotos — entra no modo sem escolher foto.
        if ui
            .add_enabled(!ocupado, egui::Button::new("Revelar fotos"))
            .on_hover_text("Revelar no navegador: exposição, cor, detalhe e enquadramento")
            .clicked()
        {
            comandos.push(Comando::Pedido(Pedido::Revelar { foto_id: None }));
        }

        // Estúdio — salva ao trocar, sem botão.
        let atual = self.estudio_escolhido.clone();
        let nome_atual = if atual.is_empty() {
            "Estúdio não informado".to_string()
        } else {
            self.estado
                .estudios
                .iter()
                .find(|e| e.id == atual)
                .map_or_else(
                    || {
                        format!(
                            "Estúdio fora do cadastro ({}…)",
                            &atual[..atual.len().min(8)]
                        )
                    },
                    |e| format!("{} — {}", e.nome, e.cidade),
                )
        };
        let mut escolhido = atual.clone();
        egui::ComboBox::from_id_salt("estudio")
            .selected_text(nome_atual)
            .width(220.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut escolhido, String::new(), "Estúdio não informado");
                for e in &self.estado.estudios {
                    ui.selectable_value(
                        &mut escolhido,
                        e.id.clone(),
                        format!("{} — {}", e.nome, e.cidade),
                    );
                }
            });
        if escolhido != atual {
            self.estudio_escolhido = escolhido.clone();
            comandos.push(Comando::Acao(Acao::Estudio {
                estudio_id: (!escolhido.is_empty()).then_some(escolhido),
            }));
        }
    }

    fn ui_dados(&self, ui: &mut egui::Ui) {
        let c = self.acervo.contagens();
        let g = &self.estado.galeria;
        ui.horizontal_wrapped(|ui| {
            dado(
                ui,
                "FOTOS",
                &format!(
                    "{} levadas · {} à venda · {} compradas{}",
                    c.levadas,
                    c.a_venda,
                    c.compradas,
                    if c.apagadas > 0 {
                        format!(" · {} apagadas", c.apagadas)
                    } else {
                        String::new()
                    }
                ),
            );
            let mut preco = format!(
                "{} ({})",
                dinheiro::formatar(self.estado.produto.preco),
                self.estado.produto.nome
            );
            let em_uso = self
                .estado
                .fotos
                .iter()
                .map(|f| f.produto_efetivo.as_str())
                .collect::<std::collections::BTreeSet<_>>()
                .len();
            if em_uso > 1 {
                preco.push_str(&format!(" · sessão mista, {em_uso} faixas"));
            }
            dado(ui, "PREÇO PADRÃO", &preco);
            if let Some(v) = &g.vence_venda {
                dado(ui, "À VENDA ATÉ", v);
            }
            if let Some(v) = &g.vence_download {
                dado(ui, "DOWNLOAD ATÉ", v);
            }
            if let Some(v) = &g.prorrogada_ate {
                dado(ui, "ADIADA ATÉ", v);
            }
            if let Some(v) = &g.expira_em {
                dado(ui, "GALERIA ATÉ", v);
            }
            dado(
                ui,
                "CRIADA",
                &format!("{} por {}", g.criada_em, g.criada_por),
            );
        });
    }

    fn ui_ultimo_aviso(&self, ui: &mut egui::Ui, comandos: &mut Vec<Comando>) {
        ui.horizontal_wrapped(|ui| {
            match self.estado.avisos.first() {
                Some(a) => {
                    ui.label(suave(format!(
                        "Último aviso: {} em {} para {} · {}",
                        a.tipo, a.enviado_em, a.destino, a.situacao
                    )));
                    if self.estado.avisos.len() > 1 {
                        ui.collapsing(
                            suave(format!("todos os {} avisos", self.estado.avisos.len())),
                            |ui| {
                                for a in &self.estado.avisos {
                                    ui.label(suave(format!(
                                        "{} · {} para {} · {}",
                                        a.tipo, a.enviado_em, a.destino, a.situacao
                                    )));
                                }
                            },
                        );
                    }
                }
                None if self.estado.galeria.email.is_some() => {
                    ui.label(suave("Nenhum aviso enviado ainda. O e-mail “fotos prontas” leva os prazos e um link que entra sem senha por 7 dias; os avisos de vencimento saem sozinhos."));
                }
                None => {
                    ui.label(suave("Só WhatsApp: não há para onde mandar o e-mail. Copie o link e mande pela conversa; a retenção trata a galeria como “não lida”."));
                }
            }
            if ui.link(suave("· política de retenção")).clicked() {
                comandos.push(Comando::Pedido(Pedido::AbrirUrl {
                    url: "/dashboard/sessoes-fotograficas/configuracoes".into(),
                }));
            }
        });
    }
}

fn dado(ui: &mut egui::Ui, rotulo: &str, valor: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(rotulo).size(10.0).color(tema::N400));
        ui.label(RichText::new(valor).size(13.0));
        ui.add_space(8.0);
    });
}
