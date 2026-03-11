//! Ticks `UiTween` components, interpolates properties, and removes on completion.

use bevy::prelude::*;

use crate::components::{TweenComplete, UiTween, UiTweenProperty};

pub fn ui_tween_system(
    mut commands: Commands,
    time: Res<Time>,
    mut tweens: Query<(Entity, &mut UiTween, &mut Node, Option<&mut BackgroundColor>)>,
) {
    let dt = time.delta_secs();

    for (entity, mut tween, mut node, bg) in &mut tweens {
        tween.elapsed += dt;
        let t = (tween.elapsed / tween.duration).clamp(0.0, 1.0);
        let eased = tween.easing.evaluate(t);
        let value = tween.start + (tween.target - tween.start) * eased;

        match &tween.property {
            UiTweenProperty::PositionX => {
                node.left = Val::Px(value);
            }
            UiTweenProperty::PositionY => {
                node.top = Val::Px(value);
            }
            UiTweenProperty::Width => {
                node.width = Val::Px(value);
            }
            UiTweenProperty::Height => {
                node.height = Val::Px(value);
            }
            UiTweenProperty::BgColorA => {
                if let Some(mut bg) = bg {
                    let mut srgba = bg.0.to_srgba();
                    srgba.alpha = value;
                    bg.0 = srgba.into();
                }
            }
            UiTweenProperty::BgColorR => {
                if let Some(mut bg) = bg {
                    let mut srgba = bg.0.to_srgba();
                    srgba.red = value;
                    bg.0 = srgba.into();
                }
            }
            UiTweenProperty::BgColorG => {
                if let Some(mut bg) = bg {
                    let mut srgba = bg.0.to_srgba();
                    srgba.green = value;
                    bg.0 = srgba.into();
                }
            }
            UiTweenProperty::BgColorB => {
                if let Some(mut bg) = bg {
                    let mut srgba = bg.0.to_srgba();
                    srgba.blue = value;
                    bg.0 = srgba.into();
                }
            }
            _ => {} // Opacity, Scale, Rotation handled by transform systems
        }

        if t >= 1.0 {
            match tween.on_complete {
                TweenComplete::Remove => {
                    commands.entity(entity).remove::<UiTween>();
                }
                TweenComplete::Loop => {
                    tween.elapsed = 0.0;
                }
                TweenComplete::PingPong => {
                    tween.elapsed = 0.0;
                    let tmp = tween.start;
                    tween.start = tween.target;
                    tween.target = tmp;
                }
            }
        }
    }
}
