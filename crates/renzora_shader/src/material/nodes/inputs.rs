//! Input nodes — what the fragment already knows: UVs (and the transforms that
//! reshape them), world position and normal, view direction, time, vertex color,
//! and the camera / object pivots.

use crate::material::graph::{PinTemplate, PinType, PinValue};

use super::{MaterialNodeDef, CAT_INPUT, CLR_INPUT};

pub static UV: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/uv",
    display_name: "UV",
    category: CAT_INPUT,
    description: "Texture coordinates (0-1)",
    pins: || {
        vec![
            PinTemplate::output("uv", "UV", PinType::Vec2),
            PinTemplate::output("u", "U", PinType::Float),
            PinTemplate::output("v", "V", PinType::Float),
        ]
    },
    color: CLR_INPUT,
};

pub static UV_SCALE: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/uv_scale",
    display_name: "UV Scale",
    category: CAT_INPUT,
    description: "Scale and offset UV coordinates for tiling",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::input("scale", "Scale", PinType::Vec2)
                .with_default(PinValue::Vec2([2.0, 2.0])),
            PinTemplate::input("offset", "Offset", PinType::Vec2)
                .with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::output("uv", "UV", PinType::Vec2),
        ]
    },
    color: CLR_INPUT,
};

pub static UV_POLAR: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/uv_polar",
    display_name: "Polar UV",
    category: CAT_INPUT,
    description: "Convert Cartesian UV to polar (x=angle [0..1], y=radius). Used for radial effects, spirals, pies.",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("center", "Center", PinType::Vec2).with_default(PinValue::Vec2([0.5, 0.5])),
        PinTemplate::output("uv", "Polar UV", PinType::Vec2),
        PinTemplate::output("angle", "Angle", PinType::Float),
        PinTemplate::output("radius", "Radius", PinType::Float),
    ],
    color: CLR_INPUT,
};

pub static UV_ROTATOR: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/uv_rotator",
    display_name: "UV Rotator",
    category: CAT_INPUT,
    description: "Rotate UV coordinates around a center point (angle in radians)",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::input("angle", "Angle", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("center", "Center", PinType::Vec2)
                .with_default(PinValue::Vec2([0.5, 0.5])),
            PinTemplate::output("uv", "UV", PinType::Vec2),
        ]
    },
    color: CLR_INPUT,
};

pub static UV_PANNER: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/uv_panner",
    display_name: "UV Panner",
    category: CAT_INPUT,
    description: "Time-driven UV pan with an arbitrary direction (matches Unreal's Panner node)",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::input("speed", "Speed", PinType::Vec2)
                .with_default(PinValue::Vec2([0.1, 0.0])),
            PinTemplate::input("time_offset", "Time Offset", PinType::Float)
                .with_default(PinValue::Float(0.0)),
            PinTemplate::output("uv", "UV", PinType::Vec2),
        ]
    },
    color: CLR_INPUT,
};

pub static WORLD_POSITION: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/world_position",
    display_name: "World Position",
    category: CAT_INPUT,
    description: "Fragment world-space position",
    pins: || {
        vec![
            PinTemplate::output("position", "Position", PinType::Vec3),
            PinTemplate::output("x", "X", PinType::Float),
            PinTemplate::output("y", "Y", PinType::Float),
            PinTemplate::output("z", "Z", PinType::Float),
        ]
    },
    color: CLR_INPUT,
};

pub static WORLD_NORMAL: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/world_normal",
    display_name: "World Normal",
    category: CAT_INPUT,
    description: "Fragment world-space normal",
    pins: || {
        vec![
            PinTemplate::output("normal", "Normal", PinType::Vec3),
            PinTemplate::output("x", "X", PinType::Float),
            PinTemplate::output("y", "Y", PinType::Float),
            PinTemplate::output("z", "Z", PinType::Float),
        ]
    },
    color: CLR_INPUT,
};

pub static VIEW_DIRECTION: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/view_direction",
    display_name: "View Direction",
    category: CAT_INPUT,
    description: "Direction from fragment to camera",
    pins: || vec![PinTemplate::output("direction", "Direction", PinType::Vec3)],
    color: CLR_INPUT,
};

pub static TIME: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/time",
    display_name: "Time",
    category: CAT_INPUT,
    description: "Time values for animation",
    pins: || {
        vec![
            PinTemplate::output("time", "Time", PinType::Float),
            PinTemplate::output("sin_time", "Sin(Time)", PinType::Float),
            PinTemplate::output("cos_time", "Cos(Time)", PinType::Float),
        ]
    },
    color: CLR_INPUT,
};

pub static VERTEX_COLOR: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/vertex_color",
    display_name: "Vertex Color",
    category: CAT_INPUT,
    description: "Per-vertex color attribute",
    pins: || {
        vec![
            PinTemplate::output("color", "Color", PinType::Color),
            PinTemplate::output("r", "R", PinType::Float),
            PinTemplate::output("g", "G", PinType::Float),
            PinTemplate::output("b", "B", PinType::Float),
            PinTemplate::output("a", "A", PinType::Float),
        ]
    },
    color: CLR_INPUT,
};

pub static CAMERA_POSITION: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/camera_position",
    display_name: "Camera Position",
    category: CAT_INPUT,
    description: "World-space camera position",
    pins: || vec![PinTemplate::output("position", "Position", PinType::Vec3)],
    color: CLR_INPUT,
};

pub static OBJECT_POSITION: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/object_position",
    display_name: "Object Position",
    category: CAT_INPUT,
    description: "Object pivot world position (for wind anchoring, etc.)",
    pins: || vec![PinTemplate::output("position", "Position", PinType::Vec3)],
    color: CLR_INPUT,
};
