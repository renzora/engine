//! A small SolidJS-style reactive layer for ember UI.
//!
//! The "signals" are ECS data (resources/components); **bindings** are effects
//! that read that data and write a node property, recomputing each frame but
//! **only writing when the computed value actually changed** (value-diffed). That
//! single trick makes them robust to resources that are dirtied every frame with
//! unchanged content — no per-panel "rebuild gate" needed. **Keyed lists**
//! (`keyed_list`) are the `<For>` equivalent: only changed/added/removed rows are
//! touched, never a full rebuild.
//!
//! A panel's `build` runs **once** (lay out the shell, declare bindings + lists);
//! everything after is driven granularly by [`run_reactions`] / [`run_keyed_lists`].
//!
//! Bindings/list-items auto-drop when their target entity despawns.

use bevy::ecs::world::CommandQueue;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use crate::font::EmberFonts;

/// Registers the reactive drivers. Added by [`crate::EmberPlugin`].
pub struct ReactivePlugin;

impl Plugin for ReactivePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ReactionRegistry>()
            .init_resource::<KeyedListRegistry>()
            .add_systems(Update, (run_reactions, run_keyed_lists));
    }
}

// ── Bindings (effects) ───────────────────────────────────────────────────────

/// A reaction: returns `false` once its target is gone (so it's dropped).
type Reaction = Box<dyn FnMut(&mut World) -> bool + Send + Sync>;

#[derive(Resource, Default)]
pub struct ReactionRegistry(Vec<Reaction>);

/// Run every binding; apply on change; drop dead ones. Exclusive so bindings can
/// read arbitrary world data and write their target node.
pub(crate) fn run_reactions(world: &mut World) {
    world.resource_scope(|world, mut reg: Mut<ReactionRegistry>| {
        reg.0.retain_mut(|r| r(world));
    });
}

/// Generic binding: recompute `value` each frame and, when it differs from last
/// frame, `apply` it to `target`. The named `bind_*` helpers are thin wrappers
/// over this; use it directly to bind any node property without a named helper.
/// Registered (deferred) via `commands`; auto-dropped when `target` despawns.
pub fn bind_with<V, F, A>(commands: &mut Commands, target: Entity, value: F, apply: A)
where
    V: PartialEq + Send + Sync + 'static,
    F: Fn(&World) -> V + Send + Sync + 'static,
    A: Fn(&mut World, Entity, &V) + Send + Sync + 'static,
{
    commands.queue(move |world: &mut World| {
        let mut last: Option<V> = None;
        if let Some(mut reg) = world.get_resource_mut::<ReactionRegistry>() {
            reg.0.push(Box::new(move |world: &mut World| {
                if world.get_entity(target).is_err() {
                    return false;
                }
                let v = value(world);
                if last.as_ref() != Some(&v) {
                    apply(world, target, &v);
                    last = Some(v);
                }
                true
            }));
        }
    });
}

/// Bind a node's [`Text`] to a computed string.
pub fn bind_text<F>(commands: &mut Commands, target: Entity, value: F)
where
    F: Fn(&World) -> String + Send + Sync + 'static,
{
    bind_with(commands, target, value, |world, target, v: &String| {
        if let Some(mut t) = world.get_mut::<Text>(target) {
            t.0.clone_from(v);
        }
    });
}

/// Bind a node's [`TextColor`] to a computed color.
pub fn bind_text_color<F>(commands: &mut Commands, target: Entity, value: F)
where
    F: Fn(&World) -> Color + Send + Sync + 'static,
{
    bind_with(commands, target, value, |world, target, v: &Color| {
        if let Some(mut c) = world.get_mut::<TextColor>(target) {
            c.0 = *v;
        }
    });
}

/// Bind a node's [`BackgroundColor`] to a computed color.
pub fn bind_bg<F>(commands: &mut Commands, target: Entity, value: F)
where
    F: Fn(&World) -> Color + Send + Sync + 'static,
{
    bind_with(commands, target, value, |world, target, v: &Color| {
        if let Some(mut bg) = world.get_mut::<BackgroundColor>(target) {
            bg.0 = *v;
        }
    });
}

/// Bind a node's visibility (`true` = `Display::Flex`, `false` = `Display::None`).
pub fn bind_display<F>(commands: &mut Commands, target: Entity, value: F)
where
    F: Fn(&World) -> bool + Send + Sync + 'static,
{
    bind_with(commands, target, value, |world, target, v: &bool| {
        if let Some(mut n) = world.get_mut::<Node>(target) {
            let d = if *v { Display::Flex } else { Display::None };
            if n.display != d {
                n.display = d;
            }
        }
    });
}

// ── Keyed list (<For>) ───────────────────────────────────────────────────────

/// A snapshot of the list this frame: one `(key, content-hash)` per item (cheap
/// to diff), plus a `build` closure that owns the data and builds the i-th item.
pub struct KeyedSnapshot {
    /// `(stable key, content hash)` for each item, in display order.
    pub items: Vec<(u64, u64)>,
    /// Build the item at index `i` (data is captured in the closure).
    pub build: Box<dyn Fn(&mut Commands, &EmberFonts, usize) -> Entity + Send + Sync>,
}

struct KeyedList {
    container: Entity,
    /// key -> (content hash, child entity)
    current: HashMap<u64, (u64, Entity)>,
    /// `(key, hash)` in display order — for a cheap "nothing changed" check.
    order: Vec<(u64, u64)>,
    snapshot: Box<dyn Fn(&World) -> KeyedSnapshot + Send + Sync>,
}

#[derive(Resource, Default)]
pub struct KeyedListRegistry(Vec<KeyedList>);

/// A keyed, granular child list (`<For>`): rebuild only changed rows, add new,
/// remove gone, reorder — never a full-list rebuild. `snapshot` returns this
/// frame's `(key, hash)` order + a builder; a row rebuilds only when its hash
/// changes. Registered (deferred) via `commands`.
pub fn keyed_list<F>(commands: &mut Commands, container: Entity, snapshot: F)
where
    F: Fn(&World) -> KeyedSnapshot + Send + Sync + 'static,
{
    commands.queue(move |world: &mut World| {
        if let Some(mut reg) = world.get_resource_mut::<KeyedListRegistry>() {
            reg.0.push(KeyedList {
                container,
                current: HashMap::default(),
                order: Vec::new(),
                snapshot: Box::new(snapshot),
            });
        }
    });
}

pub(crate) fn run_keyed_lists(world: &mut World) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    world.resource_scope(|world, mut reg: Mut<KeyedListRegistry>| {
        reg.0.retain_mut(|kl| {
            if world.get_entity(kl.container).is_err() {
                return false;
            }
            let snap = (kl.snapshot)(world);
            // Cheap fast-path: same keys + hashes in the same order → nothing to do.
            if snap.items == kl.order {
                return true;
            }

            let mut queue = CommandQueue::default();
            let mut next: HashMap<u64, (u64, Entity)> = HashMap::default();
            let mut ordered: Vec<Entity> = Vec::with_capacity(snap.items.len());
            {
                let mut commands = Commands::new(&mut queue, world);
                for (i, &(key, hash)) in snap.items.iter().enumerate() {
                    match kl.current.get(&key) {
                        Some(&(h, e)) if h == hash => {
                            next.insert(key, (h, e));
                            ordered.push(e);
                        }
                        Some(&(_, old)) => {
                            commands.entity(old).despawn();
                            let e = (snap.build)(&mut commands, &fonts, i);
                            next.insert(key, (hash, e));
                            ordered.push(e);
                        }
                        None => {
                            let e = (snap.build)(&mut commands, &fonts, i);
                            next.insert(key, (hash, e));
                            ordered.push(e);
                        }
                    }
                }
                // Despawn rows whose key vanished.
                for (k, &(_, e)) in kl.current.iter() {
                    if !next.contains_key(k) {
                        commands.entity(e).despawn();
                    }
                }
                // Set the container's children to the new order (moves existing,
                // attaches newly-built ones).
                if !ordered.is_empty() {
                    commands.entity(kl.container).insert_children(0, &ordered);
                }
            }
            queue.apply(world);
            kl.current = next;
            kl.order = snap.items;
            true
        });
    });
}
