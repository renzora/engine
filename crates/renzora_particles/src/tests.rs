//! Unit tests: the file format, the compiler, every node, and the simulation.
//!
//! The runtime systems and the shaders have their own integration tests under
//! `tests/`, because they need a Bevy app or Bevy's shader libraries.

use std::collections::HashSet;
use std::f32::consts::PI;

use bevy::math::{Affine3A, Quat, Vec3, Vec4};

use crate::asset::*;
use crate::compile::{compile_graph, CompileErrorKind};
use crate::curve::{CurvePoint, Ease, GradientStop};
use crate::ir::{Context, SinkPin, ValueType};
use crate::nodes::{PinKind, NODES, OUTPUT_NODE};
use crate::presets::PRESETS;
use crate::sim::interp::{eval, EvalInputs, Registers};
use crate::sim::{EffectSim, FrameInput, SEEK_STEP};

// ── Helpers ─────────────────────────────────────────────────────────────────

pub(crate) fn graph(nodes: Vec<VfxNode>, connections: Vec<Connection>) -> VfxGraph {
    VfxGraph { nodes, connections, ..Default::default() }
}

pub(crate) fn out_node() -> VfxNode {
    VfxNode::new(1, OUTPUT_NODE)
}

/// The sink pin a value of `ty` is read through in `ctx`.
fn sink_for(ty: ValueType, ctx: Context) -> &'static str {
    match (ctx, ty) {
        (Context::Spawn, _) => "spawn_rate",
        (Context::Init, ValueType::Float) => "lifetime",
        (Context::Init, ValueType::Vec2) => "size",
        (Context::Init, ValueType::Vec3) => "velocity",
        (Context::Init, ValueType::Color) => "color",
        (Context::Update, ValueType::Float) => "drag",
        (Context::Update, ValueType::Vec2) => "size_over_life",
        (Context::Update, ValueType::Vec3) => "acceleration",
        (Context::Update, ValueType::Color) => "color_over_life",
    }
}

const TEST_AGE: f32 = 0.5;
const TEST_LIFETIME: f32 = 2.0;
const TEST_POSITION: Vec3 = Vec3::new(1.0, 2.0, 3.0);
const TEST_VELOCITY: Vec3 = Vec3::new(4.0, 5.0, 6.0);
const TEST_TIME: f32 = 1.25;
const TEST_DT: f32 = 0.1;
const TEST_EMITTER_VELOCITY: Vec3 = Vec3::new(0.0, 0.0, 9.0);
const TEST_WIND: Vec3 = Vec3::new(3.0, 0.0, 4.0);
const TEST_TURBULENCE: f32 = 0.5;

/// Compile `g` and evaluate the sink pin `sink` in `ctx` for particles with
/// spawn indices `0..n`, returning one value per particle.
fn eval_sink(g: &VfxGraph, ctx: Context, sink: &str, n: usize) -> Vec<Vec4> {
    let compiled = compile_graph(g).unwrap_or_else(|e| panic!("compile failed: {e:?}"));
    let program = compiled.program(ctx);
    let pin = SinkPin::from_name(sink).expect("sink pin");
    let reg = program.output(pin).expect("output register");
    let ids: Vec<u32> = (0..n as u32).collect();
    let age = vec![TEST_AGE; n];
    let lifetime = vec![TEST_LIFETIME; n];
    let position = vec![TEST_POSITION; n];
    let velocity = vec![TEST_VELOCITY; n];
    let inputs = EvalInputs {
        count: n,
        seed: 42,
        ids: &ids,
        spawn_id: 0,
        age: &age,
        lifetime: &lifetime,
        position: &position,
        velocity: &velocity,
        emitter_time: TEST_TIME,
        delta_time: TEST_DT,
        emitter_velocity: TEST_EMITTER_VELOCITY,
        params: &[],
        wind: TEST_WIND,
        wind_turbulence: TEST_TURBULENCE,
    };
    let mut regs = Registers::default();
    eval(program, &compiled, &inputs, &mut regs);
    regs.get(reg).to_vec()
}

/// Evaluate output `pin` of `node` (which must have id 2) as type `ty` in
/// `ctx`, for `n` particles.
fn eval_node_n(node: VfxNode, pin: &str, ty: ValueType, ctx: Context, n: usize) -> Vec<Vec4> {
    let sink = sink_for(ty, ctx);
    let g = graph(vec![out_node(), node], vec![Connection::new(2, pin, 1, sink)]);
    eval_sink(&g, ctx, sink, n)
}

pub(crate) fn eval_node(node: VfxNode, pin: &str, ty: ValueType, ctx: Context) -> Vec4 {
    eval_node_n(node, pin, ty, ctx, 1)[0]
}

pub(crate) fn f(v: f32) -> PinValue {
    PinValue::Float(v)
}

pub(crate) fn v3(x: f32, y: f32, z: f32) -> PinValue {
    PinValue::Vec3([x, y, z])
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

fn close3(a: Vec4, b: Vec3) -> bool {
    a.truncate().abs_diff_eq(b, 1e-4)
}

/// A minimal effect: one emitter whose output node has the given inline values.
pub(crate) fn simple_effect(values: &[(&str, PinValue)], extra: Vec<VfxNode>, conns: Vec<Connection>) -> VfxAsset {
    let mut out = out_node();
    for (k, v) in values {
        out.input_values.insert(k.to_string(), v.clone());
    }
    let mut nodes = vec![out];
    nodes.extend(extra);
    VfxAsset {
        duration: 0.0,
        looping: false,
        emitters: vec![Emitter {
            capacity: 10_000,
            graph: graph(nodes, conns),
            ..Default::default()
        }],
        ..Default::default()
    }
}

pub(crate) fn sim_of(asset: &VfxAsset) -> EffectSim {
    let (sim, failures) = EffectSim::new(asset);
    assert!(failures.is_empty(), "emitters failed to compile");
    sim
}

pub(crate) fn steps(sim: &mut EffectSim, dt: f32, n: usize, frame: &FrameInput) {
    for _ in 0..n {
        sim.step(dt, frame);
    }
}

// ── File format ─────────────────────────────────────────────────────────────

#[test]
fn every_preset_round_trips_through_ron() {
    for preset in PRESETS {
        let asset = parse_vfx(preset.source).unwrap_or_else(|e| panic!("{}: {e}", preset.name));
        assert_eq!(asset.version, VFX_VERSION);
        assert_eq!(asset.name, preset.name);
        let text = write_vfx(&asset).expect("serialize");
        let again = parse_vfx(&text).unwrap_or_else(|e| panic!("{}: reparse: {e}\n{text}", preset.name));
        assert_eq!(asset, again, "{} changed on a round trip", preset.name);
    }
}

#[test]
fn a_hand_written_file_can_be_short() {
    let text = r#"(emitters: [(graph: (nodes: [(id: 1, node_type: "output/particle")]))])"#;
    let asset = parse_vfx(text).expect("parse");
    assert_eq!(asset.emitters.len(), 1);
    let e = &asset.emitters[0];
    assert_eq!(e.capacity, DEFAULT_CAPACITY);
    assert_eq!(e.render, RenderSettings::default());
    assert!(compile_graph(&e.graph).is_ok());
}

#[test]
fn an_unversioned_file_is_migrated() {
    // Version 0: no `version` field, and a zero capacity that meant "default".
    let text = r#"(name: "old", emitters: [(capacity: 0, graph: (nodes: [(id: 1, node_type: "output/particle")]))])"#;
    let asset = parse_vfx(text).expect("parse");
    assert_eq!(asset.version, VFX_VERSION);
    assert_eq!(asset.emitters[0].capacity, DEFAULT_CAPACITY);

    // The same zero in a current file is taken at its word.
    let text = r#"(version: 1, emitters: [(capacity: 0)])"#;
    let asset = parse_vfx(text).expect("parse");
    assert_eq!(asset.emitters[0].capacity, 0);
}

#[test]
fn migrate_runs_only_the_steps_after_the_files_version() {
    let mut asset = VfxAsset {
        emitters: vec![Emitter {
            capacity: 0,
            ..Default::default()
        }],
        ..Default::default()
    };
    migrate(&mut asset, VFX_VERSION);
    assert_eq!(asset.emitters[0].capacity, 0, "a current file needs no step");
    migrate(&mut asset, 0);
    assert_eq!(asset.emitters[0].capacity, DEFAULT_CAPACITY);
    assert_eq!(asset.version, VFX_VERSION);
}

#[test]
fn a_file_from_a_newer_engine_is_refused() {
    let text = format!("(version: {}, some_future_field: 3)", VFX_VERSION + 1);
    match parse_vfx(&text) {
        Err(VfxLoadError::TooNew { found, supported }) => {
            assert_eq!(found, VFX_VERSION + 1);
            assert_eq!(supported, VFX_VERSION);
        }
        other => panic!("expected TooNew, got {other:?}"),
    }
}

// ── Compiler ────────────────────────────────────────────────────────────────

#[test]
fn a_cycle_is_rejected_with_a_node_in_it() {
    let g = graph(
        vec![out_node(), VfxNode::new(2, "math/add"), VfxNode::new(3, "math/add")],
        vec![
            Connection::new(2, "value", 3, "a"),
            Connection::new(3, "value", 2, "a"),
            Connection::new(3, "value", 1, "lifetime"),
        ],
    );
    let errors = compile_graph(&g).expect_err("cycle");
    assert!(errors
        .iter()
        .any(|e| e.kind == CompileErrorKind::Cycle && matches!(e.node, Some(2) | Some(3))));
}

#[test]
fn an_unreachable_cycle_is_still_rejected() {
    let g = graph(
        vec![out_node(), VfxNode::new(2, "math/add"), VfxNode::new(3, "math/add")],
        vec![Connection::new(2, "value", 3, "a"), Connection::new(3, "value", 2, "a")],
    );
    let errors = compile_graph(&g).expect_err("cycle");
    assert!(errors.iter().any(|e| e.kind == CompileErrorKind::Cycle));
}

#[test]
fn a_type_mismatch_names_the_consuming_node_and_pin() {
    // A colour into a Vec3 pin.
    let g = graph(
        vec![out_node(), VfxNode::new(2, "math/color")],
        vec![Connection::new(2, "value", 1, "position")],
    );
    let errors = compile_graph(&g).expect_err("mismatch");
    assert!(errors.iter().any(|e| e.node == Some(1)
        && matches!(&e.kind, CompileErrorKind::TypeMismatch { pin, found: ValueType::Color, .. } if pin == "position")));

    // A Vec3 and a Color into one generic node.
    let g = graph(
        vec![
            out_node(),
            VfxNode::new(2, "math/vec3"),
            VfxNode::new(3, "math/color"),
            VfxNode::new(4, "math/add"),
        ],
        vec![Connection::new(2, "value", 4, "a"), Connection::new(3, "value", 4, "b")],
    );
    let errors = compile_graph(&g).expect_err("mismatch");
    assert!(errors
        .iter()
        .any(|e| e.node == Some(4) && matches!(e.kind, CompileErrorKind::TypeMismatch { .. })));
}

#[test]
fn a_float_splats_into_a_vector_pin() {
    let g = graph(
        vec![out_node(), VfxNode::new(2, "math/float").with("value", f(0.25))],
        vec![Connection::new(2, "value", 1, "size")],
    );
    let v = eval_sink(&g, Context::Init, "size", 1)[0];
    assert!(close(v.x, 0.25) && close(v.y, 0.25));
}

#[test]
fn a_missing_output_is_reported() {
    let g = graph(vec![VfxNode::new(2, "math/add")], vec![]);
    let errors = compile_graph(&g).expect_err("no output");
    assert!(errors
        .iter()
        .any(|e| e.kind == CompileErrorKind::MissingOutput && e.node.is_none()));
}

#[test]
fn structural_errors_carry_node_ids() {
    let g = graph(
        vec![out_node(), VfxNode::new(2, "nope/nothing"), VfxNode::new(3, "math/add")],
        vec![
            Connection::new(3, "value", 1, "not_a_pin"),
            Connection::new(99, "value", 3, "a"),
        ],
    );
    let errors = compile_graph(&g).expect_err("errors");
    assert!(errors
        .iter()
        .any(|e| e.node == Some(2) && matches!(e.kind, CompileErrorKind::UnknownNodeType(_))));
    assert!(errors
        .iter()
        .any(|e| e.node == Some(1) && matches!(&e.kind, CompileErrorKind::UnknownPin(p) if p == "not_a_pin")));
    assert!(errors.iter().any(|e| e.node == Some(3)
        && matches!(e.kind, CompileErrorKind::DanglingConnection { missing: 99 })));
}

#[test]
fn a_bad_inline_value_is_reported() {
    let g = graph(vec![out_node().with("position", PinValue::Color([1.0; 4]))], vec![]);
    let errors = compile_graph(&g).expect_err("bad value");
    assert!(errors
        .iter()
        .any(|e| e.node == Some(1) && matches!(&e.kind, CompileErrorKind::BadValue(p) if p == "position")));
}

#[test]
fn compilation_is_deterministic_and_shares_subexpressions() {
    for preset in PRESETS {
        let asset = parse_vfx(preset.source).expect("parse");
        for emitter in &asset.emitters {
            let a = compile_graph(&emitter.graph).expect("compile");
            let b = compile_graph(&emitter.graph).expect("compile");
            assert_eq!(a, b, "{} compiled differently twice", preset.name);
            for ctx in [Context::Spawn, Context::Init, Context::Update] {
                let program = a.program(ctx);
                let unique: HashSet<_> = program.instrs.iter().collect();
                assert_eq!(unique.len(), program.instrs.len(), "{}: duplicate instruction", preset.name);
                for (i, instr) in program.instrs.iter().enumerate() {
                    assert!(
                        instr.op.operands().iter().all(|r| (r.0 as usize) < i),
                        "instruction {i} reads a later register"
                    );
                }
            }
        }
    }
}

#[test]
fn identical_nodes_cost_one_evaluation() {
    // Two separate Multiply nodes computing age * 2.
    let g = graph(
        vec![
            out_node(),
            VfxNode::new(2, "input/age"),
            VfxNode::new(3, "math/multiply").with("b", f(2.0)),
            VfxNode::new(4, "math/multiply").with("b", f(2.0)),
        ],
        vec![
            Connection::new(2, "value", 3, "a"),
            Connection::new(2, "value", 4, "a"),
            Connection::new(3, "value", 1, "drag"),
            Connection::new(4, "value", 1, "size_over_life"),
        ],
    );
    let compiled = compile_graph(&g).expect("compile");
    let drag = compiled.update.output(SinkPin::Drag).expect("drag");
    let size = compiled.update.output(SinkPin::SizeOverLife).expect("size");
    // The Vec2 sink splats, but what it splats is the very register drag reads.
    let size_instr = compiled.update.instrs[size.0 as usize];
    assert!(size_instr.op.operands().contains(&drag));
}

#[test]
fn every_catalog_node_type_checks_and_lowers() {
    for def in NODES.iter().filter(|d| d.node_type != OUTPUT_NODE) {
        let g = graph(vec![out_node(), VfxNode::new(2, def.node_type)], vec![]);
        compile_graph(&g).unwrap_or_else(|e| panic!("{}: {e:?}", def.node_type));
        // And through a sink, in each context it can reach.
        for out in def.outputs {
            let ty = match out.kind {
                PinKind::Fixed(t) => t,
                PinKind::Generic => ValueType::Float,
            };
            for ctx in [Context::Init, Context::Update] {
                let v = eval_node(VfxNode::new(2, def.node_type), out.name, ty, ctx);
                assert!(v.is_finite(), "{}.{} is not finite in {ctx:?}", def.node_type, out.name);
            }
        }
    }
}

#[test]
fn context_irrelevant_inputs_read_zero() {
    for node in ["input/age", "input/life", "input/lifetime"] {
        let v = eval_node(VfxNode::new(2, node), "value", ValueType::Float, Context::Init);
        assert_eq!(v.x, 0.0, "{node} at birth");
    }
    let v = eval_node(VfxNode::new(2, "input/position"), "value", ValueType::Vec3, Context::Init);
    assert_eq!(v, Vec4::ZERO);
    let v = eval_node(VfxNode::new(2, "input/index"), "value", ValueType::Float, Context::Spawn);
    assert_eq!(v.x, 0.0);
}

// ── Nodes ───────────────────────────────────────────────────────────────────

#[test]
fn input_nodes_read_their_attributes() {
    let u = Context::Update;
    let read = |t: &str, ty| eval_node(VfxNode::new(2, t), "value", ty, u);
    assert!(close(read("input/age", ValueType::Float).x, TEST_AGE));
    assert!(close(read("input/life", ValueType::Float).x, TEST_AGE / TEST_LIFETIME));
    assert!(close(read("input/lifetime", ValueType::Float).x, TEST_LIFETIME));
    assert!(close3(read("input/position", ValueType::Vec3), TEST_POSITION));
    assert!(close3(read("input/velocity", ValueType::Vec3), TEST_VELOCITY));
    assert!(close(read("input/emitter_time", ValueType::Float).x, TEST_TIME));
    assert!(close(read("input/delta_time", ValueType::Float).x, TEST_DT));
    assert!(close3(read("input/emitter_velocity", ValueType::Vec3), TEST_EMITTER_VELOCITY));
    let ids = eval_node_n(VfxNode::new(2, "input/index"), "value", ValueType::Float, Context::Init, 4);
    assert_eq!(ids.iter().map(|v| v.x).collect::<Vec<_>>(), vec![0.0, 1.0, 2.0, 3.0]);
}

#[test]
fn constants() {
    let i = Context::Init;
    assert!(close(eval_node(VfxNode::new(2, "math/float").with("value", f(3.5)), "value", ValueType::Float, i).x, 3.5));
    assert!(close3(
        eval_node(VfxNode::new(2, "math/vec3").with("value", v3(1.0, 2.0, 3.0)), "value", ValueType::Vec3, i),
        Vec3::new(1.0, 2.0, 3.0)
    ));
    let c = eval_node(
        VfxNode::new(2, "math/color").with("value", PinValue::Color([0.1, 0.2, 0.3, 0.4])),
        "value",
        ValueType::Color,
        i,
    );
    assert!(c.abs_diff_eq(Vec4::new(0.1, 0.2, 0.3, 0.4), 1e-6));
}

#[test]
fn arithmetic_nodes() {
    let i = Context::Init;
    let bin = |t: &str, a: PinValue, b: PinValue, ty| {
        eval_node(VfxNode::new(2, t).with("a", a).with("b", b), "value", ty, i)
    };
    assert!(close3(bin("math/add", v3(1.0, 2.0, 3.0), f(1.0), ValueType::Vec3), Vec3::new(2.0, 3.0, 4.0)));
    assert!(close3(bin("math/subtract", v3(1.0, 2.0, 3.0), f(1.0), ValueType::Vec3), Vec3::new(0.0, 1.0, 2.0)));
    assert!(close3(bin("math/multiply", v3(1.0, 2.0, 3.0), f(2.0), ValueType::Vec3), Vec3::new(2.0, 4.0, 6.0)));
    assert!(close(bin("math/divide", f(3.0), f(2.0), ValueType::Float).x, 1.5));
    assert_eq!(bin("math/divide", f(3.0), f(0.0), ValueType::Float).x, 0.0, "division by zero gives zero");
    assert!(close3(bin("math/min", v3(1.0, 5.0, 3.0), f(2.0), ValueType::Vec3), Vec3::new(1.0, 2.0, 2.0)));
    assert!(close3(bin("math/max", v3(1.0, 5.0, 3.0), f(2.0), ValueType::Vec3), Vec3::new(2.0, 5.0, 3.0)));
    assert!(close(bin("math/dot", v3(1.0, 2.0, 3.0), v3(4.0, 5.0, 6.0), ValueType::Float).x, 32.0));

    let un = |t: &str, v: PinValue, ty| eval_node(VfxNode::new(2, t).with("value", v), "value", ty, i);
    assert!(close(un("math/abs", f(-2.0), ValueType::Float).x, 2.0));
    assert!(close(un("math/negate", f(3.0), ValueType::Float).x, -3.0));
    assert!(close(un("math/sin", f(PI / 2.0), ValueType::Float).x, 1.0));
    assert!(close(un("math/cos", f(0.0), ValueType::Float).x, 1.0));
    assert!(close(un("math/length", v3(3.0, 4.0, 0.0), ValueType::Float).x, 5.0));
    assert!(close3(un("math/normalize", v3(0.0, 3.0, 4.0), ValueType::Vec3), Vec3::new(0.0, 0.6, 0.8)));
    assert_eq!(un("math/normalize", v3(0.0, 0.0, 0.0), ValueType::Vec3), Vec4::ZERO);

    let lerp = VfxNode::new(2, "math/lerp").with("a", f(0.0)).with("b", f(10.0)).with("t", f(0.25));
    assert!(close(eval_node(lerp, "value", ValueType::Float, i).x, 2.5));
    let clamp = VfxNode::new(2, "math/clamp").with("value", f(5.0)).with("min", f(0.0)).with("max", f(1.0));
    assert!(close(eval_node(clamp, "value", ValueType::Float, i).x, 1.0));
    let ss = VfxNode::new(2, "math/smoothstep").with("edge0", f(0.0)).with("edge1", f(1.0)).with("value", f(0.25));
    assert!(close(eval_node(ss, "value", ValueType::Float, i).x, 0.15625));
    let remap = VfxNode::new(2, "math/remap")
        .with("value", f(5.0))
        .with("in_min", f(0.0))
        .with("in_max", f(10.0))
        .with("out_min", f(100.0))
        .with("out_max", f(200.0));
    assert!(close(eval_node(remap, "value", ValueType::Float, i).x, 150.0));
    let compose = VfxNode::new(2, "math/compose_vec3").with("x", f(1.0)).with("y", f(2.0)).with("z", f(3.0));
    assert!(close3(eval_node(compose, "value", ValueType::Vec3, i), Vec3::new(1.0, 2.0, 3.0)));
    for (pin, want) in [("x", 7.0), ("y", 8.0), ("z", 9.0)] {
        let split = VfxNode::new(2, "math/split_vec3").with("value", v3(7.0, 8.0, 9.0));
        assert!(close(eval_node(split, pin, ValueType::Float, i).x, want));
    }
}

#[test]
fn random_nodes_stay_in_range_and_are_stable() {
    let i = Context::Init;
    let range = VfxNode::new(2, "random/range").with("min", f(2.0)).with("max", f(3.0));
    let a = eval_node_n(range.clone(), "value", ValueType::Float, i, 256);
    let b = eval_node_n(range, "value", ValueType::Float, i, 256);
    assert_eq!(a, b, "same seed and index give the same value");
    assert!(a.iter().all(|v| (2.0..3.0).contains(&v.x)));
    let distinct: HashSet<u32> = a.iter().map(|v| v.x.to_bits()).collect();
    assert!(distinct.len() > 250, "values should differ between particles");

    let vr = VfxNode::new(2, "random/vec3_range")
        .with("min", v3(-1.0, 10.0, 0.0))
        .with("max", v3(1.0, 20.0, 0.0));
    for v in eval_node_n(vr, "value", ValueType::Vec3, i, 128) {
        assert!((-1.0..1.0).contains(&v.x) && (10.0..20.0).contains(&v.y) && v.z == 0.0);
    }

    for v in eval_node_n(VfxNode::new(2, "random/unit_vector"), "value", ValueType::Vec3, i, 128) {
        assert!(close(v.truncate().length(), 1.0));
    }

    let cb = VfxNode::new(2, "random/color_between")
        .with("a", PinValue::Color([0.0, 0.0, 0.0, 1.0]))
        .with("b", PinValue::Color([1.0, 0.5, 0.0, 1.0]));
    for c in eval_node_n(cb, "color", ValueType::Color, i, 64) {
        assert!(close(c.y, c.x * 0.5) && c.z == 0.0 && close(c.w, 1.0));
    }
}

#[test]
fn particle_random_is_shared_by_salt_and_stable_over_life() {
    let node = |id, salt| VfxNode::new(id, "input/particle_random").with("salt", f(salt));
    let g = graph(
        vec![out_node(), node(2, 1.0), node(3, 1.0), node(4, 2.0)],
        vec![
            Connection::new(2, "value", 1, "lifetime"),
            Connection::new(3, "value", 1, "rotation"),
            Connection::new(4, "value", 1, "angular_velocity"),
        ],
    );
    let a = eval_sink(&g, Context::Init, "lifetime", 16);
    let b = eval_sink(&g, Context::Init, "rotation", 16);
    let c = eval_sink(&g, Context::Init, "angular_velocity", 16);
    assert_eq!(a, b, "two nodes with one salt agree");
    assert_ne!(a, c, "a different salt is an independent value");

    // Read in the update context, the same particle sees the same number.
    let g = graph(vec![out_node(), node(2, 1.0)], vec![Connection::new(2, "value", 1, "drag")]);
    let d = eval_sink(&g, Context::Update, "drag", 16);
    assert_eq!(a, d);
}

#[test]
fn shapes_sample_where_they_say() {
    let i = Context::Init;
    let n = 256;
    let pos = |node: VfxNode| eval_node_n(node, "position", ValueType::Vec3, i, n);
    let dir = |node: VfxNode| eval_node_n(node, "direction", ValueType::Vec3, i, n);

    let point = VfxNode::new(2, "shape/point");
    assert!(pos(point.clone()).iter().all(|p| *p == Vec4::ZERO));
    assert!(dir(point).iter().all(|d| close(d.truncate().length(), 1.0)));

    let sphere = VfxNode::new(2, "shape/sphere").with("radius", f(2.0));
    assert!(pos(sphere.clone()).iter().all(|p| p.truncate().length() <= 2.0 + 1e-4));
    assert!(pos(sphere.clone()).iter().any(|p| p.truncate().length() < 1.5));
    for (p, d) in pos(sphere.clone()).iter().zip(dir(sphere)) {
        assert!(p.truncate().normalize().abs_diff_eq(d.truncate(), 1e-3), "direction points outward");
    }
    let shell = VfxNode::new(2, "shape/sphere").with("radius", f(2.0)).with("surface", PinValue::Bool(true));
    assert!(pos(shell).iter().all(|p| close(p.truncate().length(), 2.0)));

    let cube = VfxNode::new(2, "shape/box").with("size", v3(2.0, 4.0, 6.0));
    for p in pos(cube) {
        assert!(p.x.abs() <= 1.0 && p.y.abs() <= 2.0 && p.z.abs() <= 3.0);
    }

    let cone = VfxNode::new(2, "shape/cone").with("angle", f(30.0)).with("radius", f(0.5));
    let cos_max = (30.0f32).to_radians().cos();
    for d in dir(cone.clone()) {
        assert!(close(d.truncate().length(), 1.0) && d.y >= cos_max - 1e-4);
    }
    for p in pos(cone) {
        assert!(p.y == 0.0 && Vec3::new(p.x, 0.0, p.z).length() <= 0.5 + 1e-4);
    }

    let circle = VfxNode::new(2, "shape/circle").with("radius", f(3.0)).with("edge", PinValue::Bool(true));
    assert!(pos(circle.clone()).iter().all(|p| close(p.truncate().length(), 3.0) && p.z == 0.0));
    assert!(dir(circle).iter().all(|d| close(d.truncate().length(), 1.0) && d.z == 0.0));
    let disc = VfxNode::new(2, "shape/circle").with("radius", f(3.0));
    assert!(pos(disc).iter().all(|p| p.truncate().length() <= 3.0 + 1e-4));

    let rect = VfxNode::new(2, "shape/rect").with("size", PinValue::Vec2([10.0, 2.0]));
    for p in pos(rect) {
        assert!(p.x.abs() <= 5.0 && p.y.abs() <= 1.0 && p.z == 0.0);
    }
}

#[test]
fn curve_and_gradient_nodes() {
    let u = Context::Update;
    let curve = |ease| {
        VfxNode::new(2, "curve/curve").with(
            "points",
            PinValue::Curve(vec![
                CurvePoint { t: 0.0, value: 0.0, ease },
                CurvePoint { t: 1.0, value: 10.0, ease: Ease::Linear },
            ]),
        )
    };
    // t defaults to life, which the test inputs put at 0.25.
    assert!(close(eval_node(curve(Ease::Linear), "value", ValueType::Float, u).x, 2.5));
    assert!(close(eval_node(curve(Ease::Step), "value", ValueType::Float, u).x, 0.0));
    assert!(close(eval_node(curve(Ease::EaseIn), "value", ValueType::Float, u).x, 0.625));

    let gradient = VfxNode::new(2, "curve/gradient")
        .with(
            "stops",
            PinValue::Gradient(vec![
                GradientStop { t: 0.0, color: [0.0, 0.0, 0.0, 0.0] },
                GradientStop { t: 0.5, color: [1.0, 1.0, 1.0, 1.0] },
            ]),
        )
        .with("t", f(0.25));
    let c = eval_node(gradient, "color", ValueType::Color, u);
    assert!(c.abs_diff_eq(Vec4::splat(0.5), 1e-5));
}

#[test]
fn force_nodes() {
    let u = Context::Update;
    let attractor = VfxNode::new(2, "force/attractor")
        .with("position", v3(0.0, 0.0, 0.0))
        .with("point", v3(4.0, 0.0, 0.0))
        .with("strength", f(2.0));
    assert!(close3(eval_node(attractor, "force", ValueType::Vec3, u), Vec3::new(2.0, 0.0, 0.0)));
    let falloff = VfxNode::new(2, "force/attractor")
        .with("position", v3(0.0, 0.0, 0.0))
        .with("point", v3(2.0, 0.0, 0.0))
        .with("strength", f(10.0))
        .with("falloff", f(1.0));
    assert!(close3(eval_node(falloff, "force", ValueType::Vec3, u), Vec3::new(2.0, 0.0, 0.0)));

    let vortex = VfxNode::new(2, "force/vortex").with("position", v3(1.0, 0.0, 0.0));
    assert!(close3(eval_node(vortex, "force", ValueType::Vec3, u), Vec3::new(0.0, 0.0, -1.0)));

    // Curl noise is finite, varies with position and scales with strength.
    // (Off the lattice: value noise has a zero gradient at integer points, and
    // the test position is one.)
    let curl = |strength| {
        VfxNode::new(2, "force/curl_noise")
            .with("strength", f(strength))
            .with("frequency", f(0.37))
    };
    let a = eval_node(curl(1.0), "force", ValueType::Vec3, u);
    let b = eval_node(curl(3.0), "force", ValueType::Vec3, u);
    assert!(a.is_finite() && a.truncate().length() > 0.0);
    assert!(b.truncate().abs_diff_eq(a.truncate() * 3.0, 1e-4));
    let elsewhere = curl(1.0).with("position", v3(-7.3, 0.4, 2.2));
    assert_ne!(a, eval_node(elsewhere, "force", ValueType::Vec3, u));
}

#[test]
fn curl_noise_is_divergence_free() {
    let p = Vec3::new(0.37, 1.91, -2.4);
    let h = 1e-2;
    let d = |axis: Vec3| (crate::noise::curl(p + axis * h) - crate::noise::curl(p - axis * h)) / (2.0 * h);
    let div = d(Vec3::X).x + d(Vec3::Y).y + d(Vec3::Z).z;
    let scale = crate::noise::curl(p).length().max(1.0);
    assert!(div.abs() < 0.05 * scale, "divergence {div}");
}

// ── Simulation ──────────────────────────────────────────────────────────────

#[test]
fn spawn_rate_accumulates_fractions() {
    // 8/s in exact eighth-second steps: exactly one particle per step.
    let asset = simple_effect(&[("spawn_rate", f(8.0)), ("lifetime", f(100.0))], vec![], vec![]);
    let mut sim = sim_of(&asset);
    steps(&mut sim, 0.125, 8, &FrameInput::default());
    assert_eq!(sim.alive(), 8);

    // 10/s at 60 Hz: a sixth of a particle a step, still ten a second.
    let asset = simple_effect(&[("spawn_rate", f(10.0)), ("lifetime", f(100.0))], vec![], vec![]);
    let mut sim = sim_of(&asset);
    steps(&mut sim, 1.0 / 60.0, 180, &FrameInput::default());
    assert!((29..=30).contains(&sim.alive()), "{}", sim.alive());
}

#[test]
fn bursts_fire_on_schedule() {
    let burst = |count: f32, interval: f32, cycles: f32| {
        simple_effect(
            &[
                ("spawn_rate", f(0.0)),
                ("lifetime", f(100.0)),
                ("burst_count", f(count)),
                ("burst_interval", f(interval)),
                ("burst_cycles", f(cycles)),
            ],
            vec![],
            vec![],
        )
    };
    let frame = FrameInput::default();

    let mut sim = sim_of(&burst(5.0, 0.5, 3.0));
    steps(&mut sim, 1.0 / 60.0, 180, &frame);
    assert_eq!(sim.alive(), 15, "three cycles of five");

    let mut sim = sim_of(&burst(5.0, 0.5, 0.0));
    steps(&mut sim, 0.125, 8, &frame);
    assert_eq!(sim.alive(), 10, "unlimited cycles: bursts at 0 and 0.5 in the first second");

    // Looping effect: the burst schedule restarts every cycle.
    let mut asset = burst(4.0, 10.0, 1.0);
    asset.duration = 1.0;
    asset.looping = true;
    let mut sim = sim_of(&asset);
    steps(&mut sim, 0.125, 24, &frame);
    assert_eq!(sim.alive(), 12, "one burst per loop for three loops");

    // Non-looping: nothing after the duration.
    let mut asset = burst(4.0, 0.5, 0.0);
    asset.duration = 1.0;
    let mut sim = sim_of(&asset);
    steps(&mut sim, 0.125, 40, &frame);
    assert_eq!(sim.alive(), 8);
}

#[test]
fn particles_die_after_their_lifetime() {
    let asset = simple_effect(
        &[("spawn_rate", f(0.0)), ("burst_count", f(10.0)), ("lifetime", f(0.5))],
        vec![],
        vec![],
    );
    let mut sim = sim_of(&asset);
    let frame = FrameInput::default();
    steps(&mut sim, 0.125, 4, &frame);
    assert_eq!(sim.alive(), 10, "age 0.5 is not past a 0.5 lifetime");
    sim.step(0.125, &frame);
    assert_eq!(sim.alive(), 0);
}

#[test]
fn capacity_is_enforced_and_drops_are_counted() {
    let mut asset = simple_effect(
        &[("spawn_rate", f(0.0)), ("burst_count", f(12.0)), ("lifetime", f(10.0))],
        vec![],
        vec![],
    );
    asset.emitters[0].capacity = 5;
    let mut sim = sim_of(&asset);
    sim.step(0.1, &FrameInput::default());
    assert_eq!(sim.alive(), 5);
    assert_eq!(sim.dropped(), 7);
    sim.burst(3, &FrameInput::default());
    assert_eq!(sim.dropped(), 10);
    sim.reset();
    assert_eq!(sim.dropped(), 0);
}

#[test]
fn integration_applies_velocity_gravity_and_drag() {
    let asset = simple_effect(
        &[
            ("spawn_rate", f(0.0)),
            ("burst_count", f(1.0)),
            ("lifetime", f(10.0)),
            ("velocity", v3(1.0, 0.0, 0.0)),
            ("gravity", v3(0.0, -10.0, 0.0)),
        ],
        vec![],
        vec![],
    );
    let mut sim = sim_of(&asset);
    sim.step(0.5, &FrameInput::default());
    let p = &sim.emitters[0].particles;
    assert!(p.velocity[0].abs_diff_eq(Vec3::new(1.0, -5.0, 0.0), 1e-5));
    assert!(p.position[0].abs_diff_eq(Vec3::new(0.5, -2.5, 0.0), 1e-5));

    let asset = simple_effect(
        &[
            ("spawn_rate", f(0.0)),
            ("burst_count", f(1.0)),
            ("lifetime", f(10.0)),
            ("velocity", v3(4.0, 0.0, 0.0)),
            ("drag", f(1.0)),
        ],
        vec![],
        vec![],
    );
    let mut sim = sim_of(&asset);
    sim.step(1.0, &FrameInput::default());
    assert!(close(sim.emitters[0].particles.velocity[0].x, 2.0), "implicit drag halves at drag*dt = 1");
}

#[test]
fn over_life_multipliers_scale_size_and_color() {
    let asset = simple_effect(
        &[
            ("spawn_rate", f(0.0)),
            ("burst_count", f(1.0)),
            ("lifetime", f(1.0)),
            ("size", PinValue::Vec2([2.0, 4.0])),
            ("color", PinValue::Color([1.0, 0.5, 0.25, 1.0])),
        ],
        vec![VfxNode::new(2, "input/life"), VfxNode::new(3, "math/color").with("value", PinValue::Color([0.5; 4]))],
        vec![
            Connection::new(2, "value", 1, "size_over_life"),
            Connection::new(3, "value", 1, "color_over_life"),
        ],
    );
    let mut sim = sim_of(&asset);
    sim.step(0.25, &FrameInput::default());
    let p = &sim.emitters[0].particles;
    assert!(p.size[0].abs_diff_eq(bevy::math::Vec2::new(0.5, 1.0), 1e-5));
    assert!(p.color[0].abs_diff_eq(Vec4::new(0.5, 0.25, 0.125, 0.5), 1e-5));
    assert!(p.base_size[0].abs_diff_eq(bevy::math::Vec2::new(2.0, 4.0), 1e-6));
}

fn noisy_effect(seed: u64) -> VfxAsset {
    let mut asset = parse_vfx(PRESETS[0].source).expect("campfire");
    for e in &mut asset.emitters {
        e.seed = seed;
    }
    asset
}

fn snapshot(sim: &EffectSim) -> Vec<crate::sim::ParticleBuffer> {
    sim.emitters.iter().map(|e| e.particles.clone()).collect()
}

#[test]
fn the_same_seed_gives_the_same_particles() {
    let frame = FrameInput::default();
    let mut a = sim_of(&noisy_effect(9));
    let mut b = sim_of(&noisy_effect(9));
    let mut c = sim_of(&noisy_effect(10));
    steps(&mut a, 1.0 / 60.0, 90, &frame);
    steps(&mut b, 1.0 / 60.0, 90, &frame);
    steps(&mut c, 1.0 / 60.0, 90, &frame);
    assert!(a.alive() > 0);
    assert_eq!(snapshot(&a), snapshot(&b));
    assert_ne!(snapshot(&a), snapshot(&c));
}

#[test]
fn simulate_to_equals_stepping() {
    let frame = FrameInput {
        transform: Affine3A::from_rotation_translation(Quat::from_rotation_y(0.7), Vec3::new(3.0, 1.0, -2.0)),
        emitter_velocity: Vec3::ZERO,
        ..Default::default()
    };
    let mut stepped = sim_of(&noisy_effect(4));
    steps(&mut stepped, SEEK_STEP, 150, &frame);
    let mut seeked = sim_of(&noisy_effect(4));
    seeked.simulate_to(150.0 * SEEK_STEP, &frame);
    assert_eq!(snapshot(&stepped), snapshot(&seeked));
    assert_eq!(stepped.time, seeked.time);

    // Seeking is a replay: seeking back reproduces the earlier state too.
    let mut early = sim_of(&noisy_effect(4));
    steps(&mut early, SEEK_STEP, 30, &frame);
    seeked.simulate_to(30.0 * SEEK_STEP, &frame);
    assert_eq!(snapshot(&early), snapshot(&seeked));
}

#[test]
fn a_long_seek_is_capped() {
    let mut sim = sim_of(&noisy_effect(1));
    sim.simulate_to(10_000.0, &FrameInput::default());
    assert!((sim.time - 10_000.0).abs() < 1.0, "{}", sim.time);
}

#[test]
fn local_and_world_space() {
    let make = |space| {
        let mut asset = simple_effect(
            &[
                ("spawn_rate", f(0.0)),
                ("burst_count", f(1.0)),
                ("lifetime", f(10.0)),
                ("position", v3(0.0, 1.0, 0.0)),
                ("velocity", v3(1.0, 0.0, 0.0)),
            ],
            vec![],
            vec![],
        );
        asset.emitters[0].space = space;
        sim_of(&asset)
    };
    let spawn_frame = FrameInput {
        transform: Affine3A::from_rotation_translation(Quat::from_rotation_z(PI / 2.0), Vec3::new(10.0, 0.0, 0.0)),
        emitter_velocity: Vec3::ZERO,
        ..Default::default()
    };
    let moved = FrameInput {
        transform: Affine3A::from_translation(Vec3::new(-50.0, 0.0, 0.0)),
        emitter_velocity: Vec3::ZERO,
        ..Default::default()
    };

    let mut local = make(SimulationSpace::Local);
    local.step(1.0, &spawn_frame);
    local.step(1.0, &moved);
    let p = &local.emitters[0].particles;
    assert!(p.position[0].abs_diff_eq(Vec3::new(2.0, 1.0, 0.0), 1e-4), "local ignores the transform: {:?}", p.position[0]);

    let mut world = make(SimulationSpace::World);
    world.step(1.0, &spawn_frame);
    world.step(1.0, &moved);
    let p = &world.emitters[0].particles;
    // Born at the transformed (0,1,0) = (9,0,0) with velocity rotated to +Y,
    // then left alone when the emitter moved.
    assert!(p.position[0].abs_diff_eq(Vec3::new(9.0, 2.0, 0.0), 1e-4), "world: {:?}", p.position[0]);
    assert!(p.velocity[0].abs_diff_eq(Vec3::new(0.0, 1.0, 0.0), 1e-4));
}

#[test]
fn emitter_velocity_is_inherited_only_when_wired() {
    let frame = FrameInput {
        transform: Affine3A::IDENTITY,
        emitter_velocity: Vec3::new(5.0, 0.0, 0.0),
        ..Default::default()
    };
    let base = [("spawn_rate", f(0.0)), ("burst_count", f(1.0)), ("lifetime", f(10.0))];
    let mut plain = sim_of(&simple_effect(&base, vec![], vec![]));
    plain.step(0.1, &frame);
    assert_eq!(plain.emitters[0].particles.velocity[0], Vec3::ZERO);

    let mut inherit = sim_of(&simple_effect(
        &base,
        vec![VfxNode::new(2, "input/emitter_velocity")],
        vec![Connection::new(2, "value", 1, "velocity")],
    ));
    inherit.step(0.1, &frame);
    assert!(inherit.emitters[0].particles.velocity[0].abs_diff_eq(Vec3::new(5.0, 0.0, 0.0), 1e-5));
}

#[test]
fn a_one_shot_finishes() {
    let asset = parse_vfx(PRESETS[1].source).expect("sparks");
    assert!(!asset.looping);
    let mut sim = sim_of(&asset);
    let frame = FrameInput::default();
    steps(&mut sim, 1.0 / 60.0, 6, &frame);
    assert!(sim.alive() > 100);
    assert!(!sim.is_finished());
    steps(&mut sim, 1.0 / 60.0, 120, &frame);
    assert!(sim.is_finished());
}

#[test]
fn every_preset_compiles_and_simulates_three_seconds_in_wind_without_nans() {
    for preset in PRESETS {
        let asset = parse_vfx(preset.source).expect("parse");
        let (mut sim, failures) = EffectSim::new(&asset);
        if let Some(f) = failures.first() {
            panic!("{}: emitter {} failed: {:?}", preset.name, f.name, f.errors);
        }
        assert_eq!(sim.emitters.len(), asset.emitters.len());
        let mut peak = 0;
        for k in 0..180 {
            // A moving, turning emitter, so world-space paths are exercised,
            // in a gusting turbulent wind, so the wind force is too.
            let t = k as f32 / 60.0;
            let frame = FrameInput {
                transform: Affine3A::from_rotation_translation(
                    Quat::from_rotation_y(t),
                    Vec3::new(t.sin() * 3.0, 0.0, t.cos()),
                ),
                emitter_velocity: Vec3::new(t.cos() * 3.0, 0.0, -t.sin()),
                wind: Vec3::new(6.0, 0.0, 2.0) * (1.0 + 0.5 * (t * 1.3).sin()),
                wind_turbulence: 0.6,
            };
            sim.step(1.0 / 60.0, &frame);
            peak = peak.max(sim.alive());
            for e in &sim.emitters {
                let p = &e.particles;
                let finite = p.position.iter().all(|v| v.is_finite())
                    && p.velocity.iter().all(|v| v.is_finite())
                    && p.size.iter().all(|v| v.is_finite())
                    && p.color.iter().all(|v| v.is_finite())
                    && p.rotation.iter().all(|v| v.is_finite())
                    && p.age.iter().all(|v| v.is_finite())
                    && p.lifetime.iter().all(|v| v.is_finite());
                assert!(finite, "{}: non-finite particle at step {k}", preset.name);
            }
        }
        assert!(peak > 0, "{} never spawned anything", preset.name);
    }
}

// ── Mesh building ───────────────────────────────────────────────────────────

#[test]
fn meshes_hold_four_vertices_per_particle_and_sane_bounds() {
    use bevy::mesh::Mesh;
    let asset = simple_effect(
        &[
            ("spawn_rate", f(0.0)),
            ("burst_count", f(3.0)),
            ("lifetime", f(10.0)),
            ("size", PinValue::Vec2([2.0, 2.0])),
            ("position", v3(5.0, 0.0, 0.0)),
        ],
        vec![],
        vec![],
    );
    let mut sim = sim_of(&asset);
    sim.step(0.1, &FrameInput::default());
    let mut mesh = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    let render = RenderSettings::default();
    let bounds = crate::render::write_mesh(&mut mesh, &sim.emitters[0].particles, &render, Affine3A::IDENTITY)
        .expect("bounds");
    assert_eq!(mesh.count_vertices(), 12);
    assert_eq!(mesh.indices().map(|i| i.len()), Some(18));
    let min = Vec3::from(bounds.min());
    let max = Vec3::from(bounds.max());
    assert!(min.x <= 4.0 && max.x >= 6.0, "padded by the particle's radius: {min} {max}");

    // Writing again reuses the mesh and an empty buffer leaves one hidden quad.
    let empty = crate::sim::ParticleBuffer::default();
    assert!(crate::render::write_mesh(&mut mesh, &empty, &render, Affine3A::IDENTITY).is_none());
    assert_eq!(mesh.count_vertices(), 4);
}

#[test]
fn world_space_particles_are_drawn_relative_to_the_entity() {
    use bevy::prelude::{GlobalTransform, Transform};
    let global = GlobalTransform::from(Transform::from_xyz(10.0, 0.0, 0.0));
    let local = Transform::default();
    let m = crate::render::sim_to_mesh(SimulationSpace::World, &global, &local).expect("invertible");
    assert!(m.transform_point3(Vec3::new(10.0, 1.0, 0.0)).abs_diff_eq(Vec3::new(0.0, 1.0, 0.0), 1e-5));
    let m = crate::render::sim_to_mesh(SimulationSpace::Local, &global, &local).expect("invertible");
    assert!(m.transform_point3(Vec3::new(1.0, 1.0, 0.0)).abs_diff_eq(Vec3::new(1.0, 1.0, 0.0), 1e-5));
    let zero = GlobalTransform::from(Transform::from_scale(Vec3::ZERO));
    assert!(crate::render::sim_to_mesh(SimulationSpace::World, &zero, &local).is_none());
}

// ── Wind ────────────────────────────────────────────────────────────────────

#[test]
fn the_wind_node_reads_the_world_wind_with_a_bounded_cross_wind() {
    let u = Context::Update;
    // Strength is the bare speed over the reference: |(3, 0, 4)| = 5, scaled 2.
    let node = VfxNode::new(2, "input/wind").with("scale", f(2.0));
    let s = eval_node(node.clone(), "strength", ValueType::Float, u);
    assert!(close(s.x, 10.0 / renzora::wind::REFERENCE_WIND_SPEED), "{s}");

    let base = TEST_WIND * 2.0;
    let speed = base.length();
    let v = eval_node_n(node, "velocity", ValueType::Vec3, u, 16);
    let mut distinct = HashSet::new();
    for w in &v {
        let d = w.truncate() - base;
        // The wobble is at most the turbulence times the speed (two sines
        // weighted 0.65 + 0.35 across, 0.3 up), and its horizontal part is a
        // cross-wind, at right angles to the wind.
        assert!(d.length() <= TEST_TURBULENCE * speed * 1.05 + 1e-4, "{d}");
        assert!(d.y.abs() <= 0.3 * TEST_TURBULENCE * speed + 1e-4);
        let across = Vec3::new(d.x, 0.0, d.z);
        assert!(across.dot(base).abs() < 1e-3 * speed * speed, "not across the wind: {d}");
        distinct.insert(w.truncate().to_array().map(f32::to_bits));
    }
    assert!(distinct.len() > 8, "every particle shares one gust");
}

#[test]
fn the_wind_force_relaxes_toward_the_wind_implicitly() {
    let u = Context::Update;
    let force = |coupling: f32, falloff: f32| {
        VfxNode::new(2, "force/wind")
            .with("position", v3(0.0, 1.0, 0.0))
            .with("velocity", v3(1.0, 0.0, 0.0))
            .with("wind", v3(5.0, 0.0, 0.0))
            .with("coupling", f(coupling))
            .with("height_falloff", f(falloff))
    };
    // (wind - v) * k / (1 + k dt) = 4 * 2 / 1.2.
    let a = eval_node(force(2.0, 0.0), "force", ValueType::Vec3, u);
    assert!(close3(a, Vec3::new(8.0 / 1.2, 0.0, 0.0)), "{a}");
    // Half the wind at half the falloff height: smoothstep(0, 2, 1) = 0.5.
    let a = eval_node(force(2.0, 2.0), "force", ValueType::Vec3, u);
    assert!(close3(a, Vec3::new(1.5 * 2.0 / 1.2, 0.0, 0.0)), "{a}");
    // No coupling, no force; a negative coupling is treated as none.
    assert!(close3(eval_node(force(0.0, 0.0), "force", ValueType::Vec3, u), Vec3::ZERO));
    assert!(close3(eval_node(force(-3.0, 0.0), "force", ValueType::Vec3, u), Vec3::ZERO));
}

/// One particle, born still, pushed by nothing but `force/wind`.
fn wind_effect(coupling: f32, space: SimulationSpace) -> VfxAsset {
    let mut asset = simple_effect(
        &[("spawn_rate", f(0.0)), ("burst_count", f(1.0)), ("lifetime", f(100.0))],
        vec![VfxNode::new(2, "force/wind").with("coupling", f(coupling))],
        vec![Connection::new(2, "force", 1, "acceleration")],
    );
    asset.emitters[0].space = space;
    asset
}

#[test]
fn the_wind_force_converges_a_particle_onto_the_wind() {
    let wind = Vec3::new(4.0, 0.0, -2.0);
    let frame = FrameInput { wind, ..Default::default() };
    let mut sim = sim_of(&wind_effect(2.0, SimulationSpace::World));
    steps(&mut sim, 1.0 / 60.0, 180, &frame);
    let v = sim.emitters[0].particles.velocity[0];
    // e^(-2 * 3) of the gap is left: 0.25 % of 4.5 m/s.
    assert!(v.abs_diff_eq(wind, 0.02), "{v}");

    // A stiff coupling on a long step reaches the wind without overshooting
    // it, which the explicit form would (k dt = 30).
    let mut stiff = sim_of(&wind_effect(1000.0, SimulationSpace::World));
    for _ in 0..10 {
        stiff.step(1.0 / 30.0, &frame);
        let v = stiff.emitters[0].particles.velocity[0];
        assert!(v.x <= wind.x + 1e-3 && v.x >= 0.0, "overshot: {v}");
    }
    assert!(stiff.emitters[0].particles.velocity[0].abs_diff_eq(wind, 0.05));

    // Calm air is plain drag: a particle at rest stays at rest.
    let mut calm = sim_of(&wind_effect(2.0, SimulationSpace::World));
    steps(&mut calm, 1.0 / 60.0, 60, &FrameInput::default());
    assert_eq!(calm.emitters[0].particles.velocity[0], Vec3::ZERO);
}

#[test]
fn a_local_emitter_feels_the_wind_in_its_own_space() {
    // The entity is turned a quarter about Y, so the world's +X wind arrives
    // along the emitter's own +Z; drawn through the entity it is +X again.
    let transform = Affine3A::from_rotation_y(PI / 2.0);
    let wind = Vec3::new(3.0, 0.0, 0.0);
    let frame = FrameInput { wind, ..FrameInput::at(transform) };
    let mut sim = sim_of(&wind_effect(5.0, SimulationSpace::Local));
    steps(&mut sim, 1.0 / 60.0, 240, &frame);
    let local = sim.emitters[0].particles.velocity[0];
    assert!(transform.transform_vector3(local).abs_diff_eq(wind, 0.02), "{local}");
    assert!(!local.abs_diff_eq(wind, 0.5), "the wind was not taken into the emitter's space");
}

#[test]
fn frame_input_takes_the_gusting_wind_from_the_wind_state() {
    let state = renzora::wind::WindState {
        direction: bevy::math::Vec2::new(0.0, 1.0),
        speed: 4.0,
        gust_strength: 0.5,
        gust: 1.0,
        turbulence: 0.3,
        ..Default::default()
    };
    let frame = FrameInput::default().with_wind(&state);
    // A full gust at depth 0.5 lifts 4 m/s to 6, toward +Z.
    assert!(frame.wind.abs_diff_eq(Vec3::new(0.0, 0.0, 6.0), 1e-5), "{}", frame.wind);
    assert!(close(frame.wind_turbulence, 0.3));
    // No wind plugin: the default state is dead calm.
    let calm = FrameInput::default().with_wind(&renzora::wind::WindState::default());
    assert_eq!(calm.wind, Vec3::ZERO);
}

// ── Blackbody ───────────────────────────────────────────────────────────────

#[test]
fn blackbody_matches_the_planckian_locus_at_known_temperatures() {
    use crate::blackbody::blackbody_rgb;
    // Mitchell Charity's blackbody table (CIE 1931 2 degree, Rec.709, D65),
    // decoded to linear and normalised to the brightest channel.
    let cases: [(f32, [f32; 3], f32); 5] = [
        (1000.0, [1.0, 0.040, 0.0], 0.03),
        (2000.0, [1.0, 0.250, 0.006], 0.04),
        (3000.0, [1.0, 0.456, 0.147], 0.05),
        (6500.0, [1.0, 0.947, 0.980], 0.06),
        (10000.0, [0.585, 0.694, 1.0], 0.07),
    ];
    for (kelvin, want, tol) in cases {
        let got = blackbody_rgb(kelvin);
        for i in 0..3 {
            assert!((got[i] - want[i]).abs() <= tol, "{kelvin} K: got {got:?}, want {want:?}");
        }
    }
    // Hotter is never redder: green climbs monotonically through flame range.
    let mut last = 0.0;
    for k in (800..=4000).step_by(200) {
        let g = blackbody_rgb(k as f32)[1];
        assert!(g >= last, "green fell at {k} K");
        last = g;
    }
}

#[test]
fn the_blackbody_node_scales_rgb_by_intensity_and_keeps_alpha() {
    let u = Context::Update;
    let node = VfxNode::new(2, "color/blackbody").with("temperature", f(2000.0)).with("intensity", f(3.0));
    let c = eval_node(node, "color", ValueType::Color, u);
    let want = crate::blackbody::blackbody_rgb(2000.0);
    for i in 0..3 {
        assert!((c[i] - 3.0 * want[i]).abs() < 0.03, "{c} vs {want:?}");
    }
    assert!(close(c.w, 1.0));
    // Out of range clamps to the table's ends rather than extrapolating.
    let cold = eval_node(VfxNode::new(2, "color/blackbody").with("temperature", f(10.0)), "color", ValueType::Color, u);
    assert!(cold.is_finite() && close(cold.x, 1.0) && cold.y < 0.05);
}

// ── Realistic Fire ──────────────────────────────────────────────────────────

pub(crate) fn realistic_fire() -> VfxAsset {
    let preset = PRESETS.iter().find(|p| p.name == "Realistic Fire").expect("the preset exists");
    parse_vfx(preset.source).expect("parse")
}

pub(crate) fn layer<'a>(sim: &'a EffectSim, asset: &VfxAsset, name: &str) -> &'a crate::sim::ParticleBuffer {
    let i = asset.emitters.iter().position(|e| e.name == name).unwrap_or_else(|| panic!("no layer {name}"));
    &sim.emitters.iter().find(|e| e.emitter_index == i).expect("compiled").particles
}

#[test]
fn realistic_fire_has_its_five_layers_drawn_the_right_way() {
    let asset = realistic_fire();
    let names: Vec<&str> = asset.emitters.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["flame core", "outer flames", "smoke", "embers", "coals"]);
    let render = |n: &str| &asset.emitters.iter().find(|e| e.name == n).unwrap().render;
    for n in ["flame core", "outer flames"] {
        assert_eq!(render(n).shading, Shading::ProceduralFlame);
        assert_eq!(render(n).blend, BlendMode::Additive);
        assert_eq!(render(n).facing, Facing::VelocityAligned);
    }
    assert_eq!(render("smoke").shading, Shading::ProceduralSmoke);
    assert_eq!(render("smoke").blend, BlendMode::Alpha);
    // Every layer drags through the wind.
    for e in &asset.emitters {
        assert!(e.graph.nodes.iter().any(|n| n.node_type == "force/wind"), "{} ignores the wind", e.name);
    }
}

#[test]
fn realistic_fire_is_log_fire_sized_and_its_layers_take_the_wind_by_weight() {
    let asset = realistic_fire();
    // Calm: the flames rise to about the reference's height.
    let mut calm = sim_of(&asset);
    steps(&mut calm, 1.0 / 60.0, 180, &FrameInput::default());
    // Particle centres; a tongue reaches about 0.2 m above its centre.
    let top = |n: &str| layer(&calm, &asset, n).position.iter().map(|p| p.y).fold(0.0f32, f32::max);
    let (core, outer) = (top("flame core"), top("outer flames"));
    assert!((0.45..1.0).contains(&core), "flame core tops out at {core} m");
    assert!((0.3..0.9).contains(&outer), "outer flames top out at {outer} m");
    assert!(!layer(&calm, &asset, "smoke").is_empty(), "no smoke after three seconds");

    // Windy: smoke rides it, embers drift, flames lean, coals stay put.
    let mut windy = sim_of(&asset);
    let frame = FrameInput { wind: Vec3::new(4.0, 0.0, 0.0), ..Default::default() };
    steps(&mut windy, 1.0 / 60.0, 180, &frame);
    let mean_vx = |n: &str| {
        let p = layer(&windy, &asset, n);
        assert!(!p.is_empty(), "{n} is empty");
        p.velocity.iter().map(|v| v.x).sum::<f32>() / p.len() as f32
    };
    let (smoke, embers, flames, coals) =
        (mean_vx("smoke"), mean_vx("embers"), mean_vx("flame core"), mean_vx("coals"));
    assert!(smoke > embers && embers > flames && flames > coals, "{smoke} {embers} {flames} {coals}");
    assert!(smoke > 2.5 && smoke <= 4.5, "smoke {smoke}");
    assert!(coals < 0.2, "coals {coals}");
}

// ── Render settings ─────────────────────────────────────────────────────────

#[test]
fn new_render_fields_are_not_written_until_used() {
    let text = write_vfx(&simple_effect(&[], vec![], vec![])).expect("serialize");
    for field in ["shading", "stretch", "frame_blend", "erosion", "soft_distance"] {
        assert!(!text.contains(field), "{field} written at its default:\n{text}");
    }
    let mut asset = simple_effect(&[], vec![], vec![]);
    asset.emitters[0].render.shading = Shading::ProceduralSmoke;
    asset.emitters[0].render.erosion = 0.5;
    let again = parse_vfx(&write_vfx(&asset).expect("serialize")).expect("parse");
    assert_eq!(again.emitters[0].render, asset.emitters[0].render);
}

#[test]
fn blended_frames_keep_their_fraction_and_reach_the_last_frame_at_death() {
    let mut render = RenderSettings {
        sprite_sheet: SpriteSheet { columns: 4, rows: 2, mode: SheetMode::OverLifetime },
        ..Default::default()
    };
    let frame = crate::render::frame;
    assert_eq!(frame(&render, 0.5, 1.0), 4.0);
    render.frame_blend = true;
    assert!(close(frame(&render, 0.5, 1.0), 3.5));
    assert!(close(frame(&render, 1.0, 1.0), 7.0));
    render.sprite_sheet.mode = SheetMode::Fps(10.0);
    // 1.25 s at 10 fps is 12.5 frames, wrapped into 8.
    assert!(close(frame(&render, 1.25, 9.0), 4.5));
    render.frame_blend = false;
    assert_eq!(frame(&render, 1.25, 9.0), 4.0);
}

#[test]
fn meshes_carry_a_seed_and_life_and_stretch_along_velocity() {
    use bevy::mesh::{Mesh, VertexAttributeValues};
    let asset = simple_effect(
        &[
            ("spawn_rate", f(0.0)),
            ("burst_count", f(2.0)),
            ("lifetime", f(4.0)),
            ("size", PinValue::Vec2([0.1, 0.2])),
            ("velocity", v3(0.0, 3.0, 0.0)),
        ],
        vec![],
        vec![],
    );
    let mut sim = sim_of(&asset);
    sim.step(1.0, &FrameInput::default());
    let mut mesh = Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default());
    let render = RenderSettings { facing: Facing::VelocityAligned, stretch: 0.1, ..Default::default() };
    crate::render::write_mesh(&mut mesh, &sim.emitters[0].particles, &render, Affine3A::IDENTITY).expect("bounds");
    let Some(VertexAttributeValues::Float32x4(extra)) = mesh.attribute(crate::render::ATTRIBUTE_PARTICLE_EXTRA) else {
        panic!("no extra attribute");
    };
    let Some(VertexAttributeValues::Float32x4(params)) = mesh.attribute(crate::render::ATTRIBUTE_PARTICLE_PARAMS) else {
        panic!("no params attribute");
    };
    // One second into a four-second life.
    assert!(close(extra[0][1], 0.25));
    // All four corners of a quad agree on the seed; two particles differ.
    assert_eq!(extra[0][0], extra[3][0]);
    assert_ne!(extra[0][0], extra[4][0]);
    assert!((0.0..1.0).contains(&extra[0][0]));
    // 0.2 tall plus 3 m/s for 0.1 s.
    assert!(close(params[0][1], 0.5), "{:?}", params[0]);
    assert!(close(params[0][0], 0.1));
}
