// Theme Selector
// Provides theme variants using egui's built-in Visuals

use egui::{Context, Visuals, Color32, Stroke, style::Widgets};

/// Available theme variants for the application
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeVariant {
    /// Custom VintageLightbox dark theme (default)
    VintageDark,
    /// Dark gray theme with purple accents
    MochaDark,
    /// Dark blue theme
    MacchiatoDark,
    /// Warm dark theme
    FrappeDark,
    /// Light theme
    LatteLight,
}

impl ThemeVariant {
    /// Apply this theme to the egui context
    pub fn apply_to_context(&self, ctx: &Context) {
        match self {
            Self::VintageDark => {
                // Use the custom theme from theme.rs
                super::theme::Theme::apply_to_context(ctx);
            }
            Self::MochaDark => {
                ctx.set_visuals(Self::mocha_dark());
            }
            Self::MacchiatoDark => {
                ctx.set_visuals(Self::macchiato_dark());
            }
            Self::FrappeDark => {
                ctx.set_visuals(Self::frappe_dark());
            }
            Self::LatteLight => {
                ctx.set_visuals(Self::latte_light());
            }
        }
    }

    /// Get all available theme variants
    pub fn all() -> Vec<Self> {
        vec![
            Self::VintageDark,
            Self::MochaDark,
            Self::MacchiatoDark,
            Self::FrappeDark,
            Self::LatteLight,
        ]
    }

    /// Get a user-friendly display name for this theme
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::VintageDark => "Vintage Dark",
            Self::MochaDark => "Mocha Dark",
            Self::MacchiatoDark => "Macchiato Dark",
            Self::FrappeDark => "Frappe Dark",
            Self::LatteLight => "Latte Light",
        }
    }

    // Theme implementations using egui Visuals

    fn mocha_dark() -> Visuals {
        let mut visuals = Visuals::dark();
        // Catppuccin Mocha colors
        visuals.widgets = Widgets {
            noninteractive: egui::style::WidgetVisuals {
                bg_fill: Color32::from_rgb(30, 30, 46), // Base
                weak_bg_fill: Color32::from_rgb(24, 24, 37), // Mantle
                bg_stroke: Stroke::new(1.0, Color32::from_rgb(88, 91, 112)), // Surface1
                fg_stroke: Stroke::new(1.0, Color32::from_rgb(205, 214, 244)), // Text
                ..visuals.widgets.noninteractive
            },
            inactive: egui::style::WidgetVisuals {
                bg_fill: Color32::from_rgb(49, 50, 68), // Surface0
                weak_bg_fill: Color32::from_rgb(24, 24, 37),
                bg_stroke: Stroke::new(1.0, Color32::from_rgb(108, 112, 134)),
                fg_stroke: Stroke::new(1.0, Color32::from_rgb(205, 214, 244)),
                ..visuals.widgets.inactive
            },
            hovered: egui::style::WidgetVisuals {
                bg_fill: Color32::from_rgb(69, 71, 90), // Surface1
                weak_bg_fill: Color32::from_rgb(49, 50, 68),
                bg_stroke: Stroke::new(1.0, Color32::from_rgb(137, 180, 250)), // Blue
                fg_stroke: Stroke::new(1.5, Color32::from_rgb(205, 214, 244)),
                ..visuals.widgets.hovered
            },
            active: egui::style::WidgetVisuals {
                bg_fill: Color32::from_rgb(137, 180, 250), // Blue
                weak_bg_fill: Color32::from_rgb(49, 50, 68),
                bg_stroke: Stroke::new(1.0, Color32::from_rgb(116, 199, 236)), // Sky
                fg_stroke: Stroke::new(2.0, Color32::from_rgb(30, 30, 46)),
                ..visuals.widgets.active
            },
            ..visuals.widgets
        };
        visuals.selection.bg_fill = Color32::from_rgb(137, 180, 250); // Blue
        visuals.selection.stroke = Stroke::new(1.0, Color32::from_rgb(205, 214, 244));
        visuals
    }

    fn macchiato_dark() -> Visuals {
        let mut visuals = Visuals::dark();
        // Catppuccin Macchiato colors (blue-tinted)
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(36, 39, 58); // Base
        visuals.widgets.noninteractive.weak_bg_fill = Color32::from_rgb(30, 32, 48); // Mantle
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(202, 211, 245)); // Text
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(54, 58, 79); // Surface0
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(91, 96, 120); // Overlay0
        visuals.widgets.active.bg_fill = Color32::from_rgb(138, 173, 244); // Blue
        visuals.selection.bg_fill = Color32::from_rgb(138, 173, 244);
        visuals
    }

    fn frappe_dark() -> Visuals {
        let mut visuals = Visuals::dark();
        // Catppuccin Frappe colors (warm gray)
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(48, 52, 70); // Base
        visuals.widgets.noninteractive.weak_bg_fill = Color32::from_rgb(41, 44, 60); // Mantle
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(198, 208, 245)); // Text
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(65, 69, 89); // Surface0
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(98, 104, 128); // Overlay0
        visuals.widgets.active.bg_fill = Color32::from_rgb(140, 170, 238); // Blue
        visuals.selection.bg_fill = Color32::from_rgb(140, 170, 238);
        visuals
    }

    fn latte_light() -> Visuals {
        let mut visuals = Visuals::light();
        // Catppuccin Latte colors (light theme)
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(239, 241, 245); // Base
        visuals.widgets.noninteractive.weak_bg_fill = Color32::from_rgb(230, 233, 239); // Mantle
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(76, 79, 105)); // Text
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(220, 224, 232); // Surface0
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(188, 192, 204); // Overlay0
        visuals.widgets.active.bg_fill = Color32::from_rgb(30, 102, 245); // Blue
        visuals.widgets.active.fg_stroke = Stroke::new(2.0, Color32::WHITE);
        visuals.selection.bg_fill = Color32::from_rgb(30, 102, 245);
        visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
        visuals
    }
}

impl Default for ThemeVariant {
    fn default() -> Self {
        Self::VintageDark
    }
}
