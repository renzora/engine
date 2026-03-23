//! Console panel for displaying logs with interactive Rhai input and built-in commands

use bevy_egui::egui::{self, Color32, CursorIcon, RichText, ScrollArea, Rounding, Key};

use crate::core::{
    ConsoleState, LogEntry, LogLevel, EditorSettings, PlayModeState, PlayState,
    SelectionHighlightMode, CollisionGizmoVisibility,
};
use crate::gizmo::state::GizmoState;
use crate::scripting::RhaiScriptEngine;
use renzora_theme::Theme;

use egui_phosphor::regular::{
    TRASH, FUNNEL, INFO, CHECK_CIRCLE, WARNING, X_CIRCLE, MAGNIFYING_GLASS, CLIPBOARD,
    CARET_RIGHT, ARROW_ELBOW_DOWN_LEFT,
};

/// Context for console command execution, bundling mutable refs to engine state
pub struct ConsoleCommandContext<'a> {
    pub settings: &'a mut EditorSettings,
    pub gizmo: &'a mut GizmoState,
    pub play_mode: &'a mut PlayModeState,
    pub fps: f64,
    pub frame_time_ms: f64,
}

/// A grouped log entry that combines consecutive identical messages
struct GroupedLogEntry<'a> {
    entry: &'a LogEntry,
    count: usize,
}

/// Render the console content
pub fn render_console_content(
    ui: &mut egui::Ui,
    console: &mut ConsoleState,
    theme: &Theme,
    rhai_engine: &RhaiScriptEngine,
    cmd_ctx: Option<&mut ConsoleCommandContext>,
) {
    let muted_color = theme.text.muted.to_color32();
    let disabled_color = theme.text.disabled.to_color32();

    let info_active = theme.semantic.accent.to_color32();
    let success_active = theme.semantic.success.to_color32();
    let warning_active = theme.semantic.warning.to_color32();
    let error_active = theme.semantic.error.to_color32();

    let available_width = ui.available_width();
    let is_narrow = available_width < 500.0;

    // --- Toolbar ---
    ui.add_space(4.0);
    render_toolbar(ui, console, theme, muted_color, disabled_color, info_active, success_active, warning_active, error_active, is_narrow);
    ui.add_space(2.0);
    ui.separator();
    ui.add_space(2.0);

    // --- Log entries (takes remaining space minus input bar) ---
    let filtered_entries: Vec<_> = console.filtered_entries().collect();
    let grouped_entries = group_consecutive_entries(&filtered_entries);

    let text_color = theme.text.primary.to_color32();
    let category_color = theme.text.hyperlink.to_color32();

    // Scroll area fills all available space except the input row at the bottom
    // Reserve: separator (~2px) + input row (~20px) + breathing room to avoid status bar clipping
    let available = ui.available_height() - 42.0;

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(available.max(20.0))
        .stick_to_bottom(console.auto_scroll)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            for grouped in &grouped_entries {
                render_log_entry(ui, grouped.entry, grouped.count, text_color, category_color, theme, is_narrow);
            }

            if grouped_entries.is_empty() {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new(crate::locale::t("console.empty"))
                            .size(13.0)
                            .color(muted_color)
                    );
                });
            }
        });

    // --- Input bar fills remaining bottom space ---
    ui.separator();
    render_input_bar(ui, console, theme, rhai_engine, cmd_ctx);
}

/// Render the toolbar — adapts layout for narrow widths
fn render_toolbar(
    ui: &mut egui::Ui,
    console: &mut ConsoleState,
    _theme: &Theme,
    muted_color: Color32,
    disabled_color: Color32,
    info_active: Color32,
    success_active: Color32,
    warning_active: Color32,
    error_active: Color32,
    is_narrow: bool,
) {
    ui.horizontal(|ui| {
        // Clear button
        let clear_label = if is_narrow {
            TRASH.to_string()
        } else {
            format!("{} {}", TRASH, crate::locale::t("console.clear"))
        };
        let clear_btn = ui.button(RichText::new(clear_label).size(12.0));
        if clear_btn.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if clear_btn.on_hover_text(crate::locale::t("console.clear")).clicked() {
            console.clear();
        }

        // Copy button
        if !is_narrow {
            let copy_btn = ui.button(RichText::new(format!("{} {}", CLIPBOARD, crate::locale::t("console.copy"))).size(12.0));
            if copy_btn.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if copy_btn.on_hover_text("Copy filtered logs to clipboard").clicked() {
                copy_filtered_logs(ui, console);
            }
        } else {
            let copy_btn = ui.button(RichText::new(CLIPBOARD).size(12.0));
            if copy_btn.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if copy_btn.on_hover_text("Copy filtered logs to clipboard").clicked() {
                copy_filtered_logs(ui, console);
            }
        }

        ui.separator();

        // Filter toggles
        let filters: &[(bool, &str, Color32)] = &[
            (console.show_info, INFO, info_active),
            (console.show_success, CHECK_CIRCLE, success_active),
            (console.show_warnings, WARNING, warning_active),
            (console.show_errors, X_CIRCLE, error_active),
        ];

        let toggles: Vec<bool> = filters.iter().map(|(active, icon, active_color)| {
            let color = if *active { *active_color } else { disabled_color };
            let btn = ui.add(egui::Button::new(
                RichText::new(*icon).color(color).size(14.0)
            ).fill(Color32::TRANSPARENT));
            if btn.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            btn.clicked()
        }).collect();

        if toggles[0] { console.show_info = !console.show_info; }
        if toggles[1] { console.show_success = !console.show_success; }
        if toggles[2] { console.show_warnings = !console.show_warnings; }
        if toggles[3] { console.show_errors = !console.show_errors; }

        ui.separator();

        // Search box — adapts width
        let search_width = if is_narrow { 80.0 } else { 150.0 };
        ui.add_space(4.0);
        ui.label(RichText::new(MAGNIFYING_GLASS).size(12.0).color(muted_color));
        ui.add(
            egui::TextEdit::singleline(&mut console.search_filter)
                .hint_text(crate::locale::t("console.search"))
                .desired_width(search_width)
        );

        // Category filter — hidden when narrow
        if !is_narrow {
            ui.add_space(8.0);
            ui.label(RichText::new(FUNNEL).size(12.0).color(muted_color));
            ui.add(
                egui::TextEdit::singleline(&mut console.category_filter)
                    .hint_text("Category...")
                    .desired_width(100.0)
            );
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !is_narrow {
                ui.checkbox(&mut console.auto_scroll, crate::locale::t("console.autoscroll"));
            } else {
                ui.checkbox(&mut console.auto_scroll, "");
            }

            let total = console.entries.len();
            let filtered: Vec<_> = console.filtered_entries().collect();
            ui.label(
                RichText::new(format!("{}/{}", filtered.len(), total))
                    .size(11.0)
                    .color(muted_color)
            );
        });
    });
}

fn copy_filtered_logs(ui: &mut egui::Ui, console: &ConsoleState) {
    let filtered: Vec<_> = console.filtered_entries().collect();
    let text = filtered
        .iter()
        .map(|e| {
            let level = match e.level {
                LogLevel::Info => "INFO",
                LogLevel::Success => "SUCCESS",
                LogLevel::Warning => "WARNING",
                LogLevel::Error => "ERROR",
            };
            if e.category.is_empty() {
                format!("[{}] {}", level, e.message)
            } else {
                format!("[{}] [{}] {}", level, e.category, e.message)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    ui.ctx().copy_text(text);
}

/// Render the input bar at the bottom of the console
fn render_input_bar(
    ui: &mut egui::Ui,
    console: &mut ConsoleState,
    theme: &Theme,
    rhai_engine: &RhaiScriptEngine,
    cmd_ctx: Option<&mut ConsoleCommandContext>,
) {
    let accent_color = theme.semantic.accent.to_color32();
    let muted_color = theme.text.muted.to_color32();
    let text_color = theme.text.primary.to_color32();

    // Fill remaining space with centered-vertical layout
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.add_space(6.0);

        // Prompt chevron
        ui.label(RichText::new(CARET_RIGHT).size(14.0).color(accent_color));

        // Frameless input field — blends with panel
        let input_id = ui.id().with("console_input");
        let response = ui.add(
            egui::TextEdit::singleline(&mut console.input_buffer)
                .hint_text("Type /help for commands...")
                .desired_width(ui.available_width() - 24.0)
                .font(egui::TextStyle::Monospace)
                .text_color(text_color)
                .frame(false)
                .id(input_id)
        );

        // Focus on first render or when requested
        if console.focus_input {
            response.request_focus();
            console.focus_input = false;
        }

        // Submit hint icon
        ui.label(RichText::new(ARROW_ELBOW_DOWN_LEFT).size(12.0).color(muted_color));

        // Handle keyboard in the input
        // Check submission via lost_focus (Enter in singleline causes focus loss)
        let submitted = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        let has_focus = response.has_focus();

        if has_focus || submitted {
            let (up, down) = ui.input(|i| {
                (
                    i.key_pressed(Key::ArrowUp),
                    i.key_pressed(Key::ArrowDown),
                )
            });

            if submitted && !console.input_buffer.trim().is_empty() {
                let command = console.input_buffer.trim().to_string();

                // Log the command as an input entry
                console.log(LogLevel::Info, "Input", format!("> {}", command));

                // Try console command first (starts with /)
                let handled = if command.starts_with('/') {
                    if let Some(ctx) = cmd_ctx {
                        execute_command(&command, console, ctx)
                    } else {
                        console.log(LogLevel::Error, "Command", "Commands not available in this context".to_string());
                        true
                    }
                } else {
                    false
                };

                // Fall through to Rhai if not a command
                if !handled {
                    match rhai_engine.eval_expression(&command) {
                        Ok(result) => {
                            if !result.is_empty() {
                                console.log(LogLevel::Success, "Output", result);
                            }
                        }
                        Err(err) => {
                            console.log(LogLevel::Error, "Output", err);
                        }
                    }
                }

                // Push to history
                console.command_history.push(command);
                console.history_index = None;
                console.saved_input.clear();
                console.input_buffer.clear();
                console.auto_scroll = true;

                // Re-focus input after submission
                console.focus_input = true;
            }

            // History navigation: Up
            if up && !console.command_history.is_empty() {
                match console.history_index {
                    None => {
                        // Save current input and go to last history entry
                        console.saved_input = console.input_buffer.clone();
                        let idx = console.command_history.len() - 1;
                        console.history_index = Some(idx);
                        console.input_buffer = console.command_history[idx].clone();
                    }
                    Some(idx) if idx > 0 => {
                        let new_idx = idx - 1;
                        console.history_index = Some(new_idx);
                        console.input_buffer = console.command_history[new_idx].clone();
                    }
                    _ => {}
                }
            }

            // History navigation: Down
            if down {
                match console.history_index {
                    Some(idx) => {
                        if idx + 1 < console.command_history.len() {
                            let new_idx = idx + 1;
                            console.history_index = Some(new_idx);
                            console.input_buffer = console.command_history[new_idx].clone();
                        } else {
                            // Restore saved input
                            console.history_index = None;
                            console.input_buffer = console.saved_input.clone();
                            console.saved_input.clear();
                        }
                    }
                    None => {}
                }
            }
        }
    });
}

// ── Console Commands ───────────────────────────────────────────────

/// Command definition for help text
struct CommandDef {
    name: &'static str,
    usage: &'static str,
    description: &'static str,
}

const COMMANDS: &[CommandDef] = &[
    CommandDef { name: "clear",     usage: "/clear",                            description: "Clear console output" },
    CommandDef { name: "help",      usage: "/help [command]",                   description: "List all commands, or show help for a specific command" },
    CommandDef { name: "set",       usage: "/set <path> <value>",               description: "Set a setting value (bool, float, int, string)" },
    CommandDef { name: "get",       usage: "/get <path>",                       description: "Query current value of a setting" },
    CommandDef { name: "toggle",    usage: "/toggle <path>",                    description: "Toggle a boolean setting" },
    CommandDef { name: "list",      usage: "/list",                             description: "List all available setting paths" },
    CommandDef { name: "wireframe", usage: "/wireframe",                        description: "Toggle wireframe mode" },
    CommandDef { name: "grid",      usage: "/grid",                             description: "Toggle grid visibility" },
    CommandDef { name: "shadows",   usage: "/shadows",                          description: "Toggle shadows" },
    CommandDef { name: "lighting",  usage: "/lighting",                         description: "Toggle lighting" },
    CommandDef { name: "snap",      usage: "/snap <translate|rotate|scale> [value]", description: "Toggle snap or set snap value" },
    CommandDef { name: "fps",       usage: "/fps",                              description: "Show current FPS" },
    CommandDef { name: "play",      usage: "/play",                             description: "Enter play mode" },
    CommandDef { name: "stop",      usage: "/stop",                             description: "Stop play mode" },
    CommandDef { name: "settings",  usage: "/settings",                         description: "Open settings panel" },
    CommandDef { name: "dev",       usage: "/dev",                              description: "Toggle dev mode" },
];

/// All known setting paths for /set, /get, /toggle, /list
const SETTING_PATHS: &[(&str, &str)] = &[
    ("grid",                    "Show grid (bool)"),
    ("subgrid",                 "Show subgrid (bool)"),
    ("axis_gizmo",              "Show axis gizmo (bool)"),
    ("grid.size",               "Grid size (float)"),
    ("grid.divisions",          "Grid divisions (uint)"),
    ("render.textures",         "Show textures (bool)"),
    ("render.wireframe",        "Wireframe overlay (bool)"),
    ("render.lighting",         "Enable lighting (bool)"),
    ("render.shadows",          "Enable shadows (bool)"),
    ("selection.highlight",     "Selection highlight mode (outline/gizmo)"),
    ("selection.on_top",        "Selection boundary on top (bool)"),
    ("collision.gizmos",        "Collision gizmo visibility (selected/always)"),
    ("dev_mode",                "Developer mode (bool)"),
    ("font_size",               "UI font size (float)"),
    ("camera.speed",            "Camera move speed (float)"),
    ("camera.look_sensitivity", "Camera look sensitivity (float)"),
    ("camera.orbit_sensitivity","Camera orbit sensitivity (float)"),
    ("camera.pan_sensitivity",  "Camera pan sensitivity (float)"),
    ("camera.zoom_sensitivity", "Camera zoom sensitivity (float)"),
    ("camera.invert_y",         "Invert Y axis (bool)"),
    ("camera.left_click_pan",   "Left-click drag camera pan (bool)"),
    ("scripts.rerun_on_ready",  "Rerun on_ready on script reload (bool)"),
    ("scripts.game_camera",     "Use game camera in play mode (bool)"),
    ("scripts.hide_cursor",     "Hide cursor in play mode (bool)"),
    ("snap.translate",          "Position snap enabled (bool)"),
    ("snap.translate.value",    "Position snap increment (float)"),
    ("snap.rotate",             "Rotation snap enabled (bool)"),
    ("snap.rotate.value",       "Rotation snap increment (float, degrees)"),
    ("snap.scale",              "Scale snap enabled (bool)"),
    ("snap.scale.value",        "Scale snap increment (float)"),
    ("snap.object",             "Object snap enabled (bool)"),
    ("snap.floor",              "Floor snap enabled (bool)"),
];

/// Get a setting value as a displayable string
fn get_setting(path: &str, ctx: &ConsoleCommandContext) -> Result<String, String> {
    match path {
        "grid"                     => Ok(ctx.settings.show_grid.to_string()),
        "subgrid"                  => Ok(ctx.settings.show_subgrid.to_string()),
        "axis_gizmo"               => Ok(ctx.settings.show_axis_gizmo.to_string()),
        "grid.size"                => Ok(ctx.settings.grid_size.to_string()),
        "grid.divisions"           => Ok(ctx.settings.grid_divisions.to_string()),
        "render.textures"          => Ok(ctx.settings.render_toggles.textures.to_string()),
        "render.wireframe"         => Ok(ctx.settings.render_toggles.wireframe.to_string()),
        "render.lighting"          => Ok(ctx.settings.render_toggles.lighting.to_string()),
        "render.shadows"           => Ok(ctx.settings.render_toggles.shadows.to_string()),
        "selection.highlight"      => Ok(match ctx.settings.selection_highlight_mode {
            SelectionHighlightMode::Outline => "outline".to_string(),
            SelectionHighlightMode::Gizmo => "gizmo".to_string(),
        }),
        "selection.on_top"         => Ok(ctx.settings.selection_boundary_on_top.to_string()),
        "collision.gizmos"         => Ok(match ctx.settings.collision_gizmo_visibility {
            CollisionGizmoVisibility::SelectedOnly => "selected".to_string(),
            CollisionGizmoVisibility::Always => "always".to_string(),
        }),
        "dev_mode"                 => Ok(ctx.settings.dev_mode.to_string()),
        "font_size"                => Ok(ctx.settings.font_size.to_string()),
        "camera.speed"             => Ok(ctx.settings.camera_settings.move_speed.to_string()),
        "camera.look_sensitivity"  => Ok(ctx.settings.camera_settings.look_sensitivity.to_string()),
        "camera.orbit_sensitivity" => Ok(ctx.settings.camera_settings.orbit_sensitivity.to_string()),
        "camera.pan_sensitivity"   => Ok(ctx.settings.camera_settings.pan_sensitivity.to_string()),
        "camera.zoom_sensitivity"  => Ok(ctx.settings.camera_settings.zoom_sensitivity.to_string()),
        "camera.invert_y"          => Ok(ctx.settings.camera_settings.invert_y.to_string()),
        "camera.left_click_pan"    => Ok(ctx.settings.camera_settings.left_click_pan.to_string()),
        "scripts.rerun_on_ready"   => Ok(ctx.settings.script_rerun_on_ready_on_reload.to_string()),
        "scripts.game_camera"      => Ok(ctx.settings.scripts_use_game_camera.to_string()),
        "scripts.hide_cursor"      => Ok(ctx.settings.hide_cursor_in_play_mode.to_string()),
        "snap.translate"           => Ok(ctx.gizmo.snap.translate_enabled.to_string()),
        "snap.translate.value"     => Ok(ctx.gizmo.snap.translate_snap.to_string()),
        "snap.rotate"              => Ok(ctx.gizmo.snap.rotate_enabled.to_string()),
        "snap.rotate.value"        => Ok(ctx.gizmo.snap.rotate_snap.to_string()),
        "snap.scale"               => Ok(ctx.gizmo.snap.scale_enabled.to_string()),
        "snap.scale.value"         => Ok(ctx.gizmo.snap.scale_snap.to_string()),
        "snap.object"              => Ok(ctx.gizmo.snap.object_snap_enabled.to_string()),
        "snap.floor"               => Ok(ctx.gizmo.snap.floor_snap_enabled.to_string()),
        _ => Err(format!("Unknown setting: {}", path)),
    }
}

/// Set a setting value from a string
fn set_setting(path: &str, value: &str, ctx: &mut ConsoleCommandContext) -> Result<String, String> {
    let parse_bool = |v: &str| -> Result<bool, String> {
        match v {
            "true" | "1" | "on" | "yes" => Ok(true),
            "false" | "0" | "off" | "no" => Ok(false),
            _ => Err(format!("Invalid bool: '{}' (use true/false/on/off)", v)),
        }
    };
    let parse_f32 = |v: &str| -> Result<f32, String> {
        v.parse::<f32>().map_err(|_| format!("Invalid number: '{}'", v))
    };
    let parse_u32 = |v: &str| -> Result<u32, String> {
        v.parse::<u32>().map_err(|_| format!("Invalid integer: '{}'", v))
    };

    match path {
        "grid"           => { ctx.settings.show_grid = parse_bool(value)?; Ok(format!("grid = {}", ctx.settings.show_grid)) }
        "subgrid"        => { ctx.settings.show_subgrid = parse_bool(value)?; Ok(format!("subgrid = {}", ctx.settings.show_subgrid)) }
        "axis_gizmo"     => { ctx.settings.show_axis_gizmo = parse_bool(value)?; Ok(format!("axis_gizmo = {}", ctx.settings.show_axis_gizmo)) }
        "grid.size"      => { ctx.settings.grid_size = parse_f32(value)?; Ok(format!("grid.size = {}", ctx.settings.grid_size)) }
        "grid.divisions" => { ctx.settings.grid_divisions = parse_u32(value)?; Ok(format!("grid.divisions = {}", ctx.settings.grid_divisions)) }
        "render.textures"  => { ctx.settings.render_toggles.textures = parse_bool(value)?; Ok(format!("render.textures = {}", ctx.settings.render_toggles.textures)) }
        "render.wireframe" => { ctx.settings.render_toggles.wireframe = parse_bool(value)?; Ok(format!("render.wireframe = {}", ctx.settings.render_toggles.wireframe)) }
        "render.lighting"  => { ctx.settings.render_toggles.lighting = parse_bool(value)?; Ok(format!("render.lighting = {}", ctx.settings.render_toggles.lighting)) }
        "render.shadows"   => { ctx.settings.render_toggles.shadows = parse_bool(value)?; Ok(format!("render.shadows = {}", ctx.settings.render_toggles.shadows)) }
        "selection.highlight" => {
            ctx.settings.selection_highlight_mode = match value {
                "outline" => SelectionHighlightMode::Outline,
                "gizmo" => SelectionHighlightMode::Gizmo,
                _ => return Err(format!("Invalid mode: '{}' (use outline/gizmo)", value)),
            };
            Ok(format!("selection.highlight = {}", value))
        }
        "selection.on_top" => { ctx.settings.selection_boundary_on_top = parse_bool(value)?; Ok(format!("selection.on_top = {}", ctx.settings.selection_boundary_on_top)) }
        "collision.gizmos" => {
            ctx.settings.collision_gizmo_visibility = match value {
                "selected" => CollisionGizmoVisibility::SelectedOnly,
                "always" => CollisionGizmoVisibility::Always,
                _ => return Err(format!("Invalid mode: '{}' (use selected/always)", value)),
            };
            Ok(format!("collision.gizmos = {}", value))
        }
        "dev_mode"   => { ctx.settings.dev_mode = parse_bool(value)?; Ok(format!("dev_mode = {}", ctx.settings.dev_mode)) }
        "font_size"  => { ctx.settings.font_size = parse_f32(value)?; Ok(format!("font_size = {}", ctx.settings.font_size)) }
        "camera.speed"             => { ctx.settings.camera_settings.move_speed = parse_f32(value)?; Ok(format!("camera.speed = {}", ctx.settings.camera_settings.move_speed)) }
        "camera.look_sensitivity"  => { ctx.settings.camera_settings.look_sensitivity = parse_f32(value)?; Ok(format!("camera.look_sensitivity = {}", ctx.settings.camera_settings.look_sensitivity)) }
        "camera.orbit_sensitivity" => { ctx.settings.camera_settings.orbit_sensitivity = parse_f32(value)?; Ok(format!("camera.orbit_sensitivity = {}", ctx.settings.camera_settings.orbit_sensitivity)) }
        "camera.pan_sensitivity"   => { ctx.settings.camera_settings.pan_sensitivity = parse_f32(value)?; Ok(format!("camera.pan_sensitivity = {}", ctx.settings.camera_settings.pan_sensitivity)) }
        "camera.zoom_sensitivity"  => { ctx.settings.camera_settings.zoom_sensitivity = parse_f32(value)?; Ok(format!("camera.zoom_sensitivity = {}", ctx.settings.camera_settings.zoom_sensitivity)) }
        "camera.invert_y"          => { ctx.settings.camera_settings.invert_y = parse_bool(value)?; Ok(format!("camera.invert_y = {}", ctx.settings.camera_settings.invert_y)) }
        "camera.left_click_pan"    => { ctx.settings.camera_settings.left_click_pan = parse_bool(value)?; Ok(format!("camera.left_click_pan = {}", ctx.settings.camera_settings.left_click_pan)) }
        "scripts.rerun_on_ready"   => { ctx.settings.script_rerun_on_ready_on_reload = parse_bool(value)?; Ok(format!("scripts.rerun_on_ready = {}", ctx.settings.script_rerun_on_ready_on_reload)) }
        "scripts.game_camera"      => { ctx.settings.scripts_use_game_camera = parse_bool(value)?; Ok(format!("scripts.game_camera = {}", ctx.settings.scripts_use_game_camera)) }
        "scripts.hide_cursor"      => { ctx.settings.hide_cursor_in_play_mode = parse_bool(value)?; Ok(format!("scripts.hide_cursor = {}", ctx.settings.hide_cursor_in_play_mode)) }
        "snap.translate"       => { ctx.gizmo.snap.translate_enabled = parse_bool(value)?; Ok(format!("snap.translate = {}", ctx.gizmo.snap.translate_enabled)) }
        "snap.translate.value" => { ctx.gizmo.snap.translate_snap = parse_f32(value)?; Ok(format!("snap.translate.value = {}", ctx.gizmo.snap.translate_snap)) }
        "snap.rotate"          => { ctx.gizmo.snap.rotate_enabled = parse_bool(value)?; Ok(format!("snap.rotate = {}", ctx.gizmo.snap.rotate_enabled)) }
        "snap.rotate.value"    => { ctx.gizmo.snap.rotate_snap = parse_f32(value)?; Ok(format!("snap.rotate.value = {}", ctx.gizmo.snap.rotate_snap)) }
        "snap.scale"           => { ctx.gizmo.snap.scale_enabled = parse_bool(value)?; Ok(format!("snap.scale = {}", ctx.gizmo.snap.scale_enabled)) }
        "snap.scale.value"     => { ctx.gizmo.snap.scale_snap = parse_f32(value)?; Ok(format!("snap.scale.value = {}", ctx.gizmo.snap.scale_snap)) }
        "snap.object"          => { ctx.gizmo.snap.object_snap_enabled = parse_bool(value)?; Ok(format!("snap.object = {}", ctx.gizmo.snap.object_snap_enabled)) }
        "snap.floor"           => { ctx.gizmo.snap.floor_snap_enabled = parse_bool(value)?; Ok(format!("snap.floor = {}", ctx.gizmo.snap.floor_snap_enabled)) }
        _ => Err(format!("Unknown setting: {}", path)),
    }
}

/// Toggle a boolean setting
fn toggle_setting(path: &str, ctx: &mut ConsoleCommandContext) -> Result<String, String> {
    macro_rules! toggle_bool {
        ($field:expr, $name:expr) => {{
            $field = !$field;
            Ok(format!("{} = {}", $name, $field))
        }};
    }

    match path {
        "grid"                   => toggle_bool!(ctx.settings.show_grid, "grid"),
        "subgrid"                => toggle_bool!(ctx.settings.show_subgrid, "subgrid"),
        "axis_gizmo"             => toggle_bool!(ctx.settings.show_axis_gizmo, "axis_gizmo"),
        "render.textures"        => toggle_bool!(ctx.settings.render_toggles.textures, "render.textures"),
        "render.wireframe"       => toggle_bool!(ctx.settings.render_toggles.wireframe, "render.wireframe"),
        "render.lighting"        => toggle_bool!(ctx.settings.render_toggles.lighting, "render.lighting"),
        "render.shadows"         => toggle_bool!(ctx.settings.render_toggles.shadows, "render.shadows"),
        "selection.on_top"       => toggle_bool!(ctx.settings.selection_boundary_on_top, "selection.on_top"),
        "dev_mode"               => toggle_bool!(ctx.settings.dev_mode, "dev_mode"),
        "camera.invert_y"        => toggle_bool!(ctx.settings.camera_settings.invert_y, "camera.invert_y"),
        "camera.left_click_pan"  => toggle_bool!(ctx.settings.camera_settings.left_click_pan, "camera.left_click_pan"),
        "scripts.rerun_on_ready" => toggle_bool!(ctx.settings.script_rerun_on_ready_on_reload, "scripts.rerun_on_ready"),
        "scripts.game_camera"    => toggle_bool!(ctx.settings.scripts_use_game_camera, "scripts.game_camera"),
        "scripts.hide_cursor"    => toggle_bool!(ctx.settings.hide_cursor_in_play_mode, "scripts.hide_cursor"),
        "snap.translate"         => toggle_bool!(ctx.gizmo.snap.translate_enabled, "snap.translate"),
        "snap.rotate"            => toggle_bool!(ctx.gizmo.snap.rotate_enabled, "snap.rotate"),
        "snap.scale"             => toggle_bool!(ctx.gizmo.snap.scale_enabled, "snap.scale"),
        "snap.object"            => toggle_bool!(ctx.gizmo.snap.object_snap_enabled, "snap.object"),
        "snap.floor"             => toggle_bool!(ctx.gizmo.snap.floor_snap_enabled, "snap.floor"),
        _ => Err(format!("'{}' is not a toggleable boolean setting", path)),
    }
}

/// Execute a console command. Returns true if the input was handled as a command.
fn execute_command(
    input: &str,
    console: &mut ConsoleState,
    ctx: &mut ConsoleCommandContext,
) -> bool {
    let trimmed = input.strip_prefix('/').unwrap_or(input);
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.is_empty() {
        return false;
    }

    let cmd = parts[0];
    let args = &parts[1..];

    let result: Result<String, String> = match cmd {
        "clear" => {
            console.clear();
            Ok("Console cleared.".to_string())
        }

        "help" => {
            if let Some(name) = args.first() {
                if let Some(def) = COMMANDS.iter().find(|c| c.name == *name) {
                    Ok(format!("{}\n  {}", def.usage, def.description))
                } else {
                    Err(format!("Unknown command: /{}", name))
                }
            } else {
                let mut help = String::from("Available commands:\n");
                for def in COMMANDS {
                    help.push_str(&format!("  {:30} {}\n", def.usage, def.description));
                }
                help.push_str("\nType /help <command> for details. Settings use /set, /get, /toggle, /list.");
                Ok(help)
            }
        }

        "set" => {
            if args.len() < 2 {
                Err("Usage: /set <path> <value>".to_string())
            } else {
                set_setting(args[0], args[1], ctx)
            }
        }

        "get" => {
            if args.is_empty() {
                Err("Usage: /get <path>".to_string())
            } else {
                match get_setting(args[0], ctx) {
                    Ok(val) => Ok(format!("{} = {}", args[0], val)),
                    Err(e) => Err(e),
                }
            }
        }

        "toggle" => {
            if args.is_empty() {
                Err("Usage: /toggle <path>".to_string())
            } else {
                toggle_setting(args[0], ctx)
            }
        }

        "list" => {
            let mut out = String::from("Available setting paths:\n");
            for (path, desc) in SETTING_PATHS {
                out.push_str(&format!("  {:30} {}\n", path, desc));
            }
            Ok(out)
        }

        "wireframe" => toggle_setting("render.wireframe", ctx),
        "grid"      => toggle_setting("grid", ctx),
        "shadows"   => toggle_setting("render.shadows", ctx),
        "lighting"  => toggle_setting("render.lighting", ctx),

        "snap" => {
            if args.is_empty() {
                Err("Usage: /snap <translate|rotate|scale> [value]".to_string())
            } else {
                match args[0] {
                    "translate" => {
                        if let Some(val) = args.get(1) {
                            match val.parse::<f32>() {
                                Ok(v) => { ctx.gizmo.snap.translate_snap = v; ctx.gizmo.snap.translate_enabled = true; Ok(format!("snap.translate = true, value = {}", v)) }
                                Err(_) => Err(format!("Invalid number: '{}'", val)),
                            }
                        } else {
                            ctx.gizmo.snap.translate_enabled = !ctx.gizmo.snap.translate_enabled;
                            Ok(format!("snap.translate = {}", ctx.gizmo.snap.translate_enabled))
                        }
                    }
                    "rotate" => {
                        if let Some(val) = args.get(1) {
                            match val.parse::<f32>() {
                                Ok(v) => { ctx.gizmo.snap.rotate_snap = v; ctx.gizmo.snap.rotate_enabled = true; Ok(format!("snap.rotate = true, value = {}", v)) }
                                Err(_) => Err(format!("Invalid number: '{}'", val)),
                            }
                        } else {
                            ctx.gizmo.snap.rotate_enabled = !ctx.gizmo.snap.rotate_enabled;
                            Ok(format!("snap.rotate = {}", ctx.gizmo.snap.rotate_enabled))
                        }
                    }
                    "scale" => {
                        if let Some(val) = args.get(1) {
                            match val.parse::<f32>() {
                                Ok(v) => { ctx.gizmo.snap.scale_snap = v; ctx.gizmo.snap.scale_enabled = true; Ok(format!("snap.scale = true, value = {}", v)) }
                                Err(_) => Err(format!("Invalid number: '{}'", val)),
                            }
                        } else {
                            ctx.gizmo.snap.scale_enabled = !ctx.gizmo.snap.scale_enabled;
                            Ok(format!("snap.scale = {}", ctx.gizmo.snap.scale_enabled))
                        }
                    }
                    other => Err(format!("Unknown snap axis: '{}' (use translate/rotate/scale)", other)),
                }
            }
        }

        "fps" => {
            Ok(format!("FPS: {:.1}  ({:.2} ms/frame)", ctx.fps, ctx.frame_time_ms))
        }

        "play" => {
            if ctx.play_mode.is_in_play_mode() {
                Err("Already in play mode.".to_string())
            } else {
                ctx.play_mode.request_play = true;
                Ok("Entering play mode...".to_string())
            }
        }

        "stop" => {
            if ctx.play_mode.is_editing() {
                Err("Not in play mode.".to_string())
            } else {
                ctx.play_mode.request_stop = true;
                Ok("Stopping play mode...".to_string())
            }
        }

        "settings" => {
            ctx.settings.show_settings = !ctx.settings.show_settings;
            Ok(if ctx.settings.show_settings { "Settings panel opened.".to_string() } else { "Settings panel closed.".to_string() })
        }

        "dev" => toggle_setting("dev_mode", ctx),

        _ => Err(format!("Unknown command: /{}. Type /help for a list of commands.", cmd)),
    };

    match result {
        Ok(msg) => console.log(LogLevel::Success, "Command", msg),
        Err(msg) => console.log(LogLevel::Error, "Command", msg),
    }

    true
}

/// Group consecutive identical log entries
fn group_consecutive_entries<'a>(entries: &[&'a LogEntry]) -> Vec<GroupedLogEntry<'a>> {
    let mut grouped = Vec::new();

    for entry in entries {
        let should_group = grouped.last().map_or(false, |last: &GroupedLogEntry| {
            last.entry.level == entry.level
                && last.entry.category == entry.category
                && last.entry.message == entry.message
        });

        if should_group {
            if let Some(last) = grouped.last_mut() {
                last.count += 1;
            }
        } else {
            grouped.push(GroupedLogEntry {
                entry,
                count: 1,
            });
        }
    }

    grouped
}

fn render_log_entry(
    ui: &mut egui::Ui,
    entry: &LogEntry,
    count: usize,
    text_color: Color32,
    category_color: Color32,
    theme: &Theme,
    is_narrow: bool,
) {
    let color = match entry.level {
        LogLevel::Info => theme.semantic.accent.to_color32(),
        LogLevel::Success => theme.semantic.success.to_color32(),
        LogLevel::Warning => theme.semantic.warning.to_color32(),
        LogLevel::Error => theme.semantic.error.to_color32(),
    };

    ui.horizontal(|ui| {
        // Count badge
        if count > 1 {
            let badge_text = if count > 999 {
                "999+".to_string()
            } else {
                count.to_string()
            };

            let badge_color = color.gamma_multiply(0.3);
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(24.0, 16.0),
                egui::Sense::hover()
            );

            ui.painter().rect_filled(rect, Rounding::same(8), badge_color);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &badge_text,
                egui::FontId::proportional(10.0),
                color
            );

            ui.add_space(2.0);
        }

        // Level icon
        let icon = match entry.level {
            LogLevel::Info => INFO,
            LogLevel::Success => CHECK_CIRCLE,
            LogLevel::Warning => WARNING,
            LogLevel::Error => X_CIRCLE,
        };
        ui.label(RichText::new(icon).color(color).size(12.0));

        // Category badge — hidden when narrow to save space
        if !entry.category.is_empty() && !is_narrow {
            ui.label(
                RichText::new(format!("[{}]", entry.category))
                    .size(11.0)
                    .color(category_color)
            );
        }

        // Message — use monospace for Input/Output categories
        let is_repl = entry.category == "Input" || entry.category == "Output";
        if is_repl {
            ui.label(
                RichText::new(&entry.message)
                    .size(12.0)
                    .color(text_color)
                    .monospace()
            );
        } else {
            ui.label(
                RichText::new(&entry.message)
                    .size(12.0)
                    .color(text_color)
            );
        }
    });
}
