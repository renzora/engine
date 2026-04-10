//! Renzora Editor Runtime — core engine systems.
//!
//! Editor panels are now standalone plugin DLLs loaded from the plugins/ folder.
//! This crate only contains the core engine systems needed at runtime.

// Core engine systems
pub use renzora_engine;
pub use renzora_core;
pub use renzora_scripting;
pub use renzora_blueprint;
pub use renzora_input;
pub use renzora_physics;
pub use renzora_lifecycle;
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
pub use renzora_skybox;
pub use renzora_clouds;
pub use renzora_night_stars;

// Editor framework (compiled into renzora.dll SDK)
pub use renzora_editor_framework;
pub use renzora_splash;

// Editor infrastructure
pub use renzora_viewport;
pub use renzora_camera;
pub use renzora_gizmo;
pub use renzora_grid;
pub use renzora_scene;
pub use renzora_keybindings;

// Editor panels (baked in)
pub use renzora_console;
pub use renzora_inspector;
pub use renzora_hierarchy;
pub use renzora_asset_browser;
pub use renzora_export;
pub use renzora_import_ui;
pub use renzora_settings;
pub use renzora_material_editor;
pub use renzora_shader_editor;
pub use renzora_blueprint_editor;
pub use renzora_particle_editor;
pub use renzora_terrain_editor;
pub use renzora_foliage_editor;
pub use renzora_animation_editor;
pub use renzora_network_editor;
pub use renzora_lifecycle_editor;
pub use renzora_code_editor;
pub use renzora_script_variables;
pub use renzora_mixer;
pub use renzora_daw;
pub use renzora_hub;
pub use renzora_auth;
pub use renzora_level_presets;
pub use renzora_physics_playground;
pub use renzora_gamepad;
pub use renzora_debugger;
pub use renzora_widget_gallery;
pub use renzora_tutorial;
pub use renzora_test_component;
pub use renzora_test_extension;
pub use renzora_shape_library;
