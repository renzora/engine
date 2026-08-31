# Profiling with Tracy

Renzora ships a **Tracy profiler bridge** (`plugins/tracy`) — a standalone C-ABI
plugin that streams live engine telemetry to a running
[Tracy](https://github.com/wolfpld/tracy) profiler over its native protocol:

- a **frame mark** per app frame, and
- every Bevy diagnostic as a named Tracy plot — frame time, FPS, entity count,
  per-render-pass GPU/CPU span times, and system CPU/memory where the platform
  supports it.

> **This plugin gives you plots, never a flame graph.** Per-system CPU zones and
> GPU-pass zones are `#[cfg(feature = "trace")]` inside `bevy_ecs` and
> `#[cfg(feature = "tracing-tracy")]` inside `bevy_render` — instrumentation that
> was compiled out does not exist to be switched on, so no plugin loaded at run
> time can produce it. With only this bridge running, Tracy's **Flame Graph and
> zone Statistics windows stay empty** and `tracy-csvexport -e`/`-g` return
> nothing; all the signal is in the plot rows. For the flame graph, use the
> **profiling build** below, which compiles the instrumentation in.

## The profiling build — full flame graph + per-system Statistics

The bridge's *plots* tell you *how much* (a pass cost 2 ms, FPS is 52) but not
*what ran when*. For the per-system timeline across all ~200 plugins, and the
GPU-pass zones, build with the `profiling` feature:

```
cargo renzora profile        # native: build + stage + launch, trace_tracy compiled in
```

This is just `cargo renzora run` with `--features profiling`, which turns on:

- **`bevy/trace`** — the per-system and render-node CPU spans. Bevy gates system
  zones behind this feature, so *without it the 200-plugin breakdown is invisible*
  even with Tracy connected.
- **`bevy/trace_tracy`** — installs Tracy's tracing layer (CPU zones + one frame
  mark per frame) and the GPU-timestamp zones in `bevy_render`.
- **`renzora_runtime/profiling`** — adds `RenderDiagnosticsPlugin`, which is what
  actually allocates the Tracy GPU context so the GPU-pass zones record. (GPU
  zones work on Dx12/Vulkan; macOS/Metal is excluded upstream.)

Start a Tracy **server first**, then `cargo renzora profile` — the on-demand
client only buffers once a profiler connects, so launch order doesn't lose data.

> **Leave the in-app "Tracy Profiler" toggle OFF in a profiling build.** Bevy
> already emits one frame mark per frame; the `plugins/tracy` bridge marking too
> would double-count every frame and halve the reported frame time.

**`cargo renzora` and `cargo renzora profile` both launch with `RENZORA_NO_XR=1`.**
Editing in a headset is `cargo renzora xr`.

This used to apply to the profiling lane only, which produced a memorable
symptom: the *instrumented* build ran faster than the normal one.

If an OpenXR runtime is installed and set as the system default — not connected,
not in use, merely present — the editor otherwise takes the XR-capable boot,
which disables `PipelinedRenderingPlugin`. The render sub-app then runs inline on
the main thread instead of in parallel with the sim. That showed up as
`sub app{name=RenderApp}` nested under `update` and costing ~11.6 ms of a 27 ms
frame: a serialization that swamps whatever you were actually trying to measure,
and which every ordinary editor session was silently paying.

Add `--xr` when the headset path *is* the subject:

```
cargo renzora profile --xr    # profile the XR-capable (non-pipelined) boot
```

A `RENZORA_NO_XR` already set in your environment always wins; the flag is
consumed by the xtask and never forwarded to the binary.

> **It moves the plugin ABI.** Compiling `trace_tracy` recompiles `bevy_dylib`
> (CLAUDE.md §3), so prebuilt community plugins in `plugins/` won't load against a
> profiling binary. Everything built from source in the same invocation — the
> editor bundle and every workspace plugin — still matches, so the editor and its
> built-in features are unaffected. It's a disposable build; don't ship it.

## Reading a capture

- **Frame time graph** (top ruler): each frame is one mark. A tall frame is a
  hitch; click it to zoom the timeline to that frame.
- **Flame graph** (the main timeline): nested CPU zones per frame — the call/scope
  tree of which system ran when, and for how long. A wide bar = an expensive
  system. There's a separate **GPU** track below the CPU threads with the render
  passes (`main_opaque_pass_3d`, shadow passes, `ssao`, `atmosphere_luts`,
  `bloom`, …).
- **Statistics window** (top bar → *Statistics*): every zone aggregated, sortable
  by total or self time. **This is the "which of my systems is the bottleneck"
  view** — sort by total time and the worst offenders rise to the top. Works for
  GPU zones too (switch the source), giving a per-pass GPU cost ranking.
- **Find Zone** (top bar → *Find Zone*): a per-zone histogram + call-site — use it
  once Statistics has named a suspect, to see its distribution and where it's
  invoked.
- **`prepare_windows` eating most of a frame on the CPU side is the tell for
  GPU-bound** — the CPU is blocked waiting on the GPU to present. In that case the
  answer is in the GPU track / GPU Statistics, not the CPU systems.

### Exporting for offline analysis

With the profiling build, zones exist, so the Tracy **CLI** tools can dump them to
CSV (grab the tools from the *same* Tracy release as your GUI):

```
tracy-capture -o out.tracy -s 8 -f          # capture 8s from the running editor
tracy-csvexport -e out.tracy > cpu.csv      # CPU zones: name,…,total_ns,self,counts,mean…
tracy-csvexport -g out.tracy > gpu.csv      # GPU zones: name,…,GPU execution time
```

Aggregate `cpu.csv` by `name` for total self-time per system, and `gpu.csv` for
ms/frame per pass (divide summed GPU time by frame count).

## Enabling it

One switch: **Settings → Plugins → Tracy Profiler → Enable Tracy**. It takes
effect immediately — no restart. The opt-in persists in
`~/.config/renzora/tracy.json` (`%APPDATA%\renzora\tracy.json` on Windows).

> Earlier versions also required Dev Mode and a restart. Both were consequences
> of the bridge living inside the editor binary: it had to decide at startup
> whether to install its diagnostic sources, so the switch could only be read
> once. As a plugin it installs nothing — it reads measurements the host already
> publishes — so enabling is just "start feeding".

**Turning it off** stops the feeding immediately: no plots, no frame marks. It
does **not** close the Tracy listener socket, because `tracy-client` has no
shutdown outside its `manual-lifetime` feature. That costs an idle socket and
nothing more — the client is built with `ondemand`, so it buffers no trace data
until a profiler actually connects. To get the socket back too, restart.

**To remove Tracy entirely**, delete `plugins/tracy.dll` from beside the
executable. There is no profiler code in the engine itself.

### Choosing what to plot

Under the master switch is a **Plots** list — one toggle per group, applied on
the next frame:

| Group | Covers | Default |
|---|---|---|
| Frame | `fps`, `frame_time`, `frame_count` | on |
| Entity count | `entity_count` | on |
| CPU & memory | `system/*`, `process/*` | on |
| Render passes — GPU time | `render/*/elapsed_gpu` | on |
| Render passes — CPU time | `render/*/elapsed_cpu` | on |
| Shader & pipeline counters | `*_invocations`, `*_primitives_out` | **off** |
| Other diagnostics | anything else, incl. `ui/*` reactivity | on |

Grouped rather than one toggle per plot because the render paths are per-pass and
open-ended — a heavier scene has more of them, and a checklist that grows as you
load a level is not a settings panel.

Invocation counts default off because they are the one group that is usually
noise: raw counters in the millions, which Tracy autoscales, crowding the
millisecond timings that a frame-budget question actually turns on. Turn them on
when the question is "how much geometry is this pass actually touching".

*Other diagnostics* is the catch-all and defaults on deliberately. The host's
diagnostic set is open — any engine crate or plugin can register a path — and
silently dropping unrecognised ones would look exactly like the engine having
stopped measuring.

#### Reading the `ui/*` rows

The reactive UI publishes seven diagnostics, and together they answer "why did
the editor get slower when I selected something":

Eleven `ui/*` diagnostics stream, and between them they answer "why did the
editor get slower when I selected something".

**The bevy_ui pipeline** — where the cost usually is:

| Path | Meaning |
|---|---|
| `ui/content_ms` | `Prepare` → `Content`: propagation + text measurement |
| `ui/layout_ms` | `Content` → `Layout`: taffy solving the tree |
| `ui/nodes_total` | Live `Node` count |
| `ui/text_nodes` | Live `Node` + `Text` count — what measurement scales with |

**The reactive layer** — where it usually *isn't*:

| Path | Meaning |
|---|---|
| `ui/bindings_total` | Bindings walked this frame (excludes parked) |
| `ui/bindings_parked` | Bindings behind a collapsed section — free |
| `ui/bindings_skipped` | Skipped by the dependency gate without running |
| `ui/bindings_changed` | Produced a new value, i.e. a UI write happened |
| `ui/reactions_us` | Binding recompute time, µs |
| `ui/lists_us` | Keyed-list snapshot + diff time, µs |
| `ui/rows_rebuilt` | List rows built or rebuilt |

**Check the reactive rows first, to rule them out.** Opening inspector sections
looks exactly like a reactivity problem and has twice been diagnosed as one; both
times it was not. The tell is `bindings_changed` sitting near zero while the
frame time climbs — nothing is recomputing, the rows simply exist. Note the units
differ deliberately: the reactive figures are **µs** and the pipeline ones are
**ms**, because that is the ratio between them.

Then read `content_ms` against `layout_ms`, because the two point to opposite
fixes. **Content-bound** means text measurement dominates — fewer or cheaper
labels per row. **Taffy-bound** means the tree does — fewer nodes per row.
`nodes_total` is the number both scale with, and watching it jump on selection
tells you what a row really costs: an inspector that adds ~1,000 nodes for ~120
bindings is paying for the nodes, not the bindings.

#### What "off" does to a row already on screen

Turning a group off stops it feeding on the next frame. **A row already on
Tracy's timeline stays there, showing a frozen line**, and that is a limit of
Tracy rather than a shortcut here: its protocol carries `PlotDataInt`,
`PlotDataFloat`, `PlotDataDouble`, `PlotConfig` and `PlotName`, and nothing that
removes a plot. The data model is append-only — once a name has been emitted in a
capture, the server keeps its row for the rest of that capture.

Two ways to get the clean feed:

- **Set the toggles before connecting.** A group that is off when the profiler
  attaches is never emitted, so no row is ever created. The plugin checks the
  toggle *before* it creates the plot name, which is the only moment the decision
  can still be made.
- **Reconnect.** The on-demand client discards plot data while nothing is
  attached, and replays only GPU contexts, lock names and thread names on
  connect — never plots. So the next connection shows exactly the groups that are
  enabled at that moment.

## Capturing

Enable the toggle, then start a Tracy server (the desktop `Tracy.exe`, or the
headless `tracy-capture` CLI). The editor connects and the timeline fills with
frame marks and plots. Because the plugin is Editor-scoped, it profiles the
editor — including gameplay running in the viewport's play mode.

## How it's wired (for plugin authors)

`plugins/tracy` is a standalone C-ABI plugin — it does not link Bevy, and its
only dependencies are `renzora_plugin` and `tracy-client`. It

- reads the host's measurements through the `Diagnostics` system param
  (`SystemCall::diagnostics`, ABI MINOR 4.8), which hands a system this frame's
  `DiagnosticsStore` as `(path, value, smoothed)` triples,
- registers its *Tracy Profiler* section with `App::add_settings_section`, whose
  `EmberToggle` reports through `PanelActionId`,
- persists its own opt-in, and
- creates nothing at all while off — no client, no socket, no plot names.

This is the reference example for reading diagnostics from a plugin: an FPS
overlay, a perf HUD or a telemetry uploader all want the same param.

```rust
use renzora_plugin::diagnostics::Diagnostics;

fn report(diags: Diagnostics) {
    if let Some(fps) = diags.get("fps") {
        info(&format!("{:.0} fps", fps.smoothed));
    }
}
```

Two things the host does not promise. **Which measurements exist** — an editor
carries all of them, a shipped game usually carries none, and a backend without
GPU timestamp queries has `render/*/elapsed_cpu` but not `elapsed_gpu`; `get`
returns `Option` for that reason. **That a present measurement has a value** — a
diagnostic registers before its first sample, so check `Diagnostic::is_valid()`
rather than plotting a `NaN`.

Nothing about Tracy is hardcoded into the editor or the contract.

## The UI Layout panel — bevy_ui cost without a Tracy build

Tracy answers "which system", but standing up a profiling build to ask one
question about the editor's own UI is a slow loop. The **UI Layout** panel
(Debug → UI Layout) answers the specific question that kept coming up, live and
with no rebuild: *where is bevy_ui's per-frame cost going?*

It brackets the UI pipeline with three timestamps around the public system sets:

```text
  A ── UiSystems::Prepare ‥ Propagate ‥ Content ── B ── Layout ── C
       └──────── content (text measurement) ──────┘   └─ taffy ─┘
```

and reports the two halves separately, plus a node census (total / hidden / text
/ visible text). The split is the actionable part, because the two halves have
opposite fixes: **content-bound** means fewer or cheaper labels, **taffy-bound**
means fewer nodes. The census refreshes only while the tab is open, and then only
every 30 frames — a panel about frame time should not cost frame time.

Read it against **UI Reactivity**'s `ms/frame recompute`. Whichever is larger is
the one worth optimising, and the answer is usually not the one you expect: the
measured split was **0.23 ms of reactivity against 5.48 ms of UI layout**.

> The stats resource is written *without* `bypass_change_detection`, deliberately
> — see [Reactivity](reactivity.md#bypass_change_detection-is-now-a-staleness-bug).

## Standing findings — don't undo these

These results are easy to reverse by accident, because in each case the
*expensive* option looks like the harmless default.

**Watch the filesystem; never poll it from a system.** `host::dev` used to
re-walk every plugin crate's source tree with a recursive `read_dir` every
0.25 s, diffing a map of `(mtime, len)` stamps. Measured on a splash screen with
no project open:

| | polling | `notify` watcher |
|---|---|---|
| `poll_plugin_sources` | 1278.0 µs/frame | **1.8 µs/frame** |
| its max | 31.33 ms | **0.10 ms** |
| frames in the 20.5-24 ms lump | 4.3% | **0.12%** |

351 walks at ~19 ms each across a 96 s capture, none of which found anything,
in every editor session — the install is gated on `is_editor`, not Dev Mode.
`notify` and `notify-debouncer-full` are already in the tree via bevy's
`file_watcher`, so this costs no new dependency; in `renzora_plugin` the dep is
gated behind the `host` feature so a plugin author still resolves to zero
dependencies.

Two details that are easy to get wrong and were both hit here. **Watch `src/`
and `Cargo.toml`, not the crate directory** — each plugin declares its own
`[workspace]` and therefore has its own `target/`, so a recursive watch makes
every rebuild flood the queue with its own build output. Filtering those paths
after delivery is not enough; an overflow drops real events alongside the noise.
And **an overflow must not trigger a rebuild-everything fallback** — a `git
checkout` would then rebuild all 63 plugins, which is the worst possible response
to the moment you least want one.

The remaining poll, `loader::poll_plugin_dir`, is deliberate: it stats one flat
directory of built libraries rather than a tree, and measures 15.6 µs/frame.

**Editor chrome is excluded with a query filter, never a per-entity check.** The
editor's own `bevy_ui` nodes live in the same `World` as the scene — roughly 1500
of them, ~950 of those named, on a completely empty project. Any system that means
"the scene" must therefore say so, and it must say so as a `With`/`Without` filter
so Bevy resolves it **once per archetype**. The rule is: an entity is scene content
unless it is a `bevy_ui` node, and a `bevy_ui` node is scene content only if it is
authored game UI (`UiCanvas`/`UiWidget`). `renzora_hierarchy::state::HierarchyCandidate`
is the canonical spelling.

Two places got this wrong and are worth understanding, because the mistake is
natural:

- `build_entity_tree` looped every archetype, then called `world.get::<Name>(entity)`
  on **every entity in the world** to find the named ones, then made three or four
  more random-access lookups per named entity to discard the chrome. All of that
  information is in the archetype; it was being re-derived per entity, up to 10x/sec,
  in an exclusive system.
- `ScriptComponent` was auto-inserted on every named entity, which meant every named
  UI node. Each insert is a deferred **archetype move** — the entity's whole component
  set is copied to a new table — and chrome respawns in bursts, so one panel rebuild
  became hundreds of archetype moves in a single frame. It also doubled the archetype
  count for UI, since each UI component-set then existed both with and without it.

**Behaviour contract this changes:** *nothing* receives a `ScriptComponent`
automatically any more. The auto-insert observer was first narrowed to skip `bevy_ui`
nodes and has since been removed outright — an empty component on every named entity
still cost an archetype move each and left the executor's `&ScriptComponent` query
walking entities with no scripts on them. Every path that needs the component now
creates it on demand: the inspector's **Scripts** entry, dropping a script or
blueprint file onto an entity, the hierarchy's New Asset menu, saving a blueprint
graph, and `renzora_ember::game_ui`, which still inserts one on `UiWidget`/`UiCanvas`
so `<input bind="Entity.var">` resolves. If you spawn an entity outside those paths
and need script variables on it, insert the component yourself.

**The inspector's Scripts section is nonetheless shown on every entity, and that is
not a regression of the above.** Its `has_fn` is unconditionally `true`, so the
section is inherent to the entity the way 2D Lighting is inherent to a `Camera2d` —
but the section is drawn over an *absent* component, and the add-bar inserts one only
when a script actually lands (removing the last script removes it again). Attaching a
script is among the most common things anyone does in the editor, and routing it
through Add Component → search → drop was pure ceremony in front of the action; the
fix for that is UI-side, and materialising an empty component on every entity to get
it would give back every cost listed above **plus** a `scripts: []` entry serialised
into every saved scene, since the component is registered for reflection. If you ever
want the component genuinely universal, the prerequisite is a marker component that
the executor, the hot-reload pass and the hierarchy's badge change-detection filter
on, so the empty ones sit in archetypes those queries never visit.

**Two costs that scaled with `ScriptComponent` count are also gone.**
`check_script_hot_reload` iterated `&mut ScriptComponent` every 0.5 s, and
`Mut::deref_mut` sets the change tick whether or not the write changes anything — so
every component in the scene was marked `Changed` twice a second. `renzora_hierarchy`'s
`AssetBadgeChanges` watches `Changed<ScriptComponent>` for its script badge, so that
storm set `HierarchyDirty` and forced `build_entity_tree` — a full-world scan on an
exclusive system — to run at 2 Hz forever with nothing changed. It now reads through
the immutable `Deref` and bails before touching `DerefMut` unless something is
genuinely stale. Separately, `scripts_should_run` scanned every `ScriptComponent` to
answer "is any script previewing?", and Bevy evaluates a run condition per system
rather than sharing the result — four systems gate on it, so that was four scans a
frame. The answer now lands in a `ScriptsActive` resource computed once in
`ScriptingSet::PreScript`, and the in-play case returns from `PlayModeState` without
iterating anything.

**`ghost_nodes` is deliberately off.** It's a `bevy_ui` feature that swaps
`UiChildren` for a much slower implementation on the editor's hottest path.
`update_children_recursively` calls `is_changed()` once per UI node per frame, and
it short-circuits only when a node's children actually changed — so in steady state
every node falls through to `iter_ghost_nodes()`, which returns a
`Box<dyn Iterator>`: **one heap allocation per UI node per frame**. With ~5k UI
entities that is the dominant term in `ui_layout_system`. `GhostNode` is used
exactly zero times in the workspace, so we pay it for nothing. Re-enabling it is an
[ABI bump](../extending/plugins.md) *and* a performance regression — only do it if
something genuinely needs `GhostNode`, and measure afterwards.

**`bevy_ui` has no hidden-subtree skip.** It walks the full UI tree three times a
frame unconditionally; `Display::None` does not prune it, and
`compute_hidden_layout` clears the cache and recurses, so hidden subtrees are never
even cached. This is why the dock despawns backgrounded panels rather than hiding
them (see [Panels](panels.md)) — hiding a panel does not make it free, and nothing
in the layout stage will make it free later.

**The inspector culls its off-screen sections, and must keep doing so.** Because
`bevy_ui` never prunes hidden subtrees (above), an open component section that has
scrolled out of the panel still charges a full tree walk every frame. The
inspector therefore throws a section's rows away once its body leaves the viewport
by more than half a screen, and rebuilds them when it scrolls back — the same
fill/unfill machinery collapsing a section already used, applied on a second axis.
Measured on one entity with its components open:

| | before | after |
|---|---|---|
| ms/frame UI layout | 5.48 | **3.36** |
| Layout (taffy) | 4.05 | **2.34** |
| Content (text measure) | 1.43 | **1.01** |
| Nodes total | 2814 | 2433 |

Two invariants hold it together, and both are "don't" rules that fail silently:
a section is **never culled before its height has been measured** (the reserved
height is what stops the list collapsing and the scroll range shifting under the
user), and a body is **never measured while it is not holding its rows** (that
records its padding as the section's height and reserves that forever). Both are
covered by `cull_tests` in `renzora_inspector::native`.

Note that this is *not* built on [`virtual_scroll`](widgets.md), which every other
editor list uses. That windows a `keyed_list` by measuring one row stride and
assuming every item shares it — exact for the asset grid and the hierarchy, and
wrong for the inspector, where a collapsed section is one header and an open one
with a native drawer is hundreds of px. Measuring each section's own height
sidesteps the assumption instead of fighting it.

A note on where to spend effort: across this pass, removing *discrete work*
(a system, a rebuild, a subtree) predicted its measured win 5 times out of 5, while
shaving *per-unit constants* on work that still ran predicted it 0 times out of 3.
If a change doesn't remove something from the frame entirely, be sceptical of the
estimate until Tracy confirms it.
