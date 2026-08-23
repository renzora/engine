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
//!
//! ## What gets mirrored, and why the filter is not optional
//!
//! Bevy's ray-tracing scene has no notion of visibility or render layers:
//! `prepare_raytracing_scene_bindings` puts *every* entity carrying
//! `RaytracingMesh3d` into the TLAS at full ray mask. The editor, meanwhile,
//! keeps a lot of geometry in the same `World` that the viewport never shows —
//! offscreen thumbnail/preview rigs and gizmo meshes, separated only by
//! `RenderLayers` — so mirroring indiscriminately turns all of it into invisible
//! shadow casters standing in the middle of the level. See
//! [`in_raytraced_scene`].

use bevy::prelude::*;
use bevy::camera::visibility::{InheritedVisibility, RenderLayers, VisibilitySystems};
use bevy::camera::CameraMainTextureUsages;
use bevy::core_pipeline::prepass::DeferredPrepass;
use bevy::ecs::system::SystemParam;
use bevy::light::{EnvironmentMapLight, GeneratedEnvironmentMapLight, PointLight, SpotLight};
use bevy::mesh::{Indices, MeshVertexAttributeId, PrimitiveTopology};
use bevy::platform::collections::HashMap;
use bevy::pbr::{
    extract_lights, DefaultOpaqueRendererMethod, ExtractedDirectionalLight, ExtractedPointLight,
};
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::{ExtractSchedule, RenderApp};
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
        app.init_resource::<RaytracingProxies>();
        app.init_resource::<LightProxies>();
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
                sync_shadow_map_suppression,
            ),
        );
        app.init_resource::<SuppressShadowMaps>();
        app.add_plugins(ExtractResourcePlugin::<SuppressShadowMaps>::default());
        // Applied in the render world, to the *extracted* lights, so the main
        // world's light components are never written. See `SuppressShadowMaps`.
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(ExtractSchedule, suppress_shadow_maps.after(extract_lights));
        }
        // The mesh systems READ `InheritedVisibility`, which Bevy writes in
        // `PostUpdate`/`VisibilityPropagate`. Run them in `Update` and every
        // value is a frame stale — and worse, an entity spawned this frame has
        // never been propagated at all, so it still holds the `HIDDEN` default.
        // A scene load would then look entirely hidden on its first frame and
        // mirror nothing. Ordering after the propagation removes both problems.
        app.add_systems(
            PostUpdate,
            (
                invalidate_stale_proxies,
                mirror_raytracing_meshes,
                unmirror_out_of_scene_meshes,
                unmirror_when_idle,
                sync_light_proxies,
                log_solari_coverage,
                warn_unsupported_lights,
                warn_missing_ambient_sources,
            )
                .chain()
                .after(VisibilitySystems::VisibilityPropagate),
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

/// Whether an entity's geometry belongs in the ray-traced scene at all.
///
/// Bevy's ray-tracing scene is *global and unfiltered*: `extract_raytracing_scene`
/// and `prepare_raytracing_scene_bindings` query on `RaytracingMesh3d` alone,
/// build every instance with ray mask `0xFF`, and never look at `Visibility`,
/// `ViewVisibility` or `RenderLayers`. Whatever we mirror occludes and bounces
/// light, whether or not any camera draws it.
///
/// That collides head-on with how the editor stages offscreen work. Layer 0 is
/// the scene; layer 1 and up are overlays and rigs living in the *same* `World`
/// at, or right next to, the world origin:
///
/// * `renzora_asset_browser`'s model thumbnails spawn whole GLBs into capture
///   cells, and cell 0 is exactly `(0, 0, 0)`;
/// * `renzora_hub`'s material viewer and `renzora_animation_editor`'s studio
///   preview park a subject plus a backdrop plane around the origin;
/// * `renzora_gizmo` draws its handle meshes on layers 1 and up.
///
/// Mirrored unfiltered, those become solid invisible geometry sitting inside the
/// level: a fixed dark blotch that the scene slides under as you drag it, which
/// reads exactly like baked-in shadowing. Hence both gates:
///
/// * **`InheritedVisibility`** — hierarchy-propagated `Visibility`, deliberately
///   *not* `ViewVisibility`. Off-screen geometry must stay in the BVH; shadowing
///   and bounce from outside the frustum is the entire point of ray tracing, so
///   frustum culling must never remove an instance. Only an explicit
///   `Visibility::Hidden` (the hierarchy eye toggle, the drag-and-drop model
///   ghost) may.
/// * **layer 0** — the scene layer, which drops every preview rig and gizmo mesh.
fn in_raytraced_scene(visibility: &InheritedVisibility, layers: Option<&RenderLayers>) -> bool {
    visibility.get() && layers.is_none_or(|l| l.intersects(&RenderLayers::default()))
}

/// Whether a material may be traced as the opaque surface Solari assumes.
///
/// `bevy_solari` 0.19 has **no alpha handling whatsoever** — `GpuMaterial` has no
/// alpha channel and instances are built with ray mask `0xFF` — so a blended
/// material is a solid wall to every shadow and GI ray. Glass, water, decals and
/// light shafts would cast hard black shadows they should barely tint. Worse,
/// while Solari is on those materials don't render at all (the global renderer
/// method is deferred, which can't draw them), so the viewport shows *nothing*
/// throwing a solid shadow. Leaving them out of the BVH is much the closer
/// approximation.
///
/// `Mask` and `AlphaToCoverage` stay in on purpose: a cutout leaf traced as a
/// full quad over-shadows, but dropping it removes tree shadows altogether,
/// which reads worse. Doing it properly needs alpha-tested any-hit shaders,
/// which Bevy doesn't expose.
///
/// **Emissive surfaces are always kept, whatever their alpha mode.** Solari
/// registers an emissive mesh as an area light only if that mesh is a live TLAS
/// instance, so excluding a blended one doesn't just stop it occluding — it
/// deletes a light from the scene. Neon tubes and lamp glass are exactly the
/// blended-and-emissive combination that matters, and with no point-light
/// support (see [`warn_unsupported_lights`]) they are often the only thing
/// lighting a street at all. Their occlusion cost is negligible by comparison.
fn material_is_raytraceable(material: &StandardMaterial) -> bool {
    matches!(
        material.alpha_mode,
        AlphaMode::Opaque | AlphaMode::Mask(_) | AlphaMode::AlphaToCoverage
    ) || is_emissive(material)
}

/// Matches the test `bevy_solari`'s binder uses to decide whether a mesh becomes
/// an emissive area light.
fn is_emissive(material: &StandardMaterial) -> bool {
    let emissive = material.emissive;
    emissive.red != 0.0 || emissive.green != 0.0 || emissive.blue != 0.0
}

/// Both eligibility gates in one verdict, or `None` while the material asset is
/// still loading and the question can't be answered yet.
///
/// The three callers want different things from that `None`, which is why it
/// isn't collapsed to a bool here: the mirror retries next frame, the un-mirror
/// leaves an existing instance in place (asset churn shouldn't make the BVH
/// flicker), and the diagnostic doesn't count it either way.
fn is_raytraceable_instance(
    materials: &Assets<StandardMaterial>,
    material: &MeshMaterial3d<StandardMaterial>,
    visibility: &InheritedVisibility,
    layers: Option<&RenderLayers>,
) -> Option<bool> {
    if !in_raytraced_scene(visibility, layers) {
        return Some(false);
    }
    Some(material_is_raytraceable(materials.get(&material.0)?))
}

/// The query data [`is_raytraceable_instance`] needs. Aliased because all three
/// systems below fetch exactly this, and spelled out inline it trips clippy's
/// `type_complexity` (which CI runs as `-D warnings`).
type SceneSet = (
    &'static MeshMaterial3d<StandardMaterial>,
    &'static InheritedVisibility,
    Option<&'static RenderLayers>,
);

/// Meshes eligible to be mirrored but not mirrored yet: not already in the BVH,
/// and not known-bad geometry.
type Unmirrored = (Without<RaytracingMesh3d>, Without<SolariMeshSkip>);

/// The complement of [`Unmirrored`]: meshes we have already made a decision
/// about, either way.
type Decided = Or<(With<RaytracingMesh3d>, With<SolariMeshSkip>)>;

/// Cameras running Solari that also carry an image-based light — the baked
/// atmosphere IBL, or an explicit environment map. Aliased to keep clippy's
/// `type_complexity` lint quiet, which CI runs as `-D warnings`.
type ImageBasedLit = (
    With<SolariLighting>,
    Or<(
        With<EnvironmentMapLight>,
        With<GeneratedEnvironmentMapLight>,
    )>,
);

/// [`SceneSet`] minus the material, for lights — same visibility and layer
/// reasoning, but a light has no material to check.
type SceneSetLight = (
    &'static InheritedVisibility,
    Option<&'static RenderLayers>,
);

/// While Solari is active on any camera, mirror conforming meshes into the
/// ray-tracing scene. `RaytracingMesh3d` coexists with the rasterized `Mesh3d`;
/// Solari builds a BLAS from it. Meshes that don't meet Solari's requirements
/// are marked [`SolariMeshSkip`] and left out (rather than crashing the BLAS
/// builder). Not-yet-loaded meshes are retried next frame.
///
/// [`in_raytraced_scene`] and [`material_is_raytraceable`] gate what is eligible
/// at all. Those two are checked fresh every frame rather than cached behind
/// [`SolariMeshSkip`], because both are things the user changes while the editor
/// is running — unhiding an entity or switching a material off `Blend` has to put
/// the mesh back into the BVH. [`unmirror_out_of_scene_meshes`] handles the
/// other direction.
fn mirror_raytracing_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut proxies: ResMut<RaytracingProxies>,
    materials: Res<Assets<StandardMaterial>>,
    active: Query<(), With<SolariLighting>>,
    candidates: Query<(Entity, &Mesh3d, SceneSet), Unmirrored>,
) {
    if active.is_empty() {
        return;
    }
    for (entity, mesh3d, (material, visibility, layers)) in &candidates {
        // Anything but a definite yes is left alone WITHOUT `SolariMeshSkip`:
        // ineligible is re-checked every frame, and a still-loading material is
        // simply retried, same as a not-yet-loaded mesh below.
        if is_raytraceable_instance(&materials, material, visibility, layers) != Some(true) {
            continue;
        }
        let handle = &mesh3d.0;
        let Some(mesh) = meshes.get(handle) else {
            continue; // asset still loading — try again next frame
        };
        if !mesh_base_raytraceable(mesh) {
            commands.entity(entity).try_insert(SolariMeshSkip);
            continue;
        }

        // Two routes to geometry the BLAS builder will accept. Prefer repairing
        // the source in place — it costs no extra memory and is all most GLBs
        // need — and fall back to a stripped copy for meshes whose *extra*
        // attributes are the problem, since those can't be dropped from the
        // shared asset without breaking the rasterized draw.
        let traced = if source_can_conform(mesh) {
            // Decide (immutably) what's missing, so we only take the mutable
            // borrow (and trigger asset re-extraction) when there's work to do.
            let needs_tangents = mesh.attribute(Mesh::ATTRIBUTE_TANGENT).is_none();
            let needs_u32 = matches!(mesh.indices(), Some(Indices::U16(_)));
            let needs_flag = !mesh.enable_raytracing;

            if needs_tangents || needs_u32 || needs_flag {
                let Some(mut mesh) = meshes.get_mut(handle) else {
                    continue;
                };
                if needs_u32 {
                    promote_indices_to_u32(&mut mesh);
                }
                // Generate tangents from UV+normals (the base checks guarantee
                // both, plus indexed TriangleList). Most imported GLBs lack
                // tangents, and without them the ray-tracing scene is near-empty
                // and the whole view renders almost black — so generate rather
                // than skip. If it genuinely can't (degenerate UVs), leave the
                // mesh out.
                if needs_tangents && mesh.generate_tangents().is_err() {
                    warn!("[solari] mesh excluded from ray tracing: tangent generation failed (degenerate/missing UVs)");
                    commands.entity(entity).try_insert(SolariMeshSkip);
                    continue;
                }
                mesh.enable_raytracing = true;
            }
            handle.clone()
        } else if let Some(cached) = proxies.0.get(&handle.id()).cloned() {
            // A cached `None` is a source we already failed to convert; don't
            // retry (and re-warn about) it every frame.
            let Some(proxy) = cached else {
                commands.entity(entity).try_insert(SolariMeshSkip);
                continue;
            };
            proxy
        } else {
            // `build_raytracing_proxy` returns an owned mesh, which ends the
            // immutable borrow of `meshes` that `mesh` holds — so the insert
            // below is allowed.
            let built = build_raytracing_proxy(mesh);
            let Some(proxy) = built else {
                warn!(
                    "[solari] mesh excluded from ray tracing: no BLAS-compatible copy could be \
                     built (tangent generation failed on degenerate UVs)"
                );
                proxies.0.insert(handle.id(), None);
                commands.entity(entity).try_insert(SolariMeshSkip);
                continue;
            };
            let proxy = meshes.add(proxy);
            proxies.0.insert(handle.id(), Some(proxy.clone()));
            proxy
        };

        // try_insert: the entity may despawn between this query and command
        // apply (scene reloads / asset streaming churn entities constantly in
        // the editor); a plain insert would panic on the dead entity.
        commands
            .entity(entity)
            .try_insert(RaytracingMesh3d(traced));
    }
}

/// Throw away a cached proxy when its source mesh changes, and un-mirror the
/// entities using it so [`mirror_raytracing_meshes`] rebuilds from the new
/// geometry.
///
/// Without this a sculpted or procedurally regenerated mesh would keep tracing
/// against the copy taken the first time it was seen: the raster view would
/// update and the lighting would not. Meshes repaired in place don't need this —
/// they and the BLAS both follow the asset — so only the proxy path is affected.
///
/// `SolariMeshSkip` is cleared too, since a mesh edit can perfectly well turn
/// unusable geometry into usable geometry.
fn invalidate_stale_proxies(
    mut commands: Commands,
    mut events: MessageReader<AssetEvent<Mesh>>,
    mut proxies: ResMut<RaytracingProxies>,
    mirrored: Query<(Entity, &Mesh3d), Decided>,
) {
    let stale: Vec<AssetId<Mesh>> = events
        .read()
        .filter_map(|event| match event {
            AssetEvent::Modified { id } | AssetEvent::Removed { id } => Some(*id),
            _ => None,
        })
        // Only meshes we actually made a copy of; everything else is either
        // repaired in place or was never traced.
        .filter(|id| proxies.0.remove(id).is_some())
        .collect();
    if stale.is_empty() {
        return;
    }
    for (entity, mesh3d) in &mirrored {
        if stale.contains(&mesh3d.0.id()) {
            commands
                .entity(entity)
                .try_remove::<(RaytracingMesh3d, SolariMeshSkip)>();
        }
    }
}

/// Drop the ray-tracing mirror from anything that has stopped qualifying for it
/// while Solari is still running: an entity hidden with the hierarchy eye, one
/// moved off layer 0, or a material switched to a blended alpha mode.
///
/// [`mirror_raytracing_meshes`] can't do this itself — it filters on
/// `Without<RaytracingMesh3d>`, so once a mesh is mirrored it never looks at it
/// again. Without this counterpart, hiding an object would leave it casting
/// ray-traced shadows for the rest of the session.
fn unmirror_out_of_scene_meshes(
    mut commands: Commands,
    materials: Res<Assets<StandardMaterial>>,
    active: Query<(), With<SolariLighting>>,
    mirrored: Query<(Entity, SceneSet), (With<RaytracingMesh3d>, Without<SolariLightProxy>)>,
) {
    if active.is_empty() {
        return;
    }
    for (entity, (material, visibility, layers)) in &mirrored {
        // Only a definite no un-mirrors. An unloaded material is transient asset
        // churn, not an authoring change, and dropping the instance for those
        // frames would make the BVH flicker;
        // `prepare_raytracing_scene_bindings` already skips it meanwhile.
        if is_raytraceable_instance(&materials, material, visibility, layers) == Some(false) {
            commands.entity(entity).try_remove::<RaytracingMesh3d>();
        }
    }
}

/// When Solari is no longer active on any camera, drop the ray-tracing mirror so
/// the BLAS resources are freed and meshes are re-evaluated if it's re-enabled.
fn unmirror_when_idle(
    mut commands: Commands,
    mut proxies: ResMut<RaytracingProxies>,
    active: Query<(), With<SolariLighting>>,
    mirrored: Query<Entity, With<RaytracingMesh3d>>,
    skipped: Query<Entity, With<SolariMeshSkip>>,
) {
    if !active.is_empty() {
        return;
    }
    // Dropping the handles frees the duplicated vertex buffers; they are rebuilt
    // if Solari is switched back on.
    if !proxies.0.is_empty() {
        proxies.0.clear();
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

/// Whether shadow-map rendering should be suppressed this frame: Solari is
/// lighting, and the user hasn't turned the [`SolariGi::suppress_shadow_maps`]
/// toggle off.
///
/// Extracted to the render world, where [`suppress_shadow_maps`] acts on it.
#[derive(Resource, Clone, Copy, Default, ExtractResource)]
struct SuppressShadowMaps(bool);

/// Track whether shadow maps should be suppressed, for the render world to read.
///
/// Suppression needs *both* halves to be true. Solari being active isn't enough
/// on its own, because the toggle exists precisely so you can keep raster
/// shadows and compare.
fn sync_shadow_map_suppression(
    active: Query<(), With<SolariLighting>>,
    sources: Query<&SolariGi>,
    mut suppress: ResMut<SuppressShadowMaps>,
) {
    let want = !active.is_empty()
        && sources
            .iter()
            .any(|gi| gi.enabled && gi.suppress_shadow_maps);
    // Only write on a change; this resource is extracted every frame and a
    // needless write would mark it changed for every consumer.
    if suppress.0 != want {
        suppress.0 = want;
    }
}

/// Clear `shadow_maps_enabled` on the extracted lights so Bevy queues no shadow
/// passes at all while Solari is lighting.
///
/// Solari traces visibility instead of sampling shadow maps, and a Solari camera
/// carries `SkipDeferredLighting` — which removes the only pass that would read
/// one. So without this every directional cascade and every point-light cubemap
/// is rendered in full and then discarded. Bevy's `SolariLightingPlugin` docs
/// say as much: "it's highly recommended to set `shadow_maps_enabled: false` on
/// all lights, as Solari replaces traditional shadow mapping."
///
/// **Why the render world and not the main world.** Flipping the real
/// `PointLight`/`DirectionalLight` components would also skip the main-world
/// per-light mesh culling, which is a bigger saving — but those components are
/// serialized, so a scene saved while Solari happened to be on would silently
/// persist shadows-off and stay broken after Solari was turned back off. The
/// extracted copies are rebuilt from scratch every frame and never written to
/// disk, so acting on them is free of that whole class of problem.
///
/// Solari's own lighting is unaffected: its binder reads colour, illuminance,
/// transform and sun-disk size from `ExtractedDirectionalLight`, never
/// `shadow_maps_enabled`.
///
/// Note this is global rather than per-camera, in the same way Solari's deferred
/// renderer method is: while it's on, no camera renders shadow maps.
fn suppress_shadow_maps(
    suppress: Res<SuppressShadowMaps>,
    mut directional: Query<&mut ExtractedDirectionalLight>,
    mut point: Query<&mut ExtractedPointLight>,
) {
    if !suppress.0 {
        return;
    }
    for mut light in &mut directional {
        if light.shadow_maps_enabled {
            light.shadow_maps_enabled = false;
        }
    }
    // Spot lights extract as `ExtractedPointLight` with `spot_light_angles` set,
    // so this covers them too.
    for mut light in &mut point {
        if light.shadow_maps_enabled {
            light.shadow_maps_enabled = false;
        }
    }
}

/// Marks an emissive sphere standing in for a point or spot light. Owned
/// entirely by [`sync_light_proxies`]; nothing else should touch these.
#[derive(Component)]
struct SolariLightProxy;

/// The traced-only spheres standing in for point and spot lights, and the unit
/// sphere mesh they all share.
#[derive(Resource, Default)]
struct LightProxies {
    sphere: Option<Handle<Mesh>>,
    /// light entity -> (proxy entity, its emissive material)
    proxies: HashMap<Entity, (Entity, Handle<StandardMaterial>)>,
}

/// The smallest sphere we will stand in for a light, in metres.
///
/// Radiance goes as `1/r²`, so a light left at Bevy's default `radius: 0.0`
/// would need infinite radiance to carry its power. Clamping to a plausible
/// bulb size keeps the conversion finite and the sampling well-conditioned.
const MIN_PROXY_RADIUS: f32 = 0.02;

/// Emissive radiance, in cd/m², for a sphere of `radius` carrying `lumens` of
/// luminous power in `colour`.
///
/// Bevy stores `PointLight::intensity` and `SpotLight::intensity` as luminous
/// power in lumens and converts to luminous intensity with `Φ / 4π`
/// (`bevy_pbr`'s `extract_lights`). A uniformly-emitting sphere of radius `r`
/// and radiance `L` has intensity `I = L · π r²` in every direction, so
/// matching the two gives:
///
/// ```text
///   L · π r² = Φ / 4π      =>      L = Φ / (4 π² r²)
/// ```
///
/// The result is a real photometric quantity, which is what Solari wants:
/// `StandardMaterial::emissive` is used directly as radiance, and the view's
/// exposure is applied afterwards exactly as it is for the sun.
fn proxy_emissive(colour: LinearRgba, lumens: f32, radius: f32) -> LinearRgba {
    let radiance = lumens / (4.0 * core::f32::consts::PI * core::f32::consts::PI * radius * radius);
    LinearRgba::rgb(
        colour.red * radiance,
        colour.green * radiance,
        colour.blue * radiance,
    )
}

/// A unit sphere that satisfies Solari's BLAS rules exactly.
fn build_proxy_sphere() -> Option<Mesh> {
    let mut mesh = Sphere::new(1.0).mesh().ico(2).ok()?;
    promote_indices_to_u32(&mut mesh);
    if mesh.attribute(Mesh::ATTRIBUTE_TANGENT).is_none() && mesh.generate_tangents().is_err() {
        return None;
    }
    mesh.enable_raytracing = true;
    Some(mesh)
}

/// Stand each point and spot light up as an emissive sphere in the ray-tracing
/// scene, so Solari has something to sample.
///
/// Solari knows two light kinds — directional, and emissive mesh — and a Solari
/// camera carries `SkipDeferredLighting`, which removes Bevy's clustered-light
/// pass along with the deferred lighting it does. Between them, a `PointLight`
/// contributes nothing at all: not dimmed, not approximated, absent. An emissive
/// mesh, though, is a first-class area light, so we give each light one.
///
/// The proxy carries `RaytracingMesh3d` **without** `Mesh3d`. That is the whole
/// trick: `RaytracingMesh3d` requires only a material, a transform and
/// `SyncToRenderWorld`, so the sphere is real to the ray tracer and does not
/// exist as far as the rasterizer is concerned — no glowing ball appears in the
/// viewport, and nothing is added to the user's scene.
///
/// Proxies are spawned as roots, not as children of the light, so the user's
/// hierarchy is never modified; the light's `GlobalTransform` is copied across
/// instead. They carry no `Name`, so scene save (which only serializes named
/// entities) ignores them.
///
/// Approximations worth knowing: a spot light becomes omnidirectional, since a
/// sphere can't carry a cone — Bevy applies the cone as an angular mask on top
/// of the same `Φ / 4π` intensity, so the in-cone brightness is right and the
/// out-of-cone spill is new. `AmbientLight` has no equivalent at all: it would
/// need an enclosing emissive dome, which would then occlude the sun.
fn sync_light_proxies(
    mut commands: Commands,
    mut store: LightProxyStore,
    active: Query<(), With<SolariLighting>>,
    sources: Query<&SolariGi>,
    point_lights: Query<(Entity, &PointLight, &GlobalTransform, SceneSetLight)>,
    spot_lights: Query<(Entity, &SpotLight, &GlobalTransform, SceneSetLight)>,
) {
    let wanted = !active.is_empty() && sources.iter().any(|gi| gi.enabled && gi.light_proxies);
    if !wanted {
        if !store.proxies.proxies.is_empty() {
            for (proxy, _) in store.proxies.proxies.values() {
                commands.entity(*proxy).try_despawn();
            }
            store.proxies.proxies.clear();
        }
        return;
    }

    if store.proxies.sphere.is_none() {
        let Some(sphere) = build_proxy_sphere() else {
            warn!("[solari] could not build the light-proxy sphere; point/spot lights stay unlit");
            return;
        };
        let handle = store.meshes.add(sphere);
        store.proxies.sphere = Some(handle);
    }

    let mut live = Vec::new();
    for (entity, light, transform, (visibility, layers)) in &point_lights {
        if upsert_light_proxy(
            &mut store,
            &mut commands,
            entity,
            light.color.into(),
            light.intensity,
            light.radius,
            transform,
            visibility,
            layers,
        ) {
            live.push(entity);
        }
    }
    for (entity, light, transform, (visibility, layers)) in &spot_lights {
        if upsert_light_proxy(
            &mut store,
            &mut commands,
            entity,
            light.color.into(),
            light.intensity,
            light.radius,
            transform,
            visibility,
            layers,
        ) {
            live.push(entity);
        }
    }

    // Drop proxies whose light was deleted, hidden, or moved off layer 0.
    store.proxies.proxies.retain(|light, (proxy, _)| {
        let keep = live.contains(light);
        if !keep {
            commands.entity(*proxy).try_despawn();
        }
        keep
    });
}

/// The asset stores [`sync_light_proxies`] writes through, grouped so the system
/// stays under clippy's argument limit.
#[derive(SystemParam)]
struct LightProxyStore<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    proxies: ResMut<'w, LightProxies>,
}

/// Create or refresh one light's proxy. Returns whether the light should have
/// one at all — `false` means any existing proxy is due to be dropped.
#[expect(
    clippy::too_many_arguments,
    reason = "one call site; splitting the light's photometric parameters into a \
              struct would only move the noise"
)]
fn upsert_light_proxy(
    store: &mut LightProxyStore,
    commands: &mut Commands,
    light: Entity,
    colour: LinearRgba,
    lumens: f32,
    radius: f32,
    transform: &GlobalTransform,
    visibility: &InheritedVisibility,
    layers: Option<&RenderLayers>,
) -> bool {
    // A hidden light, or one on a preview rig's layer, must not light the scene
    // — the same reasoning as `in_raytraced_scene`.
    if !in_raytraced_scene(visibility, layers) {
        return false;
    }
    let Some(sphere) = store.proxies.sphere.clone() else {
        return false;
    };

    let radius = radius.max(MIN_PROXY_RADIUS);
    let emissive = proxy_emissive(colour, lumens, radius);
    // Scale the unit sphere to the light's radius, positioned where it is.
    let placement = transform.mul_transform(Transform::from_scale(Vec3::splat(radius)));

    match store.proxies.proxies.get(&light) {
        Some((proxy, material_handle)) => {
            if let Some(mut material) = store.materials.get_mut(material_handle) {
                // Guarded so a static light doesn't mark its material changed
                // every frame — Solari re-uploads every `StandardMaterial` that
                // does.
                if material.emissive != emissive {
                    material.emissive = emissive;
                }
            }
            // Written every frame: a moving lamp has to drag its light with it,
            // and `GlobalTransform` too, because ours is a root entity whose
            // propagation has already run by the time we get here.
            commands
                .entity(*proxy)
                .try_insert((Transform::from(placement), placement));
        }
        None => {
            let material = store.materials.add(StandardMaterial {
                // Black, so the sphere reflects nothing — it is a light, not a
                // surface. Opaque keeps it in the BVH unconditionally.
                base_color: Color::BLACK,
                emissive,
                alpha_mode: AlphaMode::Opaque,
                ..default()
            });
            let proxy = commands
                .spawn((
                    SolariLightProxy,
                    RaytracingMesh3d(sphere),
                    MeshMaterial3d(material.clone()),
                    Transform::from(placement),
                    placement,
                ))
                .id();
            store.proxies.proxies.insert(light, (proxy, material));
        }
    }
    true
}

/// Diagnostic: name the lights Solari silently ignores.
///
/// `bevy_solari` 0.19 knows exactly two kinds of light — see
/// `LIGHT_SOURCE_KIND_DIRECTIONAL` and `LIGHT_SOURCE_KIND_EMISSIVE_MESH` in its
/// `raytracing_scene_bindings.wgsl`. **Point and spot lights contribute
/// nothing.** They are not dimmed or approximated; the binder never looks at
/// them.
///
/// There is no way for a user to discover that from the viewport: a street full
/// of lamp posts simply comes out dark, and the obvious conclusion is that GI is
/// broken rather than that the lamps aren't lights any more. Hence a warning
/// that says so and points at the one workaround that does work — an emissive
/// material on the bulb geometry, which Solari samples as a real area light.
fn warn_unsupported_lights(
    active: Query<(), With<SolariLighting>>,
    sources: Query<&SolariGi>,
    directional: Query<(), With<DirectionalLight>>,
    point: Query<(), With<PointLight>>,
    spot: Query<(), With<SpotLight>>,
    proxies: Res<LightProxies>,
    mut last: Local<Option<(usize, usize, usize, usize)>>,
) {
    if active.is_empty() {
        return;
    }
    let proxied = proxies.proxies.len();
    let now = (
        directional.iter().count(),
        point.iter().count(),
        spot.iter().count(),
        proxied,
    );
    if *last == Some(now) {
        return;
    }
    *last = Some(now);

    let proxies_on = sources.iter().any(|gi| gi.enabled && gi.light_proxies);
    if now.1 + now.2 > 0 {
        if proxies_on {
            info!(
                "[solari] {} point + {} spot lights stood up as emissive area lights ({} \
                 proxies). Solari samples no point/spot lights of its own; a spot's cone is \
                 lost in the conversion.",
                now.1, now.2, now.3
            );
        } else {
            warn!(
                "[solari] {} point + {} spot lights contribute NOTHING — Solari samples only \
                 directional lights and emissive meshes, and light proxies are turned off. \
                 Enable \"Point/Spot Light Proxies\" on the World Environment, or give the \
                 lamp geometry an emissive material.",
                now.1, now.2
            );
        }
    }
    if now.0 == 0 {
        warn!(
            "[solari] no directional light in the scene — with no sun and no emissive meshes, \
             Solari has nothing to light from and the view will be black."
        );
    }
}

/// Warn once about the ambient and image-based light sources Solari throws away.
///
/// Unlike point and spot lights, these have no workaround. Both are applied in
/// Bevy's deferred lighting pass, which a Solari camera's `SkipDeferredLighting`
/// removes, and Solari has no ambient term and no miss-radiance hook to put them
/// back. The only shape they could take in a traced scene is an enclosing
/// emissive dome — which would then sit between every surface and the sun.
///
/// This is worth its own warning because **it is usually the single biggest
/// reason a scene looks darker under Solari**, and it is invisible in the scene
/// tree. An outdoor daylight scene gets a large share of its light from the sky:
/// the procedural atmosphere baked into an `EnvironmentMapLight` (Renzora's
/// World Environment does this by default) is what fills in every surface not
/// facing the sun. Losing it takes facades and shadowed ground close to black
/// while the sunlit ground stays correct — which reads as "GI is broken" rather
/// than "the sky stopped contributing".
fn warn_missing_ambient_sources(
    active: Query<(), With<SolariLighting>>,
    // Both are per-camera components in Bevy 0.19, so ask the Solari cameras
    // rather than looking for global resources.
    ambient: Query<&AmbientLight, With<SolariLighting>>,
    image_based: Query<(), ImageBasedLit>,
    mut warned: Local<bool>,
) {
    if active.is_empty() {
        *warned = false;
        return;
    }
    if *warned {
        return;
    }
    let has_ambient = ambient.iter().any(|a| a.brightness > 0.0);
    let has_ibl = !image_based.is_empty();
    if !has_ambient && !has_ibl {
        return;
    }
    *warned = true;
    warn!(
        "[solari] the scene's sky/ambient lighting is ignored{}{} — Solari samples only \
         directional lights and emissive meshes, so everything not facing the sun is lit by \
         bounce alone. This is inherent to bevy_solari and is usually the main reason a Solari \
         render looks much darker than the raster one.",
        if has_ibl {
            " (environment map / baked atmosphere IBL)"
        } else {
            ""
        },
        if has_ambient { " (AmbientLight)" } else { "" },
    );
}

/// Diagnostic: log the ray-tracing scene coverage whenever the tallies change
/// while Solari is active. Three numbers, because a dark or wrongly-lit scene
/// usually shows up in exactly one of them:
///
/// * **mirrored** — instances actually in the TLAS.
/// * **skipped** — geometry Solari's BLAS builder can't take (no UVs, not an
///   indexed triangle list, tangent generation failed). A high count means an
///   under-populated BVH, which renders as an almost-black scene.
/// * **excluded** — geometry deliberately held back by [`in_raytraced_scene`] /
///   [`material_is_raytraceable`]: hidden, off layer 0, or blended. Watch this
///   one while the asset browser is generating thumbnails — that is the count
///   which used to be silently *mirrored*, putting invisible models at the world
///   origin as solid shadow casters.
fn log_solari_coverage(
    active: Query<(), With<SolariLighting>>,
    materials: Res<Assets<StandardMaterial>>,
    mirrored: Query<(), With<RaytracingMesh3d>>,
    skipped: Query<(), With<SolariMeshSkip>>,
    candidates: Query<SceneSet, (With<Mesh3d>, Unmirrored)>,
    proxies: Res<RaytracingProxies>,
    mut last: Local<Option<(usize, usize, usize, usize)>>,
) {
    if active.is_empty() {
        return;
    }
    let excluded = candidates
        .iter()
        .filter(|(material, visibility, layers)| {
            is_raytraceable_instance(&materials, material, visibility, *layers) == Some(false)
        })
        .count();
    let now = (
        mirrored.iter().count(),
        proxies.0.values().flatten().count(),
        skipped.iter().count(),
        excluded,
    );
    if *last == Some(now) {
        return;
    }
    *last = Some(now);

    if now.0 == 0 {
        // Worth a warning rather than a number: with no instances the scene bind
        // group is never created, `solari_lighting` returns early, and the
        // camera's `SkipDeferredLighting` means nothing else lights the
        // G-buffer. Every opaque surface renders pure black while forward-path
        // geometry (blended foliage, glass, the sky) still looks lit — which
        // reads as a broken scene rather than as missing GI.
        warn!(
            "[solari] ray-tracing scene is EMPTY ({} skipped, {} excluded) — every opaque \
             surface will render black. Solari lights only from the G-buffer, and with no \
             acceleration structure there is no lighting pass at all.",
            now.2, now.3
        );
        return;
    }

    info!(
        "[solari] ray-tracing scene: {} meshes mirrored ({} via stripped copies), {} skipped, \
         {} excluded (hidden / off-layer / blended)",
        now.0, now.1, now.2, now.3
    );
}

/// The exact attribute list `bevy_solari`'s BLAS builder demands, in the order
/// it demands. Order matters because `Mesh` stores attributes in a `BTreeMap`
/// keyed by id, so iteration is id-sorted: POSITION 0, NORMAL 1, UV_0 2,
/// **UV_1 3**, TANGENT 4, COLOR 5, JOINT_WEIGHT 6, JOINT_INDEX 7.
const BLAS_ATTRIBUTES: [MeshVertexAttributeId; 4] = [
    Mesh::ATTRIBUTE_POSITION.id,
    Mesh::ATTRIBUTE_NORMAL.id,
    Mesh::ATTRIBUTE_UV_0.id,
    Mesh::ATTRIBUTE_TANGENT.id,
];

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

/// Whether the mesh asset itself can be made BLAS-compatible in place, i.e. its
/// attribute set is already exactly [`BLAS_ATTRIBUTES`] or that set minus the
/// tangents we can generate.
///
/// This is the check that has to be exact, and getting it wrong is silent and
/// expensive. `bevy_solari`'s `is_mesh_raytracing_compatible` gates on
/// `mesh.attributes().map(|(a, _)| a.id).eq([POSITION, NORMAL, UV_0, TANGENT])` —
/// an ordered comparison of the **whole** attribute list, not a subset test. A
/// mesh carrying anything extra fails it: a second UV set (lightmap UVs, and
/// `renzora_wind` reads UV_1), vertex colours, or skinning weights. Bevy then
/// builds no BLAS for it, logs nothing, and `prepare_raytracing_scene_bindings`
/// quietly drops every instance of it from the TLAS.
///
/// That failure mode is far worse than it sounds. If enough meshes fall out, the
/// TLAS ends up empty, the scene bind group is never created, and
/// `solari_lighting` returns early — and because a Solari camera also carries
/// `SkipDeferredLighting`, *nothing else* lights the G-buffer. Every opaque
/// surface renders pure black while forward-path geometry (blended foliage,
/// glass, the sky) still looks perfectly lit. See
/// [`build_raytracing_proxy`] for what we do about it.
fn source_can_conform(mesh: &Mesh) -> bool {
    let Ok(attributes) = mesh.try_attributes() else {
        return false; // data already moved to the render world
    };
    let ids: Vec<_> = attributes.map(|(attribute, _)| attribute.id).collect();
    ids == BLAS_ATTRIBUTES[..] || ids == BLAS_ATTRIBUTES[..3]
}

/// Promote 16-bit indices to the 32-bit ones the BLAS builder requires.
fn promote_indices_to_u32(mesh: &mut Mesh) {
    if let Some(Indices::U16(u16s)) = mesh.indices() {
        let u32s: Vec<u32> = u16s.iter().map(|&i| i as u32).collect();
        mesh.insert_indices(Indices::U32(u32s));
    }
}

/// Build a ray-tracing-only copy of a mesh whose *extra* attributes are what
/// disqualify it (see [`source_can_conform`]).
///
/// We can't strip those attributes from the shared asset: the rasterized draw
/// still needs them — UV_1 feeds `renzora_wind`, vertex colours tint materials,
/// joint weights drive skinning — and `Mesh3d` and `RaytracingMesh3d` point at
/// the same handle by default. But they don't *have* to. `RaytracingMesh3d`
/// carries its own `Handle<Mesh>`, so we hand Solari a stripped copy and leave
/// the original untouched.
///
/// The copy costs one extra position/normal/uv/tangent buffer per distinct
/// source asset — cached in [`RaytracingProxies`] so instances share it, and
/// dropped when Solari goes idle. Returns `None` when tangents can't be derived
/// (degenerate UVs), which is the one case we genuinely can't repair.
fn build_raytracing_proxy(source: &Mesh) -> Option<Mesh> {
    let mut proxy = source.clone();

    let extra: Vec<MeshVertexAttributeId> = proxy
        .try_attributes()
        .ok()?
        .map(|(attribute, _)| attribute.id)
        .filter(|id| !BLAS_ATTRIBUTES.contains(id))
        .collect();
    for id in extra {
        proxy.try_remove_attribute(id).ok()?;
    }

    promote_indices_to_u32(&mut proxy);
    if proxy.attribute(Mesh::ATTRIBUTE_TANGENT).is_none() && proxy.generate_tangents().is_err() {
        return None;
    }
    proxy.enable_raytracing = true;
    Some(proxy)
}

/// Ray-tracing-only copies of meshes that can't be repaired in place, keyed by
/// the source asset so every instance of a mesh shares one copy.
///
/// A `None` entry records a source we already failed to convert, so a mesh with
/// degenerate UVs isn't retried (and re-warned about) every single frame.
#[derive(Resource, Default)]
struct RaytracingProxies(HashMap<AssetId<Mesh>, Option<Handle<Mesh>>>);


renzora::add!(SolariPlugin);

#[cfg(test)]
mod tests {
    use super::*;

    /// What visibility propagation would have written for an entity that is (or
    /// isn't) visible once its whole ancestor chain is taken into account.
    fn inherited(visible: bool) -> InheritedVisibility {
        if visible {
            InheritedVisibility::VISIBLE
        } else {
            InheritedVisibility::HIDDEN
        }
    }

    #[test]
    fn hidden_geometry_stays_out_of_the_ray_traced_scene() {
        // The hierarchy eye toggle and the drag-and-drop model ghost both work
        // by setting `Visibility::Hidden`. Solari's TLAS ignores visibility
        // entirely, so if we mirrored these they would keep casting shadows.
        assert!(!in_raytraced_scene(&inherited(false), None));
        assert!(in_raytraced_scene(&inherited(true), None));
    }

    #[test]
    fn only_layer_zero_is_the_scene() {
        // Layer 0 is the scene; 1+ are gizmos and the editor's offscreen rigs
        // (model thumbnails at the world origin, material viewer, studio
        // preview). Those must never occlude the level.
        let vis = inherited(true);
        assert!(in_raytraced_scene(&vis, Some(&RenderLayers::layer(0))));
        assert!(!in_raytraced_scene(&vis, Some(&RenderLayers::layer(1))));
        assert!(!in_raytraced_scene(&vis, Some(&RenderLayers::layer(8))));
        assert!(!in_raytraced_scene(&vis, Some(&RenderLayers::layer(14))));
        // A camera-overlay mesh that also draws in the scene still counts.
        assert!(in_raytraced_scene(
            &vis,
            Some(&RenderLayers::from_layers(&[0, 1]))
        ));
        // No component at all means the Bevy default, which is layer 0.
        assert!(in_raytraced_scene(&vis, None));
    }

    #[test]
    fn visibility_and_layer_are_both_required() {
        assert!(!in_raytraced_scene(
            &inherited(false),
            Some(&RenderLayers::layer(0))
        ));
    }

    #[test]
    fn blended_materials_are_not_traced_as_solid() {
        // Solari has no alpha handling, so anything it traces is opaque to every
        // ray. Cutouts are kept (a leaf card over-shadows, but no shadow at all
        // is worse); true transparency is not (glass must not cast a hard black
        // shadow — especially since deferred won't even draw it).
        let opaque = |mode| StandardMaterial {
            alpha_mode: mode,
            ..default()
        };
        assert!(material_is_raytraceable(&opaque(AlphaMode::Opaque)));
        assert!(material_is_raytraceable(&opaque(AlphaMode::Mask(0.5))));
        assert!(material_is_raytraceable(&opaque(AlphaMode::AlphaToCoverage)));
        assert!(!material_is_raytraceable(&opaque(AlphaMode::Blend)));
        assert!(!material_is_raytraceable(&opaque(AlphaMode::Premultiplied)));
        assert!(!material_is_raytraceable(&opaque(AlphaMode::Add)));
        assert!(!material_is_raytraceable(&opaque(AlphaMode::Multiply)));
    }

    #[test]
    fn an_emissive_surface_is_kept_whatever_its_alpha_mode() {
        // A blended emissive surface — a neon tube, lamp glass — is an area
        // LIGHT to Solari, but only while it is a live TLAS instance. Excluding
        // it would delete the light, not just stop it occluding, and with no
        // point-light support those are often all a night scene has.
        let neon = StandardMaterial {
            alpha_mode: AlphaMode::Blend,
            emissive: LinearRgba::rgb(4.0, 0.2, 0.2),
            ..default()
        };
        assert!(is_emissive(&neon));
        assert!(material_is_raytraceable(&neon));
    }

    /// A minimal indexed triangle with the attributes named, in insertion order
    /// (which is irrelevant — `Mesh` sorts them by attribute id).
    fn mesh_with(attributes: &[&str]) -> Mesh {
        use bevy::asset::RenderAssetUsages;
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        for name in attributes {
            match *name {
                "position" => mesh.insert_attribute(
                    Mesh::ATTRIBUTE_POSITION,
                    vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                ),
                "normal" => mesh.insert_attribute(
                    Mesh::ATTRIBUTE_NORMAL,
                    vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]],
                ),
                "uv0" => mesh.insert_attribute(
                    Mesh::ATTRIBUTE_UV_0,
                    vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
                ),
                "uv1" => mesh.insert_attribute(
                    Mesh::ATTRIBUTE_UV_1,
                    vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
                ),
                "tangent" => mesh.insert_attribute(
                    Mesh::ATTRIBUTE_TANGENT,
                    vec![[1.0, 0.0, 0.0, 1.0]; 3],
                ),
                "color" => mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0; 4]; 3]),
                other => panic!("unknown attribute {other}"),
            }
        }
        mesh.insert_indices(Indices::U32(vec![0, 1, 2]));
        mesh
    }

    /// The exact rule `bevy_solari`'s BLAS builder applies, replicated here so
    /// the test fails if our copy ever drifts from it.
    fn bevy_would_build_a_blas(mesh: &Mesh) -> bool {
        mesh.enable_raytracing
            && mesh.primitive_topology() == PrimitiveTopology::TriangleList
            && mesh
                .attributes()
                .map(|(attribute, _)| attribute.id)
                .eq(BLAS_ATTRIBUTES)
            && matches!(mesh.indices(), Some(Indices::U32(..)))
    }

    #[test]
    fn an_extra_attribute_disqualifies_the_source_mesh() {
        // The trap: Solari compares the WHOLE attribute list, in id order, so a
        // second UV set or vertex colours silently costs you the BLAS. UV_1 sits
        // at id 3, between UV_0 and TANGENT, and `renzora_wind` uses it.
        assert!(source_can_conform(&mesh_with(&["position", "normal", "uv0"])));
        assert!(source_can_conform(&mesh_with(&[
            "position", "normal", "uv0", "tangent"
        ])));
        assert!(!source_can_conform(&mesh_with(&[
            "position", "normal", "uv0", "uv1", "tangent"
        ])));
        assert!(!source_can_conform(&mesh_with(&[
            "position", "normal", "uv0", "tangent", "color"
        ])));
        // Insertion order must not matter — `Mesh` sorts by attribute id.
        assert!(source_can_conform(&mesh_with(&[
            "tangent", "uv0", "normal", "position"
        ])));
    }

    #[test]
    fn a_proxy_makes_an_over_attributed_mesh_traceable() {
        // The whole point: the source keeps UV_1 for the rasterized draw, and
        // the copy we hand Solari is stripped down to what its BLAS accepts.
        let source = mesh_with(&["position", "normal", "uv0", "uv1", "color"]);
        assert!(!source_can_conform(&source));

        let proxy = build_raytracing_proxy(&source).expect("proxy should build");
        assert!(bevy_would_build_a_blas(&proxy));
        assert!(proxy.attribute(Mesh::ATTRIBUTE_UV_1).is_none());
        assert!(proxy.attribute(Mesh::ATTRIBUTE_COLOR).is_none());
        // Tangents were generated, not copied — the source never had any.
        assert!(proxy.attribute(Mesh::ATTRIBUTE_TANGENT).is_some());
        // The source is untouched, so wind and vertex colours still work — and
        // it did NOT gain the generated tangents (that would mark the shared
        // asset modified and re-extract it every reload).
        assert!(source.attribute(Mesh::ATTRIBUTE_UV_1).is_some());
        assert!(source.attribute(Mesh::ATTRIBUTE_COLOR).is_some());
        assert!(source.attribute(Mesh::ATTRIBUTE_TANGENT).is_none());
    }

    #[test]
    fn sixteen_bit_indices_are_promoted() {
        let mut source = mesh_with(&["position", "normal", "uv0", "uv1"]);
        source.insert_indices(Indices::U16(vec![0, 1, 2]));

        let proxy = build_raytracing_proxy(&source).expect("proxy should build");
        assert!(matches!(proxy.indices(), Some(Indices::U32(..))));
        assert!(bevy_would_build_a_blas(&proxy));
    }

    #[test]
    fn the_proxy_sphere_satisfies_solaris_blas_rules() {
        // If this sphere doesn't conform, Bevy builds no BLAS for it and every
        // light proxy is silently missing from the TLAS — the exact failure
        // mode that made the buildings render black.
        let sphere = build_proxy_sphere().expect("sphere should build");
        assert!(bevy_would_build_a_blas(&sphere));
    }

    #[test]
    fn proxy_emissive_matches_the_lights_luminous_intensity() {
        // Bevy converts a light's luminous power to intensity with `I = P / 4pi`
        // (bevy_pbr `extract_lights`). A sphere of radius r and radiance L has
        // `I = L * pi * r^2` in every direction, so a correct proxy satisfies
        // `L * pi * r^2 == P / 4pi`. Checking that identity rather than the
        // formula means the test still fails if the derivation is wrong.
        let pi = core::f32::consts::PI;
        for (lumens, radius) in [(800.0_f32, 0.05_f32), (12000.0, 0.25), (40.0, 0.02)] {
            let emissive = proxy_emissive(LinearRgba::WHITE, lumens, radius);
            let intensity_from_proxy = emissive.red * pi * radius * radius;
            let intensity_from_bevy = lumens / (4.0 * pi);
            assert!(
                (intensity_from_proxy - intensity_from_bevy).abs() < intensity_from_bevy * 1e-4,
                "{lumens} lm at r={radius}: proxy {intensity_from_proxy} cd vs bevy {intensity_from_bevy} cd"
            );
        }
    }

    #[test]
    fn proxy_emissive_carries_the_light_colour() {
        let warm = LinearRgba::rgb(1.0, 0.6, 0.3);
        let emissive = proxy_emissive(warm, 800.0, 0.05);
        // Ratios preserved, magnitude scaled.
        assert!((emissive.green / emissive.red - 0.6).abs() < 1e-4);
        assert!((emissive.blue / emissive.red - 0.3).abs() < 1e-4);
        assert!(emissive.red > 1.0);
    }

    #[test]
    fn a_zero_radius_light_cannot_produce_infinite_radiance() {
        // Bevy's default `radius` is 0.0 and radiance goes as 1/r^2, so the
        // clamp is what stops a default point light becoming a NaN/inf emitter
        // that poisons the whole estimate.
        let radius = 0.0_f32.max(MIN_PROXY_RADIUS);
        let emissive = proxy_emissive(LinearRgba::WHITE, 800.0, radius);
        assert!(emissive.red.is_finite() && emissive.red > 0.0);
    }

    #[test]
    fn shadow_suppression_needs_solari_active_and_the_toggle_on() {
        // Mirrors `sync_shadow_map_suppression`'s condition. Solari being active
        // is not enough on its own: the toggle exists so raster shadows can be
        // kept while comparing backends.
        let want = |active: bool, sources: &[SolariGi]| {
            active
                && sources
                    .iter()
                    .any(|gi| gi.enabled && gi.suppress_shadow_maps)
        };
        let on = SolariGi::default();
        let toggled_off = SolariGi {
            suppress_shadow_maps: false,
            ..default()
        };
        let disabled = SolariGi {
            enabled: false,
            ..default()
        };

        assert!(want(true, std::slice::from_ref(&on)));
        assert!(!want(false, std::slice::from_ref(&on)));
        assert!(!want(true, &[toggled_off]));
        assert!(!want(true, &[disabled]));
        assert!(!want(true, &[]));
    }

    #[test]
    fn shadow_suppression_defaults_on_for_scenes_saved_before_the_field_existed() {
        // `#[reflect(default = ...)]` / `#[serde(default = ...)]` point at
        // `shadow_map_suppression_default`, NOT `bool::default()` — otherwise an
        // older scene would load with `false` and quietly keep paying for shadow
        // maps nothing reads.
        assert!(SolariGi::default().suppress_shadow_maps);
    }

    #[test]
    fn a_still_loading_material_is_undecided_not_excluded() {
        // `None` keeps a mirrored instance in place through asset churn (a
        // texture-tier swap shouldn't make the BVH flicker) and makes the mirror
        // retry next frame instead of marking the mesh permanently skipped.
        let materials = Assets::<StandardMaterial>::default();
        let missing = MeshMaterial3d::<StandardMaterial>(Handle::default());
        assert_eq!(
            is_raytraceable_instance(&materials, &missing, &inherited(true), None),
            None
        );
        // Hidden still short-circuits to a definite no — no material needed.
        assert_eq!(
            is_raytraceable_instance(&materials, &missing, &inherited(false), None),
            Some(false)
        );
    }
}
