//! Shape library editor panel.

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CursorIcon, Vec2};
use egui_phosphor::regular;
use renzora_core::{MeshColor, MeshPrimitive, ShapeEntry, ShapeRegistry};
use renzora_editor::{EditorCommands, EditorPanel, PanelLocation, ShapeDragState};
use renzora_theme::ThemeManager;

#[derive(Default)]
struct ShapeLibraryState {
    search_filter: String,
}

pub struct ShapeLibraryPanel {
    state: RwLock<ShapeLibraryState>,
}

impl Default for ShapeLibraryPanel {
    fn default() -> Self {
        Self {
            state: RwLock::new(ShapeLibraryState::default()),
        }
    }
}

impl EditorPanel for ShapeLibraryPanel {
    fn id(&self) -> &str {
        "shape_library"
    }

    fn title(&self) -> &str {
        "Shapes"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::CUBE)
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Left
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
        let registry = match world.get_resource::<ShapeRegistry>() {
            Some(r) => r,
            None => return,
        };
        let drag_state = world.get_resource::<ShapeDragState>();
        let current_dragging = drag_state.and_then(|s| s.dragging_shape);

        let mut state = self.state.write().unwrap();

        let accent = theme.semantic.accent.to_color32();
        let text_primary = theme.text.primary.to_color32();
        let text_muted = theme.text.muted.to_color32();

        let panel_padding = 6.0;
        let spacing = 4.0;

        ui.add_space(panel_padding);

        // Search bar
        ui.horizontal(|ui| {
            ui.add_space(panel_padding);
            let available = ui.available_width() - panel_padding;
            ui.add_sized(
                [available, 20.0],
                egui::TextEdit::singleline(&mut state.search_filter)
                    .hint_text(format!("{} Search shapes...", regular::MAGNIFYING_GLASS)),
            );
        });

        ui.add_space(spacing);

        // Filter shapes by search only (no categories)
        let search_lower = state.search_filter.to_lowercase();
        let shapes: Vec<&ShapeEntry> = registry
            .iter()
            .filter(|s| search_lower.is_empty() || s.name.to_lowercase().contains(&search_lower))
            .collect();

        // Responsive grid
        egui::ScrollArea::vertical().show(ui, |ui| {
            let available_width = ui.available_width() - panel_padding * 2.0;
            let min_tile = 48.0;
            let cols =
                ((available_width + spacing) / (min_tile + spacing)).floor().max(1.0) as usize;
            let tile_size =
                (available_width - spacing * (cols as f32 - 1.0)) / cols as f32;
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
                            if idx >= shapes.len() {
                                break;
                            }
                            let shape = shapes[idx];
                            let is_this_dragging = current_dragging == Some(shape.id);
                            let total_height = tile_size + label_height;
                            let (rect, response) = ui.allocate_exact_size(
                                Vec2::new(tile_size, total_height),
                                egui::Sense::click_and_drag(),
                            );

                            let hovered = response.hovered();
                            let dragging = response.dragged();
                            let bg = if dragging || is_this_dragging {
                                accent.linear_multiply(0.3)
                            } else if hovered {
                                theme.widgets.hovered_bg.to_color32()
                            } else {
                                theme.surfaces.faint.to_color32()
                            };

                            ui.painter().rect_filled(rect, 6.0, bg);

                            if hovered || dragging || is_this_dragging {
                                let border_color = if dragging || is_this_dragging {
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
                                if !dragging && !is_this_dragging {
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
                                shape.icon,
                                egui::FontId::proportional(28.0),
                                if hovered || dragging || is_this_dragging {
                                    accent
                                } else {
                                    text_primary
                                },
                            );

                            // Label
                            ui.painter().text(
                                egui::pos2(rect.center().x, rect.max.y - label_height / 2.0),
                                egui::Align2::CENTER_CENTER,
                                shape.name,
                                egui::FontId::proportional(9.5),
                                text_muted,
                            );

                            response
                                .clone()
                                .on_hover_text(format!("Drag to place {}", shape.name));

                            // Drag started — set dragging_shape on ShapeDragState
                            if response.drag_started() {
                                let shape_id = shape.id;
                                commands.push(move |world: &mut World| {
                                    world.resource_mut::<ShapeDragState>().dragging_shape =
                                        Some(shape_id);
                                });
                            }

                            // Show floating drag indicator
                            if dragging || is_this_dragging {
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
                                        shape.name,
                                        egui::FontId::proportional(11.0),
                                        accent,
                                    );
                                }
                            }

                            // Click to spawn at origin (fallback)
                            if response.clicked() && current_dragging.is_none() {
                                let create_mesh = shape.create_mesh;
                                let name = shape.name;
                                let color = shape.default_color;
                                let shape_id = shape.id;
                                commands.push(move |world: &mut World| {
                                    let mesh =
                                        create_mesh(&mut world.resource_mut::<Assets<Mesh>>());
                                    let material = world
                                        .resource_mut::<Assets<StandardMaterial>>()
                                        .add(StandardMaterial {
                                            base_color: color,
                                            perceptual_roughness: 0.9,
                                            ..default()
                                        });
                                    world.spawn((
                                        Name::new(name),
                                        Transform::default(),
                                        Mesh3d(mesh),
                                        MeshMaterial3d(material),
                                        MeshPrimitive(shape_id.to_string()),
                                        MeshColor(color),
                                    ));
                                });
                            }
                        }
                    });
                }
            });
            ui.add_space(panel_padding);
        });
    }
}
