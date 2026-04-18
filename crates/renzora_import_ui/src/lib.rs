//! Import overlay for converting and importing 3D models into Renzora projects.
//!
//! Provides a modal overlay triggered by file drops or the asset browser's
//! import button. All models are converted to GLB and placed in the project's
//! assets directory.

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod overlay;

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

    let auto_import = world
        .get_resource::<renzora_editor_framework::EditorSettings>()
        .map(|s| s.auto_import_on_drop)
        .unwrap_or(true);

    // Check for ImportRequested marker from the asset browser
    let import_requested = world
        .remove_resource::<renzora::core::ImportRequested>()
        .is_some();
    let requested_target = world.remove_resource::<renzora::core::ImportTargetDir>();

    if import_requested {
        if let Some(ref target) = requested_target {
            world.resource_mut::<overlay::ImportOverlayState>().target_directory = target.0.clone();
        }
        if !auto_import {
            world.resource_mut::<overlay::ImportOverlayState>().visible = true;
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
            if !auto_import {
                state.visible = true;
            }
        }
    }

    // Auto-import path: kick off the worker silently when files are pending
    // and the user has opted into auto-import. The overlay stays hidden.
    if auto_import {
        overlay::poll_import_task(world);

        let should_start = {
            let state = world.resource::<overlay::ImportOverlayState>();
            !state.pending_files.is_empty() && state.active_task.is_none()
        };
        if should_start {
            overlay::run_import(world);
        }

        // Reset idle terminal state so the next drop starts fresh.
        let done = matches!(
            world.resource::<overlay::ImportOverlayState>().progress,
            overlay::ImportProgress::Done(_) | overlay::ImportProgress::Error(_)
        );
        if done {
            let mut state = world.resource_mut::<overlay::ImportOverlayState>();
            state.pending_files.clear();
            state.progress = overlay::ImportProgress::Idle;
            state.log_entries.clear();
        }
        return;
    }

    let show = world.resource::<overlay::ImportOverlayState>().visible;
    if !show {
        return;
    }

    overlay::draw_import_overlay(world, &ctx);
}

renzora::add!(ImportPlugin, Editor);
