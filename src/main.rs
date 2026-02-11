// Hide console window on Windows
#![windows_subsystem = "windows"]

mod blueprint;
mod brushes;
mod commands;
mod component_system;
mod core;
mod crash;
mod embedded;
mod export;
mod gizmo;
mod gltf_animation;
mod input;
#[cfg(feature = "solari")]
mod meshlet;
mod particles;
mod pixel_editor;
mod play_mode;
mod plugin_core;
mod project;
mod scene;
mod scripting;
mod shader_preview;
mod shader_thumbnail;
mod shared;
mod spawn;
mod surface_painting;
mod terrain;
mod theming;
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

use crate::core::AppState;

/// Marker component for the splash screen camera
#[derive(Component)]
struct SplashCamera;

/// Set the window icon from embedded icon data (Windows only)
#[cfg(windows)]
fn set_window_icon(windows: Option<NonSend<WinitWindows>>) {
    let Some(windows) = windows else { return };
    let icon_bytes = include_bytes!("../icon.ico");

    // Parse ICO file to get the largest image (typically 256x256)
    if let Some((rgba, width, height)) = parse_ico_largest(icon_bytes) {
        if let Ok(icon) = winit::window::Icon::from_rgba(rgba, width, height) {
            for window in windows.windows.values() {
                window.set_window_icon(Some(icon.clone()));
            }
        }
    }
}

#[cfg(not(windows))]
fn set_window_icon() {}

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

    // When packed, extract embedded updater and plugins to disk
    embedded::extract_embedded_updater();
    embedded::extract_embedded_plugins();

    let mut app = App::new();

    // When packed, register embedded asset source before DefaultPlugins
    embedded::setup_embedded_assets(&mut app);

    // DLSS requires a project ID before plugin initialization
    // Use a fixed UUID for Renzora Engine (generated once, kept consistent)
    #[cfg(feature = "solari")]
    app.insert_resource(DlssProjectId(Uuid::from_u128(0x52454e5a4f52415f454e47494e455f31)));

    app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Renzora Engine r4".to_string(),
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
        // Register types for Bevy's scene system
        // Shared components
        .register_type::<shared::MeshNodeData>()
        .register_type::<shared::MeshPrimitiveType>()
        // Brush components
        .register_type::<brushes::BrushData>()
        .register_type::<brushes::BrushType>()
        // Terrain components
        .register_type::<terrain::TerrainData>()
        .register_type::<terrain::TerrainChunkData>()
        .register_type::<surface_painting::PaintableSurfaceData>()
        .register_type::<shared::CameraNodeData>()
        .register_type::<shared::CameraRigData>()
        .register_type::<shared::MeshInstanceData>()
        .register_type::<shared::SceneInstanceData>()
        .register_type::<shared::PhysicsBodyData>()
        .register_type::<shared::PhysicsBodyType>()
        .register_type::<shared::CollisionShapeData>()
        .register_type::<shared::CollisionShapeType>()
        .register_type::<shared::Sprite2DData>()
        .register_type::<shared::Camera2DData>()
        .register_type::<shared::UIPanelData>()
        .register_type::<shared::UILabelData>()
        .register_type::<shared::UIButtonData>()
        .register_type::<shared::UIImageData>()
        // Animation components
        .register_type::<shared::GltfAnimations>()
        // Light components
        .register_type::<shared::PointLightData>()
        .register_type::<shared::DirectionalLightData>()
        .register_type::<shared::SpotLightData>()
        .register_type::<shared::SolariLightingData>()
        .register_type::<shared::DlssQualityMode>()
        .register_type::<shared::SunData>()
        // Environment components
        .register_type::<shared::WorldEnvironmentData>()
        .register_type::<shared::SkyMode>()
        .register_type::<shared::ProceduralSkyData>()
        .register_type::<shared::PanoramaSkyData>()
        .register_type::<shared::TonemappingMode>()
        // Post-processing components
        .register_type::<shared::SkyboxData>()
        .register_type::<shared::FogData>()
        .register_type::<shared::AntiAliasingData>()
        .register_type::<shared::AmbientOcclusionData>()
        .register_type::<shared::ReflectionsData>()
        .register_type::<shared::BloomData>()
        .register_type::<shared::TonemappingData>()
        .register_type::<shared::DepthOfFieldData>()
        .register_type::<shared::MotionBlurData>()
        .register_type::<shared::AmbientLightData>()
        .register_type::<shared::CloudsData>()
        // Core components
        .register_type::<core::EditorEntity>()
        .register_type::<core::SceneNode>()
        .register_type::<core::SceneTabId>()
        .register_type::<core::WorldEnvironmentMarker>()
        // Scene roots
        .register_type::<spawn::EditorSceneRoot>()
        .register_type::<spawn::SceneType>()
        // Scripting components
        .register_type::<scripting::ScriptComponent>()
        .register_type::<scripting::ScriptVariables>()
        .register_type::<scripting::ScriptValue>()
        // Scene metadata (editor-only, stripped during export)
        .register_type::<scene::EditorSceneMetadata>()
        // Meshlet components
        .register_type::<shared::MeshletMeshData>()
        // Generic types used by components
        .register_type::<Option<String>>()
        .register_type::<std::path::PathBuf>()
        .register_type::<Option<std::path::PathBuf>>()
        .register_type::<std::collections::HashMap<String, scripting::ScriptValue>>()
        .add_plugins((
            core::CorePlugin,
            core::DiagnosticsPlugin,
            commands::CommandPlugin,
            project::ProjectPlugin,
            component_system::ComponentSystemPlugin,
            viewport::ViewportPlugin,
            viewport::StudioPreviewPlugin,
            viewport::ParticlePreviewPlugin,
            shader_preview::ShaderPreviewPlugin,
            gizmo::GizmoPlugin,
            input::InputPlugin,
            ui::UiPlugin,
            scripting::ScriptingPlugin,
            plugin_core::PluginCorePlugin,
            play_mode::PlayModePlugin,
        ))
        .add_plugins((
            shader_thumbnail::ShaderThumbnailPlugin,
            blueprint::BlueprintPlugin,
            blueprint::MaterialPreviewPlugin,
            brushes::BrushPlugin,
            terrain::TerrainPlugin,
            surface_painting::SurfacePaintingPlugin,
            // Physics plugin (starts paused in editor, activated during play mode)
            shared::RenzoraPhysicsPlugin::new(true),
            // Auto-update system
            update::UpdatePlugin,
            // GLTF animation playback
            gltf_animation::GltfAnimationPlugin,
            // GPU particle effects (Hanabi)
            particles::ParticlesPlugin,
        ));

    // Meshlet/Virtual Geometry integration (requires solari feature)
    #[cfg(feature = "solari")]
    app.add_plugins(meshlet::MeshletIntegrationPlugin);

    app
        // Observer for Bevy scene loading completion
        .add_observer(scene::on_bevy_scene_ready)
        // Initialize app state
        .init_state::<AppState>()
        // Scene saveable registry (required for scene saving)
        .insert_resource(scene::create_default_registry())
        // Crash report window state
        .init_resource::<crash::CrashReportWindowState>()
        // Check for previous crash on startup
        .add_systems(Startup, crash::check_for_previous_crash)
        // Crash report window UI
        .add_systems(
            EguiPrimaryContextPass,
            crash::render_crash_report_window.run_if(in_state(AppState::Editor)),
        )
        // Setup splash camera and window icon on startup
        .add_systems(Startup, (set_window_icon, setup_splash_camera))
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
        // Inspector content exclusive system (renders inspector panel content with World access)
        // Must run after editor_ui to get the panel rect
        .add_systems(
            EguiPrimaryContextPass,
            ui::inspector_content_exclusive.after(ui::editor_ui),
        )
        // Thumbnail loading system - loads and registers asset preview thumbnails
        .add_systems(
            EguiPrimaryContextPass,
            ui::thumbnail_loading_system
                .after(ui::editor_ui)
                .run_if(in_state(AppState::Editor)),
        )
        // Model preview texture registration (for 3D model thumbnails)
        .add_systems(
            EguiPrimaryContextPass,
            viewport::register_model_preview_textures
                .after(ui::editor_ui)
                .run_if(in_state(AppState::Editor)),
        )
        // Shader preview texture registration
        .add_systems(
            EguiPrimaryContextPass,
            shader_preview::register_shader_preview_texture
                .after(ui::editor_ui)
                .run_if(in_state(AppState::Editor)),
        )
        // Shader thumbnail texture registration (for asset browser thumbnails)
        .add_systems(
            EguiPrimaryContextPass,
            shader_thumbnail::register_shader_thumbnail_textures
                .after(ui::editor_ui)
                .run_if(in_state(AppState::Editor)),
        )
        // Window actions system - runs in same schedule as egui to handle drag immediately
        .add_systems(EguiPrimaryContextPass, ui::handle_window_actions.after(ui::editor_ui))
        // Apply orbit camera changes from UI immediately (view angle buttons, axis gizmo clicks, etc.)
        .add_systems(
            EguiPrimaryContextPass,
            viewport::apply_orbit_to_camera
                .after(ui::editor_ui)
                .run_if(in_state(AppState::Editor)),
        )
        // Editor non-UI systems (run when in Editor state) - split into multiple groups
        .add_systems(
            Update,
            (
                viewport::resize_viewport_texture,
                viewport::update_camera_preview,
                input::handle_selection,
                input::handle_view_angles,
                input::handle_view_toggles,
                input::handle_play_mode,
                viewport::update_camera_projection,
                viewport::camera_controller,
                viewport::camera2d_controller,
                viewport::toggle_viewport_cameras,
            )
                .chain()
                .run_if(in_state(AppState::Editor)),
        )
        // Modal transform systems (Blender-style G/R/S shortcuts)
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
                // 3D gizmo systems
                gizmo::gizmo_hover_system,
                gizmo::gizmo_interaction_system,
                gizmo::object_drag_system,
                gizmo::draw_selection_gizmo,
                gizmo::update_selection_outlines,
                // Terrain chunk selection highlight (yellow border on click)
                gizmo::terrain_chunk_selection_system,
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
                // Collider edit systems
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
                // 2D/UI visual rendering
                viewport::update_2d_visuals,
                viewport::cleanup_2d_visuals,
                // Input handling
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
        // Script/blueprint drop onto hierarchy entities
        .add_systems(
            Update,
            input::handle_script_hierarchy_drop
                .after(input::handle_scene_hierarchy_drop)
                .run_if(in_state(AppState::Editor)),
        )
        // Effect file drop onto viewport
        .add_systems(
            Update,
            input::handle_effect_panel_drop
                .run_if(in_state(AppState::Editor)),
        )
        // Drag preview systems (show ghost mesh while dragging model over viewport)
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
        // Ground alignment for dropped models (runs after spawn, needs Aabb computed)
        .add_systems(
            Update,
            input::align_models_to_ground
                .after(input::spawn_loaded_gltfs)
                .run_if(in_state(AppState::Editor)),
        )
        // Scene component rehydration systems (recreate rendering components from data components after scene load)
        .add_systems(
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
                // RaytracingMesh3d is now managed by sync_rendering_settings in viewport/mod.rs
                // based on whether Solari lighting is enabled in the scene
                // Ensure meshes have UVs and tangents for Solari (when enabled)
                scene::prepare_meshes_for_solari,
            )
                .run_if(in_state(AppState::Editor)),
        )
        // Scene management exclusive system (needs &mut World for saving)
        // Must run before camera_controller to avoid 1-frame camera delay on tab switch
        // Must run before assign_scene_tab_ids so newly loaded entities get tab IDs assigned
        .add_systems(
            Update,
            scene::handle_scene_requests
                .before(viewport::camera_controller)
                .before(scene::assign_scene_tab_ids)
                .run_if(in_state(AppState::Editor)),
        )
        // Auto-save system (checks periodically if scene needs saving)
        .add_systems(
            Update,
            scene::auto_save_scene
                .before(scene::handle_scene_requests)
                .run_if(in_state(AppState::Editor)),
        )
        // Scene instance loading exclusive system (needs &mut World for spawning)
        .add_systems(
            Update,
            input::load_scene_instances
                .after(input::handle_scene_hierarchy_drop)
                .run_if(in_state(AppState::Editor)),
        )
        // When entering Editor state: maximize window, despawn splash camera, load scene, and load editor state
        .add_systems(OnEnter(AppState::Editor), (maximize_window, despawn_splash_camera, load_project_scene, load_editor_state).chain())
        // Save editor state periodically (every 5 seconds if dirty)
        .add_systems(Update, save_editor_state_periodic.run_if(in_state(AppState::Editor)))
        // Initialize editor state tracking resources
        .init_resource::<project::EditorStateDirty>()
        .init_resource::<project::LoadedEditorState>()
        // Initialize modal transform state
        .init_resource::<gizmo::ModalTransformState>();

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
    mut theme_manager: ResMut<theming::ThemeManager>,
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
    theme_manager: Res<theming::ThemeManager>,
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
