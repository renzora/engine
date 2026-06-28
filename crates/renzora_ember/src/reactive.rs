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
            .init_resource::<ReactiveStats>()
            // Chained: run_reactions resets the per-frame stats that
            // run_keyed_lists then adds to.
            .add_systems(Update, (run_reactions, run_keyed_lists).chain());
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
    /// Registered bindings currently alive.
    pub bindings_total: usize,
    /// Bindings whose recompute produced a *new* value this frame (i.e. a UI
    /// write actually happened).
    pub changed_this_frame: usize,
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
        }
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
fn has_hidden_ancestor(world: &World, node: Entity, cache: &mut HashMap<Entity, bool>) -> bool {
    let Some(parent) = world.get::<ChildOf>(node).map(|c| c.parent()) else {
        return false;
    };
    in_collapsed_subtree(world, parent, cache)
}

/// True if `start` or any ancestor has `Display::None`. Memoized per frame; a
/// despawned `start` has no `Node`/parent and resolves to `false`.
fn in_collapsed_subtree(world: &World, start: Entity, cache: &mut HashMap<Entity, bool>) -> bool {
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
            break true;
        }
        match world.get::<ChildOf>(e) {
            Some(c) => e = c.parent(),
            None => break false,
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

struct ReactionEntry {
    f: Box<dyn FnMut(&mut World) -> ReactionOutcome + Send + Sync>,
    meta: EntryMeta,
}

#[derive(Resource, Default)]
pub struct ReactionRegistry(Vec<ReactionEntry>);

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
            reg.0.push(ReactionEntry {
                meta: EntryMeta::new(Some(target), kind),
                f: Box::new(move |world: &mut World| {
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
            });
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
pub fn react<F>(commands: &mut Commands, reaction: F)
where
    F: FnMut(&mut World) -> bool + Send + Sync + 'static,
{
    commands.queue(move |world: &mut World| {
        if let Some(mut reg) = world.get_resource_mut::<ReactionRegistry>() {
            let mut reaction = reaction;
            reg.0.push(ReactionEntry {
                meta: EntryMeta::new(None, "raw"),
                f: Box::new(move |world: &mut World| {
                    if reaction(world) {
                        ReactionOutcome::Unchanged
                    } else {
                        ReactionOutcome::Dead
                    }
                }),
            });
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
            reg.0.push(ReactionEntry {
                meta: EntryMeta::new(Some(target), "2way"),
                f: Box::new(move |world: &mut World| {
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
            });
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

    world.resource_scope(|world, mut reg: Mut<ReactionRegistry>| {
        let mut changed = 0usize;
        let mut total_us = 0.0f32;
        let mut hidden_cache: HashMap<Entity, bool> = HashMap::default();
        reg.0.retain_mut(|entry| {
            // Skip bindings whose pane is a hidden dock tab — they're not on
            // screen, so recomputing them is wasted frame time. (Raw reactions
            // with no target entity can't be located, so they always run.)
            if let Some(target) = entry.meta.target {
                if has_hidden_ancestor(world, target, &mut hidden_cache) {
                    return true;
                }
            }
            let t0 = Instant::now();
            let outcome = (entry.f)(world);
            let us = t0.elapsed().as_secs_f32() * 1e6;
            match outcome {
                ReactionOutcome::Dead => false,
                ReactionOutcome::Unchanged => {
                    entry.meta.record(us, false);
                    total_us += us;
                    true
                }
                ReactionOutcome::Changed => {
                    entry.meta.record(us, true);
                    changed += 1;
                    total_us += us;
                    true
                }
            }
        });

        world.resource_scope(|world, mut stats: Mut<ReactiveStats>| {
            stats.frame += 1;
            stats.bindings_total = reg.0.len();
            stats.changed_this_frame = changed;
            stats.reactions_us = total_us;
            // Keyed lists reset here, accumulate in run_keyed_lists (chained).
            stats.lists_us = 0.0;
            stats.rows_rebuilt_this_frame = 0;

            // ~1s change-rate windows, advanced by wall-clock delta.
            roll_rate_windows(&mut stats, &mut reg, dt);

            if stats.frame.is_multiple_of(30) {
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
        for entry in &mut reg.0 {
            total += entry.meta.changes_window;
            entry.meta.roll_window(elapsed);
        }
        stats.changes_per_sec = total as f32 / elapsed;
        stats.window_elapsed = 0.0;
    }
}

fn build_reports(world: &World, reg: &ReactionRegistry, stats: &mut ReactiveStats) {
    let mut by_cost: Vec<usize> = (0..reg.0.len()).collect();
    by_cost.sort_by(|&a, &b| {
        reg.0[b]
            .meta
            .cost_ema_us
            .total_cmp(&reg.0[a].meta.cost_ema_us)
    });
    stats.top_cost = by_cost
        .iter()
        .take(ReactiveStats::TOP_N)
        .map(|&i| report_row(world, &reg.0[i].meta))
        .collect();

    let mut by_churn: Vec<usize> = (0..reg.0.len()).collect();
    by_churn.sort_by(|&a, &b| {
        let ma = &reg.0[a].meta;
        let mb = &reg.0[b].meta;
        mb.change_rate
            .total_cmp(&ma.change_rate)
            .then(mb.changes.cmp(&ma.changes))
    });
    stats.top_churn = by_churn
        .iter()
        .take(ReactiveStats::TOP_N)
        .map(|&i| report_row(world, &reg.0[i].meta))
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

struct KeyedList {
    container: Entity,
    /// key -> (content hash, child entity)
    current: HashMap<u64, (u64, Entity)>,
    /// `(key, hash)` in display order — for a cheap "nothing changed" check.
    order: Vec<(u64, u64)>,
    snapshot: Box<dyn Fn(&World) -> KeyedSnapshot + Send + Sync>,
    /// Optional cheap check run before the snapshot each frame. When it returns
    /// the same value as the previous frame, the snapshot is skipped — so a list
    /// whose snapshot is expensive to produce doesn't pay for it on frames where
    /// nothing changed. `None` means always run the snapshot.
    token: Option<Box<dyn Fn(&World) -> u64 + Send + Sync>>,
    last_token: Option<u64>,
    meta: EntryMeta,
    rows_rebuilt: u64,
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
        if let Some(mut reg) = world.get_resource_mut::<KeyedListRegistry>() {
            reg.0.push(KeyedList {
                container,
                current: HashMap::default(),
                order: Vec::new(),
                snapshot: Box::new(snapshot),
                token,
                last_token: None,
                meta: EntryMeta::new(Some(container), "list"),
                rows_rebuilt: 0,
            });
        }
    });
}

pub(crate) fn run_keyed_lists(world: &mut World) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    world.resource_scope(|world, mut reg: Mut<KeyedListRegistry>| {
        let mut total_us = 0.0f32;
        let mut rows_rebuilt = 0usize;
        let mut hidden_cache: HashMap<Entity, bool> = HashMap::default();
        reg.0.retain_mut(|kl| {
            if world.get_entity(kl.container).is_err() {
                return false;
            }
            // Hidden dock tab → don't run the snapshot. This is the big win: a
            // backgrounded list (e.g. the asset browser hashing every file in a
            // folder each frame) stops costing anything until it's shown again,
            // where the snapshot re-runs and catches up.
            if has_hidden_ancestor(world, kl.container, &mut hidden_cache) {
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
            let snap = (kl.snapshot)(world);
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
                    match kl.current.get(&key) {
                        Some(&(h, e)) if h == hash => {
                            next.insert(key, (h, e));
                            ordered.push(e);
                        }
                        Some(&(_, old)) => {
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

            if stats.frame.is_multiple_of(30) {
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
