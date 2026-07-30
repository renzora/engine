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
            Some(id) => sys::ComponentId(id.index() as u32),
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
        if desc.queries.is_null() || desc.query_count == 0 || desc.flags != 0 {
            error!("plugin sent a malformed SystemDesc");
            return sys::RegisterStatus::Invalid;
        }

        let mut plans = Vec::with_capacity(desc.query_count);
        for q in std::slice::from_raw_parts(desc.queries, desc.query_count) {
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

        let system = build_dispatcher(ctx.world, plans, res_plan, desc.entry, desc.user as usize);
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
    pub meshes: Vec<Handle<Mesh>>,
    pub materials: Vec<Handle<StandardMaterial>>,
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
        let mut store = ctx
            .world
            .get_resource_or_insert_with(PluginAssets::default);
        store.meshes.push(handle);
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
        let mut store = ctx
            .world
            .get_resource_or_insert_with(PluginAssets::default);
        store.materials.push(handle);
        sys::AssetHandle((store.materials.len() - 1) as u64)
    })
}

// ── Commands ─────────────────────────────────────────────────────────────────

/// Backs `sys::CommandSink` for one system invocation.
///
/// `sink` must be the FIRST field: the plugin holds a `*mut CommandSink` and the
/// host casts it back to this, so the two must share an address.
#[repr(C)]
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
                        let m = store.meshes.get(d.mesh.0 as usize).cloned();
                        let mat = store.materials.get(d.material.0 as usize).cloned();
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
            // A command kind from a newer ABI. Dropping it is the only
            // option: what the payload means is exactly the thing this build
            // does not know.
            other => {
                warn!("plugin queued command kind {} which this build does not have", other.0);
            }
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
    pub phase: sys::RenderPhase,
    pub order: f32,
    pub callback: sys::RenderCallback,
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
            id,
            shader: handle,
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
    )
        .build_state(world)
        .build_system(move |mut queries: Vec<Query<FilteredEntityMut>>,
                            mut resources: FilteredResourcesMut,
                            time: Res<Time>,
                            mut commands: Commands| {
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
    // MUST be the `'static` table, not `interface()`. A plugin stores this
    // pointer so its render callbacks can reach the interface on later frames;
    // handing it a stack local leaves it dangling the moment this returns, and
    // the next `render_set_pipeline` reads a garbage function pointer. Systems
    // were unaffected because they get their interface from `SystemCall::iface`.
    let mut ctx = HostCtx { world };
    unsafe { init(&IFACE, (&mut ctx as *mut HostCtx).cast()) }
}
