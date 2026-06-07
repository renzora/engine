pub mod auth;
pub mod config;
pub mod github;
pub mod loading;
mod native;
mod native_bg;
mod native_city;
mod native_loading;
pub mod project;
#[cfg(target_arch = "wasm32")]
pub mod web_storage;

pub use auth::SplashAuth;
pub use config::{AppConfig, UpdateConfig};
pub use github::GithubStats;
pub use loading::{
    EditorLoadingOverlayActive, LoadingTask, LoadingTaskHandle, LoadingTasks,
};
#[cfg(not(target_arch = "wasm32"))]
pub use project::create_project;
pub use project::{open_project, CurrentProject, ProjectConfig, WindowConfig};

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
            .insert_resource(SplashAuth::new())
            .insert_resource(GithubStats::new())
            .init_resource::<LoadingTasks>()
            .init_resource::<EditorLoadingOverlayActive>()
            .add_systems(
                Update,
                loading::auto_advance_to_editor.run_if(in_state(SplashState::Loading)),
            )
            .add_systems(Update, handle_request_open_project)
            .add_systems(OnEnter(SplashState::Loading), loading::log_loading_entered);

        // Dev shortcut: `--project <path>` skips the splash UI and jumps
        // straight into the project. This moved here from the binary's `main()`
        // when the editor became a removable bundle — the splash plugin lives
        // in the bundle, so the lean game binary no longer references it.
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(Startup, apply_project_arg);

        // Native (bevy_ui) splash, loading screen, background and city scene.
        native::register(app);
        native_loading::register(app);
        native_bg::register(app);
        native_city::register(app);
    }
}

/// Consume `renzora::RequestOpenProject` markers (inserted by the editor's
/// File menu). Owns the file dialog + validation + AppConfig update +
/// state transition so `renzora_editor` doesn't need to depend on splash.
#[cfg(not(target_arch = "wasm32"))]
fn handle_request_open_project(
    mut commands: Commands,
    request: Option<Res<renzora::RequestOpenProject>>,
    mut app_config: ResMut<AppConfig>,
    mut next_state: ResMut<NextState<SplashState>>,
) {
    if request.is_none() {
        return;
    }
    commands.remove_resource::<renzora::RequestOpenProject>();

    let Some(file) = rfd::FileDialog::new()
        .set_title("Open Project")
        .add_filter("Project File", &["toml"])
        .pick_file()
    else {
        return;
    };

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

/// Startup: honor a `--project <path>` argument by opening that project and
/// arming the splash to jump straight to Loading (skipping the UI). Inserts
/// `CurrentProject` + `PendingProjectReopen`; `native_reopen` picks the marker
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

/// Marker resource: splash should immediately transition back to editor
/// (e.g. project opened via File menu).
#[derive(Resource)]
pub struct PendingProjectReopen;

renzora::add!(SplashPlugin, Editor);
