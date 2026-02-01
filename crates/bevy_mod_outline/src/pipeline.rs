use std::borrow::Cow;

use bevy::asset::uuid_handle;
use bevy::ecs::system::lifetimeless::SRes;
use bevy::ecs::system::SystemParamItem;
use bevy::pbr::SkinUniforms;
use bevy::prelude::*;
use bevy::render::batching::{gpu_preprocessing, GetBatchData, GetFullBatchData};
use bevy::render::mesh::allocator::MeshAllocator;
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer_sized};
use bevy::render::render_resource::{
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntries, BlendState, ColorTargetState, ColorWrites,
    CompareFunction, DepthBiasState, DepthStencilState, Face, FragmentState, FrontFace,
    GpuArrayBuffer, MultisampleState, PolygonMode, PrimitiveState, ShaderStages,
    ShaderType, StencilState, TextureFormat, VertexState,
};
use bevy::shader::ShaderDefVal;
use bevy::render::renderer::RenderDevice;
use bevy::render::settings::WgpuSettings;
use bevy::render::sync_world::MainEntity;
use bevy::render::view::ViewTarget;
use bevy::{
    pbr::MeshPipeline,
    mesh::MeshVertexBufferLayoutRef,
    render::render_resource::{
        RenderPipelineDescriptor, SpecializedMeshPipeline, SpecializedMeshPipelineError,
    },
};
use nonmax::NonMaxU32;
use wgpu_types::{Backends, PushConstantRange, SamplerBindingType, TextureSampleType};

use crate::pipeline_key::{DerivedPipelineKey, PassType};
use crate::uniforms::{DepthMode, OutlineInstanceUniform, RenderOutlineInstances};
use crate::view_uniforms::OutlineViewUniform;
use crate::ATTRIBUTE_OUTLINE_NORMAL;

pub(crate) const COMMON_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("aee41cd9-fc8f-4788-9ea4-f85bd8070c65");

pub(crate) const OUTLINE_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("910c269f-f115-47ba-b757-6ae51bf0c79f");

pub(crate) const FRAGMENT_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("1f5b5967-7cbb-4392-8f34-421587938a12");

#[derive(Resource)]
pub(crate) struct OutlinePipeline {
    mesh_pipeline: MeshPipeline,
    pub outline_view_bind_group_layout: BindGroupLayout,
    pub outline_view_bind_group_layout_descriptor: BindGroupLayoutDescriptor,
    pub outline_instance_bind_group_layout: BindGroupLayout,
    pub outline_instance_bind_group_layout_descriptor: BindGroupLayoutDescriptor,
    pub alpha_mask_bind_group_layout: BindGroupLayout,
    pub alpha_mask_bind_group_layout_descriptor: BindGroupLayoutDescriptor,
    pub instance_batch_size: Option<u32>,
}

impl FromWorld for OutlinePipeline {
    fn from_world(world: &mut World) -> Self {
        let mesh_pipeline = world.get_resource::<MeshPipeline>().unwrap().clone();
        let render_device = world.get_resource::<RenderDevice>().unwrap();
        let limits = render_device.limits();

        let outline_view_entries = BindGroupLayoutEntries::single(
            ShaderStages::VERTEX,
            uniform_buffer_sized(true, Some(OutlineViewUniform::min_size())),
        );
        let outline_view_bind_group_layout = render_device.create_bind_group_layout(
            "outline_view_bind_group_layout",
            &outline_view_entries,
        );
        let outline_view_bind_group_layout_descriptor = BindGroupLayoutDescriptor::new(
            "outline_view_bind_group_layout",
            &outline_view_entries,
        );

        let outline_instance_entries = BindGroupLayoutEntries::single(
            ShaderStages::VERTEX,
            GpuArrayBuffer::<OutlineInstanceUniform>::binding_layout(&limits),
        );
        let outline_instance_bind_group_layout = render_device.create_bind_group_layout(
            "outline_instance_bind_group_layout",
            &outline_instance_entries,
        );
        let outline_instance_bind_group_layout_descriptor = BindGroupLayoutDescriptor::new(
            "outline_instance_bind_group_layout",
            &outline_instance_entries,
        );

        let alpha_mask_entries = BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
            ),
        );
        let alpha_mask_bind_group_layout = render_device.create_bind_group_layout(
            "alpha_mask_bind_group_layout",
            &alpha_mask_entries,
        );
        let alpha_mask_bind_group_layout_descriptor = BindGroupLayoutDescriptor::new(
            "alpha_mask_bind_group_layout",
            &alpha_mask_entries,
        );

        let instance_batch_size =
            GpuArrayBuffer::<OutlineInstanceUniform>::batch_size(&limits);
        OutlinePipeline {
            mesh_pipeline,
            outline_view_bind_group_layout,
            outline_view_bind_group_layout_descriptor,
            outline_instance_bind_group_layout,
            outline_instance_bind_group_layout_descriptor,
            alpha_mask_bind_group_layout,
            alpha_mask_bind_group_layout_descriptor,
            instance_batch_size,
        }
    }
}

impl SpecializedMeshPipeline for OutlinePipeline {
    type Key = DerivedPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut targets = vec![];
        let mut vertex_defs = vec![];
        let mut fragment_defs = vec![];
        let mut buffer_attrs = vec![Mesh::ATTRIBUTE_POSITION.at_shader_location(0)];

        let bind_layouts = vec![
            self.outline_view_bind_group_layout_descriptor.clone(),
            self.mesh_pipeline.mesh_layouts.model_only.clone().into(),
            self.outline_instance_bind_group_layout_descriptor.clone(),
            self.alpha_mask_bind_group_layout_descriptor.clone(),
        ];

        if key.alpha_mask_texture() {
            let val = ShaderDefVal::from("ALPHA_MASK_TEXTURE");
            vertex_defs.push(val.clone());
            fragment_defs.push(val);

            let channel_def = ShaderDefVal::UInt(
                "ALPHA_MASK_CHANNEL".to_string(),
                key.alpha_mask_channel_int(),
            );
            fragment_defs.push(channel_def);

            buffer_attrs.push(Mesh::ATTRIBUTE_UV_0.at_shader_location(2));
        }

        if let Some(sz) = self.instance_batch_size {
            vertex_defs.push(ShaderDefVal::Int(
                "INSTANCE_BATCH_SIZE".to_string(),
                sz as i32,
            ));
        }

        let cull_mode;
        if key.depth_mode() == DepthMode::Flat {
            let val = ShaderDefVal::from("FLAT_DEPTH");
            vertex_defs.push(val.clone());
            fragment_defs.push(val);
            if key.double_sided() {
                cull_mode = None;
            } else {
                cull_mode = Some(Face::Back);
            }
        } else if key.pass_type() == PassType::Stencil {
            cull_mode = Some(Face::Back);
        } else {
            cull_mode = Some(Face::Front);
        }
        if key.vertex_offset_zero() {
            vertex_defs.push(ShaderDefVal::from("VERTEX_OFFSET_ZERO"));
        } else {
            buffer_attrs.push(
                if layout.0.contains(ATTRIBUTE_OUTLINE_NORMAL) {
                    ATTRIBUTE_OUTLINE_NORMAL
                } else {
                    Mesh::ATTRIBUTE_NORMAL
                }
                .at_shader_location(1),
            );
        }
        if key.plane_offset_zero() {
            vertex_defs.push(ShaderDefVal::from("PLANE_OFFSET_ZERO"));
        }
        match key.pass_type() {
            PassType::Stencil => {}
            PassType::Volume => {
                let val = ShaderDefVal::from("VOLUME");
                vertex_defs.push(val.clone());
                fragment_defs.push(val);
                targets.push(Some(ColorTargetState {
                    format: if key.hdr_format() {
                        ViewTarget::TEXTURE_FORMAT_HDR
                    } else {
                        TextureFormat::bevy_default()
                    },
                    blend: Some(if key.transparent() {
                        BlendState::ALPHA_BLENDING
                    } else {
                        BlendState::REPLACE
                    }),
                    write_mask: ColorWrites::ALL,
                }));
            }
            #[cfg(feature = "flood")]
            PassType::FloodInit => {
                let val = ShaderDefVal::from("FLOOD_INIT");
                vertex_defs.push(val.clone());
                fragment_defs.push(val);
                targets.push(Some(ColorTargetState {
                    format: TextureFormat::Rgba16Float,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                }));
            }
        }
        let depth_stencil = match key.pass_type() {
            PassType::Stencil | PassType::Volume => Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: CompareFunction::Greater,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            #[cfg(feature = "flood")]
            PassType::FloodInit => None,
        };
        let buffers = vec![layout.0.get_layout(&buffer_attrs)?];
        let mut push_constant_ranges = Vec::with_capacity(1);
        // Proxy for webgl feature flag in bevy
        if WgpuSettings::default().backends == Some(Backends::GL) {
            push_constant_ranges.push(PushConstantRange {
                stages: ShaderStages::VERTEX,
                range: 0..4,
            });
        }
        Ok(RenderPipelineDescriptor {
            vertex: VertexState {
                shader: OUTLINE_SHADER_HANDLE,
                entry_point: Some("vertex".into()),
                shader_defs: vertex_defs,
                buffers,
            },
            fragment: Some(FragmentState {
                shader: FRAGMENT_SHADER_HANDLE,
                shader_defs: fragment_defs,
                entry_point: Some("fragment".into()),
                targets,
            }),
            layout: bind_layouts,
            primitive: PrimitiveState {
                front_face: FrontFace::Ccw,
                cull_mode,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
                topology: key.primitive_topology(),
                strip_index_format: None,
            },
            depth_stencil,
            multisample: MultisampleState {
                count: key.msaa().samples(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            push_constant_ranges,
            label: Some(Cow::Borrowed("outline_pipeline")),
            zero_initialize_workgroup_memory: false,
        })
    }
}

impl GetBatchData for OutlinePipeline {
    type Param = (
        SRes<RenderOutlineInstances>,
        SRes<MeshAllocator>,
        SRes<SkinUniforms>,
    );
    type CompareData = (AssetId<Mesh>, Option<AssetId<Image>>);
    type BufferData = OutlineInstanceUniform;

    fn get_batch_data(
        (render_outlines, mesh_allocator, skin_uniforms): &SystemParamItem<Self::Param>,
        (_entity, main_entity): (Entity, MainEntity),
    ) -> Option<(Self::BufferData, Option<Self::CompareData>)> {
        let outline = render_outlines.get(&main_entity)?;
        let instance_data = outline.instance_data.prepare_instance(
            &outline.mesh_id,
            main_entity,
            mesh_allocator,
            skin_uniforms,
        );

        // Only batch entities with the same mesh and alpha mask texture
        let batch_data = if outline.automatic_batching {
            Some((outline.mesh_id, outline.alpha_mask_id))
        } else {
            None
        };

        Some((instance_data, batch_data))
    }
}

impl GetFullBatchData for OutlinePipeline {
    type BufferInputData = ();

    fn get_binned_batch_data(
        (render_outlines, mesh_allocator, skin_uniforms): &SystemParamItem<Self::Param>,
        main_entity: MainEntity,
    ) -> Option<Self::BufferData> {
        let outline = render_outlines.get(&main_entity)?;
        Some(outline.instance_data.prepare_instance(
            &outline.mesh_id,
            main_entity,
            mesh_allocator,
            skin_uniforms,
        ))
    }

    fn get_index_and_compare_data(
        _param: &SystemParamItem<Self::Param>,
        _main_entity: MainEntity,
    ) -> Option<(NonMaxU32, Option<Self::CompareData>)> {
        unimplemented!("GPU batching is not used.");
    }

    fn get_binned_index(
        _param: &SystemParamItem<Self::Param>,
        _main_entity: MainEntity,
    ) -> Option<NonMaxU32> {
        unimplemented!("GPU batching is not used.");
    }

    fn write_batch_indirect_parameters_metadata(
        _indexed: bool,
        _base_output_index: u32,
        _batch_set_index: Option<NonMaxU32>,
        _phase_indirect_parameters_buffers: &mut gpu_preprocessing::UntypedPhaseIndirectParametersBuffers,
        _indirect_parameters_offset: u32,
    ) {
        unimplemented!("GPU batching is not used.");
    }
}
