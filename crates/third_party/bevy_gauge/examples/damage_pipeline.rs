//! # Damage Pipeline Example — Multi-Phase Attack Resolution
//!
//! Demonstrates a multi-phase damage pipeline:
//!
//! - **`define_tags!`** for damage element types (physical, fire, cold)
//! - **`AttributeQueries::evaluate_expr_with_roles`** for multi-entity expression
//!   evaluation (`@attacker`, `@weapon`)
//! - **Tagged evaluation** — weapon damage and defender resistance are tagged
//!   by element, queried via `evaluate_tagged`
//! - **`set_base`** for applying damage to `Life.current`
//! - Multi-phase resolution: hit → dodge → block → armor → resistance → damage
//!
//! Run with: `cargo run --example damage_pipeline`

use bevy::prelude::*;
use bevy_gauge::prelude::*;

// ---------------------------------------------------------------------------
// Tags
// ---------------------------------------------------------------------------

define_tags! {
    Tags,
    physical,
    element {
        fire,
        cold,
    },
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

#[derive(Component)]
struct DamageElement(TagMask);

#[derive(Component)]
struct AbilityHit(Expr);

// ---------------------------------------------------------------------------
// Resource
// ---------------------------------------------------------------------------

#[derive(Resource)]
struct Entities {
    warrior: Entity,
    mage: Entity,
    goblin: Entity,
    sword: Entity,
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
                register_tags,
                spawn_entities.after(register_tags),
                run_demo.after(spawn_entities),
            ),
        )
        .run();
}

fn register_tags(mut resolver: ResMut<TagResolver>) {
    Tags::register(&mut resolver);
}

// ---------------------------------------------------------------------------
// Spawn
// ---------------------------------------------------------------------------

fn spawn_entities(mut commands: Commands) {
    let warrior = commands
        .spawn((
            Name::new("Warrior"),
            attributes! {
                "Strength"       => 30.0,
                "AccuracyRating" => 80.0,
                "Life"           => 200.0,
                "Life.current"   => 200.0,
            },
        ))
        .id();

    let mage = commands
        .spawn((
            Name::new("Mage"),
            attributes! {
                "Intelligence"   => 40.0,
                "AccuracyRating" => 60.0,
                "Life"           => 120.0,
                "Life.current"   => 120.0,
            },
        ))
        .id();

    let goblin = commands
        .spawn((
            Name::new("Goblin"),
            attributes! {
                "Life"         => 80.0,
                "Life.current" => 80.0,
                "DodgeRating"  => 20.0,
                "BlockChance"  => 0.20,
                "BlockRating"  => 15.0,
                "ArmorRating"  => 10.0,
                "Resistance" [Tags::PHYSICAL] => 0.10,
                "Resistance" [Tags::FIRE]     => 0.0,
                "Resistance" [Tags::COLD]     => 0.25,
            },
        ))
        .id();

    let sword = commands
        .spawn((
            Name::new("Iron Sword"),
            DamageElement(Tags::PHYSICAL),
            attributes! {
                "Damage" [Tags::PHYSICAL] => 25.0,
            },
        ))
        .id();

    let hit_expr = Expr::compile("Intelligence@attacker * 0.5 + Damage@weapon", None)
        .expect("valid hit expression");

    let staff = commands
        .spawn((
            Name::new("Fire Staff"),
            DamageElement(Tags::FIRE),
            AbilityHit(hit_expr),
            attributes! {
                "Damage" [Tags::FIRE] => 35.0,
            },
        ))
        .id();

    commands.insert_resource(Entities {
        warrior,
        mage,
        goblin,
        sword,
        staff,
    });

    println!("--- Entities spawned ---\n");
}

// ---------------------------------------------------------------------------
// Demo: run attacks through the pipeline
// ---------------------------------------------------------------------------

fn run_demo(
    handles: Res<Entities>,
    mut attributes: AttributesMut,
    q_name: Query<&Name>,
    q_element: Query<&DamageElement>,
    q_ability_hit: Query<&AbilityHit>,
) {
    println!("=== Attack 1: Warrior swings Iron Sword at Goblin ===\n");
    resolve_attack(
        handles.warrior,
        handles.goblin,
        handles.sword,
        &mut attributes,
        &q_name,
        &q_element,
        &q_ability_hit,
    );

    println!("\n=== Attack 2: Mage casts Fire Staff at Goblin ===\n");
    resolve_attack(
        handles.mage,
        handles.goblin,
        handles.staff,
        &mut attributes,
        &q_name,
        &q_element,
        &q_ability_hit,
    );

    println!("\n=== Final Goblin state ===\n");
    let life = attributes.evaluate(handles.goblin, "Life.current");
    let max = attributes.evaluate(handles.goblin, "Life");
    println!("  Life: {life:.1} / {max:.0}");

    println!("\n--- Done ---");
    std::process::exit(0);
}

// ---------------------------------------------------------------------------
// Pipeline: resolve a full attack
// ---------------------------------------------------------------------------

fn element_name(element: TagMask) -> &'static str {
    if element == Tags::PHYSICAL {
        "Physical"
    } else if element == Tags::FIRE {
        "Fire"
    } else if element == Tags::COLD {
        "Cold"
    } else {
        "Unknown"
    }
}

fn resolve_attack(
    attacker: Entity,
    defender: Entity,
    weapon: Entity,
    attributes: &mut AttributesMut,
    q_name: &Query<&Name>,
    q_element: &Query<&DamageElement>,
    q_ability_hit: &Query<&AbilityHit>,
) {
    let atk_name = q_name.get(attacker).map(|n| n.as_str()).unwrap_or("???");
    let def_name = q_name.get(defender).map(|n| n.as_str()).unwrap_or("???");
    let wpn_name = q_name.get(weapon).map(|n| n.as_str()).unwrap_or("???");

    let element = q_element
        .get(weapon)
        .map(|d| d.0)
        .unwrap_or(Tags::PHYSICAL);

    let element_name = element_name(element);

    // Phase 1: Calculate hit value
    let hit_value = if let Ok(ability_hit) = q_ability_hit.get(weapon) {
        let roles: Vec<(&str, Entity)> =
            vec![("attacker", attacker), ("weapon", weapon)];
        attributes.evaluate_expr_with_roles(&ability_hit.0, attacker, &roles)
    } else {
        let accuracy = attributes.value(attacker, "AccuracyRating");
        let wpn_damage = attributes.value(weapon, "Damage");
        accuracy * 0.5 + wpn_damage
    };

    println!("  {atk_name} attacks {def_name} with {wpn_name} ({element_name})");
    println!("  Hit value: {hit_value:.1}");

    // Phase 2: Dodge check
    let def_attrs = attributes.get_attributes(defender).unwrap();
    let dodge_rating = def_attrs.value("DodgeRating");

    if dodge_rating > 0.0 && dodge_rating > hit_value * 0.5 {
        println!("  >> DODGED! (DodgeRating {dodge_rating:.0} > threshold {:.0})",
            hit_value * 0.5);
        return;
    }
    println!("  Dodge check passed (DodgeRating {dodge_rating:.0})");

    // Phase 3: Block check
    let block_chance = def_attrs.value("BlockChance");
    let block_rating = def_attrs.value("BlockRating");

    let remaining_hit = if block_chance > 0.0 {
        let blocked = block_rating.min(hit_value * block_chance);
        let after_block = hit_value - blocked;
        println!("  Block absorbed {blocked:.1} (BlockRating {block_rating:.0}, chance {:.0}%)",
            block_chance * 100.0);
        println!("  Remaining hit: {after_block:.1}");
        after_block
    } else {
        println!("  No block");
        hit_value
    };

    // Phase 4: Armor mitigation
    let armor = def_attrs.value("ArmorRating");
    let after_armor = if armor > 0.0 {
        let mitigated = armor.min(remaining_hit * 0.5);
        let result = remaining_hit - mitigated;
        println!("  Armor absorbed {mitigated:.1} (ArmorRating {armor:.0})");
        result
    } else {
        remaining_hit
    };
    println!("  After armor: {after_armor:.1}");

    // Phase 5: Resistance (tagged by element)
    let resistance = attributes.evaluate_tagged(defender, "Resistance", element);
    let final_damage = after_armor * (1.0 - resistance);
    println!("  {element_name} Resistance: {:.0}%", resistance * 100.0);
    println!("  Final damage: {final_damage:.1}");

    // Phase 6: Apply damage
    let current_life = attributes.evaluate(defender, "Life.current");
    let new_life = current_life - final_damage;
    attributes.set_base(defender, "Life.current", new_life);

    println!("  Life: {current_life:.1} → {new_life:.1}");

    if new_life <= 0.0 && current_life > 0.0 {
        println!("  >> {def_name} has been KILLED by {atk_name}!");
    }
}
