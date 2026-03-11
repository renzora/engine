//! Renzora Settings — floating overlay window for editor settings.
//!
//! Reads from decentralized resources (`EditorSettings`, `KeyBindings`,
//! `ViewportSettings`, `ThemeManager`) and writes back via direct mutation.

use bevy::prelude::*;
use bevy::ecs::system::SystemState;
use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, RichText, Stroke, Vec2};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass};

use egui_phosphor::regular::{
    CARET_DOWN, CARET_RIGHT, CODE, DESKTOP, FOLDER_OPEN, VIDEO_CAMERA, KEYBOARD,
    PALETTE, TEXT_AA, GAUGE, WRENCH, GRID_FOUR, CUBE,
};

use renzora_editor::{EditorSettings, MonoFont, SelectionHighlightMode, SettingsTab, UiFont};
use renzora_keybindings::{bindable_keys, EditorAction, KeyBinding, KeyBindings};
use renzora_theme::{Theme, ThemeManager};
use renzora_viewport::settings::{CollisionGizmoVisibility, ViewportSettings};

const LABEL_WIDTH: f32 = 100.0;

// ── Plugin ──────────────────────────────────────────────────────────────────

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] SettingsPlugin");
        app.add_systems(
            EguiPrimaryContextPass,
            settings_overlay_system
                .run_if(in_state(renzora_splash::SplashState::Editor)),
        );
    }
}

// ── Overlay system ──────────────────────────────────────────────────────────

#[derive(Resource)]
struct SettingsEguiState(SystemState<EguiContexts<'static, 'static>>);

fn settings_overlay_system(world: &mut World) {
    // Early-out if settings not visible
    let show = world
        .get_resource::<EditorSettings>()
        .map_or(false, |s| s.show_settings);
    if !show {
        return;
    }

    // Get egui context
    if !world.contains_resource::<SettingsEguiState>() {
        let s = SettingsEguiState(SystemState::new(world));
        world.insert_resource(s);
    }
    let mut cached = world.remove_resource::<SettingsEguiState>().unwrap();
    let mut contexts = cached.0.get_mut(world);
    let Ok(ctx) = contexts.ctx_mut() else {
        world.insert_resource(cached);
        return;
    };
    let ctx = ctx.clone();
    cached.0.apply(world);
    world.insert_resource(cached);

    draw_settings_overlay(world, &ctx);
}

fn draw_settings_overlay(world: &mut World, ctx: &egui::Context) {
    let theme = match world.get_resource::<ThemeManager>() {
        Some(tm) => tm.active_theme.clone(),
        None => return,
    };

    // Read snapshots
    let settings = world.get_resource::<EditorSettings>().cloned().unwrap_or_default();
    let keybindings = world.get_resource::<KeyBindings>().cloned().unwrap_or_default();
    let viewport_settings = world.get_resource::<ViewportSettings>().cloned().unwrap_or_default();

    // Project config snapshot + available scene files
    let (project_config, scene_files) = if let Some(project) = world.get_resource::<renzora_core::CurrentProject>() {
        let scenes_dir = project.resolve_path("scenes");
        let files: Vec<String> = std::fs::read_dir(&scenes_dir)
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("ron") {
                    Some(format!("scenes/{}", path.file_name()?.to_str()?))
                } else {
                    None
                }
            })
            .collect();
        (Some(project.config.clone()), files)
    } else {
        (None, Vec::new())
    };
    let mut project_config_mut = project_config.clone();

    // Clone theme data for the theme tab
    let (theme_name, available_themes) = world
        .get_resource::<ThemeManager>()
        .map(|tm| (tm.active_theme_name.clone(), tm.available_themes.clone()))
        .unwrap_or_default();
    let mut theme_edit = ThemeEditState {
        active_name: theme_name.clone(),
        available: available_themes,
        theme: theme.clone(),
        save_requested: false,
    };

    let mut settings_mut = settings.clone();
    let mut keybindings_mut = keybindings.clone();
    let mut viewport_mut = viewport_settings.clone();

    // Handle key capture for rebinding
    if let Some(action) = keybindings_mut.rebinding {
        capture_key_for_rebind(ctx, &mut keybindings_mut, action);
    }

    let mut open = true;
    let screen = ctx.input(|i| i.screen_rect());
    let default_size = egui::Vec2::new(420.0, 500.0);
    let default_pos = egui::Pos2::new(
        (screen.width() - default_size.x) / 2.0,
        (screen.height() - default_size.y) / 2.0,
    );

    egui::Window::new("Settings")
        .open(&mut open)
        .default_size(default_size)
        .default_pos(default_pos)
        .resizable(true)
        .collapsible(false)
        .frame(egui::Frame::window(&ctx.style()).fill(theme.surfaces.panel.to_color32()))
        .show(ctx, |ui| {
            render_tabs_inline(ui, &mut settings_mut, &theme);
            ui.add_space(8.0);

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());

                match settings_mut.settings_tab {
                    SettingsTab::General => render_general_tab(ui, &mut settings_mut, &mut project_config_mut, &scene_files, &theme),
                    SettingsTab::Viewport => render_viewport_tab(ui, &mut settings_mut, &mut viewport_mut, &theme),
                    SettingsTab::Shortcuts => render_shortcuts_tab(ui, &mut keybindings_mut, &theme),
                    SettingsTab::Theme => render_theme_tab(ui, &mut theme_edit, &theme),
                    SettingsTab::Plugins => render_placeholder_tab(ui, &theme, "Plugins", "Plugin management coming soon."),
                    SettingsTab::Updates => render_placeholder_tab(ui, &theme, "Updates", "Update checking coming soon."),
                }
            });
        });

    if !open {
        settings_mut.show_settings = false;
    }

    // Write back mutations
    if settings_mut != settings {
        if let Some(mut res) = world.get_resource_mut::<EditorSettings>() {
            *res = settings_mut;
        }
    }

    if keybindings_mut.bindings != keybindings.bindings || keybindings_mut.rebinding != keybindings.rebinding {
        if let Some(mut res) = world.get_resource_mut::<KeyBindings>() {
            *res = keybindings_mut;
        }
    }

    if viewport_mut != viewport_settings {
        if let Some(mut res) = world.get_resource_mut::<ViewportSettings>() {
            *res = viewport_mut;
        }
    }

    // Write back project config changes
    if project_config_mut != project_config {
        if let Some(new_config) = project_config_mut {
            if let Some(mut project) = world.get_resource_mut::<renzora_core::CurrentProject>() {
                project.config = new_config;
                if let Err(e) = project.save_config() {
                    warn!("Failed to save project.toml: {}", e);
                }
            }
        }
    }

    // Write back theme changes
    if theme_edit.active_name != theme_name {
        if let Some(mut tm) = world.get_resource_mut::<ThemeManager>() {
            tm.load_theme(&theme_edit.active_name);
        }
    } else if theme_edit.theme != theme {
        if let Some(mut tm) = world.get_resource_mut::<ThemeManager>() {
            tm.active_theme = theme_edit.theme;
            tm.mark_modified();
        }
    }

    if theme_edit.save_requested {
        if let Some(mut tm) = world.get_resource_mut::<ThemeManager>() {
            let name = tm.active_theme_name.clone();
            tm.save_theme(&name);
        }
    }
}

// ── Category + row helpers ──────────────────────────────────────────────────

struct CategoryStyle {
    accent_color: Color32,
    header_bg: Color32,
}

impl CategoryStyle {
    fn interface() -> Self { Self { accent_color: Color32::from_rgb(99, 178, 238), header_bg: Color32::from_rgb(35, 45, 55) } }
    fn camera() -> Self { Self { accent_color: Color32::from_rgb(178, 132, 209), header_bg: Color32::from_rgb(42, 38, 52) } }
    fn developer() -> Self { Self { accent_color: Color32::from_rgb(236, 154, 120), header_bg: Color32::from_rgb(50, 40, 38) } }
    fn grid() -> Self { Self { accent_color: Color32::from_rgb(134, 188, 126), header_bg: Color32::from_rgb(35, 48, 42) } }
    fn gizmos() -> Self { Self { accent_color: Color32::from_rgb(120, 200, 200), header_bg: Color32::from_rgb(35, 48, 50) } }
    fn shortcuts() -> Self { Self { accent_color: Color32::from_rgb(247, 207, 100), header_bg: Color32::from_rgb(50, 45, 35) } }
    fn theme() -> Self { Self { accent_color: Color32::from_rgb(191, 166, 242), header_bg: Color32::from_rgb(42, 40, 52) } }
    fn scripting() -> Self { Self { accent_color: Color32::from_rgb(100, 200, 140), header_bg: Color32::from_rgb(35, 50, 42) } }
}

fn render_category(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    style: CategoryStyle,
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
        egui::Frame::new()
            .fill(frame_bg)
            .corner_radius(CornerRadius::same(6))
            .show(ui, |ui| {
                let header_rect = ui.scope(|ui| {
                    egui::Frame::new()
                        .fill(style.header_bg)
                        .corner_radius(CornerRadius {
                            nw: 6, ne: 6,
                            sw: if state.is_open() { 0 } else { 6 },
                            se: if state.is_open() { 0 } else { 6 },
                        })
                        .inner_margin(egui::Margin::symmetric(8, 6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let caret = if state.is_open() { CARET_DOWN } else { CARET_RIGHT };
                                ui.label(RichText::new(caret).size(12.0).color(text_muted));
                                ui.label(RichText::new(icon).size(15.0).color(style.accent_color));
                                ui.add_space(4.0);
                                ui.label(RichText::new(label).size(13.0).strong().color(text_heading));
                                ui.allocate_space(ui.available_size());
                            });
                        });
                }).response.rect;

                let header_response = ui.interact(header_rect, id.with("header"), egui::Sense::click());
                if header_response.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                if header_response.clicked() {
                    state.toggle(ui);
                }

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
                ui.add_sized(
                    [LABEL_WIDTH, 18.0],
                    egui::Label::new(RichText::new(label).size(12.0)).truncate(),
                );
                add_widget(ui)
            })
            .inner
        })
        .inner
}

// ── Tab bar ─────────────────────────────────────────────────────────────────

fn render_tabs_inline(ui: &mut egui::Ui, settings: &mut EditorSettings, theme: &Theme) {
    let text_primary = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let accent_color = theme.semantic.accent.to_color32();
    let tab_active_bg = theme.panels.tab_active.to_color32();
    let tab_inactive_bg = theme.panels.tab_inactive.to_color32();

    let tabs: &[(SettingsTab, &str, &str)] = &[
        (SettingsTab::General,   DESKTOP,  "General"),
        (SettingsTab::Viewport,  CUBE,     "Viewport"),
        (SettingsTab::Shortcuts, KEYBOARD, "Shortcuts"),
        (SettingsTab::Theme,     PALETTE,  "Theme"),
    ];

    ui.horizontal(|ui| {
        for (tab, icon, label) in tabs {
            let is_active = settings.settings_tab == *tab;
            let text_color = if is_active { text_primary } else { text_muted };
            let bg_color = if is_active { tab_active_bg } else { tab_inactive_bg };

            let button = egui::Button::new(
                RichText::new(format!("{} {}", icon, label)).color(text_color).size(12.0),
            )
            .fill(bg_color)
            .corner_radius(CornerRadius::same(4))
            .stroke(if is_active { Stroke::new(1.0, accent_color) } else { Stroke::NONE });

            if ui.add(button).clicked() {
                settings.settings_tab = *tab;
            }
        }
    });
}

// ── General tab ─────────────────────────────────────────────────────────────

fn render_general_tab(
    ui: &mut egui::Ui,
    settings: &mut EditorSettings,
    project_config: &mut Option<renzora_core::ProjectConfig>,
    scene_files: &[String],
    theme: &Theme,
) {
    // Project Section
    if let Some(config) = project_config {
        render_category(ui, FOLDER_OPEN, "Project", CategoryStyle::interface(), "settings_project", true, theme, |ui| {
            settings_row(ui, 0, "Name", theme, |ui| {
                ui.add(egui::TextEdit::singleline(&mut config.name).desired_width(150.0))
            });

            settings_row(ui, 1, "Boot Scene", theme, |ui| {
                egui::ComboBox::from_id_salt("boot_scene_selector")
                    .selected_text(&config.main_scene)
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        for scene in scene_files {
                            ui.selectable_value(&mut config.main_scene, scene.clone(), scene);
                        }
                    })
            });

            settings_row(ui, 2, "Window Width", theme, |ui| {
                ui.add(egui::DragValue::new(&mut config.window.width)
                    .range(320..=7680)
                    .speed(1))
            });

            settings_row(ui, 3, "Window Height", theme, |ui| {
                ui.add(egui::DragValue::new(&mut config.window.height)
                    .range(240..=4320)
                    .speed(1))
            });

            settings_row(ui, 4, "Resizable", theme, |ui| {
                ui.checkbox(&mut config.window.resizable, "")
            });

            settings_row(ui, 5, "Fullscreen", theme, |ui| {
                ui.checkbox(&mut config.window.fullscreen, "")
            });
        });
    }

    // Interface / Font Section
    render_category(ui, TEXT_AA, "Interface", CategoryStyle::interface(), "settings_interface", true, theme, |ui| {
        settings_row(ui, 0, "UI Font", theme, |ui| {
            egui::ComboBox::from_id_salt("ui_font_selector")
                .selected_text(settings.ui_font.label())
                .show_ui(ui, |ui| {
                    for &font in UiFont::ALL {
                        ui.selectable_value(&mut settings.ui_font, font, font.label());
                    }
                })
        });

        settings_row(ui, 1, "Code Font", theme, |ui| {
            egui::ComboBox::from_id_salt("mono_font_selector")
                .selected_text(settings.mono_font.label())
                .show_ui(ui, |ui| {
                    for &font in MonoFont::ALL {
                        ui.selectable_value(&mut settings.mono_font, font, font.label());
                    }
                })
        });

        settings_row(ui, 2, "Font Size", theme, |ui| {
            ui.add(egui::DragValue::new(&mut settings.font_size)
                .range(10.0..=24.0)
                .speed(0.5)
                .suffix(" px"))
        });
    });

    // Developer Section
    render_category(ui, WRENCH, "Developer", CategoryStyle::developer(), "settings_developer", true, theme, |ui| {
        settings_row(ui, 0, "Dev Mode", theme, |ui| {
            ui.checkbox(&mut settings.dev_mode, "Enable plugin tools")
        });
    });

    // Scripting Section
    render_category(ui, CODE, "Scripting", CategoryStyle::scripting(), "settings_scripting", true, theme, |ui| {
        settings_row(ui, 0, "Hot Reload", theme, |ui| {
            ui.checkbox(&mut settings.script_rerun_on_ready_on_reload, "Re-run on_ready on hot reload")
        });
        settings_row(ui, 1, "Script Camera", theme, |ui| {
            ui.checkbox(&mut settings.scripts_use_game_camera, "Use game camera when running scripts")
        });
        settings_row(ui, 2, "Cursor", theme, |ui| {
            ui.checkbox(&mut settings.hide_cursor_in_play_mode, "Hide and lock cursor in play mode")
        });
    });
}

// ── Viewport tab ────────────────────────────────────────────────────────────

fn render_viewport_tab(
    ui: &mut egui::Ui,
    settings: &mut EditorSettings,
    viewport: &mut ViewportSettings,
    theme: &Theme,
) {
    // Grid Section
    render_category(ui, GRID_FOUR, "Grid", CategoryStyle::grid(), "settings_grid", true, theme, |ui| {
        settings_row(ui, 0, "Show Grid", theme, |ui| {
            ui.checkbox(&mut viewport.show_grid, "")
        });
        settings_row(ui, 1, "Show Subgrid", theme, |ui| {
            ui.checkbox(&mut viewport.show_subgrid, "")
        });
        settings_row(ui, 2, "Axis Gizmo", theme, |ui| {
            ui.checkbox(&mut viewport.show_axis_gizmo, "")
        });
    });

    // Camera Section
    render_category(ui, VIDEO_CAMERA, "Camera", CategoryStyle::camera(), "settings_camera", true, theme, |ui| {
        settings_row(ui, 0, "Move Speed", theme, |ui| {
            ui.add(egui::DragValue::new(&mut viewport.camera.move_speed)
                .range(1.0..=50.0)
                .speed(0.1))
        });
        settings_row(ui, 1, "Look Sensitivity", theme, |ui| {
            ui.add(egui::DragValue::new(&mut viewport.camera.look_sensitivity)
                .range(0.05..=2.0)
                .speed(0.01))
        });
        settings_row(ui, 2, "Orbit Sensitivity", theme, |ui| {
            ui.add(egui::DragValue::new(&mut viewport.camera.orbit_sensitivity)
                .range(0.05..=2.0)
                .speed(0.01))
        });
        settings_row(ui, 3, "Pan Sensitivity", theme, |ui| {
            ui.add(egui::DragValue::new(&mut viewport.camera.pan_sensitivity)
                .range(0.1..=5.0)
                .speed(0.01))
        });
        settings_row(ui, 4, "Zoom Sensitivity", theme, |ui| {
            ui.add(egui::DragValue::new(&mut viewport.camera.zoom_sensitivity)
                .range(0.1..=5.0)
                .speed(0.01))
        });
        settings_row(ui, 5, "Invert Y", theme, |ui| {
            ui.checkbox(&mut viewport.camera.invert_y, "")
        });
        settings_row(ui, 6, "Distance Speed", theme, |ui| {
            ui.checkbox(&mut viewport.camera.distance_relative_speed, "Scale speed by distance")
        });
    });

    // Gizmos Section
    render_category(ui, GAUGE, "Gizmos", CategoryStyle::gizmos(), "settings_gizmos", true, theme, |ui| {
        settings_row(ui, 0, "Colliders", theme, |ui| {
            egui::ComboBox::from_id_salt("collision_gizmo_visibility")
                .selected_text(match viewport.collision_gizmo_visibility {
                    CollisionGizmoVisibility::SelectedOnly => "Selected Only",
                    CollisionGizmoVisibility::Always => "Always",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut viewport.collision_gizmo_visibility, CollisionGizmoVisibility::SelectedOnly, "Selected Only");
                    ui.selectable_value(&mut viewport.collision_gizmo_visibility, CollisionGizmoVisibility::Always, "Always");
                })
        });
        settings_row(ui, 1, "Selection", theme, |ui| {
            egui::ComboBox::from_id_salt("selection_highlight_mode")
                .selected_text(match settings.selection_highlight_mode {
                    SelectionHighlightMode::Outline => "Outline",
                    SelectionHighlightMode::Gizmo => "Gizmo",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut settings.selection_highlight_mode, SelectionHighlightMode::Outline, "Outline");
                    ui.selectable_value(&mut settings.selection_highlight_mode, SelectionHighlightMode::Gizmo, "Gizmo");
                })
        });
        settings_row(ui, 2, "Boundary", theme, |ui| {
            egui::ComboBox::from_id_salt("selection_boundary_depth")
                .selected_text(if settings.selection_boundary_on_top { "On Top" } else { "Depth Tested" })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut settings.selection_boundary_on_top, true, "On Top");
                    ui.selectable_value(&mut settings.selection_boundary_on_top, false, "Depth Tested");
                })
        });
    });
}

// ── Shortcuts tab ───────────────────────────────────────────────────────────

fn render_shortcuts_tab(ui: &mut egui::Ui, keybindings: &mut KeyBindings, theme: &Theme) {
    let text_primary = theme.text.primary.to_color32();
    let item_bg = theme.panels.item_bg.to_color32();
    let border_color = theme.widgets.border.to_color32();

    let mut current_category = "";
    let mut category_actions: Vec<(String, Vec<EditorAction>)> = Vec::new();

    for action in EditorAction::all() {
        let category = action.category();
        if category != current_category {
            category_actions.push((category.to_string(), vec![action]));
            current_category = category;
        } else if let Some((_, actions)) = category_actions.last_mut() {
            actions.push(action);
        }
    }

    for (category, actions) in category_actions {
        render_category(ui, KEYBOARD, &category, CategoryStyle::shortcuts(), &format!("shortcuts_{}", category), true, theme, |ui| {
            for (i, action) in actions.iter().enumerate() {
                render_keybinding_row(ui, i, keybindings, *action, theme);
            }
        });
    }

    ui.add_space(8.0);

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
    theme: &Theme,
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
                ui.add_sized(
                    [LABEL_WIDTH + 20.0, 18.0],
                    egui::Label::new(RichText::new(action.display_name()).size(12.0)).truncate(),
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

                    let rebind_bg = warning_color.gamma_multiply(0.3);

                    let button = egui::Button::new(button_text)
                        .fill(if is_rebinding { rebind_bg } else { item_bg })
                        .stroke(Stroke::new(1.0, if is_rebinding { warning_color } else { border_color }))
                        .corner_radius(CornerRadius::same(4))
                        .min_size(Vec2::new(90.0, 20.0));

                    if ui.add(button).clicked() {
                        keybindings.rebinding = if is_rebinding { None } else { Some(action) };
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

fn keycode_to_egui(key: bevy::prelude::KeyCode) -> Option<egui::Key> {
    use bevy::prelude::KeyCode;
    match key {
        KeyCode::KeyA => Some(egui::Key::A), KeyCode::KeyB => Some(egui::Key::B),
        KeyCode::KeyC => Some(egui::Key::C), KeyCode::KeyD => Some(egui::Key::D),
        KeyCode::KeyE => Some(egui::Key::E), KeyCode::KeyF => Some(egui::Key::F),
        KeyCode::KeyG => Some(egui::Key::G), KeyCode::KeyH => Some(egui::Key::H),
        KeyCode::KeyI => Some(egui::Key::I), KeyCode::KeyJ => Some(egui::Key::J),
        KeyCode::KeyK => Some(egui::Key::K), KeyCode::KeyL => Some(egui::Key::L),
        KeyCode::KeyM => Some(egui::Key::M), KeyCode::KeyN => Some(egui::Key::N),
        KeyCode::KeyO => Some(egui::Key::O), KeyCode::KeyP => Some(egui::Key::P),
        KeyCode::KeyQ => Some(egui::Key::Q), KeyCode::KeyR => Some(egui::Key::R),
        KeyCode::KeyS => Some(egui::Key::S), KeyCode::KeyT => Some(egui::Key::T),
        KeyCode::KeyU => Some(egui::Key::U), KeyCode::KeyV => Some(egui::Key::V),
        KeyCode::KeyW => Some(egui::Key::W), KeyCode::KeyX => Some(egui::Key::X),
        KeyCode::KeyY => Some(egui::Key::Y), KeyCode::KeyZ => Some(egui::Key::Z),
        KeyCode::Digit0 => Some(egui::Key::Num0), KeyCode::Digit1 => Some(egui::Key::Num1),
        KeyCode::Digit2 => Some(egui::Key::Num2), KeyCode::Digit3 => Some(egui::Key::Num3),
        KeyCode::Digit4 => Some(egui::Key::Num4), KeyCode::Digit5 => Some(egui::Key::Num5),
        KeyCode::Digit6 => Some(egui::Key::Num6), KeyCode::Digit7 => Some(egui::Key::Num7),
        KeyCode::Digit8 => Some(egui::Key::Num8), KeyCode::Digit9 => Some(egui::Key::Num9),
        KeyCode::Escape => Some(egui::Key::Escape),
        KeyCode::F1 => Some(egui::Key::F1), KeyCode::F2 => Some(egui::Key::F2),
        KeyCode::F3 => Some(egui::Key::F3), KeyCode::F4 => Some(egui::Key::F4),
        KeyCode::F5 => Some(egui::Key::F5), KeyCode::F6 => Some(egui::Key::F6),
        KeyCode::F7 => Some(egui::Key::F7), KeyCode::F8 => Some(egui::Key::F8),
        KeyCode::F9 => Some(egui::Key::F9), KeyCode::F10 => Some(egui::Key::F10),
        KeyCode::F11 => Some(egui::Key::F11), KeyCode::F12 => Some(egui::Key::F12),
        KeyCode::Space => Some(egui::Key::Space), KeyCode::Tab => Some(egui::Key::Tab),
        KeyCode::Enter => Some(egui::Key::Enter), KeyCode::Backspace => Some(egui::Key::Backspace),
        KeyCode::Delete => Some(egui::Key::Delete), KeyCode::Insert => Some(egui::Key::Insert),
        KeyCode::Home => Some(egui::Key::Home), KeyCode::End => Some(egui::Key::End),
        KeyCode::PageUp => Some(egui::Key::PageUp), KeyCode::PageDown => Some(egui::Key::PageDown),
        KeyCode::ArrowUp => Some(egui::Key::ArrowUp), KeyCode::ArrowDown => Some(egui::Key::ArrowDown),
        KeyCode::ArrowLeft => Some(egui::Key::ArrowLeft), KeyCode::ArrowRight => Some(egui::Key::ArrowRight),
        KeyCode::Comma => Some(egui::Key::Comma), KeyCode::Period => Some(egui::Key::Period),
        KeyCode::Minus => Some(egui::Key::Minus), KeyCode::Equal => Some(egui::Key::Equals),
        _ => None,
    }
}

// ── Theme tab ───────────────────────────────────────────────────────────────

struct ThemeEditState {
    active_name: String,
    available: Vec<String>,
    theme: Theme,
    save_requested: bool,
}

fn render_theme_tab(ui: &mut egui::Ui, state: &mut ThemeEditState, theme: &Theme) {
    let text_primary = theme.text.primary.to_color32();
    let item_bg = theme.panels.item_bg.to_color32();
    let border_color = theme.widgets.border.to_color32();

    render_category(ui, PALETTE, "Active Theme", CategoryStyle::theme(), "settings_active_theme", true, theme, |ui| {
        settings_row(ui, 0, "Theme", theme, |ui| {
            let resp = egui::ComboBox::from_id_salt("theme_selector")
                .selected_text(&state.active_name)
                .show_ui(ui, |ui| {
                    for name in state.available.clone() {
                        ui.selectable_value(&mut state.active_name, name.clone(), name);
                    }
                });
            resp
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(RichText::new("Save").color(text_primary).size(11.0))
                .fill(item_bg)
                .stroke(Stroke::new(1.0, border_color))
                .corner_radius(CornerRadius::same(4))
            ).clicked() {
                state.save_requested = true;
            }
        });
    });

    // Color editing sections
    let row_even = theme.panels.inspector_row_even.to_color32();
    let row_odd = theme.panels.inspector_row_odd.to_color32();

    render_theme_color_section(ui, PALETTE, "Semantic Colors", "theme_semantic", CategoryStyle::theme(), theme, row_even, row_odd, &mut [
        ("Accent", &mut state.theme.semantic.accent),
        ("Success", &mut state.theme.semantic.success),
        ("Warning", &mut state.theme.semantic.warning),
        ("Error", &mut state.theme.semantic.error),
        ("Selection", &mut state.theme.semantic.selection),
        ("Sel. Stroke", &mut state.theme.semantic.selection_stroke),
    ]);

    render_theme_color_section(ui, PALETTE, "Surfaces", "theme_surfaces", CategoryStyle::theme(), theme, row_even, row_odd, &mut [
        ("Window", &mut state.theme.surfaces.window),
        ("Window Stroke", &mut state.theme.surfaces.window_stroke),
        ("Panel", &mut state.theme.surfaces.panel),
        ("Popup", &mut state.theme.surfaces.popup),
        ("Faint", &mut state.theme.surfaces.faint),
        ("Extreme", &mut state.theme.surfaces.extreme),
    ]);

    render_theme_color_section(ui, TEXT_AA, "Text", "theme_text", CategoryStyle::interface(), theme, row_even, row_odd, &mut [
        ("Primary", &mut state.theme.text.primary),
        ("Secondary", &mut state.theme.text.secondary),
        ("Muted", &mut state.theme.text.muted),
        ("Heading", &mut state.theme.text.heading),
        ("Disabled", &mut state.theme.text.disabled),
        ("Hyperlink", &mut state.theme.text.hyperlink),
    ]);

    render_theme_color_section(ui, CUBE, "Widgets", "theme_widgets", CategoryStyle::gizmos(), theme, row_even, row_odd, &mut [
        ("Inactive BG", &mut state.theme.widgets.inactive_bg),
        ("Inactive FG", &mut state.theme.widgets.inactive_fg),
        ("Hovered BG", &mut state.theme.widgets.hovered_bg),
        ("Hovered FG", &mut state.theme.widgets.hovered_fg),
        ("Active BG", &mut state.theme.widgets.active_bg),
        ("Active FG", &mut state.theme.widgets.active_fg),
        ("Border", &mut state.theme.widgets.border),
    ]);

    render_theme_color_section(ui, DESKTOP, "Panels", "theme_panels", CategoryStyle::interface(), theme, row_even, row_odd, &mut [
        ("Tree Line", &mut state.theme.panels.tree_line),
        ("Drop Line", &mut state.theme.panels.drop_line),
        ("Tab Active", &mut state.theme.panels.tab_active),
        ("Tab Inactive", &mut state.theme.panels.tab_inactive),
    ]);
}

fn render_theme_color_section(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    id_source: &str,
    style: CategoryStyle,
    theme: &Theme,
    row_even: Color32,
    row_odd: Color32,
    colors: &mut [(&str, &mut renzora_theme::ThemeColor)],
) {
    render_category(ui, icon, label, style, id_source, false, theme, |ui| {
        for (i, (name, color)) in colors.iter_mut().enumerate() {
            theme_color_row_mut(ui, i, name, *color, row_even, row_odd);
        }
    });
}

fn theme_color_row_mut(
    ui: &mut egui::Ui,
    row_index: usize,
    label: &str,
    color: &mut renzora_theme::ThemeColor,
    row_even: Color32,
    row_odd: Color32,
) {
    let bg_color = if row_index % 2 == 0 { row_even } else { row_odd };

    egui::Frame::new()
        .fill(bg_color)
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_sized(
                    [LABEL_WIDTH, 18.0],
                    egui::Label::new(RichText::new(label).size(12.0)).truncate(),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let [r, g, b, a] = color.0.to_array();
                    let mut srgba = [r, g, b, a];
                    if ui.color_edit_button_srgba_unmultiplied(&mut srgba).changed() {
                        *color = renzora_theme::ThemeColor::with_alpha(srgba[0], srgba[1], srgba[2], srgba[3]);
                    }
                });
            });
        });
}

// ── Placeholder tab ─────────────────────────────────────────────────────────

fn render_placeholder_tab(ui: &mut egui::Ui, theme: &Theme, title: &str, message: &str) {
    let text_muted = theme.text.muted.to_color32();
    ui.add_space(20.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(title).size(16.0).strong());
        ui.add_space(8.0);
        ui.label(RichText::new(message).size(12.0).color(text_muted));
    });
}
