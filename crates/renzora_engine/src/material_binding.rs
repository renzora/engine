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
///
/// Splits the string itself rather than going through `std::path`, and
/// normalizes separators *before* the split. `Path::parent` is
/// platform-sensitive: on Linux a backslash is an ordinary filename
/// character, so a Windows-style `models\bistro\bistro.glb` is a single
/// component with no parent, and normalizing afterwards is too late to
/// recover the directory. That matters beyond the test that caught it —
/// an asset path is recorded by whichever machine did the import, so a
/// project authored on Windows has to resolve on a Linux runtime, where
/// the fallback would silently bind every mesh to a bare `materials/`
/// that isn't there. A wrong `MaterialRef` fails quietly, leaving the raw
/// glTF material in place, which is the exact failure this module exists
/// to prevent.
fn materials_dir_for(model_path: &str) -> String {
    let normalized = model_path.replace('\\', "/");
    let model_dir = normalized.rsplit_once('/').map_or("", |(dir, _)| dir);
    if model_dir.is_empty() {
        "materials".to_string()
    } else {
        format!("{}/materials", model_dir)
    }
}

/// The `.material` a mesh with the authored glTF material `material_name`
/// binds to, or `None` when the importer never wrote one.
///
/// Only an imported model has a `materials/` folder: a `.glb` copied into the
/// project by hand, by a tool or from the engine's presets renders with the
/// materials inside it and nothing else. Binding it anyway pointed every mesh
/// at a file that is not there, which the resolver reports as an error, once
/// per mesh entity, so a scattered preset filled the Console with thousands
/// of them. With no file the mesh keeps the glTF material it already wears,
/// which is the right material for such a model.
///
/// `exists` answers for an asset-relative path, so the test needs no project.
fn bound_material_path(
    materials_dir_rel: &str,
    material_name: &str,
    exists: impl FnOnce(&str) -> bool,
) -> Option<String> {
    let path = format!(
        "{}/{}.material",
        materials_dir_rel,
        sanitize_material_name(material_name)
    );
    exists(&path).then_some(path)
}

/// Whether the `.material` at asset-relative `path` can be read, resolved the
/// way `renzora_shader`'s resolver resolves a `MaterialRef`, so a file this
/// says is there is one the resolver can open: on disk in the editor, in the
/// archive in a packed game.
fn material_file_exists(
    path: &str,
    project: Option<&renzora::CurrentProject>,
    reader: &renzora::VirtualFileReader,
) -> bool {
    let fs_path = match project {
        Some(project) if !std::path::Path::new(path).is_absolute() => {
            let raw = project.resolve_path(path).to_string_lossy().replace('\\', "/");
            raw.strip_prefix("./").unwrap_or(&raw).to_string()
        }
        _ => path.to_string(),
    };
    reader.read_string(&fs_path).is_some()
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
    project: Option<Res<renzora::CurrentProject>>,
    file_reader: Option<Res<renzora::VirtualFileReader>>,
) {
    let default_reader = renzora::VirtualFileReader::default();
    let reader = file_reader.as_deref().unwrap_or(&default_reader);
    // One read per material per frame, not per mesh: a model's meshes mostly
    // share a handful of materials.
    let mut exists: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
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
                // Bind to `<material name>.material`, the same name
                // `extract_glb_materials` used for the file, when that file
                // was written. Otherwise (no authored name, or a model that
                // was never imported) mark it done so it is not retried.
                let bound = mat_name.and_then(|name| {
                    bound_material_path(&materials_dir_rel, &name.0, |path| {
                        *exists
                            .entry(path.to_string())
                            .or_insert_with(|| material_file_exists(path, project.as_deref(), reader))
                    })
                });
                match bound {
                    Some(path) => {
                        commands
                            .entity(entity)
                            .insert((renzora::MaterialRef(path), MaterialBindingDone));
                    }
                    None => {
                        commands.entity(entity).insert(MaterialBindingDone);
                    }
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

    /// A model copied in rather than imported has no `materials/` folder, so
    /// its meshes keep their glTF material instead of naming a missing file.
    #[test]
    fn binds_only_to_a_material_the_importer_wrote() {
        let written = ["models/rock/materials/Moss_Rock.material"];
        let exists = |path: &str| written.contains(&path);
        assert_eq!(
            bound_material_path("models/rock/materials", "Moss Rock", exists),
            Some("models/rock/materials/Moss_Rock.material".to_string())
        );
        assert_eq!(bound_material_path("models/materials", "boulder_01", exists), None);
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
