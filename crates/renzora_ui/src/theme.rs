//! Editor theme application and font management

use bevy_egui::egui::{self, Color32, CornerRadius, Stroke, Visuals};
use renzora_theme::Theme;

/// Initialize all bundled fonts (call once at startup).
///
/// Loads proportional, monospace, CJK fallback, and icon fonts into egui.
/// The default families use `ui_font_key` / `mono_font_key` as their primaries.
pub fn init_fonts(ctx: &egui::Context, ui_font_key: &str, mono_font_key: &str) {
    let mut fonts = egui::FontDefinitions::default();

    // -- Proportional fonts --
    fonts.font_data.insert(
        "roboto".into(),
        egui::FontData::from_static(include_bytes!("../../../assets/fonts/Roboto-Regular.ttf"))
            .into(),
    );
    fonts.font_data.insert(
        "open-sans".into(),
        egui::FontData::from_static(include_bytes!(
            "../../../assets/fonts/OpenSans-Regular.ttf"
        ))
        .into(),
    );
    fonts.font_data.insert(
        "noto-sans".into(),
        egui::FontData::from_static(include_bytes!(
            "../../../assets/fonts/NotoSans-Regular.ttf"
        ))
        .into(),
    );

    // -- Monospace fonts --
    fonts.font_data.insert(
        "jetbrains-mono".into(),
        egui::FontData::from_static(include_bytes!(
            "../../../assets/fonts/JetBrainsMono-Regular.ttf"
        ))
        .into(),
    );
    fonts.font_data.insert(
        "fira-code".into(),
        egui::FontData::from_static(include_bytes!(
            "../../../assets/fonts/FiraCode-Regular.ttf"
        ))
        .into(),
    );
    fonts.font_data.insert(
        "source-code-pro".into(),
        egui::FontData::from_static(include_bytes!(
            "../../../assets/fonts/SourceCodePro-Regular.ttf"
        ))
        .into(),
    );

    // -- CJK fallback (Japanese, Chinese, Korean) --
    fonts.font_data.insert(
        "noto-sans-jp".into(),
        egui::FontData::from_static(include_bytes!(
            "../../../assets/fonts/NotoSansJP-Regular.ttf"
        ))
        .into(),
    );

    // -- Icon fonts --
    fonts.font_data.insert(
        "phosphor".into(),
        egui::FontData::from_static(egui_phosphor::Variant::Regular.font_bytes()).into(),
    );

    // Default families
    fonts.families.insert(
        egui::FontFamily::Proportional,
        vec![
            ui_font_key.into(),
            "noto-sans-jp".into(),
            "phosphor".into(),
        ],
    );
    fonts.families.insert(
        egui::FontFamily::Monospace,
        vec![mono_font_key.into()],
    );

    ctx.set_fonts(fonts);
}

/// Switch the active proportional (UI) font family at runtime.
pub fn set_ui_font(ctx: &egui::Context, font_key: &str) {
    let mut fonts = ctx.fonts(|f| f.definitions().clone());
    fonts.families.insert(
        egui::FontFamily::Proportional,
        vec![
            font_key.into(),
            "noto-sans-jp".into(),
            "phosphor".into(),
        ],
    );
    ctx.set_fonts(fonts);
}

/// Switch the active monospace (code) font family at runtime.
pub fn set_mono_font(ctx: &egui::Context, font_key: &str) {
    let mut fonts = ctx.fonts(|f| f.definitions().clone());
    fonts.families.insert(
        egui::FontFamily::Monospace,
        vec![font_key.into()],
    );
    ctx.set_fonts(fonts);
}

/// Round a font size so it maps to an integer number of physical pixels.
/// This eliminates sub-pixel blurriness on fractional-DPI displays (e.g. 125%, 150%).
fn snap_font_size(size: f32, pixels_per_point: f32) -> f32 {
    (size * pixels_per_point).round() / pixels_per_point
}

/// Apply the editor theme to the egui context (visuals + spacing + font sizes).
pub fn apply_theme(ctx: &egui::Context, theme: &Theme, font_size: f32) {
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

    // Scale all text styles based on font_size (base is 14.0)
    let scale = font_size / 14.0;
    let ppp = ctx.pixels_per_point();
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::proportional(snap_font_size(11.0 * scale, ppp)),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::proportional(snap_font_size(14.0 * scale, ppp)),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::monospace(snap_font_size(14.0 * scale, ppp)),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::proportional(snap_font_size(14.0 * scale, ppp)),
    );
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::proportional(snap_font_size(19.0 * scale, ppp)),
    );

    ctx.set_style(style);
}
