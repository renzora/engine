# Code Style

Coding conventions for contributing to the Renzora engine workspace — formatting, lints, naming, and how crates are laid out.

## Formatting

Renzora uses **rustfmt defaults**. There is no top-level `rustfmt.toml` (the only one in the tree belongs to a vendored crate, `crates/bevy_silk`), so the standard style applies everywhere. Format before you commit, running rustfmt through the toolchain container:

```bash
renzora shell -- cargo fmt --all
```

> CI runs two jobs — **tests** and **Clippy** (`.github/workflows/test.yml`); there is no separate `cargo fmt --check` gate. Run rustfmt anyway so diffs stay clean and reviewable.

## Lints and Clippy

### Workspace lints

The workspace defines exactly one lint rule in the root `Cargo.toml`, and every first-party crate opts into it:

```toml
# root Cargo.toml
[workspace.lints.rust]
unexpected_cfgs = { level = "allow", check-cfg = ['cfg(feature, values("dlopen"))'] }
```

```toml
# each crate's Cargo.toml
[lints]
workspace = true
```

That single `allow` exists because `renzora::add!` gates its FFI exports behind `#[cfg(feature = "dlopen")]`, and that `cfg` is evaluated in the **calling** crate. Workspace plugins that have no `dlopen` feature would otherwise warn on every `add!` invocation — the `check-cfg` entry tells the compiler the feature is expected-but-unset. **Keep the `[lints] workspace = true` block in new crates** so they inherit this.

### Clippy

CI denies all warnings. Lint locally with `renzora check`, which runs the following inside the `ghcr.io/renzora/linux` container:

```bash
cargo clippy --workspace --no-deps \
    --exclude bevy_gauge \
    --exclude bevy_hanabi \
    --exclude bevy_mod_outline \
    --exclude bevy_silk \
    --exclude vleue_navigator \
    --exclude bevy_mod_openxr \
    --exclude bevy_mod_xr \
    --exclude bevy_xr_utils \
    -- -D warnings \
    -A clippy::too_many_arguments \
    -A clippy::type_complexity
```

- `--no-deps` keeps Clippy off the vendored third-party crates that leak in as path-dependencies; the `--exclude` flags drop the vendored Bevy-ecosystem crates from the lint scope (they re-test upstream code against our Bevy version — noise, not signal).
- `clippy::too_many_arguments` and `clippy::type_complexity` are **allowed** because they are inherent to Bevy systems and queries (Bevy allows them too). Do not fight those two.
- Everything else is `-D warnings`. Don't paper over a real lint with `#[allow(...)]` — fix it, or add a one-line comment explaining why the allow is correct.

## Tests

Tests run with the same first-party scope as Clippy, excluding the vendored crates. Run them with `renzora test`, which wraps:

```bash
cargo test --workspace \
    --exclude bevy_gauge --exclude bevy_hanabi --exclude bevy_mod_outline \
    --exclude bevy_silk --exclude vleue_navigator \
    --exclude bevy_mod_openxr --exclude bevy_mod_xr --exclude bevy_xr_utils
```

New first-party crates are covered automatically because the workspace globs pick them up (see below) and `--workspace` runs their suites.

## Naming conventions

Standard Rust naming, plus the `renzora_` crate prefix:

| Item | Convention | Example |
|------|-----------|---------|
| Functions, methods | `snake_case` | `spawn_entity()` |
| Variables, fields | `snake_case` | `player_health` |
| Types, traits, enums | `PascalCase` | `PhysicsPlugin`, `ScriptComponent` |
| Enum variants | `PascalCase` | `PluginScope::Runtime` |
| Constants, statics | `SCREAMING_SNAKE_CASE` | `MAX_CLIENTS` |
| Modules, files | `snake_case` | `scene_io.rs` |
| First-party crates | `snake_case`, `renzora_` prefix | `renzora_physics` |

- Plugin types end in `Plugin` (`PhysicsPlugin`, `MarkupPlugin`, `LumenPlugin`).
- The editor half of a dual-mode crate is named `renzora_<name>_editor` and its plugin is `<Name>EditorPlugin` (e.g. `renzora_physics_editor` / `PhysicsEditorPlugin`).
- Vendored crates keep their upstream names (`bevy_hui`, `bevy_silk`, `vleue_navigator`) — don't rename them to fit the prefix.

## Workspace and crate layout

Crates live **flat** under `crates/` — there is no `crates/core/` (or any other) subgrouping. The root `Cargo.toml` auto-includes members via globs, so adding a crate never means editing `workspace.members`:

```toml
members = [
    ".",
    "crates/renzora",
    "crates/renzora_*",        # every plugin crate
    "crates/renzora_*/editor", # nested editor halves of dual-mode crates
    "crates/dynamic_plugin_loader",
    # vendored bevy_* crates listed explicitly (bevy_oxr is deliberately excluded)
]
```

Many features are split into a **dual-mode pair**:

- `crates/renzora_<name>/` — the lean runtime crate (an `rlib`), no editor code.
- `crates/renzora_<name>/editor/` — the editor-only half, package name `renzora_<name>_editor`, linked only by the editor bundle.

To add a crate, just create its directory; the glob picks it up. Check a single crate from the workspace root with:

```bash
renzora check -p renzora_<name>
```

> See [Project Structure](/docs/r1-alpha5/setup/project-structure) for the full layout and [Building From Source](/docs/r1-alpha5/setup/building-from-source) for the `renzora` CLI commands that drive editor/runtime builds.

## Inside a crate

A typical crate keeps the plugin and public API in `lib.rs` and splits the rest by responsibility — one concept per file:

```text
crates/renzora_physics/src/
├── lib.rs          # Plugin struct, public API, re-exports
├── components.rs   # Component definitions
├── systems.rs      # System functions
├── resources.rs    # Resource definitions
└── events.rs       # Event definitions
```

- `lib.rs` exports the public surface; internal helpers stay `pub(crate)` or private.
- Split large files by responsibility rather than letting one module sprawl.
- Order `use` imports `std` → external crates → internal modules.

### Dependencies

Pull shared crates from the workspace and depend on the SDK with default features off:

```toml
[package]
name = "renzora_myplugin"
version = "0.1.0"
edition = "2021"

[dependencies]
bevy = { workspace = true }                                # one shared Bevy
renzora = { path = "../renzora", default-features = false } # SDK contracts

[lints]
workspace = true
```

- `bevy` (and `log`) come from `[workspace.dependencies]` — use `{ workspace = true }`, don't pin your own version.
- All first-party crates are `edition = "2021"`.
- Add `features = ["editor"]` to the `renzora` dependency **only** when you derive `Inspectable`, use `#[renzora::post_process(...)]`, or touch the inspector/toolbar/shortcut registries — those live behind the SDK's `editor` feature (default features are empty).
- Import the SDK with `use renzora::*;` (or pull individual items like `use renzora::Inspectable;`). **There is no `renzora::prelude`** — it does not exist and will not compile. For ECS types use Bevy's own `use bevy::prelude::*;`.

## Registering a plugin

Wire a plugin in with a single macro — there is no central plugin list and no manual `app.add_plugins(...)` call:

```rust
use bevy::prelude::*;

#[derive(Default)]
pub struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) { /* ... */ }
}

renzora::add!(MyPlugin);            // Runtime scope (default)
// renzora::add!(MyEditorTool, Editor);
// renzora::add!(MyFoundation, Runtime, priority = -100);
```

`PluginScope` is exactly `{ Editor, Runtime }` with equality matching — there is **no "both" scope**. A feature that needs editor tooling on top of runtime behaviour ships two plugins (e.g. `GameUiPlugin` + `GameUiEditorPlugin`). See [Building Plugins](/docs/r1-alpha5/extending/plugins) for the full model.

## Error handling and panics

| Context | Approach |
|---------|----------|
| Library crates with a real error domain | A `thiserror`-derived error enum (`renzora_engine`, `renzora_import`, `renzora_rmip` do this) |
| Fallible functions | Return `Result<_, _>` and propagate with `?` |
| Bevy systems | Log and continue — `warn!()` / `error!()`, never panic to unwind a frame |
| Genuinely infallible operations | `.expect("clear reason")`, never a bare `.unwrap()` in shipping code |

> `anyhow` is **not** a workspace convention here — it appears in only one (vendored) crate. Prefer a typed `thiserror` error for libraries, or Bevy's own `Result`/`BevyError` for system-level fallibility.

```rust
// Good: log and continue inside a system
if let Err(e) = save_scene(&scene) {
    error!("failed to save scene: {e}");
}

// Avoid: unwrap without context in non-test code
let file = std::fs::read_to_string(&path).unwrap(); // don't
```

> ⚠️ **Don't panic in `Plugin::build`.** The editor bundle installs each plugin inside `catch_unwind` and nothing unwinds across the FFI boundary — a panic there is caught and counted, but it silently drops your plugin. Surface recoverable problems through the engine's `runtime_warnings` ring buffer (in `renzora.dll`) instead.

## Unsafe code

- Avoid `unsafe` unless it is genuinely necessary — the main legitimate place is the FFI the `add!`/`export_plugin_bundle!` macros generate (the `extern "C"` plugin exports the loader dlopens) and GPU interop.
- Every `unsafe` block needs a `// SAFETY:` comment stating the invariant that makes it sound.
- Prefer a safe abstraction over leaving `unsafe` at the call site.

```rust
// SAFETY: `create_fn` is the `plugin_create` export of a library whose
// `plugin_bevy_hash` already matched the host's RENZORA_ABI_HASH, so the
// returned `*mut dyn Plugin` was built against this exact bevy ABI.
let plugin: Box<dyn Plugin> = unsafe { Box::from_raw(create_fn()) };
```

## Documentation

- `///` doc comments on public items; the first line is a one-line summary (shown in IDE hover).
- Skip private/internal items unless the logic is non-obvious.

```rust
/// Synchronizes physics body transforms back onto Bevy `Transform`s.
///
/// Runs after the physics step; handles interpolation when the physics
/// tick rate differs from the frame rate.
fn sync_physics_transforms(mut query: Query<(&mut Transform, &RigidBody)>) { /* ... */ }
```

## Commit messages

The repo uses **`<scope>: <summary>`**, where the scope is the crate or subsystem touched and the summary is a short, lower-case, imperative phrase:

```text
shader: fix unnecessary_sort_by lint in MaterialPerf::snapshot
ember: guard gauge/chart/waveform at both add sites
editor: completely remove the renzora_gauges plugin
physics: split into lean runtime + editor crate
ci: re-enable renzora_shader in tests + clippy
docs: refresh to post-merge state
```

Common scopes: a crate short-name (`physics`, `ember`, `shader`, `animation`), or an area (`editor`, `engine`, `scene`, `export`, `plugins`, `ci`, `docs`, `README`). Keep one logical change per commit.

## Pull requests

- One logical change per PR.
- Describe **why**, not just what.
- Add or update tests for new behaviour and bug fixes; make sure `renzora test` and `renzora check` pass before requesting review.
- Keep vendored `bevy_*` / `vleue_navigator` crates out of scope — they are upstream copies, not first-party code.

## What's next?

- [Building From Source](/docs/r1-alpha5/setup/building-from-source) — the `renzora` CLI commands the workflow actually uses
- [Project Structure](/docs/r1-alpha5/setup/project-structure) — how the workspace's ~187 crates are organized
- [Building Plugins](/docs/r1-alpha5/extending/plugins) — the `renzora::add!` plugin model in depth
