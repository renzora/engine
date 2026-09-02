//! Nested scenes — `SceneInstance` expansion, prefab write-back, and the
//! reference-cycle guard that keeps a scene from instancing itself.
//!
//! A scene holding an instance of itself is the failure this module is shaped
//! around: it does not crash, it loads twice, duplicating every entity
//! including the camera. The guard therefore lives in
//! [`spawn_scene_instance`] itself rather than only at the three call sites
//! that used to check it — a guard three callers must remember is one a fourth
//! will not.

use bevy::prelude::*;
use renzora::console_log::*;
use renzora::CurrentProject;
use renzora_bsn::bsn::{BsnSerializer, SceneSerializer};
use renzora_bsn::DynamicSceneBuilder;
use std::path::Path;

use super::deny::{DenyOptionalSubsystems, DenyUiCameraTargets};
use super::load::{load_scene, loading_stack_contains};
use super::save::restore_viewport_gated_visibility;

/// Expand every `SceneInstance` entity in the world that has no children yet:
/// load the referenced source scene and reparent its roots under the instance.
///
/// Re-runnable: already-expanded instances (any with children) are skipped.
pub fn expand_scene_instances(world: &mut World) {

    let project_root = world
        .get_resource::<CurrentProject>()
        .map(|p| p.path.clone());

    // While world streaming is in effect (shipped game / editor play), a
    // `streamed` instance's expansion is owned by the distance driver
    // (`scene_stream::drive_streamed_scene_instances`) — eagerly expanding it
    // here would defeat the point of streaming. In editor edit mode streamed
    // instances expand like ordinary ones so designers can author them.
    let skip_streamed = renzora::world_streaming_active(world);

    let mut to_expand: Vec<(Entity, std::path::PathBuf)> = Vec::new();
    {
        let mut q = world.query::<(Entity, &renzora::SceneInstance)>();
        for (entity, inst) in q.iter(world) {
            if skip_streamed && inst.streamed {
                continue;
            }
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
            // Skip if this scene is already being loaded further up the stack —
            // a cycle, including the case where a scene instances *itself*.
            let in_stack = loading_stack_contains(&abs);
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
        // No push here — `load_scene` records what it is loading, which is what
        // makes a self-referencing instance detectable. Pushing here as well
        // would put the path on the stack before `load_scene` checked it, and
        // every nested instance would look like a cycle.
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
    }
}

/// Returns `true` if `source_path` resolves to the same file as
/// `host_scene_path` — i.e. a direct self-reference. Used by drop handlers
/// to reject a scene being dropped into itself.
pub fn is_self_reference(host_scene_path: &Path, source_path: &Path) -> bool {
    paths_equal(host_scene_path, source_path)
}

pub fn paths_equal(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => a == b,
    }
}

/// Cache of scene → outgoing scene references, keyed by canonical path with
/// mtime validation. Populated lazily by [`would_create_reference_cycle`] and
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

    /// Return the outgoing `SceneInstance.source` list for `path`, reading and
    /// scanning from disk only if the cache is missing or stale.
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
/// from the referenced .ron files. Backed by [`SceneReferenceCache`] —
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

/// Spawn a new `SceneInstance` entity that references `source_path` and
/// immediately expand it by loading the source scene's entities under it.
///
/// `source_path` is absolute; it's stored as an asset-relative string.
/// Returns the newly-spawned instance root entity, or `None` if no project
/// is open (paths can't be resolved).
pub fn spawn_scene_instance(
    world: &mut World,
    source_path: &Path,
    parent: Option<Entity>,
    transform: Transform,
) -> Option<Entity> {
    // Refuse a cycle here, not only at the drop handlers.
    //
    // Three call sites check `would_create_reference_cycle` before calling this
    // — the hierarchy's context menu, the hierarchy drop and the viewport drop —
    // and any fourth way in gets no check at all. That is how a scene ended up
    // holding an instance of itself: it loaded twice on every open, duplicating
    // every entity including the camera, and nothing crashed because the loader
    // caught the *inner* recursion. A guard three callers have to remember is
    // one a fourth will not.
    let host_and_root = world
        .get_resource::<CurrentProject>()
        .map(|p| (p.main_scene_path(), p.path.clone()));
    if let Some((host, root)) = host_and_root {
        let cycles = world.resource_scope(|_w, mut cache: Mut<SceneReferenceCache>| {
            would_create_reference_cycle(&mut cache, &root, &host, source_path)
        });
        if cycles {
            warn!(
                "[scene] refusing to instance {} into {} — it would form a reference cycle",
                source_path.display(),
                host.display()
            );
            return None;
        }
    }

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
        renzora::SceneInstance {
            source: relative,
            ..Default::default()
        },
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
        // Children's GlobalTransform reflects the instance root's world-space
        // position. If serialized, it would "bake in" the host's placement
        // for anyone opening car.ron standalone. Bevy recomputes it each
        // frame from Transform anyway.
        .deny_component::<GlobalTransform>()
        .deny_component::<bevy::transform::components::TransformTreeChanged>()
        .deny_component::<bevy::camera::primitives::Aabb>()
        // Runtime mirror of the camera's projection, rebuilt every frame.
        .deny_component::<crate::camera_script::CameraReadState>()
        .deny_component::<bevy::render::sync_world::SyncToRenderWorld>()
        .deny_component::<bevy::input::gamepad::Gamepad>()
        .deny_component::<bevy::input::gamepad::GamepadSettings>()
        .deny_animation_state()
        .deny_network_components()
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
            if type_name.starts_with("avian3d::") || type_name.starts_with("avian2d::") {
                return false;
            }
            // Gaussian-splat runtime components are resolved on load from the
            // serializable renzora::GaussianSplat — same filter as `save_scene`.
            if type_name.starts_with("bevy_gaussian_splatting::") {
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

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // Streaming-field backward compatibility
    // ------------------------------------------------------------------

    /// Scenes saved before `SceneInstance` grew its streaming fields carry
    /// only `source:` — they MUST still deserialize (via `#[reflect(default)]`)
    /// with streaming off, or every pre-existing nested-scene reference in
    /// user projects silently drops its component on load.
    #[test]
    fn scene_instance_without_streaming_fields_gets_defaults() {
        use renzora_bsn::bsn::{BsnSerializer, SceneSerializer};

        let atr = bevy::ecs::reflect::AppTypeRegistry::default();
        atr.write().register::<renzora::SceneInstance>();

        // Serialize a current-format instance, then strip the streaming
        // fields from the text — the exact shape of a scene saved before
        // those fields existed.
        let mut src = World::new();
        src.insert_resource(atr.clone());
        let e = src
            .spawn(renzora::SceneInstance {
                source: "scenes/a.bsn".into(),
                ..Default::default()
            })
            .id();
        let scene = DynamicSceneBuilder::from_world(&src)
            .extract_entity(e)
            .build();
        let text = {
            let reg = atr.read();
            BsnSerializer.serialize(&scene, &reg).expect("serialize")
        };
        // Cut from the comma before `streamed` to the component's closing
        // paren — format-agnostic (compact vs pretty RON both survive).
        let start = text
            .find(",streamed")
            .or_else(|| text.find(", streamed"))
            .expect("serialized SceneInstance should contain the streamed field");
        let end = start
            + text[start..]
                .find(')')
                .expect("component value should close with a paren");
        let old_format = format!("{}{}", &text[..start], &text[end..]);
        assert!(
            !old_format.contains("streamed"),
            "test setup failed to strip the new fields — serializer output \
             changed shape?\n{text}"
        );

        let (scene, skipped) = {
            let reg = atr.read();
            BsnSerializer
                .deserialize_lossy(&old_format, &reg)
                .expect("old-format SceneInstance must deserialize")
        };
        assert!(
            skipped.is_empty(),
            "SceneInstance was skipped instead of defaulting: {skipped:?}"
        );

        let mut world = World::new();
        world.insert_resource(atr.clone());
        let mut map = bevy::ecs::entity::EntityHashMap::default();
        scene.write_to_world(&mut world, &mut map).expect("write");
        let entity = *map.values().next().expect("one entity");
        let inst = world
            .get::<renzora::SceneInstance>(entity)
            .expect("SceneInstance present");
        assert_eq!(inst.source, "scenes/a.bsn");
        assert_eq!(inst.load_radius, 150.0);
        assert_eq!(inst.unload_radius, 200.0);
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
