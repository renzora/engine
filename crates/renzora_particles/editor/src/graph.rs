//! The Effect Graph panel: one emitter's node graph on ember's node graph
//! view, with a tab per emitter above it.
//!
//! Built exactly the way the material editor builds its graph
//! (`renzora_material_editor/src/graph.rs`), because ember's widget is the one
//! the user asked to see: `node_graph_view` for the canvas, `graph_node_view`
//! / `graph_wire_view` / `graph_comment_view` mounted through `keyed_list`
//! keyed on graph *structure*, so dragging a node, selecting it or scrubbing a
//! value never rebuilds it, and the widget's recorded `GraphEdit`s drained back
//! into the document by [`graph_sync`]. The add-node palette is ember's
//! `search_menu` with the catalog's categories, pin hints and descriptions;
//! comments come from the Comment button, the C key and the palette's first
//! entry.
//!
//! What differs is the model. Nodes come from `renzora_particles`' catalog;
//! every emitter's graph ends in a Particle Output, which the sync refuses to
//! delete; and a compile error is pinned to its node (red header icon,
//! tooltip) and summarised in a strip under the toolbar.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora::core::keybindings::KeyBinding;
use renzora_editor_framework::{AppEditorExt, ShortcutEntry, SplashState};
use renzora_ember::dock::{Dock, DockWindows, FixedDock};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text, bind_with, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    bind_text_input, button, comment_search_entry, graph_comment_view, graph_node_view, graph_wire_view, menu_item,
    menu_item_styled, menu_sep, node_graph_view, overlay_sized, screen_menu, search_menu, text_input, GraphEdit,
    HoverTooltip, NodeGraphView, SearchEntry, TooltipAnchorAbove,
};
use renzora_ember::UiInteraction;
use renzora_particles::asset::{NodeId, PinValue};
use renzora_particles::ir::ValueType;
use renzora_particles::nodes::{node_def, NodeDef, PinKind, PropKind, NODES, OUTPUT_NODE};

use crate::curve_editor::{table_preview, PropTarget};
use crate::model;
use crate::pin_editors::{bool_prop_editor, editor_kind, pin_editor, prop_row, text_prop_editor, EditorKind};
use crate::state::{save_document, VfxEditorState};

pub const PANEL: &str = "vfx_graph";

pub struct VfxGraphPanel;

impl Plugin for VfxGraphPanel {
    fn build(&self, app: &mut App) {
        app.register_panel_content(PANEL, false, build);
        app.init_resource::<RenameDraft>();
        // Ctrl+S through the shortcut registry, as the material graph does:
        // rebindable, listed in the command palette, and sharing the chord
        // with the scene save on purpose (one keystroke saves everything).
        app.register_shortcut(ShortcutEntry::new(
            "vfx_graph.save",
            "Save Particle Effect",
            "File",
            KeyBinding::new(KeyCode::KeyS).ctrl(),
            |w| {
                let visible = ["vfx_graph", "vfx_preview", "vfx_emitter"].iter().any(|id| {
                    renzora_ember::dock::panel_visible_anywhere(
                        id,
                        w.get_resource::<Dock>(),
                        w.get_resource::<FixedDock>(),
                        w.get_resource::<DockWindows>(),
                    )
                });
                if visible && w.resource::<VfxEditorState>().dirty {
                    save_document(w);
                }
            },
        ));
        app.add_systems(
            Update,
            (
                toolbar_click,
                view_op_click,
                add_node_open,
                context_menu_open,
                tab_click,
                tab_context_menu,
                rename_confirm,
                fit_on_switch,
            )
                .run_if(in_state(SplashState::Editor))
                .run_if(renzora_ember::dock::panel_active(PANEL)),
        );
        app.add_systems(
            Update,
            graph_sync
                .run_if(in_state(SplashState::Editor))
                .run_if(any_with_component::<VfxGraphView>)
                .run_if(renzora_ember::dock::panel_active(PANEL)),
        );
    }
}

// ── Catalog presentation ─────────────────────────────────────────────────────

/// Header colour per catalog category, in the material editor's register:
/// saturated enough to tell kinds apart at a glance, dark enough to carry the
/// white title.
pub fn category_color(category: &str) -> (u8, u8, u8) {
    match category {
        "Output" => (196, 72, 72),
        "Shape" => (62, 132, 204),
        "Shape 2D" => (52, 160, 180),
        "Input" => (142, 92, 206),
        "Random" => (214, 120, 52),
        "Math" => (58, 150, 108),
        "Curve" => (196, 152, 40),
        "Force" => (186, 76, 150),
        "Collision" => (150, 110, 70),
        "Color" => (204, 96, 52),
        _ => (110, 120, 160),
    }
}

fn category_icon(category: &str) -> &'static str {
    match category {
        "Output" => "sign-out",
        "Shape" => "cube",
        "Shape 2D" => "square",
        "Input" => "sign-in",
        "Random" => "dice-five",
        "Math" => "calculator",
        "Curve" => "chart-line",
        "Force" => "wind",
        "Collision" => "wall",
        "Color" => "thermometer",
        _ => "circle",
    }
}

/// Categories in catalog order, the order the add-node palette lists them.
pub fn categories() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for def in NODES {
        if !out.contains(&def.category) {
            out.push(def.category);
        }
    }
    out
}

/// Pin colour per value type, the material editor's palette. A generic pin
/// takes whatever is wired into it, so it gets a colour of its own.
fn pin_color(kind: PinKind) -> (u8, u8, u8) {
    match kind {
        PinKind::Fixed(ValueType::Float) => (0, 212, 170),
        PinKind::Fixed(ValueType::Vec2) => (127, 204, 25),
        PinKind::Fixed(ValueType::Vec3) => (255, 215, 0),
        PinKind::Fixed(ValueType::Color) => (255, 140, 60),
        PinKind::Generic => (170, 150, 235),
    }
}

/// The widget's connect group. All particle values interconnect (a Float
/// splats into a vector, generic pins take anything); a real mismatch is
/// caught by the compiler and shown on the node, which says why, where a
/// refused drag says nothing.
const VALUE_GROUP: u32 = 0;

/// "Min, Max +1 \u{203a} Value": a node's pins in a few characters, as the
/// material palette shows them.
fn pin_hint(def: &NodeDef) -> String {
    let side = |names: Vec<&str>| {
        let mut s = names.iter().take(2).copied().collect::<Vec<_>>().join(", ");
        if names.len() > 2 {
            s.push_str(&format!(" +{}", names.len() - 2));
        }
        s
    };
    let ins = side(def.inputs.iter().map(|p| p.label).collect());
    let outs = side(def.outputs.iter().map(|p| p.label).collect());
    match (ins.is_empty(), outs.is_empty()) {
        (true, true) => String::new(),
        (true, false) => outs,
        (false, true) => ins,
        (false, false) => format!("{ins} \u{203a} {outs}"),
    }
}

// ── Build ────────────────────────────────────────────────────────────────────

/// On the graph viewport.
#[derive(Component)]
struct VfxGraphView;

#[derive(Component, Clone, Copy)]
enum ToolAction {
    AddNode,
    Save,
    NewEffect,
    AddEmitter,
}

#[derive(Clone, Copy)]
enum ViewOp {
    Fit,
    Center,
    ZoomIn,
    ZoomOut,
    Comment,
}

/// A view button, bound to its own viewport (the panel can be open twice).
#[derive(Component)]
struct ViewOpBtn {
    op: ViewOp,
    view: Entity,
}

/// An emitter tab.
#[derive(Component)]
struct EmitterTab(usize);

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() },
            BackgroundColor(rgb(panel_bg())),
            Name::new("vfx-graph-panel"),
        ))
        .id();

    let empty = crate::templates::empty_state(commands, fonts, "Open a .vfx from the Assets panel to edit its graph, or start from a template.");
    bind_display(commands, empty, |w| !w.get_resource::<VfxEditorState>().is_some_and(VfxEditorState::is_open));

    let editor = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(0.0), flex_direction: FlexDirection::Column, ..default() })
        .id();
    bind_display(commands, editor, |w| w.get_resource::<VfxEditorState>().is_some_and(VfxEditorState::is_open));

    let handle = node_graph_view(commands, fonts);
    let (canvas, viewport) = (handle.canvas, handle.viewport);
    commands.entity(viewport).insert(VfxGraphView);

    let header = emitter_tabs(commands, fonts);
    let tools = build_toolbar(commands, fonts, viewport);
    commands.entity(header).add_children(&tools);
    let errors = error_strip(commands, fonts);

    let layer = |commands: &mut Commands| {
        commands
            .spawn(Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() })
            .id()
    };
    // Comments behind the nodes, wires in viewport space, nodes on the canvas:
    // the material graph's three layers.
    let comments_layer = layer(commands);
    commands.entity(canvas).add_child(comments_layer);
    keyed_list(commands, comments_layer, move |w| comment_snapshot(w, canvas, viewport));
    let wires_layer = layer(commands);
    commands.entity(viewport).add_child(wires_layer);
    keyed_list(commands, wires_layer, move |w| wire_snapshot(w, viewport));
    let nodes_layer = layer(commands);
    commands.entity(canvas).add_child(nodes_layer);
    keyed_list(commands, nodes_layer, move |w| node_snapshot(w, canvas, viewport));

    commands.entity(editor).add_children(&[header, errors, viewport]);
    commands.entity(root).add_children(&[empty, editor]);
    renzora_editor_framework::mark_drop_zone(commands, root);
    root
}

/// The graph's one header strip: an emitter chip each and "+ Emitter" on the
/// left, then (appended by the caller) the graph actions on the right.
///
/// It is drawn in the graph's own background colour with no rule under it, so
/// the chips read as sitting on the canvas the way the approved mockup has
/// them. It was a tab row in the header colour over a second toolbar row: two
/// bars of chrome before the graph started. It stays a row above the canvas
/// rather than floating over it, because a press on an overlay would also
/// reach the graph underneath and start a marquee or a pan.
fn emitter_tabs(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(44.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(10.0)),
                flex_shrink: 0.0,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            Name::new("vfx-emitter-tabs"),
        ))
        .id();
    let list = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), flex_shrink: 0.0, ..default() }).id();
    keyed_list(commands, list, |w| {
        let Some(st) = w.get_resource::<VfxEditorState>() else { return empty() };
        let names: Vec<String> = st.doc.emitters.iter().map(|e| e.name.clone()).collect();
        let items = names
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let mut h = hasher();
                n.hash(&mut h);
                (i as u64, h.finish())
            })
            .collect();
        KeyedSnapshot { items, build: Box::new(move |c, f, i| emitter_tab(c, f, i, &names[i])) }
    });
    let add = link_button(commands, fonts, "+ Emitter", "Add an emitter to this effect", accent());
    commands.entity(add).insert(ToolAction::AddEmitter);
    commands.entity(bar).add_children(&[list, add]);
    bar
}

/// An emitter's colour swatch on its chip. Emitters carry no colour of their
/// own, so it is picked by position from a small palette of distinct, warm to
/// cool hues: enough to tell Flames from Embers from Smoke at a glance.
pub(crate) fn emitter_swatch(index: usize) -> (u8, u8, u8) {
    const PALETTE: [(u8, u8, u8); 6] = [(255, 138, 61), (255, 210, 74), (138, 148, 171), (127, 209, 168), (143, 176, 255), (196, 155, 242)];
    PALETTE[index % PALETTE.len()]
}

/// A plain text action, the mockup's "+ Emitter" / "+ Add Node" / "Comment":
/// no fill at rest, the hover wash under it.
fn link_button(commands: &mut Commands, fonts: &EmberFonts, label: &str, tip: &str, color: (u8, u8, u8)) -> Entity {
    let btn = commands
        .spawn((
            Node {
                height: Val::Px(26.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            HoverTooltip::new(tip),
            Name::new("vfx-graph-link"),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        if matches!(w.get::<UiInteraction>(btn), Some(UiInteraction::Hovered | UiInteraction::Pressed)) {
            rgb(hover_bg())
        } else {
            Color::NONE
        }
    });
    let t = commands
        .spawn((Text::new(label), ui_font(&fonts.ui, 12.0), TextColor(rgb(color)), renzora_ember::interaction::PASS_THROUGH))
        .id();
    commands.entity(btn).add_child(t);
    btn
}

fn emitter_tab(commands: &mut Commands, fonts: &EmberFonts, index: usize, name: &str) -> Entity {
    let tab = commands
        .spawn((
            Node {
                height: Val::Px(26.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(rgb(border())),
            UiInteraction::default(),
            RelativeCursorPosition::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            HoverTooltip::new("Click to edit this emitter; right-click to rename, duplicate or delete"),
            EmitterTab(index),
            Name::new("vfx-emitter-tab"),
        ))
        .id();
    bind_with(
        commands,
        tab,
        move |w| {
            let active = w.get_resource::<VfxEditorState>().and_then(VfxEditorState::emitter_index) == Some(index);
            let hot = matches!(w.get::<UiInteraction>(tab), Some(UiInteraction::Hovered | UiInteraction::Pressed));
            (active, hot)
        },
        |w, e, &(active, hot): &(bool, bool)| {
            if let Some(mut bg) = w.get_mut::<BackgroundColor>(e) {
                bg.0 = if active {
                    renzora_ember::theme::mix(card_bg(), accent(), 0.22)
                } else if hot {
                    rgb(hover_bg())
                } else {
                    rgb(card_bg())
                };
            }
            if let Some(mut b) = w.get_mut::<BorderColor>(e) {
                *b = BorderColor::all(rgb(if active { accent() } else { border() }));
            }
        },
    );
    let icon = commands
        .spawn((
            Node { width: Val::Px(8.0), height: Val::Px(8.0), border_radius: BorderRadius::all(Val::Px(2.0)), ..default() },
            BackgroundColor(rgb(emitter_swatch(index))),
            renzora_ember::interaction::PASS_THROUGH,
        ))
        .id();
    let label = commands
        .spawn((Text::new(name), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())), renzora_ember::interaction::PASS_THROUGH))
        .id();
    // Disabled emitters read dimmer; broken ones carry the error mark.
    bind_with(
        commands,
        label,
        move |w| w.get_resource::<VfxEditorState>().and_then(|s| s.doc.emitters.get(index)).is_some_and(|e| e.enabled),
        |w, e, on: &bool| {
            if let Some(mut c) = w.get_mut::<TextColor>(e) {
                c.0 = rgb(if *on { text_primary() } else { text_muted() });
            }
        },
    );
    let warn = icon_text(commands, &fonts.phosphor, "warning", close_red(), 11.0);
    commands.entity(warn).insert(renzora_ember::interaction::PASS_THROUGH);
    bind_display(commands, warn, move |w| {
        w.get_resource::<VfxEditorState>().and_then(|s| s.errors.get(index)).is_some_and(|e| !e.is_empty())
    });
    commands.entity(tab).add_children(&[icon, label, warn]);
    tab
}

/// The right half of the header strip: the graph actions, then the file and
/// Save, then the view controls. Returned as items for the strip to hold.
fn build_toolbar(commands: &mut Commands, fonts: &EmberFonts, view: Entity) -> Vec<Entity> {
    // The file and its unsaved mark sit beside Save, so what Save writes is on
    // screen next to it.
    let file = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), flex_shrink: 0.0, ..default() }).id();
    let glyph = icon_text(commands, &fonts.phosphor, "magic-wand", (255, 140, 70), 13.0);
    let name = commands.spawn((Text::new(""), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())))).id();
    bind_text(commands, name, |w| {
        w.get_resource::<VfxEditorState>()
            .and_then(|s| s.path.as_deref())
            .and_then(|p| std::path::Path::new(p).file_name().and_then(|n| n.to_str()).map(str::to_string))
            .unwrap_or_default()
    });
    let dirty = commands
        .spawn((
            Node { width: Val::Px(7.0), height: Val::Px(7.0), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(rgb(accent())),
            HoverTooltip::new("Unsaved changes (Ctrl+S saves)"),
        ))
        .id();
    bind_display(commands, dirty, |w| w.get_resource::<VfxEditorState>().is_some_and(|s| s.dirty));
    commands.entity(file).add_children(&[glyph, name, dirty]);

    let add = link_button(commands, fonts, "+ Add Node", "Add a node (or right-click the graph)", text_primary());
    commands.entity(add).insert((ToolAction::AddNode, RelativeCursorPosition::default()));
    let comment = link_button(commands, fonts, "Comment", "Group the selected nodes in a comment box, or add an empty one (C)", text_primary());
    commands.entity(comment).insert(ViewOpBtn { op: ViewOp::Comment, view });
    let new = link_button(commands, fonts, "Templates", "Make a new effect from a template", text_primary());
    commands.entity(new).insert(ToolAction::NewEffect);
    let save = tool_button(commands, fonts, "floppy-disk", Some("Save"), "Write the effect to its .vfx file (Ctrl+S)", true);
    commands.entity(save).insert(ToolAction::Save);
    let rule = commands
        .spawn((Node { width: Val::Px(1.0), height: Val::Px(18.0), flex_shrink: 0.0, margin: UiRect::horizontal(Val::Px(4.0)), ..default() }, BackgroundColor(rgb(border()))))
        .id();

    let sep = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let group = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(2.0), flex_shrink: 0.0, ..default() }).id();
    let mut views = Vec::new();
    for (icon, tip, op) in [
        ("arrows-in", "Fit the graph in view", ViewOp::Fit),
        ("crosshair-simple", "Center the view", ViewOp::Center),
        ("magnifying-glass-plus", "Zoom in", ViewOp::ZoomIn),
        ("magnifying-glass-minus", "Zoom out", ViewOp::ZoomOut),
    ] {
        let b = tool_button(commands, fonts, icon, None, tip, false);
        commands.entity(b).insert(ViewOpBtn { op, view });
        views.push(b);
    }
    commands.entity(group).add_children(&views);
    vec![sep, add, comment, rule, file, new, save, group]
}

/// A toolbar button in the material graph's flat style: no fill at rest, a
/// hover wash, and only the primary action (Save) filled in the accent.
fn tool_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: Option<&str>, tip: &str, primary: bool) -> Entity {
    let btn = commands
        .spawn((
            Node {
                height: Val::Px(28.0),
                min_width: Val::Px(28.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(if label.is_some() { 12.0 } else { 0.0 })),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            HoverTooltip::new(tip),
            Name::new("vfx-graph-tool"),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        let hot = matches!(w.get::<UiInteraction>(btn), Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed));
        match (primary, hot) {
            (true, false) => rgb(accent()),
            (true, true) => renzora_ember::theme::mix(accent(), text_primary(), 0.15),
            (false, true) => rgb(hover_bg()),
            (false, false) => Color::NONE,
        }
    });
    let icon_color = if primary { on_accent() } else if label.is_some() { accent() } else { text_muted() };
    let glyph = icon_text(commands, &fonts.phosphor, icon, icon_color, 14.0);
    commands.entity(glyph).insert(renzora_ember::interaction::PASS_THROUGH);
    let mut kids = vec![glyph];
    if let Some(label) = label {
        let t = commands
            .spawn((
                Text::new(label),
                ui_font(&fonts.ui, 12.0),
                TextColor(rgb(if primary { on_accent() } else { text_primary() })),
                renzora_ember::interaction::PASS_THROUGH,
            ))
            .id();
        kids.push(t);
    }
    commands.entity(btn).add_children(&kids);
    btn
}

/// The compile error strip: hidden while the effect compiles.
fn error_strip(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let strip = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                border: UiRect::vertical(Val::Px(1.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            BorderColor::all(rgb(border())),
            Name::new("vfx-graph-errors"),
        ))
        .id();
    let icon = icon_text(commands, &fonts.phosphor, "warning", close_red(), 13.0);
    let text = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())))).id();
    bind_text(commands, text, |w| w.get_resource::<VfxEditorState>().and_then(VfxEditorState::error_summary).unwrap_or_default());
    bind_display(commands, strip, |w| w.get_resource::<VfxEditorState>().is_some_and(|s| s.error_summary().is_some()));
    commands.entity(strip).add_children(&[icon, text]);
    strip
}

// ── Snapshots ────────────────────────────────────────────────────────────────

type Port = (String, String, (u8, u8, u8), u32);

struct NodeSnap {
    id: NodeId,
    title: String,
    color: (u8, u8, u8),
    pos: [f32; 2],
    inputs: Vec<Port>,
    outputs: Vec<Port>,
    selected: bool,
    /// Per input, index-aligned: the editor it gets and its current value.
    editors: Vec<Option<(EditorKind, Option<PinValue>)>>,
    def: Option<&'static NodeDef>,
    is_output: bool,
    error: Option<String>,
}

fn node_snapshot(w: &Rx, canvas: Entity, viewport: Entity) -> KeyedSnapshot {
    let Some(st) = w.get_resource::<VfxEditorState>() else { return empty() };
    let Some(ei) = st.emitter_index() else { return empty() };
    let graph = &st.doc.emitters[ei].graph;
    let nodes: Vec<NodeSnap> = graph
        .nodes
        .iter()
        .map(|n| {
            let def = node_def(&n.node_type);
            let ports = |pins: Vec<(&str, &str, PinKind)>| -> Vec<Port> {
                pins.into_iter().map(|(name, label, kind)| (name.to_string(), label.to_string(), pin_color(kind), VALUE_GROUP)).collect()
            };
            let (inputs, outputs, editors) = match def {
                Some(d) => (
                    ports(d.inputs.iter().map(|p| (p.name, p.label, p.kind)).collect()),
                    ports(d.outputs.iter().map(|p| (p.name, p.label, p.kind)).collect()),
                    d.inputs
                        .iter()
                        .map(|p| {
                            let current = n.input_values.get(p.name);
                            editor_kind(current, p, model::is_connected(graph, n.id, p.name)).map(|k| (k, current.cloned()))
                        })
                        .collect(),
                ),
                None => (Vec::new(), Vec::new(), Vec::new()),
            };
            NodeSnap {
                id: n.id,
                title: def.map(|d| d.display_name.to_string()).unwrap_or_else(|| n.node_type.clone()),
                color: def.map(|d| category_color(d.category)).unwrap_or((170, 45, 45)),
                pos: n.position,
                inputs,
                outputs,
                selected: st.selected_node == Some(n.id),
                editors,
                def,
                is_output: n.node_type == OUTPUT_NODE,
                error: st.node_error(n.id),
            }
        })
        .collect();
    let items = nodes
        .iter()
        .map(|n| {
            let mut k = hasher();
            (ei, n.id).hash(&mut k);
            let mut h = hasher();
            // Structure only, never position, selection or values, so a drag,
            // a click and a scrub update in place. The editor kinds are in it
            // so wiring a pin removes its field (and unwiring brings it back),
            // and the error so the warning appears and clears.
            let kinds: Vec<Option<EditorKind>> = n.editors.iter().map(|e| e.as_ref().map(|(k, _)| *k)).collect();
            (&n.title, n.color, &n.inputs, &n.outputs, &kinds, &n.error).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let n = &nodes[i];
            let editors: Vec<Option<Entity>> = match n.def {
                Some(d) => n
                    .editors
                    .iter()
                    .zip(d.inputs)
                    .map(|(e, pin)| e.as_ref().map(|(kind, init)| pin_editor(c, f, ei, n.id, pin, *kind, init.as_ref())))
                    .collect(),
                None => Vec::new(),
            };
            let header = header_controls(c, f, n.error.is_some(), n.is_output);
            let node = graph_node_view(c, f, canvas, viewport, n.id, &n.title, n.color, &n.inputs, &n.outputs, n.pos[0], n.pos[1], n.selected, None, &editors, header);
            if let Some(d) = n.def {
                for prop in d.props {
                    let control = match prop.kind {
                        PropKind::Bool(default) => bool_prop_editor(c, ei, n.id, prop.name, default),
                        PropKind::Curve => table_preview(c, PropTarget { emitter: ei, node: n.id, prop: prop.name, gradient: false }),
                        PropKind::Gradient => table_preview(c, PropTarget { emitter: ei, node: n.id, prop: prop.name, gradient: true }),
                        PropKind::Text(default) => text_prop_editor(c, f, ei, n.id, prop.name, default),
                    };
                    let row = prop_row(c, f, prop.label, control);
                    c.entity(node).add_child(row);
                }
            }
            let tip = match (&n.error, n.is_output) {
                (Some(e), _) => Some(e.clone()),
                (None, true) => Some("Every emitter ends here. Spawn pins run once per step, init pins once per particle at birth, update pins every step.".to_string()),
                _ => n.def.map(|d| d.description.to_string()),
            };
            if let Some(tip) = tip {
                c.entity(node).insert((HoverTooltip::new(tip), TooltipAnchorAbove));
            }
            node
        }),
    }
}

/// The title bar's trailing icons: a lock on the Particle Output (it cannot be
/// deleted, and the icon says so before anyone tries) and a red warning when
/// the compiler has something to say about the node.
fn header_controls(commands: &mut Commands, fonts: &EmberFonts, has_error: bool, is_output: bool) -> Option<Entity> {
    if !has_error && !is_output {
        return None;
    }
    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), flex_shrink: 0.0, ..default() }).id();
    if is_output {
        let lock = icon_text(commands, &fonts.phosphor, "lock-simple", on_accent(), 11.0);
        commands.entity(row).add_child(lock);
    }
    if has_error {
        let icon = icon_text(commands, &fonts.phosphor, "warning", close_red(), 13.0);
        commands.entity(row).add_child(icon);
    }
    Some(row)
}

fn wire_snapshot(w: &Rx, viewport: Entity) -> KeyedSnapshot {
    let Some(st) = w.get_resource::<VfxEditorState>() else { return empty() };
    let Some(ei) = st.emitter_index() else { return empty() };
    let wires: Vec<(NodeId, String, NodeId, String)> = st.doc.emitters[ei]
        .graph
        .connections
        .iter()
        .map(|c| (c.from_node, c.from_pin.clone(), c.to_node, c.to_pin.clone()))
        .collect();
    let items = wires
        .iter()
        .map(|wire| {
            let mut k = hasher();
            (ei, wire).hash(&mut k);
            let h = k.finish();
            (h, h)
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, _f, i| {
            let (a, ap, b, bp) = &wires[i];
            graph_wire_view(c, viewport, *a, ap, *b, bp)
        }),
    }
}

fn comment_snapshot(w: &Rx, canvas: Entity, viewport: Entity) -> KeyedSnapshot {
    let Some(st) = w.get_resource::<VfxEditorState>() else { return empty() };
    let Some(ei) = st.emitter_index() else { return empty() };
    let comments: Vec<(u64, String, [f32; 4], (u8, u8, u8))> = st.doc.emitters[ei]
        .graph
        .comments
        .iter()
        .map(|c| (c.id, c.text.clone(), c.rect, (c.color[0], c.color[1], c.color[2])))
        .collect();
    let items = comments
        .iter()
        .map(|(id, ..)| {
            let mut k = hasher();
            (ei, id).hash(&mut k);
            let h = k.finish();
            (h, h)
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (id, text, rect, color) = &comments[i];
            graph_comment_view(c, f, canvas, viewport, *id, text, *rect, *color)
        }),
    }
}

fn empty() -> KeyedSnapshot {
    KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) }
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

// ── Systems ──────────────────────────────────────────────────────────────────

/// Drain the view's recorded edits into the active emitter's graph.
fn graph_sync(world: &mut World) {
    let mut edits: Vec<GraphEdit> = Vec::new();
    let mut q = world.query_filtered::<&mut NodeGraphView, With<VfxGraphView>>();
    for mut view in q.iter_mut(world) {
        if !view.pending.is_empty() {
            edits.append(&mut view.pending);
        }
    }
    if edits.is_empty() {
        return;
    }
    let mut notes: Vec<String> = Vec::new();
    {
        let mut st = world.resource_mut::<VfxEditorState>();
        let mut selected = st.selected_node;
        let Some(graph) = st.graph_mut() else { return };
        for edit in edits {
            match edit {
                GraphEdit::NodeMoved { id, x, y } => {
                    if let Some(n) = model::node_mut(graph, id) {
                        if n.position != [x, y] {
                            n.position = [x, y];
                        }
                    }
                }
                GraphEdit::Connect { from_node, from_pin, to_node, to_pin } => {
                    if let Err(e) = model::connect(graph, from_node, &from_pin, to_node, &to_pin) {
                        notes.push(format!("Connection refused ({from_pin} to {to_pin}): {e:?}"));
                    }
                }
                GraphEdit::Disconnect { to_node, to_pin, .. } => {
                    model::disconnect(graph, to_node, &to_pin);
                }
                GraphEdit::Delete { id } => {
                    if model::is_output(graph, id) {
                        notes.push("The Particle Output cannot be deleted: every emitter ends in one.".to_string());
                    } else if model::remove_node(graph, id) && selected == Some(id) {
                        selected = None;
                    }
                }
                GraphEdit::Select { id } => selected = id,
                GraphEdit::AddComment { rect } => {
                    model::add_comment(graph, rect);
                }
                GraphEdit::CommentMoved { id, x, y } => {
                    if let Some(c) = model::comment_mut(graph, id) {
                        c.rect[0] = x;
                        c.rect[1] = y;
                    }
                }
                GraphEdit::CommentResized { id, w, h } => {
                    if let Some(c) = model::comment_mut(graph, id) {
                        c.rect[2] = w;
                        c.rect[3] = h;
                    }
                }
                GraphEdit::CommentRetitled { id, text } => {
                    if let Some(c) = model::comment_mut(graph, id) {
                        c.text = text;
                    }
                }
                GraphEdit::DeleteComment { id } => model::remove_comment(graph, id),
            }
        }
        if st.selected_node != selected {
            st.selected_node = selected;
        }
    }
    for note in notes {
        renzora::console_log::console_info("Particles", note);
    }
}

/// Frame the graph when another effect or emitter comes up: its node
/// positions have nothing to do with where the last one left the view.
fn fit_on_switch(state: Res<VfxEditorState>, mut views: Query<&mut NodeGraphView, With<VfxGraphView>>, mut last: Local<Option<(Option<String>, usize)>>) {
    let key = (state.path.clone(), state.active_emitter);
    if last.as_ref() == Some(&key) {
        return;
    }
    *last = Some(key);
    for mut v in &mut views {
        v.fit_request = true;
    }
}

fn view_op_click(q: Query<(&UiInteraction, &ViewOpBtn), Changed<UiInteraction>>, mut views: Query<&mut NodeGraphView, With<VfxGraphView>>) {
    for (i, btn) in &q {
        if *i != UiInteraction::Pressed {
            continue;
        }
        let Ok(mut v) = views.get_mut(btn.view) else { continue };
        match btn.op {
            ViewOp::Fit => v.fit_request = true,
            ViewOp::Center => v.center_request = true,
            ViewOp::ZoomIn => v.zoom_request = Some(1.25),
            ViewOp::ZoomOut => v.zoom_request = Some(0.8),
            ViewOp::Comment => v.request_comment(None),
        }
    }
}

fn toolbar_click(q: Query<(&UiInteraction, &ToolAction), Changed<UiInteraction>>, mut commands: Commands) {
    for (i, action) in &q {
        if *i != UiInteraction::Pressed {
            continue;
        }
        match action {
            ToolAction::Save => commands.queue(save_document),
            ToolAction::NewEffect => commands.queue(crate::templates::request_gallery),
            ToolAction::AddEmitter => commands.queue(|w: &mut World| {
                let mut st = w.resource_mut::<VfxEditorState>();
                if st.is_open() {
                    let i = model::add_emitter(&mut st.doc);
                    st.active_emitter = i;
                    st.selected_node = None;
                }
            }),
            // Opened by `add_node_open`, which needs the button's rect.
            ToolAction::AddNode => {}
        }
    }
}

fn add_node_open(
    q: Query<(&UiInteraction, &ToolAction, &RelativeCursorPosition, &ComputedNode), Changed<UiInteraction>>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, _, rcp, cn)) = q.iter().find(|(i, a, ..)| **i == UiInteraction::Pressed && matches!(a, ToolAction::AddNode)) else { return };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else { return };
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    search_menu(&mut commands, &fonts, top_left.x, top_left.y + size.y + 2.0, node_entries([60.0, 60.0]));
}

/// Right-click on empty canvas opens the palette at the cursor; a cable
/// dropped on empty canvas opens it to wire the new node to that pin.
fn context_menu_open(fonts: Option<Res<EmberFonts>>, mut commands: Commands, mut views: Query<(Entity, &mut NodeGraphView), With<VfxGraphView>>) {
    let Some(fonts) = fonts else { return };
    for (view, mut v) in &mut views {
        if let Some((screen, canvas)) = v.context_menu.take() {
            let mut entries = vec![comment_search_entry(view, canvas)];
            entries.extend(node_entries([canvas.x, canvas.y]));
            search_menu(&mut commands, &fonts, screen.x, screen.y, entries);
        }
        if let Some(cd) = v.connect_drag.take() {
            let src = (cd.node, cd.pin, cd.is_output);
            search_menu(&mut commands, &fonts, cd.screen.x, cd.screen.y, connect_entries([cd.canvas.x, cd.canvas.y], src));
        }
    }
}

/// Every catalog node but the Output (one per emitter, already there).
fn node_entries(at: [f32; 2]) -> Vec<SearchEntry> {
    let mut entries = Vec::new();
    for category in categories() {
        if category == "Output" {
            continue;
        }
        let icon = category_icon(category);
        for def in NODES.iter().filter(|d| d.category == category) {
            let node_type = def.node_type;
            entries.push(
                SearchEntry::new(icon, def.display_name, category, move |w| add_node(w, node_type, at, None))
                    .with_detail(pin_hint(def))
                    .with_description(def.description),
            );
        }
    }
    entries
}

fn connect_entries(at: [f32; 2], src: (NodeId, String, bool)) -> Vec<SearchEntry> {
    let mut entries = Vec::new();
    for category in categories() {
        if category == "Output" {
            continue;
        }
        let icon = category_icon(category);
        for def in NODES.iter().filter(|d| d.category == category) {
            // Only nodes that have a pin on the side the cable needs.
            let fits = if src.2 { !def.inputs.is_empty() } else { !def.outputs.is_empty() };
            if !fits {
                continue;
            }
            let node_type = def.node_type;
            let src = src.clone();
            entries.push(
                SearchEntry::new(icon, def.display_name, category, move |w| add_node(w, node_type, at, Some(src.clone())))
                    .with_detail(pin_hint(def))
                    .with_description(def.description),
            );
        }
    }
    entries
}

/// Add a node to the active emitter, select it, and (from a dropped cable)
/// wire it to the pin the cable came from, picking the new node's best match:
/// the same type, else a generic pin, else the first.
fn add_node(w: &mut World, node_type: &str, at: [f32; 2], src: Option<(NodeId, String, bool)>) {
    let mut st = w.resource_mut::<VfxEditorState>();
    let Some(graph) = st.graph_mut() else { return };
    let Some(id) = model::add_node(graph, node_type, at) else { return };
    if let Some((src_node, src_pin, src_is_output)) = src {
        let src_kind = model::node(graph, src_node).and_then(|n| node_def(&n.node_type)).and_then(|d| {
            if src_is_output { d.output(&src_pin).map(|p| p.kind) } else { d.input(&src_pin).map(|p| p.kind) }
        });
        if let Some(def) = node_def(node_type) {
            let rank = |k: PinKind| match (src_kind, k) {
                (Some(a), b) if a == b => 0u8,
                (_, PinKind::Generic) | (Some(PinKind::Generic), _) => 1,
                _ => 2,
            };
            let pick = if src_is_output {
                def.inputs.iter().filter(|p| !matches!(p.default, renzora_particles::nodes::PinDefault::Attr(_))).min_by_key(|p| rank(p.kind)).or(def.inputs.first()).map(|p| p.name)
            } else {
                def.outputs.iter().min_by_key(|p| rank(p.kind)).map(|p| p.name)
            };
            if let Some(pin) = pick {
                let _ = if src_is_output {
                    model::connect(graph, src_node, &src_pin, id, pin)
                } else {
                    model::connect(graph, id, pin, src_node, &src_pin)
                };
            }
        }
    }
    st.selected_node = Some(id);
}

fn tab_click(q: Query<(&UiInteraction, &EmitterTab), Changed<UiInteraction>>, mut state: ResMut<VfxEditorState>) {
    for (i, tab) in &q {
        if *i == UiInteraction::Pressed && state.active_emitter != tab.0 {
            state.active_emitter = tab.0;
            state.selected_node = None;
        }
    }
}

/// Right-click a tab: Rename, Duplicate, Delete.
fn tab_context_menu(
    mouse: Res<ButtonInput<MouseButton>>,
    tabs: Query<(&RelativeCursorPosition, &EmitterTab)>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    state: Res<VfxEditorState>,
    mut commands: Commands,
) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    let (Some(fonts), Some(cursor)) = (fonts, windows.iter().find_map(|w| w.cursor_position())) else { return };
    let Some((_, tab)) = tabs.iter().find(|(r, _)| r.cursor_over) else { return };
    let index = tab.0;
    let can_delete = state.doc.emitters.len() > 1;
    let menu = screen_menu(&mut commands, cursor.x, cursor.y);
    let rename = menu_item(&mut commands, &fonts, "pencil-simple", "Rename", move |w| open_rename(w, index));
    let dup = menu_item(&mut commands, &fonts, "copy", "Duplicate", move |w| {
        let mut st = w.resource_mut::<VfxEditorState>();
        if let Some(i) = model::duplicate_emitter(&mut st.doc, index) {
            st.active_emitter = i;
            st.selected_node = None;
        }
    });
    let mut kids = vec![rename, dup];
    if can_delete {
        let sep = menu_sep(&mut commands);
        let del = menu_item_styled(&mut commands, &fonts, "trash", "Delete", close_red(), close_red(), move |w| {
            let mut st = w.resource_mut::<VfxEditorState>();
            if model::remove_emitter(&mut st.doc, index) {
                if st.active_emitter >= index && st.active_emitter > 0 {
                    st.active_emitter -= 1;
                }
                st.selected_node = None;
            }
        });
        kids.extend([sep, del]);
    }
    commands.entity(menu).add_children(&kids);
}

/// The name being typed into the rename dialog, and which emitter it renames.
#[derive(Resource, Default)]
struct RenameDraft {
    emitter: usize,
    text: String,
}

#[derive(Component)]
struct RenameOk;

#[derive(Component)]
struct RenameRoot;

fn open_rename(w: &mut World, index: usize) {
    let Some(name) = w.resource::<VfxEditorState>().doc.emitters.get(index).map(|e| e.name.clone()) else { return };
    {
        let mut d = w.resource_mut::<RenameDraft>();
        d.emitter = index;
        d.text = name.clone();
    }
    let Some(fonts) = w.get_resource::<EmberFonts>().cloned() else { return };
    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, w);
        let (root, content) = overlay_sized(&mut commands, &fonts, "Rename Emitter", 360.0, 150.0, true);
        commands.entity(root).insert(RenameRoot);
        commands.entity(content).insert(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::all(Val::Px(14.0)),
            ..default()
        });
        let input = text_input(&mut commands, &fonts.ui, "Emitter name", &name);
        bind_text_input(&mut commands, input, |w| w.get_resource::<RenameDraft>().map(|d| d.text.clone()).unwrap_or_default(), |w, v| {
            w.resource_mut::<RenameDraft>().text = v;
        });
        let row = commands.spawn(Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::FlexEnd, ..default() }).id();
        let ok = button(&mut commands, &fonts.ui, "Rename");
        commands.entity(ok).insert((RenameOk, renzora_ember::style::Styled::new(renzora_ember::style::Role::ButtonAccent)));
        commands.entity(row).add_child(ok);
        commands.entity(content).add_children(&[input, row]);
    }
    queue.apply(w);
}

fn rename_confirm(
    q: Query<&UiInteraction, (With<RenameOk>, Changed<UiInteraction>)>,
    keys: Res<ButtonInput<KeyCode>>,
    roots: Query<Entity, With<RenameRoot>>,
    draft: Res<RenameDraft>,
    mut state: ResMut<VfxEditorState>,
    mut commands: Commands,
) {
    if roots.is_empty() {
        return;
    }
    let ok = q.iter().any(|i| *i == UiInteraction::Pressed) || keys.just_pressed(KeyCode::Enter);
    if !ok {
        return;
    }
    if model::rename_emitter(&mut state.doc, draft.emitter, &draft.text) || state.doc.emitters.get(draft.emitter).is_some_and(|e| e.name == draft.text.trim()) {
        for r in &roots {
            commands.entity(r).despawn();
        }
    } else {
        renzora::console_log::console_warn("Particles", "That emitter name is empty or already taken");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The palette is built from the catalog, so a node added there is in the
    /// add-node menu with nothing else to register; this holds it to that,
    /// and to every category having its own header colour and icon.
    #[test]
    fn every_catalog_node_but_the_output_is_in_the_palette() {
        assert_eq!(node_entries([0.0, 0.0]).len(), NODES.len() - 1);
        for t in [
            "input/wind",
            "force/wind",
            "color/blackbody",
            "force/speed_limit",
            "force/drag",
            "collide/plane",
            "collide/sphere",
            "collide/box",
        ] {
            let def = node_def(t).expect("in the catalog");
            assert!(categories().contains(&def.category), "{t}");
        }
        for c in categories() {
            assert_ne!(category_icon(c), "circle", "{c} has no icon");
            assert_ne!(category_color(c), (110, 120, 160), "{c} has no colour");
        }
    }
}
