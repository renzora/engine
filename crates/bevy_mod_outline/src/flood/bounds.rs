use bevy::ecs::component::Component;
use bevy::math::{Affine3A, Mat4, UVec2, Vec4Swizzles};
use bevy::prelude::*;
use bevy::render::camera::Viewport;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::primitives::Aabb;

use crate::computed::ComputedOutline;
use crate::uniforms::DrawMode;

/// Component for storing mesh bounds information needed for scissoring jump-flood operations
#[derive(Clone, Component)]
pub struct FloodMeshBounds {
    /// The axis-aligned bounding box of the mesh in model space
    pub aabb: Aabb,
    /// The world-from-local transform for the entity
    pub world_from_local: Affine3A,
}

impl FloodMeshBounds {
    pub fn calculate_screen_space_bounds(
        &self,
        clip_from_world: &Mat4,
        viewport: &Viewport,
        border: u32,
    ) -> Option<URect> {
        // Get the 8 corners of the AABB
        let min = self.aabb.min();
        let max = self.aabb.max();
        let corners = [
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(min.x, max.y, max.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(max.x, max.y, max.z),
        ];

        // Initialize min/max screen coordinates with extreme values
        let mut min_screen = UVec2::MAX;
        let mut max_screen = UVec2::MIN;

        // Use the physical target size for width and height
        let width = viewport.physical_size.x;
        let height = viewport.physical_size.y;

        // Project each corner to screen space and update min/max values
        for corner in corners.iter() {
            // Convert corner to world space
            let world_pos = self.world_from_local.transform_point3(*corner);

            // Project to clip space
            let clip_pos = *clip_from_world * world_pos.extend(1.0);

            // Perspective divide to get NDC coordinates
            let ndc = clip_pos.xyz() / clip_pos.w;

            // Skip if behind camera
            if ndc.z < -1.0 || ndc.z > 1.0 {
                continue;
            }

            // Convert from NDC to pixel coordinates
            let screen_x = ((ndc.x + 1.0) * 0.5 * width as f32).max(0.0) as u32;
            let screen_y = ((-ndc.y + 1.0) * 0.5 * height as f32).max(0.0) as u32;

            min_screen.x = min_screen.x.min(screen_x);
            min_screen.y = min_screen.y.min(screen_y);
            max_screen.x = max_screen.x.max(screen_x);
            max_screen.y = max_screen.y.max(screen_y);
        }

        // If nothing is in view, return None
        if min_screen.x >= max_screen.x || min_screen.y >= max_screen.y {
            None
        } else {
            Some(URect::new(
                viewport.physical_position.x + min_screen.x.saturating_sub(border).min(width),
                viewport.physical_position.y + min_screen.y.saturating_sub(border).min(height),
                viewport.physical_position.x + (max_screen.x + border).min(width),
                viewport.physical_position.y + (max_screen.y + border).min(height),
            ))
        }
    }
}

impl ExtractComponent for FloodMeshBounds {
    type QueryData = (
        &'static Aabb,
        &'static GlobalTransform,
        &'static ComputedOutline,
    );
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(
        (aabb, global_transform, computed_outline): bevy::ecs::query::QueryItem<Self::QueryData>,
    ) -> Option<Self::Out> {
        let Some(computed_outline) = &computed_outline.0 else {
            return None;
        };

        match computed_outline.mode.value.draw_mode {
            DrawMode::JumpFlood => Some(FloodMeshBounds {
                aabb: *aabb,
                world_from_local: global_transform.affine(),
            }),
            _ => None,
        }
    }
}
