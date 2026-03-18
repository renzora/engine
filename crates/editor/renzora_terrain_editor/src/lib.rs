//! Terrain Editor — sculpting, painting, and brush gizmo systems.

mod panel;
mod systems;

use bevy::prelude::*;
use egui_phosphor::regular;
use renzora_editor::{AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry};
use renzora_terrain::data::{TerrainData, TerrainTab, TerrainToolState};

pub struct TerrainEditorPlugin;

impl Plugin for TerrainEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] TerrainEditorPlugin");
        app.register_panel(panel::TerrainToolsPanel::new())
            .register_inspector(terrain_data_entry())
            // Sculpt systems — active when tool is on and tab is Sculpt
            .add_systems(
                Update,
                (
                    systems::terrain_sculpt_hover_system,
                    systems::terrain_sculpt_system,
                    systems::terrain_brush_scroll_system,
                )
                    .run_if(resource_equals(TerrainToolState { active: true }))
                    .run_if(terrain_tab_is(TerrainTab::Sculpt)),
            )
            // Paint systems — active when tool is on and tab is Paint
            .add_systems(
                Update,
                (
                    systems::terrain_paint_activate_system,
                    systems::terrain_paint_hover_system,
                    systems::terrain_paint_system,
                    systems::terrain_paint_scroll_system,
                    systems::terrain_paint_command_system,
                )
                    .run_if(resource_equals(TerrainToolState { active: true }))
                    .run_if(terrain_tab_is(TerrainTab::Paint)),
            )
            // Undo/redo — active when terrain mode is on
            .add_systems(
                Update,
                (
                    systems::terrain_stroke_begin_system,
                    systems::terrain_stroke_end_system,
                    systems::terrain_undo_redo_system,
                )
                    .run_if(resource_equals(TerrainToolState { active: true })),
            );
    }
}

fn terrain_tab_is(tab: TerrainTab) -> impl FnMut(Option<Res<renzora_terrain::data::TerrainSettings>>) -> bool {
    move |settings: Option<Res<renzora_terrain::data::TerrainSettings>>| {
        settings.map_or(false, |s| s.tab == tab)
    }
}

fn terrain_data_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "terrain_data",
        display_name: "Terrain",
        icon: regular::MOUNTAINS,
        category: "component",
        has_fn: |world, entity| world.get::<TerrainData>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Chunks X",
                field_type: FieldType::Float { speed: 1.0, min: 1.0, max: 32.0 },
                get_fn: |world, entity| {
                    world.get::<TerrainData>(entity).map(|t| FieldValue::Float(t.chunks_x as f32))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut t) = world.get_mut::<TerrainData>(entity) {
                            t.chunks_x = (v as u32).max(1);
                        }
                    }
                },
            },
            FieldDef {
                name: "Chunks Z",
                field_type: FieldType::Float { speed: 1.0, min: 1.0, max: 32.0 },
                get_fn: |world, entity| {
                    world.get::<TerrainData>(entity).map(|t| FieldValue::Float(t.chunks_z as f32))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut t) = world.get_mut::<TerrainData>(entity) {
                            t.chunks_z = (v as u32).max(1);
                        }
                    }
                },
            },
            FieldDef {
                name: "Chunk Size",
                field_type: FieldType::Float { speed: 1.0, min: 8.0, max: 256.0 },
                get_fn: |world, entity| {
                    world.get::<TerrainData>(entity).map(|t| FieldValue::Float(t.chunk_size))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut t) = world.get_mut::<TerrainData>(entity) {
                            t.chunk_size = v.max(8.0);
                        }
                    }
                },
            },
            FieldDef {
                name: "Resolution",
                field_type: FieldType::Float { speed: 1.0, min: 3.0, max: 257.0 },
                get_fn: |world, entity| {
                    world.get::<TerrainData>(entity).map(|t| FieldValue::Float(t.chunk_resolution as f32))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut t) = world.get_mut::<TerrainData>(entity) {
                            t.chunk_resolution = (v as u32).max(3);
                        }
                    }
                },
            },
            FieldDef {
                name: "Max Height",
                field_type: FieldType::Float { speed: 1.0, min: 0.0, max: 1000.0 },
                get_fn: |world, entity| {
                    world.get::<TerrainData>(entity).map(|t| FieldValue::Float(t.max_height))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut t) = world.get_mut::<TerrainData>(entity) {
                            t.max_height = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Min Height",
                field_type: FieldType::Float { speed: 1.0, min: -500.0, max: 0.0 },
                get_fn: |world, entity| {
                    world.get::<TerrainData>(entity).map(|t| FieldValue::Float(t.min_height))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut t) = world.get_mut::<TerrainData>(entity) {
                            t.min_height = v;
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

