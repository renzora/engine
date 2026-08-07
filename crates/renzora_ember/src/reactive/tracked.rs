//! The dependency-tracked `bind_*` / `keyed_list` constructors.
//!
//! These are the same reactions as the ones in [`super`], with one difference:
//! the closure is handed an [`Rx`] instead of a `&World`, so every read it makes
//! is recorded and the reaction can be skipped on frames where none of the data
//! it read has changed.
//!
//! ## Why a separate module rather than a changed signature
//!
//! There are ~900 `bind_*` call sites across ~40 crates. Changing the closure
//! parameter type in place is the eventual destination, but it cannot be a
//! single edit: ~410 of those bodies hand `world` to a helper that takes
//! `&World`, and each one needs a decision (thread the `Rx` through the helper,
//! or accept [`Rx::untracked`]). A trait that accepts both signatures is not
//! available either — `impl<F: Fn(&World) -> V>` and `impl<F: Fn(&Rx) -> V>`
//! overlap from the compiler's point of view, so it is a coherence error rather
//! than a design choice.
//!
//! So migration is per-file and mechanical: swap
//!
//! ```ignore
//! use renzora_ember::reactive::{bind_text, bind_display};
//! ```
//!
//! for
//!
//! ```ignore
//! use renzora_ember::reactive::tracked::{bind_text, bind_display};
//! ```
//!
//! and fix whatever stops compiling. Bodies that only read through the
//! accessors compile verbatim. Bodies that do not, fail loudly — which is the
//! point, because those are exactly the ones where a wrong guess would matter.
//!
//! Until a file is migrated its bindings keep running every frame, exactly as
//! they do today. Mixing the two is safe by construction; see [`super::rx`].

use bevy::prelude::*;

use super::rx::{DepSet, Rx};
use super::{
    Bound, EntryMeta, KeyedSnapshot, PendingKeyedLists, ReactionEntry, ReactionOutcome,
    ReactionRegistry,
};

/// Tracked [`super::bind_with`]: recompute `value` only when something it read
/// last frame has changed, and write `target` only when the result differs.
pub fn bind_with<V, F, A>(commands: &mut Commands, target: Entity, value: F, apply: A)
where
    V: PartialEq + Send + Sync + 'static,
    F: for<'w> Fn(&Rx<'w>) -> V + Send + Sync + 'static,
    A: Fn(&mut World, Entity, &V) + Send + Sync + 'static,
{
    bind_with_kind(commands, target, "custom*", value, apply);
}

/// Shared body. `kind` carries a `*` suffix so the reactivity debug panel shows
/// at a glance which bindings are tracked and which are still legacy.
pub(crate) fn bind_with_kind<V, F, A>(
    commands: &mut Commands,
    target: Entity,
    kind: &'static str,
    value: F,
    apply: A,
) where
    V: PartialEq + Send + Sync + 'static,
    F: for<'w> Fn(&Rx<'w>) -> V + Send + Sync + 'static,
    A: Fn(&mut World, Entity, &V) + Send + Sync + 'static,
{
    commands.queue(move |world: &mut World| {
        let mut last: Option<V> = None;
        if let Some(mut reg) = world.get_resource_mut::<ReactionRegistry>() {
            reg.active.push(ReactionEntry::new(
                EntryMeta::new(Some(target), kind),
                Box::new(move |world: &mut World, deps: &mut DepSet| {
                    if world.get_entity(target).is_err() {
                        return ReactionOutcome::Dead;
                    }
                    // The recorded set is rebuilt from scratch every run, so a
                    // closure that branches onto different data (a match on some
                    // mode resource) re-subscribes correctly each time rather
                    // than accumulating dependencies it no longer reads.
                    let rx = Rx::new(world);
                    let v = value(&rx);
                    *deps = rx.into_deps();

                    if last.as_ref() != Some(&v) {
                        apply(world, target, &v);
                        last = Some(v);
                        ReactionOutcome::Changed
                    } else {
                        ReactionOutcome::Unchanged
                    }
                }),
            ));
        }
    });
}

/// Tracked [`super::bind_text`].
pub fn bind_text<F>(commands: &mut Commands, target: Entity, value: F)
where
    F: for<'w> Fn(&Rx<'w>) -> String + Send + Sync + 'static,
{
    bind_with_kind(commands, target, "text*", value, |world, target, v: &String| {
        if let Some(mut t) = world.get_mut::<Text>(target) {
            t.0.clone_from(v);
        }
    });
}

/// Tracked [`super::bind_text_color`].
pub fn bind_text_color<F>(commands: &mut Commands, target: Entity, value: F)
where
    F: for<'w> Fn(&Rx<'w>) -> Color + Send + Sync + 'static,
{
    bind_with_kind(commands, target, "color*", value, |world, target, v: &Color| {
        if let Some(mut c) = world.get_mut::<TextColor>(target) {
            c.0 = *v;
        }
    });
}

/// Tracked [`super::bind_bg`].
pub fn bind_bg<F>(commands: &mut Commands, target: Entity, value: F)
where
    F: for<'w> Fn(&Rx<'w>) -> Color + Send + Sync + 'static,
{
    bind_with_kind(commands, target, "bg*", value, |world, target, v: &Color| {
        if let Some(mut bg) = world.get_mut::<BackgroundColor>(target) {
            bg.0 = *v;
        }
    });
}

/// Tracked [`super::bind_display`].
///
/// This is the highest-count binding in the codebase (~274 sites) and almost
/// all of them read one bool off one resource, so it is the single biggest
/// beneficiary of tracking.
pub fn bind_display<F>(commands: &mut Commands, target: Entity, value: F)
where
    F: for<'w> Fn(&Rx<'w>) -> bool + Send + Sync + 'static,
{
    bind_with_kind(commands, target, "display*", value, |world, target, v: &bool| {
        if let Some(mut n) = world.get_mut::<Node>(target) {
            let d = if *v { Display::Flex } else { Display::None };
            if n.display != d {
                n.display = d;
            }
        }
    });
}

/// Tracked [`super::bind_2way`].
///
/// The widget's own [`Bound<T>`] is read **through the `Rx`** rather than
/// straight off the world. That is load-bearing rather than tidy: the user's
/// edit arrives as a write to `Bound<T>`, so if it were not a recorded
/// dependency the gate would find `get`'s data unchanged, skip the reaction and
/// drop the edit on the floor. Reading it through the tracker subscribes to the
/// widget as well as to the state, which is what a two-way binding means.
pub fn bind_2way<T, G, S>(commands: &mut Commands, target: Entity, get: G, set: S)
where
    T: PartialEq + Clone + Send + Sync + 'static,
    G: for<'w> Fn(&Rx<'w>) -> T + Send + Sync + 'static,
    S: Fn(&mut World, &T) + Send + Sync + 'static,
{
    commands.queue(move |world: &mut World| {
        // Seed the model from state if the widget doesn't already carry one.
        if world.get::<Bound<T>>(target).is_none() {
            let rx = Rx::new(&*world);
            let sv = get(&rx);
            drop(rx);
            if let Ok(mut em) = world.get_entity_mut(target) {
                em.insert(Bound(sv));
            }
        }
        let mut last: Option<T> = None;
        if let Some(mut reg) = world.get_resource_mut::<ReactionRegistry>() {
            reg.active.push(ReactionEntry::new(
                EntryMeta::new(Some(target), "2way*"),
                Box::new(move |world: &mut World, deps: &mut DepSet| {
                    if world.get_entity(target).is_err() {
                        return ReactionOutcome::Dead;
                    }
                    let rx = Rx::new(&*world);
                    let sv = get(&rx);
                    let bound = rx.get::<Bound<T>>(target).map(|b| b.0.clone());
                    *deps = rx.into_deps();

                    if last.as_ref() != Some(&sv) {
                        // First run, or state changed externally → model ← state.
                        if let Some(mut b) = world.get_mut::<Bound<T>>(target) {
                            if b.0 != sv {
                                b.0 = sv.clone();
                            }
                        }
                        last = Some(sv);
                        ReactionOutcome::Changed
                    } else if let Some(bv) = bound {
                        // State stable; the user edited the widget → state ← model.
                        if bv != sv {
                            set(world, &bv);
                            last = Some(bv);
                            ReactionOutcome::Changed
                        } else {
                            ReactionOutcome::Unchanged
                        }
                    } else {
                        ReactionOutcome::Unchanged
                    }
                }),
            ));
        }
    });
}

/// Tracked [`super::keyed_list_tokened`].
///
/// Both gates apply, and they are not redundant. The dep gate answers "did
/// anything the snapshot reads get written"; the token answers "did the values
/// actually come out different". A resource rewritten every frame with
/// unchanged contents is dirty by the first test and clean by the second, which
/// is exactly the case `virtual_scroll` uses this for.
///
/// The token's own reads are not recorded — it is an extra filter layered on
/// the snapshot's dependencies, not a replacement for them.
pub fn keyed_list_tokened<T, F>(commands: &mut Commands, container: Entity, token: T, snapshot: F)
where
    T: for<'w> Fn(&Rx<'w>) -> u64 + Send + Sync + 'static,
    F: for<'w> Fn(&Rx<'w>) -> KeyedSnapshot + Send + Sync + 'static,
{
    commands.queue(move |world: &mut World| {
        world
            .get_resource_or_insert_with(PendingKeyedLists::default)
            .0
            .push(super::KeyedList::new_tracked_tokened(
                container,
                Box::new(move |world: &World| token(&Rx::new(world))),
                Box::new(move |world: &World, deps: &mut DepSet| {
                    let rx = Rx::new(world);
                    let snap = snapshot(&rx);
                    *deps = rx.into_deps();
                    snap
                }),
            ));
    });
}

/// Tracked [`super::keyed_list`].
///
/// This subsumes [`super::keyed_list_tokened`] for most callers: the hand-rolled
/// dirty token exists precisely because there was no way to know whether the
/// snapshot's inputs had changed, and tracking answers that question directly.
/// The tokened form still earns its place where the "did anything change" signal
/// is *not* an ECS read — a scroll window computed from layout, say.
pub fn keyed_list<F>(commands: &mut Commands, container: Entity, snapshot: F)
where
    F: for<'w> Fn(&Rx<'w>) -> KeyedSnapshot + Send + Sync + 'static,
{
    commands.queue(move |world: &mut World| {
        world
            .get_resource_or_insert_with(PendingKeyedLists::default)
            .0
            .push(super::KeyedList::new_tracked(
                container,
                Box::new(move |world: &World, deps: &mut DepSet| {
                    let rx = Rx::new(world);
                    let snap = snapshot(&rx);
                    *deps = rx.into_deps();
                    snap
                }),
            ));
    });
}
