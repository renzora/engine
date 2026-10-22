//! The features that brought the particle system level with Hanabi: the
//! Gravity Well, Orbit and Conform To Sphere forces (and the Attractor's step
//! cap), Collide Scene, and the Parameter node scripts set by name. Each is
//! held to what it promises: the formula, stability on a long frame, and the
//! GPU path where it has one.

use bevy::math::{Affine3A, Vec3, Vec4};

use crate::asset::*;
use crate::compile::{compile_graph, CompileErrorKind};
use crate::gpu::select::{choose, ActiveBackend, GpuCaps};
use crate::ir::{ColliderShape, Context, ValueType};
use crate::params::{keyed_pins, parameter_name, parameter_names, EmitterTracks, ParamTrack};
use crate::sim::scene::{ground_hits, SKIN};
use crate::sim::{EffectSim, FrameInput, SEEK_STEP};
use crate::tests::{eval_node, f, graph, out_node, sim_of, simple_effect, v3};

/// One particle at `position` with `velocity` and no gravity, with `force`
/// (node id 2) wired into acceleration.
fn one_particle(position: Vec3, velocity: Vec3, force: VfxNode) -> VfxAsset {
    simple_effect(
        &[
            ("spawn_rate", f(0.0)),
            ("burst_count", f(1.0)),
            ("lifetime", f(1000.0)),
            ("position", v3(position.x, position.y, position.z)),
            ("velocity", v3(velocity.x, velocity.y, velocity.z)),
        ],
        vec![force],
        vec![Connection::new(2, "force", 1, "acceleration")],
    )
}

fn particle(sim: &EffectSim) -> (Vec3, Vec3) {
    let p = &sim.emitters[0].particles;
    (p.position[0], p.velocity[0])
}

fn force(node: VfxNode) -> Vec3 {
    eval_node(node, "force", ValueType::Vec3, Context::Update).truncate()
}

// ── Attractor ───────────────────────────────────────────────────────────────

#[test]
fn the_attractor_lands_on_its_point_instead_of_flying_past_it() {
    // 1 cm from a pull of 100 m/s² on a 0.1 s step: uncapped, the step would
    // carry it a metre past. Capped, it closes the distance exactly.
    let pull = VfxNode::new(2, "force/attractor").with("strength", f(100.0));
    let mut sim = sim_of(&one_particle(Vec3::new(0.01, 0.0, 0.0), Vec3::ZERO, pull));
    sim.step(0.1, &FrameInput::default());
    let (p, _) = particle(&sim);
    assert!(p.length() < 1e-4, "{p}");
    // A push from the very point still pushes: the cap is on pulls only.
    let push = VfxNode::new(2, "force/attractor").with("strength", f(-5.0));
    let mut sim = sim_of(&one_particle(Vec3::new(1e-3, 0.0, 0.0), Vec3::ZERO, push));
    sim.step(0.1, &FrameInput::default());
    assert!(particle(&sim).0.x > 1e-3 + 0.04, "{}", particle(&sim).0);
}

// ── Gravity Well ────────────────────────────────────────────────────────────

#[test]
fn the_gravity_well_is_inverse_square_softened_and_capped() {
    // A weak well, at rest: the step barely moves the particle, so the
    // average of the pull here and where the step lands is the pull here.
    let well = |pos: PinValue| {
        VfxNode::new(2, "force/gravity_well").with("position", pos).with("velocity", v3(0.0, 0.0, 0.0)).with("strength", f(0.1))
    };
    // Far out it is the inverse square: a quarter of the 1 m pull at 2 m.
    let g = force(well(v3(2.0, 0.0, 0.0)).with("softening", f(0.0)));
    assert!(g.abs_diff_eq(Vec3::new(-0.025, 0.0, 0.0), 1e-5), "{g}");
    // Softened, Plummer's formula: 0.1 * 2 / (4 + 1)^1.5 at e = 1.
    let g = force(well(v3(2.0, 0.0, 0.0)).with("softening", f(1.0)));
    assert!((g.x + 0.2 / 5f32.powf(1.5)).abs() < 1e-5, "{g}");
    // Nothing at the centre, and nothing infinite next to it.
    assert_eq!(force(well(v3(0.0, 0.0, 0.0))), Vec3::ZERO);
    let near = force(well(v3(1e-4, 0.0, 0.0)));
    assert!(near.is_finite() && near.length() < 1.0, "{near}");
    // Max Force holds the size; a negative strength pushes.
    let capped = force(well(v3(0.3, 0.0, 0.0)).with("softening", f(0.0)).with("max_force", f(0.5)));
    assert!((capped.length() - 0.5).abs() < 1e-4, "{capped}");
    let push = force(well(v3(2.0, 0.0, 0.0)).with("strength", f(-10.0)).with("softening", f(0.0)));
    assert!(push.x > 0.0, "{push}");
}

#[test]
fn a_particle_falling_through_a_gravity_well_never_slingshots() {
    // A strong, barely softened well and long frames: the textbook way to
    // fling a particle out at many times its starting distance. Released at
    // rest 1 m out it must stay within reach of where it started.
    for dt in [1.0 / 120.0, 1.0 / 30.0, 0.1] {
        let well = VfxNode::new(2, "force/gravity_well").with("strength", f(200.0)).with("softening", f(0.02));
        let mut sim = sim_of(&one_particle(Vec3::X, Vec3::ZERO, well));
        let mut farthest: f32 = 0.0;
        for _ in 0..(10.0 / dt) as usize {
            sim.step(dt, &FrameInput::default());
            let (p, v) = particle(&sim);
            assert!(p.is_finite() && v.is_finite());
            farthest = farthest.max(p.length());
        }
        assert!(farthest < 1.05, "slung out to {farthest} m at dt {dt}");
    }
    // And a well the step can resolve (the defaults) is not a sink either:
    // the particle falls through and climbs most of the way back, again and
    // again, as a particle in a real well does.
    let well = VfxNode::new(2, "force/gravity_well");
    let mut sim = sim_of(&one_particle(Vec3::X, Vec3::ZERO, well));
    let mut late_farthest: f32 = 0.0;
    for i in 0..600 {
        sim.step(SEEK_STEP, &FrameInput::default());
        if i >= 480 {
            late_farthest = late_farthest.max(particle(&sim).0.length());
        }
    }
    assert!((0.8..1.05).contains(&late_farthest), "after ten seconds it swings out to {late_farthest} m");
}

// ── Orbit ───────────────────────────────────────────────────────────────────

/// Distance from the Y axis and angle around it.
fn polar(p: Vec3) -> (f32, f32) {
    (Vec3::new(p.x, 0.0, p.z).length(), p.z.atan2(p.x))
}

#[test]
fn an_orbit_circles_the_axis_at_its_speed_and_holds_the_distance() {
    // Grip high: on speed within a step. At 1 rad/s and 2 m out, five
    // seconds later it is still 2 m out and has gone five radians round.
    let orbit = VfxNode::new(2, "force/orbit").with("speed", f(1.0)).with("grip", f(200.0));
    let mut sim = sim_of(&one_particle(Vec3::new(2.0, 0.0, 0.0), Vec3::ZERO, orbit));
    let mut turned = 0.0;
    let mut last = 0.0;
    for _ in 0..300 {
        sim.step(SEEK_STEP, &FrameInput::default());
        let (r, a) = polar(particle(&sim).0);
        assert!((r - 2.0).abs() < 0.08, "drifted to {r} m");
        let mut da = a - last;
        if da < -std::f32::consts::PI {
            da += std::f32::consts::TAU;
        } else if da > std::f32::consts::PI {
            da -= std::f32::consts::TAU;
        }
        turned += da;
        last = a;
    }
    // cross(+Y, +X) is -Z: the right-handed way round +Y, angle decreasing.
    assert!((turned.abs() - 5.0).abs() < 0.25, "turned {turned} rad");
    let speed = particle(&sim).1.length();
    assert!((speed - 2.0).abs() < 0.1, "{speed} m/s");
}

#[test]
fn radial_pull_draws_an_orbit_onto_its_ring_without_ringing() {
    let orbit = VfxNode::new(2, "force/orbit")
        .with("speed", f(2.0))
        .with("radius", f(1.5))
        .with("radial_pull", f(9.0));
    let mut sim = sim_of(&one_particle(Vec3::new(3.0, 0.0, 0.0), Vec3::ZERO, orbit));
    let mut closest = f32::INFINITY;
    for _ in 0..360 {
        sim.step(SEEK_STEP, &FrameInput::default());
        closest = closest.min(polar(particle(&sim).0).0);
    }
    let (r, _) = polar(particle(&sim).0);
    assert!((r - 1.5).abs() < 0.05, "settled at {r} m");
    // Critically damped: it never dives far inside the ring on the way.
    assert!(closest > 1.3, "dived to {closest} m");
}

#[test]
fn an_orbit_is_stable_on_huge_frames_and_stiff_settings() {
    let orbit = VfxNode::new(2, "force/orbit")
        .with("speed", f(20.0))
        .with("grip", f(1000.0))
        .with("radius", f(1.0))
        .with("radial_pull", f(10_000.0));
    let mut sim = sim_of(&one_particle(Vec3::new(4.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 30.0), orbit));
    for _ in 0..200 {
        sim.step(0.25, &FrameInput::default());
        let (p, v) = particle(&sim);
        assert!(p.is_finite() && v.is_finite());
        assert!(p.length() < 50.0, "flung to {p}");
    }
}

// ── Conform To Sphere ───────────────────────────────────────────────────────

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `ConformToSphereModifier`'s WGSL (bevy_hanabi `modifier/force.rs`), in
/// Rust: the velocity change one step makes.
#[allow(clippy::too_many_arguments)]
fn hanabi_conform(p: Vec3, v: Vec3, c: Vec3, r: f32, influence: f32, accel: f32, max_speed: f32, sticky: f32, shell: f32, dt: f32) -> Vec3 {
    let rel = c - p;
    let dir = rel.normalize_or_zero();
    let sd = rel.length() - r;
    if sd > influence {
        return Vec3::ZERO;
    }
    let current = v.dot(dir);
    let shell_f = smoothstep(0.0, shell, sd.abs());
    let target = sd.signum() * shell_f * max_speed;
    let delta = target - current;
    let sticky_a = accel * sticky;
    let conforming = sticky_a + (accel - sticky_a) * shell_f;
    delta.signum() * delta.abs().min(dt * conforming) * dir
}

#[test]
fn conform_to_sphere_makes_hanabis_velocity_change_exactly() {
    let dt = 0.1; // the test harness's step
    let cases = [
        (Vec3::new(2.5, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)),
        (Vec3::new(0.0, 1.05, 0.0), Vec3::new(3.0, -2.0, 0.0)),
        (Vec3::new(0.2, 0.3, -0.4), Vec3::new(-1.0, 0.0, 0.5)),
        (Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, 4.0)),
        (Vec3::new(9.0, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0)),
    ];
    for (p, v) in cases {
        let node = VfxNode::new(2, "force/conform_sphere")
            .with("position", v3(p.x, p.y, p.z))
            .with("velocity", v3(v.x, v.y, v.z))
            .with("center", v3(0.0, 0.0, 0.0))
            .with("radius", f(1.0))
            .with("influence", f(3.0))
            .with("attraction", f(5.0))
            .with("max_speed", f(2.0))
            .with("sticky", f(2.0))
            .with("shell", f(0.1));
        let got = force(node) * dt;
        let want = hanabi_conform(p, v, Vec3::ZERO, 1.0, 3.0, 5.0, 2.0, 2.0, 0.1, dt);
        assert!(got.abs_diff_eq(want, 1e-5), "at {p} moving {v}: {got} vs Hanabi's {want}");
    }
}

#[test]
fn conformed_particles_reach_the_surface_and_slide_around_it() {
    let node = VfxNode::new(2, "force/conform_sphere")
        .with("radius", f(1.0))
        .with("attraction", f(8.0))
        .with("max_speed", f(3.0));
    // Off to one side and drifting sideways: drawn in, then held on the
    // shell while it keeps travelling round.
    let mut sim = sim_of(&one_particle(Vec3::new(2.5, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.6), node));
    for _ in 0..240 {
        sim.step(SEEK_STEP, &FrameInput::default());
    }
    let mut start = particle(&sim).0;
    for _ in 0..120 {
        sim.step(SEEK_STEP, &FrameInput::default());
        let (p, _) = particle(&sim);
        assert!((p.length() - 1.0).abs() < 0.12, "off the shell at {}", p.length());
    }
    let (end, v) = particle(&sim);
    assert!(end.distance(start) > 0.3, "it should keep sliding: {start} to {end}");
    start = end;
    assert!(v.dot(start.normalize()).abs() < 0.5, "moving along the surface, not through it: {v}");
}

#[test]
fn conform_to_sphere_leaves_particles_beyond_its_influence_alone() {
    let node = VfxNode::new(2, "force/conform_sphere").with("position", v3(5.0, 0.0, 0.0)).with("influence", f(3.0));
    assert_eq!(force(node), Vec3::ZERO);
    // Inside the sphere always counts, however small the influence.
    let inside = VfxNode::new(2, "force/conform_sphere")
        .with("position", v3(0.3, 0.0, 0.0))
        .with("velocity", v3(0.0, 0.0, 0.0))
        .with("influence", f(0.0));
    assert!(force(inside).x > 0.0, "pushed out toward the surface");
}

// ── GPU ─────────────────────────────────────────────────────────────────────

#[test]
fn the_new_forces_run_on_the_gpu_and_generate_valid_wgsl() {
    for t in ["force/gravity_well", "force/orbit", "force/conform_sphere", "force/attractor"] {
        let mut asset = one_particle(Vec3::X, Vec3::ZERO, VfxNode::new(2, t));
        asset.emitters[0].backend = Backend::Gpu;
        let e = &asset.emitters[0];
        let compiled = compile_graph(&e.graph).unwrap();
        assert_eq!(choose(e, &compiled, false, &GpuCaps::all()).active, ActiveBackend::Gpu, "{t}");
        let p = crate::gpu::codegen::generate(&compiled, SimulationSpace::Local).unwrap();
        renzora::wgsl::check(&p.source).unwrap_or_else(|e| panic!("{t}:\n{e}"));
    }
}

// ── Collide Scene ───────────────────────────────────────────────────────────

/// One frame: step, then the scene pass against a floor at `floor`.
fn scene_frame(sim: &mut EffectSim, frame: &FrameInput, dt: f32, floor: f32, budget: usize) -> usize {
    sim.step(dt, frame);
    scene_pass(sim, frame, floor, budget)
}

/// The runtime's scene pass, with a floor for the scene. How many it cast.
fn scene_pass(sim: &mut EffectSim, frame: &FrameInput, floor: f32, budget: usize) -> usize {
    let (mut rays, mut probes, mut budget) = (Vec::new(), Vec::new(), budget);
    sim.scene_rays(&frame.transform, false, &mut budget, &mut rays, &mut probes);
    let hits = ground_hits(&rays, floor);
    sim.apply_scene_hits(&frame.transform, &probes, &hits);
    rays.len()
}

fn falling(collider: VfxNode, count: f32, space: SimulationSpace) -> VfxAsset {
    let mut asset = simple_effect(
        &[
            ("spawn_rate", f(0.0)),
            ("burst_count", f(count)),
            ("lifetime", f(100.0)),
            ("position", v3(0.0, 1.0, 0.0)),
            ("velocity", v3(1.0, 0.0, 0.0)),
            ("gravity", v3(0.0, -9.8, 0.0)),
        ],
        vec![collider],
        vec![],
    );
    asset.emitters[0].space = space;
    asset
}

#[test]
fn scene_collision_bounces_settles_and_slides_to_a_stop() {
    let node = VfxNode::new(2, "collide/scene").with("bounce", f(0.5)).with("friction", f(0.4));
    let mut sim = sim_of(&falling(node, 1.0, SimulationSpace::Local));
    let frame = FrameInput::default();
    let mut bounced = false;
    for _ in 0..600 {
        scene_frame(&mut sim, &frame, 1.0 / 60.0, 0.0, 64);
        let (p, v) = particle(&sim);
        assert!(p.y >= -1e-4, "fell through the scene: {p}");
        bounced |= v.y > 0.5;
    }
    assert!(bounced, "never bounced");
    let (p, v) = particle(&sim);
    assert!((p.y - SKIN).abs() < 1e-3, "resting on the floor: {p}");
    assert!(v.length() < 1e-2, "stopped by friction: {v}");
}

#[test]
fn a_scene_kill_removes_the_particle_and_raises_its_death() {
    let node = VfxNode::new(2, "collide/scene").with("kill", PinValue::Bool(true));
    let mut sim = sim_of(&falling(node, 1.0, SimulationSpace::Local));
    let frame = FrameInput::default();
    for _ in 0..20 {
        scene_frame(&mut sim, &frame, 1.0 / 60.0, 0.0, 64);
    }
    assert_eq!(sim.alive(), 1, "still falling");
    for _ in 0..30 {
        scene_frame(&mut sim, &frame, 1.0 / 60.0, 0.0, 64);
    }
    assert_eq!(sim.alive(), 0, "killed on reaching the floor");
}

#[test]
fn scene_casts_are_capped_and_taken_in_turns_without_tunnelling() {
    // Ten particles, three casts a frame: everyone gets a turn within four
    // frames, and one that waited is swept over its whole path, so a floor
    // crossed between turns still stops it.
    let node = VfxNode::new(2, "collide/scene").with("bounce", f(0.0));
    let mut sim = sim_of(&falling(node, 10.0, SimulationSpace::Local));
    let frame = FrameInput::default();
    let mut casts = Vec::new();
    for _ in 0..90 {
        casts.push(scene_frame(&mut sim, &frame, 1.0 / 60.0, 0.0, 3));
    }
    assert!(casts.iter().all(|&n| n <= 3), "{casts:?}");
    // Between turns a waiting particle may be under the floor for a frame or
    // two; once every one has had its turn none is left there.
    for _ in 0..4 {
        scene_pass(&mut sim, &frame, 0.0, 3);
    }
    let p = &sim.emitters[0].particles;
    assert!(p.position.iter().all(|q| q.y >= -1e-4), "{:?}", p.position);
}

#[test]
fn scene_collision_works_through_the_entitys_transform() {
    // A Local emitter on an entity standing 5 m up meets the world floor 5 m
    // below its own origin; World space is already in world coordinates.
    let up = FrameInput::at(Affine3A::from_translation(Vec3::new(0.0, 5.0, 0.0)));
    for space in [SimulationSpace::Local, SimulationSpace::World] {
        let node = VfxNode::new(2, "collide/scene").with("bounce", f(0.0));
        let mut sim = sim_of(&falling(node, 1.0, space));
        for _ in 0..240 {
            scene_frame(&mut sim, &up, 1.0 / 60.0, 0.0, 64);
        }
        let p = particle(&sim).0;
        let world_y = match space {
            SimulationSpace::Local => p.y + 5.0,
            SimulationSpace::World => p.y,
        };
        assert!((world_y - SKIN).abs() < 1e-3, "{space:?}: at {world_y}");
    }
}

#[test]
fn a_seek_starts_the_scenes_watch_from_where_the_replay_left_particles() {
    let node = VfxNode::new(2, "collide/scene");
    let mut sim = sim_of(&falling(node, 1.0, SimulationSpace::Local));
    sim.simulate_to(0.2, &FrameInput::default());
    let p = &sim.emitters[0].particles;
    assert_eq!(p.scene[0].from, p.position[0]);
}

#[test]
fn collide_scene_keeps_its_emitter_on_the_cpu_and_says_why() {
    let mut asset = falling(VfxNode::new(2, "collide/scene"), 1.0, SimulationSpace::Local);
    asset.emitters[0].backend = Backend::Gpu;
    let e = &asset.emitters[0];
    let compiled = compile_graph(&e.graph).unwrap();
    assert_eq!(compiled.colliders.len(), 1);
    assert_eq!(compiled.colliders[0].shape, ColliderShape::Scene);
    let choice = choose(e, &compiled, false, &GpuCaps::all());
    assert_eq!(choice.active, ActiveBackend::Cpu);
    assert!(choice.fell_back);
    assert!(choice.reason.unwrap().contains("collides with the scene"));
    // A shader generated anyway still validates; it just has no scene.
    let p = crate::gpu::codegen::generate(&compiled, SimulationSpace::Local).unwrap();
    renzora::wgsl::check(&p.source).unwrap_or_else(|e| panic!("{e}"));
    // An emitter without the node tracks nothing.
    let plain = sim_of(&simple_effect(&[], vec![], vec![]));
    assert!(!plain.wants_scene());
}

// ── Parameters ──────────────────────────────────────────────────────────────

fn parameter(id: NodeId, name: &str, value: PinValue) -> VfxNode {
    VfxNode::new(id, "input/parameter").with("name", PinValue::Text(name.into())).with("value", value)
}

#[test]
fn a_parameter_reads_its_value_until_a_script_sets_it() {
    let rate = parameter(2, "rate", f(0.0));
    let asset = simple_effect(&[("lifetime", f(100.0))], vec![rate], vec![Connection::new(2, "value", 1, "spawn_rate")]);
    let mut sim = sim_of(&asset);
    let frame = FrameInput::default();
    for _ in 0..60 {
        sim.step(SEEK_STEP, &frame);
    }
    assert_eq!(sim.alive(), 0, "its own value is 0 per second");
    assert!(sim.set_parameter("rate", Some(Vec4::splat(120.0))));
    assert!(!sim.set_parameter("nope", Some(Vec4::ONE)), "an unknown name is reported");
    for _ in 0..60 {
        sim.step(SEEK_STEP, &frame);
    }
    let alive = sim.alive();
    assert!((115..=121).contains(&alive), "{alive}");
    sim.set_parameter("rate", None);
    for _ in 0..60 {
        sim.step(SEEK_STEP, &frame);
    }
    assert_eq!(sim.alive(), alive, "cleared, it is back to 0 per second");
}

#[test]
fn a_parameter_takes_vectors_and_colours_and_every_node_of_its_name() {
    let mut asset = simple_effect(
        &[("spawn_rate", f(0.0)), ("burst_count", f(1.0)), ("lifetime", f(100.0))],
        vec![parameter(2, "launch", v3(0.0, 0.0, 0.0)), parameter(3, "tint", PinValue::Color([1.0; 4]))],
        vec![Connection::new(2, "value", 1, "velocity"), Connection::new(3, "value", 1, "color")],
    );
    let mut second = asset.emitters[0].clone();
    second.name = "second".into();
    asset.emitters.push(second);
    let mut sim = sim_of(&asset);
    sim.set_parameter("launch", Some(Vec3::new(1.0, 2.0, 3.0).extend(1.0)));
    sim.set_parameter("tint", Some(Vec4::new(0.5, 0.25, 1.0, 0.75)));
    sim.step(SEEK_STEP, &FrameInput::default());
    for e in &sim.emitters {
        assert_eq!(e.particles.velocity[0], Vec3::new(1.0, 2.0, 3.0), "{}", e.emitter_index);
        assert_eq!(e.particles.base_color[0], Vec4::new(0.5, 0.25, 1.0, 0.75));
    }
    assert_eq!(parameter_names(&asset), ["launch", "tint"]);
}

#[test]
fn a_script_value_wins_over_the_timeline_until_it_is_cleared() {
    let mut asset = simple_effect(&[("lifetime", f(100.0))], vec![parameter(2, "rate", f(0.0))], vec![Connection::new(2, "value", 1, "spawn_rate")]);
    let mut track = ParamTrack::pin(0, 2, "value");
    track.upsert(0.0, f(30.0));
    asset.tracks.push(track);
    // The keyed pin and the parameter are one slot, not two.
    assert_eq!(keyed_pins(&asset, 0), vec![(2, "value".to_string())]);
    let tracks = EmitterTracks::from_asset(&asset, 0);
    assert_eq!(tracks.names, vec![Some("rate".to_string())]);
    let mut out = Vec::new();
    tracks.fill(0.5, &mut out);
    assert_eq!(out[0].x, 30.0, "the keys play");
    let mut tracks = tracks;
    tracks.set_parameter("rate", Some(Vec4::splat(5.0)));
    tracks.fill(0.5, &mut out);
    assert_eq!(out[0].x, 5.0, "the script wins");
    tracks.set_parameter("rate", None);
    tracks.fill(0.5, &mut out);
    assert_eq!(out[0].x, 30.0, "and gives it back");
}

#[test]
fn a_gpu_step_carries_the_script_value() {
    let mut asset = simple_effect(&[("spawn_rate", f(10.0))], vec![parameter(2, "push", v3(0.0, 0.0, 0.0))], vec![Connection::new(2, "value", 1, "acceleration")]);
    asset.emitters[0].backend = Backend::Gpu;
    let (mut sim, _) = EffectSim::with_backends(&asset, &GpuCaps::all());
    assert!(sim.emitters[0].is_gpu());
    sim.set_parameter("push", Some(Vec4::new(0.0, 4.0, 0.0, 1.0)));
    sim.step(SEEK_STEP, &FrameInput::default());
    let plan = *sim.emitters[0].gpu.as_ref().unwrap().plans.last().unwrap();
    assert_eq!(plan.params[0], Vec4::new(0.0, 4.0, 0.0, 1.0));
    let compiled = compile_graph(&asset.emitters[0].graph).unwrap();
    let keyed = keyed_pins(&asset, 0);
    let with = crate::compile::compile_graph_with_params(&asset.emitters[0].graph, &keyed).unwrap();
    let p = crate::gpu::codegen::generate(&with, SimulationSpace::Local).unwrap();
    assert!(p.source.contains("step.params[0]"), "the parameter reads its slot");
    renzora::wgsl::check(&p.source).unwrap_or_else(|e| panic!("{e}"));
    // Compiled without parameters (the editor's checker), it is its value.
    assert!(compiled.update.instrs.iter().all(|i| !matches!(i.op, crate::ir::Op::Param(_))));
}

#[test]
fn parameter_names_trim_and_default_and_the_name_round_trips() {
    assert_eq!(parameter_name(&parameter(2, "  glow ", f(1.0))), Some("glow"));
    assert_eq!(parameter_name(&parameter(2, "   ", f(1.0))), None);
    assert_eq!(parameter_name(&VfxNode::new(2, "input/parameter")), Some("param"), "the catalog default");
    assert_eq!(parameter_name(&VfxNode::new(2, "math/float")), None);
    let asset = simple_effect(&[], vec![parameter(2, "glow", f(1.0))], vec![]);
    let text = write_vfx(&asset).unwrap();
    assert!(text.contains("Text(\"glow\")"), "{text}");
    assert_eq!(parse_vfx(&text).unwrap(), asset);
    // A name that is not text is a bad value, named on the node.
    let bad = VfxNode::new(2, "input/parameter").with("name", f(3.0));
    let err = compile_graph(&graph(vec![out_node(), bad], vec![])).unwrap_err();
    assert!(err.iter().any(|e| e.node == Some(2) && matches!(e.kind, CompileErrorKind::BadValue(_))), "{err:?}");
}
