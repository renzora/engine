use bevy::prelude::KeyCode;
use bevy_egui::egui::{self, Color32, CornerRadius, RichText, Stroke, StrokeKind, Vec2};

use crate::core::{CollisionGizmoVisibility, EditorSettings, EditorAction, KeyBinding, KeyBindings, SettingsTab, bindable_keys};
use crate::theming::{Theme, ThemeManager};

/// Render the settings window as a centered modal overlay
pub fn render_settings_window(
    ctx: &egui::Context,
    settings: &mut EditorSettings,
    keybindings: &mut KeyBindings,
    theme_manager: &mut ThemeManager,
) {
    if !settings.show_settings_window {
        return;
    }

    // Clone the theme to avoid borrow conflicts with theme editor tab
    let theme_clone = theme_manager.active_theme.clone();
    let theme = &theme_clone;
    let panel_bg = theme.surfaces.popup.to_color32();
    let overlay_bg = theme.surfaces.overlay.to_color32();
    let _accent_color = theme.semantic.accent.to_color32();
    let _text_primary = theme.text.primary.to_color32();
    let _text_muted = theme.text.muted.to_color32();
    let _text_heading = theme.text.heading.to_color32();
    let border_color = theme.widgets.border.to_color32();
    let _item_bg = theme.panels.item_bg.to_color32();
    let _item_hover = theme.panels.item_hover.to_color32();
    let _tab_active = theme.panels.tab_active.to_color32();
    let _tab_inactive = theme.panels.tab_inactive.to_color32();
    let _close_hover = theme.panels.close_hover.to_color32();

    // Handle key capture for rebinding
    if let Some(action) = keybindings.rebinding {
        capture_key_for_rebind(ctx, keybindings, action);
    }

    // Darkened overlay behind the modal
    egui::Area::new(egui::Id::new("settings_overlay"))
        .fixed_pos(egui::pos2(0.0, 0.0))
        .order(egui::Order::Background)
        .show(ctx, |ui| {
            let screen_rect = ctx.content_rect();
            ui.painter().rect_filled(screen_rect, 0.0, overlay_bg);

            // Make overlay clickable to close
            let response = ui.allocate_rect(screen_rect, egui::Sense::click());
            if response.clicked() {
                settings.show_settings_window = false;
            }
        });

    // Modal window
    let screen_rect = ctx.content_rect();
    let modal_width = 550.0;
    let modal_height = 580.0;
    let modal_pos = egui::pos2(
        (screen_rect.width() - modal_width) / 2.0,
        (screen_rect.height() - modal_height) / 2.0,
    );

    egui::Area::new(egui::Id::new("settings_modal"))
        .fixed_pos(modal_pos)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let modal_rect = egui::Rect::from_min_size(modal_pos, Vec2::new(modal_width, modal_height));

            // Draw modal background
            ui.painter().rect(
                modal_rect,
                CornerRadius::same(12),
                panel_bg,
                Stroke::new(1.0, border_color),
                StrokeKind::Outside,
            );

            // Content area
            #[allow(deprecated)]
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(modal_rect), |ui| {
                ui.set_clip_rect(modal_rect);

                // Header with title and close button
                render_header(ui, settings, modal_width, theme);

                // Tab bar
                let content_rect = egui::Rect::from_min_size(
                    modal_rect.min + Vec2::new(0.0, 56.0),
                    Vec2::new(modal_width, modal_height - 56.0),
                );

                #[allow(deprecated)]
                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(content_rect), |ui| {
                    render_tabs(ui, settings, modal_width, theme);

                    // Tab content
                    let tab_content_rect = egui::Rect::from_min_size(
                        content_rect.min + Vec2::new(24.0, 52.0),
                        Vec2::new(modal_width - 48.0, modal_height - 56.0 - 52.0 - 24.0),
                    );

                    #[allow(deprecated)]
                    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(tab_content_rect), |ui| {
                        match settings.settings_tab {
                            SettingsTab::General => render_general_tab(ui, settings, theme),
                            SettingsTab::Viewport => render_viewport_tab(ui, settings, theme),
                            SettingsTab::Shortcuts => render_shortcuts_tab(ui, keybindings, theme),
                            SettingsTab::Theme => render_theme_tab(ui, theme_manager),
                        }
                    });
                });
            });
        });
}

fn render_header(ui: &mut egui::Ui, settings: &mut EditorSettings, modal_width: f32, theme: &Theme) {
    let text_primary = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let close_hover = theme.panels.close_hover.to_color32();
    let border_color = theme.widgets.border.to_color32();

    let header_rect = egui::Rect::from_min_size(
        ui.max_rect().min,
        Vec2::new(modal_width, 56.0),
    );

    // Title
    let title_pos = header_rect.min + Vec2::new(24.0, 18.0);
    ui.painter().text(
        title_pos,
        egui::Align2::LEFT_TOP,
        "Settings",
        egui::FontId::proportional(20.0),
        text_primary,
    );

    // Close button (X)
    let close_size = 32.0;
    let close_rect = egui::Rect::from_min_size(
        header_rect.right_top() + Vec2::new(-close_size - 16.0, 12.0),
        Vec2::new(close_size, close_size),
    );

    let close_response = ui.allocate_rect(close_rect, egui::Sense::click());
    let close_color = if close_response.hovered() { close_hover } else { text_muted };

    // Draw X
    let center = close_rect.center();
    let half = 8.0;
    ui.painter().line_segment(
        [center + Vec2::new(-half, -half), center + Vec2::new(half, half)],
        Stroke::new(2.0, close_color),
    );
    ui.painter().line_segment(
        [center + Vec2::new(half, -half), center + Vec2::new(-half, half)],
        Stroke::new(2.0, close_color),
    );

    if close_response.clicked() {
        settings.show_settings_window = false;
    }

    // Header separator
    let sep_y = header_rect.max.y;
    ui.painter().line_segment(
        [egui::pos2(header_rect.min.x, sep_y), egui::pos2(header_rect.max.x, sep_y)],
        Stroke::new(1.0, border_color),
    );
}

fn render_tabs(ui: &mut egui::Ui, settings: &mut EditorSettings, modal_width: f32, theme: &Theme) {
    let text_primary = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let accent_color = theme.semantic.accent.to_color32();
    let tab_active = theme.panels.tab_active.to_color32();
    let tab_inactive = theme.panels.tab_inactive.to_color32();
    let item_hover = theme.panels.item_hover.to_color32();
    let border_color = theme.widgets.border.to_color32();

    let tabs = [
        (SettingsTab::General, "General"),
        (SettingsTab::Viewport, "Viewport"),
        (SettingsTab::Shortcuts, "Shortcuts"),
        (SettingsTab::Theme, "Theme"),
    ];

    let tab_width = (modal_width - 48.0) / tabs.len() as f32;
    let tab_height = 36.0;
    let start_x = ui.max_rect().min.x + 24.0;
    let start_y = ui.max_rect().min.y + 8.0;

    for (i, (tab, label)) in tabs.iter().enumerate() {
        let tab_rect = egui::Rect::from_min_size(
            egui::pos2(start_x + i as f32 * tab_width, start_y),
            Vec2::new(tab_width, tab_height),
        );

        let response = ui.allocate_rect(tab_rect, egui::Sense::click());
        let is_active = settings.settings_tab == *tab;
        let is_hovered = response.hovered();

        // Tab background
        let bg_color = if is_active {
            tab_active
        } else if is_hovered {
            item_hover
        } else {
            tab_inactive
        };

        let corner_radius = if i == 0 {
            CornerRadius { nw: 8, ne: 0, sw: 0, se: 0 }
        } else if i == tabs.len() - 1 {
            CornerRadius { nw: 0, ne: 8, sw: 0, se: 0 }
        } else {
            CornerRadius::ZERO
        };

        ui.painter().rect(
            tab_rect,
            corner_radius,
            bg_color,
            Stroke::NONE,
            StrokeKind::Outside,
        );

        // Active indicator
        if is_active {
            let indicator_rect = egui::Rect::from_min_size(
                egui::pos2(tab_rect.min.x, tab_rect.max.y - 2.0),
                Vec2::new(tab_width, 2.0),
            );
            ui.painter().rect_filled(indicator_rect, 0.0, accent_color);
        }

        // Tab text
        let text_color = if is_active { text_primary } else { text_muted };
        ui.painter().text(
            tab_rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(13.0),
            text_color,
        );

        if response.clicked() {
            settings.settings_tab = *tab;
        }
    }

    // Tab bar bottom border
    let border_y = start_y + tab_height;
    ui.painter().line_segment(
        [egui::pos2(ui.max_rect().min.x, border_y), egui::pos2(ui.max_rect().max.x, border_y)],
        Stroke::new(1.0, border_color),
    );
}

fn render_general_tab(ui: &mut egui::Ui, settings: &mut EditorSettings, theme: &Theme) {
    let text_muted = theme.text.muted.to_color32();

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.set_width(ui.available_width());

        // Interface Section
        render_section_header(ui, "Interface");
        ui.add_space(8.0);

        render_setting_row(ui, "Font Size", |ui| {
            ui.add(egui::Slider::new(&mut settings.font_size, 10.0..=20.0)
                .step_by(1.0)
                .suffix("pt")
                .show_value(true));
        });
        ui.label(RichText::new("Base font size for text (default: 13pt)").size(11.0).color(text_muted));

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            if ui.button("Reset to Default").clicked() {
                settings.font_size = 13.0;
            }
        });

        ui.add_space(16.0);

        // Camera Section
        render_section_header(ui, "Camera");
        ui.add_space(8.0);

        render_setting_row(ui, "Move Speed", |ui| {
            ui.add(egui::DragValue::new(&mut settings.camera_move_speed)
                .range(1.0..=50.0)
                .speed(0.1));
        });

        ui.add_space(16.0);

        // Developer Section
        render_section_header(ui, "Developer");
        ui.add_space(8.0);

        render_setting_row(ui, "Developer Mode", |ui| {
            ui.checkbox(&mut settings.dev_mode, "");
        });
        ui.label(RichText::new("Enables Dev menu for plugin development").size(11.0).color(text_muted));
    });
}

fn render_viewport_tab(ui: &mut egui::Ui, settings: &mut EditorSettings, theme: &Theme) {
    let text_muted = theme.text.muted.to_color32();

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.set_width(ui.available_width());

        // Grid Section
        render_section_header(ui, "Grid");
        ui.add_space(8.0);

        render_setting_row(ui, "Show Grid", |ui| {
            ui.checkbox(&mut settings.show_grid, "");
        });

        render_setting_row(ui, "Grid Size", |ui| {
            ui.add(egui::DragValue::new(&mut settings.grid_size)
                .range(1.0..=100.0)
                .speed(0.5));
        });

        render_setting_row(ui, "Grid Divisions", |ui| {
            ui.add(egui::DragValue::new(&mut settings.grid_divisions)
                .range(1..=50));
        });

        render_setting_row(ui, "Grid Color", |ui| {
            let mut color = [
                (settings.grid_color[0] * 255.0) as u8,
                (settings.grid_color[1] * 255.0) as u8,
                (settings.grid_color[2] * 255.0) as u8,
            ];
            if ui.color_edit_button_srgb(&mut color).changed() {
                settings.grid_color = [
                    color[0] as f32 / 255.0,
                    color[1] as f32 / 255.0,
                    color[2] as f32 / 255.0,
                ];
            }
        });

        ui.add_space(16.0);

        // Gizmos Section
        render_section_header(ui, "Gizmos");
        ui.add_space(8.0);

        render_setting_row(ui, "Collision Gizmos", |ui| {
            egui::ComboBox::from_id_salt("collision_gizmo_visibility")
                .selected_text(match settings.collision_gizmo_visibility {
                    CollisionGizmoVisibility::SelectedOnly => "Selected Only",
                    CollisionGizmoVisibility::Always => "Always Visible",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut settings.collision_gizmo_visibility,
                        CollisionGizmoVisibility::SelectedOnly,
                        "Selected Only",
                    );
                    ui.selectable_value(
                        &mut settings.collision_gizmo_visibility,
                        CollisionGizmoVisibility::Always,
                        "Always Visible",
                    );
                });
        });
        ui.label(RichText::new("Controls when collision shape gizmos are displayed").size(11.0).color(text_muted));
    });
}

fn render_shortcuts_tab(ui: &mut egui::Ui, keybindings: &mut KeyBindings, theme: &Theme) {
    let text_primary = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let item_bg = theme.panels.item_bg.to_color32();
    let border_color = theme.widgets.border.to_color32();

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.set_width(ui.available_width());

        ui.label(RichText::new("Click on a key to rebind it").size(12.0).color(text_muted));
        ui.add_space(12.0);

        let mut current_category = "";

        for action in EditorAction::all() {
            let category = action.category();
            if category != current_category {
                if !current_category.is_empty() {
                    ui.add_space(12.0);
                }
                render_section_header(ui, category);
                ui.add_space(8.0);
                current_category = category;
            }

            render_keybinding_row(ui, keybindings, action, theme);
        }

        ui.add_space(16.0);

        // Reset to defaults button
        let btn = egui::Button::new(RichText::new("Reset to Defaults").color(text_primary))
            .fill(item_bg)
            .stroke(Stroke::new(1.0, border_color))
            .corner_radius(CornerRadius::same(6))
            .min_size(Vec2::new(140.0, 32.0));

        if ui.add(btn).clicked() {
            *keybindings = KeyBindings::default();
        }
    });
}

fn render_section_header(ui: &mut egui::Ui, label: &str) {
    // Use egui style text color (set by theme via style.rs)
    let text_heading = ui.style().visuals.weak_text_color();
    ui.label(RichText::new(label.to_uppercase()).size(11.0).color(text_heading).strong());
}

fn render_setting_row(ui: &mut egui::Ui, label: &str, add_control: impl FnOnce(&mut egui::Ui)) {
    // Use egui style text color (set by theme via style.rs)
    let text_color = ui.style().visuals.text_color();
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(text_color));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add_control);
    });
    ui.add_space(4.0);
}

fn render_keybinding_row(ui: &mut egui::Ui, keybindings: &mut KeyBindings, action: EditorAction, theme: &Theme) {
    let text_primary = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let accent_color = theme.semantic.accent.to_color32();
    let item_bg = theme.panels.item_bg.to_color32();
    let border_color = theme.widgets.border.to_color32();
    let warning_color = theme.semantic.warning.to_color32();

    ui.horizontal(|ui| {
        ui.label(RichText::new(action.display_name()).color(text_primary));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let is_rebinding = keybindings.rebinding == Some(action);

            let button_text = if is_rebinding {
                RichText::new("Press a key...").color(warning_color)
            } else if let Some(binding) = keybindings.get(action) {
                RichText::new(binding.display()).color(accent_color).monospace()
            } else {
                RichText::new("Unbound").color(text_muted)
            };

            // Rebinding highlight color
            let rebind_bg = Color32::from_rgb(60, 50, 30);
            let rebind_border = warning_color;

            let button = egui::Button::new(button_text)
                .fill(if is_rebinding { rebind_bg } else { item_bg })
                .stroke(Stroke::new(1.0, if is_rebinding { rebind_border } else { border_color }))
                .corner_radius(CornerRadius::same(4))
                .min_size(Vec2::new(120.0, 24.0));

            if ui.add(button).clicked() {
                if is_rebinding {
                    keybindings.rebinding = None;
                } else {
                    keybindings.rebinding = Some(action);
                }
            }
        });
    });
    ui.add_space(2.0);
}

fn capture_key_for_rebind(ctx: &egui::Context, keybindings: &mut KeyBindings, action: EditorAction) {
    let keys = bindable_keys();

    ctx.input(|input| {
        // Check modifiers
        let ctrl = input.modifiers.ctrl;
        let shift = input.modifiers.shift;
        let alt = input.modifiers.alt;

        // Check for key press
        for key in &keys {
            let egui_key = keycode_to_egui(*key);
            if let Some(egui_key) = egui_key {
                if input.key_pressed(egui_key) {
                    let mut binding = KeyBinding::new(*key);
                    if ctrl {
                        binding = binding.ctrl();
                    }
                    if shift {
                        binding = binding.shift();
                    }
                    if alt {
                        binding = binding.alt();
                    }
                    keybindings.set(action, binding);
                    keybindings.rebinding = None;
                    return;
                }
            }
        }

        // Cancel on Escape (without setting it as the binding)
        if input.key_pressed(egui::Key::Escape) && !ctrl && !shift && !alt {
            keybindings.rebinding = None;
        }
    });
}

fn keycode_to_egui(key: KeyCode) -> Option<egui::Key> {
    match key {
        KeyCode::KeyA => Some(egui::Key::A),
        KeyCode::KeyB => Some(egui::Key::B),
        KeyCode::KeyC => Some(egui::Key::C),
        KeyCode::KeyD => Some(egui::Key::D),
        KeyCode::KeyE => Some(egui::Key::E),
        KeyCode::KeyF => Some(egui::Key::F),
        KeyCode::KeyG => Some(egui::Key::G),
        KeyCode::KeyH => Some(egui::Key::H),
        KeyCode::KeyI => Some(egui::Key::I),
        KeyCode::KeyJ => Some(egui::Key::J),
        KeyCode::KeyK => Some(egui::Key::K),
        KeyCode::KeyL => Some(egui::Key::L),
        KeyCode::KeyM => Some(egui::Key::M),
        KeyCode::KeyN => Some(egui::Key::N),
        KeyCode::KeyO => Some(egui::Key::O),
        KeyCode::KeyP => Some(egui::Key::P),
        KeyCode::KeyQ => Some(egui::Key::Q),
        KeyCode::KeyR => Some(egui::Key::R),
        KeyCode::KeyS => Some(egui::Key::S),
        KeyCode::KeyT => Some(egui::Key::T),
        KeyCode::KeyU => Some(egui::Key::U),
        KeyCode::KeyV => Some(egui::Key::V),
        KeyCode::KeyW => Some(egui::Key::W),
        KeyCode::KeyX => Some(egui::Key::X),
        KeyCode::KeyY => Some(egui::Key::Y),
        KeyCode::KeyZ => Some(egui::Key::Z),
        KeyCode::Digit0 => Some(egui::Key::Num0),
        KeyCode::Digit1 => Some(egui::Key::Num1),
        KeyCode::Digit2 => Some(egui::Key::Num2),
        KeyCode::Digit3 => Some(egui::Key::Num3),
        KeyCode::Digit4 => Some(egui::Key::Num4),
        KeyCode::Digit5 => Some(egui::Key::Num5),
        KeyCode::Digit6 => Some(egui::Key::Num6),
        KeyCode::Digit7 => Some(egui::Key::Num7),
        KeyCode::Digit8 => Some(egui::Key::Num8),
        KeyCode::Digit9 => Some(egui::Key::Num9),
        KeyCode::Escape => Some(egui::Key::Escape),
        KeyCode::F1 => Some(egui::Key::F1),
        KeyCode::F2 => Some(egui::Key::F2),
        KeyCode::F3 => Some(egui::Key::F3),
        KeyCode::F4 => Some(egui::Key::F4),
        KeyCode::F5 => Some(egui::Key::F5),
        KeyCode::F6 => Some(egui::Key::F6),
        KeyCode::F7 => Some(egui::Key::F7),
        KeyCode::F8 => Some(egui::Key::F8),
        KeyCode::F9 => Some(egui::Key::F9),
        KeyCode::F10 => Some(egui::Key::F10),
        KeyCode::F11 => Some(egui::Key::F11),
        KeyCode::F12 => Some(egui::Key::F12),
        KeyCode::Space => Some(egui::Key::Space),
        KeyCode::Tab => Some(egui::Key::Tab),
        KeyCode::Enter => Some(egui::Key::Enter),
        KeyCode::Backspace => Some(egui::Key::Backspace),
        KeyCode::Delete => Some(egui::Key::Delete),
        KeyCode::Insert => Some(egui::Key::Insert),
        KeyCode::Home => Some(egui::Key::Home),
        KeyCode::End => Some(egui::Key::End),
        KeyCode::PageUp => Some(egui::Key::PageUp),
        KeyCode::PageDown => Some(egui::Key::PageDown),
        KeyCode::ArrowUp => Some(egui::Key::ArrowUp),
        KeyCode::ArrowDown => Some(egui::Key::ArrowDown),
        KeyCode::ArrowLeft => Some(egui::Key::ArrowLeft),
        KeyCode::ArrowRight => Some(egui::Key::ArrowRight),
        KeyCode::Comma => Some(egui::Key::Comma),
        KeyCode::Period => Some(egui::Key::Period),
        KeyCode::Minus => Some(egui::Key::Minus),
        KeyCode::Equal => Some(egui::Key::Equals),
        _ => None,
    }
}

/// State for the theme editor save-as dialog
static mut THEME_SAVE_NAME: Option<String> = None;

fn render_theme_tab(ui: &mut egui::Ui, theme_manager: &mut ThemeManager) {
    // Get theme colors for this UI
    let text_primary = theme_manager.active_theme.text.primary.to_color32();
    let text_heading = theme_manager.active_theme.text.heading.to_color32();
    let item_bg = theme_manager.active_theme.panels.item_bg.to_color32();
    let border_color = theme_manager.active_theme.widgets.border.to_color32();
    let accent_color = theme_manager.active_theme.semantic.accent.to_color32();
    let error_color = theme_manager.active_theme.semantic.error.to_color32();

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.set_width(ui.available_width());

        // Theme selector section
        ui.label(RichText::new("ACTIVE THEME").size(11.0).color(text_heading).strong());
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new("Theme").color(text_primary));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::ComboBox::from_id_salt("theme_selector")
                    .selected_text(&theme_manager.active_theme_name)
                    .show_ui(ui, |ui| {
                        for name in theme_manager.available_themes.clone() {
                            if ui.selectable_value(&mut theme_manager.active_theme_name.clone(), name.clone(), &name).clicked() {
                                theme_manager.load_theme(&name);
                            }
                        }
                    });
            });
        });
        ui.add_space(4.0);

        // Save/Create buttons
        ui.horizontal(|ui| {
            // Save As button (always available)
            if ui.add(egui::Button::new(RichText::new("Save As...").color(text_primary))
                .fill(item_bg)
                .stroke(Stroke::new(1.0, border_color))
                .corner_radius(CornerRadius::same(6))
            ).clicked() {
                unsafe {
                    THEME_SAVE_NAME = Some(format!("{} Copy", theme_manager.active_theme_name));
                }
            }

            // Save button (only for custom themes)
            if !theme_manager.is_builtin(&theme_manager.active_theme_name) {
                if ui.add(egui::Button::new(RichText::new("Save").color(text_primary))
                    .fill(item_bg)
                    .stroke(Stroke::new(1.0, border_color))
                    .corner_radius(CornerRadius::same(6))
                ).clicked() {
                    let name = theme_manager.active_theme_name.clone();
                    theme_manager.save_theme(&name);
                }
            }

            // Reset button (only if modified)
            if theme_manager.has_unsaved_changes {
                if ui.add(egui::Button::new(RichText::new("Reset").color(error_color))
                    .fill(item_bg)
                    .stroke(Stroke::new(1.0, border_color))
                    .corner_radius(CornerRadius::same(6))
                ).clicked() {
                    let name = theme_manager.active_theme_name.clone();
                    theme_manager.load_theme(&name);
                }
            }
        });

        // Save As dialog
        let mut close_dialog = false;
        unsafe {
            if let Some(ref mut save_name) = THEME_SAVE_NAME {
                ui.add_space(8.0);
                egui::Frame::new()
                    .fill(item_bg)
                    .corner_radius(CornerRadius::same(6))
                    .stroke(Stroke::new(1.0, accent_color))
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("New theme name:").color(text_primary));
                            ui.add(egui::TextEdit::singleline(save_name).desired_width(150.0));

                            if ui.add(egui::Button::new(RichText::new("Create").color(Color32::WHITE))
                                .fill(accent_color)
                                .corner_radius(CornerRadius::same(4))
                            ).clicked() {
                                if !save_name.is_empty() {
                                    theme_manager.save_theme(save_name);
                                    close_dialog = true;
                                }
                            }

                            if ui.add(egui::Button::new("Cancel")
                                .fill(item_bg)
                                .corner_radius(CornerRadius::same(4))
                            ).clicked() {
                                close_dialog = true;
                            }
                        });
                    });
            }
            if close_dialog {
                THEME_SAVE_NAME = None;
            }
        }

        ui.add_space(16.0);

        // Track if any colors were modified this frame
        let mut any_modified = false;

        // Color editor sections
        {
            let theme = &mut theme_manager.active_theme;

            // Semantic Colors
            ui.label(RichText::new("SEMANTIC COLORS").size(11.0).color(text_heading).strong());
            ui.add_space(8.0);
            any_modified |= render_color_row(ui, "Accent", &mut theme.semantic.accent);
            any_modified |= render_color_row(ui, "Success", &mut theme.semantic.success);
            any_modified |= render_color_row(ui, "Warning", &mut theme.semantic.warning);
            any_modified |= render_color_row(ui, "Error", &mut theme.semantic.error);
            any_modified |= render_color_row(ui, "Selection", &mut theme.semantic.selection);
            any_modified |= render_color_row(ui, "Selection Stroke", &mut theme.semantic.selection_stroke);

            ui.add_space(16.0);

            // Surface Colors
            render_section_header(ui, "Surfaces");
            ui.add_space(8.0);
            any_modified |= render_color_row(ui, "Window", &mut theme.surfaces.window);
            any_modified |= render_color_row(ui, "Window Stroke", &mut theme.surfaces.window_stroke);
            any_modified |= render_color_row(ui, "Panel", &mut theme.surfaces.panel);
            any_modified |= render_color_row(ui, "Popup", &mut theme.surfaces.popup);
            any_modified |= render_color_row(ui, "Faint", &mut theme.surfaces.faint);
            any_modified |= render_color_row(ui, "Extreme", &mut theme.surfaces.extreme);

            ui.add_space(16.0);

            // Text Colors
            render_section_header(ui, "Text");
            ui.add_space(8.0);
            any_modified |= render_color_row(ui, "Primary", &mut theme.text.primary);
            any_modified |= render_color_row(ui, "Secondary", &mut theme.text.secondary);
            any_modified |= render_color_row(ui, "Muted", &mut theme.text.muted);
            any_modified |= render_color_row(ui, "Heading", &mut theme.text.heading);
            any_modified |= render_color_row(ui, "Disabled", &mut theme.text.disabled);
            any_modified |= render_color_row(ui, "Hyperlink", &mut theme.text.hyperlink);

            ui.add_space(16.0);

            // Widget Colors
            render_section_header(ui, "Widgets");
            ui.add_space(8.0);
            any_modified |= render_color_row(ui, "Inactive BG", &mut theme.widgets.inactive_bg);
            any_modified |= render_color_row(ui, "Inactive FG", &mut theme.widgets.inactive_fg);
            any_modified |= render_color_row(ui, "Hovered BG", &mut theme.widgets.hovered_bg);
            any_modified |= render_color_row(ui, "Hovered FG", &mut theme.widgets.hovered_fg);
            any_modified |= render_color_row(ui, "Active BG", &mut theme.widgets.active_bg);
            any_modified |= render_color_row(ui, "Active FG", &mut theme.widgets.active_fg);
            any_modified |= render_color_row(ui, "Border", &mut theme.widgets.border);

            ui.add_space(16.0);

            // Panel Colors
            render_section_header(ui, "Panels");
            ui.add_space(8.0);
            any_modified |= render_color_row(ui, "Tree Line", &mut theme.panels.tree_line);
            any_modified |= render_color_row(ui, "Drop Line", &mut theme.panels.drop_line);
            any_modified |= render_color_row(ui, "Tab Active", &mut theme.panels.tab_active);
            any_modified |= render_color_row(ui, "Tab Inactive", &mut theme.panels.tab_inactive);

            ui.add_space(16.0);

            // Category Colors (collapsible)
            egui::CollapsingHeader::new(RichText::new("COMPONENT CATEGORIES").size(11.0).color(text_heading).strong())
                .default_open(false)
                .show(ui, |ui| {
                    ui.add_space(8.0);
                    any_modified |= render_category_colors(ui, "Transform", &mut theme.categories.transform);
                    any_modified |= render_category_colors(ui, "Lighting", &mut theme.categories.lighting);
                    any_modified |= render_category_colors(ui, "Camera", &mut theme.categories.camera);
                    any_modified |= render_category_colors(ui, "Physics", &mut theme.categories.physics);
                    any_modified |= render_category_colors(ui, "Scripting", &mut theme.categories.scripting);
                    any_modified |= render_category_colors(ui, "Environment", &mut theme.categories.environment);
                    any_modified |= render_category_colors(ui, "UI", &mut theme.categories.ui);
                    any_modified |= render_category_colors(ui, "2D Nodes", &mut theme.categories.nodes_2d);
                });

            ui.add_space(16.0);

            // Blueprint Colors (collapsible)
            egui::CollapsingHeader::new(RichText::new("BLUEPRINT EDITOR").size(11.0).color(text_heading).strong())
                .default_open(false)
                .show(ui, |ui| {
                    ui.add_space(8.0);
                    any_modified |= render_color_row(ui, "Grid Dot", &mut theme.blueprint.grid_dot);
                    any_modified |= render_color_row(ui, "Node BG", &mut theme.blueprint.node_bg);
                    any_modified |= render_color_row(ui, "Node Border", &mut theme.blueprint.node_border);
                    any_modified |= render_color_row(ui, "Selected Border", &mut theme.blueprint.node_selected_border);
                    any_modified |= render_color_row(ui, "Connection", &mut theme.blueprint.connection);
                });

            ui.add_space(16.0);
        }

        // Viewport Colors (collapsible)
        {
            let theme = &mut theme_manager.active_theme;
            egui::CollapsingHeader::new(RichText::new("VIEWPORT").size(11.0).color(text_heading).strong())
                .default_open(false)
                .show(ui, |ui| {
                    ui.add_space(8.0);
                    any_modified |= render_color_row(ui, "Grid Line", &mut theme.viewport.grid_line);
                    any_modified |= render_color_row(ui, "Gizmo X", &mut theme.viewport.gizmo_x);
                    any_modified |= render_color_row(ui, "Gizmo Y", &mut theme.viewport.gizmo_y);
                    any_modified |= render_color_row(ui, "Gizmo Z", &mut theme.viewport.gizmo_z);
                    any_modified |= render_color_row(ui, "Gizmo Selected", &mut theme.viewport.gizmo_selected);
                });
        }

        // Now mark modified outside the borrow
        if any_modified {
            theme_manager.mark_modified();
        }
    });
}

/// Render a single color picker row, returns true if changed
fn render_color_row(ui: &mut egui::Ui, label: &str, color: &mut crate::theming::ThemeColor) -> bool {
    let mut changed = false;
    let text_color = ui.style().visuals.text_color();
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(text_color));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let [r, g, b, a] = color.0.to_array();
            let mut srgba = [r, g, b, a];
            if ui.color_edit_button_srgba_unmultiplied(&mut srgba).changed() {
                *color = crate::theming::ThemeColor::with_alpha(srgba[0], srgba[1], srgba[2], srgba[3]);
                changed = true;
            }
        });
    });
    ui.add_space(2.0);
    changed
}

/// Render category colors (accent + header), returns true if changed
fn render_category_colors(ui: &mut egui::Ui, label: &str, style: &mut crate::theming::CategoryStyle) -> bool {
    let mut changed = false;
    let text_color = ui.style().visuals.text_color();
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}", label)).color(text_color));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Header BG
            let [r, g, b, a] = style.header_bg.0.to_array();
            let mut srgba_hdr = [r, g, b, a];
            if ui.color_edit_button_srgba_unmultiplied(&mut srgba_hdr).on_hover_text("Header BG").changed() {
                style.header_bg = crate::theming::ThemeColor::with_alpha(srgba_hdr[0], srgba_hdr[1], srgba_hdr[2], srgba_hdr[3]);
                changed = true;
            }

            // Accent
            let [r, g, b, a] = style.accent.0.to_array();
            let mut srgba_acc = [r, g, b, a];
            if ui.color_edit_button_srgba_unmultiplied(&mut srgba_acc).on_hover_text("Accent").changed() {
                style.accent = crate::theming::ThemeColor::with_alpha(srgba_acc[0], srgba_acc[1], srgba_acc[2], srgba_acc[3]);
                changed = true;
            }
        });
    });
    ui.add_space(2.0);
    changed
}
