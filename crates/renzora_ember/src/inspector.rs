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
//!         renzora_ember::reactive::tracked::bind_2way(commands, dv,
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

use crate::reactive::Rx;
use crate::font::{ui_font, EmberFonts};
use crate::reactive::tracked::{bind_2way, bind_with};
use crate::theme::*;
use crate::widgets::{bind_hsv_picker, hsv_picker, slider, Popup};
use crate::UiInteraction;

/// Maximum width of the inspector label column. The column is proportional
/// ([`INSPECTOR_LABEL_PCT`] of the row) capped at this, so on a wide panel the
/// value controls line up at a familiar fixed offset, while a narrow panel
/// gives the width back to the controls instead of squashing them.
///
/// 136 fits labels like "Suppress Shadows" and "Angular Diameter" at the
/// inspector's label size; the 112 before cut them mid-word.
pub const INSPECTOR_LABEL_W: f32 = 136.0;

/// The label column's share of the row before the [`INSPECTOR_LABEL_W`] cap
/// kicks in (cap wins above ~340px-wide rows). Every row uses the same
/// fraction, so labels and controls still line up in columns at any width.
pub const INSPECTOR_LABEL_PCT: f32 = 40.0;

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

/// Label size for a row in a tree list (the hierarchy, the asset tree). Both
/// panels list things side by side in the same editor, so they share one size
/// rather than each picking its own.
///
/// 14 lands on the 13px rung of `font::FONT_SIZE_LADDER` (after
/// `TEXT_SCALE`); 12 and 13, which the two panels used before, both snapped to
/// 11px, which is what made the trees read as cramped next to a design drawn
/// at 13.
pub const LIST_ROW_TEXT: f32 = 14.0;

/// Icon size for a tree row. 16 is a ladder rung itself (icons skip
/// `TEXT_SCALE`); the 13 and 14 used before both drew at 13px, small beside the
/// larger label.
pub const LIST_ROW_ICON: f32 = 16.0;

/// How far a tree row's highlight sits in from the panel's sides, and its
/// corner radius. The highlight is a rounded pill rather than a full-width
/// band: with the odd/even stripes gone, rows are flat, and a band running into
/// both panel edges read as a stripe that had been left behind.
pub const LIST_ROW_INSET: f32 = 4.0;
pub const LIST_ROW_RADIUS: f32 = 4.0;

/// The highlight behind a selected tree row: a soft wash of the theme's accent.
/// The old near-solid accent band was loud once nothing else in the list had a
/// background to compete with it.
pub fn list_row_selected() -> Color {
    rgb(accent()).with_alpha(0.24)
}

/// The highlight behind a hovered tree row.
pub fn list_row_hover() -> Color {
    rgb(hover_bg())
}

/// The node for a tree row's highlight: absolute, inset by [`LIST_ROW_INSET`]
/// and rounded. Callers paint its `BackgroundColor` reactively.
pub fn list_row_highlight_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(LIST_ROW_INSET),
        right: Val::Px(LIST_ROW_INSET),
        top: Val::Px(1.0),
        bottom: Val::Px(1.0),
        border_radius: BorderRadius::all(Val::Px(LIST_ROW_RADIUS)),
        ..default()
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
            InspectorLabelText,
        ))
        .id();
    // Starts at the fixed column; `fit_label_columns` narrows it to the
    // longest label among this row's siblings once their text is measured.
    let label_col = commands
        .spawn((
            Node {
                width: Val::Px(INSPECTOR_LABEL_W),
                max_width: Val::Percent(INSPECTOR_LABEL_PCT),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            InspectorLabelCol { text: lbl },
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
    fill_if_growable(commands, control);
    row
}

/// An [`inspector_row`]'s label column, and the text inside it.
#[derive(Component)]
pub struct InspectorLabelCol {
    text: Entity,
}

/// The text of an [`InspectorLabelCol`]. Marked so [`fit_label_columns`] wakes
/// for a label being laid out, not for every text in the editor (a frame
/// counter changes every frame).
#[derive(Component)]
pub struct InspectorLabelText;

/// Gap kept between the longest label in a group and the controls.
const LABEL_FIT_PAD: f32 = 10.0;

/// Size each group of [`inspector_row`] label columns to its longest label.
///
/// The column used to be a flat 40% of the row, capped at
/// [`INSPECTOR_LABEL_W`], which is what the longest labels anywhere need
/// ("Suppress Shadows"). Transform's "Position" needs about half of that, so on
/// a narrow panel its three axis boxes were squeezed beside a column that was
/// mostly empty. Now a group takes only what its own longest label needs,
/// still capped at the fixed width and at [`INSPECTOR_LABEL_PCT`] of the row.
///
/// A group is the rows sharing one parent, which is one component's body, so
/// its rows still line up with each other. Runs only when a label is laid
/// out again (new rows, a rename, a font or DPI change), and writes a column
/// only when its width actually moves.
pub(crate) fn fit_label_columns(
    changed: Query<(), (With<InspectorLabelText>, Changed<bevy::text::TextLayoutInfo>)>,
    added: Query<(), Added<InspectorLabelCol>>,
    mut cols: Query<(&InspectorLabelCol, &ChildOf, &mut Node)>,
    rows: Query<&ChildOf, Without<InspectorLabelCol>>,
    layouts: Query<&bevy::text::TextLayoutInfo>,
) {
    if changed.is_empty() && added.is_empty() {
        return;
    }
    // Widest label per group, keyed by the parent of the label's row.
    let mut widest: std::collections::HashMap<Entity, f32> = std::collections::HashMap::new();
    for (col, row, _) in &cols {
        let Ok(group) = rows.get(row.parent()) else { continue };
        let w = layouts
            .get(col.text)
            .map(|l| l.size.x / l.scale_factor.max(1e-3))
            .unwrap_or(0.0);
        let e = widest.entry(group.parent()).or_insert(0.0);
        *e = e.max(w);
    }
    for (_, row, mut node) in &mut cols {
        let Ok(group) = rows.get(row.parent()) else { continue };
        let Some(w) = widest.get(&group.parent()) else { continue };
        // Not measured yet: leave the fixed column rather than collapse it.
        if *w <= 0.0 {
            continue;
        }
        let want = Val::Px((w + LABEL_FIT_PAD).ceil().min(INSPECTOR_LABEL_W));
        if node.width != want {
            node.width = want;
        }
    }
}

/// [`fill_control`] for a control that is a number, text or dropdown box
/// itself, leaving toggles, swatches and composite rows at their own size.
///
/// The declarative field rows stretch their controls, but a native drawer that
/// handed a bare `drag_value` to [`inspector_row`] got one at its 58px minimum.
/// Side by side in one inspector, Fog's fields stopped a third of the way
/// across the row while Clouds' ran to the edge, and the column looked broken.
/// Checking here fixes every drawer at once. It runs as a queued command
/// because the widget's components are only in the world once the builder's
/// commands apply.
fn fill_if_growable(commands: &mut Commands, control: Entity) {
    commands.queue(move |w: &mut World| {
        let Ok(mut e) = w.get_entity_mut(control) else { return };
        let grows = e.contains::<crate::widgets::EmberDragValue>()
            || e.contains::<crate::widgets::DropdownState>()
            || e.contains::<crate::widgets::EmberTextInput>();
        if !grows {
            return;
        }
        if let Some(mut n) = e.get_mut::<Node>() {
            n.flex_grow = 1.0;
            n.flex_basis = Val::Px(0.0);
            n.min_width = Val::Px(0.0);
        }
    });
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

/// Corner radius of a colour swatch: half its 16-18px height, so the chip is a
/// **pill** rather than a rounded rectangle.
///
/// That shape is doing a job. A colour row draws a saturated block at the left
/// of the value column, and a ranged number draws its value bar in exactly the
/// same place — at a squared-off 3px they were the same object at a glance, and
/// a half-lit blue swatch read as a slider sitting at 40%. Nothing else in the
/// inspector is a pill, so the shape alone says "this is a colour, not a
/// quantity", at any size and in any theme.
const SWATCH_RADIUS: f32 = 8.0;

/// A compact color field: a live swatch that opens a proper HSV picker popup
/// (sat/val square + hue strip), two-way bound to an RGB `[f32; 3]` (0..1) via
/// `get`/`set`. Returns a relative wrapper (swatch + popup) to drop into a row.
/// The popup uses the generic [`Popup`] (click-outside dismiss + auto flip-up).
pub fn color_field(
    commands: &mut Commands,
    get: impl Fn(&Rx) -> [f32; 3] + Clone + Send + Sync + 'static,
    set: impl Fn(&mut World, [f32; 3]) + Send + Sync + 'static,
) -> Entity {
    color_field_anchored(commands, get, set, false)
}

/// [`color_field`] with a compact square swatch whose picker opens to the
/// *left*, for a control that sits near the right edge of its panel.
///
/// The default field grows rightwards from a 44 px swatch, which is right in an
/// inspector row and wrong in a header: the picker is ~200 px wide, so a swatch
/// 90 px from the panel's right edge would open a popup that the panel's own
/// clip rect eats most of.
pub fn color_swatch_field_right(
    commands: &mut Commands,
    get: impl Fn(&Rx) -> [f32; 3] + Clone + Send + Sync + 'static,
    set: impl Fn(&mut World, [f32; 3]) + Send + Sync + 'static,
) -> Entity {
    color_field_anchored(commands, get, set, true)
}

/// Shared body of [`color_field`] / [`color_swatch_field_right`]. `compact`
/// squares the swatch and right-aligns the popup.
fn color_field_anchored(
    commands: &mut Commands,
    get: impl Fn(&Rx) -> [f32; 3] + Clone + Send + Sync + 'static,
    set: impl Fn(&mut World, [f32; 3]) + Send + Sync + 'static,
    compact: bool,
) -> Entity {
    // Seed HSV; bind_hsv_picker re-syncs from the real value next frame.
    let picker = hsv_picker(commands, 0.0, 0.0, 0.5);
    bind_hsv_picker(commands, picker, get.clone(), set);

    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(100.0),
                left: if compact { Val::Auto } else { Val::Px(0.0) },
                right: if compact { Val::Px(0.0) } else { Val::Auto },
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
                width: Val::Px(if compact { 18.0 } else { 44.0 }),
                height: Val::Px(if compact { 18.0 } else { 16.0 }),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(SWATCH_RADIUS)),
                ..default()
            },
            BackgroundColor(rgb(placeholder())),
            BorderColor::all(rgb(border())),
            UiInteraction::default(),
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
    get: impl Fn(&Rx) -> [f32; 4] + Clone + Send + Sync + 'static,
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
                let a = g_a(&Rx::new(&*w))[3];
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
                // A setter holds `&mut World`; the getter now takes `&Rx`, so
                // wrap. The dep set is discarded — a setter is not a reaction.
                let c = g_rgb(&Rx::new(&*w));
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
                border_radius: BorderRadius::all(Val::Px(SWATCH_RADIUS)),
                ..default()
            },
            BackgroundColor(rgb(placeholder())),
            BorderColor::all(rgb(border())),
            UiInteraction::default(),
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
