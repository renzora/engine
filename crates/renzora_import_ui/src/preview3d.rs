//! Offscreen 3D preview of a staged import.
//!
//! Renders the staged GLB to a texture that the inspector shows in an
//! `ImageNode`, so the user can look at the model before it enters the project.
//!
//! The staged tree is what makes this work at all: a GLB references its
//! textures by *relative* URI, and Bevy resolves those against the file's own
//! folder. Previewing from the converted bytes alone would show an untextured
//! model. `crate::staged` writes the GLB and its `textures/` together, so
//! loading the staged file gets the real thing.
//!
//! ## Keeping the preview out of the main viewport
//!
//! `SceneSpawner` gives new entities no `RenderLayers`, so they default to
//! layer 0 — the editor's viewport — and would render *there* as well as here.
//! Everything spawned is therefore walked and pinned to [`PREVIEW_LAYER`] the
//! moment the scene finishes spawning, and the camera is confined to the same
//! layer so it sees nothing else.

use std::path::PathBuf;

use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Hdr, RenderTarget};
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass, NormalPrepass};
use bevy::light::{CascadeShadowConfig, CascadeShadowConfigBuilder};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy::world_serialization::{WorldAssetRoot, WorldInstanceReady};

use renzora::core::{EditorLocked, HideInHierarchy, IsolatedCamera};

/// A render layer of this preview's own.
///
/// It started on 14 and collided with the marketplace material viewer, whose
/// sphere is permanently visible at the world origin — which is exactly where
/// this preview re-centres its model, so it appeared as a grey ball embedded in
/// every import. The hand-maintained list of taken layers that used to live in
/// this comment is now the registry in the contract crate; that is the only
/// place a new rig should read or write.
use renzora::core::viewport_types::IMPORT_PREVIEW_LAYER as PREVIEW_LAYER;

/// Initial render target size. The texture is resized every frame to match the
/// panel it is drawn into (see [`match_viewport_size`]) — a fixed size is both
/// blurry when the panel is bigger and *distorted* when the aspect differs,
/// since the camera takes its aspect from the target, not from the node.
const RTT_W: u32 = 1280;
const RTT_H: u32 = 800;

/// How much of the frame the model's bounding sphere should fill.
const FILL_FRACTION: f32 = 0.82;
/// tan(half vertical FOV) for Bevy's default 45° perspective.
const FOV_HALF_TAN: f32 = 0.4142;

/// Give up waiting for the GLB after this many frames (~15s at 60fps). A
/// staged Bistro is 120 MB, so the budget is generous on purpose.
const LOAD_TIMEOUT_FRAMES: u32 = 1800;

// Sensitivity matched to the editor viewport's defaults, so the two cameras
// feel identical: `look_sensitivity 0.3` and `orbit_sensitivity 0.5` both times
// the 0.01 per-pixel base, and `move_speed 10` against a `distance / 10`
// multiplier works out to one orbit-distance per second.
const LOOK_SPEED: f32 = 0.003;
const ORBIT_SPEED: f32 = 0.005;
const FLY_SPEED_PER_DISTANCE: f32 = 1.0;

#[derive(Resource)]
pub struct ImportPreviewImage {
    pub handle: Handle<Image>,
}

#[derive(Resource)]
struct ImportPreviewRig {
    camera: Entity,
    turntable: Entity,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum PreviewStatus {
    #[default]
    Idle,
    Loading,
    Ready,
    Failed,
}

#[derive(Resource, Default)]
pub struct ImportPreview {
    pub status: PreviewStatus,
    /// Path currently shown, so re-entering the same staged file is a no-op.
    pub path: Option<PathBuf>,
    gltf: Option<Handle<Gltf>>,
    scene_root: Option<Entity>,
    scene_ready: bool,
    framed: bool,
    frames_waited: u32,
}

#[derive(Component)]
struct PreviewCamera;
#[derive(Component)]
struct PreviewPivot;
/// The one shadow-casting light, whose cascades follow the model's size.
#[derive(Component)]
struct PreviewKeyLight;

/// Marker for the UI node the preview texture is drawn into. Camera input is
/// gated on this node's `Interaction` rather than a geometric cursor test, so
/// dragging something that overlaps the preview does not grab the camera.
#[derive(Component)]
pub struct ImportPreviewViewport;

/// Where the camera is looking from. Written by the input system and by the
/// framing pass, applied to the camera transform each frame.
#[derive(Resource)]
pub struct ImportPreviewOrbit {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: Vec3,
    /// The distance framing chose, so zoom can be clamped relative to the
    /// model rather than to arbitrary world units — a 200 m street and a 20 cm
    /// prop need very different limits.
    framed_distance: f32,
    /// Where the camera is actually sitting. Input writes the fields above and
    /// this eases toward them, which is what turns a wheel notch from a jump
    /// into a movement — a scroll wheel arrives as discrete steps, so applying
    /// it directly can only ever look stepped.
    smooth_yaw: f32,
    smooth_pitch: f32,
    smooth_distance: f32,
    smooth_target: Vec3,
    /// Set when framing jumps the camera, so it snaps rather than sliding in
    /// from wherever the previous model left it.
    snap: bool,
}

impl Default for ImportPreviewOrbit {
    fn default() -> Self {
        Self {
            yaw: 0.6,
            pitch: 0.35,
            distance: 3.0,
            target: Vec3::ZERO,
            framed_distance: 3.0,
            smooth_yaw: 0.6,
            smooth_pitch: 0.35,
            smooth_distance: 3.0,
            smooth_target: Vec3::ZERO,
            snap: true,
        }
    }
}

/// Mouse drag modes, matching the editor viewport camera so the two do not
/// need separate muscle memory: right-drag looks around (and flies with
/// WASD/QE), Shift+right pans, middle or Alt+left orbits.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Drag {
    /// Rotate about the focus point.
    Orbit,
    /// Rotate in place — the camera stays put and the focus is recomputed.
    Look,
    Pan,
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<ImportPreview>()
        .init_resource::<ImportPreviewOrbit>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                sync_camera_active,
                match_viewport_size,
                poll_gltf,
                isolate_selection,
                frame_model,
                preview_input,
                apply_orbit,
            )
                .chain(),
        )
        .add_observer(on_scene_ready);
}

/// Read the preview texture for the UI binding.
pub fn preview_image(world: &World) -> Option<Handle<Image>> {
    world
        .get_resource::<ImportPreviewImage>()
        .map(|i| i.handle.clone())
}

/// Show `path` in the preview. Cheap to call every frame — it returns early
/// once the path is already loaded.
pub fn show(world: &mut World, path: &std::path::Path) {
    {
        let preview = world.resource::<ImportPreview>();
        if preview.path.as_deref() == Some(path) {
            return;
        }
    }
    despawn_scene(world);

    let handle = world.resource::<AssetServer>().load::<Gltf>(path.to_path_buf());
    let mut preview = world.resource_mut::<ImportPreview>();
    preview.path = Some(path.to_path_buf());
    preview.gltf = Some(handle);
    preview.status = PreviewStatus::Loading;
    preview.frames_waited = 0;
    preview.scene_ready = false;
    preview.framed = false;
}

/// Tear the preview down — called when the inspector closes.
pub fn clear(world: &mut World) {
    if world.resource::<ImportPreview>().path.is_none() {
        return;
    }
    despawn_scene(world);
    let mut preview = world.resource_mut::<ImportPreview>();
    preview.path = None;
    preview.gltf = None;
    preview.status = PreviewStatus::Idle;
}

fn despawn_scene(world: &mut World) {
    let root = world.resource_mut::<ImportPreview>().scene_root.take();
    if let Some(root) = root {
        if let Ok(entity) = world.get_entity_mut(root) {
            entity.despawn();
        }
    }
    let mut preview = world.resource_mut::<ImportPreview>();
    preview.scene_ready = false;
    preview.framed = false;
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
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
    commands.insert_resource(ImportPreviewImage {
        handle: handle.clone(),
    });

    let camera = commands
        .spawn((
            Camera3d::default(),
            // Grouped to stay under the bundle-tuple limit. Matches the editor
            // viewport's render config so the pbr and prepass pipelines
            // specialize to one consistent format rather than two.
            (Hdr, NormalPrepass, DepthPrepass, MotionVectorPrepass),
            Msaa::Off,
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgb(0.11, 0.12, 0.145)),
                order: -9,
                is_active: false,
                ..default()
            },
            RenderTarget::Image(handle.into()),
            Transform::from_xyz(0.0, 0.7, 3.2).looking_at(Vec3::ZERO, Vec3::Y),
            // Imported models often arrive with no environment map. Without a
            // per-camera ambient lift their shadow side reads as crushed
            // black, which looks like a broken import rather than a lighting
            // choice.
            AmbientLight {
                color: Color::srgb(0.85, 0.88, 1.0),
                // Enough to keep the shadow side off pure black, but low
                // enough that the key's shadows actually read. The old 350
                // was set when nothing cast.
                brightness: 160.0,
                affects_lightmapped_meshes: false,
            },
            // TAA needs the motion-vector prepass above and `Msaa::Off`, both
            // already set. It matters more here than in the viewport: the
            // camera is usually still, so without it the specular on imported
            // metal and the foliage edges crawl.
            TemporalAntiAliasing::default(),
            RenderLayers::layer(PREVIEW_LAYER),
            PreviewCamera,
            IsolatedCamera,
            HideInHierarchy,
            EditorLocked,
            Name::new("Import Preview Camera"),
        ))
        .id();

    // Three-point rig: warm key from upper front, cool fill opposite and low,
    // rim from behind to separate the silhouette from the background.
    //
    // Only the key casts, and its cascades are resized to the model when it is
    // framed. Bevy's default cascade set reaches ~100 units, and an imported
    // street is comfortably larger than that — which is why shadows appeared to
    // be off entirely rather than merely coarse.
    for (transform, color, illuminance, shadows) in [
        (
            Transform::from_xyz(4.0, 6.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
            Color::srgb(1.0, 0.96, 0.9),
            9000.0,
            true,
        ),
        (
            Transform::from_xyz(-5.0, 1.5, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
            Color::srgb(0.8, 0.87, 1.0),
            3000.0,
            false,
        ),
        (
            Transform::from_xyz(0.0, 3.0, -6.0).looking_at(Vec3::ZERO, Vec3::Y),
            Color::srgb(0.9, 0.93, 1.0),
            4000.0,
            false,
        ),
    ] {
        commands.spawn((
            DirectionalLight {
                color,
                illuminance,
                // Only the key casts. Three shadow-casting directionals over a
                // scene-sized import is three full shadow passes for detail
                // nobody reads in a preview.
                shadow_maps_enabled: shadows,
                ..default()
            },
            transform,
            CascadeShadowConfigBuilder {
                num_cascades: 4,
                minimum_distance: 0.1,
                maximum_distance: 400.0,
                first_cascade_far_bound: 12.0,
                overlap_proportion: 0.2,
            }
            .build(),
            RenderLayers::layer(PREVIEW_LAYER),
            PreviewKeyLight,
            HideInHierarchy,
            EditorLocked,
            Name::new("Import Preview Light"),
        ));
    }

    let turntable = commands
        .spawn((
            Transform::default(),
            Visibility::Visible,
            RenderLayers::layer(PREVIEW_LAYER),
            PreviewPivot,
            HideInHierarchy,
            EditorLocked,
            Name::new("Import Preview Turntable"),
        ))
        .id();

    commands.insert_resource(ImportPreviewRig { camera, turntable });
}

/// Only render while something is actually being previewed — an always-on
/// offscreen camera costs a full pass every frame for nothing.
fn sync_camera_active(
    preview: Res<ImportPreview>,
    mut cameras: Query<&mut Camera, With<PreviewCamera>>,
) {
    let want = preview.path.is_some();
    for mut cam in &mut cameras {
        if cam.is_active != want {
            cam.is_active = want;
        }
    }
}

/// Keep the render texture the same pixel size as the panel showing it, so the
/// image is never up- or down-scaled and its aspect always matches.
fn match_viewport_size(
    q: Query<&bevy::ui::ComputedNode, With<ImportPreviewViewport>>,
    image: Option<Res<ImportPreviewImage>>,
    mut images: ResMut<Assets<Image>>,
    mut current: Local<UVec2>,
) {
    let Some(image) = image else { return };
    let Some(cn) = q.iter().next() else { return };
    let size = cn.size();
    // Physical pixels, which is what a render target wants. Clamped so a panel
    // collapsed to nothing cannot ask for a zero-sized texture.
    let want = UVec2::new(
        (size.x as u32).clamp(64, 7680),
        (size.y as u32).clamp(64, 4320),
    );
    if *current == want {
        return;
    }
    if let Some(mut img) = images.get_mut(&image.handle) {
        img.resize(Extent3d {
            width: want.x,
            height: want.y,
            depth_or_array_layers: 1,
        });
        *current = want;
    }
}

fn poll_gltf(
    mut commands: Commands,
    mut preview: ResMut<ImportPreview>,
    rig: Option<Res<ImportPreviewRig>>,
    gltf_assets: Option<Res<Assets<Gltf>>>,
    asset_server: Res<AssetServer>,
) {
    if preview.status != PreviewStatus::Loading || preview.scene_root.is_some() {
        return;
    }
    let (Some(rig), Some(gltf_assets), Some(handle)) = (rig, gltf_assets, preview.gltf.clone())
    else {
        return;
    };

    preview.frames_waited += 1;
    if matches!(
        asset_server.get_load_state(&handle),
        Some(bevy::asset::LoadState::Failed(_))
    ) {
        warn!("[import preview] staged GLB failed to load");
        preview.status = PreviewStatus::Failed;
        return;
    }
    let Some(gltf) = gltf_assets.get(&handle) else {
        if preview.frames_waited > LOAD_TIMEOUT_FRAMES {
            preview.status = PreviewStatus::Failed;
        }
        return;
    };
    let Some(scene) = gltf
        .default_scene
        .clone()
        .or_else(|| gltf.scenes.first().cloned())
    else {
        preview.status = PreviewStatus::Failed;
        return;
    };

    // Reset the turntable before spawning so the framing pass below measures an
    // un-rotated model; the spin only starts once framing is done.
    commands.entity(rig.turntable).insert(Transform::default());

    let child = commands
        .spawn((
            WorldAssetRoot(scene),
            Transform::default(),
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
            ViewVisibility::default(),
            ChildOf(rig.turntable),
            RenderLayers::layer(PREVIEW_LAYER),
            HideInHierarchy,
            EditorLocked,
            Name::new("Import Preview Root"),
        ))
        .id();
    preview.scene_root = Some(child);
    preview.frames_waited = 0;
}

/// Confine every spawned descendant to the preview layer the moment the scene
/// lands — without this they default to layer 0 and appear in the viewport.
fn on_scene_ready(
    trigger: On<WorldInstanceReady>,
    mut commands: Commands,
    mut preview: ResMut<ImportPreview>,
    children_q: Query<&Children>,
) {
    let root = trigger.event().entity;
    if root == Entity::PLACEHOLDER || preview.scene_root != Some(root) {
        return;
    }
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        commands
            .entity(e)
            .try_insert((RenderLayers::layer(PREVIEW_LAYER), HideInHierarchy, EditorLocked));
        if let Ok(kids) = children_q.get(e) {
            stack.extend(kids.iter());
        }
    }
    preview.scene_ready = true;
}

/// Centre the model on the turntable axis and pull the camera back to fit its
/// bounding sphere. Sphere-based rather than box-based so the spin can never
/// clip a corner into the near plane.
fn frame_model(
    mut preview: ResMut<ImportPreview>,
    mut orbit: ResMut<ImportPreviewOrbit>,
    rig: Option<Res<ImportPreviewRig>>,
    mut cascades: Query<&mut CascadeShadowConfig, With<PreviewKeyLight>>,
    mut transforms: Query<&mut Transform>,
    children_q: Query<&Children>,
    aabb_q: Query<(&Aabb, &GlobalTransform)>,
    mesh_q: Query<(), With<Mesh3d>>,
) {
    if !preview.scene_ready || preview.framed {
        return;
    }
    let (Some(_rig), Some(root)) = (rig, preview.scene_root) else {
        return;
    };

    // Wait until every mesh has an Aabb, otherwise the first frame measures a
    // partially-populated tree and the model ends up mis-scaled.
    let mut stack = vec![root];
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    let mut found = false;
    while let Some(e) = stack.pop() {
        if mesh_q.get(e).is_ok() {
            let Ok((aabb, gt)) = aabb_q.get(e) else {
                // A mesh without an AABB yet — try again next frame.
                preview.frames_waited += 1;
                if preview.frames_waited > LOAD_TIMEOUT_FRAMES {
                    preview.framed = true;
                    preview.status = PreviewStatus::Ready;
                }
                return;
            };
            let centre = gt.transform_point(Vec3::from(aabb.center));
            let he = Vec3::from(aabb.half_extents);
            // Conservative: scale the half-extent by the transform's largest
            // axis rather than rotating the box, which is enough for framing.
            let scale = gt.scale().abs().max_element();
            let r = he.length() * scale;
            min = min.min(centre - Vec3::splat(r));
            max = max.max(centre + Vec3::splat(r));
            found = true;
        }
        if let Ok(kids) = children_q.get(e) {
            stack.extend(kids.iter());
        }
    }
    if !found {
        preview.framed = true;
        preview.status = PreviewStatus::Ready;
        return;
    }

    let centre = (min + max) * 0.5;
    let radius = ((max - min).length() * 0.5).max(1e-4);

    // Re-centre the model under the pivot so it orbits about its own middle.
    if let Ok(mut t) = transforms.get_mut(root) {
        t.translation = -centre;
    }
    // Pull back far enough that the bounding sphere fills FILL_FRACTION of the
    // frame's vertical extent.
    let distance = radius / (FOV_HALF_TAN * FILL_FRACTION);
    orbit.yaw = 0.6;
    orbit.pitch = 0.35;
    orbit.distance = distance;
    orbit.framed_distance = distance;
    orbit.target = Vec3::ZERO;
    orbit.snap = true;

    // Cascades cover the model plus headroom. Sized from the bounding sphere so
    // a 20 cm prop gets tight, detailed cascades and a 200 m street still gets
    // shadows at all.
    for mut cascade in &mut cascades {
        *cascade = CascadeShadowConfigBuilder {
            num_cascades: 4,
            minimum_distance: (radius * 0.01).max(0.05),
            maximum_distance: (radius * 8.0).max(20.0),
            first_cascade_far_bound: (radius * 0.5).max(2.0),
            overlap_proportion: 0.2,
        }
        .build();
    }

    preview.framed = true;
    preview.status = PreviewStatus::Ready;
}

/// Show only what is selected in the Scene tab, and frame the camera on it.
///
/// The spawned scene has no index back to the glTF node list, but Bevy's glTF
/// loader copies each node's name onto its entity — so the selection resolves
/// by name. Names are not unique in glTF; the first match wins, which is what
/// every DCC outliner does.
///
/// Selecting *isolates*: everything outside the chosen subtree is hidden, so a
/// single lamp post in a street scene is actually visible rather than being a
/// speck the camera has flown to. Selecting nothing restores the whole model.
fn isolate_selection(
    preview: Res<ImportPreview>,
    state: Option<Res<crate::overlay::ImportOverlayState>>,
    nav: Option<Res<crate::native::ImportNav>>,
    mut orbit: ResMut<ImportPreviewOrbit>,
    names: Query<&Name>,
    children_q: Query<&Children>,
    aabb_q: Query<(&Aabb, &GlobalTransform)>,
    mesh_q: Query<(), With<Mesh3d>>,
    mut visibility: Query<&mut Visibility>,
    mut last: Local<Option<Option<crate::native::TreeItem>>>,
) {
    if preview.status != PreviewStatus::Ready {
        return;
    }
    let (Some(state), Some(nav)) = (state, nav) else {
        return;
    };
    let selection = nav.sel_item;
    if *last == Some(selection) {
        return;
    }
    *last = Some(selection);
    let Some(root) = preview.scene_root else { return };

    // A *surface* is one primitive of a mesh, and glTF primitives carry no
    // name — but Bevy spawns one entity per primitive, in order, so the k-th
    // renderable under the scene root is surface k. That indexing is what makes
    // the surface list clickable at all; for a transcoded model, where the
    // whole import is one node and one mesh, it is the only level with more
    // than one thing in it.
    if let Some(crate::native::TreeItem::Prim(_, k)) = selection {
        let mut renderables = Vec::new();
        collect_renderables(root, &children_q, &mesh_q, &mut renderables);
        if let Some(&target) = renderables.get(k) {
            set_subtree_visibility(root, false, &children_q, &mut visibility);
            if let Ok(mut v) = visibility.get_mut(root) {
                *v = Visibility::Visible;
            }
            for e in ancestors_of(root, target, &children_q) {
                if let Ok(mut v) = visibility.get_mut(e) {
                    *v = Visibility::Visible;
                }
            }
            set_subtree_visibility(target, true, &children_q, &mut visibility);
            if let Some((centre, radius)) = subtree_bounds(target, &children_q, &aabb_q) {
                aim(&mut orbit, centre, radius);
            }
            return;
        }
    }

    // Nodes and meshes do have names, so those resolve by lookup.
    let wanted = selection.and_then(|item| {
        let stats = state.current().and_then(|s| s.stats.as_ref())?;
        match item {
            crate::native::TreeItem::Node(i) => stats.node_list.get(i).map(|n| n.name.clone()),
            crate::native::TreeItem::Mesh(m) | crate::native::TreeItem::Prim(m, _) => {
                stats.mesh_list.get(m).map(|m| m.name.clone())
            }
        }
    });

    // Nothing selected — everything visible again, and re-frame the model.
    let Some(wanted) = wanted else {
        set_subtree_visibility(root, true, &children_q, &mut visibility);
        if let Some((centre, radius)) = subtree_bounds(root, &children_q, &aabb_q) {
            aim(&mut orbit, centre, radius);
        }
        return;
    };

    let mut stack = vec![root];
    let mut found = None;
    while let Some(e) = stack.pop() {
        if names.get(e).is_ok_and(|n| n.as_str() == wanted) {
            found = Some(e);
            break;
        }
        if let Ok(kids) = children_q.get(e) {
            stack.extend(kids.iter());
        }
    }
    let Some(target) = found else {
        // A node with no entity of its own (an empty transform, or a name the
        // loader did not carry through). Leave the view alone rather than
        // blanking it.
        set_subtree_visibility(root, true, &children_q, &mut visibility);
        return;
    };

    // Hide everything, then re-show the chosen subtree. Hiding the root and
    // showing a descendant would not work — `Visibility::Inherited` means a
    // hidden ancestor wins — so the walk sets `Visible` explicitly on the
    // subtree and `Hidden` on everything else.
    set_subtree_visibility(root, false, &children_q, &mut visibility);
    if let Ok(mut v) = visibility.get_mut(root) {
        *v = Visibility::Visible;
    }
    set_subtree_visibility(target, true, &children_q, &mut visibility);
    // The chain from the root down to the target has to stay visible too, or
    // an ancestor's `Hidden` masks the subtree we just re-enabled.
    let mut chain = vec![root];
    while let Some(e) = chain.pop() {
        if e == target {
            break;
        }
        if let Ok(kids) = children_q.get(e) {
            for k in kids.iter() {
                if contains(k, target, &children_q) {
                    if let Ok(mut v) = visibility.get_mut(k) {
                        *v = Visibility::Visible;
                    }
                    chain.push(k);
                }
            }
        }
    }

    if let Some((centre, radius)) = subtree_bounds(target, &children_q, &aabb_q) {
        aim(&mut orbit, centre, radius);
    }
}

/// Every entity that actually draws something, in depth-first child order —
/// which is the order Bevy spawns glTF primitives in.
fn collect_renderables(
    root: Entity,
    children_q: &Query<&Children>,
    mesh_q: &Query<(), With<Mesh3d>>,
    out: &mut Vec<Entity>,
) {
    if mesh_q.get(root).is_ok() {
        out.push(root);
    }
    if let Ok(kids) = children_q.get(root) {
        for k in kids.iter() {
            collect_renderables(k, children_q, mesh_q, out);
        }
    }
}

/// The chain of entities from `root` down to `target`, exclusive of `target`.
/// Those have to stay visible or an ancestor's `Hidden` masks the subtree.
fn ancestors_of(root: Entity, target: Entity, children_q: &Query<&Children>) -> Vec<Entity> {
    fn walk(
        e: Entity,
        target: Entity,
        children_q: &Query<&Children>,
        path: &mut Vec<Entity>,
    ) -> bool {
        if e == target {
            return true;
        }
        path.push(e);
        if let Ok(kids) = children_q.get(e) {
            for k in kids.iter() {
                if walk(k, target, children_q, path) {
                    return true;
                }
            }
        }
        path.pop();
        false
    }
    let mut path = Vec::new();
    walk(root, target, children_q, &mut path);
    path
}

/// Point the camera at a sphere, easing rather than snapping — seeing the
/// camera travel is what tells you where in the model the thing you clicked is.
fn aim(orbit: &mut ImportPreviewOrbit, centre: Vec3, radius: f32) {
    orbit.target = centre;
    orbit.distance = (radius / (FOV_HALF_TAN * FILL_FRACTION)).max(radius * 1.2);
}

/// Is `needle` at or below `root`?
fn contains(root: Entity, needle: Entity, children_q: &Query<&Children>) -> bool {
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if e == needle {
            return true;
        }
        if let Ok(kids) = children_q.get(e) {
            stack.extend(kids.iter());
        }
    }
    false
}

fn set_subtree_visibility(
    root: Entity,
    visible: bool,
    children_q: &Query<&Children>,
    visibility: &mut Query<&mut Visibility>,
) {
    let want = if visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if let Ok(mut v) = visibility.get_mut(e) {
            if *v != want {
                *v = want;
            }
        }
        if let Ok(kids) = children_q.get(e) {
            stack.extend(kids.iter());
        }
    }
}

/// World-space bounding sphere of `root` and everything under it.
fn subtree_bounds(
    root: Entity,
    children_q: &Query<&Children>,
    aabb_q: &Query<(&Aabb, &GlobalTransform)>,
) -> Option<(Vec3, f32)> {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    let mut any = false;
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if let Ok((aabb, gt)) = aabb_q.get(e) {
            let centre = gt.transform_point(Vec3::from(aabb.center));
            let r = Vec3::from(aabb.half_extents).length() * gt.scale().abs().max_element();
            min = min.min(centre - Vec3::splat(r));
            max = max.max(centre + Vec3::splat(r));
            any = true;
        }
        if let Ok(kids) = children_q.get(e) {
            stack.extend(kids.iter());
        }
    }
    any.then(|| ((min + max) * 0.5, ((max - min).length() * 0.5).max(1e-3)))
}

/// Camera input, bound the same way as the editor viewport so the two share
/// muscle memory: right-drag looks around with WASD/QE flying, Shift+right
/// pans, middle-drag or Alt+left orbits, and the wheel dollies.
///
/// Only active while the cursor is over the preview, or a drag started there.
#[allow(clippy::too_many_arguments)]
fn preview_input(
    hover: Query<&Interaction, With<ImportPreviewViewport>>,
    preview: Res<ImportPreview>,
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    mut orbit: ResMut<ImportPreviewOrbit>,
    mut drag: Local<Option<Drag>>,
    mut velocity: Local<Vec3>,
) {
    if preview.status != PreviewStatus::Ready {
        *drag = None;
        *velocity = Vec3::ZERO;
        return;
    }
    let over = hover
        .iter()
        .any(|i| matches!(i, Interaction::Hovered | Interaction::Pressed));
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let alt = keyboard.pressed(KeyCode::AltLeft) || keyboard.pressed(KeyCode::AltRight);

    if drag.is_none() && over {
        if mouse.just_pressed(MouseButton::Right) {
            *drag = Some(if shift { Drag::Pan } else { Drag::Look });
        } else if mouse.just_pressed(MouseButton::Middle)
            || (mouse.just_pressed(MouseButton::Left) && alt)
        {
            *drag = Some(Drag::Orbit);
        } else if mouse.just_pressed(MouseButton::Left) {
            // Plain left-drag orbits too. The editor gives left-drag to
            // selection, but there is nothing to select in here.
            *drag = Some(Drag::Orbit);
        }
    }
    if !mouse.pressed(MouseButton::Left)
        && !mouse.pressed(MouseButton::Right)
        && !mouse.pressed(MouseButton::Middle)
    {
        *drag = None;
    }
    // Shift can be pressed or released mid-drag; follow it, like the editor.
    if *drag == Some(Drag::Look) && shift {
        *drag = Some(Drag::Pan);
    } else if *drag == Some(Drag::Pan) && mouse.pressed(MouseButton::Right) && !shift {
        *drag = Some(Drag::Look);
    }

    let (sy, cy) = orbit.yaw.sin_cos();
    let (sp, cp) = orbit.pitch.sin_cos();
    let view_dir = Vec3::new(cp * sy, sp, cp * cy);
    let right_dir = Vec3::new(cy, 0.0, -sy);

    // ── WASD / QE fly while right-dragging ──────────────────────────────
    let flying = matches!(*drag, Some(Drag::Look) | Some(Drag::Pan))
        && mouse.pressed(MouseButton::Right);
    let mut target_velocity = Vec3::ZERO;
    if flying {
        let mut d = Vec3::ZERO;
        if keyboard.pressed(KeyCode::KeyW) {
            d -= view_dir;
        }
        if keyboard.pressed(KeyCode::KeyS) {
            d += view_dir;
        }
        if keyboard.pressed(KeyCode::KeyA) {
            d -= right_dir;
        }
        if keyboard.pressed(KeyCode::KeyD) {
            d += right_dir;
        }
        if keyboard.pressed(KeyCode::KeyE) {
            d += Vec3::Y;
        }
        if keyboard.pressed(KeyCode::KeyQ) {
            d -= Vec3::Y;
        }
        if d.length_squared() > 0.0 {
            // Speed scales with how far out you are, so flying feels the same
            // across a prop and a street.
            let speed = orbit.distance.max(0.5) * FLY_SPEED_PER_DISTANCE;
            let boost = if keyboard.pressed(KeyCode::ControlLeft) { 0.25 } else { 1.0 };
            target_velocity = d.normalize() * speed * boost;
        }
    }
    let ease = 1.0 - (-14.0 * time.delta_secs()).exp();
    *velocity = velocity.lerp(target_velocity, ease);
    if velocity.length_squared() > 1e-8 {
        let step = *velocity * time.delta_secs();
        orbit.target += step;
        orbit.smooth_target += step;
    } else {
        *velocity = Vec3::ZERO;
    }

    // ── Mouse ───────────────────────────────────────────────────────────
    if let Some(mode) = *drag {
        let d = motion.delta;
        if d != Vec2::ZERO {
            match mode {
                Drag::Orbit => {
                    orbit.yaw -= d.x * ORBIT_SPEED;
                    // Stop just short of the poles: looking straight down the
                    // up-axis makes `looking_at` degenerate and the view rolls.
                    orbit.pitch = (orbit.pitch + d.y * ORBIT_SPEED).clamp(-1.45, 1.45);
                }
                Drag::Look => {
                    // Rotate in place: hold the eye still and move the focus,
                    // which is what the editor's right-drag does.
                    let eye = orbit.target + view_dir * orbit.distance;
                    orbit.yaw -= d.x * LOOK_SPEED;
                    orbit.pitch = (orbit.pitch + d.y * LOOK_SPEED).clamp(-1.45, 1.45);
                    let (ny_s, ny_c) = orbit.yaw.sin_cos();
                    let (np_s, np_c) = orbit.pitch.sin_cos();
                    let new_dir = Vec3::new(np_c * ny_s, np_s, np_c * ny_c);
                    orbit.target = eye - new_dir * orbit.distance;
                    // Looking must not lag, or the eye visibly drifts while the
                    // eased target catches up.
                    orbit.snap = true;
                }
                Drag::Pan => {
                    let up_dir = right_dir.cross(view_dir).normalize();
                    let scale = orbit.distance.max(0.5) * 0.001;
                    orbit.target += right_dir * (-d.x * scale) + up_dir * (d.y * scale);
                }
            }
        }
    }

    if over && scroll.delta.y != 0.0 {
        let framed = orbit.framed_distance.max(1e-3);
        // Multiplicative so each notch is the same *proportional* step, which
        // is what makes zoom feel even whether you are close in or far out.
        let factor = 0.88f32.powf(scroll.delta.y);
        orbit.distance = (orbit.distance * factor).clamp(framed * 0.02, framed * 8.0);
    }
}

/// Apply the orbit to the camera. Kept separate from input so framing, the
/// scroll wheel and a drag all write one state and only this decides where the
/// camera ends up.
fn apply_orbit(
    time: Res<Time>,
    mut orbit: ResMut<ImportPreviewOrbit>,
    rig: Option<Res<ImportPreviewRig>>,
    mut transforms: Query<&mut Transform>,
) {
    let Some(rig) = rig else { return };

    if orbit.snap {
        orbit.smooth_yaw = orbit.yaw;
        orbit.smooth_pitch = orbit.pitch;
        orbit.smooth_distance = orbit.distance;
        orbit.smooth_target = orbit.target;
        orbit.snap = false;
    } else {
        // Frame-rate independent exponential ease: the fraction of the
        // remaining gap closed per second is constant, so the motion feels the
        // same at 60 and at 144.
        let k = 1.0 - (-18.0 * time.delta_secs()).exp();
        orbit.smooth_yaw += (orbit.yaw - orbit.smooth_yaw) * k;
        orbit.smooth_pitch += (orbit.pitch - orbit.smooth_pitch) * k;
        // Distance eases geometrically — halving twice should look like two
        // equal steps, which a linear ease does not give you.
        let ratio = (orbit.distance / orbit.smooth_distance.max(1e-6)).ln();
        orbit.smooth_distance *= (ratio * k).exp();
        let d = orbit.target - orbit.smooth_target;
        orbit.smooth_target += d * k;
    }

    let Ok(mut t) = transforms.get_mut(rig.camera) else {
        return;
    };
    let (sy, cy) = orbit.smooth_yaw.sin_cos();
    let (sp, cp) = orbit.smooth_pitch.sin_cos();
    let dist = orbit.smooth_distance;
    let eye = orbit.smooth_target + Vec3::new(dist * cp * sy, dist * sp, dist * cp * cy);
    t.translation = eye;
    *t = t.looking_at(orbit.smooth_target, Vec3::Y);
}
