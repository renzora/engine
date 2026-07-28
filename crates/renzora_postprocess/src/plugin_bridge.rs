//! Turns a plugin's `add_render_pass` registration into a real render-graph pass.
//!
//! This is the render half of `renzora_plugin`'s C ABI. It lives here rather
//! than in `renzora_plugin` for a dependency reason that is worth stating
//! plainly: `renzora_plugin` must be publishable to crates.io so third-party
//! authors can `cargo add` it, and **a published crate cannot have path
//! dependencies**. So it never depends on `renzora`. Engine crates depend on
//! *it*, never the reverse — and this module is the engine side of that.
//!
//! ## Why the work is split across a world boundary
//!
//! A plugin registers during `renzora_plugin_init`, which runs in the **main**
//! world, where `Assets<Shader>` lives — so the WGSL becomes a shader asset
//! there. The pipeline cannot be built there: that needs `RenderDevice`,
//! `PipelineCache` and `FullscreenShader`, all **render**-world resources. So
//! registration parks a [`PendingRenderPass`] and this plugin drains the queue in
//! `finish`, which Bevy runs after every plugin's `build` — by which point
//! `load_global_plugins` has loaded the cdylibs and the render sub-app exists.

use bevy::core_pipeline::FullscreenShader;
use bevy::prelude::*;
use bevy::render::render_resource::binding_types::{sampler, texture_2d};
use bevy::render::render_resource::{
    BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, CachedRenderPipelineId,
    ColorTargetState, ColorWrites, FragmentState, Operations, PipelineCache,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor, Sampler,
    SamplerBindingType, SamplerDescriptor, ShaderStages, TextureFormat, TextureSampleType,
};
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::view::ViewTarget;
use bevy::render::RenderApp;
use renzora::postprocess::{RenderCompositionAppExt, RenderPass};
use renzora::RenderPhase;
use renzora_plugin::host::{PendingRenderPasses, RenderCallCtx};
use renzora_plugin::sys;

/// Drains plugin render-pass registrations and installs them.
pub struct PluginRenderBridgePlugin;

impl Plugin for PluginRenderBridgePlugin {
    fn build(&self, _app: &mut App) {}

    fn finish(&self, app: &mut App) {
        let Some(pending) = app.world_mut().remove_resource::<PendingRenderPasses>() else {
            return;
        };
        if pending.0.is_empty() {
            return;
        }
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            warn!("[plugin] no render sub-app — plugin render passes will not run");
            return;
        };

        for p in pending.0 {
            // Pipeline construction touches render-world resources whose presence
            // and timing we do not fully control, and this runs during app build
            // — a panic here means the editor never reaches the splash screen. A
            // plugin's render pass failing must cost that pass, not the session.
            let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build_pipeline(render_app.world_mut(), &p.shader, &p.id)
            }));
            let Ok(Some(built)) = built else {
                error!(
                    "[plugin] could not build a pipeline for render pass `{}` — skipping it",
                    p.id
                );
                continue;
            };
            // `add_render_pass` wants a `&'static str`. A plugin's id is only
            // known at runtime, so it leaks — bounded by the number of passes a
            // plugin registers at load, which happens once per process.
            let id: &'static str = Box::leak(p.id.clone().into_boxed_str());
            render_app.add_render_pass(
                id,
                phase(p.phase),
                p.order,
                PluginPass {
                    id: p.id.clone(),
                    callback: p.callback,
                    layout: built.layout,
                    sampler: built.sampler,
                    shader: built.shader,
                    vertex: built.vertex,
                    pipelines: std::sync::Mutex::new(Default::default()),
                },
            );
            info!("[plugin] registered render pass `{id}` (phase {:?}, order {})", p.phase, p.order);
        }
    }
}

struct Built {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    shader: Handle<Shader>,
    vertex: bevy::render::render_resource::VertexState,
}

/// Prepare everything a pass needs *except* the pipeline.
///
/// The pipeline deliberately is NOT built here. `init_pipeline` queues a fixed
/// pair — `Rgba8UnormSrgb` and `Rgba16Float` — and picks between them at draw
/// time. That works for the built-in effects but is a guess, and a wrong guess
/// is not a soft failure: a pipeline whose colour target does not match the
/// render pass is a wgpu validation error, which this engine escalates to an
/// unrecoverable GPU panic. This machine's surface is `Bgra8UnormSrgb`, which is
/// neither of the two.
///
/// So pipelines are built lazily, keyed on the format the view actually has (see
/// `PluginPass::pipeline_for`). One entry per format ever encountered — in
/// practice one or two.
fn build_pipeline(world: &mut World, shader: &Handle<Shader>, label: &str) -> Option<Built> {
    let layout = BindGroupLayoutDescriptor::new(
        "plugin_pass_bind_group_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
            ),
        ),
    );
    let sampler = world
        .get_resource::<RenderDevice>()?
        .create_sampler(&SamplerDescriptor::default());
    let vertex = world.get_resource::<FullscreenShader>()?.to_vertex_state();

    let _ = label;
    Some(Built {
        layout,
        sampler,
        shader: shader.clone(),
        vertex,
    })
}

fn phase(p: sys::RenderPhase) -> RenderPhase {
    match p {
        sys::RenderPhase::Gi => RenderPhase::Gi,
        sys::RenderPhase::HdrPost => RenderPhase::HdrPost,
        sys::RenderPhase::LdrPost => RenderPhase::LdrPost,
        sys::RenderPhase::Overlay => RenderPhase::Overlay,
    }
}

struct PluginPass {
    id: String,
    callback: sys::RenderCallback,
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    shader: Handle<Shader>,
    vertex: bevy::render::render_resource::VertexState,
    /// One pipeline per colour format this pass has been asked to draw into.
    /// `Mutex` because `RenderPass::run` takes `&self`; contention is nil since
    /// entries are created once per format.
    pipelines: std::sync::Mutex<bevy::platform::collections::HashMap<TextureFormat, CachedRenderPipelineId>>,
}

impl PluginPass {
    fn pipeline_for(&self, cache: &PipelineCache, format: TextureFormat) -> CachedRenderPipelineId {
        let mut map = self.pipelines.lock().unwrap();
        *map.entry(format).or_insert_with(|| {
            cache.queue_render_pipeline(RenderPipelineDescriptor {
                label: Some(format!("plugin_pass_{}", self.id).into()),
                layout: vec![self.layout.clone()],
                vertex: self.vertex.clone(),
                fragment: Some(FragmentState {
                    shader: self.shader.clone(),
                    targets: vec![Some(ColorTargetState {
                        format,
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    })],
                    ..default()
                }),
                ..default()
            })
        })
    }
}

impl RenderPass for PluginPass {
    fn run(
        &self,
        world: &World,
        render_context: &mut RenderContext,
        view_target: &ViewTarget,
        _view_entity: Entity,
    ) {
        let cache = world.resource::<PipelineCache>();
        let fmt = view_target.main_texture_format();
        bevy::log::debug_once!("[plugin] pass `{}` first run, view format {:?}", self.id, fmt);
        let post_process = view_target.post_process_write();
        // Key on the DESTINATION's format, not a guess: this is the texture the
        // pass writes into, and it is what wgpu validates the pipeline against.
        let id = self.pipeline_for(cache, view_target.main_texture_format());
        // Still compiling on the first frames a pass exists — skipping is
        // correct, not an error.
        let Some(pipeline) = cache.get_render_pipeline(id) else {
            match cache.get_render_pipeline_state(id) {
                bevy::render::render_resource::CachedPipelineState::Err(e) => {
                    bevy::log::error_once!("[plugin] pass `{}` pipeline FAILED: {e}", self.id);
                }
                s => {
                    bevy::log::debug_once!("[plugin] pass `{}` pipeline not ready: {s:?}", self.id);
                }
            }
            return;
        };
        bevy::log::debug_once!("[plugin] pass `{}` pipeline ready, invoking callback", self.id);

        let bind_group = render_context.render_device().create_bind_group(
            "plugin_pass_bind_group",
            &cache.get_bind_group_layout(&self.layout),
            &BindGroupEntries::sequential((post_process.source, &self.sampler)),
        );

        let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("plugin_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: post_process.destination,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        // The plugin records its own draw commands through the interface, which
        // is the point of the slice: plugin code executing inside the render
        // graph. `ctx` is only valid for this call — `RenderCtx` documents that,
        // and the borrow it wraps dies with this statement.
        let mut ctx = RenderCallCtx {
            pass: &mut pass,
            pipeline,
            bind_group: &bind_group,
        };
        let status = unsafe {
            (self.callback)(
                sys::RenderCtx(&mut ctx as *mut RenderCallCtx as *mut core::ffi::c_void),
                sys::PipelineId(0),
            )
        };
        if status == sys::SystemStatus::Panicked {
            error!("[plugin] render callback panicked");
        }
    }
}
