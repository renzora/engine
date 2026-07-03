//! Drag & drop between templates.
//!
//! - `drag_item` on a node makes it a drag source; its **payload** is the
//!   binding host (so inside a `<for tag="inventory">` row, the payload is that
//!   item entity).
//! - `dropzone drop_tag="basket" on_drop="..."` marks a drop target. On
//!   release over it, the payload entity's `EntityTag` is set to `drop_tag`
//!   (moving it between `<for>` lists, which re-render), and the optional
//!   `on_drop` callback fires to scripts' `on_ui(name, {}, payload_bits)`.
//!
//! On pickup the dragged item is **reparented to its `UiCanvas`** so its
//! `absolute` position is screen-space and it follows the cursor directly (a
//! deep list item's `absolute` is otherwise parent-relative). On a successful
//! drop the payload is retagged and the `<for>` rebuilds (despawning the moved
//! item and creating fresh ones); on a miss the item is reparented back.

use bevy::input::ButtonInput;
use bevy::prelude::*;
use bevy::ui::{GlobalZIndex, RelativeCursorPosition, UiScale};
use bevy::window::PrimaryWindow;
use renzora::{EntityTag, ScriptUiInbox, UiCallback};
use crate::game_ui::UiCanvas;

/// A drag source. `payload` is the data entity that moves (the binding host).
#[derive(Component)]
pub struct DragItem {
    pub payload: Entity,
}

/// A drop target. On drop, retag the payload to `drop_tag` and/or fire
/// `on_drop`.
#[derive(Component)]
pub struct DropZone {
    pub drop_tag: Option<String>,
    pub on_drop: Option<String>,
}

/// In-flight drag bookkeeping.
#[derive(Resource, Default)]
pub struct DragState {
    item: Option<Entity>,
    payload: Option<Entity>,
    /// Original parent, to restore on a missed drop.
    orig_parent: Option<Entity>,
    orig_pos_type: PositionType,
    orig_left: Val,
    orig_top: Val,
}

impl DragState {
    /// True while an item is being dragged.
    pub fn is_dragging(&self) -> bool {
        self.item.is_some()
    }
}

/// Nearest `UiCanvas` ancestor (or self), else the topmost UI node.
fn canvas_of(mut e: Entity, child_of: &Query<&ChildOf>, canvases: &Query<(), With<UiCanvas>>) -> Entity {
    loop {
        if canvases.get(e).is_ok() {
            return e;
        }
        match child_of.get(e) {
            Ok(c) => e = c.parent(),
            Err(_) => return e,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn dnd_system(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    items: Query<(Entity, &Interaction, &DragItem)>,
    zones: Query<(&DropZone, &RelativeCursorPosition)>,
    child_of: Query<&ChildOf>,
    canvases: Query<(), With<UiCanvas>>,
    mut nodes: Query<&mut Node>,
    mut drag: ResMut<DragState>,
    mut inbox: ResMut<ScriptUiInbox>,
    mut commands: Commands,
) {
    let cursor = windows.iter().find_map(|w| w.cursor_position());
    let scale = ui_scale.0.max(f32::EPSILON);

    // ── Pickup → reparent the item to its canvas (screen space) ──
    if mouse.just_pressed(MouseButton::Left) && drag.item.is_none() {
        for (e, interaction, di) in &items {
            if *interaction == Interaction::Pressed {
                if let Ok(n) = nodes.get(e) {
                    drag.orig_pos_type = n.position_type;
                    drag.orig_left = n.left;
                    drag.orig_top = n.top;
                }
                drag.orig_parent = child_of.get(e).ok().map(|c| c.parent());
                let canvas = canvas_of(e, &child_of, &canvases);
                commands.entity(canvas).add_child(e);
                // Render above the drop zones / panels while dragging.
                commands.entity(e).insert(GlobalZIndex(i32::MAX - 1));
                drag.item = Some(e);
                drag.payload = Some(di.payload);
                break;
            }
        }
    }

    // ── Follow the cursor ──
    if let (Some(item), Some(c)) = (drag.item, cursor) {
        if let Ok(mut n) = nodes.get_mut(item) {
            n.position_type = PositionType::Absolute;
            n.left = Val::Px(c.x / scale - 16.0);
            n.top = Val::Px(c.y / scale - 14.0);
        }
    }

    // ── Drop ──
    if mouse.just_released(MouseButton::Left) {
        if let Some(item) = drag.item.take() {
            let payload = drag.payload.take();
            let mut hit = false;
            for (zone, rel) in &zones {
                if rel.cursor_over {
                    hit = true;
                    if let (Some(p), Some(tag)) = (payload, zone.drop_tag.as_ref()) {
                        commands.entity(p).insert(EntityTag { tag: tag.clone() });
                    }
                    if let Some(cb) = &zone.on_drop {
                        inbox.pending.push(UiCallback {
                            name: cb.clone(),
                            args: Default::default(),
                            entity_bits: payload.map(|e| e.to_bits()).unwrap_or(0),
                        });
                    }
                    break;
                }
            }

            if hit {
                // Retag triggers a `<for>` rebuild that recreates fresh items;
                // despawn this reparented one so it isn't orphaned on the canvas.
                commands.entity(item).despawn();
            } else {
                // Missed: return the item to its list + original layout, and
                // drop the drag-time z-index so it sits normally again.
                if let Some(parent) = drag.orig_parent {
                    commands.entity(parent).add_child(item);
                }
                if let Ok(mut n) = nodes.get_mut(item) {
                    n.position_type = drag.orig_pos_type;
                    n.left = drag.orig_left;
                    n.top = drag.orig_top;
                }
                commands.entity(item).remove::<GlobalZIndex>();
            }
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<DragState>()
        .add_systems(Update, dnd_system);
}
