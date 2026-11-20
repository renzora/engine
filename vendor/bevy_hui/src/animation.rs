use bevy::{math::UVec2, prelude::{Component, ImageNode, Query, Res}, reflect::Reflect, time::{Time, Timer}};

use crate::styles::HtmlStyle;

#[derive(Debug, Clone, Reflect)]
#[reflect]
pub struct Atlas {
    pub size: UVec2,
    pub columns: u32,
    pub rows: u32,
    pub padding: Option<UVec2>,
    pub offset: Option<UVec2>,
}

impl Default for Atlas {
    fn default() -> Self {
        Atlas { size: [0,0].into(), columns: 0, rows: 0, padding: None, offset: None }
    }
}

#[derive(Debug, Default, Reflect, PartialEq, Clone, Component)]
#[reflect]
pub enum AnimationDirection {
    #[default]
    Forward,
    Reverse,
    AlternateForward,
    AlternateReverse,
}

#[derive(Component)]
pub struct ActiveAnimation {
    pub timer: Timer,
    pub frame: usize,
    pub iterations: i64,
    pub duration: f32,
    pub direction: AnimationDirection,
}

pub fn run_animations(
    time: Res<Time>,
    mut query: Query<(&mut ActiveAnimation, &mut ImageNode, &HtmlStyle)>,
) {
    for (mut active_animation, mut node, style) in query.iter_mut() {
        if active_animation.iterations == 0 {
            continue;
        }

        if style.computed.duration > 0.0 {
            active_animation.duration = active_animation.duration - time.delta_secs();

            if active_animation.duration <= 0.0 {
                continue;
            }
        }

        active_animation.timer.tick(time.delta());

        if style.computed.frames.len() == 1 {
            continue;
        }

        if active_animation.timer.is_finished() {
            let atlas = node.texture_atlas.as_mut().unwrap();
            let atlas_details = style.computed.atlas.as_ref().unwrap();

            if style.computed.frames.len() == 0 {
                let frame_count = (atlas_details.columns * atlas_details.rows) as usize;

                match active_animation.direction {
                    AnimationDirection::Forward => {
                        if atlas.index == frame_count - 1 {
                            if style.computed.direction == AnimationDirection::AlternateForward || style.computed.direction == AnimationDirection::AlternateReverse{
                                active_animation.direction = AnimationDirection::Reverse;
                                active_animation.frame = frame_count - 2;
                            } else {
                                active_animation.frame = 0;
                            }
                            active_animation.iterations = active_animation.iterations - 1;
                        } else {
                            active_animation.frame = active_animation.frame + 1;
                        }
                    }
                    AnimationDirection::Reverse => {
                        if atlas.index == 0 {
                            if style.computed.direction == AnimationDirection::AlternateForward || style.computed.direction == AnimationDirection::AlternateReverse{
                                active_animation.direction = AnimationDirection::Forward;
                                active_animation.frame = 1;
                            } else {
                                active_animation.frame = frame_count - 1;
                            }
                            active_animation.iterations = active_animation.iterations - 1;
                        } else {
                            active_animation.frame = active_animation.frame - 1;
                        }
                    }
                    _ => (),
                }

                node.texture_atlas.as_mut().unwrap().index = active_animation.frame;
            } else {
                let frame_count = style.computed.frames.len();

                match active_animation.direction {
                    AnimationDirection::Forward => {
                        if active_animation.frame == frame_count - 1 {
                            if style.computed.direction == AnimationDirection::AlternateForward || style.computed.direction == AnimationDirection::AlternateReverse{
                                active_animation.direction = AnimationDirection::Reverse;
                                active_animation.frame = frame_count - 2;
                            } else {
                                active_animation.frame = 0;
                            }
                            active_animation.iterations = active_animation.iterations - 1;
                        } else {
                            active_animation.frame = active_animation.frame + 1;
                        }
                    },
                    AnimationDirection::Reverse => {
                        if active_animation.frame == 0 {
                            if style.computed.direction == AnimationDirection::AlternateForward || style.computed.direction == AnimationDirection::AlternateReverse{
                                active_animation.direction = AnimationDirection::Forward;
                                active_animation.frame = 1;
                            } else {
                                active_animation.frame = frame_count - 1;
                            }
                            active_animation.iterations = active_animation.iterations - 1;
                        } else {
                            active_animation.frame = active_animation.frame - 1;
                        }
                    }
                    _ => (),
                }

                node.texture_atlas.as_mut().unwrap().index = style.computed.frames[active_animation.frame] as usize;
            }
        }
    }
}
