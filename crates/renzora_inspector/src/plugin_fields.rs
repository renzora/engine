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
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_plugin::host::PluginComponentSchemas;
use renzora_plugin::sys::FieldKind;

/// How many plugin components can have their own inspector section.
///
/// Raising it is mechanical — extend the `slots!` list below. It is a ceiling on
/// *distinct component types across all loaded plugins*, not on instances.
pub const MAX_PLUGIN_COMPONENTS: usize = 32;

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
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31,
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
    let fields: Vec<(String, FieldKind, usize, f32)> = world
        .get_resource::<PluginComponentSchemas>()
        .and_then(|s| s.0.iter().find(|i| i.id == cid))
        .map(|i| {
            i.fields
                .iter()
                .map(|f| (f.name.clone(), f.kind, f.offset))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
        .into_iter()
        .map(|(name, kind, offset)| {
            let v = match kind {
                FieldKind::I32 | FieldKind::Bool => read_i32(world, entity, cid, offset) as f32,
                _ => read_f32(world, entity, cid, offset),
            };
            (name, kind, offset, v)
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

        for (i, (name, kind, offset, seed)) in fields.into_iter().enumerate() {
            let control = match kind {
                FieldKind::F32 => {
                    let e = renzora_ember::widgets::drag_value(
                        &mut commands,
                        &fonts.ui,
                        "",
                        (150, 150, 150),
                        seed,
                        0.01,
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
                        move |w: &World| read_i32(w, entity, cid, offset) != 0,
                        move |w: &mut World, v: &bool| write_i32(w, entity, cid, offset, *v as i32),
                    );
                    e
                }
                // Vec3/Quat need three or four sub-rows. Skipped rather than
                // drawn wrong, so a component carrying one is still usable.
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
