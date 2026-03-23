//! Editor theme application

use bevy_egui::egui::{self, Color32, CornerRadius, Stroke, Visuals};
use renzora_theme::Theme;

/// Initialize fonts (including phosphor icons). Call once at startup.
pub fn init_fonts(ctx: &egui::Context) {
    let mut fonts = ctx.fonts(|f| f.definitions().clone());

    // Phosphor icon fonts
    fonts.font_data.insert(
        "phosphor".into(),
        egui::FontData::from_static(egui_phosphor::Variant::Regular.font_bytes()).into(),
    );

    // Add phosphor as fallback to proportional family
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .push("phosphor".into());

    ctx.set_fonts(fonts);
}

/// Apply the editor theme to the egui context (visuals + spacing + font sizes).
pub fn apply_theme(ctx: &egui::Context, theme: &Theme) {
    let mut visuals = Visuals::dark();

    // Window styling
    visuals.window_fill = theme.surfaces.window.to_color32();
    visuals.window_stroke = Stroke::new(1.0, theme.surfaces.window_stroke.to_color32());
    visuals.window_corner_radius = CornerRadius::same(0);

    // Panel styling
    visuals.panel_fill = theme.surfaces.panel.to_color32();

    // Text colors
    visuals.override_text_color = Some(theme.text.primary.to_color32());
    visuals.warn_fg_color = theme.semantic.warning.to_color32();
    visuals.error_fg_color = theme.semantic.error.to_color32();

    // Widget styling
    visuals.widgets.noninteractive.bg_fill = theme.widgets.noninteractive_bg.to_color32();
    visuals.widgets.noninteractive.weak_bg_fill = theme.widgets.noninteractive_bg.to_color32();
    visuals.widgets.noninteractive.fg_stroke =
        Stroke::new(1.0, theme.text.primary.to_color32());
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(4);

    visuals.widgets.inactive.bg_fill = theme.widgets.inactive_bg.to_color32();
    visuals.widgets.inactive.weak_bg_fill = theme.widgets.inactive_bg.to_color32();
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, theme.text.primary.to_color32());
    visuals.widgets.inactive.corner_radius = CornerRadius::same(4);

    visuals.widgets.hovered.bg_fill = theme.widgets.hovered_bg.to_color32();
    visuals.widgets.hovered.weak_bg_fill = theme.widgets.hovered_bg.to_color32();
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, theme.text.primary.to_color32());
    visuals.widgets.hovered.corner_radius = CornerRadius::same(4);

    visuals.widgets.active.bg_fill = theme.widgets.active_bg.to_color32();
    visuals.widgets.active.weak_bg_fill = theme.widgets.active_bg.to_color32();
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, theme.text.primary.to_color32());
    visuals.widgets.active.corner_radius = CornerRadius::same(4);

    visuals.widgets.open.bg_fill = theme.widgets.active_bg.to_color32();
    visuals.widgets.open.weak_bg_fill = theme.widgets.active_bg.to_color32();
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, theme.text.primary.to_color32());
    visuals.widgets.open.corner_radius = CornerRadius::same(4);

    // Selection
    visuals.selection.bg_fill = theme.semantic.selection.to_color32();
    visuals.selection.stroke =
        Stroke::new(1.0, theme.semantic.selection_stroke.to_color32());

    // Hyperlink
    visuals.hyperlink_color = theme.text.hyperlink.to_color32();

    // Faint / extreme backgrounds
    visuals.faint_bg_color = theme.surfaces.faint.to_color32();
    visuals.extreme_bg_color = theme.surfaces.extreme.to_color32();

    // Shadows
    visuals.popup_shadow = egui::epaint::Shadow {
        offset: [0, 4],
        blur: 8,
        spread: 0,
        color: Color32::from_black_alpha(100),
    };
    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 4],
        blur: 12,
        spread: 0,
        color: Color32::from_black_alpha(120),
    };

    // Separator / border
    visuals.widgets.noninteractive.bg_stroke =
        Stroke::new(1.0, theme.widgets.border.to_color32());

    ctx.set_visuals(visuals);

    // Spacing and font sizes
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.window_margin = egui::Margin::same(10);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.indent = 18.0;
    style.spacing.scroll = egui::style::ScrollStyle {
        bar_width: 10.0,
        ..Default::default()
    };

    let font_size: f32 = 13.0;
    let scale = font_size / 13.0;
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::proportional(10.0 * scale),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::proportional(13.0 * scale),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::monospace(13.0 * scale),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::proportional(13.0 * scale),
    );
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::proportional(18.0 * scale),
    );

    ctx.set_style(style);
}
