//! Built-in example scripts

use bevy::prelude::*;

use super::{
    GameScript, ScriptContext, ScriptValue, ScriptVariableDefinition, ScriptVariables,
};

/// Script that rotates an object continuously
pub struct RotateScript;

impl GameScript for RotateScript {
    fn id(&self) -> &'static str {
        "builtin.rotate"
    }

    fn name(&self) -> &'static str {
        "Rotate"
    }

    fn description(&self) -> &'static str {
        "Continuously rotates the object around an axis"
    }

    fn category(&self) -> &'static str {
        "Movement"
    }

    fn variables(&self) -> Vec<ScriptVariableDefinition> {
        vec![
            ScriptVariableDefinition::new("speed", ScriptValue::Float(45.0))
                .with_display_name("Speed (deg/s)")
                .with_hint("Rotation speed in degrees per second"),
            ScriptVariableDefinition::new("axis", ScriptValue::Vec3(Vec3::Y))
                .with_display_name("Axis")
                .with_hint("Rotation axis (normalized)"),
        ]
    }

    fn on_update(&self, ctx: &mut ScriptContext, vars: &ScriptVariables) {
        let speed = vars.get_float("speed").unwrap_or(45.0);
        let axis = vars.get_vec3("axis").unwrap_or(Vec3::Y).normalize_or_zero();

        if axis.length_squared() > 0.0 {
            let angle = speed * ctx.time.delta;
            let rotation = Quat::from_axis_angle(axis, angle.to_radians());
            ctx.transform.rotation = rotation * ctx.transform.rotation;
            ctx.set_rotation(ctx.transform.rotation);
        }
    }
}

/// Script for basic WASD movement
pub struct SimpleMovementScript;

impl GameScript for SimpleMovementScript {
    fn id(&self) -> &'static str {
        "builtin.simple_movement"
    }

    fn name(&self) -> &'static str {
        "Simple Movement"
    }

    fn description(&self) -> &'static str {
        "Basic WASD movement in the XZ plane"
    }

    fn category(&self) -> &'static str {
        "Movement"
    }

    fn variables(&self) -> Vec<ScriptVariableDefinition> {
        vec![
            ScriptVariableDefinition::new("speed", ScriptValue::Float(5.0))
                .with_display_name("Speed")
                .with_hint("Movement speed in units per second"),
            ScriptVariableDefinition::new("use_local_space", ScriptValue::Bool(false))
                .with_display_name("Local Space")
                .with_hint("Move relative to object rotation"),
        ]
    }

    fn on_update(&self, ctx: &mut ScriptContext, vars: &ScriptVariables) {
        let speed = vars.get_float("speed").unwrap_or(5.0);
        let local_space = vars.get_bool("use_local_space").unwrap_or(false);

        let input = ctx.input.get_movement_vector();
        if input.length_squared() > 0.0 {
            let movement = Vec3::new(input.x, 0.0, -input.y) * speed * ctx.time.delta;

            if local_space {
                ctx.translate_local(movement);
            } else {
                ctx.translate(movement);
            }
        }
    }
}

/// Script for first-person camera look
pub struct MouseLookScript;

impl GameScript for MouseLookScript {
    fn id(&self) -> &'static str {
        "builtin.mouse_look"
    }

    fn name(&self) -> &'static str {
        "Mouse Look"
    }

    fn description(&self) -> &'static str {
        "First-person style mouse look rotation"
    }

    fn category(&self) -> &'static str {
        "Camera"
    }

    fn variables(&self) -> Vec<ScriptVariableDefinition> {
        vec![
            ScriptVariableDefinition::new("sensitivity", ScriptValue::Float(0.2))
                .with_display_name("Sensitivity")
                .with_hint("Mouse sensitivity"),
            ScriptVariableDefinition::new("invert_y", ScriptValue::Bool(false))
                .with_display_name("Invert Y")
                .with_hint("Invert vertical look"),
            ScriptVariableDefinition::new("clamp_pitch", ScriptValue::Bool(true))
                .with_display_name("Clamp Pitch")
                .with_hint("Limit vertical look angle"),
        ]
    }

    fn on_update(&self, ctx: &mut ScriptContext, vars: &ScriptVariables) {
        // Only look when right mouse is held
        if !ctx.input.is_mouse_pressed(MouseButton::Right) {
            return;
        }

        let sensitivity = vars.get_float("sensitivity").unwrap_or(0.2);
        let invert_y = vars.get_bool("invert_y").unwrap_or(false);
        let clamp_pitch = vars.get_bool("clamp_pitch").unwrap_or(true);

        let delta = ctx.input.mouse_delta * sensitivity;
        if delta.length_squared() == 0.0 {
            return;
        }

        let yaw = -delta.x.to_radians();
        let pitch_mult = if invert_y { 1.0 } else { -1.0 };
        let pitch = delta.y.to_radians() * pitch_mult;

        // Get current euler angles
        let (current_pitch, current_yaw, _) = ctx.transform.rotation.to_euler(EulerRot::XYZ);

        // Apply new rotation
        let mut new_pitch = current_pitch + pitch;
        let new_yaw = current_yaw + yaw;

        // Clamp pitch if enabled
        if clamp_pitch {
            let limit = 89.0_f32.to_radians();
            new_pitch = new_pitch.clamp(-limit, limit);
        }

        let rotation = Quat::from_euler(EulerRot::XYZ, new_pitch, new_yaw, 0.0);
        ctx.set_rotation(rotation);
    }
}

/// Script that makes object follow another
pub struct FollowScript;

impl GameScript for FollowScript {
    fn id(&self) -> &'static str {
        "builtin.follow"
    }

    fn name(&self) -> &'static str {
        "Follow Target"
    }

    fn description(&self) -> &'static str {
        "Smoothly follow a target position"
    }

    fn category(&self) -> &'static str {
        "Movement"
    }

    fn variables(&self) -> Vec<ScriptVariableDefinition> {
        vec![
            ScriptVariableDefinition::new("target", ScriptValue::Vec3(Vec3::ZERO))
                .with_display_name("Target Position")
                .with_hint("Position to follow"),
            ScriptVariableDefinition::new("smoothing", ScriptValue::Float(5.0))
                .with_display_name("Smoothing")
                .with_hint("How smoothly to follow (higher = faster)"),
            ScriptVariableDefinition::new("offset", ScriptValue::Vec3(Vec3::ZERO))
                .with_display_name("Offset")
                .with_hint("Offset from target position"),
        ]
    }

    fn on_update(&self, ctx: &mut ScriptContext, vars: &ScriptVariables) {
        let target = vars.get_vec3("target").unwrap_or(Vec3::ZERO);
        let smoothing = vars.get_float("smoothing").unwrap_or(5.0);
        let offset = vars.get_vec3("offset").unwrap_or(Vec3::ZERO);

        let desired = target + offset;
        let new_pos = ctx.transform.position.lerp(desired, smoothing * ctx.time.delta);
        ctx.set_position(new_pos);
    }
}

/// Script that oscillates position (bob up and down)
pub struct BobScript;

impl GameScript for BobScript {
    fn id(&self) -> &'static str {
        "builtin.bob"
    }

    fn name(&self) -> &'static str {
        "Bob"
    }

    fn description(&self) -> &'static str {
        "Oscillate position up and down"
    }

    fn category(&self) -> &'static str {
        "Movement"
    }

    fn variables(&self) -> Vec<ScriptVariableDefinition> {
        vec![
            ScriptVariableDefinition::new("amplitude", ScriptValue::Float(0.5))
                .with_display_name("Amplitude")
                .with_hint("Height of oscillation"),
            ScriptVariableDefinition::new("speed", ScriptValue::Float(2.0))
                .with_display_name("Speed")
                .with_hint("Oscillation speed"),
            ScriptVariableDefinition::new("axis", ScriptValue::Vec3(Vec3::Y))
                .with_display_name("Axis")
                .with_hint("Oscillation axis"),
        ]
    }

    fn on_update(&self, ctx: &mut ScriptContext, vars: &ScriptVariables) {
        let amplitude = vars.get_float("amplitude").unwrap_or(0.5);
        let speed = vars.get_float("speed").unwrap_or(2.0);
        let axis = vars.get_vec3("axis").unwrap_or(Vec3::Y).normalize_or_zero();

        let offset = (ctx.time.elapsed as f32 * speed).sin() * amplitude;
        let base_pos = ctx.transform.position - axis * ((ctx.time.elapsed as f32 - ctx.time.delta) * speed).sin() * amplitude;
        ctx.set_position(base_pos + axis * offset);
    }
}

/// Script that prints debug info
pub struct DebugScript;

impl GameScript for DebugScript {
    fn id(&self) -> &'static str {
        "builtin.debug"
    }

    fn name(&self) -> &'static str {
        "Debug Info"
    }

    fn description(&self) -> &'static str {
        "Prints debug information to the console"
    }

    fn category(&self) -> &'static str {
        "Debug"
    }

    fn variables(&self) -> Vec<ScriptVariableDefinition> {
        vec![
            ScriptVariableDefinition::new("print_position", ScriptValue::Bool(true))
                .with_display_name("Print Position"),
            ScriptVariableDefinition::new("print_rotation", ScriptValue::Bool(false))
                .with_display_name("Print Rotation"),
            ScriptVariableDefinition::new("print_input", ScriptValue::Bool(false))
                .with_display_name("Print Input"),
            ScriptVariableDefinition::new("interval", ScriptValue::Float(1.0))
                .with_display_name("Interval (s)")
                .with_hint("How often to print (0 = every frame)"),
        ]
    }

    fn on_update(&self, ctx: &mut ScriptContext, vars: &ScriptVariables) {
        let interval = vars.get_float("interval").unwrap_or(1.0);

        // Check if we should print this frame
        if interval > 0.0 {
            let current_sec = (ctx.time.elapsed / interval as f64).floor();
            let prev_sec = ((ctx.time.elapsed - ctx.time.delta as f64) / interval as f64).floor();
            if current_sec == prev_sec {
                return;
            }
        }

        if vars.get_bool("print_position").unwrap_or(true) {
            ctx.print(&format!("Position: {:?}", ctx.transform.position));
        }

        if vars.get_bool("print_rotation").unwrap_or(false) {
            ctx.print(&format!("Rotation: {:?}", ctx.transform.euler_angles_degrees()));
        }

        if vars.get_bool("print_input").unwrap_or(false) {
            ctx.print(&format!(
                "Input - Move: {:?}, Mouse: {:?}",
                ctx.input.get_movement_vector(),
                ctx.input.mouse_position
            ));
        }
    }
}

/// Register all built-in scripts
pub fn register_builtin_scripts(registry: &mut super::ScriptRegistry) {
    registry.register(RotateScript);
    registry.register(SimpleMovementScript);
    registry.register(MouseLookScript);
    registry.register(FollowScript);
    registry.register(BobScript);
    registry.register(DebugScript);
}
