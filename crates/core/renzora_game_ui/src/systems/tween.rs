//! Ticks `UiTween` components, interpolates properties, and removes on completion.

use bevy::prelude::*;

use crate::components::{TweenComplete, UiTween, UiTweenProperty, UiWidgetStyle, UiFill};

pub fn ui_tween_system(
    mut commands: Commands,
    time: Res<Time>,
    mut tweens: Query<(Entity, &mut UiTween, &mut Node, Option<&mut UiWidgetStyle>)>,
) {
    let dt = time.delta_secs();

    for (entity, mut tween, mut node, mut style) in &mut tweens {
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
            UiTweenProperty::Opacity => {
                if let Some(ref mut style) = style {
                    style.opacity = value;
                }
            }
            UiTweenProperty::BgColorR
            | UiTweenProperty::BgColorG
            | UiTweenProperty::BgColorB
            | UiTweenProperty::BgColorA => {
                if let Some(ref mut style) = style {
                    let mut srgba = style.fill.primary_color().to_srgba();
                    match &tween.property {
                        UiTweenProperty::BgColorR => srgba.red = value,
                        UiTweenProperty::BgColorG => srgba.green = value,
                        UiTweenProperty::BgColorB => srgba.blue = value,
                        UiTweenProperty::BgColorA => srgba.alpha = value,
                        _ => {}
                    }
                    style.fill = UiFill::Solid(srgba.into());
                }
            }
            _ => {} // Scale, Rotation handled by transform systems
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
