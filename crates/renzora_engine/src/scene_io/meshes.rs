//! Mesh rehydration — rebuilding what a save deliberately did not store.
//!
//! A scene persists the *description* (a `MeshPrimitive` id, an `EditedMesh`
//! blob, a `MeshInstanceData` model path) and never the built `Mesh3d`. These
//! systems turn each description back into renderable geometry after a load.

use bevy::prelude::*;
#[cfg(all(feature = "render_3d", feature = "gltf"))]
use renzora_scene_api::MeshInstanceData;

#[cfg(feature = "render_3d")]
use renzora_scene_api::{MeshColor, MeshPrimitive, ShapeRegistry};

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
            Option<&renzora_material_api::MaterialRef>,
        ),
        Without<Mesh3d>,
    >,
    registry: Res<ShapeRegistry>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    grid: Option<Res<renzora_render_api::GridTexture>>,
    mut shared: ResMut<crate::blockout::BlockoutMaterials>,
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
            //
            // But the resolver is asked again: `MaterialResolved` comes off.
            // A mesh rebuilt here on an entity that already had its material
            // (its shape changed, or its mesh was removed) drew nothing at
            // all, because the renderer had dropped the entity's material
            // with the old mesh and nothing sent the unchanged material again.
            // Re-resolving hits the cache and re-inserts the same material,
            // which the renderer then picks up.
            commands
                .entity(entity)
                .try_insert(Mesh3d(mesh))
                .remove::<renzora_material_api::MaterialResolved>();
            continue;
        }

        let base_color = color.map_or(Color::WHITE, |c| c.0);
        // Same "no texture yet" blockout grid the shape was spawned with — a
        // reloaded untextured primitive shouldn't come back flat. Only
        // `MeshColor` is serialized, so this system *is* the material after a
        // reload: anything it gets wrong shows up as shapes changing appearance
        // when you save and open the scene again.
        // Shared with every primitive of this colour, so they batch.
        let material = shared.get(base_color, &mut materials, grid.as_deref());

        commands
            .entity(entity)
            .try_insert((Mesh3d(mesh), MeshMaterial3d(material)));
    }
}

/// Rebuild a shape whose [`MeshPrimitive`] was changed after it was built: a
/// cylinder set to `torus` in the Inspector, or by a tool or script.
///
/// The mesh is built once, from the id, when the entity has none, so changing
/// the id alone did nothing until the scene was reloaded. Dropping the mesh
/// hands it back to [`rehydrate_meshes`], which builds the new shape next
/// frame exactly as a load would. A newly added id is left alone: that is the
/// ordinary build, not a change.
#[cfg(feature = "render_3d")]
pub fn rebuild_changed_primitives(
    mut commands: Commands,
    changed: Query<(Entity, Ref<MeshPrimitive>), (Changed<MeshPrimitive>, With<Mesh3d>)>,
) {
    for (entity, primitive) in &changed {
        if primitive.is_added() {
            continue;
        }
        commands.entity(entity).remove::<Mesh3d>();
    }
}

/// What an entity's mesh comes from: a library shape, or an imported model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MeshSource {
    /// A [`MeshPrimitive`] id from the shape registry, such as `torus`.
    Shape(String),
    /// A project-relative model file: `.glb`, `.gltf` or `.rvg`.
    Model(String),
}

/// What `entity`'s mesh comes from now, if it has one of either kind.
#[cfg(all(feature = "render_3d", feature = "gltf"))]
pub fn mesh_source(world: &World, entity: Entity) -> Option<MeshSource> {
    if let Some(p) = world.get::<MeshPrimitive>(entity) {
        return Some(MeshSource::Shape(p.0.clone()));
    }
    world
        .get::<MeshInstanceData>(entity)
        .and_then(|m| m.model_path.clone())
        .map(MeshSource::Model)
}

/// Give `entity` a different mesh, a shape or a model, keeping everything
/// else about it: name, transform, scripts, physics, its place in the
/// hierarchy.
///
/// Built the way a loaded scene is: the old source's pieces come off, the new
/// one's description goes on, and the ordinary rehydration builds it (a
/// shape's mesh next frame, a model once its file has loaded). So what a swap
/// produces is exactly what reopening the scene produces.
///
/// Leaving a model clears everything under the entity. That is all the
/// model's own content: a save leaves out every descendant of a model, so
/// nothing of the user's can live there across a save. Becoming a model is
/// refused for an entity with children of its own, for the same reason in
/// reverse: they would be dropped from the scene at the next save, silently.
#[cfg(all(feature = "render_3d", feature = "gltf"))]
pub fn set_mesh_source(world: &mut World, entity: Entity, source: MeshSource) -> Result<(), String> {
    if world.get_entity(entity).is_err() {
        return Err("that entity no longer exists".into());
    }
    if mesh_source(world, entity).as_ref() == Some(&source) {
        return Ok(());
    }
    let was_model = world.get::<MeshInstanceData>(entity).is_some();
    let has_children = world.get::<Children>(entity).is_some_and(|c| !c.is_empty());
    if matches!(source, MeshSource::Model(_)) && !was_model && has_children {
        return Err(
            "it has children of its own, and a model's children are not saved with the scene; \
             move them out first"
                .into(),
        );
    }

    // The old model's content, all of it (see above).
    if was_model {
        let kids: Vec<Entity> = world.get::<Children>(entity).map(|c| c.to_vec()).unwrap_or_default();
        for kid in kids {
            if let Ok(e) = world.get_entity_mut(kid) {
                e.despawn();
            }
        }
    }

    match source {
        MeshSource::Shape(id) => {
            let mut e = world.entity_mut(entity);
            e.remove::<(
                MeshInstanceData,
                PendingMeshInstanceRehydrate,
                MeshInstanceLoadFailed,
                crate::material_binding::ImportedRoot,
            )>();
            // A shape that had a colour keeps it; one coming from a model
            // starts white, as a new shape does.
            if e.get::<MeshColor>().is_none() {
                e.insert(MeshColor(Color::WHITE));
            }
            match e.get_mut::<MeshPrimitive>() {
                // Changing the id: `rebuild_changed_primitives` drops the old
                // mesh and rehydration builds the new one.
                Some(mut p) => p.0 = id,
                None => {
                    e.remove::<Mesh3d>();
                    e.insert(MeshPrimitive(id));
                }
            }
        }
        MeshSource::Model(path) => {
            let handle: Handle<Gltf> = world.resource::<AssetServer>().load(path.clone());
            let mut e = world.entity_mut(entity);
            // A shape's own mesh and material: the model's meshes bring
            // theirs, each bound to its own `.material`.
            // `ImportedRoot` comes off too: its absence is how the spawn
            // observer recognises a model that has just loaded and binds its
            // materials, so a model swapped for another kept it and the new
            // one was never bound.
            e.remove::<(
                MeshPrimitive,
                MeshColor,
                Mesh3d,
                MeshMaterial3d<StandardMaterial>,
                renzora_material_api::MaterialRef,
                renzora_material_api::MaterialResolved,
                MeshInstanceLoadFailed,
                crate::material_binding::ImportedRoot,
                crate::material_binding::PendingMaterialBinding,
            )>();
            e.insert((MeshInstanceData { model_path: Some(path) }, PendingMeshInstanceRehydrate(handle)));
        }
    }
    Ok(())
}

/// Apply persisted mesh edits after scene load: entities carrying an
/// [`renzora_scene_api::EditedMesh`] without the applied marker get a fresh
/// `Mesh3d` built from the stored geometry. Runs after [`rehydrate_meshes`]
/// (chained at registration) so the edit override deterministically replaces
/// the primitive/glTF mesh. The editor inserts the marker itself when baking,
/// so live edits never round-trip through here.
#[cfg(feature = "render_3d")]
pub fn apply_edited_meshes(
    mut commands: Commands,
    query: Query<
        (Entity, &renzora_scene_api::EditedMesh),
        Without<renzora_scene_api::EditedMeshApplied>,
    >,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
) {
    let Some(meshes) = meshes.as_mut() else {
        return;
    };
    for (entity, edited) in &query {
        if edited.positions.len() < 9 || edited.indices.len() < 3 {
            // Degenerate payload — don't build an empty mesh over the source.
            commands
                .entity(entity)
                .try_insert(renzora_scene_api::EditedMeshApplied);
            continue;
        }
        let handle = meshes.add(edited.to_mesh());
        commands
            .entity(entity)
            .try_insert((Mesh3d(handle), renzora_scene_api::EditedMeshApplied));
    }
}

/// Rehydrate mesh instances — loads GLTF scenes for entities with `MeshInstanceData`
/// but no children yet (i.e. the GLTF scene hasn't been spawned).
///
/// Triggers on `Added<MeshInstanceData>` (scene load). Skips entities that already
/// have children (e.g. model_drop already spawned the SceneRoot child).
#[cfg(feature = "render_3d")]
#[cfg(feature = "gltf")]
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
#[cfg(feature = "gltf")]
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
#[cfg(feature = "gltf")]
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

        commands
            .entity(entity)
            .remove::<PendingMeshInstanceRehydrate>();
        if let Some(scene) = scene_handle {
            // Spawned only if the model still exists when the command runs.
            // A streamed model can be despawned between this system and its
            // commands applying (its cell streamed out); a plain spawn with
            // `ChildOf` to the dead model then lost the link and left a named
            // "SceneRoot" at the top of the scene, drawing a stray copy of the
            // model at the origin and saved with the scene from then on.
            commands.queue(move |world: &mut World| {
                if world.get_entity(entity).is_err() {
                    return;
                }
                world.spawn((
                    Name::new("SceneRoot"),
                    // Bevy 0.19: glTF scenes are `Handle<WorldAsset>` and are
                    // instantiated via `WorldAssetRoot` (the old `SceneRoot` is gone).
                    bevy::world_serialization::WorldAssetRoot(scene),
                    Transform::default(),
                    Visibility::default(),
                    ChildOf(entity),
                ));
            });
        } else {
            // Loaded, but nothing to place: the `.rvg` kept beside a `.glb`
            // holds only that model's virtual geometry and loads as an empty
            // model, and a glTF may have no scene at all. No scene root ever
            // fills, so the loading overlay waited on it forever; counted as
            // failed, it is done.
            warn!(
                "[scene] model has no scene to place, skipping: {}",
                asset_server
                    .get_path(pending.0.id())
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "<unknown>".into())
            );
            commands.entity(entity).insert(MeshInstanceLoadFailed);
        }
    }
}

#[cfg(all(test, feature = "render_3d", feature = "gltf"))]
mod tests {
    use super::*;

    /// A model turned into a shape loses the model's content and becomes the
    /// shape, white since it had no colour of its own.
    #[test]
    fn a_model_becomes_a_shape() {
        let mut world = World::new();
        let model = world
            .spawn((Name::new("chair"), MeshInstanceData { model_path: Some("models/chair.glb".into()) }))
            .id();
        let part = world.spawn((Name::new("leg"), ChildOf(model))).id();

        set_mesh_source(&mut world, model, MeshSource::Shape("torus".into())).expect("swap");
        assert!(world.get_entity(part).is_err(), "the model's content stayed");
        assert!(world.get::<MeshInstanceData>(model).is_none());
        assert_eq!(world.get::<MeshPrimitive>(model).map(|p| p.0.as_str()), Some("torus"));
        assert_eq!(world.get::<MeshColor>(model).map(|c| c.0), Some(Color::WHITE));
        assert_eq!(mesh_source(&world, model), Some(MeshSource::Shape("torus".into())));
    }

    /// A shape with children of its own is not made a model: a model's
    /// children are left out of a save, so they would be lost.
    #[test]
    fn a_shape_with_children_is_not_made_a_model() {
        let mut world = World::new();
        let shape = world.spawn((Name::new("crate"), MeshPrimitive("cube".into()))).id();
        world.spawn((Name::new("lid"), ChildOf(shape)));
        let result = set_mesh_source(&mut world, shape, MeshSource::Model("models/chair.glb".into()));
        assert!(result.is_err());
        assert_eq!(world.get::<MeshPrimitive>(shape).map(|p| p.0.as_str()), Some("cube"));
    }
}
