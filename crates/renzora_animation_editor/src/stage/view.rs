//! The stage panel (`stage`), its camera, and where the cursor is on it.
//!
//! The header carries the mode tabs (Rig, Skin, Shapes, Animate, Logic), the model
//! on the stage and the camera presets. Over the picture float a toolbar
//! (skeleton, floor, auto key, frame), in Animate the gizmo's tool strip down
//! the right, and a line along the bottom saying what a click does now. With
//! nothing on the stage, the middle of it lists the models to choose from. In
//! Logic the state machine takes the body and the picture moves to the
//! corner, so the graph has the room and the character is still in sight.
//!
//! The left mouse button is the tools' (bones, the gizmo, points, the brush),
//! so the camera is on the others: right drag orbits, middle drag pans, the
//! wheel zooms, and Alt with the left button orbits for a trackpad. F frames
//! the model again.

use std::hash::{Hash, Hasher};

use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_display, bind_text, bind_text_color, bind_with, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::dropdown_compact;
use renzora_ember::UiInteraction;

use super::models::{choices, choose, Choice};
use super::{StageMode, StageUi, Tool, STAGE_PANEL};
use crate::rig::on_click;
use crate::studio_preview::{StudioPreviewCamera, StudioPreviewImage, StudioPreviewOrbit, StudioPreviewTracker};

/// The stage's picture: where the cursor is measured against.
#[derive(Component)]
pub(crate) struct StageImage;

/// Where the cursor is on the stage, this frame.
#[derive(Resource, Default)]
pub(crate) struct StageCursor {
    /// Over the picture, and not over something drawn on top of it.
    pub(crate) over: bool,
    /// In the picture's pixels, which are the stage camera's viewport pixels.
    pub(crate) px: Vec2,
    pub(crate) ray: Option<Ray3d>,
}

/// The camera is being dragged: the tools leave the mouse alone.
#[derive(Resource, Default)]
pub(crate) struct StageNav {
    pub(crate) dragging: Option<MouseButton>,
}

/// The cursor, the camera and the tool keys, read before any tool acts on a
/// click.
#[derive(SystemSet, Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) struct StageInputSet;

pub(super) fn register(app: &mut App) {
    app.init_resource::<StageCursor>().init_resource::<StageNav>();
    app.register_panel_content(STAGE_PANEL, false, build).systems(
        Update,
        (update_cursor, navigate, tool_keys)
            .chain()
            .in_set(StageInputSet)
            .run_if(in_state(renzora_editor_framework::SplashState::Editor))
            .run_if(super::stage_mounted),
    );
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

fn mode_is(w: &Rx, m: StageMode) -> bool {
    w.get_resource::<StageUi>().is_some_and(|u| u.mode == m)
}

fn has_model(w: &Rx) -> bool {
    w.get_resource::<StudioPreviewTracker>().is_some_and(|t| t.has_model)
}

/// The floating surfaces' fill: the panel colour, nearly opaque, so the
/// character reads through at the edges without the text losing contrast.
fn glass() -> Color {
    rgb(faint_bg()).with_alpha(0.88)
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() },
            BackgroundColor(rgb(panel_bg())),
            Name::new("stage"),
        ))
        .id();
    let header = build_header(commands, fonts);
    let body = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(0.0), position_type: PositionType::Relative, ..default() })
        .id();

    // Logic: the state machine fills the body.
    let logic = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            top: Val::Px(0.0),
            bottom: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            overflow: Overflow::scroll_y(),
            ..default()
        })
        .id();
    let graph = crate::state_machine::build(commands, fonts);
    commands.entity(logic).add_child(graph);
    bind_display(commands, logic, |w| mode_is(w, StageMode::Logic));

    // The picture: the whole body, or the corner in Logic.
    let frame = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(rgb(border())),
        ))
        .id();
    bind_with(
        commands,
        frame,
        |w| mode_is(w, StageMode::Logic),
        |w, e, corner: &bool| {
            if let Some(mut n) = w.get_mut::<Node>(e) {
                if *corner {
                    n.left = Val::Auto;
                    n.top = Val::Auto;
                    n.right = Val::Px(12.0);
                    n.bottom = Val::Px(12.0);
                    n.width = Val::Px(300.0);
                    n.height = Val::Px(220.0);
                    n.border = UiRect::all(Val::Px(1.0));
                    n.border_radius = BorderRadius::all(Val::Px(8.0));
                } else {
                    n.left = Val::Px(0.0);
                    n.top = Val::Px(0.0);
                    n.right = Val::Px(0.0);
                    n.bottom = Val::Px(0.0);
                    n.width = Val::Auto;
                    n.height = Val::Auto;
                    n.border = UiRect::all(Val::Px(0.0));
                    n.border_radius = BorderRadius::ZERO;
                }
            }
        },
    );
    let img = commands
        .spawn((
            ImageNode::default(),
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            BackgroundColor(rgb(panel_bg())),
            UiInteraction::default(),
            RelativeCursorPosition::default(),
            StageImage,
            Name::new("stage-image"),
        ))
        .id();
    bind_with(
        commands,
        img,
        |w| w.get_resource::<StudioPreviewImage>().map(|p| p.handle.clone()),
        |w, e, h: &Option<Handle<Image>>| {
            if let (Some(h), Some(mut n)) = (h, w.get_mut::<ImageNode>(e)) {
                if n.image != *h {
                    n.image = h.clone();
                }
            }
        },
    );
    let toolbar = build_toolbar(commands, fonts);
    let tools = build_tool_strip(commands, fonts);
    let hint = build_hint(commands, fonts);
    let empty = build_empty(commands, fonts);
    commands.entity(frame).add_children(&[img, toolbar, tools, hint, empty]);
    commands.entity(body).add_children(&[logic, frame]);
    commands.entity(root).add_children(&[header, body]);
    root
}

// ── Buttons ──────────────────────────────────────────────────────────────────

/// A button with an icon and a label, lit while `on`: the stage's own, so the
/// mode tabs, the toolbar and the tool strip all read as one set.
fn pill(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    size: f32,
    on: impl for<'w> Fn(&Rx<'w>) -> bool + Send + Sync + Clone + 'static,
    click: impl Fn(&mut World) + Send + Sync + 'static,
) -> Entity {
    let b = commands
        .spawn((
            Node {
                height: Val::Px(size),
                padding: UiRect::horizontal(Val::Px(if label.is_empty() { 0.0 } else { 10.0 })),
                min_width: Val::Px(size),
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    let lit = on.clone();
    bind_bg(commands, b, move |w| {
        let hot = matches!(w.get::<UiInteraction>(b), Some(UiInteraction::Hovered | UiInteraction::Pressed));
        match (lit(w), hot) {
            (true, _) => rgb(accent()),
            (false, true) => rgb(hover_bg()),
            _ => Color::NONE,
        }
    });
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 14.0);
    let lit = on.clone();
    bind_text_color(commands, ic, move |w| if lit(w) { rgb(on_accent()) } else { rgb(text_muted()) });
    commands.entity(b).add_child(ic);
    if !label.is_empty() {
        let t = commands.spawn((Text::new(label), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())))).id();
        bind_text_color(commands, t, move |w| if on(w) { rgb(on_accent()) } else { rgb(text_primary()) });
        commands.entity(b).add_child(t);
    }
    on_click(commands, b, click);
    b
}

/// A rounded strip holding pills.
fn tray(commands: &mut Commands, vertical: bool, fill: Color) -> Entity {
    commands
        .spawn((
            Node {
                flex_direction: if vertical { FlexDirection::Column } else { FlexDirection::Row },
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(3.0)),
                column_gap: Val::Px(2.0),
                row_gap: Val::Px(2.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(fill),
            BorderColor::all(rgb(border())),
        ))
        .id()
}

fn divider(commands: &mut Commands, vertical: bool) -> Entity {
    commands
        .spawn((
            Node {
                width: if vertical { Val::Px(18.0) } else { Val::Px(1.0) },
                height: if vertical { Val::Px(1.0) } else { Val::Px(16.0) },
                margin: UiRect::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(border())),
        ))
        .id()
}

// ── Header ───────────────────────────────────────────────────────────────────

fn build_header(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let header = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(40.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(rgb(header_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();

    let modes = tray(commands, false, rgb(faint_bg()));
    for (m, icon) in StageMode::ALL.into_iter().zip(["bone", "paint-brush", "drop-half", "person-simple-run", "graph"]) {
        let p = pill(commands, fonts, icon, m.label(), 28.0, move |w| mode_is(w, m), move |w| w.resource_mut::<StageUi>().mode = m);
        commands.entity(modes).add_child(p);
    }
    let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();

    // The model on the stage.
    let subject = commands.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
    let person = icon_text(commands, &fonts.phosphor, "person", text_muted(), 14.0);
    let picker = commands.spawn(Node { align_items: AlignItems::Center, ..default() }).id();
    keyed_list(commands, picker, subject_snapshot);
    commands.entity(subject).add_children(&[person, picker]);

    let views = tray(commands, false, rgb(faint_bg()));
    for (i, (icon, label)) in [("user-focus", "Front"), ("arrows-out-cardinal", "Side"), ("cube", "Persp")].into_iter().enumerate() {
        let p = pill(
            commands,
            fonts,
            icon,
            label,
            26.0,
            move |w| w.get_resource::<StageUi>().is_some_and(|u| u.view == i),
            move |w| set_view(w, i),
        );
        commands.entity(views).add_child(p);
    }
    bind_display(commands, views, |w| !mode_is(w, StageMode::Logic));
    commands.entity(header).add_children(&[modes, spacer, subject, views]);
    header
}

fn set_view(w: &mut World, i: usize) {
    w.resource_mut::<StageUi>().view = i;
    if let Some(mut orbit) = w.get_resource_mut::<StudioPreviewOrbit>() {
        let (yaw, pitch) = match i {
            0 => (0.0, 0.05),
            1 => (std::f32::consts::FRAC_PI_2, 0.05),
            _ => (0.5, 0.2),
        };
        orbit.yaw = yaw;
        orbit.pitch = pitch;
    }
}

fn subject_snapshot(w: &Rx) -> KeyedSnapshot {
    let world = w.untracked();
    let options = choices(world);
    let current = crate::rig::subject(world);
    let selected = options.iter().position(|c| match c {
        Choice::Instance(e, _) => Some(*e) == current || current.is_some_and(|c| is_under(world, *e, c)),
        Choice::File(_) => false,
    });
    let mut h = hasher();
    options.hash(&mut h);
    selected.hash(&mut h);
    KeyedSnapshot {
        items: vec![(1, h.finish())],
        build: Box::new(move |c, f, _| {
            if options.is_empty() {
                return c.spawn((Text::new("No models in this project"), ui_font(&f.ui, 12.0), TextColor(rgb(text_muted())))).id();
            }
            let mut labels: Vec<String> = Vec::with_capacity(options.len() + 1);
            if selected.is_none() {
                labels.push("Choose a model".to_string());
            }
            labels.extend(options.iter().map(Choice::label));
            let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
            let offset = usize::from(selected.is_none());
            let initial = selected.map(|s| s + offset).unwrap_or(0);
            let dd = dropdown_compact(c, f, &refs, initial, 210.0);
            let options = options.clone();
            bind_2way(
                c,
                dd,
                move |_: &Rx| initial,
                move |w: &mut World, i: &usize| {
                    if let Some(choice) = i.checked_sub(offset).and_then(|i| options.get(i)) {
                        choose(w, choice);
                    }
                },
            );
            dd
        }),
    }
}

/// Whether `inner` is `outer` or one of its descendants.
fn is_under(world: &World, inner: Entity, outer: Entity) -> bool {
    std::iter::successors(Some(inner), |&e| world.get::<ChildOf>(e).map(|c| c.parent())).any(|e| e == outer)
}

// ── Overlays ─────────────────────────────────────────────────────────────────

fn build_toolbar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = tray(commands, false, glass());
    commands.entity(bar).entry::<Node>().and_modify(|mut n| {
        n.position_type = PositionType::Absolute;
        n.left = Val::Px(12.0);
        n.top = Val::Px(12.0);
    });
    let skeleton = pill(
        commands,
        fonts,
        "bone",
        "Skeleton",
        26.0,
        |w| w.get_resource::<StageUi>().is_some_and(|u| u.show_skeleton),
        |w| {
            let mut u = w.resource_mut::<StageUi>();
            u.show_skeleton = !u.show_skeleton;
        },
    );
    let floor = pill(
        commands,
        fonts,
        "grid-four",
        "Floor",
        26.0,
        |w| w.get_resource::<crate::studio_preview::StudioPreviewSettings>().is_some_and(|s| s.show_floor),
        |w| {
            let mut s = w.resource_mut::<crate::studio_preview::StudioPreviewSettings>();
            s.show_floor = !s.show_floor;
        },
    );
    let autokey = pill(
        commands,
        fonts,
        "record",
        "Auto Key",
        26.0,
        |w| w.get_resource::<crate::AnimationEditorState>().is_some_and(|s| s.record_enabled),
        |w| {
            let mut s = w.resource_mut::<crate::AnimationEditorState>();
            s.record_enabled = !s.record_enabled;
        },
    );
    bind_display(commands, autokey, |w| mode_is(w, StageMode::Animate));
    let gap = divider(commands, false);
    let frame = pill(commands, fonts, "frame-corners", "", 26.0, |_| false, |w| w.resource_mut::<StudioPreviewTracker>().auto_fitted = false);
    let onion = super::onion::toolbar_toggle(commands, fonts);
    commands.entity(bar).add_children(&[skeleton, floor, autokey, onion, gap, frame]);
    bind_display(commands, bar, |w| !mode_is(w, StageMode::Logic) && has_model(w));
    bar
}

/// Animate's gizmo tools, down the right edge where a hand on the mouse
/// reaches for them.
fn build_tool_strip(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let strip = tray(commands, true, glass());
    commands.entity(strip).entry::<Node>().and_modify(|mut n| {
        n.position_type = PositionType::Absolute;
        n.right = Val::Px(12.0);
        n.top = Val::Px(12.0);
    });
    let rotate = pill(
        commands,
        fonts,
        "arrows-clockwise",
        "",
        30.0,
        |w| w.get_resource::<StageUi>().is_some_and(|u| u.tool == Tool::Rotate),
        |w| w.resource_mut::<StageUi>().tool = Tool::Rotate,
    );
    let mv = pill(
        commands,
        fonts,
        "arrows-out-cardinal",
        "",
        30.0,
        |w| w.get_resource::<StageUi>().is_some_and(|u| u.tool == Tool::Move),
        |w| w.resource_mut::<StageUi>().tool = Tool::Move,
    );
    let gap = divider(commands, true);
    let local = pill(
        commands,
        fonts,
        "cube",
        "",
        30.0,
        |w| w.get_resource::<StageUi>().is_some_and(|u| u.local),
        |w| w.resource_mut::<StageUi>().local = true,
    );
    let world = pill(
        commands,
        fonts,
        "globe",
        "",
        30.0,
        |w| w.get_resource::<StageUi>().is_some_and(|u| !u.local),
        |w| w.resource_mut::<StageUi>().local = false,
    );
    commands.entity(strip).add_children(&[rotate, mv, gap, local, world]);
    bind_display(commands, strip, |w| mode_is(w, StageMode::Animate) && has_model(w));
    strip
}

/// The line along the bottom saying what a click does now.
fn build_hint(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let wrap = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(14.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            bevy::picking::Pickable::IGNORE,
        ))
        .id();
    let bar = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(7.0)),
                column_gap: Val::Px(8.0),
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(14.0)),
                max_width: Val::Percent(86.0),
                ..default()
            },
            BackgroundColor(glass()),
            BorderColor::all(rgb(border())),
            bevy::picking::Pickable::IGNORE,
        ))
        .id();
    let dot = commands
        .spawn((
            Node { width: Val::Px(8.0), height: Val::Px(8.0), border_radius: BorderRadius::all(Val::Px(4.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(rgb(accent())),
            bevy::picking::Pickable::IGNORE,
        ))
        .id();
    bind_bg(commands, dot, |w| {
        match w.get_resource::<crate::rig::markers::Markers>().and_then(|m| m.placing) {
            Some((set, i)) => set.colour(i),
            None => rgb(accent()),
        }
    });
    let text = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())), bevy::picking::Pickable::IGNORE))
        .id();
    bind_text(commands, text, hint_text);
    commands.entity(bar).add_children(&[dot, text]);
    commands.entity(wrap).add_child(bar);
    bind_display(commands, wrap, |w| !mode_is(w, StageMode::Logic) && has_model(w));
    wrap
}

fn hint_text(w: &Rx) -> String {
    let mode = w.get_resource::<StageUi>().map(|u| u.mode).unwrap_or_default();
    if let Some((set, i)) = w.get_resource::<crate::rig::markers::Markers>().and_then(|m| m.placing) {
        let label = set.labels().get(i).copied().unwrap_or("point");
        let side = if set.mirrored(i) { " One side is enough." } else { "" };
        return format!("Click the model at: {label}.{side} Esc to stop.");
    }
    let rigged = w.get_resource::<super::clone::StageRig>().is_some_and(|r| r.rigged());
    match mode {
        StageMode::Rig if !rigged => "No skeleton yet. Place the points from the panel on the left, then build.".to_string(),
        StageMode::Rig => "Click a bone to read it in the panel on the right.".to_string(),
        StageMode::Skin if !rigged => "Weights need a skeleton. Build one in Rig first.".to_string(),
        StageMode::Skin => "Paint with the left button. Ctrl+click a bone to paint it.".to_string(),
        StageMode::Shapes => "Sculpt the shape with the left button. Ctrl inverts, Shift smooths, [ and ] size the brush.".to_string(),
        StageMode::Animate if !rigged => "No skeleton to pose. Build one in Rig first.".to_string(),
        StageMode::Animate => "Click a bone. E turns it, W moves it, K keys the pose.".to_string(),
        StageMode::Logic => String::new(),
    }
}

/// Nothing on the stage: the models to choose from, in the middle of it.
fn build_empty(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let overlay = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            top: Val::Px(0.0),
            bottom: Val::Px(0.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .id();
    let card = commands
        .spawn((
            Node {
                width: Val::Px(440.0),
                max_width: Val::Percent(90.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(18.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(glass()),
            BorderColor::all(rgb(border())),
        ))
        .id();
    let title = commands.spawn((Text::new("Choose a character"), ui_font(&fonts.ui, 15.0), TextColor(rgb(text_primary())))).id();
    let sub = commands
        .spawn((
            Text::new("A model from the scene, or a file from the project, which is added to the scene."),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::bottom(Val::Px(4.0)), ..default() },
        ))
        .id();
    let list = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), max_height: Val::Px(360.0), overflow: Overflow::scroll_y(), ..default() })
        .id();
    keyed_list(commands, list, choice_list_snapshot);
    commands.entity(card).add_children(&[title, sub, list]);
    commands.entity(overlay).add_child(card);
    bind_display(commands, overlay, |w| !has_model(w) && !mode_is(w, StageMode::Logic));
    overlay
}

fn choice_list_snapshot(w: &Rx) -> KeyedSnapshot {
    if has_model(w) {
        return KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) };
    }
    let options = choices(w.untracked());
    let items = options
        .iter()
        .map(|c| {
            let mut k = hasher();
            c.hash(&mut k);
            (k.finish(), 0)
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let choice = options[i].clone();
            // A name over where it lives: files often share a name across
            // folders, so the folder is what tells two of them apart.
            let (icon, label, note) = match &choice {
                Choice::Instance(_, n) => ("person", n.clone(), "In the scene".to_string()),
                Choice::File(rel) => match rel.rsplit_once('/') {
                    Some((dir, file)) => ("plus", file.to_string(), dir.to_string()),
                    None => ("plus", rel.clone(), "Project folder".to_string()),
                },
            };
            let row = c
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(40.0),
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(10.0),
                        padding: UiRect::horizontal(Val::Px(8.0)),
                        border_radius: BorderRadius::all(Val::Px(6.0)),
                        flex_shrink: 0.0,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    UiInteraction::default(),
                    renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                ))
                .id();
            bind_bg(c, row, move |w| {
                if matches!(w.get::<UiInteraction>(row), Some(UiInteraction::Hovered | UiInteraction::Pressed)) { rgb(hover_bg()) } else { Color::NONE }
            });
            let ic = icon_text(c, &f.phosphor, icon, accent(), 14.0);
            let text = c
                .spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, flex_shrink: 1.0, min_width: Val::Px(0.0), row_gap: Val::Px(1.0), overflow: Overflow::clip(), ..default() })
                .id();
            let name = c.spawn((Text::new(label.clone()), ui_font(&f.ui, 12.5), TextColor(rgb(text_primary())))).id();
            renzora_ember::widgets::ellipsize(c, name, label, Some(row));
            let where_ = c.spawn((Text::new(note.clone()), ui_font(&f.ui, 10.5), TextColor(rgb(text_muted())))).id();
            renzora_ember::widgets::ellipsize(c, where_, note, Some(row));
            c.entity(text).add_children(&[name, where_]);
            c.entity(row).add_children(&[ic, text]);
            on_click(c, row, move |w| choose(w, &choice));
            row
        }),
    }
}

// ── Input ────────────────────────────────────────────────────────────────────

/// Where the cursor is on the picture, and the camera ray through it. Also
/// sizes the render target to the picture, so a pixel on the stage is a pixel
/// of the camera's.
fn update_cursor(
    images: Query<(&ComputedNode, &RelativeCursorPosition, &InheritedVisibility, &UiInteraction), With<StageImage>>,
    mut preview: ResMut<StudioPreviewImage>,
    cams: Query<(&Camera, &GlobalTransform), With<StudioPreviewCamera>>,
    mut cursor: ResMut<StageCursor>,
) {
    *cursor = StageCursor::default();
    let Some((node, rel, _, hover)) = images.iter().find(|(n, _, v, _)| v.get() && n.size().x > 8.0 && n.size().y > 8.0) else {
        return;
    };
    let size = node.size();
    let want = (size.x.round() as u32, size.y.round() as u32);
    if preview.requested_size != want {
        preview.requested_size = want;
    }
    // Inside the picture is not enough: a toolbar or a popup drawn over it has
    // the pointer, and a click there is the toolbar's, not a bone's.
    let picked = matches!(hover, UiInteraction::Hovered | UiInteraction::Pressed);
    let (Some(normalized), true, true) = (rel.normalized, rel.cursor_over, picked) else { return };
    let (w, h) = preview.current_size;
    let px = (normalized + Vec2::splat(0.5)) * Vec2::new(w as f32, h as f32);
    cursor.over = true;
    cursor.px = px;
    if let Ok((cam, gt)) = cams.single() {
        cursor.ray = cam.viewport_to_world(gt, px).ok();
    }
}

#[allow(clippy::too_many_arguments)]
fn navigate(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut motion: MessageReader<MouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    cursor: Res<StageCursor>,
    focus: Option<Res<renzora::InputFocusState>>,
    mut nav: ResMut<StageNav>,
    mut orbit: ResMut<StudioPreviewOrbit>,
    mut tracker: ResMut<StudioPreviewTracker>,
    mut ui: ResMut<StageUi>,
) {
    let delta: Vec2 = motion.read().map(|m| m.delta).sum();
    let alt = keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
    if nav.dragging.is_none() && cursor.over {
        if mouse.just_pressed(MouseButton::Right) {
            nav.dragging = Some(MouseButton::Right);
        } else if mouse.just_pressed(MouseButton::Middle) {
            nav.dragging = Some(MouseButton::Middle);
        } else if alt && mouse.just_pressed(MouseButton::Left) {
            nav.dragging = Some(MouseButton::Left);
        }
    }
    if let Some(button) = nav.dragging {
        if !mouse.pressed(button) {
            nav.dragging = None;
        } else if delta != Vec2::ZERO {
            let pan = button == MouseButton::Middle || (button == MouseButton::Right && keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]));
            if pan {
                let offset = Vec3::new(orbit.pitch.cos() * orbit.yaw.sin(), orbit.pitch.sin(), orbit.pitch.cos() * orbit.yaw.cos());
                let forward = -offset;
                let right = forward.cross(Vec3::Y).normalize_or_zero();
                let up = right.cross(forward).normalize_or_zero();
                let k = orbit.distance * 0.0016;
                orbit.target += (-right * delta.x + up * delta.y) * k;
            } else {
                orbit.yaw -= delta.x * 0.006;
                orbit.pitch = (orbit.pitch + delta.y * 0.006).clamp(-1.5, 1.5);
                // Orbiting leaves the preset, so the header stops claiming it.
                if ui.view != 2 {
                    ui.view = 2;
                }
            }
        }
    }
    let mut dy = 0.0;
    for ev in wheel.read() {
        dy += match ev.unit {
            MouseScrollUnit::Line => ev.y,
            MouseScrollUnit::Pixel => ev.y / 40.0,
        };
    }
    if dy != 0.0 && cursor.over {
        orbit.distance = (orbit.distance * (1.0 - dy * 0.1)).clamp(0.05, 200.0);
    }
    let typing = focus.is_some_and(|f| f.ui_wants_keyboard);
    if cursor.over && !typing && keys.just_pressed(KeyCode::KeyF) {
        tracker.auto_fitted = false;
    }
}

/// E and W pick the gizmo's tool, Esc stops placing points: with the cursor on
/// the stage, so the same keys still drive the scene viewport elsewhere.
fn tool_keys(
    keys: Res<ButtonInput<KeyCode>>,
    cursor: Res<StageCursor>,
    focus: Option<Res<renzora::InputFocusState>>,
    mut ui: ResMut<StageUi>,
    mut markers: ResMut<crate::rig::markers::Markers>,
) {
    if keys.just_pressed(KeyCode::Escape) && markers.placing.is_some() {
        markers.placing = None;
    }
    if !cursor.over || focus.is_some_and(|f| f.ui_wants_keyboard) {
        return;
    }
    if keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight]) {
        return;
    }
    if keys.just_pressed(KeyCode::KeyE) {
        ui.tool = Tool::Rotate;
    } else if keys.just_pressed(KeyCode::KeyW) {
        ui.tool = Tool::Move;
    }
}
