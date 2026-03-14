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

/// Cached SystemStates for the splash exclusive system (avoids per-frame allocation).
#[derive(Resource)]
struct SplashSystemStates {
    egui: SystemState<EguiContexts<'static, 'static>>,
    commands: SystemState<(Commands<'static, 'static>, ResMut<'static, NextState<SplashState>>)>,
}

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
        info!("[editor] SplashPlugin");
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
    info!("[splash] splash_ui_system running");

    // If a project was opened from the editor File menu, skip splash and go straight back
    if world.remove_resource::<PendingProjectReopen>().is_some() {
        world
            .resource_mut::<NextState<SplashState>>()
            .set(SplashState::Editor);
        return;
    }

    // Initialise cached system states on first run
    if !world.contains_resource::<SplashSystemStates>() {
        let states = SplashSystemStates {
            egui: SystemState::new(world),
            commands: SystemState::new(world),
        };
        world.insert_resource(states);
    }

    // Get egui context
    let mut states = world.remove_resource::<SplashSystemStates>().unwrap();
    let mut contexts = states.egui.get_mut(world);
    let ctx = match contexts.ctx_mut() {
        Ok(c) => {
            info!("[splash] Got egui context successfully");
            c.clone()
        }
        Err(e) => {
            warn!("[splash] Failed to get egui context: {:?}", e);
            world.insert_resource(states);
            return;
        }
    };
    states.egui.apply(world);

    // Extract mutable resources
    let mut app_config = world.remove_resource::<AppConfig>().unwrap_or_default();

    let (mut commands, mut next_state) = states.commands.get_mut(world);

    info!("[splash] About to render splash UI");
    ui::render_splash(&ctx, &mut app_config, &mut commands, &mut next_state);
    info!("[splash] Splash UI rendered successfully");

    states.commands.apply(world);
    world.insert_resource(app_config);
    world.insert_resource(states);
}
