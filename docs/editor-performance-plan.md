# Editor performance plan

Working plan for the editor frame-time work. Derived from a real Tracy capture
(689 frames, Bistro scene loaded) plus a 31-agent audit of the reactive layer,
the inspector, and `bevy_ui` layout — 82 findings, 24 adversarially verified.

**Not published.** `.github/workflows/sync-docs.yml` mirrors only
`docs/r1-alpha*/**`, so this top-level file stays in the repo (same as
`ui_plan.md`, `renzora_lumen_plan.md`).

Costs marked **[measured]** come from the Tracy capture. Costs marked **[est.]**
are derived from the audit and have *not* been measured — treat them as
hypotheses to verify, not promises.

---

## Baseline

Fixed already: the editor was taking the XR-capable boot whenever an OpenXR
runtime was merely *installed and set as system default* (an idle Oculus runtime
with no headset attached counts). That boot disables `PipelinedRenderingPlugin`,
which serialises the render sub-app after the main world on one thread.
`cargo renzora profile` now sets `RENZORA_NO_XR=1` by default (`--xr` opts back
in). See `docs/r1-alpha7/editor-dev/profiling.md`.

| | XR-capable | flat / pipelined |
|---|---|---|
| avg frame | 28.28 ms (35.4 fps) | **17.90 ms (55.9 fps)** |
| p95 | 32.86 ms | 19.61 ms |
| p99 | 42.45 ms | 32.38 ms |
| GPU total | 2.90 ms | 2.74 ms |

**The editor is CPU-bound, not GPU-bound.** All GPU passes together are 2.74 ms
of a 17.9 ms frame. Every item below is therefore a CPU fix; nothing here is a
rendering-quality tradeoff.

Current critical path: `update` 17.02 ms = `main app` 14.81 ms + ~2.2 ms waiting
on the render thread. The render thread (12.38 ms) is off the critical path by
only 2.4 ms — **it becomes the next wall as soon as the sim gets faster**, so
Tier 1 items 1.4/1.5 are not optional for long.

Where `main app` goes:

- **PostUpdate 5.41 ms** — serial UI chain: `ui_layout_system` 2.70 →
  `ui_stack_system` 0.39 → `update_clipping_system` 0.35.
- **Update 6.86 ms** — almost entirely scheduler tax. Only ~1 ms is named work
  (`rebuild_scripts` 0.61 avg, `update_hierarchy_cache` 0.35). The rest is
  2.19 ms schedule self + 1298 executor tasks/frame, with 3064 system runs/frame
  averaging under 5 µs.

Hitches driving the p99 tail: `inspector::scripts::rebuild_scripts` **130 ms** in
one frame, `run_fixed_main_schedule` 7.31 ms, `mark_dirty_trees` 5.42 ms,
`prepare_windows` 10.16 ms.

---

## Measure `main app`, not fps

**From Tier 0 onward, fps is a misleading metric on this machine.** After
0.1 + 0.2, ~28% of frames sit at 16.4-17.0 ms — the 60 Hz vsync cap — and
`prepare_windows` blocking rose 1.21 → 2.20 ms avg. That rise is the surface
wait, not new work: the sim finished early and the frame parked on vsync.

The tell: `main app` fell **1.20 ms** but avg frame only **0.82 ms**. The
difference was absorbed by vsync. Track `main app` ms for every remaining tier;
quote fps only as a sanity check.

### ~~Rebuild-burst zones under-read their own cost~~ — RETRACTED, was a Tracy artifact

**This was wrong.** The original claim — that a rebuild's layout lands on the
*following* frame, based on `ui_layout_system` being absent from the frame where
`rebuild_picker` cost 8.04 ms — has been disproven by direct measurement:

- Main schedule runs that straddle a Tracy frame mark: **909/909 (100%)**. Under
  pipelined rendering the frame mark is emitted by the *render* thread, so it
  never aligns with a main-world schedule run.
- An `Update` system and `ui_layout_system` landing in the same `Main` run:
  **400/400**. Rebuild cost and its layout are **always the same frame**, exactly
  as the schedule ordering implies (`Update` precedes `PostUpdate`).
- But they are filed under *different Tracy frame indices* **~15% of the time**.

The frame-724 observation was a 1-in-7 bucketing artifact, not evidence.

**Protocol, now mandatory for any rebuild-burst measurement:** bucket by
`schedule{name=Main}` occurrence, **not** frame index. `get_zones_in_frame` will
silently split a burst across two buckets about one time in seven.

### Idle vs interaction profiles are not comparable

Numbers captured while driving the UI (typing, opening popups) are a different
regime and must not be diffed against an idle baseline. Measured: idle
`main app` 13.61 ms vs interaction 16.20 ms; avg frame 16.80 vs 18.79; p95 18.86
vs 31.29. Under keystroke load `ui_layout_system` rises 2.39 → 2.93 ms and
`sync_inputs` 0.605 → 0.709 ms — i.e. **layout costs more during interaction than
the idle baseline suggests**, which raises the value of 0.6 / 1.1 / 1.2 above what
the idle capture implies. Always label a capture idle or interactive.

---

## Results — Tier 0 items 0.1 + 0.2 (725 frames)

| | before | after 0.1 + 0.2 | |
|---|---|---|---|
| **main app** | 14.81 ms | **13.61 ms** | **−1.20 ms** |
| avg frame | 17.62 ms (56.7 fps) | 16.80 ms (59.5 fps) | −0.82 ms |
| p50 | 16.66 | 16.52 | −0.14 |
| p95 | 19.59 | 18.86 | −0.73 |
| p99 | 28.83 | 27.38 | −1.45 |
| worst frame | 191.34 | 59.31 | **−132 ms** |

**0.2 (`scan_scripts` cache) — confirmed emphatically.** `rebuild_scripts`
0.609 → **0.012 ms** avg (50×), and its 130.06 ms spike → 0.16 ms max. The worst
frame in the whole capture dropped by 132 ms. The hitch is gone.

**0.1 (`ghost_nodes`) — real but modest; the estimate was wrong.**
`ui_layout_system` 2.704 → **2.390 ms** (−11.6%), `ui_stack_system` 0.39 → 0.30,
`update_clipping_system` 0.345 → 0.254; serial UI chain 3.44 → 2.94 ms (−14%).

That is well short of what this document originally implied ("large slice of
2.70 ms"). The mechanism was real — one `Box::new` per UI node per frame — but it
is only the *per-node constant*. It does nothing about **why the tree re-solves
every frame**, which is the actual dominant term and is what 0.6 / 1.1 / 1.2
address. Treat every remaining **[est.]** number in this document with that
correction in mind: the audit reliably identifies *mechanisms*, and reliably
over-weights their *share*.

### Hitch leaders after Tier 0

With the 130 ms `rebuild_scripts` spike gone, the new tail is:

| system | max | avg | shape |
|---|---|---|---|
| `bevy_ui::widget::text_system` | 18.68 ms | — | burst on mass Text spawn |
| `material_editor::native_material_ref::rebuild_picker` | 13.92 ms | 0.051 | rebuild burst |
| `inspector::native::rebuild_inspector` | 6.36 ms | — | rebuild burst — item 1.1 |

**`rebuild_picker` is two bugs compounded** (new item 0.9 below).

### Unchanged after Tier 0

- Scheduler tax: 3079 sub-5 µs system runs/frame (3.95 ms) + 1291 executor tasks
  (4.98 ms). `Update` is 6.09 ms with ~0.4 ms of named work. Only item 1.3 touches
  this.
- Render thread is now **co-critical** with `main app` (13.61 ms real, once the
  vsync wait is subtracted). Items 1.4 / 1.5 — `camera_driver` 3.44 ms encode and
  the light-probe env map still regenerating every frame (0.67 ms) — are the next
  wall, exactly as predicted.

### 0.9 Fix `rebuild_picker` (found by the post-Tier-0 capture)

`crates/renzora_material_editor/src/native_material_ref.rs:392-440` ·
**[measured: 13.92 ms max]**

Two independent bugs in one function, and it is worth fixing as the template for
both classes:

1. **Inline filesystem walk** — `rebuild_one_picker` calls `find_material_files`
   (`material_inspector.rs:98`, a depth-limited recursive `read_dir` of the whole
   project) directly, on the main thread. Identical to the 0.2 bug.
2. **Eager unbounded list build** — it builds up to 200 `picker_item`s at once
   (`.take(200)`), each several entities. There is **zero** `keyed_list` or
   `virtual_scroll` use in the file, though `virtual_scroll` exists for exactly
   this.

And the trigger makes both worse: `bind_search` (`:499-512`) bumps
`MatPickerFilter.sig` on **every keystroke**, and `rebuild_picker` rebuilds on any
sig change (`:385`). So **typing one character in the material search box does a
full project filesystem walk and rebuilds up to 200 UI items.**

**0.9a — the fs walk: DONE.** Landed as a `MaterialIndex` resource mirroring
`ScriptIndex`: the walk runs on the IO task pool, publishes an `Arc` snapshot, and
bumps `MatPickerFilter.sig` **only when the file set actually changed** (a no-op
rescan must not churn the list under the user — a rebuild despawns every row and
its thumbnail binding). `rebuild_one_picker` now clones the `Arc`. The stale doc
comment on `find_material_files` (which claimed "well under a millisecond" and
"rebuilds every frame it's open") has been corrected to say it must never be
called from a system.

*Measured (809 frames, picker driven hard, 11 rebuilds): `rebuild_picker` max
13.92 → **8.04 ms**. `refresh_material_index` costs **3.3 µs avg** / 0.25 ms max
over 807 runs — the filesystem term is off the frame entirely. The distribution
confirms the diagnosis: 793 of 806 runs are sub-100 µs and the 11 expensive ones
line up one-per-keystroke (8.04, 7.84, 7.11, 6.04, 5.26, 4.20, 2.78, …), exactly
the shape expected once the walk is gone and only the row build remains.*

**0.9b — the eager 200-row build: JUSTIFIED, but as maintenance, not a fire.**
1–8 ms per keystroke, not a 130 ms-class hitch; only 1 of the 11 rebuilds produced
a >25 ms frame. **Do it with 1.1** — same mechanism, and it wants one
virtualization design rather than two. Note the 8.04 ms zone is an undercount: see
the rebuild-burst caveat above.

Each
`picker_item` spawns ~5 entities *plus* a `bind_with` thumbnail binding, so a
200-row list is ~1000 entities and 200 new bindings per keystroke — and the old
200 bindings only die by despawn-detection on the *next* frame, so both sets
coexist for a frame. Converting the list to `keyed_list`/`virtual_scroll` would
make filtering reuse rows instead of despawn/respawn.

Staging this separately paid off as a method: 0.9a alone was measured before
committing to the refactor, which confirmed the filesystem term was the whole
emergency and reduced 0.9b from "urgent" to "worth doing properly, with 1.1".

---

## Tier 0 — do now

Cheap, independent, and aimed at costs we actually measured. Any order.

### 0.1 Drop `"ghost_nodes"` from the bevy feature list — DONE

`Cargo.toml:149` · **[measured target: 2.70 ms]** · **moves the ABI**

*Landed. Removed from the feature array; the reasoning (including the
non-equivalence caveat below) is recorded in the manifest's "Deliberately
OMITTED" block. `cargo check --workspace` clean.*

Verified before landing: zero `GhostNode` references anywhere in the workspace
or any vendored crate; `bevy_feathers` / `bevy_ui_widgets` / `bevy_ui_render`
neither use nor enable it; the feature is a leaf in the registry-wide graph, so
nothing transitively re-enables it.

**Caveat worth recording** — the two `UiChildren` impls are *not* equivalent
beyond cost. The ghost `iter_ui_children` filters children to
`Or<(With<Node>, With<GhostNode>)>`; the non-ghost one yields the raw `Children`
slice unfiltered. That is safe here for two independent reasons: ghost traversal
always required the explicit `GhostNode` marker (a bare component-less child was
never traversed under either setting), and all five `bevy_ui` consumers
(`stack`, `update`, `ui_surface`, `layout`, `accessibility`) re-filter on `&Node`
themselves. It is a latent coupling to bevy internals, not a current break.

The feature is enabled and used **zero** times in the workspace (0 `GhostNode`
hits outside vendored bevy). It is opt-in, not a bevy default. Turning it on
swaps `UiChildren` for a much slower implementation: `iter_ui_children` builds
and reverses a `SmallVec<[Entity;8]>` — heap-allocating for any node with >8
children, which dock lists, hierarchy rows and asset grids all exceed — and does
a filtered `query.get()` per child, where the non-ghost path returns a plain
slice iterator with no allocation and no per-child lookup. Worse,
`UiChildren::is_changed` short-circuits only when children actually changed, so
in steady state every node falls through to `iter_ghost_nodes`, which does
`Box::new(...)`: **one heap allocation per UI node per frame**. It is called ~3×
per node per frame inside `ui_layout_system`, plus once in `ui_stack_system`.

Recompiles `bevy_dylib` and so moves the plugin ABI (CLAUDE.md §3) — fine under
the source-first model, but it is a full rebuild and prebuilt community plugins
would need rebuilding.

### 0.2 Cache `scan_scripts` — DONE

`crates/renzora_inspector/src/scripts.rs` · **[measured: 130 ms hitch]**

*Landed as a `ScriptIndex` resource: the walk runs on the IO task pool (whose
workers have the 32 MiB stacks from `init_io_task_pool_with_large_stack`) and
publishes an `Arc` snapshot when it lands. `rebuild_scripts` now clones that
`Arc` — no syscalls on the main thread at all.*

Two details that are load-bearing:

- **The index hash is folded into `scripts_sig`.** `build_add_bar` *moves* the
  available-script list into the Add-Script menu's click closure, so the menu is
  a baked snapshot, not a live read — without this a newly created script never
  appears until something else forces a rebuild.
- **…but it is a content hash, not a generation counter.** A counter bumped on
  every rescan would rebuild the drawer every throttle window, and a rebuild
  despawns every child — destroying focus and in-progress typing in a
  script-variable field.

**No file-watch signal exists to hook**, which is why this polls: scripts are read
with raw `std::fs` by the backends and never go through the `AssetServer`, so
bevy's `file_watcher` never emits an event for a `.lua`/`.rhai`. Nothing in
`crates/` consumes bevy's watcher for project files. This matches what every
comparable path in the codebase already does (asset-browser listing throttle,
`check_script_hot_reload`'s 0.5 s timer). Follow-up if 3 s ever proves too slow:
a `dirty` flag set from the asset browser's four existing edit sites — but that
needs a signal in the `renzora` contract crate to cross the boundary.

*Original problem, for reference:* `rebuild_scripts` (exclusive) called
`scan_scripts` → a recursive `read_dir` of the entire project tree,
synchronously on the main thread, skipping `target`/`node_modules`/`.git` but
**not** asset directories.

`rebuild_scripts` is an exclusive system. When the script signature changes it
calls `scan_scripts` → `scan_scripts_inner`, which **recursively `read_dir`s the
entire project tree** synchronously on the main thread. It skips
`target`/`node_modules`/`.git` but not asset directories, so a project like
Bistro walks thousands of files. This is the visible stutter.

Fix: cache the result and invalidate on file-watch events (bevy's `file_watcher`
feature is already enabled), or move the scan to an async task. It should never
run inline in an exclusive system.

### 0.3 Gate `sync_inputs` on `<input>` nodes existing — DONE

Two gates, cheapest first: bail entirely when no `<input>` exists (the common
case, and the one the 0.62 ms was measured in), then build the name map only if
some input actually uses a dotted `Entity.var` bind — `resolve_bind` consults the
map for nothing else, and an empty map is *correct* if that test is ever wrong,
since it degrades to the same `None` as an unresolved name.

Note this was nearly skipped alongside 0.6-0.8 under "main-world is invisible".
That lumping was wrong: at **0.62 ms measured** it is 10-20× those items, and a
whole-world scan with a `String` per named entity every frame is worth removing
whether or not it currently reaches the frame.

`crates/renzora_ember/src/markup/input_field.rs:125` · **[measured: 0.62 ms]**

Rebuilds a whole-world `Name → Entity` HashMap, allocating a `String` per named
entity, **every frame, unconditionally — even when zero `<input>` nodes exist**.
It also duplicates the already-dirty-gated `MarkupNameIndex` in
`markup/binding.rs`. Either early-out when no inputs are present, or reuse
`MarkupNameIndex`.

### 0.4 Gate the reactive stats reports — DONE

`crates/renzora_ember/src/reactive.rs:42, 496, 519` · **[est. 20-60 µs/frame]**

`ReactivePlugin` registers its drivers with **no `run_if` at all**. Every 30
frames `build_reports` does two O(N log N) sorts plus an unbounded per-list
`entry_label` `format!` — whether or not the UI Reactivity panel is open. Its
only consumer is `renzora_debugger/src/native/reactivity.rs`. Gate on
`panel_active("ui_reactivity")`, the established pattern in `dock.rs:713`.

### 0.5 Cache the inspector's `QueryState` — DONE

`crates/renzora_inspector/src/native.rs:767` · **[est. tens of µs/frame]**

`world.query_filtered::<Entity, With<InspectorRoot>>()` builds a fresh
`QueryState` every frame. Because `With<T>` goes through `and_with` and never
populates `FilteredAccess::required`, `update_archetypes` takes the
from-generation-0 full-scan branch. Use a `Local<QueryState>`.

### 0.6-0.8 — DELIBERATELY NOT DONE (superseded by the critical-path inversion)

These were sized when the **main world** was the critical path. It no longer is:
`main app` sits below `sub app{RenderApp}`, so main-world savings are absorbed by
vsync and never reach the frame. At ~30-60 µs each they are not worth the churn,
and 0.7's `VecDeque` change would alter a `pub` field's type for a ~960-byte
memmove. Revisit only if the render thread comes down far enough to make the main
world the wall again.

Kept below for the record; each is still accurate as a description of the waste.

### 0.6 Equality-guard the unguarded `Node` writes

`markup/widgets.rs:232`, `markup/vector.rs:559`, `renzora_export/src/native.rs:776`

`value_fill_system` writes `Node.width` through `get_mut` with no equality guard,
so it marks the node changed every frame and dirties the ancestor chain via
`bevy_ui`'s `node.is_changed()` gate, even when the bound value is static.

`reactive.rs::bind_display` is **not** a culprit — verified clean, and doubly
guarded (`bind_with_kind` only applies on a value change, and the applier then
compares `n.display != d` before the `DerefMut`).

### 0.7 Reuse the hidden-ancestor cache; drop the history memmove

`crates/renzora_ember/src/reactive.rs:455, 645, 757` · **[est. 30-60 µs/frame]**

`hidden_cache` is a fresh `HashMap` allocated independently in each of the two
chained drivers, so capacity is thrown away every frame and ancestor chains
already resolved for bindings are re-walked cold for list containers. Make it one
shared `Local<HashMap>` with `.clear()`. Separately, `history_us.remove(0)` is a
960-byte memmove every frame — use a `VecDeque`.

### 0.8 Put the instrumentation timers behind a flag

`crates/renzora_ember/src/reactive.rs:465-467, 657-742` · **[est. 50 µs - 1.6 ms]**

Two `Instant::now()` reads per executed entry per frame, with no feature or
runtime toggle. ~50 µs/frame at N=1000 on TSC-backed QPC — but **~1.6 ms/frame if
QPC falls back to HPET**, which is machine-dependent and worth ruling out. Read a
`ReactiveProfiling` flag once per frame instead of timing every entry.

---

### 0.10 Font-size ladder — DONE, but a **null result** on frame time

`crates/renzora_ember/src/font.rs` · **memory win only — keep it, don't tighten it**

**Measured A/B (904 frames per leg, both pipelined, same script):** the ladder
makes **no measurable CPU difference.**

| | ladder ON | ladder OFF |
|---|---|---|
| avg frame | 16.78 ms | 16.79 ms |
| `text_system` max | 14.54 ms | 14.78 ms |
| events >5 ms | 9 | 9 |
| total text work | 168.0 ms | 177.5 ms |
| `ui_layout_system` | 2.754 ms/f | 2.392 ms/f |

The prediction in this document was "13.4 ms → low single digits". Measured 14.54,
i.e. indistinguishable from the leg *without* the feature. The 5% gap in total text
work is inside the noise floor, which this pair conveniently calibrates:
`ui_layout_system` moved 0.36 ms/frame in *favour of the ladder-OFF leg*, so
sub-0.4 ms/frame differences here mean nothing.

**Keep the ladder anyway** — it is a real result, just not a frame-time one. Six
rungs cut standing atlas cost ~10× (each `(font, size)` owns a 1 MiB 512×512 RGBA
atlas; ~59 pairs ≈ 59 MiB → ~6 MiB) and shrink the un-evicted `FontAtlasSet` leak
rate by the same factor. **Do not tighten it further for performance**, and do not
chase the 19 bypassing chrome sites: the visual redesign needed to reach 2-3 sizes
per font buys nothing measurable.

**Why the hypothesis failed — and what the test cannot prove.** Two caveats,
recorded so nobody re-runs this reasoning:

1. 6 rungs × 3 fonts is still up to ~18 live keys against swash's 8 slots, so the
   ladder may simply not have crossed the cliff rather than the cliff not existing.
   Either way, further tightening is speculative work against an unmeasured
   mechanism.
2. **"The ladder has no CPU effect" and "`RENZORA_NO_FONT_LADDER=1` never reached
   the process" produce byte-identical CPU traces.** There is no trace-visible
   signature to separate them — no allocation tracking compiled in, no plots, no
   font-atlas zones, and atlas count is a *memory* property this capture cannot
   see. The discriminator is RSS: `Get-Process renzora | Select WorkingSet64`
   under each leg. Tens of MiB apart → the var is live. Within a few MiB → the A/B
   was one binary tested against itself. **Always pick a discriminator outside the
   metric you are testing when a feature's kill-switch is an env var** — a prior
   leg in this same investigation silently lost `--no-xr` from its command line.

**The real conclusion: `text_system` is not its own item.** The burst cost is per
**new text entity**, not per `(font, size)` pair — hundreds of entities shaped at
once when the inspector or picker repopulates. That is the same root as 1.1 and
0.9b, so fold it into the virtualization work.

And the peak has survived every fix so far: **18.68 → 13.39 → 14.54 → 14.78**
across four captures is **one stable phenomenon, not a trend** — the apparent
early improvement was capture variance, not progress.

<details><summary>Original hypothesis (kept for the record — the mechanism is real, its share was not)</summary>

Rasterizing a glyph run builds a hinted `swash` scaler, cached in a table of
exactly **8** entries (`MAX_CACHED_HINT_INSTANCES`), keyed on
`(font id, size, variation coords)` and shared across every TrueType font. On a
miss it re-runs the font's `fpgm`/`prep` bytecode through the interpreter. Bevy
compounds it by building that scaler *before* checking the glyph atlas
(`bevy_text-0.19.0/src/pipeline.rs:347-353`, cache check at `:361`). Ember spread
text across ~59 distinct pairs — 24 UI sizes + 9 mono + 2 phosphor-via-`ui_font`
+ 24 phosphor-via-`icon_text`. The arithmetic appeared to match exactly
(~1000 nodes × ~15-20 µs ≈ 15-20 ms vs the observed 18.68 ms). It did not survive
measurement.

</details>

*What landed:* `FONT_SIZE_LADDER` (6 rungs) + `snap_font_px()` in `ui_font()` and
`icon_text()`, covering ~889 call sites; the three dominant sizes (11.0 → 297
sites, 10.0 → 142, 12.0 → 120) collapse onto two rungs.
`RENZORA_NO_FONT_LADDER=1` restores the old behaviour.

**Left alone deliberately, and now permanently** (the null result removes the
reason to touch them): 19 editor-chrome sites build `TextFont` directly instead of
going through the helpers — 7 in `renzora_hierarchy/src/native` (5 in `row.rs`,
spawned per hierarchy row) plus singles in viewport/shell/settings/inspector. A
further 19 in `renzora_ember/src/game_ui` are the shipped-game UI surface, where
changing default text sizes is a product decision, not a perf one.

**Related bug, still open — a memory bug, not a frame-time one.** `FontAtlasSet`
has **no eviction**: a bare `HashMap` no cleanup system ever touches. Each
`(font, size)` owns a 512×512 RGBA atlas (**1 MiB**), and every UI Scale or Font
Size change mints a whole new generation and **permanently strands the previous
one**. The ladder reduces the leak *rate* ~10× but nothing reclaims a stranded
generation. Worth fixing on its own terms; it will never show up in a CPU profile.

### 0.11 Filesystem I/O on per-frame paths — the systemic sweep

A sweep of all 104 non-vendored files touching `read_dir` / `exists()` /
`metadata()` / `read_to_string` found **12 confirmed hot-path instances** beyond
the two already fixed (0.2, 0.9a). Highest value first:

| site | trigger |
|---|---|
| `renzora_scripting/src/backends/lua.rs:141` | `metadata()` per scripted entity per frame — filesystem mode only (editor Play/Simulate/preview), not VFS-backed exports |
| `renzora_splash/src/native.rs:817` | stats `project.toml` per recent project **twice per frame** (tokenless `keyed_list` *and* a `bind_display`) |
| `renzora_scene/src/panel.rs` | `read_dir`s the scenes directory every frame the tab is active |
| `renzora_tutorial/src/state.rs:499` | walks the **entire project tree** every frame during the Import Model step |
| `renzora_daw/src/waveform_cache.rs:132` | re-fingerprints every timeline clip from disk every frame |
| `renzora_code_editor/src/native_scripts.rs:178` | stats every script row every frame (`virtual_scroll` passes no token) |
| `renzora_code_editor/src/lib.rs:65` | stats before its `last_sig` gate |
| `renzora_shell/src/lib.rs:256` | stats theme files every frame in an ungated `Update` |

**The systemic finding is worth more than the list:** four of these apply their
change-gate *after* the filesystem work rather than before — and three carry doc
comments claiming they avoid per-frame filesystem cost. "Stat, then decide nothing
changed" is the recurring shape. Worth a review rule, and worth grepping for on
every new panel.

Individually these are sub-millisecond; none is a 130 ms-class hitch. Given this
document's demonstrated habit of over-weighting mechanisms, **measure before
fixing** — the two `keyed_list`/`virtual_scroll` tokenless ones and the
per-frame-per-entity `lua.rs` stat are the only ones likely to show up.

## iGPU quality tiers — the original 13 fps problem

The Graphics Quality tier system (`renzora_level_presets/src/graphics_quality.rs`)
already existed and is well built: it forces the tier onto the *routed viewport
cameras* rather than the authored scene sources, so it can never bleed into a
saved scene, and it re-pokes `EffectRouting` on a tier change so raising the tier
restores cleanly. Default is `Medium`, which drops SSGI.

### SSAO added to the tier gating — DONE

`crates/renzora/src/core/viewport_types.rs` (new `ssao()` predicate) +
`graphics_quality.rs` (removes `ScreenSpaceAmbientOcclusion` at `Low`).

SSAO was **ungated at every tier**, including `Low` — so a user who explicitly
picked `Low` for frame rate still paid for it. Profiling ranked it **second only
to the deferred prepass** among GPU passes (0.46 ms of a 2.63 ms GPU frame on a
discrete card; proportionally far worse on the integrated GPUs `Low` exists for).
It is a fullscreen, resolution-bound pass — exactly the class the tiers target.

`cargo check --workspace` clean, both files clippy-clean. No UI string to update
(the settings row is a bare label with no per-tier description).

**Verified on desktop (current tier → Low):**

| pass | before | Low | |
|---|---|---|---|
| ssao | 0.598 | **0.000** | zone absent entirely |
| bloom | 0.173 | **0.000** | zone absent entirely |
| taa | 0.116 | **0.000** | zone absent entirely |
| early deferred prepass | 0.763 | 0.620 | −0.143 (second-order) |
| main_opaque_pass_3d | 0.527 | 0.433 | −0.094 (second-order) |
| **GPU total** | **2.870** | **1.798** | **−37%** |

16 passes → 13. CPU side follows: SSAO's prepare/bind-group systems 0.341 → 0.015
ms/f, bloom 0.397 → 0.031, TAA 0.094 → 0.026.

**SSAO alone was 21% of all GPU work** — larger than bloom + TAA combined (0.598 vs
0.289). On a fill-rate-bound integrated adapter that ratio should be *worse*, since
SSAO is fullscreen and resolution-bound while the geometry passes are not.

Two passes also got cheaper without being removed (prepass −0.143, main opaque
−0.094): ~0.26 ms of second-order saving, presumably work those passes were doing
to feed SSGI.

**The prepass caveat is confirmed and it is the ceiling.** `early deferred prepass`
is still the largest GPU pass at Low — 0.620 of 1.798, i.e. **34% of GPU time**,
up from 27%. Low thins the fullscreen stack by 37% and then hits exactly the wall
documented in `renzora_engine::camera` (prepass attachment layout is fixed at
camera spawn; toggling it at runtime trips a wgpu validation crash).

Frame time is unchanged on desktop, as expected — it is CPU-bound with the GPU
idle. The render thread got 2.1 ms lighter (RenderApp 12.44 → 10.34) and the main
world absorbed the slack via vsync; the corresponding `main app` rise is **not** a
regression.

#### ⚠ Known gap: auto-exposure is NOT gated despite the predicate saying it is

Measured: `auto_exposure` 0.045 ms/f GPU and 0.115 ms/f CPU across 16 zones at
`Low` — statistically unchanged from 0.047 / 0.114 before. So the claim "Low drops
SSGI, SSAO, bloom, TAA and auto-exposure" is **wrong about the last one**; the
first four are confirmed.

`GraphicsQuality::auto_exposure()` correctly returns `false` at `Low`, and
`renzora_auto_exposure` does insert bevy's own
`post_process::auto_exposure::AutoExposure` (`lib.rs:193`) — the same type the
enforcement query filters on. So the predicate and the type both look right and
the enforcement still is not taking effect. **Cause not determined.** Candidate
leads, none verified:

- The gate filters `With<ViewportCamera>`, but `EffectRouting` also builds targets
  from `scene_cameras` (`renzora_viewport/src/effect_routing.rs:63-64`) — copies on
  a non-`ViewportCamera` target would never match the gate. Does not obviously
  explain why bloom/TAA/SSAO *did* work through the same routing, so this is
  incomplete as a theory.
- `sync_auto_exposure` re-applies when `AeCompensation.is_changed()`, not only on
  source/routing change. If that resource is touched every frame the router would
  re-insert every frame, fighting the PostUpdate strip.

**Refinement:** bevy registers `auto_exposure` as a **`Core3d` graph system with
no run condition** (`bevy_post_process .../auto_exposure/mod.rs:80-84`), so the
zone is emitted per view every frame whether or not that view carries the
component. **So "16 zones still present" is expected and is NOT evidence the gate
failed** — unlike SSAO/bloom/TAA, whose zones vanish entirely. The only real
signal is that the *cost* is unchanged (0.047 → 0.045).

Ruled out by inspection: `build_compensation_curve` is properly key-gated and does
not touch `AeCompensation` every frame (`ResMut` marks changed on `DerefMut`, and
it early-returns first), so the "router re-inserts every frame" theory is dead.
Nothing outside `renzora_auto_exposure` inserts `AutoExposure` — the only other
mentions workspace-wide are the gate itself and a debug-log string.

Remaining untested hypothesis, which mirrors a bug already confirmed elsewhere in
this document: bevy keeps `AutoExposureBuffers` / the extracted per-view buffer
alive after the component is removed, exactly as `prepare_uniform_components`
retains its uniform buffer (see 1.3b). If so the dispatch survives the component
and no amount of component-removal will gate it.

**This needs runtime evidence, not more source reading** — a single log line in
the gate's removal branch would settle in one launch whether the removal fires at
all. Small on desktop; worth resolving before the laptop measurement.

### Integrated-GPU hint — DONE

`renzora::GpuIsIntegrated` (contract) + `renzora_runtime::gpu_is_integrated()`
(probe) + `graphics_quality::suggest_low_tier_on_integrated_gpu` (the hint).

The probe reuses the `raytracing_supported()` shape exactly — `OnceLock`-cached,
mirroring `platform_wgpu_settings()`' backend selection so it sees the same
adapter the renderer will — and publishes to the contract dylib next to
`GpuRaytracing`, resolved before dlopen plugins load.

**A hint, never an action.** Nothing changes the user's tier for them: a silently
applied override is indistinguishable from the engine misbehaving, and someone on
integrated graphics may have chosen their tier deliberately.

**No "already asked" flag on disk, because the condition is self-clearing.** It
fires only while the tier is not already `Low`, so acting on it silences it
permanently; ignoring it costs one toast per launch, which is the right pressure
for a hint the user hasn't acted on. `wgpu::DeviceType::Other` counts as *not*
integrated — it usually means a driver that didn't report a type, and wrongly
nudging a discrete-GPU user toward `Low` is worse than staying quiet.

String is `settings.hint.integrated_gpu` in `languages/en.toml`; the other 19
packs degrade to English until translated.

### ~~Remaining gap: no hardware detection on first run~~ — resolved above

The tier **is** persisted (`PersistedViewportSettings.graphics_quality`, a label
string; `default_graphics_quality()` supplies `Medium` when absent). What is
missing is that nothing ever *derives* the default from the hardware, so an
integrated-GPU user gets `Medium` — bloom + TAA + auto-exposure + (until now)
SSAO — unless they find Settings → Viewport → Performance themselves.

The engine already has the probe it needs: `renzora_runtime::raytracing_supported()`
does a `OnceLock`-cached wgpu adapter request, and `wgpu::AdapterInfo::device_type`
distinguishes `IntegratedGpu` / `Cpu` from `DiscreteGpu`. The runtime already
publishes one probe result to the contract crate as a resource (`GpuRaytracing`),
so the pattern exists.

The open question is **not** how to detect, it is **who to apply it to**:

- Applying only when the config has *no* tier recorded is safe but nearly
  useless — the field is written on every settings save, so existing users
  (including the one with the 13 fps machine) already have `"Medium"` on disk and
  would be treated as having chosen it.
- Overriding an existing `"Medium"` reaches those users but silently changes a
  setting the user may have deliberately picked.

Needs a product decision before implementing. See the options recorded with it.

### Splash residual cost — DONE (mechanism verified, magnitude below noise)

`renzora_splash/src/native.rs`, `native_loading.rs`, `native_post.rs`

Three exclusive systems (`manage_splash`, `manage_loading_screen`,
`manage_editor_overlay`) polled every frame for the editor's entire life to
rediscover they had nothing to do. All three now carry **self-clearing** gates —
`in_state(..).or_else(any_with_component::<Root>)`, because each **builds *and*
tears down**: a plain state gate would stop them on the frame the state left and
strand the splash UI on screen forever.

**Verified by count, not time:** all three went **1.00 → 0.00 runs/frame**. Zero,
not cheaper — which is the whole value of count-based verification, since the
timing was never going to resolve.

**Magnitude:** `main app` 10.560 → 10.413 (−0.147), splash zones 0.251 → 0.206
(−0.045). The zone delta matches the three systems' own measured total
(0.018+0.018+0.014 = 0.050) almost exactly; everything else is inside the
±0.36 ms noise floor.

#### ⚠ Withdrawn heuristic: exclusive systems do NOT cost "more than they measure"

I claimed the `&mut World` scheduling barrier made these cost far more than their
~18 µs, and wrote that into the source comments. **The data refutes it.** Removing
three per-frame exclusive systems moved `main app` 0.147 ms — inside noise — while
the splash zones fell by precisely their own measured cost. If the barrier were
material, the frame should have moved by more than the systems' own time. It did
not.

Treat an exclusive system as costing roughly what it measures. **Do not prioritise
hunting them expecting outsized wins.** The source comments have been corrected;
this is recorded because it is a heuristic that would otherwise keep being applied.

#### Cinematic disabled on integrated GPUs — DONE

`gate_post_camera` now also requires `!GpuIsIntegrated`. The splash cinematic is a
full-window multi-pass post chain (a volumetric light chamber → spectral/film
shaders) at physical resolution — exactly the fill-rate-bound workload an integrated
adapter is worst at, and the **first thing a user sees**, so a weak machine's opening
impression was a stuttering animation before the editor had even loaded. It is
decorative; the splash UI itself is unaffected.

With the camera inactive the offscreen target keeps its initial clear
(`Color::NONE`), so the backdrop reads **transparent**, not black — untested
visually. If that looks wrong the fix is a solid fill behind the backdrop image,
not re-enabling the camera.

Superseded in part by the Light Chamber rewrite (2026-08-06): the note above that
only the *camera* was gated no longer holds. `native_chamber::manage_chamber` gates
the 3D scene on the same `!GpuIsIntegrated` condition, so on an integrated adapter
the scene is never built at all — no volumetric raymarch, no shadow maps, no
per-frame animation. The old terrain/sky simulation systems this warned about are
gone with the terrain.

#### Remaining splash cost: ~0.206 ms/f, and it is the 1.3b wall again

All that is left is the render-app systems for the splash's `UiMaterial` types
(after the Light Chamber rewrite: `ChamberMaterial`, `PostMaterial`,
`ApertureMaterial`, `HazeMaterial` — four, down from five) that can never draw again
after boot. It is tempting to "scope those registrations to a state that ends" —
but **Bevy has no plugin-removal API**, and `UiMaterialPlugin` adds its systems
with no `SystemSet` and no run condition. So it is the identical wall as 1.3b:
either fork bevy's generic systems into the contract crate (a permanent
maintenance liability, already advised against) or stop using `UiMaterial` for the
splash backdrop, which is a visual rewrite. Neither is worth 0.2 ms.

## Tier 1 — structural

### 1.0 `virtual_scroll` metric guards — DONE (prerequisite for 0.9b)

`crates/renzora_ember/src/virtual_scroll.rs:230, 262`

`virtual_scroll_impl` falls back to building **every** row whenever
`!measured || row_h <= 0 || columns == 0 || viewport_h <= 0` (`:124`), and
`update_virtual_metrics` was re-arming that state in two ways:

1. It wrote `viewport_h = 0` for a list in a collapsed dock tab or closed popup,
   so re-showing the panel built every row for a frame.
2. It cleared `measured` whenever fewer than two distinct rows were visible — so
   a search narrowing to one hit, then backspaced, rebuilt the whole list.

Both now keep the last good measurement instead. A stale stride mis-sizes the
spacers for one frame and self-corrects; rendering every row costs far more.

This is **not picker-specific** — it fixes the same latent re-show burst in the
three existing virtualized panels (hierarchy, asset browser, shape library).

### 0.9b Material picker virtualization — DONE, measured, all predictions held

**Measured (913 frames, picker-typing burst):**

| | before (ladder-ON leg) | after 0.9b |
|---|---|---|
| avg | 16.78 ms | 16.62 ms |
| p95 | 19.04 | 18.05 |
| p99 | 29.48 | **19.42** |
| max | 43.46 | **22.23** |
| frames >25 ms | 14 | **0** |
| `text_system` max | 14.54 | **2.58 ms** |
| text events >5 ms | 9 | **0** |
| total text work | 168.0 ms | **81.0 ms** |
| `measure_text` max | 3.89 | **0.42 ms** |

`rebuild_picker` / `rebuild_one_picker` no longer exist as zones.
`refresh_material_index` costs 0.002 ms/f, `run_keyed_lists` stayed flat at
0.121 ms/f (max 2.31), and the `VirtualMetrics` seed worked — no first-open burst
anywhere in the capture.

**The tail is what matters:** zero frames over 25 ms in 913, against 14 and 19 in
the two prior legs. p99 29.48 → 19.42 means the tail for this interaction class is
*gone*, not shortened.

**This settled the text question by controlled comparison, not inference.**
Correlating the eight largest `text_system` spikes in the ladder-ON capture
against what ran alongside them: the 14.54 ms peak sat next to a 4.58 ms
`rebuild_picker`, and the 8.50 / 8.29 ms spikes next to 7.59 / 5.89 ms rebuilds.
Same text, same fonts, same atlas keys — the only variable removed was ~200 rows
despawning and respawning. **Fewer text entities moved it 5.6×; fewer atlas keys
moved it 0.** The cost is per new text entity, exactly as reframed, and 0.10's
hypothesis is retired for good.

**Caveat bounding the claim:** four of those eight spikes (7.41, 7.17, 7.14,
5.17 ms) sat next to `rebuild_inspector` at 7.7-8.0 ms with the picker idle. This
capture exercised the picker only — `rebuild_inspector` peaked at 0.24 ms here
versus 8-9 ms in earlier legs — so the inspector-driven text burst is **untested
and almost certainly still present**. Part of "zero frames >25 ms" is that
interaction not happening. The picker numbers above are the like-for-like part.
This is now the direct, measured motivation for 1.1.

### The critical path has inverted

`sub app{RenderApp}` **14.03 ms/f** now exceeds `main app` **13.43 ms/f**.

Read with care: `prepare_windows` is 3.22 ms of that and is the vsync surface
wait, not work, so real render cost is ~10.8 ms. With ~30% of frames pinned at the
60 Hz cap, the honest statement is that **main-world work no longer sets frame
time for this interaction** — not that the render thread is saturated.

`camera_driver` 3.51 ms and the light-probe env map still regenerating every frame
at 0.65 ms (items 1.4 / 1.5). Largest remaining main-world system is unchanged in
character: `ui_layout_system` 2.27 ms/f avg, 5.58 ms max.

*Landed as designed. `cargo check --workspace` clean, file clippy-clean.*

**`rebuild_picker` and `rebuild_one_picker` are deleted outright** — there is no
longer a system that rebuilds the popup. The shell (search box + scrolled list) is
built once when the slot is created, and the rows are a `virtual_scroll_versioned`
keyed list registered on the inner `list` node.

**What to look for when measuring** (typing in the picker search box, popup open):

- The `rebuild_picker` zone **should not exist at all** any more.
- Typing should produce no 1-8 ms events; the previous run showed 11 of them,
  one per keystroke, peaking at 8.04 ms.
- Cost moves into `run_keyed_lists`, which should stay flat — only rows whose
  key/hash changed are rebuilt, and only within the scroll window (+4 overscan).
- Watch `text_system` too: with ~200 rows no longer despawn/respawning per
  keystroke, its per-keystroke burst should drop sharply. That is the same root
  the ladder failed to touch, approached from the correct end (fewer text
  entities rather than fewer atlas keys).

**Also removed:** `MatPickerFilter.sig` and `MatPickerPanel.entity`. The dirty
token reads the query text, `MaterialIndex.generation` and `material_path(entity)`
directly, so the three former `sig` producers are gone — including the poke in
`rebuild_material` that forced a repopulate on selection change.

Design as implemented:

- Delete `rebuild_picker` / `rebuild_one_picker`. Move the popup shell into
  `build_slot`, built **once**: panel children become `[search_input,
  scroll_area(list)]`.
- Register `virtual_scroll_versioned` on the **inner `list` node, not the panel**.
  That makes the search box structurally untouchable — `replace_children` only
  ever rewrites rows — and the `i == 0` skip and `if existing.is_empty()` branch
  both disappear as artifacts of the old container choice. Focus and in-progress
  typing survive every filter change because the input is never despawned.
- **Key** = `hash(rel)`. **Content hash** = `hash(abs, is_current)`.
  Deliberately *not* the thumbnail `Handle<Image>` — that rides on the per-row
  `bind_with`, which value-diffs and writes `ImageNode.image` in place. Hashing it
  would make every thumbnail that lands despawn and rebuild its row.
- Delete `MatPickerFilter.sig`; replace with a dirty token
  `hash(filter.text, MaterialIndex.generation, MaterialRef(entity).0)`.
  `virtual_scroll_versioned` folds the scroll window in automatically. Requires
  adding a `generation` counter to `MaterialIndex` (it currently signals change by
  bumping `sig`).
- Seed `VirtualMetrics { viewport_h: 280.0, row_h: 27.0, columns: 1, measured:
  true }` on the list node *after* registering (the caller's insert is queued
  after the wrapper's, so it wins), so the list is windowed from the first frame.
- Empty state must **not** key on `u64::MAX` / `u64::MAX - 1` — those are
  `virtual_scroll`'s spacer keys. Use `hash("\0<no-matches>")`.
  (`renzora_shape_library` uses `u64::MAX` for its empty row; do not copy it.)

### ⚠ Blocker between 1.1 and 0.9b — resolve before doing both

**The picker popup is not root-spawned.** `native_material_ref.rs:429` does
`commands.entity(name_btn).add_children(&[name_text, panel])`, so despite
`position_type: Absolute` + `GlobalZIndex(1000)` it is structurally a descendant
of the inspector section subtree.

That collides with a **confirmed** bug: `register_keyed_list` (`reactive.rs:623`)
does `world.get_resource_mut::<KeyedListRegistry>()`, but `run_keyed_lists`
(`:642`) holds that resource in a `resource_scope` across `queue.apply(world)`
(`:738`). A `keyed_list` registered from inside a row builder therefore finds
`None` and is **discarded with no panic and no log**.

So if the inspector's sections become a `keyed_list` **and** the picker registers
its own `keyed_list` from a drawer built inside that closure, the picker's list
silently never runs. Either item alone is safe; the combination is not.

Resolutions, in preference order (1 and 2 are complementary):

1. **Fix the nesting bug — DONE.** `register_keyed_list` now stages into a
   `PendingKeyedLists` resource that nothing scopes out (`reactive.rs`), drained
   into the real registry at the top of `run_keyed_lists` *before* the
   `resource_scope` that made direct registration unreachable. Nesting works by
   construction; the silent-drop failure mode is gone.

   **No latency change for existing consumers:** if the command flush lands
   before `run_keyed_lists`, the drain picks it up the same frame; if after, it is
   processed next frame — identical to the old direct push. Only the previously
   *impossible* nested case changes behaviour. `cargo check --workspace` clean.
2. **Keep native drawers off the keyed path** — build the section shell in the
   `keyed_list` and fill drawers from a follow-up exclusive system, so nothing
   registers from inside a build closure. (Note `KeyedSnapshot::build` receives
   only `&mut Commands` — never `&World` or `&mut World` — so a drawer needing
   exclusive access cannot run there anyway.)
3. Root-spawn the picker popup, which is arguably correct for a
   `GlobalZIndex(1000)` overlay and matches the established modal convention.

### ⚠ 1.1 was designed, critiqued, and REORDERED — read before implementing

A full design + adversarial critique pass returned **"do not ship as written."**
The important finding is strategic, not a defect list:

**Converting the section list to a keyed list is a net REGRESSION on the most
common inspector interaction.** The key must be `hash(entity, type_id)` — not
`type_id` alone — because `build_section` and `build_field_value` capture
`entity` by value in every `bind_2way` / `LockBtn` / `RemoveBtn` /
`EnableToggleCmd` closure (`native.rs:1334-1371`, `:1601-1783`). A section reused
across a selection change would silently write to the **previously inspected
entity** — data corruption, not a cosmetic bug.

But that means selecting a different entity changes **all** ~21 keys, so
`run_keyed_lists` rebuilds everything *plus* ~21 `DefaultHasher` passes over every
field name/kind/extension and a `HashMap` rebuild. Selection change is the
highest-frequency inspector interaction, and it gets marginally **slower**.

So 1.1b's real deliverable is: the two *rarest* interactions (component-filter
typing, enable-toggle) get much faster, the most common one gets slightly slower,
and the only user-visible win is collapse persistence — which is 1.1a.

**Reordered: 1.1a → 1.1c → 1.1b.**

- **1.1a — DONE.** Remember section collapse state; stop treating list position as
  content. No perf claim — a UX fix that also makes `open` and position safe to
  exclude from a hash in 1.1b. `cargo check --workspace` clean.

  *UX decided (F10):* collapse state persists **per component type** and follows
  the user across selections, but the **Inspector Expand Default setting always
  wins** — changing it clears the memory *and* re-applies to the live sections
  (`apply_expand_policy_change`), so the preference can never be permanently
  shadowed by accumulated toggles. This also pre-empts F2, which would otherwise
  have made that setting inert once 1.1b lands.

  *Bug caught while implementing:* the Essentials policy was matching on display
  names, and the "ID" section's `type_id` is `"name"` — so a naive `type_id` port
  would have silently collapsed it. Both call sites now share one `policy_open()`
  helper keyed on `type_id`, so they cannot drift, and no code matches on a
  display string that could be reworded.

  `stripe_collapsed_headers` now derives the zebra index from the live child order
  of `InspectorRoot` rather than a baked-in `index` field.
- **1.1c** — *lazy collapsed bodies*: don't build the body subtree of a collapsed
  section. This is where the selection-change win actually lives, it is the
  highest-frequency interaction, and **it does not need the keyed list at all** —
  only the `PendingKeyedLists` nesting fix, which already shipped.
- **1.1b** — the keyed-list conversion. Do last, once F1-F4 are resolved.

#### Blocking defects for 1.1b (all fixable, none started)

- **F1 — the lock button becomes a no-op.** `locked` is in the global signature
  but nothing in a per-section hash; clicking lock changes only `state.locked`,
  consumed solely by the name section's glyph (`native.rs:1324-1336`). After
  conversion every `(key, hash)` is identical → `reactive.rs:708` early-out → the
  glyph never updates. Fold `locked_here` into the name section's hash, or
  `bind_with` the glyph.
- **F2 — the Inspector Expand Default setting becomes a no-op.** Clearing the
  remembered-state map changes no hash, so nothing rebuilds. Must drive live
  `Section`s via `set_section_open`, as `expand_all_click` already does
  (`native.rs:2243-2258`).
- **F3 — every `ReadOnly` row freezes forever.** `FieldKind::ReadOnly` spawns a
  bare `Text` with **no binding** (`native.rs:1822-1836`) — the design's "field
  values are all `bind_2way`'d" premise is false for it. It is the catch-all in
  `#[derive(Inspectable)]` (`renzora_macros/src/inspectable.rs:132`), so this is
  common, not a corner. Give `ReadOnly` a `bind_text`.
- **F4 — six drawers are *already* the rejected option (a).** Scripts, Animator,
  Audio Player, Skybox, Rich Text and Camera Presets return an empty marker node
  filled later by their own exclusive system. Their rebuild systems have **no
  ordering constraint** against `rebuild_inspector` *or* `run_keyed_lists`, so
  moving the drawer call into `run_keyed_lists` silently reshuffles an
  unconstrained topological accident. Add explicit ordering or document a
  one-frame fill.
- **F7** duplicate `type_id` in `InspectorRegistry` (a bare `Vec` with no dedup)
  would collide keys and leak a row. **F8** stale-lock cleanup moving out of the
  exclusive system makes "No components" flash for a frame when the locked entity
  is deleted. **F9** the token is *not* cheap — `has_fn` per registry entry plus a
  `Vec<String>` allocation per `DynamicEnum` per frame; unchanged from today, but
  do not describe it as cheap.

#### ⚠ Correction: rebuild layout does NOT land on the following frame

The "rebuild-burst zones under-read their own cost" note earlier in this document
is **contested**. All `bevy_ui` layout/content systems run in **`PostUpdate`**;
`rebuild_inspector` (`native.rs:308`) and `run_keyed_lists` (`reactive.rs:43`)
both run in **`Update`** — so a rebuild should be laid out and text-measured in
the **same frame's** `PostUpdate`.

That contradicts the direct observation that `ui_layout_system` did not run in the
frame where `rebuild_picker` cost 8.04 ms. One of the two is wrong and it is not
yet settled. **Resolve this before quoting any 1.1 number**, because it decides
which frame the profiler should be pointed at.

#### F6 — no recorded baseline

The 8-9 ms `rebuild_inspector` / 7.4 ms `text_system` figures this plan is
anchored to exist **nowhere in the repository** —
`docs/r1-alpha7/editor-dev/profiling.md` has no inspector numbers. Land the
baseline there as step 0 so 1.1's "before" is reproducible by someone else.

### 1.1b Make the inspector section list a keyed list (**"S4"**)

`crates/renzora_inspector/src/native.rs:940` (`collect_sections`), `:780-815` ·
**[est. 5-15 ms → ~0.2 ms per event]**

The inspector has **zero** `keyed_list` call sites against 24 `bind_*`, for a
panel that can build ~4000 entities. Every structural change therefore goes
through full teardown: `rebuild_inspector` recursively despawns every child of
`InspectorRoot` and every component-menu button, then respawns the tree —
~230 entities for a plain Transform+Mesh+Material entity, ~1200 for a camera with
12 post-process effects, ~4000 for one with 40 — inside one exclusive system, in
one frame, followed by a full taffy relayout.

`collect_sections` already returns `Vec<SectionSpec>`, which is a `KeyedSnapshot`
in all but name. Key on `entry.type_id`; hash *that section's own* signature (its
field set, its enabled bit, its own `DynamicEnum` options). Flipping
`Bloom.enabled` then rebuilds the Bloom section, not the panel.

This also makes 3.3 moot rather than needing a separate fix: the enabled bit stays
in a hash, it just stops being a global one.

**This is the item the user can feel.** If only one thing on this page ships,
ship this.

### 1.2 Despawn inactive dock panes — DONE, and it moved the floor

`crates/renzora_ember/src/dock.rs` (`sync_panes`, `TabPane`)

**`main app` 14.105 → 10.560 ms — 3.5 ms, 25%.** The largest single main-world win
of the session, and larger than everything Tier 0 and Tier 1 delivered *combined*.

| | Bistro + panels open | Bistro, nothing selected | Bistro + empty workspace |
|---|---|---|---|
| avg frame | 16.14 ms (62.0 fps) | 13.27 ms (75.4 fps) | **12.67 ms (78.9 fps)** |
| p95 / p99 | 20.31 / 23.28 | 15.22 / 16.90 | 14.62 / 16.29 |
| main app | 14.105 | 11.461 | **10.560** |
| RenderApp | 13.210 | 11.365 | 10.905 |
| `ui_layout_system` | — | 1.192 | 1.062 |

⚠ **Do not diff "empty workspace" against the empty-*scene* capture.** "Empty
workspace" means an empty **dock**, with Bistro still loaded — that capture has
`early deferred prepass` 0.924, `main_opaque_pass_3d` 0.405 and GPU total 2.774,
against 0.957 and no geometry passes for the empty scene. Diffing them credits this
change with the render-side difference between two different scenes. An earlier
version of this entry made exactly that mistake.

**The win came entirely from removing UI *nodes*, not systems** — and that is the
load-bearing finding. Per-crate system counts are unchanged to within sampling
(`renzora_ember` 305.3 → 305.1, `viewport` 91.9 → 91.9, `animation_editor` 57.5 →
57.3), as are whole-frame counts (3266 → 3260 runs, 2967 → 2922 sub-5 µs). The
panels' systems still run; they simply have nothing to query.

Two consequences: hidden **nodes** were the dominant term, not hidden systems — and
**the `PanelScope` sweep is entirely ahead of us, with its ~0.6-0.9 ms additive to
this.** This change did roughly 4-6× what the whole system-gating sweep is
projected to do.

`sync_panes` now despawns every pane that is not its leaf's active tab, instead of
hiding it with `Display::None`. One rule replaces the old two-case split and covers
both cases: other workspaces (their leaves go with the tree swap) and inactive tabs
in the current workspace. `build_active_panels` already rebuilt lazily on
activation, so the cycle was already half-built.

**Why hiding was never enough:** `ui_layout_system` does three *unconditional*
full-tree walks per frame and does **not** skip `Display::None`; taffy's
`compute_hidden_layout` clears the cache and recurses, so hidden subtrees are
re-walked from scratch every frame and never cached. Lazy building only deferred
the accumulation — visit five workspaces and you permanently hold five workspaces
of hidden panes.

**Crash avoided, worth knowing about.** The dock rebuild detaches only *preserved*
panes to the root, because a node detached **and** despawned in the same frame
frees its taffy slotmap key while the old leaf still lists it as a child →
`invalid SlotMap key` panic (a recorded prior bug; see `DockTree::active_tab_ids`).
This change takes `sync_panes` from despawning rarely to despawning on essentially
every multi-tab frame, turning that collision from a corner into a likely case — so
`sync_panes` now stands down entirely while `DockDirty` is armed and lets the
rebuild own pane lifetimes for that frame.

**Tradeoff taken deliberately:** a tab switch now costs a rebuild rather than a
`display` flip, and transient in-panel state (scroll offset, search text, expanded
rows, in-progress typing) is lost with the entities. Dock *layout* still persists
via `layout.json`. State that should survive belongs in a resource keyed by panel
id — the pattern `ScriptSectionsOpen` / `InspectorSectionsOpen` already use.

**Still to verify:** switching to a heavy panel now pays its build cost.
`rebuild_inspector` peaked at 8.20 ms, so the Inspector tab is the most likely to
feel it. If it does, the fix is 1.1's virtualization, not reverting this.

### 1.2 (original entry) Despawn or age out inactive dock panes

`crates/renzora_ember/src/dock.rs` (`sync_panes`)

Panes are built once and kept forever; tab switches only toggle
`Node::display`. So every panel ever opened stays in the ECS and the taffy tree
and is re-walked 3× per frame by `ui_layout_system` plus once by
`ui_stack_system` — none of those walks check dirtiness or skip `Display::None`.

Taffy makes it strictly worse: `compute_hidden_layout` calls `cache_clear` and
recurses into every child, and `Cache::get` returns `None` for
`RunMode::PerformHiddenLayout` — **hidden subtrees are never cached and are
re-walked from scratch on every `compute_layout`.**

Note this is a deliberate design decision today (persistent pane content), so
changing it needs care: see `[[bottom-panel-toggle]]`-style stash requirements —
panel state must survive, or panels are lost.

### 1.3 Run-conditions — surveyed, and most existing "gating" is illusory

**The headline finding.** Of 1625 leaf system registrations, 1535 land in
per-frame schedules and **1283 of those (83.6%) run unconditionally in an idle
editor**. 718 have no `run_if` at all — but the gap to 1283 is the important part:
**753 of 817 "gated" registrations use `in_state(SplashState::Editor)`, which is
always true once the editor is up.** It gates the boot phase, not idleness. Same
for `not_in_play_mode` / `not_editing` / `in_editing_mode`. Only 41 registrations
use `panel_active` and only 15 use `any_with_component`.

So "add run conditions" is not a matter of filling gaps — most of the schedule is
genuinely ungated and was only ever *appearing* gated.

#### 1.3a Post-process framework central gate — DONE

`crates/renzora/src/postprocess.rs:821`

Every per-frame system the ~52 effect cdylibs own comes from **one generic
function**, `PostProcessPlugin<T>::build`. No effect crate adds a render-app
system of its own, and there was not one `run_if` across all 52 crates. Four
systems per effect: `proxy_effect_to_camera::<T>`, `cleanup_proxy_effect::<T>`,
`extract_components::<T>`, `prepare_uniform_components::<T>` — **208 systems/frame
from this cluster alone**, 113 main-world (3.7% of 3079) and 105 render-world
(**12.4% of 845**). A scene uses 1-3 effects, so ~200 of those are pure no-ops.

The module doc's claim that "inactive effects have zero render graph overhead" is
true of the graph *node* (it bails per view) and **false for the four ECS
systems**, which were added unconditionally.

*Landed:* one `.run_if(any_with_component::<T>)` on the two main-world systems —
**gates 104 systems across 52 plugins from a single line.** Verified correct in
both directions; the removal frame is preserved because the condition matches the
*camera's* proxied copy and `try_remove` is deferred.

**Measured (idle → idle):** `proxy_effect` zones went from **208 → 0** (104
`system{}` + 104 `system_commands{}`). Measured directly in the baseline trace,
what was removed cost **0.097 ms/f dispatch + 0.117 ms/f command application =
0.214 ms/f** aggregate across worker threads. System executions 3459 → 3283
(−176); `Update` total 6.097 → 5.623 ms.

The predicted split held — **the command-queue half was the bigger payer**
(0.117 vs 0.097), i.e. the bug mattered more than the dispatch saving. But both
halves are small, which puts 1.3a in the **constant-shaving** category:
**now 0/3 for large wins there**, against 3/3 for work-removal. Still the right
change — a real bug fixed, 208 fewer zone-emitting runs per frame, one line — but
the defensible figure is **~0.2 ms/frame**, not the raw `main app` delta.

**Do NOT credit 1.3a with the −1.175 ms `main app` drop in that capture.** The
only comparable idle baseline was four changes back, and the drop is concentrated
in UI systems neither change touches (`ui_layout_system` −0.412, `text_system`
−0.121, `ui_focus_system` −0.081, `update_clipping_system` −0.075 — −0.69 of the
−0.78 in PostUpdate). The likely cause is **1.1a as a UI-state confound**: per-type
collapse memory meant fewer inspector sections were expanded in that run, so there
was less to lay out. That is not a code improvement. Idle `main app` depends on
what is on screen; reproducing it would require both builds with the same sections
expanded.

**It also fixes a real bug, not just idle dispatch.** `cleanup_proxy_effect`'s
`sources.is_empty()` branch is taken *precisely when the effect is inactive* — so
it was queueing a `try_remove::<T>()` boxed command for **every routed camera,
every frame, forever**, for all ~50 inactive effects, to remove components that
were never there.

### ⚠ The critical path has fully inverted — this re-ranks everything

`main app` **12.45 ms** now sits *below* `sub app{RenderApp}` **13.03 ms**, and
the render side is unchanged (`run_render_schedule` +0.021 ms = noise). Frame
quality is good: p95 17.58, p99 18.07, max 19.16, **zero frames over 20 ms**.

**Consequence: further main-world work is invisible.** Main-world savings only
reach the frame if the render thread comes down with them. Any item below that
operates on the main world should be treated as low value until that changes.

**This reverses the earlier preference for 1.3c over 1.3b.**

- **1.3b (render world) is now the most valuable remaining item.** It is the only
  outstanding change that reduces work on the *new* critical path — 105 render
  systems, **12.4% of the render world's 845** — and it fixes the uniform-buffer
  bug as a side effect.
- **1.3c (built-in wrappers) is now low value**, despite scaling with scene size,
  because it is entirely main-world. Keep it recorded; do not prioritise it.

The two previously-named render leads are both dead ends, already investigated:
`queue_submit` is one submission whose buffer count is a function of render-system
count (batching breaks submission ordering), and pass-count reduction is a quality
decision. **1.3b is the only safe render-side optimisation left on the board.**

#### ⚠ 1.3b RE-EXAMINED — the "~25 lines" estimate was wrong, and both halves are blocked

Checked against the actual `bevy_render-0.19.0` source before starting. The
re-ranking above promoted 1.3b on *value* without re-checking *feasibility*, and
it does not survive contact:

- **The uniform half cannot be copied.** `UniformComponentPlugin::build` is only
  10 lines, but the system it registers — `prepare_uniform_components::<C>` — is
  **private**, and `ComponentUniforms<C>` exposes its buffer through `Deref` and
  `uniforms()` only, both **immutable** (`bevy_render/src/uniform.rs:60-74`). A
  gated copy needs `&mut` to call `get_writer`, so there is no way to write one
  without forking `ComponentUniforms` itself — and then every bind group reading
  `ComponentUniforms<T>` (plus the `DynamicUniformIndex<T>` handler gate) has to
  be repointed at the fork. That is not 25 lines; it is a fork of the uniform
  plumbing. **The estimate covered `ExtractComponentPlugin`, not this.**
- **The extract half is the stale-render-world bug in disguise.** Gating
  `extract_components::<T>` on the main world having any `T` means that when the
  last `T` is removed, extract stops running and the render world **keeps its
  stale `T` forever** — the effect renders permanently after being switched off.
  This is exactly the `ExtractComponentPlugin` never-removes bug already hit and
  recorded for the Firefly config. A safe gate would have to be "main world has
  any `T` **or** render world has any `T`", which costs a cross-world check per
  effect per frame and gives most of the saving back.

**Verdict: 1.3b is not the cheap central win 1.3a was.** The earlier "fork
liability" read was correct; the later promotion was based on value alone. Do not
start it without a measurement showing the render world is still the ceiling and
that this specific cost is a large share of it.

#### Remaining in this cluster (not done)

- **1.3b — Level 2, the render-world half (104 more systems).** See the
  re-examination directly above before acting on this entry. Bevy's
  `ExtractComponentPlugin` / `UniformComponentPlugin` add their systems with no
  `SystemSet` and no condition, so they cannot be gated after the fact — the
  framework would need its own copies (~25 lines lifted from `bevy_render`).
  `prepare_uniform_components::<T>` gates cleanly on render-world
  `any_with_component::<T>`, which would also fix a second upstream bug: bevy keeps
  calling `write_buffer_with` every frame once the buffer is allocated even for a
  zero-component frame, so **every effect the user has ever toggled on keeps paying
  a staging-buffer map per frame after being turned off**.
- **1.3c — the ~14 bevy-builtin wrappers** (vignette, bloom, dof, ssr, ssao,
  motion_blur, tonemapping, volumetric_fog, antialiasing, auto_exposure,
  atmosphere, distance_fog, bluenoise, environment_map). These hand-roll the same
  `sync_X`/`cleanup_X` pair — ~35 ungated `Update` systems the framework fix does
  **not** cover. Worse, their `sync_X` bodies scan `routes × source_list` with **no
  early-out**, and `source_list` is *every named non-UI scene entity*
  (`renzora_viewport/src/effect_routing.rs:99`). That is
  **O(cameras × scene_entities) `Query::get` calls per frame per inactive effect —
  cost that grows with scene size**, which makes it the more interesting of the two
  on a large scene like Bistro. Fix per crate with
  `.run_if(any_with_component::<XSettings>)`, or better, hoist a shared
  `sync_routed_effect::<Settings, Target>` helper into the contract crate so they
  inherit the framework gate too.

### 1.3 (original entry) Run-conditions on idle plugins

Workspace-wide · **[measured: ~4 ms aggregate]**

3064 system runs/frame averaging under 5 µs = ~3.98 ms of dispatch for systems
doing nothing, plus 1298 executor tasks/frame. This is the ~200-plugin dispatch
cost. It will not yield to micro-optimisation — it needs *fewer systems running*,
i.e. run-conditions that keep idle plugins' systems out of the schedule.

### Render thread — profiled and attributed. 1.4 and 1.5 are both closed.

```
14.03  run_render_schedule
 14.02   Render schedule          (self 6.96 = waiting on the parallel prepare/queue batch)
  5.35     render_system -> RenderGraph
   3.51       camera_driver
    2.94         viewport camera (Core3d)
    0.53         second camera
   1.53       submit_pending_command_buffers
    1.51         queue_submit{count=30}
  0.53     apply_extract_commands
```

**Real render cost is ~10.8 ms, not 14.** `prepare_windows` is 3.22 ms of vsync
surface wait sitting inside the parallel batch that feeds the 6.96 ms of schedule
self time. It is not work.

**`camera_driver` is many small nodes, not one hotspot.** The viewport camera's
2.94 ms is 0.758 ms of Core3d dispatch overhead plus ~14 graph nodes between 0.05
and 0.43 ms — shadow 0.428, bloom 0.289, gpu clustering 0.199, deferred prepass
0.134, gpu_preprocess 0.116, ssao 0.105, outline 0.081, atmosphere 0.063,
auto_exposure 0.060. **There is nothing to optimise.** Cutting `camera_driver`
means running *fewer passes*, which is the Graphics Quality tiers / Render Toggles
lever — a quality decision, not a perf fix.

**Encode costs ~2× the work it describes.** 5.35 ms of CPU command encoding
produces 2.63 ms of GPU execution (deferred prepass 0.836, ssao 0.462, main opaque
0.366, ui 0.317, bloom 0.229). **The GPU has been idle in every capture taken.**

#### 1.4 — PARKED (do not implement)

The engine **already gates this**: `gate_environment_generation`
(`renzora_environment_map/src/lib.rs:244`) stashes and removes
`GeneratedEnvironmentMapLight` when the environment is inactive, and its doc
comment already documents the upstream cause — `downsampling_system` /
`filtering_system` are registered unconditionally in `Render`
(`bevy_pbr .../light_probe/generate.rs:139-154`) with no bake-once or dirty mode.
The 0.65 ms is the cost of an **active** environment, which is the normal case.

Extending the gate to detect a *static* environment would be a **0.65 ms ceiling
against a lighting-corruption failure mode**: the atmosphere LUTs evolve over
several frames after a change, so "nothing changed this frame" is not "the map
converged". Stash too eagerly and you freeze a half-converged map — subtly wrong
lighting, hard to notice, harder to attribute. Bad trade. Parked.

*Calibration worth remembering:* `prepare_generated_environment_map_bind_groups`
at 0.654 ms is the second-largest **named** prepare system, behind only the vsync
wait. It looked big because it sat in a list of small things — not because it was
big. Rank against the frame, not against its neighbours.

#### `queue_submit{count=30}`, 1.51 ms — INVESTIGATED, also closed

Initially recorded as "the best remaining render-side item — fewer submissions is
mechanical with no failure mode." **That was wrong on both counts.**

It is **one** submission carrying 30 command buffers, not 30 submissions:
`FlushCommands::flush()` does a single `queue.submit(buffers)`
(`bevy_render .../renderer/render_context.rs:198`). So "fewer submissions" is not
a lever — there is already only one.

The 30 buffers come from `RenderContextState`, which is a **`SystemBuffer`**: its
`queue()` runs at the end of **every render system** that touched the encoder,
finishing it and pushing a buffer (`render_context.rs:105-125`). So
**30 buffers ≈ 30 render systems that encoded something.** The count is a direct
function of render-system count, not a batching setting.

And the flush is deliberate — the code comments it *"flush to ensure correct
submission order."* Batching encoders across systems would break that ordering
guarantee: a correctness hazard, not a free win, exactly like 1.4.

**Conclusion: the isolated structural wins are done.** 1.4, 1.5 and `queue_submit`
all dissolved on inspection — each looked like an optimisation and each turned out
to be either a quality trade or a correctness hazard. What remains with real
aggregate size is the scheduler tax (1.3), and it is the only item where cutting
system count would *also* cut buffer count and encode time.

#### 1.3 also applies to the render world — the plan missed this

**845 distinct prepare/queue/extract systems, 13.92 ms aggregate across worker
threads.** Same shape as the main world's 3079 sub-5 µs systems. Item 1.3's
run-conditions apply on both sides; the plan only accounted for the main world.

### 1.4 (original entry) Stop regenerating the light-probe environment map every frame

Render app · part of **[measured: 12.38 ms render thread]**

### 1.5 Investigate `camera_driver` graph encode

**[measured: 3.54 ms for 2 cameras]**, inside `render_system` 5.60 ms, with
render schedule self 5.02 ms (prepare/queue). The render thread does 12.38 ms of
CPU work to produce 2.74 ms of GPU work.

### 1.6 Persist inspector section expand/collapse state

`native.rs:1299`, `renzora_ember/src/widgets/section.rs:22`

Open state lives only in the `Section` component on header entities that the
rebuild despawns, so `inspector_expand_default` re-applies every time. Move it to
a `HashMap<TypeId, bool>` on `NativeInspectorState`. Matters much less after 1.1,
but it is the difference between "correct" and "correct and invisible".

---

## Tier 2 — reactivity redesign (gated on a measurement)

**Do not start this until `reactions_us` has been read off the UI Reactivity
panel with a real scene loaded.**

The audit shows that 97-99% of binding recomputes are structurally wasted — but
wasted is not the same as expensive. `run_reactions` did **not** appear in the
Tracy capture's top costs, which suggests most bindings are trivial. Decision
rule:

- `reactions_us` under ~200 µs → skip Tier 2 for now, revisit after Tier 0/1.
- `reactions_us` 1-3 ms → the redesign pays for itself.

### Root cause

`ReactionRegistry` knows *binding → target*; nothing knows *data → binding*.
With no declared dependencies, the only sound way to know whether a binding's
output changed is to recompute it, so `let v = value(world)` (`reactive.rs:284`)
is unconditional. The `PartialEq` diff at `:285` is a **write filter, not a work
filter** — it correctly suppresses the component write and the Bevy change tick
(which is what keeps `ui_layout_system` from re-running taffy, so it must be
*kept*), but it runs after the expensive part.

Every mitigation already in the file is a hand-rolled approximation of the
missing edge: `has_hidden_ancestor` approximates "is anyone looking",
`keyed_list_tokened`'s token approximates "did my input change", `virtual_scroll`
windowing approximates "does this row matter". Three ad-hoc dependency systems a
developer must remember to use — and the flagship user gets it wrong: the asset
browser folds `time.elapsed_secs()` into its token, silently turning a version
check into a 1 Hz poll.

### Verified language constraints

Confirmed with compiled Rust spikes, not reasoning:

- `World`'s inherent methods always beat an extension trait → auto-tracking
  **requires** changing the closure parameter type. There is no dual-signature
  bridge (`E0119`).
- An inherent method on a wrapper with the **same name** shadows it, so existing
  closure *bodies* compile verbatim.
- Only **32** call sites workspace-wide annotate `|w: &World|`; ~900 infer it.
- `Rx` must **not** implement `Deref<Target = World>` — fall-through reads record
  nothing and produce silent staleness. A compile error is the correct failure.
- Accessor surface is tiny: 798 `get_resource::<`, 388 `resource::<`,
  630 `get::<`, 0 `get_entity(`.

### Stages

| Stage | Scope | Call-site churn | Risk |
|---|---|---|---|
| **S1** Ownership | Slot map + generational `SlotId`; `BoundSlots` component + `on_remove` hook; `PendingRegistrations` | zero for `bind_*`; 5 `react` sites gain an anchor | low, internal |
| **S2** `Rx` + pull dep-check | `Rx` type, `DepSet`, interned `DepTable`; signature change; conservative fallback | **32** | medium |
| **S3** Push inversion | Subscriber lists on `DepSlot` + dirty queue → steady state O(U+D), N drops out | zero | low once S2 lands |
| **S4** Inspector sectioning | see 1.1 — **independent of S1-S3** | ~24 sites | medium |
| **S5** Parallel `poll` | two-phase `Reaction` trait + `ComputeTaskPool::scope` | zero | likely not worth it |

**The safety property that makes this shippable:** a binding with an empty dep
set is treated as *always dirty*. Tracking can only remove work; it can never
introduce staleness. A half-migrated codebase is exactly as correct as today's.

**S3 is not optional.** The pull version of S2 is only a partial win — for a
binding whose compute is one field read, checking its dep costs about what
running it costs. The subscriber-list inversion is what removes the per-binding
check and makes the win uniform across all 272 `bind_display` sites.

### S1 fixes two bugs for free

Deterministic drop at despawn retires 3.1 (nested-list silent drop, via a
`PendingRegistrations` resource that is never scoped out) and 3.5 (`react()`'s
`target: None`, via a real owner scope). It also deletes the `retain_mut`
compaction and the N per-frame `get_entity` liveness probes.

### Best auto-reactivity target: markup

`update_text_bindings` (`markup/binding.rs:61`), `update_show_bindings` (`:613`),
`value_fill_system` (`markup/widgets.rs:211`), `vector_dial_sync` /
`vector_series_sync` (`markup/vector.rs:527/598`) and `update_foreach`
(`markup/foreach.rs:46`) sit outside the registry entirely — no hidden-pane gate,
no cache, no deps, and invisible to `ReactiveStats` (so a slow markup canvas
reads as zero in the profiler panel). But markup's `read_path` already resolves
`{{ Resource.field }}` from the source text: **the dependency is literally
written in the template.** Folding these into the registry gives an exact
dependency set with zero developer involvement. This is the strongest
proof-of-concept in the codebase.

---

## Tier 3 — correctness (fix regardless of perf) — COMPLETE

All five resolved. Unlike the perf tiers, none of these were affected by the
critical-path inversion — a wrong render is wrong whichever thread is the wall.

| # | Resolution |
|---|---|
| 3.1 | **Fixed** by S1 (`PendingKeyedLists`) |
| 3.2 | **Fixed** — new `style::StyleOwnsPadding` opt-out |
| 3.3 | **Fixed** — module doc corrected, with the two real exceptions named |
| 3.4 | **Documented, deliberately not fixed** — the cure costs more than the bug |
| 3.5 | **Fixed** — new `reactive::react_anchored`, opt-in |

Two judgement calls worth recording:

- **3.5 is opt-in, not a signature change.** Changing `react()` itself would have
  silently anchored `renzora_export`'s and `renzora_splash`'s reactions, which do
  work that must keep running while their panel is backgrounded — pausing those
  would be a worse bug than the one being fixed. Only the two widget call sites
  (`bind_text_input`, `bind_hsv_picker`) were converted.
- **3.4 was closed by documenting rather than fixing.** Folding `get_fn`'s
  Some/None into the signature means calling it for every field of every present
  component *every frame, before the early-out*, to guard against something only
  three `get_fn`s in the workspace can even express. Per-section hashing would
  make it nearly free, so it belongs with that work.

### 3.1 Nested `keyed_list` is silently discarded — CONFIRMED

`reactive.rs:623` vs `:642, :738`

`register_keyed_list` does `if let Some(mut reg) =
world.get_resource_mut::<KeyedListRegistry>()`, but `run_keyed_lists` has taken
that resource out via `resource_scope` before applying the row-build queue. A
`keyed_list` registered from inside a row builder is dropped with **no panic, no
warning, no log**. Fixed structurally by S1.

### 3.2 `apply_theme` clobbers binding writes permanently — FIXED

`style.rs:684-706` (bare `Update`, unordered) + `reactive.rs:285`

`apply_theme` unconditionally writes `node.padding` for every `Changed<Styled>`
entity. Where a binding also writes padding — `icon_label_button_collapsing` is a
live instance — the loss is **permanent, not a one-frame flicker**, because the
binding diffs its *source* value, still matches, and returns `Unchanged` forever.
Sibling instances at `markup/vector.rs:560-561`,
`renzora_export/src/native.rs:776-777`.

The structural fix is to record the *target* component's change tick at apply
time and re-apply when it moved and it wasn't us — which retires the whole
"last writer wins permanently" class.

### 3.3 Module doc claim is false — FIXED

`native.rs:8-11` vs `:915-917`

The doc says field-value edits don't trigger a rebuild. `inspector_signature`
folds in `is_enabled_fn`, and **37 of 39 implementations read a component field**
(`s.enabled`, `l.active`, `v.debug_draw`, …). So enable toggles *do* rebuild the
whole inspector. Ordinary scalar/Vec3/colour edits genuinely don't (verified:
`record_field_change` → `FieldChangeCmd` only calls `set_fn`). Either fix the doc
or let 1.1 make it irrelevant.

### 3.4 Row visibility isn't in the signature — DOCUMENTED, deliberately not fixed

`native.rs:1039` vs `:883-938`

Row existence depends on `get_fn` returning `None`, which is not hashed
anywhere → stale rows are possible. Near-unreachable with today's three
conditional `get_fn`s, but it is a real gap. Fold per-field `get_fn(..).is_some()`
into the per-section hash from 1.1.

### 3.5 `react()` bypasses the hidden-pane skip — FIXED

`reactive.rs:314, 460`

Raw reactions store `target: None`, so the hidden-pane skip never applies —
`bind_text_input` (`widgets/text_input.rs:256`) and `bind_hsv_picker`
(`widgets/colorpicker/mod.rs:265`) recompute every frame for **hidden** panes,
cloning 2-3 `String`s each, and render as `"(world)"` in the debug panel. Fixed
structurally by S1's owner scope.

---

## Rejected — verified not worth doing

| Item | Why not |
|---|---|
| Dirty-flagging UI layout | Taffy already caches per node and dirty propagates up-only, so siblings keep valid caches and the incremental recompute is cheap. Stopping all dirtying would leave the 2.70 ms largely intact. **The cost is node count × per-node walk, not dirtying.** |
| `bind_2way` per-frame clone | `T` is only ever `f32`/`usize`/`bool` across all 133 sites — a register-width copy. Cosmetic. |
| Non-exclusive reactive drivers | `react()` and `bind_2way`'s `set` genuinely need `&mut World`; a read-only variant still needs whole-`&World`, which the scheduler serialises anyway. Trades a barrier for a barrier across a 936-site rewrite. |
| Field-level change tracking | Bevy's change detection is per-component/per-resource. Field granularity would need `#[derive(Signal)]` wrappers on every resource — a data-model rewrite. **Therefore keep the value-diff; it is the correct second gate.** |
| Eliminating false-positive dirties | Bevy marks changed on `DerefMut`, not on actual mutation. Any `ResMut<T>` touched unconditionally wakes its subscribers. Nothing in the UI layer can fix this; best mitigation is a diagnostic ("dep X woke 84 bindings, 0 produced a new value"). |

---

## Sequencing

- **0.1 – 0.8 are all independent.** 0.1 and 0.2 target the two biggest measured
  items; do those first.
- **1.1 (S4) is independent of Tier 2.** Ship it even if the `Rx` redesign never
  happens.
- **S2 requires S1; S3 requires S2.** 3.1 and 3.5 fall out of S1 for free.
- **0.1 is the only ABI-moving change.** Everything else is a normal incremental
  build.
- After each tier: re-capture 12 s with `cargo renzora profile` and diff against
  the 17.90 ms baseline. Do not trust the **[est.]** numbers without this.

## Verification

Per `CLAUDE.md` §2, `cargo check` to iterate but verify with `renzora check` /
`renzora test` in the container before considering anything done. Docs updates
under `docs/r1-alpha7/` are part of "done" for any user-visible change.
