//! Inspector sections for components the engine has no Rust type for.
//!
//! A plugin component is registered by layout, so it has no `TypeRegistration`
//! and the reflection-driven inspector cannot see it. What it does have is a
//! field schema — name, [`FieldKind`], byte offset — collected at load time.
//! This module turns that into a normal-looking component section: `Spinner`
//! appears as "Spinner", with its own header, trash button and editable rows,
//! exactly like a built-in.
//!
//! ## The slot pool, and why it exists
//!
//! Every extension point in `InspectorRegistry` is a bare `fn` pointer with no
//! per-entry state: `has_fn(&World, Entity)`, `remove_fn(&mut World, Entity)`,
//! `NativeInspectorDrawer(&mut World, Entity)`. None of them receives *which*
//! component it is being called for. For a built-in that is fine — the `fn` is
//! written against a concrete type. A plugin component's identity only exists at
//! runtime, so there is nothing to write the `fn` against.
//!
//! The way out is const generics: `has_slot::<0>` and `has_slot::<1>` are
//! distinct fn *items*, so they have distinct fn *pointers*, and each reads its
//! own entry from [`SLOTS`]. A plugin component claims a slot when it registers
//! and from then on has a genuine per-component `fn` trio.
//!
//! The cost is a fixed ceiling ([`MAX_PLUGIN_COMPONENTS`]). The alternative was
//! threading a context parameter through `InspectorEntry`, which would have
//! touched every one of the ~50 existing registrations to serve a case none of
//! them care about.

use bevy::ecs::component::ComponentId;
use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use core::sync::atomic::{AtomicU32, Ordering};
use renzora_ember::font::EmberFonts;
use renzora_plugin::host::PluginComponentSchemas;
use renzora_plugin::sys::FieldKind;

/// How many plugin components can have their own inspector section.
///
/// Raising it is mechanical — extend the `slots!` list below. It is a ceiling on
/// *distinct component types across all loaded plugins*, not on instances.
/// 32 was enough while the post-process effects were Bevy-linking dylibs with
/// their own inspector path. Moving ~45 of them onto the C ABI put every one of
/// their settings structs through here at once and blew straight past it, so the
/// components registered after the 32nd silently lost their inspector.
pub const MAX_PLUGIN_COMPONENTS: usize = 96;

const EMPTY: u32 = u32::MAX;

/// `ComponentId` index per slot, or [`EMPTY`]. Written once at registration and
/// only read afterwards.
static SLOTS: [AtomicU32; MAX_PLUGIN_COMPONENTS] =
    [const { AtomicU32::new(EMPTY) }; MAX_PLUGIN_COMPONENTS];

fn slot_component(n: usize) -> Option<ComponentId> {
    match SLOTS[n].load(Ordering::Relaxed) {
        EMPTY => None,
        i => Some(ComponentId::new(i as usize)),
    }
}

fn has_slot<const N: usize>(world: &World, entity: Entity) -> bool {
    let Some(cid) = slot_component(N) else {
        return false;
    };
    world.get_entity(entity).is_ok_and(|e| e.contains_id(cid))
}

fn remove_slot<const N: usize>(world: &mut World, entity: Entity) {
    let Some(cid) = slot_component(N) else {
        return;
    };
    if let Ok(mut e) = world.get_entity_mut(entity) {
        e.remove_by_id(cid);
    }
}

fn draw_slot<const N: usize>(world: &mut World, entity: Entity) -> Entity {
    match slot_component(N) {
        Some(cid) => draw_component(world, entity, cid),
        None => world.spawn(Node::default()).id(),
    }
}

/// Materialise one `fn` trio per slot. The literals are the const-generic
/// arguments, so each expansion is a distinct fn item with its own pointer.
macro_rules! slots {
    ($($n:literal),* $(,)?) => {
        static HAS: [fn(&World, Entity) -> bool; MAX_PLUGIN_COMPONENTS] = [$(has_slot::<$n>),*];
        static REMOVE: [fn(&mut World, Entity); MAX_PLUGIN_COMPONENTS] = [$(remove_slot::<$n>),*];
        static DRAW: [fn(&mut World, Entity) -> Entity; MAX_PLUGIN_COMPONENTS] =
            [$(draw_slot::<$n>),*];
    };
}

slots!(
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
    25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47,
    48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70,
    71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93,
    94, 95,
);

// ── Raw field access ─────────────────────────────────────────────────────────

/// Read one field out of a plugin component's bytes.
///
/// Unaligned because the offset came from `offset_of!` in the plugin: correct
/// for that type's layout, but read here through a `*const u8` that carries no
/// alignment guarantee of its own.
fn read_f32(world: &World, entity: Entity, cid: ComponentId, offset: usize) -> f32 {
    world
        .get_entity(entity)
        .ok()
        .and_then(|e| e.get_by_id(cid).ok())
        .map(|p| unsafe { p.as_ptr().add(offset).cast::<f32>().read_unaligned() })
        .unwrap_or(0.0)
}

fn write_f32(world: &mut World, entity: Entity, cid: ComponentId, offset: usize, v: f32) {
    if let Ok(mut e) = world.get_entity_mut(entity) {
        if let Ok(mut ptr) = e.get_mut_by_id(cid) {
            // `as_mut()` marks the component changed, which is what makes the
            // plugin's systems and the render extraction see the edit.
            unsafe {
                ptr.as_mut()
                    .as_ptr()
                    .add(offset)
                    .cast::<f32>()
                    .write_unaligned(v)
            };
        }
    }
}

/// A `bool` is ONE byte, and reading it as an `i32` reads three bytes that are
/// not part of the field. `struct Flags { a: bool, b: bool }` is two bytes with
/// align 1, so a 4-byte write at offset 0 scribbles over `b` and two bytes of
/// whatever the allocator put next — which surfaces as an unrelated component
/// changing when you toggle a checkbox.
fn read_bool(world: &World, entity: Entity, cid: ComponentId, offset: usize) -> bool {
    world
        .get_entity(entity)
        .ok()
        .and_then(|e| e.get_by_id(cid).ok())
        .map(|p| unsafe { p.as_ptr().add(offset).read() != 0 })
        .unwrap_or(false)
}

fn write_bool(world: &mut World, entity: Entity, cid: ComponentId, offset: usize, v: bool) {
    if let Ok(mut e) = world.get_entity_mut(entity) {
        if let Ok(mut ptr) = e.get_mut_by_id(cid) {
            unsafe { ptr.as_mut().as_ptr().add(offset).write(v as u8) };
        }
    }
}

/// Read a `sys::Str256` out of component storage.
///
/// The length is clamped rather than trusted: it was written by a plugin, and
/// an over-long one would slice past the field into whatever the plugin
/// declared next.
fn read_str(world: &World, entity: Entity, cid: ComponentId, offset: usize) -> String {
    world
        .get_entity(entity)
        .ok()
        .and_then(|e| e.get_by_id(cid).ok())
        .map(|p| unsafe {
            let base = p.as_ptr().add(offset);
            let len = base
                .add(renzora_plugin::sys::STR_CAP)
                .cast::<u32>()
                .read_unaligned() as usize;
            let len = len.min(renzora_plugin::sys::STR_CAP);
            let bytes = std::slice::from_raw_parts(base, len);
            String::from_utf8_lossy(bytes).into_owned()
        })
        .unwrap_or_default()
}

/// Write a `sys::Str256`, truncating at a character boundary.
///
/// Truncating rather than refusing, unlike the plugin-side constructor: this is
/// a text box, and dropping a keystroke because a paste was too long is worse
/// than keeping what fits. The whole field is zeroed first so a shorter string
/// leaves no tail of the previous one behind.
fn write_str(world: &mut World, entity: Entity, cid: ComponentId, offset: usize, v: &str) {
    let cap = renzora_plugin::sys::STR_CAP;
    let mut end = v.len().min(cap);
    while end > 0 && !v.is_char_boundary(end) {
        end -= 1;
    }
    if let Ok(mut e) = world.get_entity_mut(entity) {
        if let Ok(mut ptr) = e.get_mut_by_id(cid) {
            unsafe {
                let base = ptr.as_mut().as_ptr().add(offset);
                std::ptr::write_bytes(base, 0, cap + 4);
                std::ptr::copy_nonoverlapping(v.as_ptr(), base, end);
                base.add(cap).cast::<u32>().write_unaligned(end as u32);
            }
        }
    }
}

fn read_i32(world: &World, entity: Entity, cid: ComponentId, offset: usize) -> i32 {
    world
        .get_entity(entity)
        .ok()
        .and_then(|e| e.get_by_id(cid).ok())
        .map(|p| unsafe { p.as_ptr().add(offset).cast::<i32>().read_unaligned() })
        .unwrap_or(0)
}

fn write_i32(world: &mut World, entity: Entity, cid: ComponentId, offset: usize, v: i32) {
    if let Ok(mut e) = world.get_entity_mut(entity) {
        if let Ok(mut ptr) = e.get_mut_by_id(cid) {
            unsafe {
                ptr.as_mut()
                    .as_ptr()
                    .add(offset)
                    .cast::<i32>()
                    .write_unaligned(v)
            };
        }
    }
}

// ── Drawing ──────────────────────────────────────────────────────────────────

/// Draw the rows for one plugin component.
fn draw_component(world: &mut World, entity: Entity, cid: ComponentId) -> Entity {
    let fonts = world.resource::<EmberFonts>().clone();

    // Snapshot schema AND current values before `Commands` borrows the world
    // mutably — widgets need a seed at construction time.
    type Row = (String, FieldKind, usize, f32, Option<renzora_plugin::sys::FieldRange>);
    let fields: Vec<Row> = world
        .get_resource::<PluginComponentSchemas>()
        .and_then(|s| s.0.iter().find(|i| i.id == cid))
        .map(|i| {
            i.fields
                .iter()
                .map(|f| (f.name.clone(), f.kind, f.offset, f.range))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
        .into_iter()
        .map(|(name, kind, offset, range)| {
            let v = match kind {
                FieldKind::Bool => read_bool(world, entity, cid, offset) as i32 as f32,
                FieldKind::I32 => read_i32(world, entity, cid, offset) as f32,
                FieldKind::F32 => read_f32(world, entity, cid, offset),
                // Anything else reads nothing. `Vec3`, `Quat` and `Str` are drawn
                // by their own rows further down, and a kind from a newer ABI has
                // no size this build knows — so the previous `_ => read_f32` was
                // four bytes at an offset nothing had measured, which the host
                // deliberately keeps in the schema precisely because it cannot
                // size it. Zero is a value; an out-of-bounds read is not.
                _ => 0.0,
            };
            (name, kind, offset, v, range)
        })
        .collect();

    let mut queue = CommandQueue::default();
    let root;
    {
        let mut commands = Commands::new(&mut queue, world);
        root = commands
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                width: Val::Percent(100.0),
                ..default()
            })
            .id();

        for (i, (name, kind, offset, seed, range)) in fields.into_iter().enumerate() {
            let control = match kind {
                // A ranged field gets a real slider. That is the whole reason
                // `set_field_range` exists: a post-process effect whose curvature
                // runs 0..1 is unusable as an unbounded drag, and every one of
                // those effects already declared its range for the old inspector.
                FieldKind::F32 if range.is_some() => {
                    let r = range.expect("checked by the guard");
                    let e = renzora_ember::widgets::slider_ranged(
                        &mut commands,
                        seed,
                        r.min,
                        r.max,
                    );
                    renzora_ember::reactive::bind_2way(
                        &mut commands,
                        e,
                        move |w: &World| read_f32(w, entity, cid, offset),
                        move |w: &mut World, v: &f32| write_f32(w, entity, cid, offset, *v),
                    );
                    e
                }
                FieldKind::F32 => {
                    let e = renzora_ember::widgets::drag_value(
                        &mut commands,
                        &fonts.ui,
                        "",
                        (150, 150, 150),
                        seed,
                        // The plugin's speed if it gave one; otherwise the old
                        // fixed step.
                        range.map_or(0.01, |r| r.speed),
                    );
                    renzora_ember::reactive::bind_2way(
                        &mut commands,
                        e,
                        move |w: &World| read_f32(w, entity, cid, offset),
                        move |w: &mut World, v: &f32| write_f32(w, entity, cid, offset, *v),
                    );
                    e
                }
                FieldKind::I32 => {
                    let e = renzora_ember::widgets::drag_value(
                        &mut commands,
                        &fonts.ui,
                        "",
                        (150, 150, 150),
                        seed,
                        1.0,
                    );
                    renzora_ember::reactive::bind_2way(
                        &mut commands,
                        e,
                        move |w: &World| read_i32(w, entity, cid, offset) as f32,
                        move |w: &mut World, v: &f32| {
                            write_i32(w, entity, cid, offset, v.round() as i32)
                        },
                    );
                    e
                }
                FieldKind::Bool => {
                    let e = renzora_ember::widgets::toggle_switch(&mut commands, seed != 0.0);
                    renzora_ember::reactive::bind_2way(
                        &mut commands,
                        e,
                        move |w: &World| read_bool(w, entity, cid, offset),
                        move |w: &mut World, v: &bool| write_bool(w, entity, cid, offset, *v),
                    );
                    e
                }
                FieldKind::Str => {
                    let e = renzora_ember::widgets::text_input(
                        &mut commands,
                        &fonts.ui,
                        "",
                        &read_str(world, entity, cid, offset),
                    );
                    renzora_ember::reactive::bind_2way(
                        &mut commands,
                        e,
                        move |w: &World| read_str(w, entity, cid, offset),
                        move |w: &mut World, v: &String| write_str(w, entity, cid, offset, v),
                    );
                    e
                }
                // Vec3/Quat need three or four sub-rows, and a kind from a newer
                // ABI is one this build cannot draw at all. Both are skipped
                // rather than drawn wrong, so the rest of the component stays
                // usable.
                _ => continue,
            };
            let row =
                renzora_ember::inspector::inspector_row(&mut commands, &fonts.ui, &name, control);
            commands
                .entity(row)
                .insert(BackgroundColor(renzora_ember::inspector::inspector_stripe(
                    i,
                )));
            commands.entity(root).add_child(row);
        }
    }
    queue.apply(world);
    root
}

// ── Registration ─────────────────────────────────────────────────────────────

/// Give every registered plugin component its own inspector section.
///
/// Runs once after plugins have loaded — their components do not exist before
/// that, so there is nothing to register at plugin-build time.
pub fn register_plugin_component_sections(world: &mut World) {
    use renzora_editor_framework::{InspectorEntry, InspectorRegistry, NativeInspectorRegistry};

    let schemas: Vec<(ComponentId, String, String)> = world
        .get_resource::<PluginComponentSchemas>()
        .map(|s| {
            s.0.iter()
                // Resources get no per-entity section, and skipping them here
                // also stops them eating slots from the fixed pool.
                .filter(|i| !i.is_resource)
                .map(|i| (i.id, i.type_path.clone(), i.display_name.clone()))
                .collect()
        })
        .unwrap_or_default();
    if schemas.is_empty() {
        return;
    }

    for (cid, type_path, label) in schemas {
        // Already has a slot — a reload re-registers the same components, and
        // burning a fresh slot each time would exhaust the pool.
        if (0..MAX_PLUGIN_COMPONENTS).any(|n| slot_component(n) == Some(cid)) {
            continue;
        }
        let Some(slot) = (0..MAX_PLUGIN_COMPONENTS).find(|n| slot_component(*n).is_none()) else {
            error!(
                "[plugin] more than {MAX_PLUGIN_COMPONENTS} plugin components — `{label}` gets \
                 no inspector section. Raise MAX_PLUGIN_COMPONENTS."
            );
            break;
        };
        SLOTS[slot].store(cid.index() as u32, Ordering::Relaxed);

        // Leaked because `InspectorEntry` wants `&'static str` and these names
        // are only known at runtime. Bounded by the component count, once per
        // process.
        let type_id: &'static str = Box::leak(type_path.into_boxed_str());
        let display_name: &'static str = Box::leak(label.into_boxed_str());

        world
            .get_resource_or_insert_with(InspectorRegistry::default)
            .register(InspectorEntry {
                type_id,
                display_name,
                icon: "puzzle-piece",
                category: "plugin",
                has_fn: HAS[slot],
                // Added via the Add Component overlay, which has the default
                // bytes — see `AddPluginComponentCmd`.
                add_fn: None,
                remove_fn: Some(REMOVE[slot]),
                is_enabled_fn: None,
                set_enabled_fn: None,
                fields: Vec::new(),
            });

        world
            .get_resource_or_insert_with(NativeInspectorRegistry::default)
            .register(type_id, DRAW[slot]);

        info!("[plugin] inspector section for `{type_id}` (slot {slot})");
    }
}
