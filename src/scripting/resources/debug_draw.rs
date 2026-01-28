//! Debug drawing system for scripts
//!
//! Allows scripts to draw debug shapes (lines, spheres, boxes, etc.)
//! that are rendered using Bevy's gizmos system.

use bevy::prelude::*;
use std::collections::VecDeque;

/// A debug draw command
#[derive(Clone, Debug)]
pub enum DebugDrawCommand {
    Line {
        start: Vec3,
        end: Vec3,
        color: Color,
        duration: f32,
    },
    Ray {
        origin: Vec3,
        direction: Vec3,
        length: f32,
        color: Color,
        duration: f32,
    },
    Sphere {
        center: Vec3,
        radius: f32,
        color: Color,
        duration: f32,
    },
    Box {
        center: Vec3,
        half_extents: Vec3,
        color: Color,
        duration: f32,
    },
    Point {
        position: Vec3,
        size: f32,
        color: Color,
        duration: f32,
    },
}

/// A debug draw that persists for a duration
struct PersistentDraw {
    command: DebugDrawCommand,
    /// Remaining time before expiration (seconds)
    remaining: f32,
}

/// Resource for immediate and persistent debug draws
#[derive(Resource, Default)]
pub struct DebugDrawQueue {
    /// Draws that happen this frame only (duration = 0)
    pub immediate: Vec<DebugDrawCommand>,
    /// Draws that persist for a duration
    pub persistent: VecDeque<PersistentDraw>,
}

impl DebugDrawQueue {
    /// Add a debug draw command
    pub fn push(&mut self, cmd: DebugDrawCommand) {
        let duration = match &cmd {
            DebugDrawCommand::Line { duration, .. } => *duration,
            DebugDrawCommand::Ray { duration, .. } => *duration,
            DebugDrawCommand::Sphere { duration, .. } => *duration,
            DebugDrawCommand::Box { duration, .. } => *duration,
            DebugDrawCommand::Point { duration, .. } => *duration,
        };

        if duration <= 0.0 {
            self.immediate.push(cmd);
        } else {
            self.persistent.push_back(PersistentDraw {
                command: cmd,
                remaining: duration,
            });
        }
    }

    /// Tick persistent draws and remove expired ones
    pub fn tick(&mut self, delta: f32) {
        // Clear immediate draws (they were rendered last frame)
        self.immediate.clear();

        // Update persistent draws
        self.persistent.retain_mut(|draw| {
            draw.remaining -= delta;
            draw.remaining > 0.0
        });
    }

    /// Get all draws to render this frame
    pub fn get_draws(&self) -> impl Iterator<Item = &DebugDrawCommand> {
        self.immediate
            .iter()
            .chain(self.persistent.iter().map(|d| &d.command))
    }

    /// Clear all draws (on play mode stop)
    pub fn clear(&mut self) {
        self.immediate.clear();
        self.persistent.clear();
    }
}

/// Helper to convert [f32; 4] color array to Bevy Color
pub fn array_to_color(arr: [f32; 4]) -> Color {
    Color::srgba(arr[0], arr[1], arr[2], arr[3])
}
