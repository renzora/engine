//! The **UI Palette** panel — the way to *add* markup without typing it.
//!
//! Until this existed the only way to put a node in a template was the code
//! editor, so the canvas could rearrange a UI but never grow one.
//!
//! A panel with a searchable tile grid rather than a toolbar popup, built to the
//! same shape as the Shape Library, because the two do the same job: a catalogue
//! you browse and drag from. A popup was fine for eight entries and would be
//! useless at eighty.
//!
//! Two ways to place an element, matching the Shape Library exactly:
//!
//! - **Drag** onto the canvas — the drop lands where the insertion line shows,
//!   reusing the flow drag's own `drop_target_at`. One rule for "where does a
//!   thing go", whether it is an existing node being moved or a new one being
//!   added.
//! - **Click** — into the selection: inside a selected container, after a
//!   selected leaf, else the template root.
//!
//! Either way the write is `insert_node_in_markup` — the same byte-splice the
//! drag uses to move a node. An inserted element is real markup in the `.html`
//! from the moment it appears, not a live entity the next hot-reload discards.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use renzora::{EditorCommands, EditorSelection, SplashState};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::markup::provenance::MarkupSource;
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::tracked::bind_bg;
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{text_input, EmberTextInput};

use crate::game_ui::NativeCanvasState;

/// Tile height. Two lines of label are reserved whether or not a name uses both
/// — that is what keeps a row level regardless of which entries matched a
/// search. The *width* is not a constant: tiles fill their grid column.
const LABEL_H: f32 = 26.0;
const TILE_H: f32 = 34.0 + LABEL_H + 12.0;
/// Never fewer than four columns. Three tiles across a narrow panel reads as a
/// list that failed to be a grid.
const MIN_COLUMNS: u16 = 4;
/// Column count is picked from the panel width against this: wide enough for
/// the icon and a short label, narrow enough that four fit the default column.
const COLUMN_TARGET: f32 = 62.0;
/// Cursor travel before a press becomes a drag rather than a click.
const DRAG_THRESHOLD: f32 = 6.0;

/// One catalogue entry: how it looks in the grid, and the markup it writes.
pub(crate) struct Element {
    pub id: &'static str,
    pub icon: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub markup: &'static str,
}

/// The catalogue.
///
/// Grouped rather than flat so it can grow without becoming a wall: Layout is
/// the boxes you arrange things in, Content is what goes in them, Widgets are
/// assemblies that would be tedious to build by hand. New entries are one line
/// each — the markup is the whole definition, so adding a widget needs no code.
pub(crate) const ELEMENTS: &[Element] = &[
    // ── Layout ───────────────────────────────────────────────────────────
    Element {
        id: "column",
        icon: "rows",
        label: "Column",
        group: "Layout",
        markup: "<node flex_direction=\"column\" row_gap=\"8px\">\n</node>",
    },
    Element {
        id: "row",
        icon: "columns",
        label: "Row",
        group: "Layout",
        markup: "<node flex_direction=\"row\" align_items=\"center\" column_gap=\"8px\">\n</node>",
    },
    Element {
        id: "panel",
        icon: "square",
        label: "Panel",
        group: "Layout",
        markup: "<node flex_direction=\"column\" row_gap=\"8px\" padding=\"12px 12px 12px 12px\" border_radius=\"8px\" background=\"#141A24\" border=\"1px\" border_color=\"#232B37\">\n</node>",
    },
    // A spacer has no visible form, which is exactly why it belongs in a
    // palette: it is the piece people open the code editor to write.
    Element {
        id: "spacer",
        icon: "arrows-out-line-horizontal",
        label: "Spacer",
        group: "Layout",
        markup: "<node flex_grow=\"1\" />",
    },
    Element {
        id: "divider",
        icon: "minus",
        label: "Divider",
        group: "Layout",
        markup: "<node width=\"100%\" height=\"1px\" background=\"#232B37\" />",
    },
    // ── Content ──────────────────────────────────────────────────────────
    Element {
        id: "text",
        icon: "text-aa",
        label: "Text",
        group: "Content",
        markup: "<text font_size=\"14\" font_color=\"#D7DEEA\">Text</text>",
    },
    Element {
        id: "heading",
        icon: "text-h",
        label: "Heading",
        group: "Content",
        markup: "<text font_size=\"22\" font_color=\"#F2F5FA\">Heading</text>",
    },
    Element {
        id: "image",
        icon: "image",
        label: "Image",
        group: "Content",
        markup: "<image src=\"\" width=\"64px\" height=\"64px\" />",
    },
    Element {
        id: "icon",
        icon: "star",
        label: "Icon",
        group: "Content",
        markup: "<icon name=\"star\" font_size=\"16\" font_color=\"#7AA2FF\" />",
    },
    // ── Widgets ──────────────────────────────────────────────────────────
    Element {
        id: "button",
        icon: "cursor-click",
        label: "Button",
        group: "Widgets",
        markup: "<button on_press=\"\" padding=\"9px 14px 9px 14px\" border_radius=\"8px\" background=\"#141A24\" hover:background=\"#1B2330\">\n    <text font_size=\"13\" font_color=\"#D7DEEA\">Button</text>\n</button>",
    },
    Element {
        id: "icon_button",
        icon: "cursor-click",
        label: "Icon Button",
        group: "Widgets",
        markup: "<button on_press=\"\" width=\"34px\" height=\"34px\" align_items=\"center\" justify_content=\"center\" border_radius=\"8px\" background=\"#141A24\" hover:background=\"#1B2330\">\n    <icon name=\"gear\" font_size=\"16\" font_color=\"#9AA6B8\" />\n</button>",
    },
    Element {
        id: "labelled_row",
        icon: "list-dashes",
        label: "Label Row",
        group: "Widgets",
        markup: "<node flex_direction=\"row\" align_items=\"center\" column_gap=\"10px\">\n    <text font_size=\"13\" font_color=\"#D7DEEA\" flex_grow=\"1\">Label</text>\n    <text font_size=\"13\" font_color=\"#8A93A2\">Value</text>\n</node>",
    },
    Element {
        id: "progress",
        icon: "battery-medium",
        label: "Bar",
        group: "Widgets",
        markup: "<node width=\"160px\" height=\"8px\" border_radius=\"4px\" background=\"#161B24\" border=\"1px\" border_color=\"#232B37\">\n    <node width=\"60%\" height=\"100%\" border_radius=\"4px\" background=\"#4C8BF5\" />\n</node>",
    },
    Element {
        id: "gauge",
        icon: "gauge",
        label: "Gauge",
        group: "Widgets",
        markup: "<node vector=\"gauge\" width=\"110px\" height=\"110px\" min_width=\"110px\" flex_shrink=\"0\" value=\"60\" min=\"0\" max=\"100\" start=\"135\" sweep=\"270\" color=\"#4C8BF5\" track=\"#141C26\" thickness=\"10\" readout=\"60\" unit=\"%\" readsize=\"22\" />",
    },
    Element {
        id: "card",
        icon: "cards",
        label: "Card",
        group: "Widgets",
        markup: "<node flex_direction=\"column\" row_gap=\"6px\" padding=\"14px 14px 14px 14px\" border_radius=\"10px\" background=\"#0E121A\" border=\"1px\" border_color=\"#1D2431\">\n    <text font_size=\"14\" font_color=\"#F2F5FA\">Title</text>\n    <text font_size=\"12\" font_color=\"#8A93A2\">Supporting line of text.</text>\n</node>",
    },
    // ── Bound ────────────────────────────────────────────────────────────
    //
    // These carry *behaviour*, from the markup widget kernel in
    // `markup::widgets` — `toggle` flips a bool, `drag_value` scrubs a number,
    // `fill` sizes a node from one, `toggles` shows and hides by name. All four
    // ship in the runtime, so these work in an exported game exactly as they do
    // in the editor.
    //
    // The `Path.field` targets are placeholders: point them at a component field
    // or a script var on the canvas and they are live. Left empty they render
    // and do nothing, which is the right failure for a template you are still
    // laying out.
    Element {
        id: "switch",
        icon: "toggle-right",
        label: "Switch",
        group: "Bound",
        markup: "<node toggle=\"Player.Settings.enabled\" width=\"36px\" height=\"20px\" border_radius=\"10px\" padding=\"2px 2px 2px 2px\" align_items=\"center\" background=\"#1C2431\" hover:background=\"#243040\" duration=\"120ms\">\n    <node width=\"16px\" height=\"16px\" border_radius=\"8px\" background=\"#8A93A2\" />\n</node>",
    },
    Element {
        id: "slider",
        icon: "faders-horizontal",
        label: "Slider",
        group: "Bound",
        markup: "<node drag_value=\"Player.Settings.volume\" drag_min=\"0\" drag_max=\"100\" width=\"160px\" height=\"16px\" align_items=\"center\">\n    <node width=\"100%\" height=\"4px\" border_radius=\"2px\" background=\"#161D28\">\n        <node fill=\"Player.Settings.volume\" fill_min=\"0\" fill_max=\"100\" height=\"100%\" border_radius=\"2px\" background=\"#4C8BF5\" />\n    </node>\n</node>",
    },
    Element {
        id: "meter",
        icon: "gauge",
        label: "Meter",
        group: "Bound",
        markup: "<node width=\"160px\" height=\"8px\" border_radius=\"4px\" background=\"#161B24\" border=\"1px\" border_color=\"#232B37\">\n    <node fill=\"Player.Health.current\" fill_min=\"0\" fill_max=\"100\" height=\"100%\" border_radius=\"4px\" background=\"#E06C75\" />\n</node>",
    },
    Element {
        id: "disclosure",
        icon: "caret-down",
        label: "Disclosure",
        group: "Bound",
        markup: "<node flex_direction=\"column\" row_gap=\"6px\">\n    <button toggles=\"panel_body\" flex_direction=\"row\" align_items=\"center\" column_gap=\"8px\" padding=\"8px 10px 8px 10px\" border_radius=\"6px\" background=\"#141A24\" hover:background=\"#1B2330\">\n        <icon name=\"caret-down\" font_size=\"12\" font_color=\"#8A93A2\" />\n        <text font_size=\"13\" font_color=\"#D7DEEA\">Section</text>\n    </button>\n    <node name=\"panel_body\" flex_direction=\"column\" row_gap=\"6px\" padding=\"0 0 0 18px\">\n        <text font_size=\"12\" font_color=\"#8A93A2\">Body</text>\n    </node>\n</node>",
    },
];

#[derive(Resource, Default)]
struct PaletteState {
    search: String,
}

/// A pressed-but-unresolved tile: becomes a drag once the cursor travels, or a
/// click (insert at the selection) on release in place.
#[derive(Resource, Default)]
struct PalettePress(Option<(&'static str, Vec2)>);

/// An element being dragged from the palette. While this is set the canvas draws
/// its usual drop feedback and a release over it inserts.
#[derive(Resource, Default)]
pub(crate) struct PaletteDrag(pub Option<&'static str>);

#[derive(Component)]
struct PaletteSearch;

/// The tile grid, so [`palette_columns`] can measure it.
#[derive(Component)]
struct PaletteGrid;

#[derive(Component, Clone, Copy)]
struct PaletteTile(&'static str);

pub(crate) fn register(app: &mut App) {
    use renzora::core::RenzoraShellExt;
    app.init_resource::<PaletteState>();
    app.init_resource::<PalettePress>();
    app.init_resource::<PaletteDrag>();
    app.register_shell_panel("ui_palette", "UI Widgets", "shapes", "Scene");
    app.register_panel_content("ui_palette", true, build)
        .systems(
            Update,
            (palette_search_sync, palette_columns).run_if(in_state(SplashState::Editor)),
        )
        // NOT panel-gated. `PanelScope::systems` runs a system only while its
        // panel is the active tab, which is right for a panel's own upkeep and
        // wrong for resolving a gesture that *leaves* it: a drag from the
        // palette onto the canvas can change which tab is active, and a press
        // that stops being resolved half-way through is a drag that silently
        // does nothing.
        .always(
            Update,
            (palette_press, palette_drag_or_click)
                .chain()
                .run_if(in_state(SplashState::Editor)),
        );
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        })
        .id();

    let search = text_input(commands, &fonts.ui, "Search elements...", "");
    commands.entity(search).insert((
        PaletteSearch,
        Node {
            width: Val::Percent(100.0),
            min_width: Val::Px(0.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
    ));

    // A grid, not a wrapping row of fixed-width tiles. A wrap row leaves
    // whatever the last column could not use as dead space against the right
    // edge — at the default panel width that was three tiles and a visible gap.
    // Equal `flex` tracks divide the width exactly, so the grid always meets
    // both edges whatever the column count works out to.
    let grid = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                display: Display::Grid,
                grid_template_columns: vec![RepeatedGridTrack::flex(MIN_COLUMNS, 1.0)],
                align_content: AlignContent::FlexStart,
                column_gap: Val::Px(6.0),
                row_gap: Val::Px(6.0),
                ..default()
            },
            PaletteGrid,
        ))
        .id();
    renzora_ember::virtual_scroll::virtual_scroll_versioned(
        commands,
        grid,
        6,
        palette_token,
        palette_snapshot,
    );

    commands.entity(root).add_children(&[search, grid]);
    root
}

/// Dirty token: the catalogue is static, so only the search changes what shows.
fn palette_token(world: &Rx) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    world
        .get_resource::<PaletteState>()
        .map(|s| s.search.to_lowercase())
        .unwrap_or_default()
        .hash(&mut h);
    h.finish()
}

fn matches(e: &Element, needle: &str) -> bool {
    needle.is_empty()
        || e.label.to_lowercase().contains(needle)
        || e.group.to_lowercase().contains(needle)
}

fn palette_snapshot(world: &Rx) -> KeyedSnapshot {
    let needle = world
        .get_resource::<PaletteState>()
        .map(|s| s.search.trim().to_lowercase())
        .unwrap_or_default();
    let shown: Vec<&'static Element> = ELEMENTS.iter().filter(|e| matches(e, &needle)).collect();
    let items: Vec<(u64, u64)> = shown
        .iter()
        .map(|e| {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            e.id.hash(&mut h);
            (h.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| tile(c, f, shown[i])),
    }
}

/// The floating shelf inside the UI editor: the pieces you reach for constantly,
/// one click away, without going to another tab.
///
/// Layout and Content only. Widgets and Bound are assemblies you place once and
/// then configure, so they belong in the panel where they have names and a
/// search; a shelf of sixteen unlabelled glyphs is a memory test.
///
/// The buttons carry the same `PaletteTile` the panel's tiles do, so the press,
/// drag and insert systems drive both with no second code path.
pub(crate) fn build_shelf(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let shelf = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                // Clear of the vertical ruler, which owns the first 16px.
                left: Val::Px(26.0),
                top: Val::Px(26.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(2.0),
                padding: UiRect::all(Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(rgb(border())),
            renzora_ember::widgets::OverlaySurface,
            Name::new("ui-canvas-shelf"),
        ))
        .id();

    let mut last_group = "";
    let mut kids: Vec<Entity> = Vec::new();
    for e in ELEMENTS.iter().filter(|e| is_shelf_group(e.group)) {
        if !last_group.is_empty() && e.group != last_group {
            kids.push(
                commands
                    .spawn((
                        Node {
                            width: Val::Px(18.0),
                            height: Val::Px(1.0),
                            margin: UiRect::vertical(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(rgb(border())),
                    ))
                    .id(),
            );
        }
        last_group = e.group;
        kids.push(shelf_button(commands, fonts, e));
    }
    commands.entity(shelf).add_children(&kids);
    shelf
}

fn is_shelf_group(group: &str) -> bool {
    matches!(group, "Layout" | "Content")
}

fn shelf_button(commands: &mut Commands, fonts: &EmberFonts, e: &'static Element) -> Entity {
    let b = commands
        .spawn((
            Node {
                width: Val::Px(26.0),
                height: Val::Px(26.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            // The shelf has no room for labels, so the tooltip is the only thing
            // naming these — it is not optional here the way it is on a tile.
            renzora_ember::widgets::HoverTooltip::new(e.label),
            PaletteTile(e.id),
            Name::new(format!("ui-shelf:{}", e.id)),
        ))
        .id();
    bind_bg(commands, b, move |w| {
        if matches!(
            w.get::<Interaction>(b),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            rgb(hover_bg())
        } else {
            Color::NONE
        }
    });
    let ic = icon_text(commands, &fonts.phosphor, e.icon, text_muted(), 15.0);
    commands.entity(ic).insert(bevy::ui::FocusPolicy::Pass);
    commands.entity(b).add_child(ic);
    b
}

fn tile(commands: &mut Commands, fonts: &EmberFonts, e: &'static Element) -> Entity {
    let t = commands
        .spawn((
            Node {
                // Fills its grid column — the grid decides the width, so the
                // tiles meet both edges of the panel at any size.
                width: Val::Percent(100.0),
                min_width: Val::Px(0.0),
                height: Val::Px(TILE_H),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                padding: UiRect::vertical(Val::Px(6.0)),
                row_gap: Val::Px(4.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(format!("{} · {}", e.label, e.group)),
            PaletteTile(e.id),
            Name::new(format!("ui-element:{}", e.id)),
        ))
        .id();
    bind_bg(commands, t, move |w| {
        if matches!(
            w.get::<Interaction>(t),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            rgb(hover_bg())
        } else {
            rgb(section_bg())
        }
    });
    // The border carries the hover, not the fill: `section_bg` to `hover_bg` is
    // a couple of percent of lightness on a dark theme, which across a grid is
    // not enough to say which tile the cursor is on.
    renzora_ember::reactive::tracked::bind_with(
        commands,
        t,
        move |w| {
            matches!(
                w.get::<Interaction>(t),
                Some(Interaction::Hovered) | Some(Interaction::Pressed)
            )
        },
        |w, ent, hot: &bool| {
            let c = if *hot { rgb(accent()) } else { rgb(border()) };
            if let Some(mut b) = w.get_mut::<BorderColor>(ent) {
                *b = BorderColor::all(c);
            }
        },
    );

    let ic = icon_text(commands, &fonts.phosphor, e.icon, text_primary(), 24.0);
    commands.entity(ic).insert(Node {
        height: Val::Px(30.0),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        ..default()
    });
    let lbl = commands
        .spawn((
            bevy::ui::widget::Text::new(e.label),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(LABEL_H),
                justify_content: JustifyContent::Center,
                ..default()
            },
            bevy::text::TextLayout::justify(bevy::text::Justify::Center),
        ))
        .id();
    commands.entity(t).add_children(&[ic, lbl]);
    t
}

/// Re-fit the grid's column count to the panel's width.
///
/// Measured rather than fixed because this panel is docked in a column the user
/// resizes: a count that suits the default width is wrong the moment they drag
/// the splitter. Written only on a change, so it does not dirty `Node` — and
/// therefore re-run layout for the whole tree — every frame.
fn palette_columns(mut grid: Query<(&bevy::ui::ComputedNode, &mut Node), With<PaletteGrid>>) {
    for (cn, mut node) in &mut grid {
        let width = cn.size().x * cn.inverse_scale_factor();
        if width <= 0.0 {
            continue;
        }
        let n = ((width / COLUMN_TARGET).floor() as u16).clamp(MIN_COLUMNS, 8);
        let want = vec![RepeatedGridTrack::flex(n, 1.0)];
        if node.grid_template_columns != want {
            node.grid_template_columns = want;
        }
    }
}

fn palette_search_sync(
    input: Query<&EmberTextInput, With<PaletteSearch>>,
    mut state: ResMut<PaletteState>,
) {
    for inp in &input {
        if state.search != inp.value {
            state.search = inp.value.clone();
        }
    }
}

fn palette_press(
    q: Query<(&Interaction, &PaletteTile), Changed<Interaction>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut press: ResMut<PalettePress>,
) {
    for (interaction, tile) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let start = windows
            .single()
            .ok()
            .and_then(|w| w.cursor_position())
            .unwrap_or(Vec2::ZERO);
        press.0 = Some((tile.0, start));
    }
}

/// Resolve a pending press: a drag once the cursor travels (handed to the canvas
/// via [`PaletteDrag`]), else an insert-at-the-selection on release in place.
fn palette_drag_or_click(
    mut press: ResMut<PalettePress>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut drag: ResMut<PaletteDrag>,
    selection: Option<Res<EditorSelection>>,
    state: Option<Res<NativeCanvasState>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some((id, start)) = press.0 else { return };

    if mouse.just_released(MouseButton::Left) {
        press.0 = None;
        let (Some(cmds), Some(state)) = (cmds, state) else {
            return;
        };
        let selected = selection.as_ref().and_then(|s| s.get());
        let canvas = state.active_canvas;
        cmds.push(move |w: &mut World| {
            let Some(markup) = markup_for(id) else { return };
            let Some((parent, before)) = insertion_point(w, selected, canvas) else {
                // No canvas, or a canvas whose template has not built yet, so
                // there is nothing to insert *into*. Said out loud because the
                // alternative is a palette that looks broken.
                warn!("ui palette: no UI canvas with a built template to insert into");
                return;
            };
            renzora_ember::markup::writeback::insert_node_in_markup(w, parent, before, markup);
        });
        return;
    }

    let cursor = windows.single().ok().and_then(|w| w.cursor_position());
    if let Some(cursor) = cursor {
        if (cursor - start).length() > DRAG_THRESHOLD {
            drag.0 = Some(id);
            press.0 = None;
        }
    }
}

pub(crate) fn markup_for(id: &str) -> Option<&'static str> {
    ELEMENTS.iter().find(|e| e.id == id).map(|e| e.markup)
}

/// Resolve where a click-insert lands: `(parent, before)`.
///
/// A leaf becomes a *sibling* rather than a parent. `<text>` and `<image>` can
/// technically take children in the markup, but nothing sensible renders from
/// it, so treating them as containers would quietly produce a broken template
/// from a perfectly reasonable click.
pub(crate) fn insertion_point(
    world: &World,
    selected: Option<Entity>,
    canvas: Option<Entity>,
) -> Option<(Entity, Option<Entity>)> {
    let root = || {
        // The template's root is the canvas's only markup-bearing child.
        let canvas = canvas?;
        world
            .get::<Children>(canvas)?
            .iter()
            .find(|c| world.get::<MarkupSource>(*c).is_some())
    };
    let Some(sel) = selected.filter(|e| world.get::<MarkupSource>(*e).is_some()) else {
        return root().map(|r| (r, None));
    };
    let is_leaf = world.get::<bevy::ui::widget::Text>(sel).is_some()
        || world.get::<bevy::ui::widget::ImageNode>(sel).is_some();
    if is_leaf {
        let parent = world.get::<ChildOf>(sel).map(|c| c.parent())?;
        let next = world.get::<Children>(parent).and_then(|kids| {
            let i = kids.iter().position(|c| c == sel)?;
            kids.get(i + 1).copied()
        });
        return Some((parent, next));
    }
    Some((sel, None))
}
