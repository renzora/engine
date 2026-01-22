use bevy_egui::egui::{self, Color32, CornerRadius, Stroke, Visuals};

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

/// Apply the editor's dark theme styling
pub fn apply_editor_style(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();

    // Window styling
    visuals.window_fill = Color32::from_rgb(26, 26, 31);
    visuals.window_stroke = Stroke::new(1.0, Color32::from_rgb(50, 50, 58));
    visuals.window_corner_radius = CornerRadius::same(0);

    // Panel styling
    visuals.panel_fill = Color32::from_rgb(26, 26, 31);

    // Widget styling
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(36, 36, 42);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(180, 180, 190));
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(4);

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(46, 46, 56);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(200, 200, 210));
    visuals.widgets.inactive.corner_radius = CornerRadius::same(4);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(56, 56, 68);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::from_rgb(220, 220, 230));
    visuals.widgets.hovered.corner_radius = CornerRadius::same(4);

    visuals.widgets.active.bg_fill = Color32::from_rgb(66, 150, 250);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.widgets.active.corner_radius = CornerRadius::same(4);

    // Selection
    visuals.selection.bg_fill = Color32::from_rgb(66, 150, 250);
    visuals.selection.stroke = Stroke::new(1.0, Color32::from_rgb(100, 180, 255));

    // Hyperlink
    visuals.hyperlink_color = Color32::from_rgb(100, 180, 255);

    // Faint background for code/scrollbars
    visuals.faint_bg_color = Color32::from_rgb(20, 20, 24);
    visuals.extreme_bg_color = Color32::from_rgb(15, 15, 18);

    // Separator
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(45, 45, 52));

    ctx.set_visuals(visuals);

    // Set spacing
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.window_margin = egui::Margin::same(10);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.indent = 18.0;
    style.spacing.scroll = egui::style::ScrollStyle {
        bar_width: 10.0,
        ..Default::default()
    };
    ctx.set_style(style);
}

/// Get accent color for highlighted elements
#[allow(dead_code)]
pub fn accent_color() -> Color32 {
    Color32::from_rgb(66, 150, 250)
}

/// Get success color (green)
#[allow(dead_code)]
pub fn success_color() -> Color32 {
    Color32::from_rgb(89, 191, 115)
}

/// Get warning color (orange)
#[allow(dead_code)]
pub fn warning_color() -> Color32 {
    Color32::from_rgb(242, 166, 64)
}

/// Get error color (red)
#[allow(dead_code)]
pub fn error_color() -> Color32 {
    Color32::from_rgb(230, 89, 89)
}
