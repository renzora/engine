# bevy_gauge

A attribute system for [Bevy](https://bevy.org/).

Built for games where attributes depend on other attributes — RPGs with derived attributes, ARPGs with PoE-style damage pipelines, or anything where changing one value should automatically ripple through a chain of dependencies, even across entities.

## Quick Start

```rust
use bevy::prelude::*;
use bevy_gauge::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(AttributesPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(attributes! {
        "Strength"  => 20.0,
        "Vitality"  => 15.0,
        "MaxHealth"  => "Vitality * 10.0 + 50.0",
    });
}
```

Change `Vitality` later and `MaxHealth` updates automatically.

## Features

### Flat Attributes

Simple numeric values. Modifiers are summed:

```rust
attributes.flat_attribute(entity, "Armor", 50.0);
attributes.add_modifier(entity, "Armor", 25.0); // now 75
```

### Expression Modifiers

Modifiers can be expressions that reference other attributes. Dependencies are tracked automatically:

```rust
attributes.add_expr_modifier(entity, "MaxHealth", "Vitality * 10.0")?;
attributes.add_expr_modifier(entity, "HealthRegen", "MaxHealth * 0.01")?;

// Changing Vitality propagates → MaxHealth → HealthRegen
attributes.add_modifier(entity, "Vitality", 5.0);
```

Expressions support arithmetic, parentheses, `min`/`max`/`abs`/`clamp`, cross-entity references (`Strength@Wielder`), and tag queries (`Damage{FIRE|MELEE}`).

### Complex Attributes

Named parts combined by an expression — each part receives modifiers independently:

```rust
// PoE-style: base * (1 + increased) * more
attributes.complex_attribute(
    entity,
    "Damage",
    &[
        ("base",      ReduceFn::Sum),
        ("increased", ReduceFn::Sum),
        ("more",      ReduceFn::Product),
    ],
    "base * (1 + increased) * more",
)?;

attributes.add_modifier(entity, "Damage.base", 100.0);
attributes.add_modifier(entity, "Damage.increased", 0.5);  // +50%
attributes.add_modifier(entity, "Damage.more", 0.2);        // 20% more → 1.2x
// Damage = 100 * 1.5 * 1.2 = 180
```

`Sum` adds modifiers together. `Product` multiplies them as `(1+v)` factors.

### Tags and Filtered Evaluation

Attach tags to modifiers, then query with a filter. Modifiers are inserted *generally* — a modifier tagged `FIRE` applies to any query that includes fire. Queries are *specific* — you query a leaf combination like `FIRE | SWORD` to get the total for that exact damage instance.

This lets you naturally express things like "+10 fire damage" (applies to fire swords, fire bows, etc.), "+5 sword damage" (applies to physical swords, fire swords, etc.), and "+3 damage" (applies to everything).

Define your tag hierarchy with the `define_tags!` macro:

```rust
define_tags! {
    DamageTags,
    damage_type {
        elemental { fire, cold, lightning },
        physical,
        chaos,
    },
    weapon_type {
        melee { sword, axe },
        ranged { bow, wand },
    },
}

// Register at startup
fn setup_tags(mut resolver: ResMut<TagResolver>) {
    DamageTags::register(&mut resolver);
}
```

This generates a struct with `TagMask` constants: `DamageTags::FIRE`, `DamageTags::SWORD`, `DamageTags::ELEMENTAL`, etc. Group tags like `ELEMENTAL` are the OR of their children (`FIRE | COLD | LIGHTNING`).

```rust
// Inserting — tag as broadly as the modifier applies
attributes.add_modifier_tagged(entity, "Damage.added", 100.0, DamageTags::PHYSICAL);  // all physical
attributes.add_modifier_tagged(entity, "Damage.added", 10.0, DamageTags::FIRE);       // all fire
attributes.add_modifier_tagged(entity, "Damage.added", 5.0, DamageTags::SWORD);       // all sword
attributes.add_modifier(entity, "Damage.added", 3.0);                                  // all damage (global)

// Querying
let fire_sword = attributes.evaluate_tagged(entity, "Damage", DamageTags::FIRE | DamageTags::SWORD);
// = fire(10) + sword(5) + global(3) = 18

let phys_sword = attributes.evaluate_tagged(entity, "Damage", DamageTags::PHYSICAL | DamageTags::SWORD);
// = physical(100) + sword(5) + global(3) = 108

let fire_bow = attributes.evaluate_tagged(entity, "Damage", DamageTags::FIRE | DamageTags::BOW);
// = fire(10) + global(3) = 13
```

A modifier matches a query when all of its tag bits are present in the query. Untagged modifiers are global — they match everything.

### Cross-Entity Dependencies

Attributes on one entity can reference attributes on another through source aliases:

```rust
// Sword damage scales with wielder's Strength
attributes.add_expr_modifier(sword, "Damage.increased", "Strength@Wielder / 200.0")?;
attributes.register_source(sword, "Wielder", warrior);

// Hand the sword to someone else — one call, everything updates
attributes.register_source(sword, "Wielder", mage);
```

### Instant (One-Shot) Mutations

Apply attribute changes once without leaving persistent modifiers. Used for damage application, ability effects, etc.

Expressions can reference attributes on **role entities** via `@role` syntax. Roles are temporary source aliases that only exist for the duration of the evaluation:

```rust
// An arrow hits a target. Damage depends on the arrow, the bow, and the attacker.
let on_hit = instant! {
    "Life.current" -= "Damage@arrow + Damage@bow + (Agility@attacker * 0.1)",
};

let roles: &[(&str, Entity)] = &[
    ("arrow", arrow_entity),
    ("bow", bow_entity),
    ("attacker", attacker_entity),
];
attributes.apply_instant(&on_hit, roles, target_entity);
```

### Batch Operations

```rust
// Spawn-time initialization
commands.spawn(attributes! {
    "Strength" => 50.0,
    "MaxHealth" => "Strength * 2.0 + 100.0",
    "Damage.added" [FIRE | MELEE] => 10.0,
});

// Runtime buffs
let enchant = mod_set! {
    "Damage.added" [FIRE | MELEE] => 20.0,
    "Damage.increased" [FIRE] => 0.15,
};
enchant.apply(sword, &mut attributes);
```

### Attribute Requirements

Boolean gates over attributes for equipment prerequisites, ability conditions, state machine transitions:

```rust
commands.spawn(requires! { "Strength >= 10", "Level >= 5" });
```

### Derived Components

Sync struct fields with attribute values using a derive macro:

```rust
#[derive(Component, Default, AttributeComponent)]
pub struct Life {
    #[read("Life")]
    pub max: f32,
    #[write]
    pub current: f32,  // writes back to "Life.current"
}
```

Fields with `#[read]` update from attributes automatically. Fields with `#[write]` push values back into the attribute system.

## Reading vs Writing

**Reading** only needs `&Attributes`:

```rust
fn print_health(q_attrs: Query<&Attributes>) {
    for attrs in &q_attrs {
        println!("Health: {}", attrs.value("MaxHealth"));
    }
}
```

**Writing** goes through `AttributesMut`, which maintains dependency edges and propagates changes:

```rust
fn buff_strength(mut attributes: AttributesMut) {
    attributes.add_modifier(entity, "Strength", 10.0);
}
```

## License

MIT OR Apache-2.0
