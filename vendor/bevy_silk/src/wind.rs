use bevy::{ecs::prelude::Resource, math::Vec3, reflect::Reflect};

/// Wind definition for cloth physics
#[derive(Debug, Clone, Reflect)]
pub enum Wind {
    /// Constant Wind force
    ConstantWind {
        /// Wind velocity
        velocity: Vec3,
    },
    /// Wind force following a sin wave
    SinWave {
        /// Wind velocity at the top of the sin wave
        max_velocity: Vec3,
        /// sin wave frequency
        frequency: f32,
        /// If set to true the wave will be normalized between 0 and 1 and avoid
        /// negative values
        normalize: bool,
        /// Use absolute values, making the wave act as a bouncing signal
        abs: bool,
    },
}

/// Wind forces resource for cloth physics
#[derive(Debug, Clone, Reflect, Resource, Default)]
pub struct Winds {
    /// Array of wind forces
    pub wind_forces: Vec<Wind>,
}

impl Default for Wind {
    fn default() -> Self {
        Self::SinWave {
            max_velocity: Vec3::ZERO,
            frequency: 0.5,
            normalize: true,
            abs: false,
        }
    }
}

impl Wind {
    /// Retrieves the current wind velocity according to the elapsed time since
    /// startup
    #[must_use]
    pub fn current_velocity(&self, elapsed_time: f32) -> Vec3 {
        match self {
            Self::ConstantWind { velocity } => *velocity,
            Self::SinWave {
                max_velocity,
                frequency,
                normalize,
                abs,
            } => {
                let mut sin_value = (elapsed_time * frequency).sin();
                if *normalize {
                    sin_value = f32::midpoint(sin_value, 1.0);
                }
                if *abs {
                    sin_value = sin_value.abs();
                }
                sin_value * *max_velocity
            }
        }
    }
}

impl Winds {
    /// Retrieves the current winds velocity sum according to the elapsed time
    /// since startup
    #[must_use]
    pub fn current_velocity(&self, elapsed_time: f32) -> Vec3 {
        self.wind_forces
            .iter()
            .map(|w| w.current_velocity(elapsed_time))
            .sum()
    }
}

impl From<Wind> for Winds {
    fn from(wind: Wind) -> Self {
        Self {
            wind_forces: vec![wind],
        }
    }
}

impl From<Vec<Wind>> for Winds {
    fn from(wind_forces: Vec<Wind>) -> Self {
        Self { wind_forces }
    }
}
