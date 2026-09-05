//! The splash **dashboard** — a title bar, a navigation rail, one page at a
//! time, and a status strip, all floating over the Light Chamber cinematic
//! (`chamber.rs`).
//!
//! It was a launcher: a search field, two buttons and a recents list, centred
//! over the cinematic. That is now the *Projects* page, one of several, because
//! everything else a launcher is for — signing in, installing a plugin, reading
//! what shipped — had no home but the editor, and every one of them is something
//! you want to do *before* you open a project. Installing a plugin especially:
//! plugins load from the engine's own `plugins/` directory at startup, so
//! installing one from inside a project only takes effect on the next launch you
//! are already standing in.
//!
//! The cinematic still runs behind the whole window and every dashboard surface
//! is translucent over it. The old layout was readable because the chamber is
//! *built* to leave the centre column alone — every gate has a clear tunnel down
//! the view axis, so the light banding stays out at the edges. A full-window
//! dashboard covers those edges, which is why the surfaces are dark and
//! translucent rather than opaque: keep them that way.
//!
//! Renders while in [`SplashState::Splash`].
//!
//! **Every clickable node here carries an explicit [`FocusPolicy::Block`].** In
//! Bevy 0.19 `Node` *requires* `FocusPolicy`, and its `Default` is `Pass` — so a
//! node with no policy of its own no longer captures the pointer, it lets the
//! press fall through to every node behind it that also contains the cursor,
//! ancestors included. That is what made clicking the ✕ on a recents row both
//! remove the entry *and* open the project (GH #82). It also used to hand the
//! press to the whole-window drag handle, because the splash root *was* the drag
//! handle; the dashboard's drag handle is the title bar alone, so the rail and
//! the page host block instead — a press on their empty background does nothing,
//! rather than picking the window up.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::time::Real;
use bevy::ui::FocusPolicy;

use renzora_ember::font::EmberFonts;

use crate::github::GithubStats;
use crate::releases::poll_releases;
use crate::SplashState;

pub(crate) mod account;
pub(crate) mod changelog;
pub(crate) mod chrome;
pub(crate) mod projects;
pub(crate) mod sections;
pub(crate) mod style;

pub use sections::{
    register_splash_section, ActiveSection, SectionBuilder, SplashSection, SplashSections,
};

use style::*;

#[derive(Component)]
pub(crate) struct SplashRoot;

/// Smoothed real-time FPS shown in the status strip. The splash is GPU-light, so
/// this is a baseline for "is the app/window itself smooth?" to compare against
/// the editor's much heavier per-frame render cost.
#[derive(Resource, Default)]
pub(crate) struct SplashFps(pub f32);

pub(crate) fn register(app: &mut App) {
    app.init_resource::<SplashFps>()
        .init_resource::<SplashSections>()
        .init_resource::<ActiveSection>();

    // The built-in pages register through the same door a plugin uses.
    projects::register(app);
    changelog::register(app);

    app.add_systems(
        Update,
        (
            reopen_last_project,
            native_splash_poll.run_if(in_state(SplashState::Splash)),
            poll_releases.run_if(in_state(SplashState::Splash)),
            update_fps.run_if(in_state(SplashState::Splash)),
            // `manage_splash` is exclusive (`&mut World`) and ran every frame for
            // the editor's entire life to rediscover "no splash, nothing to do".
            //
            // It costs about the ~18 µs it measures — MEASURED, after an earlier
            // version of this comment claimed the exclusive-system scheduling
            // barrier made it "cost far more than it measures". Gating all three
            // splash pollers moved `main app` 0.147 ms, inside the ±0.36 ms noise
            // floor, while the splash zones fell by exactly their own measured
            // total. So don't hunt exclusive systems expecting outsized wins; the
            // reason to gate this one is that it is 100% waste, not that it is big.
            //
            // The condition is an `or`, not a plain `in_state`, because this system
            // both *builds and tears down*: on leaving `Splash` it must still get
            // one pass to despawn `SplashRoot`. Gating on state alone would strand
            // the splash UI in the editor forever. Once torn down, neither arm
            // holds and it stops for good — self-clearing, no flag needed.
            //
            // `rebuild_section` is chained after it so the page host exists on the
            // frame the dashboard is built, rather than one frame later.
            (manage_splash, sections::rebuild_section).chain().run_if(
                in_state(SplashState::Splash).or_else(any_with_component::<SplashRoot>),
            ),
            sections::nav_click,
            chrome::window_btn_click,
            chrome::drag_handle,
            chrome::resize_zone_click,
            chrome::url_click,
            tick_aperture,
            #[cfg(target_arch = "wasm32")]
            collect_web_project_pick,
        ),
    );

    projects::systems(app);
    account::systems(app);
}

fn native_splash_poll(mut stats: ResMut<GithubStats>) {
    stats.poll();
}

/// Exponentially-smoothed real FPS, updated only while the splash is shown.
fn update_fps(time: Res<Time<Real>>, mut fps: ResMut<SplashFps>) {
    let dt = time.delta_secs();
    if dt > 0.0 {
        let instant = 1.0 / dt;
        fps.0 = if fps.0 <= 0.0 { instant } else { fps.0 * 0.9 + instant * 0.1 };
    }
}

fn reopen_last_project(
    mut commands: Commands,
    reopen: Option<Res<crate::PendingProjectReopen>>,
    mut next_state: ResMut<NextState<SplashState>>,
) {
    if reopen.is_some() {
        commands.remove_resource::<crate::PendingProjectReopen>();
        next_state.set(SplashState::Loading);
    }
}

// ── Lifecycle ────────────────────────────────────────────────────────────────

fn manage_splash(world: &mut World) {
    let want = matches!(world.resource::<State<SplashState>>().get(), SplashState::Splash);
    let mut q = world.query_filtered::<Entity, With<SplashRoot>>();
    let existing: Vec<Entity> = q.iter(world).collect();

    if want && existing.is_empty() {
        if world.get_resource::<EmberFonts>().is_none() {
            return;
        }
        // The post camera (created at startup by `post`) must exist before we
        // can route the background to it.
        let Some(post_cam) = world.get_resource::<crate::post::SplashPost>().map(|p| p.camera)
        else {
            return;
        };
        let fonts = world.resource::<EmberFonts>().clone();
        // Read out of the page registry here: `spawn_splash` has only `Commands`
        // and cannot see a resource.
        let rail = sections::rail_entries(world);
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            spawn_splash(&mut commands, &fonts, post_cam, &rail);
        }
        queue.apply(world);
    } else if !want && !existing.is_empty() {
        for e in existing {
            world.entity_mut(e).despawn();
        }
    }
}

fn spawn_splash(
    commands: &mut Commands,
    fonts: &EmberFonts,
    post_cam: Entity,
    rail: &[sections::RailEntry],
) {
    // The root's colour is what shows when the cinematic isn't running
    // (integrated GPU — see `post::gate_post_camera`), so it has to stand on its
    // own: a near black with a trace of blue in it, matching the chamber's unlit
    // air.
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(window_bg()),
            GlobalZIndex(500),
            // Blocks so a press on the dashboard cannot reach anything behind it,
            // but carries no drag handle any more — that is the title bar's.
            FocusPolicy::Block,
            Interaction::default(),
            SplashRoot,
            Name::new("splash-root"),
        ))
        .id();

    // The cinematic (the Light Chamber render, through its spectral finishing pass)
    // is drawn into the offscreen post camera via its own UI root, so `post.wgsl`
    // can sample it as a whole frame. It carries `SplashRoot` too, so it's torn down
    // with the rest of the splash.
    let bg_host = commands
        .spawn((
            fullscreen_abs(),
            FocusPolicy::Pass,
            bevy::ui::UiTargetCamera(post_cam),
            SplashRoot,
            Name::new("splash-bg-host"),
        ))
        .id();
    let chamber = commands
        .spawn((
            fullscreen_abs(),
            FocusPolicy::Pass,
            crate::chamber::ChamberView,
            Name::new("splash-chamber"),
        ))
        .id();
    commands.entity(bg_host).add_child(chamber);

    // The post-processed background, shown on the main camera behind the UI.
    let post_view = commands
        .spawn((
            fullscreen_abs(),
            FocusPolicy::Pass,
            crate::post::PostView,
            Name::new("splash-post"),
        ))
        .id();

    let shell = build_shell(commands, fonts, rail);

    // Iris transition overlay, above everything (idle = fully transparent, so it
    // doesn't block input until a project is chosen).
    let aperture = commands
        .spawn((
            fullscreen_abs(),
            GlobalZIndex(700),
            FocusPolicy::Pass,
            crate::post::ApertureView,
            Name::new("splash-aperture"),
        ))
        .id();

    commands.entity(root).add_children(&[post_view, shell, aperture]);
    chrome::build_resize_zones(commands, root);
}

fn fullscreen_abs() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        right: Val::Px(0.0),
        bottom: Val::Px(0.0),
        ..default()
    }
}

/// Title bar over rail + page over status strip.
fn build_shell(
    commands: &mut Commands,
    fonts: &EmberFonts,
    rail_entries: &[sections::RailEntry],
) -> Entity {
    let shell = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                width: Val::Percent(100.0),
                // See `sections::build_page_host`: every node between the window
                // and a page's scroll view has to be allowed to shrink below its
                // content, or a tall page grows the whole column instead of
                // scrolling inside it.
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            FocusPolicy::Pass,
            Name::new("splash-shell"),
        ))
        .id();

    let title_bar = chrome::build_title_bar(commands, fonts);

    let body = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let rail = sections::build_rail(commands, fonts, rail_entries);
    let host = sections::build_page_host(commands);
    commands.entity(body).add_children(&[rail, host]);

    let status = account::build_status_bar(commands, fonts);

    commands.entity(shell).add_children(&[title_bar, body, status]);
    shell
}

// ── Entering a project ───────────────────────────────────────────────────────

pub(crate) fn enter_project(world: &mut World, project: crate::project::CurrentProject) {
    if let Some(mut cfg) = world.get_resource_mut::<crate::config::AppConfig>() {
        cfg.add_recent_project(project.path.clone());
        let _ = cfg.save();
    }
    world.insert_resource(project);
    // Close the iris over the cinematic; `tick_aperture` switches to Loading when
    // it finishes.
    world.insert_resource(crate::Aperture::default());
}

/// Advance the iris close; when it completes, drop into the loading screen. Uses
/// real time so it plays at a consistent speed.
fn tick_aperture(
    time: Res<Time<Real>>,
    aperture: Option<ResMut<crate::Aperture>>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<SplashState>>,
) {
    let Some(mut ap) = aperture else { return };
    ap.timer += time.delta_secs();
    if ap.timer >= crate::APERTURE_DURATION {
        commands.remove_resource::<crate::Aperture>();
        next_state.set(SplashState::Loading);
    }
}

/// Finish a web Open Project once the browser's picker has resolved.
///
/// The desktop path is a single blocking call — `rfd` opens the dialog and
/// returns the chosen path. The browser's picker cannot work that way: it
/// resolves whenever the user gets round to choosing, which is no particular
/// frame. So the click only *starts* the pick, and this collects the result on
/// whichever frame it lands.
#[cfg(target_arch = "wasm32")]
fn collect_web_project_pick(mut commands: Commands) {
    let Some(picked) = renzora_webfs::take_picked_project() else {
        return;
    };
    commands.queue(move |world: &mut World| {
        let root = std::path::PathBuf::from(&picked.name);
        let config: crate::project::ProjectConfig = match picked.project_toml {
            Some(ref toml_src) => match toml::from_str(toml_src) {
                Ok(c) => c,
                Err(e) => {
                    error!("[webfs] project.toml is not valid: {e}");
                    return;
                }
            },
            // A new project. Mirrors the desktop `create_project`: the same
            // config, the same `scenes/` + `plugins/` skeleton, and the same
            // empty interim-BSN scene, so a project made in the browser opens
            // on the desktop and vice versa.
            None => {
                let config = crate::project::ProjectConfig {
                    name: picked.name.clone(),
                    version: "0.1.0".to_string(),
                    main_scene: "scenes/main.bsn".to_string(),
                    ..Default::default()
                };
                let Ok(toml_src) = toml::to_string_pretty(&config) else {
                    error!("[webfs] could not serialize the new project config");
                    return;
                };
                // Fire-and-forget: these are local writes that land in
                // milliseconds, and the editor reads scenes lazily through the
                // same cache. If a very early read ever beats the write, it
                // shows as a missing main.bsn on first entry and is fixed by
                // awaiting these before entering.
                renzora_webfs::spawn_create_dir(root.join("plugins"));
                renzora_webfs::spawn_write_text(
                    root.join("scenes").join("main.bsn"),
                    "// renzora interim bsn v1\n".to_string(),
                );
                renzora_webfs::spawn_write_text(root.join("project.toml"), toml_src);
                info!("[webfs] created project '{}'", picked.name);
                config
            }
        };
        // The browser hands back a directory HANDLE, never a path, so the only
        // identifier available is the folder's own name. Everything that reads
        // `CurrentProject::path` on the web is therefore addressing the picked
        // directory relatively — which is exactly what the handle wants anyway.
        let project = crate::project::CurrentProject {
            path: root,
            config,
        };
        info!("[webfs] opening project '{}'", picked.name);
        enter_project(world, project);
    });
}
