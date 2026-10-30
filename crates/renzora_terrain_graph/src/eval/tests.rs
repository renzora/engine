use std::sync::Arc;

use bevy::math::Vec2;

use super::*;
use crate::catalog::{PinKind, NODES};
use crate::field::{Field, Grid};
use crate::format::{ParamValue, TerrainGraph};

fn grid(n: usize) -> Grid {
    Grid::fitting(Vec2::new(256.0, 256.0), n)
}

fn run(g: &TerrainGraph, req: &EvalRequest) -> EvalResult {
    evaluate(g, req, &mut EvalCache::default(), &EvalControl::new()).expect("not cancelled")
}

/// One node fed by a noise into every Height input and a gradient into every
/// Mask input, out to the terrain.
fn single(node_type: &str, params: &[(&str, ParamValue)]) -> (TerrainGraph, u64) {
    let mut g = TerrainGraph::new("t");
    let src = g.add_node("noise", [0.0, 0.0]);
    g.set_param(src, "height", ParamValue::Number(50.0));
    let mask = g.add_node("gradient", [0.0, 0.0]);
    let id = g.add_node(node_type, [0.0, 0.0]);
    for (k, v) in params {
        g.set_param(id, k, v.clone());
    }
    let def = crate::catalog::node_def(node_type).unwrap();
    for pin in def.inputs {
        let from = if pin.kind == PinKind::Mask && !pin.required { mask } else { src };
        let from_pin = if from == mask { "mask" } else { "height" };
        g.connect(from, from_pin, id, pin.name).unwrap();
    }
    if !def.is_output() {
        if let Some(first) = def.outputs.first() {
            let out = g.add_node("terrain_output", [0.0, 0.0]);
            g.set_param(out, "mode", ParamValue::Text("absolute".into()));
            g.connect(id, first.name, out, "height").unwrap();
        }
    }
    (g, id)
}

fn outputs(r: &EvalResult, id: u64) -> &[Field] {
    &r.nodes[&id].outputs
}

#[test]
fn every_catalog_node_is_implemented() {
    for def in NODES {
        assert!(is_implemented(def.node_type), "{} has no implementation", def.node_type);
    }
}

#[test]
fn every_node_runs_on_its_defaults() {
    let current = Arc::new(Field::filled(9, 9, 12.0));
    for def in NODES {
        let (g, id) = single(def.node_type, &[]);
        let mut req = EvalRequest::new(grid(33), 5);
        req.current = Some(current.clone());
        let r = run(&g, &req);
        let res = &r.nodes[&id];
        if def.node_type == "import_heightmap" {
            assert!(res.error.is_some(), "an import with no file should say so");
            continue;
        }
        assert!(res.error.is_none(), "{}: {:?}", def.node_type, res.error);
        let want = if def.is_output() { 1 } else { def.outputs.len() };
        assert_eq!(res.outputs.len(), want, "{}", def.node_type);
        for (o, f) in res.outputs.iter().enumerate() {
            assert_eq!((f.w, f.h), (33, 33), "{} output {o}", def.node_type);
            assert!(f.data.iter().all(|v| v.is_finite()), "{} output {o} not finite", def.node_type);
            let kind = def.outputs.get(o).map(|p| p.kind);
            if kind == Some(PinKind::Mask) {
                let (lo, hi) = f.min_max();
                assert!(lo >= 0.0 && hi <= 1.0, "{} mask {o}: {lo}..{hi}", def.node_type);
            }
        }
    }
}

#[test]
fn evaluation_is_deterministic() {
    let g = crate::presets::highlands();
    // 129 square crosses the parallel threshold, so the row bands are in play.
    let req = EvalRequest::new(grid(129), 1337);
    let a = run(&g, &req);
    let b = run(&g, &req);
    assert_eq!(a.height, b.height);
    assert_eq!(a.masks, b.masks);
    let mut other = req.clone();
    other.seed = 1338;
    let c = run(&g, &other);
    assert_ne!(a.height, c.height, "a different seed is a different terrain");
}

#[test]
fn a_quarter_resolution_preview_looks_like_the_build() {
    let (g, _) = single("noise", &[("mode", ParamValue::Text("fbm".into()))]);
    let full = run(&g, &EvalRequest::new(grid(129), 3)).height.unwrap();
    let quarter = run(&g, &EvalRequest::new(grid(33), 3)).height.unwrap();
    // Every quarter sample sits exactly on a full one.
    let mut worst = 0.0f32;
    for z in 0..33 {
        for x in 0..33 {
            worst = worst.max((quarter.at(x, z) - full.at(x * 4, z * 4)).abs());
        }
    }
    assert!(worst < 1e-3, "{worst}");
    // And a radius is metres, not cells: a blur reads alike at both sizes.
    let (g, id) = single("blur", &[("radius", ParamValue::Number(20.0))]);
    let full = run(&g, &EvalRequest::new(grid(129), 3));
    let quarter = run(&g, &EvalRequest::new(grid(33), 3));
    let (f, q) = (&outputs(&full, id)[0], &outputs(&quarter, id)[0]);
    let mut err = 0.0f32;
    for z in 0..33 {
        for x in 0..33 {
            err += (q.at(x, z) - f.at(x * 4, z * 4)).abs();
        }
    }
    assert!(err / (33.0 * 33.0) < 1.0, "mean blur difference {}", err / 1089.0);
}

#[test]
fn the_cache_reruns_only_what_an_edit_touches() {
    let mut g = TerrainGraph::new("t");
    let a = g.add_node("noise", [0.0, 0.0]);
    let b = g.add_node("blur", [0.0, 0.0]);
    let c = g.add_node("terrain_output", [0.0, 0.0]);
    let side = g.add_node("noise", [0.0, 0.0]);
    g.connect(a, "height", b, "height").unwrap();
    g.connect(b, "height", c, "height").unwrap();
    let req = EvalRequest::new(grid(17), 1);
    let mut cache = EvalCache::default();
    let ctl = EvalControl::new();
    let first = evaluate(&g, &req, &mut cache, &ctl).unwrap();
    let entries = cache.len();
    let again = evaluate(&g, &req, &mut cache, &ctl).unwrap();
    assert_eq!(cache.len(), entries, "nothing new to compute");
    assert_eq!(first.height, again.height);
    g.set_param(b, "radius", ParamValue::Number(30.0));
    let edited = evaluate(&g, &req, &mut cache, &ctl).unwrap();
    assert_eq!(edited.nodes[&a].key, first.nodes[&a].key, "upstream untouched");
    assert_eq!(edited.nodes[&side].key, first.nodes[&side].key, "unrelated untouched");
    assert_ne!(edited.nodes[&b].key, first.nodes[&b].key);
    assert_ne!(edited.nodes[&c].key, first.nodes[&c].key, "downstream follows");
    // Back to the old value: a cache hit, the same result as the first run.
    g.set_param(b, "radius", ParamValue::Number(8.0));
    let back = evaluate(&g, &req, &mut cache, &ctl).unwrap();
    assert_eq!(back.height, first.height);
}

#[test]
fn a_cancelled_run_returns_nothing() {
    let g = crate::presets::highlands();
    let ctl = EvalControl::new();
    ctl.cancel();
    let r = evaluate(&g, &EvalRequest::new(grid(33), 1), &mut EvalCache::default(), &ctl);
    assert_eq!(r.err(), Some(Cancelled));
}

#[test]
fn progress_reaches_one() {
    let g = crate::presets::rolling_hills();
    let ctl = EvalControl::new();
    evaluate(&g, &EvalRequest::new(grid(17), 1), &mut EvalCache::default(), &ctl).unwrap();
    assert_eq!(ctl.progress(), 1.0);
}

#[test]
fn exposed_overrides_reach_their_node() {
    let g = crate::presets::empty();
    let mut req = EvalRequest::new(grid(17), 1);
    let base = run(&g, &req).height.unwrap();
    assert!((base.min_max().1 - 60.0).abs() < 1e-3);
    req.overrides = vec![("peak_height".into(), 250.0)];
    let tall = run(&g, &req).height.unwrap();
    assert!((tall.min_max().1 - 250.0).abs() < 1e-3);
}

#[test]
fn a_missing_required_input_is_an_error_on_that_node() {
    let mut g = TerrainGraph::new("t");
    let blur = g.add_node("blur", [0.0, 0.0]);
    let r = run(&g, &EvalRequest::new(grid(9), 1));
    assert!(r.nodes[&blur].error.as_deref().is_some_and(|e| e.contains("needs an input")));
    assert!(r.errors.iter().any(|(n, _)| n.is_none()), "no terrain output is reported");
}

// ── Node families ───────────────────────────────────────────────────────────

#[test]
fn primitives_have_their_shapes() {
    // Crater: below zero in the middle, a rim above zero.
    let (g, id) = single("crater", &[]);
    let r = run(&g, &EvalRequest::new(grid(65), 1));
    let f = &outputs(&r, id)[0];
    assert!(f.at(32, 32) < -20.0);
    assert!(f.min_max().1 > 5.0);
    // Plateau: flat top at its height.
    let (g, id) = single("plateau", &[("edge_noise", ParamValue::Number(0.0))]);
    let r = run(&g, &EvalRequest::new(grid(65), 1));
    assert!((outputs(&r, id)[0].at(32, 32) - 40.0).abs() < 1e-3);
    // Volcano: the summit crater dips below the rim around it.
    let (g, id) = single("volcano", &[("roughness", ParamValue::Number(0.0))]);
    let r = run(&g, &EvalRequest::new(grid(65), 1));
    let f = &outputs(&r, id)[0];
    assert!(f.at(32, 32) < f.at(32 + 8, 32), "{} vs {}", f.at(32, 32), f.at(40, 32));
    // Gradient: north end high.
    let (g, id) = single("gradient", &[]);
    let r = run(&g, &EvalRequest::new(grid(17), 1));
    let f = &outputs(&r, id)[0];
    assert!(f.at(8, 16) > 0.99 && f.at(8, 0) < 0.01);
    // Dunes and cells stay inside their height.
    for t in ["dunes", "cells", "mountain_range"] {
        let (g, id) = single(t, &[]);
        let r = run(&g, &EvalRequest::new(grid(33), 1));
        let (lo, hi) = outputs(&r, id)[0].min_max();
        assert!(lo >= -1e-3 && hi > 0.0, "{t}: {lo}..{hi}");
    }
}

#[test]
fn an_imported_heightmap_is_read_and_stretched() {
    let dir = std::env::temp_dir().join(format!("tgraph-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // A 4 by 4 RAW16 ramp, black at the left to white at the right.
    let mut bytes = Vec::new();
    for _z in 0..4 {
        for x in 0..4u16 {
            bytes.extend_from_slice(&(x * 21845).to_le_bytes());
        }
    }
    std::fs::write(dir.join("ramp.r16"), &bytes).unwrap();
    let (g, id) = single("import_heightmap", &[("path", ParamValue::Text("ramp.r16".into())), ("height", ParamValue::Number(80.0))]);
    let mut req = EvalRequest::new(grid(17), 1);
    req.root = Some(dir.clone());
    let r = run(&g, &req);
    assert!(r.nodes[&id].error.is_none(), "{:?}", r.nodes[&id].error);
    let f = &outputs(&r, id)[0];
    assert!(f.at(0, 8) < 1.0 && (f.at(16, 8) - 80.0).abs() < 1.0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn shape_nodes_do_what_they_say() {
    // Combine max is at least both inputs.
    let mut g = TerrainGraph::new("t");
    let a = g.add_node("noise", [0.0, 0.0]);
    let b = g.add_node("noise", [0.0, 0.0]);
    let m = g.add_node("combine", [0.0, 0.0]);
    g.set_param(m, "mode", ParamValue::Text("max".into()));
    g.connect(a, "height", m, "a").unwrap();
    g.connect(b, "height", m, "b").unwrap();
    let r = run(&g, &EvalRequest::new(grid(17), 1));
    let (fa, fb, fm) = (&outputs(&r, a)[0], &outputs(&r, b)[0], &outputs(&r, m)[0]);
    for i in 0..fm.data.len() {
        assert!(fm.data[i] >= fa.data[i].max(fb.data[i]) - 1e-5);
    }
    // Terrace leaves only a few distinct levels at full sharpness.
    let (g, id) = single("terrace", &[("step", ParamValue::Number(10.0)), ("sharpness", ParamValue::Number(1.0))]);
    let r = run(&g, &EvalRequest::new(grid(33), 1));
    let on_tread = outputs(&r, id)[0].data.iter().filter(|v| ((*v / 10.0) - (*v / 10.0).round()).abs() < 0.02).count();
    assert!(on_tread * 2 > 33 * 33, "{on_tread} of {} on a tread", 33 * 33);
    // Height Range spans exactly min..max.
    let (g, id) = single("height_range", &[("min", ParamValue::Number(-5.0)), ("max", ParamValue::Number(15.0))]);
    let r = run(&g, &EvalRequest::new(grid(17), 1));
    let (lo, hi) = outputs(&r, id)[0].min_max();
    assert!((lo + 5.0).abs() < 1e-3 && (hi - 15.0).abs() < 1e-3);
    // Clamp, invert, curve.
    let (g, id) = single("clamp", &[("max", ParamValue::Number(10.0))]);
    let r = run(&g, &EvalRequest::new(grid(17), 1));
    assert!(outputs(&r, id)[0].min_max().1 <= 10.0);
    let (g, id) = single("invert", &[]);
    let r = run(&g, &EvalRequest::new(grid(17), 1));
    let src = &outputs(&r, 1)[0];
    let (lo, hi) = src.min_max();
    assert!((outputs(&r, id)[0].at(3, 3) - (lo + hi - src.at(3, 3))).abs() < 1e-4);
    assert!((crate::eval::shape::curve_eval(&[[0.0, 0.0], [1.0, 1.0]], 0.3) - 0.3).abs() < 1e-5);
    assert!(crate::eval::shape::curve_eval(&[[0.0, 0.0], [0.5, 0.8], [1.0, 1.0]], 0.5) > 0.79);
    // Blur smooths.
    let (g, id) = single("blur", &[("radius", ParamValue::Number(30.0))]);
    let r = run(&g, &EvalRequest::new(grid(33), 1));
    let var = |f: &Field| {
        let mean = f.data.iter().sum::<f32>() / f.data.len() as f32;
        f.data.iter().map(|v| (v - mean).powi(2)).sum::<f32>()
    };
    assert!(var(&outputs(&r, id)[0]) < var(&outputs(&r, 1)[0]));
    // Flatten pulls to its height where the mask is full.
    let (g, id) = single("flatten", &[("height", ParamValue::Number(7.0))]);
    let r = run(&g, &EvalRequest::new(grid(17), 1));
    assert!((outputs(&r, id)[0].at(8, 16) - 7.0).abs() < 0.1, "north end, mask 1");
}

#[test]
fn simulations_move_ground_and_report_masks() {
    let (g, id) = single("erosion", &[("rain", ParamValue::Number(2.0))]);
    let r = run(&g, &EvalRequest::new(grid(65), 9));
    let src = &outputs(&r, 1)[0];
    let out = outputs(&r, id);
    let moved = out[0].data.iter().zip(&src.data).filter(|(a, b)| (*a - *b).abs() > 1e-3).count();
    assert!(moved > 100, "erosion moved {moved} samples");
    assert!(out[1].min_max().1 > 0.5, "water runs somewhere");
    assert!(out[2].min_max().1 > 0.5, "sediment lands somewhere");

    // Lakes fill a pit and report the water.
    let mut g = TerrainGraph::new("t");
    let pit = g.add_node("crater", [0.0, 0.0]);
    let lake = g.add_node("lakes", [0.0, 0.0]);
    g.set_param(lake, "fill", ParamValue::Bool(true));
    g.connect(pit, "height", lake, "height").unwrap();
    let r = run(&g, &EvalRequest::new(grid(65), 1));
    let out = outputs(&r, lake);
    assert!(out[1].at(32, 32) > 0.99, "the crater floor is under water");
    assert!(out[0].at(32, 32) > outputs(&r, pit)[0].at(32, 32));

    // Snow on the high ground only.
    let mut g = TerrainGraph::new("t");
    let ramp = g.add_node("gradient", [0.0, 0.0]);
    let tall = g.add_node("height_range", [0.0, 0.0]);
    g.set_param(tall, "max", ParamValue::Number(40.0));
    g.connect(ramp, "mask", tall, "height").unwrap();
    let snow = g.add_node("snow", [0.0, 0.0]);
    g.set_param(snow, "altitude", ParamValue::Number(30.0));
    g.set_param(snow, "blend", ParamValue::Number(2.0));
    g.connect(tall, "height", snow, "height").unwrap();
    let r = run(&g, &EvalRequest::new(grid(33), 1));
    let mask = &outputs(&r, snow)[1];
    assert!(mask.at(16, 32) > 0.9 && mask.at(16, 0) < 0.01);

    // Rivers carve where flow gathers.
    let (g, id) = single("rivers", &[("catchment", ParamValue::Number(500.0))]);
    let r = run(&g, &EvalRequest::new(grid(65), 2));
    let out = outputs(&r, id);
    assert!(out[1].min_max().1 > 0.5);
    let src = &outputs(&r, 1)[0];
    assert!(out[0].data.iter().zip(&src.data).any(|(a, b)| b - a > 1.0));
}

#[test]
fn data_masks_measure_the_ground() {
    // A flat terrain: slope 0 everywhere, so a 0..25 degree mask is full.
    let mut g = TerrainGraph::new("t");
    let flat = g.add_node("constant", [0.0, 0.0]);
    let slope = g.add_node("slope", [0.0, 0.0]);
    g.connect(flat, "height", slope, "height").unwrap();
    let hm = g.add_node("height_mask", [0.0, 0.0]);
    g.set_param(hm, "min", ParamValue::Number(10.0));
    g.connect(flat, "height", hm, "height").unwrap();
    let r = run(&g, &EvalRequest::new(grid(17), 1));
    assert!(outputs(&r, slope)[0].data.iter().all(|v| *v > 0.99));
    assert!(outputs(&r, hm)[0].data.iter().all(|v| *v < 0.01), "0 m is below the band");

    // A ramp up to the north faces south.
    let mut g = TerrainGraph::new("t");
    let ramp = g.add_node("gradient", [0.0, 0.0]);
    let tall = g.add_node("height_range", [0.0, 0.0]);
    g.connect(ramp, "mask", tall, "height").unwrap();
    let south = g.add_node("aspect", [0.0, 0.0]);
    g.connect(tall, "height", south, "height").unwrap();
    let north = g.add_node("aspect", [0.0, 0.0]);
    g.set_param(north, "direction", ParamValue::Number(0.0));
    g.connect(tall, "height", north, "height").unwrap();
    let flow = g.add_node("flow", [0.0, 0.0]);
    g.connect(tall, "height", flow, "height").unwrap();
    let r = run(&g, &EvalRequest::new(grid(33), 1));
    assert!(outputs(&r, south)[0].at(16, 16) > 0.9);
    assert!(outputs(&r, north)[0].at(16, 16) < 0.1);
    let f = &outputs(&r, flow)[0];
    assert!(f.at(16, 0) > f.at(16, 32), "water gathers downhill");
}
