//! Rhai API function registration modules
//!
//! Each module registers functions for a specific category that scripts can call.

mod transform;
mod input;
mod math;
mod ecs;
mod physics;
mod audio;
mod time;
mod debug;
mod environment;
mod rendering;
mod animation;
mod camera;
mod components;
mod scene;

use rhai::Engine;

/// Register all Rhai API functions
pub fn register_all(engine: &mut Engine) {
    transform::register(engine);
    input::register(engine);
    math::register(engine);
    ecs::register(engine);
    physics::register(engine);
    audio::register(engine);
    time::register(engine);
    debug::register(engine);
    environment::register(engine);
    rendering::register(engine);
    animation::register(engine);
    camera::register(engine);
    components::register(engine);
    scene::register(engine);
}
