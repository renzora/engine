//! The editor half of world partition: streaming cells around the viewport
//! camera while editing, keeping their edits, the World Partition section of
//! the Inspector, and "Partition into cells" on the Hierarchy's right-click
//! menu.
//!
//! The runtime half (`renzora_engine::world_partition`) owns the cell grid,
//! the bounds reader and the partition action; a game streams cells as plain
//! streamed instances and needs nothing here. What the editor adds:
//!
//! - **A driver for edit mode** ([`drive_partition_cells_in_editor`]). The
//!   engine's instance driver runs only while a game is running, because in
//!   edit mode ordinary instances stay expanded to be authored. A partition is
//!   exactly the case where that cannot work, so its cells stream here instead,
//!   from the viewport camera, through the same async stream machinery.
//! - **Edits survive a cell streaming out.** A loaded cell's contents exist
//!   only in the world until written. Before a cell unloads, and before a tab
//!   switch despawns the scene, loaded cells are written back to their files
//!   whenever the scene has unsaved changes. Saving the scene writes them too.
//! - **Pins and "load all"**, so a designer can hold an area resident while
//!   working, or see the whole map when it really is needed.

use bevy::prelude::*;
use renzora_log_api::*;
use renzora_scene_api::{PartitionCell, PartitionCellPinned, SceneInstance, WorldPartition};
use renzora_project_api::CurrentProject;
use renzora_engine::scene_stream::{self, SceneStreams};
use renzora_engine::world_partition::{
    self as wp, cell_is_loaded, ground_distance, partition_settings,
};

#[cfg(not(target_arch = "wasm32"))]
mod hlod;
mod inspector;
mod map;

/// At most this many cells stream in at once. A camera that jumps across the
/// map (or "load all" on a city) would otherwise start hundreds of parses in
/// one frame; nearest cells go first, so what is in front of the camera
/// arrives before what is behind it.
const MAX_CONCURRENT_CELL_LOADS: usize = 4;

/// Whether the active scene tab has unsaved changes. Loaded cells are written
/// back before they despawn only then: a clean scene's cells already match
/// their files, and rewriting them would only churn the disk.
fn scene_has_unsaved_changes(world: &World) -> bool {
    world
        .get_resource::<renzora_ui::DocumentTabState>()
        .and_then(|tabs| tabs.tabs.get(tabs.active_tab))
        .is_some_and(|tab| tab.is_modified)
}

fn mark_scene_modified(world: &mut World) {
    if let Some(mut tabs) = world.get_resource_mut::<renzora_ui::DocumentTabState>() {
        let active = tabs.active_tab;
        if let Some(tab) = tabs.tabs.get_mut(active) {
            tab.is_modified = true;
        }
    }
}

/// Called right before the scene crate despawns the scene for a tab switch:
/// loaded cells hold edits that exist nowhere else yet.
///
/// Unconditional, unlike the write-back when a cell streams out. By the time
/// a tab switch gets here the active tab may already be the new one, so its
/// unsaved flag would answer for the wrong scene; rewriting a clean cell costs
/// a file write, and skipping a dirty one would lose work.
pub(crate) fn before_scene_despawn(world: &mut World) {
    wp::write_back_loaded_cells(world);
}

/// Stream partition cells in and out around the viewport camera while
/// editing. Runs only while [`wp::partition_streams_in_editor_condition`]
/// holds, so it costs nothing in a scene without a partition or in Play.
pub(crate) fn drive_partition_cells_in_editor(world: &mut World) {
    let Some(settings) = partition_settings(world) else {
        return;
    };
    let Some(root) = world.get_resource::<CurrentProject>().map(|p| p.path.clone()) else {
        return;
    };
    // No camera means no viewpoint to stream from: leave residency alone
    // rather than unload everything (the camera is respawned on a project
    // reopen, for one).
    let camera = wp::editor_camera_pos(world);
    let load_radius = settings.load_radius.max(0.0);
    let unload_radius = settings.effective_unload_radius();

    struct Cell {
        entity: Entity,
        source: String,
        distance: f32,
        pinned: bool,
        loaded: bool,
        streaming: bool,
    }
    let cells: Vec<Cell> = {
        let mut q = world.query_filtered::<
            (Entity, &SceneInstance, &GlobalTransform, Has<PartitionCellPinned>),
            With<PartitionCell>,
        >();
        let world: &World = world;
        let streams = world.resource::<SceneStreams>();
        q.iter(world)
            .map(|(entity, instance, transform, pinned)| Cell {
                entity,
                source: instance.source.clone(),
                distance: camera.map_or(f32::INFINITY, |c| ground_distance(c, transform.translation())),
                pinned,
                loaded: cell_is_loaded(world, entity),
                streaming: streams.has_stream_under(entity),
            })
            .collect()
    };

    let wants = |c: &Cell| settings.editor_load_all || c.pinned || c.distance <= load_radius;
    let should_unload = |c: &Cell| {
        camera.is_some() && !settings.editor_load_all && !c.pinned && c.distance > unload_radius
    };

    // Unload first: what leaves frees memory for what arrives.
    let write_back = scene_has_unsaved_changes(world);
    for cell in cells.iter().filter(|c| (c.loaded || c.streaming) && should_unload(c)) {
        scene_stream::cancel_streams_under(world, cell.entity);
        if cell.loaded {
            if write_back {
                wp::write_back_cell(world, cell.entity, &root.join(&cell.source));
            }
            let children: Vec<Entity> = world
                .get::<Children>(cell.entity)
                .map(|c| c.iter().collect())
                .unwrap_or_default();
            for child in children {
                if world.get_entity(child).is_ok() {
                    world.despawn(child);
                }
            }
        }
    }

    let in_flight = cells.iter().filter(|c| c.streaming).count();
    let mut candidates: Vec<&Cell> = cells
        .iter()
        .filter(|c| !c.loaded && !c.streaming && wants(c))
        .collect();
    candidates.sort_by(|a, b| a.distance.total_cmp(&b.distance));
    for cell in candidates
        .into_iter()
        .take(MAX_CONCURRENT_CELL_LOADS.saturating_sub(in_flight))
    {
        let path = root.join(&cell.source);
        // A cell with no file is one whose contents were all moved out or
        // deleted; there is nothing to load, and no reason to warn each frame.
        if path.exists() {
            scene_stream::start_scene_stream_under(world, &path, cell.entity);
        }
    }

    // The viewport's progress card while cells are arriving, so a camera
    // moved to a new district shows that its streets are on the way. Counted
    // from this frame's decisions: cells wanted but not yet resident.
    let wanted = cells.iter().filter(|c| wants(c)).count() as u32;
    let pending = cells.iter().filter(|c| wants(c) && !c.loaded).count() as u32;
    report_progress(world, wanted - pending, wanted);
}

fn report_progress(world: &mut World, done: u32, total: u32) {
    let next = (done < total).then(|| renzora_viewport_api::WorkItem {
        id: "world_partition",
        label: "Loading world cells".into(),
        done,
        total,
    });
    let Some(work) = world.get_resource::<renzora_viewport_api::ViewportWork>() else {
        return;
    };
    if work.differs("world_partition", next.as_ref()) {
        world
            .resource_mut::<renzora_viewport_api::ViewportWork>()
            .set("world_partition", next);
    }
}

/// Clear the progress card when the driver stops running (Play started, the
/// partition was removed), or it would be left up showing stale work.
pub(crate) fn clear_partition_progress(
    work: Option<ResMut<renzora_viewport_api::ViewportWork>>,
) {
    if let Some(mut work) = work {
        if work.get("world_partition").is_some() {
            work.set("world_partition", None);
        }
    }
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

/// Partition into cells, from the Hierarchy's right-click menu: the whole
/// selection when the right-clicked row is part of it, otherwise just that row
/// (as Delete does). Releases the editor's pins on the models moved, so the
/// cells they went into can free them by streaming out, and marks the scene
/// modified: the host scene now lists the cells.
pub(crate) fn partition_from_hierarchy(world: &mut World, clicked: Entity) {
    let selected: Vec<Entity> = match world.get_resource::<renzora_editor_api::EditorSelection>() {
        Some(s) if s.is_selected(clicked) => s.get_all(),
        _ => vec![clicked],
    };
    match wp::partition_entities(world, &selected) {
        Ok(report) => {
            let tab = world
                .get_resource::<renzora_ui::DocumentTabState>()
                .and_then(|t| t.active_tab_id());
            if let (Some(tab), Some(mut cache)) =
                (tab, world.get_resource_mut::<crate::tab_asset_cache::TabAssetCache>())
            {
                cache.release_paths(tab, &report.moved_models);
                cache.drop_tab_live(tab);
            }
            if let Some(selection) = world.get_resource::<renzora_editor_api::EditorSelection>() {
                selection.set(None);
            }
            mark_scene_modified(world);
        }
        Err(e) => console_warn("World Partition", e),
    }
}

/// Pin or unpin a cell so it stays loaded whatever the camera does.
pub(crate) fn toggle_cell_pin(world: &mut World, coord: IVec2) {
    let cell = {
        let mut q = world.query::<(Entity, &PartitionCell)>();
        q.iter(world).find(|(_, c)| c.coord() == coord).map(|(e, _)| e)
    };
    let Some(cell) = cell else { return };
    if world.get::<PartitionCellPinned>(cell).is_some() {
        world.entity_mut(cell).remove::<PartitionCellPinned>();
    } else {
        world.entity_mut(cell).insert(PartitionCellPinned);
    }
}

/// Build every cell's distant stand-in, in the background.
pub(crate) fn build_distant_lods(world: &mut World) {
    #[cfg(not(target_arch = "wasm32"))]
    hlod::start_hlod_bake(world);
    #[cfg(target_arch = "wasm32")]
    console_warn("World Partition", "Distant LODs are built in the desktop editor");
}

pub(crate) fn unpin_all_cells(world: &mut World) {
    let pinned: Vec<Entity> = {
        let mut q = world.query_filtered::<Entity, With<PartitionCellPinned>>();
        q.iter(world).collect()
    };
    for e in pinned {
        world.entity_mut(e).remove::<PartitionCellPinned>();
    }
}

// ---------------------------------------------------------------------------
// Inspector
// ---------------------------------------------------------------------------

/// The section's header. Its body is the native drawer in [`inspector`],
/// which replaces declarative fields entirely, so none are listed here.
pub(crate) fn world_partition_entry() -> renzora_editor_framework::InspectorEntry {
    use renzora_editor_framework::InspectorEntry;

    InspectorEntry {
        type_id: "world_partition",
        display_name: "World Partition",
        icon: "grid-four",
        category: "object",
        has_fn: |world, entity| world.get::<WorldPartition>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(WorldPartition::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<WorldPartition>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
    }
}

/// Whether "Partition into cells" is offered for a right-clicked row: not on
/// a cell or the World Partition entity itself, which are the partition, not
/// content for it.
fn can_partition(world: &World, entity: Entity) -> bool {
    world.get::<PartitionCell>(entity).is_none() && world.get::<WorldPartition>(entity).is_none()
}

/// Wire the editor half into the scene plugin.
pub(crate) fn register(app: &mut App) {
    use renzora_project_api::SplashState;
    use renzora_shell_api::{EntityAction, RegisterEntityAction};
    use renzora_editor_framework::AppEditorExt;

    app.register_inspector(world_partition_entry());
    app.register_entity_action(
        EntityAction::new("partition_into_cells", partition_from_hierarchy)
            .label("hierarchy.action.partition_into_cells", "Partition into cells")
            .icon("squares-four")
            .applies_to(can_partition),
    );
    #[cfg(not(target_arch = "wasm32"))]
    app.init_resource::<hlod::HlodBake>().add_systems(
        Update,
        hlod::poll_hlod_bake
            .run_if(|b: Res<hlod::HlodBake>| b.running())
            .run_if(in_state(SplashState::Editor)),
    );
    app.add_systems(
        Update,
        (
            drive_partition_cells_in_editor
                .run_if(wp::partition_streams_in_editor_condition),
            clear_partition_progress
                .run_if(not(wp::partition_streams_in_editor_condition)),
        )
            .run_if(in_state(SplashState::Editor)),
    );
    inspector::register(app);
}
