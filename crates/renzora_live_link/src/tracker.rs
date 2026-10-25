//! What changed since the last look, as messages for the other side.
//!
//! Both ends run one of these over their own world. It holds what the other
//! side is known to have (each scene entity's components, serialized), and each
//! [`Tracker::scan`] compares the world against that and reports the
//! difference. Messages that arrive from the other side are recorded with
//! [`Tracker::note`] as they are applied, so an edit received is not then sent
//! straight back.
//!
//! Comparing serialized values rather than trusting change detection is what
//! keeps the link quiet. Plenty of systems write a component every frame
//! without changing it, and plenty of components are derived rather than
//! authored. Change detection only narrows the search to entities worth
//! serializing; the comparison decides what is sent, and the save's own
//! extraction ([`renzora_engine::scene_io::extract_authored`]) decides what
//! counts as authored at all.

use std::collections::{HashMap, HashSet};

use bevy::ecs::archetype::ArchetypeId;
use bevy::ecs::change_detection::Tick;
use bevy::prelude::*;
use bevy::reflect::serde::TypedReflectSerializer;
use bevy::reflect::{PartialReflect, ReflectRef};
use renzora::{SceneEntityId, ViewportGateHidden};
use renzora_engine::scene_io::{assign_scene_entity_ids, extract_authored, saveable_entities};

use crate::LinkMessage;

/// Entities that joined the scene with no id are looked for on one scan in
/// this many. Finding them walks every named entity's ancestry, which is not
/// free in an editor full of named UI; a new entity showing up a few tenths of
/// a second late costs nothing.
const NEW_ENTITY_SCAN_EVERY: u32 = 10;

#[derive(Default)]
pub struct Tracker {
    /// What the other side has: per scene id, each component's serialized
    /// value by type path.
    known: HashMap<u64, HashMap<String, String>>,
    /// Each tracked entity's archetype at the last scan. A different one means
    /// a component was added or removed, which change detection does not see.
    shapes: HashMap<Entity, ArchetypeId>,
    last_tick: Tick,
    scans: u32,
}

impl Tracker {
    /// Record every scene entity as already known to the other side.
    ///
    /// Right for a fresh connection: the game loaded the scene file Play saved
    /// a moment earlier, so both worlds start from the same place.
    pub fn baseline(world: &mut World) -> Self {
        let last_tick = world.change_tick();
        let tracked: Vec<Entity> = ids(world).into_iter().map(|(e, _)| e).collect();
        let shapes = tracked
            .iter()
            .map(|&e| (e, world.entity(e).archetype().id()))
            .collect();
        let known = authored_values(world, tracked)
            .into_iter()
            .map(|v| (v.id, v.values))
            .collect();
        Self { known, shapes, last_tick, scans: 0 }
    }

    /// What the other side has, by scene id and type path.
    pub fn known(&self) -> &HashMap<u64, HashMap<String, String>> {
        &self.known
    }

    /// Record that the other side has what `message` describes: it sent it, or
    /// is about to be sent it.
    pub fn note(&mut self, message: &LinkMessage) {
        match message {
            LinkMessage::Spawn { id, .. } => {
                self.known.entry(*id).or_default();
            }
            LinkMessage::Despawn { id } => {
                self.known.remove(id);
            }
            LinkMessage::Set { id, ty, value } => {
                self.known.entry(*id).or_default().insert(ty.clone(), value.clone());
            }
            LinkMessage::Remove { id, ty } => {
                if let Some(values) = self.known.get_mut(id) {
                    values.remove(ty);
                }
            }
            _ => {}
        }
    }

    /// Everything that differs from what the other side has, in an order the
    /// other side can apply directly: despawns, then spawns parents first, then
    /// component values. Each message is also [`noted`](Self::note).
    pub fn scan(&mut self, world: &mut World) -> Vec<LinkMessage> {
        let now = world.change_tick();
        self.scans = self.scans.wrapping_add(1);

        if self.scans % NEW_ENTITY_SCAN_EVERY == 1 {
            let fresh: Vec<Entity> = saveable_entities(world)
                .into_iter()
                .filter(|&e| world.get::<SceneEntityId>(e).is_none_or(|id| id.0 == 0))
                .collect();
            if !fresh.is_empty() {
                assign_scene_entity_ids(world, &fresh);
            }
        }

        let tracked = ids(world);
        let present: HashSet<u64> = tracked.iter().map(|(_, id)| *id).collect();
        let mut out = Vec::new();

        let gone: Vec<u64> = self.known.keys().filter(|id| !present.contains(id)).copied().collect();
        for id in gone {
            out.push(LinkMessage::Despawn { id });
        }
        self.shapes.retain(|e, _| world.get_entity(*e).is_ok());

        // New to the other side: spawn, parents before children so each
        // `parent` already exists when its child arrives.
        let mut fresh: Vec<(usize, Entity, u64)> = tracked
            .iter()
            .filter(|(_, id)| !self.known.contains_key(id))
            .map(|&(e, id)| (depth(world, e), e, id))
            .collect();
        fresh.sort_by_key(|(d, _, _)| *d);
        for &(_, entity, id) in &fresh {
            let parent = world
                .get::<ChildOf>(entity)
                .and_then(|c| world.get::<SceneEntityId>(c.parent()))
                .map(|p| p.0)
                .filter(|p| *p != 0);
            let name = world.get::<Name>(entity).map(|n| n.as_str().to_string()).unwrap_or_default();
            out.push(LinkMessage::Spawn { id, parent, name });
        }

        let last = self.last_tick;
        let dirty: Vec<Entity> = tracked
            .iter()
            .map(|(e, _)| *e)
            .filter(|&e| {
                let entity = world.entity(e);
                let archetype = entity.archetype();
                self.shapes.get(&e) != Some(&archetype.id())
                    || archetype.components().iter().any(|&c| {
                        entity
                            .get_change_ticks_by_id(c)
                            .is_some_and(|t| t.is_changed(last, now))
                    })
            })
            .collect();
        for &e in &dirty {
            self.shapes.insert(e, world.entity(e).archetype().id());
        }
        self.last_tick = now;

        for current in authored_values(world, dirty) {
            let known = self.known.get(&current.id);
            for (ty, value) in &current.values {
                if known.and_then(|k| k.get(ty)) != Some(value) {
                    out.push(LinkMessage::Set { id: current.id, ty: ty.clone(), value: value.clone() });
                }
            }
            if let Some(known) = known {
                for ty in known.keys() {
                    if !current.values.contains_key(ty) && !current.withheld.contains(ty) {
                        out.push(LinkMessage::Remove { id: current.id, ty: ty.clone() });
                    }
                }
            }
        }

        for message in &out {
            self.note(message);
        }
        out
    }
}

/// Every entity with a scene id.
fn ids(world: &mut World) -> Vec<(Entity, u64)> {
    let mut q = world.query::<(Entity, &SceneEntityId)>();
    q.iter(world)
        .filter(|(_, id)| id.0 != 0)
        .map(|(e, id)| (e, id.0))
        .collect()
}

fn depth(world: &World, mut entity: Entity) -> usize {
    let mut d = 0;
    while let Some(parent) = world.get::<ChildOf>(entity).map(|c| c.parent()) {
        entity = parent;
        d += 1;
    }
    d
}

pub(crate) struct EntityValues {
    pub id: u64,
    pub values: HashMap<String, String>,
    /// Types deliberately left out of `values` this time, which must not be
    /// read as removed. See [`authored_values`].
    pub withheld: HashSet<String>,
}

/// Each entity's authored components, serialized, keyed by type path.
///
/// Left out, beyond what a save leaves out:
///
/// - Anything holding an `Entity`. An entity id means nothing in the other
///   world, and applying one there would point at whatever happens to have
///   that index: a parent link to a random entity, a joint to nothing.
///   Parenting travels in [`LinkMessage::Spawn`] instead.
/// - `GlobalTransform`: a save keeps it, but it is recomputed from `Transform`
///   every frame on both sides, so sending it would only double the traffic of
///   every move.
/// - The scene id itself, which is the address, not a value.
///
/// And `Visibility` on an entity the editor's viewport gate is hiding is read
/// from the gate, which holds the real value while it shows `Hidden`. The gate
/// hides the whole scene whenever no viewport is on screen, which is the
/// moment someone switches to the Game panel. Reading the live value would
/// hide the running game's world; leaving the value out, as this first did,
/// left a gap in the baseline Stop restores from, and Stop then removed the
/// component instead of putting it back.
pub(crate) fn authored_values(world: &mut World, entities: Vec<Entity>) -> Vec<EntityValues> {
    if entities.is_empty() {
        return Vec::new();
    }
    let ids: HashMap<Entity, u64> = entities
        .iter()
        .filter_map(|&e| world.get::<SceneEntityId>(e).map(|id| (e, id.0)))
        .collect();
    let gated: HashMap<Entity, Visibility> = entities
        .iter()
        .filter_map(|&e| world.get::<ViewportGateHidden>(e).map(|g| (e, g.0)))
        .collect();

    let scene = extract_authored(world, entities, false);
    let registry = world.resource::<AppTypeRegistry>().clone();
    let registry = registry.read();
    let visibility = Visibility::type_path();
    let skipped = [GlobalTransform::type_path(), SceneEntityId::type_path()];

    let mut out = Vec::with_capacity(scene.entities.len());
    for entity in &scene.entities {
        let Some(&id) = ids.get(&entity.entity) else { continue };
        let mut values = HashMap::new();
        let mut withheld = HashSet::new();
        for component in &entity.components {
            let ty = component.reflect_type_path();
            if skipped.contains(&ty) || holds_entity(component.as_partial_reflect()) {
                withheld.insert(ty.to_string());
                continue;
            }
            let real_visibility = (ty == visibility)
                .then(|| gated.get(&entity.entity))
                .flatten();
            let serialized = match real_visibility {
                Some(real) => ron::ser::to_string(&TypedReflectSerializer::new(real, &registry)),
                None => ron::ser::to_string(&TypedReflectSerializer::new(
                    component.as_partial_reflect(),
                    &registry,
                )),
            };
            if let Ok(value) = serialized {
                values.insert(ty.to_string(), value);
            }
        }
        out.push(EntityValues { id, values, withheld });
    }
    out
}

/// Whether `value` contains an `Entity` anywhere inside it.
pub(crate) fn holds_entity(value: &dyn PartialReflect) -> bool {
    if value.represents::<Entity>() {
        return true;
    }
    match value.reflect_ref() {
        ReflectRef::Struct(s) => s.iter_fields().any(|(_, f)| holds_entity(f)),
        ReflectRef::TupleStruct(s) => s.iter_fields().any(holds_entity),
        ReflectRef::Tuple(t) => t.iter_fields().any(holds_entity),
        ReflectRef::List(l) => l.iter().any(holds_entity),
        ReflectRef::Array(a) => a.iter().any(holds_entity),
        ReflectRef::Map(m) => m.iter().any(|(k, v)| holds_entity(k) || holds_entity(v)),
        ReflectRef::Set(s) => s.iter().any(holds_entity),
        ReflectRef::Enum(e) => e.iter_fields().any(|f| holds_entity(f.value())),
        _ => false,
    }
}
