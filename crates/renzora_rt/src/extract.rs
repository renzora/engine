use crate::{prepare::RtLightingResources, settings::RtLighting};
use bevy::camera::CameraMainTextureUsages;
use bevy::prelude::*;
use bevy::render::{render_resource::TextureUsages, sync_world::RenderEntity, MainWorld};

/// Extracted primary directional light direction (toward light, world space).
/// Falls back to a default sun direction if no directional light exists.
#[derive(Resource, Clone, Copy)]
pub struct ExtractedLightDirection(pub [f32; 3]);

impl Default for ExtractedLightDirection {
    fn default() -> Self {
        // Default: sun at ~50° elevation
        let dir = Vec3::new(0.3, 1.0, 0.2).normalize();
        Self([dir.x, dir.y, dir.z])
    }
}

pub fn extract_rt_lighting(mut main_world: ResMut<MainWorld>, mut commands: Commands) {
    // Extract primary directional light direction
    let mut light_query = main_world.query::<(&DirectionalLight, &GlobalTransform)>();
    let mut light_dir = ExtractedLightDirection::default();
    // Find the first active directional light
    for (dlight, transform) in light_query.iter(&main_world) {
        if dlight.illuminance > 0.0 {
            // DirectionalLight shines along -Z of its transform (back direction = toward light)
            let dir: Vec3 = transform.back().into();
            light_dir = ExtractedLightDirection([dir.x, dir.y, dir.z]);
            break;
        }
    }
    commands.insert_resource(light_dir);

    // Extract RtLighting from cameras — only if CameraMainTextureUsages includes
    // STORAGE_BINDING, otherwise the ViewTarget texture won't have the right usage
    // flags and bind group creation will panic.
    let mut cameras = main_world.query::<(
        RenderEntity,
        &Camera,
        Option<&mut RtLighting>,
        Option<&CameraMainTextureUsages>,
    )>();
    for (entity, camera, rt_lighting, tex_usages) in cameras.iter_mut(&mut main_world) {
        let mut entity_commands = commands
            .get_entity(entity)
            .expect("Camera entity wasn't synced.");
        let has_storage = tex_usages
            .map(|u| u.0.contains(TextureUsages::STORAGE_BINDING))
            .unwrap_or(false);
        if let Some(mut rt_lighting) = rt_lighting {
            if camera.is_active && rt_lighting.enabled && has_storage {
                entity_commands.insert(rt_lighting.clone());
                rt_lighting.reset = false;
            } else {
                entity_commands.remove::<(RtLighting, RtLightingResources)>();
            }
        } else {
            entity_commands.remove::<(RtLighting, RtLightingResources)>();
        }
    }
}
