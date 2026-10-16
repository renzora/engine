pub mod backend;
pub mod backends;
pub mod file;
pub mod legacy_loader;
pub mod material;
pub mod registry;
pub mod runtime;

use bevy::prelude::*;

#[derive(Default)]
pub struct ShaderPlugin;

impl Plugin for ShaderPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] ShaderPlugin");
        app.init_resource::<registry::ShaderBackendRegistry>()
            .add_plugins(runtime::CodeShaderMaterialPlugin)
            // After Bevy's own `.wgsl` loader, so this one is used: it keeps
            // shaders written for the pre-0.20 preprocessor loading.
            .register_asset_loader(legacy_loader::LegacyWgslLoader);

        // Register built-in backends
        let mut reg = app
            .world_mut()
            .resource_mut::<registry::ShaderBackendRegistry>();
        reg.register(Box::new(backends::bevy::BevyBackend));
        reg.register(Box::new(backends::wgsl::WgslBackend));
        reg.register(Box::new(backends::wesl::WeslBackend));
        reg.register(Box::new(backends::glsl::GlslBackend));
        reg.register(Box::new(backends::shadertoy::ShaderToyBackend));
    }
}

renzora::add!(ShaderPlugin);
