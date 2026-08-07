//! A panel for the global state plugins own.
//!
//! A plugin resource has the same field schema a plugin component does — name,
//! [`FieldKind`], byte offset — so the rows draw the same way. What it does not
//! have is an entity, and that is the whole reason this is a panel rather than
//! an inspector section: the inspector draws what is *selected*, and a resource
//! is not selectable. Registering one as a component section would have put a
//! copy of a global onto whatever happened to be highlighted.
//!
//! ## Why the content is built by a system rather than by the panel builder
//!
//! [`register_panel_content`] hands the builder a `Commands` and the fonts, and
//! nothing else. That is fine for a panel whose shape is known at compile time —
//! bind each row reactively and the values look after themselves. It is not
//! enough here, because the *set of rows* is world data: which resources exist,
//! and which fields each one has, is only known after plugins have loaded.
//!
//! So the builder spawns an empty marked container and a gated system fills it.
//! The system re-fills when the registered set changes, which also covers a
//! plugin loading after the panel was first opened.

use bevy::ecs::component::ComponentId;
use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::Rx;
use renzora_ember::panel::RegisterPanelContent;
use renzora_plugin::host::PluginComponentSchemas;
use renzora_plugin::sys::FieldKind;

pub const PANEL_ID: &str = "plugin_resources";

/// The container a [`fill`] pass writes into.
#[derive(Component)]
struct ResourcePanelRoot {
    /// How many resources were drawn last time. A resource cannot lose fields
    /// without the plugin being rebuilt and reloaded, so the count is enough to
    /// notice a change without hashing the whole schema every frame.
    drawn: usize,
}

// ── Raw field access ─────────────────────────────────────────────────────────
//
// The component equivalents in `plugin_fields` go through an entity; these go
// through the resource cache instead. Reads are unaligned for the same reason:
// the offset came from `offset_of!` in the plugin, which is correct for that
// type's layout, but it is applied here to a `*const u8` that carries no
// alignment guarantee of its own.
//
// `pub(crate)` so `plugin_panels` binds a panel widget to a resource field
// through the same accessors the inspector rows use. Two implementations of
// "poke this offset" would be two chances to get the `bool` case wrong.

/// A dependency-tracked reflection read of a plugin resource field.
///
/// Same shape as the component-side helper in `plugin_fields`, and here for the
/// same reason: these fields are addressed by `(ComponentId, offset)`, so the
/// typed `Rx::resource::<R>` cannot name them. Declaring the dep and doing the
/// read in one place keeps the pair from drifting.
pub(crate) fn tracked_read<T>(
    rx: &Rx,
    cid: ComponentId,
    offset: usize,
    read: impl Fn(&World, ComponentId, usize) -> T,
) -> T {
    rx.track_resource_id(cid);
    read(rx.manually_tracked(), cid, offset)
}

pub(crate) fn read_f32(world: &World, cid: ComponentId, offset: usize) -> f32 {
    world
        .get_resource_by_id(cid)
        .map(|p| unsafe { p.as_ptr().add(offset).cast::<f32>().read_unaligned() })
        .unwrap_or(0.0)
}

pub(crate) fn write_f32(world: &mut World, cid: ComponentId, offset: usize, v: f32) {
    if let Some(mut ptr) = world.get_resource_mut_by_id(cid) {
        // `as_mut()` marks the resource changed, which is what makes a plugin
        // system taking `ResMut` see the edit.
        unsafe {
            ptr.as_mut()
                .as_ptr()
                .add(offset)
                .cast::<f32>()
                .write_unaligned(v)
        };
    }
}

/// A `bool` is ONE byte, and reading it as an `i32` reads three bytes that are
/// not part of the field. `struct Flags { a: bool, b: bool }` is two bytes with
/// align 1, so a 4-byte write at offset 0 scribbles over `b` and two bytes of
/// whatever the allocator put next — which surfaces as an unrelated component
/// changing when you toggle a checkbox.
pub(crate) fn read_bool(world: &World, cid: ComponentId, offset: usize) -> bool {
    world
        .get_resource_by_id(cid)
        .map(|p| unsafe { p.as_ptr().add(offset).read() != 0 })
        .unwrap_or(false)
}

pub(crate) fn write_bool(world: &mut World, cid: ComponentId, offset: usize, v: bool) {
    if let Some(mut ptr) = world.get_resource_mut_by_id(cid) {
        unsafe { ptr.as_mut().as_ptr().add(offset).write(v as u8) };
    }
}

pub(crate) fn read_i32(world: &World, cid: ComponentId, offset: usize) -> i32 {
    world
        .get_resource_by_id(cid)
        .map(|p| unsafe { p.as_ptr().add(offset).cast::<i32>().read_unaligned() })
        .unwrap_or(0)
}

pub(crate) fn write_i32(world: &mut World, cid: ComponentId, offset: usize, v: i32) {
    if let Some(mut ptr) = world.get_resource_mut_by_id(cid) {
        unsafe {
            ptr.as_mut()
                .as_ptr()
                .add(offset)
                .cast::<i32>()
                .write_unaligned(v)
        };
    }
}

// ── Drawing ──────────────────────────────────────────────────────────────────

/// One resource: a header, then a row per editable field.
fn draw_resource(
    commands: &mut Commands,
    fonts: &EmberFonts,
    title: &str,
    cid: ComponentId,
    fields: &[(String, FieldKind, usize, f32)],
) -> Entity {
    let section = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            margin: UiRect::bottom(Val::Px(8.0)),
            ..default()
        })
        .id();

    let header = commands
        .spawn((
            Text::new(title.to_string()),
            ui_font(&fonts.ui, 12.0),
            Node {
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                ..default()
            },
        ))
        .id();
    commands.entity(section).add_child(header);

    for (i, (name, kind, offset, seed)) in fields.iter().enumerate() {
        let (offset, seed) = (*offset, *seed);
        let control = match *kind {
            FieldKind::F32 => {
                let e = renzora_ember::widgets::drag_value(
                    commands,
                    &fonts.ui,
                    "",
                    (150, 150, 150),
                    seed,
                    0.01,
                );
                renzora_ember::reactive::tracked::bind_2way(
                    commands,
                    e,
                    move |rx: &Rx| tracked_read(rx, cid, offset, read_f32),
                    move |w: &mut World, v: &f32| write_f32(w, cid, offset, *v),
                );
                e
            }
            FieldKind::I32 => {
                let e = renzora_ember::widgets::drag_value(
                    commands,
                    &fonts.ui,
                    "",
                    (150, 150, 150),
                    seed,
                    1.0,
                );
                renzora_ember::reactive::tracked::bind_2way(
                    commands,
                    e,
                    move |rx: &Rx| tracked_read(rx, cid, offset, read_i32) as f32,
                    move |w: &mut World, v: &f32| write_i32(w, cid, offset, v.round() as i32),
                );
                e
            }
            FieldKind::Bool => {
                let e = renzora_ember::widgets::toggle_switch(commands, seed != 0.0);
                renzora_ember::reactive::tracked::bind_2way(
                    commands,
                    e,
                    move |rx: &Rx| tracked_read(rx, cid, offset, read_bool),
                    move |w: &mut World, v: &bool| write_bool(w, cid, offset, *v),
                );
                e
            }
            // Vec3/Quat want three or four sub-rows, and a kind from a newer
            // ABI is one this build cannot draw at all. Both are skipped rather
            // than drawn wrong, so the rest of the resource stays usable.
            _ => continue,
        };
        let row = renzora_ember::inspector::inspector_row(commands, &fonts.ui, name, control);
        commands
            .entity(row)
            .insert(BackgroundColor(renzora_ember::inspector::inspector_stripe(
                i,
            )));
        commands.entity(section).add_child(row);
    }

    section
}

/// Populate the panel when the set of registered resources changes.
///
/// Gated on the panel being visible, so a hidden tab costs nothing — the same
/// rule every other panel follows, and the reason the editor's idle frame stays
/// cheap.
fn fill(world: &mut World) {
    let Some((root, drawn)) = world
        .query::<(Entity, &ResourcePanelRoot)>()
        .iter(world)
        .map(|(e, r)| (e, r.drawn))
        .next()
    else {
        return;
    };

    // Snapshot the schema first, then read the values in a second pass — the
    // schema borrow of the world has to end before the value reads begin.
    let schema: Vec<(String, ComponentId, Vec<(String, FieldKind, usize)>)> = world
        .get_resource::<PluginComponentSchemas>()
        .map(|s| {
            s.0.iter()
                .filter(|i| i.is_resource)
                .map(|i| {
                    (
                        i.display_name.clone(),
                        i.id,
                        i.fields
                            .iter()
                            .map(|f| (f.name.clone(), f.kind, f.offset))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();

    if schema.len() == drawn {
        return;
    }

    let resources: Vec<(String, ComponentId, Vec<(String, FieldKind, usize, f32)>)> = schema
        .into_iter()
        .map(|(name, cid, fields)| {
            let fields = fields
                .into_iter()
                .map(|(n, kind, offset)| {
                    let v = match kind {
                        FieldKind::Bool => read_bool(world, cid, offset) as i32 as f32,
                        FieldKind::I32 => read_i32(world, cid, offset) as f32,
                        FieldKind::F32 => read_f32(world, cid, offset),
                        // Same as the component path: a kind this build cannot
                        // size must not be read at a guessed width.
                        _ => 0.0,
                    };
                    (n, kind, offset, v)
                })
                .collect::<Vec<_>>();
            (name, cid, fields)
        })
        .collect();

    let fonts = world.resource::<EmberFonts>().clone();
    let count = resources.len();

    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        // Rebuild wholesale rather than diffing: this runs when the registered
        // set changes, which happens at most once per plugin load.
        commands.entity(root).despawn_related::<Children>();

        if resources.is_empty() {
            let empty = commands
                .spawn((
                    Text::new("No plugin resources registered."),
                    ui_font(&fonts.ui, 12.0),
                    Node {
                        padding: UiRect::all(Val::Px(8.0)),
                        ..default()
                    },
                ))
                .id();
            commands.entity(root).add_child(empty);
        }

        for (name, cid, fields) in resources {
            let section = draw_resource(&mut commands, &fonts, &name, cid, &fields);
            commands.entity(root).add_child(section);
        }
        commands.entity(root).insert(ResourcePanelRoot { drawn: count });
    }
    queue.apply(world);
}

pub fn register(app: &mut App) {
    app.register_panel_content(PANEL_ID, true, |commands, _fonts| {
        commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    width: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(4.0)),
                    ..default()
                },
                // `usize::MAX` rather than 0 so the first `fill` always runs:
                // zero registered resources is a legitimate state that must
                // still replace the empty container with the empty message.
                ResourcePanelRoot { drawn: usize::MAX },
                Name::new("plugin_resources_root"),
            ))
            .id()
    })
    // `systems` already gates on the panel being visible — no explicit
    // `run_if` needed, and adding one would just duplicate the condition.
    .systems(Update, fill);
}
