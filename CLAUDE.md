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

## 3. Plugin ABI — the `RENZORA_ABI_HASH`

Community/distribution plugins are `dlopen`'d at runtime and share **one
compiled `bevy_dylib`** with the host. The ABI guard is `RENZORA_ABI_HASH`,
exported by every plugin as `plugin_bevy_hash()` and checked by
`dynamic_plugin_loader` before a plugin is allowed to touch the `App`. If the
host and a plugin were built against an incompatible bevy, every
component/resource crossing the boundary would be a *different* type, so the
loader rejects the mismatch.

### The hash is computed explicitly from THREE inputs — nothing else

`RENZORA_ABI_HASH` is a stable hash of exactly:

1. **bevy version** — the resolved version from `Cargo.lock` (e.g. `0.19.0`).
2. **rust toolchain** — `rustc` release + commit-hash (target triple deliberately
   excluded, so the hash is the same on every platform).
3. **the curated bevy feature set** — the `[workspace.dependencies] bevy.features`
   list, sorted. (Only *bevy's* features matter; a plugin's own unrelated cargo
   features don't change bevy's compilation, so they're ignored.)

A change to none of those three cannot move the hash; a change to any of them
moves it deterministically. This is implemented in the **`abi_hash`** crate
(`crates/abi_hash`): `renzora`'s `build.rs` calls it to bake `RENZORA_ABI_HASH`
(plus `RENZORA_ABI_INPUTS`) into the shared `renzora` dylib, so the host and
every in-tree plugin compute the same value. All three inputs are pinned in
**`docker/base/Dockerfile`** (rustc + the bevy feature set) and the workspace
`Cargo.toml` / `Cargo.lock` (bevy version) — a per-platform Dockerfile edit
cannot move the hash.

### What `RENZORA_ABI_HASH` does and does NOT guarantee

It is a **persistent identifier of the ABI configuration** (bevy version + rustc +
feature set) — identical on every machine and environment for the same three
inputs. It deliberately does NOT capture the build flags/profile/env, so it can't
detect those. That's by design: real binary compatibility (the `bevy_dylib` cargo
metadata hash, which governs the actual exported symbols) ALSO depends on build
flags and is therefore **environment-specific**. Empirically, a local native
`--profile dist` build and the Docker cross-build produce the *same*
`RENZORA_ABI_HASH` but *different* `bevy_dylib` symbol hashes (different
`RUSTFLAGS`/env). So **all real building must happen in Docker** (the one
canonical flag/env set) — plugins built in Docker against a given release are
mutually loadable; a plugin from a different build environment (or a different
profile) is not, and the OS linker rejects it. `RENZORA_ABI_HASH` is the stable
label for "which ABI config"; reproducible Docker builds are what make plugins of
that config actually interchangeable.

### The pin lives in `abi.lock` — verify it, don't eyeball a dylib filename

`abi.lock` (repo root, committed) records the three inputs + the resulting hash.
The `abi_hash` CLI is the tool:

```
cargo run -p abi_hash -- show     # print current inputs + hash
cargo run -p abi_hash -- verify   # diff current inputs vs abi.lock (exit 1 on drift)
cargo run -p abi_hash -- update   # re-pin abi.lock after an INTENDED change
```

**On every `renzora run` / `renzora build` (and in CI):** run `abi_hash verify`.

- **Stable** → the plugin ABI is unchanged; all existing distribution plugins
  keep loading.
- **Changed** → `verify` prints the exact cause (`bevy version: X → Y`,
  `rust toolchain: … → …`, or `feature added/removed: ±name`). **Stop and inform
  the user**: every existing distribution plugin will fail the ABI check until
  rebuilt. If the change is intended, re-pin with `abi_hash update`; otherwise
  investigate (an unintended move now points at a specific input, not a mystery).

At load time the loader's rejection log likewise diffs the engine's inputs
against the plugin's (via the plugin's `plugin_abi_info()` export), so a failed
load says *which* input differs.

### Current state

`abi.lock` reflects the **current** workspace, which has the profiling/experimental
features `trace_tracy`, `meshlet`, `meshlet_processor`, and `bevy_solari` enabled.
Toggling any of those will (correctly and visibly) move the hash — `verify` will
name the feature. A shipping build that drops `trace_tracy`/`meshlet` is expected
to re-pin once. There is no per-platform hash divergence anymore (the triple is
not an input).

The current pinned hash is:

```
STABLE ABI HASH: 3bbffe3e96cbadf859829a69319e3f75
```

`abi.lock` is the source of truth (`abi_hash show` prints it with its inputs).
The hash holds across source/lockfile/profile/target changes and moves only on
the three real inputs.

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
- **Keep the ABI hash pinned** (`eb7a819cba90e0c8`) and flag any unexpected drift.
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
| `crates/dynamic_plugin_loader/src/lib.rs` | dlopen loader + ABI hash gate + hot-reload |
| `crates/abi_hash/` | Computes/verifies `RENZORA_ABI_HASH` (bevy ver + rustc + bevy features); `renzora` build-dep + CLI (`show`/`verify`/`update`) |
| `abi.lock` | Pinned ABI inputs + hash; `abi_hash verify` diffs against it |
| `crates/renzora_scripting/` | Lua + Rhai backends, `ScriptExtension` trait |
| `crates/renzora_lumen`, `crates/renzora_cloth` | Distribution `cdylib` plugin templates |
| `docker/base/Dockerfile` | Shared base image (rust + Linux deps + LLVM-19); the Rust/Bevy pin |
| `docker/<platform>/Dockerfile` | Per-platform toolchain image, `FROM base` (linux/windows/macos/ios/android/wasm) |
| `docker/build-all.sh` | In-container build orchestrator (run once per platform container) |
| `.github/workflows/docker-image.yml` | Publishes base + each <platform> image to GHCR |
| `docs/r1-alpha6/` | Current docs (edit here); `extending/plugins.md` for the plugin API |
| `docs/BEVY_0.19_MIGRATION.md` | Bevy 0.19 upgrade notes (ABI hash will change) |
| `.github/workflows/test.yml` | CI: container test + clippy gate |
| `.github/workflows/sync-docs.yml` | Auto-publish docs to renzora.com |
</content>
</invoke>
