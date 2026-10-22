//! Unit tests for the emitter features that act around the graph rather than
//! in it: multi-octave curl noise and the speed nodes, collisions, trails and
//! ribbon meshes, light from particles, and sub-emitters. Shared helpers come
//! from the main test module.

use bevy::math::{Affine3A, Vec2, Vec3, Vec4};

use crate::asset::*;
use crate::ir::{Context, Op, TernaryOp, ValueType};
use crate::light::{aggregate, flicker, weight, LightRig, LightSample};
use crate::sim::collide::{collide, ColliderInput, Hit};
use crate::sim::sub::{accepted_rules, interval_events, MAX_SUB_DEPTH};
use crate::sim::{FrameInput, ParticleBuffer, SEEK_STEP};
use crate::tests::{eval_node, f, graph, layer, out_node, realistic_fire, sim_of, simple_effect, steps, v3};

// ── Noise and speed ─────────────────────────────────────────────────────────

#[test]
fn one_octave_of_curl_noise_is_the_plain_curl_and_more_add_finer_swirls() {
    let p = Vec3::new(0.37, 1.91, -2.4);
    let plain = crate::noise::curl(p);
    assert_eq!(crate::noise::curl_fbm(p, 1.0, 2.0, 0.5), plain);
    assert_eq!(crate::noise::curl_fbm(p, 0.0, 2.0, 0.5), Vec3::ZERO);
    // Two octaves: the second at twice the frequency and half the amplitude.
    let two = crate::noise::curl_fbm(p, 2.0, 2.0, 0.5);
    let second = two - plain;
    let half = crate::noise::curl_fbm(p, 1.5, 2.0, 0.5) - plain;
    assert!(second.length() > 0.0);
    assert!(half.abs_diff_eq(second * 0.5, 1e-5), "a fractional octave fades the last one in");
    // Persistence scales it, lacunarity moves it.
    let loud = crate::noise::curl_fbm(p, 2.0, 2.0, 1.0) - plain;
    assert!(loud.abs_diff_eq(second * 2.0, 1e-5));
    assert_ne!(crate::noise::curl_fbm(p, 2.0, 3.0, 0.5), two);
    // Capped, and never NaN on nonsense.
    assert_eq!(crate::noise::curl_fbm(p, 50.0, 2.0, 0.5), crate::noise::curl_fbm(p, 8.0, 2.0, 0.5));
    assert!(crate::noise::curl_fbm(p, f32::NAN, 2.0, 0.5).is_finite());

    // Still divergence-free: a sum of curls is a curl.
    let h = 1e-2;
    let field = |q: Vec3| crate::noise::curl_fbm(q, 3.0, 2.0, 0.5);
    let d = |axis: Vec3| (field(p + axis * h) - field(p - axis * h)) / (2.0 * h);
    let div = d(Vec3::X).x + d(Vec3::Y).y + d(Vec3::Z).z;
    assert!(div.abs() < 0.1 * field(p).length().max(1.0), "divergence {div}");
}

#[test]
fn the_curl_node_compiles_one_octave_as_before_and_more_as_a_fractal() {
    let compile = |octaves: Option<f32>| {
        let mut n = VfxNode::new(2, "force/curl_noise");
        if let Some(o) = octaves {
            n = n.with("octaves", f(o));
        }
        let g = graph(vec![out_node(), n], vec![Connection::new(2, "force", 1, "acceleration")]);
        crate::compile::compile_graph(&g).expect("compiles")
    };
    let has_fbm = |c: &crate::ir::CompiledEmitter| {
        c.update.instrs.iter().any(|i| matches!(i.op, Op::Ternary(TernaryOp::CurlFbm, ..)))
    };
    assert!(!has_fbm(&compile(None)));
    assert!(!has_fbm(&compile(Some(1.0))));
    assert!(has_fbm(&compile(Some(3.0))));
    // Through the node: three octaves at the defaults are curl_fbm exactly.
    let node = VfxNode::new(2, "force/curl_noise")
        .with("position", v3(0.3, 0.7, -1.1))
        .with("frequency", f(1.0))
        .with("octaves", f(3.0));
    let got = eval_node(node, "force", ValueType::Vec3, Context::Update).truncate();
    assert!(got.abs_diff_eq(crate::noise::curl_fbm(Vec3::new(0.3, 0.7, -1.1), 3.0, 2.0, 0.5), 1e-5));
}

#[test]
fn speed_limit_brakes_only_the_excess_implicitly() {
    let u = Context::Update;
    // |v| = 10 against a limit of 4 at rate 8 and dt 0.1: k / (1 + k dt) is
    // 8 / 1.8, times the 6 m/s of excess, straight back along v.
    let node = VfxNode::new(2, "force/speed_limit")
        .with("velocity", v3(10.0, 0.0, 0.0))
        .with("limit", f(4.0))
        .with("rate", f(8.0));
    let force = eval_node(node, "force", ValueType::Vec3, u).truncate();
    assert!(force.abs_diff_eq(Vec3::new(-6.0 * 8.0 / 1.8, 0.0, 0.0), 1e-3), "{force}");
    let slow = VfxNode::new(2, "force/speed_limit").with("velocity", v3(1.0, 2.0, 0.0)).with("limit", f(4.0));
    assert_eq!(eval_node(slow, "force", ValueType::Vec3, u).truncate(), Vec3::ZERO);
}

#[test]
fn drag_is_linear_plus_quadratic_and_stable() {
    let u = Context::Update;
    // |v| = 5: k = 1 + 0.5 * 5 = 3.5, applied as 3.5 / (1 + 0.35).
    let node = VfxNode::new(2, "force/drag")
        .with("velocity", v3(3.0, 0.0, 4.0))
        .with("linear", f(1.0))
        .with("quadratic", f(0.5));
    let force = eval_node(node, "force", ValueType::Vec3, u).truncate();
    let k = 3.5 / 1.35;
    assert!(force.abs_diff_eq(Vec3::new(-3.0, 0.0, -4.0) * k, 1e-3), "{force}");
    // However stiff, one step never reverses the velocity.
    let stiff = VfxNode::new(2, "force/drag").with("velocity", v3(50.0, 0.0, 0.0)).with("quadratic", f(100.0));
    let force = eval_node(stiff, "force", ValueType::Vec3, u).truncate();
    assert!(50.0 + force.x * 0.1 >= 0.0, "{force}");
}

#[test]
fn max_speed_caps_the_velocity_after_every_force() {
    let asset = simple_effect(
        &[
            ("spawn_rate", f(0.0)),
            ("burst_count", f(1.0)),
            ("lifetime", f(10.0)),
            ("velocity", v3(10.0, 0.0, 0.0)),
            ("gravity", v3(0.0, -50.0, 0.0)),
            ("max_speed", f(3.0)),
        ],
        vec![],
        vec![],
    );
    let mut sim = sim_of(&asset);
    sim.step(0.1, &FrameInput::default());
    let p = &sim.emitters[0].particles;
    assert!((p.velocity[0].length() - 3.0).abs() < 1e-4, "{}", p.velocity[0]);
    assert!((p.position[0].length() - 0.3).abs() < 1e-4, "the step itself obeys the cap");
}

// ── Collisions ──────────────────────────────────────────────────────────────

fn plane(bounce: f32, friction: f32, kill: bool) -> ColliderInput {
    ColliderInput {
        shape: crate::ir::ColliderShape::Plane,
        contain: false,
        kill,
        a: Vec4::ZERO,
        b: Vec4::new(0.0, 1.0, 0.0, 0.0),
        bounce,
        friction,
    }
}

#[test]
fn a_plane_bounces_by_restitution_and_brakes_by_coulomb_friction() {
    let c = plane(0.5, 0.2, false);
    let (p0, mut p1, mut v) = (Vec3::new(0.0, 0.05, 0.0), Vec3::new(0.05, -0.05, 0.0), Vec3::new(1.0, -2.0, 0.0));
    assert_eq!(collide(&c, p0, &mut p1, &mut v, Vec3::ZERO), Hit::Bounced);
    assert!(p1.abs_diff_eq(Vec3::new(0.05, 0.0, 0.0), 1e-6), "back on the plane: {p1}");
    // Normal: -2 out at half, +1. Tangent: friction 0.2 times the 3 m/s the
    // hit took out of the normal, off the 1 m/s of slide.
    assert!(v.abs_diff_eq(Vec3::new(0.4, 1.0, 0.0), 1e-5), "{v}");
    // Above the plane nothing happens; a zero normal is no plane.
    let (mut q, mut w) = (Vec3::new(0.0, 1.0, 0.0), Vec3::NEG_Y);
    assert_eq!(collide(&c, Vec3::Y * 2.0, &mut q, &mut w, Vec3::ZERO), Hit::None);
    let degenerate = ColliderInput { b: Vec4::ZERO, ..c };
    let mut q = Vec3::new(0.0, -1.0, 0.0);
    assert_eq!(collide(&degenerate, Vec3::ZERO, &mut q, &mut w, Vec3::ZERO), Hit::None);
    // Kill on hit reports it and leaves the velocity alone.
    let (mut q, mut w) = (Vec3::new(0.0, -0.1, 0.0), Vec3::NEG_Y);
    assert_eq!(collide(&plane(0.5, 0.2, true), Vec3::Y, &mut q, &mut w, Vec3::ZERO), Hit::Killed);
}

#[test]
fn solid_shapes_catch_a_particle_that_crosses_them_in_one_step() {
    let sphere = ColliderInput {
        shape: crate::ir::ColliderShape::Sphere,
        contain: false,
        kill: false,
        a: Vec4::ZERO,
        b: Vec4::new(0.5, 0.0, 0.0, 0.0),
        bounce: 1.0,
        friction: 0.0,
    };
    // From x = -2 to x = +2 in one step: straight through the ball.
    let (mut p1, mut v) = (Vec3::new(2.0, 0.0, 0.0), Vec3::new(40.0, 0.0, 0.0));
    assert_eq!(collide(&sphere, Vec3::new(-2.0, 0.0, 0.0), &mut p1, &mut v, Vec3::ZERO), Hit::Bounced);
    assert!(p1.abs_diff_eq(Vec3::new(-0.5, 0.0, 0.0), 1e-4), "stopped at the entry point: {p1}");
    assert!(v.abs_diff_eq(Vec3::new(-40.0, 0.0, 0.0), 1e-3), "and bounced: {v}");

    let cube = ColliderInput { shape: crate::ir::ColliderShape::Box, b: Vec4::new(1.0, 1.0, 1.0, 0.0), ..sphere };
    let (mut p1, mut v) = (Vec3::new(0.0, -3.0, 0.0), Vec3::new(0.0, -60.0, 0.0));
    assert_eq!(collide(&cube, Vec3::new(0.0, 3.0, 0.0), &mut p1, &mut v, Vec3::ZERO), Hit::Bounced);
    assert!(p1.abs_diff_eq(Vec3::new(0.0, 0.5, 0.0), 1e-4), "entered through the top: {p1}");
    assert!(v.y > 0.0);
    // Born inside: pushed out of the nearest face.
    let (mut p1, mut v) = (Vec3::new(0.0, 0.0, 0.45), Vec3::ZERO);
    assert_eq!(collide(&cube, Vec3::new(0.0, 0.0, 0.45), &mut p1, &mut v, Vec3::ZERO), Hit::Bounced);
    assert!((p1.z - 0.5).abs() < 1e-5, "{p1}");
}

#[test]
fn containing_shapes_keep_particles_in() {
    let bowl = ColliderInput {
        shape: crate::ir::ColliderShape::Sphere,
        contain: true,
        kill: false,
        a: Vec4::ZERO,
        b: Vec4::new(1.0, 0.0, 0.0, 0.0),
        bounce: 1.0,
        friction: 0.0,
    };
    let (mut p1, mut v) = (Vec3::new(1.5, 0.0, 0.0), Vec3::new(5.0, 0.0, 0.0));
    assert_eq!(collide(&bowl, Vec3::new(0.9, 0.0, 0.0), &mut p1, &mut v, Vec3::ZERO), Hit::Bounced);
    assert!((p1.length() - 1.0).abs() < 1e-5 && v.x < 0.0, "{p1} {v}");

    let room = ColliderInput { shape: crate::ir::ColliderShape::Box, b: Vec4::new(2.0, 2.0, 2.0, 0.0), ..bowl };
    // Out through a corner: both walls answer.
    let (mut p1, mut v) = (Vec3::new(1.2, -1.3, 0.0), Vec3::new(3.0, -3.0, 0.0));
    assert_eq!(collide(&room, Vec3::ZERO, &mut p1, &mut v, Vec3::ZERO), Hit::Bounced);
    assert!(p1.abs_diff_eq(Vec3::new(1.0, -1.0, 0.0), 1e-5) && v.x < 0.0 && v.y > 0.0, "{p1} {v}");
    let kill_room = ColliderInput { kill: true, ..room };
    let mut p1 = Vec3::new(0.0, 5.0, 0.0);
    assert_eq!(collide(&kill_room, Vec3::ZERO, &mut p1, &mut v, Vec3::ZERO), Hit::Killed);
}

fn falling_onto(plane_node: VfxNode, space: SimulationSpace) -> VfxAsset {
    let mut asset = simple_effect(
        &[
            ("spawn_rate", f(0.0)),
            ("burst_count", f(1.0)),
            ("lifetime", f(100.0)),
            ("position", v3(0.0, 1.0, 0.0)),
            ("velocity", v3(1.0, 0.0, 0.0)),
            ("gravity", v3(0.0, -9.8, 0.0)),
        ],
        vec![plane_node],
        vec![],
    );
    asset.emitters[0].space = space;
    asset
}

#[test]
fn a_particle_bounces_settles_and_slides_to_a_stop_on_the_ground() {
    let ground = VfxNode::new(2, "collide/plane").with("bounce", f(0.5)).with("friction", f(0.4));
    let mut sim = sim_of(&falling_onto(ground, SimulationSpace::Local));
    let frame = FrameInput::default();
    let mut bounced = false;
    for _ in 0..600 {
        sim.step(1.0 / 60.0, &frame);
        let p = &sim.emitters[0].particles;
        assert!(p.position[0].y >= -1e-5, "fell through: {}", p.position[0]);
        bounced |= p.velocity[0].y > 0.5;
    }
    assert!(bounced, "never bounced");
    let p = &sim.emitters[0].particles;
    assert!(p.position[0].y.abs() < 1e-3, "resting on the ground: {}", p.position[0]);
    assert!(p.velocity[0].length() < 1e-3, "stopped by friction: {}", p.velocity[0]);
}

#[test]
fn colliders_live_in_the_entity_space_for_world_emitters_too() {
    // A world-space particle released above an entity standing at y = 5
    // lands on the entity's ground, not the world's.
    let ground = VfxNode::new(2, "collide/plane").with("bounce", f(0.0));
    let mut sim = sim_of(&falling_onto(ground, SimulationSpace::World));
    let frame = FrameInput::at(Affine3A::from_translation(Vec3::new(0.0, 5.0, 0.0)));
    steps(&mut sim, 1.0 / 60.0, 120, &frame);
    let y = sim.emitters[0].particles.position[0].y;
    assert!((y - 5.0).abs() < 1e-3, "{y}");

    // A World Space plane is the world's floor instead: a local particle on
    // an entity at y = 5 falls five metres past its own origin to reach it.
    let floor = VfxNode::new(2, "collide/plane").with("bounce", f(0.0)).with("world", PinValue::Bool(true));
    let mut sim = sim_of(&falling_onto(floor, SimulationSpace::Local));
    steps(&mut sim, 1.0 / 60.0, 180, &frame);
    let y = sim.emitters[0].particles.position[0].y;
    assert!((y + 5.0).abs() < 1e-3, "{y}");
}

#[test]
fn a_kill_collider_removes_the_particle() {
    let ground = VfxNode::new(2, "collide/plane").with("kill", PinValue::Bool(true));
    let mut sim = sim_of(&falling_onto(ground, SimulationSpace::Local));
    // From 1 m up at 9.8 m/s^2 it lands at about 0.45 s.
    steps(&mut sim, 1.0 / 60.0, 20, &FrameInput::default());
    assert_eq!(sim.alive(), 1, "still falling");
    steps(&mut sim, 1.0 / 60.0, 20, &FrameInput::default());
    assert_eq!(sim.alive(), 0, "killed on reaching the ground");
}

#[test]
fn collision_nodes_type_check_without_being_wired() {
    let bad = VfxNode::new(2, "collide/sphere").with("radius", PinValue::Vec3([1.0, 1.0, 1.0]));
    let err = crate::compile::compile_graph(&graph(vec![out_node(), bad], vec![])).expect_err("a Vec3 radius");
    assert!(err.iter().any(|e| e.node == Some(2)), "{err:?}");
    let ok = VfxNode::new(2, "collide/box").with("inside", PinValue::Bool(true));
    let c = crate::compile::compile_graph(&graph(vec![out_node(), ok], vec![])).expect("compiles");
    assert_eq!(c.colliders.len(), 1);
    assert!(c.colliders[0].contain && !c.colliders[0].kill);
}

// ── Trails and ribbons ──────────────────────────────────────────────────────

#[test]
fn a_trail_keeps_spaced_points_newest_first_in_a_bounded_ring() {
    let mut p = ParticleBuffer { trail_stride: 4, ..Default::default() };
    for (id, x) in [(0u32, 0.0f32), (1, 100.0)] {
        p.push(crate::sim::Birth {
            id,
            lifetime: 1.0,
            position: Vec3::new(x, 0.0, 0.0),
            velocity: Vec3::ZERO,
            size: Vec2::ONE,
            rotation: 0.0,
            angular_velocity: 0.0,
            color: Vec4::ONE,
            generation: 0,
        });
    }
    // Moving 0.04 a step against a spacing of 0.1: a point every third step.
    for k in 1..=30 {
        let x = 0.04 * k as f32;
        p.position[0].x = x;
        p.record_trail(0, Vec3::new(x, 0.0, 0.0), 0.1);
    }
    let kept: Vec<f32> = p.trail(0).map(|q| q.x).collect();
    assert_eq!(kept.len(), 4, "the ring holds four");
    assert!(kept.windows(2).all(|w| w[0] > w[1]), "newest first: {kept:?}");
    assert!(kept.windows(2).all(|w| w[0] - w[1] >= 0.1 - 1e-5), "spaced: {kept:?}");
    // Removing the first particle moves the second's ring with it.
    p.swap_remove(0);
    assert_eq!(p.trail(0).collect::<Vec<_>>(), vec![Vec3::new(100.0, 0.0, 0.0)]);
    assert_eq!(p.trail.len(), 4);
}

#[test]
fn ribbon_meshes_take_two_vertices_a_point_and_run_their_uvs_head_to_tail() {
    use bevy::mesh::{Mesh, VertexAttributeValues};
    let mut asset = simple_effect(
        &[
            ("spawn_rate", f(0.0)),
            ("burst_count", f(2.0)),
            ("lifetime", f(10.0)),
            ("velocity", v3(1.0, 0.0, 0.0)),
            ("size", PinValue::Vec2([0.2, 0.2])),
        ],
        vec![],
        vec![],
    );
    let render = &mut asset.emitters[0].render;
    render.facing = Facing::Ribbon;
    render.ribbon = Ribbon { points: 5, min_distance: 0.0, tail_width: 0.5, tail_alpha: 0.0, ..Default::default() };
    let render = render.clone();
    let mut sim = sim_of(&asset);
    // Born, then three steps: each keeps a point, so the trail holds four and
    // the strip is the live head plus the three older ones.
    steps(&mut sim, 0.1, 4, &FrameInput::default());
    let particles = &sim.emitters[0].particles;
    // The strip is the live head plus every kept point but the newest, which
    // is where the head already is: as many points as the trail holds.
    let m = particles.trail(0).count();
    assert_eq!(m, 5);
    let (verts, indices) = crate::render::ribbon::strip_size(m);
    let mut mesh = Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default());
    crate::render::write_mesh(&mut mesh, particles, &render, Affine3A::IDENTITY).expect("drawn");
    assert_eq!(mesh.count_vertices(), 2 * verts);
    assert_eq!(mesh.indices().map(|i| i.len()), Some(2 * indices));
    let Some(VertexAttributeValues::Float32x2(corners)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0) else { panic!() };
    assert_eq!(corners[0], [-0.5, -0.5], "head");
    assert_eq!(corners[verts - 1], [0.5, 0.5], "tail");
    let Some(VertexAttributeValues::Float32x4(params)) = mesh.attribute(crate::render::ATTRIBUTE_PARTICLE_PARAMS)
    else {
        panic!()
    };
    assert!((params[0][0] - 0.2).abs() < 1e-6 && (params[verts - 1][0] - 0.1).abs() < 1e-6, "tapered");
    let Some(VertexAttributeValues::Float32x4(colors)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR) else { panic!() };
    assert_eq!(colors[verts - 1][3], 0.0, "faded at the tail");

    // A newborn has no trail yet and draws nothing.
    let mut fresh = sim_of(&asset);
    fresh.burst(1, &FrameInput::default());
    assert!(crate::render::write_mesh(&mut mesh, &fresh.emitters[0].particles, &render, Affine3A::IDENTITY).is_none());
    assert_eq!(crate::render::ribbon::strip_size(1), (0, 0));
}

#[test]
fn a_connect_ribbon_strings_the_particles_into_its_groups() {
    use bevy::mesh::Mesh;
    let asset = simple_effect(
        &[("spawn_rate", f(0.0)), ("burst_count", f(6.0)), ("lifetime", f(10.0)), ("velocity", v3(0.0, 1.0, 0.0))],
        vec![VfxNode::new(2, "random/unit_vector")],
        vec![Connection::new(2, "value", 1, "position")],
    );
    let mut sim = sim_of(&asset);
    sim.step(1.0 / 60.0, &FrameInput::default());
    let particles = &sim.emitters[0].particles;
    assert_eq!(particles.len(), 6);
    let render = RenderSettings {
        facing: Facing::Ribbon,
        ribbon: Ribbon { kind: RibbonKind::Connect, groups: 2, ..Default::default() },
        ..Default::default()
    };
    let mut mesh = Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default());
    crate::render::write_mesh(&mut mesh, particles, &render, Affine3A::IDENTITY).expect("drawn");
    // Two strips of three particles.
    assert_eq!(mesh.count_vertices(), 2 * 6);
    assert_eq!(mesh.indices().map(|i| i.len()), Some(2 * 12));
}

#[test]
fn trails_replay_exactly_under_a_seek() {
    let mut asset = simple_effect(
        &[("spawn_rate", f(20.0)), ("lifetime", f(1.5)), ("velocity", v3(0.0, 2.0, 0.0))],
        vec![VfxNode::new(2, "force/curl_noise").with("strength", f(3.0))],
        vec![Connection::new(2, "force", 1, "acceleration")],
    );
    asset.emitters[0].render.facing = Facing::Ribbon;
    let mut stepped = sim_of(&asset);
    steps(&mut stepped, SEEK_STEP, 90, &FrameInput::default());
    let mut seeked = sim_of(&asset);
    seeked.simulate_to(90.0 * SEEK_STEP, &FrameInput::default());
    assert_eq!(stepped.emitters[0].particles, seeked.emitters[0].particles);
    assert!(stepped.emitters[0].particles.trail_stride > 0);
}

// ── Light ───────────────────────────────────────────────────────────────────

fn buffer(colors: &[Vec4], positions: &[Vec3]) -> ParticleBuffer {
    ParticleBuffer {
        id: (0..colors.len() as u32).collect(),
        color: colors.to_vec(),
        position: positions.to_vec(),
        ..Default::default()
    }
}

#[test]
fn a_per_emitter_light_sums_the_particles_and_averages_their_colour() {
    let p = buffer(&[Vec4::new(1.0, 0.0, 0.0, 1.0), Vec4::new(0.0, 1.0, 0.0, 0.5)], &[Vec3::ZERO, Vec3::ONE]);
    let light = ParticleLight {
        mode: LightMode::PerEmitter,
        intensity: 100.0,
        color: [1.0, 0.5, 1.0],
        offset: [0.0, 0.3, 0.0],
        ..Default::default()
    };
    let mut out = Vec::new();
    aggregate(&p, &light, Affine3A::IDENTITY, &mut out);
    assert_eq!(out.len(), 1);
    let expected = (0.2126 + 0.7152 * 0.5) * 100.0;
    assert!((out[0].intensity - expected).abs() < 1e-3, "{}", out[0].intensity);
    // rgb * alpha summed is (1, 0.5, 0): already brightest-channel 1, then
    // tinted.
    assert!(out[0].color.abs_diff_eq(Vec3::new(1.0, 0.25, 0.0), 1e-5), "{}", out[0].color);
    assert_eq!(out[0].position, Vec3::new(0.0, 0.3, 0.0));
    // No particles: dark, not NaN.
    aggregate(&ParticleBuffer::default(), &light, Affine3A::IDENTITY, &mut out);
    assert_eq!(out[0].intensity, 0.0);
    assert!(out[0].color.is_finite());
    assert_eq!(weight(Vec4::new(f32::NAN, 0.0, 0.0, 1.0)), 0.0);
}

#[test]
fn brightest_n_lights_the_heaviest_particles_up_to_the_cap() {
    let colors: Vec<Vec4> = (0..30).map(|k| Vec4::new(k as f32 / 30.0, 0.0, 0.0, 1.0)).collect();
    let positions: Vec<Vec3> = (0..30).map(|k| Vec3::new(k as f32, 0.0, 0.0)).collect();
    let p = buffer(&colors, &positions);
    let light = ParticleLight { mode: LightMode::BrightestN(3), intensity: 1.0, ..Default::default() };
    let mut out = Vec::new();
    aggregate(&p, &light, Affine3A::from_translation(Vec3::Y), &mut out);
    let xs: Vec<f32> = out.iter().map(|l| l.position.x).collect();
    assert_eq!(xs, [29.0, 28.0, 27.0], "heaviest first");
    assert!(out.iter().all(|l| l.position.y == 1.0), "taken into the entity's space");
    let many = ParticleLight { mode: LightMode::BrightestN(500), ..light };
    assert_eq!(many.light_count(), MAX_PARTICLE_LIGHTS as usize);
    aggregate(&p, &many, Affine3A::IDENTITY, &mut out);
    assert_eq!(out.len(), MAX_PARTICLE_LIGHTS as usize);
}

#[test]
fn a_light_rig_eases_snaps_on_a_seek_and_follows_its_nearest_target() {
    let at = |x: f32, i: f32| LightSample { position: Vec3::new(x, 0.0, 0.0), color: Vec3::ONE, intensity: i };
    let mut rig = LightRig::new(1);
    rig.update(&[at(0.0, 100.0)], 0.0, 0.5);
    assert_eq!(rig.lights[0].intensity, 100.0, "the first frame snaps");
    // One smoothing time later it has gone 1 - 1/e of the way.
    rig.update(&[at(0.0, 0.0)], 0.5, 0.5);
    assert!((rig.lights[0].intensity - 100.0 / std::f32::consts::E).abs() < 1e-3, "{}", rig.lights[0].intensity);
    // Time going backwards (a restart) snaps.
    rig.update(&[at(0.0, 40.0)], 0.1, 0.5);
    assert_eq!(rig.lights[0].intensity, 40.0);

    // Two lights keep their own particles when the brightness order swaps.
    let mut rig = LightRig::new(2);
    rig.update(&[at(0.0, 10.0), at(5.0, 5.0)], 0.0, 0.1);
    rig.update(&[at(5.2, 20.0), at(0.1, 1.0)], 0.02, 0.1);
    assert!(rig.lights[0].position.x < 1.0 && rig.lights[1].position.x > 4.0, "{:?}", rig.lights);
    // With no target a light fades out instead of vanishing.
    rig.update(&[], 0.04, 0.1);
    assert!(rig.lights[0].intensity > 0.0 && rig.lights[0].intensity < 10.0);
}

#[test]
fn flicker_dims_within_its_amount_smoothly_and_differs_per_seed() {
    let light = ParticleLight { flicker: 0.4, flicker_rate: 5.0, ..Default::default() };
    let samples: Vec<f32> = (0..400).map(|k| flicker(&light, 7, k as f32 * 0.01)).collect();
    assert!(samples.iter().all(|&m| (0.6 - 1e-5..=1.0).contains(&m)));
    let spread = samples.iter().cloned().fold(f32::MIN, f32::max) - samples.iter().cloned().fold(f32::MAX, f32::min);
    assert!(spread > 0.1, "it flickers: {spread}");
    assert!(samples.windows(2).all(|w| (w[0] - w[1]).abs() < 0.1), "smoothly");
    assert_eq!(flicker(&light, 7, 1.23), flicker(&light, 7, 1.23));
    assert_ne!(flicker(&light, 7, 1.23), flicker(&light, 8, 1.23));
    assert_eq!(flicker(&ParticleLight::default(), 7, 1.23), 1.0);
}

/// The fire presets' lights land near the old Hanabi campfire's 120000 lm
/// once the fire is in full flow.
#[test]
fn the_fire_presets_light_the_scene_at_a_campfires_brightness() {
    for name in ["Realistic Fire", "Campfire"] {
        let preset = crate::presets::PRESETS.iter().find(|p| p.name == name).expect("preset");
        let asset = parse_vfx(preset.source).expect("parse");
        let lit: Vec<usize> = (0..asset.emitters.len()).filter(|&i| asset.emitters[i].light.mode != LightMode::Off).collect();
        assert_eq!(lit.len(), 1, "{name}: one light");
        let light = asset.emitters[lit[0]].light;
        assert_eq!(light.mode, LightMode::PerEmitter);
        assert!(light.flicker > 0.0 && !light.shadows, "{name}: flickering, no shadow maps");
        let mut sim = sim_of(&asset);
        let mut total = 0.0;
        let mut out = Vec::new();
        for k in 0..240 {
            sim.step(1.0 / 60.0, &FrameInput::default());
            if k >= 120 {
                let e = sim.emitters.iter().find(|e| e.emitter_index == lit[0]).expect("compiled");
                aggregate(&e.particles, &light, Affine3A::IDENTITY, &mut out);
                total += out[0].intensity / 120.0;
            }
        }
        assert!((80_000.0..170_000.0).contains(&total), "{name}: {total} lm");
    }
}

// ── Sub-emitters ────────────────────────────────────────────────────────────

/// A parent that bursts `n` short-lived particles, and a child emitter it
/// spawns from them.
fn parent_and_child(trigger: SubEmitterTrigger, count: u32) -> VfxAsset {
    let parent = Emitter {
        name: "parent".into(),
        capacity: 1000,
        seed: 3,
        graph: graph(
            vec![out_node()
                .with("spawn_rate", f(0.0))
                .with("burst_count", f(4.0))
                .with("lifetime", f(0.5))
                .with("velocity", v3(2.0, 1.0, 0.0))
                .with("color", PinValue::Color([1.0, 0.5, 0.25, 1.0]))],
            vec![],
        ),
        sub_emitters: vec![SubEmitter { trigger, target: 1, count, inherit_velocity: 1.0, inherit_color: true }],
        ..Default::default()
    };
    let child = Emitter {
        name: "child".into(),
        capacity: 1000,
        seed: 4,
        graph: graph(
            vec![
                out_node().with("spawn_rate", f(50.0)).with("lifetime", f(10.0)),
                VfxNode::new(2, "random/vec3_range").with("min", v3(-0.1, -0.1, -0.1)).with("max", v3(0.1, 0.1, 0.1)),
            ],
            vec![Connection::new(2, "value", 1, "velocity")],
        ),
        ..Default::default()
    };
    VfxAsset { duration: 0.0, looping: false, emitters: vec![parent, child], ..Default::default() }
}

#[test]
fn a_death_spawns_children_where_the_parent_died_and_only_then() {
    let asset = parent_and_child(SubEmitterTrigger::OnDeath, 3);
    let mut sim = sim_of(&asset);
    assert!(sim.emitters[1].driven, "a target emits nothing on its own");
    steps(&mut sim, 1.0 / 60.0, 20, &FrameInput::default());
    assert_eq!(sim.emitters[1].particles.len(), 0, "nothing yet: its own spawn rate of 50 is ignored");
    let parents_at_death: Vec<Vec3> = sim.emitters[0].particles.position.clone();
    steps(&mut sim, 1.0 / 60.0, 20, &FrameInput::default());
    assert_eq!(sim.emitters[0].particles.len(), 0);
    let children = &sim.emitters[1].particles;
    assert_eq!(children.len(), 4 * 3);
    // Children start near a parent (one step of their own motion away), carry
    // its velocity and are tinted by its colour.
    for (i, c) in children.position.iter().enumerate() {
        let near = parents_at_death.iter().any(|p| p.distance(*c) < 1.5);
        assert!(near, "child {i} at {c} is nowhere near a parent");
        assert!(children.velocity[i].x > 1.5, "inherited velocity: {}", children.velocity[i]);
        assert_eq!(children.color[i], Vec4::new(1.0, 0.5, 0.25, 1.0));
        assert_eq!(children.generation[i], 1);
    }
}

#[test]
fn sub_emitters_are_deterministic_under_a_seek() {
    for trigger in [SubEmitterTrigger::OnBirth, SubEmitterTrigger::OnDeath, SubEmitterTrigger::Interval(0.1)] {
        let asset = parent_and_child(trigger, 2);
        let mut a = sim_of(&asset);
        steps(&mut a, SEEK_STEP, 60, &FrameInput::default());
        let mut b = sim_of(&asset);
        b.simulate_to(60.0 * SEEK_STEP, &FrameInput::default());
        assert!(a.emitters[1].particles.len() > 0, "{trigger:?} spawned nothing");
        assert_eq!(a.emitters[1].particles, b.emitters[1].particles, "{trigger:?}");
    }
}

#[test]
fn interval_events_land_on_their_multiples_and_are_capped() {
    assert_eq!(interval_events(0.0, 0.1, 0.25), 0..0);
    assert_eq!(interval_events(0.2, 0.3, 0.25), 1..2);
    assert_eq!(interval_events(0.24, 0.76, 0.25), 1..4);
    assert_eq!(interval_events(0.0, 100.0, 0.001).len(), crate::sim::sub::MAX_INTERVAL_EVENTS_PER_STEP as usize);
    assert_eq!(interval_events(0.0, 1.0, 0.0), 0..0);
    assert_eq!(interval_events(f32::NAN, 1.0, 0.1), 0..0);
    // Over a 0.5 s life, an interval of 0.1 fires four or five times a parent.
    let asset = parent_and_child(SubEmitterTrigger::Interval(0.1), 1);
    let mut sim = sim_of(&asset);
    steps(&mut sim, 1.0 / 60.0, 60, &FrameInput::default());
    let n = sim.emitters[1].particles.len();
    assert!((4 * 4..=4 * 5).contains(&n), "{n}");
}

#[test]
fn a_sub_emitter_loop_is_refused_and_named() {
    let mut asset = parent_and_child(SubEmitterTrigger::OnDeath, 1);
    asset.emitters[1].sub_emitters.push(SubEmitter { target: 0, ..Default::default() });
    asset.emitters[0].sub_emitters.push(SubEmitter { target: 0, ..Default::default() });
    asset.emitters[0].sub_emitters.push(SubEmitter { target: 9, ..Default::default() });
    let (accepted, issues) = accepted_rules(&asset);
    assert_eq!(accepted, vec![(0, 0)], "only the first edge survives");
    assert_eq!(issues.len(), 3, "{issues:?}");
    let sim = sim_of(&asset);
    assert_eq!(sim.sub_emitter_issues, issues);
    assert!(sim.emitters[1].rules.is_empty() && sim.emitters[0].rules.len() == 1);
    // A refused loop no longer drives its target: the parent spawns itself.
    assert!(!sim.emitters[0].driven);
}

#[test]
fn a_chain_of_sub_emitters_stops_at_the_depth_cap() {
    let n = MAX_SUB_DEPTH as usize + 3;
    let emitters = (0..n)
        .map(|i| Emitter {
            name: format!("e{i}"),
            capacity: 100,
            graph: graph(
                vec![out_node().with("spawn_rate", f(0.0)).with("burst_count", f(1.0)).with("lifetime", f(10.0))],
                vec![],
            ),
            sub_emitters: if i + 1 < n {
                vec![SubEmitter { trigger: SubEmitterTrigger::OnBirth, target: i + 1, count: 1, ..Default::default() }]
            } else {
                Vec::new()
            },
            ..Default::default()
        })
        .collect();
    let asset = VfxAsset { duration: 0.0, looping: false, emitters, ..Default::default() };
    let mut sim = sim_of(&asset);
    steps(&mut sim, 1.0 / 60.0, 3, &FrameInput::default());
    let counts: Vec<usize> = sim.emitters.iter().map(|e| e.particles.len()).collect();
    // Generation 0 to MAX_SUB_DEPTH exist; the one after would be past it.
    for (i, &c) in counts.iter().enumerate() {
        let expected = usize::from(i <= MAX_SUB_DEPTH as usize);
        assert_eq!(c, expected, "emitter {i}: {counts:?}");
    }
}

#[test]
fn removing_or_duplicating_an_emitter_keeps_sub_emitter_targets_right() {
    let mut asset = parent_and_child(SubEmitterTrigger::OnDeath, 1);
    asset.emitters.insert(1, Emitter { name: "copy".into(), ..asset.emitters[0].clone() });
    crate::params::on_emitter_inserted(&mut asset, 1);
    assert_eq!(asset.emitters[0].sub_emitters[0].target, 2);
    assert_eq!(asset.emitters[1].sub_emitters[0].target, 2, "the copy's rules were renumbered too");
    asset.emitters.remove(2);
    crate::params::on_emitter_removed(&mut asset, 2);
    assert!(asset.emitters[0].sub_emitters.is_empty(), "nothing left to spawn");
}

// ── Presets ─────────────────────────────────────────────────────────────────

#[test]
fn fire_embers_bounce_off_the_ground_at_the_fires_base() {
    for name in ["Realistic Fire", "Campfire"] {
        let preset = crate::presets::PRESETS.iter().find(|p| p.name == name).expect("preset");
        let asset = parse_vfx(preset.source).expect("parse");
        let embers = asset.emitters.iter().find(|e| e.name == "embers").expect("embers");
        assert!(embers.graph.nodes.iter().any(|n| n.node_type == "collide/plane"), "{name}: no ground");
        let mut sim = sim_of(&asset);
        let mut landed = false;
        for _ in 0..360 {
            sim.step(1.0 / 60.0, &FrameInput::default());
            let p = layer(&sim, &asset, "embers");
            for (i, pos) in p.position.iter().enumerate() {
                assert!(pos.y >= -1e-4, "{name}: an ember fell through the ground at {pos}");
                landed |= pos.y < 0.01 && p.age[i] > 0.3;
            }
        }
        assert!(landed, "{name}: no ember ever came down");
    }
}

#[test]
fn every_preset_draws_its_meshes_without_nans() {
    use bevy::mesh::Mesh;
    let mut presets: Vec<VfxAsset> =
        crate::presets::PRESETS.iter().map(|p| parse_vfx(p.source).expect("parse")).collect();
    // And one of each new kind of emitter, so the paths are exercised.
    let mut ribbon = realistic_fire();
    for e in &mut ribbon.emitters {
        e.render.facing = Facing::Ribbon;
    }
    ribbon.emitters[0].render.ribbon.kind = RibbonKind::Connect;
    presets.push(ribbon);
    for asset in presets {
        let mut sim = sim_of(&asset);
        steps(&mut sim, 1.0 / 60.0, 180, &FrameInput::default());
        for e in &sim.emitters {
            let mut mesh = Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default());
            let render = &asset.emitters[e.emitter_index].render;
            if let Some(b) = crate::render::write_mesh(&mut mesh, &e.particles, render, Affine3A::IDENTITY) {
                assert!(Vec3::from(b.center).is_finite() && Vec3::from(b.half_extents).is_finite(), "{}", asset.name);
            }
        }
    }
}

#[test]
fn new_emitter_fields_are_not_written_until_used() {
    let text = write_vfx(&VfxAsset { emitters: vec![Emitter::default()], ..Default::default() }).expect("write");
    for field in ["light", "sub_emitters", "ribbon"] {
        assert!(!text.contains(field), "{field} written while unused:\n{text}");
    }
    let mut asset = parent_and_child(SubEmitterTrigger::Interval(0.2), 2);
    asset.emitters[0].light = ParticleLight { mode: LightMode::BrightestN(3), ..Default::default() };
    asset.emitters[1].render.facing = Facing::Ribbon;
    asset.emitters[1].render.ribbon.kind = RibbonKind::Connect;
    let back = parse_vfx(&write_vfx(&asset).expect("write")).expect("parse");
    assert_eq!(back, asset);
}
