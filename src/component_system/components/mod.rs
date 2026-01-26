//! Component definitions for the component registry

mod audio;
mod camera;
mod lighting;
mod physics;
mod rendering;
mod scripting;
mod ui;

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
}
