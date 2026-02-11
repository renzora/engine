use bevy::{
    ecs::prelude::{Component, ReflectComponent, Resource},
    math::Vec3,
    reflect::Reflect,
};

/// Defines how verlet physics acceleration components like gravity and winds
/// are smoothed through every frame.
///
/// By default, accelerations are multiplied by the squared value of the elapsed
/// time since last frame (delta time) but if you notice some jittery behaviour
/// a fixed coefficient can enforce a smooth simulation
#[derive(Debug, Copy, Clone, Default, Reflect)]
pub enum AccelerationSmoothing {
    #[default]
    /// Default smoothing behaviour, accelerations are multiplied by the squared
    /// value of the elapsed time since last frame (delta time)
    SquaredDeltaTime,
    /// Fixed smooth coefficient, applied to gravity and wind once per frame.
    ///
    /// Note: Pick a very small number (< 0.1)
    FixedCoefficient(f32),
}

/// Cloth physics configuration.
///
/// Used as a resource, it is used as a global configuration for every cloth
/// entity. Used as a component on a cloth entity, it overrides the global
/// values for that cloth.
#[derive(Debug, Clone, Component, Reflect, Resource)]
#[reflect(Component)]
pub struct ClothConfig {
    /// Custom gravity, classic (0, -9.81, 0) is used by default
    pub gravity: Vec3,
    /// Custom friction to apply to velocity. Useful to reduce the elasticity of
    /// a cloth.
    ///
    /// Note: The friction is not applied to external accelerations like gravity
    /// and winds Note: will be clamped between 0.0 and 1.0
    pub friction: f32,
    /// Sets the number of sticks computation iteration.
    /// The higher the value, the more precision and less elasticity for the
    /// sticks but the cost is increased
    pub sticks_computation_depth: u8,
    /// Smoothing behaviour for gravity and winds
    pub acceleration_smoothing: AccelerationSmoothing,
}

impl ClothConfig {
    /// Default Y value for gravity
    pub const DEFAULT_GRAVITY: f32 = -9.81;

    #[must_use]
    #[inline]
    pub(crate) fn friction_coefficient(&self) -> f32 {
        1.0 - self.friction.clamp(0.0, 1.0)
    }

    /// Applies smoothing to a given `acceleration` value.
    ///
    /// # Arguments
    ///
    /// * `acceleration`- the acceleration vector to smooth
    /// * `delta_time` - elapsed time since last frame in seconds
    #[inline]
    #[must_use]
    pub fn smoothed_acceleration(&self, acceleration: Vec3, delta_time: f32) -> Vec3 {
        acceleration * self.smooth_value(delta_time)
    }

    /// Retrieves the current smooth value
    ///
    /// # Arguments
    ///
    /// * `delta_time` - elapsed time since last frame in seconds
    #[inline]
    #[must_use]
    pub fn smooth_value(&self, delta_time: f32) -> f32 {
        match self.acceleration_smoothing {
            AccelerationSmoothing::SquaredDeltaTime => delta_time * delta_time,
            AccelerationSmoothing::FixedCoefficient(coef) => coef,
        }
    }

    /// Initializes a cloth config with no gravity force
    #[must_use]
    #[inline]
    pub fn no_gravity() -> Self {
        Self {
            gravity: Vec3::ZERO,
            ..Default::default()
        }
    }
}

impl Default for ClothConfig {
    fn default() -> Self {
        Self {
            gravity: Vec3::Y * Self::DEFAULT_GRAVITY,
            friction: 0.01,
            sticks_computation_depth: 5,
            acceleration_smoothing: Default::default(),
        }
    }
}
