//! Integration tests for `evaluate_instant` / `apply_instant` with real ECS
//! entities. Ensures that cross-entity `@role` expressions resolve correctly.

use bevy::prelude::*;
use bevy_gauge::prelude::*;

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(AttributesPlugin);
    app
}

#[test]
fn evaluate_instant_resolves_cross_entity_roles() {
    let mut app = test_app();

    app.add_systems(Startup, |mut commands: Commands| {
        let archer = commands
            .spawn(attributes! { "Agility" => 30.0 })
            .id();
        let target = commands
            .spawn(attributes! {
                "Life" => 100.0,
                "Life.current" => 100.0,
            })
            .id();
        let arrow = commands
            .spawn(attributes! { "Damage" => 15.0 })
            .id();

        commands.insert_resource(TestEntities { archer, target, arrow });
    });

    app.add_systems(
        Update,
        |handles: Res<TestEntities>, mut attributes: AttributesMut| {
            let on_hit = instant! {
                "Life.current" -= "Damage@arrow + Agility@attacker * 0.1",
            };

            let roles: &[(&str, Entity)] = &[
                ("arrow", handles.arrow),
                ("attacker", handles.archer),
            ];

            let preview = attributes.evaluate_instant(&on_hit, roles, handles.target);
            assert_eq!(preview.len(), 1);
            let damage = preview[0].value;
            // 15 (arrow Damage) + 30 * 0.1 (attacker Agility) = 18.0
            assert!(
                (damage - 18.0).abs() < 0.01,
                "expected 18.0 damage, got {damage}"
            );

            attributes.apply_evaluated_instant(&preview, handles.target);
            let life = attributes.evaluate(handles.target, "Life.current");
            assert!(
                (life - 82.0).abs() < 0.01,
                "expected life 82.0, got {life}"
            );

            std::process::exit(0);
        },
    );

    app.run();
}

#[test]
fn apply_instant_resolves_cross_entity_roles() {
    let mut app = test_app();

    app.add_systems(Startup, |mut commands: Commands| {
        let attacker = commands
            .spawn(attributes! { "Strength" => 20.0 })
            .id();
        let target = commands
            .spawn(attributes! {
                "Life" => 100.0,
                "Life.current" => 100.0,
            })
            .id();

        commands.insert_resource(SimpleTest { attacker, target });
    });

    app.add_systems(
        Update,
        |handles: Res<SimpleTest>, mut attributes: AttributesMut| {
            let on_hit = instant! {
                "Life.current" -= "Strength@attacker",
            };

            let roles: &[(&str, Entity)] = &[("attacker", handles.attacker)];
            attributes.apply_instant(&on_hit, roles, handles.target);

            let life = attributes.evaluate(handles.target, "Life.current");
            // 100 - 20 = 80
            assert!(
                (life - 80.0).abs() < 0.01,
                "expected life 80.0, got {life}"
            );

            std::process::exit(0);
        },
    );

    app.run();
}

#[test]
fn evaluate_instant_with_literal_values() {
    let mut app = test_app();

    app.add_systems(Startup, |mut commands: Commands| {
        let target = commands
            .spawn(attributes! {
                "Life" => 100.0,
                "Life.current" => 100.0,
            })
            .id();
        commands.insert_resource(SingleTarget(target));
    });

    app.add_systems(
        Update,
        |handles: Res<SingleTarget>, mut attributes: AttributesMut| {
            let on_hit = instant! {
                "Life.current" -= 25.0,
            };

            let preview = attributes.evaluate_instant(&on_hit, &[], handles.0);
            assert_eq!(preview.len(), 1);
            assert!((preview[0].value - 25.0).abs() < 0.01);

            attributes.apply_evaluated_instant(&preview, handles.0);
            let life = attributes.evaluate(handles.0, "Life.current");
            assert!(
                (life - 75.0).abs() < 0.01,
                "expected life 75.0, got {life}"
            );

            std::process::exit(0);
        },
    );

    app.run();
}

// --- Helper resources ---

#[derive(Resource)]
struct TestEntities {
    archer: Entity,
    target: Entity,
    arrow: Entity,
}

#[derive(Resource)]
struct SimpleTest {
    attacker: Entity,
    target: Entity,
}

#[derive(Resource)]
struct SingleTarget(Entity);
