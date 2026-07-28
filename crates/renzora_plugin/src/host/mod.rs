//! Host side of the `renzora_plugin` C ABI.
//!
//! Counterpart to `renzora_plugin::sys`. Builds the function table, hands it to
//! a plugin's `renzora_plugin_init`, and turns whatever the plugin registers
//! into real Bevy components and systems.
//!
//! ## Why this is a separate path from `dynamic_plugin_loader`
//!
//! That crate loads `dylib` plugins which share `bevy_dylib` with the host and
//! get `&mut App` directly. It requires the plugin to have been compiled in the
//! same environment as the engine — same rustc, same Bevy feature set, same
//! `bevy_dylib-<hash>` — which is why third-party prebuilts are so painful.
//!
//! This path has no such requirement: the plugin links nothing, exports one
//! symbol, and receives every capability through a `#[repr(C)]` function table.
//! The two mechanisms coexist; in-tree engine plugins keep using the old one.
//!
//! ## The interesting part: dynamic systems that still run in parallel
//!
//! A plugin declares its query up front (`sys::QueryDesc`). We turn that into a
//! real Bevy query with `QueryParamBuilder`, so the resulting system carries
//! proper component access and the multi-threaded executor can schedule it
//! against everything else. The alternative — giving plugins open-ended `&mut
//! World` access — would force every plugin system to be exclusive and
//! serialise the whole schedule.

pub mod loader;

use bevy::ecs::component::{ComponentDescriptor, ComponentId, StorageType};
use bevy::ecs::query::QueryBuilder;
use bevy::ecs::schedule::{ScheduleLabel, Schedules};
use bevy::ecs::system::{ParamBuilder, QueryParamBuilder, SystemParamBuilder};
use bevy::ecs::world::{FilteredEntityMut, FilteredEntityRef};
use bevy::prelude::*;
use bevy::render::render_phase::TrackedRenderPass;
use bevy::render::render_resource::{BindGroup, RenderPipeline};
use bevy::shader::Shader;
use crate::sys;
use std::alloc::Layout;
use std::ffi::c_void;

/// What the opaque `sys::Host` pointer actually points at.
///
/// Only valid for the duration of one `renzora_plugin_init` call — we hand the
/// plugin a pointer to a stack value and it may only call back while that frame
/// is live. A plugin that squirrels the pointer away and calls later is using
/// the API wrong; there is nothing we can do to stop it, exactly as with any C
/// API.
struct HostCtx<'w> {
    world: &'w mut World,
}

/// How one query term crosses the boundary.
///
/// The distinction exists because the host's own types have no layout guarantee.
/// See `renzora_plugin::sys` — `bevy::Transform` is not `#[repr(C)]` and
/// `glam::Quat` changes representation per SIMD backend, so we cannot hand out a
/// pointer and let the plugin cast.
#[derive(Clone, Copy, PartialEq)]
enum Marshal {
    /// Copy the component's bytes verbatim. Correct for plugin-owned components,
    /// whose layout the plugin itself defined.
    Raw,
    /// Convert to and from `sys::Transform`.
    Transform,
}

#[derive(Clone)]
struct TermPlan {
    id: ComponentId,
    access: sys::Access,
    marshal: Marshal,
    /// Size of one cell *as the plugin sees it*, which for a mirrored term is
    /// the mirror's size, not the host type's.
    cell_size: usize,
}

/// A `'static` copy of the table, so a running system can be handed a pointer to
/// it that outlives the frame. Safe to share because every field is a plain `fn`
/// item — there is no state to race on.
static IFACE: sys::Interface = sys::Interface {
    version_major: sys::VERSION_MAJOR,
    version_minor: sys::VERSION_MINOR,
    register_component,
    component_id_by_name,
    add_system,
    add_render_pass,
    render_set_pipeline,
    render_draw,
    log,
};

/// Build the interface table. The function pointers are plain `extern "C"` items
/// with no captured state — everything they need arrives through `host`.
fn interface() -> sys::Interface {
    sys::Interface {
        version_major: sys::VERSION_MAJOR,
        version_minor: sys::VERSION_MINOR,
        register_component,
        component_id_by_name,
        add_system,
        add_render_pass,
        render_set_pipeline,
        render_draw,
        log,
    }
}

// ── Interface implementations ────────────────────────────────────────────────

unsafe extern "C" fn register_component(
    host: *mut sys::Host,
    desc: *const sys::ComponentDesc,
) -> sys::ComponentId {
    let ctx = &mut *(host as *mut HostCtx);
    let desc = &*desc;
    let name = desc.name.as_str().to_string();

    // Re-registering the same name must return the same id: a plugin reloaded
    // mid-session would otherwise get a second component and silently stop
    // matching the entities carrying the first.
    if let Some(existing) = lookup_component(ctx.world, &name) {
        return sys::ComponentId(existing.index() as u32);
    }

    let Ok(layout) = Layout::from_size_align(desc.size, desc.align) else {
        error!("plugin component `{name}` has an invalid layout ({} / {})", desc.size, desc.align);
        return sys::ComponentId::INVALID;
    };

    // SAFETY: the plugin supplied the layout for its own type, and `drop` (if
    // any) is that type's destructor. We never construct one ourselves — the
    // plugin writes into storage we allocate to this layout.
    let descriptor = unsafe {
        ComponentDescriptor::new_with_layout(
            name,
            StorageType::Table,
            layout.pad_to_align(),
            desc.drop.map(|f| {
                // The plugin's drop takes a plain `*mut u8`; Bevy hands an
                // `OwningPtr`. The shim is why `drop` can stay a boring C
                // signature the plugin can write without knowing Bevy exists.
                let _ = f;
                unimplemented!("component destructors are not supported yet — keep plugin components POD")
            }),
            true,
            bevy::ecs::component::ComponentCloneBehavior::Default,
            None,
        )
    };

    let id = ctx.world.register_component_with_descriptor(descriptor);
    let type_path = desc.name.as_str().to_string();
    ctx.world
        .get_resource_or_insert_with(PluginComponents::default)
        .0
        .insert(type_path.clone(), id);

    // Copy the schema out of the plugin's memory now, while we know it is valid.
    let fields = if desc.fields.is_null() {
        Vec::new()
    } else {
        std::slice::from_raw_parts(desc.fields, desc.field_count)
            .iter()
            .map(|f| PluginField {
                name: f.name.as_str().to_string(),
                kind: f.kind,
                offset: f.offset,
            })
            .collect()
    };
    let default_value = match desc.default_init {
        Some(init) => {
            let mut buf = vec![0u8; desc.size];
            init(buf.as_mut_ptr());
            buf
        }
        None => Vec::new(),
    };
    let display_name = {
        let d = desc.display_name.as_str();
        if d.is_empty() {
            type_path.rsplit("::").next().unwrap_or(&type_path).to_string()
        } else {
            d.to_string()
        }
    };
    ctx.world
        .get_resource_or_insert_with(PluginComponentSchemas::default)
        .0
        .push(PluginComponentInfo {
            id,
            type_path,
            display_name,
            fields,
            size: desc.size,
            default_value,
        });

    sys::ComponentId(id.index() as u32)
}

unsafe extern "C" fn component_id_by_name(
    host: *mut sys::Host,
    name: sys::StrRef,
) -> sys::ComponentId {
    let ctx = &mut *(host as *mut HostCtx);
    let name = name.as_str();
    match lookup_component(ctx.world, name) {
        Some(id) => sys::ComponentId(id.index() as u32),
        None => {
            // Only an error on THIS path. `register_component` uses the same
            // lookup to dedup, where "not found" is the normal case for a first
            // registration — logging there produced a scary error during a
            // perfectly successful load.
            error!(
                "plugin asked for host component `{name}`, which this build does not expose.                  It must be registered for reflection AND listed in                  `loader::register_exposed_components`."
            );
            sys::ComponentId::INVALID
        }
    }
}

unsafe extern "C" fn add_system(
    host: *mut sys::Host,
    schedule: sys::Schedule,
    entry: sys::SystemEntry,
    query: *const sys::QueryDesc,
    user: *mut c_void,
) {
    let ctx = &mut *(host as *mut HostCtx);
    let query = &*query;
    let terms = std::slice::from_raw_parts(query.terms, query.term_count);

    let Some(plan) = build_plan(ctx.world, terms) else {
        return;
    };
    let system = build_dispatcher(ctx.world, plan, entry, user as usize, host as usize);
    ctx.world
        .resource_mut::<Schedules>()
        .entry(bevy_label(schedule))
        .add_systems(system);
}

// ── Rendering ────────────────────────────────────────────────────────────────

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
    pub phase: sys::RenderPhase,
    pub order: f32,
    pub callback: sys::RenderCallback,
}

/// Registered-but-not-yet-built plugin render passes.
#[derive(Resource, Default)]
pub struct PendingRenderPasses(pub Vec<PendingRenderPass>);

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

unsafe extern "C" fn add_render_pass(host: *mut sys::Host, desc: *const sys::RenderPassDesc) {
    let ctx = &mut *(host as *mut HostCtx);
    let desc = &*desc;
    let id = desc.id.as_str().to_string();

    // `Shader::from_wgsl` takes the source verbatim; a plugin has no AssetServer
    // and no path we could resolve, so the WGSL crosses the boundary as text.
    let shader = Shader::from_wgsl(desc.fragment_wgsl.as_str().to_string(), id.clone());
    let handle = ctx.world.resource_mut::<Assets<Shader>>().add(shader);

    ctx.world
        .get_resource_or_insert_with(PendingRenderPasses::default)
        .0
        .push(PendingRenderPass {
            id,
            shader: handle,
            phase: desc.phase,
            order: desc.order,
            callback: desc.callback,
        });
}

unsafe extern "C" fn render_set_pipeline(ctx: sys::RenderCtx, _pipeline: sys::PipelineId) {
    if ctx.0.is_null() {
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

unsafe extern "C" fn render_draw(ctx: sys::RenderCtx, vertices: u32, instances: u32) {
    if ctx.0.is_null() {
        return;
    }
    let c = &mut *(ctx.0 as *mut RenderCallCtx);
    c.pass.draw(0..vertices, 0..instances);
}

unsafe extern "C" fn log(_host: *mut sys::Host, level: sys::LogLevel, msg: sys::StrRef) {
    let msg = msg.as_str();
    match level {
        sys::LogLevel::Trace => trace!("[plugin] {msg}"),
        sys::LogLevel::Debug => debug!("[plugin] {msg}"),
        sys::LogLevel::Info => info!("[plugin] {msg}"),
        sys::LogLevel::Warn => warn!("[plugin] {msg}"),
        sys::LogLevel::Error => error!("[plugin] {msg}"),
    }
}

// ── Plan + dispatcher ────────────────────────────────────────────────────────

/// Resolve each declared term into how it will actually be marshalled, or bail
/// if the plugin asked for a component that does not exist. Failing here is much
/// kinder than registering a system whose query silently matches nothing.
fn build_plan(world: &World, terms: &[sys::Term]) -> Option<Vec<TermPlan>> {
    let transform_id = world.component_id::<Transform>();
    let mut plan = Vec::with_capacity(terms.len());

    for t in terms {
        let id = ComponentId::new(t.component.0 as usize);
        let Some(info) = world.components().get_info(id) else {
            error!("plugin declared an unknown component id {}", t.component.0);
            return None;
        };
        let (marshal, cell_size) = if Some(id) == transform_id {
            (Marshal::Transform, size_of::<sys::Transform>())
        } else {
            (Marshal::Raw, info.layout().size())
        };
        plan.push(TermPlan {
            id,
            access: t.access,
            marshal,
            cell_size,
        });
    }
    Some(plan)
}

/// Build the Bevy system that services one registered plugin system.
///
/// `user` is carried as `usize` rather than `*mut c_void` so the closure stays
/// `Send + Sync`; it is the plugin's own opaque token and we never dereference
/// it.
fn build_dispatcher(
    world: &mut World,
    plan: Vec<TermPlan>,
    entry: sys::SystemEntry,
    user: usize,
    host: usize,
) -> impl System<In = (), Out = ()> {
    let build_terms = plan.clone();
    // Latched off after a panic. Without this a system that panics does so every
    // frame forever — thousands of identical errors, and the real first one
    // scrolls away. `AtomicBool` rather than `Cell` because a Bevy system must be
    // `Send + Sync`.
    let disabled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    // One builder per system param: the query needs custom construction, `Res<Time>`
    // does not. The tuple arity here MUST match the closure's parameter count.
    (
        QueryParamBuilder::new(move |builder: &mut QueryBuilder<FilteredEntityMut>| {
            for t in &build_terms {
                match t.access {
                    sys::Access::Read => {
                        builder.ref_id(t.id);
                    }
                    sys::Access::Write => {
                        builder.mut_id(t.id);
                    }
                    sys::Access::With => {
                        builder.with_id(t.id);
                    }
                    sys::Access::Without => {
                        builder.without_id(t.id);
                    }
                }
            }
        }),
        // `ParamBuilder::resource::<Time>()` rather than bare `ParamBuilder`:
        // `build_state` runs before `build_system`, so nothing has pinned the
        // param type yet and inference stalls on `_: SystemParam`.
        ParamBuilder::resource::<Time>(),
    )
        .build_state(world)
        .build_system(move |mut q: Query<FilteredEntityMut>, time: Res<Time>| {
            if disabled.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            // Everything the plugin sees lives in staging buffers we own. That
            // costs a copy per cell, but it is what makes the call sound: we
            // never expose a pointer into component storage whose layout the
            // plugin would have to assume. Optimising the `Marshal::Raw` case to
            // a direct pointer is possible later — those layouts ARE the
            // plugin's own — but it needs a careful aliasing argument, so
            // correctness first.
            // Filter terms contribute no cell, so the plugin indexes by
            // data-term position. Collapse the plan to just those, once.
            let cells_plan: Vec<&TermPlan> =
                plan.iter().filter(|t| t.access.has_cell()).collect();

            let mut staging: Vec<Vec<u8>> = cells_plan
                .iter()
                .map(|t| Vec::<u8>::with_capacity(t.cell_size * 64))
                .collect();
            let mut entities: Vec<sys::Entity> = Vec::new();

            for e in q.iter() {
                entities.push(sys::Entity(e.id().to_bits()));
                for (i, t) in cells_plan.iter().enumerate() {
                    let bytes = read_cell(&e, t);
                    staging[i].extend_from_slice(&bytes);
                }
            }

            if entities.is_empty() {
                return;
            }

            // Cells are row-major `entity_count × term_count`, matching what
            // `sys::SystemCall` documents.
            let mut cells: Vec<*mut u8> = Vec::with_capacity(entities.len() * cells_plan.len());
            for row in 0..entities.len() {
                for (i, t) in cells_plan.iter().enumerate() {
                    cells.push(unsafe { staging[i].as_mut_ptr().add(row * t.cell_size) });
                }
            }

            let call = sys::SystemCall {
                cells: cells.as_mut_ptr(),
                entities: entities.as_ptr(),
                entity_count: entities.len(),
                cell_count: cells_plan.len(),
                frame: sys::FrameCtx {
                    delta_secs: time.delta_secs(),
                    elapsed_secs: time.elapsed_secs(),
                },
                user: user as *mut c_void,
                iface: &IFACE,
                host: host as *mut sys::Host,
            };

            // SAFETY: `entry` came from a `dlopen`'d library the loader keeps
            // alive for the process lifetime, and every pointer in `call` points
            // at a buffer that outlives this statement.
            let status = unsafe { entry(&call) };
            if status == sys::SystemStatus::Panicked {
                error!("[plugin] system panicked — disabling it for this session");
                disabled.store(true, std::sync::atomic::Ordering::Relaxed);
                // Skip write-back: the plugin's partial output is not something
                // to trust into the world.
                return;
            }

            // Write back only the terms the plugin declared `&mut`.
            for (row, mut e) in q.iter_mut().enumerate() {
                for (i, t) in cells_plan.iter().enumerate() {
                    if t.access != sys::Access::Write {
                        continue;
                    }
                    let start = row * t.cell_size;
                    write_cell(&mut e, t, &staging[i][start..start + t.cell_size]);
                }
            }
        })
}

/// Copy one component out of storage into the plugin-facing representation.
fn read_cell(e: &FilteredEntityRef, t: &TermPlan) -> Vec<u8> {
    match t.marshal {
        Marshal::Transform => {
            let src = e
                .get::<Transform>()
                .copied()
                .unwrap_or(Transform::IDENTITY);
            let mirror = to_mirror(&src);
            // SAFETY: `sys::Transform` is `#[repr(C)]` and plain-old-data.
            unsafe {
                std::slice::from_raw_parts(
                    (&mirror as *const sys::Transform).cast::<u8>(),
                    size_of::<sys::Transform>(),
                )
            }
            .to_vec()
        }
        Marshal::Raw => match e.get_by_id(t.id) {
            // SAFETY: the query matched, so the component is present and its
            // storage is `cell_size` bytes.
            Some(ptr) => unsafe {
                std::slice::from_raw_parts(ptr.as_ptr(), t.cell_size).to_vec()
            },
            None => vec![0u8; t.cell_size],
        },
    }
}

/// Copy one component back from the plugin-facing representation into storage.
fn write_cell(e: &mut FilteredEntityMut, t: &TermPlan, bytes: &[u8]) {
    match t.marshal {
        Marshal::Transform => {
            // SAFETY: `bytes` is exactly one `sys::Transform`, written by us.
            let mirror = unsafe { *bytes.as_ptr().cast::<sys::Transform>() };
            if let Some(mut dst) = e.get_mut::<Transform>() {
                *dst = from_mirror(&mirror);
            }
        }
        Marshal::Raw => {
            if let Some(mut ptr) = e.get_mut_by_id(t.id) {
                // SAFETY: same component, same size; the plugin owns this layout.
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        bytes.as_ptr(),
                        ptr.as_mut().as_ptr(),
                        t.cell_size,
                    );
                }
            }
        }
    }
}

// ── Mirror conversions ───────────────────────────────────────────────────────

fn to_mirror(t: &Transform) -> sys::Transform {
    sys::Transform {
        translation: sys::Vec3 {
            x: t.translation.x,
            y: t.translation.y,
            z: t.translation.z,
        },
        rotation: sys::Quat {
            x: t.rotation.x,
            y: t.rotation.y,
            z: t.rotation.z,
            w: t.rotation.w,
        },
        scale: sys::Vec3 {
            x: t.scale.x,
            y: t.scale.y,
            z: t.scale.z,
        },
    }
}

fn from_mirror(m: &sys::Transform) -> Transform {
    Transform {
        translation: Vec3::new(m.translation.x, m.translation.y, m.translation.z),
        rotation: Quat::from_xyzw(m.rotation.x, m.rotation.y, m.rotation.z, m.rotation.w),
        scale: Vec3::new(m.scale.x, m.scale.y, m.scale.z),
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Names of components this plugin registered, so a reload resolves to the same
/// `ComponentId` rather than minting a second one that matches no existing
/// entity.
#[derive(Resource, Default)]
pub struct PluginComponents(pub std::collections::HashMap<String, ComponentId>);

/// One editable field of a plugin component, copied out of the plugin's
/// `sys::FieldDesc` at registration.
///
/// Owned rather than borrowed on purpose: the plugin's statics live in its
/// library, and while we never unload one today, a schema the editor holds
/// across frames should not depend on that staying true.
#[derive(Clone, Debug)]
pub struct PluginField {
    pub name: String,
    pub kind: sys::FieldKind,
    pub offset: usize,
}

/// Everything the editor needs to show a plugin component it has no Rust type
/// for: what to call it, what fields it has, and what a fresh one looks like.
#[derive(Clone)]
pub struct PluginComponentInfo {
    pub id: ComponentId,
    pub type_path: String,
    pub display_name: String,
    pub fields: Vec<PluginField>,
    pub size: usize,
    /// A default-valued instance, `size` bytes. Empty if the plugin supplied
    /// none, in which case the editor falls back to zeroed memory.
    pub default_value: Vec<u8>,
}

/// Schemas for every registered plugin component.
///
/// Read by `renzora_plugin_host_editor` to populate the inspector. Kept here
/// rather than in the editor crate because registration happens during plugin
/// init, which the editor is not involved in.
#[derive(Resource, Default)]
pub struct PluginComponentSchemas(pub Vec<PluginComponentInfo>);

/// Look a component up by its type path. Silent — callers decide whether a miss
/// is an error.
///
/// Name-based rather than `TypeId`-based on purpose: a plugin never linked the
/// host's types, so it has no `TypeId` for them — the string IS the shared
/// identity. That is also why renaming a component type breaks plugins exactly
/// like it breaks saved scenes.
///
/// Resolution goes through the **reflection type registry**, not
/// `ComponentInfo::name()`. That is not a stylistic choice: `ComponentInfo::name`
/// returns a `DebugName`, whose inner string is `#[cfg(feature = "debug")]` — in
/// a build without it there is no name to compare, so this lookup would quietly
/// resolve nothing in release while working perfectly in dev. `TypePath` is
/// always present for a registered type.
///
/// The consequence worth knowing: **a host component must be `register_type`'d
/// to be reachable from a plugin.** That is a real part of the contract, not an
/// implementation detail.
fn lookup_component(world: &World, name: &str) -> Option<ComponentId> {
    if let Some(map) = world.get_resource::<PluginComponents>() {
        if let Some(id) = map.0.get(name) {
            return Some(*id);
        }
    }
    let registry = world.get_resource::<AppTypeRegistry>()?;
    let type_id = registry.read().get_with_type_path(name).map(|r| r.type_id())?;
    // Bevy registers a component lazily — the type exists for reflection but has
    // no `ComponentId` until something uses one. `register_exposed_components`
    // is what makes this reliable at load time.
    world.components().get_id(type_id)
}

fn bevy_label(s: sys::Schedule) -> impl ScheduleLabel {
    // `sys::Schedule` is a closed `#[repr(u32)]` set precisely so this mapping
    // is total and the plugin cannot name a schedule we do not run.
    match s {
        sys::Schedule::First => First.intern(),
        sys::Schedule::PreUpdate => PreUpdate.intern(),
        sys::Schedule::Update => Update.intern(),
        sys::Schedule::PostUpdate => PostUpdate.intern(),
        sys::Schedule::Last => Last.intern(),
    }
}

// ── Loading ──────────────────────────────────────────────────────────────────

/// Call a freshly-`dlopen`'d plugin's init function.
///
/// The library handle must outlive the process: every function pointer the
/// plugin registered points into it, so dropping it would leave the schedule
/// holding dangling entries. (Unloading safely needs a registration ledger and
/// a teardown pass — a separate piece of work.)
pub fn init_plugin(world: &mut World, init: sys::ExtensionInit) -> sys::InitResult {
    let iface = interface();
    let mut ctx = HostCtx { world };
    // SAFETY: `ctx` outlives the call, and the plugin may only call back into
    // the interface while this frame is live.
    unsafe { init(&iface, (&mut ctx as *mut HostCtx).cast()) }
}
