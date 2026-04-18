use bevy::prelude::*;

use crate::attributes::Attributes;
use crate::derived::{AttributeRegistration, AttributeDerivedSet, WriteBackSet};
use crate::graph::DependencyGraph;
use crate::modifier_set::apply_initial_attributes;
use crate::attribute_id::Interner;
use crate::tags::{TagResolver, TagRegistration};

/// The main plugin.
///
/// Initializes the global [`Interner`], adds the [`DependencyGraph`] and
/// [`TagResolver`] resources, and sets up:
/// - Observer: clean up dependency edges when entities with `Attributes` are despawned.
/// - Observer: apply `AttributeInitializer` modifier sets when they are added to entities.
/// - System sets: `WriteBackSet` → `AttributeDerivedSet` in both `PreUpdate`
///   and `PostUpdate`. The `PreUpdate` pass flushes pending component-side
///   writes so that `Update` systems see fresh attributes and components.
///   The `PostUpdate` pass syncs any attribute changes made during `Update`
///   back to derived components.
/// - Auto-registration: iterates all [`AttributeRegistration`] entries
///   submitted via `inventory` (from `attribute_component!`, `register_derived!`,
///   or `register_write_back!`).
pub struct AttributesPlugin;

impl Plugin for AttributesPlugin {
    fn build(&self, app: &mut App) {
        Interner::new().set_global();

        let mut tag_resolver = TagResolver::new();
        for reg in inventory::iter::<TagRegistration> {
            (reg.register_fn)(&mut tag_resolver);
        }

        app.init_resource::<DependencyGraph>()
            .insert_resource(tag_resolver);

        app.add_observer(on_attributes_removed)
            .add_observer(apply_initial_attributes)
            .configure_sets(
                PreUpdate,
                (WriteBackSet, AttributeDerivedSet).chain(),
            )
            .configure_sets(
                PostUpdate,
                (WriteBackSet, AttributeDerivedSet).chain(),
            );

        for reg in inventory::iter::<AttributeRegistration> {
            (reg.register_fn)(app);
        }
    }
}

/// Observer that fires when an entity with `Attributes` is removed/despawned.
/// Cleans up all dependency edges in the global graph.
fn on_attributes_removed(
    trigger: On<Remove, Attributes>,
    mut graph: ResMut<DependencyGraph>,
) {
    let entity = trigger.entity;
    graph.remove_entity(entity);
}
