# Testing

How to write and run tests for the Renzora engine workspace with the `renzora test` CLI.

## Running tests

Renzora is one Cargo workspace, and `renzora test` runs its suite inside the pinned toolchain container (it forwards to `cargo test`, so the usual selectors still work):

```bash
# All first-party crates
renzora test

# A single crate
renzora test --package renzora_network

# A single test by name (substring match)
renzora test host_client_is_promoted

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
| `crates/renzora_network/tests/host_server.rs` | `--host` listen-server wiring: a single `App` can hold both Lightyear `ClientPlugins` and `ServerPlugins`, and a local client is promoted to a `HostClient`. |
| `crates/renzora_ember/tests/parse_templates.rs` | Every shipped `.html` UI template parses through bevy_hui's parser — markup syntax errors are caught in CI without a GPU. |
| `crates/renzora_ember/tests/inspector_writeback.rs` | The inspector → `.html` writeback round-trip patches the source file on disk and keeps the span cache coherent. |

### Network example

`host_server.rs` builds an app with both plugin sets, registers the real protocol, and ticks until Lightyear promotes the local client. It is a pure-Lightyear probe that runs without the engine binary:

```rust
use bevy::prelude::*;
use core::time::Duration;
use std::net::SocketAddr;

use lightyear::connection::host::HostClient;
use lightyear::prelude::client::ClientPlugins;
use lightyear::prelude::server::{NetcodeConfig, NetcodeServer, ServerPlugins, ServerUdpIo, Start};
use lightyear::prelude::{Client, Connect, LinkOf, LocalAddr};

#[test]
fn local_client_is_promoted_to_host_client() {
    let tick = Duration::from_secs_f64(1.0 / 64.0);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(ClientPlugins { tick_duration: tick });
    app.add_plugins(ServerPlugins { tick_duration: tick });
    renzora_network::protocol::register_protocol(&mut app);

    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let server = app
        .world_mut()
        .spawn((
            ServerUdpIo::default(),
            LocalAddr(addr),
            NetcodeServer::new(NetcodeConfig::default()),
        ))
        .id();
    app.world_mut().trigger(Start { entity: server });
    for _ in 0..10 {
        app.update();
    }

    let client = app
        .world_mut()
        .spawn((Client::default(), LinkOf { server }))
        .id();
    app.world_mut().trigger(Connect { entity: client });
    for _ in 0..10 {
        app.update();
    }

    assert!(app.world().get::<HostClient>(client).is_some());
}
```

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

## Notes

- The workspace has **no benchmark suite** today — there are no `benches/` directories or `criterion` setup in any first-party crate. If you add one, it is a standard `cargo bench` target.
- There is **no enforced coverage threshold** and no coverage tooling wired into CI. New features should still include tests for their critical paths.
- Tests run headless via `MinimalPlugins`; nothing in the test suite requires a GPU or a window, which is what lets the whole suite run inside the Docker CI container.
