//! Runtime camera spawning and render target syncing.

use crate::{
    EditorCamera, EditorCamera2d, EditorLocked, HideInHierarchy, PrimaryViewportCamera,
    ViewportCamera, ViewportCamera2d,
};
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Camera, ClearColorConfig, RenderTarget};
use bevy::core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass, NormalPrepass};
use bevy::core_pipeline::Skybox;
use bevy::image::Image;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::light::{
    AtmosphereEnvironmentMapLight, EnvironmentMapLight, GeneratedEnvironmentMapLight,
};
#[cfg(feature = "render_3d")]
use bevy::pbr::{AtmosphereSettings, DistanceFog, FogFalloff};
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
    mut images: ResMut<Assets<Image>>,
) {
    use renzora::core::viewport_types::VIEWPORT_COUNT;

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
            // NOTE: `bevy::pbr::ContactShadows` is NOT attached here. Contact
            // shadows live at `mesh_view` binding 16; the *forward* opaque mesh
            // pipeline specializes the `CONTACT_SHADOWS` key (so its layout has
            // binding 16), but the deferred lighting pipeline does not — attach
            // it to a deferred camera and its bind group exposes binding 16 while
            // the pipeline's layout omits it, a wgpu hard-crash.
            // `ensure_contact_shadows_on_forward_cameras` (lib.rs) attaches it to
            // FORWARD cameras only, where it works; the `Sun`'s contact-shadow
            // toggle then takes effect there. (`seed_contact_shadows_offset`
            // closes the separate first-frame specialization race.)
        ));

        // Resident, no-op distance fog (3D only — bevy_pbr mesh-view binding 13).
        // Kept on the camera from spawn so binding 13 is always in PBR's layout;
        // the `WorldEnvironment` fog reconcile only *updates* it, never adds/removes
        // (toggling presence restructures the shared layout and crashes wgpu).
        #[cfg(feature = "render_3d")]
        entity.insert(DistanceFog {
            color: Color::NONE,
            directional_light_color: Color::NONE,
            directional_light_exponent: 8.0,
            falloff: FogFalloff::Exponential { density: 0.0 },
        });

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
            // 0.19: the `Atmosphere` itself lives on a separate stationary
            // "planet" entity managed by `renzora_atmosphere` (putting it on the
            // camera makes the sky rotate with the view). The camera only carries
            // `AtmosphereSettings` (the per-view render mode) + the IBL bake.
            entity.insert((
                PrimaryViewportCamera,
                EditorCamera,
                AtmosphereEnvironmentMapLight {
                    intensity: 0.0,
                    ..default()
                },
            ));
            // Per-view atmosphere render mode — bevy_pbr, 3D only.
            #[cfg(feature = "render_3d")]
            entity.insert(AtmosphereSettings::default());
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

/// Spawns the editor's 2D scene-navigation cameras — one orthographic
/// [`Camera2d`] per viewport slot, the 2D sibling of [`spawn_editor_camera`].
///
/// Each renders the 2D scene into its own slot's offscreen image with its own
/// independent pan/zoom, so viewports 2/3/4 show the 2D scene just as they show
/// the 3D scene (from a different framing) — not an empty panel. They start
/// inactive; `sync_viewport_camera_activation` in `renzora_viewport` activates
/// the docked slots' 2D cameras when `ViewportView::Two` is selected (and
/// deactivates those slots' 3D cameras, whose image the 2D camera takes over).
///
/// Only the *focused* slot's 2D camera carries the [`EditorCamera2d`] marker
/// (slot 0 to begin with; `relocate_editor_2d_marker` moves it with focus), so
/// the single-camera 2D picker / grid / overlay stack operates on the focused
/// view — the same focused-mirror trick the 3D cameras use.
pub fn spawn_editor_2d_camera(
    mut commands: Commands,
    mut viewports: ResMut<renzora::core::viewport_types::Viewports>,
) {
    use renzora::core::viewport_types::VIEWPORT_COUNT;

    for i in 0..VIEWPORT_COUNT {
        let slot_image = viewports.slots[i].image.clone();
        let mut entity = commands.spawn((
            Camera2d,
            Camera {
                // Render AFTER the slot's 3D camera (order -1). In 2D view the
                // primary slot's 3D camera stays active (it owns the
                // atmosphere/IBL probe, which panics if deactivated) and targets
                // this same image, so a higher order makes the 2D camera run
                // last; Camera2d clears the target first, so its output (grid +
                // sprites + tilemaps) replaces the 3D pass instead of fighting a
                // non-deterministic tie at the same order. (Secondary slots'
                // 3D cameras are deactivated in 2D view, so there the 2D camera
                // is the only one drawing anyway.)
                order: 0,
                // Explicit clear — NOT `ClearColorConfig::Default`. Default only
                // clears for the *first* camera on a target; layered after the
                // always-on primary 3D camera the 2D camera would otherwise
                // composite on top of the 3D render, so the 3D scene + its
                // infinite grid would show through and "fight" the 2D grid while
                // panning. A `Custom` clear wipes the 3D pass every frame.
                clear_color: ClearColorConfig::Custom(Color::srgb(0.11, 0.11, 0.13)),
                // Inactive until the user picks 2D view; otherwise the 2D and 3D
                // cameras would race for the shared slot image.
                is_active: false,
                ..default()
            },
            Transform::default(),
            // Bevy's default Camera2d carries `Msaa::Sample4`. Multisampling
            // does nothing useful for axis-aligned sprite quads, and it lets
            // edge pixels rasterize with EXTRAPOLATED UVs (sample inside the
            // quad, pixel centre outside) that overshoot a sprite's atlas
            // `rect` by well over the crop's anti-bleed inset — painting a
            // 1px line of the NEIGHBOURING atlas cell along tile/object edges
            // at fractional zoom. The runtime's offscreen cameras already
            // force this off (see `viewport_stretch.rs`); the editor 2D
            // cameras (which in-panel play renders through) need it too.
            Msaa::Off,
            ViewportCamera2d(i),
            // Layer 0 (the 2D scene — sprites, tilemaps, lights) plus this slot's
            // own grid layer, so each viewport renders only its own independent
            // grid mesh (built in `renzora_gizmo::grid_2d` framed to this slot's
            // zoom). A default `Camera2d` sees only layer 0, so adding this keeps
            // everything it saw before and adds the private grid layer.
            RenderLayers::from_layers(&[
                0,
                renzora::core::viewport_types::VIEWPORT_2D_GRID_LAYER_BASE + i,
            ]),
            HideInHierarchy,
            EditorLocked,
            Name::new(format!("Editor Camera 2D {i}")),
        ));

        // Slot 0 begins as the focused view, so it carries the `EditorCamera2d`
        // marker from spawn (keeps the many `With<EditorCamera2d>` singleton
        // queries valid before `relocate_editor_2d_marker` first runs).
        if i == 0 {
            entity.insert(EditorCamera2d);
        }

        // Slot images are created in `setup_viewport` (PostStartup), which runs
        // before this (`OnEnter(Editor)`), so they exist now. `sync_2d_camera_targets`
        // re-binds as a fallback if any weren't ready.
        if let Some(ref image) = slot_image {
            entity.insert(RenderTarget::Image(image.clone().into()));
        }

        viewports.slots[i].camera_2d_entity = Some(entity.id());
    }
}

// `LastSelectionForView2dSwitch` + `auto_switch_view_on_2d_selection` moved to
// the `renzora_engine_editor` crate (editor-only; they read `EditorSelection`,
// which is behind renzora's `editor` feature).

/// Pan + zoom controls for the editor 2D camera.
///
/// Only acts when `viewport_view == Two`, the cursor is over the viewport
/// panel, and we're in editing mode. Middle- or right-mouse drag pans (screen
/// pixels converted to world units via the orthographic scale so drag stays
/// 1:1 with the cursor at any zoom; right-drag is safe here — the 2D view has
/// no fly-camera or context menu on that button). Scroll wheel adjusts ortho
/// scale, translating the camera so the world point under the cursor stays
/// pinned to the cursor — Photoshop-style zoom-toward-cursor. Shift+scroll
/// pans vertically and Ctrl+scroll horizontally instead of zooming; a native
/// horizontal wheel (trackpads) always pans horizontally.
pub fn editor_2d_camera_controller(
    settings: Option<Res<ViewportSettings>>,
    viewport: Option<Res<ViewportState>>,
    play_mode: Option<Res<PlayModeState>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
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

    // Pan: middle- or right-mouse drag converts screen pixels to world units
    // via scale.
    if hovered
        && (mouse_button.pressed(MouseButton::Middle)
            || mouse_button.pressed(MouseButton::Right))
    {
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
    // With Shift (vertical) or Ctrl (horizontal) held the wheel pans instead.
    if hovered {
        let (mut wheel_x, mut wheel_y) = (0.0_f32, 0.0_f32);
        for ev in scroll_events.read() {
            wheel_x += ev.x;
            wheel_y += ev.y;
        }
        let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
        if shift || ctrl || wheel_x != 0.0 {
            let scale = match &*projection {
                Projection::Orthographic(o) => o.scale,
                _ => 1.0,
            };
            // Panel pixels per wheel notch, converted to world units so a
            // notch travels the same on-screen distance at any zoom.
            let step = 60.0 * scale;
            if shift {
                // Wheel up → view moves up.
                transform.translation.y += wheel_y * step;
            } else if ctrl {
                // Wheel up → view moves left (browser-style shift-wheel).
                transform.translation.x -= wheel_y * step;
            }
            // Native horizontal wheel (trackpad / tilt wheel) always pans.
            transform.translation.x += wheel_x * step;
            return;
        }
        let zoom = wheel_y;
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

/// Frame the project's game boundary to fill the panel the first time the 2D
/// view is shown, so the default view isn't stuck zoomed far out. Runs once per
/// session (tracked by a `Local`); afterwards the user's own pan/zoom sticks.
pub fn frame_2d_default(
    settings: Option<Res<ViewportSettings>>,
    viewport: Option<Res<ViewportState>>,
    project: Option<Res<renzora::core::CurrentProject>>,
    mut camera_query: Query<(&mut Transform, &mut Projection), With<EditorCamera2d>>,
    mut framed: Local<bool>,
) {
    if *framed {
        return;
    }
    if settings.map(|s| s.viewport_view).unwrap_or_default() != ViewportView::Two {
        return;
    }
    let Some(vs) = viewport else { return };
    let img = vs.current_size.as_vec2();
    if img.x < 2.0 || img.y < 2.0 {
        return;
    }
    let Some(project) = project else { return };
    let w = project.config.viewport.width.max(1) as f32;
    let h = project.config.viewport.height.max(1) as f32;
    let Ok((mut transform, mut projection)) = camera_query.single_mut() else {
        return;
    };
    let Projection::Orthographic(o) = projection.as_mut() else {
        return;
    };
    // Ortho scale is world-units per image-pixel, so scale = boundary / image on
    // the tighter axis fits the whole boundary; ×1.08 leaves an 8% margin.
    let fit = (w / img.x).max(h / img.y) * 1.08;
    o.scale = fit;
    // viewport_origin is top-left, so the camera translation is the world point
    // at the image's top-left corner — offset it so the boundary is centred.
    let visible = img * fit;
    transform.translation.x = w * 0.5 - visible.x * 0.5;
    transform.translation.y = -h * 0.5 + visible.y * 0.5;
    *framed = true;
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

/// Bind each 2D viewport camera to its own slot's offscreen image, mirroring
/// [`sync_viewport_camera_targets`] for the orthographic siblings.
///
/// The slot images are created once (in `setup_viewport`) and only resized in
/// place, so we bind exactly once — when every 2D camera and every image exist —
/// then idle. Resizing keeps the handle, so it needs no rebind. (`spawn_editor_2d_camera`
/// already binds at spawn when the images are ready; this is the fallback for
/// any that weren't.)
pub fn sync_2d_camera_targets(
    viewports: Res<renzora::core::viewport_types::Viewports>,
    cameras: Query<(Entity, &ViewportCamera2d)>,
    mut bound: Local<bool>,
    mut commands: Commands,
) {
    use renzora::core::viewport_types::VIEWPORT_COUNT;
    if *bound {
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
        *bound = true;
    }
}

/// Keep each 2D viewport camera on its own independent pan/zoom.
///
/// The focused slot's 2D camera carries [`EditorCamera2d`] and is driven live by
/// [`editor_2d_camera_controller`] / [`frame_2d_default`]; this system persists
/// that framing into its slot, then drives every *other* slot's 2D camera from
/// its own stored framing so the views can't converge. A slot that's never been
/// framed (`zoom_2d == 0`) inherits the focused view's framing the first time
/// it's shown, then diverges — so opening viewport 2 lands on the same view as
/// viewport 1 rather than an empty corner, and pans away from there.
///
/// The write-back slot is keyed off the marked camera's **own** [`ViewportCamera2d`]
/// index, NOT `Viewports.focused` — this is the 2D twin of the 3D
/// `OrbitMirror` focus-race fix. `relocate_editor_2d_marker` runs in PreUpdate
/// while `resolve_viewport_slots` updates `focused` in Update, so on a frame
/// where the cursor drifts to another viewport (common mid-zoom) the marker
/// still sits on the previous slot's camera while `focused` already points at
/// the new one. Reading the marker's index instead keeps the mirror
/// self-consistent — it can never copy one view's framing into another slot.
///
/// Runs after the 2D controller so it mirrors the latest focused edits, and the
/// two camera-sets are disjoint (`With` vs `Without<EditorCamera2d>`), so no
/// camera is both read live and overwritten in the same frame.
pub fn sync_2d_viewport_cameras(
    mut viewports: ResMut<renzora::core::viewport_types::Viewports>,
    focused_cam: Query<(&ViewportCamera2d, &Transform, &Projection), With<EditorCamera2d>>,
    mut other_cams: Query<
        (&ViewportCamera2d, &mut Transform, &mut Projection),
        Without<EditorCamera2d>,
    >,
) {
    // Mirror the marked camera's live framing into ITS OWN slot, and snapshot
    // that framing to seed any viewport that's never been framed yet.
    let mut seed = None;
    if let Ok((vc, transform, Projection::Orthographic(ortho))) = focused_cam.single() {
        if let Some(slot) = viewports.slots.get_mut(vc.0) {
            slot.pan_2d = transform.translation.truncate();
            slot.zoom_2d = ortho.scale;
            seed = Some((slot.pan_2d, slot.zoom_2d));
        }
    }

    // Drive every non-focused 2D camera from its own stored framing.
    for (vc, mut transform, mut projection) in other_cams.iter_mut() {
        let Some(slot) = viewports.slots.get_mut(vc.0) else {
            continue;
        };
        if slot.zoom_2d <= 0.0 {
            let Some((pan, zoom)) = seed else { continue };
            slot.pan_2d = pan;
            slot.zoom_2d = zoom;
        }
        transform.translation.x = slot.pan_2d.x;
        transform.translation.y = slot.pan_2d.y;
        if let Projection::Orthographic(ortho) = projection.as_mut() {
            ortho.scale = slot.zoom_2d;
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
