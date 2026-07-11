//! Reflection-based scene IR, ported from `bevy_scene 0.18`'s `dynamic_scene.rs`
//! to Bevy 0.19. The RON `serialize`/`from_scene`/`Asset` bits are intentionally
//! dropped — the on-disk format lives in [`crate::bsn`] instead.

use crate::{DynamicSceneBuilder, SceneSpawnError};
use bevy::ecs::component::ComponentCloneBehavior;
use bevy::ecs::relationship::RelationshipHookMode;
use bevy::ecs::{
    entity::{Entity, EntityHashMap, SceneEntityMapper},
    reflect::{AppTypeRegistry, ReflectComponent},
    world::World,
};
use bevy::reflect::PartialReflect;

/// A collection of serializable resources and dynamic entities.
///
/// Each dynamic entity carries its own run-time-defined set of reflected
/// components. Build one with [`DynamicSceneBuilder`], serialize with
/// [`crate::bsn`], and instantiate with [`DynamicScene::write_to_world`].
#[derive(Default)]
pub struct DynamicScene {
    /// Resources stored in the dynamic scene.
    pub resources: Vec<Box<dyn PartialReflect>>,
    /// Entities contained in the dynamic scene.
    pub entities: Vec<DynamicEntity>,
}

/// A reflection-powered serializable representation of an entity and its components.
pub struct DynamicEntity {
    /// The identifier of the entity, unique within a scene (and the world it may
    /// have been generated from). Components that reference this entity must
    /// consistently use this identifier.
    pub entity: Entity,
    /// Boxed reflected components belonging to this entity.
    pub components: Vec<Box<dyn PartialReflect>>,
}

impl DynamicScene {
    /// Create a new dynamic scene from a given world (every entity + resources).
    pub fn from_world(world: &World) -> Self {
        DynamicSceneBuilder::from_world(world)
            .extract_entities(
                // Sidestep default query filters by walking archetypes directly,
                // so custom-disabled entities are still captured.
                world
                    .archetypes()
                    .iter()
                    .flat_map(bevy::ecs::archetype::Archetype::entities)
                    .map(bevy::ecs::archetype::ArchetypeEntity::id),
            )
            .extract_resources()
            .build()
    }

    /// Ensure every scene entity has a corresponding world entity in the map,
    /// spawning empties for the unseen ones. Cheap even for large scenes (no
    /// components are touched), so an incremental spawner can run it up front
    /// in one frame — entity references in later component batches then always
    /// remap to a live target regardless of spawn order.
    pub fn allocate_entities(&self, world: &mut World, entity_map: &mut EntityHashMap<Entity>) {
        for scene_entity in &self.entities {
            entity_map
                .entry(scene_entity.entity)
                .or_insert_with(|| world.spawn_empty().id());
        }
    }

    /// Apply the components of the single scene entity at `index` onto its
    /// mapped world entity. [`allocate_entities`](Self::allocate_entities) must
    /// have run first so every entity reference has a mapping. This is the unit
    /// of work for incremental (streamed) scene spawning — callers spread the
    /// per-entity reflection cost over multiple frames.
    pub fn write_entity_to_world(
        &self,
        index: usize,
        world: &mut World,
        entity_map: &mut EntityHashMap<Entity>,
        type_registry: &AppTypeRegistry,
    ) -> Result<(), SceneSpawnError> {
        let type_registry = type_registry.read();
        let scene_entity = &self.entities[index];
        let entity = *entity_map
            .get(&scene_entity.entity)
            .expect("allocate_entities should have spawned an empty entity");

        for component in &scene_entity.components {
            let type_info = component.get_represented_type_info().ok_or_else(|| {
                SceneSpawnError::NoRepresentedType {
                    type_path: component.reflect_type_path().to_string(),
                }
            })?;
            let registration = type_registry.get(type_info.type_id()).ok_or_else(|| {
                SceneSpawnError::UnregisteredButReflectedType {
                    type_path: type_info.type_path().to_string(),
                }
            })?;
            let reflect_component =
                registration.data::<ReflectComponent>().ok_or_else(|| {
                    SceneSpawnError::UnregisteredComponent {
                        type_path: type_info.type_path().to_string(),
                    }
                })?;

            {
                let component_id = reflect_component.register_component(world);
                // Registered immediately above, so the info exists.
                let component_info = world
                    .components()
                    .get_info(component_id)
                    .expect("component just registered");
                if matches!(
                    *component_info.clone_behavior(),
                    ComponentCloneBehavior::Ignore
                ) {
                    continue;
                }
            }

            SceneEntityMapper::world_scope(entity_map, world, |world, mapper| {
                reflect_component.apply_or_insert_mapped(
                    &mut world.entity_mut(entity),
                    component.as_partial_reflect(),
                    &type_registry,
                    mapper,
                    RelationshipHookMode::Skip,
                );
            });
        }
        Ok(())
    }

    /// Write the resources, the dynamic entities, and their components into the
    /// given world, remapping entity references through `entity_map`.
    pub fn write_to_world_with(
        &self,
        world: &mut World,
        entity_map: &mut EntityHashMap<Entity>,
        type_registry: &AppTypeRegistry,
    ) -> Result<(), SceneSpawnError> {
        self.allocate_entities(world, entity_map);
        for index in 0..self.entities.len() {
            self.write_entity_to_world(index, world, entity_map, type_registry)?;
        }

        // Resources are intentionally not written: the interim BSN format does
        // not serialize them (renzora's scene save denies all resources, and
        // Bevy 0.19's resource-storage rework removed the extraction path). See
        // `DynamicSceneBuilder::extract_resources`. `self.resources` is empty.

        Ok(())
    }

    /// Write into `world` using the world's own `AppTypeRegistry`.
    pub fn write_to_world(
        &self,
        world: &mut World,
        entity_map: &mut EntityHashMap<Entity>,
    ) -> Result<(), SceneSpawnError> {
        let registry = world.resource::<AppTypeRegistry>().clone();
        self.write_to_world_with(world, entity_map, &registry)
    }
}
