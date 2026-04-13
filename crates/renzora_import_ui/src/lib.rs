//! Import overlay for converting and importing 3D models into Renzora projects.
//!
//! Provides a modal overlay triggered by file drops or the asset browser's
//! import button. All models are converted to GLB and placed in the project's
//! assets directory.

#[cfg(not(target_arch = "wasm32"))]
mod overlay;

use bevy::prelude::*;

#[derive(Default)]
pub struct ImportPlugin;

impl Plugin for ImportPlugin {
    fn build(&self, _app: &mut App) {
        info!("[editor] ImportPlugin");
        #[cfg(not(target_arch = "wasm32"))]
        {
            use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};

            if !_app.is_plugin_added::<EguiPlugin>() {
                _app.add_plugins(EguiPlugin::default());
            }

            _app.init_resource::<overlay::ImportOverlayState>()
                .add_systems(EguiPrimaryContextPass, import_overlay_system);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource)]
struct ImportEguiState(bevy::ecs::system::SystemState<bevy_egui::EguiContexts<'static, 'static>>);

#[cfg(not(target_arch = "wasm32"))]
fn import_overlay_system(world: &mut World) {
    use bevy::ecs::system::SystemState;
    

    if !world.contains_resource::<ImportEguiState>() {
        let s = ImportEguiState(SystemState::new(world));
        world.insert_resource(s);
    }
    let mut cached = world.remove_resource::<ImportEguiState>().unwrap();
    let mut contexts = cached.0.get_mut(world);
    let Ok(ctx) = contexts.ctx_mut() else {
        world.insert_resource(cached);
        return;
    };
    let ctx = ctx.clone();
    cached.0.apply(world);
    world.insert_resource(cached);

    // Check for ImportRequested marker from the asset browser
    if world
        .remove_resource::<renzora::core::ImportRequested>()
        .is_some()
    {
        world.resource_mut::<overlay::ImportOverlayState>().visible = true;

        // Pick up suggested target directory
        if let Some(target) = world.remove_resource::<renzora::core::ImportTargetDir>() {
            world.resource_mut::<overlay::ImportOverlayState>().target_directory = target.0;
        }
    }

    // Check for global file drops (3D model files)
    {
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| {
            i.raw.dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .filter(|p| renzora_import::formats::is_supported(p))
                .collect()
        });

        if !dropped.is_empty() {
            let mut state = world.resource_mut::<overlay::ImportOverlayState>();
            if !state.visible {
                state.visible = true;
            }
            let was_empty = state.pending_files.is_empty();
            for path in &dropped {
                if !state.pending_files.contains(path) {
                    state.pending_files.push(path.clone());
                }
            }
            // Auto-detect unit scale from the first file
            if was_empty && state.settings.scale == 1.0 {
                if let Some(scale) = renzora_import::units::detect_unit_scale(&dropped[0]) {
                    state.settings.scale = scale;
                }
            }
        }
    }

    // Draw hover hint when dragging 3D files over the editor
    {
        let has_3d_hover = ctx.input(|i| {
            i.raw
                .hovered_files
                .iter()
                .any(|f| {
                    f.path
                        .as_ref()
                        .map(|p| renzora_import::formats::is_supported(p))
                        .unwrap_or(false)
                })
        });

        if has_3d_hover && !world.resource::<overlay::ImportOverlayState>().visible {
            let screen = ctx.input(|i| i.viewport_rect());
            let painter = ctx.layer_painter(bevy_egui::egui::LayerId::new(
                bevy_egui::egui::Order::Foreground,
                bevy_egui::egui::Id::new("import_drop_hint"),
            ));
            painter.rect_filled(
                screen,
                0.0,
                bevy_egui::egui::Color32::from_rgba_premultiplied(30, 80, 200, 40),
            );
            painter.rect_stroke(
                screen.shrink(4.0),
                8.0,
                bevy_egui::egui::Stroke::new(2.0, bevy_egui::egui::Color32::from_rgb(80, 140, 255)),
                bevy_egui::egui::StrokeKind::Outside,
            );
            painter.text(
                screen.center(),
                bevy_egui::egui::Align2::CENTER_CENTER,
                "Drop 3D model to import",
                bevy_egui::egui::FontId::proportional(20.0),
                bevy_egui::egui::Color32::from_rgb(180, 210, 255),
            );
        }
    }

    let show = world.resource::<overlay::ImportOverlayState>().visible;
    if !show {
        return;
    }

    overlay::draw_import_overlay(world, &ctx);
}

renzora::add!(ImportPlugin, Editor);
