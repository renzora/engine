//! The graphs the engine ships: Highlands, Rolling Hills and Desert.
//!
//! Built in Rust rather than kept as JSON files, so a preset cannot fall out
//! of step with the catalog: a renamed parameter is a compile error here, and
//! the tests below evaluate every preset and check its ranges. The editor
//! writes them into a project from the Assets panel's create menu.

use crate::format::{ParamValue, TerrainGraph};

/// One preset: `(id, display name, one-line description, builder)`.
pub type Preset = (&'static str, &'static str, &'static str, fn() -> TerrainGraph);

/// Every preset.
pub const PRESETS: &[Preset] = &[
    ("highlands", "Highlands", "Eroded mountain ranges over meadow foothills", highlands),
    ("rolling_hills", "Rolling Hills", "Soft grassy hills with gentle valleys", rolling_hills),
    ("desert", "Desert", "Dune fields around terraced mesas", desert),
    ("empty", "Empty", "One noise into the terrain output", empty),
];

fn n(v: f32) -> ParamValue {
    ParamValue::Number(v)
}

fn t(v: &str) -> ParamValue {
    ParamValue::Text(v.to_string())
}

/// A small builder so a preset reads as a list of nodes and wires.
struct Builder {
    g: TerrainGraph,
}

impl Builder {
    fn new(name: &str) -> Self {
        Self { g: TerrainGraph::new(name) }
    }

    fn node(&mut self, node_type: &str, x: f32, y: f32, params: &[(&str, ParamValue)]) -> u64 {
        debug_assert!(crate::catalog::node_def(node_type).is_some(), "{node_type}");
        let id = self.g.add_node(node_type, [x, y]);
        for (k, v) in params {
            debug_assert!(
                crate::catalog::node_def(node_type).and_then(|d| d.param(k)).is_some(),
                "{node_type}.{k}"
            );
            self.g.set_param(id, k, v.clone());
        }
        id
    }

    fn wire(&mut self, from: u64, from_pin: &str, to: u64, to_pin: &str) {
        self.g.connect(from, from_pin, to, to_pin).expect("preset wiring is acyclic");
    }

    fn mask(&mut self, from: u64, from_pin: &str, name: &str, x: f32, y: f32) {
        let out = self.node("mask_output", x, y, &[("name", t(name))]);
        self.wire(from, from_pin, out, "mask");
    }

    fn finish(self) -> TerrainGraph {
        self.g
    }
}

/// Mountain ranges and ridged noise combined by Max, eroded, out to the
/// terrain; Gravel where water runs, Dirt where it drops sediment, Meadow
/// grass on the low, gentle, southern foothills.
pub fn highlands() -> TerrainGraph {
    let mut b = Builder::new("Highlands");
    let range = b.node(
        "mountain_range",
        0.0,
        0.0,
        &[("scale", n(800.0)), ("height", n(260.0)), ("direction", n(35.0)), ("sharpness", n(1.3))],
    );
    let ridged = b.node("noise", 0.0, 220.0, &[("mode", t("ridge")), ("scale", n(160.0)), ("height", n(90.0)), ("exponent", n(1.6))]);
    let combine = b.node("combine", 280.0, 100.0, &[("mode", t("max")), ("ratio", n(0.4))]);
    b.wire(range, "height", combine, "a");
    b.wire(ridged, "height", combine, "b");
    let erosion = b.node("erosion", 560.0, 100.0, &[("rain", n(1.2)), ("thermal", n(10.0))]);
    b.wire(combine, "height", erosion, "height");
    let out = b.node("terrain_output", 860.0, 40.0, &[("mode", t("fit")), ("floor", n(0.0)), ("peak", n(290.0))]);
    b.wire(erosion, "height", out, "height");

    b.mask(erosion, "flow", "Gravel", 860.0, 220.0);
    b.mask(erosion, "deposits", "Dirt", 860.0, 300.0);

    // Foothills: low ground, gentle ground, and more of it to the south.
    let gradient = b.node("gradient", 0.0, 420.0, &[("direction", n(180.0))]);
    let curve = b.node("curve", 280.0, 420.0, &[("points", ParamValue::Points(vec![[0.0, 0.15], [0.5, 0.4], [1.0, 1.0]]))]);
    b.wire(gradient, "mask", curve, "height");
    let low = b.node("height_mask", 560.0, 330.0, &[("min", n(-50.0)), ("max", n(70.0)), ("falloff", n(30.0))]);
    b.wire(erosion, "height", low, "height");
    let gentle = b.node("slope", 560.0, 430.0, &[("min", n(0.0)), ("max", n(18.0)), ("falloff", n(6.0))]);
    b.wire(erosion, "height", gentle, "height");
    let both = b.node("combine", 760.0, 380.0, &[("mode", t("multiply"))]);
    b.wire(low, "mask", both, "a");
    b.wire(gentle, "mask", both, "b");
    let south = b.node("combine", 760.0, 480.0, &[("mode", t("multiply"))]);
    b.wire(both, "height", south, "a");
    b.wire(curve, "height", south, "b");
    b.mask(south, "height", "Meadow grass", 1000.0, 480.0);

    let mut g = b.finish();
    g.expose("peak_height", "Peak height", out, "peak", 10.0, 3000.0);
    g.expose("range_scale", "Range scale", range, "scale", 50.0, 20000.0);
    g.expose("rain", "Rain", erosion, "rain", 0.0, 10.0);
    g
}

/// Broad FBM hills with billowy detail, warped so nothing runs in rows, a
/// light erosion pass, and Grass, Dirt and Rock masks.
pub fn rolling_hills() -> TerrainGraph {
    let mut b = Builder::new("Rolling Hills");
    let broad = b.node("noise", 0.0, 0.0, &[("mode", t("fbm")), ("scale", n(350.0)), ("height", n(40.0)), ("octaves", n(5.0))]);
    let detail = b.node("noise", 0.0, 200.0, &[("mode", t("billow")), ("scale", n(90.0)), ("height", n(6.0)), ("octaves", n(4.0))]);
    let add = b.node("combine", 260.0, 80.0, &[("mode", t("add"))]);
    b.wire(broad, "height", add, "a");
    b.wire(detail, "height", add, "b");
    let warp = b.node("warp", 500.0, 80.0, &[("strength", n(25.0)), ("scale", n(220.0))]);
    b.wire(add, "height", warp, "height");
    let erosion = b.node("erosion", 740.0, 80.0, &[("rain", n(0.6)), ("thermal", n(4.0)), ("strength", n(0.2))]);
    b.wire(warp, "height", erosion, "height");
    let out = b.node("terrain_output", 1020.0, 40.0, &[("mode", t("fit")), ("floor", n(0.0)), ("peak", n(55.0))]);
    b.wire(erosion, "height", out, "height");

    let grass = b.node("slope", 1020.0, 200.0, &[("min", n(0.0)), ("max", n(18.0)), ("falloff", n(6.0))]);
    b.wire(erosion, "height", grass, "height");
    b.mask(grass, "mask", "Grass", 1260.0, 200.0);
    b.mask(erosion, "deposits", "Dirt", 1020.0, 300.0);
    let rock = b.node("slope", 1020.0, 400.0, &[("min", n(24.0)), ("max", n(90.0)), ("falloff", n(6.0))]);
    b.wire(erosion, "height", rock, "height");
    b.mask(rock, "mask", "Rock", 1260.0, 400.0);

    let mut g = b.finish();
    g.expose("peak_height", "Peak height", out, "peak", 5.0, 1000.0);
    g.expose("hill_scale", "Hill scale", broad, "scale", 20.0, 5000.0);
    g
}

/// Dunes over a slow undulation, Max-combined with two terraced mesas, then
/// weathered. Sand on the gentle ground, Rock on the mesa walls.
pub fn desert() -> TerrainGraph {
    let mut b = Builder::new("Desert");
    let dunes = b.node("dunes", 0.0, 0.0, &[("wavelength", n(70.0)), ("height", n(9.0)), ("direction", n(60.0))]);
    let swell = b.node("noise", 0.0, 180.0, &[("mode", t("fbm")), ("scale", n(500.0)), ("height", n(12.0)), ("octaves", n(3.0))]);
    let sand = b.node("combine", 260.0, 80.0, &[("mode", t("add"))]);
    b.wire(dunes, "height", sand, "a");
    b.wire(swell, "height", sand, "b");

    let mesa_a = b.node(
        "plateau",
        0.0,
        360.0,
        &[("offset_x", n(-70.0)), ("offset_z", n(50.0)), ("radius", n(55.0)), ("height", n(45.0)), ("edge", n(25.0)), ("edge_noise", n(0.45))],
    );
    let mesa_b = b.node(
        "plateau",
        0.0,
        560.0,
        &[("offset_x", n(80.0)), ("offset_z", n(-60.0)), ("radius", n(35.0)), ("height", n(30.0)), ("edge", n(18.0)), ("edge_noise", n(0.4)), ("seed", n(3.0))],
    );
    let mesas = b.node("combine", 260.0, 440.0, &[("mode", t("max"))]);
    b.wire(mesa_a, "height", mesas, "a");
    b.wire(mesa_b, "height", mesas, "b");
    let terrace = b.node("terrace", 500.0, 440.0, &[("step", n(7.0)), ("sharpness", n(0.9))]);
    b.wire(mesas, "height", terrace, "height");

    let all = b.node("combine", 740.0, 200.0, &[("mode", t("max"))]);
    b.wire(sand, "height", all, "a");
    b.wire(terrace, "height", all, "b");
    let weather = b.node("thermal", 960.0, 200.0, &[("passes", n(20.0)), ("repose", n(33.0))]);
    b.wire(all, "height", weather, "height");
    let out = b.node("terrain_output", 1200.0, 160.0, &[("mode", t("fit")), ("floor", n(0.0)), ("peak", n(70.0))]);
    b.wire(weather, "height", out, "height");

    let flat = b.node("slope", 1200.0, 300.0, &[("min", n(0.0)), ("max", n(20.0)), ("falloff", n(6.0))]);
    b.wire(weather, "height", flat, "height");
    b.mask(flat, "mask", "Sand", 1440.0, 300.0);
    let walls = b.node("slope", 1200.0, 420.0, &[("min", n(28.0)), ("max", n(90.0)), ("falloff", n(6.0))]);
    b.wire(weather, "height", walls, "height");
    b.mask(walls, "mask", "Rock", 1440.0, 420.0);

    let mut g = b.finish();
    g.expose("peak_height", "Peak height", out, "peak", 5.0, 1000.0);
    g.expose("dune_height", "Dune height", dunes, "height", 0.0, 100.0);
    g
}

/// The starting point for a graph of your own: Eroded noise into the output.
pub fn empty() -> TerrainGraph {
    let mut b = Builder::new("Terrain Graph");
    let noise = b.node("noise", 0.0, 0.0, &[("scale", n(160.0)), ("height", n(60.0))]);
    let out = b.node("terrain_output", 300.0, 0.0, &[("peak", n(60.0))]);
    b.wire(noise, "height", out, "height");
    let mut g = b.finish();
    g.expose("peak_height", "Peak height", out, "peak", 1.0, 3000.0);
    g
}

/// The Generate tool's noise settings as a graph: one Noise node with the same
/// mode, scale, octaves and shaping, into an Absolute output at the same
/// base and height. The bridge for retiring the Generate panel: whatever it
/// was set to can become a graph and grow from there. (The seed is mixed per
/// node in a graph, so the same seed number is a different, equally random,
/// landscape.)
pub fn from_generate_settings(g: &renzora_terrain::generate::TerrainGenSettings) -> TerrainGraph {
    let mut b = Builder::new("Generated");
    let noise = b.node(
        "noise",
        0.0,
        0.0,
        &[
            ("mode", t(crate::eval::noise_mode_key(g.mode))),
            ("scale", n(g.scale)),
            ("height", n(g.height)),
            ("octaves", n(g.octaves as f32)),
            ("lacunarity", n(g.lacunarity)),
            ("persistence", n(g.persistence)),
            ("warp", n(g.warp_strength)),
            ("exponent", n(g.exponent)),
        ],
    );
    let out = b.node("terrain_output", 300.0, 0.0, &[("mode", t("absolute")), ("offset", n(g.base))]);
    b.wire(noise, "height", out, "height");
    let mut graph = b.finish();
    graph.expose("height", "Height", noise, "height", 0.0, 5000.0);
    graph
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::{evaluate, EvalCache, EvalControl, EvalRequest};

    #[test]
    fn the_generate_tools_settings_become_a_graph() {
        let settings = renzora_terrain::generate::TerrainGenSettings { base: 5.0, height: 40.0, ..Default::default() };
        let g = from_generate_settings(&settings);
        let grid = Grid::fitting(Vec2::new(256.0, 256.0), 33);
        let r = evaluate(&g, &EvalRequest::new(grid, 1), &mut EvalCache::default(), &EvalControl::new()).unwrap();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        let (lo, hi) = r.height.unwrap().min_max();
        assert!(lo >= 5.0 - 1e-3 && hi <= 45.0 + 1e-3, "{lo}..{hi}");
    }
    use crate::field::Grid;
    use bevy::math::Vec2;

    #[test]
    fn every_preset_evaluates_to_sane_ranges() {
        for (id, _, _, build) in PRESETS {
            let g = build();
            // Through the file format too: a preset must survive being written.
            let g = TerrainGraph::parse(&g.to_json()).expect("round trips");
            let grid = Grid::fitting(Vec2::new(512.0, 512.0), 65);
            let r = evaluate(&g, &EvalRequest::new(grid, 1337), &mut EvalCache::default(), &EvalControl::new()).unwrap();
            assert!(r.errors.is_empty(), "{id}: {:?}", r.errors);
            let h = r.height.as_ref().expect("has a terrain output");
            let (lo, hi) = h.min_max();
            let peak = g.exposed_value("peak_height").unwrap();
            assert!(lo >= -0.5 && hi <= peak + 0.5, "{id}: {lo}..{hi} outside 0..{peak}");
            assert!(hi - lo > peak * 0.5, "{id}: only {lo}..{hi} of relief");
            for (name, m) in &r.masks {
                let (a, b) = m.min_max();
                assert!(a >= 0.0 && b <= 1.0, "{id} mask {name}: {a}..{b}");
            }
        }
    }

    #[test]
    fn highlands_has_the_designed_outputs() {
        let g = highlands();
        let grid = Grid::fitting(Vec2::new(1024.0, 1024.0), 65);
        let r = evaluate(&g, &EvalRequest::new(grid, 7), &mut EvalCache::default(), &EvalControl::new()).unwrap();
        for name in ["Gravel", "Dirt", "Meadow grass"] {
            let m = r.mask(name).unwrap_or_else(|| panic!("no {name} mask"));
            assert!(m.data.iter().any(|v| *v > 0.2), "{name} is empty");
        }
        assert_eq!(g.nodes.len(), 14);
    }
}
