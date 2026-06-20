//! Studio Preview — isolated 3D viewport for animation preview.
//!
//! Creates an offscreen render target with its own camera, light, and a cloned
//! copy of the selected entity's model. The animation system drives playback
//! on the real entity while this panel mirrors it visually.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::camera::Hdr;
use bevy::core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass, NormalPrepass};
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use renzora::core::{EditorLocked, HideInHierarchy, IsolatedCamera, MeshInstanceData};
use renzora_editor_framework::DockingState;

use crate::AnimationEditorState;

pub const STUDIO_PREVIEW_LAYER: usize = 10;

/// Run condition: `true` when the Studio Preview panel is in the active dock
/// tree. All expensive studio-preview work (camera rendering, model cloning,
/// skeleton drawing, etc.) is gated on this so other layouts don't pay for it.
pub fn studio_preview_panel_mounted(docking: Option<Res<DockingState>>) -> bool {
    docking.is_some_and(|d| d.tree.contains_panel("studio_preview"))
}

/// Toggles the studio-preview camera on/off with panel visibility and tears
/// down the cloned model + tracker when the panel closes. Runs every frame
/// regardless of panel state so close transitions are always caught.
pub fn sync_studio_preview_activation(
    docking: Option<Res<DockingState>>,
    mut camera_q: Query<&mut Camera, With<StudioPreviewCamera>>,
    preview_q: Query<Entity, With<StudioPreviewModel>>,
    mut tracker: ResMut<StudioPreviewTracker>,
    mut commands: Commands,
) {
    let mounted = docking.is_some_and(|d| d.tree.contains_panel("studio_preview"));
    for mut camera in camera_q.iter_mut() {
        if camera.is_active != mounted {
            camera.is_active = mounted;
        }
    }
    if !mounted {
        for entity in preview_q.iter() {
            commands.entity(entity).despawn();
        }
        // Reset so the next remount re-clones even if selection hasn't changed.
        if tracker.source_entity.is_some() {
            tracker.source_entity = None;
            tracker.auto_fitted = false;
            tracker.has_model = false;
        }
    }
}

/// Toggle settings for the studio preview viewport.
#[derive(Resource)]
pub struct StudioPreviewSettings {
    /// Show skeleton bone gizmos.
    pub show_skeleton: bool,
    /// Show the checkerboard floor.
    pub show_floor: bool,
    /// Show the wireframe overlay.
    pub show_wireframe: bool,
}

impl Default for StudioPreviewSettings {
    fn default() -> Self {
        Self {
            show_skeleton: true,
            show_floor: true,
            show_wireframe: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct StudioPreviewImage {
    pub handle: Handle<Image>,
    pub current_size: (u32, u32),
    pub requested_size: (u32, u32),
}

impl Default for StudioPreviewImage {
    fn default() -> Self {
        Self {
            handle: Handle::default(),
            current_size: (512, 512),
            requested_size: (512, 512),
        }
    }
}

#[derive(Resource)]
pub struct StudioPreviewOrbit {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: Vec3,
}

impl Default for StudioPreviewOrbit {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.3,
            distance: 3.0,
            target: Vec3::new(0.0, 1.0, 0.0),
        }
    }
}

/// Tracks which scene entity is currently cloned into the preview.
#[derive(Resource, Default)]
pub struct StudioPreviewTracker {
    pub source_entity: Option<Entity>,
    /// Whether the orbit has been auto-fitted to the model bounds.
    pub auto_fitted: bool,
    /// Whether a model clone is actually spawned in the studio (false when
    /// nothing is selected or the selection has no model) — drives the
    /// panel's hint overlay.
    pub has_model: bool,
}

// ---------------------------------------------------------------------------
// Marker components
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct StudioPreviewCamera;

#[derive(Component)]
pub struct StudioPreviewLight;

/// Root of the cloned model in the preview scene.
#[derive(Component)]
pub struct StudioPreviewModel;

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

pub fn setup_studio_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let size = Extent3d {
        width: 512,
        height: 512,
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

    let image_handle = images.add(image);

    commands.insert_resource(StudioPreviewImage {
        handle: image_handle.clone(),
        current_size: (512, 512),
        requested_size: (512, 512),
    });

    // Camera
    commands.spawn((
        Camera3d::default(),
            Hdr,
            NormalPrepass,
            DepthPrepass,
            MotionVectorPrepass,
        Msaa::Off,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgba(0.12, 0.12, 0.14, 1.0)),
            order: -5,
            is_active: true,
            ..default()
        },
        RenderTarget::Image(image_handle.into()),
        Transform::from_xyz(0.0, 1.5, 3.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
        RenderLayers::layer(STUDIO_PREVIEW_LAYER),
        StudioPreviewCamera,
        IsolatedCamera,
        HideInHierarchy,
        EditorLocked,
        Name::new("Studio Preview Camera"),
    ));

    // ── 3-point lighting rig ──

    // Key light — warm, strong, upper-right front
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.97, 0.92),
            illuminance: 6000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, 0.5, 0.0)),
        RenderLayers::layer(STUDIO_PREVIEW_LAYER),
        StudioPreviewLight,
        HideInHierarchy,
        EditorLocked,
        Name::new("Studio Preview Key Light"),
    ));

    // Fill light — cool, softer, opposite side to reduce harsh shadows
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.75, 0.82, 1.0),
            illuminance: 2500.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.3, -0.9, 0.0)),
        RenderLayers::layer(STUDIO_PREVIEW_LAYER),
        StudioPreviewLight,
        HideInHierarchy,
        EditorLocked,
        Name::new("Studio Preview Fill Light"),
    ));

    // Rim/back light — subtle edge highlight from behind
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.85, 0.9, 1.0),
            illuminance: 1800.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.2, 3.0, 0.0)),
        RenderLayers::layer(STUDIO_PREVIEW_LAYER),
        StudioPreviewLight,
        HideInHierarchy,
        EditorLocked,
        Name::new("Studio Preview Rim Light"),
    ));

    // Ambient light — gentle fill to lift dark areas
    commands.spawn((
        PointLight {
            color: Color::srgb(0.9, 0.9, 0.95),
            intensity: 50_000.0,
            range: 30.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 4.0, 0.0),
        RenderLayers::layer(STUDIO_PREVIEW_LAYER),
        StudioPreviewLight,
        HideInHierarchy,
        EditorLocked,
        Name::new("Studio Preview Ambient"),
    ));

    // ── Backdrop — large curved wall behind the model ──
    // A half-cylinder behind the subject for a studio-like environment
    let backdrop_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.14, 0.14, 0.16),
        perceptual_roughness: 1.0,
        metallic: 0.0,
        reflectance: 0.0,
        unlit: false,
        ..default()
    });

    // Back wall — tall plane behind the model
    let wall_mesh = meshes.add(Plane3d::new(Vec3::Z, Vec2::new(8.0, 5.0)));
    commands.spawn((
        Mesh3d(wall_mesh),
        MeshMaterial3d(backdrop_material.clone()),
        Transform::from_xyz(0.0, 5.0, -6.0),
        RenderLayers::layer(STUDIO_PREVIEW_LAYER),
        HideInHierarchy,
        EditorLocked,
        Name::new("Studio Preview Backdrop"),
    ));

    // Checkerboard floor
    let checker_size = 16u32;
    let checker_tiles = 8u32; // 8x8 checker pattern
    let tex_dim = checker_size * checker_tiles;
    let mut checker_data = vec![0u8; (tex_dim * tex_dim * 4) as usize];
    for y in 0..tex_dim {
        for x in 0..tex_dim {
            let tx = x / checker_size;
            let ty = y / checker_size;
            let is_light = (tx + ty).is_multiple_of(2);
            let (r, g, b) = if is_light { (55, 55, 60) } else { (35, 35, 40) };
            let idx = ((y * tex_dim + x) * 4) as usize;
            checker_data[idx] = b; // BGRA
            checker_data[idx + 1] = g;
            checker_data[idx + 2] = r;
            checker_data[idx + 3] = 255;
        }
    }

    let mut checker_image = Image {
        data: Some(checker_data),
        ..default()
    };
    checker_image.texture_descriptor.size = Extent3d {
        width: tex_dim,
        height: tex_dim,
        depth_or_array_layers: 1,
    };
    checker_image.texture_descriptor.format = TextureFormat::Bgra8UnormSrgb;
    checker_image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;

    let checker_tex = images.add(checker_image);

    let floor_material = materials.add(StandardMaterial {
        base_color_texture: Some(checker_tex),
        perceptual_roughness: 0.9,
        metallic: 0.0,
        reflectance: 0.1,
        ..default()
    });

    let floor_mesh = meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(5.0)));

    commands.spawn((
        Mesh3d(floor_mesh),
        MeshMaterial3d(floor_material),
        Transform::from_translation(Vec3::ZERO),
        RenderLayers::layer(STUDIO_PREVIEW_LAYER),
        StudioPreviewFloor,
        HideInHierarchy,
        EditorLocked,
        Name::new("Studio Preview Floor"),
    ));
}

// ---------------------------------------------------------------------------
// Resize render target to match panel size
// ---------------------------------------------------------------------------

pub fn resize_preview(mut preview: ResMut<StudioPreviewImage>, mut images: ResMut<Assets<Image>>) {
    let (rw, rh) = preview.requested_size;
    let (cw, ch) = preview.current_size;

    if rw == cw && rh == ch {
        return;
    }

    let w = rw.clamp(64, 3840);
    let h = rh.clamp(64, 2160);

    if let Some(mut image) = images.get_mut(&preview.handle) {
        image.resize(Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        });
        preview.current_size = (w, h);
    }
}

// ---------------------------------------------------------------------------
// Model sync — clone the selected entity's GLTF scene into the preview layer
// ---------------------------------------------------------------------------

pub fn sync_preview_model(
    mut commands: Commands,
    editor_state: Res<AnimationEditorState>,
    mut tracker: ResMut<StudioPreviewTracker>,
    asset_server: Res<AssetServer>,
    mesh_query: Query<&MeshInstanceData>,
    animator_query: Query<&renzora_animation::AnimatorComponent>,
    parent_query: Query<&ChildOf>,
    children_query: Query<&Children>,
    project: Option<Res<renzora::core::CurrentProject>>,
    existing_preview: Query<Entity, With<StudioPreviewModel>>,
) {
    let selected = editor_state.selected_entity;

    // If selection hasn't changed, nothing to do
    if tracker.source_entity == selected {
        return;
    }
    tracker.source_entity = selected;
    tracker.auto_fitted = false;
    tracker.has_model = false;

    // Despawn old preview model
    for entity in existing_preview.iter() {
        commands.entity(entity).despawn();
    }

    // If nothing selected, done
    let Some(source) = selected else {
        info!("[studio_preview] No entity selected");
        return;
    };

    let Some(model_path) = resolve_model_path(
        source,
        &mesh_query,
        &animator_query,
        &parent_query,
        &children_query,
        project.as_deref(),
    ) else {
        warn!(
            "[studio_preview] No model path resolvable for selected entity {:?}",
            source
        );
        return;
    };

    // Load the default scene directly from the GLB file
    let scene_path = format!("{}#Scene0", model_path);
    info!("[studio_preview] Loading scene from '{}'", scene_path);
    let scene_handle: Handle<bevy::world_serialization::WorldAsset> = asset_server.load(&scene_path);

    // Spawn the preview model on the studio preview render layer
    let root = commands
        .spawn((
            Transform::default(),
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
            ViewVisibility::default(),
            RenderLayers::layer(STUDIO_PREVIEW_LAYER),
            StudioPreviewModel,
            HideInHierarchy,
            EditorLocked,
            Name::new("Studio Preview Model"),
        ))
        .id();

    commands.spawn((
        bevy::world_serialization::WorldAssetRoot(scene_handle),
        Transform::default(),
        Visibility::Visible,
        InheritedVisibility::VISIBLE,
        ViewVisibility::default(),
        RenderLayers::layer(STUDIO_PREVIEW_LAYER),
        HideInHierarchy,
        ChildOf(root),
    ));

    tracker.has_model = true;
    info!(
        "[studio_preview] Loaded model '{}' into preview",
        model_path
    );
}

/// Find the GLB to preview for an arbitrary selected entity. The path is not
/// always on the selection itself — users pick mesh children or bones, group
/// roots hold the `MeshInstanceData`, and some animator-bearing entities have
/// no model reference at all. Resolution order:
/// 1. `MeshInstanceData.model_path` on the entity or any ancestor,
/// 2. on any descendant,
/// 3. inferred from the animator's clip paths — clips live in
///    `<model_dir>/animations/`, so scan `<model_dir>` for a `.glb`/`.gltf`.
fn resolve_model_path(
    source: Entity,
    mesh_query: &Query<&MeshInstanceData>,
    animator_query: &Query<&renzora_animation::AnimatorComponent>,
    parent_query: &Query<&ChildOf>,
    children_query: &Query<&Children>,
    project: Option<&renzora::core::CurrentProject>,
) -> Option<String> {
    let model_of = |e: Entity| mesh_query.get(e).ok().and_then(|m| m.model_path.clone());
    let ancestry = |start: Entity| {
        std::iter::successors(Some(start), |&e| parent_query.get(e).ok().map(|c| c.parent()))
    };

    // 1. Self + ancestors.
    if let Some(path) = ancestry(source).find_map(model_of) {
        return Some(path);
    }

    // 2. Descendants (group parents whose model lives on a child).
    let mut stack: Vec<Entity> = children_query
        .get(source)
        .map(|c| c.iter().collect())
        .unwrap_or_default();
    while let Some(e) = stack.pop() {
        if let Some(path) = model_of(e) {
            return Some(path);
        }
        if let Ok(children) = children_query.get(e) {
            stack.extend(children.iter());
        }
    }

    // 3. Infer from the animator's clip paths.
    let project = project?;
    let animator = ancestry(source).find_map(|e| animator_query.get(e).ok())?;
    let clip_path = animator.clips.first().map(|c| c.path.clone())?;
    let clip_dir = std::path::Path::new(&clip_path).parent()?;
    let model_dir = if clip_dir.file_name().is_some_and(|n| n == "animations") {
        clip_dir.parent()?
    } else {
        clip_dir
    };
    let abs_dir = project.path.join(model_dir);
    let dir_name = model_dir.file_name().and_then(|n| n.to_str());
    let mut fallback: Option<String> = None;
    for entry in std::fs::read_dir(&abs_dir).ok()?.filter_map(|e| e.ok()) {
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase());
        if !matches!(ext.as_deref(), Some("glb") | Some("gltf")) {
            continue;
        }
        let rel = model_dir
            .join(entry.file_name())
            .to_string_lossy()
            .replace('\\', "/");
        let stem_matches = path
            .file_stem()
            .and_then(|s| s.to_str())
            .is_some_and(|s| Some(s) == dir_name);
        if stem_matches {
            return Some(rel);
        }
        fallback.get_or_insert(rel);
    }
    fallback
}

/// Observer: the moment any entity is parented into the studio-preview subtree
/// (e.g. a GLB-internal node like `RootNode.001` that Bevy's `SpawnScene`
/// schedule just inserted), stamp it with `HideInHierarchy` + the preview
/// `RenderLayers`. Runs synchronously with `ChildOf` insertion, so the entity
/// never leaks to the main camera or the hierarchy panel for a frame.
pub fn hide_new_preview_descendants(
    trigger: On<Insert, ChildOf>,
    parent_q: Query<&ChildOf>,
    preview_root_q: Query<(), With<StudioPreviewModel>>,
    already_hidden: Query<(), With<HideInHierarchy>>,
    mut commands: Commands,
) {
    let entity = trigger.entity;
    if already_hidden.contains(entity) {
        return;
    }
    let mut cursor = entity;
    while let Ok(child_of) = parent_q.get(cursor) {
        let parent = child_of.parent();
        if preview_root_q.contains(parent) || already_hidden.contains(parent) {
            commands
                .entity(entity)
                .try_insert((RenderLayers::layer(STUDIO_PREVIEW_LAYER), HideInHierarchy));
            return;
        }
        cursor = parent;
    }
}

/// Continuously propagate `RenderLayers` + `HideInHierarchy` to all
/// descendants of preview model entities. GLTF scenes spawn children
/// asynchronously over multiple frames and those children get default
/// `RenderLayers` (layer 0) and no `HideInHierarchy` — so they'd leak into
/// the main viewport render and show up in the hierarchy panel as if they
/// were loose scene entities. We walk the subtree each frame and fix both.
pub fn propagate_preview_layer(
    mut commands: Commands,
    preview_roots: Query<Entity, With<StudioPreviewModel>>,
    children_query: Query<&Children>,
    layer_query: Query<&RenderLayers>,
    hide_query: Query<(), With<HideInHierarchy>>,
) {
    let target = RenderLayers::layer(STUDIO_PREVIEW_LAYER);

    for root in preview_roots.iter() {
        let mut stack: Vec<Entity> = Vec::new();
        if let Ok(children) = children_query.get(root) {
            stack.extend(children.iter());
        }

        while let Some(child) = stack.pop() {
            let needs_layer = match layer_query.get(child) {
                Ok(layers) => *layers != target,
                Err(_) => true,
            };
            if needs_layer {
                commands.entity(child).insert(target.clone());
            }

            if hide_query.get(child).is_err() {
                commands.entity(child).insert(HideInHierarchy);
            }

            if let Ok(grandchildren) = children_query.get(child) {
                stack.extend(grandchildren.iter());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Auto-fit camera to model bounds
// ---------------------------------------------------------------------------

pub fn auto_fit_preview_camera(
    mut tracker: ResMut<StudioPreviewTracker>,
    mut orbit: ResMut<StudioPreviewOrbit>,
    preview_roots: Query<Entity, With<StudioPreviewModel>>,
    children_query: Query<&Children>,
    aabb_query: Query<(&bevy::camera::primitives::Aabb, &GlobalTransform)>,
) {
    if tracker.auto_fitted {
        return;
    }

    // Need a preview model to exist
    let Some(root) = preview_roots.iter().next() else {
        return;
    };

    // Walk all descendants and compute world-space bounding box
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    let mut found_any = false;

    let mut stack: Vec<Entity> = Vec::new();
    if let Ok(children) = children_query.get(root) {
        stack.extend(children.iter());
    }

    while let Some(child) = stack.pop() {
        if let Ok((aabb, global_transform)) = aabb_query.get(child) {
            let center = Vec3::from(aabb.center);
            let half = Vec3::from(aabb.half_extents);

            for sx in [-1.0f32, 1.0] {
                for sy in [-1.0f32, 1.0] {
                    for sz in [-1.0f32, 1.0] {
                        let corner = center + half * Vec3::new(sx, sy, sz);
                        let world_pos = global_transform.transform_point(corner);
                        min = min.min(world_pos);
                        max = max.max(world_pos);
                        found_any = true;
                    }
                }
            }
        }
        if let Ok(grandchildren) = children_query.get(child) {
            stack.extend(grandchildren.iter());
        }
    }

    if !found_any {
        return; // Meshes haven't spawned yet, retry next frame
    }

    let center = (min + max) * 0.5;
    let extents = max - min;
    let radius = extents.length() * 0.5;

    orbit.target = center;
    orbit.distance = (radius * 2.5).max(1.0);
    orbit.yaw = 0.5;
    orbit.pitch = 0.2;

    tracker.auto_fitted = true;
    info!(
        "[studio_preview] Auto-fitted camera: center={:?}, radius={:.2}, distance={:.2}",
        center, radius, orbit.distance
    );
}

// ---------------------------------------------------------------------------
// Orbit camera
// ---------------------------------------------------------------------------

pub fn update_studio_preview_camera(
    orbit: Res<StudioPreviewOrbit>,
    mut camera: Query<&mut Transform, With<StudioPreviewCamera>>,
) {
    for mut transform in camera.iter_mut() {
        let x = orbit.distance * orbit.pitch.cos() * orbit.yaw.sin();
        let y = orbit.distance * orbit.pitch.sin();
        let z = orbit.distance * orbit.pitch.cos() * orbit.yaw.cos();

        transform.translation = orbit.target + Vec3::new(x, y, z);
        transform.look_at(orbit.target, Vec3::Y);
    }
}

// ---------------------------------------------------------------------------
// Skeleton gizmo — draws bone hierarchy on the preview model
// ---------------------------------------------------------------------------

use bevy::animation::AnimationTargetId;
use bevy::gizmos::config::{GizmoConfig, GizmoConfigGroup, GizmoLineConfig};
use bevy::gizmos::AppGizmoBuilder;

/// Gizmo group that renders on the studio preview layer.
#[derive(Default, Reflect, GizmoConfigGroup)]
#[reflect(Default)]
pub struct StudioPreviewGizmoGroup;

/// Register the studio preview gizmo config.
pub fn register_preview_gizmos(app: &mut bevy::app::App) {
    app.insert_gizmo_config(
        StudioPreviewGizmoGroup,
        GizmoConfig {
            depth_bias: -1.0,
            line: GizmoLineConfig {
                width: 2.0,
                ..default()
            },
            render_layers: RenderLayers::layer(STUDIO_PREVIEW_LAYER),
            ..default()
        },
    );
}

/// Draw skeleton bones on the preview model using gizmos.
pub fn draw_preview_skeleton(
    mut gizmos: Gizmos<StudioPreviewGizmoGroup>,
    settings: Res<StudioPreviewSettings>,
    preview_roots: Query<Entity, With<StudioPreviewModel>>,
    children_q: Query<&Children>,
    parent_q: Query<&ChildOf>,
    target_q: Query<(), With<AnimationTargetId>>,
    global_transforms: Query<&GlobalTransform>,
) {
    if !settings.show_skeleton {
        return;
    }

    let Some(root) = preview_roots.iter().next() else {
        return;
    };

    let bone_color = Color::srgba(0.9, 0.9, 0.9, 0.6);
    let joint_color = Color::srgba(0.4, 0.85, 1.0, 0.8);

    // Collect all animation target entities
    let mut bones = Vec::new();
    collect_bones_recursive(root, &children_q, &target_q, &mut bones);

    for &bone in &bones {
        let Ok(bone_gt) = global_transforms.get(bone) else {
            continue;
        };
        let bone_pos = bone_gt.translation();

        // Joint sphere
        gizmos.sphere(Isometry3d::from_translation(bone_pos), 0.015, joint_color);

        // Line to parent if parent is also a bone
        if let Ok(child_of) = parent_q.get(bone) {
            let parent = child_of.parent();
            if target_q.get(parent).is_ok() {
                if let Ok(parent_gt) = global_transforms.get(parent) {
                    gizmos.line(parent_gt.translation(), bone_pos, bone_color);
                }
            }
        }
    }
}

fn collect_bones_recursive(
    entity: Entity,
    children_q: &Query<&Children>,
    target_q: &Query<(), With<AnimationTargetId>>,
    out: &mut Vec<Entity>,
) {
    if target_q.get(entity).is_ok() {
        out.push(entity);
    }
    if let Ok(children) = children_q.get(entity) {
        for child in children.iter() {
            collect_bones_recursive(child, children_q, target_q, out);
        }
    }
}

/// Marker for the preview floor entity.
#[derive(Component)]
pub struct StudioPreviewFloor;

/// Toggle floor visibility based on settings.
pub fn sync_floor_visibility(
    settings: Res<StudioPreviewSettings>,
    mut floor_q: Query<&mut Visibility, With<StudioPreviewFloor>>,
) {
    let target = if settings.show_floor {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for mut vis in floor_q.iter_mut() {
        *vis = target;
    }
}
