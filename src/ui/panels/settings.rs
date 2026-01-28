use bevy::prelude::KeyCode;
use bevy_egui::egui::{self, Color32, CornerRadius, RichText, Stroke, Vec2};

use crate::core::{CollisionGizmoVisibility, EditorSettings, EditorAction, KeyBinding, KeyBindings, SettingsTab, bindable_keys};
use crate::theming::{Theme, ThemeManager};

// Phosphor icons
use egui_phosphor::regular::{
    CARET_DOWN, CARET_RIGHT, DESKTOP, VIDEO_CAMERA, KEYBOARD, PALETTE,
    TEXT_AA, GAUGE, WRENCH, GRID_FOUR, CUBE,
};

/// Background colors for alternating rows (matching inspector)
const ROW_BG_EVEN: Color32 = Color32::from_rgb(32, 34, 38);
const ROW_BG_ODD: Color32 = Color32::from_rgb(38, 40, 44);

/// Width reserved for property labels
const LABEL_WIDTH: f32 = 100.0;

/// Render the settings panel content for docked panels
pub fn render_settings_content(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    settings: &mut EditorSettings,
    keybindings: &mut KeyBindings,
    theme_manager: &mut ThemeManager,
) {
    // Clone the theme to avoid borrow conflicts with theme editor tab
    let theme_clone = theme_manager.active_theme.clone();
    let theme = &theme_clone;

    // Handle key capture for rebinding
    if let Some(action) = keybindings.rebinding {
        capture_key_for_rebind(ctx, keybindings, action);
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.set_width(ui.available_width());

        // Tab bar styled like inspector tabs
        render_tabs_inline(ui, settings, theme);

        ui.add_space(8.0);

        // Tab content
        match settings.settings_tab {
            SettingsTab::General => render_general_tab(ui, settings, theme),
            SettingsTab::Viewport => render_viewport_tab(ui, settings, theme),
            SettingsTab::Shortcuts => render_shortcuts_tab(ui, keybindings, theme),
            SettingsTab::Theme => render_theme_tab(ui, theme_manager),
        }
    });
}

/// Category style for settings sections
struct SettingsCategoryStyle {
    accent_color: Color32,
    header_bg: Color32,
}

impl SettingsCategoryStyle {
    fn interface() -> Self {
        Self {
            accent_color: Color32::from_rgb(99, 178, 238),   // Blue
            header_bg: Color32::from_rgb(35, 45, 55),
        }
    }

    fn camera() -> Self {
        Self {
            accent_color: Color32::from_rgb(178, 132, 209),  // Purple
            header_bg: Color32::from_rgb(42, 38, 52),
        }
    }

    fn developer() -> Self {
        Self {
            accent_color: Color32::from_rgb(236, 154, 120),  // Orange
            header_bg: Color32::from_rgb(50, 40, 38),
        }
    }

    fn grid() -> Self {
        Self {
            accent_color: Color32::from_rgb(134, 188, 126),  // Green
            header_bg: Color32::from_rgb(35, 48, 42),
        }
    }

    fn gizmos() -> Self {
        Self {
            accent_color: Color32::from_rgb(120, 200, 200),  // Cyan
            header_bg: Color32::from_rgb(35, 48, 50),
        }
    }

    fn shortcuts() -> Self {
        Self {
            accent_color: Color32::from_rgb(247, 207, 100),  // Yellow
            header_bg: Color32::from_rgb(50, 45, 35),
        }
    }

    fn theme() -> Self {
        Self {
            accent_color: Color32::from_rgb(191, 166, 242),  // Light purple
            header_bg: Color32::from_rgb(42, 40, 52),
        }
    }
}

/// Renders a styled settings category with header and content (matching inspector style)
fn render_settings_category(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    style: SettingsCategoryStyle,
    id_source: &str,
    default_open: bool,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let id = ui.make_persistent_id(id_source);
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, default_open);

    ui.scope(|ui| {
        // Outer frame for the entire category
        egui::Frame::new()
            .fill(Color32::from_rgb(30, 32, 36))
            .corner_radius(CornerRadius::same(6))
            .show(ui, |ui| {

                // Header bar
                let header_rect = ui.scope(|ui| {
                    egui::Frame::new()
                        .fill(style.header_bg)
                        .corner_radius(CornerRadius {
                            nw: 6,
                            ne: 6,
                            sw: if state.is_open() { 0 } else { 6 },
                            se: if state.is_open() { 0 } else { 6 },
                        })
                        .inner_margin(egui::Margin::symmetric(8, 6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Collapse indicator
                                let caret = if state.is_open() { CARET_DOWN } else { CARET_RIGHT };
                                ui.label(RichText::new(caret).size(12.0).color(Color32::from_rgb(140, 142, 148)));

                                // Icon
                                ui.label(RichText::new(icon).size(15.0).color(style.accent_color));

                                ui.add_space(4.0);

                                // Label
                                ui.label(RichText::new(label).size(13.0).strong().color(Color32::from_rgb(220, 222, 228)));

                                // Fill remaining width
                                ui.allocate_space(ui.available_size());
                            });
                        });
                }).response.rect;

                // Make header clickable
                let header_response = ui.interact(header_rect, id.with("header"), egui::Sense::click());
                if header_response.clicked() {
                    state.toggle(ui);
                }

                // Content area with padding
                if state.is_open() {
                    ui.add_space(4.0);
                    egui::Frame::new()
                        .inner_margin(egui::Margin { left: 4, right: 4, top: 0, bottom: 4 })
                        .show(ui, |ui| {
                            add_contents(ui);
                        });
                }
            });
    });

    state.store(ui.ctx());

    ui.add_space(6.0);
}

/// Helper to render an inline property with label on left, widget on right
fn settings_row<R>(
    ui: &mut egui::Ui,
    row_index: usize,
    label: &str,
    add_widget: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let bg_color = if row_index % 2 == 0 { ROW_BG_EVEN } else { ROW_BG_ODD };
    let available_width = ui.available_width();

    egui::Frame::new()
        .fill(bg_color)
        .inner_margin(egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ui.set_min_width(available_width - 12.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                // Fixed-width label
                ui.add_sized(
                    [LABEL_WIDTH, 18.0],
                    egui::Label::new(egui::RichText::new(label).size(12.0)).truncate()
                );
                // Widget fills remaining space
                add_widget(ui)
            })
            .inner
        })
        .inner
}

/// Render tabs inline for docked panel (horizontal layout)
fn render_tabs_inline(ui: &mut egui::Ui, settings: &mut EditorSettings, theme: &Theme) {
    let text_primary = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let accent_color = theme.semantic.accent.to_color32();

    let tabs = [
        (SettingsTab::General, DESKTOP, "General"),
        (SettingsTab::Viewport, CUBE, "Viewport"),
        (SettingsTab::Shortcuts, KEYBOARD, "Shortcuts"),
        (SettingsTab::Theme, PALETTE, "Theme"),
    ];

    ui.horizontal(|ui| {
        for (tab, icon, label) in tabs.iter() {
            let is_active = settings.settings_tab == *tab;
            let text_color = if is_active { text_primary } else { text_muted };
            let bg_color = if is_active {
                Color32::from_rgb(45, 47, 52)
            } else {
                Color32::from_rgb(35, 37, 42)
            };

            let button = egui::Button::new(
                RichText::new(format!("{} {}", icon, label)).color(text_color).size(12.0)
            )
                .fill(bg_color)
                .corner_radius(CornerRadius::same(4))
                .stroke(if is_active {
                    Stroke::new(1.0, accent_color)
                } else {
                    Stroke::NONE
                });

            if ui.add(button).clicked() {
                settings.settings_tab = *tab;
            }
        }
    });
}

fn render_general_tab(ui: &mut egui::Ui, settings: &mut EditorSettings, _theme: &Theme) {
    // Interface Section
    render_settings_category(
        ui,
        TEXT_AA,
        "Interface",
        SettingsCategoryStyle::interface(),
        "settings_interface",
        true,
        |ui| {
            settings_row(ui, 0, "Font Size", |ui| {
                ui.add(egui::Slider::new(&mut settings.font_size, 10.0..=20.0)
                    .step_by(1.0)
                    .suffix("pt")
                    .show_value(true))
            });
        },
    );

    // Camera Section
    render_settings_category(
        ui,
        VIDEO_CAMERA,
        "Camera",
        SettingsCategoryStyle::camera(),
        "settings_camera",
        true,
        |ui| {
            settings_row(ui, 0, "Move Speed", |ui| {
                ui.add(egui::DragValue::new(&mut settings.camera_move_speed)
                    .range(1.0..=50.0)
                    .speed(0.1))
            });
        },
    );

    // Developer Section
    render_settings_category(
        ui,
        WRENCH,
        "Developer",
        SettingsCategoryStyle::developer(),
        "settings_developer",
        true,
        |ui| {
            settings_row(ui, 0, "Dev Mode", |ui| {
                ui.checkbox(&mut settings.dev_mode, "Enable plugin tools")
            });
        },
    );
}

fn render_viewport_tab(ui: &mut egui::Ui, settings: &mut EditorSettings, _theme: &Theme) {
    // Grid Section
    render_settings_category(
        ui,
        GRID_FOUR,
        "Grid",
        SettingsCategoryStyle::grid(),
        "settings_grid",
        true,
        |ui| {
            settings_row(ui, 0, "Show Grid", |ui| {
                ui.checkbox(&mut settings.show_grid, "")
            });

            settings_row(ui, 1, "Grid Size", |ui| {
                ui.add(egui::DragValue::new(&mut settings.grid_size)
                    .range(1.0..=100.0)
                    .speed(0.5))
            });

            settings_row(ui, 2, "Divisions", |ui| {
                ui.add(egui::DragValue::new(&mut settings.grid_divisions)
                    .range(1..=50))
            });

            settings_row(ui, 3, "Color", |ui| {
                let mut color = [
                    (settings.grid_color[0] * 255.0) as u8,
                    (settings.grid_color[1] * 255.0) as u8,
                    (settings.grid_color[2] * 255.0) as u8,
                ];
                let resp = ui.color_edit_button_srgb(&mut color);
                if resp.changed() {
                    settings.grid_color = [
                        color[0] as f32 / 255.0,
                        color[1] as f32 / 255.0,
                        color[2] as f32 / 255.0,
                    ];
                }
                resp
            });
        },
    );

    // Gizmos Section
    render_settings_category(
        ui,
        GAUGE,
        "Gizmos",
        SettingsCategoryStyle::gizmos(),
        "settings_gizmos",
        true,
        |ui| {
            settings_row(ui, 0, "Colliders", |ui| {
                egui::ComboBox::from_id_salt("collision_gizmo_visibility")
                    .selected_text(match settings.collision_gizmo_visibility {
                        CollisionGizmoVisibility::SelectedOnly => "Selected Only",
                        CollisionGizmoVisibility::Always => "Always",
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
                            "Always",
                        );
                    })
            });
        },
    );
}

fn render_shortcuts_tab(ui: &mut egui::Ui, keybindings: &mut KeyBindings, theme: &Theme) {
    let text_primary = theme.text.primary.to_color32();
    let item_bg = theme.panels.item_bg.to_color32();
    let border_color = theme.widgets.border.to_color32();

    let mut current_category = "";
    let mut category_actions: Vec<(String, Vec<EditorAction>)> = Vec::new();

    // Group actions by category
    for action in EditorAction::all() {
        let category = action.category();
        if category != current_category {
            category_actions.push((category.to_string(), vec![action]));
            current_category = category;
        } else if let Some((_, actions)) = category_actions.last_mut() {
            actions.push(action);
        }
    }

    // Render each category
    for (category, actions) in category_actions {
        render_settings_category(
            ui,
            KEYBOARD,
            &category,
            SettingsCategoryStyle::shortcuts(),
            &format!("shortcuts_{}", category),
            true,
            |ui| {
                for (i, action) in actions.iter().enumerate() {
                    render_keybinding_row(ui, i, keybindings, *action, theme);
                }
            },
        );
    }

    ui.add_space(8.0);

    // Reset to defaults button
    let btn = egui::Button::new(RichText::new("Reset All to Defaults").color(text_primary))
        .fill(item_bg)
        .stroke(Stroke::new(1.0, border_color))
        .corner_radius(CornerRadius::same(6))
        .min_size(Vec2::new(160.0, 28.0));

    if ui.add(btn).clicked() {
        *keybindings = KeyBindings::default();
    }
}

fn render_keybinding_row(
    ui: &mut egui::Ui,
    row_index: usize,
    keybindings: &mut KeyBindings,
    action: EditorAction,
    theme: &Theme
) {
    let text_muted = theme.text.muted.to_color32();
    let accent_color = theme.semantic.accent.to_color32();
    let item_bg = theme.panels.item_bg.to_color32();
    let border_color = theme.widgets.border.to_color32();
    let warning_color = theme.semantic.warning.to_color32();

    let bg_color = if row_index % 2 == 0 { ROW_BG_EVEN } else { ROW_BG_ODD };

    egui::Frame::new()
        .fill(bg_color)
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Action name
                ui.add_sized(
                    [LABEL_WIDTH + 20.0, 18.0],
                    egui::Label::new(egui::RichText::new(action.display_name()).size(12.0)).truncate()
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let is_rebinding = keybindings.rebinding == Some(action);

                    let button_text = if is_rebinding {
                        RichText::new("Press key...").color(warning_color).size(11.0)
                    } else if let Some(binding) = keybindings.get(action) {
                        RichText::new(binding.display()).color(accent_color).monospace().size(11.0)
                    } else {
                        RichText::new("Unbound").color(text_muted).size(11.0)
                    };

                    let rebind_bg = Color32::from_rgb(60, 50, 30);

                    let button = egui::Button::new(button_text)
                        .fill(if is_rebinding { rebind_bg } else { item_bg })
                        .stroke(Stroke::new(1.0, if is_rebinding { warning_color } else { border_color }))
                        .corner_radius(CornerRadius::same(4))
                        .min_size(Vec2::new(90.0, 20.0));

                    if ui.add(button).clicked() {
                        if is_rebinding {
                            keybindings.rebinding = None;
                        } else {
                            keybindings.rebinding = Some(action);
                        }
                    }
                });
            });
        });
}

fn capture_key_for_rebind(ctx: &egui::Context, keybindings: &mut KeyBindings, action: EditorAction) {
    let keys = bindable_keys();

    ctx.input(|input| {
        let ctrl = input.modifiers.ctrl;
        let shift = input.modifiers.shift;
        let alt = input.modifiers.alt;

        for key in &keys {
            let egui_key = keycode_to_egui(*key);
            if let Some(egui_key) = egui_key {
                if input.key_pressed(egui_key) {
                    let mut binding = KeyBinding::new(*key);
                    if ctrl { binding = binding.ctrl(); }
                    if shift { binding = binding.shift(); }
                    if alt { binding = binding.alt(); }
                    keybindings.set(action, binding);
                    keybindings.rebinding = None;
                    return;
                }
            }
        }

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
    let text_primary = theme_manager.active_theme.text.primary.to_color32();
    let _text_heading = theme_manager.active_theme.text.heading.to_color32();
    let item_bg = theme_manager.active_theme.panels.item_bg.to_color32();
    let border_color = theme_manager.active_theme.widgets.border.to_color32();
    let accent_color = theme_manager.active_theme.semantic.accent.to_color32();
    let error_color = theme_manager.active_theme.semantic.error.to_color32();

    // Theme selector section
    render_settings_category(
        ui,
        PALETTE,
        "Active Theme",
        SettingsCategoryStyle::theme(),
        "settings_active_theme",
        true,
        |ui| {
            settings_row(ui, 0, "Theme", |ui| {
                egui::ComboBox::from_id_salt("theme_selector")
                    .selected_text(&theme_manager.active_theme_name)
                    .show_ui(ui, |ui| {
                        for name in theme_manager.available_themes.clone() {
                            if ui.selectable_value(&mut theme_manager.active_theme_name.clone(), name.clone(), &name).clicked() {
                                theme_manager.load_theme(&name);
                            }
                        }
                    })
            });

            ui.add_space(4.0);

            // Action buttons
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(RichText::new("Save As...").color(text_primary).size(11.0))
                    .fill(item_bg)
                    .stroke(Stroke::new(1.0, border_color))
                    .corner_radius(CornerRadius::same(4))
                ).clicked() {
                    unsafe {
                        THEME_SAVE_NAME = Some(format!("{} Copy", theme_manager.active_theme_name));
                    }
                }

                if !theme_manager.is_builtin(&theme_manager.active_theme_name) {
                    if ui.add(egui::Button::new(RichText::new("Save").color(text_primary).size(11.0))
                        .fill(item_bg)
                        .stroke(Stroke::new(1.0, border_color))
                        .corner_radius(CornerRadius::same(4))
                    ).clicked() {
                        let name = theme_manager.active_theme_name.clone();
                        theme_manager.save_theme(&name);
                    }
                }

                if theme_manager.has_unsaved_changes {
                    if ui.add(egui::Button::new(RichText::new("Reset").color(error_color).size(11.0))
                        .fill(item_bg)
                        .stroke(Stroke::new(1.0, border_color))
                        .corner_radius(CornerRadius::same(4))
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
                        .corner_radius(CornerRadius::same(4))
                        .stroke(Stroke::new(1.0, accent_color))
                        .inner_margin(egui::Margin::same(6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Name:").color(text_primary).size(11.0));
                                ui.add(egui::TextEdit::singleline(save_name).desired_width(100.0));

                                if ui.add(egui::Button::new(RichText::new("Create").color(Color32::WHITE).size(11.0))
                                    .fill(accent_color)
                                    .corner_radius(CornerRadius::same(4))
                                ).clicked() && !save_name.is_empty() {
                                    theme_manager.save_theme(save_name);
                                    close_dialog = true;
                                }

                                if ui.add(egui::Button::new(RichText::new("Cancel").size(11.0))
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
        },
    );

    let mut any_modified = false;

    // Semantic Colors
    render_settings_category(
        ui,
        PALETTE,
        "Semantic Colors",
        SettingsCategoryStyle::theme(),
        "theme_semantic",
        false,
        |ui| {
            let theme = &mut theme_manager.active_theme;
            any_modified |= theme_color_row(ui, 0, "Accent", &mut theme.semantic.accent);
            any_modified |= theme_color_row(ui, 1, "Success", &mut theme.semantic.success);
            any_modified |= theme_color_row(ui, 2, "Warning", &mut theme.semantic.warning);
            any_modified |= theme_color_row(ui, 3, "Error", &mut theme.semantic.error);
            any_modified |= theme_color_row(ui, 4, "Selection", &mut theme.semantic.selection);
            any_modified |= theme_color_row(ui, 5, "Selection Stroke", &mut theme.semantic.selection_stroke);
        },
    );

    // Surface Colors
    render_settings_category(
        ui,
        PALETTE,
        "Surfaces",
        SettingsCategoryStyle::theme(),
        "theme_surfaces",
        false,
        |ui| {
            let theme = &mut theme_manager.active_theme;
            any_modified |= theme_color_row(ui, 0, "Window", &mut theme.surfaces.window);
            any_modified |= theme_color_row(ui, 1, "Window Stroke", &mut theme.surfaces.window_stroke);
            any_modified |= theme_color_row(ui, 2, "Panel", &mut theme.surfaces.panel);
            any_modified |= theme_color_row(ui, 3, "Popup", &mut theme.surfaces.popup);
            any_modified |= theme_color_row(ui, 4, "Faint", &mut theme.surfaces.faint);
            any_modified |= theme_color_row(ui, 5, "Extreme", &mut theme.surfaces.extreme);
        },
    );

    // Text Colors
    render_settings_category(
        ui,
        TEXT_AA,
        "Text",
        SettingsCategoryStyle::interface(),
        "theme_text",
        false,
        |ui| {
            let theme = &mut theme_manager.active_theme;
            any_modified |= theme_color_row(ui, 0, "Primary", &mut theme.text.primary);
            any_modified |= theme_color_row(ui, 1, "Secondary", &mut theme.text.secondary);
            any_modified |= theme_color_row(ui, 2, "Muted", &mut theme.text.muted);
            any_modified |= theme_color_row(ui, 3, "Heading", &mut theme.text.heading);
            any_modified |= theme_color_row(ui, 4, "Disabled", &mut theme.text.disabled);
            any_modified |= theme_color_row(ui, 5, "Hyperlink", &mut theme.text.hyperlink);
        },
    );

    // Widget Colors
    render_settings_category(
        ui,
        CUBE,
        "Widgets",
        SettingsCategoryStyle::gizmos(),
        "theme_widgets",
        false,
        |ui| {
            let theme = &mut theme_manager.active_theme;
            any_modified |= theme_color_row(ui, 0, "Inactive BG", &mut theme.widgets.inactive_bg);
            any_modified |= theme_color_row(ui, 1, "Inactive FG", &mut theme.widgets.inactive_fg);
            any_modified |= theme_color_row(ui, 2, "Hovered BG", &mut theme.widgets.hovered_bg);
            any_modified |= theme_color_row(ui, 3, "Hovered FG", &mut theme.widgets.hovered_fg);
            any_modified |= theme_color_row(ui, 4, "Active BG", &mut theme.widgets.active_bg);
            any_modified |= theme_color_row(ui, 5, "Active FG", &mut theme.widgets.active_fg);
            any_modified |= theme_color_row(ui, 6, "Border", &mut theme.widgets.border);
        },
    );

    // Panel Colors
    render_settings_category(
        ui,
        DESKTOP,
        "Panels",
        SettingsCategoryStyle::interface(),
        "theme_panels",
        false,
        |ui| {
            let theme = &mut theme_manager.active_theme;
            any_modified |= theme_color_row(ui, 0, "Tree Line", &mut theme.panels.tree_line);
            any_modified |= theme_color_row(ui, 1, "Drop Line", &mut theme.panels.drop_line);
            any_modified |= theme_color_row(ui, 2, "Tab Active", &mut theme.panels.tab_active);
            any_modified |= theme_color_row(ui, 3, "Tab Inactive", &mut theme.panels.tab_inactive);
        },
    );

    if any_modified {
        theme_manager.mark_modified();
    }
}

/// Render a theme color row with alternating background
fn theme_color_row(ui: &mut egui::Ui, row_index: usize, label: &str, color: &mut crate::theming::ThemeColor) -> bool {
    let mut changed = false;
    let bg_color = if row_index % 2 == 0 { ROW_BG_EVEN } else { ROW_BG_ODD };

    egui::Frame::new()
        .fill(bg_color)
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_sized(
                    [LABEL_WIDTH, 18.0],
                    egui::Label::new(egui::RichText::new(label).size(12.0)).truncate()
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let [r, g, b, a] = color.0.to_array();
                    let mut srgba = [r, g, b, a];
                    if ui.color_edit_button_srgba_unmultiplied(&mut srgba).changed() {
                        *color = crate::theming::ThemeColor::with_alpha(srgba[0], srgba[1], srgba[2], srgba[3]);
                        changed = true;
                    }
                });
            });
        });

    changed
}
