//! Shape Library panel — visual grid of mesh primitives for quick spawning

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CursorIcon, Vec2};

use crate::component_system::MeshPrimitiveType;
use crate::theming::Theme;

use egui_phosphor::regular::{
    CUBE, SPHERE, CYLINDER, SQUARE, TRIANGLE, CIRCLE, DIAMOND, POLYGON,
    STAIRS, WALL, PIPE, HEXAGON, MAGNIFYING_GLASS, DOOR, FRAME_CORNERS,
    PLUS, SPIRAL, COLUMNS,
};

/// State resource for the Shape Library panel
#[derive(Resource, Default)]
pub struct ShapeLibraryState {
    /// Shape currently being dragged from the panel
    pub dragging_shape: Option<MeshPrimitiveType>,
    /// Shape clicked in the panel — to be spawned at origin by a system
    pub pending_spawn: Option<MeshPrimitiveType>,
    /// Search filter text
    pub search_filter: String,
}

/// Get the icon for a mesh primitive type
fn shape_icon(mesh_type: MeshPrimitiveType) -> &'static str {
    match mesh_type {
        MeshPrimitiveType::Cube => CUBE,
        MeshPrimitiveType::Sphere => SPHERE,
        MeshPrimitiveType::Cylinder => CYLINDER,
        MeshPrimitiveType::Plane => SQUARE,
        MeshPrimitiveType::Cone => TRIANGLE,
        MeshPrimitiveType::Torus => CIRCLE,
        MeshPrimitiveType::Capsule => CYLINDER,
        MeshPrimitiveType::Wedge => TRIANGLE,
        MeshPrimitiveType::Stairs => STAIRS,
        MeshPrimitiveType::Arch => CIRCLE,
        MeshPrimitiveType::HalfCylinder => CYLINDER,
        MeshPrimitiveType::QuarterPipe => POLYGON,
        MeshPrimitiveType::Corner => POLYGON,
        MeshPrimitiveType::Prism => HEXAGON,
        MeshPrimitiveType::Pyramid => DIAMOND,
        MeshPrimitiveType::Pipe => PIPE,
        MeshPrimitiveType::Ring => CIRCLE,
        MeshPrimitiveType::Wall => WALL,
        MeshPrimitiveType::Ramp => TRIANGLE,
        MeshPrimitiveType::Hemisphere => SPHERE,
        MeshPrimitiveType::CurvedWall => WALL,
        MeshPrimitiveType::Doorway => DOOR,
        MeshPrimitiveType::WindowWall => FRAME_CORNERS,
        MeshPrimitiveType::LShape => POLYGON,
        MeshPrimitiveType::TShape => POLYGON,
        MeshPrimitiveType::CrossShape => PLUS,
        MeshPrimitiveType::Funnel => TRIANGLE,
        MeshPrimitiveType::Gutter => CYLINDER,
        MeshPrimitiveType::SpiralStairs => SPIRAL,
        MeshPrimitiveType::Pillar => COLUMNS,
    }
}

/// Render the Shape Library panel content
pub fn render_shape_library_content(
    ui: &mut egui::Ui,
    state: &mut ShapeLibraryState,
    theme: &Theme,
) {
    let accent = theme.semantic.accent.to_color32();
    let text_primary = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();

    let panel_padding = 6.0;
    let spacing = 4.0;

    ui.add_space(panel_padding);

    // Search bar (magnifying glass icon, matching hierarchy/assets pattern)
    ui.horizontal(|ui| {
        ui.add_space(panel_padding);
        let available = ui.available_width() - panel_padding;
        ui.add_sized(
            [available, 20.0],
            egui::TextEdit::singleline(&mut state.search_filter)
                .hint_text(format!("{} Search...", MAGNIFYING_GLASS))
        );
    });

    ui.add_space(spacing);

    // Filter shapes by search
    let search_lower = state.search_filter.to_lowercase();
    let shapes: Vec<MeshPrimitiveType> = MeshPrimitiveType::all()
        .iter()
        .filter(|t| {
            search_lower.is_empty()
                || t.display_name().to_lowercase().contains(&search_lower)
        })
        .copied()
        .collect();

    // Responsive shape grid
    egui::ScrollArea::vertical().show(ui, |ui| {
        let available_width = ui.available_width() - panel_padding * 2.0;
        let min_tile = 56.0;
        let max_tile = 80.0;
        let cols = ((available_width + spacing) / (min_tile + spacing)).floor().max(1.0) as usize;
        let tile_size = ((available_width - spacing * (cols as f32 - 1.0)) / cols as f32)
            .clamp(min_tile, max_tile);
        let label_height = 16.0;

        ui.add_space(2.0);

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = spacing;

            let rows = (shapes.len() + cols - 1) / cols;
            for row in 0..rows {
                ui.horizontal(|ui| {
                    ui.add_space(panel_padding);
                    ui.spacing_mut().item_spacing.x = spacing;
                    for col in 0..cols {
                        let idx = row * cols + col;
                        if idx < shapes.len() {
                            let mesh_type = shapes[idx];
                            let icon = shape_icon(mesh_type);
                            let name = mesh_type.display_name();

                            let total_height = tile_size + label_height;
                            let (rect, response) = ui.allocate_exact_size(
                                Vec2::new(tile_size, total_height),
                                egui::Sense::drag(),
                            );

                            let hovered = response.hovered();
                            let dragging = response.dragged();
                            let bg = if dragging {
                                accent.linear_multiply(0.3)
                            } else if hovered {
                                theme.widgets.hovered_bg.to_color32()
                            } else {
                                theme.surfaces.faint.to_color32()
                            };

                            // Tile background
                            ui.painter().rect_filled(rect, 6.0, bg);

                            // Border on hover/drag
                            if hovered || dragging {
                                let border_color = if dragging {
                                    accent
                                } else {
                                    accent.linear_multiply(0.6)
                                };
                                ui.painter().rect_stroke(
                                    rect,
                                    6.0,
                                    egui::Stroke::new(1.0, border_color),
                                    egui::StrokeKind::Outside,
                                );
                                if !dragging {
                                    ui.ctx().set_cursor_icon(CursorIcon::Grab);
                                }
                            }

                            // Icon
                            let icon_rect = egui::Rect::from_min_size(
                                rect.min,
                                Vec2::new(tile_size, tile_size),
                            );
                            ui.painter().text(
                                icon_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                icon,
                                egui::FontId::proportional(28.0),
                                if hovered || dragging { accent } else { text_primary },
                            );

                            // Label
                            let label_pos = egui::pos2(
                                rect.center().x,
                                rect.max.y - label_height / 2.0,
                            );
                            ui.painter().text(
                                label_pos,
                                egui::Align2::CENTER_CENTER,
                                name,
                                egui::FontId::proportional(9.5),
                                text_muted,
                            );

                            // Tooltip
                            response.clone().on_hover_text(format!("Drag to place {name}"));

                            // Drag to viewport
                            if response.drag_started() {
                                state.dragging_shape = Some(mesh_type);
                            }

                            // Show floating drag indicator
                            if dragging && state.dragging_shape.is_some() {
                                if let Some(pointer_pos) = ui.ctx().pointer_hover_pos() {
                                    let tooltip_rect = egui::Rect::from_center_size(
                                        egui::pos2(pointer_pos.x + 20.0, pointer_pos.y - 10.0),
                                        Vec2::new(80.0, 22.0),
                                    );
                                    ui.painter().rect_filled(
                                        tooltip_rect,
                                        4.0,
                                        Color32::from_rgba_premultiplied(30, 30, 35, 220),
                                    );
                                    ui.painter().text(
                                        tooltip_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        name,
                                        egui::FontId::proportional(11.0),
                                        accent,
                                    );
                                }
                            }
                        }
                    }
                });
            }
        });

        ui.add_space(panel_padding);
    });
}
