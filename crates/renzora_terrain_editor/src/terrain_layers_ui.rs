//! "Layers" section of the terrain inspector, backed by the `Painter`
//! component on the terrain entity.
//!
//! The section edits the **active** layer (picked by the dropdown or the
//! terrain panel's layer list). Layer selection is written through
//! `SurfacePaintSettings.active_layer` — that resource is the source of
//! truth the panel and `painter_command_system` already agree on; writing
//! `Painter.active_layer` directly would be overwritten a frame later.

use bevy::prelude::*;
use renzora_editor_framework::{FieldDef, FieldType, FieldValue, InspectorEntry};
use renzora_terrain::data::TerrainData;
use renzora_terrain::paint::{SurfacePaintCommand, SurfacePaintSettings, SurfacePaintState};
use renzora_terrain::painter::{PaintLayer, Painter};

/// `ActiveBrushLayer` moved into the `Painter` itself (`active_layer`), but
/// during the transition we still register this type so the old
/// brush-layer paint path links. It's no longer written by the new UI.
#[derive(Resource, Default)]
// kept during the brush-layer transition so the old paint path still links
pub struct ActiveBrushLayer(#[allow(dead_code)] pub Option<Entity>);

fn active_layer(w: &World, e: Entity) -> Option<&PaintLayer> {
    let painter = w.get::<Painter>(e)?;
    painter.active_layer.and_then(|i| painter.layers.get(i))
}

/// Mutate the active layer, comparing before writing so a no-op edit doesn't
/// flag the `Painter` changed (that would churn the layer-mesh sync system).
fn edit_active_layer(w: &mut World, e: Entity, f: impl FnOnce(&mut PaintLayer)) {
    let Some(mut painter) = w.get_mut::<Painter>(e) else {
        return;
    };
    let Some(idx) = painter.active_layer else {
        return;
    };
    if let Some(layer) = painter.layers.get_mut(idx) {
        f(layer);
    }
}

pub fn terrain_layers_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "terrain_layers",
        display_name: "Layers",
        icon: "stack",
        category: "component",
        has_fn: |world, entity| world.get::<TerrainData>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Active Layer",
                field_type: FieldType::DynamicEnum {
                    options: |w, e| {
                        w.get::<Painter>(e)
                            .map(|p| p.layers.iter().map(|l| l.name.clone()).collect())
                            .unwrap_or_default()
                    },
                },
                get_fn: |w, e| {
                    let painter = w.get::<Painter>(e)?;
                    painter
                        .active_layer
                        .map(|i| FieldValue::Float(i as f32))
                },
                set_fn: |w, e, v| {
                    if let FieldValue::Float(idx) = v {
                        let count = w.get::<Painter>(e).map(|p| p.layers.len()).unwrap_or(0);
                        if count == 0 {
                            return;
                        }
                        let want = (idx.round() as usize).min(count - 1);
                        if let Some(mut settings) = w.get_resource_mut::<SurfacePaintSettings>() {
                            if settings.active_layer != want {
                                settings.active_layer = want;
                            }
                        }
                    }
                },
            },
            FieldDef {
                name: "Name",
                field_type: FieldType::String,
                get_fn: |w, e| active_layer(w, e).map(|l| FieldValue::String(l.name.clone())),
                set_fn: |w, e, v| {
                    if let FieldValue::String(name) = v {
                        edit_active_layer(w, e, |l| {
                            if l.name != name {
                                l.name = name;
                            }
                        });
                    }
                },
            },
            FieldDef {
                name: "Material",
                field_type: FieldType::Asset {
                    extensions: vec!["material".to_string()],
                },
                get_fn: |w, e| active_layer(w, e).map(|l| FieldValue::Asset(l.material_path.clone())),
                set_fn: |w, e, v| {
                    if let FieldValue::Asset(path) = v {
                        edit_active_layer(w, e, |l| {
                            if l.material_path != path {
                                l.material_path = path;
                                l.material_dirty = true;
                            }
                        });
                    }
                },
            },
            FieldDef {
                name: "Height Offset",
                field_type: FieldType::Float { speed: 0.002, min: 0.0, max: 2.0 },
                get_fn: |w, e| active_layer(w, e).map(|l| FieldValue::Float(l.height_offset)),
                set_fn: |w, e, v| {
                    if let FieldValue::Float(h) = v {
                        edit_active_layer(w, e, |l| {
                            if l.height_offset != h {
                                l.height_offset = h;
                                l.mesh_dirty = true;
                            }
                        });
                    }
                },
            },
            FieldDef {
                name: "Coverage Threshold",
                field_type: FieldType::Float { speed: 0.002, min: 0.0, max: 1.0 },
                get_fn: |w, e| {
                    active_layer(w, e).map(|l| FieldValue::Float(l.coverage_threshold))
                },
                set_fn: |w, e, v| {
                    if let FieldValue::Float(t) = v {
                        edit_active_layer(w, e, |l| {
                            if l.coverage_threshold != t {
                                l.coverage_threshold = t;
                                l.mesh_dirty = true;
                            }
                        });
                    }
                },
            },
            FieldDef {
                name: "Enabled",
                field_type: FieldType::Bool,
                get_fn: |w, e| active_layer(w, e).map(|l| FieldValue::Bool(l.enabled)),
                set_fn: |w, e, v| {
                    if let FieldValue::Bool(on) = v {
                        edit_active_layer(w, e, |l| {
                            if l.enabled != on {
                                l.enabled = on;
                            }
                        });
                    }
                },
            },
            FieldDef {
                name: "Add Layer",
                field_type: FieldType::Button { icon: "plus" },
                get_fn: |_, _| None,
                set_fn: |w, _, _| {
                    if let Some(mut state) = w.get_resource_mut::<SurfacePaintState>() {
                        state.pending_commands.push(SurfacePaintCommand::AddLayer);
                    }
                },
            },
            FieldDef {
                name: "Remove Layer",
                field_type: FieldType::Button { icon: "trash" },
                get_fn: |_, _| None,
                set_fn: |w, e, _| {
                    let Some(idx) = w.get::<Painter>(e).and_then(|p| p.active_layer) else {
                        return;
                    };
                    if let Some(mut state) = w.get_resource_mut::<SurfacePaintState>() {
                        state
                            .pending_commands
                            .push(SurfacePaintCommand::RemoveLayer(idx));
                    }
                },
            },
        ],
    }
}
