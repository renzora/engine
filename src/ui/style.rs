use bevy_egui::egui::{self, Color32, CornerRadius, Stroke, Visuals};

use crate::theming::Theme;

/// Initialize phosphor icons font (call once at startup)
pub fn init_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // Add phosphor regular icon font using raw bytes (to avoid version mismatch)
    fonts.font_data.insert(
        "phosphor".into(),
        egui::FontData::from_static(egui_phosphor::Variant::Regular.font_bytes()).into(),
    );

    // Add phosphor fill icon font
    fonts.font_data.insert(
        "phosphor-fill".into(),
        egui::FontData::from_static(egui_phosphor::Variant::Fill.font_bytes()).into(),
    );

    // Add to proportional family as fallback (after default font)
    if let Some(font_keys) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        font_keys.insert(1, "phosphor".into());
        font_keys.insert(2, "phosphor-fill".into());
    }

    ctx.set_fonts(fonts);
}

/// Apply the editor's theme styling
#[allow(dead_code)]
pub fn apply_editor_style(ctx: &egui::Context) {
    apply_editor_style_with_theme(ctx, &Theme::dark(), 13.0);
}

/// Apply the editor's theme styling with a specific theme and font size
pub fn apply_editor_style_with_theme(ctx: &egui::Context, theme: &Theme, font_size: f32) {
    let mut visuals = Visuals::dark();

    // Window styling
    visuals.window_fill = theme.surfaces.window.to_color32();
    visuals.window_stroke = Stroke::new(1.0, theme.surfaces.window_stroke.to_color32());
    visuals.window_corner_radius = CornerRadius::same(0);

    // Panel styling
    visuals.panel_fill = theme.surfaces.panel.to_color32();

    // Text colors - this is the key for theming text!
    visuals.override_text_color = Some(theme.text.primary.to_color32());
    visuals.warn_fg_color = theme.semantic.warning.to_color32();
    visuals.error_fg_color = theme.semantic.error.to_color32();

    // Widget styling - with proper text colors in fg_stroke
    visuals.widgets.noninteractive.bg_fill = theme.widgets.noninteractive_bg.to_color32();
    visuals.widgets.noninteractive.weak_bg_fill = theme.widgets.noninteractive_bg.to_color32();
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, theme.text.primary.to_color32());
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

    // Open menu styling
    visuals.widgets.open.bg_fill = theme.widgets.active_bg.to_color32();
    visuals.widgets.open.weak_bg_fill = theme.widgets.active_bg.to_color32();
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, theme.text.primary.to_color32());
    visuals.widgets.open.corner_radius = CornerRadius::same(4);

    // Selection
    visuals.selection.bg_fill = theme.semantic.selection.to_color32();
    visuals.selection.stroke = Stroke::new(1.0, theme.semantic.selection_stroke.to_color32());

    // Hyperlink
    visuals.hyperlink_color = theme.text.hyperlink.to_color32();

    // Faint background for code/scrollbars
    visuals.faint_bg_color = theme.surfaces.faint.to_color32();
    visuals.extreme_bg_color = theme.surfaces.extreme.to_color32();

    // Popup shadow
    visuals.popup_shadow = egui::epaint::Shadow {
        offset: [0, 4],
        blur: 8,
        spread: 0,
        color: Color32::from_black_alpha(100),
    };

    // Window shadow
    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 4],
        blur: 12,
        spread: 0,
        color: Color32::from_black_alpha(120),
    };

    // Separator/border
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, theme.widgets.border.to_color32());

    ctx.set_visuals(visuals);

    // Set spacing and font sizes
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.window_margin = egui::Margin::same(10);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.indent = 18.0;
    style.spacing.scroll = egui::style::ScrollStyle {
        bar_width: 10.0,
        ..Default::default()
    };

    // Apply font size scaling
    apply_font_size(&mut style, font_size);

    ctx.set_style(style);
}

/// Get accent color for highlighted elements (uses default theme)
#[allow(dead_code)]
pub fn accent_color() -> Color32 {
    Theme::dark().semantic.accent.to_color32()
}

/// Get success color (green) (uses default theme)
#[allow(dead_code)]
pub fn success_color() -> Color32 {
    Theme::dark().semantic.success.to_color32()
}

/// Get warning color (orange) (uses default theme)
#[allow(dead_code)]
pub fn warning_color() -> Color32 {
    Theme::dark().semantic.warning.to_color32()
}

/// Get error color (red) (uses default theme)
#[allow(dead_code)]
pub fn error_color() -> Color32 {
    Theme::dark().semantic.error.to_color32()
}

/// Apply font size to style (call this with apply_editor_style_with_theme)
pub fn apply_font_size(style: &mut egui::Style, font_size: f32) {
    // Scale all text styles based on font_size (default is 13.0)
    let scale_factor = font_size / 13.0;

    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::proportional(10.0 * scale_factor),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::proportional(13.0 * scale_factor),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::monospace(13.0 * scale_factor),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::proportional(13.0 * scale_factor),
    );
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::proportional(18.0 * scale_factor),
    );
}
