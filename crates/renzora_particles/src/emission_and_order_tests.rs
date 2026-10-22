//! The small extras on the CPU: the Emission pin, inherited velocity, the
//! draw orders and tube ribbons, and that every one of them leaves an old
//! `.vfx` loading and saving exactly as it did.

use bevy::asset::RenderAssetUsages;
use bevy::math::{Affine3A, Vec3};
use bevy::mesh::{Mesh, PrimitiveTopology, VertexAttributeValues};

use crate::asset::*;
use crate::curve::{CurvePoint, Ease};
use crate::render::write_mesh;
use crate::sim::{EffectSim, FrameInput, SEEK_STEP};
use crate::tests::{sim_of, simple_effect};

fn positions(mesh: &Mesh) -> Vec<[f32; 3]> {
    match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(p)) => p.clone(),
        _ => panic!("positions"),
    }
}

fn new_mesh() -> Mesh {
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
}

// ── Emission ────────────────────────────────────────────────────────────

#[test]
fn emission_scales_the_light_and_not_the_coverage() {
    let curve = VfxNode::new(2, "curve/curve").with(
        "points",
        PinValue::Curve(vec![
            CurvePoint { t: 0.0, value: 4.0, ease: Ease::Linear },
            CurvePoint { t: 1.0, value: 1.0, ease: Ease::Linear },
        ]),
    );
    let asset = simple_effect(
        &[
            ("spawn_rate", PinValue::Float(0.0)),
            ("lifetime", PinValue::Float(10.0)),
            ("color", PinValue::Color([0.5, 0.25, 1.0, 0.5])),
        ],
        vec![curve],
        vec![Connection::new(2, "value", 1, "emission")],
    );
    let mut sim = sim_of(&asset);
    sim.burst(1, &FrameInput::default());
    sim.step(SEEK_STEP, &FrameInput::default());
    let c = sim.emitters[0].particles.color[0];
    let life = sim.emitters[0].particles.age[0] / 10.0;
    let glow = 4.0 + (1.0 - 4.0) * life;
    assert!((c.x - 0.5 * glow).abs() < 1e-4 && (c.z - glow).abs() < 1e-4, "{c:?}");
    assert_eq!(c.w, 0.5, "alpha is the colour's, whatever the glow");

    // Unwired, the pin is 1 and the colour is exactly what it was.
    let plain = simple_effect(&[("spawn_rate", PinValue::Float(0.0)), ("color", PinValue::Color([0.3, 0.6, 0.9, 0.7]))], vec![], vec![]);
    let mut sim = sim_of(&plain);
    sim.burst(1, &FrameInput::default());
    sim.step(SEEK_STEP, &FrameInput::default());
    assert_eq!(sim.emitters[0].particles.color[0].to_array(), [0.3, 0.6, 0.9, 0.7]);
}

// ── Inherit velocity ────────────────────────────────────────────────────

#[test]
fn a_world_emitter_inherits_its_entitys_velocity_and_a_local_one_does_not() {
    let mut asset = simple_effect(&[("spawn_rate", PinValue::Float(0.0)), ("velocity", PinValue::Vec3([0.0, 1.0, 0.0]))], vec![], vec![]);
    asset.emitters[0].inherit_velocity = 0.5;
    let frame = FrameInput { emitter_velocity: Vec3::new(4.0, 0.0, -2.0), ..Default::default() };
    for space in [SimulationSpace::World, SimulationSpace::Local] {
        asset.emitters[0].space = space;
        let mut sim = sim_of(&asset);
        sim.burst(1, &frame);
        let v = sim.emitters[0].particles.velocity[0];
        match space {
            SimulationSpace::World => assert_eq!(v, Vec3::new(2.0, 1.0, -1.0)),
            SimulationSpace::Local => assert_eq!(v, Vec3::new(0.0, 1.0, 0.0), "a Local emitter already moves with the entity"),
        }
    }
    assert_eq!(Emitter { inherit_velocity: f32::NAN, space: SimulationSpace::World, ..Default::default() }.inherited_velocity(), 0.0);
}

// ── Draw order ──────────────────────────────────────────────────────────

#[test]
fn the_age_orders_draw_oldest_or_youngest_first() {
    let asset = simple_effect(&[("spawn_rate", PinValue::Float(0.0)), ("lifetime", PinValue::Float(10.0))], vec![], vec![]);
    let mut sim = sim_of(&asset);
    let frame = FrameInput::default();
    // Three particles born a step apart, then put in a row so a quad's
    // place says which it is.
    for _ in 0..3 {
        sim.burst(1, &frame);
        sim.step(SEEK_STEP, &frame);
    }
    let p = &mut sim.emitters[0].particles;
    for i in 0..3 {
        p.position[i] = Vec3::new(p.age[i], 0.0, 0.0);
    }
    let draw = |order: DrawOrder| -> Vec<f32> {
        let mut render = RenderSettings::default();
        render.set_draw_order(order);
        let mut mesh = new_mesh();
        write_mesh(&mut mesh, &sim.emitters[0].particles, &render, Affine3A::IDENTITY);
        positions(&mesh).iter().step_by(4).map(|p| p[0]).collect()
    };
    let oldest = draw(DrawOrder::OldestFirst);
    assert!(oldest.windows(2).all(|w| w[0] > w[1]), "oldest first: {oldest:?}");
    let youngest = draw(DrawOrder::YoungestFirst);
    assert!(youngest.windows(2).all(|w| w[0] < w[1]), "youngest first: {youngest:?}");
}

#[test]
fn a_draw_order_applies_only_where_it_shows() {
    let mut r = RenderSettings::default();
    r.set_draw_order(DrawOrder::OldestFirst);
    assert_eq!(r.draw_order(Dimension::TwoD), DrawOrder::OldestFirst, "age orders work in 2D");
    assert!(!r.sort, "only View Depth writes the old flag");
    r.set_draw_order(DrawOrder::ViewDepth);
    assert!(r.sort && r.sorts(Dimension::ThreeD));
    assert_eq!(r.draw_order(Dimension::TwoD), DrawOrder::Index, "2D has no depth to sort by");
    r.blend = BlendMode::Additive;
    assert_eq!(r.draw_order(Dimension::ThreeD), DrawOrder::Index, "light adds up in any order");
    r.blend = BlendMode::Alpha;
    r.facing = Facing::Ribbon;
    assert_eq!(r.draw_order(Dimension::ThreeD), DrawOrder::Index, "a ribbon is drawn along its strip");
    // A file from before the field: `sort: true` is View Depth.
    let old = RenderSettings { sort: true, ..Default::default() };
    assert_eq!(old.draw_order(Dimension::ThreeD), DrawOrder::ViewDepth);
}

// ── Tubes ───────────────────────────────────────────────────────────────

#[test]
fn a_tube_writes_a_ring_per_point_and_a_flat_strip_two_vertices() {
    let mut asset = simple_effect(
        &[("spawn_rate", PinValue::Float(0.0)), ("lifetime", PinValue::Float(10.0)), ("velocity", PinValue::Vec3([1.0, 0.0, 0.0]))],
        vec![],
        vec![],
    );
    asset.emitters[0].render.facing = Facing::Ribbon;
    asset.emitters[0].render.ribbon.points = 8;
    asset.emitters[0].render.ribbon.min_distance = 0.0;
    let mut sim = sim_of(&asset);
    let frame = FrameInput::default();
    sim.burst(1, &frame);
    for _ in 0..6 {
        sim.step(SEEK_STEP, &frame);
    }
    // The particle, then its kept points; the newest was kept this step,
    // where the particle is, and a strip skips that zero-length segment.
    let p = &sim.emitters[0].particles;
    let skipped = p.trail(0).take_while(|q| q.distance_squared(p.position[0]) < 1e-12).count();
    let points = 1 + p.trail(0).count() - skipped;
    for (shape, segments) in [(RibbonShape::Flat, 1usize), (RibbonShape::Tube, 5)] {
        let mut render = asset.emitters[0].render.clone();
        render.ribbon.shape = shape;
        render.ribbon.radial_segments = segments as u32;
        let mut mesh = new_mesh();
        write_mesh(&mut mesh, &sim.emitters[0].particles, &render, Affine3A::IDENTITY).expect("drawn");
        let (vertices, indices) = crate::render::ribbon::tube_size(points, segments);
        assert_eq!(mesh.count_vertices(), vertices, "{shape:?}");
        assert_eq!(mesh.indices().unwrap().len(), indices, "{shape:?}");
        if shape == RibbonShape::Tube {
            // Once round: the first and last corners of a ring are -0.5 and 0.5.
            let Some(VertexAttributeValues::Float32x2(corner)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0) else { panic!() };
            assert_eq!(corner[0][0], -0.5);
            assert_eq!(corner[segments][0], 0.5);
        }
    }
    assert_eq!(crate::render::ribbon::strip_size(5), crate::render::ribbon::tube_size(5, 1));
    let tube = RenderSettings { facing: Facing::Ribbon, ribbon: Ribbon { shape: RibbonShape::Tube, ..Default::default() }, ..Default::default() };
    assert!(tube.draws_tube(Dimension::ThreeD) && !tube.draws_tube(Dimension::TwoD));
    assert_eq!(Ribbon { radial_segments: 1000, ..Default::default() }.tube_segments(), MAX_TUBE_SEGMENTS);
    assert_eq!(Ribbon { radial_segments: 0, ..Default::default() }.tube_segments(), MIN_TUBE_SEGMENTS);
}

// ── Old files ───────────────────────────────────────────────────────────

#[test]
fn old_files_load_unchanged_and_new_fields_stay_out_until_set() {
    // A ribbon, a sort and a World emitter written before any of the new
    // fields existed.
    let old = r#"(
        version: 1,
        emitters: [(
            name: "trail",
            space: World,
            graph: (nodes: [(id: 1, node_type: "output/particle")]),
            render: (facing: Ribbon, sort: true, ribbon: (kind: Connect, groups: 3, tail_width: 0.5)),
        )],
    )"#;
    let asset = parse_vfx(old).expect("an old file still parses");
    let e = &asset.emitters[0];
    assert_eq!(e.inherit_velocity, 0.0);
    assert_eq!(e.render.draw_order, DrawOrder::Index);
    assert!(e.render.sort, "the old flag is kept as written");
    assert_eq!(e.render.ribbon.shape, RibbonShape::Flat);
    assert_eq!(e.render.ribbon.radial_segments, DEFAULT_TUBE_SEGMENTS);
    assert_eq!((e.render.ribbon.kind, e.render.ribbon.groups), (RibbonKind::Connect, 3));
    let text = write_vfx(&asset).unwrap();
    for field in ["inherit_velocity", "draw_order", "shape", "radial_segments", "emission"] {
        assert!(!text.contains(field), "{field} written for an effect that never set it:\n{text}");
    }
    assert_eq!(parse_vfx(&text).unwrap(), asset, "and it round-trips");

    // Set, each survives a save.
    let mut set = asset.clone();
    set.emitters[0].inherit_velocity = 0.75;
    set.emitters[0].render.set_draw_order(DrawOrder::YoungestFirst);
    set.emitters[0].render.ribbon.shape = RibbonShape::Tube;
    set.emitters[0].render.ribbon.radial_segments = 12;
    let back = parse_vfx(&write_vfx(&set).unwrap()).unwrap();
    assert_eq!(back, set);
}

#[test]
fn an_effect_without_the_emission_pin_compiles_to_a_glow_of_one() {
    // Every built-in effect predates the pin; each still plays its colours.
    for preset in crate::presets::PRESETS {
        let asset = parse_vfx(preset.source).unwrap();
        let (sim, failures) = EffectSim::new(&asset);
        assert!(failures.is_empty(), "{}", preset.name);
        for e in &sim.emitters {
            let reg = e.compiled.update.output(crate::ir::SinkPin::Emission).expect("every update has the pin");
            let instr = e.compiled.update.instrs[reg.0 as usize];
            assert_eq!(instr.op, crate::ir::Op::constant([1.0, 0.0, 0.0, 0.0]), "{}", preset.name);
        }
    }
}
