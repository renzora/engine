//! Unified post-process pipeline that supports multiple simultaneous effects.
//!
//! Instead of adding a separate render graph node per effect, all effects are
//! processed by a single `UnifiedPostProcessNode`. Each effect registers a
//! type-erased handler that only executes when the camera has the corresponding
//! component. This means inactive effects have zero render graph overhead.

use core::marker::PhantomData;

use bevy::core_pipeline::{
    core_3d::graph::{Core3d, Node3d},
    FullscreenShader,
};
use bevy::prelude::*;
use bevy::render::{
    extract_component::{
        ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
        UniformComponentPlugin,
    },
    render_graph::{
        InternedRenderLabel, InternedRenderSubGraph, NodeRunError, RenderGraphContext,
        RenderGraphError, RenderGraphExt, RenderLabel, RenderSubGraph, ViewNode, ViewNodeRunner,
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

// ---------------------------------------------------------------------------
// Public trait
// ---------------------------------------------------------------------------

/// Trait for post-process effects.
///
/// Only `fragment_shader()` is required. The other methods are kept for
/// backwards compatibility but are no longer used by the unified pipeline.
pub trait PostProcessEffect:
    Component + ExtractComponent + Clone + Copy + ShaderType + WriteInto + Default + 'static
{
    fn fragment_shader() -> ShaderRef;

    /// No longer used — effect ordering is determined by plugin registration order.
    fn node_edges() -> Vec<InternedRenderLabel> { vec![] }

    /// No longer used — all effects run in the Core3d sub-graph.
    fn sub_graph() -> Option<InternedRenderSubGraph> {
        Some(Core3d.intern())
    }

    /// No longer used — effects no longer have individual render graph nodes.
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

// ---------------------------------------------------------------------------
// Per-effect pipeline resource (unchanged)
// ---------------------------------------------------------------------------

#[derive(Resource)]
struct PostProcessPipeline<T: PostProcessEffect> {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
    pipeline_id_hdr: CachedRenderPipelineId,
    _marker: PhantomData<T>,
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

// ---------------------------------------------------------------------------
// Type-erased effect handler
// ---------------------------------------------------------------------------

trait EffectHandler: Send + Sync + 'static {
    /// Execute this effect for a single view. Returns silently if the view
    /// does not have the required component (effect is inactive).
    fn execute(
        &self,
        world: &World,
        render_context: &mut RenderContext,
        view_target: &ViewTarget,
        view_entity: Entity,
    ) -> Result<(), NodeRunError>;
}

struct TypedEffectHandler<T: PostProcessEffect>(PhantomData<T>);

impl<T: PostProcessEffect> EffectHandler for TypedEffectHandler<T> {
    fn execute(
        &self,
        world: &World,
        render_context: &mut RenderContext,
        view_target: &ViewTarget,
        view_entity: Entity,
    ) -> Result<(), NodeRunError> {
        // Skip if this view doesn't have the effect component
        let Some(settings_index) = world.get::<DynamicUniformIndex<T>>(view_entity) else {
            return Ok(());
        };

        let Some(pipeline) = world.get_resource::<PostProcessPipeline<T>>() else {
            return Ok(());
        };
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

// ---------------------------------------------------------------------------
// Unified render graph node
// ---------------------------------------------------------------------------

/// Registry of all registered post-process effect handlers.
#[derive(Resource, Default)]
struct PostProcessRegistry {
    handlers: Vec<Box<dyn EffectHandler>>,
}

/// Render label for the single unified post-process node.
#[derive(Debug, Hash, PartialEq, Eq, Clone)]
struct UnifiedPostProcess;

impl RenderLabel for UnifiedPostProcess {
    fn dyn_clone(&self) -> Box<dyn RenderLabel> {
        Box::new(self.clone())
    }
}

/// Single render graph node that processes all active post-process effects.
/// Only effects whose component is present on the camera will execute.
#[derive(Default)]
struct UnifiedPostProcessNode;

impl ViewNode for UnifiedPostProcessNode {
    type ViewQuery = (Entity, &'static ViewTarget);

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (entity, view_target): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let registry = world.resource::<PostProcessRegistry>();
        for handler in &registry.handlers {
            handler.execute(world, render_context, view_target, entity)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Core plugin (added once, sets up the unified node)
// ---------------------------------------------------------------------------

struct PostProcessCorePlugin;

impl Plugin for PostProcessCorePlugin {
    fn build(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.init_resource::<PostProcessRegistry>();

        render_app
            .add_render_graph_node::<ViewNodeRunner<UnifiedPostProcessNode>>(
                Core3d,
                UnifiedPostProcess,
            );

        if let Some(mut render_graph) =
            render_app.world_mut().get_resource_mut::<bevy::render::render_graph::RenderGraph>()
        {
            if let Some(graph) = render_graph.get_sub_graph_mut(Core3d) {
                let _ = graph.try_add_node_edge(
                    Node3d::Tonemapping.intern(),
                    UnifiedPostProcess.intern(),
                );
                let _ = graph.try_add_node_edge(
                    UnifiedPostProcess.intern(),
                    Node3d::EndMainPassPostProcessing.intern(),
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Public plugin (one per effect type)
// ---------------------------------------------------------------------------

/// Plugin that sets up a post-process effect with its own pipeline.
///
/// Each effect gets its own `PostProcessPipeline<T>` resource and registers
/// a handler with the unified post-process node. The effect only executes
/// when a camera entity has the corresponding component.
#[derive(Default)]
pub struct PostProcessPlugin<T: PostProcessEffect> {
    _marker: PhantomData<T>,
}

/// Copies a post-process settings component from source entities to target
/// cameras based on the EffectRouting table.
fn proxy_effect_to_camera<T: PostProcessEffect>(
    mut commands: Commands,
    sources: Query<(Entity, &T)>,
    routing: Res<renzora_core::EffectRouting>,
    mut removed: RemovedComponents<T>,
) {
    let any_removed = removed.read().next().is_some();
    for (target, source_list) in routing.iter() {
        let mut found = false;
        for &src in source_list {
            if let Ok((_, settings)) = sources.get(src) {
                commands.entity(*target).insert(*settings);
                found = true;
                break;
            }
        }
        if !found && (routing.is_changed() || any_removed) {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<T>();
            }
        }
    }
}

/// Removes the proxied component from all routed cameras when all sources are removed.
fn cleanup_proxy_effect<T: PostProcessEffect>(
    mut commands: Commands,
    sources: Query<(), With<T>>,
    routing: Res<renzora_core::EffectRouting>,
) {
    if sources.is_empty() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<T>();
            }
        }
    }
}

impl<T: PostProcessEffect> Plugin for PostProcessPlugin<T> {
    fn build(&self, app: &mut App) {
        // Ensure the unified node is set up (idempotent check)
        if !app.is_plugin_added::<PostProcessCorePlugin>() {
            app.add_plugins(PostProcessCorePlugin);
        }

        // Proxy effects from any entity to the camera
        app.add_systems(Update, (proxy_effect_to_camera::<T>, cleanup_proxy_effect::<T>));

        // Extract + uniform plugins handle moving data to the render world
        app.add_plugins((
            ExtractComponentPlugin::<T>::default(),
            UniformComponentPlugin::<T>::default(),
        ));

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        // Initialize this effect's GPU pipeline
        render_app.add_systems(RenderStartup, init_pipeline::<T>);

        // Register the type-erased handler with the unified node
        render_app
            .world_mut()
            .resource_mut::<PostProcessRegistry>()
            .handlers
            .push(Box::new(TypedEffectHandler::<T>(PhantomData)));
    }
}
