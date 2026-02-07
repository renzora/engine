//! Runtime Script Executor

use bevy::prelude::*;
use bevy::ecs::system::SystemParam;
use std::collections::HashMap;

use super::engine::RuntimeScriptEngine;
use super::context::{RhaiScriptContext, ScriptTime, ScriptTransform};
use super::commands::RhaiCommand;
use super::resources::*;
use crate::shared::{EditorEntity, SceneNode, ScriptComponent};

/// System parameter that bundles all script command queues together
#[derive(SystemParam)]
pub struct ScriptCommandQueues<'w> {
    pub physics: ResMut<'w, PhysicsCommandQueue>,
    pub timers: ResMut<'w, ScriptTimers>,
    pub debug_draw: ResMut<'w, DebugDrawQueue>,
    pub audio: ResMut<'w, AudioCommandQueue>,
    pub rendering: ResMut<'w, RenderingCommandQueue>,
    pub camera: ResMut<'w, CameraCommandQueue>,
    pub animation: ResMut<'w, AnimationCommandQueue>,
    pub health: ResMut<'w, HealthCommandQueue>,
}

/// System parameter that bundles read-only script resources
#[derive(SystemParam)]
pub struct ScriptReadResources<'w> {
    pub collisions: Res<'w, ScriptCollisionEvents>,
    pub raycast_results: Res<'w, RaycastResults>,
    pub input: Res<'w, ScriptInput>,
}

/// System that runs Rhai scripts on all entities with ScriptComponent
pub fn run_runtime_scripts(
    mut commands: Commands,
    time: Res<Time>,
    engine: Res<RuntimeScriptEngine>,
    mut scripts: Query<(Entity, &mut ScriptComponent, &mut Transform)>,
    mut all_transforms: Query<&mut Transform, Without<ScriptComponent>>,
    editor_entities: Query<(Entity, &EditorEntity)>,
    mut queues: ScriptCommandQueues,
    read_res: ScriptReadResources,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let script_time = ScriptTime {
        elapsed: time.elapsed_secs_f64(),
        delta: time.delta_secs(),
        fixed_delta: 1.0 / 60.0,
        frame_count: 0,
    };

    // Pre-compute entity lookups
    let mut entities_by_name: HashMap<String, u64> = HashMap::new();
    let mut entities_by_tag: HashMap<String, Vec<u64>> = HashMap::new();

    for (entity, editor_entity) in editor_entities.iter() {
        let entity_id = entity.to_bits();
        entities_by_name.insert(editor_entity.name.clone(), entity_id);

        if !editor_entity.tag.is_empty() {
            for tag in editor_entity.tag.split(',') {
                let tag = tag.trim();
                if !tag.is_empty() {
                    entities_by_tag
                        .entry(tag.to_string())
                        .or_insert_with(Vec::new)
                        .push(entity_id);
                }
            }
        }
    }

    let mut all_commands: Vec<(Entity, RhaiCommand)> = Vec::new();

    for (entity, mut script_comp, mut transform) in scripts.iter_mut() {
        if !script_comp.enabled {
            continue;
        }

        let Some(script_path) = &script_comp.script_path else {
            continue;
        };

        let compiled = match engine.load_script_file(script_path) {
            Ok(c) => {
                if script_comp.runtime_state.has_error {
                    info!("[Script] '{}' loaded successfully", c.name);
                    script_comp.runtime_state.has_error = false;
                }
                c
            }
            Err(e) => {
                if !script_comp.runtime_state.has_error {
                    error!("[Script] Failed to load '{}': {}", script_path, e);
                    script_comp.runtime_state.has_error = true;
                }
                continue;
            }
        };

        let script_transform = ScriptTransform::from_transform(&transform);
        let mut ctx = RhaiScriptContext::new(script_time, script_transform);

        ctx.self_entity_id = entity.to_bits();
        ctx.self_entity_name = editor_entities
            .get(entity)
            .map(|(_, e)| e.name.clone())
            .unwrap_or_else(|_| format!("Entity_{}", entity.index()));

        ctx.found_entities = entities_by_name.clone();
        ctx.entities_by_tag = entities_by_tag.clone();

        ctx.collisions_entered = read_res.collisions.get_collisions_entered(entity)
            .iter().map(|e| e.to_bits()).collect();
        ctx.collisions_exited = read_res.collisions.get_collisions_exited(entity)
            .iter().map(|e| e.to_bits()).collect();
        ctx.active_collisions = read_res.collisions.get_active_collisions(entity)
            .iter().map(|e| e.to_bits()).collect();

        ctx.timers_just_finished = queues.timers.get_just_finished();

        for ((req_entity, var_name), hit) in read_res.raycast_results.results.iter() {
            if *req_entity == entity {
                ctx.raycast_results.insert(var_name.clone(), hit.clone());
            }
        }

        ctx.input_movement = read_res.input.get_movement_vector();
        ctx.mouse_position = read_res.input.mouse_position;
        ctx.mouse_delta = read_res.input.mouse_delta;
        ctx.gamepad_left_stick = Vec2::new(
            read_res.input.get_gamepad_left_stick_x(0),
            read_res.input.get_gamepad_left_stick_y(0),
        );
        ctx.gamepad_right_stick = Vec2::new(
            read_res.input.get_gamepad_right_stick_x(0),
            read_res.input.get_gamepad_right_stick_y(0),
        );

        if !script_comp.runtime_state.initialized {
            info!("[Script] Initializing '{}'", compiled.name);
            if let Err(e) = engine.run_on_ready(&compiled, &mut ctx, &script_comp.variables) {
                error!("[Script] on_ready error in '{}': {}", compiled.name, e);
                script_comp.runtime_state.has_error = true;
                continue;
            }
            script_comp.runtime_state.initialized = true;
        }

        if let Err(e) = engine.run_on_update(&compiled, &mut ctx, &script_comp.variables) {
            error!("[Script] on_update error in '{}': {}", compiled.name, e);
            script_comp.runtime_state.has_error = true;
            continue;
        }

        if let Some(new_pos) = ctx.new_position {
            transform.translation = new_pos;
        }
        if let Some(new_rot) = ctx.new_rotation {
            transform.rotation = new_rot;
        }
        if let Some(new_scale) = ctx.new_scale {
            transform.scale = new_scale;
        }

        for cmd in ctx.commands.drain(..) {
            all_commands.push((entity, cmd));
        }
    }

    // Process commands
    for (source_entity, cmd) in all_commands {
        process_command(
            &mut commands,
            cmd,
            source_entity,
            &mut queues,
            &mut all_transforms,
            &mut meshes,
            &mut materials,
        );
    }
}

fn process_command(
    commands: &mut Commands,
    cmd: RhaiCommand,
    source_entity: Entity,
    queues: &mut ScriptCommandQueues,
    all_transforms: &mut Query<&mut Transform, Without<ScriptComponent>>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    match cmd {
        RhaiCommand::Log { message } => {
            info!("[Script] {}", message);
        }
        RhaiCommand::SpawnEntity { name } => {
            commands.spawn((
                Transform::default(),
                Visibility::default(),
                EditorEntity {
                    name,
                    tag: String::new(),
                    visible: true,
                    locked: false,
                },
                SceneNode,
            ));
        }
        RhaiCommand::DespawnEntity { entity_id } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                commands.entity(entity).despawn();
            }
        }
        RhaiCommand::SpawnPrimitive { name, primitive_type, position } => {
            let mesh = match primitive_type.as_str() {
                "cube" => meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
                "sphere" => meshes.add(Sphere::new(0.5)),
                "plane" => meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(1.0))),
                "cylinder" => meshes.add(Cylinder::new(0.5, 1.0)),
                _ => meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            };
            let material = materials.add(StandardMaterial::default());
            let pos = position.unwrap_or(Vec3::ZERO);

            // Note: RaytracingMesh3d is managed by sync_rendering_settings based on Solari state
            commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_translation(pos),
                Visibility::default(),
                EditorEntity { name, tag: String::new(), visible: true, locked: false },
                SceneNode,
            ));
        }
        RhaiCommand::SetPosition { entity_id, position } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                if let Ok(mut transform) = all_transforms.get_mut(entity) {
                    transform.translation = position;
                }
            }
        }
        RhaiCommand::SetRotation { entity_id, rotation } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                if let Ok(mut transform) = all_transforms.get_mut(entity) {
                    transform.rotation = rotation;
                }
            }
        }
        RhaiCommand::SetScale { entity_id, scale } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                if let Ok(mut transform) = all_transforms.get_mut(entity) {
                    transform.scale = scale;
                }
            }
        }
        RhaiCommand::Translate { entity_id, delta } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                if let Ok(mut transform) = all_transforms.get_mut(entity) {
                    transform.translation += delta;
                }
            }
        }
        RhaiCommand::Rotate { entity_id, rotation } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                if let Ok(mut transform) = all_transforms.get_mut(entity) {
                    transform.rotation = rotation * transform.rotation;
                }
            }
        }
        RhaiCommand::LookAt { entity_id, target } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                if let Ok(mut transform) = all_transforms.get_mut(entity) {
                    transform.look_at(target, Vec3::Y);
                }
            }
        }
        RhaiCommand::ApplyForce { entity_id, force } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                queues.physics.push(PhysicsCommand::ApplyForce { entity, force });
            }
        }
        RhaiCommand::ApplyImpulse { entity_id, impulse } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                queues.physics.push(PhysicsCommand::ApplyImpulse { entity, impulse });
            }
        }
        RhaiCommand::ApplyTorque { entity_id, torque } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                queues.physics.push(PhysicsCommand::ApplyTorque { entity, torque });
            }
        }
        RhaiCommand::SetVelocity { entity_id, velocity } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                queues.physics.push(PhysicsCommand::SetVelocity { entity, velocity });
            }
        }
        RhaiCommand::SetAngularVelocity { entity_id, velocity } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                queues.physics.push(PhysicsCommand::SetAngularVelocity { entity, velocity });
            }
        }
        RhaiCommand::SetGravityScale { entity_id, scale } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                queues.physics.push(PhysicsCommand::SetGravityScale { entity, scale });
            }
        }
        RhaiCommand::Raycast { origin, direction, max_distance, result_var } => {
            queues.physics.push(PhysicsCommand::Raycast {
                origin, direction, max_distance, requester_entity: source_entity, result_var,
            });
        }
        RhaiCommand::PlaySound { path, volume, looping } => {
            queues.audio.push(AudioCommand::PlaySound { path, volume, looping });
        }
        RhaiCommand::PlaySound3D { path, position, volume, looping } => {
            queues.audio.push(AudioCommand::PlaySound3D { path, position, volume, looping });
        }
        RhaiCommand::PlayMusic { path, volume, fade_in } => {
            queues.audio.push(AudioCommand::PlayMusic { path, volume, fade_in });
        }
        RhaiCommand::StopMusic { fade_out } => {
            queues.audio.push(AudioCommand::StopMusic { fade_out });
        }
        RhaiCommand::SetMasterVolume { volume } => {
            queues.audio.push(AudioCommand::SetMasterVolume { volume });
        }
        RhaiCommand::StopAllSounds => {
            queues.audio.push(AudioCommand::StopAllSounds);
        }
        RhaiCommand::StartTimer { name, duration, repeat } => {
            queues.timers.start(name, duration, repeat);
        }
        RhaiCommand::StopTimer { name } => {
            queues.timers.stop(&name);
        }
        RhaiCommand::PauseTimer { name } => {
            queues.timers.pause(&name);
        }
        RhaiCommand::ResumeTimer { name } => {
            queues.timers.resume(&name);
        }
        RhaiCommand::DrawLine { start, end, color, duration } => {
            queues.debug_draw.push(DebugDrawCommand::Line { start, end, color }, duration);
        }
        RhaiCommand::DrawSphere { center, radius, color, duration } => {
            queues.debug_draw.push(DebugDrawCommand::Sphere { center, radius, color }, duration);
        }
        RhaiCommand::DrawBox { center, half_extents, color, duration } => {
            queues.debug_draw.push(DebugDrawCommand::Box { center, half_extents, color }, duration);
        }
        RhaiCommand::DrawRay { origin, direction, color, duration } => {
            queues.debug_draw.push(DebugDrawCommand::Ray { origin, direction, color }, duration);
        }
        RhaiCommand::DrawPoint { position, color, duration } => {
            queues.debug_draw.push(DebugDrawCommand::Point { position, color }, duration);
        }
        RhaiCommand::SetMaterialColor { entity_id, color } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                queues.rendering.push(RenderingCommand::SetMaterialColor { entity, color });
            }
        }
        RhaiCommand::SetLightIntensity { entity_id, intensity } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                queues.rendering.push(RenderingCommand::SetLightIntensity { entity, intensity });
            }
        }
        RhaiCommand::SetLightColor { entity_id, color } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                queues.rendering.push(RenderingCommand::SetLightColor { entity, color });
            }
        }
        RhaiCommand::SetVisibility { entity_id, visible } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                queues.rendering.push(RenderingCommand::SetVisibility { entity, visible });
            }
        }
        RhaiCommand::SetCameraTarget { target_entity_id } => {
            let target = target_entity_id.and_then(|id| Entity::try_from_bits(id));
            queues.camera.push(CameraCommand::SetTarget { target });
        }
        RhaiCommand::SetCameraZoom { zoom } => {
            queues.camera.push(CameraCommand::SetZoom { zoom });
        }
        RhaiCommand::ScreenShake { intensity, duration } => {
            queues.camera.push(CameraCommand::ScreenShake { intensity, duration });
        }
        RhaiCommand::SetCameraOffset { offset } => {
            queues.camera.push(CameraCommand::SetOffset { offset });
        }
        RhaiCommand::PlayAnimation { entity_id, clip_name, looping, speed } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                queues.animation.push(AnimationCommand::Play { entity, clip_name, looping, speed });
            }
        }
        RhaiCommand::StopAnimation { entity_id } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                queues.animation.push(AnimationCommand::Stop { entity });
            }
        }
        RhaiCommand::PauseAnimation { entity_id } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                queues.animation.push(AnimationCommand::Pause { entity });
            }
        }
        RhaiCommand::ResumeAnimation { entity_id } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                queues.animation.push(AnimationCommand::Resume { entity });
            }
        }
        RhaiCommand::SetAnimationSpeed { entity_id, speed } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                queues.animation.push(AnimationCommand::SetSpeed { entity, speed });
            }
        }
        RhaiCommand::Damage { entity_id, amount } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                queues.health.push(HealthCommand::Damage { entity, amount });
            }
        }
        RhaiCommand::Heal { entity_id, amount } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                queues.health.push(HealthCommand::Heal { entity, amount });
            }
        }
        RhaiCommand::SetHealth { entity_id, amount } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                queues.health.push(HealthCommand::SetHealth { entity, amount });
            }
        }
        RhaiCommand::SetMaxHealth { entity_id, amount } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                queues.health.push(HealthCommand::SetMaxHealth { entity, amount });
            }
        }
        RhaiCommand::SetInvincible { entity_id, invincible, duration } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                queues.health.push(HealthCommand::SetInvincible { entity, invincible, duration });
            }
        }
        RhaiCommand::Kill { entity_id } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                queues.health.push(HealthCommand::Kill { entity });
            }
        }
        RhaiCommand::Revive { entity_id } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                queues.health.push(HealthCommand::Revive { entity });
            }
        }
        // Unimplemented commands - log and skip
        _ => {
            info!("[Script] Command not implemented in runtime: {:?}", cmd);
        }
    }
}
