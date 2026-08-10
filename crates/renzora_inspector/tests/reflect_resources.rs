//! The Resources panel reads and writes resources through the *component*
//! reflection path, which only works because Bevy 0.19 made `Resource:
//! Component` and stores a resource's value on a hidden entity. That is an
//! implementation detail of Bevy's, not a documented contract, so these tests
//! pin the three shapes the panel depends on: a named struct, a newtype, and a
//! resource that is itself the value.

use bevy::prelude::*;
use renzora::{FieldType, FieldValue};
use renzora_inspector::reflect_source::{read_field, resource_fields, world_resources, write_field};

#[derive(Resource, Reflect, Default)]
#[reflect(Resource)]
struct Settings {
    intensity: f32,
    enabled: bool,
}

/// The newtype shape — reflection addresses its member by index, not by name.
#[derive(Resource, Reflect, Default)]
#[reflect(Resource)]
struct Score(i32);

/// A resource with no members at all: the value *is* the resource.
#[derive(Resource, Reflect, Default, PartialEq, Debug)]
#[reflect(Resource)]
enum Mode {
    #[default]
    Explore,
    Combat,
}

fn app() -> App {
    let mut app = App::new();
    app.init_resource::<AppTypeRegistry>();
    app.register_type::<Settings>();
    app.register_type::<Score>();
    app.register_type::<Mode>();
    app.insert_resource(Settings {
        intensity: 2.5,
        enabled: true,
    });
    app.insert_resource(Score(7));
    app.insert_resource(Mode::Combat);
    app
}

/// Find a listed resource by its short type name.
fn entry(world: &World, name: &str) -> (Entity, &'static str) {
    let found = world_resources(world)
        .reflected
        .into_iter()
        .find(|e| e.type_path.ends_with(name))
        .unwrap_or_else(|| panic!("{name} was not listed"));
    (found.entity, found.type_path)
}

#[test]
fn named_struct_fields_read_and_write() {
    let mut app = app();
    let (entity, type_path) = entry(app.world(), "Settings");

    let fields = resource_fields(app.world(), entity, type_path);
    let paths: Vec<&str> = fields.iter().map(|f| f.path).collect();
    assert_eq!(paths, vec!["intensity", "enabled"]);
    assert!(matches!(fields[0].value, FieldValue::Float(v) if v == 2.5));
    assert!(matches!(fields[1].field_type, FieldType::Bool));

    write_field(
        app.world_mut(),
        entity,
        type_path,
        "intensity",
        FieldValue::Float(9.0),
    );
    assert_eq!(app.world().resource::<Settings>().intensity, 9.0);
    assert!(matches!(
        read_field(app.world(), entity, type_path, "intensity", false),
        Some(FieldValue::Float(v)) if v == 9.0
    ));
}

#[test]
fn newtype_is_addressed_by_index() {
    let mut app = app();
    let (entity, type_path) = entry(app.world(), "Score");

    let fields = resource_fields(app.world(), entity, type_path);
    assert_eq!(fields.len(), 1);
    // The path is the member index, and the label falls back to "Value" because
    // an unnamed single member has no name to show.
    assert_eq!(fields[0].path, "0");
    assert_eq!(fields[0].label, "Value");

    write_field(app.world_mut(), entity, type_path, "0", FieldValue::Float(42.0));
    assert_eq!(app.world().resource::<Score>().0, 42);
}

#[test]
fn a_bare_enum_resource_gets_one_row_at_the_root() {
    let mut app = app();
    let (entity, type_path) = entry(app.world(), "Mode");

    let fields = resource_fields(app.world(), entity, type_path);
    assert_eq!(fields.len(), 1);
    // The empty path resolves to the resource itself.
    assert_eq!(fields[0].path, "");
    assert!(matches!(&fields[0].value, FieldValue::Enum(v) if v == "Combat"));

    write_field(
        app.world_mut(),
        entity,
        type_path,
        "",
        FieldValue::Enum("Explore".to_string()),
    );
    assert_eq!(*app.world().resource::<Mode>(), Mode::Explore);
}

/// An unreflected resource is counted, not listed — there is no name to list it
/// under without Bevy's `debug` feature, which this workspace does not enable.
#[test]
fn unreflected_resources_are_counted_not_listed() {
    #[derive(Resource)]
    struct Opaque(#[allow(dead_code)] u8);

    let mut app = app();
    let before = world_resources(app.world()).unreflected;
    app.insert_resource(Opaque(1));

    let listed = world_resources(app.world());
    assert_eq!(listed.unreflected, before + 1);
    assert!(!listed
        .reflected
        .iter()
        .any(|e| e.type_path.contains("Opaque")));
}

/// Asking for a type path the registry doesn't hold yields no rows rather than
/// panicking — the detail pane can outlive a resource being removed.
#[test]
fn an_unknown_type_path_yields_no_fields() {
    let app = app();
    let (entity, _) = entry(app.world(), "Settings");
    assert!(resource_fields(app.world(), entity, "nonexistent::Gone").is_empty());
}
