//! Module containing the custom `Render Pipelines` used by Firefly.

use std::borrow::Cow;

use bevy::{
    asset::{embedded_asset, load_embedded_asset},
    core_pipeline::{FullscreenShader, tonemapping::get_lut_bind_group_layout_entries},
    mesh::{PrimitiveTopology, VertexBufferLayout, VertexFormat},
    prelude::*,
    render::{
        RenderApp, RenderStartup,
        render_resource::{
            BindGroupLayoutDescriptor, BindGroupLayoutEntries, BlendComponent, BlendFactor,
            BlendOperation, BlendState, CachedRenderPipelineId, ColorTargetState, ColorWrites,
            FilterMode, FragmentState, FrontFace, MultisampleState, PolygonMode, PrimitiveState,
            RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages,
            SpecializedRenderPipeline, SpecializedRenderPipelines, TextureFormat,
            TextureSampleType, VertexAttribute, VertexState, VertexStepMode,
            binding_types::{
                sampler, storage_buffer_read_only, texture_2d, texture_2d_array, uniform_buffer,
            },
        },
        renderer::RenderDevice,
        view::{
            COLOR_TARGET_FORMAT_MASK_BITS, ViewTarget, ViewUniform, texture_format_from_code,
            texture_format_to_code,
        },
    },
    shader::{ShaderDefVal, load_shader_library},
};

use crate::{
    buffers::{BinIndices, OccluderPointer},
    data::UniformFireflyConfig,
    lights::UniformPointLight,
    occluders::{UniformOccluder, UniformRoundOccluder},
};

/// Plugin that initializes various Pipelines. Added automatically by [`FireflyPlugin`](crate::prelude::FireflyPlugin).
pub struct PipelinePlugin;

impl Plugin for PipelinePlugin {
    fn build(&self, app: &mut App) {
        load_shader_library!(app, "shaders/types.wgsl");
        load_shader_library!(app, "shaders/utils.wgsl");

        embedded_asset!(app, "shaders/create_lightmap.wgsl");
        embedded_asset!(app, "shaders/apply_lightmap.wgsl");
        embedded_asset!(app, "shaders/combine_lightmaps.wgsl");
        embedded_asset!(app, "shaders/sprite.wgsl");

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<SpecializedRenderPipelines<LightmapCreationPipeline>>()
            .init_resource::<SpecializedRenderPipelines<LightmapApplicationPipeline>>()
            .init_resource::<SpecializedRenderPipelines<LightmapCombinationPipeline>>()
            .init_resource::<SpecializedRenderPipelines<SpritePipeline>>();

        render_app.add_systems(
            RenderStartup,
            (
                init_lightmap_creation_pipeline,
                init_lightmap_application_pipeline,
                init_lightmap_combination_pipeline,
                init_sprite_pipeline,
            ),
        );
    }
}

/// Pipeline that creates the lightmap from the relevant bindings.
#[derive(Resource)]
pub struct LightmapCreationPipeline {
    pub layout: BindGroupLayoutDescriptor,
    pub lut_layout: BindGroupLayoutDescriptor,
    pub sampler: Sampler,
    pub vertex_state: VertexState,
    pub shader: Handle<Shader>,
}

fn init_lightmap_creation_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    fullscreen_shader: Res<FullscreenShader>,
    asset_server: Res<AssetServer>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "create lightmap layout",
        &BindGroupLayoutEntries::with_indices(
            ShaderStages::FRAGMENT,
            (
                // sampler
                (0, sampler(SamplerBindingType::Filtering)),
                // point lights
                (1, storage_buffer_read_only::<UniformPointLight>(false)),
                (2, storage_buffer_read_only::<u32>(false)),
                // round occluders
                (3, storage_buffer_read_only::<UniformRoundOccluder>(false)),
                // poly occluders
                (4, storage_buffer_read_only::<UniformOccluder>(false)),
                // vertices
                (5, storage_buffer_read_only::<Vec2>(false)),
                // occluders
                (6, storage_buffer_read_only::<OccluderPointer>(false)),
                // bins
                (7, storage_buffer_read_only::<BinIndices>(false)),
                // sprite stencil
                (8, texture_2d(TextureSampleType::Float { filterable: true })),
                // sprite normal map
                (9, texture_2d(TextureSampleType::Float { filterable: true })),
                // config,
                (10, uniform_buffer::<UniformFireflyConfig>(false)),
            ),
        ),
    );

    let tonemapping_lut_entries = get_lut_bind_group_layout_entries();
    let lut_layout = BindGroupLayoutDescriptor::new(
        "sprite_view_layout",
        &BindGroupLayoutEntries::with_indices(
            ShaderStages::VERTEX_FRAGMENT,
            (
                (0, uniform_buffer::<ViewUniform>(true)),
                (
                    1,
                    tonemapping_lut_entries[0].visibility(ShaderStages::FRAGMENT),
                ),
                (
                    2,
                    tonemapping_lut_entries[1].visibility(ShaderStages::FRAGMENT),
                ),
            ),
        ),
    );

    let sampler = render_device.create_sampler(&SamplerDescriptor::default());
    let vertex_state = fullscreen_shader.to_vertex_state();

    commands.insert_resource(LightmapCreationPipeline {
        layout,
        lut_layout,
        sampler,
        vertex_state,
        shader: load_embedded_asset!(asset_server.as_ref(), "shaders/create_lightmap.wgsl"),
    });
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    #[repr(transparent)]
    // NOTE: Apparently quadro drivers support up to 64x MSAA.
    // MSAA uses the highest 3 bits for the MSAA log2(sample count) to support up to 128x MSAA.
    pub struct LightPipelineKey: u32 {
        const NONE                              = 0;
        const TONEMAP_IN_SHADER                 = 1 << 0;
        const DEBAND_DITHER                     = 1 << 1;
        const SRGB_COMPOSITING                  = 1 << 2;
        const OKLAB_COMPOSITING                 = 1 << 3;
        const COLOR_TARGET_FORMAT_RESERVED_BITS = Self::COLOR_TARGET_FORMAT_MASK_BITS << Self::COLOR_TARGET_FORMAT_SHIFT_BITS;
        const MSAA_RESERVED_BITS                = Self::MSAA_MASK_BITS << Self::MSAA_SHIFT_BITS;
        const TONEMAP_METHOD_RESERVED_BITS      = Self::TONEMAP_METHOD_MASK_BITS << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_NONE               = 0 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_REINHARD           = 1 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_REINHARD_LUMINANCE = 2 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_ACES_FITTED        = 3 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_AGX                = 4 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_SOMEWHAT_BORING_DISPLAY_TRANSFORM = 5 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_TONY_MC_MAPFACE    = 6 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_BLENDER_FILMIC     = 7 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_PBR_NEUTRAL        = 8 << Self::TONEMAP_METHOD_SHIFT_BITS;

        const COMBINE_LIGHTMAPS                 = 1 << 31;
        const LIGHTMAP_FILTERING                = 1 << 30;
    }
}

impl LightPipelineKey {
    const COLOR_TARGET_FORMAT_MASK_BITS: u32 = COLOR_TARGET_FORMAT_MASK_BITS;
    const COLOR_TARGET_FORMAT_SHIFT_BITS: u32 = 4;
    const MSAA_MASK_BITS: u32 = 0b111;
    const MSAA_SHIFT_BITS: u32 = 16 - Self::MSAA_MASK_BITS.count_ones();
    const TONEMAP_METHOD_MASK_BITS: u32 = 0b1111;
    const TONEMAP_METHOD_SHIFT_BITS: u32 =
        Self::MSAA_SHIFT_BITS - Self::TONEMAP_METHOD_MASK_BITS.count_ones();

    #[inline]
    pub const fn from_msaa_samples(msaa_samples: u32) -> Self {
        let msaa_bits =
            (msaa_samples.trailing_zeros() & Self::MSAA_MASK_BITS) << Self::MSAA_SHIFT_BITS;
        Self::from_bits_retain(msaa_bits)
    }

    #[inline]
    pub const fn msaa_samples(&self) -> u32 {
        1 << ((self.bits() >> Self::MSAA_SHIFT_BITS) & Self::MSAA_MASK_BITS)
    }

    #[inline]
    pub fn from_target_format(format: TextureFormat) -> Self {
        let code = texture_format_to_code(format)
            .expect("Texture format is not supported by the pipeline") as u32;
        Self::from_bits_retain(
            (code & Self::COLOR_TARGET_FORMAT_MASK_BITS) << Self::COLOR_TARGET_FORMAT_SHIFT_BITS,
        )
    }

    #[inline]
    pub fn target_format(&self) -> TextureFormat {
        let code = ((self.bits() >> Self::COLOR_TARGET_FORMAT_SHIFT_BITS)
            & Self::COLOR_TARGET_FORMAT_MASK_BITS) as u8;
        texture_format_from_code(code)
            .expect("Unknown bits in `COLOR_TARGET_FORMAT_MASK_BITS` of the pipeline key")
    }
}

impl SpecializedRenderPipeline for LightmapCreationPipeline {
    type Key = LightPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let mut shader_defs = Vec::new();
        if key.contains(LightPipelineKey::TONEMAP_IN_SHADER) {
            shader_defs.push("TONEMAP_IN_SHADER".into());

            shader_defs.push(ShaderDefVal::UInt(
                "TONEMAPPING_LUT_TEXTURE_BINDING_INDEX".into(),
                1,
            ));
            shader_defs.push(ShaderDefVal::UInt(
                "TONEMAPPING_LUT_SAMPLER_BINDING_INDEX".into(),
                2,
            ));

            let method = key.intersection(LightPipelineKey::TONEMAP_METHOD_RESERVED_BITS);

            if method == LightPipelineKey::TONEMAP_METHOD_NONE {
                shader_defs.push("TONEMAP_METHOD_NONE".into());
            } else if method == LightPipelineKey::TONEMAP_METHOD_REINHARD {
                shader_defs.push("TONEMAP_METHOD_REINHARD".into());
            } else if method == LightPipelineKey::TONEMAP_METHOD_REINHARD_LUMINANCE {
                shader_defs.push("TONEMAP_METHOD_REINHARD_LUMINANCE".into());
            } else if method == LightPipelineKey::TONEMAP_METHOD_ACES_FITTED {
                shader_defs.push("TONEMAP_METHOD_ACES_FITTED".into());
            } else if method == LightPipelineKey::TONEMAP_METHOD_AGX {
                shader_defs.push("TONEMAP_METHOD_AGX".into());
            } else if method == LightPipelineKey::TONEMAP_METHOD_SOMEWHAT_BORING_DISPLAY_TRANSFORM {
                shader_defs.push("TONEMAP_METHOD_SOMEWHAT_BORING_DISPLAY_TRANSFORM".into());
            } else if method == LightPipelineKey::TONEMAP_METHOD_BLENDER_FILMIC {
                shader_defs.push("TONEMAP_METHOD_BLENDER_FILMIC".into());
            } else if method == LightPipelineKey::TONEMAP_METHOD_TONY_MC_MAPFACE {
                shader_defs.push("TONEMAP_METHOD_TONY_MC_MAPFACE".into());
            }

            // Debanding is tied to tonemapping in the shader, cannot run without it.
            if key.contains(LightPipelineKey::DEBAND_DITHER) {
                shader_defs.push("DEBAND_DITHER".into());
            }
        }

        let format = key.target_format();
        RenderPipelineDescriptor {
            label: Some(Cow::Borrowed("lightmap creation pipeline")),
            layout: vec![self.lut_layout.clone(), self.layout.clone()],
            vertex: self.vertex_state.clone(),
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                targets: vec![Some(ColorTargetState {
                    format,
                    blend: Some(BlendState {
                        color: BlendComponent {
                            src_factor: BlendFactor::One,
                            dst_factor: BlendFactor::One,
                            operation: BlendOperation::Max,
                        },
                        alpha: BlendComponent::REPLACE,
                    }),
                    write_mask: ColorWrites::ALL,
                })],
                shader_defs,
                entry_point: Some(Cow::Borrowed("fragment")),
            }),
            primitive: default(),
            depth_stencil: default(),
            multisample: MultisampleState {
                count: 1,
                ..default()
            },
            ..default()
        }
    }
}

/// Pipeline that applies the lightmap over the fullscreen view.
#[derive(Resource)]
pub struct LightmapApplicationPipeline {
    pub layout: BindGroupLayoutDescriptor,
    pub filtering_sampler: Sampler,
    pub non_filtering_sampler: Sampler,
    pub vertex_state: VertexState,
    pub shader: Handle<Shader>,
}

impl LightmapApplicationPipeline {
    pub(crate) fn specialize_layout(
        &self,
        combined: bool,
        filter_lightmap: bool,
    ) -> BindGroupLayoutDescriptor {
        let mut layout = self.layout.clone();

        if !filter_lightmap {
            layout.entries[3] =
                sampler(SamplerBindingType::NonFiltering).build(3, ShaderStages::FRAGMENT);
        }

        if combined {
            layout.entries.push(
                texture_2d_array(TextureSampleType::Float { filterable: true })
                    .build(5, ShaderStages::FRAGMENT),
            );
        }

        layout
    }
}

#[derive(Component)]
pub struct SpecializedApplicationPipeline {
    pub id: CachedRenderPipelineId,
    pub is_combined: bool,
    pub filter_lightmap: bool,
}

fn init_lightmap_application_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    fullscreen_shader: Res<FullscreenShader>,
    asset_server: Res<AssetServer>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "apply lightmap layout simple",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                // screen texture
                texture_2d(TextureSampleType::Float { filterable: true }),
                // lightmap texture
                texture_2d(TextureSampleType::Float { filterable: true }),
                // screen filter
                sampler(SamplerBindingType::Filtering),
                // lightmap filter
                sampler(SamplerBindingType::Filtering),
                // config
                uniform_buffer::<UniformFireflyConfig>(false),
            ),
        ),
    );

    let filtering_sampler = render_device.create_sampler(&SamplerDescriptor {
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..default()
    });

    let non_filtering_sampler = render_device.create_sampler(&SamplerDescriptor {
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        ..default()
    });

    let vertex_state = fullscreen_shader.to_vertex_state();

    commands.insert_resource(LightmapApplicationPipeline {
        layout,
        filtering_sampler,
        non_filtering_sampler,
        vertex_state,
        shader: load_embedded_asset!(asset_server.as_ref(), "shaders/apply_lightmap.wgsl"),
    });
}

impl SpecializedRenderPipeline for LightmapApplicationPipeline {
    type Key = LightPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let format = key.target_format();

        let mut shader_defs = vec![];
        let mut combined = false;

        if key.contains(LightPipelineKey::COMBINE_LIGHTMAPS) {
            shader_defs.push("IS_COMBINED".into());
            combined = true;
        }

        if key.contains(LightPipelineKey::LIGHTMAP_FILTERING) {
            shader_defs.push("FILTER_LIGHTMAP".into());
        }

        let filter_lightmap = key.contains(LightPipelineKey::LIGHTMAP_FILTERING);

        RenderPipelineDescriptor {
            label: Some(Cow::Borrowed("lightmap application pipeline")),
            layout: vec![self.specialize_layout(combined, filter_lightmap)],
            vertex: self.vertex_state.clone(),
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                targets: vec![Some(ColorTargetState {
                    format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                shader_defs,
                entry_point: Some(Cow::Borrowed("fragment")),
            }),
            primitive: default(),
            depth_stencil: default(),
            multisample: MultisampleState {
                count: key.msaa_samples(),
                ..default()
            },
            ..default()
        }
    }
}

/// Pipeline that multiplies an array of lightmaps.
#[derive(Resource)]
pub struct LightmapCombinationPipeline {
    pub layout: BindGroupLayoutDescriptor,
    pub sampler: Sampler,
    pub vertex_state: VertexState,
    pub shader: Handle<Shader>,
}

#[derive(Component)]
pub struct SpecializedCombinationPipeline(pub CachedRenderPipelineId);

fn init_lightmap_combination_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    fullscreen_shader: Res<FullscreenShader>,
    asset_server: Res<AssetServer>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "`combine lightmaps layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<UniformFireflyConfig>(false),
            ),
        ),
    );

    let sampler = render_device.create_sampler(&SamplerDescriptor::default());
    let vertex_state = fullscreen_shader.to_vertex_state();

    commands.insert_resource(LightmapCombinationPipeline {
        layout,
        sampler,
        vertex_state,
        shader: load_embedded_asset!(asset_server.as_ref(), "shaders/combine_lightmaps.wgsl"),
    });
}

impl SpecializedRenderPipeline for LightmapCombinationPipeline {
    type Key = LightPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let format = key.target_format();
        RenderPipelineDescriptor {
            label: Some(Cow::Borrowed("lightmap combination pipeline")),
            layout: vec![self.layout.clone()],
            vertex: self.vertex_state.clone(),
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                targets: vec![Some(ColorTargetState {
                    format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                shader_defs: default(),
                entry_point: Some(Cow::Borrowed("fragment")),
            }),
            primitive: default(),
            depth_stencil: default(),
            multisample: MultisampleState {
                count: key.msaa_samples(),
                ..default()
            },
            ..default()
        }
    }
}

/// Pipeline that produces the stencil and normal textures from the sprite bindings.
#[derive(Resource)]
#[allow(dead_code)]
pub struct SpritePipeline {
    pub view_layout: BindGroupLayoutDescriptor,
    pub material_layout: BindGroupLayoutDescriptor,
    pub shader: Handle<Shader>,
}

fn init_sprite_pipeline(mut commands: Commands, asset_server: Res<AssetServer>) {
    let tonemapping_lut_entries = get_lut_bind_group_layout_entries();
    let view_layout = BindGroupLayoutDescriptor::new(
        "sprite_view_layout",
        &BindGroupLayoutEntries::with_indices(
            ShaderStages::VERTEX_FRAGMENT,
            (
                (0, uniform_buffer::<ViewUniform>(true)),
                (
                    1,
                    tonemapping_lut_entries[0].visibility(ShaderStages::FRAGMENT),
                ),
                (
                    2,
                    tonemapping_lut_entries[1].visibility(ShaderStages::FRAGMENT),
                ),
            ),
        ),
    );

    let material_layout = BindGroupLayoutDescriptor::new(
        "sprite_material_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                // sprite texture
                texture_2d(TextureSampleType::Float { filterable: true }),
                // normal map texture
                texture_2d(TextureSampleType::Float { filterable: true }),
                // sampler
                sampler(SamplerBindingType::Filtering),
                // dummy normal bool
                uniform_buffer::<u32>(false),
            ),
        ),
    );

    commands.insert_resource(SpritePipeline {
        view_layout,
        material_layout,
        shader: load_embedded_asset!(asset_server.as_ref(), "shaders/sprite.wgsl"),
    });
}

impl SpecializedRenderPipeline for SpritePipeline {
    type Key = SpritePipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let mut shader_defs = Vec::new();
        if key.contains(SpritePipelineKey::TONEMAP_IN_SHADER) {
            shader_defs.push("TONEMAP_IN_SHADER".into());
            shader_defs.push(ShaderDefVal::UInt(
                "TONEMAPPING_LUT_TEXTURE_BINDING_INDEX".into(),
                1,
            ));
            shader_defs.push(ShaderDefVal::UInt(
                "TONEMAPPING_LUT_SAMPLER_BINDING_INDEX".into(),
                2,
            ));

            let method = key.intersection(SpritePipelineKey::TONEMAP_METHOD_RESERVED_BITS);

            if method == SpritePipelineKey::TONEMAP_METHOD_NONE {
                shader_defs.push("TONEMAP_METHOD_NONE".into());
            } else if method == SpritePipelineKey::TONEMAP_METHOD_REINHARD {
                shader_defs.push("TONEMAP_METHOD_REINHARD".into());
            } else if method == SpritePipelineKey::TONEMAP_METHOD_REINHARD_LUMINANCE {
                shader_defs.push("TONEMAP_METHOD_REINHARD_LUMINANCE".into());
            } else if method == SpritePipelineKey::TONEMAP_METHOD_ACES_FITTED {
                shader_defs.push("TONEMAP_METHOD_ACES_FITTED".into());
            } else if method == SpritePipelineKey::TONEMAP_METHOD_AGX {
                shader_defs.push("TONEMAP_METHOD_AGX".into());
            } else if method == SpritePipelineKey::TONEMAP_METHOD_SOMEWHAT_BORING_DISPLAY_TRANSFORM
            {
                shader_defs.push("TONEMAP_METHOD_SOMEWHAT_BORING_DISPLAY_TRANSFORM".into());
            } else if method == SpritePipelineKey::TONEMAP_METHOD_BLENDER_FILMIC {
                shader_defs.push("TONEMAP_METHOD_BLENDER_FILMIC".into());
            } else if method == SpritePipelineKey::TONEMAP_METHOD_TONY_MC_MAPFACE {
                shader_defs.push("TONEMAP_METHOD_TONY_MC_MAPFACE".into());
            }

            // Debanding is tied to tonemapping in the shader, cannot run without it.
            if key.contains(SpritePipelineKey::DEBAND_DITHER) {
                shader_defs.push("DEBAND_DITHER".into());
            }
        }

        let instance_rate_vertex_buffer_layout = VertexBufferLayout {
            array_stride: 80,
            step_mode: VertexStepMode::Instance,
            attributes: vec![
                // @location(0) i_model_transpose_col0: vec4<f32>,
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: 0,
                    shader_location: 0,
                },
                // @location(1) i_model_transpose_col1: vec4<f32>,
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: 16,
                    shader_location: 1,
                },
                // @location(2) i_model_transpose_col2: vec4<f32>,
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: 32,
                    shader_location: 2,
                },
                // @location(3) i_uv_offset_scale: vec4<f32>,
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: 48,
                    shader_location: 3,
                },
                // @location(4) z: f32,
                VertexAttribute {
                    format: VertexFormat::Float32,
                    offset: 64,
                    shader_location: 4,
                },
                // @location(5) height: f32,
                VertexAttribute {
                    format: VertexFormat::Float32,
                    offset: 68,
                    shader_location: 5,
                },
                // @location(6) y: f32,
                VertexAttribute {
                    format: VertexFormat::Float32,
                    offset: 72,
                    shader_location: 6,
                },
            ],
        };

        let stencil_format = match key.contains(SpritePipelineKey::ENABLED_32BIT_STENCIL) {
            false => TextureFormat::Rgba16Float,
            true => TextureFormat::Rgba32Float,
        };

        RenderPipelineDescriptor {
            vertex: VertexState {
                shader: self.shader.clone(),
                entry_point: Some("vertex".into()),
                shader_defs: shader_defs.clone(),
                buffers: vec![instance_rate_vertex_buffer_layout],
            },
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                shader_defs,
                entry_point: Some("fragment".into()),
                targets: vec![
                    Some(ColorTargetState {
                        format: stencil_format,
                        blend: Some(BlendState::ALPHA_BLENDING),
                        write_mask: ColorWrites::ALL,
                    }),
                    Some(ColorTargetState {
                        format: TextureFormat::Rgba16Float,
                        blend: Some(BlendState::ALPHA_BLENDING),
                        write_mask: ColorWrites::ALL,
                    }),
                ],
            }),
            layout: vec![self.view_layout.clone(), self.material_layout.clone()],
            primitive: PrimitiveState {
                front_face: FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,

                polygon_mode: PolygonMode::Fill,
                conservative: false,
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
            },
            depth_stencil: None,
            multisample: default(),
            label: Some("sprite_stencil_pipeline".into()),
            zero_initialize_workgroup_memory: false,
            ..default()
        }
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    #[repr(transparent)]
    // NOTE: Apparently quadro drivers support up to 64x MSAA.
    // MSAA uses the highest 3 bits for the MSAA log2(sample count) to support up to 128x MSAA.
    pub struct SpritePipelineKey: u32 {
        const NONE                              = 0;
        const TONEMAP_IN_SHADER                 = 1 << 0;
        const DEBAND_DITHER                     = 1 << 1;
        const SRGB_COMPOSITING                  = 1 << 2;
        const OKLAB_COMPOSITING                 = 1 << 3;
        const COLOR_TARGET_FORMAT_RESERVED_BITS = Self::COLOR_TARGET_FORMAT_MASK_BITS << Self::COLOR_TARGET_FORMAT_SHIFT_BITS;
        const MSAA_RESERVED_BITS                = Self::MSAA_MASK_BITS << Self::MSAA_SHIFT_BITS;
        const TONEMAP_METHOD_RESERVED_BITS      = Self::TONEMAP_METHOD_MASK_BITS << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_NONE               = 0 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_REINHARD           = 1 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_REINHARD_LUMINANCE = 2 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_ACES_FITTED        = 3 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_AGX                = 4 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_SOMEWHAT_BORING_DISPLAY_TRANSFORM = 5 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_TONY_MC_MAPFACE    = 6 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_BLENDER_FILMIC     = 7 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_PBR_NEUTRAL        = 8 << Self::TONEMAP_METHOD_SHIFT_BITS;

        const ENABLED_32BIT_STENCIL = 1 << 31;
    }
}

impl SpritePipelineKey {
    const COLOR_TARGET_FORMAT_MASK_BITS: u32 = COLOR_TARGET_FORMAT_MASK_BITS;
    const COLOR_TARGET_FORMAT_SHIFT_BITS: u32 = 4;
    const MSAA_MASK_BITS: u32 = 0b111;
    const MSAA_SHIFT_BITS: u32 = 20 - Self::MSAA_MASK_BITS.count_ones();
    const TONEMAP_METHOD_MASK_BITS: u32 = 0b1111;
    const TONEMAP_METHOD_SHIFT_BITS: u32 =
        Self::MSAA_SHIFT_BITS - Self::TONEMAP_METHOD_MASK_BITS.count_ones();

    #[inline]
    pub const fn from_msaa_samples(msaa_samples: u32) -> Self {
        let msaa_bits =
            (msaa_samples.trailing_zeros() & Self::MSAA_MASK_BITS) << Self::MSAA_SHIFT_BITS;
        Self::from_bits_retain(msaa_bits)
    }

    #[inline]
    pub const fn msaa_samples(&self) -> u32 {
        1 << ((self.bits() >> Self::MSAA_SHIFT_BITS) & Self::MSAA_MASK_BITS)
    }

    /// Create a pipeline key from the view's color target format.
    #[inline]
    pub fn from_target_format(format: TextureFormat) -> Self {
        let code = texture_format_to_code(format)
            .expect("Texture format is not supported by the pipeline") as u32;
        Self::from_bits_retain(
            (code & Self::COLOR_TARGET_FORMAT_MASK_BITS) << Self::COLOR_TARGET_FORMAT_SHIFT_BITS,
        )
    }

    /// Color target format of the main pass for this pipeline key.
    #[inline]
    pub fn target_format(&self) -> TextureFormat {
        let code = ((self.bits() >> Self::COLOR_TARGET_FORMAT_SHIFT_BITS)
            & Self::COLOR_TARGET_FORMAT_MASK_BITS) as u8;
        texture_format_from_code(code)
            .expect("Unknown bits in `COLOR_TARGET_FORMAT_MASK_BITS` of the pipeline key")
    }
}
