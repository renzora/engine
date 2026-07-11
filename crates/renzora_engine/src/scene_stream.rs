//! Streaming scene loader — off-thread parse + incremental, time-budgeted spawn.
//!
//! [`scene_io::load_scene`] deserializes and spawns a whole scene inside one
//! frame, which is correct behind the editor/boot loading screens but hitches
//! a *running* game for the full parse+spawn cost when a script calls
//! `load_scene("level2")`. This module splits that work:
//!
//! 1. **Parse** — file read (Vfs/rpak first, disk fallback) and BSN
//!    deserialization run on the [`AsyncComputeTaskPool`]. `DynamicScene` is
//!    `Send` (reflected values are `Send + Sync`) and the `AppTypeRegistry` is
//!    an `Arc`, so the whole stage moves off the main thread.
//! 2. **Spawn** — all scene entities are allocated as empties in one cheap
//!    pass (so entity cross-references always remap to a live target), then
//!    components are applied a few entities at a time under a per-frame time
//!    budget. The `Added<T>` rehydrate systems (meshes, terrain, physics …)
//!    pick entities up as they materialize, exactly as they do after a
//!    synchronous load.
//!
//! Streams also power world streaming: a stream started with
//! [`start_scene_stream_under`] reparents the loaded roots under an existing
//! entity when it finishes (used by streamed `SceneInstance` expansion), and
//! several streams can be in flight at once — the frame budget is shared.

use bevy::ecs::entity::EntityHashMap;
use bevy::prelude::*;
use bevy::tasks::{block_on, poll_once, AsyncComputeTaskPool, Task};
use renzora::console_log::*;
use renzora_bsn::bsn::{BsnSerializer, SceneSerializer};
use renzora_bsn::DynamicScene;
use std::path::Path;

use crate::scene_io::{
    self, SceneLoadFailed, SceneLoadPhase, SceneLoadState, SceneLoaded,
    SceneLoadedWithSkippedTypes,
};

/// Per-frame budget for applying streamed components, in milliseconds.
///
/// Component application is reflection-heavy (registry lookup + dynamic apply
/// per component), so the budget is on wall time, not entity count — one
/// terrain chunk with a fat height buffer costs orders of magnitude more than
/// an empty group node. 3 ms leaves headroom inside a 60 fps frame even with
/// rehydration systems churning behind the spawner.
const SPAWN_BUDGET_MS: f32 = 3.0;

/// Off-thread stage output. `None` means the file was empty (a valid,
/// entity-less scene — matches the sync loader's empty-scene early-out).
struct ParsedScene {
    scene: DynamicScene,
    skipped_types: Vec<String>,
    pruned_orphans: usize,
    pruned_ui: usize,
}

enum StreamStage {
    /// Waiting on the off-thread read+parse task.
    Parsing(Task<Result<Option<ParsedScene>, String>>),
    /// Applying components onto pre-allocated entities, `cursor` entities done.
    Spawning {
        scene: DynamicScene,
        entity_map: EntityHashMap<Entity>,
        cursor: usize,
    },
}

/// One in-flight scene stream.
pub struct SceneStream {
    path_str: String,
    stage: StreamStage,
    /// Reparent the loaded scene's roots under this entity on completion
    /// (streamed `SceneInstance` expansion). `None` for a full scene load.
    spawn_parent: Option<Entity>,
    /// The main scene load drives `SceneLoadState`, fires `SceneLoaded`, and
    /// expands nested scene instances when done; instance streams don't.
    is_main: bool,
}

/// All in-flight scene streams. At most one `is_main` stream exists at a time
/// ([`start_scene_stream`] cancels the previous one); instance streams stack.
#[derive(Resource, Default)]
pub struct SceneStreams {
    streams: Vec<SceneStream>,
}

/// Read-only description of one in-flight stream, for debug UI.
pub struct SceneStreamSummary {
    pub path: String,
    pub is_main: bool,
    /// `"parsing"` or `"spawning N/M"`.
    pub stage: String,
}

impl SceneStreams {
    /// True while the main scene is still parsing/spawning. Scripts and UI can
    /// combine this with `SceneLoadState.progress` for load feedback.
    pub fn main_in_flight(&self) -> bool {
        self.streams.iter().any(|s| s.is_main)
    }

    /// True if `parent` already has a stream expanding under it — used by the
    /// world-streaming driver to avoid double-starting an expansion.
    pub fn has_stream_under(&self, parent: Entity) -> bool {
        self.streams.iter().any(|s| s.spawn_parent == Some(parent))
    }

    /// Debug-UI view of every in-flight stream (the Streaming panel).
    pub fn summaries(&self) -> Vec<SceneStreamSummary> {
        self.streams
            .iter()
            .map(|s| SceneStreamSummary {
                path: s.path_str.clone(),
                is_main: s.is_main,
                stage: match &s.stage {
                    StreamStage::Parsing(_) => "parsing".to_string(),
                    StreamStage::Spawning { cursor, scene, .. } => {
                        format!("spawning {}/{}", cursor, scene.entities.len())
                    }
                },
            })
            .collect()
    }
}

/// Start streaming the scene at `path` as the new main scene. Any previous
/// main stream is cancelled (its partially-spawned entities despawned) — the
/// caller is expected to have despawned the outgoing scene already, as
/// `process_pending_scene_loads` does.
pub fn start_scene_stream(world: &mut World, path: &Path) {
    cancel_main_stream(world);
    start_stream_inner(world, path, None, true);
}

/// Start streaming the scene at `path` under `parent` — the roots of the
/// loaded scene are reparented beneath it on completion. Used by streamed
/// `SceneInstance` expansion; does not touch `SceneLoadState`.
pub fn start_scene_stream_under(world: &mut World, path: &Path, parent: Entity) {
    start_stream_inner(world, path, Some(parent), false);
}

fn start_stream_inner(world: &mut World, path: &Path, spawn_parent: Option<Entity>, is_main: bool) {
    let path_str = path.to_string_lossy().to_string();
    console_info(
        "Scene",
        format!("=== Streaming scene from {} ===", path_str),
    );

    if is_main {
        if let Some(mut state) = world.get_resource_mut::<SceneLoadState>() {
            state.phase = SceneLoadPhase::Loading;
            state.current_path = Some(path_str.clone());
            state.progress = 0.0;
        }
    }

    let vfs = world.get_resource::<crate::Vfs>().cloned();
    let registry = world.resource::<AppTypeRegistry>().clone();
    let task_path = path.to_path_buf();

    let task = AsyncComputeTaskPool::get().spawn(async move {
        // Vfs (rpak archive) first — normalize to a forward-slash archive key.
        let content = vfs.as_ref().and_then(|vfs| {
            let key = task_path.to_string_lossy().replace('\\', "/");
            let key = key.strip_prefix("./").unwrap_or(&key).to_string();
            vfs.read_string(&key)
        });
        let content = match content {
            Some(c) => c,
            None => {
                if !task_path.exists() {
                    return Err(format!(
                        "Scene file does not exist: {}",
                        task_path.display()
                    ));
                }
                std::fs::read_to_string(&task_path)
                    .map_err(|e| format!("Failed to read scene file: {e}"))?
            }
        };

        let trimmed = content.trim();
        if trimmed.is_empty() || trimmed == "(entities: {}, resources: {})" {
            return Ok(None);
        }

        let registry = registry.read();
        let (mut scene, skipped_types) = BsnSerializer
            .deserialize_lossy(&content, &registry)
            .map_err(|e| e.to_string())?;
        // Pruning is pure scene-IR surgery — run it off-thread too.
        let pruned_orphans = scene_io::prune_orphaned_entities(&mut scene);
        let pruned_ui = scene_io::prune_leaked_ui(&mut scene);
        Ok(Some(ParsedScene {
            scene,
            skipped_types,
            pruned_orphans,
            pruned_ui,
        }))
    });

    world.resource_mut::<SceneStreams>().streams.push(SceneStream {
        path_str,
        stage: StreamStage::Parsing(task),
        spawn_parent,
        is_main,
    });
}

/// Cancel the in-flight main stream, if any, despawning whatever it had
/// already spawned. Called before starting a replacement load and by the
/// scene-clear path so half-streamed scenes never leak unnamed empties (the
/// standard clear pass only matches `With<Name>`, which pre-allocated empties
/// don't have yet).
pub fn cancel_main_stream(world: &mut World) {
    let cancelled: Vec<SceneStream> = {
        let mut streams = world.resource_mut::<SceneStreams>();
        let (main, rest): (Vec<_>, Vec<_>) =
            streams.streams.drain(..).partition(|s| s.is_main);
        streams.streams = rest;
        main
    };
    for stream in cancelled {
        despawn_stream_entities(world, stream);
    }
}

/// Despawn every entity a cancelled stream had allocated/spawned. Dropping the
/// parse task cancels it (async_task semantics), so `Parsing` needs no cleanup.
fn despawn_stream_entities(world: &mut World, stream: SceneStream) {
    if let StreamStage::Spawning { entity_map, .. } = stream.stage {
        for &entity in entity_map.values() {
            if world.get_entity(entity).is_ok() {
                world.despawn(entity);
            }
        }
    }
    console_info(
        "Scene",
        format!("Cancelled scene stream: {}", stream.path_str),
    );
}

/// Drive all in-flight scene streams: poll parse tasks, then apply components
/// under the shared frame budget. Exclusive — component application needs
/// `&mut World` for reflection + entity remapping.
pub fn drive_scene_streams(world: &mut World) {
    if world.resource::<SceneStreams>().streams.is_empty() {
        return;
    }
    // Take the streams out so we can freely pass `world` around; unfinished
    // ones go back at the end. New streams pushed while we work (a finishing
    // main stream expanding instances) are preserved by re-appending.
    let mut streams = std::mem::take(&mut world.resource_mut::<SceneStreams>().streams);
    let budget_start = std::time::Instant::now();
    let mut kept: Vec<SceneStream> = Vec::new();

    for mut stream in streams.drain(..) {
        // A stream expanding under a despawned parent has lost its anchor —
        // the world moved on (scene swap, instance despawn). Drop it.
        if let Some(parent) = stream.spawn_parent {
            if world.get_entity(parent).is_err() {
                despawn_stream_entities(world, stream);
                continue;
            }
        }

        match &mut stream.stage {
            StreamStage::Parsing(task) => {
                let Some(result) = block_on(poll_once(task)) else {
                    kept.push(stream);
                    continue;
                };
                match result {
                    Ok(Some(parsed)) => {
                        report_parse(world, &stream, &parsed);
                        let scene = parsed.scene;
                        let mut entity_map = EntityHashMap::default();
                        scene.allocate_entities(world, &mut entity_map);
                        stream.stage = StreamStage::Spawning {
                            scene,
                            entity_map,
                            cursor: 0,
                        };
                        if stream.is_main {
                            if let Some(mut state) =
                                world.get_resource_mut::<SceneLoadState>()
                            {
                                state.progress = 0.1;
                            }
                        }
                        kept.push(stream);
                    }
                    Ok(None) => {
                        console_info(
                            "Scene",
                            format!("Scene is empty: {}", stream.path_str),
                        );
                        finish_stream(world, stream);
                    }
                    Err(e) => {
                        fail_stream(world, &stream, e);
                    }
                }
            }
            StreamStage::Spawning { .. } => match spawn_within_budget(world, &mut stream, budget_start) {
                SpawnOutcome::InProgress => kept.push(stream),
                SpawnOutcome::Done => finish_stream(world, stream),
                SpawnOutcome::Cancelled => despawn_stream_entities(world, stream),
                SpawnOutcome::Failed(e) => {
                    // Unlike a cancel, a reflection failure leaves an
                    // inconsistent half-scene — report, then take it all down.
                    fail_stream(world, &stream, e);
                    despawn_stream_entities(world, stream);
                }
            },
        }
    }

    world.resource_mut::<SceneStreams>().streams.append(&mut kept);
}

enum SpawnOutcome {
    InProgress,
    Done,
    /// The scene was cleared beneath us (a mapped entity vanished) — despawn
    /// the remainder quietly; whoever cleared the scene owns the world now.
    Cancelled,
    Failed(String),
}

fn spawn_within_budget(
    world: &mut World,
    stream: &mut SceneStream,
    budget_start: std::time::Instant,
) -> SpawnOutcome {
    let registry = world.resource::<AppTypeRegistry>().clone();
    let StreamStage::Spawning {
        scene,
        entity_map,
        cursor,
    } = &mut stream.stage
    else {
        return SpawnOutcome::InProgress;
    };

    let total = scene.entities.len();
    while *cursor < total {
        if budget_start.elapsed().as_secs_f32() * 1000.0 > SPAWN_BUDGET_MS {
            break;
        }
        let scene_entity = scene.entities[*cursor].entity;
        let target = *entity_map
            .get(&scene_entity)
            .expect("allocate_entities mapped every scene entity");
        if world.get_entity(target).is_err() {
            return SpawnOutcome::Cancelled;
        }
        if let Err(e) = scene.write_entity_to_world(*cursor, world, entity_map, &registry) {
            return SpawnOutcome::Failed(e.to_string());
        }
        // Reflection inserts `ChildOf` without firing the hierarchy hooks that
        // maintain the parent's `Children` — re-insert to fire them, same as
        // the sync loader (which does this in a batch after write_to_world).
        if let Some(parent) = world.get::<ChildOf>(target).map(|c| c.parent()) {
            world.entity_mut(target).remove::<ChildOf>();
            world.entity_mut(target).insert(ChildOf(parent));
        }
        *cursor += 1;
    }

    if stream.is_main {
        if let Some(mut state) = world.get_resource_mut::<SceneLoadState>() {
            // 0.1 was parse; spawn fills the rest.
            state.progress = 0.1 + 0.9 * (*cursor as f32 / total.max(1) as f32);
        }
    }

    if *cursor >= total {
        SpawnOutcome::Done
    } else {
        SpawnOutcome::InProgress
    }
}

fn report_parse(world: &mut World, stream: &SceneStream, parsed: &ParsedScene) {
    if !parsed.skipped_types.is_empty() {
        for type_path in &parsed.skipped_types {
            warn!(
                "[scene] {} skipped unregistered type `{}`",
                stream.path_str, type_path
            );
        }
        world.trigger(SceneLoadedWithSkippedTypes {
            path: stream.path_str.clone(),
            skipped: parsed.skipped_types.clone(),
        });
    }
    if parsed.pruned_orphans > 0 {
        warn!(
            "[scene] {} pruned {} orphaned entities (leaked editor-chrome / missing parent)",
            stream.path_str, parsed.pruned_orphans
        );
    }
    if parsed.pruned_ui > 0 {
        warn!(
            "[scene] {} pruned {} leaked editor-UI entities (no UiCanvas ancestor)",
            stream.path_str, parsed.pruned_ui
        );
    }
}

fn finish_stream(world: &mut World, stream: SceneStream) {
    // Reparent the stream's roots under the requested parent (streamed
    // instance expansion). Roots = mapped entities that ended up with no
    // `ChildOf` — mirrors `expand_scene_instances`' root diff.
    if let Some(parent) = stream.spawn_parent {
        if let StreamStage::Spawning { ref entity_map, .. } = stream.stage {
            let roots: Vec<Entity> = entity_map
                .values()
                .filter(|&&e| {
                    world.get_entity(e).is_ok()
                        && world.get::<ChildOf>(e).is_none()
                        && e != parent
                })
                .copied()
                .collect();
            for root in roots {
                world.entity_mut(root).insert(ChildOf(parent));
            }
        }
    }

    // Nested (non-streamed) scene instances expand synchronously here,
    // matching the sync loader — prefab-sized content, not worth a stream.
    // Also required for *instance* streams: a streamed instance whose source
    // contains ordinary nested instances must expand those on arrival.
    // (Streamed nested instances stay collapsed — the distance driver owns
    // them; `expand_scene_instances` skips them while streaming is active.)
    scene_io::expand_scene_instances(world);

    if stream.is_main {
        if let Some(mut state) = world.get_resource_mut::<SceneLoadState>() {
            state.phase = SceneLoadPhase::Ready;
            state.progress = 1.0;
        }
        world.trigger(SceneLoaded {
            path: stream.path_str.clone(),
        });
    }

    console_success(
        "Scene",
        format!("=== Scene stream complete: {} ===", stream.path_str),
    );
}

/// Cancel any in-flight stream expanding under `parent`, despawning whatever
/// it had spawned so far. Used when a streamed instance leaves its unload
/// radius while its content is still loading.
pub fn cancel_streams_under(world: &mut World, parent: Entity) {
    let cancelled: Vec<SceneStream> = {
        let mut streams = world.resource_mut::<SceneStreams>();
        let (under, rest): (Vec<_>, Vec<_>) = streams
            .streams
            .drain(..)
            .partition(|s| s.spawn_parent == Some(parent));
        streams.streams = rest;
        under
    };
    for stream in cancelled {
        despawn_stream_entities(world, stream);
    }
}

/// Expand/collapse `streamed` scene instances by camera distance. Runs only
/// while world streaming is in effect (shipped game, or editor play/simulate
/// — see [`renzora::world_streaming_active`]); in editor edit mode streamed
/// instances stay expanded like ordinary ones so designers can work on them.
///
/// Expansion goes through the async stream machinery (parse off-thread,
/// spawn under budget), so crossing a load boundary never hitches the frame.
/// Collapse keeps the instance root — its transform and host overrides are
/// scene state — and drops only the expanded children.
pub fn drive_streamed_scene_instances(world: &mut World, mut was_active: Local<bool>) {
    if !renzora::world_streaming_active(world) {
        // Leaving play/simulate: anything play-time streaming collapsed must
        // come back for edit mode. One expansion on the transition edge —
        // not every frame, so a missing source file can't spam retries.
        if *was_active {
            scene_io::expand_scene_instances(world);
        }
        *was_active = false;
        return;
    }
    *was_active = true;
    let Some(camera_pos) = renzora::streaming_camera_pos(world) else {
        return;
    };
    let Some(project_root) = world
        .get_resource::<renzora::CurrentProject>()
        .map(|p| p.path.clone())
    else {
        return;
    };

    struct Decision {
        entity: Entity,
        action: StreamAction,
        source: String,
    }
    enum StreamAction {
        Load,
        Unload,
    }

    let mut decisions: Vec<Decision> = Vec::new();
    {
        let mut q = world.query::<(
            Entity,
            &renzora::SceneInstance,
            &GlobalTransform,
            Option<&Children>,
        )>();
        for (entity, instance, transform, children) in q.iter(world) {
            if !instance.streamed || instance.source.is_empty() {
                continue;
            }
            let expanded = children.is_some_and(|c| c.iter().count() > 0);
            let dist = camera_pos.distance(transform.translation());
            // Enforce hysteresis at evaluation time rather than trusting the
            // authored values — a zero/inverted unload radius would otherwise
            // load+unload every frame on the boundary.
            let load_radius = instance.load_radius.max(0.0);
            let unload_radius = instance.unload_radius.max(load_radius + 10.0);

            if dist <= load_radius && !expanded {
                decisions.push(Decision {
                    entity,
                    action: StreamAction::Load,
                    source: instance.source.clone(),
                });
            } else if dist > unload_radius && expanded {
                decisions.push(Decision {
                    entity,
                    action: StreamAction::Unload,
                    source: instance.source.clone(),
                });
            } else if dist > unload_radius {
                // Not expanded but may still be loading — cancel the stream.
                if world
                    .resource::<SceneStreams>()
                    .has_stream_under(entity)
                {
                    decisions.push(Decision {
                        entity,
                        action: StreamAction::Unload,
                        source: instance.source.clone(),
                    });
                }
            }
        }
    }

    for decision in decisions {
        match decision.action {
            StreamAction::Load => {
                if world
                    .resource::<SceneStreams>()
                    .has_stream_under(decision.entity)
                {
                    continue; // already on its way in
                }
                let path = project_root.join(&decision.source);
                start_scene_stream_under(world, &path, decision.entity);
            }
            StreamAction::Unload => {
                cancel_streams_under(world, decision.entity);
                let children: Vec<Entity> = world
                    .get::<Children>(decision.entity)
                    .map(|c| c.iter().collect())
                    .unwrap_or_default();
                for child in children {
                    if world.get_entity(child).is_ok() {
                        world.despawn(child);
                    }
                }
            }
        }
    }
}

fn fail_stream(world: &mut World, stream: &SceneStream, error: String) {
    error!("Failed to stream scene {}: {}", stream.path_str, error);
    console_error(
        "Scene",
        format!("Failed to stream scene {}: {}", stream.path_str, error),
    );
    if stream.is_main {
        if let Some(mut state) = world.get_resource_mut::<SceneLoadState>() {
            state.phase = SceneLoadPhase::Failed;
        }
        world.trigger(SceneLoadFailed {
            path: stream.path_str.clone(),
            error,
        });
    }
}
