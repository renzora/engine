//! What the host records about plugin-owned types, and the name-based lookup
//! that lets a plugin address host types it never linked.
//!
//! A plugin has no `TypeId` for anything the host owns, so **the string is the
//! shared identity** — which is why renaming a component type breaks plugins
//! exactly like it breaks saved scenes, and why resolution goes through the
//! reflection registry rather than `ComponentInfo::name()`.
//!
//! [`HostDataComponents`] is the other half: filtering by a host component is
//! unrestricted and free, but *reading its bytes* is allowlisted, because a
//! mirror the plugin wrote is matched by name and nothing checks that its
//! layout agrees.

use bevy::ecs::component::ComponentId;
use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;

use crate::sys;

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

/// Host components a plugin may read and write as **data**, by type path, with
/// the host-side size each one is expected to have.
///
/// Filtering is unrestricted — `With<Camera3d>` works for anything the engine
/// registered for reflection, and costs nothing because a filter term produces no
/// cell. *Data* is the dangerous direction, and it was unrestricted too.
///
/// A plugin declares a mirror of a host component with `host_component!`, naming
/// it by string, and then reads it as plain bytes. Nothing checked that the
/// component was safe to hand over that way. Two things went wrong with that:
///
/// - **Owned memory.** `bevy_window::window::Window` contains a `String`. A plugin
///   could name it, be handed its bytes, and follow a pointer into the engine's
///   heap. The no-destructor rule the derive now enforces covers a plugin's *own*
///   types and never covered these.
/// - **Layouts nobody promised.** A mirror is matched by name; field order and
///   size are the author's problem, and a mismatch is a wrong-offset read rather
///   than a compile error. Worse, some engine types have no stable layout to
///   mirror at all — `GlobalTransform` wraps a `glam::Affine3A`, whose
///   representation changes with the SIMD backend the engine was built with.
///
/// So a host component is readable as data only if something deliberately said so.
///
/// Entries come from the crate that owns the type — `renzora_animation` exposes
/// its own `PluginAnimState` — for the same reason bridges live there: this crate
/// must not learn the name of every domain in the engine.
///
/// **This restricts; it does not verify.** Nothing here can check that a plugin's
/// mirror actually matches the host type, because the plugin never sends its
/// mirror's size or layout — `component_id_by_name` carries a name and nothing
/// else. So an author who exposes a type here is promising the layout is stable
/// and documented, and an author who mirrors it is still responsible for getting
/// the fields right. Closing that second gap needs a size beside the name in the
/// ABI, which is a MINOR bump nobody has spent yet.
#[derive(Resource, Default)]
pub struct HostDataComponents {
    /// Readable as `&T`.
    pub readable: std::collections::HashSet<String>,
    /// Writable as `&mut T`. Always a subset of [`Self::readable`].
    ///
    /// Separate because the two have different blast radii, and the write one is
    /// worse. A mirror that is *smaller* than the host type reads bytes it should
    /// not see; a mirror that is *larger* writes past the end of its staging row,
    /// into the host's own buffer. Nothing in the ABI carries the plugin's mirror
    /// size, so neither can be detected — only permitted or not.
    ///
    /// Most mirrors want read only. `AnimState` and `PhysicsState` are state a
    /// plugin observes, not state it sets: an engine system owns them and
    /// overwrites them every frame, so a plugin write would be discarded anyway.
    pub writable: std::collections::HashSet<String>,
}

/// Let plugins read `T` as data, by its reflected type path.
///
/// For an in-tree crate exposing a mirror it owns. `T` must be `#[repr(C)]` plain
/// data with no destructor and no layout that varies with build configuration —
/// the whole point of the list is that somebody checked.
pub fn expose_component_data<T: Component + bevy::reflect::TypePath>(app: &mut App) {
    let path = <T as bevy::reflect::TypePath>::type_path().to_string();
    app.world_mut()
        .get_resource_or_insert_with(HostDataComponents::default)
        .readable
        .insert(path);
}

/// Let plugins **write** `T` as well as read it.
///
/// Deliberately separate, and deliberately more work to say. Reading a mirror
/// that disagrees with the host type is a wrong value; writing one is a write
/// past the end of the host's staging row, and no size crosses the ABI for
/// either to be checked against.
///
/// Only reach for this when a plugin is meant to *own* the value. A mirror an
/// engine system rewrites every frame — which is what most of them are — should
/// stay read-only, because a plugin's write to one is discarded anyway.
pub fn expose_component_data_mut<T: Component + bevy::reflect::TypePath>(app: &mut App) {
    let path = <T as bevy::reflect::TypePath>::type_path().to_string();
    let mut allowed = app
        .world_mut()
        .get_resource_or_insert_with(HostDataComponents::default);
    allowed.readable.insert(path.clone());
    allowed.writable.insert(path);
}

/// A component's reflected type path, which is the name plugins address it by.
///
/// The inverse of [`lookup_component`], and it must go through the same registry
/// rather than `ComponentInfo::name()`: that returns a `DebugName` whose string
/// exists only under bevy_utils' `debug` feature, so in a normal build it is a
/// fixed placeholder identical for every component.
///
/// `None` for a component with no reflected type — a plugin-owned one, or an
/// engine type nothing registered.
pub(crate) fn component_type_path(world: &World, id: ComponentId) -> Option<String> {
    let type_id = world.components().get_info(id)?.type_id()?;
    let registry = world.get_resource::<AppTypeRegistry>()?;
    let registry = registry.read();
    let path = registry.get(type_id)?.type_info().type_path().to_string();
    Some(path)
}

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
pub(crate) fn lookup_component(world: &World, name: &str) -> Option<ComponentId> {
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

pub(crate) fn bevy_label(s: sys::Schedule) -> impl ScheduleLabel {
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
