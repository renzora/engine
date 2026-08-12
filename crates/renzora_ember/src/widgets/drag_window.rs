//! Drag a floating panel around by a handle inside it.
//!
//! The pattern any floating card wants: a small grip in the header, dragging
//! which moves the *whole card*. That "handle moves its target" indirection is
//! what separates this from [`crate::markup::drag::Draggable`], which moves the
//! tagged node itself — correct for a game-UI element you drag directly, wrong
//! for a window, where tagging the card would make every press on it (buttons,
//! list rows, text) start a drag.
//!
//! ```ignore
//! let card = commands.spawn(( /* absolute-positioned card */ )).id();
//! let grip = drag_grip(&mut commands, &fonts.phosphor, card);
//! commands.entity(header).add_child(grip);
//! ```
//!
//! Three things this handles that a naive `Node.left += delta` does not:
//!
//! - **Anchor handover.** Cards are usually pinned with `right`/`bottom`. The
//!   drag resolves the target's *actual* on-screen rect from `UiGlobalTransform`
//!   on the first frame and then writes `left`/`top`, clearing the opposite pair
//!   — otherwise the two anchor sets fight and the card jumps.
//! - **Staying reachable.** The target is clamped so a margin of it always
//!   remains on screen; a card flung past the edge takes its close button with
//!   it.
//! - **Release anywhere.** Drag end keys off the mouse button, not `Interaction`,
//!   so moving faster than the handle follows doesn't drop the drag.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;

use crate::font::icon_text;
use crate::theme::text_muted;

/// Marker: pressing this entity drags `target` (usually an ancestor).
#[derive(Component, Debug, Clone, Copy)]
pub struct DragHandle {
    /// The entity actually moved — the card/window this handle belongs to.
    pub target: Entity,
    /// How much of the target must stay on screen, in logical px.
    pub margin: f32,
}

impl DragHandle {
    /// A handle that drags `target`, keeping 24px of it on screen.
    pub fn new(target: Entity) -> Self {
        Self {
            target,
            margin: 24.0,
        }
    }

    /// Override how much of the target must remain reachable.
    pub fn with_margin(mut self, margin: f32) -> Self {
        self.margin = margin;
        self
    }
}

/// Live drag bookkeeping, inserted on the handle while the button is held. A
/// component rather than a resource so several draggable windows can coexist.
///
/// `pub` only to satisfy the system signature — `drag_window` is a private
/// module and this is deliberately not re-exported, so it stays internal.
#[derive(Component, Debug, Clone, Copy)]
pub struct HandleDrag {
    /// Cursor position when the drag started (logical window px).
    start_cursor: Vec2,
    /// The target's top-left when the drag started (logical px).
    start_target: Vec2,
}

/// The conventional grip glyph — six dots — already wired to drag `target`.
/// Spawns it unparented; add it to your header.
pub fn drag_grip(commands: &mut Commands, phosphor: &Handle<Font>, target: Entity) -> Entity {
    let grip = icon_text(commands, phosphor, "dots-six-vertical", text_muted(), 16.0);
    commands.entity(grip).insert((
        // `Interaction` is what the drag system watches — without it the glyph is
        // decorative and never reports a press.
        Interaction::default(),
        DragHandle::new(target),
        Name::new("drag-grip"),
    ));
    grip
}

/// Press a [`DragHandle`] → start a drag; hold → move its target; release → stop.
///
/// Registered by `WidgetsPlugin`; crate-visible because its query mentions the
/// private [`HandleDrag`] bookkeeping component.
pub(crate) fn drag_handle_move(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    idle: Query<(Entity, &Interaction, &DragHandle), Without<HandleDrag>>,
    active: Query<(Entity, &DragHandle, &HandleDrag)>,
    mut targets: Query<(&mut Node, &ComputedNode, &UiGlobalTransform)>,
) {
    if mouse.just_released(MouseButton::Left) {
        for (e, _, _) in &active {
            commands.entity(e).remove::<HandleDrag>();
        }
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };

    // Start: LMB went down with a handle under the cursor.
    if mouse.just_pressed(MouseButton::Left) {
        for (entity, interaction, handle) in &idle {
            if !matches!(interaction, Interaction::Pressed | Interaction::Hovered) {
                continue;
            }
            let Ok((_, cn, ugt)) = targets.get(handle.target) else {
                continue;
            };
            // Resolve where the target actually IS rather than trusting
            // `Node.left`: while it's still anchored by right/bottom, `left` is
            // `Val::Auto` and would read as zero, teleporting the card.
            let isf = cn.inverse_scale_factor();
            commands.entity(entity).insert(HandleDrag {
                start_cursor: cursor,
                start_target: (ugt.translation - cn.size() * 0.5) * isf,
            });
        }
    }

    // Continue: every held handle moves its target.
    for (_, handle, drag) in &active {
        let Ok((mut node, cn, _)) = targets.get_mut(handle.target) else {
            continue;
        };
        let size = cn.size() * cn.inverse_scale_factor();
        let want = drag.start_target + (cursor - drag.start_cursor);
        let min = Vec2::new(handle.margin - size.x, handle.margin - size.y);
        let max = Vec2::new(window.width() - handle.margin, window.height() - handle.margin);
        // `min` can exceed `max` for a window narrower than the margin; clamp
        // would panic on that, so order the bounds first.
        let pos = want.max(min.min(max)).min(max.max(min));

        node.left = Val::Px(pos.x);
        node.top = Val::Px(pos.y);
        node.position_type = PositionType::Absolute;
        // Hand anchoring over to left/top, or the two pairs fight each other.
        node.right = Val::Auto;
        node.bottom = Val::Auto;
    }
}
