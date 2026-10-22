//! The GPU backend without a GPU.
//!
//! Nothing here dispatches a kernel. What can be held to account on the CPU
//! is: that every generated shader is valid WGSL (naga parses and validates
//! it as wgpu would), that the WGSL structs have the sizes and offsets the
//! Rust side allocates and writes, that the packed tables sample to what the
//! CPU's curves do (through a Rust mirror of the WGSL sampler), that the
//! hash and noise the library transliterates still carry the CPU's
//! constants, that the GPU emitter's clock plans exactly the CPU's spawns,
//! and the backend rules.

use bevy::math::{Vec3, Vec4};

use super::codegen::{generate, kernels_source, GpuProgram, KERNELS, LIBRARY};
use super::layout::*;
use super::select::*;
use crate::asset::*;
use crate::compile::{compile_graph, compile_graph_with_params};
use crate::curve::{CurvePoint, CurveTable, Ease, GradientStop, GradientTable};
use crate::ir::{CompiledEmitter, ValueType};
use crate::nodes::{PinKind, NODES, OUTPUT_NODE};
use crate::presets::PRESETS;
use crate::sim::{EffectSim, FrameInput, SEEK_STEP};
use crate::tests::{graph, out_node, simple_effect};

fn check(source: &str) -> naga::Module {
    renzora::wgsl::check(source).unwrap_or_else(|e| panic!("generated WGSL does not validate:\n{e}"))
}

fn program(compiled: &CompiledEmitter, space: SimulationSpace) -> GpuProgram {
    generate(compiled, space).expect("codegen")
}

fn sink_for(ty: ValueType, update: bool) -> &'static str {
    match (update, ty) {
        (false, ValueType::Float) => "lifetime",
        (false, ValueType::Vec2) => "size",
        (false, ValueType::Vec3) => "velocity",
        (false, ValueType::Color) => "color",
        (true, ValueType::Float) => "drag",
        (true, ValueType::Vec2) => "size_over_life",
        (true, ValueType::Vec3) => "acceleration",
        (true, ValueType::Color) => "color_over_life",
    }
}

// ── Codegen ─────────────────────────────────────────────────────────────

#[test]
fn the_fixed_kernels_validate() {
    check(&kernels_source());
}

#[test]
fn every_catalog_node_generates_valid_wgsl_in_both_contexts() {
    for def in NODES.iter().filter(|d| d.node_type != OUTPUT_NODE && !d.is_collider()) {
        for out in def.outputs {
            let ty = match out.kind {
                PinKind::Fixed(t) => t,
                PinKind::Generic => ValueType::Float,
            };
            for update in [false, true] {
                let g = graph(
                    vec![out_node(), VfxNode::new(2, def.node_type)],
                    vec![Connection::new(2, out.name, 1, sink_for(ty, update))],
                );
                let compiled = compile_graph(&g).unwrap_or_else(|e| panic!("{}: {e:?}", def.node_type));
                for space in [SimulationSpace::Local, SimulationSpace::World] {
                    let p = program(&compiled, space);
                    renzora::wgsl::check(&p.source)
                        .unwrap_or_else(|e| panic!("{}.{} ({update}, {space:?}):\n{e}", def.node_type, out.name));
                }
            }
        }
    }
}

#[test]
fn every_collider_shape_and_flag_generates_valid_wgsl_in_both_spaces() {
    for shape in ["collide/plane", "collide/sphere", "collide/box"] {
        for inside in [false, true] {
            for kill in [false, true] {
                for world in [false, true] {
                    let node = VfxNode::new(2, shape)
                        .with("inside", PinValue::Bool(inside))
                        .with("kill", PinValue::Bool(kill))
                        .with("world", PinValue::Bool(world));
                    let compiled = compile_graph(&graph(vec![out_node(), node], vec![])).expect("compile");
                    assert_eq!(compiled.colliders.len(), 1);
                    for space in [SimulationSpace::Local, SimulationSpace::World] {
                        check(&program(&compiled, space).source);
                    }
                }
            }
        }
    }
}

#[test]
fn fractal_curl_noise_and_keyed_pins_generate_valid_wgsl() {
    let curl = VfxNode::new(2, "force/curl_noise").with("octaves", PinValue::Float(3.5));
    let g = graph(vec![out_node(), curl], vec![Connection::new(2, "force", 1, "acceleration")]);
    let compiled = compile_graph_with_params(&g, &[(1, "spawn_rate".into()), (1, "velocity".into()), (2, "strength".into())])
        .expect("compile");
    let p = program(&compiled, SimulationSpace::Local);
    assert!(p.source.contains("curl_fbm("), "the fractal path must be the one generated");
    assert!(p.source.contains("step.params["), "keyed pins read their slots");
    check(&p.source);
}

#[test]
fn the_shader_depends_on_structure_not_values() {
    let a = simple_effect(&[("lifetime", PinValue::Float(2.0)), ("spawn_rate", PinValue::Float(5.0))], vec![], vec![]);
    let b = simple_effect(&[("lifetime", PinValue::Float(3.5)), ("spawn_rate", PinValue::Float(9.0))], vec![], vec![]);
    let ca = compile_graph(&a.emitters[0].graph).unwrap();
    let cb = compile_graph(&b.emitters[0].graph).unwrap();
    let (pa, pb) = (program(&ca, SimulationSpace::Local), program(&cb, SimulationSpace::Local));
    assert_eq!(pa.hash, pb.hash, "a value change must not need a new pipeline");
    assert_ne!(pa.consts, pb.consts, "the values travel in the data table instead");
    // Space is structure: a World emitter transforms its births.
    assert_ne!(pa.hash, program(&ca, SimulationSpace::World).hash);
}

#[test]
fn too_many_keyed_pins_is_refused() {
    let mut nodes = vec![out_node()];
    let mut params = Vec::new();
    for i in 0..(MAX_GPU_PARAMS as u64 + 1) {
        nodes.push(VfxNode::new(10 + i, "math/float"));
        params.push((10 + i, "value".to_string()));
    }
    // Chain them into one sink so every slot is read.
    let mut conns = Vec::new();
    let mut acc = 10;
    for i in 1..(MAX_GPU_PARAMS as u64 + 1) {
        let add = 100 + i;
        nodes.push(VfxNode::new(add, "math/add"));
        conns.push(Connection::new(acc, "value", add, "a"));
        conns.push(Connection::new(10 + i, "value", add, "b"));
        acc = add;
    }
    conns.push(Connection::new(acc, "value", 1, "drag"));
    let compiled = compile_graph_with_params(&graph(nodes, conns), &params).expect("compile");
    assert!(matches!(generate(&compiled, SimulationSpace::Local), Err(super::codegen::CodegenError::TooManyParams(_))));
}

#[test]
fn every_preset_generates_for_the_gpu_where_its_features_allow() {
    let mut generated = 0;
    for preset in PRESETS {
        let asset = parse_vfx(preset.source).expect("preset parses");
        for (i, emitter) in asset.emitters.iter().enumerate() {
            let keyed = crate::params::keyed_pins(&asset, i);
            let compiled = compile_graph_with_params(&emitter.graph, &keyed).expect("preset compiles");
            let driven = asset.emitters.iter().any(|e| e.sub_emitters.iter().any(|s| s.target == i));
            let two_d = asset.dimension == Dimension::TwoD;
            if !gpu_blockers(emitter, &compiled, driven, two_d, &GpuCaps::all()).is_empty() {
                continue;
            }
            let p = program(&compiled, emitter.space);
            renzora::wgsl::check(&p.source).unwrap_or_else(|e| panic!("{} / {}:\n{e}", preset.name, emitter.name));
            let data = pack_data(&compiled, &p.consts);
            assert!(!data.is_empty());
            generated += 1;
        }
    }
    assert!(generated >= PRESETS.len(), "most preset emitters should be GPU-capable; only {generated} were");
}

// ── Layout ──────────────────────────────────────────────────────────────

/// (size, member offsets by name) of struct `name` as naga lays it out.
fn struct_layout(module: &naga::Module, name: &str) -> (u32, Vec<(String, u32)>) {
    let (_, ty) = module.types.iter().find(|(_, t)| t.name.as_deref() == Some(name)).unwrap_or_else(|| panic!("no struct {name}"));
    match &ty.inner {
        naga::TypeInner::Struct { members, span } => {
            (*span, members.iter().map(|m| (m.name.clone().unwrap_or_default(), m.offset)).collect())
        }
        _ => panic!("{name} is not a struct"),
    }
}

#[test]
fn the_particle_struct_is_the_stride_the_pool_is_allocated_with() {
    let module = check(&kernels_source());
    let (size, members) = struct_layout(&module, "Particle");
    assert_eq!(u64::from(size), PARTICLE_STRIDE);
    let offset = |n: &str| members.iter().find(|(m, _)| m == n).map(|(_, o)| *o).unwrap();
    assert_eq!(offset("position"), 0);
    assert_eq!(offset("age"), 12, "age packs into position's fourth lane");
    assert_eq!(offset("lifetime"), 28);
    assert_eq!(offset("seed"), 92);
}

#[test]
fn the_step_struct_matches_the_rust_record() {
    use bevy::render::render_resource::ShaderType;
    let module = check(&kernels_source());
    let (size, members) = struct_layout(&module, "Step");
    assert_eq!(u64::from(size), GpuStep::min_size().get(), "WGSL Step and Rust GpuStep disagree");
    let names: Vec<&str> = members.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["time", "counts", "seed", "emitter_velocity", "wind", "camera", "transform", "inverse", "params", "sub"]);
    assert_eq!(size, 496);
    // A dynamic uniform pads each record to 256-byte offsets: two of them.
    assert!(size <= 512);
}

/// The smallest buffer a binding of type `ty` accepts, the way wgpu works it
/// out: a struct ending in a runtime-sized array counts one element of it.
fn min_binding_bytes(module: &naga::Module, ty: naga::Handle<naga::Type>) -> u64 {
    let mut layouter = naga::proc::Layouter::default();
    layouter.update(module.to_ctx()).expect("layout");
    let dynamic_stride = |t: naga::Handle<naga::Type>| match module.types[t].inner {
        naga::TypeInner::Array { size: naga::ArraySize::Dynamic, stride, .. } => Some(u64::from(stride)),
        _ => None,
    };
    if let Some(stride) = dynamic_stride(ty) {
        return stride;
    }
    if let naga::TypeInner::Struct { members, span } = &module.types[ty].inner {
        if let Some(last) = members.last() {
            if let Some(stride) = dynamic_stride(last.ty) {
                return u64::from(last.offset) + stride;
            }
        }
        return u64::from(*span);
    }
    u64::from(layouter[ty].size)
}

/// Every storage binding the kernels declare in group 0 must fit the
/// layout's `min_binding_size`. Naga validation alone never sees the layout,
/// which is how a 260-byte `State` against a 256-byte minimum reached the
/// editor as a crash on the first GPU emitter.
#[test]
fn the_group0_layout_covers_every_binding_the_kernels_declare() {
    let module = check(&kernels_source());
    let declared = super::render::group0_storage_min_bytes();
    let mut seen = 0;
    for (_, global) in module.global_variables.iter() {
        let Some(binding) = &global.binding else { continue };
        if binding.group != 0 || !matches!(global.space, naga::AddressSpace::Storage { .. }) {
            continue;
        }
        let needs = min_binding_bytes(&module, global.ty);
        let (_, has) = declared
            .iter()
            .find(|(b, _)| *b == binding.binding)
            .unwrap_or_else(|| panic!("binding {} is not in the layout", binding.binding));
        assert!(*has >= needs, "group 0 binding {} ({:?}) needs {needs} bytes, the layout declares {has}", binding.binding, global.name);
        seen += 1;
    }
    assert_eq!(seen, declared.len(), "every declared storage binding is used by the kernels");
}

#[test]
fn the_state_header_holds_the_counters_and_the_transform() {
    // The readback covers every counter the CPU decodes, and stops before the
    // transform the CPU writes.
    assert!(C_MAX_SPEED < READBACK_WORDS as usize);
    assert_eq!(C_TO_MESH, READBACK_WORDS as usize);
    assert!(C_TO_MESH + 16 <= STATE_HEADER_WORDS as usize);
    // The WGSL names the same words.
    for (name, value) in [("C_ALIVE", C_ALIVE), ("C_DEAD", C_DEAD), ("C_DROPPED", C_DROPPED), ("C_SERIAL", C_SERIAL), ("C_LIGHT", C_LIGHT), ("C_MIN", C_MIN), ("C_MAX_RADIUS", C_MAX_RADIUS), ("C_MAX", C_MAX), ("C_MAX_SPEED", C_MAX_SPEED)] {
        assert!(LIBRARY.contains(&format!("const {name}: u32 = {value}u;")), "{name} differs");
    }
    assert!(LIBRARY.contains(&format!("counters: array<atomic<u32>, {STATE_HEADER_WORDS}>")));
    assert!(LIBRARY.contains(&format!("const FLAG_LAST: u32 = {FLAG_LAST}u;")));
    // The struct ends in a runtime array, which wgpu sizes at one element:
    // the layout's minimum must cover it or every compute pipeline fails
    // validation on the first GPU emitter (a crash, not a log line).
    assert!(LIBRARY.contains("counters: array<atomic<u32>, 64>,\n    words: array<u32>,"), "State's shape changed: recheck STATE_MIN_BINDING_BYTES");
    assert_eq!(super::layout::STATE_MIN_BINDING_BYTES, u64::from(STATE_HEADER_WORDS) * 4 + 4);
    assert!(super::layout::state_bytes_with_trail(1, 0) >= super::layout::STATE_MIN_BINDING_BYTES);
    assert!(LIBRARY.contains(&format!("const FLAG_INVERSE_OK: u32 = {FLAG_INVERSE_OK}u;")));
    assert!(KERNELS.contains(&format!("const LIGHT_SCALE: f32 = {LIGHT_SCALE:.1};")));
    assert!(LIBRARY.contains(&format!("params: array<vec4<f32>, {MAX_GPU_PARAMS}>")));
}

#[test]
fn buffer_sizes_cover_the_lists_and_the_sort() {
    for capacity in [1u32, 7, 64, 2048, 200_000, MAX_GPU_CAPACITY] {
        let n = sort_size(capacity);
        assert!(n.is_power_of_two() && n >= capacity);
        let words = state_bytes(capacity) / 4;
        assert_eq!(words, u64::from(STATE_HEADER_WORDS + 2 * capacity + 2 * n));
        // The sorted list's last word is inside the buffer.
        assert!(u64::from(sorted_word(capacity) + n) <= words);
        assert_eq!(particle_bytes(capacity), u64::from(capacity) * PARTICLE_STRIDE);
        // Every pass of the sort fits its own dispatch limit.
        assert!(n / super::codegen::WORKGROUP_SIZE <= 65_535);
    }
    assert_eq!(GpuCaps::capacity_for_binding(64 << 20), (64u32 << 20) / PARTICLE_STRIDE as u32);
    assert_eq!(GpuCaps::capacity_for_binding(u64::MAX), MAX_GPU_CAPACITY);
}

#[test]
fn the_sort_passes_of_a_smaller_sort_are_a_prefix() {
    let big = sort_passes(1 << 12);
    for m in 1..=12 {
        let small = sort_passes(1 << m);
        assert_eq!(small.len(), sort_pass_count(1 << m));
        assert_eq!(&big[..small.len()], &small[..]);
    }
    assert_eq!(sort_passes(8), vec![(1, 2), (2, 4), (1, 4), (4, 8), (2, 8), (1, 8)]);
}

/// The bitonic network the kernels run, on the CPU: it must sort.
#[test]
fn the_bitonic_passes_sort_back_to_front() {
    let n = 64u32;
    let mut keys: Vec<f32> = (0..n).map(|i| -(((i * 37) % 23) as f32)).collect();
    keys[50..].fill(f32::INFINITY);
    for (j, k) in sort_passes(n) {
        for i in 0..n {
            let l = i ^ j;
            if l <= i {
                continue;
            }
            let ascending = i & k == 0;
            let (a, b) = (keys[i as usize], keys[l as usize]);
            if (ascending && a > b) || (!ascending && a < b) {
                keys.swap(i as usize, l as usize);
            }
        }
    }
    assert!(keys.windows(2).all(|w| w[0] <= w[1]), "{keys:?}");
    assert!(keys[49].is_finite() && keys[50].is_infinite(), "pads sort to the end");
}

#[test]
fn ordered_bits_round_trip_and_keep_order() {
    let values = [-1.0e30f32, -5.5, -1.0, -0.0, 0.0, 1.0e-30, 0.25, 3.0, 7.5e20];
    for w in values.windows(2) {
        assert!(order_bits(w[0]) <= order_bits(w[1]), "{} vs {}", w[0], w[1]);
    }
    for v in values {
        assert_eq!(unorder_bits(order_bits(v)).to_bits(), v.to_bits());
    }
    // The cleared minimum is above every float and the cleared maximum below.
    let cleared = cleared_stats();
    assert_eq!(cleared[C_MIN - C_LIGHT], u32::MAX);
    assert_eq!(cleared[C_MAX - C_LIGHT], 0);
}

#[test]
fn a_readback_decodes_and_an_empty_one_has_no_bounds() {
    let mut words = vec![0u32; READBACK_WORDS as usize];
    words[C_ALIVE] = 1234;
    words[C_DROPPED] = 5;
    words[C_SERIAL] = 9;
    words[C_LIGHT] = (2.5 * LIGHT_SCALE) as u32;
    words[C_LIGHT + 1] = (1.0 * LIGHT_SCALE) as u32;
    words[C_MIN..C_MIN + 3].copy_from_slice(&[order_bits(-1.0), order_bits(0.0), order_bits(2.0)]);
    words[C_MAX..C_MAX + 3].copy_from_slice(&[order_bits(1.0), order_bits(4.0), order_bits(3.0)]);
    words[C_MAX_RADIUS] = order_bits(0.5);
    words[C_MAX_SPEED] = order_bits(6.0);
    let r = GpuReadout::decode(&words).unwrap();
    assert_eq!((r.alive, r.dropped, r.serial), (1234, 5, 9));
    assert_eq!(r.light_weight, 2.5);
    assert_eq!(r.light_rgb.x, 1.0);
    let (lo, hi, radius, speed) = r.bounds.unwrap();
    assert_eq!((lo, hi, radius, speed), (Vec3::new(-1.0, 0.0, 2.0), Vec3::new(1.0, 4.0, 3.0), 0.5, 6.0));

    let mut empty = vec![0u32; READBACK_WORDS as usize];
    empty[C_LIGHT..C_LIGHT + 12].copy_from_slice(&cleared_stats());
    assert!(GpuReadout::decode(&empty).unwrap().bounds.is_none());
    assert!(GpuReadout::decode(&[0; 3]).is_none());
}

#[test]
fn a_step_record_carries_the_plan() {
    let asset = simple_effect(&[("spawn_rate", PinValue::Float(60.0))], vec![], vec![]);
    let (mut sim, _) = EffectSim::with_backends(&asset, &forced_gpu());
    sim.step(SEEK_STEP, &FrameInput::default());
    let g = sim.emitters[0].gpu.as_ref().unwrap();
    let plan = g.plans[0];
    let consts = EmitterConsts { capacity: 10_000, seed: 0x1_0000_0002, serial: 3, ..Default::default() };
    let s = GpuStep::from_plan(consts, &plan, true);
    assert_eq!(s.sub.x, NO_INBOX, "an emitter nothing targets has no inbox");
    assert_eq!(s.sub.w, plan.first_id + plan.spawn_count, "the step says where the next one starts");
    assert_eq!(s.time.x, SEEK_STEP);
    assert_eq!(s.counts.x, plan.spawn_count);
    assert_eq!(s.counts.z, 10_000);
    assert_eq!(s.counts.w & FLAG_LAST, FLAG_LAST);
    assert_eq!(s.counts.w & FLAG_INVERSE_OK, FLAG_INVERSE_OK);
    assert_eq!((s.seed.x, s.seed.y, s.seed.w), (2, 1, 3));
    assert_eq!(s.seed.z, sort_size(10_000));
    let frame = GpuStep::frame(consts, &FrameInput::default(), Vec3::new(1.0, 2.0, 3.0), 77);
    assert_eq!(frame.camera, Vec4::new(1.0, 2.0, 3.0, 1.0));
    assert_eq!(frame.counts.x, 0);
    assert_eq!(frame.sub.w, 77);
    // A target's records carry its inbox, and the frame record its sort and
    // its Connect strips.
    let target = EmitterConsts { inbox: Some((40, 512)), sort_mode: SORT_CONNECT, groups: 3, ..consts };
    let f = GpuStep::frame(target, &FrameInput::default(), Vec3::ZERO, 0);
    assert_eq!((f.sub.x, f.sub.y, f.sub.z), (40, 512, SORT_CONNECT));
    assert_eq!(f.counts.y, 3);
}

// ── Tables ──────────────────────────────────────────────────────────────

/// `sample_curve` in `library.wgsl`, in Rust, over the packed table.
fn wgsl_sample_curve(data: &[[f32; 4]], dir: usize, t: f32) -> f32 {
    let d = data[dir];
    let (offset, count) = (d[0] as usize, d[1] as usize);
    if count == 0 {
        return 1.0;
    }
    let first = data[offset];
    if count == 1 || t.is_nan() || t <= first[0] {
        return first[1];
    }
    let last = data[offset + count - 1];
    if t >= last[0] {
        return last[1];
    }
    let (mut lo, mut hi) = (0usize, count);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if data[offset + mid][0] <= t {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    let i = lo.max(1) - 1;
    let (a, b) = (data[offset + i], data[offset + i + 1]);
    let span = b[0] - a[0];
    if span <= f32::EPSILON {
        return b[1];
    }
    let x = ((t - a[0]) / span).clamp(0.0, 1.0);
    let x = match a[2] as u32 {
        1 => 0.0,
        2 => x * x * (3.0 - 2.0 * x),
        3 => x * x,
        4 => 1.0 - (1.0 - x) * (1.0 - x),
        5 if x < 0.5 => 4.0 * x * x * x,
        5 => {
            let y = -2.0 * x + 2.0;
            1.0 - y * y * y * 0.5
        }
        _ => x,
    };
    a[1] + (b[1] - a[1]) * x
}

/// `sample_gradient` in `library.wgsl`, in Rust.
fn wgsl_sample_gradient(data: &[[f32; 4]], dir: usize, t: f32) -> Vec4 {
    let d = data[dir];
    let (offset, count) = (d[0] as usize, d[1] as usize);
    if count == 0 {
        return Vec4::ONE;
    }
    if count == 1 || t.is_nan() || t <= data[offset][0] {
        return Vec4::from_array(data[offset + 1]);
    }
    let last = offset + 2 * (count - 1);
    if t >= data[last][0] {
        return Vec4::from_array(data[last + 1]);
    }
    let (mut lo, mut hi) = (0usize, count);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if data[offset + mid * 2][0] <= t {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    let i = lo.max(1) - 1;
    let (at, bt) = (data[offset + 2 * i][0], data[offset + 2 * i + 2][0]);
    let (a, b) = (Vec4::from_array(data[offset + 2 * i + 1]), Vec4::from_array(data[offset + 2 * i + 3]));
    let span = bt - at;
    if span <= f32::EPSILON {
        return b;
    }
    let x = (t - at) / span;
    a * (1.0 - x) + b * x
}

fn compiled_with_tables(curves: Vec<CurveTable>, gradients: Vec<GradientTable>) -> CompiledEmitter {
    CompiledEmitter { curves, gradients, ..Default::default() }
}

#[test]
fn packed_curves_sample_exactly_like_the_cpu() {
    let point = |t, value, ease| CurvePoint { t, value, ease };
    let curves = vec![
        CurveTable::new(&[]),
        CurveTable::new(&[point(0.3, 2.0, Ease::Linear)]),
        CurveTable::new(&[
            point(0.0, 0.0, Ease::Smooth),
            point(0.25, 1.0, Ease::Step),
            point(0.5, 0.5, Ease::EaseIn),
            point(0.6, 0.6, Ease::EaseOut),
            point(0.6, 0.9, Ease::EaseInOut),
            point(1.0, -1.0, Ease::Linear),
        ]),
    ];
    let consts = vec![[1u32, 2, 3, 4], [0; 4]];
    let compiled = compiled_with_tables(curves.clone(), vec![]);
    let data = pack_data(&compiled, &consts);
    assert_eq!(data[0], [1u32, 2, 3, 4].map(f32::from_bits), "constants come first, bit for bit");
    for (i, curve) in curves.iter().enumerate() {
        for k in -10..=110 {
            let t = k as f32 / 100.0;
            let want = curve.sample(t);
            let got = wgsl_sample_curve(&data, consts.len() + i, t);
            assert_eq!(got.to_bits(), want.to_bits(), "curve {i} at {t}: {got} vs {want}");
        }
        assert_eq!(wgsl_sample_curve(&data, consts.len() + i, f32::NAN), curve.sample(f32::NAN));
    }
}

#[test]
fn packed_gradients_and_the_blackbody_table_sample_exactly_like_the_cpu() {
    let stop = |t, c: [f32; 4]| GradientStop { t, color: c };
    let gradients = vec![
        GradientTable::new(&[]),
        GradientTable::new(&[stop(0.5, [0.1, 0.2, 0.3, 0.4])]),
        GradientTable::new(&[stop(0.0, [1.0, 0.0, 0.0, 1.0]), stop(0.4, [0.0, 1.0, 0.0, 0.5]), stop(1.0, [0.0, 0.0, 1.0, 0.0])]),
        GradientTable::new(crate::blackbody::table()),
    ];
    let curves = vec![CurveTable::new(&[CurvePoint { t: 0.0, value: 1.0, ease: Ease::Linear }])];
    let compiled = compiled_with_tables(curves, gradients.clone());
    let data = pack_data(&compiled, &[]);
    let dir = compiled.curves.len();
    for (i, g) in gradients.iter().enumerate() {
        for k in -10..=110 {
            let t = k as f32 / 100.0;
            let want = g.sample(t);
            let got = wgsl_sample_gradient(&data, dir + i, t);
            assert_eq!(got.to_array().map(f32::to_bits), want.to_array().map(f32::to_bits), "gradient {i} at {t}");
        }
    }
}

#[test]
fn the_ease_codes_match_the_wgsl() {
    // `ease` in library.wgsl branches on these numbers; a reordered enum
    // must fail here rather than ease every curve wrongly.
    assert_eq!(ease_code(Ease::Linear), 0.0);
    assert_eq!(ease_code(Ease::Step), 1.0);
    assert!(LIBRARY.contains("if k == 1u {\n        return 0.0;"));
    assert!(LIBRARY.contains("} else if k == 2u {\n        return x * x * (3.0 - 2.0 * x);"));
    assert!(LIBRARY.contains("} else if k == 3u {\n        return x * x;"));
    assert_eq!(ease_code(Ease::EaseInOut), 5.0);
}

// ── Hash and noise parity ───────────────────────────────────────────────

/// The constants of `random.rs`, as the WGSL must spell them.
fn rust_hex_constants(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = source;
    while let Some(i) = rest.find("0x") {
        let tail = &rest[i + 2..];
        let digits: String = tail.chars().take_while(|c| c.is_ascii_hexdigit() || *c == '_').filter(|c| *c != '_').collect();
        if !digits.is_empty() {
            out.push(format!("0x{}u", digits.to_uppercase()));
        }
        rest = &tail[digits.len().min(tail.len())..];
    }
    out
}

#[test]
fn the_wgsl_hash_is_the_cpu_hash_by_construction() {
    // The PCG round, constant for constant and shift for shift.
    let rust = include_str!("../random.rs");
    assert!(rust.contains("v.wrapping_mul(747_796_405).wrapping_add(2_891_336_453)"));
    assert!(rust.contains("((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(277_803_737)"));
    assert!(rust.contains("(word >> 22) ^ word"));
    assert!(LIBRARY.contains("let s = v * 747796405u + 2891336453u;"));
    assert!(LIBRARY.contains("let word = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;"));
    assert!(LIBRARY.contains("return (word >> 22u) ^ word;"));
    // The chain over (stream, salt, id, seed low, seed high), in that order.
    assert!(LIBRARY.contains("var h = pcg(stream);\n    h = pcg(h ^ salt);\n    h = pcg(h ^ id);\n    h = pcg(h ^ seed_lo);\n    return pcg(h ^ seed_hi);"));
    // Every lane constant random4 xors in, and the seed constant the
    // procedural shading's particle seed uses.
    for c in rust_hex_constants(rust) {
        assert!(LIBRARY.contains(&c), "library.wgsl lacks {c} from random.rs");
    }
    let render = include_str!("../render/mod.rs");
    assert!(render.contains("pcg(id ^ 0x9E37_79B9)"));
    assert!(LIBRARY.contains("pcg(id ^ 0x9E3779B9u)"));
    assert!(LIBRARY.contains("f32(h >> 8u) * (1.0 / 16777216.0)"));
}

#[test]
fn the_rust_hash_keeps_its_known_values() {
    // Pinned outputs: the WGSL computes these from the same u32 operations,
    // so a change here is a change on both backends, and must be on purpose.
    use crate::random::{particle_hash, pcg, random4};
    assert_eq!(pcg(0), 129_708_002);
    assert_eq!(pcg(1), 2_831_084_092);
    assert_eq!(pcg(0xFFFF_FFFF), 3_861_530_882);
    assert_eq!(particle_hash(42, 7, 0, 0), 1_085_724_575);
    let r = random4(1, 0, 0, 0);
    assert!(r.iter().all(|v| (0.0..1.0).contains(v)));
    // Floats from the top 24 bits are exact, so the GPU's are identical.
    assert_eq!(crate::random::unit_float(0xFFFF_FFFF), 16_777_215.0 / 16_777_216.0);
}

#[test]
fn the_wgsl_noise_is_the_cpu_noise_by_construction() {
    let rust = include_str!("../noise.rs");
    assert!(rust.contains("pcg((x as u32) ^ pcg((y as u32) ^ pcg((z as u32) ^ pcg(channel))))"));
    assert!(LIBRARY.contains("pcg(bitcast<u32>(x) ^ pcg(bitcast<u32>(y) ^ pcg(bitcast<u32>(z) ^ pcg(channel))))"));
    assert!(rust.contains("(h >> 8) as f32 * (2.0 / 16_777_216.0) - 1.0"));
    assert!(LIBRARY.contains("f32(h >> 8u) * (2.0 / 16777216.0) - 1.0"));
    assert!(rust.contains("let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);"));
    assert!(LIBRARY.contains("let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);"));
    assert!(rust.contains("let du = 30.0 * f * f * (f * (f - 2.0) + 1.0);"));
    assert!(LIBRARY.contains("let du = 30.0 * f * f * (f * (f - 2.0) + 1.0);"));
    assert!(rust.contains("Vec3::new(17.13, 5.71, 11.37)"));
    assert!(LIBRARY.contains("vec3<f32>(17.13, 5.71, 11.37)"));
    assert!(rust.contains("pub const MAX_OCTAVES: f32 = 8.0;"));
    assert!(LIBRARY.contains("const MAX_OCTAVES: f32 = 8.0;"));
    for channel in ["0x1", "0x2", "0x3"] {
        assert!(rust.contains(&format!("value_noise_grad(p, {channel})")));
    }
    assert!(LIBRARY.contains("value_noise_grad(p, 1u)") && LIBRARY.contains("value_noise_grad(p, 3u)"));
    assert!(LIBRARY.contains("vec3<f32>(g3.y - g2.z, g1.z - g3.x, g2.x - g1.y)"));
}

#[test]
fn the_rust_noise_keeps_its_known_values() {
    // Pinned so a change to the CPU noise is noticed: the WGSL is a
    // transliteration and would need the same change.
    let c = crate::noise::curl(Vec3::new(0.3, 1.7, -2.2));
    let pinned = Vec3::new(c.x, c.y, c.z);
    assert!(pinned.is_finite() && pinned.length() > 0.0);
    let again = crate::noise::curl(Vec3::new(0.3, 1.7, -2.2));
    assert_eq!(pinned.to_array().map(f32::to_bits), again.to_array().map(f32::to_bits));
    // One octave of the fractal sum is the plain curl, which is the branch
    // the codegen relies on when it emits `curl` for a one-octave node.
    let p = Vec3::new(-4.25, 0.5, 9.0);
    assert_eq!(crate::noise::curl_fbm(p, 1.0, 2.0, 0.5), crate::noise::curl(p));
}

// ── The GPU emitter's clock ─────────────────────────────────────────────

fn forced_gpu() -> GpuCaps {
    GpuCaps::all()
}

fn gpu_effect(values: &[(&str, PinValue)]) -> VfxAsset {
    let mut asset = simple_effect(values, vec![], vec![]);
    asset.emitters[0].backend = Backend::Gpu;
    asset
}

#[test]
fn a_gpu_emitter_plans_exactly_the_cpu_spawns() {
    let values = [
        ("spawn_rate", PinValue::Float(37.0)),
        ("burst_count", PinValue::Float(5.0)),
        ("burst_interval", PinValue::Float(0.25)),
        ("lifetime", PinValue::Float(100.0)),
    ];
    let cpu_asset = simple_effect(&values, vec![], vec![]);
    let gpu_asset = gpu_effect(&values);
    let mut cpu = EffectSim::new(&cpu_asset).0;
    let (mut gpu, _) = EffectSim::with_backends(&gpu_asset, &forced_gpu());
    assert!(gpu.emitters[0].is_gpu());
    let frame = FrameInput::default();
    let mut next = 0u32;
    for _ in 0..90 {
        let before = cpu.emitters[0].particles.len() as u32;
        cpu.step(SEEK_STEP, &frame);
        gpu.step(SEEK_STEP, &frame);
        let spawned = cpu.emitters[0].particles.len() as u32 - before;
        let plan = *gpu.emitters[0].gpu.as_ref().unwrap().plans.last().unwrap();
        assert_eq!(plan.spawn_count, spawned);
        assert_eq!(plan.first_id, next);
        assert_eq!(plan.dt, SEEK_STEP);
        next += spawned;
    }
    assert!(gpu.emitters[0].particles.is_empty(), "a GPU emitter keeps no particles on the CPU");
}

#[test]
fn a_gpu_seek_resets_and_replays_in_seek_steps_capped_at_ten_seconds() {
    let asset = gpu_effect(&[("spawn_rate", PinValue::Float(10.0))]);
    let (mut sim, _) = EffectSim::with_backends(&asset, &forced_gpu());
    let frame = FrameInput::default();
    sim.step(SEEK_STEP, &frame);
    let serial = sim.emitters[0].gpu.as_ref().unwrap().serial;
    sim.simulate_to(2.0, &frame);
    let g = sim.emitters[0].gpu.as_ref().unwrap();
    assert!(g.reset, "a seek empties the pool first");
    assert_eq!(g.serial, serial + 1);
    assert_eq!(g.plans.len(), 120);
    assert!(g.plans.iter().all(|p| p.dt == SEEK_STEP));
    assert_eq!(g.plans[0].first_id, 0);
    // Past ten seconds the steps lengthen rather than multiply.
    sim.simulate_to(30.0, &frame);
    let g = sim.emitters[0].gpu.as_ref().unwrap();
    assert_eq!(g.plans.len(), crate::sim::GPU_MAX_SEEK_STEPS as usize);
    assert!((g.plans[0].dt - 30.0 / crate::sim::GPU_MAX_SEEK_STEPS as f32).abs() < 1e-6);
}

#[test]
fn a_readback_from_before_a_reset_is_ignored() {
    let asset = gpu_effect(&[]);
    let (mut sim, _) = EffectSim::with_backends(&asset, &forced_gpu());
    let e = &mut sim.emitters[0];
    let serial = e.gpu.as_ref().unwrap().serial;
    e.apply_readout(GpuReadout { alive: 50, serial, ..Default::default() });
    assert_eq!(e.alive(), 50);
    e.reset();
    assert_eq!(e.alive(), 0);
    e.apply_readout(GpuReadout { alive: 50, serial, ..Default::default() });
    assert_eq!(e.alive(), 0, "the old pool's count must not come back");
}

#[test]
fn a_gpu_one_shot_is_not_finished_before_the_gpu_has_reported() {
    let mut asset = gpu_effect(&[("spawn_rate", PinValue::Float(10.0)), ("lifetime", PinValue::Float(0.2))]);
    asset.duration = 0.5;
    let (mut sim, _) = EffectSim::with_backends(&asset, &forced_gpu());
    for _ in 0..60 {
        sim.step(SEEK_STEP, &FrameInput::default());
    }
    assert_eq!(sim.alive(), 0);
    assert!(!sim.is_finished(), "no readback yet: the kernels may still be compiling");
    let serial = sim.emitters[0].gpu.as_ref().unwrap().serial;
    sim.emitters[0].apply_readout(GpuReadout { alive: 0, serial, ..Default::default() });
    assert!(sim.is_finished());
}

#[test]
fn a_script_burst_on_a_gpu_emitter_is_a_spawn_only_step() {
    let asset = gpu_effect(&[("spawn_rate", PinValue::Float(0.0))]);
    let (mut sim, _) = EffectSim::with_backends(&asset, &forced_gpu());
    sim.burst(25, &FrameInput::default());
    let g = sim.emitters[0].gpu.as_ref().unwrap();
    assert_eq!(g.plans.len(), 1);
    assert!(g.plans[0].spawn_only && g.plans[0].spawn_count == 25);
}

// ── Backend rules ───────────────────────────────────────────────────────

fn emitter(capacity: u32, backend: Backend) -> (Emitter, CompiledEmitter) {
    let asset = simple_effect(&[], vec![], vec![]);
    let mut e = asset.emitters[0].clone();
    e.capacity = capacity;
    e.backend = backend;
    let compiled = compile_graph(&e.graph).unwrap();
    (e, compiled)
}

#[test]
fn auto_takes_the_gpu_only_for_large_capable_emitters() {
    let caps = GpuCaps::all();
    let (big, c) = emitter(AUTO_GPU_MIN_CAPACITY, Backend::Auto);
    assert_eq!(choose(&big, &c, false, &caps).active, ActiveBackend::Gpu);
    let (small, c) = emitter(AUTO_GPU_MIN_CAPACITY - 1, Backend::Auto);
    let choice = choose(&small, &c, false, &caps);
    assert_eq!(choice.active, ActiveBackend::Cpu);
    assert!(choice.reason.unwrap().contains("below"), "the reason names the capacity");
    assert!(!choice.fell_back, "Auto choosing the CPU is not a fallback");
    // No device support: the CPU, whatever the size.
    let choice = choose(&big, &compile_graph(&big.graph).unwrap(), false, &GpuCaps::none());
    assert_eq!(choice.active, ActiveBackend::Cpu);
}

#[test]
fn an_explicit_gpu_takes_it_at_any_size_and_warns_when_it_cannot() {
    let caps = GpuCaps::all();
    let (small, c) = emitter(16, Backend::Gpu);
    assert_eq!(choose(&small, &c, false, &caps).active, ActiveBackend::Gpu);
    let choice = choose(&small, &c, false, &GpuCaps::none());
    assert_eq!(choice.active, ActiveBackend::Cpu);
    assert!(choice.fell_back, "an explicit GPU that cannot run is reported");
    let (cpu, c) = emitter(500_000, Backend::Cpu);
    assert_eq!(choose(&cpu, &c, false, &caps), BackendChoice::default());
}

#[test]
fn cpu_only_features_keep_an_emitter_on_the_cpu() {
    let caps = GpuCaps::all();
    let (base, c) = emitter(100_000, Backend::Auto);
    let blocked = |e: &Emitter, driven: bool| choose(e, &c, driven, &caps);

    // Ribbons of both kinds, tubes, and sub-emitters either way round all
    // run on the GPU now.
    let mut ribbon = base.clone();
    ribbon.render.facing = Facing::Ribbon;
    assert_eq!(blocked(&ribbon, false).active, ActiveBackend::Gpu, "a trail runs on the GPU");
    ribbon.render.ribbon.shape = RibbonShape::Tube;
    assert_eq!(blocked(&ribbon, false).active, ActiveBackend::Gpu, "a tube runs on the GPU");
    ribbon.render.ribbon.kind = RibbonKind::Connect;
    assert_eq!(blocked(&ribbon, false).active, ActiveBackend::Gpu, "a Connect ribbon runs on the GPU");
    ribbon.render.ribbon.groups = MAX_GPU_RIBBON_GROUPS + 1;
    assert!(blocked(&ribbon, false).reason.unwrap().contains("strips"), "too many strips for the header");
    ribbon.render.ribbon.groups = 2;
    assert!(blocked(&ribbon, true).reason.unwrap().contains("spawn order"), "a Connect target keeps no spawn order");

    let mut source = base.clone();
    source.sub_emitters.push(SubEmitter::default());
    assert_eq!(blocked(&source, false).active, ActiveBackend::Gpu, "a sub-emitter source runs on the GPU");
    assert_eq!(blocked(&base, true).active, ActiveBackend::Gpu, "a sub-emitter target runs on the GPU");

    let mut brightest = base.clone();
    brightest.light.mode = LightMode::BrightestN(4);
    assert_eq!(blocked(&brightest, false).active, ActiveBackend::Cpu);
    let mut per_emitter = base.clone();
    per_emitter.light.mode = LightMode::PerEmitter;
    assert_eq!(blocked(&per_emitter, false).active, ActiveBackend::Gpu, "a PerEmitter light is reduced on the GPU");

    let mut huge = base.clone();
    huge.capacity = MAX_GPU_CAPACITY + 1;
    assert_eq!(blocked(&huge, false).active, ActiveBackend::Cpu);
    let small_device = GpuCaps { max_capacity: 50_000, ..caps };
    assert_eq!(choose(&base, &c, false, &small_device).active, ActiveBackend::Cpu);

    let no_2d = GpuCaps { renderer_2d: false, ..caps };
    assert_eq!(choose_in(&base, &c, false, true, &no_2d).active, ActiveBackend::Cpu);
    assert_eq!(choose_in(&base, &c, false, false, &no_2d).active, ActiveBackend::Gpu);
}

#[test]
fn the_backend_field_is_auto_by_default_and_not_written_until_set() {
    let plain = simple_effect(&[], vec![], vec![]);
    let text = write_vfx(&plain).unwrap();
    assert!(!text.contains("backend"), "{text}");
    assert!(!text.contains("sort"), "{text}");
    assert_eq!(parse_vfx(&text).unwrap().emitters[0].backend, Backend::Auto);
    let mut gpu = plain.clone();
    gpu.emitters[0].backend = Backend::Gpu;
    gpu.emitters[0].render.sort = true;
    let text = write_vfx(&gpu).unwrap();
    let back = parse_vfx(&text).unwrap();
    assert_eq!(back.emitters[0].backend, Backend::Gpu);
    assert!(back.emitters[0].render.sort);
}

#[test]
fn sorting_applies_only_where_order_matters() {
    let mut r = RenderSettings { sort: true, ..Default::default() };
    assert!(r.sorts(Dimension::ThreeD));
    assert!(!r.sorts(Dimension::TwoD));
    r.blend = BlendMode::Additive;
    assert!(!r.sorts(Dimension::ThreeD), "additive light does not depend on order");
    r.blend = BlendMode::Premultiplied;
    r.facing = Facing::Ribbon;
    assert!(!r.sorts(Dimension::ThreeD));
}

#[test]
fn the_cpu_path_writes_sorted_quads_back_to_front() {
    use crate::render::write_mesh_sorted;
    use bevy::math::Affine3A;
    use bevy::mesh::VertexAttributeValues;
    let mut sim = crate::tests::sim_of(&simple_effect(
        &[("spawn_rate", PinValue::Float(0.0)), ("lifetime", PinValue::Float(10.0))],
        vec![],
        vec![],
    ));
    sim.burst(3, &FrameInput::default());
    let p = &mut sim.emitters[0].particles;
    p.position[0] = Vec3::new(0.0, 0.0, -1.0);
    p.position[1] = Vec3::new(0.0, 0.0, -9.0);
    p.position[2] = Vec3::new(0.0, 0.0, -4.0);
    let mut mesh = bevy::mesh::Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default());
    let render = RenderSettings { sort: true, ..Default::default() };
    write_mesh_sorted(&mut mesh, &sim.emitters[0].particles, &render, Affine3A::IDENTITY, Some(Vec3::ZERO));
    let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute(bevy::mesh::Mesh::ATTRIBUTE_POSITION) else {
        panic!("positions")
    };
    let zs: Vec<f32> = pos.iter().step_by(4).map(|p| p[2]).collect();
    assert_eq!(zs, [-9.0, -4.0, -1.0], "farthest first");
}
