//! The converter against every shipped `.particle`, plus one test per mapping
//! that matters for how an effect looks. The tests of writing a conversion to
//! disk are with the editor, which is the only thing that does.

use std::path::{Path, PathBuf};

use super::format::*;
// The new system's `BlendMode` and `SimulationSpace` are written out in full
// below: the glob above brings in the Hanabi ones of the same names.
use crate::asset::{parse_vfx, Dimension, Facing, PinValue, VfxAsset, VfxGraph, VfxNode};
use crate::compile::compile_graph;
use crate::nodes::OUTPUT_NODE;
use crate::sim::{EffectSim, FrameInput};

use super::*;

/// Read and convert one file, the way the `.particle` loader does.
fn convert_file(path: &Path) -> Result<Conversion, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("could not read {}: {e}", path.display()))?;
    convert_text(&text, &path.file_stem().unwrap().to_string_lossy())
}

fn shipped_particles() -> Vec<PathBuf> {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/particles");
    let mut pending = vec![PathBuf::from(root)];
    let mut files = Vec::new();
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display())).flatten() {
            let p = entry.path();
            if p.is_dir() {
                pending.push(p);
            } else if p.extension().and_then(|e| e.to_str()) == Some("particle") {
                files.push(p);
            }
        }
    }
    files.sort();
    files
}

fn output(graph: &VfxGraph) -> &VfxNode {
    graph.nodes.iter().find(|n| n.node_type == OUTPUT_NODE).expect("an output node")
}

fn inline<'a>(graph: &'a VfxGraph, pin: &str) -> Option<&'a PinValue> {
    output(graph).input_values.get(pin)
}

/// The node wired into the output's `pin`.
fn source<'a>(graph: &'a VfxGraph, pin: &str) -> Option<&'a VfxNode> {
    let out = output(graph).id;
    let c = graph.connections.iter().find(|c| c.to_node == out && c.to_pin == pin)?;
    graph.nodes.iter().find(|n| n.id == c.from_node)
}

fn convert(def: &HanabiEffectDefinition) -> (VfxAsset, Vec<String>) {
    let c = convert_definition(def);
    for e in &c.asset.emitters {
        if let Err(errors) = compile_graph(&e.graph) {
            panic!("converted graph does not compile: {errors:?}");
        }
    }
    (c.asset, c.warnings.iter().map(|w| w.feature.clone()).collect())
}

/// Simulate and return every live particle's position and velocity.
fn simulate(asset: &VfxAsset, seconds: f32) -> Vec<(bevy::math::Vec3, bevy::math::Vec3)> {
    let (mut sim, failures) = EffectSim::new(asset);
    assert!(failures.is_empty());
    sim.simulate_to(seconds, &FrameInput::default());
    sim.emitters
        .iter()
        .flat_map(|e| e.particles.position.iter().copied().zip(e.particles.velocity.iter().copied()))
        .collect()
}

/// The per-emitter particle cap for the whole-library simulation below.
const SIM_CAPACITY: u32 = 2000;

#[test]
fn every_shipped_effect_converts_compiles_and_simulates() {
    let files = shipped_particles();
    assert!(files.len() > 50, "expected the shipped effects, found {}", files.len());
    let mut failures = Vec::new();
    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let conversion = match convert_file(path) {
            Ok(c) => c,
            Err(e) => {
                failures.push(format!("{name}: {e}"));
                continue;
            }
        };
        let text = vfx_text(&conversion, &name).unwrap();
        let reparsed = match parse_vfx(&text) {
            Ok(a) => a,
            Err(e) => {
                failures.push(format!("{name}: the written .vfx does not parse: {e}"));
                continue;
            }
        };
        assert_eq!(reparsed, conversion.asset, "{name}: the .vfx text does not round-trip");
        // Simulate a capped copy for one second. The library has 238 effects,
        // some at 100000 particles, and three seconds of each at full size
        // took minutes optimized and stalled CI's unoptimized test build for
        // over half an hour. A blow-up (NaN, runaway force, bad light) shows
        // in a particle's first second, and a cap of 2000 still runs every
        // node of every graph on thousands of particles.
        let mut capped = reparsed.clone();
        for e in &mut capped.emitters {
            e.capacity = e.capacity.min(SIM_CAPACITY);
        }
        let (mut sim, errors) = EffectSim::new(&capped);
        if let Some(e) = errors.first() {
            failures.push(format!("{name}: {:?}", e.errors));
            continue;
        }
        sim.simulate_to(1.0, &FrameInput::default());
        for e in &sim.emitters {
            let p = &e.particles;
            let finite = p.position.iter().all(|v| v.is_finite())
                && p.velocity.iter().all(|v| v.is_finite())
                && p.size.iter().all(|v| v.is_finite())
                && p.color.iter().all(|v| v.is_finite())
                && p.lifetime.iter().all(|v| v.is_finite());
            if !finite {
                failures.push(format!("{name}: a non-finite particle after 3 s"));
            }
            // Its light and its mesh (quads or a ribbon) come out finite too.
            let emitter = &reparsed.emitters[e.emitter_index];
            let mut lights = Vec::new();
            crate::light::aggregate(p, &emitter.light, bevy::math::Affine3A::IDENTITY, &mut lights);
            if lights.iter().any(|l| !l.intensity.is_finite() || !l.color.is_finite()) {
                failures.push(format!("{name}: a non-finite light"));
            }
            let mut mesh = bevy::mesh::Mesh::new(
                bevy::mesh::PrimitiveTopology::TriangleList,
                bevy::asset::RenderAssetUsages::default(),
            );
            let aabb = crate::render::write_mesh(&mut mesh, p, &emitter.render, bevy::math::Affine3A::IDENTITY);
            if aabb.is_some_and(|b| !bevy::math::Vec3::from(b.half_extents).is_finite()) {
                failures.push(format!("{name}: non-finite mesh bounds"));
            }
        }
    }
    assert!(failures.is_empty(), "{} of {} failed:\n{}", failures.len(), files.len(), failures.join("\n"));
}

#[test]
fn conversion_is_deterministic() {
    for path in shipped_particles().iter().take(20) {
        let a = vfx_text(&convert_file(path).unwrap(), "x").unwrap();
        let b = vfx_text(&convert_file(path).unwrap(), "x").unwrap();
        assert_eq!(a, b, "{}", path.display());
    }
}

#[test]
fn the_layout_flows_into_the_output_without_overlaps() {
    for path in shipped_particles().iter().take(40) {
        let asset = convert_file(path).unwrap().asset;
        let g = &asset.emitters[0].graph;
        let out = output(g);
        for n in &g.nodes {
            if n.id != out.id {
                assert!(n.position[0] < out.position[0], "{}: node {} is not left of the output", path.display(), n.id);
            }
        }
        // Every wire runs left to right.
        for c in &g.connections {
            let from = g.nodes.iter().find(|n| n.id == c.from_node).unwrap();
            let to = g.nodes.iter().find(|n| n.id == c.to_node).unwrap();
            assert!(from.position[0] < to.position[0], "{}: wire {c:?} runs backwards", path.display());
        }
        // No two nodes share a spot in a column.
        for a in &g.nodes {
            for b in &g.nodes {
                if a.id < b.id && a.position[0] == b.position[0] {
                    assert!((a.position[1] - b.position[1]).abs() >= 40.0, "{}: nodes {} and {} overlap", path.display(), a.id, b.id);
                }
            }
        }
    }
}

#[test]
fn continuous_rate_loops_and_emits_at_the_rate() {
    let def = HanabiEffectDefinition { spawn_mode: SpawnMode::Rate, spawn_rate: 37.0, ..Default::default() };
    let (asset, _) = convert(&def);
    assert!(asset.looping);
    let g = &asset.emitters[0].graph;
    assert_eq!(inline(g, "spawn_rate"), Some(&PinValue::Float(37.0)));
}

#[test]
fn a_timed_rate_with_cycles_is_one_shot_for_their_total() {
    let def = HanabiEffectDefinition { spawn_duration: 1.5, spawn_cycle_count: 2, ..Default::default() };
    let (asset, _) = convert(&def);
    assert!(!asset.looping);
    assert!((asset.duration - 3.0).abs() < 1e-6);
}

#[test]
fn a_single_burst_spawns_once_and_does_not_loop() {
    let def = HanabiEffectDefinition { spawn_mode: SpawnMode::Burst, spawn_count: 40, capacity: 100, ..Default::default() };
    let (asset, _) = convert(&def);
    assert!(!asset.looping);
    let g = &asset.emitters[0].graph;
    assert_eq!(inline(g, "spawn_rate"), Some(&PinValue::Float(0.0)));
    assert_eq!(inline(g, "burst_count"), Some(&PinValue::Float(40.0)));
    let (mut sim, _) = EffectSim::new(&asset);
    sim.simulate_to(0.5, &FrameInput::default());
    assert_eq!(sim.alive(), 40);
}

#[test]
fn repeated_bursts_keep_their_period_and_cycles() {
    let def = HanabiEffectDefinition {
        spawn_mode: SpawnMode::BurstRate,
        spawn_rate: 4.0,
        spawn_count: 5,
        spawn_cycle_count: 3,
        lifetime_min: 5.0,
        lifetime_max: 5.0,
        ..Default::default()
    };
    let (asset, _) = convert(&def);
    let g = &asset.emitters[0].graph;
    assert_eq!(inline(g, "burst_interval"), Some(&PinValue::Float(0.25)));
    assert_eq!(inline(g, "burst_cycles"), Some(&PinValue::Float(3.0)));
    let (mut sim, _) = EffectSim::new(&asset);
    sim.simulate_to(2.0, &FrameInput::default());
    assert_eq!(sim.alive(), 15, "three bursts of five");
}

#[test]
fn a_lifetime_range_is_a_random_range() {
    let def = HanabiEffectDefinition { lifetime_min: 0.5, lifetime_max: 1.25, ..Default::default() };
    let (asset, _) = convert(&def);
    let n = source(&asset.emitters[0].graph, "lifetime").expect("a lifetime node");
    assert_eq!(n.node_type, "random/range");
    assert_eq!(n.input_values.get("min"), Some(&PinValue::Float(0.5)));
    assert_eq!(n.input_values.get("max"), Some(&PinValue::Float(1.25)));
}

#[test]
fn a_surface_sphere_keeps_its_radius_and_surface() {
    let def = HanabiEffectDefinition {
        emit_shape: HanabiEmitShape::Sphere { radius: 2.0, dimension: ShapeDimension::Surface },
        velocity_magnitude: 0.0,
        acceleration: [0.0; 3],
        ..Default::default()
    };
    let (asset, _) = convert(&def);
    let n = source(&asset.emitters[0].graph, "position").unwrap();
    assert_eq!(n.node_type, "shape/sphere");
    assert_eq!(n.input_values.get("surface"), Some(&PinValue::Bool(true)));
    for (p, _) in simulate(&asset, 0.1) {
        assert!((p.length() - 2.0).abs() < 1e-3, "{p:?} is not on the sphere");
    }
}

#[test]
fn a_3d_circle_lies_flat_on_the_ground() {
    let def = HanabiEffectDefinition {
        emit_shape: HanabiEmitShape::Circle { radius: 1.0, dimension: ShapeDimension::Surface },
        velocity_magnitude: 0.0,
        acceleration: [0.0; 3],
        ..Default::default()
    };
    let (asset, _) = convert(&def);
    let ps = simulate(&asset, 0.2);
    assert!(!ps.is_empty());
    for (p, _) in ps {
        assert!(p.y.abs() < 1e-4, "{p:?} is off the XZ plane");
        assert!(((p.x * p.x + p.z * p.z).sqrt() - 1.0).abs() < 1e-3, "{p:?} is not on the edge");
    }
}

#[test]
fn a_2d_effect_stays_in_the_xy_plane() {
    let def = HanabiEffectDefinition {
        plane_2d: true,
        emit_shape: HanabiEmitShape::Sphere { radius: 10.0, dimension: ShapeDimension::Volume },
        velocity_mode: VelocityMode::Radial,
        velocity_magnitude: 30.0,
        noise_amplitude: 5.0,
        noise_frequency: 0.1,
        acceleration: [0.0; 3],
        ..Default::default()
    };
    let (asset, _) = convert(&def);
    assert_eq!(asset.dimension, Dimension::TwoD);
    for (p, v) in simulate(&asset, 0.5) {
        assert!(p.z.abs() < 1e-3 && v.z.abs() < 1e-3, "{p:?} / {v:?} left the plane");
    }
}

#[test]
fn directional_spread_is_the_old_box_exactly() {
    let def = HanabiEffectDefinition {
        velocity_mode: VelocityMode::Directional,
        velocity_direction: [0.0, 2.0, 0.0],
        velocity_magnitude: 4.0,
        velocity_spread: 0.5,
        acceleration: [0.0; 3],
        ..Default::default()
    };
    let (asset, _) = convert(&def);
    let n = source(&asset.emitters[0].graph, "velocity").unwrap();
    assert_eq!(n.node_type, "random/vec3_range");
    let s = 4.0 * 0.5f32.sin();
    let Some(PinValue::Vec3(min)) = n.input_values.get("min") else { panic!() };
    let Some(PinValue::Vec3(max)) = n.input_values.get("max") else { panic!() };
    for (got, want) in min.iter().zip([-s, 4.0 - s, -s]).chain(max.iter().zip([s, 4.0 + s, s])) {
        assert!((got - want).abs() < 1e-5);
    }
}

#[test]
fn radial_velocity_points_away_from_the_centre_at_the_speed() {
    let def = HanabiEffectDefinition {
        emit_shape: HanabiEmitShape::Sphere { radius: 1.0, dimension: ShapeDimension::Surface },
        velocity_mode: VelocityMode::Radial,
        velocity_speed_min: 2.0,
        velocity_speed_max: 3.0,
        acceleration: [0.0; 3],
        // A burst, so the first step already has particles to look at.
        spawn_mode: SpawnMode::Burst,
        spawn_count: 32,
        ..Default::default()
    };
    let (asset, _) = convert(&def);
    let ps = simulate(&asset, 1.0 / 60.0);
    assert!(!ps.is_empty());
    for (p, v) in ps {
        let speed = v.length();
        assert!((2.0 - 1e-3..=3.0 + 1e-3).contains(&speed), "speed {speed}");
        assert!(p.normalize().dot(v.normalize()) > 0.999, "{v:?} is not outward at {p:?}");
    }
}

#[test]
fn tangent_velocity_circles_the_axis() {
    let def = HanabiEffectDefinition {
        emit_shape: HanabiEmitShape::Circle { radius: 1.0, dimension: ShapeDimension::Surface },
        velocity_mode: VelocityMode::Tangent,
        velocity_magnitude: 2.0,
        velocity_axis: [0.0, 1.0, 0.0],
        acceleration: [0.0; 3],
        spawn_mode: SpawnMode::Burst,
        spawn_count: 32,
        ..Default::default()
    };
    let (asset, _) = convert(&def);
    let ps = simulate(&asset, 1.0 / 60.0);
    assert!(!ps.is_empty());
    for (p, v) in ps {
        assert!((v.length() - 2.0).abs() < 1e-3);
        assert!(p.normalize().dot(v.normalize()).abs() < 0.05, "{v:?} is not tangent at {p:?}");
    }
}

#[test]
fn gravity_drag_and_forces_reach_the_update_pins() {
    let def = HanabiEffectDefinition {
        acceleration: [0.0, -9.8, 0.0],
        linear_drag: 0.7,
        radial_acceleration: 2.0,
        tangent_acceleration: 1.0,
        noise_amplitude: 1.0,
        noise_frequency: 2.0,
        noise_octaves: 1,
        ..Default::default()
    };
    let (asset, warnings) = convert(&def);
    let g = &asset.emitters[0].graph;
    assert_eq!(inline(g, "gravity"), Some(&PinValue::Vec3([0.0, -9.8, 0.0])));
    assert_eq!(inline(g, "drag"), Some(&PinValue::Float(0.7)));
    assert!(source(g, "acceleration").is_some());
    for t in ["force/attractor", "force/vortex", "force/curl_noise"] {
        assert!(g.nodes.iter().any(|n| n.node_type == t), "{t} missing");
    }
    assert!(warnings.is_empty(), "{warnings:?}");
}

fn node_of<'a>(graph: &'a VfxGraph, node_type: &str) -> Vec<&'a VfxNode> {
    graph.nodes.iter().filter(|n| n.node_type == node_type).collect()
}

fn float_of(node: &VfxNode, pin: &str) -> Option<f32> {
    match node.input_values.get(pin) {
        Some(PinValue::Float(v)) => Some(*v),
        _ => None,
    }
}

#[test]
fn conform_to_sphere_maps_field_for_field_without_a_warning() {
    let def = HanabiEffectDefinition {
        conform_to_sphere: Some(ConformToSphere {
            origin: [1.0, 2.0, 3.0],
            radius: 1.5,
            influence_dist: 4.0,
            attraction_accel: 6.0,
            max_attraction_speed: 2.5,
            shell_half_thickness: 0.2,
            sticky_factor: 0.5,
        }),
        ..Default::default()
    };
    let (asset, warnings) = convert(&def);
    assert!(warnings.is_empty(), "{warnings:?}");
    let g = &asset.emitters[0].graph;
    let nodes = node_of(g, "force/conform_sphere");
    assert_eq!(nodes.len(), 1);
    let n = nodes[0];
    assert_eq!(n.input_values.get("center"), Some(&PinValue::Vec3([1.0, 2.0, 3.0])));
    for (pin, want) in [("radius", 1.5), ("influence", 4.0), ("attraction", 6.0), ("max_speed", 2.5), ("shell", 0.2), ("sticky", 0.5)] {
        assert_eq!(float_of(n, pin), Some(want), "{pin}");
    }
    assert!(source(g, "acceleration").is_some(), "wired into acceleration");
}

#[test]
fn attractors_become_the_conform_to_sphere_they_always_were() {
    // The old builder made each attractor a ConformToSphereModifier with
    // Hanabi's default shell and sticky factor.
    let def = HanabiEffectDefinition {
        attractors: vec![
            Attractor { position: [0.0, 1.0, 0.0], radius: 0.3, influence_dist: 2.0, strength: 5.0, max_speed: 7.0 },
            Attractor { position: [3.0, 0.0, 0.0], radius: 0.0, influence_dist: 1.0, strength: -2.0, max_speed: 1.0 },
        ],
        ..Default::default()
    };
    let (asset, warnings) = convert(&def);
    let g = &asset.emitters[0].graph;
    let conform = node_of(g, "force/conform_sphere");
    assert_eq!(conform.len(), 1);
    let n = conform[0];
    assert_eq!(float_of(n, "radius"), Some(0.3));
    assert_eq!(float_of(n, "attraction"), Some(5.0));
    assert_eq!(float_of(n, "max_speed"), Some(7.0));
    assert_eq!(float_of(n, "sticky"), Some(crate::nodes::CONFORM_STICKY));
    assert_eq!(float_of(n, "shell"), Some(crate::nodes::CONFORM_SHELL));
    // A repelling one is a push, and says it is approximated.
    assert_eq!(node_of(g, "force/attractor").len(), 1);
    assert_eq!(warnings, ["repelling attractors"], "{warnings:?}");
}

#[test]
fn an_orbit_maps_to_the_orbit_node_at_its_speed_and_ring() {
    let def = HanabiEffectDefinition {
        orbit: Some(OrbitSettings { center: [0.0, 1.0, 0.0], axis: [0.0, 2.0, 0.0], speed: 3.0, radial_pull: 5.0, orbit_radius: 1.25 }),
        ..Default::default()
    };
    let (asset, warnings) = convert(&def);
    assert!(warnings.is_empty(), "{warnings:?}");
    let g = &asset.emitters[0].graph;
    let orbit = node_of(g, "force/orbit");
    assert_eq!(orbit.len(), 1);
    let n = orbit[0];
    assert_eq!(n.input_values.get("axis"), Some(&PinValue::Vec3([0.0, 1.0, 0.0])), "normalised");
    assert_eq!(float_of(n, "speed"), Some(3.0));
    assert_eq!(float_of(n, "radius"), Some(1.25));
    assert_eq!(float_of(n, "radial_pull"), Some(5.0));
    // With no pull the old orbit had no ring: neither does the new one.
    let def = HanabiEffectDefinition { orbit: Some(OrbitSettings { radial_pull: 0.0, ..Default::default() }), ..Default::default() };
    let (asset, _) = convert(&def);
    let n = node_of(&asset.emitters[0].graph, "force/orbit")[0];
    assert_eq!(float_of(n, "radius"), None);
    // And the converted orbit circles at its speed rather than winding up.
    let def = HanabiEffectDefinition {
        orbit: Some(OrbitSettings { speed: 2.0, ..Default::default() }),
        emit_shape: HanabiEmitShape::Circle { radius: 1.0, dimension: ShapeDimension::Surface },
        lifetime_min: 10.0,
        lifetime_max: 10.0,
        ..Default::default()
    };
    let (asset, _) = convert(&def);
    for (p, v) in simulate(&asset, 3.0) {
        let r = bevy::math::Vec3::new(p.x, 0.0, p.z).length();
        if r > 0.5 {
            let around = bevy::math::Vec3::new(v.x, 0.0, v.z).length();
            assert!(around < 2.0 * r * 1.2, "{around} m/s at {r} m: wound past its speed");
        }
    }
}

#[test]
fn a_size_curve_becomes_the_size_over_life_curve() {
    let def = HanabiEffectDefinition {
        size_curve: vec![
            CurvePoint { time: 1.0, value: 0.0 },
            CurvePoint { time: 0.0, value: 0.2 },
            CurvePoint { time: 0.4, value: 0.5 },
        ],
        ..Default::default()
    };
    let (asset, _) = convert(&def);
    let g = &asset.emitters[0].graph;
    let n = source(g, "size_over_life").unwrap();
    let Some(PinValue::Curve(points)) = n.input_values.get("points") else { panic!() };
    let ts: Vec<f32> = points.iter().map(|p| p.t).collect();
    assert_eq!(ts, [0.0, 0.4, 1.0], "sorted");
    assert_eq!(inline(g, "size"), Some(&PinValue::Float(1.0)), "the curve holds the absolute size");
}

#[test]
fn a_random_start_size_scales_the_curve() {
    let def = HanabiEffectDefinition { size_start: 0.2, size_end: 0.0, size_start_min: 0.1, size_start_max: 0.3, ..Default::default() };
    let (asset, _) = convert(&def);
    let n = source(&asset.emitters[0].graph, "size").unwrap();
    assert_eq!(n.node_type, "random/range");
    assert_eq!(n.input_values.get("min"), Some(&PinValue::Float(0.5)));
    assert_eq!(n.input_values.get("max"), Some(&PinValue::Float(1.5)));
}

#[test]
fn non_uniform_size_lerps_both_axes() {
    let def = HanabiEffectDefinition {
        size_non_uniform: true,
        size_start_x: 0.1,
        size_start_y: 0.4,
        size_end_x: 0.0,
        size_end_y: 0.2,
        lifetime_min: 1.0,
        lifetime_max: 1.0,
        ..Default::default()
    };
    let (asset, _) = convert(&def);
    assert_eq!(source(&asset.emitters[0].graph, "size_over_life").unwrap().node_type, "math/lerp");
    let (mut sim, _) = EffectSim::new(&asset);
    sim.simulate_to(0.5, &FrameInput::default());
    let oldest = &sim.emitters[0].particles;
    let i = (0..oldest.len()).max_by(|a, b| oldest.age[*a].total_cmp(&oldest.age[*b])).unwrap();
    let life = oldest.age[i] / oldest.lifetime[i];
    let s = oldest.size[i];
    assert!((s.x - 0.1 * (1.0 - life)).abs() < 0.01 && (s.y - (0.4 - 0.2 * life)).abs() < 0.01, "{s:?} at {life}");
}

#[test]
fn hdr_intensity_becomes_the_emission_pin() {
    let def = HanabiEffectDefinition {
        use_hdr_color: true,
        hdr_intensity: 3.0,
        color_gradient: vec![
            GradientStop { position: 0.0, color: [1.0, 0.5, 0.0, 1.0] },
            GradientStop { position: 1.0, color: [0.2, 0.0, 0.0, 0.0] },
        ],
        ..Default::default()
    };
    let (asset, _) = convert(&def);
    let g = &asset.emitters[0].graph;
    let n = source(g, "color_over_life").unwrap();
    let Some(PinValue::Gradient(stops)) = n.input_values.get("stops") else { panic!() };
    assert_eq!(stops[0].color, [1.0, 0.5, 0.0, 1.0], "the stops keep the colours the old editor showed");
    assert_eq!(stops[1].t, 1.0);
    assert_eq!(inline(g, "emission"), Some(&PinValue::Float(3.0)), "the intensity is the emission");
    // And it still glows as bright: the young particle's colour is 3x.
    let (mut sim, _) = EffectSim::new(&asset);
    sim.simulate_to(0.05, &FrameInput::default());
    let p = &sim.emitters[0].particles;
    assert!(!p.is_empty());
    let young = (0..p.len()).min_by(|a, b| p.age[*a].total_cmp(&p.age[*b])).unwrap();
    assert!(p.color[young].x > 2.5, "{:?}", p.color[young]);
    // No HDR, no pin.
    let (plain, _) = convert(&HanabiEffectDefinition { color_gradient: def.color_gradient.clone(), ..Default::default() });
    assert_eq!(inline(&plain.emitters[0].graph, "emission"), None);
}

#[test]
fn a_flat_colour_is_the_spawn_colour() {
    let def = HanabiEffectDefinition { use_flat_color: true, flat_color: [0.1, 0.2, 0.3, 0.4], ..Default::default() };
    let (asset, _) = convert(&def);
    let g = &asset.emitters[0].graph;
    assert_eq!(inline(g, "color"), Some(&PinValue::Color([0.1, 0.2, 0.3, 0.4])));
    assert!(source(g, "color_over_life").is_none());
}

#[test]
fn render_settings_carry_over() {
    let def = HanabiEffectDefinition {
        blend_mode: BlendMode::Additive,
        texture_path: Some("textures/spark.png".into()),
        orient_mode: ParticleOrientMode::AlongVelocity,
        simulation_space: SimulationSpace::World,
        flipbook: Some(FlipbookSettings { grid_columns: 4, grid_rows: 2 }),
        rotation_speed: 1.5,
        ..Default::default()
    };
    let (asset, _) = convert(&def);
    let e = &asset.emitters[0];
    assert_eq!(e.render.blend, crate::asset::BlendMode::Additive);
    assert_eq!(e.render.texture.as_deref(), Some("textures/spark.png"));
    assert_eq!(e.render.facing, Facing::VelocityAligned);
    assert_eq!(e.render.sprite_sheet.columns, 4);
    assert_eq!(e.space, crate::asset::SimulationSpace::World);
    assert_eq!(inline(&e.graph, "angular_velocity"), Some(&PinValue::Float(1.5)));
    let def = HanabiEffectDefinition { alpha_mode: ParticleAlphaMode::Premultiply, billboard_mode: BillboardMode::FaceCameraY, ..Default::default() };
    let (asset, _) = convert(&def);
    assert_eq!(asset.emitters[0].render.blend, crate::asset::BlendMode::Premultiplied);
    assert_eq!(asset.emitters[0].render.facing, Facing::FixedAxis([0.0, 1.0, 0.0]));
}

/// Hanabi's alpha mask and opaque modes are the depth-written blend modes,
/// the mask with its threshold as the cutoff, and neither warns any more.
/// Multiply, the last that had no equivalent, now converts to its own mode.
#[test]
fn alpha_mask_and_opaque_convert_to_their_blend_modes() {
    use crate::asset::BlendMode as B;
    let mask = HanabiEffectDefinition { alpha_mode: ParticleAlphaMode::Mask, alpha_mask_threshold: 0.3, ..Default::default() };
    let (asset, warnings) = convert(&mask);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(asset.emitters[0].render.blend, B::AlphaMask);
    assert!((asset.emitters[0].render.alpha_cutoff - 0.3).abs() < 1e-6);
    let opaque = HanabiEffectDefinition { alpha_mode: ParticleAlphaMode::Opaque, ..Default::default() };
    let (asset, warnings) = convert(&opaque);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(asset.emitters[0].render.blend, B::Opaque);
    // Round trip through the file: the converted effect saves what it maps.
    let text = crate::asset::write_vfx(&asset).unwrap();
    assert_eq!(crate::asset::parse_vfx(&text).unwrap().emitters[0].render.blend, B::Opaque);
    let multiply = HanabiEffectDefinition { alpha_mode: ParticleAlphaMode::Multiply, ..Default::default() };
    let (asset, warnings) = convert(&multiply);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(asset.emitters[0].render.blend, B::Multiply);
}

#[test]
fn features_without_a_node_are_named_in_the_warnings() {
    let def = HanabiEffectDefinition {
        kill_zones: vec![KillZone::default()],
        velocity_limit: 3.0,
        ribbon: Some(RibbonSettings::default()),
        light: Some(ParticleLightSettings::default()),
        screen_space_size: true,
        simulation_space: SimulationSpace::World,
        ..Default::default()
    };
    let (asset, warnings) = convert(&def);
    assert!(warnings.is_empty(), "{warnings:?}");
    // These have equivalents now and say nothing: screen-space size is the
    // render setting of the same name. A world-space effect's kill zone stays
    // in the world, as it did.
    assert!(asset.emitters[0].render.screen_space_size);
    let zone = asset.emitters[0].graph.nodes.iter().find(|n| n.node_type == "collide/sphere").expect("zone");
    assert_eq!(zone.input_values.get("world"), Some(&PinValue::Bool(true)));
    let local = HanabiEffectDefinition { kill_zones: vec![KillZone::default()], ..Default::default() };
    let (asset, warnings) = convert(&local);
    assert!(warnings.is_empty());
    let zone = asset.emitters[0].graph.nodes.iter().find(|n| n.node_type == "collide/sphere").expect("zone");
    assert_eq!(zone.input_values.get("world"), None);
}

#[test]
fn noise_octaves_and_lacunarity_map_exactly() {
    let def = HanabiEffectDefinition {
        noise_amplitude: 1.5,
        noise_frequency: 2.0,
        noise_octaves: 4,
        noise_lacunarity: 2.5,
        ..Default::default()
    };
    let (asset, warnings) = convert(&def);
    assert!(warnings.is_empty(), "{warnings:?}");
    let g = &asset.emitters[0].graph;
    let noise = g.nodes.iter().find(|n| n.node_type == "force/curl_noise").expect("curl noise");
    assert_eq!(noise.input_values.get("octaves"), Some(&PinValue::Float(4.0)));
    assert_eq!(noise.input_values.get("lacunarity"), Some(&PinValue::Float(2.5)));
    // The old persistence was fixed at a half, the node's default.
    assert_eq!(noise.input_values.get("persistence"), None);
    assert_eq!(noise.input_values.get("scroll"), Some(&PinValue::Vec3([0.5, 0.5, 0.5])), "the old drift");
    // One octave stays the plain node.
    let one = HanabiEffectDefinition { noise_octaves: 1, ..def.clone() };
    let (asset, _) = convert(&one);
    let noise = asset.emitters[0].graph.nodes.iter().find(|n| n.node_type == "force/curl_noise").expect("noise");
    assert!(!noise.input_values.contains_key("octaves") && !noise.input_values.contains_key("lacunarity"));
}

#[test]
fn a_velocity_limit_is_the_outputs_max_speed() {
    let (asset, warnings) = convert(&HanabiEffectDefinition { velocity_limit: 3.0, ..Default::default() });
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(inline(&asset.emitters[0].graph, "max_speed"), Some(&PinValue::Float(3.0)));
}

#[test]
fn the_effect_light_is_a_per_emitter_light_as_bright_in_full_flow() {
    let l = ParticleLightSettings { color: [1.0, 0.5, 0.2], intensity: 120_000.0, range: 9.0, flicker: 0.4, shadows: true };
    let def = HanabiEffectDefinition { light: Some(l), spawn_rate: 40.0, lifetime_min: 1.0, lifetime_max: 1.0, ..Default::default() };
    let (asset, warnings) = convert(&def);
    assert!(warnings.is_empty(), "{warnings:?}");
    let light = asset.emitters[0].light;
    assert_eq!(light.mode, crate::asset::LightMode::PerEmitter);
    assert_eq!(light.color, [1.0, 0.5, 0.2]);
    assert_eq!(light.range, 9.0);
    assert!(light.shadows && light.flicker == 0.4);
    // Run it into its steady state and measure: the light the particles
    // make, at the old flicker's average, is the old lumens.
    let (mut sim, _) = EffectSim::new(&asset);
    sim.simulate_to(3.0, &FrameInput::default());
    let mut out = Vec::new();
    crate::light::aggregate(&sim.emitters[0].particles, &light, bevy::math::Affine3A::IDENTITY, &mut out);
    let average = out[0].intensity * (1.0 - 0.5 * light.flicker);
    assert!((average / 120_000.0 - 1.0).abs() < 0.15, "{average} lm");
}

#[test]
fn a_ribbon_effect_strings_its_particles_like_the_old_one() {
    let def = HanabiEffectDefinition { ribbon: Some(RibbonSettings { groups: 3 }), ..Default::default() };
    let (asset, warnings) = convert(&def);
    assert!(warnings.is_empty(), "{warnings:?}");
    let render = &asset.emitters[0].render;
    assert_eq!(render.facing, Facing::Ribbon);
    assert_eq!(render.ribbon.kind, crate::asset::RibbonKind::Connect);
    assert_eq!(render.ribbon.groups, 3);
    assert_eq!((render.ribbon.tail_width, render.ribbon.tail_alpha), (1.0, 1.0), "no taper, as before");
    // And it draws.
    let (mut sim, _) = EffectSim::new(&asset);
    sim.simulate_to(1.0, &FrameInput::default());
    let mut mesh = bevy::mesh::Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    assert!(crate::render::write_mesh(&mut mesh, &sim.emitters[0].particles, render, bevy::math::Affine3A::IDENTITY).is_some());
}

#[test]
fn kill_zones_become_collision_nodes_that_kill() {
    let def = HanabiEffectDefinition {
        kill_zones: vec![
            KillZone::Sphere { center: [0.0, 2.0, 0.0], radius: 0.5, kill_inside: true },
            KillZone::Aabb { center: [0.0, 0.0, 0.0], half_size: [3.0, 3.0, 3.0], kill_inside: false },
        ],
        ..Default::default()
    };
    let (asset, warnings) = convert(&def);
    assert!(warnings.is_empty(), "{warnings:?}");
    let g = &asset.emitters[0].graph;
    let sphere = g.nodes.iter().find(|n| n.node_type == "collide/sphere").expect("the sphere survived pruning");
    assert_eq!(sphere.input_values.get("center"), Some(&PinValue::Vec3([0.0, 2.0, 0.0])));
    assert_eq!(sphere.input_values.get("radius"), Some(&PinValue::Float(0.5)));
    assert_eq!(sphere.input_values.get("kill"), Some(&PinValue::Bool(true)));
    assert_eq!(sphere.input_values.get("inside"), None, "killing what is inside is a solid ball");
    let cube = g.nodes.iter().find(|n| n.node_type == "collide/box").expect("box");
    assert_eq!(cube.input_values.get("size"), Some(&PinValue::Vec3([6.0, 6.0, 6.0])));
    assert_eq!(cube.input_values.get("inside"), Some(&PinValue::Bool(true)), "killing what leaves is a room");
    let compiled = compile_graph(g).expect("compiles");
    assert_eq!(compiled.colliders.len(), 2);
    assert!(compiled.colliders.iter().all(|c| c.kill));
}

#[test]
fn erosion_converts_to_the_render_erosion() {
    let (asset, warnings) = convert(&HanabiEffectDefinition { erosion: true, ..Default::default() });
    assert!(asset.emitters[0].render.erosion > 0.0);
    assert!(!warnings.iter().any(|w| w == "erosion"), "{warnings:?}");
    let (plain, _) = convert(&HanabiEffectDefinition::default());
    assert_eq!(plain.emitters[0].render.erosion, 0.0);
}

#[test]
fn a_plain_effect_converts_without_warnings() {
    let (_, warnings) = convert(&HanabiEffectDefinition::default());
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn a_broken_particle_text_is_an_error_not_a_default_effect() {
    assert!(convert_text("(this is not an effect", "bad").is_err());
}

#[test]
fn an_unnamed_effect_takes_the_fallback_name() {
    let def = HanabiEffectDefinition { name: "  ".into(), ..Default::default() };
    let c = convert_text(&ron::to_string(&def).unwrap(), "torch").unwrap();
    assert_eq!(c.asset.name, "torch");
}

#[test]
fn a_scene_path_gets_its_sibling_vfx_name() {
    assert_eq!(sibling_vfx_str("particles/fire.particle"), "particles/fire.vfx");
    assert_eq!(sibling_vfx_str("fire.particle"), "fire.vfx");
    // A dot in a folder name is not the extension.
    assert_eq!(sibling_vfx_str("fx.v2/fire"), "fx.v2/fire.vfx");
    assert!(is_particle_path("a/B.PARTICLE"));
    assert!(!is_particle_path("a/b.vfx"));
}

/// The `source` field of a scene's old component. `Asset` is a struct variant,
/// so its body is `Asset(path: "...")`; the newtype spelling with a second set
/// of parentheses never parsed, and still must not, or the migration would
/// accept scenes the old engine silently dropped the component from.
#[test]
fn the_effect_source_keeps_its_old_spelling() {
    let ok: EffectSource = ron::from_str(r#"Asset(path:"particles/fountain.particle")"#).unwrap();
    assert!(matches!(ok, EffectSource::Asset { path } if path == "particles/fountain.particle"));
    assert!(ron::from_str::<EffectSource>(r#"Asset((path:"particles/fountain.particle"))"#).is_err());
    let inline: EffectSource = ron::from_str(&format!(
        "Inline(definition:{})",
        ron::to_string(&HanabiEffectDefinition::default()).unwrap()
    ))
    .unwrap();
    assert!(matches!(inline, EffectSource::Inline { .. }));
}
