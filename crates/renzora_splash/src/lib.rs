#[cfg(not(target_arch = "wasm32"))]
pub mod bevy_scaffold;
pub mod config;
pub mod github;
mod haze;
pub mod launcher;
pub mod loading;
mod loading_ui;
pub mod project;
pub mod releases;
#[cfg(not(target_arch = "wasm32"))]
pub mod untitled;
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
            // Open from the start in the browser, closed on the desktop.
            //
            // The desktop opens it from `show_overlay_on_launch`, once the
            // editor it overlays actually exists. The browser has no scratch
            // project to open into, so it stays in `SplashState::Splash` with no
            // editor behind the panel, and nothing would ever set this: the
            // dashboard *is* the web editor's first screen, as it used to be
            // everywhere.
            .insert_resource(renzora::SplashOverlay {
                open: cfg!(target_arch = "wasm32"),
            })
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
                    handle_request_create_project,
                    mirror_recent_projects,
                ),
            )
            .add_systems(
                OnEnter(SplashState::Loading),
                (loading::log_loading_entered, close_overlay_for_project_load),
            )
            .add_systems(OnEnter(SplashState::Editor), show_overlay_on_launch);

        // Dev shortcut: `--project <path>` skips the splash UI and jumps
        // straight into the project. This moved here from the binary's `main()`
        // when the editor became a removable bundle — the splash plugin lives
        // in the bundle, so the lean game binary no longer references it.
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(Startup, apply_project_arg);

        // Kept in step with whatever project is open, from every route that can
        // open one: the dashboard, the File menu, `--project`, and the startup
        // fallback above. One system rather than an insert at each of those
        // call sites, because the failure mode of missing one is an editor that
        // says "Untitled" while sitting in a real project.
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(Update, track_untitled_marker);

        // The launcher overlay and the loading screen. The Light Chamber
        // cinematic and its post chain used to be registered here too; they were
        // a full-window background, and the dashboard stopped having one when it
        // became a panel over a live editor.
        launcher::register(app);
        loading_ui::register(app);
        haze::register(app);
    }
}

/// Put the dashboard up the first time the editor is ready, and only then.
///
/// `OnEnter(Editor)` fires every time a project finishes loading, which is
/// exactly the wrong moment to show a project picker: someone who just chose a
/// project would watch it load and then be asked to choose again. So the marker
/// is consumed here, and there is only ever one of it.
///
/// It is absent when `--project` named a project, which is someone saying what
/// they want to open clearly enough that asking again would be rude.
#[cfg(not(target_arch = "wasm32"))]
fn show_overlay_on_launch(
    mut commands: Commands,
    pending: Option<Res<PendingSplashOverlay>>,
    mut overlay: ResMut<renzora::SplashOverlay>,
) {
    if pending.is_none() {
        return;
    }
    commands.remove_resource::<PendingSplashOverlay>();
    overlay.open = true;
}

#[cfg(target_arch = "wasm32")]
fn show_overlay_on_launch() {}

/// Close the overlay whenever a project starts loading.
///
/// Every route into a project passes through `Loading`, which makes this the one
/// place that has to know: the dashboard's project cards, File > Open Project,
/// File > Recent Projects and the templates page would otherwise each have to
/// remember to close the thing they were clicked in.
///
/// Without it the overlay reappears the moment loading finishes, because `open`
/// would still be true when `Editor` is entered again.
fn close_overlay_for_project_load(mut overlay: ResMut<renzora::SplashOverlay>) {
    overlay.open = false;
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

/// Startup: decide which project the editor opens into, and arm the jump
/// straight to `Loading`.
///
/// Two sources, in order of how deliberate they are:
///
/// 1. `--project <path>`, which is someone naming a project outright;
/// 2. the scratch project, for everyone else.
///
/// There is no third case where nothing is opened. The editor is always behind
/// the splash overlay now, and it cannot run without a project (see
/// [`UntitledProject`](renzora::UntitledProject)), so "no project" is not a
/// state that exists: dismissing the overlay lands you in
/// [`untitled`](crate::untitled), not in an empty shell.
///
/// One system rather than two chained ones, because the second has to see
/// whether the first inserted anything and `Commands` are deferred: reading the
/// decision from a local is a fact, reading it from the world is a question
/// about when the schedule applied its queue.
///
/// Inserts `CurrentProject` + [`PendingProjectReopen`], which
/// `reopen_last_project` picks up on the first `Update` to transition to
/// `Loading`.
#[cfg(not(target_arch = "wasm32"))]
fn apply_project_arg(mut commands: Commands, mut app_config: ResMut<AppConfig>) {
    let from_arg = std::env::args()
        .skip_while(|a| a != "--project")
        .nth(1)
        .map(std::path::PathBuf::from);

    let named_on_the_command_line = from_arg.is_some();
    let project = match from_arg {
        Some(path) => {
            info!("[splash] --project {}", path.display());
            match project::open_project(&path.join("project.toml")) {
                Ok(project) => Some(project),
                Err(e) => {
                    // Deliberately not falling through to the scratch project.
                    // Someone who named a project on the command line wants that
                    // one, and silently opening a different one instead would
                    // hide the typo behind an editor that looks like it worked.
                    error!("[splash] Failed to open --project: {}", e);
                    None
                }
            }
        }
        None => match untitled::open_or_create() {
            Ok(project) => {
                info!("[splash] opening the scratch project at {}", project.path.display());
                // Recorded in recents like any other project, which is the only
                // route back to it: open a real project from the overlay and
                // the untitled work would otherwise have no entry anywhere to
                // return to.
                app_config.add_recent_project(project.path.clone());
                let _ = app_config.save();
                Some(project)
            }
            Err(e) => {
                error!("[splash] {e}");
                None
            }
        },
    };

    let Some(project) = project else { return };

    // The dashboard goes up over the editor once it is ready, unless the project
    // was named on the command line. Armed here, where the answer is known,
    // rather than re-derived later from the arguments.
    if !named_on_the_command_line {
        commands.insert_resource(PendingSplashOverlay);
    }

    commands.insert_resource(project);
    commands.insert_resource(PendingProjectReopen);
}

/// Copy the open project somewhere the user picks, and open the copy.
///
/// The end of an untitled session: File > Create Project, after the save prompt
/// has written whatever was unsaved. The scene, the assets and the manifest are
/// all already on disk by this point, so the operation is a directory copy and
/// an ordinary project open, with no special case anywhere downstream. The new
/// project is a real one from the moment it exists, which is why nothing here
/// has to convert anything.
#[cfg(not(target_arch = "wasm32"))]
fn handle_request_create_project(
    mut commands: Commands,
    request: Option<Res<renzora::RequestCreateProjectFromCurrent>>,
    project: Option<Res<renzora::CurrentProject>>,
    mut app_config: ResMut<AppConfig>,
    mut next_state: ResMut<NextState<SplashState>>,
) {
    if request.is_none() {
        return;
    }
    commands.remove_resource::<renzora::RequestCreateProjectFromCurrent>();
    let Some(project) = project else { return };

    let Some(dest) = rfd::FileDialog::new()
        .set_title(renzora::lang::t_or(
            "splash.create_project_pick_folder",
            "Create Project: choose an empty folder",
        ))
        .pick_folder()
    else {
        return;
    };

    // Refusing rather than merging, because merging is how someone loses a
    // project: pick the wrong folder once and its scenes are overwritten by the
    // scratch ones with no way back.
    let occupied = std::fs::read_dir(&dest).map(|mut d| d.next().is_some()).unwrap_or(false);
    if occupied {
        rfd::MessageDialog::new()
            .set_title("Folder Not Empty")
            .set_description(format!(
                "{} already has something in it. Choose an empty folder, or make a new one.",
                dest.display()
            ))
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
        return;
    }

    let copied = match untitled::copy_project(&project.path, &dest) {
        Ok(copied) => copied,
        Err(e) => {
            error!("[splash] could not create the project at {}: {e}", dest.display());
            rfd::MessageDialog::new()
                .set_title("Could Not Create Project")
                .set_description(format!("{} could not be written:\n{e}", dest.display()))
                .set_buttons(rfd::MessageButtons::Ok)
                .show();
            return;
        }
    };

    // The name follows the folder, which is the only name the user has given it.
    // Written back into the copy's manifest so the project does not introduce
    // itself as "Untitled" for the rest of its life.
    let name = dest
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Project".to_string());
    let mut config = project.config.clone();
    config.name = name.clone();
    if let Ok(manifest) = toml::to_string_pretty(&config) {
        let _ = std::fs::write(dest.join("project.toml"), manifest);
    }

    info!("[splash] created {} from the scratch project ({copied} files)", dest.display());
    app_config.add_recent_project(dest.clone());
    let _ = app_config.save();
    commands.insert_resource(renzora::CurrentProject { path: dest, config });
    next_state.set(SplashState::Loading);
}

/// The browser has no folder to copy a project into.
#[cfg(target_arch = "wasm32")]
fn handle_request_create_project() {}

/// Keep [`UntitledProject`](renzora::UntitledProject) in step with whatever
/// project is open.
///
/// Every route that opens a project inserts `CurrentProject` and nothing else,
/// which is what lets there be five of them. This is the one place that decides
/// whether what was just opened is the scratch folder, so opening a real project
/// from the overlay clears the marker and File > Create Project stops offering
/// itself, without any of those routes knowing the marker exists.
#[cfg(not(target_arch = "wasm32"))]
fn track_untitled_marker(
    mut commands: Commands,
    project: Option<Res<renzora::CurrentProject>>,
    marked: Option<Res<renzora::UntitledProject>>,
) {
    let Some(project) = project else {
        // No project at all is a momentary state during startup, before the
        // system above has run. Leaving the marker alone rather than clearing
        // it keeps this from fighting whatever is mid-swap.
        return;
    };
    // Gated on the change tick and nothing else: `is_scratch` canonicalizes two
    // paths, which is filesystem work that has no business happening every
    // frame for an answer that only moves when the project does.
    if !project.is_changed() {
        return;
    }
    match (untitled::is_scratch(&project), marked.is_some()) {
        (true, false) => {
            commands.insert_resource(renzora::UntitledProject);
        }
        (false, true) => {
            commands.remove_resource::<renzora::UntitledProject>();
        }
        _ => {}
    }
}

/// Marker resource: splash should immediately transition back to editor
/// (e.g. project opened via File menu).
#[derive(Resource)]
pub struct PendingProjectReopen;

/// Marker resource: show the dashboard once the editor is up.
///
/// Set at startup and consumed by the first `OnEnter(SplashState::Editor)`, so
/// the overlay appears on launch and not after every subsequent project load.
#[derive(Resource)]
pub struct PendingSplashOverlay;

renzora::add!(SplashPlugin, Editor);
