//! Export overlay for packaging Renzora projects into distributable builds.
//!
//! Provides a modal overlay with export settings (platform, packaging mode,
//! window config, icon, etc.) and handles packing assets into `.rpak` archives
//! using pre-built runtime templates.

#[cfg(not(target_arch = "wasm32"))]
mod apk_signer;
#[cfg(not(target_arch = "wasm32"))]
mod overlay;
#[cfg(not(target_arch = "wasm32"))]
mod templates;

#[cfg(not(target_arch = "wasm32"))]
pub use overlay::ExportOverlayState;
#[cfg(not(target_arch = "wasm32"))]
pub use templates::{ExportTemplate, TemplateManager, Platform};

use bevy::prelude::*;

#[derive(Default)]
pub struct ExportPlugin;

impl Plugin for ExportPlugin {
    fn build(&self, _app: &mut App) {
        info!("[editor] ExportPlugin");
        #[cfg(not(target_arch = "wasm32"))]
        {
            use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};

            if !_app.is_plugin_added::<EguiPlugin>() {
                _app.add_plugins(EguiPlugin::default());
            }

            _app.init_resource::<ExportOverlayState>()
                .init_resource::<TemplateManager>()
                .add_systems(EguiPrimaryContextPass, export_overlay_system);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource)]
struct ExportEguiState(bevy::ecs::system::SystemState<bevy_egui::EguiContexts<'static, 'static>>);

#[cfg(not(target_arch = "wasm32"))]
fn export_overlay_system(world: &mut World) {
    use bevy::ecs::system::SystemState;
    

    if !world.contains_resource::<ExportEguiState>() {
        let s = ExportEguiState(SystemState::new(world));
        world.insert_resource(s);
    }
    let mut cached = world.remove_resource::<ExportEguiState>().unwrap();
    let mut contexts = cached.0.get_mut(world);
    let Ok(ctx) = contexts.ctx_mut() else {
        world.insert_resource(cached);
        return;
    };
    let ctx = ctx.clone();
    cached.0.apply(world);
    world.insert_resource(cached);

    // Check for ExportRequested marker from the editor menu
    if world.remove_resource::<renzora::core::ExportRequested>().is_some() {
        world.resource_mut::<ExportOverlayState>().visible = true;
    }

    let show = world.resource::<ExportOverlayState>().visible;
    if !show {
        return;
    }

    overlay::draw_export_overlay(world, &ctx);
}

