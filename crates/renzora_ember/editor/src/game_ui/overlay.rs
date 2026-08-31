//! The editing overlay that sits over the rendered-canvas image (inside the
//! design frame, so its coordinate space is design × zoom). It is a transparent
//! hit layer (captures clicks/drags for the interaction systems) holding one
//! selection box — with 8 corner/edge handles — per selected widget.
//!
//! Selection boxes are spawned by a `keyed_list` keyed on the *selection set*
//! (so they appear/disappear with selection) and repositioned every frame by
//! [`position_sel_boxes`] from the live widget geometry — so dragging a widget
//! never rebuilds the box.

use std::hash::{Hash, Hasher};

use bevy::math::Rot2;
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, UiTransform};
use bevy::window::SystemCursorIcon;

use renzora::{EditorSelection, SplashState};
use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::reactive::{KeyedSnapshot};
use renzora_ember::reactive::Rx;
use renzora_ember::reactive::tracked::{keyed_list};
use renzora_ember::theme::*;

use crate::game_ui::NativeCanvasState;

/// Transparent full-frame layer that receives canvas clicks/drags.
#[derive(Component)]
pub(crate) struct CanvasHitLayer;

#[derive(Component)]
struct SelBox(Entity);

/// One of the 8 resize handles.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ResizeHandle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

impl ResizeHandle {
    /// Which sides move when dragged: (left, top, right, bottom).
    pub(crate) fn sides(self) -> (bool, bool, bool, bool) {
        match self {
            Self::TopLeft => (true, true, false, false),
            Self::Top => (false, true, false, false),
            Self::TopRight => (false, true, true, false),
            Self::Right => (false, false, true, false),
            Self::BottomRight => (false, false, true, true),
            Self::Bottom => (false, false, false, true),
            Self::BottomLeft => (true, false, false, true),
            Self::Left => (true, false, false, false),
        }
    }

    /// OS resize cursor that matches this handle's drag axis (diagonals for the
    /// corners). Shown on hover so the handle reads as a resize grip.
    fn cursor(self) -> SystemCursorIcon {
        match self {
            Self::TopLeft | Self::BottomRight => SystemCursorIcon::NwseResize,
            Self::TopRight | Self::BottomLeft => SystemCursorIcon::NeswResize,
            Self::Top | Self::Bottom => SystemCursorIcon::NsResize,
            Self::Left | Self::Right => SystemCursorIcon::EwResize,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum HandleKind {
    Resize(ResizeHandle),
    Rotate,
}

/// A grab handle on a selection box — carries the widget it transforms.
#[derive(Component, Clone, Copy)]
pub(crate) struct CanvasHandle {
    pub widget: Entity,
    pub kind: HandleKind,
}

pub(crate) fn register(app: &mut App) {
    app.add_systems(
        Update,
        (
            position_sel_boxes,
            position_marquee,
            position_drop,
            position_drop_slots,
            position_sel_labels,
            position_hover,
        )
            // After the geometry snapshot so the box tracks the same frame's
            // widget sizes instead of trailing a frame behind during a resize.
            .after(crate::game_ui::geometry::snapshot_widgets)
            .run_if(in_state(SplashState::Editor)),
    );
}

/// Marker on the marquee (rubber-band) rectangle drawn during a box-select.
#[derive(Component)]
struct MarqueeRect;

/// Build the overlay layer (added as a child of the design frame, over the image).
pub(crate) fn build(commands: &mut Commands, fonts: &renzora_ember::font::EmberFonts) -> Entity {
    let layer = commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            // Above the backdrop (0) + UI render (1) so handles are visible/clickable.
            ZIndex(5),
            CanvasHitLayer,
            Name::new("ui-canvas-overlay"),
        ))
        .id();
    let boxes = commands
        .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, FocusPolicy::Pass))
        .id();
    keyed_list(commands, boxes, selection_snapshot);
    // Rubber-band rectangle, hidden until a marquee drag is in progress.
    let marquee = commands
        .spawn((
            Node { position_type: PositionType::Absolute, border: UiRect::all(Val::Px(1.0)), ..default() },
            BackgroundColor(rgb(accent()).with_alpha(0.12)),
            BorderColor::all(rgb(accent())),
            FocusPolicy::Pass,
            Visibility::Hidden,
            MarqueeRect,
            Name::new("ui-canvas-marquee"),
        ))
        .id();
    // Flow-drop feedback: the container that would receive the node, named, plus
    // the slot the node would land in. Both are single persistent nodes toggled
    // by `position_drop`, like the marquee — a drag is not the moment to be
    // spawning entities every frame.
    let drop_box = commands
        .spawn((
            Node { position_type: PositionType::Absolute, border: UiRect::all(Val::Px(1.0)), ..default() },
            BorderColor::all(rgb(accent()).with_alpha(0.55)),
            BackgroundColor(rgb(accent()).with_alpha(0.06)),
            FocusPolicy::Pass,
            Visibility::Hidden,
            DropBox,
            Name::new("ui-canvas-dropbox"),
        ))
        .id();
    // The container's name, sitting on its top edge. Which container you are
    // about to drop into is otherwise a guess — nested rows and columns in a
    // template look alike from the outside.
    let drop_label = name_badge(commands, fonts);
    commands.entity(drop_label).insert(DropLabel);
    commands.entity(drop_box).add_child(drop_label);
    let drop_line = commands
        .spawn((
            Node { position_type: PositionType::Absolute, ..default() },
            BackgroundColor(rgb(accent())),
            FocusPolicy::Pass,
            Visibility::Hidden,
            DropLine,
            Name::new("ui-canvas-dropline"),
        ))
        .id();
    // Hover chrome: the node under the cursor, and the container it sits in.
    // Persistent nodes toggled by `position_hover`, like the drop feedback —
    // the cursor moves every frame and this is not the place to spawn entities.
    //
    // The group box is drawn first so the node's own outline sits on top of it;
    // they overlap by definition, and the inner one is the one you are pointing
    // at.
    let hover_group = commands
        .spawn((
            Node { position_type: PositionType::Absolute, border: UiRect::all(Val::Px(1.0)), ..default() },
            BorderColor::all(rgb(accent()).with_alpha(0.35)),
            BackgroundColor(rgb(accent()).with_alpha(0.04)),
            FocusPolicy::Pass,
            Visibility::Hidden,
            HoverGroupBox,
            Name::new("ui-canvas-hovergroup"),
        ))
        .id();
    let hover_group_label = name_badge(commands, fonts);
    commands
        .entity(hover_group_label)
        .insert((HoverGroupLabel, BackgroundColor(rgb(accent()).with_alpha(0.55))));
    commands.entity(hover_group).add_child(hover_group_label);

    let hover_box = commands
        .spawn((
            Node { position_type: PositionType::Absolute, border: UiRect::all(Val::Px(1.0)), ..default() },
            BorderColor::all(rgb(accent()).with_alpha(0.8)),
            FocusPolicy::Pass,
            Visibility::Hidden,
            HoverBox,
            Name::new("ui-canvas-hoverbox"),
        ))
        .id();
    let hover_label = name_badge(commands, fonts);
    commands.entity(hover_label).insert(HoverLabel);
    commands.entity(hover_box).add_child(hover_label);

    // Faint ticks for every other slot in the target container. A fixed pool
    // repositioned each frame — a drag is the worst moment to be spawning.
    let mut slot_ticks: Vec<Entity> = Vec::with_capacity(DROP_SLOTS);
    for _ in 0..DROP_SLOTS {
        slot_ticks.push(
            commands
                .spawn((
                    Node { position_type: PositionType::Absolute, ..default() },
                    BackgroundColor(rgb(accent()).with_alpha(0.30)),
                    FocusPolicy::Pass,
                    Visibility::Hidden,
                    DropSlotTick,
                    Name::new("ui-canvas-dropslot"),
                ))
                .id(),
        );
    }
    commands.entity(layer).add_children(&slot_ticks);

    commands
        .entity(layer)
        .add_children(&[boxes, marquee, hover_group, hover_box, drop_box, drop_line]);
    layer
}

/// How many slot ticks the overlay can show. A container with more children
/// than this is one where the ticks would be a smear anyway.
const DROP_SLOTS: usize = 24;

/// One of the faint "you could also drop here" marks.
#[derive(Component)]
struct DropSlotTick;

/// Show every slot in the drop target's container, faintly.
///
/// The active slot has its own solid line; these are the alternatives. Without
/// them a container whose children are bunched at one end — a row with
/// `justify_content: flex_end` — looks like it has a single fixed drop point at
/// that end, when in fact the cursor picks between a slot before each child.
fn position_drop_slots(
    state: Res<NativeCanvasState>,
    mut ticks: Query<(&mut Node, &mut Visibility), With<DropSlotTick>>,
) {
    let zoom = state.zoom;
    let active = state.drop.map(|d| d.line);
    let mut slots = state.drop_slots.0.iter();
    for (mut node, mut vis) in &mut ticks {
        // The active slot is drawn solid by `position_drop`; skip it here so the
        // two do not stack into a brighter-than-intended line.
        let next = slots
            .by_ref()
            .find(|s| active.is_none_or(|a| (a.0 - s.0).length_squared() > 0.5));
        match next {
            Some((a, b)) => {
                let (min, max) = (a.min(*b), a.max(*b));
                node.left = Val::Px(min.x * zoom - 0.5);
                node.top = Val::Px(min.y * zoom - 0.5);
                node.width = Val::Px((max.x - min.x) * zoom + 1.0);
                node.height = Val::Px((max.y - min.y) * zoom + 1.0);
                if *vis != Visibility::Inherited {
                    *vis = Visibility::Inherited;
                }
            }
            None => {
                if *vis != Visibility::Hidden {
                    *vis = Visibility::Hidden;
                }
            }
        }
    }
}

/// Outline of the node under the cursor.
#[derive(Component)]
struct HoverBox;
/// Its name badge.
#[derive(Component)]
struct HoverLabel;
/// Outline of the container the cursor is inside — the hovered node's parent.
#[derive(Component)]
struct HoverGroupBox;
/// Its name badge.
#[derive(Component)]
struct HoverGroupLabel;

/// Draw the hover chrome from `NativeCanvasState::hovered`.
///
/// Hidden entirely during a drag: the drop feedback is already drawing a
/// container box and an insertion line, and a hover outline underneath it is two
/// overlays answering the same question with different edges.
#[allow(clippy::too_many_arguments)]
fn position_hover(
    state: Res<NativeCanvasState>,
    names: Query<&Name>,
    sources: Query<&renzora_ember::markup::provenance::MarkupSource>,
    mut hover: Query<(&mut Node, &mut Visibility), (With<HoverBox>, Without<HoverGroupBox>)>,
    mut group: Query<(&mut Node, &mut Visibility), (With<HoverGroupBox>, Without<HoverBox>)>,
    mut hover_lbl: Query<
        (&mut bevy::ui::widget::Text, &mut Visibility),
        (With<HoverLabel>, Without<HoverGroupLabel>, Without<HoverBox>, Without<HoverGroupBox>),
    >,
    mut group_lbl: Query<
        (&mut bevy::ui::widget::Text, &mut Visibility),
        (With<HoverGroupLabel>, Without<HoverLabel>, Without<HoverBox>, Without<HoverGroupBox>),
    >,
) {
    let zoom = state.zoom;
    let dragging = state.drop.is_some() || state.marquee.is_some();
    let target = if dragging || !state.hover_outline {
        None
    } else {
        state.hovered
    };
    let geom = target.and_then(|e| state.widgets.iter().find(|g| g.entity == e));
    // The container is the hovered node's parent, and only when it is itself a
    // tracked widget — the canvas root is not one, so pointing at a top-level
    // node does not light up the whole canvas.
    let group_geom = geom
        .filter(|_| state.hover_group)
        .and_then(|g| g.parent)
        .and_then(|p| state.widgets.iter().find(|g| g.entity == p));

    let place = |node: &mut Node, vis: &mut Visibility, g: Option<&crate::game_ui::geometry::WidgetGeom>| {
        match g {
            Some(g) => {
                node.left = Val::Px(g.x * zoom);
                node.top = Val::Px(g.y * zoom);
                node.width = Val::Px(g.width * zoom);
                node.height = Val::Px(g.height * zoom);
                *vis = Visibility::Visible;
            }
            None => *vis = Visibility::Hidden,
        }
    };
    for (mut node, mut vis) in &mut hover {
        place(&mut node, &mut vis, geom);
    }
    for (mut node, mut vis) in &mut group {
        place(&mut node, &mut vis, group_geom);
    }

    let text_for = |e: Option<Entity>| {
        e.filter(|_| state.show_names)
            .map(|e| node_label(e, &names, &sources))
    };
    write_badge(&mut hover_lbl, text_for(geom.map(|g| g.entity)));
    write_badge(&mut group_lbl, text_for(group_geom.map(|g| g.entity)));
}

/// Set a badge's text, or hide it when there is nothing to name.
///
/// Generic over the query filter because the hover badge and the container
/// badge are distinct components with mutually-exclusive filters — a closure
/// cannot take both, since each `Query` is its own type.
fn write_badge<F: bevy::ecs::query::QueryFilter>(
    q: &mut Query<(&mut bevy::ui::widget::Text, &mut Visibility), F>,
    want: Option<String>,
) {
    for (mut text, mut vis) in q.iter_mut() {
        let v = if want.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != v {
            *vis = v;
        }
        if let Some(w) = want.as_ref() {
            if text.0 != *w {
                text.0 = w.clone();
            }
        }
    }
}

/// A node's name, as a small tab sitting on the top-left of the box that frames
/// it. Used for both the drop target and the selection, so the two read as the
/// same piece of information rather than two similar-looking ones.
///
/// Offset up by its own height so it sits *above* the frame rather than over the
/// node's first line of content.
fn name_badge(commands: &mut Commands, fonts: &renzora_ember::font::EmberFonts) -> Entity {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(-1.0),
                top: Val::Px(-15.0),
                padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(rgb(accent())),
            bevy::ui::widget::Text::new(""),
            renzora_ember::font::ui_font(&fonts.ui, 9.0),
            TextColor(Color::WHITE),
            FocusPolicy::Pass,
            Name::new("ui-canvas-name-badge"),
        ))
        .id()
}

/// Best display name for a node: the markup's `name="..."` when it has one,
/// otherwise its tag. A template is mostly anonymous `<node>`s, so falling back
/// to the entity id would label almost everything with a number.
fn node_label(
    entity: Entity,
    names: &Query<&Name>,
    kinds: &Query<&renzora_ember::markup::provenance::MarkupSource>,
) -> String {
    if let Ok(n) = names.get(entity) {
        return n.as_str().to_string();
    }
    if kinds.get(entity).is_ok() {
        return "node".to_string();
    }
    format!("{entity}")
}

/// Outline of the container a flow drag would drop into.
#[derive(Component)]
struct DropBox;

/// The container's name, on the top edge of [`DropBox`].
#[derive(Component)]
struct DropLabel;

/// The slot a flow drag would land in — a line between two siblings.
#[derive(Component)]
struct DropLine;

/// Draw the flow-drop feedback from `NativeCanvasState::drop`, or hide it.
///
/// The label reads the container's `Name` when the markup gave it one
/// (`name="actions"`) and falls back to its entity id, which is still enough to
/// tell two nested columns apart while dragging between them.
fn position_drop(
    state: Res<NativeCanvasState>,
    names: Query<&Name>,
    mut boxes: Query<(&mut Node, &mut Visibility), (With<DropBox>, Without<DropLine>)>,
    mut lines: Query<(&mut Node, &mut Visibility), (With<DropLine>, Without<DropBox>)>,
    mut labels: Query<&mut bevy::ui::widget::Text, With<DropLabel>>,
) {
    let zoom = state.zoom;
    let drop = state.drop;
    for (mut node, mut vis) in &mut boxes {
        match drop {
            Some(d) => {
                let (x, y, w, h) = d.parent_box;
                node.left = Val::Px(x * zoom);
                node.top = Val::Px(y * zoom);
                node.width = Val::Px(w * zoom);
                node.height = Val::Px(h * zoom);
                *vis = Visibility::Visible;
            }
            None => *vis = Visibility::Hidden,
        }
    }
    for (mut node, mut vis) in &mut lines {
        match drop {
            Some(d) => {
                let (a, b) = d.line;
                let (min, max) = (a.min(b), a.max(b));
                // One axis is degenerate; give it a visible thickness centred on
                // the boundary rather than a zero-width node.
                node.left = Val::Px(min.x * zoom - 1.0);
                node.top = Val::Px(min.y * zoom - 1.0);
                node.width = Val::Px((max.x - min.x) * zoom + 2.0);
                node.height = Val::Px((max.y - min.y) * zoom + 2.0);
                *vis = Visibility::Visible;
            }
            None => *vis = Visibility::Hidden,
        }
    }
    for mut text in &mut labels {
        let want = match drop {
            Some(d) => names
                .get(d.parent)
                .map(|n| n.as_str().to_string())
                .unwrap_or_else(|_| format!("{}", d.parent)),
            None => String::new(),
        };
        if text.0 != want {
            text.0 = want;
        }
    }
}

fn selection_snapshot(world: &Rx) -> KeyedSnapshot {
    let selected = world.get_resource::<EditorSelection>().map(|s| s.get_all()).unwrap_or_default();
    let present: Vec<Entity> = match world.get_resource::<NativeCanvasState>() {
        Some(state) => selected.into_iter().filter(|e| state.widgets.iter().any(|g| g.entity == *e)).collect(),
        None => Vec::new(),
    };
    let items: Vec<(u64, u64)> = present
        .iter()
        .map(|e| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            e.hash(&mut k);
            (k.finish(), k.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| sel_box(c, f, present[i])),
    }
}

fn sel_box(
    commands: &mut Commands,
    fonts: &renzora_ember::font::EmberFonts,
    entity: Entity,
) -> Entity {
    let b = commands
        .spawn((
            Node { position_type: PositionType::Absolute, border: UiRect::all(Val::Px(1.0)), ..default() },
            BorderColor::all(rgb(accent())),
            UiTransform::IDENTITY,
            FocusPolicy::Pass,
            SelBox(entity),
            Name::new("ui-canvas-selbox"),
        ))
        .id();
    // The node's name, hidden until `position_sel_labels` decides it applies.
    let label = name_badge(commands, fonts);
    commands
        .entity(label)
        .insert((SelLabel, Visibility::Hidden));
    commands.entity(b).add_child(label);
    // 8 resize handles: 4 corners + 4 edge midpoints, positioned relative to the box.
    let handles = [
        ((0.0, 0.0), ResizeHandle::TopLeft),
        ((0.5, 0.0), ResizeHandle::Top),
        ((1.0, 0.0), ResizeHandle::TopRight),
        ((1.0, 0.5), ResizeHandle::Right),
        ((1.0, 1.0), ResizeHandle::BottomRight),
        ((0.5, 1.0), ResizeHandle::Bottom),
        ((0.0, 1.0), ResizeHandle::BottomLeft),
        ((0.0, 0.5), ResizeHandle::Left),
    ];
    for ((lx, ly), rh) in handles {
        let h = commands
            .spawn((
                Node { position_type: PositionType::Absolute, left: Val::Percent(lx * 100.0), top: Val::Percent(ly * 100.0), width: Val::Px(8.0), height: Val::Px(8.0), margin: UiRect::all(Val::Px(-5.0)), border: UiRect::all(Val::Px(1.0)), ..default() },
                BackgroundColor(rgb(window_bg())),
                BorderColor::all(rgb(accent())),
                Interaction::default(),
                HoverCursor(rh.cursor()),
                CanvasHandle { widget: entity, kind: HandleKind::Resize(rh) },
            ))
            .id();
        commands.entity(b).add_child(h);
    }
    // Rotation handle above the top-center edge.
    let rot = commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Percent(50.0), top: Val::Px(0.0), width: Val::Px(9.0), height: Val::Px(9.0), margin: UiRect { left: Val::Px(-5.0), top: Val::Px(-20.0), ..default() }, border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() },
            BackgroundColor(rgb(window_bg())),
            BorderColor::all(rgb(accent())),
            Interaction::default(),
            HoverCursor(SystemCursorIcon::Grab),
            CanvasHandle { widget: entity, kind: HandleKind::Rotate },
        ))
        .id();
    commands.entity(b).add_child(rot);
    b
}

/// Show the selected (or hovered) node's name on its selection box, per
/// [`NodeBadge`].
///
/// The drag already labels its drop target; this answers the same question
/// outside a drag — "which of these boxes am I looking at" — which is the one a
/// template full of anonymous `<node>`s makes hard.
fn position_sel_labels(
    state: Res<NativeCanvasState>,
    selection: Option<Res<EditorSelection>>,
    names: Query<&Name>,
    sources: Query<&renzora_ember::markup::provenance::MarkupSource>,
    boxes: Query<(&SelBox, &Children)>,
    mut labels: Query<(&mut bevy::ui::widget::Text, &mut Visibility), With<SelLabel>>,
) {
    let target = if state.show_names {
        selection.and_then(|s| s.get())
    } else {
        None
    };
    for (sb, kids) in &boxes {
        // Only the box framing the target gets a label — with a multi-selection
        // every box has one of these, and labelling all of them turns the canvas
        // into a wall of tags.
        let show = target == Some(sb.0);
        let text = if show {
            node_label(sb.0, &names, &sources)
        } else {
            String::new()
        };
        for kid in kids.iter() {
            let Ok((mut t, mut vis)) = labels.get_mut(kid) else {
                continue;
            };
            let want_vis = if show {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *vis != want_vis {
                *vis = want_vis;
            }
            if show && t.0 != text {
                t.0 = text.clone();
            }
        }
    }
}

/// The name badge attached to a selection box.
#[derive(Component)]
struct SelLabel;

/// Reposition + rotate each selection box from the live widget geometry × zoom
/// (so the box + its handles track and rotate with the widget).
fn position_sel_boxes(state: Res<NativeCanvasState>, mut q: Query<(&SelBox, &mut Node, &mut UiTransform)>) {
    let zoom = state.zoom;
    for (sb, mut node, mut tf) in &mut q {
        if let Some(g) = state.widgets.iter().find(|g| g.entity == sb.0) {
            node.left = Val::Px(g.x * zoom);
            node.top = Val::Px(g.y * zoom);
            node.width = Val::Px(g.width * zoom);
            node.height = Val::Px(g.height * zoom);
            tf.rotation = Rot2::radians(g.rotation);
        }
    }
}

/// Draw / hide the marquee rectangle from `NativeCanvasState.marquee`
/// (design-space corners) in frame space (× zoom).
fn position_marquee(state: Res<NativeCanvasState>, mut q: Query<(&mut Node, &mut Visibility), With<MarqueeRect>>) {
    let zoom = state.zoom;
    for (mut node, mut vis) in &mut q {
        match state.marquee {
            Some((a, b)) => {
                let (min, max) = (a.min(b), a.max(b));
                node.left = Val::Px(min.x * zoom);
                node.top = Val::Px(min.y * zoom);
                node.width = Val::Px((max.x - min.x) * zoom);
                node.height = Val::Px((max.y - min.y) * zoom);
                *vis = Visibility::Visible;
            }
            None => *vis = Visibility::Hidden,
        }
    }
}
