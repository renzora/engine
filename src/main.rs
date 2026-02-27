// Hide console window on Windows
#![windows_subsystem = "windows"]

mod blueprint;
mod brushes;
mod commands;
mod component_system;
mod core;
mod crash;
mod embedded;
mod gizmo;
mod import;
mod gltf_animation;
mod input;
mod mesh_sculpt;
#[cfg(feature = "solari")]
mod meshlet;
mod particles;
mod play_mode;
mod post_process;
mod plugin_core;
mod project;
mod scene;
mod scripting;
mod shader_preview;
mod shader_thumbnail;
mod spawn;
mod surface_painting;
mod terrain;
mod thumbnail_cli;
mod ui;
mod ui_api;
mod update;
mod viewport;
use bevy::asset::UnapprovedPathMode;
use bevy::prelude::*;
use bevy::render::{
    render_resource::WgpuFeatures,
    settings::{RenderCreation, WgpuSettings},
    RenderPlugin,
};
#[cfg(feature = "solari")]
use bevy::solari::SolariPlugins;
#[cfg(feature = "solari")]
use bevy::pbr::experimental::meshlet::MeshletPlugin;
#[cfg(feature = "solari")]
use bevy::anti_alias::dlss::DlssProjectId;
#[cfg(feature = "solari")]
use bevy::asset::uuid::Uuid;
use bevy::window::{WindowMode, WindowResizeConstraints};
use bevy::winit::{WinitSettings, WinitWindows};
use bevy_egui::EguiPrimaryContextPass;

use crate::core::{AppState, RuntimeConfig};

/// Marker component for the splash screen camera
#[derive(Component)]
struct SplashCamera;

/// Set the window icon. Uses the project's custom icon if available (runtime/play mode),
/// otherwise falls back to the embedded engine icon.
#[cfg(windows)]
fn set_window_icon(
    windows: Option<NonSend<WinitWindows>>,
    current_project: Option<Res<project::CurrentProject>>,
) {
    let Some(windows) = windows else { return };

    // Try loading project-specific icon first
    let icon_data = current_project
        .as_ref()
        .and_then(|proj| proj.config.icon.as_ref())
        .and_then(|icon_rel| {
            let icon_path = current_project.as_ref().unwrap().path.join(icon_rel);
            if icon_path.exists() {
                load_icon_from_file(&icon_path)
            } else {
                // Also check next to the executable (exported game)
                std::env::current_exe().ok()
                    .and_then(|exe| exe.parent().map(|d| d.to_path_buf()))
                    .and_then(|dir| {
                        let filename = std::path::Path::new(icon_rel).file_name()?;
                        let path = dir.join(filename);
                        if path.exists() { load_icon_from_file(&path) } else { None }
                    })
            }
        });

    // Fall back to embedded icon
    let icon_data = icon_data.or_else(|| parse_ico_largest(include_bytes!("../icon.ico")));

    if let Some((rgba, width, height)) = icon_data {
        if let Ok(icon) = winit::window::Icon::from_rgba(rgba, width, height) {
            for window in windows.windows.values() {
                window.set_window_icon(Some(icon.clone()));
            }
        }
    }
}

#[cfg(not(windows))]
fn set_window_icon() {}

/// Load an icon from a file path (.ico or .png)
#[cfg(windows)]
fn load_icon_from_file(path: &std::path::Path) -> Option<(Vec<u8>, u32, u32)> {
    let data = std::fs::read(path).ok()?;
    parse_ico_largest(&data)
}

/// Parse ICO file and extract the largest image as RGBA
#[cfg(windows)]
fn parse_ico_largest(data: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    use image::ImageReader;
    use std::io::Cursor;

    // Use image crate to decode ICO
    let reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .ok()?;
    let img = reader.decode().ok()?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some((rgba.into_raw(), width, height))
}

fn main() {
    // Install custom panic hook for crash reporting
    crash::install_panic_hook();

    // Check for headless thumbnail rendering mode
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 4 && args[1] == "--render-thumbnail" {
        thumbnail_cli::run_thumbnail_renderer(args[2].clone(), args[3].clone());
        return;
    }

    // Check for project.toml next to the executable (exported/runtime game)
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));

    let runtime_project = if !args.iter().any(|a| a == "--play") {
        exe_dir.as_ref().and_then(|dir| {
            let toml_path = dir.join("project.toml");
            if toml_path.exists() {
                project::open_project(&toml_path).ok()
            } else {
                None
            }
        })
    } else {
        None
    };

    // Check for --play mode: launches editor, auto-opens project, auto-triggers play mode (F5)
    let play_project = runtime_project.or_else(|| {
        if args.len() >= 3 && args[1] == "--play" {
            // Attach a console window so logs are visible
            #[cfg(windows)]
            unsafe { windows_sys::Win32::System::Console::AllocConsole(); }

            let project_path = std::path::PathBuf::from(&args[2]);
            let toml_path = project_path.join("project.toml");
            match project::open_project(&toml_path) {
                Ok(proj) => Some(proj),
                Err(e) => {
                    eprintln!("Error: Could not open project at {}: {}", project_path.display(), e);
                    std::process::exit(1);
                }
            }
        } else {
            None
        }
    });
    let is_play_mode = play_project.is_some();

    // When packed, extract embedded updater and plugins to disk
    embedded::extract_embedded_updater();
    embedded::extract_embedded_plugins();

    let mut app = App::new();

    // Register custom asset reader (with project-local override support) before DefaultPlugins
    embedded::setup_embedded_assets(&mut app);

    // DLSS requires a project ID before plugin initialization
    // Use a fixed UUID for Renzora Engine (generated once, kept consistent)
    #[cfg(feature = "solari")]
    app.insert_resource(DlssProjectId(Uuid::from_u128(0x52454e5a4f52415f454e47494e455f31)));

    // Window config differs for --play mode (decorated, project-titled)
    let window = if let Some(ref proj) = play_project {
        let wc = &proj.config.window;
        Window {
            title: proj.config.name.clone(),
            resolution: (wc.width, wc.height).into(),
            position: WindowPosition::Centered(MonitorSelection::Primary),
            mode: if wc.fullscreen {
                WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
            } else {
                WindowMode::Windowed
            },
            decorations: true,
            resizable: wc.resizable,
            resize_constraints: WindowResizeConstraints {
                min_width: 640.0,
                min_height: 360.0,
                ..default()
            },
            ..default()
        }
    } else {
        Window {
            title: "Renzora Engine r1".to_string(),
            resolution: (800u32, 600u32).into(),
            position: WindowPosition::Centered(MonitorSelection::Primary),
            decorations: false,
            resizable: true,
            resize_constraints: WindowResizeConstraints {
                min_width: 800.0,
                min_height: 600.0,
                ..default()
            },
            ..default()
        }
    };

    app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(window),
                    ..default()
                })
                .set(AssetPlugin {
                    unapproved_path_mode: UnapprovedPathMode::Allow,
                    ..default()
                })
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(WgpuSettings {
                        // Enable wireframe rendering (native only - DX12, Vulkan, Metal)
                        features: WgpuFeatures::POLYGON_MODE_LINE,
                        ..default()
                    }),
                    ..default()
                })
        )
        .add_plugins(bevy_egui::EguiPlugin::default());

    // Pause rendering when the window is not focused to save CPU/GPU
    app.insert_resource(WinitSettings {
        focused_mode: bevy::winit::UpdateMode::Continuous,
        unfocused_mode: bevy::winit::UpdateMode::reactive_low_power(
            std::time::Duration::from_secs(1),
        ),
    });

    #[cfg(feature = "solari")]
    app.add_plugins(SolariPlugins)
        .add_plugins(MeshletPlugin { cluster_buffer_slots: 8192 });

    app.add_plugins(bevy::picking::mesh_picking::MeshPickingPlugin)
        .add_plugins(bevy_mod_outline::OutlinePlugin)
        .register_type::<component_system::MeshNodeData>()
        .register_type::<component_system::MeshPrimitiveType>()
        .register_type::<brushes::BrushData>()
        .register_type::<brushes::BrushType>()
        .register_type::<terrain::TerrainData>()
        .register_type::<terrain::TerrainChunkData>()
        .register_type::<surface_painting::PaintableSurfaceData>()
        .register_type::<component_system::CameraNodeData>()
        .register_type::<component_system::CameraRigData>()
        .register_type::<component_system::MeshInstanceData>()
        .register_type::<component_system::SceneInstanceData>()
        .register_type::<component_system::PhysicsBodyData>()
        .register_type::<component_system::PhysicsBodyType>()
        .register_type::<component_system::CollisionShapeData>()
        .register_type::<component_system::CollisionShapeType>()
        .register_type::<component_system::Sprite2DData>()
        .register_type::<component_system::Camera2DData>()
        .register_type::<component_system::UIPanelData>()
        .register_type::<component_system::UILabelData>()
        .register_type::<component_system::UIButtonData>()
        .register_type::<component_system::UIImageData>()
        .register_type::<component_system::GltfAnimations>()
        .register_type::<component_system::PointLightData>()
        .register_type::<component_system::DirectionalLightData>()
        .register_type::<component_system::SpotLightData>()
        .register_type::<component_system::SolariLightingData>()
        .register_type::<component_system::DlssQualityMode>()
        .register_type::<component_system::SunData>()
        .register_type::<component_system::WorldEnvironmentData>()
        .register_type::<component_system::SkyMode>()
        .register_type::<component_system::ProceduralSkyData>()
        .register_type::<component_system::PanoramaSkyData>()
        .register_type::<component_system::TonemappingMode>()
        .register_type::<component_system::SkyboxData>()
        .register_type::<component_system::FogData>()
        .register_type::<component_system::AntiAliasingData>()
        .register_type::<component_system::AmbientOcclusionData>()
        .register_type::<component_system::ReflectionsData>()
        .register_type::<component_system::BloomData>()
        .register_type::<component_system::TonemappingData>()
        .register_type::<component_system::DepthOfFieldData>()
        .register_type::<component_system::MotionBlurData>()
        .register_type::<component_system::AmbientLightData>()
        .register_type::<component_system::CloudsData>()
        .register_type::<component_system::TaaData>()
        .register_type::<component_system::SmaaData>()
        .register_type::<component_system::SmaaPresetMode>()
        .register_type::<component_system::CasData>()
        .register_type::<component_system::ChromaticAberrationData>()
        .register_type::<component_system::AutoExposureData>()
        .register_type::<component_system::VolumetricFogData>()
        .register_type::<component_system::VignetteData>()
        .register_type::<component_system::FilmGrainData>()
        .register_type::<component_system::PixelationData>()
        .register_type::<component_system::CrtData>()
        .register_type::<component_system::GodRaysData>()
        .register_type::<component_system::GaussianBlurData>()
        .register_type::<component_system::PaletteQuantizationData>()
        .register_type::<component_system::DistortionData>()
        .register_type::<component_system::UnderwaterData>()
        .register_type::<core::EditorEntity>()
        .register_type::<core::SceneNode>()
        .register_type::<core::SceneTabId>()
        .register_type::<core::WorldEnvironmentMarker>()
        .register_type::<spawn::EditorSceneRoot>()
        .register_type::<spawn::SceneType>()
        .register_type::<scripting::ScriptComponent>()
        .register_type::<scripting::ScriptVariables>()
        .register_type::<scripting::ScriptValue>()
        .register_type::<scene::EditorSceneMetadata>()
        .register_type::<component_system::MeshletMeshData>()
        .register_type::<Option<String>>()
        .register_type::<std::path::PathBuf>()
        .register_type::<Option<std::path::PathBuf>>()
        .register_type::<std::collections::HashMap<String, scripting::ScriptValue>>()
        // All plugins (no conditional loading — play mode uses the full editor)
        .add_plugins(core::CorePlugin)
        .add_plugins(commands::CommandPlugin)
        .add_plugins(project::ProjectPlugin)
        .add_plugins(component_system::ComponentSystemPlugin)
        .add_plugins(plugin_core::PluginCorePlugin)
        .add_plugins(component_system::RenzoraPhysicsPlugin::new(true)) // Always start paused; play mode unpauses
        .add_plugins(vleue_navigator::prelude::VleueNavigatorPlugin)
        .add_plugins(vleue_navigator::prelude::NavmeshUpdaterPlugin::<
            avian3d::prelude::Collider, avian3d::prelude::Collider
        >::default())
        .add_plugins(vleue_navigator::prelude::AvoidancePlugin)
        .add_plugins(bevy_silk::ClothPlugin)
        .add_plugins(gltf_animation::GltfAnimationPlugin)
        .add_plugins(particles::ParticlesPlugin)
        .add_plugins(core::DiagnosticsPlugin)
        .add_plugins(viewport::ViewportPlugin)
        .add_plugins(viewport::StudioPreviewPlugin)
        .add_plugins(viewport::ParticlePreviewPlugin)
        .add_plugins(shader_preview::ShaderPreviewPlugin)
        .add_plugins(gizmo::GizmoPlugin)
        .add_plugins(input::InputPlugin)
        .add_plugins(ui::UiPlugin)
        .add_plugins(scripting::ScriptingPlugin)
        .add_plugins(play_mode::PlayModePlugin)
        .add_plugins(shader_thumbnail::ShaderThumbnailPlugin)
        .add_plugins(blueprint::BlueprintPlugin)
        .add_plugins(blueprint::MaterialPreviewPlugin)
        .add_plugins(brushes::BrushPlugin)
        .add_plugins(terrain::TerrainPlugin)
        .add_plugins(mesh_sculpt::MeshSculptPlugin)
        .add_plugins(surface_painting::SurfacePaintingPlugin)
        .add_plugins(update::UpdatePlugin);

    // Meshlet/Virtual Geometry integration (requires solari feature)
    #[cfg(feature = "solari")]
    app.add_plugins(meshlet::MeshletIntegrationPlugin);

    // Shared setup: observer, state, scene registry
    app.add_observer(scene::on_bevy_scene_ready)
        .insert_resource(scene::create_default_registry());

    // State initialization
    app.init_state::<AppState>(); // Always starts at Splash
    if let Some(proj) = play_project {
        app.insert_resource(RuntimeConfig { project_path: proj.path.clone() });
        app.insert_resource(proj);
        // Transition to Editor on first frame (after Startup commands flush, so ViewportImage exists)
        app.add_systems(Startup, |mut next_state: ResMut<NextState<AppState>>| {
            next_state.set(AppState::Editor);
        });
    }

    // Scene component rehydration systems
    app.add_systems(
        Update,
        (
            scene::rehydrate_mesh_components,
            scene::rehydrate_point_lights,
            scene::rehydrate_directional_lights,
            scene::rehydrate_spot_lights,
            scene::rehydrate_sun_lights,
            scene::rehydrate_terrain_chunks,
            scene::apply_terrain_materials,
            scene::rebuild_children_from_child_of,
            scene::rehydrate_cameras_3d,
            scene::rehydrate_camera_rigs,
            scene::rehydrate_cameras_2d,
            scene::prepare_meshes_for_solari,
            scene::assign_node_icons,
        )
            .run_if(in_state(AppState::Editor).or(in_state(AppState::Runtime))),
    );

    {
        // Editor mode systems
        app.init_resource::<crash::CrashReportWindowState>()
            .add_systems(Startup, crash::check_for_previous_crash)
            .add_systems(
                EguiPrimaryContextPass,
                crash::render_crash_report_window.run_if(in_state(AppState::Editor)),
            )
            .add_systems(Startup, (set_window_icon, setup_splash_camera))
            .add_systems(
                EguiPrimaryContextPass,
                ui::splash_ui.run_if(in_state(AppState::Splash).and(not(resource_exists::<RuntimeConfig>))),
            )
            .add_systems(
                EguiPrimaryContextPass,
                ui::editor_ui.run_if(not(resource_exists::<RuntimeConfig>)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                ui::inspector_content_exclusive
                    .after(ui::editor_ui)
                    .run_if(not(resource_exists::<RuntimeConfig>)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                ui::thumbnail_loading_system
                    .after(ui::editor_ui)
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                viewport::register_model_preview_textures
                    .after(ui::editor_ui)
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                shader_preview::register_shader_preview_texture
                    .after(ui::editor_ui)
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                shader_thumbnail::register_shader_thumbnail_textures
                    .after(ui::editor_ui)
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(EguiPrimaryContextPass, ui::handle_window_actions.after(ui::editor_ui).run_if(not(resource_exists::<RuntimeConfig>)))
            .add_systems(
                EguiPrimaryContextPass,
                viewport::apply_orbit_to_camera
                    .after(ui::editor_ui)
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                (
                    viewport::resize_viewport_texture,
                    viewport::update_camera_preview,
                    input::handle_selection,
                    input::handle_view_angles,
                    input::handle_view_toggles,
                    input::handle_play_mode,
                    input::handle_hierarchy_shortcuts,
                    viewport::update_camera_projection,
                    viewport::camera_controller,
                    viewport::camera2d_controller,
                    viewport::toggle_viewport_cameras,
                )
                    .chain()
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                viewport::camera_focus_selected.run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                (
                    gizmo::modal_transform_input_system,
                    gizmo::modal_transform_keyboard_system,
                    gizmo::modal_transform_apply_system,
                    gizmo::modal_transform_overlay_system,
                )
                    .chain()
                    .before(gizmo::gizmo_hover_system)
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                (
                    gizmo::gizmo_hover_system,
                    gizmo::gizmo_interaction_system,
                    gizmo::object_drag_system,
                    gizmo::draw_selection_gizmo,
                    gizmo::update_selection_outlines,
                    gizmo::terrain_chunk_selection_system,
                    gizmo::gizmo_2d_hover_system,
                    gizmo::gizmo_2d_interaction_system,
                    gizmo::gizmo_2d_drag_system,
                    gizmo::draw_selection_gizmo_2d,
                    gizmo::handle_2d_picking,
                )
                    .chain()
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                (
                    gizmo::collider_edit_selection_sync,
                    gizmo::collider_edit_hover_system,
                    gizmo::collider_edit_interaction_system,
                    gizmo::collider_edit_drag_system,
                )
                    .chain()
                    .after(gizmo::handle_2d_picking)
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                (
                    gizmo::draw_physics_gizmos,
                    gizmo::draw_collider_edit_handles,
                    gizmo::draw_grid,
                    gizmo::draw_camera_gizmos,
                    viewport::draw_grid_2d,
                    viewport::update_2d_visuals,
                    viewport::cleanup_2d_visuals,
                    input::handle_file_drop,
                    input::handle_asset_panel_drop,
                    input::handle_image_panel_drop,
                    input::handle_material_panel_drop,
                    input::handle_pending_skybox_drop,
                    input::apply_material_data,
                    input::handle_scene_hierarchy_drop,
                    input::spawn_loaded_gltfs,
                    input::check_mesh_instance_models,
                    input::spawn_mesh_instance_models,
                    scene::handle_save_shortcut,
                    scene::handle_make_default_camera,
                    scene::assign_scene_tab_ids,
                )
                    .chain()
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                scene::handle_snap_camera_to_viewport
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                input::drag_surface_raycast_system
                    .before(input::update_shape_drag_preview)
                    .before(input::update_drag_preview)
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                (input::handle_shape_library_spawn, input::update_shape_drag_preview)
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                input::handle_script_hierarchy_drop
                    .after(input::handle_scene_hierarchy_drop)
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                input::handle_effect_panel_drop
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                (
                    input::start_drag_preview,
                    input::update_drag_preview,
                    input::cleanup_drag_preview,
                )
                    .chain()
                    .before(input::handle_asset_panel_drop)
                    .run_if(in_state(AppState::Editor).and(input::drag_preview_active)),
            )
            .add_systems(
                Update,
                input::align_models_to_ground
                    .after(input::spawn_loaded_gltfs)
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                scene::handle_scene_requests
                    .before(viewport::camera_controller)
                    .before(scene::assign_scene_tab_ids)
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                scene::auto_save_scene
                    .before(scene::handle_scene_requests)
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                scene::handle_export_request
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                input::load_scene_instances
                    .after(input::handle_scene_hierarchy_drop)
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(OnEnter(AppState::Editor), (maximize_window, despawn_splash_camera, load_project_scene, load_editor_state).chain())
            .add_systems(Update, handle_pending_project_reopen.run_if(in_state(AppState::Splash)))
            .add_systems(Update, save_editor_state_periodic.run_if(in_state(AppState::Editor)))
            .init_resource::<project::EditorStateDirty>()
            .init_resource::<project::LoadedEditorState>()
            .init_resource::<gizmo::ModalTransformState>();
    }

    // Auto-trigger play mode when launched with --play flag
    if is_play_mode {
        app.add_systems(Update, auto_play_after_scene_load.run_if(in_state(AppState::Editor)));
    }

    app.run();
}

/// Setup a simple camera for the splash screen
fn setup_splash_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Msaa::Off,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12)),
            ..default()
        },
        SplashCamera,
        Name::new("UI Camera"),
    ));
}

/// When a project was opened via File > Open Project, we transition to Splash first
/// so that OnEnter(Editor) can re-run. This system detects the pending reopen marker
/// and immediately transitions back to Editor.
fn handle_pending_project_reopen(
    mut commands: Commands,
    pending: Option<Res<scene::PendingProjectReopen>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if pending.is_some() {
        commands.remove_resource::<scene::PendingProjectReopen>();
        next_state.set(AppState::Editor);
    }
}

/// Keep the splash camera for egui UI rendering in editor mode.
/// In --play mode, deactivate it entirely (no editor UI).
fn despawn_splash_camera(
    mut query: Query<&mut Camera, With<SplashCamera>>,
    runtime_config: Option<Res<RuntimeConfig>>,
) {
    for mut camera in query.iter_mut() {
        if runtime_config.is_some() {
            // --play mode: deactivate so no editor UI renders over the game
            camera.is_active = false;
        } else {
            // Editor mode: repurpose as UI camera with high order
            camera.order = 100;
        }
    }
}

/// Maximize the window when entering the editor (skip in --play mode)
fn maximize_window(
    mut windows: Query<&mut Window>,
    runtime_config: Option<Res<RuntimeConfig>>,
) {
    // In --play mode, keep the configured window size (1280x720)
    if runtime_config.is_some() { return; }

    for mut window in windows.iter_mut() {
        window.set_maximized(true);
        window.mode = WindowMode::Windowed;
    }
}

/// System to load the project's main scene when entering Editor state
fn load_project_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    viewport_image: Res<viewport::ViewportImage>,
    orbit: Res<core::OrbitCameraState>,
    viewport: Res<core::ViewportState>,
    camera2d_state: Res<viewport::Camera2DState>,
    mut scene_state: ResMut<core::SceneManagerState>,
    current_project: Option<Res<project::CurrentProject>>,
) {
    // Always set up the editor camera for the viewport (3D)
    scene::setup_editor_camera(&mut commands, &mut meshes, &mut materials, &viewport_image, &orbit, &viewport);

    // Set up the 2D editor camera
    viewport::setup_editor_camera_2d(&mut commands, &viewport_image, &viewport, &camera2d_state);

    // Add ambient light
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 200.0,
        ..default()
    });

    if let Some(project) = current_project {
        console_info!("Project", "Opening project: {}", project.config.name);

        // Scene path from project config (already has .ron extension)
        let scene_path = project.main_scene_path();

        if scene_path.exists() {
            console_info!("Scene", "Loading scene: {}", scene_path.display());

            // Load using Bevy's DynamicScene system (async)
            // Editor metadata (camera state, expanded entities) is embedded in the scene
            // and will be applied automatically by on_bevy_scene_ready
            let _result = scene::load_scene_bevy(&mut commands, &asset_server, &scene_path, 0);

            // Set the current scene path so Ctrl+S knows where to save
            scene_state.current_scene_path = Some(scene_path.clone());

            // Update the first tab with the loaded scene info
            let scene_name = scene_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("main")
                .to_string();
            if let Some(tab) = scene_state.scene_tabs.get_mut(0) {
                tab.name = scene_name.clone();
                tab.path = Some(scene_path.clone());
            }

            info!("Loading scene: {}", scene_path.display());
            console_success!("Scene", "Loading: {}", scene_name);
        } else {
            console_warn!("Scene", "Main scene not found: {}", scene_path.display());
        }
    }
}

/// System to load and apply editor state when entering Editor state
fn load_editor_state(
    current_project: Option<Res<project::CurrentProject>>,
    mut viewport: ResMut<core::ViewportState>,
    mut settings: ResMut<core::EditorSettings>,
    mut assets: ResMut<core::AssetBrowserState>,
    mut docking: ResMut<core::DockingState>,
    mut loaded_state: ResMut<project::LoadedEditorState>,
    mut theme_manager: ResMut<renzora_theme::ThemeManager>,
    mut scene_state: ResMut<core::SceneManagerState>,
) {
    let Some(project) = current_project else { return };

    // Load editor state from project
    let state = project::EditorStateConfig::load(&project.path);

    // Apply layout settings (clamp to safe maximums to prevent crashes)
    viewport.hierarchy_width = state.layout.hierarchy_width.clamp(100.0, 500.0);
    viewport.inspector_width = state.layout.inspector_width.clamp(100.0, 500.0);
    viewport.assets_height = state.layout.assets_height.clamp(100.0, 400.0);
    viewport.bottom_panel_tab = match state.layout.bottom_panel_tab.as_str() {
        "console" => core::BottomPanelTab::Console,
        _ => core::BottomPanelTab::Assets,
    };
    viewport.viewport_mode = match state.viewport.mode.as_str() {
        "2d" => viewport::ViewportMode::Mode2D,
        _ => viewport::ViewportMode::Mode3D,
    };

    // Apply editor settings
    settings.dev_mode = state.settings.dev_mode;
    settings.camera_move_speed = state.settings.camera_move_speed;
    settings.show_grid = state.settings.show_grid;
    settings.grid_size = state.settings.grid_size;
    settings.grid_divisions = state.settings.grid_divisions;
    settings.grid_color = state.settings.grid_color;
    settings.render_toggles.textures = state.settings.render.textures;
    settings.render_toggles.wireframe = state.settings.render.wireframe;
    settings.render_toggles.lighting = state.settings.render.lighting;
    settings.render_toggles.shadows = state.settings.render.shadows;
    settings.selection_highlight_mode = match state.settings.selection_highlight_mode.as_str() {
        "gizmo" => core::SelectionHighlightMode::Gizmo,
        _ => core::SelectionHighlightMode::Outline,
    };

    // Apply auto-save settings
    scene_state.auto_save_enabled = state.settings.auto_save_enabled;
    scene_state.auto_save_interval = state.settings.auto_save_interval.clamp(10.0, 600.0);

    // Apply asset browser settings
    assets.zoom = state.asset_browser.zoom;
    assets.view_mode = match state.asset_browser.view_mode.as_str() {
        "list" => core::AssetViewMode::List,
        _ => core::AssetViewMode::Grid,
    };

    // Apply docking/layout settings
    docking.load_from_config(state.docking.clone());

    // Apply theme settings
    theme_manager.set_project_path(&project.path);
    theme_manager.load_theme(&state.active_theme);

    // Store loaded state for saving back
    loaded_state.0 = Some(state);

    console_info!("Editor", "Loaded editor state from project");
}

/// Timer for periodic editor state saving
#[derive(Resource)]
struct EditorStateSaveTimer(Timer);

impl Default for EditorStateSaveTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(5.0, TimerMode::Repeating))
    }
}

/// System to periodically save editor state if dirty
fn save_editor_state_periodic(
    time: Res<Time>,
    mut timer: Local<EditorStateSaveTimer>,
    current_project: Option<Res<project::CurrentProject>>,
    viewport: Res<core::ViewportState>,
    settings: Res<core::EditorSettings>,
    assets: Res<core::AssetBrowserState>,
    docking: Res<core::DockingState>,
    theme_manager: Res<renzora_theme::ThemeManager>,
    scene_state: Res<core::SceneManagerState>,
    mut dirty: ResMut<project::EditorStateDirty>,
) {
    timer.0.tick(time.delta());

    if !timer.0.just_finished() {
        return;
    }

    let Some(project) = current_project else { return };

    // Collect current state
    let state = project::EditorStateConfig {
        layout: project::editor_state::LayoutConfig {
            hierarchy_width: viewport.hierarchy_width,
            inspector_width: viewport.inspector_width,
            assets_height: viewport.assets_height,
            bottom_panel_tab: match viewport.bottom_panel_tab {
                core::BottomPanelTab::Console => "console".to_string(),
                core::BottomPanelTab::Assets => "assets".to_string(),
                core::BottomPanelTab::Animation => "animation".to_string(),
            },
        },
        settings: project::editor_state::SettingsConfig {
            dev_mode: settings.dev_mode,
            camera_move_speed: settings.camera_move_speed,
            show_grid: settings.show_grid,
            grid_size: settings.grid_size,
            grid_divisions: settings.grid_divisions,
            grid_color: settings.grid_color,
            render: project::editor_state::RenderConfig {
                textures: settings.render_toggles.textures,
                wireframe: settings.render_toggles.wireframe,
                lighting: settings.render_toggles.lighting,
                shadows: settings.render_toggles.shadows,
            },
            auto_save_enabled: scene_state.auto_save_enabled,
            auto_save_interval: scene_state.auto_save_interval,
            selection_highlight_mode: match settings.selection_highlight_mode {
                core::SelectionHighlightMode::Outline => "outline".to_string(),
                core::SelectionHighlightMode::Gizmo => "gizmo".to_string(),
            },
        },
        asset_browser: project::editor_state::AssetBrowserConfig {
            zoom: assets.zoom,
            view_mode: match assets.view_mode {
                core::AssetViewMode::List => "list".to_string(),
                core::AssetViewMode::Grid => "grid".to_string(),
            },
        },
        viewport: project::editor_state::ViewportConfig {
            mode: match viewport.viewport_mode {
                viewport::ViewportMode::Mode2D => "2d".to_string(),
                viewport::ViewportMode::Mode3D => "3d".to_string(),
            },
        },
        docking: docking.save_to_config(),
        active_theme: theme_manager.active_theme_name.clone(),
    };

    // Save to disk
    if let Err(e) = state.save(&project.path) {
        error!("Failed to save editor state: {}", e);
    }

    // Clear dirty flag
    dirty.0 = false;
}

// ---- Auto-play system for --play flag ----

/// Waits for the scene to finish loading, then auto-triggers play mode (same as F5).
/// Only runs when launched with --play flag (RuntimeConfig resource present).
fn auto_play_after_scene_load(
    mut play_mode: ResMut<core::PlayModeState>,
    pending: Query<(), With<scene::loader::PendingSceneLoad>>,
    mut seen_pending: Local<bool>,
    mut frames_ready: Local<u32>,
    mut done: Local<bool>,
) {
    if *done { return; }

    // Phase 1: Wait for scene load to start
    if !pending.is_empty() {
        *seen_pending = true;
        *frames_ready = 0;
        return;
    }

    // Phase 2: Scene loaded (or no scene). Wait for rehydration.
    *frames_ready += 1;
    let min_frames = if *seen_pending { 10 } else { 60 };

    if *frames_ready >= min_frames {
        play_mode.request_play = true;
        *done = true;
        info!("Auto-play: Triggering play mode");
    }
}
