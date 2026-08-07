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
//!
//! ## Instrumentation
//!
//! Every binding and keyed list carries per-entry counters (runs, value
//! changes, smoothed recompute cost) that [`run_reactions`] /
//! [`run_keyed_lists`] aggregate into the public [`ReactiveStats`] resource —
//! the data source for the editor's "UI Reactivity" debug panel. The
//! overhead is two `Instant` reads per entry per frame (tens of ns each).

use std::time::Instant;

use bevy::ecs::change_detection::{CheckChangeTicks, Tick};
use bevy::ecs::world::CommandQueue;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use crate::font::EmberFonts;

pub mod rx;
pub mod tracked;

#[cfg(test)]
mod bench;
#[cfg(test)]
mod tests;

pub use rx::{DepSet, Rx};

/// Registers the reactive drivers. Added by [`crate::EmberPlugin`].
pub struct ReactivePlugin;

impl Plugin for ReactivePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ReactionRegistry>()
            .init_resource::<KeyedListRegistry>()
            .init_resource::<PendingKeyedLists>()
            .init_resource::<ReactiveStats>()
            // Chained: run_reactions resets the per-frame stats that
            // run_keyed_lists then adds to.
            .add_systems(Update, (run_reactions, run_keyed_lists).chain());
        app.add_observer(clamp_stored_ticks);
    }
}

/// Keep our parked `last_run` ticks inside the window `Tick::is_newer_than` can
/// reason about.
///
/// Bevy's tick counter is a `u32` that wraps, so it periodically walks every
/// component, resource and system and clamps any tick that has drifted further
/// than `MAX_CHANGE_AGE` into the past. It cannot know about the `last_run`
/// ticks sitting in our two registries. Left alone, a long-lived reaction on a
/// quiet dependency would eventually have its `last_run` fall outside that
/// window, at which point the comparison saturates and the reaction reports
/// **clean forever** — a panel that silently stops updating after the editor has
/// been open long enough. Riding the same notification is the whole fix.
fn clamp_stored_ticks(
    check: On<CheckChangeTicks>,
    reactions: Option<ResMut<ReactionRegistry>>,
    lists: Option<ResMut<KeyedListRegistry>>,
) {
    if let Some(mut reactions) = reactions {
        for entry in reactions.iter_all_mut() {
            entry.last_run.check_tick(*check);
        }
    }
    if let Some(mut lists) = lists {
        for kl in &mut lists.0 {
            kl.last_run.check_tick(*check);
        }
    }
}

// ── Stats ────────────────────────────────────────────────────────────────────

/// One binding's row in the [`ReactiveStats`] top-N reports.
#[derive(Clone, Debug)]
pub struct BindingReport {
    /// Nearest `Name` up the target's ancestor chain, plus the target entity id.
    pub label: String,
    /// Which `bind_*` helper registered it ("text", "bg", "2way", "raw", …).
    pub kind: &'static str,
    /// Smoothed recompute cost, µs per frame (EMA).
    pub cost_ema_us: f32,
    /// Value changes per second over the last ~1s window.
    pub change_rate: f32,
    /// Total value changes since registration.
    pub changes: u64,
}

/// One keyed list's row in the [`ReactiveStats`] report.
#[derive(Clone, Debug)]
pub struct ListReport {
    /// Nearest `Name` up the container's ancestor chain + container entity id.
    pub label: String,
    /// Row count after the last run.
    pub rows: usize,
    /// Smoothed snapshot cost, µs per frame (EMA). The snapshot closure runs
    /// every frame even when nothing changed — this is the number to watch.
    pub cost_ema_us: f32,
    /// Total rows built/rebuilt since registration.
    pub rows_rebuilt: u64,
}

/// Live reactivity diagnostics, updated every frame by the reactive drivers.
/// Read by the "UI Reactivity" debug panel; available to any system.
#[derive(Resource, Default)]
pub struct ReactiveStats {
    /// Frames counted by `run_reactions` since startup.
    pub frame: u64,
    /// Bindings walked this frame. Excludes parked ones.
    pub bindings_total: usize,
    /// Bindings set aside behind a collapsed subtree — alive and restorable,
    /// but out of the per-frame walk entirely. A collapsed 200-row section
    /// moves 200 entries here and costs one `Node` lookup a frame instead.
    pub parked_total: usize,
    /// Bindings whose recompute produced a *new* value this frame (i.e. a UI
    /// write actually happened).
    pub changed_this_frame: usize,
    /// Bindings the dependency gate skipped without running this frame —
    /// nothing they read had changed. The headline number for tracking: against
    /// `bindings_total` it is the fraction of the old per-frame cost that is no
    /// longer being paid.
    pub skipped_this_frame: usize,
    /// Total binding recompute time this frame, µs.
    pub reactions_us: f32,
    /// Registered keyed lists currently alive.
    pub lists_total: usize,
    /// Total keyed-list snapshot+diff time this frame, µs.
    pub lists_us: f32,
    /// List rows built or rebuilt this frame.
    pub rows_rebuilt_this_frame: usize,
    /// Binding value-changes per second over the last ~1s window.
    pub changes_per_sec: f32,
    /// Recent total recompute time per frame (`reactions_us + lists_us`),
    /// oldest → newest, capped at [`Self::HISTORY_LEN`]. Chart fodder.
    pub history_us: Vec<f32>,
    /// Top bindings by smoothed recompute cost. Rebuilt every 30 frames.
    pub top_cost: Vec<BindingReport>,
    /// Top bindings by value-change rate ("churn") — bindings whose computed
    /// value keeps coming back different. Rebuilt every 30 frames.
    pub top_churn: Vec<BindingReport>,
    /// All keyed lists, sorted by snapshot cost. Rebuilt every 30 frames.
    pub list_reports: Vec<ListReport>,
    /// Internal: seconds accumulated toward the next change-rate window roll.
    window_elapsed: f32,
}

impl ReactiveStats {
    pub const HISTORY_LEN: usize = 240;
    pub const TOP_N: usize = 12;
}

/// Per-entry counters shared by bindings and keyed lists.
struct EntryMeta {
    /// The bound node (bindings) or list container — label/liveness anchor.
    target: Option<Entity>,
    kind: &'static str,
    runs: u64,
    changes: u64,
    cost_ema_us: f32,
    /// Changes accumulated in the current ~1s rate window.
    changes_window: u32,
    /// Changes/sec measured over the last completed window.
    change_rate: f32,
    /// Frames the dependency gate skipped this entry outright. Paired with
    /// `runs`, this is the number that says whether tracking is earning its
    /// keep for a given binding — a binding with a high `runs` and a zero
    /// `skips` is either untracked or genuinely churning.
    skips: u64,
}

impl EntryMeta {
    fn new(target: Option<Entity>, kind: &'static str) -> Self {
        Self {
            target,
            kind,
            runs: 0,
            changes: 0,
            cost_ema_us: 0.0,
            changes_window: 0,
            change_rate: 0.0,
            skips: 0,
        }
    }

    /// A frame the dependency gate answered "clean". Costs no time worth
    /// measuring, so it is counted rather than timed.
    fn record_skip(&mut self) {
        self.skips += 1;
    }

    fn record(&mut self, us: f32, changed: bool) {
        self.runs += 1;
        if changed {
            self.changes += 1;
            self.changes_window += 1;
        }
        // EMA with a ~20-frame horizon; first run seeds directly.
        self.cost_ema_us = if self.runs == 1 {
            us
        } else {
            self.cost_ema_us * 0.95 + us * 0.05
        };
    }

    fn roll_window(&mut self, elapsed_secs: f32) {
        self.change_rate = self.changes_window as f32 / elapsed_secs.max(1e-3);
        self.changes_window = 0;
    }
}

/// `label` for a report row: nearest `Name` walking up the ancestor chain,
/// suffixed with the entity id so identical names stay distinguishable.
fn entry_label(world: &World, target: Option<Entity>) -> String {
    let Some(target) = target else {
        return "(world)".to_string();
    };
    let mut e = target;
    for _ in 0..10 {
        if let Some(name) = world.get::<Name>(e) {
            return format!("{name} ({target})");
        }
        match world.get::<ChildOf>(e) {
            Some(c) => e = c.parent(),
            None => break,
        }
    }
    format!("(unnamed) {target}")
}

/// True if a binding/list whose node is `node` should be skipped because it
/// lives in a hidden dock tab — i.e. some **ancestor** is collapsed
/// (`Display::None`). Inactive panes aren't laid out or painted, so recomputing
/// their bindings/lists is pure waste — and it was real waste: a backgrounded
/// heavy panel (e.g. the asset browser hashing a big folder) kept dragging the
/// frame rate down even after switching away from it.
///
/// Only *ancestors* are checked, never `node` itself: a binding may toggle its
/// own target's `Display` (e.g. `bind_display`), and skipping it when its own
/// node is collapsed would strand it hidden forever — it could never run to
/// un-hide itself.
///
/// `cache` memoizes results for one frame so shared ancestors (a panel's whole
/// subtree resolves to the same answer) aren't re-walked per binding.
/// Returns **which** ancestor is collapsed, not merely whether one is.
///
/// The identity matters because a hidden entry is parked under that entity and
/// only reconsidered when that one entity reopens — see [`ReactionRegistry`].
fn collapsed_ancestor(
    world: &World,
    node: Entity,
    cache: &mut HashMap<Entity, Option<Entity>>,
) -> Option<Entity> {
    let parent = world.get::<ChildOf>(node).map(|c| c.parent())?;
    in_collapsed_subtree(world, parent, cache)
}

/// The nearest `Display::None` ancestor at or above `start`, if any. Memoized
/// per frame; a despawned `start` has no `Node`/parent and resolves to `None`.
fn in_collapsed_subtree(
    world: &World,
    start: Entity,
    cache: &mut HashMap<Entity, Option<Entity>>,
) -> Option<Entity> {
    let mut path: Vec<Entity> = Vec::new();
    let mut e = start;
    let result = loop {
        if let Some(&v) = cache.get(&e) {
            break v;
        }
        path.push(e);
        let collapsed = world
            .get::<bevy::ui::Node>(e)
            .is_some_and(|n| n.display == bevy::ui::Display::None);
        if collapsed {
            break Some(e);
        }
        match world.get::<ChildOf>(e) {
            Some(c) => e = c.parent(),
            None => break None,
        }
    };
    for p in path {
        cache.insert(p, result);
    }
    result
}

// ── Bindings (effects) ───────────────────────────────────────────────────────

/// What one reaction run did — drives liveness and the change counters.
enum ReactionOutcome {
    /// Target despawned; drop the reaction.
    Dead,
    /// Recomputed; value identical to last frame, nothing written.
    Unchanged,
    /// Recomputed to a new value and applied it.
    Changed,
}

type ReactionFn = Box<dyn FnMut(&mut World, &mut DepSet) -> ReactionOutcome + Send + Sync>;

struct ReactionEntry {
    /// Runs the reaction and overwrites `deps` with what it read. A legacy
    /// `Fn(&World)` binding cannot record anything, so it simply leaves the set
    /// alone — and an empty set means dirty, so it keeps running every frame
    /// exactly as before. See [`rx`] for why that default is the safe one.
    f: ReactionFn,
    meta: EntryMeta,
    /// What the last run read. Checked against Bevy's change ticks to decide
    /// whether this frame's run can be skipped outright.
    deps: DepSet,
    /// World change tick as of the end of the last run — the `last_run` half of
    /// [`Tick::is_newer_than`], giving these reactions the same change-detection
    /// semantics an ordinary Bevy system gets.
    last_run: Tick,
}

impl ReactionEntry {
    fn new(meta: EntryMeta, f: ReactionFn) -> Self {
        Self {
            f,
            meta,
            // Empty ⇒ dirty ⇒ every reaction runs at least once before it can
            // ever be skipped, which is what seeds `deps` in the first place.
            deps: DepSet::default(),
            last_run: Tick::new(0),
        }
    }
}

/// Live bindings, plus the ones set aside while their UI is collapsed.
///
/// ## Why parked rather than dropped
///
/// A collapsed subtree is *hidden*, not gone: `bind_display` flips
/// `Node::display` and the children keep their entities. There are ~357 of those
/// toggles, and nothing rebuilds a subtree when it reopens — a panel's `build`
/// runs once. So dropping a hidden binding would free the only copy of its
/// closure and the section would come back permanently stale. Dropping is
/// correct exactly when the target is *despawned*, which is a different
/// question and handled separately.
///
/// Parking gets the same result the drop was after — a hidden binding is out of
/// the per-frame walk entirely — without losing the closure.
///
/// ## Why keyed by the collapsed ancestor
///
/// The obvious shape, one flat list of parked entries, would still cost an
/// ancestor walk per entry per frame to notice a reopen, which is what the skip
/// already cost. Filing entries under the entity that *caused* the hiding turns
/// that into one `Node` lookup per distinct collapsed root — typically a handful
/// — no matter how many bindings are behind them. Collapsing a 200-row section
/// then costs one lookup a frame instead of 200.
#[derive(Resource, Default)]
pub struct ReactionRegistry {
    active: Vec<ReactionEntry>,
    /// `collapsed ancestor -> entries hidden behind it`.
    parked: HashMap<Entity, Vec<ReactionEntry>>,
}

impl ReactionRegistry {
    /// Move entries back to `active` when the ancestor that hid them has
    /// reopened — or has been despawned, in which case the active pass's
    /// liveness check is what should decide their fate, not this map.
    ///
    /// Also sweeps parked entries for dead targets on a slow cadence. A reopen
    /// normally covers that, but a row despawned *while* its section is
    /// collapsed would otherwise sit here until the section is next opened,
    /// which may be never.
    fn unpark_revealed(&mut self, world: &World, sweep_dead: bool) {
        let mut revive: Vec<ReactionEntry> = Vec::new();
        self.parked.retain(|&anchor, entries| {
            let still_hidden = world
                .get_entity(anchor)
                .is_ok_and(|_| {
                    world
                        .get::<bevy::ui::Node>(anchor)
                        .is_some_and(|n| n.display == bevy::ui::Display::None)
                });
            if !still_hidden {
                revive.append(entries);
                return false;
            }
            if sweep_dead {
                entries.retain(|e| {
                    e.meta
                        .target
                        .is_none_or(|t| world.get_entity(t).is_ok())
                });
            }
            !entries.is_empty()
        });
        self.active.append(&mut revive);
    }

    fn park(&mut self, anchor: Entity, entry: ReactionEntry) {
        self.parked.entry(anchor).or_default().push(entry);
    }

    /// Bindings currently being walked each frame.
    pub fn active_len(&self) -> usize {
        self.active.len()
    }

    /// Bindings set aside behind a collapsed subtree.
    pub fn parked_len(&self) -> usize {
        self.parked.values().map(Vec::len).sum()
    }

    /// Every entry, parked or not.
    ///
    /// Only the `mut` direction exists: the one caller is the tick clamp, which
    /// must reach parked entries too — a `last_run` left unclamped behind a
    /// collapsed section would compare wrongly against the wrapped tick when the
    /// section reopens. Read-only walks (the debug panel's reports) deliberately
    /// cover the active set alone, since a parked binding has no current value to
    /// report.
    fn iter_all_mut(&mut self) -> impl Iterator<Item = &mut ReactionEntry> {
        self.active
            .iter_mut()
            .chain(self.parked.values_mut().flatten())
    }
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
    bind_with_kind(commands, target, "custom", value, apply);
}

fn bind_with_kind<V, F, A>(
    commands: &mut Commands,
    target: Entity,
    kind: &'static str,
    value: F,
    apply: A,
) where
    V: PartialEq + Send + Sync + 'static,
    F: Fn(&World) -> V + Send + Sync + 'static,
    A: Fn(&mut World, Entity, &V) + Send + Sync + 'static,
{
    commands.queue(move |world: &mut World| {
        let mut last: Option<V> = None;
        if let Some(mut reg) = world.get_resource_mut::<ReactionRegistry>() {
            reg.active.push(ReactionEntry::new(
                EntryMeta::new(Some(target), kind),
                Box::new(move |world: &mut World, _deps: &mut DepSet| {
                    if world.get_entity(target).is_err() {
                        return ReactionOutcome::Dead;
                    }
                    let v = value(world);
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

/// Register a raw reaction: a closure run every frame with `&mut World` that
/// returns `false` once it should be dropped. This is the low-level escape hatch
/// the `bind_*` helpers build on; widgets use it to implement two-way bindings
/// (read a widget's value and write it back to state, or vice-versa). Registered
/// (deferred) via `commands`.
///
/// Raw reactions can't report value changes, so they show up in
/// [`ReactiveStats`] with cost but zero churn.
///
/// **Runs even when its pane is a hidden dock tab.** With no target entity there
/// is nothing to locate it in the UI tree, so [`run_reactions`]' hidden-pane skip
/// cannot apply, and it also labels as `"(world)"` in the debug panel. If the
/// reaction belongs to a widget, prefer [`react_anchored`].
pub fn react<F>(commands: &mut Commands, reaction: F)
where
    F: FnMut(&mut World) -> bool + Send + Sync + 'static,
{
    react_inner(commands, None, reaction);
}

/// [`react`], but anchored to a widget entity so it participates in the
/// hidden-pane skip and reports against a real name in the debug panel.
///
/// Use this whenever the reaction only matters while its widget is on screen —
/// text inputs and colour pickers were each cloning several `String`s per frame
/// for panes the user could not see. Do **not** use it for work that must keep
/// running while its panel is backgrounded (export progress, background loads):
/// anchoring those would silently pause them.
///
/// The anchor is also the liveness handle — the reaction is dropped when the
/// entity despawns, exactly like a `bind_*`.
pub fn react_anchored<F>(commands: &mut Commands, anchor: Entity, reaction: F)
where
    F: FnMut(&mut World) -> bool + Send + Sync + 'static,
{
    react_inner(commands, Some(anchor), reaction);
}

fn react_inner<F>(commands: &mut Commands, anchor: Option<Entity>, reaction: F)
where
    F: FnMut(&mut World) -> bool + Send + Sync + 'static,
{
    commands.queue(move |world: &mut World| {
        if let Some(mut reg) = world.get_resource_mut::<ReactionRegistry>() {
            let mut reaction = reaction;
            reg.active.push(ReactionEntry::new(
                EntryMeta::new(anchor, "raw"),
                Box::new(move |world: &mut World, _deps: &mut DepSet| {
                    if reaction(world) {
                        ReactionOutcome::Unchanged
                    } else {
                        ReactionOutcome::Dead
                    }
                }),
            ));
        }
    });
}

/// A widget's bound model value — the "signal" a user input edits and a binding
/// keeps in sync with state. Interactive widgets carry `Bound<T>` (e.g.
/// `Bound<f32>` on a fader/knob/slider, `Bound<bool>` on a toggle/checkbox):
/// their input system writes it, and a small per-widget system mirrors it to the
/// visuals. [`bind_2way`] is the generic glue to a piece of state.
#[derive(Component)]
pub struct Bound<T: Send + Sync + 'static>(pub T);

/// Two-way-bind any widget that carries a [`Bound<T>`] to a piece of state.
/// `get` reads the state value each frame; `set` writes the user's edit back.
/// Value-diffed in both directions (no feedback loop): an external state change
/// wins ties, otherwise the user's edit propagates to state. Generic over the
/// model type, so one function serves every interactive widget — the widget owns
/// only "input → `Bound`" and "`Bound` → visuals".
pub fn bind_2way<T, G, S>(commands: &mut Commands, target: Entity, get: G, set: S)
where
    T: PartialEq + Clone + Send + Sync + 'static,
    G: Fn(&World) -> T + Send + Sync + 'static,
    S: Fn(&mut World, &T) + Send + Sync + 'static,
{
    commands.queue(move |world: &mut World| {
        // Seed the model from state if the widget doesn't already carry one.
        if world.get::<Bound<T>>(target).is_none() {
            let sv = get(world);
            if let Ok(mut em) = world.get_entity_mut(target) {
                em.insert(Bound(sv));
            }
        }
        let mut last: Option<T> = None;
        if let Some(mut reg) = world.get_resource_mut::<ReactionRegistry>() {
            reg.active.push(ReactionEntry::new(
                EntryMeta::new(Some(target), "2way"),
                Box::new(move |world: &mut World, _deps: &mut DepSet| {
                    if world.get_entity(target).is_err() {
                        return ReactionOutcome::Dead;
                    }
                    let sv = get(world);
                    if last.as_ref() != Some(&sv) {
                        // First run, or state changed externally → model ← state.
                        if let Some(mut b) = world.get_mut::<Bound<T>>(target) {
                            if b.0 != sv {
                                b.0 = sv.clone();
                            }
                        }
                        last = Some(sv);
                        ReactionOutcome::Changed
                    } else if let Some(bv) = world.get::<Bound<T>>(target).map(|b| b.0.clone()) {
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

/// Bind a node's [`Text`] to a computed string.
pub fn bind_text<F>(commands: &mut Commands, target: Entity, value: F)
where
    F: Fn(&World) -> String + Send + Sync + 'static,
{
    bind_with_kind(commands, target, "text", value, |world, target, v: &String| {
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
    bind_with_kind(commands, target, "color", value, |world, target, v: &Color| {
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
    bind_with_kind(commands, target, "bg", value, |world, target, v: &Color| {
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
    bind_with_kind(commands, target, "display", value, |world, target, v: &bool| {
        if let Some(mut n) = world.get_mut::<Node>(target) {
            let d = if *v { Display::Flex } else { Display::None };
            if n.display != d {
                n.display = d;
            }
        }
    });
}

/// Run every binding; apply on change; drop dead ones. Exclusive so bindings can
/// read arbitrary world data and write their target node. Also owns the
/// [`ReactiveStats`] frame bookkeeping (counter reset, rate windows, top-N
/// reports) — [`run_keyed_lists`] adds its share afterwards.
pub(crate) fn run_reactions(world: &mut World) {
    let dt = world
        .get_resource::<Time>()
        .map(|t| t.delta_secs())
        .unwrap_or(0.0);

    let this_run = world.change_tick();

    world.resource_scope(|world, mut reg: Mut<ReactionRegistry>| {
        let mut changed = 0usize;
        let mut skipped = 0usize;
        let mut total_us = 0.0f32;
        let mut hidden_cache: HashMap<Entity, Option<Entity>> = HashMap::default();

        // Bring back anything whose collapsed ancestor has reopened (or gone).
        // Once a second, also sweep parked entries whose target died while
        // hidden — see [`ReactionRegistry::unpark_revealed`].
        let sweep_dead = {
            let f = world
                .get_resource::<ReactiveStats>()
                .map(|s| s.frame)
                .unwrap_or(0);
            f.is_multiple_of(60)
        };
        reg.unpark_revealed(world, sweep_dead);

        // Taken by value so entries can be *moved* into the parked map; a
        // `retain_mut` only ever hands out `&mut`, which cannot express "this
        // one leaves the active list but must not be dropped".
        let mut previous = core::mem::take(&mut reg.active);
        let mut next: Vec<ReactionEntry> = Vec::with_capacity(previous.len());
        let mut newly_parked: Vec<(Entity, ReactionEntry)> = Vec::new();

        for mut entry in previous.drain(..) {
            if let Some(target) = entry.meta.target {
                // Liveness FIRST, before either gate below.
                //
                // The only thing that reports `Dead` is the reaction closure,
                // and both gates below keep the entry without calling it. So a
                // binding whose target has been despawned but whose recorded
                // dependencies happen to be clean would never be looked at
                // again, and its entry would sit in this registry for the rest
                // of the session.
                //
                // That is not a corner case. `dock::sync_panes` keeps exactly
                // one pane alive per leaf and **despawns every inactive one**,
                // rebuilding on activation — so closing a panel and merely
                // switching tabs are the same event here, and both happen
                // constantly. Without this check the registry would grow with
                // every tab switch and never shrink, which is the one way a
                // dependency gate can end up *costing* more than it saves.
                if world.get_entity(target).is_err() {
                    continue;
                }
                // Hidden behind a collapsed subtree: park it under whichever
                // ancestor did the hiding and stop walking it altogether. It
                // comes back when that ancestor reopens.
                //
                // Only *ancestors* count, never the node itself — a binding may
                // toggle its own target's `Display` (`bind_display` does), and
                // parking it on its own collapse would strand it hidden with
                // nothing left to run and un-hide it.
                if let Some(anchor) = collapsed_ancestor(world, target, &mut hidden_cache) {
                    newly_parked.push((anchor, entry));
                    continue;
                }
            }
            // The dependency gate: nothing this reaction read has moved, so its
            // closure cannot produce a different value and there is no reason to
            // run it. An untracked or not-yet-seeded reaction always reports
            // dirty here, so this can only ever remove work — see [`rx`].
            if !entry.deps.is_dirty(world, entry.last_run, this_run) {
                entry.meta.record_skip();
                skipped += 1;
                next.push(entry);
                continue;
            }
            let t0 = Instant::now();
            let outcome = (entry.f)(world, &mut entry.deps);
            entry.last_run = this_run;
            let us = t0.elapsed().as_secs_f32() * 1e6;
            match outcome {
                ReactionOutcome::Dead => continue,
                ReactionOutcome::Unchanged => {
                    entry.meta.record(us, false);
                    total_us += us;
                    next.push(entry);
                }
                ReactionOutcome::Changed => {
                    entry.meta.record(us, true);
                    changed += 1;
                    total_us += us;
                    next.push(entry);
                }
            }
        }
        reg.active = next;
        for (anchor, entry) in newly_parked {
            reg.park(anchor, entry);
        }

        world.resource_scope(|world, mut stats: Mut<ReactiveStats>| {
            stats.frame += 1;
            stats.bindings_total = reg.active_len();
            stats.parked_total = reg.parked_len();
            stats.changed_this_frame = changed;
            stats.skipped_this_frame = skipped;
            stats.reactions_us = total_us;
            // Keyed lists reset here, accumulate in run_keyed_lists (chained).
            stats.lists_us = 0.0;
            stats.rows_rebuilt_this_frame = 0;

            // ~1s change-rate windows, advanced by wall-clock delta.
            roll_rate_windows(&mut stats, &mut reg, dt);

            // Only build the top-N reports when someone is actually looking.
            // They cost two O(N log N) sorts plus an `entry_label` `format!` per
            // row, and their sole consumer is the debugger's "UI Reactivity"
            // panel — so with it closed (the overwhelmingly common case) this was
            // pure waste every 30 frames.
            if stats.frame.is_multiple_of(30) && reactivity_panel_open(world) {
                build_reports(world, &reg, &mut stats);
            }
        });
    });
}

/// Once a second of wall-clock time has accumulated, convert every binding's
/// in-window change count into a changes/sec rate and reset the window.
fn roll_rate_windows(stats: &mut ReactiveStats, reg: &mut ReactionRegistry, dt: f32) {
    stats.window_elapsed += dt;
    if stats.window_elapsed >= 1.0 {
        let elapsed = stats.window_elapsed;
        let mut total = 0u32;
        for entry in reg.iter_all_mut() {
            total += entry.meta.changes_window;
            entry.meta.roll_window(elapsed);
        }
        stats.changes_per_sec = total as f32 / elapsed;
        stats.window_elapsed = 0.0;
    }
}

/// Is the debugger's "UI Reactivity" panel the active tab anywhere?
///
/// The reports below exist only to feed it. Checked inline rather than via
/// `dock::panel_active` because that returns a run-condition closure, and these
/// drivers are exclusive systems that need the check mid-body.
fn reactivity_panel_open(world: &World) -> bool {
    const PANEL: &str = "ui_reactivity";
    world
        .get_resource::<crate::dock::Dock>()
        .is_some_and(|d| d.tree.is_active_tab(PANEL))
        || world
            .get_resource::<crate::dock::DockWindows>()
            .is_some_and(|w| w.0.iter().any(|s| s.tree.is_active_tab(PANEL)))
}

/// Reports cover **active bindings only**. A parked one is behind a collapsed
/// subtree and costs nothing this frame, so listing it among the most expensive
/// bindings would point the reader at work that is not happening.
fn build_reports(world: &World, reg: &ReactionRegistry, stats: &mut ReactiveStats) {
    let mut by_cost: Vec<&EntryMeta> = reg.active.iter().map(|e| &e.meta).collect();
    by_cost.sort_by(|a, b| b.cost_ema_us.total_cmp(&a.cost_ema_us));
    stats.top_cost = by_cost
        .iter()
        .take(ReactiveStats::TOP_N)
        .map(|m| report_row(world, m))
        .collect();

    let mut by_churn: Vec<&EntryMeta> = reg.active.iter().map(|e| &e.meta).collect();
    by_churn.sort_by(|a, b| {
        b.change_rate
            .total_cmp(&a.change_rate)
            .then(b.changes.cmp(&a.changes))
    });
    stats.top_churn = by_churn
        .iter()
        .take(ReactiveStats::TOP_N)
        .map(|m| report_row(world, m))
        .filter(|r| r.changes > 0)
        .collect();
}

fn report_row(world: &World, meta: &EntryMeta) -> BindingReport {
    BindingReport {
        label: entry_label(world, meta.target),
        kind: meta.kind,
        cost_ema_us: meta.cost_ema_us,
        change_rate: meta.change_rate,
        changes: meta.changes,
    }
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

/// Builds this frame's snapshot, recording into the out-param what it read.
/// Untracked lists ignore the `DepSet` and so stay permanently dirty, which is
/// today's behaviour — see [`rx`].
type SnapshotFn = Box<dyn Fn(&World, &mut DepSet) -> KeyedSnapshot + Send + Sync>;

struct KeyedList {
    container: Entity,
    /// key -> (content hash, child entity)
    current: HashMap<u64, (u64, Entity)>,
    /// `(key, hash)` in display order — for a cheap "nothing changed" check.
    order: Vec<(u64, u64)>,
    snapshot: SnapshotFn,
    /// What the last snapshot read, and when it ran. Same gate the bindings
    /// use, and a strictly better one than `token` where it applies: a keyed
    /// list's snapshot is usually the most expensive closure in a panel.
    deps: DepSet,
    last_run: Tick,
    /// Optional cheap check run before the snapshot each frame. When it returns
    /// the same value as the previous frame, the snapshot is skipped — so a list
    /// whose snapshot is expensive to produce doesn't pay for it on frames where
    /// nothing changed. `None` means always run the snapshot.
    token: Option<Box<dyn Fn(&World) -> u64 + Send + Sync>>,
    last_token: Option<u64>,
    meta: EntryMeta,
    rows_rebuilt: u64,
}

impl KeyedList {
    fn new(
        container: Entity,
        token: Option<Box<dyn Fn(&World) -> u64 + Send + Sync>>,
        snapshot: SnapshotFn,
        kind: &'static str,
    ) -> Self {
        Self {
            container,
            current: HashMap::default(),
            order: Vec::new(),
            snapshot,
            deps: DepSet::default(),
            last_run: Tick::new(0),
            token,
            last_token: None,
            meta: EntryMeta::new(Some(container), kind),
            rows_rebuilt: 0,
        }
    }

    /// A list whose snapshot records its own dependencies — see
    /// [`tracked::keyed_list`].
    pub(crate) fn new_tracked(container: Entity, snapshot: SnapshotFn) -> Self {
        Self::new(container, None, snapshot, "list*")
    }

    /// Tracked, plus the caller's own value-hash gate — see
    /// [`tracked::keyed_list_tokened`].
    pub(crate) fn new_tracked_tokened(
        container: Entity,
        token: Box<dyn Fn(&World) -> u64 + Send + Sync>,
        snapshot: SnapshotFn,
    ) -> Self {
        Self::new(container, Some(token), snapshot, "list*")
    }
}

#[derive(Resource, Default)]
pub struct KeyedListRegistry(Vec<KeyedList>);

/// Keyed lists that have been registered but not yet installed into
/// [`KeyedListRegistry`].
///
/// Registration cannot write straight to `KeyedListRegistry`: [`run_keyed_lists`]
/// holds that resource in a `resource_scope` for its entire pass, and applies the
/// row-build `CommandQueue` *inside* it. So a `keyed_list` registered from within
/// a row builder found the resource missing and was dropped — **silently**, with
/// no panic and no log, which made nested lists look like they simply never ran.
///
/// Staging registrations in a separate resource that nothing scopes out makes
/// nesting work by construction. Entries are drained into the real registry at
/// the top of the next pass, so a nested list starts one frame later — the same
/// deferral bindings already have.
#[derive(Resource, Default)]
struct PendingKeyedLists(Vec<KeyedList>);

/// A keyed, granular child list (`<For>`): rebuild only changed rows, add new,
/// remove gone, reorder — never a full-list rebuild. `snapshot` returns this
/// frame's `(key, hash)` order + a builder; a row rebuilds only when its hash
/// changes. Registered (deferred) via `commands`.
pub fn keyed_list<F>(commands: &mut Commands, container: Entity, snapshot: F)
where
    F: Fn(&World) -> KeyedSnapshot + Send + Sync + 'static,
{
    register_keyed_list(commands, container, None, snapshot);
}

/// Like [`keyed_list`], but runs `token` (a cheap `&World -> u64`) before the
/// snapshot each frame and skips the snapshot when the token is unchanged.
/// Use this when the snapshot is expensive to build and the consumer can cheaply
/// signal whether anything affecting the list changed (a content version, plus
/// the scroll window for a virtualized list — see [`crate::virtual_scroll`]).
pub fn keyed_list_tokened<T, F>(commands: &mut Commands, container: Entity, token: T, snapshot: F)
where
    T: Fn(&World) -> u64 + Send + Sync + 'static,
    F: Fn(&World) -> KeyedSnapshot + Send + Sync + 'static,
{
    register_keyed_list(commands, container, Some(Box::new(token)), snapshot);
}

fn register_keyed_list<F>(
    commands: &mut Commands,
    container: Entity,
    token: Option<Box<dyn Fn(&World) -> u64 + Send + Sync>>,
    snapshot: F,
) where
    F: Fn(&World) -> KeyedSnapshot + Send + Sync + 'static,
{
    commands.queue(move |world: &mut World| {
        // Stage rather than pushing straight to `KeyedListRegistry` — see
        // [`PendingKeyedLists`]. `get_resource_or_insert_with` so a registration
        // that lands before `ReactivePlugin` has initialised the resource still
        // survives instead of being silently dropped.
        world
            .get_resource_or_insert_with(PendingKeyedLists::default)
            .0
            .push(KeyedList::new(
                container,
                token,
                // Legacy snapshots take `&World` and cannot record anything, so
                // the dep set stays empty and the list keeps running every
                // frame — unchanged behaviour.
                Box::new(move |world: &World, _deps: &mut DepSet| snapshot(world)),
                "list",
            ));
    });
}

pub(crate) fn run_keyed_lists(world: &mut World) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    // Install anything registered since the last pass, including lists
    // registered from inside a row builder (which cannot reach the registry
    // directly — see `PendingKeyedLists`). Done before the `resource_scope`
    // below, which is precisely what makes those registrations unreachable.
    let pending: Vec<KeyedList> = world
        .get_resource_mut::<PendingKeyedLists>()
        .map(|mut p| std::mem::take(&mut p.0))
        .unwrap_or_default();
    if !pending.is_empty() {
        world
            .get_resource_or_insert_with(KeyedListRegistry::default)
            .0
            .extend(pending);
    }
    let this_run = world.change_tick();
    world.resource_scope(|world, mut reg: Mut<KeyedListRegistry>| {
        let mut total_us = 0.0f32;
        let mut rows_rebuilt = 0usize;
        let mut hidden_cache: HashMap<Entity, Option<Entity>> = HashMap::default();
        reg.0.retain_mut(|kl| {
            if world.get_entity(kl.container).is_err() {
                return false;
            }
            // Hidden dock tab → don't run the snapshot. This is the big win: a
            // backgrounded list (e.g. the asset browser hashing every file in a
            // folder each frame) stops costing anything until it's shown again,
            // where the snapshot re-runs and catches up.
            if collapsed_ancestor(world, kl.container, &mut hidden_cache).is_some() {
                return true;
            }
            // Dependency gate, same as the bindings'. For a tracked list this
            // replaces the hand-rolled `token`: the question the token exists to
            // answer is "did anything the snapshot reads change", and that is
            // exactly what the recorded dep set answers, without the caller
            // having to keep a version counter honest.
            if !kl.deps.is_dirty(world, kl.last_run, this_run) {
                kl.meta.record_skip();
                return true;
            }
            let t0 = Instant::now();
            // If a dirty token is supplied and matches last frame, nothing the
            // list depends on changed — skip building the snapshot entirely.
            if let Some(token) = &kl.token {
                let tok = token(world);
                if kl.last_token == Some(tok) {
                    let us = t0.elapsed().as_secs_f32() * 1e6;
                    kl.meta.record(us, false);
                    total_us += us;
                    return true;
                }
                kl.last_token = Some(tok);
            }
            let snap = (kl.snapshot)(world, &mut kl.deps);
            kl.last_run = this_run;
            // Cheap fast-path: same keys + hashes in the same order → nothing to do.
            if snap.items == kl.order {
                let us = t0.elapsed().as_secs_f32() * 1e6;
                kl.meta.record(us, false);
                total_us += us;
                return true;
            }

            let mut built = 0usize;
            let mut queue = CommandQueue::default();
            let mut next: HashMap<u64, (u64, Entity)> = HashMap::default();
            let mut ordered: Vec<Entity> = Vec::with_capacity(snap.items.len());
            {
                let mut commands = Commands::new(&mut queue, world);
                for (i, &(key, hash)) in snap.items.iter().enumerate() {
                    // A tracked row is reusable only if it is BOTH unchanged and
                    // still ALIVE. The liveness half is not paranoia: the two
                    // arms below already say a tracked row may have been
                    // despawned by another rebuild path and its slot reused, and
                    // that applies just as much to a row whose hash did not
                    // change. Pushing a dead entity into `ordered` used to reach
                    // `replace_children`, which — unlike `try_despawn` — has no
                    // fallible variant, so it surfaced as a bare "entity is
                    // invalid; its index now has generation N" warning naming a
                    // command the log could not even name.
                    //
                    // Filtering here rather than filtering `ordered` afterwards,
                    // because a vanished row must be REBUILT (it falls through to
                    // the `None` arm) rather than silently dropped from the list.
                    let tracked = kl
                        .current
                        .get(&key)
                        .copied()
                        .filter(|&(_, e)| world.get_entity(e).is_ok());
                    match tracked {
                        Some((h, e)) if h == hash => {
                            next.insert(key, (h, e));
                            ordered.push(e);
                        }
                        Some((_, old)) => {
                            // `try_despawn`: the tracked row may already be gone
                            // (its slot despawned + reused by another rebuild path
                            // → a generation mismatch), and a plain `despawn` would
                            // panic on that stale handle. We rebuild `next` from
                            // scratch anyway, so silently skipping a vanished row is
                            // correct.
                            commands.entity(old).try_despawn();
                            let e = (snap.build)(&mut commands, &fonts, i);
                            next.insert(key, (hash, e));
                            ordered.push(e);
                            built += 1;
                        }
                        None => {
                            let e = (snap.build)(&mut commands, &fonts, i);
                            next.insert(key, (hash, e));
                            ordered.push(e);
                            built += 1;
                        }
                    }
                }
                // Despawn rows whose key vanished. `try_despawn` for the same
                // stale-slot reason as above.
                for (k, &(_, e)) in kl.current.iter() {
                    if !next.contains_key(k) {
                        commands.entity(e).try_despawn();
                    }
                }
                // Set the container's children to the new order (moves existing,
                // attaches newly-built ones).
                //
                // Use `replace_children`, NOT `insert_children(0, …)`. Bevy
                // 0.19's `OrderedRelationshipSourceCollection::place` (which
                // `insert_children` calls per already-related child) clamps the
                // target index with `index.min(self.len())` *before* removing
                // the entity from the collection, then inserts *after* the
                // removal — so moving an existing child to a tail index panics
                // with "insertion index (is N) should be <= len (is N-1)".
                // Whether it fires depends on the exact add/move/remove pattern
                // of a given reconcile, so it surfaced only for specific folders
                // (e.g. the blueprints folder's item set). `replace_children`
                // clears the collection and re-extends it from the slice with no
                // `place` calls, sidestepping the bug entirely.
                if !ordered.is_empty() {
                    commands.entity(kl.container).replace_children(&ordered);
                }
            }
            queue.apply(world);
            kl.current = next;
            kl.order = snap.items;
            kl.rows_rebuilt += built as u64;
            let us = t0.elapsed().as_secs_f32() * 1e6;
            kl.meta.record(us, true);
            total_us += us;
            rows_rebuilt += built;
            true
        });

        world.resource_scope(|world, mut stats: Mut<ReactiveStats>| {
            stats.lists_total = reg.0.len();
            stats.lists_us = total_us;
            stats.rows_rebuilt_this_frame = rows_rebuilt;

            // Frame total → history ring (this runs after run_reactions).
            let frame_total = stats.reactions_us + total_us;
            if stats.history_us.len() >= ReactiveStats::HISTORY_LEN {
                stats.history_us.remove(0);
            }
            stats.history_us.push(frame_total);

            // Same gate as the binding reports: an `entry_label` `format!` per
            // list plus a sort, for a panel that is usually closed.
            if stats.frame.is_multiple_of(30) && reactivity_panel_open(world) {
                let mut reports: Vec<ListReport> = reg
                    .0
                    .iter()
                    .map(|kl| ListReport {
                        label: entry_label(world, Some(kl.container)),
                        rows: kl.order.len(),
                        cost_ema_us: kl.meta.cost_ema_us,
                        rows_rebuilt: kl.rows_rebuilt,
                    })
                    .collect();
                reports.sort_by(|a, b| b.cost_ema_us.total_cmp(&a.cost_ema_us));
                stats.list_reports = reports;
            }
        });
    });
}
