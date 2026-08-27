//! What a Rust script is handed: itself, and the world it lives in.
//!
//! ```ignore
//! fn update(ctx: &mut ScriptCtx) {
//!     let dt = ctx.delta();
//!     if let Some(mut t) = ctx.get_mut::<Transform>() {
//!         t.rotate_y(dt);
//!     }
//! }
//!
//! renzora::script!(update);
//! ```
//!
//! # Why this exists rather than `(&mut World, Entity)`
//!
//! That was the first shape, and it read badly. A script's most common act is
//! touching its *own* entity, and every one of those looked like
//! `world.get_mut::<Transform>(me)` — passing `me` back to the world that just
//! handed it to you, as if the entity had to be looked up. It does not; the
//! script simply had no way to say "mine".
//!
//! So the entity and the world travel together and the common case gets short:
//! [`get`](Self::get), [`get_mut`](Self::get_mut), [`insert`](Self::insert) and
//! [`remove`](Self::remove) act on the script's own entity with no argument.
//!
//! # Nothing is taken away
//!
//! This is not a sandbox or a vocabulary. [`world`](Self::world) hands back the
//! whole `&mut World`, so anything Bevy allows is still one call away — spawning
//! hierarchies, building UI, querying every entity, swapping assets. The context
//! shortens the common case; it does not narrow the uncommon one.

use bevy::ecs::component::Mutable;
use bevy::prelude::*;

/// A script's handle on itself and on the world.
pub struct ScriptCtx<'w> {
    world: &'w mut World,
    entity: Entity,
}

impl<'w> ScriptCtx<'w> {
    /// Built by the dispatcher, once per script call.
    pub fn new(world: &'w mut World, entity: Entity) -> Self {
        Self { world, entity }
    }

    /// The entity this script is attached to.
    ///
    /// Needed when handing yourself to something else — a parent, a resource, a
    /// spawned child's `ChildOf`.
    pub fn entity(&self) -> Entity {
        self.entity
    }

    /// The whole world. The escape hatch, and not a last resort — spawning,
    /// querying and asset access all go through here.
    pub fn world(&mut self) -> &mut World {
        self.world
    }

    /// Seconds since the last frame.
    ///
    /// On the context because almost every script wants it and
    /// `ctx.world().resource::<Time>().delta_secs()` is a lot of ceremony for
    /// something that common.
    pub fn delta(&self) -> f32 {
        self.world.get_resource::<Time>().map(|t| t.delta_secs()).unwrap_or(0.0)
    }

    /// Seconds since startup.
    pub fn elapsed(&self) -> f32 {
        self.world.get_resource::<Time>().map(|t| t.elapsed_secs()).unwrap_or(0.0)
    }

    // ── This entity ──────────────────────────────────────────────────────────

    /// A component on this entity.
    pub fn get<T: Component>(&self) -> Option<&T> {
        self.world.get::<T>(self.entity)
    }

    /// A component on this entity, mutably.
    pub fn get_mut<T: Component<Mutability = Mutable>>(&mut self) -> Option<Mut<'_, T>> {
        self.world.get_mut::<T>(self.entity)
    }

    /// Whether this entity has a component.
    pub fn has<T: Component>(&self) -> bool {
        self.world.get::<T>(self.entity).is_some()
    }

    /// Add components to this entity.
    ///
    /// Silently does nothing if the entity has been despawned — by an earlier
    /// script this frame, or by this one. That is deliberate: a script should not
    /// have to check it still exists before every write.
    pub fn insert(&mut self, bundle: impl Bundle) {
        if let Ok(mut e) = self.world.get_entity_mut(self.entity) {
            e.insert(bundle);
        }
    }

    /// Remove components from this entity. Also a no-op once despawned.
    pub fn remove<T: Bundle>(&mut self) {
        if let Ok(mut e) = self.world.get_entity_mut(self.entity) {
            e.remove::<T>();
        }
    }

    /// This entity's name, if it has one.
    pub fn name(&self) -> Option<&str> {
        self.get::<Name>().map(|n| n.as_str())
    }

    // ── Other entities and resources ─────────────────────────────────────────

    /// A component on some *other* entity.
    pub fn get_on<T: Component>(&self, entity: Entity) -> Option<&T> {
        self.world.get::<T>(entity)
    }

    /// A component on some other entity, mutably.
    pub fn get_mut_on<T: Component<Mutability = Mutable>>(
        &mut self,
        entity: Entity,
    ) -> Option<Mut<'_, T>> {
        self.world.get_mut::<T>(entity)
    }

    /// A resource, if it exists.
    ///
    /// `get_` rather than a panicking `resource()`: a script asking for something
    /// an export stripped should not take the process down.
    pub fn get_resource<T: Resource>(&self) -> Option<&T> {
        self.world.get_resource::<T>()
    }

    /// A resource, mutably, if it exists.
    ///
    /// Bevy gates mutable resource access on `Mutability = Mutable`, the same way
    /// it does for components, so that bound is carried through rather than
    /// hidden — an immutable resource should fail here, not at the call inside.
    pub fn get_resource_mut<T: Resource<Mutability = Mutable>>(&mut self) -> Option<Mut<'_, T>> {
        self.world.get_resource_mut::<T>()
    }

    /// This entity's children, if any.
    pub fn children(&self) -> Vec<Entity> {
        self.world
            .get::<Children>(self.entity)
            .map(|c| c.iter().collect())
            .unwrap_or_default()
    }

    /// This entity's parent, if any.
    pub fn parent(&self) -> Option<Entity> {
        self.world.get::<ChildOf>(self.entity).map(|c| c.parent())
    }
}
