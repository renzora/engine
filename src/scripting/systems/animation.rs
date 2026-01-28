//! Animation systems for processing commands and playing animations

use bevy::prelude::*;

use crate::core::{PlayModeState, PlayState};
use crate::shared::components::animation::{AnimationData, AnimatableProperty, KeyframeValue};
use crate::scripting::resources::{
    ActiveTween, ActiveTweens, AnimationCommand, AnimationCommandQueue, AnimationPlayer,
    AnimationState, TweenProperty, TweenValue,
};

/// Process animation commands from the queue
pub fn process_animation_commands(
    mut commands: Commands,
    mut queue: ResMut<AnimationCommandQueue>,
    mut players: Query<&mut AnimationPlayer>,
    mut tweens: ResMut<ActiveTweens>,
    transforms: Query<&Transform>,
) {
    while let Some(cmd) = queue.commands.pop_front() {
        match cmd {
            AnimationCommand::Play {
                entity,
                clip_name,
                looping,
                speed,
            } => {
                if let Ok(mut player) = players.get_mut(entity) {
                    player.play(clip_name, looping, speed);
                } else {
                    // Entity doesn't have AnimationPlayer, add one
                    commands.entity(entity).insert(AnimationPlayer {
                        state: AnimationState::Playing,
                        current_clip: Some(clip_name),
                        current_time: 0.0,
                        speed,
                        looping,
                    });
                }
            }
            AnimationCommand::Stop { entity } => {
                if let Ok(mut player) = players.get_mut(entity) {
                    player.stop();
                }
            }
            AnimationCommand::Pause { entity } => {
                if let Ok(mut player) = players.get_mut(entity) {
                    player.pause();
                }
            }
            AnimationCommand::Resume { entity } => {
                if let Ok(mut player) = players.get_mut(entity) {
                    player.resume();
                }
            }
            AnimationCommand::SetSpeed { entity, speed } => {
                if let Ok(mut player) = players.get_mut(entity) {
                    player.set_speed(speed);
                }
            }
            AnimationCommand::Tween {
                entity,
                property,
                target,
                duration,
                easing,
            } => {
                // Get current value from transform
                if let Ok(transform) = transforms.get(entity) {
                    let start_value = get_tween_start_value(transform, property);
                    tweens.add(ActiveTween {
                        entity,
                        property,
                        start_value,
                        end_value: TweenValue::Float(target),
                        duration,
                        elapsed: 0.0,
                        easing,
                    });
                }
            }
            AnimationCommand::TweenPosition {
                entity,
                target,
                duration,
                easing,
            } => {
                if let Ok(transform) = transforms.get(entity) {
                    tweens.add(ActiveTween {
                        entity,
                        property: TweenProperty::Position,
                        start_value: TweenValue::Vec3(transform.translation),
                        end_value: TweenValue::Vec3(target),
                        duration,
                        elapsed: 0.0,
                        easing,
                    });
                }
            }
            AnimationCommand::TweenRotation {
                entity,
                target,
                duration,
                easing,
            } => {
                if let Ok(transform) = transforms.get(entity) {
                    let current_euler = transform.rotation.to_euler(EulerRot::XYZ);
                    tweens.add(ActiveTween {
                        entity,
                        property: TweenProperty::Rotation,
                        start_value: TweenValue::Vec3(Vec3::new(
                            current_euler.0.to_degrees(),
                            current_euler.1.to_degrees(),
                            current_euler.2.to_degrees(),
                        )),
                        end_value: TweenValue::Vec3(target),
                        duration,
                        elapsed: 0.0,
                        easing,
                    });
                }
            }
            AnimationCommand::TweenScale {
                entity,
                target,
                duration,
                easing,
            } => {
                if let Ok(transform) = transforms.get(entity) {
                    tweens.add(ActiveTween {
                        entity,
                        property: TweenProperty::Scale,
                        start_value: TweenValue::Vec3(transform.scale),
                        end_value: TweenValue::Vec3(target),
                        duration,
                        elapsed: 0.0,
                        easing,
                    });
                }
            }
        }
    }
}

fn get_tween_start_value(transform: &Transform, property: TweenProperty) -> TweenValue {
    match property {
        TweenProperty::PositionX => TweenValue::Float(transform.translation.x),
        TweenProperty::PositionY => TweenValue::Float(transform.translation.y),
        TweenProperty::PositionZ => TweenValue::Float(transform.translation.z),
        TweenProperty::Position => TweenValue::Vec3(transform.translation),
        TweenProperty::RotationX => {
            let euler = transform.rotation.to_euler(EulerRot::XYZ);
            TweenValue::Float(euler.0.to_degrees())
        }
        TweenProperty::RotationY => {
            let euler = transform.rotation.to_euler(EulerRot::XYZ);
            TweenValue::Float(euler.1.to_degrees())
        }
        TweenProperty::RotationZ => {
            let euler = transform.rotation.to_euler(EulerRot::XYZ);
            TweenValue::Float(euler.2.to_degrees())
        }
        TweenProperty::Rotation => {
            let euler = transform.rotation.to_euler(EulerRot::XYZ);
            TweenValue::Vec3(Vec3::new(
                euler.0.to_degrees(),
                euler.1.to_degrees(),
                euler.2.to_degrees(),
            ))
        }
        TweenProperty::ScaleX => TweenValue::Float(transform.scale.x),
        TweenProperty::ScaleY => TweenValue::Float(transform.scale.y),
        TweenProperty::ScaleZ => TweenValue::Float(transform.scale.z),
        TweenProperty::Scale => TweenValue::Vec3(transform.scale),
        TweenProperty::Opacity => TweenValue::Float(1.0), // Default, would need material access
    }
}

/// Update active tweens
pub fn update_tweens(
    time: Res<Time>,
    mut tweens: ResMut<ActiveTweens>,
    mut transforms: Query<&mut Transform>,
) {
    let delta = time.delta_secs();
    let mut completed = Vec::new();

    for (i, tween) in tweens.tweens.iter_mut().enumerate() {
        tween.elapsed += delta;
        let t = (tween.elapsed / tween.duration).min(1.0);
        let eased_t = tween.easing.apply(t);

        if let Ok(mut transform) = transforms.get_mut(tween.entity) {
            let interpolated = tween.start_value.lerp(&tween.end_value, eased_t);
            apply_tween_value(&mut transform, tween.property, interpolated);
        }

        if t >= 1.0 {
            completed.push(i);
        }
    }

    // Remove completed tweens in reverse order to maintain indices
    for i in completed.into_iter().rev() {
        tweens.tweens.remove(i);
    }
}

fn apply_tween_value(transform: &mut Transform, property: TweenProperty, value: TweenValue) {
    match (property, value) {
        (TweenProperty::PositionX, TweenValue::Float(v)) => transform.translation.x = v,
        (TweenProperty::PositionY, TweenValue::Float(v)) => transform.translation.y = v,
        (TweenProperty::PositionZ, TweenValue::Float(v)) => transform.translation.z = v,
        (TweenProperty::Position, TweenValue::Vec3(v)) => transform.translation = v,
        (TweenProperty::RotationX, TweenValue::Float(v)) => {
            let euler = transform.rotation.to_euler(EulerRot::XYZ);
            transform.rotation =
                Quat::from_euler(EulerRot::XYZ, v.to_radians(), euler.1, euler.2);
        }
        (TweenProperty::RotationY, TweenValue::Float(v)) => {
            let euler = transform.rotation.to_euler(EulerRot::XYZ);
            transform.rotation =
                Quat::from_euler(EulerRot::XYZ, euler.0, v.to_radians(), euler.2);
        }
        (TweenProperty::RotationZ, TweenValue::Float(v)) => {
            let euler = transform.rotation.to_euler(EulerRot::XYZ);
            transform.rotation =
                Quat::from_euler(EulerRot::XYZ, euler.0, euler.1, v.to_radians());
        }
        (TweenProperty::Rotation, TweenValue::Vec3(v)) => {
            transform.rotation = Quat::from_euler(
                EulerRot::XYZ,
                v.x.to_radians(),
                v.y.to_radians(),
                v.z.to_radians(),
            );
        }
        (TweenProperty::ScaleX, TweenValue::Float(v)) => transform.scale.x = v,
        (TweenProperty::ScaleY, TweenValue::Float(v)) => transform.scale.y = v,
        (TweenProperty::ScaleZ, TweenValue::Float(v)) => transform.scale.z = v,
        (TweenProperty::Scale, TweenValue::Vec3(v)) => transform.scale = v,
        _ => {}
    }
}

/// Update animation playback for entities with AnimationPlayer and AnimationData
pub fn update_animation_playback(
    time: Res<Time>,
    mut players: Query<(Entity, &mut AnimationPlayer, &AnimationData, &mut Transform)>,
) {
    let delta = time.delta_secs();

    for (_entity, mut player, anim_data, mut transform) in players.iter_mut() {
        if player.state != AnimationState::Playing {
            continue;
        }

        // Find the clip by name
        let clip = anim_data.clips.iter().find(|c| {
            player
                .current_clip
                .as_ref()
                .map(|name| &c.name == name)
                .unwrap_or(false)
        });

        let Some(clip) = clip else {
            continue;
        };

        // Advance time
        player.current_time += delta * player.speed * clip.speed;

        let duration = clip.duration();
        if duration > 0.0 {
            if player.current_time >= duration {
                if player.looping {
                    player.current_time %= duration;
                } else {
                    player.current_time = duration;
                    player.state = AnimationState::Stopped;
                }
            }
        }

        // Sample and apply values
        let values = clip.sample(player.current_time);
        for (prop, value) in values {
            apply_animation_value(&mut transform, prop, value);
        }
    }
}

fn apply_animation_value(
    transform: &mut Transform,
    property: AnimatableProperty,
    value: KeyframeValue,
) {
    match (property, value) {
        (AnimatableProperty::PositionX, KeyframeValue::Float(v)) => transform.translation.x = v,
        (AnimatableProperty::PositionY, KeyframeValue::Float(v)) => transform.translation.y = v,
        (AnimatableProperty::PositionZ, KeyframeValue::Float(v)) => transform.translation.z = v,
        (AnimatableProperty::RotationX, KeyframeValue::Float(v)) => {
            let euler = transform.rotation.to_euler(EulerRot::XYZ);
            transform.rotation =
                Quat::from_euler(EulerRot::XYZ, v.to_radians(), euler.1, euler.2);
        }
        (AnimatableProperty::RotationY, KeyframeValue::Float(v)) => {
            let euler = transform.rotation.to_euler(EulerRot::XYZ);
            transform.rotation =
                Quat::from_euler(EulerRot::XYZ, euler.0, v.to_radians(), euler.2);
        }
        (AnimatableProperty::RotationZ, KeyframeValue::Float(v)) => {
            let euler = transform.rotation.to_euler(EulerRot::XYZ);
            transform.rotation =
                Quat::from_euler(EulerRot::XYZ, euler.0, euler.1, v.to_radians());
        }
        (AnimatableProperty::ScaleX, KeyframeValue::Float(v)) => transform.scale.x = v,
        (AnimatableProperty::ScaleY, KeyframeValue::Float(v)) => transform.scale.y = v,
        (AnimatableProperty::ScaleZ, KeyframeValue::Float(v)) => transform.scale.z = v,
        _ => {}
    }
}

/// Cleanup animation state when play mode stops
pub fn clear_animation_on_stop(
    play_mode: Res<PlayModeState>,
    mut last_state: Local<PlayState>,
    mut queue: ResMut<AnimationCommandQueue>,
    mut sprite_queue: ResMut<SpriteAnimationCommandQueue>,
    mut tweens: ResMut<ActiveTweens>,
    mut commands: Commands,
    players: Query<Entity, With<AnimationPlayer>>,
    sprite_players: Query<Entity, With<SpriteAnimationPlayer>>,
) {
    // Detect transition from Playing to Editing
    if *last_state == PlayState::Playing && play_mode.state == PlayState::Editing {
        // Clear command queue
        queue.commands.clear();
        sprite_queue.commands.clear();
        // Clear active tweens
        tweens.tweens.clear();
        // Remove AnimationPlayer components that were added at runtime
        for entity in players.iter() {
            commands.entity(entity).remove::<AnimationPlayer>();
        }
        // Remove SpriteAnimationPlayer components that were added at runtime
        for entity in sprite_players.iter() {
            commands.entity(entity).remove::<SpriteAnimationPlayer>();
        }
        bevy::log::info!("[Animation] Cleared animation state on play stop");
    }
    *last_state = play_mode.state;
}

// =============================================================================
// Sprite Animation Systems
// =============================================================================

use crate::shared::components::SpriteSheetData;
use crate::scripting::resources::{
    SpriteAnimationCommand, SpriteAnimationCommandQueue, SpriteAnimationPlayer,
};

/// Process sprite animation commands from the queue
pub fn process_sprite_animation_commands(
    mut commands: Commands,
    mut queue: ResMut<SpriteAnimationCommandQueue>,
    mut players: Query<(&mut SpriteAnimationPlayer, &SpriteSheetData)>,
    sprite_sheets: Query<&SpriteSheetData, Without<SpriteAnimationPlayer>>,
) {
    if queue.is_empty() {
        return;
    }

    for cmd in queue.drain() {
        match cmd {
            SpriteAnimationCommand::Play {
                entity,
                animation_name,
                looping,
            } => {
                if let Ok((mut player, sheet_data)) = players.get_mut(entity) {
                    // Find the animation in the sprite sheet data
                    if let Some(anim) = sheet_data.get_animation(&animation_name) {
                        player.current_animation = Some(animation_name.clone());
                        player.first_frame = anim.first_frame;
                        player.last_frame = anim.last_frame;
                        player.current_frame = 0;
                        player.frame_timer = 0.0;
                        player.frame_duration = anim.frame_duration;
                        player.looping = looping;
                        player.playing = true;
                        debug!(
                            "Playing sprite animation '{}' on {:?} (frames {}-{})",
                            animation_name, entity, anim.first_frame, anim.last_frame
                        );
                    } else {
                        warn!(
                            "Sprite animation '{}' not found on entity {:?}",
                            animation_name, entity
                        );
                    }
                } else if let Ok(sheet_data) = sprite_sheets.get(entity) {
                    // Entity has SpriteSheetData but no player - add one
                    if let Some(anim) = sheet_data.get_animation(&animation_name) {
                        commands.entity(entity).insert(SpriteAnimationPlayer {
                            current_animation: Some(animation_name.clone()),
                            first_frame: anim.first_frame,
                            last_frame: anim.last_frame,
                            current_frame: 0,
                            frame_timer: 0.0,
                            frame_duration: anim.frame_duration,
                            looping,
                            playing: true,
                        });
                        debug!(
                            "Created SpriteAnimationPlayer for '{}' on {:?}",
                            animation_name, entity
                        );
                    } else {
                        warn!(
                            "Sprite animation '{}' not found on entity {:?}",
                            animation_name, entity
                        );
                    }
                } else {
                    warn!(
                        "Entity {:?} has no SpriteSheetData component",
                        entity
                    );
                }
            }

            SpriteAnimationCommand::Stop { entity } => {
                if let Ok((mut player, _)) = players.get_mut(entity) {
                    player.playing = false;
                    player.current_animation = None;
                    debug!("Stopped sprite animation on {:?}", entity);
                }
            }

            SpriteAnimationCommand::SetFrame { entity, frame } => {
                if let Ok((mut player, _)) = players.get_mut(entity) {
                    let max_frame = player.last_frame - player.first_frame;
                    player.current_frame = frame.min(max_frame);
                    player.frame_timer = 0.0;
                    debug!(
                        "Set sprite frame to {} on {:?}",
                        player.current_frame, entity
                    );
                }
            }

            SpriteAnimationCommand::SetAbsoluteFrame { entity, frame } => {
                if let Ok((mut player, sheet_data)) = players.get_mut(entity) {
                    let frame = frame.min(sheet_data.frame_count.saturating_sub(1));
                    // Set the player to show this absolute frame
                    player.first_frame = frame;
                    player.last_frame = frame;
                    player.current_frame = 0;
                    player.playing = false;
                    debug!("Set absolute sprite frame to {} on {:?}", frame, entity);
                } else {
                    // Just try to set the TextureAtlas index directly
                    debug!(
                        "SetAbsoluteFrame {} on {:?} (no player)",
                        frame, entity
                    );
                }
            }
        }
    }
}

/// Update sprite animations - advance frames and update TextureAtlas
pub fn update_sprite_animations(
    time: Res<Time>,
    mut query: Query<(&mut SpriteAnimationPlayer, &mut Sprite)>,
) {
    let delta = time.delta_secs();

    for (mut player, mut sprite) in query.iter_mut() {
        if !player.playing {
            // Still update the sprite to show current frame even when not animating
            let index = player.current_sprite_index();
            if let Some(ref mut atlas) = sprite.texture_atlas {
                if atlas.index != index {
                    atlas.index = index;
                }
            }
            continue;
        }

        // Advance timer
        player.frame_timer += delta;

        // Check if we should advance frame
        if player.frame_timer >= player.frame_duration {
            player.frame_timer -= player.frame_duration;

            let frame_count = player.last_frame - player.first_frame + 1;
            player.current_frame += 1;

            if player.current_frame >= frame_count {
                if player.looping {
                    player.current_frame = 0;
                } else {
                    player.current_frame = frame_count - 1;
                    player.playing = false;
                }
            }
        }

        // Update the TextureAtlas index
        let index = player.current_sprite_index();
        if let Some(ref mut atlas) = sprite.texture_atlas {
            if atlas.index != index {
                atlas.index = index;
            }
        }
    }
}
