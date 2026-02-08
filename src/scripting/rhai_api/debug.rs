//! Debug API functions for Rhai scripts

use rhai::Engine;
use rhai::ImmutableString;
use super::super::rhai_commands::RhaiCommand;
use crate::core::resources::console::{console_log, LogLevel};

/// Register debug functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Logging
    // ===================

    // log(message) - Log info message (executes immediately)
    engine.register_fn("log", |message: ImmutableString| {
        console_log(LogLevel::Info, "Script", message.to_string());
    });

    // log_info(message)
    engine.register_fn("log_info", |message: ImmutableString| {
        console_log(LogLevel::Info, "Script", message.to_string());
    });

    // log_warn(message)
    engine.register_fn("log_warn", |message: ImmutableString| {
        console_log(LogLevel::Warning, "Script", message.to_string());
    });

    // log_error(message)
    engine.register_fn("log_error", |message: ImmutableString| {
        console_log(LogLevel::Error, "Script", message.to_string());
    });

    // log_debug(message)
    engine.register_fn("log_debug", |message: ImmutableString| {
        console_log(LogLevel::Info, "Script", message.to_string());
    });

    // ===================
    // Debug Drawing
    // ===================

    // draw_line(start_x, start_y, start_z, end_x, end_y, end_z)
    engine.register_fn("draw_line", |sx: f64, sy: f64, sz: f64, ex: f64, ey: f64, ez: f64| {
        super::push_command(RhaiCommand::DrawLine {
            start: bevy::prelude::Vec3::new(sx as f32, sy as f32, sz as f32),
            end: bevy::prelude::Vec3::new(ex as f32, ey as f32, ez as f32),
            color: [1.0, 1.0, 1.0, 1.0], duration: 0.0,
        });
    });

    // draw_line_color(sx, sy, sz, ex, ey, ez, r, g, b)
    engine.register_fn("draw_line_color", |sx: f64, sy: f64, sz: f64, ex: f64, ey: f64, ez: f64, r: f64, g: f64, b: f64| {
        super::push_command(RhaiCommand::DrawLine {
            start: bevy::prelude::Vec3::new(sx as f32, sy as f32, sz as f32),
            end: bevy::prelude::Vec3::new(ex as f32, ey as f32, ez as f32),
            color: [r as f32, g as f32, b as f32, 1.0], duration: 0.0,
        });
    });

    // draw_line_duration(sx, sy, sz, ex, ey, ez, r, g, b, duration)
    engine.register_fn("draw_line_duration", |sx: f64, sy: f64, sz: f64, ex: f64, ey: f64, ez: f64, r: f64, g: f64, b: f64, duration: f64| {
        super::push_command(RhaiCommand::DrawLine {
            start: bevy::prelude::Vec3::new(sx as f32, sy as f32, sz as f32),
            end: bevy::prelude::Vec3::new(ex as f32, ey as f32, ez as f32),
            color: [r as f32, g as f32, b as f32, 1.0], duration: duration as f32,
        });
    });

    // draw_ray(origin_x, origin_y, origin_z, dir_x, dir_y, dir_z, length)
    engine.register_fn("draw_ray", |ox: f64, oy: f64, oz: f64, dx: f64, dy: f64, dz: f64, length: f64| {
        super::push_command(RhaiCommand::DrawRay {
            origin: bevy::prelude::Vec3::new(ox as f32, oy as f32, oz as f32),
            direction: bevy::prelude::Vec3::new(dx as f32, dy as f32, dz as f32),
            length: length as f32, color: [1.0, 0.0, 0.0, 1.0], duration: 0.0,
        });
    });

    // draw_sphere(center_x, center_y, center_z, radius)
    engine.register_fn("draw_sphere", |x: f64, y: f64, z: f64, radius: f64| {
        super::push_command(RhaiCommand::DrawSphere {
            center: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32),
            radius: radius as f32, color: [0.0, 1.0, 0.0, 0.5], duration: 0.0,
        });
    });

    // draw_sphere_color(x, y, z, radius, r, g, b)
    engine.register_fn("draw_sphere_color", |x: f64, y: f64, z: f64, radius: f64, r: f64, g: f64, b: f64| {
        super::push_command(RhaiCommand::DrawSphere {
            center: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32),
            radius: radius as f32, color: [r as f32, g as f32, b as f32, 0.5], duration: 0.0,
        });
    });

    // draw_box(center_x, center_y, center_z, half_x, half_y, half_z)
    engine.register_fn("draw_box", |x: f64, y: f64, z: f64, hx: f64, hy: f64, hz: f64| {
        super::push_command(RhaiCommand::DrawBox {
            center: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32),
            half_extents: bevy::prelude::Vec3::new(hx as f32, hy as f32, hz as f32),
            color: [0.0, 0.5, 1.0, 0.5], duration: 0.0,
        });
    });

    // draw_box_color(x, y, z, hx, hy, hz, r, g, b)
    engine.register_fn("draw_box_color", |x: f64, y: f64, z: f64, hx: f64, hy: f64, hz: f64, r: f64, g: f64, b: f64| {
        super::push_command(RhaiCommand::DrawBox {
            center: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32),
            half_extents: bevy::prelude::Vec3::new(hx as f32, hy as f32, hz as f32),
            color: [r as f32, g as f32, b as f32, 0.5], duration: 0.0,
        });
    });

    // draw_point(x, y, z, size)
    engine.register_fn("draw_point", |x: f64, y: f64, z: f64, size: f64| {
        super::push_command(RhaiCommand::DrawPoint {
            position: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32),
            size: size as f32, color: [1.0, 1.0, 0.0, 1.0], duration: 0.0,
        });
    });

    // ===================
    // Assert/Check
    // ===================

    // assert(condition, message) - Log error if condition is false (executes immediately)
    engine.register_fn("assert", |condition: bool, message: ImmutableString| {
        if !condition {
            console_log(LogLevel::Error, "Script", format!("ASSERT FAILED: {}", message));
        }
    });
}
