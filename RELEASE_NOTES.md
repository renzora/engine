<!-- r1-alpha7 -->

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
  and opens a real project, and every nightly refreshes the playable build at
  renzora.com/engine.
- **Rendering**: 3D gaussian splatting, FFT ocean, volumetric clouds, GPU
  foliage, one world wind, Solari fixes, streaming and mesh LODs.
- **VR play mode**, with fully decorated eye cameras and a live in-headset
  environment.
- **Modeling and sculpting**: vertex, edge and face editing behind its own
  extrude-and-inset gizmo, six brushes including Snake Hook, Clay, Crease and
  Scrape, a sculpt mask that holds geometry still under every brush, dyntopo in
  the interactive stroke, X symmetry, and a Matcap view that shades by curvature
  for judging a form.
- **A shading switch** across the top of the viewport: Wireframe, Solid,
  Material, Rendered, each adding exactly one thing to the one below it. The
  blockout grid behind them is a projected greybox texture now, world-scaled
  rather than stretched across a mesh's authored UVs, so it stays square through
  an extrude, an inset and a resize.
- **A Terminal panel**: a real shell on a real pseudo-terminal, with tabs that
  shrink before they clip, 10,000 lines of scrollback and full key coverage, so
  `vim`, `htop`, `git add -p` and a dev server run inside the dock and behave as
  they do anywhere else.
- **One settings file**: every editor preference lives in
  `~/.renzora/settings.toml` and an older install's settings move across on
  first launch. With it, the two thirds of the Settings pages that were
  forgotten on restart now persist, and a rebound shortcut survives one.
- **The splash is a dashboard**: recent projects as cards showing the last
  snapshot of each main scene, plugins and starter templates installed from the
  marketplace without opening a project, and every page translated into all 20
  shipped languages.
- **Import is built around the model**, not the file list: a lit preview with
  the viewport's own sky, grid and axis gizmo, per-mesh inspection, per-file
  conversion progress, a destination folder picker, and a Reconvert button
  instead of a silent reconversion on every settings change.
- **Terrain paint layers render their own material**, procedural graphs
  included, tiled in world units with a per-layer Tile Size instead of one
  repeat stretched across the whole terrain.
- **More of the editor is reachable from a plugin**: register a whole workspace
  rather than only a panel, borrow the transform gizmo, undo a command you just
  recorded, read the material node catalogue, queue a folder through the import
  pipeline, claim the keyboard for a frame, and save your own settings
  (ABI 4.11).
- **Editor**: full undo/redo across every panel, floating windows and
  multi-monitor docking, Simulate mode, a global bottom panel, reorderable
  inspector sections, View > Reset to Defaults, and an exit that is immediate
  rather than a 7.6 second teardown.
- **Export**: lean web export, capability auto-detection, `.AppImage`/`.app`
  bundles, and modding on by default.
- **The engine publishes and updates itself**: nightlies, releases, and in-place
  updates from the editor.
- **Binaries are ~87% smaller**: `renzora.exe` 187 MB to 24.9 MB; the installed
  tree ~470 MB to ~77 MB.

## Security

- **A marketplace plugin archive containing an absolute path escaped the install
  directory** and could write anywhere the editor could. The guard tested for
  `..`, which such a name does not contain, and `Path::join` discards its base
  when handed an absolute path. Since an installed plugin is compiled and
  loaded, this was a code-execution channel. Both extraction loops now use the
  zip crate's own `enclosed_name` check.
- **A malformed USD or Alembic file is reported instead of taking the editor
  down.** All 28 offset reads go through checked helpers; the guards they
  replace were written `off + 8 > len`, which wraps rather than catching
  anything in a profile with overflow checks off.

## Breaking

- **Rebuild every prebuilt plugin against ABI 4.11.** The C ABI was introduced
  and revised twice this cycle; two of those were majors (3.0 repaired an
  `Interface` table whose functions had been inserted rather than appended, 4.0
  made the three crossing enums newtypes).
- **First-party plugins are no longer in this repository.** Install them from
  the marketplace; `cargo renzora plugin <name>` is gone.
- **Four editor features moved out of the binary and into plugins**: modeling
  and sculpting (without it there is no Mesh Edit section and no Edit or Sculpt
  viewport mode), the thirteen debug panels and the Debug workspace, the status
  bar's CPU/RAM/GPU readouts, and the Terminal panel.
- **Editor preferences leave `project.toml`.** Camera sensitivity, the grid,
  gizmos, snapping and the scene and tabs you had open are per user now, and
  `project.toml` keeps only what a shipped game needs. An existing project
  migrates on first launch.
- **A primitive saved in an older scene keeps its box collider.** Stairs, ramps,
  wedges and seventeen other building blocks now default to a collider shaped
  like the mesh, but existing scene objects keep whatever was saved with them:
  switch the shape to Mesh in the Physics inspector to pick the fix up.
- **Built-in shapes spawn white**, instead of each carrying a hue of its own.
  The tint is still per-shape and still yours; it just starts neutral, which is
  what a greybox and its grid want.

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
