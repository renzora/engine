//! Keep the separate render process's scene identical to the editor's.
//!
//! The render host loads the scene from disk on startup. The moment it is
//! connected, this sends the whole scene as the editor has it, unsaved edits
//! included, and from then on a patch whenever anything in it changes. The host
//! applies patches onto the entities already there (see
//! `renzora_engine::scene_io::apply_entity_snapshot`), so an edit never reloads
//! the scene.
//!
//! # Noticing changes without asking every tool
//!
//! Nothing announces an edit. The editor has on the order of a hundred ways to
//! change the scene (gizmo, every inspector field, the hierarchy, terrain,
//! importers, scripts), and replicating *operations* would mean finding and
//! serializing every one, forever. This replicates *state* instead: it notices
//! that an entity changed, however it changed. Two mechanisms, because neither
//! alone is enough:
//!
//! - **Change ticks** catch every mutation and every component added.
//! - **Archetype identity** catches removals, which leave no tick: an entity's
//!   archetype is its component set, so diffing the old and new one names what
//!   went away.
//!
//! What counts as the scene is exactly what a save writes
//! (`is_document_entity`), so editor chrome never reaches the host.

use bevy::ecs::archetype::ArchetypeId;
use bevy::ecs::change_detection::Tick;
use bevy::ecs::component::ComponentId;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use renzora_engine::scene_io::{is_document_entity, snapshot_entities};
use renzora_render_link::SceneMessage;

use crate::render_host::RenderHostClient;

/// How often the scene is scanned. A gizmo drag writes every frame; scanning
/// at 30 Hz coalesces that into one patch per scan and keeps the exclusive
/// scan off most frames.
const SCAN_HZ: f64 = 30.0;

#[derive(Resource, Default)]
pub struct RenderSync {
    /// Whether the current host has had its full scene.
    reset_sent: bool,
    /// Each document entity's archetype when last sent.
    tracked: HashMap<Entity, ArchetypeId>,
    /// The tick of the last scan; changes after it are new.
    since: Option<Tick>,
    next_scan: f64,
}

/// Components that change on their own every frame, or are rebuilt on the
/// other side, and so say nothing about an edit.
fn ignored(world: &World) -> HashSet<ComponentId> {
    use bevy::camera::primitives::Aabb;
    use bevy::camera::visibility::{InheritedVisibility, ViewVisibility};
    use bevy::transform::components::TransformTreeChanged;
    [
        world.components().component_id::<GlobalTransform>(),
        world.components().component_id::<TransformTreeChanged>(),
        world.components().component_id::<ViewVisibility>(),
        world.components().component_id::<InheritedVisibility>(),
        world.components().component_id::<Aabb>(),
        world.components().component_id::<Children>(),
        world.components().component_id::<bevy::pbr::PreviousGlobalTransform>(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Every entity that is part of the scene document.
fn document_entities(world: &mut World) -> Vec<Entity> {
    let mut q = world.query_filtered::<Entity, (With<Name>, Without<Node>)>();
    let candidates: Vec<Entity> = q.iter(world).collect();
    candidates
        .into_iter()
        .filter(|&e| is_document_entity(world, e))
        .collect()
}

pub fn sync_scene_to_render_host(world: &mut World) {
    let connected = world
        .get_resource::<RenderHostClient>()
        .is_some_and(|c| c.is_connected());
    if !connected {
        // A host that went away takes its copy of the scene with it; the next
        // one starts from a full scene again.
        let mut sync = world.resource_mut::<RenderSync>();
        if sync.reset_sent {
            *sync = RenderSync::default();
        }
        return;
    }

    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    {
        let mut sync = world.resource_mut::<RenderSync>();
        if sync.reset_sent && now < sync.next_scan {
            return;
        }
        sync.next_scan = now + 1.0 / SCAN_HZ;
    }

    let this_run = world.change_tick();
    let entities = document_entities(world);

    if !world.resource::<RenderSync>().reset_sent {
        let bsn = snapshot_entities(world, &entities).unwrap_or_default();
        let tracked = entities
            .iter()
            .filter_map(|&e| world.get_entity(e).ok().map(|r| (e, r.archetype().id())))
            .collect();
        world.resource::<RenderHostClient>().send(&SceneMessage::Reset { bsn });
        let mut sync = world.resource_mut::<RenderSync>();
        sync.tracked = tracked;
        sync.since = Some(this_run);
        sync.reset_sent = true;
        info!("[render-host] sent the scene ({} entities)", entities.len());
        return;
    }

    let since = world.resource::<RenderSync>().since.unwrap_or(this_run);
    let ignored = ignored(world);
    let registry = world.resource::<AppTypeRegistry>().clone();

    let mut changed: Vec<Entity> = Vec::new();
    let mut removed: Vec<(u64, Vec<String>)> = Vec::new();
    let mut seen: HashSet<Entity> = HashSet::default();
    {
        let sync = world.resource::<RenderSync>();
        for &entity in &entities {
            seen.insert(entity);
            let Ok(entity_ref) = world.get_entity(entity) else { continue };
            let archetype = entity_ref.archetype();
            match sync.tracked.get(&entity) {
                None => changed.push(entity),
                Some(&old) if old != archetype.id() => {
                    changed.push(entity);
                    // Components the old archetype had and the new one lacks.
                    let Some(old_archetype) = world.archetypes().get(old) else { continue };
                    let gone: Vec<String> = old_archetype
                        .components()
                        .iter()
                        .filter(|id| !archetype.contains(**id))
                        .filter_map(|id| {
                            let type_id = world.components().get_info(*id)?.type_id()?;
                            let registry = registry.read();
                            Some(registry.get(type_id)?.type_info().type_path().to_string())
                        })
                        .collect();
                    if !gone.is_empty() {
                        removed.push((entity.to_bits(), gone));
                    }
                }
                Some(_) => {
                    let edited = archetype.components().iter().any(|id| {
                        !ignored.contains(id)
                            && entity_ref
                                .get_change_ticks_by_id(*id)
                                .is_some_and(|t| t.is_changed(since, this_run))
                    });
                    if edited {
                        changed.push(entity);
                    }
                }
            }
        }
    }

    let despawned: Vec<u64> = world
        .resource::<RenderSync>()
        .tracked
        .keys()
        .filter(|e| !seen.contains(*e))
        .map(|e| e.to_bits())
        .collect();

    {
        let new_tracked: HashMap<Entity, ArchetypeId> = entities
            .iter()
            .filter_map(|&e| world.get_entity(e).ok().map(|r| (e, r.archetype().id())))
            .collect();
        let mut sync = world.resource_mut::<RenderSync>();
        sync.tracked = new_tracked;
        sync.since = Some(this_run);
    }

    if changed.is_empty() && removed.is_empty() && despawned.is_empty() {
        return;
    }
    let bsn = if changed.is_empty() {
        String::new()
    } else {
        snapshot_entities(world, &changed).unwrap_or_default()
    };
    world
        .resource::<RenderHostClient>()
        .send(&SceneMessage::Patch { bsn, removed, despawned });
}
