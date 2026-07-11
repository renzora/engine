//! Helper for writing native inspector drawers
//! (`renzora_editor_framework::NativeInspectorDrawer`).
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
use bevy::ui::RelativeCursorPosition;

use crate::font::{ui_font, EmberFonts};
use crate::reactive::{bind_2way, bind_with};
use crate::theme::*;
use crate::widgets::{bind_hsv_picker, hsv_picker, slider, Popup};

/// Maximum width of the inspector label column. The column is proportional
/// ([`INSPECTOR_LABEL_PCT`] of the row) capped at this, so on a wide panel the
/// value controls line up at a familiar fixed offset, while a narrow panel
/// gives the width back to the controls instead of squashing them.
pub const INSPECTOR_LABEL_W: f32 = 112.0;

/// The label column's share of the row before the [`INSPECTOR_LABEL_W`] cap
/// kicks in (cap wins above ~320px-wide rows). Every row uses the same
/// fraction, so labels and controls still line up in columns at any width.
pub const INSPECTOR_LABEL_PCT: f32 = 35.0;

/// Alternating row background for an inspector/list row at `row_index`. Insert
/// as the row's `BackgroundColor` so panels stripe consistently.
pub fn inspector_stripe(row_index: usize) -> Color {
    // Solid, theme-derived stripe colors (a transparent/black-alpha overlay read
    // as muddy on light themes — these track the palette on both light and dark).
    if row_index % 2 == 1 {
        rgb(row_odd())
    } else {
        rgb(row_even())
    }
}

/// A consistent inspector property row: a **left-aligned** fixed-width label
/// column + the `control` immediately to its right (also left-aligned). Both the
/// declarative field renderer and native drawers use this so every row matches.
pub fn inspector_row(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    label: &str,
    control: Entity,
) -> Entity {
    let lbl = commands
        .spawn((
            Text::new(label),
            ui_font(font, 11.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
        ))
        .id();
    let label_col = commands
        .spawn((
            Node {
                width: Val::Percent(INSPECTOR_LABEL_PCT),
                max_width: Val::Px(INSPECTOR_LABEL_W),
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

/// Stretch an already-built control so it absorbs the row's free width. Widgets
/// like `drag_value` / `text_input` / `dropdown` size to a fixed `min_width` by
/// default (right for toolbars), which leaves a dead gap between the control and
/// the row's trailing keyframe/reset buttons when the inspector panel is wide.
/// `flex_basis: 0` (not just `flex_grow`) is what makes several siblings — e.g.
/// a Vec3's X/Y/Z boxes — split the space *evenly* regardless of content width,
/// and the zeroed `min_width` lets them shrink back down on narrow panels.
pub fn fill_control(commands: &mut Commands, control: Entity) {
    commands.entity(control).entry::<Node>().and_modify(|mut n| {
        n.flex_grow = 1.0;
        n.flex_basis = Val::Px(0.0);
        n.min_width = Val::Px(0.0);
    });
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

/// A compact color field: a live swatch that opens a proper HSV picker popup
/// (sat/val square + hue strip), two-way bound to an RGB `[f32; 3]` (0..1) via
/// `get`/`set`. Returns a relative wrapper (swatch + popup) to drop into a row.
/// The popup uses the generic [`Popup`] (click-outside dismiss + auto flip-up).
pub fn color_field(
    commands: &mut Commands,
    get: impl Fn(&World) -> [f32; 3] + Clone + Send + Sync + 'static,
    set: impl Fn(&mut World, [f32; 3]) + Send + Sync + 'static,
) -> Entity {
    // Seed HSV; bind_hsv_picker re-syncs from the real value next frame.
    let picker = hsv_picker(commands, 0.0, 0.0, 0.5);
    bind_hsv_picker(commands, picker, get.clone(), set);

    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(100.0),
                left: Val::Px(0.0),
                margin: UiRect::top(Val::Px(2.0)),
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            GlobalZIndex(700),
            RelativeCursorPosition::default(),
            Name::new("color-panel"),
        ))
        .id();
    commands.entity(panel).add_child(picker);

    let swatch = commands
        .spawn((
            Node {
                width: Val::Px(44.0),
                height: Val::Px(16.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(placeholder())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            Popup::new(panel),
            Name::new("color-swatch"),
        ))
        .id();
    bind_with(commands, swatch, get, |w, e, col: &[f32; 3]| {
        if let Some(mut bg) = w.get_mut::<BackgroundColor>(e) {
            bg.0 = Color::srgb(col[0], col[1], col[2]);
        }
    });

    let wrap = commands
        .spawn((
            Node {
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                ..default()
            },
            Name::new("color-wrap"),
        ))
        .id();
    commands.entity(wrap).add_children(&[swatch, panel]);
    wrap
}

/// RGBA color field — like [`color_field`] plus an alpha track: the HSV picker
/// drives RGB (preserving alpha) and a slider drives alpha (preserving RGB), so
/// all four channels are editable. Get/set use straight (unmultiplied) `[r,g,b,a]`.
pub fn color_field_rgba(
    commands: &mut Commands,
    get: impl Fn(&World) -> [f32; 4] + Clone + Send + Sync + 'static,
    set: impl Fn(&mut World, [f32; 4]) + Clone + Send + Sync + 'static,
) -> Entity {
    let picker = hsv_picker(commands, 0.0, 0.0, 0.5);
    // HSV picker drives RGB, preserving the current alpha.
    {
        let (g_rgb, g_a, s) = (get.clone(), get.clone(), set.clone());
        bind_hsv_picker(
            commands,
            picker,
            move |w| {
                let c = g_rgb(w);
                [c[0], c[1], c[2]]
            },
            move |w, rgb: [f32; 3]| {
                let a = g_a(w)[3];
                s(w, [rgb[0], rgb[1], rgb[2], a]);
            },
        );
    }

    // Alpha slider, preserving RGB.
    let alpha = slider(commands, 1.0);
    commands.entity(alpha).insert(Node {
        width: Val::Percent(100.0),
        height: Val::Px(16.0),
        position_type: PositionType::Relative,
        align_items: AlignItems::Center,
        margin: UiRect::top(Val::Px(6.0)),
        ..default()
    });
    {
        let (g_a, g_rgb, s) = (get.clone(), get.clone(), set.clone());
        bind_2way(
            commands,
            alpha,
            move |w| g_a(w)[3],
            move |w, a: &f32| {
                let c = g_rgb(w);
                s(w, [c[0], c[1], c[2], *a]);
            },
        );
    }

    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(100.0),
                left: Val::Px(0.0),
                margin: UiRect::top(Val::Px(2.0)),
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                flex_direction: FlexDirection::Column,
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            GlobalZIndex(700),
            RelativeCursorPosition::default(),
            Name::new("color-panel-rgba"),
        ))
        .id();
    commands.entity(panel).add_children(&[picker, alpha]);

    let swatch = commands
        .spawn((
            Node {
                width: Val::Px(44.0),
                height: Val::Px(16.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(placeholder())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            Popup::new(panel),
            Name::new("color-swatch-rgba"),
        ))
        .id();
    {
        let g = get.clone();
        bind_with(commands, swatch, g, |w, e, col: &[f32; 4]| {
            if let Some(mut bg) = w.get_mut::<BackgroundColor>(e) {
                bg.0 = Color::srgba(col[0], col[1], col[2], col[3]);
            }
        });
    }

    let wrap = commands
        .spawn((
            Node {
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                ..default()
            },
            Name::new("color-wrap-rgba"),
        ))
        .id();
    commands.entity(wrap).add_children(&[swatch, panel]);
    wrap
}
