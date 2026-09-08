# Testing

How to write and run tests for the Renzora engine workspace — natively while you iterate, and in the pinned container when you need to match CI exactly.

## Running tests natively

Per-crate tests link and run natively on every host platform, including Windows, and this is the fast way to iterate — an order of magnitude quicker than a container round-trip:

```bash
cargo test --profile dist -p renzora_physics
cargo test --profile dist -p renzora_ember parse_templates   # one test by substring
```

> **Always pass `--profile dist`.** A bare `cargo test` builds the `dev` profile and creates a second full artefact tree; this workspace is far too large for two of them.

> **`cargo test --workspace` does not pass natively**, and that is expected rather than something to fix. It builds *example* targets, and two vendored XR crates ship examples that never got a Bevy 0.19 rename. CI never sees this because it excludes those crates. Test per-crate, or use `renzora test` below.

## Running tests the way CI does

`renzora test` runs the suite inside the pinned toolchain container (it forwards to `cargo test`, so the usual selectors still work):

```bash
# All first-party crates
renzora test

# A single crate
renzora test --package renzora_net

# A single test by name (substring match)
renzora test dynamic_2d_body_blocked_by_static_collider

# Show stdout / println! from passing tests
renzora test -- --nocapture
```

> `renzora test` wraps `cargo test --workspace` inside the pinned toolchain container, so the suite runs against the exact rustc and libs CI uses. (CI itself invokes `cargo test` directly inside the same image — see [CI](#what-ci-runs).)

### Excluding the vendored crates

A bare `renzora test` (which runs `cargo test --workspace`) also tries to run the test suites of the vendored Bevy-ecosystem crates (`bevy_*`, `vleue_navigator`). Those are third-party code copied into the tree; running them just re-tests upstream against our Bevy version and breaks on API drift. CI excludes them, and you can too:

```bash
renzora test \
  --exclude bevy_gauge \
  --exclude bevy_hanabi \
  --exclude bevy_mod_outline \
  --exclude bevy_silk \
  --exclude vleue_navigator \
  --exclude bevy_mod_openxr \
  --exclude bevy_mod_xr \
  --exclude bevy_xr_utils
```

New first-party crates stay covered automatically — they match the `crates/renzora_*` workspace globs and are picked up by `--workspace`.

## Unit tests

Standard Rust unit tests live in a `#[cfg(test)]` module in the same file as the code they cover:

```rust
#[derive(Component)]
struct Health {
    current: f32,
    max: f32,
}

fn clamp_health(h: &mut Health) {
    h.current = h.current.min(h.max);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_clamps_to_max() {
        let mut health = Health { current: 150.0, max: 100.0 };
        clamp_health(&mut health);
        assert_eq!(health.current, 100.0);
    }
}
```

## Testing Bevy systems

Most engine logic is a Bevy system, so the real pattern (used throughout the workspace) is to build an `App` with `MinimalPlugins`, run a frame with `app.update()`, then read state back out of the `World`:

```rust
use bevy::prelude::*;

#[derive(Component)]
struct Health(f32);

fn regenerate_health(mut query: Query<&mut Health>) {
    for mut h in &mut query {
        h.0 += 1.0;
    }
}

#[test]
fn health_regenerates_each_frame() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_systems(Update, regenerate_health);

    let entity = app.world_mut().spawn(Health(50.0)).id();

    app.update();

    let health = app.world().get::<Health>(entity).unwrap();
    assert!(health.0 > 50.0, "health should have regenerated");
}
```

`MinimalPlugins` gives you the scheduler and time without a window or GPU, so these tests run headless in CI. Add `AssetPlugin::default()` when a test needs the `AssetServer` (see [Integration tests](#integration-tests)).

### Verifying a function is a valid system

```rust
use bevy::ecs::system::assert_is_system;

#[test]
fn signatures_are_valid_systems() {
    assert_is_system(regenerate_health);
}
```

## Testing with `World` directly

For lower-level ECS tests you can skip the `App` and drive a `World` yourself:

```rust
use bevy::prelude::*;

#[derive(Component)]
struct Enemy;
#[derive(Component)]
struct Health(f32);

#[test]
fn query_filters_enemies() {
    let mut world = World::new();
    world.spawn(Health(100.0));
    world.spawn((Health(50.0), Enemy));
    world.spawn((Health(75.0), Enemy));

    let mut query = world.query_filtered::<&Health, With<Enemy>>();
    assert_eq!(query.iter(&world).count(), 2);
}
```

## Integration tests

Cross-crate tests go in a crate's `tests/` directory (`crates/<crate>/tests/*.rs`). They run as separate binaries against the crate's public API. The workspace ships a few real ones worth copying from:

| Test file | What it proves |
|---|---|
| `crates/renzora_plugin/tests/abi_order.rs` | The C-ABI interface table's field order and prefix hashes match what plugins negotiate against — the check that catches a "minor append" that actually inserted into the middle. |
| `crates/renzora_net/tests/round_trip.rs` | The whole HTTP chain end to end — `fetch` on a background thread → queue → frame pump → an `extern "C"` call into a backend → events → back to the parked thread — against a table-driven fake backend, so there are no sockets to flake. |
| `crates/renzora_physics/tests/avian2d_collision.rs` | A dynamic 2D body driven by `LinearVelocity` is blocked by a static collider-only entity — the shape a tilemap's merged colliders take. A regression here is "the player walks through walls". |
| `crates/renzora_bsn/tests/raw_roundtrip.rs` | The scene format survives a serialize → deserialize round-trip. |
| `crates/renzora_ember/tests/parse_templates.rs` | Every shipped `.html` UI template parses through bevy_hui's parser — markup syntax errors are caught in CI without a GPU. |
| `crates/renzora_ember/tests/inspector_writeback.rs` | The inspector → `.html` writeback round-trip patches the source file on disk and keeps the span cache coherent. |

### Driving a fixed-timestep system

Anything on the fixed-timestep schedule — physics above all — needs two things a bare `MinimalPlugins` app doesn't give you. `avian2d_collision.rs` shows both:

```rust
use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use renzora_physics::PhysicsPlugin;

fn physics_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        // Avian's tree/spatial-query systems take its diagnostics resources,
        // which it only inserts when bevy's diagnostics are present.
        bevy::diagnostic::DiagnosticsPlugin,
        bevy::asset::AssetPlugin::default(),
        TransformPlugin,
    ));
    app.init_asset::<Mesh>();
    app.add_plugins(PhysicsPlugin);
    // Manual time, or the fixed-timestep schedule never runs during app.update().
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / 60.0,
    )));
    // Avian registers its diagnostics resources in `Plugin::finish`, which a real
    // app's runner calls but a bare `app.update()` loop never does.
    app.finish();
    app
}
```

`TimeUpdateStrategy::ManualDuration` is the one people miss: without it, wall-clock time barely advances between `app.update()` calls and the fixed schedule may never tick, so the test asserts on a simulation that never ran.

### Headless asset / UI tests

Because the editor and game UI are now plain `bevy_ui` (egui is fully removed), UI and asset behavior can be tested headlessly. `parse_templates.rs` shows the pattern: spin up `MinimalPlugins + AssetPlugin`, then exercise the parser or loader. When loading an asset through `AssetServer` you must tick the app until the load completes, since asset loading is async:

```rust
fn pump_until_loaded(app: &mut App, handle: &Handle<HtmlTemplate>) {
    for _ in 0..200 {
        app.update();
        if app.world().resource::<Assets<HtmlTemplate>>().get(handle).is_some() {
            return;
        }
    }
    panic!("asset did not load within 200 frames");
}
```

> There is no editor-panel test harness or `register_panel`-style test helper. Editor panels register at runtime via `register_shell_panel` / `register_panel_content` / `register_shell_status_item`; to test panel logic headlessly, build a `MinimalPlugins` app and exercise the systems or content-builder functions directly.

## What CI runs

`.github/workflows/test.yml` runs on every push and pull request to `main`. Both jobs run **inside the shared base toolchain image** `ghcr.io/renzora/base:latest` — native Linux `cargo test`/`clippy` need only rustc 1.95 and the Linux dev libs, which the base carries (the per-platform cross toolchains aren't needed to test first-party crates). There is nothing to install on the runner.

Two jobs:

```bash
# job: test — first-party crates only
cargo test --workspace \
  --exclude bevy_gauge --exclude bevy_hanabi --exclude bevy_mod_outline \
  --exclude bevy_silk --exclude vleue_navigator \
  --exclude bevy_mod_openxr --exclude bevy_mod_xr --exclude bevy_xr_utils

# job: clippy — lints, warnings are errors
cargo clippy --workspace --no-deps \
  --exclude bevy_gauge --exclude bevy_hanabi --exclude bevy_mod_outline \
  --exclude bevy_silk --exclude vleue_navigator \
  --exclude bevy_mod_openxr --exclude bevy_mod_xr --exclude bevy_xr_utils \
  -- -D warnings \
  -A clippy::too_many_arguments \
  -A clippy::type_complexity
```

Notes on the clippy lane:

- `--no-deps` keeps clippy off the vendored crates that leak in as path-deps.
- `too_many_arguments` and `type_complexity` are allowed because they are inherent to Bevy systems and queries (Bevy allows them too).
- The image deliberately ships without the `clippy` component (it would race the parallel docker build lanes on the rustup download), so the job adds it with `rustup component add clippy`.

> CI does **not** currently run `cargo fmt --check` or `cargo doc` as gating steps — only the `test` and `clippy` jobs above must pass before merge. Match local builds to CI by using the pinned Rust version — `docker/base/Dockerfile` (`rust:1.95.0-bookworm`) for the container, mirrored by `rust-toolchain.toml` for native `cargo renzora` builds.

## The test harness — start here

`renzora_test_harness` is a dev-dependency crate that builds the `App` for you. Add it and pick the cheapest tier the code under test will run in:

```toml
[dev-dependencies]
renzora_test_harness = { path = "../renzora_test_harness" }
```

| Builder | What you get | Use it for |
|---|---|---|
| `minimal_app()` | `MinimalPlugins` + assets + transforms + diagnostics | Pure logic, data transforms, a single system. Milliseconds. |
| `headless_app()` | Full `DefaultPlugins`, **no** wgpu backend | Most `Plugin::build` bodies, resources, events, asset loaders. No GPU needed. |
| `gpu_app()` | Full `DefaultPlugins` **with** an adapter | Render-graph nodes, pipeline specialization, materials, post-process. Opt-in. |

```rust
use renzora_test_harness::{headless_app, pump, pump_until, with_manual_time};

#[test]
fn the_plugin_registers_its_settings_resource() {
    let mut app = headless_app();
    app.add_plugins(MyPlugin);
    pump(&mut app, 1);
    assert!(app.world().get_resource::<MySettings>().is_some());
}
```

`headless_app()` is the one that unlocks most crates, and it is not a mock — it is the same configuration the dedicated server ships (`backends: None`, so Bevy skips renderer init entirely; no `RenderDevice`, no `RenderApp`). A plugin that panics under it has a real bug in its headless guard, not a harness limitation.

Three helpers matter more than they look:

- **`with_manual_time(&mut app, 60.0)`** — without it, wall-clock time barely advances between `app.update()` calls and `FixedUpdate` may never tick at all. A physics or network test then asserts on a simulation that never ran, and *passes* whenever the assertion happens to hold for the initial state. This is the single most-missed step in the workspace.
- **`pump_until(&mut app, 200, "the asset loaded", |a| …)`** — asset loads, task-pool completions and command application are all asynchronous across frames, so "load it and assert" is a race. Panics rather than returning a bool, so a failed wait reports itself instead of surfacing as a confusing downstream assertion.
- **`pump(&mut app, n)`** — run exactly `n` frames.

### GPU-backed tests

`gpu_app()` returns `None` unless `RENZORA_GPU_TESTS=1` is set, and a test should treat that as *skip*:

```rust
#[test]
fn the_post_process_node_writes_its_target() {
    let Some(mut app) = gpu_app() else { return };
    // …
}
```

Bevy requests its adapter inside `Plugin::finish`, and on a machine with no usable adapter that is a panic deep in renderer init with no way to ask first — so probing is not an option and an env opt-in is. CI's `gpu` job sets it after installing lavapipe (Mesa's software Vulkan); set it locally to run the same tests against your real GPU.

## Coverage

Coverage is measured with `cargo-llvm-cov` and gated by a **per-crate ratchet** in `coverage-floors.txt`.

```bash
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked

cargo renzora coverage                 # measure the workspace, print the table
cargo renzora coverage --plugins       # the standalone C-ABI plugins too
cargo renzora coverage --check         # fail if any crate fell below its floor
cargo renzora coverage --bless         # record the current numbers as the new floors
cargo renzora coverage --report-only   # re-read the last run's lcov, no rebuild
```

The table prints worst-first, so the top of it is the next work to do. An HTML report lands in `target/llvm-cov/html`.

### Why the gate is per crate and never one workspace threshold

One global "must be ≥ N%" is the obvious design and the wrong one here. Testability varies by two orders of magnitude across the workspace — `renzora_input`'s action map is pure data transformation, `renzora_ssao` is a render-graph node that cannot execute without a GPU. A single number lets a well-tested crate's gains silently pay for a regression elsewhere, and it has to be set low enough for the worst crate, so it gates nothing.

So the rule is only **never go down**, per crate. A crate at 12% may not drop to 11%; a crate at 0% is pinned at 0%. `--bless` raises the floors after you add tests — raise them in the same commit, and never lower a line to make CI green.

### Bless from a Linux measurement

Coverage is **not bit-reproducible across platforms**: a `#[cfg(windows)]` branch is an uncovered line on Linux and vice versa. Floors are therefore written 1.5 points below the measured value, and that margin is a workaround rather than a fix — it was widened from 0.5 after `renzora`, blessed at 39.0 from a Windows run, measured 38.8 in CI and failed the gate for no behavioural reason. The contract crate carries a lot of platform-gated path handling, so its gap is the widest in the workspace.

Blessing from the platform CI measures on removes the skew entirely. Either run the measurement in the container, or take CI's:

```bash
# Grab the lcov the Coverage workflow uploaded, then bless from it — no rebuild.
gh run download <run-id> -n coverage -D target/coverage
cargo renzora coverage --report-only --bless
```

`--report-only` re-reads `target/coverage/workspace.lcov` without recompiling or re-running anything, so this is seconds rather than an hour.

### What the numbers do and do not mean

Line coverage on the `dist` profile. `opt-level = 2` means the optimizer merges and inlines before instrumentation lands, so the percentages run slightly **optimistic** versus an unoptimized build. They are a trend line and a regression gate, not a proof of exhaustiveness — a fully covered function whose assertions are `assert!(true)` is still untested.

Two mechanical gotchas worth knowing before you debug a zero:

- The `dist` profile sets `strip = "symbols"`, and llvm-cov resolves coverage records through the symbol table. An unmodified `dist` build reports **every crate at 0% with no error**. The xtask forces `CARGO_PROFILE_DIST_STRIP=none` for its own runs; a hand-rolled `cargo llvm-cov` invocation must too.
- Instrumentation changes every crate's fingerprint, so cargo-llvm-cov keeps artifacts in `target/llvm-cov-target/` — a second full artifact tree, disposable, delete it when you need the disk back. This is the one deliberate exception to the one-profile rule in CLAUDE.md §2.
- **Run it on an otherwise idle machine.** It is a second full compile of the workspace and cargo will use every core. Started alongside a plain `cargo test`, the two together exhausted RAM and the pagefile here — and, exactly like the full-disk failure in CLAUDE.md §2, that surfaces as compile errors in crates nobody touched (`only metadata stub found for dylib dependency std`, `failed to mmap file '...rlib': The paging file is too small`) rather than as an out-of-memory message. It goes away on a re-run with the machine to itself.

Vendored crates, dependency checkouts, and the generated `plugins.rs` lists are excluded from the report.

## What CI runs on top of the above

Beyond `test` and `clippy`, two jobs exist specifically to close coverage holes:

- **`plugins`** — loops over `plugins/*/Cargo.toml` and runs each plugin's suite. They are excluded from the workspace on purpose (as members they would inherit the engine's feature unification and link Bevy), and the unintended consequence was that ~14k lines of C-ABI boundary code had never been compiled or tested by CI at all.
- **`gpu`** — installs lavapipe and re-runs the render-touching crates with `RENZORA_GPU_TESTS=1`.

`coverage.yml` is separate and runs on pushes to `main` plus weekly, not on pull requests. That is a disk decision, not a policy one: a runner gives ~20 GB usable, `test` already flirts with filling it, and a coverage run adds a second full artifact tree. It uploads the lcov and HTML report as artifacts and enforces the floors.

## Notes

- The workspace has **no benchmark suite** today — there are no `benches/` directories or `criterion` setup in any first-party crate. If you add one, it is a standard `cargo bench` target.
- Tests run headless; only the opt-in `gpu` lane needs an adapter, which is what lets the rest of the suite run inside the Docker CI container.
