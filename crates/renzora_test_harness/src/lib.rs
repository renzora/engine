//! Shared headless `App` builders for the engine's test suites.
//!
//! ## Why this crate exists
//!
//! Around 80 of the ~127 first-party crates had no tests at all, and the reason
//! was almost never that the code was untestable — it was that standing up an
//! `App` a given plugin would consent to `build()` into took thirty lines of
//! boilerplate that had to be rediscovered per crate. `renzora_physics`'s
//! integration test is the worst case and a fair illustration: it needs
//! `MinimalPlugins`, plus `DiagnosticsPlugin` (Avian's spatial-query systems take
//! diagnostics resources it only inserts when Bevy's are present), plus
//! `AssetPlugin`, plus `TransformPlugin`, plus an `init_asset::<Mesh>()`, plus a
//! manual `TimeUpdateStrategy`, plus an explicit `app.finish()`. Miss the last
//! two and the test asserts on a simulation that never ticked — it passes or
//! fails for reasons unrelated to the code under test.
//!
//! Everything here is a dev-dependency. Nothing in a shipped binary links it.
//!
//! ## The three tiers
//!
//! Pick the cheapest one that still lets the code under test run. They differ by
//! roughly an order of magnitude in cost each.
//!
//! | Builder | Plugins | Use it for |
//! |---|---|---|
//! | [`minimal_app`] | `MinimalPlugins` + assets + transforms | Pure logic, data transforms, single systems. Milliseconds. |
//! | [`headless_app`] | Full `DefaultPlugins`, **no** wgpu backend | Anything that needs the engine's real type surface — most `Plugin::build` bodies, resources, events, asset loaders. No GPU required. |
//! | [`gpu_app`] | Full `DefaultPlugins` **with** an adapter | Render-graph nodes, pipeline specialization, materials, post-process. Opt-in; see below. |
//!
//! [`headless_app`] is the one that unlocks most of the untested crates, and it
//! is not a mock: it is the same configuration the dedicated server ships
//! (`renzora_runtime::add_headless_rendering`) — `backends: None` makes Bevy skip
//! renderer initialization entirely, so there is no `RenderDevice` and no
//! `RenderApp`, but every other plugin still finds the types and resources it
//! expects. Plugins that touch the render sub-app already have to guard for its
//! absence, exactly as Bevy's own render plugins do, so a crate that panics under
//! [`headless_app`] has found a real bug in its headless guard rather than a
//! limitation of the harness.
//!
//! ## GPU tests are opt-in, on purpose
//!
//! [`gpu_app`] returns `None` unless `RENZORA_GPU_TESTS=1` is set, and a test
//! that needs it should treat `None` as "skip", not "fail". Bevy requests its
//! adapter inside `Plugin::finish`, and on a machine with no usable adapter that
//! is a panic deep in renderer init with no way to ask first — so probing is not
//! an option and an env opt-in is. CI's `gpu` lane sets it after installing
//! lavapipe (Mesa's software Vulkan); a developer with a real GPU can set it
//! locally and get the same coverage against real hardware.

use std::time::Duration;

use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;

/// Environment variable that opts a run into the GPU-backed tier.
pub const GPU_TESTS_ENV: &str = "RENZORA_GPU_TESTS";

/// The cheapest useful app: scheduler, time, assets, transforms, hierarchy.
///
/// No render types, no windowing, no input. Reach for this first — if the code
/// under test compiles against it, the test runs in milliseconds and cannot
/// flake on a driver.
pub fn minimal_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        // Avian and anything else that reads Bevy's own diagnostics resources
        // takes them as a hard `Res<...>` dependency, and the failure mode is a
        // system-parameter panic rather than a graceful skip. It costs nothing
        // to have present, so it is unconditional here.
        bevy::diagnostic::DiagnosticsPlugin,
        TransformPlugin,
    ));
    app
}

/// Full `DefaultPlugins` with no graphics backend — the tier most engine crates
/// want.
///
/// Mirrors the dedicated server's configuration, with three further changes that
/// only matter inside a test binary:
///
/// - **`LogPlugin` disabled.** It installs a *global* tracing subscriber, and a
///   test binary builds many apps in one process (in parallel, on separate
///   threads). The second install is at best ignored and at worst a panic, which
///   would make an unrelated test fail depending on scheduling order.
/// - **`WinitPlugin` disabled.** It creates the OS event loop in `build()`, which
///   fails outright on a headless Linux runner with no X or Wayland session —
///   before any test code runs, and regardless of whether a window was asked
///   for.
/// - **No `ScheduleRunnerPlugin` re-added.** Tests drive frames with
///   `app.update()`; a runner would only matter to `app.run()`, which a test must
///   never call.
pub fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(headless_plugins(None));
    // Bevy resolves an adapter and inserts a number of resources in
    // `Plugin::finish`, which a real app's runner calls and a bare `app.update()`
    // loop never does. Calling it here is what stops a test from asserting on a
    // half-initialized app.
    app.finish();
    app
}

/// [`headless_app`] with the asset root pointed somewhere specific.
///
/// Bevy resolves the default asset root relative to the *current directory*,
/// which for a test binary is the crate root — so a test that loads a fixture
/// from `tests/fixtures/` has to say so. Pass a path relative to the crate root.
pub fn headless_app_with_assets(asset_root: &str) -> App {
    let mut app = App::new();
    app.add_plugins(headless_plugins(Some(asset_root.to_string())));
    app.finish();
    app
}

/// Full `DefaultPlugins` **with** a real graphics adapter, or `None` when the
/// run has not opted in.
///
/// Treat `None` as skip:
///
/// ```no_run
/// # use renzora_test_harness::gpu_app;
/// #[test]
/// fn the_post_process_node_writes_its_target() {
///     let Some(mut app) = gpu_app() else { return };
///     // ...
/// }
/// ```
///
/// The window is `None` rather than a hidden window: a surface needs a real
/// display server, and none of the render-graph work under test draws to one.
/// Render-to-texture targets work exactly as they do in the editor's viewport.
pub fn gpu_app() -> Option<App> {
    if std::env::var(GPU_TESTS_ENV).unwrap_or_default() != "1" {
        return None;
    }
    let mut app = App::new();
    app.add_plugins(gpu_plugins());
    app.finish();
    Some(app)
}

/// True when this run opted into GPU-backed tests.
///
/// Useful for a test that wants to *assert* rather than skip when the lane is
/// meant to be active — `assert!(gpu_tests_enabled())` in a CI-only test.
pub fn gpu_tests_enabled() -> bool {
    std::env::var(GPU_TESTS_ENV).unwrap_or_default() == "1"
}

fn headless_plugins(asset_root: Option<String>) -> PluginGroupBuilder {
    use bevy::render::{
        settings::{RenderCreation, WgpuSettings},
        RenderPlugin,
    };
    use bevy::window::{ExitCondition, WindowPlugin};

    base_plugins(asset_root)
        .set(RenderPlugin {
            // No backend at all: Bevy detects no wgpu backend and skips renderer
            // initialization entirely — no adapter request, no `RenderDevice`,
            // no `RenderApp` sub-app. This is what lets the tier run on a CI
            // container with no GPU and no drivers.
            render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
                backends: None,
                ..default()
            })),
            ..default()
        })
        .set(WindowPlugin {
            primary_window: None,
            exit_condition: ExitCondition::DontExit,
            ..default()
        })
}

fn gpu_plugins() -> PluginGroupBuilder {
    use bevy::window::{ExitCondition, WindowPlugin};

    // Deliberately the *default* `RenderPlugin` rather than a configured one:
    // whatever adapter the host offers is the one we want to test against, and
    // pinning a backend here would silently skip the lane on a runner whose
    // software Vulkan is fine but whose backend bit we guessed wrong.
    base_plugins(None).set(WindowPlugin {
        primary_window: None,
        exit_condition: ExitCondition::DontExit,
        ..default()
    })
}

fn base_plugins(asset_root: Option<String>) -> PluginGroupBuilder {
    let mut group = DefaultPlugins
        .build()
        .disable::<bevy::log::LogPlugin>()
        .disable::<bevy::winit::WinitPlugin>();
    if let Some(path) = asset_root {
        group = group.set(bevy::asset::AssetPlugin {
            file_path: path,
            ..default()
        });
    }
    group
}

// ── driving frames ───────────────────────────────────────────────────────────

/// Give an app a deterministic clock so fixed-timestep schedules actually tick.
///
/// Without this, wall-clock time barely advances between `app.update()` calls and
/// `FixedUpdate` may never run at all — so a physics or network test asserts on a
/// simulation that never happened, and *passes* whenever the assertion happens to
/// hold for the initial state. This is the single most-missed step in the
/// workspace's existing tests.
///
/// `hz` is the simulated frame rate, not the fixed-timestep rate: each
/// `app.update()` advances the clock by `1/hz` seconds.
///
/// **The first `app.update()` still has a delta of zero.** Bevy has no previous
/// frame to measure against, so anything scaled by `time.delta_secs()`
/// legitimately does nothing on frame one. Pump at least twice before asserting
/// that a dt-driven system moved something, or the test fails against correct
/// code.
pub fn with_manual_time(app: &mut App, hz: f64) {
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / hz,
    )));
}

/// Run exactly `frames` frames.
pub fn pump(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

/// Tick until `cond` holds, up to `max_frames`; panics with `what` if it never
/// does.
///
/// Asset loading, task-pool completions and command application are all
/// asynchronous across frames, so "load it and assert" is a race. This is the
/// bounded wait that replaces it — and it panics rather than returning a bool
/// because a test that silently proceeds past a failed wait reports a confusing
/// downstream assertion instead of the real cause.
pub fn pump_until(app: &mut App, max_frames: usize, what: &str, cond: impl Fn(&App) -> bool) {
    for _ in 0..max_frames {
        if cond(app) {
            return;
        }
        app.update();
    }
    if cond(app) {
        return;
    }
    panic!("`{what}` did not become true within {max_frames} frames");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Ticks(u32);

    fn count(mut t: ResMut<Ticks>) {
        t.0 += 1;
    }

    #[test]
    fn minimal_app_runs_systems() {
        let mut app = minimal_app();
        app.init_resource::<Ticks>().add_systems(Update, count);
        pump(&mut app, 3);
        assert_eq!(app.world().resource::<Ticks>().0, 3);
    }

    #[test]
    fn minimal_app_has_the_asset_server() {
        let app = minimal_app();
        assert!(
            app.world().get_resource::<AssetServer>().is_some(),
            "AssetPlugin should be installed"
        );
    }

    /// The tier's whole promise: a full `DefaultPlugins` app with no GPU. If this
    /// regresses, every crate-level test built on `headless_app` fails at once,
    /// so it is worth asserting directly.
    #[test]
    fn headless_app_boots_without_a_render_device() {
        let mut app = headless_app();
        app.init_resource::<Ticks>().add_systems(Update, count);
        pump(&mut app, 2);
        assert_eq!(app.world().resource::<Ticks>().0, 2);
        assert!(
            app.get_sub_app(bevy::render::RenderApp).is_none(),
            "backends: None should mean no RenderApp at all"
        );
    }

    #[test]
    fn manual_time_advances_the_clock() {
        let mut app = minimal_app();
        with_manual_time(&mut app, 60.0);
        pump(&mut app, 10);
        let elapsed = app.world().resource::<Time>().elapsed_secs_f64();
        assert!(
            elapsed >= 9.0 / 60.0,
            "10 frames at 60 Hz should advance ~0.167s, got {elapsed}"
        );
    }

    #[test]
    fn pump_until_returns_as_soon_as_the_condition_holds() {
        let mut app = minimal_app();
        app.init_resource::<Ticks>().add_systems(Update, count);
        pump_until(&mut app, 100, "five ticks", |a| {
            a.world().resource::<Ticks>().0 >= 5
        });
        assert_eq!(app.world().resource::<Ticks>().0, 5);
    }

    #[test]
    #[should_panic(expected = "did not become true")]
    fn pump_until_panics_rather_than_passing_silently() {
        let mut app = minimal_app();
        pump_until(&mut app, 3, "never", |_| false);
    }

    #[test]
    fn gpu_app_is_none_unless_opted_in() {
        if !gpu_tests_enabled() {
            assert!(gpu_app().is_none());
        }
    }
}
