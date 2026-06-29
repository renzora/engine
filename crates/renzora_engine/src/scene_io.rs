#![allow(unused_mut, dead_code, unused_variables)]

//! Shared scene load/save and rehydration — used by both editor and runtime.

use bevy::core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass, NormalPrepass};
use bevy::ecs::world::FilteredEntityRef;
use bevy::light::atmosphere::ScatteringMedium;
use bevy::light::{Atmosphere, AtmosphereEnvironmentMapLight};
// Interim BSN scene IR + format (replaces Bevy 0.18's deleted DynamicScene/RON).
use renzora_bsn::bsn::{BsnSerializer, SceneSerializer};
use renzora_bsn::{DynamicEntity, DynamicScene, DynamicSceneBuilder};
#[cfg(feature = "render_3d")]
use bevy::pbr::AtmosphereSettings;
use bevy::prelude::*;
use bevy::camera::Hdr;
use renzora::console_log::*;
use renzora::{
    CurrentProject, DefaultCamera, EditorCamera, HideInHierarchy, MeshColor, MeshInstanceData,
    MeshPrimitive, PlayModeCamera, PlayModeState, SceneCamera, ShapeRegistry,
    ViewportRenderTarget,
};
use renzora_lighting::Sun;
use std::collections::BTreeSet;
use std::path::Path;

/// Conditionally-compiled `deny_component` calls for the optional `animation`
/// and `terrain` subsystems. When the lean exporter strips those crates, their
/// component types don't exist, so the deny is a no-op — but the call sites in
/// the three save chains stay identical, keeping them readable instead of
/// breaking each chain apart with an inline `#[cfg]`.
trait DenyOptionalSubsystems: Sized {
    fn deny_animation_state(self) -> Self;
    fn deny_terrain_material(self) -> Self;
    fn deny_physics_components(self) -> Self;
    fn deny_render_3d_materials(self) -> Self;
}

impl DenyOptionalSubsystems for DynamicSceneBuilder<'_> {
    #[cfg(feature = "animation")]
    fn deny_animation_state(self) -> Self {
        // AnimatorReadState is a runtime mirror — rebuilt each frame.
        self.deny_component::<renzora_animation::AnimatorReadState>()
    }
    #[cfg(not(feature = "animation"))]
    fn deny_animation_state(self) -> Self {
        self
    }

    #[cfg(feature = "terrain")]
    fn deny_terrain_material(self) -> Self {
        self.deny_component::<MeshMaterial3d<renzora_terrain::material::TerrainCheckerboardMaterial>>()
    }
    #[cfg(not(feature = "terrain"))]
    fn deny_terrain_material(self) -> Self {
        self
    }

    // Avian runtime components are regenerated on load from our serializable
    // PhysicsBodyData + CollisionShapeData; persisting them causes
    // duplicate-reflect-type errors during deserialization. Stripped with the
    // `physics` subsystem (no avian → these types don't exist).
    #[cfg(feature = "physics")]
    fn deny_physics_components(self) -> Self {
        self.deny_component::<avian3d::prelude::Collider>()
            .deny_component::<avian3d::collision::collider::ColliderAabb>()
            .deny_component::<avian3d::prelude::RigidBody>()
            .deny_component::<avian3d::prelude::LinearVelocity>()
            .deny_component::<avian3d::prelude::AngularVelocity>()
            .deny_component::<avian3d::prelude::Mass>()
            .deny_component::<avian3d::prelude::Friction>()
            .deny_component::<avian3d::prelude::Restitution>()
            .deny_component::<avian3d::prelude::GravityScale>()
            .deny_component::<avian3d::prelude::LinearDamping>()
            .deny_component::<avian3d::prelude::AngularDamping>()
            .deny_component::<avian3d::prelude::LockedAxes>()
            .deny_component::<avian3d::prelude::Sensor>()
    }
    #[cfg(not(feature = "physics"))]
    fn deny_physics_components(self) -> Self {
        self
    }

    // The 3D mesh/material runtime components (bevy_pbr `Mesh3d`/`StandardMaterial`
    // + renzora_shader's `GraphMaterial`/`MaterialResolved`). Stripped with the
    // `render_3d` subsystem — in a 2D-only export bevy_pbr/renzora_shader are gone,
    // so these types don't exist. The serializable mesh/material refs persist
    // instead and rehydrate on load.
    #[cfg(feature = "render_3d")]
    fn deny_render_3d_materials(self) -> Self {
        self.deny_component::<Mesh3d>()
            .deny_component::<MeshMaterial3d<StandardMaterial>>()
            .deny_component::<MeshMaterial3d<renzora_shader::material::runtime::GraphMaterial>>()
            .deny_component::<renzora_shader::material::resolver::MaterialResolved>()
    }
    #[cfg(not(feature = "render_3d"))]
    fn deny_render_3d_materials(self) -> Self {
        self
    }
}

// ============================================================================
// Scene load state + events
// ============================================================================

/// Coarse phase of the most recent scene load.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SceneLoadPhase {
    #[default]
    Idle,
    Loading,
    Ready,
    Failed,
}

/// Tracks the state of scene loading so UI can reflect progress.
///
/// `progress` is 0..1. Scene loading is currently synchronous, so the value
/// jumps from 0 → 1 in a single frame; a future async split can make this a
/// true progress readout without changing this resource's shape.
#[derive(Resource, Default)]
pub struct SceneLoadState {
    pub phase: SceneLoadPhase,
    pub current_path: Option<String>,
    pub progress: f32,
}

#[derive(Event, Clone, Debug)]
pub struct SceneLoaded {
    pub path: String,
}

#[derive(Event, Clone, Debug)]
pub struct SceneLoadFailed {
    pub path: String,
    pub error: String,
}

/// Fired after a scene loads when one or more component/resource types
/// were skipped because they aren't registered in the type registry.
/// The editor turns this into a toast; the runtime just logs it.
///
/// Most-common cause: an editor-only component (e.g.
/// `renzora_camera::OrbitCameraState`) was serialized into the scene and
/// then loaded by a runtime build that doesn't register editor types.
#[derive(Event, Clone, Debug)]
pub struct SceneLoadedWithSkippedTypes {
    pub path: String,
    pub skipped: Vec<String>,
}

// ============================================================================
// Save
// ============================================================================

/// Whether any ancestor of `e` carries [`HideInHierarchy`]. The bevy_ui editor
/// chrome lives under such a root (renzora_shell tags its `ShellRoot`; gizmos and
/// previews tag theirs), but the marker sits ONLY on the root — its named child
/// widgets (dock tabs, hierarchy rows, inspector fields, glyph icons) otherwise
/// pass the direct `Without<HideInHierarchy>` save filter and get serialized into
/// the scene, where on reload they paint full-window over the editor (blank) and
/// the game (black). Mirrors the ancestor walk the scene-clear despawn path uses.
fn has_hidden_ancestor(world: &World, mut e: Entity) -> bool {
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
fn restore_viewport_gated_visibility(world: &mut World) {
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

    let mut scene = DynamicSceneBuilder::from_world(world)
        .deny_all_resources()
        .deny_render_3d_materials()
        .deny_terrain_material()
        .deny_component::<Camera3d>()
        .deny_component::<Camera>()
        // Bevy UI camera-target plumbing. UiTargetCamera holds an Entity
        // reference that doesn't remap across loads (e.g. an editor-only
        // play-mode camera), and ComputedUiTargetCamera is a runtime-derived
        // mirror — persisting either makes UI render to a dead entity in
        // the runtime and silently disappear.
        .deny_component::<bevy::ui::UiTargetCamera>()
        .deny_component::<bevy::ui::ComputedUiTargetCamera>()
        .deny_component::<ViewVisibility>()
        .deny_component::<Children>()
        .deny_component::<bevy::transform::components::TransformTreeChanged>()
        .deny_component::<bevy::camera::primitives::Aabb>()
        .deny_component::<bevy::render::sync_world::SyncToRenderWorld>()
        .deny_component::<bevy::input::gamepad::Gamepad>()
        .deny_component::<bevy::input::gamepad::GamepadSettings>()
        // Animation runtime state — ephemeral, must rebuild on load.
        .deny_component::<bevy::animation::AnimationPlayer>()
        .deny_component::<bevy::animation::transition::AnimationTransitions>()
        // `AnimatedBy` stores an Entity reference that doesn't remap across
        // scene loads — must be reconstructed by the animator rehydration.
        .deny_component::<bevy::animation::AnimatedBy>()
        .deny_animation_state()
        // Networking: Lightyear internals should not persist to scene files.
        // Networked/NetworkOwner/NetworkId are runtime-only markers.
        .deny_component::<renzora_network::Networked>()
        .deny_component::<renzora_network::NetworkOwner>()
        .deny_component::<renzora_network::NetworkId>()
        // Avian runtime components are regenerated on load from our
        // serializable PhysicsBodyData + CollisionShapeData. Persisting them
        // causes duplicate-reflect-type errors during deserialization.
        .deny_physics_components()
        .extract_entities(entities.into_iter())
        .build();

    // Strip components that can't be serialized or are editor-only.
    {
        let registry = type_registry.read();
        for entity in &mut scene.entities {
            entity.components.retain(|component| {
                // Filter editor-only types by name (not available as deps in runtime)
                let type_name = component.reflect_type_path();
                if type_name.starts_with("bevy_mod_outline::") {
                    return false;
                }
                // Never serialize avian runtime components — they're regenerated
                // on load from PhysicsBodyData + CollisionShapeData. Persisting
                // them causes duplicate-reflect-type errors on deserialize.
                if type_name.starts_with("avian3d::") {
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

    let mut scene = DynamicSceneBuilder::from_world(world)
        .deny_all_resources()
        .deny_render_3d_materials()
        .deny_terrain_material()
        .deny_component::<Camera3d>()
        .deny_component::<Camera>()
        // Bevy UI camera-target plumbing. UiTargetCamera holds an Entity
        // reference that doesn't remap across loads (e.g. an editor-only
        // play-mode camera), and ComputedUiTargetCamera is a runtime-derived
        // mirror — persisting either makes UI render to a dead entity in
        // the runtime and silently disappear.
        .deny_component::<bevy::ui::UiTargetCamera>()
        .deny_component::<bevy::ui::ComputedUiTargetCamera>()
        .deny_component::<ViewVisibility>()
        .deny_component::<Children>()
        .deny_component::<bevy::transform::components::TransformTreeChanged>()
        .deny_component::<bevy::camera::primitives::Aabb>()
        .deny_component::<bevy::render::sync_world::SyncToRenderWorld>()
        .deny_component::<bevy::input::gamepad::Gamepad>()
        .deny_component::<bevy::input::gamepad::GamepadSettings>()
        // Animation runtime state — ephemeral, must rebuild on load.
        .deny_component::<bevy::animation::AnimationPlayer>()
        .deny_component::<bevy::animation::transition::AnimationTransitions>()
        // `AnimatedBy` stores an Entity reference that doesn't remap across
        // scene loads — must be reconstructed by the animator rehydration.
        .deny_component::<bevy::animation::AnimatedBy>()
        .deny_animation_state()
        .deny_component::<renzora_network::Networked>()
        .deny_component::<renzora_network::NetworkOwner>()
        .deny_component::<renzora_network::NetworkId>()
        // Avian runtime components are regenerated on load from our
        // serializable PhysicsBodyData + CollisionShapeData. Persisting them
        // causes duplicate-reflect-type errors during deserialization.
        .deny_physics_components()
        .extract_entities(entities.into_iter())
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
                if type_name.starts_with("avian3d::") {
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

/// Load a scene from a RON string into the world (same logic as `load_scene` but from string).
pub fn load_scene_from_string(world: &mut World, ron: &str) {
    let trimmed = ron.trim();
    if trimmed.is_empty() || trimmed == "(entities: {}, resources: {})" {
        return;
    }

    let (mut scene, skipped_types) = match deserialize_scene_lossy(world, ron) {
        Ok(pair) => pair,
        Err(e) => {
            error!("Failed to deserialize scene from string: {}", e);
            return;
        }
    };
    if !skipped_types.is_empty() {
        for type_path in &skipped_types {
            warn!(
                "[scene] string scene skipped unregistered type `{}`",
                type_path
            );
        }
        // No path to report for string scenes — pass an empty marker.
        world.trigger(SceneLoadedWithSkippedTypes {
            path: String::new(),
            skipped: skipped_types.clone(),
        });
    }

    let pruned = prune_orphaned_entities(&mut scene);
    if pruned > 0 {
        warn!(
            "[scene] pruned {} orphaned entities (leaked editor-chrome / missing parent) from string scene",
            pruned
        );
    }
    let ui_pruned = prune_leaked_ui(&mut scene);
    if ui_pruned > 0 {
        warn!(
            "[scene] pruned {} leaked editor-UI entities (no UiCanvas ancestor) from string scene",
            ui_pruned
        );
    }

    let mut entity_map = bevy::ecs::entity::EntityHashMap::default();
    match scene.write_to_world(world, &mut entity_map) {
        Ok(()) => {
            // Re-insert ChildOf to trigger hierarchy hooks
            let children_with_parents: Vec<(Entity, Entity)> = entity_map
                .values()
                .filter_map(|&entity| {
                    world
                        .get_entity(entity)
                        .ok()?
                        .get::<ChildOf>()
                        .map(|c| (entity, c.parent()))
                })
                .collect();

            for (child, parent) in children_with_parents {
                world.entity_mut(child).remove::<ChildOf>();
                world.entity_mut(child).insert(ChildOf(parent));
            }
        }
        Err(e) => {
            error!("Failed to write scene from string to world: {}", e);
        }
    }
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

// ============================================================================
// Load
// ============================================================================

/// Try to deserialize a scene RON, transparently skipping any
/// component/resource entries whose type isn't registered.
///
/// Bevy's [`SceneDeserializer`] aborts on the first unknown type, so
/// we loop: parse the offending type out of the error message, strip
/// that entry from the RON, retry. Each pass either makes progress
/// (one type stripped) or returns an error we can't massage away.
///
/// Returns the parsed scene plus the list of skipped type paths so the
/// caller can surface a warning.
fn deserialize_scene_lossy(
    world: &World,
    text: &str,
) -> Result<(DynamicScene, Vec<String>), String> {
    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let registry = type_registry.read();
    // The interim BSN parser skips unregistered / un-deserializable components
    // itself (returning their type paths), so the old RON strip-and-retry loop
    // is unnecessary — a scene authored with a now-absent plugin still loads.
    BsnSerializer
        .deserialize_lossy(text, &registry)
        .map_err(|e| e.to_string())
}

/// Pull the offending type path out of a Bevy/serde error message of the
/// form "no registration found for `some::type::path`". Returns `None`
/// if the error isn't of that shape — caller should surface the original
/// error verbatim in that case.
///
/// Obsolete: the interim BSN parser skips unregistered components itself (see
/// `deserialize_scene_lossy`). Retained with its tests pending removal.
#[allow(dead_code)]
fn extract_unregistered_type(error_message: &str) -> Option<String> {
    let needle = "no registration found for ";
    let pos = error_message.find(needle)?;
    let rest = &error_message[pos + needle.len()..];
    // Tolerate both `... for \`T\`` and `... for type \`T\``.
    let rest = rest.strip_prefix("type ").unwrap_or(rest);
    let rest = rest.strip_prefix('`')?;
    let close = rest.find('`')?;
    Some(rest[..close].to_string())
}

/// Remove the entry `"<type_path>": ( ... )` (and its trailing comma /
/// own line) from a RON scene string, walking balanced parens to find
/// the closing `)` while respecting string literals. Returns `None` if
/// the key isn't present or paren-matching fails.
///
/// Removing the whole line keeps the surrounding map well-formed
/// regardless of whether the entry is first, last, or middle: leftover
/// commas are RON-tolerated (trailing commas allowed), and we never
/// leave back-to-back commas because we consume one trailing comma when
/// it's there.
///
/// Obsolete under the interim BSN format (no RON text surgery). Retained with
/// its tests pending removal.
#[allow(dead_code)]
fn strip_component_entry(ron: &str, type_path: &str) -> Option<String> {
    let key = format!("\"{}\"", type_path);
    let key_pos = ron.find(&key)?;
    let key_end = key_pos + key.len();
    let bytes = ron.as_bytes();

    // Find the opening `(` after the key (skipping the `:` and whitespace).
    let mut i = key_end;
    while i < bytes.len() && bytes[i] != b'(' {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    let open_pos = i;

    // Walk balanced parens from after the open. String literals don't
    // count toward depth — track escapes so an escaped quote inside a
    // string doesn't terminate it prematurely.
    let mut depth: i32 = 1;
    let mut in_string = false;
    let mut prev_escape = false;
    let mut close_pos: Option<usize> = None;
    for (j, &c) in bytes.iter().enumerate().skip(open_pos + 1) {
        if in_string {
            if c == b'"' && !prev_escape {
                in_string = false;
            }
            prev_escape = c == b'\\' && !prev_escape;
            continue;
        }
        prev_escape = false;
        match c {
            b'"' => in_string = true,
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    close_pos = Some(j);
                    break;
                }
            }
            _ => {}
        }
    }
    let close_pos = close_pos?;

    // Extend forward to consume the trailing comma + the rest of the
    // line (including the line break). Keeps the surrounding indentation
    // pristine.
    let mut end = close_pos + 1;
    while end < bytes.len() && (bytes[end] == b' ' || bytes[end] == b'\t') {
        end += 1;
    }
    if end < bytes.len() && bytes[end] == b',' {
        end += 1;
    }
    while end < bytes.len() && (bytes[end] == b' ' || bytes[end] == b'\t') {
        end += 1;
    }
    if end < bytes.len() && bytes[end] == b'\n' {
        end += 1;
    }

    // Extend backward to the start of the key's own line so we don't
    // leave a blank, indented line behind.
    let mut start = key_pos;
    while start > 0 {
        let c = bytes[start - 1];
        if c == b'\n' {
            break;
        }
        if !c.is_ascii_whitespace() {
            break;
        }
        start -= 1;
    }

    let mut out = String::with_capacity(ron.len());
    out.push_str(&ron[..start]);
    out.push_str(&ron[end..]);
    Some(out)
}

/// The `ChildOf` parent recorded on a serialized scene entity, if any. Read
/// straight out of the reflected components — the scene isn't in the World yet,
/// so we can't query it.
fn scene_entity_parent(dyn_ent: &DynamicEntity) -> Option<Entity> {
    for comp in &dyn_ent.components {
        let is_child_of = comp
            .get_represented_type_info()
            .map(|ti| ti.type_path() == <ChildOf as bevy::reflect::TypePath>::type_path())
            .unwrap_or(false);
        if !is_child_of {
            continue;
        }
        if let bevy::reflect::ReflectRef::TupleStruct(ts) = comp.reflect_ref() {
            if let Some(parent) = ts.field(0).and_then(|f| f.try_downcast_ref::<Entity>()) {
                return Some(*parent);
            }
        }
    }
    None
}

/// Drop entities whose `ChildOf` ancestor chain leads to a parent that ISN'T in
/// the scene. Such an entity is an orphan of a root that was excluded at save
/// time — almost always leaked editor-chrome widgets: the `HideInHierarchy`
/// shell root is correctly filtered out of saves, but older scenes baked in its
/// named children (dock tabs, glyph icons, inspector rows). On load those would
/// reparent to the window root and paint full-window over the editor (blank).
///
/// Cascades for free: a child of a pruned entity is pruned too, because its own
/// chain still climbs to the same missing root. A well-formed scene has complete
/// hierarchies, so nothing is dropped. Returns how many were pruned.
fn prune_orphaned_entities(scene: &mut DynamicScene) -> usize {
    use std::collections::{HashMap, HashSet};
    let ids: HashSet<Entity> = scene.entities.iter().map(|e| e.entity).collect();
    if ids.is_empty() {
        return 0;
    }
    let parent_of: HashMap<Entity, Entity> = scene
        .entities
        .iter()
        .filter_map(|e| scene_entity_parent(e).map(|p| (e.entity, p)))
        .collect();

    let orphaned = |start: Entity| -> bool {
        let mut cur = start;
        let mut seen = HashSet::new();
        loop {
            if !seen.insert(cur) {
                return false; // cycle — keep rather than loop forever
            }
            match parent_of.get(&cur) {
                None => return false,                   // a root → valid
                Some(p) if ids.contains(p) => cur = *p, // climb toward the root
                Some(_) => return true,                 // parent absent → orphan
            }
        }
    };

    let before = scene.entities.len();
    // Restrict to UI entities: leaked chrome is always `bevy_ui` nodes, so this
    // can never drop legit 3D scene data even if some non-UI entity were
    // orphaned for an unrelated reason.
    scene.entities.retain(|e| !(orphaned(e.entity) && scene_entity_is_ui(e)));
    before - scene.entities.len()
}

/// Whether a serialized scene entity is a `bevy_ui` node (carries `Node`).
fn scene_entity_is_ui(dyn_ent: &DynamicEntity) -> bool {
    dyn_ent.components.iter().any(|c| {
        c.get_represented_type_info()
            .map(|ti| ti.type_path() == "bevy_ui::ui_node::Node")
            .unwrap_or(false)
    })
}

/// Whether a serialized scene entity is a game-UI `UiCanvas` root. Legitimate
/// game UI lives under one of these; matched by reflected type-path so this crate
/// needn't depend on `renzora_ember`.
fn scene_entity_is_canvas(dyn_ent: &DynamicEntity) -> bool {
    dyn_ent.components.iter().any(|c| {
        c.get_represented_type_info()
            .map(|ti| ti.type_path() == "renzora_ember::game_ui::components::canvas::UiCanvas")
            .unwrap_or(false)
    })
}

/// Drop leaked editor UI: any `bevy_ui` node with no `UiCanvas` self-or-ancestor.
///
/// The only legitimate UI in a scene is game UI, which always sits under a
/// [`UiCanvas`] root (the serializable source of truth, rebuilt on load); 3D
/// content carries no `Node`. So a `Node` entity outside every canvas is editor
/// chrome an over-eager save baked in — classically auto-save firing while an
/// overlay (e.g. Settings) was open, serializing its whole node tree. Unlike
/// [`prune_orphaned_entities`] (which only catches nodes whose parent is missing)
/// this also removes *connected* chrome trees that kept an intact root, so it
/// self-heals scenes already polluted before the save-side guard existed.
fn prune_leaked_ui(scene: &mut DynamicScene) -> usize {
    use std::collections::{HashMap, HashSet};
    let ids: HashSet<Entity> = scene.entities.iter().map(|e| e.entity).collect();
    if ids.is_empty() {
        return 0;
    }
    let parent_of: HashMap<Entity, Entity> = scene
        .entities
        .iter()
        .filter_map(|e| scene_entity_parent(e).map(|p| (e.entity, p)))
        .collect();
    let canvases: HashSet<Entity> = scene
        .entities
        .iter()
        .filter(|e| scene_entity_is_canvas(e))
        .map(|e| e.entity)
        .collect();

    // Whether `start` or any in-scene ancestor is a `UiCanvas`.
    let under_canvas = |start: Entity| -> bool {
        let mut cur = start;
        let mut seen = HashSet::new();
        loop {
            if canvases.contains(&cur) {
                return true;
            }
            if !seen.insert(cur) {
                return false; // cycle guard
            }
            match parent_of.get(&cur) {
                Some(p) if ids.contains(p) => cur = *p,
                _ => return false,
            }
        }
    };

    let before = scene.entities.len();
    scene
        .entities
        .retain(|e| !(scene_entity_is_ui(e) && !under_canvas(e.entity)));
    before - scene.entities.len()
}

/// Load a scene from a RON file into the world.
///
/// Tries the Vfs (rpak archive) first, then falls back to disk.
pub fn load_scene(world: &mut World, path: &Path) {
    console_info(
        "Scene",
        format!("=== Loading scene from {} ===", path.display()),
    );

    let path_str = path.to_string_lossy().to_string();
    if let Some(mut state) = world.get_resource_mut::<SceneLoadState>() {
        state.phase = SceneLoadPhase::Loading;
        state.current_path = Some(path_str.clone());
        state.progress = 0.0;
    }

    // Try reading from Vfs (rpak archive) first.
    let content = if let Some(vfs) = world.get_resource::<crate::Vfs>() {
        // Normalize to forward-slash archive-relative path, stripping leading "./" or ".\"
        let path_str = path.to_string_lossy().replace('\\', "/");
        let archive_key = path_str.strip_prefix("./").unwrap_or(&path_str);
        if let Some(s) = vfs.read_string(archive_key) {
            console_info(
                "Scene",
                format!("Read {} bytes from rpak: {}", s.len(), archive_key),
            );
            Some(s)
        } else {
            None
        }
    } else {
        None
    };

    // Fall back to disk if Vfs didn't have it.
    let content = match content {
        Some(c) => c,
        None => {
            if !path.exists() {
                console_warn(
                    "Scene",
                    format!("Scene file does not exist: {}", path.display()),
                );
                info!("Scene file does not exist yet: {}", path.display());
                return;
            }
            match std::fs::read_to_string(path) {
                Ok(c) => {
                    console_info(
                        "Scene",
                        format!("Read {} bytes from {}", c.len(), path.display()),
                    );
                    c
                }
                Err(e) => {
                    console_error(
                        "Scene",
                        format!("Failed to read scene file {}: {}", path.display(), e),
                    );
                    error!("Failed to read scene file {}: {}", path.display(), e);
                    return;
                }
            }
        }
    };

    let trimmed = content.trim();
    if trimmed.is_empty() || trimmed == "(entities: {}, resources: {})" {
        console_info("Scene", format!("Scene is empty: {}", path.display()));
        info!("Scene is empty: {}", path.display());
        if let Some(mut state) = world.get_resource_mut::<SceneLoadState>() {
            state.phase = SceneLoadPhase::Ready;
            state.progress = 1.0;
        }
        world.trigger(SceneLoaded {
            path: path_str.clone(),
        });
        return;
    }

    let (mut scene, skipped_types) = match deserialize_scene_lossy(world, &content) {
        Ok(pair) => pair,
        Err(e) => {
            error!("Failed to deserialize scene {}: {}", path.display(), e);
            return;
        }
    };
    if !skipped_types.is_empty() {
        for type_path in &skipped_types {
            warn!(
                "[scene] {} skipped unregistered type `{}`",
                path.display(),
                type_path
            );
        }
        world.trigger(SceneLoadedWithSkippedTypes {
            path: path_str.clone(),
            skipped: skipped_types.clone(),
        });
    }

    let pruned = prune_orphaned_entities(&mut scene);
    if pruned > 0 {
        console_info(
            "Scene",
            format!("Pruned {pruned} orphaned editor-chrome entities on load"),
        );
        warn!(
            "[scene] {} pruned {} orphaned entities (leaked editor-chrome / missing parent)",
            path.display(),
            pruned
        );
    }
    let ui_pruned = prune_leaked_ui(&mut scene);
    if ui_pruned > 0 {
        console_info(
            "Scene",
            format!("Pruned {ui_pruned} leaked editor-UI entities (no UiCanvas ancestor) on load"),
        );
        warn!(
            "[scene] {} pruned {} leaked editor-UI entities (no UiCanvas ancestor)",
            path.display(),
            ui_pruned
        );
    }

    let mut entity_map = bevy::ecs::entity::EntityHashMap::default();
    match scene.write_to_world(world, &mut entity_map) {
        Ok(()) => {
            console_info(
                "Scene",
                format!(
                    "Scene written to world: {} entities mapped from {}",
                    entity_map.len(),
                    path.display()
                ),
            );

            // Log each mapped entity
            for (&scene_entity, &world_entity) in &entity_map {
                let name = world
                    .get::<Name>(world_entity)
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "unnamed".into());
                let has_scene_cam = world.get::<SceneCamera>(world_entity).is_some();
                let has_default = world.get::<DefaultCamera>(world_entity).is_some();
                let mut tags = Vec::new();
                if has_scene_cam {
                    tags.push("SceneCamera");
                }
                if has_default {
                    tags.push("DefaultCamera");
                }
                let tag_str = if tags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", tags.join(", "))
                };
                console_info(
                    "Scene",
                    format!(
                        "  scene:{:?} -> world:{:?} \"{}\"{}",
                        scene_entity, world_entity, name, tag_str
                    ),
                );
            }

            info!(
                "Loaded scene from {} ({} entities mapped)",
                path.display(),
                entity_map.len()
            );

            // Bevy's write_to_world inserts ChildOf via reflection, which may not
            // trigger the on_insert hooks that maintain the parent's Children component.
            // Re-insert ChildOf on each child to force the hooks to fire.
            let children_with_parents: Vec<(Entity, Entity)> = entity_map
                .values()
                .filter_map(|&entity| {
                    world
                        .get_entity(entity)
                        .ok()?
                        .get::<ChildOf>()
                        .map(|c| (entity, c.parent()))
                })
                .collect();

            console_info(
                "Scene",
                format!(
                    "Re-inserting ChildOf on {} entities to trigger hierarchy hooks",
                    children_with_parents.len()
                ),
            );

            for (child, parent) in children_with_parents {
                // Remove and re-insert ChildOf to trigger hooks
                world.entity_mut(child).remove::<ChildOf>();
                world.entity_mut(child).insert(ChildOf(parent));
            }

            // Expand nested scene instances referenced from the host scene.
            expand_scene_instances(world);

            console_success(
                "Scene",
                format!("=== Scene load complete: {} ===", path.display()),
            );

            if let Some(mut state) = world.get_resource_mut::<SceneLoadState>() {
                state.phase = SceneLoadPhase::Ready;
                state.progress = 1.0;
            }
            world.trigger(SceneLoaded {
                path: path_str.clone(),
            });
        }
        Err(e) => {
            console_error(
                "Scene",
                format!("Failed to write scene to world {}: {}", path.display(), e),
            );
            error!("Failed to write scene to world {}: {}", path.display(), e);

            if let Some(mut state) = world.get_resource_mut::<SceneLoadState>() {
                state.phase = SceneLoadPhase::Failed;
            }
            let err_str = e.to_string();
            world.trigger(SceneLoadFailed {
                path: path_str.clone(),
                error: err_str,
            });
        }
    }
}

/// Expand every `SceneInstance` entity in the world that has no children yet:
/// load the referenced source scene and reparent its roots under the instance.
///
/// Re-runnable: already-expanded instances (any with children) are skipped.
pub fn expand_scene_instances(world: &mut World) {
    // Re-entrancy guard: if expand recursively triggers another load of a
    // scene that's already in the expand stack, bail out to avoid infinite
    // recursion on cyclic SceneInstance references.
    thread_local! {
        static EXPAND_STACK: std::cell::RefCell<Vec<std::path::PathBuf>> =
            const { std::cell::RefCell::new(Vec::new()) };
    }

    let project_root = world
        .get_resource::<CurrentProject>()
        .map(|p| p.path.clone());

    let mut to_expand: Vec<(Entity, std::path::PathBuf)> = Vec::new();
    {
        let mut q = world.query::<(Entity, &renzora::SceneInstance)>();
        for (entity, inst) in q.iter(world) {
            // Skip entities that already have children (already expanded, or
            // user added children before save).
            if world
                .get::<Children>(entity)
                .is_some_and(|c| c.iter().count() > 0)
            {
                continue;
            }
            let Some(ref root) = project_root else {
                continue;
            };
            let abs = root.join(&inst.source);
            // Skip if this path is already being expanded up the stack (cycle).
            let in_stack = EXPAND_STACK.with(|s| s.borrow().iter().any(|p| p == &abs));
            if in_stack {
                console_warn(
                    "Scene",
                    format!(
                        "Skipping recursive scene instance: {} (already expanding)",
                        abs.display()
                    ),
                );
                continue;
            }
            to_expand.push((entity, abs));
        }
    }

    for (instance_entity, source_path) in to_expand {
        EXPAND_STACK.with(|s| s.borrow_mut().push(source_path.clone()));

        let existing_roots: std::collections::HashSet<Entity> = {
            let mut q = world.query_filtered::<Entity, (With<Name>, Without<ChildOf>)>();
            q.iter(world).collect()
        };

        load_scene(world, &source_path);

        let mut new_roots: Vec<Entity> = Vec::new();
        {
            let mut q = world.query_filtered::<Entity, (With<Name>, Without<ChildOf>)>();
            for e in q.iter(world) {
                if !existing_roots.contains(&e) && e != instance_entity {
                    new_roots.push(e);
                }
            }
        }

        for root in new_roots {
            world.entity_mut(root).insert(ChildOf(instance_entity));
        }

        EXPAND_STACK.with(|s| {
            s.borrow_mut().pop();
        });
    }
}

/// Spawn a new `SceneInstance` entity that references `source_path` and
/// immediately expand it by loading the source scene's entities under it.
///
/// `source_path` is absolute; it's stored as an asset-relative string.
/// Returns the newly-spawned instance root entity, or `None` if no project
/// is open (paths can't be resolved).
/// Returns `true` if `source_path` resolves to the same file as
/// `host_scene_path` — i.e. a direct self-reference. Used by drop handlers
/// to reject a scene being dropped into itself.
pub fn is_self_reference(host_scene_path: &Path, source_path: &Path) -> bool {
    paths_equal(host_scene_path, source_path)
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => a == b,
    }
}

/// Cache of scene → outgoing scene references, keyed by canonical path with
/// mtime validation. Populated lazily by `would_create_reference_cycle` and
/// invalidated transparently when the file on disk has been modified.
///
/// Worst-case drop cost is one disk read per changed scene in the cycle
/// graph; repeated drops and deep reference graphs reuse cached entries.
#[derive(Resource, Default)]
pub struct SceneReferenceCache {
    entries: std::collections::HashMap<std::path::PathBuf, CachedRefs>,
}

struct CachedRefs {
    mtime: Option<std::time::SystemTime>,
    sources: Vec<String>,
}

impl SceneReferenceCache {
    pub fn invalidate(&mut self, path: &Path) {
        let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        self.entries.remove(&canon);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return the outgoing `SceneInstance.source` list for `path`, reading
    /// + scanning from disk only if the cache is missing or stale.
    fn references_for(&mut self, path: &Path) -> Option<&[String]> {
        let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let fresh_mtime = std::fs::metadata(&canon).and_then(|m| m.modified()).ok();

        let needs_reload = match self.entries.get(&canon) {
            Some(e) => e.mtime != fresh_mtime,
            None => true,
        };

        if needs_reload {
            let text = std::fs::read_to_string(&canon).ok()?;
            let sources = extract_scene_instance_sources(&text);
            self.entries.insert(
                canon.clone(),
                CachedRefs {
                    mtime: fresh_mtime,
                    sources,
                },
            );
        }

        self.entries.get(&canon).map(|e| e.sources.as_slice())
    }
}

/// Returns `true` if dropping `source_path` into `host_scene_path` would
/// create a cycle — either directly (source == host) or transitively
/// (source, or any scene source references through its `SceneInstance`
/// components, references host).
///
/// `project_root` is used to resolve asset-relative `source` fields read
/// from the referenced .ron files. Backed by `SceneReferenceCache` —
/// repeated calls reuse cached parses until file mtimes change.
pub fn would_create_reference_cycle(
    cache: &mut SceneReferenceCache,
    project_root: &Path,
    host_scene_path: &Path,
    source_path: &Path,
) -> bool {
    if paths_equal(host_scene_path, source_path) {
        return true;
    }
    let mut visited: std::collections::HashSet<std::path::PathBuf> =
        std::collections::HashSet::new();
    let mut stack: Vec<std::path::PathBuf> = vec![source_path.to_path_buf()];

    while let Some(current) = stack.pop() {
        let canon = current.canonicalize().unwrap_or_else(|_| current.clone());
        if !visited.insert(canon) {
            continue;
        }

        let Some(sources) = cache.references_for(&current) else {
            continue;
        };
        // Clone out so we can drop the borrow on `cache` before recursing.
        let sources: Vec<String> = sources.to_vec();
        for rel in sources {
            let next = project_root.join(&rel);
            if paths_equal(host_scene_path, &next) {
                return true;
            }
            stack.push(next);
        }
    }
    false
}

/// Scrape `renzora::core::SceneInstance` `source:` values out of a scene
/// .ron file's text. Intentionally avoids full RON deserialization — it's
/// faster and robust to unknown components.
fn extract_scene_instance_sources(text: &str) -> Vec<String> {
    const MARKER: &str = "\"renzora::core::SceneInstance\"";
    const KEY: &str = "source:";
    let mut out = Vec::new();
    let mut cursor = 0usize;
    while let Some(mi) = text[cursor..].find(MARKER) {
        let pos = cursor + mi;
        let Some(ki) = text[pos..].find(KEY) else {
            break;
        };
        let kpos = pos + ki + KEY.len();
        // Skip whitespace until opening quote.
        let mut i = kpos;
        let bytes = text.as_bytes();
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'"' {
            cursor = kpos;
            continue;
        }
        i += 1;
        let start = i;
        while i < bytes.len() && bytes[i] != b'"' {
            i += 1;
        }
        if i <= bytes.len() {
            out.push(text[start..i].to_string());
        }
        cursor = i;
    }
    out
}

pub fn spawn_scene_instance(
    world: &mut World,
    source_path: &Path,
    parent: Option<Entity>,
    transform: Transform,
) -> Option<Entity> {
    // Convert to asset-relative for portable storage.
    let relative = world
        .get_resource::<CurrentProject>()?
        .make_relative(source_path)?;

    let name = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Scene")
        .to_string();

    let mut e = world.spawn((
        Name::new(name),
        renzora::SceneInstance { source: relative },
        transform,
        Visibility::default(),
    ));
    if let Some(p) = parent {
        e.insert(ChildOf(p));
    }
    let entity = e.id();

    // Expand nested scene contents in-place.
    expand_scene_instances(world);

    Some(entity)
}

/// Save the entity tree under a `SceneInstance` back to its source `.ron`
/// file. The instance's direct children become root entities in the output
/// file; deeper descendants keep their parent-child relationships.
///
/// Returns `Ok(())` on success. Does nothing and returns `Ok(())` when the
/// instance has no descendants (empty source file is written).
pub fn save_prefab_source(
    world: &mut World,
    instance_entity: Entity,
    source_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    restore_viewport_gated_visibility(world);
    let type_registry = world.resource::<AppTypeRegistry>().clone();

    // Collect all descendants of the instance, breadth-first.
    //
    // Two filters mirror `save_scene`:
    //   1. Skip unnamed entities.
    //   2. Do NOT descend below a `MeshInstanceData` entity. Its children are
    //      runtime gltf mount points (spawned by `rehydrate_mesh_instances`)
    //      that must NOT be serialized — if they're in source.ron on reload,
    //      the `Without<Children>` rehydration guard skips the spawn and the
    //      mesh becomes invisible.
    let mut descendants: Vec<Entity> = Vec::new();
    let mut queue: Vec<Entity> = Vec::new();
    if let Some(children) = world.get::<Children>(instance_entity) {
        queue.extend(children.iter());
    }
    while let Some(e) = queue.pop() {
        if world.get::<Name>(e).is_none() {
            continue;
        }
        descendants.push(e);
        // Stop descending into gltf-owned subtrees.
        if world.get::<renzora::core::MeshInstanceData>(e).is_some() {
            continue;
        }
        if let Some(kids) = world.get::<Children>(e) {
            queue.extend(kids.iter());
        }
    }

    if descendants.is_empty() {
        // Safety: never overwrite the source file with an empty scene. If the
        // instance has no descendants at save time (e.g. expand hadn't run,
        // or the user's query state was stale), leaving the file alone is
        // much safer than clobbering it. Users can still edit the source
        // directly by opening it.
        console_warn(
            "Scene",
            format!(
                "Skipping save for {} — instance has no descendants (not overwriting with empty scene)",
                source_path.display()
            ),
        );
        return Ok(());
    }

    let mut scene = DynamicSceneBuilder::from_world(world)
        .deny_all_resources()
        .deny_render_3d_materials()
        .deny_terrain_material()
        .deny_component::<Camera3d>()
        .deny_component::<Camera>()
        // Bevy UI camera-target plumbing. UiTargetCamera holds an Entity
        // reference that doesn't remap across loads (e.g. an editor-only
        // play-mode camera), and ComputedUiTargetCamera is a runtime-derived
        // mirror — persisting either makes UI render to a dead entity in
        // the runtime and silently disappear.
        .deny_component::<bevy::ui::UiTargetCamera>()
        .deny_component::<bevy::ui::ComputedUiTargetCamera>()
        .deny_component::<ViewVisibility>()
        .deny_component::<Children>()
        // Children's GlobalTransform reflects the instance root's world-space
        // position. If serialized, it would "bake in" the host's placement
        // for anyone opening car.ron standalone. Bevy recomputes it each
        // frame from Transform anyway.
        .deny_component::<GlobalTransform>()
        .deny_component::<bevy::transform::components::TransformTreeChanged>()
        .deny_component::<bevy::camera::primitives::Aabb>()
        .deny_component::<bevy::render::sync_world::SyncToRenderWorld>()
        .deny_component::<bevy::input::gamepad::Gamepad>()
        .deny_component::<bevy::input::gamepad::GamepadSettings>()
        .deny_component::<bevy::animation::AnimationPlayer>()
        .deny_component::<bevy::animation::transition::AnimationTransitions>()
        .deny_component::<bevy::animation::AnimatedBy>()
        .deny_animation_state()
        .deny_component::<renzora_network::Networked>()
        .deny_component::<renzora_network::NetworkOwner>()
        .deny_component::<renzora_network::NetworkId>()
        .deny_physics_components()
        .extract_entities(descendants.into_iter())
        .build();

    // Strip the `ChildOf` components that point at the instance entity
    // (direct children) — in the source file those become root-level
    // entities, reparented to the instance on load by
    // `expand_scene_instances`.
    let instance_entity_field = instance_entity;
    for entity in &mut scene.entities {
        entity.components.retain(|component| {
            let type_name = component.reflect_type_path();
            // Same editor-only filters as save_scene.
            if type_name.starts_with("bevy_mod_outline::") {
                return false;
            }
            if type_name.starts_with("avian3d::") {
                return false;
            }
            // Drop ChildOf components that reference the instance entity.
            if type_name.ends_with("::ChildOf") || type_name == "bevy_ecs::hierarchy::ChildOf" {
                if let Some(reflect_any) = component.as_partial_reflect().try_as_reflect() {
                    // ChildOf has a single Entity field.
                    if let Some(co) = reflect_any.downcast_ref::<ChildOf>() {
                        if co.parent() == instance_entity_field {
                            return false;
                        }
                    }
                }
            }
            let registry = type_registry.read();
            let serializer = bevy::reflect::serde::TypedReflectSerializer::new(
                component.as_partial_reflect(),
                &registry,
            );
            ron::ser::to_string(&serializer).is_ok()
        });
    }

    let registry = type_registry.read();
    let serialized = BsnSerializer
        .serialize(&scene, &registry)
        .map_err(|e| format!("Prefab serialization failed: {e}"))?;

    if let Some(parent) = source_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(source_path, &serialized)?;
    console_info(
        "Scene",
        format!(
            "Saved prefab source to {} ({} entities)",
            source_path.display(),
            scene.entities.len()
        ),
    );
    Ok(())
}

/// Walk every `SceneInstance` in the world and write its descendant subtree
/// back to its source `.ron` file. Call this from the host scene save flow
/// so edits to nested entities propagate to their source prefab.
///
/// `host_scene_path` is the path of the scene currently being saved — used
/// to skip self-referencing instances (an instance of car.ron inside
/// car.ron would otherwise corrupt car.ron on save).
///
/// Instances that share a source path are also skipped (with a warning):
/// picking which copy's interior to push back is ambiguous, and a silent
/// last-write-wins would clobber edits made to the other copies.
pub fn save_all_scene_instances(world: &mut World, host_scene_path: &Path) {
    let project_path = match world.get_resource::<CurrentProject>() {
        Some(p) => p.path.clone(),
        None => return,
    };

    let instances: Vec<(Entity, String)> = {
        let mut q = world.query::<(Entity, &renzora::SceneInstance)>();
        q.iter(world)
            .map(|(e, inst)| (e, inst.source.clone()))
            .collect()
    };

    // Count how many instances share each source path so we can flag dupes.
    let mut source_counts: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    for (_, rel) in &instances {
        *source_counts.entry(rel.clone()).or_insert(0) += 1;
    }

    let host_canon = host_scene_path.canonicalize().ok();

    for (entity, source_rel) in instances {
        let source_abs = project_path.join(&source_rel);

        // Guard 1: self-reference. Saving an instance of the host scene
        // back into the host scene file would either clobber or recursively
        // inline it.
        let source_canon = source_abs.canonicalize().ok();
        if let (Some(host), Some(src)) = (&host_canon, &source_canon) {
            if host == src {
                console_warn(
                    "Scene",
                    format!(
                        "Skipping self-referencing instance → {} (source == host scene)",
                        source_rel
                    ),
                );
                continue;
            }
        }

        // Guard 2: multiple instances with the same source in this host.
        // We can't pick which interior to propagate, so skip all of them.
        if source_counts.get(&source_rel).copied().unwrap_or(0) > 1 {
            console_warn(
                "Scene",
                format!(
                    "Skipping instance {} — multiple instances share this source in the host; \
                 edit the source directly or unpack to propagate changes",
                    source_rel
                ),
            );
            continue;
        }

        if let Err(e) = save_prefab_source(world, entity, &source_abs) {
            console_error(
                "Scene",
                format!("Failed to save prefab source {}: {e}", source_abs.display()),
            );
        }
    }
}

/// Load the current project's main scene.
pub fn load_current_scene(world: &mut World) {
    let Some(project) = world.get_resource::<CurrentProject>() else {
        warn!("load_current_scene: no CurrentProject resource");
        return;
    };
    let path = project.main_scene_path();
    info!("load_current_scene: loading from {}", path.display());
    load_scene(world, &path);
}

// ============================================================================
// Rehydration
// ============================================================================

/// Rehydrate mesh primitives — spawns `Mesh3d` + `MeshMaterial3d` for entities that have
/// `MeshPrimitive` but no `Mesh3d` yet (e.g. after scene deserialization).
///
/// Entities that also carry a `MaterialRef` get only `Mesh3d` here — the
/// material resolver is the authority on their material. Inserting a
/// `StandardMaterial` alongside causes a command-ordering race where the
/// resolver's `MeshMaterial3d<GraphMaterial>` lands first, then this system
/// drops a fresh `StandardMaterial` on top, and Bevy ends up rendering the
/// wrong one (visible as the bright fallback color where the user expects
/// their custom shader). Symptom appeared as the runtime plane rendering
/// gray while the editor rendered correctly — the build's plugin set
/// changes the system schedule, which decides the race.
#[cfg(feature = "render_3d")]
pub fn rehydrate_meshes(
    mut commands: Commands,
    query: Query<
        (
            Entity,
            &MeshPrimitive,
            Option<&MeshColor>,
            Option<&renzora::core::MaterialRef>,
        ),
        Without<Mesh3d>,
    >,
    registry: Res<ShapeRegistry>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        return;
    };
    for (entity, primitive, color, material_ref) in &query {
        let Some(mesh) = registry.create_mesh(&primitive.0, &mut meshes) else {
            warn!("Unknown shape ID '{}' — skipping rehydration", primitive.0);
            continue;
        };

        if material_ref.is_some() {
            // Resolver will own the material. Inserting one here would race
            // against `resolve_material_refs` and clobber its result.
            commands.entity(entity).try_insert(Mesh3d(mesh));
            continue;
        }

        let base_color = color.map_or(Color::WHITE, |c| c.0);
        let material = materials.add(StandardMaterial {
            base_color,
            ..default()
        });

        commands
            .entity(entity)
            .try_insert((Mesh3d(mesh), MeshMaterial3d(material)));
    }
}

/// Loads `Sprite.image` from `SpriteImagePath` whenever the path
/// component is added or its string changes.
///
/// Two responsibilities:
/// 1. **Update existing sprite**: bind / clear the image handle and
///    swap placeholder-blue ↔ white as appropriate.
/// 2. **Re-create missing sprite**: Bevy 0.18's `Sprite` doesn't have
///    `#[reflect(Serialize, Deserialize)]`, so scene save drops it
///    entirely. On load, an entity carries `SpriteImagePath` and the
///    required components (Anchor, Transform, Visibility), but no
///    `Sprite` — nothing renders. Inserting a fresh `Sprite` with the
///    bound image (or placeholder colour for an empty path) restores
///    rendering. This is the rehydration path mirroring
///    `rehydrate_meshes` for `MeshPrimitive`.
pub fn on_sprite_image_path_inserted(
    trigger: On<Insert, renzora::core::SpriteImagePath>,
    paths: Query<&renzora::core::SpriteImagePath>,
    has_sprite: Query<(), With<bevy::sprite::Sprite>>,
    mut sprites_mut: Query<&mut bevy::sprite::Sprite>,
    asset_server: Res<AssetServer>,
    project: Option<Res<renzora::CurrentProject>>,
    mut commands: Commands,
) {
    let entity = trigger.entity;
    let Ok(path) = paths.get(entity) else {
        return;
    };
    let filter = sprite_filter(project.as_deref());
    if has_sprite.get(entity).is_ok() {
        apply_sprite_image_path(entity, &path.0, &mut sprites_mut, &asset_server, filter);
    } else {
        spawn_sprite_for_path(entity, &path.0, &asset_server, &mut commands, filter);
    }
}

/// Companion observer: when `Sprite` is inserted on an entity that
/// already has `SpriteImagePath`, bind the image. Catches the
/// reverse insert order — preset spawns and drag-drop both insert
/// Sprite and SpriteImagePath in the same bundle, so whichever
/// observer fires last finds the other component already present.
pub fn on_sprite_inserted_apply_image_path(
    trigger: On<Insert, bevy::sprite::Sprite>,
    paths: Query<&renzora::core::SpriteImagePath>,
    mut sprites_mut: Query<&mut bevy::sprite::Sprite>,
    asset_server: Res<AssetServer>,
    project: Option<Res<renzora::CurrentProject>>,
) {
    let entity = trigger.entity;
    let Ok(path) = paths.get(entity) else {
        return;
    };
    let filter = sprite_filter(project.as_deref());
    apply_sprite_image_path(entity, &path.0, &mut sprites_mut, &asset_server, filter);
}

/// Resolve the project's configured 2D image filter. Defaults to
/// `Nearest` when no project is loaded — keeps the behaviour
/// pixel-perfect by default.
fn sprite_filter(project: Option<&renzora::CurrentProject>) -> renzora::core::TextureFilter {
    project
        .map(|p| p.config.rendering_2d.image_filter)
        .unwrap_or_default()
}

fn apply_sprite_image_path(
    entity: Entity,
    path: &str,
    sprites_mut: &mut Query<&mut bevy::sprite::Sprite>,
    asset_server: &AssetServer,
    filter: renzora::core::TextureFilter,
) {
    let Ok(mut sprite) = sprites_mut.get_mut(entity) else {
        return;
    };
    let placeholder_blue = Color::srgba(0.5, 0.7, 1.0, 1.0);

    if path.is_empty() {
        if sprite.image != Handle::<Image>::default() {
            info!("[sprite] {:?} cleared image (empty path)", entity);
            sprite.image = Default::default();
            sprite.color = placeholder_blue;
            if sprite.custom_size.is_none() {
                sprite.custom_size = Some(Vec2::splat(100.0));
            }
        }
        return;
    }

    let expected = load_sprite_image(asset_server, path, filter);
    if sprite.image.id() != expected.id() {
        info!(
            "[sprite] {:?} bound image \"{}\" (replaced handle, filter={:?})",
            entity, path, filter
        );
        sprite.image = expected;
        sprite.color = Color::WHITE;
        // Use the image's native dimensions — Bevy reads them off the
        // loaded asset when `custom_size` is `None`. Forcing a fixed
        // size here would silently squash a 1024×1024 source into 100
        // world units, blowing away most of the pixel-art detail.
        sprite.custom_size = None;
    } else if sprite.color == placeholder_blue {
        sprite.color = Color::WHITE;
        sprite.custom_size = None;
    }
}

/// Load a sprite texture with the project's configured filter. Bevy's
/// default ImagePlugin uses linear filtering (right for 3D PBR, wrong
/// for pixel art — every scaled pixel becomes a smear). Per-asset
/// override via `load_with_settings` keeps 3D textures linear while
/// sprite textures land with whatever the project asks for.
fn load_sprite_image(
    asset_server: &AssetServer,
    path: &str,
    filter: renzora::core::TextureFilter,
) -> Handle<Image> {
    use bevy::image::{ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
    let descriptor = match filter {
        renzora::core::TextureFilter::Nearest => ImageSamplerDescriptor::nearest(),
        renzora::core::TextureFilter::Linear => ImageSamplerDescriptor::linear(),
    };
    asset_server.load_with_settings::<Image, ImageLoaderSettings>(
        path.to_owned(),
        move |settings: &mut ImageLoaderSettings| {
            settings.sampler = ImageSampler::Descriptor(descriptor.clone());
        },
    )
}

/// Insert a `Sprite` from scratch when one's missing. Used by the
/// rehydration path — reflection-loaded entities carry
/// `SpriteImagePath` but not `Sprite`. Defaults match the editor's
/// preset: 100×100 placeholder for empty path, white-tinted with
/// the loaded texture for a bound path.
fn spawn_sprite_for_path(
    entity: Entity,
    path: &str,
    asset_server: &AssetServer,
    commands: &mut Commands,
    filter: renzora::core::TextureFilter,
) {
    let placeholder_blue = Color::srgba(0.5, 0.7, 1.0, 1.0);
    let sprite = if path.is_empty() {
        bevy::sprite::Sprite {
            color: placeholder_blue,
            custom_size: Some(Vec2::splat(100.0)),
            ..Default::default()
        }
    } else {
        // `custom_size: None` → Bevy uses the loaded image's native
        // pixel dimensions, so a 32×32 source renders as 32 world units,
        // a 1024×1024 source as 1024. Critical for pixel art: forcing
        // a fixed size silently downsamples the source before our
        // viewport upscale, killing the crisp-pixel look.
        bevy::sprite::Sprite {
            color: Color::WHITE,
            custom_size: None,
            image: load_sprite_image(asset_server, path, filter),
            ..Default::default()
        }
    };
    info!(
        "[sprite] {:?} rehydrated Sprite component (path \"{}\", filter={:?})",
        entity, path, filter
    );
    commands.entity(entity).insert(sprite);
}

/// Ensure parent entities have `Visibility` so transform/visibility propagation works.
/// Fixes groups/empty parents that were saved without `Visibility`.
pub fn rehydrate_visibility(
    mut commands: Commands,
    query: Query<Entity, (With<Children>, Without<Visibility>)>,
) {
    for entity in &query {
        commands.entity(entity).try_insert(Visibility::default());
    }
}

/// Rehydrate scene cameras — spawns `Camera3d` for entities that have `SceneCamera` but no `Camera3d`.
///
/// In runtime mode (no editor), the `DefaultCamera` is active; if none is marked,
/// the first scene camera wins. All others are inactive.
/// In editor mode, all scene cameras are inactive (the editor camera renders).
/// In play mode, the default camera becomes the active play mode camera with the
/// viewport render target.
///
/// `Without<Camera2d>` is critical: `Camera2d` and `Camera3d` are mutually
/// exclusive markers and stacking them on the same entity makes Bevy pick
/// the 3D pipeline, breaking sprite rendering. Authored 2D scene cameras
/// stay 2D-only.
pub fn rehydrate_cameras(
    mut commands: Commands,
    query: Query<
        (Entity, Option<&DefaultCamera>),
        (With<SceneCamera>, Without<Camera3d>, Without<Camera2d>),
    >,
    play_mode: Option<Res<PlayModeState>>,
    render_target: Option<Res<ViewportRenderTarget>>,
    editor_session: Option<Res<renzora::EditorSession>>,
    mut mediums: Option<ResMut<Assets<ScatteringMedium>>>,
) {
    if query.is_empty() {
        return;
    }

    let in_play_mode = play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode());
    // We're in editor mode when the runtime `EditorSession` flag is set.
    //
    // This used to read `cfg!(feature = "editor")`, but after the
    // editor/runtime crate split `renzora_engine` compiles lean (no
    // `editor` cargo feature), so that flag is now ALWAYS false here —
    // which made rehydrate treat the editor like a runtime export and set
    // `is_active: true` on the default scene camera. Combined with a
    // play→stop cycle (which strips `Camera3d`, so the `Without<Camera3d>`
    // filter re-matches), that reactivated the scene camera to render the
    // whole scene to the primary window BEHIND the editor chrome — a
    // second full-scene pass on top of the editor camera, ~33% frame-time
    // regression with shadows on.
    //
    // `EditorSession` is inserted at boot by `add_engine_plugins(is_editor)`
    // (the same signal `RuntimePlugin` branches on), so it's already correct
    // when rehydrate runs during `SplashState::Loading` — no editor-camera
    // spawn race. Absent ⇒ game build ⇒ `false` (the safe shipping default).
    let is_editor = editor_session.as_ref().map(|s| s.0).unwrap_or(false) && !in_play_mode;

    // Find which entity should be the active camera in runtime mode
    let default_entity = query
        .iter()
        .find(|(_, dc)| dc.is_some())
        .or_else(|| query.iter().next())
        .map(|(e, _)| e);

    for (entity, _) in &query {
        let is_active = !is_editor && default_entity == Some(entity);

        commands.entity(entity).try_insert((
            Camera3d::default(),
            Camera {
                is_active,
                ..default()
            },
        ));

        // When this scene camera will be the active runtime camera (i.e.
        // we're not in editor mode and this is the default camera), it
        // needs the full prepass + atmosphere stack the editor camera
        // has. `Msaa::Off` is required because atmosphere binds depth
        // as non-multisampled. `DeferredPrepass` is added separately by
        // `ensure_deferred_prepass_on_cameras` (the single source of
        // truth for the Forward/Deferred toggle across editor, play
        // mode, and runtime) — that's why it's not in this tuple.
        //
        // Mirrors `renzora_viewport::play_mode::enter_play_mode` for the
        // 3D-only setup. (See `renzora_engine::camera::spawn_editor_camera`
        // for why all other prepass markers must be attached at spawn.)
        if is_active {
            let medium_handle = mediums
                .as_mut()
                .map(|m| m.add(ScatteringMedium::default()))
                .unwrap_or_default();
            commands.entity(entity).try_insert((
                Hdr,
                NormalPrepass,
                DepthPrepass,
                MotionVectorPrepass,
                AtmosphereEnvironmentMapLight {
                    intensity: 0.0,
                    ..default()
                },
                // ContactShadows intentionally omitted — see camera.rs (bevy 0.19
                // deferred + area_light_luts bind-group-layout conflict).
                Msaa::Off,
            ));
            // Per-view atmosphere render mode — bevy_pbr, render_3d only.
            #[cfg(feature = "render_3d")]
            commands
                .entity(entity)
                .insert(AtmosphereSettings::default());
            // 0.19: `Atmosphere` belongs on a dedicated world entity, never the
            // camera (else `world_to_atmosphere` rotates with the view and the
            // sky glitches on pan). Spawn a runtime sky for the active camera.
            // The entity's `GlobalTransform` IS the planet center, so place it
            // 6,360 km below the origin (surface at Y=0) and give it NO
            // `Transform` — a `Transform` would let propagation overwrite this
            // back to the origin (camera underground → no sky). Named-but-not-
            // `HideInHierarchy` so a scene clear recycles it instead of leaking
            // one per load. See `renzora_atmosphere::AtmospherePlanet`.
            commands.spawn((
                Atmosphere {
                    inner_radius: 6_360_000.0,
                    outer_radius: 6_460_000.0,
                    ground_albedo: Vec3::splat(0.3),
                    medium: medium_handle,
                },
                GlobalTransform::from(Transform::from_translation(Vec3::NEG_Y * 6_360_000.0)),
                Name::new("Sky Atmosphere"),
            ));
        }

        // During play mode, configure the default camera as the play mode camera
        // with the viewport render target (mirrors what enter_play_mode does).
        if in_play_mode && is_active {
            use renzora::console_log::*;
            let name = commands.entity(entity).id();
            console_info(
                "Rehydration",
                format!(
                    "Play mode active — configuring {:?} as play mode camera",
                    name
                ),
            );
            commands.entity(entity).try_insert(PlayModeCamera);
            if let Some(img) = render_target.as_ref().and_then(|rt| rt.image.as_ref()) {
                commands
                    .entity(entity)
                    .try_insert(bevy::camera::RenderTarget::Image(
                        Handle::<Image>::clone(img).into(),
                    ));
            }
        }
    }
}

/// Keeps `PlayModeState.active_game_camera` in sync when a scene transition
/// during play mode spawns a new `PlayModeCamera` entity.
pub fn sync_play_mode_camera(
    query: Query<Entity, Added<PlayModeCamera>>,
    play_mode: Option<ResMut<PlayModeState>>,
) {
    let Some(mut play_mode) = play_mode else {
        return;
    };
    for entity in &query {
        if play_mode.active_game_camera != Some(entity) {
            renzora::console_log::console_info(
                "PlayMode",
                format!(
                    "Play mode camera updated: {:?} -> {:?}",
                    play_mode.active_game_camera, entity
                ),
            );
            play_mode.active_game_camera = Some(entity);
        }
    }
}

/// Ensures only the default scene camera is active in runtime mode.
///
/// Runs every frame (cheap — early-exits if no changes). Handles cameras that
/// were deserialized with `Camera3d` already present (so `rehydrate_cameras` skipped them).
pub fn enforce_single_active_camera(
    mut cameras: Query<(Entity, &mut Camera, Option<&DefaultCamera>), With<SceneCamera>>,
    editor_camera: Query<(), With<EditorCamera>>,
    play_mode: Option<Res<PlayModeState>>,
) {
    if cameras.is_empty() {
        return;
    }

    let in_play_mode = play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode());
    let in_editor = !editor_camera.is_empty() && !in_play_mode;

    if in_editor {
        // Editor view: every scene camera should be inactive — the
        // editor camera owns the viewport. Without this pin, the
        // SceneCamera authored in `main.ron` ends up `is_active: true`
        // because Bevy auto-inserts a default `Camera` (which defaults
        // to active) when the scene reflects in its required-component
        // graph, and `try_insert` in `rehydrate_cameras` doesn't
        // always win that race. With this enforced, the whole scene
        // stops rendering twice in editor — recovers ~33% frame time
        // in heavy scenes with shadows on.
        for (_, mut camera, _) in &mut cameras {
            if camera.is_active {
                camera.is_active = false;
            }
        }
        return;
    }

    // Runtime / play mode: pick the DefaultCamera (or first scene
    // camera) and make sure only it is active.
    let default_entity = cameras
        .iter()
        .find(|(_, _, dc)| dc.is_some())
        .or_else(|| cameras.iter().next())
        .map(|(e, _, _)| e);

    for (entity, mut camera, _) in &mut cameras {
        let should_be_active = default_entity == Some(entity);
        if camera.is_active != should_be_active {
            camera.is_active = should_be_active;
        }
    }
}

fn should_sync(type_path: &str) -> bool {
    type_path.ends_with("Settings")
}

/// Tracks the previous sync state so we only log when something changes.
#[derive(Resource, Default)]
struct SceneCameraSyncState {
    prev_src: Option<Entity>,
    prev_synced: BTreeSet<&'static str>,
}

/// Sync post-process (and other reflected) components from the **default**
/// SceneCamera entity to the EditorCamera.
///
/// In editor mode the viewport renders through the EditorCamera, but users attach
/// effects to the SceneCamera entity. This system mirrors those components so they
/// take effect during editing.
///
/// Skipped during play mode (the play-mode camera receives effects via
/// `RenderTarget` + the individual `sync_*` systems).
pub fn sync_scene_camera_to_editor_camera(world: &mut World) {
    // Skip during play mode — effects route through RenderTarget instead.
    let is_playing = world
        .get_resource::<PlayModeState>()
        .is_some_and(|pm| pm.is_in_play_mode());
    if is_playing {
        return;
    }

    // Find the editor camera.
    let mut q = world.query_filtered::<Entity, With<EditorCamera>>();
    let editor_cam = q.iter(world).next();
    let Some(dst) = editor_cam else {
        return;
    };

    // Find the scene camera — prefer DefaultCamera, fall back to first SceneCamera.
    let mut default_cam = None;
    let mut first_cam = None;
    let mut q = world.query_filtered::<(Entity, Option<&DefaultCamera>), With<SceneCamera>>();
    for (e, dc) in q.iter(world) {
        if dc.is_some() {
            default_cam = Some(e);
            break;
        }
        if first_cam.is_none() {
            first_cam = Some(e);
        }
    }
    let scene_cam = default_cam.or(first_cam);

    // If no scene camera exists, remove all synced components from the editor camera.
    let Some(src) = scene_cam else {
        let type_registry = world.resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let mut to_remove: Vec<bevy::ecs::reflect::ReflectComponent> = Vec::new();
        let editor_ref = world.entity(dst);
        for reg in registry.iter() {
            let Some(reflect_component) = reg.data::<bevy::ecs::reflect::ReflectComponent>() else {
                continue;
            };
            let type_path = reg.type_info().type_path();
            if !should_sync(type_path) {
                continue;
            }
            if reflect_component.contains(FilteredEntityRef::from(editor_ref)) {
                to_remove.push(reflect_component.clone());
            }
        }
        drop(registry);
        for reflect_component in &to_remove {
            reflect_component.remove(&mut world.entity_mut(dst));
        }
        return;
    };

    if src == dst {
        return;
    }

    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let registry = type_registry.read();

    // Collect reflected component data from the scene camera.
    let mut components_to_sync: Vec<(bevy::ecs::reflect::ReflectComponent, Box<dyn Reflect>)> =
        Vec::new();
    let mut synced_type_paths: Vec<&'static str> = Vec::new();

    let entity_ref = world.entity(src);
    for reg in registry.iter() {
        let Some(reflect_component) = reg.data::<bevy::ecs::reflect::ReflectComponent>() else {
            continue;
        };
        let type_path = reg.type_info().type_path();
        if !should_sync(type_path) {
            continue;
        }
        if let Some(reflected) = reflect_component.reflect(FilteredEntityRef::from(entity_ref)) {
            if let Ok(cloned) = reflected.reflect_clone() {
                components_to_sync.push((reflect_component.clone(), cloned));
                synced_type_paths.push(type_path);
            }
        }
    }
    drop(registry);

    // Apply collected components to the editor camera.
    {
        let registry = type_registry.read();
        for (reflect_component, value) in &components_to_sync {
            let mut entity_mut = world.entity_mut(dst);
            if reflect_component.contains(entity_mut.as_readonly()) {
                reflect_component.apply(entity_mut, value.as_partial_reflect());
            } else {
                reflect_component.insert(&mut entity_mut, value.as_partial_reflect(), &registry);
            }
        }
    }

    // Remove components from editor camera that were removed from scene camera.
    let registry = type_registry.read();
    let mut to_remove: Vec<(bevy::ecs::reflect::ReflectComponent, &'static str)> = Vec::new();
    let editor_ref = world.entity(dst);
    for reg in registry.iter() {
        let Some(reflect_component) = reg.data::<bevy::ecs::reflect::ReflectComponent>() else {
            continue;
        };
        let type_path = reg.type_info().type_path();
        if !should_sync(type_path) {
            continue;
        }
        if reflect_component.contains(FilteredEntityRef::from(editor_ref))
            && !synced_type_paths.contains(&type_path)
        {
            to_remove.push((reflect_component.clone(), type_path));
        }
    }
    drop(registry);

    // Only log when the source camera or set of synced types actually changes.
    let current_set: BTreeSet<&'static str> = synced_type_paths.iter().copied().collect();
    let removed_paths: Vec<&str> = to_remove.iter().map(|(_, p)| *p).collect();

    let mut state = world
        .remove_resource::<SceneCameraSyncState>()
        .unwrap_or_default();

    let src_changed = state.prev_src != Some(src);
    let set_changed = state.prev_synced != current_set;
    let has_removals = !removed_paths.is_empty();

    if src_changed || set_changed || has_removals {
        crate::debug_log::log_scene_camera_sync(
            Some(src),
            Some(dst),
            &synced_type_paths,
            &removed_paths,
        );
        if src_changed {
            let src_name = world
                .get::<Name>(src)
                .map(|n| n.to_string())
                .unwrap_or_else(|| "unnamed".into());
            let has_default = world.get::<DefaultCamera>(src).is_some();
            renzora::console_log::console_info(
                "PostProcess",
                format!(
                    "Sync source camera: {:?} \"{}\" default={}",
                    src, src_name, has_default
                ),
            );
        }
        state.prev_src = Some(src);
        state.prev_synced = current_set;
    }

    world.insert_resource(state);

    for (reflect_component, _) in &to_remove {
        reflect_component.remove(&mut world.entity_mut(dst));
    }
}

/// Rehydrate mesh instances — loads GLTF scenes for entities with `MeshInstanceData`
/// but no children yet (i.e. the GLTF scene hasn't been spawned).
///
/// Triggers on `Added<MeshInstanceData>` (scene load). Skips entities that already
/// have children (e.g. model_drop already spawned the SceneRoot child).
#[cfg(feature = "render_3d")]
pub fn rehydrate_mesh_instances(
    mut commands: Commands,
    query: Query<
        (Entity, &MeshInstanceData),
        (
            Without<Children>,
            Without<PendingMeshInstanceRehydrate>,
            Added<MeshInstanceData>,
        ),
    >,
    asset_server: Res<AssetServer>,
) {
    for (entity, instance) in &query {
        let Some(ref model_path) = instance.model_path else {
            continue;
        };

        let gltf_handle: Handle<Gltf> = asset_server.load(model_path.clone());

        // We need to wait for the GLTF to load before spawning the scene.
        // Insert a pending-load marker so a follow-up system can spawn the scene child.
        commands
            .entity(entity)
            .try_insert(PendingMeshInstanceRehydrate(gltf_handle));
    }
}

/// Marker: waiting for GLTF to finish loading so we can attach the scene child.
#[derive(Component)]
#[cfg(feature = "render_3d")]
pub struct PendingMeshInstanceRehydrate(pub Handle<Gltf>);

/// Marker: the mesh instance's GLB asset failed to load — typically because the
/// model file was deleted (or renamed/moved) while the editor was closed, so the
/// scene still references a path that no longer exists. The asset never lands in
/// `Assets<Gltf>`, so loading-progress systems must treat the entity as resolved
/// instead of waiting on it forever (which hangs the loading screen).
#[derive(Component)]
pub struct MeshInstanceLoadFailed;

/// Finishes mesh-instance rehydration once the GLTF asset is ready.
#[cfg(feature = "render_3d")]
pub fn finish_mesh_instance_rehydrate(
    mut commands: Commands,
    query: Query<(Entity, &PendingMeshInstanceRehydrate)>,
    gltf_assets: Option<Res<Assets<Gltf>>>,
    asset_server: Res<AssetServer>,
) {
    let Some(gltf_assets) = gltf_assets else {
        return;
    };
    for (entity, pending) in &query {
        let Some(gltf) = gltf_assets.get(&pending.0) else {
            // Not in the store yet — still loading, or the load failed. A
            // failed load (missing/deleted file) never produces a `Gltf`, so
            // without this branch the entity keeps `PendingMeshInstanceRehydrate`
            // forever and the loading screen never advances. Detect the failure,
            // drop the pending marker, and tag it so progress systems count it
            // as done.
            if matches!(
                asset_server.get_load_state(pending.0.id()),
                Some(bevy::asset::LoadState::Failed(_))
            ) {
                warn!(
                    "[scene] model failed to load (missing or deleted?), skipping: {}",
                    asset_server
                        .get_path(pending.0.id())
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "<unknown>".into())
                );
                commands
                    .entity(entity)
                    .remove::<PendingMeshInstanceRehydrate>()
                    .insert(MeshInstanceLoadFailed);
            }
            continue;
        };

        let scene_handle = gltf
            .default_scene
            .clone()
            .or_else(|| gltf.scenes.first().cloned());

        if let Some(scene) = scene_handle {
            commands.spawn((
                Name::new("SceneRoot"),
                // Bevy 0.19: glTF scenes are `Handle<WorldAsset>` and are
                // instantiated via `WorldAssetRoot` (the old `SceneRoot` is gone).
                bevy::world_serialization::WorldAssetRoot(scene),
                Transform::default(),
                Visibility::default(),
                ChildOf(entity),
            ));
        }

        commands
            .entity(entity)
            .remove::<PendingMeshInstanceRehydrate>();
    }
}

/// Rehydrate sun entities — syncs `DirectionalLight` + `Transform` from `Sun` on newly added entities.
pub fn rehydrate_suns(mut query: Query<(&Sun, &mut DirectionalLight, &mut Transform), Added<Sun>>) {
    for (sun, mut light, mut transform) in &mut query {
        light.color = Color::srgb(sun.color.x, sun.color.y, sun.color.z);
        light.illuminance = sun.illuminance;
        light.shadow_maps_enabled = sun.shadows_enabled;
        *transform =
            Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, sun.direction()));
    }
}

/// Backfill required components on light entities loaded from a scene.
///
/// Bevy's `DynamicScene::write_to_world` inserts components via reflection,
/// which doesn't run the required-components machinery. Lights deserialize
/// fine but often arrive missing `Transform`, `Visibility`, etc — the
/// dependent components that `#[require(...)]` would normally auto-insert
/// when the light was first added via Commands. Without those, the light
/// either doesn't render or renders at world origin.
///
/// This system runs every frame and patches in any missing companions on
/// freshly-loaded light entities. Cheap when there are no lights to fix
/// (the `Without<...>` filters keep the query empty).
pub fn rehydrate_lights(
    mut commands: Commands,
    needs_transform: Query<
        Entity,
        (
            Or<(
                With<bevy::light::PointLight>,
                With<bevy::light::SpotLight>,
                With<bevy::light::DirectionalLight>,
            )>,
            Without<Transform>,
        ),
    >,
    needs_visibility: Query<
        Entity,
        (
            Or<(
                With<bevy::light::PointLight>,
                With<bevy::light::SpotLight>,
                With<bevy::light::DirectionalLight>,
            )>,
            Without<Visibility>,
        ),
    >,
) {
    for entity in &needs_transform {
        commands.entity(entity).try_insert(Transform::default());
    }
    for entity in &needs_visibility {
        commands.entity(entity).try_insert(Visibility::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // extract_unregistered_type
    // ------------------------------------------------------------------

    #[test]
    fn extract_type_basic_backtick_form() {
        let msg = "no registration found for `my::crate::Foo`";
        assert_eq!(
            extract_unregistered_type(msg),
            Some("my::crate::Foo".to_string())
        );
    }

    #[test]
    fn extract_type_with_type_keyword() {
        // Tolerate the `... for type \`T\`` variant.
        let msg = "no registration found for type `bevy::pbr::StandardMaterial`";
        assert_eq!(
            extract_unregistered_type(msg),
            Some("bevy::pbr::StandardMaterial".to_string())
        );
    }

    #[test]
    fn extract_type_embedded_in_larger_message() {
        let msg = "deserialization error at line 5: no registration found for `a::B`, aborting";
        assert_eq!(extract_unregistered_type(msg), Some("a::B".to_string()));
    }

    #[test]
    fn extract_type_returns_none_when_pattern_absent() {
        assert_eq!(extract_unregistered_type("some unrelated error"), None);
    }

    #[test]
    fn extract_type_returns_none_without_closing_backtick() {
        let msg = "no registration found for `unterminated";
        assert_eq!(extract_unregistered_type(msg), None);
    }

    #[test]
    fn extract_type_returns_none_without_opening_backtick() {
        let msg = "no registration found for plainname";
        assert_eq!(extract_unregistered_type(msg), None);
    }

    // ------------------------------------------------------------------
    // strip_component_entry
    // ------------------------------------------------------------------

    #[test]
    fn strip_middle_entry_keeps_neighbors() {
        let ron = "(\n  \"a::A\": (x: 1),\n  \"b::B\": (y: 2),\n  \"c::C\": (z: 3),\n)";
        let out = strip_component_entry(ron, "b::B").expect("entry should be found");
        assert!(!out.contains("b::B"), "stripped key must be gone");
        assert!(out.contains("a::A"), "preceding entry must remain");
        assert!(out.contains("c::C"), "following entry must remain");
        // No back-to-back commas left behind.
        assert!(!out.contains(",,"));
    }

    #[test]
    fn strip_entry_with_nested_parens() {
        // The closing paren of the target must be found via balanced-paren
        // walking, not the first ')' encountered.
        let ron = "(\n  \"t::T\": (inner: (a: 1, b: (2)), tail: 9),\n  \"u::U\": (k: 0),\n)";
        let out = strip_component_entry(ron, "t::T").expect("entry should be found");
        assert!(!out.contains("t::T"));
        assert!(out.contains("u::U"));
        // The inner data of u::U must survive intact.
        assert!(out.contains("k: 0"));
    }

    #[test]
    fn strip_entry_with_paren_inside_string_literal() {
        // A ')' inside a quoted string must not be treated as the closing
        // paren of the entry.
        let ron = "(\n  \"s::S\": (label: \"a ) b ( c\"),\n  \"v::V\": (n: 1),\n)";
        let out = strip_component_entry(ron, "s::S").expect("entry should be found");
        assert!(!out.contains("s::S"));
        assert!(out.contains("v::V"));
        assert!(out.contains("n: 1"));
    }

    #[test]
    fn strip_entry_with_escaped_quote_in_string() {
        // An escaped quote must not prematurely end the string scan.
        let ron = "(\n  \"e::E\": (txt: \"x \\\" ) still in string\"),\n  \"w::W\": (m: 2),\n)";
        let out = strip_component_entry(ron, "e::E").expect("entry should be found");
        assert!(!out.contains("e::E"));
        assert!(out.contains("w::W"));
        assert!(out.contains("m: 2"));
    }

    #[test]
    fn strip_returns_none_for_missing_key() {
        let ron = "(\n  \"a::A\": (x: 1),\n)";
        assert_eq!(strip_component_entry(ron, "z::Z"), None);
    }

    #[test]
    fn strip_returns_none_on_unbalanced_parens() {
        // Opening paren but never closed -> paren matching fails -> None.
        let ron = "(\n  \"a::A\": (x: 1,\n";
        assert_eq!(strip_component_entry(ron, "a::A"), None);
    }

    #[test]
    fn strip_last_entry_is_removed() {
        let ron = "(\n  \"a::A\": (x: 1),\n  \"b::B\": (y: 2),\n)";
        let out = strip_component_entry(ron, "b::B").expect("entry should be found");
        assert!(!out.contains("b::B"));
        assert!(out.contains("a::A"));
    }

    // ------------------------------------------------------------------
    // extract_scene_instance_sources
    // ------------------------------------------------------------------

    #[test]
    fn extract_sources_single() {
        let text = r#"
        "renzora::core::SceneInstance": (
            source: "scenes/car.ron",
        ),
        "#;
        assert_eq!(
            extract_scene_instance_sources(text),
            vec!["scenes/car.ron".to_string()]
        );
    }

    #[test]
    fn extract_sources_multiple_in_order() {
        let text = concat!(
            "\"renzora::core::SceneInstance\": ( source: \"a.ron\" ),\n",
            "other stuff,\n",
            "\"renzora::core::SceneInstance\": ( source: \"b.ron\" ),\n",
        );
        assert_eq!(
            extract_scene_instance_sources(text),
            vec!["a.ron".to_string(), "b.ron".to_string()]
        );
    }

    #[test]
    fn extract_sources_none_when_marker_absent() {
        let text = "\"some::Other\": ( source: \"x.ron\" )";
        assert!(extract_scene_instance_sources(text).is_empty());
    }

    #[test]
    fn extract_sources_empty_string_source() {
        let text = "\"renzora::core::SceneInstance\": ( source: \"\" )";
        assert_eq!(
            extract_scene_instance_sources(text),
            vec![String::new()]
        );
    }

    #[test]
    fn extract_sources_tab_whitespace_before_quote() {
        let text = "\"renzora::core::SceneInstance\": (source:\t\"tabbed.ron\")";
        assert_eq!(
            extract_scene_instance_sources(text),
            vec!["tabbed.ron".to_string()]
        );
    }

    // ------------------------------------------------------------------
    // paths_equal / is_self_reference
    // ------------------------------------------------------------------

    #[test]
    fn paths_equal_identical_uncanonicalizable_paths() {
        // Non-existent paths can't canonicalize, so the fallback is a
        // direct == comparison.
        let a = Path::new("/nonexistent/scenes/foo.ron");
        let b = Path::new("/nonexistent/scenes/foo.ron");
        assert!(paths_equal(a, b));
        assert!(is_self_reference(a, b));
    }

    #[test]
    fn paths_not_equal_different_uncanonicalizable_paths() {
        let a = Path::new("/nonexistent/scenes/foo.ron");
        let b = Path::new("/nonexistent/scenes/bar.ron");
        assert!(!paths_equal(a, b));
        assert!(!is_self_reference(a, b));
    }
}
