//! # Custom Extensions Example — Typed API over String Keys
//!
//! Demonstrates how to wrap `Attributes` and `AttributesMut` with extension
//! traits that provide a typed, game-specific API.
//!
//! - **`DamageMutExt`** on `AttributesMut` — `setup_damage_pipeline(entity)`
//!   wires up a `tagged_attribute` with `added`, `increased`, `more` parts
//! - **`DamageExt`** on `Attributes` — `.damage(tags)` reads the evaluated
//!   damage for a specific tag combo
//! - The typed layer composes cleanly with the underlying string-key system
//!
//! Run with: `cargo run --example custom_extensions`

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
// Extension trait: reading damage
// ---------------------------------------------------------------------------

trait DamageExt {
    fn damage(&self, tags: TagMask) -> f32;
    fn damage_added(&self, tags: TagMask) -> f32;
    fn damage_increased(&self, tags: TagMask) -> f32;
    fn damage_more(&self, tags: TagMask) -> f32;
}

impl DamageExt for Attributes {
    fn damage(&self, tags: TagMask) -> f32 {
        self.value_tagged("Damage", tags)
    }

    fn damage_added(&self, tags: TagMask) -> f32 {
        self.value_tagged("Damage.added", tags)
    }

    fn damage_increased(&self, tags: TagMask) -> f32 {
        self.value_tagged("Damage.increased", tags)
    }

    fn damage_more(&self, tags: TagMask) -> f32 {
        self.value_tagged("Damage.more", tags)
    }
}

// ---------------------------------------------------------------------------
// Extension trait: mutating damage
// ---------------------------------------------------------------------------

trait DamageMutExt {
    fn setup_damage_pipeline(&mut self, entity: Entity);
    fn add_damage(&mut self, entity: Entity, part: &str, value: f32, tags: TagMask);
    fn evaluate_damage(&mut self, entity: Entity, tags: TagMask) -> f32;
}

impl<F: bevy::ecs::query::QueryFilter> DamageMutExt for AttributesMut<'_, '_, F> {
    fn setup_damage_pipeline(&mut self, entity: Entity) {
        self.tagged_attribute(
            entity,
            "Damage",
            &[
                ("added", ReduceFn::Sum),
                ("increased", ReduceFn::Sum),
                ("more", ReduceFn::Product),
            ],
            "added * (1 + increased) * more",
        )
        .expect("valid damage pipeline expression");
    }

    fn add_damage(&mut self, entity: Entity, part: &str, value: f32, tags: TagMask) {
        let path = format!("Damage.{part}");
        self.add_modifier_tagged(entity, &path, value, tags);
    }

    fn evaluate_damage(&mut self, entity: Entity, tags: TagMask) -> f32 {
        self.evaluate_tagged(entity, "Damage", tags)
    }
}

// ---------------------------------------------------------------------------
// Resource
// ---------------------------------------------------------------------------

#[derive(Resource)]
struct Entities {
    sword: Entity,
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
                configure_sword.after(spawn_entities),
                demo.after(configure_sword),
            ),
        )
        .run();
}

fn register_tags(mut resolver: ResMut<TagResolver>) {
    Tags::register(&mut resolver);
    println!("--- Tags registered ---\n");
}

// ---------------------------------------------------------------------------
// Setup: create a sword, then configure it via the typed API
// ---------------------------------------------------------------------------

fn spawn_entities(mut commands: Commands) {
    let sword = commands
        .spawn((
            Name::new("Flaming Greatsword"),
            Attributes::new(),
        ))
        .id();

    commands.insert_resource(Entities { sword });
}

fn configure_sword(handles: Res<Entities>, mut attributes: AttributesMut) {
    let sword = handles.sword;

    attributes.setup_damage_pipeline(sword);

    attributes.add_damage(sword, "added", 100.0, Tags::PHYSICAL);
    attributes.add_damage(sword, "added", 30.0, Tags::FIRE);
    attributes.add_damage(sword, "added", 5.0, Tags::SWORD);
    attributes.add_damage(sword, "increased", 0.50, Tags::PHYSICAL);
    attributes.add_damage(sword, "increased", 0.25, Tags::FIRE);
    attributes.add_damage(sword, "more", 0.20, Tags::PHYSICAL);

    println!("--- Sword configured via typed API ---\n");
}

// ---------------------------------------------------------------------------
// Demo: read damage using both typed and string-key APIs
// ---------------------------------------------------------------------------

fn demo(
    handles: Res<Entities>,
    mut attributes: AttributesMut,
) {
    let sword = handles.sword;

    // --- Read via typed API on AttributesMut ---
    let phys_sword = attributes.evaluate_damage(sword, Tags::PHYSICAL | Tags::SWORD);
    let fire_sword = attributes.evaluate_damage(sword, Tags::FIRE | Tags::SWORD);

    println!("=== Damage via typed AttributesMut API ===\n");
    println!("  Damage [PHYSICAL|SWORD]: {phys_sword:.2}");
    println!("  Damage [FIRE|SWORD]:     {fire_sword:.2}");

    // --- Read via typed API on &Attributes (after evaluation has cached values) ---
    if let Some(attrs) = attributes.get_attributes(sword) {
        println!("\n=== Damage via typed Attributes API (read-only) ===\n");
        let phys = attrs.damage(Tags::PHYSICAL | Tags::SWORD);
        let fire = attrs.damage(Tags::FIRE | Tags::SWORD);
        let phys_added = attrs.damage_added(Tags::PHYSICAL | Tags::SWORD);
        let phys_inc = attrs.damage_increased(Tags::PHYSICAL | Tags::SWORD);
        let phys_more = attrs.damage_more(Tags::PHYSICAL | Tags::SWORD);

        println!("  Damage [PHYSICAL|SWORD]: {phys:.2}");
        println!("    added:     {phys_added:.1}");
        println!("    increased: {phys_inc:.2}");
        println!("    more:      {phys_more:.2}");
        println!("  Damage [FIRE|SWORD]:     {fire:.2}");
    }

    // --- String keys still work alongside typed accessors ---
    println!("\n=== String-key API still works ===\n");
    let raw_added = attributes.evaluate_tagged(sword, "Damage.added", Tags::PHYSICAL | Tags::SWORD);
    println!("  attrs.evaluate_tagged(\"Damage.added\", PHYSICAL|SWORD): {raw_added:.1}");

    println!("\n  Expected:");
    println!("    Physical+Sword added:     100 (physical) + 5 (sword) = 105");
    println!("    Physical+Sword increased: 0.50");
    println!("    Physical+Sword more:      1.20 (Product: 1 + 0.20)");
    println!("    Physical+Sword total:     105 * (1 + 0.50) * 1.20 = 189.00");
    println!("    Fire+Sword added:         30 (fire) + 5 (sword) = 35");
    println!("    Fire+Sword increased:     0.25");
    println!("    Fire+Sword more:          1.00 (no fire-tagged more)");
    println!("    Fire+Sword total:         35 * (1 + 0.25) * 1.00 = 43.75");

    println!("\n--- Done ---");
    std::process::exit(0);
}
