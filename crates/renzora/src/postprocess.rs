//! Render phases: the ordered slots a render pass registers into.
//!
//! A pass names a [`RenderPhase`] and an `order`, and the dispatcher for that
//! phase runs every registered pass in turn, per view. The phases are anchored
//! once, here, to Bevy's own temporal AA, tonemapping and FXAA/SMAA, so no
//! pass ever orders itself against those systems directly.
//!
//! The shader-file effects of the Post Process component are one pass in
//! [`RenderPhase::LdrPost`] (`renzora_postprocess`); global illumination and
//! installed plugins add their own.

// Bevy built-in post-process systems used as fixed ORDERING ANCHORS by the
// render-composition layer (see `RenderPhase`). The framework is the one place
// that imports these: passes never do; they just pick a phase.
// Gated on `antialiasing`, not `render_3d`: `bevy_anti_alias` used to arrive
// only via bevy's `3d` meta, but the root manifest now names it directly so the
// exporter can strip anti-aliasing on its own. These anchors only order
// renzora's phases against TAA/FXAA/SMAA; without them the ordering is a
// harmless no-op (the anchor isn't in the schedule anyway).
#[cfg(feature = "antialiasing")]
use bevy::anti_alias::fxaa::fxaa;
#[cfg(feature = "antialiasing")]
use bevy::anti_alias::smaa::smaa;
#[cfg(feature = "antialiasing")]
use bevy::anti_alias::taa::temporal_anti_alias;
use bevy::core_pipeline::tonemapping::tonemapping;
use bevy::core_pipeline::{Core3d, Core3dSystems};
use bevy::prelude::*;
use bevy::render::{
    renderer::{RenderContext, ViewQuery},
    view::ViewTarget,
    RenderApp,
};

/// Executes one registered pass for one view.
trait EffectHandler: Send + Sync + 'static {
    fn execute(
        &self,
        world: &World,
        render_context: &mut RenderContext,
        view_target: &ViewTarget,
        view_entity: Entity,
    );
}

/// A custom render pass a plugin can slot into the render-composition pipeline.
///
/// Implement this on a (usually unit) struct, then register it with
/// [`RenderCompositionAppExt::add_render_pass`] into a [`RenderPhase`]. `run` is
/// called once per view, in phase + `order` sequence, from the render world:
/// - read your pipeline / bind-group / extracted-data **resources** from `world`,
/// - record GPU work on `render_context` (its command encoder is auto-submitted),
/// - read/write the camera color via `view_target` (`post_process_write()` gives
///   the source+destination ping-pong),
/// - self-skip views that don't carry your per-entity component (check `world`/
///   `view_entity`) so an inactive pass costs nothing.
///
/// A single fullscreen fragment effect does not need one of these: write it as
/// an effect shader file and add it to a Post Process component.
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

/// Bridges a public [`RenderPass`] into the internal `EffectHandler` the
/// registry stores.
struct RenderPassAdapter<P: RenderPass>(P);

impl<P: RenderPass> EffectHandler for RenderPassAdapter<P> {
    fn execute(
        &self,
        world: &World,
        render_context: &mut RenderContext,
        view_target: &ViewTarget,
        view_entity: Entity,
    ) {
        self.0.run(world, render_context, view_target, view_entity);
    }
}

pub use crate::RenderPhase;

/// One registered render pass: the data the render-composition layer orders.
/// `id`/`phase`/`order`/`enabled` are public so a future render-pipeline node
/// editor can read and rewrite them at runtime; `handler` is the type-erased
/// executor.
pub struct RenderPassEntry {
    /// Stable identifier (for the editor + reorder); e.g. `"post_effects"`.
    pub id: &'static str,
    /// Which ordering phase this pass runs in.
    pub phase: RenderPhase,
    /// Sort key within the phase: lower runs first.
    pub order: f32,
    /// Global on/off (per-ENTITY gating stays component-driven in the pass).
    pub enabled: bool,
    handler: Box<dyn EffectHandler>,
}

/// The central, data-driven render-pass registry (render world). Passes are kept
/// sorted by `(phase, order)`; the per-phase dispatcher systems iterate it, so
/// GI, reflections and post-process effects all compose in one defined order.
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
        // The dispatcher + phase anchoring are installed by `PostProcessCorePlugin`.
        // Ensure the registry exists so a pass can be added before that plugin's
        // build runs.
        self.world_mut()
            .get_resource_or_insert_with(RenderComposition::default)
            .add_pass(id, phase, order, pass);
        self
    }
}

/// Run every enabled pass registered to `phase`, in order, for one view. Each
/// pass self-skips views without its per-entity component, so an inactive one
/// costs nothing. Encoded passes are auto-submitted when the system ends.
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
            entry
                .handler
                .execute(world, render_context, view_target, entity);
        }
    }
}

// One dispatcher per phase: each runs `.in_set(RenderPhase::X)`, so it inherits
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

/// Installs the render-composition dispatchers and the phase anchoring.
///
/// **Add this if you register passes**, or nothing dispatches them: a pass
/// registered into `RenderComposition` with no dispatcher installed silently
/// does nothing, because registration and dispatch are wired up in two
/// different places.
///
/// Also embeds `postprocess_copy.wesl`, the pass-through shader a pass uses to
/// copy the live frame into a texture of its own.
///
/// Idempotent: adding it twice is a no-op via `is_plugin_added`.
pub struct PostProcessCorePlugin;

impl Plugin for PostProcessCorePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] PostProcessCorePlugin");
        bevy::asset::embedded_asset!(app, "postprocess_copy.wesl");

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.init_resource::<RenderComposition>();

        // The canonical pipeline, configured in ONE place: position renzora's
        // phases around bevy's built-in post-process anchors. This is the only
        // spot that imports `temporal_anti_alias` / `tonemapping`; every pass
        // just picks a `RenderPhase`. (Ordering against an anchor that isn't in
        // the schedule, e.g. TAA plugin absent, is a harmless no-op.)
        use Core3dSystems::{EarlyPostProcess, PostProcess};
        // The TAA/FXAA/SMAA anchors need `bevy_anti_alias`, which the exporter's
        // Anti-aliasing capability can strip on its own. Build each phase set,
        // conditionally add the anti-alias ordering, then configure; `tonemapping`
        // + the inter-phase ordering always apply.
        let gi = RenderPhase::Gi.in_set(EarlyPostProcess);
        #[cfg(feature = "antialiasing")]
        let gi = gi.before(temporal_anti_alias);

        let hdr_post = RenderPhase::HdrPost.in_set(EarlyPostProcess);
        #[cfg(feature = "antialiasing")]
        let hdr_post = hdr_post.after(temporal_anti_alias);

        let ldr_post = RenderPhase::LdrPost.in_set(PostProcess).after(tonemapping);
        #[cfg(feature = "antialiasing")]
        let ldr_post = ldr_post.before(fxaa).before(smaa);

        let overlay = RenderPhase::Overlay.in_set(PostProcess);
        #[cfg(feature = "antialiasing")]
        let overlay = overlay.after(fxaa).after(smaa);
        let overlay = overlay.after(RenderPhase::LdrPost);

        render_app
            .configure_sets(Core3d, gi)
            .configure_sets(Core3d, hdr_post)
            .configure_sets(Core3d, ldr_post)
            .configure_sets(Core3d, overlay);

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
