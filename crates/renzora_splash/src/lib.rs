pub mod auth;
pub mod config;
pub mod github;
pub mod loading;
pub mod project;
mod ui;
#[cfg(target_arch = "wasm32")]
pub mod web_storage;

pub use auth::SplashAuth;
pub use config::{AppConfig, UpdateConfig};
pub use github::GithubStats;
pub use loading::{LoadingTask, LoadingTaskHandle, LoadingTasks};
pub use project::{CurrentProject, ProjectConfig, WindowConfig, open_project};
#[cfg(not(target_arch = "wasm32"))]
pub use project::create_project;

use bevy::prelude::*;
use bevy::app::AppExit;
use bevy::ecs::system::SystemState;
use bevy::window::{PrimaryWindow, Window};
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};

pub use ui::WindowAction;

#[derive(Resource)]
struct SplashSystemStates {
    egui: SystemState<EguiContexts<'static, 'static>>,
    commands: SystemState<(Commands<'static, 'static>, ResMut<'static, NextState<SplashState>>)>,
}

/// Flag so we only register the phosphor font once.
#[derive(Resource, Default)]
struct PhosphorFontInstalled(bool);

/// Tracks whether the borderless splash window is currently maximized.
/// Bevy's Window component only exposes `set_maximized`, so we mirror state
/// locally to toggle correctly.
#[derive(Resource, Default)]
pub struct SplashWindowState {
    pub maximized: bool,
}

// SplashState now lives in the `renzora` SDK — coordination contract used
// by both the splash UI and the editor framework. Re-exported here for
// back-compat so existing `renzora_splash::SplashState` paths keep working.
pub use renzora::SplashState;

pub struct SplashPlugin;

impl Plugin for SplashPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] SplashPlugin");
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }

        let app_config = AppConfig::load();

        app.init_state::<SplashState>()
            .insert_resource(app_config)
            .insert_resource(SplashAuth::new())
            .insert_resource(GithubStats::new())
            .init_resource::<PhosphorFontInstalled>()
            .init_resource::<SplashWindowState>()
            .init_resource::<LoadingTasks>()
            .add_systems(
                EguiPrimaryContextPass,
                splash_ui_system.run_if(in_state(SplashState::Splash)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                loading::loading_ui_system.run_if(in_state(SplashState::Loading)),
            )
            .add_systems(
                Update,
                loading::auto_advance_to_editor.run_if(in_state(SplashState::Loading)),
            )
            .add_systems(Update, handle_request_open_project)
            .add_systems(OnEnter(SplashState::Loading), loading::log_loading_entered);
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
                .set_description(&format!("Failed to open project: {}", e))
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

/// Compact size used by the splash launcher overlay. Shared constant so the
/// splash and editor paths agree on loader dimensions.
pub const LOADING_WINDOW_SIZE: (f32, f32) = (600.0, 180.0);

/// Marker resource: splash should immediately transition back to editor
/// (e.g. project opened via File menu).
#[derive(Resource)]
pub struct PendingProjectReopen;

/// Install the phosphor icon font into egui so icon characters render.
/// Called once on the first splash frame.
fn install_phosphor_font(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "phosphor".into(),
        egui::FontData::from_static(egui_phosphor::Variant::Regular.font_bytes()).into(),
    );
    if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        fam.push("phosphor".into());
    }
    if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        fam.push("phosphor".into());
    }
    ctx.set_fonts(fonts);
}

fn splash_ui_system(world: &mut World) {
    if world.remove_resource::<PendingProjectReopen>().is_some() {
        world
            .resource_mut::<NextState<SplashState>>()
            .set(SplashState::Loading);
        return;
    }

    if !world.contains_resource::<SplashSystemStates>() {
        let states = SplashSystemStates {
            egui: SystemState::new(world),
            commands: SystemState::new(world),
        };
        world.insert_resource(states);
    }

    let mut states = world.remove_resource::<SplashSystemStates>().unwrap();
    let mut contexts = states.egui.get_mut(world);
    let ctx = match contexts.ctx_mut() {
        Ok(c) => c.clone(),
        Err(e) => {
            warn!("[splash] Failed to get egui context: {:?}", e);
            world.insert_resource(states);
            return;
        }
    };
    states.egui.apply(world);

    if let Some(mut flag) = world.get_resource_mut::<PhosphorFontInstalled>() {
        if !flag.0 {
            install_phosphor_font(&ctx);
            flag.0 = true;
        }
    }

    let mut app_config = world.remove_resource::<AppConfig>().unwrap_or_default();
    let mut splash_auth = world.remove_resource::<SplashAuth>().unwrap_or_default();
    let mut github_stats = world.remove_resource::<GithubStats>().unwrap_or_default();
    let mut win_state = world.remove_resource::<SplashWindowState>().unwrap_or_default();

    let (mut commands, mut next_state) = states.commands.get_mut(world);

    let action = ui::render_splash(
        &ctx,
        &mut app_config,
        &mut splash_auth,
        &mut github_stats,
        &win_state,
        &mut commands,
        &mut next_state,
    );

    states.commands.apply(world);
    world.insert_resource(app_config);
    world.insert_resource(splash_auth);
    world.insert_resource(github_stats);
    world.insert_resource(states);

    // Apply the window action the UI requested.
    match action {
        WindowAction::None => {}
        WindowAction::Close => {
            world.write_message(AppExit::Success);
        }
        WindowAction::Minimize => {
            with_primary_window(world, |w| w.set_minimized(true));
        }
        WindowAction::ToggleMaximize => {
            win_state.maximized = !win_state.maximized;
            let max = win_state.maximized;
            with_primary_window(world, |w| w.set_maximized(max));
        }
        WindowAction::StartDrag => {
            // Standard OS behaviour: dragging the title of a maximized window
            // restores it first so the drag actually moves the window.
            let was_maximized = win_state.maximized;
            if was_maximized {
                win_state.maximized = false;
            }
            with_primary_window(world, move |w| {
                if was_maximized {
                    w.set_maximized(false);
                }
                w.start_drag_move();
            });
        }
        WindowAction::StartResize(dir) => {
            with_primary_window(world, move |w| w.start_drag_resize(dir));
        }
    }
    world.insert_resource(win_state);
}

fn with_primary_window(world: &mut World, f: impl FnOnce(&mut Window)) {
    let mut q = world.query_filtered::<&mut Window, With<PrimaryWindow>>();
    if let Ok(mut w) = q.single_mut(world) {
        f(&mut *w);
    }
}
