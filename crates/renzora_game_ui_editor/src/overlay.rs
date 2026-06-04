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

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_editor::{EditorSelection, SplashState};
use renzora_ember::reactive::{keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;

use crate::NativeCanvasState;

/// Transparent full-frame layer that receives canvas clicks/drags.
#[derive(Component)]
pub(crate) struct CanvasHitLayer;

#[derive(Component)]
struct SelBox(Entity);

pub(crate) fn register(app: &mut App) {
    app.add_systems(Update, position_sel_boxes.run_if(in_state(SplashState::Editor)));
}

/// Build the overlay layer (added as a child of the design frame, over the image).
pub(crate) fn build(commands: &mut Commands) -> Entity {
    let layer = commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            CanvasHitLayer,
            Name::new("ui-canvas-overlay"),
        ))
        .id();
    let boxes = commands
        .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, FocusPolicy::Pass))
        .id();
    keyed_list(commands, boxes, selection_snapshot);
    commands.entity(layer).add_child(boxes);
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
            FocusPolicy::Pass,
            SelBox(entity),
            Name::new("ui-canvas-selbox"),
        ))
        .id();
    // 8 handles: 4 corners + 4 edge midpoints, positioned relative to the box.
    for (lx, ly) in [(0.0, 0.0), (0.5, 0.0), (1.0, 0.0), (1.0, 0.5), (1.0, 1.0), (0.5, 1.0), (0.0, 1.0), (0.0, 0.5)] {
        let h = commands
            .spawn((
                Node { position_type: PositionType::Absolute, left: Val::Percent(lx * 100.0), top: Val::Percent(ly * 100.0), width: Val::Px(7.0), height: Val::Px(7.0), margin: UiRect::all(Val::Px(-4.0)), border: UiRect::all(Val::Px(1.0)), ..default() },
                BackgroundColor(rgb(window_bg())),
                BorderColor::all(rgb(accent())),
                FocusPolicy::Pass,
            ))
            .id();
        commands.entity(b).add_child(h);
    }
    b
}

/// Reposition each selection box from the live widget geometry × zoom.
fn position_sel_boxes(state: Res<NativeCanvasState>, mut q: Query<(&SelBox, &mut Node)>) {
    let zoom = state.zoom;
    for (sb, mut node) in &mut q {
        if let Some(g) = state.widgets.iter().find(|g| g.entity == sb.0) {
            node.left = Val::Px(g.x * zoom);
            node.top = Val::Px(g.y * zoom);
            node.width = Val::Px(g.width * zoom);
            node.height = Val::Px(g.height * zoom);
        }
    }
}
