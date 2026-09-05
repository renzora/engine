# Renzora Engine `r1-alpha7`

Two months and 703 commits. The plugin system became a real C ABI, the editor
gained a 2D mode and a UI workspace, the whole engine now builds for the
browser, and the first-party plugins moved out to the marketplace.

|                |                                                                                          |
| -------------- | ---------------------------------------------------------------------------------------- |
| **Previous**   | [`r1-alpha6`](https://github.com/renzora/engine/releases/tag/r1-alpha6) · 29 Jun 2026     |
| **Commits**    | 703 — 303 features, 225 fixes, 58 refactors, 15 perf                                     |
| **Changed**    | 2,017 files · +364,401 / −187,921                                                        |
| **Plugin ABI** | 4.10 — the C ABI is new in this release                                                  |

## Highlights

- **C-ABI plugins** — write plugins in Rust with **no Bevy dependency**, so one
  built by any rustc loads into any engine. Covers physics, HTTP, audio,
  textures, materials, render passes, input and file dialogs, with hot-swap and
  a `no_std` mode. **Native plugins** ship as Rust source and compile on the
  user's machine.
- **Rust scripts** — per-entity native code with full `World` access, running in
  exported games as well as the editor.
- **A 2D editor** — avian2d physics, tilemaps, sprite animation, 2D lighting and
  particles, and an independent 2D camera per viewport.
- **A UI workspace** — building a menu no longer takes over the scene view, with
  world-space UI on 3D quads and a full visual UI editor.
- **The web editor** — the editor itself compiles for `wasm32`, boots, renders
  and opens a real project.
- **Rendering** — 3D gaussian splatting, FFT ocean, volumetric clouds, GPU
  foliage, one world wind, Solari fixes, streaming and mesh LODs.
- **VR play mode**, with fully decorated eye cameras and a live in-headset
  environment.
- **Editor** — full undo/redo across every panel, floating windows and
  multi-monitor docking, mesh modeling and sculpting, Simulate mode, a global
  bottom panel, and localization in 20 languages.
- **Export** — lean web export, capability auto-detection, `.AppImage`/`.app`
  bundles, and modding on by default.
- **The engine publishes and updates itself** — nightlies, releases, and
  in-place updates from the editor.
- **Binaries are ~87% smaller** — `renzora.exe` 187 MB → 24.9 MB; the installed
  tree ~470 MB → ~77 MB.

## Breaking

- **Rebuild every prebuilt plugin against ABI 4.10.** The C ABI was introduced
  and revised twice this cycle; two of those were majors (3.0 repaired an
  `Interface` table whose functions had been inserted rather than appended, 4.0
  made the three crossing enums newtypes).
- **First-party plugins are no longer in this repository.** Install them from
  the marketplace; `cargo renzora plugin <name>` is gone.

## Contributors

**Kassinity** (shader and material error attribution, WGSL validation) ·
**Lucas Mundim** (`no_std` plugin linking, crash timestamps, Solari) ·
**saki2fifty** (transform snapping, material-graph Apply) · **dreamersilly**
(undo reparenting, green CI) · **Umut Faruk** (directory drag-and-drop import)

Merged PRs: [#89](https://github.com/renzora/engine/pull/89),
[#92](https://github.com/renzora/engine/pull/92),
[#96](https://github.com/renzora/engine/pull/96),
[#98](https://github.com/renzora/engine/pull/98),
[#102](https://github.com/renzora/engine/pull/102).
