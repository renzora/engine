//! Renzora Runtime — game engine core without editor dependencies.
//!
//! Provides the game camera and core systems.
//! When the editor is present, it renders to an offscreen image.
//! When standalone, it renders directly to the window.

pub mod asset_progress;
pub mod asset_reader;
pub mod autoload;
pub mod camera;
pub mod crash;
pub mod debug_log;
pub mod procedural_meshes;
pub mod scene_io;
pub mod vfs;

pub use asset_progress::{AssetLoadProgress, LoadProgressState};
pub use asset_reader::{setup_asset_reader, ProjectAssetPath, SharedArchive};
pub use renzora::{
    open_project, CurrentProject, DefaultCamera, EditorCamera, EditorCamera2d, EditorLocked,
    EffectRouting, HideInHierarchy, IsolatedCamera, MeshColor, MeshInstanceData, MeshPrimitive,
    PendingSceneLoad, Persistent, PlayModeCamera, PlayModeState, PlayState, ProjectConfig,
    EnvironmentBakeCamera, PrimaryViewportCamera, RenderingMode, ResolvedRenderingMode, SceneCamera,
    ShapeEntry, ShapeRegistry, ViewportCamera, ViewportRenderTarget, WindowConfig,
};
pub use vfs::Vfs;

// Re-export audio crate so downstream can use renzora_engine::audio types
pub use renzora_audio;
// Re-export physics crate for downstream access
pub use renzora_physics;

use bevy::core_pipeline::prepass::DeferredPrepass;
use bevy::pbr::DefaultOpaqueRendererMethod;
use bevy::prelude::*;
use renzora_lighting::Sun;

/// Set `DefaultOpaqueRendererMethod` to match the resolved rendering
/// mode. Bevy's PbrPlugin inserts a Forward default during its own
/// build; we override here so materials follow our resolved choice.
/// `insert_resource` is idempotent, so calling this multiple times is
/// safe.
fn apply_rendering_mode(app: &mut App, mode: RenderingMode) {
    match mode {
        RenderingMode::Deferred => {
            app.insert_resource(DefaultOpaqueRendererMethod::deferred());
        }
        // Auto should already be resolved at this point — treat any
        // non-Deferred as Forward (which it semantically is).
        _ => {
            app.insert_resource(DefaultOpaqueRendererMethod::forward());
        }
    }
}

/// Safety net: when the project is in Deferred mode, every `Camera3d`
/// in the scene needs `DeferredPrepass` so its prepass queue has the
/// deferred opaque phase. The main editor camera attaches it explicitly
/// at spawn (see `spawn_editor_camera`), but the editor also spins up
/// many auxiliary 3D cameras — material previews, particle/shader
/// previews, model+camera+canvas previews, animation studio, asset-
/// browser and material-editor thumbnails — each in its own crate.
/// Without DeferredPrepass on every one of them, `queue_prepass_material_meshes`
/// panics when a material's `OpaqueRendererMethod::Auto` resolves to
/// Deferred via the global default but the view has no deferred phase.
///
/// We also force `Msaa::Off` on each one. Deferred shading writes the
/// G-buffer at 1× while the depth attachment would be MSAA-resolved —
/// wgpu rejects the mismatched sample counts ("depth view count 4 but
/// color view count 1"). Several thumbnail / preview cameras opt into
/// 4× MSAA for crisper small-resolution renders; in Deferred mode they
/// have to give it up. The visual cost on a 128px thumbnail is minor.
///
/// We deliberately do NOT use an `Added<Camera3d>` filter. The resolved
/// rendering mode is `Forward` at app startup (default) and only flips
/// to `Deferred` once the project loads via `sync_rendering_mode_from_project`
/// at `OnEnter(SplashState::Editor)`. Any `Camera3d` spawned *before*
/// that moment (asset-browser/material thumbnails during splash, GLB-
/// embedded camera nodes added by scene rehydration during `Loading`,
/// etc.) was seen by an `Added<>` query when the mode check still
/// returned `false`, and would never be revisited. The `Without<DeferredPrepass>`
/// filter is what makes scanning every frame cheap: as soon as the
/// marker is on an entity, the query stops returning it.
fn ensure_deferred_prepass_on_cameras(
    rendering_mode: Res<ResolvedRenderingMode>,
    cameras: Query<Entity, (With<Camera3d>, Without<DeferredPrepass>)>,
    mut commands: Commands,
) {
    if !rendering_mode.is_deferred() {
        return;
    }
    for entity in &cameras {
        commands
            .entity(entity)
            .try_insert((DeferredPrepass, Msaa::Off));
    }
}

/// Attach the `ContactShadows` receiver to FORWARD cameras only.
///
/// Bevy 0.19's *deferred* lighting pipeline doesn't wire up contact shadows
/// (`prepare_deferred_lighting_pipelines` never queries `Has<ContactShadows>`,
/// so the deferred pipeline's mesh-view layout omits the contact-shadows binding
/// and mismatches the camera's bind group — an upstream bug, see the bevy docs
/// which claim deferred support). The *forward* mesh pipeline specializes it
/// correctly, so we only attach the receiver when the resolved mode is forward.
/// A light's `contact_shadows_enabled` (e.g. the Sun's toggle) then takes effect
/// on these views. Same cheap `Without<>` scan + before-first-render timing as
/// [`ensure_deferred_prepass_on_cameras`].
///
/// `Without<IsolatedCamera>` is load-bearing: contact shadows are a main-view
/// feature, and the offscreen utility cameras (material/model thumbnails, studio
/// previews, env bakes, the game-UI canvas) all carry `IsolatedCamera`. Attaching
/// `ContactShadows` to one of those — e.g. the material-thumbnail capture camera —
/// makes its mesh-view bind group expose binding 16 (`ContactShadowsUniform`)
/// while that render path's `pbr_opaque_mesh_pipeline` specializes *without* the
/// `CONTACT_SHADOWS` key (binding 16 absent), so wgpu hard-quits with a layout
/// mismatch. `renzora_skybox`/`renzora_night_stars` exclude these same cameras.
fn ensure_contact_shadows_on_forward_cameras(
    rendering_mode: Res<ResolvedRenderingMode>,
    add_cameras: Query<
        Entity,
        (
            With<Camera3d>,
            Without<bevy::pbr::ContactShadows>,
            Without<IsolatedCamera>,
            Without<DeferredPrepass>,
        ),
    >,
    // Cameras that must NOT keep `ContactShadows`: a deferred prepass (whose
    // lighting pipeline omits binding 16) or an isolated utility view.
    conflicting: Query<
        Entity,
        (
            With<bevy::pbr::ContactShadows>,
            Or<(With<DeferredPrepass>, With<IsolatedCamera>)>,
        ),
    >,
    mut commands: Commands,
) {
    // Strip `ContactShadows` from any camera it conflicts with, no matter how it
    // got there — a Forward→Deferred mode switch, a `DeferredPrepass` attached
    // later (e.g. for SSR), or a scene that saved the component while forward.
    // Without this, such a camera's mesh-view bind group exposes binding 16
    // while the deferred lighting pipeline's layout omits it → wgpu hard-crash.
    for entity in &conflicting {
        commands
            .entity(entity)
            .remove::<bevy::pbr::ContactShadows>();
    }
    if rendering_mode.is_deferred() {
        return;
    }
    for entity in &add_cameras {
        commands
            .entity(entity)
            .try_insert(bevy::pbr::ContactShadows::default());
    }
}

/// Render-world companion to [`ensure_contact_shadows_on_forward_cameras`] that
/// closes a one-frame wgpu crash window in Bevy 0.19's contact-shadows path.
///
/// Bevy decides a mesh pipeline's `CONTACT_SHADOWS` key in
/// `check_views_need_specialization` (render set `PrepareAssets`) by testing for
/// a `ViewContactShadowsUniformOffset` on the view — but that offset isn't
/// written until `prepare_contact_shadows_settings` (set `PrepareResources`,
/// which the set chain `ExtractCommands → PrepareAssets → … → Prepare` runs
/// *later* the same frame). So the first frame a forward camera gains
/// `ContactShadows`, the opaque mesh pipeline specializes WITHOUT binding 16,
/// while that same frame's mesh-view bind group *does* emit binding 16. wgpu
/// validates the two layouts, finds binding 16 in the bind group but not the
/// pipeline, and hard-crashes ("Assigned entry with binding 16 not found in
/// expected bind group layout").
///
/// Our `PostUpdate` guard attaches `ContactShadows` reactively to the already
/// live editor camera, so it hits this window every time. Seeding a placeholder
/// offset *before* `check_views_need_specialization` makes the pipeline key
/// include `CONTACT_SHADOWS` from frame one; `prepare_contact_shadows_settings`
/// overwrites the real value later the same frame, before the bind group is
/// built. Fixes the crash for reactive attach, camera spawn, and
/// Forward↔Deferred switches alike.
fn seed_contact_shadows_offset(
    mut commands: Commands,
    views: Query<
        Entity,
        (
            With<bevy::render::view::ExtractedView>,
            With<bevy::pbr::ContactShadows>,
            Without<bevy::pbr::ViewContactShadowsUniformOffset>,
        ),
    >,
) {
    for entity in &views {
        commands
            .entity(entity)
            .insert(bevy::pbr::ViewContactShadowsUniformOffset(0));
    }
}

/// Pull the rendering mode from the just-loaded project and propagate
/// it to `ResolvedRenderingMode` + `DefaultOpaqueRendererMethod`
/// **before** the editor camera spawns. Runs as the first step of
/// `OnEnter(SplashState::Editor)`, chained ahead of camera spawn so
/// the prepass attachments reflect the project's choice.
///
/// No-op when there's no project (engine started without one — splash
/// keeps spinning or the user backed out).
pub fn sync_rendering_mode_from_project(
    project: Option<Res<CurrentProject>>,
    mut resolved: ResMut<ResolvedRenderingMode>,
    mut default_method: ResMut<DefaultOpaqueRendererMethod>,
) {
    let Some(project) = project else { return; };
    let mode = project.config.rendering.mode.resolve();
    resolved.0 = mode;
    *default_method = match mode {
        RenderingMode::Deferred => DefaultOpaqueRendererMethod::deferred(),
        _ => DefaultOpaqueRendererMethod::forward(),
    };
    info!("[runtime] rendering mode synced from project: {:?}", mode);
}

/// Plugin that adds the game runtime: camera, scene, and core systems.
/// In non-editor mode, also handles project loading from CLI args.
#[derive(Default)]
pub struct RuntimePlugin;

impl Plugin for RuntimePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] RuntimePlugin");
        // Editor-vs-game branch. `renzora_engine` is compiled lean (no `editor`
        // cargo feature), so the editor/runtime split that used to be
        // `#[cfg]`-gated is decided at RUNTIME from `EditorSession`, inserted by
        // `add_engine_plugins(is_editor)` before this plugin builds. Defaults to
        // game (`false`) if absent — the safe shipping behaviour.
        let is_editor = app
            .world()
            .get_resource::<renzora::EditorSession>()
            .map(|s| s.0)
            .unwrap_or(false);
        // Default rendering mode (auto-resolved by platform). Gets
        // overridden below if `project.toml` specifies an explicit
        // mode. Must exist before camera spawn so the spawn site can
        // decide whether to attach `DeferredPrepass`.
        app.init_resource::<ResolvedRenderingMode>();
        let initial_mode = app.world().resource::<ResolvedRenderingMode>().0;
        info!("[runtime] default rendering mode: {:?}", initial_mode);
        apply_rendering_mode(app, initial_mode);
        app.register_type::<MeshPrimitive>()
            .register_type::<MeshColor>()
            .register_type::<MeshInstanceData>()
            .register_type::<SceneCamera>()
            .register_type::<renzora::SceneInstance>()
            .register_type::<renzora::DefaultCamera>()
            .register_type::<renzora::core::CameraRenderResolution>()
            .register_type::<renzora::core::viewport_types::RenderResolution>()
            .register_type::<renzora::CameraPreset>()
            .register_type::<renzora::CameraPresets>()
            .register_type::<renzora::EntityTag>()
            .register_type::<renzora::Persistent>()
            .register_type::<renzora::core::Node2d>()
            .register_type::<renzora::core::SpriteImagePath>()
            .register_type::<renzora::core::ReflectionProbeSource>()
            .register_type::<renzora::WorldEnvironment>()
            .register_type::<Sun>();

        // Register the .rmip asset loader so import-baked mipmapped
        // textures can be loaded via `asset_server.load("...rmip")`.
        app.init_asset_loader::<renzora_rmip::RmipAssetLoader>();

        // Asset-path rename/move notifications. Observers (MeshInstanceData,
        // AnimatorComponent, etc.) listen and patch stored asset-relative
        // paths so moved assets don't leave dangling references in the scene.
        app.add_observer(apply_asset_path_changes_to_mesh_instances);

        // Camera2d viewport_origin override (Godot convention: world (0,0)
        // renders at the top-left of the viewport instead of the centre).
        // Registered unconditionally so the editor's preset spawns and the
        // runtime's scene load *both* fix the projection. The companion
        // observer catches reflection-loaded `Projection` overwrites.
        app.add_observer(camera::on_camera_2d_inserted);
        app.add_observer(camera::on_projection_inserted_for_2d);

        // Sprite image binding — needs to run in both editor and runtime
        // builds. In the editor it picks up drag-drop / inspector edits;
        // in the runtime it re-binds Handle<Image> from the path string
        // after scene reflection load (Handle IDs don't survive saves).
        // The observer pattern catches reflection inserts where
        // `Changed<>` doesn't. Two observers cover both insert orders:
        // the path-insert observer fires when `SpriteImagePath` arrives
        // (post-Sprite case), and the sprite-insert observer catches
        // the reverse order (Sprite arrives after the path is already
        // there — common with reflection scene loads).
        app.add_observer(scene_io::on_sprite_image_path_inserted);
        app.add_observer(scene_io::on_sprite_inserted_apply_image_path);

        app.add_plugins(debug_log::DebugLogPlugin);

        // Game startup: rpak/project/scene load + scene rehydration. Runs only
        // in a game session — in the editor the splash/project flow owns this.
        if !is_editor {
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
                            // Same timing fix as the disk path below: set
                            // the asset reader path before Startup so
                            // observer-driven asset loads resolve correctly.
                            if let Some(asset_path) = app.world().get_resource::<ProjectAssetPath>()
                            {
                                asset_path.set(project_path.clone());
                            }
                            // Override the default rendering mode if the
                            // project explicitly specifies one. `Auto`
                            // (default) resolves to platform-appropriate.
                            let resolved = config.rendering.mode.resolve();
                            info!("[runtime] rendering mode (rpak): {:?}", resolved);
                            app.insert_resource(ResolvedRenderingMode(resolved));
                            apply_rendering_mode(app, resolved);
                            app.insert_resource(CurrentProject {
                                path: project_path,
                                config,
                            });
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
                app.insert_resource(renzora::VirtualFileReader::new(move |path| {
                    vfs_for_reader.read_string(path)
                }));
                app.insert_resource(vfs);
            } else {
                app.insert_resource(vfs);

                #[cfg(not(target_arch = "wasm32"))]
                let project_path = parse_project_arg().or_else(|| {
                    let local = std::path::PathBuf::from("project.toml");
                    if local.exists() {
                        Some(local)
                    } else {
                        None
                    }
                });
                #[cfg(target_arch = "wasm32")]
                let project_path: Option<std::path::PathBuf> = None;

                if let Some(toml_path) = project_path {
                    match open_project(&toml_path) {
                        Ok(project) => {
                            info!(
                                "Loaded project: {} ({})",
                                project.config.name,
                                project.path.display()
                            );
                            // Set the asset reader path *immediately*,
                            // before any Startup system runs. Otherwise
                            // `load_current_scene` (Startup) fires
                            // observers like `on_sprite_image_path_inserted`
                            // which call `asset_server.load(...)` while
                            // the asset reader still has no project_path
                            // — the load resolves to "not found" and
                            // sprites render invisibly. The Update-time
                            // `sync_project_asset_path` system also runs,
                            // but only after the damage is done.
                            if let Some(asset_path) = app.world().get_resource::<ProjectAssetPath>()
                            {
                                asset_path.set(project.path.clone());
                            }
                            // Override default rendering mode if the
                            // project specifies one.
                            let resolved = project.config.rendering.mode.resolve();
                            info!("[runtime] rendering mode (disk): {:?}", resolved);
                            app.insert_resource(ResolvedRenderingMode(resolved));
                            apply_rendering_mode(app, resolved);
                            app.insert_resource(project);
                        }
                        Err(e) => {
                            error!("Failed to load project from {}: {}", toml_path.display(), e);
                        }
                    }
                }
            }

            app.add_systems(
                Startup,
                (
                    setup_vfs_script_reader,
                    autoload::load_autoloads,
                    scene_io::load_current_scene,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    scene_io::rehydrate_meshes,
                    scene_io::rehydrate_suns,
                    scene_io::rehydrate_lights,
                    scene_io::rehydrate_visibility,
                ),
            )
            // Loading glTF models is purely visual — a dedicated server has no
            // render world and its `server.rpak` strips meshes, so skip it
            // there (otherwise it logs "Path not found" for every model).
            .add_systems(
                Update,
                (
                    scene_io::rehydrate_mesh_instances,
                    scene_io::finish_mesh_instance_rehydrate,
                )
                    .run_if(not(resource_exists::<renzora::DedicatedServer>)),
            )
            .add_systems(
                Update,
                (
                    scene_io::rehydrate_cameras,
                    scene_io::sync_play_mode_camera,
                    scene_io::enforce_single_active_camera,
                ),
            );
        }

        // Keep ProjectAssetPath in sync with CurrentProject so the asset reader
        // always resolves from the correct project directory.
        app.add_systems(Update, sync_project_asset_path);
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(Update, install_audio_asset_loader);

        // Safety net: in Deferred mode, ensure every 3D camera carries
        // DeferredPrepass so its prepass queue includes the deferred
        // opaque phase. Covers editor previews/thumbnails that spawn
        // their own Camera3d entities without our explicit attachment.
        app.add_systems(
            PostUpdate,
            (
                ensure_deferred_prepass_on_cameras,
                ensure_contact_shadows_on_forward_cameras,
            ),
        );

        // Render-world half of the contact-shadows fix: seed the view's
        // `ViewContactShadowsUniformOffset` before Bevy reads it to pick the
        // mesh pipeline key, so the pipeline and the bind group agree on binding
        // 16 from the first frame. See `seed_contact_shadows_offset` for the race.
        if let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) {
            render_app.add_systems(
                bevy::render::Render,
                seed_contact_shadows_offset
                    .in_set(bevy::render::RenderSystems::PrepareAssets)
                    .before(bevy::pbr::check_views_need_specialization),
            );
        }

        app.init_resource::<ViewportRenderTarget>()
            .init_resource::<renzora::core::viewport_types::Viewports>()
            .init_resource::<camera::ViewportTargetsBound>()
            .init_resource::<scene_io::SceneLoadState>()
            .init_resource::<scene_io::SceneReferenceCache>()
            .init_resource::<asset_progress::AssetLoadProgress>()
            .add_systems(
                Update,
                (
                    asset_progress::tick_asset_load_progress,
                    asset_progress::publish_asset_progress_to_bridge,
                )
                    .chain(),
            );
        {
            use bevy::prelude::*;
            use procedural_meshes as pm;
            let mut reg = ShapeRegistry::default();
            // Basic
            reg.register(ShapeEntry {
                id: "cube",
                name: "Cube",
                icon: "",
                category: "Basic",
                create_mesh: |m| m.add(Cuboid::new(1.0, 1.0, 1.0)),
                default_color: Color::srgb(0.8, 0.3, 0.2),
            });
            reg.register(ShapeEntry {
                id: "sphere",
                name: "Sphere",
                icon: "",
                category: "Basic",
                create_mesh: |m| m.add(Sphere::new(0.5).mesh().ico(5).unwrap()),
                default_color: Color::srgb(0.2, 0.5, 0.8),
            });
            reg.register(ShapeEntry {
                id: "cylinder",
                name: "Cylinder",
                icon: "",
                category: "Basic",
                create_mesh: |m| m.add(Cylinder::new(0.5, 1.0)),
                default_color: Color::srgb(0.3, 0.7, 0.4),
            });
            reg.register(ShapeEntry {
                id: "plane",
                name: "Plane",
                icon: "",
                category: "Basic",
                create_mesh: |m| m.add(Plane3d::default().mesh().size(2.0, 2.0)),
                default_color: Color::srgb(0.35, 0.35, 0.35),
            });
            reg.register(ShapeEntry {
                id: "cone",
                name: "Cone",
                icon: "",
                category: "Basic",
                create_mesh: |m| {
                    m.add(Cone {
                        radius: 0.5,
                        height: 1.0,
                    })
                },
                default_color: Color::srgb(0.7, 0.5, 0.2),
            });
            reg.register(ShapeEntry {
                id: "torus",
                name: "Torus",
                icon: "",
                category: "Basic",
                create_mesh: |m| {
                    m.add(Torus {
                        minor_radius: 0.15,
                        major_radius: 0.35,
                    })
                },
                default_color: Color::srgb(0.6, 0.3, 0.7),
            });
            reg.register(ShapeEntry {
                id: "capsule",
                name: "Capsule",
                icon: "",
                category: "Basic",
                create_mesh: |m| m.add(Capsule3d::new(0.25, 0.5)),
                default_color: Color::srgb(0.3, 0.6, 0.6),
            });
            reg.register(ShapeEntry {
                id: "hemisphere",
                name: "Hemisphere",
                icon: "",
                category: "Basic",
                create_mesh: |m| m.add(pm::create_hemisphere_mesh(16)),
                default_color: Color::srgb(0.5, 0.4, 0.7),
            });
            // Level
            reg.register(ShapeEntry {
                id: "wedge",
                name: "Wedge",
                icon: "",
                category: "Level",
                create_mesh: |m| m.add(pm::create_wedge_mesh()),
                default_color: Color::srgb(0.6, 0.6, 0.5),
            });
            reg.register(ShapeEntry {
                id: "stairs",
                name: "Stairs",
                icon: "",
                category: "Level",
                create_mesh: |m| m.add(pm::create_stairs_mesh(6)),
                default_color: Color::srgb(0.5, 0.5, 0.6),
            });
            reg.register(ShapeEntry {
                id: "arch",
                name: "Arch",
                icon: "",
                category: "Level",
                create_mesh: |m| m.add(pm::create_arch_mesh(16)),
                default_color: Color::srgb(0.6, 0.5, 0.4),
            });
            reg.register(ShapeEntry {
                id: "half_cylinder",
                name: "Half Cylinder",
                icon: "",
                category: "Level",
                create_mesh: |m| m.add(pm::create_half_cylinder_mesh(16)),
                default_color: Color::srgb(0.5, 0.6, 0.5),
            });
            reg.register(ShapeEntry {
                id: "quarter_pipe",
                name: "Quarter Pipe",
                icon: "",
                category: "Level",
                create_mesh: |m| m.add(pm::create_quarter_pipe_mesh(16)),
                default_color: Color::srgb(0.55, 0.55, 0.5),
            });
            reg.register(ShapeEntry {
                id: "corner",
                name: "Corner",
                icon: "",
                category: "Level",
                create_mesh: |m| m.add(pm::create_corner_mesh()),
                default_color: Color::srgb(0.5, 0.5, 0.55),
            });
            reg.register(ShapeEntry {
                id: "wall",
                name: "Wall",
                icon: "",
                category: "Level",
                create_mesh: |m| m.add(Cuboid::new(1.0, 2.0, 0.1)),
                default_color: Color::srgb(0.55, 0.5, 0.5),
            });
            reg.register(ShapeEntry {
                id: "ramp",
                name: "Ramp",
                icon: "",
                category: "Level",
                create_mesh: |m| m.add(pm::create_ramp_mesh()),
                default_color: Color::srgb(0.5, 0.55, 0.5),
            });
            reg.register(ShapeEntry {
                id: "curved_wall",
                name: "Curved Wall",
                icon: "",
                category: "Level",
                create_mesh: |m| m.add(pm::create_curved_wall_mesh(16)),
                default_color: Color::srgb(0.55, 0.55, 0.55),
            });
            reg.register(ShapeEntry {
                id: "doorway",
                name: "Doorway",
                icon: "",
                category: "Level",
                create_mesh: |m| m.add(pm::create_doorway_mesh()),
                default_color: Color::srgb(0.5, 0.5, 0.6),
            });
            reg.register(ShapeEntry {
                id: "window_wall",
                name: "Window Wall",
                icon: "",
                category: "Level",
                create_mesh: |m| m.add(pm::create_window_wall_mesh()),
                default_color: Color::srgb(0.5, 0.55, 0.55),
            });
            reg.register(ShapeEntry {
                id: "l_shape",
                name: "L-Shape",
                icon: "",
                category: "Level",
                create_mesh: |m| m.add(pm::create_l_shape_mesh()),
                default_color: Color::srgb(0.55, 0.5, 0.55),
            });
            reg.register(ShapeEntry {
                id: "t_shape",
                name: "T-Shape",
                icon: "",
                category: "Level",
                create_mesh: |m| m.add(pm::create_t_shape_mesh()),
                default_color: Color::srgb(0.5, 0.55, 0.6),
            });
            reg.register(ShapeEntry {
                id: "cross_shape",
                name: "Cross",
                icon: "",
                category: "Level",
                create_mesh: |m| m.add(pm::create_cross_shape_mesh()),
                default_color: Color::srgb(0.55, 0.55, 0.6),
            });
            reg.register(ShapeEntry {
                id: "spiral_stairs",
                name: "Spiral Stairs",
                icon: "",
                category: "Level",
                create_mesh: |m| m.add(pm::create_spiral_stairs_mesh(16)),
                default_color: Color::srgb(0.5, 0.5, 0.55),
            });
            reg.register(ShapeEntry {
                id: "pillar",
                name: "Pillar",
                icon: "",
                category: "Level",
                create_mesh: |m| m.add(pm::create_pillar_mesh()),
                default_color: Color::srgb(0.55, 0.5, 0.5),
            });
            // Curved
            reg.register(ShapeEntry {
                id: "pipe",
                name: "Pipe",
                icon: "",
                category: "Curved",
                create_mesh: |m| m.add(pm::create_pipe_mesh(24)),
                default_color: Color::srgb(0.4, 0.5, 0.6),
            });
            reg.register(ShapeEntry {
                id: "ring",
                name: "Ring",
                icon: "",
                category: "Curved",
                create_mesh: |m| m.add(pm::create_ring_mesh(24)),
                default_color: Color::srgb(0.5, 0.4, 0.6),
            });
            reg.register(ShapeEntry {
                id: "funnel",
                name: "Funnel",
                icon: "",
                category: "Curved",
                create_mesh: |m| m.add(pm::create_funnel_mesh(24)),
                default_color: Color::srgb(0.6, 0.4, 0.5),
            });
            reg.register(ShapeEntry {
                id: "gutter",
                name: "Gutter",
                icon: "",
                category: "Curved",
                create_mesh: |m| m.add(pm::create_gutter_mesh(16)),
                default_color: Color::srgb(0.4, 0.6, 0.5),
            });
            // Advanced
            reg.register(ShapeEntry {
                id: "prism",
                name: "Prism",
                icon: "",
                category: "Advanced",
                create_mesh: |m| m.add(pm::create_prism_mesh()),
                default_color: Color::srgb(0.5, 0.5, 0.7),
            });
            reg.register(ShapeEntry {
                id: "pyramid",
                name: "Pyramid",
                icon: "",
                category: "Advanced",
                create_mesh: |m| m.add(pm::create_pyramid_mesh()),
                default_color: Color::srgb(0.7, 0.5, 0.5),
            });
            app.insert_resource(reg);
        }
        app.init_resource::<renzora::EffectRouting>();
        app.init_resource::<renzora::PendingSceneLoad>();
        app.add_systems(Update, process_pending_scene_loads);

        // In a game session, populate EffectRouting from scene cameras. The
        // editor wires effect routing through its own viewport cameras (the
        // editor camera registration lives in `renzora_engine_editor`).
        if !is_editor {
            app.add_systems(Update, update_runtime_effect_routing);
        }

        // Editor camera lifecycle, the save-scene observer and the 2D
        // auto-view-switch moved to the `renzora_engine_editor` crate
        // (`EngineEditorPlugin`, Editor scope) — installed only by the editor
        // bundle, so the lean runtime carries none of it.
    }
}

/// Wire the VFS file reader into the scripting engine so scripts can be loaded
/// from rpak archives (Android, exported builds) instead of the filesystem.
fn setup_vfs_script_reader(
    vfs: Res<Vfs>,
    mut engine: Option<ResMut<renzora_scripting::ScriptEngine>>,
) {
    if !vfs.has_archive() {
        return;
    }
    let Some(ref mut engine) = engine else {
        return;
    };
    let vfs = vfs.clone();
    engine.set_file_reader(std::sync::Arc::new(move |path: &std::path::Path| {
        // Try archive-relative key: strip leading "./" and use forward slashes
        let key = path.to_string_lossy().replace('\\', "/");
        let key = key.trim_start_matches("./");
        vfs.read_string(key)
    }));
    info!("[runtime] VFS file reader set on scripting engine");
}

/// In a game session, route effects from the default scene camera (and all
/// non-camera entities with Settings) to the active rendering camera. Gated at
/// the call site by `EditorSession` (added only when `!is_editor`).
fn update_runtime_effect_routing(
    mut routing: ResMut<renzora::EffectRouting>,
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
        let mut pending = world.resource_mut::<renzora::PendingSceneLoad>();
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
        renzora::console_log::console_error("Scene", "No project loaded — cannot load scene");
        return;
    };

    renzora::console_log::console_info(
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
            Without<Persistent>,
        )>();
        for entity in query.iter(world) {
            // Skip descendants of a `HideInHierarchy` root — the bevy_ui editor
            // chrome (the shell's `ShellRoot` carries it) and other editor-internal
            // subtrees must survive scene loads.
            if !has_hidden_ancestor(world, entity) {
                to_despawn.push(entity);
            }
        }
    }

    renzora::console_log::console_info(
        "Scene",
        format!(
            "Despawning {} entities from current scene",
            to_despawn.len()
        ),
    );

    for entity in to_despawn {
        if world.get_entity(entity).is_ok() {
            world.despawn(entity);
        }
    }

    // 2. Load the new scene
    scene_io::load_scene(world, &scene_path);
}

/// Whether any ancestor of `e` is marked [`HideInHierarchy`] (editor-internal —
/// the bevy_ui shell chrome, gizmos, previews — that must survive scene loads).
fn has_hidden_ancestor(world: &World, mut e: Entity) -> bool {
    while let Some(parent) = world.get::<ChildOf>(e).map(|c| c.parent()) {
        if world.get::<HideInHierarchy>(parent).is_some() {
            return true;
        }
        e = parent;
    }
    false
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
    info!(
        "[asset_reader] Project path set: {}",
        project.path.display()
    );
    asset_path.set(project.path.clone());
}

/// Install the audio byte loader so Kira can load clips from the virtual
/// filesystem — the `.rpak` archive in exported games, or loose files on disk
/// in the editor — rather than only the process working directory (which is
/// where `from_file` would otherwise look, and miss).
#[cfg(not(target_arch = "wasm32"))]
fn install_audio_asset_loader(
    project: Option<Res<CurrentProject>>,
    vfs: Option<Res<crate::vfs::Vfs>>,
) {
    let Some(project) = project else {
        return;
    };
    let vfs_changed = vfs.as_ref().is_some_and(|v| v.is_changed());
    if !(project.is_changed() || vfs_changed) {
        return;
    }
    let root = project.path.clone();
    let archive = vfs.as_ref().and_then(|v| v.archive_arc());
    renzora::core::set_asset_byte_loader(Box::new(move |key: &str| {
        if let Some(ref archive) = archive {
            if let Some(bytes) = archive.get(key) {
                return Some(bytes);
            }
        }
        std::fs::read(root.join(key)).ok()
    }));
}

#[cfg(not(target_arch = "wasm32"))]
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
    trigger: On<renzora::AssetPathChanged>,
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
