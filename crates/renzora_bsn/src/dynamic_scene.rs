//! Reflection-based scene IR, ported from `bevy_scene 0.18`'s `dynamic_scene.rs`
//! to Bevy 0.19. The RON `serialize`/`from_scene`/`Asset` bits are intentionally
//! dropped — the on-disk format lives in [`crate::bsn`] instead.

use crate::{
    DynamicSceneBuilder, OrphanedRawComponents, OrphanedRawScene, RawComponentRegistry,
    SceneSpawnError,
};
use bevy::ecs::component::{ComponentCloneBehavior, ComponentId};
use bevy::ecs::relationship::RelationshipHookMode;
use bevy::ecs::resource::IsResource;
use bevy::ecs::{
    entity::{Entity, EntityHashMap, SceneEntityMapper},
    reflect::{AppTypeRegistry, ReflectComponent},
    world::World,
};
use bevy::log::warn;
use bevy::ptr::OwningPtr;
use bevy::reflect::PartialReflect;
use core::ptr::NonNull;

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
    /// Plugin-owned resources, as raw bytes. See [`RawComponent`].
    pub raw_resources: Vec<RawComponent>,
    /// One schema per distinct raw type used anywhere in this scene, written
    /// once per file rather than once per instance.
    pub raw_schemas: Vec<RawSchema>,
}

/// A reflection-powered serializable representation of an entity and its components.
pub struct DynamicEntity {
    /// The identifier of the entity, unique within a scene (and the world it may
    /// have been generated from). Components that reference this entity must
    /// consistently use this identifier.
    pub entity: Entity,
    /// Boxed reflected components belonging to this entity.
    pub components: Vec<Box<dyn PartialReflect>>,
    /// Components the reflection path cannot see. See [`RawComponent`].
    pub raw: Vec<RawComponent>,
}

/// A component stored as raw bytes and keyed by name rather than by `TypeId`.
///
/// The reflected path cannot carry these. A component registered by layout —
/// which is how every C-ABI plugin component arrives — gets
/// `ComponentDescriptor::new_with_layout`, and that constructor hard-codes
/// `type_id: None`. The whole extraction path keys on `TypeId`, so the component
/// is invisible to it and vanishes on save. There is no reflection workaround:
/// `TypeRegistration::of::<T>` is the only constructor, and it needs a Rust type
/// that by definition does not exist here.
///
/// So raw components travel in a parallel channel: bytes as the payload, and the
/// type path as the key. Bytes are lossless and safe because a plugin component
/// is plain data by host enforcement — `register_component` refuses a descriptor
/// carrying a destructor.
#[derive(Clone, Debug, PartialEq)]
pub struct RawComponent {
    pub type_path: String,
    pub bytes: Vec<u8>,
}

/// One field of a raw type, as recorded in the scene file.
///
/// `kind` is a `String` rather than the plugin ABI's `FieldKind` so this crate
/// never has to name a plugin type, and so a kind added to a later ABI does not
/// change this format.
#[derive(Clone, Debug, PartialEq)]
pub struct RawField {
    pub name: String,
    pub kind: String,
    pub offset: usize,
}

/// The layout a raw type had **when the scene was written**.
///
/// Stored because bytes alone are not enough to survive the plugin changing.
/// Add, remove or reorder a field and every saved instance silently means
/// something else; with the schema, the loader can match fields by name and
/// migrate what still lines up.
#[derive(Clone, Debug, PartialEq)]
pub struct RawSchema {
    pub type_path: String,
    pub size: usize,
    pub fields: Vec<RawField>,
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

        self.write_raw_to_entity(scene_entity, entity, world);
        Ok(())
    }

    /// Insert this entity's raw components, migrating any whose layout moved.
    ///
    /// A type with no plugin loaded this session is parked in
    /// [`OrphanedRawComponents`] rather than dropped, so loading a scene in a
    /// build without the plugin and saving it again does not quietly delete
    /// every component that plugin owned.
    fn write_raw_to_entity(&self, scene_entity: &DynamicEntity, entity: Entity, world: &mut World) {
        if scene_entity.raw.is_empty() {
            return;
        }
        let registry = world.get_resource::<RawComponentRegistry>().cloned();
        let mut orphans: Vec<RawComponent> = Vec::new();

        for raw in &scene_entity.raw {
            let info = registry.as_ref().and_then(|r| r.0.resolve(&raw.type_path));
            let Some(info) = info else {
                orphans.push(raw.clone());
                continue;
            };
            if info.transient {
                continue;
            }

            let mut bytes = raw.bytes.clone();
            if let Some(migrated) = self
                .raw_schemas
                .iter()
                .find(|s| s.type_path == raw.type_path)
                .and_then(|from| crate::migrate(info, from, &bytes))
            {
                bytes = migrated;
            }
            // A scene written before the schema channel existed, or one whose
            // schema went missing. Refuse rather than pad: a short buffer would
            // be read to the full layout size, which is a heap over-read into
            // component storage.
            if bytes.len() != info.size {
                warn!(
                    "scene has {} bytes for `{}` but it is {} bytes here, and no schema to \
                     migrate through — skipping",
                    bytes.len(),
                    raw.type_path,
                    info.size
                );
                continue;
            }
            insert_raw_bytes(world, entity, info.component_id, &bytes);
        }

        if !orphans.is_empty() {
            world.entity_mut(entity).insert(OrphanedRawComponents(orphans));
        }
    }

    /// Insert plugin-owned resources from this scene.
    ///
    /// Separate from the entity pass because resources are global: a caller
    /// spawning a prefab or restoring a deleted subtree must not have world
    /// globals come along with it.
    pub fn write_raw_resources(&self, world: &mut World) {
        let registry = world.get_resource::<RawComponentRegistry>().cloned();
        let mut orphans: Vec<RawComponent> = Vec::new();

        for raw in &self.raw_resources {
            let info = registry.as_ref().and_then(|r| r.0.resolve(&raw.type_path));
            let Some(info) = info else {
                orphans.push(raw.clone());
                continue;
            };
            if info.transient || !info.is_resource {
                continue;
            }
            let mut bytes = raw.bytes.clone();
            if let Some(migrated) = self
                .raw_schemas
                .iter()
                .find(|s| s.type_path == raw.type_path)
                .and_then(|from| crate::migrate(info, from, &bytes))
            {
                bytes = migrated;
            }
            if bytes.len() != info.size {
                warn!(
                    "scene has {} bytes for resource `{}` but it is {} bytes here — skipping",
                    bytes.len(),
                    raw.type_path,
                    info.size
                );
                continue;
            }
            insert_raw_resource(world, info.component_id, &bytes);
        }

        if !orphans.is_empty() || !self.raw_schemas.is_empty() {
            let mut held = world.get_resource_or_insert_with(OrphanedRawScene::default);
            held.resources = orphans;
            // Keep every schema, not only the orphaned ones: re-saving needs a
            // layout for each blob it carries forward, and the live registry
            // cannot describe a type whose plugin is absent.
            held.schemas = self.raw_schemas.clone();
        }
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

/// Copy `bytes` into `entity`'s storage for `id`.
///
/// The allocation handed to `insert_by_id` must be one Bevy can read `len`
/// bytes from, and the pointer must address the **bytes** — an `OwningPtr` built
/// from a boxed slice points at the fat pointer instead, which is how plugin
/// components once arrived holding `{heap addr, len}`.
pub(crate) fn insert_raw_component(
    world: &mut World,
    entity: Entity,
    id: ComponentId,
    bytes: &[u8],
) {
    insert_raw_bytes(world, entity, id, bytes);
}

fn insert_raw_bytes(world: &mut World, entity: Entity, id: ComponentId, bytes: &[u8]) {
    let mut owned = bytes.to_vec();
    // SAFETY: `owned` is exactly the registered layout size for `id` (checked by
    // the caller), and holds one initialised instance of that component.
    unsafe {
        let ptr = OwningPtr::new(NonNull::new_unchecked(owned.as_mut_ptr().cast()));
        if let Ok(mut e) = world.get_entity_mut(entity) {
            e.insert_by_id(id, ptr);
        }
    }
    // `owned` drops here on purpose. `insert_by_id` copies the value into column
    // storage rather than adopting the allocation, and a `Vec<u8>` has no
    // element destructors to run twice.
}

/// The resource equivalent. A resource is a component on a hidden entity, and
/// that entity must also carry `IsResource` — it is that marker's insert hook
/// which records the entity in the world's resource cache. Without it the value
/// really is in the world and `get_resource_by_id` still returns `None`.
fn insert_raw_resource(world: &mut World, id: ComponentId, bytes: &[u8]) {
    let entity = match world.resource_entities().get(id) {
        Some(e) => e,
        None => world.spawn(IsResource::new(id)).id(),
    };
    insert_raw_bytes(world, entity, id, bytes);
}
