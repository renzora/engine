//! Script runtime - executes scripts and applies their commands

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use super::{
    ScriptComponent, ScriptInput,
    ScriptTime, ScriptTransform, RhaiScriptEngine, RhaiScriptContext, ChildNodeInfo,
    RhaiCommand,
};
use super::resources::{
    AnimationCommandQueue, AudioCommand, AudioCommandQueue, CameraCommand, CameraCommandQueue,
    DebugDrawCommand, DebugDrawQueue, EasingFunction, HealthCommand, HealthCommandQueue,
    ParticleScriptCommand, ParticleScriptCommandQueue, PhysicsCommand, PhysicsCommandQueue,
    RaycastHit, RaycastResults, RenderingCommand, RenderingCommandQueue, SceneCommandQueue,
    ScriptCollisionEvents, ScriptTimers, SpriteAnimationCommandQueue, TweenProperty, array_to_color,
};
use crate::component_system::ComponentRegistry;
use crate::core::{DisabledComponents, EditorEntity, EditorSettings, SceneNode, WorldEnvironmentMarker, PlayModeState};
use crate::core::resources::console::{console_log, LogLevel};
use crate::project::CurrentProject;

/// System parameter that bundles all script command queues together
/// This reduces the parameter count for run_rhai_scripts to stay within Bevy's 16-param limit
#[derive(SystemParam)]
pub struct ScriptCommandQueues<'w> {
    pub physics: ResMut<'w, PhysicsCommandQueue>,
    pub timers: ResMut<'w, ScriptTimers>,
    pub debug_draw: ResMut<'w, DebugDrawQueue>,
    pub audio: ResMut<'w, AudioCommandQueue>,
    pub rendering: ResMut<'w, RenderingCommandQueue>,
    pub camera: ResMut<'w, CameraCommandQueue>,
    pub scene: ResMut<'w, SceneCommandQueue>,
    pub animation: ResMut<'w, AnimationCommandQueue>,
    pub health: ResMut<'w, HealthCommandQueue>,
    pub sprite_animation: ResMut<'w, SpriteAnimationCommandQueue>,
    pub particles: ResMut<'w, ParticleScriptCommandQueue>,
    // Assets for primitive spawning
    pub meshes: ResMut<'w, Assets<Mesh>>,
    pub materials: ResMut<'w, Assets<StandardMaterial>>,
    // Collision events for scripts
    pub collisions: Res<'w, ScriptCollisionEvents>,
    // Raycast results from previous frame
    pub raycast_results: ResMut<'w, RaycastResults>,
    // Deferred property writes for cross-entity set() calls
    pub deferred_writes: ResMut<'w, DeferredPropertyWrites>,
}

/// System parameter that bundles component queries for reading entity state
#[derive(SystemParam)]
pub struct ScriptComponentQueries<'w, 's> {
    // Light queries for reading current values
    pub point_lights: Query<'w, 's, &'static PointLight>,
    pub spot_lights: Query<'w, 's, &'static SpotLight>,
    pub directional_lights: Query<'w, 's, &'static DirectionalLight>,
    // Material queries for reading current values
    pub mesh_materials: Query<'w, 's, &'static MeshMaterial3d<StandardMaterial>>,
    // Note: material_assets moved to ScriptCommandQueues as ResMut to avoid conflict
    // Sun data for scripting environment changes
    pub sun_data: Query<'w, 's, &'static mut crate::component_system::SunData>,
    // Pre-computed camera yaw from the active viewport camera
    pub script_camera_yaw: Res<'w, super::api::ScriptCameraYaw>,
    // AnimatorComponent for routing play_animation through
    pub animators: Query<'w, 's, &'static mut crate::animator::AnimatorComponent>,
}

/// Pending parent/child transform changes
struct TransformChange {
    new_position: Option<Vec3>,
    new_rotation: Option<Vec3>,
    translation: Option<Vec3>,
}

/// Marker resource to indicate we're running in standalone runtime mode (no editor)
#[derive(Resource)]
pub struct RuntimeMode;

/// System that runs Rhai file-based scripts
/// Works in both editor (with PlayModeState) and runtime (with RuntimeMode) contexts
pub fn run_rhai_scripts(
    mut commands: Commands,
    time: Res<Time>,
    input: Res<ScriptInput>,
    rhai_engine: Res<RhaiScriptEngine>,
    current_project: Option<Res<CurrentProject>>,
    play_mode: Option<Res<PlayModeState>>,
    editor_settings: Option<Res<EditorSettings>>,
    runtime_mode: Option<Res<RuntimeMode>>,
    mut scripts: Query<(Entity, &mut ScriptComponent, &mut Transform, Option<&ChildOf>, Option<&Children>, Option<&DisabledComponents>)>,
    mut all_transforms: Query<&mut Transform, Without<ScriptComponent>>,
    mut editor_entities: Query<(Entity, &mut EditorEntity)>,
    mut world_environments: Query<(
        &mut WorldEnvironmentMarker,
        Option<&mut crate::component_system::AmbientLightData>,
        Option<&mut crate::component_system::SkyboxData>,
        Option<&mut crate::component_system::FogData>,
        Option<&mut crate::component_system::TonemappingData>,
    )>,
    mut visibility_query: Query<&mut Visibility>,
    // Bundled command queues to stay within 16-param limit
    mut queues: ScriptCommandQueues,
    // Bundled component queries for reading entity state
    mut component_queries: ScriptComponentQueries,
) {
    use std::collections::HashMap;

    // Determine if we should run scripts:
    // - In runtime mode: always run
    // - In editor mode: only run during play mode
    let should_run = if runtime_mode.is_some() {
        true // Runtime mode - always run
    } else if let Some(ref pm) = play_mode {
        pm.is_scripts_running() // Editor mode - check play state
    } else {
        false // No mode resource - don't run
    };

    if !should_run {
        return;
    }

    // In editor mode, require a project. In runtime mode, scripts folder is set externally.
    if runtime_mode.is_none() && current_project.is_none() {
        return;
    }

    let script_time = ScriptTime {
        elapsed: time.elapsed_secs_f64(),
        delta: time.delta_secs(),
        fixed_delta: 1.0 / 60.0,
        frame_count: 0,
    };

    // Collect parent and child changes to apply after the main loop
    let mut parent_changes: HashMap<Entity, TransformChange> = HashMap::new();
    let mut child_changes: HashMap<Entity, TransformChange> = HashMap::new();
    // Collect all Rhai commands to process after the main loop
    let mut all_rhai_commands: Vec<(Entity, RhaiCommand)> = Vec::new();

    // Pre-compute entity lookups for scripts
    // Map of entity name → entity ID
    let mut entities_by_name: HashMap<String, u64> = HashMap::new();
    // Map of tag → list of entity IDs
    let mut entities_by_tag: HashMap<String, Vec<u64>> = HashMap::new();

    // Entity data store is populated by populate_entity_data_store (exclusive system in PreScript)
    for entity_data in editor_entities.iter() {
        let entity_id = entity_data.0.to_bits();
        let editor_entity = entity_data.1;

        // Add to name lookup
        entities_by_name.insert(editor_entity.name.clone(), entity_id);

        // Add to tag lookup (tags are comma-separated)
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

    // Pre-computed camera yaw from the active viewport camera (updated each frame in PreScript)
    let camera_yaw_deg = component_queries.script_camera_yaw.0;

    for (entity, mut script_comp, mut transform, parent_ref, children_ref, dc) in scripts.iter_mut() {
        if dc.map_or(false, |d| d.is_disabled("script")) {
            continue;
        }

        for entry in script_comp.scripts.iter_mut() {
            if !entry.enabled {
                continue;
            }

            // Only process file-based scripts
            let Some(script_path) = &entry.script_path else {
                continue;
            };

            // Resolve relative paths against project directory
            let resolved_path = if script_path.is_relative() {
                if let Some(ref proj) = current_project {
                    proj.path.join(script_path)
                } else {
                    script_path.clone()
                }
            } else {
                script_path.clone()
            };

            // Load/reload the script (supports both .rhai and .blueprint files)
            let compiled = match rhai_engine.load_script_file(&resolved_path) {
                Ok(c) => {
                    // Clear any previous error state
                    if entry.runtime_state.has_error {
                        console_log(LogLevel::Success, "Script", format!("'{}' loaded successfully", c.name));
                        entry.runtime_state.has_error = false;
                    }
                    c
                }
                Err(_e) => {
                    // Error already logged by load_script_file, just mark error state
                    entry.runtime_state.has_error = true;
                    continue;
                }
            };

            // Detect hot-reload: if the script file changed since last run, reset initialized
            let rerun_on_reload = editor_settings.as_ref()
                .map_or(true, |s| s.script_rerun_on_ready_on_reload);
            let script_modified = Some(compiled.last_modified);
            if entry.runtime_state.initialized
                && rerun_on_reload
                && entry.runtime_state.last_script_modified.is_some()
                && entry.runtime_state.last_script_modified != script_modified
            {
                console_log(LogLevel::Info, "Script", format!("Hot-reload detected for '{}', re-running on_ready", compiled.name));
                entry.runtime_state.initialized = false;
            }
            entry.runtime_state.last_script_modified = script_modified;

            // Create Rhai context
            let script_transform = ScriptTransform::from_transform(&transform);
            let mut ctx = RhaiScriptContext::new(script_time, script_transform);

            // Set self entity info
            ctx.self_entity_id = entity.to_bits();
            ctx.self_entity_name = editor_entities
                .get(entity)
                .map(|(_, e)| e.name.clone())
                .unwrap_or_else(|_| format!("Entity_{}", entity.index()));

            // Set entity lookup data (cloned per-script to allow concurrent access)
            ctx.found_entities = entities_by_name.clone();
            ctx.entities_by_tag = entities_by_tag.clone();

            // Set collision data for this entity
            ctx.collisions_entered = queues.collisions.get_collisions_entered(entity)
                .iter().map(|e| e.to_bits()).collect();
            ctx.collisions_exited = queues.collisions.get_collisions_exited(entity)
                .iter().map(|e| e.to_bits()).collect();
            ctx.active_collisions = queues.collisions.get_active_collisions(entity)
                .iter().map(|e| e.to_bits()).collect();

            // Set timer data - get list of timers that just finished
            ctx.timers_just_finished = queues.timers.get_just_finished();

            // Set raycast results for this entity
            // Extract results where the requester entity matches
            for ((req_entity, var_name), hit) in queues.raycast_results.results.iter() {
                if *req_entity == entity {
                    ctx.raycast_results.insert(var_name.clone(), hit.clone());
                }
            }

            ctx.input_movement = input.get_movement_vector();
            ctx.mouse_position = input.mouse_position;
            ctx.mouse_delta = input.mouse_delta;
            ctx.camera_yaw = camera_yaw_deg;

            // Keyboard state
            for (key, &pressed) in &input.keys_pressed {
                if pressed {
                    ctx.keys_pressed.insert(format!("{:?}", key), true);
                }
            }
            for (key, &pressed) in &input.keys_just_pressed {
                if pressed {
                    ctx.keys_just_pressed.insert(format!("{:?}", key), true);
                }
            }
            for (key, &released) in &input.keys_just_released {
                if released {
                    ctx.keys_just_released.insert(format!("{:?}", key), true);
                }
            }

            // Gamepad input (using gamepad 0)
            ctx.gamepad_left_stick = Vec2::new(
                input.get_gamepad_left_stick_x(0),
                input.get_gamepad_left_stick_y(0),
            );
            ctx.gamepad_right_stick = Vec2::new(
                input.get_gamepad_right_stick_x(0),
                input.get_gamepad_right_stick_y(0),
            );
            ctx.gamepad_left_trigger = input.get_gamepad_left_trigger(0);
            ctx.gamepad_right_trigger = input.get_gamepad_right_trigger(0);
            // Map common buttons
            use bevy::input::gamepad::GamepadButton;
            ctx.gamepad_buttons[0] = input.is_gamepad_button_pressed(0, GamepadButton::South); // A
            ctx.gamepad_buttons[1] = input.is_gamepad_button_pressed(0, GamepadButton::East);  // B
            ctx.gamepad_buttons[2] = input.is_gamepad_button_pressed(0, GamepadButton::West);  // X
            ctx.gamepad_buttons[3] = input.is_gamepad_button_pressed(0, GamepadButton::North); // Y
            ctx.gamepad_buttons[4] = input.is_gamepad_button_pressed(0, GamepadButton::LeftTrigger);  // LB
            ctx.gamepad_buttons[5] = input.is_gamepad_button_pressed(0, GamepadButton::RightTrigger); // RB
            ctx.gamepad_buttons[6] = input.is_gamepad_button_pressed(0, GamepadButton::Select);
            ctx.gamepad_buttons[7] = input.is_gamepad_button_pressed(0, GamepadButton::Start);
            ctx.gamepad_buttons[8] = input.is_gamepad_button_pressed(0, GamepadButton::LeftThumb);  // L3
            ctx.gamepad_buttons[9] = input.is_gamepad_button_pressed(0, GamepadButton::RightThumb); // R3
            ctx.gamepad_buttons[10] = input.is_gamepad_button_pressed(0, GamepadButton::DPadUp);
            ctx.gamepad_buttons[11] = input.is_gamepad_button_pressed(0, GamepadButton::DPadDown);
            ctx.gamepad_buttons[12] = input.is_gamepad_button_pressed(0, GamepadButton::DPadLeft);
            ctx.gamepad_buttons[13] = input.is_gamepad_button_pressed(0, GamepadButton::DPadRight);

            // Get parent info if available
            if let Some(child_of) = parent_ref {
                ctx.has_parent = true;
                ctx.parent_entity = Some(child_of.0);
                if let Ok(parent_transform) = all_transforms.get(child_of.0) {
                    ctx.parent_position = parent_transform.translation;
                    let (x, y, z) = parent_transform.rotation.to_euler(EulerRot::XYZ);
                    ctx.parent_rotation = Vec3::new(
                        x.to_degrees(),
                        y.to_degrees(),
                        z.to_degrees(),
                    );
                    ctx.parent_scale = parent_transform.scale;
                }
            }

            // Get children info if available - build name to entity mapping
            let mut child_name_to_entity: HashMap<String, Entity> = HashMap::new();
            if let Some(children) = children_ref {
                for child_entity in children.iter() {
                    if let Ok(child_transform) = all_transforms.get(child_entity) {
                        let child_name = editor_entities
                            .get(child_entity)
                            .map(|(_, e)| e.name.clone())
                            .unwrap_or_else(|_| format!("Entity_{}", child_entity.index()));

                        let (rx, ry, rz) = child_transform.rotation.to_euler(EulerRot::XYZ);
                        ctx.children.push(ChildNodeInfo {
                            entity: child_entity,
                            name: child_name.clone(),
                            position: child_transform.translation,
                            rotation: Vec3::new(rx.to_degrees(), ry.to_degrees(), rz.to_degrees()),
                            scale: child_transform.scale,
                        });
                        child_name_to_entity.insert(child_name, child_entity);
                    }
                }
            }

            // Get light data if this entity has a light component
            if let Ok(light) = component_queries.point_lights.get(entity) {
                ctx.self_light_intensity = light.intensity;
                ctx.self_light_color = [light.color.to_srgba().red, light.color.to_srgba().green, light.color.to_srgba().blue];
            } else if let Ok(light) = component_queries.spot_lights.get(entity) {
                ctx.self_light_intensity = light.intensity;
                ctx.self_light_color = [light.color.to_srgba().red, light.color.to_srgba().green, light.color.to_srgba().blue];
            } else if let Ok(light) = component_queries.directional_lights.get(entity) {
                ctx.self_light_intensity = light.illuminance;
                ctx.self_light_color = [light.color.to_srgba().red, light.color.to_srgba().green, light.color.to_srgba().blue];
            }

            // Get material color if this entity has a mesh material
            if let Ok(material_handle) = component_queries.mesh_materials.get(entity) {
                if let Some(material) = queues.materials.get(&material_handle.0) {
                    let color = material.base_color.to_srgba();
                    ctx.self_material_color = [color.red, color.green, color.blue, color.alpha];
                }
            }

            // Set hierarchy context for entity()/parent()/child()/children() functions
            let children_ids: Vec<u64> = children_ref
                .map(|c| c.iter().map(|e| e.to_bits()).collect())
                .unwrap_or_default();
            super::entity_data_store::set_hierarchy_context(
                super::entity_data_store::HierarchyContext {
                    self_entity_id: entity.to_bits(),
                    parent_entity_id: parent_ref.map(|p| p.0.to_bits()),
                    children_entity_ids: children_ids,
                }
            );

            // Call on_ready if not initialized
            if !entry.runtime_state.initialized {
                console_log(LogLevel::Info, "Script", format!("Initializing '{}'", compiled.name));
                rhai_engine.call_on_ready(&compiled, &mut ctx, &mut entry.variables);
                entry.runtime_state.initialized = true;
            }

            // Call on_update
            rhai_engine.call_on_update(&compiled, &mut ctx, &mut entry.variables);

            // Clear hierarchy context
            super::entity_data_store::clear_hierarchy_context();

            // Apply transform results to self
            if let Some(pos) = ctx.new_position {
                transform.translation = pos;
            }

            if let Some(rot) = ctx.new_rotation {
                transform.rotation = Quat::from_euler(
                    EulerRot::YXZ,
                    rot.y.to_radians(),
                    rot.x.to_radians(),
                    rot.z.to_radians(),
                );
            }

            // Apply rotation delta (degrees per frame)
            if let Some(rot_delta) = ctx.rotation_delta {
                let delta_quat = Quat::from_euler(
                    EulerRot::XYZ,
                    rot_delta.x.to_radians(),
                    rot_delta.y.to_radians(),
                    rot_delta.z.to_radians(),
                );
                transform.rotation = delta_quat * transform.rotation;
            }

            if let Some(delta) = ctx.translation {
                if delta.length_squared() > 0.0001 {
                    info!("[Rhai] Translating by {:?}", delta);
                }
                transform.translation += delta;
            }

            if let Some(msg) = ctx.print_message {
                console_log(LogLevel::Info, "Script", msg);
            }

            // Collect parent transform changes
            if let Some(parent_entity) = ctx.parent_entity {
                let has_parent_changes = ctx.parent_new_position.is_some()
                    || ctx.parent_new_rotation.is_some()
                    || ctx.parent_translation.is_some();

                if has_parent_changes {
                    parent_changes.insert(parent_entity, TransformChange {
                        new_position: ctx.parent_new_position,
                        new_rotation: ctx.parent_new_rotation,
                        translation: ctx.parent_translation,
                    });
                }
            }

            // Collect child transform changes
            for (child_name, change) in &ctx.child_changes {
                if let Some(&child_entity) = child_name_to_entity.get(child_name) {
                    child_changes.insert(child_entity, TransformChange {
                        new_position: change.new_position,
                        new_rotation: change.new_rotation,
                        translation: change.translation,
                    });
                }
            }

            // Apply environment changes (to all WorldEnvironmentMarker entities)
            let has_env_changes = ctx.env_sky_mode.is_some()
                || ctx.env_clear_color.is_some()
                || ctx.env_ambient_brightness.is_some()
                || ctx.env_ambient_color.is_some()
                || ctx.env_ev100.is_some()
                || ctx.env_sky_top_color.is_some()
                || ctx.env_sky_horizon_color.is_some()
                || ctx.env_sky_curve.is_some()
                || ctx.env_ground_bottom_color.is_some()
                || ctx.env_ground_horizon_color.is_some()
                || ctx.env_ground_curve.is_some()
                || ctx.env_fog_enabled.is_some()
                || ctx.env_fog_color.is_some()
                || ctx.env_fog_start.is_some()
                || ctx.env_fog_end.is_some();

            if has_env_changes {
                use crate::component_system::SkyMode;
                for (_world_env, ambient_opt, skybox_opt, fog_opt, tm_opt) in world_environments.iter_mut() {
                    // Ambient light
                    if let Some(mut ambient) = ambient_opt {
                        if let Some(brightness) = ctx.env_ambient_brightness {
                            ambient.brightness = brightness;
                        }
                        if let Some((r, g, b)) = ctx.env_ambient_color {
                            ambient.color = (r, g, b);
                        }
                    }

                    // Sky/Skybox changes
                    if let Some(mut skybox) = skybox_opt {
                        if let Some(mode) = ctx.env_sky_mode {
                            skybox.sky_mode = match mode {
                                0 => SkyMode::Color,
                                1 => SkyMode::Procedural,
                                2 => SkyMode::Panorama,
                                _ => SkyMode::Procedural,
                            };
                        }
                        if let Some((r, g, b)) = ctx.env_clear_color {
                            skybox.clear_color = (r, g, b);
                        }
                        // Procedural Sky
                        if let Some((r, g, b)) = ctx.env_sky_top_color {
                            skybox.procedural_sky.sky_top_color = (r, g, b);
                        }
                        if let Some((r, g, b)) = ctx.env_sky_horizon_color {
                            skybox.procedural_sky.sky_horizon_color = (r, g, b);
                        }
                        if let Some(curve) = ctx.env_sky_curve {
                            skybox.procedural_sky.sky_curve = curve;
                        }
                        if let Some((r, g, b)) = ctx.env_ground_bottom_color {
                            skybox.procedural_sky.ground_bottom_color = (r, g, b);
                        }
                        if let Some((r, g, b)) = ctx.env_ground_horizon_color {
                            skybox.procedural_sky.ground_horizon_color = (r, g, b);
                        }
                        if let Some(curve) = ctx.env_ground_curve {
                            skybox.procedural_sky.ground_curve = curve;
                        }
                    }

                    // Fog changes
                    if let Some(mut fog) = fog_opt {
                        if let Some(enabled) = ctx.env_fog_enabled {
                            fog.enabled = enabled;
                        }
                        if let Some((r, g, b)) = ctx.env_fog_color {
                            fog.color = (r, g, b);
                        }
                        if let Some(start) = ctx.env_fog_start {
                            fog.start = start;
                        }
                        if let Some(end) = ctx.env_fog_end {
                            fog.end = end;
                        }
                    }

                    // Tonemapping/exposure changes
                    if let Some(mut tm) = tm_opt {
                        if let Some(ev100) = ctx.env_ev100 {
                            tm.ev100 = ev100;
                        }
                    }
                }
            }

            // Apply sun changes to SunData components
            let has_sun_changes = ctx.env_sun_azimuth.is_some()
                || ctx.env_sun_elevation.is_some()
                || ctx.env_sun_color.is_some()
                || ctx.env_sun_energy.is_some()
                || ctx.env_sun_disk_scale.is_some();

            if has_sun_changes {
                for mut sun in component_queries.sun_data.iter_mut() {
                    if let Some(azimuth) = ctx.env_sun_azimuth {
                        sun.azimuth = azimuth;
                    }
                    if let Some(elevation) = ctx.env_sun_elevation {
                        sun.elevation = elevation;
                    }
                    if let Some((r, g, b)) = ctx.env_sun_color {
                        sun.color = bevy::math::Vec3::new(r, g, b);
                    }
                    if let Some(energy) = ctx.env_sun_energy {
                        sun.illuminance = energy * 10000.0;
                    }
                    if let Some(scale) = ctx.env_sun_disk_scale {
                        sun.angular_diameter = scale;
                    }
                }
            }

            // Collect Rhai commands for processing after the loop
            for cmd in ctx.commands.drain(..) {
                all_rhai_commands.push((entity, cmd));
            }
        }
    }

    // Apply collected parent changes
    for (parent_entity, change) in parent_changes {
        if let Ok(mut parent_transform) = all_transforms.get_mut(parent_entity) {
            if let Some(pos) = change.new_position {
                parent_transform.translation = pos;
            }
            if let Some(rot) = change.new_rotation {
                parent_transform.rotation = Quat::from_euler(
                    EulerRot::YXZ,
                    rot.y.to_radians(),
                    rot.x.to_radians(),
                    rot.z.to_radians(),
                );
            }
            if let Some(delta) = change.translation {
                parent_transform.translation += delta;
            }
        }
    }

    // Apply collected child changes
    for (child_entity, change) in child_changes {
        if let Ok(mut child_transform) = all_transforms.get_mut(child_entity) {
            if let Some(pos) = change.new_position {
                child_transform.translation = pos;
            }
            if let Some(rot) = change.new_rotation {
                child_transform.rotation = Quat::from_euler(
                    EulerRot::YXZ,
                    rot.y.to_radians(),
                    rot.x.to_radians(),
                    rot.z.to_radians(),
                );
            }
            if let Some(delta) = change.translation {
                child_transform.translation += delta;
            }
        }
    }

    // Process collected Rhai commands
    for (source_entity, cmd) in all_rhai_commands {
        // Route SetProperty to deferred writes resource
        if let RhaiCommand::SetProperty { entity_id, property, value } = cmd {
            queues.deferred_writes.writes.push((entity_id, property, value));
            continue;
        }
        process_rhai_command(
            &mut commands,
            &mut visibility_query,
            &mut editor_entities,
            &mut queues.physics,
            &mut queues.timers,
            &mut queues.debug_draw,
            &mut queues.audio,
            &mut queues.rendering,
            &mut queues.camera,
            &mut queues.scene,
            &mut queues.animation,
            &mut queues.health,
            &mut queues.sprite_animation,
            &mut queues.particles,
            &mut queues.meshes,
            &mut queues.materials,
            &mut component_queries.animators,
            source_entity,
            cmd,
        );
    }
}

/// Process a single Rhai command
fn process_rhai_command(
    commands: &mut Commands,
    visibility_query: &mut Query<&mut Visibility>,
    editor_entities: &mut Query<(Entity, &mut EditorEntity)>,
    physics_queue: &mut ResMut<PhysicsCommandQueue>,
    timers: &mut ResMut<ScriptTimers>,
    debug_draws: &mut ResMut<DebugDrawQueue>,
    audio_queue: &mut ResMut<AudioCommandQueue>,
    rendering_queue: &mut ResMut<RenderingCommandQueue>,
    camera_queue: &mut ResMut<CameraCommandQueue>,
    scene: &mut ResMut<SceneCommandQueue>,
    animation_queue: &mut ResMut<AnimationCommandQueue>,
    health_queue: &mut ResMut<HealthCommandQueue>,
    sprite_animation_queue: &mut ResMut<SpriteAnimationCommandQueue>,
    particle_queue: &mut ResMut<ParticleScriptCommandQueue>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    animator_components: &mut Query<&mut crate::animator::AnimatorComponent>,
    source_entity: Entity,
    cmd: RhaiCommand,
) {
    match cmd {
        // ECS Commands
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
        RhaiCommand::SpawnPrimitive { name, primitive_type, position, scale } => {
            // Create mesh based on primitive type
            let mesh_handle: Handle<Mesh> = match primitive_type.as_str() {
                "cube" => meshes.add(Cuboid::default()),
                "sphere" => meshes.add(Sphere::default()),
                "plane" => meshes.add(Plane3d::default().mesh().size(10.0, 10.0)),
                "cylinder" => meshes.add(Cylinder::default()),
                "capsule" => meshes.add(Capsule3d::default()),
                _ => {
                    warn!("[Rhai] Unknown primitive type: {}", primitive_type);
                    return;
                }
            };

            // Create default material (white)
            let material_handle = materials.add(StandardMaterial {
                base_color: Color::WHITE,
                ..default()
            });

            // Build transform
            let mut transform = Transform::default();
            if let Some(pos) = position {
                transform.translation = pos;
            }
            if let Some(s) = scale {
                transform.scale = s;
            }

            // Note: RaytracingMesh3d is managed by sync_rendering_settings based on Solari state
            commands.spawn((
                transform,
                Visibility::default(),
                Mesh3d(mesh_handle),
                MeshMaterial3d(material_handle),
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
            let entity = Entity::from_bits(entity_id);
            commands.entity(entity).despawn();
        }
        RhaiCommand::DespawnSelf => {
            commands.entity(source_entity).despawn();
        }
        RhaiCommand::SetEntityName { entity_id, name } => {
            let entity = Entity::from_bits(entity_id);
            commands.entity(entity).insert(EditorEntity {
                name,
                tag: String::new(),
                visible: true,
                locked: false,
            });
        }
        RhaiCommand::AddTag { entity_id, tag } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            // Find and modify the EditorEntity
            if let Some((_, mut editor_entity)) = editor_entities.iter_mut().find(|(e, _)| *e == entity) {
                // Check if tag already exists (tags are comma-separated)
                let existing_tags: Vec<&str> = editor_entity.tag.split(',')
                    .map(|t| t.trim())
                    .filter(|t| !t.is_empty())
                    .collect();

                if !existing_tags.contains(&tag.as_str()) {
                    // Add the new tag
                    if editor_entity.tag.is_empty() {
                        editor_entity.tag = tag.clone();
                    } else {
                        editor_entity.tag = format!("{}, {}", editor_entity.tag, tag);
                    }
                    debug!("[Rhai] Added tag '{}' to entity {:?}", tag, entity);
                } else {
                    debug!("[Rhai] Entity {:?} already has tag '{}'", entity, tag);
                }
            } else {
                warn!("[Rhai] AddTag: entity {:?} not found or has no EditorEntity component", entity);
            }
        }
        RhaiCommand::RemoveTag { entity_id, tag } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            // Find and modify the EditorEntity
            if let Some((_, mut editor_entity)) = editor_entities.iter_mut().find(|(e, _)| *e == entity) {
                // Remove the tag from comma-separated list
                let new_tags: Vec<&str> = editor_entity.tag.split(',')
                    .map(|t| t.trim())
                    .filter(|t| !t.is_empty() && *t != tag.as_str())
                    .collect();
                editor_entity.tag = new_tags.join(", ");
                debug!("[Rhai] Removed tag '{}' from entity {:?}", tag, entity);
            } else {
                warn!("[Rhai] RemoveTag: entity {:?} not found or has no EditorEntity component", entity);
            }
        }

        // Audio Commands - queue for processing by audio system
        RhaiCommand::PlaySound { path, volume, looping, bus } => {
            audio_queue.push(AudioCommand::PlaySound { path, volume, looping, bus, entity: Some(source_entity) });
        }
        RhaiCommand::PlaySound3D { path, volume, position, bus } => {
            audio_queue.push(AudioCommand::PlaySound3D { path, volume, position, bus, entity: Some(source_entity) });
        }
        RhaiCommand::PlayMusic { path, volume, fade_in, bus } => {
            audio_queue.push(AudioCommand::PlayMusic { path, volume, fade_in, bus });
        }
        RhaiCommand::StopMusic { fade_out } => {
            audio_queue.push(AudioCommand::StopMusic { fade_out });
        }
        RhaiCommand::StopAllSounds => {
            audio_queue.push(AudioCommand::StopAllSounds);
        }
        RhaiCommand::SetMasterVolume { volume } => {
            audio_queue.push(AudioCommand::SetMasterVolume { volume });
        }
        RhaiCommand::PauseSound => {
            audio_queue.push(AudioCommand::PauseSound { entity: Some(source_entity) });
        }
        RhaiCommand::PauseSoundEntity { entity_id } => {
            audio_queue.push(AudioCommand::PauseSound { entity: Some(Entity::from_bits(entity_id)) });
        }
        RhaiCommand::ResumeSound => {
            audio_queue.push(AudioCommand::ResumeSound { entity: Some(source_entity) });
        }
        RhaiCommand::ResumeSoundEntity { entity_id } => {
            audio_queue.push(AudioCommand::ResumeSound { entity: Some(Entity::from_bits(entity_id)) });
        }
        RhaiCommand::SetSoundVolume { volume, fade } => {
            audio_queue.push(AudioCommand::SetSoundVolume { entity: source_entity, volume, fade });
        }
        RhaiCommand::SetSoundVolumeEntity { entity_id, volume, fade } => {
            audio_queue.push(AudioCommand::SetSoundVolume { entity: Entity::from_bits(entity_id), volume, fade });
        }
        RhaiCommand::SetSoundPitch { pitch, fade } => {
            audio_queue.push(AudioCommand::SetSoundPitch { entity: source_entity, pitch, fade });
        }
        RhaiCommand::SetSoundPitchEntity { entity_id, pitch, fade } => {
            audio_queue.push(AudioCommand::SetSoundPitch { entity: Entity::from_bits(entity_id), pitch, fade });
        }
        RhaiCommand::CrossfadeMusic { path, volume, duration, bus } => {
            audio_queue.push(AudioCommand::CrossfadeMusic { path, volume, duration, bus });
        }

        // Debug Commands
        RhaiCommand::Log { level, message } => {
            let log_level = match level.as_str() {
                "error" => LogLevel::Error,
                "warn" => LogLevel::Warning,
                "debug" | "trace" => LogLevel::Info,
                _ => LogLevel::Info,
            };
            console_log(log_level, "Script", message);
        }
        RhaiCommand::DrawLine { start, end, color, duration } => {
            debug_draws.push(DebugDrawCommand::Line {
                start,
                end,
                color: array_to_color(color),
                duration,
            });
        }
        RhaiCommand::DrawRay { origin, direction, length, color, duration } => {
            debug_draws.push(DebugDrawCommand::Ray {
                origin,
                direction,
                length,
                color: array_to_color(color),
                duration,
            });
        }
        RhaiCommand::DrawSphere { center, radius, color, duration } => {
            debug_draws.push(DebugDrawCommand::Sphere {
                center,
                radius,
                color: array_to_color(color),
                duration,
            });
        }
        RhaiCommand::DrawBox { center, half_extents, color, duration } => {
            debug_draws.push(DebugDrawCommand::Box {
                center,
                half_extents,
                color: array_to_color(color),
                duration,
            });
        }
        RhaiCommand::DrawPoint { position, size, color, duration } => {
            debug_draws.push(DebugDrawCommand::Point {
                position,
                size,
                color: array_to_color(color),
                duration,
            });
        }

        // Physics Commands - queue for processing by physics system
        RhaiCommand::ApplyForce { entity_id, force } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            physics_queue.push(PhysicsCommand::ApplyForce { entity, force });
        }
        RhaiCommand::ApplyImpulse { entity_id, impulse } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            physics_queue.push(PhysicsCommand::ApplyImpulse { entity, impulse });
        }
        RhaiCommand::ApplyTorque { entity_id, torque } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            physics_queue.push(PhysicsCommand::ApplyTorque { entity, torque });
        }
        RhaiCommand::SetVelocity { entity_id, velocity } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            physics_queue.push(PhysicsCommand::SetVelocity { entity, velocity });
        }
        RhaiCommand::SetAngularVelocity { entity_id, velocity } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            physics_queue.push(PhysicsCommand::SetAngularVelocity { entity, velocity });
        }
        RhaiCommand::SetGravityScale { entity_id, scale } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            physics_queue.push(PhysicsCommand::SetGravityScale { entity, scale });
        }
        RhaiCommand::Raycast { origin, direction, max_distance, result_var } => {
            physics_queue.push(PhysicsCommand::Raycast {
                origin,
                direction,
                max_distance,
                requester: source_entity,
                result_var,
            });
        }

        // Timer Commands - process immediately
        RhaiCommand::StartTimer { name, duration, repeat } => {
            timers.start(name, duration, repeat);
        }
        RhaiCommand::StopTimer { name } => {
            timers.stop(&name);
        }
        RhaiCommand::PauseTimer { name } => {
            timers.pause(&name);
        }
        RhaiCommand::ResumeTimer { name } => {
            timers.resume(&name);
        }

        // Scene Commands
        RhaiCommand::LoadScene { path } => {
            scene.load_scene(&path);
            info!("[Rhai] LoadScene '{}' queued", path);
        }
        RhaiCommand::UnloadScene { handle_id } => {
            if handle_id == 0 {
                // Unload all runtime prefabs
                scene.unload_all_prefabs();
                info!("[Rhai] UnloadScene: despawning all runtime prefabs");
            } else {
                // Unload specific entity
                if let Some(entity) = Entity::try_from_bits(handle_id) {
                    scene.unload_entity(entity);
                    info!("[Rhai] UnloadScene: despawning entity {:?}", entity);
                } else {
                    warn!("[Rhai] UnloadScene: invalid entity id {}", handle_id);
                }
            }
        }
        RhaiCommand::SpawnPrefab { path, position, rotation } => {
            scene.spawn_prefab(&path, position, rotation);
            info!("[Rhai] SpawnPrefab '{}' at {:?} rotation={:?} queued", path, position, rotation);
        }

        // Animation Commands
        RhaiCommand::PlayAnimation { entity_id, name, looping: _, speed: _ } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            // Check if entity has AnimatorComponent — if so, route through it
            if let Ok(mut animator) = animator_components.get_mut(entity) {
                animator.current_clip = if name.is_empty() { None } else { Some(name.clone()) };
                info!("[Rhai] PlayAnimation (AnimatorComponent) '{}' on {:?}", name, entity);
            } else {
                animation_queue.play(entity, name.clone(), true, 1.0);
                info!("[Rhai] PlayAnimation '{}' on {:?}", name, entity);
            }
        }
        RhaiCommand::StopAnimation { entity_id } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            animation_queue.stop(entity);
            info!("[Rhai] StopAnimation on {:?}", entity);
        }
        RhaiCommand::PauseAnimation { entity_id } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            animation_queue.pause(entity);
            info!("[Rhai] PauseAnimation on {:?}", entity);
        }
        RhaiCommand::ResumeAnimation { entity_id } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            animation_queue.resume(entity);
            info!("[Rhai] ResumeAnimation on {:?}", entity);
        }
        RhaiCommand::SetAnimationSpeed { entity_id, speed } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            animation_queue.set_speed(entity, speed);
            info!("[Rhai] SetAnimationSpeed {} on {:?}", speed, entity);
        }

        // Sprite Animation Commands
        RhaiCommand::PlaySpriteAnimation { entity_id, name, looping } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            sprite_animation_queue.play(entity, name.clone(), looping);
            debug!("[Rhai] PlaySpriteAnimation '{}' on {:?} looping={}", name, entity, looping);
        }
        RhaiCommand::SetSpriteFrame { entity_id, frame } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            sprite_animation_queue.set_absolute_frame(entity, frame as usize);
            debug!("[Rhai] SetSpriteFrame {} on {:?}", frame, entity);
        }

        // Tween Commands
        RhaiCommand::Tween { entity_id, property, target, duration, easing } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            if let Some(prop) = TweenProperty::from_str(&property) {
                let easing_fn = EasingFunction::from_str(&easing);
                animation_queue.tween(entity, prop, target, duration, easing_fn);
                info!("[Rhai] Tween {:?} to {} over {}s on {:?}", prop, target, duration, entity);
            } else {
                warn!("[Rhai] Unknown tween property: {}", property);
            }
        }
        RhaiCommand::TweenPosition { entity_id, target, duration, easing } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            let easing_fn = EasingFunction::from_str(&easing);
            animation_queue.tween_position(entity, target, duration, easing_fn);
            info!("[Rhai] TweenPosition to {:?} over {}s on {:?}", target, duration, entity);
        }
        RhaiCommand::TweenRotation { entity_id, target, duration, easing } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            let easing_fn = EasingFunction::from_str(&easing);
            animation_queue.tween_rotation(entity, target, duration, easing_fn);
            info!("[Rhai] TweenRotation to {:?} over {}s on {:?}", target, duration, entity);
        }
        RhaiCommand::TweenScale { entity_id, target, duration, easing } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            let easing_fn = EasingFunction::from_str(&easing);
            animation_queue.tween_scale(entity, target, duration, easing_fn);
            info!("[Rhai] TweenScale to {:?} over {}s on {:?}", target, duration, entity);
        }

        // Rendering Commands
        RhaiCommand::SetVisibility { entity_id, visible } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            if let Ok(mut vis) = visibility_query.get_mut(entity) {
                *vis = if visible { Visibility::Inherited } else { Visibility::Hidden };
            }
        }
        RhaiCommand::SetMaterialColor { entity_id, color } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            rendering_queue.push(RenderingCommand::SetMaterialColor { entity, color });
        }
        RhaiCommand::SetLightIntensity { entity_id, intensity } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            rendering_queue.push(RenderingCommand::SetLightIntensity { entity, intensity });
        }
        RhaiCommand::SetLightColor { entity_id, color } => {
            let entity = entity_id
                .map(|id| Entity::from_bits(id))
                .unwrap_or(source_entity);
            rendering_queue.push(RenderingCommand::SetLightColor { entity, color });
        }

        // Camera Commands - queue for processing by camera system
        RhaiCommand::SetCameraTarget { position } => {
            camera_queue.push(CameraCommand::SetTarget { position });
        }
        RhaiCommand::SetCameraZoom { zoom } => {
            camera_queue.push(CameraCommand::SetZoom { zoom });
        }
        RhaiCommand::ScreenShake { intensity, duration } => {
            camera_queue.push(CameraCommand::ScreenShake { intensity, duration });
        }
        RhaiCommand::CameraFollow { entity_id, offset, smoothing } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                camera_queue.push(CameraCommand::FollowEntity { entity, offset, smoothing });
            } else {
                warn!("[Script] CameraFollow: invalid entity id {}", entity_id);
            }
        }
        RhaiCommand::StopCameraFollow => {
            camera_queue.push(CameraCommand::StopFollow);
        }

        // Component Commands - Health
        RhaiCommand::SetHealth { entity_id, value } => {
            let entity = entity_id
                .and_then(|id| Entity::try_from_bits(id))
                .unwrap_or(source_entity);
            health_queue.push(HealthCommand::SetHealth { entity, value });
        }
        RhaiCommand::SetMaxHealth { entity_id, value } => {
            let entity = entity_id
                .and_then(|id| Entity::try_from_bits(id))
                .unwrap_or(source_entity);
            health_queue.push(HealthCommand::SetMaxHealth { entity, value });
        }
        RhaiCommand::Damage { entity_id, amount } => {
            let entity = entity_id
                .and_then(|id| Entity::try_from_bits(id))
                .unwrap_or(source_entity);
            health_queue.push(HealthCommand::Damage { entity, amount });
        }
        RhaiCommand::Heal { entity_id, amount } => {
            let entity = entity_id
                .and_then(|id| Entity::try_from_bits(id))
                .unwrap_or(source_entity);
            health_queue.push(HealthCommand::Heal { entity, amount });
        }
        RhaiCommand::SetInvincible { entity_id, invincible, duration } => {
            let entity = entity_id
                .and_then(|id| Entity::try_from_bits(id))
                .unwrap_or(source_entity);
            health_queue.push(HealthCommand::SetInvincible { entity, invincible, duration });
        }
        RhaiCommand::Kill { entity_id } => {
            let entity = entity_id
                .and_then(|id| Entity::try_from_bits(id))
                .unwrap_or(source_entity);
            health_queue.push(HealthCommand::Kill { entity });
        }
        RhaiCommand::Revive { entity_id } => {
            let entity = entity_id
                .and_then(|id| Entity::try_from_bits(id))
                .unwrap_or(source_entity);
            health_queue.push(HealthCommand::Revive { entity });
        }
        RhaiCommand::SetComponentField { entity_id, component_type, field_name, value } => {
            let entity = entity_id
                .and_then(|id| Entity::try_from_bits(id))
                .unwrap_or(source_entity);
            // Generic component field setting - log for now, would need reflection system
            debug!("[Script] SetComponentField({:?}, {}, {}) = {:?}", entity, component_type, field_name, value);
        }

        // Particle Commands
        RhaiCommand::ParticlePlay { entity_id } => {
            particle_queue.push(ParticleScriptCommand::Play { entity_id });
        }
        RhaiCommand::ParticlePause { entity_id } => {
            particle_queue.push(ParticleScriptCommand::Pause { entity_id });
        }
        RhaiCommand::ParticleStop { entity_id } => {
            particle_queue.push(ParticleScriptCommand::Stop { entity_id });
        }
        RhaiCommand::ParticleReset { entity_id } => {
            particle_queue.push(ParticleScriptCommand::Reset { entity_id });
        }
        RhaiCommand::ParticleBurst { entity_id, count } => {
            particle_queue.push(ParticleScriptCommand::Burst { entity_id, count });
        }
        RhaiCommand::ParticleSetRate { entity_id, multiplier } => {
            particle_queue.push(ParticleScriptCommand::SetRate { entity_id, multiplier });
        }
        RhaiCommand::ParticleSetScale { entity_id, multiplier } => {
            particle_queue.push(ParticleScriptCommand::SetScale { entity_id, multiplier });
        }
        RhaiCommand::ParticleSetTimeScale { entity_id, scale } => {
            particle_queue.push(ParticleScriptCommand::SetTimeScale { entity_id, scale });
        }
        RhaiCommand::ParticleSetTint { entity_id, r, g, b, a } => {
            particle_queue.push(ParticleScriptCommand::SetTint { entity_id, r, g, b, a });
        }
        RhaiCommand::ParticleSetVariableFloat { entity_id, name, value } => {
            particle_queue.push(ParticleScriptCommand::SetVariableFloat { entity_id, name, value });
        }
        RhaiCommand::ParticleSetVariableColor { entity_id, name, r, g, b, a } => {
            particle_queue.push(ParticleScriptCommand::SetVariableColor { entity_id, name, r, g, b, a });
        }
        RhaiCommand::ParticleSetVariableVec3 { entity_id, name, x, y, z } => {
            particle_queue.push(ParticleScriptCommand::SetVariableVec3 { entity_id, name, x, y, z });
        }
        RhaiCommand::ParticleEmitAt { entity_id, x, y, z, count } => {
            particle_queue.push(ParticleScriptCommand::EmitAt { entity_id, x, y, z, count });
        }

        // Transform, environment, and property commands are consumed by
        // process_command_buffer() in rhai_engine.rs and should never reach here.
        RhaiCommand::SetPosition { .. }
        | RhaiCommand::SetRotation { .. }
        | RhaiCommand::SetScale { .. }
        | RhaiCommand::Translate { .. }
        | RhaiCommand::Rotate { .. }
        | RhaiCommand::LookAt { .. }
        | RhaiCommand::ParentSetPosition { .. }
        | RhaiCommand::ParentSetRotation { .. }
        | RhaiCommand::ParentTranslate { .. }
        | RhaiCommand::ChildSetPosition { .. }
        | RhaiCommand::ChildSetRotation { .. }
        | RhaiCommand::ChildTranslate { .. }
        | RhaiCommand::SetSunAngles { .. }
        | RhaiCommand::SetAmbientBrightness { .. }
        | RhaiCommand::SetAmbientColor { .. }
        | RhaiCommand::SetSkyTopColor { .. }
        | RhaiCommand::SetSkyHorizonColor { .. }
        | RhaiCommand::SetFog { .. }
        | RhaiCommand::SetFogColor { .. }
        | RhaiCommand::SetEv100 { .. } => {
            // These are consumed by process_command_buffer() and should not reach here
        }
        RhaiCommand::SetProperty { .. } => {
            // SetProperty should not reach here — it's consumed by process_command_buffer()
            // and routed to DeferredPropertyWrites
        }
    }
}

// =============================================================================
// Registry-based entity data store population
// =============================================================================

use crate::component_system::PropertyValue;

/// Convert a PropertyValue to a Rhai Dynamic
fn property_value_to_dynamic(pv: &PropertyValue) -> rhai::Dynamic {
    match pv {
        PropertyValue::Float(v) => rhai::Dynamic::from(*v as f64),
        PropertyValue::Int(v) => rhai::Dynamic::from(*v as i64),
        PropertyValue::Bool(v) => rhai::Dynamic::from(*v),
        PropertyValue::String(v) => rhai::Dynamic::from(v.clone()),
        PropertyValue::Vec2(v) => {
            let mut map = rhai::Map::new();
            map.insert("x".into(), rhai::Dynamic::from(v.x as f64));
            map.insert("y".into(), rhai::Dynamic::from(v.y as f64));
            rhai::Dynamic::from(map)
        }
        PropertyValue::Vec3(v) => {
            let mut map = rhai::Map::new();
            map.insert("x".into(), rhai::Dynamic::from(v.x as f64));
            map.insert("y".into(), rhai::Dynamic::from(v.y as f64));
            map.insert("z".into(), rhai::Dynamic::from(v.z as f64));
            rhai::Dynamic::from(map)
        }
        PropertyValue::Color(v) => {
            let mut map = rhai::Map::new();
            map.insert("r".into(), rhai::Dynamic::from(v.x as f64));
            map.insert("g".into(), rhai::Dynamic::from(v.y as f64));
            map.insert("b".into(), rhai::Dynamic::from(v.z as f64));
            map.insert("a".into(), rhai::Dynamic::from(v.w as f64));
            rhai::Dynamic::from(map)
        }
    }
}

/// Exclusive system that populates the entity data store with all entity properties.
/// Runs in PreScript before script execution.
/// Uses registry get_script_properties_fn for component-specific properties.
pub fn populate_entity_data_store(world: &mut World) {
    use crate::core::PlayModeState;

    // Check if we should run (play mode or runtime mode)
    let should_run = if world.get_resource::<RuntimeMode>().is_some() {
        true
    } else if let Some(pm) = world.get_resource::<PlayModeState>() {
        pm.is_scripts_running()
    } else {
        false
    };

    if !should_run {
        return;
    }

    // Clear entity data store for this frame
    super::entity_data_store::clear_store();

    // Populate audio playing entities for is_sound_playing() queries
    {
        let playing_entities: std::collections::HashSet<u64> = world
            .get_non_send_resource::<crate::audio::KiraAudioManager>()
            .map(|audio| audio.active_sounds.keys().map(|e| e.to_bits()).collect())
            .unwrap_or_default();
        super::rhai_api::set_audio_playing_entities(playing_entities);
    }

    // Collect get_script_properties functions from registry
    let get_fns: Vec<crate::component_system::GetScriptPropertiesFn> = {
        let Some(registry) = world.get_resource::<ComponentRegistry>() else {
            return;
        };
        registry.all()
            .filter_map(|def| def.get_script_properties_fn)
            .collect()
    };

    // Collect all editor entities with their data
    let mut entity_data_list: Vec<(Entity, String)> = Vec::new();
    {
        let mut query = world.query::<(Entity, &EditorEntity)>();
        for (entity, editor_entity) in query.iter(world) {
            entity_data_list.push((entity, editor_entity.name.clone()));
        }
    }

    // For each entity, collect transform + registry properties
    for (entity, name) in &entity_data_list {
        let entity_id = entity.to_bits();
        let mut props = super::entity_data_store::EntityProperties::new();
        props.insert("name".to_string(), rhai::Dynamic::from(name.clone()));
        props.insert("entity_id".to_string(), rhai::Dynamic::from(entity_id as i64));

        // Transform properties
        if let Some(t) = world.get::<Transform>(*entity) {
            props.insert("position_x".to_string(), rhai::Dynamic::from(t.translation.x as f64));
            props.insert("position_y".to_string(), rhai::Dynamic::from(t.translation.y as f64));
            props.insert("position_z".to_string(), rhai::Dynamic::from(t.translation.z as f64));
            let (rx, ry, rz) = t.rotation.to_euler(EulerRot::XYZ);
            props.insert("rotation_x".to_string(), rhai::Dynamic::from(rx.to_degrees() as f64));
            props.insert("rotation_y".to_string(), rhai::Dynamic::from(ry.to_degrees() as f64));
            props.insert("rotation_z".to_string(), rhai::Dynamic::from(rz.to_degrees() as f64));
            props.insert("scale_x".to_string(), rhai::Dynamic::from(t.scale.x as f64));
            props.insert("scale_y".to_string(), rhai::Dynamic::from(t.scale.y as f64));
            props.insert("scale_z".to_string(), rhai::Dynamic::from(t.scale.z as f64));
        }

        // Registry component properties
        for get_fn in &get_fns {
            for (key, value) in get_fn(world, *entity) {
                props.insert(key.to_string(), property_value_to_dynamic(&value));
            }
        }

        super::entity_data_store::insert_entity(entity_id, name, props);
    }
}

// =============================================================================
// Deferred property writes (for cross-entity set() calls)
// =============================================================================

/// Resource that buffers property writes from set() calls for exclusive system processing
#[derive(Resource, Default)]
pub struct DeferredPropertyWrites {
    pub writes: Vec<(u64, String, PropertyValue)>, // (entity_id, property_name, value)
}

/// Exclusive system that applies deferred property writes using the component registry.
/// Runs after run_rhai_scripts in the CommandProcessing set.
pub fn apply_deferred_property_writes(world: &mut World) {
    // Drain the writes buffer
    let writes = {
        let Some(mut deferred) = world.get_resource_mut::<DeferredPropertyWrites>() else {
            return;
        };
        std::mem::take(&mut deferred.writes)
    };

    if writes.is_empty() {
        return;
    }

    // Collect set functions from registry (fn pointers are Copy — no borrow conflict)
    let set_fns: Vec<crate::component_system::SetScriptPropertyFn> = {
        let Some(registry) = world.get_resource::<ComponentRegistry>() else {
            return;
        };
        registry.all()
            .filter_map(|def| def.set_script_property_fn)
            .collect()
    };

    for (entity_id, property, value) in writes {
        let Some(entity) = Entity::try_from_bits(entity_id) else {
            continue;
        };

        // Try transform properties first
        if try_set_transform_property(world, entity, &property, &value) {
            continue;
        }

        // Try script variables
        if try_set_script_variable(world, entity, &property, &value) {
            continue;
        }

        // Try component properties via registry
        let mut handled = false;
        for set_fn in &set_fns {
            if set_fn(world, entity, &property, &value) {
                handled = true;
                break;
            }
        }

        if !handled {
            debug!("[Script] SetProperty: unhandled property '{}' on entity {:?}", property, entity);
        }
    }
}

/// Try to set a transform property on an entity. Returns true if handled.
fn try_set_transform_property(world: &mut World, entity: Entity, property: &str, value: &PropertyValue) -> bool {
    let PropertyValue::Float(v) = value else {
        return false;
    };

    let Some(mut transform) = world.get_mut::<Transform>(entity) else {
        return false;
    };

    match property {
        "position_x" => { transform.translation.x = *v; true }
        "position_y" => { transform.translation.y = *v; true }
        "position_z" => { transform.translation.z = *v; true }
        "rotation_x" => {
            let (_, ry, rz) = transform.rotation.to_euler(EulerRot::XYZ);
            transform.rotation = Quat::from_euler(EulerRot::XYZ, v.to_radians(), ry, rz);
            true
        }
        "rotation_y" => {
            let (rx, _, rz) = transform.rotation.to_euler(EulerRot::XYZ);
            transform.rotation = Quat::from_euler(EulerRot::XYZ, rx, v.to_radians(), rz);
            true
        }
        "rotation_z" => {
            let (rx, ry, _) = transform.rotation.to_euler(EulerRot::XYZ);
            transform.rotation = Quat::from_euler(EulerRot::XYZ, rx, ry, v.to_radians());
            true
        }
        "scale_x" => { transform.scale.x = *v; true }
        "scale_y" => { transform.scale.y = *v; true }
        "scale_z" => { transform.scale.z = *v; true }
        _ => false,
    }
}

/// Try to set a script variable on an entity. Returns true if handled.
fn try_set_script_variable(world: &mut World, entity: Entity, property: &str, value: &PropertyValue) -> bool {
    use super::ScriptValue;

    let Some(mut script_comp) = world.get_mut::<ScriptComponent>(entity) else {
        return false;
    };

    // Convert PropertyValue to ScriptValue
    let script_val = match value {
        PropertyValue::Float(v) => ScriptValue::Float(*v),
        PropertyValue::Int(v) => ScriptValue::Int(*v),
        PropertyValue::Bool(v) => ScriptValue::Bool(*v),
        PropertyValue::String(v) => ScriptValue::String(v.clone()),
        PropertyValue::Vec2(v) => ScriptValue::Vec2(*v),
        PropertyValue::Vec3(v) => ScriptValue::Vec3(*v),
        PropertyValue::Color(v) => ScriptValue::Color(*v),
    };

    // Try each script entry's variables — only update if the variable already exists
    for entry in script_comp.scripts.iter_mut() {
        if entry.variables.get(property).is_some() {
            entry.variables.set(property, script_val);
            return true;
        }
    }

    false
}
