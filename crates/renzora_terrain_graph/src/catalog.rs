//! Every node a terrain graph can hold: its pins, its parameters, and what it
//! means. The evaluator ([`crate::eval`]) dispatches on [`NodeDef::node_type`];
//! the editor builds its palette, its pins and its parameter fields from this
//! table and nothing else, so a node added here is in the palette with no
//! second registration.
//!
//! # Units
//!
//! Heights are metres. Distances are metres. Angles are degrees. Masks are
//! 0..1. Nothing is in grid cells: a parameter means the same thing whether the
//! graph is evaluated as a 64 cell preview or a 2049 cell build, which is what
//! lets a quarter-resolution preview look like the full build.

use crate::format::ParamValue;

/// What a pin carries. Both are `f32` fields and either connects to either;
/// the kind sets the pin colour and how a preview draws it (a height as
/// shaded relief, a mask as grey).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PinKind {
    /// Metres above the graph's zero.
    Height,
    /// 0..1.
    Mask,
}

#[derive(Debug)]
pub struct PinDef {
    pub name: &'static str,
    pub label: &'static str,
    pub kind: PinKind,
    /// An input with nothing wired reads as this constant, unless `required`.
    pub unconnected: f32,
    /// The node cannot do anything without it: unwired, the node reports an
    /// error and outputs a flat field.
    pub required: bool,
}

const fn input(name: &'static str, label: &'static str, kind: PinKind) -> PinDef {
    PinDef { name, label, kind, unconnected: 0.0, required: true }
}

const fn optional(name: &'static str, label: &'static str, kind: PinKind, unconnected: f32) -> PinDef {
    PinDef { name, label, kind, unconnected, required: false }
}

const fn output(name: &'static str, label: &'static str, kind: PinKind) -> PinDef {
    PinDef { name, label, kind, unconnected: 0.0, required: false }
}

/// How a number reads in the editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    None,
    Metres,
    SquareMetres,
    Degrees,
}

impl Unit {
    pub fn suffix(self) -> &'static str {
        match self {
            Unit::None => "",
            Unit::Metres => "m",
            Unit::SquareMetres => "m²",
            Unit::Degrees => "°",
        }
    }
}

#[derive(Debug)]
pub enum ParamKind {
    Number { min: f32, max: f32, unit: Unit },
    /// A whole number (octaves, passes, a seed). Stored as a JSON number.
    Int { min: f32, max: f32 },
    Bool,
    /// One of a fixed set, stored by its value (the first of each pair); the
    /// second is what the dropdown shows.
    Choice(&'static [(&'static str, &'static str)]),
    /// Free text, such as a mask output's name.
    Text,
    /// A file, project-relative or absolute, with the extensions it takes.
    Path(&'static [&'static str]),
    /// `[x, y]` control points, both 0..1, sorted by x.
    Points,
}

#[derive(Debug)]
pub enum ParamDefault {
    Number(f32),
    Bool(bool),
    Text(&'static str),
    Points(&'static [[f32; 2]]),
}

impl ParamDefault {
    pub fn value(&self) -> ParamValue {
        match self {
            ParamDefault::Number(v) => ParamValue::Number(*v),
            ParamDefault::Bool(b) => ParamValue::Bool(*b),
            ParamDefault::Text(s) => ParamValue::Text((*s).to_string()),
            ParamDefault::Points(p) => ParamValue::Points(p.to_vec()),
        }
    }
}

#[derive(Debug)]
pub struct ParamDef {
    pub name: &'static str,
    pub label: &'static str,
    pub kind: ParamKind,
    pub default: ParamDefault,
    pub help: &'static str,
}

const fn num(name: &'static str, label: &'static str, min: f32, max: f32, unit: Unit, default: f32, help: &'static str) -> ParamDef {
    ParamDef { name, label, kind: ParamKind::Number { min, max, unit }, default: ParamDefault::Number(default), help }
}

const fn int(name: &'static str, label: &'static str, min: f32, max: f32, default: f32, help: &'static str) -> ParamDef {
    ParamDef { name, label, kind: ParamKind::Int { min, max }, default: ParamDefault::Number(default), help }
}

const fn flag(name: &'static str, label: &'static str, default: bool, help: &'static str) -> ParamDef {
    ParamDef { name, label, kind: ParamKind::Bool, default: ParamDefault::Bool(default), help }
}

const fn choice(
    name: &'static str,
    label: &'static str,
    options: &'static [(&'static str, &'static str)],
    default: &'static str,
    help: &'static str,
) -> ParamDef {
    ParamDef { name, label, kind: ParamKind::Choice(options), default: ParamDefault::Text(default), help }
}

const SEED: ParamDef = int(
    "seed",
    "Seed",
    0.0,
    9999.0,
    0.0,
    "Added to the terrain's seed for this node only, so two copies of a node can differ.",
);
const OFFSET_X: ParamDef = num("offset_x", "Offset X", -20000.0, 20000.0, Unit::Metres, 0.0, "East of the terrain's centre.");
const OFFSET_Z: ParamDef = num("offset_z", "Offset Z", -20000.0, 20000.0, Unit::Metres, 0.0, "North of the terrain's centre.");

pub const NOISE_MODES: &[(&str, &str)] = &[
    ("eroded", "Eroded"),
    ("fbm", "FBM"),
    ("ridge", "Ridged"),
    ("billow", "Billow"),
    ("warped", "Warped"),
    ("hybrid", "Hybrid"),
];

pub const COMBINE_MODES: &[(&str, &str)] = &[
    ("add", "Add"),
    ("subtract", "Subtract"),
    ("max", "Max"),
    ("min", "Min"),
    ("multiply", "Multiply"),
    ("blend", "Blend"),
    ("difference", "Difference"),
];

/// One node type.
#[derive(Debug)]
pub struct NodeDef {
    /// The id stored in the file.
    pub node_type: &'static str,
    pub display_name: &'static str,
    /// Palette group: Primitives, Shape, Simulate, Data or Output.
    pub category: &'static str,
    pub description: &'static str,
    pub inputs: &'static [PinDef],
    pub outputs: &'static [PinDef],
    pub params: &'static [ParamDef],
}

impl NodeDef {
    pub fn param(&self, name: &str) -> Option<&'static ParamDef> {
        self.params.iter().find(|p| p.name == name)
    }

    pub fn input(&self, name: &str) -> Option<&'static PinDef> {
        self.inputs.iter().find(|p| p.name == name)
    }

    pub fn output(&self, name: &str) -> Option<&'static PinDef> {
        self.outputs.iter().find(|p| p.name == name)
    }

    /// Whether this node is one of the graph's results rather than a step.
    pub fn is_output(&self) -> bool {
        self.category == "Output"
    }

    /// Simulation nodes cost far more than the rest; progress weighs them so.
    pub fn cost(&self) -> f32 {
        if self.category == "Simulate" {
            12.0
        } else {
            1.0
        }
    }
}

const HEIGHT_OUT: &[PinDef] = &[output("height", "Height", PinKind::Height)];
const MASK_OUT: &[PinDef] = &[output("mask", "Mask", PinKind::Mask)];
const HEIGHT_IN: &[PinDef] = &[input("height", "Height", PinKind::Height)];

/// The catalog, in palette order.
pub static NODES: &[NodeDef] = &[
    // ── Primitives ─────────────────────────────────────────────────────────
    NodeDef {
        node_type: "noise",
        display_name: "Noise",
        category: "Primitives",
        description: "Fractal noise in any of the sculpt and Generate modes. Eroded reads as weathered ground before any erosion runs.",
        inputs: &[],
        outputs: HEIGHT_OUT,
        params: &[
            choice("mode", "Mode", NOISE_MODES, "eroded", "Which noise. Eroded: warped soft-crested ridges with detail that thins on steep ground."),
            num("scale", "Scale", 1.0, 20000.0, Unit::Metres, 120.0, "Size of the largest features."),
            num("height", "Height", 0.0, 5000.0, Unit::Metres, 60.0, "Peak-to-floor relief."),
            int("octaves", "Octaves", 1.0, 10.0, 6.0, "Layers of finer detail."),
            num("lacunarity", "Lacunarity", 1.5, 3.0, Unit::None, 2.0, "How much finer each octave is."),
            num("persistence", "Persistence", 0.1, 0.9, Unit::None, 0.5, "How much weaker each octave is."),
            num("warp", "Warp", 0.0, 4.0, Unit::None, 0.5, "Domain warp, for the Warped and Eroded modes."),
            num("exponent", "Exponent", 0.1, 5.0, Unit::None, 1.0, "Above 1 sharpens peaks and widens valleys; below 1 flattens tops."),
            OFFSET_X,
            OFFSET_Z,
            SEED,
        ],
    },
    NodeDef {
        node_type: "mountain_range",
        display_name: "Mountain Range",
        category: "Primitives",
        description: "Long ridged, warped ranges running in one direction, with spurs and passes.",
        inputs: &[],
        outputs: HEIGHT_OUT,
        params: &[
            num("scale", "Scale", 10.0, 50000.0, Unit::Metres, 800.0, "Distance between ranges."),
            num("height", "Height", 0.0, 5000.0, Unit::Metres, 250.0, "Height of the highest ridges."),
            num("direction", "Direction", 0.0, 360.0, Unit::Degrees, 30.0, "Compass bearing the ranges run along (0 is north)."),
            num("elongation", "Elongation", 1.0, 6.0, Unit::None, 2.0, "How much longer than wide a range is."),
            num("warp", "Warp", 0.0, 4.0, Unit::None, 0.8, "How much the ridgelines wander."),
            int("octaves", "Octaves", 1.0, 10.0, 6.0, "Layers of finer ridges."),
            num("sharpness", "Sharpness", 0.3, 4.0, Unit::None, 1.3, "Above 1 narrows the crests and widens the valleys."),
            SEED,
        ],
    },
    NodeDef {
        node_type: "plateau",
        display_name: "Plateau",
        category: "Primitives",
        description: "A flat-topped mesa with a ragged edge.",
        inputs: &[],
        outputs: HEIGHT_OUT,
        params: &[
            OFFSET_X,
            OFFSET_Z,
            num("radius", "Radius", 1.0, 20000.0, Unit::Metres, 150.0, "Radius of the top."),
            num("height", "Height", 0.0, 5000.0, Unit::Metres, 40.0, "Height of the top."),
            num("edge", "Edge", 0.0, 5000.0, Unit::Metres, 40.0, "Width of the slope down from the top."),
            num("edge_noise", "Edge Noise", 0.0, 1.0, Unit::None, 0.3, "How ragged the outline is."),
            num("noise_scale", "Noise Scale", 1.0, 5000.0, Unit::Metres, 80.0, "Size of the outline's bays and headlands."),
            SEED,
        ],
    },
    NodeDef {
        node_type: "crater",
        display_name: "Crater",
        category: "Primitives",
        description: "An impact crater: a bowl below zero with a raised rim.",
        inputs: &[],
        outputs: HEIGHT_OUT,
        params: &[
            OFFSET_X,
            OFFSET_Z,
            num("radius", "Radius", 1.0, 20000.0, Unit::Metres, 80.0, "Radius to the crest of the rim."),
            num("depth", "Depth", 0.0, 2000.0, Unit::Metres, 25.0, "Depth of the bowl below zero."),
            num("rim_height", "Rim Height", 0.0, 2000.0, Unit::Metres, 10.0, "Height of the rim above zero."),
            num("rim_width", "Rim Width", 1.0, 5000.0, Unit::Metres, 25.0, "How far the rim spreads."),
        ],
    },
    NodeDef {
        node_type: "volcano",
        display_name: "Volcano",
        category: "Primitives",
        description: "A cone with concave flanks and a summit crater.",
        inputs: &[],
        outputs: HEIGHT_OUT,
        params: &[
            OFFSET_X,
            OFFSET_Z,
            num("radius", "Radius", 1.0, 50000.0, Unit::Metres, 300.0, "Radius of the base."),
            num("height", "Height", 0.0, 5000.0, Unit::Metres, 180.0, "Height of the summit."),
            num("crater_radius", "Crater Radius", 0.0, 5000.0, Unit::Metres, 40.0, "Radius of the summit crater."),
            num("crater_depth", "Crater Depth", 0.0, 2000.0, Unit::Metres, 50.0, "Depth of the summit crater."),
            num("slope", "Flank Curve", 0.5, 4.0, Unit::None, 1.6, "Above 1 gives concave flanks that steepen toward the top."),
            num("roughness", "Roughness", 0.0, 1.0, Unit::None, 0.25, "Gullies and lobes on the flanks."),
            SEED,
        ],
    },
    NodeDef {
        node_type: "dunes",
        display_name: "Dunes",
        category: "Primitives",
        description: "Wind-shaped dune ridges: a long gentle windward slope and a short steep lee.",
        inputs: &[],
        outputs: HEIGHT_OUT,
        params: &[
            num("wavelength", "Wavelength", 1.0, 5000.0, Unit::Metres, 60.0, "Crest to crest."),
            num("height", "Height", 0.0, 500.0, Unit::Metres, 8.0, "Height of a crest."),
            num("direction", "Wind", 0.0, 360.0, Unit::Degrees, 0.0, "Bearing the wind blows toward (0 is north)."),
            num("asymmetry", "Asymmetry", 0.5, 0.95, Unit::None, 0.75, "Share of each dune that is windward slope."),
            num("meander", "Meander", 0.0, 2.0, Unit::None, 0.5, "How much the crests wander."),
            SEED,
        ],
    },
    NodeDef {
        node_type: "cells",
        display_name: "Cells",
        category: "Primitives",
        description: "Voronoi cells: pits, cracked walls, or flat steps.",
        inputs: &[],
        outputs: HEIGHT_OUT,
        params: &[
            choice(
                "mode",
                "Mode",
                &[("distance", "Distance"), ("ridges", "Ridges"), ("cells", "Flat Cells")],
                "distance",
                "Distance rises away from each cell centre; Ridges rises along the borders; Flat Cells gives each cell one height.",
            ),
            num("scale", "Scale", 1.0, 20000.0, Unit::Metres, 80.0, "Cell size."),
            num("height", "Height", 0.0, 5000.0, Unit::Metres, 20.0, "Relief."),
            num("jitter", "Jitter", 0.0, 1.0, Unit::None, 0.9, "0 is a square grid, 1 fully random."),
            SEED,
        ],
    },
    NodeDef {
        node_type: "gradient",
        display_name: "Gradient",
        category: "Primitives",
        description: "A ramp across the terrain, as a mask: linear along a bearing, or radial from the centre.",
        inputs: &[],
        outputs: MASK_OUT,
        params: &[
            choice("mode", "Mode", &[("linear", "Linear"), ("radial", "Radial")], "linear", "Linear runs edge to edge; radial runs from the centre out."),
            num("direction", "Direction", 0.0, 360.0, Unit::Degrees, 0.0, "Bearing the ramp rises toward (0 is north)."),
            num("start", "Start", 0.0, 1.0, Unit::None, 0.0, "Value at the low end."),
            num("end", "End", 0.0, 1.0, Unit::None, 1.0, "Value at the high end."),
        ],
    },
    NodeDef {
        node_type: "constant",
        display_name: "Constant",
        category: "Primitives",
        description: "The same height everywhere.",
        inputs: &[],
        outputs: HEIGHT_OUT,
        params: &[num("height", "Height", -5000.0, 5000.0, Unit::Metres, 0.0, "")],
    },
    NodeDef {
        node_type: "import_heightmap",
        display_name: "Import Heightmap",
        category: "Primitives",
        description: "A PNG (8 or 16 bit grey) or RAW16 heightmap stretched over the terrain.",
        inputs: &[],
        outputs: HEIGHT_OUT,
        params: &[
            ParamDef {
                name: "path",
                label: "File",
                kind: ParamKind::Path(&["png", "r16", "raw"]),
                default: ParamDefault::Text(""),
                help: "Project-relative or absolute path.",
            },
            num("height", "Height", 0.0, 5000.0, Unit::Metres, 100.0, "Metres for white."),
            flag("normalize", "Normalize", true, "Stretch the image's own value range to the full height."),
            flag("invert", "Invert", false, "Swap black and white."),
        ],
    },
    NodeDef {
        node_type: "current_terrain",
        display_name: "Current Terrain",
        category: "Primitives",
        description: "The terrain's generator layer as it is now (its last build, or its heights before the first one).",
        inputs: &[],
        outputs: HEIGHT_OUT,
        params: &[],
    },
    // ── Shape ──────────────────────────────────────────────────────────────
    NodeDef {
        node_type: "combine",
        display_name: "Combine",
        category: "Shape",
        description: "Combine two heights. Ratio and the mask fade from A to the combined result.",
        inputs: &[
            optional("a", "A", PinKind::Height, 0.0),
            optional("b", "B", PinKind::Height, 0.0),
            optional("mask", "Mask", PinKind::Mask, 1.0),
        ],
        outputs: HEIGHT_OUT,
        params: &[
            choice("mode", "Mode", COMBINE_MODES, "add", "Blend takes B; the rest combine A and B."),
            num("ratio", "Ratio", 0.0, 1.0, Unit::None, 1.0, "0 keeps A, 1 is the full result."),
        ],
    },
    NodeDef {
        node_type: "curve",
        display_name: "Curve",
        category: "Shape",
        description: "Remap heights through a curve over the input's own range: lift the lowlands, flatten the tops.",
        inputs: HEIGHT_IN,
        outputs: HEIGHT_OUT,
        params: &[ParamDef {
            name: "points",
            label: "Points",
            kind: ParamKind::Points,
            default: ParamDefault::Points(&[[0.0, 0.0], [1.0, 1.0]]),
            help: "Control points, x is the input and y the output, both 0..1 of the input's range.",
        }],
    },
    NodeDef {
        node_type: "terrace",
        display_name: "Terrace",
        category: "Shape",
        description: "Steps: flat treads joined by risers.",
        inputs: HEIGHT_IN,
        outputs: HEIGHT_OUT,
        params: &[
            num("step", "Step Height", 0.1, 1000.0, Unit::Metres, 10.0, "Height of one step."),
            num("sharpness", "Sharpness", 0.0, 1.0, Unit::None, 0.8, "0 is a gentle ramp, 1 a short steep riser."),
            num("offset", "Offset", -1000.0, 1000.0, Unit::Metres, 0.0, "Shifts the steps up or down."),
        ],
    },
    NodeDef {
        node_type: "warp",
        display_name: "Warp",
        category: "Shape",
        description: "Push the input sideways, by two masks or by noise when none is wired.",
        inputs: &[
            input("height", "Height", PinKind::Height),
            optional("warp_x", "Warp X", PinKind::Mask, 0.5),
            optional("warp_z", "Warp Z", PinKind::Mask, 0.5),
        ],
        outputs: HEIGHT_OUT,
        params: &[
            num("strength", "Strength", 0.0, 5000.0, Unit::Metres, 20.0, "Furthest a point moves."),
            num("scale", "Noise Scale", 1.0, 20000.0, Unit::Metres, 120.0, "Size of the warp when it comes from noise."),
            SEED,
        ],
    },
    NodeDef {
        node_type: "blur",
        display_name: "Blur",
        category: "Shape",
        description: "Soften the input.",
        inputs: HEIGHT_IN,
        outputs: HEIGHT_OUT,
        params: &[num("radius", "Radius", 0.0, 2000.0, Unit::Metres, 8.0, "")],
    },
    NodeDef {
        node_type: "sharpen",
        display_name: "Sharpen",
        category: "Shape",
        description: "Exaggerate detail finer than the radius.",
        inputs: HEIGHT_IN,
        outputs: HEIGHT_OUT,
        params: &[
            num("radius", "Radius", 0.5, 2000.0, Unit::Metres, 8.0, "Detail finer than this is boosted."),
            num("amount", "Amount", 0.0, 4.0, Unit::None, 1.0, ""),
        ],
    },
    NodeDef {
        node_type: "transform",
        display_name: "Transform",
        category: "Shape",
        description: "Scale, rotate and move the input across the ground, and scale or lift its heights.",
        inputs: HEIGHT_IN,
        outputs: HEIGHT_OUT,
        params: &[
            num("scale", "Scale", 0.05, 20.0, Unit::None, 1.0, "Above 1 enlarges."),
            num("rotate", "Rotate", -360.0, 360.0, Unit::Degrees, 0.0, "Clockwise, about the centre."),
            OFFSET_X,
            OFFSET_Z,
            num("height_scale", "Height Scale", -10.0, 10.0, Unit::None, 1.0, ""),
            num("height_offset", "Height Offset", -5000.0, 5000.0, Unit::Metres, 0.0, ""),
        ],
    },
    NodeDef {
        node_type: "clamp",
        display_name: "Clamp",
        category: "Shape",
        description: "Cut everything off below Min and above Max.",
        inputs: HEIGHT_IN,
        outputs: HEIGHT_OUT,
        params: &[
            num("min", "Min", -5000.0, 5000.0, Unit::Metres, 0.0, ""),
            num("max", "Max", -5000.0, 5000.0, Unit::Metres, 200.0, ""),
        ],
    },
    NodeDef {
        node_type: "invert",
        display_name: "Invert",
        category: "Shape",
        description: "Turn the input upside down: peaks become pits.",
        inputs: HEIGHT_IN,
        outputs: HEIGHT_OUT,
        params: &[choice(
            "mode",
            "Mode",
            &[("range", "Within Its Range"), ("mask", "Mask (1 - x)")],
            "range",
            "Within its range keeps the lowest and highest heights where they were.",
        )],
    },
    NodeDef {
        node_type: "height_range",
        display_name: "Height Range",
        category: "Shape",
        description: "Stretch the input so its lowest point lands on Min and its highest on Max.",
        inputs: HEIGHT_IN,
        outputs: HEIGHT_OUT,
        params: &[
            num("min", "Min", -5000.0, 5000.0, Unit::Metres, 0.0, ""),
            num("max", "Max", -5000.0, 5000.0, Unit::Metres, 100.0, ""),
        ],
    },
    NodeDef {
        node_type: "flatten",
        display_name: "Flatten",
        category: "Shape",
        description: "Pull the input toward one height where the mask is set: building plots, roads, lake beds.",
        inputs: &[input("height", "Height", PinKind::Height), optional("mask", "Mask", PinKind::Mask, 1.0)],
        outputs: HEIGHT_OUT,
        params: &[
            num("height", "Height", -5000.0, 5000.0, Unit::Metres, 20.0, "The height to flatten to."),
            num("strength", "Strength", 0.0, 1.0, Unit::None, 1.0, ""),
        ],
    },
    // ── Simulate ───────────────────────────────────────────────────────────
    NodeDef {
        node_type: "erosion",
        display_name: "Erosion",
        category: "Simulate",
        description: "Hydraulic erosion (raindrops carve gullies and drop sediment in valleys), then thermal weathering.",
        inputs: &[input("height", "Height", PinKind::Height), optional("mask", "Mask", PinKind::Mask, 1.0)],
        outputs: &[
            output("height", "Height", PinKind::Height),
            output("flow", "Flow", PinKind::Mask),
            output("deposits", "Deposits", PinKind::Mask),
        ],
        params: &[
            num("rain", "Rain", 0.0, 10.0, Unit::None, 1.0, "Raindrops per 3 m cell of ground."),
            num("strength", "Erosion", 0.0, 1.0, Unit::None, 0.3, "How fast running water picks up soil."),
            num("deposition", "Deposition", 0.0, 1.0, Unit::None, 0.3, "How fast it drops soil where it slows."),
            num("capacity", "Capacity", 0.5, 16.0, Unit::None, 4.0, "How much soil fast water carries."),
            num("radius", "Radius", 1.0, 40.0, Unit::Metres, 6.0, "Width a raindrop erodes."),
            int("thermal", "Weathering", 0.0, 200.0, 8.0, "Thermal passes after the rain."),
            num("repose", "Repose", 5.0, 80.0, Unit::Degrees, 38.0, "Slope loose ground rests at."),
            SEED,
        ],
    },
    NodeDef {
        node_type: "thermal",
        display_name: "Thermal",
        category: "Simulate",
        description: "Weathering on its own: slopes steeper than the repose angle shed scree.",
        inputs: HEIGHT_IN,
        outputs: &[output("height", "Height", PinKind::Height), output("talus", "Talus", PinKind::Mask)],
        params: &[
            int("passes", "Passes", 1.0, 1000.0, 30.0, ""),
            num("repose", "Repose", 5.0, 80.0, Unit::Degrees, 35.0, "Slope loose ground rests at."),
        ],
    },
    NodeDef {
        node_type: "rivers",
        display_name: "Rivers",
        category: "Simulate",
        description: "Carve channels where enough water gathers.",
        inputs: HEIGHT_IN,
        outputs: &[output("height", "Height", PinKind::Height), output("water", "Water", PinKind::Mask)],
        params: &[
            num("catchment", "Catchment", 100.0, 1.0e8, Unit::SquareMetres, 20000.0, "Uphill area a point must drain to become a river."),
            num("depth", "Depth", 0.0, 200.0, Unit::Metres, 4.0, "Depth of a full river."),
            num("width", "Width", 1.0, 500.0, Unit::Metres, 10.0, "Width of a full river."),
        ],
    },
    NodeDef {
        node_type: "lakes",
        display_name: "Lakes",
        category: "Simulate",
        description: "Fill every basin to the level it would spill at.",
        inputs: HEIGHT_IN,
        outputs: &[output("height", "Height", PinKind::Height), output("water", "Water", PinKind::Mask)],
        params: &[
            num("min_depth", "Min Depth", 0.0, 100.0, Unit::Metres, 0.5, "Shallower water is ignored."),
            flag("fill", "Fill", false, "Raise the ground to the water level (flat lake beds) instead of leaving it."),
        ],
    },
    NodeDef {
        node_type: "snow",
        display_name: "Snow",
        category: "Simulate",
        description: "Snow that settles on high, gentle ground.",
        inputs: HEIGHT_IN,
        outputs: &[output("height", "Height", PinKind::Height), output("snow", "Snow", PinKind::Mask)],
        params: &[
            num("altitude", "Snow Line", -5000.0, 10000.0, Unit::Metres, 150.0, ""),
            num("blend", "Blend", 0.0, 2000.0, Unit::Metres, 40.0, "Height over which snow fades in."),
            num("max_slope", "Max Slope", 0.0, 90.0, Unit::Degrees, 35.0, "Steeper ground sheds it."),
            num("depth", "Depth", 0.0, 20.0, Unit::Metres, 0.5, "Snow added to the height."),
        ],
    },
    // ── Data ───────────────────────────────────────────────────────────────
    NodeDef {
        node_type: "slope",
        display_name: "Slope",
        category: "Data",
        description: "Mask of ground whose steepness is between Min and Max.",
        inputs: HEIGHT_IN,
        outputs: MASK_OUT,
        params: &[
            num("min", "Min", 0.0, 90.0, Unit::Degrees, 0.0, ""),
            num("max", "Max", 0.0, 90.0, Unit::Degrees, 25.0, ""),
            num("falloff", "Falloff", 0.0, 45.0, Unit::Degrees, 5.0, ""),
        ],
    },
    NodeDef {
        node_type: "height_mask",
        display_name: "Height",
        category: "Data",
        description: "Mask of ground between two heights.",
        inputs: HEIGHT_IN,
        outputs: MASK_OUT,
        params: &[
            num("min", "Min", -5000.0, 10000.0, Unit::Metres, 0.0, ""),
            num("max", "Max", -5000.0, 10000.0, Unit::Metres, 50.0, ""),
            num("falloff", "Falloff", 0.0, 2000.0, Unit::Metres, 10.0, ""),
        ],
    },
    NodeDef {
        node_type: "curvature",
        display_name: "Curvature",
        category: "Data",
        description: "Ridges (convex) or hollows (concave), measured over a radius.",
        inputs: HEIGHT_IN,
        outputs: MASK_OUT,
        params: &[
            choice("mode", "Mode", &[("convex", "Convex"), ("concave", "Concave"), ("both", "Both (grey is flat)")], "convex", ""),
            num("radius", "Radius", 0.5, 1000.0, Unit::Metres, 10.0, "Size of the features measured."),
            num("range", "Range", 0.01, 100.0, Unit::Metres, 2.0, "Curvature that reads as full white."),
        ],
    },
    NodeDef {
        node_type: "flow",
        display_name: "Flow",
        category: "Data",
        description: "Where water runs: the uphill area draining through each point, on a log scale.",
        inputs: HEIGHT_IN,
        outputs: MASK_OUT,
        params: &[
            num("min_area", "Min Area", 1.0, 1.0e8, Unit::SquareMetres, 500.0, "Drainage that reads as black."),
            num("max_area", "Max Area", 1.0, 1.0e9, Unit::SquareMetres, 200000.0, "Drainage that reads as white."),
        ],
    },
    NodeDef {
        node_type: "wetness",
        display_name: "Wetness",
        category: "Data",
        description: "Where ground stays damp: gathered water, hollows and flat ground.",
        inputs: HEIGHT_IN,
        outputs: MASK_OUT,
        params: &[
            num("min_area", "Min Area", 1.0, 1.0e8, Unit::SquareMetres, 500.0, ""),
            num("max_area", "Max Area", 1.0, 1.0e9, Unit::SquareMetres, 200000.0, ""),
        ],
    },
    NodeDef {
        node_type: "aspect",
        display_name: "Aspect",
        category: "Data",
        description: "Mask of slopes facing one way (sunny or shaded sides).",
        inputs: HEIGHT_IN,
        outputs: MASK_OUT,
        params: &[
            num("direction", "Facing", 0.0, 360.0, Unit::Degrees, 180.0, "Bearing the slope faces (180 is south)."),
            num("spread", "Spread", 1.0, 180.0, Unit::Degrees, 90.0, "How far either side still counts."),
        ],
    },
    NodeDef {
        node_type: "select_range",
        display_name: "Select Range",
        category: "Data",
        description: "Mask of a band of the input's own range, 0 its lowest and 1 its highest.",
        inputs: HEIGHT_IN,
        outputs: MASK_OUT,
        params: &[
            num("low", "Low", 0.0, 1.0, Unit::None, 0.25, ""),
            num("high", "High", 0.0, 1.0, Unit::None, 0.75, ""),
            num("falloff", "Falloff", 0.0, 0.5, Unit::None, 0.1, ""),
        ],
    },
    // ── Output ─────────────────────────────────────────────────────────────
    NodeDef {
        node_type: "terrain_output",
        display_name: "Terrain Output",
        category: "Output",
        description: "The terrain's generator layer. Fit stretches the input between Floor and Peak; Absolute takes its metres as they are.",
        inputs: HEIGHT_IN,
        outputs: &[],
        params: &[
            choice("mode", "Range", &[("fit", "Fit Floor to Peak"), ("absolute", "Absolute")], "fit", ""),
            num("floor", "Floor", -5000.0, 10000.0, Unit::Metres, 0.0, "Height of the lowest point (Fit)."),
            num("peak", "Peak", -5000.0, 10000.0, Unit::Metres, 200.0, "Height of the highest point (Fit)."),
            num("offset", "Offset", -5000.0, 5000.0, Unit::Metres, 0.0, "Added to every height (Absolute)."),
        ],
    },
    NodeDef {
        node_type: "mask_output",
        display_name: "Mask Output",
        category: "Output",
        description: "A named mask in the build result, for paint layers and foliage to read.",
        inputs: &[input("mask", "Mask", PinKind::Mask)],
        outputs: &[],
        params: &[ParamDef {
            name: "name",
            label: "Name",
            kind: ParamKind::Text,
            default: ParamDefault::Text("Mask"),
            help: "What the layer or foliage that reads it looks it up by.",
        }],
    },
];

pub fn node_def(node_type: &str) -> Option<&'static NodeDef> {
    NODES.iter().find(|d| d.node_type == node_type)
}

/// Categories in palette order.
pub fn categories() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for def in NODES {
        if !out.contains(&def.category) {
            out.push(def.category);
        }
    }
    out
}

/// A numeric parameter's catalog default.
pub fn default_number(node_type: &str, param: &str) -> Option<f32> {
    match node_def(node_type)?.param(param)?.default {
        ParamDefault::Number(v) => Some(v),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_types_are_unique_and_every_category_known() {
        let mut seen = std::collections::HashSet::new();
        for d in NODES {
            assert!(seen.insert(d.node_type), "duplicate {}", d.node_type);
            assert!(["Primitives", "Shape", "Simulate", "Data", "Output"].contains(&d.category), "{}", d.node_type);
            let mut params = std::collections::HashSet::new();
            for p in d.params {
                assert!(params.insert(p.name), "{}.{} twice", d.node_type, p.name);
                if let (ParamKind::Choice(opts), ParamDefault::Text(v)) = (&p.kind, &p.default) {
                    assert!(opts.iter().any(|(o, _)| o == v), "{}.{} default not an option", d.node_type, p.name);
                }
            }
        }
    }

    #[test]
    fn every_noise_mode_of_the_generate_tool_is_a_noise_node_mode() {
        for mode in renzora_terrain::data::NoiseMode::all() {
            let key = crate::eval::noise_mode_key(*mode);
            assert!(NOISE_MODES.iter().any(|(k, _)| *k == key), "{mode:?}");
        }
    }
}
