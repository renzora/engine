//! Resize handles and the "a resize gesture is in flight" flag panels consult.
//!
//! A resize handle has to be comfortably grabbable, so it is always **bigger
//! than the seam it sizes** and always sits *over* content it doesn't belong to:
//! a dock divider's line is 1px but its grab strip is 11px, so the strip
//! overhangs ~5px into the panes on either side; a window's edge zones are laid
//! over the perimeter of whatever is docked against it. That overlap is the
//! point — and it's also why a press on a handle keeps reaching things it
//! shouldn't:
//!
//! - Panels that decide "was this press mine?" **geometrically** — testing
//!   `RelativeCursorPosition::cursor_over` on their content node — see the press
//!   as landing in their own empty space, because that test is pure geometry and
//!   knows nothing about what's drawn on top. This is what made dragging the
//!   hierarchy panel's edge divider also sweep-select the rows the drag passed
//!   (GH #81), and what made dragging the global bottom panel upward arm the
//!   3D viewport's selection box — its grip straddles the panel's own top edge,
//!   so it hangs over the scene.
//! - Panels that hit-test with `UiInteraction` see it too if the handle lets
//!   the pointer through: a press reaches every node under the cursor down to
//!   the first one that blocks.
//!
//! [`ResizeHandle`] fixes the second by construction (it blocks picking, so
//! nothing behind a handle is hovered or pressed) and the first via
//! [`ResizeBusy`], which geometric panels check on left-press. Mark every node
//! whose press starts a resize with it.
//!
//! [`ResizeBusy`] is the sibling of
//! [`ScrollbarBusy`](crate::widgets::ScrollbarBusy), which covers the other
//! widget drawn over every panel's content: the scrollbar inside it.
//!
//! The flag itself is defined in the contract crate
//! ([`renzora::core::resize`]) and re-exported here, because consumers of it
//! reach past the UI crates entirely — the viewport's picking lives in
//! `renzora_gizmo`, which links no UI crate. This module keeps what only makes
//! sense next to bevy_ui: the handle marker and the refresh.

use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;
use bevy::ui::UiSystems;

use crate::UiInteraction;

pub use renzora::core::resize::ResizeBusy;

/// Marks a node whose press starts a resize gesture: a dock split's divider
/// handle, a leaf's tab-bar filler (which doubles as one), the global bottom
/// panel's grip band, a floating dock window's edge/corner zone, the shell
/// window's own edge grips.
///
/// Marking a node is the whole job: it makes the node block picking so the
/// press stops here instead of also firing on the content the handle
/// overhangs, and it drives [`ResizeBusy`] for the panels that resolve presses
/// geometrically.
///
/// The block is applied by an insertion hook, not a requirement, so it wins
/// over a pass-through `Pickable` the node was spawned with as well (pinned by
/// `tests/resize_handle_blocks.rs`).
#[derive(Component, Default)]
#[component(on_add = block_picking)]
pub struct ResizeHandle;

/// Make a node block picking the moment it becomes a handle.
fn block_picking(mut world: DeferredWorld, ctx: HookContext) {
    world.commands().entity(ctx.entity).insert(crate::interaction::BLOCK);
}

/// Refresh [`ResizeBusy`].
///
/// The flag **latches** for as long as the button is held rather than tracking
/// `UiInteraction` frame by frame: a resize drag deliberately continues after
/// the cursor has left the handle (dock dividers latch the same way, and an OS
/// window resize takes the pointer away entirely), and a handle's state can
/// drop to `None` the moment an overlay or a window move takes the pointer.
/// Tracking `Pressed` live would clear the flag partway through a drag.
fn resize_busy(
    mouse: Res<ButtonInput<MouseButton>>,
    handles: Query<&UiInteraction, With<ResizeHandle>>,
    mut busy: ResMut<ResizeBusy>,
) {
    if !mouse.pressed(MouseButton::Left) {
        if busy.0 {
            busy.0 = false;
        }
        return;
    }
    if !busy.0 && handles.iter().any(|i| *i == UiInteraction::Pressed) {
        busy.0 = true;
    }
}

/// Registers [`ResizeBusy`] and its refresh. Called by `EmberPlugin`.
pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<ResizeBusy>();
    // After `UiSystems::Focus`, where `UiInteraction` is derived, and after
    // ember's own correction clears it on nodes an overlay/modal covers: a
    // handle under an open menu must not latch the flag.
    app.add_systems(
        PreUpdate,
        resize_busy
            .after(UiSystems::Focus)
            .after(crate::correct_pointer_state),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The gesture the flag exists for: press the handle, drag off it, release.
    /// Step 2 is the one that matters: the handle's state drops once the drag
    /// leaves it, so a non-latching flag would go false right there and the
    /// panel underneath would start its own drag mid-resize.
    #[test]
    fn busy_latches_for_the_whole_hold() {
        let mut app = App::new();
        app.init_resource::<ResizeBusy>();
        app.init_resource::<ButtonInput<MouseButton>>();
        app.add_systems(Update, resize_busy);
        let handle = app.world_mut().spawn((ResizeHandle, UiInteraction::None)).id();

        let set = |app: &mut App, down: bool, i: UiInteraction| {
            let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
            if down {
                mouse.press(MouseButton::Left);
            } else {
                mouse.release(MouseButton::Left);
            }
            *app.world_mut().get_mut::<UiInteraction>(handle).unwrap() = i;
            app.update();
            app.world().resource::<ResizeBusy>().active()
        };

        assert!(!set(&mut app, false, UiInteraction::Hovered), "idle hover isn't a gesture");
        assert!(set(&mut app, true, UiInteraction::Pressed), "press on the handle latches");
        assert!(set(&mut app, true, UiInteraction::None), "the drag left the handle");
        assert!(!set(&mut app, false, UiInteraction::None), "release clears it");
    }

    /// A press that missed every handle must not latch, or panels would ignore
    /// perfectly ordinary presses for as long as the button is held.
    #[test]
    fn a_press_elsewhere_never_latches() {
        let mut app = App::new();
        app.init_resource::<ResizeBusy>();
        app.init_resource::<ButtonInput<MouseButton>>();
        app.add_systems(Update, resize_busy);
        app.world_mut().spawn((ResizeHandle, UiInteraction::None));

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(!app.world().resource::<ResizeBusy>().active());
    }
}
