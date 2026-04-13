//! Renzora Editor Runtime — core engine systems.
//!
//! Editor panels are now standalone plugin DLLs loaded from the plugins/ folder.
//! This crate only contains the core engine systems needed at runtime.

// Runtime (app setup functions: init_app, build_runtime_app, etc.)
pub use renzora_runtime;

// Core engine systems
pub use renzora_engine;
pub use renzora;
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
pub use renzora_undo;

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

// Critical editor infrastructure (must load before dynamic plugins)
pub use renzora_viewport;
pub use renzora_camera;
pub use renzora_gizmo;
pub use renzora_grid;
pub use renzora_scene;
pub use renzora_keybindings;
pub use renzora_console;
