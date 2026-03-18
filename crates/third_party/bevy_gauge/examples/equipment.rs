//! # Equipment Example — Enum State Machine with ModifierSet
//!
//! Demonstrates equipping and unequipping items that grant stat modifiers,
//! using an enum-based state machine driven by Bevy events.
//!
//! - **`ModifierSet`** with `apply()` / `remove()` for clean attach/detach
//! - **`AttributeRequirements`** via `requires!` for equipment prerequisites
//! - **Expression modifiers** on equipment (bonus life from wielder intelligence)
//! - **Enum state machine** (`Unequipped` / `Equipped`) with event transitions
//!
//! Run with: `cargo run --example equipment`

use bevy::prelude::*;
use bevy_gauge::prelude::*;

// ---------------------------------------------------------------------------
// Equipment components
// ---------------------------------------------------------------------------

#[derive(Component, Default, Debug, Clone, PartialEq, Eq)]
enum EquipmentState {
    #[default]
    Unequipped,
    Equipped,
}

#[derive(Component, Debug, Clone)]
struct EquipmentModifiers(ModifierSet);

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Event)]
struct TryEquip {
    item: Entity,
    wielder: Entity,
}

#[derive(Event)]
struct TryUnequip {
    item: Entity,
}

// ---------------------------------------------------------------------------
// Resource for the demo
// ---------------------------------------------------------------------------

#[derive(Resource)]
struct Entities {
    wizard: Entity,
    warrior: Entity,
    staff: Entity,
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
                spawn_entities,
                run_demo.after(spawn_entities),
            ),
        )
        .add_observer(handle_equip)
        .add_observer(handle_unequip)
        .run();
}

// ---------------------------------------------------------------------------
// Spawn characters and equipment
// ---------------------------------------------------------------------------

fn spawn_entities(mut commands: Commands) {
    let wizard = commands
        .spawn((
            Name::new("Wizard"),
            attributes! {
                "Strength"     => 8.0,
                "Intelligence" => 20.0,
                "Life"         => 100.0,
            },
        ))
        .id();

    let warrior = commands
        .spawn((
            Name::new("Warrior"),
            attributes! {
                "Strength"     => 25.0,
                "Intelligence" => 6.0,
                "Life"         => 150.0,
            },
        ))
        .id();

    let mut staff_mods = ModifierSet::new();
    staff_mods.add("Intelligence", 5.0);
    staff_mods.add_expr("Life", "Intelligence / 5");

    let staff = commands
        .spawn((
            Name::new("Staff of Wisdom"),
            EquipmentState::default(),
            EquipmentModifiers(staff_mods),
            requires! { "Intelligence >= 15" },
        ))
        .id();

    commands.insert_resource(Entities {
        wizard,
        warrior,
        staff,
    });

    println!("--- Entities spawned ---\n");
}

// ---------------------------------------------------------------------------
// Demo: equip on wizard, print, unequip, try on warrior (fails)
// ---------------------------------------------------------------------------

fn run_demo(
    handles: Res<Entities>,
    mut attributes: AttributesMut,
    mut q_equipment: Query<(
        &Name,
        &mut EquipmentState,
        &EquipmentModifiers,
        &mut AttributeRequirements,
    )>,
    q_name: Query<&Name>,
) {
    println!("=== Before equipping ===\n");
    print_stats("Wizard", handles.wizard, &mut attributes);
    print_stats("Warrior", handles.warrior, &mut attributes);

    // --- Equip on Wizard ---
    println!("=== Wizard equips Staff of Wisdom ===\n");
    equip(
        handles.staff,
        handles.wizard,
        &mut attributes,
        &mut q_equipment,
        &q_name,
    );

    println!("\n=== After Wizard equips ===\n");
    print_stats("Wizard", handles.wizard, &mut attributes);

    // --- Unequip ---
    println!("=== Wizard unequips Staff of Wisdom ===\n");
    unequip(
        handles.staff,
        &mut attributes,
        &mut q_equipment,
        &q_name,
    );

    println!("\n=== After Wizard unequips ===\n");
    print_stats("Wizard", handles.wizard, &mut attributes);

    // --- Try on Warrior (should fail) ---
    println!("=== Warrior tries to equip Staff of Wisdom ===\n");
    equip(
        handles.staff,
        handles.warrior,
        &mut attributes,
        &mut q_equipment,
        &q_name,
    );

    println!("\n--- Done ---");
    std::process::exit(0);
}

// ---------------------------------------------------------------------------
// Equip / unequip logic
// ---------------------------------------------------------------------------

fn equip(
    item: Entity,
    wielder: Entity,
    attributes: &mut AttributesMut,
    q_equipment: &mut Query<(
        &Name,
        &mut EquipmentState,
        &EquipmentModifiers,
        &mut AttributeRequirements,
    )>,
    q_name: &Query<&Name>,
) {
    let wielder_name = q_name.get(wielder).map(|n| n.as_str()).unwrap_or("???");

    let Ok((item_name, mut state, mods, mut reqs)) = q_equipment.get_mut(item) else {
        println!("  Item has no equipment components!");
        return;
    };
    let item_name = item_name.clone();

    if *state == EquipmentState::Equipped {
        println!("  {item_name} is already equipped!");
        return;
    }

    let Some(wielder_attrs) = attributes.get_attributes(wielder) else {
        println!("  {wielder_name} has no Attributes!");
        return;
    };

    if !reqs.met(wielder_attrs) {
        println!("  BLOCKED: {wielder_name} does not meet requirements for {item_name}");
        println!("    Requirements: {:?}",
            reqs.0.iter().map(|r| r.source()).collect::<Vec<_>>());
        let int_val = wielder_attrs.value("Intelligence");
        println!("    {wielder_name}'s Intelligence: {int_val:.0}");
        return;
    }

    let wielder_int = wielder_attrs.value("Intelligence");
    println!("  Requirements met: {wielder_name} (Int {wielder_int:.0} >= 15)");

    let mods = mods.0.clone();
    *state = EquipmentState::Equipped;

    attributes.register_source(item, "Wielder", wielder);
    mods.apply(wielder, attributes);

    println!("  {item_name} equipped on {wielder_name} — modifiers applied");
}

fn unequip(
    item: Entity,
    attributes: &mut AttributesMut,
    q_equipment: &mut Query<(
        &Name,
        &mut EquipmentState,
        &EquipmentModifiers,
        &mut AttributeRequirements,
    )>,
    q_name: &Query<&Name>,
) {
    let Ok((item_name, mut state, mods, _)) = q_equipment.get_mut(item) else {
        println!("  Item has no equipment components!");
        return;
    };
    let item_name = item_name.clone();

    if *state == EquipmentState::Unequipped {
        println!("  {item_name} is not equipped!");
        return;
    }

    let Some(wielder) = attributes.resolve_source(item, "Wielder") else {
        println!("  {item_name} has no wielder!");
        return;
    };
    let wielder_name = q_name.get(wielder).map(|n| n.as_str()).unwrap_or("???");

    let mods = mods.0.clone();
    *state = EquipmentState::Unequipped;

    mods.remove(wielder, attributes);
    attributes.unregister_source(item, "Wielder");

    println!("  {item_name} unequipped from {wielder_name} — modifiers removed");
}

// ---------------------------------------------------------------------------
// Event-driven systems (for real game use)
// ---------------------------------------------------------------------------

fn handle_equip(
    try_equip: On<TryEquip>,
    mut attributes: AttributesMut,
    mut q_equipment: Query<(
        &Name,
        &mut EquipmentState,
        &EquipmentModifiers,
        &mut AttributeRequirements,
    )>,
    q_name: Query<&Name>,
) {
    equip(
        try_equip.item,
        try_equip.wielder,
        &mut attributes,
        &mut q_equipment,
        &q_name,
    );
}

fn handle_unequip(
    try_unequip: On<TryUnequip>,
    mut attributes: AttributesMut,
    mut q_equipment: Query<(
        &Name,
        &mut EquipmentState,
        &EquipmentModifiers,
        &mut AttributeRequirements,
    )>,
    q_name: Query<&Name>,
) {
    unequip(
        try_unequip.item,
        &mut attributes,
        &mut q_equipment,
        &q_name,
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn print_stats(label: &str, entity: Entity, attributes: &mut AttributesMut) {
    let str_val = attributes.evaluate(entity, "Strength");
    let int_val = attributes.evaluate(entity, "Intelligence");
    let life_val = attributes.evaluate(entity, "Life");

    println!("  {label}:");
    println!("    Strength:     {str_val:.0}");
    println!("    Intelligence: {int_val:.0}");
    println!("    Life:         {life_val:.1}");
    println!();
}
