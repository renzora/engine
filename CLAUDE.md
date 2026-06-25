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

All real work goes through the `renzora` CLI, which drives a pinned Docker
container. It is a **separately published tool**, not part of this workspace.

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

## 2. Building & testing — Docker is the ONLY supported path

**Do ALL building and testing in Docker via the `renzora` CLI. Do not use the
local/native toolchain for builds or tests.**

The reason is a hard limit, not a preference: the shared `renzora` dylib plus
the full plugin set exceeds the **65,535 exported-symbol cap** of the Windows PE
format. Native MSVC `link.exe` refuses it; the container's `rust-lld` does not.
So:

- ✅ `cargo check` natively / via the editor — **allowed** (it doesn't link). This
  is the fast local gate while editing.
- ✅ `renzora check`, `renzora test`, `renzora build`, `renzora run` — the real
  builds, all inside the container.
- ❌ Native `cargo build` / `cargo test` of the workspace — **will fail to link.**
  Don't propose it, don't try to "fix" the link error by stripping the dylib.

Pinned toolchain (single source of truth = `docker/base/Dockerfile`): **Rust
1.95.0**, **Bevy 0.19**. The base image is the foundation every platform image
builds `FROM`, so the Rust version lives there (a bump cascades to all
platforms — see §3). CI (`.github/workflows/test.yml`) runs `cargo test` + `cargo
clippy -D warnings` in the `base` image, excluding the vendored `bevy_*` /
`vleue_navigator` crates. Keep clippy green; the vendored crates must stay
excluded.

---

## 3. Plugin ABI — the `bevy_dylib` it links

Community/distribution plugins are `dlopen`'d at runtime and share **one
compiled `bevy_dylib`** with the host. The ABI guard is `plugin_bevy_hash()`,
exported by every plugin and the editor bundle, and checked by
`dynamic_plugin_loader` before a plugin is allowed to touch the `App`. It returns
`TypeId::of::<bevy::ecs::world::World>()` (transmuted to `[u64; 2]`); the loader
computes its own `TypeId::of::<World>()` and **rejects any plugin whose value
differs**. If a plugin linked a different `bevy_dylib`, its `World` would be a
distinct type and every component/resource crossing the boundary would mismatch,
so the loader refuses it. There is no separate hash crate, no baked
`RENZORA_ABI_HASH`, and no `abi.lock` — the guard is the `World` `TypeId` itself,
computed by the compiler, so it can never go stale relative to the build.

### What actually decides compatibility

Two layers, both real:

1. **The `bevy_dylib` filename — the OS-enforced gate.** Cargo names the shared
   library `bevy_dylib-<metadata>.dll` (e.g. `bevy_dylib-0acc7716eed29df6.dll`),
   where `<metadata>` is cargo's own hash of the *full* build: package id, bevy
   feature set, profile, `RUSTFLAGS`, target, and rustc. The host `renzora.exe`
   imports that exact filename; a plugin that shares bevy must import the *same*
   filename. Build a plugin in a different environment and it imports a
   differently-named `bevy_dylib` that isn't beside the exe → the OS loader fails
   it **before `plugin_bevy_hash()` is even called**. This is why **all real
   building happens in Docker** (the one canonical flag/env set): only then do the
   host and every plugin produce the same `bevy_dylib-<metadata>` and the same
   `World` `TypeId`.
2. **The `World` `TypeId` guard — the clean rejection.** When two `bevy_dylib`s
   *do* coexist (e.g. a plugin shipping its own), the filename gate can't catch
   it; the `TypeId` check does, and turns a cryptic loader failure into an
   "incompatible bevy version" rejection. Mitigation that makes this rare: there
   is exactly **one** `bevy_dylib`, beside the exe — never ship one inside
   `plugins/`.

### The model: a fixed ABI per canonical release, source-first for everyone else

The ABI is whatever the **canonical editor release** (the prebuilt binary, built
from a fixed commit in Docker) compiled. That release's `bevy_dylib`/`World`
`TypeId` is its fixed ABI, frozen in time. So:

- **Marketplace plugins** are built *by the marketplace* against a canonical
  release's ABI, so a downloaded prebuilt always matches that editor. A new
  release = a new ABI = the marketplace rebuilds plugins for it.
- **Engine developers build plugins from source**, not from a prebuilt dylib.
  `renzora run`/`build` compiles every in-workspace plugin in the dev's own Docker
  env, so they share that build's `bevy_dylib` by construction — whatever bevy
  features the dev added or removed. Source-first means a custom feature set just
  works; there is no hash to match.

Because of this, the precise value of the ABI tag doesn't matter and needs no
pinning: matching is guaranteed by *building in the same Docker env* (prebuilts)
or *building from source* (everything else), and the OS linker + `World` `TypeId`
enforce it.

### Feature changes still move the ABI — `trace_tracy` in particular

Changing the bevy feature set recompiles `bevy_dylib` and moves the ABI (the
filename metadata and the `World` `TypeId` both shift), so every existing
prebuilt plugin for the old ABI stops loading until rebuilt — fine under the
model above (marketplace rebuilds; devs rebuild from source). One feature to keep
**out** of the normal build: `trace_tracy`. Bevy installs its Tracy layer in
`LogPlugin` at boot whenever that feature is compiled in, with no runtime
off-switch, so it would arm Tracy (and grow RAM) on every launch. Tracy is opt-in
via the gated `renzora_tracy` plugin (frame marks + diagnostic plots, started
only on its Dev-Mode + toggle gate); per-system CPU zones need a dedicated
profiling build that re-adds `trace_tracy` and so moves the ABI.

---

## 4. Versioning & documentation

- **Current dev version: `r1-alpha6`.** From now on, **only edit
  `docs/r1-alpha6/`.** `docs/r1-alpha5/` is released and **frozen** — do not
  mirror changes into it. Top-level non-versioned `docs/*.md` are still fair game.
- **Always update the docs after adding or changing a feature.** Stale docs are
  treated as a bug. If you ship a feature (new scripting function, new inspector
  field, new plugin capability, new editor panel), update the matching page under
  `docs/r1-alpha6/` in the same change.
- Docs are also published at <https://renzora.com/docs>. Pushing `docs/r1-alpha*`
  changes to `main` auto-publishes via `.github/workflows/sync-docs.yml` (rsync
  into the website repo, which redeploys). You do not copy anything by hand.

`docs/r1-alpha6/` sections include: `getting-started`, `setup`, `scripting`,
`api`, `editor`, `editor-dev`, `engine-core`, `rendering`, `extending`,
`exporting`, `packaging`, `multiplayer`, `marketplace`, `platform-api`,
`contributing`.

---

## 5. Architecture (orientation)

- **`crates/renzora` is the contract crate** (`crate-type = ["dylib", "rlib"]`,
  zero deps beyond Bevy + serde). It holds the shared types, events, components,
  resources, the post-process framework, and the editor contract (`editor`
  feature). **Every boundary-crossing type lives here** so all crates and
  dlopen'd plugins agree on one `TypeId`.
- **Plugin registry:** every plugin self-registers with `renzora::add!(MyPlugin
  [, Editor|Runtime] [, priority = N])`. The macro emits both an `inventory`
  entry (static linking, all platforms) and — gated on the *calling crate's*
  `dlopen` feature — the `extern "C"` FFI trio (`plugin_create`, `plugin_scope`,
  `plugin_bevy_hash`) used by the dynamic loader. See
  `crates/renzora/src/plugin_meta.rs`.
- **Editor / runtime split.** A plugin's scope is exclusively `Runtime` or
  `Editor` (there is no "both"). Runtime plugins run in the editor viewport AND
  the shipped game; Editor plugins run only when the editor bundle is present.
  A feature needing editor tooling on top of runtime behaviour ships **two**
  plugins. The lean pattern is `renzora_<name>` (runtime, in the binary) +
  `renzora_<name>/editor/` (`renzora_<name>_editor`, linked only by the editor
  bundle).
- **The editor is a removable `cdylib` bundle** (`renzora_editor`) loaded once
  from beside the exe via `load_bundle`. Present → editor mode; absent → game.
- **Building also builds the runtime** by design — an editor build always
  produces the runtime too. Don't propose editor-only scoping of a build.

---

## 6. Writing plugins

**Before creating or modifying a plugin, ALWAYS research the plugin API first.**
Read `docs/r1-alpha6/extending/plugins.md` and `crates/renzora/src/plugin_meta.rs`,
and look at an existing distribution plugin (`renzora_lumen`, `renzora_cloth`)
as a template. Use `renzora add <name>` to scaffold.

Principles (in priority order):

1. **Make plugins as modular as possible.** One plugin = one cohesive feature.
   Prefer a self-contained `cdylib` distribution plugin over wiring a feature
   deep into the host.
2. **Refrain from linking crates as much as possible.** Minimize a plugin's
   dependency on other `renzora_*` crates. When a type must cross the plugin↔host
   boundary, **move that type into the `renzora` contract dylib** rather than
   depending on the crate that defines it. This is the established pattern (GI
   settings, etc. live in `renzora`, not in their plugin).
3. **Exactly one `add!` per distribution cdylib** (the FFI symbols are
   unmangled and would collide). Multi-plugin engine crates stay rlibs and rely
   on the `inventory` path only. Bundles use `export_plugin_bundle!`.
4. A plugin that mutates files in parallel with others, or that must initialize
   before another, is the rare case — most ordering should use Bevy's own system
   sets, not plugin `priority`.

---

## 7. Extending the scripting API

Scripts are **Lua and Rhai** (dual backend, `crates/renzora_scripting`). Scripts
live in `<project>/scripts/*.lua|*.rhai`, attach to entities via
`ScriptComponent`, and run through hooks: `on_ready`, `on_update`, `on_rpc`,
`on_ui`, `on_animation_event`, `on_http`, `on_player_joined`, `on_player_left`.

**When writing scripts, refer to the scripting API first**
(`docs/r1-alpha6/scripting/` + `docs/r1-alpha6/api/scripting.md`, and the
backends in `crates/renzora_scripting/src/backends/`).

**If a script needs a function that doesn't exist yet:**

1. Tell the user the function isn't in the API and explain how to proceed.
2. If feasible, **extend the scripting API** rather than working around it.
3. **Always prefer registering new script functions from the owning `renzora`
   plugin itself**, via the `ScriptExtension` trait — not by bolting them into
   the core backend. The domain crate implements `ScriptExtension`
   (`register_lua_functions` / `register_rhai_functions` / `populate_context`)
   in its own `src/script_extension.rs`, then registers it in its plugin
   `build()`:

   ```rust
   let mut extensions = app.world_mut().get_resource_or_insert_with(
       renzora_scripting::extension::ScriptExtensions::default,
   );
   extensions.register(my_crate::script_extension::MyScriptExtension);
   ```

   Both backends then expose the function automatically. See
   `renzora_animation`, `renzora_physics`, `renzora_navmesh`,
   `renzora_network`, `renzora_audio`, `renzora_game_ui` for real examples.
4. **Update `docs/r1-alpha6/` for the new function** (see §4).

Core/engine-wide primitives (`set_position`, `play_sound`, `spawn_entity`, the
reflection `set`/`get`/`set_on`, …) live in the backends' `register_api()`.
Domain functions belong in that domain crate's extension.

---

## 8. Code conventions

- **Comment the WHY, not the what.** This codebase's hallmark is doc-comments
  (`//!` module, `///` item) that explain *why* the code is shaped this way, what
  edge case it handles, and what previously went wrong. Match that density and
  voice. Don't add narration that just restates the code.
- **Module layout:** `lib.rs` (module doc + plugin), `systems.rs` (systems),
  `native.rs` (bevy_ui / native editor UI). Types → systems → helpers.
- **Naming:** `PascalCase` types, `snake_case` fns/modules, `SCREAMING_SNAKE`
  consts. Crates are `renzora_<name>`.
- Follow Bevy ECS idioms. Avoid `unwrap()` in production paths. Default rustfmt.
- **Commits:** Conventional style — `feat:`, `fix:`, `docs:`, `refactor:`,
  `security:`, with optional scope, e.g. `feat(plugin): …`. Imperative mood,
  no trailing period.

---

## 9. Best practices (audit summary)

- **Trust the constraints.** Docker-only builds, the single shared `bevy_dylib`,
  the one-`TypeId` contract crate, and the frozen-vs-current docs split are all
  load-bearing. Work *with* them.
- **`cargo check` to iterate, `renzora test`/`renzora check` to verify.** Never
  claim something builds/passes based on a native build — it can't link natively.
- **Put shared types in `renzora`.** Any type two crates (or a plugin and the
  host) both need crosses the dylib boundary and must have one definition.
- **Two plugins, not one "both" plugin,** when a feature needs editor tooling +
  runtime behaviour.
- **Plugin ABI is the shared `bevy_dylib`** — guaranteed by building in the same
  Docker env (prebuilts) or from source (everything else); the `World` `TypeId`
  guard rejects mismatches. There is no pinned hash to maintain.
- **Docs are part of "done."** A feature without its `docs/r1-alpha6/` update is
  unfinished.
- **Verify before contradicting the user** about working-tree state; check the
  actual files.

---

## 10. Key file map

| Path | What it is |
|---|---|
| `crates/renzora/` | Contract dylib: shared types/events/components, editor contract |
| `crates/renzora/src/plugin_meta.rs` | `add!` / `export_plugin_bundle!`, `PluginScope` |
| `crates/dynamic_plugin_loader/src/lib.rs` | dlopen loader + `World` `TypeId` ABI gate + hot-reload |
| `crates/renzora_scripting/` | Lua + Rhai backends, `ScriptExtension` trait |
| `crates/renzora_lumen`, `crates/renzora_cloth` | Distribution `cdylib` plugin templates |
| `docker/base/Dockerfile` | Shared base image (rust + Linux deps + LLVM-19); the Rust/Bevy pin |
| `docker/<platform>/Dockerfile` | Per-platform toolchain image, `FROM base` (linux/windows/macos/ios/android/wasm) |
| `docker/build-all.sh` | In-container build orchestrator (run once per platform container) |
| `.github/workflows/docker-image.yml` | Publishes base + each <platform> image to GHCR |
| `docs/r1-alpha6/` | Current docs (edit here); `extending/plugins.md` for the plugin API |
| `docs/BEVY_0.19_MIGRATION.md` | Bevy 0.19 upgrade notes (plugin ABI will change) |
| `.github/workflows/test.yml` | CI: container test + clippy gate |
| `.github/workflows/sync-docs.yml` | Auto-publish docs to renzora.com |
</content>
</invoke>
