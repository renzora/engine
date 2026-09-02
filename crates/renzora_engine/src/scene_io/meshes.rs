//! Mesh rehydration — rebuilding what a save deliberately did not store.
//!
//! A scene persists the *description* (a `MeshPrimitive` id, an `EditedMesh`
//! blob, a `MeshInstanceData` model path) and never the built `Mesh3d`. These
//! systems turn each description back into renderable geometry after a load.

use bevy::prelude::*;
use renzora::MeshInstanceData;

#[cfg(feature = "render_3d")]
use renzora::{MeshColor, MeshPrimitive, ShapeRegistry};

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
    grid: Option<Res<renzora::core::GridTexture>>,
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
        // Same "no texture yet" blockout grid the shape was spawned with — a
        // reloaded untextured primitive shouldn't come back flat. Only
        // `MeshColor` is serialized, so this system *is* the material after a
        // reload: anything it gets wrong shows up as shapes changing appearance
        // when you save and open the scene again.
        let material = materials.add(crate::blockout::blockout_material(
            base_color,
            grid.as_deref(),
        ));

        commands
            .entity(entity)
            .try_insert((Mesh3d(mesh), MeshMaterial3d(material)));
    }
}

/// Apply persisted mesh edits after scene load: entities carrying an
/// [`renzora::core::EditedMesh`] without the applied marker get a fresh
/// `Mesh3d` built from the stored geometry. Runs after [`rehydrate_meshes`]
/// (chained at registration) so the edit override deterministically replaces
/// the primitive/glTF mesh. The editor inserts the marker itself when baking,
/// so live edits never round-trip through here.
#[cfg(feature = "render_3d")]
pub fn apply_edited_meshes(
    mut commands: Commands,
    query: Query<
        (Entity, &renzora::core::EditedMesh),
        Without<renzora::core::EditedMeshApplied>,
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
                .try_insert(renzora::core::EditedMeshApplied);
            continue;
        }
        let handle = meshes.add(edited.to_mesh());
        commands
            .entity(entity)
            .try_insert((Mesh3d(handle), renzora::core::EditedMeshApplied));
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
