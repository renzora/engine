//! What a plugin registers that this crate cannot act on itself: render passes,
//! post-process effects, custom materials, editor panels, and the script /
//! audio / network backends.
//!
//! Everything here is *held*, not executed. `renzora_plugin` must stay
//! publishable to crates.io, so it cannot depend on `renzora_postprocess`,
//! the editor, `renzora_audio` or anything else in the engine — it carries
//! bytes and function pointers it does not interpret, and the owning crate
//! drains the queue on its own side.
//!
//! The two-world split is the other reason: a shader asset is created here
//! because `Assets<Shader>` lives in the main world, but the *pipeline* needs
//! `RenderDevice` and `PipelineCache`, which are render-world resources.

use bevy::ecs::component::ComponentId;
use bevy::prelude::*;
use bevy::render::render_phase::TrackedRenderPass;
use bevy::render::render_resource::{BindGroup, RenderPipeline};
use bevy::shader::Shader;

use crate::sys;

use super::assets::{MaterialSlot, PluginAssets};
use super::reload::{guard_host, HostCtx};

/// A render pass a plugin registered, waiting for the render world to build its
/// pipeline.
///
/// The shader asset is created here, in the main world, because `Assets<Shader>`
/// lives here. The *pipeline* cannot be — that needs `RenderDevice` and
/// `PipelineCache`, which are render-world resources — so the bridge drains this
/// queue on the other side. See `renzora_postprocess::plugin_passes`.
pub struct PendingRenderPass {
    pub id: String,
    pub shader: Handle<Shader>,
    /// The source the handle was built from, kept so a reload can tell whether the
    /// shader actually changed and swap it into the ALREADY-REGISTERED handle. The
    /// pass holds that handle inside a built pipeline, so replacing the asset is
    /// what makes the pipeline cache recompile — a fresh handle would be ignored.
    pub wgsl: String,
    pub phase: sys::RenderPhase,
    pub order: f32,
    pub callback: sys::RenderCallback,
    /// Registering plugin slot, so a reload replaces this rather than adding a
    /// second copy. See [`super::reload::retire_slot`].
    pub owner: usize,
}

/// Registered-but-not-yet-built plugin render passes.
#[derive(Resource, Default)]
pub struct PendingRenderPasses(pub Vec<PendingRenderPass>);

/// An editor panel a plugin registered.
///
/// Held rather than acted on, because `renzora_plugin` must not depend on the
/// editor — it has to stay publishable, and a plugin author's build should not
/// pull in `bevy_ui`. `renzora_inspector` drains this and does the work.
pub struct PluginPanel {
    pub id: String,
    pub title: String,
    pub icon: String,
    pub category: String,
    /// The markup, copied out of plugin memory at registration.
    pub markup: String,
    pub on_action: Option<sys::PanelActionEntry>,
    /// The plugin's opaque token, as `usize` so this stays `Send + Sync` — it is
    /// handed straight back and never dereferenced here.
    pub user: usize,
    /// Registering plugin slot — see [`super::reload::retire_slot`].
    pub owner: usize,
    /// Renders on the Settings overlay's Plugins tab rather than in the dock.
    ///
    /// A flag rather than a second registry because everything else about the
    /// two is identical — the id, the markup, the action thunk, and crucially
    /// the `set_panel_content` path that updates them. Splitting them would
    /// mean two of each, and the second copy would be the one nobody tests.
    pub settings: bool,
}

/// Every panel registered by every loaded plugin.
#[derive(Resource, Default)]
pub struct PluginPanels(pub Vec<PluginPanel>);

/// A scripting language a plugin registered.
///
/// The strings are copied out of plugin memory at registration, so this
/// outlives the descriptor the plugin passed — which may well have been a
/// stack local.
pub struct PluginScriptBackend {
    /// Human-readable, for logs and the language picker.
    pub name: String,
    /// Extensions claimed, lowercased and without the dot.
    pub extensions: Vec<String>,
    pub entry: sys::ScriptEntry,
    /// Registering plugin slot — see [`super::reload::retire_slot`].
    pub owner: usize,
}

/// Every scripting language registered by every loaded plugin.
///
/// `renzora_scripting` drains this into its own `ScriptEngine`, which is what
/// keeps this crate from needing to know what a script *is*. All it holds is a
/// name, some extensions and a function pointer.
#[derive(Resource, Default)]
pub struct PluginScriptBackends(pub Vec<PluginScriptBackend>);

/// The audio backend a plugin registered.
///
/// `state` is stored as a `usize` rather than a `*mut c_void` so the resource
/// stays `Send + Sync` without an unsafe impl. The host never dereferences it —
/// it is handed straight back to the plugin on every call — so the pointer's
/// only requirement is that it round-trips unchanged.
pub struct PluginAudioBackendEntry {
    /// Human-readable, for logs and the editor's audio settings.
    pub name: String,
    /// Opaque plugin state, passed back with every call.
    pub state: usize,
    pub entry: sys::AudioEntry,
    /// Registering plugin slot — see [`super::reload::retire_slot`].
    pub owner: usize,
}

/// The one audio backend, if a plugin registered one.
///
/// `Option` rather than a `Vec`, unlike [`PluginScriptBackends`]: two languages
/// coexist in one project because a script picks one by its file extension, and
/// there is no equivalent for audio — a second backend would open the same
/// output device and mix over the first.
///
/// `renzora_audio` drains this into its own engine, which is what keeps this
/// crate from needing to know what a sound *is*. All it holds is a name, an
/// opaque pointer and a function pointer.
#[derive(Resource, Default)]
pub struct PluginAudioBackend(pub Option<PluginAudioBackendEntry>);

/// The network backend a plugin registered.
///
/// `state` is stored as a `usize` for the reason [`PluginAudioBackendEntry`]
/// does it — the host never dereferences it, so the pointer's only requirement
/// is that it round-trips unchanged, and a `usize` keeps the resource
/// `Send + Sync` without an unsafe impl.
pub struct PluginNetBackendEntry {
    /// Human-readable, for logs and the editor's network settings.
    pub name: String,
    /// Opaque plugin state, passed back with every call.
    pub state: usize,
    pub entry: sys::NetEntry,
    /// Registering plugin slot — see [`super::reload::retire_slot`].
    pub owner: usize,
}

/// The one network backend, if a plugin registered one.
///
/// `renzora_net` drains this into its own client, which is what keeps this crate
/// from needing to know what a URL is. All it holds is a name, an opaque pointer
/// and a function pointer.
#[derive(Resource, Default)]
pub struct PluginNetBackend(pub Option<PluginNetBackendEntry>);

/// Turns BSN source into entities.
///
/// A function pointer rather than a direct call because this crate publishes to
/// crates.io and cannot take a path dependency on `renzora_bsn` — the same
/// reason the render bridge lives in `renzora_postprocess`. Something that
/// depends on both installs this; without it a `SpawnBsn` command is refused
/// with a message rather than silently doing nothing.
///
/// `root` is a reserved entity the spawner must use for the first tree in the
/// source, so the plugin's id is valid in the frame it asked for it.
#[derive(Resource, Clone, Copy)]
pub struct BsnSpawner(pub fn(&mut World, Entity, &str));

/// A parameterised effect a plugin registered.
///
/// Unlike [`PendingRenderPass`] the host owns the whole draw, so it needs the
/// settings component's id and size to build the bind group layout and to copy
/// the bytes out of the world each frame.
pub struct PendingPostProcess {
    pub id: String,
    pub shader: Handle<Shader>,
    /// Kept alongside the handle so the bridge can validate the shader against
    /// `settings_size` before wgpu sees it — a mismatch there is a fatal GPU
    /// error, not a recoverable one.
    pub wgsl: String,
    pub settings: ComponentId,
    pub settings_size: u64,
    pub phase: sys::RenderPhase,
    pub order: f32,
    /// Registering plugin slot — see [`super::reload::retire_slot`].
    pub owner: usize,
}

/// Registered-but-not-yet-built plugin effects.
#[derive(Resource, Default)]
pub struct PendingPostProcesses(pub Vec<PendingPostProcess>);

/// What `sys::RenderCtx` actually points at, for the duration of one callback.
///
/// Lifetimes are erased across the FFI boundary, which is sound only because the
/// plugin cannot retain the handle: `RenderCtx` is documented as valid solely
/// inside the callback that received it, and the borrow it wraps outlives that
/// call. A plugin that stashes one is misusing the API in exactly the way any C
/// API can be misused.
pub struct RenderCallCtx<'a, 'w> {
    pub pass: &'a mut TrackedRenderPass<'w>,
    pub pipeline: &'a RenderPipeline,
    pub bind_group: &'a BindGroup,
}

pub(crate) unsafe extern "C" fn add_render_pass(
    host: *mut sys::Host,
    desc: *const sys::RenderPassDesc,
) {
    guard_host("add_render_pass", (), || {
    let ctx = &mut *(host as *mut HostCtx);
    let desc = &*desc;
    let id = desc.id.as_str().to_string();

    // `Shader::from_wgsl` takes the source verbatim; a plugin has no AssetServer
    // and no path we could resolve, so the WGSL crosses the boundary as text.
    let shader = Shader::from_wgsl(desc.fragment_wgsl.as_str().to_string(), id.clone());
    let Some(mut shaders) = ctx.world.get_resource_mut::<Assets<Shader>>() else {
        // No renderer in this build (headless, server, or a test app on
        // MinimalPlugins). Skipping is correct — the plugin's systems still work.
        warn!("[plugin] render pass `{id}` ignored — this build has no renderer");
        return;
    };
    let handle = shaders.add(shader);

    ctx.world
        .get_resource_or_insert_with(PendingRenderPasses::default)
        .0
        .push(PendingRenderPass {
            owner: ctx.slot,
            id,
            shader: handle,
            wgsl: desc.fragment_wgsl.as_str().to_string(),
            phase: desc.phase,
            order: desc.order,
            callback: desc.callback,
        });
    })
}

pub(crate) unsafe extern "C" fn add_post_process(
    host: *mut sys::Host,
    desc: *const sys::PostProcessDesc,
) {
    guard_host("add_post_process", (), || {
        let ctx = &mut *(host as *mut HostCtx);
        let desc = &*desc;
        let id = desc.id.as_str().to_string();

        if !desc.settings.is_valid() {
            error!("[plugin] effect `{id}` has no settings component — refusing to register");
            return;
        }

        let shader = Shader::from_wgsl(desc.fragment_wgsl.as_str().to_string(), id.clone());
        let Some(mut shaders) = ctx.world.get_resource_mut::<Assets<Shader>>() else {
            warn!("[plugin] effect `{id}` ignored — this build has no renderer");
            return;
        };
        let handle = shaders.add(shader);

        ctx.world
            .get_resource_or_insert_with(PendingPostProcesses::default)
            .0
            .push(PendingPostProcess {
                owner: ctx.slot,
                id,
                shader: handle,
                wgsl: desc.fragment_wgsl.as_str().to_string(),
                settings: ComponentId::new(desc.settings.0 as usize),
                settings_size: desc.settings_size,
                phase: desc.phase,
                order: desc.order,
            });
    })
}

/// A custom shaded material a plugin registered, waiting for the render bridge.
pub struct PendingMaterial {
    pub id: String,
    pub shader: Handle<Shader>,
    /// Kept alongside the handle so the bridge can validate the shader's
    /// declared uniform against `settings_size` before wgpu sees it.
    pub wgsl: String,
    pub settings: ComponentId,
    pub settings_size: u64,
    pub alpha_mode: sys::AlphaMode,
    /// Images the material binds, already resolved to real handles.
    pub textures: Vec<Handle<Image>>,
    /// Index into `PluginAssets::materials`, so a spawn can name it like any
    /// other material handle.
    pub slot: usize,
    /// Registering plugin slot — see [`super::reload::retire_slot`].
    pub owner: usize,
}

/// Registered-but-not-yet-built plugin materials.
#[derive(Resource, Default)]
pub struct PendingMaterials(pub Vec<PendingMaterial>);

pub(crate) unsafe extern "C" fn add_material_shader(
    host: *mut sys::Host,
    desc: *const sys::MaterialShaderDesc,
) -> sys::AssetHandle {
    guard_host("add_material_shader", sys::AssetHandle::INVALID, || {
        let ctx = &mut *(host as *mut HostCtx);
        let desc = &*desc;
        let id = desc.id.as_str().to_string();

        if !desc.settings.is_valid() {
            error!("[plugin] material `{id}` has no settings component — refusing to register");
            return sys::AssetHandle::INVALID;
        }
        // The bind-group layout is decided once for the shared material type, so
        // a plugin cannot be given more room than it reserves. Refused rather
        // than truncated: a uniform read past its buffer is undefined on the GPU.
        if desc.settings_size > sys::MATERIAL_UNIFORM_CAP {
            error!(
                "[plugin] material `{id}` wants a {}-byte uniform; the cap is {}",
                desc.settings_size,
                sys::MATERIAL_UNIFORM_CAP
            );
            return sys::AssetHandle::INVALID;
        }
        if !desc.alpha_mode.is_known() {
            warn!(
                "[plugin] material `{id}` used alpha mode {}, which this build does not have",
                desc.alpha_mode.0
            );
            return sys::AssetHandle::INVALID;
        }

        if desc.texture_count > sys::MAX_MATERIAL_TEXTURES {
            error!(
                "[plugin] material `{id}` binds {} textures; the cap is {}",
                desc.texture_count,
                sys::MAX_MATERIAL_TEXTURES
            );
            return sys::AssetHandle::INVALID;
        }
        // Resolved now rather than stored as indices: the bridge builds the
        // asset later and would otherwise have to reach back into the slot
        // table, which a reload may have reordered.
        let mut textures = Vec::with_capacity(desc.texture_count);
        if desc.texture_count > 0 && !desc.textures.is_null() {
            let slots = std::slice::from_raw_parts(desc.textures, desc.texture_count);
            let store = ctx.world.get_resource::<PluginAssets>();
            for slot in slots {
                match store.and_then(|s| s.images.get(slot.0 as usize)).cloned() {
                    Some((_, h)) => textures.push(h),
                    None => {
                        error!(
                            "[plugin] material `{id}` names image slot {}, which was never created",
                            slot.0
                        );
                        return sys::AssetHandle::INVALID;
                    }
                }
            }
        }

        let shader = Shader::from_wgsl(desc.wgsl.as_str().to_string(), id.clone());
        let Some(mut shaders) = ctx.world.get_resource_mut::<Assets<Shader>>() else {
            warn!("[plugin] material `{id}` ignored — this build has no renderer");
            return sys::AssetHandle::INVALID;
        };
        let shader = shaders.add(shader);

        // A slot is reserved in the same store `add_material` uses, so a plugin
        // spawning a mesh does not have to know which kind of material it holds.
        // The bridge fills it in once it can build the real asset.
        let owner = ctx.slot;
        let slot = {
            let mut store = ctx.world.get_resource_or_insert_with(PluginAssets::default);
            store.materials.push((owner, MaterialSlot::Custom));
            store.materials.len() - 1
        };

        ctx.world
            .get_resource_or_insert_with(PendingMaterials::default)
            .0
            .push(PendingMaterial {
                owner,
                id,
                shader,
                wgsl: desc.wgsl.as_str().to_string(),
                settings: ComponentId::new(desc.settings.0 as usize),
                settings_size: desc.settings_size,
                alpha_mode: desc.alpha_mode,
                textures,
                slot,
            });
        sys::AssetHandle(slot as u64)
    })
}

pub(crate) unsafe extern "C" fn render_set_pipeline(
    ctx: sys::RenderCtx,
    _pipeline: sys::PipelineId,
) {
    if ctx.0.is_null() {
        error!("[plugin] render_set_pipeline called with a null ctx");
        return;
    }
    let c = &mut *(ctx.0 as *mut RenderCallCtx);
    c.pass.set_render_pipeline(c.pipeline);
    // Binding 0 is the view texture, 1 the sampler — the fullscreen contract the
    // pass descriptor documents. Bound here rather than exposed as a separate
    // call because this slice has exactly one bind group; a general API would
    // let the plugin build and bind its own.
    c.pass.set_bind_group(0, c.bind_group, &[]);
}

pub(crate) unsafe extern "C" fn render_draw(ctx: sys::RenderCtx, vertices: u32, instances: u32) {
    if ctx.0.is_null() {
        error!("[plugin] render_draw called with a null ctx");
        return;
    }
    let c = &mut *(ctx.0 as *mut RenderCallCtx);
    c.pass.draw(0..vertices, 0..instances);
}
