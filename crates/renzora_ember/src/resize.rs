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
//!   (GH #81).
//! - Panels that hit-test with `Interaction` see it too, because Bevy 0.19's
//!   `FocusPolicy` defaults to `Pass`: the focus walk marks *every* node under
//!   the cursor `Pressed`, not just the front one, until something blocks.
//!
//! [`ResizeHandle`] fixes the second by construction — it forces
//! `FocusPolicy::Block`, so a handle stops the walk and nothing behind it is
//! pressed — and the first via [`ResizeBusy`], which geometric panels check on
//! left-press. Mark every node whose press starts a resize with it.
//!
//! [`ResizeBusy`] is the sibling of
//! [`ScrollbarBusy`](crate::widgets::ScrollbarBusy), which covers the other
//! widget drawn over every panel's content: the scrollbar inside it.

use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, UiSystems};

/// Marks a node whose press starts a resize gesture — a dock split's divider
/// handle, a leaf's tab-bar filler (which doubles as one), a floating dock
/// window's edge/corner zone, the shell window's own edge grips.
///
/// Marking a node is the whole job: it forces `FocusPolicy::Block` so the press
/// stops here instead of also firing on the content the handle overhangs, and it
/// drives [`ResizeBusy`] for the panels that resolve presses geometrically.
///
/// The block is applied by an insertion hook rather than
/// `#[require(FocusPolicy::Block)]` because that requirement would lose:
/// `Node` requires the same component with the opposite value, requirements
/// resolve in bundle order, and a handle is always spawned `Node`-first. An
/// explicit insert always wins, whatever the order (pinned by
/// `tests/resize_handle_blocks.rs`).
#[derive(Component, Default)]
#[component(on_add = block_focus)]
pub struct ResizeHandle;

/// Force `FocusPolicy::Block` onto a node the moment it becomes a handle.
fn block_focus(mut world: DeferredWorld, ctx: HookContext) {
    world.commands().entity(ctx.entity).insert(FocusPolicy::Block);
}

/// True from the moment the left button goes down on a [`ResizeHandle`] until it
/// is released — the whole gesture, not just the press frame.
///
/// Panels that act on a press in their empty content (clear the selection, start
/// a rubber-band sweep, begin a drag) and resolve that press geometrically must
/// skip it while this is set; see the module docs for why `cursor_over` can't
/// tell a handle press apart from a press on their own content.
///
/// Refreshed in `PreUpdate` after the pointer state settles, so it is already
/// correct whatever order the `Update` systems reading it run in.
#[derive(Resource, Default)]
pub struct ResizeBusy(pub bool);

impl ResizeBusy {
    /// Whether a resize gesture is currently in flight.
    pub fn active(&self) -> bool {
        self.0
    }
}

/// Refresh [`ResizeBusy`].
///
/// The flag **latches** for as long as the button is held rather than tracking
/// `Interaction` frame by frame: the focus system only reports `Pressed` on the
/// frame the button goes down, and a resize drag deliberately continues after
/// the cursor has left the handle (dock dividers latch the same way, and an OS
/// window resize takes the pointer away entirely). Tracking `Pressed` live would
/// clear the flag one frame into every drag.
fn resize_busy(
    mouse: Res<ButtonInput<MouseButton>>,
    handles: Query<&Interaction, With<ResizeHandle>>,
    mut busy: ResMut<ResizeBusy>,
) {
    if !mouse.pressed(MouseButton::Left) {
        if busy.0 {
            busy.0 = false;
        }
        return;
    }
    if !busy.0 && handles.iter().any(|i| *i == Interaction::Pressed) {
        busy.0 = true;
    }
}

/// Registers [`ResizeBusy`] and its refresh. Called by `EmberPlugin`.
pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<ResizeBusy>();
    // After `UiSystems::Focus` writes `Interaction`, and after ember's own
    // correction clears it on nodes an overlay/modal covers — a handle under an
    // open menu must not latch the flag.
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
    /// Step 2 is the one that matters — the focus system reports `Pressed` only
    /// on the press frame, so a non-latching flag would go false right there and
    /// the panel underneath would start its own drag mid-resize.
    #[test]
    fn busy_latches_for_the_whole_hold() {
        let mut app = App::new();
        app.init_resource::<ResizeBusy>();
        app.init_resource::<ButtonInput<MouseButton>>();
        app.add_systems(Update, resize_busy);
        let handle = app.world_mut().spawn((ResizeHandle, Interaction::None)).id();

        let set = |app: &mut App, down: bool, i: Interaction| {
            let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
            if down {
                mouse.press(MouseButton::Left);
            } else {
                mouse.release(MouseButton::Left);
            }
            *app.world_mut().get_mut::<Interaction>(handle).unwrap() = i;
            app.update();
            app.world().resource::<ResizeBusy>().active()
        };

        assert!(!set(&mut app, false, Interaction::Hovered), "idle hover isn't a gesture");
        assert!(set(&mut app, true, Interaction::Pressed), "press on the handle latches");
        assert!(set(&mut app, true, Interaction::None), "the drag left the handle");
        assert!(!set(&mut app, false, Interaction::None), "release clears it");
    }

    /// A press that missed every handle must not latch, or panels would ignore
    /// perfectly ordinary presses for as long as the button is held.
    #[test]
    fn a_press_elsewhere_never_latches() {
        let mut app = App::new();
        app.init_resource::<ResizeBusy>();
        app.init_resource::<ButtonInput<MouseButton>>();
        app.add_systems(Update, resize_busy);
        app.world_mut().spawn((ResizeHandle, Interaction::None));

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(!app.world().resource::<ResizeBusy>().active());
    }
}
