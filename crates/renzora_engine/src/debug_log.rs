//! Rendering debug logger — extensive console logging for cameras,
//! render targets, play mode, post-processing, scene loading, and rehydration.
//!
//! Enable by inserting the [`RenderingDebugLog`] resource with `enabled: true`.
//! All output goes to the editor console panel via `renzora::console_log`.

use bevy::ecs::world::FilteredEntityRef;
use bevy::prelude::*;
use renzora::console_log::*;
use renzora::{
    DefaultCamera, EditorCamera, EffectRouting, IsolatedCamera, PlayModeCamera, PlayModeState,
    PlayState, SceneCamera, ViewportRenderTarget,
};

// ---------------------------------------------------------------------------
// Toggle resource
// ---------------------------------------------------------------------------

/// Insert this resource to enable/disable the rendering debug logger.
#[derive(Resource)]
pub struct RenderingDebugLog {
    pub enabled: bool,
}

impl Default for RenderingDebugLog {
    fn default() -> Self {
        Self { enabled: true }
    }
}

fn is_enabled(debug: &Option<Res<RenderingDebugLog>>) -> bool {
    debug.as_ref().is_some_and(|d| d.enabled)
}

// ---------------------------------------------------------------------------
// Camera state logging
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
struct CamSnap {
    entity: Entity,
    name: String,
    active: bool,
    order: isize,
    has_cam3d: bool,
    role: &'static str,
    has_default: bool,
}

/// Logs every camera entity whenever any camera state changes.
pub fn debug_log_cameras(
    cameras: Query<(
        Entity,
        &Camera,
        Option<&Name>,
        Option<&Camera3d>,
        Option<&EditorCamera>,
        Option<&SceneCamera>,
        Option<&PlayModeCamera>,
        Option<&DefaultCamera>,
    )>,
    mut prev: Local<Vec<CamSnap>>,
    debug: Option<Res<RenderingDebugLog>>,
) {
    if !is_enabled(&debug) {
        return;
    }

    let current: Vec<CamSnap> = cameras
        .iter()
        .map(
            |(entity, cam, name, cam3d, editor, scene, play, default_cam)| {
                let role = if editor.is_some() {
                    "Editor"
                } else if play.is_some() {
                    "PlayMode"
                } else if scene.is_some() {
                    "Scene"
                } else {
                    "Other"
                };
                CamSnap {
                    entity,
                    name: name.map(|n| n.to_string()).unwrap_or_else(|| "unnamed".into()),
                    active: cam.is_active,
                    order: cam.order as isize,
                    has_cam3d: cam3d.is_some(),
                    role,
                    has_default: default_cam.is_some(),
                }
            },
        )
        .collect();

    if current != *prev {
        console_info(
            "Camera",
            format!("--- Camera state changed ({} cameras) ---", current.len()),
        );
        for snap in &current {
            console_info(
                "Camera",
                format!(
                    "  {:?} \"{}\" role={} active={} order={} cam3d={} default={}",
                    snap.entity,
                    snap.name,
                    snap.role,
                    snap.active,
                    snap.order,
                    snap.has_cam3d,
                    snap.has_default,
                ),
            );
        }
        if current.is_empty() {
            console_warn("Camera", "No camera entities found!");
        }
        *prev = current;
    }
}

// ---------------------------------------------------------------------------
// Effect routing logging
// ---------------------------------------------------------------------------

/// Logs whenever the EffectRouting resource changes.
pub fn debug_log_effect_routing(
    routing: Res<EffectRouting>,
    cameras: Query<(Option<&Name>, Option<&EditorCamera>, Option<&SceneCamera>, Option<&PlayModeCamera>)>,
    mut prev_len: Local<usize>,
    debug: Option<Res<RenderingDebugLog>>,
) {
    if !is_enabled(&debug) {
        return;
    }
    if !routing.is_changed() {
        return;
    }

    let describe = |entity: Entity| -> String {
        if let Ok((name, editor, scene, play)) = cameras.get(entity) {
            let role = if editor.is_some() {
                "Editor"
            } else if play.is_some() {
                "PlayMode"
            } else if scene.is_some() {
                "Scene"
            } else {
                "Other"
            };
            let n = name.map(|n| n.to_string()).unwrap_or_else(|| "unnamed".into());
            format!("{:?} \"{}\" ({})", entity, n, role)
        } else {
            format!("{:?}", entity)
        }
    };

    console_info(
        "EffectRouting",
        format!("--- Routing updated ({} routes) ---", routing.routes.len()),
    );
    for (target, sources) in routing.iter() {
        console_info(
            "EffectRouting",
            format!(
                "  target {} <- {} sources",
                describe(*target),
                sources.len()
            ),
        );
    }

    *prev_len = routing.routes.len();
}

// ---------------------------------------------------------------------------
// Play mode logging
// ---------------------------------------------------------------------------

/// Logs play mode state transitions.
pub fn debug_log_play_mode(
    play_mode: Option<Res<PlayModeState>>,
    mut prev_state: Local<Option<PlayState>>,
    mut prev_camera: Local<Option<Entity>>,
    cameras: Query<Option<&Name>>,
    debug: Option<Res<RenderingDebugLog>>,
) {
    if !is_enabled(&debug) {
        return;
    }
    let Some(pm) = play_mode else {
        return;
    };

    let state = pm.state;
    let cam = pm.active_game_camera;

    if Some(state) != *prev_state {
        let state_str = match state {
            PlayState::Editing => "Editing",
            PlayState::Playing => "Playing",
            PlayState::Paused => "Paused",
            PlayState::ScriptsOnly => "ScriptsOnly",
            PlayState::ScriptsPaused => "ScriptsPaused",
        };
        console_info("PlayMode", format!("State -> {}", state_str));
        *prev_state = Some(state);
    }

    if cam != *prev_camera {
        match cam {
            Some(e) => {
                let name = cameras
                    .get(e)
                    .ok()
                    .flatten()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "unnamed".into());
                console_info(
                    "PlayMode",
                    format!("Active game camera -> {:?} \"{}\"", e, name),
                );
            }
            None => {
                console_info("PlayMode", "Active game camera -> None");
            }
        }
        *prev_camera = cam;
    }
}

// ---------------------------------------------------------------------------
// Viewport render target logging
// ---------------------------------------------------------------------------

/// Logs changes to the ViewportRenderTarget resource.
pub fn debug_log_viewport_render_target(
    vrt: Option<Res<ViewportRenderTarget>>,
    mut prev_has_image: Local<Option<bool>>,
    debug: Option<Res<RenderingDebugLog>>,
) {
    if !is_enabled(&debug) {
        return;
    }
    let Some(vrt) = vrt else {
        return;
    };
    if !vrt.is_changed() {
        return;
    }

    let has = vrt.image.is_some();
    if Some(has) != *prev_has_image {
        if has {
            console_info("RenderTarget", "ViewportRenderTarget: image handle set");
        } else {
            console_info("RenderTarget", "ViewportRenderTarget: image handle cleared");
        }
        *prev_has_image = Some(has);
    }
}

// ---------------------------------------------------------------------------
// Post-processing effects logging (reflection-based)
// ---------------------------------------------------------------------------

/// Logs which *Settings components are present on camera entities and
/// the render target entity, using reflection. Runs as an exclusive system
/// so it can access the type registry.
pub fn debug_log_post_processing(world: &mut World) {
    // Check enabled
    let enabled = world
        .get_resource::<RenderingDebugLog>()
        .is_some_and(|d| d.enabled);
    if !enabled {
        return;
    }

    let routing = world.get_resource::<EffectRouting>();
    let route_targets: Vec<Entity> = routing
        .map(|r| r.routes.iter().map(|(t, _)| *t).collect())
        .unwrap_or_default();

    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let registry = type_registry.read();

    // Collect all entities that have at least one *Settings component
    // or are camera entities.
    let mut camera_entities: Vec<Entity> = Vec::new();
    let mut q = world.query_filtered::<Entity, Or<(With<EditorCamera>, With<SceneCamera>, With<PlayModeCamera>)>>();
    for e in q.iter(world) {
        camera_entities.push(e);
    }

    // Collect isolated camera entities (e.g. Camera Preview)
    let mut isolated_entities: Vec<Entity> = Vec::new();
    let mut q = world.query_filtered::<Entity, With<IsolatedCamera>>();
    for e in q.iter(world) {
        isolated_entities.push(e);
    }

    // Also include routing targets if not already in list
    for &rt in &route_targets {
        if !camera_entities.contains(&rt) {
            camera_entities.push(rt);
        }
    }

    // Find all *Settings type registrations
    let settings_types: Vec<_> = registry
        .iter()
        .filter_map(|reg| {
            let type_path = reg.type_info().type_path();
            if type_path.ends_with("Settings") {
                reg.data::<bevy::ecs::reflect::ReflectComponent>()
                    .map(|rc| (type_path, rc.clone()))
            } else {
                None
            }
        })
        .collect();

    // Also find known native Bevy effect components (non-Settings)
    let native_effect_names: &[&str] = &[
        "Bloom", "Atmosphere", "DistanceFog", "DepthOfField", "MotionBlur",
        "Fxaa", "Smaa", "TemporalAntiAliasing", "ContrastAdaptiveSharpening",
        "ScreenSpaceAmbientOcclusion", "ScreenSpaceReflections",
        "Tonemapping", "DebandDither", "AutoExposure",
    ];
    let native_types: Vec<_> = registry
        .iter()
        .filter_map(|reg| {
            let type_path = reg.type_info().type_path();
            let short = type_path.rsplit("::").next().unwrap_or(type_path);
            if native_effect_names.contains(&short) {
                reg.data::<bevy::ecs::reflect::ReflectComponent>()
                    .map(|rc| (type_path, rc.clone()))
            } else {
                None
            }
        })
        .collect();

    // Build current snapshot: for each entity of interest, which settings are present
    let mut snapshot: Vec<(Entity, Vec<&str>)> = Vec::new();

    // Check camera entities
    for &entity in &camera_entities {
        let Ok(entity_ref) = world.get_entity(entity) else { continue };
        let mut effects: Vec<&str> = Vec::new();

        for (type_path, reflect_component) in &settings_types {
            if reflect_component.contains(FilteredEntityRef::from(entity_ref)) {
                effects.push(type_path);
            }
        }

        if !effects.is_empty() {
            snapshot.push((entity, effects));
        }
    }

    // Check isolated camera entities (preview cameras etc.)
    for &entity in &isolated_entities {
        if camera_entities.contains(&entity) {
            continue;
        }
        let Ok(entity_ref) = world.get_entity(entity) else { continue };
        let mut effects: Vec<&str> = Vec::new();
        for (type_path, reflect_component) in &settings_types {
            if reflect_component.contains(FilteredEntityRef::from(entity_ref)) {
                effects.push(type_path);
            }
        }
        if !effects.is_empty() {
            snapshot.push((entity, effects));
        }
    }

    // Also check non-camera entities that have settings components
    // (these are the source entities for proxy, e.g. World Environment)
    let mut all_q = world.query_filtered::<Entity, With<Name>>();
    let all_entities: Vec<Entity> = all_q.iter(world).collect();
    for entity in &all_entities {
        if camera_entities.contains(entity) || isolated_entities.contains(entity) {
            continue;
        }
        let Ok(entity_ref) = world.get_entity(*entity) else { continue };
        let mut effects: Vec<&str> = Vec::new();
        for (type_path, reflect_component) in &settings_types {
            if reflect_component.contains(FilteredEntityRef::from(entity_ref)) {
                effects.push(type_path);
            }
        }
        if !effects.is_empty() {
            snapshot.push((*entity, effects));
        }
    }

    drop(registry);

    // Compare with previous snapshot
    let mut state = world
        .remove_resource::<PostProcessDebugState>()
        .unwrap_or_default();

    // Convert to comparable format
    let current: Vec<(Entity, Vec<String>)> = snapshot
        .iter()
        .map(|(e, effects)| (*e, effects.iter().map(|s| s.to_string()).collect()))
        .collect();

    if current != state.prev_effects {
        console_info(
            "PostProcess",
            "--- Post-processing component map changed ---",
        );

        for &rt in &route_targets {
            let rt_name = world
                .get::<Name>(rt)
                .map(|n| n.to_string())
                .unwrap_or_else(|| "unnamed".into());

            // Check which native Bevy effect components are present on the route target
            let Ok(rt_ref) = world.get_entity(rt) else { continue };
            let mut active_native: Vec<&str> = Vec::new();
            for (type_path, reflect_component) in &native_types {
                if reflect_component.contains(FilteredEntityRef::from(rt_ref)) {
                    active_native.push(
                        type_path.rsplit("::").next().unwrap_or(type_path),
                    );
                }
            }

            if active_native.is_empty() {
                console_warn(
                    "PostProcess",
                    format!(
                        "  Route target: {:?} \"{}\" — NO native effects active!",
                        rt, rt_name
                    ),
                );
            } else {
                console_info(
                    "PostProcess",
                    format!(
                        "  Route target: {:?} \"{}\" native=[{}]",
                        rt, rt_name, active_native.join(", ")
                    ),
                );
            }
        }
        if route_targets.is_empty() {
            console_warn("PostProcess", "  No routing targets — no camera will receive native effects!");
        }

        // Track effect types seen on non-isolated source entities to detect duplicates
        let mut effect_sources: Vec<(&str, Entity)> = Vec::new();

        for (entity, effects) in &current {
            let name = world
                .get::<Name>(*entity)
                .map(|n| n.to_string())
                .unwrap_or_else(|| "unnamed".into());
            let is_camera = camera_entities.contains(entity);
            let is_isolated = isolated_entities.contains(entity);
            let is_rt = route_targets.contains(entity);

            let role = if is_rt {
                "[ROUTE TARGET]"
            } else if is_isolated {
                "[ISOLATED]"
            } else if is_camera {
                "[CAMERA]"
            } else {
                "[SOURCE]"
            };

            // Shorten type paths for readability
            let short_effects: Vec<String> = effects
                .iter()
                .map(|e| {
                    e.rsplit("::")
                        .next()
                        .unwrap_or(e.as_str())
                        .to_string()
                })
                .collect();

            console_info(
                "PostProcess",
                format!(
                    "  {:?} \"{}\" {} -> [{}]",
                    entity,
                    name,
                    role,
                    short_effects.join(", ")
                ),
            );

            // Collect sources for duplicate detection (skip isolated and render target)
            if !is_isolated && !is_rt {
                for effect in effects {
                    effect_sources.push((effect.as_str(), *entity));
                }
            }
        }

        // Warn about duplicate effect sources (same effect type on multiple non-isolated entities)
        let mut seen_effects: Vec<(&str, Entity)> = Vec::new();
        for (effect, entity) in &effect_sources {
            if let Some((_, prev_entity)) = seen_effects.iter().find(|(e, _)| e == effect) {
                if prev_entity != entity {
                    let short = effect.rsplit("::").next().unwrap_or(effect);
                    let name_a = world.get::<Name>(*prev_entity).map(|n| n.to_string()).unwrap_or_else(|| "unnamed".into());
                    let name_b = world.get::<Name>(*entity).map(|n| n.to_string()).unwrap_or_else(|| "unnamed".into());
                    console_warn(
                        "PostProcess",
                        format!(
                            "Duplicate effect source: {} on {:?} \"{}\" AND {:?} \"{}\" — last writer wins",
                            short, prev_entity, name_a, entity, name_b
                        ),
                    );
                }
            } else {
                seen_effects.push((effect, *entity));
            }
        }

        state.prev_effects = current;
    }

    world.insert_resource(state);
}

#[derive(Resource, Default)]
struct PostProcessDebugState {
    prev_effects: Vec<(Entity, Vec<String>)>,
}

// ---------------------------------------------------------------------------
// Rehydration logging
// ---------------------------------------------------------------------------

/// Logs when meshes are rehydrated from MeshPrimitive.
pub fn debug_log_rehydrate_meshes(
    query: Query<(Entity, &renzora::MeshPrimitive, Option<&Name>), Without<Mesh3d>>,
    debug: Option<Res<RenderingDebugLog>>,
) {
    if !is_enabled(&debug) {
        return;
    }
    for (entity, primitive, name) in &query {
        let n = name.map(|n| n.to_string()).unwrap_or_else(|| "unnamed".into());
        console_info(
            "Rehydration",
            format!(
                "Mesh rehydration needed: {:?} \"{}\" shape=\"{}\"",
                entity, n, primitive.0
            ),
        );
    }
}

/// Logs when cameras are rehydrated (SceneCamera without Camera3d).
pub fn debug_log_rehydrate_cameras(
    query: Query<(Entity, Option<&Name>, Option<&DefaultCamera>), (With<SceneCamera>, Without<Camera3d>)>,
    editor_camera: Query<(), With<EditorCamera>>,
    debug: Option<Res<RenderingDebugLog>>,
) {
    if !is_enabled(&debug) {
        return;
    }
    if query.is_empty() {
        return;
    }

    let is_editor = !editor_camera.is_empty();
    console_info(
        "Rehydration",
        format!(
            "Camera rehydration: {} scene cameras need Camera3d (editor={})",
            query.iter().count(),
            is_editor
        ),
    );
    for (entity, name, default_cam) in &query {
        let n = name.map(|n| n.to_string()).unwrap_or_else(|| "unnamed".into());
        console_info(
            "Rehydration",
            format!(
                "  {:?} \"{}\" default={} will_be_active={}",
                entity,
                n,
                default_cam.is_some(),
                !is_editor && default_cam.is_some()
            ),
        );
    }
}

/// Logs when visibility is rehydrated on parent entities.
pub fn debug_log_rehydrate_visibility(
    query: Query<(Entity, Option<&Name>), (With<Children>, Without<Visibility>)>,
    debug: Option<Res<RenderingDebugLog>>,
) {
    if !is_enabled(&debug) {
        return;
    }
    for (entity, name) in &query {
        let n = name.map(|n| n.to_string()).unwrap_or_else(|| "unnamed".into());
        console_info(
            "Rehydration",
            format!("Visibility rehydration: {:?} \"{}\"", entity, n),
        );
    }
}

// ---------------------------------------------------------------------------
// Scene camera -> editor camera sync logging
// ---------------------------------------------------------------------------

/// Logs the sync_scene_camera_to_editor_camera bridge activity.
/// Call this from within the exclusive sync system.
pub fn log_scene_camera_sync(
    scene_cam: Option<Entity>,
    editor_cam: Option<Entity>,
    synced_types: &[&str],
    removed_types: &[&str],
) {
    if synced_types.is_empty() && removed_types.is_empty() {
        return;
    }

    if !synced_types.is_empty() {
        let short: Vec<&str> = synced_types
            .iter()
            .map(|t| t.rsplit("::").next().unwrap_or(t))
            .collect();
        console_info(
            "PostProcess",
            format!(
                "Synced SceneCamera {:?} -> EditorCamera {:?}: [{}]",
                scene_cam,
                editor_cam,
                short.join(", ")
            ),
        );
    }
    if !removed_types.is_empty() {
        let short: Vec<&str> = removed_types
            .iter()
            .map(|t| t.rsplit("::").next().unwrap_or(t))
            .collect();
        console_warn(
            "PostProcess",
            format!(
                "Removed from EditorCamera {:?}: [{}]",
                editor_cam,
                short.join(", ")
            ),
        );
    }
}

// ---------------------------------------------------------------------------
// Native effect sync logging
// ---------------------------------------------------------------------------

/// Logs when a native Bevy effect is synced to the render target.
/// Call from individual sync systems (bloom, atmosphere, etc.) when they actually do work.
pub fn log_effect_sync(effect_name: &str, source_entity: Entity, target_entity: Entity, enabled: bool) {
    if enabled {
        console_info(
            "PostProcess",
            format!(
                "Sync {} from {:?} -> target {:?} (enabled)",
                effect_name, source_entity, target_entity
            ),
        );
    } else {
        console_info(
            "PostProcess",
            format!(
                "Sync {} from {:?} -> target {:?} (DISABLED, removing)",
                effect_name, source_entity, target_entity
            ),
        );
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Plugin that registers all debug logging systems.
/// Insert [`RenderingDebugLog`] resource with `enabled: true` to activate.
pub struct DebugLogPlugin;

impl Plugin for DebugLogPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] DebugLogPlugin");
        app.init_resource::<RenderingDebugLog>();
        app.init_resource::<PostProcessDebugState>();
        app.add_systems(
            Update,
            (
                debug_log_cameras,
                debug_log_effect_routing,
                debug_log_play_mode,
                debug_log_viewport_render_target,
                debug_log_rehydrate_meshes,
                debug_log_rehydrate_cameras,
                debug_log_rehydrate_visibility,
            ),
        );
        // Post-processing debug runs as exclusive system since it needs
        // world access for reflection.
        app.add_systems(Update, debug_log_post_processing);
    }
}
