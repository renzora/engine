//! # Dependency Chains Example — Multi-Entity Propagation
//!
//! Demonstrates cross-entity wiring where changes cascade through a chain:
//!
//! - **Equipment** (Necromancer's Crown) has `IncreasedMinionLife` = 0.25
//! - **Owner** (Necromancer) equips the crown, gaining access to the stat
//! - **Minion** (Skeleton) has base `Life`, scaled by the owner's bonus
//!
//! ```text
//! Crown ──@Equipment──> Necromancer ──@Owner──> Skeleton
//! ```
//!
//! Changing the crown's stat auto-propagates to the minion. Swapping the
//! owner rewires everything.
//!
//! Run with: `cargo run --example dependency_chains`

use bevy::prelude::*;
use bevy_gauge::prelude::*;

// ---------------------------------------------------------------------------
// Resource
// ---------------------------------------------------------------------------

#[derive(Resource)]
struct Entities {
    crown: Entity,
    necromancer: Entity,
    warlock: Entity,
    skeleton: Entity,
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
// Spawn
// ---------------------------------------------------------------------------

fn spawn(mut commands: Commands) {
    let crown = commands
        .spawn((
            Name::new("Necromancer's Crown"),
            attributes! {
                "IncreasedMinionLife" => 0.25,
            },
        ))
        .id();

    let necromancer = commands
        .spawn((
            Name::new("Necromancer"),
            attributes! {
                "Intelligence" => 30.0,
            },
        ))
        .id();

    let warlock = commands
        .spawn((
            Name::new("Warlock"),
            attributes! {
                "Intelligence" => 50.0,
            },
        ))
        .id();

    let skeleton = commands
        .spawn((
            Name::new("Skeleton"),
            attributes! {
                "Life.base" => 100.0,
            },
        ))
        .id();

    commands.insert_resource(Entities {
        crown,
        necromancer,
        warlock,
        skeleton,
    });

    println!("--- Entities spawned ---\n");
}

// ---------------------------------------------------------------------------
// Demo
// ---------------------------------------------------------------------------

fn demo(handles: Res<Entities>, mut attributes: AttributesMut) {
    let crown = handles.crown;
    let necro = handles.necromancer;
    let warlock = handles.warlock;
    let skeleton = handles.skeleton;

    // Structure Life as: Life = base * (1 + increased)
    // This avoids self-referential expressions (a modifier on Life reading Life).
    attributes
        .complex_attribute(
            skeleton,
            "Life",
            &[("base", ReduceFn::Sum), ("increased", ReduceFn::Sum)],
            "base * (1 + increased)",
        )
        .expect("valid expression");

    // --- Initial state ---
    println!("=== Initial state ===\n");
    print_skeleton_life("Skeleton", skeleton, &mut attributes);

    // --- Step 1: Necromancer equips the crown ---
    println!("=== Necromancer equips Crown ===\n");
    attributes.register_source(necro, "Equipment", crown);
    attributes
        .add_expr_modifier(necro, "IncreasedMinionLife", "IncreasedMinionLife@Equipment")
        .expect("valid expression");

    let necro_bonus = attributes.evaluate(necro, "IncreasedMinionLife");
    println!("  Necromancer's IncreasedMinionLife: {necro_bonus:.2} (from Crown's 0.25)");

    // --- Step 2: Wire skeleton to owner ---
    println!("\n=== Skeleton linked to Necromancer as Owner ===\n");
    attributes.register_source(skeleton, "Owner", necro);
    attributes
        .add_expr_modifier(skeleton, "Life.increased", "IncreasedMinionLife@Owner")
        .expect("valid expression");

    let skel_life = attributes.evaluate(skeleton, "Life");
    println!("  Skeleton Life: {skel_life:.1}");
    println!("  Expected: 100 * (1 + 0.25) = 125.0");

    // --- Step 3: Upgrade the crown ---
    println!("\n=== Upgrading Crown: IncreasedMinionLife 0.25 → 0.50 ===\n");
    attributes.set_base(crown, "IncreasedMinionLife", 0.50);

    let necro_bonus = attributes.evaluate(necro, "IncreasedMinionLife");
    let skel_life = attributes.evaluate(skeleton, "Life");
    println!("  Necromancer's IncreasedMinionLife: {necro_bonus:.2}");
    println!("  Skeleton Life: {skel_life:.1}");
    println!("  Expected: 100 * (1 + 0.50) = 150.0");

    // --- Step 4: Swap owner to Warlock ---
    println!("\n=== Warlock takes the Crown from Necromancer ===\n");

    // Unequip from necro: zero out the stat and disconnect the source
    attributes.set_base(necro, "IncreasedMinionLife", 0.0);
    attributes.unregister_source(necro, "Equipment");

    // Equip on warlock
    attributes.register_source(warlock, "Equipment", crown);
    attributes
        .add_expr_modifier(warlock, "IncreasedMinionLife", "IncreasedMinionLife@Equipment")
        .expect("valid expression");

    // Re-point the skeleton's owner
    attributes.register_source(skeleton, "Owner", warlock);

    let warlock_bonus = attributes.evaluate(warlock, "IncreasedMinionLife");
    let skel_life = attributes.evaluate(skeleton, "Life");
    println!("  Warlock's IncreasedMinionLife: {warlock_bonus:.2}");
    println!("  Skeleton Life: {skel_life:.1}");
    println!("  Expected: 100 * (1 + 0.50) = 150.0 (same crown, different owner)");

    // --- Step 5: Upgrade crown again ---
    println!("\n=== Upgrading Crown again: 0.50 → 1.00 ===\n");
    attributes.set_base(crown, "IncreasedMinionLife", 1.00);

    let warlock_bonus = attributes.evaluate(warlock, "IncreasedMinionLife");
    let skel_life = attributes.evaluate(skeleton, "Life");
    println!("  Warlock's IncreasedMinionLife: {warlock_bonus:.2}");
    println!("  Skeleton Life: {skel_life:.1}");
    println!("  Expected: 100 * (1 + 1.00) = 200.0");

    // --- Step 6: Unequip crown entirely ---
    println!("\n=== Warlock unequips Crown ===\n");
    attributes.set_base(warlock, "IncreasedMinionLife", 0.0);
    attributes.unregister_source(warlock, "Equipment");

    let warlock_bonus = attributes.evaluate(warlock, "IncreasedMinionLife");
    let skel_life = attributes.evaluate(skeleton, "Life");
    println!("  Warlock's IncreasedMinionLife: {warlock_bonus:.2}");
    println!("  Skeleton Life: {skel_life:.1}");
    println!("  Expected: 100 * (1 + 0.00) = 100.0 (no crown equipped)");

    println!("\n--- Done ---");
    std::process::exit(0);
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn print_skeleton_life(label: &str, entity: Entity, attributes: &mut AttributesMut) {
    let life = attributes.evaluate(entity, "Life");
    println!("  {label} Life: {life:.1}");
    println!();
}
