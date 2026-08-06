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
| **S2** | `Rx` + pull dependency checking | 32 sites |
| **S3** | Push inversion. **Not optional** — pull alone is a wash across the 272 `bind_display` sites | none |
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
