//! Dependency tracking — the middle third of a SolidJS-style reactive system.
//!
//! Ember already had the two ends: ECS data is the "signal", a `bind_*` reaction
//! is the "effect", and `apply(world, target, &v)` is the fine-grained write. The
//! piece that was missing is the **data → binding edge**. Without it nothing can
//! conclude a binding is clean without running it, so [`super::run_reactions`]
//! recomputed all ~900 of them every frame and threw ~98% of the results away.
//! The `PartialEq` diff downstream is a *write* filter — it stops the component
//! write, and therefore the taffy relayout, but only after the closure has
//! already allocated its `String`s.
//!
//! [`Rx`] closes that gap. It wraps `&World` and records every resource and
//! component a closure reads; the recorded [`DepSet`] is then checked against
//! Bevy's change ticks on later frames, and the closure is skipped entirely when
//! nothing it read has moved.
//!
//! ## The safety property that makes this landable in pieces
//!
//! **An empty dep set means dirty, never clean.** A closure that recorded no
//! dependency — because it has not run yet, because it reads an `Instant` or the
//! filesystem, or because it is a legacy `Fn(&World)` binding that never touches
//! an [`Rx`] at all — is treated exactly as it is treated today: run it. So
//! tracking can only ever *remove* work; there is no arrangement of tracked and
//! untracked bindings that makes the UI go stale. That is what lets the ~900
//! call sites migrate a file at a time instead of in one flag day.
//!
//! The corollary is that a genuinely constant closure re-runs every frame
//! forever. That is deliberate and it is the right trade: the alternative is
//! distinguishing "read nothing" from "read nothing *yet*", and getting that
//! wrong freezes a panel.
//!
//! ## Why reads return `&'w T` and not `&T`
//!
//! The accessors borrow from the *world*, not from `&self`, so two reads can be
//! held at once:
//!
//! ```ignore
//! let a = rx.resource::<A>();
//! let b = rx.resource::<B>();   // would not compile if `a` borrowed `rx`
//! format!("{} {}", a.x, b.y)
//! ```
//!
//! Existing closure bodies do this constantly, and the whole migration plan
//! depends on them compiling verbatim. That is also why recording goes through a
//! `RefCell` rather than `&mut self`.
//!
//! ## Why there is no `Deref<Target = World>`
//!
//! It would be convenient and it would be a silent-staleness generator: any read
//! that fell through the deref would record no dependency, so the binding would
//! go clean while the data it actually reads kept changing. A missing accessor
//! must be a compile error that the author resolves — by adding the accessor, or
//! by reaching for [`Rx::untracked`], which is honest about what it costs.

use core::cell::{Cell, RefCell};

use bevy::ecs::change_detection::Tick;
use bevy::ecs::component::ComponentId;
use bevy::prelude::*;

/// One recorded read. `existed` is whether the datum was present *at record
/// time*, which is what makes appearance and disappearance both count as
/// changes — a binding that read a missing resource must re-run when it shows
/// up, and Bevy has no tick to offer for something that is not there.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Dep {
    Resource {
        id: ComponentId,
        existed: bool,
    },
    Component {
        entity: Entity,
        id: ComponentId,
        existed: bool,
    },
}

impl Dep {
    /// Same key, ignoring `existed` — two reads of the same datum in one run are
    /// one dependency.
    fn same_slot(&self, other: &Dep) -> bool {
        match (self, other) {
            (Dep::Resource { id: a, .. }, Dep::Resource { id: b, .. }) => a == b,
            (
                Dep::Component {
                    entity: ea, id: ia, ..
                },
                Dep::Component {
                    entity: eb, id: ib, ..
                },
            ) => ea == eb && ia == ib,
            _ => false,
        }
    }

    fn is_dirty(&self, world: &World, last_run: Tick, this_run: Tick) -> bool {
        let (ticks, existed) = match *self {
            Dep::Resource { id, existed } => (world.get_resource_change_ticks_by_id(id), existed),
            Dep::Component { entity, id, existed } => (
                world
                    .get_entity(entity)
                    .ok()
                    .and_then(|e| e.get_change_ticks_by_id(id)),
                existed,
            ),
        };
        match ticks {
            // Present now: dirty if it appeared since, or was written since.
            Some(t) => !existed || t.changed.is_newer_than(last_run, this_run),
            // Absent now: dirty only if it used to be there.
            None => existed,
        }
    }
}

/// The dependencies one reaction recorded on its last run.
///
/// Default is empty, which per the module doc means **dirty** — so a freshly
/// registered reaction always runs at least once before it can ever be skipped.
#[derive(Default)]
pub struct DepSet {
    deps: Vec<Dep>,
    /// Set when a read happened that cannot be expressed as a tick lookup
    /// ([`Rx::untracked`], or a type with no `ComponentId` yet). Forces dirty.
    untracked: bool,
}

impl DepSet {
    /// How many distinct slots a reaction may record before it gives up and
    /// declares itself untracked.
    ///
    /// Sized for the shape it protects: `bind_*` closures read one to a
    /// handful, so nothing real is lost, while a `keyed_list` snapshot reading
    /// hundreds bails out after a bounded amount of work instead of paying an
    /// O(n²) dedup every frame.
    pub const MAX_DEPS: usize = 32;

    /// Number of distinct data slots this reaction reads. Diagnostics only.
    pub fn len(&self) -> usize {
        self.deps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.deps.is_empty()
    }

    /// True if this reaction opted out of tracking and must always run.
    pub fn is_untracked(&self) -> bool {
        self.untracked
    }

    /// Whether this reaction can be skipped this frame.
    ///
    /// The empty case answers `true` (dirty) — see the module doc; that single
    /// line is what makes a half-migrated codebase exactly as correct as an
    /// unmigrated one.
    pub fn is_dirty(&self, world: &World, last_run: Tick, this_run: Tick) -> bool {
        if self.untracked || self.deps.is_empty() {
            return true;
        }
        self.deps
            .iter()
            .any(|d| d.is_dirty(world, last_run, this_run))
    }

    /// Returns `true` if this push tipped the set past the cap, so the caller
    /// can mirror the bail-out into [`Rx::bailed`] and take the fast path from
    /// here on.
    fn push(&mut self, dep: Dep) -> bool {
        // Past the cap, stop recording and fall back to always-dirty.
        //
        // This exists because the linear dedup below is O(n²) in the number of
        // distinct slots read, and while that is free for a `bind_*` closure
        // (one to a handful) it is emphatically not for a `keyed_list`
        // snapshot, which reads across every row it builds. Measured in the
        // editor, tracking pushed keyed-list time from 0.10 ms to 1.33 ms a
        // frame — the gate cost more than the work it was gating.
        //
        // Giving up is the right answer rather than a cleverer container: a
        // closure reading this many distinct slots will have *something* among
        // them change on almost every frame, so it would be reported dirty
        // anyway. Capping buys back the bounded cost and lands exactly on the
        // pre-tracking behaviour for those closures, which is correct by the
        // empty/untracked rule.
        if self.untracked {
            return true;
        }
        if self.deps.len() >= Self::MAX_DEPS {
            self.untracked = true;
            // Free the vector: nothing will read it again, and `is_dirty`
            // short-circuits on `untracked` before it would be scanned.
            self.deps = Vec::new();
            return true;
        }
        // Linear scan: below the cap the set is small enough that this beats
        // hashing on both time and allocation.
        if !self.deps.iter().any(|d| d.same_slot(&dep)) {
            self.deps.push(dep);
        }
        false
    }
}

/// A tracking view of the world, handed to a reactive closure in place of
/// `&World`.
///
/// Every read through it is recorded. See the module doc for why the accessors
/// return world-lifetime references and why there is no `Deref`.
pub struct Rx<'w> {
    world: &'w World,
    deps: RefCell<DepSet>,
    /// Mirrors `DepSet::untracked`, kept deliberately *outside* the `RefCell`.
    ///
    /// Once a closure has bailed out past [`DepSet::MAX_DEPS`] — or reached for
    /// [`Rx::untracked`] — every later read is a plain world read and should
    /// cost nothing beyond the read itself. Testing the flag here is a bare
    /// `bool` load; testing it through the `RefCell` would add a borrow-flag
    /// increment, decrement and panic branch to every one of the hundreds of
    /// reads a `keyed_list` snapshot makes.
    bailed: Cell<bool>,
}

impl<'w> Rx<'w> {
    /// Build a tracking view over a world.
    ///
    /// Normally the reactive driver does this for you. It is public for one
    /// recurring case: a widget's *setter* closure takes `&mut World` and needs
    /// to call its own *getter*, which now takes `&Rx`. Wrapping there
    /// (`g(&Rx::new(&*world))`) is correct and costs nothing — the dep set is
    /// simply dropped, because a setter is not a reaction and has nothing to
    /// subscribe on its behalf.
    pub fn new(world: &'w World) -> Self {
        Self {
            world,
            deps: RefCell::new(DepSet::default()),
            bailed: Cell::new(false),
        }
    }

    /// Consume the view and take what it recorded.
    pub(crate) fn into_deps(self) -> DepSet {
        let mut deps = self.deps.into_inner();
        // Fold the hot-path mirror back in, so a bail-out that happened via
        // the `Cell` is visible to `is_dirty`.
        deps.untracked |= self.bailed.get();
        deps
    }

    /// Escape hatch: the untracked world.
    ///
    /// Recording is impossible past this point, so the reaction is marked
    /// always-dirty and behaves exactly as it does today. Use it when a closure
    /// hands the world to a helper that takes `&World`, or reads something the
    /// ECS cannot tick (an `Instant`, an `Arc`, the filesystem).
    ///
    /// It is deliberately not free of consequence and deliberately easy to
    /// grep for: every call is one binding that still burns its full cost every
    /// frame, and a good chunk of them are helpers that would track fine if the
    /// helper took an `&Rx` instead.
    pub fn untracked(&self) -> &'w World {
        self.bailed.set(true);
        self.deps.borrow_mut().untracked = true;
        self.world
    }

    /// Read a resource, recording a dependency on it.
    pub fn get_resource<R: Resource>(&self) -> Option<&'w R> {
        let world = self.world;
        // Already untracked: the answer cannot change, so skip the `TypeId`
        // lookup and the tick probe and just read. This is what keeps a
        // bailed-out snapshot running at native speed after the cap, rather
        // than paying a hashmap lookup per read for a set nobody will consult.
        if self.bailed.get() {
            return world.get_resource::<R>();
        }
        // `get_id`, not the 0.19-deprecated `resource_id` — resources and
        // components share one id space and both spell it this way underneath.
        match world.components().get_id(core::any::TypeId::of::<R>()) {
            Some(id) => {
                let v = world.get_resource::<R>();
                if self.deps.borrow_mut().push(Dep::Resource {
                    id,
                    existed: v.is_some(),
                }) {
                    self.bailed.set(true);
                }
                v
            }
            // No `ComponentId` means the type has never been registered, so
            // there is no slot to subscribe to. It may be registered later, so
            // stay conservative rather than caching a "clean" answer; once it
            // exists a subsequent run records a real dep and this self-heals.
            None => {
                self.deps.borrow_mut().untracked = true;
                None
            }
        }
    }

    /// Read a resource that must exist, recording a dependency on it.
    ///
    /// Panics identically to [`World::resource`].
    #[track_caller]
    pub fn resource<R: Resource>(&self) -> &'w R {
        match self.get_resource::<R>() {
            Some(v) => v,
            None => panic!(
                "requested resource {} does not exist",
                core::any::type_name::<R>()
            ),
        }
    }

    /// True if the resource exists, recording a dependency on its presence.
    pub fn contains_resource<R: Resource>(&self) -> bool {
        self.get_resource::<R>().is_some()
    }

    /// Read a component off an entity, recording a dependency on that exact
    /// `(entity, component)` slot.
    pub fn get<C: Component>(&self, entity: Entity) -> Option<&'w C> {
        let world = self.world;
        if self.bailed.get() {
            return world.get::<C>(entity);
        }
        match world.components().component_id::<C>() {
            Some(id) => {
                let v = world.get::<C>(entity);
                if self.deps.borrow_mut().push(Dep::Component {
                    entity,
                    id,
                    existed: v.is_some(),
                }) {
                    self.bailed.set(true);
                }
                v
            }
            None => {
                self.deps.borrow_mut().untracked = true;
                None
            }
        }
    }

    /// Declare a dependency on a resource named by a runtime [`ComponentId`],
    /// without reading it through this view.
    pub fn track_resource_id(&self, id: ComponentId) {
        if self.bailed.get() {
            return;
        }
        let existed = self.world.get_resource_change_ticks_by_id(id).is_some();
        if self.deps.borrow_mut().push(Dep::Resource { id, existed }) {
                    self.bailed.set(true);
                }
    }

    /// Declare a dependency on one entity's component, named by a runtime
    /// [`ComponentId`].
    ///
    /// This is the reflection case, and it is not a niche one: the inspector
    /// reads every field it shows by `(ComponentId, offset)` rather than by
    /// type, so `get::<C>` cannot describe what it depends on. Pair with
    /// [`Rx::manually_tracked`].
    pub fn track_component_id(&self, entity: Entity, id: ComponentId) {
        if self.bailed.get() {
            return;
        }
        let existed = self
            .world
            .get_entity(entity)
            .ok()
            .and_then(|e| e.get_change_ticks_by_id(id))
            .is_some();
        if self.deps.borrow_mut().push(Dep::Component {
            entity,
            id,
            existed,
        }) {
                    self.bailed.set(true);
                }
    }

    /// The raw world, *without* forcing the reaction dirty — on the explicit
    /// understanding that the caller has already declared every dependency by
    /// hand with the `track_*` methods above.
    ///
    /// Unlike [`Rx::untracked`] this keeps the reaction skippable, so it is the
    /// one place in the design where being wrong causes staleness rather than
    /// merely wasted work. Use it only where the reads are genuinely dynamic and
    /// the `track_*` call sits directly above it, so the two can be read
    /// together and cannot drift apart:
    ///
    /// ```ignore
    /// move |rx| {
    ///     rx.track_component_id(entity, cid);
    ///     read_f32(rx.manually_tracked(), entity, cid, offset)
    /// }
    /// ```
    ///
    /// If you are reaching for this to hand the world to a helper that could
    /// just as well take an `&Rx`, change the helper instead.
    pub fn manually_tracked(&self) -> &'w World {
        self.world
    }

    /// Whether the entity is alive.
    ///
    /// Marked untracked: entity liveness is not a component slot, so there is no
    /// tick to watch. A closure that only needs "does this still exist" is
    /// usually better served by reading the component it actually cares about,
    /// which does track.
    pub fn contains_entity(&self, entity: Entity) -> bool {
        self.deps.borrow_mut().untracked = true;
        self.world.get_entity(entity).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Counter(u32);

    #[derive(Resource, Default)]
    struct Other(u32);

    #[derive(Component, Default)]
    struct Label(u32);

    /// Read a resource, then bump an unrelated one: the reaction stays clean.
    /// This is the entire point of the module.
    #[test]
    fn an_unrelated_write_does_not_dirty() {
        let mut world = World::new();
        world.insert_resource(Counter(0));
        world.insert_resource(Other(0));

        let rx = Rx::new(&world);
        let _ = rx.resource::<Counter>();
        let deps = rx.into_deps();
        assert_eq!(deps.len(), 1);

        let last_run = world.change_tick();
        world.increment_change_tick();
        world.resource_mut::<Other>().0 += 1;
        let this_run = world.change_tick();
        assert!(
            !deps.is_dirty(&world, last_run, this_run),
            "writing an unread resource dirtied the reaction"
        );

        world.increment_change_tick();
        world.resource_mut::<Counter>().0 += 1;
        let this_run = world.change_tick();
        assert!(
            deps.is_dirty(&world, last_run, this_run),
            "writing the read resource did not dirty the reaction"
        );
    }

    /// A resource that was missing when read must dirty the reaction when it
    /// appears — otherwise a panel bound to an optional resource never wakes up.
    #[test]
    fn an_absent_resource_dirties_when_it_appears() {
        let mut world = World::new();
        // Register the type so there is a `ComponentId` to depend on; without
        // this the read is conservatively untracked, which is a different path.
        world.insert_resource(Counter(0));
        world.remove_resource::<Counter>();

        let rx = Rx::new(&world);
        assert!(rx.get_resource::<Counter>().is_none());
        let deps = rx.into_deps();
        assert!(!deps.is_untracked(), "a registered type should still track");

        let last_run = world.change_tick();
        world.increment_change_tick();
        let this_run = world.change_tick();
        assert!(!deps.is_dirty(&world, last_run, this_run), "still absent");

        world.insert_resource(Counter(7));
        world.increment_change_tick();
        let this_run = world.change_tick();
        assert!(
            deps.is_dirty(&world, last_run, this_run),
            "the resource appeared and the reaction did not notice"
        );
    }

    /// Component deps are per `(entity, component)`, so a write to a sibling
    /// entity's same-typed component must not dirty.
    #[test]
    fn component_deps_are_per_entity() {
        let mut world = World::new();
        let a = world.spawn(Label(1)).id();
        let b = world.spawn(Label(2)).id();

        let rx = Rx::new(&world);
        let _ = rx.get::<Label>(a);
        let deps = rx.into_deps();

        let last_run = world.change_tick();
        world.increment_change_tick();
        world.entity_mut(b).get_mut::<Label>().unwrap().0 = 99;
        let this_run = world.change_tick();
        assert!(
            !deps.is_dirty(&world, last_run, this_run),
            "a write to another entity dirtied the reaction"
        );

        world.increment_change_tick();
        world.entity_mut(a).get_mut::<Label>().unwrap().0 = 99;
        let this_run = world.change_tick();
        assert!(deps.is_dirty(&world, last_run, this_run));
    }

    /// Despawning the watched entity must dirty, not silently stay clean.
    #[test]
    fn a_despawned_entity_dirties() {
        let mut world = World::new();
        let a = world.spawn(Label(1)).id();

        let rx = Rx::new(&world);
        let _ = rx.get::<Label>(a);
        let deps = rx.into_deps();

        let last_run = world.change_tick();
        world.increment_change_tick();
        world.despawn(a);
        let this_run = world.change_tick();
        assert!(deps.is_dirty(&world, last_run, this_run));
    }

    /// The safety property: nothing recorded means always dirty.
    #[test]
    fn an_empty_dep_set_is_always_dirty() {
        let world = World::new();
        let deps = DepSet::default();
        let t = world.read_change_tick();
        assert!(
            deps.is_dirty(&world, t, t),
            "an empty dep set must be dirty, or an unmigrated binding would freeze"
        );
    }

    /// Reaching for the world directly opts the whole reaction out.
    #[test]
    fn untracked_forces_dirty_even_with_other_deps() {
        let mut world = World::new();
        world.insert_resource(Counter(0));

        let rx = Rx::new(&world);
        let _ = rx.resource::<Counter>();
        let _ = rx.untracked();
        let deps = rx.into_deps();
        assert!(deps.is_untracked());

        let last_run = world.change_tick();
        world.increment_change_tick();
        let this_run = world.change_tick();
        assert!(
            deps.is_dirty(&world, last_run, this_run),
            "an untracked read must pin the reaction dirty"
        );
    }

    /// A closure that reads more distinct slots than the cap gives up on
    /// tracking rather than paying an O(n²) dedup for a set that would report
    /// dirty every frame anyway.
    #[test]
    fn a_wide_reader_bails_out_instead_of_growing_without_bound() {
        let mut world = World::new();
        let entities: Vec<Entity> = (0..DepSet::MAX_DEPS * 4)
            .map(|i| world.spawn(Label(i as u32)).id())
            .collect();

        let rx = Rx::new(&world);
        for &e in &entities {
            let _ = rx.get::<Label>(e);
        }
        let deps = rx.into_deps();

        assert!(
            deps.is_untracked(),
            "a snapshot-shaped closure kept recording past the cap"
        );
        assert_eq!(deps.len(), 0, "the abandoned dep vector was not released");

        // Untracked means always dirty, which is exactly the pre-tracking
        // behaviour — bailing out can cost time, never correctness.
        let last_run = world.change_tick();
        world.increment_change_tick();
        let this_run = world.change_tick();
        assert!(deps.is_dirty(&world, last_run, this_run));
    }

    /// The pattern the inspector runs on: declare the slot by `ComponentId`,
    /// then read through the manual hatch.
    ///
    /// This is how a contract-crate `fn(&World, Entity)` — which cannot take an
    /// `&Rx` without the zero-dep crate depending on ember — stays skippable.
    /// The dep must behave exactly as a typed `get::<C>` would.
    #[test]
    fn a_manually_declared_component_dep_gates_like_a_typed_read() {
        let mut world = World::new();
        let watched = world.spawn(Label(1)).id();
        let other = world.spawn(Label(1)).id();
        world.insert_resource(Counter(0));
        let cid = world.components().component_id::<Label>().unwrap();

        let rx = Rx::new(&world);
        rx.track_component_id(watched, cid);
        // Stand-in for `(entry.get_fn)(world, entity)`.
        let _ = rx.manually_tracked().get::<Label>(watched);
        let deps = rx.into_deps();
        assert!(
            !deps.is_untracked(),
            "manually_tracked must not pin the reaction dirty — that is the \
             whole difference from untracked()"
        );
        assert_eq!(deps.len(), 1);

        let last_run = world.change_tick();

        // A sibling entity's same-typed component: irrelevant.
        world.increment_change_tick();
        world.entity_mut(other).get_mut::<Label>().unwrap().0 = 9;
        let now = world.change_tick();
        assert!(!deps.is_dirty(&world, last_run, now));

        // An unrelated resource: irrelevant.
        world.increment_change_tick();
        world.resource_mut::<Counter>().0 += 1;
        let now = world.change_tick();
        assert!(!deps.is_dirty(&world, last_run, now));

        // The declared slot: wakes it.
        world.increment_change_tick();
        world.entity_mut(watched).get_mut::<Label>().unwrap().0 = 9;
        let now = world.change_tick();
        assert!(deps.is_dirty(&world, last_run, now));
    }

    /// Reading the same slot twice is one dependency, not two.
    #[test]
    fn repeated_reads_dedupe() {
        let mut world = World::new();
        world.insert_resource(Counter(0));
        let e = world.spawn(Label(1)).id();

        let rx = Rx::new(&world);
        let _ = rx.resource::<Counter>();
        let _ = rx.resource::<Counter>();
        let _ = rx.get::<Label>(e);
        let _ = rx.get::<Label>(e);
        assert_eq!(rx.into_deps().len(), 2);
    }
}
