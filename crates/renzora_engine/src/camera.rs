//! Runtime camera spawning and render target syncing.

use crate::{
    EditorCamera, EditorCamera2d, EditorLocked, HideInHierarchy, PrimaryViewportCamera,
    ViewportCamera, ViewportRenderTarget,
};
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Camera, RenderTarget};
use bevy::core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass, NormalPrepass};
use bevy::core_pipeline::Skybox;
use bevy::image::Image;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::light::atmosphere::ScatteringMedium;
use bevy::light::{
    Atmosphere, AtmosphereEnvironmentMapLight, EnvironmentMapLight, GeneratedEnvironmentMapLight,
};
use bevy::pbr::AtmosphereSettings;
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
    TextureViewDimension,
};
use bevy::camera::Hdr;
use renzora::core::viewport_types::{ViewportSettings, ViewportState, ViewportView};
use renzora::core::PlayModeState;
use renzora::viewport_types::EditorCameraMatrix;

/// Spawns the editor's 3D scene-navigation camera.
///
/// If `ViewportRenderTarget` already has an image (editor mode),
/// the camera renders to it. Otherwise it renders to the window.
/// The camera is hidden from the hierarchy and locked from editing.
///
/// Render-effect components (`Atmosphere`, `AtmosphereSettings`,
/// `AtmosphereEnvironmentMapLight`, `Msaa::Off`, etc.) are attached at
/// spawn so Bevy 0.18's atmosphere/sky pipeline can lock in its bind
/// group layout once and never need to grow it. Trying to add atmosphere
/// at runtime crashes wgpu with "20 vs 23 bindings" — Bevy specializes
/// the layout per-camera at first render, and atmosphere bindings are
/// gated on whether the component existed at that moment.
///
/// `EffectRouting` + `renzora_atmosphere::sync_atmosphere` then *update*
/// these components in-place from a `WorldEnvironment` source entity (or
/// any entity with `AtmosphereComponentSettings`), giving us one logical
/// source of truth that drives both editor and play cameras identically.
/// The plugin replaces, never removes — see its file for the why.
pub fn spawn_editor_camera(
    mut commands: Commands,
    mut viewports: ResMut<renzora::core::viewport_types::Viewports>,
    mut mediums: ResMut<Assets<ScatteringMedium>>,
    mut images: ResMut<Assets<Image>>,
) {
    use renzora::core::viewport_types::VIEWPORT_COUNT;

    // One ScatteringMedium asset shared by every viewport camera's atmosphere.
    let default_medium = mediums.add(ScatteringMedium::default());

    // Valid placeholder cubemap so the secondary cameras can carry an
    // `EnvironmentMapLight` from spawn (the IBL bind-group slots can't be added
    // at runtime). `share_ibl_to_secondary_viewports` swaps these handles for
    // the primary's real prefiltered maps once the bake is ready.
    let placeholder_cube = make_placeholder_cube(&mut images);

    for i in 0..VIEWPORT_COUNT {
        let slot = &viewports.slots[i];
        let transform = orbit_transform(slot.focus, slot.distance, slot.yaw, slot.pitch);
        let slot_image = slot.image.clone();

        let mut entity = commands.spawn((
            Camera3d::default(),
            Camera {
                order: -1,
                // Secondary views start inactive; the panel-visibility gate in
                // `renzora_viewport` activates each one when its panel is docked.
                is_active: i == 0,
                ..default()
            },
            Projection::Perspective(PerspectiveProjection {
                far: 100_000.0,
                ..default()
            }),
            transform,
            ViewportCamera(i),
            HideInHierarchy,
            EditorLocked,
            RenderLayers::from_layers(&[0, 1]),
            Name::new(format!("Editor Camera {i}")),
            Hdr,
            // Atmosphere/sky binds depth as non-multisampled (binding 13);
            // any MSAA on the same camera trips a wgpu validation crash.
            Msaa::Off,
            // Force the prepass to carry world normals, depth, and motion
            // vectors. NormalPrepass is required for `pbr_input_from_vertex_output`
            // to compile against `alpha_mode = Mask` materials. DepthPrepass +
            // MotionVectorPrepass are required for SSGI / Lumen `ScreenSpace`
            // temporal reprojection. Bevy 0.18 specializes the prepass pipeline
            // once on first render; all three must be present at spawn — adding
            // any later trips a wgpu validation crash on the prepass attachment
            // list. (TAA also auto-attaches MotionVectorPrepass; doing it here
            // means the layout doesn't change when the user toggles TAA.)
            //
            // `DeferredPrepass` (and a matching `Msaa::Off`) is attached
            // by `ensure_deferred_prepass_on_cameras` in `PostUpdate` when
            // the resolved rendering mode is Deferred. That same system
            // covers every other `Camera3d` in the editor (previews,
            // thumbnails, etc.), so we don't special-case it here.
            (NormalPrepass, DepthPrepass, MotionVectorPrepass),
        ));

        // Only the PRIMARY camera carries the procedural sky + IBL. In Bevy
        // 0.18 the `Atmosphere` component makes a camera's mesh-view layout
        // expect the environment-map (IBL) bindings, and the per-camera IBL
        // bake can't be duplicated (four bakes race → "26 vs 29" wgpu crash) or
        // toggled at runtime — so sky and IBL are an inseparable, single-camera
        // unit. The extra views are lightweight 3D angles (prepass + HDR). True
        // sharing across views requires rendering the atmosphere to one shared
        // cubemap and feeding it back as a Skybox + EnvironmentMapLight; see the
        // multi-viewport notes.
        // Only the PRIMARY camera carries the atmosphere + IBL bake (one bake,
        // shared out). Secondary views carry an `EnvironmentMapLight` from spawn
        // (placeholder maps + zero intensity — invisible, but keeps the IBL
        // bind-group slots stable since they can't be added at runtime);
        // `share_ibl_to_secondary_viewports` swaps in the primary's prefiltered
        // maps, and `share_sky_to_secondary_viewports` gives them the primary's
        // baked cubemap as a Skybox.
        if i == 0 {
            entity.insert((
                PrimaryViewportCamera,
                EditorCamera,
                Atmosphere {
                    inner_radius: 6_360_000.0,
                    outer_radius: 6_460_000.0,
                    ground_albedo: Vec3::splat(0.3),
                    medium: default_medium.clone(),
                },
                AtmosphereSettings::default(),
                AtmosphereEnvironmentMapLight {
                    intensity: 0.0,
                    ..default()
                },
            ));
        } else {
            entity.insert(EnvironmentMapLight {
                diffuse_map: placeholder_cube.clone(),
                specular_map: placeholder_cube.clone(),
                intensity: 0.0,
                rotation: Quat::IDENTITY,
                affects_lightmapped_mesh_diffuse: true,
            });
        }

        if let Some(ref image) = slot_image {
            entity.insert(RenderTarget::Image(image.clone().into()));
        }

        let id = entity.id();
        viewports.slots[i].camera_entity = Some(id);
    }

    info!("[camera] Spawned {VIEWPORT_COUNT} editor viewport cameras (primary bakes the shared environment)");
}

/// Create a tiny valid Rgba16Float cubemap to seed the secondary cameras'
/// `EnvironmentMapLight` at spawn. It's only ever displayed for the frame or two
/// before [`share_ibl_to_secondary_viewports`] swaps in the primary's real
/// prefiltered maps, so 1×1×6 is plenty.
fn make_placeholder_cube(images: &mut Assets<Image>) -> Handle<Image> {
    // 1 texel × 6 faces × 8 bytes (Rgba16Float = 4×f16).
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

/// Share the primary's prefiltered IBL (diffuse + specular environment maps,
/// produced once by its atmosphere bake) with every other viewport camera, so
/// all views are lit by the same environment — no per-camera bake. The maps are
/// shared as handles (one set of textures in VRAM); only intensity/rotation are
/// copied. Runs when the primary's `EnvironmentMapLight` changes.
pub fn share_ibl_to_secondary_viewports(
    primary: Query<Ref<EnvironmentMapLight>, With<PrimaryViewportCamera>>,
    mut secondary: Query<
        &mut EnvironmentMapLight,
        (With<ViewportCamera>, Without<PrimaryViewportCamera>),
    >,
) {
    let Ok(source) = primary.single() else {
        return;
    };
    if !source.is_changed() {
        return;
    }
    for mut env in secondary.iter_mut() {
        env.diffuse_map = source.diffuse_map.clone();
        env.specular_map = source.specular_map.clone();
        env.intensity = source.intensity;
        env.rotation = source.rotation;
        env.affects_lightmapped_mesh_diffuse = source.affects_lightmapped_mesh_diffuse;
    }
}

/// Brightness multiplier for the shared-sky `Skybox` on the secondary viewport
/// cameras. The primary's baked atmosphere cubemap is HDR radiance; this scales
/// it into the cd/m² the skybox pass expects. TUNABLE — if the extra views'
/// sky comes out too dark or blown out vs. the primary, this is the knob.
const SHARED_SKY_BRIGHTNESS: f32 = 1.0;

/// Fan the primary camera's baked atmosphere cubemap out to the other viewport
/// cameras as a `Skybox`, so every view shows the same sky from the single bake.
/// The primary renders its sky through its own `Atmosphere` pass. `Skybox` is a
/// standalone render pass (not a mesh-view binding), so attaching it at runtime
/// is safe. We only (re)insert when the cubemap handle changes.
pub fn share_sky_to_secondary_viewports(
    primary: Query<&GeneratedEnvironmentMapLight, With<PrimaryViewportCamera>>,
    secondary: Query<
        (Entity, Option<&Skybox>),
        (With<ViewportCamera>, Without<PrimaryViewportCamera>),
    >,
    mut commands: Commands,
) {
    let Ok(generated) = primary.single() else {
        return;
    };
    let image = &generated.environment_map;
    for (entity, skybox) in &secondary {
        // Bevy 0.19: `Skybox.image` is now `Option<Handle<Image>>`.
        let up_to_date = skybox.is_some_and(|s| s.image.as_ref() == Some(image));
        if !up_to_date {
            commands.entity(entity).insert(Skybox {
                image: Some(image.clone()),
                brightness: SHARED_SKY_BRIGHTNESS,
                rotation: Quat::IDENTITY,
            });
        }
    }
}

/// Compute a look-at transform from orbit parameters. Mirrors
/// `renzora_camera::OrbitCameraState::calculate_transform` but lives here so
/// the engine crate doesn't depend on the camera crate.
pub fn orbit_transform(focus: Vec3, distance: f32, yaw: f32, pitch: f32) -> Transform {
    let pos = focus
        + Vec3::new(
            distance * pitch.cos() * yaw.sin(),
            distance * pitch.sin(),
            distance * pitch.cos() * yaw.cos(),
        );
    Transform::from_translation(pos).looking_at(focus, Vec3::Y)
}

/// Spawns the editor's 2D scene-navigation camera.
///
/// Sibling of [`spawn_editor_camera`] — orthographic camera that renders
/// to the same viewport offscreen image. Starts inactive; the
/// `sync_viewport_camera_activation` system in `renzora_viewport` toggles
/// it active when `ViewportSettings.viewport_view == ViewportView::Two`.
///
/// Only one of the two editor cameras is ever active at a time, so they
/// can safely share the render target.
pub fn spawn_editor_2d_camera(mut commands: Commands, render_target: Res<ViewportRenderTarget>) {
    let mut entity = commands.spawn((
        Camera2d,
        Camera {
            // Match the 3D editor camera's order so cycling between views
            // doesn't change z-stacking against any other cameras (e.g. UI).
            order: -1,
            // Inactive until the user picks the 2D viewport view; otherwise
            // both editor cameras would race for the offscreen target.
            is_active: false,
            ..default()
        },
        Transform::default(),
        EditorCamera2d,
        HideInHierarchy,
        EditorLocked,
        Name::new("Editor Camera 2D"),
    ));

    if let Some(ref image) = render_target.image {
        entity.insert(RenderTarget::Image(image.clone().into()));
    }
}

// `LastSelectionForView2dSwitch` + `auto_switch_view_on_2d_selection` moved to
// the `renzora_engine_editor` crate (editor-only; they read `EditorSelection`,
// which is behind renzora's `editor` feature).

/// Pan + zoom controls for the editor 2D camera.
///
/// Only acts when `viewport_view == Two`, the cursor is over the viewport
/// panel, and we're in editing mode. Middle-mouse drag pans (screen pixels
/// converted to world units via the orthographic scale so drag stays
/// 1:1 with the cursor at any zoom). Scroll wheel adjusts ortho scale,
/// translating the camera so the world point under the cursor stays
/// pinned to the cursor — Photoshop-style zoom-toward-cursor.
pub fn editor_2d_camera_controller(
    settings: Option<Res<ViewportSettings>>,
    viewport: Option<Res<ViewportState>>,
    play_mode: Option<Res<PlayModeState>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut scroll_events: MessageReader<MouseWheel>,
    windows: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    mut camera_query: Query<(&Camera, &mut Transform, &mut Projection), With<EditorCamera2d>>,
) {
    let in_play = play_mode.is_some_and(|pm| pm.is_in_play_mode());
    let view = settings.map(|s| s.viewport_view).unwrap_or_default();
    if in_play || view != ViewportView::Two {
        mouse_motion.clear();
        scroll_events.clear();
        return;
    }

    let hovered = viewport.as_ref().is_some_and(|v| v.hovered);
    let Ok((camera, mut transform, mut projection)) = camera_query.single_mut() else {
        mouse_motion.clear();
        scroll_events.clear();
        return;
    };

    // Pan: middle-mouse drag converts screen pixels to world units via scale.
    if hovered && mouse_button.pressed(MouseButton::Middle) {
        let mut delta = Vec2::ZERO;
        for ev in mouse_motion.read() {
            delta += ev.delta;
        }
        if delta != Vec2::ZERO {
            let scale = match &*projection {
                Projection::Orthographic(o) => o.scale,
                _ => 1.0,
            };
            transform.translation.x -= delta.x * scale;
            // Screen y increases downward, world y increases upward.
            transform.translation.y += delta.y * scale;
        }
    } else {
        mouse_motion.clear();
    }

    // Zoom: scroll wheel. Each notch is 10% in/out, clamped to a
    // generous range so the camera doesn't disappear from extreme edits.
    if hovered {
        let mut zoom = 0.0_f32;
        for ev in scroll_events.read() {
            zoom += ev.y;
        }
        if zoom != 0.0 {
            if let Projection::Orthographic(ref mut o) = *projection {
                // Capture cursor world position BEFORE scale change so we
                // can adjust the camera translation to keep that world
                // point under the cursor.
                let cursor_world_before = viewport.as_deref().and_then(|vs| {
                    let window = windows.single().ok()?;
                    let cursor = window.cursor_position()?;
                    let in_rect = cursor - vs.screen_position;
                    if in_rect.x < 0.0
                        || in_rect.y < 0.0
                        || in_rect.x >= vs.screen_size.x
                        || in_rect.y >= vs.screen_size.y
                    {
                        return None;
                    }
                    let image_size = vs.current_size.as_vec2();
                    if image_size.x <= 0.0 || image_size.y <= 0.0 {
                        return None;
                    }
                    let scaled = Vec2::new(
                        in_rect.x * image_size.x / vs.screen_size.x,
                        in_rect.y * image_size.y / vs.screen_size.y,
                    );
                    let cam_gt = GlobalTransform::from(*transform);
                    camera.viewport_to_world_2d(&cam_gt, scaled).ok()
                });

                let old_scale = o.scale;
                let step: f32 = 0.9;
                o.scale = (o.scale * step.powf(zoom)).clamp(0.01, 1000.0);
                let new_scale = o.scale;

                // Translate so the captured world point stays pinned to
                // the cursor. The new world point at the same cursor
                // image-pixel scales linearly with the projection scale,
                // so `delta_cam = (cursor_world - cam) * (1 - new/old)`.
                if let Some(cursor_world) = cursor_world_before {
                    if old_scale > 0.0 {
                        let cam_xy = transform.translation.truncate();
                        let offset = cursor_world - cam_xy;
                        let zoom_factor = new_scale / old_scale;
                        transform.translation.x += offset.x * (1.0 - zoom_factor);
                        transform.translation.y += offset.y * (1.0 - zoom_factor);
                    }
                }
            }
        }
    } else {
        scroll_events.clear();
    }
}

/// Observer: every time a `Camera2d` is inserted (preset spawn, scene
/// reflection load, runtime spawn), shift the projection's
/// `viewport_origin` so the camera's transform position maps to the
/// **top-left** of the rendered viewport.
///
/// This keeps Bevy's 2D world consistent with our Godot-style
/// convention: the project's window-area outline is anchored at world
/// (0, 0) extending to (width, -height), and a Camera 2D at world
/// origin renders that area exactly. Default Bevy `viewport_origin`
/// is `(0.5, 0.5)` (centred), which would render world (0, 0) at the
/// middle of the runtime window — sprites authored against the
/// outline would appear off-centre.
pub fn on_camera_2d_inserted(
    trigger: On<Insert, Camera2d>,
    mut projections: Query<&mut Projection>,
) {
    let entity = trigger.entity;
    if let Ok(mut projection) = projections.get_mut(entity) {
        if let Projection::Orthographic(ortho) = projection.as_mut() {
            ortho.viewport_origin = Vec2::new(0.0, 1.0);
        }
    }
}

/// Companion observer: when `Projection` itself is inserted on an
/// entity that already carries `Camera2d`, re-apply the top-left
/// `viewport_origin`. Reflection-based scene loads insert `Camera2d`
/// first (which fires `on_camera_2d_inserted` and fixes the projection)
/// *and then* deserialize the saved `Projection` on top — overwriting
/// the fix with whatever was on disk. This catches that second insert
/// so legacy scenes converge to the Godot-style convention as soon as
/// they load. No-op for `Camera3d` and any other camera kind.
pub fn on_projection_inserted_for_2d(
    trigger: On<Insert, Projection>,
    cameras_2d: Query<(), With<Camera2d>>,
    mut projections: Query<&mut Projection>,
) {
    let entity = trigger.entity;
    if cameras_2d.get(entity).is_err() {
        return;
    }
    if let Ok(mut projection) = projections.get_mut(entity) {
        if let Projection::Orthographic(ortho) = projection.as_mut() {
            ortho.viewport_origin = Vec2::new(0.0, 1.0);
        }
    }
}

/// Watches for changes to `ViewportRenderTarget` and points the editor 2D
/// camera at the primary viewport's offscreen image.
///
/// The 3D viewport cameras are handled per-slot by
/// [`sync_viewport_camera_targets`]; the 2D camera shares the primary slot's
/// image (it's a mode of the primary viewport, mutually exclusive with the
/// primary 3D camera).
pub fn sync_camera_render_target(
    render_target: Res<ViewportRenderTarget>,
    cameras_2d: Query<Entity, With<EditorCamera2d>>,
    mut commands: Commands,
) {
    if !render_target.is_changed() {
        return;
    }

    if let Some(ref image) = render_target.image {
        for entity in cameras_2d.iter() {
            commands
                .entity(entity)
                .insert(RenderTarget::Image(image.clone().into()));
        }
    }
}

/// Set once every 3D viewport camera has been pointed at its slot image.
#[derive(Resource, Default)]
pub struct ViewportTargetsBound(pub bool);

/// Assigns each 3D viewport camera its own slot render-target image.
///
/// The slot images are created once (in `setup_viewport`) and only resized in
/// place afterwards, so we bind targets exactly once — when all cameras and all
/// images exist — then idle. Resizing keeps the handle, so it needs no rebind.
pub fn sync_viewport_camera_targets(
    viewports: Res<renzora::core::viewport_types::Viewports>,
    cameras: Query<(Entity, &ViewportCamera)>,
    mut bound: ResMut<ViewportTargetsBound>,
    mut commands: Commands,
) {
    use renzora::core::viewport_types::VIEWPORT_COUNT;
    if bound.0 {
        return;
    }
    let mut all_ready = true;
    for (entity, vc) in cameras.iter() {
        match viewports.slots.get(vc.0).and_then(|s| s.image.as_ref()) {
            Some(image) => {
                commands
                    .entity(entity)
                    .insert(RenderTarget::Image(image.clone().into()));
            }
            None => all_ready = false,
        }
    }
    if all_ready && cameras.iter().count() == VIEWPORT_COUNT {
        bound.0 = true;
        info!("[camera] All {VIEWPORT_COUNT} viewport cameras bound to render targets");
    }
}

/// Cache the editor camera's clip-from-world matrix into a resource each frame,
/// so overlay systems (grid, gizmos) can CPU-project geometry without querying
/// the camera themselves (which requires mutable World access).
pub fn update_editor_camera_matrix(
    cameras: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    mut mat: ResMut<EditorCameraMatrix>,
) {
    let Ok((camera, transform)) = cameras.single() else {
        mat.valid = false;
        return;
    };
    let view_from_world = transform.affine().inverse();
    let clip_from_view = camera.clip_from_view();
    mat.clip_from_world = clip_from_view * Mat4::from(view_from_world);
    mat.world_from_clip = mat.clip_from_world.inverse();
    mat.cam_pos = transform.translation();
    mat.cam_forward = *transform.forward();
    mat.valid = true;
}
