# Reactivity

How ember decides what to redraw, why it currently redraws too much, and the
design for fixing it.

The goal is the one SolidJS has: **a value changes, and exactly the things that
read it update — nothing else runs at all.** Ember is not there. This page says
precisely how far off it is, what has been verified, and what the staged route
looks like, so the analysis does not have to be redone.

## Where it stands today

`crates/renzora_ember/src/reactive.rs` registers reactions as *binding → target*
pairs. Every registered binding runs every frame:

```rust
let v = value(world);            // computed unconditionally
if last.as_ref() != Some(&v) {   // the diff happens AFTER
    apply(world, target, &v);
}
```

**The `PartialEq` diff is a write filter, not a work filter.** It does real and
necessary work — suppressing the component write is what stops the Bevy change
tick dirtying `ui_layout_system` and re-running taffy — but only after the
closure has already run. At the measured ~1–3% of bindings changing per frame,
97–99% of the compute is wasted, and it scales with the number of panels open.

### …but measure the magnitude before spending on it

That 97–99% is a **proportion**, and an earlier draft of this page stopped
there. It is the wrong number to plan against on its own. Measured in release
(`cargo test -p renzora_ember --lib reactive::bench -- --nocapture --ignored`),
against closures shaped like a typical `bind_text` — read a resource, format a
string, allocate:

| bindings | legacy | tracked | skipped | saved |
|---|---|---|---|---|
| 100 | 0.026 ms | 0.002 ms | 98/100 | 91% |
| 400 | 0.104 ms | 0.010 ms | 392/400 | 91% |
| 900 | 0.232 ms | 0.019 ms | 882/900 | 92% |
| 2000 | 0.512 ms | 0.046 ms | 1960/2000 | 91% |

The gate does what it claims — ~98% of bindings skipped, ~91% of the cost gone.
But **900 bindings only cost 0.23 ms/frame to begin with**, so migrating every
one of the ~850 remaining call sites buys back roughly **1% of a 60 fps frame**.
That is not an FPS mover, and it is not worth a workspace-wide sweep on its own.

Two caveats that push the other way, and are the reason this is "measure first"
rather than "don't bother":

- **The bench models no `keyed_list` at all.** Snapshots are the expensive
  closures — the asset browser hashing a folder every frame is the standing
  example — and there are 93 legacy `keyed_list` sites. The per-site win there
  is potentially orders of magnitude larger than for a `bind_text`.
- **Rebuild spikes are a different problem.** The inspector's ~4000-entity burst
  on selection is a *hitch*, not steady-state cost; tracking does nothing for it.
  That is S4's job, and S4 is independent of the tracking core.

Debug-profile numbers say "50% saved" and flatter the gate's relative cost
badly — the gate is fixed cost and the closure work is what optimises away.
**Always measure this in release.**

So the order of operations is: read `ms/frame recompute` and the `Top Cost`
table in the **UI Reactivity** panel *for the editor you actually have open*. If
recompute is well under a millisecond, reactivity is not the bottleneck and
further migration is busywork. If it is several milliseconds, `Top Cost` names
the handful of bindings worth migrating, and migrating those beats migrating all
851.

In Solid terms: ember has effects, and no signals. There is no
**data → binding** edge anywhere in the system, so nothing can conclude a
binding is clean without running it. `has_hidden_ancestor`, `keyed_list`'s token
and `virtual_scroll`'s windowing are three separate hand-rolled approximations
of that one missing edge.

The inspector is the worst case: **24 `bind_*` sites and zero `keyed_list`**, for
a panel that builds up to ~4000 entities.

## The target, in Solid's vocabulary

| SolidJS | Ember equivalent |
|---|---|
| `createSignal` | a component or resource in the `World` — already exists |
| dependency auto-tracking | **missing** — `Rx<'w>`, below |
| `createEffect` | a `bind_*` reaction — already exists |
| fine-grained DOM update | `apply(world, target, &v)` — already exists |

Only the second row is absent. Ember has the ends and not the middle: reactions
are already targeted at one entity, so once a reaction is *skipped* correctly,
the "only update that one item" half is already true today.

## Design

`Rx<'w> { world, deps }` — a tracking accessor that records every component and
resource a closure reads, into an interned `DepTable` carrying **subscriber
lists**. Push, not pull: a write marks its subscribers dirty, so steady state is
O(changed + dirty) and the total binding count drops out of the loop entirely.

Five things are already established and should not be re-litigated:

- **Inherent methods on `World` always beat extension traits**, so auto-tracking
  *requires* changing the closure parameter type. A dual-signature bridge that
  accepts both `&World` and `Rx` is an `E0119` coherence error, not a
  design choice.
- **Closure bodies still compile verbatim.** An inherent method on the wrapper
  with the same name shadows the `World` one. Only **32 sites workspace-wide**
  annotate `|w: &World|`; ~900 infer the type and need no edit.
- **`Rx` must not implement `Deref<Target = World>`.** Fall-through reads would
  record no dependency and go silently stale. A loud compile error is worth more
  than the convenience.
- **A half-migrated codebase is exactly as correct as today.** An empty
  dependency set is treated as always-dirty, so tracking can only ever remove
  work — it can never introduce staleness. This is what makes the migration
  safe to do in pieces.
- **The accessor surface is small**: ~800 `get_resource::<`, ~390 `resource::<`,
  ~630 `get::<`, zero `get_entity(`.

Lifetimes are handled structurally rather than by polling: a `BoundSlots`
component plus an `on_remove` hook drops a reaction deterministically when its
target despawns, which removes the current `retain_mut` liveness sweep.

### What is built

`crates/renzora_ember/src/reactive/` — `rx.rs` (the tracker), `tracked.rs` (the
`bind_*` / `keyed_list` constructors taking `Fn(&Rx)`), `tests.rs`, `bench.rs`.
The gate lives in `run_reactions` / `run_keyed_lists`; `ReactiveStats` gained
`skipped_this_frame`, surfaced in the UI Reactivity panel as `% skipped
(tracked)`. **0%** there means nothing on screen is migrated — inert, not broken.

Implemented as *pull* (compare each dep against Bevy's change ticks), not the
push subscriber lists sketched above. Pull turned out to be enough: the measured
per-binding check is far below the closure it replaces, and push only starts to
pay once the binding count is large enough for the per-frame scan itself to
matter — which, per the numbers above, it is not.

Two details that are load-bearing rather than incidental:

- **Dynamic dependencies.** `track_component_id` / `track_resource_id` +
  `manually_tracked` exist because the inspector reads every field by runtime
  `(ComponentId, offset)`, not by type — `Rx::get::<C>` cannot name what it
  depends on. Without these the inspector, the single most-suspected panel, was
  not migratable at all. `manually_tracked` is the one API in the design where
  being wrong causes staleness rather than wasted work, so it is only used where
  the `track_*` call sits directly above the read.
- **Tick clamping.** Bevy's `u32` change tick wraps, and it periodically walks
  components, resources and systems to clamp stale ticks — but it cannot know
  about the `last_run` ticks parked in ember's registries. Left alone, a
  long-lived reaction on a quiet dependency eventually falls outside the
  comparison window and reports clean *forever*: a panel that silently stops
  updating after the editor has been open long enough. A `CheckChangeTicks`
  observer rides Bevy's own notification.

### Wide readers bail out — the keyed-list regression

The first version of this shipped a dedup that scanned the recorded set
linearly, on the reasoning that "dep sets are tiny". That is true of a `bind_*`
closure, which reads one to a handful of slots. It is emphatically false of a
`keyed_list` snapshot, which reads across **every row it builds** — so recording
was O(n²) in the row count, plus a `TypeId` hashmap lookup per read.

Measured live in the editor:

| | before tracking | with tracking | after the cap |
|---|---|---|---|
| Bindings time | 0.14 ms | 0.09 ms | 0.09 ms |
| **Lists time** | **0.10 ms** | **1.33 ms** | *re-measure* |

One list (`ribbon`, 8 rows) went to 201 µs/frame on its own. The gate was
costing an order of magnitude more than the work it was gating — the exact
failure the "worst case" bench was supposed to catch, and did not, because that
bench only modelled `bind_*`-shaped closures. `report_wide_reader_cost` now
models the snapshot shape.

The fix is [`DepSet::MAX_DEPS`]: past 32 distinct slots a closure gives up,
marks itself untracked, releases the vector, and every later read takes a fast
path straight to the world. Bailing out is the right answer rather than a
cleverer container — a closure reading hundreds of slots will have *something*
among them change on almost every frame, so it would be reported dirty anyway.
What it buys back is a bounded cost, and the result is exactly the pre-tracking
behaviour for those closures, which is correct by the empty/untracked rule.

`Rx::bailed` is a `Cell<bool>` mirroring the flag *outside* the `RefCell`, so the
post-bail fast path is a bare `bool` load rather than a borrow-flag
increment/decrement and panic branch on every one of hundreds of reads.

The general lesson, worth keeping: **a per-read cost is fine for effects and
ruinous for iteration.** Any future work here should be benchmarked against a
snapshot-shaped closure, not just a binding-shaped one.

### Collapsed subtrees are parked, not dropped

A hidden binding should not be walked at all — but it must not be *dropped*
either, and the difference is the whole design here.

A collapsed subtree is hidden, not gone: `bind_display` flips `Node::display`
and the children keep their entities. There are ~357 of those toggles, and
**nothing rebuilds a subtree when it reopens** — a panel's `build` runs once.
The registry holds the only copy of each closure, so freeing one means the
section comes back permanently stale. Dropping is right when the target is
*despawned*; that is a different question, answered above.

So `run_reactions` **parks** them: the entry moves out of the walked list into
`ReactionRegistry::parked`, filed under the entity whose `Display::None` hid it.

Keying by that ancestor is what makes it pay. A flat list of parked entries
would still need an ancestor walk each, per frame, to notice a reopen — which is
what the old skip already cost. Filing under the collapsing entity turns it into
**one `Node` lookup per distinct collapsed root**, however many bindings sit
behind it. Collapse a 200-row section and it costs one lookup a frame instead of
200 walks.

Three properties the tests pin, each of which is a way to get this wrong:

- **Reopening restores and catches up.** The parked entry keeps its stale
  `last_run`, so the dep gate finds it dirty and it recomputes whatever it
  missed.
- **A despawned subtree drops its parked entries.** They are filed under an
  anchor that no longer exists, so the unpark pass returns them and the liveness
  check collects them. Parked entries are also swept for dead targets once a
  second, for the row that dies while its section is closed and would otherwise
  wait for an open that may never come.
- **A binding that hides its own target is never parked.** Only *ancestors*
  count. `bind_display` sets `Display::None` on the node it is bound to, and
  parking it on its own collapse would strand the node hidden with nothing left
  running to un-hide it.

`ReactiveStats::parked_total` reports the count; the UI Reactivity panel shows
it as *Parked (collapsed)*. `bindings_total` now means *walked this frame*, and
the Top Cost / Top Churn tables cover active bindings only — a parked one costs
nothing, so ranking it among the expensive ones would point at work that is not
happening.

### `bypass_change_detection` is now a staleness bug

Before tracking, bypassing change detection on a resource was a free
optimisation: bindings recomputed every frame regardless, so suppressing the
tick only saved downstream `Changed<T>` filters from firing.

With the gate that is no longer true. A binding's dependency *is* the change
tick, so a resource written via `bypass_change_detection` looks permanently
clean and **every binding reading it stops running**.

This was found the hard way. The UI Layout panel's stats were written with
`bypass_change_detection` — reasoning that a per-frame write would otherwise
dirty the panel's own bindings, in a panel whose purpose is to not add frame
cost. The numbers updated correctly and the panel sat frozen on its initial
zeroes.

The rule: **if any binding reads a resource, its writes must go through change
detection.** For a live profiler the per-frame dirty is the correct outcome —
it is what makes it live — and it costs only the handful of bindings that panel
owns.

Audited after the fix: the only other `bypass_change_detection` in editor code
is `text_input_sync`, which bypasses on a *component* inside a
`Changed<EmberTextInput>` loop to avoid re-triggering itself, and writes its
`Text` child directly rather than through a binding. Not affected.

### Liveness must be checked before the gate

Order matters in `run_reactions`, and getting it wrong is a leak rather than a
visible bug:

1. **target alive?** — drop the entry if not
2. hidden dock tab? — skip, keep
3. dependencies clean? — skip, keep
4. otherwise run

The only thing that reports `Dead` is the reaction closure itself, and steps 2
and 3 both keep the entry *without calling it*. Put liveness anywhere but first
and a binding whose target has been despawned while its dependencies happen to
be clean is never re-examined, so its entry survives for the rest of the
session.

That is not a rare shape. `dock::sync_panes` keeps exactly one pane alive per
leaf and **despawns every inactive one**, rebuilding on activation — so closing
a panel and merely switching tabs are the same event here. Without the check the
registry gains a few entries per tab switch and never gives them back, which is
the one way a dependency gate ends up costing more than it saves.

`run_keyed_lists` always had this order. Bindings did not, because before
tracking the closure ran unconditionally and caught the despawn every frame — the
gate is what introduced the gap. Two tests pin it
(`a_despawned_target_drops_even_when_its_dependencies_are_clean`,
`repeated_panel_cycles_do_not_grow_the_registry`); note that the older
`a_despawned_target_drops_its_reaction` does **not**, because it writes to the
dependency and so opens the gate itself.

### Migration status: done

**All 119 binding files import from `tracked::`; none remain on the legacy
path.** The `Fn(&World)` constructors still exist in `reactive` and still work —
they are what makes an unmigrated or partially-migrated file safe — but nothing
in the workspace uses them any more.

Roughly 250 `Rx::untracked` calls survive that migration. Each one is a read
that genuinely cannot be tracked, and each one pins its reaction permanently
dirty — i.e. exactly as expensive as before. The big families:

- `renzora` **contract-crate function pointers** — `InspectorEntry::has_fn`,
  `FieldSpec::get_fn`, `ToolEntry::visible`/`is_active`, `StatusItem::render`.
  These take `&World` and cannot take an `&Rx` without the zero-dependency
  contract crate depending on ember, which is not a trade worth making.
- **Whole-world scans** — `world.archetypes()`, entity iteration.

### Making contract function pointers skippable

The inspector's share of those was the biggest, and it turned out not to need a
contract change at all.

`InspectorEntry::has_fn`, `is_enabled_fn` and every `FieldDef::get_fn` are
`fn(&World, Entity)` living in the zero-dependency `renzora` crate. They cannot
take an `&Rx` — that would make the contract crate depend on ember. The first
pass therefore wrapped each call in `Rx::untracked`, which is correct but pins
the reaction permanently dirty.

The way out is to stop trying to *record* the read and instead **declare** it.
Of the 248 `get_fn` definitions in the workspace, 247 are literally

```rust
get_fn: |w, e| w.get::<C>(e).map(|c| FieldValue::Float(c.field)),
```

— one component, one entity — and the single exception ignores both arguments.
`has_fn` is `w.get::<C>(e).is_some()`; `is_enabled_fn` reads `C::enabled`. So the
dependency is known statically even though the body is opaque.

Every section already carries its component's type path (`InspectorEntry::type_id`,
`SectionSpec::type_id`, a reflected section's `type_path`), so `component_id_for`
resolves it through `AppTypeRegistry` once at section-build time and stores it on
the spec. The bindings then use:

```rust
fn tracked_read<T>(rx: &Rx, entity: Entity, cid: Option<ComponentId>,
                   f: impl FnOnce(&World) -> T) -> T {
    match cid {
        Some(cid) => { rx.track_component_id(entity, cid); f(rx.manually_tracked()) }
        None      => f(rx.untracked()),   // unresolved: unchanged behaviour
    }
}
```

`None` falling back to untracked is what keeps this safe: a type that is not
registered, or not a component, behaves exactly as it did before.

What is left untracked in the inspector is `collect_sections` — it calls
`has_fn` for *every* registered component, so its dep set would blow past
`MAX_DEPS` and bail anyway — and `DynEnum`'s `options`, which computes a list
from arbitrary world state and is the one shape where declaring a single
component dep would genuinely under-report.

The general recipe, for the `ToolEntry`/`StatusItem` pointers that are still
pinned: if the caller knows *what* the opaque function reads, declare it with
`track_*` and use `manually_tracked`. Only reach for `untracked` when it truly
could read anything.

### Migrating a file, and the one hazard

Swap `use renzora_ember::reactive::{bind_text, …}` for
`…::reactive::tracked::{bind_text, …}` and fix what stops compiling. Bodies that
only read through the accessors compile verbatim; bodies that hand `world` to a
helper fail loudly, and take either a threaded `&Rx` or `Rx::untracked`.

**The hazard is partial recording.** A closure that reads a tracked resource
*and* something untrackable — an `Instant`, an atomic, an `Arc<Mutex<…>>`, the
filesystem — records a non-empty dep set, so the gate engages, but the untracked
source can change without any dep moving. That is a genuinely stale binding,
and it is the only way to get one. A closure reading *only* untrackable state is
safe: it records nothing, and an empty set means dirty.

About 25 files reference such sources near their bindings. Each was checked
after the sweep: in every case the `Instant`/atomic/`Arc<Mutex>` read sits in an
ordinary Bevy system (a button-press loop, a `Query<&mut Text>` pass), not in a
binding closure — so no reaction currently mixes the two. `Rx::untracked` on the
whole closure is the correct answer if one ever does.

Worth re-checking whenever a binding is added to
`renzora_viewport::native_nav` (atomics behind `NavState`), since that is where
the two kinds of read live closest together.

## What this cannot do

Worth stating plainly, because it bounds the ambition:

- **Field-level granularity is not achievable.** Bevy's change detection is
  per-component, not per-field. So the value diff **stays** as a second gate —
  once tracking exists it is not redundant, it is the only thing distinguishing
  "the component was touched" from "the value I read actually changed".
- **False-positive dirties survive.** `DerefMut` marks a component changed even
  when nothing was mutated.
- **Non-ECS reads stay conservative.** A closure reading the filesystem, an
  `Instant` or an `Arc` cannot be tracked and must be treated as always-dirty.

## Staged route

`S4` is independent of the rest — ship it first if only one stage ever lands.

| Stage | What | Churn |
|---|---|---|
| **S0** | Hygiene: `run_if` on `build_reports`, a shared `Local` hidden-cache, gate the `Instant` pairs, `VecDeque` history, `Local<QueryState>` in the inspector, add the missing `Node`-write diffs | none |
| **S1** | Ownership. Also fixes a confirmed silent drop: a nested `keyed_list` built from a row builder is discarded with no panic and no log, because the registry is `resource_scope`'d out during `queue.apply` | none |
| **S2** | `Rx` + pull dependency checking — **built**; 11 production sites migrated (inspector plugin fields/resources) of ~850 | 32 sites |
| ~~**S3**~~ | ~~Push inversion~~ — **dropped**. The claim that pull alone would be "a wash" was never measured; it isn't. Pull skips ~98% of bindings at a per-check cost far below the closure, and the whole layer is only ~0.23 ms/frame, so push has nothing left to win | — |
| **S4** | Inspector sectioning: make `collect_sections` a `keyed_list`, turning a ~4000-entity burst into ~40. This is the visible 5–15 ms hitch on every entity selection | contained |
| **S5** | Parallel polling. Probably not worth it | — |

## Bugs to check before measuring anything

These came out of the 2026-07 audit and will distort any profile taken before
they are dealt with. **Re-verify each before acting — one has already been
fixed since**, and a stale finding is worse than no finding.

- ~~**`sync_inputs`** rebuilt a whole-world `Name → Entity` map every frame even
  with zero `<input>` on screen~~ — **fixed**. `markup/input_field.rs` now gates
  on `inputs.is_empty()` first; the comment there records the ~0.62 ms/frame it
  used to cost.
- **`apply_theme`** (`style.rs`) clobbers binding-written padding, and the
  source-value diff then makes the clobber *permanent*.
- **The inspector's signature** folds component field values through
  `is_enabled_fn` (37 of 39 impls read `s.enabled`), so the module doc's claim
  that field-value edits do not rebuild is false for enable toggles.

## The plugin-panel equivalent

C-ABI plugins have the same problem one layer up. `set_panel_content` replaces a
panel's markup wholesale, so a plugin cannot update one label without respawning
every widget in the panel — which drops input focus mid-keystroke. `ai_chat`
works around it by tracking a dirty flag per surface and never re-sending the
surface being typed into.

The fix is the same shape: a targeted `set_panel_field(panel, marker, value)`
that resolves a marker to an entity and writes one component. It rides the
service channel, so it costs no `VERSION_MINOR` bump. It is independent of
everything above.
