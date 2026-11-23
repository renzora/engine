//! What the rest of the engine needs to know about ray-traced lighting.
//!
//! Solari (Bevy's hardware-raytraced lighting) is the engine's only GI. Its
//! settings and diagnostics are in `renzora_solari_api`, which only the Solari
//! crates, anti-aliasing, the exporter, level presets and virtual geometry
//! link. What stays here is the small, stable vocabulary that many crates need
//! without caring how Solari is configured: the GPU capability flags, the
//! "Solari is lighting this camera" marker, and the stand-in contracts between
//! the material and geometry crates and the ray tracer.
//!
//! The two were one file until the settings' edit rate (several a week, while
//! Solari was being tuned) rebuilt every crate that only wanted a marker.

use bevy::prelude::*;

/// GPU ray-tracing capability flag, decided ONCE by the host at startup.
///
/// Bevy's `bevy_solari` needs ray-tracing wgpu features (`EXPERIMENTAL_RAY_QUERY`
/// plus acceleration structures) enabled on the `RenderDevice` *at creation time*,
/// and that is decided before any plugin's `build()` runs. So the host
/// (`renzora_runtime`) probes the GPU adapter at boot, requests those features
/// when supported, and records the verdict here. The `renzora_solari` plugin
/// reads it in `build()` (before the device exists) to decide whether installing
/// `SolariPlugins` is safe: adding ray-tracing render nodes on a GPU that can't
/// create them would crash the engine, so when this is absent/`false` the plugin
/// stays inert and the engine boots normally on non-RT GPUs.
///
/// Lives in the contract crate so the host (producer) and the plugin (consumer)
/// share one definition.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct GpuRaytracing {
    pub enabled: bool,
}

/// Whether the GPU the renderer picked is integrated (or a software fallback)
/// rather than a discrete card, probed once at boot by `renzora_runtime`
/// alongside [`GpuRaytracing`], from the same `wgpu` adapter request.
///
/// The editor's cost is dominated by **fullscreen, resolution-bound** passes
/// (SSGI, SSAO, bloom, auto-exposure, TAA), which is exactly what an integrated
/// GPU is worst at: the same scene that runs comfortably on a discrete card can
/// sit in single digits. The Graphics Quality tier exists to trade those passes
/// away, but it defaults to `Medium` and users generally never find it, so this
/// flag lets the editor *point them at it* once.
///
/// Deliberately only a hint: nothing changes a user's tier on their behalf.
///
/// Lives in the contract dylib so the host (producer) and the editor plugins
/// (consumers) share one `TypeId` across the dlopen boundary.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct GpuIsIntegrated {
    pub yes: bool,
}

/// On a camera Solari is lighting right now: added and removed by the Solari
/// plugin with Bevy's `SolariLighting`, so it is never on a camera where ray
/// tracing is unavailable and Solari fell back.
///
/// Here so crates that must stand aside for Solari can see it without linking
/// the Solari plugin. SSAO is one: Solari traces its own ambient occlusion, and
/// screen-space AO on top only darkens corners a second time.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct SolariLit;

// ── Stand-in materials for the ray tracer ──────────────────────────────────

/// A plain `StandardMaterial` standing in for a mesh's real material, on a mesh
/// whose material the ray tracer cannot read.
///
/// Solari shades a ray hit from `StandardMaterial` data alone: it cannot run a
/// per-material shader at a hit point, and it only takes a mesh into its scene
/// when that mesh carries `MeshMaterial3d<StandardMaterial>`. A node-graph
/// material is an extended material with a generated shader, so without this
/// such a mesh is simply absent from the traced scene: no shadow, no bounce, a
/// hole in every reflection.
///
/// The material crate (`renzora_shader`) attaches this to its meshes, holding
/// its best `StandardMaterial` approximation of the graph, and keeps the asset
/// in step with the graph. The ray tracer (`renzora_solari`) traces the mesh
/// with it. Neither crate depends on the other; this type is the whole contract.
///
/// Maintained only while [`TracedStandInsWanted`] exists, so a project that
/// never turns Solari on never pays for the extra assets.
#[cfg(feature = "render_3d")]
#[derive(Component, Clone, Debug)]
pub struct TracedStandIn(pub Handle<bevy::pbr::StandardMaterial>);

/// On a mesh the ray tracer lights itself as a see-through surface (glass, a
/// clear-coat shell), so the raster passes must not draw it as well.
///
/// Solari lights the nearest see-through surface at each pixel with traced
/// reflections. Bevy's forward transparent pass would otherwise draw the same
/// surface again on top, lit from the environment map alone. Inserted and
/// removed by `renzora_solari` while Solari is on; read by
/// `renzora_virtual_geometry`, which skips such a mesh's transparent draw.
///
/// An ordinary mesh is taken out of Bevy's mesh passes while this is on it, by
/// dropping `Mesh3d` from its `VisibilityClass` (and putting it back after),
/// the way virtual geometry takes over a mesh. A mesh virtual geometry already
/// draws has no `Mesh3d` class to drop, and is left to that crate.
#[cfg(feature = "render_3d")]
#[derive(Component, Default, Debug)]
#[component(on_add = take_from_mesh_passes, on_remove = return_to_mesh_passes)]
pub struct SolariShadedTransparent {
    /// Whether adding this took `Mesh3d` out of the visibility class, and so
    /// whether removing it should put it back.
    took_mesh_pass: bool,
}

#[cfg(feature = "render_3d")]
fn take_from_mesh_passes(mut world: bevy::ecs::world::DeferredWorld, ctx: bevy::ecs::lifecycle::HookContext) {
    let mesh = core::any::TypeId::of::<Mesh3d>();
    let mut took = false;
    if let Some(mut class) = world.get_mut::<bevy::camera::visibility::VisibilityClass>(ctx.entity) {
        if let Some(index) = class.iter().position(|id| *id == mesh) {
            class.remove(index);
            took = true;
        }
    }
    if let Some(mut marker) = world.get_mut::<SolariShadedTransparent>(ctx.entity) {
        marker.took_mesh_pass = took;
    }
}

#[cfg(feature = "render_3d")]
fn return_to_mesh_passes(mut world: bevy::ecs::world::DeferredWorld, ctx: bevy::ecs::lifecycle::HookContext) {
    let took = world
        .get::<SolariShadedTransparent>(ctx.entity)
        .is_some_and(|m| m.took_mesh_pass);
    if !took {
        return;
    }
    let mesh = core::any::TypeId::of::<Mesh3d>();
    if let Some(mut class) = world.get_mut::<bevy::camera::visibility::VisibilityClass>(ctx.entity) {
        if !class.contains(&mesh) {
            class.push(mesh);
        }
    }
}

/// Present while a ray tracer wants [`TracedStandIn`] kept on meshes. Inserted
/// and removed by `renzora_solari` as Solari turns on and off; the material
/// crate attaches stand-ins while it exists and strips them once it is gone.
#[derive(Resource, Default, Debug)]
pub struct TracedStandInsWanted;

// ── Full-detail geometry for the ray tracer ────────────────────────────────

/// On a mesh whose `Mesh3d` is only a coarse stand-in for what the camera
/// sees: the surface a ray tracer should trace instead, once there is one.
///
/// A model stored as `.rvg` draws its full detail as virtual geometry, from
/// clusters, while its `Mesh3d` holds a coarser copy (about a third of the
/// triangles) for shadows, physics and picking. Traced, that copy is a second
/// surface a few millimetres off the drawn one: wherever it bulges above, rays
/// leaving the visible surface start underneath it and hit it at once, and
/// those pixels go black in polygon-shaped patches.
///
/// The virtual geometry crate puts this on every such mesh as `None`, which
/// tells the ray tracer to leave the mesh out rather than trace the stand-in,
/// and fills in the full-detail mesh while [`TracedGeometryWanted`] exists.
/// Neither crate depends on the other; this type is the whole contract.
#[derive(Component, Clone, Debug, Default)]
pub struct TracedGeometry(pub Option<Handle<bevy::mesh::Mesh>>);

/// Present while a ray tracer wants [`TracedGeometry`] filled in. Inserted and
/// removed by `renzora_solari` as Solari turns on and off, so the full-detail
/// meshes, which cost as much memory as the model's ordinary meshes would, are
/// only built while something traces them.
#[derive(Resource, Default, Debug)]
pub struct TracedGeometryWanted;
