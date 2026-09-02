//! Writing a scene out.
//!
//! [`save_scene`] and [`serialize_scene_to_string`] are the same pipeline with
//! different sinks: gather the named, non-chrome, non-derived entities, deny
//! every component that is runtime-rebuilt rather than authored, then serialize.
//! The two exclusion passes that matter most are the editor-chrome walk (see
//! [`has_hidden_ancestor`]) and the scene-instance / mesh-instance descendant
//! filters — content that lives in *another* file and would otherwise be baked
//! into this one on every save.

use bevy::prelude::*;
use renzora::console_log::*;
use renzora::{CurrentProject, EditorCamera, HideInHierarchy, MeshInstanceData};
use renzora_bsn::bsn::{BsnSerializer, SceneSerializer};
use renzora_bsn::DynamicSceneBuilder;
use std::path::Path;

use super::deny::{DenyOptionalSubsystems, DenyUiCameraTargets};

/// Whether any ancestor of `e` carries [`HideInHierarchy`]. The bevy_ui editor
/// chrome lives under such a root (renzora_shell tags its `ShellRoot`; gizmos and
/// previews tag theirs), but the marker sits ONLY on the root — its named child
/// widgets (dock tabs, hierarchy rows, inspector fields, glyph icons) otherwise
/// pass the direct `Without<HideInHierarchy>` save filter and get serialized into
/// the scene, where on reload they paint full-window over the editor (blank) and
/// the game (black). Mirrors the ancestor walk the scene-clear despawn path uses.
pub(crate) fn has_hidden_ancestor(world: &World, mut e: Entity) -> bool {
    while let Some(parent) = world.get::<ChildOf>(e).map(|c| c.parent()) {
        if world.get::<HideInHierarchy>(parent).is_some() {
            return true;
        }
        e = parent;
    }
    false
}

/// Scene saves must serialize the *authored* visibility, not the viewport
/// gate's override (the editor hides the whole scene while no viewport panel
/// is visible — see `renzora_viewport::gate_scene_visibility`). Restores the
/// stored values in place; while the gate condition still holds, its system
/// re-hides everything on the next frame, so nothing flickers on screen.
pub(crate) fn restore_viewport_gated_visibility(world: &mut World) {
    let gated: Vec<(Entity, Visibility)> = {
        let mut q = world.query::<(Entity, &renzora::core::ViewportGateHidden)>();
        q.iter(world).map(|(e, g)| (e, g.0)).collect()
    };
    for (entity, vis) in gated {
        if let Some(mut v) = world.get_mut::<Visibility>(entity) {
            *v = vis;
        }
    }
}

/// Save specific entities to a RON file.
pub fn save_scene(world: &mut World, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    restore_viewport_gated_visibility(world);
    let type_registry = world.resource::<AppTypeRegistry>().clone();

    let mut entities: Vec<Entity> = Vec::new();
    let mut query = world.query_filtered::<Entity, (
        With<Name>,
        Without<HideInHierarchy>,
        Without<EditorCamera>,
        Without<bevy::input::gamepad::Gamepad>,
        // `Persistent` entities belong to a global/autoload scene, not to this
        // one — they were already in the world before it loaded and outlive it.
        // Saving them would bake a copy into every scene that happened to be
        // open, so the next load spawns two of each: two music players, two
        // HUD roots.
        Without<renzora::Persistent>,
    )>();
    for entity in query.iter(world) {
        entities.push(entity);
    }

    // Exclude editor-chrome descendants (see `has_hidden_ancestor`): the shell
    // tags only its root with `HideInHierarchy`, so its named child widgets would
    // otherwise be baked into the scene and overlay the window on reload.
    {
        let before = entities.len();
        entities.retain(|&entity| !has_hidden_ancestor(world, entity));
        let excluded = before - entities.len();
        if excluded > 0 {
            console_info(
                "Scene",
                format!("Excluded {} editor-chrome entities from save", excluded),
            );
        }
    }

    // Exclude descendants of SceneInstance entities — those come from the
    // referenced source scene file and live there, not here. Only the instance
    // root (with its transform + any host overrides) is saved in the host.
    {
        let instance_roots: Vec<Entity> = {
            let mut q = world.query_filtered::<Entity, With<renzora::SceneInstance>>();
            q.iter(world).collect()
        };
        if !instance_roots.is_empty() {
            let before = entities.len();
            entities.retain(|&entity| {
                let mut cursor = entity;
                while let Some(child_of) = world.get::<ChildOf>(cursor) {
                    let parent = child_of.parent();
                    if instance_roots.contains(&parent) {
                        return false; // descendant of a scene instance — skip
                    }
                    cursor = parent;
                }
                true
            });
            let excluded = before - entities.len();
            if excluded > 0 {
                console_info(
                    "Scene",
                    format!(
                        "Excluded {} nested-scene descendant entities from save",
                        excluded
                    ),
                );
            }
        }
    }

    // Exclude descendants of MeshInstanceData entities — those are spawned GLTF
    // children that get regenerated by rehydration. Only the parent (which holds
    // the model_path) should be saved.
    {
        let mesh_instance_entities: Vec<Entity> = {
            let mut q = world.query_filtered::<Entity, With<MeshInstanceData>>();
            q.iter(world).collect()
        };
        if !mesh_instance_entities.is_empty() {
            let before = entities.len();
            entities.retain(|&entity| {
                // Walk up the parent chain; if we hit a MeshInstanceData entity
                // and it's not *this* entity, exclude it.
                let mut cursor = entity;
                while let Some(child_of) = world.get::<ChildOf>(cursor) {
                    let parent = child_of.parent();
                    if mesh_instance_entities.contains(&parent) {
                        return false; // descendant of a mesh instance — skip
                    }
                    cursor = parent;
                }
                true
            });
            let excluded = before - entities.len();
            if excluded > 0 {
                console_info(
                    "Scene",
                    format!("Excluded {} GLTF descendant entities from save", excluded),
                );
            }
        }
    }

    if entities.is_empty() {
        let content = "(entities: {}, resources: {})";
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        console_info("Scene", format!("Saved empty scene to {}", path.display()));
        info!("Saved empty scene to {}", path.display());
        return Ok(());
    }

    // Cheap and idempotent. Called here rather than relying on the Startup
    // system alone, so a plugin that registered a type after boot — a reload, a
    // late load — is still described by the time its bytes are written.
    crate::plugin_scene_bridge::refresh_raw_component_registry(world);

    let mut scene = DynamicSceneBuilder::from_world(world)
        .deny_all_resources()
        .deny_render_3d_materials()
        .deny_terrain_material()
        .deny_component::<Camera3d>()
        .deny_component::<Camera>()
        // Bevy UI camera-target plumbing — see `DenyUiCameraTargets`.
        .deny_ui_camera_targets()
        .deny_component::<ViewVisibility>()
        .deny_component::<Children>()
        .deny_component::<bevy::transform::components::TransformTreeChanged>()
        .deny_component::<bevy::camera::primitives::Aabb>()
        // Runtime mirror of the camera's projection, rebuilt every frame.
        .deny_component::<crate::camera_script::CameraReadState>()
        .deny_component::<bevy::render::sync_world::SyncToRenderWorld>()
        .deny_component::<bevy::input::gamepad::Gamepad>()
        .deny_component::<bevy::input::gamepad::GamepadSettings>()
        // Animation runtime state — ephemeral, must rebuild on load.
        .deny_animation_state()
        // Networking: replication bookkeeping is runtime-only.
        .deny_network_components()
        // Avian runtime components are regenerated on load from our
        // serializable PhysicsBodyData + CollisionShapeData. Persisting them
        // causes duplicate-reflect-type errors during deserialization.
        .deny_physics_components()
        .extract_entities(entities.into_iter())
        // Plugin-owned globals. Opt-in per call site, because `deny_all_resources`
        // above cannot express this — it filters by `TypeId`, which a
        // layout-registered type does not have. A full scene save wants them; a
        // subtree snapshot or a prefab does not, and neither calls this.
        .extract_raw_resources()
        .build();

    // Strip components that can't be serialized or are editor-only.
    {
        let registry = type_registry.read();
        for entity in &mut scene.entities {
            entity.components.retain(|component| {
                // Filter editor-only types by name (not available as deps in runtime)
                let type_name = component.reflect_type_path();
                // Legacy: `bevy_mod_outline` was removed (selection highlight is
                // now the bounding-box gizmo only). Kept as a cheap string check so
                // a scene saved before that still loads without unknown-type noise.
                if type_name.starts_with("bevy_mod_outline::") {
                    return false;
                }
                // Never serialize avian runtime components — they're regenerated
                // on load from PhysicsBodyData + CollisionShapeData. Persisting
                // them causes duplicate-reflect-type errors on deserialize.
                if type_name.starts_with("avian3d::") || type_name.starts_with("avian2d::") {
                    return false;
                }
                // Gaussian-splat runtime components (CloudSettings, selection
                // markers) are resolved on load from the serializable
                // renzora::GaussianSplat by the renzora_gaussian_splatting
                // plugin's sync system. Persisting them duplicates that state
                // and makes scenes warn on hosts running without the plugin.
                if type_name.starts_with("bevy_gaussian_splatting::") {
                    return false;
                }
                // Transient render-world links + per-frame computed data that
                // 0.19 made reflectable, so they now leak into saves. `RenderEntity`
                // is a stale render-world id; the `Cascades*` blobs are recomputed
                // shadow matrices each frame (they're what bloated this file to
                // ~85 KB); `InheritedVisibility` is derived from `Visibility`
                // (`ViewVisibility` is already denied above). All are re-added at
                // runtime, so dropping them is lossless.
                if matches!(
                    type_name,
                    "bevy_render::sync_world::RenderEntity"
                        | "bevy_light::cascade::Cascades"
                        | "bevy_camera::primitives::CascadesFrusta"
                        | "bevy_camera::visibility::CascadesVisibleEntities"
                        | "bevy_camera::visibility::InheritedVisibility"
                ) {
                    return false;
                }
                let serializer = bevy::reflect::serde::TypedReflectSerializer::new(
                    component.as_partial_reflect(),
                    &registry,
                );
                // Try serializing to a throwaway RON value
                ron::ser::to_string(&serializer).is_ok()
            });
        }
    }

    let registry = type_registry.read();
    let serialized = BsnSerializer
        .serialize(&scene, &registry)
        .map_err(|e| format!("Scene serialization failed: {e}"))?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, &serialized)?;
    console_info(
        "Scene",
        format!(
            "Saved scene to {} ({} entities)",
            path.display(),
            scene.entities.len()
        ),
    );
    info!(
        "Saved scene to {} ({} entities)",
        path.display(),
        scene.entities.len()
    );
    Ok(())
}

/// Serialize scene entities to a RON string (same logic as `save_scene` but returns a string).
pub fn serialize_scene_to_string(world: &mut World) -> Result<String, Box<dyn std::error::Error>> {
    restore_viewport_gated_visibility(world);
    let type_registry = world.resource::<AppTypeRegistry>().clone();

    let mut entities: Vec<Entity> = Vec::new();
    let mut query = world.query_filtered::<Entity, (
        With<Name>,
        Without<HideInHierarchy>,
        Without<EditorCamera>,
        Without<bevy::input::gamepad::Gamepad>,
        // Global/autoload scene content — see the matching filter above.
        Without<renzora::Persistent>,
    )>();
    for entity in query.iter(world) {
        entities.push(entity);
    }

    // Exclude editor-chrome descendants (see `has_hidden_ancestor`).
    entities.retain(|&entity| !has_hidden_ancestor(world, entity));

    // Exclude descendants of MeshInstanceData entities
    {
        let mesh_instance_entities: Vec<Entity> = {
            let mut q = world.query_filtered::<Entity, With<MeshInstanceData>>();
            q.iter(world).collect()
        };
        if !mesh_instance_entities.is_empty() {
            entities.retain(|&entity| {
                let mut cursor = entity;
                while let Some(child_of) = world.get::<ChildOf>(cursor) {
                    let parent = child_of.parent();
                    if mesh_instance_entities.contains(&parent) {
                        return false;
                    }
                    cursor = parent;
                }
                true
            });
        }
    }

    if entities.is_empty() {
        return Ok("(entities: {}, resources: {})".to_string());
    }

    // Cheap and idempotent. Called here rather than relying on the Startup
    // system alone, so a plugin that registered a type after boot — a reload, a
    // late load — is still described by the time its bytes are written.
    crate::plugin_scene_bridge::refresh_raw_component_registry(world);

    let mut scene = DynamicSceneBuilder::from_world(world)
        .deny_all_resources()
        .deny_render_3d_materials()
        .deny_terrain_material()
        .deny_component::<Camera3d>()
        .deny_component::<Camera>()
        // Bevy UI camera-target plumbing — see `DenyUiCameraTargets`.
        .deny_ui_camera_targets()
        .deny_component::<ViewVisibility>()
        .deny_component::<Children>()
        .deny_component::<bevy::transform::components::TransformTreeChanged>()
        .deny_component::<bevy::camera::primitives::Aabb>()
        // Runtime mirror of the camera's projection, rebuilt every frame.
        .deny_component::<crate::camera_script::CameraReadState>()
        .deny_component::<bevy::render::sync_world::SyncToRenderWorld>()
        .deny_component::<bevy::input::gamepad::Gamepad>()
        .deny_component::<bevy::input::gamepad::GamepadSettings>()
        // Animation runtime state — ephemeral, must rebuild on load.
        .deny_animation_state()
        .deny_network_components()
        // Avian runtime components are regenerated on load from our
        // serializable PhysicsBodyData + CollisionShapeData. Persisting them
        // causes duplicate-reflect-type errors during deserialization.
        .deny_physics_components()
        .extract_entities(entities.into_iter())
        // Plugin-owned globals. Opt-in per call site, because `deny_all_resources`
        // above cannot express this — it filters by `TypeId`, which a
        // layout-registered type does not have. A full scene save wants them; a
        // subtree snapshot or a prefab does not, and neither calls this.
        .extract_raw_resources()
        .build();

    // Strip components that can't be serialized or are editor-only.
    {
        let registry = type_registry.read();
        for entity in &mut scene.entities {
            entity.components.retain(|component| {
                let type_name = component.reflect_type_path();
                if type_name.starts_with("bevy_mod_outline::") {
                    return false;
                }
                // Avian runtime components (e.g. ColliderMarker) are
                // regenerated on load from PhysicsBodyData + CollisionShapeData.
                // Persisting them causes duplicate-reflect-type errors on
                // deserialize — same filter as `save_scene`.
                if type_name.starts_with("avian3d::") || type_name.starts_with("avian2d::") {
                    return false;
                }
                // Gaussian-splat runtime components (CloudSettings, selection
                // markers) are resolved on load from the serializable
                // renzora::GaussianSplat by the renzora_gaussian_splatting
                // plugin's sync system. Persisting them duplicates that state
                // and makes scenes warn on hosts running without the plugin.
                if type_name.starts_with("bevy_gaussian_splatting::") {
                    return false;
                }
                let serializer = bevy::reflect::serde::TypedReflectSerializer::new(
                    component.as_partial_reflect(),
                    &registry,
                );
                ron::ser::to_string(&serializer).is_ok()
            });
        }
    }

    let registry = type_registry.read();
    let serialized = BsnSerializer
        .serialize(&scene, &registry)
        .map_err(|e| format!("Scene serialization failed: {e}"))?;

    Ok(serialized)
}

/// Save the current project's main scene.
pub fn save_current_scene(world: &mut World) {
    let Some(project) = world.get_resource::<CurrentProject>() else {
        warn!("No project open — cannot save scene");
        return;
    };
    let path = project.main_scene_path();
    if let Err(e) = save_scene(world, &path) {
        error!("Failed to save scene: {}", e);
    }
}
