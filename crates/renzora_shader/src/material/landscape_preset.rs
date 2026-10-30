//! Landscape materials built in code: the engine's default, Alpine Landscape,
//! and the same graph shape configured from any list of layers.
//!
//! The graph is generated rather than hand-written because it is large (a
//! layer is a dozen nodes: coordinates, four texture reads, a tint, a height
//! remap and its slope and height rules) and because a terrain saved with the
//! old hand-written surface is migrated into one of these, configured from that
//! surface's own layers (`renzora_terrain::landscape::migrate`). One builder
//! serves both, so a migrated terrain is exactly the default graph with other
//! numbers in it.
//!
//! What the graph reproduces of the old surface shader: world-space tiling
//! that runs on across chunks, a tint per layer, height blending between
//! layers (the taller texture wins where two meet), slope and height rules with
//! noise-broken edges, and a slow brightness drift across the land. What it
//! leaves out: the hex anti-tiling and the far-distance second tiling, which
//! cost several texture reads per layer each and would not fit the sixteen
//! image slots a graph has; and a layer marked triplanar projects only its
//! colour from three sides (its normal, roughness and height stay top-down),
//! for the same reason.

use super::graph::{MaterialDomain, MaterialGraph, NodeId, PinValue};
use super::landscape::LandscapeBlend;

/// Where the default landscape material lives in a project, and in the
/// engine's `assets/` (its presets).
pub const ALPINE_LANDSCAPE_PATH: &str = "materials/landscape/alpine_landscape.material";

/// Where the default layer textures live in a project.
///
/// `.rmip`, block compressed with a full mip chain, which the editor bakes
/// from the JPEGs it carries as it writes them (see
/// `renzora_terrain_editor::surface_ui`). The JPEGs themselves loaded with no
/// mips: a 2048 px map tiled every few metres and seen from hundreds of metres
/// away then samples a random texel per pixel, and the ground drew as grey
/// static with every normal pointing somewhere else. Uncompressed they were
/// also 16 MB of video memory each, 268 MB for the set.
pub fn default_texture_path(folder: &str, map: &str) -> String {
    format!("textures/terrain/v3/{folder}/{map}.rmip")
}

/// The texture maps each default layer has, in the order the editor bakes them.
pub const DEFAULT_TEXTURE_MAPS: [&str; 4] = ["albedo", "normal", "arm", "height"];

/// The folders of the default layers.
pub const DEFAULT_TEXTURE_FOLDERS: [&str; 4] = ["grass", "dirt", "rock", "snow"];

/// The default textures as earlier versions named them (`v2`, plain JPEGs),
/// each with the path that replaces it. A material or a surface saved before
/// the switch is pointed at the baked copies through this.
pub fn superseded_default_textures() -> Vec<(String, String)> {
    DEFAULT_TEXTURE_FOLDERS
        .iter()
        .flat_map(|f| {
            DEFAULT_TEXTURE_MAPS.map(|m| (format!("textures/terrain/v2/{f}/{m}.jpg"), default_texture_path(f, m)))
        })
        .collect()
}

/// `path`, or the baked default that replaced it.
pub fn current_default_texture(path: &str) -> String {
    superseded_default_textures()
        .into_iter()
        .find(|(old, _)| old == path)
        .map_or_else(|| path.to_string(), |(_, new)| new)
}

/// A slope or height band a rule paints a layer on within: `min..max`, with a
/// `blend`-wide fade at each edge (degrees for slope, metres for height).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    pub min: f32,
    pub max: f32,
    pub blend: f32,
}

/// One layer of a generated landscape material.
#[derive(Clone, Debug, PartialEq)]
pub struct LandscapeLayerSpec {
    pub name: String,
    /// Project-relative texture paths. An empty colour map draws `tint`
    /// alone; an empty normal map lights the ground flat; an empty packed map
    /// reads as rough, unoccluded ground; an empty height map is flat 0.5.
    pub albedo: String,
    pub normal: String,
    /// R = ambient occlusion, G = roughness (B, metallic, is ignored: ground
    /// is never metal).
    pub arm: String,
    pub height: String,
    /// Multiplies the colour map (linear RGB).
    pub tint: [f32; 3],
    /// Ground one repeat of the textures covers, in metres.
    pub tile_size: f32,
    /// Project the colour map from three sides, for cliff faces.
    pub triplanar: bool,
    /// The height map's mean and standard deviation (0..1), when known.
    /// Height maps come at any exposure, and height blending compares one
    /// layer's height against another's, so each is re-centred on 0.5 with a
    /// common spread or the brighter map wins every edge.
    pub height_stats: Option<(f32, f32)>,
    /// Paint this layer on by slope.
    pub slope: Option<Band>,
    /// Paint this layer on by height.
    pub height_band: Option<Band>,
    /// 0..1: how much noise moves the rule's edges, so a band ends in a ragged
    /// natural line rather than a contour.
    pub breakup: f32,
    pub blend: LandscapeBlend,
}

impl LandscapeLayerSpec {
    fn has_rule(&self) -> bool {
        self.slope.is_some() || self.height_band.is_some()
    }
}

/// A whole generated landscape material.
#[derive(Clone, Debug, PartialEq)]
pub struct LandscapeSpec {
    pub layers: Vec<LandscapeLayerSpec>,
    /// 0..1: how crisp height-blended edges are.
    pub height_blending: f32,
    /// Multiplies how far the normal maps tilt the surface.
    pub normal_strength: f32,
    /// 0..1: brightness that drifts across the land at `macro_size` metres.
    pub macro_variation: f32,
    pub macro_size: f32,
}

/// The engine's default ground: grass, gravel on moderate slopes, rock on
/// cliffs and snow on high gentle ground. The numbers are the old default
/// surface's, so a terrain looks the same after the switch.
pub fn alpine_spec() -> LandscapeSpec {
    let layer = |name: &str, folder: &str, tint: [f32; 3], tile: f32, stats: (f32, f32)| LandscapeLayerSpec {
        name: name.into(),
        albedo: default_texture_path(folder, "albedo"),
        normal: default_texture_path(folder, "normal"),
        arm: default_texture_path(folder, "arm"),
        height: default_texture_path(folder, "height"),
        tint,
        tile_size: tile,
        triplanar: false,
        height_stats: Some(stats),
        slope: None,
        height_band: None,
        breakup: 0.0,
        blend: LandscapeBlend::Height,
    };
    // Height statistics measured from the shipped maps (`assets/terrain/`).
    LandscapeSpec {
        layers: vec![
            // The tint pulls the photo's mix of green blades and red-brown
            // leaves toward a temperate summer green.
            layer("Grass", "grass", [0.36, 0.64, 0.32], 3.0, (0.370, 0.065)),
            LandscapeLayerSpec {
                slope: Some(Band { min: 24.0, max: 40.0, blend: 5.0 }),
                breakup: 0.7,
                ..layer("Gravel", "dirt", [1.0, 1.0, 1.0], 2.5, (0.472, 0.102))
            },
            LandscapeLayerSpec {
                slope: Some(Band { min: 34.0, max: 90.0, blend: 4.0 }),
                breakup: 0.5,
                triplanar: true,
                ..layer("Rock", "rock", [1.0, 1.0, 1.0], 8.0, (0.638, 0.092))
            },
            LandscapeLayerSpec {
                slope: Some(Band { min: 0.0, max: 40.0, blend: 6.0 }),
                height_band: Some(Band { min: 190.0, max: 100_000.0, blend: 20.0 }),
                breakup: 0.8,
                blend: LandscapeBlend::Weight,
                ..layer("Snow", "snow", [1.0, 1.0, 1.0], 5.0, (0.559, 0.098))
            },
        ],
        height_blending: 0.8,
        normal_strength: 1.15,
        macro_variation: 0.4,
        macro_size: 120.0,
    }
}

/// The default landscape material's graph.
pub fn alpine_landscape_graph() -> MaterialGraph {
    build_landscape_graph("Alpine Landscape", &alpine_spec())
}

/// Every texture path a spec names, without repeats.
pub fn spec_textures(spec: &LandscapeSpec) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for l in &spec.layers {
        for p in [&l.albedo, &l.normal, &l.arm, &l.height] {
            if !p.is_empty() && !out.contains(p) {
                out.push(p.clone());
            }
        }
    }
    out
}

/// The layout grid, in canvas px. A node is 160 wide; a texture read with its
/// preview about 160 tall, a normal-map read about 180, a Layer Blend about
/// 770 with all eight slots. The first version spaced nodes 60 to 120 apart,
/// smaller than the nodes themselves, so every layer's texture reads, math and
/// rules lay on top of one another.
const COL: f32 = 220.0;
/// Between one texture read and the next, in a layer's column.
const TEX_PITCH: f32 = 200.0;
/// Between small nodes stacked in one column (a rule's two smoothsteps).
const ROW_PITCH: f32 = 120.0;
/// One layer's band: four texture reads and a gap.
const LAYER_H: f32 = 4.0 * TEX_PITCH + 60.0;
/// Where the rule chain starts, and where the blends and the output sit.
const RULE_X: f32 = 3.0 * COL;
const BLEND_X: f32 = 9.0 * COL + 120.0;
/// Between the three Layer Blends.
const BLEND_PITCH: f32 = 900.0;

/// Small helpers over a graph under construction.
struct Builder {
    g: MaterialGraph,
}

impl Builder {
    fn node(&mut self, ty: &str, x: f32, y: f32) -> NodeId {
        self.g.add_node(ty, [x, y])
    }
    fn set(&mut self, id: NodeId, pin: &str, v: PinValue) {
        if let Some(n) = self.g.get_node_mut(id) {
            n.input_values.insert(pin.to_string(), v);
        }
    }
    fn float(&mut self, id: NodeId, pin: &str, v: f32) {
        self.set(id, pin, PinValue::Float(v));
    }
    fn wire(&mut self, from: NodeId, from_pin: &str, to: NodeId, to_pin: &str) {
        self.g.connect(from, from_pin, to, to_pin);
    }
    fn texture(&mut self, ty: &str, path: &str, x: f32, y: f32) -> NodeId {
        let n = self.node(ty, x, y);
        self.set(n, "texture", PinValue::TexturePath(path.to_string()));
        n
    }

    /// `smoothstep(lo - b, lo + b, x) * (1 - smoothstep(hi - b, hi + b, x))`,
    /// leaving out an edge that is outside the range `x` can reach.
    fn band(&mut self, x: (NodeId, &str), band: Band, open_low: f32, open_high: f32, px: f32, py: f32) -> (NodeId, &'static str) {
        let b = band.blend.max(0.0001);
        let mut parts: Vec<(NodeId, &'static str)> = Vec::new();
        if band.min > open_low {
            let s = self.node("math/smoothstep", px, py);
            self.float(s, "edge0", band.min - b);
            self.float(s, "edge1", band.min + b);
            self.wire(x.0, x.1, s, "value");
            parts.push((s, "result"));
        }
        if band.max < open_high {
            let s = self.node("math/smoothstep", px, py + ROW_PITCH);
            self.float(s, "edge0", band.max - b);
            self.float(s, "edge1", band.max + b);
            self.wire(x.0, x.1, s, "value");
            let inv = self.node("math/one_minus", px + COL, py + ROW_PITCH);
            self.wire(s, "result", inv, "value");
            parts.push((inv, "result"));
        }
        match parts.len() {
            0 => {
                let one = self.node("color/float", px, py);
                self.float(one, "value", 1.0);
                (one, "value")
            }
            1 => parts[0],
            _ => {
                let m = self.node("math/multiply", px + 2.0 * COL, py + ROW_PITCH / 2.0);
                self.wire(parts[0].0, parts[0].1, m, "a");
                self.wire(parts[1].0, parts[1].1, m, "b");
                (m, "result")
            }
        }
    }

    /// `value + jitter * scale`: a rule's input moved by the breakup noise.
    fn jittered(&mut self, value: (NodeId, &'static str), jitter: (NodeId, &'static str), scale: f32, px: f32, py: f32) -> (NodeId, &'static str) {
        if scale == 0.0 {
            return value;
        }
        let m = self.node("math/multiply", px, py);
        self.wire(jitter.0, jitter.1, m, "a");
        self.float(m, "b", scale);
        let a = self.node("math/add", px + COL, py);
        self.wire(value.0, value.1, a, "a");
        self.wire(m, "result", a, "b");
        (a, "result")
    }
}

/// Build a landscape material from `spec`: one Layer Blend for colour, one
/// for normals and one for the packed AO / roughness map, all three fed the
/// same per-layer heights and rules, so the layers always agree.
pub fn build_landscape_graph(name: &str, spec: &LandscapeSpec) -> MaterialGraph {
    let mut b = Builder { g: MaterialGraph::new(name, MaterialDomain::Surface) };
    let out = b.g.output_node().map(|n| n.id).unwrap_or(1);
    // Laid out left to right in the order the data flows: each layer is one
    // horizontal band (coordinates, its four texture reads, tint and height
    // remap, then its slope and height rule), the shared rule inputs sit above
    // the bands, and the three Layer Blends stand in a column to the right
    // with the output beyond them.
    if let Some(n) = b.g.get_node_mut(out) {
        n.position = [BLEND_X + 3.0 * COL + 100.0, 0.0];
    }

    let colour = b.node("landscape/layer_blend", BLEND_X, 0.0);
    let normal = b.node("landscape/layer_blend", BLEND_X, BLEND_PITCH);
    let packed = b.node("landscape/layer_blend", BLEND_X, 2.0 * BLEND_PITCH);
    for blend in [colour, normal, packed] {
        b.float(blend, "height_blending", spec.height_blending.clamp(0.0, 1.0));
    }

    // Shared by every rule: the slope and height, and the breakup noise
    // (`fbm(p * 0.08) - 0.5`, the old surface's).
    let any_rule = spec.layers.iter().any(LandscapeLayerSpec::has_rule);
    let (slope, jitter) = if any_rule {
        let coords = b.node("landscape/layer_coords", 0.0, -380.0);
        b.float(coords, "scale", 1.0 / 0.08);
        let fbm = b.node("procedural/noise_fbm", COL, -380.0);
        b.float(fbm, "scale", 1.0);
        b.wire(coords, "uv", fbm, "uv");
        let centred = b.node("math/subtract", 2.0 * COL, -380.0);
        let slope = b.node("landscape/slope", RULE_X, -380.0);
        b.wire(fbm, "value", centred, "a");
        b.float(centred, "b", 0.5);
        (Some(slope), Some((centred, "result")))
    } else {
        (None, None)
    };

    for (slot, layer) in spec.layers.iter().take(super::nodes::LANDSCAPE_MAX_LAYERS).enumerate() {
        let y = slot as f32 * LAYER_H;
        let tile = layer.tile_size.max(0.01);
        // Albedo, normal, packed and height reads, one under the other.
        let [ya, yn, yp, yh] = [0.0, 1.0, 2.0, 3.0].map(|i| y + i * TEX_PITCH);

        let coords = b.node("landscape/layer_coords", 0.0, y + 1.5 * TEX_PITCH);
        b.float(coords, "scale", tile);

        // Colour, tinted.
        let tint = PinValue::Color([layer.tint[0], layer.tint[1], layer.tint[2], 1.0]);
        let colour_value: (NodeId, &str) = if layer.albedo.is_empty() {
            let c = b.node("color/constant", COL, ya);
            b.set(c, "color", tint.clone());
            (c, "color")
        } else {
            let sample = if layer.triplanar {
                let t = b.texture("texture/triplanar", &layer.albedo, COL, ya);
                b.float(t, "scale", 1.0 / tile);
                b.float(t, "sharpness", 4.0);
                t
            } else {
                let t = b.texture("texture/sample", &layer.albedo, COL, ya);
                b.wire(coords, "uv", t, "uv");
                t
            };
            let tinted = b.node("math/multiply", 2.0 * COL, ya);
            b.wire(sample, "color", tinted, "a");
            b.set(tinted, "b", tint.clone());
            (tinted, "result")
        };

        // Normal.
        let normal_value: (NodeId, &str) = if layer.normal.is_empty() {
            let n = b.node("input/world_normal", COL, yn);
            (n, "normal")
        } else {
            let n = b.texture("texture/sample_normal", &layer.normal, COL, yn);
            b.wire(coords, "uv", n, "uv");
            b.float(n, "strength", spec.normal_strength.max(0.0));
            (n, "normal")
        };

        // Packed AO / roughness.
        let packed_value: (NodeId, &str) = if layer.arm.is_empty() {
            let c = b.node("color/constant", COL, yp);
            b.set(c, "color", PinValue::Color([1.0, 0.9, 0.0, 1.0]));
            (c, "color")
        } else {
            let t = b.texture("texture/sample", &layer.arm, COL, yp);
            b.wire(coords, "uv", t, "uv");
            (t, "color")
        };

        // Height, re-centred.
        let height_value: Option<(NodeId, &str)> = if layer.height.is_empty() {
            None
        } else {
            let t = b.texture("texture/sample", &layer.height, COL, yh);
            b.wire(coords, "uv", t, "uv");
            match layer.height_stats {
                Some((mean, spread)) if spread > 0.0001 => {
                    // 0.5 + (h - mean) * 0.18 / spread, as a remap.
                    let k = 0.5 * spread / 0.18;
                    let r = b.node("math/remap", 2.0 * COL, yh);
                    b.wire(t, "r", r, "value");
                    b.float(r, "in_min", mean - k);
                    b.float(r, "in_max", mean + k);
                    b.float(r, "out_min", 0.0);
                    b.float(r, "out_max", 1.0);
                    Some((r, "result"))
                }
                _ => Some((t, "r")),
            }
        };

        // The rule: slope band times height band, edges moved by the noise.
        let rule: Option<(NodeId, &str)> = match (slope, jitter) {
            (Some(slope), Some(jitter)) if layer.has_rule() => {
                let mut parts = Vec::new();
                // Slope rule in the band's top half, height rule below it.
                let (ys, yb) = (y + 0.5 * TEX_PITCH, y + 2.0 * TEX_PITCH);
                if let Some(band) = layer.slope {
                    let x = b.jittered((slope, "degrees"), jitter, layer.breakup * 2.0 * band.blend.max(1.0), RULE_X, ys);
                    parts.push(b.band(x, band, 0.0, 90.0, RULE_X + 2.0 * COL, ys));
                }
                if let Some(band) = layer.height_band {
                    let x = b.jittered((slope, "height"), jitter, layer.breakup * 2.0 * band.blend.max(0.5), RULE_X, yb);
                    // The old surface's "no limit" is plus or minus 100 km.
                    parts.push(b.band(x, band, -99_999.0, 99_999.0, RULE_X + 2.0 * COL, yb));
                }
                if parts.len() == 2 {
                    let m = b.node("math/multiply", RULE_X + 5.0 * COL, (ys + yb) / 2.0);
                    b.wire(parts[0].0, parts[0].1, m, "a");
                    b.wire(parts[1].0, parts[1].1, m, "b");
                    Some((m, "result"))
                } else {
                    parts.first().copied()
                }
            }
            _ => None,
        };

        let blend_name = PinValue::String(layer.blend.display_name().to_string());
        for (blend, value) in [(colour, colour_value), (normal, normal_value), (packed, packed_value)] {
            b.set(blend, &format!("name_{slot}"), PinValue::String(layer.name.clone()));
            b.set(blend, &format!("blend_{slot}"), blend_name.clone());
            b.wire(value.0, value.1, blend, &format!("layer_{slot}"));
            if let Some(h) = height_value {
                b.wire(h.0, h.1, blend, &format!("height_{slot}"));
            }
            if let Some(r) = rule {
                b.wire(r.0, r.1, blend, &format!("auto_{slot}"));
            }
        }
    }

    // A slow brightness drift across the land, so a hillside is not one
    // repeated swatch: `1 + (fbm - 0.5) * 1.4 * strength`.
    let colour_out: (NodeId, &str) = if spec.macro_variation > 0.0 {
        let coords = b.node("landscape/layer_coords", BLEND_X, -380.0);
        b.float(coords, "scale", spec.macro_size.max(1.0));
        let fbm = b.node("procedural/noise_fbm", BLEND_X + COL, -380.0);
        b.float(fbm, "scale", 1.0);
        b.wire(coords, "uv", fbm, "uv");
        let drift = b.node("math/remap", BLEND_X + 2.0 * COL, -380.0);
        b.wire(fbm, "value", drift, "value");
        b.float(drift, "in_min", 0.0);
        b.float(drift, "in_max", 1.0);
        b.float(drift, "out_min", 1.0 - 0.7 * spec.macro_variation);
        b.float(drift, "out_max", 1.0 + 0.7 * spec.macro_variation);
        let m = b.node("math/multiply", BLEND_X + 2.0 * COL, 0.0);
        b.wire(colour, "result", m, "a");
        b.wire(drift, "result", m, "b");
        (m, "result")
    } else {
        (colour, "result")
    };

    let split = b.node("vector/split_vec3", BLEND_X + 2.0 * COL, 2.0 * BLEND_PITCH);
    b.wire(packed, "rgb", split, "vector");
    b.wire(colour_out.0, colour_out.1, out, "base_color");
    b.wire(normal, "normal", out, "normal");
    b.wire(split, "x", out, "ao");
    b.wire(split, "y", out, "roughness");

    // Frames naming each part, added last so the nodes keep the ids they had.
    let layers = spec.layers.len().min(super::nodes::LANDSCAPE_MAX_LAYERS);
    let mut frame = |rect: [f32; 4], text: String, color: [u8; 3]| {
        let id = b.g.add_comment(rect);
        if let Some(c) = b.g.comments.iter_mut().find(|c| c.id == id) {
            c.text = text;
            c.color = color;
        }
    };
    let band_w = RULE_X + 6.0 * COL + 80.0;
    if any_rule {
        frame([-40.0, -440.0, band_w, 260.0], "Rule inputs: breakup noise, slope and height".into(), [110, 96, 150]);
    }
    for (slot, layer) in spec.layers.iter().take(layers).enumerate() {
        let y = slot as f32 * LAYER_H;
        let colour = [
            (layer.tint[0] * 160.0) as u8,
            (layer.tint[1] * 160.0) as u8,
            (layer.tint[2] * 160.0) as u8,
        ];
        frame([-40.0, y - 50.0, band_w, LAYER_H - 30.0], format!("{}: textures, then its rule", layer.name), colour);
    }
    if spec.macro_variation > 0.0 {
        frame([BLEND_X - 40.0, -440.0, 3.0 * COL + 60.0, 260.0], "Macro variation".into(), [120, 120, 90]);
    }
    // Wide enough for a Layer Blend, short of the multiply and split beside it.
    frame([BLEND_X - 40.0, -60.0, COL + 200.0, 3.0 * BLEND_PITCH - 40.0], "Colour, normal and AO / roughness blends".into(), [70, 120, 90]);
    b.g
}

#[cfg(test)]
mod tests {
    use super::super::codegen;
    use super::super::landscape::landscape_layers;
    use super::*;

    #[test]
    fn the_alpine_landscape_compiles_within_the_texture_slots() {
        let graph = alpine_landscape_graph();
        let result = codegen::compile(&graph);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.warnings.is_empty(), "{:?}", result.warnings);
        assert_eq!(result.landscape_layers, ["Grass", "Gravel", "Rock", "Snow"]);
        // Four maps for each of four layers, less snow's height map: snow
        // blends by weight, so its height is never read and never bound.
        let paths: Vec<&str> = result.texture_bindings.iter().map(|t| t.asset_path.as_str()).collect();
        assert_eq!(paths.len(), 15, "{paths:?}");
        assert!(!paths.contains(&default_texture_path("snow", "height").as_str()));
        let s = &result.fragment_shader;
        assert!(!s.contains("Unknown node type"), "{s}");
        assert!(codegen::supports_deferred(s), "{s}");
        assert!(codegen::supports_virtual_geometry(s), "{s}");
        // Never the trivial StandardMaterial path: it would drop the blend.
        assert!(graph.connections.iter().any(|c| c.to_pin == "uv"));
    }

    #[test]
    fn a_spec_without_textures_or_rules_still_compiles() {
        let spec = LandscapeSpec {
            layers: vec![
                LandscapeLayerSpec {
                    name: "Sand".into(),
                    albedo: String::new(),
                    normal: String::new(),
                    arm: String::new(),
                    height: String::new(),
                    tint: [0.8, 0.7, 0.5],
                    tile_size: 4.0,
                    triplanar: false,
                    height_stats: None,
                    slope: None,
                    height_band: None,
                    breakup: 0.0,
                    blend: LandscapeBlend::Weight,
                },
                LandscapeLayerSpec { name: "Mud".into(), tint: [0.2, 0.15, 0.1], ..alpine_spec().layers[1].clone() },
            ],
            macro_variation: 0.0,
            ..alpine_spec()
        };
        let graph = build_landscape_graph("Plain", &spec);
        let result = codegen::compile(&graph);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(landscape_layers(&graph), ["Sand", "Mud"]);
    }

    /// The preset the engine ships in `assets/` is this builder's output, so
    /// the file and the code never drift apart. Regenerate it with
    /// `cargo test --profile dist -p renzora_shader write_alpine_landscape_preset -- --ignored`.
    #[test]
    fn the_shipped_preset_is_the_builder_output() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join(ALPINE_LANDSCAPE_PATH);
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let mut shipped: MaterialGraph = serde_json::from_str(&text).expect("the preset parses");
        shipped.compiled = None;
        assert_eq!(shipped, alpine_landscape_graph());
    }

    /// No two nodes of the default graph overlap, measured the way the graph
    /// view lays a node out: a 24 px header, an 18 px row per pin (inputs and
    /// outputs share rows), and on a texture read a preview beside its
    /// outputs. The first layout spaced nodes closer than their own size, and
    /// the graph opened as one pile per layer.
    #[test]
    fn the_default_graph_lays_out_without_overlaps() {
        use super::super::graph::PinDir;
        let graph = alpine_landscape_graph();
        let rect = |n: &super::super::graph::MaterialNode| {
            let pins = super::super::nodes::node_def(&n.node_type).map(|d| (d.pins)()).unwrap_or_default();
            let inputs = pins.iter().filter(|p| p.direction == PinDir::Input).count() as f32;
            let outputs = pins.iter().filter(|p| p.direction == PinDir::Output).count() as f32;
            let body = if n.node_type.starts_with("texture/") {
                inputs * 18.0 + (outputs * 18.0).max(100.0)
            } else {
                inputs.max(outputs) * 18.0
            };
            // The output node is wider than the rest; a little slack on all.
            let w = if n.node_type.contains("layer_blend") || n.node_type == "output/surface" { 340.0 } else { 170.0 };
            (n.position[0], n.position[1], w, 30.0 + body)
        };
        let rects: Vec<_> = graph.nodes.iter().map(|n| (n.node_type.clone(), n.position, rect(n))).collect();
        for (i, (ta, pa, a)) in rects.iter().enumerate() {
            for (tb, pb, b) in rects.iter().skip(i + 1) {
                let apart = a.0 + a.2 <= b.0 || b.0 + b.2 <= a.0 || a.1 + a.3 <= b.1 || b.1 + b.3 <= a.1;
                assert!(apart, "{ta} at {pa:?} overlaps {tb} at {pb:?}");
            }
        }
    }

    #[test]
    fn an_old_default_texture_path_maps_to_its_baked_copy() {
        assert_eq!(current_default_texture("textures/terrain/v2/rock/normal.jpg"), default_texture_path("rock", "normal"));
        assert_eq!(current_default_texture("textures/mine/rock.png"), "textures/mine/rock.png");
        assert_eq!(superseded_default_textures().len(), 16);
    }

    #[test]
    #[ignore = "writes the shipped preset; run by hand after changing the builder"]
    fn write_alpine_landscape_preset() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join(ALPINE_LANDSCAPE_PATH);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        // With the compiled shader embedded, as an editor save writes it, so
        // a game shipping the preset never compiles it at load.
        let mut graph = alpine_landscape_graph();
        let (json, report) = super::super::precompiled::save_compiled_and_serialize(&mut graph, &path).unwrap();
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        std::fs::write(&path, json).unwrap();
    }
}
