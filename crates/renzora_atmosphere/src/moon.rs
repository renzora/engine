//! The moon in the night sky: a lit, surfaced sphere where the moonlight comes
//! from, with a glow around it.
//!
//! Bevy's `SunDisk` draws a flat disc where a directional light points, which
//! was the moon until it needed a surface: it has no texture and no phase. So
//! the moon is drawn here as a real sphere with its own shader (`moon.wesl`),
//! placed where `Sun::moon_direction` puts the moonlight.
//!
//! # Phase
//!
//! The moon is not lit by the scene's sun. Its sky position is chosen for the
//! scene's lighting (opposite the sun, held well above the horizon), which
//! would make it full every night. Instead the shading light is built around
//! the viewer from the phase angle `Sun::moon_phase_angle` gives for the day
//! of the lunar month: straight from behind the viewer at full, from the side
//! at a quarter, from behind the moon at new. Waxing moons are lit on their
//! right, waning on their left, as seen from the northern hemisphere.
//!
//! # Glow and eclipse
//!
//! The glow is a second, larger sphere around the moon whose shader adds a
//! halo falling off with angular distance from the moon's centre. An eclipse
//! (`Sun::moon_eclipse`) slides the Earth's shadow across the face: grey at
//! its soft edge, dim copper red inside, which is a blood moon at totality.
//!
//! Both follow the viewing camera at a fixed distance inside the star dome (so
//! the moon hides the stars behind it) and inside a camera's far plane. Like
//! the cloud and star domes they are camera-centred chrome: hidden from the
//! hierarchy, casting no shadows, never saved.

use bevy::camera::visibility::RenderLayers;
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use renzora_world_api::Sun;

/// How far from the camera the moon is drawn, in metres. Inside the star
/// dome (800 m) and inside the scene camera's 1000 m far plane.
const MOON_DISTANCE: f32 = 700.0;

/// The glow sphere's radius, in moon radii. The halo has faded to nothing
/// well before its edge.
const GLOW_EXTENT: f32 = 10.0;

/// Luminance of a fully lit, average patch of the moon per lux of moonlight,
/// in nits. At the default 150 lux that is 1200 nits, several stops above the
/// moonlit street as a real moon is: at 0.6 it read as a dull grey disc. The
/// maria and craters still show through the tonemapper's highlight roll-off.
const NITS_PER_LUX: f32 = 8.0;

/// The same for the glow, kept apart so a brighter face does not also swell
/// the halo into a fog.
const GLOW_NITS_PER_LUX: f32 = 1.2;

#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub struct MoonUniform {
    /// `xyz` = unit vector toward the light that shades the face (see the
    /// module doc on phase), `w` = lit luminance in nits.
    pub light: Vec4,
    /// `rgb` = face tint, `a` = fade (0 hidden, 1 risen).
    pub tint: Vec4,
    /// `xyz` = the moon's right on screen, `w` = eclipse 0 to 1.
    pub right: Vec4,
    /// `xyz` = the moon's up on screen, `w` = glow strength.
    pub up: Vec4,
    /// `xyz` = the moon's centre in world space, `w` = its angular radius.
    pub centre: Vec4,
    /// `x` = 1 for the glow sphere, 0 for the moon; `y` = lit fraction;
    /// `z` = the glow's luminance in nits.
    pub params: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct MoonMaterial {
    #[uniform(0)]
    pub uniform: MoonUniform,
    /// The glow is added to the sky; the moon is opaque and hides the stars.
    pub glow: bool,
}

impl Material for MoonMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("embedded://renzora_atmosphere/moon.wesl".into())
    }

    fn alpha_mode(&self) -> AlphaMode {
        if self.glow { AlphaMode::Add } else { AlphaMode::Opaque }
    }
}

#[derive(Component)]
struct MoonMarker;

#[derive(Resource, Default)]
struct MoonState {
    body: Option<(Entity, Handle<MoonMaterial>)>,
    glow: Option<(Entity, Handle<MoonMaterial>)>,
}

pub(crate) fn build(app: &mut App) {
    bevy::asset::embedded_asset!(app, "moon.wesl");
    app.add_plugins(MaterialPlugin::<MoonMaterial>::default())
        .init_resource::<MoonState>()
        .add_systems(Update, sync_moon);
}

/// What the shader needs for this sun, seen from `camera`, or `None` while
/// the moon is down.
fn moon_uniform(sun: &Sun, camera: Vec3) -> Option<(MoonUniform, Vec3, f32)> {
    let fade = if sun.moon_illuminance > 0.0 { sun.moonrise() } else { 0.0 };
    if fade <= 0.0 {
        return None;
    }
    // The moonlight travels along `moon_direction`, so the moon is behind it.
    let toward_moon = -sun.moon_direction();
    let centre = camera + toward_moon * MOON_DISTANCE;
    let angular_radius = (sun.moon_size.max(0.05) * 0.5).to_radians();
    let radius = MOON_DISTANCE * angular_radius.tan();

    // Screen axes on the disc: right as a camera with world up sees it.
    let right = toward_moon.cross(Vec3::Y).normalize_or(Vec3::X);
    let up = right.cross(toward_moon).normalize_or(Vec3::Y);
    // The shading light, swung round from behind the viewer by the phase
    // angle: toward the viewer at full, to the right while waxing.
    let to_viewer = -toward_moon;
    let alpha = sun.moon_phase_angle();
    let light = (to_viewer * alpha.cos() + right * alpha.sin()).normalize_or(to_viewer);

    let uniform = MoonUniform {
        light: light.extend(sun.moon_illuminance * NITS_PER_LUX),
        tint: sun.moon_tint.max(Vec3::ZERO).extend(fade),
        right: right.extend(sun.moon_eclipse.clamp(0.0, 1.0)),
        up: up.extend(sun.moon_glow.max(0.0)),
        centre: centre.extend(angular_radius),
        params: Vec4::new(0.0, sun.moon_lit_fraction(), sun.moon_illuminance * GLOW_NITS_PER_LUX, 0.0),
    };
    Some((uniform, centre, radius))
}

/// Spawn, place and light the moon and its glow while the moon is up; remove
/// them when it is not.
fn sync_moon(
    mut commands: Commands,
    mut state: ResMut<MoonState>,
    suns: Query<&Sun>,
    cameras: Query<(&GlobalTransform, &Camera, Option<&RenderLayers>), With<Camera3d>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<MoonMaterial>>,
) {
    // The brightest sun, as the clouds and the glare choose theirs.
    let sun = suns.iter().max_by(|a, b| a.illuminance.total_cmp(&b.illuminance));

    // The camera that sees the scene: active and on the default layer. The
    // editor's offscreen preview rigs sit on their own layers.
    let on_default = |layers: Option<&RenderLayers>| layers.is_none_or(|l| l.intersects(&RenderLayers::default()));
    let camera = cameras
        .iter()
        .filter(|(_, cam, layers)| cam.is_active && on_default(*layers))
        .max_by_key(|(_, cam, _)| cam.order)
        .map(|(t, _, _)| t.translation());

    let Some((uniform, centre, radius)) = sun.zip(camera).and_then(|(s, c)| moon_uniform(s, c)) else {
        for (entity, _) in [state.body.take(), state.glow.take()].into_iter().flatten() {
            commands.entity(entity).try_despawn();
        }
        return;
    };

    let mut glow_uniform = uniform;
    glow_uniform.params.x = 1.0;
    let MoonState { body, glow } = &mut *state;
    let parts = [(body, uniform, radius, false), (glow, glow_uniform, radius * GLOW_EXTENT, true)];
    for (slot, uniform, scale, glow) in parts {
        let transform = Transform::from_translation(centre).with_scale(Vec3::splat(scale));
        if let Some((entity, handle)) = slot.as_ref() {
            if let Some(mut material) = materials.get_mut(handle) {
                material.uniform = uniform;
            }
            commands.entity(*entity).try_insert(transform);
            continue;
        }
        let handle = materials.add(MoonMaterial { uniform, glow });
        let entity = commands
            .spawn((
                Mesh3d(meshes.add(Sphere::new(1.0).mesh().uv(64, 32))),
                MeshMaterial3d(handle.clone()),
                transform,
                MoonMarker,
                // Chrome, not scene content: see the cloud dome for why a
                // `Name` would be wrong and nothing at all would get it
                // despawned.
                renzora_scene_api::HideInHierarchy,
                bevy::light::NotShadowCaster,
                bevy::light::NotShadowReceiver,
            ))
            .id();
        *slot = Some((entity, handle));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn night(day: f32) -> Sun {
        Sun { elevation: -30.0, moon_follows_calendar: false, moon_day: day, ..default() }
    }

    /// Full moon: lit from the viewer's side. Waxing: lit from the right.
    #[test]
    fn the_shading_light_follows_the_phase() {
        let (full, _, _) = moon_uniform(&night(renzora_world_api::sun::SYNODIC_MONTH_DAYS * 0.5), Vec3::ZERO).unwrap();
        let toward_moon = -night(0.0).moon_direction();
        assert!(full.light.truncate().dot(-toward_moon) > 0.99);

        let (waxing, _, _) = moon_uniform(&night(renzora_world_api::sun::SYNODIC_MONTH_DAYS * 0.25), Vec3::ZERO).unwrap();
        assert!(waxing.light.truncate().dot(waxing.right.truncate()) > 0.99);
    }

    /// The shader compiles as wgpu will compile it, with its two imports
    /// replaced by stand-ins carrying the fields it reads. Nothing else checks
    /// it before the editor loads it, and a shader that fails there only logs.
    #[test]
    fn the_moon_shader_compiles() {
        const STUB: &str = "
            struct VertexOutput {
                @builtin(position) position: vec4<f32>,
                @location(0) world_position: vec4<f32>,
                @location(1) world_normal: vec3<f32>,
            }
            struct View { world_position: vec3<f32>, exposure: f32 }
            @group(0) @binding(0) var<uniform> view: View;
        ";
        let source: String = std::iter::once(STUB.to_string())
            .chain(
                include_str!("moon.wesl")
                    .lines()
                    .filter(|line| !line.trim_start().starts_with("import "))
                    .map(str::to_string),
            )
            .collect::<Vec<_>>()
            .join("\n");
        renzora_shader_api::wgsl::check(&source).unwrap_or_else(|err| panic!("moon.wesl: {err}"));
    }

    #[test]
    fn no_moon_by_day_or_with_no_moonlight() {
        assert!(moon_uniform(&Sun::default(), Vec3::ZERO).is_none());
        let dark = Sun { moon_illuminance: 0.0, ..night(14.0) };
        assert!(moon_uniform(&dark, Vec3::ZERO).is_none());
    }
}
