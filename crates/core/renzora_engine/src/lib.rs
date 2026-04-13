//! Renzora Runtime — game engine core without editor dependencies.
//!
//! Provides the game camera and core systems.
//! When the editor is present, it renders to an offscreen image.
//! When standalone, it renders directly to the window.

pub mod asset_reader;
pub mod camera;
pub mod crash;
pub mod debug_log;
pub mod procedural_meshes;
pub mod scene_io;
pub mod vfs;

pub use asset_reader::{setup_asset_reader, ProjectAssetPath, SharedArchive};
pub use renzora_core::{CurrentProject, MeshInstanceData, PendingSceneLoad, ProjectConfig, WindowConfig, open_project, DefaultCamera, EditorCamera, EditorLocked, EffectRouting, HideInHierarchy, IsolatedCamera, MeshColor, MeshPrimitive, PlayModeCamera, PlayModeState, PlayState, SceneCamera, ShapeEntry, ShapeRegistry, ViewportRenderTarget};
pub use vfs::Vfs;

// Re-export audio crate so downstream can use renzora_engine::audio types
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

        // Asset-path rename/move notifications. Observers (MeshInstanceData,
        // AnimatorComponent, etc.) listen and patch stored asset-relative
        // paths so moved assets don't leave dangling references in the scene.
        app.add_observer(apply_asset_path_changes_to_mesh_instances);

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
                // Provide a VirtualFileReader backed by Vfs so material/shader
                // resolution reads from the rpak archive instead of disk.
                let vfs_for_reader = vfs.clone();
                app.insert_resource(renzora_core::VirtualFileReader::new(move |path| {
                    vfs_for_reader.read_string(path)
                }));
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
        {
            use bevy::prelude::*;
            use procedural_meshes as pm;
            let mut reg = ShapeRegistry::default();
            // Basic
            reg.register(ShapeEntry { id: "cube", name: "Cube", icon: "", category: "Basic", create_mesh: |m| m.add(Cuboid::new(1.0, 1.0, 1.0)), default_color: Color::srgb(0.8, 0.3, 0.2) });
            reg.register(ShapeEntry { id: "sphere", name: "Sphere", icon: "", category: "Basic", create_mesh: |m| m.add(Sphere::new(0.5).mesh().ico(5).unwrap()), default_color: Color::srgb(0.2, 0.5, 0.8) });
            reg.register(ShapeEntry { id: "cylinder", name: "Cylinder", icon: "", category: "Basic", create_mesh: |m| m.add(Cylinder::new(0.5, 1.0)), default_color: Color::srgb(0.3, 0.7, 0.4) });
            reg.register(ShapeEntry { id: "plane", name: "Plane", icon: "", category: "Basic", create_mesh: |m| m.add(Plane3d::default().mesh().size(2.0, 2.0)), default_color: Color::srgb(0.35, 0.35, 0.35) });
            reg.register(ShapeEntry { id: "cone", name: "Cone", icon: "", category: "Basic", create_mesh: |m| m.add(Cone { radius: 0.5, height: 1.0 }), default_color: Color::srgb(0.7, 0.5, 0.2) });
            reg.register(ShapeEntry { id: "torus", name: "Torus", icon: "", category: "Basic", create_mesh: |m| m.add(Torus { minor_radius: 0.15, major_radius: 0.35 }), default_color: Color::srgb(0.6, 0.3, 0.7) });
            reg.register(ShapeEntry { id: "capsule", name: "Capsule", icon: "", category: "Basic", create_mesh: |m| m.add(Capsule3d::new(0.25, 0.5)), default_color: Color::srgb(0.3, 0.6, 0.6) });
            reg.register(ShapeEntry { id: "hemisphere", name: "Hemisphere", icon: "", category: "Basic", create_mesh: |m| m.add(pm::create_hemisphere_mesh(16)), default_color: Color::srgb(0.5, 0.4, 0.7) });
            // Level
            reg.register(ShapeEntry { id: "wedge", name: "Wedge", icon: "", category: "Level", create_mesh: |m| m.add(pm::create_wedge_mesh()), default_color: Color::srgb(0.6, 0.6, 0.5) });
            reg.register(ShapeEntry { id: "stairs", name: "Stairs", icon: "", category: "Level", create_mesh: |m| m.add(pm::create_stairs_mesh(6)), default_color: Color::srgb(0.5, 0.5, 0.6) });
            reg.register(ShapeEntry { id: "arch", name: "Arch", icon: "", category: "Level", create_mesh: |m| m.add(pm::create_arch_mesh(16)), default_color: Color::srgb(0.6, 0.5, 0.4) });
            reg.register(ShapeEntry { id: "half_cylinder", name: "Half Cylinder", icon: "", category: "Level", create_mesh: |m| m.add(pm::create_half_cylinder_mesh(16)), default_color: Color::srgb(0.5, 0.6, 0.5) });
            reg.register(ShapeEntry { id: "quarter_pipe", name: "Quarter Pipe", icon: "", category: "Level", create_mesh: |m| m.add(pm::create_quarter_pipe_mesh(16)), default_color: Color::srgb(0.55, 0.55, 0.5) });
            reg.register(ShapeEntry { id: "corner", name: "Corner", icon: "", category: "Level", create_mesh: |m| m.add(pm::create_corner_mesh()), default_color: Color::srgb(0.5, 0.5, 0.55) });
            reg.register(ShapeEntry { id: "wall", name: "Wall", icon: "", category: "Level", create_mesh: |m| m.add(Cuboid::new(1.0, 2.0, 0.1)), default_color: Color::srgb(0.55, 0.5, 0.5) });
            reg.register(ShapeEntry { id: "ramp", name: "Ramp", icon: "", category: "Level", create_mesh: |m| m.add(pm::create_ramp_mesh()), default_color: Color::srgb(0.5, 0.55, 0.5) });
            reg.register(ShapeEntry { id: "curved_wall", name: "Curved Wall", icon: "", category: "Level", create_mesh: |m| m.add(pm::create_curved_wall_mesh(16)), default_color: Color::srgb(0.55, 0.55, 0.55) });
            reg.register(ShapeEntry { id: "doorway", name: "Doorway", icon: "", category: "Level", create_mesh: |m| m.add(pm::create_doorway_mesh()), default_color: Color::srgb(0.5, 0.5, 0.6) });
            reg.register(ShapeEntry { id: "window_wall", name: "Window Wall", icon: "", category: "Level", create_mesh: |m| m.add(pm::create_window_wall_mesh()), default_color: Color::srgb(0.5, 0.55, 0.55) });
            reg.register(ShapeEntry { id: "l_shape", name: "L-Shape", icon: "", category: "Level", create_mesh: |m| m.add(pm::create_l_shape_mesh()), default_color: Color::srgb(0.55, 0.5, 0.55) });
            reg.register(ShapeEntry { id: "t_shape", name: "T-Shape", icon: "", category: "Level", create_mesh: |m| m.add(pm::create_t_shape_mesh()), default_color: Color::srgb(0.5, 0.55, 0.6) });
            reg.register(ShapeEntry { id: "cross_shape", name: "Cross", icon: "", category: "Level", create_mesh: |m| m.add(pm::create_cross_shape_mesh()), default_color: Color::srgb(0.55, 0.55, 0.6) });
            reg.register(ShapeEntry { id: "spiral_stairs", name: "Spiral Stairs", icon: "", category: "Level", create_mesh: |m| m.add(pm::create_spiral_stairs_mesh(16)), default_color: Color::srgb(0.5, 0.5, 0.55) });
            reg.register(ShapeEntry { id: "pillar", name: "Pillar", icon: "", category: "Level", create_mesh: |m| m.add(pm::create_pillar_mesh()), default_color: Color::srgb(0.55, 0.5, 0.5) });
            // Curved
            reg.register(ShapeEntry { id: "pipe", name: "Pipe", icon: "", category: "Curved", create_mesh: |m| m.add(pm::create_pipe_mesh(24)), default_color: Color::srgb(0.4, 0.5, 0.6) });
            reg.register(ShapeEntry { id: "ring", name: "Ring", icon: "", category: "Curved", create_mesh: |m| m.add(pm::create_ring_mesh(24)), default_color: Color::srgb(0.5, 0.4, 0.6) });
            reg.register(ShapeEntry { id: "funnel", name: "Funnel", icon: "", category: "Curved", create_mesh: |m| m.add(pm::create_funnel_mesh(24)), default_color: Color::srgb(0.6, 0.4, 0.5) });
            reg.register(ShapeEntry { id: "gutter", name: "Gutter", icon: "", category: "Curved", create_mesh: |m| m.add(pm::create_gutter_mesh(16)), default_color: Color::srgb(0.4, 0.6, 0.5) });
            // Advanced
            reg.register(ShapeEntry { id: "prism", name: "Prism", icon: "", category: "Advanced", create_mesh: |m| m.add(pm::create_prism_mesh()), default_color: Color::srgb(0.5, 0.5, 0.7) });
            reg.register(ShapeEntry { id: "pyramid", name: "Pyramid", icon: "", category: "Advanced", create_mesh: |m| m.add(pm::create_pyramid_mesh()), default_color: Color::srgb(0.7, 0.5, 0.5) });
            app.insert_resource(reg);
        }
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
            app.init_resource::<renzora_core::viewport_types::EditorCameraMatrix>()
                .add_systems(Startup, camera::spawn_editor_camera)
                .add_systems(Update, (
                    camera::sync_camera_render_target,
                    camera::update_editor_camera_matrix,
                ));
            // Listen for save-scene event from the editor
            app.add_observer(on_save_current_scene);
        }
    }
}

#[cfg(feature = "editor")]
fn on_save_current_scene(
    _trigger: On<renzora_core::SaveCurrentScene>,
    mut commands: Commands,
) {
    commands.queue(|world: &mut World| {
        scene_io::save_current_scene(world);
    });
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

/// Rewrites [`MeshInstanceData::model_path`] on every entity when an asset
/// is renamed or moved, so scene references stay valid without a user-
/// initiated save. Animation paths are handled analogously in `renzora_animation`.
fn apply_asset_path_changes_to_mesh_instances(
    trigger: On<renzora_core::AssetPathChanged>,
    mut query: Query<&mut MeshInstanceData>,
) {
    let ev = trigger.event();
    for mut data in query.iter_mut() {
        if let Some(ref path) = data.model_path {
            if let Some(new_path) = ev.rewrite(path) {
                info!(
                    "[asset-move] rewriting MeshInstanceData '{}' → '{}'",
                    path, new_path
                );
                data.model_path = Some(new_path);
            }
        }
    }
}
