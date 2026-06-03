//! Helper for writing native inspector drawers
//! (`renzora_editor::NativeInspectorDrawer`).
//!
//! A drawer is `fn(&mut World, Entity) -> Entity` — it builds an arbitrary
//! bevy_ui subtree (using ember widgets + `bind_2way`) and returns its root,
//! which the inspector parents under the component's section. [`inspector_body`]
//! wraps the `CommandQueue` + fonts boilerplate so a drawer reads:
//!
//! ```ignore
//! fn my_inspector(world: &mut World, entity: Entity) -> Entity {
//!     renzora_ember::inspector::inspector_body(world, |commands, fonts| {
//!         let col = commands.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).id();
//!         let dv = renzora_ember::widgets::drag_value(commands, &fonts.ui, "", (210,210,220), 0.0, 0.1);
//!         renzora_ember::reactive::bind_2way(commands, dv,
//!             move |w| w.get::<MyComp>(entity).map(|c| c.value).unwrap_or(0.0),
//!             move |w, v: &f32| { if let Some(mut c) = w.get_mut::<MyComp>(entity) { c.value = *v; } });
//!         commands.entity(col).add_child(dv);
//!         col
//!     })
//! }
//! ```

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use crate::font::EmberFonts;

/// Run a drawer body builder with a fresh `Commands` (backed by a local
/// `CommandQueue` that's applied before returning) + the live [`EmberFonts`].
/// Returns the root entity your `build` produced. Returns a bare node if fonts
/// aren't ready yet (shouldn't happen — the inspector gates on fonts).
pub fn inspector_body(
    world: &mut World,
    build: impl FnOnce(&mut Commands, &EmberFonts) -> Entity,
) -> Entity {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return world.spawn(Node::default()).id();
    };
    let mut queue = CommandQueue::default();
    let root = {
        let mut commands = Commands::new(&mut queue, world);
        build(&mut commands, &fonts)
    };
    queue.apply(world);
    root
}
