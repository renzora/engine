use bevy::prelude::KeyCode;
use bevy_egui::egui::{self, Color32, CornerRadius, RichText, Stroke, StrokeKind, Vec2};

use crate::core::{CollisionGizmoVisibility, EditorSettings, EditorAction, KeyBinding, KeyBindings, SettingsTab, bindable_keys};

// Colors matching the splash screen aesthetic
const PANEL_BG: Color32 = Color32::from_rgb(28, 28, 35);
const OVERLAY_BG: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 180);
const ACCENT_COLOR: Color32 = Color32::from_rgb(100, 160, 255);
const TEXT_PRIMARY: Color32 = Color32::from_rgb(230, 230, 240);
const TEXT_MUTED: Color32 = Color32::from_rgb(140, 140, 155);
const TEXT_HEADING: Color32 = Color32::from_rgb(180, 180, 195);
const BORDER_COLOR: Color32 = Color32::from_rgb(50, 50, 60);
const ITEM_BG: Color32 = Color32::from_rgb(35, 35, 45);
const ITEM_HOVER: Color32 = Color32::from_rgb(45, 45, 58);
const TAB_ACTIVE: Color32 = Color32::from_rgb(45, 45, 58);
const TAB_INACTIVE: Color32 = Color32::from_rgb(28, 28, 35);
const CLOSE_HOVER: Color32 = Color32::from_rgb(200, 60, 60);

/// Render the settings window as a centered modal overlay
pub fn render_settings_window(
    ctx: &egui::Context,
    settings: &mut EditorSettings,
    keybindings: &mut KeyBindings,
) {
    if !settings.show_settings_window {
        return;
    }

    // Handle key capture for rebinding
    if let Some(action) = keybindings.rebinding {
        capture_key_for_rebind(ctx, keybindings, action);
    }

    // Darkened overlay behind the modal
    egui::Area::new(egui::Id::new("settings_overlay"))
        .fixed_pos(egui::pos2(0.0, 0.0))
        .order(egui::Order::Background)
        .show(ctx, |ui| {
            let screen_rect = ctx.screen_rect();
            ui.painter().rect_filled(screen_rect, 0.0, OVERLAY_BG);

            // Make overlay clickable to close
            let response = ui.allocate_rect(screen_rect, egui::Sense::click());
            if response.clicked() {
                settings.show_settings_window = false;
            }
        });

    // Modal window
    let screen_rect = ctx.screen_rect();
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
                PANEL_BG,
                Stroke::new(1.0, BORDER_COLOR),
                StrokeKind::Outside,
            );

            // Content area
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(modal_rect), |ui| {
                ui.set_clip_rect(modal_rect);

                // Header with title and close button
                render_header(ui, settings, modal_width);

                // Tab bar
                let content_rect = egui::Rect::from_min_size(
                    modal_rect.min + Vec2::new(0.0, 56.0),
                    Vec2::new(modal_width, modal_height - 56.0),
                );

                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(content_rect), |ui| {
                    render_tabs(ui, settings, modal_width);

                    // Tab content
                    let tab_content_rect = egui::Rect::from_min_size(
                        content_rect.min + Vec2::new(24.0, 52.0),
                        Vec2::new(modal_width - 48.0, modal_height - 56.0 - 52.0 - 24.0),
                    );

                    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(tab_content_rect), |ui| {
                        match settings.settings_tab {
                            SettingsTab::General => render_general_tab(ui, settings),
                            SettingsTab::Viewport => render_viewport_tab(ui, settings),
                            SettingsTab::Shortcuts => render_shortcuts_tab(ui, keybindings),
                        }
                    });
                });
            });
        });
}

fn render_header(ui: &mut egui::Ui, settings: &mut EditorSettings, modal_width: f32) {
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
        TEXT_PRIMARY,
    );

    // Close button (X)
    let close_size = 32.0;
    let close_rect = egui::Rect::from_min_size(
        header_rect.right_top() + Vec2::new(-close_size - 16.0, 12.0),
        Vec2::new(close_size, close_size),
    );

    let close_response = ui.allocate_rect(close_rect, egui::Sense::click());
    let close_color = if close_response.hovered() { CLOSE_HOVER } else { TEXT_MUTED };

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
        Stroke::new(1.0, BORDER_COLOR),
    );
}

fn render_tabs(ui: &mut egui::Ui, settings: &mut EditorSettings, modal_width: f32) {
    let tabs = [
        (SettingsTab::General, "General"),
        (SettingsTab::Viewport, "Viewport"),
        (SettingsTab::Shortcuts, "Shortcuts"),
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
            TAB_ACTIVE
        } else if is_hovered {
            ITEM_HOVER
        } else {
            TAB_INACTIVE
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
            ui.painter().rect_filled(indicator_rect, 0.0, ACCENT_COLOR);
        }

        // Tab text
        let text_color = if is_active { TEXT_PRIMARY } else { TEXT_MUTED };
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
        Stroke::new(1.0, BORDER_COLOR),
    );
}

fn render_general_tab(ui: &mut egui::Ui, settings: &mut EditorSettings) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.set_width(ui.available_width());

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
        ui.label(RichText::new("Enables Dev menu for plugin development").size(11.0).color(TEXT_MUTED));
    });
}

fn render_viewport_tab(ui: &mut egui::Ui, settings: &mut EditorSettings) {
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
        ui.label(RichText::new("Controls when collision shape gizmos are displayed").size(11.0).color(TEXT_MUTED));
    });
}

fn render_shortcuts_tab(ui: &mut egui::Ui, keybindings: &mut KeyBindings) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.set_width(ui.available_width());

        ui.label(RichText::new("Click on a key to rebind it").size(12.0).color(TEXT_MUTED));
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

            render_keybinding_row(ui, keybindings, action);
        }

        ui.add_space(16.0);

        // Reset to defaults button
        let btn = egui::Button::new(RichText::new("Reset to Defaults").color(TEXT_PRIMARY))
            .fill(ITEM_BG)
            .stroke(Stroke::new(1.0, BORDER_COLOR))
            .corner_radius(CornerRadius::same(6))
            .min_size(Vec2::new(140.0, 32.0));

        if ui.add(btn).clicked() {
            *keybindings = KeyBindings::default();
        }
    });
}

fn render_section_header(ui: &mut egui::Ui, label: &str) {
    ui.label(RichText::new(label.to_uppercase()).size(11.0).color(TEXT_HEADING).strong());
}

fn render_setting_row(ui: &mut egui::Ui, label: &str, add_control: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(TEXT_PRIMARY));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add_control);
    });
    ui.add_space(4.0);
}

fn render_keybinding_row(ui: &mut egui::Ui, keybindings: &mut KeyBindings, action: EditorAction) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(action.display_name()).color(TEXT_PRIMARY));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let is_rebinding = keybindings.rebinding == Some(action);

            let button_text = if is_rebinding {
                RichText::new("Press a key...").color(Color32::from_rgb(255, 220, 100))
            } else if let Some(binding) = keybindings.get(action) {
                RichText::new(binding.display()).color(ACCENT_COLOR).monospace()
            } else {
                RichText::new("Unbound").color(TEXT_MUTED)
            };

            let button = egui::Button::new(button_text)
                .fill(if is_rebinding { Color32::from_rgb(60, 50, 30) } else { ITEM_BG })
                .stroke(Stroke::new(1.0, if is_rebinding { Color32::from_rgb(255, 220, 100) } else { BORDER_COLOR }))
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
