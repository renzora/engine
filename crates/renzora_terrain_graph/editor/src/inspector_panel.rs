//! The Terrain Graph Inspector: the selected node's parameters, the graph's
//! exposed parameters, and the Build controls for the terrain it targets.
//!
//! Every field is bound two-way to the working copy in
//! [`TerrainGraphEditorState`]. A field starts on the catalog default and
//! writes the node's parameter only once it actually changes, so a node you
//! never touched stays short in the file (see `renzora_terrain_graph::format`).
//!
//! The pin beside a number exposes it: the graph then names it, and a terrain
//! using the graph can override it (the seed and peak height of the
//! approved Generator card are exposed parameters).

use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_display, bind_text, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_text_input, checkbox, drag_value, dropdown, text_input, DragRange, DragSnap, HoverTooltip};
use renzora_ember::UiInteraction;
use renzora_terrain_graph::catalog::{node_def, ParamDef, ParamKind};
use renzora_terrain_graph::{BuildState, ParamValue, TerrainGenerator, TerrainGraphBuilds};

use crate::graph_panel::tool_button;
use crate::state::TerrainGraphEditorState;

pub const PANEL: &str = "terrain_graph_inspector";

/// The terrain the open graph targets, resolved once a frame for the panel's
/// bindings (which only get `&World`).
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub struct InspectorTarget(pub Option<Entity>);

#[derive(Component, Clone, Copy)]
enum Action {
    Build,
    Cancel,
    Reroll,
    Expose(u64, &'static str),
}

pub fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            Name::new("terrain-graph-inspector"),
        ))
        .id();
    let none = commands.spawn((Text::new("Open a .tgraph to edit it."), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted())))).id();
    bind_display(commands, none, |w| !open(w));

    // ── Node ──
    let node_box = column(commands);
    bind_display(commands, node_box, open);
    let title = commands.spawn((Text::new(""), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary())))).id();
    bind_text(commands, title, |w| match selected_def(w) {
        Some(d) => format!("{}  ·  {}", d.display_name, d.category),
        None => "No node selected".to_string(),
    });
    let desc = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
    bind_text(commands, desc, |w| selected_def(w).map(|d| d.description.to_string()).unwrap_or_else(|| "Click a node to edit its parameters.".into()));
    let rows = column(commands);
    keyed_list(commands, rows, param_snapshot);
    commands.entity(node_box).add_children(&[title, desc, rows]);

    // ── Exposed ──
    let exposed_box = column(commands);
    bind_display(commands, exposed_box, open);
    let exposed_title = heading(commands, fonts, "Exposed parameters");
    let exposed = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
    bind_text(commands, exposed, |w| {
        let Some(st) = w.get_resource::<TerrainGraphEditorState>() else { return String::new() };
        if st.graph.params.is_empty() {
            return "None yet. The pin beside a number exposes it, so each terrain can set its own.".into();
        }
        st.graph
            .params
            .iter()
            .map(|p| format!("{} ({}): {}", p.label, p.name, st.graph.exposed_value(&p.name).map_or("?".into(), |v| format!("{v:.1}"))))
            .collect::<Vec<_>>()
            .join("\n")
    });
    commands.entity(exposed_box).add_children(&[exposed_title, exposed]);

    // ── Build ──
    let build_box = column(commands);
    bind_display(commands, build_box, open);
    let build_title = heading(commands, fonts, "Build");
    let target = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
    bind_text(commands, target, |w| {
        let Some(e) = w.get_resource::<InspectorTarget>().and_then(|t| t.0) else {
            return "No terrain: select one to build this graph into.".into();
        };
        let name = w.get::<Name>(e).map_or_else(|| format!("{e}"), |n| n.as_str().to_string());
        let path = w.get_resource::<TerrainGraphEditorState>().and_then(|s| s.path.clone());
        match w.get::<TerrainGenerator>(e) {
            Some(g) if Some(&g.graph) == path.as_ref() => format!("Terrain: {name}"),
            Some(g) if !g.graph.is_empty() => format!("Terrain: {name} (uses {}; Build switches it to this graph)", g.graph),
            _ => format!("Terrain: {name} (Build points it at this graph)"),
        }
    });
    let seed_row = row(commands, fonts, "Seed");
    let seed = drag_value(commands, &fonts.ui, "", value_text(), 1337.0, 1.0);
    commands.entity(seed).insert((DragSnap(1.0), DragRange { min: 0.0, max: 9_999_999.0 }));
    bind_2way(
        commands,
        seed,
        |w: &Rx| {
            w.get_resource::<InspectorTarget>()
                .and_then(|t| t.0)
                .and_then(|e| w.get::<TerrainGenerator>(e))
                .map_or(1337.0, |g| g.seed as f32)
        },
        |w: &mut World, v: &f32| set_seed(w, v.max(0.0).round() as u64),
    );
    grow(commands, seed);
    let reroll = tool_button(commands, fonts, "dice-five", None, "A new seed: the same kind of landscape, a different one", false);
    commands.entity(reroll).insert(Action::Reroll);
    commands.entity(seed_row).add_children(&[seed, reroll]);

    let buttons = commands.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() }).id();
    let build = tool_button(commands, fonts, "hammer", Some("Build"), "Build the terrain at full resolution; sculpting is kept", true);
    commands.entity(build).insert(Action::Build);
    let cancel = tool_button(commands, fonts, "x", Some("Cancel"), "Stop the build", false);
    commands.entity(cancel).insert(Action::Cancel);
    bind_display(commands, cancel, |w| running(w).is_some());
    commands.entity(buttons).add_children(&[build, cancel]);
    let status = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
    bind_text(commands, status, |w| {
        let Some(e) = w.get_resource::<InspectorTarget>().and_then(|t| t.0) else { return String::new() };
        match w.get_resource::<TerrainGraphBuilds>().and_then(|b| b.get(e)).map(|b| b.state.clone()) {
            Some(BuildState::Running { progress }) => format!("Building {:.0}%", progress * 100.0),
            Some(BuildState::Done { summary }) => format!("Built: {summary}"),
            Some(BuildState::Failed { error }) => format!("Failed: {error}"),
            Some(BuildState::Cancelled) => "Cancelled".into(),
            None => "Sculpting stays on its own layer above the generator, so a rebuild keeps your hand edits.".into(),
        }
    });
    commands.entity(build_box).add_children(&[build_title, target, seed_row, buttons, status]);

    commands.entity(root).add_children(&[none, node_box, exposed_box, build_box]);
    root
}

fn open(w: &Rx) -> bool {
    w.get_resource::<TerrainGraphEditorState>().is_some_and(TerrainGraphEditorState::is_open)
}

fn running(w: &Rx) -> Option<f32> {
    let e = w.get_resource::<InspectorTarget>()?.0?;
    w.get_resource::<TerrainGraphBuilds>()?.progress(e)
}

fn selected_def(w: &Rx) -> Option<&'static renzora_terrain_graph::catalog::NodeDef> {
    let st = w.get_resource::<TerrainGraphEditorState>()?;
    let n = st.graph.node(st.selected_node?)?;
    node_def(&n.node_type)
}

fn column(commands: &mut Commands) -> Entity {
    commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() }).id()
}

fn heading(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands.spawn((Text::new(text), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())))).id()
}

fn row(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let r = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), min_height: Val::Px(22.0), ..default() })
        .id();
    let t = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
            Node { width: Val::Px(96.0), flex_shrink: 0.0, overflow: Overflow::clip(), ..default() },
        ))
        .id();
    commands.entity(r).add_child(t);
    r
}

fn grow(commands: &mut Commands, e: Entity) {
    commands.entity(e).entry::<Node>().and_modify(|mut n| {
        n.flex_grow = 1.0;
        n.min_width = Val::Px(40.0);
    });
}

// ── Parameter rows ───────────────────────────────────────────────────────────

fn read(w: &Rx, node: u64, p: &'static ParamDef) -> ParamValue {
    w.get_resource::<TerrainGraphEditorState>()
        .and_then(|st| st.graph.node(node))
        .and_then(|n| n.params.get(p.name).cloned())
        .unwrap_or_else(|| p.default.value())
}

/// Write a parameter, touching the document only when the value moved, and
/// never writing an untouched default into the file.
fn write(w: &mut World, node: u64, p: &'static ParamDef, value: ParamValue) {
    let current = w
        .resource::<TerrainGraphEditorState>()
        .graph
        .node(node)
        .map(|n| n.params.get(p.name).cloned());
    match current {
        None => {}
        Some(Some(v)) if v == value => {}
        Some(None) if value == p.default.value() => {}
        Some(_) => {
            w.resource_mut::<TerrainGraphEditorState>().graph.set_param(node, p.name, value);
        }
    }
}

fn param_snapshot(w: &Rx) -> KeyedSnapshot {
    let Some(st) = w.get_resource::<TerrainGraphEditorState>() else { return empty() };
    let Some(node) = st.selected_node.and_then(|id| st.graph.node(id)) else { return empty() };
    let Some(def) = node_def(&node.node_type) else { return empty() };
    let id = node.id;
    let exposed: Vec<bool> = def.params.iter().map(|p| st.graph.params.iter().any(|e| e.node == id && e.param == p.name)).collect();
    let items = def
        .params
        .iter()
        .zip(&exposed)
        .map(|(p, e)| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            (id, p.name).hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (id, p.name, e).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    let init: Vec<ParamValue> = def.params.iter().map(|p| read(w, id, p)).collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| param_row(c, f, id, &def.params[i], &init[i], exposed[i])),
    }
}

fn param_row(commands: &mut Commands, fonts: &EmberFonts, node: u64, p: &'static ParamDef, init: &ParamValue, exposed: bool) -> Entity {
    let label = match &p.kind {
        ParamKind::Number { unit, .. } if !unit.suffix().is_empty() => format!("{} ({})", p.label, unit.suffix()),
        _ => p.label.to_string(),
    };
    let r = row(commands, fonts, &label);
    if !p.help.is_empty() {
        commands.entity(r).insert(HoverTooltip::new(p.help));
    }
    let control = match &p.kind {
        ParamKind::Number { min, max, .. } | ParamKind::Int { min, max } => {
            let is_int = matches!(p.kind, ParamKind::Int { .. });
            let v = init.as_number().unwrap_or(0.0);
            let step = if is_int { 1.0 } else { (v.abs() * 0.01).max((max - min) * 0.0002).max(0.001) };
            let dv = drag_value(commands, &fonts.ui, "", value_text(), v, step);
            commands.entity(dv).insert(DragRange { min: *min, max: *max });
            if is_int {
                commands.entity(dv).insert(DragSnap(1.0));
            }
            bind_2way(
                commands,
                dv,
                move |w: &Rx| read(w, node, p).as_number().unwrap_or(0.0),
                move |w: &mut World, v: &f32| write(w, node, p, ParamValue::Number(*v)),
            );
            dv
        }
        ParamKind::Bool => {
            let cb = checkbox(commands, init.as_bool().unwrap_or(false));
            bind_2way(
                commands,
                cb,
                move |w: &Rx| read(w, node, p).as_bool().unwrap_or(false),
                move |w: &mut World, v: &bool| write(w, node, p, ParamValue::Bool(*v)),
            );
            cb
        }
        ParamKind::Choice(options) => {
            let labels: Vec<&str> = options.iter().map(|(_, l)| *l).collect();
            let index = |v: &ParamValue| options.iter().position(|(k, _)| Some(*k) == v.as_text()).unwrap_or(0);
            let dd = dropdown(commands, fonts, &labels, index(init));
            bind_2way(
                commands,
                dd,
                move |w: &Rx| index(&read(w, node, p)),
                move |w: &mut World, i: &usize| {
                    if let Some((k, _)) = options.get(*i) {
                        write(w, node, p, ParamValue::Text((*k).to_string()));
                    }
                },
            );
            dd
        }
        ParamKind::Text | ParamKind::Path(_) => {
            let text = init.as_text().unwrap_or("").to_string();
            let placeholder = if matches!(p.kind, ParamKind::Path(_)) { "path/to/heightmap.png" } else { "" };
            let field = text_input(commands, &fonts.ui, placeholder, &text);
            bind_text_input(
                commands,
                field,
                move |w: &Rx| read(w, node, p).as_text().unwrap_or("").to_string(),
                move |w: &mut World, v: String| write(w, node, p, ParamValue::Text(v)),
            );
            field
        }
        ParamKind::Points => {
            let field = text_input(commands, &fonts.ui, "0,0  1,1", &points_text(init));
            bind_text_input(
                commands,
                field,
                move |w: &Rx| points_text(&read(w, node, p)),
                move |w: &mut World, v: String| {
                    if let Some(points) = parse_points(&v) {
                        write(w, node, p, ParamValue::Points(points));
                    }
                },
            );
            field
        }
    };
    grow(commands, control);
    commands.entity(r).add_child(control);
    if matches!(p.kind, ParamKind::Number { .. } | ParamKind::Int { .. }) && p.name != "seed" {
        let pin = expose_button(commands, fonts, exposed);
        commands.entity(pin).insert(Action::Expose(node, p.name));
        commands.entity(r).add_child(pin);
    }
    r
}

fn expose_button(commands: &mut Commands, fonts: &EmberFonts, exposed: bool) -> Entity {
    let btn = commands
        .spawn((
            Node { width: Val::Px(22.0), height: Val::Px(22.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(4.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            HoverTooltip::new(if exposed { "Exposed: terrains using this graph can override it. Click to stop." } else { "Expose, so each terrain using this graph can set its own value" }),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        if matches!(w.get::<UiInteraction>(btn), Some(UiInteraction::Hovered | UiInteraction::Pressed)) {
            rgb(hover_bg())
        } else {
            Color::NONE
        }
    });
    let icon = icon_text(commands, &fonts.phosphor, "push-pin", if exposed { accent() } else { text_muted() }, 12.0);
    commands.entity(icon).insert(renzora_ember::interaction::PASS_THROUGH);
    commands.entity(btn).add_child(icon);
    btn
}

/// `x,y` pairs separated by spaces or semicolons.
pub fn points_text(v: &ParamValue) -> String {
    v.as_points()
        .unwrap_or(&[])
        .iter()
        .map(|[x, y]| format!("{x:.2},{y:.2}"))
        .collect::<Vec<_>>()
        .join("  ")
}

pub fn parse_points(s: &str) -> Option<Vec<[f32; 2]>> {
    let mut out = Vec::new();
    for pair in s.split(|c: char| c == ';' || c.is_whitespace()).filter(|p| !p.is_empty()) {
        let (x, y) = pair.split_once(',')?;
        out.push([x.trim().parse().ok()?, y.trim().parse().ok()?]);
    }
    (out.len() >= 2).then_some(out)
}

fn empty() -> KeyedSnapshot {
    KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) }
}

// ── Systems ──────────────────────────────────────────────────────────────────

pub fn track_target(world: &mut World) {
    let target = InspectorTarget(crate::state::target_terrain(world));
    if *world.resource::<InspectorTarget>() != target {
        world.insert_resource(target);
    }
}

fn set_seed(world: &mut World, seed: u64) {
    let Some(e) = world.resource::<InspectorTarget>().0 else { return };
    let path = world.resource::<TerrainGraphEditorState>().path.clone().unwrap_or_default();
    match world.get_mut::<TerrainGenerator>(e) {
        Some(mut g) => {
            if g.seed != seed {
                g.seed = seed;
            }
        }
        None => {
            world.entity_mut(e).insert(TerrainGenerator::new(path, seed));
        }
    }
}

fn action_click(q: Query<(&UiInteraction, &Action), Changed<UiInteraction>>, mut commands: Commands) {
    for (i, action) in &q {
        if *i != UiInteraction::Pressed {
            continue;
        }
        let action = *action;
        commands.queue(move |w: &mut World| match action {
            Action::Build => crate::build::build_open_graph(w),
            Action::Cancel => {
                if let Some(e) = w.resource::<InspectorTarget>().0 {
                    crate::build::cancel_build(w, e);
                }
            }
            Action::Reroll => {
                let current = w.resource::<InspectorTarget>().0.and_then(|e| w.get::<TerrainGenerator>(e)).map_or(1337, |g| g.seed);
                set_seed(w, current.wrapping_add(1));
            }
            Action::Expose(node, param) => toggle_exposed(w, node, param),
        });
    }
}

/// Expose `node.param` under a name made from the parameter, or stop
/// exposing it.
fn toggle_exposed(world: &mut World, node: u64, param: &str) {
    let mut st = world.resource_mut::<TerrainGraphEditorState>();
    if let Some(i) = st.graph.params.iter().position(|p| p.node == node && p.param == param) {
        st.graph.params.remove(i);
        return;
    }
    let Some(def) = st.graph.node(node).and_then(|n| node_def(&n.node_type)) else { return };
    let Some(p) = def.param(param) else { return };
    let (min, max) = match p.kind {
        ParamKind::Number { min, max, .. } | ParamKind::Int { min, max } => (min, max),
        _ => (0.0, 1.0),
    };
    let mut name = param.to_string();
    let mut n = 2;
    while st.graph.params.iter().any(|e| e.name == name) {
        name = format!("{param}_{n}");
        n += 1;
    }
    let label = format!("{} {}", def.display_name, p.label.to_lowercase());
    st.graph.expose(&name, &label, node, param, min, max);
}

pub struct TerrainGraphInspectorPanel;

impl Plugin for TerrainGraphInspectorPanel {
    fn build(&self, app: &mut App) {
        use renzora_ember::panel::RegisterPanelContent;
        app.init_resource::<InspectorTarget>();
        app.register_panel_content(PANEL, true, build).systems(
            Update,
            (track_target, action_click).run_if(in_state(renzora_editor_framework::SplashState::Editor)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_round_trip_through_text() {
        let v = ParamValue::Points(vec![[0.0, 0.1], [0.5, 0.4], [1.0, 1.0]]);
        let back = parse_points(&points_text(&v)).unwrap();
        assert_eq!(back.len(), 3);
        assert!((back[1][1] - 0.4).abs() < 1e-4);
        assert!(parse_points("0,0").is_none(), "a curve needs two points");
        assert!(parse_points("0,0 nope").is_none());
    }
}
