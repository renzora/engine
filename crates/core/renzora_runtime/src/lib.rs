//! Renzora Runtime — game engine core without editor dependencies.
//!
//! Provides the game camera and core systems.
//! When the editor is present, it renders to an offscreen image.
//! When standalone, it renders directly to the window.

pub mod camera;
pub mod scene_io;
pub mod vfs;

pub use renzora_core::{CurrentProject, ProjectConfig, WindowConfig, open_project, DefaultCamera, EditorCamera, EditorLocked, HideInHierarchy, MeshColor, MeshPrimitive, PlayModeCamera, PlayModeState, PlayState, SceneCamera, ShapeEntry, ShapeRegistry, ViewportRenderTarget};
pub use vfs::Vfs;

// Re-export audio crate so downstream can use renzora_runtime::audio types
pub use renzora_audio;
// Re-export physics crate for downstream access
pub use renzora_physics;

use bevy::prelude::*;
use renzora_lighting::Sun;

/// Plugin that adds the game runtime: camera, scene, and core systems.
/// In non-editor mode, also handles project loading from CLI args.
pub struct RuntimePlugin;

impl Plugin for RuntimePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<MeshPrimitive>()
            .register_type::<MeshColor>()
            .register_type::<SceneCamera>()
            .register_type::<renzora_core::DefaultCamera>()
            .register_type::<renzora_core::EntityTag>()
            .register_type::<Sun>();

        #[cfg(not(feature = "editor"))]
        {
            // Try VFS first (rpak), then CLI --project, then local project.toml
            let vfs = Vfs::detect();

            if vfs.has_archive() {
                // Load project config from the rpak archive
                if let Some(toml_str) = vfs.read_string("project.toml") {
                    match toml::from_str::<ProjectConfig>(&toml_str) {
                        Ok(config) => {
                            // Extract archive to temp so scene_io can read scene files from disk
                            #[cfg(not(target_arch = "wasm32"))]
                            let project_path = vfs.extract_to_temp()
                                .unwrap_or_else(|| std::path::PathBuf::from("."));
                            #[cfg(target_arch = "wasm32")]
                            let project_path = std::path::PathBuf::from(".");
                            info!("Loaded project from rpak: {} (extracted to {})", config.name, project_path.display());
                            app.insert_resource(CurrentProject { path: project_path, config });
                        }
                        Err(e) => {
                            error!("Failed to parse project.toml from rpak: {}", e);
                        }
                    }
                } else {
                    error!("rpak archive has no project.toml");
                }
                app.insert_resource(vfs);
            } else {
                app.insert_resource(vfs);

                #[cfg(not(target_arch = "wasm32"))]
                let project_path = parse_project_arg()
                    .or_else(|| {
                        let local = std::path::PathBuf::from("project.toml");
                        if local.exists() { Some(local) } else { None }
                    });
                #[cfg(target_arch = "wasm32")]
                let project_path: Option<std::path::PathBuf> = None;

                if let Some(toml_path) = project_path {
                    match open_project(&toml_path) {
                        Ok(project) => {
                            info!("Loaded project: {} ({})", project.config.name, project.path.display());
                            app.insert_resource(project);
                        }
                        Err(e) => {
                            error!("Failed to load project from {}: {}", toml_path.display(), e);
                        }
                    }
                }
            }

            app.add_systems(Startup, scene_io::load_current_scene)
                .add_systems(Update, (scene_io::rehydrate_meshes, scene_io::rehydrate_suns, scene_io::rehydrate_visibility))
                .add_systems(Update, (scene_io::rehydrate_cameras, scene_io::enforce_single_active_camera)
                    .run_if(stinger_done));
        }

        app.init_resource::<ViewportRenderTarget>();
        app.init_resource::<ShapeRegistry>();

        #[cfg(feature = "editor")]
        {
            app.add_systems(Startup, camera::spawn_editor_camera)
                .add_systems(Update, camera::sync_camera_render_target);
        }
    }
}

/// Run condition: stinger is finished (or was never added).
#[cfg(not(feature = "editor"))]
fn stinger_done(state: Option<Res<State<renzora_stinger::StingerState>>>) -> bool {
    match state {
        Some(s) => *s.get() == renzora_stinger::StingerState::Game,
        None => true, // no stinger plugin
    }
}

#[cfg(all(not(feature = "editor"), not(target_arch = "wasm32")))]
fn parse_project_arg() -> Option<std::path::PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--project" {
            if let Some(path_str) = args.get(i + 1) {
                let path = std::path::PathBuf::from(path_str);
                let toml = if path.is_dir() {
                    path.join("project.toml")
                } else {
                    path
                };
                return Some(toml);
            }
        }
    }
    None
}
