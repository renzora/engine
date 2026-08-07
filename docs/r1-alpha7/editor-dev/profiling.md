# Profiling with Tracy

Renzora ships a **Tracy profiler bridge** (`renzora_tracy`) — a standalone
distribution plugin that streams live engine telemetry to a running
[Tracy](https://github.com/wolfpld/tracy) profiler over its native protocol:

- a **frame mark** per app frame, and
- every Bevy diagnostic as a named Tracy plot — frame time, FPS, entity count,
  per-render-pass GPU/CPU span times, and system CPU/memory where the platform
  supports it.

> **Per-system CPU zones** (the detailed timeline of which ECS system ran when)
> and **GPU-pass zones** come from Bevy's `trace_tracy` feature, which is **not**
> in the normal build — it has no runtime off-switch and would arm Tracy at every
> launch. The bridge above gives frame marks + plots with no such cost. When you
> need the full flame graph, use the **profiling build** below — it re-adds the
> instrumentation for a throwaway binary.

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
> already emits one frame mark per frame; the `renzora_tracy` bridge marking too
> would double-count every frame and halve the reported frame time.

`cargo renzora profile` launches with **`RENZORA_NO_XR=1`**. If an OpenXR runtime
is installed and set as the system default, the editor otherwise takes the
XR-capable boot, which disables `PipelinedRenderingPlugin` — the render sub-app
then runs inline on the main thread instead of in parallel with the sim. That
showed up as `sub app{name=RenderApp}` nested under `update` and costing ~11.6 ms
of a 27 ms frame: a serialization that swamps whatever you were actually trying to
measure. Add `--xr` when the headset path *is* the subject:

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

Tracy is **gated behind two switches**, because activating it both connects the
Tracy client (a network listener + capture ring buffers) and turns on Bevy's
per-frame system-stat sampling — all of which cost real RAM/CPU. It stays
completely dormant unless **both** are on:

1. **Dev Mode** — Settings → Editor → Developer → *Dev Mode*.
2. **Tracy Profiler** — Settings → Plugins → *Tracy Profiler* → *Enable Tracy*.

The gate is **read once at startup**, so changing either switch takes effect the
next time you launch the editor. Both persist across runs (Dev Mode in
`~/.renzora/editor.toml`; the Tracy opt-in in `~/.config/renzora/tracy.json`,
or `%APPDATA%\renzora\tracy.json` on Windows).

> **Leave Tracy off when you're not profiling.** When dormant the plugin adds
> *nothing* — no client, no diagnostic sampling, no per-frame work, so it has a
> zero memory footprint. Only when both switches are on (and after a restart)
> does it stand up the client and the system-stat diagnostics that consume RAM.

## Capturing

Enable the two switches above and restart the editor, then start a Tracy server
(the desktop `Tracy.exe` profiler, or the headless `tracy-capture` CLI). The
editor connects and the timeline fills with frame marks and plots. Because the
bridge is Editor-scoped, it profiles the editor — including gameplay running in
the viewport's play mode.

## How it's wired (for plugin authors)

`renzora_tracy` is a self-contained distribution plugin: it depends only on
`bevy`, the `renzora` contract, `renzora_ember` (its settings toggle), and
`renzora_ui` (the "applies on restart" toast). It

- reads the host's dev-mode flag via `renzora::load_dev_mode()` — a persisted
  accessor on the shared contract, so the plugin needn't link the editor's
  `EditorSettings` type,
- registers its own *Tracy Profiler* category with `register_settings_section`,
- persists its opt-in to the user's config dir itself, and
- gates the bridge at startup on `dev_mode && opt-in`, adding *nothing* (not even
  the diagnostic sources) when off.

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

Two results from the r1-alpha7 profiling pass are easy to reverse by accident,
because in both cases the *expensive* setting looks like the harmless default.

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
