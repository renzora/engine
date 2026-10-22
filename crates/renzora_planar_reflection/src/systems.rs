//! The per-frame driver: decide which mirrors render, keep a capture rig for
//! each, and point it at the right place.
//!
//! # What a rig is
//!
//! One per (surface, viewing camera) pair:
//!
//! - a **mirror camera**, rendering the scene into its own HDR image from the
//!   viewing camera's pose reflected across the plane, through an
//!   [`ObliqueProjection`] whose near plane is the mirror;
//! - an **overlay**, a child of the surface drawing the same mesh again with
//!   [`ReflectionOverlayMaterial`], which samples that image in screen space
//!   over the surface's own material.
//!
//! Per pair rather than per surface because the capture is only correct for
//! the camera it was mirrored from. Two docked viewports looking at one mirror
//! need two captures, and each overlay must show only its own; the overlay
//! shader discards in every view but the one it belongs to (see
//! `reflection_overlay.wesl`).
//!
//! # Which camera is "the" camera
//!
//! In the editor, the viewport cameras (`ViewportCamera`): every docked slot
//! is a separate view of the scene, and play mode drives the same cameras onto
//! the game camera's pose, so they are the scene's view in play too. Inactive
//! slots (undocked, or idled by the redraw gate) cost nothing: their rigs stay
//! but render no frames. In a shipped game, the camera effect routing targets:
//! the active `DefaultCamera`, else the first active `SceneCamera`, else the
//! first active ordinary `Camera3d` a script may have spawned.
//!
//! # Why recursion cannot happen
//!
//! A mirror camera never draws any overlay. Its own surface's overlay lies on
//! the plane, which the oblique near plane (pushed out by `clip_offset`) clips;
//! every other overlay discards because the mirror camera is not the view it
//! was captured for. So a mirror seen in another mirror shows its own material,
//! never a reflection of a reflection, and no capture can depend on another
//! capture made the same frame.
//!
//! The per-view discard was chosen over a dedicated render layer because a
//! layer only works if every viewing camera renders it. The editor's cameras
//! could be given one, but a game's camera is the user's, with whatever
//! `RenderLayers` the game set, and rewriting those from here would be a quiet
//! change to someone else's entity. The overlay inherits the surface's own
//! layers instead, so whichever camera sees the surface sees its reflection.
//!
//! # Change detection
//!
//! The editor renders the viewport only while something changes
//! (`renzora_viewport::redraw`), and an overlay is a scene entity it watches.
//! Every write below is therefore guarded by a comparison, and nothing on the
//! overlay is touched while its viewing camera is idle; writing unconditionally
//! would keep the viewport awake forever.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Exposure, Hdr, RenderTarget};
use bevy::core_pipeline::prepass::{
    DeferredPrepass, DepthPrepass, MotionVectorPrepass, NormalPrepass,
};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::core_pipeline::Skybox;
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::light::{EnvironmentMapLight, GeneratedEnvironmentMapLight, NotShadowCaster, NotShadowReceiver};
#[cfg(not(target_arch = "wasm32"))]
use bevy::pbr::DistanceFog;
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
    TextureViewDimension,
};
use renzora::core::viewport_types::{ViewportSettings, ViewportView};
use renzora::{
    DefaultCamera, EditorLocked, HideInHierarchy, IsolatedCamera, ReflectionCamera,
    ResolvedRenderingMode, SceneCamera, ViewportCamera,
};

use crate::component::{PlanarReflection, PlanarReflectionSettings};
use crate::material::{OverlayParams, ReflectionOverlayMaterial};
use crate::math::{mirror_camera_transform, plane_in_view_space, ObliqueProjection, ReflectionPlane};
use crate::select::{evaluate, scene_layers, select_active, update_due, Candidate, SurfaceView};

/// Brightness of the sky a mirror camera draws when it borrows the viewing
/// camera's baked environment as a `Skybox`. Matches the editor's secondary
/// viewports, which show the same baked cubemap the same way.
const BORROWED_SKY_BRIGHTNESS: f32 = 1.0;

/// On a mirror camera: which surface and which viewing camera it captures for.
#[derive(Component, Clone, Copy, Debug)]
pub struct MirrorCamera {
    pub surface: Entity,
    pub source: Entity,
}

/// On an overlay: which surface it covers and which viewing camera it shows.
#[derive(Component, Clone, Copy, Debug)]
pub struct MirrorOverlay {
    pub surface: Entity,
    pub source: Entity,
}

/// One capture rig. See the module docs.
#[derive(Debug)]
struct Rig {
    camera: Entity,
    overlay: Entity,
    image: Handle<Image>,
    material: Handle<ReflectionOverlayMaterial>,
    size: UVec2,
    /// The image holds a finished capture, so the overlay may show it.
    rendered: bool,
    /// The camera was set to render last frame, which makes `rendered` true
    /// this frame.
    active_last_frame: bool,
    stagger: u64,
}

/// Every live rig, keyed by (surface, viewing camera).
#[derive(Resource, Default)]
pub struct ReflectionRigs {
    rigs: HashMap<(Entity, Entity), Rig>,
    next_stagger: u64,
    /// A 1x1 cube every mirror camera carries as its `EnvironmentMapLight`
    /// until the viewing camera's real maps are copied over. The IBL bindings
    /// of a camera's mesh-view layout are fixed when it first renders, so the
    /// component has to be there from spawn even when there is nothing to put
    /// in it yet.
    placeholder_cube: Option<Handle<Image>>,
}

impl ReflectionRigs {
    /// How many rigs exist (rendering or not).
    pub fn len(&self) -> usize {
        self.rigs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rigs.is_empty()
    }

    /// The mirror camera capturing `surface` for `source`, if any.
    pub fn camera_for(&self, surface: Entity, source: Entity) -> Option<Entity> {
        self.rigs.get(&(surface, source)).map(|r| r.camera)
    }
}

/// This frame's decision, written by [`plan_reflections`].
#[derive(Resource, Default, Debug)]
pub struct ReflectionPlan {
    /// The pairs that render this frame, nearest first.
    pub active: Vec<Candidate>,
    /// The viewing cameras that are live this frame. A rig whose camera is not
    /// in here is left exactly as it is (see "Change detection").
    pub live_cameras: Vec<Entity>,
    /// Frame counter for `update_every`.
    pub frame: u64,
}

impl ReflectionPlan {
    fn is_active(&self, surface: Entity, source: Entity) -> bool {
        self.active
            .iter()
            .any(|c| c.surface == surface && c.camera == source)
    }
}

type SourceItem = (
    Entity,
    &'static Camera,
    &'static GlobalTransform,
    &'static Projection,
    Has<ViewportCamera>,
    Has<SceneCamera>,
    Has<DefaultCamera>,
    Has<IsolatedCamera>,
);

/// Cameras that may be viewing cameras: every 3D camera except captures.
type SourceFilter = (With<Camera3d>, Without<MirrorCamera>, Without<ReflectionCamera>);

/// The viewing cameras this frame. See the module docs for the rules.
fn viewing_cameras(
    sources: &Query<SourceItem, SourceFilter>,
    view: Option<ViewportView>,
) -> Vec<Entity> {
    let mut viewport = Vec::new();
    let mut any_viewport = false;
    for (entity, camera, _, _, is_viewport, _, _, _) in sources.iter() {
        if is_viewport {
            any_viewport = true;
            if camera.is_active {
                viewport.push(entity);
            }
        }
    }
    if any_viewport {
        // In the 2D view the primary 3D camera stays active only to keep the
        // sky bake alive; the panel shows the 2D camera.
        if view == Some(ViewportView::Two) {
            return Vec::new();
        }
        viewport.sort_unstable_by_key(|e| e.to_bits());
        return viewport;
    }
    let active = |&(_, camera, ..): &(Entity, &Camera, &GlobalTransform, &Projection, bool, bool, bool, bool)| camera.is_active;
    let pick = sources
        .iter()
        .filter(active)
        .find(|(_, _, _, _, _, scene, default, _)| *scene && *default)
        .or_else(|| sources.iter().filter(active).find(|(_, _, _, _, _, scene, _, _)| *scene))
        .or_else(|| sources.iter().filter(active).find(|(.., isolated)| !*isolated));
    pick.map(|(e, ..)| vec![e]).unwrap_or_default()
}

/// Decide which pairs render this frame.
pub fn plan_reflections(
    settings: Res<PlanarReflectionSettings>,
    viewport_settings: Option<Res<ViewportSettings>>,
    surfaces: Query<(
        Entity,
        &PlanarReflection,
        &GlobalTransform,
        Option<&Aabb>,
        &InheritedVisibility,
        Option<&crate::fit::MeshPlaneFit>,
    )>,
    sources: Query<SourceItem, SourceFilter>,
    mut plan: ResMut<ReflectionPlan>,
) {
    plan.frame = plan.frame.wrapping_add(1);
    let live = viewing_cameras(&sources, viewport_settings.map(|s| s.viewport_view));

    let mut candidates = Vec::new();
    for &camera_entity in &live {
        let Ok((_, _, camera_transform, projection, ..)) = sources.get(camera_entity) else {
            continue;
        };
        // Only perspective views are mirrored. An orthographic camera's
        // reflection is a different projection altogether, and no editor or
        // game camera here uses one for 3D.
        if !matches!(projection, Projection::Perspective(_)) {
            continue;
        }
        let frustum = projection.compute_frustum(camera_transform);
        let camera_pos = camera_transform.translation();
        for (surface, reflection, transform, aabb, visibility, fit) in &surfaces {
            if !reflection.enabled || !visibility.get() || reflection.strength() <= 0.0 {
                continue;
            }
            let Some(plane) = ReflectionPlane::for_surface(transform, reflection, aabb, fit) else {
                continue;
            };
            let world_from_local = transform.affine();
            let view = SurfaceView {
                plane,
                world_from_local: &world_from_local,
                aabb,
                max_distance: reflection.max_distance(),
            };
            if let Ok(distance) = evaluate(&view, camera_pos, &frustum) {
                candidates.push(Candidate {
                    surface,
                    camera: camera_entity,
                    distance,
                });
            }
        }
    }

    plan.active = select_active(candidates, settings.max_active);
    plan.live_cameras = live;
}

/// The capture image: HDR, linear-filtered and clamped, GPU only.
fn capture_image(size: UVec2) -> Image {
    let mut image = Image::new_uninit(
        Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        // Rgba16Float, the same format as the HDR main texture, so the capture
        // keeps values above 1 for the viewing camera's single tonemap.
        TextureFormat::Rgba16Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    image
}

fn placeholder_cube(images: &mut Assets<Image>) -> Handle<Image> {
    // 1 texel x 6 faces x 8 bytes (Rgba16Float).
    let mut image = Image {
        data: Some(vec![0u8; 6 * 8]),
        ..default()
    };
    image.texture_descriptor.size = Extent3d {
        width: 1,
        height: 1,
        depth_or_array_layers: 6,
    };
    image.texture_descriptor.dimension = TextureDimension::D2;
    image.texture_descriptor.format = TextureFormat::Rgba16Float;
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING;
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    images.add(image)
}

/// The capture size for a viewing camera: its viewport at the surface's scale.
fn capture_size(camera: &Camera, scale: f32) -> Option<UVec2> {
    let full = camera.physical_viewport_size()?;
    if full.x == 0 || full.y == 0 {
        return None;
    }
    let scaled = (full.as_vec2() * scale).round().as_uvec2();
    Some(scaled.max(UVec2::ONE))
}

type SpawnSurfaceItem = (
    &'static PlanarReflection,
    Option<&'static Mesh3d>,
    Option<&'static RenderLayers>,
);

type SpawnSourceItem = (
    &'static Camera,
    Option<&'static RenderLayers>,
    Option<&'static DistanceFogRef>,
);

// The engine attaches no `DistanceFog` on the web (it would cost a uniform
// slot WebGPU does not have; see `renzora_engine::camera`). A mirror camera
// copies the viewing camera's fog, so there it copies a marker nothing ever
// carries, which keeps the code below free of per-platform branches.
#[cfg(not(target_arch = "wasm32"))]
type DistanceFogRef = DistanceFog;
#[cfg(target_arch = "wasm32")]
type DistanceFogRef = NoFog;
/// Stand-in for `DistanceFog` on the web, where no camera has fog.
#[cfg(target_arch = "wasm32")]
#[derive(Component, Clone)]
pub struct NoFog;

/// Spawn rigs for newly active pairs and retire rigs whose surface or viewing
/// camera is gone.
#[allow(clippy::too_many_arguments)]
pub fn maintain_rigs(
    mut commands: Commands,
    plan: Res<ReflectionPlan>,
    mut rigs: ResMut<ReflectionRigs>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<ReflectionOverlayMaterial>>,
    surfaces: Query<SpawnSurfaceItem, Without<MirrorOverlay>>,
    sources: Query<SpawnSourceItem, SourceFilter>,
    rendering_mode: Option<Res<ResolvedRenderingMode>>,
) {
    // Retire first, so a surface that lost its component and got it back in
    // one frame is rebuilt rather than left pointing at stale state.
    let retired: Vec<(Entity, Entity)> = rigs
        .rigs
        .keys()
        .copied()
        .filter(|(surface, source)| !surfaces.contains(*surface) || !sources.contains(*source))
        .collect();
    for key in retired {
        if let Some(rig) = rigs.rigs.remove(&key) {
            commands.entity(rig.camera).try_despawn();
            commands.entity(rig.overlay).try_despawn();
            // Dropping the handles frees the image and the material.
        }
    }

    let deferred = rendering_mode.is_some_and(|m| m.is_deferred());
    for candidate in &plan.active {
        let key = (candidate.surface, candidate.camera);
        if rigs.rigs.contains_key(&key) {
            continue;
        }
        let Ok((reflection, Some(mesh), surface_layers)) = surfaces.get(candidate.surface) else {
            continue;
        };
        let Ok((source_camera, source_layers, source_fog)) = sources.get(candidate.camera)
        else {
            continue;
        };
        let Some(size) = capture_size(source_camera, reflection.resolution_scale()) else {
            continue;
        };

        let cube = rigs
            .placeholder_cube
            .get_or_insert_with(|| placeholder_cube(&mut images))
            .clone();
        let image = images.add(capture_image(size));
        let material = materials.add(ReflectionOverlayMaterial {
            params: OverlayParams::default(),
            reflection: image.clone(),
        });

        // In a Deferred project every 3D camera needs the deferred prepass
        // before its first frame. `renzora_engine` adds it in PostUpdate too,
        // but unordered against this, and a prepass attachment that appears
        // after the first render is a wgpu validation crash. It goes on BEFORE
        // `Camera3d`, so Solari's insert observer, which forces a deferred
        // prepass onto new cameras while it is active, sees one already there:
        // otherwise it would claim this one as forced and strip it again when
        // Solari goes idle, in a project that needs it.
        let mut camera = if deferred {
            commands.spawn(DeferredPrepass)
        } else {
            commands.spawn_empty()
        };
        camera.insert((
            Camera3d::default(),
            Camera {
                // Before the viewing camera, so the capture is finished when
                // the main pass samples it in the same frame.
                order: source_camera.order - 1,
                // Off until `drive_rigs`, which runs right after this, has
                // posed it, so it can never render a frame from the origin.
                is_active: false,
                clear_color: source_camera.clear_color,
                ..default()
            },
            RenderTarget::Image(image.clone().into()),
            Projection::custom(ObliqueProjection::new(PerspectiveProjection::default(), None)),
            Transform::default(),
            // HDR in and out with tonemapping off: the viewing camera's main
            // pass tonemaps once, after the overlay has blended the capture in.
            (Hdr, Tonemapping::None, DebandDither::Disabled, Msaa::Off),
            // The same prepass set every editor and preview camera carries, so
            // the mirror shares their one PBR and prepass pipeline layout. The
            // set must be complete at spawn: Bevy fixes a camera's prepass
            // attachments the first time it renders.
            (NormalPrepass, DepthPrepass, MotionVectorPrepass),
            scene_layers(source_layers),
            EnvironmentMapLight {
                diffuse_map: cube.clone(),
                specular_map: cube,
                intensity: 0.0,
                rotation: Quat::IDENTITY,
                affects_lightmapped_mesh_diffuse: true,
            },
            // `IsolatedCamera` keeps every routed effect (bloom, AO, TAA,
            // Solari, tonemapping settings) and the skybox and contact-shadow
            // helpers off it. `HideInHierarchy` keeps it out of the hierarchy,
            // out of scene saves and out of scene clears; `EditorLocked` out of
            // editing.
            (IsolatedCamera, ReflectionCamera, HideInHierarchy, EditorLocked),
            MirrorCamera {
                surface: candidate.surface,
                source: candidate.camera,
            },
            Name::new("Planar Reflection Camera"),
        ));
        if let Some(fog) = source_fog {
            // Present from spawn for the same layout reason (fog is a
            // mesh-view binding); `drive_rigs` keeps it in step.
            camera.insert(fog.clone());
        }
        let camera = camera.id();

        let mut overlay = commands.spawn((
            Mesh3d(mesh.0.clone()),
            MeshMaterial3d(material.clone()),
            Transform::IDENTITY,
            // Hidden until the capture has rendered once, or the first frame
            // would show an empty image over the surface.
            Visibility::Hidden,
            NotShadowCaster,
            NotShadowReceiver,
            HideInHierarchy,
            EditorLocked,
            MirrorOverlay {
                surface: candidate.surface,
                source: candidate.camera,
            },
            ChildOf(candidate.surface),
        ));
        if let Some(layers) = surface_layers {
            overlay.insert(layers.clone());
        }
        let overlay = overlay.id();

        let stagger = rigs.next_stagger;
        rigs.next_stagger = rigs.next_stagger.wrapping_add(1);
        rigs.rigs.insert(
            key,
            Rig {
                camera,
                overlay,
                image,
                material,
                size,
                rendered: false,
                active_last_frame: false,
                stagger,
            },
        );
    }
}

type DriveSourceItem = (
    &'static Camera,
    &'static GlobalTransform,
    &'static Projection,
    Option<&'static RenderLayers>,
    &'static Exposure,
    Option<&'static Skybox>,
    Option<&'static GeneratedEnvironmentMapLight>,
    Option<&'static EnvironmentMapLight>,
    Option<&'static DistanceFogRef>,
);

type MirrorCameraItem = (
    &'static mut Camera,
    &'static mut Transform,
    &'static mut GlobalTransform,
    &'static mut Projection,
    &'static mut RenderTarget,
    &'static mut RenderLayers,
    &'static mut Exposure,
    &'static mut EnvironmentMapLight,
    Option<&'static Skybox>,
    Option<&'static mut DistanceFogRef>,
);

type OverlayItem = (
    &'static mut Visibility,
    &'static mut Mesh3d,
    Option<&'static RenderLayers>,
);

type SurfaceItem = (
    &'static PlanarReflection,
    &'static GlobalTransform,
    Option<&'static Mesh3d>,
    Option<&'static RenderLayers>,
    Option<&'static Aabb>,
    Option<&'static crate::fit::MeshPlaneFit>,
);

/// Pose every rig, switch its camera on or off, and keep its overlay in step.
#[allow(clippy::too_many_arguments)]
pub fn drive_rigs(
    mut commands: Commands,
    plan: Res<ReflectionPlan>,
    mut rigs: ResMut<ReflectionRigs>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<ReflectionOverlayMaterial>>,
    surfaces: Query<SurfaceItem, (Without<MirrorOverlay>, Without<MirrorCamera>)>,
    sources: Query<DriveSourceItem, SourceFilter>,
    mut cameras: Query<MirrorCameraItem, (With<MirrorCamera>, Without<MirrorOverlay>)>,
    mut overlays: Query<OverlayItem, (With<MirrorOverlay>, Without<MirrorCamera>)>,
) {
    for (&(surface, source), rig) in rigs.rigs.iter_mut() {
        if rig.active_last_frame {
            rig.rendered = true;
        }
        let Ok((
            mut camera,
            mut transform,
            mut global,
            mut projection,
            mut target,
            mut layers,
            mut exposure,
            mut env,
            skybox,
            mut fog,
        )) = cameras.get_mut(rig.camera)
        else {
            continue;
        };
        let live = plan.live_cameras.contains(&source);

        if !plan.is_active(surface, source) {
            if camera.is_active {
                camera.is_active = false;
            }
            rig.active_last_frame = false;
            // Only hide the overlay while its viewing camera is live: an idle
            // viewport must not see a change it would wake up for.
            if live {
                rig.rendered = false;
                if let Ok((mut visibility, ..)) = overlays.get_mut(rig.overlay) {
                    visibility.set_if_neq(Visibility::Hidden);
                }
            }
            continue;
        }

        let (Ok((reflection, surface_transform, surface_mesh, surface_layers, surface_aabb, surface_fit)), Ok(source_item)) =
            (surfaces.get(surface), sources.get(source))
        else {
            continue;
        };
        let (
            source_camera,
            source_transform,
            source_projection,
            source_layers,
            source_exposure,
            source_skybox,
            source_generated,
            source_env,
            source_fog,
        ) = source_item;
        let Projection::Perspective(perspective) = source_projection else {
            continue;
        };
        let Some(plane) = ReflectionPlane::for_surface(surface_transform, reflection, surface_aabb, surface_fit) else {
            continue;
        };

        // Pose: the viewing camera reflected across the plane. Written to the
        // GlobalTransform as well as the Transform because this runs after
        // transform propagation, so the frustum and the render both use this
        // frame's pose rather than last frame's.
        let pose = mirror_camera_transform(source_transform, &plane);
        transform.set_if_neq(pose);
        global.set_if_neq(GlobalTransform::from(pose));

        // Clip plane: the mirror pushed `clip_offset` toward the reflected
        // side, so the surface itself and its overlay fall behind the near
        // plane of their own capture.
        let clip = plane.offset(reflection.clip_offset()).to_vec4();
        let clip_view = plane_in_view_space(clip, &GlobalTransform::from(pose).affine());
        let wanted = ObliqueProjection::new(perspective.clone(), Some(clip_view));
        let same = match &*projection {
            Projection::Custom(custom) => custom
                .get::<ObliqueProjection>()
                .is_some_and(|current| current.same_as(&wanted)),
            _ => false,
        };
        if !same {
            *projection = Projection::custom(wanted);
        }

        // Resolution: follows the viewing camera's viewport. A new image
        // rather than a resize in place, because the overlay's bind group
        // holds the old texture view and is only rebuilt when the material
        // asset changes, which assigning the new handle does.
        if let Some(size) = capture_size(source_camera, reflection.resolution_scale()) {
            if size != rig.size {
                rig.image = images.add(capture_image(size));
                rig.size = size;
                rig.rendered = false;
                *target = RenderTarget::Image(rig.image.clone().into());
            }
        }

        let due = update_due(plan.frame, reflection.update_every(), rig.stagger);
        if camera.is_active != due {
            camera.is_active = due;
        }
        rig.active_last_frame = due;
        let order = source_camera.order - 1;
        if camera.order != order {
            camera.order = order;
        }

        layers.set_if_neq(scene_layers(source_layers));
        if exposure.ev100 != source_exposure.ev100 {
            exposure.ev100 = source_exposure.ev100;
        }

        // Image-based light: the viewing camera's own maps, so metal looks
        // the same in the reflection as out of it.
        if let Some(source_env) = source_env {
            if env.diffuse_map != source_env.diffuse_map
                || env.specular_map != source_env.specular_map
                || env.intensity != source_env.intensity
                || env.rotation != source_env.rotation
            {
                env.diffuse_map = source_env.diffuse_map.clone();
                env.specular_map = source_env.specular_map.clone();
                env.intensity = source_env.intensity;
                env.rotation = source_env.rotation;
            }
        }

        // Sky: a mirror camera cannot carry an `Atmosphere` (a second
        // atmosphere camera is a second bake, which crashes wgpu; see
        // `renzora_engine::camera`). It borrows the viewing camera's skybox,
        // or the primary editor camera's baked atmosphere cubemap, the way the
        // secondary viewports do. `Skybox` is its own pass, not a mesh-view
        // binding, so adding and removing it is safe.
        let wanted_sky = source_skybox.cloned().or_else(|| {
            source_generated.map(|g| Skybox {
                image: Some(g.environment_map.clone()),
                brightness: BORROWED_SKY_BRIGHTNESS,
                rotation: Quat::IDENTITY,
            })
        });
        match (&wanted_sky, skybox) {
            (Some(want), Some(have))
                if want.image == have.image
                    && want.brightness == have.brightness
                    && want.rotation == have.rotation => {}
            (Some(want), _) => {
                commands.entity(rig.camera).try_insert(want.clone());
            }
            (None, Some(_)) => {
                commands.entity(rig.camera).try_remove::<Skybox>();
            }
            (None, None) => {}
        }

        // Fog: copied whole every frame. `FogFalloff` has no `PartialEq` to
        // compare against, and this camera is isolated, so the change flag it
        // raises reaches nothing but its own fog uniform.
        if let (Some(fog), Some(source_fog)) = (fog.as_deref_mut(), source_fog) {
            *fog = source_fog.clone();
        }

        // Material: the shading settings, the plane, and which view this
        // overlay belongs to.
        let source_pos = source_transform.translation();
        let params = OverlayParams {
            tint: reflection.tint.to_linear().to_vec4(),
            shading: Vec4::new(reflection.strength(), reflection.fresnel(), reflection.blur(), 0.0),
            normal: plane.normal.extend(0.0),
            // Mirror cameras sit at least `2 * FACING_EPSILON` from the
            // viewing camera, so this tolerance can never mistake one for the
            // other, while still absorbing f32 noise far from the origin.
            view_origin: source_pos.extend(1e-3 + 1e-5 * source_pos.length()),
            view_forward: source_transform.forward().as_vec3().extend(0.0),
        };
        let needs_update = materials
            .get(&rig.material)
            .is_none_or(|m| m.params != params || m.reflection != rig.image);
        if needs_update {
            if let Some(mut material) = materials.get_mut(&rig.material) {
                material.params = params;
                material.reflection = rig.image.clone();
            }
        }

        if let Ok((mut visibility, mut overlay_mesh, overlay_layers)) =
            overlays.get_mut(rig.overlay)
        {
            let wanted_visibility = if rig.rendered {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            visibility.set_if_neq(wanted_visibility);
            if let Some(mesh) = surface_mesh {
                if overlay_mesh.0 != mesh.0 {
                    overlay_mesh.0 = mesh.0.clone();
                }
            }
            if overlay_layers != surface_layers {
                match surface_layers {
                    Some(l) => {
                        commands.entity(rig.overlay).try_insert(l.clone());
                    }
                    None => {
                        commands.entity(rig.overlay).try_remove::<RenderLayers>();
                    }
                }
            }
        }
    }
}
