//! The runtime: load, build, step, draw.
//!
//! Everything runs in `PostUpdate` after transform propagation and before
//! visibility, in one chain:
//!
//! 1. [`resolve_handles`] turns each [`ParticleSystem`]'s saved path into a
//!    handle, which is what makes a loaded scene's effects come back.
//! 2. [`sync_instances`] compiles an effect the first time its asset is ready,
//!    and again whenever it changes on disk (hot reload), keeping the current
//!    time so an edit does not visibly restart a long-running effect.
//! 3. [`step_effects`] applies playback requests and advances the simulation by
//!    the frame's scaled delta, in this frame's `WindState` (the instantaneous
//!    value, gust included: particles respond to a gust as a force, the way
//!    foliage does, rather than to the smoothed sustained speed).
//! 4. `update_lights` (3D builds) points each emitter's light children at
//!    what its particles say now ([`crate::light`]).
//! 5. [`write_meshes`] copies particles into each emitter's mesh, back to
//!    front from the camera for an emitter that sorts.
//!
//! A GPU emitter ([`crate::gpu`]) gets no CPU mesh: `sync_instances` leaves
//! its renderer slot empty, and the GPU backend's own systems, slotted around
//! these, give it a GPU renderer, apply its readback before the step and hand
//! the step's planned work to the render world after it.
//!
//! After propagation, so a `World` emitter spawns at where its entity is this
//! frame and its trail is drawn through this frame's transform; before
//! visibility, so the AABBs written here are the ones culling uses.

use std::collections::HashSet;

use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::math::Affine3A;
use bevy::prelude::*;
use renzora::wind::WindState;

use crate::asset::{Dimension, ParticleLight, RenderSettings, VfxAsset};
use crate::component::{OneShot, ParticlePlayback, ParticleStats, ParticleSystem, PlayState};
use crate::light::{LightRig, LightSample};
use crate::render::{sim_to_mesh, spawn_renderer, write_mesh, write_mesh_sorted, ParticleRenderer, RendererAssets};
use crate::sim::{EffectSim, FrameInput, ParticleBuffer};

/// The live state behind a [`ParticleSystem`]. Not saved; rebuilt from the
/// asset on load.
#[derive(Component)]
pub struct ParticleRuntime {
    pub sim: EffectSim,
    pub asset_id: AssetId<VfxAsset>,
    pub dimension: Dimension,
    /// One render child per entry of `sim.emitters`, same order. `None` when
    /// this build has no renderer for the effect's dimension.
    pub renderers: Vec<Option<Entity>>,
    /// Each emitter's render settings, cloned so drawing needs no asset lookup.
    pub render: Vec<RenderSettings>,
    /// One entry per entry of `sim.emitters`: the emitter's light children
    /// and their eased state, or `None` for an emitter that casts no light.
    pub lights: Vec<Option<EmitterLights>>,
    last_translation: Option<Vec3>,
    /// Particles changed since the meshes were last written.
    dirty: bool,
}

/// The `PointLight` children of one emitter ([`crate::light`]).
pub struct EmitterLights {
    pub entities: Vec<Entity>,
    pub rig: LightRig,
    pub settings: ParticleLight,
    /// The emitter's seed, so its flicker differs from its neighbour's.
    pub seed: u64,
    targets: Vec<LightSample>,
}

impl ParticleRuntime {
    /// Every child entity this runtime spawned: renderers and lights.
    fn children(&self) -> impl Iterator<Item = Entity> + '_ {
        self.renderers
            .iter()
            .flatten()
            .copied()
            .chain(self.lights.iter().flatten().flat_map(|l| l.entities.iter().copied()))
    }
}

impl ParticleRuntime {
    fn frame_input(&self, global: &GlobalTransform, dt: f32, wind: &WindState) -> FrameInput {
        let translation = global.translation();
        let emitter_velocity = match self.last_translation {
            Some(last) if dt > 0.0 => (translation - last) / dt,
            _ => Vec3::ZERO,
        };
        FrameInput {
            transform: global.affine(),
            emitter_velocity: if emitter_velocity.is_finite() { emitter_velocity } else { Vec3::ZERO },
            ..Default::default()
        }
        .with_wind(wind)
    }
}

/// Load each effect from its saved path.
///
/// `asset_server.load` on a path already loading or loaded returns the same
/// handle, so calling it on every change is cheap and idempotent. The handle is
/// written without triggering change detection: the runtime watches the asset
/// id itself, and flagging the component would re-run this every frame.
pub fn resolve_handles(
    asset_server: Res<AssetServer>,
    mut systems: Query<&mut ParticleSystem, Changed<ParticleSystem>>,
) {
    for mut system in &mut systems {
        let path = system.path.trim();
        if path.is_empty() {
            // A handle with a path came from an earlier `path` the user has now
            // cleared; one without came from code and is left alone.
            if asset_server.get_path(system.asset.id()).is_some() {
                system.bypass_change_detection().asset = Handle::default();
            }
            continue;
        }
        let handle: Handle<VfxAsset> = asset_server.load(path.to_string());
        if handle.id() != system.asset.id() {
            system.bypass_change_detection().asset = handle;
        }
    }
}

/// Build or rebuild runtimes whose asset is ready, new, or changed.
pub fn sync_instances(
    mut commands: Commands,
    assets: Res<Assets<VfxAsset>>,
    mut events: MessageReader<AssetEvent<VfxAsset>>,
    mut render_assets: RendererAssets,
    systems: Query<(
        Entity,
        &ParticleSystem,
        &GlobalTransform,
        Option<&ParticleRuntime>,
        Option<&RenderLayers>,
    )>,
    mut removed: RemovedComponents<ParticleSystem>,
    mut warned_dimension: Local<HashSet<AssetId<VfxAsset>>>,
    wind: Option<Res<WindState>>,
    support: Option<Res<crate::gpu::GpuSupport>>,
    mut known_caps: Local<Option<crate::gpu::GpuCaps>>,
) {
    let wind = wind.map(|w| *w).unwrap_or_default();
    let mut changed: HashSet<AssetId<VfxAsset>> = HashSet::new();
    for event in events.read() {
        if let AssetEvent::Modified { id } | AssetEvent::LoadedWithDependencies { id } = event {
            changed.insert(*id);
        }
    }
    // The render device can arrive frames after the first effect is built;
    // once it says what it can do, every effect is rebuilt so an emitter that
    // started on the CPU for want of an answer moves to the GPU.
    let caps = support.and_then(|s| s.caps());
    let caps_changed = caps != *known_caps;
    *known_caps = caps;
    let caps = caps.unwrap_or_default();

    for entity in removed.read() {
        teardown(&mut commands, entity, None);
    }

    for (entity, system, global, runtime, layers) in &systems {
        let id = system.asset.id();
        let Some(asset) = assets.get(id) else {
            // Not loaded yet, failed, or no effect chosen. A runtime for some
            // other effect is stale either way.
            if runtime.is_some_and(|r| r.asset_id != id) {
                teardown(&mut commands, entity, runtime);
            }
            continue;
        };
        let up_to_date = runtime.is_some_and(|r| r.asset_id == id) && !changed.contains(&id) && !caps_changed;
        if up_to_date {
            continue;
        }

        let (mut sim, failures) = EffectSim::with_backends(asset, &caps);
        let label = if system.path.is_empty() { asset.name.as_str() } else { system.path.as_str() };
        for e in sim.emitters.iter().filter(|e| e.backend.fell_back) {
            let name = &asset.emitters[e.emitter_index].name;
            let why = e.backend.reason.as_deref().unwrap_or("the GPU backend is unavailable");
            renzora::console_log::console_warn(
                "Particles",
                format!("{label}: emitter `{name}` is set to the GPU but runs on the CPU, because {why}"),
            );
        }
        for failure in &failures {
            for error in &failure.errors {
                renzora::console_log::console_warn(
                    "Particles",
                    format!("{label}: emitter `{}` was skipped: {error}", failure.name),
                );
            }
        }
        for issue in &sim.sub_emitter_issues {
            renzora::console_log::console_warn("Particles", format!("{label}: {issue}"));
        }

        // Rebuilding a running effect (a hot reload): replay to where it was so
        // an edit shows up in place rather than as a restart.
        let resume_at = runtime.map(|r| r.sim.time).unwrap_or(0.0);
        let frame = FrameInput::at(global.affine()).with_wind(&wind);
        if resume_at > 0.0 {
            sim.simulate_to(resume_at, &frame);
        }
        if let Some(old) = runtime {
            for child in old.children() {
                commands.entity(child).try_despawn();
            }
        }

        let mut renderers = Vec::with_capacity(sim.emitters.len());
        let mut render = Vec::with_capacity(sim.emitters.len());
        let mut lights = Vec::with_capacity(sim.emitters.len());
        for e in &sim.emitters {
            let emitter = &asset.emitters[e.emitter_index];
            lights.push(spawn_lights(&mut commands, entity, emitter, e.seed, asset.dimension, layers));
            // A GPU emitter's renderer is the GPU backend's to make (its
            // `attach_gpu_renderers`), from the generated kernels.
            if e.is_gpu() {
                renderers.push(None);
                render.push(emitter.render.clone());
                continue;
            }
            let renderer =
                spawn_renderer(&mut commands, &mut render_assets, entity, emitter, asset.dimension, layers);
            if renderer.is_none() && warned_dimension.insert(id) {
                renzora::console_log::console_warn(
                    "Particles",
                    format!(
                        "{label}: this build has no {} particle renderer, so the effect is simulated but not drawn",
                        match asset.dimension {
                            Dimension::TwoD => "2D",
                            Dimension::ThreeD => "3D",
                        }
                    ),
                );
            }
            renderers.push(renderer);
            render.push(emitter.render.clone());
        }

        commands.entity(entity).try_insert((
            ParticleRuntime {
                sim,
                asset_id: id,
                dimension: asset.dimension,
                renderers,
                render,
                lights,
                last_translation: Some(global.translation()),
                dirty: true,
            },
            ParticleStats::default(),
        ));
    }
}

fn teardown(commands: &mut Commands, entity: Entity, runtime: Option<&ParticleRuntime>) {
    if let Some(runtime) = runtime {
        for child in runtime.children() {
            commands.entity(child).try_despawn();
        }
    } else {
        // The component is gone; the runtime may still be there with its
        // renderers and lights. Despawn whichever children it spawned.
        commands.queue(move |world: &mut World| {
            let renderers: Vec<Entity> =
                world.get::<ParticleRuntime>(entity).map(|r| r.children().collect()).unwrap_or_default();
            for r in renderers {
                if let Ok(e) = world.get_entity_mut(r) {
                    e.despawn();
                }
            }
        });
    }
    if let Ok(mut e) = commands.get_entity(entity) {
        e.try_remove::<(ParticleRuntime, ParticleStats)>();
    }
}

/// Apply playback requests and advance every effect.
pub fn step_effects(
    mut commands: Commands,
    time: Res<Time>,
    mut effects: Query<(
        Entity,
        &GlobalTransform,
        &mut ParticlePlayback,
        &mut ParticleRuntime,
        Option<&mut ParticleStats>,
        Option<&crate::component::ParticleParams>,
    )>,
    wind: Option<Res<WindState>>,
) {
    let dt = time.delta_secs();
    // Dead calm when the wind plugin is not in the build, per the contract on
    // `WindState`.
    let wind = wind.map(|w| *w).unwrap_or_default();
    for (entity, global, mut playback, mut runtime, stats, params) in &mut effects {
        let frame = runtime.frame_input(global, dt, &wind);
        let runtime = &mut *runtime;
        // Every frame, not on change: a hot reload rebuilds the simulation
        // with no script values, and this is what puts them back.
        if let Some(params) = params {
            runtime.sim.apply_parameters(&params.values);
        }

        // Transient requests are cleared without flagging the component as
        // changed: they are not edits, and the editor treats a changed
        // component as an unsaved scene.
        if playback.restart_requested {
            playback.bypass_change_detection().restart_requested = false;
            runtime.sim.reset();
            runtime.dirty = true;
        }
        if let Some(t) = playback.seek_to {
            playback.bypass_change_detection().seek_to = None;
            runtime.sim.simulate_to(t, &frame);
            runtime.dirty = true;
        }
        if playback.pending_bursts > 0 {
            let n = playback.pending_bursts;
            playback.bypass_change_detection().pending_bursts = 0;
            runtime.sim.burst(n, &frame);
            runtime.dirty = true;
        }

        // An editor holding the effect's clock (`editor_clock`) plays it at
        // its own rate whatever the saved play state says.
        let state = if playback.editor_clock.is_some() { PlayState::Playing } else { playback.state };
        match state {
            PlayState::Playing => {
                let scale = playback.editor_clock.unwrap_or(playback.time_scale);
                let scale = if scale.is_finite() { scale.max(0.0) } else { 0.0 };
                if scale > 0.0 && dt > 0.0 {
                    runtime.sim.advance(dt * scale, &frame);
                    runtime.dirty = true;
                }
            }
            PlayState::Paused => {}
            PlayState::Stopped => {
                if runtime.sim.time > 0.0 || runtime.sim.alive() > 0 {
                    runtime.sim.reset();
                    runtime.dirty = true;
                }
            }
        }
        runtime.last_translation = Some(global.translation());

        if playback.editor_clock.is_none() && playback.state == PlayState::Playing && runtime.sim.is_finished() {
            match playback.one_shot {
                OneShot::Keep => {}
                OneShot::Deactivate => playback.state = PlayState::Stopped,
                OneShot::Despawn => {
                    commands.entity(entity).try_despawn();
                    continue;
                }
            }
        }

        if let Some(mut stats) = stats {
            let next = ParticleStats {
                alive: runtime.sim.alive() as u32,
                dropped: runtime.sim.dropped(),
                time: runtime.sim.time,
            };
            if stats.alive != next.alive || stats.dropped != next.dropped || stats.time != next.time {
                *stats = next;
            }
        }
    }
}

/// Spawn an emitter's light children, or `None` when it casts no light.
///
/// 3D only: Bevy's 2D renderer has no point lights. Each light is editor
/// plumbing like a renderer: hidden from the hierarchy (and therefore never
/// written into a saved scene), outside selection bounds, on the effect's own
/// render layers so a preview rig's fire lights only the preview. Spawned
/// dark and hidden; [`update_lights`] turns them on once particles exist.
#[cfg_attr(not(feature = "render_3d"), allow(unused_variables))]
fn spawn_lights(
    commands: &mut Commands,
    parent: Entity,
    emitter: &crate::asset::Emitter,
    seed: u64,
    dimension: Dimension,
    layers: Option<&RenderLayers>,
) -> Option<EmitterLights> {
    let settings = emitter.light;
    let count = settings.light_count();
    if count == 0 || dimension != Dimension::ThreeD {
        return None;
    }
    #[cfg(feature = "render_3d")]
    {
        let entities = (0..count)
            .map(|k| {
                let id = commands
                    .spawn((
                        Name::new(format!("vfx light {} {}", emitter.name, k + 1)),
                        renzora::HideInHierarchy,
                        renzora::NoSelectionBounds,
                        ChildOf(parent),
                        bevy::light::PointLight {
                            intensity: 0.0,
                            range: finite_or(settings.range, 6.0).max(0.0),
                            radius: finite_or(settings.radius, 0.1).max(0.0),
                            shadow_maps_enabled: settings.shadows,
                            ..default()
                        },
                        Transform::default(),
                        Visibility::Hidden,
                    ))
                    .id();
                if let Some(layers) = layers {
                    commands.entity(id).insert(layers.clone());
                }
                id
            })
            .collect();
        Some(EmitterLights { entities, rig: LightRig::new(count), settings, seed, targets: Vec::new() })
    }
    #[cfg(not(feature = "render_3d"))]
    None
}

#[cfg(feature = "render_3d")]
fn finite_or(v: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        fallback
    }
}

/// Point every emitter light at what its particles say now ([`crate::light`]).
///
/// Runs after the step and writes each light's `GlobalTransform` as well as
/// its `Transform`: propagation has already run this frame, and a light that
/// follows a particle would otherwise trail it by one.
#[cfg(feature = "render_3d")]
pub fn update_lights(
    mut effects: Query<(&mut ParticleRuntime, &GlobalTransform)>,
    mut lights: Query<
        (&mut bevy::light::PointLight, &mut Transform, &mut GlobalTransform, &mut Visibility),
        Without<ParticleRuntime>,
    >,
) {
    for (mut runtime, global) in &mut effects {
        let runtime = &mut *runtime;
        if runtime.lights.iter().all(Option::is_none) {
            continue;
        }
        let time = runtime.sim.time;
        let world_to_local = global.affine().inverse();
        for (slot, emitter) in runtime.sim.emitters.iter().enumerate() {
            let Some(Some(rig)) = runtime.lights.get_mut(slot) else { continue };
            let to_local = match emitter.space {
                crate::asset::SimulationSpace::Local => Affine3A::IDENTITY,
                crate::asset::SimulationSpace::World if world_to_local.is_finite() => world_to_local,
                crate::asset::SimulationSpace::World => continue,
            };
            match &emitter.gpu {
                // The GPU reduces a PerEmitter light's sums on the device;
                // BrightestN never runs there (see `gpu::select`).
                Some(g) => {
                    rig.targets.clear();
                    if rig.settings.mode == crate::asset::LightMode::PerEmitter {
                        let r = &g.readout;
                        rig.targets.push(crate::light::per_emitter(&rig.settings, r.light_weight, r.light_rgb));
                    }
                }
                None => crate::light::aggregate(&emitter.particles, &rig.settings, to_local, &mut rig.targets),
            }
            rig.rig.update(&rig.targets, time, rig.settings.smoothing);
            let flicker = crate::light::flicker(&rig.settings, rig.seed, time);
            for (sample, &entity) in rig.rig.lights.iter().zip(&rig.entities) {
                let Ok((mut light, mut transform, mut light_global, mut visibility)) = lights.get_mut(entity) else {
                    continue;
                };
                let intensity = (sample.intensity * flicker).max(0.0);
                // An unlit light is hidden rather than left at zero, so it
                // costs no cluster slot and gets no Solari proxy.
                if intensity < 1e-3 || !intensity.is_finite() {
                    visibility.set_if_neq(Visibility::Hidden);
                    continue;
                }
                visibility.set_if_neq(Visibility::Inherited);
                light.intensity = intensity;
                light.color = Color::linear_rgb(sample.color.x, sample.color.y, sample.color.z);
                if transform.translation != sample.position {
                    transform.translation = sample.position;
                }
                *light_global = global.mul_transform(*transform);
            }
        }
    }
}

/// Copy particles into each emitter's mesh, and set its bounds and visibility.
///
/// An emitter that sorts back to front is rewritten every frame, paused or
/// not: the order depends on the camera, which moves on its own.
pub fn write_meshes(
    mut meshes: ResMut<Assets<Mesh>>,
    mut effects: Query<(&mut ParticleRuntime, &GlobalTransform, Option<&RenderLayers>)>,
    mut renderers: Query<(&ParticleRenderer, &Transform, &mut Aabb, &mut Visibility, Option<Ref<crate::render::MeshParticles>>)>,
    cameras: Query<(&Camera, &GlobalTransform, Option<&RenderLayers>), With<Camera3d>>,
) {
    for (mut runtime, global, layers) in &mut effects {
        let dimension = runtime.dimension;
        let sorting = runtime.render.iter().any(|r| r.sorts(dimension));
        // Mesh particles whose template just arrived need writing even when
        // nothing simulated.
        let mesh_pending = runtime.renderers.iter().any(|r| {
            let Some(r) = r else { return false };
            renderers.get(*r).is_ok_and(|(.., m)| m.is_some_and(|m| m.is_changed()))
        });
        if !runtime.dirty && !sorting && !mesh_pending {
            continue;
        }
        runtime.dirty = false;
        let runtime = &*runtime;
        let eye = if sorting { crate::render::sort_eye(cameras.iter(), layers) } else { None };
        for (slot, emitter) in runtime.sim.emitters.iter().enumerate() {
            let Some(Some(renderer_entity)) = runtime.renderers.get(slot) else {
                continue;
            };
            let Ok((renderer, local, mut aabb, mut visibility, mesh_particles)) = renderers.get_mut(*renderer_entity) else {
                continue;
            };
            let Some(mut mesh) = meshes.get_mut(&renderer.mesh) else {
                continue;
            };
            // An entity scaled to zero has no inverse; draw nothing rather
            // than particles at NaN.
            let render = &runtime.render[slot];
            let mesh_particles = mesh_particles.as_deref();
            let bounds = match sim_to_mesh(emitter.space, global, local) {
                Some(to_mesh) => {
                    // The camera in mesh space, where the quads' centres are.
                    let mesh_to_world = global.affine() * local.compute_affine();
                    let mesh_from_world = mesh_to_world.inverse();
                    let eye = eye
                        .filter(|_| render.sorts(dimension))
                        .map(|e| mesh_from_world.transform_point3(e))
                        .filter(|e| e.is_finite());
                    match mesh_particles {
                        Some(m) => {
                            crate::render::write_mesh_renderer(&mut mesh, &emitter.particles, render, to_mesh, eye, m)
                        }
                        None => write_mesh_sorted(&mut mesh, &emitter.particles, render, to_mesh, eye),
                    }
                }
                None => match mesh_particles {
                    Some(m) => crate::render::write_mesh_renderer(
                        &mut mesh,
                        &ParticleBuffer::default(),
                        render,
                        Affine3A::IDENTITY,
                        None,
                        m,
                    ),
                    None => write_mesh(&mut mesh, &ParticleBuffer::default(), render, Affine3A::IDENTITY),
                },
            };
            match bounds {
                Some(b) => {
                    if *aabb != b {
                        *aabb = b;
                    }
                    visibility.set_if_neq(Visibility::Inherited);
                }
                None => {
                    visibility.set_if_neq(Visibility::Hidden);
                }
            }
        }
    }
}
