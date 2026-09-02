//! The function table itself, and the registration calls a plugin makes during
//! `renzora_plugin_init`.
//!
//! [`IFACE`] is a `static` on purpose and it is load-bearing: a plugin stores
//! the pointer so its render callbacks can reach the interface on later frames,
//! and handing it a stack local would leave it dangling the moment init
//! returns. `prefix_hashes` points into another `static` for the same reason.
//!
//! Every function here is `extern "C"` and therefore wrapped in
//! [`guard_host`] — a panic that unwound out of one of these would abort the
//! process rather than fail the call.
//!
//! The extern fns for assets and rendering live in [`super::assets`] and
//! [`super::render`] with the types they build; the table below is what binds
//! all of them into one ABI surface, so it stays whole in one place.

use bevy::ecs::component::{ComponentDescriptor, ComponentId, StorageType};
use bevy::ecs::schedule::Schedules;
use bevy::prelude::*;
use std::alloc::Layout;

use crate::sys;

use super::assets::{add_image, add_material, add_mesh, add_mesh_data};
use super::query::{build_dispatcher, build_plan};
use super::reload::{guard_host, verify_same_layout, HostCtx};
use super::render::{
    add_material_shader, add_post_process, add_render_pass, render_draw, render_set_pipeline,
    PluginAudioBackend, PluginAudioBackendEntry, PluginNetBackend, PluginNetBackendEntry,
    PluginPanel, PluginPanels, PluginScriptBackend, PluginScriptBackends,
};
use super::schema::{
    bevy_label, lookup_component, PluginComponentInfo, PluginComponentSchemas, PluginComponents,
    PluginField, PluginResources,
};

/// A `'static` copy of the table, so a running system can be handed a pointer to
/// it that outlives the frame. Safe to share because every field is a plain `fn`
/// item — there is no state to race on.
pub(crate) static IFACE: sys::Interface = sys::Interface {
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
    add_material_shader,
    add_image,
    // Points at a `static`, so this is valid for the life of the process — which
    // matters because a plugin may read it at any point during its init.
    prefix_hashes: sys::INTERFACE_PREFIX_HASHES.as_ptr(),
    prefix_count: sys::INTERFACE_PREFIX_HASHES.len(),
    add_script_backend,
    add_audio_backend,
    add_settings_section,
    add_net_backend,
};

// ── Interface implementations ────────────────────────────────────────────────

/// Bytes a field of `kind` occupies, or 0 if this build has no idea.
///
/// Zero for an unknown kind is what makes the bounds check above refuse rather
/// than guess: a `FieldKind` from a newer ABI has no size this build can know,
/// and the consumers that would otherwise read it default to four bytes at an
/// offset nothing measured.
fn field_width(kind: sys::FieldKind) -> usize {
    match kind {
        sys::FieldKind::F32 | sys::FieldKind::I32 => 4,
        sys::FieldKind::Bool => 1,
        sys::FieldKind::Vec3 => size_of::<sys::Vec3>(),
        sys::FieldKind::Quat => size_of::<sys::Quat>(),
        sys::FieldKind::Str => size_of::<sys::Str256>(),
        _ => 0,
    }
}

unsafe extern "C" fn register_component(
    host: *mut sys::Host,
    desc: *const sys::ComponentDesc,
) -> sys::ComponentId {
    guard_host("register_component", sys::ComponentId::INVALID, || {
    let ctx = &mut *(host as *mut HostCtx);
    let desc = &*desc;
    let name = desc.name.as_str().to_string();

    // Refused rather than ignored: a component with a destructor whose drop is
    // never run leaks whatever it owns, silently, for the life of the process.
    //
    // **Before the early return below, deliberately.** This used to sit after it,
    // so a reload — which takes the re-registration path — skipped the check
    // entirely, and the one moment a plugin's layout can legitimately change was
    // the one moment nothing looked. The derive now refuses these at compile time
    // too; this stays because the ABI is public and a hand-written `Component`
    // impl reaches here without passing through the derive at all.
    if desc.drop.is_some() {
        error!(
            "plugin component `{name}` declares a destructor, which is not supported yet — \
             keep plugin components plain data (no String, Vec or Box fields)"
        );
        return sys::ComponentId::INVALID;
    }

    // Re-registering the same name must return the same id: a plugin reloaded
    // mid-session would otherwise get a second component and silently stop
    // matching the entities carrying the first.
    if let Some(existing) = lookup_component(ctx.world, &name) {
        verify_same_layout(ctx, existing, desc, &name);
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
    //
    // Every field is bounds-checked against the component's own size here, at the
    // one moment the whole schema is in front of us. Downstream consumers — the
    // inspector, the scene writer — read and write `width` bytes at `offset` into
    // live component storage, and an offset past the end is an out-of-bounds
    // access with the component's own allocation as the base. That is reachable
    // from a plugin built against a newer ABI: an unknown `FieldKind` is not
    // something those consumers can size, and the field is trusted rather than
    // measured.
    //
    // A bad field is dropped rather than refusing the whole component, because a
    // component that registers with one field missing is a visibly wrong
    // inspector row, while one that refuses to register at all is a plugin that
    // silently does nothing.
    let fields = if desc.fields.is_null() {
        Vec::new()
    } else {
        std::slice::from_raw_parts(desc.fields, desc.field_count)
            .iter()
            .filter(|f| {
                // A kind this build cannot size is KEPT. That is deliberate and
                // has its own test: the schema is data, and dropping a field the
                // inspector cannot draw would silently change the component's
                // shape for the scene writer too. Consumers are responsible for
                // skipping what they cannot read — which is the half of this that
                // was actually broken.
                let width = field_width(f.kind);
                if width == 0 {
                    return true;
                }
                let fits = f.offset.saturating_add(width) <= desc.size;
                if !fits {
                    error!(
                        "plugin component `{}` field `{}` is {width} bytes at offset {} but the \
                         component is only {} — dropping the field",
                        desc.name.as_str(),
                        f.name.as_str(),
                        f.offset,
                        desc.size
                    );
                }
                fits
            })
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
        // Same reasoning as `register_component`, and it has to be repeated
        // rather than inherited: the `Some(id)` arm below never calls that
        // function, so a resource that owns memory would sail past on every run
        // after the first.
        if (*desc).drop.is_some() {
            error!(
                "plugin resource `{}` declares a destructor, which is not supported yet — \
                 keep plugin resources plain data (no String, Vec or Box fields)",
                (*desc).name.as_str()
            );
            return sys::ComponentId::INVALID;
        }

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
            settings: false,
        });
        sys::RegisterStatus::Ok
    })
}

/// Register a Settings-overlay section. Backs [`sys::Interface::add_settings_section`].
///
/// Deliberately the same body as `add_panel` bar the flag, including the
/// duplicate-id refusal: ids are one namespace across panels AND sections
/// because `set_panel_content` resolves against one list, so a section sharing
/// an id with a panel would have its content applied to the wrong one.
unsafe extern "C" fn add_settings_section(
    host: *mut sys::Host,
    desc: *const sys::PanelDesc,
) -> sys::RegisterStatus {
    guard_host("add_settings_section", sys::RegisterStatus::Invalid, || {
        if desc.is_null() {
            return sys::RegisterStatus::Invalid;
        }
        let desc = &*desc;
        let id = desc.id.as_str().to_string();
        if id.is_empty() || desc.markup.as_str().is_empty() {
            error!("plugin registered a settings section with no id or no markup");
            return sys::RegisterStatus::Invalid;
        }

        let ctx = &mut *(host as *mut HostCtx);
        let owner = ctx.slot;
        let mut panels = ctx
            .world
            .get_resource_or_insert_with(PluginPanels::default);
        if panels.0.iter().any(|p| p.id == id) {
            error!("two plugins registered `{id}` — the second is ignored");
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
            category: desc.category.as_str().to_string(),
            markup: desc.markup.as_str().to_string(),
            on_action: desc.on_action,
            user: desc.user as usize,
            settings: true,
        });
        sys::RegisterStatus::Ok
    })
}

unsafe extern "C" fn add_script_backend(
    host: *mut sys::Host,
    desc: *const sys::ScriptBackendDesc,
) -> sys::RegisterStatus {
    guard_host("add_script_backend", sys::RegisterStatus::Invalid, || {
        if desc.is_null() {
            return sys::RegisterStatus::Invalid;
        }
        let desc = &*desc;
        let name = desc.name.as_str().to_string();
        if name.is_empty() {
            error!("plugin registered a script backend with no name");
            return sys::RegisterStatus::Invalid;
        }
        if desc.extensions.is_null() || desc.extension_count == 0 {
            error!("script backend `{name}` claims no file extensions, so nothing would route to it");
            return sys::RegisterStatus::Invalid;
        }

        // Copy now. The descriptor may point at a plugin stack local, and it
        // certainly points at plugin memory that a hot reload would unmap.
        let raw = std::slice::from_raw_parts(desc.extensions, desc.extension_count);
        let extensions: Vec<String> = raw
            .iter()
            .map(|e| e.as_str().trim_start_matches('.').to_ascii_lowercase())
            .filter(|e| !e.is_empty())
            .collect();
        if extensions.is_empty() {
            error!("script backend `{name}` claims only empty file extensions");
            return sys::RegisterStatus::Invalid;
        }

        let ctx = &mut *(host as *mut HostCtx);
        let owner = ctx.slot;
        let mut backends = ctx
            .world
            .get_resource_or_insert_with(PluginScriptBackends::default);

        // First claim wins. Two backends fighting over `.lua` would make which
        // interpreter runs a script depend on plugin load order, which is
        // directory iteration order — so the same project would behave
        // differently on two machines.
        let taken: Vec<&str> = extensions
            .iter()
            .filter(|e| backends.0.iter().any(|b| b.extensions.contains(e)))
            .map(String::as_str)
            .collect();
        if !taken.is_empty() {
            error!(
                "script backend `{name}` claims .{} which another backend already handles — \
                 it is ignored",
                taken.join(", .")
            );
            return sys::RegisterStatus::Invalid;
        }

        info!(
            "[scripting] backend `{name}` handles .{}",
            extensions.join(", .")
        );
        backends.0.push(PluginScriptBackend {
            name,
            extensions,
            entry: desc.entry,
            owner,
        });
        sys::RegisterStatus::Ok
    })
}

unsafe extern "C" fn add_audio_backend(
    host: *mut sys::Host,
    desc: *const sys::AudioBackendDesc,
) -> sys::RegisterStatus {
    guard_host("add_audio_backend", sys::RegisterStatus::Invalid, || {
        if desc.is_null() {
            return sys::RegisterStatus::Invalid;
        }
        let desc = &*desc;
        let name = desc.name.as_str().to_string();
        if name.is_empty() {
            error!("plugin registered an audio backend with no name");
            return sys::RegisterStatus::Invalid;
        }

        let ctx = &mut *(host as *mut HostCtx);
        let owner = ctx.slot;
        let mut backend = ctx
            .world
            .get_resource_or_insert_with(PluginAudioBackend::default);

        // First claim wins, and unlike scripting there is no key to share. Two
        // language plugins coexist because a script names one by its file
        // extension; two audio backends would both open the default output
        // device and the user would hear both mixes at once.
        if let Some(existing) = &backend.0 {
            error!(
                "audio backend `{name}` is ignored — `{}` is already registered, and there is                  only one pair of speakers",
                existing.name
            );
            return sys::RegisterStatus::Invalid;
        }

        info!("[audio] backend `{name}` registered");
        backend.0 = Some(PluginAudioBackendEntry {
            name,
            state: desc.state as usize,
            entry: desc.entry,
            owner,
        });
        sys::RegisterStatus::Ok
    })
}

unsafe extern "C" fn add_net_backend(
    host: *mut sys::Host,
    desc: *const sys::NetBackendDesc,
) -> sys::RegisterStatus {
    guard_host("add_net_backend", sys::RegisterStatus::Invalid, || {
        if desc.is_null() {
            return sys::RegisterStatus::Invalid;
        }
        let desc = &*desc;
        let name = desc.name.as_str().to_string();
        if name.is_empty() {
            error!("plugin registered a network backend with no name");
            return sys::RegisterStatus::Invalid;
        }

        let ctx = &mut *(host as *mut HostCtx);
        let owner = ctx.slot;
        let mut backend = ctx
            .world
            .get_resource_or_insert_with(PluginNetBackend::default);

        // First claim wins, as with audio. Scripting can hold several because a
        // script names its language by file extension; a request carries no
        // such key, and two clients would each hold half of one session's
        // cookies and connection pool.
        if let Some(existing) = &backend.0 {
            error!(
                "network backend `{name}` is ignored — `{}` is already registered",
                existing.name
            );
            return sys::RegisterStatus::Invalid;
        }

        info!("[net] backend `{name}` registered");
        backend.0 = Some(PluginNetBackendEntry {
            name,
            state: desc.state as usize,
            entry: desc.entry,
            owner,
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
pub(crate) unsafe fn write_resource_bytes(world: &mut World, id: ComponentId, bytes: &[u8]) {
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
