//! A gamepad character controller with a built-in third-person camera. One
//! script drives an animated character and its chase camera, with a
//! locomotion and action state machine layered on top.
//!
//! - Left stick / WASD: move, relative to where the camera faces.
//! - Right stick / mouse: orbit the camera around the character.
//! - South (A, Cross) / Space: jump. Gravity brings the character back down;
//!   falling plays the fall clip and a heavy landing the hard-landing clip.
//! - West (X, Square) / J: punch. North (Y, Triangle) / K: kick.
//! - East (B, Circle) / C: toggle crouch. L1 / Left Ctrl (hold): crouch.
//! - R2 / Left Shift (hold): sprint.
//! - L2 / right mouse (hold): strafe lock. The character keeps facing away
//!   from the camera, so the stick picks forward run, backward walk or a side
//!   strafe instead of turning to face travel.
//!
//! Grounded, the character turns straight to face travel; a skid clip plays
//! when stopping while West is held.
//!
//! The controller is physics-driven: it integrates its own gravity into a
//! vertical velocity and moves every frame through the engine's
//! collide-and-slide, reading `grounded` back from `PhysicsReadState`. That
//! is what lets it stand on and slide along real colliders, and step off any
//! ledge, rather than walking on a flat plane.
//!
//! Setup: attach to the character entity (the one with the Animator). It
//! needs a Physics Body (Kinematic) and a Collision Shape (a capsule sized to
//! the model), or it will not collide and will simply fall. Anything to stand
//! on needs a Physics Body (Static) and a Collision Shape. The camera is the
//! entity named "camera" (`CAMERA_NAME`).

use bevy::prelude::*;
use renzora::core::reflection::get_reflected_field;
use renzora::script_bridge::{PropertyValue, ScriptAction};
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

// Movement.
const MOVE_SPEED: f32 = 3.0;
const SPRINT_SPEED: f32 = 6.0;
const CROUCH_SPEED: f32 = 1.4;
/// Degrees per second the character turns.
const TURN_SPEED: f32 = 540.0;
/// Stick magnitude below this is ignored.
const DEADZONE: f32 = 0.2;
/// Added to the facing yaw when the model faces the wrong way (try 180).
const FACING_OFFSET: f32 = 0.0;

// Vertical motion.
/// Downward acceleration, units per second squared.
const GRAVITY: f32 = 18.0;
/// Initial upward velocity of a jump.
const JUMP_SPEED: f32 = 6.0;
/// Descending faster than this plays the fall clip.
const FALL_SPEED: f32 = 1.5;
/// Landing faster than this plays the hard-landing clip.
const LAND_SPEED: f32 = 6.0;

// Animation clips.
const IDLE_ANIM: &str = "Idle";
const RUN_ANIM: &str = "Standard_Run";
const WALK_BACK_ANIM: &str = "Walking_Backwards";
const STRAFE_L_ANIM: &str = "Walk_Strafe_Left";
const STRAFE_R_ANIM: &str = "Right_Shimmy";
/// Skid clip when stopping with West held; empty disables it.
const STOP_ANIM: &str = "Push_Stop";
const CRAWL_ANIM: &str = "Crawling";
const CROUCH_ANIM: &str = "Crawling";
/// Rising clip; "Jump" is the alternative in this set.
const JUMP_ANIM: &str = "Jump__1_";
/// Falling clip; "Jumping_Down__1_" is the alternative in this set.
const FALL_ANIM: &str = "Jumping_Down";
const LAND_ANIM: &str = "Hard_Landing";
const PUNCH_ANIM: &str = "Punching";
const KICK_ANIM: &str = "Mma_Kick";
/// Seconds the punch lasts; the clip is time-stretched to fit.
const PUNCH_TIME: f32 = 1.1;
/// Seconds the kick lasts; the clip is time-stretched to fit.
const KICK_TIME: f32 = 1.3;
/// Crossfade time between clips, in seconds.
const BLEND: f32 = 0.15;

// Camera.
const CAMERA_NAME: &str = "camera";
const CAM_DISTANCE: f32 = 4.5;
/// Look-at height above the character's origin.
const CAM_HEIGHT: f32 = 1.2;
/// Right-stick magnitude below this is ignored, which stops orbit drift.
const CAM_DEADZONE: f32 = 0.15;
/// Right-stick orbit speed, degrees per second.
const ORBIT_SPEED: f32 = 150.0;
/// Mouse orbit sensitivity, the fallback when the stick is at rest.
const MOUSE_ORBIT: f32 = 0.2;
/// Camera position smoothing; higher is snappier.
const FOLLOW_SMOOTH: f32 = 10.0;
const PITCH_MIN: f32 = -10.0;
const PITCH_MAX: f32 = 80.0;
const INVERT_Y: bool = false;

#[derive(Component, Clone)]
struct Character {
    /// Character facing yaw, degrees.
    yaw: f32,
    /// Camera orbit yaw and pitch, degrees.
    cam_yaw: f32,
    cam_pitch: f32,
    /// Smoothed camera position; `None` until the first frame places it.
    cam_pos: Option<Vec3>,
    /// The clip last crossfaded to, so a clip is only started on a change.
    anim: String,
    /// Vertical velocity.
    vy: f32,
    /// Grounded last frame, for landing detection.
    was_grounded: bool,
    /// The full-body action clip playing, empty when none.
    lock_anim: String,
    lock_timer: f32,
    stop_timer: f32,
    was_moving: bool,
    crouch_toggle: bool,
    /// Buttons held last frame, so a hold fires once.
    prev_jump: bool,
    prev_punch: bool,
    prev_kick: bool,
    prev_crouch: bool,
}

/// This frame's input, gathered up front so no resource borrow is held.
#[derive(Default)]
struct Pad {
    right_stick: Vec2,
    left_stick: Vec2,
    mouse_delta: Vec2,
    left_trigger: f32,
    right_trigger: f32,
    escape: bool,
    keys_x: f32,
    keys_y: f32,
    jump: bool,
    punch: bool,
    kick: bool,
    crouch_button: bool,
    crouch_hold: bool,
    sprint_button: bool,
}

fn read_input(ctx: &mut ScriptCtx) -> Pad {
    let Some(input) = ctx.get_resource::<ScriptInput>() else { return Pad::default() };
    let key = |k| input.is_key_pressed(k);
    let button = |b| input.first_gamepad().is_some_and(|pad| input.is_gamepad_button_pressed(pad, b));
    let pad = input.first_gamepad();
    let mut keys_x = 0.0;
    let mut keys_y = 0.0;
    if key(KeyCode::KeyA) {
        keys_x -= 1.0;
    }
    if key(KeyCode::KeyD) {
        keys_x += 1.0;
    }
    if key(KeyCode::KeyW) {
        keys_y += 1.0;
    }
    if key(KeyCode::KeyS) {
        keys_y -= 1.0;
    }
    Pad {
        right_stick: pad.map_or(Vec2::ZERO, |p| input.get_gamepad_right_stick(p)),
        left_stick: pad.map_or(Vec2::ZERO, |p| input.get_gamepad_left_stick(p)),
        mouse_delta: input.mouse_delta,
        left_trigger: pad.map_or(0.0, |p| input.get_gamepad_trigger(p, true)),
        right_trigger: pad.map_or(0.0, |p| input.get_gamepad_trigger(p, false)),
        escape: input.is_key_just_pressed(KeyCode::Escape),
        keys_x,
        keys_y,
        jump: button(GamepadButton::South) || key(KeyCode::Space),
        punch: button(GamepadButton::West) || key(KeyCode::KeyJ),
        kick: button(GamepadButton::North) || key(KeyCode::KeyK),
        crouch_button: button(GamepadButton::East) || key(KeyCode::KeyC),
        crouch_hold: button(GamepadButton::LeftTrigger) || key(KeyCode::ControlLeft),
        sprint_button: button(GamepadButton::RightTrigger2) || key(KeyCode::ShiftLeft),
    }
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let me = ctx.entity();
    let Some(mut s) = ctx.get::<Character>().map(|c| c.clone()) else { return };
    let input = read_input(ctx);
    let (mouse_click, mouse_right) = ctx
        .get_resource::<ButtonInput<MouseButton>>()
        .map_or((false, false), |b| {
            (b.just_pressed(MouseButton::Left), b.pressed(MouseButton::Right))
        });

    // Camera orbit. Below the deadzone a resting, slightly drifting stick
    // would keep orbiting slowly, so it falls back to mouse look instead.
    let orbit = if input.right_stick.length() < CAM_DEADZONE {
        Vec2::new(input.mouse_delta.x, -input.mouse_delta.y) * MOUSE_ORBIT * 0.5
    } else {
        input.right_stick * ORBIT_SPEED * dt
    };
    s.cam_yaw = wrap180(s.cam_yaw - orbit.x);
    let dy = if INVERT_Y { -orbit.y } else { orbit.y };
    s.cam_pitch = (s.cam_pitch - dy).clamp(PITCH_MIN, PITCH_MAX);

    if input.escape {
        ScriptAction::new("unlock_cursor", me).send(ctx.world());
    }
    if mouse_click {
        ScriptAction::new("lock_cursor", me).send(ctx.world());
    }

    // Movement input: the stick plus WASD, clamped to unit length.
    let mut stick = input.left_stick + Vec2::new(input.keys_x, input.keys_y);
    if stick.length() > 1.0 {
        stick = stick.normalize();
    }
    let (mx, my) = (stick.x, stick.y);

    // Camera-relative ground directions from the orbit yaw.
    let ry = s.cam_yaw.to_radians();
    let fwd = Vec2::new(-ry.sin(), -ry.cos());
    let right = Vec2::new(ry.cos(), -ry.sin());
    let dir = fwd * my + right * mx;
    let moving = dir.length() > DEADZONE;

    let aim_held = input.left_trigger > 0.5 || mouse_right;
    let jump_edge = input.jump && !s.prev_jump;
    let punch_edge = input.punch && !s.prev_punch;
    let kick_edge = input.kick && !s.prev_kick;
    let crouch_edge = input.crouch_button && !s.prev_crouch;
    s.prev_jump = input.jump;
    s.prev_punch = input.punch;
    s.prev_kick = input.kick;
    s.prev_crouch = input.crouch_button;

    if crouch_edge {
        s.crouch_toggle = !s.crouch_toggle;
    }
    let crouch_held = s.crouch_toggle || input.crouch_hold;
    let sprint_held = input.right_trigger > 0.5 || input.sprint_button;

    if s.lock_timer > 0.0 {
        s.lock_timer -= dt;
    }
    if s.stop_timer > 0.0 {
        s.stop_timer -= dt;
    }
    let mut locked = s.lock_timer > 0.0;
    // An action just finished: restore normal playback speed (the action may
    // have stretched it) before any locomotion clip resumes.
    if !locked && !s.lock_anim.is_empty() {
        set_animation_speed(ctx, 1.0);
        s.lock_anim.clear();
    }

    // The result of last frame's collide-and-slide.
    let mut grounded = matches!(
        get_reflected_field(ctx.world(), me, "PhysicsReadState", "grounded"),
        Some(PropertyValue::Bool(true))
    );

    if grounded && !s.was_grounded && s.vy < -LAND_SPEED && !locked {
        start_action(ctx, &mut s, LAND_ANIM, None);
        locked = true;
    }

    if !locked && grounded {
        if jump_edge {
            s.vy = JUMP_SPEED;
            grounded = false;
        } else if punch_edge {
            start_action(ctx, &mut s, PUNCH_ANIM, Some(PUNCH_TIME));
            locked = true;
        } else if kick_edge {
            start_action(ctx, &mut s, KICK_ANIM, Some(KICK_TIME));
            locked = true;
        }
    }

    // A small downward bias on the ground keeps the slide glued to the
    // surface and walking down small steps; in the air, accelerate down.
    if grounded && s.vy <= 0.0 {
        s.vy = -2.0;
    } else {
        s.vy -= GRAVITY * dt;
    }

    let mut step = Vec2::ZERO;
    if !locked {
        if moving {
            let speed = if crouch_held && grounded {
                CROUCH_SPEED
            } else if sprint_held {
                SPRINT_SPEED
            } else {
                MOVE_SPEED
            };
            step = dir * speed * dt;
            // Strafe lock faces away from the camera; otherwise face travel.
            let target = if aim_held && grounded {
                wrap180(fwd.x.atan2(fwd.y).to_degrees() + FACING_OFFSET)
            } else {
                dir.x.atan2(dir.y).to_degrees() + FACING_OFFSET
            };
            let turn = wrap180(target - s.yaw).clamp(-TURN_SPEED * dt, TURN_SPEED * dt);
            s.yaw = wrap180(s.yaw + turn);
        }
        let yaw = s.yaw;
        if let Some(mut transform) = ctx.get_mut::<Transform>() {
            transform.rotation = Quat::from_rotation_y(yaw.to_radians());
        }
    }

    // Slide the whole move through physics so the character stops on and
    // against colliders instead of clipping through; this also refreshes
    // `grounded` for next frame.
    ScriptAction::new("kinematic_slide", me)
        .arg("x", step.x)
        .arg("y", s.vy * dt)
        .arg("z", step.y)
        .send(ctx.world());

    // Pick the clip, highest priority first. A locked action is already
    // playing and is left alone.
    if !locked {
        if !grounded {
            let clip = if s.vy > FALL_SPEED { JUMP_ANIM } else { FALL_ANIM };
            set_anim(ctx, &mut s, clip, false);
        } else if crouch_held {
            let clip = if moving { CRAWL_ANIM } else { CROUCH_ANIM };
            set_anim(ctx, &mut s, clip, true);
        } else if aim_held {
            let clip = if !moving {
                IDLE_ANIM
            } else if my.abs() >= mx.abs() {
                if my >= 0.0 { RUN_ANIM } else { WALK_BACK_ANIM }
            } else if mx < 0.0 {
                STRAFE_L_ANIM
            } else {
                STRAFE_R_ANIM
            };
            set_anim(ctx, &mut s, clip, true);
        } else if moving {
            set_anim(ctx, &mut s, RUN_ANIM, true);
        } else {
            // A skid only while West is held; a plain stick release goes
            // straight to idle.
            let skid = !STOP_ANIM.is_empty() && input.punch;
            if skid && s.was_moving && s.stop_timer <= 0.0 {
                let len = clip_length(ctx, STOP_ANIM);
                s.stop_timer = if len > 0.0 { len.min(0.5) } else { 0.4 };
            }
            if skid && s.stop_timer > 0.0 {
                set_anim(ctx, &mut s, STOP_ANIM, false);
            } else {
                set_anim(ctx, &mut s, IDLE_ANIM, true);
            }
        }
    }

    s.was_moving = moving && grounded;
    s.was_grounded = grounded;

    // Chase camera.
    let Some(origin) = ctx.get::<Transform>().map(|t| t.translation) else { return };
    let focus = origin + Vec3::Y * CAM_HEIGHT;
    let rp = s.cam_pitch.to_radians();
    let desired = focus
        + Vec3::new(ry.sin() * rp.cos(), rp.sin(), ry.cos() * rp.cos()) * CAM_DISTANCE;
    let cam_pos = match s.cam_pos {
        None => desired,
        Some(pos) => pos.lerp(desired, 1.0 - (-FOLLOW_SMOOTH * dt).exp()),
    };
    s.cam_pos = Some(cam_pos);

    if let Some(mut state) = ctx.get_mut::<Character>() {
        *state = s;
    }

    let Some(camera) = find_named(ctx.world(), CAMERA_NAME) else { return };
    if let Some(mut transform) = ctx.get_mut_on::<Transform>(camera) {
        transform.translation = cam_pos;
        transform.rotation = Quat::from_rotation_y(ry) * Quat::from_rotation_x(-rp);
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        let (yaw, _, _) = ctx
            .get::<Transform>()
            .map_or((0.0, 0.0, 0.0), |t| t.rotation.to_euler(EulerRot::YXZ));
        ctx.insert(Character {
            yaw: yaw.to_degrees(),
            cam_yaw: 0.0,
            cam_pitch: 20.0,
            cam_pos: None,
            anim: String::new(),
            vy: 0.0,
            was_grounded: true,
            lock_anim: String::new(),
            lock_timer: 0.0,
            stop_timer: 0.0,
            was_moving: false,
            crouch_toggle: false,
            prev_jump: false,
            prev_punch: false,
            prev_kick: false,
            prev_crouch: false,
        });
        let me = ctx.entity();
        ScriptAction::new("lock_cursor", me).send(ctx.world());
    }
}

/// Wrap an angle in degrees into [-180, 180).
fn wrap180(a: f32) -> f32 {
    (a + 180.0).rem_euclid(360.0) - 180.0
}

/// Crossfade into a clip only when it actually changes.
fn set_anim(ctx: &mut ScriptCtx, s: &mut Character, name: &str, looping: bool) {
    if s.anim != name {
        crossfade(ctx, name, looping);
        s.anim = name.to_string();
    }
}

/// Start a one-shot, full-body action that blocks movement until it ends.
/// With `hold`, the clip is time-stretched through playback speed to last
/// exactly that long, which makes short punch and kick clips read longer.
fn start_action(ctx: &mut ScriptCtx, s: &mut Character, name: &str, hold: Option<f32>) {
    crossfade(ctx, name, false);
    s.anim = name.to_string();
    s.lock_anim = name.to_string();
    let mut len = clip_length(ctx, name);
    if len <= 0.0 {
        // The length is unknown until the clip has loaded.
        len = 0.6;
    }
    match hold {
        Some(hold) if hold > 0.0 => {
            set_animation_speed(ctx, len / hold);
            s.lock_timer = hold;
        }
        _ => s.lock_timer = len,
    }
}

fn crossfade(ctx: &mut ScriptCtx, name: &str, looping: bool) {
    let me = ctx.entity();
    ScriptAction::new("crossfade_animation", me)
        .arg("name", name)
        .arg("duration", BLEND)
        .arg("looping", looping)
        .send(ctx.world());
}

fn set_animation_speed(ctx: &mut ScriptCtx, speed: f32) {
    let me = ctx.entity();
    ScriptAction::new("set_animation_speed", me).arg("speed", speed).send(ctx.world());
}

/// A clip's length in seconds from the animator's read mirror; 0 if unknown.
fn clip_length(ctx: &mut ScriptCtx, name: &str) -> f32 {
    let me = ctx.entity();
    // `clip_lengths` is a map, so the key needs reflection's `["key"]` form.
    let path = format!("clip_lengths[\"{name}\"]");
    match get_reflected_field(ctx.world(), me, "AnimatorReadState", &path) {
        Some(PropertyValue::Float(len)) => len,
        _ => 0.0,
    }
}

/// The entity with this name (its snake_case id), if there is one.
fn find_named(world: &mut World, name: &str) -> Option<Entity> {
    let mut names = world.query::<(Entity, &Name)>();
    names.iter(world).find(|(_, n)| n.as_str() == name).map(|(e, _)| e)
}

renzora::script!(update, hooks = hooks);
