//! Native 3D turntable preview for the marketplace item overlay.
//!
//! When the overlay opens on a "3D Models" / "Animations" asset, we download the
//! GLB, render it to an offscreen texture on a spinning turntable, and show that
//! texture as the overlay's main viewer (the same `Handle<Image>`-in-an-`ImageNode`
//! path the image gallery uses — no ember change needed).
//!
//! Why render-to-texture rather than a static thumbnail: a store thumbnail is one
//! frozen angle. A live turntable lets the shopper read silhouette, scale, and
//! surfacing before buying, which is the whole point of a "preview".
//!
//! ## The GLB-from-downloaded-bytes problem
//!
//! `AssetServer::load::<Gltf>` loads by *path* from an asset source, but the model
//! bytes come off the marketplace, not the project. The engine's custom asset
//! reader (`renzora_engine::asset_reader::EmbeddedAssetReader`) resolves **absolute
//! paths by reading the filesystem directly** (its first lookup branch), and Bevy's
//! `From<PathBuf> for AssetPath` keeps the path on the default source without any
//! string parsing (so a Windows `C:\…` drive letter is *not* mistaken for an asset
//! source). GLBs are self-contained (buffers + textures are embedded), so there are
//! no file-relative sub-asset references to break. Put together: we write the
//! downloaded `.glb` to a temp cache file and `asset_server.load::<Gltf>(that
//! absolute PathBuf)`. No temp `AssetSource` registration required.
//!
//! ## Confining the spawned scene to the preview layer
//!
//! `SceneSpawner` writes new entities with no `RenderLayers`, so they default to
//! layer 0 — the editor's main viewport — and would render *there* (and be
//! invisible to our preview camera). On `WorldInstanceReady` we walk the spawned
//! subtree and stamp every descendant onto the preview layer, exactly like the
//! asset browser's model-thumbnail capture.

use std::path::PathBuf;

use bevy::asset::LoadState;
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::Hdr;
use bevy::camera::RenderTarget;
use bevy::core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass, NormalPrepass};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy::world_serialization::{WorldAssetRoot, WorldInstanceReady};
use crossbeam_channel::{unbounded, Receiver, TryRecvError};

use renzora::core::{EditorLocked, HideInHierarchy, IsolatedCamera};
use renzora::SplashState;
use renzora_auth::marketplace::AssetSummary;
use renzora_grid::{InfiniteGrid, InfiniteGridSettings};

/// A fresh render layer for the marketplace preview — distinct from the material
/// preview (8), studio preview (10), and the asset-browser thumbnail captures
/// (7/8) so none of them can see each other's content.
const MODEL_VIEWER_LAYER: usize = 13;

/// Offscreen render-target resolution. 16:9 to match the overlay's landscape
/// header viewer; 640×360 is crisp at the ~600px card width with HiDPI headroom.
const RTT_W: u32 = 640;
const RTT_H: u32 = 360;

/// Fraction of the frame the model's bounding sphere should fill (a touch of
/// margin so a spinning silhouette never kisses the frame edge).
const FILL_FRACTION: f32 = 0.85;
/// `tan(fov/2)` for Bevy's default 45° perspective FOV. The RTT is 16:9, so the
/// vertical axis is the tighter one — framing to it fits horizontally too.
const FOV_HALF_TAN: f32 = 0.4142;

/// Frames to wait for the GLB to load + its scene to settle before giving up
/// (~15s at 60fps). A broken/oversized file then falls back to the gallery
/// instead of spinning forever on a placeholder.
const LOAD_TIMEOUT_FRAMES: u32 = 900;

// ── Resources ────────────────────────────────────────────────────────────────

/// The offscreen render target the overlay displays. Handed to `bind_with` in
/// `item_overlay::build_header`, mirroring `MaterialPreviewImage`.
#[derive(Resource)]
pub(crate) struct ModelPreviewImage {
    pub handle: Handle<Image>,
    #[allow(dead_code)]
    pub size: (u32, u32),
}

/// The persistent preview rig (camera + turntable parent). Created once at
/// startup and reused across every asset the overlay shows; only the spawned GLB
/// scene under the turntable is per-asset.
#[derive(Resource)]
struct ModelViewerRig {
    camera: Entity,
    turntable: Entity,
    grid: Entity,
}

/// Where the currently-shown model is in its download → load → frame lifecycle.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum ModelStatus {
    /// No model asset on show (or a non-model asset).
    #[default]
    Idle,
    /// Bytes downloading, or the GLB is loading / spawning / settling.
    Loading,
    /// The turntable is framed and rendering — show the RTT image.
    Ready,
    /// Download or load failed — the overlay falls back to the static gallery.
    Failed,
}

/// Per-asset preview session state. Reset to default whenever the overlay closes
/// or swaps to a different asset. Only one model preview exists at a time.
#[derive(Resource, Default)]
struct ModelPreview {
    /// True when the open asset's category is a 3D model / animation, so the
    /// header shows the turntable instead of the image gallery.
    is_model: bool,
    /// The asset id, used both for the cache filename and to dedup re-opens.
    asset_id: Option<String>,
    /// In-flight byte download (native only).
    rx: Option<Receiver<Result<Vec<u8>, String>>>,
    /// The GLB asset handle once bytes are cached and the load is kicked. Kept
    /// strong so the asset can't evict mid-preview.
    gltf: Option<Handle<Gltf>>,
    /// The spawned `WorldAssetRoot` child of the turntable, despawned on
    /// close/swap. `None` until the GLB loads.
    scene_root: Option<Entity>,
    status: ModelStatus,
    /// Set by the `WorldInstanceReady` observer once the subtree is stamped onto
    /// the preview layer — the cue to start polling AABB readiness.
    scene_ready: bool,
    /// Set once the camera is framed to the model; gates auto-rotate so we never
    /// spin (and mis-frame) before the bounds are known.
    framed: bool,
    /// Frames since the load/settle wait began, for the timeout.
    frames_waited: u32,
    /// Whether the offscreen camera should render (the overlay is open on a
    /// model). Toggled off on close so a hidden preview costs nothing.
    active: bool,
}

// ── Components ────────────────────────────────────────────────────────────────

#[derive(Component)]
struct ModelPreviewCamera;
#[derive(Component)]
struct ModelPreviewLight;
/// The parent we rotate; the GLB scene spawns as its child so it spins in place.
#[derive(Component)]
struct ModelViewerTurntable;
/// The studio ground grid. A sibling of the turntable (so it stays put while the
/// model spins); `frame_model` drops it to the model's feet and scales its line
/// spacing to the model so a tiny prop and a huge vehicle both read well.
#[derive(Component)]
struct ModelPreviewGrid;

// ── Public API (called from item_overlay) ─────────────────────────────────────

/// True for the categories that get a 3D turntable. Mirrors `native_store`'s
/// category matcher (`contains("model") || contains("3d")` for models, plus
/// animations) so the two views agree on what "a model" is.
pub(crate) fn is_model_category(category: &str) -> bool {
    let c = category.to_lowercase();
    c.contains("model") || c.contains("3d") || c.contains("anim")
}

/// The RTT handle for the overlay's `ImageNode` binding.
pub(crate) fn preview_image_handle(w: &World) -> Option<Handle<Image>> {
    w.get_resource::<ModelPreviewImage>().map(|p| p.handle.clone())
}

/// True when the turntable has rendered its first framed frame — show the RTT.
pub(crate) fn model_ready(w: &World) -> bool {
    w.get_resource::<ModelPreview>()
        .map(|p| p.is_model && p.status == ModelStatus::Ready)
        .unwrap_or(false)
}

/// True while a model asset's preview is still downloading / loading — show the
/// "Loading 3D preview…" placeholder.
pub(crate) fn model_loading(w: &World) -> bool {
    w.get_resource::<ModelPreview>()
        .map(|p| p.is_model && matches!(p.status, ModelStatus::Loading))
        .unwrap_or(false)
}

/// True when the overlay should show the static image gallery instead of the
/// turntable: a non-model asset, or a model whose preview failed to load.
pub(crate) fn show_gallery(w: &World) -> bool {
    match w.get_resource::<ModelPreview>() {
        Some(p) => !p.is_model || p.status == ModelStatus::Failed,
        None => true,
    }
}

/// Begin a preview for `asset`: reset any prior model, and — for a model/animation
/// category — kick the GLB download and activate the offscreen camera. For a
/// non-model asset this leaves the rig inert so the overlay shows its gallery.
pub(crate) fn open_model_preview(world: &mut World, asset: &AssetSummary) {
    despawn_scene(world);

    let is_model = is_model_category(&asset.category);
    let Some(mut preview) = world.get_resource_mut::<ModelPreview>() else {
        return;
    };
    *preview = ModelPreview {
        is_model,
        ..default()
    };
    if !is_model {
        return;
    }
    preview.asset_id = Some(asset.id.clone());

    // A model asset is usually uploaded as an FBX (which Bevy can't load) with a
    // GLB companion, so we CAN'T use the `preview-file` proxy — it serves only the
    // first file, often that FBX. Instead list the asset's files and pick the
    // `.glb`/`.gltf`. Its `download_url` is a presigned URL for FREE assets; for a
    // PAID asset it's `None`, so we error → status Failed → the overlay falls back
    // to the static thumbnail gallery. That's the desired paid behavior without a
    // separate code path.
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (tx, rx) = unbounded();
        preview.rx = Some(rx);
        preview.status = ModelStatus::Loading;
        preview.active = true;
        let id = asset.id.clone();
        std::thread::spawn(move || {
            let result = renzora_auth::marketplace::get_asset_files(&id)
                .and_then(|files| {
                    files
                        .into_iter()
                        .find(|f| {
                            let n = f.original_filename.to_ascii_lowercase();
                            n.ends_with(".glb") || n.ends_with(".gltf")
                        })
                        .ok_or_else(|| "asset has no glTF file to preview".to_string())
                })
                .and_then(|f| {
                    f.download_url
                        .ok_or_else(|| "no download URL (paid, not owned)".to_string())
                })
                .and_then(|url| renzora_auth::marketplace::download_file(&url));
            let _ = tx.send(result);
        });
    }
    // No blocking HTTP on wasm — fall straight back to the gallery.
    #[cfg(target_arch = "wasm32")]
    {
        preview.status = ModelStatus::Failed;
    }
}

/// Tear down the preview when the overlay closes: despawn the model, drop the
/// camera to idle, and reset the session state. The rig (camera/turntable/RTT)
/// persists for the next open.
pub(crate) fn close_model_preview(world: &mut World) {
    despawn_scene(world);
    if let Some(mut preview) = world.get_resource_mut::<ModelPreview>() {
        *preview = ModelPreview::default();
    }
}

/// Despawn the spawned GLB scene (if any) and clear the handle. Shared by open
/// (swap) and close so a model never leaks between asset views.
fn despawn_scene(world: &mut World) {
    let root = world
        .get_resource::<ModelPreview>()
        .and_then(|p| p.scene_root);
    if let Some(e) = root {
        if let Ok(entity) = world.get_entity_mut(e) {
            entity.despawn();
        }
    }
    if let Some(mut preview) = world.get_resource_mut::<ModelPreview>() {
        preview.scene_root = None;
    }
}

// ── Setup ─────────────────────────────────────────────────────────────────────

fn setup_model_viewer(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // RTT target — same descriptor as every other in-panel preview (Bgra8 srgb,
    // texture-binding + copy-dst + render-attachment).
    let size = Extent3d {
        width: RTT_W,
        height: RTT_H,
        depth_or_array_layers: 1,
    };
    let mut image = Image {
        data: Some(vec![0u8; (size.width * size.height * 4) as usize]),
        ..default()
    };
    image.texture_descriptor.size = size;
    image.texture_descriptor.format = TextureFormat::Bgra8UnormSrgb;
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    let handle = images.add(image);
    commands.insert_resource(ModelPreviewImage {
        handle: handle.clone(),
        size: (RTT_W, RTT_H),
    });

    // Off-screen camera — copied component-for-component from the material
    // preview rig (Hdr + the three prepasses so the PBR/prepass pipelines
    // specialize to one consistent format), starting inactive and on a unique
    // negative render order so it never contends with the main viewport.
    let camera = commands
        .spawn((
            Camera3d::default(),
            // Grouped to stay under the 15-element bundle-tuple limit; matches
            // the editor viewport camera's render config so the pbr + prepass
            // pipelines specialize to one consistent format.
            (Hdr, NormalPrepass, DepthPrepass, MotionVectorPrepass),
            Msaa::Off,
            Camera {
                // A cool studio charcoal rather than near-black, so the model
                // reads against a *surface* (with the grid) instead of floating
                // in a void.
                clear_color: ClearColorConfig::Custom(Color::srgb(0.12, 0.13, 0.16)),
                order: -8,
                is_active: false,
                ..default()
            },
            RenderTarget::Image(handle.into()),
            Transform::from_xyz(0.0, 0.6, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
            // A per-camera ambient lift so PBR materials without an environment
            // map don't read as crushed black on their shadow side (we skip IBL
            // for v1 — see module docs). Kept modest now that a 3-point rig does
            // most of the shaping.
            AmbientLight {
                color: Color::srgb(0.85, 0.88, 1.0),
                brightness: 350.0,
                affects_lightmapped_meshes: false,
            },
            RenderLayers::layer(MODEL_VIEWER_LAYER),
            ModelPreviewCamera,
            IsolatedCamera,
            HideInHierarchy,
            EditorLocked,
            Name::new("Marketplace Model Preview Camera"),
        ))
        .id();

    // Three-point rig on the preview layer for a studio look: a warm key from the
    // upper-front, a soft cool fill from the opposite low side to open the shadow
    // face, and a bright cool rim from behind-above to pop the silhouette off the
    // background. Shadows stay off — there's no ground receiver (the grid is a
    // shader plane, not a mesh), and a directional shadow map can't fit both a
    // tiny prop and a huge vehicle, so it would only add self-shadow acne.
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.97, 0.92),
            illuminance: 5500.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.7, 0.5, 0.0)),
        RenderLayers::layer(MODEL_VIEWER_LAYER),
        ModelPreviewLight,
        HideInHierarchy,
        EditorLocked,
        Name::new("Marketplace Model Preview Key Light"),
    ));
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.62, 0.72, 0.92),
            illuminance: 1600.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, 0.25, -0.9, 0.0)),
        RenderLayers::layer(MODEL_VIEWER_LAYER),
        ModelPreviewLight,
        HideInHierarchy,
        EditorLocked,
        Name::new("Marketplace Model Preview Fill Light"),
    ));
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.9, 0.94, 1.0),
            illuminance: 4200.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.35, 2.5, 0.0)),
        RenderLayers::layer(MODEL_VIEWER_LAYER),
        ModelPreviewLight,
        HideInHierarchy,
        EditorLocked,
        Name::new("Marketplace Model Preview Rim Light"),
    ));

    // Studio ground grid (Blender-style): a sibling of the turntable so it stays
    // level while the model spins. Position + line spacing are set per-model in
    // `frame_model`; these are just the resting colors. `InfiniteGrid` `#[require]`s
    // its Transform/Visibility/etc., so we only add the layer + markers.
    let grid = commands
        .spawn((
            InfiniteGrid,
            InfiniteGridSettings {
                x_axis_color: Color::srgb(0.75, 0.35, 0.38),
                z_axis_color: Color::srgb(0.35, 0.55, 0.85),
                minor_line_color: Color::srgba(0.55, 0.58, 0.64, 0.45),
                major_line_color: Color::srgba(0.72, 0.76, 0.82, 0.75),
                fadeout_distance: 50.0,
                dot_fadeout_strength: 0.25,
                scale: 1.0,
            },
            RenderLayers::layer(MODEL_VIEWER_LAYER),
            ModelPreviewGrid,
            HideInHierarchy,
            EditorLocked,
            Name::new("Marketplace Model Preview Grid"),
        ))
        .id();

    // The turntable parent — the GLB scene spawns as its child; we rotate this.
    let turntable = commands
        .spawn((
            Transform::default(),
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
            ViewVisibility::default(),
            RenderLayers::layer(MODEL_VIEWER_LAYER),
            ModelViewerTurntable,
            HideInHierarchy,
            EditorLocked,
            Name::new("Marketplace Model Turntable"),
        ))
        .id();

    commands.insert_resource(ModelViewerRig {
        camera,
        turntable,
        grid,
    });
    commands.init_resource::<ModelPreview>();
}

// ── Lifecycle systems ─────────────────────────────────────────────────────────

/// Toggle the offscreen camera with the preview being active. Only render while
/// the overlay is showing a model — a hidden preview must cost nothing.
fn sync_model_camera_active(
    preview: Res<ModelPreview>,
    mut cameras: Query<&mut Camera, With<ModelPreviewCamera>>,
) {
    let want = preview.active;
    for mut cam in cameras.iter_mut() {
        if cam.is_active != want {
            cam.is_active = want;
        }
    }
}

/// Drain the byte download: on success write the GLB to a temp cache file and
/// kick the `Gltf` load from that absolute path (see module docs); on failure
/// mark the preview failed so the overlay falls back to the gallery.
fn poll_model_download(mut preview: ResMut<ModelPreview>, asset_server: Res<AssetServer>) {
    let Some(rx) = preview.rx.take() else {
        return;
    };
    match rx.try_recv() {
        Ok(Ok(bytes)) => {
            let id = preview.asset_id.clone().unwrap_or_else(|| "preview".to_string());
            let path = cache_glb_path(&id);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::write(&path, &bytes) {
                Ok(()) => {
                    // Absolute path → the engine's asset reader serves it via a
                    // direct filesystem read; `From<PathBuf>` keeps it on the
                    // default source without drive-letter mis-parsing.
                    let handle: Handle<Gltf> = asset_server.load(path);
                    preview.gltf = Some(handle);
                    preview.status = ModelStatus::Loading;
                    preview.frames_waited = 0;
                }
                Err(e) => {
                    warn!("[model_viewer] couldn't cache preview GLB: {e}");
                    preview.status = ModelStatus::Failed;
                }
            }
        }
        Ok(Err(e)) => {
            // Paid-asset 401, a network error, etc. Fall back to the gallery.
            info!("[model_viewer] preview download failed: {e}");
            preview.status = ModelStatus::Failed;
        }
        Err(TryRecvError::Empty) => preview.rx = Some(rx),
        Err(TryRecvError::Disconnected) => preview.status = ModelStatus::Failed,
    }
}

/// Once the GLB asset resolves, spawn its default scene as a child of the
/// turntable on the preview layer, and reset the turntable to identity so the
/// framing math (which assumes an un-rotated parent) is valid.
fn poll_model_gltf(
    mut commands: Commands,
    mut preview: ResMut<ModelPreview>,
    rig: Option<Res<ModelViewerRig>>,
    gltf_assets: Option<Res<Assets<Gltf>>>,
    asset_server: Res<AssetServer>,
) {
    if preview.status != ModelStatus::Loading || preview.scene_root.is_some() {
        return;
    }
    let (Some(rig), Some(gltf_assets), Some(handle)) = (rig, gltf_assets, preview.gltf.clone())
    else {
        return;
    };

    preview.frames_waited += 1;
    if matches!(asset_server.get_load_state(&handle), Some(LoadState::Failed(_))) {
        warn!("[model_viewer] GLB failed to load");
        preview.status = ModelStatus::Failed;
        return;
    }
    let Some(gltf) = gltf_assets.get(&handle) else {
        if preview.frames_waited > LOAD_TIMEOUT_FRAMES {
            preview.status = ModelStatus::Failed;
        }
        return;
    };

    let Some(scene) = gltf
        .default_scene
        .clone()
        .or_else(|| gltf.scenes.first().cloned())
    else {
        warn!("[model_viewer] GLB has no scenes");
        preview.status = ModelStatus::Failed;
        return;
    };

    // Reset the turntable BEFORE the scene spawns so world-space AABB framing
    // sees an un-rotated parent (spin only starts once framed).
    commands.entity(rig.turntable).insert(Transform::default());

    let child = commands
        .spawn((
            WorldAssetRoot(scene),
            Transform::default(),
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
            ViewVisibility::default(),
            ChildOf(rig.turntable),
            RenderLayers::layer(MODEL_VIEWER_LAYER),
            HideInHierarchy,
            EditorLocked,
            Name::new("Marketplace Model Preview Root"),
        ))
        .id();
    preview.scene_root = Some(child);
    preview.frames_waited = 0;
}

/// Observer: the moment the GLB scene finishes spawning, confine every spawned
/// descendant to the preview layer (they'd otherwise default to layer 0 and
/// render in the main viewport) and flag the scene ready to frame.
fn on_model_scene_ready(
    trigger: On<WorldInstanceReady>,
    mut commands: Commands,
    mut preview: ResMut<ModelPreview>,
    children_q: Query<&Children>,
) {
    let root = trigger.event().entity;
    if root == Entity::PLACEHOLDER || preview.scene_root != Some(root) {
        return;
    }
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        commands.entity(e).try_insert((
            RenderLayers::layer(MODEL_VIEWER_LAYER),
            HideInHierarchy,
            EditorLocked,
        ));
        if let Ok(kids) = children_q.get(e) {
            stack.extend(kids.iter());
        }
    }
    preview.scene_ready = true;
}

/// Frame the camera once the model's AABBs have settled: center the model on the
/// turntable's axis and pull the camera back to fit its bounding sphere at
/// ~`FILL_FRACTION`. Rotation-invariant (sphere-based) so the spin never clips.
fn frame_model(
    mut preview: ResMut<ModelPreview>,
    rig: Option<Res<ModelViewerRig>>,
    mut transforms: Query<&mut Transform>,
    // Separate from `transforms` (no `Transform` access) so the two queries can't
    // conflict on the grid entity, which both would otherwise match.
    mut grid_settings: Query<&mut InfiniteGridSettings, With<ModelPreviewGrid>>,
    children_q: Query<&Children>,
    aabb_q: Query<(&Aabb, &GlobalTransform)>,
    mesh_q: Query<(), With<Mesh3d>>,
    has_aabb_q: Query<(), With<Aabb>>,
) {
    if preview.framed || !preview.active || preview.status != ModelStatus::Loading {
        return;
    }
    let (Some(rig), Some(root)) = (rig, preview.scene_root) else {
        return;
    };

    // Wait for the spawn observer, then for `compute_aabb_system` to catch up on
    // every spawned mesh. Both are bounded by the load timeout.
    if !preview.scene_ready || !subtree_aabbs_ready(root, &children_q, &mesh_q, &has_aabb_q) {
        preview.frames_waited += 1;
        if preview.frames_waited > LOAD_TIMEOUT_FRAMES {
            warn!("[model_viewer] model never settled — falling back to gallery");
            preview.status = ModelStatus::Failed;
        }
        return;
    }

    let Some((min_world, max_world)) = world_aabb(root, &children_q, &aabb_q) else {
        warn!("[model_viewer] spawned scene had no meshes");
        preview.status = ModelStatus::Failed;
        return;
    };
    let center = (min_world + max_world) * 0.5;
    // Bounding-sphere radius: rotation-invariant, so a spinning turntable stays
    // framed no matter the yaw.
    let radius = ((max_world - min_world) * 0.5).length().max(0.01);

    // Offset the scene root so the model centers on the turntable's rotation axis
    // (origin). With the parent at identity, local == world here.
    if let Ok(mut t) = transforms.get_mut(root) {
        t.translation = -center;
    }

    // Fixed 3/4 view direction; distance fits the sphere to FILL_FRACTION of the
    // vertical FOV (the tighter axis on a 16:9 target).
    let yaw = 0.6_f32;
    let pitch = -0.28_f32;
    let look_dir =
        (Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch) * Vec3::Z).normalize();
    let distance = (radius / (FILL_FRACTION * FOV_HALF_TAN)).max(0.5);
    if let Ok(mut cam) = transforms.get_mut(rig.camera) {
        *cam = Transform::from_translation(look_dir * distance).looking_at(Vec3::ZERO, Vec3::Y);
    }

    // Drop the grid to the model's *feet* (the centered AABB's minimum Y) so the
    // model sits on it rather than being bisected by a grid through its middle.
    let base_y = min_world.y - center.y;
    if let Ok(mut g) = transforms.get_mut(rig.grid) {
        g.translation = Vec3::new(0.0, base_y, 0.0);
    }
    // Scale line spacing and fade to the model so a coin and a spaceship both get
    // a readable floor: minor lines ~radius/4 apart (≈8 across the model), and a
    // fade a few model-widths past it.
    if let Ok(mut s) = grid_settings.single_mut() {
        s.scale = (4.0 / radius).clamp(0.02, 100.0);
        s.fadeout_distance = (distance * 2.5).max(10.0);
    }

    preview.framed = true;
    preview.status = ModelStatus::Ready;
}

/// Auto-rotate the turntable while a framed model is on show (yaw ≈ 0.5 rad/s).
fn spin_turntable(
    time: Res<Time>,
    preview: Res<ModelPreview>,
    mut turntables: Query<&mut Transform, With<ModelViewerTurntable>>,
) {
    if !preview.framed || !preview.active {
        return;
    }
    for mut t in turntables.iter_mut() {
        t.rotate_y(time.delta_secs() * 0.5);
    }
}

// ── AABB helpers (mirrors the asset browser's model-thumbnail framing) ─────────

/// True once every `Mesh3d` in the subtree also has an `Aabb`. `compute_aabb_system`
/// fills these incrementally each `PostUpdate`, so this flips false→true when
/// Bevy has caught up on the just-spawned scene. A mesh-less subtree returns true
/// (the caller then bails because `world_aabb` yields `None`).
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

/// Union every descendant `Aabb` (in world space) into a (min, max) pair, or
/// `None` if the subtree has no meshes.
fn world_aabb(
    root: Entity,
    children_q: &Query<&Children>,
    aabb_q: &Query<(&Aabb, &GlobalTransform)>,
) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut found = false;
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if let Ok((aabb, tf)) = aabb_q.get(e) {
            let center = Vec3::from(aabb.center);
            let half = Vec3::from(aabb.half_extents);
            for sx in [-1.0f32, 1.0] {
                for sy in [-1.0f32, 1.0] {
                    for sz in [-1.0f32, 1.0] {
                        let world = tf.transform_point(center + half * Vec3::new(sx, sy, sz));
                        min = min.min(world);
                        max = max.max(world);
                        found = true;
                    }
                }
            }
        }
        if let Ok(kids) = children_q.get(e) {
            stack.extend(kids.iter());
        }
    }
    found.then_some((min, max))
}

/// Absolute temp path for an asset's cached preview GLB. The id (a slug/uuid) is
/// sanitized so it can never escape the cache directory.
fn cache_glb_path(asset_id: &str) -> PathBuf {
    let safe: String = asset_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut dir = std::env::temp_dir();
    dir.push("renzora");
    dir.push("marketplace_preview");
    dir.push(format!("{safe}.glb"));
    dir
}

// ── Plugin ─────────────────────────────────────────────────────────────────────

pub(crate) struct ModelViewerPlugin;

impl Plugin for ModelViewerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, setup_model_viewer)
            .add_observer(on_model_scene_ready)
            .add_systems(
                Update,
                (
                    sync_model_camera_active,
                    poll_model_download,
                    poll_model_gltf,
                    frame_model,
                    spin_turntable,
                )
                    .chain()
                    .run_if(in_state(SplashState::Editor)),
            );
    }
}
