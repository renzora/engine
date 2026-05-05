//! Offscreen capture of model file thumbnails (`.glb`, `.gltf`).
//!
//! Mirrors `renzora_material_editor::file_thumbnails` but for whole GLB
//! scenes: load the model, spawn its `SceneRoot` on a dedicated render
//! layer, frame a camera at the model's AABB, capture the framebuffer,
//! save a PNG to `<project>/.cache/thumbnails/models/<rel>.png`, despawn.
//!
//! Static framing — fixed yaw/pitch (Unity-style) with distance derived
//! from the AABB's max half-extent. Good-enough for browser previews
//! across most model shapes; we can revisit with smart framing if a
//! particular project's models look bad.
//!
//! Persistence: subsequent sessions hit the PNG and skip every step
//! after "load PNG via asset_server". Invalidation is automatic — the
//! cache lookup compares mtime against the source.

use std::path::PathBuf;

use bevy::asset::LoadState;
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::scene::SceneInstanceReady;
use bevy_egui::{EguiTextureHandle, EguiUserTextures};

use renzora::core::{CurrentProject, EditorLocked, HideInHierarchy, IsolatedCamera};
use renzora_editor::{model_thumb_path, ModelThumbnailRegistry};

/// Render layer the offscreen capture lives on so the user's viewport
/// camera doesn't see the spawned model. Different from the material
/// thumbnail's layer 7 so the two systems can run concurrently without
/// rendering each other's content.
pub const MODEL_THUMBNAIL_LAYER: usize = 8;

/// Output resolution for the captured PNG. Same as material thumbnails
/// — asset browser displays at ~96px so 256 leaves headroom for HiDPI.
const THUMB_SIZE: u32 = 256;

/// Maximum `intake_model_requests` work per frame. Each intake either
/// kicks an asset load or skips to disk-load — both cheap, but bulk
/// re-opens still benefit from a per-frame cap.
const MAX_INTAKE_PER_FRAME: usize = 4;

/// Concurrent captures. Each occupies its own world-space cell so its
/// camera doesn't see another job's model. Bevy's directional-light
/// limit (10) and the render-target sharing both make multi-capture
/// trivial; pick a number that's high enough to drain a project's
/// models in the splash window without flooding the GPU.
const MAX_CONCURRENT_CAPTURES: usize = 4;

/// World-space spacing between concurrent capture cells. Models are
/// spawned at their cell origin and the camera is offset relative to
/// that origin, so cells must be far enough apart that a wide model
/// in cell N doesn't bleed into cell N+1's camera frustum.
const CAPTURE_CELL_SPACING: f32 = 200.0;

/// Frames to wait after the camera is placed before triggering the
/// screenshot. Two costs need to be amortized over this window:
///   * Texture decode + GPU upload — a complex GLB (car, building) can
///     reference 50+ textures. Bevy's loader streams them in async; if
///     we screenshot before they land the model renders with the
///     fallback white material.
///   * StandardMaterial pipeline specialization — wgpu compiles a
///     specialized pipeline the first time each material variant
///     hits the render pass. With many materials per GLB the queue
///     can take a second to drain.
/// 90 frames ≈ 1.5s at 60fps — generous but captures are async and
/// each model only pays this once per session (subsequent sessions
/// hit the PNG cache).
const CAPTURE_WARMUP_FRAMES: u32 = 90;

/// Frames to wait for the GLB asset to load before giving up. ~10s at
/// 60fps — enough for any reasonable project asset on disk.
const ASSET_LOAD_TIMEOUT_FRAMES: u32 = 600;

/// State for a model thumbnail request that's been picked up from the
/// registry but isn't yet captured. Covers the load → spawn → wait →
/// screenshot lifecycle as a single struct so we can iterate the work
/// list cheaply each frame.
struct PendingCapture {
    model_path: PathBuf,
    thumb_path: PathBuf,
    /// Strong handle to the GLB asset. Kept alive until despawn so the
    /// asset doesn't evict mid-capture if some other system happens to
    /// drop its handle.
    gltf_handle: Handle<Gltf>,
    /// `None` until the GLB asset finishes loading and we've spawned
    /// the parent + scene root. Once set, this is the parent we'll
    /// despawn (recursively) after the screenshot.
    parent: Option<Entity>,
    /// `None` until the scene's `SceneInstanceReady` event fires and we
    /// know the AABB. Once set, this is the camera we'll despawn after
    /// the screenshot.
    camera: Option<Entity>,
    /// `None` until we've created the render target image.
    render_image: Option<Handle<Image>>,
    /// World-space cell index allocated to this capture. The model and
    /// camera both live at `cell_origin = (cell * SPACING, 0, 0)`.
    cell_idx: usize,
    /// Frames since this capture started. Used for the asset-load
    /// timeout so we don't spin forever on a missing/broken GLB.
    frames_waited: u32,
    /// `Some(_)` once `SceneInstanceReady` fires (observer flips it).
    /// Distinguishes "we know the subtree exists" from "we're still
    /// waiting on the spawn." The actual readiness check (AABBs done,
    /// transforms settled) is condition-based — see
    /// [`subtree_aabbs_ready`].
    scene_ready: bool,
    /// Once the scene is ready, settled, and the camera is positioned,
    /// count down `CAPTURE_WARMUP_FRAMES` before triggering the
    /// screenshot. `None` means we haven't placed the camera yet.
    warmup_remaining: Option<u32>,
    /// Set once the screenshot has been dispatched, so the cleanup
    /// observer can despawn entities exactly once.
    screenshot_dispatched: bool,
}

// AABB readiness is condition-based, not frame-based. After
// `SceneInstanceReady` fires we walk the subtree each frame and check
// for any `Mesh3d` entity that's still missing `Aabb`. Once that
// count drops to zero, `compute_aabb_system` has caught up and we can
// trust the bounding-box union to reflect the full model.

#[derive(Resource, Default)]
struct PendingCaptures {
    jobs: Vec<PendingCapture>,
}

/// Disk-cached PNG loads in flight. When `Handle<Image>` reaches
/// `LoadState::Loaded`, register the texture with egui and complete
/// the registry entry.
#[derive(Resource, Default)]
struct PendingDiskLoads {
    entries: Vec<PendingDiskLoad>,
}

struct PendingDiskLoad {
    model_path: PathBuf,
    handle: Handle<Image>,
}

/// Bitmask cell allocator — same pattern as material thumbnails. Lets
/// us run up to `MAX_CONCURRENT_CAPTURES` (≤ 64) without colliding.
#[derive(Resource, Default)]
struct CaptureCells {
    busy_mask: u64,
}

impl CaptureCells {
    fn alloc(&mut self) -> Option<usize> {
        for i in 0..MAX_CONCURRENT_CAPTURES {
            let bit = 1u64 << i;
            if self.busy_mask & bit == 0 {
                self.busy_mask |= bit;
                return Some(i);
            }
        }
        None
    }
    fn free(&mut self, idx: usize) {
        if idx < 64 {
            self.busy_mask &= !(1u64 << idx);
        }
    }
    fn cell_origin(idx: usize) -> Vec3 {
        Vec3::new(idx as f32 * CAPTURE_CELL_SPACING, 0.0, 0.0)
    }
}

/// Marker on the parent entity of a model-thumbnail capture so we can
/// match the `SceneInstanceReady` event back to its job. `parent_index`
/// is the index into `PendingCaptures::jobs`; we compare directly
/// rather than tracking entity IDs because Bevy may shuffle entities.
#[derive(Component)]
struct ModelThumbnailJob {
    /// The job's `model_path` — used to look it up in `PendingCaptures`.
    /// String compare is cheap and survives indexing changes.
    model_path: PathBuf,
}

/// Plugin entry — registers resources + systems. Asset browser owns
/// this so the renderer is colocated with `ThumbnailCache` (textures);
/// dependency direction is browser → editor (registry contract).
pub struct ModelThumbnailPlugin;

impl Plugin for ModelThumbnailPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingCaptures>()
            .init_resource::<PendingDiskLoads>()
            .init_resource::<CaptureCells>()
            .add_observer(on_scene_instance_ready)
            .add_systems(
                Update,
                (
                    intake_model_requests,
                    tick_capture_lifecycle,
                    tick_warmup_and_dispatch,
                    resolve_model_disk_loads,
                )
                    .chain(),
            );
    }
}

/// Drain incoming requests from the registry. For each, decide between
/// disk-cache hit (cheap reload) and offscreen capture (full pipeline).
fn intake_model_requests(
    mut registry: ResMut<ModelThumbnailRegistry>,
    mut pending: ResMut<PendingCaptures>,
    mut disk_loads: ResMut<PendingDiskLoads>,
    mut cells: ResMut<CaptureCells>,
    asset_server: Res<AssetServer>,
    project: Option<Res<CurrentProject>>,
) {
    let Some(project) = project else {
        // No project — drop the queue. Each request can be retried
        // after the user opens a project.
        registry.incoming_requests.clear();
        return;
    };

    for _ in 0..MAX_INTAKE_PER_FRAME {
        let Some(model_path) = registry.incoming_requests.pop_front() else {
            break;
        };

        let thumb_path = model_thumb_path(&model_path, &project);

        // Disk cache hit: load the cached PNG via asset_server and
        // complete the registry entry once the texture is ready.
        if cached_thumb_is_fresh(&thumb_path, &model_path) {
            if let Some(handle) = try_load_cached_thumb(&thumb_path, &asset_server, &project) {
                disk_loads
                    .entries
                    .push(PendingDiskLoad { model_path, handle });
                continue;
            }
        }

        // Cache miss / stale: kick a GLB load. The capture lifecycle
        // system will spawn the scene once the asset arrives.
        let asset_path = project.make_asset_relative(&model_path);
        if std::path::Path::new(&asset_path).is_absolute() {
            // Path isn't under the project root — can't load via the
            // asset server. Leave the in-flight flag set so the
            // browser shows the fallback icon and never re-requests.
            registry.cancel(&model_path);
            continue;
        }
        let gltf_handle: Handle<Gltf> = asset_server.load(asset_path);

        // Allocate a cell up-front. If all are busy, push back to the
        // front of the queue and try again next frame; intake budget
        // limits how many we'd be re-pushing in the worst case.
        let Some(cell_idx) = cells.alloc() else {
            registry.incoming_requests.push_front(model_path);
            return;
        };

        // Spawn nothing yet — we wait for the GLB to load before
        // creating the SceneRoot child. `tick_capture_lifecycle` polls
        // each job and advances when the asset is ready.
        pending.jobs.push(PendingCapture {
            model_path,
            thumb_path,
            gltf_handle,
            parent: None,
            camera: None,
            render_image: None,
            cell_idx,
            frames_waited: 0,
            scene_ready: false,
            warmup_remaining: None,
            screenshot_dispatched: false,
        });
    }
}

/// Per-frame: advance each pending capture through its lifecycle.
/// Phases:
/// 1. **GLB loading** — wait for `Assets<Gltf>::get` to return a scene.
///    Once available, spawn parent entity + SceneRoot child + render
///    target + lights on the thumbnail layer.
/// 2. **Scene spawning** — Bevy's SceneSpawner is in-flight. The
///    `on_scene_instance_ready` observer flips `frames_since_scene_ready`
///    to `Some(0)` once the spawn commits.
/// 3. **AABB settle** — wait `FRAMING_SETTLE_FRAMES` frames after the
///    scene is ready so transform propagation + `compute_aabb_system`
///    have run on every newly-spawned mesh entity. Then walk the AABB
///    union, frame the camera, arm the warmup countdown.
/// 4. **Warmup** — `tick_warmup_and_dispatch` counts down then fires
///    the screenshot.
/// 5. **Screenshot in flight** — its observer despawns entities, frees
///    the cell, completes the registry entry.
///
/// Timeouts: any job that exceeds `ASSET_LOAD_TIMEOUT_FRAMES` without
/// reaching phase 3 is cancelled (registry entry cleared, cell freed).
fn tick_capture_lifecycle(
    mut commands: Commands,
    mut pending: ResMut<PendingCaptures>,
    mut cells: ResMut<CaptureCells>,
    mut images: ResMut<Assets<Image>>,
    mut registry: ResMut<ModelThumbnailRegistry>,
    gltf_assets: Option<Res<Assets<Gltf>>>,
    children_q: Query<&Children>,
    aabb_q: Query<(&Aabb, &GlobalTransform)>,
    mesh_q: Query<(), With<Mesh3d>>,
    has_aabb_q: Query<(), With<Aabb>>,
) {
    let Some(gltf_assets) = gltf_assets else {
        return;
    };

    // Walk in reverse so we can swap_remove dead entries cheaply.
    for i in (0..pending.jobs.len()).rev() {
        let job = &mut pending.jobs[i];
        job.frames_waited += 1;

        // Phase 1: GLB still loading?
        if job.parent.is_none() {
            let Some(gltf) = gltf_assets.get(&job.gltf_handle) else {
                if job.frames_waited > ASSET_LOAD_TIMEOUT_FRAMES {
                    warn!(
                        "[model_thumbnails] timeout loading {} after {} frames",
                        job.model_path.display(),
                        job.frames_waited
                    );
                    cells.free(job.cell_idx);
                    registry.cancel(&job.model_path);
                    pending.jobs.swap_remove(i);
                }
                continue;
            };

            let Some(scene_handle) = gltf
                .default_scene
                .clone()
                .or_else(|| gltf.scenes.first().cloned())
            else {
                warn!(
                    "[model_thumbnails] {} has no scenes",
                    job.model_path.display()
                );
                cells.free(job.cell_idx);
                registry.cancel(&job.model_path);
                pending.jobs.swap_remove(i);
                continue;
            };

            spawn_capture_entities(&mut commands, &mut images, job, scene_handle);
            continue;
        }

        // Camera already placed — tick_warmup_and_dispatch handles the
        // countdown.
        if job.warmup_remaining.is_some() {
            continue;
        }

        // Phase 2: spawn committed?
        if !job.scene_ready {
            // Defensive timeout: GLB spawned but SceneInstanceReady
            // never fires (empty scene, broken file, etc.).
            if job.frames_waited > ASSET_LOAD_TIMEOUT_FRAMES {
                warn!(
                    "[model_thumbnails] no SceneInstanceReady for {} after {} frames",
                    job.model_path.display(),
                    job.frames_waited
                );
                if let Some(parent) = job.parent {
                    commands.entity(parent).despawn();
                }
                cells.free(job.cell_idx);
                registry.cancel(&job.model_path);
                pending.jobs.swap_remove(i);
            }
            continue;
        }

        // Phase 3: AABBs ready? Condition-based — every `Mesh3d`
        // entity in the subtree must have an `Aabb`. `compute_aabb_system`
        // processes entities incrementally each PostUpdate, so on a
        // heavy GLB this can take several frames; polling for the
        // condition rather than waiting a fixed window means we frame
        // the moment the data is actually valid.
        let Some(parent) = job.parent else { continue };
        if !subtree_aabbs_ready(parent, &children_q, &mesh_q, &has_aabb_q) {
            continue;
        }

        // Settled — frame the model and place the camera. After this,
        // `warmup_remaining = Some(N)` and the dispatch system takes
        // over.
        place_capture_camera(&mut commands, job, parent, &children_q, &aabb_q);

        // If place_capture_camera couldn't find any AABB (subtree
        // had no meshes), bail.
        if job.warmup_remaining.is_none() {
            warn!(
                "[model_thumbnails] no AABBs found under spawned scene for {} — skipping",
                job.model_path.display()
            );
            commands.entity(parent).despawn();
            cells.free(job.cell_idx);
            registry.cancel(&job.model_path);
            pending.jobs.swap_remove(i);
        }
    }
}

/// True iff every `Mesh3d` entity in the subtree rooted at `root` also
/// has an `Aabb` component. `compute_aabb_system` runs each PostUpdate
/// and processes `Without<Aabb>, With<Mesh3d>` entities — so this
/// condition flips from false → true once Bevy has caught up on the
/// just-spawned scene. A subtree with no meshes also returns `true`
/// (degenerate-scene case); the caller will then bail out of framing
/// because `world_aabb` will return `None`.
fn subtree_aabbs_ready(
    root: Entity,
    children_q: &Query<&Children>,
    mesh_q: &Query<(), With<Mesh3d>>,
    has_aabb_q: &Query<(), With<Aabb>>,
) -> bool {
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if mesh_q.contains(e) && !has_aabb_q.contains(e) {
            return false;
        }
        if let Ok(kids) = children_q.get(e) {
            stack.extend(kids.iter());
        }
    }
    true
}

/// Walk the spawned scene's AABB and place the camera so the model
/// fills ~80% of a 45° FOV frame. Must run *after* transform
/// propagation + `compute_aabb_system` have settled — see
/// [`FRAMING_SETTLE_FRAMES`]. Sets `job.camera` and `warmup_remaining`
/// on success; leaves them `None` if no AABB was found in the subtree.
fn place_capture_camera(
    commands: &mut Commands,
    job: &mut PendingCapture,
    parent: Entity,
    children_q: &Query<&Children>,
    aabb_q: &Query<(&Aabb, &GlobalTransform)>,
) {
    // Find the SceneRoot child under the parent — its descendants are
    // the GLB entities whose AABBs we need. The lights and (later)
    // camera are also children, so we have to pick the one that's
    // *not* a light or camera. The SceneRoot is always the first
    // sibling we spawn though (lights come after), so taking the
    // first child works in practice.
    let Some(scene_root) = children_q
        .get(parent)
        .ok()
        .and_then(|kids| kids.iter().next())
    else {
        return;
    };

    let Some((min_world, max_world)) = world_aabb(scene_root, children_q, aabb_q) else {
        return;
    };
    let center_world = (min_world + max_world) * 0.5;
    let half_extents = (max_world - min_world) * 0.5;

    // Smart framing: project the AABB's 8 corners onto the camera's
    // view plane and pick a distance that fits the projection in the
    // frame at ~80% fill — independent of the model's aspect ratio.
    //
    // Static framing (radius * fixed factor) over-allocates space for
    // round-ish models and under-allocates for elongated ones; smart
    // framing tightens both cases by using the actual silhouette
    // extent along the camera's right and up axes.
    //
    // Direction: 30° yaw (horizontal angle of attack) + 20° pitch down.
    // Far enough from front to show side detail (cars), tame enough
    // pitch to keep tall objects (buildings) from running off the top.
    let yaw = 0.55_f32; // ~31°
    let pitch = -0.35_f32; // ~20° down
    let look_dir = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch) * Vec3::Z;
    let look_dir = look_dir.normalize();

    // Camera basis: right and up vectors orthogonal to look_dir.
    // Choosing world Y as the "rough up" gives a stable orientation
    // for the right vector; if look_dir happens to point straight up
    // (it won't with our yaw/pitch) we'd need a fallback.
    let right = look_dir.cross(Vec3::Y).normalize();
    let up = right.cross(look_dir).normalize();

    // Project the AABB extents onto right/up. The projected silhouette
    // half-width is the sum of |half_extents·right| over the 3 axes;
    // half-height is similarly the sum onto `up`. This is exact for
    // axis-aligned boxes against any oriented camera.
    let half_w = (half_extents.x * right.x).abs()
        + (half_extents.y * right.y).abs()
        + (half_extents.z * right.z).abs();
    let half_h = (half_extents.x * up.x).abs()
        + (half_extents.y * up.y).abs()
        + (half_extents.z * up.z).abs();

    // 45° FOV → vertical half-angle 22.5° → tan ≈ 0.4142. The thumbnail
    // is square (1:1), so horizontal half-angle is the same.
    // Distance to fit half_h vertically: d = half_h / tan(22.5°).
    // Distance to fit half_w horizontally: same formula since aspect=1.
    // Take the larger so neither dimension clips, then divide by 0.85
    // for ~85% fill (a touch of margin so silhouettes don't kiss the
    // frame edge).
    const FOV_HALF_TAN: f32 = 0.4142; // tan(45°/2)
    const FILL_FRACTION: f32 = 0.85;
    let fit_distance = (half_w.max(half_h) / FOV_HALF_TAN) / FILL_FRACTION;
    // Floor so a microscopic AABB still gives a sane camera position.
    let distance = fit_distance.max(0.5);

    // Camera looks along -look_dir toward center, so position is
    // center + look_dir * distance (positive = away from look-target).
    let camera_pos = center_world + look_dir * distance;

    let Some(render_image_handle) = job.render_image.clone() else {
        return;
    };

    let camera = commands
        .spawn((
            Camera3d::default(),
            Msaa::Sample4,
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgba(0.08, 0.08, 0.1, 1.0)),
                order: -2000 - (job.cell_idx as isize),
                is_active: true,
                ..default()
            },
            RenderTarget::Image(render_image_handle.into()),
            Transform::from_translation(camera_pos).looking_at(center_world, Vec3::Y),
            // Per-camera ambient override so PBR materials without an
            // environment map don't have crushed-black indirect terms
            // — the global ambient is too low (80 cd/m²) for a clean
            // 3-light preview rig.
            AmbientLight {
                color: Color::srgb(0.9, 0.9, 1.0),
                brightness: 800.0,
                affects_lightmapped_meshes: false,
            },
            ChildOf(parent),
            RenderLayers::layer(MODEL_THUMBNAIL_LAYER),
            IsolatedCamera,
            HideInHierarchy,
            EditorLocked,
            Name::new("Model Thumbnail Camera"),
        ))
        .id();
    job.camera = Some(camera);
    job.warmup_remaining = Some(CAPTURE_WARMUP_FRAMES);
}

/// Spawn the parent + SceneRoot child + render target image + lights
/// for a capture. Camera is added later by the `SceneInstanceReady`
/// observer once it knows the model's AABB.
fn spawn_capture_entities(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    job: &mut PendingCapture,
    scene_handle: Handle<bevy::scene::Scene>,
) {
    let cell_origin = CaptureCells::cell_origin(job.cell_idx);

    // Render target — the camera (added later) will write into this,
    // and the screenshot observer will read from it.
    let extent = Extent3d {
        width: THUMB_SIZE,
        height: THUMB_SIZE,
        depth_or_array_layers: 1,
    };
    let mut render_image = Image {
        data: Some(vec![0u8; (extent.width * extent.height * 4) as usize]),
        ..default()
    };
    render_image.texture_descriptor.size = extent;
    render_image.texture_descriptor.format = TextureFormat::Bgra8UnormSrgb;
    render_image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_DST
        | TextureUsages::COPY_SRC
        | TextureUsages::RENDER_ATTACHMENT;
    let render_image_handle = images.add(render_image);
    job.render_image = Some(render_image_handle.clone());

    // Parent entity — holds the job marker so the SceneInstanceReady
    // observer can find its way back to this capture by model_path.
    let parent = commands
        .spawn((
            Name::new(format!(
                "Model Thumbnail Parent ({})",
                job.model_path.display()
            )),
            Transform::from_translation(cell_origin),
            Visibility::Visible,
            RenderLayers::layer(MODEL_THUMBNAIL_LAYER),
            HideInHierarchy,
            EditorLocked,
            ModelThumbnailJob {
                model_path: job.model_path.clone(),
            },
        ))
        .id();

    // SceneRoot child — Bevy's SceneSpawner will populate this with
    // the GLB hierarchy. Forces all spawned descendants onto the
    // thumbnail render layer via `RenderLayers` propagation.
    commands.spawn((
        Name::new("Model Thumbnail SceneRoot"),
        bevy::scene::SceneRoot(scene_handle),
        Transform::default(),
        Visibility::Visible,
        ChildOf(parent),
        RenderLayers::layer(MODEL_THUMBNAIL_LAYER),
        HideInHierarchy,
        EditorLocked,
    ));

    // Three-light rig for the offscreen capture:
    //   * Key light — bright warm directional, models the dominant
    //     illumination from "above-front-left".
    //   * Fill light — cooler directional from the opposite side so
    //     shadow surfaces aren't pitch black.
    //   * Ambient — unshadowed diffuse fill so PBR materials without
    //     an environment map (the typical case for the asset browser
    //     since we don't load IBL) don't have crushed-black indirect
    //     terms. Without this, anything not facing the key/fill
    //     directly reads as featureless dark grey.
    //
    // All three are children of the parent so they despawn together.
    // Higher illuminance than material thumbnails because models are
    // typically much larger than the 1-unit sphere (so per-fragment
    // exposure runs through more of the tonemapping curve before
    // saturating).
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.98, 0.95),
            illuminance: 12000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.6, 0.4, 0.0)),
        RenderLayers::layer(MODEL_THUMBNAIL_LAYER),
        ChildOf(parent),
        HideInHierarchy,
        EditorLocked,
        Name::new("Model Thumbnail Key Light"),
    ));
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.6, 0.7, 0.9),
            illuminance: 4000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, 0.3, -0.8, 0.0)),
        RenderLayers::layer(MODEL_THUMBNAIL_LAYER),
        ChildOf(parent),
        HideInHierarchy,
        EditorLocked,
        Name::new("Model Thumbnail Fill Light"),
    ));
    // Ambient light is added to the camera entity in
    // `place_capture_camera` — `AmbientLight` is a per-camera
    // component (#[require(Camera)]) so it must be on the camera or
    // it'd auto-insert its own default Camera and conflict with our
    // setup.

    job.parent = Some(parent);
}

/// Observer: when a model's GLB scene finishes spawning, do two
/// things:
///
/// 1. **Confine the spawned hierarchy to the thumbnail render layer.**
///    `RenderLayers` doesn't propagate from parent to child — Bevy's
///    `SceneSpawner` writes new entities with no `RenderLayers`
///    component, which means they default to layer 0. Layer 0 is what
///    the editor's main viewport camera renders, so without this walk
///    every captured model briefly appears in the editor scene before
///    the screenshot finishes and we despawn it. Walking the subtree
///    once and inserting `RenderLayers::layer(MODEL_THUMBNAIL_LAYER)`
///    on every entity makes them invisible to any camera that doesn't
///    explicitly include this layer.
///
/// 2. **Mark the job's scene as "spawned"** so `tick_capture_lifecycle`
///    knows to start polling for AABB readiness. We deliberately *don't*
///    place the camera here — the observer runs in the SpawnScene
///    schedule, before PostUpdate, so transforms haven't propagated
///    and `compute_aabb_system` hasn't run yet on the new mesh
///    entities. Polling for "all `Mesh3d` in the subtree have `Aabb`"
///    is condition-based and doesn't depend on a magic frame count.
fn on_scene_instance_ready(
    trigger: On<SceneInstanceReady>,
    mut commands: Commands,
    mut pending: ResMut<PendingCaptures>,
    parents: Query<&ChildOf>,
    job_markers: Query<&ModelThumbnailJob>,
    children_q: Query<&Children>,
) {
    let scene_root_entity = trigger.event().entity;
    if scene_root_entity == Entity::PLACEHOLDER {
        return;
    }
    let Ok(child_of) = parents.get(scene_root_entity) else {
        return;
    };
    let parent_entity = child_of.parent();
    let Ok(marker) = job_markers.get(parent_entity) else {
        // Not a model-thumbnail scene — some other system's
        // SceneInstanceReady. Bail.
        return;
    };

    // Walk the SceneRoot's subtree and put every entity on the
    // thumbnail render layer so the main viewport camera doesn't
    // see them. Iterative DFS keeps the stack flat.
    let mut stack = vec![scene_root_entity];
    while let Some(e) = stack.pop() {
        commands
            .entity(e)
            .try_insert(RenderLayers::layer(MODEL_THUMBNAIL_LAYER));
        if let Ok(kids) = children_q.get(e) {
            stack.extend(kids.iter());
        }
    }

    if let Some(job) = pending
        .jobs
        .iter_mut()
        .find(|j| j.model_path == marker.model_path)
    {
        job.scene_ready = true;
    }
}

/// Walk a scene tree starting from `root` and union all AABBs (in
/// world space) into a (min, max) pair. Returns `None` if no AABB is
/// found in the subtree.
fn world_aabb(
    root: Entity,
    children_q: &Query<&Children>,
    aabb_q: &Query<(&Aabb, &GlobalTransform)>,
) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut found_any = false;
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if let Ok((aabb, tf)) = aabb_q.get(e) {
            let center = Vec3::from(aabb.center);
            let half = Vec3::from(aabb.half_extents);
            for sx in [-1.0f32, 1.0] {
                for sy in [-1.0f32, 1.0] {
                    for sz in [-1.0f32, 1.0] {
                        let local = center + half * Vec3::new(sx, sy, sz);
                        let world = tf.transform_point(local);
                        min = min.min(world);
                        max = max.max(world);
                        found_any = true;
                    }
                }
            }
        }
        if let Ok(kids) = children_q.get(e) {
            stack.extend(kids.iter());
        }
    }
    if found_any {
        Some((min, max))
    } else {
        None
    }
}

/// Resolve disk-cached PNG loads — when each handle reaches `Loaded`,
/// register with egui and complete the registry entry.
fn resolve_model_disk_loads(
    mut disk_loads: ResMut<PendingDiskLoads>,
    mut user_textures: ResMut<EguiUserTextures>,
    mut registry: ResMut<ModelThumbnailRegistry>,
    asset_server: Res<AssetServer>,
) {
    let mut i = 0;
    while i < disk_loads.entries.len() {
        let state = asset_server.get_load_state(&disk_loads.entries[i].handle);
        match state {
            Some(LoadState::Loaded) => {
                let entry = disk_loads.entries.swap_remove(i);
                user_textures.add_image(EguiTextureHandle::Strong(entry.handle.clone()));
                match user_textures.image_id(entry.handle.id()) {
                    Some(tid) => registry.complete(entry.model_path, tid),
                    None => registry.cancel(&entry.model_path),
                }
            }
            Some(LoadState::Failed(_)) => {
                let entry = disk_loads.entries.swap_remove(i);
                warn!(
                    "[model_thumbnails] cached PNG failed to load for {}; will recapture",
                    entry.model_path.display()
                );
                registry.cancel(&entry.model_path);
            }
            _ => i += 1,
        }
    }
}

/// True iff the cached thumbnail file is fresh — exists and its mtime
/// is newer than the source's mtime. Identical pattern to the
/// texture cache's freshness check; duplicated to keep the model
/// renderer self-contained (no dep on `thumbnails::cached_thumb_is_fresh`).
fn cached_thumb_is_fresh(cache_path: &std::path::Path, source_path: &std::path::Path) -> bool {
    let Ok(cache_meta) = std::fs::metadata(cache_path) else {
        return false;
    };
    let Ok(source_meta) = std::fs::metadata(source_path) else {
        return true;
    };
    let (Ok(cache_mtime), Ok(source_mtime)) = (cache_meta.modified(), source_meta.modified())
    else {
        return false;
    };
    cache_mtime >= source_mtime
}

/// Load a cached PNG via the asset server. Returns `None` if the path
/// can't be made asset-relative (i.e. it isn't under the project root).
fn try_load_cached_thumb(
    thumb_path: &std::path::Path,
    asset_server: &AssetServer,
    project: &CurrentProject,
) -> Option<Handle<Image>> {
    let rel = project.make_asset_relative(thumb_path);
    if std::path::Path::new(&rel).is_absolute() {
        return None;
    }
    Some(asset_server.load(rel))
}

/// Per-frame warmup countdown + screenshot dispatch. Separated from
/// `tick_capture_lifecycle` so we can run after `on_scene_instance_ready`
/// has armed the warmup — observers run between systems, and we need
/// the warmup_remaining = Some(N) state to be visible by the time
/// this runs the same frame.
pub fn tick_warmup_and_dispatch(mut commands: Commands, mut pending: ResMut<PendingCaptures>) {
    // Two passes: countdown, then dispatch ready ones. Avoids the
    // borrow-checker complaint about iterating + mutating + spawning
    // in one pass.
    for job in pending.jobs.iter_mut() {
        if let Some(remaining) = job.warmup_remaining.as_mut() {
            if *remaining > 0 {
                *remaining -= 1;
            }
        }
    }

    let mut to_dispatch: Vec<usize> = Vec::new();
    for (i, job) in pending.jobs.iter().enumerate() {
        if job.screenshot_dispatched {
            continue;
        }
        if matches!(job.warmup_remaining, Some(0)) {
            to_dispatch.push(i);
        }
    }

    for &i in to_dispatch.iter() {
        let job = &mut pending.jobs[i];
        let (Some(parent), Some(render_image)) = (job.parent, job.render_image.clone()) else {
            continue;
        };
        let model_path = job.model_path.clone();
        let thumb_path = job.thumb_path.clone();
        let cell_idx = job.cell_idx;
        job.screenshot_dispatched = true;

        commands
            .spawn((
                Screenshot::image(render_image),
                HideInHierarchy,
                EditorLocked,
                Name::new("Model Thumbnail Screenshot"),
            ))
            .observe(
                move |trigger: On<ScreenshotCaptured>,
                      mut cmds: Commands,
                      mut imgs: ResMut<Assets<Image>>,
                      mut tex: ResMut<EguiUserTextures>,
                      mut reg: ResMut<ModelThumbnailRegistry>,
                      mut pend: ResMut<PendingCaptures>,
                      mut cls: ResMut<CaptureCells>| {
                    cls.free(cell_idx);
                    cmds.entity(parent).despawn();

                    let captured = trigger.image.clone();
                    if let Some(parent_dir) = thumb_path.parent() {
                        let _ = std::fs::create_dir_all(parent_dir);
                    }
                    match captured.clone().try_into_dynamic() {
                        Ok(dyn_img) => {
                            let rgba = dyn_img.to_rgba8();
                            if let Err(e) = rgba.save(&thumb_path) {
                                warn!(
                                    "[model_thumbnails] couldn't write {}: {}",
                                    thumb_path.display(),
                                    e
                                );
                            }
                        }
                        Err(e) => warn!("[model_thumbnails] capture format unsupported: {}", e),
                    }

                    let captured_handle = imgs.add(captured);
                    tex.add_image(EguiTextureHandle::Strong(captured_handle.clone()));
                    match tex.image_id(captured_handle.id()) {
                        Some(tid) => reg.complete(model_path.clone(), tid),
                        None => reg.cancel(&model_path),
                    }

                    if let Some(idx) = pend.jobs.iter().position(|j| j.model_path == model_path) {
                        pend.jobs.swap_remove(idx);
                    }
                },
            );
    }

    // `cls` (CaptureCells) is freed inside the screenshot observer.
    // The local borrow above isn't used after dispatch, but having
    // it as a system arg ensures Bevy schedules us with exclusive
    // access during the frame — preventing two concurrent dispatches
    // from racing on the cell mask.
    let _ = &mut pending;
}
