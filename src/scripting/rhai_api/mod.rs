//! Rhai API function registration modules
//!
//! Each module registers functions for a specific category that scripts can call.
//! Command functions automatically queue their Maps via a thread-local buffer,
//! so scripts don't need to manually push into `_commands`.

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
mod particles;
mod entity_access;

use rhai::Engine;
use std::cell::RefCell;

use super::rhai_commands::RhaiCommand;

thread_local! {
    /// Buffer for typed commands produced by API functions during a single script call.
    /// Drained by the engine after each on_ready / on_update invocation.
    static COMMAND_BUFFER: RefCell<Vec<RhaiCommand>> = RefCell::new(Vec::new());
}

/// Push a typed command into the thread-local buffer (called by API functions).
pub fn push_command(cmd: RhaiCommand) {
    COMMAND_BUFFER.with(|buf| buf.borrow_mut().push(cmd));
}

/// Drain all buffered commands (called by the engine after script execution).
pub fn drain_commands() -> Vec<RhaiCommand> {
    COMMAND_BUFFER.with(|buf| buf.borrow_mut().drain(..).collect())
}

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
    particles::register(engine);
    entity_access::register(engine);
}
