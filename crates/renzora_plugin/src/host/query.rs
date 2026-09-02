//! Turning a plugin's declared query into a real Bevy query, and running it.
//!
//! This is the interesting part of the whole boundary. A plugin declares what it
//! wants up front (`sys::QueryDesc`); we build a genuine `QueryBuilder` from it,
//! so the resulting system carries proper component access and the
//! multi-threaded executor can schedule it against everything else. The
//! alternative — handing plugins open-ended `&mut World` — would force every
//! plugin system to be exclusive and serialise the entire schedule.
//!
//! Everything the plugin sees lives in staging buffers we own. That costs a copy
//! per cell and it is what makes the call sound: no pointer into component
//! storage is ever exposed, so the plugin never has to assume a host layout it
//! did not define.
//!
//! Two invariants are easy to break and expensive when broken:
//!
//! - **A cell index is not a term index.** Filters produce no cell, so dropping
//!   or inventing a term silently shifts every later cell and the plugin reads
//!   its own data at the wrong offsets. That is why an unknown access kind
//!   refuses the whole system rather than skipping the term.
//! - **`gather` and `scatter` must skip identically.** They are two independent
//!   walks of the same query indexed by ordinal, so the tick mask is *recorded*
//!   and replayed rather than recomputed — `write_cell` marks components changed
//!   as it goes, so a second evaluation would see a different answer than the
//!   first.

use bevy::diagnostic::DiagnosticsStore;
use bevy::ecs::component::ComponentId;
use bevy::ecs::lifecycle::{RemovedComponentEntity, RemovedComponentMessages};
use bevy::ecs::message::MessageCursor;
use bevy::ecs::query::QueryBuilder;
use bevy::ecs::system::{
    FilteredResourcesMutParamBuilder, ParamBuilder, QueryParamBuilder, SystemChangeTick,
    SystemParamBuilder,
};
use bevy::ecs::world::FilteredResourcesMut;
use bevy::ecs::world::{FilteredEntityMut, FilteredEntityRef};
use bevy::prelude::*;
use std::collections::HashMap;
use std::ffi::c_void;

use crate::sys;

use super::assets::PluginAssets;
use super::commands::{
    apply_queued, diagnostics_read, http_poll, http_poll_stream, image_write, mesh_read,
    mesh_write, removed_read, reply_poll, sink_push, sink_reserve, DiagnosticSourceImpl,
    HttpSourceImpl, ImageSourceImpl, MeshSourceImpl, PluginHttpInbox, PluginServiceReplies,
    RemovedSourceImpl, ReplySourceImpl, SinkImpl,
};
use super::iface::IFACE;
use super::input;
use super::reload::{component_info, GenGate};
use super::schema::{component_type_path, HostDataComponents};

/// How one query term crosses the boundary.
///
/// The distinction exists because the host's own types have no layout guarantee.
/// See `renzora_plugin::sys` — `bevy::Transform` is not `#[repr(C)]` and
/// `glam::Quat` changes representation per SIMD backend, so we cannot hand out a
/// pointer and let the plugin cast.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Marshal {
    /// Copy the component's bytes verbatim. Correct for plugin-owned components,
    /// whose layout the plugin itself defined.
    Raw,
    /// Convert to and from `sys::Transform`.
    Transform,
}

#[derive(Clone)]
pub(crate) struct TermPlan {
    pub(crate) id: ComponentId,
    pub(crate) access: sys::Access,
    pub(crate) marshal: Marshal,
    /// Size of one cell *as the plugin sees it*, which for a mirrored term is
    /// the mirror's size, not the host type's.
    pub(crate) cell_size: usize,
}

/// Resolve each declared term into how it will actually be marshalled, or bail
/// if the plugin asked for a component that does not exist. Failing here is much
/// kinder than registering a system whose query silently matches nothing.
pub(crate) fn build_plan(world: &World, terms: &[sys::Term]) -> Option<Vec<TermPlan>> {
    let transform_id = world.component_id::<Transform>();
    let mut plan = Vec::with_capacity(terms.len());
    // Nesting depth of `Or` brackets, so a change-tick term inside one can be
    // refused — see the match below for why that matters.
    let mut or_depth = 0usize;

    for t in terms {
        // Refuse the whole system rather than skip the term. An unknown access
        // kind may have wanted a cell, and dropping it silently would shift
        // every later cell index — the plugin would then read its own data at
        // the wrong offsets, which presents as garbage values rather than as a
        // version problem.
        if !t.access.is_known() {
            error!(
                "plugin used access kind {} which this build does not have —                  refusing the system rather than mis-indexing its data",
                t.access.0
            );
            return None;
        }
        // A change-tick test is a per-row predicate the dispatcher evaluates, not
        // something `QueryBuilder` can express — it has no tick dimension at all.
        // Inside an `Or` group that is fatal in the quiet direction: `apply_filters`
        // would drop the term through its `_ => {}` arm, leaving the branch EMPTY,
        // and an empty `FilteredAccess` is `matches_everything()`. One empty
        // disjunct makes the whole `Or` match every entity in the world.
        //
        // The `_ => {}` arm is justified for an unknown kind, where widening the
        // match is harmless. It is not justified here, so refuse instead — the
        // same reflex as the unknown-access arm above.
        match t.access {
            sys::Access::OrBegin => or_depth += 1,
            sys::Access::OrEnd => or_depth = or_depth.saturating_sub(1),
            sys::Access::Added | sys::Access::Changed if or_depth > 0 => {
                error!(
                    "plugin used `{}` inside an `Or` group. A change-tick test is a per-row \
                     predicate and the query builder has no tick dimension, so the branch would \
                     be empty — and an empty branch makes the whole `Or` match every entity in \
                     the world. Refusing the system; move the tick filter to the top level of \
                     the filter tuple.",
                    t.access.name()
                );
                return None;
            }
            _ => {}
        }
        // `Or` brackets name nothing. They survive into the plan so the query
        // builder can still see the grouping, and are filtered out everywhere
        // that walks terms for data.
        if t.access.is_marker() {
            plan.push(TermPlan {
                id: ComponentId::new(0),
                access: t.access,
                marshal: Marshal::Raw,
                cell_size: 0,
            });
            continue;
        }
        let id = ComponentId::new(t.component.0 as usize);
        let Some(info) = world.components().get_info(id) else {
            error!("plugin declared an unknown component id {}", t.component.0);
            return None;
        };
        if t.access.is_resource() {
            plan.push(TermPlan {
                id,
                access: t.access,
                marshal: Marshal::Raw,
                cell_size: info.layout().size(),
            });
            continue;
        }
        // A term that carries data and names a component this plugin did not
        // register is a *host* component being read as plain bytes. That needs
        // permission — see [`HostDataComponents`] for what goes wrong without it.
        // Filter terms never reach here, so `With<Camera3d>` stays free.
        if t.access.has_cell() && component_info(world, id).is_none() && Some(id) != transform_id {
            // Resolve the reflected type path rather than `info.name()`.
            //
            // `ComponentInfo::name()` returns a `DebugName`, whose inner string is
            // `#[cfg(feature = "debug")]` in bevy_utils — a feature this workspace
            // does not enable. Without it, dereferencing yields the literal
            // "<Enable the debug feature to see the name>" for every component, so
            // comparing it against a real type path never matched and this gate
            // refused everything in shipped builds.
            //
            // It looked correct under `cargo test` only because a dev-dependency
            // pulls bevy with `debug` and resolver-2 unifies dev features into the
            // test build. A guard that is live in tests and dead in release is
            // worse than no guard, because it reports success.
            // A component with no reflected type path cannot have been exposed,
            // and naming it in the error is still the useful thing to do.
            let path = component_type_path(world, id)
                .unwrap_or_else(|| format!("<unreflected component #{}>", t.component.0));
            // A write needs its own permission: a mirror larger than the host
            // type writes past the end of its staging row, not merely reads the
            // wrong bytes.
            let writes = matches!(t.access, sys::Access::Write | sys::Access::WriteOptional);
            match world.get_resource::<HostDataComponents>() {
                Some(allowed)
                    if allowed.readable.contains(&path)
                        && (!writes || allowed.writable.contains(&path)) => {}
                Some(allowed) if writes && allowed.readable.contains(&path) => {
                    error!(
                        "plugin asked to WRITE engine component `{path}`, which is exposed for \
                         reading only. Reading it works; `&mut` needs the owning crate to call \
                         `expose_component_data_mut`, which is a stronger promise — a mirror \
                         that disagrees writes past its row rather than merely reading wrong"
                    );
                    return None;
                }
                Some(_) => {
                    error!(
                        "plugin asked to read engine component `{path}` as data, which is not \
                         exposed for that. Filtering on it (`With`/`Without`) is fine and needs \
                         nothing; reading its bytes needs the crate that owns the type to call \
                         `renzora_plugin::host::expose_component_data`, which is a promise that \
                         its layout is stable enough to mirror"
                    );
                    return None;
                }
                // Nothing has exposed anything at all, which is almost never what
                // an author meant — it means the plugin host was added before the
                // crates that expose their mirrors. Worth its own message: the
                // one above would send someone to add a call that is already there.
                None => {
                    error!(
                        "plugin asked to read engine component `{path}` as data, but nothing has \
                         exposed any engine component for plugin reads. If this is a full engine \
                         build, `RenzoraPluginHostPlugin` was added before the crates owning \
                         those mirrors — it has to come after them, because plugins resolve \
                         components during its `build`"
                    );
                    return None;
                }
            }
        }

        let (marshal, cell_size) = if Some(id) == transform_id {
            (Marshal::Transform, size_of::<sys::Transform>())
        } else {
            (Marshal::Raw, info.layout().size())
        };
        plan.push(TermPlan {
            id,
            access: t.access,
            marshal,
            cell_size,
        });
    }
    Some(plan)
}

/// Split the bracketed run that follows an `OrBegin` at `start` into its
/// branches, returning them and the index just past the matching `OrEnd`.
///
/// Depth-tracked so a nested `Or` inside a branch does not close the outer
/// group.
fn split_or_branches(terms: &[TermPlan], start: usize) -> (Vec<Vec<TermPlan>>, usize) {
    let mut branches: Vec<Vec<TermPlan>> = vec![Vec::new()];
    let mut depth = 0usize;
    let mut i = start;
    while i < terms.len() {
        let inner = terms[i].clone();
        i += 1;
        match inner.access {
            sys::Access::OrEnd if depth == 0 => break,
            sys::Access::OrNext if depth == 0 => branches.push(Vec::new()),
            _ => {
                if inner.access == sys::Access::OrBegin {
                    depth += 1;
                } else if inner.access == sys::Access::OrEnd {
                    depth -= 1;
                }
                branches.last_mut().unwrap().push(inner);
            }
        }
    }
    (branches, i)
}

/// Apply a run of filter terms to `builder`.
///
/// Recursive, because an `Or` branch may itself contain an `Or` — `Or<T>` is a
/// `QueryFilter` like any other, so `Or<(With<A>, Or<(With<B>, With<C>)>)>` is
/// ordinary code to write. A flat walk drops the inner brackets while still
/// emitting the inner `with_id`s, which silently turns the inner `Or` into an
/// `AND` and matches strictly fewer entities than asked for.
fn apply_filters(builder: &mut QueryBuilder, terms: &[TermPlan]) {
    let mut i = 0;
    while i < terms.len() {
        let t = &terms[i];
        i += 1;
        match t.access {
            sys::Access::With => {
                builder.with_id(t.id);
            }
            sys::Access::Without => {
                builder.without_id(t.id);
            }
            sys::Access::OrBegin => {
                let (branches, next) = split_or_branches(terms, i);
                i = next;
                builder.or(|b| {
                    for branch in &branches {
                        b.and(|bb| apply_filters(bb, branch));
                    }
                });
            }
            // Only filters make sense inside a group: data access would have to
            // be conditional on which branch matched, which no cell layout can
            // express. An unknown kind lands here too, harmlessly — a group is
            // pure filtering, so skipping a term only widens the match.
            _ => {}
        }
    }
}

/// Translate a flat term list into a Bevy query.
///
/// Flat rather than nested because the ABI carries one term array: an `Or` is a
/// bracketed run — `OrBegin`, branches separated by `OrNext`, `OrEnd` — so the
/// filter grammar can grow without the boundary struct changing shape.
fn build_query(builder: &mut QueryBuilder<FilteredEntityMut>, terms: &[TermPlan]) {
    let mut i = 0;
    while i < terms.len() {
        let t = &terms[i];
        i += 1;
        match t.access {
            sys::Access::Read
            // `ref_id`, not `with_id`, and that is mandatory rather than
            // stylistic: `FilteredEntityRef::get_change_ticks_by_id` is gated on
            // the same `access.has_read(id)` that `get_by_id` is. A `with_id`
            // term contributes filter sets and no read, so every row would
            // return `None` and the filter would match nothing, forever, with no
            // error. `ref_id`'s footprint is byte-identical to Bevy's own
            // `Changed<T>` — both end in `FilteredAccess::add_read` — so this
            // also inherits Bevy's implied `With<T>` and its scheduling.
            | sys::Access::Added
            | sys::Access::Changed => {
                builder.ref_id(t.id);
            }
            sys::Access::Write => {
                builder.mut_id(t.id);
            }
            sys::Access::With => {
                builder.with_id(t.id);
            }
            sys::Access::Without => {
                builder.without_id(t.id);
            }
            // Declares the access without the `with` that `ref_id`/`mut_id` imply,
            // so the entity matches whether or not it has the component.
            sys::Access::ReadOptional => {
                let id = t.id;
                builder.optional(move |b| {
                    b.ref_id(id);
                });
            }
            sys::Access::WriteOptional => {
                let id = t.id;
                builder.optional(move |b| {
                    b.mut_id(id);
                });
            }
            // Resources are not part of the entity query at all — they come in
            // through their own param.
            sys::Access::ResRead | sys::Access::ResWrite => {}
            sys::Access::OrBegin => {
                let (branches, next) = split_or_branches(terms, i);
                i = next;
                builder.or(|b| {
                    for branch in &branches {
                        // Each branch is one alternative, so its own terms must
                        // AND together before being OR-ed with the next.
                        b.and(|bb| apply_filters(bb, branch));
                    }
                });
            }
            sys::Access::OrNext | sys::Access::OrEnd => {}
            // An access kind from a newer ABI. Ignoring it is the only safe
            // move — the term may have wanted a cell, and inventing one would
            // shift every later cell index and hand the plugin the wrong data.
            // `build_plan` refuses the system outright for the same reason;
            // this arm exists so the match is total.
            other => {
                warn!("plugin used access kind {} which this build does not have", other.0);
            }
        }
    }
}

/// One query's staging buffers, rebuilt each call.
///
/// Split out per query because a system now has as many of these as it declared
/// `Query` parameters, and the plugin indexes cells within a view rather than
/// across the whole call.
struct ViewState {
    /// The plan's data terms only — filters contribute no cell, so a cell index
    /// is not a term index.
    cells_plan: Vec<TermPlan>,
    /// Column-major: `staging[term]` holds every row's bytes for that term.
    staging: Vec<Vec<u8>>,
    /// A copy taken before the plugin ran, for the terms it can write.
    ///
    /// Write-back used to be unconditional, which marked every matched component
    /// changed every frame — that does not just cost time, it destroys change
    /// detection for the whole engine, since `Changed<Transform>` anywhere
    /// becomes true whenever any plugin merely *looks* at a transform.
    baseline: Vec<Vec<u8>>,
    /// Only optional terms can be absent, but tracking presence for every term
    /// keeps row indexing uniform.
    present: Vec<Vec<bool>>,
    entities: Vec<sys::Entity>,
    cells: Vec<*mut u8>,
    /// Change-tick filters, which carry a component but produce no cell and so
    /// are absent from `cells_plan`.
    tick_plan: Vec<(ComponentId, TickKind)>,
    /// Precomputed `!tick_plan.is_empty()`, so the common unfiltered case pays
    /// nothing per row — and so an empty `kept` is unambiguous, rather than also
    /// meaning "zero rows matched".
    filtered: bool,
    /// One entry per row the query **iterated**, not per row staged.
    ///
    /// `gather` and `scatter` are two independent walks of the same query, each
    /// indexing by enumeration ordinal. They agree today only because both walk
    /// the identical unfiltered query. Once `gather` can skip, `scatter` has to
    /// skip exactly the same rows — replaying a recorded mask makes them aligned
    /// by construction, where recomputing the predicate would not: `write_cell`
    /// marks components changed as it goes, so the second evaluation would see a
    /// different answer than the first.
    kept: Vec<bool>,
}

/// Which tick predicate a filter term carries.
#[derive(Clone, Copy)]
enum TickKind {
    Added,
    Changed,
}

impl ViewState {
    fn new(cells_plan: Vec<TermPlan>, tick_plan: Vec<(ComponentId, TickKind)>) -> Self {
        let n = cells_plan.len();
        Self {
            filtered: !tick_plan.is_empty(),
            tick_plan,
            kept: Vec::new(),
            staging: cells_plan
                .iter()
                .map(|t| Vec::<u8>::with_capacity(t.cell_size * 64))
                .collect(),
            baseline: vec![Vec::new(); n],
            present: vec![Vec::new(); n],
            cells_plan,
            entities: Vec::new(),
            cells: Vec::new(),
        }
    }

    fn is_writable(t: &TermPlan) -> bool {
        matches!(
            t.access,
            sys::Access::Write | sys::Access::WriteOptional
        )
    }

    /// Copy every matched row into the staging buffers.
    fn gather(&mut self, q: &mut Query<FilteredEntityMut>, ticks: SystemChangeTick) {
        for e in q.iter() {
            // Before `read_cell`, deliberately: that allocates and copies per
            // cell, so a filtered-out row now costs a tick comparison instead of
            // a heap allocation per term. Skipping here also compacts for free —
            // everything below indexes by staged position, and nothing is pushed
            // for a skipped row.
            if self.filtered {
                let keep = self.tick_plan.iter().all(|(id, kind)| {
                    match e.get_change_ticks_by_id(*id) {
                        Some(t) => match kind {
                            TickKind::Added => t.is_added(ticks.last_run(), ticks.this_run()),
                            TickKind::Changed => t.is_changed(ticks.last_run(), ticks.this_run()),
                        },
                        // Unreachable in a correct build: the term was emitted
                        // with `ref_id`, which grants the read this getter is
                        // gated on and implies `With`. Drop rather than keep, so
                        // a host bug presents as "matches nothing" instead of
                        // silently widening the match.
                        None => false,
                    }
                });
                // Recorded for EVERY iterated row, including kept ones, and
                // before the `continue` — `scatter` indexes it by raw ordinal.
                self.kept.push(keep);
                if !keep {
                    continue;
                }
            }
            self.entities.push(sys::Entity(e.id().to_bits()));
            for (i, t) in self.cells_plan.iter().enumerate() {
                match read_cell(&e, t) {
                    Some(bytes) => {
                        self.staging[i].extend_from_slice(&bytes);
                        self.present[i].push(true);
                    }
                    // Still reserve the row so offsets stay uniform; the plugin
                    // sees a null cell and never reads these bytes.
                    None => {
                        let len = self.staging[i].len();
                        self.staging[i].resize(len + t.cell_size, 0);
                        self.present[i].push(false);
                    }
                }
            }
        }

        for (i, t) in self.cells_plan.iter().enumerate() {
            if Self::is_writable(t) {
                self.baseline[i] = self.staging[i].clone();
            }
        }

        // Row-major `entity_count × cell_count`, matching what `sys::QueryView`
        // documents. `present` is indexed [term][row] while this walks
        // row-major, so the range loop is the transpose, not something to
        // iterate away.
        self.cells
            .reserve(self.entities.len() * self.cells_plan.len());
        #[allow(clippy::needless_range_loop)]
        for row in 0..self.entities.len() {
            for (i, t) in self.cells_plan.iter().enumerate() {
                self.cells.push(if self.present[i][row] {
                    unsafe { self.staging[i].as_mut_ptr().add(row * t.cell_size) }
                } else {
                    std::ptr::null_mut()
                });
            }
        }
    }

    fn view(&mut self) -> sys::QueryView {
        sys::QueryView {
            cells: self.cells.as_mut_ptr(),
            entities: self.entities.as_ptr(),
            entity_count: self.entities.len(),
            cell_count: self.cells_plan.len(),
        }
    }

    /// Push back only the cells the plugin declared `&mut` **and** actually
    /// changed.
    fn scatter(&self, q: &mut Query<FilteredEntityMut>) {
        // Nothing writable: skip the iteration entirely rather than pay for a
        // second pass over every matched entity.
        if !self.cells_plan.iter().any(Self::is_writable) {
            return;
        }
        // A fully-filtered frame otherwise pays a whole second walk of the
        // unfiltered query to write nothing.
        if self.entities.is_empty() {
            return;
        }
        // Two cursors. `iterated` walks the query exactly as `gather` did;
        // `staged` counts only the rows that survived, which is what every
        // buffer is indexed by.
        //
        // The mask is replayed rather than recomputed, and that is a correctness
        // requirement, not a saving: `write_cell` reaches storage through
        // `MutUntyped::as_mut`, which marks the component changed. Re-evaluating
        // `Changed<T>` here would see rows this very loop had just dirtied and
        // give a different answer than `gather` did — the predicate would be
        // self-referential, and a `Query<&mut Foo, Changed<Foo>>` would write to
        // the wrong entities.
        let mut staged = 0usize;
        for (iterated, mut e) in q.iter_mut().enumerate() {
            if self.filtered && !self.kept.get(iterated).copied().unwrap_or(false) {
                continue;
            }
            let row = staged;
            staged += 1;
            for (i, t) in self.cells_plan.iter().enumerate() {
                if !Self::is_writable(t) || !self.present[i][row] {
                    continue;
                }
                let start = row * t.cell_size;
                let end = start + t.cell_size;
                // The comparison is what keeps change detection meaningful. It
                // costs a memcmp per writable cell and saves a write plus a
                // change-tick bump on every cell the plugin left alone, which is
                // most of them in most frames.
                if self.baseline[i][start..end] == self.staging[i][start..end] {
                    continue;
                }
                write_cell(&mut e, t, &self.staging[i][start..end]);
            }
        }
    }
}

/// Build the Bevy system that services one registered plugin system.
///
/// `user` is carried as `usize` rather than `*mut c_void` so the closure stays
/// `Send + Sync`; it is the plugin's own opaque token and we never dereference
/// it.
pub(crate) fn build_dispatcher(
    world: &mut World,
    plans: Vec<Vec<TermPlan>>,
    resource_plan: Vec<TermPlan>,
    entry: sys::SystemEntry,
    user: usize,
    gate: GenGate,
) -> impl System<In = (), Out = ()> {
    let build_terms = plans.clone();
    // Latched off after a panic. Without this a system that panics does so every
    // frame forever — thousands of identical errors, and the real first one
    // scrolls away. `AtomicBool` rather than `Cell` because a Bevy system must be
    // `Send + Sync`.
    let disabled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    // Resources are declared per-system rather than taken as a blanket
    // `&mut World`, which is what keeps plugin systems scheduling in parallel:
    // two systems touching different resources still have disjoint access.
    // Carries whether the plugin asked to write, because that decides which
    // accessor can reach the value: `get_mut_by_id` refuses an id the system only
    // declared `add_read_by_id` for, and a refusal here is indistinguishable from
    // the resource not existing.
    let mut resource_ids: Vec<(ComponentId, bool)> = Vec::new();
    for term in resource_plan.iter().filter(|t| t.access.is_resource()) {
        let write = term.access == sys::Access::ResWrite;
        match resource_ids.iter_mut().find(|(id, _)| *id == term.id) {
            // Two params naming the same resource: the stronger access wins, so
            // `Res` alongside `ResMut` still resolves.
            Some((_, w)) => *w |= write,
            None => resource_ids.push((term.id, write)),
        }
    }
    let resource_build = resource_plan.clone();

    // A `Vec` of builders produces a `Vec` of params, which is what lifts the
    // old one-query-per-system limit: `SystemParamBuilder` tuples are fixed at
    // compile time, so an arity-N tuple would have meant capping N and
    // generating an impl per arity.
    let query_builders: Vec<_> = build_terms
        .into_iter()
        .map(|terms| {
            QueryParamBuilder::new(move |builder: &mut QueryBuilder<FilteredEntityMut>| {
                build_query(builder, &terms);
            })
        })
        .collect();

    // One builder per system param. The tuple arity here MUST match the
    // closure's parameter count.
    (
        query_builders,
        FilteredResourcesMutParamBuilder::new(move |builder| {
            for t in &resource_build {
                match t.access {
                    sys::Access::ResRead => {
                        builder.add_read_by_id(t.id);
                    }
                    sys::Access::ResWrite => {
                        builder.add_write_by_id(t.id);
                    }
                    _ => {}
                }
            }
        }),
        // `ParamBuilder::resource::<Time>()` rather than bare `ParamBuilder`:
        // `build_state` runs before `build_system`, so nothing has pinned the
        // param type yet and inference stalls on `_: SystemParam`.
        ParamBuilder::resource::<Time>(),
        // Structural changes go through Bevy's own deferred queue, so a plugin
        // spawning mid-iteration is exactly as safe as a Rust system doing it.
        ParamBuilder::of::<Commands>(),
        // Read-only, and declared by every plugin system whether it reads input or
        // not. That costs nothing to schedule — a shared borrow never conflicts —
        // and it avoids the alternative, which is knowing at build time whether the
        // plugin's signature mentions `Input`.
        //
        // `Option`, because a host is not obliged to have input at all: a headless
        // server installs no input plugins, and a test app on `MinimalPlugins` has
        // none either. Requiring it made every plugin system panic wherever it was
        // absent, which is a lot of blast radius for a parameter most systems
        // ignore.
        ParamBuilder::of::<Option<Res<input::PluginInput>>>(),
        // Mesh reading. `Option`, because a headless host has no renderer and so
        // no `Assets<Mesh>` — a plugin there simply never gets geometry back.
        ParamBuilder::of::<Option<ResMut<Assets<Mesh>>>>(),
        // Read-only, and `Mesh3d` is filter-only across the ABI (a plugin can
        // name it in `With` but never get a data cell for it), so this cannot
        // conflict with the dynamic queries above.
        ParamBuilder::of::<Query<&'static Mesh3d>>(),
        // HTTP delivery. `Option` because a host without an HTTP bridge simply
        // never completes a request, which a plugin sees as "not ready yet".
        ParamBuilder::of::<Option<ResMut<PluginHttpInbox>>>(),
        ParamBuilder::of::<Option<ResMut<PluginServiceReplies>>>(),
        // The slot table, so `MeshSource::write` can resolve a handle the
        // plugin was handed at init.
        ParamBuilder::of::<Option<Res<PluginAssets>>>(),
        // Pixel writes for plugin-created images.
        ParamBuilder::of::<Option<ResMut<Assets<Image>>>>(),
        // Removal tracking. Declares no access at all — it reads a message
        // buffer, not component storage — so it can never conflict with the
        // dynamic queries above, and adding it to every dispatcher costs nothing
        // to schedule.
        ParamBuilder::of::<&RemovedComponentMessages>(),
        // Per-system cursors, which is what makes the semantics match Bevy's:
        // this `Local` belongs to THIS dispatcher, so each plugin system sees
        // every removal exactly once even when several watch the same component.
        ParamBuilder::of::<Local<HashMap<ComponentId, MessageCursor<RemovedComponentEntity>>>>(),
        // The same `last_run`/`this_run` a real `Changed<T>` in this system would
        // see — `SystemChangeTick` reads them straight off `SystemMeta`, declares
        // no access, and costs nothing to schedule. Because the host builds one
        // real Bevy system per plugin system, per-system change scoping maps 1:1.
        //
        // Never cache these in a `Local`. `World::check_change_ticks` clamps ticks
        // wherever it can reach them, and a tick hidden in a `Local` is not
        // somewhere it can reach — past the threshold it starts returning wrong
        // answers, silently.
        ParamBuilder::of::<SystemChangeTick>(),
        // This frame's measurements. `Option` because diagnostics are assembled
        // by the host, not by Bevy's core: the editor adds them, a shipped game
        // usually does not, and a plugin there reads an empty store rather than
        // panicking. Read-only and not component storage, so like the removal
        // messages above it can never conflict with the dynamic queries.
        ParamBuilder::of::<Option<Res<DiagnosticsStore>>>(),
    )
        .build_state(world)
        .build_system(move |mut queries: Vec<Query<FilteredEntityMut>>,
                            mut resources: FilteredResourcesMut,
                            time: Res<Time>,
                            mut commands: Commands,
                            plugin_input: Option<Res<input::PluginInput>>,
                            mut mesh_assets: Option<ResMut<Assets<Mesh>>>,
                            mesh_handles: Query<&Mesh3d>,
                            http_inbox: Option<ResMut<PluginHttpInbox>>,
                            service_replies: Option<ResMut<PluginServiceReplies>>,
                            plugin_assets: Option<Res<PluginAssets>>,
                            mut image_assets: Option<ResMut<Assets<Image>>>,
                            removed_messages: &RemovedComponentMessages,
                            mut removed_cursors: Local<
            HashMap<ComponentId, MessageCursor<RemovedComponentEntity>>,
        >,
                            system_ticks: SystemChangeTick,
                            diagnostics_store: Option<Res<DiagnosticsStore>>| {
            if disabled.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            // The plugin that registered this has been reloaded, and a newer
            // build has already registered its replacement. Retiring here rather
            // than unregistering is what keeps hot-reload from costing every
            // build a swappable sub-schedule — see `GenGate`.
            //
            // Checked before the staging buffers are built, so a retired system
            // costs an atomic load and nothing else. It still pays the param
            // fetch Bevy did to call it; that is the accumulating cost.
            if gate.stale() {
                return;
            }
            // Everything the plugin sees lives in staging buffers we own. That
            // costs a copy per cell, but it is what makes the call sound: we
            // never expose a pointer into component storage whose layout the
            // plugin would have to assume. Optimising the `Marshal::Raw` case to
            // a direct pointer is possible later — those layouts ARE the
            // plugin's own — but it needs a careful aliasing argument, so
            // correctness first.
            let mut states: Vec<ViewState> = plans
                .iter()
                .map(|plan| {
                    ViewState::new(
                        plan.iter()
                            .filter(|t| t.access.has_cell())
                            .cloned()
                            .collect(),
                        // Tick filters carry a component but produce no cell, so
                        // they are absent from the list above and need their own.
                        plan.iter()
                            .filter_map(|t| match t.access {
                                sys::Access::Added => Some((t.id, TickKind::Added)),
                                sys::Access::Changed => Some((t.id, TickKind::Changed)),
                                _ => None,
                            })
                            .collect(),
                    )
                })
                .collect();

            for (state, q) in states.iter_mut().zip(queries.iter_mut()) {
                state.gather(q, system_ticks);
            }

            // Deliberately NOT skipped when every query is empty.
            //
            // This used to return early on the reasoning that a system with no
            // rows has nothing to say. That is true of a system whose whole job
            // is its query, and false of any plugin holding state outside the
            // ECS — which is most of the interesting ones, because a plugin
            // component is a closed set of numeric kinds and anything richer
            // has to live in the plugin's own memory.
            //
            // `plugins/hair` is the case that found it: it spawns a render
            // entity per groom and tracks it plugin-side, and the ABI gives it
            // no `RemovedComponents` and no despawn hook. Absence of a row IS
            // the teardown signal, so skipping the call is precisely the frame
            // it needed. The symptom was hair left standing in the scene after
            // its model was deleted, with nothing to blame in the plugin.
            //
            // The saving was small in any case: the staging buffers and the
            // gather already ran above, so this only avoided one FFI call and
            // the resource-slot setup on an idle system.

            let views: Vec<sys::QueryView> = states.iter_mut().map(ViewState::view).collect();

            // Resolved once per call rather than per access: a system may read
            // the same resource from several parameters, and each `get_mut_by_id`
            // takes a fresh borrow.
            let mut slots: Vec<sys::ResourceSlot> = Vec::with_capacity(resource_ids.len());
            for (id, write) in &resource_ids {
                let ptr = if *write {
                    resources
                        .get_mut_by_id(*id)
                        .map(|mut m| m.as_mut().as_ptr())
                        .unwrap_or(std::ptr::null_mut())
                } else {
                    // Cast away const: the slot is a plain address, and only
                    // `ResMut` — which requires the write branch above — ever
                    // hands out a `&mut` to it.
                    resources
                        .get_by_id(*id)
                        .map(|p| p.as_ptr())
                        .unwrap_or(std::ptr::null_mut())
                };
                slots.push(sys::ResourceSlot {
                    id: sys::ComponentId(id.index() as u32),
                    ptr,
                });
            }

            let mut reply_src = ReplySourceImpl {
                src: sys::ReplySource { poll: reply_poll },
                replies: service_replies.map(|r| r.into_inner()),
            };
            let mut http_src = HttpSourceImpl {
                src: sys::HttpSource {
                    poll: http_poll,
                    poll_stream: http_poll_stream,
                },
                inbox: http_inbox.map(|i| i.into_inner()),
            };
            let mut image_src = ImageSourceImpl {
                src: sys::ImageSource { write: image_write },
                assets: image_assets.as_deref_mut(),
                store: plugin_assets.as_deref(),
            };
            let mut mesh_src = MeshSourceImpl {
                src: sys::MeshSource { read: mesh_read, write: mesh_write },
                assets: mesh_assets.as_deref_mut(),
                handles: &mesh_handles,
                store: plugin_assets.as_deref(),
            };
            let mut removed_src = RemovedSourceImpl {
                src: sys::RemovedSource { read: removed_read },
                messages: Some(removed_messages),
                cursors: &mut removed_cursors,
            };
            let mut diagnostic_src = DiagnosticSourceImpl {
                src: sys::DiagnosticSource {
                    read: diagnostics_read,
                },
                store: diagnostics_store.as_deref(),
            };
            let mut sink = SinkImpl {
                sink: sys::CommandSink {
                    reserve_entity: sink_reserve,
                    push: sink_push,
                },
                commands: &mut commands,
                queued: Vec::new(),
            };
            let call = sys::SystemCall {
                views: views.as_ptr(),
                view_count: views.len(),
                frame: sys::FrameCtx {
                    delta_secs: time.delta_secs(),
                    elapsed_secs: time.elapsed_secs(),
                },
                user: user as *mut c_void,
                iface: &IFACE,
                // Deliberately null: a `Host` handle only means something during
                // init, when the host holds `&mut World`. While this system runs
                // the world is borrowed by the query, so the init-time pointer
                // would be dangling — handing it over was a trap waiting for the
                // first plugin that called back.
                host: core::ptr::null_mut(),
                commands: (&mut sink as *mut SinkImpl).cast(),
                resources: slots.as_ptr(),
                resource_count: slots.len(),
                // Borrowed from the resource, which lives in the world and so
                // outlives the call. Never a temporary: a pointer to one would
                // dangle the moment this struct was built. Null when the host has
                // no input, which the guest turns into "nothing is pressed".
                input: plugin_input
                    .as_ref()
                    .map_or(core::ptr::null(), |i| &i.0 as *const sys::InputState),
                meshes: (&mut mesh_src as *mut MeshSourceImpl).cast(),
                images: (&mut image_src as *mut ImageSourceImpl).cast(),
                http: (&mut http_src as *mut HttpSourceImpl).cast(),
                removed: (&mut removed_src as *mut RemovedSourceImpl).cast(),
                replies: (&mut reply_src as *mut ReplySourceImpl).cast(),
                diagnostics: (&mut diagnostic_src as *mut DiagnosticSourceImpl).cast(),
            };

            // SAFETY: `entry` came from a `dlopen`'d library the loader keeps
            // alive for the process lifetime, and every pointer in `call` points
            // at a buffer that outlives this statement.
            let status = unsafe { entry(&call) };
            let queued = std::mem::take(&mut sink.queued);
            // `!is_known` counts as failure, not as success. A status this
            // build has no name for came from a plugin built against a newer
            // ABI, and treating it as `Ok` would write back output produced by
            // a system whose own report we could not read.
            if status == sys::SystemStatus::Panicked || !status.is_known() {
                error!("[plugin] system panicked — disabling it for this session");
                disabled.store(true, std::sync::atomic::Ordering::Relaxed);
                // Skip write-back: the plugin's partial output is not something
                // to trust into the world.
                return;
            }

            apply_queued(&mut commands, queued);

            for (state, q) in states.iter().zip(queries.iter_mut()) {
                state.scatter(q);
            }
        })
}

/// Copy one component out of storage into the plugin-facing representation.
///
/// `None` means the entity does not have it, which only happens for an optional
/// term — a required one was a precondition of matching the query.
fn read_cell(e: &FilteredEntityRef, t: &TermPlan) -> Option<Vec<u8>> {
    match t.marshal {
        Marshal::Transform => {
            let src = *e.get::<Transform>()?;
            let m = to_mirror(&src);
            // SAFETY: `sys::Transform` is `#[repr(C)]` and plain-old-data.
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    (&m as *const sys::Transform).cast::<u8>(),
                    size_of::<sys::Transform>(),
                )
            }
            .to_vec();
            Some(bytes)
        }
        // SAFETY: presence was just checked, and the component occupies
        // `cell_size` bytes because that is where the size came from.
        Marshal::Raw => e
            .get_by_id(t.id)
            .map(|ptr| unsafe { std::slice::from_raw_parts(ptr.as_ptr(), t.cell_size).to_vec() }),
    }
}

/// Copy one component back from the plugin-facing representation into storage.
fn write_cell(e: &mut FilteredEntityMut, t: &TermPlan, bytes: &[u8]) {
    match t.marshal {
        Marshal::Transform => {
            // SAFETY: `bytes` is exactly one `sys::Transform`, written by us.
            //
            // `read_unaligned` rather than a plain deref, matching every other
            // decode site. The buffer behind `bytes` is a `Vec<u8>`, which
            // requests align 1, while `sys::Transform` needs align 4. It happens
            // to work because allocators return aligned blocks and the row stride
            // preserves it — but that is an allocator property, not a guarantee,
            // and it was the only site in the crate relying on it.
            let mirror = unsafe { bytes.as_ptr().cast::<sys::Transform>().read_unaligned() };
            if let Some(mut dst) = e.get_mut::<Transform>() {
                *dst = from_mirror(&mirror);
            }
        }
        Marshal::Raw => {
            if let Some(mut ptr) = e.get_mut_by_id(t.id) {
                // SAFETY: same component, same size; the plugin owns this layout.
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        bytes.as_ptr(),
                        ptr.as_mut().as_ptr(),
                        t.cell_size,
                    );
                }
            }
        }
    }
}

// ── Mirror conversions ───────────────────────────────────────────────────────

fn to_mirror(t: &Transform) -> sys::Transform {
    sys::Transform {
        translation: sys::Vec3 {
            x: t.translation.x,
            y: t.translation.y,
            z: t.translation.z,
        },
        rotation: sys::Quat {
            x: t.rotation.x,
            y: t.rotation.y,
            z: t.rotation.z,
            w: t.rotation.w,
        },
        scale: sys::Vec3 {
            x: t.scale.x,
            y: t.scale.y,
            z: t.scale.z,
        },
    }
}

pub(crate) fn from_mirror(m: &sys::Transform) -> Transform {
    Transform {
        translation: Vec3::new(m.translation.x, m.translation.y, m.translation.z),
        rotation: Quat::from_xyzw(m.rotation.x, m.rotation.y, m.rotation.z, m.rotation.w),
        scale: Vec3::new(m.scale.x, m.scale.y, m.scale.z),
    }
}
