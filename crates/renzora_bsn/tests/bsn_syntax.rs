//! The runtime BSN parser and spawner.
//!
//! Run with `renzora test`; `cargo test` cannot link the full workspace natively
//! on Windows (CLAUDE.md §2).

use bevy::ecs::component::{ComponentDescriptor, ComponentId, StorageType};
use bevy::prelude::*;
use renzora_bsn::bsn_tree::{parse, parse_list, spawn, spawn_list};
use renzora_bsn::{RawComponentRegistry, RawField, RawTypeInfo, RawTypeTable};
use std::alloc::Layout;
use std::sync::Arc;

#[derive(Component, Reflect, Default, Debug, PartialEq)]
#[reflect(Component)]
struct Health {
    current: f32,
    max: f32,
}

#[derive(Component, Reflect, Default, Debug, PartialEq)]
#[reflect(Component)]
struct Marker;

/// Stands in for a C-ABI plugin component: registered by layout, so it has no
/// `TypeId` and reflection cannot see it at all.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
struct Spinner {
    speed: f32,
    turns: i32,
    enabled: bool,
}

const SPINNER_PATH: &str = "myplugin::Spinner";

fn test_world() -> World {
    let mut world = World::new();
    world.init_resource::<AppTypeRegistry>();
    {
        let registry = world.resource::<AppTypeRegistry>().clone();
        let mut w = registry.write();
        w.register::<Health>();
        w.register::<Marker>();
        w.register::<Name>();
    }

    // SAFETY: no destructor declared; `Spinner` is plain data.
    let desc = unsafe {
        ComponentDescriptor::new_with_layout(
            SPINNER_PATH.to_string(),
            StorageType::Table,
            Layout::new::<Spinner>().pad_to_align(),
            None,
            true,
            bevy::ecs::component::ComponentCloneBehavior::Default,
            None,
        )
    };
    let id = world.register_component_with_descriptor(desc);

    let default = Spinner {
        speed: 1.0,
        turns: 0,
        enabled: true,
    };
    let default_value = unsafe {
        std::slice::from_raw_parts(
            (&default as *const Spinner).cast::<u8>(),
            size_of::<Spinner>(),
        )
    }
    .to_vec();

    let mut table = RawTypeTable::default();
    table.by_component.insert(id, SPINNER_PATH.into());
    table
        .by_short
        .insert("Spinner".into(), Some(SPINNER_PATH.into()));
    table.by_path.insert(
        SPINNER_PATH.into(),
        RawTypeInfo {
            component_id: id,
            type_path: SPINNER_PATH.into(),
            size: size_of::<Spinner>(),
            is_resource: false,
            transient: false,
            default_value,
            fields: vec![
                RawField { name: "speed".into(), kind: "f32".into(), offset: 0 },
                RawField { name: "turns".into(), kind: "i32".into(), offset: 4 },
                RawField { name: "enabled".into(), kind: "bool".into(), offset: 8 },
            ],
        },
    );
    world.insert_resource(RawComponentRegistry(Arc::new(table)));
    world
}

fn spinner_id(world: &World) -> ComponentId {
    world
        .resource::<RawComponentRegistry>()
        .0
        .by_path
        .get(SPINNER_PATH)
        .unwrap()
        .component_id
}

#[test]
fn components_are_space_separated_not_comma_separated() {
    // That is BSN, and it is what lets a component body use commas freely — a
    // comma-separated component list could not tell `Health { current: 1, max: 2 }`
    // apart from two components.
    let tree = parse(r#"Health { current: 30.0, max: 100.0 } Marker"#).expect("parse");
    assert_eq!(tree.components.len(), 2, "{:?}", tree.components);
    assert_eq!(tree.components[0].0, "Health");
    // Source spacing is preserved — RON does not care, and normalising it would
    // mean re-tokenising a body that reflection is about to parse anyway.
    assert_eq!(
        tree.components[0].1.replace(' ', ""),
        "(current:30.0,max:100.0)"
    );
    // A bare marker gets a unit body, which is what reflection wants.
    assert_eq!(tree.components[1], ("Marker".into(), "()".into()));
}

#[test]
fn children_nest_without_a_closure_per_level() {
    // The whole reason this beats an imperative builder: four levels of nesting
    // is four levels of brackets, not four nested closures.
    let tree = parse(
        r#"
        #Root Marker Children [
            ( Marker Children [
                ( Marker Children [
                    ( #Leaf Marker )
                ] )
            ] )
        ]
        "#,
    )
    .expect("parse");

    assert_eq!(tree.key.as_deref(), Some("Root"));
    let depth3 = &tree.children[0].children[0].children[0];
    assert_eq!(depth3.key.as_deref(), Some("Leaf"));
}

#[test]
fn a_reflected_component_lands_as_the_real_type() {
    let mut world = test_world();
    let tree = parse(r#"#Player Health { current: 30.0, max: 100.0 }"#).expect("parse");
    let e = spawn(&mut world, &tree, None);

    assert_eq!(
        world.entity(e).get::<Health>(),
        Some(&Health { current: 30.0, max: 100.0 }),
        "the body was not reflected into a real component"
    );
    // `#Key` becomes a Name, so the entity is findable and the scene file reads.
    assert_eq!(world.entity(e).get::<Name>().map(|n| n.as_str()), Some("Player"));
}

#[test]
fn a_plugin_component_lands_via_its_schema() {
    // The point of keying on names: this type has no `TypeId` and reflection
    // cannot see it, but the host knows its fields from the same schema that
    // serializes it into scene files.
    let mut world = test_world();
    let id = spinner_id(&world);
    let tree = parse(r#"Spinner { speed: 2.5, turns: 7, enabled: false }"#).expect("parse");
    let e = spawn(&mut world, &tree, None);

    let ptr = world.entity(e).get_by_id(id).expect("component missing");
    let got = unsafe { ptr.deref::<Spinner>().to_owned() };
    assert_eq!(
        got,
        Spinner { speed: 2.5, turns: 7, enabled: false }
    );
}

#[test]
fn an_omitted_field_keeps_the_plugin_default() {
    // Zeroing would give `speed: 0.0` — present, valid, and doing nothing, which
    // reads as a broken plugin rather than as an omitted field.
    let mut world = test_world();
    let id = spinner_id(&world);
    let tree = parse(r#"Spinner { turns: 3 }"#).expect("parse");
    let e = spawn(&mut world, &tree, None);

    let ptr = world.entity(e).get_by_id(id).unwrap();
    let got = unsafe { ptr.deref::<Spinner>().to_owned() };
    assert_eq!(got.speed, 1.0, "an omitted field did not keep its default");
    assert_eq!(got.turns, 3);
    assert!(got.enabled);
}

#[test]
fn engine_and_plugin_components_mix_on_one_entity() {
    // A plugin author should not have to know which side of the ABI boundary a
    // component lives on.
    let mut world = test_world();
    let id = spinner_id(&world);
    let tree = parse(r#"Health { current: 5.0, max: 5.0 } Spinner { speed: 9.0 } Marker"#)
        .expect("parse");
    let e = spawn(&mut world, &tree, None);

    assert_eq!(world.entity(e).get::<Health>().unwrap().max, 5.0);
    assert!(world.entity(e).get::<Marker>().is_some());
    let ptr = world.entity(e).get_by_id(id).unwrap();
    assert_eq!(unsafe { ptr.deref::<Spinner>().speed }, 9.0);
}

#[test]
fn a_bool_field_writes_one_byte_not_four() {
    // `enabled` sits at offset 8 with `turns` at 4. A four-byte write at 8 would
    // run past a 12-byte component; a wide write at a lower offset would take
    // its neighbour with it.
    let mut world = test_world();
    let id = spinner_id(&world);
    let tree = parse(r#"Spinner { turns: 12345, enabled: false }"#).expect("parse");
    let e = spawn(&mut world, &tree, None);

    let ptr = world.entity(e).get_by_id(id).unwrap();
    let got = unsafe { ptr.deref::<Spinner>().to_owned() };
    assert_eq!(got.turns, 12345, "writing `enabled` clobbered `turns`");
    assert!(!got.enabled);
}

#[test]
fn a_tree_becomes_a_real_hierarchy() {
    let mut world = test_world();
    let tree = parse(r#"#Root Marker Children [ ( #A Marker ), ( #B Marker ) ]"#).expect("parse");
    let root = spawn(&mut world, &tree, None);

    let children = world.entity(root).get::<Children>().expect("no children");
    assert_eq!(children.len(), 2);
    let names: Vec<String> = children
        .iter()
        .map(|c| world.entity(c).get::<Name>().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["A".to_string(), "B".to_string()]);
}

#[test]
fn a_list_spawns_siblings() {
    let mut world = test_world();
    let trees = parse_list(r#"( #One Marker ), ( #Two Marker ), ( #Three Marker )"#).expect("parse");
    let ids = spawn_list(&mut world, &trees, None);
    assert_eq!(ids.len(), 3);
    for (e, want) in ids.iter().zip(["One", "Two", "Three"]) {
        assert_eq!(world.entity(*e).get::<Name>().unwrap().as_str(), want);
    }
}

#[test]
fn an_unknown_component_costs_itself_not_the_tree() {
    // A plugin built against a newer engine, or a typo. Losing the layout an
    // unknown component was part of would be far worse than losing the component.
    let mut world = test_world();
    let tree = parse(r#"Health { current: 1.0, max: 2.0 } Hologram { spin: 3.0 } Marker"#)
        .expect("parse");
    let e = spawn(&mut world, &tree, None);

    assert!(world.entity(e).get::<Health>().is_some());
    assert!(world.entity(e).get::<Marker>().is_some());
}

/// Real `bevy_ui` types, because a component body that does not reflect is only
/// a logged warning — the entity spawns, the component is absent, and the UI is
/// merely laid out wrong. Asserting the value landed is the only thing that
/// catches it.
#[test]
fn a_bevy_ui_node_reflects_its_real_field_shapes() {
    use bevy::ui::{FlexDirection, Node, UiRect, Val};

    let mut world = test_world();
    {
        let registry = world.resource::<AppTypeRegistry>().clone();
        let mut w = registry.write();
        w.register::<Node>();
    }

    // `Val` IS an enum, so `Px(6.0)` is a variant. `UiRect` is NOT — it is a
    // struct of four `Val`s, and `padding: All(Px(4.0))` names a variant that
    // does not exist. Both spellings are here so the difference is pinned.
    let tree = parse(
        r#"Node {
            flex_direction: Column,
            row_gap: Px(6.0),
            padding: { left: Px(4.0), right: Px(4.0), top: Px(4.0), bottom: Px(4.0) },
        }"#,
    )
    .expect("parse");
    let e = spawn(&mut world, &tree, None);

    let node = world.entity(e).get::<Node>().expect("Node did not reflect");
    assert_eq!(node.flex_direction, FlexDirection::Column);
    assert_eq!(node.row_gap, Val::Px(6.0));
    assert_eq!(
        node.padding,
        UiRect::all(Val::Px(4.0)),
        "padding did not reflect — a wrong body is a warning, not an error, so \
         this is the only place it shows up"
    );
}

#[test]
fn a_string_body_survives_braces_and_commas() {
    let tree = parse(r#"Name("Hello, { world }")"#).expect("parse");
    assert_eq!(
        tree.components[0].1, r#"("Hello, { world }")"#,
        "brace translation ran inside a string literal"
    );
}

#[test]
fn an_unterminated_entity_is_an_error_not_a_panic() {
    let err = parse(r#"Health { current: 1.0 "#).unwrap_err();
    assert!(err.message.contains("unterminated"), "{err}");
}
