//! Renzora Runtime — game engine core without editor dependencies.
//!
//! Provides the game camera and core systems.
//! When the editor is present, it renders to an offscreen image.
//! When standalone, it renders directly to the window.

pub mod camera;
pub mod scene_io;

pub use renzora_core::{CurrentProject, ProjectConfig, WindowConfig, open_project, EditorCamera, EditorLocked, HideInHierarchy, MeshColor, MeshPrimitive, SceneCamera, ViewportRenderTarget};

use bevy::prelude::*;
use renzora_lighting::SunData;

/// Plugin that adds the game runtime: camera, scene, and core systems.
/// In non-editor mode, also handles project loading from CLI args.
pub struct RuntimePlugin;

impl Plugin for RuntimePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<MeshPrimitive>()
            .register_type::<MeshColor>()
            .register_type::<SceneCamera>()
            .register_type::<SunData>();

        #[cfg(not(feature = "editor"))]
        {
            let project_path = parse_project_arg()
                .or_else(|| {
                    let local = std::path::PathBuf::from("project.toml");
                    if local.exists() { Some(local) } else { None }
                });

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

            app.add_systems(Startup, scene_io::load_current_scene)
                .add_systems(Update, (scene_io::rehydrate_meshes, scene_io::rehydrate_cameras, scene_io::rehydrate_suns));
        }

        app.init_resource::<ViewportRenderTarget>()
            .add_systems(Startup, camera::spawn_editor_camera)
            .add_systems(Update, camera::sync_camera_render_target);
    }
}

#[cfg(not(feature = "editor"))]
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
