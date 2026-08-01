//! A custom shaded material, driven by the plugin's own component, over a
//! texture the plugin regenerates every frame.
//!
//! ## What it is for
//!
//! It exists to be *looked at*. Between them, [`Ripple`] and its shader exercise
//! every part of the material path in one entity: the uniform at
//! `@group(3) @binding(0)`, a texture and sampler above it, per-frame uniform
//! refresh from the component, and per-frame pixel upload through
//! [`Images::write`]. Any of those failing is visible rather than silent — see
//! *Reading the result* below.
//!
//! That mattered because the path had never drawn a frame when this was written.
//! A material plugin is the one part of the surface where a mistake produces a
//! black quad instead of a compiler error, so the surface wanted something that
//! turns each possible mistake into a distinguishable picture.
//!
//! ## Reading the result
//!
//! Add **Ripple** to any entity. It becomes a two-metre plane showing coloured
//! rings travelling outwards over a drifting plasma.
//!
//! | What you see | What is wrong |
//! |---|---|
//! | Rings over plasma, animating | Nothing — this is the whole path working |
//! | Flat black | The uniform is not arriving; the bind group index is wrong |
//! | Rings, but flat white background | The texture is not bound; sampling fell back |
//! | Rings frozen, plasma frozen | The per-frame refresh is not running |
//! | Plasma drifts, rings frozen | [`Images::write`] works, the uniform refresh does not |
//! | Nothing at all | The material was refused at init — check the log |
//!
//! ## The one thing that reads oddly
//!
//! [`Ripple`] carries two `_pad` fields it never uses, and one of them holds
//! plugin state. That is not sloppiness: the component *is* the uniform, byte for
//! byte, so a field added for the plugin's own bookkeeping would shift every
//! member after it and silently corrupt the shader's view. Padding the WGSL side
//! already requires is the only space available. See [`Ripple::_ready`].

use renzora_plugin::prelude::*;
use renzora_plugin::sys::{AlphaMode, AssetHandle, ImageFormat};
use std::sync::atomic::{AtomicU64, Ordering};

/// Side length of the generated texture.
///
/// Small on purpose. It is rewritten from the CPU every frame, so this is a
/// memcpy plus a GPU upload per frame per material, and the demo should not be
/// the reason someone's frame time moved.
const TEX: usize = 64;

/// Rings and a plasma, drawn by the plugin's own WGSL.
///
/// The field order is load-bearing — it is the uniform block the shader reads.
/// `params` is the first `vec4` and `tint` the second, so anything reordered here
/// has to be reordered in `ripple.wgsl` too.
#[derive(Component)]
#[component(name = "Ripple")]
#[repr(C)]
pub struct Ripple {
    /// Seconds since this entity started rippling. Driven by [`animate`], not by
    /// hand — hence `skip`, since a slider that fights a system every frame is
    /// worse than no slider.
    #[field(skip)]
    pub time: f32,
    /// Ring travel speed.
    #[field(min = 0.0, max = 20.0, speed = 0.05)]
    pub speed: f32,
    /// How much of the colour the rings modulate. 0 is a flat tint.
    #[field(min = 0.0, max = 1.0)]
    pub amplitude: f32,
    /// Padding: `params.w`, which the shader ignores.
    pub _pad0: f32,

    /// Linear RGB, as three scalars rather than a `Vec3`.
    ///
    /// A `Vec3` would be the obvious choice and is the wrong one: WGSL aligns
    /// `vec3<f32>` to 16 bytes and Rust aligns `[f32; 3]` to 4, so the same
    /// struct would be a different size on each side. Scalars cannot disagree.
    #[field(min = 0.0, max = 1.0)]
    pub tint_r: f32,
    #[field(min = 0.0, max = 1.0)]
    pub tint_g: f32,
    #[field(min = 0.0, max = 1.0)]
    pub tint_b: f32,

    /// Padding — `tint.w` — doubling as "this entity already has its mesh".
    ///
    /// There is nowhere else to put it. The component is the uniform block, so a
    /// real field for this would move `tint` and hand the shader the wrong bytes.
    /// The shader never reads `tint.w`, so the slot is genuinely free.
    ///
    /// It also wants *not* to survive a scene load, and being `_`-prefixed it
    /// does not: a freshly loaded entity re-applies its mesh, which is the
    /// correct behaviour rather than an accident.
    pub _ready: f32,
}

impl Default for Ripple {
    fn default() -> Self {
        Self {
            time: 0.0,
            speed: 6.0,
            amplitude: 0.7,
            _pad0: 0.0,
            tint_r: 0.2,
            tint_g: 0.6,
            tint_b: 0.9,
            _ready: 0.0,
        }
    }
}

/// Handles from `build`, parked where zero-sized systems can reach them.
///
/// The same `static` shape every plugin that creates assets ends up with: a
/// handle made during init cannot be captured by a closure the host has to own.
static MESH: AtomicU64 = AtomicU64::new(u64::MAX);
static MATERIAL: AtomicU64 = AtomicU64::new(u64::MAX);
static TEXTURE: AtomicU64 = AtomicU64::new(u64::MAX);

/// Give any entity carrying [`Ripple`] the plane and the custom material.
///
/// Absence of a `_ready` flag is the trigger, since there is no `Added<T>` across
/// the boundary. Re-issuing `make_renderable` every frame would work and cost a
/// command per entity per frame; the flag is what makes it once.
fn apply(mut q: Query<(Entity, &mut Ripple, &Transform)>, mut commands: Commands) {
    let mesh = AssetHandle(MESH.load(Ordering::Relaxed));
    let material = AssetHandle(MATERIAL.load(Ordering::Relaxed));
    for (entity, ripple, transform) in &mut q {
        if ripple._ready != 0.0 {
            continue;
        }
        ripple._ready = 1.0;
        // The transform passed back is the one just read. `make_renderable` sets
        // all three of mesh, material and transform, so handing it a default
        // would teleport the entity to the origin — which it did, until an
        // afternoon of watching cubes jump proved the point.
        commands
            .entity(entity)
            .make_renderable(mesh, material, *transform);
    }
}

/// Advance each ripple's clock.
///
/// The host copies the component's bytes into the uniform after this runs, so
/// writing `time` here is the whole of "animating a shader parameter" — there is
/// no second GPU-side struct to keep in step.
fn animate(mut q: Query<&mut Ripple>, time: Res<Time>) {
    let dt = time.delta_secs();
    for ripple in &mut q {
        ripple.time += dt;
    }
}

/// Redraw the shared plasma texture.
///
/// One texture for every rippling entity, which is why this is not inside
/// [`animate`]: the cost is per frame, not per entity, and a hundred ripples
/// upload exactly as much as one.
///
/// It runs unconditionally rather than only when something has [`Ripple`],
/// because a system whose query is empty still runs and this one has no query to
/// be empty. That is fine — the demo is the only thing that owns this texture.
fn plasma(time: Res<Time>, images: Images) {
    let handle = AssetHandle(TEXTURE.load(Ordering::Relaxed));
    if handle.0 == u64::MAX {
        return;
    }
    let t = time.elapsed_secs();
    let mut pixels = vec![0u8; TEX * TEX * 4];
    for y in 0..TEX {
        for x in 0..TEX {
            let fx = x as f32 / TEX as f32;
            let fy = y as f32 / TEX as f32;
            // Two interfering sine fields. Cheap, and it drifts visibly enough
            // that a frozen upload is obvious at a glance.
            let v = ((fx * 12.0 + t).sin() + (fy * 9.0 - t * 0.7).sin()) * 0.25 + 0.5;
            let i = (y * TEX + x) * 4;
            let b = (v.clamp(0.0, 1.0) * 255.0) as u8;
            pixels[i] = b;
            pixels[i + 1] = b;
            pixels[i + 2] = b;
            pixels[i + 3] = 255;
        }
    }
    images.write(handle, &pixels);
}

pub struct RipplePlugin;

impl Plugin for RipplePlugin {
    fn build(&self, app: &mut App) {
        let mesh = app.add_mesh(renzora_plugin::sys::Primitive::Plane, Vec3::splat(2.0));

        // Mid grey rather than black: if the per-frame upload never runs, the
        // material still shows its rings over a flat background instead of
        // reading as "the material failed".
        let texture = app.add_image(
            TEX as u32,
            TEX as u32,
            ImageFormat::Rgba8,
            &vec![128u8; TEX * TEX * 4],
        );

        // `Opaque` because nothing here is transparent, and an alpha mode that
        // does not need to be blended should not be — it keeps the material in
        // the opaque pass, where it is depth-sorted for free.
        let material = app.add_material_shader::<Ripple>(
            "ripple",
            include_str!("ripple.wgsl"),
            AlphaMode::Opaque,
            &[texture],
        );

        MESH.store(mesh.0, Ordering::Relaxed);
        MATERIAL.store(material.0, Ordering::Relaxed);
        TEXTURE.store(texture.0, Ordering::Relaxed);

        app.register_component::<Ripple>()
            .add_systems(Update, apply)
            .add_systems(Update, animate)
            .add_systems(Update, plasma);
    }
}

renzora_plugin::add!(RipplePlugin);
