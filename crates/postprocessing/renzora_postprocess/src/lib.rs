//! Generic post-process pipeline that supports multiple simultaneous effects.
//!
//! Bevy's built-in `FullscreenMaterialPlugin` uses a non-generic resource,
//! so multiple instances overwrite each other. This crate provides a typed
//! `PostProcessPipeline<T>` resource so each effect gets its own pipeline.

use core::marker::PhantomData;

use bevy::core_pipeline::{
    core_3d::graph::Core3d,
    FullscreenShader,
};
use bevy::prelude::*;
use bevy::render::{
    extract_component::{
        ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
        UniformComponentPlugin,
    },
    render_graph::{
        InternedRenderLabel, InternedRenderSubGraph, NodeRunError, RenderGraphContext, RenderGraphError,
        RenderGraphExt, RenderLabel, RenderSubGraph, ViewNode, ViewNodeRunner,
    },
    render_resource::{
        binding_types::{sampler, texture_2d, uniform_buffer},
        encase::internal::WriteInto,
        BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
        CachedRenderPipelineId, ColorTargetState, ColorWrites, FragmentState, Operations,
        PipelineCache, RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor,
        Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, ShaderType, TextureFormat,
        TextureSampleType,
    },
    renderer::{RenderContext, RenderDevice},
    view::ViewTarget,
    RenderApp, RenderStartup,
};
use bevy::shader::ShaderRef;
use bevy::image::BevyDefault;
use bevy::ecs::query::QueryItem;
use bevy::utils::default;

/// Trait for post-process effects. Same interface as Bevy's `FullscreenMaterial`
/// but works with multiple simultaneous effects.
pub trait PostProcessEffect:
    Component + ExtractComponent + Clone + Copy + ShaderType + WriteInto + Default + 'static
{
    fn fragment_shader() -> ShaderRef;
    fn node_edges() -> Vec<InternedRenderLabel>;

    fn sub_graph() -> Option<InternedRenderSubGraph> {
        Some(Core3d.intern())
    }

    fn node_label() -> impl RenderLabel {
        PostProcessLabel(core::any::type_name::<Self>())
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
struct PostProcessLabel(&'static str);

impl RenderLabel for PostProcessLabel
where
    Self: 'static + Send + Sync + Clone + Eq + core::fmt::Debug + core::hash::Hash,
{
    fn dyn_clone(&self) -> Box<dyn RenderLabel> {
        Box::new(self.clone())
    }
}

/// Typed pipeline resource — each `T` gets its own instance.
#[derive(Resource)]
struct PostProcessPipeline<T: PostProcessEffect> {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
    pipeline_id_hdr: CachedRenderPipelineId,
    _marker: PhantomData<T>,
}

/// Plugin that sets up a post-process effect with its own pipeline.
#[derive(Default)]
pub struct PostProcessPlugin<T: PostProcessEffect> {
    _marker: PhantomData<T>,
}

impl<T: PostProcessEffect> Plugin for PostProcessPlugin<T> {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractComponentPlugin::<T>::default(),
            UniformComponentPlugin::<T>::default(),
        ));

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.add_systems(RenderStartup, init_pipeline::<T>);

        if let Some(sub_graph) = T::sub_graph() {
            render_app
                .add_render_graph_node::<ViewNodeRunner<PostProcessNode<T>>>(
                    sub_graph,
                    T::node_label(),
                );

            if let Some(mut render_graph) =
                render_app.world_mut().get_resource_mut::<bevy::render::render_graph::RenderGraph>()
            {
              if let Some(graph) = render_graph.get_sub_graph_mut(sub_graph) {
                for window in T::node_edges().windows(2) {
                    let [a, b] = window else { break };
                    let Err(err) = graph.try_add_node_edge(*a, *b) else {
                        continue;
                    };
                    match err {
                        RenderGraphError::EdgeAlreadyExists(_) => {}
                        _ => panic!("{err:?}"),
                    }
                }
              }
            }
        }
    }
}

fn init_pipeline<T: PostProcessEffect>(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    fullscreen_shader: Res<FullscreenShader>,
    pipeline_cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "post_process_bind_group_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<T>(true),
            ),
        ),
    );
    let sampler = render_device.create_sampler(&SamplerDescriptor::default());
    let shader = match T::fragment_shader() {
        ShaderRef::Default => {
            unimplemented!("PostProcessEffect::fragment_shader() must not return ShaderRef::Default")
        }
        ShaderRef::Handle(handle) => handle,
        ShaderRef::Path(path) => asset_server.load(path),
    };
    let vertex_state = fullscreen_shader.to_vertex_state();
    let mut desc = RenderPipelineDescriptor {
        label: Some("post_process_pipeline".into()),
        layout: vec![layout.clone()],
        vertex: vertex_state,
        fragment: Some(FragmentState {
            shader,
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::bevy_default(),
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    };
    let pipeline_id = pipeline_cache.queue_render_pipeline(desc.clone());
    desc.fragment.as_mut().unwrap().targets[0]
        .as_mut()
        .unwrap()
        .format = ViewTarget::TEXTURE_FORMAT_HDR;
    let pipeline_id_hdr = pipeline_cache.queue_render_pipeline(desc);
    commands.insert_resource(PostProcessPipeline::<T> {
        layout,
        sampler,
        pipeline_id,
        pipeline_id_hdr,
        _marker: PhantomData,
    });
}

#[derive(Default)]
struct PostProcessNode<T: PostProcessEffect> {
    _marker: PhantomData<T>,
}

impl<T: PostProcessEffect> ViewNode for PostProcessNode<T> {
    type ViewQuery = (&'static ViewTarget, &'static DynamicUniformIndex<T>);

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, settings_index): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<PostProcessPipeline<T>>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline_id = if view_target.is_hdr() {
            pipeline.pipeline_id_hdr
        } else {
            pipeline.pipeline_id
        };

        let Some(render_pipeline) = pipeline_cache.get_render_pipeline(pipeline_id) else {
            return Ok(());
        };

        let data_uniforms = world.resource::<ComponentUniforms<T>>();
        let Some(settings_binding) = data_uniforms.uniforms().binding() else {
            return Ok(());
        };

        let post_process = view_target.post_process_write();

        let bind_group = render_context.render_device().create_bind_group(
            "post_process_bind_group",
            &pipeline_cache.get_bind_group_layout(&pipeline.layout),
            &BindGroupEntries::sequential((
                post_process.source,
                &pipeline.sampler,
                settings_binding.clone(),
            )),
        );

        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("post_process_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: post_process.destination,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_render_pipeline(render_pipeline);
        render_pass.set_bind_group(0, &bind_group, &[settings_index.index()]);
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}
