//! Terrain Editor — sculpting, painting, and brush gizmo systems.

mod panel;
mod systems;
mod tool_options;
mod tool_panel;

use bevy::prelude::*;
use renzora::egui_phosphor::regular;
use renzora::editor::{ActiveTool, AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry, ToolOptionsRegistry};
use renzora_terrain::data::TerrainData;

#[derive(Default)]
pub struct TerrainEditorPlugin;

impl Plugin for TerrainEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] TerrainEditorPlugin");
        app.register_panel(tool_panel::ToolSettingsPanel::new())
            .register_inspector(terrain_data_entry())
            .init_resource::<ToolOptionsRegistry>();

        // Register context-sensitive viewport-header options for brush tools.
        {
            let mut reg = app.world_mut().resource_mut::<ToolOptionsRegistry>();
            reg.register(ActiveTool::TerrainSculpt, tool_options::draw_sculpt_options);
            reg.register(ActiveTool::TerrainPaint,  tool_options::draw_paint_options);
        }

        app
            // Sculpt systems — active when ActiveTool is TerrainSculpt
            .add_systems(
                Update,
                (
                    systems::terrain_sculpt_hover_system,
                    systems::terrain_sculpt_system,
                    systems::terrain_brush_scroll_system,
                )
                    .run_if(active_tool_is(ActiveTool::TerrainSculpt))
                    .run_if(renzora::core::not_in_play_mode),
            )
            // Paint systems — active when ActiveTool is TerrainPaint
            .add_systems(
                Update,
                (
                    systems::terrain_paint_activate_system,
                    systems::terrain_paint_hover_system,
                    systems::terrain_paint_system,
                    systems::terrain_paint_scroll_system,
                    systems::terrain_paint_command_system,
                )
                    .run_if(active_tool_is(ActiveTool::TerrainPaint))
                    .run_if(renzora::core::not_in_play_mode),
            )
            // Undo/redo — active when any terrain tool is on
            .add_systems(
                Update,
                (
                    systems::terrain_stroke_begin_system,
                    systems::terrain_stroke_end_system,
                    systems::terrain_undo_redo_system,
                )
                    .run_if(|tool: Option<Res<ActiveTool>>| {
                        tool.map_or(false, |t| t.is_terrain())
                    })
                    .run_if(renzora::core::not_in_play_mode),
            );
    }
}

fn active_tool_is(expected: ActiveTool) -> impl FnMut(Option<Res<ActiveTool>>) -> bool {
    move |tool: Option<Res<ActiveTool>>| {
        tool.map_or(false, |t| *t == expected)
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


renzora::add!(TerrainEditorPlugin, Editor);
