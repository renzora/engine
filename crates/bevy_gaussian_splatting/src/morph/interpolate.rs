use std::{any::TypeId, marker::PhantomData};

use bevy::{
    asset::{Assets, LoadState, load_internal_asset, uuid_handle},
    core_pipeline::{Core3d, Core3dSystems, prepass::PreviousViewUniformOffset},
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        extract_component::DynamicUniformIndex,
        render_asset::RenderAssets,
        render_resource::{
            BindGroup, BindGroupLayout, CachedComputePipelineId, CachedPipelineState,
            ComputePassDescriptor, ComputePipelineDescriptor, PipelineCache,
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        sync_world::{RenderEntity, SyncToRenderWorld},
        view::ViewUniformOffset,
    },
};
use bevy_interleave::prelude::*;

use crate::{
    camera::GaussianCamera,
    gaussian::formats::planar_3d::{Gaussian3d, PlanarGaussian3d, PlanarGaussian3dHandle},
    render::{
        CloudPipeline, CloudPipelineKey, CloudUniform, GaussianComputeViewBindGroup,
        GaussianUniformBindGroups, PlanarStorageRebindQueue, shader_defs,
        storage_layout_descriptor,
    },
};

const INTERPOLATE_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("b0b03f7e-9ec2-4e7d-bc96-3ddc1a8c5942");
const WORKGROUP_SIZE: u32 = 256;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct InterpolateLabel;

pub struct InterpolatePlugin<R: PlanarSync> {
    phantom: PhantomData<fn() -> R>,
}

impl<R: PlanarSync> Default for InterpolatePlugin<R> {
    fn default() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<R> Plugin for InterpolatePlugin<R>
where
    R: PlanarSync + Send + Sync + 'static,
    R::GpuPlanarType: GpuPlanarStorage,
    <R::GpuPlanarType as GpuPlanar>::PackedType: ReflectInterleaved,
{
    fn build(&self, app: &mut App) {
        if TypeId::of::<R::PlanarType>() != TypeId::of::<PlanarGaussian3d>() {
            return;
        }

        load_internal_asset!(
            app,
            INTERPOLATE_SHADER_HANDLE,
            "interpolate.wgsl",
            Shader::from_wgsl
        );

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .add_systems(ExtractSchedule, extract_gaussian_interpolate::<R>)
                .add_systems(
                    Render,
                    (queue_gaussian_interpolate_bind_groups::<R>.in_set(RenderSystems::Queue),),
                )
                .add_systems(
                    Core3d,
                    run_gaussian_interpolate::<R>
                        .in_set(InterpolateLabel)
                        .before(Core3dSystems::Prepass),
                );
        }

        app.add_systems(PostUpdate, ensure_gaussian_interpolate_synced::<R>);
        app.add_systems(PostUpdate, ensure_gaussian_interpolate_output_gaussian3d);
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<GaussianInterpolatePipeline<R>>();
        }
    }
}

fn ensure_gaussian_interpolate_synced<R: PlanarSync>(
    mut commands: Commands,
    query: Query<(Entity, Option<&SyncToRenderWorld>), With<GaussianInterpolate<R>>>,
) {
    for (entity, sync_tag) in &query {
        if sync_tag.is_none() {
            debug!(
                ?entity,
                "adding SyncToRenderWorld to GaussianInterpolate entity"
            );
            commands.entity(entity).insert(SyncToRenderWorld);
        }
    }
}

fn ensure_gaussian_interpolate_output_gaussian3d(
    mut commands: Commands,
    mut planar_assets: ResMut<Assets<PlanarGaussian3d>>,
    mut rebind_queue: ResMut<PlanarStorageRebindQueue<Gaussian3d>>,
    query: Query<(
        Entity,
        &GaussianInterpolate<Gaussian3d>,
        Option<&PlanarGaussian3dHandle>,
    )>,
) {
    for (entity, interpolate, existing_output) in &query {
        if existing_output.is_some() {
            continue;
        }

        let lhs_handle = interpolate.lhs.handle();
        let Some(cloned_asset) = planar_assets
            .get(lhs_handle)
            .map(|asset| asset.iter().collect::<PlanarGaussian3d>())
        else {
            debug!(
                ?entity,
                "lhs planar asset not available for GaussianInterpolate output"
            );
            continue;
        };

        let output_handle_raw = planar_assets.add(cloned_asset);
        let output_handle = PlanarGaussian3dHandle(output_handle_raw.clone());

        debug!(?entity, asset_id = ?output_handle_raw.id(), "initialized GaussianInterpolate output asset from lhs");

        rebind_queue.push_unique(output_handle_raw.id());
        commands.entity(entity).insert(output_handle);
    }
}

#[derive(Component, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct GaussianInterpolate<R: PlanarSync> {
    pub lhs: R::PlanarTypeHandle,
    pub rhs: R::PlanarTypeHandle,
}

impl<R: PlanarSync> Clone for GaussianInterpolate<R>
where
    R::PlanarTypeHandle: Clone,
{
    fn clone(&self) -> Self {
        Self {
            lhs: self.lhs.clone(),
            rhs: self.rhs.clone(),
        }
    }
}
#[derive(Component)]
pub struct GaussianInterpolateBindGroups<R: PlanarSync> {
    pub lhs: BindGroup,
    pub rhs: BindGroup,
    pub output: BindGroup,
    phantom: PhantomData<fn() -> R>,
}

#[derive(Resource)]
pub struct GaussianInterpolatePipeline<R: PlanarSync> {
    pub output_layout: BindGroupLayout,
    pub interpolate_pipeline: CachedComputePipelineId,
    phantom: PhantomData<fn() -> R>,
}

impl<R: PlanarSync> FromWorld for GaussianInterpolatePipeline<R>
where
    R::GpuPlanarType: GpuPlanarStorage,
    <R::GpuPlanarType as GpuPlanar>::PackedType: ReflectInterleaved,
{
    fn from_world(render_world: &mut World) -> Self {
        let render_device = render_world.resource::<RenderDevice>();
        let gaussian_cloud_pipeline = render_world.resource::<CloudPipeline<R>>();
        let pipeline_cache = render_world.resource::<PipelineCache>();

        let output_layout = R::GpuPlanarType::bind_group_layout(render_device, false);
        let output_layout_desc = storage_layout_descriptor::<
            <R::GpuPlanarType as GpuPlanar>::PackedType,
        >("gaussian_interpolate_output_layout", false);

        let key = CloudPipelineKey {
            binary_gaussian_op: true,
            ..Default::default()
        };
        let shader_defs = shader_defs(key);

        let interpolate_pipeline =
            pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some("gaussian_interpolate_pipeline".into()),
                layout: vec![
                    gaussian_cloud_pipeline.compute_view_layout_desc.clone(),
                    gaussian_cloud_pipeline.gaussian_uniform_layout_desc.clone(),
                    gaussian_cloud_pipeline.gaussian_cloud_layout_desc.clone(),
                    gaussian_cloud_pipeline.gaussian_cloud_layout_desc.clone(),
                    output_layout_desc,
                ],
                immediate_size: 0,
                shader: INTERPOLATE_SHADER_HANDLE,
                shader_defs,
                entry_point: Some("interpolate_gaussians".into()),
                zero_initialize_workgroup_memory: true,
            });

        Self {
            output_layout,
            interpolate_pipeline,
            phantom: PhantomData,
        }
    }
}

pub fn extract_gaussian_interpolate<R>(
    mut commands: Commands,
    query: Extract<Query<(RenderEntity, &GaussianInterpolate<R>)>>,
) where
    R: PlanarSync,
    R::PlanarTypeHandle: Clone,
{
    let mut extracted: Vec<(Entity, (GaussianInterpolate<R>,))> = Vec::new();

    for (render_entity, component) in query.iter() {
        debug!(?render_entity, "queueing GaussianInterpolate extraction");
        extracted.push((render_entity, (component.clone(),)));
    }

    if extracted.is_empty() {
        debug!("no GaussianInterpolate components extracted this frame");
    } else {
        let count = extracted.len();
        debug!(
            count,
            "inserting GaussianInterpolate components into render world"
        );
        for (entity, bundle) in extracted {
            match commands.get_entity(entity) {
                Ok(mut entity_cmd) => {
                    entity_cmd.insert(bundle);
                }
                Err(_) => {
                    debug!(
                        ?entity,
                        "skipping GaussianInterpolate insertion; render entity missing"
                    );
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn queue_gaussian_interpolate_bind_groups<R: PlanarSync>(
    mut commands: Commands,
    interpolate_pipeline: Res<GaussianInterpolatePipeline<R>>,
    gaussian_cloud_pipeline: Res<CloudPipeline<R>>,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    gpu_planars: Res<RenderAssets<R::GpuPlanarType>>,
    mut rebind_queue: ResMut<PlanarStorageRebindQueue<R>>,
    mut query: Query<(
        Entity,
        Ref<GaussianInterpolate<R>>,
        &R::PlanarTypeHandle,
        Option<&GaussianInterpolateBindGroups<R>>,
    )>,
) where
    R::GpuPlanarType: GpuPlanarStorage,
{
    let inputs_changed = gaussian_cloud_pipeline.is_changed() || gpu_planars.is_changed();
    let mut pending_inserts: Vec<(Entity, GaussianInterpolateBindGroups<R>)> = Vec::new();

    for (entity, interpolate, output_handle, existing) in query.iter_mut() {
        let mut rebuild = inputs_changed || interpolate.is_changed();
        if existing.is_none() {
            rebuild = true;
        }

        if !rebuild {
            debug!(
                ?entity,
                "GaussianInterpolate bind groups unchanged; skipping"
            );
            continue;
        }

        let lhs_handle = interpolate.lhs.handle().clone();
        let rhs_handle = interpolate.rhs.handle().clone();
        let output_asset_handle = output_handle.handle().clone();

        let mut ready = true;
        for (label, handle) in [
            ("lhs", &lhs_handle),
            ("rhs", &rhs_handle),
            ("output", &output_asset_handle),
        ] {
            // Assets created at runtime (like the interpolation output) are not tracked by the AssetServer, so
            // `get_load_state` returns `None` even though the data is ready. Treat `None` as ready and only block
            // while the server explicitly reports a non-loaded state.
            if let Some(load_state) = asset_server.get_load_state(handle.id())
                && !matches!(load_state, LoadState::Loaded)
            {
                debug!(
                    ?entity,
                    handle_label = label,
                    ?load_state,
                    "waiting for GaussianInterpolate asset load"
                );
                ready = false;
                break;
            }

            if gpu_planars.get(handle.id()).is_none() {
                debug!(
                    ?entity,
                    handle_label = label,
                    "GaussianInterpolate GPU asset not ready"
                );
                ready = false;
                break;
            }
        }

        if !ready {
            debug!(?entity, "deferring GaussianInterpolate bind group rebuild");
            continue;
        }

        rebind_queue.push_unique(output_asset_handle.id());

        let lhs_gpu = gpu_planars.get(lhs_handle.id()).unwrap();
        let rhs_gpu = gpu_planars.get(rhs_handle.id()).unwrap();
        let output_gpu = gpu_planars.get(output_asset_handle.id()).unwrap();

        let lhs_bind_group = lhs_gpu.bind_group(
            render_device.as_ref(),
            &gaussian_cloud_pipeline.gaussian_cloud_layout,
        );
        let rhs_bind_group = rhs_gpu.bind_group(
            render_device.as_ref(),
            &gaussian_cloud_pipeline.gaussian_cloud_layout,
        );
        let output_bind_group =
            output_gpu.bind_group(render_device.as_ref(), &interpolate_pipeline.output_layout);

        let gaussian_count = output_gpu.len();
        debug!(
            ?entity,
            gaussian_count, "queued GaussianInterpolate bind groups"
        );

        pending_inserts.push((
            entity,
            GaussianInterpolateBindGroups::<R> {
                lhs: lhs_bind_group,
                rhs: rhs_bind_group,
                output: output_bind_group,
                phantom: PhantomData,
            },
        ));
    }

    if pending_inserts.is_empty() {
        debug!("no GaussianInterpolate bind groups queued this frame");
    } else {
        let count = pending_inserts.len();
        debug!(
            count,
            "inserted GaussianInterpolate bind groups into render world"
        );
        commands.try_insert_batch(pending_inserts);
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn run_gaussian_interpolate<R: PlanarSync>(
    mut render_context: RenderContext,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Res<GaussianInterpolatePipeline<R>>,
    gaussian_uniforms: Res<GaussianUniformBindGroups>,
    gpu_planars: Res<RenderAssets<R::GpuPlanarType>>,
    view_bind_group: ViewQuery<(
        &'static GaussianCamera,
        &'static GaussianComputeViewBindGroup,
        &'static ViewUniformOffset,
        &'static PreviousViewUniformOffset,
    )>,
    gaussian_clouds: Query<(
        &'static GaussianInterpolate<R>,
        &'static GaussianInterpolateBindGroups<R>,
        &'static DynamicUniformIndex<CloudUniform>,
        &'static R::PlanarTypeHandle,
    )>,
) where
    R::GpuPlanarType: GpuPlanarStorage,
{
    match pipeline_cache.get_compute_pipeline_state(pipeline.interpolate_pipeline) {
        CachedPipelineState::Ok(_) => {}
        state => {
            debug!(
                ?state,
                "GaussianInterpolate pipeline not ready; skipping dispatch"
            );
            return;
        }
    }

    let Some(uniform_bind_group) = gaussian_uniforms.base_bind_group.as_ref() else {
        debug!("GaussianInterpolate run skipped: GaussianUniform base bind group missing");
        return;
    };

    let (_camera, view_bind_group, view_uniform_offset, previous_view_uniform_offset) =
        view_bind_group.into_inner();
    let command_encoder = render_context.command_encoder();

    debug!("GaussianInterpolate run starting");

    for (_interpolate, bind_groups, cloud_uniform_index, output_handle) in &gaussian_clouds {
        let output_asset_id = output_handle.handle().id();
        let Some(output_gpu) = gpu_planars.get(output_handle.handle()) else {
            debug!(output_asset_id = ?output_asset_id, "GaussianInterpolate output GPU asset missing");
            continue;
        };

        let gaussian_count = output_gpu.len() as u32;
        if gaussian_count == 0 {
            debug!(output_asset_id = ?output_asset_id, "GaussianInterpolate output has no gaussians; skipping dispatch");
            continue;
        }

        let workgroups = gaussian_count.div_ceil(WORKGROUP_SIZE);
        let pipeline_id = pipeline_cache
            .get_compute_pipeline(pipeline.interpolate_pipeline)
            .unwrap();

        let mut pass = command_encoder.begin_compute_pass(&ComputePassDescriptor::default());

        pass.set_pipeline(pipeline_id);
        pass.set_bind_group(
            0,
            &view_bind_group.value,
            &[
                view_uniform_offset.offset,
                previous_view_uniform_offset.offset,
            ],
        );
        pass.set_bind_group(1, uniform_bind_group, &[cloud_uniform_index.index()]);
        pass.set_bind_group(2, &bind_groups.lhs, &[]);
        pass.set_bind_group(3, &bind_groups.rhs, &[]);
        pass.set_bind_group(4, &bind_groups.output, &[]);

        debug!(
            output_asset_id = ?output_asset_id,
            gaussian_count,
            workgroups,
            uniform_index = cloud_uniform_index.index(),
            "dispatched GaussianInterpolate compute pass"
        );

        pass.dispatch_workgroups(workgroups, 1, 1);
    }

    debug!("GaussianInterpolate run completed");
}
