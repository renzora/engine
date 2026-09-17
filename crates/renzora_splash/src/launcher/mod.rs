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

/// The dimmed backdrop behind the overlay panel. Pressing it dismisses.
#[derive(Component)]
pub(crate) struct SplashScrim;

/// The panel header's close button.
#[derive(Component)]
pub(crate) struct SplashClose;

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
            native_splash_poll.run_if(overlay_open),
            poll_releases.run_if(overlay_open),
            update_fps.run_if(overlay_open),
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
            // The condition is an `or`, not a plain "is the overlay open",
            // because this system both *builds and tears down*: on the overlay
            // closing it must still get one pass to despawn `SplashRoot`. Gating
            // on the flag alone would strand the dashboard over the editor
            // forever. Once torn down, neither arm holds and it stops for good:
            // self-clearing, no flag needed.
            //
            // `rebuild_section` is chained after it so the page host exists on the
            // frame the dashboard is built, rather than one frame later.
            (manage_splash, sections::rebuild_section)
                .chain()
                .run_if(overlay_open.or_else(any_with_component::<SplashRoot>)),
            sections::nav_click,
            dismiss_on_scrim_press,
            dismiss_on_close_press,
            dismiss_on_escape,
            chrome::url_click,
            #[cfg(target_arch = "wasm32")]
            collect_web_project_pick,
        ),
    );

    projects::systems(app);
    account::systems(app);
}

/// Is the dashboard up?
///
/// Two conditions, not one. `open` is the user's intent, and the state check is
/// what keeps the overlay from painting over the loading screen: picking a
/// project leaves the editor for a moment, and a dashboard drawn on top of the
/// progress bar for that moment is the one frame where the user most wants to
/// see what is happening.
fn overlay_open(
    overlay: Option<Res<renzora::SplashOverlay>>,
    state: Option<Res<State<SplashState>>>,
) -> bool {
    overlay.is_some_and(|overlay| overlay.open) && state.is_some_and(|state| showable(state.get()))
}

/// The states the dashboard may be drawn in: anything that is not `Loading`.
///
/// `Editor` is the case that matters on the desktop, where the overlay sits over
/// a live editor from the first frame. `Splash` is there for the browser, which
/// has no scratch project to open into (no home directory, no `rustc`, no
/// `--project`) and so never leaves the state it starts in. Excluding only
/// `Loading` says what is actually meant, rather than listing the two states
/// that happen to be left.
fn showable(state: &SplashState) -> bool {
    !matches!(state, SplashState::Loading)
}

/// Pressing the dimmed editor behind the panel dismisses the overlay.
///
/// The gesture that makes the overlay feel like something sitting on your work
/// rather than a screen you are stuck on: press what you can see, and get to it.
/// The panel itself carries `FocusPolicy::Block`, so a press inside it never
/// reaches here.
fn dismiss_on_scrim_press(
    mut overlay: ResMut<renzora::SplashOverlay>,
    scrim: Query<&Interaction, (Changed<Interaction>, With<SplashScrim>)>,
) {
    if scrim.iter().any(|i| matches!(i, Interaction::Pressed)) {
        overlay.open = false;
    }
}

/// The ✕ in the panel header, for anyone who does not know the scrim is
/// clickable.
fn dismiss_on_close_press(
    mut overlay: ResMut<renzora::SplashOverlay>,
    close: Query<&Interaction, (Changed<Interaction>, With<SplashClose>)>,
) {
    if close.iter().any(|i| matches!(i, Interaction::Pressed)) {
        overlay.open = false;
    }
}

/// Escape dismisses, as it does for every other modal surface in the editor.
fn dismiss_on_escape(
    mut overlay: ResMut<renzora::SplashOverlay>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if overlay.open && keys.just_pressed(KeyCode::Escape) {
        overlay.open = false;
    }
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

/// Spawn the dashboard while the splash is the screen, tear it down when it
/// isn't, and rebuild it when the active language changes.
///
/// The rebuild is why this reads `lang::revision()`. Every string on the
/// dashboard is resolved through `lang::t()` at *build* time and baked into a
/// `Text`, so picking a language from the rail footer changed the shared table
/// and left the screen exactly as it was: the picker did not even re-tick
/// itself. Rebuilding is the same answer the editor chrome reaches for
/// (`renzora_shell::relocalize_on_language_change`), and doing it here rather
/// than from a separate despawn system means the respawn happens in the same
/// pass, so there is no blank frame in between.
///
/// The counter is bumped by every pack registration too, so it ticks several
/// times while the language runtime loads its built-ins; the first observed
/// value is swallowed rather than treated as a change.
fn manage_splash(world: &mut World, mut last_rev: Local<u64>, mut seen_once: Local<bool>) {
    let want = world
        .get_resource::<renzora::SplashOverlay>()
        .is_some_and(|overlay| overlay.open)
        && showable(world.resource::<State<SplashState>>().get());
    let mut q = world.query_filtered::<Entity, With<SplashRoot>>();
    let mut existing: Vec<Entity> = q.iter(world).collect();

    let rev = renzora::lang::revision();
    let language_changed = if *last_rev != rev {
        *last_rev = rev;
        std::mem::replace(&mut *seen_once, true)
    } else {
        false
    };
    if language_changed && want {
        for e in existing.drain(..) {
            world.entity_mut(e).despawn();
        }
    }

    if want && existing.is_empty() {
        if world.get_resource::<EmberFonts>().is_none() {
            return;
        }
        let fonts = world.resource::<EmberFonts>().clone();
        // Read out of the page registry here: `spawn_splash` has only `Commands`
        // and cannot see a resource.
        let rail = sections::rail_entries(world);
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            spawn_splash(&mut commands, &fonts, &rail);
        }
        queue.apply(world);
    } else if !want && !existing.is_empty() {
        for e in existing {
            world.entity_mut(e).despawn();
        }
    }
}

/// The dashboard's size as an overlay, in logical pixels.
///
/// Large enough for the rail plus a page of plugin listings, which is what it
/// was sized for as a window, and bounded so it stays a panel on a 4K display
/// rather than growing back into a full screen. The scrim around it is what
/// makes it read as sitting *on* the editor.
const PANEL: (f32, f32) = (1040.0, 700.0);

/// Above the editor's chrome, below the overlays the dashboard itself opens.
///
/// Both halves are load-bearing and they pull in opposite directions. The
/// overlay is modal, so it has to cover the editor underneath: the highest
/// editor surface is the play controls at 5000, and the dock, panels and top bar
/// are far below that.
///
/// But the dashboard's own pages are the marketplace, and pressing Install or
/// opening a listing raises an overlay from `renzora_marketplace`: the store at
/// 9400, a listing at 9600, the install progress at 9700, the hub lightbox at
/// 9900. Those are opened *from* this panel and have to appear over it, so
/// anything at or above 9400 would hide the dashboard's own buttons behind the
/// dashboard.
///
/// 8000 sits in the gap, and the gap is why there is a comment rather than a
/// number.
const OVERLAY_Z: i32 = 8000;

fn spawn_splash(commands: &mut Commands, fonts: &EmberFonts, rail: &[sections::RailEntry]) {
    // The scrim: the editor stays visible through it, dimmed, which is the whole
    // point of the overlay. It used to be opaque and cover a window that had no
    // editor behind it yet.
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(scrim()),
            GlobalZIndex(OVERLAY_Z),
            // Blocks, so the editor underneath cannot be clicked while the
            // overlay is modal, and carries `Interaction` so a press on the
            // scrim itself is the dismiss gesture.
            FocusPolicy::Block,
            Interaction::default(),
            SplashScrim,
            SplashRoot,
            Name::new("splash-scrim"),
        ))
        .id();

    let panel = commands
        .spawn((
            Node {
                width: Val::Px(PANEL.0),
                height: Val::Px(PANEL.1),
                // Shrinks on a window too small to hold it rather than
                // overflowing off the edges, which a fixed size would do on a
                // laptop with the editor un-maximized.
                max_width: Val::Percent(92.0),
                max_height: Val::Percent(92.0),
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(window_bg()),
            // Blocks so a press inside the panel is not also a press on the
            // scrim, which would dismiss the overlay on every click in it.
            FocusPolicy::Block,
            Interaction::default(),
            Name::new("splash-panel"),
        ))
        .id();

    let shell = build_shell(commands, fonts, rail);
    commands.entity(panel).add_children(&[shell]);
    commands.entity(root).add_children(&[panel]);
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

/// Does opening `root` mean restarting rather than transitioning?
///
/// A Bevy project's code is a plugin, and a plugin is installed while the `App`
/// is being built, a moment that has long passed by the time anyone is looking
/// at the launcher. So opening one from a running editor cannot load it; the
/// process has to be replaced by one launched for that project.
///
/// Returns the request to insert, or `None` to carry on as normal. It **decides**
/// rather than acts so that both shapes of caller can use it: [`enter_project`]
/// holds a `&mut World`, while File ▸ Open and File ▸ Recent are systems holding
/// `Commands`.
///
/// **Every path that opens a project asks this**, because they do not share a
/// tail. Without one helper, the dashboard's Open button routed correctly and
/// clicking a card on the very same page did not, which is exactly how this was
/// found.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn restart_needed_for(
    launched: Option<&renzora::core::bevy_project::LaunchedBevyProject>,
    root: &std::path::Path,
) -> Option<renzora::RequestImportBevyProject> {
    if !renzora::core::bevy_project::is_bevy_project(root) {
        return None;
    }
    // Already the project this process was launched for. Re-entering it (File ▸
    // New Project and back, say) is an ordinary transition: restarting would be
    // a gratuitous ten seconds, and if its code had failed to build it would be
    // an endless one.
    if launched.is_some_and(|launched| launched.is(root)) {
        return None;
    }
    info!("[splash] {} is a Bevy project; restarting to load its code", root.display());
    Some(renzora::RequestImportBevyProject(Some(root.to_path_buf())))
}

/// The browser has no second process to restart into, and no `rustc` to build a
/// project with in the first place.
#[cfg(target_arch = "wasm32")]
pub(crate) fn restart_needed_for(
    _launched: Option<&renzora::core::bevy_project::LaunchedBevyProject>,
    _root: &std::path::Path,
) -> Option<renzora::RequestImportBevyProject> {
    None
}

pub(crate) fn enter_project(world: &mut World, project: crate::project::CurrentProject) {
    let launched = world
        .get_resource::<renzora::core::bevy_project::LaunchedBevyProject>()
        .cloned();
    if let Some(request) = restart_needed_for(launched.as_ref(), &project.path) {
        world.insert_resource(request);
        return;
    }
    if let Some(mut cfg) = world.get_resource_mut::<crate::config::AppConfig>() {
        cfg.add_recent_project(project.path.clone());
        let _ = cfg.save();
    }
    world.insert_resource(project);
    // Straight into loading. This used to insert an `Aperture` and let a
    // spectral iris close over the cinematic for 0.55s first; picking a project
    // is a decision already made, so the animation was 0.55s of nothing between
    // the click and the work.
    world.resource_mut::<NextState<SplashState>>().set(SplashState::Loading);
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
                    created_with: Some(renzora::version::ENGINE_VERSION.to_string()),
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
                // No `plugins/`: nothing reads a project's own plugins folder,
                // and the web has no plugin loading at all. See the native
                // `create_project`.
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
