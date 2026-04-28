//! Renzora Runtime — shared engine library used by every Renzora binary.
//!
//! `add_engine_plugins(app)` registers all runtime/engine plugins. Editor
//! builds additionally call `add_editor_plugins(app)` (gated behind the
//! `editor` feature) to install the editor framework + first-party panels.
//!
//! Adding a new plugin:
//! - Editor + runtime (engine system): add to `add_engine_plugins`.
//! - Editor only (panel, infra): add to `add_editor_plugins` and to the
//!   `editor` feature in `Cargo.toml`.
//! - Third-party: ship as a dylib that uses `renzora::add!` — loaded at
//!   runtime by `dynamic_plugin_loader`, no engine source edits required.

use bevy::prelude::*;

// Core
pub use renzora_engine;
pub use renzora;
pub use renzora_scripting;
pub use renzora_blueprint;
pub use renzora_input;
pub use renzora_physics;
pub use renzora_terrain;
pub use renzora_lighting;
pub use renzora_water;
pub use renzora_animation;
pub use renzora_game_ui;
pub use renzora_gauges;
pub use renzora_hanabi;
pub use renzora_network;
pub use renzora_audio;
pub use renzora_shader;

// Postprocess framework + vital effects
pub use renzora_postprocess;
pub use renzora_tonemapping;
pub use renzora_bloom_effect;
pub use renzora_antialiasing;
pub use renzora_ssao;
pub use renzora_ssr;
pub use renzora_auto_exposure;
pub use renzora_oit;
pub use renzora_dof;
pub use renzora_motion_blur;
pub use renzora_distance_fog;

// Environment
pub use renzora_atmosphere;
pub use renzora_environment_map;
pub use renzora_skybox;
pub use renzora_clouds;
pub use renzora_night_stars;

// ── Editor-only re-exports (only present when `editor` feature is on) ─────
#[cfg(feature = "editor")]
mod editor_reexports {
    // Editor framework + infrastructure
    pub use renzora_undo;
    pub use renzora_editor;
    pub use renzora_splash;
    pub use renzora_viewport;
    pub use renzora_camera;
    pub use renzora_gizmo;
    pub use renzora_grid;
    pub use renzora_scene;
    pub use renzora_keybindings;
    pub use renzora_console;

    // First-party editor panels
    pub use renzora_animation_editor;
    pub use renzora_asset_browser;
    pub use renzora_auth;
    pub use renzora_blueprint_editor;
    pub use renzora_code_editor;
    pub use renzora_command_palette;
    pub use renzora_context_menu;
    pub use renzora_daw;
    pub use renzora_debugger;
    pub use renzora_export;
    pub use renzora_foliage_editor;
    pub use renzora_gamepad;
    pub use renzora_hierarchy;
    pub use renzora_history;
    pub use renzora_hub;
    pub use renzora_import_ui;
    pub use renzora_inspector;
    pub use renzora_level_presets;
    pub use renzora_material_editor;
    pub use renzora_mesh_draw;
    pub use renzora_mesh_edit;
    pub use renzora_mixer;
    pub use renzora_network_editor;
    pub use renzora_particle_editor;
    pub use renzora_physics_playground;
    pub use renzora_script_variables;
    pub use renzora_sequencer;
    pub use renzora_settings;
    pub use renzora_shader_editor;
    pub use renzora_shape_library;
    pub use renzora_system_monitor;
    pub use renzora_terrain_editor;
    pub use renzora_test_component;
    pub use renzora_theme_status;
    pub use renzora_tutorial;
    pub use renzora_vst;
    pub use renzora_widget_gallery;
}

#[cfg(feature = "editor")]
pub use editor_reexports::*;

// ── App setup (single source of truth for engine plugin registration) ────

pub fn platform_wgpu_settings() -> bevy::render::settings::WgpuSettings {
    #[cfg(target_os = "android")]
    {
        use bevy::render::settings::{Backends, WgpuSettings};
        WgpuSettings {
            backends: Some(Backends::VULKAN),
            ..default()
        }
    }

    #[cfg(not(target_os = "android"))]
    {
        use bevy::render::settings::{WgpuFeatures, WgpuSettings};
        WgpuSettings {
            features: WgpuFeatures::POLYGON_MODE_LINE,
            ..default()
        }
    }
}

pub fn init_app() -> App {
    let mut app = App::new();
    renzora_engine::setup_asset_reader(&mut app);
    app
}

pub fn add_default_rendering(app: &mut App) {
    use bevy::render::{settings::RenderCreation, RenderPlugin};
    use bevy::window::{Window, WindowPlugin};
    app.add_plugins(
        DefaultPlugins
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(platform_wgpu_settings()),
                ..default()
            })
            .set(ImagePlugin {
                default_sampler: bevy::image::ImageSamplerDescriptor {
                    address_mode_u: bevy::image::ImageAddressMode::Repeat,
                    address_mode_v: bevy::image::ImageAddressMode::Repeat,
                    address_mode_w: bevy::image::ImageAddressMode::Repeat,
                    // Trilinear + 16x anisotropic filtering by default. Bevy's
                    // default sampler runs with `anisotropy_clamp = 1` which
                    // looks blocky on textures viewed at oblique angles
                    // (brick walls, ground planes). Most assets here come from
                    // Sketchfab GLBs without baked mipmaps — anisotropy alone
                    // already cleans up the worst aliasing; full mipmap
                    // generation is a separate piece of work.
                    mag_filter: bevy::image::ImageFilterMode::Linear,
                    min_filter: bevy::image::ImageFilterMode::Linear,
                    mipmap_filter: bevy::image::ImageFilterMode::Linear,
                    anisotropy_clamp: 16,
                    ..default()
                },
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Renzora".into(),
                    decorations: false,
                    resizable: true,
                    ..default()
                }),
                ..default()
            })
    );
    app.add_systems(Startup, maximize_primary_window);
}

fn maximize_primary_window(
    mut windows: Query<&mut bevy::window::Window, With<bevy::window::PrimaryWindow>>,
) {
    if let Ok(mut window) = windows.single_mut() {
        window.set_maximized(true);
    }
}

pub fn add_engine_plugins(app: &mut App) {
    app.add_plugins(renzora_engine::RuntimePlugin);
    app.add_plugins(renzora_scripting::ScriptingPlugin::new());
    app.add_plugins(renzora_blueprint::BlueprintPlugin);
    app.add_plugins(renzora_input::InputPlugin);
    app.add_plugins(renzora_physics::PhysicsPlugin);
    app.add_plugins(renzora_globals::GlobalsPlugin);
    app.add_plugins(renzora_terrain::TerrainPlugin);
    app.add_plugins(renzora_spline::SplinePlugin);
    app.add_plugins(renzora_lighting::LightingPlugin);
    app.add_plugins(renzora_water::WaterPlugin);
    app.add_plugins(renzora_terrain::foliage::FoliagePlugin);
    app.add_plugins(renzora_animation::AnimationPlugin);
    app.add_plugins(renzora_game_ui::GameUiPlugin);
    app.add_plugins(renzora_shader::material::MaterialPlugin);
    app.add_plugins(renzora_gauges::GaugesPlugin);
    app.add_plugins(renzora_hanabi::HanabiParticlePlugin);
    app.add_plugins(renzora_network::NetworkPlugin);
    app.add_plugins(renzora_audio::KiraPlugin);
    app.add_plugins(renzora_shader::ShaderPlugin);
    app.add_plugins(renzora_skybox::SkyboxPlugin);
    app.add_plugins(renzora_night_stars::NightStarsPlugin);
    app.add_plugins(renzora_clouds::CloudsPlugin);
    app.add_plugins(renzora_tonemapping::TonemappingPlugin);
    app.add_plugins(renzora_bloom_effect::BloomEffectPlugin);
    app.add_plugins(renzora_dof::DepthOfFieldPlugin);
    app.add_plugins(renzora_motion_blur::MotionBlurPlugin);
    app.add_plugins(renzora_antialiasing::AntiAliasingPlugin);
    app.add_plugins(renzora_distance_fog::DistanceFogPlugin);
    app.add_plugins(renzora_atmosphere::AtmospherePlugin);
    app.add_plugins(renzora_environment_map::EnvironmentMapPlugin);
    app.add_plugins(renzora_ssao::SsaoPlugin);
    app.add_plugins(renzora_ssr::SsrPlugin);
    app.add_plugins(renzora_auto_exposure::AutoExposurePlugin);
    app.add_plugins(renzora_oit::OitPlugin);
}

/// Build the full runtime app (rendering + all engine plugins).
pub fn build_runtime_app() -> App {
    let mut app = init_app();
    add_default_rendering(&mut app);
    add_engine_plugins(&mut app);
    app
}

// ── Editor plugin registration (only present when `editor` feature is on) ─

/// Registers the editor framework, infrastructure, and all first-party
/// panels with the App. Mirrors `add_engine_plugins` — single source of
/// truth for editor-side plugin registration. Called from `main.rs` after
/// `add_engine_plugins`.
#[cfg(feature = "editor")]
pub fn add_editor_plugins(app: &mut App) {
    // Editor infrastructure (must load before panels)
    app.add_plugins(renzora_undo::UndoPlugin);
    app.add_plugins(renzora_splash::SplashPlugin);
    app.add_plugins(renzora_asset_registry::AssetRegistryPlugin);
    app.add_plugins(renzora_editor::RenzoraEditorPlugin);
    app.add_plugins(renzora_grid::GridPlugin);
    app.add_plugins(renzora_keybindings::KeybindingsPlugin);
    app.add_plugins(renzora_viewport::ViewportPlugin);
    app.add_plugins(renzora_camera::CameraPlugin);
    app.add_plugins(renzora_gizmo::GizmoPlugin);
    app.add_plugins(renzora_scene::ScenePlugin);
    app.add_plugins(renzora_console::ConsolePlugin);

    // First-party editor panels
    app.add_plugins(renzora_animation_editor::AnimationEditorPlugin);
    app.add_plugins(renzora_asset_browser::AssetBrowserPlugin);
    app.add_plugins(renzora_auth::AuthPlugin);
    app.add_plugins(renzora_blueprint_editor::BlueprintEditorPlugin);
    app.add_plugins(renzora_code_editor::CodeEditorPlugin);
    app.add_plugins(renzora_command_palette::CommandPalettePlugin);
    app.add_plugins(renzora_context_menu::ContextMenuPlugin);
    app.add_plugins(renzora_daw::DawPlugin);
    app.add_plugins(renzora_debugger::DebuggerPlugin);
    app.add_plugins(renzora_export::ExportPlugin);
    app.add_plugins(renzora_foliage_editor::FoliageEditorPlugin);
    app.add_plugins(renzora_gamepad::GamepadPlugin);
    app.add_plugins(renzora_hierarchy::HierarchyPanelPlugin);
    app.add_plugins(renzora_history::HistoryPanelPlugin);
    app.add_plugins(renzora_hub::HubPlugin);
    app.add_plugins(renzora_import_ui::ImportPlugin);
    app.add_plugins(renzora_inspector::InspectorPanelPlugin);
    app.add_plugins(renzora_level_presets::LevelPresetsPlugin);
    app.add_plugins(renzora_material_editor::MaterialEditorPlugin);
    app.add_plugins(renzora_mesh_draw::MeshDrawPlugin);
    app.add_plugins(renzora_mesh_edit::MeshEditPlugin);
    app.add_plugins(renzora_mixer::MixerPlugin);
    app.add_plugins(renzora_network_editor::NetworkEditorPlugin);
    app.add_plugins(renzora_particle_editor::ParticleEditorPlugin);
    app.add_plugins(renzora_physics_playground::PhysicsPanelPlugin);
    app.add_plugins(renzora_script_variables::ScriptVariablesPlugin);
    app.add_plugins(renzora_sequencer::SequencerPlugin);
    app.add_plugins(renzora_settings::SettingsPlugin);
    app.add_plugins(renzora_shader_editor::ShaderEditorPlugin);
    app.add_plugins(renzora_shape_library::ShapeLibraryPlugin);
    app.add_plugins(renzora_system_monitor::SystemMonitorPlugin);
    app.add_plugins(renzora_terrain_editor::TerrainEditorPlugin);
    app.add_plugins(renzora_test_component::TestComponentPlugin);
    app.add_plugins(renzora_theme_status::ThemeStatusPlugin);
    app.add_plugins(renzora_tutorial::TutorialPlugin);
    app.add_plugins(renzora_vst::PluginHostPlugin);
    app.add_plugins(renzora_widget_gallery::WidgetGalleryPlugin);
}

