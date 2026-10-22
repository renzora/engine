//! The GPU backend's render-world half: pipelines, the per-emitter step
//! queue, and the compute dispatches.
//!
//! ## A frame
//!
//! 1. **Extract.** Each render child's [`GpuEmitterLink`] is copied in. A
//!    batch the queue has not seen is appended (after emptying it, if the
//!    batch resets).
//! 2. **Prepare.** Per group of emitters whose pipelines are compiled and
//!    whose buffers are uploaded: up to [`MAX_STEPS_PER_FRAME`] queued steps
//!    become step records in one dynamic uniform buffer, plus one frame
//!    record per emitter for the fixed kernels; each state header gets this
//!    frame's simulation-to-mesh transform and cleared stats. A group that is
//!    not ready keeps its queues for a later frame.
//! 3. **Dispatch**, before any camera draws, in one compute pass: the resets
//!    that are due, then per step `spawn` and `update` for every emitter of
//!    the group, then `deliver` and `clear_inbox` for each sub-emitter target
//!    of it; then `stats`, and the sort for an emitter that orders its
//!    particles. Dispatches within a pass see each other's writes, so the
//!    order above is the order of effect.
//!
//! ## Groups
//!
//! Emitters joined by sub-emitter rules share their effect's event buffer
//! and must step together: a parent's step `n` raises the events its
//! targets deliver in step `n`, as `EffectSim::step` does. So they are one
//! group, ready only when all of them are, taking the same number of steps
//! each frame, members in the topological order the main world gave them
//! (parents before children, so a child's own birth events reach its targets
//! before their `deliver`). The CPU plans the same number of steps for every
//! emitter of an effect (a script burst gives a target an empty one), so
//! their queues stay the same length. An emitter with no rules is a group of
//! one, and runs exactly as it always did.
//!
//! Bevy's readback then copies the stats out, and the cameras draw the pool
//! through the particle material.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::num::NonZeroU64;

use bevy::asset::AssetId;
use bevy::math::{Affine3A, UVec4};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::binding_types::{storage_buffer_read_only_sized, storage_buffer_sized, uniform_buffer};
use bevy::render::render_resource::{
    BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, Buffer, BufferInitDescriptor,
    BufferUsages, CachedComputePipelineId, ComputePassDescriptor, ComputePipelineDescriptor, DownlevelFlags,
    DynamicUniformBuffer, PipelineCache, ShaderStages, ShaderType,
};
use bevy::render::renderer::{RenderAdapter, RenderContext, RenderDevice, RenderGraph, RenderGraphSystems, RenderQueue};
use bevy::render::storage::{GpuShaderBuffer, ShaderBuffer};
use bevy::render::{Extract, ExtractSchedule, Render, RenderApp, RenderStartup, RenderSystems};

use super::codegen::{kernels_source, WORKGROUP_SIZE};
use super::layout::{self, EmitterConsts, GpuStep, C_GROUPS, C_LIGHT, C_TO_MESH, SORT_CONNECT, SORT_NONE, STATE_HEADER_WORDS};
use super::runtime::GpuEmitterLink;
use super::{GpuCaps, GpuSupport};
use crate::sim::{FrameInput, GpuStepPlan};

/// Most steps one emitter runs in a frame: a ten-second seek at the seek
/// step, plus a frame's worth of play. More waits for the next frame.
pub const MAX_STEPS_PER_FRAME: usize = crate::sim::GPU_MAX_SEEK_STEPS as usize + 16;

/// A queue longer than this is cut from the front before it grows further: a
/// render world that cannot run an emitter for a long time (its pipeline
/// failed) must not grow its queue without bound.
const MAX_QUEUED_STEPS: usize = 8 * MAX_STEPS_PER_FRAME;

/// The fixed kernels' shader, made in the main world and extracted.
#[derive(Resource, Clone)]
pub struct GpuCommonShader(pub Handle<Shader>);

/// Register the render-world half. `support` is the resource both worlds
/// share; it is filled at `RenderStartup`.
pub fn build(app: &mut App) {
    // A headless app has no render world: nothing to dispatch, and the
    // support stays unknown, so every emitter runs on the CPU.
    if app.get_sub_app(RenderApp).is_none() {
        return;
    }
    let support = app.world_mut().get_resource_or_insert_with(GpuSupport::default).clone();
    let Some(mut shaders) = app.world_mut().get_resource_mut::<Assets<Shader>>() else { return };
    let shader = shaders.add(renzora::shader_source::shader(kernels_source(), "renzora_particles/gpu/kernels.wgsl"));
    app.insert_resource(GpuCommonShader(shader.clone()));
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
    render_app
        .insert_resource(support)
        .insert_resource(GpuCommonShader(shader))
        .init_resource::<GpuEmitters>()
        .init_resource::<GpuDispatches>()
        .add_systems(RenderStartup, detect_support)
        .add_systems(ExtractSchedule, extract_emitters)
        .add_systems(Render, prepare_dispatches.in_set(RenderSystems::PrepareBindGroups))
        .add_systems(
            RenderGraph,
            dispatch.in_set(RenderGraphSystems::Render).before(bevy::core_pipeline::schedule::camera_driver),
        );
}

/// What the kernels need from the device: compute, storage buffers a vertex
/// shader can read (the draw fetches from the pool), and room for five
/// storage bindings in a stage. WebGL2 and many mobile GPUs in compatibility
/// mode have neither of the first two.
fn detect_support(device: Res<RenderDevice>, adapter: Res<RenderAdapter>, support: Res<GpuSupport>) {
    let flags = adapter.get_downlevel_capabilities().flags;
    let limits = device.limits();
    let compute = flags.contains(DownlevelFlags::COMPUTE_SHADERS)
        && flags.contains(DownlevelFlags::VERTEX_STORAGE)
        && limits.max_storage_buffers_per_shader_stage >= 5
        && limits.max_compute_workgroup_size_x >= WORKGROUP_SIZE
        && limits.max_compute_invocations_per_workgroup >= WORKGROUP_SIZE
        && limits.max_compute_workgroups_per_dimension >= layout::MAX_GPU_CAPACITY / WORKGROUP_SIZE;
    let max_capacity = GpuCaps::capacity_for_binding(limits.max_storage_buffer_binding_size);
    support.set(GpuCaps {
        compute,
        max_capacity,
        renderer_3d: cfg!(feature = "render_3d"),
        renderer_2d: cfg!(feature = "render_2d"),
        max_binding: limits.max_storage_buffer_binding_size,
    });
    info!("[particles] GPU backend {}", if compute { "available" } else { "unavailable on this device" });
}

/// One emitter as the render world keeps it between frames.
struct RenderEmitter {
    particles: AssetId<ShaderBuffer>,
    state: AssetId<ShaderBuffer>,
    events: AssetId<ShaderBuffer>,
    shader: Handle<Shader>,
    capacity: u32,
    seed: u64,
    sort_mode: u32,
    ribbon_groups: u32,
    /// The effect entity, for emitters that share events; `None` alone.
    group: Option<Entity>,
    /// Place in the group's topological order.
    order: u32,
    inbox: Option<(u32, u32)>,
    data: std::sync::Arc<Vec<[f32; 4]>>,
    data_buffer: Option<Buffer>,
    queue: VecDeque<GpuStepPlan>,
    reset_pending: bool,
    serial: u32,
    last_frame: u64,
    /// The spawn index the next step starts at, as of the last step run.
    next_id: u32,
    /// The newest frame's transform and wind, camera and mesh transform.
    frame_input: FrameInput,
    camera: Vec3,
    to_mesh: Affine3A,
    seen: bool,
}

#[derive(Resource, Default)]
struct GpuEmitters(HashMap<Entity, RenderEmitter>);

fn extract_emitters(mut emitters: ResMut<GpuEmitters>, links: Extract<Query<(Entity, &GpuEmitterLink)>>) {
    for e in emitters.0.values_mut() {
        e.seen = false;
    }
    for (entity, link) in &links {
        let e = emitters.0.entry(entity).or_insert_with(|| RenderEmitter {
            particles: link.particles.id(),
            state: link.state.id(),
            events: link.events.id(),
            shader: link.shader.clone(),
            capacity: link.capacity,
            seed: link.seed,
            sort_mode: link.sort_mode,
            ribbon_groups: link.ribbon_groups,
            group: link.group,
            order: link.order,
            inbox: link.inbox,
            data: link.data.clone(),
            data_buffer: None,
            queue: VecDeque::new(),
            reset_pending: false,
            serial: 0,
            last_frame: 0,
            next_id: 0,
            frame_input: link.frame_input,
            camera: link.camera,
            to_mesh: link.to_mesh,
            seen: true,
        });
        e.seen = true;
        if link.frame == e.last_frame {
            continue;
        }
        e.last_frame = link.frame;
        if link.reset {
            e.queue.clear();
            e.reset_pending = true;
            e.serial = link.serial;
            e.next_id = 0;
        }
        e.queue.extend(link.steps.iter().copied());
        if e.queue.len() > MAX_QUEUED_STEPS {
            let excess = e.queue.len() - MAX_QUEUED_STEPS;
            e.queue.drain(..excess);
        }
        e.frame_input = link.frame_input;
        e.camera = link.camera;
        e.to_mesh = link.to_mesh;
    }
    emitters.0.retain(|_, e| e.seen);
}

/// Pipelines by shader, queued the first time an emitter needs them.
#[derive(Default)]
struct Pipelines {
    common: Option<CommonPipelines>,
    emitters: HashMap<AssetId<Shader>, EmitterPipelines>,
    layout0: Option<BindGroupLayoutDescriptor>,
    layout1: Option<BindGroupLayoutDescriptor>,
}

#[derive(Clone, Copy)]
struct EmitterPipelines {
    spawn: CachedComputePipelineId,
    update: CachedComputePipelineId,
    /// Only a sub-emitter target's shader has the entry point.
    deliver: Option<CachedComputePipelineId>,
}

struct CommonPipelines {
    reset: CachedComputePipelineId,
    stats: CachedComputePipelineId,
    sort_keys: CachedComputePipelineId,
    sort_step: CachedComputePipelineId,
    clear_inbox: CachedComputePipelineId,
}

/// One bitonic pass's `(j, k)`, padded by the dynamic uniform buffer to its
/// offset alignment.
#[derive(Clone, Copy, Default, ShaderType)]
struct SortPass {
    pass: UVec4,
}

/// One emitter's dispatches this frame.
struct EmitterDispatch {
    group0: BindGroup,
    pipelines: EmitterPipelines,
    capacity: u32,
    sort_size: u32,
    reset: bool,
    sort: bool,
    inbox_cap: Option<u32>,
    frame_offset: u32,
    /// (record offset, spawn threads, spawn only)
    steps: Vec<(u32, u32, bool)>,
}

/// One group's dispatches: its emitters in topological order, every one
/// with the same number of steps.
struct GroupDispatch {
    members: Vec<EmitterDispatch>,
    steps: usize,
}

#[derive(Resource, Default)]
struct GpuDispatches {
    pipelines: Pipelines,
    steps: DynamicUniformBuffer<GpuStep>,
    sort_passes: DynamicUniformBuffer<SortPass>,
    sort_offsets: Vec<u32>,
    group1: Option<BindGroup>,
    list: Vec<GroupDispatch>,
}

/// The `min_binding_size` of group 0's storage bindings, by binding (3, the
/// step uniform, sizes itself from `GpuStep`). One list so the test that
/// holds them to the kernels' own types checks exactly what the layout
/// declares: wgpu refuses the pipeline, unrecoverably, when a binding's
/// declared minimum is below what the shader's type needs.
pub(crate) fn group0_storage_min_bytes() -> [(u32, u64); 4] {
    [
        (0, layout::PARTICLE_STRIDE),
        (1, layout::STATE_MIN_BINDING_BYTES),
        (2, 16),
        (4, u64::from(layout::EVENT_HEADER_WORDS) * 4),
    ]
}

fn layout0() -> BindGroupLayoutDescriptor {
    let [particles, state, data, events] = group0_storage_min_bytes().map(|(_, bytes)| NonZeroU64::new(bytes));
    BindGroupLayoutDescriptor::new(
        "renzora_particles:gpu:group0",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                storage_buffer_sized(false, particles),
                storage_buffer_sized(false, state),
                storage_buffer_read_only_sized(false, data),
                uniform_buffer::<GpuStep>(true),
                storage_buffer_sized(false, events),
            ),
        ),
    )
}

fn layout1() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "renzora_particles:gpu:group1",
        &BindGroupLayoutEntries::single(ShaderStages::COMPUTE, uniform_buffer::<SortPass>(true)),
    )
}

fn queue(cache: &PipelineCache, label: &'static str, layout: Vec<BindGroupLayoutDescriptor>, shader: &Handle<Shader>, entry: &'static str) -> CachedComputePipelineId {
    cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some(label.into()),
        layout,
        shader: shader.clone(),
        entry_point: Some(entry.into()),
        zero_initialize_workgroup_memory: true,
        ..Default::default()
    })
}

fn groups(threads: u32) -> u32 {
    threads.div_ceil(WORKGROUP_SIZE).max(1)
}

fn words_bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn data_bytes(data: &[[f32; 4]]) -> Vec<u8> {
    data.iter().flat_map(|v| v.iter().flat_map(|f| f.to_le_bytes())).collect()
}

#[allow(clippy::too_many_arguments)]
fn prepare_dispatches(
    mut dispatches: ResMut<GpuDispatches>,
    mut emitters: ResMut<GpuEmitters>,
    common_shader: Res<GpuCommonShader>,
    cache: Res<PipelineCache>,
    buffers: Res<RenderAssets<GpuShaderBuffer>>,
    device: Res<RenderDevice>,
    queue_res: Res<RenderQueue>,
) {
    let d = &mut *dispatches;
    d.list.clear();
    d.steps.clear();
    if emitters.0.is_empty() {
        return;
    }
    let l0 = d.pipelines.layout0.get_or_insert_with(layout0).clone();
    let l1 = d.pipelines.layout1.get_or_insert_with(layout1).clone();
    let common = d.pipelines.common.get_or_insert_with(|| {
        let layouts = vec![l0.clone(), l1.clone()];
        let s = &common_shader.0;
        CommonPipelines {
            reset: queue(&cache, "renzora_particles:gpu:reset", layouts.clone(), s, "reset"),
            stats: queue(&cache, "renzora_particles:gpu:stats", layouts.clone(), s, "stats"),
            sort_keys: queue(&cache, "renzora_particles:gpu:sort_keys", layouts.clone(), s, "sort_keys"),
            sort_step: queue(&cache, "renzora_particles:gpu:sort_step", layouts.clone(), s, "sort_step"),
            clear_inbox: queue(&cache, "renzora_particles:gpu:clear_inbox", layouts, s, "clear_inbox"),
        }
    });
    let common_ready = [common.reset, common.stats, common.sort_keys, common.sort_step, common.clear_inbox]
        .iter()
        .all(|id| cache.get_compute_pipeline(*id).is_some());

    // The sort passes for the largest pool, once: every smaller sort is a
    // prefix of it (see `layout::sort_passes`).
    if d.sort_offsets.is_empty() {
        for (j, k) in layout::sort_passes(layout::sort_size(layout::MAX_GPU_CAPACITY)) {
            let offset = d.sort_passes.push(&SortPass { pass: UVec4::new(j, k, 0, 0) });
            d.sort_offsets.push(offset);
        }
        d.sort_passes.write_buffer(&device, &queue_res);
    }

    // Groups, each in topological order, in a stable order of their own.
    let mut grouped: BTreeMap<Entity, Vec<Entity>> = BTreeMap::new();
    for (entity, e) in &emitters.0 {
        grouped.entry(e.group.unwrap_or(*entity)).or_default().push(*entity);
    }
    for members in grouped.values_mut() {
        members.sort_by_key(|m| (emitters.0.get(m).map(|e| e.order).unwrap_or(0), *m));
    }

    // Records first: every emitter's offsets into the one buffer, then the
    // buffer is written and the bind groups made against it.
    struct Planned {
        entity: Entity,
        pipelines: EmitterPipelines,
        reset: bool,
        frame_offset: u32,
        steps: Vec<(u32, u32, bool)>,
    }
    let mut planned_groups: Vec<(Vec<Planned>, usize)> = Vec::new();
    for members in grouped.values() {
        let mut ready = common_ready;
        let mut take = MAX_STEPS_PER_FRAME;
        let mut pipes = Vec::with_capacity(members.len());
        for m in members {
            let Some(e) = emitters.0.get(m) else {
                ready = false;
                continue;
            };
            let target = e.inbox.is_some();
            let p = *d.pipelines.emitters.entry(e.shader.id()).or_insert_with(|| {
                let layouts = vec![l0.clone()];
                EmitterPipelines {
                    spawn: queue(&cache, "renzora_particles:gpu:spawn", layouts.clone(), &e.shader, "spawn"),
                    update: queue(&cache, "renzora_particles:gpu:update", layouts.clone(), &e.shader, "update"),
                    deliver: target.then(|| queue(&cache, "renzora_particles:gpu:deliver", layouts, &e.shader, "deliver")),
                }
            });
            ready &= cache.get_compute_pipeline(p.spawn).is_some()
                && cache.get_compute_pipeline(p.update).is_some()
                && p.deliver.is_none_or(|id| cache.get_compute_pipeline(id).is_some())
                && (!target || p.deliver.is_some())
                && buffers.get(e.particles).is_some()
                && buffers.get(e.state).is_some()
                && buffers.get(e.events).is_some();
            take = take.min(e.queue.len());
            pipes.push(p);
        }
        if !ready {
            continue;
        }
        let mut planned = Vec::with_capacity(members.len());
        for (m, pipelines) in members.iter().zip(pipes) {
            let Some(e) = emitters.0.get_mut(m) else { continue };
            let consts = EmitterConsts {
                capacity: e.capacity,
                seed: e.seed,
                serial: e.serial,
                inbox: e.inbox,
                sort_mode: e.sort_mode,
                groups: e.ribbon_groups,
            };
            // The last step that updates rebuilds the alive list.
            let last_update = e.queue.iter().take(take).rposition(|p| !p.spawn_only);
            let mut steps = Vec::with_capacity(take);
            for (i, plan) in e.queue.drain(..take).enumerate() {
                let offset = d.steps.push(&GpuStep::from_plan(consts, &plan, Some(i) == last_update));
                steps.push((offset, plan.spawn_count.min(e.capacity), plan.spawn_only));
                e.next_id = plan.first_id.wrapping_add(plan.spawn_count);
            }
            let frame_offset = d.steps.push(&GpuStep::frame(consts, &e.frame_input, e.camera, e.next_id));
            let reset = std::mem::take(&mut e.reset_pending);
            planned.push(Planned { entity: *m, pipelines, reset, frame_offset, steps });
        }
        planned_groups.push((planned, take));
    }
    if planned_groups.is_empty() {
        return;
    }
    d.steps.write_buffer(&device, &queue_res);
    let (Some(step_binding), Some(sort_binding)) = (d.steps.binding(), d.sort_passes.binding()) else { return };
    let layout0 = cache.get_bind_group_layout(&l0);
    let layout1 = cache.get_bind_group_layout(&l1);
    d.group1 = Some(device.create_bind_group("renzora_particles:gpu:group1", &layout1, &BindGroupEntries::single(sort_binding)));
    let cleared = words_bytes(&layout::cleared_stats());
    let no_groups = words_bytes(&[0u32; (STATE_HEADER_WORDS as usize) - C_GROUPS]);
    let mut list = Vec::with_capacity(planned_groups.len());
    for (planned, steps) in planned_groups {
        let mut members = Vec::with_capacity(planned.len());
        for p in planned {
            let Some(e) = emitters.0.get_mut(&p.entity) else { continue };
            let (Some(particles), Some(state), Some(events)) = (buffers.get(e.particles), buffers.get(e.state), buffers.get(e.events))
            else {
                continue;
            };
            let data = e.data_buffer.get_or_insert_with(|| {
                device.create_buffer_with_data(&BufferInitDescriptor {
                    label: Some("renzora_particles:gpu:data"),
                    contents: &data_bytes(&e.data),
                    usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                })
            });
            let group0 = device.create_bind_group(
                "renzora_particles:gpu:group0",
                &layout0,
                &BindGroupEntries::sequential((
                    particles.buffer.as_entire_binding(),
                    state.buffer.as_entire_binding(),
                    data.as_entire_binding(),
                    step_binding.clone(),
                    events.buffer.as_entire_binding(),
                )),
            );
            // This frame's header words: where the vertex shader draws the
            // pool from, the stats the kernel below accumulates into, and a
            // Connect ribbon's per-strip counts.
            queue_res.write_buffer(&state.buffer, (C_TO_MESH * 4) as u64, &words_bytes(&layout::to_mesh_words(e.to_mesh)));
            queue_res.write_buffer(&state.buffer, (C_LIGHT * 4) as u64, &cleared);
            if e.sort_mode == SORT_CONNECT {
                queue_res.write_buffer(&state.buffer, (C_GROUPS * 4) as u64, &no_groups);
            }
            members.push(EmitterDispatch {
                group0,
                pipelines: p.pipelines,
                capacity: e.capacity,
                sort_size: layout::sort_size(e.capacity),
                reset: p.reset,
                sort: e.sort_mode != SORT_NONE,
                inbox_cap: e.inbox.map(|(_, cap)| cap),
                frame_offset: p.frame_offset,
                steps: p.steps,
            });
        }
        list.push(GroupDispatch { members, steps });
    }
    d.list = list;
}

fn dispatch(mut ctx: RenderContext, dispatches: Res<GpuDispatches>, cache: Res<PipelineCache>) {
    if dispatches.list.is_empty() {
        return;
    }
    let Some(common) = dispatches.pipelines.common.as_ref() else { return };
    let Some(group1) = dispatches.group1.as_ref() else { return };
    let get = |id| cache.get_compute_pipeline(id);
    let (Some(reset), Some(stats), Some(sort_keys), Some(sort_step), Some(clear_inbox)) =
        (get(common.reset), get(common.stats), get(common.sort_keys), get(common.sort_step), get(common.clear_inbox))
    else {
        return;
    };
    let encoder = ctx.command_encoder();
    let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor { label: Some("renzora_particles:gpu"), timestamp_writes: None });
    for group in &dispatches.list {
        // A member whose kernels vanished from the cache since prepare is
        // left out, and so is the whole group's stepping, so the others
        // never step past it.
        let resolved: Option<Vec<_>> = group
            .members
            .iter()
            .map(|e| {
                let spawn = get(e.pipelines.spawn)?;
                let update = get(e.pipelines.update)?;
                let deliver = match e.pipelines.deliver {
                    Some(id) => Some(get(id)?),
                    None => None,
                };
                Some((spawn, update, deliver))
            })
            .collect();
        let Some(resolved) = resolved else { continue };
        for e in &group.members {
            if e.reset {
                pass.set_pipeline(reset);
                pass.set_bind_group(0, &e.group0, &[e.frame_offset]);
                pass.set_bind_group(1, group1, &[0]);
                pass.dispatch_workgroups(groups(e.capacity), 1, 1);
                if e.inbox_cap.is_some() {
                    // Events raised before the reset belong to particles
                    // that no longer exist.
                    pass.set_pipeline(clear_inbox);
                    pass.dispatch_workgroups(1, 1, 1);
                }
            }
        }
        for s in 0..group.steps {
            for (e, (spawn, update, _)) in group.members.iter().zip(&resolved) {
                let Some(&(offset, threads, spawn_only)) = e.steps.get(s) else { continue };
                pass.set_pipeline(spawn);
                pass.set_bind_group(0, &e.group0, &[offset]);
                pass.dispatch_workgroups(groups(threads), 1, 1);
                if !spawn_only {
                    pass.set_pipeline(update);
                    pass.set_bind_group(0, &e.group0, &[offset]);
                    pass.dispatch_workgroups(groups(e.capacity), 1, 1);
                }
            }
            for (e, (_, _, deliver)) in group.members.iter().zip(&resolved) {
                let (Some(deliver), Some(cap)) = (deliver, e.inbox_cap) else { continue };
                let Some(&(offset, _, _)) = e.steps.get(s) else { continue };
                pass.set_pipeline(deliver);
                pass.set_bind_group(0, &e.group0, &[offset]);
                pass.dispatch_workgroups(groups(cap), 1, 1);
                pass.set_pipeline(clear_inbox);
                pass.set_bind_group(0, &e.group0, &[offset]);
                pass.set_bind_group(1, group1, &[0]);
                pass.dispatch_workgroups(1, 1, 1);
            }
        }
        for e in &group.members {
            let pool = groups(e.capacity);
            pass.set_pipeline(stats);
            pass.set_bind_group(0, &e.group0, &[e.frame_offset]);
            pass.set_bind_group(1, group1, &[0]);
            pass.dispatch_workgroups(pool, 1, 1);
            if e.sort {
                let sort_groups = groups(e.sort_size);
                pass.set_pipeline(sort_keys);
                pass.set_bind_group(0, &e.group0, &[e.frame_offset]);
                pass.set_bind_group(1, group1, &[0]);
                pass.dispatch_workgroups(sort_groups, 1, 1);
                pass.set_pipeline(sort_step);
                let passes = layout::sort_pass_count(e.sort_size);
                for &offset in dispatches.sort_offsets.iter().take(passes) {
                    pass.set_bind_group(1, group1, &[offset]);
                    pass.dispatch_workgroups(sort_groups, 1, 1);
                }
            }
        }
    }
}
