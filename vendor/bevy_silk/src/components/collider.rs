use bevy::{ecs::component::Component, reflect::Reflect};

/// Enables collisions on a cloth entity
///
/// The collisions will be detected through a cuboid shape using the cloth AABB
/// bounding box.
#[derive(Debug, Clone, Component, Reflect)]
pub struct ClothCollider {
    /// offset to apply on collision projected point to prevent clipping
    pub offset: f32,
    /// Coefficient of the received velocity to apply to cloth:
    /// - 0 meaning no velocity will be applied
    /// - 1 meaning velocity is fully applied
    /// - 2 meaning the double velocity is applied
    pub velocity_coefficient: f32,
    /// Defines the velocity reduction coefficient for dynamic rigibodies
    /// colliding with the cloth, improving the cloth effect.
    pub dampen_others: Option<f32>,
}

impl Default for ClothCollider {
    fn default() -> Self {
        Self {
            offset: 0.25,
            velocity_coefficient: 1.0,
            dampen_others: None,
        }
    }
}
