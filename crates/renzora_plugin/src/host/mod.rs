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

pub mod dev;
pub mod input;
pub mod loader;

use bevy::ecs::component::{ComponentDescriptor, ComponentId, StorageType};
use bevy::ecs::query::QueryBuilder;
use bevy::ecs::schedule::{ScheduleLabel, Schedules};
use bevy::ecs::system::{
    FilteredResourcesMutParamBuilder, ParamBuilder, QueryParamBuilder, SystemParamBuilder,
};
use bevy::ecs::world::FilteredResourcesMut;
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
    /// Which reload of which plugin is registering. Handed to every system this
    /// init call creates, so a later reload can retire them.
    gate: GenGate,
    /// The plugin's slot index, stamped on everything it registers so
    /// [`retire_slot`] can take it back on the next reload.
    slot: usize,
    /// Set when a reload re-registers a component with a different memory layout
    /// than the live one. Fails the whole init — see [`init_plugin_gen`].
    layout_conflict: bool,
}

/// Refuse the reload if `desc` is not byte-compatible with what is already
/// registered under this name; otherwise refresh the names the editor reads.
///
/// Bevy fixes a `ComponentId`'s layout permanently at registration, so a reload
/// that moved or added a field would have the plugin writing at new offsets into
/// storage sized for the old struct. Every live instance would be misread, and
/// nothing would say so.
///
/// Called from BOTH `register_component` and `register_resource`. That is the
/// whole reason it is a function: `register_resource` short-circuits on a known
/// name and never reaches `register_component`, so a guard living only in the
/// latter covered components and quietly missed every resource — which is the
/// worse case, since a resource's storage is a single allocation that a
/// grown struct writes straight off the end of.
///
/// Migrating instead — a second `ComponentId` plus a field-name remap, the way
/// `renzora_bsn::raw_registry` does it for scenes — is the real fix and is worth
/// doing. It is just much larger than making the hazard impossible.
///
/// # Safety
///
/// `desc.fields` must be valid for `desc.field_count` entries.
unsafe fn verify_same_layout(
    ctx: &mut HostCtx,
    existing: ComponentId,
    desc: &sys::ComponentDesc,
    name: &str,
) {
    let Some(stored) = component_info(ctx.world, existing) else {
        return;
    };
    match layout_change(&stored, desc) {
        Some(reason) => {
            let kind = if stored.is_resource { "resource" } else { "component" };
            error!(
                "plugin {kind} `{name}` changed layout on reload ({reason}) — refusing \
                 the reload, since what already holds it was allocated for the old \
                 layout. Restart to pick this up."
            );
            ctx.layout_conflict = true;
        }
        // Byte-compatible, so the data is still valid — but a field may have been
        // renamed or the display name changed, and the editor reads those.
        None => refresh_component_schema(ctx.world, existing, desc),
    }
}

/// The stored schema for a component id, if the host has one.
fn component_info(world: &World, id: ComponentId) -> Option<PluginComponentInfo> {
    world
        .get_resource::<PluginComponentSchemas>()?
        .0
        .iter()
        .find(|i| i.id == id)
        .cloned()
}

/// Why `desc` is not byte-compatible with the live registration, or `None` if it
/// is.
///
/// Compares what actually decides whether existing bytes are still readable:
/// total size, and each field's offset and kind. Field *names* are deliberately
/// not part of this — a rename leaves every byte where it was, so it is a schema
/// refresh rather than a layout change.
///
/// # Safety
///
/// `desc.fields` must be valid for `desc.field_count` entries.
unsafe fn layout_change(
    stored: &PluginComponentInfo,
    desc: &sys::ComponentDesc,
) -> Option<String> {
    if stored.size != desc.size {
        return Some(format!("size {} → {}", stored.size, desc.size));
    }
    let fields = if desc.fields.is_null() {
        &[][..]
    } else {
        std::slice::from_raw_parts(desc.fields, desc.field_count)
    };
    if stored.fields.len() != fields.len() {
        return Some(format!(
            "{} field(s) → {}",
            stored.fields.len(),
            fields.len()
        ));
    }
    for (old, new) in stored.fields.iter().zip(fields) {
        if old.offset != new.offset {
            return Some(format!(
                "field `{}` moved from offset {} to {}",
                old.name, old.offset, new.offset
            ));
        }
        if old.kind != new.kind {
            return Some(format!(
                "field `{}` changed from {} to {}",
                old.name,
                old.kind.name(),
                new.kind.name()
            ));
        }
    }
    None
}

/// Update the names and default of a component whose layout did not change.
///
/// # Safety
///
/// `desc.fields` must be valid for `desc.field_count` entries, and
/// `desc.default_init` (if set) must write `desc.size` bytes.
unsafe fn refresh_component_schema(
    world: &mut World,
    id: ComponentId,
    desc: &sys::ComponentDesc,
) {
    let names: Vec<String> = if desc.fields.is_null() {
        Vec::new()
    } else {
        std::slice::from_raw_parts(desc.fields, desc.field_count)
            .iter()
            .map(|f| f.name.as_str().to_string())
            .collect()
    };
    let display = desc.display_name.as_str().to_string();
    let Some(mut schemas) = world.get_resource_mut::<PluginComponentSchemas>() else {
        return;
    };
    let Some(info) = schemas.0.iter_mut().find(|i| i.id == id) else {
        return;
    };
    for (field, name) in info.fields.iter_mut().zip(names) {
        field.name = name;
    }
    if !display.is_empty() {
        info.display_name = display;
    }
}

/// Drop everything slot `slot` registered, except the things a reload must keep.
///
/// **Kept:** components and resources. Their `ComponentId`s are name-keyed and
/// reload-stable by design, and the data lives in the host's ECS — which is the
/// whole reason hot-reload is tractable here. Retiring them would delete the state
/// the reload exists to preserve.
///
/// **Taken back:** panels, render passes and post-process effects, all of which the
/// new build re-registers. Without this a reload would duplicate them.
///
/// **Not here:** systems. Bevy cannot remove one from a schedule, so they retire
/// themselves by generation instead — see [`GenGate`].
pub fn retire_slot(world: &mut World, slot: usize) {
    if let Some(mut panels) = world.get_resource_mut::<PluginPanels>() {
        panels.0.retain(|p| p.owner != slot);
    }
    if let Some(mut passes) = world.get_resource_mut::<PendingRenderPasses>() {
        passes.0.retain(|p| p.owner != slot);
    }
    if let Some(mut effects) = world.get_resource_mut::<PendingPostProcesses>() {
        effects.0.retain(|e| e.owner != slot);
    }

    // GPU assets are the one thing that leaks visibly if this is skipped: a
    // reloaded plugin creates a fresh mesh and material every cycle, and
    // `sys.rs` notes the VRAM growth that follows. Dropping the strong handle is
    // enough — `Assets<T>` frees the underlying resource once nothing holds it.
    let assets = world
        .get_resource_mut::<PluginAssets>()
        .map(|mut a| {
            let meshes = std::mem::take(&mut a.meshes);
            let materials = std::mem::take(&mut a.materials);
            (meshes, materials)
        })
        .unwrap_or_default();
    let (meshes, materials) = assets;
    let mut kept_meshes = Vec::new();
    for (owner, handle) in meshes {
        if owner == slot {
            drop(handle);
        } else {
            kept_meshes.push((owner, handle));
        }
    }
    let mut kept_materials = Vec::new();
    for (owner, handle) in materials {
        if owner == slot {
            drop(handle);
        } else {
            kept_materials.push((owner, handle));
        }
    }
    if let Some(mut a) = world.get_resource_mut::<PluginAssets>() {
        a.meshes = kept_meshes;
        a.materials = kept_materials;
    }
}

/// A plugin slot's reload counter, shared between the slot and every system the
/// plugin registered.
///
/// One `Arc` per slot rather than a `World` lookup because a dispatcher checks it
/// on every run: reading an atomic it already owns costs nothing, whereas a
/// resource lookup would mean declaring access the system does not otherwise need
/// and would serialise plugin systems against each other.
pub type PluginGeneration = std::sync::Arc<std::sync::atomic::AtomicU32>;

/// Lets a system tell whether the plugin that registered it has since reloaded.
///
/// Bevy cannot remove a system from a schedule, so a reloaded plugin's old
/// systems stay in it forever. Rather than restructure every registration to live
/// in a swappable sub-schedule — which would force the runner to be exclusive and
/// stop plugin systems parallelising with engine systems in *every* build,
/// reloading or not — a retired system stays scheduled and returns immediately.
///
/// The cost is that a long dev session accumulates no-op systems, each still
/// paying its param fetch. That is a dev-only cost, cleared by a restart, and a
/// shipped game never reloads so it never has one.
#[derive(Clone)]
struct GenGate {
    counter: PluginGeneration,
    /// The counter's value when the capturing system registered.
    at: u32,
}

impl GenGate {
    /// Live only while this system's generation IS the slot's current one.
    ///
    /// The counter is bumped only after init succeeds, so:
    ///
    /// - **Reload succeeded** — counter moves to N. The previous build's systems
    ///   (N-1) go stale, the new build's (N) are live.
    /// - **Reload failed** — counter stays at N-1. The previous build's systems are
    ///   still live and the new build's, which registered at N before the failure
    ///   was known, are stale. That is what keeps a bad reload from running two
    ///   builds at once.
    ///
    /// This was `at < counter` — "stale once the counter moves PAST you" — on the
    /// reasoning that a system registered during init must not be stale before the
    /// bump. It cannot run during init: the whole reload happens inside one
    /// exclusive system, so no frame elapses between registration and the bump. The
    /// asymmetry solved nothing and broke the failure case, leaving a refused
    /// build's systems live alongside the previous build's — two sets of systems,
    /// one of them reading a struct whose layout the host had just rejected.
    fn stale(&self) -> bool {
        self.at != self.counter.load(std::sync::atomic::Ordering::Relaxed)
    }
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
    log,
    add_render_pass,
    render_set_pipeline,
    render_draw,
    add_post_process,
    add_mesh,
    add_material,
    register_resource,
    insert_resource,
    add_panel,
    set_field_range,
    add_mesh_data,
};


// ── Interface implementations ────────────────────────────────────────────────

unsafe extern "C" fn register_component(
    host: *mut sys::Host,
    desc: *const sys::ComponentDesc,
) -> sys::ComponentId {
    guard_host("register_component", sys::ComponentId::INVALID, || {
    let ctx = &mut *(host as *mut HostCtx);
    let desc = &*desc;
    let name = desc.name.as_str().to_string();

    // Re-registering the same name must return the same id: a plugin reloaded
    // mid-session would otherwise get a second component and silently stop
    // matching the entities carrying the first.
    if let Some(existing) = lookup_component(ctx.world, &name) {
        verify_same_layout(ctx, existing, desc, &name);
        return sys::ComponentId(existing.index() as u32);
    }

    // Refused rather than ignored: a component with a destructor whose drop is
    // never run leaks whatever it owns, silently, for the life of the process.
    if desc.drop.is_some() {
        error!(
            "plugin component `{name}` declares a destructor, which is not supported yet — \
             keep plugin components plain data (no String, Vec or Box fields)"
        );
        return sys::ComponentId::INVALID;
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
            // Deliberately `None`, having already refused a non-`None` drop
            // above. This used to be `desc.drop.map(|_| unimplemented!(..))`,
            // which reads like a guard and is not one: `Option::map` evaluates
            // its body, so any component declaring a destructor panicked here,
            // and `guard_host` swallowed the explanatory message and reported
            // only "host call 'register_component' panicked".
            None,
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
                // Filled in afterwards by `set_field_range`, if the plugin calls
                // it — a field's range cannot ride in `FieldDesc` without changing
                // the array stride. See `sys::Interface::set_field_range`.
                range: None,
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
            is_resource: false,
        });

    sys::ComponentId(id.index() as u32)
    })
}

/// Register a plugin-owned resource and give it its default value.
///
/// A resource in Bevy is a component on a hidden entity — `register_resource` is
/// deprecated in favour of `register_component` for exactly that reason — so this
/// reuses the component path wholesale and then performs the one extra step a
/// resource needs: putting a value in the world, since nothing will ever spawn
/// an entity carrying it.
///
/// Idempotent. Two systems both taking `ResMut<Score>` each drive a registration
/// during init, and the second must not wipe what the first inserted.
unsafe extern "C" fn register_resource(
    host: *mut sys::Host,
    desc: *const sys::ComponentDesc,
) -> sys::ComponentId {
    guard_host("register_resource", sys::ComponentId::INVALID, || {
        let existing = {
            let ctx = &mut *(host as *mut HostCtx);
            lookup_component(ctx.world, (*desc).name.as_str())
        };
        let id = match existing {
            Some(id) => {
                // The layout check lives in `register_component`, which this
                // branch skips — so do it here too, or a resource that grew a
                // field reloads happily and every write past the old size lands
                // outside its allocation.
                let ctx = &mut *(host as *mut HostCtx);
                verify_same_layout(ctx, id, &*desc, (*desc).name.as_str());
                sys::ComponentId(id.index() as u32)
            }
            None => register_component(host, desc),
        };
        if !id.is_valid() {
            return id;
        }

        let ctx = &mut *(host as *mut HostCtx);
        let bevy_id = ComponentId::new(id.0 as usize);
        if let Some(mut schemas) = ctx.world.get_resource_mut::<PluginComponentSchemas>() {
            if let Some(info) = schemas.0.iter_mut().find(|i| i.id == bevy_id) {
                info.is_resource = true;
            }
        }
        if ctx.world.get_resource_by_id(bevy_id).is_none() {
            let size = (*desc).size;
            let mut bytes = vec![0u8; size];
            // No default constructor is not an error — zeroed is a defensible
            // starting value for a POD resource, and refusing to register would
            // break a plugin over something it can fix in a system.
            if let Some(init) = (*desc).default_init {
                init(bytes.as_mut_ptr());
            }
            write_resource_bytes(ctx.world, bevy_id, &bytes);
        }
        // Guarded, not unconditional: registration is idempotent and two
        // systems both taking `ResMut<Score>` each drive one, so an unguarded
        // push listed the same resource once per referencing system.
        let mut listed = ctx
            .world
            .get_resource_or_insert_with(PluginResources::default);
        if !listed.0.contains(&bevy_id) {
            listed.0.push(bevy_id);
        }
        id
    })
}

unsafe extern "C" fn set_field_range(
    host: *mut sys::Host,
    component: sys::ComponentId,
    field: usize,
    range: *const sys::FieldRange,
) -> sys::RegisterStatus {
    guard_host("set_field_range", sys::RegisterStatus::Invalid, || {
        if range.is_null() || !component.is_valid() {
            return sys::RegisterStatus::Invalid;
        }
        let mut range = *range;
        // A range the wrong way round would make every clamp reject everything and
        // every slider sit dead at one end. Swapping is friendlier than refusing,
        // and unambiguous.
        if range.max < range.min {
            core::mem::swap(&mut range.min, &mut range.max);
        }
        // `0.0` asks the host to choose. A thousandth of the span keeps a 0..1
        // field and a 0..1000 field equally draggable, where a fixed step makes one
        // of them unusable.
        if range.speed <= 0.0 {
            range.speed = ((range.max - range.min).abs() / 1000.0).max(f32::EPSILON);
        }

        let ctx = &mut *(host as *mut HostCtx);
        let id = ComponentId::new(component.0 as usize);
        let Some(mut schemas) = ctx.world.get_resource_mut::<PluginComponentSchemas>() else {
            return sys::RegisterStatus::Invalid;
        };
        let Some(info) = schemas.0.iter_mut().find(|i| i.id == id) else {
            return sys::RegisterStatus::UnknownComponent;
        };
        match info.fields.get_mut(field) {
            Some(f) => {
                f.range = Some(range);
                sys::RegisterStatus::Ok
            }
            None => sys::RegisterStatus::Invalid,
        }
    })
}

unsafe extern "C" fn add_panel(
    host: *mut sys::Host,
    desc: *const sys::PanelDesc,
) -> sys::RegisterStatus {
    guard_host("add_panel", sys::RegisterStatus::Invalid, || {
        if desc.is_null() {
            return sys::RegisterStatus::Invalid;
        }
        let desc = &*desc;
        let id = desc.id.as_str().to_string();
        if id.is_empty() || desc.markup.as_str().is_empty() {
            error!("plugin registered a panel with no id or no markup");
            return sys::RegisterStatus::Invalid;
        }

        let ctx = &mut *(host as *mut HostCtx);
        let owner = ctx.slot;
        let mut panels = ctx
            .world
            .get_resource_or_insert_with(PluginPanels::default);
        // A duplicate id would produce two panels fighting over one dock slot
        // and one layout entry, which reads as a panel that will not stay put.
        if panels.0.iter().any(|p| p.id == id) {
            error!("two plugins registered a panel called `{id}` — the second is ignored");
            return sys::RegisterStatus::Invalid;
        }
        panels.0.push(PluginPanel {
            owner,
            title: {
                let t = desc.title.as_str();
                if t.is_empty() { id.clone() } else { t.to_string() }
            },
            id,
            icon: desc.icon.as_str().to_string(),
            category: {
                let c = desc.category.as_str();
                if c.is_empty() { "Plugins".to_string() } else { c.to_string() }
            },
            markup: desc.markup.as_str().to_string(),
            on_action: desc.on_action,
            user: desc.user as usize,
        });
        sys::RegisterStatus::Ok
    })
}

unsafe extern "C" fn insert_resource(
    host: *mut sys::Host,
    id: sys::ComponentId,
    value: *const u8,
    len: usize,
) {
    guard_host("insert_resource", (), || {
        let ctx = &mut *(host as *mut HostCtx);
        let bevy_id = ComponentId::new(id.0 as usize);
        let Some(info) = ctx.world.components().get_info(bevy_id) else {
            error!("plugin inserted an unregistered resource id {}", id.0);
            return;
        };
        // A short write would leave the tail uninitialised and a long one would
        // scribble past the allocation, so mismatched sizes are refused rather
        // than truncated. In practice this only fires if a plugin hand-rolls the
        // ABI call, since the shim always passes `size_of::<T>()`.
        if info.layout().size() != len {
            error!(
                "plugin resource `{}` is {} bytes here but the plugin sent {len}",
                info.name(),
                info.layout().size()
            );
            return;
        }
        let bytes = std::slice::from_raw_parts(value, len);
        write_resource_bytes(ctx.world, bevy_id, bytes);
    })
}

/// Move `bytes` into the world as the value of resource `id`.
///
/// Spawns the backing entity rather than calling `insert_resource_by_id`, because
/// that alone does not make a resource *findable*. In Bevy 0.19 a resource is a
/// component on an entity that also carries `IsResource`, and it is that marker's
/// insert hook which records the entity in the world's resource cache. Without
/// it the value is really in the world and `get_resource_by_id` still returns
/// `None` — the component is there, nothing knows where.
///
/// The allocation handed over must be one Bevy can take over, and the pointer
/// must address the *bytes* — an `OwningPtr` built from a boxed slice points at
/// the fat pointer instead, which is how plugin components once arrived holding
/// `{heap addr, len}` and rendered as nonsense in the inspector.
unsafe fn write_resource_bytes(world: &mut World, id: ComponentId, bytes: &[u8]) {
    let entity = match world.resource_entities().get(id) {
        Some(e) => e,
        None => world.spawn(bevy::ecs::resource::IsResource::new(id)).id(),
    };
    let mut owned = bytes.to_vec();
    let owning = bevy::ptr::OwningPtr::new(std::ptr::NonNull::new_unchecked(
        owned.as_mut_ptr().cast(),
    ));
    world.entity_mut(entity).insert_by_id(id, owning);
    // `owned` drops here, which is correct and NOT a double free. `insert_by_id`
    // copies the value into column storage — it never adopts the caller's
    // allocation — so the buffer is still ours to release. Forgetting it, as this
    // did, leaked `size_of::<T>()` bytes on every single insert. Dropping a
    // `Vec<u8>` runs no element destructors, so the moved-out value is not
    // dropped twice either.
}

unsafe extern "C" fn component_id_by_name(
    host: *mut sys::Host,
    name: sys::StrRef,
) -> sys::ComponentId {
    guard_host("component_id_by_name", sys::ComponentId::INVALID, || {
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
    })
}

unsafe extern "C" fn add_system(
    host: *mut sys::Host,
    desc: *const sys::SystemDesc,
) -> sys::RegisterStatus {
    // The fallback is `AccessConflict` rather than `Ok`, because the one thing
    // that realistically panics in here is Bevy's B0001 check on a conflicting
    // access pattern. Reporting `Ok` from the guard would put the plugin back in
    // the state this return value exists to end: loaded, and silently short a
    // system.
    guard_host("add_system", sys::RegisterStatus::AccessConflict, || {
        let ctx = &mut *(host as *mut HostCtx);
        if desc.is_null() {
            return sys::RegisterStatus::Invalid;
        }
        let desc = &*desc;
        // A system with NO queries is legal and normal: `fn tick(mut s:
        // ResMut<Settings>, time: Res<Time>)` touches no entities at all. This
        // used to require `query_count > 0`, which silently refused every
        // resource-only system a plugin declared — the plugin loaded fine and one
        // of its systems just never ran.
        if desc.flags != 0 || (desc.query_count > 0 && desc.queries.is_null()) {
            error!(
                "plugin sent a malformed SystemDesc (flags {}, {} queries, {} null)",
                desc.flags,
                desc.query_count,
                if desc.queries.is_null() { "ptr" } else { "no ptr" }
            );
            return sys::RegisterStatus::Invalid;
        }

        let mut plans = Vec::with_capacity(desc.query_count);
        let declared = if desc.query_count == 0 {
            &[][..]
        } else {
            std::slice::from_raw_parts(desc.queries, desc.query_count)
        };
        for q in declared {
            let terms = std::slice::from_raw_parts(q.terms, q.term_count);
            let Some(plan) = build_plan(ctx.world, terms) else {
                return sys::RegisterStatus::UnknownComponent;
            };
            plans.push(plan);
        }

        let resources = if desc.resources.is_null() {
            &[][..]
        } else {
            std::slice::from_raw_parts(desc.resources, desc.resource_count)
        };
        let Some(res_plan) = build_plan(ctx.world, resources) else {
            return sys::RegisterStatus::UnknownComponent;
        };

        let gate = ctx.gate.clone();
        let system =
            build_dispatcher(ctx.world, plans, res_plan, desc.entry, desc.user as usize, gate);
        ctx.world
            .resource_mut::<Schedules>()
            .entry(bevy_label(desc.schedule))
            .add_systems(system);
        sys::RegisterStatus::Ok
    })
}

// ── Assets ───────────────────────────────────────────────────────────────────

/// Assets a plugin asked the host to create.
///
/// A plugin never holds a real `Handle` — it gets an index into these. That
/// keeps `Handle`'s layout (and `Assets<T>`'s existence) entirely on this side
/// of the boundary, and means an unloaded plugin's assets are still reachable
/// for cleanup.
#[derive(Resource, Default)]
pub struct PluginAssets {
    /// `(owning slot, handle)`. The owner is what lets a reload drop only its own
    /// meshes — the strong handle here is usually the only one, so dropping it is
    /// what actually frees the GPU memory.
    pub meshes: Vec<(usize, Handle<Mesh>)>,
    pub materials: Vec<(usize, Handle<StandardMaterial>)>,
}

unsafe extern "C" fn add_mesh(host: *mut sys::Host, desc: *const sys::MeshDesc) -> sys::AssetHandle {
    guard_host("add_mesh", sys::AssetHandle::INVALID, || {
        let ctx = &mut *(host as *mut HostCtx);
        let d = &*desc;
        let s = d.size;
        let mesh: Mesh = match d.primitive {
            sys::Primitive::Cuboid => Cuboid::new(s.x, s.y, s.z).into(),
            sys::Primitive::Sphere => Sphere::new(s.x).into(),
            sys::Primitive::Plane => {
                Plane3d::default().mesh().size(s.x, s.z).into()
            }
            sys::Primitive::Cylinder => Cylinder::new(s.x, s.y).into(),
            sys::Primitive::Capsule => Capsule3d::new(s.x, s.y).into(),
            // The ABI documents `x` = major radius and `y` = minor radius, but
            // `Torus::new` takes (inner, outer). Passing (y, x) made bevy derive
            // major = (x+y)/2 and minor = (x-y)/2, so a plugin got a different
            // torus from the one it asked for. inner = major - minor and
            // outer = major + minor invert bevy's arithmetic exactly.
            sys::Primitive::Torus => Torus::new(s.x - s.y, s.x + s.y).into(),
            // A shape this build cannot make. A visible cube beats a missing
            // mesh, which reads as "the spawn silently failed".
            other => {
                warn!("plugin asked for primitive {} which this build does not have", other.0);
                Cuboid::new(s.x, s.y, s.z).into()
            }
        };
        let Some(mut meshes) = ctx.world.get_resource_mut::<Assets<Mesh>>() else {
            warn!("[plugin] add_mesh ignored — this build has no renderer");
            return sys::AssetHandle::INVALID;
        };
        let handle = meshes.add(mesh);
        let owner = ctx.slot;
        let mut store = ctx
            .world
            .get_resource_or_insert_with(PluginAssets::default);
        store.meshes.push((owner, handle));
        sys::AssetHandle((store.meshes.len() - 1) as u64)
    })
}

/// Build a `Mesh` from plugin-generated vertex data.
///
/// Every slice is copied before this returns — the plugin may be pointing at
/// stack locals — and every length is treated as untrusted, because these are
/// raw pointers out of another compilation unit and a bad one is a read off the
/// end of the plugin's heap, not a panic.
unsafe extern "C" fn add_mesh_data(
    host: *mut sys::Host,
    desc: *const sys::MeshDataDesc,
) -> sys::AssetHandle {
    guard_host("add_mesh_data", sys::AssetHandle::INVALID, || {
        let ctx = &mut *(host as *mut HostCtx);
        let d = &*desc;

        if d.positions.is_null() || d.position_count == 0 {
            error!("[plugin] add_mesh_data with no positions");
            return sys::AssetHandle::INVALID;
        }
        let positions: Vec<[f32; 3]> = std::slice::from_raw_parts(d.positions, d.position_count)
            .iter()
            .map(|v| [v.x, v.y, v.z])
            .collect();

        // Indices are validated against the vertex count rather than trusted.
        // An out-of-range index is not a soft failure downstream: wgpu reads
        // past the vertex buffer and the result is a GPU fault, which takes the
        // whole process with it rather than this one mesh.
        let indices: Option<Vec<u32>> = if d.indices.is_null() || d.index_count == 0 {
            None
        } else {
            let raw = std::slice::from_raw_parts(d.indices, d.index_count);
            if let Some(&bad) = raw.iter().find(|&&i| i as usize >= positions.len()) {
                error!(
                    "[plugin] add_mesh_data index {bad} is out of range for {} vertices — \
                     refusing the mesh rather than letting the GPU read past the buffer",
                    positions.len()
                );
                return sys::AssetHandle::INVALID;
            }
            if raw.len() % 3 != 0 {
                error!(
                    "[plugin] add_mesh_data got {} indices, which is not a whole number of \
                     triangles",
                    raw.len()
                );
                return sys::AssetHandle::INVALID;
            }
            Some(raw.to_vec())
        };
        if indices.is_none() && positions.len() % 3 != 0 {
            error!(
                "[plugin] add_mesh_data got {} unindexed positions, which is not a whole \
                 number of triangles",
                positions.len()
            );
            return sys::AssetHandle::INVALID;
        }

        // A short attribute array is refused rather than padded. Padding would
        // hand back a mesh that renders with silently wrong shading or UVs on
        // the tail vertices, which is harder to notice than nothing at all.
        let normals: Option<Vec<[f32; 3]>> = if d.normals.is_null() || d.normal_count == 0 {
            None
        } else if d.normal_count != positions.len() {
            error!(
                "[plugin] add_mesh_data got {} normals for {} vertices",
                d.normal_count,
                positions.len()
            );
            return sys::AssetHandle::INVALID;
        } else {
            Some(
                std::slice::from_raw_parts(d.normals, d.normal_count)
                    .iter()
                    .map(|v| [v.x, v.y, v.z])
                    .collect(),
            )
        };
        let uvs: Option<Vec<[f32; 2]>> = if d.uvs.is_null() || d.uv_count == 0 {
            None
        } else if d.uv_count != positions.len() {
            error!(
                "[plugin] add_mesh_data got {} uvs for {} vertices",
                d.uv_count,
                positions.len()
            );
            return sys::AssetHandle::INVALID;
        } else {
            Some(std::slice::from_raw_parts(d.uvs, d.uv_count).to_vec())
        };

        let mut mesh = Mesh::new(
            bevy::render::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        // UVs before normals: `compute_normals` needs the indices in place but
        // not the UVs, and inserting them first keeps the attribute set complete
        // whichever branch runs.
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_UV_0,
            uvs.unwrap_or_else(|| vec![[0.0, 0.0]; mesh.count_vertices()]),
        );
        if let Some(indices) = indices {
            mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));
        }
        match normals {
            Some(n) => mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, n),
            // Bevy's own derivation, so a plugin that skips normals gets the
            // same shading an engine crate would have produced by hand.
            None => mesh.compute_normals(),
        }

        let Some(mut meshes) = ctx.world.get_resource_mut::<Assets<Mesh>>() else {
            warn!("[plugin] add_mesh_data ignored — this build has no renderer");
            return sys::AssetHandle::INVALID;
        };
        let handle = meshes.add(mesh);
        let owner = ctx.slot;
        let mut store = ctx.world.get_resource_or_insert_with(PluginAssets::default);
        store.meshes.push((owner, handle));
        sys::AssetHandle((store.meshes.len() - 1) as u64)
    })
}

unsafe extern "C" fn add_material(
    host: *mut sys::Host,
    desc: *const sys::MaterialDesc,
) -> sys::AssetHandle {
    guard_host("add_material", sys::AssetHandle::INVALID, || {
        let ctx = &mut *(host as *mut HostCtx);
        let d = &*desc;
        let material = StandardMaterial {
            base_color: Color::linear_rgba(d.color[0], d.color[1], d.color[2], d.color[3]),
            metallic: d.metallic,
            perceptual_roughness: d.roughness,
            emissive: LinearRgba::new(d.emissive[0], d.emissive[1], d.emissive[2], d.emissive[3]),
            ..default()
        };
        let Some(mut materials) = ctx.world.get_resource_mut::<Assets<StandardMaterial>>() else {
            warn!("[plugin] add_material ignored — this build has no renderer");
            return sys::AssetHandle::INVALID;
        };
        let handle = materials.add(material);
        let owner = ctx.slot;
        let mut store = ctx
            .world
            .get_resource_or_insert_with(PluginAssets::default);
        store.materials.push((owner, handle));
        sys::AssetHandle((store.materials.len() - 1) as u64)
    })
}

// ── Commands ─────────────────────────────────────────────────────────────────

/// Backs `sys::CommandSink` for one system invocation.
///
/// `sink` must be the FIRST field: the plugin holds a `*mut CommandSink` and the
/// host casts it back to this, so the two must share an address.
#[repr(C)]
/// Completed HTTP responses waiting for the plugin that asked for them.
///
/// Held here rather than acted on for the same reason [`PluginServiceCalls`] is:
/// this crate has no HTTP client and cannot depend on one. Whichever engine
/// crate owns networking drains the *requests* and pushes results back here.
///
/// Keyed by the plugin's own tag. Nothing ages entries out: a plugin that fires
/// a request and never polls for it leaks one response, which is bounded by how
/// many requests it makes and is the plugin's own bug to fix. Dropping them on a
/// timer would instead make a slow frame look like a network failure.
#[derive(Resource, Default)]
pub struct PluginHttpInbox(pub Vec<PluginHttpResponse>);

/// One completed response.
pub struct PluginHttpResponse {
    /// The tag the plugin supplied with the request.
    pub tag: u64,
    /// HTTP status, or 0 if the request never completed — `body` then holds the
    /// error text.
    pub status: u16,
    pub body: String,
}

/// Backs [`sys::HttpSource`] for one system call.
#[repr(C)]
struct HttpSourceImpl<'a> {
    src: sys::HttpSource,
    inbox: Option<&'a mut PluginHttpInbox>,
}

/// Hand the plugin the next response for `tag`.
///
/// **The probe pass does not consume.** A caller that learns the length and then
/// fails to allocate must be able to try again; removing on the first call would
/// drop the response on the floor. The filling pass — the one that actually
/// takes the bytes — is what removes it.
unsafe extern "C" fn http_poll(
    src: *mut sys::HttpSource,
    tag: u64,
    out: *mut sys::HttpRead,
) -> bool {
    let me = &mut *(src as *mut HttpSourceImpl);
    let out = &mut *out;
    out.body_len = 0;
    out.status = 0;

    let Some(inbox) = me.inbox.as_deref_mut() else {
        return false;
    };
    let Some(at) = inbox.0.iter().position(|r| r.tag == tag) else {
        return false;
    };

    let consuming = !out.body.is_null() && out.body_capacity > 0;
    {
        let r = &inbox.0[at];
        out.status = r.status;
        out.body_len = r.body.len();
        if consuming {
            let n = out.body_capacity.min(r.body.len());
            std::ptr::copy_nonoverlapping(r.body.as_ptr(), out.body, n);
            out.body_len = n;
        }
    }
    if consuming {
        inbox.0.remove(at);
    }
    true
}

/// Backs [`sys::MeshSource`] for one system call.
///
/// `src` is first so a `*mut MeshSourceImpl` can be handed over as a
/// `*mut sys::MeshSource` — the same layout trick [`SinkImpl`] uses, which is
/// what lets the plugin call a plain function pointer and get back here.
#[repr(C)]
struct MeshSourceImpl<'a, 'w, 's> {
    src: sys::MeshSource,
    assets: Option<&'a Assets<Mesh>>,
    handles: &'a Query<'w, 's, &'static Mesh3d>,
}

/// Copy one mesh's geometry into the plugin's buffers.
///
/// Counts are always reported in full, whatever the capacity, so the two-pass
/// probe works: the first call passes zero capacity and reads the sizes back.
unsafe extern "C" fn mesh_read(
    src: *mut sys::MeshSource,
    entity: sys::Entity,
    out: *mut sys::MeshRead,
) -> bool {
    let me = &*(src as *mut MeshSourceImpl);
    let out = &mut *out;
    out.position_count = 0;
    out.normal_count = 0;
    out.uv_count = 0;
    out.index_count = 0;

    let Some(assets) = me.assets else {
        return false;
    };
    let Some(entity) = Entity::try_from_bits(entity.0) else {
        return false;
    };
    let Ok(handle) = me.handles.get(entity) else {
        return false;
    };
    // A miss here is the normal early-frame state, not an error: mesh assets
    // load asynchronously, so a plugin polls until this succeeds.
    let Some(mesh) = assets.get(&handle.0) else {
        return false;
    };

    /// Copy up to `cap` items into `dst`, and report how many exist.
    unsafe fn fill<T: Copy>(dst: *mut T, cap: usize, src: &[T]) -> usize {
        if !dst.is_null() && cap > 0 {
            let n = cap.min(src.len());
            std::ptr::copy_nonoverlapping(src.as_ptr(), dst, n);
        }
        src.len()
    }

    if let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(p)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    {
        // `sys::Vec3` is three `f32`s in order, so `[f32; 3]` is the same bytes.
        out.position_count = fill(out.positions.cast::<[f32; 3]>(), out.position_capacity, p);
    }
    if let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(n)) =
        mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
    {
        out.normal_count = fill(out.normals.cast::<[f32; 3]>(), out.normal_capacity, n);
    }
    if let Some(bevy::render::mesh::VertexAttributeValues::Float32x2(u)) =
        mesh.attribute(Mesh::ATTRIBUTE_UV_0)
    {
        out.uv_count = fill(out.uvs, out.uv_capacity, u);
    }
    // Widened to u32 rather than refused: a 16-bit index buffer is the common
    // case for small meshes, and a plugin should not have to handle both.
    match mesh.indices() {
        Some(bevy::render::mesh::Indices::U32(i)) => {
            out.index_count = fill(out.indices, out.index_capacity, i);
        }
        Some(bevy::render::mesh::Indices::U16(i)) => {
            let widened: Vec<u32> = i.iter().map(|&x| x as u32).collect();
            out.index_count = fill(out.indices, out.index_capacity, &widened);
        }
        None => {}
    }
    true
}

struct SinkImpl<'a, 'w, 's> {
    sink: sys::CommandSink,
    commands: &'a mut Commands<'w, 's>,
    queued: Vec<(sys::Command, Vec<u8>)>,
}

/// The host's interface table.
///
/// A `'static` pointer, so a caller outside the system dispatch — a panel
/// action, say — can hand it to a plugin without owning one.
pub fn interface() -> *const sys::Interface {
    &IFACE
}

/// A command sink for a call that is not a system dispatch.
///
/// A plugin invoked from the editor's UI still needs to be able to spawn and
/// despawn, and structural changes must go through Bevy's deferred queue there
/// for the same reason they do in a system.
pub struct HostCommandSink<'a, 'w, 's>(SinkImpl<'a, 'w, 's>);

impl<'a, 'w, 's> HostCommandSink<'a, 'w, 's> {
    pub fn new(commands: &'a mut Commands<'w, 's>) -> Self {
        Self(SinkImpl {
            sink: sys::CommandSink {
                reserve_entity: sink_reserve,
                push: sink_push,
            },
            commands,
            queued: Vec::new(),
        })
    }

    /// The pointer to hand across the ABI. Valid until this is dropped.
    pub fn as_ptr(&mut self) -> *mut sys::CommandSink {
        (&mut self.0 as *mut SinkImpl).cast()
    }

    /// Apply whatever the plugin queued.
    ///
    /// Takes `self` and applies through the borrow it already holds — asking the
    /// caller for `&mut Commands` again would be a second mutable borrow of the
    /// one this sink is built on.
    pub fn drain(mut self) {
        let queued = std::mem::take(&mut self.0.queued);
        apply_queued(self.0.commands, queued);
    }
}

unsafe extern "C" fn sink_reserve(sink: *mut sys::CommandSink) -> sys::Entity {
    let me = &mut *(sink as *mut SinkImpl);
    // `spawn_empty` reserves an id that is valid immediately and materialises
    // when commands are applied — which is what lets a plugin use the id in the
    // same frame it asked for it.
    sys::Entity(me.commands.spawn_empty().id().to_bits())
}

unsafe extern "C" fn sink_push(sink: *mut sys::CommandSink, cmd: *const sys::Command) {
    let me = &mut *(sink as *mut SinkImpl);
    let cmd = &*cmd;
    // Copy the payload NOW. `data` may point at a plugin stack local that is gone
    // by the time commands are applied.
    let data = if cmd.data.is_null() || cmd.data_len == 0 {
        Vec::new()
    } else {
        std::slice::from_raw_parts(cmd.data, cmd.data_len).to_vec()
    };
    me.queued.push((
        sys::Command {
            kind: cmd.kind,
            entity: cmd.entity,
            component: cmd.component,
            data: std::ptr::null(),
            data_len: 0,
        },
        data,
    ));
}

/// Apply what a system queued. Runs after the system body, never during it.
fn apply_queued(commands: &mut Commands, queued: Vec<(sys::Command, Vec<u8>)>) {
    for (cmd, data) in queued {
        let Some(entity) = Entity::try_from_bits(cmd.entity.0) else {
            continue;
        };
        match cmd.kind {
            sys::CommandKind::Despawn => {
                commands.entity(entity).try_despawn();
            }
            sys::CommandKind::Remove => {
                if cmd.component.is_valid() {
                    let id = ComponentId::new(cmd.component.0 as usize);
                    commands.queue(move |world: &mut World| {
                        if let Ok(mut e) = world.get_entity_mut(entity) {
                            e.remove_by_id(id);
                        }
                    });
                }
            }
            sys::CommandKind::SpawnMesh => {
                if data.len() < size_of::<sys::SpawnMeshDesc>() {
                    continue;
                }
                // SAFETY: pushed by `make_renderable`, which writes exactly one.
                let d = unsafe { *data.as_ptr().cast::<sys::SpawnMeshDesc>() };
                commands.queue(move |world: &mut World| {
                    let (mesh, material) = {
                        let Some(store) = world.get_resource::<PluginAssets>() else {
                            return;
                        };
                        // `.1` — the store keys each handle by owning slot so a
                        // reload can free its own; a spawn only wants the handle.
                        let m = store.meshes.get(d.mesh.0 as usize).map(|(_, h)| h.clone());
                        let mat = store
                            .materials
                            .get(d.material.0 as usize)
                            .map(|(_, h)| h.clone());
                        match (m, mat) {
                            (Some(m), Some(mat)) => (m, mat),
                            _ => {
                                error!("[plugin] spawn_mesh used an unknown asset handle");
                                return;
                            }
                        }
                    };
                    if let Ok(mut e) = world.get_entity_mut(entity) {
                        e.insert((
                            Mesh3d(mesh),
                            MeshMaterial3d(material),
                            from_mirror(&d.transform),
                        ));
                    }
                });
            }
            sys::CommandKind::Insert => {
                // An invalid id here means the plugin inserted a component it
                // never registered or queried. `component_id_of` reads the id
                // the host assigned at init, and nothing assigns one for a type
                // the plugin only ever inserts — so this was a silent no-op, the
                // worst possible outcome for "my component never appears".
                if !cmd.component.is_valid() {
                    error!(
                        "plugin queued an insert for a component it never registered —                          call `app.register_component::<T>()` in `build()` for every type                          you insert, including host types like `Transform`"
                    );
                    continue;
                }
                if data.is_empty() {
                    continue;
                }
                let id = ComponentId::new(cmd.component.0 as usize);
                commands.queue(move |world: &mut World| {
                    // The plugin sent bytes in ITS representation. For a
                    // plugin-owned component that is also the host's, so the
                    // bytes go in verbatim. For a host component it is the frozen
                    // mirror, which is a different size AND a different field
                    // layout — `sys::Transform` is 40 bytes with rotation at
                    // offset 12, `bevy::Transform` is 48 with rotation at 16 —
                    // so it must be marshalled exactly as the query write-back
                    // marshals it.
                    if Some(id) == world.component_id::<Transform>() {
                        if data.len() != size_of::<sys::Transform>() {
                            error!(
                                "plugin sent {} bytes for a Transform; expected {}",
                                data.len(),
                                size_of::<sys::Transform>()
                            );
                            return;
                        }
                        // SAFETY: length checked, and `sys::Transform` is
                        // `#[repr(C)]` plain-old-data.
                        let mirror =
                            unsafe { data.as_ptr().cast::<sys::Transform>().read_unaligned() };
                        if let Ok(mut e) = world.get_entity_mut(entity) {
                            e.insert(from_mirror(&mirror));
                        }
                        return;
                    }

                    // Size is checked against the LIVE layout, not against what
                    // the plugin claimed. `insert_by_id` copies `layout.size()`
                    // bytes from the pointer regardless of how many the plugin
                    // actually sent, so a short buffer is a heap over-read that
                    // lands in component storage and surfaces later as garbage in
                    // an unrelated field.
                    let Some(size) = world
                        .components()
                        .get_info(id)
                        .map(|i| i.layout().size())
                    else {
                        error!("plugin inserted an unregistered component id {}", id.index());
                        return;
                    };
                    if data.len() != size {
                        error!(
                            "plugin sent {} bytes for component id {}; it is {size} bytes here",
                            data.len(),
                            id.index()
                        );
                        return;
                    }

                    let mut bytes = data;
                    // SAFETY: `bytes` is one instance of this component, copied
                    // from the plugin at push time, and its length now matches
                    // the registered layout.
                    unsafe {
                        let ptr = bevy::ptr::OwningPtr::new(
                            std::ptr::NonNull::new_unchecked(bytes.as_mut_ptr().cast()),
                        );
                        if let Ok(mut e) = world.get_entity_mut(entity) {
                            e.insert_by_id(id, ptr);
                        }
                    }
                    // `bytes` drops here — see `write_resource_bytes` for why
                    // that is correct rather than a double free.
                });
            }
            sys::CommandKind::SpawnBsn => {
                let Ok(source) = String::from_utf8(data) else {
                    error!("plugin sent BSN that is not valid UTF-8");
                    continue;
                };
                commands.queue(move |world: &mut World| {
                    let Some(spawner) = world.get_resource::<BsnSpawner>().copied() else {
                        error!(
                            "a plugin spawned BSN but nothing installed a `BsnSpawner` — \
                             the tree was dropped"
                        );
                        return;
                    };
                    (spawner.0)(world, entity, &source);
                });
            }
            sys::CommandKind::Service => {
                let hdr_len = size_of::<sys::ServiceCall>();
                if data.len() < hdr_len {
                    error!(
                        "plugin sent {} bytes for a service call; the header alone is {hdr_len}",
                        data.len()
                    );
                    continue;
                }
                // SAFETY: length checked, and `sys::ServiceCall` is `#[repr(C)]`
                // plain-old-data.
                let hdr = unsafe { data.as_ptr().cast::<sys::ServiceCall>().read_unaligned() };
                let payload = data[hdr_len..].to_vec();
                // Parked, not applied, and deliberately not inspected: what these
                // bytes mean is the consumer's business. This crate cannot depend
                // on any engine crate — see the module doc — so it does not know
                // and must not guess.
                commands.queue(move |world: &mut World| {
                    world
                        .get_resource_or_insert_with(PluginServiceCalls::default)
                        .0
                        .push(ServiceCall {
                            entity,
                            service: hdr.service,
                            op: hdr.op,
                            payload,
                        });
                });
            }
            // A command kind from a newer ABI. Dropping it is the only
            // option: what the payload means is exactly the thing this build
            // does not know.
            other => {
                warn!("plugin queued command kind {} which this build does not have", other.0);
            }
        }
    }
}

// ── Services ─────────────────────────────────────────────────────────────────

/// One [`sys::CommandKind::Service`] call, as parked for its consumer.
pub struct ServiceCall {
    pub entity: Entity,
    /// From `sys::service_id`. Which crate this is for.
    pub service: u64,
    /// The operation, in that service's own numbering.
    pub op: u32,
    /// The payload, exactly as the plugin wrote it. **Not** interpreted here —
    /// this crate has no idea what any of it means, which is the point.
    pub payload: Vec<u8>,
}

/// Service calls plugins queued this frame, waiting for whoever claims them.
///
/// Held rather than acted on, for the same reason [`PluginPanel`] is: this crate
/// must stay publishable to crates.io, so it cannot depend on `renzora_animation`
/// — or on anything else in the engine. It carries bytes it does not read.
///
/// **Nothing draining a service is a valid configuration.** A dedicated server or
/// a lean export that dropped the crate in question simply discards those calls;
/// see [`discard_unhandled_service_calls`].
#[derive(Resource, Default)]
pub struct PluginServiceCalls(pub Vec<ServiceCall>);

impl PluginServiceCalls {
    /// Take every call for one service, leaving the rest for other consumers.
    ///
    /// Per-service rather than "drain everything", because more than one bridge
    /// reads this queue and a consumer that took the lot would silently eat
    /// another domain's calls — a failure with no symptom except a feature that
    /// quietly stops working when an unrelated crate is present.
    pub fn take(&mut self, service: u64) -> Vec<ServiceCall> {
        let mut taken = Vec::new();
        let mut i = 0;
        while i < self.0.len() {
            if self.0[i].service == service {
                taken.push(self.0.remove(i));
            } else {
                i += 1;
            }
        }
        taken
    }
}

/// Discards service calls nothing claimed, at the end of the frame.
///
/// Registered by the host, and it has to be: without a consumer the queue is
/// append-only, and a plugin calling into a service every frame in a build that
/// lacks its bridge would grow it until the process died. Real consumers drain
/// their own service earlier in the frame and this sees only what is left.
pub fn discard_unhandled_service_calls(queue: Option<ResMut<PluginServiceCalls>>) {
    if let Some(mut queue) = queue {
        if !queue.0.is_empty() {
            queue.0.clear();
        }
    }
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
    /// The source the handle was built from, kept so a reload can tell whether the
    /// shader actually changed and swap it into the ALREADY-REGISTERED handle. The
    /// pass holds that handle inside a built pipeline, so replacing the asset is
    /// what makes the pipeline cache recompile — a fresh handle would be ignored.
    pub wgsl: String,
    pub phase: sys::RenderPhase,
    pub order: f32,
    pub callback: sys::RenderCallback,
    /// Registering plugin slot, so a reload replaces this rather than adding a
    /// second copy. See [`retire_slot`].
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
    /// Registering plugin slot — see [`retire_slot`].
    pub owner: usize,
}

/// Every panel registered by every loaded plugin.
#[derive(Resource, Default)]
pub struct PluginPanels(pub Vec<PluginPanel>);

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
    /// Registering plugin slot — see [`retire_slot`].
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

unsafe extern "C" fn add_render_pass(host: *mut sys::Host, desc: *const sys::RenderPassDesc) {
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

unsafe extern "C" fn add_post_process(host: *mut sys::Host, desc: *const sys::PostProcessDesc) {
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

unsafe extern "C" fn render_set_pipeline(ctx: sys::RenderCtx, _pipeline: sys::PipelineId) {
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

unsafe extern "C" fn render_draw(ctx: sys::RenderCtx, vertices: u32, instances: u32) {
    if ctx.0.is_null() {
        error!("[plugin] render_draw called with a null ctx");
        return;
    }
    let c = &mut *(ctx.0 as *mut RenderCallCtx);
    c.pass.draw(0..vertices, 0..instances);
}

/// Run a host interface body, converting a panic into a caller-visible failure.
///
/// Every function in [`Interface`] is `extern "C"`, so a panic inside one cannot
/// unwind and aborts the process instead — the editor dies because a plugin
/// asked for something in a state we did not anticipate. This is the host-side
/// counterpart to the guard the ergonomic layer puts around plugin systems; the
/// boundary is dangerous in both directions and it took a test-suite abort to
/// notice we had only armed one side.
fn guard_host<R>(what: &str, fallback: R, body: impl FnOnce() -> R) -> R {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)) {
        Ok(v) => v,
        Err(_) => {
            error!("[plugin] host call `{what}` panicked — returning a failure to the plugin");
            fallback
        }
    }
}

unsafe extern "C" fn log(_host: *mut sys::Host, level: sys::LogLevel, msg: sys::StrRef) {
    let msg = msg.as_str();
    match level {
        sys::LogLevel::Trace => trace!("[plugin] {msg}"),
        sys::LogLevel::Debug => debug!("[plugin] {msg}"),
        sys::LogLevel::Info => info!("[plugin] {msg}"),
        sys::LogLevel::Warn => warn!("[plugin] {msg}"),
        sys::LogLevel::Error => error!("[plugin] {msg}"),
        // A level from a newer ABI. Logging it at all beats dropping it.
        _ => info!("[plugin] {msg}"),
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
        // Refuse the whole system rather than skip the term. An unknown access
        // kind may have wanted a cell, and dropping it silently would shift
        // every later cell index — the plugin would then read its own data at
        // the wrong offsets, which presents as garbage values rather than as a
        // version problem.
        if !t.access.is_known() {
            error!(
                "plugin used access kind {} which this build does not have —                  refusing the system rather than mis-indexing its data",
                t.access.0
            );
            return None;
        }
        // `Or` brackets name nothing. They survive into the plan so the query
        // builder can still see the grouping, and are filtered out everywhere
        // that walks terms for data.
        if t.access.is_marker() {
            plan.push(TermPlan {
                id: ComponentId::new(0),
                access: t.access,
                marshal: Marshal::Raw,
                cell_size: 0,
            });
            continue;
        }
        let id = ComponentId::new(t.component.0 as usize);
        let Some(info) = world.components().get_info(id) else {
            error!("plugin declared an unknown component id {}", t.component.0);
            return None;
        };
        if t.access.is_resource() {
            plan.push(TermPlan {
                id,
                access: t.access,
                marshal: Marshal::Raw,
                cell_size: info.layout().size(),
            });
            continue;
        }
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

/// Translate a flat term list into a Bevy query.
///
/// Flat rather than nested because the ABI carries one term array: an `Or` is a
/// bracketed run — `OrBegin`, branches separated by `OrNext`, `OrEnd` — so the
/// filter grammar can grow without the boundary struct changing shape.
/// Split the bracketed run that follows an `OrBegin` at `start` into its
/// branches, returning them and the index just past the matching `OrEnd`.
///
/// Depth-tracked so a nested `Or` inside a branch does not close the outer
/// group.
fn split_or_branches(terms: &[TermPlan], start: usize) -> (Vec<Vec<TermPlan>>, usize) {
    let mut branches: Vec<Vec<TermPlan>> = vec![Vec::new()];
    let mut depth = 0usize;
    let mut i = start;
    while i < terms.len() {
        let inner = terms[i].clone();
        i += 1;
        match inner.access {
            sys::Access::OrEnd if depth == 0 => break,
            sys::Access::OrNext if depth == 0 => branches.push(Vec::new()),
            _ => {
                if inner.access == sys::Access::OrBegin {
                    depth += 1;
                } else if inner.access == sys::Access::OrEnd {
                    depth -= 1;
                }
                branches.last_mut().unwrap().push(inner);
            }
        }
    }
    (branches, i)
}

/// Apply a run of filter terms to `builder`.
///
/// Recursive, because an `Or` branch may itself contain an `Or` — `Or<T>` is a
/// `QueryFilter` like any other, so `Or<(With<A>, Or<(With<B>, With<C>)>)>` is
/// ordinary code to write. A flat walk drops the inner brackets while still
/// emitting the inner `with_id`s, which silently turns the inner `Or` into an
/// `AND` and matches strictly fewer entities than asked for.
fn apply_filters(builder: &mut QueryBuilder, terms: &[TermPlan]) {
    let mut i = 0;
    while i < terms.len() {
        let t = &terms[i];
        i += 1;
        match t.access {
            sys::Access::With => {
                builder.with_id(t.id);
            }
            sys::Access::Without => {
                builder.without_id(t.id);
            }
            sys::Access::OrBegin => {
                let (branches, next) = split_or_branches(terms, i);
                i = next;
                builder.or(|b| {
                    for branch in &branches {
                        b.and(|bb| apply_filters(bb, branch));
                    }
                });
            }
            // Only filters make sense inside a group: data access would have to
            // be conditional on which branch matched, which no cell layout can
            // express. An unknown kind lands here too, harmlessly — a group is
            // pure filtering, so skipping a term only widens the match.
            _ => {}
        }
    }
}

fn build_query(builder: &mut QueryBuilder<FilteredEntityMut>, terms: &[TermPlan]) {
    let mut i = 0;
    while i < terms.len() {
        let t = &terms[i];
        i += 1;
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
            // Declares the access without the `with` that `ref_id`/`mut_id` imply,
            // so the entity matches whether or not it has the component.
            sys::Access::ReadOptional => {
                let id = t.id;
                builder.optional(move |b| {
                    b.ref_id(id);
                });
            }
            sys::Access::WriteOptional => {
                let id = t.id;
                builder.optional(move |b| {
                    b.mut_id(id);
                });
            }
            // Resources are not part of the entity query at all — they come in
            // through their own param.
            sys::Access::ResRead | sys::Access::ResWrite => {}
            sys::Access::OrBegin => {
                let (branches, next) = split_or_branches(terms, i);
                i = next;
                builder.or(|b| {
                    for branch in &branches {
                        // Each branch is one alternative, so its own terms must
                        // AND together before being OR-ed with the next.
                        b.and(|bb| apply_filters(bb, branch));
                    }
                });
            }
            sys::Access::OrNext | sys::Access::OrEnd => {}
            // An access kind from a newer ABI. Ignoring it is the only safe
            // move — the term may have wanted a cell, and inventing one would
            // shift every later cell index and hand the plugin the wrong data.
            // `build_plan` refuses the system outright for the same reason;
            // this arm exists so the match is total.
            other => {
                warn!("plugin used access kind {} which this build does not have", other.0);
            }
        }
    }
}

/// One query's staging buffers, rebuilt each call.
///
/// Split out per query because a system now has as many of these as it declared
/// `Query` parameters, and the plugin indexes cells within a view rather than
/// across the whole call.
struct ViewState {
    /// The plan's data terms only — filters contribute no cell, so a cell index
    /// is not a term index.
    cells_plan: Vec<TermPlan>,
    /// Column-major: `staging[term]` holds every row's bytes for that term.
    staging: Vec<Vec<u8>>,
    /// A copy taken before the plugin ran, for the terms it can write.
    ///
    /// Write-back used to be unconditional, which marked every matched component
    /// changed every frame — that does not just cost time, it destroys change
    /// detection for the whole engine, since `Changed<Transform>` anywhere
    /// becomes true whenever any plugin merely *looks* at a transform.
    baseline: Vec<Vec<u8>>,
    /// Only optional terms can be absent, but tracking presence for every term
    /// keeps row indexing uniform.
    present: Vec<Vec<bool>>,
    entities: Vec<sys::Entity>,
    cells: Vec<*mut u8>,
}

impl ViewState {
    fn new(cells_plan: Vec<TermPlan>) -> Self {
        let n = cells_plan.len();
        Self {
            staging: cells_plan
                .iter()
                .map(|t| Vec::<u8>::with_capacity(t.cell_size * 64))
                .collect(),
            baseline: vec![Vec::new(); n],
            present: vec![Vec::new(); n],
            cells_plan,
            entities: Vec::new(),
            cells: Vec::new(),
        }
    }

    fn is_writable(t: &TermPlan) -> bool {
        matches!(
            t.access,
            sys::Access::Write | sys::Access::WriteOptional
        )
    }

    /// Copy every matched row into the staging buffers.
    fn gather(&mut self, q: &mut Query<FilteredEntityMut>) {
        for e in q.iter() {
            self.entities.push(sys::Entity(e.id().to_bits()));
            for (i, t) in self.cells_plan.iter().enumerate() {
                match read_cell(&e, t) {
                    Some(bytes) => {
                        self.staging[i].extend_from_slice(&bytes);
                        self.present[i].push(true);
                    }
                    // Still reserve the row so offsets stay uniform; the plugin
                    // sees a null cell and never reads these bytes.
                    None => {
                        let len = self.staging[i].len();
                        self.staging[i].resize(len + t.cell_size, 0);
                        self.present[i].push(false);
                    }
                }
            }
        }

        for (i, t) in self.cells_plan.iter().enumerate() {
            if Self::is_writable(t) {
                self.baseline[i] = self.staging[i].clone();
            }
        }

        // Row-major `entity_count × cell_count`, matching what `sys::QueryView`
        // documents. `present` is indexed [term][row] while this walks
        // row-major, so the range loop is the transpose, not something to
        // iterate away.
        self.cells
            .reserve(self.entities.len() * self.cells_plan.len());
        #[allow(clippy::needless_range_loop)]
        for row in 0..self.entities.len() {
            for (i, t) in self.cells_plan.iter().enumerate() {
                self.cells.push(if self.present[i][row] {
                    unsafe { self.staging[i].as_mut_ptr().add(row * t.cell_size) }
                } else {
                    std::ptr::null_mut()
                });
            }
        }
    }

    fn view(&mut self) -> sys::QueryView {
        sys::QueryView {
            cells: self.cells.as_mut_ptr(),
            entities: self.entities.as_ptr(),
            entity_count: self.entities.len(),
            cell_count: self.cells_plan.len(),
        }
    }

    /// Push back only the cells the plugin declared `&mut` **and** actually
    /// changed.
    fn scatter(&self, q: &mut Query<FilteredEntityMut>) {
        // Nothing writable: skip the iteration entirely rather than pay for a
        // second pass over every matched entity.
        if !self.cells_plan.iter().any(Self::is_writable) {
            return;
        }
        for (row, mut e) in q.iter_mut().enumerate() {
            for (i, t) in self.cells_plan.iter().enumerate() {
                if !Self::is_writable(t) || !self.present[i][row] {
                    continue;
                }
                let start = row * t.cell_size;
                let end = start + t.cell_size;
                // The comparison is what keeps change detection meaningful. It
                // costs a memcmp per writable cell and saves a write plus a
                // change-tick bump on every cell the plugin left alone, which is
                // most of them in most frames.
                if self.baseline[i][start..end] == self.staging[i][start..end] {
                    continue;
                }
                write_cell(&mut e, t, &self.staging[i][start..end]);
            }
        }
    }
}

/// Build the Bevy system that services one registered plugin system.
///
/// `user` is carried as `usize` rather than `*mut c_void` so the closure stays
/// `Send + Sync`; it is the plugin's own opaque token and we never dereference
/// it.
fn build_dispatcher(
    world: &mut World,
    plans: Vec<Vec<TermPlan>>,
    resource_plan: Vec<TermPlan>,
    entry: sys::SystemEntry,
    user: usize,
    gate: GenGate,
) -> impl System<In = (), Out = ()> {
    let build_terms = plans.clone();
    // Latched off after a panic. Without this a system that panics does so every
    // frame forever — thousands of identical errors, and the real first one
    // scrolls away. `AtomicBool` rather than `Cell` because a Bevy system must be
    // `Send + Sync`.
    let disabled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    // Resources are declared per-system rather than taken as a blanket
    // `&mut World`, which is what keeps plugin systems scheduling in parallel:
    // two systems touching different resources still have disjoint access.
    // Carries whether the plugin asked to write, because that decides which
    // accessor can reach the value: `get_mut_by_id` refuses an id the system only
    // declared `add_read_by_id` for, and a refusal here is indistinguishable from
    // the resource not existing.
    let mut resource_ids: Vec<(ComponentId, bool)> = Vec::new();
    for term in resource_plan.iter().filter(|t| t.access.is_resource()) {
        let write = term.access == sys::Access::ResWrite;
        match resource_ids.iter_mut().find(|(id, _)| *id == term.id) {
            // Two params naming the same resource: the stronger access wins, so
            // `Res` alongside `ResMut` still resolves.
            Some((_, w)) => *w |= write,
            None => resource_ids.push((term.id, write)),
        }
    }
    let resource_build = resource_plan.clone();

    // A `Vec` of builders produces a `Vec` of params, which is what lifts the
    // old one-query-per-system limit: `SystemParamBuilder` tuples are fixed at
    // compile time, so an arity-N tuple would have meant capping N and
    // generating an impl per arity.
    let query_builders: Vec<_> = build_terms
        .into_iter()
        .map(|terms| {
            QueryParamBuilder::new(move |builder: &mut QueryBuilder<FilteredEntityMut>| {
                build_query(builder, &terms);
            })
        })
        .collect();

    // One builder per system param. The tuple arity here MUST match the
    // closure's parameter count.
    (
        query_builders,
        FilteredResourcesMutParamBuilder::new(move |builder| {
            for t in &resource_build {
                match t.access {
                    sys::Access::ResRead => {
                        builder.add_read_by_id(t.id);
                    }
                    sys::Access::ResWrite => {
                        builder.add_write_by_id(t.id);
                    }
                    _ => {}
                }
            }
        }),
        // `ParamBuilder::resource::<Time>()` rather than bare `ParamBuilder`:
        // `build_state` runs before `build_system`, so nothing has pinned the
        // param type yet and inference stalls on `_: SystemParam`.
        ParamBuilder::resource::<Time>(),
        // Structural changes go through Bevy's own deferred queue, so a plugin
        // spawning mid-iteration is exactly as safe as a Rust system doing it.
        ParamBuilder::of::<Commands>(),
        // Read-only, and declared by every plugin system whether it reads input or
        // not. That costs nothing to schedule — a shared borrow never conflicts —
        // and it avoids the alternative, which is knowing at build time whether the
        // plugin's signature mentions `Input`.
        //
        // `Option`, because a host is not obliged to have input at all: a headless
        // server installs no input plugins, and a test app on `MinimalPlugins` has
        // none either. Requiring it made every plugin system panic wherever it was
        // absent, which is a lot of blast radius for a parameter most systems
        // ignore.
        ParamBuilder::of::<Option<Res<input::PluginInput>>>(),
        // Mesh reading. `Option`, because a headless host has no renderer and so
        // no `Assets<Mesh>` — a plugin there simply never gets geometry back.
        ParamBuilder::of::<Option<Res<Assets<Mesh>>>>(),
        // Read-only, and `Mesh3d` is filter-only across the ABI (a plugin can
        // name it in `With` but never get a data cell for it), so this cannot
        // conflict with the dynamic queries above.
        ParamBuilder::of::<Query<&'static Mesh3d>>(),
        // HTTP delivery. `Option` because a host without an HTTP bridge simply
        // never completes a request, which a plugin sees as "not ready yet".
        ParamBuilder::of::<Option<ResMut<PluginHttpInbox>>>(),
    )
        .build_state(world)
        .build_system(move |mut queries: Vec<Query<FilteredEntityMut>>,
                            mut resources: FilteredResourcesMut,
                            time: Res<Time>,
                            mut commands: Commands,
                            plugin_input: Option<Res<input::PluginInput>>,
                            mesh_assets: Option<Res<Assets<Mesh>>>,
                            mesh_handles: Query<&Mesh3d>,
                            http_inbox: Option<ResMut<PluginHttpInbox>>| {
            if disabled.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            // The plugin that registered this has been reloaded, and a newer
            // build has already registered its replacement. Retiring here rather
            // than unregistering is what keeps hot-reload from costing every
            // build a swappable sub-schedule — see `GenGate`.
            //
            // Checked before the staging buffers are built, so a retired system
            // costs an atomic load and nothing else. It still pays the param
            // fetch Bevy did to call it; that is the accumulating cost.
            if gate.stale() {
                return;
            }
            // Everything the plugin sees lives in staging buffers we own. That
            // costs a copy per cell, but it is what makes the call sound: we
            // never expose a pointer into component storage whose layout the
            // plugin would have to assume. Optimising the `Marshal::Raw` case to
            // a direct pointer is possible later — those layouts ARE the
            // plugin's own — but it needs a careful aliasing argument, so
            // correctness first.
            let mut states: Vec<ViewState> = plans
                .iter()
                .map(|plan| {
                    ViewState::new(
                        plan.iter()
                            .filter(|t| t.access.has_cell())
                            .cloned()
                            .collect(),
                    )
                })
                .collect();

            for (state, q) in states.iter_mut().zip(queries.iter_mut()) {
                state.gather(q);
            }

            // A system with queries but no rows has nothing to say. One with no
            // queries at all — `fn(Commands)` — still runs, because its whole
            // job is the side effect.
            if !states.is_empty() && states.iter().all(|s| s.entities.is_empty()) {
                return;
            }

            let views: Vec<sys::QueryView> = states.iter_mut().map(ViewState::view).collect();

            // Resolved once per call rather than per access: a system may read
            // the same resource from several parameters, and each `get_mut_by_id`
            // takes a fresh borrow.
            let mut slots: Vec<sys::ResourceSlot> = Vec::with_capacity(resource_ids.len());
            for (id, write) in &resource_ids {
                let ptr = if *write {
                    resources
                        .get_mut_by_id(*id)
                        .map(|mut m| m.as_mut().as_ptr())
                        .unwrap_or(std::ptr::null_mut())
                } else {
                    // Cast away const: the slot is a plain address, and only
                    // `ResMut` — which requires the write branch above — ever
                    // hands out a `&mut` to it.
                    resources
                        .get_by_id(*id)
                        .map(|p| p.as_ptr())
                        .unwrap_or(std::ptr::null_mut())
                };
                slots.push(sys::ResourceSlot {
                    id: sys::ComponentId(id.index() as u32),
                    ptr,
                });
            }

            let mut http_src = HttpSourceImpl {
                src: sys::HttpSource { poll: http_poll },
                inbox: http_inbox.map(|i| i.into_inner()),
            };
            let mut mesh_src = MeshSourceImpl {
                src: sys::MeshSource { read: mesh_read },
                assets: mesh_assets.as_deref(),
                handles: &mesh_handles,
            };
            let mut sink = SinkImpl {
                sink: sys::CommandSink {
                    reserve_entity: sink_reserve,
                    push: sink_push,
                },
                commands: &mut commands,
                queued: Vec::new(),
            };
            let call = sys::SystemCall {
                views: views.as_ptr(),
                view_count: views.len(),
                frame: sys::FrameCtx {
                    delta_secs: time.delta_secs(),
                    elapsed_secs: time.elapsed_secs(),
                },
                user: user as *mut c_void,
                iface: &IFACE,
                // Deliberately null: a `Host` handle only means something during
                // init, when the host holds `&mut World`. While this system runs
                // the world is borrowed by the query, so the init-time pointer
                // would be dangling — handing it over was a trap waiting for the
                // first plugin that called back.
                host: core::ptr::null_mut(),
                commands: (&mut sink as *mut SinkImpl).cast(),
                resources: slots.as_ptr(),
                resource_count: slots.len(),
                // Borrowed from the resource, which lives in the world and so
                // outlives the call. Never a temporary: a pointer to one would
                // dangle the moment this struct was built. Null when the host has
                // no input, which the guest turns into "nothing is pressed".
                input: plugin_input
                    .as_ref()
                    .map_or(core::ptr::null(), |i| &i.0 as *const sys::InputState),
                meshes: (&mut mesh_src as *mut MeshSourceImpl).cast(),
                http: (&mut http_src as *mut HttpSourceImpl).cast(),
            };

            // SAFETY: `entry` came from a `dlopen`'d library the loader keeps
            // alive for the process lifetime, and every pointer in `call` points
            // at a buffer that outlives this statement.
            let status = unsafe { entry(&call) };
            let queued = std::mem::take(&mut sink.queued);
            if status == sys::SystemStatus::Panicked {
                error!("[plugin] system panicked — disabling it for this session");
                disabled.store(true, std::sync::atomic::Ordering::Relaxed);
                // Skip write-back: the plugin's partial output is not something
                // to trust into the world.
                return;
            }

            apply_queued(&mut commands, queued);

            for (state, q) in states.iter().zip(queries.iter_mut()) {
                state.scatter(q);
            }
        })
}

/// Copy one component out of storage into the plugin-facing representation.
///
/// `None` means the entity does not have it, which only happens for an optional
/// term — a required one was a precondition of matching the query.
fn read_cell(e: &FilteredEntityRef, t: &TermPlan) -> Option<Vec<u8>> {
    match t.marshal {
        Marshal::Transform => {
            let src = *e.get::<Transform>()?;
            let m = to_mirror(&src);
            // SAFETY: `sys::Transform` is `#[repr(C)]` and plain-old-data.
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    (&m as *const sys::Transform).cast::<u8>(),
                    size_of::<sys::Transform>(),
                )
            }
            .to_vec();
            Some(bytes)
        }
        // SAFETY: presence was just checked, and the component occupies
        // `cell_size` bytes because that is where the size came from.
        Marshal::Raw => e
            .get_by_id(t.id)
            .map(|ptr| unsafe { std::slice::from_raw_parts(ptr.as_ptr(), t.cell_size).to_vec() }),
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

/// Every resource a plugin registered, so the editor can list and inspect them.
#[derive(bevy::prelude::Resource, Default)]
pub struct PluginResources(pub Vec<ComponentId>);

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
    /// Editing range, if the plugin gave one. `None` means an unbounded drag,
    /// which is what every field had before ranges existed.
    pub range: Option<sys::FieldRange>,
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
    /// Whether this is a resource rather than a component.
    ///
    /// Both share this list because they share a registry in Bevy, and the
    /// schema is what draws editable rows either way. The editor must still tell
    /// them apart: a resource has no entity to sit on, so offering it in Add
    /// Component would put a second copy of a global on some arbitrary entity.
    pub is_resource: bool,
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
    match s {
        sys::Schedule::First => First.intern(),
        sys::Schedule::PreUpdate => PreUpdate.intern(),
        sys::Schedule::Update => Update.intern(),
        sys::Schedule::PostUpdate => PostUpdate.intern(),
        sys::Schedule::Last => Last.intern(),
        // A schedule this build does not run, from a plugin built against a
        // newer ABI. `Update` is the least surprising home: the system runs at
        // the wrong time rather than not at all, and the warning says so.
        // Silently dropping it would present as "my plugin loaded and does
        // nothing", which is the hardest failure to diagnose.
        other => {
            warn!(
                "plugin asked for schedule {} which this build does not have —                  running it in Update instead",
                other.0
            );
            Update.intern()
        }
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
    init_plugin_gen(world, init, PluginGeneration::default(), 0, usize::MAX)
}

/// Initialise a plugin as a numbered reload of a slot.
///
/// `counter`/`generation` are the slot's shared reload counter and the value it
/// holds for this load. Every system registered during this call captures them and
/// retires itself once the counter moves on — see [`GenGate`].
pub fn init_plugin_gen(
    world: &mut World,
    init: sys::ExtensionInit,
    counter: PluginGeneration,
    generation: u32,
    slot: usize,
) -> sys::InitResult {
    // MUST be the `'static` table, not `interface()`. A plugin stores this
    // pointer so its render callbacks can reach the interface on later frames;
    // handing it a stack local leaves it dangling the moment this returns, and
    // the next `render_set_pipeline` reads a garbage function pointer. Systems
    // were unaffected because they get their interface from `SystemCall::iface`.
    let mut ctx = HostCtx {
        world,
        gate: GenGate {
            counter,
            at: generation,
        },
        slot,
        layout_conflict: false,
    };
    let result = unsafe { init(&IFACE, (&mut ctx as *mut HostCtx).cast()) };
    // A layout change is only discoverable once the plugin registers, i.e. part
    // way through init. Reporting failure here is what makes it a no-op: the
    // loader leaves the generation counter alone, so this build's systems are
    // permanently stale and the previous build carries on running.
    if ctx.layout_conflict {
        return sys::InitResult::Failed;
    }
    result
}
