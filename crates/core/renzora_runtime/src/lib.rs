//! Renzora Runtime — game engine core without editor dependencies.
//!
//! Provides the game camera and core systems.
//! When the editor is present, it renders to an offscreen image.
//! When standalone, it renders directly to the window.

pub mod asset_reader;
pub mod camera;
pub mod crash;
pub mod debug_log;
pub mod scene_io;
pub mod vfs;

pub use asset_reader::{setup_asset_reader, ProjectAssetPath, SharedArchive};
pub use renzora_core::{CurrentProject, MeshInstanceData, PendingSceneLoad, ProjectConfig, WindowConfig, open_project, DefaultCamera, EditorCamera, EditorLocked, EffectRouting, HideInHierarchy, IsolatedCamera, MeshColor, MeshPrimitive, PlayModeCamera, PlayModeState, PlayState, SceneCamera, ShapeEntry, ShapeRegistry, ViewportRenderTarget};
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
        info!("[runtime] RuntimePlugin");
        app.register_type::<MeshPrimitive>()
            .register_type::<MeshColor>()
            .register_type::<MeshInstanceData>()
            .register_type::<SceneCamera>()
            .register_type::<renzora_core::DefaultCamera>()
            .register_type::<renzora_core::EntityTag>()
            .register_type::<Sun>();

        app.add_plugins(debug_log::DebugLogPlugin);

        #[cfg(not(feature = "editor"))]
        {
            // Try VFS first (rpak), then CLI --project, then local project.toml
            let vfs = Vfs::detect();

            if vfs.has_archive() {
                // Share the archive with the asset reader so it can serve
                // assets directly from memory (no temp extraction needed).
                if let Some(archive_arc) = vfs.archive_arc() {
                    if let Some(shared) = app.world().get_resource::<SharedArchive>() {
                        shared.set(archive_arc);
                    }
                }

                // Load project config from the rpak archive
                if let Some(toml_str) = vfs.read_string("project.toml") {
                    match toml::from_str::<ProjectConfig>(&toml_str) {
                        Ok(config) => {
                            info!("Loaded project from rpak: {}", config.name);
                            // Use a sentinel path — scene_io reads from Vfs, not disk.
                            let project_path = std::path::PathBuf::from(".");
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

            app.add_systems(Startup, (setup_vfs_script_reader, scene_io::load_current_scene).chain())
                .add_systems(Update, (
                    scene_io::rehydrate_meshes,
                    scene_io::rehydrate_suns,
                    scene_io::rehydrate_visibility,
                    scene_io::rehydrate_mesh_instances,
                    scene_io::finish_mesh_instance_rehydrate,
                ))
                .add_systems(Update, (scene_io::rehydrate_cameras, scene_io::sync_play_mode_camera, scene_io::enforce_single_active_camera));
        }

        // Keep ProjectAssetPath in sync with CurrentProject so the asset reader
        // always resolves from the correct project directory.
        app.add_systems(Update, sync_project_asset_path);

        app.init_resource::<ViewportRenderTarget>();
        app.init_resource::<ShapeRegistry>();
        app.init_resource::<renzora_core::EffectRouting>();
        app.init_resource::<renzora_core::PendingSceneLoad>();
        app.add_systems(Update, process_pending_scene_loads);

        // In standalone (non-editor) mode, populate EffectRouting from scene cameras.
        #[cfg(not(feature = "editor"))]
        {
            app.add_systems(Update, update_runtime_effect_routing);
        }

        #[cfg(feature = "editor")]
        {
            app.add_systems(Startup, camera::spawn_editor_camera)
                .add_systems(Update, camera::sync_camera_render_target);
        }
    }
}

/// Wire the VFS file reader into the scripting engine so scripts can be loaded
/// from rpak archives (Android, exported builds) instead of the filesystem.
#[cfg(not(feature = "editor"))]
fn setup_vfs_script_reader(
    vfs: Res<Vfs>,
    mut engine: Option<ResMut<renzora_scripting::ScriptEngine>>,
) {
    if !vfs.has_archive() { return; }
    let Some(ref mut engine) = engine else { return; };
    let vfs = vfs.clone();
    engine.set_file_reader(std::sync::Arc::new(move |path: &std::path::Path| {
        // Try archive-relative key: strip leading "./" and use forward slashes
        let key = path.to_string_lossy().replace('\\', "/");
        let key = key.trim_start_matches("./");
        vfs.read_string(key)
    }));
    info!("[runtime] VFS file reader set on scripting engine");
}

/// In standalone (non-editor) mode, route effects from the default scene camera
/// (and all non-camera entities with Settings) to the active rendering camera.
#[cfg(not(feature = "editor"))]
fn update_runtime_effect_routing(
    mut routing: ResMut<renzora_core::EffectRouting>,
    cameras: Query<(Entity, Option<&DefaultCamera>, &Camera), With<SceneCamera>>,
    all_entities: Query<Entity, Without<Camera>>,
) {
    // Find the active camera (DefaultCamera > first active SceneCamera)
    let active_cam = cameras
        .iter()
        .find(|(_, dc, cam)| dc.is_some() && cam.is_active)
        .or_else(|| cameras.iter().find(|(_, _, cam)| cam.is_active))
        .map(|(e, _, _)| e);

    let Some(target) = active_cam else {
        if !routing.routes.is_empty() {
            routing.routes.clear();
        }
        return;
    };

    // Sources: default camera entity itself + all non-camera entities (World Environment etc.)
    let mut sources: Vec<Entity> = vec![target];
    for entity in &all_entities {
        sources.push(entity);
    }

    let new_routes = vec![(target, sources)];
    if routing.routes != new_routes {
        routing.routes = new_routes;
    }
}

/// Process pending scene load requests from scripts/blueprints.
///
/// Clears the current scene (despawns all named non-editor entities),
/// then loads the requested scene.
fn process_pending_scene_loads(world: &mut World) {
    let requests = {
        let mut pending = world.resource_mut::<renzora_core::PendingSceneLoad>();
        if pending.requests.is_empty() {
            return;
        }
        std::mem::take(&mut pending.requests)
    };

    // Only process the last request if multiple were queued in one frame
    let scene_name = requests.last().unwrap();

    let scene_path = if let Some(project) = world.get_resource::<CurrentProject>() {
        project.resolve_path(scene_name)
    } else {
        renzora_core::console_log::console_error("Scene", "No project loaded — cannot load scene");
        return;
    };

    renzora_core::console_log::console_info(
        "Scene",
        format!("Loading scene '{}' → {}", scene_name, scene_path.display()),
    );

    // 1. Despawn all named non-editor entities (the current scene)
    let mut to_despawn = Vec::new();
    {
        let mut query = world.query_filtered::<Entity, (
            With<Name>,
            Without<EditorCamera>,
            Without<HideInHierarchy>,
        )>();
        for entity in query.iter(world) {
            to_despawn.push(entity);
        }
    }

    renzora_core::console_log::console_info(
        "Scene",
        format!("Despawning {} entities from current scene", to_despawn.len()),
    );

    for entity in to_despawn {
        if world.get_entity(entity).is_ok() {
            world.despawn(entity);
        }
    }

    // 2. Load the new scene
    scene_io::load_scene(world, &scene_path);
}

/// Keep `ProjectAssetPath` in sync whenever `CurrentProject` changes.
fn sync_project_asset_path(
    project: Option<Res<CurrentProject>>,
    asset_path: Option<Res<ProjectAssetPath>>,
) {
    let (Some(project), Some(asset_path)) = (project, asset_path) else {
        return;
    };
    if !project.is_changed() {
        return;
    }
    info!("[asset_reader] Project path set: {}", project.path.display());
    asset_path.set(project.path.clone());
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
