//! UI Canvas panel — 2D visual editor for laying out bevy_ui widgets.
//!
//! Renders an egui canvas that mirrors the bevy_ui hierarchy. Each UiWidget
//! entity is drawn as a rectangle that can be selected, moved, and resized.

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use egui_phosphor::regular;
use renzora_editor::{EditorCommands, EditorPanel, EditorSelection, PanelLocation};
use renzora_theme::ThemeManager;

use crate::components::{UiCanvas, UiWidget, UiWidgetType};

// ── Snapshot types ────────────────────────────────────────────────────────────

/// A snapshot of one UI widget taken from the ECS each frame.
#[derive(Clone, Debug)]
struct WidgetSnapshot {
    entity: Entity,
    name: String,
    widget_type: UiWidgetType,
    locked: bool,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    parent: Option<Entity>,
    has_bg: bool,
    bg_color: [f32; 4],
    has_border: bool,
    border_color: [f32; 4],
}

/// Canvas editor state.
struct CanvasState {
    /// Pan offset in canvas pixels.
    pan: Vec2,
    /// Zoom level (1.0 = 100%).
    zoom: f32,
    /// Snapped widget data from the ECS.
    widgets: Vec<WidgetSnapshot>,
    /// Canvas entities.
    canvases: Vec<(Entity, String)>,
    /// Active canvas entity being edited.
    active_canvas: Option<Entity>,
    /// Drag state for moving widgets.
    dragging: Option<DragState>,
    /// Drag state for resizing.
    resizing: Option<ResizeState>,
    /// Box-select state.
    box_select: Option<BoxSelectState>,
    /// Canvas background resolution (logical game window size).
    canvas_width: f32,
    canvas_height: f32,
    /// Multi-select: entities currently selected (in addition to EditorSelection).
    multi_selection: Vec<Entity>,
    /// Snap-to-grid enabled.
    snap_enabled: bool,
    /// Grid spacing in logical UI pixels.
    grid_size: f32,
    /// Show grid lines on canvas.
    show_grid: bool,
    /// Clipboard for copy/paste (widget type + offset from first widget).
    clipboard: Vec<ClipboardEntry>,
}

#[derive(Clone)]
struct ClipboardEntry {
    widget_type: UiWidgetType,
    name: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    has_bg: bool,
    bg_color: [f32; 4],
    has_border: bool,
    border_color: [f32; 4],
}

#[derive(Clone)]
struct DragState {
    entities: Vec<Entity>,
    start_pos: Pos2,
    /// Original positions for each entity (same order as entities).
    originals: Vec<(f32, f32)>,
}

#[derive(Clone)]
struct ResizeState {
    entity: Entity,
    start_pos: Pos2,
    original_w: f32,
    original_h: f32,
    original_x: f32,
    original_y: f32,
    handle: ResizeHandle,
}

#[derive(Clone)]
struct BoxSelectState {
    start: Pos2,
    current: Pos2,
}

#[derive(Clone, Copy, PartialEq)]
enum ResizeHandle {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl CanvasState {
    fn new() -> Self {
        Self {
            zoom: 1.0,
            canvas_width: 1280.0,
            canvas_height: 720.0,
            pan: Vec2::ZERO,
            widgets: Vec::new(),
            canvases: Vec::new(),
            active_canvas: None,
            dragging: None,
            resizing: None,
            box_select: None,
            multi_selection: Vec::new(),
            snap_enabled: true,
            grid_size: 10.0,
            show_grid: true,
            clipboard: Vec::new(),
        }
    }

    /// Returns all selected entities (primary + multi).
    fn all_selected(&self, primary: Option<Entity>) -> Vec<Entity> {
        let mut all = self.multi_selection.clone();
        if let Some(e) = primary {
            if !all.contains(&e) {
                all.push(e);
            }
        }
        all
    }

    fn toggle_multi(&mut self, entity: Entity) {
        if let Some(pos) = self.multi_selection.iter().position(|e| *e == entity) {
            self.multi_selection.remove(pos);
        } else {
            self.multi_selection.push(entity);
        }
    }
}

/// Snap a value to the nearest grid line.
fn snap(v: f32, grid: f32) -> f32 {
    (v / grid).round() * grid
}

// ── Panel ─────────────────────────────────────────────────────────────────────

pub struct UiCanvasPanel {
    state: RwLock<CanvasState>,
}

impl Default for UiCanvasPanel {
    fn default() -> Self {
        Self {
            state: RwLock::new(CanvasState::new()),
        }
    }
}

/// Convert a Val to design-space pixels given a reference dimension.
///
/// Handles `Val::Percent` (converting back using reference) and `Val::Px`
/// (for backwards compatibility with older scenes).
fn val_to_design_px(v: bevy::ui::Val, reference: f32) -> f32 {
    match v {
        bevy::ui::Val::Percent(p) => p * reference / 100.0,
        bevy::ui::Val::Px(px) => px,
        _ => 0.0,
    }
}

/// Convert a widget snapshot position to screen rect given canvas_rect and zoom.
fn ws_screen_rect(ws: &WidgetSnapshot, canvas_rect: Rect, z: f32) -> Rect {
    let x = canvas_rect.min.x + ws.x * z;
    let y = canvas_rect.min.y + ws.y * z;
    Rect::from_min_size(Pos2::new(x, y), Vec2::new(ws.width * z, ws.height * z))
}

impl EditorPanel for UiCanvasPanel {
    fn id(&self) -> &str {
        "ui_canvas"
    }

    fn title(&self) -> &str {
        "UI Canvas"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::FRAME_CORNERS)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };
        let commands = match world.get_resource::<EditorCommands>() {
            Some(c) => c,
            None => return,
        };
        let selection = match world.get_resource::<EditorSelection>() {
            Some(s) => s,
            None => return,
        };

        let mut state = self.state.write().unwrap();

        // ── Snapshot canvases ────────────────────────────────────────────
        state.canvases.clear();
        for archetype in world.archetypes().iter() {
            for arch_entity in archetype.entities() {
                let entity = arch_entity.id();
                if world.get::<UiCanvas>(entity).is_some() {
                    let name = world
                        .get::<Name>(entity)
                        .map(|n| n.as_str().to_string())
                        .unwrap_or_else(|| "Unnamed Canvas".to_string());
                    state.canvases.push((entity, name));
                }
            }
        }

        // Auto-select first canvas if none active
        if state.active_canvas.is_none()
            || !state
                .canvases
                .iter()
                .any(|(e, _)| Some(*e) == state.active_canvas)
        {
            state.active_canvas = state.canvases.first().map(|(e, _)| *e);
        }

        // Sync canvas size from active canvas component's reference resolution
        if let Some(active) = state.active_canvas {
            if let Some(canvas) = world.get::<UiCanvas>(active) {
                state.canvas_width = canvas.reference_width;
                state.canvas_height = canvas.reference_height;
            }
        }

        // Reference resolution for px↔percent conversion in closures
        let ref_w = state.canvas_width;
        let ref_h = state.canvas_height;

        // ── Toolbar ─────────────────────────────────────────────────────
        let text_muted = theme.text.muted.to_color32();
        let accent = theme.semantic.accent.to_color32();
        let surface = theme.surfaces.panel.to_color32();

        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!("{} Canvas:", regular::FRAME_CORNERS))
                    .size(11.0)
                    .color(text_muted),
            );

            if state.canvases.is_empty() {
                ui.label(
                    egui::RichText::new("No canvases — add one from Widgets")
                        .size(11.0)
                        .color(text_muted),
                );
            } else {
                let mut clicked_canvas = None;
                for (entity, name) in &state.canvases {
                    let is_active = state.active_canvas == Some(*entity);
                    let color = if is_active { accent } else { text_muted };
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(name).size(11.0).color(color),
                            )
                            .fill(if is_active {
                                surface
                            } else {
                                Color32::TRANSPARENT
                            }),
                        )
                        .clicked()
                    {
                        clicked_canvas = Some(*entity);
                    }
                }
                if let Some(e) = clicked_canvas {
                    state.active_canvas = Some(e);
                }
            }

            // Right side: grid toggle, snap toggle, zoom
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);

                // Zoom label
                ui.label(
                    egui::RichText::new(format!("{:.0}%", state.zoom * 100.0))
                        .size(10.0)
                        .color(text_muted),
                );

                ui.separator();

                // Grid size
                let mut gs = state.grid_size;
                ui.add(egui::DragValue::new(&mut gs).range(2.0..=100.0).speed(1.0).prefix("grid: "));
                state.grid_size = gs;

                // Snap toggle
                let snap_color = if state.snap_enabled { accent } else { text_muted };
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new(regular::DOTS_NINE).size(14.0).color(snap_color),
                    ).fill(Color32::TRANSPARENT))
                    .on_hover_text("Toggle snap to grid")
                    .clicked()
                {
                    state.snap_enabled = !state.snap_enabled;
                }

                // Grid toggle
                let grid_color = if state.show_grid { accent } else { text_muted };
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new(regular::GRID_FOUR).size(14.0).color(grid_color),
                    ).fill(Color32::TRANSPARENT))
                    .on_hover_text("Toggle grid")
                    .clicked()
                {
                    state.show_grid = !state.show_grid;
                }
            });
        });

        // ── Align toolbar (visible when multi-selected) ─────────────────
        let selected_entity = selection.get();
        let all_sel = state.all_selected(selected_entity);

        if all_sel.len() >= 2 {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!("{} selected", all_sel.len()))
                        .size(10.0)
                        .color(text_muted),
                );
                ui.separator();

                let align_buttons: &[(&str, &str, AlignAction)] = &[
                    (regular::ALIGN_LEFT, "Align left", AlignAction::Left),
                    (regular::ALIGN_CENTER_HORIZONTAL, "Align center H", AlignAction::CenterH),
                    (regular::ALIGN_RIGHT, "Align right", AlignAction::Right),
                    (regular::ALIGN_TOP, "Align top", AlignAction::Top),
                    (regular::ALIGN_CENTER_VERTICAL, "Align center V", AlignAction::CenterV),
                    (regular::ALIGN_BOTTOM, "Align bottom", AlignAction::Bottom),
                ];
                for (icon, tooltip, action) in align_buttons {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(*icon).size(13.0).color(text_muted),
                            )
                            .fill(Color32::TRANSPARENT),
                        )
                        .on_hover_text(*tooltip)
                        .clicked()
                    {
                        let snapshots: Vec<_> = state
                            .widgets
                            .iter()
                            .filter(|w| all_sel.contains(&w.entity))
                            .cloned()
                            .collect();
                        let moves = compute_align(&snapshots, *action);
                        for (entity, new_x, new_y) in moves {
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    if let Some(mut node) = em.get_mut::<Node>() {
                                        node.left = bevy::ui::Val::Percent(new_x / ref_w * 100.0);
                                        node.top = bevy::ui::Val::Percent(new_y / ref_h * 100.0);
                                        node.position_type = bevy::ui::PositionType::Absolute;
                                    }
                                }
                            });
                        }
                    }
                }

                // Distribute
                ui.separator();
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(regular::ARROWS_OUT_LINE_HORIZONTAL)
                                .size(13.0)
                                .color(text_muted),
                        )
                        .fill(Color32::TRANSPARENT),
                    )
                    .on_hover_text("Distribute horizontally")
                    .clicked()
                {
                    let snapshots: Vec<_> = state
                        .widgets
                        .iter()
                        .filter(|w| all_sel.contains(&w.entity))
                        .cloned()
                        .collect();
                    let moves = compute_distribute_h(&snapshots);
                    for (entity, new_x) in moves {
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(entity) {
                                if let Some(mut node) = em.get_mut::<Node>() {
                                    node.left = bevy::ui::Val::Percent(new_x / ref_w * 100.0);
                                    node.position_type = bevy::ui::PositionType::Absolute;
                                }
                            }
                        });
                    }
                }
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(regular::ARROWS_OUT_LINE_VERTICAL)
                                .size(13.0)
                                .color(text_muted),
                        )
                        .fill(Color32::TRANSPARENT),
                    )
                    .on_hover_text("Distribute vertically")
                    .clicked()
                {
                    let snapshots: Vec<_> = state
                        .widgets
                        .iter()
                        .filter(|w| all_sel.contains(&w.entity))
                        .cloned()
                        .collect();
                    let moves = compute_distribute_v(&snapshots);
                    for (entity, new_y) in moves {
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(entity) {
                                if let Some(mut node) = em.get_mut::<Node>() {
                                    node.top = bevy::ui::Val::Percent(new_y / ref_h * 100.0);
                                    node.position_type = bevy::ui::PositionType::Absolute;
                                }
                            }
                        });
                    }
                }
            });
        }

        ui.separator();

        // ── Snapshot widgets for active canvas ───────────────────────────
        state.widgets.clear();
        if let Some(active_canvas) = state.active_canvas {
            for archetype in world.archetypes().iter() {
                for arch_entity in archetype.entities() {
                    let entity = arch_entity.id();
                    let Some(widget) = world.get::<UiWidget>(entity) else {
                        continue;
                    };
                    if !is_descendant_of(world, entity, active_canvas) {
                        continue;
                    }
                    let Some(node) = world.get::<Node>(entity) else {
                        continue;
                    };

                    let name = world
                        .get::<Name>(entity)
                        .map(|n| n.as_str().to_string())
                        .unwrap_or_default();
                    let bg = world.get::<BackgroundColor>(entity);
                    let border = world.get::<BorderColor>(entity);
                    let parent = world.get::<ChildOf>(entity).map(|c| c.parent());

                    state.widgets.push(WidgetSnapshot {
                        entity,
                        name,
                        widget_type: widget.widget_type.clone(),
                        locked: widget.locked,
                        x: val_to_design_px(node.left, ref_w),
                        y: val_to_design_px(node.top, ref_h),
                        width: val_to_design_px(node.width, ref_w),
                        height: val_to_design_px(node.height, ref_h),
                        parent,
                        has_bg: bg.is_some(),
                        bg_color: bg
                            .map(|b| b.0.to_srgba().to_f32_array())
                            .unwrap_or([0.0; 4]),
                        has_border: border.is_some(),
                        border_color: border
                            .map(|b| b.top.to_srgba().to_f32_array())
                            .unwrap_or([0.0; 4]),
                    });
                }
            }
        }

        // ── Canvas area ──────────────────────────────────────────────────
        let available = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(available, egui::Sense::click_and_drag());
        let painter = ui.painter_at(available);

        // Background
        painter.rect_filled(available, 0.0, Color32::from_rgb(30, 30, 35));

        // Handle pan (middle mouse / right drag)
        if response.dragged_by(egui::PointerButton::Middle)
            || response.dragged_by(egui::PointerButton::Secondary)
        {
            state.pan += response.drag_delta();
        }

        // Handle zoom (scroll)
        let scroll = ui.input(|i| i.raw_scroll_delta.y);
        if scroll != 0.0 && response.hovered() {
            let factor = if scroll > 0.0 { 1.1 } else { 1.0 / 1.1 };
            state.zoom = (state.zoom * factor).clamp(0.1, 5.0);
        }

        // Canvas origin: center of the panel, offset by pan
        let origin = Pos2::new(
            available.center().x + state.pan.x,
            available.center().y + state.pan.y,
        );
        let z = state.zoom;

        // Draw canvas background (game window area)
        let cw = state.canvas_width * z;
        let ch = state.canvas_height * z;
        let canvas_rect = Rect::from_min_size(
            Pos2::new(origin.x - cw / 2.0, origin.y - ch / 2.0),
            Vec2::new(cw, ch),
        );
        painter.rect_filled(canvas_rect, 0.0, Color32::from_rgb(20, 20, 24));
        painter.rect_stroke(
            canvas_rect,
            0.0,
            Stroke::new(1.0, Color32::from_rgb(60, 60, 70)),
            egui::StrokeKind::Outside,
        );

        // ── Grid lines ───────────────────────────────────────────────────
        if state.show_grid {
            let grid_color = Color32::from_rgba_unmultiplied(255, 255, 255, 8);
            let gs = state.grid_size * z;
            if gs > 3.0 {
                // Vertical lines
                let mut x = canvas_rect.min.x;
                while x <= canvas_rect.max.x {
                    painter.line_segment(
                        [Pos2::new(x, canvas_rect.min.y), Pos2::new(x, canvas_rect.max.y)],
                        Stroke::new(0.5, grid_color),
                    );
                    x += gs;
                }
                // Horizontal lines
                let mut y = canvas_rect.min.y;
                while y <= canvas_rect.max.y {
                    painter.line_segment(
                        [Pos2::new(canvas_rect.min.x, y), Pos2::new(canvas_rect.max.x, y)],
                        Stroke::new(0.5, grid_color),
                    );
                    y += gs;
                }
            }
        }

        // Size label
        painter.text(
            Pos2::new(canvas_rect.center().x, canvas_rect.max.y + 14.0),
            egui::Align2::CENTER_CENTER,
            format!(
                "{}x{}",
                state.canvas_width as u32, state.canvas_height as u32
            ),
            egui::FontId::proportional(10.0),
            text_muted,
        );

        // Recalculate all_sel after widgets are snapshot (for drawing)
        let all_sel = state.all_selected(selected_entity);

        // ── Draw widgets ─────────────────────────────────────────────────
        let widget_snapshots = state.widgets.clone();
        for ws in &widget_snapshots {
            let rect = ws_screen_rect(ws, canvas_rect, z);

            // Background
            let bg = if ws.has_bg {
                Color32::from_rgba_unmultiplied(
                    (ws.bg_color[0] * 255.0) as u8,
                    (ws.bg_color[1] * 255.0) as u8,
                    (ws.bg_color[2] * 255.0) as u8,
                    (ws.bg_color[3] * 255.0) as u8,
                )
            } else {
                Color32::from_rgba_unmultiplied(50, 50, 60, 40)
            };
            painter.rect_filled(rect, 2.0, bg);

            // Border / selection highlight
            let is_sel = all_sel.contains(&ws.entity);
            let border_c = if is_sel {
                accent
            } else if ws.has_border {
                Color32::from_rgba_unmultiplied(
                    (ws.border_color[0] * 255.0) as u8,
                    (ws.border_color[1] * 255.0) as u8,
                    (ws.border_color[2] * 255.0) as u8,
                    (ws.border_color[3] * 255.0) as u8,
                )
            } else {
                Color32::from_rgba_unmultiplied(80, 80, 90, 100)
            };
            let sw = if is_sel { 2.0 } else { 1.0 };
            painter.rect_stroke(rect, 2.0, Stroke::new(sw, border_c), egui::StrokeKind::Outside);

            // Widget label
            if rect.height() > 16.0 && rect.width() > 30.0 {
                let label = format!("{} {}", ws.widget_type.icon(), ws.name);
                painter.text(
                    Pos2::new(rect.min.x + 4.0, rect.min.y + 2.0),
                    egui::Align2::LEFT_TOP,
                    &label,
                    egui::FontId::proportional(10.0 * z.min(1.5)),
                    if is_sel { accent } else { text_muted },
                );
            }

            // Resize handles for selected widget (single selection only)
            if is_sel && all_sel.len() == 1 && !ws.locked {
                let hs = 6.0;
                for (hx, hy) in [
                    (rect.min.x, rect.min.y),
                    (rect.max.x, rect.min.y),
                    (rect.min.x, rect.max.y),
                    (rect.max.x, rect.max.y),
                ] {
                    painter.rect_filled(
                        Rect::from_center_size(Pos2::new(hx, hy), Vec2::splat(hs)),
                        1.0,
                        accent,
                    );
                }
            }
        }

        // ── Box-select rendering ─────────────────────────────────────────
        if let Some(bs) = &state.box_select {
            let sel_rect = Rect::from_two_pos(bs.start, bs.current);
            painter.rect_filled(
                sel_rect,
                0.0,
                Color32::from_rgba_unmultiplied(100, 150, 255, 25),
            );
            painter.rect_stroke(
                sel_rect,
                0.0,
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(100, 150, 255, 120)),
                egui::StrokeKind::Outside,
            );
        }

        // ── Keyboard shortcuts ───────────────────────────────────────────
        let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd);
        let shift = ui.input(|i| i.modifiers.shift);

        // Delete selected widgets
        if response.has_focus() || response.hovered() {
            if ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
                let to_delete = all_sel.clone();
                if !to_delete.is_empty() {
                    state.multi_selection.clear();
                    commands.push(move |world: &mut World| {
                        for entity in &to_delete {
                            if world.get_entity(*entity).is_ok() {
                                world.despawn(*entity);
                            }
                        }
                        if let Some(sel) = world.get_resource::<EditorSelection>() {
                            sel.set(None);
                        }
                    });
                }
            }

            // Arrow key nudge
            let nudge = if shift { 10.0 } else { 1.0 };
            let nudge = if state.snap_enabled && !shift {
                state.grid_size
            } else {
                nudge
            };

            let mut dx = 0.0_f32;
            let mut dy = 0.0_f32;
            if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                dx = -nudge;
            }
            if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                dx = nudge;
            }
            if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                dy = -nudge;
            }
            if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                dy = nudge;
            }

            if dx != 0.0 || dy != 0.0 {
                let entities_to_nudge = all_sel.clone();
                let snap_on = state.snap_enabled && !shift;
                let grid = state.grid_size;
                // Read current positions from snapshots
                let positions: Vec<_> = entities_to_nudge
                    .iter()
                    .filter_map(|e| {
                        widget_snapshots.iter().find(|w| w.entity == *e).map(|w| (*e, w.x, w.y))
                    })
                    .collect();
                for (entity, wx, wy) in positions {
                    let mut nx = wx + dx;
                    let mut ny = wy + dy;
                    if snap_on {
                        nx = snap(nx, grid);
                        ny = snap(ny, grid);
                    }
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut node) = em.get_mut::<Node>() {
                                node.left = bevy::ui::Val::Percent(nx / ref_w * 100.0);
                                node.top = bevy::ui::Val::Percent(ny / ref_h * 100.0);
                                node.position_type = bevy::ui::PositionType::Absolute;
                            }
                        }
                    });
                }
            }

            // Copy (Ctrl+C)
            if ctrl && ui.input(|i| i.key_pressed(egui::Key::C)) {
                state.clipboard.clear();
                let sel_widgets: Vec<_> = widget_snapshots
                    .iter()
                    .filter(|w| all_sel.contains(&w.entity))
                    .collect();
                if !sel_widgets.is_empty() {
                    let base_x = sel_widgets.iter().map(|w| w.x).fold(f32::MAX, f32::min);
                    let base_y = sel_widgets.iter().map(|w| w.y).fold(f32::MAX, f32::min);
                    for w in &sel_widgets {
                        state.clipboard.push(ClipboardEntry {
                            widget_type: w.widget_type.clone(),
                            name: w.name.clone(),
                            x: w.x - base_x,
                            y: w.y - base_y,
                            width: w.width,
                            height: w.height,
                            has_bg: w.has_bg,
                            bg_color: w.bg_color,
                            has_border: w.has_border,
                            border_color: w.border_color,
                        });
                    }
                }
            }

            // Paste (Ctrl+V)
            if ctrl && ui.input(|i| i.key_pressed(egui::Key::V)) {
                let entries = state.clipboard.clone();
                let active_canvas = state.active_canvas;
                if !entries.is_empty() {
                    commands.push(move |world: &mut World| {
                        for entry in &entries {
                            let offset_x = entry.x + 20.0; // Paste offset
                            let offset_y = entry.y + 20.0;
                            let wt = entry.widget_type.clone();
                            crate::palette::spawn_widget(world, &wt, active_canvas);
                            // After spawn, update position/size on the last spawned entity
                            // spawn_widget selects the entity, so we can find it via selection
                            if let Some(sel) = world.get_resource::<EditorSelection>() {
                                if let Some(entity) = sel.get() {
                                    if let Ok(mut em) = world.get_entity_mut(entity) {
                                        if let Some(mut name) = em.get_mut::<Name>() {
                                            name.set(format!("{} (copy)", entry.name));
                                        }
                                        if let Some(mut node) = em.get_mut::<Node>() {
                                            node.left = bevy::ui::Val::Percent(offset_x / ref_w * 100.0);
                                            node.top = bevy::ui::Val::Percent(offset_y / ref_h * 100.0);
                                            node.width = bevy::ui::Val::Percent(entry.width / ref_w * 100.0);
                                            node.height = bevy::ui::Val::Percent(entry.height / ref_h * 100.0);
                                            node.position_type = bevy::ui::PositionType::Absolute;
                                        }
                                        if entry.has_bg {
                                            let c = entry.bg_color;
                                            em.insert(BackgroundColor(Color::srgba(
                                                c[0], c[1], c[2], c[3],
                                            )));
                                        }
                                        if entry.has_border {
                                            let c = entry.border_color;
                                            em.insert(BorderColor::all(Color::srgba(
                                                c[0], c[1], c[2], c[3],
                                            )));
                                        }
                                    }
                                }
                            }
                        }
                    });
                }
            }

            // Duplicate (Ctrl+D)
            if ctrl && ui.input(|i| i.key_pressed(egui::Key::D)) {
                let sel_widgets: Vec<ClipboardEntry> = widget_snapshots
                    .iter()
                    .filter(|w| all_sel.contains(&w.entity))
                    .map(|w| ClipboardEntry {
                        widget_type: w.widget_type.clone(),
                        name: w.name.clone(),
                        x: w.x + 20.0,
                        y: w.y + 20.0,
                        width: w.width,
                        height: w.height,
                        has_bg: w.has_bg,
                        bg_color: w.bg_color,
                        has_border: w.has_border,
                        border_color: w.border_color,
                    })
                    .collect();
                let active_canvas = state.active_canvas;
                if !sel_widgets.is_empty() {
                    commands.push(move |world: &mut World| {
                        for entry in &sel_widgets {
                            let wt = entry.widget_type.clone();
                            crate::palette::spawn_widget(world, &wt, active_canvas);
                            if let Some(sel) = world.get_resource::<EditorSelection>() {
                                if let Some(entity) = sel.get() {
                                    if let Ok(mut em) = world.get_entity_mut(entity) {
                                        if let Some(mut name) = em.get_mut::<Name>() {
                                            name.set(format!("{} (copy)", entry.name));
                                        }
                                        if let Some(mut node) = em.get_mut::<Node>() {
                                            node.left = bevy::ui::Val::Percent(entry.x / ref_w * 100.0);
                                            node.top = bevy::ui::Val::Percent(entry.y / ref_h * 100.0);
                                            node.width = bevy::ui::Val::Percent(entry.width / ref_w * 100.0);
                                            node.height = bevy::ui::Val::Percent(entry.height / ref_h * 100.0);
                                            node.position_type = bevy::ui::PositionType::Absolute;
                                        }
                                        if entry.has_bg {
                                            let c = entry.bg_color;
                                            em.insert(BackgroundColor(Color::srgba(
                                                c[0], c[1], c[2], c[3],
                                            )));
                                        }
                                        if entry.has_border {
                                            let c = entry.border_color;
                                            em.insert(BorderColor::all(Color::srgba(
                                                c[0], c[1], c[2], c[3],
                                            )));
                                        }
                                    }
                                }
                            }
                        }
                    });
                }
            }

            // Select all (Ctrl+A)
            if ctrl && ui.input(|i| i.key_pressed(egui::Key::A)) {
                state.multi_selection = widget_snapshots.iter().map(|w| w.entity).collect();
            }
        }

        // ── Interaction: click, shift-click, box-select, drag ────────────

        // Click: select or shift-toggle
        if response.clicked() {
            let pointer = response.interact_pointer_pos().unwrap_or_default();
            let mut clicked_entity = None;

            for ws in widget_snapshots.iter().rev() {
                let rect = ws_screen_rect(ws, canvas_rect, z);
                if rect.contains(pointer) {
                    clicked_entity = Some(ws.entity);
                    break;
                }
            }

            if shift {
                // Shift+click: toggle in multi-selection
                if let Some(e) = clicked_entity {
                    state.toggle_multi(e);
                }
            } else {
                // Normal click: clear multi, set primary
                state.multi_selection.clear();
                let entity = clicked_entity;
                commands.push(move |world: &mut World| {
                    if let Some(sel) = world.get_resource::<EditorSelection>() {
                        sel.set(entity);
                    }
                });
            }
        }

        // Drag start
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(pointer) = response.interact_pointer_pos() {
                let all_sel = state.all_selected(selected_entity);

                // Check resize handles (single selection only)
                if all_sel.len() == 1 {
                    if let Some(ws) = widget_snapshots.iter().find(|w| w.entity == all_sel[0]) {
                        let rect = ws_screen_rect(ws, canvas_rect, z);
                        let handles = [
                            (Pos2::new(rect.min.x, rect.min.y), ResizeHandle::TopLeft),
                            (Pos2::new(rect.max.x, rect.min.y), ResizeHandle::TopRight),
                            (Pos2::new(rect.min.x, rect.max.y), ResizeHandle::BottomLeft),
                            (Pos2::new(rect.max.x, rect.max.y), ResizeHandle::BottomRight),
                        ];
                        let mut found_handle = false;
                        for (pos, handle) in &handles {
                            let hr = Rect::from_center_size(*pos, Vec2::splat(8.0));
                            if hr.contains(pointer) && !ws.locked {
                                state.resizing = Some(ResizeState {
                                    entity: ws.entity,
                                    start_pos: pointer,
                                    original_w: ws.width,
                                    original_h: ws.height,
                                    original_x: ws.x,
                                    original_y: ws.y,
                                    handle: *handle,
                                });
                                found_handle = true;
                                break;
                            }
                        }
                        if found_handle {
                            // handled below
                        } else if rect.contains(pointer) && !ws.locked {
                            // Single widget drag
                            state.dragging = Some(DragState {
                                entities: all_sel.clone(),
                                start_pos: pointer,
                                originals: all_sel
                                    .iter()
                                    .filter_map(|e| {
                                        widget_snapshots
                                            .iter()
                                            .find(|w| w.entity == *e)
                                            .map(|w| (w.x, w.y))
                                    })
                                    .collect(),
                            });
                        } else {
                            // Start box-select
                            state.box_select = Some(BoxSelectState {
                                start: pointer,
                                current: pointer,
                            });
                        }
                    } else {
                        state.box_select = Some(BoxSelectState {
                            start: pointer,
                            current: pointer,
                        });
                    }
                } else if !all_sel.is_empty() {
                    // Multi-drag: check if pointer is on any selected widget
                    let on_selected = widget_snapshots.iter().rev().any(|ws| {
                        all_sel.contains(&ws.entity)
                            && ws_screen_rect(ws, canvas_rect, z).contains(pointer)
                    });
                    if on_selected {
                        state.dragging = Some(DragState {
                            entities: all_sel.clone(),
                            start_pos: pointer,
                            originals: all_sel
                                .iter()
                                .filter_map(|e| {
                                    widget_snapshots
                                        .iter()
                                        .find(|w| w.entity == *e)
                                        .map(|w| (w.x, w.y))
                                })
                                .collect(),
                        });
                    } else {
                        state.box_select = Some(BoxSelectState {
                            start: pointer,
                            current: pointer,
                        });
                    }
                } else {
                    // Nothing selected, start box select
                    state.box_select = Some(BoxSelectState {
                        start: pointer,
                        current: pointer,
                    });
                }
            }
        }

        // Apply drag movement (multi-widget)
        if let Some(drag) = &state.dragging {
            if response.dragged_by(egui::PointerButton::Primary) {
                if let Some(pointer) = ui.ctx().pointer_latest_pos() {
                    let dx = (pointer.x - drag.start_pos.x) / z;
                    let dy = (pointer.y - drag.start_pos.y) / z;
                    let snap_on = state.snap_enabled;
                    let grid = state.grid_size;

                    for (i, entity) in drag.entities.iter().enumerate() {
                        if let Some(&(ox, oy)) = drag.originals.get(i) {
                            let mut nx = ox + dx;
                            let mut ny = oy + dy;
                            if snap_on {
                                nx = snap(nx, grid);
                                ny = snap(ny, grid);
                            }
                            let e = *entity;
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(e) {
                                    if let Some(mut node) = em.get_mut::<Node>() {
                                        node.left = bevy::ui::Val::Percent(nx / ref_w * 100.0);
                                        node.top = bevy::ui::Val::Percent(ny / ref_h * 100.0);
                                        node.position_type = bevy::ui::PositionType::Absolute;
                                    }
                                }
                            });
                        }
                    }
                }
            }
        }

        // Apply resize (with snap)
        if let Some(resize) = &state.resizing {
            if response.dragged_by(egui::PointerButton::Primary) {
                if let Some(pointer) = ui.ctx().pointer_latest_pos() {
                    let dx = (pointer.x - resize.start_pos.x) / z;
                    let dy = (pointer.y - resize.start_pos.y) / z;
                    let snap_on = state.snap_enabled;
                    let grid = state.grid_size;

                    let (mut nw, mut nh, mut nx, mut ny) = match resize.handle {
                        ResizeHandle::BottomRight => (
                            (resize.original_w + dx).max(10.0),
                            (resize.original_h + dy).max(10.0),
                            resize.original_x,
                            resize.original_y,
                        ),
                        ResizeHandle::TopRight => (
                            (resize.original_w + dx).max(10.0),
                            (resize.original_h - dy).max(10.0),
                            resize.original_x,
                            resize.original_y + dy,
                        ),
                        ResizeHandle::BottomLeft => (
                            (resize.original_w - dx).max(10.0),
                            (resize.original_h + dy).max(10.0),
                            resize.original_x + dx,
                            resize.original_y,
                        ),
                        ResizeHandle::TopLeft => (
                            (resize.original_w - dx).max(10.0),
                            (resize.original_h - dy).max(10.0),
                            resize.original_x + dx,
                            resize.original_y + dy,
                        ),
                    };

                    if snap_on {
                        nw = snap(nw, grid).max(grid);
                        nh = snap(nh, grid).max(grid);
                        nx = snap(nx, grid);
                        ny = snap(ny, grid);
                    }

                    let entity = resize.entity;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut node) = em.get_mut::<Node>() {
                                node.width = bevy::ui::Val::Percent(nw / ref_w * 100.0);
                                node.height = bevy::ui::Val::Percent(nh / ref_h * 100.0);
                                node.left = bevy::ui::Val::Percent(nx / ref_w * 100.0);
                                node.top = bevy::ui::Val::Percent(ny / ref_h * 100.0);
                                node.position_type = bevy::ui::PositionType::Absolute;
                            }
                        }
                    });
                }
            }
        }

        // Update box-select
        if let Some(bs) = &mut state.box_select {
            if response.dragged_by(egui::PointerButton::Primary) {
                if let Some(pointer) = ui.ctx().pointer_latest_pos() {
                    bs.current = pointer;
                }
            }
        }

        // End drag/resize/box-select on release
        if !ui.ctx().input(|i| i.pointer.any_down()) {
            state.dragging = None;
            state.resizing = None;

            // Finalize box-select
            if let Some(bs) = state.box_select.take() {
                let sel_rect = Rect::from_two_pos(bs.start, bs.current);
                if sel_rect.area() > 16.0 {
                    // Select all widgets that intersect the box
                    state.multi_selection.clear();
                    for ws in &widget_snapshots {
                        let rect = ws_screen_rect(ws, canvas_rect, z);
                        if sel_rect.intersects(rect) {
                            state.multi_selection.push(ws.entity);
                        }
                    }
                    // Set primary selection to first
                    if let Some(&first) = state.multi_selection.first() {
                        commands.push(move |world: &mut World| {
                            if let Some(sel) = world.get_resource::<EditorSelection>() {
                                sel.set(Some(first));
                            }
                        });
                    }
                }
            }
        }

        // ── Empty state ──────────────────────────────────────────────────
        if state.canvases.is_empty() {
            painter.text(
                available.center(),
                egui::Align2::CENTER_CENTER,
                "Add a UI Canvas from the Widget palette to get started",
                egui::FontId::proportional(13.0),
                text_muted,
            );
        } else if widget_snapshots.is_empty() {
            painter.text(
                canvas_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Click widgets in the palette to add them here",
                egui::FontId::proportional(12.0),
                text_muted,
            );
        }
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Center
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Check if `entity` is a descendant of `ancestor` by walking up the parent chain.
fn is_descendant_of(world: &World, entity: Entity, ancestor: Entity) -> bool {
    let mut current = entity;
    for _ in 0..32 {
        if current == ancestor {
            return true;
        }
        match world.get::<ChildOf>(current) {
            Some(child_of) => current = child_of.parent(),
            None => return false,
        }
    }
    false
}

// ── Alignment ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum AlignAction {
    Left,
    CenterH,
    Right,
    Top,
    CenterV,
    Bottom,
}

fn compute_align(widgets: &[WidgetSnapshot], action: AlignAction) -> Vec<(Entity, f32, f32)> {
    if widgets.is_empty() {
        return vec![];
    }
    match action {
        AlignAction::Left => {
            let min_x = widgets.iter().map(|w| w.x).fold(f32::MAX, f32::min);
            widgets.iter().map(|w| (w.entity, min_x, w.y)).collect()
        }
        AlignAction::Right => {
            let max_right = widgets
                .iter()
                .map(|w| w.x + w.width)
                .fold(f32::MIN, f32::max);
            widgets
                .iter()
                .map(|w| (w.entity, max_right - w.width, w.y))
                .collect()
        }
        AlignAction::CenterH => {
            let min_x = widgets.iter().map(|w| w.x).fold(f32::MAX, f32::min);
            let max_right = widgets
                .iter()
                .map(|w| w.x + w.width)
                .fold(f32::MIN, f32::max);
            let center = (min_x + max_right) / 2.0;
            widgets
                .iter()
                .map(|w| (w.entity, center - w.width / 2.0, w.y))
                .collect()
        }
        AlignAction::Top => {
            let min_y = widgets.iter().map(|w| w.y).fold(f32::MAX, f32::min);
            widgets.iter().map(|w| (w.entity, w.x, min_y)).collect()
        }
        AlignAction::Bottom => {
            let max_bottom = widgets
                .iter()
                .map(|w| w.y + w.height)
                .fold(f32::MIN, f32::max);
            widgets
                .iter()
                .map(|w| (w.entity, w.x, max_bottom - w.height))
                .collect()
        }
        AlignAction::CenterV => {
            let min_y = widgets.iter().map(|w| w.y).fold(f32::MAX, f32::min);
            let max_bottom = widgets
                .iter()
                .map(|w| w.y + w.height)
                .fold(f32::MIN, f32::max);
            let center = (min_y + max_bottom) / 2.0;
            widgets
                .iter()
                .map(|w| (w.entity, w.x, center - w.height / 2.0))
                .collect()
        }
    }
}

fn compute_distribute_h(widgets: &[WidgetSnapshot]) -> Vec<(Entity, f32)> {
    if widgets.len() < 3 {
        return vec![];
    }
    let mut sorted: Vec<_> = widgets.to_vec();
    sorted.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
    let first_x = sorted.first().unwrap().x;
    let last_x = sorted.last().unwrap().x;
    let step = (last_x - first_x) / (sorted.len() - 1) as f32;
    sorted
        .iter()
        .enumerate()
        .map(|(i, w)| (w.entity, first_x + step * i as f32))
        .collect()
}

fn compute_distribute_v(widgets: &[WidgetSnapshot]) -> Vec<(Entity, f32)> {
    if widgets.len() < 3 {
        return vec![];
    }
    let mut sorted: Vec<_> = widgets.to_vec();
    sorted.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap());
    let first_y = sorted.first().unwrap().y;
    let last_y = sorted.last().unwrap().y;
    let step = (last_y - first_y) / (sorted.len() - 1) as f32;
    sorted
        .iter()
        .enumerate()
        .map(|(i, w)| (w.entity, first_y + step * i as f32))
        .collect()
}
