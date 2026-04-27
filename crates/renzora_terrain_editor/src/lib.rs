//! Terrain Editor — sculpting, painting, and brush gizmo systems.

mod brush_layer_paint;
mod panel;
mod spline_gizmos;
mod systems;
mod terrain_inspector;
mod terrain_layers_ui;
mod tool_options;
mod tool_panel;

use bevy::prelude::*;
use egui_phosphor::regular;
use renzora_editor::{
    ActiveTool, AppEditorExt, EditorSelection, EntityPreset, InspectorEntry, ToolEntry,
    ToolOptionsRegistry, ToolSection,
};
use renzora_spline::SplinePath;
use renzora_terrain::data::TerrainData;

use terrain_inspector::{
    render_terrain_inspector, sync_active_tool_system, TerrainInspectorTab,
};

#[derive(Default)]
pub struct TerrainEditorPlugin;

impl Plugin for TerrainEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] TerrainEditorPlugin");
        app.register_panel(panel::TerrainToolsPanel::new())
            .register_inspector(terrain_data_entry())
            .register_inspector(terrain_layers_ui::terrain_layers_entry())
            .init_resource::<ToolOptionsRegistry>()
            .init_resource::<TerrainInspectorTab>()
            .init_resource::<terrain_layers_ui::ActiveBrushLayer>();

        // Register context-sensitive viewport-header options for brush tools.
        {
            let mut reg = app.world_mut().resource_mut::<ToolOptionsRegistry>();
            reg.register(ActiveTool::TerrainSculpt, tool_options::draw_sculpt_options);
            reg.register(ActiveTool::TerrainPaint,  tool_options::draw_paint_options);
        }

        // Terrain toolbar buttons — visible whenever a terrain exists in the
        // scene (even if not currently selected). Clicking selects the terrain,
        // switches the inspector tab, and activates the brush tool. Clicking
        // the active button again reverts to Select.
        app.register_tool(
            ToolEntry::new(
                "builtin.terrain_sculpt",
                regular::MOUNTAINS,
                "Sculpt Terrain",
                ToolSection::Terrain,
            )
            .order(0)
            .visible_if(terrain_exists_in_scene)
            .active_if(|w| {
                w.get_resource::<ActiveTool>()
                    .copied()
                    .map_or(false, |t| t == ActiveTool::TerrainSculpt)
            })
            .on_activate(|w| activate_terrain_tool(w, TerrainInspectorTab::Sculpt, ActiveTool::TerrainSculpt)),
        );
        app.register_tool(
            ToolEntry::new(
                "builtin.terrain_paint",
                regular::PAINT_BRUSH,
                "Paint Terrain Layers",
                ToolSection::Terrain,
            )
            .order(1)
            .visible_if(terrain_exists_in_scene)
            .active_if(|w| {
                w.get_resource::<ActiveTool>()
                    .copied()
                    .map_or(false, |t| t == ActiveTool::TerrainPaint)
            })
            .on_activate(|w| activate_terrain_tool(w, TerrainInspectorTab::Paint, ActiveTool::TerrainPaint)),
        );
        app.register_tool(
            ToolEntry::new(
                "builtin.foliage_paint",
                regular::TREE,
                "Paint Foliage",
                ToolSection::Terrain,
            )
            .order(2)
            .visible_if(terrain_exists_in_scene)
            .active_if(|w| {
                w.get_resource::<ActiveTool>()
                    .copied()
                    .map_or(false, |t| t == ActiveTool::FoliagePaint)
            })
            .on_activate(|w| activate_terrain_tool(w, TerrainInspectorTab::Foliage, ActiveTool::FoliagePaint)),
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
            )
            // Keep the layer preview cache in sync so the terrain inspector
            // always shows current layer state.
            .add_systems(
                Update,
                systems::sync_layer_preview_system
                    .run_if(renzora::core::not_in_play_mode),
            )
            // Spline gizmos — always drawn in the editor (not in play mode).
            .add_systems(
                Update,
                spline_gizmos::draw_spline_gizmos_system
                    .run_if(renzora::core::not_in_play_mode),
            );

        app.register_entity_preset(EntityPreset {
            id: "spline",
            display_name: "Spline",
            icon: regular::PATH,
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
    move |tool: Option<Res<ActiveTool>>| {
        tool.map_or(false, |t| *t == expected)
    }
}

/// Toolbar visibility predicate: true if any entity in the world has `TerrainData`.
fn terrain_exists_in_scene(world: &World) -> bool {
    first_terrain_entity(world).is_some()
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
    let cur = world.get_resource::<ActiveTool>().copied().unwrap_or_default();
    if cur == tool {
        world.insert_resource(ActiveTool::Select);
        return;
    }

    if let Some(entity) = first_terrain_entity(world) {
        let already_selected = world
            .get_resource::<EditorSelection>()
            .and_then(|s| s.get())
            .map_or(false, |sel| sel == entity);
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
        icon: regular::MOUNTAINS,
        category: "component",
        has_fn: |world, entity| world.get::<TerrainData>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
        custom_ui_fn: Some(render_terrain_inspector),
    }
}


