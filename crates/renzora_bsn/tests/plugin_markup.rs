//! What a plugin's generated markup actually parses to.
//!
//! `parse` is pure — no `World`, no type registry — so the shape a plugin emits
//! can be pinned here rather than inferred from editor warnings. Written after
//! `ai_chat` spent three rounds emitting markup that logged "could not read
//! `12.0`" and "no component called `Button`", with no way from the outside to
//! see which half was wrong.
//!
//! Already learned the hard way and recorded here so nobody repeats it: a
//! parenthesised group is only valid INSIDE a `Children [ .. ]` list, never as
//! a whole tree. `parse("( A B )")` is `expected a component name` at offset 0.

use renzora_bsn::bsn_tree::{parse, BsnTree};

fn first_child(src: &str) -> BsnTree {
    let tree = parse(src).unwrap_or_else(|e| panic!("parse failed: {e:?}\nsource:\n{src}"));
    tree.children
        .into_iter()
        .next()
        .expect("the parent parsed but had no children")
}

fn dump(label: &str, tree: &BsnTree) -> Vec<String> {
    println!("--- {label} ---");
    for (n, b) in &tree.components {
        println!("  {n} => {b:?}");
    }
    tree.components.iter().map(|(n, _)| n.clone()).collect()
}

/// Two components in one group must come out as two, with the second one's body
/// intact. If a body arrives with its braces still attached, every field inside
/// it fails to deserialize — which is exactly what "`TextFont.font_size` could
/// not read `12.0`" looks like from the editor.
#[test]
fn a_group_splits_into_separate_components() {
    let child = first_child(concat!(
        "Node\n",
        "Children [\n",
        "    ( Text(\"Ready\") TextFont { font_size: 12.0 } ),\n",
        "]\n",
    ));
    let names = dump("Text + TextFont", &child);
    assert!(names.iter().any(|x| x == "Text"), "Text lost: {names:?}");
    assert!(
        names.iter().any(|x| x == "TextFont"),
        "TextFont lost: {names:?}"
    );
}

/// A field-less marker sitting between components that do have bodies.
///
/// `Button` and `Interaction` are the case that matters: a panel needs them on
/// ONE entity alongside `Node` and `PanelActionId`, because dispatch reads the
/// interaction and the action id together and `#[require(..)]` is not applied
/// by a reflected spawn.
#[test]
fn a_bare_marker_between_bodied_components_survives() {
    let child = first_child(concat!(
        "Node\n",
        "Children [\n",
        "    ( Node { flex_direction: Row } Button Interaction PanelActionId { action: 4 } ),\n",
        "]\n",
    ));
    let names = dump("Node + Button + Interaction + PanelActionId", &child);
    for want in ["Node", "Button", "Interaction", "PanelActionId"] {
        assert!(names.iter().any(|x| x == want), "`{want}` lost: {names:?}");
    }
}

/// The same group with a trailing `Children` list, which is how a real button
/// carries its label.
#[test]
fn a_marker_survives_alongside_a_children_list() {
    let child = first_child(concat!(
        "Node\n",
        "Children [\n",
        "    ( Node Button Interaction PanelActionId { action: 4 } Children [ Text(\"Browse\") ] ),\n",
        "]\n",
    ));
    let names = dump("group with trailing Children", &child);
    assert!(
        names.iter().any(|x| x == "Button"),
        "Button lost once a Children list followed it: {names:?}"
    );
    assert_eq!(
        child.children.len(),
        1,
        "the trailing Children list did not become a child"
    );
}

/// The half the parse test cannot reach: a body that parses cleanly still has
/// to *deserialize* per field.
///
/// A local component stands in for `TextFont` so this needs no bevy_text — the
/// question is whether an `f32` field written as `12.0` survives
/// `insert_component`, and that is the same code path whatever the type.
#[test]
fn an_f32_field_deserializes_from_a_plain_float() {
    use bevy::prelude::*;

    #[derive(Component, Reflect, Default, Debug)]
    #[reflect(Component, Default)]
    struct Sized2 {
        font_size: f32,
    }

    let mut world = World::new();
    let registry = AppTypeRegistry::default();
    registry.write().register::<Sized2>();
    registry.write().register::<f32>();
    world.insert_resource(registry);

    let tree = parse(concat!(
        "Node\n",
        "Children [\n",
        "    ( Sized2 { font_size: 12.0 } ),\n",
        "]\n",
    ))
    .expect("parse failed");
    let child = &tree.children[0];
    println!("body = {:?}", child.components[0].1);

    let e = world.spawn_empty().id();
    renzora_bsn::bsn_tree::spawn_into(&mut world, child, e);

    let got = world.entity(e).get::<Sized2>();
    println!("landed = {got:?}");
    assert_eq!(
        got.map(|s| s.font_size),
        Some(12.0),
        "an f32 field written as `12.0` did not survive insert_component"
    );
}
