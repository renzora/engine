//! Name to entity lookup for constraint references.
//!
//! Constraints name their targets (see [`crate::components`]), and a linear
//! scan of every `Name` in the world per constraint per frame would cost more
//! than the solving. So the index is kept up to date from `Changed<Name>` and
//! removals, which is work only when names change.
//!
//! Names are not unique. Two instances of one character share every bone name
//! and probably their marker names too, so a lookup prefers the candidate
//! under the same top-level root as whoever is asking, and a bone lookup
//! ([`NameLookup::find_under`]) accepts only a candidate inside the asking
//! character. Every candidate is also checked against its current `Name`: a
//! removal can be missed (removal events only live two frames) and the id
//! reused, and a stale hit is worse than none.

use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Resource, Default)]
pub struct NameLookup {
    by_name: HashMap<String, Vec<Entity>>,
    by_entity: HashMap<Entity, String>,
}

impl NameLookup {
    fn remove(&mut self, entity: Entity) {
        if let Some(old) = self.by_entity.remove(&entity) {
            if let Some(list) = self.by_name.get_mut(&old) {
                list.retain(|e| *e != entity);
                if list.is_empty() {
                    self.by_name.remove(&old);
                }
            }
        }
    }

    fn insert(&mut self, entity: Entity, name: &str) {
        self.remove(entity);
        self.by_entity.insert(entity, name.to_string());
        self.by_name.entry(name.to_string()).or_default().push(entity);
    }

    fn candidates<'a, 'w, 's, 'n>(
        &'a self,
        name: &'a str,
        names: &'a Query<'w, 's, &'n Name>,
    ) -> impl Iterator<Item = Entity> + use<'a, 'w, 's, 'n> {
        self.by_name
            .get(name)
            .into_iter()
            .flatten()
            .copied()
            .filter(move |e| names.get(*e).is_ok_and(|n| n.as_str() == name))
    }

    /// The entity called `name`, preferring one under the same top-level root
    /// as `near`.
    pub fn find(&self, name: &str, near: Entity, names: &Query<&Name>, parents: &Query<&ChildOf>) -> Option<Entity> {
        if name.is_empty() {
            return None;
        }
        let home = top_root(near, parents);
        let mut first = None;
        for e in self.candidates(name, names) {
            if top_root(e, parents) == home {
                return Some(e);
            }
            first.get_or_insert(e);
        }
        first
    }

    /// The entity called `name` that is `ancestor` or one of its descendants.
    pub fn find_under(&self, name: &str, ancestor: Entity, names: &Query<&Name>, parents: &Query<&ChildOf>) -> Option<Entity> {
        if name.is_empty() {
            return None;
        }
        self.candidates(name, names).find(|e| is_under(*e, ancestor, parents))
    }
}

/// Keep [`NameLookup`] in step with the world. Runs every frame, play or not:
/// it costs nothing without changes, and skipping frames would lose removals.
pub fn update_name_lookup(
    mut lookup: ResMut<NameLookup>,
    changed: Query<(Entity, &Name), Changed<Name>>,
    mut removed: RemovedComponents<Name>,
) {
    for entity in removed.read() {
        lookup.remove(entity);
    }
    for (entity, name) in &changed {
        lookup.insert(entity, name.as_str());
    }
}

/// The topmost ancestor of `entity` (itself when it has no parent).
pub fn top_root(entity: Entity, parents: &Query<&ChildOf>) -> Entity {
    let mut e = entity;
    // Bounded: a cycle cannot exist in a valid hierarchy, but a guard costs
    // nothing and a hang would cost the game.
    for _ in 0..1024 {
        match parents.get(e) {
            Ok(p) => e = p.parent(),
            Err(_) => break,
        }
    }
    e
}

/// True when `entity` is `ancestor` or below it.
pub fn is_under(entity: Entity, ancestor: Entity, parents: &Query<&ChildOf>) -> bool {
    let mut e = entity;
    for _ in 0..1024 {
        if e == ancestor {
            return true;
        }
        match parents.get(e) {
            Ok(p) => e = p.parent(),
            Err(_) => return false,
        }
    }
    false
}
