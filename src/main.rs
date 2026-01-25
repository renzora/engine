// Hide console window on Windows
#![windows_subsystem = "windows"]

mod commands;
mod core;
mod export;
mod gizmo;
mod input;
mod node_system;
mod play_mode;
mod plugin_core;
mod project;
mod scene;
mod scene_file;
mod scripting;
mod shared;
mod ui;
mod ui_api;
mod viewport;

use bevy::asset::UnapprovedPathMode;
use bevy::prelude::*;
use bevy::render::{
    render_resource::WgpuFeatures,
    settings::{RenderCreation, WgpuSettings},
    RenderPlugin,
};
use bevy::window::WindowMode;
use bevy_egui::EguiPrimaryContextPass;

use crate::core::AppState;

/// Marker component for the splash screen camera
#[derive(Component)]
struct SplashCamera;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Renzora Engine r4".to_string(),
                        resolution: (800u32, 600u32).into(),
                        position: WindowPosition::Centered(MonitorSelection::Primary),
                        decorations: false,
                        ..default()
                    }),
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
        .add_plugins(bevy_egui::EguiPlugin::default())
        .add_plugins((
            core::CorePlugin,
            commands::CommandPlugin,
            project::ProjectPlugin,
            node_system::NodeSystemPlugin,
            viewport::ViewportPlugin,
            gizmo::GizmoPlugin,
            input::InputPlugin,
            ui::UiPlugin,
            scripting::ScriptingPlugin,
            plugin_core::PluginCorePlugin,
            play_mode::PlayModePlugin,
        ))
        // Initialize app state
        .init_state::<AppState>()
        // Setup splash camera on startup
        .add_systems(Startup, setup_splash_camera)
        // Splash screen systems (run when in Splash state)
        .add_systems(
            EguiPrimaryContextPass,
            ui::splash_ui.run_if(in_state(AppState::Splash)),
        )
        // Editor UI system (in EguiPrimaryContextPass)
        // Note: run_if(in_state) doesn't work with EguiPrimaryContextPass, so state is checked inside the system
        .add_systems(
            EguiPrimaryContextPass,
            ui::editor_ui,
        )
        // Window actions system - runs in same schedule as egui to handle drag immediately
        .add_systems(EguiPrimaryContextPass, ui::handle_window_actions.after(ui::editor_ui))
        // Editor non-UI systems (run when in Editor state) - split into multiple groups
        .add_systems(
            Update,
            (
                viewport::resize_viewport_texture,
                viewport::update_camera_preview,
                input::handle_selection,
                viewport::camera_controller,
                viewport::camera2d_controller,
                viewport::toggle_viewport_cameras,
            )
                .chain()
                .run_if(in_state(AppState::Editor)),
        )
        .add_systems(
            Update,
            (
                // 3D gizmo systems
                gizmo::gizmo_hover_system,
                gizmo::gizmo_interaction_system,
                gizmo::object_drag_system,
                gizmo::draw_selection_gizmo,
                // 2D gizmo systems
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
                gizmo::draw_physics_gizmos,
                gizmo::draw_grid,
                viewport::draw_grid_2d,
                // 2D/UI visual rendering
                viewport::update_2d_visuals,
                viewport::cleanup_2d_visuals,
                // Input handling
                input::handle_file_drop,
                input::handle_asset_panel_drop,
                input::handle_scene_hierarchy_drop,
                input::spawn_loaded_gltfs,
                input::check_mesh_instance_models,
                input::spawn_mesh_instance_models,
                node_system::handle_save_shortcut,
                node_system::handle_make_default_camera,
                node_system::assign_scene_tab_ids,
            )
                .chain()
                .run_if(in_state(AppState::Editor)),
        )
        // Scene management exclusive system (needs &mut World for saving)
        // Must run before camera_controller to avoid 1-frame camera delay on tab switch
        // Must run before assign_scene_tab_ids so newly loaded entities get tab IDs assigned
        .add_systems(
            Update,
            node_system::handle_scene_requests
                .before(viewport::camera_controller)
                .before(node_system::assign_scene_tab_ids)
                .run_if(in_state(AppState::Editor)),
        )
        // Scene instance loading exclusive system (needs &mut World for spawning)
        .add_systems(
            Update,
            input::load_scene_instances
                .after(input::handle_scene_hierarchy_drop)
                .run_if(in_state(AppState::Editor)),
        )
        // When entering Editor state: maximize window, despawn splash camera and load scene
        .add_systems(OnEnter(AppState::Editor), (maximize_window, despawn_splash_camera, load_project_scene).chain())
        .run();
}

/// Setup a simple camera for the splash screen
fn setup_splash_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12)),
            ..default()
        },
        SplashCamera,
    ));
}

/// Marker component for the UI camera in editor mode
#[derive(Component)]
struct UiCamera;

/// Keep the splash camera for egui UI rendering in editor mode
fn despawn_splash_camera(mut query: Query<&mut Camera, With<SplashCamera>>) {
    for mut camera in query.iter_mut() {
        // Just update the order to render after 3D viewport, keep everything else
        camera.order = 100;
    }
}

/// Maximize the window when entering the editor
fn maximize_window(mut windows: Query<&mut Window>) {
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
    viewport_image: Res<viewport::ViewportImage>,
    mut orbit: ResMut<core::OrbitCameraState>,
    viewport: Res<core::ViewportState>,
    camera2d_state: Res<viewport::Camera2DState>,
    mut scene_state: ResMut<core::SceneManagerState>,
    mut hierarchy: ResMut<core::HierarchyState>,
    current_project: Option<Res<project::CurrentProject>>,
    node_registry: Res<node_system::NodeRegistry>,
) {
    // Always set up the editor camera for the viewport (3D)
    scene::setup_editor_camera(&mut commands, &mut meshes, &mut materials, &viewport_image, &orbit, &viewport);

    // Set up the 2D editor camera
    viewport::setup_editor_camera_2d(&mut commands, &viewport_image, &viewport, &camera2d_state);

    // Add ambient light
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 200.0,
        ..default()
    });

    if let Some(project) = current_project {
        console_info!("Project", "Opening project: {}", project.config.name);

        let scene_path = project.main_scene_path();
        if scene_path.exists() {
            console_info!("Scene", "Loading scene: {}", scene_path.display());

            match node_system::load_scene(
                &scene_path,
                &mut commands,
                &mut meshes,
                &mut materials,
                &node_registry,
            ) {
                Ok(result) => {
                    info!("Loaded scene: {}", scene_path.display());
                    console_success!("Scene", "Scene loaded successfully: {}", scene_path.file_name().unwrap_or_default().to_string_lossy());

                    // Set the current scene path so Ctrl+S knows where to save
                    scene_state.current_scene_path = Some(scene_path.clone());

                    // Update the first tab with the loaded scene info
                    let scene_name = scene_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("main")
                        .to_string();
                    if let Some(tab) = scene_state.scene_tabs.get_mut(0) {
                        tab.name = scene_name;
                        tab.path = Some(scene_path);
                    }

                    // Apply camera state
                    orbit.focus = Vec3::new(
                        result.editor_camera.orbit_focus[0],
                        result.editor_camera.orbit_focus[1],
                        result.editor_camera.orbit_focus[2],
                    );
                    orbit.distance = result.editor_camera.orbit_distance;
                    orbit.yaw = result.editor_camera.orbit_yaw;
                    orbit.pitch = result.editor_camera.orbit_pitch;

                    // Restore expanded entities in hierarchy
                    for entity in result.expanded_entities {
                        hierarchy.expanded_entities.insert(entity);
                    }
                }
                Err(e) => {
                    error!("Failed to load scene: {}", e);
                    console_error!("Scene", "Failed to load scene: {}", e);
                }
            }
        } else {
            console_warn!("Scene", "Main scene not found: {}", scene_path.display());
        }
    }
}
