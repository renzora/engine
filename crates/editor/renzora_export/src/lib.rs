//! Export overlay for packaging Renzora projects into distributable builds.
//!
//! Provides a modal overlay with export settings (platform, packaging mode,
//! window config, icon, etc.) and handles packing assets into `.rpak` archives
//! using pre-built runtime templates.

mod overlay;
mod templates;

pub use overlay::ExportOverlayState;
pub use templates::{ExportTemplate, TemplateManager, Platform};

use bevy::prelude::*;
use bevy::ecs::system::SystemState;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass};

pub struct ExportPlugin;

impl Plugin for ExportPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }

        app.init_resource::<ExportOverlayState>()
            .init_resource::<TemplateManager>()
            .add_systems(EguiPrimaryContextPass, export_overlay_system);
    }
}

fn export_overlay_system(world: &mut World) {
    let mut state = SystemState::<EguiContexts>::new(world);
    let mut contexts = state.get_mut(world);
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let ctx = ctx.clone();
    state.apply(world);

    // Check for ExportRequested marker from the editor menu
    if world.remove_resource::<renzora_core::ExportRequested>().is_some() {
        world.resource_mut::<ExportOverlayState>().visible = true;
    }

    let show = world.resource::<ExportOverlayState>().visible;
    if !show {
        return;
    }

    overlay::draw_export_overlay(world, &ctx);
}
