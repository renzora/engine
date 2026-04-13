//! # RPG Combat Example — PoE-style tagged damage
//!
//! Demonstrates `bevy_gauge` with a Path of Exile-style damage model:
//!
//! - **`define_tags!`** — declares a tag hierarchy with group tags that OR
//!   their children together.
//! - **`tagged_attribute`** — sets up a multi-part attribute (Added × (1 + Increased))
//!   with per-tag-combo expressions materialized lazily.
//! - **Insert broadly, query specifically** — a modifier tagged `PHYSICAL`
//!   applies to all physical damage. When dealing a physical sword hit, query
//!   `PHYSICAL | SWORD` to pull in global, physical-only, sword-only, and
//!   physical+sword modifiers.
//! - **Cross-entity deps** — the sword references its wielder's attributes via
//!   `@Wielder`. Swapping the wielder automatically rewires everything.
//! - **`attributes!` / `mod_set!` macros** — ergonomic batch init and buffs.
//!
//! Run with: `cargo run --example path_of_exile`

use bevy::prelude::*;
use bevy_gauge::prelude::*;

// ---------------------------------------------------------------------------
// Tags
// ---------------------------------------------------------------------------

define_tags! {
    Tags,
    damage_type {
        elemental { fire, cold },
        physical,
    },
    weapon_type {
        melee { sword, axe },
        ranged { bow },
    },
}

// ---------------------------------------------------------------------------
// Marker components & handles
// ---------------------------------------------------------------------------

#[derive(Component)]
struct Sword;

#[derive(Resource)]
struct Entities {
    warrior: Entity,
    mage: Entity,
    sword: Entity,
}

// ---------------------------------------------------------------------------
// AttributeDerived — auto-syncs a component from tagged attribute values
// ---------------------------------------------------------------------------

#[derive(Component, Default, Debug, AttributeComponent)]
struct SwordDamageDisplay {
    #[read("Damage", Tags::PHYSICAL | Tags::SWORD)]
    physical_sword: f32,
    #[read("Damage", Tags::FIRE | Tags::SWORD)]
    fire_sword: f32,
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
                setup_sword_attributes.after(spawn_entities),
                equip_warrior.after(setup_sword_attributes),
                print_warrior.after(equip_warrior),
                swap_to_mage.after(print_warrior),
                print_mage.after(swap_to_mage),
                show_tag_queries.after(print_mage),
                apply_buff_and_show.after(show_tag_queries),
            ),
        )
        .run();
}

// ---------------------------------------------------------------------------
// Step 0: Register tag names so expressions can use {FIRE|SWORD} syntax
// ---------------------------------------------------------------------------

fn register_tags(mut resolver: ResMut<TagResolver>) {
    Tags::register(&mut resolver);
    println!("--- Tags registered ---\n");
}

// ---------------------------------------------------------------------------
// Step 1: Spawn entities
// ---------------------------------------------------------------------------

fn spawn_entities(mut commands: Commands) {
    let warrior = commands
        .spawn((
            attributes! {
                "Strength"     => 50.0,
                "Intelligence" => 10.0,
            },
            Name::new("Warrior"),
        ))
        .id();

    let mage = commands
        .spawn((
            attributes! {
                "Strength"     => 15.0,
                "Intelligence" => 60.0,
            },
            Name::new("Mage"),
        ))
        .id();

    let sword = commands
        .spawn((
            Sword,
            SwordDamageDisplay::default(),
            Name::new("Flaming Greatsword"),
            Attributes::new(),
        ))
        .id();

    commands.insert_resource(Entities {
        warrior,
        mage,
        sword,
    });

    println!("--- Entities spawned ---\n");
}

// ---------------------------------------------------------------------------
// Step 2: Configure sword attributes
// ---------------------------------------------------------------------------

fn setup_sword_attributes(mut attributes: AttributesMut, handles: Res<Entities>) {
    let sword = handles.sword;

    attributes
        .tagged_attribute(
            sword,
            "Damage",
            &[("added", ReduceFn::Sum), ("increased", ReduceFn::Sum), ("more", ReduceFn::Product)],
            "added * (1 + increased) * more",
        )
        .expect("valid tagged attribute");

    // Flat damage — tagged broadly by damage type
    attributes.add_modifier_tagged(sword, "Damage.added", 25.0, Tags::PHYSICAL);
    attributes.add_modifier_tagged(sword, "Damage.added", 10.0, Tags::FIRE);

    // Increased damage — scales from wielder stats
    attributes
        .add_expr_modifier_tagged(
            sword,
            "Damage.increased",
            "Strength@Wielder / 200",
            Tags::PHYSICAL,
        )
        .expect("valid expression");

    attributes
        .add_expr_modifier_tagged(
            sword,
            "Damage.increased",
            "Intelligence@Wielder / 300",
            Tags::FIRE,
        )
        .expect("valid expression");

    println!("--- Sword attributes configured ---\n");
}

// ---------------------------------------------------------------------------
// Step 3: Equip on the Warrior
// ---------------------------------------------------------------------------

fn equip_warrior(mut attributes: AttributesMut, handles: Res<Entities>) {
    println!("=== Warrior equips the Flaming Greatsword ===\n");
    attributes.register_source(handles.sword, "Wielder", handles.warrior);
}

// ---------------------------------------------------------------------------
// Step 4: Print Warrior damage — query with leaf combos
// ---------------------------------------------------------------------------

fn print_warrior(mut attributes: AttributesMut, handles: Res<Entities>) {
    let sword = handles.sword;

    let phys_added = attributes.evaluate_tagged(sword, "Damage.added", Tags::PHYSICAL | Tags::SWORD);
    let phys_inc = attributes.evaluate_tagged(sword, "Damage.increased", Tags::PHYSICAL | Tags::SWORD);
    let phys_total = attributes.evaluate_tagged(sword, "Damage", Tags::PHYSICAL | Tags::SWORD);
    let fire_added = attributes.evaluate_tagged(sword, "Damage.added", Tags::FIRE | Tags::SWORD);
    let fire_inc = attributes.evaluate_tagged(sword, "Damage.increased", Tags::FIRE | Tags::SWORD);
    let fire_total = attributes.evaluate_tagged(sword, "Damage", Tags::FIRE | Tags::SWORD);

    println!("  Physical Sword Damage:");
    println!("    Added:     {phys_added:.1}");
    println!("    Increased: {phys_inc:.4} ({:.1}%)", phys_inc * 100.0);
    println!("    Total:     {phys_total:.2}  = {phys_added:.1} * (1 + {phys_inc:.4})");

    println!("  Fire Sword Damage:");
    println!("    Added:     {fire_added:.1}");
    println!("    Increased: {fire_inc:.4} ({:.1}%)", fire_inc * 100.0);
    println!("    Total:     {fire_total:.2}  = {fire_added:.1} * (1 + {fire_inc:.4})");

    println!("\n  Expected (Warrior: Str 50, Int 10):");
    println!("    Physical: 25 * (1 + 50/200) = 25 * 1.25  = 31.25");
    println!("    Fire:     10 * (1 + 10/300) = 10 * 1.033 = 10.33");
    println!();
}

// ---------------------------------------------------------------------------
// Step 5: Swap wielder to the Mage
// ---------------------------------------------------------------------------

fn swap_to_mage(mut attributes: AttributesMut, handles: Res<Entities>) {
    println!("=== Mage takes the sword from the Warrior ===");
    println!("    (one call to register_source — edges auto-rewire)\n");
    attributes.register_source(handles.sword, "Wielder", handles.mage);
}

// ---------------------------------------------------------------------------
// Step 6: Print Mage damage
// ---------------------------------------------------------------------------

fn print_mage(mut attributes: AttributesMut, handles: Res<Entities>) {
    let sword = handles.sword;

    let phys_added = attributes.evaluate_tagged(sword, "Damage.added", Tags::PHYSICAL | Tags::SWORD);
    let phys_inc = attributes.evaluate_tagged(sword, "Damage.increased", Tags::PHYSICAL | Tags::SWORD);
    let phys_total = attributes.evaluate_tagged(sword, "Damage", Tags::PHYSICAL | Tags::SWORD);
    let fire_added = attributes.evaluate_tagged(sword, "Damage.added", Tags::FIRE | Tags::SWORD);
    let fire_inc = attributes.evaluate_tagged(sword, "Damage.increased", Tags::FIRE | Tags::SWORD);
    let fire_total = attributes.evaluate_tagged(sword, "Damage", Tags::FIRE | Tags::SWORD);

    println!("  Physical Sword Damage:");
    println!("    Added:     {phys_added:.1}");
    println!("    Increased: {phys_inc:.4} ({:.1}%)", phys_inc * 100.0);
    println!("    Total:     {phys_total:.2}  = {phys_added:.1} * (1 + {phys_inc:.4})");

    println!("  Fire Sword Damage:");
    println!("    Added:     {fire_added:.1}");
    println!("    Increased: {fire_inc:.4} ({:.1}%)", fire_inc * 100.0);
    println!("    Total:     {fire_total:.2}  = {fire_added:.1} * (1 + {fire_inc:.4})");

    println!("\n  Expected (Mage: Str 15, Int 60):");
    println!("    Physical: 25 * (1 + 15/200) = 25 * 1.075 = 26.88");
    println!("    Fire:     10 * (1 + 60/300) = 10 * 1.200 = 12.00");
    println!();
}

// ---------------------------------------------------------------------------
// Step 7: Show how tag specificity works
// ---------------------------------------------------------------------------

fn show_tag_queries(mut attributes: AttributesMut, handles: Res<Entities>) {
    println!("=== Tag Query Specificity (Mage wielding) ===\n");

    let sword = handles.sword;

    // Damage.added has two modifiers:
    //   25.0 [PHYSICAL]  and  10.0 [FIRE]
    //
    // A modifier matches when ALL its tags are present in the query.
    // Broad modifiers match more queries.

    let phys_sword = attributes.evaluate_tagged(sword, "Damage.added", Tags::PHYSICAL | Tags::SWORD);
    let fire_sword = attributes.evaluate_tagged(sword, "Damage.added", Tags::FIRE | Tags::SWORD);

    println!("  Damage.added [PHYSICAL|SWORD]: {phys_sword:.1}  (25 physical matches)");
    println!("  Damage.added [FIRE|SWORD]:     {fire_sword:.1}  (10 fire matches)");

    // Add a generic sword modifier — tagged SWORD only, applies to all sword queries
    println!("\n  Adding +5 generic sword damage (tagged SWORD only)...\n");
    attributes.add_modifier_tagged(sword, "Damage.added", 5.0, Tags::SWORD);

    let phys_sword2 = attributes.evaluate_tagged(sword, "Damage.added", Tags::PHYSICAL | Tags::SWORD);
    let fire_sword2 = attributes.evaluate_tagged(sword, "Damage.added", Tags::FIRE | Tags::SWORD);

    println!("  Damage.added [PHYSICAL|SWORD]: {phys_sword2:.1}  (25 physical + 5 sword)");
    println!("  Damage.added [FIRE|SWORD]:     {fire_sword2:.1}  (10 fire + 5 sword)");

    // Add a global modifier — no tags, applies to everything
    println!("\n  Adding +3 global damage (untagged)...\n");
    attributes.add_modifier(sword, "Damage.added", 3.0);

    let phys_sword3 = attributes.evaluate_tagged(sword, "Damage.added", Tags::PHYSICAL | Tags::SWORD);
    let fire_sword3 = attributes.evaluate_tagged(sword, "Damage.added", Tags::FIRE | Tags::SWORD);

    println!("  Damage.added [PHYSICAL|SWORD]: {phys_sword3:.1}  (25 physical + 5 sword + 3 global)");
    println!("  Damage.added [FIRE|SWORD]:     {fire_sword3:.1}  (10 fire + 5 sword + 3 global)");
    println!();
}

// ---------------------------------------------------------------------------
// Step 8: Apply a buff using mod_set!
// ---------------------------------------------------------------------------

fn apply_buff_and_show(mut attributes: AttributesMut, handles: Res<Entities>) {
    println!("=== Applying Fire Enchantment via mod_set! ===\n");

    let enchantment = mod_set! {
        "Damage.added" [Tags::FIRE] => 20.0,
    };
    enchantment.apply(handles.sword, &mut attributes);

    let fire_added = attributes.evaluate_tagged(handles.sword, "Damage.added", Tags::FIRE | Tags::SWORD);
    let fire_total = attributes.evaluate_tagged(handles.sword, "Damage", Tags::FIRE | Tags::SWORD);

    println!("  After +20 fire enchantment:");
    println!("    Damage.added [FIRE|SWORD]: {fire_added:.1}  (10 fire + 5 sword + 3 global + 20 fire = 38)");
    println!("    Damage [FIRE|SWORD]:       {fire_total:.2}  = 38 * (1 + 60/300) = 38 * 1.2 = 45.60");

    println!("\n--- Done ---");
    std::process::exit(0);
}
