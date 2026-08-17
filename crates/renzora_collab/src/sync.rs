//! Scene replication: what changed, how it is described, and how it is applied.
//!
//! ## Why state, not operations
//!
//! The obvious design for collaborative editing is to replicate *operations* —
//! "move entity 4 to (1,2,3)", "add a point light" — and replay them on the far
//! side. It was rejected here for a specific reason: this editor has on the
//! order of a hundred distinct mutation paths (the gizmo, every inspector field,
//! the hierarchy, terrain, tilemaps, the shape library, importers, scripts), and
//! operation replication means every one of them must be found, given a
//! serializable form, and kept in sync forever. Miss one and it silently does
//! not replicate; the failure is invisible until two people disagree about what
//! the scene contains.
//!
//! Replicating *state* inverts that. Nothing announces an edit — this module
//! notices that an entity changed, whoever changed it and however. A tool
//! written next year replicates with no knowledge that collaboration exists. The
//! price is that a change is described by its result rather than its intent, so
//! two people editing the same entity at once produce a last-writer-wins
//! flicker rather than a merge; [`crate::lease`] is the answer to that, and it
//! answers it by keeping them off the same entity in the first place.
//!
//! ## Noticing a change without asking every tool
//!
//! Two mechanisms, because neither alone is enough:
//!
//! - **Component change ticks.** Every component records the tick it was last
//!   written. An entity is dirty if any of its components changed since we last
//!   sent it. This catches every mutation and every component *addition*.
//! - **Archetype identity.** A component *removal* leaves no tick behind — the
//!   data is simply gone, with nothing to have a timestamp. But an entity's
//!   archetype **is** its component set, so a changed `ArchetypeId` means the set
//!   changed, and diffing the two archetypes says exactly which components went
//!   away. Storing one `ArchetypeId` per entity costs four bytes and replaces
//!   what would otherwise be a stored copy of every entity's component list.

use std::collections::{HashMap, HashSet};

use bevy::ecs::archetype::ArchetypeId;
use bevy::ecs::change_detection::Tick;
use bevy::ecs::component::ComponentId;
use bevy::prelude::*;
use renzora_engine::scene_io::{
    apply_entity_snapshot, has_hidden_ancestor, snapshot_entities, Descend, ExternalParents,
};

use crate::identity::{CollabId, CollabIds};
use crate::protocol::CollabMsg;
use crate::session::{CollabInbox, CollabSession};

/// How often the world is scanned for changes.
///
/// Not every frame: the scan is exclusive-world work, and a 60 Hz sync would put
/// it in front of the frame it is trying not to disturb. 15 Hz is under the
/// threshold where a collaborator's drag stops reading as continuous, and it
/// coalesces a gizmo drag's per-frame writes into one message instead of sixty.
const SCAN_HZ: f64 = 15.0;

/// Per-entity replication bookkeeping. See the module docs for what each field
/// is for.
struct Track {
    archetype: ArchetypeId,
    /// The tick at which this entity was last brought into agreement with the
    /// session — whether by sending it or by receiving it. Receiving counts:
    /// applying a peer's change marks the components changed locally, and
    /// without this the receiver would immediately "notice" that change and send
    /// it straight back.
    synced: Tick,
}

#[derive(Resource, Default)]
pub struct SyncTracker {
    tracked: HashMap<u64, Track>,
    next_scan: f64,
    /// Set while a peer's change is being written, so the change-detection scan
    /// can tell "the world changed because someone edited" from "the world
    /// changed because we just applied a message".
    pub applying: bool,
    /// Rolling counts for the panel.
    pub sent: u64,
    pub received: u64,
}

impl SyncTracker {
    pub fn reset(&mut self) {
        self.tracked.clear();
        self.sent = 0;
        self.received = 0;
    }
}

// ── What counts as part of the document ─────────────────────────────────────

/// Whether an entity is part of the scene rather than the editor around it.
///
/// This deliberately mirrors the save filter in `renzora_engine::scene_io`,
/// because the two questions are the same one: a session replicates the document
/// a save would write. Where it is stricter is the ancestor walk — editor chrome
/// tags only its root, so a bare `Without<HideInHierarchy>` would let every dock
/// tab and inspector row through, and a peer would receive the *other* editor's
/// user interface as scene content.
fn replicable(world: &World, entity: Entity) -> bool {
    let Ok(e) = world.get_entity(entity) else {
        return false;
    };
    if e.get::<Name>().is_none() {
        return false;
    }
    if e.contains::<renzora::core::HideInHierarchy>()
        || e.contains::<renzora::core::EditorCamera>()
        || e.contains::<renzora::Persistent>()
        || e.contains::<bevy::input::gamepad::Gamepad>()
    {
        return false;
    }
    if has_hidden_ancestor(world, entity) {
        return false;
    }
    // Descendants of a gltf instance or a nested scene are rebuilt from their
    // source on the far side, exactly as they are on load. Sending them would
    // both waste the wire and fight the rehydration that recreates them.
    let mut cursor = entity;
    while let Some(parent) = world.get::<ChildOf>(cursor).map(|c| c.parent()) {
        if world.get::<renzora::MeshInstanceData>(parent).is_some()
            || world.get::<renzora::SceneInstance>(parent).is_some()
        {
            return false;
        }
        cursor = parent;
    }
    true
}

/// Whether an entity's id belongs to *this* session.
///
/// A `CollabId` is a component, so it outlives the session that minted it: leave
/// a session and every entity still carries its old id. Trusting that on the
/// next connection would hand out ids from a slice of the space this peer no
/// longer owns, and two entities in different editors would end up answering to
/// the same name. Checking the registry instead makes a stale id self-heal — it
/// is simply re-minted — with no teardown pass to forget.
fn id_is_current(world: &World, entity: Entity) -> bool {
    match world.get::<CollabId>(entity) {
        None => false,
        Some(id) => world.resource::<CollabIds>().entity(id.0) == Some(entity),
    }
}

/// Every entity currently part of the document.
///
/// Builds a fresh `QueryState` per call, which is O(archetypes) rather than
/// free. Tolerated because this runs at [`SCAN_HZ`] and only while a session is
/// open; if it ever shows up in a profile, the fix is to cache the state rather
/// than to scan less often — the scan rate is a latency decision, not a budget.
fn collect_replicable(world: &mut World) -> Vec<Entity> {
    let candidates: Vec<Entity> = {
        let mut q = world.query_filtered::<Entity, With<Name>>();
        q.iter(world).collect()
    };
    candidates.into_iter().filter(|&e| replicable(world, e)).collect()
}

// ── Sending ─────────────────────────────────────────────────────────────────

/// Assign ids to new entities, find what changed, and send it.
pub fn scan_and_send(world: &mut World) {
    {
        let session = world.resource::<CollabSession>();
        if !session.is_active() {
            return;
        }
        // A guest with no control watches. It still tracks ids and ticks (so a
        // later grant does not resend the entire scene), it just says nothing.
        let elapsed = world.resource::<Time>().elapsed_secs_f64();
        let mut tracker = world.resource_mut::<SyncTracker>();
        if elapsed < tracker.next_scan {
            return;
        }
        tracker.next_scan = elapsed + 1.0 / SCAN_HZ;
    }

    let this_tick = world.change_tick();
    let entities = collect_replicable(world);

    // 1. Anything without a *valid* id is new to the session.
    let unassigned: Vec<Entity> = entities
        .iter()
        .copied()
        .filter(|&e| !id_is_current(world, e))
        .collect();
    if !unassigned.is_empty() {
        let mut minted: Vec<(Entity, CollabId)> = Vec::with_capacity(unassigned.len());
        {
            let mut ids = world.resource_mut::<CollabIds>();
            for e in unassigned {
                minted.push((e, ids.mint()));
            }
        }
        for (e, id) in &minted {
            world.entity_mut(*e).insert(*id);
            world.resource_mut::<CollabIds>().bind(id.0, *e);
        }
    }

    // 2. Dirty = changed since we last agreed, or a changed component set.
    let mut dirty: Vec<Entity> = Vec::new();
    let mut removals: Vec<(u64, Vec<String>)> = Vec::new();
    {
        let mut archetype_changes: Vec<(u64, ArchetypeId, ArchetypeId)> = Vec::new();
        {
            let tracker = world.resource::<SyncTracker>();
            for &entity in &entities {
                let Some(&id) = world.get::<CollabId>(entity) else {
                    continue;
                };
                let Ok(eref) = world.get_entity(entity) else {
                    continue;
                };
                let archetype = eref.archetype().id();
                match tracker.tracked.get(&id.0) {
                    None => dirty.push(entity),
                    Some(track) => {
                        if track.archetype != archetype {
                            archetype_changes.push((id.0, track.archetype, archetype));
                            dirty.push(entity);
                        } else if changed_since(&eref, track.synced, this_tick) {
                            dirty.push(entity);
                        }
                    }
                }
            }
        }
        for (id, before, after) in archetype_changes {
            let gone = removed_type_paths(world, before, after);
            if !gone.is_empty() {
                removals.push((id, gone));
            }
        }
    }

    // 3. Entities that were being tracked and are no longer there were despawned.
    let despawned: Vec<u64> = world.resource::<CollabIds>().dead(world);
    if !despawned.is_empty() {
        let mut ids = world.resource_mut::<CollabIds>();
        for id in &despawned {
            ids.forget(*id);
        }
        let mut tracker = world.resource_mut::<SyncTracker>();
        for id in &despawned {
            tracker.tracked.remove(id);
        }
    }

    if dirty.is_empty() && despawned.is_empty() {
        return;
    }

    let may_send = world.resource::<CollabSession>().may_edit();
    if !may_send {
        // Still record what we saw, or a later grant of control would present
        // the whole scene as "changed by me" and overwrite the host with it.
        mark_synced(world, &dirty, this_tick);
        return;
    }

    if !despawned.is_empty() {
        let session = world.resource::<CollabSession>();
        let msg = CollabMsg::EntityDespawn { ids: despawned };
        session.send_up(msg);
    }

    if !dirty.is_empty() {
        if let Some(msg) = build_upsert(world, &dirty, removals) {
            let session = world.resource::<CollabSession>();
            session.send_up(msg);
            world.resource_mut::<SyncTracker>().sent += 1;
        }
        mark_synced(world, &dirty, this_tick);
    }
}

/// Whether any of the entity's components were written since `since`.
fn changed_since(entity: &bevy::ecs::world::EntityRef, since: Tick, now: Tick) -> bool {
    entity.archetype().components().iter().any(|&component| {
        entity
            .get_change_ticks_by_id(component)
            .is_some_and(|ticks| ticks.is_changed(since, now))
    })
}

/// Type paths present in `before` but not in `after` — the components a removal
/// took away.
fn removed_type_paths(world: &World, before: ArchetypeId, after: ArchetypeId) -> Vec<String> {
    let archetypes = world.archetypes();
    let (Some(before), Some(after)) = (archetypes.get(before), archetypes.get(after)) else {
        return Vec::new();
    };
    let kept: HashSet<ComponentId> = after.components().iter().copied().collect();
    before
        .components()
        .iter()
        .copied()
        .filter(|c| !kept.contains(c))
        .filter_map(|c| world.components().get_info(c))
        .map(|info| info.name().to_string())
        .collect()
}

/// Describe `dirty` as one message.
fn build_upsert(
    world: &mut World,
    dirty: &[Entity],
    removals: Vec<(u64, Vec<String>)>,
) -> Option<CollabMsg> {
    // Exactly the changed entities — see `Descend::ExactSet` for why descending
    // would be wrong here.
    let bsn = snapshot_entities(world, dirty, Descend::ExactSet)?;

    // The id table has to cover more than the snapshot's own entities: a
    // `ChildOf` inside it names a parent that may not be in this batch, and the
    // receiver can only resolve that name through an id it recognises.
    let mut ids: Vec<(u64, u64)> = Vec::new();
    let mut seen: HashSet<u64> = HashSet::new();
    for &entity in dirty {
        push_id(world, entity, &mut ids, &mut seen);
        if let Some(parent) = world.get::<ChildOf>(entity).map(|c| c.parent()) {
            push_id(world, parent, &mut ids, &mut seen);
        }
    }
    Some(CollabMsg::EntityUpsert { bsn, ids, removed: removals })
}

fn push_id(world: &World, entity: Entity, out: &mut Vec<(u64, u64)>, seen: &mut HashSet<u64>) {
    if let Some(id) = world.get::<CollabId>(entity) {
        if seen.insert(entity.to_bits()) {
            out.push((entity.to_bits(), id.0));
        }
    }
}

/// Record that these entities now agree with the session as of `tick`.
fn mark_synced(world: &mut World, entities: &[Entity], tick: Tick) {
    let states: Vec<(u64, ArchetypeId)> = entities
        .iter()
        .filter_map(|&e| {
            let id = world.get::<CollabId>(e)?.0;
            let archetype = world.get_entity(e).ok()?.archetype().id();
            Some((id, archetype))
        })
        .collect();
    let mut tracker = world.resource_mut::<SyncTracker>();
    for (id, archetype) in states {
        tracker.tracked.insert(id, Track { archetype, synced: tick });
    }
}

// ── Receiving ───────────────────────────────────────────────────────────────

/// Apply everything the pump queued, and answer anyone waiting for the document.
pub fn apply_inbox(world: &mut World) {
    let pending: Vec<(u64, CollabMsg)> = {
        let mut inbox = world.resource_mut::<CollabInbox>();
        inbox.queue.drain(..).collect()
    };
    let joiners: Vec<u64> = {
        let mut inbox = world.resource_mut::<CollabInbox>();
        std::mem::take(&mut inbox.needs_scene)
    };

    for (from, msg) in pending {
        match msg {
            CollabMsg::SceneReset { bsn, ids } => apply_scene_reset(world, bsn, ids),
            CollabMsg::EntityUpsert { bsn, ids, removed } => {
                if accept_edit_from(world, from) {
                    apply_upsert(world, from, bsn, ids, removed);
                }
            }
            CollabMsg::EntityDespawn { ids } => {
                if accept_edit_from(world, from) {
                    apply_despawn(world, from, ids);
                }
            }
            // File transfer and leases are handled by their own modules; they are
            // queued here only so that ordering against scene messages is kept.
            other => crate::files::handle(world, from, other),
        }
    }

    for peer in joiners {
        // Scene first, manifest second. The scene is what makes the session
        // visible immediately; the manifest starts a transfer that can take
        // minutes, and a guest staring at an empty viewport while it runs would
        // have no idea anything had worked.
        send_full_scene(world, peer);
        crate::files::send_manifest(world, peer);
    }
}

/// Whether an edit arriving from `from` should be honoured.
///
/// A host applies a guest's edit only while it has handed out control; a guest
/// always applies the host's, because the host is the authority by definition.
fn accept_edit_from(world: &mut World, from: u64) -> bool {
    let session = world.resource::<CollabSession>();
    match session.role {
        crate::session::CollabRole::Offline => false,
        crate::session::CollabRole::Guest => true,
        crate::session::CollabRole::Hosting => {
            if session.allow_control {
                true
            } else {
                // Not an error and not worth a log line per message — the guest
                // is editing locally and being quietly overruled, which is
                // exactly what "watching" means here.
                let _ = from;
                false
            }
        }
    }
}

/// Replace the local document with the sender's.
fn apply_scene_reset(world: &mut World, bsn: String, ids: Vec<(u64, u64)>) {
    // Everything currently in the document belongs to a session we are leaving
    // behind. Despawning rather than merging is deliberate: a reset is sent when
    // the two sides cannot be reconciled incrementally, so merging would layer a
    // stale scene under a fresh one.
    let existing: Vec<Entity> = collect_replicable(world)
        .into_iter()
        .filter(|&e| world.get::<ChildOf>(e).is_none())
        .collect();
    for entity in existing {
        if let Ok(e) = world.get_entity_mut(entity) {
            e.despawn();
        }
    }
    world.resource_mut::<CollabIds>().clear();
    world.resource_mut::<SyncTracker>().reset();

    apply_snapshot(world, &bsn, &ids);
    let count = world.resource::<CollabIds>().len();
    world.resource_mut::<CollabSession>().note(format!("received the scene ({count} entities)"));
}

fn apply_upsert(
    world: &mut World,
    from: u64,
    bsn: String,
    ids: Vec<(u64, u64)>,
    removed: Vec<(u64, Vec<String>)>,
) {
    apply_component_removals(world, &removed);
    apply_snapshot(world, &bsn, &ids);
    world.resource_mut::<SyncTracker>().received += 1;

    // A host is the hub: a guest's edit is only real to the other guests once the
    // host has relayed it. Relayed verbatim, after applying — if applying failed
    // the host is not in a position to vouch for it.
    let session = world.resource::<CollabSession>();
    if session.is_host() {
        session.broadcast_except(from, CollabMsg::EntityUpsert { bsn, ids, removed });
    }
}

/// Write a snapshot onto the entities we already have, spawning the rest.
fn apply_snapshot(world: &mut World, bsn: &str, ids: &[(u64, u64)]) {
    // Match the sender's entity ids to ours through the session ids. Anything
    // unmatched is new here and will be spawned.
    let mut seed = bevy::ecs::entity::EntityHashMap::default();
    {
        let registry = world.resource::<CollabIds>();
        for &(sender_bits, collab_id) in ids {
            if let Some(local) = registry.entity(collab_id) {
                seed.insert(Entity::from_bits(sender_bits), local);
            }
        }
    }

    // `ExternalParents::MappedOnly` because the sender's entity ids are
    // meaningless here — an unmapped one that happens to be live locally is a
    // coincidence, and honouring it would reparent the entity onto whatever
    // unrelated object holds that index.
    let applied = {
        let mut tracker = world.resource_mut::<SyncTracker>();
        tracker.applying = true;
        let applied = apply_entity_snapshot(world, bsn, &seed, ExternalParents::MappedOnly);
        world.resource_mut::<SyncTracker>().applying = false;
        applied
    };

    // Bind the ids of everything that was newly spawned, and stamp every touched
    // entity as synced so the scan does not read our own application of a peer's
    // change as a local edit and send it straight back.
    let by_sender: HashMap<u64, u64> = ids.iter().map(|&(bits, id)| (bits, id)).collect();
    let tick = world.change_tick();
    let mut touched: Vec<Entity> = Vec::new();
    for (sender_entity, local) in applied.iter() {
        let Some(&collab_id) = by_sender.get(&sender_entity.to_bits()) else {
            continue;
        };
        world.resource_mut::<CollabIds>().bind(collab_id, *local);
        // Overwrite rather than fill in: an entity spawned locally before the
        // session, or left over from an earlier one, already carries an id that
        // is not the one the session knows it by.
        if world.get::<CollabId>(*local).map(|id| id.0) != Some(collab_id) {
            world.entity_mut(*local).insert(CollabId(collab_id));
        }
        touched.push(*local);
    }
    mark_synced(world, &touched, tick);
}

/// Remove components that vanished on the sender.
///
/// A snapshot cannot express this — it lists what an entity *has*, and a
/// component that was removed is simply not mentioned, which is
/// indistinguishable from unchanged. Without this, turning a light off on one
/// machine would leave it on forever on the other.
fn apply_component_removals(world: &mut World, removed: &[(u64, Vec<String>)]) {
    if removed.is_empty() {
        return;
    }
    let registry = world.resource::<AppTypeRegistry>().clone();
    for (collab_id, type_paths) in removed {
        let Some(entity) = world.resource::<CollabIds>().entity(*collab_id) else {
            continue;
        };
        for path in type_paths {
            let remover = {
                let types = registry.read();
                types
                    .get_with_type_path(path)
                    .and_then(|reg| reg.data::<bevy::ecs::reflect::ReflectComponent>().cloned())
            };
            if let Some(remover) = remover {
                if let Ok(mut e) = world.get_entity_mut(entity) {
                    remover.remove(&mut e);
                }
            }
        }
    }
}

fn apply_despawn(world: &mut World, from: u64, ids: Vec<u64>) {
    for &id in &ids {
        let entity = world.resource_mut::<CollabIds>().forget(id);
        if let Some(entity) = entity {
            if let Ok(e) = world.get_entity_mut(entity) {
                e.despawn();
            }
        }
        world.resource_mut::<SyncTracker>().tracked.remove(&id);
    }
    let session = world.resource::<CollabSession>();
    if session.is_host() {
        session.broadcast_except(from, CollabMsg::EntityDespawn { ids });
    }
}

/// Send the whole document to one peer.
pub fn send_full_scene(world: &mut World, peer: u64) {
    let entities = collect_replicable(world);

    // Everything in the document needs an id before it can be described.
    let mut minted: Vec<(Entity, CollabId)> = Vec::new();
    {
        let unassigned: Vec<Entity> =
            entities.iter().copied().filter(|&e| !id_is_current(world, e)).collect();
        let mut ids = world.resource_mut::<CollabIds>();
        for e in unassigned {
            minted.push((e, ids.mint()));
        }
    }
    for (e, id) in &minted {
        world.entity_mut(*e).insert(*id);
        world.resource_mut::<CollabIds>().bind(id.0, *e);
    }

    let Some(bsn) = snapshot_entities(world, &entities, Descend::ExactSet) else {
        world.resource::<CollabSession>().send_to(
            peer,
            CollabMsg::SceneReset { bsn: String::new(), ids: Vec::new() },
        );
        return;
    };
    let ids: Vec<(u64, u64)> = entities
        .iter()
        .filter_map(|&e| world.get::<CollabId>(e).map(|id| (e.to_bits(), id.0)))
        .collect();

    let tick = world.change_tick();
    mark_synced(world, &entities, tick);

    let count = ids.len();
    let session = world.resource::<CollabSession>();
    session.send_to(peer, CollabMsg::SceneReset { bsn, ids });
    let mut session = world.resource_mut::<CollabSession>();
    session.note(format!("sent the scene ({count} entities)"));
}
