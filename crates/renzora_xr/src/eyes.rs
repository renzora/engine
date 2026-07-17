//! Fully-decorated XR eye cameras.
//!
//! `bevy_mod_openxr` normally spawns each eye as a bare
//! `(RenderTarget, XrCamera, Projection)` — which renders, but with none of
//! the engine's camera stack: no HDR, no prepasses, no fog slot, no
//! atmosphere/IBL bindings. And those can't be added later: Bevy locks a
//! camera's mesh-view bind-group layout at its FIRST render, so a camera that
//! renders once without e.g. `AtmosphereSettings` crashes wgpu if it gains it
//! afterwards ("20 vs 23 bindings" — see `renzora_engine::camera`'s notes).
//!
//! So `xr_plugins` sets `OxrRenderPlugin { spawn_cameras: false }` (the
//! swapchain texture views still get registered) and this module spawns the
//! eyes itself on `XrSessionCreated`, carrying the same decoration the engine
//! puts on a secondary viewport camera at spawn: HDR + `Msaa::Off`, the
//! prepass triple, a resident no-op `DistanceFog`, a placeholder-maps
//! `EnvironmentMapLight` (IBL slots reserved; real maps shared in by
//! `environment::share_ibl_to_xr_cameras`), and `AtmosphereSettings` so the
//! procedural sky renders in-headset. Every other XR system adopts the
//! cameras by their `XrCamera(i)` marker, exactly as if the backend had
//! spawned them (`DeferredPrepass` arrives via the engine's blanket
//! `ensure_deferred_prepass_on_cameras` the same frame, before first render).

use bevy::camera::{Hdr, ManualTextureViewHandle, RenderTarget};
use bevy::core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass, NormalPrepass};
use bevy::light::EnvironmentMapLight;
use bevy::pbr::{AtmosphereSettings, DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
    TextureViewDimension,
};
use bevy_mod_openxr::render::XR_TEXTURE_INDEX;
use bevy_mod_xr::camera::{XrCamera, XrProjection, XrViewInit};
use bevy_mod_xr::session::XrSessionCreated;

pub(crate) fn register(app: &mut App) {
    // After XrViewInit so the swapchain's ManualTextureViews exist (the
    // handles are deterministic constants either way; update_cameras re-points
    // targets every frame).
    app.add_systems(XrSessionCreated, spawn_eye_cameras.after(XrViewInit));
}

fn spawn_eye_cameras(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    existing: Query<(), With<XrCamera>>,
) {
    // Sessions can be created repeatedly per app run (every VR play in the
    // editor); only spawn when the previous session's eyes are gone.
    if !existing.is_empty() {
        return;
    }
    let placeholder_cube = make_placeholder_cube(&mut images);

    for index in 0..2u32 {
        commands.spawn((
            XrCamera(index),
            RenderTarget::TextureView(ManualTextureViewHandle(XR_TEXTURE_INDEX + index)),
            Projection::custom(XrProjection::default()),
            Name::new(format!("XR Eye {index}")),
            Hdr,
            // Atmosphere/sky binds depth non-multisampled — same reason the
            // editor cameras run MSAA off.
            Msaa::Off,
            (NormalPrepass, DepthPrepass, MotionVectorPrepass),
            // Resident no-op fog: keeps the fog binding in the layout so the
            // engine's distance-fog systems can drive it live.
            DistanceFog {
                color: Color::NONE,
                directional_light_color: Color::NONE,
                directional_light_exponent: 8.0,
                falloff: FogFalloff::Exponential { density: 0.0 },
            },
            // IBL slots reserved from spawn (placeholder maps, zero
            // intensity); the primary viewport's baked maps are shared in
            // each frame once the session runs.
            EnvironmentMapLight {
                diffuse_map: placeholder_cube.clone(),
                specular_map: placeholder_cube.clone(),
                intensity: 0.0,
                rotation: Quat::IDENTITY,
                affects_lightmapped_mesh_diffuse: true,
            },
            AtmosphereSettings::default(),
        ));
    }
    info!("[XR] spawned decorated eye cameras (env/prepass/fog/IBL slots)");
}

/// 1-texel cubemap standing in until the real IBL maps are shared in —
/// mirrors `renzora_engine::camera::make_placeholder_cube` (secondary
/// viewport cameras use the same trick to keep IBL bind slots stable).
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
