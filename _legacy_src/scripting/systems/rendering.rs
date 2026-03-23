//! Rendering command processing system
//!
//! Processes queued rendering commands from scripts to modify materials and lights.

use bevy::prelude::*;
use crate::scripting::resources::{RenderingCommand, RenderingCommandQueue};

/// System to process queued rendering commands
pub fn process_rendering_commands(
    mut queue: ResMut<RenderingCommandQueue>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mesh_materials: Query<&MeshMaterial3d<StandardMaterial>>,
    mut point_lights: Query<&mut PointLight>,
    mut spot_lights: Query<&mut SpotLight>,
    mut directional_lights: Query<&mut DirectionalLight>,
) {
    if queue.is_empty() {
        return;
    }

    for cmd in queue.drain() {
        match cmd {
            RenderingCommand::SetMaterialColor { entity, color } => {
                // Get the material handle from the entity
                if let Ok(material_handle) = mesh_materials.get(entity) {
                    // Get and modify the material
                    if let Some(material) = materials.get_mut(&material_handle.0) {
                        material.base_color = Color::srgba(color[0], color[1], color[2], color[3]);
                    }
                }
            }

            RenderingCommand::SetLightIntensity { entity, intensity } => {
                // Try each light type
                if let Ok(mut light) = point_lights.get_mut(entity) {
                    light.intensity = intensity;
                } else if let Ok(mut light) = spot_lights.get_mut(entity) {
                    light.intensity = intensity;
                } else if let Ok(mut light) = directional_lights.get_mut(entity) {
                    light.illuminance = intensity;
                }
            }

            RenderingCommand::SetLightColor { entity, color } => {
                let light_color = Color::srgb(color[0], color[1], color[2]);

                // Try each light type
                if let Ok(mut light) = point_lights.get_mut(entity) {
                    light.color = light_color;
                } else if let Ok(mut light) = spot_lights.get_mut(entity) {
                    light.color = light_color;
                } else if let Ok(mut light) = directional_lights.get_mut(entity) {
                    light.color = light_color;
                }
            }
        }
    }
}
