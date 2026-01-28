//! Camera effects resources for scripts
//!
//! Provides screen shake, camera follow, and zoom control for scripts.

use bevy::prelude::*;

/// A queued camera command from a script
#[derive(Clone, Debug)]
pub enum CameraCommand {
    SetTarget {
        position: Vec3,
    },
    SetZoom {
        zoom: f32,
    },
    ScreenShake {
        intensity: f32,
        duration: f32,
    },
    /// Follow an entity smoothly
    FollowEntity {
        entity: Entity,
        offset: Vec3,
        smoothing: f32,
    },
    /// Stop following
    StopFollow,
}

/// Resource to queue camera commands from scripts
#[derive(Resource, Default)]
pub struct CameraCommandQueue {
    pub commands: Vec<CameraCommand>,
}

impl CameraCommandQueue {
    pub fn push(&mut self, cmd: CameraCommand) {
        self.commands.push(cmd);
    }

    pub fn drain(&mut self) -> Vec<CameraCommand> {
        std::mem::take(&mut self.commands)
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

/// Resource tracking active camera effects
#[derive(Resource)]
pub struct ScriptCameraState {
    /// Target position for camera to look at (if set)
    pub look_target: Option<Vec3>,

    /// Zoom level (1.0 = default, <1.0 = zoomed out, >1.0 = zoomed in)
    pub zoom: f32,

    /// Screen shake state
    pub shake: Option<ScreenShakeState>,

    /// Entity to follow (if any)
    pub follow_entity: Option<Entity>,
    /// Offset from followed entity
    pub follow_offset: Vec3,
    /// Smoothing factor for follow (0.0 = instant, 1.0 = very slow)
    pub follow_smoothing: f32,

    /// Original camera transform before effects (to restore on stop)
    pub original_transform: Option<Transform>,
}

impl Default for ScriptCameraState {
    fn default() -> Self {
        Self {
            look_target: None,
            zoom: 1.0,
            shake: None,
            follow_entity: None,
            follow_offset: Vec3::ZERO,
            follow_smoothing: 0.1,
            original_transform: None,
        }
    }
}

/// State for active screen shake effect
#[derive(Clone, Debug)]
pub struct ScreenShakeState {
    /// Current intensity (decreases over time)
    pub intensity: f32,
    /// Initial intensity
    pub initial_intensity: f32,
    /// Remaining duration
    pub remaining: f32,
    /// Total duration
    pub duration: f32,
    /// Random seed for shake pattern
    pub seed: f32,
}

impl ScreenShakeState {
    pub fn new(intensity: f32, duration: f32) -> Self {
        Self {
            intensity,
            initial_intensity: intensity,
            remaining: duration,
            duration,
            seed: rand_seed(),
        }
    }

    /// Update the shake state, returns the shake offset for this frame
    pub fn update(&mut self, delta: f32) -> Vec3 {
        self.remaining -= delta;

        if self.remaining <= 0.0 {
            return Vec3::ZERO;
        }

        // Decay intensity over time
        let progress = 1.0 - (self.remaining / self.duration);
        self.intensity = self.initial_intensity * (1.0 - progress);

        // Generate pseudo-random shake offset
        let time = self.duration - self.remaining;
        let freq = 25.0; // Shake frequency

        let x = (time * freq + self.seed).sin() * self.intensity;
        let y = (time * freq * 1.3 + self.seed * 2.0).cos() * self.intensity;
        let z = (time * freq * 0.7 + self.seed * 3.0).sin() * self.intensity * 0.5;

        Vec3::new(x, y, z)
    }

    /// Check if shake is still active
    pub fn is_active(&self) -> bool {
        self.remaining > 0.0
    }
}

/// Simple pseudo-random seed generator
fn rand_seed() -> f32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(12345);
    (nanos as f32) / 1_000_000_000.0 * 100.0
}
