pub mod config;
pub mod project;
mod ui;
#[cfg(target_arch = "wasm32")]
pub mod web_storage;

pub use config::AppConfig;
pub use project::{CurrentProject, ProjectConfig, WindowConfig, open_project};
#[cfg(not(target_arch = "wasm32"))]
pub use project::create_project;

use bevy::prelude::*;
use bevy::ecs::system::SystemState;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass};

/// Controls whether the splash screen or the editor is shown.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SplashState {
    #[default]
    Splash,
    Editor,
}

pub struct SplashPlugin;

impl Plugin for SplashPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }

        let app_config = AppConfig::load();

        app.init_state::<SplashState>()
            .insert_resource(app_config)
            .add_systems(
                EguiPrimaryContextPass,
                splash_ui_system.run_if(in_state(SplashState::Splash)),
            );
    }
}

/// Marker resource: splash should immediately transition back to editor
/// (e.g. project opened via File menu).
#[derive(Resource)]
pub struct PendingProjectReopen;

fn splash_ui_system(world: &mut World) {
    // If a project was opened from the editor File menu, skip splash and go straight back
    if world.remove_resource::<PendingProjectReopen>().is_some() {
        world
            .resource_mut::<NextState<SplashState>>()
            .set(SplashState::Editor);
        return;
    }

    // Get egui context
    let mut state = SystemState::<EguiContexts>::new(world);
    let mut contexts = state.get_mut(world);
    let ctx = match contexts.ctx_mut() {
        Ok(c) => c.clone(),
        Err(_) => return,
    };
    state.apply(world);

    // Extract mutable resources
    let mut app_config = world.remove_resource::<AppConfig>().unwrap_or_default();

    let mut sys_state = SystemState::<(Commands, ResMut<NextState<SplashState>>)>::new(world);
    let (mut commands, mut next_state) = sys_state.get_mut(world);

    ui::render_splash(&ctx, &mut app_config, &mut commands, &mut next_state);

    sys_state.apply(world);
    world.insert_resource(app_config);
}
