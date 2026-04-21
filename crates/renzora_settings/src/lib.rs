#![allow(deprecated)] // egui API rename pending; will migrate at next bevy_egui bump.

#![allow(dead_code)] // Public surface area kept for upcoming features.

//! Renzora Settings — floating overlay window for editor settings.
//!
//! Reads from decentralized resources (`EditorSettings`, `KeyBindings`,
//! `ViewportSettings`, `ThemeManager`) and writes back via direct mutation.

use bevy::prelude::*;
use bevy::ecs::system::SystemState;
use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, RichText, Stroke, Vec2};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass};

use egui_phosphor::regular::{
    CARET_DOWN, CARET_RIGHT, CODE, DESKTOP,
    FOLDER_OPEN, VIDEO_CAMERA, KEYBOARD, PALETTE, TEXT_AA, GAUGE,
    WRENCH, GRID_FOUR, CUBE, GAME_CONTROLLER, INFO, PLUS, TRASH, LIST_PLUS, X,
};

use renzora_editor_framework::{CustomFonts, EditorSettings, MonoFont, SelectionHighlightMode, SettingsTab, UiFont};
use renzora_keybindings::{bindable_keys, EditorAction, KeyBinding, KeyBindings};
use renzora_theme::{Theme, ThemeManager};
use renzora_viewport::settings::{CollisionGizmoVisibility, ViewportSettings};

const LABEL_WIDTH: f32 = 100.0;

// ── Plugin ──────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] SettingsPlugin");
        app.add_systems(
            EguiPrimaryContextPass,
            settings_overlay_system
                .run_if(in_state(renzora_editor_framework::SplashState::Editor)),
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
    let custom_fonts = world.get_resource::<CustomFonts>().cloned().unwrap_or_default();
    let keybindings = world.get_resource::<KeyBindings>().cloned().unwrap_or_default();
    // Snapshot plugin shortcut metadata so the Shortcuts tab can display
    // plugin-registered shortcuts alongside built-in ones.
    let plugin_shortcuts: Vec<PluginShortcutRow> = world
        .get_resource::<renzora_editor_framework::ShortcutRegistry>()
        .map(|reg| {
            reg.entries()
                .iter()
                .map(|e| PluginShortcutRow {
                    id: e.id,
                    display_name: e.display_name,
                    category: e.category,
                })
                .collect()
        })
        .unwrap_or_default();
    let viewport_settings = world.get_resource::<ViewportSettings>().cloned().unwrap_or_default();

    // Project config snapshot + available scene files
    let (project_config, scene_files, project_path) = if let Some(project) = world.get_resource::<renzora::core::CurrentProject>() {
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
        (Some(project.config.clone()), files, Some(project.path.clone()))
    } else {
        (None, Vec::new(), None)
    };
    let mut project_config_mut = project_config.clone();
    let mut clear_thumbnail_cache = false;

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

    let input_map = world.get_resource::<renzora_input::InputMap>().cloned().unwrap_or_default();
    let mut input_map_mut = input_map.clone();
    let mut input_ui_state = world.remove_resource::<InputUiState>().unwrap_or_default();

    // Capture gamepad input for the listen-mode binding UI.
    // Must be read here (outside egui closure) since gamepads are Bevy resources.
    input_ui_state.captured_gamepad = None;
    if input_ui_state.listening {
        use bevy::input::gamepad::{Gamepad, GamepadButton as GpBtn};
        let mut gp_query = world.query::<&Gamepad>();
        for gamepad in gp_query.iter(world) {
            for btn in [
                GpBtn::South, GpBtn::East, GpBtn::West, GpBtn::North,
                GpBtn::LeftTrigger, GpBtn::RightTrigger,
                GpBtn::LeftTrigger2, GpBtn::RightTrigger2,
                GpBtn::Select, GpBtn::Start,
                GpBtn::LeftThumb, GpBtn::RightThumb,
                GpBtn::DPadUp, GpBtn::DPadDown,
                GpBtn::DPadLeft, GpBtn::DPadRight,
            ] {
                if gamepad.just_pressed(btn) {
                    input_ui_state.captured_gamepad = Some(
                        renzora_input::InputBinding::GamepadButton(format!("{:?}", btn))
                    );
                    break;
                }
            }
            if input_ui_state.captured_gamepad.is_some() { break; }

            // Also check stick axes for axis bindings
            let left = gamepad.left_stick();
            let right = gamepad.right_stick();
            if left.length() > 0.8 {
                input_ui_state.captured_gamepad = Some(
                    renzora_input::InputBinding::GamepadAxis("LeftStickX".into())
                );
                break;
            }
            if right.length() > 0.8 {
                input_ui_state.captured_gamepad = Some(
                    renzora_input::InputBinding::GamepadAxis("RightStickX".into())
                );
                break;
            }
        }
    }

    let mut settings_mut = settings.clone();
    let mut keybindings_mut = keybindings.clone();
    let mut viewport_mut = viewport_settings.clone();

    // Handle key capture for rebinding (built-in actions)
    if let Some(action) = keybindings_mut.rebinding {
        capture_key_for_rebind(ctx, &mut keybindings_mut, action);
    }
    // Handle key capture for plugin shortcut rebinding
    if let Some(plugin_id) = keybindings_mut.plugin_rebinding {
        capture_key_for_plugin_rebind(ctx, &mut keybindings_mut, plugin_id);
    }

    let mut open = true;
    let screen = ctx.input(|i| i.screen_rect());
    let default_size = egui::Vec2::new(880.0, 620.0);
    let default_pos = egui::Pos2::new(
        (screen.width() - default_size.x) / 2.0,
        (screen.height() - default_size.y) / 2.0,
    );

    let mut close_clicked = false;
    egui::Window::new("Settings")
        .open(&mut open)
        .title_bar(false)
        .fixed_size(default_size)
        .default_pos(default_pos)
        .resizable(false)
        .collapsible(false)
        .frame(egui::Frame::window(&ctx.style())
            .fill(theme.surfaces.extreme.to_color32())
            .corner_radius(CornerRadius { nw: 6, ne: 0, sw: 0, se: 0 })
            .inner_margin(egui::Margin::ZERO))
        .show(ctx, |ui| {
            // Custom title bar — no separator line.
            let mut close_hovered = false;
            let header_resp = egui::Frame::new()
                .fill(theme.surfaces.extreme.to_color32().gamma_multiply(0.55))
                .corner_radius(CornerRadius { nw: 6, ne: 0, sw: 0, se: 0 })
                .inner_margin(egui::Margin { left: 12, right: 8, top: 6, bottom: 6 })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Settings").size(14.0).strong().color(theme.text.heading.to_color32()));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let btn_size = egui::vec2(22.0, 22.0);
                            let (rect, resp) = ui.allocate_exact_size(btn_size, egui::Sense::click());
                            if resp.hovered() {
                                close_hovered = true;
                                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                            }
                            let bg = if resp.hovered() {
                                theme.semantic.error.to_color32().gamma_multiply(0.25)
                            } else {
                                theme.surfaces.extreme.to_color32().gamma_multiply(0.8)
                            };
                            let fg = if resp.hovered() {
                                theme.semantic.error.to_color32()
                            } else {
                                theme.text.muted.to_color32()
                            };
                            ui.painter().rect_filled(rect, CornerRadius::same(4), bg);
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                X,
                                egui::FontId::proportional(14.0),
                                fg,
                            );
                            if resp.clicked() {
                                close_clicked = true;
                            }
                        });
                    });
                }).response;
            let header_hover = ui.interact(header_resp.rect, ui.id().with("settings_header_hover"), egui::Sense::hover());
            if header_hover.hovered() && !close_hovered {
                ui.ctx().set_cursor_icon(CursorIcon::Move);
            }
            ui.add_space(4.0);

            ui.horizontal_top(|ui| {
                ui.add_space(6.0);
                // Left: vertical tab rail
                egui::Frame::new()
                    .fill(theme.surfaces.extreme.to_color32())
                    .inner_margin(egui::Margin::symmetric(6, 8))
                    .show(ui, |ui| {
                        ui.set_width(160.0);
                        ui.set_min_height(ui.available_height());
                        render_tabs_vertical(ui, &mut settings_mut, &theme);
                    });

                ui.add_space(8.0);

                // Right: content — paint lighter background behind the pane.
                let content_rect = ui.available_rect_before_wrap();
                let content_corners = CornerRadius { nw: 6, ne: 0, sw: 0, se: 0 };
                ui.painter().rect_filled(
                    content_rect,
                    content_corners,
                    theme.surfaces.panel.to_color32(),
                );
                ui.painter().rect_stroke(
                    content_rect,
                    content_corners,
                    Stroke::new(1.0, theme.widgets.border.to_color32().gamma_multiply(0.5)),
                    egui::StrokeKind::Inside,
                );
                ui.vertical(|ui| {
                    egui::Frame::new()
                        .inner_margin(egui::Margin { left: 14, right: 14, top: 12, bottom: 12 })
                        .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());

                            match settings_mut.settings_tab {
                                SettingsTab::Project => render_project_tab(ui, &mut project_config_mut, &scene_files, project_path.as_deref(), &mut clear_thumbnail_cache, &theme),
                                SettingsTab::Interface => render_interface_tab(ui, &mut settings_mut, &custom_fonts, &theme),
                                SettingsTab::Editor => render_editor_tab(ui, &mut settings_mut, &theme),
                                SettingsTab::Viewport => render_viewport_tab(ui, &mut settings_mut, &mut viewport_mut, &theme),
                                SettingsTab::Scripting => render_scripting_tab(ui, &mut settings_mut, &theme),
                                SettingsTab::Assets => render_assets_tab(ui, &mut settings_mut, &theme),
                                SettingsTab::Input => render_input_tab(ui, &mut input_map_mut, &mut input_ui_state, &theme),
                                SettingsTab::Shortcuts => render_shortcuts_tab(ui, &mut keybindings_mut, &plugin_shortcuts, &theme),
                                SettingsTab::Theme => render_theme_tab(ui, &mut theme_edit, &theme),
                                SettingsTab::Plugins => render_plugins_tab(ui, &mut settings_mut, &theme),
                            }
                        });
                        });
                });
            });
        });

    if !open || close_clicked {
        settings_mut.show_settings = false;
    }

    // Write back mutations
    if settings_mut != settings {
        if let Some(mut res) = world.get_resource_mut::<EditorSettings>() {
            *res = settings_mut;
        }
    }

    if keybindings_mut.bindings != keybindings.bindings
        || keybindings_mut.rebinding != keybindings.rebinding
        || keybindings_mut.plugin_bindings != keybindings.plugin_bindings
        || keybindings_mut.plugin_rebinding != keybindings.plugin_rebinding
    {
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
            if let Some(mut project) = world.get_resource_mut::<renzora::core::CurrentProject>() {
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

    // Write back input map changes and save to disk
    if input_map_mut.actions != input_map.actions {
        // Save to project file
        if let Some(project) = world.get_resource::<renzora::core::CurrentProject>() {
            if let Err(e) = renzora_input::save_input_map(&input_map_mut, project) {
                warn!("Failed to save input map: {}", e);
            }
        }
        if let Some(mut res) = world.get_resource_mut::<renzora_input::InputMap>() {
            *res = input_map_mut;
        }
    }
    world.insert_resource(input_ui_state);

    if clear_thumbnail_cache {
        if let Some(ref path) = project_path {
            let thumbs_dir = path.join(".thumbs").join("materials");
            match std::fs::remove_dir_all(&thumbs_dir) {
                Ok(_) => info!("[settings] Cleared material thumbnail cache at {}", thumbs_dir.display()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    info!("[settings] Material thumbnail cache was already empty");
                }
                Err(e) => warn!("[settings] Failed to clear thumbnail cache: {}", e),
            }
        }
        if let Some(mut registry) = world.get_resource_mut::<renzora_editor_framework::MaterialThumbnailRegistry>() {
            *registry = renzora_editor_framework::MaterialThumbnailRegistry::default();
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
                    egui::Frame::new()
                        .inner_margin(egui::Margin { left: 4, right: 4, top: 0, bottom: 4 })
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
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
        theme.panels.inspector_row_even.to_color32().gamma_multiply(0.8)
    } else {
        theme.panels.inspector_row_odd.to_color32().gamma_multiply(0.8)
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

fn render_tabs_vertical(ui: &mut egui::Ui, settings: &mut EditorSettings, theme: &Theme) {
    let text_primary = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let accent_color = theme.semantic.accent.to_color32();
    let tab_active_bg = theme.panels.tab_active.to_color32();

    let tabs: &[(SettingsTab, &str, &str)] = &[
        (SettingsTab::Project,   FOLDER_OPEN,      "Project"),
        (SettingsTab::Interface, TEXT_AA,          "Interface"),
        (SettingsTab::Editor,    WRENCH,           "Editor"),
        (SettingsTab::Viewport,  CUBE,             "Viewport"),
        (SettingsTab::Scripting, CODE,             "Scripting"),
        (SettingsTab::Assets,    DESKTOP,          "Assets"),
        (SettingsTab::Input,     GAME_CONTROLLER,  "Input"),
        (SettingsTab::Shortcuts, KEYBOARD,         "Shortcuts"),
        (SettingsTab::Theme,     PALETTE,          "Theme"),
    ];

    let tab_hover_bg = theme.panels.tab_hover.to_color32();

    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        for (tab, icon, label) in tabs {
            let is_active = settings.settings_tab == *tab;
            let size = egui::vec2(ui.available_width(), 30.0);
            let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

            if response.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }

            let bg_color = if is_active {
                tab_active_bg
            } else if response.hovered() {
                tab_hover_bg
            } else {
                Color32::TRANSPARENT
            };
            let text_color = if is_active || response.hovered() { text_primary } else { text_muted };

            let painter = ui.painter();
            painter.rect_filled(rect, CornerRadius::same(4), bg_color);
            if is_active {
                painter.rect_stroke(rect, CornerRadius::same(4), Stroke::new(1.0, accent_color), egui::StrokeKind::Inside);
            }
            painter.text(
                rect.left_center() + egui::vec2(10.0, 0.0),
                egui::Align2::LEFT_CENTER,
                format!("{}   {}", icon, label),
                egui::FontId::proportional(13.0),
                text_color,
            );

            if response.clicked() {
                settings.settings_tab = *tab;
            }
        }
    });
}

// ── Tab content ─────────────────────────────────────────────────────────────

#[derive(Default)]
struct CacheStats {
    file_count: u64,
    total_bytes: u64,
}

fn cache_stats(dir: &std::path::Path) -> CacheStats {
    let mut stats = CacheStats::default();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                stack.push(entry.path());
            } else if meta.is_file() {
                stats.file_count += 1;
                stats.total_bytes += meta.len();
            }
        }
    }
    stats
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut i = 0;
    while value >= 1024.0 && i < UNITS.len() - 1 {
        value /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{} {}", bytes, UNITS[i])
    } else {
        format!("{:.1} {}", value, UNITS[i])
    }
}

fn render_project_tab(
    ui: &mut egui::Ui,
    project_config: &mut Option<renzora::core::ProjectConfig>,
    scene_files: &[String],
    project_path: Option<&std::path::Path>,
    clear_thumbnail_cache: &mut bool,
    theme: &Theme,
) {
    let Some(config) = project_config else {
        render_placeholder_tab(ui, theme, "Project", "No project is currently loaded.");
        return;
    };

    render_category(ui, FOLDER_OPEN, "Project", CategoryStyle::interface(), "settings_project", true, theme, |ui| {
        settings_row(ui, 0, "Name", theme, |ui| {
            ui.add(egui::TextEdit::singleline(&mut config.name).desired_width(200.0))
        });

        settings_row(ui, 1, "Boot Scene", theme, |ui| {
            egui::ComboBox::from_id_salt("boot_scene_selector")
                .selected_text(&config.main_scene)
                .width(240.0)
                .show_ui(ui, |ui| {
                    for scene in scene_files {
                        ui.selectable_value(&mut config.main_scene, scene.clone(), scene);
                    }
                })
        });
    });

    render_category(ui, TRASH, "Cache", CategoryStyle::interface(), "settings_cache", true, theme, |ui| {
        // Show the disk size so the user knows what they're clearing.
        let thumbs_dir = project_path.map(|p| p.join(".thumbs").join("materials"));
        let cache_info = thumbs_dir
            .as_deref()
            .map(cache_stats)
            .unwrap_or_default();

        settings_row(ui, 0, "Material Thumbnails", theme, |ui| {
            let label = if cache_info.file_count > 0 {
                format!("{} files · {}", cache_info.file_count, format_bytes(cache_info.total_bytes))
            } else {
                "Empty".to_string()
            };
            ui.label(
                RichText::new(label)
                    .size(11.5)
                    .color(theme.text.muted.to_color32()),
            );
            let button = egui::Button::new(
                RichText::new("Clear Cache").color(theme.text.primary.to_color32()),
            )
                .fill(theme.panels.item_bg.to_color32())
                .stroke(Stroke::new(1.0, theme.widgets.border.to_color32()))
                .corner_radius(CornerRadius::same(5))
                .min_size(Vec2::new(110.0, 24.0));
            if ui.add_enabled(cache_info.file_count > 0, button).clicked() {
                *clear_thumbnail_cache = true;
            }
            ui.label(RichText::new("").size(1.0)) // keep settings_row's return type happy
        });
    });

    render_category(ui, DESKTOP, "Window", CategoryStyle::interface(), "settings_window", true, theme, |ui| {
        settings_row(ui, 0, "Width", theme, |ui| {
            ui.add(egui::DragValue::new(&mut config.window.width)
                .range(320..=7680)
                .speed(1))
        });
        settings_row(ui, 1, "Height", theme, |ui| {
            ui.add(egui::DragValue::new(&mut config.window.height)
                .range(240..=4320)
                .speed(1))
        });
        settings_row(ui, 2, "Resizable", theme, |ui| {
            ui.checkbox(&mut config.window.resizable, "")
        });
        settings_row(ui, 3, "Fullscreen", theme, |ui| {
            ui.checkbox(&mut config.window.fullscreen, "")
        });
    });
}

fn render_interface_tab(
    ui: &mut egui::Ui,
    settings: &mut EditorSettings,
    custom_fonts: &CustomFonts,
    theme: &Theme,
) {
    render_category(ui, TEXT_AA, "Fonts", CategoryStyle::interface(), "settings_interface", true, theme, |ui| {
        settings_row(ui, 0, "UI Font", theme, |ui| {
            egui::ComboBox::from_id_salt("ui_font_selector")
                .selected_text(settings.ui_font.label())
                .show_ui(ui, |ui| {
                    for font in UiFont::BUILTIN {
                        ui.selectable_value(&mut settings.ui_font, font.clone(), font.label());
                    }
                    if !custom_fonts.names.is_empty() {
                        ui.separator();
                        for name in &custom_fonts.names {
                            let custom = UiFont::Custom(name.clone());
                            ui.selectable_value(&mut settings.ui_font, custom, name.as_str());
                        }
                    }
                })
        });

        settings_row(ui, 1, "Code Font", theme, |ui| {
            egui::ComboBox::from_id_salt("mono_font_selector")
                .selected_text(settings.mono_font.label())
                .show_ui(ui, |ui| {
                    for font in MonoFont::BUILTIN {
                        ui.selectable_value(&mut settings.mono_font, font.clone(), font.label());
                    }
                    if !custom_fonts.names.is_empty() {
                        ui.separator();
                        for name in &custom_fonts.names {
                            let custom = MonoFont::Custom(name.clone());
                            ui.selectable_value(&mut settings.mono_font, custom, name.as_str());
                        }
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
}

fn render_editor_tab(ui: &mut egui::Ui, settings: &mut EditorSettings, theme: &Theme) {
    render_category(ui, WRENCH, "Developer", CategoryStyle::developer(), "settings_developer", true, theme, |ui| {
        settings_row(ui, 0, "Dev Mode", theme, |ui| {
            ui.checkbox(&mut settings.dev_mode, "Enable plugin tools")
        });
    });
    render_category(ui, DESKTOP, "UI Workspace", CategoryStyle::interface(), "settings_ui_workspace", true, theme, |ui| {
        settings_row(ui, 0, "Preview", theme, |ui| {
            ui.checkbox(&mut settings.ui_preview_by_default, "Show game viewport behind canvas by default")
        });
    });
}

fn render_scripting_tab(ui: &mut egui::Ui, settings: &mut EditorSettings, theme: &Theme) {
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

    render_category(ui, CODE, "Code Editor", CategoryStyle::scripting(), "settings_code_editor", true, theme, |ui| {
        settings_row(ui, 0, "Auto-close pairs", theme, |ui| {
            ui.checkbox(
                &mut settings.code_auto_close_pairs,
                "Insert matching ) ] } \" ' when typing the opener",
            )
        });
        settings_row(ui, 1, "Trim on save", theme, |ui| {
            ui.checkbox(
                &mut settings.code_trim_trailing_whitespace_on_save,
                "Strip trailing whitespace from each line on save",
            )
        });
        settings_row(ui, 2, "Minimap", theme, |ui| {
            ui.checkbox(&mut settings.code_show_minimap, "Show minimap sidebar")
        });
        settings_row(ui, 3, "Whitespace markers", theme, |ui| {
            ui.checkbox(
                &mut settings.code_show_whitespace,
                "Show · for spaces and → for tabs",
            )
        });
    });
}

fn render_assets_tab(ui: &mut egui::Ui, settings: &mut EditorSettings, theme: &Theme) {
    render_category(ui, FOLDER_OPEN, "Import", CategoryStyle::interface(), "settings_assets", true, theme, |ui| {
        settings_row(ui, 0, "Drop Import", theme, |ui| {
            ui.checkbox(&mut settings.auto_import_on_drop, "Auto-import on drop (skip import overlay)")
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

/// Snapshot of one plugin-registered shortcut for the Settings UI.
#[derive(Clone)]
struct PluginShortcutRow {
    id: &'static str,
    display_name: &'static str,
    category: &'static str,
}

fn render_shortcuts_tab(
    ui: &mut egui::Ui,
    keybindings: &mut KeyBindings,
    plugin_shortcuts: &[PluginShortcutRow],
    theme: &Theme,
) {
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

    // Plugin-registered shortcuts, grouped by their declared category.
    let mut plugin_by_cat: Vec<(&str, Vec<&PluginShortcutRow>)> = Vec::new();
    for entry in plugin_shortcuts {
        if let Some(bucket) = plugin_by_cat.iter_mut().find(|(c, _)| *c == entry.category) {
            bucket.1.push(entry);
        } else {
            plugin_by_cat.push((entry.category, vec![entry]));
        }
    }
    for (category, entries) in plugin_by_cat {
        render_category(ui, KEYBOARD, category, CategoryStyle::shortcuts(), &format!("shortcuts_plugin_{}", category), true, theme, |ui| {
            for (i, entry) in entries.iter().enumerate() {
                render_plugin_keybinding_row(ui, i, keybindings, entry, theme);
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
        theme.panels.inspector_row_even.to_color32().gamma_multiply(0.8)
    } else {
        theme.panels.inspector_row_odd.to_color32().gamma_multiply(0.8)
    };

    egui::Frame::new()
        .fill(bg_color)
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    egui::Label::new(RichText::new(action.display_name()).size(12.0)).wrap_mode(egui::TextWrapMode::Extend),
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

fn render_plugin_keybinding_row(
    ui: &mut egui::Ui,
    row_index: usize,
    keybindings: &mut KeyBindings,
    entry: &PluginShortcutRow,
    theme: &Theme,
) {
    let text_muted = theme.text.muted.to_color32();
    let accent_color = theme.semantic.accent.to_color32();
    let item_bg = theme.panels.item_bg.to_color32();
    let border_color = theme.widgets.border.to_color32();
    let warning_color = theme.semantic.warning.to_color32();

    let bg_color = if row_index % 2 == 0 {
        theme.panels.inspector_row_even.to_color32().gamma_multiply(0.8)
    } else {
        theme.panels.inspector_row_odd.to_color32().gamma_multiply(0.8)
    };

    egui::Frame::new()
        .fill(bg_color)
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    egui::Label::new(RichText::new(entry.display_name).size(12.0)).wrap_mode(egui::TextWrapMode::Extend),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let is_rebinding = keybindings.plugin_rebinding == Some(entry.id);

                    let button_text = if is_rebinding {
                        RichText::new("Press key...").color(warning_color).size(11.0)
                    } else if let Some(binding) = keybindings.get_plugin(entry.id) {
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
                        // Clear any in-progress built-in rebind so there's
                        // only one active capture at a time.
                        keybindings.rebinding = None;
                        keybindings.plugin_rebinding = if is_rebinding { None } else { Some(entry.id) };
                    }
                });
            });
        });
}

fn capture_key_for_plugin_rebind(ctx: &egui::Context, keybindings: &mut KeyBindings, id: &'static str) {
    let keys = bindable_keys();
    ctx.input(|input| {
        let ctrl = input.modifiers.ctrl;
        let shift = input.modifiers.shift;
        let alt = input.modifiers.alt;

        for key in &keys {
            if let Some(egui_key) = keycode_to_egui(*key) {
                if input.key_pressed(egui_key) {
                    let mut binding = KeyBinding::new(*key);
                    if ctrl { binding = binding.ctrl(); }
                    if shift { binding = binding.shift(); }
                    if alt { binding = binding.alt(); }
                    keybindings.set_plugin(id, binding);
                    keybindings.plugin_rebinding = None;
                    return;
                }
            }
        }

        if input.key_pressed(egui::Key::Escape) && !ctrl && !shift && !alt {
            keybindings.plugin_rebinding = None;
        }
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
    render_category(ui, icon, label, style, id_source, true, theme, |ui| {
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

// ── Input tab ───────────────────────────────────────────────────────────────

/// Persistent UI state for the input settings tab.
#[derive(Resource, Default)]
struct InputUiState {
    /// Index of the currently selected action.
    selected: Option<usize>,
    /// When `Some`, we are listening for the next key/button press to create a binding.
    listening: bool,
    /// Name buffer for the new action being added.
    new_action_name: String,
    /// Gamepad binding captured this frame (read from Bevy input, outside egui closure).
    captured_gamepad: Option<renzora_input::InputBinding>,
}

fn render_input_tab(
    ui: &mut egui::Ui,
    input_map: &mut renzora_input::InputMap,
    ui_state: &mut InputUiState,
    theme: &Theme,
) {
    use renzora_input::{ActionKind, InputAction, InputBinding};

    let text_primary = theme.text.primary.to_color32();
    let accent_color = theme.semantic.accent.to_color32();

    // Info section (at top)
    render_category(ui, INFO, "About Input Actions", CategoryStyle::interface(), "settings_input_info", true, theme, |ui| {
        let muted = theme.text.muted.to_color32();
        ui.label(RichText::new(
            "Input actions map logical names (e.g. \"Jump\", \"Move\") to keys, mouse buttons, \
             or gamepad inputs. Scripts query these names instead of raw keys so bindings stay \
             remappable without code changes."
        ).size(11.5).color(muted));
        ui.add_space(4.0);
        ui.label(RichText::new(
            "Button — pressed/released state. Axis2D — two-axis value (e.g. movement stick)."
        ).size(11.5).color(muted));
        ui.add_space(4.0);
        ui.label(RichText::new(
            "Select an action below to edit its bindings."
        ).size(11.5).color(muted));
    });

    // Add Action category
    render_category(ui, LIST_PLUS, "Add Action", CategoryStyle::interface(), "settings_input_add_action", true, theme, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add(egui::TextEdit::singleline(&mut ui_state.new_action_name)
                .desired_width(180.0)
                .hint_text("Action name..."));

            if ui.button(RichText::new("Button").size(11.0)).clicked() && !ui_state.new_action_name.is_empty() {
                let name = std::mem::take(&mut ui_state.new_action_name);
                input_map.add(InputAction::button(name, vec![]));
            }
            if ui.button(RichText::new("Axis2D").size(11.0)).clicked() && !ui_state.new_action_name.is_empty() {
                let name = std::mem::take(&mut ui_state.new_action_name);
                input_map.add(InputAction::axis_2d(name, vec![], 0.15));
            }
        });
    });

    // Actions list — each row expands inline when selected.
    render_category(ui, GAME_CONTROLLER, "Input Actions", CategoryStyle::interface(), "settings_input_actions", true, theme, |ui| {
        let mut remove_idx: Option<usize> = None;
        let action_count = input_map.actions.len();
        for i in 0..action_count {
            let is_selected = ui_state.selected == Some(i);
            let (kind_label, kind_is_axis) = {
                let action = &input_map.actions[i];
                let label = match action.kind {
                    ActionKind::Button => "Button",
                    ActionKind::Axis1D => "Axis1D",
                    ActionKind::Axis2D => "Axis2D",
                };
                (label, action.kind != ActionKind::Button)
            };
            let action_name = input_map.actions[i].name.clone();

            let row_bg = if is_selected {
                theme.panels.tab_active.to_color32()
            } else if i % 2 == 0 {
                theme.panels.inspector_row_even.to_color32().gamma_multiply(0.8)
            } else {
                theme.panels.inspector_row_odd.to_color32().gamma_multiply(0.8)
            };

            // Clickable action row — full-row hit area.
            let row_resp = egui::Frame::new()
                .fill(row_bg)
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width() - 20.0);
                    ui.horizontal(|ui| {
                        let caret = if is_selected { CARET_DOWN } else { CARET_RIGHT };
                        ui.label(RichText::new(caret).size(11.0).color(theme.text.muted.to_color32()));
                        ui.label(RichText::new(&action_name).size(13.0).color(text_primary).strong());
                        ui.label(RichText::new(kind_label).size(11.0).color(theme.text.muted.to_color32()));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(egui::Button::new(RichText::new(TRASH).size(14.0).color(theme.semantic.error.to_color32()))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE))
                                .on_hover_text("Delete action")
                                .clicked()
                            {
                                remove_idx = Some(i);
                            }
                        });
                    });
                }).response;

            let row_click = ui.interact(row_resp.rect, ui.id().with(("action_row", i)), egui::Sense::click());
            if row_click.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if row_click.clicked() {
                ui_state.selected = if is_selected { None } else { Some(i) };
                ui_state.listening = false;
            }

            // Inline details panel when expanded.
            if is_selected {
                egui::Frame::new()
                    .fill(theme.surfaces.extreme.to_color32())
                    .inner_margin(egui::Margin { left: 24, right: 12, top: 8, bottom: 10 })
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
                        let action = &mut input_map.actions[i];

                        if kind_is_axis {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Dead Zone").size(11.5).color(theme.text.muted.to_color32()));
                                ui.add(egui::Slider::new(&mut action.dead_zone, 0.0..=0.5).step_by(0.01));
                            });
                        }

                        let mut remove_binding: Option<usize> = None;
                        if action.bindings.is_empty() {
                            ui.label(RichText::new("No bindings yet.").size(11.5).italics().color(theme.text.muted.to_color32()));
                        } else {
                            for (bi, binding) in action.bindings.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("\u{2022}").size(11.0).color(theme.text.muted.to_color32()));
                                    ui.label(RichText::new(format_binding(binding)).size(12.0).color(text_primary));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.add(egui::Button::new(RichText::new(TRASH).size(12.0).color(theme.semantic.error.to_color32()))
                                            .fill(Color32::TRANSPARENT)
                                            .stroke(Stroke::NONE))
                                            .clicked()
                                        {
                                            remove_binding = Some(bi);
                                        }
                                    });
                                });
                            }
                        }
                        if let Some(bi) = remove_binding {
                            action.bindings.remove(bi);
                        }

                        ui.add_space(2.0);

                        if ui_state.listening {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Press any key, mouse button, or gamepad button...")
                                    .size(11.5).color(accent_color).italics());
                                if ui.small_button("Cancel").clicked() {
                                    ui_state.listening = false;
                                }
                            });

                            let captured = ui.input(|input| {
                                for event in &input.events {
                                    match event {
                                        egui::Event::Key { key, pressed: true, .. } => {
                                            if let Some(key_str) = egui_key_to_key_string(*key) {
                                                return Some(InputBinding::Key(key_str));
                                            }
                                        }
                                        egui::Event::PointerButton { button, pressed: true, .. } => {
                                            let mb_str = match button {
                                                egui::PointerButton::Primary => "Left",
                                                egui::PointerButton::Secondary => "Right",
                                                egui::PointerButton::Middle => "Middle",
                                                _ => "Left",
                                            };
                                            return Some(InputBinding::MouseButton(mb_str.into()));
                                        }
                                        _ => {}
                                    }
                                }
                                None
                            });

                            let final_binding = captured.or_else(|| ui_state.captured_gamepad.take());
                            if let Some(binding) = final_binding {
                                action.bindings.push(binding);
                                ui_state.listening = false;
                            }
                        } else {
                            ui.horizontal(|ui| {
                                if ui.button(RichText::new(format!("{} Add Binding", PLUS)).size(11.0)).clicked() {
                                    ui_state.listening = true;
                                }
                                if action.kind == ActionKind::Axis2D {
                                    if ui.button(RichText::new("WASD").size(11.0)).clicked() {
                                        action.bindings.push(InputBinding::Composite2D {
                                            up: "KeyW".into(), down: "KeyS".into(),
                                            left: "KeyA".into(), right: "KeyD".into(),
                                        });
                                    }
                                    if ui.button(RichText::new("Arrows").size(11.0)).clicked() {
                                        action.bindings.push(InputBinding::Composite2D {
                                            up: "ArrowUp".into(), down: "ArrowDown".into(),
                                            left: "ArrowLeft".into(), right: "ArrowRight".into(),
                                        });
                                    }
                                }
                            });
                        }
                    });
            }
        }

        if let Some(idx) = remove_idx {
            input_map.actions.remove(idx);
            if ui_state.selected == Some(idx) {
                ui_state.selected = None;
            } else if let Some(sel) = ui_state.selected {
                if sel > idx {
                    ui_state.selected = Some(sel - 1);
                }
            }
        }
    });

    // The `text_primary` / `accent_color` locals above are used inside the
    // expanded-row closure; suppress the unused warning when no row is open.
    let _ = text_primary;
    let _ = accent_color;
}

fn format_binding(binding: &renzora_input::InputBinding) -> String {
    use renzora_input::InputBinding;
    match binding {
        InputBinding::Key(key) => format!("Key: {}", key),
        InputBinding::MouseButton(btn) => format!("Mouse: {}", btn),
        InputBinding::GamepadButton(btn) => format!("Gamepad: {}", btn),
        InputBinding::GamepadAxis(axis) => format!("Gamepad Axis: {}", axis),
        InputBinding::Composite2D { up, down, left, right } => {
            format!("Composite: {}/{}/{}/{}", up, left, down, right)
        }
    }
}

/// Convert an egui key to a Bevy KeyCode debug string.
fn egui_key_to_key_string(key: egui::Key) -> Option<String> {
    use egui::Key;
    Some(match key {
        Key::A => "KeyA", Key::B => "KeyB", Key::C => "KeyC",
        Key::D => "KeyD", Key::E => "KeyE", Key::F => "KeyF",
        Key::G => "KeyG", Key::H => "KeyH", Key::I => "KeyI",
        Key::J => "KeyJ", Key::K => "KeyK", Key::L => "KeyL",
        Key::M => "KeyM", Key::N => "KeyN", Key::O => "KeyO",
        Key::P => "KeyP", Key::Q => "KeyQ", Key::R => "KeyR",
        Key::S => "KeyS", Key::T => "KeyT", Key::U => "KeyU",
        Key::V => "KeyV", Key::W => "KeyW", Key::X => "KeyX",
        Key::Y => "KeyY", Key::Z => "KeyZ",
        Key::Num0 => "Digit0", Key::Num1 => "Digit1",
        Key::Num2 => "Digit2", Key::Num3 => "Digit3",
        Key::Num4 => "Digit4", Key::Num5 => "Digit5",
        Key::Num6 => "Digit6", Key::Num7 => "Digit7",
        Key::Num8 => "Digit8", Key::Num9 => "Digit9",
        Key::Space => "Space",
        Key::Enter => "Enter",
        Key::Escape => "Escape",
        Key::Tab => "Tab",
        Key::Backspace => "Backspace",
        Key::ArrowUp => "ArrowUp", Key::ArrowDown => "ArrowDown",
        Key::ArrowLeft => "ArrowLeft", Key::ArrowRight => "ArrowRight",
        Key::F1 => "F1", Key::F2 => "F2", Key::F3 => "F3",
        Key::F4 => "F4", Key::F5 => "F5", Key::F6 => "F6",
        Key::F7 => "F7", Key::F8 => "F8", Key::F9 => "F9",
        Key::F10 => "F10", Key::F11 => "F11", Key::F12 => "F12",
        _ => return None,
    }.into())
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

fn render_plugins_tab(ui: &mut egui::Ui, settings: &mut EditorSettings, theme: &Theme) {
    let text_primary = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();

    ui.add_space(8.0);
    ui.label(RichText::new("Plugins Directory").size(13.0).color(text_primary));
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        let response = ui.add(
            egui::TextEdit::singleline(&mut settings.plugins_dir)
                .desired_width(300.0)
                .hint_text("Path to plugins folder"),
        );
        if ui.button(FOLDER_OPEN).clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                settings.plugins_dir = path.display().to_string();
            }
        }
        let _ = response;
    });

    ui.add_space(4.0);
    ui.label(RichText::new("Restart the editor to load plugins from a new directory.").size(11.0).color(text_muted));
}

renzora::add!(SettingsPlugin, Editor);
