//! The element palette — the way to *add* markup without typing it.
//!
//! Until this existed the only way to put a node in a template was the code
//! editor, which meant the canvas could rearrange a UI but never grow one.
//!
//! Each entry is a snippet of markup text, and adding one is
//! `insert_node_in_markup` — the same byte-splice the drag uses to move a node,
//! writing the file and letting the loader rebuild. So an inserted element is
//! real markup in the `.html` from the moment it appears, not a live entity that
//! the next hot-reload would quietly discard.
//!
//! **Where it lands** follows the selection, because that is the thing the user
//! last pointed at:
//!
//! - a selected container → inside it, last;
//! - a selected leaf (a `<text>`, an `<icon>`) → after it, as a sibling, since
//!   nesting a node inside a text node is never what was meant;
//! - nothing selected → the template root.

use bevy::prelude::*;

use renzora::{EditorCommands, EditorSelection, SplashState};
use renzora_ember::font::EmberFonts;
use renzora_ember::markup::provenance::MarkupSource;
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    icon_popup_trigger, popup_anchor, popup_panel, settings_section, settings_separator,
};

use crate::game_ui::NativeCanvasState;

/// One palette entry: what it looks like in the list, and the markup it writes.
struct Element {
    icon: &'static str,
    label: &'static str,
    markup: &'static str,
}

/// Deliberately short. A palette of forty widgets is a catalogue to read;
/// these are the pieces every layout is actually built from, and anything else
/// is one of these with attributes changed in the inspector.
const LAYOUT: &[Element] = &[
    Element {
        icon: "rows",
        label: "Column",
        markup: "<node flex_direction=\"column\" row_gap=\"8px\">\n</node>",
    },
    Element {
        icon: "columns",
        label: "Row",
        markup: "<node flex_direction=\"row\" align_items=\"center\" column_gap=\"8px\">\n</node>",
    },
    Element {
        icon: "square",
        label: "Panel",
        markup: "<node flex_direction=\"column\" row_gap=\"8px\" padding=\"12px 12px 12px 12px\" border_radius=\"8px\" background=\"#141A24\" border=\"1px\" border_color=\"#232B37\">\n</node>",
    },
    // A spacer is the one piece with no visible form, so it is the one people
    // reach for the code editor to write. It belongs here most of all.
    Element {
        icon: "arrows-out-line-horizontal",
        label: "Spacer",
        markup: "<node flex_grow=\"1\" />",
    },
];

const CONTENT: &[Element] = &[
    Element {
        icon: "text-aa",
        label: "Text",
        markup: "<text font_size=\"14\" font_color=\"#D7DEEA\">Text</text>",
    },
    Element {
        icon: "cursor-click",
        label: "Button",
        markup: "<button on_press=\"\" padding=\"9px 14px 9px 14px\" border_radius=\"8px\" background=\"#141A24\" hover:background=\"#1B2330\">\n    <text font_size=\"13\" font_color=\"#D7DEEA\">Button</text>\n</button>",
    },
    Element {
        icon: "image",
        label: "Image",
        markup: "<image src=\"\" width=\"64px\" height=\"64px\" />",
    },
    Element {
        icon: "star",
        label: "Icon",
        markup: "<icon name=\"star\" font_size=\"16\" font_color=\"#7AA2FF\" />",
    },
];

/// Marker carrying the markup a palette row writes.
#[derive(Component, Clone)]
struct PaletteEntry(&'static str);

pub(crate) fn register(app: &mut App) {
    app.add_systems(Update, palette_click.run_if(in_state(SplashState::Editor)));
}

/// Build the palette trigger + popup for the canvas toolbar.
pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let mut kids = vec![settings_section(commands, fonts, "Layout")];
    for e in LAYOUT {
        kids.push(row(commands, fonts, e));
    }
    kids.push(settings_separator(commands));
    kids.push(settings_section(commands, fonts, "Content"));
    for e in CONTENT {
        kids.push(row(commands, fonts, e));
    }
    let panel = popup_panel(commands, &kids);
    let trigger = icon_popup_trigger(commands, fonts, "plus", panel);
    popup_anchor(commands, trigger, panel)
}

fn row(commands: &mut Commands, fonts: &EmberFonts, e: &Element) -> Entity {
    let icon = renzora_ember::font::icon_text(commands, &fonts.phosphor, e.icon, text_muted(), 13.0);
    commands.entity(icon).insert(bevy::ui::FocusPolicy::Pass);
    let label = commands
        .spawn((
            bevy::ui::widget::Text::new(e.label),
            renzora_ember::font::ui_font(&fonts.ui, 12.0),
            TextColor(rgb(value_text())),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::axes(Val::Px(4.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            PaletteEntry(e.markup),
            Name::new("ui-palette-row"),
        ))
        .id();
    commands.entity(row).add_children(&[icon, label]);
    row
}

fn palette_click(
    q: Query<(&Interaction, &PaletteEntry), Changed<Interaction>>,
    selection: Option<Res<EditorSelection>>,
    state: Option<Res<NativeCanvasState>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let (Some(cmds), Some(state)) = (cmds, state) else {
        return;
    };
    for (interaction, entry) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let markup = entry.0;
        let selected = selection.as_ref().and_then(|s| s.get());
        let canvas = state.active_canvas;
        cmds.push(move |w: &mut World| {
            let Some((parent, before)) = insertion_point(w, selected, canvas) else {
                return;
            };
            renzora_ember::markup::writeback::insert_node_in_markup(w, parent, before, markup);
        });
    }
}

/// Resolve where a palette insert lands: `(parent, before)`.
///
/// A leaf becomes a *sibling* rather than a parent. `<text>` and `<icon>` can
/// technically take children in the markup, but nothing sensible renders from
/// it, so treating them as containers would quietly produce a broken template
/// from a perfectly reasonable click.
fn insertion_point(
    world: &World,
    selected: Option<Entity>,
    canvas: Option<Entity>,
) -> Option<(Entity, Option<Entity>)> {
    let root = || {
        // The template's root node is the canvas's only `Node`-bearing child.
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
        // Directly after the selection, which is where "add one of these next to
        // that" means.
        let next = world.get::<Children>(parent).and_then(|kids| {
            let i = kids.iter().position(|c| c == sel)?;
            kids.get(i + 1).copied()
        });
        return Some((parent, next));
    }
    Some((sel, None))
}
