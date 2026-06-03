//! Helper for writing native inspector drawers
//! (`renzora_editor::NativeInspectorDrawer`).
//!
//! A drawer is `fn(&mut World, Entity) -> Entity` — it builds an arbitrary
//! bevy_ui subtree (using ember widgets + `bind_2way`) and returns its root,
//! which the inspector parents under the component's section. [`inspector_body`]
//! wraps the `CommandQueue` + fonts boilerplate so a drawer reads:
//!
//! ```ignore
//! fn my_inspector(world: &mut World, entity: Entity) -> Entity {
//!     renzora_ember::inspector::inspector_body(world, |commands, fonts| {
//!         let col = commands.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).id();
//!         let dv = renzora_ember::widgets::drag_value(commands, &fonts.ui, "", (210,210,220), 0.0, 0.1);
//!         renzora_ember::reactive::bind_2way(commands, dv,
//!             move |w| w.get::<MyComp>(entity).map(|c| c.value).unwrap_or(0.0),
//!             move |w, v: &f32| { if let Some(mut c) = w.get_mut::<MyComp>(entity) { c.value = *v; } });
//!         commands.entity(col).add_child(dv);
//!         col
//!     })
//! }
//! ```

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use crate::font::{ui_font, EmberFonts};
use crate::theme::{rgb, TEXT_MUTED};

/// Width of the inspector label column. Labels are left-aligned and fixed-width
/// so the value controls line up in a column directly to their right.
pub const INSPECTOR_LABEL_W: f32 = 112.0;

/// Alternating row background for an inspector/list row at `row_index`
/// (odd rows tinted, even rows transparent). Insert as the row's
/// `BackgroundColor` so panels stripe consistently.
pub fn inspector_stripe(row_index: usize) -> Color {
    // Even rows are fully transparent — they show the real panel background
    // unchanged. Odd rows get a faint darkening overlay, so the alternation is
    // subtle and the panel base is untouched.
    if row_index % 2 == 1 {
        Color::srgba(0.0, 0.0, 0.0, 0.22)
    } else {
        Color::NONE
    }
}

/// A consistent inspector property row: a **left-aligned** fixed-width label
/// column + the `control` immediately to its right (also left-aligned). Both the
/// declarative field renderer and native drawers use this so every row matches.
pub fn inspector_row(
    commands: &mut Commands,
    font: &Handle<Font>,
    label: &str,
    control: Entity,
) -> Entity {
    let lbl = commands
        .spawn((
            Text::new(label),
            ui_font(font, 11.0),
            TextColor(rgb(TEXT_MUTED)),
            bevy::text::TextLayout::new_with_no_wrap(),
        ))
        .id();
    let label_col = commands
        .spawn((
            Node {
                width: Val::Px(INSPECTOR_LABEL_W),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            Name::new("inspector-label"),
        ))
        .id();
    commands.entity(label_col).add_child(lbl);
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                ..default()
            },
            Name::new("inspector-row"),
        ))
        .id();
    commands.entity(row).add_children(&[label_col, control]);
    row
}

/// Run a drawer body builder with a fresh `Commands` (backed by a local
/// `CommandQueue` that's applied before returning) + the live [`EmberFonts`].
/// Returns the root entity your `build` produced. Returns a bare node if fonts
/// aren't ready yet (shouldn't happen — the inspector gates on fonts).
pub fn inspector_body(
    world: &mut World,
    build: impl FnOnce(&mut Commands, &EmberFonts) -> Entity,
) -> Entity {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return world.spawn(Node::default()).id();
    };
    let mut queue = CommandQueue::default();
    let root = {
        let mut commands = Commands::new(&mut queue, world);
        build(&mut commands, &fonts)
    };
    queue.apply(world);
    root
}
