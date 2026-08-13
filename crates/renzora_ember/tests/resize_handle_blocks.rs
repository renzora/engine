//! `ResizeHandle` must actually end up `FocusPolicy::Block`.
//!
//! `Node` requires that same component with the opposite value (`Pass` is Bevy
//! 0.19's default), and requirements resolve in bundle order — so a plain
//! `#[require(FocusPolicy::Block)]` on the marker loses on every handle, which
//! is spawned `Node`-first. Hence the insertion hook. If the block were ever
//! lost, presses on a dock divider would resume firing on the panel content it
//! overhangs — the half of GH #81 that `ResizeBusy` doesn't cover — and nothing
//! else would look wrong, so pin the outcome here.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use renzora_ember::resize::ResizeHandle;

#[test]
fn resize_handle_blocks_regardless_of_bundle_order() {
    let mut world = World::new();
    // Both orders: required components resolve depth-first, left-to-right, so
    // the winner could in principle depend on where `Node` sits in the tuple.
    let node_first = world
        .spawn((Node::default(), Interaction::default(), ResizeHandle))
        .id();
    let handle_first = world
        .spawn((ResizeHandle, Node::default(), Interaction::default()))
        .id();
    // The hook queues the insert; apply it like a schedule's sync point would.
    world.flush();

    assert_eq!(world.get::<FocusPolicy>(node_first), Some(&FocusPolicy::Block));
    assert_eq!(world.get::<FocusPolicy>(handle_first), Some(&FocusPolicy::Block));
}
