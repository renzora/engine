//! Pixel Canvas Panel - Main drawing area with integrated vertical toolbar
//!
//! Left toolbar strip with tool icons, zoom/pan canvas, checkerboard background,
//! pixel grid, tool interaction, cursor preview, new project dialog, and info bar.

use bevy_egui::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use crate::pixel_editor::{PixelEditorState, PixelTool, BrushShape};
use crate::theming::Theme;

use egui_phosphor::regular::{
    PENCIL_SIMPLE, ERASER, PAINT_BUCKET, LINE_SEGMENT, RECTANGLE, CIRCLE,
    SELECTION, EYEDROPPER, ARROWS_OUT_CARDINAL,
};

const TOOLBAR_W: f32 = 34.0;
const TOOL_BTN: f32 = 26.0;
const TOOL_GAP: f32 = 2.0;
const INFO_H: f32 = 20.0;

/// Render the pixel canvas panel with integrated toolbar
pub fn render_pixel_canvas_content(
    ui: &mut egui::Ui,
    state: &mut PixelEditorState,
    theme: &Theme,
) {
    if state.new_project_dialog {
        render_new_project_dialog(ui, state, theme);
        return;
    }
    if state.project.is_none() {
        render_no_project(ui, state, theme);
        return;
    }

    let available = ui.available_rect_before_wrap();

    // Layout regions
    let toolbar_rect = Rect::from_min_max(
        available.min,
        Pos2::new(available.min.x + TOOLBAR_W, available.max.y),
    );
    let canvas_rect = Rect::from_min_max(
        Pos2::new(available.min.x + TOOLBAR_W, available.min.y),
        Pos2::new(available.max.x, available.max.y - INFO_H),
    );
    let info_rect = Rect::from_min_max(
        Pos2::new(available.min.x, available.max.y - INFO_H),
        available.max,
    );

    // Keyboard shortcuts
    handle_keyboard_shortcuts(ui, state);

    // Render toolbar
    render_toolbar(ui, state, theme, toolbar_rect);

    // Extract state values before borrowing project
    let zoom = state.zoom;
    let grid_visible = state.grid_visible;
    let brush_size = state.brush_size;
    let brush_shape = state.brush_shape;
    let tool = state.tool;
    let is_drawing = state.is_drawing;
    let shape_start = state.shape_start;
    let draw_color = state.draw_color();
    let pan_offset = state.pan_offset;

    let project = state.project.as_mut().unwrap();

    // Canvas interaction area
    let response = ui.allocate_rect(canvas_rect, Sense::click_and_drag());

    // Checkerboard background
    draw_checkerboard(ui, canvas_rect);

    // Canvas positioning
    let canvas_w = project.width as f32 * zoom;
    let canvas_h = project.height as f32 * zoom;
    let canvas_origin = Pos2::new(
        canvas_rect.center().x - canvas_w / 2.0 + pan_offset[0],
        canvas_rect.center().y - canvas_h / 2.0 + pan_offset[1],
    );
    let pixel_area = Rect::from_min_size(canvas_origin, Vec2::new(canvas_w, canvas_h));

    // Flatten and render texture
    if project.texture_dirty || state.canvas_texture.is_none() {
        let data = project.flatten_layers(project.active_frame);
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [project.width as usize, project.height as usize],
            &data,
        );
        state.canvas_texture = Some(ui.ctx().load_texture(
            "pixel_canvas", image, egui::TextureOptions::NEAREST,
        ));
        project.texture_dirty = false;
    }

    if let Some(tex) = &state.canvas_texture {
        ui.painter().image(
            tex.id(), pixel_area,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    // Pixel grid at high zoom
    if grid_visible && zoom >= 4.0 {
        draw_pixel_grid(ui, canvas_rect, canvas_origin, project.width, project.height, zoom);
    }

    // Canvas border
    ui.painter().rect_stroke(
        pixel_area, 0.0,
        Stroke::new(1.0, Color32::from_gray(55)),
        egui::StrokeKind::Outside,
    );

    // Mouse -> pixel coordinates
    let mouse_pos = response.hover_pos().or_else(|| ui.ctx().pointer_hover_pos());
    let pixel_pos = mouse_pos.map(|pos| {
        let px = ((pos.x - canvas_origin.x) / zoom).floor() as i32;
        let py = ((pos.y - canvas_origin.y) / zoom).floor() as i32;
        (px, py)
    });
    let pointer_in_canvas = mouse_pos.map_or(false, |p| canvas_rect.contains(p));

    // Zoom with scroll wheel
    if response.hovered() {
        let scroll = ui.ctx().input(|i| i.raw_scroll_delta.y);
        if scroll != 0.0 {
            let old_zoom = state.zoom;
            state.zoom = if scroll > 0.0 {
                (state.zoom * 1.2).min(128.0)
            } else {
                (state.zoom / 1.2).max(0.5)
            };
            if let Some(pos) = mouse_pos {
                let r = state.zoom / old_zoom;
                state.pan_offset[0] = (state.pan_offset[0] + pos.x - canvas_rect.center().x) * r
                    - (pos.x - canvas_rect.center().x);
                state.pan_offset[1] = (state.pan_offset[1] + pos.y - canvas_rect.center().y) * r
                    - (pos.y - canvas_rect.center().y);
            }
            project.texture_dirty = true;
        }
    }

    // Middle-mouse pan
    if response.dragged_by(egui::PointerButton::Middle) {
        state.pan_offset[0] += response.drag_delta().x;
        state.pan_offset[1] += response.drag_delta().y;
    }

    let proj_w = project.width;
    let proj_h = project.height;
    let mut should_push_recent = false;

    // Use pointer state directly for reliable single-click detection
    let primary_pressed = ui.ctx().input(|i| i.pointer.primary_pressed());
    let primary_down = ui.ctx().input(|i| i.pointer.primary_down());
    let primary_released = ui.ctx().input(|i| i.pointer.primary_released());

    // Brush cursor preview
    if let Some((px, py)) = pixel_pos {
        if response.hovered() && px >= 0 && py >= 0
            && px < proj_w as i32 && py < proj_h as i32
        {
            let half = brush_size as f32 / 2.0;
            let cursor_rect = Rect::from_min_size(
                Pos2::new(
                    canvas_origin.x + (px as f32 - half + 0.5) * zoom,
                    canvas_origin.y + (py as f32 - half + 0.5) * zoom,
                ),
                Vec2::splat(brush_size as f32 * zoom),
            );
            let rounding = if brush_shape == BrushShape::Circle {
                cursor_rect.width() / 2.0
            } else {
                0.0
            };
            ui.painter().rect_stroke(
                cursor_rect, rounding,
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 150)),
                egui::StrokeKind::Outside,
            );
        }
    }

    // Tool interaction
    if let Some((px, py)) = pixel_pos {
        if pointer_in_canvas {
            match tool {
                PixelTool::Pencil | PixelTool::Eraser => {
                    if primary_pressed {
                        project.push_undo("Draw");
                        state.is_drawing = true;
                        if px >= 0 && py >= 0 {
                            project.draw_brush(px, py, draw_color, brush_size, brush_shape);
                        }
                    } else if (is_drawing || state.is_drawing) && primary_down {
                        if px >= 0 && py >= 0 {
                            project.draw_brush(px, py, draw_color, brush_size, brush_shape);
                        }
                    }
                }
                PixelTool::Fill => {
                    if primary_pressed && px >= 0 && py >= 0
                        && (px as u32) < proj_w && (py as u32) < proj_h
                    {
                        project.push_undo("Fill");
                        project.flood_fill(px as u32, py as u32, draw_color);
                        should_push_recent = true;
                    }
                }
                PixelTool::Line => {
                    if primary_pressed {
                        project.push_undo("Line");
                        state.shape_start = Some((px, py));
                    }
                    if primary_released {
                        if let Some((sx, sy)) = shape_start {
                            project.draw_line(sx, sy, px, py, draw_color, brush_size, brush_shape);
                            should_push_recent = true;
                        }
                    }
                }
                PixelTool::Rect => {
                    if primary_pressed {
                        project.push_undo("Rect");
                        state.shape_start = Some((px, py));
                    }
                    if primary_released {
                        if let Some((sx, sy)) = shape_start {
                            project.draw_rect(sx, sy, px, py, draw_color, brush_size, brush_shape);
                            should_push_recent = true;
                        }
                    }
                }
                PixelTool::Circle => {
                    if primary_pressed {
                        project.push_undo("Circle");
                        state.shape_start = Some((px, py));
                    }
                    if primary_released {
                        if let Some((sx, sy)) = shape_start {
                            let dx = (px - sx) as f32;
                            let dy = (py - sy) as f32;
                            let radius = (dx * dx + dy * dy).sqrt() as i32;
                            project.draw_circle(sx, sy, radius, draw_color, brush_size, brush_shape);
                            should_push_recent = true;
                        }
                    }
                }
                PixelTool::Eyedropper => {
                    if primary_pressed && px >= 0 && py >= 0
                        && (px as u32) < proj_w && (py as u32) < proj_h
                    {
                        let al = project.active_layer;
                        let af = project.active_frame;
                        state.primary_color = project.get_pixel(al, af, px as u32, py as u32);
                    }
                }
                PixelTool::Select | PixelTool::Move => {}
            }
        }
    }

    // Undo/redo
    let do_undo = ui.ctx().input(|i| {
        i.modifiers.ctrl && i.key_pressed(egui::Key::Z) && !i.modifiers.shift
    });
    let do_redo = ui.ctx().input(|i| {
        i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(egui::Key::Z)
    });
    if do_undo { project.undo(); }
    if do_redo { project.redo(); }

    let _ = project;

    // Shape tool live pixel preview while dragging
    if primary_down && !primary_pressed {
        if let Some((sx, sy)) = state.shape_start {
            if let Some((px, py)) = pixel_pos {
                let preview_color = if draw_color[3] < 10 {
                    Color32::from_rgba_unmultiplied(255, 100, 100, 120)
                } else {
                    Color32::from_rgba_unmultiplied(
                        draw_color[0], draw_color[1], draw_color[2],
                        ((draw_color[3] as f32) * 0.7) as u8,
                    )
                };

                match tool {
                    PixelTool::Line => {
                        preview_line(ui, sx, sy, px, py, brush_size, brush_shape,
                            canvas_origin, zoom, preview_color);
                    }
                    PixelTool::Rect => {
                        preview_line(ui, sx, sy, px, sy, brush_size, brush_shape,
                            canvas_origin, zoom, preview_color);
                        preview_line(ui, px, sy, px, py, brush_size, brush_shape,
                            canvas_origin, zoom, preview_color);
                        preview_line(ui, px, py, sx, py, brush_size, brush_shape,
                            canvas_origin, zoom, preview_color);
                        preview_line(ui, sx, py, sx, sy, brush_size, brush_shape,
                            canvas_origin, zoom, preview_color);
                    }
                    PixelTool::Circle => {
                        let dx = (px - sx) as f32;
                        let dy = (py - sy) as f32;
                        let radius = (dx * dx + dy * dy).sqrt() as i32;
                        preview_circle(ui, sx, sy, radius, brush_size, brush_shape,
                            canvas_origin, zoom, preview_color);
                    }
                    _ => {}
                }
            }
        }
    }

    // Global cleanup on mouse release
    if primary_released {
        if state.is_drawing {
            should_push_recent = true;
        }
        state.is_drawing = false;
        state.shape_start = None;
    }

    if should_push_recent {
        state.push_recent_color();
    }

    // Info bar
    render_info_bar(ui, theme, info_rect, pixel_pos, proj_w, proj_h, state.zoom);
}

/// Vertical toolbar with tool icons and color swatches
fn render_toolbar(ui: &mut egui::Ui, state: &mut PixelEditorState, theme: &Theme, rect: Rect) {
    let accent = theme.semantic.accent.to_color32();
    let text_color = theme.text.primary.to_color32();
    let muted = theme.text.muted.to_color32();

    // Background
    ui.painter().rect_filled(rect, 0.0, Color32::from_gray(28));
    ui.painter().line_segment(
        [rect.right_top(), rect.right_bottom()],
        Stroke::new(1.0, Color32::from_gray(45)),
    );

    let tools: &[(PixelTool, &str)] = &[
        (PixelTool::Pencil, PENCIL_SIMPLE),
        (PixelTool::Eraser, ERASER),
        (PixelTool::Fill, PAINT_BUCKET),
        (PixelTool::Line, LINE_SEGMENT),
        (PixelTool::Rect, RECTANGLE),
        (PixelTool::Circle, CIRCLE),
        (PixelTool::Select, SELECTION),
        (PixelTool::Eyedropper, EYEDROPPER),
        (PixelTool::Move, ARROWS_OUT_CARDINAL),
    ];

    let pad_x = (TOOLBAR_W - TOOL_BTN) / 2.0;
    let mut y = rect.min.y + 6.0;

    for (idx, (tool, icon)) in tools.iter().enumerate() {
        // Group separators
        if idx == 3 || idx == 6 {
            y += 2.0;
            ui.painter().line_segment(
                [Pos2::new(rect.min.x + 6.0, y), Pos2::new(rect.max.x - 6.0, y)],
                Stroke::new(1.0, Color32::from_gray(45)),
            );
            y += 4.0;
        }

        let btn_rect = Rect::from_min_size(
            Pos2::new(rect.min.x + pad_x, y),
            Vec2::splat(TOOL_BTN),
        );
        let resp = ui.allocate_rect(btn_rect, Sense::click());
        let is_active = state.tool == *tool;

        let bg = if is_active {
            accent.gamma_multiply(0.35)
        } else if resp.hovered() {
            Color32::from_gray(48)
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(btn_rect, 4.0, bg);

        if is_active {
            ui.painter().rect_stroke(
                btn_rect, 4.0,
                Stroke::new(1.0, accent.gamma_multiply(0.8)),
                egui::StrokeKind::Outside,
            );
        }

        ui.painter().text(
            btn_rect.center(),
            egui::Align2::CENTER_CENTER,
            *icon,
            egui::FontId::proportional(14.0),
            if is_active { text_color } else { muted },
        );

        if resp.clicked() {
            state.tool = *tool;
        }
        resp.on_hover_text(format!("{} ({})", tool.name(), tool.shortcut()));

        y += TOOL_BTN + TOOL_GAP;
    }

    // Color swatches
    y += 6.0;
    ui.painter().line_segment(
        [Pos2::new(rect.min.x + 6.0, y), Pos2::new(rect.max.x - 6.0, y)],
        Stroke::new(1.0, Color32::from_gray(45)),
    );
    y += 8.0;

    let swatch = 16.0;
    let fg_pos = Pos2::new(rect.min.x + pad_x, y);
    let bg_pos = Pos2::new(rect.min.x + pad_x + 8.0, y + 8.0);

    // Secondary color (behind)
    let bg_rect = Rect::from_min_size(bg_pos, Vec2::splat(swatch));
    let sec_c = Color32::from_rgba_unmultiplied(
        state.secondary_color[0], state.secondary_color[1],
        state.secondary_color[2], state.secondary_color[3],
    );
    ui.painter().rect_filled(bg_rect, 2.0, sec_c);
    ui.painter().rect_stroke(
        bg_rect, 2.0,
        Stroke::new(1.0, Color32::from_gray(80)),
        egui::StrokeKind::Outside,
    );

    // Primary color (on top)
    let fg_rect = Rect::from_min_size(fg_pos, Vec2::splat(swatch));
    let pri_c = Color32::from_rgba_unmultiplied(
        state.primary_color[0], state.primary_color[1],
        state.primary_color[2], state.primary_color[3],
    );
    ui.painter().rect_filled(fg_rect, 2.0, pri_c);
    ui.painter().rect_stroke(
        fg_rect, 2.0,
        Stroke::new(1.5, Color32::WHITE),
        egui::StrokeKind::Outside,
    );

    // Swap button
    let swap_rect = Rect::from_min_size(
        Pos2::new(rect.min.x + pad_x + swatch + 2.0, y - 2.0),
        Vec2::new(12.0, 12.0),
    );
    let swap_resp = ui.allocate_rect(swap_rect, Sense::click());
    ui.painter().text(
        swap_rect.center(),
        egui::Align2::CENTER_CENTER,
        "\u{21C4}",
        egui::FontId::proportional(9.0),
        if swap_resp.hovered() { text_color } else { muted },
    );
    if swap_resp.clicked() {
        state.swap_colors();
    }
    swap_resp.on_hover_text("Swap colors (X)");
}

fn handle_keyboard_shortcuts(ui: &mut egui::Ui, state: &mut PixelEditorState) {
    ui.ctx().input(|i| {
        if !i.modifiers.ctrl && !i.modifiers.alt {
            if i.key_pressed(egui::Key::P) { state.tool = PixelTool::Pencil; }
            if i.key_pressed(egui::Key::E) { state.tool = PixelTool::Eraser; }
            if i.key_pressed(egui::Key::G) { state.tool = PixelTool::Fill; }
            if i.key_pressed(egui::Key::L) { state.tool = PixelTool::Line; }
            if i.key_pressed(egui::Key::U) { state.tool = PixelTool::Rect; }
            if i.key_pressed(egui::Key::O) { state.tool = PixelTool::Circle; }
            if i.key_pressed(egui::Key::S) && !i.modifiers.ctrl {
                state.tool = PixelTool::Select;
            }
            if i.key_pressed(egui::Key::I) { state.tool = PixelTool::Eyedropper; }
            if i.key_pressed(egui::Key::M) { state.tool = PixelTool::Move; }
            if i.key_pressed(egui::Key::X) { state.swap_colors(); }
        }
    });
}

fn render_info_bar(
    ui: &mut egui::Ui,
    theme: &Theme,
    rect: Rect,
    pixel_pos: Option<(i32, i32)>,
    width: u32,
    height: u32,
    zoom: f32,
) {
    let text_color = theme.text.muted.to_color32();
    ui.painter().rect_filled(rect, 0.0, Color32::from_gray(25));
    ui.painter().line_segment(
        [rect.left_top(), rect.right_top()],
        Stroke::new(1.0, Color32::from_gray(45)),
    );

    let zoom_pct = zoom * 100.0 / 8.0;
    let text = if let Some((px, py)) = pixel_pos {
        format!("  {}x{}   {:.0}%   ({}, {})", width, height, zoom_pct, px, py)
    } else {
        format!("  {}x{}   {:.0}%", width, height, zoom_pct)
    };
    ui.painter().text(
        Pos2::new(rect.min.x + 4.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        egui::FontId::monospace(10.0),
        text_color,
    );
}

fn render_no_project(ui: &mut egui::Ui, state: &mut PixelEditorState, theme: &Theme) {
    let available = ui.available_rect_before_wrap();
    let _ = ui.allocate_rect(available, Sense::hover());
    ui.painter().rect_filled(available, 0.0, Color32::from_gray(30));

    let muted = theme.text.muted.to_color32();
    let accent = theme.semantic.accent.to_color32();

    ui.painter().text(
        Pos2::new(available.center().x, available.center().y - 30.0),
        egui::Align2::CENTER_CENTER,
        "Pixel Art Editor",
        egui::FontId::proportional(18.0),
        muted,
    );
    ui.painter().text(
        Pos2::new(available.center().x, available.center().y - 8.0),
        egui::Align2::CENTER_CENTER,
        "Create a new project to get started",
        egui::FontId::proportional(12.0),
        Color32::from_gray(80),
    );

    let btn_rect = Rect::from_center_size(
        Pos2::new(available.center().x, available.center().y + 24.0),
        Vec2::new(130.0, 30.0),
    );
    let btn = ui.allocate_rect(btn_rect, Sense::click());
    let bg = if btn.hovered() { accent } else { accent.gamma_multiply(0.6) };
    ui.painter().rect_filled(btn_rect, 6.0, bg);
    ui.painter().text(
        btn_rect.center(),
        egui::Align2::CENTER_CENTER,
        "New Project",
        egui::FontId::proportional(13.0),
        Color32::WHITE,
    );
    if btn.clicked() {
        state.new_project_dialog = true;
    }
}

fn render_new_project_dialog(ui: &mut egui::Ui, state: &mut PixelEditorState, theme: &Theme) {
    let available = ui.available_rect_before_wrap();
    let _ = ui.allocate_rect(available, Sense::hover());
    ui.painter().rect_filled(
        available, 0.0,
        Color32::from_rgba_unmultiplied(20, 20, 22, 240),
    );

    let dialog_size = Vec2::new(300.0, 260.0);
    let dialog_rect = Rect::from_center_size(available.center(), dialog_size);

    ui.painter().rect_filled(dialog_rect, 8.0, Color32::from_gray(35));
    ui.painter().rect_stroke(
        dialog_rect, 8.0,
        Stroke::new(1.0, Color32::from_gray(55)),
        egui::StrokeKind::Outside,
    );

    let accent = theme.semantic.accent.to_color32();
    let muted = theme.text.muted.to_color32();

    let mut child = ui.new_child(
        egui::UiBuilder::new().max_rect(dialog_rect.shrink(20.0)),
    );

    child.vertical(|ui| {
        ui.label(egui::RichText::new("New Project").size(16.0).color(Color32::WHITE));
        ui.add_space(12.0);

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Width:").size(12.0).color(muted));
            let mut w = state.new_project_width as i32;
            if ui.add(egui::DragValue::new(&mut w).range(1..=4096).speed(1)).changed() {
                state.new_project_width = w.max(1) as u32;
            }
        });
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Height:").size(12.0).color(muted));
            let mut h = state.new_project_height as i32;
            if ui.add(egui::DragValue::new(&mut h).range(1..=4096).speed(1)).changed() {
                state.new_project_height = h.max(1) as u32;
            }
        });
        ui.add_space(8.0);

        ui.label(egui::RichText::new("Presets:").size(11.0).color(muted));
        ui.horizontal_wrapped(|ui| {
            for size in &[16u32, 32, 64, 128, 256] {
                if ui.small_button(&format!("{}x{}", size, size)).clicked() {
                    state.new_project_width = *size;
                    state.new_project_height = *size;
                }
            }
        });
        ui.add_space(16.0);

        let w = state.new_project_width;
        let h = state.new_project_height;
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(
                egui::RichText::new("Create").color(Color32::WHITE)
            ).fill(accent).rounding(4.0)).clicked() {
                state.new_project(w, h);
                state.new_project_dialog = false;
            }
            ui.add_space(8.0);
            if ui.button("Cancel").clicked() {
                state.new_project_dialog = false;
            }
        });
    });
}

fn draw_checkerboard(ui: &mut egui::Ui, rect: Rect) {
    let check: f32 = 12.0;
    let c1 = Color32::from_gray(35);
    let c2 = Color32::from_gray(42);

    let mut y = rect.min.y;
    let mut row = 0;
    while y < rect.max.y {
        let mut x = rect.min.x;
        let mut col = 0;
        while x < rect.max.x {
            let color = if (row + col) % 2 == 0 { c1 } else { c2 };
            ui.painter().rect_filled(
                Rect::from_min_size(
                    Pos2::new(x, y),
                    Vec2::new(check.min(rect.max.x - x), check.min(rect.max.y - y)),
                ),
                0.0, color,
            );
            x += check;
            col += 1;
        }
        y += check;
        row += 1;
    }
}

fn draw_pixel_grid(
    ui: &mut egui::Ui,
    clip: Rect,
    origin: Pos2,
    w: u32, h: u32,
    zoom: f32,
) {
    let color = Color32::from_rgba_unmultiplied(255, 255, 255, 15);
    let painter = ui.painter();
    for x in 0..=w {
        let px = origin.x + x as f32 * zoom;
        if px >= clip.min.x && px <= clip.max.x {
            painter.line_segment(
                [
                    Pos2::new(px, origin.y.max(clip.min.y)),
                    Pos2::new(px, (origin.y + h as f32 * zoom).min(clip.max.y)),
                ],
                Stroke::new(0.5, color),
            );
        }
    }
    for y in 0..=h {
        let py = origin.y + y as f32 * zoom;
        if py >= clip.min.y && py <= clip.max.y {
            painter.line_segment(
                [
                    Pos2::new(origin.x.max(clip.min.x), py),
                    Pos2::new((origin.x + w as f32 * zoom).min(clip.max.x), py),
                ],
                Stroke::new(0.5, color),
            );
        }
    }
}

// --- Pixel-accurate shape preview helpers ---

/// Paint a single brush stamp as preview pixels
fn preview_stamp(
    painter: &egui::Painter,
    cx: i32, cy: i32,
    brush_size: u32, brush_shape: BrushShape,
    origin: Pos2, zoom: f32, color: Color32,
) {
    let half = brush_size as i32 / 2;
    let radius_sq = (brush_size as f32 / 2.0).powi(2);
    for dy in -half..=(half.max(0)) {
        for dx in -half..=(half.max(0)) {
            if brush_shape == BrushShape::Circle {
                let dist_sq = (dx as f32 + 0.5).powi(2) + (dy as f32 + 0.5).powi(2);
                if dist_sq > radius_sq { continue; }
            }
            let px = cx + dx;
            let py = cy + dy;
            painter.rect_filled(
                Rect::from_min_size(
                    Pos2::new(origin.x + px as f32 * zoom, origin.y + py as f32 * zoom),
                    Vec2::splat(zoom),
                ),
                0.0, color,
            );
        }
    }
}

/// Preview a Bresenham line with brush stamps (matches PixelProject::draw_line exactly)
fn preview_line(
    ui: &egui::Ui,
    x0: i32, y0: i32, x1: i32, y1: i32,
    brush_size: u32, brush_shape: BrushShape,
    origin: Pos2, zoom: f32, color: Color32,
) {
    let painter = ui.painter();
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut cx = x0;
    let mut cy = y0;

    loop {
        preview_stamp(painter, cx, cy, brush_size, brush_shape, origin, zoom, color);
        if cx == x1 && cy == y1 { break; }
        let e2 = 2 * err;
        if e2 >= dy { err += dy; cx += sx; }
        if e2 <= dx { err += dx; cy += sy; }
    }
}

/// Preview a midpoint circle with brush stamps (matches PixelProject::draw_circle exactly)
fn preview_circle(
    ui: &egui::Ui,
    cx: i32, cy: i32, radius: i32,
    brush_size: u32, brush_shape: BrushShape,
    origin: Pos2, zoom: f32, color: Color32,
) {
    let painter = ui.painter();
    let mut x = radius;
    let mut y = 0i32;
    let mut err = 1 - radius;

    while x >= y {
        preview_stamp(painter, cx + x, cy + y, brush_size, brush_shape, origin, zoom, color);
        preview_stamp(painter, cx - x, cy + y, brush_size, brush_shape, origin, zoom, color);
        preview_stamp(painter, cx + x, cy - y, brush_size, brush_shape, origin, zoom, color);
        preview_stamp(painter, cx - x, cy - y, brush_size, brush_shape, origin, zoom, color);
        preview_stamp(painter, cx + y, cy + x, brush_size, brush_shape, origin, zoom, color);
        preview_stamp(painter, cx - y, cy + x, brush_size, brush_shape, origin, zoom, color);
        preview_stamp(painter, cx + y, cy - x, brush_size, brush_shape, origin, zoom, color);
        preview_stamp(painter, cx - y, cy - x, brush_size, brush_shape, origin, zoom, color);
        y += 1;
        if err < 0 {
            err += 2 * y + 1;
        } else {
            x -= 1;
            err += 2 * (y - x) + 1;
        }
    }
}
