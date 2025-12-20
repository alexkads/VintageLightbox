// VintageLightbox Design System - Design Tokens
// Single source of truth for all visual design decisions
// Ported from Slint to egui

#![allow(dead_code)]

use egui::Color32;

pub struct Theme;

impl Theme {
    // ============================================
    // COLORS - Background
    // ============================================
    pub const BG_APP: Color32 = Color32::from_rgb(26, 26, 26);
    pub const BG_SURFACE: Color32 = Color32::from_rgb(37, 37, 37);
    pub const BG_ELEVATED: Color32 = Color32::from_rgb(45, 45, 45);
    pub const BG_HOVER: Color32 = Color32::from_rgb(53, 53, 53);
    pub const BG_ACTIVE: Color32 = Color32::from_rgb(58, 58, 58);
    pub const BG_OVERLAY: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 204); // #000000cc

    // ============================================
    // COLORS - Text
    // ============================================
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(224, 224, 224);
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(176, 176, 176);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(128, 128, 128);
    pub const TEXT_DISABLED: Color32 = Color32::from_rgb(96, 96, 96);
    pub const TEXT_HINT: Color32 = Color32::from_rgb(80, 80, 80);

    // ============================================
    // COLORS - Accent & Interactive
    // ============================================
    pub const ACCENT_PRIMARY: Color32 = Color32::from_rgb(74, 158, 255);
    pub const ACCENT_HOVER: Color32 = Color32::from_rgb(90, 175, 255);
    pub const ACCENT_PRESSED: Color32 = Color32::from_rgb(58, 142, 230);
    pub const ACCENT_WARNING: Color32 = Color32::from_rgb(255, 215, 0);
    pub const ACCENT_SUCCESS: Color32 = Color32::from_rgb(74, 222, 128);
    pub const ACCENT_ERROR: Color32 = Color32::from_rgb(239, 68, 68);

    // ============================================
    // COLORS - Rating & Labels
    // ============================================
    pub const RATING_ACTIVE: Color32 = Color32::from_rgb(255, 215, 0);
    pub const RATING_INACTIVE: Color32 = Color32::from_rgb(64, 64, 64);

    pub const LABEL_RED: Color32 = Color32::from_rgb(239, 68, 68);
    pub const LABEL_YELLOW: Color32 = Color32::from_rgb(251, 191, 36);
    pub const LABEL_GREEN: Color32 = Color32::from_rgb(74, 222, 128);
    pub const LABEL_BLUE: Color32 = Color32::from_rgb(74, 158, 255);
    pub const LABEL_PURPLE: Color32 = Color32::from_rgb(168, 85, 247);

    // ============================================
    // COLORS - Borders
    // ============================================
    pub const BORDER_DEFAULT: Color32 = Color32::from_rgb(51, 51, 51);
    pub const BORDER_FOCUS: Color32 = Color32::from_rgb(74, 158, 255);
    pub const BORDER_HOVER: Color32 = Color32::from_rgb(64, 64, 64);

    // ============================================
    // SPACING
    // ============================================
    pub const SPACE_XXS: f32 = 2.0;
    pub const SPACE_XS: f32 = 4.0;
    pub const SPACE_SM: f32 = 8.0;
    pub const SPACE_MD: f32 = 12.0;
    pub const SPACE_LG: f32 = 16.0;
    pub const SPACE_XL: f32 = 24.0;
    pub const SPACE_XXL: f32 = 32.0;

    // ============================================
    // SIZING - Fixed Dimensions
    // ============================================
    pub const TOOLBAR_HEIGHT: f32 = 48.0;
    pub const SIDEBAR_WIDTH: f32 = 220.0;
    pub const PANEL_WIDTH: f32 = 280.0;
    pub const FILMSTRIP_HEIGHT: f32 = 100.0;
    pub const TILE_WIDTH: f32 = 140.0;
    pub const TILE_HEIGHT: f32 = 160.0;
    pub const THUMBNAIL_SIZE: f32 = 80.0;

    // ============================================
    // BORDER RADIUS
    // ============================================
    pub const RADIUS_XS: f32 = 2.0;
    pub const RADIUS_SM: f32 = 3.0;
    pub const RADIUS_MD: f32 = 4.0;
    pub const RADIUS_LG: f32 = 6.0;
    pub const RADIUS_XL: f32 = 8.0;
    pub const RADIUS_FULL: f32 = 9999.0;

    // ============================================
    // TYPOGRAPHY - Font Sizes
    // ============================================
    pub const FONT_XS: f32 = 10.0;
    pub const FONT_SM: f32 = 11.0;
    pub const FONT_MD: f32 = 12.0;
    pub const FONT_LG: f32 = 14.0;
    pub const FONT_XL: f32 = 18.0;
    pub const FONT_XXL: f32 = 24.0;
    pub const FONT_DISPLAY: f32 = 64.0;

    // ============================================
    // TYPOGRAPHY - Font Weights (conceptual for egui)
    // ============================================
    pub const WEIGHT_NORMAL: i32 = 400;
    pub const WEIGHT_MEDIUM: i32 = 500;
    pub const WEIGHT_SEMIBOLD: i32 = 600;
    pub const WEIGHT_BOLD: i32 = 700;

    // ============================================
    // ANIMATION (durations in milliseconds)
    // ============================================
    pub const DURATION_FAST: f32 = 100.0;
    pub const DURATION_NORMAL: f32 = 200.0;
    pub const DURATION_SLOW: f32 = 300.0;

    // ============================================
    // Z-INDEX (conceptual layering for egui Areas)
    // ============================================
    pub const LAYER_BASE: usize = 0;
    pub const LAYER_ELEVATED: usize = 10;
    pub const LAYER_OVERLAY: usize = 100;
    pub const LAYER_MODAL: usize = 1000;

    /// Apply the VintageLightbox dark theme to the egui context
    pub fn apply_to_context(ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        let visuals = &mut style.visuals;

        // Dark theme base
        visuals.dark_mode = true;
        visuals.window_fill = Self::BG_APP;
        visuals.panel_fill = Self::BG_SURFACE;
        visuals.extreme_bg_color = Self::BG_ELEVATED;
        visuals.faint_bg_color = Self::BG_HOVER;

        // Widget colors
        visuals.widgets.noninteractive.bg_fill = Self::BG_ELEVATED;
        visuals.widgets.noninteractive.weak_bg_fill = Self::BG_SURFACE;
        visuals.widgets.noninteractive.fg_stroke.color = Self::TEXT_PRIMARY;

        visuals.widgets.inactive.bg_fill = Self::BG_ELEVATED;
        visuals.widgets.inactive.weak_bg_fill = Self::BG_SURFACE;
        visuals.widgets.inactive.fg_stroke.color = Self::TEXT_SECONDARY;

        visuals.widgets.hovered.bg_fill = Self::BG_HOVER;
        visuals.widgets.hovered.weak_bg_fill = Self::BG_HOVER;
        visuals.widgets.hovered.fg_stroke.color = Self::TEXT_PRIMARY;

        visuals.widgets.active.bg_fill = Self::BG_ACTIVE;
        visuals.widgets.active.weak_bg_fill = Self::BG_ACTIVE;
        visuals.widgets.active.fg_stroke.color = Self::TEXT_PRIMARY;

        // Selection color
        visuals.selection.bg_fill = Self::ACCENT_PRIMARY;
        visuals.selection.stroke.color = Self::ACCENT_PRIMARY;

        // Hyperlink color
        visuals.hyperlink_color = Self::ACCENT_PRIMARY;

        // Flat shadows for minimalist look
        visuals.window_shadow = egui::epaint::Shadow {
            offset: [0, 0],
            blur: 0,
            spread: 0,
            color: egui::Color32::TRANSPARENT,
        };

        ctx.set_style(style);
    }
}
