# Standalone companion: plan

Status: steps 1 to 6 done (the shared crate, protocol 2, the feature gates, the server skeleton, the profiler, the render worker). The server is the companion the editor starts; `RENZORA_COMPANION_SERVER=0` starts the engine companion instead, which still has the jobs steps 7 to 9 move.

## Why

The companion takes slow work off the editor so the editor's frame rate stays smooth. Today it is a second copy of the engine: the same executable started with `--companion`, a full Bevy app with a window, ember UI, a GPU device and the asset system. Measured on Windows it holds about 980 MB committed, 630 MB resident and 345 MB of video memory, all the time, even with nothing queued.

Almost none of its work needs any of that. Of its fifteen job kinds, only the three that render thumbnails (models, materials, Shape Library shapes) need Bevy's renderer. The rest are file, CPU and network work. The profiler is plain data over TCP.

The plan: a small server with no Bevy in it at all, holding the job queue, the project cache, the profiler and the debug views, with a light UI. When a thumbnail has to be rendered, it starts a headless render worker (the engine, no window), sends it the batch, and lets it exit once the queue is empty. The engine-sized memory then exists only while thumbnails are actually rendering.

Expected idle cost of the server: roughly 50 to 150 MB with almost no video memory. This is an estimate, to be measured once the skeleton runs.

## Shape

```
editor (renzora)  <--- protocol (JSON lines, TCP) --->  companion server (renzora-companion, no Bevy)
                                                            |  jobs, cache, watcher, profiler UI (egui)
games (any device) --- profiler wire (binary, TCP) -------->|
                                                            |  spawns on demand, same protocol
                                                            v
                                                 render worker (renzora --render-worker, headless)
                                                  model / material / shape thumbnails, exits when idle
```

- **Editor.** Unchanged in role. `renzora_companion::host` starts the new executable instead of itself (since step 5; `RENZORA_COMPANION_SERVER=0` goes back to itself), and keeps doing a job itself when the companion cannot.
- **Companion server.** A new binary, `renzora-companion`, staged beside `renzora` in `dist/<platform>/`. It links no Bevy crate, directly or through another crate; a check in CI enforces that (see Guarding the line).
- **Render worker.** The engine started with `--render-worker`: no window, no editor UI, the thumbnail capture plugins from today's `app/render.rs` unchanged, so a thumbnail looks the same wherever it is made. It connects back to the server with the same protocol and exits after a short idle time (30 seconds to start with).

The server is a second executable, which is a deliberate exception to "one binary". The point of it is that it does not carry the engine, and the engine's one binary is the engine.

## Protocol changes

`PROTOCOL_VERSION` is 2 (done).

- **`Hello` lists the job kinds this side can run** (done). `CompanionJobs::submit` and `CompanionRemote::run` send a kind only when the connected companion listed it (`CompanionJobs::can_run`), and otherwise return `None` so the editor does the work itself, exactly as it does with no companion. This is what lets job kinds move to the server one at a time with nothing broken in between, and it is how the server says which kinds it hands on to the render worker. The Bevy companion's list is the `KINDS` constant beside each module's handlers (`work`, `render`, `cache`, `analysis`); a kind missing from it quietly runs in the editor.
- **The render worker speaks the same protocol to the server** that the server speaks to the editor: `Hello` (with the kinds it renders), `Submit`, `Progress`, `Done`, `Cancel`. The server forwards a job, and relays its progress and result back to the editor under the editor's job id.

The profiler's binary wire format does not change.

## Shared crates without Bevy

The server can only link code that does not pull Bevy in. The job code itself is almost all plain Rust already; what pulls Bevy in is a derive on a data type, a log macro, or a type that lives in the `renzora` contract crate (which always links Bevy). The splits, cheapest first:

| What | Today | Change |
|---|---|---|
| Protocol, `job_args` | `renzora_companion::protocol` (plain), `renzora::job_args` (plain, in a file with Bevy types) | New crate `renzora_companion_proto`. `renzora` re-exports `job_args` from it. |
| Profiler wire, frame types, probe reports | `renzora_profiler::wire`/`frame`, `renzora::profiling::{Edge, SpanEvent}`, `renzora::probes::{ProbeReport, ProbeSection, ProbeItem, ProbeLevel}` | Into `renzora_companion_proto` too; the old paths re-export. `ProbeDef` and the Bevy resources stay in `renzora`. |
| Texture bake (`renzora_rmip::bake`) | Bevy only in the asset loader | A default `bevy` feature gating `mod loader`. |
| SVG thumbnails (`renzora_svg::raster::thumbnail`) | Bevy only in the loader and `to_image`/`rasterize` | A default `bevy` feature gating those; move `SvgLoadError` out of `loader.rs`. |
| Virtual geometry bake (`renzora_rvg::bake`) | Bevy only in `RvgFile`'s `Asset` derive and the loader | A default `bevy` feature gating both; move `RvgLoadError` out of `loader.rs`. |
| Rust scripts | `renzora_rust_script::build_to_path` (plain std plus `Sdk`) in a Bevy crate | Move `build_to_path` and `build_key` into `renzora_plugin_build`, which has no Bevy. |
| Import | Conversion code is plain; it uses data types from `renzora` (animation clips, `PbrAdvanced`, `sanitize_id`) and one `warn!` | New crate `renzora_formats` for those types, re-exported by `renzora`. The animation easing type is Bevy's `EaseFunction`: mirror it, or depend on `bevy_math` alone (no App, no ECS). Gate `pipeline.rs` (it needs `World`) behind a feature. |
| Marketplace install, hub images, uploads | Job logic is plain, but sits in UI modules; every request goes through `renzora::net`, whose queue only moves while a Bevy system pumps it | New crate `renzora_marketplace_core` (auth API, install, upload, hub image fetch), `AuthSession` as a plain struct. HTTP through a blocking client (`ureq`, as `renzora_http` already uses). Move `prebuild` into `renzora_plugin_build`. |
| Export | Plain process and file work, spread across many files that each touch something Bevy (`CurrentProject`, `ProjectConfig`, `renzora_native_plugin`, `renzora_net`, log macros) | New crate `renzora_export_core`; `ProjectConfig` and `WindowMode` move into `renzora_formats`; a plain record replaces `InstalledNativePlugin`. The largest single split. |

`renzora_plugin_build` (plugin compiles) needs no change: it has no Bevy already.

## The server's own parts

Ported from `crates/renzora_companion/src/app/`, about 5,000 lines today, most of it UI:

- **Link** (`link.rs`): the editor connection. Plain TCP and threads.
- **Jobs** (`jobs.rs`, `work.rs`): the queue, priorities, workers (three), the hold while a game plays or a profile records. Plain, once the Bevy `Resource`/system wrapping is replaced by a loop.
- **Cache** (`cache.rs`): the project's files, its watcher and the thumbnail backlog. The watcher becomes the `notify` crate directly instead of Bevy's file watcher.
- **Games and capture** (`games.rs`, `capture.rs`, `analysis.rs`): LAN discovery, the profiled game's stream, kept captures and the questions the editor asks of them. Plain data.
- **UI** (`ui/`): rewritten in egui on eframe with the OpenGL (glow) backend. The pages stay: Profiler (timeline, frame graph, top systems, GPU passes), Render graph, Debug (probe cards), Jobs, Cache, Games. egui's painter replaces the ember canvas (`ui/canvas.rs`), which today draws the timeline as meshes on its own camera.

egui is new to the workspace; it is not built on Bevy, so it comes from crates.io like any other plain crate.

The asset ledger (what each scene loaded, who holds it, when it was freed) stays editor-side bookkeeping, reported as a probe; the server's Debug page shows it like any other probe, so it needs nothing of its own here.

## Order of work

Each step leaves everything working, and can be committed on its own.

1. **`renzora_companion_proto`** (done). The protocol, `job_args`, the profiler wire and frame types (with `Edge` and `SpanEvent`), and the probe report types, re-exported from where they were. No behaviour changes.
2. **Protocol 2** (done). `Hello` carries the job kinds; `CompanionJobs::submit` sends only those. The Bevy companion lists all its kinds, so nothing changes yet.
3. **Feature gates** (done). A default `bevy` feature on `renzora_rmip` (the loader), `renzora_svg` (the loader, the plugin, `to_image` and `rasterize`; `SvgLoadError` moved to `error.rs`) and `renzora_rvg` (the loader and `RvgFile`'s `Asset` derive; `RvgLoadError` moved to `error.rs`). `build_to_path` and `build_key` moved into `renzora_plugin_build::script`. With `bevy` off, `cargo tree` shows no Bevy crate in any of the three, nor in `renzora_plugin_build` or `renzora_companion_proto`.
4. **Server skeleton** (done). `crates/renzora_companion_server`, binary `renzora-companion`, staged by xtask beside `renzora`. How it came out:
   - **Kinds.** `texture_thumbnail`, `texture_bake`, `rvg_bake`, `rvg_migrate`, `rust_script` (`work::KINDS`) and `project_index` (`cache::KINDS`). The plan said plugin builds too, but nothing submits a plugin build as a job, so there is no kind to move.
   - **One state thread.** `server::run`, on the main thread, owns the queue, the cache and the editor's state. The editor's lines, the workers' reports, the watcher's changes, walks and measures landing, and the window's buttons all arrive as `server::Event`s on one channel. Nothing else holds the state, so there is no lock.
   - **The window is a process.** A `Show` starts `renzora-companion --window` (`window.rs` on the server's side, `ui.rs` in the child). The server sends it a `view::View` of its state as a JSON line on its stdin after each batch that changed it, and reads `view::Command`s (Cancel, Clear thumbnail cache) from its stdout; its stderr is the server's, so its log reaches the editor's Console. Closing the window ends the process; the companion going closes its stdin, which ends it too. egui 0.37, glow backend.
     The first version drew the window on the server's main thread and closed eframe on close. Measured, that kept about 90 MB and seven threads in the server for the rest of the session: the GL driver (`nvoglv64.dll` here) stays loaded once a context has been made, whatever happens to the context. In a child, the server never makes a GL context at all, so it stays at the 3.5 MB it measured before the window first opened, and the driver goes with the child.
   - **Foreground.** The editor calls `AllowSetForegroundWindow(ASFW_ANY)` when the spanner is pressed, since only the process that got the click may let another take the foreground; without it the window could open behind the editor.
   - **Watcher.** `notify` directly. A burst is collected for 500 ms and each changed path is then looked at as it is now (file, folder, or gone), rather than trusting the event kinds, which differ between platforms for renames and folders moved in.
   - **Not yet: rendering thumbnails ahead of time.** The engine companion renders every missing model and material thumbnail once the project is listed. That needs the renderer, so it waits for the render worker (step 6); until then the editor renders one when a folder asks. Deleting a model's thumbnail with the model does work here.
   - **Opt-in until step 5.** Starting it by default before the profiler moved would have lost the profiler and the MCP `profiler_*` tools, which only the engine companion answered.
   - **Its own cargo invocation.** Cargo unifies features across every package one invocation selects, so inside the `--workspace` build the engine turns `bevy` back on in `renzora_rmip`, `renzora_svg` and `renzora_rvg` and the server links the engine. `xtask::build_companion` builds it alone, and the workspace build, the SDK stager and `docker/build-all.sh` exclude it. `cargo renzora check-bevy` reads its graph alone too (`bevy_source::ENGINE_FREE`) and fails if any Bevy crate is in it.
   - **Moved into `renzora_companion_proto::project_files`** so both sides share one rule: `is_ignored`, `is_transient`, `IGNORED_DIRS`, `IndexedFile`, `walk_project`, and `cached_thumbnail`/`thumbnail_cache_dir` (the editor's `model_thumb_path` and `material_thumb_path` call it). The old paths re-export.
   - **A CRT stand-in.** `intel_tex_2`'s prebuilt ASTC archive imports the DLL C runtime's `_wassert`; the engine's links never pull that object in, the server's does. `renzora_rmip::bake` supplies `__imp__wassert` beside its `_Xlength_error`.
   - The engine companion's `work.rs` and `jobs.rs` are ported, not shared: they go when it does (step 10).
5. **Profiler** (done). How it came out:
   - **Ported, not shared.** `games.rs` (beacons and the connection, now one `Profiling` the server thread owns), `capture.rs` and `analysis.rs` (the `profiler.*` questions the MCP tools ask) are the engine companion's with the Bevy resources and systems taken out. `BEACON_PORT` and `BEACON_MAGIC` moved into `renzora_companion_proto::wire`; `renzora_profiler::server` re-exports them.
   - **The window keeps its own copy.** The server passes each packet from the game on to the window as it arrives, in the wire's own binary form (`view.rs`: a tag byte, then a JSON line or a wire packet), and the window keeps the last 360 frames (`WINDOW_FRAMES`) in a `Capture` of its own. Drawing from a copy is what lets the window be a separate process at all; sending a picture of each frame instead would have put the drawing work back in the server. When the window opens, or the capture is replaced (a new game, an opened file), the server sends a `CaptureReset` and the newest frames.
   - **The server never waits on the window.** A writer thread owns the pipe, and frames are dropped (not queued) while 30 are waiting for it, so a window that falls behind loses frames and nothing else does.
   - **Jobs wait while recording,** with "Run anyway" on the Jobs screen, and frames recorded while a job ran are marked, as in the engine companion.
   - **The screens** are egui painter calls: the frame graph, the timeline (wheel zoom, Shift+wheel and drag to pan, Ctrl+wheel or the wheel over the names for lanes), the slowest-in-view strip, the render graph as rows of nodes, and the probe cards. The probes watched are the Debug screen's while it shows, and none once the window closes.
   - **Measured** (profiling the editor itself, about 9,000 spans a frame): the server holds 10 to 15 MB a second of recording, so two minutes (`MAX_FRAMES`) of the editor is around 1.5 GB, the same frames the engine companion kept. Storing frames as their compact wire bytes instead of `Span` structs would roughly halve that; not done yet. The window holds 70 MB plus its 360 frames, about 180 MB for the editor. A game with a few hundred spans a frame costs a small part of that.
6. **Render worker** (done). How it came out:
   - **It is the engine companion, slimmed.** `renzora --companion --render-worker` boots the engine companion's app (`renzora_companion::app`) with only its link, queue and `render.rs`: no ember UI, no profiler, no watcher, no network. Its hello lists only the three rendering kinds. Reusing it keeps one copy of the capture plumbing until step 10, when `app/` shrinks to just this.
   - **The server plays the editor's part** (`render.rs` in the server): a loopback listener and a token on the worker's command line, and the same NDJSON protocol. Each job goes over under a number of the worker's own and its `Progress` and `Done` are put back under the queue's id. Not the queue's id itself: the server's own jobs carry `OWN_JOB`, which the engine companion (and so the worker) reads as one of its own and never reports, so the first thumbnail rendered ahead of time stayed "running" and the backlog behind it never moved.
   - **In the queue like any job.** A rendering job waits while Play runs or a profile records, counts as running while the worker has it, and is cancelled there with a `Cancel`. The queue hands started rendering jobs out (`JobQueue::take_render`) instead of to a thread.
   - **Started on demand, stopped when idle.** The first rendering job starts the worker; one idle for 30 seconds (`IDLE_EXIT`) has its link closed, which ends it, and a reaper thread kills it if it has not exited 10 seconds later. A worker that does not connect within 90 seconds, exits while starting, or drops its link fails every job it had; the editor then renders those itself, as it does with no companion.
   - **Memory per thumbnail.** The capture code's registries keep every finished thumbnail's image for the Assets panel; in a worker (and the engine companion before it) nothing shows them, and mcp_test's backlog grew the worker by about 1.5 MB a thumbnail. `render::finish_jobs` now drops each image once its PNG is written, which held the worker flat over plain materials and models. Materials with textures still grow it about 2 MB each (1,060 MB to 1,740 MB over 250 thumbnails of mcp_test, with dips), most likely the textures the material system's caches keep, as the editor wants them to. So the server replaces a worker after 200 thumbnails (`RECYCLE_AFTER`), at the next moment it has none in hand: a few hundred megabytes of growth at most, for about two seconds of engine start every couple of minutes of a backlog.
   - **Its log is filtered.** The server reads the worker's output and passes on only warnings, errors and panics (`render::pass_on`). Passed on whole, the capture code's four lines a thumbnail filled the editor's Console over a backlog and pushed everything else out of it.
   - **Only with an engine beside it.** The rendering kinds are announced to the editor only when `renzora` is beside `renzora-companion`, so a companion without one never takes a thumbnail it cannot render.
   - **Thumbnails ahead of time are back** (`cache.rs`): once the listing lands, a backlog of every model and material goes to the queue one at a time, only while it is idle, under ids with `OWN_JOB` set (not reported to the editor); a model or material added or changed is queued at once. The Cache screen's **Render missing thumbnails** fills the backlog again.
   - **Measured** with the previous engine binary standing in for the worker (it ignored `--render-worker` and booted the full engine companion): one model thumbnail took 4.1 s from the request, engine start included; the worker peaked at 1.1 GB private; the server stayed at 2 MB; the worker stopped 31 s after its last job. The slimmed worker, rendering a project's missing material and model thumbnails ahead of time: both on disk 5.3 s after the listing, 756 MB private at its peak, stopped 30 s after its last job, the server at 2 MB throughout.
7. **Import.** `renzora_formats`, and the import and model-build job kinds.
8. **Marketplace.** `renzora_marketplace_core` and a blocking HTTP client; installs, hub images, uploads.
9. **Export.** `renzora_export_core`.
10. **Remove the Bevy companion.** Delete `app/` from `renzora_companion` (the host stays) and the `--companion` boot path.

Between steps 6 and 9 the job kinds not yet moved (import, marketplace, export) run in the editor, as they did before the companion existed. `RENZORA_COMPANION_SERVER=0` starts the engine companion, which still runs them, for anyone who needs them off the editor's thread before then.

## Guarding the line

A crate that should not link Bevy will gain it back the first time someone adds a log macro. `cargo renzora check-bevy` (before every build, and in CI) reads `cargo tree -p renzora_companion_server` and fails if any Bevy crate appears in its graph at all, naming the crate that pulled it in (done, step 4). A package added to `bevy_source::ENGINE_FREE` gets the same check.

## Measuring

Before step 4 lands: the Bevy companion's idle committed, resident and video memory (the current figures above, measured again on the same machine). After it: the same figures for the server idle, the server with a full job queue, and the render worker while rendering a batch and after it exits. These go into this page.

## Open questions

- **Render worker idle time.** 30 seconds (`render::IDLE_EXIT`) is a guess. Too short and a stream of thumbnails pays the engine's startup again and again; too long and it holds memory nobody is using.
- **Shape Library thumbnails from plugins.** Shapes a plugin registers are rendered by the editor today, since the companion does not load plugins. The render worker could load them, at the cost of plugin loading in its startup time.
- **Web and mobile.** The companion is a desktop editor tool and is not built for wasm, iOS or Android; the server keeps that.
