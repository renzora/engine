#[cfg(not(target_arch = "wasm32"))]
pub mod bevy_scaffold;
pub mod config;
pub mod github;
mod chamber;
pub mod launcher;
pub mod loading;
mod loading_ui;
mod post;
pub mod project;
pub mod releases;
#[cfg(target_arch = "wasm32")]
pub mod web_storage;

pub use config::AppConfig;
pub use github::GithubStats;
/// The dashboard page registry. A crate that can depend on this one adds its own
/// page with [`launcher::register_splash_section`] — see `launcher::sections`.
pub use launcher::{
    register_splash_section, ActiveSection, SectionBuilder, SplashSection, SplashSections,
};
pub use releases::{ReleaseEntry, ReleaseFeed};
pub use loading::{
    EditorLoadingOverlayActive, LoadingBytes, LoadingTask, LoadingTaskHandle, LoadingTasks,
    TextureLoadProgress,
};
#[cfg(not(target_arch = "wasm32"))]
pub use project::create_project;
pub use project::{open_project, CurrentProject, ProjectConfig, WindowConfig};

/// The id of the **Templates** dashboard page.
///
/// Declared here rather than by the page itself because the dependency runs the
/// other way: the Projects page's "New from Template" button has to name the
/// section it switches to, and the crate that *provides* that section
/// (`renzora_marketplace`) depends on this one. A build without the marketplace
/// registers no such section, and the button hides itself.
pub const TEMPLATES_SECTION_ID: &str = "templates";

/// Open a project that was just created, as the splash's own New Project does:
/// record it in recents and start the transition into the editor.
///
/// Public because a dashboard page can be registered from another crate, and a
/// page that creates a project needs the same ending as the one built in.
pub fn enter_created_project(world: &mut World, project: CurrentProject) {
    launcher::enter_project(world, project);
}

use bevy::prelude::*;

// SplashState now lives in the `renzora` SDK — coordination contract used
// by both the splash UI and the editor framework. Re-exported here for
// back-compat so existing `renzora_splash::SplashState` paths keep working.
pub use renzora::SplashState;

#[derive(Default)]
pub struct SplashPlugin;

impl Plugin for SplashPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] SplashPlugin");

        let app_config = AppConfig::load();

        app.init_state::<SplashState>()
            .insert_resource(app_config)
            .insert_resource(GithubStats::new())
            // Kicked off here, not on first view of the Changelog page: the
            // request is unauthenticated and rate-limited per IP, so it is worth
            // exactly one per launch, and starting it now means the page has an
            // answer by the time anyone clicks through to it.
            .insert_resource(releases::ReleaseFeed::new())
            .init_resource::<LoadingTasks>()
            .init_resource::<LoadingBytes>()
            .init_resource::<TextureLoadProgress>()
            .init_resource::<EditorLoadingOverlayActive>()
            .add_systems(
                Update,
                loading::auto_advance_to_editor.run_if(in_state(SplashState::Loading)),
            )
            .init_resource::<renzora::RecentProjects>()
            .add_systems(
                Update,
                (
                    handle_request_open_project,
                    handle_request_open_project_path,
                    handle_request_import_bevy_project,
                    mirror_recent_projects,
                ),
            )
            .add_systems(
                OnEnter(SplashState::Loading),
                (loading::log_loading_entered, maximize_for_editor),
            );

        // Dev shortcut: `--project <path>` skips the splash UI and jumps
        // straight into the project. This moved here from the binary's `main()`
        // when the editor became a removable bundle — the splash plugin lives
        // in the bundle, so the lean game binary no longer references it.
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(Startup, apply_project_arg);

        // The launcher + loading screen, the Light Chamber cinematic, and the
        // post/transition chain that finishes and closes over it.
        launcher::register(app);
        loading_ui::register(app);
        chamber::register(app);
        post::register(app);
    }
}

/// Consume `renzora::RequestOpenProject` markers (inserted by the editor's
/// File menu). Owns the file dialog + validation + AppConfig update +
/// state transition so `renzora_editor_framework` doesn't need to depend on splash.
#[cfg(not(target_arch = "wasm32"))]
fn handle_request_open_project(
    mut commands: Commands,
    request: Option<Res<renzora::RequestOpenProject>>,
    launched: Option<Res<renzora::core::bevy_project::LaunchedBevyProject>>,
    mut app_config: ResMut<AppConfig>,
    mut next_state: ResMut<NextState<SplashState>>,
) {
    if request.is_none() {
        return;
    }
    commands.remove_resource::<renzora::RequestOpenProject>();

    // A folder, not a file. It used to ask for a `project.toml`, which a Bevy
    // project does not have (its manifest is `Cargo.toml`) and asking the user
    // to pick the right one of two files to say "this folder" was never the
    // question anyway. `open_project` decides which manifest to read from the
    // folder it is given.
    let Some(file) = rfd::FileDialog::new()
        .set_title(renzora::lang::t_or("splash.open_project_pick_folder", "Open Project"))
        .pick_folder()
    else {
        return;
    };

    // A Bevy project is opened by restarting, not by transitioning. Checked
    // before the project is even parsed: the reason has nothing to do with
    // whether its config reads, and `restart_needed_for` explains it once for
    // every entry point.
    if let Some(request) = launcher::restart_needed_for(launched.as_deref(), &file) {
        commands.insert_resource(request);
        return;
    }

    let project = match project::open_project(&file) {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to open project: {}", e);
            rfd::MessageDialog::new()
                .set_title("Invalid Project")
                .set_description(format!("Failed to open project: {}", e))
                .set_buttons(rfd::MessageButtons::Ok)
                .show();
            return;
        }
    };

    app_config.add_recent_project(project.path.clone());
    let _ = app_config.save();
    commands.insert_resource(project);
    commands.insert_resource(PendingProjectReopen);
    next_state.set(SplashState::Splash);
    info!("Opening project...");
}

#[cfg(target_arch = "wasm32")]
fn handle_request_open_project(
    mut commands: Commands,
    request: Option<Res<renzora::RequestOpenProject>>,
) {
    if request.is_some() {
        commands.remove_resource::<renzora::RequestOpenProject>();
        warn!("Open Project is not available in the browser");
    }
}

/// Consume `renzora::RequestImportBevyProject`: File > Import Bevy Project, and
/// the dashboard button of the same name.
///
/// Ends in a **restart**, which is the whole reason this is not a branch inside
/// [`handle_request_open_project`]. A Bevy project's code is a plugin, a plugin
/// is installed while the `App` is being built, and that moment is long past by
/// the time anyone clicks a menu. So the pick is validated here, where a bad one
/// can still be reported in a dialog the user is looking at, and the good one is
/// carried across the restart by
/// [`renzora::core::bevy_project::restart_into`].
///
/// Validation is structural only: is this a crate, does it depend on Bevy.
/// Whether its `fn main` can be read, and whether it compiles, are answered
/// after the restart by the loader, which has the SDK and reports into the
/// Console and the Problems panel. Answering them here would mean running a
/// compiler from inside a file dialog's callback.
#[cfg(not(target_arch = "wasm32"))]
fn handle_request_import_bevy_project(
    mut commands: Commands,
    request: Option<Res<renzora::RequestImportBevyProject>>,
    mut app_config: ResMut<AppConfig>,
) {
    let Some(request) = request else { return };
    let picked = request.0.clone();
    commands.remove_resource::<renzora::RequestImportBevyProject>();

    // Already chosen by Open Project, which found a Bevy crate where it expected
    // a Renzora one. Asking for the folder again would be asking the user to
    // repeat themselves.
    let root = match picked {
        Some(root) => root,
        None => {
            let Some(root) = rfd::FileDialog::new()
                .set_title(renzora::lang::t("splash.import_bevy_project"))
                .pick_folder()
            else {
                return;
            };
            root
        }
    };

    // Two different "no", and they need different words. A folder with no
    // `Cargo.toml` is not a crate at all; one with a `Cargo.toml` and no `bevy`
    // is somebody's CLI tool. Both used to be "could not open project".
    if renzora::core::bevy_project::inspect(&root).is_none() {
        let detail = if root.join("Cargo.toml").is_file() {
            "Its Cargo.toml has no `bevy` dependency.\n\nIf this is a workspace, name the game \
             crate in the root Cargo.toml:\n\n    [workspace.metadata.renzora]\n    member = \
             \"crates/game\""
        } else {
            "There is no Cargo.toml here. Pick the folder that holds your crate."
        };
        rfd::MessageDialog::new()
            .set_title("Not a Bevy Project")
            .set_description(format!("{}\n\n{detail}", root.display()))
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
        return;
    }

    // Recorded before the restart, not after: the child process reads this file
    // at launch, so writing it afterwards would be writing it in a process that
    // no longer exists.
    app_config.add_recent_project(root.clone());
    let _ = app_config.save();
    info!("[splash] importing Bevy project {}", root.display());
    renzora::core::bevy_project::restart_into(&root);
}

#[cfg(target_arch = "wasm32")]
fn handle_request_import_bevy_project(
    mut commands: Commands,
    request: Option<Res<renzora::RequestImportBevyProject>>,
) {
    if request.is_some() {
        commands.remove_resource::<renzora::RequestImportBevyProject>();
        // Not a missing feature so much as a missing platform: the browser has
        // no `rustc` to compile the crate and no `dlopen` to load it with.
        warn!("Importing a Bevy project needs a Rust toolchain, so it is desktop-only");
    }
}

/// Consume `renzora::RequestOpenProjectPath` — File > Recent Projects, which
/// already knows the root and so skips the dialog `handle_request_open_project`
/// opens. Everything after the pick is the same, deliberately: the entry is
/// re-recorded so opening a project moves it back to the top of the list.
///
/// A recents entry can name a folder that has since been moved or deleted, so
/// the failure here is ordinary rather than exceptional — say so and leave the
/// editor where it is, exactly as an invalid pick does.
#[cfg(not(target_arch = "wasm32"))]
fn handle_request_open_project_path(
    mut commands: Commands,
    request: Option<Res<renzora::RequestOpenProjectPath>>,
    launched: Option<Res<renzora::core::bevy_project::LaunchedBevyProject>>,
    mut app_config: ResMut<AppConfig>,
    mut next_state: ResMut<NextState<SplashState>>,
) {
    let Some(request) = request else { return };
    let root = request.0.clone();
    commands.remove_resource::<renzora::RequestOpenProjectPath>();

    if let Some(request) = launcher::restart_needed_for(launched.as_deref(), &root) {
        commands.insert_resource(request);
        return;
    }

    let project = match project::open_project(&root.join("project.toml")) {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to open recent project {}: {}", root.display(), e);
            rfd::MessageDialog::new()
                .set_title("Project Unavailable")
                .set_description(format!("Could not open {}:\n{}", root.display(), e))
                .set_buttons(rfd::MessageButtons::Ok)
                .show();
            return;
        }
    };

    app_config.add_recent_project(project.path.clone());
    let _ = app_config.save();
    commands.insert_resource(project);
    commands.insert_resource(PendingProjectReopen);
    next_state.set(SplashState::Splash);
    info!("Opening recent project {}", root.display());
}

/// Web: a recents entry is a folder *name*, and reopening it goes back through
/// the directory handle the browser stored when it was first picked — the same
/// route the dashboard's recents cards take.
#[cfg(target_arch = "wasm32")]
fn handle_request_open_project_path(
    mut commands: Commands,
    request: Option<Res<renzora::RequestOpenProjectPath>>,
) {
    let Some(request) = request else { return };
    let root = request.0.clone();
    commands.remove_resource::<renzora::RequestOpenProjectPath>();
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| root.to_string_lossy().to_string());
    renzora_webfs::reopen_project(name);
}

/// Publish the launcher's recents list to the contract crate, so the editor's
/// File > Recent Projects submenu can read it without depending on this crate.
///
/// A copy rather than a move because `AppConfig` is what is written to disk and
/// what the dashboard's cards are built from; this is the read-only view of it
/// that crosses a crate boundary.
fn mirror_recent_projects(cfg: Res<AppConfig>, mut recents: ResMut<renzora::RecentProjects>) {
    if !cfg.is_changed() {
        return;
    }
    recents.0 = cfg.recent_projects.clone();
}

/// Startup: honor a `--project <path>` argument by opening that project and
/// arming the splash to jump straight to Loading (skipping the UI). Inserts
/// `CurrentProject` + `PendingProjectReopen`; `reopen_last_project` picks the marker
/// up on the first `Update` and transitions to `Loading`. No-op without the
/// argument or on a failed open.
#[cfg(not(target_arch = "wasm32"))]
fn apply_project_arg(mut commands: Commands) {
    let Some(path) = std::env::args()
        .skip_while(|a| a != "--project")
        .nth(1)
        .map(std::path::PathBuf::from)
    else {
        return;
    };
    info!("[splash] --project {}", path.display());
    let project_toml = path.join("project.toml");
    match project::open_project(&project_toml) {
        Ok(project) => {
            commands.insert_resource(project);
            commands.insert_resource(PendingProjectReopen);
        }
        Err(e) => error!("[splash] Failed to open --project: {}", e),
    }
}

/// Grow the window to fill the screen now that a project is opening.
///
/// The editor window is *created* at `renzora_runtime::SPLASH_WINDOW` — a
/// launcher-sized box in the middle of the screen — because that is the right
/// size for the dashboard, and the wrong one for an editor. This is the moment
/// the second becomes true: `Loading` is entered only from choosing a project,
/// so it is the last frame before the workspace appears.
///
/// Not on `OnEnter(Editor)`: the loading screen is the editor's own, and having
/// it play out inside the launcher-sized window only to snap open at the end
/// reads as a stutter at exactly the point the user is waiting.
///
/// A user who maximized the splash themselves gets a no-op, and a user who
/// wants a smaller editor window can still resize it — nothing here runs again.
#[cfg(not(target_arch = "wasm32"))]
fn maximize_for_editor(
    mut windows: Query<&mut bevy::window::Window, With<bevy::window::PrimaryWindow>>,
) {
    if let Ok(mut window) = windows.single_mut() {
        window.set_maximized(true);
    }
}

/// The browser has no OS window to maximize; the canvas is sized by the page.
#[cfg(target_arch = "wasm32")]
fn maximize_for_editor() {}

/// Marker resource: splash should immediately transition back to editor
/// (e.g. project opened via File menu).
#[derive(Resource)]
pub struct PendingProjectReopen;

renzora::add!(SplashPlugin, Editor);
