use bevy::prelude::KeyCode;
use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, RichText, Stroke, Vec2};

use crate::core::{CollisionGizmoVisibility, EditorSettings, EditorAction, KeyBinding, KeyBindings, MonoFont, SelectionHighlightMode, SettingsTab, SceneManagerState, UiFont, bindable_keys};
use crate::plugin_core::{PluginHost, PluginSource};
use crate::project::AppConfig;
use renzora_theme::{Theme, ThemeManager};
use crate::update::{UpdateState, UpdateDialogState};

// Phosphor icons
use egui_phosphor::regular::{
    CARET_DOWN, CARET_RIGHT, CODE, DESKTOP, VIDEO_CAMERA, KEYBOARD, PALETTE,
    TEXT_AA, GAUGE, WRENCH, GRID_FOUR, CUBE, ARROW_CLOCKWISE,
    DOWNLOAD_SIMPLE, CHECK_CIRCLE, CHECK, WARNING, FLOPPY_DISK, PUZZLE_PIECE,
};

/// Width reserved for property labels
const LABEL_WIDTH: f32 = 100.0;

/// Render the settings panel content for docked panels
pub fn render_settings_content(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    settings: &mut EditorSettings,
    keybindings: &mut KeyBindings,
    theme_manager: &mut ThemeManager,
    app_config: &mut AppConfig,
    update_state: &mut UpdateState,
    update_dialog: &mut UpdateDialogState,
    scene_state: &mut SceneManagerState,
    plugin_host: &mut PluginHost,
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
            SettingsTab::General => render_general_tab(ui, settings, scene_state, theme),
            SettingsTab::Viewport => render_viewport_tab(ui, settings, theme),
            SettingsTab::Shortcuts => render_shortcuts_tab(ui, keybindings, theme),
            SettingsTab::Theme => render_theme_tab(ui, theme_manager),
            SettingsTab::Plugins => render_plugins_tab(ui, plugin_host, app_config, theme),
            SettingsTab::Updates => render_updates_tab(ui, app_config, update_state, update_dialog, theme),
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

    fn updates() -> Self {
        Self {
            accent_color: Color32::from_rgb(100, 200, 160),  // Teal
            header_bg: Color32::from_rgb(35, 50, 48),
        }
    }

    fn auto_save() -> Self {
        Self {
            accent_color: Color32::from_rgb(120, 180, 230),  // Light blue
            header_bg: Color32::from_rgb(35, 42, 52),
        }
    }

    fn scripting() -> Self {
        Self {
            accent_color: Color32::from_rgb(100, 200, 140),  // Green/teal
            header_bg: Color32::from_rgb(35, 50, 42),
        }
    }

    fn plugins() -> Self {
        Self {
            accent_color: Color32::from_rgb(180, 140, 220),  // Purple
            header_bg: Color32::from_rgb(44, 38, 54),
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
    theme: &Theme,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let id = ui.make_persistent_id(id_source);
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, default_open);

    let frame_bg = theme.surfaces.extreme.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let text_heading = theme.text.heading.to_color32();

    ui.scope(|ui| {
        // Outer frame for the entire category
        egui::Frame::new()
            .fill(frame_bg)
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
                                ui.label(RichText::new(caret).size(12.0).color(text_muted));

                                // Icon
                                ui.label(RichText::new(icon).size(15.0).color(style.accent_color));

                                ui.add_space(4.0);

                                // Label
                                ui.label(RichText::new(label).size(13.0).strong().color(text_heading));

                                // Fill remaining width
                                ui.allocate_space(ui.available_size());
                            });
                        });
                }).response.rect;

                // Make header clickable
                let header_response = ui.interact(header_rect, id.with("header"), egui::Sense::click());
                if header_response.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
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
    theme: &Theme,
    add_widget: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let bg_color = if row_index % 2 == 0 {
        theme.panels.inspector_row_even.to_color32()
    } else {
        theme.panels.inspector_row_odd.to_color32()
    };
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
        (SettingsTab::Plugins, PUZZLE_PIECE, "Plugins"),
        (SettingsTab::Updates, ARROW_CLOCKWISE, "Updates"),
    ];

    let tab_active_bg = theme.panels.tab_active.to_color32();
    let tab_inactive_bg = theme.panels.tab_inactive.to_color32();

    ui.horizontal(|ui| {
        for (tab, icon, label) in tabs.iter() {
            let is_active = settings.settings_tab == *tab;
            let text_color = if is_active { text_primary } else { text_muted };
            let bg_color = if is_active { tab_active_bg } else { tab_inactive_bg };

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

fn render_general_tab(ui: &mut egui::Ui, settings: &mut EditorSettings, scene_state: &mut SceneManagerState, theme: &Theme) {
    let text_muted = theme.text.muted.to_color32();

    // Interface / Font Section
    render_settings_category(
        ui,
        TEXT_AA,
        "Interface",
        SettingsCategoryStyle::interface(),
        "settings_interface",
        true,
        theme,
        |ui| {
            settings_row(ui, 0, "UI Font", theme, |ui| {
                let prev = settings.ui_font;
                egui::ComboBox::from_id_salt("ui_font_selector")
                    .selected_text(settings.ui_font.label())
                    .show_ui(ui, |ui| {
                        for &font in UiFont::ALL {
                            ui.selectable_value(&mut settings.ui_font, font, font.label());
                        }
                    });
                if settings.ui_font != prev {
                    crate::ui::style::set_ui_font(ui.ctx(), settings.ui_font);
                }
            });

            settings_row(ui, 1, "Code Font", theme, |ui| {
                let prev = settings.mono_font;
                egui::ComboBox::from_id_salt("mono_font_selector")
                    .selected_text(settings.mono_font.label())
                    .show_ui(ui, |ui| {
                        for &font in MonoFont::ALL {
                            ui.selectable_value(&mut settings.mono_font, font, font.label());
                        }
                    });
                if settings.mono_font != prev {
                    crate::ui::style::set_mono_font(ui.ctx(), settings.mono_font);
                }
            });

            settings_row(ui, 2, "Font Size", theme, |ui| {
                ui.add(egui::DragValue::new(&mut settings.font_size)
                    .range(10.0..=24.0)
                    .speed(0.5)
                    .suffix(" px"))
            });
        },
    );

    // Auto Save Section
    render_settings_category(
        ui,
        FLOPPY_DISK,
        "Auto Save",
        SettingsCategoryStyle::auto_save(),
        "settings_auto_save",
        true,
        theme,
        |ui| {
            settings_row(ui, 0, "Enabled", theme, |ui| {
                ui.checkbox(&mut scene_state.auto_save_enabled, "Auto-save when modified")
            });

            settings_row(ui, 1, "Interval", theme, |ui| {
                ui.horizontal(|ui| {
                    let response = ui.add(egui::DragValue::new(&mut scene_state.auto_save_interval)
                        .range(10.0..=600.0)
                        .speed(1.0)
                        .suffix(" sec"));

                    // Show human-readable time
                    let mins = (scene_state.auto_save_interval / 60.0).floor() as i32;
                    let secs = (scene_state.auto_save_interval % 60.0) as i32;
                    let time_str = if mins > 0 {
                        format!("({}m {}s)", mins, secs)
                    } else {
                        format!("({}s)", secs)
                    };
                    ui.label(egui::RichText::new(time_str).size(11.0).color(text_muted));
                    response
                }).inner
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
        theme,
        |ui| {
            settings_row(ui, 0, "Move Speed", theme, |ui| {
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
        theme,
        |ui| {
            settings_row(ui, 0, "Dev Mode", theme, |ui| {
                ui.checkbox(&mut settings.dev_mode, "Enable plugin tools")
            });
        },
    );

    // Scripting Section
    render_settings_category(
        ui,
        CODE,
        "Scripting",
        SettingsCategoryStyle::scripting(),
        "settings_scripting",
        true,
        theme,
        |ui| {
            settings_row(ui, 0, "Hot Reload", theme, |ui| {
                ui.checkbox(&mut settings.script_rerun_on_ready_on_reload, "Re-run on_ready on hot reload")
            });
        },
    );
}

fn render_viewport_tab(ui: &mut egui::Ui, settings: &mut EditorSettings, theme: &Theme) {
    // Grid Section
    render_settings_category(
        ui,
        GRID_FOUR,
        "Grid",
        SettingsCategoryStyle::grid(),
        "settings_grid",
        true,
        theme,
        |ui| {
            settings_row(ui, 0, "Show Grid", theme, |ui| {
                ui.checkbox(&mut settings.show_grid, "")
            });

            settings_row(ui, 1, "Grid Size", theme, |ui| {
                ui.add(egui::DragValue::new(&mut settings.grid_size)
                    .range(1.0..=100.0)
                    .speed(0.5))
            });

            settings_row(ui, 2, "Divisions", theme, |ui| {
                ui.add(egui::DragValue::new(&mut settings.grid_divisions)
                    .range(1..=50))
            });

            settings_row(ui, 3, "Color", theme, |ui| {
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
        theme,
        |ui| {
            settings_row(ui, 0, "Colliders", theme, |ui| {
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
            settings_row(ui, 1, "Selection", theme, |ui| {
                egui::ComboBox::from_id_salt("selection_highlight_mode")
                    .selected_text(match settings.selection_highlight_mode {
                        SelectionHighlightMode::Outline => "Outline",
                        SelectionHighlightMode::Gizmo => "Gizmo",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut settings.selection_highlight_mode,
                            SelectionHighlightMode::Outline,
                            "Outline",
                        );
                        ui.selectable_value(
                            &mut settings.selection_highlight_mode,
                            SelectionHighlightMode::Gizmo,
                            "Gizmo",
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
            theme,
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

    let bg_color = if row_index % 2 == 0 {
        theme.panels.inspector_row_even.to_color32()
    } else {
        theme.panels.inspector_row_odd.to_color32()
    };

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

                    // Use a warmer background when rebinding
                    let rebind_bg = theme.semantic.warning.to_color32().gamma_multiply(0.3);

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
    let row_even = theme_manager.active_theme.panels.inspector_row_even.to_color32();
    let row_odd = theme_manager.active_theme.panels.inspector_row_odd.to_color32();

    // Clone theme for passing to render_settings_category
    let theme_clone = theme_manager.active_theme.clone();
    let theme = &theme_clone;

    // Theme selector section
    render_settings_category(
        ui,
        PALETTE,
        "Active Theme",
        SettingsCategoryStyle::theme(),
        "settings_active_theme",
        true,
        theme,
        |ui| {
            settings_row(ui, 0, "Theme", theme, |ui| {
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
        theme,
        |ui| {
            let t = &mut theme_manager.active_theme;
            any_modified |= theme_color_row(ui, 0, "Accent", &mut t.semantic.accent, row_even, row_odd);
            any_modified |= theme_color_row(ui, 1, "Success", &mut t.semantic.success, row_even, row_odd);
            any_modified |= theme_color_row(ui, 2, "Warning", &mut t.semantic.warning, row_even, row_odd);
            any_modified |= theme_color_row(ui, 3, "Error", &mut t.semantic.error, row_even, row_odd);
            any_modified |= theme_color_row(ui, 4, "Selection", &mut t.semantic.selection, row_even, row_odd);
            any_modified |= theme_color_row(ui, 5, "Selection Stroke", &mut t.semantic.selection_stroke, row_even, row_odd);
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
        theme,
        |ui| {
            let t = &mut theme_manager.active_theme;
            any_modified |= theme_color_row(ui, 0, "Window", &mut t.surfaces.window, row_even, row_odd);
            any_modified |= theme_color_row(ui, 1, "Window Stroke", &mut t.surfaces.window_stroke, row_even, row_odd);
            any_modified |= theme_color_row(ui, 2, "Panel", &mut t.surfaces.panel, row_even, row_odd);
            any_modified |= theme_color_row(ui, 3, "Popup", &mut t.surfaces.popup, row_even, row_odd);
            any_modified |= theme_color_row(ui, 4, "Faint", &mut t.surfaces.faint, row_even, row_odd);
            any_modified |= theme_color_row(ui, 5, "Extreme", &mut t.surfaces.extreme, row_even, row_odd);
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
        theme,
        |ui| {
            let t = &mut theme_manager.active_theme;
            any_modified |= theme_color_row(ui, 0, "Primary", &mut t.text.primary, row_even, row_odd);
            any_modified |= theme_color_row(ui, 1, "Secondary", &mut t.text.secondary, row_even, row_odd);
            any_modified |= theme_color_row(ui, 2, "Muted", &mut t.text.muted, row_even, row_odd);
            any_modified |= theme_color_row(ui, 3, "Heading", &mut t.text.heading, row_even, row_odd);
            any_modified |= theme_color_row(ui, 4, "Disabled", &mut t.text.disabled, row_even, row_odd);
            any_modified |= theme_color_row(ui, 5, "Hyperlink", &mut t.text.hyperlink, row_even, row_odd);
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
        theme,
        |ui| {
            let t = &mut theme_manager.active_theme;
            any_modified |= theme_color_row(ui, 0, "Inactive BG", &mut t.widgets.inactive_bg, row_even, row_odd);
            any_modified |= theme_color_row(ui, 1, "Inactive FG", &mut t.widgets.inactive_fg, row_even, row_odd);
            any_modified |= theme_color_row(ui, 2, "Hovered BG", &mut t.widgets.hovered_bg, row_even, row_odd);
            any_modified |= theme_color_row(ui, 3, "Hovered FG", &mut t.widgets.hovered_fg, row_even, row_odd);
            any_modified |= theme_color_row(ui, 4, "Active BG", &mut t.widgets.active_bg, row_even, row_odd);
            any_modified |= theme_color_row(ui, 5, "Active FG", &mut t.widgets.active_fg, row_even, row_odd);
            any_modified |= theme_color_row(ui, 6, "Border", &mut t.widgets.border, row_even, row_odd);
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
        theme,
        |ui| {
            let t = &mut theme_manager.active_theme;
            any_modified |= theme_color_row(ui, 0, "Tree Line", &mut t.panels.tree_line, row_even, row_odd);
            any_modified |= theme_color_row(ui, 1, "Drop Line", &mut t.panels.drop_line, row_even, row_odd);
            any_modified |= theme_color_row(ui, 2, "Tab Active", &mut t.panels.tab_active, row_even, row_odd);
            any_modified |= theme_color_row(ui, 3, "Tab Inactive", &mut t.panels.tab_inactive, row_even, row_odd);
        },
    );

    if any_modified {
        theme_manager.mark_modified();
    }
}

/// Render a theme color row with alternating background
fn theme_color_row(ui: &mut egui::Ui, row_index: usize, label: &str, color: &mut renzora_theme::ThemeColor, row_even: Color32, row_odd: Color32) -> bool {
    let mut changed = false;
    let bg_color = if row_index % 2 == 0 { row_even } else { row_odd };

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
                        *color = renzora_theme::ThemeColor::with_alpha(srgba[0], srgba[1], srgba[2], srgba[3]);
                        changed = true;
                    }
                });
            });
        });

    changed
}

fn render_plugins_tab(ui: &mut egui::Ui, plugin_host: &mut PluginHost, app_config: &mut AppConfig, theme: &Theme) {
    let text_muted = theme.text.muted.to_color32();

    // Collect all plugins (loaded + disabled) grouped by source
    let mut system_plugins: Vec<(String, String, String, bool)> = Vec::new();
    let mut project_plugins: Vec<(String, String, String, bool)> = Vec::new();

    for (manifest, source, enabled) in plugin_host.all_plugins() {
        let entry = (manifest.id.clone(), manifest.name.clone(), manifest.version.clone(), enabled);
        match source {
            PluginSource::System => system_plugins.push(entry),
            PluginSource::Project => project_plugins.push(entry),
        }
    }

    system_plugins.sort_by(|a, b| a.1.cmp(&b.1));
    project_plugins.sort_by(|a, b| a.1.cmp(&b.1));

    // Collect toggle actions to apply after rendering (avoid borrow conflicts)
    let mut toggle_actions: Vec<(String, bool)> = Vec::new();

    // System Plugins
    render_settings_category(
        ui,
        PUZZLE_PIECE,
        "System Plugins",
        SettingsCategoryStyle::plugins(),
        "settings_system_plugins",
        true,
        theme,
        |ui| {
            if system_plugins.is_empty() {
                ui.label(RichText::new("No system plugins installed").size(12.0).color(text_muted));
            } else {
                for (i, (id, name, version, enabled)) in system_plugins.iter().enumerate() {
                    if let Some(action) = render_plugin_row(ui, i, id, name, version, *enabled, theme) {
                        toggle_actions.push(action);
                    }
                }
            }
        },
    );

    // Project Plugins
    render_settings_category(
        ui,
        PUZZLE_PIECE,
        "Project Plugins",
        SettingsCategoryStyle::plugins(),
        "settings_project_plugins",
        true,
        theme,
        |ui| {
            if project_plugins.is_empty() {
                ui.label(RichText::new("No project plugins installed").size(12.0).color(text_muted));
            } else {
                for (i, (id, name, version, enabled)) in project_plugins.iter().enumerate() {
                    if let Some(action) = render_plugin_row(ui, i, id, name, version, *enabled, theme) {
                        toggle_actions.push(action);
                    }
                }
            }
        },
    );

    // Apply toggle actions and persist
    for (plugin_id, enable) in toggle_actions {
        if enable {
            plugin_host.enable_plugin(&plugin_id);
            app_config.disabled_plugins.retain(|id| id != &plugin_id);
        } else {
            plugin_host.disable_plugin(&plugin_id);
            if !app_config.disabled_plugins.contains(&plugin_id) {
                app_config.disabled_plugins.push(plugin_id);
            }
        }
        let _ = app_config.save();
    }
}

/// Returns Some((plugin_id, new_enabled_state)) if the checkbox was toggled
fn render_plugin_row(
    ui: &mut egui::Ui,
    row_index: usize,
    plugin_id: &str,
    name: &str,
    version: &str,
    enabled: bool,
    theme: &Theme,
) -> Option<(String, bool)> {
    let text_muted = theme.text.muted.to_color32();

    let bg_color = if row_index % 2 == 0 {
        theme.panels.inspector_row_even.to_color32()
    } else {
        theme.panels.inspector_row_odd.to_color32()
    };

    let mut toggled = None;

    egui::Frame::new()
        .fill(bg_color)
        .inner_margin(egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let mut is_enabled = enabled;
                if ui.checkbox(&mut is_enabled, "").changed() {
                    toggled = Some((plugin_id.to_string(), is_enabled));
                }

                ui.label(RichText::new(name).size(12.0));
                ui.label(RichText::new(format!("v{}", version)).size(11.0).color(text_muted));
            });
        });

    toggled
}

fn render_updates_tab(
    ui: &mut egui::Ui,
    app_config: &mut AppConfig,
    update_state: &mut UpdateState,
    update_dialog: &mut UpdateDialogState,
    theme: &Theme,
) {
    let text_primary = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let accent = theme.semantic.accent.to_color32();
    let success = theme.semantic.success.to_color32();
    let item_bg = theme.panels.item_bg.to_color32();
    let border = theme.widgets.border.to_color32();

    // Update Settings
    render_settings_category(
        ui,
        ARROW_CLOCKWISE,
        "Update Settings",
        SettingsCategoryStyle::updates(),
        "settings_updates",
        true,
        theme,
        |ui| {
            settings_row(ui, 0, "Auto-check", theme, |ui| {
                if ui.checkbox(&mut app_config.update_config.auto_check, "Check on startup").changed() {
                    let _ = app_config.save();
                }
            });

            settings_row(ui, 1, "Current Version", theme, |ui| {
                ui.label(RichText::new(crate::update::current_version()).size(12.0).color(text_primary))
            });
        },
    );

    // Check for Updates section
    render_settings_category(
        ui,
        DOWNLOAD_SIMPLE,
        "Check for Updates",
        SettingsCategoryStyle::updates(),
        "settings_check_updates",
        true,
        theme,
        |ui| {
            // Status display
            if update_state.checking {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(14.0).color(accent));
                    ui.add_space(8.0);
                    ui.label(RichText::new("Checking for updates...").size(12.0).color(text_muted));
                });
            } else if let Some(ref result) = update_state.check_result {
                if result.update_available {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(CHECK_CIRCLE).size(14.0).color(success));
                        ui.add_space(4.0);
                        if let Some(ref version) = result.latest_version {
                            ui.label(RichText::new(format!("Version {} available!", version)).size(12.0).color(success));
                        } else {
                            ui.label(RichText::new("Update available!").size(12.0).color(success));
                        }
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(CHECK).size(14.0).color(success));
                        ui.add_space(4.0);
                        ui.label(RichText::new("You're up to date").size(12.0).color(text_muted));
                    });
                }
            } else if let Some(ref err) = update_state.error {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(WARNING).size(14.0).color(theme.semantic.error.to_color32()));
                    ui.add_space(4.0);
                    ui.label(RichText::new(err).size(12.0).color(theme.semantic.error.to_color32()));
                });
            }

            ui.add_space(8.0);

            // Buttons
            ui.horizontal(|ui| {
                // Check for Updates button
                let check_enabled = !update_state.checking;
                if ui.add_enabled(check_enabled,
                    egui::Button::new(RichText::new("Check for Updates").size(12.0).color(text_primary))
                        .fill(item_bg)
                        .stroke(Stroke::new(1.0, border))
                        .corner_radius(CornerRadius::same(4))
                        .min_size(Vec2::new(130.0, 26.0))
                ).clicked() {
                    update_state.start_check();
                }

                // View Details button (if update available)
                if let Some(ref result) = update_state.check_result {
                    if result.update_available {
                        if ui.add(
                            egui::Button::new(RichText::new("View Details").size(12.0).color(Color32::WHITE))
                                .fill(accent)
                                .corner_radius(CornerRadius::same(4))
                                .min_size(Vec2::new(100.0, 26.0))
                        ).clicked() {
                            update_dialog.open = true;
                        }
                    }
                }
            });

            // Skipped version info
            let skipped_version = app_config.update_config.skipped_version.clone();
            if let Some(skipped) = skipped_version {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Skipped version: {}", skipped)).size(11.0).color(text_muted));
                    if ui.small_button("Reset").clicked() {
                        app_config.update_config.skipped_version = None;
                        let _ = app_config.save();
                    }
                });
            }
        },
    );
}
