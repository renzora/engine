//! Runtime scripting resources

use bevy::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};

// =============================================================================
// INPUT
// =============================================================================

/// Input state available to scripts
#[derive(Resource, Default)]
pub struct ScriptInput {
    pub keys_pressed: Vec<KeyCode>,
    pub keys_just_pressed: Vec<KeyCode>,
    pub keys_just_released: Vec<KeyCode>,
    pub mouse_buttons_pressed: Vec<MouseButton>,
    pub mouse_buttons_just_pressed: Vec<MouseButton>,
    pub mouse_position: Vec2,
    pub mouse_delta: Vec2,
    pub mouse_scroll: f32,
    pub gamepad_axes: [[f32; 6]; 4],
    pub gamepad_buttons_pressed: [[bool; 16]; 4],
    pub gamepad_buttons_just_pressed: [[bool; 16]; 4],
}

impl ScriptInput {
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.keys_pressed.contains(&key)
    }

    pub fn is_key_just_pressed(&self, key: KeyCode) -> bool {
        self.keys_just_pressed.contains(&key)
    }

    pub fn get_movement_vector(&self) -> Vec2 {
        let mut movement = Vec2::ZERO;
        if self.is_key_pressed(KeyCode::KeyW) || self.is_key_pressed(KeyCode::ArrowUp) {
            movement.y += 1.0;
        }
        if self.is_key_pressed(KeyCode::KeyS) || self.is_key_pressed(KeyCode::ArrowDown) {
            movement.y -= 1.0;
        }
        if self.is_key_pressed(KeyCode::KeyA) || self.is_key_pressed(KeyCode::ArrowLeft) {
            movement.x -= 1.0;
        }
        if self.is_key_pressed(KeyCode::KeyD) || self.is_key_pressed(KeyCode::ArrowRight) {
            movement.x += 1.0;
        }
        if movement != Vec2::ZERO {
            movement = movement.normalize();
        }
        movement
    }

    pub fn get_gamepad_left_stick_x(&self, gamepad: usize) -> f32 {
        if gamepad < 4 { self.gamepad_axes[gamepad][0] } else { 0.0 }
    }

    pub fn get_gamepad_left_stick_y(&self, gamepad: usize) -> f32 {
        if gamepad < 4 { self.gamepad_axes[gamepad][1] } else { 0.0 }
    }

    pub fn get_gamepad_right_stick_x(&self, gamepad: usize) -> f32 {
        if gamepad < 4 { self.gamepad_axes[gamepad][2] } else { 0.0 }
    }

    pub fn get_gamepad_right_stick_y(&self, gamepad: usize) -> f32 {
        if gamepad < 4 { self.gamepad_axes[gamepad][3] } else { 0.0 }
    }
}

// =============================================================================
// PHYSICS
// =============================================================================

#[derive(Clone, Debug)]
pub enum PhysicsCommand {
    ApplyForce { entity: Entity, force: Vec3 },
    ApplyImpulse { entity: Entity, impulse: Vec3 },
    ApplyTorque { entity: Entity, torque: Vec3 },
    SetVelocity { entity: Entity, velocity: Vec3 },
    SetAngularVelocity { entity: Entity, velocity: Vec3 },
    SetGravityScale { entity: Entity, scale: f32 },
    Raycast {
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        requester_entity: Entity,
        result_var: String,
    },
}

#[derive(Resource, Default)]
pub struct PhysicsCommandQueue {
    pub commands: Vec<PhysicsCommand>,
}

impl PhysicsCommandQueue {
    pub fn push(&mut self, cmd: PhysicsCommand) {
        self.commands.push(cmd);
    }

    pub fn drain(&mut self) -> impl Iterator<Item = PhysicsCommand> + '_ {
        self.commands.drain(..)
    }
}

#[derive(Clone, Debug)]
pub struct RaycastHit {
    pub entity: Entity,
    pub point: Vec3,
    pub normal: Vec3,
    pub distance: f32,
}

#[derive(Resource, Default)]
pub struct RaycastResults {
    pub results: HashMap<(Entity, String), RaycastHit>,
}

// =============================================================================
// TIMERS
// =============================================================================

#[derive(Clone, Debug)]
pub struct ScriptTimer {
    pub duration: f32,
    pub elapsed: f32,
    pub repeat: bool,
    pub paused: bool,
    pub just_finished: bool,
}

impl ScriptTimer {
    pub fn new(duration: f32, repeat: bool) -> Self {
        Self {
            duration,
            elapsed: 0.0,
            repeat,
            paused: false,
            just_finished: false,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if self.paused {
            return;
        }

        self.just_finished = false;
        self.elapsed += dt;

        if self.elapsed >= self.duration {
            self.just_finished = true;
            if self.repeat {
                self.elapsed = 0.0;
            }
        }
    }
}

#[derive(Resource, Default)]
pub struct ScriptTimers {
    pub timers: HashMap<String, ScriptTimer>,
}

impl ScriptTimers {
    pub fn start(&mut self, name: String, duration: f32, repeat: bool) {
        self.timers.insert(name, ScriptTimer::new(duration, repeat));
    }

    pub fn stop(&mut self, name: &str) {
        self.timers.remove(name);
    }

    pub fn pause(&mut self, name: &str) {
        if let Some(timer) = self.timers.get_mut(name) {
            timer.paused = true;
        }
    }

    pub fn resume(&mut self, name: &str) {
        if let Some(timer) = self.timers.get_mut(name) {
            timer.paused = false;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        for timer in self.timers.values_mut() {
            timer.tick(dt);
        }
    }

    pub fn get_just_finished(&self) -> Vec<String> {
        self.timers
            .iter()
            .filter(|(_, t)| t.just_finished)
            .map(|(name, _)| name.clone())
            .collect()
    }
}

// =============================================================================
// DEBUG DRAW
// =============================================================================

#[derive(Clone, Debug)]
pub enum DebugDrawCommand {
    Line { start: Vec3, end: Vec3, color: [f32; 4] },
    Sphere { center: Vec3, radius: f32, color: [f32; 4] },
    Box { center: Vec3, half_extents: Vec3, color: [f32; 4] },
    Ray { origin: Vec3, direction: Vec3, color: [f32; 4] },
    Point { position: Vec3, color: [f32; 4] },
}

struct TimedDraw {
    command: DebugDrawCommand,
    remaining: f32,
}

#[derive(Resource, Default)]
pub struct DebugDrawQueue {
    draws: VecDeque<TimedDraw>,
}

impl DebugDrawQueue {
    pub fn push(&mut self, command: DebugDrawCommand, duration: f32) {
        self.draws.push_back(TimedDraw {
            command,
            remaining: duration.max(0.016), // At least one frame
        });
    }

    pub fn tick(&mut self, dt: f32) {
        self.draws.retain_mut(|draw| {
            draw.remaining -= dt;
            draw.remaining > 0.0
        });
    }

    pub fn iter(&self) -> impl Iterator<Item = &DebugDrawCommand> {
        self.draws.iter().map(|d| &d.command)
    }
}

// =============================================================================
// AUDIO
// =============================================================================

#[derive(Clone, Debug)]
pub enum AudioCommand {
    PlaySound { path: String, volume: f32, looping: bool },
    PlaySound3D { path: String, position: Vec3, volume: f32, looping: bool },
    PlayMusic { path: String, volume: f32, fade_in: f32 },
    StopMusic { fade_out: f32 },
    SetMasterVolume { volume: f32 },
    StopAllSounds,
}

#[derive(Resource, Default)]
pub struct AudioCommandQueue {
    pub commands: Vec<AudioCommand>,
}

impl AudioCommandQueue {
    pub fn push(&mut self, cmd: AudioCommand) {
        self.commands.push(cmd);
    }

    pub fn drain(&mut self) -> impl Iterator<Item = AudioCommand> + '_ {
        self.commands.drain(..)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FadeType {
    In,
    Out,
}

#[derive(Clone, Debug)]
pub struct AudioFade {
    pub fade_type: FadeType,
    pub duration: f32,
    pub elapsed: f32,
    pub start_volume: f32,
    pub target_volume: f32,
}

#[derive(Resource, Default)]
pub struct AudioState {
    pub master_volume: f32,
    pub music_volume: f32,
    pub current_music: Option<Entity>,
    pub fade: Option<AudioFade>,
}

impl AudioState {
    pub fn new() -> Self {
        Self {
            master_volume: 1.0,
            music_volume: 1.0,
            current_music: None,
            fade: None,
        }
    }
}

// =============================================================================
// RENDERING
// =============================================================================

#[derive(Clone, Debug)]
pub enum RenderingCommand {
    SetMaterialColor { entity: Entity, color: [f32; 4] },
    SetLightIntensity { entity: Entity, intensity: f32 },
    SetLightColor { entity: Entity, color: [f32; 4] },
    SetVisibility { entity: Entity, visible: bool },
}

#[derive(Resource, Default)]
pub struct RenderingCommandQueue {
    pub commands: Vec<RenderingCommand>,
}

impl RenderingCommandQueue {
    pub fn push(&mut self, cmd: RenderingCommand) {
        self.commands.push(cmd);
    }

    pub fn drain(&mut self) -> impl Iterator<Item = RenderingCommand> + '_ {
        self.commands.drain(..)
    }
}

// =============================================================================
// CAMERA
// =============================================================================

#[derive(Clone, Debug)]
pub enum CameraCommand {
    SetTarget { target: Option<Entity> },
    SetZoom { zoom: f32 },
    ScreenShake { intensity: f32, duration: f32 },
    SetOffset { offset: Vec3 },
}

#[derive(Resource, Default)]
pub struct CameraCommandQueue {
    pub commands: Vec<CameraCommand>,
}

impl CameraCommandQueue {
    pub fn push(&mut self, cmd: CameraCommand) {
        self.commands.push(cmd);
    }

    pub fn drain(&mut self) -> impl Iterator<Item = CameraCommand> + '_ {
        self.commands.drain(..)
    }
}

#[derive(Resource, Default)]
pub struct ScriptCameraState {
    pub follow_target: Option<Entity>,
    pub zoom: f32,
    pub offset: Vec3,
    pub shake_intensity: f32,
    pub shake_duration: f32,
    pub shake_elapsed: f32,
}

// =============================================================================
// COLLISIONS
// =============================================================================

#[derive(Resource, Default)]
pub struct ScriptCollisionEvents {
    pub entered: HashMap<Entity, HashSet<Entity>>,
    pub exited: HashMap<Entity, HashSet<Entity>>,
    pub active: HashMap<Entity, HashSet<Entity>>,
}

impl ScriptCollisionEvents {
    pub fn add_collision_started(&mut self, e1: Entity, e2: Entity) {
        self.entered.entry(e1).or_default().insert(e2);
        self.entered.entry(e2).or_default().insert(e1);
        self.active.entry(e1).or_default().insert(e2);
        self.active.entry(e2).or_default().insert(e1);
    }

    pub fn add_collision_ended(&mut self, e1: Entity, e2: Entity) {
        self.exited.entry(e1).or_default().insert(e2);
        self.exited.entry(e2).or_default().insert(e1);
        if let Some(set) = self.active.get_mut(&e1) {
            set.remove(&e2);
        }
        if let Some(set) = self.active.get_mut(&e2) {
            set.remove(&e1);
        }
    }

    pub fn clear_frame_events(&mut self) {
        self.entered.clear();
        self.exited.clear();
    }

    pub fn get_collisions_entered(&self, entity: Entity) -> Vec<Entity> {
        self.entered.get(&entity).map(|s| s.iter().copied().collect()).unwrap_or_default()
    }

    pub fn get_collisions_exited(&self, entity: Entity) -> Vec<Entity> {
        self.exited.get(&entity).map(|s| s.iter().copied().collect()).unwrap_or_default()
    }

    pub fn get_active_collisions(&self, entity: Entity) -> Vec<Entity> {
        self.active.get(&entity).map(|s| s.iter().copied().collect()).unwrap_or_default()
    }
}

// =============================================================================
// ANIMATION
// =============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum AnimationPlaybackState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

#[derive(Component, Clone, Debug)]
pub struct RuntimeAnimationPlayer {
    pub current_clip: Option<String>,
    pub looping: bool,
    pub speed: f32,
    pub state: AnimationPlaybackState,
    pub current_time: f32,
}

impl Default for RuntimeAnimationPlayer {
    fn default() -> Self {
        Self {
            current_clip: None,
            looping: true,
            speed: 1.0,
            state: AnimationPlaybackState::Stopped,
            current_time: 0.0,
        }
    }
}

#[derive(Clone, Debug)]
pub enum AnimationCommand {
    Play { entity: Entity, clip_name: String, looping: bool, speed: f32 },
    Stop { entity: Entity },
    Pause { entity: Entity },
    Resume { entity: Entity },
    SetSpeed { entity: Entity, speed: f32 },
}

#[derive(Resource, Default)]
pub struct AnimationCommandQueue {
    pub commands: Vec<AnimationCommand>,
}

impl AnimationCommandQueue {
    pub fn push(&mut self, cmd: AnimationCommand) {
        self.commands.push(cmd);
    }

    pub fn drain(&mut self) -> impl Iterator<Item = AnimationCommand> + '_ {
        self.commands.drain(..)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum EasingFunction {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

impl EasingFunction {
    pub fn apply(&self, t: f32) -> f32 {
        match self {
            EasingFunction::Linear => t,
            EasingFunction::EaseIn => t * t,
            EasingFunction::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            EasingFunction::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum TweenProperty {
    Position { start: Vec3, end: Vec3 },
    Rotation { start: Quat, end: Quat },
    Scale { start: Vec3, end: Vec3 },
}

#[derive(Clone, Debug)]
pub struct ActiveTween {
    pub entity: Entity,
    pub property: TweenProperty,
    pub duration: f32,
    pub elapsed: f32,
    pub easing: EasingFunction,
}

#[derive(Resource, Default)]
pub struct ActiveTweens {
    pub tweens: Vec<ActiveTween>,
}

// =============================================================================
// HEALTH
// =============================================================================

#[derive(Clone, Debug)]
pub enum HealthCommand {
    Damage { entity: Entity, amount: f32 },
    Heal { entity: Entity, amount: f32 },
    SetHealth { entity: Entity, amount: f32 },
    SetMaxHealth { entity: Entity, amount: f32 },
    SetInvincible { entity: Entity, invincible: bool, duration: f32 },
    Kill { entity: Entity },
    Revive { entity: Entity },
}

#[derive(Resource, Default)]
pub struct HealthCommandQueue {
    pub commands: Vec<HealthCommand>,
}

impl HealthCommandQueue {
    pub fn push(&mut self, cmd: HealthCommand) {
        self.commands.push(cmd);
    }

    pub fn drain(&mut self) -> impl Iterator<Item = HealthCommand> + '_ {
        self.commands.drain(..)
    }
}

// =============================================================================
// HELPERS
// =============================================================================

/// Convert [f32; 4] to Color
pub fn array_to_color(arr: [f32; 4]) -> Color {
    Color::srgba(arr[0], arr[1], arr[2], arr[3])
}
