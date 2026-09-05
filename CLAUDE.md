# CLAUDE.md — Renzora Engine

> **This file is the authoritative guide for working in this repository.** It
> overrides assumptions and habits from other Rust/Bevy projects. Read it before
> building, testing, writing plugins, extending the scripting API, or editing
> docs. When something here conflicts with what "usually" works, this file wins.

Renzora is a Bevy-based game engine + editor. The workspace is ~150 `renzora_*`
crates plus a small set of vendored/forked Bevy ecosystem crates. The engine
ships as a **single binary** that runs as the editor when the editor bundle is
present beside it, and as the shipped game/server when it isn't.

---

## 1. The `renzora` CLI

The `renzora` CLI drives a pinned Docker container, and its job is
**cross-compilation** — building for platforms you do not own. To build and run
on the machine you are sitting at, use `cargo renzora` (§2); you do not need any
of this. It is a **separately published tool**, not part of this workspace.

- Install: `cargo install renzora`
- crates.io: <https://crates.io/crates/renzora>
- Source: <https://github.com/renzora/cli>

| Command | What it does |
|---|---|
| `renzora init` | Pull/build the host toolchain image + create/start its container (idempotent) |
| `renzora check` | `cargo check` in the linux container (clippy-style gate) |
| `renzora test [args]` | Run the test suite in the linux container (no args = workspace suite) |
| `renzora build [platforms]` | Cross-build for one or more platforms (no args = all) |
| `renzora run` | Build for this host and launch it (editor by default) |
| `renzora add <name>` | Scaffold a new plugin crate |
| `renzora remove <name>` | Delete a plugin crate |
| `renzora shell` | Interactive shell inside the linux container |
| `renzora destroy` | Remove this checkout's containers + build-cache volumes |
| `renzora prune` | Remove this checkout's stale (non-current) toolchain images |
| `renzora new` | Create a new project by cloning the engine |

**Split toolchain images.** The toolchain is one shared base image
(`base`: rust + Linux deps + LLVM-19) plus one image per platform built
`FROM` it (`linux`, `windows`, `macos`, `ios`,
`android`, `wasm`). `renzora run` pulls only the host platform
image; `renzora build` (no args) pulls all; `renzora build windows` pulls only
Windows. Each platform runs in its own container; Linux-native ops (`test`,
`check`, `shell`, `clean`, `add`/`remove`, `upx`) use the linux container. Tags
are content hashes: `baseTag = sha256(docker/base/Dockerfile)` and
`<plat>Tag = sha256(baseTag + docker/<plat>/Dockerfile)`, so a base edit
cascades to every platform while a platform edit moves only its own tag. Stale
tags are pruned automatically on update.

If you need the user to run an interactive/auth command, suggest they prefix it
with `!` in the prompt so its output lands in the session.

---

## 2. Building & testing — native first, Docker for cross-compilation

**`cargo renzora` is how you install and run Renzora on your own machine.**
It builds the workspace natively, stages `dist/<platform>/` exactly the way the
container's `build-all.sh` does, and launches it. No Docker, no image pull, no
container. `rust-toolchain.toml` pins rustc so a native build matches the images.

**Docker is a cross-compiler, not the install path.** Its job is producing
**export templates for platforms you do not own** — building a macOS or Android
or wasm bundle from a Windows box, or a Windows one from Linux. That is a real
need and the reason the images exist, but it is a *shipping* concern. Reaching
for a container to run the editor on the machine you are sitting at is paying
for cross-compilation you are not doing.

### Why the C-ABI plugin system settles this

This used to be a genuinely hard call, because the plugin ABI depended on the
build environment. A distribution plugin shares one compiled `bevy_dylib` with
the host, and cargo names it `bevy_dylib-<metadata>` from a hash of the whole
build — feature set, profile, `RUSTFLAGS`, target, rustc. Build the engine in a
different environment and a prebuilt plugin imports a filename that is not there,
so "canonical env" meant "the env in which plugins were built", and Docker was
the only way to guarantee it.

**Standalone C-ABI plugins (§3) do not link Bevy at all.** They export one symbol
and import nothing — the interface is passed *in* as a function table — so there
is no filename to match, no `TypeId` to line up, and no environment to be
canonical about. A plugin built with any rustc on any machine loads into an
engine built with any other. That removed the last reason an ordinary user needed
the container, and it is why this section now reads the way it does.

In-workspace plugins are statically linked `rlib`s wired in by a build-time
generator, so they need no canonical build environment either.

**Native plugins (§3) bring the shared images back, and solve the same problem a
different way.** One of those *does* link the real Bevy and the real contract
crate, so it is bound to one engine build exactly as a distribution plugin used
to be. The difference is that it ships as **source** and is compiled on the
machine that installs it, against a **staged SDK** cut from the engine sitting
right there. There is no environment to match because the plugin is always built
in the environment it will run in — and when the engine moves, the recorded stamp
stops matching and the plugin quietly rebuilds itself. §3 covers all three paths.

### NEVER build the `dev` (debug) profile — one `target/` profile directory only

**Every cargo command in this repo takes `--profile dist`, or goes through
`cargo renzora`. No exceptions.** A bare `cargo build`/`check`/`clippy`/`test`
defaults to the `dev` profile and creates a *second* full set of artefacts under
`target/debug/`, and this workspace is far too large for two of them to coexist.

```sh
cargo renzora            # build + stage + run   ← the normal way to work
cargo renzora xr         # same, but XR-capable (headset editing; not pipelined)
cargo renzora dist       # build + stage, don't launch
cargo check  --profile dist [-p <crate>]
cargo clippy --profile dist [-p <crate>]
cargo test   --profile dist -p <crate>
```

**Why this is a hard rule and not a preference.** On 2026-08-11 `target/` reached
**314.5 GB** and filled a 929 GB disk to 1.38 GB free, because `dev` and `dist`
artefacts had both accumulated. A full disk does not fail cleanly here — rustc
writes **truncated `.rmeta`/`.rlib` files** and the next crate to read them fails
with errors that look like source bugs in code nobody touched:

- `renzora_inspector`: 1323 errors, every `bevy::prelude` item "not found in this
  scope" (`default`, `BackgroundColor`, `TextColor`, `in_state`)
- `renzora_import`: 302 errors, `cannot find module or crate 'validation'` — the
  `gltf` submodules
- `renzora_mixer`: a phantom `E0061` wrong-argument-count
- `rust-lld` crashing with a stack dump instead of a diagnostic

All of them vanished on a re-run once a little space came back, which is the
tell: **a compile error in a crate you did not touch, that disappears when you
run again, is a disk-space error.** Check `Get-PSDrive C` before believing it.

If `target/` has already grown a `debug/` directory, delete it —
`Remove-Item -Recurse -Force target\debug`. Nothing needs it.

### What runs where

- ✅ **`cargo renzora`** — native build + stage + run on the host platform. The
  normal way to work. Uses `--profile dist`. **Launches with `RENZORA_NO_XR=1`**:
  merely having an OpenXR runtime installed and set as the system default
  otherwise takes the XR-capable boot, which disables `PipelinedRenderingPlugin`
  and serializes the render sub-app onto the main thread (~11.6 ms of a 27 ms
  frame). Use `cargo renzora xr` to edit in a headset.
- ✅ `cargo check --profile dist` natively / via the editor — the fast gate while
  editing (doesn't link).
- ✅ `cargo clippy --profile dist` natively — links nothing, so it reproduces the
  CI gate exactly. Mirror CI's exclude list from `.github/workflows/test.yml`
  (notably `polyanya`), and don't add `--all-targets` — CI doesn't, and the extra
  test targets pull in vendored crates CI never lints.
- ✅ **`cargo test --profile dist -p <crate>` links and runs natively on
  Windows.** This used to be false: the test harness pushed the `renzora` dylib's
  export count to ~875k against the PE format's 65,535 ceiling and rust-lld
  hard-errored (`too many exported symbols`). The C-ABI plugin work removed the
  dylib that caused it, so the cap is no longer reached. Verified 2026-08
  (`cargo test -p renzora_ember` → links, runs, ~20 s warm). Prefer it for
  iterating — it is an order of magnitude faster than a container round-trip.
- ⚠️ **`cargo test --workspace` still fails**, but not on the export cap — on two
  vendored XR crates whose *examples* never got the Bevy 0.19 `shadows_enabled`
  → `shadow_maps_enabled` rename (`bevy_oxr`'s `3d_scene`, `bevy_xr_utils`'
  `tracking_utils`). `--workspace` builds example targets; CI does not hit this
  because it excludes those crates. Test per-crate, or use `renzora test`.
- ✅ `renzora build [platform]` — **cross-compilation, the reason Docker is
  here.** Required for export templates and release artefacts.
- ✅ `renzora check` / `renzora test` — reproduce CI exactly. Use when a result
  must match what CI will say, not as the default way to build. They run in the
  container, so they cost nothing in the host's `target/`.
- ❌ Don't "fix" a perceived link error by disabling `prefer-dynamic` or dropping
  `dynamic_linking` from the default features. `bevy_dylib` is no longer a plugin
  ABI concern (§3) — it is a *build-time* one: turning it off relinks the whole of
  Bevy statically into every build and costs minutes per iteration. Note that a
  *standalone* plugin must not inherit `prefer-dynamic`; `plugins/.cargo/config.toml`
  turns it off with an explicit `=no`, because an inherited one makes the plugin
  import a toolchain-versioned `std-<hash>.dll` that isn't there.

A note on the old "native can't link" claim: the shared `renzora` dylib plus the
full plugin set exceeds the PE 65,535 exported-symbol cap, which MSVC `link.exe`
refuses. That is not a blocker, because `.cargo/config.toml` pins the linker to
**`rust-lld`** for `x86_64-pc-windows-msvc` (host and container alike), which
raises the cap far enough for the normal build. Native links succeed; we simply
never use `link.exe`.

Pinned toolchain — **Rust 1.95.0**, **Bevy 0.19** (currently 0.19.**1**, and the
patch matters: `renzora_wind`'s prepass vertex shader reads the material bind
group, which bevy_pbr 0.19.0's `PrepassPipeline::specialize` replaces with an
*empty* layout on any depth-only opaque prepass. 0.19.1 added the
`prepass_reads_material()` escape hatch that keeps it bound. Downgrading to
0.19.0 brings back a wgpu validation crash — "group 3 binding 100 is not
available in the pipeline layout" — the moment a swaying mesh renders). The Rust version lives in TWO
files kept in lockstep: `docker/base/Dockerfile` (`FROM rust:1.95.0`, the
container) and `rust-toolchain.toml` (native `cargo renzora` / `cargo check`); a
bump must edit both. The base image is the foundation every platform image builds
`FROM`, so a container bump cascades to all platforms — see §3. CI
(`.github/workflows/test.yml`) runs `cargo test` + `cargo clippy -D warnings` in
the `base` image, excluding the vendored `bevy_*` / `vleue_navigator` crates. Keep
clippy green; the vendored crates must stay excluded.

---

## 3. Plugin mechanisms — three of them, for three deployments

There is no single plugin ABI. The old `plugin_bevy_hash()` export and the
`World` `TypeId` gate enforced by a `dynamic_plugin_loader` crate are gone — the
crate was deleted and `add!` emits no FFI. What replaced it is **three unrelated
mechanisms**, and the first question about any plugin is which one it is.

| | Crate type | Links Bevy | Access | Ships in |
|---|---|---|---|---|
| **In-workspace** | `rlib` + `add!` | statically, at build | full `&mut World` | the engine binary |
| **Native** | `dylib` in `plugins/<name>/` | the shared images | full `&mut World` | the editor, as source |
| **C-ABI** | `cdylib` in `plugins/` | not at all | a function table | a shipped game |

A native plugin extends the **editor**; a C-ABI plugin ships inside the **game**.
Neither replaces the other, and the reason is structural — a lean export is fully
static with no shared images, and wasm/mobile have no dylibs at all, so there is
nothing there for a native plugin to bind to.

### In-workspace plugins are statically linked, via a build-time generator

`renzora::add!(MyPlugin [, Editor|Runtime] [, priority = N])` is **not** a
runtime registry — it is a directive the build generator reads *as text*. The
generator finds every `add!` line and writes two committed files,
`crates/renzora_runtime/src/plugins.rs` and
`crates/renzora_editor/src/plugins.rs`, each an ordinary list of
`app.add_plugins(...)` calls. Dropping a crate into `crates/` with an `add!` line
in it is the whole job.

Because both lists are committed, a plain `cargo build` needs no generator run;
CI checks that regenerating produces no diff, which is what stops a stale list
from shipping. Keep the declaration on one line at the top level of the file —
the parse requires the full `add!(..);` form at line start, so a commented-out or
string-embedded one is ignored — and keep every module on the plugin's path
`pub`, since the type is resolved from the module the file defines. A wrong path
is a compile error in the generated file, never a silently missing plugin.

There is no ABI here at all: a named type in a generated list is just a linker
symbol. Deleting the old `inventory` registry also deleted the three dead-strip
workarounds that existed only to keep its constructors alive.

### Third-party extensions are standalone C-ABI plugins that link no Bevy

A standalone plugin cdylib exports exactly **one** required symbol,
`renzora_plugin_init` (`sys::INIT_SYMBOL`), plus an optional
`renzora_plugin_scope` (`sys::SCOPE_SYMBOL`; absent = `Runtime`). It imports
nothing from the host — the whole interface is passed *in* as a function table.
No Bevy, no `TypeId`, no shared dylib, so a plugin built with any rustc on any
machine loads into an engine built with any other. This is what removed the last
reason an ordinary user needed the container for *this* kind of plugin (§2).

Compatibility is negotiated in two layers, both in `crates/renzora_plugin/src/sys`:

1. **A version handshake** — `VERSION_MAJOR` (currently 4) and `VERSION_MINOR`
   (currently 10). Major breaks; minor appends. The full history sits above the
   constants in `sys/mod.rs`, including the two releases that *claimed* to append
   but actually inserted into the middle of `Interface` — which is why MAJOR is 4,
   and why layer 2 exists.
2. **`INTERFACE_PREFIX_HASHES`** — entry *n* hashes the shape of the first *n*
   fields of the interface table, so a plugin verifies the table it was handed
   rather than trusting the two numbers above. This is the layer that catches a
   mis-declared "append".

The loader (`crates/renzora_plugin/src/host/loader.rs`) is deliberately
symbol-dispatched: a library is a plugin only if it exports `INIT_SYMBOL`, and
anything else is skipped silently. It **never drops a loaded `Library`** — every
function pointer a plugin registered points into that image, a retired system is
still *in* the schedule merely returning early, and dropping the handle has
deadlocked in `FreeLibrary` here before. A reload therefore leaks one image; a
restart reclaims it.

### Native plugins are Bevy plugins shipped as source, built against a staged SDK

A `plugins/<name>/` **directory** holding `src/lib.rs` and a `crate-type =
["dylib"]` manifest. It exports one symbol, `renzora_native_plugin_ctor`
(`fn() -> Box<dyn Plugin>`, written by `renzora::plugin!`), and it links the real
Bevy and the real contract crate — so it takes `&mut World`, calls
`app.add_systems`, and sees the same `Transform` the engine does.

That is sound **only** because of the shared images. `dynamic_linking` is in the
default feature set and pulls three: `bevy_dylib`, `renzora_dylib` and
`renzora_ember_dylib`. All three are process-global-state problems, not merely
size ones — the contract crate owns the translation table, the Problems and
Console buffers and the asset loader; ember owns the theme palette, the
stylesheet, the UI font scale and the viewport-toolbar lists. A privately linked
copy of either gives a plugin its own set and every one of them then fails
**silently**. The loader declines to load anything at all when
`dynamic_linking` is off, because there is no runtime check available.

- `crates/renzora_native_plugin` — the loader. Scans directories (the C-ABI
  loader scans loose library files, so the two never collide), rebuilds what is
  stale, `ManuallyDrop`s every image.
- `crates/renzora_plugin_build` — the compiler driver. Invokes `rustc` directly,
  not cargo: a plugin is one crate whose dependencies are already compiled, so
  there is no graph to resolve and nothing to get `-C metadata` wrong.
- `xtask/src/sdk.rs` — stages `dist/<platform>/sdk/` on every build. The file
  list comes from `cargo --message-format=json`, **never** a directory scan;
  `deps/` holds many `-C metadata` variants of the same crate and name-matching
  produces a set that looks complete and fails to compile.

**The stamp is a content hash**, recorded beside a built plugin and compared on
load. It has to be: cargo derives a `-C metadata` filename hash from the build
*configuration*, never from source, so an earlier filename-based stamp did not
move when `crates/renzora` changed — and Rust mangles symbols from a crate's
stable id rather than its contents, so nothing downstream caught it either.

Never run `cargo build` inside a native plugin directory. `plugins/` is outside
the workspace, so cargo resolves it a fresh Bevy from crates.io; the result
builds, loads, and corrupts the World.

Rust **scripts** (`crates/renzora_rust_script`) are the same mechanism with a
per-entity convention on top — same driver, same SDK, same loading. See §7.

### `trace_tracy` still stays out of the normal build

No longer an ABI concern — a runtime one. Bevy installs its Tracy layer in
`LogPlugin` at boot whenever that feature is compiled in, with no runtime
off-switch, so it would arm Tracy (and grow RAM) on every launch. Tracy is opt-in
via the Tracy plugin from the marketplace, a standalone C-ABI plugin (frame
marks + diagnostic plots,
started on its own Settings toggle); per-system CPU zones need a dedicated
profiling build that re-adds `trace_tracy`.

---

## 4. Versioning & documentation

- **Current dev version: `r1-alpha7`.** From now on, **only edit
  `docs/r1-alpha7/`.** `docs/r1-alpha6/` is released and **frozen** (its frozen
  ABI hash + release commit are recorded in `releases.json` at the repo root) —
  do not mirror changes into it, nor into the older frozen `docs/r1-alpha5/`.
  Top-level non-versioned `docs/*.md` are still fair game.
- **The next version is opened after its predecessor's tag is pushed, not
  before.** `ENGINE_VERSION` is what the release workflow compares against the
  tag to decide whether it is building a release or a nightly, so the constant
  cannot run ahead of the tag — and `docs/_versions.json` must not run ahead of
  the constant. Bump, fork `docs/<next>/` and reset `RELEASE_NOTES.md` in one
  commit *after* the tag. `scripts/check-versions.sh` enforces the agreement.
- **Always update the docs after adding or changing a feature.** Stale docs are
  treated as a bug. If you ship a feature (new scripting function, new inspector
  field, new plugin capability, new editor panel), update the matching page under
  `docs/r1-alpha7/` in the same change.
- Docs are also published at <https://renzora.com/docs>. Pushing `docs/r1-alpha*`
  changes to `main` auto-publishes via `.github/workflows/sync-docs.yml` (rsync
  into the website repo, which redeploys). You do not copy anything by hand.

`docs/r1-alpha7/` sections include: `getting-started`, `setup`, `scripting`,
`api`, `editor`, `editor-dev`, `engine-core`, `rendering`, `extending`,
`exporting`, `packaging`, `multiplayer`, `marketplace`, `platform-api`,
`contributing`.

### Release notes

- **Every new feature and every fix adds its line to `RELEASE_NOTES.md`** at the
  repo root, in the same change that ships it. Treat a missing note exactly like
  a missing docs update: the change is unfinished.
- This is not bookkeeping for its own sake. **A nightly publishes every day
  something lands on `main`**, so these notes are the only record of what any
  given nightly actually contains. Reconstructing that afterwards means reading
  the commit range by hand — `r1-alpha7` was 700 commits, and that is the work
  this rule exists to stop repeating.
- New lines go under **`## Unreleased`**, always the top section. Below it the
  file is a history, newest first, of one section per published nightly:

```markdown
# Renzora Engine `r1-alpha7`

## Unreleased
- feat(editor): what it does, in the commit-subject voice

## r1-alpha7-nightly-06sep26
- fix(plugin): what broke, and what now happens instead

## r1-alpha7-nightly-05sep26
- ...
```

- **The publish job puts `## Unreleased` into the nightly's release page**, under
  *Since the last nightly*. So the section is written before the build that
  ships it, not after — which is why the heading is `Unreleased` and not a date.
  A nightly's tag is only known at build time (the schedule skips a quiet day),
  so naming the section ahead of time would mean guessing a date, and a guess
  that missed would publish the wrong list under the wrong tag.
- **After a nightly publishes, rename its section to the tag that shipped it**
  and open a fresh `## Unreleased` above. That is the only manual step, it is
  never urgent, and nothing breaks if it is late: CI keys on the `## Unreleased`
  heading alone and ignores every heading below it.
- An empty or absent `## Unreleased` is fine — the nightly falls back to the
  asset boilerplate on its own, which is what it published before these notes
  existed.
- **A major release overwrites the file.** Cutting `r1-alphaN` replaces
  `RELEASE_NOTES.md` with the curated notes for that version — prose, not the
  running list, since the running list has by then done its job. The notes it
  replaced stay published on each nightly's GitHub page, which is the permanent
  copy. The next version then starts a fresh `## Unreleased` under the new
  heading.
- **The first line must name the version being released.** The `setup` job
  refuses to start a release whose `RELEASE_NOTES.md` still names the previous
  one — the failure mode of a hand-written file is a stale one, not a missing
  one. See `docs/<version>/contributing/releases.md`.

---

## 5. Architecture (orientation)

- **`crates/renzora` is the contract crate** (`crate-type = ["rlib"]`, zero deps
  beyond Bevy + serde). It holds the shared types, events, components, resources,
  the post-process framework, and the editor contract (`editor` feature). **Every
  boundary-crossing type lives here** so all crates agree on one definition.
- **Plugin declaration:** every plugin declares itself with `renzora::add!(MyPlugin
  [, Editor|Runtime] [, priority = N])`. This is read *as text* at build time by a
  generator that writes the committed `plugins.rs` lists — it emits no `inventory`
  entry and no FFI (§3). See `crates/renzora/src/plugin_meta.rs`.
- **Editor / runtime split.** A plugin's scope is exclusively `Runtime` or
  `Editor` (there is no "both"). Runtime plugins run in the editor viewport AND
  the shipped game; Editor plugins run only when the editor bundle is present.
  A feature needing editor tooling on top of runtime behaviour ships **two**
  plugins. The lean pattern is `renzora_<name>` (runtime, in the binary) +
  `renzora_<name>/editor/` (`renzora_<name>_editor`, linked only by the editor
  bundle).
- **One binary; the editor is a loadable image.** `renzora` (package
  `renzora_app`) is the runtime *and* the editor: it looks for
  `renzora_editor.<dll|so|dylib>` beside itself at startup and installs it if it
  is there. Present → the binary is the editor; absent → the same binary is the
  shipped game, so "remove the editor" is deleting one file. See
  `renzora_runtime::editor_image`.

  It was a second executable for exactly as long as Bevy was statically linked,
  when a loadable editor would have carried its own copy of Bevy and therefore
  its own `World` type. `dynamic_linking` is back on by default, so that
  constraint is gone.

  **A loadable image must have every shared image in its link graph** —
  `renzora_editor` depends on `renzora_dylib` and `renzora_ember_dylib` for their
  side effect only. Without them it embeds a private copy of the contract crate,
  gets its own translation table, Console buffer and theme palette, and every one
  of them fails **silently**: the whole UI renders raw keys (`menu.file`,
  `common.settings`) with nothing logged, because a missing translation is not an
  error. Crates that hold no process-global state may be duplicated freely — a
  `TypeId` comes from a crate's stable id, not from which artifact swallowed it.

  `renzora_editor_app` survives for **wasm only** (`required-features = ["wasm"]`),
  which has no dynamic linking and builds the editor into a second `.wasm`
  bundle.
- **Building also builds the runtime** by design — an editor build always
  produces the runtime too. Don't propose editor-only scoping of a build.

---

## 6. Writing plugins

**Before creating or modifying a plugin, ALWAYS research the plugin API first.**
Read `docs/r1-alpha7/extending/plugins.md` and `crates/renzora/src/plugin_meta.rs`,
and look at an existing distribution plugin (`renzora_lumen`, `renzora_cloth`)
as a template. Use `renzora add <name>` to scaffold.

Principles (in priority order):

1. **Make plugins as modular as possible.** One plugin = one cohesive feature.
   Prefer a self-contained plugin over wiring a feature deep into the host.
2. **Pick the right of the two kinds.** An optional or third-party feature belongs
   in `plugins/` as a **standalone C-ABI cdylib** — it links no Bevy, loads into
   any build, and can ship independently. An engine feature belongs in `crates/`
   as an **`rlib` with an `add!` line**, statically linked by the generator (§3).
3. **Refrain from linking crates as much as possible.** Minimize a plugin's
   dependency on other `renzora_*` crates. When a type must cross a crate
   boundary, **move it into the `renzora` contract crate** rather than depending
   on the crate that defines it. This is the established pattern (GI settings,
   etc. live in `renzora`, not in their plugin).
4. **Multiple `add!` lines per crate are fine** (`renzora_ember` has four) — the
   old one-per-cdylib rule died with the FFI exports. Keep each on one line at the
   top level of its file so the generator's text parse sees it.
5. A plugin that mutates files in parallel with others, or that must initialize
   before another, is the rare case — most ordering should use Bevy's own system
   sets, not plugin `priority`.

---

## 7. Extending the scripting API

**The scripting system is statically linked; the interpreter is a plugin.**
`crates/renzora_scripting` owns the hooks, the command vocabulary, the context
and the queue that applies commands to the world. It contains no interpreter.
Lua is a standalone C-ABI cdylib installed from the marketplace — so which
language a game can
be scripted in is decided by which plugin is present, not by how the engine was
compiled. Rhai is gone.

Scripts live in `<project>/scripts/*.lua`, attach via `ScriptComponent`, and run
through hooks: `on_ready`, `on_update`, `on_rpc`, `on_ui`, `on_draw`,
`on_animation_event`, `on_http`, `on_player_joined`, `on_player_left`. Hooks are
selected by op code across the boundary, so adding a tenth is not an ABI break.

**`.rs` is the exception, and it deliberately bypasses that model.**
`crates/renzora_rust_script` compiles a `<project>/scripts/*.rs` into a native
plugin (§3) and calls it once per frame per entity with `&mut World`. It splits
the two jobs a backend usually does together: `RustScriptBackend` **claims** the
extension so the Scripts component accepts one and the execution loop does not
flag it as broken, and `dispatch` **runs** it from an exclusive system. It
returns no `ScriptCommand`s because there genuinely are none — the whole reason
to write Rust is the `&mut World` no command vocabulary can stand in for. Gated
on play mode exactly like Lua; recompiles on save, off the main thread.

**When writing scripts, refer to the scripting API first**
(`docs/r1-alpha7/scripting/` + `docs/r1-alpha7/api/scripting.md`). The
interpreter itself is not in this repository — see the note on plugins in §3.

**If a script needs a function that doesn't exist yet:**

1. Tell the user the function isn't in the API and explain how to proceed.
2. If feasible, **extend the scripting API** rather than working around it.
3. **Always prefer declaring new script functions from the owning `renzora`
   crate**, via the `ScriptExtension` trait — not by bolting them into the
   interpreter. The trait is now purely declarative: the crate says what a
   function is called and what arguments it takes, and *every* language backend
   builds it. A domain crate therefore links no interpreter at all.

   ```rust
   impl ScriptExtension for MyScriptExtension {
       fn name(&self) -> &str { "combat" }
       fn bindings(&self) -> Vec<Binding> {
           vec![Bind::action("deal_damage", "deal_damage")
               .arg("amount", ParamKind::Float)
               .build()]
       }
   }

   let mut extensions = app.world_mut().get_resource_or_insert_with(
       renzora_scripting::extension::ScriptExtensions::default,
   );
   extensions.register(my_crate::script_extension::MyScriptExtension);
   ```

   See `renzora_animation`, `renzora_physics`, `renzora_navmesh`,
   `renzora_ragdoll`, `renzora_lang` for real examples.
4. **Update `docs/r1-alpha7/` for the new function** (see §4).

Core/engine-wide primitives (`set_position`, `play_sound`, `spawn_entity`, the
reflection `set`/`get`/`set_on`, …) live in the language plugin's
`register_api()`. Domain functions belong in that domain crate's declaration.

**Adding a language** is a plugin: implement `renzora_plugin::script::Backend`,
claim your extensions, and the engine routes to you by file extension. Two
languages coexist in one project. See `docs/r1-alpha7/extending/script-backends.md`.

## 8. Code conventions

- **Edit source files with the Edit/Write tools — never through the shell.**
  No `python - <<'EOF'` heredocs, no `sed -i`, no `perl -pi`, no "write a patch
  script and run it". Those rewrites are **invisible**: the transcript shows a
  shell command and `ok`, so the change cannot be reviewed as it happens, a
  wrong edit is not caught early, and there is no diff to read. Read the file,
  then `Edit` with a unique `old_string` (or `replace_all` for a repeated
  change); use `Write` for a genuinely new file. If a change feels too fiddly
  for `Edit`, that is a signal it is too large for one step — split it, don't
  reach for a script. Shell remains correct for building, testing, searching,
  inspecting binaries and running probes; the rule is about *mutating source*.
- **Never use an em dash. Anywhere.** Not in code, comments, doc-comments,
  markdown, commit messages, release notes, UI strings, log lines, error text or
  panic messages. This is absolute: there is no context in this repository where
  `—` is the right character, and "it reads better here" is not an exception.
  The same goes for an en dash (`–`) used as punctuation; `-` in identifiers,
  flags and ranges is fine, and `−` in a numeric diff is a minus sign, not
  punctuation.
  Reach for a colon when the second half explains the first, a comma or
  parentheses for an aside, a semicolon for two joined clauses, or a full stop
  and a new sentence. Almost every em dash is one of those four wearing a
  costume, and picking the right one says what the dash left ambiguous.
- **Comment the WHY, not the what.** This codebase's hallmark is doc-comments
  (`//!` module, `///` item) that explain *why* the code is shaped this way, what
  edge case it handles, and what previously went wrong. Match that density and
  voice. Don't add narration that just restates the code.
- **Module layout:** `lib.rs` (module doc + plugin), `systems.rs` (systems),
  and one module per thing the crate builds — `panel.rs`, `graph.rs`,
  `inspector.rs`, `drawer.rs`. Types → systems → helpers.
  **Name a module for what it is, never for what it is not.** Every editor
  crate used to carry a `native.rs` / `native_*.rs`, from when each was a
  bevy_ui port sitting beside an egui original; the originals are gone, so the
  prefix distinguished a module from nothing. Same for a `Native*` type — it is
  a `FoliagePanel`, not a `NativeFoliage`. The one place `native` still means
  something is a `#[cfg(not(target_arch = "wasm32"))]` gate, and that is a
  platform, not a UI toolkit.
- **A file past about 1500 lines wants splitting**, into a directory of modules
  named for the sections it already has — `foo.rs` becomes `foo/mod.rs` plus
  siblings, with a flat `pub use` so every path a caller writes still resolves.
  The exceptions are worth knowing: a **single exhaustive `match`** (the wire
  codec in `renzora_plugin/src/script/command.rs`) is long because the compiler
  is proving every variant is handled, and splitting it into per-group functions
  would need a `_ =>` arm in each — turning a compile error into a silent
  runtime one. Length there is the price of the proof, not a defect.
- **Naming:** `PascalCase` types, `snake_case` fns/modules, `SCREAMING_SNAKE`
  consts. Crates are `renzora_<name>`.
- Follow Bevy ECS idioms. Avoid `unwrap()` in production paths. Default rustfmt.
- **Commits:** Conventional style — `feat:`, `fix:`, `docs:`, `refactor:`,
  `security:`, with optional scope, e.g. `feat(plugin): …`. Imperative mood,
  no trailing period.

---

## 9. Best practices (audit summary)

- **Edit files with `Edit`/`Write`, never a shell heredoc or `sed -i`** (§8).
  A change nobody can see in the transcript is a change nobody reviewed.
- **Trust the constraints.** The one-definition contract crate, the two-layer
  C-ABI negotiation, and the frozen-vs-current docs split are all load-bearing.
  Work *with* them.
- **`cargo renzora` to build and run, `cargo check --profile dist` /
  `cargo clippy --profile dist` to iterate, `renzora test` to verify.** Docker is
  for cross-compiling export templates, not for installing the engine on your own
  machine.
- **Never build the `dev` profile.** Every cargo command takes `--profile dist`;
  a bare one creates a second 300 GB `target/debug/` and a full disk shows up as
  nonsense compile errors in untouched crates, not as a disk error (§2).
- **Put shared types in `renzora`.** Any type two crates both need must have one
  definition, and that is where it lives.
- **Two plugins, not one "both" plugin,** when a feature needs editor tooling +
  runtime behaviour.
- **Nothing is `dlopen`'d against Bevy.** In-workspace plugins are statically
  linked `rlib`s; third-party ones are C-ABI cdylibs that link no Bevy and
  negotiate via version + `INTERFACE_PREFIX_HASHES` (§3). There is no
  `bevy_dylib` gate and no hash to maintain.
- **Docs are part of "done."** A feature without its `docs/r1-alpha7/` update is
  unfinished.
- **Verify before contradicting the user** about working-tree state; check the
  actual files.

---

## 10. Key file map

| Path | What it is |
|---|---|
| `crates/renzora/` | Contract crate (`rlib`): shared types/events/components, editor contract |
| `crates/renzora/src/plugin_meta.rs` | `add!` + `PluginScope`; what the build generator parses |
| `crates/renzora_runtime/src/plugins.rs`, `crates/renzora_editor/src/plugins.rs` | **Generated + committed.** The static plugin lists the `add!` generator writes; CI fails if regenerating them diffs |
| `crates/renzora_plugin/src/sys/mod.rs` | The C-ABI: `INIT_SYMBOL`, `VERSION_MAJOR`/`MINOR`, `INTERFACE_PREFIX_HASHES`, and the version history |
| `crates/renzora_plugin/src/host/loader.rs` | The C-ABI plugin loader: symbol-dispatched, never drops a `Library` |
| `crates/renzora_native_plugin/` | The **native** plugin loader: scans `plugins/<name>/` directories, rebuilds stale ones, `ManuallyDrop`s every image |
| `crates/renzora_plugin_build/` | The compiler driver — reads `sdk/manifest.json` and invokes `rustc` directly. Shared by the loader and by xtask |
| `crates/renzora_dylib/`, `crates/renzora_ember_dylib/` | The shared **contract** and **UI** images. Hold no code of their own; they exist so the process-global statics in `renzora` / `renzora_ember` are singular |
| `crates/renzora_rust_script/` | `.rs` scripts — a native plugin per script, dispatched per entity with `&mut World` |
| `xtask/src/sdk.rs`, `xtask/src/native_plugin.rs` | Stage the plugin SDK; build the repo's own native plugins the way a user's machine builds an installed one |
| `crates/renzora_scripting/` | Scripting system: hooks, commands, context, declarative `ScriptExtension` |
| `crates/renzora_plugin/src/script/` | The language-backend boundary (codec, contexts, `Backend`) |
| `crates/renzora_static_plugins/` | **Generated.** The list of C-ABI plugins a lean export linked into the binary. The checked-in copy is an empty stub; `renzora_export::build::stage_static_plugins` rewrites it inside `target/export-src/`. Editing it by hand changes nothing about an export |
| `crates/renzora_lumen`, `crates/renzora_cloth` | In-workspace `rlib` plugin templates (`add!`-declared, statically linked) |
| `docker/base/Dockerfile` | Shared base image (rust + Linux deps + LLVM-19); the Rust/Bevy pin |
| `docker/<platform>/Dockerfile` | Per-platform toolchain image, `FROM base` (linux/windows/macos/ios/android/wasm) |
| `docker/build-all.sh` | In-container build orchestrator (run once per platform container) |
| `.github/workflows/docker-image.yml` | Publishes base + each <platform> image to GHCR |
| `docs/r1-alpha7/` | Current docs (edit here); `extending/plugins.md` for the plugin API |
| `docs/BEVY_0.19_MIGRATION.md` | Bevy 0.19 upgrade notes (plugin ABI will change) |
| `.github/workflows/test.yml` | CI: container test + clippy gate |
| `.github/workflows/sync-docs.yml` | Auto-publish docs to renzora.com |
</content>
</invoke>
