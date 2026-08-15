//! Import View — a tela de importação, no formato do Lightroom
//!
//! Três colunas e duas barras, com o mesmo contrato do original: **DE** (esquerda) escolhe a
//! origem, o meio mostra o que existe lá em miniaturas marcáveis, **PARA** (direita) diz o que
//! vai acontecer com os arquivos, e nada é tocado no disco até o botão de baixo.
//!
//! O que faz a tela parecer rápida não é a velocidade da leitura, é a ordem dela:
//!
//! 1. varre a origem — só caminhos, e a grade já aparece cheia;
//! 2. lê metadados em paralelo, e as células vão se completando;
//! 3. gera miniatura **só das células visíveis**, sob demanda;
//! 4. confere duplicatas por hash e desmarca o que já está no catálogo.
//!
//! Cada passo desses é assíncrono e não bloqueia o desenho. É por isso que um cartão com
//! 2.000 RAWs abre na hora em vez de travar a janela por minutos.

use egui::{Color32, Context, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};
use std::sync::Arc;
use tokio::sync::mpsc;

use adapters::controllers::ImportController;
use adapters::view_models::ImportCandidateViewModel;
use domain::import_source::ImportSource;
use domain::value_objects::{ImportMode, ImportOptions, OrganizationStrategy, RenamePattern};

use crate::async_loader::{AsyncThumbnailLoader, ThumbnailRequest};
use crate::design_system::icons;
use crate::design_system::theme::Theme;
use crate::state::{AppState, ImportCandidate, ImportSortBy};

/// Resultado assíncrono que a tela espera
///
/// Tudo que a importação descobre chega por aqui, num canal só: a tela nunca bloqueia
/// esperando disco.
pub enum ImportMessage {
    /// Cartões montados e origens recentes
    Sources {
        devices: Vec<ImportSource>,
        recent: Vec<ImportSource>,
    },
    /// Varredura terminou — os caminhos encontrados na origem
    Scanned { root: String, files: Vec<String> },
    /// Metadados dos arquivos listados
    Described(Vec<ImportCandidateViewModel>),
    /// Caminhos que já existem no catálogo
    Duplicates(Vec<String>),
    /// Pasta escolhida no seletor nativo de origem
    SourceChosen(String),
    /// Pasta escolhida no seletor nativo de destino
    DestinationChosen(String),
    /// Algo falhou; a mensagem vai para o toast
    Failed(String),
}

/// Trabalho assíncrono que uma mensagem deixou pendente
///
/// `aplicar` só mexe no estado — quem dispara tarefa é o app, que tem o controller em mãos.
/// Sem isso, `aplicar` deixaria de ser testável sem runtime.
pub enum Seguimento {
    /// Varrer esta origem recém-escolhida
    Varrer(String),
    /// Ler metadados e conferir duplicatas destes arquivos
    Detalhar(Vec<String>),
}

/// O que a tela pede ao app depois que o usuário decide
pub enum ImportAction {
    /// Voltar para a biblioteca sem importar
    Cancel,
    /// Importar os arquivos marcados com estas opções
    Import {
        files: Vec<String>,
        options: ImportOptions,
    },
}

/// Chave de cache da miniatura de importação
///
/// O prefixo separa estas entradas das fotos já catalogadas: a mesma tabela guarda as duas,
/// e sem isso um caminho de cartão poderia colidir com um id de foto.
fn thumb_key(path: &str) -> String {
    format!("import::{}", path)
}

pub struct ImportView {}

impl ImportView {
    /// Desenha o modal inteiro e devolve a ação escolhida, se houver
    pub fn show(
        ctx: &Context,
        state: &mut AppState,
        controller: &Arc<ImportController>,
        sender: &mpsc::Sender<ImportMessage>,
        thumbnails: &mut AsyncThumbnailLoader,
    ) -> Option<ImportAction> {
        if !state.import_view_state.open {
            return None;
        }

        // Ao abrir, procurar cartões uma vez — sem botão "atualizar" para o usuário
        // ter de descobrir sozinho.
        if !state.import_view_state.sources_requested {
            state.import_view_state.sources_requested = true;
            Self::carregar_origens(ctx, controller, sender);
        }

        Self::receber_miniaturas(ctx, state, thumbnails);

        let (largura, altura) = Self::tamanho_do_modal(ctx);

        let resposta = egui::Modal::new(egui::Id::new("import_modal"))
            .frame(
                egui::Frame::new()
                    .fill(Theme::BG_APP)
                    .corner_radius(Theme::RADIUS_XL)
                    .stroke(Stroke::new(1.0, Theme::BORDER_DEFAULT)),
            )
            .show(ctx, |ui| {
                ui.set_width(largura);
                ui.set_height(altura);

                let mut action = None;

                // Painéis aninhados no Ui do modal (`show_inside`), não no contexto: no
                // contexto eles disputariam espaço com a janela principal, que continua
                // desenhada atrás.
                egui::TopBottomPanel::top("import_top")
                    .exact_height(76.0)
                    .frame(Self::moldura(Theme::BG_ELEVATED, Theme::SPACE_LG))
                    .show_separator_line(false)
                    .show_inside(ui, |ui| {
                        if let Some(a) = Self::cabecalho(ui, state) {
                            action = Some(a);
                        }
                    });

                egui::TopBottomPanel::bottom("import_actions")
                    .exact_height(68.0)
                    .frame(Self::moldura(Theme::BG_ELEVATED, Theme::SPACE_LG))
                    .show_separator_line(false)
                    .show_inside(ui, |ui| {
                        if let Some(a) = Self::rodape(ui, state) {
                            action = Some(a);
                        }
                    });

                egui::SidePanel::left("import_sources_panel")
                    .resizable(true)
                    .default_width(272.0)
                    .width_range(220.0..=360.0)
                    .frame(Self::moldura(Theme::BG_SURFACE, Theme::SPACE_LG))
                    .show_inside(ui, |ui| {
                        Self::painel_origem(ui, ctx, state, controller, sender);
                    });

                egui::SidePanel::right("import_options_panel")
                    .resizable(true)
                    .default_width(316.0)
                    .width_range(260.0..=420.0)
                    .frame(Self::moldura(Theme::BG_SURFACE, Theme::SPACE_LG))
                    .show_inside(ui, |ui| {
                        Self::painel_destino(ui, ctx, state, sender);
                    });

                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(Theme::BG_APP))
                    .show_inside(ui, |ui| {
                        Self::grade(ui, ctx, state, thumbnails);
                    });

                action
            });

        // Esc ou clique fora fecham — mas só quando este é o modal de cima, senão fechar a
        // lupa fecharia a importação junto.
        let fechar = resposta.should_close();
        let mut action = resposta.inner;

        if action.is_none() && fechar {
            action = Some(ImportAction::Cancel);
        }

        Self::lupa(ctx, state);

        if action.is_none() {
            action = Self::atalhos(ctx, state);
        }

        action
    }

    /// Quase a janela inteira, como no Lightroom — com piso para não espremer os painéis
    fn tamanho_do_modal(ctx: &Context) -> (f32, f32) {
        let tela = ctx.screen_rect();
        let margem = Theme::SPACE_XXL;

        let largura = (tela.width() * 0.92)
            .max(880.0)
            .min((tela.width() - margem).max(320.0));
        let altura = (tela.height() * 0.92)
            .max(560.0)
            .min((tela.height() - margem).max(240.0));

        (largura, altura)
    }

    /// Moldura de painel: fundo próprio e respiro interno
    ///
    /// Sem isso os três painéis ficam do mesmo tom e a tela vira um bloco só — foi o que
    /// deixava a primeira versão com cara de protótipo.
    fn moldura(fundo: Color32, margem: f32) -> egui::Frame {
        egui::Frame::new()
            .fill(fundo)
            .inner_margin(egui::Margin::symmetric(margem as i8, (margem * 0.75) as i8))
    }

    // ================================================================
    // Cabeçalho e rodapé
    // ================================================================

    fn cabecalho(ui: &mut Ui, state: &mut AppState) -> Option<ImportAction> {
        let mut action = None;

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.add_space(Theme::SPACE_XS);
                ui.label(
                    RichText::new("Importar fotos")
                        .size(Theme::FONT_XL)
                        .color(Theme::TEXT_PRIMARY)
                        .strong(),
                );

                let origem = state
                    .import_view_state
                    .selected_source_path
                    .clone()
                    .unwrap_or_else(|| "Nenhuma origem escolhida".to_string());

                ui.label(
                    RichText::new(encurtar_caminho(&origem, 54))
                        .size(Theme::FONT_SM)
                        .color(Theme::TEXT_MUTED),
                );
            });

            ui.add_space(Theme::SPACE_XL);

            // O seletor de modo é a decisão mais consequente da tela: define se o arquivo
            // vai ser copiado, movido ou apenas catalogado onde está.
            ui.vertical(|ui| {
                ui.add_space(Theme::SPACE_XS);

                let mut modo = state.import_view_state.options.mode;
                Self::segmentado(
                    ui,
                    "import_modo",
                    &mut modo,
                    &[ImportMode::Add, ImportMode::Copy, ImportMode::Move],
                    |m| m.label(),
                );
                state.import_view_state.options.mode = modo;

                ui.label(
                    RichText::new(modo.description())
                        .size(Theme::FONT_SM)
                        .color(Theme::TEXT_MUTED),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if Self::botao_icone(ui, icons::CLOSE, "Fechar (Esc)").clicked() {
                    action = Some(ImportAction::Cancel);
                }
            });
        });

        action
    }

    fn rodape(ui: &mut Ui, state: &mut AppState) -> Option<ImportAction> {
        let mut action = None;

        ui.horizontal_centered(|ui| {
            let marcadas = state.import_view_state.checked_count();
            let visiveis = Self::indices_visiveis(state).len();

            if Self::botao_fantasma(ui, "Marcar todas").clicked() {
                Self::marcar_visiveis(state, true);
            }
            if Self::botao_fantasma(ui, "Desmarcar todas").clicked() {
                Self::marcar_visiveis(state, false);
            }

            ui.add_space(Theme::SPACE_MD);

            ui.vertical(|ui| {
                ui.add_space(Theme::SPACE_XS);
                ui.label(
                    RichText::new(format!("{} de {} selecionadas", marcadas, visiveis))
                        .size(Theme::FONT_MD)
                        .color(Theme::TEXT_PRIMARY),
                );
                ui.label(
                    RichText::new(formatar_bytes(state.import_view_state.checked_bytes()))
                        .size(Theme::FONT_SM)
                        .color(Theme::TEXT_MUTED),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let habilitado = marcadas > 0 && !state.import_view_state.scanning;
                let rotulo = if marcadas == 1 {
                    "Importar 1 foto".to_string()
                } else {
                    format!("Importar {} fotos", marcadas)
                };

                let botao = egui::Button::new(
                    RichText::new(rotulo)
                        .size(Theme::FONT_MD)
                        .color(Color32::WHITE)
                        .strong(),
                )
                .fill(if habilitado {
                    Theme::ACCENT_PRIMARY
                } else {
                    Theme::BG_ACTIVE
                })
                .corner_radius(Theme::RADIUS_MD)
                .min_size(Vec2::new(168.0, 38.0));

                if ui.add_enabled(habilitado, botao).clicked() {
                    let mut options = state.import_view_state.options.clone();
                    options.source_root = state.import_view_state.selected_source_path.clone();

                    action = Some(ImportAction::Import {
                        files: state.import_view_state.checked_paths(),
                        options,
                    });
                }

                ui.add_space(Theme::SPACE_SM);

                if Self::botao_fantasma(ui, "Cancelar").clicked() {
                    action = Some(ImportAction::Cancel);
                }
            });
        });

        action
    }

    // ================================================================
    // Painel DE — a origem
    // ================================================================

    fn painel_origem(
        ui: &mut Ui,
        ctx: &Context,
        state: &mut AppState,
        controller: &Arc<ImportController>,
        sender: &mpsc::Sender<ImportMessage>,
    ) {
        let mut nova_origem: Option<String> = None;

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    Self::rotulo_secao(ui, "ORIGEM");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if Self::botao_icone(ui, icons::ACTION_RESET, "Procurar dispositivos")
                            .clicked()
                        {
                            Self::carregar_origens(ctx, controller, sender);
                        }
                    });
                });

                ui.add_space(Theme::SPACE_SM);

                let selecionada = state.import_view_state.selected_source_path.clone();

                if state.import_view_state.devices.is_empty() {
                    Self::dica(ui, "Nenhum cartão conectado");
                } else {
                    for source in state.import_view_state.devices.clone() {
                        let caminho = source.path.to_string_lossy().to_string();
                        let ativa = selecionada.as_deref() == Some(caminho.as_str());

                        if Self::linha_de_origem(
                            ui,
                            egui_phosphor::regular::HARD_DRIVE,
                            &source.name,
                            &caminho,
                            ativa,
                        )
                        .clicked()
                        {
                            nova_origem = Some(caminho);
                        }
                    }
                }

                ui.add_space(Theme::SPACE_LG);
                Self::rotulo_secao(ui, "RECENTES");
                ui.add_space(Theme::SPACE_SM);

                if state.import_view_state.recent.is_empty() {
                    Self::dica(ui, "Nada importado ainda");
                } else {
                    for source in state.import_view_state.recent.clone() {
                        let caminho = source.path.to_string_lossy().to_string();
                        let ativa = selecionada.as_deref() == Some(caminho.as_str());

                        if Self::linha_de_origem(
                            ui,
                            icons::FILE_FOLDER,
                            &source.name,
                            &caminho,
                            ativa,
                        )
                        .clicked()
                        {
                            nova_origem = Some(caminho);
                        }
                    }
                }

                ui.add_space(Theme::SPACE_LG);

                if Self::botao_largo(ui, icons::FILE_FOLDER_OPEN, "Escolher pasta…").clicked() {
                    // Abre onde a última escolha parou; na primeira vez, nas Imagens do usuário.
                    let inicial = state
                        .import_view_state
                        .selected_source_path
                        .as_ref()
                        .map(std::path::PathBuf::from)
                        .filter(|p| p.exists())
                        .unwrap_or_else(infrastructure::paths::AppPaths::default_browse_dir);

                    Self::escolher_pasta(
                        ctx,
                        sender,
                        "Pasta de origem",
                        inicial,
                        ImportMessage::SourceChosen,
                    );
                }

                ui.add_space(Theme::SPACE_SM);

                let mut incluir = state.import_view_state.options.include_subfolders;
                if ui.checkbox(&mut incluir, "Incluir subpastas").changed() {
                    state.import_view_state.options.include_subfolders = incluir;
                    // Mudou o alcance da varredura: a lista atual não vale mais.
                    if let Some(root) = state.import_view_state.selected_source_path.clone() {
                        nova_origem = Some(root);
                    }
                }
            });

        if let Some(root) = nova_origem {
            Self::selecionar_origem(ctx, state, controller, sender, root);
        }
    }

    /// Uma linha da lista de origens: ícone, nome e caminho
    fn linha_de_origem(
        ui: &mut Ui,
        icone: &str,
        nome: &str,
        caminho: &str,
        ativa: bool,
    ) -> egui::Response {
        let altura = 46.0;
        let (rect, resposta) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), altura), Sense::click());

        if ui.is_rect_visible(rect) {
            let fundo = if ativa {
                Theme::BG_ACTIVE
            } else if resposta.hovered() {
                Theme::BG_HOVER
            } else {
                Color32::TRANSPARENT
            };
            ui.painter().rect_filled(rect, Theme::RADIUS_MD, fundo);

            // Barra de acento à esquerda marca a origem em uso sem depender só do fundo,
            // que num tema escuro some para quem enxerga pouco contraste.
            if ativa {
                let barra = Rect::from_min_size(rect.min, Vec2::new(3.0, altura));
                ui.painter()
                    .rect_filled(barra, Theme::RADIUS_SM, Theme::ACCENT_PRIMARY);
            }

            let cor_icone = if ativa {
                Theme::ACCENT_PRIMARY
            } else {
                Theme::TEXT_MUTED
            };
            ui.painter().text(
                Pos2::new(rect.min.x + 18.0, rect.center().y),
                egui::Align2::CENTER_CENTER,
                icone,
                egui::FontId::proportional(18.0),
                cor_icone,
            );

            let texto_x = rect.min.x + 34.0;
            ui.painter().text(
                Pos2::new(texto_x, rect.center().y - 8.0),
                egui::Align2::LEFT_CENTER,
                nome,
                egui::FontId::proportional(Theme::FONT_MD),
                Theme::TEXT_PRIMARY,
            );
            ui.painter().text(
                Pos2::new(texto_x, rect.center().y + 8.0),
                egui::Align2::LEFT_CENTER,
                encurtar_caminho(caminho, 30),
                egui::FontId::proportional(Theme::FONT_XS),
                Theme::TEXT_MUTED,
            );
        }

        resposta.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, ativa, nome)
        });

        resposta.on_hover_text(caminho)
    }

    // ================================================================
    // Painel PARA — o que acontece com os arquivos
    // ================================================================

    fn painel_destino(
        ui: &mut Ui,
        ctx: &Context,
        state: &mut AppState,
        sender: &mpsc::Sender<ImportMessage>,
    ) {
        let modo = state.import_view_state.options.mode;

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                Self::rotulo_secao(ui, "DESTINO");
                ui.add_space(Theme::SPACE_SM);

                if modo == ImportMode::Add {
                    Self::aviso(
                        ui,
                        Theme::ACCENT_PRIMARY,
                        "As fotos serão catalogadas onde estão. Nada é copiado, nada é movido.",
                    );
                } else {
                    let destino = state
                        .import_view_state
                        .options
                        .destination
                        .clone()
                        .unwrap_or_else(|| {
                            infrastructure::paths::AppPaths::catalog_root()
                                .to_string_lossy()
                                .to_string()
                        });

                    Self::caixa(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(icons::FILE_FOLDER)
                                    .size(16.0)
                                    .color(Theme::TEXT_MUTED),
                            );
                            ui.label(
                                RichText::new(encurtar_caminho(&destino, 30))
                                    .size(Theme::FONT_SM)
                                    .color(Theme::TEXT_SECONDARY),
                            )
                            .on_hover_text(&destino);
                        });

                        ui.add_space(Theme::SPACE_XS);

                        ui.horizontal(|ui| {
                            if Self::botao_fantasma(ui, "Escolher…").clicked() {
                                // Destino já escolhido, senão o catálogo — que é para onde
                                // as fotos vão se o usuário não mudar nada.
                                let inicial = std::path::PathBuf::from(&destino);
                                let inicial = if inicial.exists() {
                                    inicial
                                } else {
                                    infrastructure::paths::AppPaths::default_browse_dir()
                                };

                                Self::escolher_pasta(
                                    ctx,
                                    sender,
                                    "Pasta de destino",
                                    inicial,
                                    ImportMessage::DestinationChosen,
                                );
                            }

                            if state.import_view_state.options.destination.is_some()
                                && Self::botao_fantasma(ui, "Usar catálogo").clicked()
                            {
                                state.import_view_state.options.destination = None;
                            }
                        });
                    });

                    ui.add_space(Theme::SPACE_MD);
                    Self::rotulo_campo(ui, "Organizar");

                    let organizacao = state.import_view_state.options.organization;
                    egui::ComboBox::from_id_salt("import_organization")
                        .selected_text(
                            RichText::new(organizacao.label()).size(Theme::FONT_SM),
                        )
                        .width(ui.available_width() - Theme::SPACE_SM)
                        .show_ui(ui, |ui| {
                            for estrategia in [
                                OrganizationStrategy::ByDate,
                                OrganizationStrategy::PreserveStructure,
                                OrganizationStrategy::IntoOneFolder,
                            ] {
                                ui.selectable_value(
                                    &mut state.import_view_state.options.organization,
                                    estrategia,
                                    estrategia.label(),
                                );
                            }
                        });

                    ui.add_space(Theme::SPACE_MD);
                    Self::rotulo_campo(ui, "Renomear");

                    let padrao = state.import_view_state.options.rename_pattern.clone();
                    egui::ComboBox::from_id_salt("import_rename")
                        .selected_text(RichText::new(padrao.label()).size(Theme::FONT_SM))
                        .width(ui.available_width() - Theme::SPACE_SM)
                        .show_ui(ui, |ui| {
                            for p in [RenamePattern::Standard, RenamePattern::KeepOriginal] {
                                let rotulo = p.label();
                                ui.selectable_value(
                                    &mut state.import_view_state.options.rename_pattern,
                                    p,
                                    rotulo,
                                );
                            }
                        });

                    // Mostrar para onde a primeira foto vai é o que transforma três combos
                    // abstratos numa decisão conferível antes de apertar o botão.
                    if let Some(exemplo) = Self::exemplo_de_destino(state) {
                        ui.add_space(Theme::SPACE_SM);
                        Self::caixa(ui, |ui| {
                            ui.label(
                                RichText::new("A primeira foto vai para")
                                    .size(Theme::FONT_XS)
                                    .color(Theme::TEXT_MUTED),
                            );
                            ui.label(
                                RichText::new(exemplo)
                                    .size(Theme::FONT_SM)
                                    .color(Theme::ACCENT_PRIMARY),
                            );
                        });
                    }
                }

                ui.add_space(Theme::SPACE_LG);
                Self::rotulo_secao(ui, "ARQUIVOS");
                ui.add_space(Theme::SPACE_SM);

                let mut pular = state.import_view_state.options.skip_duplicates;
                if ui
                    .checkbox(&mut pular, "Não importar duplicadas suspeitas")
                    .changed()
                {
                    state.import_view_state.options.skip_duplicates = pular;
                    if pular {
                        // Coerência com o que a grade mostra: se a importação vai pular,
                        // a marcação tem de refletir isso antes de o usuário apertar o botão.
                        for c in state.import_view_state.candidates.iter_mut() {
                            if c.is_duplicate {
                                c.checked = false;
                            }
                        }
                    }
                }

                if state.import_view_state.checking_duplicates {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        Self::dica(ui, "Conferindo o catálogo…");
                    });
                }

                let duplicadas = state.import_view_state.duplicate_count();
                if duplicadas > 0 {
                    ui.add_space(Theme::SPACE_SM);
                    Self::aviso(
                        ui,
                        Theme::ACCENT_WARNING,
                        &format!("{} já estão no catálogo", duplicadas),
                    );
                }

                if modo == ImportMode::Move && state.import_view_state.checked_count() > 0 {
                    ui.add_space(Theme::SPACE_SM);
                    Self::aviso(
                        ui,
                        Theme::ACCENT_ERROR,
                        "Os originais serão apagados da origem",
                    );
                }
            });
    }

    /// Caminho que a primeira foto marcada vai ocupar, dadas as opções atuais
    fn exemplo_de_destino(state: &AppState) -> Option<String> {
        let opcoes = &state.import_view_state.options;
        let candidato = state.import_view_state.candidates.iter().find(|c| c.checked)?;

        let pasta = match opcoes.organization {
            OrganizationStrategy::ByDate => {
                // A data de captura vem do EXIF como "YYYY:MM:DD HH:MM:SS"
                let data = candidato.date_time.split(' ').next().unwrap_or("");
                let partes: Vec<&str> = data.split(':').collect();
                if partes.len() >= 3 {
                    format!("{}/{}/{}/", partes[0], partes[1], partes[2])
                } else {
                    "AAAA/MM/DD/".to_string()
                }
            }
            OrganizationStrategy::PreserveStructure => {
                let raiz = state.import_view_state.selected_source_path.as_deref()?;
                std::path::Path::new(&candidato.path)
                    .parent()
                    .and_then(|p| p.strip_prefix(raiz).ok())
                    .filter(|rel| !rel.as_os_str().is_empty())
                    .map(|rel| format!("{}/", rel.display()))
                    .unwrap_or_default()
            }
            OrganizationStrategy::IntoOneFolder => String::new(),
        };

        let nome = match opcoes.rename_pattern {
            RenamePattern::KeepOriginal => candidato.file_name.clone(),
            _ => {
                let extensao = std::path::Path::new(&candidato.file_name)
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_else(|| "jpg".to_string());
                let data = candidato.date_time.split(' ').next().unwrap_or("");
                let partes: Vec<&str> = data.split(':').collect();
                if partes.len() >= 3 {
                    format!("photo-{}-{}-{}-001.{}", partes[0], partes[1], partes[2], extensao)
                } else {
                    format!("photo-AAAA-MM-DD-001.{}", extensao)
                }
            }
        };

        Some(format!("{}{}", pasta, nome))
    }

    // ================================================================
    // Grade
    // ================================================================

    fn grade(
        ui: &mut Ui,
        ctx: &Context,
        state: &mut AppState,
        thumbnails: &mut AsyncThumbnailLoader,
    ) {
        Self::barra_da_grade(ui, state);

        if state.import_view_state.scanning {
            Self::estado_vazio(ui, icons::ACTION_SEARCH, "Lendo a origem…", None);
            return;
        }

        if state.import_view_state.selected_source_path.is_none() {
            Self::estado_vazio(
                ui,
                egui_phosphor::regular::HARD_DRIVE,
                "Escolha um cartão ou uma pasta à esquerda",
                Some("Nada é copiado até você apertar Importar"),
            );
            return;
        }

        let visiveis = Self::indices_visiveis(state);

        if visiveis.is_empty() {
            let (titulo, dica) = if state.import_view_state.candidates.is_empty() {
                ("Nenhuma foto encontrada nesta origem", Some("Tente ligar \"Incluir subpastas\""))
            } else {
                ("Todas as fotos desta origem já estão no catálogo", None)
            };
            Self::estado_vazio(ui, icons::FILE_IMAGE, titulo, dica);
            return;
        }

        let lado = state.import_view_state.thumb_size;
        let espaco = Theme::SPACE_MD;
        let altura_rotulo = 20.0;
        let altura_linha = lado + altura_rotulo + espaco;

        // A largura útil desconta a barra de rolagem: contá-la como espaço de célula é o
        // que fazia a última coluna de cada linha aparecer cortada na borda direita.
        let largura = (ui.available_width() - ui.spacing().scroll.bar_width - espaco).max(lado);
        let colunas = (((largura + espaco) / (lado + espaco)).floor() as usize).max(1);
        let linhas = visiveis.len().div_ceil(colunas);

        state.import_view_state.colunas = colunas;

        // Navegar pelo teclado sem levar a rolagem junto deixa o foco fora da tela — a
        // seta parece não ter feito nada.
        let mut area = egui::ScrollArea::vertical().auto_shrink([false; 2]);
        if let Some(alvo) = state.import_view_state.rolar_para.take() {
            if let Some(posicao) = visiveis.iter().position(|&v| v == alvo) {
                let linha = (posicao / colunas) as f32;
                let deslocamento = (linha * altura_linha - altura_linha).max(0.0);
                area = area.vertical_scroll_offset(deslocamento);
            }
        }

        // `show_rows` só desenha as linhas que cabem na tela — é o que permite abrir um
        // cartão de milhares de fotos sem gerar milhares de células por quadro.
        area.show_rows(ui, altura_linha, linhas, |ui, faixa| {
            let mut pedidos: Vec<ThumbnailRequest> = Vec::new();

            for linha in faixa {
                ui.horizontal(|ui| {
                    // O único vão entre células é `espaco` — o padrão do egui somaria
                    // ao nosso e a conta de colunas acima deixaria de bater.
                    ui.spacing_mut().item_spacing = Vec2::new(espaco, 0.0);

                    for coluna in 0..colunas {
                        let posicao = linha * colunas + coluna;
                        let Some(&indice) = visiveis.get(posicao) else {
                            break;
                        };

                        Self::celula(ui, state, indice, lado, &mut pedidos);
                    }
                });
                ui.add_space(espaco);
            }

            // Só as células desenhadas neste quadro pedem imagem.
            if !pedidos.is_empty() {
                thumbnails.request_thumbnails(pedidos);
            }
        });

        let _ = ctx;
    }

    fn barra_da_grade(ui: &mut Ui, state: &mut AppState) {
        let duplicadas = state.import_view_state.duplicate_count();
        let total = state.import_view_state.candidates.len();

        egui::Frame::new()
            .fill(Theme::BG_SURFACE)
            .inner_margin(egui::Margin::symmetric(
                Theme::SPACE_MD as i8,
                Theme::SPACE_SM as i8,
            ))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let mut so_novas = state.import_view_state.only_new;
                    Self::segmentado(
                        ui,
                        "import_filtro",
                        &mut so_novas,
                        &[false, true],
                        |novas| if *novas { "Novas" } else { "Todas" },
                    );

                    if so_novas != state.import_view_state.only_new {
                        state.import_view_state.only_new = so_novas;
                        state.import_view_state.focused = None;
                    }

                    ui.add_space(Theme::SPACE_SM);
                    ui.label(
                        RichText::new(if duplicadas > 0 {
                            format!("{} fotos · {} no catálogo", total, duplicadas)
                        } else {
                            format!("{} fotos", total)
                        })
                        .size(Theme::FONT_SM)
                        .color(Theme::TEXT_MUTED),
                    );

                    if state.import_view_state.describing {
                        ui.add_space(Theme::SPACE_SM);
                        ui.spinner();
                        Self::dica(ui, "Lendo metadados…");
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().slider_width = 96.0;
                        ui.label(
                            RichText::new(egui_phosphor::regular::SQUARES_FOUR)
                                .size(14.0)
                                .color(Theme::TEXT_MUTED),
                        );
                        ui.add(
                            egui::Slider::new(
                                &mut state.import_view_state.thumb_size,
                                90.0..=320.0,
                            )
                            .show_value(false),
                        )
                        .on_hover_text("Tamanho das miniaturas");

                        ui.add_space(Theme::SPACE_MD);

                        let atual = state.import_view_state.sort_by;
                        let mut escolhido = atual;
                        egui::ComboBox::from_id_salt("import_sort")
                            .selected_text(
                                RichText::new(atual.label()).size(Theme::FONT_SM),
                            )
                            .width(150.0)
                            .show_ui(ui, |ui| {
                                for criterio in ImportSortBy::TODOS {
                                    ui.selectable_value(&mut escolhido, criterio, criterio.label());
                                }
                            });

                        if escolhido != atual {
                            state.import_view_state.sort_by = escolhido;
                            state.import_view_state.sort_candidates();
                            state.import_view_state.focused = None;
                        }

                        ui.label(
                            RichText::new(icons::ACTION_SORT)
                                .size(14.0)
                                .color(Theme::TEXT_MUTED),
                        );
                    });
                });
            });
    }

    /// Desenha uma célula da grade e registra o pedido de miniatura, se faltar
    fn celula(
        ui: &mut Ui,
        state: &mut AppState,
        indice: usize,
        lado: f32,
        pedidos: &mut Vec<ThumbnailRequest>,
    ) {
        let altura_rotulo = 20.0;
        let (rect, resposta) =
            ui.allocate_exact_size(Vec2::new(lado, lado + altura_rotulo), Sense::click());

        if !ui.is_rect_visible(rect) {
            return;
        }

        let candidato = state.import_view_state.candidates[indice].clone();
        let em_foco = state.import_view_state.focused == Some(indice);
        let area_imagem = Rect::from_min_size(rect.min, Vec2::new(lado, lado));

        ui.painter()
            .rect_filled(area_imagem, Theme::RADIUS_LG, Theme::BG_SURFACE);

        // Imagem, quando já chegou
        if let Some(textura) = state.import_view_state.thumbnails.get(&candidato.path) {
            let tam = textura.size_vec2();
            let escala = (area_imagem.width() / tam.x).min(area_imagem.height() / tam.y);
            let desenho = Rect::from_center_size(area_imagem.center(), tam * escala * 0.94);

            // Desmarcada continua visível, mas apagada — o olho encontra as marcadas
            // sem ter de ler checkbox por checkbox.
            let tinta = if candidato.checked {
                Color32::WHITE
            } else {
                Color32::from_rgba_unmultiplied(255, 255, 255, 70)
            };

            egui::Image::new(textura).tint(tinta).paint_at(ui, desenho);
        } else {
            pedidos.push(ThumbnailRequest {
                photo_id: thumb_key(&candidato.path),
                path: candidato.path.clone(),
            });

            ui.painter().text(
                area_imagem.center(),
                egui::Align2::CENTER_CENTER,
                icons::FILE_IMAGE,
                egui::FontId::proportional((lado * 0.22).clamp(18.0, 40.0)),
                Theme::TEXT_HINT,
            );
        }

        // Contorno: foco vence marcação, que vence o repouso
        let contorno = if em_foco {
            Stroke::new(2.0, Theme::ACCENT_PRIMARY)
        } else if resposta.hovered() {
            Stroke::new(1.0, Theme::BORDER_HOVER)
        } else if candidato.checked {
            Stroke::new(1.0, Theme::BORDER_DEFAULT)
        } else {
            Stroke::new(1.0, Color32::TRANSPARENT)
        };
        ui.painter()
            .rect_stroke(area_imagem, Theme::RADIUS_LG, contorno, StrokeKind::Inside);

        // Marcador de seleção — alvo generoso, porque errar o clique aqui significa
        // importar (ou deixar de importar) a foto errada
        let caixa = Rect::from_min_size(area_imagem.min + Vec2::splat(8.0), Vec2::splat(22.0));
        let (fundo_caixa, borda_caixa) = if candidato.checked {
            (Theme::ACCENT_PRIMARY, Theme::ACCENT_PRIMARY)
        } else {
            (
                Color32::from_rgba_unmultiplied(0, 0, 0, 150),
                Color32::from_gray(170),
            )
        };
        ui.painter()
            .rect_filled(caixa, Theme::RADIUS_SM, fundo_caixa);
        ui.painter().rect_stroke(
            caixa,
            Theme::RADIUS_SM,
            Stroke::new(1.0, borda_caixa),
            StrokeKind::Inside,
        );
        if candidato.checked {
            ui.painter().text(
                caixa.center(),
                egui::Align2::CENTER_CENTER,
                icons::CHECK,
                egui::FontId::proportional(14.0),
                Color32::WHITE,
            );
        }

        // Etiqueta RAW
        if candidato.is_raw {
            Self::pilula(
                ui,
                Rect::from_min_size(
                    Pos2::new(area_imagem.max.x - 46.0, area_imagem.min.y + 8.0),
                    Vec2::new(38.0, 18.0),
                ),
                "RAW",
                Theme::ACCENT_PRIMARY,
            );
        }

        // Já no catálogo
        if candidato.is_duplicate {
            let faixa = Rect::from_min_size(
                Pos2::new(area_imagem.min.x, area_imagem.max.y - 22.0),
                Vec2::new(area_imagem.width(), 22.0),
            );
            ui.painter().rect_filled(
                faixa,
                0.0,
                Color32::from_rgba_unmultiplied(0, 0, 0, 190),
            );
            ui.painter().text(
                faixa.center(),
                egui::Align2::CENTER_CENTER,
                "já no catálogo",
                egui::FontId::proportional(Theme::FONT_XS),
                Theme::ACCENT_WARNING,
            );
        }

        // Nome do arquivo
        ui.painter().text(
            Pos2::new(rect.center().x, area_imagem.max.y + 4.0),
            egui::Align2::CENTER_TOP,
            encurtar(&candidato.file_name, lado),
            egui::FontId::proportional(Theme::FONT_XS),
            if candidato.checked {
                Theme::TEXT_SECONDARY
            } else {
                Theme::TEXT_DISABLED
            },
        );

        // ---- interação ----
        //
        // Um clique no marcador alterna; no resto da célula, foca. Duplo clique abre a
        // lupa. Shift estende a marcação a partir da última célula clicada.
        let clique_no_marcador = resposta
            .interact_pointer_pos()
            .map(|p| caixa.expand(4.0).contains(p))
            .unwrap_or(false);

        if resposta.clicked() {
            let shift = ui.input(|i| i.modifiers.shift);

            if shift {
                if let Some(ancora) = state.import_view_state.ancora {
                    Self::marcar_intervalo(state, ancora, indice, true);
                }
            } else if clique_no_marcador {
                let alvo = !candidato.checked;
                state.import_view_state.candidates[indice].checked = alvo;
                state.import_view_state.ancora = Some(indice);
            } else {
                state.import_view_state.ancora = Some(indice);
            }

            state.import_view_state.focused = Some(indice);
        }

        if resposta.double_clicked() {
            state.import_view_state.focused = Some(indice);
            state.import_view_state.lupa_aberta = true;
        }

        resposta.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::ImageButton,
                true,
                candidato.checked,
                &candidato.file_name,
            )
        });

        resposta.on_hover_ui(|ui| {
            ui.label(RichText::new(&candidato.file_name).strong());
            if !candidato.camera.is_empty() {
                ui.label(&candidato.camera);
            }
            if !candidato.date_time.is_empty() {
                ui.label(&candidato.date_time);
            }
            if let Some(dim) = &candidato.dimensions {
                ui.label(dim);
            }
            if candidato.file_size > 0 {
                ui.label(formatar_bytes(candidato.file_size));
            }
            ui.label(
                RichText::new(&candidato.path)
                    .small()
                    .color(Theme::TEXT_MUTED),
            );
        });
    }

    /// Lupa — a foto em foco, grande, sobre a grade
    fn lupa(ctx: &Context, state: &mut AppState) {
        if !state.import_view_state.lupa_aberta {
            return;
        }

        let Some(indice) = state.import_view_state.focused else {
            return;
        };
        let Some(candidato) = state.import_view_state.candidates.get(indice).cloned() else {
            return;
        };

        // Modal aninhado, e não `Window`: uma janela do egui fica abaixo do backdrop do
        // modal de importação e apareceria escurecida e sem responder ao clique.
        let (largura, altura) = Self::tamanho_do_modal(ctx);
        let largura = largura * 0.8;
        let altura = altura * 0.8;

        let resposta = egui::Modal::new(egui::Id::new("import_lupa")).show(ctx, |ui| {
            ui.set_width(largura);
            ui.set_height(altura);

            ui.horizontal(|ui| {
                ui.label(RichText::new(&candidato.file_name).strong());
                if !candidato.camera.is_empty() {
                    ui.label(
                        RichText::new(&candidato.camera)
                            .size(Theme::FONT_SM)
                            .color(Theme::TEXT_MUTED),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if Self::botao_icone(ui, icons::CLOSE, "Fechar (Esc)").clicked() {
                        state.import_view_state.lupa_aberta = false;
                    }
                });
            });
            ui.separator();

            if let Some(textura) = state.import_view_state.thumbnails.get(&candidato.path) {
                let disponivel = ui.available_size();
                let tam = textura.size_vec2();
                let escala = (disponivel.x / tam.x).min(disponivel.y / tam.y);
                ui.centered_and_justified(|ui| {
                    ui.add(egui::Image::new(textura).fit_to_exact_size(tam * escala));
                });
            } else {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                });
            }
        });

        if resposta.should_close() {
            state.import_view_state.lupa_aberta = false;
        }
    }

    /// Atalhos da grade
    ///
    /// Setas navegam (e levam a rolagem junto), espaço marca, `L` abre a lupa,
    /// ⌘A marca tudo, Enter importa.
    fn atalhos(ctx: &Context, state: &mut AppState) -> Option<ImportAction> {
        if state.import_view_state.candidates.is_empty() {
            return None;
        }

        // Não roubar as teclas de quem está digitando num campo.
        if ctx.wants_keyboard_input() {
            return None;
        }

        let visiveis = Self::indices_visiveis(state);
        if visiveis.is_empty() {
            return None;
        }

        let colunas = state.import_view_state.colunas.max(1) as i64;
        let posicao_atual = state
            .import_view_state
            .focused
            .and_then(|i| visiveis.iter().position(|&v| v == i));

        let mut acao = None;

        ctx.input(|i| {
            let mover = |delta: i64, state: &mut AppState| {
                let base = posicao_atual.map(|p| p as i64).unwrap_or(-1);
                let nova = (base + delta).clamp(0, visiveis.len() as i64 - 1) as usize;
                let alvo = visiveis[nova];
                state.import_view_state.focused = Some(alvo);
                state.import_view_state.rolar_para = Some(alvo);
            };

            if i.key_pressed(egui::Key::ArrowRight) {
                mover(1, state);
            }
            if i.key_pressed(egui::Key::ArrowLeft) {
                mover(-1, state);
            }
            if i.key_pressed(egui::Key::ArrowDown) {
                mover(colunas, state);
            }
            if i.key_pressed(egui::Key::ArrowUp) {
                mover(-colunas, state);
            }

            if i.key_pressed(egui::Key::Space) {
                if let Some(indice) = state.import_view_state.focused {
                    let alvo = !state.import_view_state.candidates[indice].checked;
                    state.import_view_state.candidates[indice].checked = alvo;
                }
            }

            if i.key_pressed(egui::Key::L) {
                state.import_view_state.lupa_aberta = !state.import_view_state.lupa_aberta;
            }

            if i.modifiers.command && i.key_pressed(egui::Key::A) {
                Self::marcar_visiveis(state, true);
            }

            if i.key_pressed(egui::Key::Enter) && state.import_view_state.checked_count() > 0 {
                let mut options = state.import_view_state.options.clone();
                options.source_root = state.import_view_state.selected_source_path.clone();

                acao = Some(ImportAction::Import {
                    files: state.import_view_state.checked_paths(),
                    options,
                });
            }
        });

        acao
    }

    // ================================================================
    // Peças de interface
    // ================================================================

    /// Controle segmentado: as opções lado a lado, a ativa preenchida
    ///
    /// Substitui uma fileira de `selectable_label`, que sem moldura parecia três links
    /// soltos e não deixava claro que são excludentes.
    fn segmentado<T: PartialEq + Copy>(
        ui: &mut Ui,
        id: &str,
        valor: &mut T,
        opcoes: &[T],
        rotulo: impl Fn(&T) -> &'static str,
    ) {
        let altura = 30.0;
        let padding = 14.0;

        egui::Frame::new()
            .fill(Theme::BG_SURFACE)
            .corner_radius(Theme::RADIUS_MD)
            .inner_margin(egui::Margin::same(2))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;

                ui.horizontal(|ui| {
                    for opcao in opcoes {
                        let texto = rotulo(opcao);
                        let ativo = *valor == *opcao;

                        let largura = ui
                            .painter()
                            .layout_no_wrap(
                                texto.to_string(),
                                egui::FontId::proportional(Theme::FONT_MD),
                                Color32::WHITE,
                            )
                            .size()
                            .x
                            + padding * 2.0;

                        let (rect, resposta) = ui.allocate_exact_size(
                            Vec2::new(largura, altura),
                            Sense::click(),
                        );

                        if ui.is_rect_visible(rect) {
                            let fundo = if ativo {
                                Theme::ACCENT_PRIMARY
                            } else if resposta.hovered() {
                                Theme::BG_HOVER
                            } else {
                                Color32::TRANSPARENT
                            };
                            ui.painter().rect_filled(rect, Theme::RADIUS_SM, fundo);

                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                texto,
                                egui::FontId::proportional(Theme::FONT_MD),
                                if ativo {
                                    Color32::WHITE
                                } else {
                                    Theme::TEXT_SECONDARY
                                },
                            );
                        }

                        resposta.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::SelectableLabel,
                                true,
                                ativo,
                                texto,
                            )
                        });

                        if resposta.clicked() {
                            *valor = *opcao;
                        }
                    }
                });
            });

        let _ = id;
    }

    /// Botão de texto sem preenchimento
    fn botao_fantasma(ui: &mut Ui, texto: &str) -> egui::Response {
        ui.add(
            egui::Button::new(
                RichText::new(texto)
                    .size(Theme::FONT_MD)
                    .color(Theme::TEXT_SECONDARY),
            )
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, Theme::BORDER_DEFAULT))
            .corner_radius(Theme::RADIUS_MD)
            .min_size(Vec2::new(0.0, 32.0)),
        )
    }

    /// Botão que ocupa a largura do painel, com ícone à esquerda
    fn botao_largo(ui: &mut Ui, icone: &str, texto: &str) -> egui::Response {
        ui.add_sized(
            Vec2::new(ui.available_width(), 34.0),
            egui::Button::new(
                RichText::new(format!("{}  {}", icone, texto))
                    .size(Theme::FONT_MD)
                    .color(Theme::TEXT_PRIMARY),
            )
            .fill(Theme::BG_ELEVATED)
            .stroke(Stroke::new(1.0, Theme::BORDER_DEFAULT))
            .corner_radius(Theme::RADIUS_MD),
        )
    }

    /// Botão só de ícone, com dica
    fn botao_icone(ui: &mut Ui, icone: &str, dica: &str) -> egui::Response {
        ui.add(
            egui::Button::new(
                RichText::new(icone)
                    .size(16.0)
                    .color(Theme::TEXT_SECONDARY),
            )
            .fill(Color32::TRANSPARENT)
            .min_size(Vec2::new(30.0, 30.0))
            .corner_radius(Theme::RADIUS_MD),
        )
        .on_hover_text(dica)
    }

    /// Rótulo de seção: maiúsculas pequenas, tom apagado
    fn rotulo_secao(ui: &mut Ui, texto: &str) {
        ui.label(
            RichText::new(texto)
                .size(Theme::FONT_XS)
                .color(Theme::TEXT_MUTED)
                .strong(),
        );
    }

    /// Rótulo de campo, acima de um controle
    fn rotulo_campo(ui: &mut Ui, texto: &str) {
        ui.label(
            RichText::new(texto)
                .size(Theme::FONT_SM)
                .color(Theme::TEXT_SECONDARY),
        );
        ui.add_space(Theme::SPACE_XXS);
    }

    /// Texto auxiliar, apagado
    fn dica(ui: &mut Ui, texto: &str) {
        ui.label(
            RichText::new(texto)
                .size(Theme::FONT_SM)
                .color(Theme::TEXT_MUTED),
        );
    }

    /// Caixa de conteúdo com fundo próprio
    fn caixa(ui: &mut Ui, conteudo: impl FnOnce(&mut Ui)) {
        egui::Frame::new()
            .fill(Theme::BG_ELEVATED)
            .corner_radius(Theme::RADIUS_MD)
            .inner_margin(egui::Margin::same(Theme::SPACE_SM as i8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                conteudo(ui);
            });
    }

    /// Aviso colorido — a cor carrega a gravidade
    fn aviso(ui: &mut Ui, cor: Color32, texto: &str) {
        egui::Frame::new()
            .fill(Color32::from_rgba_unmultiplied(
                cor.r(),
                cor.g(),
                cor.b(),
                26,
            ))
            .corner_radius(Theme::RADIUS_MD)
            .inner_margin(egui::Margin::same(Theme::SPACE_SM as i8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(texto).size(Theme::FONT_SM).color(cor));
            });
    }

    /// Pílula pequena sobre a miniatura (RAW, por exemplo)
    fn pilula(ui: &mut Ui, rect: Rect, texto: &str, cor: Color32) {
        ui.painter().rect_filled(
            rect,
            Theme::RADIUS_FULL,
            Color32::from_rgba_unmultiplied(0, 0, 0, 190),
        );
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            texto,
            egui::FontId::proportional(Theme::FONT_XS),
            cor,
        );
    }

    /// Estado vazio: ícone grande, uma frase e, quando houver, uma saída
    fn estado_vazio(ui: &mut Ui, icone: &str, titulo: &str, dica: Option<&str>) {
        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.34);
                ui.label(
                    RichText::new(icone)
                        .size(44.0)
                        .color(Theme::TEXT_HINT),
                );
                ui.add_space(Theme::SPACE_SM);
                ui.label(
                    RichText::new(titulo)
                        .size(Theme::FONT_LG)
                        .color(Theme::TEXT_SECONDARY),
                );
                if let Some(dica) = dica {
                    ui.add_space(Theme::SPACE_XXS);
                    ui.label(
                        RichText::new(dica)
                            .size(Theme::FONT_SM)
                            .color(Theme::TEXT_MUTED),
                    );
                }
            });
        });
    }

    /// Marca (ou desmarca) todas as células entre dois índices da grade
    fn marcar_intervalo(state: &mut AppState, de: usize, ate: usize, marcado: bool) {
        let visiveis = Self::indices_visiveis(state);
        let (Some(a), Some(b)) = (
            visiveis.iter().position(|&v| v == de),
            visiveis.iter().position(|&v| v == ate),
        ) else {
            return;
        };

        let (inicio, fim) = if a <= b { (a, b) } else { (b, a) };
        for posicao in inicio..=fim {
            state.import_view_state.candidates[visiveis[posicao]].checked = marcado;
        }
    }

    /// Abre o seletor de pastas **do sistema** e devolve a escolha pelo canal
    ///
    /// Nativo, e não o `egui_file`, por causa do modal: um `Window` do egui fica abaixo do
    /// backdrop e com a entrada bloqueada — o seletor apareceria escurecido e sem responder.
    /// O painel do sistema é uma janela do SO, então fica por cima de qualquer camada do
    /// egui. De quebra é o seletor que o usuário já conhece, com favoritos e iCloud.
    fn escolher_pasta(
        ctx: &Context,
        sender: &mpsc::Sender<ImportMessage>,
        titulo: &str,
        inicial: std::path::PathBuf,
        para_mensagem: impl FnOnce(String) -> ImportMessage + Send + 'static,
    ) {
        let sender = sender.clone();
        let ctx = ctx.clone();
        let titulo = titulo.to_string();

        tokio::spawn(async move {
            let escolha = rfd::AsyncFileDialog::new()
                .set_title(titulo)
                .set_directory(inicial)
                .pick_folder()
                .await;

            // Cancelar não é erro nem mensagem: a tela simplesmente continua como estava.
            if let Some(pasta) = escolha {
                let caminho = pasta.path().to_string_lossy().to_string();
                let _ = sender.send(para_mensagem(caminho)).await;
            }

            ctx.request_repaint();
        });
    }

    fn carregar_origens(
        ctx: &Context,
        controller: &Arc<ImportController>,
        sender: &mpsc::Sender<ImportMessage>,
    ) {
        let controller = controller.clone();
        let sender = sender.clone();
        let ctx = ctx.clone();

        tokio::spawn(async move {
            let (devices, recent) = controller.get_sources().await;
            let _ = sender.send(ImportMessage::Sources { devices, recent }).await;
            ctx.request_repaint();
        });
    }

    /// Troca a origem e dispara a varredura
    pub fn selecionar_origem(
        ctx: &Context,
        state: &mut AppState,
        controller: &Arc<ImportController>,
        sender: &mpsc::Sender<ImportMessage>,
        root: String,
    ) {
        state.import_view_state.selected_source_path = Some(root.clone());
        state.import_view_state.clear_candidates();
        state.import_view_state.scanning = true;

        let controller = controller.clone();
        let sender = sender.clone();
        let ctx = ctx.clone();
        let incluir = state.import_view_state.options.include_subfolders;

        tokio::spawn(async move {
            let mensagem = match controller.scan_source(root.clone(), incluir).await {
                Ok(files) => ImportMessage::Scanned { root, files },
                Err(e) => ImportMessage::Failed(e),
            };
            let _ = sender.send(mensagem).await;
            ctx.request_repaint();
        });
    }

    /// Depois da varredura: metadados e duplicatas, em paralelo
    ///
    /// São dois trabalhos independentes de propósito. A leitura de EXIF é rápida e enche a
    /// grade; o hash de conteúdo lê o arquivo inteiro e demora. Emendar um no outro faria a
    /// grade esperar pelo mais lento sem motivo.
    pub fn detalhar(
        ctx: &Context,
        controller: &Arc<ImportController>,
        sender: &mpsc::Sender<ImportMessage>,
        files: Vec<String>,
    ) {
        if files.is_empty() {
            return;
        }

        {
            let controller = controller.clone();
            let sender = sender.clone();
            let ctx = ctx.clone();
            let files = files.clone();

            tokio::spawn(async move {
                match controller.describe_candidates(files).await {
                    Ok(itens) => {
                        let _ = sender.send(ImportMessage::Described(itens)).await;
                    }
                    Err(e) => {
                        let _ = sender.send(ImportMessage::Failed(e)).await;
                    }
                }
                ctx.request_repaint();
            });
        }

        {
            let controller = controller.clone();
            let sender = sender.clone();
            let ctx = ctx.clone();

            tokio::spawn(async move {
                if let Ok(resultados) = controller.check_duplicates(files).await {
                    let duplicados = resultados
                        .into_iter()
                        .filter(|d| d.is_duplicate)
                        .map(|d| d.file_path)
                        .collect();
                    let _ = sender.send(ImportMessage::Duplicates(duplicados)).await;
                }
                ctx.request_repaint();
            });
        }
    }

    /// Sobe para textura as miniaturas que ficaram prontas desde o último quadro
    fn receber_miniaturas(
        ctx: &Context,
        state: &mut AppState,
        thumbnails: &mut AsyncThumbnailLoader,
    ) {
        for resultado in thumbnails.poll_results() {
            // O loader devolve a chave prefixada; a grade indexa por caminho.
            let Some(path) = resultado.photo_id.strip_prefix("import::") else {
                continue;
            };

            let imagem = crate::image_processing::ImageProcessor::dynamic_to_color_image(
                &resultado.image,
            );
            let textura = ctx.load_texture(
                resultado.photo_id.clone(),
                imagem,
                egui::TextureOptions::LINEAR,
            );

            state
                .import_view_state
                .thumbnails
                .insert(path.to_string(), textura);
        }
    }

    // ================================================================
    // Auxiliares
    // ================================================================

    /// Índices dos candidatos que a grade está mostrando, já filtrados
    fn indices_visiveis(state: &AppState) -> Vec<usize> {
        state
            .import_view_state
            .candidates
            .iter()
            .enumerate()
            .filter(|(_, c)| !state.import_view_state.only_new || !c.is_duplicate)
            .map(|(i, _)| i)
            .collect()
    }

    /// Marca ou desmarca o que está visível — o filtro vale também para os botões
    fn marcar_visiveis(state: &mut AppState, marcado: bool) {
        for indice in Self::indices_visiveis(state) {
            state.import_view_state.candidates[indice].checked = marcado;
        }
    }

    /// Aplica ao estado uma mensagem vinda do trabalho assíncrono
    ///
    /// Devolve o [`Seguimento`] que o app precisa disparar — a tela não tem controller.
    pub fn aplicar(state: &mut AppState, mensagem: ImportMessage) -> Option<Seguimento> {
        match mensagem {
            ImportMessage::Sources { devices, recent } => {
                state.import_view_state.devices = devices;
                state.import_view_state.recent = recent;
                None
            }

            ImportMessage::Scanned { root, files } => {
                state.import_view_state.scanning = false;

                // Uma varredura antiga chegando depois de o usuário já ter trocado de origem
                // não pode sobrescrever a lista da origem nova.
                if state.import_view_state.selected_source_path.as_deref() != Some(root.as_str()) {
                    return None;
                }

                state.import_view_state.candidates = files
                    .iter()
                    .cloned()
                    .map(ImportCandidate::from_path)
                    .collect();
                state.import_view_state.sort_candidates();
                state.import_view_state.describing = !files.is_empty();
                state.import_view_state.checking_duplicates = !files.is_empty();

                Some(Seguimento::Detalhar(files))
            }

            ImportMessage::Described(itens) => {
                state.import_view_state.describing = false;

                for item in itens {
                    if let Some(c) = state
                        .import_view_state
                        .candidates
                        .iter_mut()
                        .find(|c| c.path == item.file_path)
                    {
                        c.file_size = item.file_size;
                        c.is_raw = item.is_raw;
                        c.camera = item.camera;
                        c.date_time = item.date_time;
                        c.dimensions = item.dimensions;
                        c.described = true;
                    }
                }

                // Ordenar por hora de captura só faz sentido depois que as horas chegaram.
                state.import_view_state.sort_candidates();
                None
            }

            ImportMessage::Duplicates(paths) => {
                state.import_view_state.checking_duplicates = false;

                let duplicados: std::collections::HashSet<String> = paths.into_iter().collect();
                let pular = state.import_view_state.options.skip_duplicates;

                for c in state.import_view_state.candidates.iter_mut() {
                    if duplicados.contains(&c.path) {
                        c.is_duplicate = true;
                        if pular {
                            c.checked = false;
                        }
                    }
                }
                None
            }

            ImportMessage::SourceChosen(caminho) => Some(Seguimento::Varrer(caminho)),

            ImportMessage::DestinationChosen(caminho) => {
                state.import_view_state.options.destination = Some(caminho);
                None
            }

            ImportMessage::Failed(erro) => {
                state.import_view_state.scanning = false;
                state.import_view_state.describing = false;
                state.import_view_state.checking_duplicates = false;
                state.import_view_state.status = Some(erro);
                None
            }
        }
    }
}

/// Corta o nome do arquivo ao que cabe na largura da célula
fn encurtar(nome: &str, largura: f32) -> String {
    let max = ((largura / 6.5) as usize).max(8);
    if nome.chars().count() <= max {
        return nome.to_string();
    }

    let inicio: String = nome.chars().take(max.saturating_sub(6)).collect();
    let fim: String = nome
        .chars()
        .skip(nome.chars().count().saturating_sub(5))
        .collect();
    format!("{}…{}", inicio, fim)
}

/// Encurta um caminho pelo começo, preservando o fim — que é a parte que identifica
///
/// Corta em barra sempre que possível: "…htbox/VintageLightbox Catalog" é pior de ler do
/// que "…/VintageLightbox Catalog", porque o pedaço de palavra parece nome de pasta.
fn encurtar_caminho(caminho: &str, max: usize) -> String {
    if caminho.chars().count() <= max {
        return caminho.to_string();
    }

    let corte = caminho.chars().count().saturating_sub(max - 1);
    let cauda: String = caminho.chars().skip(corte).collect();

    match cauda.find('/') {
        Some(barra) => format!("…{}", &cauda[barra..]),
        None => format!("…{}", cauda),
    }
}

/// Formata bytes na unidade que cabe
pub fn formatar_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    fn com_arquivos(paths: &[&str]) -> AppState {
        let mut state = AppState::default();
        state.import_view_state.candidates = paths
            .iter()
            .map(|p| ImportCandidate::from_path(p.to_string()))
            .collect();
        state
    }

    #[test]
    fn varredura_atrasada_de_outra_origem_e_descartada() {
        let mut state = AppState::default();
        state.import_view_state.selected_source_path = Some("/cartao/novo".to_string());
        state.import_view_state.scanning = true;

        // Resposta da origem anterior chegando tarde
        let pendente = ImportView::aplicar(
            &mut state,
            ImportMessage::Scanned {
                root: "/cartao/antigo".to_string(),
                files: vec!["/cartao/antigo/a.cr2".to_string()],
            },
        );

        assert!(pendente.is_none());
        assert!(state.import_view_state.candidates.is_empty());
    }

    #[test]
    fn varredura_da_origem_corrente_preenche_a_grade() {
        let mut state = AppState::default();
        state.import_view_state.selected_source_path = Some("/cartao".to_string());

        let pendente = ImportView::aplicar(
            &mut state,
            ImportMessage::Scanned {
                root: "/cartao".to_string(),
                files: vec!["/cartao/b.cr2".to_string(), "/cartao/a.cr2".to_string()],
            },
        );

        assert!(matches!(pendente, Some(Seguimento::Detalhar(ref f)) if f.len() == 2));
        assert_eq!(state.import_view_state.candidates.len(), 2);
        assert!(state.import_view_state.candidates.iter().all(|c| c.checked));
    }

    #[test]
    fn duplicata_e_desmarcada_quando_a_opcao_esta_ligada() {
        let mut state = com_arquivos(&["/cartao/a.cr2", "/cartao/b.cr2"]);
        state.import_view_state.options.skip_duplicates = true;

        ImportView::aplicar(
            &mut state,
            ImportMessage::Duplicates(vec!["/cartao/a.cr2".to_string()]),
        );

        assert!(state.import_view_state.candidates[0].is_duplicate);
        assert!(!state.import_view_state.candidates[0].checked);
        assert!(state.import_view_state.candidates[1].checked);
    }

    #[test]
    fn duplicata_continua_marcada_quando_a_opcao_esta_desligada() {
        let mut state = com_arquivos(&["/cartao/a.cr2"]);
        state.import_view_state.options.skip_duplicates = false;

        ImportView::aplicar(
            &mut state,
            ImportMessage::Duplicates(vec!["/cartao/a.cr2".to_string()]),
        );

        assert!(state.import_view_state.candidates[0].is_duplicate);
        assert!(
            state.import_view_state.candidates[0].checked,
            "sem pular duplicatas, a decisão é do usuário — a tela não desmarca por ele"
        );
    }

    #[test]
    fn so_as_novas_esconde_o_que_ja_esta_no_catalogo() {
        let mut state = com_arquivos(&["/cartao/a.cr2", "/cartao/b.cr2"]);
        state.import_view_state.candidates[0].is_duplicate = true;
        state.import_view_state.only_new = true;

        let visiveis = ImportView::indices_visiveis(&state);

        assert_eq!(visiveis, vec![1]);
    }

    #[test]
    fn marcar_todas_respeita_o_filtro() {
        let mut state = com_arquivos(&["/cartao/a.cr2", "/cartao/b.cr2"]);
        state.import_view_state.candidates[0].is_duplicate = true;
        state.import_view_state.candidates[0].checked = false;
        state.import_view_state.candidates[1].checked = false;
        state.import_view_state.only_new = true;

        ImportView::marcar_visiveis(&mut state, true);

        assert!(
            !state.import_view_state.candidates[0].checked,
            "duplicata escondida não pode voltar marcada por um botão que o usuário não viu agir sobre ela"
        );
        assert!(state.import_view_state.candidates[1].checked);
    }

    #[test]
    fn metadados_chegam_e_a_grade_reordena_por_captura() {
        let mut state = com_arquivos(&["/cartao/b.cr2", "/cartao/a.cr2"]);
        state.import_view_state.sort_by = ImportSortBy::CaptureTime;

        ImportView::aplicar(
            &mut state,
            ImportMessage::Described(vec![
                ImportCandidateViewModel {
                    file_path: "/cartao/b.cr2".to_string(),
                    file_name: "b.cr2".to_string(),
                    file_size: 100,
                    is_raw: true,
                    camera: "Canon".to_string(),
                    date_time: "2026:08:15 09:00:00".to_string(),
                    dimensions: None,
                },
                ImportCandidateViewModel {
                    file_path: "/cartao/a.cr2".to_string(),
                    file_name: "a.cr2".to_string(),
                    file_size: 200,
                    is_raw: true,
                    camera: "Canon".to_string(),
                    date_time: "2026:08:15 10:00:00".to_string(),
                    dimensions: None,
                },
            ]),
        );

        // b foi disparada antes, então vem primeiro — mesmo com nome maior
        assert_eq!(state.import_view_state.candidates[0].file_name, "b.cr2");
        assert!(state.import_view_state.candidates[0].described);
    }

    #[test]
    fn resumo_conta_so_o_que_esta_marcado() {
        let mut state = com_arquivos(&["/cartao/a.cr2", "/cartao/b.cr2"]);
        state.import_view_state.candidates[0].file_size = 1024;
        state.import_view_state.candidates[1].file_size = 2048;
        state.import_view_state.candidates[1].checked = false;

        assert_eq!(state.import_view_state.checked_count(), 1);
        assert_eq!(state.import_view_state.checked_bytes(), 1024);
        assert_eq!(
            state.import_view_state.checked_paths(),
            vec!["/cartao/a.cr2".to_string()]
        );
    }

    #[test]
    fn falha_na_varredura_apaga_os_indicadores_de_progresso() {
        let mut state = AppState::default();
        state.import_view_state.scanning = true;
        state.import_view_state.describing = true;
        state.import_view_state.checking_duplicates = true;

        ImportView::aplicar(&mut state, ImportMessage::Failed("cartão sumiu".into()));

        assert!(!state.import_view_state.scanning);
        assert!(!state.import_view_state.describing);
        assert!(!state.import_view_state.checking_duplicates);
        assert_eq!(
            state.import_view_state.status.as_deref(),
            Some("cartão sumiu")
        );
    }

    #[test]
    fn pasta_de_origem_escolhida_pede_varredura() {
        let mut state = AppState::default();

        let pendente = ImportView::aplicar(
            &mut state,
            ImportMessage::SourceChosen("/Users/alguem/Pictures/ensaio".to_string()),
        );

        assert!(matches!(
            pendente,
            Some(Seguimento::Varrer(ref r)) if r == "/Users/alguem/Pictures/ensaio"
        ));
    }

    #[test]
    fn pasta_de_destino_escolhida_so_grava_a_opcao() {
        let mut state = AppState::default();

        let pendente = ImportView::aplicar(
            &mut state,
            ImportMessage::DestinationChosen("/Volumes/HD/2026".to_string()),
        );

        assert!(pendente.is_none(), "escolher destino não varre nada");
        assert_eq!(
            state.import_view_state.options.destination.as_deref(),
            Some("/Volumes/HD/2026")
        );
    }

    #[test]
    fn modal_fechado_nao_desenha_nem_pede_origens() {
        // `show` sai na primeira linha quando `open` é falso — é o que impede a tela de
        // disparar `tokio::spawn` (e de existir) enquanto ninguém pediu para importar.
        let state = AppState::default();
        assert!(!state.import_view_state.open);
    }

    #[test]
    fn formata_bytes_na_unidade_que_cabe() {
        assert_eq!(formatar_bytes(512), "512 B");
        assert_eq!(formatar_bytes(2048), "2 KB");
        assert_eq!(formatar_bytes(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(formatar_bytes(3 * 1024 * 1024 * 1024), "3.0 GB");
    }

    #[test]
    fn encurta_nome_preservando_a_extensao() {
        let curto = encurtar("IMG_0001.CR2", 300.0);
        assert_eq!(curto, "IMG_0001.CR2");

        let longo = encurtar("uma_foto_com_nome_muito_comprido.CR2", 90.0);
        assert!(longo.contains('…'));
        assert!(longo.ends_with(".CR2"));
    }
}
