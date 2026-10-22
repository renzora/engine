//! GPU trails and sub-emitters, without a GPU.
//!
//! The same discipline as `tests.rs`: every generated kernel validates, the
//! WGSL's constants are the Rust layout's, and the WGSL that must agree with
//! the CPU (the interval count, the trail ring, the ribbon's point list) is
//! mirrored in Rust here line for line and held to the CPU's own functions.

use bevy::math::Vec3;

use super::codegen::{generate, generate_with, kernels_source, GpuEmitterConfig, GpuRule, LIBRARY};
use super::layout::*;
use super::plan::{gpu_edges, plan_effect, topological_order};
use super::select::*;
use crate::asset::*;
use crate::compile::compile_graph;
use crate::sim::sub::{generation_spans, interval_events, MAX_SUB_DEPTH};
use crate::sim::{EffectSim, FrameInput, ParticleBuffer, SEEK_STEP};
use crate::tests::{graph, out_node, simple_effect};

fn check(source: &str) {
    renzora::wgsl::check(source).unwrap_or_else(|e| panic!("generated WGSL does not validate:\n{e}"));
}

fn rule(trigger: SubEmitterTrigger, inherit_color: bool, target_space: SimulationSpace) -> GpuRule {
    GpuRule {
        trigger,
        inbox: (0, 64),
        count: 8,
        inherit_velocity: 0.5,
        inherit_color,
        target_space,
        key: 3,
    }
}

// ── Codegen ─────────────────────────────────────────────────────────────

#[test]
fn every_trigger_space_and_role_generates_valid_wgsl() {
    let compiled = compile_graph(&graph(vec![out_node()], vec![])).unwrap();
    let triggers = [SubEmitterTrigger::OnBirth, SubEmitterTrigger::OnDeath, SubEmitterTrigger::Interval(0.1)];
    for space in [SimulationSpace::Local, SimulationSpace::World] {
        for target_space in [SimulationSpace::Local, SimulationSpace::World] {
            for trigger in triggers {
                for tint in [false, true] {
                    for target in [false, true] {
                        let config = GpuEmitterConfig {
                            space,
                            inherit_velocity: 0.7,
                            trail: Some((12, 0.05)),
                            rules: vec![rule(trigger, tint, target_space)],
                            target,
                        };
                        let p = generate_with(&compiled, &config).expect("codegen");
                        renzora::wgsl::check(&p.source)
                            .unwrap_or_else(|e| panic!("{trigger:?} {space:?}->{target_space:?} tint {tint} target {target}:\n{e}"));
                        assert_eq!(p.source.contains("fn deliver("), target, "only a target delivers");
                    }
                }
            }
        }
    }
}

#[test]
fn a_collider_kill_and_every_rule_together_validate() {
    let g = graph(vec![out_node(), VfxNode::new(2, "collide/plane").with("kill", PinValue::Bool(true))], vec![]);
    let compiled = compile_graph(&g).unwrap();
    let config = GpuEmitterConfig {
        space: SimulationSpace::World,
        inherit_velocity: 1.0,
        trail: Some((MAX_TRAIL_POINTS, 0.0)),
        rules: vec![
            rule(SubEmitterTrigger::OnBirth, false, SimulationSpace::Local),
            rule(SubEmitterTrigger::OnDeath, true, SimulationSpace::World),
            rule(SubEmitterTrigger::Interval(0.25), false, SimulationSpace::Local),
        ],
        target: true,
    };
    let p = generate_with(&compiled, &config).unwrap();
    check(&p.source);
    let generated = &p.source[LIBRARY.len()..];
    // One birth, one interval, and a death raised twice in the text: by age
    // before the move, by the collider after it.
    assert_eq!(generated.matches("raise_event(").count(), 1 + 1 + 2, "{generated}");
    assert!(generated.contains("interval_range("));
    assert!(generated.contains("trail_record(") && generated.contains("trail_start("));
    // A World source raising into a Local target needs the inverse.
    assert!(generated.contains("point3(step.inverse, q.position)"));
}

#[test]
fn rule_values_are_data_not_text() {
    let compiled = compile_graph(&graph(vec![out_node()], vec![])).unwrap();
    let mut a = GpuEmitterConfig::plain(SimulationSpace::World);
    a.rules = vec![rule(SubEmitterTrigger::OnDeath, false, SimulationSpace::World)];
    a.trail = Some((8, 0.1));
    a.inherit_velocity = 0.5;
    let mut b = a.clone();
    b.rules[0].count = 40;
    b.rules[0].inherit_velocity = 0.9;
    b.rules[0].inbox = (900, 128);
    b.trail = Some((8, 0.3));
    b.inherit_velocity = 0.8;
    let (pa, pb) = (generate_with(&compiled, &a).unwrap(), generate_with(&compiled, &b).unwrap());
    assert_eq!(pa.hash, pb.hash, "editing a count, a share or a spacing must not need a new pipeline");
    assert_ne!(pa.consts, pb.consts);
    // The trail's length is structure: the ring's modulus is in the text.
    b.trail = Some((9, 0.3));
    assert_ne!(pa.hash, generate_with(&compiled, &b).unwrap().hash);
}

#[test]
fn a_plain_emitter_generates_the_plain_kernels() {
    let compiled = compile_graph(&graph(vec![out_node()], vec![])).unwrap();
    let p = generate(&compiled, SimulationSpace::Local).unwrap();
    check(&p.source);
    let generated = &p.source[LIBRARY.len()..];
    assert!(!generated.contains("raise_event("), "no rules, no events");
    assert!(!generated.contains("fn deliver("));
    assert!(!generated.contains("trail_record("));
    // Emission is in every update, a multiplier on the light alone.
    assert!(generated.contains("p.color = vec4<f32>(lit.xyz * san("), "{generated}");
    // Inherit velocity only for World space.
    let mut config = GpuEmitterConfig::plain(SimulationSpace::Local);
    config.inherit_velocity = 1.0;
    assert!(!generate_with(&compiled, &config).unwrap().source.contains("step.emitter_velocity.xyz * data["));
    config.space = SimulationSpace::World;
    assert!(generate_with(&compiled, &config).unwrap().source.contains("step.emitter_velocity.xyz * data["));
}

#[test]
fn the_fixed_kernels_still_validate_with_the_new_sorts_and_the_inbox_clear() {
    let source = kernels_source();
    check(&source);
    assert!(source.contains("fn clear_inbox("));
    for (name, value) in [("SORT_DEPTH", SORT_DEPTH), ("SORT_OLDEST", SORT_OLDEST), ("SORT_YOUNGEST", SORT_YOUNGEST), ("SORT_CONNECT", SORT_CONNECT)] {
        assert!(LIBRARY.contains(&format!("const {name}: u32 = {value}u;")), "{name}");
    }
    assert!(LIBRARY.contains(&format!("const EVENT_WORDS: u32 = {EVENT_WORDS}u;")));
    assert!(LIBRARY.contains(&format!("const EVENT_HEADER_WORDS: u32 = {EVENT_HEADER_WORDS}u;")));
    assert!(LIBRARY.contains(&format!("const NO_INBOX: u32 = {NO_INBOX}u;")));
    assert!(LIBRARY.contains(&format!("const C_GROUPS: u32 = {C_GROUPS}u;")));
    assert!(LIBRARY.contains(&format!("const MAX_INTERVAL_EVENTS: u32 = {}u;", crate::sim::sub::MAX_INTERVAL_EVENTS_PER_STEP)));
}

// ── Layout ──────────────────────────────────────────────────────────────

#[test]
fn the_trail_history_follows_the_sorted_list_and_fits_its_buffer() {
    for capacity in [1u32, 100, 4096, 200_000] {
        for points in [0u32, 1, 16, MAX_TRAIL_POINTS] {
            let words = state_bytes_with_trail(capacity, points) / 4;
            assert_eq!(u64::from(trail_word(capacity)), u64::from(sorted_word(capacity) + sort_size(capacity)));
            assert_eq!(words, u64::from(trail_word(capacity)) + u64::from(capacity) * u64::from(trail_slot_words(points)));
        }
        assert_eq!(state_bytes_with_trail(capacity, 0), state_bytes(capacity), "no trail, no history");
    }
    // The memory a 200k pool's 16-point trail costs: 54.4 MB.
    assert_eq!(trail_words(200_000, 16) * 4, 200_000 * 17 * 16);
}

#[test]
fn inboxes_are_capped_packed_and_never_overlap() {
    let layout = EventLayout::new(&[None, Some(500), Some(1_000_000), None, Some(3)]);
    assert_eq!(layout.inboxes[0], None);
    let (a, ac) = layout.inboxes[1].unwrap();
    let (b, bc) = layout.inboxes[2].unwrap();
    let (c, cc) = layout.inboxes[4].unwrap();
    assert_eq!((ac, bc, cc), (500, MAX_GPU_EVENTS, 3), "an inbox holds what the target could spawn for, capped");
    assert_eq!(a, 0);
    assert_eq!(b, a + EVENT_HEADER_WORDS + ac * EVENT_WORDS);
    assert_eq!(c, b + EVENT_HEADER_WORDS + bc * EVENT_WORDS);
    assert_eq!(layout.words, c + EVENT_HEADER_WORDS + cc * EVENT_WORDS, "the buffer ends at the last entry");
    // Nothing targeted still binds a valid buffer.
    assert!(EventLayout::new(&[None, None]).bytes() > 0);
    // The biggest inbox is 512 KB: an effect's events stay small beside its pools.
    assert_eq!(u64::from(EVENT_HEADER_WORDS + MAX_GPU_EVENTS * EVENT_WORDS) * 4, 16 + 8192 * 64);
}

// ── Interval and trail parity ───────────────────────────────────────────

/// `interval_range` in `library.wgsl`, in Rust.
fn wgsl_interval_range(before: f32, after: f32, every: f32) -> std::ops::Range<u32> {
    if !every.is_finite() || every <= 0.0 || !before.is_finite() || !after.is_finite() || after <= before {
        return 0..0;
    }
    let first = (before / every).floor() + 1.0;
    let mut last = (after / every).floor();
    if last < first || first < 1.0 {
        return 0..0;
    }
    last = last.min(first + crate::sim::sub::MAX_INTERVAL_EVENTS_PER_STEP as f32 - 1.0);
    (first.min(4.0e9) as u32)..((last + 1.0).min(4.0e9) as u32)
}

#[test]
fn the_gpu_interval_count_is_the_cpu_one() {
    let everies = [0.0, -1.0, f32::NAN, 0.001, 0.016, 0.1, 0.25, 1.0 / 3.0, 2.0];
    for every in everies {
        let mut age = 0.0f32;
        for _ in 0..400 {
            for dt in [SEEK_STEP, 0.05, 0.25] {
                let before = age;
                let after = age + dt;
                assert_eq!(wgsl_interval_range(before, after, every), interval_events(before, after, every), "{before}..{after} every {every}");
            }
            age += 0.0137;
        }
    }
    assert!(LIBRARY.contains("let first = floor(before / every) + 1.0;"));
    assert!(LIBRARY.contains("last = min(last, first + f32(MAX_INTERVAL_EVENTS) - 1.0);"));
}

/// `trail_start` and `trail_record` in `library.wgsl`, over one slot's words.
struct WgslTrail {
    words: Vec<u32>,
    points: u32,
}

impl WgslTrail {
    fn start(points: u32, at: Vec3) -> Self {
        let mut t = Self { words: vec![0; trail_slot_words(points) as usize], points };
        t.words[0] = 0;
        t.words[1] = 1;
        t.put(0, at);
        t
    }

    fn put(&mut self, ring: u32, at: Vec3) {
        let w = (4 + ring * 4) as usize;
        self.words[w] = at.x.to_bits();
        self.words[w + 1] = at.y.to_bits();
        self.words[w + 2] = at.z.to_bits();
    }

    fn get(&self, ring: u32) -> Vec3 {
        let w = (4 + ring * 4) as usize;
        Vec3::new(f32::from_bits(self.words[w]), f32::from_bits(self.words[w + 1]), f32::from_bits(self.words[w + 2]))
    }

    fn record(&mut self, at: Vec3, min_distance: f32) {
        let (head, len) = (self.words[0], self.words[1]);
        let d = self.get(head) - at;
        if len > 0 && d.dot(d) < min_distance * min_distance {
            return;
        }
        let next = (head + 1) % self.points;
        self.put(next, at);
        self.words[0] = next;
        self.words[1] = (len + 1).min(self.points);
    }

    /// `ribbon_fetch`'s point list for a particle at `here`: the particle,
    /// then the ring newest first, past every newest point that is `here`.
    fn strip(&self, here: Vec3) -> Vec<Vec3> {
        let points = self.points;
        let head = self.words[0] % points;
        let len = self.words[1].min(points);
        let at = |skip: u32, k: u32| -> Vec3 {
            if k == 0 {
                return here;
            }
            self.get((head + points - ((k - 1 + skip) % points)) % points)
        };
        let mut skip = 0;
        while skip < len && (at(0, skip + 1) - here).dot(at(0, skip + 1) - here) < 1e-12 {
            skip += 1;
        }
        let m = 1 + len - skip;
        (0..m).map(|k| at(skip, k)).collect()
    }
}

#[test]
fn the_gpu_trail_ring_keeps_the_cpu_points_and_draws_the_cpu_strip() {
    for (points, spacing) in [(1u32, 0.0f32), (4, 0.0), (16, 0.05), (64, 0.3)] {
        let mut cpu = ParticleBuffer { trail_stride: points as usize, ..Default::default() };
        let start = Vec3::new(0.5, -1.0, 2.0);
        cpu.push(crate::sim::Birth {
            id: 0,
            lifetime: 10.0,
            position: start,
            velocity: Vec3::ZERO,
            size: bevy::math::Vec2::ONE,
            rotation: 0.0,
            angular_velocity: 0.0,
            color: bevy::math::Vec4::ONE,
            generation: 0,
        });
        let mut gpu = WgslTrail::start(points, start);
        let mut here = start;
        for step in 0..200 {
            // Uneven motion, with pauses that the spacing must swallow.
            let speed = if step % 17 < 3 { 0.0 } else { 0.01 + (step % 7) as f32 * 0.02 };
            here += Vec3::new(speed, (step as f32 * 0.3).sin() * speed, 0.0);
            cpu.record_trail(0, here, spacing);
            gpu.record(here, spacing);
            let mut cpu_strip = vec![here];
            for p in cpu.trail(0) {
                if cpu_strip.len() == 1 && p.distance_squared(here) < 1e-12 {
                    continue;
                }
                cpu_strip.push(p);
            }
            assert_eq!(gpu.strip(here), cpu_strip, "{points} points, spacing {spacing}, step {step}");
        }
    }
}

// ── Plans, links and lockstep ───────────────────────────────────────────

/// Sparks that burst into embers, both GPU-sized, the embers World-space.
fn sparks_and_embers(trigger: SubEmitterTrigger) -> VfxAsset {
    let mut asset = simple_effect(&[("spawn_rate", PinValue::Float(200.0)), ("lifetime", PinValue::Float(0.5))], vec![], vec![]);
    asset.emitters[0].name = "sparks".into();
    asset.emitters[0].backend = Backend::Gpu;
    asset.emitters[0].sub_emitters.push(SubEmitter { trigger, target: 1, count: 6, inherit_velocity: 0.3, inherit_color: true });
    let mut embers = asset.emitters[0].clone();
    embers.name = "embers".into();
    embers.sub_emitters.clear();
    embers.space = SimulationSpace::World;
    asset.emitters.push(embers);
    asset
}

#[test]
fn a_gpu_parent_and_child_run_on_the_gpu_together_and_plan_their_events() {
    let asset = sparks_and_embers(SubEmitterTrigger::OnDeath);
    let (sim, _) = EffectSim::with_backends(&asset, &GpuCaps::all());
    assert!(sim.emitters.iter().all(|e| e.is_gpu()), "{:?}", sim.emitters.iter().map(|e| &e.backend).collect::<Vec<_>>());
    let plan = plan_effect(&sim, &asset);
    assert_eq!(gpu_edges(&sim), vec![(0, 1)]);
    assert_eq!(plan.linked, vec![true, true]);
    assert!(plan.order[0] < plan.order[1], "the parent steps first");
    assert!(plan.events.inboxes[0].is_none() && plan.events.inboxes[1].is_some());
    let parent = plan.configs[0].as_ref().unwrap();
    let child = plan.configs[1].as_ref().unwrap();
    assert!(!parent.target && child.target);
    assert_eq!(parent.rules.len(), 1);
    let r = parent.rules[0];
    assert_eq!((r.count, r.inherit_velocity, r.inherit_color), (6, 0.3, true));
    assert_eq!(r.inbox, plan.events.inboxes[1].unwrap());
    assert_eq!(r.target_space, SimulationSpace::World);
    for config in [parent, child] {
        check(&generate_with(&sim.emitters[0].compiled, config).unwrap().source);
    }
}

#[test]
fn a_cpu_partner_takes_its_gpu_partner_down_with_a_reason() {
    let mut asset = sparks_and_embers(SubEmitterTrigger::OnBirth);
    asset.emitters[1].backend = Backend::Cpu;
    let (sim, _) = EffectSim::with_backends(&asset, &GpuCaps::all());
    assert!(!sim.emitters[0].is_gpu() && !sim.emitters[1].is_gpu());
    let why = sim.emitters[0].backend.reason.clone().unwrap();
    assert!(why.contains("sub-emitter events") && why.contains("`embers`"), "{why}");
    assert!(sim.emitters[0].backend.fell_back, "the parent asked for the GPU, so it is a fallback worth a warning");
    assert_eq!(sim.emitters[1].backend.reason, None, "the child asked for the CPU and got it");
    // The CPU pair plays as it always has.
    let mut sim = sim;
    sim.step(SEEK_STEP, &FrameInput::default());
    assert!(!sim.emitters[1].particles.is_empty(), "births spawned embers on the CPU");
}

#[test]
fn a_small_auto_child_of_a_gpu_parent_moves_up_with_it() {
    let mut asset = sparks_and_embers(SubEmitterTrigger::OnDeath);
    asset.emitters[1].backend = Backend::Auto;
    asset.emitters[1].capacity = 300;
    let (sim, _) = EffectSim::with_backends(&asset, &GpuCaps::all());
    assert!(sim.emitters[0].is_gpu() && sim.emitters[1].is_gpu(), "{:?}", sim.emitters[1].backend);
    // Without a device both stay on the CPU, and the child is not blamed on
    // its parent: neither could run there.
    let (cpu, _) = EffectSim::with_backends(&asset, &GpuCaps::none());
    assert!(cpu.emitters.iter().all(|e| !e.is_gpu()));
}

#[test]
fn a_chain_that_straddles_the_generation_cap_stays_on_the_cpu() {
    // A -> B -> C -> D -> E, and A -> E: E's particles are generation 1 and 4.
    let spans = generation_spans(5, &[(0, 1), (1, 2), (2, 3), (3, 4), (0, 4)]);
    assert_eq!(spans[0], (0, 0));
    assert_eq!(spans[1], (1, 1));
    assert_eq!(spans[4], (1, 4));
    assert!(generation_blocker(spans[4], true).is_some());
    assert!(generation_blocker(spans[4], false).is_none(), "no rules, nothing to raise");
    assert!(generation_blocker(spans[3], true).is_none(), "all under the cap");
    assert!(generation_blocker((MAX_SUB_DEPTH, MAX_SUB_DEPTH + 1), true).is_none(), "all past the cap raise nothing, exactly");
    assert_eq!(topological_order(5, &[(0, 1), (1, 2), (2, 3), (3, 4), (0, 4)]), vec![0, 1, 2, 3, 4]);
    assert_eq!(topological_order(3, &[(2, 0), (1, 0)]), vec![2, 0, 1]);
}

#[test]
fn a_script_burst_keeps_a_gpu_target_in_step_with_its_parent() {
    let asset = sparks_and_embers(SubEmitterTrigger::OnBirth);
    let (mut sim, _) = EffectSim::with_backends(&asset, &GpuCaps::all());
    let frame = FrameInput::default();
    sim.step(SEEK_STEP, &frame);
    sim.burst(10, &frame);
    sim.burst(0, &frame);
    sim.step(SEEK_STEP, &frame);
    let plans = |i: usize| sim.emitters[i].gpu.as_ref().unwrap().plans.clone();
    let (parent, child) = (plans(0), plans(1));
    assert_eq!(parent.len(), child.len(), "the render world steps the two together");
    assert_eq!(parent.iter().map(|p| p.spawn_only).collect::<Vec<_>>(), child.iter().map(|p| p.spawn_only).collect::<Vec<_>>());
    assert!(child.iter().all(|p| p.spawn_count == 0), "a target spawns only from its inbox");
}

#[test]
fn trails_and_tubes_plan_their_history_and_meshes() {
    let mut asset = simple_effect(&[("spawn_rate", PinValue::Float(100.0))], vec![], vec![]);
    asset.emitters[0].backend = Backend::Gpu;
    asset.emitters[0].render.facing = Facing::Ribbon;
    asset.emitters[0].render.ribbon.points = 24;
    asset.emitters[0].render.ribbon.min_distance = 0.2;
    let (sim, _) = EffectSim::with_backends(&asset, &GpuCaps::all());
    assert!(sim.emitters[0].is_gpu());
    let plan = plan_effect(&sim, &asset);
    let config = plan.configs[0].as_ref().unwrap();
    assert_eq!(config.trail, Some((24, 0.2)));
    assert!(!plan.linked[0] && !plan.has_events());
    // A trail past what the device binds goes to the CPU and says how big.
    let small = GpuCaps { max_binding: 1 << 20, ..GpuCaps::all() };
    let compiled = compile_graph(&asset.emitters[0].graph).unwrap();
    let why = choose(&asset.emitters[0], &compiled, false, &small).reason.unwrap();
    assert!(why.contains("trail history"), "{why}");
}

#[cfg(all(not(target_arch = "wasm32"), any(feature = "render_3d", feature = "render_2d")))]
#[test]
fn ribbon_pool_meshes_have_a_strip_per_slot_and_a_segment_per_pair() {
    use super::runtime::{RibbonMesh, MAX_GPU_RIBBON_VERTICES};
    let mut render = RenderSettings { facing: Facing::Ribbon, ..Default::default() };
    render.ribbon.points = 16;
    let trail = RibbonMesh::for_render(&render, Dimension::ThreeD, 1000);
    assert_eq!((trail.connect, trail.rows, trail.cols, trail.strips), (false, 17, 2, 1000));
    render.ribbon.shape = RibbonShape::Tube;
    render.ribbon.radial_segments = 6;
    let tube = RibbonMesh::for_render(&render, Dimension::ThreeD, 1000);
    assert_eq!(tube.cols, 7, "a ring repeats its first vertex to close the UVs");
    assert_eq!(RibbonMesh::for_render(&render, Dimension::TwoD, 1000).cols, 2, "2D draws a tube flat");
    render.ribbon.kind = RibbonKind::Connect;
    let connect = RibbonMesh::for_render(&render, Dimension::ThreeD, 1000);
    assert_eq!((connect.connect, connect.rows, connect.strips), (true, 2, 999));
    let mesh = trail.build();
    assert_eq!(mesh.count_vertices(), 17 * 2 * 1000);
    assert_eq!(mesh.indices().unwrap().len(), 16 * 6 * 1000);
    let huge = RibbonMesh { strips: 1_000_000, ..trail }.capped();
    assert!(huge.vertices_per_strip() * huge.strips as usize <= MAX_GPU_RIBBON_VERTICES);
}
