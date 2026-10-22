//! The runtime ends of two features that need a Bevy app: a script setting an
//! effect's Parameter nodes by name, and Collide Scene asking the scene's
//! colliders through `renzora::scene_query`, here answered by a stand-in
//! floor so no physics engine is linked.
//!
//! Time is stepped manually at 60 Hz, as in `runtime.rs`.

use std::collections::HashMap;
use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use renzora::scene_query::{SceneHit, SceneQueries, SceneQuery, SceneQueryBatch};
use renzora::{ScriptAction, ScriptActionValue};
use renzora_particles::asset::{Connection, Emitter, PinValue, VfxGraph, VfxNode};
use renzora_particles::nodes::OUTPUT_NODE;
use renzora_particles::{ParticleParams, ParticleRuntime, ParticleStats, ParticleSystem, ParticlesPlugin, VfxAsset};

fn app() -> App {
    let mut app = renzora_test_harness::headless_app_with(|app| {
        app.add_plugins(ParticlesPlugin);
    });
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 60.0)));
    app
}

fn frames(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn spawn(app: &mut App, asset: VfxAsset) -> Entity {
    let handle = app.world_mut().resource_mut::<Assets<VfxAsset>>().add(asset);
    app.world_mut().spawn((ParticleSystem::from_handle(handle), Name::new("fx"))).id()
}

fn effect(values: &[(&str, PinValue)], nodes: Vec<VfxNode>, connections: Vec<Connection>) -> VfxAsset {
    let mut out = VfxNode::new(1, OUTPUT_NODE);
    for (k, v) in values {
        out.input_values.insert(k.to_string(), v.clone());
    }
    let mut all = vec![out];
    all.extend(nodes);
    VfxAsset {
        duration: 0.0,
        looping: false,
        emitters: vec![Emitter {
            capacity: 1000,
            graph: VfxGraph { nodes: all, connections, ..Default::default() },
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn alive(app: &App, e: Entity) -> u32 {
    app.world().get::<ParticleStats>(e).map(|s| s.alive).unwrap_or(0)
}

fn call(app: &mut App, entity: Entity, name: &str, args: &[(&str, ScriptActionValue)]) {
    let args: HashMap<String, ScriptActionValue> = args.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    app.world_mut().trigger(ScriptAction { name: name.to_string(), entity, target_entity: None, args });
}

#[test]
fn a_script_sets_and_clears_a_parameter_by_name() {
    let mut app = app();
    let rate = VfxNode::new(2, "input/parameter")
        .with("name", PinValue::Text("rate".into()))
        .with("value", PinValue::Float(0.0));
    let e = spawn(
        &mut app,
        effect(&[("lifetime", PinValue::Float(100.0))], vec![rate], vec![Connection::new(2, "value", 1, "spawn_rate")]),
    );
    frames(&mut app, 30);
    assert_eq!(alive(&app, e), 0, "its own value spawns nothing");

    call(&mut app, e, "vfx_set_param", &[("name", ScriptActionValue::String("rate".into())), ("value", ScriptActionValue::Float(120.0))]);
    frames(&mut app, 60);
    let set = alive(&app, e);
    assert!(set > 90, "120 per second for a second: {set}");
    assert_eq!(app.world().get::<ParticleParams>(e).unwrap().values["rate"], Vec4::splat(120.0));

    call(&mut app, e, "vfx_clear_param", &[("name", ScriptActionValue::String("rate".into()))]);
    frames(&mut app, 30);
    let after = alive(&app, e);
    assert!((set..=set + 1).contains(&after), "cleared, it spawns no more: {set} then {after}");
}

#[test]
fn the_on_forms_find_the_effect_by_name_and_values_survive_a_rebuild() {
    let mut app = app();
    let launch = VfxNode::new(2, "input/parameter")
        .with("name", PinValue::Text("launch".into()))
        .with("value", PinValue::Vec3([0.0, 0.0, 0.0]));
    let e = spawn(
        &mut app,
        effect(
            &[("spawn_rate", PinValue::Float(30.0)), ("lifetime", PinValue::Float(100.0))],
            vec![launch],
            vec![Connection::new(2, "value", 1, "velocity")],
        ),
    );
    let script = app.world_mut().spawn_empty().id();
    frames(&mut app, 2);
    call(
        &mut app,
        script,
        "vfx_set_param_vec3",
        &[
            ("entity", ScriptActionValue::String("fx".into())),
            ("name", ScriptActionValue::String("launch".into())),
            ("value", ScriptActionValue::Vec3([0.0, 5.0, 0.0])),
        ],
    );
    frames(&mut app, 10);
    let newest_up = |app: &App| {
        let r = app.world().get::<ParticleRuntime>(e).unwrap();
        let p = &r.sim.emitters[0].particles;
        (0..p.len()).max_by_key(|&i| p.id[i]).map(|i| p.velocity[i].y).unwrap_or(0.0)
    };
    assert!((newest_up(&app) - 5.0).abs() < 1e-3, "born with the script's velocity");

    // A hot reload builds a fresh simulation; the value is put back.
    let handle = app.world().get::<ParticleSystem>(e).unwrap().asset.clone();
    {
        let mut assets = app.world_mut().resource_mut::<Assets<VfxAsset>>();
        let mut asset = assets.get_mut(&handle).expect("asset");
        asset.name = "edited".into();
    }
    frames(&mut app, 10);
    assert!((newest_up(&app) - 5.0).abs() < 1e-3, "still the script's velocity after the rebuild");

    let extensions = app.world().resource::<renzora::script_extension::ScriptExtensions>();
    for name in ["vfx_set_param", "vfx_set_param_vec3", "vfx_set_param_color", "vfx_clear_param", "vfx_set_param_on"] {
        assert!(extensions.bindings().iter().any(|b| b.name == name), "{name} is not declared");
    }
}

/// The scene: a floor at y = 0 under everything.
struct Floor;

impl SceneQuery for Floor {
    fn cast(&self, _world: &mut World, batch: &mut SceneQueryBatch) {
        batch.hits = batch
            .rays
            .iter()
            .map(|r| {
                let above = r.origin.y - r.radius;
                if r.direction.y >= 0.0 || above < 0.0 {
                    return None;
                }
                let distance = above / -r.direction.y;
                (distance <= r.max_distance).then(|| SceneHit {
                    distance,
                    point: r.origin + r.direction * distance,
                    normal: Vec3::Y,
                    entity: Entity::PLACEHOLDER,
                })
            })
            .collect();
    }
}

fn falling() -> VfxAsset {
    let floor = VfxNode::new(2, "collide/scene").with("bounce", PinValue::Float(0.3));
    effect(
        &[
            ("spawn_rate", PinValue::Float(0.0)),
            ("burst_count", PinValue::Float(20.0)),
            ("lifetime", PinValue::Float(100.0)),
            ("velocity", PinValue::Vec3([0.5, 0.0, 0.0])),
            ("gravity", PinValue::Vec3([0.0, -9.8, 0.0])),
        ],
        vec![floor],
        vec![],
    )
}

#[test]
fn collide_scene_lands_particles_on_the_scenes_colliders() {
    let mut app = app();
    app.insert_resource(SceneQueries::new(Floor));
    let e = spawn(&mut app, falling());
    app.world_mut().entity_mut(e).insert(Transform::from_xyz(0.0, 2.0, 0.0));
    frames(&mut app, 240);
    let runtime = app.world().get::<ParticleRuntime>(e).expect("runtime");
    let p = &runtime.sim.emitters[0].particles;
    assert_eq!(p.len(), 20);
    // A Local emitter 2 m up: the floor is 2 m below its origin.
    for q in &p.position {
        assert!((q.y + 2.0).abs() < 0.01, "not on the floor: {q}");
    }
}

#[test]
fn without_a_scene_service_collide_scene_hits_nothing() {
    let mut app = app();
    let e = spawn(&mut app, falling());
    frames(&mut app, 60);
    let runtime = app.world().get::<ParticleRuntime>(e).expect("runtime");
    assert!(runtime.sim.emitters[0].particles.position.iter().all(|q| q.y < -1.0), "falls freely");
}
