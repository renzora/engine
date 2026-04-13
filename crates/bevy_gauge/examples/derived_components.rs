//! # Derived Components Example — Attribute → Component Sync
//!
//! Demonstrates how attribute values flow to typed Bevy components:
//!
//! - **`Life`** component with `#[read("Life")]` for max and `#[read]` for
//!   current — the derive macro auto-syncs these from attributes
//! - **Observer** on `Remove<AttributeInitializer>` to initialize current = max
//! - **Manual `AttributeDerived`** that interprets `"Alive"` (f32) as a `bool`
//!
//! Run with: `cargo run --example derived_components`

use bevy::prelude::*;
use bevy_gauge::prelude::*;

// ---------------------------------------------------------------------------
// Life component — derive macro with #[read] and #[write]
// ---------------------------------------------------------------------------

#[derive(Component, Default, Debug, AttributeComponent)]
struct Life {
    #[read("Life")]
    max: f32,
    #[read]
    current: f32,
}

// ---------------------------------------------------------------------------
// AliveStatus — manual AttributeDerived (f32 → bool)
// ---------------------------------------------------------------------------

#[derive(Component, Default, Debug)]
struct AliveStatus {
    alive: bool,
}

impl AttributeDerived for AliveStatus {
    fn should_update(&self, attrs: &Attributes) -> bool {
        let attr_alive = attrs.value("Alive") > 0.0;
        self.alive != attr_alive
    }

    fn update_from_attributes(&mut self, attrs: &Attributes) {
        self.alive = attrs.value("Alive") > 0.0;
    }
}

register_derived!(AliveStatus);

// ---------------------------------------------------------------------------
// Observer: initialize Life.current = Life.max after spawn
// ---------------------------------------------------------------------------

fn on_attributes_initialized(
    trigger: On<Remove, AttributeInitializer>,
    mut q_life: Query<(&mut Life, &Attributes)>,
) {
    let entity = trigger.event_target();
    if let Ok((mut life, attrs)) = q_life.get_mut(entity) {
        life.max = attrs.value("Life");
        life.current = life.max;
    }
}

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

#[derive(Resource)]
struct Entities {
    hero: Entity,
}

/// Drives the demo one step per frame so `PostUpdate` sync runs between steps.
#[derive(Resource)]
struct DemoStep(usize);

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(AttributesPlugin)
        .add_observer(on_attributes_initialized)
        .add_systems(Startup, spawn)
        .add_systems(Update, demo)
        .run();
}

// ---------------------------------------------------------------------------
// Spawn
// ---------------------------------------------------------------------------

fn spawn(mut commands: Commands) {
    let hero = commands
        .spawn((
            Name::new("Hero"),
            Life::default(),
            AliveStatus { alive: true },
            attributes! {
                "Life"  => 100.0,
                "Life.current" => 100.0,
                "Alive" => 1.0,
            },
        ))
        .id();

    commands.insert_resource(Entities { hero });
    commands.insert_resource(DemoStep(0));
    println!("--- Hero spawned ---\n");
}

// ---------------------------------------------------------------------------
// Demo — one step per frame so PostUpdate derived‐component sync fires between
// ---------------------------------------------------------------------------

fn demo(
    handles: Res<Entities>,
    mut attributes: AttributesMut,
    q_life: Query<&Life>,
    q_alive: Query<&AliveStatus>,
    mut step: ResMut<DemoStep>,
) {
    let hero = handles.hero;

    match step.0 {
        // Frame 0: print initial state (PostUpdate already synced after Startup)
        0 => {
            println!("=== Initial state ===\n");
            print_state(hero, &mut attributes, &q_life, &q_alive);

            println!("=== Taking 60 damage ===\n");
            let current = attributes.evaluate(hero, "Life.current");
            attributes.set_base(hero, "Life.current", current - 60.0);
        }
        // Frame 1: components synced by PostUpdate — print, then apply lethal hit
        1 => {
            print_state(hero, &mut attributes, &q_life, &q_alive);

            println!("=== Taking 50 more damage (lethal) ===\n");
            let current = attributes.evaluate(hero, "Life.current");
            attributes.set_base(hero, "Life.current", current - 50.0);
            attributes.set_base(hero, "Alive", 0.0);
        }
        // Frame 2: components synced — print, then resurrect
        2 => {
            print_state(hero, &mut attributes, &q_life, &q_alive);

            println!("=== Resurrecting (set Alive back to 1.0) ===\n");
            attributes.set_base(hero, "Alive", 1.0);
            let max = attributes.evaluate(hero, "Life");
            attributes.set_base(hero, "Life.current", max);
        }
        // Frame 3: components synced — print final state and exit
        3 => {
            print_state(hero, &mut attributes, &q_life, &q_alive);
            println!("--- Done ---");
            std::process::exit(0);
        }
        _ => {}
    }

    step.0 += 1;
}

fn print_state(
    entity: Entity,
    attributes: &mut AttributesMut,
    q_life: &Query<&Life>,
    q_alive: &Query<&AliveStatus>,
) {
    let life_val = attributes.evaluate(entity, "Life");
    let current_val = attributes.evaluate(entity, "Life.current");
    let alive_val = attributes.evaluate(entity, "Alive");

    println!("  Attributes:");
    println!("    Life:         {life_val:.0}");
    println!("    Life.current: {current_val:.1}");
    println!("    Alive:        {alive_val:.0}");

    if let Ok(life) = q_life.get(entity) {
        println!("  Life component:  max={:.0}, current={:.1}", life.max, life.current);
    }
    if let Ok(status) = q_alive.get(entity) {
        println!("  AliveStatus:     alive={}", status.alive);
    }
    println!();
}
