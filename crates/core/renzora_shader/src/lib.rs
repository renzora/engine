pub mod backend;
pub mod registry;
pub mod file;
pub mod runtime;
pub mod backends;

use bevy::prelude::*;

pub struct ShaderPlugin;

impl Plugin for ShaderPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] ShaderPlugin");
        app.init_resource::<registry::ShaderBackendRegistry>()
            .add_plugins(runtime::CodeShaderMaterialPlugin);

        // Register built-in backends
        let mut reg = app.world_mut().resource_mut::<registry::ShaderBackendRegistry>();
        reg.register(Box::new(backends::bevy::BevyBackend));
        reg.register(Box::new(backends::wgsl::WgslBackend));
        reg.register(Box::new(backends::glsl::GlslBackend));
        reg.register(Box::new(backends::shadertoy::ShaderToyBackend));
    }
}
