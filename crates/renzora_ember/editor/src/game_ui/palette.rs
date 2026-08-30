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

/// Tile metrics, matching the Shape Library's so the two panels read as one
/// family. Two lines of label are reserved whether or not a name uses both —
/// that is what keeps a row level regardless of which entries matched a search.
const TILE_W: f32 = 76.0;
const LABEL_H: f32 = 26.0;
const TILE_H: f32 = 34.0 + LABEL_H + 12.0;
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

#[derive(Component, Clone, Copy)]
struct PaletteTile(&'static str);

pub(crate) fn register(app: &mut App) {
    use renzora::core::RenzoraShellExt;
    app.init_resource::<PaletteState>();
    app.init_resource::<PalettePress>();
    app.init_resource::<PaletteDrag>();
    app.register_shell_panel("ui_palette", "UI Palette", "shapes", "Scene");
    app.register_panel_content("ui_palette", true, build)
        .systems(
            Update,
            (palette_search_sync, palette_press, palette_drag_or_click)
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

    let grid = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_content: AlignContent::FlexStart,
            column_gap: Val::Px(6.0),
            row_gap: Val::Px(6.0),
            ..default()
        })
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

fn tile(commands: &mut Commands, fonts: &EmberFonts, e: &'static Element) -> Entity {
    let t = commands
        .spawn((
            Node {
                width: Val::Px(TILE_W),
                height: Val::Px(TILE_H),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                padding: UiRect::vertical(Val::Px(6.0)),
                row_gap: Val::Px(4.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
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
