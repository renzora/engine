//! `ResizeHandle` must actually end up blocking picking.
//!
//! A handle may be spawned with a pass-through `Pickable` by code that builds
//! it from a generic row or zone, and requirements resolve in bundle order, so
//! the block is an insertion hook rather than a requirement. If the block were
//! ever lost, presses on a dock divider would resume firing on the panel
//! content it overhangs (the half of GH #81 that `ResizeBusy` doesn't cover)
//! and nothing else would look wrong, so pin the outcome here.

use bevy::picking::Pickable;
use bevy::prelude::*;
use renzora_ember::interaction::{BLOCK, PASS_THROUGH};
use renzora_ember::resize::ResizeHandle;
use renzora_ember::UiInteraction;

#[test]
fn resize_handle_blocks_regardless_of_bundle_order() {
    let mut world = World::new();
    // Both orders, and spawned pass-through: the hook has to win either way.
    let node_first = world
        .spawn((Node::default(), UiInteraction::default(), PASS_THROUGH, ResizeHandle))
        .id();
    let handle_first = world
        .spawn((ResizeHandle, Node::default(), UiInteraction::default(), PASS_THROUGH))
        .id();
    // The hook queues the insert; apply it like a schedule's sync point would.
    world.flush();

    let blocks = |e| world.get::<Pickable>(e).map(|p| p.should_block_lower);
    assert_eq!(blocks(node_first), Some(BLOCK.should_block_lower));
    assert_eq!(blocks(handle_first), Some(BLOCK.should_block_lower));
}
