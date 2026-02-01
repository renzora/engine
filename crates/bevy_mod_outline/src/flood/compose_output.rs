use bevy::{
    core_pipeline::fullscreen_vertex_shader::fullscreen_shader_vertex_state,
    platform::collections::HashMap,
    prelude::*,
    render::{
        extract_component::{ComponentUniforms, DynamicUniformIndex},
        render_resource::{
            binding_types::{sampler, texture_2d, uniform_buffer},
            BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries, CachedRenderPipelineId,
            FragmentState, PipelineCache, RenderPassDescriptor, RenderPipeline,
            RenderPipelineDescriptor, Sampler, SamplerDescriptor, ShaderType, StoreOp,
        },
        renderer::{RenderContext, RenderDevice},
        texture::CachedTexture,
        view::{ExtractedView, ViewDepthTexture, ViewTarget},
    },
};
use wgpu_types::{
    BlendState, ColorTargetState, ColorWrites, CompareFunction, DepthBiasState, DepthStencilState,
    MultisampleState, PrimitiveState, SamplerBindingType, ShaderStages, StencilState,
    TextureFormat, TextureSampleType,
};

use crate::{pipeline_key::ViewPipelineKey, uniforms::ExtractedOutline};

use super::{DrawMode, OutlineViewUniform, COMPOSE_OUTPUT_SHADER_HANDLE};

#[derive(Clone, Component, ShaderType)]
pub(crate) struct ComposeOutputUniform {
    #[align(16)]
    pub volume_offset: f32,
    pub volume_colour: Vec4,
}

pub(crate) fn prepare_compose_output_uniform(
    mut commands: Commands,
    query: Query<(Entity, &ExtractedOutline)>,
) {
    for (entity, outline) in query.iter() {
        if outline.draw_mode == DrawMode::JumpFlood {
            commands.entity(entity).insert(ComposeOutputUniform {
                volume_offset: outline.instance_data.volume_offset,
                volume_colour: outline.instance_data.volume_colour,
            });
        }
    }
}

#[derive(Clone, Resource)]
pub(crate) struct ComposeOutputPipeline {
    pub(crate) layout: BindGroupLayout,
    pub(crate) sampler: Sampler,
    pub(crate) pipeline_cache: HashMap<ViewPipelineKey, CachedRenderPipelineId>,
}

impl FromWorld for ComposeOutputPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = render_device.create_bind_group_layout(
            "outline_flood_compose_output_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    uniform_buffer::<OutlineViewUniform>(true),
                    uniform_buffer::<ComposeOutputUniform>(true),
                ),
            ),
        );

        let sampler = render_device.create_sampler(&SamplerDescriptor::default());

        Self {
            layout,
            sampler,
            pipeline_cache: HashMap::new(),
        }
    }
}

impl ComposeOutputPipeline {
    pub(crate) fn get_pipeline(
        &mut self,
        pipeline_cache: &PipelineCache,
        key: ViewPipelineKey,
    ) -> CachedRenderPipelineId {
        *self.pipeline_cache.entry(key).or_insert_with(|| {
            pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
                label: Some("outline_flood_compose_output_pipeline".into()),
                layout: vec![self.layout.clone()],
                vertex: fullscreen_shader_vertex_state(),
                fragment: Some(FragmentState {
                    shader: COMPOSE_OUTPUT_SHADER_HANDLE,
                    shader_defs: vec![],
                    entry_point: "fragment".into(),
                    targets: vec![Some(ColorTargetState {
                        format: if key.hdr_format() {
                            ViewTarget::TEXTURE_FORMAT_HDR
                        } else {
                            TextureFormat::bevy_default()
                        },
                        blend: Some(BlendState::ALPHA_BLENDING),
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                primitive: PrimitiveState::default(),
                depth_stencil: Some(DepthStencilState {
                    format: TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: CompareFunction::Greater,
                    stencil: StencilState::default(),
                    bias: DepthBiasState::default(),
                }),
                multisample: MultisampleState {
                    count: key.msaa() as u32,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                push_constant_ranges: vec![],
                zero_initialize_workgroup_memory: false,
            })
        })
    }
}

#[derive(Component)]
pub struct ComposeOutputView {
    pub(crate) pipeline_id: CachedRenderPipelineId,
}

pub(crate) fn prepare_compose_output_pass(
    mut commands: Commands,
    query: Query<(Entity, &ExtractedView, &Msaa), With<OutlineViewUniform>>,
    pipeline_cache: Res<PipelineCache>,
    mut compose_output_pipeline: ResMut<ComposeOutputPipeline>,
) {
    for (entity, view, msaa) in query.iter() {
        let pipeline_id = compose_output_pipeline.get_pipeline(
            &pipeline_cache,
            ViewPipelineKey::new()
                .with_msaa(*msaa)
                .with_hdr_format(view.hdr),
        );
        commands
            .entity(entity)
            .insert(ComposeOutputView { pipeline_id });
    }
}

pub(crate) struct ComposeOutputPass<'w> {
    world: &'w World,
    pipeline: &'w ComposeOutputPipeline,
    render_pipeline: &'w RenderPipeline,
    outline_view_uniforms: &'w ComponentUniforms<OutlineViewUniform>,
    compose_output_uniforms: &'w ComponentUniforms<ComposeOutputUniform>,
    view_target: &'w ViewTarget,
    view_depth: &'w ViewDepthTexture,
}

impl<'w> ComposeOutputPass<'w> {
    pub fn new(
        world: &'w World,
        compose_output_view: &ComposeOutputView,
        view_target: &'w ViewTarget,
        view_depth: &'w ViewDepthTexture,
    ) -> Option<Self> {
        let pipeline = world.resource::<ComposeOutputPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let render_pipeline =
            pipeline_cache.get_render_pipeline(compose_output_view.pipeline_id)?;
        let outline_view_uniforms = world.resource::<ComponentUniforms<OutlineViewUniform>>();
        let compose_output_uniforms = world.resource::<ComponentUniforms<ComposeOutputUniform>>();

        Some(Self {
            world,
            pipeline,
            render_pipeline,
            outline_view_uniforms,
            compose_output_uniforms,
            view_target,
            view_depth,
        })
    }

    pub fn execute(
        &self,
        render_context: &mut RenderContext<'_>,
        view_entity: Entity,
        render_entity: Entity,
        input: &CachedTexture,
        bounds: &URect,
    ) {
        let view_dynamic_index = self
            .world
            .entity(view_entity)
            .get::<DynamicUniformIndex<OutlineViewUniform>>()
            .unwrap()
            .index();
        let dynamic_index = self
            .world
            .entity(render_entity)
            .get::<DynamicUniformIndex<ComposeOutputUniform>>()
            .unwrap()
            .index();

        let bind_group = render_context.render_device().create_bind_group(
            "outline_flood_compose_output_bind_group",
            &self.pipeline.layout,
            &BindGroupEntries::sequential((
                &input.default_view,
                &self.pipeline.sampler,
                self.outline_view_uniforms.binding().unwrap(),
                self.compose_output_uniforms.binding().unwrap(),
            )),
        );

        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("outline_flood_compose_output_pass"),
            color_attachments: &[Some(self.view_target.get_color_attachment())],
            depth_stencil_attachment: Some(self.view_depth.get_attachment(StoreOp::Store)),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_scissor_rect(bounds.min.x, bounds.min.y, bounds.width(), bounds.height());
        render_pass.set_render_pipeline(self.render_pipeline);
        render_pass.set_bind_group(0, &bind_group, &[view_dynamic_index, dynamic_index]);
        render_pass.draw(0..3, 0..1);
    }
}
