//! Renzora Runtime — shared library re-exporting all core engine crates.
//!
//! Plugins and the editor binary link against this dylib instead of
//! statically embedding each crate. Keeps plugin DLLs small.

// Core
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

