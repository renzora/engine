use bevy::prelude::KeyCode;
use bevy_egui::egui::{self, Color32, RichText, Vec2};

use crate::core::{EditorState, EditorAction, KeyBinding, KeyBindings, bindable_keys, key_name};

/// Render the settings window
pub fn render_settings_window(
    ctx: &egui::Context,
    editor_state: &mut EditorState,
    keybindings: &mut KeyBindings,
) {
    if !editor_state.show_settings_window {
        return;
    }

    // Handle key capture for rebinding
    if let Some(action) = keybindings.rebinding {
        capture_key_for_rebind(ctx, keybindings, action);
    }

    egui::Window::new("Settings")
        .collapsible(false)
        .resizable(true)
        .default_size(Vec2::new(450.0, 550.0))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Camera Settings
                ui.heading("Camera");
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label("Move Speed:");
                    ui.add(egui::DragValue::new(&mut editor_state.camera_move_speed)
                        .range(1.0..=50.0)
                        .speed(0.1));
                });

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                // Grid Settings
                ui.heading("Grid");
                ui.add_space(4.0);

                ui.checkbox(&mut editor_state.show_grid, "Show Grid");

                ui.horizontal(|ui| {
                    ui.label("Grid Size:");
                    ui.add(egui::DragValue::new(&mut editor_state.grid_size)
                        .range(1.0..=100.0)
                        .speed(0.5));
                });

                ui.horizontal(|ui| {
                    ui.label("Divisions:");
                    ui.add(egui::DragValue::new(&mut editor_state.grid_divisions)
                        .range(1..=50));
                });

                ui.horizontal(|ui| {
                    ui.label("Grid Color:");
                    let mut color = [
                        (editor_state.grid_color[0] * 255.0) as u8,
                        (editor_state.grid_color[1] * 255.0) as u8,
                        (editor_state.grid_color[2] * 255.0) as u8,
                    ];
                    if ui.color_edit_button_srgb(&mut color).changed() {
                        editor_state.grid_color = [
                            color[0] as f32 / 255.0,
                            color[1] as f32 / 255.0,
                            color[2] as f32 / 255.0,
                        ];
                    }
                });

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                // Keybindings Section
                ui.heading("Keyboard Shortcuts");
                ui.add_space(4.0);
                ui.label(RichText::new("Click on a key to rebind it").color(Color32::GRAY).small());
                ui.add_space(8.0);

                let mut current_category = "";

                for action in EditorAction::all() {
                    let category = action.category();
                    if category != current_category {
                        if !current_category.is_empty() {
                            ui.add_space(8.0);
                        }
                        ui.label(RichText::new(category).strong().color(Color32::from_rgb(150, 150, 170)));
                        ui.add_space(4.0);
                        current_category = category;
                    }

                    render_keybinding_row(ui, keybindings, action);
                }

                ui.add_space(12.0);

                // Reset to defaults button
                if ui.button("Reset to Defaults").clicked() {
                    *keybindings = KeyBindings::default();
                }

                ui.add_space(16.0);

                // Close button
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    if ui.button("Close").clicked() {
                        editor_state.show_settings_window = false;
                    }
                });
            });
        });
}

fn render_keybinding_row(ui: &mut egui::Ui, keybindings: &mut KeyBindings, action: EditorAction) {
    ui.horizontal(|ui| {
        ui.label(action.display_name());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let is_rebinding = keybindings.rebinding == Some(action);

            let button_text = if is_rebinding {
                RichText::new("Press a key...").color(Color32::YELLOW)
            } else if let Some(binding) = keybindings.get(action) {
                RichText::new(binding.display()).color(Color32::from_rgb(150, 200, 255)).monospace()
            } else {
                RichText::new("Unbound").color(Color32::GRAY)
            };

            let button = egui::Button::new(button_text)
                .min_size(Vec2::new(120.0, 20.0));

            if ui.add(button).clicked() {
                if is_rebinding {
                    keybindings.rebinding = None;
                } else {
                    keybindings.rebinding = Some(action);
                }
            }
        });
    });
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
