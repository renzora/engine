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

// ── Painting ────────────────────────────────────────────────────────────────

impl ScriptCtx<'_> {
    /// A painter for this entity's draw surface.
    ///
    /// The Rust equivalent of Lua's `g`, and the same underlying mechanism: it
    /// appends [`DrawCmd`](crate::core::DrawCmd)s to
    /// [`ScriptDrawBuffer`](crate::core::ScriptDrawBuffer), which the UI vector
    /// renderer drains into pooled shape entities. Nothing about the drawing is
    /// Lua-specific; only the vocabulary to reach it was.
    ///
    /// Immediate mode: call this inside a [`ScriptHook::Draw`](crate::ScriptHook)
    /// and paint the whole picture each frame. The first painter obtained in a
    /// frame clears the entity's previous list, so a script that stops drawing
    /// something stops showing it, with nothing to erase.
    ///
    /// ```ignore
    /// fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    ///     if let ScriptHook::Draw { width, height } = *hook {
    ///         let mut g = ctx.painter();
    ///         g.rect(0.0, 0.0, width, height, [0.0, 0.0, 0.0, 0.5]);
    ///         g.circle(width * 0.5, height * 0.5, 40.0, [1.0, 0.8, 0.2, 1.0]);
    ///     }
    /// }
    /// ```
    pub fn painter(&mut self) -> Painter<'_> {
        let entity = self.entity;
        let buffer = self
            .world
            .get_resource_mut::<crate::core::ScriptDrawBuffer>();
        if let Some(mut buffer) = buffer {
            // Cleared on acquisition, not on drop: a script may take a painter,
            // branch, and take another, and the picture should be one list.
            buffer.per_entity.entry(entity).or_default().clear();
        }
        Painter {
            world: self.world,
            entity,
        }
    }

    /// The size of this entity's draw surface, if it has one.
    ///
    /// [`ScriptHook::Draw`](crate::ScriptHook) already carries it; this is for
    /// the rarer case of wanting it from `update`.
    pub fn surface_size(&self) -> Option<bevy::math::Vec2> {
        self.world
            .get_resource::<crate::core::ScriptDrawSurfaces>()?
            .per_entity
            .get(&self.entity)
            .copied()
    }
}

/// Immediate-mode drawing onto a script entity's canvas.
///
/// Coordinates are surface-local pixels with a **top-left origin and y down**,
/// matching CSS and the UI rather than the 2D world — a canvas is part of the
/// interface, and having it agree with the thing it is laid out inside matters
/// more than agreeing with the scene behind it. Colours are sRGB `[r, g, b, a]`
/// in `0..1`.
pub struct Painter<'w> {
    world: &'w mut World,
    entity: Entity,
}

impl Painter<'_> {
    fn push(&mut self, cmd: crate::core::DrawCmd) {
        if let Some(mut buffer) = self
            .world
            .get_resource_mut::<crate::core::ScriptDrawBuffer>()
        {
            buffer.per_entity.entry(self.entity).or_default().push(cmd);
        }
    }

    /// Straight stroke between two points.
    pub fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: [f32; 4], thickness: f32) {
        self.push(crate::core::DrawCmd::Line {
            x1,
            y1,
            x2,
            y2,
            color,
            thickness,
        });
    }

    /// Stroked circular arc. `start`/`end` in degrees, 0 = +x, clockwise.
    pub fn arc(
        &mut self,
        cx: f32,
        cy: f32,
        r: f32,
        start: f32,
        end: f32,
        color: [f32; 4],
        thickness: f32,
    ) {
        self.push(crate::core::DrawCmd::Arc {
            cx,
            cy,
            r,
            start,
            end,
            color,
            thickness,
        });
    }

    /// Filled circle.
    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, color: [f32; 4]) {
        self.push(crate::core::DrawCmd::Circle { cx, cy, r, color });
    }

    /// Filled axis-aligned rectangle.
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        self.push(crate::core::DrawCmd::Rect { x, y, w, h, color });
    }

    /// Filled triangle.
    pub fn triangle(
        &mut self,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        x3: f32,
        y3: f32,
        color: [f32; 4],
    ) {
        self.push(crate::core::DrawCmd::Triangle {
            x1,
            y1,
            x2,
            y2,
            x3,
            y3,
            color,
        });
    }

    /// Text, baseline-anchored at `(x, y)` and centred horizontally on `x`.
    pub fn text(&mut self, x: f32, y: f32, text: impl Into<String>, size: f32, color: [f32; 4]) {
        self.push(crate::core::DrawCmd::Text {
            x,
            y,
            text: text.into(),
            size,
            color,
        });
    }

    /// Stroked rectangle outline, as four lines.
    ///
    /// A convenience rather than a command: the renderer has no outline
    /// primitive, and every caller would otherwise write the same four calls.
    pub fn rect_outline(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
        thickness: f32,
    ) {
        self.line(x, y, x + w, y, color, thickness);
        self.line(x + w, y, x + w, y + h, color, thickness);
        self.line(x + w, y + h, x, y + h, color, thickness);
        self.line(x, y + h, x, y, color, thickness);
    }

    /// Filled convex polygon, fanned into triangles from the first point.
    ///
    /// The same fan Lua's `g.poly` performs. Fewer than three points draws
    /// nothing rather than erroring — a polygon built from a loop that happened
    /// to produce one point is a degenerate shape, not a bug worth aborting on.
    pub fn poly(&mut self, points: &[(f32, f32)], color: [f32; 4]) {
        if points.len() < 3 {
            return;
        }
        let (x1, y1) = points[0];
        for pair in points[1..].windows(2) {
            let ((x2, y2), (x3, y3)) = (pair[0], pair[1]);
            self.triangle(x1, y1, x2, y2, x3, y3, color);
        }
    }
}
