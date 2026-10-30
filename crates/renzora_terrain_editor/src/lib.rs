//! Terrain Editor: the Terrain component's inspector body (Manage, Sculpt,
//! Paint, Foliage), and the sculpting, painting and brush gizmo systems behind
//! it.

mod brush_gizmo;
mod cluster_bake;
mod generate_tool;
mod landscape_ops;
mod landscape_paint;
mod operations;
mod panel;
mod plane_terrain;
mod spline_gizmos;
mod surface_ui;
mod systems;
mod terrain_inspector;

use bevy::prelude::*;
use renzora_editor_framework::{
    ActiveTool, AppEditorExt, EntityPreset, FieldDef, FieldType, FieldValue, InspectorEntry, ToolEntry,
    ToolSection,
};
use renzora::SplinePath;
use renzora_terrain::data::TerrainData;

use terrain_inspector::{follow_armed_tool_system, sync_active_tool_system, ShownTerrainTab, TerrainInspectorTab};

#[derive(Default)]
pub struct TerrainEditorPlugin;

impl Plugin for TerrainEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] TerrainEditorPlugin");
        // The terrain tools, as the Terrain component's inspector body.
        app.add_plugins(panel::TerrainToolsPanel);
        operations::install(app);
        cluster_bake::install(app);
        // The material slot, the generator and every other control live in
        // the component body (`panel`); the old Landscape Material section
        // and the Generate / Resize / Edit Terrain entry points are folded in.
        app.register_inspector(terrain_data_entry())
            .init_resource::<TerrainInspectorTab>()
            .init_resource::<ShownTerrainTab>()
            .init_resource::<systems::HeightPick>();

        // **The terrain modes are not on the viewport shelf.** They were buttons
        // there next to the gizmos, until the brushes moved into this component:
        // picking a brush now arms its own mode
        // (`panel::systems::sculpt_tool_click`), so a separate mode button was a
        // second click for what the first one had already done.
        //
        // **Make Terrain is the exception, and stays.** It is the one entry here
        // that acts on something that is *not* a terrain (it converts a
        // selected plane into one), so there is no Terrain component for it to
        // live in. Without a shelf button it would have nowhere at all.
        app.register_tool(
            ToolEntry::new(
                "builtin.terrain_from_plane",
                plane_terrain::TOOL_ICON,
                "Make Terrain: sculpt this plane as terrain",
                ToolSection::Shelf("terrain.0-make"),
            )
            .visible_if(plane_terrain::plane_selected)
            .on_activate(plane_terrain::convert_selected),
        );
        app
            // ActiveTool follows the terrain inspector's armed tab + selection,
            // and the shown pane follows a brush being armed.
            .add_systems(Update, (sync_active_tool_system, follow_armed_tool_system).chain())
            // Before the runtime reads a terrain's material and the resolver
            // compiles it, so a new terrain finds Alpine Landscape and its
            // textures already in the project.
            .init_resource::<surface_ui::DefaultTextureBakes>()
            .add_systems(
                PreUpdate,
                (surface_ui::provide_default_textures, surface_ui::finish_default_texture_bakes).chain(),
            )
            // Sculpt systems: active when ActiveTool is TerrainSculpt.
            .add_systems(
                Update,
                (
                    systems::terrain_sculpt_hover_system,
                    // Between the two: an armed Pick turns the click that
                    // the hover system just sampled into a target height.
                    systems::height_pick_system
                        .after(systems::terrain_sculpt_hover_system)
                        .before(systems::terrain_sculpt_system),
                    systems::terrain_sculpt_system,
                    systems::terrain_brush_scroll_system,
                )
                    .run_if(active_tool_is(ActiveTool::TerrainSculpt))
                    .run_if(renzora::core::not_in_play_mode),
            )
            // Paint mode: hover raycast + the landscape weight brush. The
            // brush runs after the stroke opens, which clears the previous
            // stroke's undo record, so its first dab is recorded too.
            .add_systems(
                Update,
                (
                    systems::terrain_paint_hover_system,
                    landscape_paint::landscape_paint_system
                        .after(systems::terrain_paint_hover_system)
                        .after(systems::terrain_stroke_begin_system)
                        .before(systems::terrain_stroke_end_system),
                    landscape_paint::landscape_brush_scroll_system,
                )
                    .run_if(active_tool_is(ActiveTool::TerrainPaint))
                    .run_if(renzora::core::not_in_play_mode),
            )
            // Commands from the panel (a material dropped or cleared) apply
            // even while another tool is active, so the panel is never a dead
            // UI.
            .add_systems(
                Update,
                landscape_paint::landscape_command_system.run_if(renzora::core::not_in_play_mode),
            )
            // Undo capture: active when any terrain tool is on. Undo/redo
            // *replay* is handled by the central `renzora_undo` shortcut router
            // (Ctrl+Z/Ctrl+Y); strokes record onto the Scene stack here.
            .add_systems(
                Update,
                (
                    systems::terrain_stroke_begin_system,
                    systems::terrain_stroke_end_system,
                )
                    .run_if(|tool: Option<Res<ActiveTool>>| tool.is_some_and(|t| t.is_terrain()))
                    .run_if(renzora::core::not_in_play_mode),
            )
            // Keep the layer preview cache in sync so the terrain inspector
            // always shows current layer state.
            .add_systems(
                Update,
                systems::sync_layer_preview_system.run_if(renzora::core::not_in_play_mode),
            )
            // Spline gizmos: always drawn in the editor (not in play mode).
            .add_systems(
                Update,
                spline_gizmos::draw_spline_gizmos_system.run_if(renzora::core::not_in_play_mode),
            );

        app.register_entity_preset(EntityPreset {
            id: "terrain",
            display_name: "Terrain",
            icon: "mountains",
            category: "general",
            spawn_fn: |world| renzora_terrain::mesh::spawn_terrain(world),
        });

        app.register_entity_preset(EntityPreset {
            id: "spline",
            display_name: "Spline",
            icon: "path",
            category: "general",
            spawn_fn: |world| {
                world
                    .spawn((
                        Name::new("Spline"),
                        Transform::default(),
                        Visibility::default(),
                        SplinePath::with_points([
                            Vec3::new(-4.0, 0.0, 0.0),
                            Vec3::new(-1.5, 0.0, 1.5),
                            Vec3::new(1.5, 0.0, -1.5),
                            Vec3::new(4.0, 0.0, 0.0),
                        ]),
                    ))
                    .id()
            },
        });
    }
}

fn active_tool_is(expected: ActiveTool) -> impl FnMut(Option<Res<ActiveTool>>) -> bool {
    move |tool: Option<Res<ActiveTool>>| tool.is_some_and(|t| *t == expected)
}

fn terrain_data_entry() -> InspectorEntry {
    // The inspector draws the component body (`panel`) in place of these
    // fields; they stay because scripted callers (the MCP's
    // set_component_field) read and write the terrain through them. Every set
    // compares first and only writes when the value moved: every real
    // `TerrainData` change runs the chunk rebuild check.
    //
    // The *structural* fields (grid size, chunk size, resolution) are
    // deliberately not here. Each write of one rebuilds every chunk with a
    // fresh collider, so they go through the Manage tab, which stages them
    // until Apply (`panel::manage::ManageDraft`) or resizes one row at a time
    // (`panel::chunk_map`).
    InspectorEntry {
        type_id: "terrain_data",
        display_name: "Terrain",
        icon: "mountains",
        category: "environment",
        has_fn: |world, entity| world.get::<TerrainData>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Size",
                field_type: FieldType::ReadOnly,
                get_fn: |w, e| {
                    w.get::<TerrainData>(e).map(|t| {
                        FieldValue::String(format!(
                            "{} × {} chunks · {:.0} × {:.0} m",
                            t.chunks_x,
                            t.chunks_z,
                            t.total_width(),
                            t.total_depth()
                        ))
                    })
                },
                set_fn: |_, _, _| {},
            },
            FieldDef {
                name: "Min Height",
                field_type: FieldType::Float { speed: 0.25, min: -500.0, max: 500.0 },
                get_fn: |w, e| {
                    w.get::<TerrainData>(e)
                        .map(|t| FieldValue::Float(t.min_height))
                },
                set_fn: |w, e, v| {
                    if let FieldValue::Float(h) = v {
                        if let Some(mut t) = w.get_mut::<TerrainData>(e) {
                            // Keep at least 1m of range so height_range()
                            // never collapses to zero (NaN normals).
                            let want = h.min(t.max_height - 1.0);
                            if t.min_height != want {
                                t.min_height = want;
                            }
                        }
                    }
                },
            },
            FieldDef {
                name: "Max Height",
                field_type: FieldType::Float { speed: 0.25, min: -500.0, max: 500.0 },
                get_fn: |w, e| {
                    w.get::<TerrainData>(e)
                        .map(|t| FieldValue::Float(t.max_height))
                },
                set_fn: |w, e, v| {
                    if let FieldValue::Float(h) = v {
                        if let Some(mut t) = w.get_mut::<TerrainData>(e) {
                            let want = h.max(t.min_height + 1.0);
                            if t.max_height != want {
                                t.max_height = want;
                            }
                        }
                    }
                },
            },
            FieldDef {
                name: "Stream Chunks",
                field_type: FieldType::Bool,
                get_fn: |w, e| {
                    w.get::<TerrainData>(e)
                        .map(|t| FieldValue::Bool(t.stream_chunks))
                },
                set_fn: |w, e, v| {
                    if let FieldValue::Bool(b) = v {
                        if let Some(mut t) = w.get_mut::<TerrainData>(e) {
                            if t.stream_chunks != b {
                                t.stream_chunks = b;
                            }
                        }
                    }
                },
            },
            FieldDef {
                name: "Stream Radius",
                field_type: FieldType::Float { speed: 1.0, min: 32.0, max: 4096.0 },
                get_fn: |w, e| {
                    w.get::<TerrainData>(e)
                        .map(|t| FieldValue::Float(t.stream_radius))
                },
                set_fn: |w, e, v| {
                    if let FieldValue::Float(r) = v {
                        let want = r.clamp(32.0, 4096.0);
                        if let Some(mut t) = w.get_mut::<TerrainData>(e) {
                            if t.stream_radius != want {
                                t.stream_radius = want;
                            }
                        }
                    }
                },
            },
        ],
    }
}

renzora::add!(TerrainEditorPlugin, Editor);
