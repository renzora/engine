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
use renzora_ember::reactive::{keyed_list, KeyedSnapshot};
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
        (position_sel_boxes, position_marquee)
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
pub(crate) fn build(commands: &mut Commands) -> Entity {
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
    commands.entity(layer).add_children(&[boxes, marquee]);
    layer
}

fn selection_snapshot(world: &World) -> KeyedSnapshot {
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
        build: Box::new(move |c, _f, i| sel_box(c, present[i])),
    }
}

fn sel_box(commands: &mut Commands, entity: Entity) -> Entity {
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
