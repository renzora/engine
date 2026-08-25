//! The splash cinematic — **Light Chamber**: an endless corridor of slowly
//! turning slotted gates with spectrally-tinted spot lights raking through them,
//! rendered by an isolated `Camera3d` into an offscreen image the splash shows as
//! its background ([`ChamberView`]).
//!
//! There is no world here and no horizon — the subject is the *light*, not any
//! object. Each gate is two banks of vertical slats with a clear tunnel down the
//! middle; the lights sit far behind the last gate, so every slat is a silhouette
//! and every gap is a shaft. Volumetric fog makes those shafts visible in the air,
//! and because the gates each spin about the view axis at a different rate, the
//! shafts cross at continuously changing angles — the banding never repeats.
//!
//! Two deliberate choices are worth knowing before tuning this:
//!
//! * **The clear tunnel is a layout decision, not just an aesthetic one.** Slats
//!   start at [`SLAT_INNER`] from the axis and gates only ever rotate *about* that
//!   axis, so the centre of frame — where the launcher's title, recents list and
//!   buttons sit — can never be crossed by a slat. All the visual noise stays out
//!   at the edges where there is no text to fight with. It also guarantees the
//!   camera can never clip through a slat, which would strobe the whole frame black
//!   every time a gate passed.
//! * **The corridor moves, the camera doesn't.** Gates and dust drift toward the
//!   lens and wrap to the far end when they pass behind it; the camera only sways.
//!   The wrap happens ~80 units out, deep enough in the fog that a gate arriving
//!   with a different roll than the one that left is invisible. Nothing accumulates,
//!   so the shot is stable for as long as the launcher is open.
//!
//! The camera clears to opaque black and owns the whole frame — there is no
//! separate sky layer to composite against. Render-to-texture on a dedicated render
//! layer keeps it isolated from the editor's cameras, and the scene only exists
//! while in [`SplashState::Splash`] (spawned/torn down by [`manage_chamber`]).

use bevy::asset::Asset;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Hdr, RenderTarget};
use bevy::light::{FogVolume, NotShadowCaster, VolumetricFog, VolumetricLight};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, Extent3d, TextureFormat, TextureUsages};
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial};
use bevy::ui_render::UiMaterialPlugin;

use crate::SplashState;

/// This chamber's private render layer. Allocated in the contract crate
/// alongside every other offscreen rig's — see the registry there for why.
use renzora::core::viewport_types::SPLASH_CHAMBER_LAYER as CHAMBER_LAYER;

/// Offscreen render size. Deliberately below 1080p: this scene's cost is
/// dominated by the volumetric raymarch, which is pure fill rate, and the image
/// it produces is soft fog and bloom with no fine detail to lose. Dropping to 720p
/// buys back ~45% of that raymarch for something nobody can see the absence of.
const RES: UVec2 = UVec2::new(1280, 720);

// ── Corridor layout (camera on the +Z end looking down -Z) ───────────────────

/// Gate count and the world-space gap between consecutive gates.
const GATES: usize = 7;
const GATE_SPACING: f32 = 11.0;
/// Total corridor length — the distance a gate travels before it wraps.
const SPAN: f32 = GATES as f32 * GATE_SPACING;
/// A gate wraps once it passes this Z (safely behind the lens) and reappears
/// `SPAN` further out.
const NEAR_Z: f32 = 9.0;
const FAR_Z: f32 = NEAR_Z - SPAN;
/// How fast the corridor flows toward the camera (world units / sec).
const DRIFT: f32 = 2.6;

/// Distance from the far end over which a wrapped gate's slats grow back to full
/// width, and a mote fades back in.
///
/// **This is load-bearing, not polish.** The key lights are behind the *whole*
/// corridor, so a gate is an occluder for everything in front of it: one
/// materialising at [`FAR_Z`] starts shadowing every shaft in frame within a single
/// frame, and at one wrap every `GATE_SPACING / DRIFT` ≈ 4.2 s that read as the
/// entire image flashing. Fog doesn't hide it — fog dims the *gate*, but the gate's
/// shadow is cast the full length of the corridor regardless of how visible the gate
/// itself is.
///
/// Growing the slats in is the fix rather than fading them, because a shadow can't
/// be faded — a half-transparent slat still casts a full shadow. A slat scaled to
/// zero width has no cross-section to block light with, so the shadow ramps in with
/// the geometry.
const FADE_IN: f32 = 20.0;

/// Slats per side of a gate, their spacing, and the half-width of the clear
/// tunnel down the middle (see the module doc — this keeps the UI's centre column
/// clear and keeps the lens out of the geometry).
const SLATS_PER_SIDE: usize = 8;
const SLAT_PITCH: f32 = 2.45;
const SLAT_INNER: f32 = 3.4;
/// Slat dimensions. The height overshoots the frame by a wide margin so a rotated
/// gate never shows a slat *end* — they must read as bars running off-screen.
const SLAT_W: f32 = 0.72;
const SLAT_H: f32 = 44.0;
const SLAT_D: f32 = 0.55;

/// Camera Z. The corridor flows past it; it only sways.
const CAM_Z: f32 = 6.0;

// ── Lighting ─────────────────────────────────────────────────────────────────

/// The three key lights sit *behind* the far end of the corridor, which is what
/// makes every slat a silhouette and every gap a shaft.
const LIGHT_Z: f32 = -96.0;
/// How far off-axis they orbit, and how long one full orbit takes.
const LIGHT_RADIUS: f32 = 15.0;
const ORBIT_PERIOD: f32 = 74.0;

/// Lumens per key light.
///
/// This number looks absurd next to Bevy's 1e6 default because the geometry it has
/// to reach is ~25–100 units away, and point/spot falloff is inverse-square: at
/// 70 units you need ~50× the intensity you'd need at 10 to land in the same
/// exposure range. The steep falloff across that range is the point — the far end
/// of the corridor glows and the near gates go to silhouette, which is where the
/// depth comes from. **This is the first dial to reach for** if the shot comes out
/// blown-out or murky.
const LIGHT_LUMENS: f32 = 8.0e7;
const LIGHT_RANGE: f32 = 320.0;

/// Base hues (degrees) of the three keys, and how fast the whole triad rotates
/// through the colour wheel. Keeping them 120° apart means their shafts overlap
/// into oil-slick secondaries wherever two cross, which is where the iridescence
/// actually comes from — it is mixed *in the air*, not painted on in a shader.
const HUES: [f32; 3] = [188.0, 306.0, 66.0];
const HUE_DRIFT: f32 = 2.6; // degrees / sec → a full rotation every ~2.3 min

/// Number of dust motes drifting with the corridor.
const MOTES: usize = 700;

// ── Components / resources ───────────────────────────────────────────────────

/// The fullscreen UI node (in the splash root) that shows the chamber render.
#[derive(Component)]
pub(crate) struct ChamberView;

#[derive(Component)]
struct ChamberCamera;

/// A gate's root. Children are its slats; this entity carries the roll and the
/// corridor position.
#[derive(Component)]
struct Gate {
    /// Z at t = 0, before drift and wrapping.
    base_z: f32,
    /// Roll at t = 0 and its (signed) rate — every gate differs, so the crossing
    /// angle between any two gates is always changing.
    roll: f32,
    roll_rate: f32,
}

/// One of the three key lights.
#[derive(Component)]
struct KeyLight {
    /// Position in the orbit (radians) and hue (degrees) at t = 0.
    phase: f32,
    hue: f32,
}

/// One slat of a gate. Its width is scaled by the gate's fade-in (see [`FADE_IN`]).
#[derive(Component)]
struct Slat;

/// A dust mote. Motes are lit but cast no shadows, so they light up *only* where a
/// shaft passes through them — that's what makes the beams read as volumes of air
/// rather than as flat gradients.
#[derive(Component)]
struct Mote {
    base: Vec3,
    /// Seeds its lateral wobble so no two drift alike.
    wobble: f32,
    /// Full size, scaled down by the fade-in near the far end.
    scale: f32,
}

/// Marker on every world entity the chamber owns, for one-shot teardown.
#[derive(Component)]
struct ChamberEntity;

#[derive(Resource, Default)]
struct ChamberScene {
    image: Handle<Image>,
    built: bool,
}

/// UI material that runs the chamber render through `chamber.wgsl` (the spectral
/// finishing pass).
#[derive(Asset, TypePath, AsBindGroup, Clone)]
struct ChamberMaterial {
    /// x = time, y = width(px), z = height(px).
    #[uniform(0)]
    params: Vec4,
    #[texture(1)]
    #[sampler(2)]
    image: Option<Handle<Image>>,
}

impl UiMaterial for ChamberMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_splash/chamber.wgsl".into()
    }
}

pub(crate) fn register(app: &mut App) {
    bevy::asset::embedded_asset!(app, "chamber.wgsl");
    app.init_resource::<ChamberScene>()
        .add_plugins(UiMaterialPlugin::<ChamberMaterial>::default())
        .add_systems(
            Update,
            (
                manage_chamber,
                attach_chamber_view,
                animate_gates,
                animate_lights,
                animate_motes,
                sync_chamber_material,
            ),
        );
}

// ── Lifecycle ────────────────────────────────────────────────────────────────

/// Build the scene on entering the splash, tear it down on leaving.
///
/// Gated on the same integrated-GPU check as the post pass (see
/// `native_post::gate_post_camera`): a volumetric raymarch with three shadowed
/// lights is exactly the workload an integrated adapter is worst at, and this is
/// the first thing a user sees. Where the post camera merely stops *displaying*
/// the cinematic, this stops *paying* for it — without the gate here the scene
/// would render every frame into an image nothing samples.
#[allow(clippy::too_many_arguments)] // a Bevy system; each param is a distinct world access
fn manage_chamber(
    mut commands: Commands,
    state: Res<State<SplashState>>,
    integrated: Option<Res<renzora::GpuIsIntegrated>>,
    mut scene: ResMut<ChamberScene>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    owned: Query<Entity, With<ChamberEntity>>,
) {
    let cinematic_ok = !integrated.is_some_and(|g| g.yes);
    let want = matches!(state.get(), SplashState::Splash) && cinematic_ok;

    if want && !scene.built {
        if scene.image == Handle::default() {
            scene.image = images.add(make_target(RES));
        }
        spawn_chamber(&mut commands, &mut meshes, &mut materials, scene.image.clone());
        scene.built = true;
    } else if !want && scene.built {
        for e in &owned {
            commands.entity(e).try_despawn();
        }
        scene.built = false;
    }
}

fn make_target(size: UVec2) -> Image {
    let extent = Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 };
    let mut img = Image { data: Some(vec![0u8; (extent.width * extent.height * 4) as usize]), ..default() };
    img.texture_descriptor.size = extent;
    img.texture_descriptor.format = TextureFormat::Bgra8UnormSrgb;
    img.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    img
}

fn attach_chamber_view(
    mut commands: Commands,
    scene: Res<ChamberScene>,
    mut mats: ResMut<Assets<ChamberMaterial>>,
    views: Query<Entity, (With<ChamberView>, Without<MaterialNode<ChamberMaterial>>)>,
) {
    if !scene.built {
        return;
    }
    for e in &views {
        let handle = mats.add(ChamberMaterial { params: Vec4::ZERO, image: Some(scene.image.clone()) });
        commands.entity(e).insert(MaterialNode(handle));
    }
}

fn sync_chamber_material(
    time: Res<Time>,
    mut mats: ResMut<Assets<ChamberMaterial>>,
    views: Query<&MaterialNode<ChamberMaterial>, With<ChamberView>>,
) {
    let t = time.elapsed_secs();
    for mat in &views {
        if let Some(mut m) = mats.get_mut(&mat.0) {
            m.params = Vec4::new(t, RES.x as f32, RES.y as f32, 0.0);
        }
    }
}

// ── Scene construction ───────────────────────────────────────────────────────

fn spawn_chamber(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    image: Handle<Image>,
) {
    let layer = RenderLayers::layer(CHAMBER_LAYER);

    spawn_camera(commands, &layer, image);
    spawn_fog(commands, &layer);
    spawn_lights(commands, &layer);
    spawn_gates(commands, meshes, materials, &layer);
    spawn_motes(commands, meshes, materials, &layer);
}

fn spawn_camera(commands: &mut Commands, layer: &RenderLayers, image: Handle<Image>) {
    commands.spawn((
        Camera3d::default(),
        Hdr,
        // Beams are the subject, so bloom is load-bearing rather than decorative —
        // it's what turns a hard-edged shaft into light with weight. Still well
        // under the default NATURAL intensity: past ~0.3 the shafts fuse into a
        // single glow and the crossing pattern is lost.
        Bloom { intensity: 0.22, ..Bloom::NATURAL },
        // Volumetric fog supports MSAA (it samples a multisampled depth texture),
        // and the slats are long high-contrast diagonals that alias badly without it.
        Msaa::Sample4,
        Projection::Perspective(PerspectiveProjection { fov: 0.95, near: 0.1, ..default() }),
        // `jitter` is deliberately 0. Bevy's jitter is keyed on `frame_count`, so it
        // resamples every frame — with TAA that resolves to smooth fog, but there is
        // no TAA on this camera and it would show as crawling noise. The step count
        // is doing that job instead; the grain in `post.wgsl` covers what's left.
        VolumetricFog { ambient_color: Color::BLACK, ambient_intensity: 0.0, step_count: 56, jitter: 0.0 },
        Camera { clear_color: ClearColorConfig::Custom(Color::BLACK), order: -50, ..default() },
        RenderTarget::Image(image.into()),
        // Barely-there ambient. Its only job is to keep the slats from going to
        // absolute black where no shaft reaches them, so the corridor still has
        // form in the gaps between beams.
        AmbientLight { color: Color::srgb(0.35, 0.42, 0.65), brightness: 55.0, affects_lightmapped_meshes: false },
        Transform::from_xyz(0.0, 0.0, CAM_Z).looking_at(Vec3::new(0.0, 0.0, -40.0), Vec3::Y),
        layer.clone(),
        ChamberCamera,
        ChamberEntity,
        renzora::HideInHierarchy,
        Name::new("Splash Chamber Camera"),
    ));
}

/// The fog volume the shafts are drawn in.
///
/// Two boundaries matter. It has to **enclose the camera**, because fog only
/// scatters where the view ray is inside the volume — a box starting in front of the
/// lens would clip the beams off at its near face. And its far face has to sit
/// **in front of the key lights**, never around them: the raymarch samples
/// inverse-square falloff, so a sample taken near a light integrates an enormous
/// value and burns out into a blown white-hot blob. Keeping the lights outside the
/// box costs nothing — light from outside still lights the fog inside it, so the
/// shafts are unaffected — and it removes the hot spot along with the slow
/// swelling caused by the lights' Z sway carrying them across the boundary.
fn spawn_fog(commands: &mut Commands, layer: &RenderLayers) {
    commands.spawn((
        FogVolume {
            fog_color: Color::WHITE,
            // Thin enough to see the far end of the corridor through, thick enough
            // that a shaft is visible in the air rather than only where it lands.
            density_factor: 0.045,
            absorption: 0.12,
            scattering: 0.55,
            // High asymmetry (forward scattering) makes a shaft flare as it swings
            // toward the lens and fade as it swings away — the "sweep" in the shot.
            scattering_asymmetry: 0.7,
            light_intensity: 1.25,
            ..default()
        },
        // Spans z −80 … +20: past the farthest gate (−68), around the camera (+6),
        // and stopping well short of the lights (−90 … −102 with their sway).
        Transform::from_xyz(0.0, 0.0, -30.0).with_scale(Vec3::new(130.0, 100.0, 100.0)),
        layer.clone(),
        ChamberEntity,
        renzora::HideInHierarchy,
        Name::new("Splash Chamber Fog"),
    ));
}

fn spawn_lights(commands: &mut Commands, layer: &RenderLayers) {
    for (i, hue) in HUES.iter().enumerate() {
        let phase = i as f32 * std::f32::consts::TAU / HUES.len() as f32;
        commands.spawn((
            SpotLight {
                color: Color::hsl(*hue, 0.85, 0.55),
                intensity: LIGHT_LUMENS,
                range: LIGHT_RANGE,
                radius: 0.4,
                // Shadows are not optional here: an unshadowed light produces a
                // uniform glow, and every shaft in this shot is a slat's shadow.
                shadow_maps_enabled: true,
                inner_angle: 0.06,
                outer_angle: 0.55,
                ..default()
            },
            VolumetricLight,
            Transform::from_xyz(0.0, 0.0, LIGHT_Z),
            layer.clone(),
            KeyLight { phase, hue: *hue },
            ChamberEntity,
            renzora::HideInHierarchy,
            Name::new("Splash Chamber Key"),
        ));
    }
}

fn spawn_gates(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    layer: &RenderLayers,
) {
    let slat_mesh = meshes.add(Cuboid::new(SLAT_W, SLAT_H, SLAT_D));
    // Near-black polished metal. It carries almost no colour of its own — what you
    // see on a slat edge is the key lights reflected off it, which is why the edges
    // pick up the same spectral pairing as the shafts do.
    let slat_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.020, 0.022, 0.030),
        perceptual_roughness: 0.28,
        metallic: 1.0,
        // A clearcoat lobe on top of the metal gives a second, tighter highlight —
        // it's what reads as an oil-slick sheen when two differently-tinted keys
        // graze the same edge.
        clearcoat: 1.0,
        clearcoat_perceptual_roughness: 0.1,
        ..default()
    });

    for g in 0..GATES {
        let base_z = NEAR_Z - g as f32 * GATE_SPACING;
        // Alternate the spin direction and step the rate per gate. Equal rates would
        // lock the gates into a fixed relative angle and the crossings would freeze.
        let dir = if g % 2 == 0 { 1.0 } else { -1.0 };
        let roll_rate = dir * (0.045 + 0.022 * g as f32);
        let roll = g as f32 * 0.7;

        let gate = commands
            .spawn((
                Transform::from_xyz(0.0, 0.0, base_z),
                Visibility::default(),
                Gate { base_z, roll, roll_rate },
                layer.clone(),
                ChamberEntity,
                renzora::HideInHierarchy,
                Name::new("Splash Chamber Gate"),
            ))
            .id();

        let mut slats = Vec::with_capacity(SLATS_PER_SIDE * 2);
        for s in 0..SLATS_PER_SIDE {
            let offset = SLAT_INNER + s as f32 * SLAT_PITCH;
            for side in [-1.0f32, 1.0] {
                slats.push(
                    commands
                        .spawn((
                            Mesh3d(slat_mesh.clone()),
                            MeshMaterial3d(slat_mat.clone()),
                            Transform::from_xyz(offset * side, 0.0, 0.0),
                            layer.clone(),
                            Slat,
                            ChamberEntity,
                            renzora::HideInHierarchy,
                            Name::new("Splash Chamber Slat"),
                        ))
                        .id(),
                );
            }
        }
        commands.entity(gate).add_children(&slats);
    }
}

fn spawn_motes(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    layer: &RenderLayers,
) {
    let mote_mesh = meshes.add(Sphere::new(1.0).mesh().uv(6, 4));
    // Plain diffuse white, no emissive — a mote must be *dark* outside a shaft.
    // Emissive dust would glow everywhere and flatten the beams into background.
    let mote_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.85,
        metallic: 0.0,
        ..default()
    });

    for i in 0..MOTES {
        let n = i as u32;
        // Polar placement keeps the density even out to the corridor walls; a naive
        // box would clump the motes in the corners where nothing is lit.
        let angle = hash01(n * 3) * std::f32::consts::TAU;
        let radius = 1.5 + hash01(n * 5 + 1).sqrt() * 20.0;
        let base = Vec3::new(
            angle.cos() * radius,
            angle.sin() * radius,
            FAR_Z + hash01(n * 7 + 2) * SPAN,
        );
        let scale = 0.03 + hash01(n * 11 + 3) * 0.045;

        commands.spawn((
            Mesh3d(mote_mesh.clone()),
            MeshMaterial3d(mote_mat.clone()),
            Transform::from_translation(base).with_scale(Vec3::splat(scale)),
            // 700 specks in three shadow maps would cost more than they could
            // possibly occlude.
            NotShadowCaster,
            layer.clone(),
            Mote { base, wobble: hash01(n * 13 + 4) * std::f32::consts::TAU, scale },
            ChamberEntity,
            renzora::HideInHierarchy,
            Name::new("Splash Chamber Mote"),
        ));
    }
}

/// Cheap deterministic 0..1 hash (no rng crate; stable across runs, so the mote
/// field is identical every launch).
fn hash01(n: u32) -> f32 {
    let mut x = n.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    x = ((x >> ((x >> 28).wrapping_add(4))) ^ x).wrapping_mul(277_803_737);
    (((x >> 22) ^ x) & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

/// Where something starting at `base_z` sits after `t` seconds of drift, wrapped
/// into the corridor. Computed from absolute time rather than accumulated per
/// frame so it can't drift out of alignment over a long session.
fn corridor_z(base_z: f32, t: f32) -> f32 {
    FAR_Z + (base_z - FAR_Z + DRIFT * t).rem_euclid(SPAN)
}

/// How "present" something at `z` is — 0 at the instant it wraps in at the far end,
/// 1 once it's [`FADE_IN`] units into the corridor. Smoothstepped, so it eases in at
/// both ends rather than starting to grow at a constant rate the moment it appears.
fn fade_in(z: f32) -> f32 {
    let x = ((z - FAR_Z) / FADE_IN).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

// ── Animation ────────────────────────────────────────────────────────────────

/// Drive each gate's position and roll, and grow its slats in behind the wrap.
///
/// Only the *arrival* needs hiding. A gate leaving at [`NEAR_Z`] is already behind
/// the lens, and since a gate can only shadow what's in front of it, everything it
/// still occludes at that point is off-screen — so it can vanish outright with no
/// visible consequence.
fn animate_gates(
    time: Res<Time>,
    mut gates: Query<(&Gate, &Children, &mut Transform)>,
    mut slats: Query<&mut Transform, (With<Slat>, Without<Gate>)>,
) {
    let t = time.elapsed_secs();
    for (gate, children, mut tf) in &mut gates {
        let z = corridor_z(gate.base_z, t);
        tf.translation.z = z;
        tf.rotation = Quat::from_rotation_z(gate.roll + gate.roll_rate * t);

        // Scale the slat's own X (its width) rather than the gate's, which would
        // drag every slat toward the axis and read as the whole gate imploding.
        let fade = fade_in(z);
        for child in children.iter() {
            if let Ok(mut slat_tf) = slats.get_mut(child) {
                slat_tf.scale.x = fade;
            }
        }
    }
}

/// Orbit the keys around the corridor axis and rotate their hues together.
///
/// The orbit is what sweeps the banding across the frame; the hue rotation is slow
/// enough (a full wheel every ~2 minutes) that you never catch it changing, only
/// notice that the shot is a different colour than when you last looked.
fn animate_lights(time: Res<Time>, mut lights: Query<(&KeyLight, &mut SpotLight, &mut Transform)>) {
    let t = time.elapsed_secs();
    let orbit = t * std::f32::consts::TAU / ORBIT_PERIOD;
    for (key, mut light, mut tf) in &mut lights {
        let a = key.phase + orbit;
        // Breathe the orbit radius on a period that shares no factor with the orbit
        // itself, so the rig never returns to a pose you've already seen.
        let r = LIGHT_RADIUS * (1.0 + 0.22 * (t * 0.13 + key.phase).sin());
        let pos = Vec3::new(a.cos() * r, a.sin() * r, LIGHT_Z + (t * 0.09 + key.phase).sin() * 6.0);
        *tf = Transform::from_translation(pos).looking_at(Vec3::new(0.0, 0.0, CAM_Z), Vec3::Y);

        light.color = Color::hsl((key.hue + t * HUE_DRIFT).rem_euclid(360.0), 0.85, 0.55);
        // A gentle intensity swell per light, out of phase with the others, so the
        // colour balance of the chamber keeps shifting without any of them dimming
        // far enough to leave a dead patch.
        light.intensity = LIGHT_LUMENS * (1.0 + 0.28 * (t * 0.21 + key.phase * 1.7).sin());
    }
}

fn animate_motes(time: Res<Time>, mut motes: Query<(&Mote, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (mote, mut tf) in &mut motes {
        // Dust doesn't fall in straight lines — a slow lateral wobble is most of
        // what sells these as air rather than as a particle grid.
        let sway = Vec2::new(
            (t * 0.17 + mote.wobble).sin() * 0.5,
            (t * 0.23 + mote.wobble * 1.7).cos() * 0.5,
        );
        let z = corridor_z(mote.base.z, t);
        tf.translation = Vec3::new(mote.base.x + sway.x, mote.base.y + sway.y, z);
        // Same wrap problem as the gates, at a much smaller scale: ~24 motes wrap
        // every second, and one popping into existence inside a shaft is a visible
        // twinkle. They cast no shadows, so here the fade is purely cosmetic.
        tf.scale = Vec3::splat(mote.scale * fade_in(z));
    }
}
