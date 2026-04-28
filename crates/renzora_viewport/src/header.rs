//! Viewport header bar — render toggles, inline settings, and dropdown menus.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Vec2};
use egui_phosphor::regular::*;
use renzora_editor::{
    ActiveTool, EditorCommands, ToolOptionsRegistry, ViewportModeOptionsRegistry,
};
use renzora_theme::ThemeManager;

use crate::settings::*;

/// Height of the viewport header bar.
pub const HEADER_HEIGHT: f32 = 28.0;

/// Uniform height for every interactive control in the header strip.
const BTN_H: f32 = 22.0;

/// View angle preset for the camera.
#[derive(Clone, Copy)]
enum ViewAngle {
    Front, Back, Left, Right, Top, Bottom,
}

impl ViewAngle {
    const ALL: &'static [ViewAngle] = &[
        Self::Front, Self::Back, Self::Left, Self::Right, Self::Top, Self::Bottom,
    ];
    fn label(&self) -> &'static str {
        match self {
            Self::Front => "Front", Self::Back => "Back",
            Self::Left => "Left",   Self::Right => "Right",
            Self::Top => "Top",     Self::Bottom => "Bottom",
        }
    }
    fn shortcut(&self) -> &'static str {
        match self {
            Self::Front => "Num1",     Self::Back => "Ctrl+Num1",
            Self::Left => "Ctrl+Num3", Self::Right => "Num3",
            Self::Top => "Num7",       Self::Bottom => "Ctrl+Num7",
        }
    }
    fn yaw_pitch(&self) -> (f32, f32) {
        use std::f32::consts::{FRAC_PI_2, PI};
        match self {
            Self::Front => (0.0, 0.0),       Self::Back => (PI, 0.0),
            Self::Right => (FRAC_PI_2, 0.0), Self::Left => (-FRAC_PI_2, 0.0),
            Self::Top => (0.0, FRAC_PI_2),   Self::Bottom => (0.0, -FRAC_PI_2),
        }
    }
}

/// Render the viewport header bar with toggles and dropdown menus.
pub fn viewport_header(ui: &mut egui::Ui, world: &World) {
    let theme = match world.get_resource::<ThemeManager>() {
        Some(tm) => &tm.active_theme,
        None => return,
    };
    let cmds = world.get_resource::<EditorCommands>();
    let settings = world.get_resource::<ViewportSettings>();
    let (Some(settings), Some(cmds)) = (settings, cmds) else { return };

    let rect = ui.available_rect_before_wrap();
    let header_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), HEADER_HEIGHT));
    ui.advance_cursor_after_rect(header_rect);
    ui.painter().rect_filled(header_rect, 0.0, theme.surfaces.panel.to_color32());

    let active = theme.semantic.accent.to_color32();
    let inactive = theme.widgets.inactive_bg.to_color32();
    let hovered = theme.widgets.hovered_bg.to_color32();

    // Clamp the inner rect height to exactly BTN_H. Slight top-weighted
    // padding nudges the whole strip down a hair so taller widgets
    // (DragValue text, icon glyphs) read as visually centered.
    const TOP_PAD: f32 = 6.0;
    let bottom_pad = (header_rect.height() - BTN_H - TOP_PAD).max(0.0);
    let inner = Rect::from_min_max(
        egui::Pos2::new(header_rect.min.x + 8.0, header_rect.min.y + TOP_PAD),
        egui::Pos2::new(header_rect.max.x - 8.0, header_rect.max.y - bottom_pad),
    );

    // Left-aligned action strip (Undo/Redo/Save/Play/Scripts). These stay
    // visible and in a fixed location regardless of viewport mode or active
    // tool, so they don't get hidden by the mode-specific drawers below.
    let actions_w = render_left_actions(ui, world, cmds, theme, inner, hovered);

    // Right-aligned dropdown strip (Display / Snap / Camera). Painted before
    // the centered strip so the centering math can keep clear of it.
    let dropdowns_w = render_right_dropdowns(ui, settings, cmds, theme, inner);

    // Render once per frame at a centered rect of the last measured width.
    // Two-pass rendering was doubling popups (sizing pass still spawned Areas).
    let width_id = ui.id().with("viewport_header_width");
    let cached_w: f32 = ui.memory(|m| m.data.get_temp(width_id)).unwrap_or(600.0);
    // Shrink the available-for-centering width so the centered strip never
    // overlaps the left actions or the right dropdowns.
    let centering_min_x = inner.min.x + actions_w + 8.0;
    let centering_max_x = inner.max.x - dropdowns_w - 8.0;
    let available_for_center = (centering_max_x - centering_min_x).max(0.0);
    let content_w = cached_w.min(available_for_center);
    let centering_center_x = (centering_min_x + centering_max_x) / 2.0;
    let content_x = centering_center_x - content_w / 2.0;
    let centered_rect = Rect::from_min_size(
        egui::Pos2::new(content_x, inner.min.y),
        Vec2::new(content_w, inner.height()),
    );

    let mut strip = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(centered_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    strip.set_height(BTN_H);
    render_strip_contents(&mut strip, world, settings, cmds,
        active, inactive, hovered);

    let measured = strip.min_rect().width();
    if (measured - cached_w).abs() > 0.5 {
        ui.memory_mut(|m| m.data.insert_temp(width_id, measured));
    }
}

fn render_strip_contents(
    ui: &mut egui::Ui,
    world: &World,
    settings: &ViewportSettings,
    cmds: &EditorCommands,
    active: Color32,
    inactive: Color32,
    hovered: Color32,
) {
    // Uniform height across buttons and drag-value sliders so the whole strip
    // reads as one visual row.
    ui.spacing_mut().interact_size.y = BTN_H;
    ui.spacing_mut().item_spacing.x = 3.0;
    ui.spacing_mut().item_spacing.y = 0.0;
    // DragValue height = interact_size.y + button_padding.y * 2, so 0 vertical
    // padding keeps sliders exactly as tall as our 22px icon buttons.
    ui.spacing_mut().button_padding = Vec2::new(4.0, 0.0);

    // ── Viewport mode dropdown (always first so users can always switch back) ─
    mode_dropdown(ui, settings, cmds, active, inactive, hovered);
    separator(ui);

    // ── Mode-specific header (Edit / Sculpt / Paint / Animate) ──────────────
    let mode_drawer = world
        .get_resource::<ViewportModeOptionsRegistry>()
        .and_then(|r| r.drawer_for(settings.viewport_mode));
    if let Some(drawer) = mode_drawer {
        drawer(ui, world);
        return;
    }

    // ── Context-sensitive tool options (Photoshop-style) ─────────────────────
    let active_tool = world.get_resource::<ActiveTool>().copied().unwrap_or_default();
    let tool_drawer = world
        .get_resource::<ToolOptionsRegistry>()
        .and_then(|r| r.drawer_for(active_tool));

    if let Some(drawer) = tool_drawer {
        drawer(ui, world);
        return;
    }

    // ── Inline snapping: T / R / S toggles with inline value editors ─────────
    let snap = settings.snap;
    snap_pair(ui, ARROWS_OUT_CARDINAL, "Translate", snap.translate_enabled, snap.translate_snap, 0.01..=100.0, 0.02, 2, active, inactive, hovered, cmds,
        |s, on| s.snap.translate_enabled = on,
        |s, v| s.snap.translate_snap = v);
    snap_pair(ui, ARROW_CLOCKWISE, "Rotate", snap.rotate_enabled, snap.rotate_snap, 1.0..=90.0, 0.2, 0, active, inactive, hovered, cmds,
        |s, on| s.snap.rotate_enabled = on,
        |s, v| s.snap.rotate_snap = v);
    snap_pair(ui, ARROWS_OUT, "Scale", snap.scale_enabled, snap.scale_snap, 0.01..=10.0, 0.01, 2, active, inactive, hovered, cmds,
        |s, on| s.snap.scale_enabled = on,
        |s, v| s.snap.scale_snap = v);

    separator(ui);

    // ── Camera speed inline drag (rendered as a pill to match snap groups) ─
    let mut cam_speed = settings.camera.move_speed;
    egui::Frame::default()
        .fill(inactive)
        .corner_radius(CornerRadius::same(3))
        .inner_margin(egui::Margin::symmetric(2, 0))
        .stroke(egui::Stroke::NONE)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.set_height(BTN_H);

            let (rect, _) = ui.allocate_exact_size(Vec2::new(18.0, BTN_H), Sense::hover());
            if ui.is_rect_visible(rect) {
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
                    VIDEO_CAMERA, FontId::proportional(13.0), Color32::WHITE);
            }

            let resp = ui.scope(|ui| {
                let w = &mut ui.style_mut().visuals.widgets;
                w.inactive.bg_fill = Color32::TRANSPARENT;
                w.inactive.weak_bg_fill = Color32::TRANSPARENT;
                w.inactive.bg_stroke.width = 0.0;
                w.hovered.bg_fill = Color32::from_black_alpha(40);
                w.hovered.weak_bg_fill = Color32::from_black_alpha(40);
                w.hovered.bg_stroke.width = 0.0;
                ui.add(
                    egui::DragValue::new(&mut cam_speed)
                        .range(0.1..=100.0)
                        .speed(0.5)
                        .max_decimals(1),
                )
            }).inner;
            if resp.changed() {
                cmds.push(move |w: &mut World| {
                    if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                        s.camera.move_speed = cam_speed;
                    }
                });
            }
        });
}

// ── Mode dropdown ────────────────────────────────────────────────────────────

fn mode_dropdown(
    ui: &mut egui::Ui,
    settings: &ViewportSettings,
    cmds: &EditorCommands,
    active: Color32,
    inactive: Color32,
    hovered: Color32,
) {
    use renzora::core::viewport_types::ViewportMode;

    let current = settings.viewport_mode;
    let label = current.label();
    // Approximate width: label + caret.
    let btn_size = Vec2::new(80.0, BTN_H);
    let (rect, resp) = ui.allocate_exact_size(btn_size, Sense::click());
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    let popup_id = ui.make_persistent_id("viewport_mode_dropdown");
    if ui.is_rect_visible(rect) {
        let bg = if resp.hovered() { hovered } else { inactive };
        ui.painter().rect_filled(rect, CornerRadius::same(3), bg);
        let text_rect = Rect::from_min_max(
            Pos2::new(rect.min.x + 6.0, rect.min.y),
            Pos2::new(rect.max.x - 14.0, rect.max.y),
        );
        ui.painter().text(text_rect.left_center(), egui::Align2::LEFT_CENTER, label,
            FontId::proportional(12.0), Color32::WHITE);
        ui.painter().text(Pos2::new(rect.max.x - 8.0, rect.center().y),
            egui::Align2::CENTER_CENTER, CARET_DOWN, FontId::proportional(10.0), Color32::WHITE);
    }
    if resp.clicked() {
        #[allow(deprecated)]
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }
    #[allow(deprecated)]
    egui::popup_below_widget(ui, popup_id, &resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(120.0);
        ui.style_mut().spacing.item_spacing.y = 2.0;
        for mode in ViewportMode::ALL {
            let is_current = *mode == current;
            let (row_rect, row_resp) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), BTN_H), Sense::click());
            if row_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
            if ui.is_rect_visible(row_rect) {
                let bg = if is_current { active }
                    else if row_resp.hovered() { hovered }
                    else { Color32::TRANSPARENT };
                ui.painter().rect_filled(row_rect, CornerRadius::same(3), bg);
                ui.painter().text(
                    Pos2::new(row_rect.min.x + 8.0, row_rect.center().y),
                    egui::Align2::LEFT_CENTER, mode.label(),
                    FontId::proportional(12.0), Color32::WHITE);
            }
            if row_resp.clicked() {
                let m = *mode;
                cmds.push(move |w: &mut World| {
                    if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                        s.viewport_mode = m;
                    }
                });
                ui.close();
            }
        }
    });
}

// ── Inline helpers ───────────────────────────────────────────────────────────

fn separator(ui: &mut egui::Ui) {
    ui.add_space(3.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, BTN_H - 4.0), Sense::hover());
    ui.painter().line_segment(
        [rect.center_top(), rect.center_bottom()],
        egui::Stroke::new(1.0, Color32::from_gray(60)),
    );
    ui.add_space(3.0);
}

/// Paint the left-aligned "session actions" strip (Undo / Redo / Save / Play /
/// Scripts). Returns the width consumed so the centered strip can make room.
fn render_left_actions(
    ui: &mut egui::Ui,
    world: &World,
    cmds: &EditorCommands,
    theme: &renzora_theme::Theme,
    inner: Rect,
    hovered_bg: Color32,
) -> f32 {
    use renzora::core::{PlayModeState, SaveSceneRequested, SceneCamera};

    let btn_size = Vec2::new(26.0, BTN_H);
    let gap_small = 2.0;
    let gap_group = 8.0;

    let can_undo_redo = world
        .get_resource::<renzora_undo::UndoStacks>()
        .map(|s| (s.can_undo(&s.active), s.can_redo(&s.active)))
        .unwrap_or((false, false));
    let (can_undo, can_redo) = can_undo_redo;

    let can_save = world
        .get_resource::<renzora_ui::DocumentTabState>()
        .and_then(|tabs| tabs.tabs.get(tabs.active_tab).map(|t| t.is_modified))
        .unwrap_or(false);

    let has_scene_camera = {
        let mut found = false;
        for archetype in world.archetypes().iter() {
            for ae in archetype.entities() {
                if world.get::<SceneCamera>(ae.id()).is_some() { found = true; break; }
            }
            if found { break; }
        }
        found
    };

    let play_mode = world.get_resource::<PlayModeState>();
    let is_playing = play_mode.map(|p| p.is_in_play_mode()).unwrap_or(false);
    let is_scripts = play_mode.map(|p| p.is_scripts_only()).unwrap_or(false);
    let is_editing = play_mode.map(|p| p.is_editing()).unwrap_or(true);
    let play_color = theme.semantic.success.to_color32();
    let scripts_color = theme.semantic.accent.to_color32();
    let stop_color = theme.semantic.error.to_color32();

    let muted = theme.text.muted.to_color32();
    let primary = theme.text.primary.to_color32();

    let mut x = inner.min.x;
    let y = inner.min.y;

    let action_btn = |ui: &mut egui::Ui, x: &mut f32, icon: &str, enabled: bool,
                      color: Color32, tooltip: &str, id_salt: &str|
                      -> egui::Response {
        let rect = Rect::from_min_size(Pos2::new(*x, y), btn_size);
        let sense = if enabled { Sense::click() } else { Sense::hover() };
        let resp = ui.interact(rect, ui.id().with(id_salt), sense);
        if enabled && resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            ui.painter().rect_filled(rect, CornerRadius::same(3), hovered_bg);
        }
        let glyph_color = if enabled { color } else { muted };
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            icon,
            FontId::proportional(14.0),
            glyph_color,
        );
        resp.clone().on_hover_text(tooltip);
        *x += btn_size.x + gap_small;
        resp
    };

    // Undo
    let r = action_btn(ui, &mut x, ARROW_U_UP_LEFT, can_undo, primary, "Undo (Ctrl+Z)", "vp_hdr_undo");
    if can_undo && r.clicked() {
        cmds.push(|w: &mut World| renzora_undo::undo_once(w));
    }
    // Redo
    let r = action_btn(ui, &mut x, ARROW_U_UP_RIGHT, can_redo, primary, "Redo (Ctrl+Y)", "vp_hdr_redo");
    if can_redo && r.clicked() {
        cmds.push(|w: &mut World| renzora_undo::redo_once(w));
    }

    x += gap_group - gap_small;

    // Save
    let save_tip = if can_save { "Save Scene (Ctrl+S)" } else { "No unsaved changes" };
    let r = action_btn(ui, &mut x, FLOPPY_DISK, can_save, primary, save_tip, "vp_hdr_save");
    if can_save && r.clicked() {
        cmds.push(|w: &mut World| { w.insert_resource(SaveSceneRequested); });
    }

    x += gap_group - gap_small;

    // Play (or Stop when in full play mode)
    let play_clickable = is_playing || (is_editing && has_scene_camera);
    let (play_icon, play_icon_color, play_tip) = if is_playing {
        (STOP, stop_color, "Stop")
    } else if !has_scene_camera {
        (PLAY, muted, "Scene has no camera — add one to play")
    } else {
        (PLAY, play_color, "Play (F5)")
    };
    let r = action_btn(ui, &mut x, play_icon, play_clickable, play_icon_color, play_tip, "vp_hdr_play");
    if play_clickable && r.clicked() {
        cmds.push(|w: &mut World| {
            if let Some(mut pm) = w.get_resource_mut::<PlayModeState>() {
                if pm.is_in_play_mode() {
                    pm.request_stop = true;
                } else if pm.is_scripts_only() {
                    pm.request_stop = true;
                    pm.request_play = true;
                } else if pm.is_editing() {
                    pm.request_play = true;
                }
            }
        });
    }

    // Scripts (or Stop when running scripts-only)
    let scripts_clickable = is_scripts || is_editing;
    let (scr_icon, scr_color, scr_tip) = if is_scripts {
        (STOP, scripts_color, "Stop Scripts")
    } else if is_playing {
        (CODE, muted, "Stop play mode first")
    } else {
        (CODE, scripts_color, "Run Scripts (Shift+F5)")
    };
    let r = action_btn(ui, &mut x, scr_icon, scripts_clickable && !is_playing, scr_color, scr_tip, "vp_hdr_scripts");
    if scripts_clickable && !is_playing && r.clicked() {
        cmds.push(|w: &mut World| {
            if let Some(mut pm) = w.get_resource_mut::<PlayModeState>() {
                if pm.is_scripts_only() {
                    pm.request_stop = true;
                } else if pm.is_editing() {
                    pm.request_scripts_only = true;
                }
            }
        });
    }

    (x - inner.min.x).max(0.0)
}

/// Right-aligned dropdown strip (Display / Snap / Camera). Returns the total
/// width consumed so the centered strip can avoid overlapping it.
fn render_right_dropdowns(
    ui: &mut egui::Ui,
    settings: &ViewportSettings,
    cmds: &EditorCommands,
    theme: &renzora_theme::Theme,
    inner: Rect,
) -> f32 {
    const DROP_W: f32 = 40.0;
    const GAP: f32 = 3.0;
    const COUNT: f32 = 3.0;
    let total_w = DROP_W * COUNT + GAP * (COUNT - 1.0);

    let strip_rect = Rect::from_min_max(
        Pos2::new(inner.max.x - total_w, inner.min.y),
        Pos2::new(inner.max.x, inner.max.y),
    );
    let mut strip = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(strip_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    strip.set_height(BTN_H);
    strip.spacing_mut().item_spacing.x = GAP;
    strip.spacing_mut().item_spacing.y = 0.0;
    strip.spacing_mut().button_padding = Vec2::new(0.0, 0.0);

    let icon_color = theme.text.secondary.to_color32();
    let inactive = theme.widgets.inactive_bg.to_color32();
    let hovered = theme.widgets.hovered_bg.to_color32();
    let muted = theme.text.muted.to_color32();

    display_dropdown(&mut strip, settings, cmds, icon_color, inactive, hovered, muted, theme);
    snap_dropdown(&mut strip, settings, cmds, icon_color, inactive, hovered, muted, theme);
    camera_dropdown(&mut strip, settings, cmds, icon_color, inactive, hovered, muted, theme);

    total_w
}

fn snap_pair(
    ui: &mut egui::Ui, icon: &str, tip_name: &str, enabled: bool, value: f32,
    range: std::ops::RangeInclusive<f32>, speed: f64, decimals: usize,
    active: Color32, inactive: Color32, hovered: Color32,
    cmds: &EditorCommands,
    toggle_fn: fn(&mut ViewportSettings, bool),
    value_fn: fn(&mut ViewportSettings, f32),
) {
    // Render the icon toggle and the drag value as a single pill: one shared
    // background, icon on the left, slider on the right.
    egui::Frame::default()
        .fill(if enabled { active } else { inactive })
        .corner_radius(CornerRadius::same(3))
        .inner_margin(egui::Margin::symmetric(2, 0))
        .stroke(egui::Stroke::NONE)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.set_height(BTN_H);

            // Icon toggle
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(18.0, BTN_H), Sense::click());
            if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
            if ui.is_rect_visible(rect) {
                if resp.hovered() && !enabled {
                    ui.painter().rect_filled(rect, CornerRadius::same(2), hovered);
                }
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, icon,
                    FontId::proportional(13.0), Color32::WHITE);
            }
            let clicked = resp.clicked();
            let _ = resp.on_hover_text(format!("{} snap: {}", tip_name, if enabled { "ON" } else { "OFF" }));
            if clicked {
                cmds.push(move |w: &mut World| {
                    if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                        toggle_fn(&mut s, !enabled);
                    }
                });
            }

            // DragValue with transparent chrome so the pill shows through.
            let mut v = value;
            let dv = ui.scope(|ui| {
                let w = &mut ui.style_mut().visuals.widgets;
                w.inactive.bg_fill = Color32::TRANSPARENT;
                w.inactive.weak_bg_fill = Color32::TRANSPARENT;
                w.inactive.bg_stroke.width = 0.0;
                w.hovered.bg_fill = Color32::from_black_alpha(40);
                w.hovered.weak_bg_fill = Color32::from_black_alpha(40);
                w.hovered.bg_stroke.width = 0.0;
                ui.add(
                    egui::DragValue::new(&mut v)
                        .range(range)
                        .speed(speed)
                        .max_decimals(decimals),
                )
            }).inner;
            if dv.changed() {
                cmds.push(move |w: &mut World| {
                    if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                        value_fn(&mut s, v);
                    }
                });
            }
        });
}

// ── Dropdown helpers ────────────────────────────────────────────────────────

fn dropdown_button_ui(
    ui: &mut egui::Ui, icon: &str,
    icon_color: Color32, bg_color: Color32, hovered_color: Color32, caret_color: Color32,
) -> egui::Response {
    let size = Vec2::new(40.0, BTN_H);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    if ui.is_rect_visible(rect) {
        let fill = if resp.hovered() { hovered_color } else { bg_color };
        ui.painter().rect_filled(rect, CornerRadius::same(3), fill);
        ui.painter().text(Pos2::new(rect.left() + 12.0, rect.center().y),
            egui::Align2::CENTER_CENTER, icon, FontId::proportional(15.0), icon_color);
        ui.painter().text(Pos2::new(rect.right() - 7.0, rect.center().y),
            egui::Align2::CENTER_CENTER, CARET_DOWN, FontId::proportional(8.0), caret_color);
    }
    resp
}

fn check_row(ui: &mut egui::Ui, label: &str, value: &mut bool) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(12.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.checkbox(value, "");
        });
    });
}

// ── Display dropdown (visualization + render flags + overlays + collision) ──

fn display_dropdown(
    ui: &mut egui::Ui, settings: &ViewportSettings, cmds: &EditorCommands,
    icon_color: Color32, bg_color: Color32, hovered_color: Color32, caret_color: Color32,
    theme: &renzora_theme::Theme,
) {
    let id = ui.make_persistent_id("vp_display_drop");
    let resp = dropdown_button_ui(ui, EYE, icon_color, bg_color, hovered_color, caret_color);
    if resp.clicked() {
        #[allow(deprecated)]
        ui.memory_mut(|mem| mem.toggle_popup(id));
    }
    let resp = resp.on_hover_text("Display");
    let label_color = theme.text.muted.to_color32();

    let current_viz = settings.visualization_mode;
    let mut toggles = settings.render_toggles;
    let mut show_grid = settings.show_grid;
    let mut show_subgrid = settings.show_subgrid;
    let mut show_axis = settings.show_axis_gizmo;
    let mut show_scene_icons = settings.show_scene_icons;
    let collision_vis = settings.collision_gizmo_visibility;

    #[allow(deprecated)]
    egui::popup_below_widget(ui, id, &resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(190.0);
        ui.style_mut().spacing.item_spacing.y = 4.0;

        ui.label(RichText::new("Visualization").small().color(label_color));
        ui.add_space(2.0);
        for mode in VisualizationMode::ALL {
            let selected = current_viz == *mode;
            let label = if selected { format!("• {}", mode.label()) } else { format!("  {}", mode.label()) };
            if ui.add(egui::Button::new(&label).fill(Color32::TRANSPARENT)
                .corner_radius(CornerRadius::same(2))
                .min_size(Vec2::new(ui.available_width(), 0.0))).clicked()
            {
                let m = *mode;
                cmds.push(move |w: &mut World| {
                    if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                        s.visualization_mode = m;
                    }
                });
            }
        }

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);
        ui.label(RichText::new("Render").small().color(label_color));
        ui.add_space(2.0);
        check_row(ui, "Mesh",      &mut toggles.mesh);
        check_row(ui, "Textures",  &mut toggles.textures);
        check_row(ui, "Wireframe", &mut toggles.wireframe);
        check_row(ui, "Lighting",  &mut toggles.lighting);
        check_row(ui, "Shadows",   &mut toggles.shadows);

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);
        ui.label(RichText::new("Overlays").small().color(label_color));
        ui.add_space(2.0);
        check_row(ui, "Grid", &mut show_grid);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Sub-grid").size(12.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled(show_grid, egui::Checkbox::new(&mut show_subgrid, ""));
            });
        });
        check_row(ui, "Axis Gizmo", &mut show_axis);
        check_row(ui, "Scene Icons", &mut show_scene_icons);

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);
        ui.label(RichText::new("Collision Gizmos").small().color(label_color));
        ui.add_space(2.0);
        let mut selected_only = collision_vis == CollisionGizmoVisibility::SelectedOnly;
        if ui.radio_value(&mut selected_only, true,  RichText::new("Selected Only").size(12.0)).clicked() {
            cmds.push(|w: &mut World| { if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() { s.collision_gizmo_visibility = CollisionGizmoVisibility::SelectedOnly; } });
        }
        if ui.radio_value(&mut selected_only, false, RichText::new("Always").size(12.0)).clicked() {
            cmds.push(|w: &mut World| { if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() { s.collision_gizmo_visibility = CollisionGizmoVisibility::Always; } });
        }
    });

    cmds.push(move |w: &mut World| {
        if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
            s.render_toggles = toggles;
            s.show_grid = show_grid;
            s.show_subgrid = show_subgrid;
            s.show_axis_gizmo = show_axis;
            s.show_scene_icons = show_scene_icons;
        }
    });
}

// ── Camera dropdown (projection + view angles + sensitivities, merged) ─────

fn camera_dropdown(
    ui: &mut egui::Ui, settings: &ViewportSettings, cmds: &EditorCommands,
    icon_color: Color32, bg_color: Color32, hovered_color: Color32, caret_color: Color32,
    theme: &renzora_theme::Theme,
) {
    let id = ui.make_persistent_id("vp_camera_drop");
    let resp = dropdown_button_ui(ui, CUBE, icon_color, bg_color, hovered_color, caret_color);
    if resp.clicked() {
        #[allow(deprecated)]
        ui.memory_mut(|mem| mem.toggle_popup(id));
    }
    let resp = resp.on_hover_text("Camera");
    let label_color = theme.text.muted.to_color32();
    let proj = settings.projection_mode;
    let mut cam = settings.camera;

    #[allow(deprecated)]
    egui::popup_below_widget(ui, id, &resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(220.0);
        ui.style_mut().spacing.item_spacing.y = 4.0;

        ui.label(RichText::new("Projection").small().color(label_color));
        ui.add_space(2.0);
        let persp_label = if proj == ProjectionMode::Perspective { "• Perspective" } else { "  Perspective" };
        if ui.add(egui::Button::new(persp_label).fill(Color32::TRANSPARENT).corner_radius(CornerRadius::same(2)).min_size(Vec2::new(ui.available_width(), 0.0))).clicked() {
            cmds.push(|w: &mut World| { if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() { s.projection_mode = ProjectionMode::Perspective; } });
        }
        let ortho_label = if proj == ProjectionMode::Orthographic { "• Orthographic" } else { "  Orthographic" };
        if ui.add(egui::Button::new(ortho_label).fill(Color32::TRANSPARENT).corner_radius(CornerRadius::same(2)).min_size(Vec2::new(ui.available_width(), 0.0))).clicked() {
            cmds.push(|w: &mut World| { if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() { s.projection_mode = ProjectionMode::Orthographic; } });
        }

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);
        ui.label(RichText::new("View Angles").small().color(label_color));
        ui.add_space(2.0);
        for view in ViewAngle::ALL {
            let label = format!("{}  ({})", view.label(), view.shortcut());
            if ui.add(egui::Button::new(&label).fill(Color32::TRANSPARENT).corner_radius(CornerRadius::same(2)).min_size(Vec2::new(ui.available_width(), 0.0))).clicked() {
                let (yaw, pitch) = view.yaw_pitch();
                cmds.push(move |w: &mut World| {
                    if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                        s.pending_view_angle = Some(ViewAngleCommand { yaw, pitch });
                    }
                });
                ui.close();
            }
        }

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);
        ui.label(RichText::new("Sensitivities").small().color(label_color));
        ui.add_space(4.0);
        let drag_row = |ui: &mut egui::Ui, label_text: &str, val: &mut f32, range: std::ops::RangeInclusive<f64>, speed: f64, decimals: usize| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(label_text).size(12.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(egui::DragValue::new(val).range(range).speed(speed).max_decimals(decimals));
                });
            });
        };
        drag_row(ui, "Look",  &mut cam.look_sensitivity,  0.05..=2.0, 0.05, 2);
        drag_row(ui, "Orbit", &mut cam.orbit_sensitivity, 0.05..=2.0, 0.05, 2);
        drag_row(ui, "Pan",   &mut cam.pan_sensitivity,   0.1..=5.0,  0.1,  1);
        drag_row(ui, "Zoom",  &mut cam.zoom_sensitivity,  0.1..=5.0,  0.1,  1);

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);
        check_row(ui, "Invert Y Axis", &mut cam.invert_y);
        check_row(ui, "Distance Relative Speed", &mut cam.distance_relative_speed);
        ui.add_space(4.0);
        if ui.add(egui::Button::new(RichText::new("Reset to Defaults").size(12.0))
            .min_size(Vec2::new(ui.available_width(), 24.0))).clicked()
        {
            cam = CameraSettingsState::default();
        }
    });

    cmds.push(move |w: &mut World| {
        if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() { s.camera = cam; }
    });
}

// ── Snap dropdown (object/floor + edge/scale-bottom transform aids) ────────

fn snap_dropdown(
    ui: &mut egui::Ui, settings: &ViewportSettings, cmds: &EditorCommands,
    icon_color: Color32, bg_color: Color32, hovered_color: Color32, caret_color: Color32,
    theme: &renzora_theme::Theme,
) {
    let id = ui.make_persistent_id("vp_snap_drop");
    let s = &settings.snap;
    let any_active = s.object_snap_enabled || s.floor_snap_enabled
        || s.translate_edge_snap || s.scale_bottom_anchor;
    let snap_bg = if any_active { theme.semantic.accent.to_color32() } else { bg_color };
    let resp = dropdown_button_ui(ui, MAGNET, icon_color, snap_bg, hovered_color, caret_color);
    if resp.clicked() {
        #[allow(deprecated)]
        ui.memory_mut(|mem| mem.toggle_popup(id));
    }
    let resp = resp.on_hover_text("Snapping");
    let active_btn = theme.semantic.accent.to_color32();
    let inactive_btn = theme.widgets.inactive_bg.to_color32();
    let label_color = theme.text.muted.to_color32();
    let mut snap = settings.snap;

    #[allow(deprecated)]
    egui::popup_below_widget(ui, id, &resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(220.0);
        ui.style_mut().spacing.item_spacing.y = 4.0;
        ui.label(RichText::new("Object Snapping").small().color(label_color));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(RichText::new("Objects").size(12.0))
                .fill(if snap.object_snap_enabled { active_btn } else { inactive_btn })
                .min_size(Vec2::new(70.0, 20.0))).clicked() {
                snap.object_snap_enabled = !snap.object_snap_enabled;
            }
            ui.add(egui::DragValue::new(&mut snap.object_snap_distance).range(0.1..=10.0).speed(0.1).max_decimals(1).suffix(" dist"));
        });
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(RichText::new("Floor").size(12.0))
                .fill(if snap.floor_snap_enabled { active_btn } else { inactive_btn })
                .min_size(Vec2::new(70.0, 20.0))).clicked() {
                snap.floor_snap_enabled = !snap.floor_snap_enabled;
            }
            ui.add(egui::DragValue::new(&mut snap.floor_y).range(-1000.0..=1000.0).speed(0.1).max_decimals(1).suffix(" Y"));
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);
        ui.label(RichText::new("Transform Aids").small().color(label_color));
        ui.add_space(2.0);
        check_row(ui, "Edge Snap (align AABB min to grid)", &mut snap.translate_edge_snap);
        check_row(ui, "Scale from Bottom (keep AABB floor on Y)", &mut snap.scale_bottom_anchor);
    });

    cmds.push(move |w: &mut World| {
        if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() { s.snap = snap; }
    });
}
