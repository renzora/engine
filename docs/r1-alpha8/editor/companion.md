# The Companion

The editor starts a second, smaller process beside itself: the **companion**. It is where you profile your game, and where work that does not need to happen inside the editor runs, so the editor's frames go to the game you are making.

## Opening it

The companion starts with the editor and stays hidden until you want it:

- **Click the spanner** in the top bar, just right of the Settings gear, to open its window and bring it to the front. If the companion had stopped, the spanner starts it again.
- Closing its window only hides it. The companion runs as long as the editor does, and ends when the editor ends, including when the editor crashes.
- The Console shows its log lines under the `Companion` category, and `Companion ready (round trip … ms)` each time it connects.

Its window has five screens on the left: **Profiler**, **Render**, **Debug**, **Jobs** and **Games**.

## Profiling a game

1. Open the **Games** screen. It lists everything that can be profiled: the editor itself, a game started with **Play**, and games exported with profiling allowed (see below) running anywhere on your network.
2. Press **Connect** on one. Recording starts at once and the Profiler screen opens.

### The Profiler screen

- **Frames** at the top: one bar per frame, newest on the right, with a line at 16.6 ms (60 fps). Amber bars went over it. Click a bar to inspect that frame; its CPU and GPU time, span count, entities and threads show at the right of the header.
- **Timeline** below: one lane per thread, every system as a bar, nested work stacked under the system it ran in.
  - **Mouse wheel** zooms around the pointer, down to microseconds.
  - **Shift + wheel** or **dragging** pans.
  - **Wheel over the thread names**, **Ctrl + wheel**, or dragging up and down scrolls through the threads, a lane at a time. The timeline says how many threads are above and below.
  - **Fit frame** frames the selected frame; **3 frames** shows it with its neighbours.
  - The **GPU** lane shows each render pass's GPU time, laid end to end: the GPU reports how long a pass took, not when it started.
  - Lanes run main thread first, then the render thread, then the task pools. The ruler counts from the start of the frame in view.
  - Spans that wrap the whole frame (the app update, `schedule: Main`, the render sub-app) are left out of the lanes, so each lane starts at the work; the frame markers show where frames begin and end.
- **Slowest in view**, under the timeline, totals the time each system spent inside what the timeline shows, so zooming in on a spike tells you what caused it.

The header controls recording:

- **Record** starts and stops recording.
- **Live view** pauses the screens while recording carries on. Use it for precise measurements: a companion that is not redrawing costs the machine almost nothing.
- **Save capture** writes everything recorded to `~/.renzora/captures/<game>-<time>.rzprof`. Saved captures are listed on the Games screen to open later.

The companion runs at a lower priority than the editor and the game, so its own jobs give way to whatever you are measuring.

### Your own code in the profiler

Every system is timed on its own and named after its function, so your systems appear without any work. To time part of a system, wrap it in a span:

```rust
fn rebuild_navmesh(/* ... */) {
    let _span = info_span!("navmesh: bake tiles").entered();
    // ... the slow part
}
```

The span appears inside the system's bar in the timeline.

### Following the newest frame

While recording, the timeline follows the newest frame, stepping to it twice a second so there is time to read it. Zoom with the wheel or the + and - buttons to choose how much of the frame to watch; following keeps that zoom. Dragging, panning or clicking a frame stops following, and **Follow** resumes it.

### Asking from an AI assistant

With the MCP Server plugin installed, an assistant connected to the editor can profile it: `profiler_record` starts a recording (of the editor, or of any game the Games screen lists), `profiler_summary` reports frame times and the slowest systems and GPU passes, and `profiler_spikes` explains the slowest frames by what took longer than usual in each.

### The Render screen

The render graph's passes as nodes, numbered in the order they ran in the selected frame, each coloured by its time: GPU time where the GPU measured the pass, CPU time otherwise. The graph reads top to bottom: each row is a group of passes that ran at the same time, and the wires join each group's longest pass to the next row, so they trace the frame's critical path. It opens at the top, as wide as the widest row allows; scroll down to follow the frame. Click a node to see its times under the header. A scene with several cameras runs the graph once per camera; **Next camera** steps between them.

A pass's system runs on whichever worker thread is free, so the screen finds a camera's passes by time: a system that, over the last two seconds, only ever ran inside that camera's render schedule. Recording for a second or two before looking gives it enough frames to tell them from the game's own systems.

## The Debug screen

The editor's debug readouts live here, not in the editor's window: Performance, System, Render Stats, Memory, ECS Stats, Camera Debug, Culling, Material Diag, Scripting Diag, Scene Diagnostics, UI Reactivity, UI Layout and Render Toggles. Tick the ones you want in the list on the left; each shows as a card, updated a few times a second, and the switches and buttons on a card (Render Toggles, camera selection, culling distances) act on the editor or game you are connected to.

They moved because a panel that changes every frame makes the editor lay out its whole window again every frame: with the old Debug workspace open, that alone took 30 ms a frame. A readout is gathered only while its card is showing, so one you have not ticked costs nothing.

Plugins add their own; see [Debug Probes](../extending/probes).

### Profiling a game on a phone or headset

Turn on **Allow profiling** in the export dialog's Output options. The exported game then announces itself on your network and appears on the Games screen wherever it runs: a phone, a Quest, a Steam Deck.

Only your editor's companion can connect. The export carries your editor's pairing key, kept in `~/.renzora/profiling_key`; delete that file to make a new key, and builds made before stop being able to pair. Leave **Allow profiling** off for builds you ship: a game without it never announces itself or opens a port.

## Jobs

The **Jobs** screen shows work the companion is doing for the editor: what is running with its progress, what is waiting and why, and what finished. Any job can be cancelled.

These run in the companion while it is connected:

| Job | What it does |
|---|---|
| Model imports | Converts the model, bakes its textures and optimizes its meshes; the import window's progress bar follows it |
| Model files | Builds the `.rvg` an accepted import is stored as |
| Model thumbnails | Renders a model's thumbnail for the Assets panel, with the editor's own capture, into the same cache |
| Material thumbnails | Compiles a `.material` and renders its sphere (or sprite) for the Assets panel, with the material editor's own capture, into the same cache |
| Texture thumbnails | Decodes and shrinks an image for the Assets panel |
| Rust scripts | Compiles a `.rs` script after you save it |
| Virtual geometry | Builds the `.rvg` beside a `.glb` added to the project some other way |
| Marketplace uploads | Sends a publish to renzora.com |

If the companion is not connected, the editor does the same work itself, as it always has; a material the companion cannot render is rendered by the editor too. Exports and plugin builds stay in the editor.

### Jobs never run while you profile

While a recording runs, every job waits, so the profile measures only the game: an import or a compile would otherwise compete with it for the machine. Jobs that render (model and material thumbnails) also wait while **Play** runs. The Jobs screen says what each job is waiting for, and **Run anyway** starts one now. Frames recorded while a job runs are greyed in the frame graph with a strip across the top, and `profiler_summary` counts them, so a hitch the companion caused is never mistaken for one in your game.

## The Cache screen

The companion watches the open project and keeps its caches in step with it:

- Once a project has opened, every model and material without a current thumbnail gets one. The loading screen no longer waits for material thumbnails: the project opens straight away and they fill in afterwards.
- A model or material added to the project, or changed, gets its thumbnail rendered straight away, so the Assets panel already has it when you open the folder.
- A model or material you delete takes its thumbnail with it.
- The screen shows the project's files and the thumbnail cache's size, with **Render missing thumbnails** to fill it and **Clear thumbnail cache** to empty it.

Thumbnails rendered this way are rendering jobs like any other: they wait while a game plays or a profile records. They also go one at a time and only while nothing else is queued, so the thumbnails of a folder you open in the Assets panel are rendered first.

The companion also lists the project for the editor when it opens, so panels that need the project's files (the Animation workspace's model list, for one) read one list kept current by the file watcher instead of each searching the disk.

## How it works

The companion is the same `renzora` executable, started by the editor with `--companion`. Because both come from the same build they always agree with each other; there is nothing to install or keep in step.

- It never renders your scene. The editor keeps the viewport and the scene. It renders one model or material at a time for a thumbnail, and frees it when the thumbnail is done.
- It talks to the editor over a local connection only the editor's own companion can join.
- It redraws only a few times a second, and not at all while you are not looking at it.
- If it stops unexpectedly, the editor starts it again, up to three times per session.

Its code ships only with the editor. An exported game carries none of it; a game exported with profiling allowed carries only the small part that sends data.

## Turning it off

Set the environment variable `RENZORA_NO_COMPANION=1` before starting the editor to run it without the companion.

## Related

- [Play Mode](./play-mode)
- [Exporting](../exporting/overview)
