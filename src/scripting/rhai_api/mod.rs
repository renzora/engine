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
use std::collections::HashSet;

use super::rhai_commands::RhaiCommand;

thread_local! {
    /// Buffer for typed commands produced by API functions during a single script call.
    /// Drained by the engine after each on_ready / on_update invocation.
    static COMMAND_BUFFER: RefCell<Vec<RhaiCommand>> = RefCell::new(Vec::new());

    /// Set of entity IDs (as u64 bits) that currently have active sounds.
    /// Populated per-frame before script execution by the script runner.
    static AUDIO_PLAYING_ENTITIES: RefCell<HashSet<u64>> = RefCell::new(HashSet::new());
}

/// Push a typed command into the thread-local buffer (called by API functions).
pub fn push_command(cmd: RhaiCommand) {
    COMMAND_BUFFER.with(|buf| buf.borrow_mut().push(cmd));
}

/// Drain all buffered commands (called by the engine after script execution).
pub fn drain_commands() -> Vec<RhaiCommand> {
    COMMAND_BUFFER.with(|buf| buf.borrow_mut().drain(..).collect())
}

/// Set the audio playing entities for the current frame (called before script execution).
pub fn set_audio_playing_entities(entities: HashSet<u64>) {
    AUDIO_PLAYING_ENTITIES.with(|s| *s.borrow_mut() = entities);
}

/// Check if an entity has active sounds playing.
pub fn is_entity_sound_playing(entity_id: u64) -> bool {
    AUDIO_PLAYING_ENTITIES.with(|s| s.borrow().contains(&entity_id))
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
