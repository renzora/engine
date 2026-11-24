//! Make this world match what the other side sent.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy::reflect::serde::{TypedReflectDeserializer, TypedReflectSerializer};
use renzora_scene_api::SceneEntityId;
use serde::de::DeserializeSeed;

use crate::tracker::Tracker;
use crate::LinkMessage;

/// What the side applying wants to know about as it happens. The game needs
/// none of it; the editor records everything, so it can put things back when
/// the game stops.
pub trait ApplySide {
    /// `entity` was just spawned to mirror one on the other side.
    fn spawned(&mut self, _world: &mut World, _entity: Entity, _id: u64) {}
    /// `entity` is about to be despawned because the other side removed it.
    fn despawning(&mut self, _world: &mut World, _entity: Entity, _id: u64) {}
    /// Component `ty` on entity `id` is about to be set or removed.
    fn touching(&mut self, _id: u64, _ty: &str) {}
    /// A message was just applied to `entity`, scene id `id` (spawned, or a
    /// component set or removed). The side can correct anything it must not
    /// take as sent.
    fn applied(&mut self, _world: &mut World, _entity: Entity, _id: u64) {}
}

/// Nothing to record.
pub struct Quiet;
impl ApplySide for Quiet {}

/// Apply `messages` in order, noting each in `tracker` so it is not echoed
/// back. `unknown` collects type paths this world does not know, so each is
/// reported once rather than on every edit. Messages that are not edits
/// (input, screenshots) are handed back for the caller.
pub fn apply(
    world: &mut World,
    messages: Vec<LinkMessage>,
    tracker: &mut Tracker,
    side: &mut dyn ApplySide,
    unknown: &mut HashSet<String>,
) -> Vec<LinkMessage> {
    let mut rest = Vec::new();
    if messages.is_empty() {
        return rest;
    }
    let mut by_id: HashMap<u64, Entity> = {
        let mut q = world.query::<(Entity, &SceneEntityId)>();
        q.iter(world).map(|(e, id)| (id.0, e)).collect()
    };
    let registry = world.resource::<AppTypeRegistry>().clone();

    for message in messages {
        match &message {
            LinkMessage::Spawn { id, parent, name } => {
                if by_id.contains_key(id) {
                    tracker.note(&message);
                    continue;
                }
                let mut spawned = world.spawn((SceneEntityId(*id), Name::new(name.clone())));
                if let Some(parent) = parent.and_then(|p| by_id.get(&p).copied()) {
                    spawned.insert(ChildOf(parent));
                }
                let entity = spawned.id();
                by_id.insert(*id, entity);
                side.spawned(world, entity, *id);
                side.applied(world, entity, *id);
                // The name came with the spawn, so this side holds it as a
                // value the other side already has; recorded, it is not sent
                // back as an edit on the first scan of the new entity.
                let name = ron::ser::to_string(&TypedReflectSerializer::new(
                    &Name::new(name.clone()),
                    &registry.read(),
                ));
                tracker.note(&message);
                if let Ok(value) = name {
                    tracker.note(&LinkMessage::Set { id: *id, ty: Name::type_path().into(), value });
                }
                continue;
            }
            LinkMessage::Despawn { id } => {
                if let Some(entity) = by_id.remove(id) {
                    side.despawning(world, entity, *id);
                    if let Ok(e) = world.get_entity_mut(entity) {
                        e.despawn();
                    }
                }
            }
            LinkMessage::Set { id, ty, .. } | LinkMessage::Remove { id, ty } => {
                // An entity this side has already lost: nothing to do.
                let Some(&entity) = by_id.get(id) else {
                    tracker.note(&message);
                    continue;
                };
                let registry = registry.read();
                let Some(registration) = registry.get_with_type_path(ty) else {
                    if unknown.insert(ty.clone()) {
                        warn!("[live-link] {ty} is not a registered type here; edits to it are skipped");
                    }
                    continue;
                };
                let Some(reflect_component) = registration.data::<ReflectComponent>() else {
                    continue;
                };
                side.touching(*id, ty);
                let Ok(mut entity_mut) = world.get_entity_mut(entity) else { continue };
                if let LinkMessage::Set { value, .. } = &message {
                    let parsed = ron::de::Deserializer::from_str(value)
                        .map_err(|e| e.to_string())
                        .and_then(|mut de| {
                            TypedReflectDeserializer::new(registration, &registry)
                                .deserialize(&mut de)
                                .map_err(|e| e.to_string())
                        });
                    match parsed {
                        Ok(component) => reflect_component.insert(
                            &mut entity_mut,
                            component.as_partial_reflect(),
                            &registry,
                        ),
                        Err(e) => {
                            warn!("[live-link] could not read {ty}: {e}");
                            continue;
                        }
                    }
                    // Record what this side now holds, not what was sent. A
                    // value that does not survive the trip unchanged (an asset
                    // handle made at runtime, such as an icon's font, comes out
                    // as the default) would otherwise differ from the record
                    // on the next scan and be sent straight back as an edit,
                    // replacing the original on the side that had it right:
                    // a game's icons turned into blank boxes that way.
                    let held = world.get_entity(entity).ok().and_then(|e| {
                        let component = reflect_component.reflect(e)?;
                        ron::ser::to_string(&TypedReflectSerializer::new(
                            component.as_partial_reflect(),
                            &registry,
                        ))
                        .ok()
                    });
                    drop(registry);
                    side.applied(world, entity, *id);
                    tracker.note(&match held {
                        Some(value) => LinkMessage::Set { id: *id, ty: ty.clone(), value },
                        None => message,
                    });
                    continue;
                } else {
                    reflect_component.remove(&mut entity_mut);
                }
                drop(registry);
                side.applied(world, entity, *id);
            }
            _ => {
                rest.push(message);
                continue;
            }
        }
        tracker.note(&message);
    }
    rest
}
