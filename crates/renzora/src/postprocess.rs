#![allow(dead_code)] // Public surface area kept for upcoming features.

//! Unified post-process pipeline that supports multiple simultaneous effects.
//!
//! Instead of adding a separate render graph node per effect, all effects are
//! processed by a single `UnifiedPostProcessNode`. Each effect registers a
//! type-erased handler that only executes when the camera has the corresponding
//! component. This means inactive effects have zero render graph overhead.

use core::marker::PhantomData;

// Bevy built-in post-process systems used as fixed ORDERING ANCHORS by the
// render-composition layer (see `RenderPhase`). The framework is the one place
// that imports these — individual effect crates never do; they just pick a phase.
use bevy::anti_alias::fxaa::fxaa;
use bevy::anti_alias::smaa::smaa;
use bevy::anti_alias::taa::temporal_anti_alias;
use bevy::core_pipeline::tonemapping::tonemapping;
use bevy::core_pipeline::{Core3d, Core3dSystems, FullscreenShader};
use bevy::prelude::*;
use bevy::render::{
    extract_component::{
        ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
        UniformComponentPlugin,
    },
    render_resource::{
        binding_types::{sampler, texture_2d, uniform_buffer},
        encase::internal::WriteInto,
        BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
        CachedRenderPipelineId, ColorTargetState, ColorWrites, Extent3d, FragmentState, Operations,
        PipelineCache, RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor,
        Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, ShaderType, TextureDescriptor,
        TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureView,
        TextureViewDescriptor,
    },
    renderer::{RenderContext, RenderDevice, ViewQuery},
    view::ViewTarget,
    Render, RenderApp, RenderStartup, RenderSystems,
};
use bevy::shader::ShaderRef;
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

    /// When `true`, the pipeline adds two extra binding slots (texture_2d + sampler)
    /// after the uniform buffer. The effect must populate `ExtraTextureSource<Self>`
    /// so the handler can bind the texture at render time.
    fn has_extra_texture() -> bool {
        false
    }

    /// When `true` (and `has_extra_texture()` is also `true`), the extra texture is
    /// an auto-captured snapshot of the *previous* fully-composited frame, maintained
    /// by the unified node. While idle the snapshot is refreshed every frame; while a
    /// transition is active (see [`freeze_snapshot`](Self::freeze_snapshot)) it is
    /// frozen, so the shader can blend the frozen outgoing frame (binding 3) against
    /// the live incoming frame (binding 0). Used by the screen-transition effect.
    fn extra_texture_is_snapshot() -> bool {
        false
    }

    /// Per-frame: return `true` to FREEZE the snapshot (stop refreshing it) — i.e. a
    /// transition is in progress. Return `false` when idle so the snapshot tracks the
    /// live frame. Only consulted when `extra_texture_is_snapshot()` is `true`.
    fn freeze_snapshot(&self) -> bool {
        false
    }

}

// ---------------------------------------------------------------------------
// Extra texture support (for effects like LUT-based color grading)
// ---------------------------------------------------------------------------

/// Main-world resource that holds an optional `Handle<Image>` for an extra
/// texture used by effect `T`. Set this from your plugin's systems.
#[derive(Resource)]
pub struct ExtraTextureSource<T: PostProcessEffect> {
    pub handle: Option<Handle<Image>>,
    _marker: PhantomData<T>,
}

impl<T: PostProcessEffect> Default for ExtraTextureSource<T> {
    fn default() -> Self {
        Self {
            handle: None,
            _marker: PhantomData,
        }
    }
}

/// Render-world resource holding the extracted `AssetId<Image>`.
#[derive(Resource)]
struct ExtractedExtraTexture<T: PostProcessEffect> {
    id: Option<AssetId<Image>>,
    _marker: PhantomData<T>,
}

impl<T: PostProcessEffect> Default for ExtractedExtraTexture<T> {
    fn default() -> Self {
        Self {
            id: None,
            _marker: PhantomData,
        }
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
    let layout = if T::has_extra_texture() {
        BindGroupLayoutDescriptor::new(
            "post_process_bind_group_layout_extra",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    uniform_buffer::<T>(true),
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                ),
            ),
        )
    } else {
        BindGroupLayoutDescriptor::new(
            "post_process_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    uniform_buffer::<T>(true),
                ),
            ),
        )
    };
    let sampler = render_device.create_sampler(&SamplerDescriptor::default());
    let shader = match T::fragment_shader() {
        ShaderRef::Default => {
            unimplemented!(
                "PostProcessEffect::fragment_shader() must not return ShaderRef::Default"
            )
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
                format: TextureFormat::Rgba8UnormSrgb, // 0.19: bevy_default() deprecated
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
        .format = TextureFormat::Rgba16Float; // 0.19: TEXTURE_FORMAT_HDR deprecated
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
// Snapshot support (for two-image transition effects)
// ---------------------------------------------------------------------------

/// Shared fullscreen "copy" pipeline used to blit the live frame into a
/// per-view snapshot texture. One instance, reused by every snapshot effect.
#[derive(Resource)]
struct SnapshotBlitPipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
}

fn init_snapshot_blit_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    fullscreen_shader: Res<FullscreenShader>,
    pipeline_cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "snapshot_blit_bind_group_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
            ),
        ),
    );
    let sampler = render_device.create_sampler(&SamplerDescriptor::default());
    let shader = asset_server.load("embedded://renzora/postprocess_copy.wgsl");
    let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("snapshot_blit_pipeline".into()),
        layout: vec![layout.clone()],
        vertex: fullscreen_shader.to_vertex_state(),
        fragment: Some(FragmentState {
            shader,
            // Snapshot textures are always Rgba16Float so a single blit
            // pipeline serves both LDR and HDR views.
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::Rgba16Float,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    });
    commands.insert_resource(SnapshotBlitPipeline {
        layout,
        sampler,
        pipeline_id,
    });
}

/// Per-view captured snapshot of the previous frame, used as the "frozen"
/// image (binding 3) by snapshot-based transition effects. Lives on the
/// render-world view entity, persisting across frames.
#[derive(Component)]
struct EffectSnapshot<T: PostProcessEffect> {
    view: TextureView,
    size: Extent3d,
    _marker: PhantomData<fn() -> T>,
}

/// Allocate / resize the snapshot texture for every view running effect `T`.
fn prepare_effect_snapshot<T: PostProcessEffect>(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    views: Query<(Entity, &ViewTarget, Option<&EffectSnapshot<T>>), With<T>>,
) {
    for (entity, view_target, existing) in &views {
        let size = view_target.main_texture().size();
        let needs_alloc = match existing {
            Some(s) => s.size != size,
            None => true,
        };
        if !needs_alloc {
            continue;
        }
        let texture = render_device.create_texture(&TextureDescriptor {
            label: Some("effect_snapshot"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba16Float,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor::default());
        commands.entity(entity).insert(EffectSnapshot::<T> {
            view,
            size,
            _marker: PhantomData,
        });
    }
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
    ) -> Result<(), ()>;
}

/// A custom render pass a plugin can slot into the render-composition pipeline.
///
/// Implement this on a (usually unit) struct, then register it with
/// [`RenderCompositionAppExt::add_render_pass`] into a [`RenderPhase`]. `run` is
/// called once per view, in phase + `order` sequence, from the render world:
/// - read your pipeline / bind-group / extracted-data **resources** from `world`,
/// - record GPU work on `render_context` (its command encoder is auto-submitted),
/// - read/write the camera color via `view_target` (`post_process_write()` gives
///   the source+destination ping-pong, exactly like the built-in effects),
/// - self-skip views that don't carry your per-entity component (check `world`/
///   `view_entity`) so an inactive pass costs nothing.
///
/// For a simple full-screen *fragment* effect prefer `PostProcessPlugin<T>`
/// (define a `PostProcessEffect` with a shader); this trait is for full control
/// (compute passes, custom attachments, multi-draw, …).
pub trait RenderPass: Send + Sync + 'static {
    /// Record this pass for one view. See [`RenderPass`] for the contract.
    fn run(
        &self,
        world: &World,
        render_context: &mut RenderContext,
        view_target: &ViewTarget,
        view_entity: Entity,
    );
}

/// Bridges a public [`RenderPass`] into the internal `EffectHandler` the registry
/// stores (a distinct newtype, so there's no blanket-impl coherence conflict with
/// `TypedEffectHandler`).
struct RenderPassAdapter<P: RenderPass>(P);

impl<P: RenderPass> EffectHandler for RenderPassAdapter<P> {
    fn execute(
        &self,
        world: &World,
        render_context: &mut RenderContext,
        view_target: &ViewTarget,
        view_entity: Entity,
    ) -> Result<(), ()> {
        self.0.run(world, render_context, view_target, view_entity);
        Ok(())
    }
}

struct TypedEffectHandler<T: PostProcessEffect>(PhantomData<T>);

impl<T: PostProcessEffect> EffectHandler for TypedEffectHandler<T> {
    fn execute(
        &self,
        world: &World,
        render_context: &mut RenderContext,
        view_target: &ViewTarget,
        view_entity: Entity,
    ) -> Result<(), ()> {
        // Skip if this view doesn't actually carry the effect component. We gate
        // on `T` ITSELF — not just `DynamicUniformIndex<T>` — because Bevy 0.19's
        // retained render world keeps the uniform index (and the per-effect
        // `ComponentUniforms<T>` GPU buffer) alive after the component has been
        // extracted away. Gating only on the stale index kept this pass running
        // with frozen uniforms after the user removed the effect: a "ghost" of
        // the dead effect composited over the live frame, flickering as the stale
        // buffer's binding came and went. The extracted `T` *is* cleared when the
        // source is gone (verified: render-world count drops to 0), so it's the
        // reliable presence signal.
        if world.get::<T>(view_entity).is_none() {
            return Ok(());
        }
        let Some(settings_index) = world.get::<DynamicUniformIndex<T>>(view_entity) else {
            return Ok(());
        };

        let Some(pipeline) = world.get_resource::<PostProcessPipeline<T>>() else {
            return Ok(());
        };
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline_id = if view_target.main_texture_format() == TextureFormat::Rgba16Float {
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

        // ── Two-image (transition) effects: bind a frozen snapshot as the
        //    extra texture and, while idle, refresh that snapshot from the
        //    live frame via a blit. ────────────────────────────────────────
        if T::has_extra_texture() {
            let snapshot = world.get::<EffectSnapshot<T>>(view_entity);
            let freeze = world
                .get::<T>(view_entity)
                .map(|s| s.freeze_snapshot())
                .unwrap_or(false);

            // Idle: copy the current frame into the snapshot so it always holds
            // the *previous* frame at the moment a transition begins. Skipped
            // while frozen so the outgoing shot is preserved during the blend.
            if T::extra_texture_is_snapshot() && !freeze {
                if let (Some(snap), Some(blit)) =
                    (snapshot, world.get_resource::<SnapshotBlitPipeline>())
                {
                    if let Some(blit_pipeline) =
                        pipeline_cache.get_render_pipeline(blit.pipeline_id)
                    {
                        let blit_bind_group = render_context.render_device().create_bind_group(
                            "snapshot_blit_bind_group",
                            &pipeline_cache.get_bind_group_layout(&blit.layout),
                            &BindGroupEntries::sequential((post_process.source, &blit.sampler)),
                        );
                        let mut blit_pass =
                            render_context.begin_tracked_render_pass(RenderPassDescriptor {
                                label: Some("snapshot_blit_pass"),
                                color_attachments: &[Some(RenderPassColorAttachment {
                                    view: &snap.view,
                                    depth_slice: None,
                                    resolve_target: None,
                                    ops: Operations::default(),
                                })],
                                depth_stencil_attachment: None,
                                timestamp_writes: None,
                                occlusion_query_set: None,
                multiview_mask: None,
                            });
                        blit_pass.set_render_pipeline(blit_pipeline);
                        blit_pass.set_bind_group(0, &blit_bind_group, &[]);
                        blit_pass.draw(0..3, 0..1);
                    }
                }
            }

            // Extra texture = the snapshot when available, otherwise fall back to
            // the live source (A == B → the shader degrades to a no-op pass-through
            // rather than sampling an uninitialized/blank texture).
            let extra_view: &TextureView =
                snapshot.map(|s| &s.view).unwrap_or(post_process.source);

            let bind_group = render_context.render_device().create_bind_group(
                "post_process_bind_group_extra",
                &pipeline_cache.get_bind_group_layout(&pipeline.layout),
                &BindGroupEntries::sequential((
                    post_process.source,
                    &pipeline.sampler,
                    settings_binding.clone(),
                    extra_view,
                    &pipeline.sampler,
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
                multiview_mask: None,
            });
            render_pass.set_render_pipeline(render_pipeline);
            render_pass.set_bind_group(0, &bind_group, &[settings_index.index()]);
            render_pass.draw(0..3, 0..1);

            return Ok(());
        }

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
            multiview_mask: None,
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

pub use crate::RenderPhase;

/// One registered render pass — the DATA the render-composition layer orders.
/// `id`/`phase`/`order`/`enabled` are public so a future render-pipeline node
/// editor can read and rewrite them at runtime; `handler` is the type-erased
/// executor (a `PostProcessEffect`'s pass, or any custom pass).
pub struct RenderPassEntry {
    /// Stable identifier (for the editor + reorder); e.g. `"effect.vignette"`.
    pub id: &'static str,
    /// Which ordering phase this pass runs in.
    pub phase: RenderPhase,
    /// Sort key within the phase — lower runs first. User-editable later.
    pub order: f32,
    /// Global on/off (per-ENTITY gating stays component-driven in the handler).
    pub enabled: bool,
    handler: Box<dyn EffectHandler>,
}

/// The central, data-driven render-pass registry (render world). Passes are kept
/// sorted by `(phase, order)`; the per-phase dispatcher systems iterate it.
/// Replaces the old flat `PostProcessRegistry` — same one-system-per-phase model
/// the unified post-process already used, generalized to every phase so GI,
/// reflections, and post-process effects all compose in one defined order.
#[derive(Resource, Default)]
pub struct RenderComposition {
    passes: Vec<RenderPassEntry>,
}

impl RenderComposition {
    /// Register a pre-built entry, keeping the list ordered by `(phase, order)`.
    pub fn add(&mut self, entry: RenderPassEntry) {
        self.passes.push(entry);
        self.passes
            .sort_by(|a, b| a.phase.cmp(&b.phase).then(a.order.total_cmp(&b.order)));
    }

    /// Register a custom [`RenderPass`] into a phase. The usual entry point for
    /// plugin authors (via [`RenderCompositionAppExt::add_render_pass`]).
    pub fn add_pass(
        &mut self,
        id: &'static str,
        phase: RenderPhase,
        order: f32,
        pass: impl RenderPass,
    ) {
        self.add(RenderPassEntry {
            id,
            phase,
            order,
            enabled: true,
            handler: Box::new(RenderPassAdapter(pass)),
        });
    }

    /// Read-only view of the registered passes (for an editor / inspector).
    pub fn passes(&self) -> &[RenderPassEntry] {
        &self.passes
    }
}

/// Render-app extension for registering a custom [`RenderPass`] into the
/// composition pipeline. Call on the render sub-app:
///
/// ```ignore
/// app.sub_app_mut(RenderApp)
///     .add_systems(RenderStartup, init_my_pipeline)   // your GPU resources
///     .add_render_pass("myplugin.glow", RenderPhase::HdrPost, 0.0, MyGlowPass);
/// ```
pub trait RenderCompositionAppExt {
    /// Slot `pass` into `phase` (lower `order` runs first within the phase).
    fn add_render_pass(
        &mut self,
        id: &'static str,
        phase: RenderPhase,
        order: f32,
        pass: impl RenderPass,
    ) -> &mut Self;
}

impl RenderCompositionAppExt for SubApp {
    fn add_render_pass(
        &mut self,
        id: &'static str,
        phase: RenderPhase,
        order: f32,
        pass: impl RenderPass,
    ) -> &mut Self {
        // The dispatcher + phase anchoring are installed by `PostProcessCorePlugin`
        // (present whenever any post-process effect is registered). Ensure the
        // registry exists so a pass can be added before that plugin's build runs.
        self.world_mut()
            .get_resource_or_insert_with(RenderComposition::default)
            .add_pass(id, phase, order, pass);
        self
    }
}

/// Run every enabled pass registered to `phase`, in order, for one view. Each
/// handler self-skips views without its per-entity component, so an inactive
/// effect costs nothing. Encoded passes are auto-submitted when the system ends.
fn run_phase(
    world: &World,
    view: ViewQuery<&'static ViewTarget>,
    render_context: &mut RenderContext,
    phase: RenderPhase,
) {
    let entity = view.entity();
    let view_target = view.into_inner();
    let composition = world.resource::<RenderComposition>();
    for entry in composition.passes.iter() {
        if entry.phase == phase && entry.enabled {
            let _ = entry
                .handler
                .execute(world, render_context, view_target, entity);
        }
    }
}

// One dispatcher per phase — each runs `.in_set(RenderPhase::X)`, so it inherits
// that phase's position relative to the bevy anchors (configured once below).
fn dispatch_gi(world: &World, view: ViewQuery<&'static ViewTarget>, mut ctx: RenderContext) {
    run_phase(world, view, &mut ctx, RenderPhase::Gi);
}
fn dispatch_hdr_post(world: &World, view: ViewQuery<&'static ViewTarget>, mut ctx: RenderContext) {
    run_phase(world, view, &mut ctx, RenderPhase::HdrPost);
}
fn dispatch_ldr_post(world: &World, view: ViewQuery<&'static ViewTarget>, mut ctx: RenderContext) {
    run_phase(world, view, &mut ctx, RenderPhase::LdrPost);
}
fn dispatch_overlay(world: &World, view: ViewQuery<&'static ViewTarget>, mut ctx: RenderContext) {
    run_phase(world, view, &mut ctx, RenderPhase::Overlay);
}

// ---------------------------------------------------------------------------
// Core plugin (added once, sets up the render-composition pipeline)
// ---------------------------------------------------------------------------

struct PostProcessCorePlugin;

impl Plugin for PostProcessCorePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] PostProcessCorePlugin");
        // Passthrough shader used to blit the live frame into snapshot textures.
        bevy::asset::embedded_asset!(app, "postprocess_copy.wgsl");

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.init_resource::<RenderComposition>();
        render_app.add_systems(RenderStartup, init_snapshot_blit_pipeline);

        // The canonical pipeline, configured in ONE place: position renzora's
        // phases around bevy's built-in post-process anchors. This is the only
        // spot that imports `temporal_anti_alias` / `tonemapping`; every effect
        // crate just picks a `RenderPhase`. (Ordering against an anchor that
        // isn't in the schedule — e.g. TAA plugin absent — is a harmless no-op.)
        use Core3dSystems::{EarlyPostProcess, PostProcess};
        render_app
            .configure_sets(
                Core3d,
                RenderPhase::Gi
                    .in_set(EarlyPostProcess)
                    .before(temporal_anti_alias),
            )
            .configure_sets(
                Core3d,
                RenderPhase::HdrPost
                    .in_set(EarlyPostProcess)
                    .after(temporal_anti_alias),
            )
            .configure_sets(
                Core3d,
                RenderPhase::LdrPost
                    .in_set(PostProcess)
                    .after(tonemapping)
                    .before(fxaa)
                    .before(smaa),
            )
            .configure_sets(
                Core3d,
                RenderPhase::Overlay
                    .in_set(PostProcess)
                    .after(fxaa)
                    .after(smaa)
                    .after(RenderPhase::LdrPost),
            );

        // The per-phase dispatchers. The existing post-process effects register
        // into `LdrPost` (unchanged: they still compose on the tonemapped frame).
        render_app.add_systems(
            Core3d,
            (
                dispatch_gi.in_set(RenderPhase::Gi),
                dispatch_hdr_post.in_set(RenderPhase::HdrPost),
                dispatch_ldr_post.in_set(RenderPhase::LdrPost),
                dispatch_overlay.in_set(RenderPhase::Overlay),
            ),
        );
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
///
/// Change-gated: a full re-proxy only runs when something relevant changed —
/// the routing table was rebuilt, this effect's settings changed on a source,
/// or a source's component was removed. In steady state it early-outs instead
/// of re-scanning routes × sources every frame. (`sources` uses `Ref<T>` so we
/// can see per-source change ticks; the query already only matches the few
/// entities that carry `T`, so the change scan is cheap.)
fn proxy_effect_to_camera<T: PostProcessEffect>(
    mut commands: Commands,
    sources: Query<(Entity, Ref<T>)>,
    routing: Res<crate::EffectRouting>,
    mut removed: RemovedComponents<T>,
) {
    let any_removed = removed.read().next().is_some();
    let routing_changed = routing.is_changed();
    let any_changed = sources.iter().any(|(_, s)| s.is_changed());
    if !routing_changed && !any_changed && !any_removed {
        return;
    }
    for (target, source_list) in routing.iter() {
        let mut found = false;
        for &src in source_list {
            if let Ok((_, settings)) = sources.get(src) {
                // `try_insert`, not `insert`: a routed camera can be despawned
                // the SAME frame its route is still in the table (closing the
                // camera-preview panel despawns its camera, then this queued
                // command flushes at apply_deferred). A `get_entity` guard
                // doesn't help — the entity is alive at queue time and only
                // despawned by the time the command applies. `try_insert`
                // no-ops on a despawned entity instead of panicking.
                commands.entity(*target).try_insert(*settings);
                found = true;
                break;
            }
        }
        if !found && (routing_changed || any_removed) {
            // try_remove (deferred-safe, like try_insert above) — the target
            // may be despawned by the time this command flushes.
            commands.entity(*target).try_remove::<T>();
        }
    }
}

/// Removes the proxied component from all routed cameras when all sources are removed.
fn cleanup_proxy_effect<T: PostProcessEffect>(
    mut commands: Commands,
    sources: Query<(), With<T>>,
    routing: Res<crate::EffectRouting>,
) {
    if sources.is_empty() {
        for (target, _) in routing.iter() {
            // try_remove (deferred-safe) — a routed camera may be despawned by
            // the time this flushes (e.g. closing the camera-preview panel).
            commands.entity(*target).try_remove::<T>();
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
        app.add_systems(
            Update,
            (proxy_effect_to_camera::<T>, cleanup_proxy_effect::<T>),
        );

        // Snapshot-backed effects need a per-view capture texture maintained
        // each frame so the handler can blend the frozen previous frame.
        if T::has_extra_texture() && T::extra_texture_is_snapshot() {
            if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
                render_app.add_systems(
                    Render,
                    prepare_effect_snapshot::<T>.in_set(RenderSystems::PrepareResources),
                );
            }
        }

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

        // Register this effect as an `LdrPost` pass (post-tonemapping), the same
        // stage the unified post-process always ran in — behavior unchanged.
        render_app
            .world_mut()
            .resource_mut::<RenderComposition>()
            .add(RenderPassEntry {
                id: core::any::type_name::<T>(),
                phase: RenderPhase::LdrPost,
                order: 0.0,
                enabled: true,
                handler: Box::new(TypedEffectHandler::<T>(PhantomData)),
            });
    }
}
