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
use bevy::ecs::component::ComponentId;
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
    fn build(&self, app: &mut App) {
        // Settings live on main-world entities but are read in the render world.
        // A main-world system copies the raw bytes into a resource each frame and
        // `ExtractResourcePlugin` carries it across — the untyped equivalent of
        // `ExtractComponentPlugin<T>`, which needs a concrete type a plugin
        // component does not have.
        // The dispatchers that actually run registered passes live in the
        // composition core. It used to arrive as a side effect of a typed
        // `PostProcessPlugin<T>`, but every effect is a standalone plugin now, so
        // nothing added it and every plugin effect was registered and never run.
        // This bridge is the thing that registers them, so this is where the
        // dependency belongs.
        if !app.is_plugin_added::<renzora::postprocess::PostProcessCorePlugin>() {
            app.add_plugins(renzora::postprocess::PostProcessCorePlugin);
        }

        app.init_resource::<PluginEffectSettings>()
            .init_resource::<PluginEffectComponents>()
            .init_resource::<PluginShaders>()
            .add_plugins(ExtractResourcePlugin::<PluginEffectSettings>::default())
            .add_systems(PostUpdate, (collect_effect_settings, reload_plugin_shaders));
    }

    fn finish(&self, app: &mut App) {
        // Registering a pass and *running* one are wired up in two different
        // places, and for a while nothing connected them: the dispatchers arrived
        // only as a side effect of a typed `PostProcessPlugin<T>`, so once every
        // effect became a standalone plugin they stopped being installed and all
        // sixty registered into a list nothing read. Silent, because registration
        // itself succeeded.
        //
        // `build` adds the core now, so this cannot happen — but the invariant is
        // worth a line rather than a comment, because the failure it guards
        // against produced no error at any layer and took a day to find.
        debug_assert!(
            app.is_plugin_added::<renzora::postprocess::PostProcessCorePlugin>(),
            "PostProcessCorePlugin is missing — passes will register and never run"
        );

        let pending = app
            .world_mut()
            .remove_resource::<PendingRenderPasses>()
            .unwrap_or_default();
        let effects = app
            .world_mut()
            .remove_resource::<renzora_plugin::host::PendingPostProcesses>()
            .map(|p| p.0)
            .unwrap_or_default();

        // Remember which shader handle each id was built against, so a reload can
        // overwrite the asset rather than register a second pass. Recorded here
        // because this is the only place that knows the handle a pipeline actually
        // captured.
        {
            let mut known = app
                .world_mut()
                .get_resource_or_insert_with(PluginShaders::default);
            for e in &effects {
                known.0.insert(e.id.clone(), (e.shader.clone(), e.wgsl.clone()));
            }
            for p in &pending.0 {
                known.0.insert(p.id.clone(), (p.shader.clone(), p.wgsl.clone()));
            }
        }

        // Tell the main-world copier which components to collect.
        if !effects.is_empty() {
            let wanted: Vec<_> = effects
                .iter()
                .map(|e| (e.settings, e.settings_size as usize))
                .collect();
            app.world_mut().insert_resource(PluginEffectComponents(wanted));
        }

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            if !pending.0.is_empty() || !effects.is_empty() {
                warn!("[plugin] no render sub-app — plugin render passes will not run");
            }
            return;
        };

        for e in effects {
            let Ok(Some(built)) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build_effect(render_app.world_mut(), &e, &e.wgsl)
            })) else {
                error!("[plugin] could not build effect `{}` — skipping it", e.id);
                continue;
            };
            let id: &'static str = Box::leak(e.id.clone().into_boxed_str());
            render_app.add_render_pass(id, phase(e.phase), e.order, built);
            info!("[plugin] registered effect `{id}` (phase {:?}, order {})", e.phase, e.order);
        }

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

/// The shader handle each registered pass or effect is actually built against,
/// with the source it was built from.
///
/// Recorded so a reload can swap a shader in place. The pass keeps this handle
/// inside a pipeline it built once, so a reloaded plugin's *fresh* handle is
/// invisible to it — the only thing that reaches the GPU is rewriting the asset
/// this handle points at, which makes Bevy's pipeline cache recompile.
#[derive(Resource, Default)]
struct PluginShaders(std::collections::HashMap<String, (Handle<Shader>, String)>);

/// Swap in a plugin's recompiled shader without rebuilding its pipeline.
///
/// `finish` runs once, and it *removes* `PendingRenderPasses` /
/// `PendingPostProcesses`. A reloaded plugin re-registers into fresh copies of
/// those resources, and before this nothing looked at them again: the effect kept
/// running the WGSL from the build that had been replaced.
///
/// What makes this cheap is that a shader is an asset. Overwriting it invalidates
/// every pipeline depending on it and Bevy recompiles on its own, so there is no
/// pipeline to rebuild, no bind group layout to re-derive, and nothing to
/// re-register with the render graph. Which is also why this can live in a
/// main-world system with no access to the render sub-app.
fn reload_plugin_shaders(
    mut effects: Option<ResMut<renzora_plugin::host::PendingPostProcesses>>,
    mut passes: Option<ResMut<PendingRenderPasses>>,
    mut known: ResMut<PluginShaders>,
    mut shaders: ResMut<Assets<Shader>>,
) {
    // `(id, wgsl, settings_size)` — `None` for a render pass, which has no
    // uniform to validate against.
    let mut incoming: Vec<(String, String, Option<u64>)> = Vec::new();
    if let Some(effects) = effects.as_mut() {
        for e in std::mem::take(&mut effects.0) {
            incoming.push((e.id, e.wgsl, Some(e.settings_size)));
        }
    }
    if let Some(passes) = passes.as_mut() {
        for p in std::mem::take(&mut passes.0) {
            incoming.push((p.id, p.wgsl, None));
        }
    }
    if incoming.is_empty() {
        return;
    }

    for (id, wgsl, settings_size) in incoming {
        let Some((handle, current)) = known.0.get(&id) else {
            // An effect or pass that did not exist at startup. Registering one
            // needs the render sub-app, which a main-world system cannot reach.
            warn!(
                "[plugin] `{id}` is a new render pass or effect — adding one needs a \
                 restart; editing an existing one does not."
            );
            continue;
        };
        if *current == wgsl {
            continue;
        }
        // Validate BEFORE swapping. A shader whose uniform no longer matches the
        // settings struct is a fatal GPU error rather than a recoverable one, and
        // the whole point of a live reload is that a mistake costs you nothing.
        if let Some(expected) = settings_size {
            if let Err(why) = validate_effect_shader(&wgsl, expected) {
                error!("[plugin] `{id}` shader rejected, keeping the running one: {why}");
                continue;
            }
        }
        let handle = handle.clone();
        // Only fails if the handle's generation is stale, which here means the
        // shader asset was dropped between the watcher firing and this running.
        // Nothing to recover — say so and move on rather than swallowing it.
        if let Err(why) = shaders.insert(handle.id(), Shader::from_wgsl(wgsl.clone(), id.clone())) {
            error!("[plugin] `{id}` shader reload dropped: {why}");
            continue;
        }
        known.0.insert(id.clone(), (handle, wgsl));
        info!("[plugin] `{id}` shader reloaded");
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
        // A phase from a newer ABI. `LdrPost` is the safest home — it runs after
        // tonemapping, where an effect written for a phase this build lacks is
        // at worst in the wrong colour space rather than sampling a target that
        // does not exist yet.
        other => {
            bevy::log::warn!(
                "plugin asked for render phase {} which this build does not have —                  running it in LdrPost instead",
                other.0
            );
            RenderPhase::LdrPost
        }
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
        // An unrecognised status is a failure, not a success.
        if status == sys::SystemStatus::Panicked || !status.is_known() {
            error!("[plugin] render callback panicked");
        }
    }
}

// ── Parameterised effects ────────────────────────────────────────────────────
//
// The difference from a raw pass is that the host owns the whole draw: the
// plugin supplies a shader and a settings component and writes no render code.
// That means the host has to solve the problem the plugin would otherwise solve
// itself — getting main-world component data into the render world.
//
// Bevy's answer is `ExtractComponentPlugin<T>`, which is generic over a concrete
// type. A plugin component has no Rust type here, so instead a main-world system
// copies the raw bytes into a resource and `ExtractResourcePlugin` carries that
// across. One indirection more than the typed path, and no monomorphisation.

use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_resource::{Buffer, BufferDescriptor, BufferUsages};
use bevy::render::renderer::RenderQueue;

/// Latest settings bytes per effect, refreshed each frame in the main world and
/// extracted wholesale.
///
/// Keyed by the settings component. Values are the raw component bytes — the
/// host never interprets them; the shader does.
#[derive(Resource, Clone, Default, ExtractResource)]
pub struct PluginEffectSettings(pub bevy::platform::collections::HashMap<ComponentId, Vec<u8>>);

/// Which components to collect, so the copier does not walk the world blindly.
#[derive(Resource, Clone, Default)]
pub struct PluginEffectComponents(pub Vec<(ComponentId, usize)>);

/// Copy each effect's settings out of the world.
///
/// Takes the FIRST entity carrying the component. Per-camera effects would need
/// this keyed by view entity and the uniform made dynamic-offset — worth doing,
/// but it is a strictly larger change and a single global value is enough to
/// prove the parameter path.
fn collect_effect_settings(world: &mut World) {
    let Some(wanted) = world.get_resource::<PluginEffectComponents>().cloned() else {
        return;
    };
    let mut out = bevy::platform::collections::HashMap::default();
    for (id, size) in wanted.0 {
        let found = world
            .iter_entities()
            .find_map(|e| e.get_by_id(id).ok().map(|p| unsafe {
                // SAFETY: `size` is the size this component was registered with.
                std::slice::from_raw_parts(p.as_ptr(), size).to_vec()
            }));
        if let Some(bytes) = found {
            out.insert(id, bytes);
        }
    }
    world.insert_resource(PluginEffectSettings(out));
}

struct PluginEffect {
    id: String,
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    shader: Handle<Shader>,
    vertex: bevy::render::render_resource::VertexState,
    settings: ComponentId,
    /// Sized to the settings component, written every frame it is present.
    uniform: Buffer,
    pipelines: std::sync::Mutex<
        bevy::platform::collections::HashMap<TextureFormat, CachedRenderPipelineId>,
    >,
}

impl RenderPass for PluginEffect {
    fn run(
        &self,
        world: &World,
        render_context: &mut RenderContext,
        view_target: &ViewTarget,
        _view_entity: Entity,
    ) {
        // No settings on any entity = the effect is off. This is how a plugin
        // effect is enabled: put its component on something.
        // No settings on any entity = the effect is off. This is how a plugin
        // effect is enabled: put its component on something.
        let Some(settings) = world
            .get_resource::<PluginEffectSettings>()
            .and_then(|s| s.0.get(&self.settings))
        else {
            return;
        };

        let cache = world.resource::<PipelineCache>();
        let id = {
            let mut map = self.pipelines.lock().unwrap();
            let fmt = view_target.main_texture_format();
            *map.entry(fmt).or_insert_with(|| {
                cache.queue_render_pipeline(RenderPipelineDescriptor {
                    label: Some(format!("plugin_effect_{}", self.id).into()),
                    layout: vec![self.layout.clone()],
                    vertex: self.vertex.clone(),
                    fragment: Some(FragmentState {
                        shader: self.shader.clone(),
                        targets: vec![Some(ColorTargetState {
                            format: fmt,
                            blend: None,
                            write_mask: ColorWrites::ALL,
                        })],
                        ..default()
                    }),
                    ..default()
                })
            })
        };
        let Some(pipeline) = cache.get_render_pipeline(id) else {
            if let bevy::render::render_resource::CachedPipelineState::Err(e) =
                cache.get_render_pipeline_state(id)
            {
                bevy::log::error_once!("[plugin] effect `{}` pipeline FAILED: {e}", self.id);
            }
            return;
        };

        world
            .resource::<RenderQueue>()
            .write_buffer(&self.uniform, 0, settings);

        let post_process = view_target.post_process_write();
        let bind_group = render_context.render_device().create_bind_group(
            "plugin_effect_bind_group",
            &cache.get_bind_group_layout(&self.layout),
            &BindGroupEntries::sequential((
                post_process.source,
                &self.sampler,
                self.uniform.as_entire_binding(),
            )),
        );

        let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("plugin_effect_pass"),
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
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// Build a parameterised effect. Same shape as `build_pipeline` plus the uniform
/// binding at slot 2 and a buffer sized to the settings component.
fn build_effect(
    world: &mut World,
    e: &renzora_plugin::host::PendingPostProcess,
    wgsl: &str,
) -> Option<PluginEffect> {
    if let Err(why) = validate_effect_shader(wgsl, e.settings_size) {
        error!("[plugin] effect `{}` rejected: {why}", e.id);
        return None;
    }
    // `uniform_buffer_sized` rather than `uniform_buffer::<T>` — we know the
    // size but have no type, which is the whole situation.
    let layout = BindGroupLayoutDescriptor::new(
        "plugin_effect_bind_group_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                bevy::render::render_resource::binding_types::uniform_buffer_sized(
                    false,
                    std::num::NonZeroU64::new(e.settings_size.max(16)),
                ),
            ),
        ),
    );
    let device = world.get_resource::<RenderDevice>()?;
    let sampler = device.create_sampler(&SamplerDescriptor::default());
    let uniform = device.create_buffer(&BufferDescriptor {
        label: Some("plugin_effect_settings"),
        // Uniform buffers are 16-byte aligned; a smaller settings struct is legal
        // in Rust and illegal as a binding.
        size: e.settings_size.max(16).next_multiple_of(16),
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let vertex = world.get_resource::<FullscreenShader>()?.to_vertex_state();

    Some(PluginEffect {
        id: e.id.clone(),
        layout,
        sampler,
        shader: e.shader.clone(),
        vertex,
        settings: e.settings,
        uniform,
        pipelines: std::sync::Mutex::new(Default::default()),
    })
}

/// Check a plugin's shader against the settings component it declared.
///
/// This exists because the failure it catches is fatal. wgpu validates the
/// binding size when the pipeline is created, and this engine turns a device
/// validation error into an unrecoverable panic — so a third-party plugin whose
/// WGSL struct disagrees with its Rust struct takes the whole editor down before
/// anyone can read a log line.
///
/// The mismatch is easy to write by accident: WGSL aligns `vec3<f32>` to 16
/// bytes and Rust's `[f32; 3]` to 4, so the "same" padded struct is 32 bytes on
/// one side and 16 on the other.
///
/// Returns `Err` with a message meant for the plugin author, not for us.
fn validate_effect_shader(wgsl: &str, expected: u64) -> Result<(), String> {
    let module = naga::front::wgsl::parse_str(wgsl)
        .map_err(|e| format!("shader does not parse: {}", e.message()))?;
    let mut layouter = naga::proc::Layouter::default();
    layouter
        .update(module.to_ctx())
        .map_err(|e| format!("shader types could not be laid out: {e:?}"))?;

    let binding = naga::ResourceBinding { group: 0, binding: 2 };
    let Some((_, var)) = module
        .global_variables
        .iter()
        .find(|(_, v)| v.binding.as_ref() == Some(&binding))
    else {
        return Err(
            "shader declares no `@group(0) @binding(2)` uniform — an effect's settings \
             component is bound there"
                .to_string(),
        );
    };

    let actual = u64::from(layouter[var.ty].size);
    // The buffer is rounded up to a 16-byte multiple, so accept anything that
    // fits in the allocation rather than demanding an exact match.
    let allocated = expected.max(16).next_multiple_of(16);
    if actual > allocated {
        return Err(format!(
            "shader's uniform is {actual} bytes but the settings component is {expected}. \
             Most often this is `vec3<f32>` padding: WGSL aligns it to 16 bytes and Rust's \
             `[f32; 3]` to 4, so pad with scalar `f32`s on both sides instead."
        ));
    }
    Ok(())
}
