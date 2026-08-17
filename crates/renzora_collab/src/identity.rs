//! Session-wide entity identity.
//!
//! Two editors have no shared vocabulary for "that entity". Bevy's `Entity` is
//! an index into one `World` and means nothing in another; entity names are not
//! unique and the user renames them; and the scene file has no per-entity id at
//! all. So a session mints its own: a [`CollabId`] that both sides agree on for
//! as long as the session lasts.
//!
//! ## Why it is not a reflected component
//!
//! [`CollabId`] deliberately derives neither `Reflect` nor registers itself. The
//! scene serializer extracts *registered* components, so an unregistered one is
//! invisible to it — which means these ids can never leak into a `.scene` file,
//! never show up in a diff, and never survive a save/load to be mistaken for a
//! durable identity in a later session. They ride the wire in the message's own
//! id table instead, where their scope is unmistakable.
//!
//! ## Why ids are partitioned rather than host-assigned
//!
//! A guest that spawns an entity needs an id for it *before* the host has seen
//! it, or the entity cannot be described in the message that announces it. Round
//! -tripping to the host for every new id would put a network hop in front of
//! every spawn. Instead the id space is split: the top 16 bits are the peer's
//! slot, the bottom 48 a local counter, so every peer mints freely and no two
//! can collide. The host is slot 0.

use std::collections::HashMap;

use bevy::prelude::*;

/// An entity's identity for the lifetime of a session. See the module docs for
/// why this is not reflected.
#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CollabId(pub u64);

impl CollabId {
    /// Which peer minted this id.
    pub fn slot(self) -> u16 {
        (self.0 >> 48) as u16
    }
}

/// Maps ids to the local entities carrying them.
///
/// The reverse direction is the [`CollabId`] component itself, so only this
/// direction needs storing — and it is the direction every incoming message
/// needs, since a message names ids and the receiver must find its own copies.
#[derive(Resource, Default)]
pub struct CollabIds {
    by_id: HashMap<u64, Entity>,
    /// This peer's slot, in the high 16 bits of everything it mints.
    slot: u16,
    next: u64,
}

impl CollabIds {
    /// Reset for a new session, claiming `slot` of the id space.
    pub fn begin(&mut self, slot: u16) {
        self.by_id.clear();
        self.slot = slot;
        self.next = 1;
    }

    pub fn clear(&mut self) {
        self.by_id.clear();
        self.next = 1;
    }

    /// Mint an id nobody else can produce.
    ///
    /// Saturating rather than wrapping at the 48-bit boundary: wrapping would
    /// hand out an id that is already live and silently alias two entities, and
    /// a session that has minted 281 trillion ids has a bigger problem than a
    /// stuck counter.
    pub fn mint(&mut self) -> CollabId {
        let local = self.next.min((1u64 << 48) - 1);
        self.next = self.next.saturating_add(1);
        CollabId(((self.slot as u64) << 48) | local)
    }

    pub fn entity(&self, id: u64) -> Option<Entity> {
        self.by_id.get(&id).copied()
    }

    pub fn bind(&mut self, id: u64, entity: Entity) {
        self.by_id.insert(id, entity);
    }

    pub fn forget(&mut self, id: u64) -> Option<Entity> {
        self.by_id.remove(&id)
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (u64, Entity)> + '_ {
        self.by_id.iter().map(|(&id, &e)| (id, e))
    }

    /// Ids whose entity no longer exists.
    ///
    /// This is how a despawn is noticed at all. Nothing announces a despawn —
    /// the entity is simply gone next time we look — so the registry is the only
    /// record that it was ever there. Reporting rather than removing keeps this
    /// an immutable read, so the caller can hold the world while asking.
    pub fn dead(&self, world: &World) -> Vec<u64> {
        self.by_id
            .iter()
            .filter(|(_, &e)| world.get_entity(e).is_err())
            .map(|(&id, _)| id)
            .collect()
    }
}
