<!-- r1-alpha7 -->

## Unreleased
- refactor(audio): the audio engine moves from `plugins/audio` into
  `crates/renzora_audio_backend` and is linked into the binary. It registers
  through the same C-ABI backend contract, so a replacement backend still loads
  from `plugins/`; what changes is that the bundled mixer can no longer go
  missing, and is stripped by `renzora_runtime`'s `audio` feature instead of by
  deleting a file.

## Highlights

- **C-ABI plugins**: write plugins in Rust with **no Bevy dependency**, so one
  built by any rustc loads into any engine. Covers physics, HTTP, audio,
  textures, materials, render passes, input and file dialogs, with hot-swap and
  a `no_std` mode. **Native plugins** ship as Rust source and compile on the
  user's machine.
- **Rust scripts**: per-entity native code with full `World` access, running in
  exported games as well as the editor.
- **A 2D editor**: avian2d physics, tilemaps, sprite animation, 2D lighting and
  particles, and an independent 2D camera per viewport.
- **A UI workspace**: building a menu no longer takes over the scene view, with
  world-space UI on 3D quads and a full visual UI editor.
- **The web editor**: the editor itself compiles for `wasm32`, boots, renders
  and opens a real project.
- **Rendering**: 3D gaussian splatting, FFT ocean, volumetric clouds, GPU
  foliage, one world wind, Solari fixes, streaming and mesh LODs.
- **VR play mode**, with fully decorated eye cameras and a live in-headset
  environment.
- **Editor**: full undo/redo across every panel, floating windows and
  multi-monitor docking, mesh modeling and sculpting, Simulate mode, a global
  bottom panel, and localization in 20 languages.
- **Export**: lean web export, capability auto-detection, `.AppImage`/`.app`
  bundles, and modding on by default.
- **The engine publishes and updates itself**: nightlies, releases, and in-place
  updates from the editor.
- **Binaries are ~87% smaller**: `renzora.exe` 187 MB to 24.9 MB; the installed
  tree ~470 MB to ~77 MB.

## Breaking

- **Rebuild every prebuilt plugin against ABI 4.10.** The C ABI was introduced
  and revised twice this cycle; two of those were majors (3.0 repaired an
  `Interface` table whose functions had been inserted rather than appended, 4.0
  made the three crossing enums newtypes).
- **First-party plugins are no longer in this repository.** Install them from
  the marketplace; `cargo renzora plugin <name>` is gone.

## Contributors

- **Kassinity** ([#92](https://github.com/renzora/engine/pull/92)): shader and
  material compile errors attributed to the graph node that caused them,
  generated shaders validated, WGSL pulled through one `renzora::wgsl` seam
- **Lucas Mundim** ([#96](https://github.com/renzora/engine/pull/96),
  [#98](https://github.com/renzora/engine/pull/98),
  [#102](https://github.com/renzora/engine/pull/102)): unwind symbols stubbed so
  `no_std` plugins load again, a missing `SuppressShadowMaps` tolerated on
  Solari's first extract, exact civil-date crash timestamps
- **saki2fifty** ([#89](https://github.com/renzora/engine/pull/89),
  [#90](https://github.com/renzora/engine/pull/90)): snap settings honoured in
  modal G/R/S transforms, the rotate HUD readout snapped
- **dreamersilly** ([#76](https://github.com/renzora/engine/pull/76),
  [#78](https://github.com/renzora/engine/pull/78)): a green CI build after it
  had been red since 14 June 2026, deleted entities restored under their
  original parent
- **Umut Faruk** ([#87](https://github.com/renzora/engine/pull/87)): directory
  drag-and-drop import
