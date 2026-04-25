//! Drag-and-drop model spawning — detects asset drops on the viewport and
//! spawns GLTF/GLB models into the scene.
//!
//! While a model is being dragged over the viewport, a flat-grey "ghost"
//! preview is loaded mesh-only (no textures) and follows the cursor so the
//! user can see placement before the slow textured load runs. The ghost is
//! discarded on drop and replaced by the standard textured spawn pipeline.

use std::path::PathBuf;

use bevy::asset::LoadState;
use bevy::camera::primitives::Aabb;
use bevy::pbr::MeshMaterial3d;
use bevy::prelude::*;
use bevy::scene::SceneRoot;
use bevy::window::PrimaryWindow;
use bevy_egui::egui;

use renzora_animation::{AnimClipSlot, AnimatorComponent};
use renzora::core::{CurrentProject, EditorCamera, MeshInstanceData};
use renzora_editor_framework::{EditorCommands, EditorSelection};
use renzora_ui::asset_drag::AssetDragPayload;

use crate::model_flatten::{ImportedRoot, PendingFlatten};
use crate::ViewportState;

/// Extensions accepted as droppable 3D models.
pub const MODEL_EXTENSIONS: &[&str] = &["glb", "gltf"];

/// Resource tracking pending GLTF loads that need to be spawned once ready.
#[derive(Resource, Default)]
pub struct PendingGltfLoads {
    pub loads: Vec<PendingLoad>,
}

pub struct PendingLoad {
    pub handle: Handle<Gltf>,
    pub name: String,
    pub asset_path: String,
    pub spawn_position: Vec3,
}

/// Marker component — entity needs its Y adjusted so the bottom sits on the ground.
#[derive(Component)]
pub struct NeedsGroundAlignment {
    pub target_y: f32,
}

/// State for the ghost preview shown while a model is being dragged over the
/// viewport. Cleared when the drag ends (drop or cancel).
#[derive(Resource, Default)]
pub struct ModelDragPreviewState {
    /// Source path from the asset drag payload (used to detect a new drag).
    pub origin_path: Option<PathBuf>,
    /// Asset-relative path passed to the asset server.
    pub asset_path: Option<String>,
    /// Display name carried over to the real entity on drop.
    pub name: Option<String>,
    /// Mesh-only Gltf handle (loaded with `load_materials` empty).
    pub mesh_handle: Option<Handle<Gltf>>,
    /// Spawned ghost root entity. `None` until the mesh-only load completes.
    pub ghost_root: Option<Entity>,
    /// Shared flat-grey material applied to all ghost meshes.
    pub ghost_material: Option<Handle<StandardMaterial>>,
    /// Last known cursor ground position (Y=0 plane).
    pub ground_position: Vec3,
    /// True when the cursor is currently over the viewport rect.
    pub cursor_in_viewport: bool,
}

impl ModelDragPreviewState {
    /// Wipe everything except `ghost_material` — that handle is cheap to keep
    /// around and will be reused for the next drag if it happens.
    pub fn clear(&mut self) {
        self.origin_path = None;
        self.asset_path = None;
        self.name = None;
        self.mesh_handle = None;
        self.ghost_root = None;
        self.ground_position = Vec3::ZERO;
        self.cursor_in_viewport = false;
    }
}

/// Marker on the spawned ghost root entity.
#[derive(Component)]
pub struct ModelDragGhost;

/// Marker on a ghost mesh entity whose material has already been overridden,
/// so the swap system doesn't keep re-applying it every frame.
#[derive(Component)]
pub struct GhostMaterialApplied;

/// Marker: animation discovery has been attempted for this entity (hit or
/// miss). Prevents `auto_discover_animations` from re-scanning the
/// filesystem on every frame for models that have no `.anim` files.
#[derive(Component)]
pub struct AnimationDiscoveryDone;

/// Called from the viewport panel's `ui()` method (read-only `&World`).
///
/// Detects when a model asset is being dragged over the viewport and, on release,
/// queues a deferred command to initiate loading.
pub fn check_viewport_model_drop(ui: &mut egui::Ui, world: &World, viewport_rect: egui::Rect) {
    let Some(payload) = world.get_resource::<AssetDragPayload>() else {
        return;
    };
    if !payload.is_detached || !payload.matches_extensions(MODEL_EXTENSIONS) {
        return;
    }

    let pointer_pos = ui.ctx().pointer_latest_pos();
    let pointer_in_viewport = pointer_pos.map_or(false, |p| viewport_rect.contains(p));

    if !pointer_in_viewport {
        return;
    }

    // Check if the pointer was just released (= drop)
    let pointer_released = !ui.ctx().input(|i| i.pointer.any_down());
    if !pointer_released {
        return;
    }

    let path = payload.path.clone();
    let name = payload.name.clone();

    // Capture viewport info for ground-position computation in the deferred closure
    let screen_pos = pointer_pos.unwrap_or(viewport_rect.center());
    let vp_rect = viewport_rect;

    // Queue the spawn command (deferred — runs with &mut World)
    if let Some(commands) = world.get_resource::<EditorCommands>() {
        commands.push(move |world: &mut World| {
            // Prefer the ground position the ghost was tracking — it matches
            // exactly what the user saw under their cursor at drop time.
            let preview_pos = world
                .get_resource::<ModelDragPreviewState>()
                .filter(|s| s.origin_path.as_deref() == Some(path.as_path()))
                .map(|s| s.ground_position);
            let ground_pos = preview_pos
                .or_else(|| compute_ground_position(world, screen_pos, vp_rect))
                .unwrap_or(Vec3::ZERO);

            despawn_ghost(world);
            initiate_model_load(world, path, name, ground_pos);
        });
    }
}

/// Despawn the ghost entity and clear the preview state. Safe to call when
/// no ghost exists.
fn despawn_ghost(world: &mut World) {
    let ghost = world
        .get_resource_mut::<ModelDragPreviewState>()
        .and_then(|mut s| {
            let g = s.ghost_root.take();
            s.clear();
            g
        });
    if let Some(entity) = ghost {
        if let Ok(entity_mut) = world.get_entity_mut(entity) {
            entity_mut.despawn();
        }
    }
}

/// Compute a world-space ground position (Y=0 plane) from a screen-space pointer.
fn compute_ground_position(
    world: &mut World,
    screen_pos: egui::Pos2,
    viewport_rect: egui::Rect,
) -> Option<Vec3> {
    // Query the editor camera
    let mut q = world.query_filtered::<(&GlobalTransform, &Camera), With<EditorCamera>>();
    let (camera_transform, camera) = q.iter(world).next()?;
    let camera_transform = *camera_transform;
    let camera = camera.clone();

    // Convert screen position to render-target coordinates
    let vp_state = world.get_resource::<ViewportState>()?;
    let vp_x = screen_pos.x - viewport_rect.min.x;
    let vp_y = screen_pos.y - viewport_rect.min.y;
    let render_x = vp_x / viewport_rect.width() * vp_state.current_size.x as f32;
    let render_y = vp_y / viewport_rect.height() * vp_state.current_size.y as f32;

    let ray = camera.viewport_to_world(&camera_transform, Vec2::new(render_x, render_y)).ok()?;

    // Intersect with Y=0 ground plane
    if ray.direction.y.abs() < 1e-6 {
        return Some(Vec3::new(ray.origin.x, 0.0, ray.origin.z));
    }
    let t = -ray.origin.y / ray.direction.y;
    if t < 0.0 {
        return Some(Vec3::ZERO);
    }
    let hit = ray.origin + ray.direction * t;
    Some(Vec3::new(hit.x, 0.0, hit.z))
}

/// Initiate loading a model file — called from a deferred `EditorCommands` closure.
fn initiate_model_load(world: &mut World, path: PathBuf, name: String, spawn_position: Vec3) {
    // Compute asset-relative path. Each model gets its own folder under
    // `assets/models/<stem>/` so derived assets (animations, textures,
    // materials) from the proper import pipeline stay grouped with it.
    let asset_path = if let Some(project) = world.get_resource::<CurrentProject>() {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("model")
            .to_string();
        let model_dir = project.path.join("models").join(&stem);
        let _ = std::fs::create_dir_all(&model_dir);

        let file_name = path.file_name().unwrap_or_default();
        let dest = model_dir.join(file_name);

        if !dest.exists() || path.canonicalize().ok() != dest.canonicalize().ok() {
            if let Err(e) = std::fs::copy(&path, &dest) {
                error!("Failed to copy model to project: {}", e);
            } else {
                info!("Copied model to project: {:?}", dest);
            }
        }

        project.make_asset_relative(&dest)
    } else {
        path.to_string_lossy().replace('\\', "/")
    };

    // Load via AssetServer
    let handle: Handle<Gltf> = world.resource::<AssetServer>().load(&asset_path);

    info!("Loading model '{}' from '{}'", name, asset_path);

    world
        .resource_mut::<PendingGltfLoads>()
        .loads
        .push(PendingLoad {
            handle,
            name,
            asset_path,
            spawn_position,
        });
}

/// System: poll pending GLTF loads, spawn entities when ready.
pub fn spawn_loaded_gltfs(
    mut commands: Commands,
    mut pending: ResMut<PendingGltfLoads>,
    gltf_assets: Res<Assets<Gltf>>,
    selection: Res<EditorSelection>,
    project: Option<Res<CurrentProject>>,
) {
    let mut completed = Vec::new();

    for (index, load) in pending.loads.iter().enumerate() {
        let Some(gltf) = gltf_assets.get(&load.handle) else {
            continue;
        };

        // Pick the default scene, or the first scene
        let scene_handle = gltf
            .default_scene
            .clone()
            .or_else(|| gltf.scenes.first().cloned());

        let Some(scene) = scene_handle else {
            warn!("GLTF '{}' has no scenes", load.name);
            completed.push(index);
            continue;
        };

        let transform = Transform::from_translation(load.spawn_position);

        // Spawn the MeshInstance parent entity
        let parent = commands
            .spawn((
                Name::new(load.name.clone()),
                transform,
                Visibility::default(),
                MeshInstanceData {
                    model_path: Some(load.asset_path.clone()),
                },
                ImportedRoot,
            ))
            .id();

        // Auto-discover .anim files and attach AnimatorComponent
        if let Some(animator) = discover_animation_clips(&load.asset_path, project.as_deref()) {
            let clip_count = animator.clips.len();
            commands.entity(parent).insert(animator);
            info!(
                "Attached AnimatorComponent with {} clip(s) to '{}'",
                clip_count, load.name
            );
        }

        // Spawn the GLTF scene as a child. PendingFlatten triggers the
        // flatten pass once the scene spawner has populated the subtree.
        commands.spawn((
            Name::new("SceneRoot"),
            bevy::scene::SceneRoot(scene),
            Transform::default(),
            Visibility::default(),
            ChildOf(parent),
            PendingFlatten::default(),
        ));

        // Attach ground alignment marker
        commands.entity(parent).insert(NeedsGroundAlignment {
            target_y: load.spawn_position.y,
        });

        // Auto-select the new entity
        selection.set(Some(parent));

        info!("Spawned model '{}' at {:?}", load.name, load.spawn_position);
        completed.push(index);
    }

    // Remove completed loads in reverse order
    for index in completed.into_iter().rev() {
        pending.loads.remove(index);
    }
}

/// Look for `.anim` files in an `animations/` directory next to the model and build
/// an `AnimatorComponent` from them. Returns `None` if no `.anim` files are found.
fn discover_animation_clips(
    asset_path: &str,
    project: Option<&CurrentProject>,
) -> Option<AnimatorComponent> {
    let project = project?;
    // Model is e.g. "models/Man.glb" → look in "models/animations/"
    let model_dir = std::path::Path::new(asset_path).parent().unwrap_or(std::path::Path::new(""));
    let anim_dir_abs = project.path.join(model_dir).join("animations");

    if !anim_dir_abs.is_dir() {
        return None;
    }

    let mut clips = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&anim_dir_abs)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map_or(false, |ext| ext == "anim")
        })
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let file_path = entry.path();
        let stem = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("clip")
            .to_string();

        // Asset-relative path: e.g. "models/animations/HumanArmature_Man_Idle.anim"
        let anim_asset_path = model_dir
            .join("animations")
            .join(entry.file_name())
            .to_string_lossy()
            .replace('\\', "/");

        clips.push(AnimClipSlot {
            name: stem,
            path: anim_asset_path,
            looping: true,
            speed: 1.0,
            blend_in: None,
            blend_out: None,
        });
    }

    if clips.is_empty() {
        return None;
    }

    let default_clip = clips
        .iter()
        .find(|c| c.name.to_lowercase().contains("idle"))
        .or(clips.first())
        .map(|c| c.name.clone());

    Some(AnimatorComponent {
        clips,
        default_clip,
        blend_duration: 0.2,
        state_machine: None,
        layers: Vec::new(),
    })
}

/// System: once child meshes have AABBs, offset the parent so its bottom sits on the ground.
pub fn align_models_to_ground(
    mut commands: Commands,
    query: Query<(Entity, &NeedsGroundAlignment, &Children)>,
    children_query: Query<&Children>,
    aabb_query: Query<(&Aabb, &GlobalTransform)>,
    mut transform_query: Query<&mut Transform>,
) {
    for (entity, alignment, children) in query.iter() {
        let mut lowest_y: Option<f32> = None;

        // Walk all descendants looking for AABBs
        let mut stack: Vec<Entity> = children.iter().collect();
        while let Some(child) = stack.pop() {
            if let Ok((aabb, global_transform)) = aabb_query.get(child) {
                let center = Vec3::from(aabb.center);
                let half = Vec3::from(aabb.half_extents);

                // Check all 8 AABB corners in world space
                for sx in [-1.0f32, 1.0] {
                    for sy in [-1.0f32, 1.0] {
                        for sz in [-1.0f32, 1.0] {
                            let corner: Vec3 = center + half * Vec3::new(sx, sy, sz);
                            let world_pos: Vec3 = global_transform.transform_point(corner);
                            lowest_y = Some(lowest_y.map_or(world_pos.y, |prev: f32| prev.min(world_pos.y)));
                        }
                    }
                }
            }

            if let Ok(grandchildren) = children_query.get(child) {
                stack.extend(grandchildren.iter());
            }
        }

        if let Some(lowest_world_y) = lowest_y {
            let offset = alignment.target_y - lowest_world_y;
            if let Ok(mut transform) = transform_query.get_mut(entity) {
                transform.translation.y += offset;
            }
            commands.entity(entity).remove::<NeedsGroundAlignment>();
        }
    }
}

/// System: auto-discover `.anim` files for entities loaded from scenes that have
/// `MeshInstanceData` (a model) but no `AnimatorComponent` yet.
pub fn auto_discover_animations(
    mut commands: Commands,
    query: Query<
        (Entity, &MeshInstanceData),
        (Without<AnimatorComponent>, Without<renzora_animation::AnimatorState>, Without<AnimationDiscoveryDone>),
    >,
    project: Option<Res<CurrentProject>>,
) {
    let Some(ref project) = project else { return };

    for (entity, mesh_data) in query.iter() {
        let Some(ref model_path) = mesh_data.model_path else {
            commands.entity(entity).insert(AnimationDiscoveryDone);
            continue;
        };

        if let Some(animator) = discover_animation_clips(model_path, Some(project)) {
            let clip_count = animator.clips.len();
            commands.entity(entity).insert(animator);
            info!(
                "Auto-discovered {} animation clip(s) for '{}'",
                clip_count, model_path
            );
        }
        commands.entity(entity).insert(AnimationDiscoveryDone);
    }
}

// ── Drag-time mesh-only preview ────────────────────────────────────────────

/// System: track the active model drag, kick off the mesh-only Gltf load the
/// first time it enters the viewport, and update the cursor ground position
/// every frame.
pub fn track_model_drag_preview(
    mut state: ResMut<ModelDragPreviewState>,
    payload: Option<Res<AssetDragPayload>>,
    asset_server: Res<AssetServer>,
    project: Option<Res<CurrentProject>>,
    viewport: Res<ViewportState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // No payload (or wrong kind) → leave any existing ghost alone; cleanup
    // runs in its own system once the resource is removed.
    let Some(payload) = payload else {
        state.cursor_in_viewport = false;
        return;
    };
    if !payload.is_detached || !payload.matches_extensions(MODEL_EXTENSIONS) {
        state.cursor_in_viewport = false;
        return;
    }

    // First time we've seen this drag — try to start a mesh-only load. We
    // only do this once `is_detached` is true to avoid loading on every
    // accidental click.
    if state.origin_path.as_deref() != Some(payload.path.as_path()) {
        // Drop any stale state from a previous drag (the cleanup system
        // already handles entity despawn when the payload disappears).
        state.clear();
        // Mark this path as evaluated so we don't re-enter every frame even
        // when no preview is available (e.g. file outside the project).
        state.origin_path = Some(payload.path.clone());

        let Some(project) = project.as_deref() else { return };
        let asset_path = project.make_asset_relative(&payload.path);
        // Heuristic: if the path didn't strip cleanly to a relative path,
        // it's outside the project — skip the preview. Drop will still work
        // via the existing copy-into-project flow.
        if asset_path.contains(':') || asset_path.starts_with("..") {
            return;
        }

        // IMPORTANT: load with default settings. Loading the same path twice
        // with different `GltfLoaderSettings` poisons Bevy's image cache —
        // any URI-referenced texture gets registered with whichever
        // `render_asset_usages` ran first, and the later full load reuses
        // the cached handle, leaving textures missing on the dropped entity.
        // The ghost stays visually material-less because we override every
        // child mesh's `MeshMaterial3d` after spawn.
        let handle: Handle<Gltf> = asset_server.load(&asset_path);

        let ghost_material = state.ghost_material.clone().unwrap_or_else(|| {
            materials.add(StandardMaterial {
                base_color: Color::srgba(0.78, 0.80, 0.85, 0.85),
                perceptual_roughness: 0.6,
                metallic: 0.0,
                alpha_mode: AlphaMode::Blend,
                ..default()
            })
        });

        state.asset_path = Some(asset_path);
        state.name = Some(payload.name.clone());
        state.mesh_handle = Some(handle);
        state.ghost_material = Some(ghost_material);
    }

    // Update cursor ground position whenever it's over the viewport.
    let Ok(window) = window_query.single() else {
        state.cursor_in_viewport = false;
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        state.cursor_in_viewport = false;
        return;
    };

    let vp_min = viewport.screen_position;
    let vp_max = vp_min + viewport.screen_size;
    let in_vp = cursor_pos.x >= vp_min.x
        && cursor_pos.x <= vp_max.x
        && cursor_pos.y >= vp_min.y
        && cursor_pos.y <= vp_max.y;
    state.cursor_in_viewport = in_vp;
    if !in_vp {
        return;
    }

    let Some((camera, camera_transform)) = camera_query.iter().next() else {
        return;
    };

    let viewport_pos = Vec2::new(
        (cursor_pos.x - vp_min.x) / viewport.screen_size.x * viewport.current_size.x as f32,
        (cursor_pos.y - vp_min.y) / viewport.screen_size.y * viewport.current_size.y as f32,
    );
    let Ok(ray) = camera.viewport_to_world(camera_transform, viewport_pos) else {
        return;
    };

    if ray.direction.y.abs() > 1e-6 {
        let t = -ray.origin.y / ray.direction.y;
        if t > 0.0 && t < 10_000.0 {
            let hit = ray.origin + ray.direction * t;
            state.ground_position = Vec3::new(hit.x, 0.0, hit.z);
        }
    }
}

/// System: spawn the ghost root once its mesh-only Gltf is loaded; otherwise
/// just update its transform to track the cursor.
pub fn update_model_drag_ghost(
    mut commands: Commands,
    mut state: ResMut<ModelDragPreviewState>,
    gltf_assets: Res<Assets<Gltf>>,
    mut transform_query: Query<&mut Transform>,
    mut visibility_query: Query<&mut Visibility>,
) {
    // Already spawned → just sync transform + visibility.
    if let Some(root) = state.ghost_root {
        if let Ok(mut tf) = transform_query.get_mut(root) {
            tf.translation = state.ground_position;
        }
        if let Ok(mut vis) = visibility_query.get_mut(root) {
            *vis = if state.cursor_in_viewport {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
        return;
    }

    // Not spawned yet — wait until cursor is in viewport AND the gltf is
    // loaded enough to spawn its scene.
    if !state.cursor_in_viewport {
        return;
    }
    let Some(handle) = state.mesh_handle.as_ref() else { return };
    let Some(gltf) = gltf_assets.get(handle) else { return };
    let Some(scene) = gltf
        .default_scene
        .clone()
        .or_else(|| gltf.scenes.first().cloned())
    else {
        // Nothing to show — don't keep retrying.
        state.mesh_handle = None;
        return;
    };

    let root = commands
        .spawn((
            Name::new("ModelDragGhost"),
            Transform::from_translation(state.ground_position),
            Visibility::Inherited,
            ModelDragGhost,
        ))
        .id();
    commands.spawn((
        Name::new("ModelDragGhostScene"),
        SceneRoot(scene),
        Transform::default(),
        Visibility::Inherited,
        ChildOf(root),
    ));

    state.ghost_root = Some(root);
}

/// System: replace `MeshMaterial3d<StandardMaterial>` on every descendant of
/// the ghost root with the shared flat-grey override material. Runs every
/// frame because Bevy's scene spawner may add children asynchronously.
pub fn apply_ghost_material_override(
    mut commands: Commands,
    state: Res<ModelDragPreviewState>,
    children_query: Query<&Children>,
    candidates: Query<
        Entity,
        (
            With<MeshMaterial3d<StandardMaterial>>,
            Without<GhostMaterialApplied>,
        ),
    >,
) {
    let Some(root) = state.ghost_root else { return };
    let Some(material) = state.ghost_material.clone() else { return };

    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if let Ok(kids) = children_query.get(entity) {
            stack.extend(kids.iter());
        }
        if candidates.contains(entity) {
            commands
                .entity(entity)
                .insert((MeshMaterial3d(material.clone()), GhostMaterialApplied));
        }
    }
}

/// System: clean up the ghost when the asset drag resource has been removed
/// (drop or cancel) without the drop handler having already cleared it.
pub fn cleanup_model_drag_ghost(
    mut commands: Commands,
    mut state: ResMut<ModelDragPreviewState>,
    payload: Option<Res<AssetDragPayload>>,
) {
    if payload.is_some() {
        return;
    }
    if let Some(entity) = state.ghost_root.take() {
        commands.entity(entity).despawn();
    }
    state.clear();
}

/// Lightweight read-only snapshot of all in-flight model loads for the
/// viewport progress overlay. Returns `(name, fraction_or_none)` per load.
/// Mesh-only and full loads both included.
pub fn collect_model_load_progress(world: &World) -> Vec<(String, Option<f32>)> {
    let mut out = Vec::new();
    let asset_server = world.get_resource::<AssetServer>();

    if let Some(state) = world.get_resource::<ModelDragPreviewState>() {
        if let (Some(handle), Some(server), Some(name)) =
            (state.mesh_handle.as_ref(), asset_server, state.name.as_ref())
        {
            let loaded = matches!(server.get_load_state(handle.id()), Some(LoadState::Loaded));
            if !loaded {
                out.push((format!("{} (mesh)", name), None));
            }
        }
    }

    if let (Some(pending), Some(server)) = (
        world.get_resource::<PendingGltfLoads>(),
        asset_server,
    ) {
        for load in &pending.loads {
            let loaded = matches!(server.get_load_state(load.handle.id()), Some(LoadState::Loaded));
            let frac = if loaded { Some(1.0) } else { None };
            out.push((load.name.clone(), frac));
        }
    }

    out
}

