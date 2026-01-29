//! Component definitions for the component registry

mod ai;
mod audio;
mod camera;
mod combat;
mod effects;
mod environment;
pub mod gameplay;
mod interaction;
mod lighting;
mod physics;
mod rendering;
mod scripting;
mod ui;

// Re-export commonly used gameplay components
pub use gameplay::HealthData;

use super::ComponentRegistry;

/// Register all built-in components
pub fn register_all_components(registry: &mut ComponentRegistry) {
    rendering::register(registry);
    lighting::register(registry);
    camera::register(registry);
    physics::register(registry);
    audio::register(registry);
    scripting::register(registry);
    ui::register(registry);
    environment::register(registry);
    effects::register(registry);
    gameplay::register(registry);
    interaction::register(registry);
    ai::register(registry);
    combat::register(registry);
}
