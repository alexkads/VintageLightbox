// VintageLightbox Design System - Primitive Components
// Reusable base components built on design tokens
// Ported from Slint to egui

#![allow(dead_code)]

use egui::{Button, Response, Ui, Vec2, CornerRadius, Stroke, RichText, Color32, Rect};
use super::theme::Theme;

// ============================================
// PRIMARY BUTTON
// Main action button with accent color
// ============================================
pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    let button = Button::new(
        RichText::new(text)
            .color(ui.visuals().selection.stroke.color)
            .size(Theme::FONT_MD)
    )
    .fill(ui.visuals().selection.bg_fill)
    .min_size(Vec2::new(80.0, 32.0))
    .corner_radius(CornerRadius::same(Theme::RADIUS_MD as u8));

    ui.add(button)
}

// ============================================
// SECONDARY BUTTON
// Ghost/outline style button
// ============================================
pub fn secondary_button(ui: &mut Ui, text: &str) -> Response {
    let button = Button::new(
        RichText::new(text)
            .color(ui.visuals().text_color())
            .size(Theme::FONT_MD)
    )
    .fill(ui.visuals().widgets.active.bg_fill)
    .stroke(Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color))
    .min_size(Vec2::new(80.0, 32.0))
    .corner_radius(CornerRadius::same(Theme::RADIUS_MD as u8));

    ui.add(button)
}

// ============================================
// ICON BUTTON
// Circular button for icons
// ============================================
pub fn icon_button(ui: &mut Ui, icon: &str) -> Response {
    icon_button_sized(ui, icon, Theme::FONT_XL)
}

pub fn icon_button_sized(ui: &mut Ui, icon: &str, icon_size: f32) -> Response {
    let button = Button::new(
        RichText::new(icon)
            .color(ui.visuals().text_color())
            .size(icon_size)
    )
    .fill(ui.visuals().widgets.inactive.weak_bg_fill)
    .min_size(Vec2::new(40.0, 40.0))
    .corner_radius(CornerRadius::same(255));

    ui.add(button)
}

/// Icon button with tooltip
pub fn icon_button_tooltip(ui: &mut Ui, icon: &str, tooltip: &str) -> Response {
    let response = icon_button(ui, icon);
    response.on_hover_text(tooltip)
}

/// Icon button with custom size and tooltip
pub fn icon_button_tooltip_sized(ui: &mut Ui, icon: &str, tooltip: &str, icon_size: f32) -> Response {
    let response = icon_button_sized(ui, icon, icon_size);
    response.on_hover_text(tooltip)
}

/// Primary style icon button (accent color background)
pub fn icon_button_primary(ui: &mut Ui, icon: &str, tooltip: &str) -> Response {
    let button = Button::new(
        RichText::new(icon)
            .color(ui.visuals().selection.stroke.color)
            .size(Theme::FONT_XL)
    )
    .fill(ui.visuals().selection.bg_fill)
    .min_size(Vec2::new(40.0, 40.0))
    .corner_radius(CornerRadius::same(255));

    ui.add(button).on_hover_text(tooltip)
}

// ============================================
// NAV BUTTON
// Navigation button with text and optional icon
// ============================================
pub fn nav_button(ui: &mut Ui, text: &str, active: bool) -> Response {
    let bg_color = if active {
        ui.visuals().widgets.active.bg_fill
    } else {
        Color32::TRANSPARENT
    };

    let text_color = if active {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };

    let button = Button::new(
        RichText::new(text)
            .color(text_color)
            .size(Theme::FONT_MD)
    )
    .fill(bg_color)
    .min_size(Vec2::new(80.0, 32.0))
    .corner_radius(CornerRadius::same(Theme::RADIUS_MD as u8));

    ui.add(button)
}

pub fn nav_button_with_icon(ui: &mut Ui, icon: &str, text: &str, active: bool) -> Response {
    let bg_color = if active {
        ui.visuals().widgets.active.bg_fill
    } else {
        Color32::TRANSPARENT
    };

    let text_color = if active {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };

    let label = format!("{} {}", icon, text);
    let button = Button::new(
        RichText::new(label)
            .color(text_color)
            .size(Theme::FONT_MD)
    )
    .fill(bg_color)
    .min_size(Vec2::new(80.0, 32.0))
    .corner_radius(CornerRadius::same(Theme::RADIUS_MD as u8));

    ui.add(button)
}

// ============================================
// PANEL HEADER
// Collapsible panel header with title
// ============================================
pub struct PanelHeader {
    pub collapsed: bool,
}

impl PanelHeader {
    pub fn new(collapsed: bool) -> Self {
        Self { collapsed }
    }

    pub fn show(&mut self, ui: &mut Ui, title: &str) -> Response {
        let arrow = if self.collapsed { "▶" } else { "▼" };
        let label = format!("{} {}", arrow, title);

        let response = ui.add(
            Button::new(
                RichText::new(label)
                    .color(ui.visuals().text_color())
                    .size(Theme::FONT_SM)
            )
            .fill(Color32::TRANSPARENT)
            .frame(false)
        );

        if response.clicked() {
            self.collapsed = !self.collapsed;
        }

        response
    }
}

// ============================================
// MENU ITEM
// Clickable menu item with hover state
// ============================================
pub fn menu_item(ui: &mut Ui, text: &str, selected: bool) -> Response {
    menu_item_with_indent(ui, text, selected, Theme::SPACE_LG)
}

pub fn menu_item_with_indent(ui: &mut Ui, text: &str, selected: bool, indent: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 26.0),
        egui::Sense::click()
    );

    let bg_color = if selected {
        ui.visuals().selection.bg_fill
    } else if response.hovered() {
        ui.visuals().widgets.hovered.bg_fill
    } else {
        Color32::TRANSPARENT
    };

    let text_color = if selected {
        ui.visuals().strong_text_color()
    } else {
        ui.visuals().text_color()
    };

    ui.painter().rect_filled(rect, Theme::RADIUS_SM, bg_color);

    let text_pos = rect.min + Vec2::new(indent, 0.0);
    ui.painter().text(
        text_pos + Vec2::new(0.0, 13.0),
        egui::Align2::LEFT_CENTER,
        text,
        egui::FontId::proportional(Theme::FONT_SM),
        text_color,
    );

    response
}

// ============================================
// CARD
// Container with background and border-radius
// ============================================
pub fn card(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    egui::Frame::default()
        .fill(ui.visuals().panel_fill)
        .corner_radius(CornerRadius::same(Theme::RADIUS_MD as u8))
        .show(ui, add_contents);
}

// ============================================
// DIVIDER
// Horizontal divider line
// ============================================
pub fn divider(ui: &mut Ui) {
    ui.add_space(Theme::SPACE_XS);
    ui.separator();
    ui.add_space(Theme::SPACE_XS);
}

// ============================================
// SPACER
// Flexible empty space
// ============================================
pub fn spacer(ui: &mut Ui, size: f32) {
    ui.add_space(size);
}

// ============================================
// OVERLAY
// Modal overlay with busy state
// Not used as a function, but as Area in main app
// ============================================
pub fn show_busy_overlay(ctx: &egui::Context, message: &str) {
    let screen_rect = ctx.screen_rect();

    egui::Area::new(egui::Id::new("busy_overlay"))
        .fixed_pos(egui::pos2(0.0, 0.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.allocate_ui(screen_rect.size(), |ui| {
                // Semi-transparent black overlay
                ui.painter().rect_filled(
                    screen_rect,
                    0.0,
                    Theme::BG_OVERLAY,
                );

                // Center the message
                let text_pos = screen_rect.center();
                ui.painter().text(
                    text_pos,
                    egui::Align2::CENTER_CENTER,
                    message,
                    egui::FontId::proportional(Theme::FONT_XL),
                    Color32::WHITE,
                );

                // Progress bar (animated could be added later)
                let progress_rect = Rect::from_center_size(
                    text_pos + Vec2::new(0.0, 40.0),
                    Vec2::new(200.0, 4.0),
                );
                ui.painter().rect_filled(
                    progress_rect,
                    Theme::RADIUS_XS,
                    Theme::BG_ACTIVE,
                );

                let progress_fill = Rect::from_min_size(
                    progress_rect.min,
                    Vec2::new(60.0, 4.0),
                );
                ui.painter().rect_filled(
                    progress_fill,
                    Theme::RADIUS_XS,
                    Theme::ACCENT_PRIMARY,
                );
            });
        });
}

// ============================================
// SECTION TITLE
// Section header text
// ============================================
pub fn section_title(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .color(ui.visuals().weak_text_color())
            .size(Theme::FONT_SM)
    );
}

// ============================================
// LABEL TEXT
// Small label text
// ============================================
pub fn label_text(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .color(ui.visuals().weak_text_color())
            .size(Theme::FONT_SM)
    );
}

// ============================================
// VALUE TEXT
// Value display text
// ============================================
pub fn value_text(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .color(ui.visuals().text_color())
            .size(Theme::FONT_SM)
    );
}
