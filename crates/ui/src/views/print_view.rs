// Print View
// Photo printing view with layout templates, page setup, and print preview
// Styled like Adobe Lightroom's Print module

use egui::Ui;
use crate::state::AppState;
use crate::design_system::theme::Theme;
use crate::design_system::{icons, widgets};
use crate::components::filmstrip::Filmstrip;
use std::sync::Arc;
use infrastructure::cache::preview_manager::PreviewManager;

/// Print layout template
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintTemplate {
    Single,
    Grid2x2,
    Grid3x3,
    Grid4x4,
    ContactSheet,
    Custom,
}

impl PrintTemplate {
    pub fn display_name(&self) -> &'static str {
        match self {
            PrintTemplate::Single => "Single Image",
            PrintTemplate::Grid2x2 => "2 × 2",
            PrintTemplate::Grid3x3 => "3 × 3",
            PrintTemplate::Grid4x4 => "4 × 4",
            PrintTemplate::ContactSheet => "Contact Sheet",
            PrintTemplate::Custom => "Custom",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            PrintTemplate::Single => icons::VIEW_DETAIL,
            PrintTemplate::Grid2x2 | PrintTemplate::Grid3x3 | PrintTemplate::Grid4x4 => icons::VIEW_GRID,
            PrintTemplate::ContactSheet => icons::VIEW_LIST,
            PrintTemplate::Custom => icons::ACTION_SETTINGS,
        }
    }

    pub fn all() -> &'static [PrintTemplate] {
        &[
            PrintTemplate::Single,
            PrintTemplate::Grid2x2,
            PrintTemplate::Grid3x3,
            PrintTemplate::Grid4x4,
            PrintTemplate::ContactSheet,
            PrintTemplate::Custom,
        ]
    }
    
    /// Get the number of cells (columns × rows)
    pub fn grid_dimensions(&self) -> (u8, u8) {
        match self {
            PrintTemplate::Single => (1, 1),
            PrintTemplate::Grid2x2 => (2, 2),
            PrintTemplate::Grid3x3 => (3, 3),
            PrintTemplate::Grid4x4 => (4, 4),
            PrintTemplate::ContactSheet => (4, 6), // 24 photos per page
            PrintTemplate::Custom => (2, 2), // Default for custom
        }
    }
    
    pub fn photos_per_page(&self) -> usize {
        let (cols, rows) = self.grid_dimensions();
        (cols as usize) * (rows as usize)
    }
}

/// Paper size options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperSize {
    A4,
    A3,
    Letter,
    Legal,
    Tabloid,
}

impl PaperSize {
    pub fn display_name(&self) -> &'static str {
        match self {
            PaperSize::A4 => "A4 (210 × 297 mm)",
            PaperSize::A3 => "A3 (297 × 420 mm)",
            PaperSize::Letter => "Letter (8.5 × 11\")",
            PaperSize::Legal => "Legal (8.5 × 14\")",
            PaperSize::Tabloid => "Tabloid (11 × 17\")",
        }
    }

    pub fn all() -> &'static [PaperSize] {
        &[
            PaperSize::A4,
            PaperSize::A3,
            PaperSize::Letter,
            PaperSize::Legal,
            PaperSize::Tabloid,
        ]
    }
    
    /// Get dimensions in mm
    pub fn dimensions_mm(&self) -> (f32, f32) {
        match self {
            PaperSize::A4 => (210.0, 297.0),
            PaperSize::A3 => (297.0, 420.0),
            PaperSize::Letter => (215.9, 279.4),
            PaperSize::Legal => (215.9, 355.6),
            PaperSize::Tabloid => (279.4, 431.8),
        }
    }
}

/// Paper orientation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Portrait,
    Landscape,
}

impl Orientation {
    pub fn display_name(&self) -> &'static str {
        match self {
            Orientation::Portrait => "Portrait",
            Orientation::Landscape => "Landscape",
        }
    }
}

/// Print view state
#[derive(Debug, Clone)]
pub struct PrintViewState {
    pub selected_template: PrintTemplate,
    pub paper_size: PaperSize,
    pub orientation: Orientation,
    pub margin_mm: f32,
    pub cell_spacing_mm: f32,
    pub include_filename: bool,
    pub include_date: bool,
    pub include_camera: bool,
    pub include_exposure: bool,
    pub copies: u8,
    pub photo_ids: Vec<String>,
    pub custom_rows: u8,
    pub custom_cols: u8,
    /// Use all filmstrip photos (auto-select)
    pub use_all_photos: bool,
    /// Per-cell pan offsets (cell_index -> offset as fraction of cell size)
    pub cell_offsets: std::collections::HashMap<usize, egui::Vec2>,
    /// Currently dragging cell index
    pub dragging_cell: Option<usize>,
}

impl Default for PrintViewState {
    fn default() -> Self {
        Self {
            selected_template: PrintTemplate::Single,
            paper_size: PaperSize::A4,
            orientation: Orientation::Portrait,
            margin_mm: 10.0,
            cell_spacing_mm: 5.0,
            include_filename: false,
            include_date: false,
            include_camera: false,
            include_exposure: false,
            copies: 1,
            photo_ids: Vec::new(),
            custom_rows: 2,
            custom_cols: 2,
            use_all_photos: true,
            cell_offsets: std::collections::HashMap::new(),
            dragging_cell: None,
        }
    }
}

impl PrintViewState {
    pub fn new(photo_ids: Vec<String>) -> Self {
        Self {
            photo_ids,
            ..Default::default()
        }
    }
    
    /// Calculate page count based on selected photos and template
    pub fn page_count(&self) -> usize {
        if self.photo_ids.is_empty() {
            return 0;
        }
        let per_page = self.selected_template.photos_per_page().max(1);
        self.photo_ids.len().div_ceil(per_page)
    }
}

pub struct PrintView {
    filmstrip: Filmstrip,
    preview_manager: Arc<PreviewManager>,
    /// Cache of loaded textures for preview (photo_id -> TextureHandle)
    texture_cache: std::collections::HashMap<String, egui::TextureHandle>,
}

impl PrintView {
    pub fn new(preview_manager: Arc<PreviewManager>) -> Self {
        Self {
            filmstrip: Filmstrip::new(preview_manager.clone()),
            preview_manager,
            texture_cache: std::collections::HashMap::new(),
        }
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        state: &mut AppState,
        photo_controller: &std::sync::Arc<adapters::controllers::PhotoController>,
        library_controller: &std::sync::Arc<adapters::controllers::LibraryController>,
        photo_sender: &tokio::sync::mpsc::Sender<Result<Vec<adapters::view_models::PhotoViewModel>, String>>,
        ctx: &egui::Context,
    ) {
        // Initialize print state if needed
        if state.print_view_state.is_none() {
            let photo_ids: Vec<String> = if !state.selected_photo_ids.is_empty() {
                state.selected_photo_ids.iter().cloned().collect()
            } else if let Some(id) = &state.library_selected_photo_id {
                vec![id.clone()]
            } else {
                vec![]
            };
            state.print_view_state = Some(PrintViewState::new(photo_ids));
        }

        // Bottom filmstrip for photo selection
        egui::TopBottomPanel::bottom("filmstrip_print")
            .exact_height(120.0)
            .show_inside(ui, |ui| {
                self.show_filmstrip(ui, state, photo_controller, library_controller, photo_sender, ctx);
            });

        // Left sidebar - Template Browser
        egui::SidePanel::left("print_left")
            .resizable(false)
            .exact_width(Theme::SIDEBAR_WIDTH)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.show_template_browser(ui, state);
                });
            });

        // Right sidebar - Page Setup & Print Settings
        egui::SidePanel::right("print_right")
            .resizable(false)
            .exact_width(Theme::PANEL_WIDTH)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.show_print_settings(ui, state);
                    });
            });

        // Center - Print Preview
        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.show_print_preview(ui, state, ctx);
        });
    }

    fn show_filmstrip(
        &mut self,
        ui: &mut Ui,
        state: &mut AppState,
        photo_controller: &std::sync::Arc<adapters::controllers::PhotoController>,
        library_controller: &std::sync::Arc<adapters::controllers::LibraryController>,
        photo_sender: &tokio::sync::mpsc::Sender<Result<Vec<adapters::view_models::PhotoViewModel>, String>>,
        ctx: &egui::Context,
    ) {
        // Selection toolbar at top of filmstrip area
        ui.horizontal(|ui| {
            ui.add_space(Theme::SPACE_SM);
            
            // "Use All Photos" checkbox
            if let Some(ref mut print_state) = state.print_view_state {
                if ui.checkbox(&mut print_state.use_all_photos, "Use All Photos").changed() && print_state.use_all_photos {
                    // Auto-populate with all filtered photos
                    print_state.photo_ids = state.filmstrip_filter.apply(&state.photos)
                        .iter()
                        .map(|p| p.id.clone())
                        .collect();
                }
            }
            
            ui.add_space(Theme::SPACE_MD);
            ui.separator();
            ui.add_space(Theme::SPACE_MD);
            
            // Selection buttons
            if ui.small_button("Select All").clicked() {
                if let Some(ref mut print_state) = state.print_view_state {
                    print_state.photo_ids = state.filmstrip_filter.apply(&state.photos)
                        .iter()
                        .map(|p| p.id.clone())
                        .collect();
                    print_state.use_all_photos = false; // Manual selection mode
                }
            }
            
            ui.add_space(Theme::SPACE_XS);
            
            if ui.small_button("Select None").clicked() {
                if let Some(ref mut print_state) = state.print_view_state {
                    print_state.photo_ids.clear();
                    print_state.use_all_photos = false;
                }
            }
            
            ui.add_space(Theme::SPACE_XS);
            
            if ui.small_button("Flagged Only").clicked() {
                if let Some(ref mut print_state) = state.print_view_state {
                    print_state.photo_ids = state.filmstrip_filter.apply(&state.photos)
                        .iter()
                        .filter(|p| p.flag == Some(1)) // Pick flag
                        .map(|p| p.id.clone())
                        .collect();
                    print_state.use_all_photos = false;
                }
            }
            
            ui.add_space(Theme::SPACE_XS);
            
            if ui.small_button("Invert").clicked() {
                if let Some(ref mut print_state) = state.print_view_state {
                    let all_ids: std::collections::HashSet<String> = state.filmstrip_filter.apply(&state.photos)
                        .iter()
                        .map(|p| p.id.clone())
                        .collect();
                    let selected: std::collections::HashSet<String> = print_state.photo_ids.iter().cloned().collect();
                    print_state.photo_ids = all_ids.difference(&selected).cloned().collect();
                    print_state.use_all_photos = false;
                }
            }
            
            ui.add_space(Theme::SPACE_MD);
            
            // Selection count
            if let Some(ref print_state) = state.print_view_state {
                let total = state.filmstrip_filter.apply(&state.photos).len();
                ui.label(
                    egui::RichText::new(format!("{}/{} selected", print_state.photo_ids.len(), total))
                        .size(Theme::FONT_SM)
                        .color(Theme::TEXT_SECONDARY)
                );
            }
        });
        
        ui.add_space(Theme::SPACE_XS);
        
        // Get print state photo_ids for highlighting
        let _print_photo_ids: std::collections::HashSet<String> = state.print_view_state
            .as_ref()
            .map(|ps| ps.photo_ids.iter().cloned().collect())
            .unwrap_or_default();
        
        let selected_id = state.library_selected_photo_id.clone();
        
        // Filmstrip with selection indicators
        self.filmstrip.show_develop(
            ui,
            ctx,
            &state.photos,
            &selected_id,
            &mut state.filmstrip_filter,
            |photo_id| {
                // Toggle photo in print collection
                if let Some(ref mut print_state) = state.print_view_state {
                    if print_state.photo_ids.contains(&photo_id) {
                        print_state.photo_ids.retain(|id| id != &photo_id);
                    } else {
                        print_state.photo_ids.push(photo_id.clone());
                    }
                    print_state.use_all_photos = false; // Switch to manual mode
                }
                state.library_selected_photo_id = Some(photo_id);
                ctx.request_repaint();
            },
            |photo_id, flag_code| {
                // Handle flag
                let controller = photo_controller.clone();
                let library_controller = library_controller.clone();
                let photo_sender = photo_sender.clone();
                let ctx_clone = ctx.clone();
                
                tokio::spawn(async move {
                    let _ = controller.set_flag(&photo_id, flag_code).await;
                    if let Ok(photos) = library_controller.get_all_photos().await {
                        let _ = photo_sender.send(Ok(photos)).await;
                    }
                    ctx_clone.request_repaint();
                });
            }
        );
    }

    fn show_template_browser(&self, ui: &mut Ui, state: &mut AppState) {
        widgets::section_title(ui, "Template Browser");
        ui.add_space(Theme::SPACE_MD);

        if let Some(ref mut print_state) = state.print_view_state {
            for template in PrintTemplate::all() {
                let is_selected = print_state.selected_template == *template;
                let text = format!("{} {}", template.icon(), template.display_name());
                
                if widgets::menu_item(ui, &text, is_selected).clicked() {
                    print_state.selected_template = *template;
                }
            }
        }

        ui.add_space(Theme::SPACE_LG);

        // Custom Grid Settings (only shown when Custom is selected)
        if let Some(ref mut print_state) = state.print_view_state {
            if print_state.selected_template == PrintTemplate::Custom {
                widgets::section_title(ui, "Custom Grid");
                ui.add_space(Theme::SPACE_SM);

                ui.horizontal(|ui| {
                    ui.label("Columns:");
                    ui.add(egui::DragValue::new(&mut print_state.custom_cols)
                        .range(1..=6)
                        .speed(1));
                });

                ui.add_space(Theme::SPACE_XS);

                ui.horizontal(|ui| {
                    ui.label("Rows:");
                    ui.add(egui::DragValue::new(&mut print_state.custom_rows)
                        .range(1..=8)
                        .speed(1));
                });
            }
        }

        ui.add_space(Theme::SPACE_LG);

        // Collection Info
        widgets::section_title(ui, "Print Collection");
        ui.add_space(Theme::SPACE_SM);

        if let Some(ref print_state) = state.print_view_state {
            ui.label(format!("{} photos selected", print_state.photo_ids.len()));
            ui.label(format!("{} page(s)", print_state.page_count()));
        }

        ui.add_space(Theme::SPACE_MD);

        // Clear selection button
        if widgets::secondary_button(ui, "Clear Selection").clicked() {
            if let Some(ref mut print_state) = state.print_view_state {
                print_state.photo_ids.clear();
            }
        }

        ui.add_space(Theme::SPACE_SM);

        // Add all filtered photos
        if widgets::secondary_button(ui, "Add All Photos").clicked() {
            let filtered_ids: Vec<String> = state.filmstrip_filter.apply(&state.photos)
                .iter()
                .map(|p| p.id.clone())
                .collect();
            if let Some(ref mut print_state) = state.print_view_state {
                print_state.photo_ids = filtered_ids;
            }
        }
    }

    fn show_print_settings(&self, ui: &mut Ui, state: &mut AppState) {
        // Page Setup Section
        widgets::section_title(ui, "Page Setup");
        ui.add_space(Theme::SPACE_SM);

        if let Some(ref mut print_state) = state.print_view_state {
            // Paper Size
            egui::ComboBox::from_label("Paper Size")
                .selected_text(print_state.paper_size.display_name())
                .show_ui(ui, |ui| {
                    for size in PaperSize::all() {
                        ui.selectable_value(&mut print_state.paper_size, *size, size.display_name());
                    }
                });

            ui.add_space(Theme::SPACE_SM);

            // Orientation
            ui.horizontal(|ui| {
                ui.label("Orientation:");
                ui.selectable_value(&mut print_state.orientation, Orientation::Portrait, "Portrait");
                ui.selectable_value(&mut print_state.orientation, Orientation::Landscape, "Landscape");
            });

            ui.add_space(Theme::SPACE_SM);

            // Margins
            ui.horizontal(|ui| {
                ui.label("Margins (mm):");
                ui.add(egui::DragValue::new(&mut print_state.margin_mm)
                    .range(0.0..=50.0)
                    .speed(0.5));
            });

            ui.add_space(Theme::SPACE_SM);

            // Cell Spacing
            ui.horizontal(|ui| {
                ui.label("Cell Spacing (mm):");
                ui.add(egui::DragValue::new(&mut print_state.cell_spacing_mm)
                    .range(0.0..=20.0)
                    .speed(0.5));
            });
        }

        ui.add_space(Theme::SPACE_LG);
        ui.separator();
        ui.add_space(Theme::SPACE_MD);

        // Photo Info Section
        widgets::section_title(ui, "Photo Info");
        ui.add_space(Theme::SPACE_SM);

        if let Some(ref mut print_state) = state.print_view_state {
            ui.checkbox(&mut print_state.include_filename, "Filename");
            ui.checkbox(&mut print_state.include_date, "Date");
            ui.checkbox(&mut print_state.include_camera, "Camera");
            ui.checkbox(&mut print_state.include_exposure, "Exposure Settings");
        }

        ui.add_space(Theme::SPACE_LG);
        ui.separator();
        ui.add_space(Theme::SPACE_MD);

        // Print Job Section
        widgets::section_title(ui, "Print Job");
        ui.add_space(Theme::SPACE_SM);

        if let Some(ref mut print_state) = state.print_view_state {
            ui.horizontal(|ui| {
                ui.label("Copies:");
                ui.add(egui::DragValue::new(&mut print_state.copies)
                    .range(1..=99)
                    .speed(1));
            });
            
            ui.add_space(Theme::SPACE_SM);
            
            // Reset photo positions button
            if !print_state.cell_offsets.is_empty() && ui.small_button("Reset Photo Positions").clicked() {
                print_state.cell_offsets.clear();
            }
        }

        ui.add_space(Theme::SPACE_LG);

        // Print Button
        let print_label = format!("{} Print", icons::NAV_PRINT);
        let can_print = state.print_view_state.as_ref()
            .map(|s| !s.photo_ids.is_empty())
            .unwrap_or(false);
        
        ui.add_enabled_ui(can_print, |ui| {
            if widgets::primary_button(ui, &print_label).clicked() {
                if let Some(ref print_state) = state.print_view_state {
                    state.toasts.info(format!(
                        "Print feature coming soon! Would print {} pages with {} template",
                        print_state.page_count(),
                        print_state.selected_template.display_name()
                    ));
                }
            }
        });

        ui.add_space(Theme::SPACE_SM);

        // Export PDF Button
        let export_label = format!("{} Export PDF", icons::ACTION_EXPORT);
        ui.add_enabled_ui(can_print, |ui| {
            if widgets::secondary_button(ui, &export_label).clicked() {
                state.toasts.info("PDF export coming soon!");
            }
        });
    }

    fn show_print_preview(&mut self, ui: &mut Ui, state: &mut AppState, ctx: &egui::Context) {
        let available = ui.available_size();
        
        // Draw paper preview background
        let preview_rect = egui::Rect::from_center_size(
            ui.available_rect_before_wrap().center(),
            egui::vec2(available.x * 0.8, available.y * 0.9)
        );

        // Paper shadow
        ui.painter().rect_filled(
            preview_rect.translate(egui::vec2(4.0, 4.0)),
            8.0,
            egui::Color32::from_black_alpha(40),
        );

        // Paper background
        ui.painter().rect_filled(
            preview_rect,
            4.0,
            egui::Color32::WHITE,
        );

        // Paper border
        ui.painter().rect_stroke(
            preview_rect,
            4.0,
            egui::Stroke::new(1.0, egui::Color32::from_gray(180)),
            egui::epaint::StrokeKind::Middle,
        );

        // Draw grid cells based on template
        let updates = if let Some(ref print_state) = state.print_view_state {
            let (cols, rows) = if print_state.selected_template == PrintTemplate::Custom {
                (print_state.custom_cols, print_state.custom_rows)
            } else {
                print_state.selected_template.grid_dimensions()
            };

            // Calculate margins proportionally
            let margin_ratio = print_state.margin_mm / 297.0; // Relative to A4 height
            let spacing_ratio = print_state.cell_spacing_mm / 297.0;

            let paper_inner = preview_rect.shrink(preview_rect.height() * margin_ratio);
            let cell_width = (paper_inner.width() - (cols as f32 - 1.0) * preview_rect.height() * spacing_ratio) / cols as f32;
            let cell_height = (paper_inner.height() - (rows as f32 - 1.0) * preview_rect.height() * spacing_ratio) / rows as f32;

            // Clone photo_ids and cell_offsets to avoid borrow issues
            let photo_ids = print_state.photo_ids.clone();
            let cell_offsets = print_state.cell_offsets.clone();
            let mut drag_updates: Vec<(usize, egui::Vec2)> = Vec::new();

            for row in 0..rows {
                for col in 0..cols {
                    let x = paper_inner.left() + col as f32 * (cell_width + preview_rect.height() * spacing_ratio);
                    let y = paper_inner.top() + row as f32 * (cell_height + preview_rect.height() * spacing_ratio);
                    
                    let cell_rect = egui::Rect::from_min_size(
                        egui::pos2(x, y),
                        egui::vec2(cell_width, cell_height)
                    );

                    // Cell background
                    ui.painter().rect_filled(
                        cell_rect,
                        2.0,
                        egui::Color32::from_gray(240),
                    );

                    // Cell border
                    ui.painter().rect_stroke(
                        cell_rect,
                        2.0,
                        egui::Stroke::new(1.0, egui::Color32::from_gray(200)),
                        egui::epaint::StrokeKind::Middle,
                    );

                    // Photo content
                    let idx = row as usize * cols as usize + col as usize;
                    if idx < photo_ids.len() {
                        let photo_id = &photo_ids[idx];
                        
                        // Check if we already have this texture cached
                        if !self.texture_cache.contains_key(photo_id) {
                            // Try to load thumbnail from cache
                            if let Some(img) = self.preview_manager.get_thumbnail(photo_id) {
                                // Convert DynamicImage to RGBA bytes
                                let rgba = img.to_rgba8();
                                let (width, height) = rgba.dimensions();
                                let pixels: Vec<u8> = rgba.into_raw();
                                
                                // Create egui ColorImage
                                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                                    [width as usize, height as usize],
                                    &pixels,
                                );
                                
                                let texture_id = format!("print_preview_{}", photo_id);
                                let texture = ctx.load_texture(
                                    &texture_id,
                                    color_image,
                                    egui::TextureOptions::LINEAR,
                                );
                                self.texture_cache.insert(photo_id.clone(), texture);
                            }
                        }
                        
                        // Render the texture if available
                        if let Some(texture) = self.texture_cache.get(photo_id) {
                            // Calculate aspect-fit sizing
                            let tex_size = texture.size_vec2();
                            let cell_aspect = cell_rect.width() / cell_rect.height();
                            let tex_aspect = tex_size.x / tex_size.y;
                            
                            let (render_width, render_height) = if tex_aspect > cell_aspect {
                                // Texture is wider - fit to width
                                let w = cell_rect.width() * 0.95;
                                (w, w / tex_aspect)
                            } else {
                                // Texture is taller - fit to height
                                let h = cell_rect.height() * 0.95;
                                (h * tex_aspect, h)
                            };
                            
                            // Get offset for this cell
                            let offset = cell_offsets.get(&idx).copied().unwrap_or(egui::Vec2::ZERO);
                            
                            // Calculate max offset based on how much the image can move
                            let max_offset_x = (render_width - cell_rect.width() * 0.9).max(0.0) / 2.0 + render_width * 0.2;
                            let max_offset_y = (render_height - cell_rect.height() * 0.9).max(0.0) / 2.0 + render_height * 0.2;
                            
                            let clamped_offset = egui::vec2(
                                offset.x.clamp(-max_offset_x, max_offset_x),
                                offset.y.clamp(-max_offset_y, max_offset_y),
                            );
                            
                            let render_rect = egui::Rect::from_center_size(
                                cell_rect.center() + clamped_offset,
                                egui::vec2(render_width, render_height),
                            );
                            
                            // Clip to cell rect
                            ui.painter().with_clip_rect(cell_rect).image(
                                texture.id(),
                                render_rect,
                                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                egui::Color32::WHITE,
                            );
                            
                            // Handle drag interaction for positioning
                            let cell_id = ui.id().with(("print_cell", idx));
                            let response = ui.interact(cell_rect, cell_id, egui::Sense::drag());
                            
                            if response.dragged() {
                                let new_offset = offset + response.drag_delta();
                                drag_updates.push((idx, new_offset));
                            }
                            
                            // Show drag hint on hover
                            if response.hovered() {
                                ui.painter().rect_stroke(
                                    cell_rect.shrink(2.0),
                                    2.0,
                                    egui::Stroke::new(2.0, Theme::ACCENT_PRIMARY),
                                    egui::epaint::StrokeKind::Middle,
                                );
                            }
                        } else {
                            // Loading placeholder
                            ui.painter().text(
                                cell_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                icons::FILE_IMAGE,
                                egui::FontId::proportional(24.0),
                                egui::Color32::from_gray(150),
                            );
                        }
                    } else {
                        // Empty cell
                        ui.painter().text(
                            cell_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            icons::PLUS,
                            egui::FontId::proportional(20.0),
                            egui::Color32::from_gray(180),
                        );
                    }
                }
            }

            // Collect page count while still borrowing print_state
            let page_count = print_state.page_count();

            // Page indicator
            let page_text = format!("Page 1 of {}", page_count.max(1));
            ui.painter().text(
                egui::pos2(preview_rect.center().x, preview_rect.bottom() + 20.0),
                egui::Align2::CENTER_CENTER,
                page_text,
                egui::FontId::proportional(14.0),
                egui::Color32::from_gray(100),
            );
            
            // Return drag updates to apply after this scope ends
            drag_updates
        } else {
            Vec::new()
        };

        // Apply drag updates to state (outside the immutable borrow scope)
        if !updates.is_empty() {
            if let Some(ref mut ps) = state.print_view_state {
                for (idx, offset) in updates {
                    ps.cell_offsets.insert(idx, offset);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================
    // PrintTemplate Tests
    // ============================================

    #[test]
    fn test_print_template_display_names() {
        assert_eq!(PrintTemplate::Single.display_name(), "Single Image");
        assert_eq!(PrintTemplate::Grid2x2.display_name(), "2 × 2");
        assert_eq!(PrintTemplate::Grid3x3.display_name(), "3 × 3");
        assert_eq!(PrintTemplate::Grid4x4.display_name(), "4 × 4");
        assert_eq!(PrintTemplate::ContactSheet.display_name(), "Contact Sheet");
        assert_eq!(PrintTemplate::Custom.display_name(), "Custom");
    }

    #[test]
    fn test_print_template_grid_dimensions() {
        assert_eq!(PrintTemplate::Single.grid_dimensions(), (1, 1));
        assert_eq!(PrintTemplate::Grid2x2.grid_dimensions(), (2, 2));
        assert_eq!(PrintTemplate::Grid3x3.grid_dimensions(), (3, 3));
        assert_eq!(PrintTemplate::Grid4x4.grid_dimensions(), (4, 4));
        assert_eq!(PrintTemplate::ContactSheet.grid_dimensions(), (4, 6));
        assert_eq!(PrintTemplate::Custom.grid_dimensions(), (2, 2));
    }

    #[test]
    fn test_print_template_photos_per_page() {
        assert_eq!(PrintTemplate::Single.photos_per_page(), 1);
        assert_eq!(PrintTemplate::Grid2x2.photos_per_page(), 4);
        assert_eq!(PrintTemplate::Grid3x3.photos_per_page(), 9);
        assert_eq!(PrintTemplate::Grid4x4.photos_per_page(), 16);
        assert_eq!(PrintTemplate::ContactSheet.photos_per_page(), 24);
        assert_eq!(PrintTemplate::Custom.photos_per_page(), 4);
    }

    #[test]
    fn test_print_template_all_returns_all_variants() {
        let all = PrintTemplate::all();
        assert_eq!(all.len(), 6);
        assert!(all.contains(&PrintTemplate::Single));
        assert!(all.contains(&PrintTemplate::Grid2x2));
        assert!(all.contains(&PrintTemplate::Grid3x3));
        assert!(all.contains(&PrintTemplate::Grid4x4));
        assert!(all.contains(&PrintTemplate::ContactSheet));
        assert!(all.contains(&PrintTemplate::Custom));
    }

    #[test]
    fn test_print_template_icon_returns_non_empty() {
        for template in PrintTemplate::all() {
            assert!(!template.icon().is_empty());
        }
    }

    // ============================================
    // PaperSize Tests
    // ============================================

    #[test]
    fn test_paper_size_display_names() {
        assert!(PaperSize::A4.display_name().contains("210"));
        assert!(PaperSize::A3.display_name().contains("297"));
        assert!(PaperSize::Letter.display_name().contains("8.5"));
        assert!(PaperSize::Legal.display_name().contains("14"));
        assert!(PaperSize::Tabloid.display_name().contains("11"));
    }

    #[test]
    fn test_paper_size_dimensions_mm() {
        let (w, h) = PaperSize::A4.dimensions_mm();
        assert!((w - 210.0).abs() < 0.1);
        assert!((h - 297.0).abs() < 0.1);

        let (w, h) = PaperSize::Letter.dimensions_mm();
        assert!((w - 215.9).abs() < 0.1);
        assert!((h - 279.4).abs() < 0.1);
    }

    #[test]
    fn test_paper_size_all_returns_all_variants() {
        let all = PaperSize::all();
        assert_eq!(all.len(), 5);
    }

    // ============================================
    // Orientation Tests
    // ============================================

    #[test]
    fn test_orientation_display_names() {
        assert_eq!(Orientation::Portrait.display_name(), "Portrait");
        assert_eq!(Orientation::Landscape.display_name(), "Landscape");
    }

    // ============================================
    // PrintViewState Tests
    // ============================================

    #[test]
    fn test_print_view_state_default() {
        let state = PrintViewState::default();
        
        assert_eq!(state.selected_template, PrintTemplate::Single);
        assert_eq!(state.paper_size, PaperSize::A4);
        assert_eq!(state.orientation, Orientation::Portrait);
        assert!((state.margin_mm - 10.0).abs() < 0.1);
        assert!((state.cell_spacing_mm - 5.0).abs() < 0.1);
        assert!(!state.include_filename);
        assert!(!state.include_date);
        assert!(!state.include_camera);
        assert!(!state.include_exposure);
        assert_eq!(state.copies, 1);
        assert!(state.photo_ids.is_empty());
        assert_eq!(state.custom_rows, 2);
        assert_eq!(state.custom_cols, 2);
    }

    #[test]
    fn test_print_view_state_new_with_photo_ids() {
        let photo_ids = vec!["photo1".to_string(), "photo2".to_string(), "photo3".to_string()];
        let state = PrintViewState::new(photo_ids.clone());
        
        assert_eq!(state.photo_ids, photo_ids);
        assert_eq!(state.selected_template, PrintTemplate::Single);
    }

    #[test]
    fn test_print_view_state_page_count_empty() {
        let state = PrintViewState::default();
        assert_eq!(state.page_count(), 0);
    }

    #[test]
    fn test_print_view_state_page_count_single_template() {
        let mut state = PrintViewState::default();
        state.selected_template = PrintTemplate::Single;
        state.photo_ids = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        
        assert_eq!(state.page_count(), 3); // 1 photo per page = 3 pages
    }

    #[test]
    fn test_print_view_state_page_count_grid_2x2() {
        let mut state = PrintViewState::default();
        state.selected_template = PrintTemplate::Grid2x2;
        state.photo_ids = vec![
            "1".to_string(), "2".to_string(), "3".to_string(), 
            "4".to_string(), "5".to_string()
        ];
        
        assert_eq!(state.page_count(), 2); // 4 photos per page, 5 photos = 2 pages
    }

    #[test]
    fn test_print_view_state_page_count_grid_3x3() {
        let mut state = PrintViewState::default();
        state.selected_template = PrintTemplate::Grid3x3;
        state.photo_ids = vec![
            "1".to_string(), "2".to_string(), "3".to_string(),
            "4".to_string(), "5".to_string(), "6".to_string(),
            "7".to_string(), "8".to_string(), "9".to_string(),
            "10".to_string()
        ];
        
        assert_eq!(state.page_count(), 2); // 9 photos per page, 10 photos = 2 pages
    }

    #[test]
    fn test_print_view_state_page_count_contact_sheet() {
        let mut state = PrintViewState::default();
        state.selected_template = PrintTemplate::ContactSheet;
        
        // 24 photos per page for contact sheet
        let mut ids = Vec::new();
        for i in 1..=25 {
            ids.push(i.to_string());
        }
        state.photo_ids = ids;
        
        assert_eq!(state.page_count(), 2); // 24 per page, 25 photos = 2 pages
    }

    #[test]
    fn test_print_view_state_page_count_exact_fit() {
        let mut state = PrintViewState::default();
        state.selected_template = PrintTemplate::Grid2x2;
        state.photo_ids = vec![
            "1".to_string(), "2".to_string(), 
            "3".to_string(), "4".to_string()
        ];
        
        assert_eq!(state.page_count(), 1); // Exactly 4 photos = 1 page
    }

    #[test]
    fn test_print_view_state_page_count_single_photo() {
        let mut state = PrintViewState::default();
        state.selected_template = PrintTemplate::Grid4x4;
        state.photo_ids = vec!["1".to_string()];
        
        assert_eq!(state.page_count(), 1); // Even 1 photo needs 1 page
    }

    // ============================================
    // Integration-level Tests
    // ============================================

    #[test]
    fn test_all_templates_have_valid_photos_per_page() {
        for template in PrintTemplate::all() {
            let per_page = template.photos_per_page();
            assert!(per_page > 0, "Template {:?} has invalid photos_per_page", template);
            
            let (cols, rows) = template.grid_dimensions();
            assert_eq!(per_page, (cols as usize) * (rows as usize));
        }
    }

    #[test]
    fn test_paper_dimensions_are_positive() {
        for size in PaperSize::all() {
            let (w, h) = size.dimensions_mm();
            assert!(w > 0.0, "Paper size {:?} has invalid width", size);
            assert!(h > 0.0, "Paper size {:?} has invalid height", size);
        }
    }
}
