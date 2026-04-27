//! Generic field rendering — maps FieldType to the appropriate egui widget.

use bevy::prelude::*;
use bevy_egui::egui;
use renzora_editor::{
    asset_drop_target, inline_property, property_row, toggle_switch, AssetDragPayload,
    EditorCommands, FieldDef, FieldType, FieldValue,
};
use renzora_theme::Theme;
use renzora_undo::{self, FieldChangeCmd, UndoContext};

/// Axis tint colors used by Godot-style Vec3 cells.
const AXIS_X: egui::Color32 = egui::Color32::from_rgb(230, 90, 90);
const AXIS_Y: egui::Color32 = egui::Color32::from_rgb(130, 200, 90);
const AXIS_Z: egui::Color32 = egui::Color32::from_rgb(90, 150, 230);

/// Render one Godot-style axis cell: a single tinted-letter prefix glued to
/// a DragValue inside a unified rectangle. The whole cell is `width` wide so
/// three of them tile evenly across the row, and every piece is positioned
/// from a single allocated rect (no per-item layout drift between cells).
fn axis_cell(
    ui: &mut egui::Ui,
    letter: &str,
    color: egui::Color32,
    value: &mut f32,
    speed: f32,
    width: f32,
    theme: &Theme,
) {
    let cell_height = 32.0;
    let chip_width = 22.0;
    let (rect, _resp) =
        ui.allocate_exact_size(egui::vec2(width, cell_height), egui::Sense::hover());

    // Cell background — slightly inset, matches Godot's input look.
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius::same(3),
        theme.surfaces.extreme.to_color32(),
    );

    // Axis chip on the left — a colored tint behind the centered letter.
    let chip_rect = egui::Rect::from_min_max(
        rect.left_top(),
        egui::pos2(rect.left() + chip_width, rect.bottom()),
    );
    let chip_bg = egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 38);
    ui.painter().rect_filled(
        chip_rect,
        egui::CornerRadius {
            nw: 3,
            sw: 3,
            ne: 0,
            se: 0,
        },
        chip_bg,
    );
    ui.painter().text(
        chip_rect.center(),
        egui::Align2::CENTER_CENTER,
        letter,
        egui::FontId::proportional(12.0),
        color,
    );

    // DragValue covers the area to the right of the chip. Frame is stripped
    // to transparent so the single cell-bg rect shows through, giving the
    // unified-rectangle look from Godot's inspector.
    let drag_rect = egui::Rect::from_min_max(
        egui::pos2(chip_rect.right(), rect.top()),
        rect.right_bottom(),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(drag_rect)
            .layout(egui::Layout::centered_and_justified(
                egui::Direction::LeftToRight,
            )),
    );
    {
        let visuals = &mut child.style_mut().visuals.widgets;
        visuals.inactive.bg_fill = egui::Color32::TRANSPARENT;
        visuals.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
        visuals.hovered.bg_fill = egui::Color32::TRANSPARENT;
        visuals.hovered.weak_bg_fill = egui::Color32::TRANSPARENT;
        visuals.active.bg_fill = egui::Color32::TRANSPARENT;
        visuals.active.weak_bg_fill = egui::Color32::TRANSPARENT;
        visuals.inactive.bg_stroke = egui::Stroke::NONE;
        visuals.hovered.bg_stroke = egui::Stroke::NONE;
        visuals.active.bg_stroke = egui::Stroke::NONE;
    }
    child.add(egui::DragValue::new(value).speed(speed));
}

fn push_field_change(
    cmds: &EditorCommands,
    entity: Entity,
    field_name: &'static str,
    old: FieldValue,
    new: FieldValue,
    set_fn: fn(&mut World, Entity, FieldValue),
) {
    cmds.push(move |world| {
        renzora_undo::execute(world, UndoContext::Scene, Box::new(FieldChangeCmd {
            entity, field_name, old, new, set_fn,
        }));
    });
}

/// Render a small reset-to-default button. Returns true if clicked.
fn reset_button(ui: &mut egui::Ui, theme: &Theme) -> bool {
    use egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE;
    let resp = ui.add(
        egui::Button::new(
            egui::RichText::new(ARROW_COUNTER_CLOCKWISE)
                .size(10.0)
                .color(theme.text.disabled.to_color32()),
        )
        .frame(false)
        .min_size(egui::vec2(14.0, 14.0)),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.on_hover_text("Reset to default").clicked()
}

/// Render a single field row using the appropriate widget for its type.
///
/// Reads the current value via `field.get_fn`, renders an editable widget,
/// and if the value changed, pushes a deferred write via `EditorCommands`.
pub fn render_field(
    ui: &mut egui::Ui,
    field: &FieldDef,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
    row_index: usize,
) {
    let Some(value) = (field.get_fn)(world, entity) else {
        return;
    };

    match (&field.field_type, value) {
        (FieldType::Float { speed, min, max }, FieldValue::Float(mut v)) => {
            let speed = *speed;
            let min = *min;
            let max = *max;
            let set_fn = field.set_fn;

            inline_property(ui, row_index, field.name, theme, |ui| {
                let orig = v;
                ui.add(
                    egui::DragValue::new(&mut v)
                        .speed(speed)
                        .range(min..=max),
                );
                if v != orig {
                    push_field_change(cmds, entity, field.name,
                        FieldValue::Float(orig), FieldValue::Float(v), set_fn);
                }
                if reset_button(ui, theme) {
                    let new = FieldValue::Float(orig).type_default();
                    push_field_change(cmds, entity, field.name,
                        FieldValue::Float(orig), new, set_fn);
                }
            });
        }

        (FieldType::Vec3 { speed }, FieldValue::Vec3(mut v)) => {
            let speed = *speed;
            let set_fn = field.set_fn;
            let orig = v;

            // Godot-style two-row group: field name on top with the reset
            // button trailing on the right, three colored-letter cells below.
            // The whole group sits on a flat slab of `inspector_row_odd`
            // (the lighter alternating color) — no row striping inside the
            // component, but the group still reads as a distinct band.
            let _ = row_index;
            egui::Frame::new()
                .fill(theme.panels.inspector_row_odd.to_color32())
                .inner_margin(egui::Margin::symmetric(6, 4))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 4.0;
                    ui.style_mut().spacing.indent = 0.0;

                    // Header row: name (left) + reset button (right).
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(field.name).size(11.0));
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if reset_button(ui, theme) {
                                    push_field_change(
                                        cmds,
                                        entity,
                                        field.name,
                                        FieldValue::Vec3(orig),
                                        FieldValue::Vec3([0.0; 3]),
                                        set_fn,
                                    );
                                }
                            },
                        );
                    });

                    // Cells row: three equal-width axis cells.
                    ui.horizontal(|ui| {
                        let gap = 4.0;
                        ui.spacing_mut().item_spacing.x = gap;
                        let avail = ui.available_width();
                        let cell_w = ((avail - 2.0 * gap) / 3.0).floor().max(60.0);

                        axis_cell(ui, "x", AXIS_X, &mut v[0], speed, cell_w, theme);
                        axis_cell(ui, "y", AXIS_Y, &mut v[1], speed, cell_w, theme);
                        axis_cell(ui, "z", AXIS_Z, &mut v[2], speed, cell_w, theme);
                    });

                    if v != orig {
                        push_field_change(
                            cmds,
                            entity,
                            field.name,
                            FieldValue::Vec3(orig),
                            FieldValue::Vec3(v),
                            set_fn,
                        );
                    }
                });
        }

        (FieldType::Bool, FieldValue::Bool(v)) => {
            let set_fn = field.set_fn;

            inline_property(ui, row_index, field.name, theme, |ui| {
                let id = ui.id().with(field.name);
                if toggle_switch(ui, id, v) {
                    let new_val = !v;
                    push_field_change(cmds, entity, field.name,
                        FieldValue::Bool(v), FieldValue::Bool(new_val), set_fn);
                }
                if reset_button(ui, theme) {
                    push_field_change(cmds, entity, field.name,
                        FieldValue::Bool(v), FieldValue::Bool(false), set_fn);
                }
            });
        }

        (FieldType::Color, FieldValue::Color(mut rgb)) => {
            let set_fn = field.set_fn;

            inline_property(ui, row_index, field.name, theme, |ui| {
                let orig = rgb;
                if ui.color_edit_button_rgb(&mut rgb).changed() && rgb != orig {
                    push_field_change(cmds, entity, field.name,
                        FieldValue::Color(orig), FieldValue::Color(rgb), set_fn);
                }
                if reset_button(ui, theme) {
                    push_field_change(cmds, entity, field.name,
                        FieldValue::Color(orig), FieldValue::Color([1.0; 3]), set_fn);
                }
            });
        }

        (FieldType::String, FieldValue::String(mut s)) => {
            let set_fn = field.set_fn;

            inline_property(ui, row_index, field.name, theme, |ui| {
                let orig = s.clone();
                ui.add(
                    egui::TextEdit::singleline(&mut s)
                        .desired_width((ui.available_width() - 28.0).max(40.0)),
                );
                if s != orig {
                    push_field_change(cmds, entity, field.name,
                        FieldValue::String(orig.clone()), FieldValue::String(s), set_fn);
                }
                if reset_button(ui, theme) {
                    push_field_change(cmds, entity, field.name,
                        FieldValue::String(orig), FieldValue::String(String::new()), set_fn);
                }
            });
        }

        (FieldType::ReadOnly, FieldValue::ReadOnly(text)) => {
            inline_property(ui, row_index, field.name, theme, |ui| {
                ui.label(
                    egui::RichText::new(&text)
                        .size(11.0)
                        .color(theme.text.muted.to_color32()),
                );
            });
        }

        (FieldType::Asset { extensions }, FieldValue::Asset(current)) => {
            let set_fn = field.set_fn;
            let extensions = extensions.clone();

            inline_property(ui, row_index, field.name, theme, |ui| {
                let payload = world.get_resource::<AssetDragPayload>();
                let ext_refs: Vec<&str> = extensions.iter().map(|s| s.as_str()).collect();
                let current_str = current.as_deref();

                let drop_result = asset_drop_target(
                    ui,
                    ui.id().with(field.name),
                    current_str,
                    &ext_refs,
                    "Drag asset here",
                    theme,
                    payload,
                );

                if let Some(path) = drop_result.dropped_path {
                    let path_str = if let Some(project) = world.get_resource::<renzora::core::CurrentProject>() {
                        project.make_asset_relative(&path)
                    } else {
                        path.to_string_lossy().to_string()
                    };
                    push_field_change(cmds, entity, field.name,
                        FieldValue::Asset(current.clone()),
                        FieldValue::Asset(Some(path_str)), set_fn);
                }
                if drop_result.cleared {
                    push_field_change(cmds, entity, field.name,
                        FieldValue::Asset(current.clone()),
                        FieldValue::Asset(None), set_fn);
                }
                if reset_button(ui, theme) {
                    push_field_change(cmds, entity, field.name,
                        FieldValue::Asset(current.clone()),
                        FieldValue::Asset(None), set_fn);
                }
            });
        }

        _ => {} // mismatched type/value — skip
    }
}
