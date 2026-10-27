//! One pipeline per material per pass, built from the material's own Bevy
//! pipeline.
//!
//! For each material a view draws, this asks Bevy for exactly the pipeline it
//! would use on an ordinary mesh (forward, prepass or deferred G-buffer), then
//! swaps the vertex stage for a fullscreen triangle at the material's depth
//! and compiles the fragment stage with `MESHLET_MESH_MATERIAL_PASS`, which
//! makes Bevy's PBR shaders take their surface from `resolve.wesl`. Group 2,
//! which Bevy's mesh bindings would occupy, holds the virtual geometry scene
//! instead; group 3 is the material's own bind group, untouched.
//!
//! Bevy's main-pass specializer keeps its fields private, so its few lines are
//! reproduced in [`specialize_main`]. The prepass one is public and used as is.

use std::any::Any;
use std::collections::HashMap;

use bevy::asset::UntypedAssetId;
use bevy::camera::Camera3d;
use bevy::core_pipeline::prepass::{DeferredPrepass, MotionVectorPrepass, NormalPrepass};
use bevy::material::key::{ErasedMaterialPipelineKey, ErasedMeshPipelineKey};
use bevy::material::{MaterialProperties, OpaqueRendererMethod};
use bevy::mesh::{
    Mesh, MeshAttributeCompressionFlags, MeshVertexBufferLayout, MeshVertexBufferLayoutRef,
    MeshVertexBufferLayouts,
};
use bevy::pbr::{
    MaterialFragmentShader, MaterialPipeline, MaterialVertexShader, MeshPipeline,
    MeshPipelineKey, PreparedMaterial, PrepassPipeline, PrepassPipelineSpecializer,
    ViewKeyCache, MATERIAL_BIND_GROUP_INDEX,
};
use bevy::prelude::*;
use bevy::render::erased_render_asset::ErasedRenderAssets;
use bevy::render::material_bind_groups::MaterialBindGroupAllocators;
use bevy::render::mesh::VertexBufferLayout;
use bevy::render::render_resource::*;
use bevy::render::view::ExtractedView;
use bevy::shader::ShaderDefVal;

use super::pipelines::VgPipelines;
use crate::material::VgMaterialTypes;
use super::scene::GpuScene;
use super::view::VgView;

/// What a material pipeline writes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Pass {
    /// Lit colour into the view target.
    Forward,
    /// Normals and motion vectors, for a forward view with those prepasses.
    Prepass,
    /// The deferred G-buffer, plus normals and motion vectors if wanted.
    Deferred,
}

/// `(material id, pipeline, material bind group)` per pass, for one view.
#[derive(Component, Default)]
pub(crate) struct VgViewMaterials {
    pub forward: Vec<(u16, CachedRenderPipelineId, BindGroup)>,
    pub prepass: Vec<(u16, CachedRenderPipelineId, BindGroup)>,
    pub deferred: Vec<(u16, CachedRenderPipelineId, BindGroup)>,
}

/// Material pipelines take their mesh layout from the vertex buffer, and
/// virtual geometry has none, so they are specialized against the attributes
/// its passes provide: position, normal, UV and, where there is one, tangent.
/// The material passes always provide a tangent (`resolve.wesl` derives one
/// when the mesh has none); the transparent pass only has the mesh's own.
pub(crate) fn vertex_layout(layouts: &mut MeshVertexBufferLayouts, tangents: bool) -> MeshVertexBufferLayoutRef {
    let mut ids = vec![
        Mesh::ATTRIBUTE_POSITION.id,
        Mesh::ATTRIBUTE_NORMAL.id,
        Mesh::ATTRIBUTE_UV_0.id,
    ];
    let mut attributes = vec![
        VertexAttribute {
            format: Mesh::ATTRIBUTE_POSITION.format,
            offset: 0,
            shader_location: 0,
        },
        VertexAttribute {
            format: Mesh::ATTRIBUTE_NORMAL.format,
            offset: 12,
            shader_location: 1,
        },
        VertexAttribute {
            format: Mesh::ATTRIBUTE_UV_0.format,
            offset: 24,
            shader_location: 2,
        },
    ];
    if tangents {
        ids.push(Mesh::ATTRIBUTE_TANGENT.id);
        attributes.push(VertexAttribute {
            format: Mesh::ATTRIBUTE_TANGENT.format,
            offset: 32,
            shader_location: 3,
        });
    }
    layouts.insert(MeshVertexBufferLayout::new(
        ids,
        VertexBufferLayout {
            array_stride: if tangents { 48 } else { 32 },
            step_mode: VertexStepMode::Vertex,
            attributes,
        },
        MeshAttributeCompressionFlags::empty(),
    ))
}

/// Bevy's `MaterialPipelineSpecializer::specialize`, whose fields are private.
pub(crate) fn specialize_main(
    material_pipeline: &MaterialPipeline,
    properties: &MaterialProperties,
    key: ErasedMaterialPipelineKey,
    layout: &MeshVertexBufferLayoutRef,
) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
    let mesh_key: MeshPipelineKey = key.mesh_key.downcast();
    let mut descriptor = material_pipeline.mesh_pipeline.specialize(mesh_key, layout)?;
    let group = ShaderDefVal::UInt("MATERIAL_BIND_GROUP".into(), MATERIAL_BIND_GROUP_INDEX as u32);
    descriptor.vertex.shader_defs.push(group.clone());
    if let Some(fragment) = descriptor.fragment.as_mut() {
        fragment.shader_defs.push(group);
    }
    if let Some(shader) = properties.get_shader(MaterialVertexShader) {
        descriptor.vertex.shader = shader;
    }
    if let Some(shader) = properties.get_shader(MaterialFragmentShader) {
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader = shader;
        }
    }
    if let Some(material_layout) = properties.material_layout.as_ref() {
        descriptor.layout.insert(3, material_layout.clone());
    }
    if properties.bindless {
        descriptor.vertex.shader_defs.push("BINDLESS".into());
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader_defs.push("BINDLESS".into());
        }
    }
    if let Some(specialize) = properties.user_specialize {
        specialize(material_pipeline as &dyn Any, &mut descriptor, layout, key)?;
    }
    Ok(descriptor)
}

/// Turn a material's ordinary pipeline into its virtual geometry pass.
fn material_pass_descriptor(
    base: RenderPipelineDescriptor,
    view_layouts: [BindGroupLayoutDescriptor; 2],
    properties: &MaterialProperties,
    pipelines: &VgPipelines,
    fragment_override: Option<Handle<Shader>>,
) -> Option<RenderPipelineDescriptor> {
    let mut fragment = base.fragment?;
    if let Some(shader) = fragment_override {
        fragment.shader = shader;
        fragment.entry_point = Some("fragment".into());
    }
    let material_layout = properties.material_layout.clone()?;
    let mut shader_defs = fragment.shader_defs;
    shader_defs.push("MESHLET_MESH_MATERIAL_PASS".into());
    shader_defs.extend(super::pipelines::vg_bindings(2, 0));
    let [view_layout, second_layout] = view_layouts;
    Some(RenderPipelineDescriptor {
        label: Some("vg_material_pass".into()),
        layout: vec![
            view_layout,
            second_layout,
            pipelines.shade_layout.clone(),
            material_layout,
        ],
        immediate_size: 0,
        vertex: VertexState {
            shader: pipelines.material_vertex.clone(),
            shader_defs: shader_defs.clone(),
            entry_point: Some("vertex".into()),
            buffers: Vec::new(),
            ..default()
        },
        primitive: PrimitiveState::default(),
        depth_stencil: Some(DepthStencilState {
            format: TextureFormat::Depth16Unorm,
            depth_write_enabled: Some(false),
            depth_compare: Some(CompareFunction::Equal),
            stencil: StencilState::default(),
            bias: DepthBiasState::default(),
        }),
        multisample: MultisampleState::default(),
        fragment: Some(FragmentState {
            shader: fragment.shader,
            shader_defs,
            entry_point: fragment.entry_point,
            targets: fragment.targets,
            constants: fragment.constants,
        }),
        zero_initialize_workgroup_memory: false,
    })
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn queue(
    mut commands: Commands,
    gpu: Res<GpuScene>,
    pipelines: Option<Res<VgPipelines>>,
    pipeline_cache: Res<PipelineCache>,
    mesh_pipeline: Res<MeshPipeline>,
    material_pipeline: Res<MaterialPipeline>,
    prepass_pipeline: Res<PrepassPipeline>,
    render_materials: Res<ErasedRenderAssets<PreparedMaterial>>,
    allocators: Res<MaterialBindGroupAllocators>,
    mut vertex_layouts: ResMut<MeshVertexBufferLayouts>,
    mut cache: Local<HashMap<(Pass, ErasedMaterialPipelineKey), CachedRenderPipelineId>>,
    view_keys: Res<ViewKeyCache>,
    types: Option<Res<VgMaterialTypes>>,
    views: Query<
        (
            Entity,
            &ExtractedView,
            (
                Has<NormalPrepass>,
                Has<MotionVectorPrepass>,
                Has<DeferredPrepass>,
            ),
        ),
        (With<Camera3d>, With<VgView>),
    >,
) {
    let (Some(pipelines), Some(types)) = (pipelines, types) else {
        return;
    };
    let layout = vertex_layout(&mut vertex_layouts, true);

    for (view_entity, view, (normal_prepass, motion_prepass, deferred_prepass)) in &views {
        // Exactly the key Bevy specializes this view's ordinary meshes with.
        // The forward pass binds Bevy's own view bind group, whose layout is
        // chosen from the same view features (atmosphere, OIT, SSR, contact
        // shadows, fog, ...); a key rebuilt here that missed one would pick a
        // different layout, and the draw would fail validation.
        let Some(&view_key) = view_keys.get(&view.retained_view_entity) else {
            continue;
        };
        let view_key = view_key
            | MeshPipelineKey::from_primitive_topology_and_strip_index(
                PrimitiveTopology::TriangleList,
                None,
            );

        let mut out = VgViewMaterials::default();
        for &(material_id, id) in &gpu.materials {
            if let Some(entries) = queue_material(
                material_id,
                id,
                view_key,
                (normal_prepass, motion_prepass, deferred_prepass),
                &render_materials,
                &allocators,
                &mesh_pipeline,
                &material_pipeline,
                &prepass_pipeline,
                &pipelines,
                &types,
                &pipeline_cache,
                &layout,
                &mut cache,
            ) {
                for (pass, entry) in entries {
                    match pass {
                        Pass::Forward => out.forward.push(entry),
                        Pass::Prepass => out.prepass.push(entry),
                        Pass::Deferred => out.deferred.push(entry),
                    }
                }
            }
        }
        commands.entity(view_entity).insert(out);
    }
}

type Entry = (u16, CachedRenderPipelineId, BindGroup);

#[allow(clippy::too_many_arguments)]
fn queue_material(
    material_id: UntypedAssetId,
    id: u16,
    view_key: MeshPipelineKey,
    (normal_prepass, motion_prepass, deferred_prepass): (bool, bool, bool),
    render_materials: &ErasedRenderAssets<PreparedMaterial>,
    allocators: &MaterialBindGroupAllocators,
    mesh_pipeline: &MeshPipeline,
    material_pipeline: &MaterialPipeline,
    prepass_pipeline: &PrepassPipeline,
    pipelines: &VgPipelines,
    types: &VgMaterialTypes,
    pipeline_cache: &PipelineCache,
    layout: &MeshVertexBufferLayoutRef,
    cache: &mut HashMap<(Pass, ErasedMaterialPipelineKey), CachedRenderPipelineId>,
) -> Option<Vec<(Pass, Entry)>> {
    let material = render_materials.get(material_id)?;
    let type_id = material_id.type_id();
    // A type that never registered may not have a visibility buffer branch in
    // its shaders, and compiling one that does not would fail.
    if !types.0.contains(&type_id) {
        return None;
    }
    let properties = &material.properties;
    // Masked materials were cut out by their raster pass already; shading only
    // ever sees pixels that survived it.
    if !matches!(properties.alpha_mode, AlphaMode::Opaque | AlphaMode::Mask(_))
        || properties.reads_view_transmission_texture
    {
        return None;
    }
    let bind_group = allocators
        .get(&type_id)?
        .get(material.binding.group)?
        .bind_group()?
        .clone();
    let wants_deferred = matches!(properties.render_method, OpaqueRendererMethod::Deferred);
    let key_for = |mesh_key: MeshPipelineKey| ErasedMaterialPipelineKey {
        mesh_key: ErasedMeshPipelineKey::new(mesh_key | properties.mesh_pipeline_key_bits.downcast()),
        material_key: properties.material_key.clone(),
        type_id,
    };

    let mut passes = Vec::new();
    if deferred_prepass && wants_deferred {
        passes.push(Pass::Deferred);
    } else {
        passes.push(Pass::Forward);
        if normal_prepass || motion_prepass {
            passes.push(Pass::Prepass);
        }
    }

    let mut out = Vec::new();
    for pass in passes {
        // The prepass and G-buffer pipelines are specialized without the
        // deferred bit unless they are the G-buffer one, exactly as Bevy does.
        let mesh_key = match pass {
            Pass::Forward => view_key,
            Pass::Prepass => view_key & !MeshPipelineKey::DEFERRED_PREPASS,
            Pass::Deferred => view_key | MeshPipelineKey::DEFERRED_PREPASS,
        };
        let key = key_for(mesh_key);
        let pipeline = if let Some(&cached) = cache.get(&(pass, key.clone())) {
            cached
        } else {
            let descriptor = match pass {
                Pass::Forward => {
                    let base = specialize_main(material_pipeline, properties, key.clone(), layout).ok()?;
                    let view_layout = mesh_pipeline.get_view_layout(mesh_key.into());
                    material_pass_descriptor(
                        base,
                        [view_layout.main_layout, view_layout.binding_array_layout],
                        properties,
                        pipelines,
                        None,
                    )?
                }
                Pass::Prepass | Pass::Deferred => {
                    let specializer = PrepassPipelineSpecializer {
                        pipeline: prepass_pipeline.clone(),
                        properties: material.properties.clone(),
                    };
                    let base = specializer.specialize(key.clone(), layout).ok()?;
                    let view_layout = if mesh_key.contains(MeshPipelineKey::MOTION_VECTOR_PREPASS) {
                        prepass_pipeline.view_layout_motion_vectors.clone()
                    } else {
                        prepass_pipeline.view_layout_no_motion_vectors.clone()
                    };
                    // Bevy's own `StandardMaterial` prepass shader does not
                    // compile for a visibility buffer pass; see
                    // `standard_prepass.wesl`. Its G-buffer shader is fine.
                    // Every registered type starts with `StandardMaterial`'s
                    // bindings (see `material.rs`), and the node-graph
                    // material's prepass is that base shader anyway, so the
                    // replacement serves them all.
                    let fragment_override =
                        (pass == Pass::Prepass).then(|| pipelines.standard_prepass.clone());
                    material_pass_descriptor(
                        base,
                        [view_layout, prepass_pipeline.empty_layout.clone()],
                        properties,
                        pipelines,
                        fragment_override,
                    )?
                }
            };
            let id = pipeline_cache.queue_render_pipeline(descriptor);
            cache.insert((pass, key), id);
            id
        };
        out.push((pass, (id, pipeline, bind_group.clone())));
    }
    Some(out)
}
