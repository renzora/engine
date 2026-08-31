//! Drag-and-drop model spawning — detects asset drops on the viewport and
//! spawns GLTF/GLB models into the scene.
//!
//! While a model is being dragged over the viewport, the full textured GLB
//! is spawned and follows the cursor — same materials as the eventual drop,
//! so the user sees the actual placement preview rather than a flat-grey
//! placeholder. The preview entity is discarded on drop or cancel; the
//! committed entity is spawned fresh through the normal pipeline so it picks
//! up the import-pipeline-generated `.material` files via the resolver.
//!
//! # Where a dropped model lands
//!
//! There are two placement rules, and which one applies depends on whether the
//! cursor is over existing geometry:
//!
//! - **Over a mesh** — the model is bottom-aligned, so its lowest point rests
//!   on the surface under the cursor. This is what you want when placing a
//!   prop on a table or a rock on terrain.
//! - **Over empty space** — the model's *origin* goes straight onto the Y=0
//!   plane and the bounds are never consulted.
//!
//! The second rule exists because bottom-aligning against the whole bounding
//! box is only correct for a model whose lowest geometry *is* its footprint. A
//! large environment GLB frequently has one stray piece hanging below the
//! origin — a basement slab, a foundation, a below-grade prop — and aligning on
//! the bounding box lifts the entire building by however far that one piece
//! hangs down, leaving it floating with no way to seat it at Y=0. Author intent
//! lives in the origin, so on an empty drop we honour the origin and let the
//! stray geometry hang below the floor, which is what it was modelled to do.

use std::path::PathBuf;

use bevy::asset::LoadState;
use bevy::camera::primitives::Aabb;
use bevy::pbr::MeshMaterial3d;
use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings};
use bevy::prelude::*;
use bevy::world_serialization::{WorldAssetRoot, WorldInstanceReady};
use bevy::window::PrimaryWindow;

use renzora::core::{CurrentProject, EditorCamera, MeshInstanceData};
use renzora_animation::AnimatorComponent;
use renzora_editor_framework::EditorSelection;
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
    /// World Y the model's *bottom* should settle at, or `None` when the drop
    /// placed the origin directly and must not be re-seated by its bounds.
    pub bottom_align_target: Option<f32>,
}

/// Marker component — entity needs its Y adjusted so the bottom sits on the
/// surface it was dropped onto.
///
/// Only added for a drop that landed on existing geometry. A drop into empty
/// space places the origin on the Y=0 plane and is already final, so it never
/// gets this marker — see the module docs for why the bounds are the wrong
/// thing to align by there.
#[derive(Component)]
pub struct NeedsGroundAlignment {
    pub target_y: f32,
    /// Frames spent waiting for every mesh in the model to report bounds.
    /// Bounded so a mesh that can never produce an `Aabb` (no position
    /// attribute) doesn't leave the model unaligned forever.
    pub frames_waited: u32,
}

impl NeedsGroundAlignment {
    pub fn new(target_y: f32) -> Self {
        Self {
            target_y,
            frames_waited: 0,
        }
    }
}

/// Frames [`align_models_to_ground`] will wait for a model's meshes to finish
/// loading before aligning on whatever bounds have arrived. Two seconds at
/// 60 Hz — long enough for a large GLB to stream in, short enough that a model
/// which will never report full bounds still ends up on the ground.
const GROUND_ALIGNMENT_MAX_WAIT_FRAMES: u32 = 120;

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
    /// World-space point where the drag ray last hit existing scene geometry,
    /// or `None` when the cursor is over empty space. This is the switch
    /// between the module's two placement rules.
    pub surface_hit: Option<Vec3>,
    /// True when the cursor is currently over the viewport rect.
    pub cursor_in_viewport: bool,
    /// Signed distance from the preview root's origin down to the lowest point
    /// of its geometry, once every mesh has loaded. Adding it to the hit
    /// position is what puts the model's *bottom* under the cursor instead of
    /// its origin, so the drag preview stands on the surface and the drop
    /// doesn't pop the model into a new position. Only consulted for a drop
    /// onto geometry — an empty-space drop ignores the bounds entirely.
    pub bottom_offset: Option<f32>,
}

impl ModelDragPreviewState {
    pub fn clear(&mut self) {
        self.origin_path = None;
        self.asset_path = None;
        self.name = None;
        self.mesh_handle = None;
        self.ghost_root = None;
        self.ground_position = Vec3::ZERO;
        self.surface_hit = None;
        self.cursor_in_viewport = false;
        self.bottom_offset = None;
    }

    /// Where the model root sits for the cursor's current position.
    ///
    /// Both the drag preview and the drop commit read this, so releasing the
    /// mouse never moves the model away from where it was being previewed.
    pub fn placement_translation(&self) -> Vec3 {
        match self.surface_hit {
            // Lift the root so the geometry's underside meets the surface.
            // `bottom_offset` is `None` until every mesh has reported its
            // bounds; the drop's `NeedsGroundAlignment` corrects the placement
            // once they arrive.
            Some(hit) => hit + Vec3::Y * self.bottom_offset.unwrap_or(0.0),
            None => self.ground_position,
        }
    }

    /// The world Y a dropped model's *bottom* should settle at, or `None` when
    /// the drop takes the origin-on-the-ground rule and needs no correcting
    /// once its meshes finish loading.
    pub fn bottom_align_target(&self) -> Option<f32> {
        self.surface_hit.map(|hit| hit.y)
    }
}

/// The provisional box shown at the cursor while a dragged model's GLB is
/// still loading.
///
/// # Why a box and not the model
///
/// Because the model does not exist yet, and cannot be made to. Bevy's glTF
/// loader decodes **every texture inline** before it publishes the `Gltf` asset
/// — the loop is unconditional, and `load_materials: RenderAssetUsages::empty()`
/// only skips building the `StandardMaterial`s afterwards. So there is no
/// setting that yields geometry-without-textures early, and loading the same
/// path a second time with different settings poisons the image cache for the
/// first (see `track_model_drag_preview`). An untextured preview of the real
/// mesh would need a forked loader.
///
/// What was actually wrong was the silence: on a large GLB the cursor carried
/// nothing at all for several seconds, so the drag looked like it had not
/// registered. A box that appears on the first frame, sits where the model will
/// sit, and is replaced by the real thing the moment it loads fixes that much
/// without pretending to be the model — which is why it is deliberately
/// translucent and unlit rather than a convincing grey solid.
///
/// Carried as a component rather than a field on `ModelDragPreviewState`
/// because that resource's `clear()` nulls its entity fields without
/// despawning, and the callers between them handle the despawn. A marker is
/// queryable from anywhere and cannot be orphaned by a `clear()` that forgot it.
#[derive(Component)]
pub struct ModelDragPlaceholder;

/// Side of the placeholder box, in metres. One metre is a guess — the model's
/// real size is inside the file we are still waiting for — but it is the guess
/// that is wrong by the least across props, characters and furniture, and it
/// gives the eye a scale reference for where the drop will land. It is
/// centred on its own origin, so it is lifted by half of this to sit *on* the
/// placement point rather than half through it.
const PLACEHOLDER_SIZE: f32 = 1.0;

/// Marker: animation discovery has been attempted for this entity (hit or
/// miss). Prevents `auto_discover_animations` from re-scanning the
/// filesystem on every frame for models that have no `.anim` files.
#[derive(Component)]
pub struct AnimationDiscoveryDone;

/// The material-binding markers and the binder itself now live in
/// `renzora_engine` so a shipped game runs them too — see that module for why
/// an editor-only binder made spec-glossiness models render as white metal in
/// the runtime. Re-exported so this crate's existing paths keep working.
pub use renzora_engine::material_binding::{
    bind_material_refs, MaterialBindingDone, PendingMaterialBinding,
};

/// Commit a model drop at the given viewport-space pointer. Either promotes the
/// live drag-preview entity in place, or for out-of-project drags with no preview
/// runs the import-then-spawn pipeline.
///
/// Currently unused: the native drop path ([`native_model_drop`]) promotes the
/// in-project preview ghost inline and does not yet route out-of-project drags
/// through here. Kept (with the import pipeline below) so that path can be wired
/// up without re-deriving it.
#[allow(dead_code)]
pub(crate) fn commit_model_drop(
    world: &mut World,
    screen_pos: Vec2,
    vp_rect: Rect,
    path: PathBuf,
    name: String,
) {
    // Prefer the placement the ghost was tracking — it matches exactly what the
    // user saw under their cursor at drop time. With no ghost (an
    // out-of-project drag) there was no surface test either, so the model takes
    // the empty-space rule and its origin lands on the Y=0 plane.
    let preview = world
        .get_resource::<ModelDragPreviewState>()
        .filter(|s| s.origin_path.as_deref() == Some(path.as_path()))
        .map(|s| (s.placement_translation(), s.bottom_align_target()));
    let (spawn_pos, bottom_align_target) = match preview {
        Some(placement) => placement,
        None => (
            compute_ground_position(world, screen_pos, vp_rect).unwrap_or(Vec3::ZERO),
            None,
        ),
    };

    // If we spawned a preview entity during drag (in-project
    // asset), promote it in place: add the production markers
    // that drive the binder/resolver/flatten pipeline. Same
    // entity, no despawn, no second SceneSpawner instantiation.
    //
    // We clear `ghost_root` and `mesh_handle` so neither cleanup nor
    // `update_model_drag_ghost` will touch the entity again, but we leave
    // `origin_path` set so `track_model_drag_preview` skips re-initializing
    // for the still-active drag (the payload can linger one extra frame
    // after release).
    let promotion = world
        .get_resource_mut::<ModelDragPreviewState>()
        .and_then(|mut s| {
            let entity = s.ghost_root.take();
            let asset_path = s.asset_path.take();
            let gltf_handle = s.mesh_handle.take();
            s.name = None;
            s.cursor_in_viewport = false;
            entity
                .zip(asset_path)
                .zip(gltf_handle)
                .map(|((e, p), h)| (e, p, h))
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
            ));
            // Only a drop onto geometry needs re-seating once the meshes
            // load; an empty-space drop is already where it belongs.
            if let Some(target_y) = bottom_align_target {
                entity_mut.insert(NeedsGroundAlignment::new(target_y));
            }
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
            if world.get::<WorldAssetRoot>(child).is_some() {
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
        initiate_model_load(world, path, name, spawn_pos, bottom_align_target);
    }
}

/// Native (bevy_ui) model drop handler.
///
/// Unlike the egui path, this **cannot** read the [`AssetDragPayload`] at release
/// time: the native asset browser removes it via a deferred command on mouse-up,
/// and any intervening exclusive system flushes that removal before we'd see it.
/// So the drop is driven entirely off [`ModelDragPreviewState`] — which nothing
/// else touches — plus the mouse-release edge. A live `ghost_root` means an
/// in-project drag preview is active; if the cursor is over the focused viewport
/// on release we promote that entity in place (same markers the egui commit
/// adds). Released outside the viewport (or Escape, which doesn't fire
/// `just_released`) falls through to `cleanup_model_drag_ghost`, which cancels.
///
/// Runs before `cleanup_model_drag_ghost` (clears `ghost_root` synchronously via
/// `ResMut`), so cleanup never despawns a promoted entity. Gated on the bevy_ui
/// backend, so it never double-fires with the egui drop check.
#[allow(clippy::too_many_arguments)]
pub fn native_model_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    mut state: ResMut<ModelDragPreviewState>,
    viewport: Res<ViewportState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    mut commands: Commands,
    children_query: Query<&Children>,
    scene_root_query: Query<(), With<WorldAssetRoot>>,
    selection: Option<Res<EditorSelection>>,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    // Only in-project drags get this far: an out-of-project one never gets an
    // asset path or a handle, and falls through to the copy-into-project
    // pipeline. `ghost_root` is the loaded preview; a handle with no ghost is a
    // drag whose GLB is still decoding behind the placeholder box.
    if state.ghost_root.is_none() && state.mesh_handle.is_none() {
        return;
    }

    // Released over the focused viewport? Recompute from the live cursor rather
    // than trusting `cursor_in_viewport`, which `track_model_drag_preview` may
    // have already reset once the payload vanished.
    let over_viewport = window_query
        .single()
        .ok()
        .and_then(|w| w.cursor_position())
        .map(|c| {
            let min = viewport.screen_position;
            let max = min + viewport.screen_size;
            c.x >= min.x && c.y >= min.y && c.x <= max.x && c.y <= max.y
        })
        .unwrap_or(false);
    if !over_viewport {
        // Cancel — let `cleanup_model_drag_ghost` despawn the preview.
        return;
    }

    // Promote the preview entity in place: take the placement data out of the
    // state (so neither cleanup nor `update_model_drag_ghost` touch the entity
    // again) but leave `origin_path` set so `track_model_drag_preview` skips
    // re-initializing for the payload that may linger one extra frame.
    let bottom_align_target = state.bottom_align_target();
    let placement = state.placement_translation();
    let asset_path = state.asset_path.take();
    let gltf_handle = state.mesh_handle.take();
    let ghost_root = state.ghost_root.take();
    let name = state.name.take();
    state.cursor_in_viewport = false;

    let (Some(asset_path), Some(gltf_handle)) = (asset_path, gltf_handle) else {
        return;
    };

    // Dropped while the GLB was still decoding — there is no entity to promote,
    // only the placeholder box, which `cleanup_model_drag_ghost` is about to
    // remove. Hand the load off to the pending queue with the position the box
    // was standing at, so the model arrives where it was dropped.
    //
    // Before the placeholder existed this case looked like nothing happening,
    // and it was: the drop returned early on a missing ghost and the drag was
    // silently thrown away. Showing a box you can drop made that visible.
    let Some(entity) = ghost_root else {
        let name = name.unwrap_or_else(|| "Model".to_string());
        commands.queue(move |world: &mut World| {
            world.resource_mut::<PendingGltfLoads>().loads.push(PendingLoad {
                handle: gltf_handle,
                name,
                asset_path,
                spawn_position: placement,
                bottom_align_target,
            });
        });
        return;
    };

    commands.entity(entity).insert((
        MeshInstanceData {
            model_path: Some(asset_path),
        },
        ImportedRoot,
        PendingMaterialBinding { gltf_handle },
    ));

    // Dropped onto geometry: re-seat the model once its meshes finish loading,
    // since the preview's lift was measured from whatever bounds existed then.
    // Dropped onto empty space: the preview's origin-on-the-ground placement is
    // already final and must not be second-guessed by the bounds.
    if let Some(target_y) = bottom_align_target {
        commands
            .entity(entity)
            .insert(NeedsGroundAlignment::new(target_y));
    }

    // Tag the SceneRoot child so the flatten pass collapses gltf wrappers once
    // the scene is fully populated.
    if let Ok(kids) = children_query.get(entity) {
        for child in kids.iter() {
            if scene_root_query.get(child).is_ok() {
                commands.entity(child).insert(PendingFlatten::default());
                break;
            }
        }
    }

    if let Some(selection) = selection {
        selection.set(Some(entity));
    }
}

/// Compute a world-space ground position (Y=0 plane) from a viewport-space
/// pointer. `screen_pos` / `viewport_rect` are in window logical pixels — the
/// space egui pointer positions and [`ViewportState::screen_position`] share.
fn compute_ground_position(
    world: &mut World,
    screen_pos: Vec2,
    viewport_rect: Rect,
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

    let ray = camera
        .viewport_to_world(&camera_transform, Vec2::new(render_x, render_y))
        .ok()?;

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
            warn!(
                "[model_drop] convert failed for {:?}: {}; falling back to plain copy",
                source, e
            );
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
                if let Err(e) = tex.write_to(&tex_path) {
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
                project_root: project_path.to_path_buf(),
                base_color: mat.base_color,
                metallic: mat.metallic,
                roughness: mat.roughness,
                emissive: mat.emissive,
                base_color_texture: prefix(&mat.base_color_texture),
                normal_texture: prefix(&mat.normal_texture),
                metallic_roughness_texture: prefix(&mat.metallic_roughness_texture),
                roughness_texture: prefix(&mat.roughness_texture),
                metallic_texture: prefix(&mat.metallic_texture),
                emissive_texture: prefix(&mat.emissive_texture),
                occlusion_texture: prefix(&mat.occlusion_texture),
                specular_glossiness_texture: prefix(&mat.specular_glossiness_texture),
                opacity_texture: prefix(&mat.opacity_texture),
                specular_texture: prefix(&mat.specular_texture),
                advanced: mat.advanced.rewrite_textures(prefix),
                alpha_mode: match mat.alpha_mode {
                    renzora_import::ExtractedAlphaMode::Opaque => {
                        renzora::core::PbrAlphaMode::Opaque
                    }
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
fn initiate_model_load(
    world: &mut World,
    path: PathBuf,
    name: String,
    spawn_position: Vec3,
    bottom_align_target: Option<f32>,
) {
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
            bottom_align_target,
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
            bevy::world_serialization::WorldAssetRoot(scene),
            Transform::default(),
            Visibility::default(),
            ChildOf(parent),
            PendingFlatten::default(),
        ));

        // Re-seat onto the surface it was dropped on once the meshes report
        // bounds. Absent for an empty-space drop, whose spawn position is the
        // final one — see the module docs.
        if let Some(target_y) = load.bottom_align_target {
            commands
                .entity(parent)
                .insert(NeedsGroundAlignment::new(target_y));
        }

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
    renzora_animation::discover_animation_clips(asset_path, &project?.path)
}

/// Lowest world-space Y across the mesh AABBs under `root`, plus whether every
/// mesh in the subtree has reported bounds yet.
///
/// The readiness flag matters because Bevy only computes an `Aabb` once the
/// mesh asset itself has loaded, and a GLB's meshes arrive over several frames.
/// Reading the bounds of whatever subset exists right now describes a fraction
/// of the model — which is what left large imports floating or half-buried: the
/// first mesh to load decided the height, and the pieces that actually reached
/// lowest arrived afterwards with nothing left to correct them.
fn scan_mesh_bounds(
    root: Entity,
    children_query: &Query<&Children>,
    aabb_query: &Query<(&Aabb, &GlobalTransform)>,
    mesh_query: &Query<(), With<Mesh3d>>,
) -> (Option<f32>, bool) {
    let mut lowest: Option<f32> = None;
    let mut all_ready = true;
    let mut stack: Vec<Entity> = children_query
        .get(root)
        .map(|kids| kids.iter().collect())
        .unwrap_or_default();

    while let Some(entity) = stack.pop() {
        if let Ok((aabb, global_transform)) = aabb_query.get(entity) {
            let center = Vec3::from(aabb.center);
            let half = Vec3::from(aabb.half_extents);

            // All 8 corners in world space — the node may be rotated, so the
            // lowest corner is not necessarily the one with the lowest local Y.
            for sx in [-1.0f32, 1.0] {
                for sy in [-1.0f32, 1.0] {
                    for sz in [-1.0f32, 1.0] {
                        let corner: Vec3 = center + half * Vec3::new(sx, sy, sz);
                        let world_pos: Vec3 = global_transform.transform_point(corner);
                        lowest =
                            Some(lowest.map_or(world_pos.y, |prev: f32| prev.min(world_pos.y)));
                    }
                }
            }
        } else if mesh_query.get(entity).is_ok() {
            all_ready = false;
        }

        if let Ok(grandchildren) = children_query.get(entity) {
            stack.extend(grandchildren.iter());
        }
    }

    (lowest, all_ready)
}

/// [`scan_mesh_bounds`] restricted to fully-loaded models — `None` until every
/// mesh in the subtree has bounds.
fn lowest_mesh_y(
    root: Entity,
    children_query: &Query<&Children>,
    aabb_query: &Query<(&Aabb, &GlobalTransform)>,
    mesh_query: &Query<(), With<Mesh3d>>,
) -> Option<f32> {
    match scan_mesh_bounds(root, children_query, aabb_query, mesh_query) {
        (lowest, true) => lowest,
        _ => None,
    }
}

/// System: once every child mesh has an AABB, offset the parent so the model's
/// bottom sits on the surface it was dropped onto.
pub fn align_models_to_ground(
    mut commands: Commands,
    mut query: Query<(Entity, &mut NeedsGroundAlignment), With<Children>>,
    children_query: Query<&Children>,
    aabb_query: Query<(&Aabb, &GlobalTransform)>,
    mesh_query: Query<(), With<Mesh3d>>,
    mut transform_query: Query<&mut Transform>,
) {
    for (entity, mut alignment) in query.iter_mut() {
        let (lowest, all_ready) =
            scan_mesh_bounds(entity, &children_query, &aabb_query, &mesh_query);
        // Once out of patience, settle for the bounds we do have rather than
        // leaving the model hanging where the cursor happened to be — a mesh
        // with no position attribute never gets an `Aabb` at all.
        if !all_ready && alignment.frames_waited < GROUND_ALIGNMENT_MAX_WAIT_FRAMES {
            alignment.frames_waited += 1;
            continue;
        }
        let Some(lowest_world_y) = lowest else {
            alignment.frames_waited += 1;
            continue;
        };

        // `lowest_world_y` comes from last frame's propagated transforms, so
        // the correction is a delta on the current local Y rather than an
        // absolute placement.
        let offset = alignment.target_y - lowest_world_y;
        if let Ok(mut transform) = transform_query.get_mut(entity) {
            transform.translation.y += offset;
        }
        commands.entity(entity).remove::<NeedsGroundAlignment>();
    }
}

/// System: make imported models selectable as a single unit in the viewport.
///
/// Without this, a viewport click resolves to the leaf-most named child mesh
/// (see `renzora_gizmo::find_named_ancestor`), so clicking a model selects a
/// hidden sub-mesh — the Hierarchy shows no selection, and the gizmo ends up
/// rotating a different entity than the one the animation editor reads (which
/// resolves up to the `AnimatorComponent`/model root). Tagging the model root
/// (the `MeshInstanceData` bearer) with `SelectionStop` makes a click select
/// the root — the visible Hierarchy row and the entity that owns the animator.
/// Sub-meshes remain selectable via the Hierarchy tree. Covers fresh imports
/// and scene-loaded models (keyed on the persistent `MeshInstanceData`).
pub fn mark_models_selectable_as_unit(
    mut commands: Commands,
    models: Query<(Entity, &MeshInstanceData), Without<renzora::SelectionStop>>,
) {
    for (entity, data) in &models {
        if data.model_path.is_some() {
            commands.entity(entity).try_insert(renzora::SelectionStop);
        }
    }
}

/// System: auto-discover `.anim` files for entities loaded from scenes that have
/// `MeshInstanceData` (a model) but no `AnimatorComponent` yet.
pub fn auto_discover_animations(
    mut commands: Commands,
    query: Query<
        (Entity, &MeshInstanceData),
        (
            Without<AnimatorComponent>,
            Without<renzora_animation::AnimatorState>,
            Without<AnimationDiscoveryDone>,
        ),
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
#[allow(clippy::too_many_arguments)]
pub fn track_model_drag_preview(
    mut state: ResMut<ModelDragPreviewState>,
    payload: Option<Res<AssetDragPayload>>,
    asset_server: Res<AssetServer>,
    project: Option<Res<CurrentProject>>,
    viewport: Res<ViewportState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    mut mesh_ray_cast: MeshRayCast,
    parent_query: Query<&ChildOf>,
    placeholders: Query<Entity, With<ModelDragPlaceholder>>,
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

        let Some(project) = project.as_deref() else {
            return;
        };
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

        // Load with default settings — the same `Gltf` the dropped entity will
        // use, so the preview shows the real materials and the drop changes
        // nothing about how the model looks. Loading the same path twice with
        // different `GltfLoaderSettings` would poison Bevy's image cache, which
        // is why there is no faster mesh-only load running alongside this one;
        // see `ModelDragPlaceholder` for what fills the wait instead.
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

    // Is there real geometry under the cursor? That decides which placement
    // rule applies, so it has to be re-tested every frame of the drag rather
    // than latched — dragging off the edge of a table has to fall back to the
    // ground plane the moment the cursor leaves it.
    let mut exclude: Vec<Entity> = state.ghost_root.into_iter().collect();
    exclude.extend(&placeholders);
    state.surface_hit = cast_to_scene_surface(&mut mesh_ray_cast, ray, &exclude, &parent_query);
}

/// First hit of `ray` against scene geometry, skipping the drag preview's own
/// meshes.
///
/// The preview follows the cursor, so its own geometry sits directly under the
/// ray every frame — without the exclusion the model would read as standing on
/// itself and climb away from the cursor. `preview_roots` is a list because the
/// preview is *two* things over the life of a drag: the loading placeholder and
/// then the real model. Only one exists at a time, but both have to be excluded
/// while they do, and the placeholder is a separate root rather than a
/// descendant of the model that has not loaded yet.
fn cast_to_scene_surface(
    mesh_ray_cast: &mut MeshRayCast,
    ray: Ray3d,
    preview_roots: &[Entity],
    parent_query: &Query<&ChildOf>,
) -> Option<Vec3> {
    // `early_exit_test` off: the nearest hit is usually the preview itself, and
    // stopping there would report no surface at all.
    let hits = mesh_ray_cast.cast_ray(
        ray,
        &MeshRayCastSettings {
            early_exit_test: &|_| false,
            ..MeshRayCastSettings::default()
        },
    );

    'hits: for (hit_entity, hit) in hits.iter() {
        for root in preview_roots {
            if hit_entity == root || is_descendant_of(*hit_entity, *root, parent_query) {
                continue 'hits;
            }
        }
        return Some(hit.point);
    }
    None
}

/// Walk `entity`'s ancestors looking for `ancestor`.
fn is_descendant_of(entity: Entity, ancestor: Entity, parent_query: &Query<&ChildOf>) -> bool {
    let mut current = entity;
    while let Ok(child_of) = parent_query.get(current) {
        current = child_of.parent();
        if current == ancestor {
            return true;
        }
    }
    false
}

/// System: spawn the model entity once its Gltf is loaded, then track
/// the cursor with its transform until the user releases the mouse.
///
/// The entity we spawn here is the **final** scene entity — same components
/// any post-drop spawn would produce. While the drag is active, this system
/// updates its transform every frame so it follows the cursor. On release,
/// `native_model_drop` adds `NeedsGroundAlignment`
/// and clears the placement state; from there the entity is just a regular
/// scene entity. No "ghost", no despawn-and-respawn — Bevy's SceneSpawner
/// only instantiates the GLB once, and that single instance becomes the
/// real scene model.
#[allow(clippy::too_many_arguments)]
pub fn update_model_drag_ghost(
    mut commands: Commands,
    mut state: ResMut<ModelDragPreviewState>,
    gltf_assets: Res<Assets<Gltf>>,
    mut transform_query: Query<&mut Transform>,
    mut visibility_query: Query<&mut Visibility>,
    children_query: Query<&Children>,
    aabb_query: Query<(&Aabb, &GlobalTransform)>,
    mesh_query: Query<(), With<Mesh3d>>,
    global_query: Query<&GlobalTransform>,
    placeholders: Query<Entity, With<ModelDragPlaceholder>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut placeholder_assets: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
) {
    // Already spawned → just sync transform + visibility.
    if let Some(root) = state.ghost_root {
        // The real model is here; the stand-in has done its job.
        for entity in &placeholders {
            commands.entity(entity).despawn();
        }
        // Measure how far the geometry hangs below the root origin, once — a
        // GLB is free to put its origin anywhere, and a model whose origin sits
        // at the centre (or at the top of a hanging prop) would otherwise be
        // dragged half-buried into whatever it is being placed on, then jump on
        // drop when the alignment finally ran. Both values come from the same
        // propagation, so the one-frame staleness cancels out. Only the
        // drop-onto-geometry rule uses this; an empty-space drop places the
        // origin and leaves the bounds alone.
        if state.bottom_offset.is_none() {
            if let (Some(lowest_y), Ok(root_global)) = (
                lowest_mesh_y(root, &children_query, &aabb_query, &mesh_query),
                global_query.get(root),
            ) {
                state.bottom_offset = Some(root_global.translation().y - lowest_y);
            }
        }
        if let Ok(mut tf) = transform_query.get_mut(root) {
            tf.translation = state.placement_translation();
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
        for entity in &placeholders {
            commands.entity(entity).despawn();
        }
        return;
    }
    let Some(handle) = state.mesh_handle.as_ref() else {
        return;
    };
    let Some(gltf) = gltf_assets.get(handle) else {
        // Still decoding. Put a stand-in where the model will land so the drag
        // has a visible subject from the first frame — see
        // `ModelDragPlaceholder` for why it can't be the model itself.
        let at = state.placement_translation() + Vec3::Y * PLACEHOLDER_SIZE * 0.5;
        if let Some(entity) = placeholders.iter().next() {
            if let Ok(mut tf) = transform_query.get_mut(entity) {
                tf.translation = at;
            }
        } else {
            let (mesh, material) = placeholder_assets
                .get_or_insert_with(|| {
                    (
                        meshes.add(Cuboid::from_length(PLACEHOLDER_SIZE)),
                        materials.add(StandardMaterial {
                            base_color: Color::srgba(0.35, 0.62, 0.95, 0.35),
                            alpha_mode: AlphaMode::Blend,
                            // Unlit and translucent so it reads as "something is
                            // coming", not as a grey box someone dropped.
                            unlit: true,
                            ..default()
                        }),
                    )
                })
                .clone();
            commands.spawn((
                Name::new("Model Preview"),
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_translation(at),
                ModelDragPlaceholder,
                // Editor-internal: out of the hierarchy, and out of a saved
                // scene if the user saves mid-drag.
                renzora::core::HideInHierarchy,
            ));
        }
        return;
    };
    // Loaded. The stand-in is replaced by the real scene below.
    for entity in &placeholders {
        commands.entity(entity).despawn();
    }
    let Some(scene) = gltf
        .default_scene
        .clone()
        .or_else(|| gltf.scenes.first().cloned())
    else {
        // Nothing to show — don't keep retrying.
        state.mesh_handle = None;
        return;
    };

    let display_name = state.name.clone().unwrap_or_else(|| "Model".to_string());

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
        WorldAssetRoot(scene),
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
    placeholders: Query<Entity, With<ModelDragPlaceholder>>,
) {
    if payload.is_some() {
        return;
    }
    if let Some(entity) = state.ghost_root.take() {
        commands.entity(entity).despawn();
    }
    // Also the stand-in, which outlives a drag that ended before the GLB
    // finished loading — the drop path promotes the ghost and never sees one.
    for entity in &placeholders {
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
        if let (Some(handle), Some(server), Some(name)) = (
            state.mesh_handle.as_ref(),
            asset_server,
            state.name.as_ref(),
        ) {
            let loaded = matches!(server.get_load_state(handle.id()), Some(LoadState::Loaded));
            if !loaded {
                out.push((format!("{} (mesh)", name), None));
            }
        }
    }

    if let (Some(pending), Some(server)) = (world.get_resource::<PendingGltfLoads>(), asset_server)
    {
        for load in &pending.loads {
            let loaded = matches!(
                server.get_load_state(load.handle.id()),
                Some(LoadState::Loaded)
            );
            let frac = if loaded { Some(1.0) } else { None };
            out.push((load.name.clone(), frac));
        }
    }

    out
}

// ── Material binding ───────────────────────────────────────────────────────

/// Observer: bring scene-loaded model instances onto the production
/// material-binding path the moment Bevy finishes spawning the GLB
/// hierarchy.
///
/// Drag-and-drop spawns its own production markers from the deferred drop
/// handler — by the time that handler runs, the user's mouse-up has given
/// Bevy several frames to spawn the scene, so the markers don't race the
/// spawn. The load path has no such delay: `finish_mesh_instance_rehydrate`
/// spawns a `SceneRoot` child the same frame the GLB asset finishes
/// loading, and `SceneSpawner::write_to_world` is still in flight when the
/// next system runs. Polling on `Children` non-empty was racing that
/// in-flight spawn; switching to the `SceneInstanceReady` event means we
/// fire exactly once, after every entity in the scene is committed to the
/// world.
///
/// `event_target()` on a `SceneInstanceReady` is the entity holding the
/// `SceneRoot` component — that's the child we spawned in
/// `finish_mesh_instance_rehydrate`. We walk up to its `MeshInstanceData`
/// parent, skip if it already has `ImportedRoot` (drag-drop entities
/// arrive with the marker pre-attached), and add the same trio of markers
/// the drop handler does so the binder + flatten + resolver chain runs.
///
/// The material half of that walk is
/// [`renzora_engine::material_binding::arm_material_binding`], shared with
/// the game — a game has no flatten pass, but it needs its materials bound
/// exactly as the editor does. This observer is the editor's registration;
/// `RuntimePlugin` registers the material-only one when there's no editor.
pub fn decorate_rehydrated_scene_on_ready(
    trigger: On<WorldInstanceReady>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    parents: Query<&ChildOf>,
    mesh_instances: Query<&MeshInstanceData, Without<ImportedRoot>>,
) {
    let scene_root_entity = trigger.event().entity;
    let armed = renzora_engine::material_binding::arm_material_binding(
        scene_root_entity,
        &mut commands,
        &asset_server,
        &parents,
        &mesh_instances,
    );
    if armed.is_some() {
        commands
            .entity(scene_root_entity)
            .try_insert(PendingFlatten::default());
    }
}
