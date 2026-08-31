//! Bind imported glTF meshes to the `.material` files the import pipeline wrote.
//!
//! # Why this lives in the runtime crate
//!
//! The importer extracts every glTF material into an editable
//! `<model_dir>/materials/<name>.material` graph, but nothing in a `.glb`
//! points back at those files. The link is re-established at spawn time by
//! walking the spawned hierarchy and tagging each mesh with a
//! [`renzora::MaterialRef`] derived from its authored glTF material name;
//! `renzora_shader`'s resolver then compiles the file and swaps out the
//! `StandardMaterial` Bevy's glTF loader attached.
//!
//! This whole chain used to live in `renzora_viewport`, which is an **editor**
//! plugin — so it never ran in a shipped game or in window/external play mode.
//! Every `.material` the importer wrote was, at runtime, a dead file, and the
//! game rendered whatever `bevy_gltf` had made of the raw glTF instead.
//!
//! For a model authored in core metal-rough that difference is invisible: the
//! raw material and the extracted graph say the same thing. For a model using
//! `KHR_materials_pbrSpecularGlossiness` it is total. Bevy does not implement
//! that extension, and exporters that use it often omit `pbrMetallicRoughness`
//! entirely — so the base colour texture (`diffuseTexture`, inside the
//! extension) is invisible to Bevy, and the missing block falls back to the
//! glTF defaults of `metallicFactor: 1.0`, `roughnessFactor: 1.0`. The result
//! is a scene of untextured white metal: correct in the editor, washed out in
//! the game, which is exactly how the bug presented.
//!
//! The editor still owns *hierarchy* concerns (flattening pass-through nodes,
//! hiding wrappers); only the material half moved here, so a game and the
//! editor viewport now agree on what a model looks like.

use bevy::gltf::{Gltf, GltfMaterialName};
use bevy::pbr::MeshMaterial3d;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;

use renzora::core::MeshInstanceData;

/// Marker on the top-level entity of an imported model. The gizmo and
/// animation tooling use this as the default "grab the whole thing" target,
/// and the spawn observers use its absence to tell a freshly-loaded model
/// from one that has already been decorated.
#[derive(Component, Debug, Clone, Copy)]
pub struct ImportedRoot;

/// Tracks a freshly-spawned glTF model that still needs its mesh entities
/// bound to `MaterialRef` components. Held on the [`ImportedRoot`] entity.
/// The `Handle<Gltf>` keeps the asset alive long enough for the binder to
/// run.
///
/// The marker lives on the parent for the entire life of the model — the
/// binder is idempotent (its query filter excludes already-bound meshes),
/// so the descendant walk is free once everything has been bound, and any
/// late-spawned mesh from Bevy's incremental scene spawner gets caught the
/// frame it appears.
#[derive(Component)]
pub struct PendingMaterialBinding {
    pub gltf_handle: Handle<Gltf>,
}

/// Marker: this mesh entity has already been processed by the material
/// binder (it either got a `MaterialRef` or it has no extractable material).
/// Prevents repeat work on subsequent frames while the binding is still
/// pending for sibling meshes.
#[derive(Component)]
pub struct MaterialBindingDone;

/// Sanitize a material name for use as a filename. Mirrors
/// `renzora_shader::material::on_pbr_material_extracted` so binding paths
/// agree with the writer.
pub fn sanitize_material_name(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if safe.is_empty() {
        "material".to_string()
    } else {
        safe
    }
}

/// The `<model_dir>/materials` directory a model's extracted `.material`
/// files live in, as a project-relative asset path.
fn materials_dir_for(model_path: &str) -> String {
    let model_dir = std::path::Path::new(model_path)
        .parent()
        .and_then(|p| p.to_str())
        .map(|s| s.replace('\\', "/"))
        .unwrap_or_default();
    if model_dir.is_empty() {
        "materials".to_string()
    } else {
        format!("{}/materials", model_dir)
    }
}

/// Arm material binding for a scene that Bevy has just finished spawning.
///
/// `scene_root` is the entity holding the world-asset root — the child
/// `finish_mesh_instance_rehydrate` (runtime) or `renzora_scene`'s equivalent
/// (editor) spawned under the `MeshInstanceData` entity. Walks up to that
/// parent, marks it [`ImportedRoot`], and attaches [`PendingMaterialBinding`]
/// so [`bind_material_refs`] picks it up.
///
/// Returns the `MeshInstanceData` parent when it armed one, so the editor's
/// observer can hang its own (hierarchy-flattening) work off the same signal
/// without duplicating the walk.
pub fn arm_material_binding(
    scene_root: Entity,
    commands: &mut Commands,
    asset_server: &AssetServer,
    parents: &Query<&ChildOf>,
    mesh_instances: &Query<&MeshInstanceData, Without<ImportedRoot>>,
) -> Option<Entity> {
    if scene_root == Entity::PLACEHOLDER {
        return None;
    }

    // Scene-root child → `MeshInstanceData` parent. If the bearer isn't a
    // child, this isn't a model-load scene at all — bail.
    let parent = parents.get(scene_root).ok()?.parent();
    let mesh_instance = mesh_instances.get(parent).ok()?;

    let Some(model_path) = mesh_instance.model_path.clone() else {
        // No GLB to bind. Mark it imported anyway so the `Without<ImportedRoot>`
        // filter keeps us out of any future ready event this entity fires.
        commands.entity(parent).try_insert(ImportedRoot);
        return None;
    };

    // Bevy hands back the same handle for a path already loaded; calling
    // load again is a refcount bump on the cached asset.
    let gltf_handle: Handle<Gltf> = asset_server.load(model_path);
    commands
        .entity(parent)
        .try_insert((ImportedRoot, PendingMaterialBinding { gltf_handle }));
    Some(parent)
}

/// Observer: bring scene-loaded model instances onto the material-binding
/// path the moment Bevy finishes spawning the glTF hierarchy.
///
/// Polling on "has children" races Bevy's incremental spawner — the scene
/// child appears the same frame the asset lands while `write_to_world` is
/// still in flight. The ready event fires exactly once, after every entity
/// in the scene is committed to the world.
///
/// This is the game-side registration; the editor registers
/// `renzora_viewport`'s observer instead, which calls the same helper and
/// additionally arms the hierarchy flatten pass.
pub fn bind_scene_models_on_ready(
    trigger: On<WorldInstanceReady>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    parents: Query<&ChildOf>,
    mesh_instances: Query<&MeshInstanceData, Without<ImportedRoot>>,
) {
    arm_material_binding(
        trigger.event().entity,
        &mut commands,
        &asset_server,
        &parents,
        &mesh_instances,
    );
}

/// System: walks each [`PendingMaterialBinding`] model, finds its mesh
/// descendants, and inserts a `MaterialRef` pointing at the per-material
/// `.material` file the import pipeline wrote.
///
/// Runs every frame for as long as the marker exists. Bevy's scene spawner
/// populates large GLBs incrementally — a city block can take dozens of
/// frames to fully spawn, with new mesh entities appearing throughout. An
/// earlier "found one mesh → done" version left most of those meshes
/// unbound, so this just keeps going. The work is idempotent: the query
/// filter excludes meshes that already carry `MaterialRef` /
/// [`MaterialBindingDone`], so a fully-bound model costs one descendant walk
/// per frame and zero binds. The marker disappears when the parent does.
pub fn bind_material_refs(
    mut commands: Commands,
    pending_query: Query<(Entity, &PendingMaterialBinding, &MeshInstanceData)>,
    children_query: Query<&Children>,
    // Bevy 0.19: glTF materials became a separate `GltfMaterial` asset, so the
    // mesh's `StandardMaterial` AssetId no longer matches `gltf.materials` ids.
    // Bevy instead tags each mesh entity with `GltfMaterialName` (the authored
    // material name), which we match directly — more robust than the old
    // by-id map. `MeshMaterial3d` is kept only to detect "has a material".
    mesh_mat_query: Query<
        (&MeshMaterial3d<StandardMaterial>, Option<&GltfMaterialName>),
        (
            With<Mesh3d>,
            Without<MaterialBindingDone>,
            Without<renzora::MaterialRef>,
        ),
    >,
    gltf_assets: Res<Assets<Gltf>>,
) {
    for (root_entity, pending, mesh_data) in pending_query.iter() {
        if gltf_assets.get(&pending.gltf_handle).is_none() {
            // GLB still loading. Wait — `PendingMaterialBinding` holds the
            // handle so the asset is kept alive.
            continue;
        }

        // No `model_path` means there's nothing to bind to; the marker is
        // useless on this entity, so drop it.
        let Some(model_path) = mesh_data.model_path.as_deref() else {
            commands
                .entity(root_entity)
                .remove::<PendingMaterialBinding>();
            continue;
        };
        let materials_dir_rel = materials_dir_for(model_path);

        // Walk descendants and bind any meshes not yet bound. The query
        // filter skips already-bound meshes, so once every descendant has
        // been processed this loop is effectively a no-op.
        let mut stack: Vec<Entity> = vec![root_entity];
        while let Some(entity) = stack.pop() {
            if let Ok(kids) = children_query.get(entity) {
                stack.extend(kids.iter());
            }
            if let Ok((_mat, mat_name)) = mesh_mat_query.get(entity) {
                if let Some(mat_name) = mat_name {
                    // Bind to `<material name>.material` — the same name
                    // `extract_glb_materials` used for the file.
                    let safe = sanitize_material_name(&mat_name.0);
                    let path = format!("{}/{}.material", materials_dir_rel, safe);
                    commands
                        .entity(entity)
                        .insert((renzora::MaterialRef(path), MaterialBindingDone));
                } else {
                    // Mesh has a material but no authored glTF name. Mark it
                    // done so we don't keep retrying it.
                    commands.entity(entity).insert(MaterialBindingDone);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materials_dir_sits_beside_the_model() {
        assert_eq!(
            materials_dir_for("models/bistro/bistro.glb"),
            "models/bistro/materials"
        );
    }

    #[test]
    fn materials_dir_handles_a_bare_filename() {
        assert_eq!(materials_dir_for("bistro.glb"), "materials");
    }

    #[test]
    fn materials_dir_normalizes_backslashes() {
        assert_eq!(
            materials_dir_for("models\\bistro\\bistro.glb"),
            "models/bistro/materials"
        );
    }

    /// The binder and the importer have to agree on the filename or every
    /// `MaterialRef` points at a file that isn't there — which fails silently,
    /// leaving the raw glTF material in place.
    #[test]
    fn sanitize_matches_the_writer() {
        assert_eq!(sanitize_material_name("bat1-structure"), "bat1-structure");
        assert_eq!(sanitize_material_name("Wood Floor.001"), "Wood_Floor_001");
        assert_eq!(sanitize_material_name(""), "material");
    }
}
