#[cfg(feature = "morph_interpolate")]
use std::any::TypeId;
use std::collections::HashMap;

use bevy::{
    asset::{load_internal_asset, uuid_handle},
    core_pipeline::{Core3d, Core3dSystems, prepass::PreviousViewUniformOffset},
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        extract_component::DynamicUniformIndex,
        render_asset::RenderAssets,
        render_resource::{
            BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
            BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferBinding,
            BufferBindingType, BufferDescriptor, BufferInitDescriptor, BufferSize, BufferUsages,
            CachedComputePipelineId, CachedPipelineState, ComputePassDescriptor,
            ComputePipelineDescriptor, PipelineCache, ShaderStages,
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        view::ViewUniformOffset,
    },
};
use bevy_interleave::{interface::storage::PlanarStorageBindGroup, prelude::*};
use static_assertions::assert_cfg;

#[cfg(feature = "morph_interpolate")]
use crate::{gaussian::formats::planar_3d::PlanarGaussian3d, morph::interpolate::InterpolateLabel};

use crate::{
    CloudSettings, GaussianCamera, RadixSortDepthBits,
    render::{
        CloudPipeline, CloudPipelineKey, CloudUniform, GaussianUniformBindGroups, ShaderDefines,
        shader_defs_with_defines,
    },
    sort::{GpuSortedEntry, SortEntry, SortMode, SortPluginFlag, SortedEntriesHandle},
};

assert_cfg!(
    not(all(feature = "sort_radix", feature = "buffer_texture",)),
    "sort_radix and buffer_texture are incompatible",
);

const RADIX_SHADER_HANDLE: Handle<Shader> = uuid_handle!("dedb3ddf-f254-4361-8762-e221774de1ed");
const TEMPORAL_SORT_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("11986b71-25d8-410b-adfa-6afb107ae4de");
const RADIX_PIPELINE_RESET: usize = 0;
const RADIX_PIPELINE_A: usize = 1;
const RADIX_PIPELINE_B: usize = 2;
const RADIX_PIPELINE_C_COUNT: usize = 3;
const RADIX_PIPELINE_C_SCAN: usize = 4;
const RADIX_PIPELINE_C_SCATTER: usize = 5;
const RADIX_PIPELINE_COUNT: usize = 6;
const RADIX_DEPTH_BITS_VARIANT_COUNT: usize = 3;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct RadixSortLabel;

#[derive(Default)]
pub struct RadixSortPlugin<R: PlanarSync> {
    phantom: std::marker::PhantomData<R>,
}

impl<R: PlanarSync> Plugin for RadixSortPlugin<R>
where
    R::GpuPlanarType: GpuPlanarStorage,
{
    fn build(&self, app: &mut App) {
        // TODO: run once
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(
                Render,
                (queue_radix_bind_group::<R>.in_set(RenderSystems::Queue),),
            );

            render_app.init_resource::<RadixSortBuffers<R>>();
            render_app.add_systems(ExtractSchedule, update_sort_buffers::<R>);
        }

        if app.is_plugin_added::<SortPluginFlag>() {
            debug!("sort plugin already added");
            return;
        }

        load_internal_asset!(app, RADIX_SHADER_HANDLE, "radix.wgsl", Shader::from_wgsl);

        load_internal_asset!(
            app,
            TEMPORAL_SORT_SHADER_HANDLE,
            "temporal.wgsl",
            Shader::from_wgsl
        );

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            #[cfg(feature = "morph_interpolate")]
            if TypeId::of::<R::PlanarType>() == TypeId::of::<PlanarGaussian3d>() {
                render_app.add_systems(
                    Core3d,
                    run_radix_sort::<R>
                        .in_set(RadixSortLabel)
                        .after(InterpolateLabel)
                        .before(Core3dSystems::Prepass),
                );
            } else {
                render_app.add_systems(
                    Core3d,
                    run_radix_sort::<R>
                        .in_set(RadixSortLabel)
                        .before(Core3dSystems::Prepass),
                );
            }

            #[cfg(not(feature = "morph_interpolate"))]
            render_app.add_systems(
                Core3d,
                run_radix_sort::<R>
                    .in_set(RadixSortLabel)
                    .before(Core3dSystems::Prepass),
            );
        }
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<RadixSortPipeline<R>>();
        }
    }
}

#[derive(Resource)]
pub struct RadixSortBuffers<R: PlanarSync> {
    // TODO: use a more ECS-friendly approach
    pub asset_map: HashMap<AssetId<R::PlanarType>, GpuRadixBuffers>,
}

impl<R: PlanarSync> Default for RadixSortBuffers<R> {
    fn default() -> Self {
        RadixSortBuffers {
            asset_map: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GpuRadixBuffers {
    pub sorting_global_buffer: Buffer,
    pub sorting_status_counter_buffer: Buffer,
    pub sorting_pass_buffers: [Buffer; 4],
    pub entry_buffer_b: Buffer,
}

impl GpuRadixBuffers {
    pub fn new(count: usize, render_device: &RenderDevice) -> Self {
        let sorting_global_buffer = render_device.create_buffer(&BufferDescriptor {
            label: Some("sorting global buffer"),
            size: ShaderDefines::default().sorting_buffer_size as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let sorting_status_counter_buffer = render_device.create_buffer(&BufferDescriptor {
            label: Some("status counters buffer"),
            size: ShaderDefines::default().sorting_status_counters_buffer_size(count) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let sorting_pass_buffers = (0..4)
            .map(|idx| {
                render_device.create_buffer_with_data(&BufferInitDescriptor {
                    label: format!("sorting pass buffer {idx}").as_str().into(),
                    contents: &[idx as u8, 0, 0, 0],
                    usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                })
            })
            .collect::<Vec<Buffer>>()
            .try_into()
            .unwrap();

        let entry_buffer_b = render_device.create_buffer(&BufferDescriptor {
            label: Some("entry buffer b"),
            size: (count * std::mem::size_of::<SortEntry>()) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        GpuRadixBuffers {
            sorting_global_buffer,
            sorting_status_counter_buffer,
            sorting_pass_buffers,
            entry_buffer_b,
        }
    }
}

fn update_sort_buffers<R: PlanarSync>(
    gpu_gaussian_clouds: Res<RenderAssets<R::GpuPlanarType>>,
    mut sort_buffers: ResMut<RadixSortBuffers<R>>,
    render_device: Res<RenderDevice>,
) {
    for (asset_id, cloud) in gpu_gaussian_clouds.iter() {
        // TODO: handle cloud resize operations and resolve leaked stale buffers
        if sort_buffers.asset_map.contains_key(&asset_id) {
            continue;
        }

        let gpu_radix_buffers = GpuRadixBuffers::new(cloud.len(), &render_device);
        sort_buffers.asset_map.insert(asset_id, gpu_radix_buffers);
    }
}

#[derive(Resource)]
pub struct RadixSortPipeline<R: PlanarSync> {
    pub radix_sort_layout: BindGroupLayout,
    pub variants: [Option<RadixSortPipelineVariant>; RADIX_DEPTH_BITS_VARIANT_COUNT],
    sorting_layout: Vec<BindGroupLayoutDescriptor>,
    phantom: std::marker::PhantomData<R>,
}

#[derive(Clone, Copy)]
pub struct RadixSortPipelineVariant {
    pub shader_defines: ShaderDefines,
    pub radix_sort_pipelines: [CachedComputePipelineId; RADIX_PIPELINE_COUNT],
}

impl RadixSortPipelineVariant {
    fn is_loaded(&self, pipeline_cache: &PipelineCache) -> bool {
        self.radix_sort_pipelines.iter().all(|sort_pipeline| {
            matches!(
                pipeline_cache.get_compute_pipeline_state(*sort_pipeline),
                CachedPipelineState::Ok(_)
            )
        })
    }
}

impl<R: PlanarSync> RadixSortPipeline<R> {
    fn variant(
        &self,
        radix_sort_depth_bits: RadixSortDepthBits,
    ) -> Option<&RadixSortPipelineVariant> {
        self.variants[radix_sort_depth_bits.pipeline_index()].as_ref()
    }

    fn queue_variant(
        &mut self,
        pipeline_cache: &PipelineCache,
        radix_sort_depth_bits: RadixSortDepthBits,
    ) {
        let index = radix_sort_depth_bits.pipeline_index();
        if self.variants[index].is_some() {
            return;
        }

        self.variants[index] = Some(queue_radix_sort_pipeline_variant(
            pipeline_cache,
            self.sorting_layout.clone(),
            radix_sort_depth_bits,
        ));
    }
}

impl<R: PlanarSync> FromWorld for RadixSortPipeline<R> {
    fn from_world(render_world: &mut World) -> Self {
        let render_device = render_world.resource::<RenderDevice>();
        let gaussian_cloud_pipeline = render_world.resource::<CloudPipeline<R>>();

        let sorting_buffer_entry = BindGroupLayoutEntry {
            binding: 1,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: BufferSize::new(
                    ShaderDefines::default().sorting_buffer_size as u64,
                ),
            },
            count: None,
        };

        let sorting_status_counters_buffer_entry = BindGroupLayoutEntry {
            binding: 2,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: BufferSize::new(
                    ShaderDefines::default().sorting_status_counters_buffer_size(1) as u64,
                ),
            },
            count: None,
        };

        let draw_indirect_buffer_entry = BindGroupLayoutEntry {
            binding: 3,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: BufferSize::new(
                    std::mem::size_of::<wgpu::util::DrawIndirectArgs>() as u64,
                ),
            },
            count: None,
        };

        let radix_sort_layout_entries = [
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: BufferSize::new(std::mem::size_of::<u32>() as u64),
                },
                count: None,
            },
            sorting_buffer_entry,
            sorting_status_counters_buffer_entry,
            draw_indirect_buffer_entry,
            BindGroupLayoutEntry {
                binding: 4,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: BufferSize::new(std::mem::size_of::<SortEntry>() as u64),
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 5,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: BufferSize::new(std::mem::size_of::<SortEntry>() as u64),
                },
                count: None,
            },
        ];
        let radix_sort_layout_desc =
            BindGroupLayoutDescriptor::new("radix_sort_layout", &radix_sort_layout_entries);
        let radix_sort_layout = render_device
            .create_bind_group_layout(Some("radix_sort_layout"), &radix_sort_layout_entries);

        let sorting_layout = vec![
            gaussian_cloud_pipeline.compute_view_layout_desc.clone(),
            gaussian_cloud_pipeline.gaussian_uniform_layout_desc.clone(),
            gaussian_cloud_pipeline.gaussian_cloud_layout_desc.clone(),
            radix_sort_layout_desc.clone(),
        ];

        let variants = [None; RADIX_DEPTH_BITS_VARIANT_COUNT];

        RadixSortPipeline {
            radix_sort_layout,
            variants,
            sorting_layout,
            phantom: std::marker::PhantomData,
        }
    }
}

fn queue_radix_sort_pipeline_variant(
    pipeline_cache: &PipelineCache,
    sorting_layout: Vec<BindGroupLayoutDescriptor>,
    radix_sort_depth_bits: RadixSortDepthBits,
) -> RadixSortPipelineVariant {
    let shader_defines = ShaderDefines::for_radix_depth_bits(radix_sort_depth_bits);
    let shader_defs = shader_defs_with_defines(CloudPipelineKey::default(), shader_defines);
    let label_suffix = radix_sort_depth_bits.bits();

    let radix_reset = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some(format!("radix_sort_reset_{label_suffix}bit").into()),
        layout: sorting_layout.clone(),
        immediate_size: 0,
        shader: RADIX_SHADER_HANDLE,
        shader_defs: shader_defs.clone(),
        entry_point: Some("radix_reset".into()),
        zero_initialize_workgroup_memory: true,
    });

    let radix_sort_a = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some(format!("radix_sort_a_{label_suffix}bit").into()),
        layout: sorting_layout.clone(),
        immediate_size: 0,
        shader: RADIX_SHADER_HANDLE,
        shader_defs: shader_defs.clone(),
        entry_point: Some("radix_sort_a".into()),
        zero_initialize_workgroup_memory: true,
    });

    let radix_sort_b = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some(format!("radix_sort_b_{label_suffix}bit").into()),
        layout: sorting_layout.clone(),
        immediate_size: 0,
        shader: RADIX_SHADER_HANDLE,
        shader_defs: shader_defs.clone(),
        entry_point: Some("radix_sort_b".into()),
        zero_initialize_workgroup_memory: true,
    });

    let radix_sort_c_count = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some(format!("radix_sort_c_count_tiles_{label_suffix}bit").into()),
        layout: sorting_layout.clone(),
        immediate_size: 0,
        shader: RADIX_SHADER_HANDLE,
        shader_defs: shader_defs.clone(),
        entry_point: Some("radix_sort_c_count_tiles".into()),
        zero_initialize_workgroup_memory: true,
    });

    let radix_sort_c_scan = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some(format!("radix_sort_c_scan_tiles_{label_suffix}bit").into()),
        layout: sorting_layout.clone(),
        immediate_size: 0,
        shader: RADIX_SHADER_HANDLE,
        shader_defs: shader_defs.clone(),
        entry_point: Some("radix_sort_c_scan_tiles".into()),
        zero_initialize_workgroup_memory: true,
    });

    let radix_sort_c_scatter = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some(format!("radix_sort_c_scatter_{label_suffix}bit").into()),
        layout: sorting_layout,
        immediate_size: 0,
        shader: RADIX_SHADER_HANDLE,
        shader_defs,
        entry_point: Some("radix_sort_c_scatter".into()),
        zero_initialize_workgroup_memory: true,
    });

    RadixSortPipelineVariant {
        shader_defines,
        radix_sort_pipelines: [
            radix_reset,
            radix_sort_a,
            radix_sort_b,
            radix_sort_c_count,
            radix_sort_c_scan,
            radix_sort_c_scatter,
        ],
    }
}

#[derive(Component)]
pub struct RadixBindGroup {
    // For each digit pass idx in 0..RADIX_DIGIT_PLACES, we create 2 bind groups (parity 0/1):
    // index = pass_idx * 2 + parity (parity 0: input=sorted_entries, output=entry_buffer_b; parity 1: input=entry_buffer_b, output=sorted_entries)
    pub radix_sort_bind_groups: [BindGroup; 8],
}

type RadixViewQueryItem = (
    &'static GaussianCamera,
    &'static crate::render::GaussianComputeViewBindGroup,
    &'static ViewUniformOffset,
    &'static PreviousViewUniformOffset,
);

#[allow(clippy::too_many_arguments)]
pub fn queue_radix_bind_group<R: PlanarSync>(
    mut commands: Commands,
    mut radix_pipeline: ResMut<RadixSortPipeline<R>>,
    pipeline_cache: Res<PipelineCache>,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    gaussian_cloud_res: Res<RenderAssets<R::GpuPlanarType>>,
    sorted_entries_res: Res<RenderAssets<GpuSortedEntry>>,
    gaussian_clouds: Query<(
        Entity,
        &R::PlanarTypeHandle,
        &SortedEntriesHandle,
        &CloudSettings,
    )>,
    sort_buffers: Res<RadixSortBuffers<R>>,
) where
    R::GpuPlanarType: GpuPlanarStorage,
{
    for (entity, cloud_handle, sorted_entries_handle, settings) in gaussian_clouds.iter() {
        if settings.sort_mode != SortMode::Radix {
            commands.entity(entity).remove::<RadixBindGroup>();
            continue;
        }

        radix_pipeline.queue_variant(&pipeline_cache, settings.radix_sort_depth_bits);

        // TODO: deduplicate asset load checks
        if let Some(load_state) = asset_server.get_load_state(cloud_handle.handle())
            && load_state.is_loading()
        {
            continue;
        }

        if gaussian_cloud_res.get(cloud_handle.handle()).is_none() {
            continue;
        }

        if let Some(load_state) = asset_server.get_load_state(&sorted_entries_handle.0)
            && load_state.is_loading()
        {
            continue;
        }

        if sorted_entries_res.get(sorted_entries_handle).is_none() {
            continue;
        }

        if !sort_buffers
            .asset_map
            .contains_key(&cloud_handle.handle().id())
        {
            continue;
        }

        let cloud = gaussian_cloud_res.get(cloud_handle.handle()).unwrap();
        let sorted_entries = sorted_entries_res.get(sorted_entries_handle).unwrap();
        let sorting_assets = &sort_buffers.asset_map[&cloud_handle.handle().id()];

        let sorting_global_entry = BindGroupEntry {
            binding: 1,
            resource: BindingResource::Buffer(BufferBinding {
                buffer: &sorting_assets.sorting_global_buffer,
                offset: 0,
                size: BufferSize::new(sorting_assets.sorting_global_buffer.size()),
            }),
        };

        let sorting_status_counters_entry = BindGroupEntry {
            binding: 2,
            resource: BindingResource::Buffer(BufferBinding {
                buffer: &sorting_assets.sorting_status_counter_buffer,
                offset: 0,
                size: BufferSize::new(sorting_assets.sorting_status_counter_buffer.size()),
            }),
        };

        let draw_indirect_entry = BindGroupEntry {
            binding: 3,
            resource: BindingResource::Buffer(BufferBinding {
                buffer: cloud.draw_indirect_buffer(),
                offset: 0,
                size: BufferSize::new(cloud.draw_indirect_buffer().size()),
            }),
        };

        let radix_sort_bind_groups: [BindGroup; 8] = {
            let mut groups: Vec<BindGroup> = Vec::with_capacity(8);
            for pass_idx in 0..4 {
                for parity in 0..=1 {
                    let (input_buf, output_buf) = if parity == 0 {
                        (
                            &sorted_entries.sorted_entry_buffer,
                            &sorting_assets.entry_buffer_b,
                        )
                    } else {
                        (
                            &sorting_assets.entry_buffer_b,
                            &sorted_entries.sorted_entry_buffer,
                        )
                    };

                    let group = render_device.create_bind_group(
                        format!("radix_sort_bind_group pass={pass_idx} parity={parity}").as_str(),
                        &radix_pipeline.radix_sort_layout,
                        &[
                            // sorting_pass_index (u32) == pass_idx regardless of parity
                            BindGroupEntry {
                                binding: 0,
                                resource: BindingResource::Buffer(BufferBinding {
                                    buffer: &sorting_assets.sorting_pass_buffers[pass_idx],
                                    offset: 0,
                                    size: BufferSize::new(std::mem::size_of::<u32>() as u64),
                                }),
                            },
                            sorting_global_entry.clone(),
                            sorting_status_counters_entry.clone(),
                            draw_indirect_entry.clone(),
                            // input_entries
                            BindGroupEntry {
                                binding: 4,
                                resource: BindingResource::Buffer(BufferBinding {
                                    buffer: input_buf,
                                    offset: 0,
                                    size: BufferSize::new(
                                        (cloud.len() * std::mem::size_of::<SortEntry>()) as u64,
                                    ),
                                }),
                            },
                            // output_entries
                            BindGroupEntry {
                                binding: 5,
                                resource: BindingResource::Buffer(BufferBinding {
                                    buffer: output_buf,
                                    offset: 0,
                                    size: BufferSize::new(
                                        (cloud.len() * std::mem::size_of::<SortEntry>()) as u64,
                                    ),
                                }),
                            },
                        ],
                    );
                    groups.push(group);
                }
            }
            groups.try_into().unwrap()
        };

        commands.entity(entity).insert(RadixBindGroup {
            radix_sort_bind_groups,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn run_radix_sort<R: PlanarSync>(
    mut render_context: RenderContext,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Res<RadixSortPipeline<R>>,
    gaussian_uniforms: Res<GaussianUniformBindGroups>,
    sort_buffers: Res<RadixSortBuffers<R>>,
    gpu_planars: Res<RenderAssets<R::GpuPlanarType>>,
    view_bind_group: ViewQuery<RadixViewQueryItem>,
    gaussian_clouds: Query<(
        &'static <R as PlanarSync>::PlanarTypeHandle,
        &'static PlanarStorageBindGroup<R>,
        &'static RadixBindGroup,
        &'static DynamicUniformIndex<CloudUniform>,
        &'static CloudSettings,
    )>,
) where
    R::GpuPlanarType: GpuPlanarStorage,
{
    let (_camera, view_bind_group, view_uniform_offset, previous_view_uniform_offset) =
        view_bind_group.into_inner();

    let Some(uniform_bind_group) = gaussian_uniforms.base_bind_group.as_ref() else {
        debug!("RadixSort run skipped: GaussianUniform base bind group missing");
        return;
    };

    for (cloud_handle, cloud_bind_group, radix_bind_group, cloud_uniform_index, cloud_settings) in
        &gaussian_clouds
    {
        let Some(cloud) = gpu_planars.get(cloud_handle.handle()) else {
            continue;
        };

        let Some(sorting_assets) = sort_buffers.asset_map.get(&cloud_handle.handle().id()) else {
            continue;
        };

        let Some(pipeline_variant) = pipeline.variant(cloud_settings.radix_sort_depth_bits) else {
            continue;
        };
        if !pipeline_variant.is_loaded(&pipeline_cache) {
            continue;
        }

        let command_encoder = render_context.command_encoder();
        let shader_defines = pipeline_variant.shader_defines;
        let radix_digit_places = shader_defines.radix_digit_places;
        let initial_parity = shader_defines.radix_initial_parity();
        let workgroup_entries_a = shader_defines.workgroup_entries_a;
        let workgroup_entries_c = shader_defines.workgroup_entries_c;
        let tile_workgroups = (cloud.len() as u32).div_ceil(workgroup_entries_c);

        command_encoder.clear_buffer(&sorting_assets.sorting_global_buffer, 0, None);
        command_encoder.clear_buffer(&sorting_assets.sorting_status_counter_buffer, 0, None);
        command_encoder.clear_buffer(cloud.draw_indirect_buffer(), 0, None);

        {
            let mut pass = command_encoder.begin_compute_pass(&ComputePassDescriptor::default());

            // Reset per-frame counters/histograms
            let radix_reset = pipeline_cache
                .get_compute_pipeline(pipeline_variant.radix_sort_pipelines[RADIX_PIPELINE_RESET])
                .unwrap();
            pass.set_pipeline(radix_reset);
            pass.set_bind_group(
                0,
                &view_bind_group.value,
                &[
                    view_uniform_offset.offset,
                    previous_view_uniform_offset.offset,
                ],
            );
            pass.set_bind_group(1, uniform_bind_group, &[cloud_uniform_index.index()]);
            pass.set_bind_group(2, &cloud_bind_group.bind_group, &[]);
            pass.set_bind_group(
                3,
                &radix_bind_group.radix_sort_bind_groups[initial_parity],
                &[],
            );
            pass.dispatch_workgroups(1, 1, 1);

            let radix_sort_a = pipeline_cache
                .get_compute_pipeline(pipeline_variant.radix_sort_pipelines[RADIX_PIPELINE_A])
                .unwrap();
            pass.set_pipeline(radix_sort_a);

            pass.dispatch_workgroups((cloud.len() as u32).div_ceil(workgroup_entries_a), 1, 1);

            let radix_sort_b = pipeline_cache
                .get_compute_pipeline(pipeline_variant.radix_sort_pipelines[RADIX_PIPELINE_B])
                .unwrap();
            pass.set_pipeline(radix_sort_b);

            pass.dispatch_workgroups(1, radix_digit_places, 1);
        }

        // TODO: add options to only complete a fraction of the sorting process
        for pass_idx in 0..radix_digit_places {
            let mut pass = command_encoder.begin_compute_pass(&ComputePassDescriptor::default());

            // Set common bind groups for view/uniforms and cloud storage
            pass.set_bind_group(
                0,
                &view_bind_group.value,
                &[
                    view_uniform_offset.offset,
                    previous_view_uniform_offset.offset,
                ],
            );
            pass.set_bind_group(1, uniform_bind_group, &[cloud_uniform_index.index()]);
            pass.set_bind_group(2, &cloud_bind_group.bind_group, &[]);

            // Choose the initial parity so the final pass writes to sorted_entries.
            let parity = ((pass_idx as usize) + initial_parity) % 2;
            let bg_index = (pass_idx as usize) * 2 + parity;
            pass.set_bind_group(3, &radix_bind_group.radix_sort_bind_groups[bg_index], &[]);

            let radix_sort_c_count = pipeline_cache
                .get_compute_pipeline(pipeline_variant.radix_sort_pipelines[RADIX_PIPELINE_C_COUNT])
                .unwrap();
            pass.set_pipeline(radix_sort_c_count);
            pass.dispatch_workgroups(1, tile_workgroups, 1);

            let radix_sort_c_scan = pipeline_cache
                .get_compute_pipeline(pipeline_variant.radix_sort_pipelines[RADIX_PIPELINE_C_SCAN])
                .unwrap();
            pass.set_pipeline(radix_sort_c_scan);
            // ONE workgroup of RADIX_BASE lanes (lane = digit), not RADIX_BASE single-lane
            // workgroups -- see `radix_sort_c_scan_tiles`'s @workgroup_size in radix.wgsl.
            pass.dispatch_workgroups(1, 1, 1);

            let radix_sort_c_scatter = pipeline_cache
                .get_compute_pipeline(
                    pipeline_variant.radix_sort_pipelines[RADIX_PIPELINE_C_SCATTER],
                )
                .unwrap();
            pass.set_pipeline(radix_sort_c_scatter);
            pass.dispatch_workgroups(1, tile_workgroups, 1);
        }
    }
}
