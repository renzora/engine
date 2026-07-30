//! Round-trip of components the engine has no Rust type for.
//!
//! These stand in for C-ABI plugin components: registered by layout, so
//! `ComponentDescriptor::new_with_layout` gives them `type_id: None` and the
//! reflected extraction path cannot see them at all. Before the raw channel
//! existed they were silently dropped on save — the scene wrote, the file
//! looked fine, and the components were simply gone on load.
//!
//! Run with `renzora test`; `cargo test` cannot link the full workspace natively
//! on Windows (CLAUDE.md §2).

use bevy::ecs::component::{ComponentDescriptor, ComponentId, StorageType};
use bevy::prelude::*;
use bevy::ptr::OwningPtr;
use renzora_bsn::bsn::{BsnSerializer, SceneSerializer};
use renzora_bsn::*;
use std::alloc::Layout;
use std::sync::Arc;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
struct Spinner {
    speed: f32,
    turns: i32,
}

const SPINNER_PATH: &str = "spinner::Spinner";

/// Register a component the way the plugin host does: by layout, with no
/// `TypeId` and no reflection.
fn register_by_layout(world: &mut World, name: &str, layout: Layout) -> ComponentId {
    // SAFETY: no destructor is declared, and the tests only store plain data.
    let desc = unsafe {
        ComponentDescriptor::new_with_layout(
            name.to_string(),
            StorageType::Table,
            layout.pad_to_align(),
            None,
            true,
            bevy::ecs::component::ComponentCloneBehavior::Default,
            None,
        )
    };
    world.register_component_with_descriptor(desc)
}

fn spinner_fields() -> Vec<RawField> {
    vec![
        RawField { name: "speed".into(), kind: "f32".into(), offset: 0 },
        RawField { name: "turns".into(), kind: "i32".into(), offset: 4 },
    ]
}

fn registry_with(entries: Vec<RawTypeInfo>) -> RawComponentRegistry {
    let mut table = RawTypeTable::default();
    for info in entries {
        table.by_component.insert(info.component_id, info.type_path.clone());
        let short = info.type_path.rsplit("::").next().unwrap().to_string();
        table
            .by_short
            .entry(short)
            .and_modify(|e| *e = None)
            .or_insert_with(|| Some(info.type_path.clone()));
        table.by_path.insert(info.type_path.clone(), info);
    }
    RawComponentRegistry(Arc::new(table))
}

fn insert_raw<T>(world: &mut World, entity: Entity, id: ComponentId, value: T) {
    // SAFETY: `id` was registered with `T`'s layout.
    unsafe {
        let mut bytes =
            std::slice::from_raw_parts((&value as *const T).cast::<u8>(), size_of::<T>()).to_vec();
        let ptr = OwningPtr::new(std::ptr::NonNull::new_unchecked(bytes.as_mut_ptr().cast()));
        world.entity_mut(entity).insert_by_id(id, ptr);
    }
}

fn read_raw<T: Copy>(world: &World, entity: Entity, id: ComponentId) -> Option<T> {
    let ptr = world.entity(entity).get_by_id(id).ok()?;
    // SAFETY: registered with `T`'s layout.
    Some(unsafe { ptr.deref::<T>().to_owned() })
}

/// A world with everything the scene IR needs, and one layout-registered type.
fn world_with_spinner() -> (World, ComponentId) {
    let mut world = World::new();
    world.init_resource::<AppTypeRegistry>();
    world.resource_mut::<AppTypeRegistry>().write().register::<Name>();
    let id = register_by_layout(&mut world, SPINNER_PATH, Layout::new::<Spinner>());
    world.insert_resource(registry_with(vec![RawTypeInfo {
        component_id: id,
        type_path: SPINNER_PATH.into(),
        size: size_of::<Spinner>(),
        is_resource: false,
        transient: false,
        default_value: vec![0u8; size_of::<Spinner>()],
        fields: spinner_fields(),
    }]));
    (world, id)
}

#[test]
fn a_layout_registered_component_survives_save_and_load() {
    let (mut world, id) = world_with_spinner();
    let e = world.spawn(Name::new("Cube")).id();
    insert_raw(&mut world, e, id, Spinner { speed: 2.5, turns: 7 });

    let scene = DynamicSceneBuilder::from_world(&world)
        .extract_entity(e)
        .build();

    assert_eq!(scene.entities.len(), 1);
    assert_eq!(
        scene.entities[0].raw.len(),
        1,
        "the component was dropped during extraction — this is the bug the raw \
         channel exists to fix"
    );
    assert_eq!(
        scene.raw_schemas.len(),
        1,
        "no schema was emitted, so a later load could not migrate the bytes"
    );

    // Through the text format and back, which is where a scene actually lives.
    let registry = world.resource::<AppTypeRegistry>().clone();
    let text = {
        let r = registry.read();
        BsnSerializer.serialize(&scene, &r).expect("serialize")
    };
    assert!(text.contains("raw_schema"), "schema missing from the file:\n{text}");
    assert!(text.contains(SPINNER_PATH), "type path missing from the file:\n{text}");

    let parsed = {
        let r = registry.read();
        BsnSerializer.deserialize(&text, &r).expect("deserialize")
    };

    // A fresh world, as a real load is.
    let (mut dest, dest_id) = world_with_spinner();
    let mut map = bevy::ecs::entity::EntityHashMap::default();
    parsed.write_to_world(&mut dest, &mut map).expect("write");

    let new_entity = *map.get(&scene.entities[0].entity).expect("entity mapped");
    let got = read_raw::<Spinner>(&dest, new_entity, dest_id)
        .expect("component did not arrive in the destination world");
    assert_eq!(got, Spinner { speed: 2.5, turns: 7 });
}

#[test]
fn an_entity_holding_only_a_raw_component_is_not_dropped() {
    let (mut world, id) = world_with_spinner();
    let e = world.spawn_empty().id();
    insert_raw(&mut world, e, id, Spinner { speed: 1.0, turns: 1 });

    let scene = DynamicSceneBuilder::from_world(&world)
        .extract_entity(e)
        .remove_empty_entities()
        .build();

    assert_eq!(
        scene.entities.len(),
        1,
        "`remove_empty_entities` judged emptiness by the reflected list alone and \
         threw away an entity whose only content was a plugin component"
    );
}

#[test]
fn a_type_with_no_plugin_loaded_round_trips_instead_of_being_lost() {
    let (mut world, id) = world_with_spinner();
    let e = world.spawn(Name::new("Cube")).id();
    insert_raw(&mut world, e, id, Spinner { speed: 3.0, turns: 2 });
    let scene = DynamicSceneBuilder::from_world(&world).extract_entity(e).build();

    let registry = world.resource::<AppTypeRegistry>().clone();
    let text = {
        let r = registry.read();
        BsnSerializer.serialize(&scene, &r).expect("serialize")
    };

    // A world where the plugin never loaded: the type is unknown.
    let mut dest = World::new();
    dest.init_resource::<AppTypeRegistry>();
    dest.resource_mut::<AppTypeRegistry>().write().register::<Name>();
    let parsed = {
        let r = registry.read();
        BsnSerializer.deserialize(&text, &r).expect("deserialize")
    };
    let mut map = bevy::ecs::entity::EntityHashMap::default();
    parsed.write_to_world(&mut dest, &mut map).expect("write");

    let new_entity = *map.get(&scene.entities[0].entity).unwrap();
    let held = dest
        .entity(new_entity)
        .get::<OrphanedRawComponents>()
        .expect("the blob was discarded — re-saving would have destroyed it");
    assert_eq!(held.0.len(), 1);
    assert_eq!(held.0[0].type_path, SPINNER_PATH);

    // And it comes back out on the next save, so opening a scene with a plugin
    // disabled and saving does not strip everything that plugin owned.
    let again = DynamicSceneBuilder::from_world(&dest)
        .extract_entity(new_entity)
        .build();
    assert_eq!(again.entities[0].raw.len(), 1);
    assert_eq!(again.entities[0].raw[0].bytes, scene.entities[0].raw[0].bytes);
}

#[test]
fn a_field_added_to_the_plugin_migrates_by_name() {
    // Saved with two fields…
    let old = RawSchema {
        type_path: SPINNER_PATH.into(),
        size: 8,
        fields: spinner_fields(),
    };
    let saved = Spinner { speed: 4.5, turns: 9 };
    let bytes =
        unsafe { std::slice::from_raw_parts((&saved as *const Spinner).cast::<u8>(), 8) }.to_vec();

    // …and reloaded into a plugin that inserted a field *before* them, moving
    // both offsets. Matching by offset would read `speed` out of the new field.
    #[repr(C)]
    #[derive(Clone, Copy, PartialEq, Debug)]
    struct SpinnerV2 {
        enabled: i32,
        speed: f32,
        turns: i32,
    }
    let default = SpinnerV2 { enabled: 1, speed: 0.0, turns: 0 };
    let info = RawTypeInfo {
        component_id: ComponentId::new(0),
        type_path: SPINNER_PATH.into(),
        size: size_of::<SpinnerV2>(),
        is_resource: false,
        transient: false,
        default_value: unsafe {
            std::slice::from_raw_parts(
                (&default as *const SpinnerV2).cast::<u8>(),
                size_of::<SpinnerV2>(),
            )
        }
        .to_vec(),
        fields: vec![
            RawField { name: "enabled".into(), kind: "i32".into(), offset: 0 },
            RawField { name: "speed".into(), kind: "f32".into(), offset: 4 },
            RawField { name: "turns".into(), kind: "i32".into(), offset: 8 },
        ],
    };

    let migrated = migrate(&info, &old, &bytes).expect("layouts differ, so a copy was due");
    assert_eq!(migrated.len(), size_of::<SpinnerV2>());
    let got = unsafe { *migrated.as_ptr().cast::<SpinnerV2>() };
    assert_eq!(got.speed, 4.5, "speed did not follow its name to the new offset");
    assert_eq!(got.turns, 9);
    assert_eq!(
        got.enabled, 1,
        "a field the old scene never had should arrive at the plugin's default, \
         not at zero"
    );
}

#[test]
fn an_unchanged_layout_skips_the_copy() {
    let schema = RawSchema {
        type_path: SPINNER_PATH.into(),
        size: 8,
        fields: spinner_fields(),
    };
    let info = RawTypeInfo {
        component_id: ComponentId::new(0),
        type_path: SPINNER_PATH.into(),
        size: 8,
        is_resource: false,
        transient: false,
        default_value: vec![0u8; 8],
        fields: spinner_fields(),
    };
    assert!(
        migrate(&info, &schema, &[0u8; 8]).is_none(),
        "an identical layout should be recognised and left alone"
    );
}

#[test]
fn two_types_sharing_a_short_name_resolve_to_neither() {
    let mut table = RawTypeTable::default();
    for path in ["a::Settings", "b::Settings"] {
        let info = RawTypeInfo {
            component_id: ComponentId::new(0),
            type_path: path.into(),
            size: 4,
            is_resource: false,
            transient: false,
            default_value: vec![0u8; 4],
            fields: Vec::new(),
        };
        table
            .by_short
            .entry("Settings".into())
            .and_modify(|e| *e = None)
            .or_insert_with(|| Some(path.to_string()));
        table.by_path.insert(path.into(), info);
    }

    // The full path still works…
    assert!(table.resolve("a::Settings").is_some());
    // …but a short name two types claim resolves to nothing, rather than to a
    // coin flip that would load one plugin's bytes into the other's component.
    assert!(table.resolve("gone::Settings").is_none());
}
