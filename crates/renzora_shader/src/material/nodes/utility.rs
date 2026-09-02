//! Utility nodes — masks derived from world position and slope, the screen-space
//! derivatives, and the dither / hash helpers.

use crate::material::graph::{PinTemplate, PinType, PinValue};

use super::{MaterialNodeDef, CAT_UTILITY, CLR_UTILITY};

pub static WORLD_POSITION_MASK: MaterialNodeDef = MaterialNodeDef {
    node_type: "utility/world_pos_mask",
    display_name: "World Position Mask",
    category: CAT_UTILITY,
    description: "Mask by world Y height (snow on peaks, etc.)",
    pins: || {
        vec![
            PinTemplate::input("height", "Height", PinType::Float)
                .with_default(PinValue::Float(10.0)),
            PinTemplate::input("falloff", "Falloff", PinType::Float)
                .with_default(PinValue::Float(2.0)),
            PinTemplate::output("mask", "Mask", PinType::Float),
        ]
    },
    color: CLR_UTILITY,
};

pub static SLOPE_MASK: MaterialNodeDef = MaterialNodeDef {
    node_type: "utility/slope_mask",
    display_name: "Slope Mask",
    category: CAT_UTILITY,
    description: "Mask by surface slope angle (cliffs vs flat ground)",
    pins: || {
        vec![
            PinTemplate::input("threshold", "Threshold", PinType::Float)
                .with_default(PinValue::Float(0.5)),
            PinTemplate::input("falloff", "Falloff", PinType::Float)
                .with_default(PinValue::Float(0.2)),
            PinTemplate::output("mask", "Mask", PinType::Float),
        ]
    },
    color: CLR_UTILITY,
};

pub static DEPTH_FADE: MaterialNodeDef = MaterialNodeDef {
    node_type: "utility/depth_fade",
    display_name: "Depth Fade",
    category: CAT_UTILITY,
    description: "Fade based on scene depth difference (water shore foam)",
    pins: || {
        vec![
            PinTemplate::input("distance", "Distance", PinType::Float)
                .with_default(PinValue::Float(1.0)),
            PinTemplate::output("fade", "Fade", PinType::Float),
        ]
    },
    color: CLR_UTILITY,
};

pub static DPDX: MaterialNodeDef = MaterialNodeDef {
    node_type: "utility/dpdx",
    display_name: "DDX",
    category: CAT_UTILITY,
    description: "Screen-space derivative along X (rate of change horizontally)",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_UTILITY,
};

pub static DPDY: MaterialNodeDef = MaterialNodeDef {
    node_type: "utility/dpdy",
    display_name: "DDY",
    category: CAT_UTILITY,
    description: "Screen-space derivative along Y (rate of change vertically)",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_UTILITY,
};

pub static FWIDTH: MaterialNodeDef = MaterialNodeDef {
    node_type: "utility/fwidth",
    display_name: "FWidth",
    category: CAT_UTILITY,
    description: "abs(dpdx) + abs(dpdy) — pixel footprint for anti-aliasing",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_UTILITY,
};

pub static DITHER: MaterialNodeDef = MaterialNodeDef {
    node_type: "utility/dither",
    display_name: "Dither",
    category: CAT_UTILITY,
    description: "Screen-space Bayer dither (4x4) for transparency-to-coverage",
    pins: || vec![PinTemplate::output("value", "Value", PinType::Float)],
    color: CLR_UTILITY,
};

pub static HASH: MaterialNodeDef = MaterialNodeDef {
    node_type: "utility/hash",
    display_name: "Hash",
    category: CAT_UTILITY,
    description: "Deterministic 0-1 hash of a vec2 input (white-noise style)",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Vec2)
                .with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_UTILITY,
};
