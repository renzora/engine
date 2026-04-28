//! Drag-and-drop model spawning — detects asset drops on the viewport and
//! spawns GLTF/GLB models into the scene.
//!
//! While a model is being dragged over the viewport, the full textured GLB
//! is spawned and follows the cursor — same materials as the eventual drop,
//! so the user sees the actual placement preview rather than a flat-grey
//! placeholder. The preview entity is discarded on drop or cancel; the
//! committed entity is spawned fresh through the normal pipeline so it picks
//! up the import-pipeline-generated `.material` files via the resolver.

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
use renzora_editor::{EditorCommands, EditorSelection};
use renzora_ui::asset_drag::AssetDragPayload;

use crate::glb_compat;
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

/// State for the live preview shown while a model is being dragged over the
/// viewport. Cleared when the drag ends (drop or cancel).
#[derive(Resource, Default)]
pub struct ModelDragPreviewState {
    /// Source path from the asset drag payload (used to detect a new drag).
    pub origin_path: Option<PathBuf>,
    /// Asset-relative path passed to the asset server.
    pub asset_path: Option<String>,
    /// Display name carried over to the real entity on drop.
    pub name: Option<String>,
    /// Gltf handle for the previewed model.
    pub mesh_handle: Option<Handle<Gltf>>,
    /// Spawned preview root entity. `None` until the Gltf load completes.
    pub ghost_root: Option<Entity>,
    /// Last known cursor ground position (Y=0 plane).
    pub ground_position: Vec3,
    /// True when the cursor is currently over the viewport rect.
    pub cursor_in_viewport: bool,
}

impl ModelDragPreviewState {
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

/// Marker: animation discovery has been attempted for this entity (hit or
/// miss). Prevents `auto_discover_animations` from re-scanning the
/// filesystem on every frame for models that have no `.anim` files.
#[derive(Component)]
pub struct AnimationDiscoveryDone;

/// Tracks a freshly-spawned GLTF model that still needs its mesh entities
/// bound to `MaterialRef` components. Held on the `ImportedRoot` entity. The
/// `Handle<Gltf>` keeps the asset alive long enough to read its
/// `named_materials` map, which is how we recover the original material name
/// for each `MeshMaterial3d<StandardMaterial>` handle the scene spawner
/// attached.
///
/// The marker lives on the parent for the entire life of the model — the
/// binder is idempotent (the query filter excludes already-bound meshes),
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

            // If we spawned a preview entity during drag (in-project
            // asset), promote it in place: add the production markers
            // that drive the binder/resolver/flatten pipeline. Same
            // entity, no despawn, no second SceneSpawner instantiation.
            //
            // We clear `placement_entity` and `mesh_handle` so neither
            // cleanup nor `update_model_placement` will touch the entity
            // again, but we leave `origin_path` set so
            // `track_model_drag_preview` skips re-initializing for the
            // still-active drag (egui can hold the payload one extra
            // frame after release).
            let promotion = world
                .get_resource_mut::<ModelDragPreviewState>()
                .and_then(|mut s| {
                    let entity = s.ghost_root.take();
                    let asset_path = s.asset_path.take();
                    let gltf_handle = s.mesh_handle.take();
                    s.name = None;
                    s.cursor_in_viewport = false;
                    entity.zip(asset_path).zip(gltf_handle).map(|((e, p), h)| (e, p, h))
                });

            if let Some((entity, asset_path, gltf_handle)) = promotion {
                // Add production markers to the parent entity in place.
                if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
                    entity_mut.insert((
                        MeshInstanceData {
                            model_path: Some(asset_path),
                        },
                        ImportedRoot,
                        PendingMaterialBinding { gltf_handle },
                        NeedsGroundAlignment {
                            target_y: ground_pos.y,
                        },
                    ));
                }
                // Add `PendingFlatten` to the entity's SceneRoot child so
                // the flatten pass collapses gltf wrapper nodes once the
                // scene is fully populated.
                let candidate_children: Vec<Entity> = world
                    .get::<Children>(entity)
                    .map(|kids| kids.iter().collect())
                    .unwrap_or_default();
                let mut scene_root_child: Option<Entity> = None;
                for child in candidate_children {
                    if world.get::<SceneRoot>(child).is_some() {
                        scene_root_child = Some(child);
                        break;
                    }
                }
                if let Some(child) = scene_root_child {
                    world.entity_mut(child).insert(PendingFlatten::default());
                }
                if let Some(selection) = world.get_resource::<EditorSelection>() {
                    selection.set(Some(entity));
                }
            } else {
                // No placement entity — out-of-project drag (the preview
                // path skipped this asset because it wasn't already in the
                // project). Run the import-then-spawn pipeline so the GLB
                // gets copied into the project and a fresh entity spawned.
                initiate_model_load(world, path, name, ground_pos);
            }
        });
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

/// Run the import pipeline on `source`, write the result to `dest`, dump
/// extracted textures under `<model_dir>/textures/`, and fire one
/// `PbrMaterialExtracted` event per material so `renzora_shader::material`
/// writes a `.material` file per entry.
///
/// Logs and falls back to a plain file copy on failure — the GLB still loads
/// for the user, just without per-material editable graphs.
fn run_import_pipeline(
    world: &mut World,
    source: &std::path::Path,
    dest: &std::path::Path,
    model_dir: &std::path::Path,
    project_path: &std::path::Path,
) {
    use renzora_import::{convert_to_glb, ImportSettings};

    // Skip mesh optimization for the drop path — these reorder triangle
    // buffers and are only meaningful for re-importing source files. The
    // drop pipeline is for getting an existing GLB into the project quickly.
    let settings = ImportSettings {
        optimize_vertex_cache: false,
        optimize_overdraw: false,
        optimize_vertex_fetch: false,
        ..Default::default()
    };

    let result = match convert_to_glb(source, &settings) {
        Ok(r) => r,
        Err(e) => {
            warn!("[model_drop] convert failed for {:?}: {}; falling back to plain copy", source, e);
            if source != dest {
                if let Err(ce) = std::fs::copy(source, dest) {
                    error!("[model_drop] copy fallback failed: {}", ce);
                }
            }
            return;
        }
    };

    if let Err(e) = std::fs::write(dest, &result.glb_bytes) {
        error!("[model_drop] write GLB to {:?}: {}", dest, e);
        return;
    }

    if !result.extracted_textures.is_empty() {
        let tex_dir = model_dir.join("textures");
        if let Err(e) = std::fs::create_dir_all(&tex_dir) {
            warn!("[model_drop] create textures dir: {}", e);
        } else {
            for tex in &result.extracted_textures {
                let tex_path = tex_dir.join(format!("{}.{}", tex.name, tex.extension));
                if let Err(e) = std::fs::write(&tex_path, &tex.data) {
                    warn!("[model_drop] write texture '{}': {}", tex.name, e);
                }
            }
        }
    }

    if !result.extracted_materials.is_empty() {
        let mat_dir = model_dir.join("materials");
        // Texture URIs from the converter are relative to the model folder
        // (e.g. `textures/diffuse.png`). The material observer wants
        // project-relative paths so the resolver can find them — prefix with
        // the model folder's location under the project root.
        let model_rel = model_dir
            .strip_prefix(project_path)
            .ok()
            .and_then(|p| p.to_str())
            .map(|s| s.replace('\\', "/"))
            .unwrap_or_default();
        let prefix = |uri: &Option<String>| -> Option<String> {
            uri.as_ref().map(|u| {
                if model_rel.is_empty() {
                    u.clone()
                } else {
                    format!("{}/{}", model_rel, u)
                }
            })
        };

        for mat in &result.extracted_materials {
            world.trigger(renzora::core::PbrMaterialExtracted {
                name: mat.name.clone(),
                output_dir: mat_dir.clone(),
                base_color: mat.base_color,
                metallic: mat.metallic,
                roughness: mat.roughness,
                emissive: mat.emissive,
                base_color_texture: prefix(&mat.base_color_texture),
                normal_texture: prefix(&mat.normal_texture),
                metallic_roughness_texture: prefix(&mat.metallic_roughness_texture),
                emissive_texture: prefix(&mat.emissive_texture),
                occlusion_texture: prefix(&mat.occlusion_texture),
                specular_glossiness_texture: prefix(&mat.specular_glossiness_texture),
                alpha_mode: match mat.alpha_mode {
                    renzora_import::ExtractedAlphaMode::Opaque => renzora::core::PbrAlphaMode::Opaque,
                    renzora_import::ExtractedAlphaMode::Mask => renzora::core::PbrAlphaMode::Mask,
                    renzora_import::ExtractedAlphaMode::Blend => renzora::core::PbrAlphaMode::Blend,
                },
                alpha_cutoff: mat.alpha_cutoff,
                double_sided: mat.double_sided,
            });
        }
    }
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

        let project_path = project.path.clone();
        let asset_rel = project.make_asset_relative(&dest);

        // Run the import pipeline so the model lands in the project with
        // textures pulled into `textures/` and a `.material` file written per
        // material under `materials/`. Each spawned mesh entity later gets a
        // `MaterialRef` to the matching `.material`, which the resolver swaps
        // in for the GLB's embedded `StandardMaterial`. Falls back to a plain
        // copy if conversion fails — the model still loads, just without the
        // editable per-material graphs.
        run_import_pipeline(world, &path, &dest, &model_dir, &project_path);

        glb_compat::ensure_loadable(&dest);

        asset_rel
    } else {
        glb_compat::ensure_loadable(&path);
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
                PendingMaterialBinding {
                    gltf_handle: load.handle.clone(),
                },
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

/// System: track the active model drag, kick off the full Gltf load the
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

        // Patch the file in place before loading so Bevy doesn't choke on
        // unsupported `extensionsRequired` entries (e.g. third-party GLBs that
        // declare `KHR_materials_pbrSpecularGlossiness`).
        glb_compat::ensure_loadable(&payload.path);

        // Load with default settings — same Gltf the dropped entity will use,
        // so the preview shows real materials (matching Godot's drag-feel).
        // Loading the same path twice with different `GltfLoaderSettings`
        // would poison Bevy's image cache.
        let handle: Handle<Gltf> = asset_server.load(&asset_path);

        state.asset_path = Some(asset_path);
        state.name = Some(payload.name.clone());
        state.mesh_handle = Some(handle);
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

/// System: spawn the model entity once its Gltf is loaded, then track
/// the cursor with its transform until the user releases the mouse.
///
/// The entity we spawn here is the **final** scene entity — same components
/// any post-drop spawn would produce. While the drag is active, this system
/// updates its transform every frame so it follows the cursor. On release,
/// `check_viewport_model_drop`'s deferred handler adds `NeedsGroundAlignment`
/// and clears the placement state; from there the entity is just a regular
/// scene entity. No "ghost", no despawn-and-respawn — Bevy's SceneSpawner
/// only instantiates the GLB once, and that single instance becomes the
/// real scene model.
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

    let display_name = state
        .name
        .clone()
        .unwrap_or_else(|| "Model".to_string());

    // Spawn a minimal preview entity: just the SceneRoot scene under a
    // transform parent. No production markers (`MeshInstanceData`,
    // `ImportedRoot`, `PendingMaterialBinding`, `PendingFlatten`) — those
    // would kick off the binder/resolver/flatten pipeline mid-drag, which
    // we don't want until the user actually commits the placement on
    // drop. The entity *itself* is the final entity though — the drop
    // handler decorates it in place rather than despawning + respawning.
    let root = commands
        .spawn((
            Name::new(display_name),
            Transform::from_translation(state.ground_position),
            Visibility::Inherited,
        ))
        .id();

    commands.spawn((
        Name::new("SceneRoot"),
        SceneRoot(scene),
        Transform::default(),
        Visibility::Inherited,
        ChildOf(root),
    ));

    state.ghost_root = Some(root);
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

// ── Material binding ───────────────────────────────────────────────────────

/// System: walks each `PendingMaterialBinding` model, finds its mesh
/// descendants, and inserts a `MaterialRef` pointing at the per-material
/// `.material` file the import pipeline wrote. The existing
/// `MaterialResolverPlugin` then loads each file and swaps the GLB's
/// `StandardMaterial` for the editable `GraphMaterial`.
///
/// Runs every frame for as long as the marker exists. Bevy's scene spawner
/// populates large GLBs incrementally — Bistro / Audi can take dozens of
/// frames to fully spawn, with new mesh entities appearing throughout.
/// The earlier "found one mesh → done" logic was leaving most of those
/// meshes unbinded, so we just keep going. The work is idempotent: the
/// query filter excludes meshes that already carry `MaterialRef` /
/// `MaterialBindingDone`, so a fully-bound model costs one descendant walk
/// per frame and zero binds. The marker disappears when the parent is
/// despawned.
pub fn bind_material_refs(
    mut commands: Commands,
    pending_query: Query<(Entity, &PendingMaterialBinding, &MeshInstanceData)>,
    children_query: Query<&Children>,
    mesh_mat_query: Query<
        &MeshMaterial3d<StandardMaterial>,
        (With<Mesh3d>, Without<MaterialBindingDone>, Without<renzora::MaterialRef>),
    >,
    gltf_assets: Res<Assets<Gltf>>,
) {
    use std::collections::HashMap;

    for (root_entity, pending, mesh_data) in pending_query.iter() {
        let Some(gltf) = gltf_assets.get(&pending.gltf_handle) else {
            // GLB still loading. Wait — `PendingMaterialBinding` holds the
            // handle so the asset is kept alive.
            continue;
        };

        // Compute the materials directory relative to the project — the
        // `.material` files live next to the GLB at `<model_dir>/materials/`.
        // No `model_path` means there's nothing to bind to; the marker is
        // useless on this entity, drop it.
        let Some(model_path) = mesh_data.model_path.as_deref() else {
            commands.entity(root_entity).remove::<PendingMaterialBinding>();
            continue;
        };
        let model_dir_rel = std::path::Path::new(model_path)
            .parent()
            .and_then(|p| p.to_str())
            .map(|s| s.replace('\\', "/"))
            .unwrap_or_default();
        let materials_dir_rel = if model_dir_rel.is_empty() {
            "materials".to_string()
        } else {
            format!("{}/materials", model_dir_rel)
        };

        // Build lookup: every material handle in the GLB → its name. Named
        // entries use the GLTF's authored name; unnamed entries fall back to
        // `material_{index}` matching what `extract_glb_materials` produced
        // and what the `.material` filename was therefore based on.
        let mut name_by_id: HashMap<AssetId<StandardMaterial>, String> = HashMap::new();
        for (name, handle) in gltf.named_materials.iter() {
            name_by_id.insert(handle.id(), name.to_string());
        }
        for (i, handle) in gltf.materials.iter().enumerate() {
            name_by_id
                .entry(handle.id())
                .or_insert_with(|| format!("material_{}", i));
        }

        // Walk descendants and bind any meshes that haven't been bound yet.
        // The query filter ensures already-bound meshes are skipped; once
        // every descendant has been processed this loop is effectively a
        // no-op.
        let mut stack: Vec<Entity> = vec![root_entity];
        while let Some(entity) = stack.pop() {
            if let Ok(kids) = children_query.get(entity) {
                stack.extend(kids.iter());
            }
            if let Ok(mat) = mesh_mat_query.get(entity) {
                if let Some(name) = name_by_id.get(&mat.0.id()) {
                    let safe = sanitize_material_name(name);
                    let path = format!("{}/{}.material", materials_dir_rel, safe);
                    commands
                        .entity(entity)
                        .insert((renzora::MaterialRef(path), MaterialBindingDone));
                } else {
                    // Mesh has a material handle the GLB didn't expose —
                    // could happen with procedurally-spawned children. Mark
                    // done so we don't keep retrying it.
                    commands.entity(entity).insert(MaterialBindingDone);
                }
            }
        }
    }
}

/// Sanitize a material name for use as a filename. Mirrors
/// `renzora_shader::material::on_pbr_material_extracted` so binding paths
/// agree with the writer.
fn sanitize_material_name(name: &str) -> String {
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

