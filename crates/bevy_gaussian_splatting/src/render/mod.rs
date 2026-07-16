#![allow(dead_code)] // ShaderType derives emit unused check helpers
use std::{borrow::Cow, hash::Hash, num::NonZero};

use bevy::render::render_resource::TextureFormat;
use bevy::shader::ShaderDefVal;
use bevy::{
    asset::{AssetEvent, AssetId, load_internal_asset, uuid_handle},
    camera::primitives::Aabb,
    core_pipeline::{
        core_3d::{Transparent3d, TransparentSortingInfo3d},
        prepass::{
            MotionVectorPrepass, PreviousViewData, PreviousViewUniformOffset, PreviousViewUniforms,
        },
    },
    ecs::{
        query::ROQueryItem,
        system::{SystemParamItem, lifetimeless::*},
    },
    pbr::PrepassViewBindGroup,
    prelude::*,
    render::{
        Extract, Render, RenderApp, RenderSystems,
        extract_component::{ComponentUniforms, DynamicUniformIndex, UniformComponentPlugin},
        globals::{GlobalsBuffer, GlobalsUniform},
        render_asset::RenderAssets,
        render_phase::{
            AddRenderCommand, DrawFunctions, PhaseItem, PhaseItemExtraIndex, RenderCommand,
            RenderCommandResult, SetItemPipeline, TrackedRenderPass, ViewSortedRenderPhases,
        },
        render_resource::*,
        renderer::RenderDevice,
        sync_world::{MainEntity, RenderEntity},
        view::{
            ExtractedView, RenderVisibilityRanges, RenderVisibleEntities,
            VISIBILITY_RANGES_STORAGE_BUFFER_COUNT, ViewUniform, ViewUniformOffset, ViewUniforms,
        },
    },
};
use bevy_interleave::prelude::*;

#[cfg(feature = "buffer_storage")]
use crate::sort::SortEntry;
use crate::{
    camera::GaussianCamera,
    gaussian::{
        cloud::CloudVisibilityClass,
        interface::CommonCloud,
        settings::{
            CloudSettings, DrawMode, GaussianColorSpace, GaussianMode, RadixSortDepthBits,
            RasterizeMode,
        },
    },
    material::{
        spherical_harmonics::{HALF_SH_COEFF_COUNT, SH_COEFF_COUNT, SH_DEGREE, SH_VEC4_PLANES},
        spherindrical_harmonics::{SH_4D_COEFF_COUNT, SH_4D_DEGREE_TIME},
    },
    morph::MorphPlugin,
    sort::{GpuSortedEntry, SortPlugin, SortTrigger, SortedEntriesHandle},
};

#[cfg(feature = "packed")]
mod packed;

#[cfg(feature = "buffer_storage")]
mod planar;

#[cfg(all(feature = "buffer_texture", not(feature = "buffer_storage")))]
mod texture;

const BINDINGS_SHADER_HANDLE: Handle<Shader> = uuid_handle!("cfd9a3d9-a0cb-40c8-ab0b-073110a02474");
const GAUSSIAN_SHADER_HANDLE: Handle<Shader> = uuid_handle!("9a18d83b-137d-4f44-9628-e2defc4b62b0");
const GAUSSIAN_2D_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("713fb941-b4f5-408e-bbde-32fb7dc447ce");
const GAUSSIAN_3D_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("b7eb322b-983b-4ce0-a5a2-3c0d6cb06d65");
const GAUSSIAN_4D_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("26234995-0932-4dfa-ab8d-53df1e779dd4");
const HELPERS_SHADER_HANDLE: Handle<Shader> = uuid_handle!("9ca57ab0-07de-4a43-94f8-547c38e292cb");
const PACKED_SHADER_HANDLE: Handle<Shader> = uuid_handle!("5bb62086-7004-4575-9972-274dc8acccf1");
const PLANAR_SHADER_HANDLE: Handle<Shader> = uuid_handle!("d6a3f978-f795-4786-8475-26366f28d852");
const TEXTURE_SHADER_HANDLE: Handle<Shader> = uuid_handle!("500e2ebf-51a8-402e-9c88-e0d5152c3486");
const TRANSFORM_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("648516b2-87cc-4937-ae1c-d986952e9fa7");

// TODO: consider refactor to bind via bevy's mesh (dynamic vertex planes) + shared batching/instancing/preprocessing
//       utilize RawBufferVec<T> for gaussian data?
pub struct RenderPipelinePlugin<R: PlanarSync> {
    _phantom: std::marker::PhantomData<R>,
}

impl<R: PlanarSync> Default for RenderPipelinePlugin<R> {
    fn default() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<R: PlanarSync> Plugin for RenderPipelinePlugin<R>
where
    R::PlanarType: CommonCloud,
    R::GpuPlanarType: GpuPlanarStorage,
    <R::GpuPlanarType as GpuPlanar>::PackedType: ReflectInterleaved,
{
    fn build(&self, app: &mut App) {
        debug!("building render pipeline plugin");

        app.add_plugins(MorphPlugin::<R>::default());
        app.add_plugins(SortPlugin::<R>::default());
        app.init_resource::<PlanarStorageRebindQueue<R>>();
        app.add_systems(PostUpdate, queue_planar_storage_rebinds::<R>);

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .add_render_command::<Transparent3d, DrawGaussians<R>>()
                .init_resource::<GaussianUniformBindGroups>()
                .init_resource::<PlanarStorageRebindQueue<R>>()
                .add_systems(
                    ExtractSchedule,
                    (
                        extract_gaussians::<R>,
                        extract_planar_storage_rebind_queue::<R>,
                    ),
                )
                .add_systems(
                    Render,
                    (
                        refresh_planar_storage_bind_groups::<R>
                            .in_set(RenderSystems::PrepareBindGroups),
                        queue_gaussian_bind_group::<R>.in_set(RenderSystems::PrepareBindGroups),
                        queue_gaussian_view_bind_groups::<R>
                            .in_set(RenderSystems::PrepareBindGroups),
                        queue_gaussian_compute_view_bind_groups::<R>
                            .in_set(RenderSystems::PrepareBindGroups),
                        queue_gaussians::<R>.in_set(RenderSystems::Queue),
                    ),
                );
        }

        // TODO: refactor common resources into a common plugin
        if app.is_plugin_added::<UniformComponentPlugin<CloudUniform>>() {
            debug!("render plugin already added");
            return;
        }

        load_internal_asset!(
            app,
            BINDINGS_SHADER_HANDLE,
            "bindings.wgsl",
            Shader::from_wgsl
        );

        load_internal_asset!(
            app,
            GAUSSIAN_SHADER_HANDLE,
            "gaussian.wgsl",
            Shader::from_wgsl
        );

        load_internal_asset!(
            app,
            GAUSSIAN_2D_SHADER_HANDLE,
            "gaussian_2d.wgsl",
            Shader::from_wgsl
        );

        load_internal_asset!(
            app,
            GAUSSIAN_3D_SHADER_HANDLE,
            "gaussian_3d.wgsl",
            Shader::from_wgsl
        );

        load_internal_asset!(
            app,
            GAUSSIAN_4D_SHADER_HANDLE,
            "gaussian_4d.wgsl",
            Shader::from_wgsl
        );

        load_internal_asset!(
            app,
            HELPERS_SHADER_HANDLE,
            "helpers.wgsl",
            Shader::from_wgsl
        );

        load_internal_asset!(app, PACKED_SHADER_HANDLE, "packed.wgsl", Shader::from_wgsl);

        load_internal_asset!(app, PLANAR_SHADER_HANDLE, "planar.wgsl", Shader::from_wgsl);

        load_internal_asset!(
            app,
            TEXTURE_SHADER_HANDLE,
            "texture.wgsl",
            Shader::from_wgsl
        );

        load_internal_asset!(
            app,
            TRANSFORM_SHADER_HANDLE,
            "transform.wgsl",
            Shader::from_wgsl
        );

        app.add_plugins(UniformComponentPlugin::<CloudUniform>::default());

        #[cfg(all(feature = "buffer_texture", not(feature = "buffer_storage")))]
        app.add_plugins(texture::BufferTexturePlugin);
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<CloudPipeline<R>>()
                .init_resource::<SpecializedRenderPipelines<CloudPipeline<R>>>();
        }
    }
}

#[derive(Resource)]
pub struct PlanarStorageRebindQueue<R: PlanarSync> {
    handles: Vec<AssetId<R::PlanarType>>,
    marker: std::marker::PhantomData<R>,
}

impl<R: PlanarSync> Default for PlanarStorageRebindQueue<R> {
    fn default() -> Self {
        Self {
            handles: Vec::new(),
            marker: std::marker::PhantomData,
        }
    }
}

impl<R: PlanarSync> Clone for PlanarStorageRebindQueue<R> {
    fn clone(&self) -> Self {
        Self {
            handles: self.handles.clone(),
            marker: std::marker::PhantomData,
        }
    }
}

impl<R: PlanarSync> PlanarStorageRebindQueue<R> {
    pub fn push_unique(&mut self, id: AssetId<R::PlanarType>) {
        if !self.handles.contains(&id) {
            self.handles.push(id);
        }
    }
}

fn queue_planar_storage_rebinds<R: PlanarSync>(
    mut events: MessageReader<AssetEvent<R::PlanarType>>,
    mut queue: ResMut<PlanarStorageRebindQueue<R>>,
) {
    for event in events.read() {
        match event {
            AssetEvent::Modified { id } | AssetEvent::LoadedWithDependencies { id } => {
                queue.push_unique(*id);
            }
            AssetEvent::Removed { id } => {
                queue.handles.retain(|handle_id| handle_id != id);
            }
            AssetEvent::Added { .. } | AssetEvent::Unused { .. } => {}
        }
    }
}

fn extract_planar_storage_rebind_queue<R: PlanarSync>(
    mut commands: Commands,
    mut main_world: ResMut<bevy::render::MainWorld>,
) {
    let mut queue = main_world.resource_mut::<PlanarStorageRebindQueue<R>>();
    commands.insert_resource(queue.clone());
    queue.handles.clear();
}

fn refresh_planar_storage_bind_groups<R: PlanarSync>(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    gpu_planars: Res<RenderAssets<R::GpuPlanarType>>,
    bind_group_layouts: Res<bevy_interleave::interface::storage::PlanarStorageLayouts<R>>,
    mut queue: ResMut<PlanarStorageRebindQueue<R>>,
    query: Query<(Entity, &R::PlanarTypeHandle)>,
) where
    R::GpuPlanarType: GpuPlanarStorage,
{
    if queue.handles.is_empty() {
        return;
    }

    let layout = &bind_group_layouts.bind_group_layout;
    let handles = queue.handles.clone();
    queue.handles.clear();

    for id in handles {
        for (entity, planar_handle) in query.iter() {
            if planar_handle.handle().id() != id {
                continue;
            }

            if let Some(gpu_planar) = gpu_planars.get(planar_handle.handle()) {
                let bind_group = gpu_planar.bind_group(render_device.as_ref(), layout);

                commands.entity(entity).insert(PlanarStorageBindGroup::<R> {
                    bind_group,
                    phantom: std::marker::PhantomData,
                });
            }
        }
    }
}

#[derive(Bundle)]
pub struct GpuCloudBundle<R: PlanarSync> {
    pub aabb: Aabb,
    pub settings: CloudSettings,
    pub settings_uniform: CloudUniform,
    pub sorted_entries: SortedEntriesHandle,
    pub cloud_handle: R::PlanarTypeHandle,
    pub transform: GlobalTransform,
}

#[allow(type_alias_bounds)]
type GpuCloudBundleQuery<R: bevy_interleave::prelude::PlanarSync> = (
    Entity,
    &'static <R as bevy_interleave::prelude::PlanarSync>::PlanarTypeHandle,
    &'static Aabb,
    &'static SortedEntriesHandle,
    &'static CloudSettings,
    &'static GlobalTransform,
);

#[allow(type_alias_bounds)]
type GpuCloudBindGroupQuery<R: bevy_interleave::prelude::PlanarSync> = (
    Entity,
    &'static <R as bevy_interleave::prelude::PlanarSync>::PlanarTypeHandle,
    &'static SortedEntriesHandle,
    Option<&'static SortBindGroup>,
);

#[allow(clippy::too_many_arguments)]
fn queue_gaussians<R: PlanarSync>(
    gaussian_cloud_uniform: Res<ComponentUniforms<CloudUniform>>,
    transparent_3d_draw_functions: Res<DrawFunctions<Transparent3d>>,
    custom_pipeline: Res<CloudPipeline<R>>,
    mut pipelines: ResMut<SpecializedRenderPipelines<CloudPipeline<R>>>,
    pipeline_cache: Res<PipelineCache>,
    gaussian_clouds: Res<RenderAssets<R::GpuPlanarType>>,
    sorted_entries: Res<RenderAssets<GpuSortedEntry>>,
    mut transparent_render_phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    mut views: Query<(
        &ExtractedView,
        &GaussianCamera,
        &RenderVisibleEntities,
        Option<&Msaa>,
    )>,
    gaussian_splatting_bundles: Query<GpuCloudBundleQuery<R>>,
) {
    debug!("queue_gaussians");

    let warmup = views.iter().any(|(_, camera, _, _)| camera.warmup);
    if warmup {
        debug!("skipping gaussian cloud render during warmup");
        return;
    }

    // TODO: condition this system based on CloudBindGroup attachment
    if gaussian_cloud_uniform.buffer().is_none() {
        debug!("uniform buffer not initialized");
        return;
    };

    let draw_custom = transparent_3d_draw_functions
        .read()
        .id::<DrawGaussians<R>>();

    for (view, _, visible_entities, msaa) in &mut views {
        debug!("queue gaussians view");
        let Some(transparent_phase) = transparent_render_phases.get_mut(&view.retained_view_entity)
        else {
            debug!("transparent phase not found");
            continue;
        };

        debug!("visible entities...");
        let Some(visible_class) = visible_entities.get::<CloudVisibilityClass>() else {
            continue;
        };

        for (render_entity, visible_entity) in &visible_class.entities_cpu_culling {
            if gaussian_splatting_bundles.get(*render_entity).is_err() {
                debug!("gaussian splatting bundle not found");
                continue;
            }

            let (_entity, cloud_handle, aabb, sorted_entries_handle, settings, transform) =
                gaussian_splatting_bundles.get(*render_entity).unwrap();

            debug!("queue gaussians clouds");
            if gaussian_clouds.get(cloud_handle.handle()).is_none() {
                debug!("gaussian cloud asset not found");
                return;
            }

            if sorted_entries.get(sorted_entries_handle).is_none() {
                debug!("sorted entries asset not found");
                return;
            }

            let msaa = msaa.cloned().unwrap_or_default();

            let key = CloudPipelineKey {
                aabb: settings.aabb,
                binary_gaussian_op: false,
                opacity_adaptive_radius: settings.opacity_adaptive_radius,
                visualize_bounding_box: settings.visualize_bounding_box,
                draw_mode: settings.draw_mode,
                gaussian_mode: settings.gaussian_mode,
                rasterize_mode: settings.rasterize_mode,
                sample_count: msaa.samples(),
                hdr: view.target_format == TextureFormat::Rgba16Float,
                depth_write_pass: false,
            };

            let pipeline = pipelines.specialize(&pipeline_cache, &custom_pipeline, key);

            let rangefinder = view.rangefinder3d();
            let aabb_center = (aabb.min() + aabb.max()) / 2.0;
            let aabb_size = aabb.max() - aabb.min();
            let center = *transform
                * GlobalTransform::from(
                    Transform::from_translation(aabb_center.into()).with_scale(aabb_size.into()),
                );
            let distance = rangefinder.distance(&center.translation());

            transparent_phase.add_transient(Transparent3d {
                sorting_info: TransparentSortingInfo3d::Sorted {
                    mesh_center: center.translation(),
                    depth_bias: 0.0,
                },
                entity: (*render_entity, *visible_entity),
                draw_function: draw_custom,
                distance,
                pipeline,
                batch_range: 0..1,
                extra_index: PhaseItemExtraIndex::None,
                indexed: false,
            });

            // renzora patch(depth-write): second, depth-only draw of the cloud
            // at the very tail of the transparent phase (+inf sorts last in the
            // ascending back-to-front order, and depth-only writes merge
            // correctly in any order via GreaterEqual). It runs after all
            // blended color so it can't disturb blending, but populates the
            // view depth texture before the depth-reading post passes
            // (volumetric fog, DoF) sample it. The phase's item map is keyed
            // by (entity, main_entity), so the placeholder main entity keeps
            // this from overwriting the color item; draw commands only ever
            // fetch by the render entity.
            let depth_pipeline = pipelines.specialize(
                &pipeline_cache,
                &custom_pipeline,
                CloudPipelineKey {
                    depth_write_pass: true,
                    ..key
                },
            );
            transparent_phase.add_transient(Transparent3d {
                sorting_info: TransparentSortingInfo3d::Sorted {
                    mesh_center: center.translation(),
                    depth_bias: f32::INFINITY,
                },
                entity: (*render_entity, MainEntity::from(Entity::PLACEHOLDER)),
                draw_function: draw_custom,
                distance: f32::INFINITY,
                pipeline: depth_pipeline,
                batch_range: 0..1,
                extra_index: PhaseItemExtraIndex::None,
                indexed: false,
            });
        }
    }
}

// TODO: pipeline trait
// TODO: support extentions /w ComputePipelineDescriptor builder
#[derive(Resource)]
pub struct CloudPipeline<R: PlanarSync> {
    shader: Handle<Shader>,
    pub gaussian_cloud_layout: BindGroupLayout,
    pub gaussian_cloud_layout_desc: BindGroupLayoutDescriptor,
    pub gaussian_uniform_layout: BindGroupLayout,
    pub gaussian_uniform_layout_desc: BindGroupLayoutDescriptor,
    pub view_layout: BindGroupLayout,
    pub view_layout_desc: BindGroupLayoutDescriptor,
    pub compute_view_layout: BindGroupLayout,
    pub compute_view_layout_desc: BindGroupLayoutDescriptor,
    pub sorted_layout: BindGroupLayout,
    pub sorted_layout_desc: BindGroupLayoutDescriptor,
    phantom: std::marker::PhantomData<R>,
}

fn buffer_layout(
    buffer_binding_type: BufferBindingType,
    has_dynamic_offset: bool,
    min_binding_size: Option<NonZero<u64>>,
) -> BindGroupLayoutEntryBuilder {
    match buffer_binding_type {
        BufferBindingType::Uniform => {
            binding_types::uniform_buffer_sized(has_dynamic_offset, min_binding_size)
        }
        BufferBindingType::Storage { read_only } => {
            if read_only {
                binding_types::storage_buffer_read_only_sized(has_dynamic_offset, min_binding_size)
            } else {
                binding_types::storage_buffer_sized(has_dynamic_offset, min_binding_size)
            }
        }
    }
}

pub(crate) fn storage_layout_descriptor<P: ReflectInterleaved>(
    label: impl Into<Cow<'static, str>>,
    read_only: bool,
) -> BindGroupLayoutDescriptor {
    let entries = P::min_binding_sizes()
        .iter()
        .enumerate()
        .map(|(idx, size)| BindGroupLayoutEntry {
            binding: idx as u32,
            visibility: ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: BufferSize::new(*size as u64),
            },
            count: None,
        })
        .collect::<Vec<_>>();

    BindGroupLayoutDescriptor::new(label, &entries)
}

impl<R: PlanarSync> FromWorld for CloudPipeline<R>
where
    R::GpuPlanarType: GpuPlanarStorage,
    <R::GpuPlanarType as GpuPlanar>::PackedType: ReflectInterleaved,
{
    fn from_world(render_world: &mut World) -> Self {
        let render_device = render_world.resource::<RenderDevice>();

        let visibility_ranges_buffer_binding_type = render_device
            .get_supported_read_only_binding_type(VISIBILITY_RANGES_STORAGE_BUFFER_COUNT);

        let visibility_ranges_entry = buffer_layout(
            visibility_ranges_buffer_binding_type,
            false,
            Some(Vec4::min_size()),
        )
        .build(14, ShaderStages::VERTEX);

        let view_layout_entries = vec![
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX_FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: Some(ViewUniform::min_size()),
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::VERTEX_FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(GlobalsUniform::min_size()),
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::VERTEX_FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: Some(PreviousViewData::min_size()),
                },
                count: None,
            },
            visibility_ranges_entry,
        ];

        let compute_view_layout_entries = vec![
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: Some(ViewUniform::min_size()),
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(GlobalsUniform::min_size()),
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: Some(PreviousViewData::min_size()),
                },
                count: None,
            },
            visibility_ranges_entry,
        ];

        let view_layout_desc =
            BindGroupLayoutDescriptor::new("gaussian_view_layout", &view_layout_entries);
        let view_layout = render_device
            .create_bind_group_layout(Some("gaussian_view_layout"), &view_layout_entries);

        let compute_view_layout_desc = BindGroupLayoutDescriptor::new(
            "gaussian_compute_view_layout",
            &compute_view_layout_entries,
        );
        let compute_view_layout = render_device.create_bind_group_layout(
            Some("gaussian_compute_view_layout"),
            &compute_view_layout_entries,
        );

        let gaussian_uniform_layout_entries = [BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: Some(CloudUniform::min_size()),
            },
            count: None,
        }];
        let gaussian_uniform_layout_desc = BindGroupLayoutDescriptor::new(
            "gaussian_uniform_layout",
            &gaussian_uniform_layout_entries,
        );
        let gaussian_uniform_layout = render_device.create_bind_group_layout(
            Some("gaussian_uniform_layout"),
            &gaussian_uniform_layout_entries,
        );

        #[cfg(not(feature = "morph_particles"))]
        let read_only = true;
        #[cfg(feature = "morph_particles")]
        let read_only = false;

        let gaussian_cloud_layout = R::GpuPlanarType::bind_group_layout(render_device, read_only);
        let gaussian_cloud_layout_desc = storage_layout_descriptor::<
            <R::GpuPlanarType as GpuPlanar>::PackedType,
        >("gaussian_cloud_layout", read_only);

        #[cfg(feature = "buffer_storage")]
        let sorted_layout_entries = [BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX_FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: true,
                min_binding_size: BufferSize::new(std::mem::size_of::<SortEntry>() as u64),
            },
            count: None,
        }];
        #[cfg(feature = "buffer_storage")]
        let sorted_layout_desc =
            BindGroupLayoutDescriptor::new("sorted_layout", &sorted_layout_entries);
        #[cfg(feature = "buffer_storage")]
        let sorted_layout =
            render_device.create_bind_group_layout(Some("sorted_layout"), &sorted_layout_entries);
        #[cfg(all(feature = "buffer_texture", not(feature = "buffer_storage")))]
        let sorted_layout = texture::get_sorted_bind_group_layout(render_device);
        #[cfg(all(feature = "buffer_texture", not(feature = "buffer_storage")))]
        let sorted_layout_desc = BindGroupLayoutDescriptor::new(
            "texture_sorted_layout",
            &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE,
                ty: BindingType::Texture {
                    view_dimension: TextureViewDimension::D2,
                    sample_type: TextureSampleType::Uint,
                    multisampled: false,
                },
                count: None,
            }],
        );

        debug!("created cloud pipeline");

        Self {
            gaussian_cloud_layout,
            gaussian_cloud_layout_desc,
            gaussian_uniform_layout,
            gaussian_uniform_layout_desc,
            view_layout,
            view_layout_desc,
            compute_view_layout,
            compute_view_layout_desc,
            shader: GAUSSIAN_SHADER_HANDLE,
            sorted_layout,
            sorted_layout_desc,
            phantom: std::marker::PhantomData,
        }
    }
}

// TODO: allow setting shader defines via API
// TODO: separate shader defines for each pipeline
#[derive(Clone, Copy, Debug)]
pub struct ShaderDefines {
    pub radix_bits_per_digit: u32,
    pub radix_digit_places: u32,
    pub radix_key_shift: u32,
    pub radix_base: u32,
    pub entries_per_invocation_a: u32,
    pub entries_per_invocation_c: u32,
    pub workgroup_invocations_a: u32,
    pub workgroup_invocations_c: u32,
    pub workgroup_entries_a: u32,
    pub workgroup_entries_c: u32,
    pub sorting_buffer_size: u32,

    pub temporal_sort_window_size: u32,
}

impl ShaderDefines {
    pub fn for_radix_depth_bits(radix_sort_depth_bits: RadixSortDepthBits) -> Self {
        let radix_bits_per_digit = 8;
        let radix_digit_places = radix_sort_depth_bits.bits() / radix_bits_per_digit;
        let radix_key_shift = 32 - radix_sort_depth_bits.bits();
        let radix_base = 1 << radix_bits_per_digit;
        let entries_per_invocation_a = 4;
        let entries_per_invocation_c = 4;
        let workgroup_invocations_a = radix_base * radix_digit_places;
        let workgroup_invocations_c = radix_base;
        let workgroup_entries_a = workgroup_invocations_a * entries_per_invocation_a;
        let workgroup_entries_c = workgroup_invocations_c * entries_per_invocation_c;
        let sorting_buffer_size =
            radix_base * radix_digit_places * std::mem::size_of::<u32>() as u32
                + (5 + radix_base) * std::mem::size_of::<u32>() as u32;

        Self {
            radix_bits_per_digit,
            radix_digit_places,
            radix_key_shift,
            radix_base,
            entries_per_invocation_a,
            entries_per_invocation_c,
            workgroup_invocations_a,
            workgroup_invocations_c,
            workgroup_entries_a,
            workgroup_entries_c,
            sorting_buffer_size,

            temporal_sort_window_size: 16,
        }
    }

    pub fn max_tile_count(&self, count: usize) -> u32 {
        (count as u32).div_ceil(self.workgroup_entries_c)
    }

    pub fn sorting_status_counters_buffer_size(&self, count: usize) -> usize {
        self.radix_base as usize * self.max_tile_count(count) as usize * std::mem::size_of::<u32>()
    }

    pub fn radix_initial_parity(&self) -> usize {
        (self.radix_digit_places % 2) as usize
    }
}

impl Default for ShaderDefines {
    fn default() -> Self {
        Self::for_radix_depth_bits(RadixSortDepthBits::default())
    }
}

pub fn shader_defs(key: CloudPipelineKey) -> Vec<ShaderDefVal> {
    shader_defs_with_defines(key, ShaderDefines::default())
}

pub fn shader_defs_with_defines(
    key: CloudPipelineKey,
    defines: ShaderDefines,
) -> Vec<ShaderDefVal> {
    let mut shader_defs = vec![
        ShaderDefVal::UInt("SH_COEFF_COUNT".into(), SH_COEFF_COUNT as u32),
        ShaderDefVal::UInt("SH_4D_COEFF_COUNT".into(), SH_4D_COEFF_COUNT as u32),
        ShaderDefVal::UInt("SH_DEGREE".into(), SH_DEGREE as u32),
        ShaderDefVal::UInt("SH_DEGREE_TIME".into(), SH_4D_DEGREE_TIME as u32),
        ShaderDefVal::UInt("HALF_SH_COEFF_COUNT".into(), HALF_SH_COEFF_COUNT as u32),
        ShaderDefVal::UInt("SH_VEC4_PLANES".into(), SH_VEC4_PLANES as u32),
        ShaderDefVal::UInt("RADIX_BASE".into(), defines.radix_base),
        ShaderDefVal::UInt("RADIX_BITS_PER_DIGIT".into(), defines.radix_bits_per_digit),
        ShaderDefVal::UInt("RADIX_DIGIT_PLACES".into(), defines.radix_digit_places),
        ShaderDefVal::UInt("RADIX_KEY_SHIFT".into(), defines.radix_key_shift),
        ShaderDefVal::UInt(
            "ENTRIES_PER_INVOCATION_A".into(),
            defines.entries_per_invocation_a,
        ),
        ShaderDefVal::UInt(
            "ENTRIES_PER_INVOCATION_C".into(),
            defines.entries_per_invocation_c,
        ),
        ShaderDefVal::UInt(
            "WORKGROUP_INVOCATIONS_A".into(),
            defines.workgroup_invocations_a,
        ),
        ShaderDefVal::UInt(
            "WORKGROUP_INVOCATIONS_C".into(),
            defines.workgroup_invocations_c,
        ),
        ShaderDefVal::UInt("WORKGROUP_ENTRIES_C".into(), defines.workgroup_entries_c),
        ShaderDefVal::UInt(
            "TEMPORAL_SORT_WINDOW_SIZE".into(),
            defines.temporal_sort_window_size,
        ),
    ];

    if key.aabb {
        shader_defs.push("USE_AABB".into());
    }

    if !key.aabb {
        shader_defs.push("USE_OBB".into());
    }

    if key.binary_gaussian_op {
        shader_defs.push("BINARY_GAUSSIAN_OP".into());
    }

    if key.opacity_adaptive_radius {
        shader_defs.push("OPACITY_ADAPTIVE_RADIUS".into());
    }

    if key.visualize_bounding_box {
        shader_defs.push("VISUALIZE_BOUNDING_BOX".into());
    }

    #[cfg(feature = "morph_particles")]
    shader_defs.push("READ_WRITE_POINTS".into());

    #[cfg(feature = "packed")]
    shader_defs.push("PACKED".into());

    #[cfg(feature = "buffer_storage")]
    shader_defs.push("BUFFER_STORAGE".into());

    #[cfg(all(feature = "buffer_texture", not(feature = "buffer_storage")))]
    shader_defs.push("BUFFER_TEXTURE".into());

    // #[cfg(feature = "f16")]
    // shader_defs.push("F16".into());

    shader_defs.push("F32".into());

    #[cfg(feature = "packed")]
    shader_defs.push("PACKED_F32".into());

    // #[cfg(all(feature = "f16", feature = "buffer_storage"))]
    // shader_defs.push("PLANAR_F16".into());

    #[cfg(feature = "buffer_storage")]
    shader_defs.push("PLANAR_F32".into());

    // #[cfg(all(feature = "f16", feature = "buffer_texture"))]
    // shader_defs.push("PLANAR_TEXTURE_F16".into());

    #[cfg(all(feature = "buffer_texture", not(feature = "buffer_storage")))]
    shader_defs.push("PLANAR_TEXTURE_F32".into());

    #[cfg(feature = "precompute_covariance_3d")]
    shader_defs.push("PRECOMPUTE_COVARIANCE_3D".into());

    #[cfg(feature = "webgl2")]
    shader_defs.push("WEBGL2".into());

    match key.gaussian_mode {
        GaussianMode::Gaussian2d => shader_defs.push("GAUSSIAN_2D".into()),
        GaussianMode::Gaussian3d => shader_defs.push("GAUSSIAN_3D".into()),
        GaussianMode::Gaussian4d => shader_defs.push("GAUSSIAN_4D".into()),
    }

    match key.gaussian_mode {
        GaussianMode::Gaussian2d | GaussianMode::Gaussian3d => {
            shader_defs.push("GAUSSIAN_3D_STRUCTURE".into());
        }
        _ => {}
    }

    match key.rasterize_mode {
        RasterizeMode::Classification => shader_defs.push("RASTERIZE_CLASSIFICATION".into()),
        RasterizeMode::Color => shader_defs.push("RASTERIZE_COLOR".into()),
        RasterizeMode::Depth => shader_defs.push("RASTERIZE_DEPTH".into()),
        RasterizeMode::Normal => shader_defs.push("RASTERIZE_NORMAL".into()),
        RasterizeMode::OpticalFlow => shader_defs.push("RASTERIZE_OPTICAL_FLOW".into()),
        RasterizeMode::Position => shader_defs.push("RASTERIZE_POSITION".into()),
        RasterizeMode::Velocity => shader_defs.push("RASTERIZE_VELOCITY".into()),
    }

    match key.draw_mode {
        DrawMode::All => {}
        DrawMode::Selected => shader_defs.push("DRAW_SELECTED".into()),
        DrawMode::HighlightSelected => shader_defs.push("HIGHLIGHT_SELECTED".into()),
    }

    // renzora patch(depth-write): see `CloudPipelineKey::depth_write_pass`.
    if key.depth_write_pass {
        shader_defs.push("DEPTH_WRITE_PASS".into());
    }

    shader_defs
}

#[derive(PartialEq, Eq, Hash, Clone, Copy, Default)]
pub struct CloudPipelineKey {
    pub aabb: bool,
    pub binary_gaussian_op: bool,
    pub visualize_bounding_box: bool,
    pub opacity_adaptive_radius: bool,
    pub draw_mode: DrawMode,
    pub gaussian_mode: GaussianMode,
    pub rasterize_mode: RasterizeMode,
    pub sample_count: u32,
    pub hdr: bool,
    /// renzora patch(depth-write): depth-only variant — writes the solid core
    /// of each splat (fragment alpha above a threshold) into the view depth
    /// buffer with color writes masked off. Without it splats leave depth at
    /// the far plane, so depth-reading post passes (volumetric fog, DoF)
    /// treat them as sky and composite over them at full strength.
    pub depth_write_pass: bool,
}

impl<R: PlanarSync> SpecializedRenderPipeline for CloudPipeline<R> {
    type Key = CloudPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let shader_defs = shader_defs(key);

        let format = if key.hdr {
            TextureFormat::Rgba16Float
        } else {
            TextureFormat::Rgba8UnormSrgb
        };

        debug!("specializing cloud pipeline");

        RenderPipelineDescriptor {
            label: Some(if key.depth_write_pass {
                "gaussian cloud depth-write pipeline".into()
            } else {
                "gaussian cloud render pipeline".into()
            }),
            layout: vec![
                self.view_layout_desc.clone(),
                self.gaussian_uniform_layout_desc.clone(),
                self.gaussian_cloud_layout_desc.clone(),
                self.sorted_layout_desc.clone(),
            ],
            immediate_size: 0,
            vertex: VertexState {
                shader: self.shader.clone(),
                shader_defs: shader_defs.clone(),
                entry_point: Some("vs_points".into()),
                buffers: vec![],
            },
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                shader_defs,
                entry_point: Some("fs_main".into()),
                targets: vec![Some(ColorTargetState {
                    format,
                    // renzora patch(depth-write): the depth-only variant masks
                    // color off entirely — it exists purely to populate the
                    // depth buffer after the blended color pass has drawn.
                    blend: if key.depth_write_pass {
                        None
                    } else {
                        Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING)
                    },
                    write_mask: if key.depth_write_pass {
                        ColorWrites::empty()
                    } else {
                        ColorWrites::ALL
                    },
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                unclipped_depth: false,
                cull_mode: None,
                conservative: false,
                polygon_mode: PolygonMode::Fill,
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                // renzora patch(depth-write): only the depth-only variant
                // writes depth; the blended color pass never does.
                depth_write_enabled: Some(key.depth_write_pass),
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: StencilState {
                    front: StencilFaceState::IGNORE,
                    back: StencilFaceState::IGNORE,
                    read_mask: 0,
                    write_mask: 0,
                },
                bias: DepthBiasState {
                    constant: 0,
                    slope_scale: 0.0,
                    clamp: 0.0,
                },
            }),
            multisample: MultisampleState {
                count: key.sample_count,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            zero_initialize_workgroup_memory: true,
        }
    }
}

#[allow(type_alias_bounds)]
type DrawGaussians<R: bevy_interleave::prelude::PlanarSync> = (
    SetItemPipeline,
    // SetViewBindGroup<0>,
    SetPreviousViewBindGroup<0>,
    SetGaussianUniformBindGroup<1>,
    DrawGaussianInstanced<R>,
);

#[allow(dead_code)]
#[derive(Component, ShaderType, Clone, Copy)]
pub struct CloudUniform {
    pub transform: Mat4,
    pub global_opacity: f32,
    pub global_scale: f32,
    pub count: u32,
    pub count_root_ceil: u32,
    pub time: f32,
    pub time_start: f32,
    pub time_stop: f32,
    pub num_classes: u32,
    pub color_space: u32,
    pub min: Vec4,
    pub max: Vec4,
}

#[allow(clippy::type_complexity)]
pub fn extract_gaussians<R: PlanarSync>(
    mut commands: Commands,
    mut prev_commands_len: Local<usize>,
    asset_server: Res<AssetServer>,
    gaussian_cloud_res: Res<RenderAssets<R::GpuPlanarType>>,
    gaussians_query: Extract<
        Query<(
            RenderEntity,
            &ViewVisibility,
            &R::PlanarTypeHandle,
            &Aabb,
            &SortedEntriesHandle,
            &CloudSettings,
            &GlobalTransform,
        )>,
    >,
) {
    let mut commands_list = Vec::with_capacity(*prev_commands_len);
    // let visible_gaussians = gaussians_query.iter().filter(|(_, vis, ..)| vis.is_visible());

    for (entity, visibility, cloud_handle, aabb, sorted_entries, settings, transform) in
        gaussians_query.iter()
    {
        debug!("extracting gaussian cloud entity: {:?}", entity);

        if !visibility.get() {
            debug!("gaussian cloud not visible");
            continue;
        }

        if let Some(load_state) = asset_server.get_load_state(cloud_handle.handle())
            && load_state.is_loading()
        {
            debug!("gaussian cloud asset loading");
            continue;
        }

        if gaussian_cloud_res.get(cloud_handle.handle()).is_none() {
            debug!("gaussian cloud asset not found");
            continue;
        }

        let cloud = gaussian_cloud_res.get(cloud_handle.handle()).unwrap();

        let settings_uniform = CloudUniform {
            transform: transform.to_matrix(),
            global_opacity: settings.global_opacity,
            global_scale: settings.global_scale,
            count: cloud.len() as u32,
            count_root_ceil: (cloud.len() as f32).sqrt().ceil() as u32,
            time: settings.time,
            time_start: settings.time_start,
            time_stop: settings.time_stop,
            num_classes: settings.num_classes as u32,
            color_space: match settings.color_space {
                GaussianColorSpace::SrgbRec709Display => 0,
                GaussianColorSpace::LinRec709Display => 1,
            },
            min: aabb.min().extend(1.0),
            max: aabb.max().extend(1.0),
        };

        commands_list.push((
            entity,
            GpuCloudBundle::<R> {
                aabb: *aabb,
                settings: settings.clone(),
                settings_uniform,
                sorted_entries: sorted_entries.clone(),
                cloud_handle: cloud_handle.clone(),
                transform: *transform,
            },
        ));
    }
    *prev_commands_len = commands_list.len();
    commands.insert_batch(commands_list);
}

#[derive(Resource, Default)]
pub struct GaussianUniformBindGroups {
    pub base_bind_group: Option<BindGroup>,
}

#[derive(Component)]
pub struct SortBindGroup {
    pub sorted_bind_group: BindGroup,
}

#[allow(clippy::too_many_arguments)]
fn queue_gaussian_bind_group<R: PlanarSync>(
    mut commands: Commands,
    mut groups: ResMut<GaussianUniformBindGroups>,
    gaussian_cloud_pipeline: Res<CloudPipeline<R>>,
    render_device: Res<RenderDevice>,
    gaussian_uniforms: Res<ComponentUniforms<CloudUniform>>,
    asset_server: Res<AssetServer>,
    gaussian_cloud_res: Res<RenderAssets<R::GpuPlanarType>>,
    sorted_entries_res: Res<RenderAssets<GpuSortedEntry>>,
    gaussian_clouds: Query<GpuCloudBindGroupQuery<R>>,
    #[cfg(all(feature = "buffer_texture", not(feature = "buffer_storage")))] gpu_images: Res<
        RenderAssets<bevy::render::texture::GpuImage>,
    >,
) {
    let Some(resource) = gaussian_uniforms.binding() else {
        return;
    };

    let pipeline_changed = gaussian_cloud_pipeline.is_changed();
    if gaussian_uniforms.is_changed() || pipeline_changed || groups.base_bind_group.is_none() {
        groups.base_bind_group = Some(render_device.create_bind_group(
            "gaussian_uniform_bind_group",
            &gaussian_cloud_pipeline.gaussian_uniform_layout,
            &[BindGroupEntry {
                binding: 0,
                resource,
            }],
        ));
    }

    let gaussian_assets_changed = gaussian_cloud_res.is_changed();
    let sorted_assets_changed = sorted_entries_res.is_changed();
    #[cfg(all(feature = "buffer_texture", not(feature = "buffer_storage")))]
    let mut should_refresh_for_assets =
        pipeline_changed || gaussian_assets_changed || sorted_assets_changed;
    #[cfg(not(all(feature = "buffer_texture", not(feature = "buffer_storage"))))]
    let should_refresh_for_assets =
        pipeline_changed || gaussian_assets_changed || sorted_assets_changed;

    #[cfg(all(feature = "buffer_texture", not(feature = "buffer_storage")))]
    {
        let textures_changed = gpu_images.is_changed();
        should_refresh_for_assets |= textures_changed;
    }

    for query in gaussian_clouds.iter() {
        let (entity, cloud_handle, sorted_entries_handle, existing_bind_group) = query;

        if !should_refresh_for_assets && existing_bind_group.is_some() {
            continue;
        }

        if let Some(load_state) = asset_server.get_load_state(cloud_handle.handle())
            && load_state.is_loading()
        {
            debug!("queue gaussian bind group: cloud asset loading");
            continue;
        }

        if gaussian_cloud_res.get(cloud_handle.handle()).is_none() {
            debug!("queue gaussian bind group: cloud asset not found");
            continue;
        }

        if let Some(load_state) = asset_server.get_load_state(&sorted_entries_handle.0)
            && load_state.is_loading()
        {
            debug!("queue gaussian bind group: sorted entries asset loading");
            continue;
        }

        if sorted_entries_res.get(&sorted_entries_handle.0).is_none() {
            debug!("queue gaussian bind group: sorted entries asset not found");
            continue;
        }

        #[cfg(feature = "buffer_storage")]
        let cloud = gaussian_cloud_res.get(cloud_handle.handle()).unwrap();

        let sorted_entries = sorted_entries_res.get(&sorted_entries_handle.0).unwrap();

        #[cfg(feature = "buffer_storage")]
        let sorted_bind_group = render_device.create_bind_group(
            "render_sorted_bind_group",
            &gaussian_cloud_pipeline.sorted_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &sorted_entries.sorted_entry_buffer,
                    offset: 0,
                    size: BufferSize::new((cloud.len() * std::mem::size_of::<SortEntry>()) as u64),
                }),
            }],
        );
        #[cfg(all(feature = "buffer_texture", not(feature = "buffer_storage")))]
        let sorted_bind_group = render_device.create_bind_group(
            Some("render_sorted_bind_group"),
            &gaussian_cloud_pipeline.sorted_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(
                    &gpu_images
                        .get(&sorted_entries.texture)
                        .unwrap()
                        .texture_view,
                ),
            }],
        );

        debug!("inserting sorted bind group");

        commands
            .entity(entity)
            .insert(SortBindGroup { sorted_bind_group });
    }
}

#[derive(Component)]
pub struct GaussianViewBindGroup {
    pub value: BindGroup,
}

#[derive(Component)]
pub struct GaussianComputeViewBindGroup {
    pub value: BindGroup,
}

// TODO: move to gaussian camera module
// TODO: remove cloud pipeline dependency by separating view layout

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn queue_gaussian_view_bind_groups<R: PlanarSync>(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    gaussian_cloud_pipeline: Res<CloudPipeline<R>>,
    view_uniforms: Res<ViewUniforms>,
    previous_view_uniforms: Res<PreviousViewUniforms>,
    views: Query<
        (
            Entity,
            &ExtractedView,
            Option<&PreviousViewData>,
            Option<&GaussianViewBindGroup>,
        ),
        With<GaussianCamera>,
    >,
    visibility_ranges: Res<RenderVisibilityRanges>,
    globals_buffer: Res<GlobalsBuffer>,
) {
    let Some(view_binding) = view_uniforms.uniforms.binding() else {
        return;
    };
    let Some(previous_view_binding) = previous_view_uniforms.uniforms.binding() else {
        return;
    };
    let Some(globals) = globals_buffer.buffer.binding() else {
        return;
    };
    let Some(visibility_ranges_buffer) = visibility_ranges.buffer().buffer() else {
        return;
    };

    let resources_changed = gaussian_cloud_pipeline.is_changed()
        || view_uniforms.is_changed()
        || previous_view_uniforms.is_changed()
        || globals_buffer.is_changed()
        || visibility_ranges.is_changed();

    for (entity, _extracted_view, _maybe_previous_view, existing_bind_group) in &views {
        if !resources_changed && existing_bind_group.is_some() {
            continue;
        }

        let layout = &gaussian_cloud_pipeline.view_layout;

        let entries = vec![
            BindGroupEntry {
                binding: 0,
                resource: view_binding.clone(),
            },
            BindGroupEntry {
                binding: 1,
                resource: globals.clone(),
            },
            BindGroupEntry {
                binding: 2,
                resource: previous_view_binding.clone(),
            },
            BindGroupEntry {
                binding: 14,
                resource: visibility_ranges_buffer.as_entire_binding(),
            },
        ];

        let view_bind_group =
            render_device.create_bind_group("gaussian_view_bind_group", layout, &entries);

        debug!("inserting gaussian view bind group");

        commands.entity(entity).insert(GaussianViewBindGroup {
            value: view_bind_group,
        });
    }
}

// Prepare the compute view bind group using the compute_view_layout (for compute pipelines)
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn queue_gaussian_compute_view_bind_groups<R: PlanarSync>(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    gaussian_cloud_pipeline: Res<CloudPipeline<R>>,
    view_uniforms: Res<ViewUniforms>,
    previous_view_uniforms: Res<PreviousViewUniforms>,
    views: Query<
        (
            Entity,
            &ExtractedView,
            Option<&PreviousViewData>,
            Option<&GaussianComputeViewBindGroup>,
        ),
        With<GaussianCamera>,
    >,
    visibility_ranges: Res<RenderVisibilityRanges>,
    globals_buffer: Res<GlobalsBuffer>,
) where
    R::GpuPlanarType: GpuPlanarStorage,
{
    let Some(view_binding) = view_uniforms.uniforms.binding() else {
        return;
    };
    let Some(previous_view_binding) = previous_view_uniforms.uniforms.binding() else {
        return;
    };
    let Some(globals) = globals_buffer.buffer.binding() else {
        return;
    };
    let Some(visibility_ranges_buffer) = visibility_ranges.buffer().buffer() else {
        return;
    };

    let resources_changed = gaussian_cloud_pipeline.is_changed()
        || view_uniforms.is_changed()
        || previous_view_uniforms.is_changed()
        || globals_buffer.is_changed()
        || visibility_ranges.is_changed();

    for (entity, _extracted_view, _maybe_previous_view, existing_bind_group) in &views {
        if !resources_changed && existing_bind_group.is_some() {
            continue;
        }

        let layout = &gaussian_cloud_pipeline.compute_view_layout;

        let entries = vec![
            BindGroupEntry {
                binding: 0,
                resource: view_binding.clone(),
            },
            BindGroupEntry {
                binding: 1,
                resource: globals.clone(),
            },
            BindGroupEntry {
                binding: 2,
                resource: previous_view_binding.clone(),
            },
            BindGroupEntry {
                binding: 14,
                resource: visibility_ranges_buffer.as_entire_binding(),
            },
        ];

        let view_bind_group =
            render_device.create_bind_group("gaussian_compute_view_bind_group", layout, &entries);

        commands
            .entity(entity)
            .insert(GaussianComputeViewBindGroup {
                value: view_bind_group,
            });
    }
}

pub struct SetViewBindGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetViewBindGroup<I> {
    type Param = ();
    type ViewQuery = (Read<GaussianViewBindGroup>, Read<ViewUniformOffset>);
    type ItemQuery = ();

    #[inline]
    fn render<'w>(
        _: &P,
        (gaussian_view_bind_group, view_uniform): ROQueryItem<'w, 'w, Self::ViewQuery>,
        _entity: Option<()>,
        _: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        pass.set_bind_group(I, &gaussian_view_bind_group.value, &[view_uniform.offset]);

        debug!("set view bind group");

        RenderCommandResult::Success
    }
}

pub struct SetPreviousViewBindGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetPreviousViewBindGroup<I> {
    type Param = SRes<PrepassViewBindGroup>;
    type ViewQuery = (
        Read<ViewUniformOffset>,
        Option<Has<MotionVectorPrepass>>,
        Option<Read<PreviousViewUniformOffset>>,
    );
    type ItemQuery = ();

    #[inline]
    fn render<'w>(
        _: &P,
        (view_uniform_offset, has_motion_vector_prepass, previous_view_uniform_offset): ROQueryItem<
            'w,
            'w,
            Self::ViewQuery,
        >,
        _entity: Option<()>,
        prepass_view_bind_group: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let prepass_view_bind_group = prepass_view_bind_group.into_inner();
        match previous_view_uniform_offset {
            Some(previous_view_uniform_offset) if has_motion_vector_prepass.unwrap_or_default() => {
                pass.set_bind_group(
                    I,
                    prepass_view_bind_group.motion_vectors.as_ref().unwrap(),
                    &[
                        view_uniform_offset.offset,
                        previous_view_uniform_offset.offset,
                    ],
                );
            }
            _ => pass.set_bind_group(
                I,
                prepass_view_bind_group.motion_vectors.as_ref().unwrap(),
                &[view_uniform_offset.offset, 0],
            ),
        }

        debug!("set previous view bind group");

        RenderCommandResult::Success
    }
}

pub struct SetGaussianUniformBindGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetGaussianUniformBindGroup<I> {
    type Param = SRes<GaussianUniformBindGroups>;
    type ViewQuery = ();
    type ItemQuery = Read<DynamicUniformIndex<CloudUniform>>;

    #[inline]
    fn render<'w>(
        _item: &P,
        _view: (),
        gaussian_cloud_index: Option<ROQueryItem<'w, 'w, Self::ItemQuery>>,
        bind_groups: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let bind_groups = bind_groups.into_inner();
        let bind_group = bind_groups
            .base_bind_group
            .as_ref()
            .expect("bind group not initialized");

        let mut set_bind_group = |indices: &[u32]| pass.set_bind_group(I, bind_group, indices);

        if gaussian_cloud_index.is_none() {
            debug!("skipping gaussian uniform bind group\n");
            return RenderCommandResult::Skip;
        }

        let gaussian_cloud_index = gaussian_cloud_index.unwrap().index();
        set_bind_group(&[gaussian_cloud_index]);

        debug!("set gaussian uniform bind group");

        RenderCommandResult::Success
    }
}

pub struct DrawGaussianInstanced<R: PlanarSync> {
    phantom: std::marker::PhantomData<R>,
}

impl<R: PlanarSync> Default for DrawGaussianInstanced<R> {
    fn default() -> Self {
        Self {
            phantom: std::marker::PhantomData,
        }
    }
}

impl<P: PhaseItem, R: PlanarSync> RenderCommand<P> for DrawGaussianInstanced<R>
where
    R::GpuPlanarType: GpuPlanarStorage,
{
    type Param = SRes<RenderAssets<R::GpuPlanarType>>;
    type ViewQuery = Read<SortTrigger>;
    type ItemQuery = (
        Read<R::PlanarTypeHandle>,
        Read<PlanarStorageBindGroup<R>>,
        Read<SortBindGroup>,
    );

    #[inline]
    fn render<'w>(
        _item: &P,
        view: &'w SortTrigger,
        entity: Option<(
            &'w R::PlanarTypeHandle,
            &'w PlanarStorageBindGroup<R>,
            &'w SortBindGroup,
        )>,
        gaussian_clouds: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        debug!("render call");

        #[cfg(all(feature = "buffer_texture", not(feature = "buffer_storage")))]
        let _ = view;

        let (handle, planar_bind_groups, sort_bind_groups) =
            entity.expect("gaussian cloud entity not found");

        let gpu_gaussian_cloud = match gaussian_clouds.into_inner().get(handle.handle()) {
            Some(gpu_gaussian_cloud) => gpu_gaussian_cloud,
            None => {
                debug!("gpu cloud not found");
                return RenderCommandResult::Skip;
            }
        };

        debug!("drawing indirect");

        pass.set_bind_group(2, &planar_bind_groups.bind_group, &[]);

        #[cfg(feature = "buffer_storage")]
        {
            // TODO: align dynamic offset to `min_storage_buffer_offset_alignment`
            pass.set_bind_group(
                3,
                &sort_bind_groups.sorted_bind_group,
                &[view.camera_index as u32
                    * std::mem::size_of::<SortEntry>() as u32
                    * gpu_gaussian_cloud.len() as u32],
            );
        }

        #[cfg(all(feature = "buffer_texture", not(feature = "buffer_storage")))]
        {
            pass.set_bind_group(3, &sort_bind_groups.sorted_bind_group, &[]);
        }

        #[cfg(feature = "webgl2")]
        pass.draw(0..4, 0..gpu_gaussian_cloud.len() as u32);

        #[cfg(not(feature = "webgl2"))]
        pass.draw_indirect(gpu_gaussian_cloud.draw_indirect_buffer(), 0);

        RenderCommandResult::Success
    }
}
