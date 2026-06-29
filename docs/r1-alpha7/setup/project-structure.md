# Project Structure

How the Renzora engine repository is organized — one Cargo workspace, ~187 crates auto-included by globs, and the "one binary, editor-as-removable-cdylib" layout.

Renzora is a single large Cargo workspace built on **Bevy 0.19**, where almost every feature is its own crate that registers a Bevy `Plugin`. The defining structural fact is **one engine binary** (`renzora`) that is always runtime-shaped; the editor ships as a removable cdylib bundle beside it. This page maps the repository so you can find your way around the source.

## Repository layout

```text
engine/
├── Cargo.toml              # Workspace root + the renzora_app package ([[bin]] renzora)
├── Cargo.lock
├── build.rs                # Windows icon/version resource, build hash, zstd link
├── src/
│   └── main.rs             # The single binary entry point (editor / runtime / server)
├── crates/                 # ~164 top-level crates (flat — no category subfolders)
│   ├── renzora/            # The SDK / contracts crate → ships as renzora.dll
│   ├── renzora_runtime/    # Shared engine library every binary links
│   ├── renzora_engine/     # Editor-free game core (VFS, scene IO, autoload)
│   ├── renzora_editor/     # The editor BUNDLE cdylib (links ~50 editor crates)
│   ├── renzora_editor_framework/  # Editor SDK implementation (rlib-only)
│   ├── dynamic_plugin_loader/     # dlopen + hot-reload of plugins/
│   ├── renzora_<feature>/  # one crate per feature (physics, scripting, ember, …)
│   │   └── editor/         # optional editor-only half (= renzora_<feature>_editor)
│   ├── renzora_<effect>/   # one crate per post-process effect (bloom, crt, …)
│   ├── bevy_hanabi/  bevy_hui/  bevy_mod_outline/  bevy_silk/  vleue_navigator/
│   └── bevy_oxr/           # vendored — its OWN nested workspace (not globbed in)
├── docker/                 # Dockerfile + build/add/remove/upx shell scripts
├── .cargo/
│   └── config.toml         # cargo aliases (cargo renzora, cargo runtime, …)
├── templates/              # android / ios / web PACKAGING templates (not scaffolds)
├── disabled/               # crates parked OUTSIDE the workspace (won't build)
├── assets/                 # engine default assets + test scripts
├── dist/                   # build output (arch-suffixed dirs)
└── docs/                   # in-repo engine docs (markdown)
```

> There is **no** `Makefile.toml`/cargo-make and no `_legacy_src/`, but there **is** an `xtask/` crate (behind the `cargo renzora` alias) for native builds and a `rust-toolchain.toml` that pins the Rust version for them. The container's Rust version (`1.95.0`) lives in `docker/base/Dockerfile`, kept in lockstep with `rust-toolchain.toml`; common tasks run through the `renzora` CLI or the cargo aliases in `.cargo/config.toml` (see [Build System](/docs/r1-alpha5/setup/build-system)).

## The workspace member globs

The root `Cargo.toml` auto-includes members with **globs**, so adding a new plugin only requires creating its directory — there is **no manual `members` list to edit**:

```toml
[workspace]
resolver = "2"
members = [
    ".",                          # renzora_app — the binary
    "crates/renzora",             # the SDK / contracts crate
    "crates/renzora_*",           # every renzora_* feature/effect crate
    "crates/renzora_*/editor",    # nested editor halves of dual-mode crates
    "crates/dynamic_plugin_loader",
    # Vendored Bevy ecosystem crates, listed explicitly (NOT via a bevy_* glob)
    # so bevy_oxr — its own nested workspace — is not accidentally pulled in.
    "crates/bevy_hanabi",
    "crates/bevy_hui",
    "crates/bevy_mod_outline",
    "crates/bevy_silk",
    "crates/vleue_navigator",
]
```

A few consequences worth knowing:

- The glob keys on the `renzora_` prefix. `crates/mcp_server_plugin` and `crates/websocket_plugin` exist on disk but lack that prefix, so they fall **outside every glob** and are not workspace members (they also depend on a `../editor_plugin_api` crate that no longer exists). They are orphaned, not part of the build.
- `crates/bevy_oxr` is **deliberately not globbed** — it is its own nested vendored workspace (containing `bevy_openxr`, `bevy_webxr`, `bevy_xr`, `bevy_xr_utils`).
- Crates under `disabled/` (currently `disabled/renzora_vr_editor`) sit outside `crates/`, so they never enter the workspace.

Counting it up: **~164 top-level directories under `crates/`** plus **23 nested `editor/` subcrates** → **~187 workspace crates** in total.

## The core crate layers

A handful of crates form the engine's spine. Everything else is a plugin that plugs into them.

| Crate | Crate type | Role |
|-------|-----------|------|
| `renzora` | `dylib` + `rlib` (`renzora.dll`) | The SDK / "contracts" crate. Houses the `add!` / `export_plugin_bundle!` macros, `PluginScope`/`StaticPlugin`, the post-process framework, GI contract types, and (under the `editor` feature) the editor contract registries. |
| `renzora_runtime` | rlib | Shared engine library every binary links (`init_app`, `add_default_rendering`, `add_headless_rendering`, `add_engine_plugins`). |
| `renzora_engine` | rlib | The editor-free game core — VFS, custom asset reader, scene IO, autoload, crash reporting. |
| `renzora_editor` | `cdylib` | The editor **bundle** — statically links ~50 editor-only crates and the dual-mode `/editor` subcrates, and exports a single `plugin_install_scope` entry point. |
| `renzora_editor_framework` | rlib | The editor SDK **implementation** (the boundary-crossing contract types were folded into `renzora.dll`, so this emits no dll). |
| `dynamic_plugin_loader` | rlib | dlopens plugins at startup and hot-reloads new ones dropped into `<exe>/plugins/`. |

> `renzora` is the **shared SDK library**. Shipping it as `renzora.dll` (`.so` / `.dylib`) means the host binary, the dlopen'd editor bundle, and every dynamic plugin share one compiled copy and matching `TypeId`s across the dynamic-linking boundary. `bevy` itself is shared the same way via `bevy_dylib` (`dynamic_linking` + `prefer-dynamic`).

### One binary, not three

There is exactly **one** `[[bin]]` in the whole workspace: `renzora_app` → `src/main.rs`, named `renzora`. It is the engine — editor, runtime, and dedicated server in one — chosen at **runtime**:

- The default windowed launch is the **editor** if `renzora_editor.{dll,so,dylib}` is present beside the exe, otherwise the **shipped game**.
- `--no-editor` (or `RENZORA_NO_EDITOR`) forces the game even when the bundle is present.
- `--server` runs a headless dedicated server; `--host` runs a windowed listen server.

There is no `editor` compile-time feature and no separate editor or server binary. Delete the bundle file and the same binary becomes the shipped game. See [Core Concepts](/docs/r1-alpha5/getting-started/concepts) for the full model.

## Dual-mode crates and `/editor` subcrates

Because a plugin's scope is exact (`Editor` *or* `Runtime`, never both — see below), a feature that needs to run in both places is **physically split** into two crates:

```text
crates/renzora_physics/          # lean runtime crate (package: renzora_physics)
crates/renzora_physics/editor/   # editor-only half  (package: renzora_physics_editor)
```

The runtime crate is statically linked / registered everywhere; the nested `editor/` subcrate (`renzora_<name>_editor`) is linked **only by the editor bundle**. There are **23** such `editor/` subcrates today:

> antialiasing, atmosphere, auto_exposure, bloom_effect, clouds, distance_fog, dof, ember, engine, environment_map, lighting, motion_blur, navmesh, night_stars, oit, physics, scripting, skybox, ssao, ssr, tonemapping, volumetric_fog, water.

Some editor-only features (e.g. `renzora_hierarchy`, `renzora_inspector`, `renzora_blueprint_editor`) are standalone top-level crates rather than `/editor` subcrates — they have no runtime half at all.

## Two kinds of plugin crate

| Kind | Crate type | How it ships |
|------|-----------|--------------|
| **Workspace plugin** | `rlib` | Statically linked into the binary or the editor bundle; registers via its `inventory` constructor at process start. |
| **Distribution plugin** | `cdylib` with a default-on `dlopen` feature | dlopen'd at startup, or hot-loaded when dropped into `<exe>/plugins/`. Exactly **one** `renzora::add!` per cdylib. |

Most feature crates are plain rlibs. The `dlopen` feature (gated `#[cfg(feature = "dlopen")]`) emits the `extern "C"` symbols — `plugin_create`, `plugin_scope`, `plugin_bevy_hash` — that the loader checks for ABI compatibility before loading. See [Building Plugins](/docs/r1-alpha5/extending/plugins).

## Crate categories at a glance

The 164 top-level crates break down roughly as follows:

| Count | Category | Examples |
|------:|----------|----------|
| 57 | Post-process effects | bloom_effect, crt, ascii, dof, motion_blur, vignette, dream, toon |
| 37 | Editor panels / tools | hierarchy, inspector, viewport, gizmo, material_editor, debugger |
| 18 | Rendering / GI | lumen, rt, lighting, shader, skybox, atmosphere, ssao, ssr, oit |
| 8 | Editor framework | editor, editor_framework, shell, ui, theme, keybindings, undo |
| 8 | Gameplay / sim | physics, cloth, water, navmesh, hanabi, terrain, animation |
| 7 | Asset / scene / import | import, asset_registry, rmip, rpak, scene, shape_library, preview |
| 6 | Vendored Bevy | bevy_hanabi, bevy_hui, bevy_mod_outline, bevy_silk, vleue_navigator, bevy_oxr |
| 4 | Engine runtime | runtime, engine, dynamic_plugin_loader, input |
| 4 | Support / misc | globals, spline, hot_demo, test_component |
| 3 | Core / SDK | renzora, macros, postprocess (re-export shim) |
| 3 | Scripting | scripting (Lua + Rhai), script_variables, blueprint |
| 3 | Platform | android, ios, xr |
| 2 | UI / markup | ember (markup + widgets + dock), game_ui |
| 2 | Dead / experimental | mcp_server_plugin, websocket_plugin (orphaned — outside the globs) |
| 1 | Audio / media | audio (Kira) |
| 1 | Networking | network (Lightyear 0.26) |

A few naming notes that trip people up reading the tree:

- The UI/markup system is `renzora_ember`. The former `renzora_hui` crate was merged into ember and **deleted**; the still-vendored `bevy_hui` is used only as the `.html` parser.
- `renzora_postprocess` is just a re-export shim (`pub use renzora::postprocess::*;`) — the framework lives in `renzora.dll`.
- `renzora_gauges` and the `vello`-based vector renderer were removed; gauges are now an `renzora_ember` WGSL widget.
- `renzora_dream` is a live post-process effect, not editor tooling.

## Adding a new crate

Thanks to the globs, adding a feature is mostly creating a directory:

```bash
# 1. Create the crate. The crates/renzora_* glob picks it up automatically —
#    no edit to the workspace members list is needed.
cargo new --lib crates/renzora_myfeature

# 2. (Optional) for an editor-only half, add a nested subcrate.
#    This becomes the package `renzora_myfeature_editor`, matched by
#    the crates/renzora_*/editor glob and linked only by the editor bundle.
cargo new --lib crates/renzora_myfeature/editor
```

Then in the crate's `lib.rs`, register a Bevy `Plugin` with the engine:

```rust
use bevy::prelude::*;

#[derive(Default)]
pub struct MyFeaturePlugin;

impl Plugin for MyFeaturePlugin {
    fn build(&self, app: &mut App) {
        // add_systems, register_type, insert_resource, …
    }
}

// Runtime scope by default; use add!(_, Editor) for an editor-only plugin.
renzora::add!(MyFeaturePlugin);
```

Wiring it in:

- A **runtime** crate that should be statically linked is added as a dependency of `renzora_runtime`; an **editor-only** crate is added to the `renzora_editor` bundle.
- A **distribution** (cdylib) plugin needs no dependency wiring — it is dlopen'd from `<exe>/plugins/`. The `renzora add <name> [--editor|--dylib]` helper scaffolds one for you (`--editor` = an editor-scope optional dependency; `--dylib` = a cdylib distribution plugin with `default = ["dlopen"]`; the flags are mutually exclusive).

> Use `use renzora::*;` (or `use renzora::Inspectable;`). There is **no** `renzora::prelude` module. `Inspectable`, `AppEditorExt`, and the `#[field(...)]` / `#[inspectable(...)]` attributes are behind `renzora`'s `editor` feature, so an inspector-aware crate must depend on `renzora = { ..., features = ["editor"] }`.

## What's next?

- [Build System](/docs/r1-alpha5/setup/build-system) — the `renzora` CLI, the Docker toolchain, output layout
- [Core Concepts](/docs/r1-alpha5/getting-started/concepts) — ECS, plugins, and the one-binary model
- [Building Plugins](/docs/r1-alpha5/extending/plugins) — write a workspace or distribution plugin
