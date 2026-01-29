//! Runtime scripting systems

use bevy::prelude::*;
use bevy::ecs::message::MessageReader;
use bevy::ecs::system::ParamSet;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use avian3d::prelude::*;

use super::resources::*;
use crate::shared::HealthData;

// =============================================================================
// INPUT SYSTEM
// =============================================================================

pub fn update_script_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut mouse_wheel: MessageReader<MouseWheel>,
    windows: Query<&Window>,
    gamepads: Query<(Entity, &Gamepad)>,
    mut input: ResMut<ScriptInput>,
) {
    input.keys_just_pressed.clear();
    input.keys_just_released.clear();
    input.mouse_buttons_just_pressed.clear();

    input.keys_pressed = keyboard.get_pressed().cloned().collect();
    input.keys_just_pressed = keyboard.get_just_pressed().cloned().collect();
    input.keys_just_released = keyboard.get_just_released().cloned().collect();

    input.mouse_buttons_pressed = mouse_buttons.get_pressed().cloned().collect();
    input.mouse_buttons_just_pressed = mouse_buttons.get_just_pressed().cloned().collect();

    if let Some(window) = windows.iter().next() {
        if let Some(pos) = window.cursor_position() {
            input.mouse_position = pos;
        }
    }

    input.mouse_delta = Vec2::ZERO;
    for event in mouse_motion.read() {
        input.mouse_delta += event.delta;
    }

    input.mouse_scroll = 0.0;
    for event in mouse_wheel.read() {
        input.mouse_scroll += event.y;
    }

    // Reset gamepad state
    for i in 0..4 {
        input.gamepad_axes[i] = [0.0; 6];
        for j in 0..16 {
            input.gamepad_buttons_just_pressed[i][j] = false;
        }
    }

    for (idx, (_entity, gamepad)) in gamepads.iter().enumerate() {
        if idx >= 4 { break; }

        input.gamepad_axes[idx][0] = gamepad.get(GamepadAxis::LeftStickX).unwrap_or(0.0);
        input.gamepad_axes[idx][1] = gamepad.get(GamepadAxis::LeftStickY).unwrap_or(0.0);
        input.gamepad_axes[idx][2] = gamepad.get(GamepadAxis::RightStickX).unwrap_or(0.0);
        input.gamepad_axes[idx][3] = gamepad.get(GamepadAxis::RightStickY).unwrap_or(0.0);
        input.gamepad_axes[idx][4] = gamepad.get(GamepadAxis::LeftZ).unwrap_or(0.0);
        input.gamepad_axes[idx][5] = gamepad.get(GamepadAxis::RightZ).unwrap_or(0.0);

        let buttons = [
            GamepadButton::South, GamepadButton::East, GamepadButton::West, GamepadButton::North,
            GamepadButton::LeftTrigger, GamepadButton::RightTrigger,
            GamepadButton::LeftTrigger2, GamepadButton::RightTrigger2,
            GamepadButton::Select, GamepadButton::Start,
            GamepadButton::LeftThumb, GamepadButton::RightThumb,
            GamepadButton::DPadUp, GamepadButton::DPadDown, GamepadButton::DPadLeft, GamepadButton::DPadRight,
        ];

        for (btn_idx, button) in buttons.iter().enumerate() {
            input.gamepad_buttons_pressed[idx][btn_idx] = gamepad.pressed(*button);
            input.gamepad_buttons_just_pressed[idx][btn_idx] = gamepad.just_pressed(*button);
        }
    }
}

// =============================================================================
// TIMER SYSTEM
// =============================================================================

pub fn update_script_timers(time: Res<Time>, mut timers: ResMut<ScriptTimers>) {
    timers.tick(time.delta_secs());
}

// =============================================================================
// COLLISION SYSTEM
// =============================================================================

pub fn collect_collision_events(
    mut collision_started: MessageReader<CollisionStart>,
    mut collision_ended: MessageReader<CollisionEnd>,
    mut script_collisions: ResMut<ScriptCollisionEvents>,
) {
    script_collisions.clear_frame_events();

    // Process collision start events
    for event in collision_started.read() {
        script_collisions.add_collision_started(event.collider1, event.collider2);
    }

    // Process collision end events
    for event in collision_ended.read() {
        script_collisions.add_collision_ended(event.collider1, event.collider2);
    }
}

// =============================================================================
// PHYSICS COMMANDS
// =============================================================================

pub fn process_physics_commands(
    mut commands: Commands,
    mut physics_queue: ResMut<PhysicsCommandQueue>,
    mut raycast_results: ResMut<RaycastResults>,
    // ParamSet to handle conflicting queries - Forces accesses velocities internally
    mut physics_params: ParamSet<(
        Query<Forces>,                    // p0: For force/impulse/torque
        Query<&mut LinearVelocity>,       // p1: For set velocity
        Query<&mut AngularVelocity>,      // p2: For set angular velocity
    )>,
    mut gravity_scales: Query<&mut GravityScale>,
    spatial_query: SpatialQuery,
) {
    if physics_queue.commands.is_empty() {
        return;
    }

    raycast_results.results.clear();

    // Collect commands to process
    let cmds: Vec<_> = physics_queue.drain().collect();

    for cmd in cmds {
        match cmd {
            PhysicsCommand::ApplyForce { entity, force } => {
                if let Ok(mut forces) = physics_params.p0().get_mut(entity) {
                    forces.apply_force(force);
                } else {
                    commands.entity(entity).insert(ConstantForce::new(force.x, force.y, force.z));
                }
            }
            PhysicsCommand::ApplyImpulse { entity, impulse } => {
                if let Ok(mut forces) = physics_params.p0().get_mut(entity) {
                    forces.apply_linear_impulse(impulse);
                } else {
                    commands.entity(entity).insert(LinearVelocity(impulse));
                }
            }
            PhysicsCommand::ApplyTorque { entity, torque } => {
                if let Ok(mut forces) = physics_params.p0().get_mut(entity) {
                    forces.apply_torque(torque);
                } else {
                    commands.entity(entity).insert(ConstantTorque::new(torque.x, torque.y, torque.z));
                }
            }
            PhysicsCommand::SetVelocity { entity, velocity } => {
                if let Ok(mut lin_vel) = physics_params.p1().get_mut(entity) {
                    lin_vel.0 = velocity;
                } else {
                    commands.entity(entity).insert(LinearVelocity(velocity));
                }
            }
            PhysicsCommand::SetAngularVelocity { entity, velocity } => {
                if let Ok(mut ang_vel) = physics_params.p2().get_mut(entity) {
                    ang_vel.0 = velocity;
                } else {
                    commands.entity(entity).insert(AngularVelocity(velocity));
                }
            }
            PhysicsCommand::SetGravityScale { entity, scale } => {
                if let Ok(mut gravity) = gravity_scales.get_mut(entity) {
                    gravity.0 = scale;
                } else {
                    commands.entity(entity).insert(GravityScale(scale));
                }
            }
            PhysicsCommand::Raycast { origin, direction, max_distance, requester_entity, result_var } => {
                let dir = Dir3::new(direction.normalize()).unwrap_or(Dir3::NEG_Z);
                if let Some(hit) = spatial_query.cast_ray(
                    origin,
                    dir,
                    max_distance,
                    true,
                    &SpatialQueryFilter::default(),
                ) {
                    raycast_results.results.insert(
                        (requester_entity, result_var),
                        RaycastHit {
                            entity: hit.entity,
                            point: origin + direction.normalize() * hit.distance,
                            normal: hit.normal,
                            distance: hit.distance,
                        },
                    );
                }
            }
        }
    }
}

// =============================================================================
// AUDIO COMMANDS
// =============================================================================

pub fn process_audio_commands(
    mut commands: Commands,
    mut audio_queue: ResMut<AudioCommandQueue>,
    mut audio_state: ResMut<AudioState>,
    asset_server: Res<AssetServer>,
) {
    for cmd in audio_queue.drain() {
        match cmd {
            AudioCommand::PlaySound { path, volume, looping } => {
                let source = asset_server.load::<AudioSource>(&path);
                commands.spawn((
                    AudioPlayer(source),
                    PlaybackSettings {
                        mode: if looping { bevy::audio::PlaybackMode::Loop } else { bevy::audio::PlaybackMode::Despawn },
                        volume: bevy::audio::Volume::Linear(volume * audio_state.master_volume),
                        ..default()
                    },
                ));
            }
            AudioCommand::PlaySound3D { path, position, volume, looping } => {
                let source = asset_server.load::<AudioSource>(&path);
                commands.spawn((
                    AudioPlayer(source),
                    PlaybackSettings {
                        mode: if looping { bevy::audio::PlaybackMode::Loop } else { bevy::audio::PlaybackMode::Despawn },
                        volume: bevy::audio::Volume::Linear(volume * audio_state.master_volume),
                        spatial: true,
                        ..default()
                    },
                    Transform::from_translation(position),
                ));
            }
            AudioCommand::PlayMusic { path, volume, fade_in } => {
                if let Some(entity) = audio_state.current_music {
                    commands.entity(entity).despawn();
                }

                let source = asset_server.load::<AudioSource>(&path);
                let start_volume = if fade_in > 0.0 { 0.0 } else { volume };

                let entity = commands.spawn((
                    AudioPlayer(source),
                    PlaybackSettings {
                        mode: bevy::audio::PlaybackMode::Loop,
                        volume: bevy::audio::Volume::Linear(start_volume * audio_state.master_volume),
                        ..default()
                    },
                )).id();

                audio_state.current_music = Some(entity);
                audio_state.music_volume = volume;

                if fade_in > 0.0 {
                    audio_state.fade = Some(AudioFade {
                        fade_type: FadeType::In,
                        duration: fade_in,
                        elapsed: 0.0,
                        start_volume: 0.0,
                        target_volume: volume,
                    });
                }
            }
            AudioCommand::StopMusic { fade_out } => {
                if fade_out > 0.0 {
                    audio_state.fade = Some(AudioFade {
                        fade_type: FadeType::Out,
                        duration: fade_out,
                        elapsed: 0.0,
                        start_volume: audio_state.music_volume,
                        target_volume: 0.0,
                    });
                } else if let Some(entity) = audio_state.current_music.take() {
                    commands.entity(entity).despawn();
                }
            }
            AudioCommand::SetMasterVolume { volume } => {
                audio_state.master_volume = volume.clamp(0.0, 1.0);
            }
            AudioCommand::StopAllSounds => {
                if let Some(entity) = audio_state.current_music.take() {
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}

pub fn update_audio_fades(
    mut commands: Commands,
    time: Res<Time>,
    mut audio_state: ResMut<AudioState>,
    mut audio_sinks: Query<&mut AudioSink>,
) {
    // Extract values needed to avoid borrow conflicts
    let music_entity = audio_state.current_music;
    let master_volume = audio_state.master_volume;

    let mut should_clear_fade = false;
    let mut should_despawn = false;

    if let Some(ref mut fade) = audio_state.fade {
        fade.elapsed += time.delta_secs();
        let t = (fade.elapsed / fade.duration).clamp(0.0, 1.0);
        let volume = fade.start_volume + (fade.target_volume - fade.start_volume) * t;

        if let Some(entity) = music_entity {
            if let Ok(mut sink) = audio_sinks.get_mut(entity) {
                sink.set_volume(bevy::audio::Volume::Linear(volume * master_volume));
            }
        }

        if t >= 1.0 {
            if matches!(fade.fade_type, FadeType::Out) {
                should_despawn = true;
            }
            should_clear_fade = true;
        }
    }

    if should_clear_fade {
        audio_state.fade = None;
    }
    if should_despawn {
        if let Some(entity) = audio_state.current_music.take() {
            commands.entity(entity).despawn();
        }
    }
}

// =============================================================================
// RENDERING COMMANDS
// =============================================================================

pub fn process_rendering_commands(
    mut rendering_queue: ResMut<RenderingCommandQueue>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut point_lights: Query<&mut PointLight>,
    mut spot_lights: Query<&mut SpotLight>,
    mut directional_lights: Query<&mut DirectionalLight>,
    mut visibilities: Query<&mut Visibility>,
    mesh_materials: Query<&MeshMaterial3d<StandardMaterial>>,
) {
    for cmd in rendering_queue.drain() {
        match cmd {
            RenderingCommand::SetMaterialColor { entity, color } => {
                if let Ok(mat_handle) = mesh_materials.get(entity) {
                    if let Some(material) = materials.get_mut(&mat_handle.0) {
                        material.base_color = array_to_color(color);
                    }
                }
            }
            RenderingCommand::SetLightIntensity { entity, intensity } => {
                if let Ok(mut light) = point_lights.get_mut(entity) {
                    light.intensity = intensity;
                } else if let Ok(mut light) = spot_lights.get_mut(entity) {
                    light.intensity = intensity;
                } else if let Ok(mut light) = directional_lights.get_mut(entity) {
                    light.illuminance = intensity;
                }
            }
            RenderingCommand::SetLightColor { entity, color } => {
                let color = array_to_color(color);
                if let Ok(mut light) = point_lights.get_mut(entity) {
                    light.color = color;
                } else if let Ok(mut light) = spot_lights.get_mut(entity) {
                    light.color = color;
                } else if let Ok(mut light) = directional_lights.get_mut(entity) {
                    light.color = color;
                }
            }
            RenderingCommand::SetVisibility { entity, visible } => {
                if let Ok(mut vis) = visibilities.get_mut(entity) {
                    *vis = if visible { Visibility::Inherited } else { Visibility::Hidden };
                }
            }
        }
    }
}

// =============================================================================
// CAMERA COMMANDS
// =============================================================================

pub fn process_camera_commands(
    mut camera_queue: ResMut<CameraCommandQueue>,
    mut camera_state: ResMut<ScriptCameraState>,
) {
    for cmd in camera_queue.drain() {
        match cmd {
            CameraCommand::SetTarget { target } => {
                camera_state.follow_target = target;
            }
            CameraCommand::SetZoom { zoom } => {
                camera_state.zoom = zoom;
            }
            CameraCommand::ScreenShake { intensity, duration } => {
                camera_state.shake_intensity = intensity;
                camera_state.shake_duration = duration;
                camera_state.shake_elapsed = 0.0;
            }
            CameraCommand::SetOffset { offset } => {
                camera_state.offset = offset;
            }
        }
    }
}

pub fn apply_camera_effects(
    time: Res<Time>,
    mut camera_state: ResMut<ScriptCameraState>,
    mut cameras: Query<&mut Transform, With<Camera3d>>,
    targets: Query<&Transform, Without<Camera3d>>,
) {
    if camera_state.shake_duration > 0.0 {
        camera_state.shake_elapsed += time.delta_secs();
        if camera_state.shake_elapsed >= camera_state.shake_duration {
            camera_state.shake_intensity = 0.0;
            camera_state.shake_duration = 0.0;
        }
    }

    for mut camera_transform in cameras.iter_mut() {
        if let Some(target_entity) = camera_state.follow_target {
            if let Ok(target_transform) = targets.get(target_entity) {
                let target_pos = target_transform.translation + camera_state.offset;
                camera_transform.translation = camera_transform.translation.lerp(target_pos, 5.0 * time.delta_secs());
            }
        }

        if camera_state.shake_intensity > 0.0 {
            // Simple pseudo-random shake using time
            let t = time.elapsed_secs() * 50.0;
            let shake = Vec3::new(
                (t.sin() * 1.3 + (t * 2.7).cos()) * camera_state.shake_intensity,
                ((t * 1.7).cos() + (t * 3.1).sin()) * camera_state.shake_intensity,
                0.0,
            );
            camera_transform.translation += shake;
        }
    }
}

// =============================================================================
// ANIMATION COMMANDS
// =============================================================================

pub fn process_animation_commands(
    mut commands: Commands,
    mut animation_queue: ResMut<AnimationCommandQueue>,
    mut players: Query<&mut RuntimeAnimationPlayer>,
) {
    for cmd in animation_queue.drain() {
        match cmd {
            AnimationCommand::Play { entity, clip_name, looping, speed } => {
                if let Ok(mut player) = players.get_mut(entity) {
                    player.current_clip = Some(clip_name);
                    player.looping = looping;
                    player.speed = speed;
                    player.state = AnimationPlaybackState::Playing;
                    player.current_time = 0.0;
                } else {
                    commands.entity(entity).insert(RuntimeAnimationPlayer {
                        current_clip: Some(clip_name),
                        looping,
                        speed,
                        state: AnimationPlaybackState::Playing,
                        current_time: 0.0,
                    });
                }
            }
            AnimationCommand::Stop { entity } => {
                if let Ok(mut player) = players.get_mut(entity) {
                    player.state = AnimationPlaybackState::Stopped;
                    player.current_time = 0.0;
                }
            }
            AnimationCommand::Pause { entity } => {
                if let Ok(mut player) = players.get_mut(entity) {
                    player.state = AnimationPlaybackState::Paused;
                }
            }
            AnimationCommand::Resume { entity } => {
                if let Ok(mut player) = players.get_mut(entity) {
                    player.state = AnimationPlaybackState::Playing;
                }
            }
            AnimationCommand::SetSpeed { entity, speed } => {
                if let Ok(mut player) = players.get_mut(entity) {
                    player.speed = speed;
                }
            }
        }
    }
}

pub fn update_tweens(
    time: Res<Time>,
    mut tweens: ResMut<ActiveTweens>,
    mut transforms: Query<&mut Transform>,
) {
    let dt = time.delta_secs();

    tweens.tweens.retain_mut(|tween| {
        tween.elapsed += dt;
        let t = (tween.elapsed / tween.duration).clamp(0.0, 1.0);
        let eased_t = tween.easing.apply(t);

        if let Ok(mut transform) = transforms.get_mut(tween.entity) {
            match &tween.property {
                TweenProperty::Position { start, end } => {
                    transform.translation = start.lerp(*end, eased_t);
                }
                TweenProperty::Rotation { start, end } => {
                    transform.rotation = start.slerp(*end, eased_t);
                }
                TweenProperty::Scale { start, end } => {
                    transform.scale = start.lerp(*end, eased_t);
                }
            }
        }

        t < 1.0
    });
}

// =============================================================================
// HEALTH COMMANDS
// =============================================================================

pub fn process_health_commands(
    mut commands: Commands,
    mut health_queue: ResMut<HealthCommandQueue>,
    mut health_entities: Query<(Entity, &mut HealthData)>,
) {
    for cmd in health_queue.drain() {
        match cmd {
            HealthCommand::Damage { entity, amount } => {
                if let Ok((_, mut health)) = health_entities.get_mut(entity) {
                    if !health.invincible {
                        health.current_health = (health.current_health - amount).max(0.0);
                        if health.current_health <= 0.0 && health.destroy_on_death {
                            commands.entity(entity).despawn();
                        }
                    }
                }
            }
            HealthCommand::Heal { entity, amount } => {
                if let Ok((_, mut health)) = health_entities.get_mut(entity) {
                    health.current_health = (health.current_health + amount).min(health.max_health);
                }
            }
            HealthCommand::SetHealth { entity, amount } => {
                if let Ok((_, mut health)) = health_entities.get_mut(entity) {
                    health.current_health = amount.clamp(0.0, health.max_health);
                }
            }
            HealthCommand::SetMaxHealth { entity, amount } => {
                if let Ok((_, mut health)) = health_entities.get_mut(entity) {
                    health.max_health = amount;
                    health.current_health = health.current_health.min(amount);
                }
            }
            HealthCommand::SetInvincible { entity, invincible, .. } => {
                if let Ok((_, mut health)) = health_entities.get_mut(entity) {
                    health.invincible = invincible;
                }
            }
            HealthCommand::Kill { entity } => {
                if let Ok((_, mut health)) = health_entities.get_mut(entity) {
                    health.current_health = 0.0;
                    if health.destroy_on_death {
                        commands.entity(entity).despawn();
                    }
                }
            }
            HealthCommand::Revive { entity } => {
                if let Ok((_, mut health)) = health_entities.get_mut(entity) {
                    health.current_health = health.max_health;
                }
            }
        }
    }
}

// =============================================================================
// DEBUG DRAW
// =============================================================================

pub fn tick_debug_draws(time: Res<Time>, mut debug_queue: ResMut<DebugDrawQueue>) {
    debug_queue.tick(time.delta_secs());
}

pub fn render_debug_draws(debug_queue: Res<DebugDrawQueue>, mut gizmos: Gizmos) {
    for cmd in debug_queue.iter() {
        match cmd {
            DebugDrawCommand::Line { start, end, color } => {
                gizmos.line(*start, *end, array_to_color(*color));
            }
            DebugDrawCommand::Sphere { center, radius, color } => {
                gizmos.sphere(Isometry3d::from_translation(*center), *radius, array_to_color(*color));
            }
            DebugDrawCommand::Box { center, half_extents, color } => {
                gizmos.cube(
                    Transform::from_translation(*center).with_scale(*half_extents * 2.0),
                    array_to_color(*color),
                );
            }
            DebugDrawCommand::Ray { origin, direction, color } => {
                gizmos.ray(*origin, *direction, array_to_color(*color));
            }
            DebugDrawCommand::Point { position, color } => {
                gizmos.sphere(Isometry3d::from_translation(*position), 0.05, array_to_color(*color));
            }
        }
    }
}
