//! Renzora Solari — hardware-raytraced global illumination, as a drop-in plugin.
//!
//! Wraps Bevy's experimental `bevy_solari` (`SolariPlugins`: realtime raytraced
//! direct + indirect lighting, fully dynamic, no baking) behind Renzora's plugin
//! contract. Ships as a `cdylib` in `plugins/` like `renzora_lumen` — drop it in
//! to enable Solari, delete it to disable. Nothing in the host references this
//! crate.
//!
//! ## Why this needs a host capability flag
//!
//! Solari requires ray-tracing wgpu features (`EXPERIMENTAL_RAY_QUERY` +
//! acceleration structures) enabled on the `RenderDevice` *at creation time*.
//! That is frozen before any dlopen plugin's `build()` runs, so this plugin
//! cannot turn them on itself. The host (`renzora_runtime`) probes the GPU at
//! startup, requests the features when supported, and records the result in
//! [`renzora::GpuRaytracing`]. We read that here and install `SolariPlugins`
//! ONLY when ray tracing is available — otherwise adding RT render nodes on an
//! incapable GPU would crash the engine. Flag absent/false ⇒ inert (warn +
//! no-op) so the engine still boots on non-RT GPUs with the plugin present.
//!
//! ## Authoring
//!
//! [`renzora::SolariGi`] is authored on the "World Environment" source entity
//! and routed to cameras via [`renzora::EffectRouting`] (same mechanism as
//! `LumenLighting`). While enabled we attach Bevy's `SolariLighting` to each
//! routed camera (which pulls in the required HDR + prepass components) with
//! `Msaa::Off`, and mirror every *conforming* mesh into the ray-tracing scene
//! via `RaytracingMesh3d`. Solari's BLAS builder rejects meshes that lack
//! tangents/UVs or use 16-bit indices, so non-conforming meshes are skipped
//! (and marked so we don't re-check them every frame) rather than crashing.

use bevy::prelude::*;
use bevy::camera::CameraMainTextureUsages;
use bevy::core_pipeline::prepass::DeferredPrepass;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::DefaultOpaqueRendererMethod;
use bevy::render::render_resource::TextureUsages;
use bevy::render::view::Msaa;
use bevy::solari::realtime::SolariLighting;
use bevy::solari::scene::RaytracingMesh3d;
use bevy::solari::SolariPlugins;
use renzora::{EffectRouting, GpuRaytracing, SolariGi};

#[cfg(feature = "editor")]
mod editor;

#[derive(Default)]
pub struct SolariPlugin;

impl Plugin for SolariPlugin {
    fn build(&self, app: &mut App) {
        // Always register the type so `SolariGi` round-trips through scene
        // save/load even on a machine where ray tracing is unavailable (a scene
        // authored on an RT box must still load on a non-RT box).
        app.register_type::<SolariGi>();

        // The inspector entry is registered either way so the component stays
        // discoverable; the systems below only run when ray tracing is live.
        #[cfg(feature = "editor")]
        editor::register_inspectors(app);

        let rt = app
            .world()
            .get_resource::<GpuRaytracing>()
            .map(|r| r.enabled)
            .unwrap_or(false);
        if !rt {
            warn!(
                "[runtime] SolariPlugin loaded but GPU ray tracing is unavailable — \
                 Solari is inert. (Needs an RT-capable GPU on the Vulkan/DX12/Metal \
                 backend; see renzora::GpuRaytracing.)"
            );
            return;
        }

        info!("[runtime] SolariPlugin (GI: Bevy Solari hardware ray tracing)");
        app.add_plugins(SolariPlugins);
        // `bevy_solari`'s plugin globally flips `DefaultOpaqueRendererMethod` to
        // deferred in its `build()`. In Renzora that crashes EVERY camera lacking
        // a deferred prepass (previews, thumbnails, multi-viewport) the instant
        // the plugin loads — `queue_prepass_material_meshes` unwraps the missing
        // deferred phase. Reset it to forward here; `manage_solari_render_mode`
        // switches to deferred only while Solari is actually active, and then
        // gives every 3d camera the deferred prepass so the phase exists.
        app.insert_resource(DefaultOpaqueRendererMethod::forward());
        app.init_resource::<SolariActive>();
        // Observers apply the per-camera setup the INSTANT the component is
        // inserted — by our sync, a scene load, or the play-mode scene clone —
        // with no Update-system frame lag. That lag was the cause of the Play /
        // project-load crashes: a camera rendered one frame in deferred mode
        // without a deferred prepass (or without the STORAGE_BINDING texture)
        // before a lagging system could fix it, and Bevy crashes hard on that.
        app.add_observer(on_solari_lighting_inserted);
        app.add_observer(on_camera3d_inserted);
        app.add_systems(
            Update,
            (
                sync_solari_cameras,
                manage_solari_render_mode,
                mirror_raytracing_meshes,
                unmirror_when_idle,
                log_solari_coverage,
            ),
        );
        // Clear a one-shot `reset` request the frame after it's extracted (the
        // editor "Reset Temporal History" button sets it). `First` runs before
        // the inspector touches it, so the value survives to the render extract.
        app.add_systems(First, clear_solari_reset);
    }
}

/// Whether Solari is currently active on any camera. Maintained by
/// [`manage_solari_render_mode`] and read by [`on_camera3d_inserted`] so a
/// camera spawned mid-session is force-converted to the deferred prepass the
/// moment it appears (the global renderer method is deferred while active).
#[derive(Resource, Default)]
struct SolariActive(bool);

/// Camera setup Solari requires but doesn't auto-`require`: `Msaa::Off` and a
/// `STORAGE_BINDING` main texture. Applied the instant `SolariLighting` is
/// inserted — covers our sync, scene load, and the play-mode scene clone — so a
/// Solari camera never renders a frame without them (which fails
/// `solari_lighting_bind_group` creation and hard-crashes the renderer).
fn on_solari_lighting_inserted(trigger: On<Insert, SolariLighting>, mut commands: Commands) {
    commands.entity(trigger.entity).try_insert((
        Msaa::Off,
        CameraMainTextureUsages::default().with(TextureUsages::STORAGE_BINDING),
    ));
}

/// While Solari is active, give any newly-inserted `Camera3d` (play camera,
/// preview/thumbnail cameras, extra viewports) the deferred prepass immediately.
/// The global renderer method is deferred while active, and a camera that
/// renders deferred materials without a deferred phase panics in
/// `queue_prepass_material_meshes`. Doing this in an observer (not an Update
/// system) closes the one-frame gap that crashed on Play / project load.
fn on_camera3d_inserted(
    trigger: On<Insert, Camera3d>,
    state: Res<SolariActive>,
    mut commands: Commands,
) {
    if state.0 {
        commands
            .entity(trigger.entity)
            .try_insert((DeferredPrepass, Msaa::Off, SolariForcedDeferred));
    }
}

/// Marker on entities whose mesh can't be ray-traced (missing tangents/UVs,
/// non-`TriangleList`, or 16-bit indices). Keeps [`mirror_raytracing_meshes`]
/// from re-inspecting the same mesh every frame. Cleared when Solari goes idle
/// so the mesh is re-evaluated if Solari is re-enabled later.
#[derive(Component)]
struct SolariMeshSkip;

/// Route [`SolariGi`] from the World-Environment source onto each camera: while
/// enabled, give the camera Bevy's `SolariLighting` (its `#[require]`s pull in
/// HDR + the deferred/depth/motion prepasses) and force `Msaa::Off`, which
/// Solari mandates. Presence-checked so we don't churn the component every frame.
fn sync_solari_cameras(
    mut commands: Commands,
    sources: Query<&SolariGi>,
    has_solari: Query<(), With<SolariLighting>>,
    routing: Res<EffectRouting>,
) {
    for (target, source_list) in routing.iter() {
        let target = *target;
        // First source entity that carries SolariGi wins (mirrors EffectRouting
        // semantics used by Lumen).
        let enabled = source_list
            .iter()
            .find_map(|&s| sources.get(s).ok())
            .map(|gi| gi.enabled)
            .unwrap_or(false);
        let present = has_solari.get(target).is_ok();

        if enabled && !present {
            if let Ok(mut ec) = commands.get_entity(target) {
                // `on_solari_lighting_inserted` adds Msaa::Off + the
                // STORAGE_BINDING main-texture usage the moment this lands.
                ec.try_insert(SolariLighting::default());
            }
        } else if !enabled && present {
            if let Ok(mut ec) = commands.get_entity(target) {
                ec.try_remove::<SolariLighting>();
                // Only restore the main-texture usage here. MSAA + the forced
                // deferred prepass are restored together by
                // `manage_solari_render_mode` once no camera is active, so they
                // flip back in the SAME frame — otherwise restoring MSAA while a
                // camera still has DeferredPrepass triggers Bevy's
                // "MSAA incompatible with deferred rendering" warning.
                ec.try_insert(CameraMainTextureUsages::default());
            }
        }
    }
}

/// Marker on cameras we forced into the deferred prepass while Solari is active,
/// so they can be reverted when it goes idle.
#[derive(Component)]
struct SolariForcedDeferred;

/// Solari needs **deferred** materials (it reads the G-buffer), and Bevy's
/// renderer method is **global** — so while Solari is active EVERY 3d camera must
/// carry a deferred prepass, or it panics in `queue_prepass_material_meshes`
/// (a deferred material with no deferred phase). This flips
/// `DefaultOpaqueRendererMethod` to deferred and forces the deferred prepass (+
/// `Msaa::Off`) onto every `Camera3d` while any camera runs Solari, then reverts
/// to forward when none do.
///
/// Consequence: the whole viewport — and any preview/thumbnail cameras — render
/// deferred while Solari is on. That is inherent to how Bevy Solari works (it
/// sets the global deferred method itself); we only scope it to "while active"
/// and make it consistent across cameras so nothing crashes.
fn manage_solari_render_mode(
    mut commands: Commands,
    mut method: ResMut<DefaultOpaqueRendererMethod>,
    mut state: ResMut<SolariActive>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    active: Query<(), With<SolariLighting>>,
    unforced_cameras: Query<Entity, (With<Camera3d>, Without<SolariForcedDeferred>)>,
    forced_cameras: Query<Entity, With<SolariForcedDeferred>>,
) {
    if !active.is_empty() {
        if !state.0 {
            *method = DefaultOpaqueRendererMethod::deferred();
            state.0 = true;
            force_material_respecialization(&mut materials);
        }
        // Sweep every EXISTING 3d camera on activation (the observer handles ones
        // spawned later) so none renders deferred materials without a deferred
        // phase. Only DeferredPrepass (the deferred phase keys on it) + Msaa::Off
        // — DON'T touch DepthPrepass, which Renzora manages for SSAO/SSR/SSGI.
        for cam in &unforced_cameras {
            commands
                .entity(cam)
                .try_insert((DeferredPrepass, Msaa::Off, SolariForcedDeferred));
        }
    } else if state.0 {
        *method = DefaultOpaqueRendererMethod::forward();
        state.0 = false;
        force_material_respecialization(&mut materials);
        for cam in &forced_cameras {
            // try_* variants: a camera may despawn between query and apply.
            commands
                .entity(cam)
                .try_remove::<(DeferredPrepass, SolariForcedDeferred)>();
            commands.entity(cam).try_insert(Msaa::default());
        }
    }
}

/// Mark every `StandardMaterial` modified so Bevy re-runs `prepare_materials` and
/// re-resolves each material's render method against the CURRENT
/// [`DefaultOpaqueRendererMethod`].
///
/// Bevy caches the forward/deferred choice when a material is first prepared and
/// does NOT revisit it when the global default changes. So flipping the method
/// leaves already-loaded materials specialized the old way: forward-specialized
/// materials never write Solari's G-buffer (the "no materials until you toggle
/// SSR" bug on load), and deferred-specialized materials stay broken after Solari
/// is turned off. Re-touching them is exactly what toggling SSR did by hand.
fn force_material_respecialization(materials: &mut Assets<StandardMaterial>) {
    let n = materials.iter_mut().count();
    debug!("[solari] re-specialized {n} StandardMaterials for the new render method");
}

/// While Solari is active on any camera, mirror conforming meshes into the
/// ray-tracing scene. `RaytracingMesh3d` coexists with the rasterized `Mesh3d`;
/// Solari builds a BLAS from it. Meshes that don't meet Solari's requirements
/// are marked [`SolariMeshSkip`] and left out (rather than crashing the BLAS
/// builder). Not-yet-loaded meshes are retried next frame.
fn mirror_raytracing_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    active: Query<(), With<SolariLighting>>,
    candidates: Query<
        (Entity, &Mesh3d),
        (
            With<MeshMaterial3d<StandardMaterial>>,
            Without<RaytracingMesh3d>,
            Without<SolariMeshSkip>,
        ),
    >,
) {
    if active.is_empty() {
        return;
    }
    for (entity, mesh3d) in &candidates {
        let handle = &mesh3d.0;
        let Some(mesh) = meshes.get(handle) else {
            continue; // asset still loading — try again next frame
        };
        if !mesh_base_raytraceable(mesh) {
            commands.entity(entity).try_insert(SolariMeshSkip);
            continue;
        }

        // Decide (immutably) what the mesh is missing for Solari's BLAS, so we
        // only take the mutable borrow (and trigger asset re-extraction) when
        // there's actually work to do.
        let needs_tangents = mesh.attribute(Mesh::ATTRIBUTE_TANGENT).is_none();
        let needs_u32 = matches!(mesh.indices(), Some(Indices::U16(_)));
        let needs_flag = !mesh.enable_raytracing;

        if needs_tangents || needs_u32 || needs_flag {
            let Some(mut mesh) = meshes.get_mut(handle) else {
                continue;
            };
            // Solari requires 32-bit indices — promote U16 in place.
            if needs_u32 {
                if let Some(Indices::U16(u16s)) = mesh.indices() {
                    let u32s: Vec<u32> = u16s.iter().map(|&i| i as u32).collect();
                    mesh.insert_indices(Indices::U32(u32s));
                }
            }
            // Generate tangents from UV+normals (the base checks guarantee both
            // plus indexed TriangleList). Most imported GLBs lack tangents, and
            // without them Solari's ray-tracing scene is near-empty and the whole
            // view renders almost black — so generate rather than skip. If it
            // genuinely can't (degenerate UVs), leave the mesh out.
            if needs_tangents && mesh.generate_tangents().is_err() {
                warn!("[solari] mesh excluded from ray tracing: tangent generation failed (degenerate/missing UVs)");
                commands.entity(entity).try_insert(SolariMeshSkip);
                continue;
            }
            mesh.enable_raytracing = true;
        }

        // try_insert: the entity may despawn between this query and command
        // apply (scene reloads / asset streaming churn entities constantly in
        // the editor); a plain insert would panic on the dead entity.
        commands
            .entity(entity)
            .try_insert(RaytracingMesh3d(handle.clone()));
    }
}

/// When Solari is no longer active on any camera, drop the ray-tracing mirror so
/// the BLAS resources are freed and meshes are re-evaluated if it's re-enabled.
fn unmirror_when_idle(
    mut commands: Commands,
    active: Query<(), With<SolariLighting>>,
    mirrored: Query<Entity, With<RaytracingMesh3d>>,
    skipped: Query<Entity, With<SolariMeshSkip>>,
) {
    if !active.is_empty() {
        return;
    }
    for e in &mirrored {
        commands.entity(e).try_remove::<RaytracingMesh3d>();
    }
    for e in &skipped {
        commands.entity(e).try_remove::<SolariMeshSkip>();
    }
}

/// Flip `SolariLighting.reset` back off after a reset was requested (the editor
/// "Reset Temporal History" button sets it true). Runs in `First` so the flag is
/// still set when the render world extracts it at the end of the frame it was
/// pressed, then clears the next frame — a single one-shot reset. Only writes
/// when set, to avoid per-frame change-detection churn.
fn clear_solari_reset(mut cameras: Query<&mut SolariLighting>) {
    for mut s in &mut cameras {
        if s.reset {
            s.reset = false;
        }
    }
}

/// Diagnostic: log the ray-tracing scene coverage (meshes mirrored vs skipped)
/// whenever the tallies change while Solari is active. A high skip count points
/// at geometry Solari can't use (non-`StandardMaterial`, no UVs, or tangent
/// generation that failed) — useful when chasing dark/unlit surfaces.
fn log_solari_coverage(
    active: Query<(), With<SolariLighting>>,
    mirrored: Query<(), With<RaytracingMesh3d>>,
    skipped: Query<(), With<SolariMeshSkip>>,
    mut last: Local<Option<(usize, usize)>>,
) {
    if active.is_empty() {
        return;
    }
    let now = (mirrored.iter().count(), skipped.iter().count());
    if *last != Some(now) {
        info!(
            "[solari] ray-tracing scene: {} meshes mirrored, {} skipped",
            now.0, now.1
        );
        *last = Some(now);
    }
}

/// The requirements Solari's BLAS builder needs that we can't synthesize:
/// indexed `TriangleList` geometry with positions, normals, and UVs. Tangents
/// and 32-bit indices are handled on the fly (generated / promoted) in
/// [`mirror_raytracing_meshes`], so they're intentionally NOT checked here.
fn mesh_base_raytraceable(mesh: &Mesh) -> bool {
    mesh.primitive_topology() == PrimitiveTopology::TriangleList
        && mesh.indices().is_some()
        && mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some()
        && mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some()
        && mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_some()
}

renzora::add!(SolariPlugin);
