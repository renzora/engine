//! Surface painting component registration (delegates to surface_painting module).

use crate::component_system::ComponentRegistry;

pub fn register(registry: &mut ComponentRegistry) {
    crate::surface_painting::component::register(registry);
}
