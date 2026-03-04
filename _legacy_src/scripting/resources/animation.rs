//! Animation resources for scripting
//!
//! Handles animation playback state and tween commands.

use bevy::prelude::*;
use std::collections::VecDeque;

/// Easing function types for tweens
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum EasingFunction {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    // Quad
    QuadIn,
    QuadOut,
    QuadInOut,
    // Cubic
    CubicIn,
    CubicOut,
    CubicInOut,
    // Sine
    SineIn,
    SineOut,
    SineInOut,
    // Bounce
    BounceIn,
    BounceOut,
    BounceInOut,
    // Elastic
    ElasticIn,
    ElasticOut,
    ElasticInOut,
    // Back (overshoot)
    BackIn,
    BackOut,
    BackInOut,
}

impl EasingFunction {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "linear" => Self::Linear,
            "ease_in" | "easein" => Self::EaseIn,
            "ease_out" | "easeout" => Self::EaseOut,
            "ease_in_out" | "easeinout" => Self::EaseInOut,
            "quad" | "quad_in" => Self::QuadIn,
            "quad_out" => Self::QuadOut,
            "quad_in_out" => Self::QuadInOut,
            "cubic" | "cubic_in" => Self::CubicIn,
            "cubic_out" => Self::CubicOut,
            "cubic_in_out" => Self::CubicInOut,
            "sine" | "sine_in" => Self::SineIn,
            "sine_out" => Self::SineOut,
            "sine_in_out" => Self::SineInOut,
            "bounce" | "bounce_out" => Self::BounceOut,
            "bounce_in" => Self::BounceIn,
            "bounce_in_out" => Self::BounceInOut,
            "elastic" | "elastic_out" => Self::ElasticOut,
            "elastic_in" => Self::ElasticIn,
            "elastic_in_out" => Self::ElasticInOut,
            "back" | "back_out" => Self::BackOut,
            "back_in" => Self::BackIn,
            "back_in_out" => Self::BackInOut,
            _ => Self::Linear,
        }
    }

    /// Apply the easing function to a normalized time value (0.0 to 1.0)
    pub fn apply(&self, t: f32) -> f32 {
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Self::QuadIn => t * t,
            Self::QuadOut => 1.0 - (1.0 - t).powi(2),
            Self::QuadInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Self::CubicIn => t * t * t,
            Self::CubicOut => 1.0 - (1.0 - t).powi(3),
            Self::CubicInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Self::SineIn => 1.0 - (t * std::f32::consts::FRAC_PI_2).cos(),
            Self::SineOut => (t * std::f32::consts::FRAC_PI_2).sin(),
            Self::SineInOut => -(((t * std::f32::consts::PI).cos() - 1.0) / 2.0),
            Self::BounceOut => bounce_out(t),
            Self::BounceIn => 1.0 - bounce_out(1.0 - t),
            Self::BounceInOut => {
                if t < 0.5 {
                    (1.0 - bounce_out(1.0 - 2.0 * t)) / 2.0
                } else {
                    (1.0 + bounce_out(2.0 * t - 1.0)) / 2.0
                }
            }
            Self::ElasticIn => elastic_in(t),
            Self::ElasticOut => elastic_out(t),
            Self::ElasticInOut => {
                if t < 0.5 {
                    elastic_in(t * 2.0) / 2.0
                } else {
                    (elastic_out(t * 2.0 - 1.0) + 1.0) / 2.0
                }
            }
            Self::BackIn => {
                let c = 1.70158;
                (c + 1.0) * t * t * t - c * t * t
            }
            Self::BackOut => {
                let c = 1.70158;
                1.0 + (c + 1.0) * (t - 1.0).powi(3) + c * (t - 1.0).powi(2)
            }
            Self::BackInOut => {
                let c = 1.70158 * 1.525;
                if t < 0.5 {
                    ((2.0 * t).powi(2) * ((c + 1.0) * 2.0 * t - c)) / 2.0
                } else {
                    ((2.0 * t - 2.0).powi(2) * ((c + 1.0) * (t * 2.0 - 2.0) + c) + 2.0) / 2.0
                }
            }
        }
    }
}

fn bounce_out(t: f32) -> f32 {
    let n1 = 7.5625;
    let d1 = 2.75;
    if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        let t = t - 1.5 / d1;
        n1 * t * t + 0.75
    } else if t < 2.5 / d1 {
        let t = t - 2.25 / d1;
        n1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / d1;
        n1 * t * t + 0.984375
    }
}

fn elastic_in(t: f32) -> f32 {
    if t == 0.0 || t == 1.0 {
        return t;
    }
    let c = (2.0 * std::f32::consts::PI) / 3.0;
    -(2.0_f32.powf(10.0 * t - 10.0) * ((t * 10.0 - 10.75) * c).sin())
}

fn elastic_out(t: f32) -> f32 {
    if t == 0.0 || t == 1.0 {
        return t;
    }
    let c = (2.0 * std::f32::consts::PI) / 3.0;
    2.0_f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c).sin() + 1.0
}

/// Animation playback state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

/// Runtime animation player component
#[derive(Component, Debug, Clone, Default)]
pub struct AnimationPlayer {
    /// Current playback state
    pub state: AnimationState,
    /// Name of the currently playing clip
    pub current_clip: Option<String>,
    /// Current playback time in seconds
    pub current_time: f32,
    /// Playback speed multiplier
    pub speed: f32,
    /// Whether the current animation should loop
    pub looping: bool,
}

impl AnimationPlayer {
    pub fn new() -> Self {
        Self {
            state: AnimationState::Stopped,
            current_clip: None,
            current_time: 0.0,
            speed: 1.0,
            looping: true,
        }
    }

    pub fn play(&mut self, clip_name: String, looping: bool, speed: f32) {
        self.current_clip = Some(clip_name);
        self.current_time = 0.0;
        self.speed = speed;
        self.looping = looping;
        self.state = AnimationState::Playing;
    }

    pub fn stop(&mut self) {
        self.state = AnimationState::Stopped;
        self.current_time = 0.0;
    }

    pub fn pause(&mut self) {
        if self.state == AnimationState::Playing {
            self.state = AnimationState::Paused;
        }
    }

    pub fn resume(&mut self) {
        if self.state == AnimationState::Paused {
            self.state = AnimationState::Playing;
        }
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    pub fn is_playing(&self) -> bool {
        self.state == AnimationState::Playing
    }
}

/// A tween that interpolates a property over time
#[derive(Debug, Clone)]
pub struct ActiveTween {
    pub entity: Entity,
    pub property: TweenProperty,
    pub start_value: TweenValue,
    pub end_value: TweenValue,
    pub duration: f32,
    pub elapsed: f32,
    pub easing: EasingFunction,
}

/// Properties that can be tweened
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TweenProperty {
    PositionX,
    PositionY,
    PositionZ,
    Position,
    RotationX,
    RotationY,
    RotationZ,
    Rotation,
    ScaleX,
    ScaleY,
    ScaleZ,
    Scale,
    Opacity,
}

impl TweenProperty {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "position_x" | "pos_x" | "x" => Some(Self::PositionX),
            "position_y" | "pos_y" | "y" => Some(Self::PositionY),
            "position_z" | "pos_z" | "z" => Some(Self::PositionZ),
            "position" | "pos" => Some(Self::Position),
            "rotation_x" | "rot_x" => Some(Self::RotationX),
            "rotation_y" | "rot_y" => Some(Self::RotationY),
            "rotation_z" | "rot_z" => Some(Self::RotationZ),
            "rotation" | "rot" => Some(Self::Rotation),
            "scale_x" | "scl_x" => Some(Self::ScaleX),
            "scale_y" | "scl_y" => Some(Self::ScaleY),
            "scale_z" | "scl_z" => Some(Self::ScaleZ),
            "scale" | "scl" => Some(Self::Scale),
            "opacity" | "alpha" => Some(Self::Opacity),
            _ => None,
        }
    }
}

/// Values for tweens
#[derive(Debug, Clone, Copy)]
pub enum TweenValue {
    Float(f32),
    Vec3(Vec3),
}

impl TweenValue {
    pub fn lerp(&self, other: &TweenValue, t: f32) -> TweenValue {
        match (self, other) {
            (TweenValue::Float(a), TweenValue::Float(b)) => TweenValue::Float(a + (b - a) * t),
            (TweenValue::Vec3(a), TweenValue::Vec3(b)) => TweenValue::Vec3(a.lerp(*b, t)),
            _ => *self,
        }
    }
}

/// Animation command types
#[derive(Debug, Clone)]
pub enum AnimationCommand {
    Play {
        entity: Entity,
        clip_name: String,
        looping: bool,
        speed: f32,
    },
    Stop {
        entity: Entity,
    },
    Pause {
        entity: Entity,
    },
    Resume {
        entity: Entity,
    },
    SetSpeed {
        entity: Entity,
        speed: f32,
    },
    // Tweens
    Tween {
        entity: Entity,
        property: TweenProperty,
        target: f32,
        duration: f32,
        easing: EasingFunction,
    },
    TweenPosition {
        entity: Entity,
        target: Vec3,
        duration: f32,
        easing: EasingFunction,
    },
    TweenRotation {
        entity: Entity,
        target: Vec3, // Euler angles in degrees
        duration: f32,
        easing: EasingFunction,
    },
    TweenScale {
        entity: Entity,
        target: Vec3,
        duration: f32,
        easing: EasingFunction,
    },
}

/// Queue for animation commands
#[derive(Resource, Default)]
pub struct AnimationCommandQueue {
    pub commands: VecDeque<AnimationCommand>,
}

impl AnimationCommandQueue {
    pub fn play(&mut self, entity: Entity, clip_name: String, looping: bool, speed: f32) {
        self.commands.push_back(AnimationCommand::Play {
            entity,
            clip_name,
            looping,
            speed,
        });
    }

    pub fn stop(&mut self, entity: Entity) {
        self.commands.push_back(AnimationCommand::Stop { entity });
    }

    pub fn pause(&mut self, entity: Entity) {
        self.commands.push_back(AnimationCommand::Pause { entity });
    }

    pub fn resume(&mut self, entity: Entity) {
        self.commands.push_back(AnimationCommand::Resume { entity });
    }

    pub fn set_speed(&mut self, entity: Entity, speed: f32) {
        self.commands
            .push_back(AnimationCommand::SetSpeed { entity, speed });
    }

    pub fn tween(
        &mut self,
        entity: Entity,
        property: TweenProperty,
        target: f32,
        duration: f32,
        easing: EasingFunction,
    ) {
        self.commands.push_back(AnimationCommand::Tween {
            entity,
            property,
            target,
            duration,
            easing,
        });
    }

    pub fn tween_position(
        &mut self,
        entity: Entity,
        target: Vec3,
        duration: f32,
        easing: EasingFunction,
    ) {
        self.commands.push_back(AnimationCommand::TweenPosition {
            entity,
            target,
            duration,
            easing,
        });
    }

    pub fn tween_rotation(
        &mut self,
        entity: Entity,
        target: Vec3,
        duration: f32,
        easing: EasingFunction,
    ) {
        self.commands.push_back(AnimationCommand::TweenRotation {
            entity,
            target,
            duration,
            easing,
        });
    }

    pub fn tween_scale(
        &mut self,
        entity: Entity,
        target: Vec3,
        duration: f32,
        easing: EasingFunction,
    ) {
        self.commands.push_back(AnimationCommand::TweenScale {
            entity,
            target,
            duration,
            easing,
        });
    }
}

/// Active tweens resource
#[derive(Resource, Default)]
pub struct ActiveTweens {
    pub tweens: Vec<ActiveTween>,
}

impl ActiveTweens {
    pub fn add(&mut self, tween: ActiveTween) {
        // Remove any existing tween for the same entity+property
        self.tweens.retain(|t| {
            !(t.entity == tween.entity && t.property == tween.property)
        });
        self.tweens.push(tween);
    }

    pub fn clear_for_entity(&mut self, entity: Entity) {
        self.tweens.retain(|t| t.entity != entity);
    }
}

/// Sprite animation playback state
#[derive(Component, Debug, Clone, Default)]
pub struct SpriteAnimationPlayer {
    /// Name of the currently playing animation
    pub current_animation: Option<String>,
    /// Current frame index within the animation
    pub current_frame: usize,
    /// Timer for frame advancement
    pub frame_timer: f32,
    /// Duration of each frame (from SpriteAnimation)
    pub frame_duration: f32,
    /// First frame of current animation
    pub first_frame: usize,
    /// Last frame of current animation
    pub last_frame: usize,
    /// Whether the animation loops
    pub looping: bool,
    /// Whether currently playing
    pub playing: bool,
}

impl SpriteAnimationPlayer {
    /// Get the current absolute frame index in the sprite sheet
    pub fn current_sprite_index(&self) -> usize {
        self.first_frame + self.current_frame
    }
}

/// Sprite animation command
#[derive(Debug, Clone)]
pub enum SpriteAnimationCommand {
    /// Play a named animation
    Play {
        entity: Entity,
        animation_name: String,
        looping: bool,
    },
    /// Stop the current animation
    Stop {
        entity: Entity,
    },
    /// Set a specific frame (0-indexed within the current animation)
    SetFrame {
        entity: Entity,
        frame: usize,
    },
    /// Set a specific absolute frame in the sprite sheet
    SetAbsoluteFrame {
        entity: Entity,
        frame: usize,
    },
}

/// Queue for sprite animation commands
#[derive(Resource, Default)]
pub struct SpriteAnimationCommandQueue {
    pub commands: Vec<SpriteAnimationCommand>,
}

impl SpriteAnimationCommandQueue {
    pub fn play(&mut self, entity: Entity, animation_name: String, looping: bool) {
        self.commands.push(SpriteAnimationCommand::Play {
            entity,
            animation_name,
            looping,
        });
    }

    pub fn stop(&mut self, entity: Entity) {
        self.commands.push(SpriteAnimationCommand::Stop { entity });
    }

    pub fn set_frame(&mut self, entity: Entity, frame: usize) {
        self.commands.push(SpriteAnimationCommand::SetFrame { entity, frame });
    }

    pub fn set_absolute_frame(&mut self, entity: Entity, frame: usize) {
        self.commands.push(SpriteAnimationCommand::SetAbsoluteFrame { entity, frame });
    }

    pub fn drain(&mut self) -> Vec<SpriteAnimationCommand> {
        std::mem::take(&mut self.commands)
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}
