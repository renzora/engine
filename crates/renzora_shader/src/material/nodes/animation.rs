//! Animation nodes — everything that moves with time: scrolling and flowing
//! UVs, oscillators, vegetation wind and flipbook frames.

use crate::material::graph::{PinTemplate, PinType, PinValue};

use super::{MaterialNodeDef, CAT_ANIMATION, CLR_ANIMATION};

pub static UV_SCROLL: MaterialNodeDef = MaterialNodeDef {
    node_type: "animation/uv_scroll",
    display_name: "UV Scroll",
    category: CAT_ANIMATION,
    description: "Scroll UV coordinates over time",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::input("speed", "Speed", PinType::Vec2)
                .with_default(PinValue::Vec2([0.1, 0.0])),
            PinTemplate::output("uv", "UV", PinType::Vec2),
        ]
    },
    color: CLR_ANIMATION,
};

pub static FLOW_MAP: MaterialNodeDef = MaterialNodeDef {
    node_type: "animation/flow_map",
    display_name: "Flow Map",
    category: CAT_ANIMATION,
    description: "Two-phase UV distortion with crossfade (realistic water flow)",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::input("flow", "Flow Direction", PinType::Vec2)
                .with_default(PinValue::Vec2([0.1, 0.0])),
            PinTemplate::input("speed", "Speed", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::input("strength", "Strength", PinType::Float)
                .with_default(PinValue::Float(0.1)),
            PinTemplate::output("uv1", "UV Phase 1", PinType::Vec2),
            PinTemplate::output("uv2", "UV Phase 2", PinType::Vec2),
            PinTemplate::output("blend", "Blend", PinType::Float),
        ]
    },
    color: CLR_ANIMATION,
};

pub static SINE_WAVE: MaterialNodeDef = MaterialNodeDef {
    node_type: "animation/sine_wave",
    display_name: "Sine Wave",
    category: CAT_ANIMATION,
    description: "Animated sine oscillation",
    pins: || {
        vec![
            PinTemplate::input("frequency", "Frequency", PinType::Float)
                .with_default(PinValue::Float(1.0)),
            PinTemplate::input("amplitude", "Amplitude", PinType::Float)
                .with_default(PinValue::Float(1.0)),
            PinTemplate::input("offset", "Offset", PinType::Float)
                .with_default(PinValue::Float(0.0)),
            PinTemplate::output("value", "Value", PinType::Float),
        ]
    },
    color: CLR_ANIMATION,
};

pub static PING_PONG: MaterialNodeDef = MaterialNodeDef {
    node_type: "animation/ping_pong",
    display_name: "Ping Pong",
    category: CAT_ANIMATION,
    description: "Triangular wave (0→1→0 repeat)",
    pins: || {
        vec![
            PinTemplate::input("speed", "Speed", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::output("value", "Value", PinType::Float),
        ]
    },
    color: CLR_ANIMATION,
};

pub static WIND: MaterialNodeDef = MaterialNodeDef {
    node_type: "animation/wind",
    display_name: "Wind",
    category: CAT_ANIMATION,
    description: "Wind displacement for vegetation (vertex domain)",
    pins: || {
        vec![
            PinTemplate::input("strength", "Strength", PinType::Float)
                .with_default(PinValue::Float(0.3)),
            PinTemplate::input("speed", "Speed", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::input("direction", "Direction", PinType::Vec2)
                .with_default(PinValue::Vec2([1.0, 0.0])),
            PinTemplate::input("turbulence", "Turbulence", PinType::Float)
                .with_default(PinValue::Float(0.2)),
            PinTemplate::input("mask", "Mask", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::output("displacement", "Displacement", PinType::Vec3),
        ]
    },
    color: CLR_ANIMATION,
};

pub static FLIPBOOK_UV: MaterialNodeDef = MaterialNodeDef {
    node_type: "animation/flipbook_uv",
    display_name: "Flipbook UV",
    category: CAT_ANIMATION,
    description: "Compute the sub-rectangle UV for a single frame of a sprite-sheet/flipbook texture laid out on a `cols × rows` grid. Output feeds a Sample Texture node. Drive `frame` by time*fps for animated sprites, or by an integer to pick a specific tile.",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("frame", "Frame", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("cols", "Columns", PinType::Float).with_default(PinValue::Float(4.0)),
        PinTemplate::input("rows", "Rows", PinType::Float).with_default(PinValue::Float(4.0)),
        PinTemplate::output("uv", "UV", PinType::Vec2),
    ],
    color: CLR_ANIMATION,
};
