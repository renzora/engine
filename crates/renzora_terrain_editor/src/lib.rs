//! Terrain Editor — sculpting, painting, and brush gizmo systems.

mod brush_layer_paint;
mod native;
mod spline_gizmos;
mod systems;
mod terrain_inspector;
mod terrain_layers_ui;

use bevy::prelude::*;
use renzora_editor_framework::{
    ActiveTool, AppEditorExt, EditorSelection, EntityPreset, InspectorEntry, ToolEntry,
    ToolSection,
};
use renzora_spline::SplinePath;
use renzora_terrain::data::TerrainData;

use terrain_inspector::{sync_active_tool_system, TerrainInspectorTab};

#[derive(Default)]
pub struct TerrainEditorPlugin;

impl Plugin for TerrainEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] TerrainEditorPlugin");
        // Native (bevy_ui/ember) terrain tools panel (id "terrain_tools").
        app.add_plugins(native::NativeTerrain);
        app.register_inspector(terrain_data_entry())
            .register_inspector(terrain_layers_ui::terrain_layers_entry())
            .init_resource::<TerrainInspectorTab>()
            .init_resource::<terrain_layers_ui::ActiveBrushLayer>();

        // Terrain toolbar buttons — visible whenever a terrain exists in the
        // scene (even if not currently selected). Clicking selects the terrain,
        // switches the inspector tab, and activates the brush tool. Clicking
        // the active button again reverts to Select.
        app.register_tool(
            ToolEntry::new(
                "builtin.terrain_sculpt",
                "mountains",
                "Sculpt Terrain",
                ToolSection::Terrain,
            )
            .order(0)
            .visible_if(terrain_exists_in_scene)
            .active_if(|w| {
                w.get_resource::<ActiveTool>()
                    .copied() == Some(ActiveTool::TerrainSculpt)
            })
            .on_activate(|w| {
                activate_terrain_tool(w, TerrainInspectorTab::Sculpt, ActiveTool::TerrainSculpt)
            }),
        );
        app.register_tool(
            ToolEntry::new(
                "builtin.terrain_paint",
                "paint-brush",
                "Paint Terrain Layers",
                ToolSection::Terrain,
            )
            .order(1)
            .visible_if(terrain_exists_in_scene)
            .active_if(|w| {
                w.get_resource::<ActiveTool>()
                    .copied() == Some(ActiveTool::TerrainPaint)
            })
            .on_activate(|w| {
                activate_terrain_tool(w, TerrainInspectorTab::Paint, ActiveTool::TerrainPaint)
            }),
        );
        app.register_tool(
            ToolEntry::new(
                "builtin.foliage_paint",
                "tree",
                "Paint Foliage",
                ToolSection::Terrain,
            )
            .order(2)
            .visible_if(terrain_exists_in_scene)
            .active_if(|w| {
                w.get_resource::<ActiveTool>()
                    .copied() == Some(ActiveTool::FoliagePaint)
            })
            .on_activate(|w| {
                activate_terrain_tool(w, TerrainInspectorTab::Foliage, ActiveTool::FoliagePaint)
            }),
        );

        app
            // ActiveTool follows the terrain inspector's active tab + selection.
            .add_systems(Update, sync_active_tool_system)
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
            // Paint mode: hover raycast + brush-layer paint write. Old
            // splatmap paint systems (terrain_paint_system, _activate,
            // _command) are no longer registered — the new `Painter`
            // component is the single source of truth. Their source is
            // still present and will get cleaned up in Phase C.
            .add_systems(
                Update,
                (
                    systems::terrain_paint_hover_system,
                    brush_layer_paint::brush_layer_paint_system,
                    brush_layer_paint::brush_layer_scroll_system,
                )
                    .run_if(active_tool_is(ActiveTool::TerrainPaint))
                    .run_if(renzora::core::not_in_play_mode),
            )
            // Undo capture — active when any terrain tool is on. Undo/redo
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
            // Spline gizmos — always drawn in the editor (not in play mode).
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

/// Toolbar visibility predicate: a terrain exists AND the viewport is in
/// Scene mode. The mesh Edit/Sculpt modes have their own toolbar section;
/// showing terrain brushes there reads as the wrong tool set (they don't
/// operate on the edited mesh).
fn terrain_exists_in_scene(world: &World) -> bool {
    use renzora::core::viewport_types::{ViewportMode, ViewportSettings};
    let scene_mode = world
        .get_resource::<ViewportSettings>()
        .map(|s| s.viewport_mode == ViewportMode::Scene)
        .unwrap_or(true);
    scene_mode && first_terrain_entity(world).is_some()
}

fn first_terrain_entity(world: &World) -> Option<Entity> {
    // `&World` can't build cached queries, so walk archetypes directly.
    let terrain_id = world
        .components()
        .get_id(std::any::TypeId::of::<TerrainData>())?;
    for archetype in world.archetypes().iter() {
        if archetype.contains(terrain_id) {
            if let Some(entity) = archetype.entities().iter().next() {
                return Some(entity.id());
            }
        }
    }
    None
}

/// Activator for the terrain toolbar buttons. If the tool is already active,
/// toggles back to Select; otherwise selects the first terrain entity, switches
/// the inspector tab, and activates the brush tool.
fn activate_terrain_tool(world: &mut World, tab: TerrainInspectorTab, tool: ActiveTool) {
    let cur = world
        .get_resource::<ActiveTool>()
        .copied()
        .unwrap_or_default();
    if cur == tool {
        world.insert_resource(ActiveTool::Select);
        return;
    }

    if let Some(entity) = first_terrain_entity(world) {
        let already_selected = world
            .get_resource::<EditorSelection>()
            .and_then(|s| s.get()) == Some(entity);
        if !already_selected {
            if let Some(sel) = world.get_resource::<EditorSelection>() {
                sel.set(Some(entity));
            }
        }
    }
    world.insert_resource(tab);
    world.insert_resource(tool);
}

fn terrain_data_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "terrain_data",
        display_name: "Terrain",
        icon: "mountains",
        category: "component",
        has_fn: |world, entity| world.get::<TerrainData>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
    }
}

renzora::add!(TerrainEditorPlugin, Editor);
