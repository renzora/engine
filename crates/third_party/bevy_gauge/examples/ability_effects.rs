//! # Ability Effects Example — Instant Mutations with Roles
//!
//! Demonstrates the `instant!` system with the arrow/bow/attacker role pattern.
//!
//! - **`instant!`** macro with `-=` (subtract)
//! - **Role-based expressions**: `"Damage@arrow + Damage@bow + (Agility@attacker * 0.1)"`
//! - **`InstantExt::evaluate_instant`** for previewing damage before committing
//! - **`InstantExt::apply_evaluated_instant`** to commit previewed values
//! - Swapping role entities to show how results change
//!
//! Run with: `cargo run --example ability_effects`

use bevy::prelude::*;
use bevy_gauge::prelude::*;

// ---------------------------------------------------------------------------
// Resource
// ---------------------------------------------------------------------------

#[derive(Resource)]
struct Entities {
    archer: Entity,
    target: Entity,
    iron_arrow: Entity,
    fire_arrow: Entity,
    shortbow: Entity,
    longbow: Entity,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(AttributesPlugin)
        .add_systems(
            Startup,
            (
                spawn,
                demo.after(spawn),
            ),
        )
        .run();
}

// ---------------------------------------------------------------------------
// Spawn entities
// ---------------------------------------------------------------------------

fn spawn(mut commands: Commands) {
    let archer = commands
        .spawn((
            Name::new("Archer"),
            attributes! {
                "Agility" => 30.0,
            },
        ))
        .id();

    let target = commands
        .spawn((
            Name::new("Target Dummy"),
            attributes! {
                "Life"         => 200.0,
                "Life.current" => 200.0,
            },
        ))
        .id();

    let iron_arrow = commands
        .spawn((
            Name::new("Iron Arrow"),
            attributes! { "Damage" => 15.0 },
        ))
        .id();

    let fire_arrow = commands
        .spawn((
            Name::new("Fire Arrow"),
            attributes! { "Damage" => 25.0 },
        ))
        .id();

    let shortbow = commands
        .spawn((
            Name::new("Shortbow"),
            attributes! { "Damage" => 10.0 },
        ))
        .id();

    let longbow = commands
        .spawn((
            Name::new("Longbow"),
            attributes! { "Damage" => 20.0 },
        ))
        .id();

    commands.insert_resource(Entities {
        archer,
        target,
        iron_arrow,
        fire_arrow,
        shortbow,
        longbow,
    });

    println!("--- Entities spawned ---\n");
}

// ---------------------------------------------------------------------------
// Demo
// ---------------------------------------------------------------------------

fn demo(handles: Res<Entities>, mut attributes: AttributesMut) {
    let on_hit = instant! {
        "Life.current" -= "Damage@arrow + Damage@bow + (Agility@attacker * 0.1)",
    };

    // --- Shot 1: Iron Arrow + Shortbow ---
    println!("=== Shot 1: Iron Arrow + Shortbow ===\n");
    let roles: &[(&str, Entity)] = &[
        ("arrow", handles.iron_arrow),
        ("bow", handles.shortbow),
        ("attacker", handles.archer),
    ];

    let preview = attributes.evaluate_instant(&on_hit, roles, handles.target);
    for entry in &preview {
        println!("  Preview: {} {:?} {:.1}", entry.attribute, entry.op, entry.value);
    }
    println!("  Expected: 15 (arrow) + 10 (bow) + 30*0.1 (agility) = 28.0\n");

    attributes.apply_evaluated_instant(&preview, handles.target);
    let life = attributes.evaluate(handles.target, "Life.current");
    println!("  Target life after shot 1: {life:.1} / 200\n");

    // --- Shot 2: Fire Arrow + Longbow ---
    println!("=== Shot 2: Fire Arrow + Longbow ===\n");
    let roles: &[(&str, Entity)] = &[
        ("arrow", handles.fire_arrow),
        ("bow", handles.longbow),
        ("attacker", handles.archer),
    ];

    let preview = attributes.evaluate_instant(&on_hit, roles, handles.target);
    for entry in &preview {
        println!("  Preview: {} {:?} {:.1}", entry.attribute, entry.op, entry.value);
    }
    println!("  Expected: 25 (arrow) + 20 (bow) + 30*0.1 (agility) = 48.0\n");

    attributes.apply_evaluated_instant(&preview, handles.target);
    let life = attributes.evaluate(handles.target, "Life.current");
    println!("  Target life after shot 2: {life:.1} / 200\n");

    // --- Shot 3: Direct apply (no preview) ---
    println!("=== Shot 3: Iron Arrow + Longbow (direct apply) ===\n");
    let roles: &[(&str, Entity)] = &[
        ("arrow", handles.iron_arrow),
        ("bow", handles.longbow),
        ("attacker", handles.archer),
    ];

    attributes.apply_instant(&on_hit, roles, handles.target);
    let life = attributes.evaluate(handles.target, "Life.current");
    println!("  Expected damage: 15 + 20 + 3 = 38.0");
    println!("  Target life after shot 3: {life:.1} / 200\n");

    // --- Summary ---
    println!("=== Summary ===\n");
    println!("  Total damage dealt: {:.1}", 200.0 - life);
    println!("  Remaining life:     {life:.1}");

    println!("\n--- Done ---");
    std::process::exit(0);
}
