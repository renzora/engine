//! Parameter nodes — named graph-boundary inputs.
//!
//! The `name` slot is the parameter identifier material instances use to
//! override the default. The `default` slot is the value baked into the master
//! shader; instances replace it via uniforms.

use crate::material::graph::{PinTemplate, PinType, PinValue};

use super::{MaterialNodeDef, CAT_PARAMETER, CLR_PARAMETER};

pub static PARAM_FLOAT: MaterialNodeDef = MaterialNodeDef {
    node_type: "param/float",
    display_name: "Float Parameter",
    category: CAT_PARAMETER,
    description: "Named float parameter — material instances can override its value.",
    pins: || {
        vec![
            PinTemplate::input("name", "Name", PinType::String)
                .with_default(PinValue::String("FloatParam".to_string())),
            PinTemplate::input("default", "Default", PinType::Float)
                .with_default(PinValue::Float(0.0)),
            PinTemplate::output("value", "Value", PinType::Float),
        ]
    },
    color: CLR_PARAMETER,
};

pub static PARAM_COLOR: MaterialNodeDef = MaterialNodeDef {
    node_type: "param/color",
    display_name: "Color Parameter",
    category: CAT_PARAMETER,
    description: "Named color parameter — material instances can override its value.",
    pins: || {
        vec![
            PinTemplate::input("name", "Name", PinType::String)
                .with_default(PinValue::String("ColorParam".to_string())),
            PinTemplate::input("default", "Default", PinType::Color)
                .with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
            PinTemplate::output("value", "Value", PinType::Color),
        ]
    },
    color: CLR_PARAMETER,
};

pub static PARAM_VEC2: MaterialNodeDef = MaterialNodeDef {
    node_type: "param/vec2",
    display_name: "Vec2 Parameter",
    category: CAT_PARAMETER,
    description: "Named vec2 parameter — material instances can override its value.",
    pins: || {
        vec![
            PinTemplate::input("name", "Name", PinType::String)
                .with_default(PinValue::String("Vec2Param".to_string())),
            PinTemplate::input("default", "Default", PinType::Vec2)
                .with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::output("value", "Value", PinType::Vec2),
        ]
    },
    color: CLR_PARAMETER,
};

pub static PARAM_VEC3: MaterialNodeDef = MaterialNodeDef {
    node_type: "param/vec3",
    display_name: "Vec3 Parameter",
    category: CAT_PARAMETER,
    description: "Named vec3 parameter — material instances can override its value.",
    pins: || {
        vec![
            PinTemplate::input("name", "Name", PinType::String)
                .with_default(PinValue::String("Vec3Param".to_string())),
            PinTemplate::input("default", "Default", PinType::Vec3)
                .with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
            PinTemplate::output("value", "Value", PinType::Vec3),
        ]
    },
    color: CLR_PARAMETER,
};

pub static PARAM_VEC4: MaterialNodeDef = MaterialNodeDef {
    node_type: "param/vec4",
    display_name: "Vec4 Parameter",
    category: CAT_PARAMETER,
    description: "Named vec4 parameter — material instances can override its value.",
    pins: || {
        vec![
            PinTemplate::input("name", "Name", PinType::String)
                .with_default(PinValue::String("Vec4Param".to_string())),
            PinTemplate::input("default", "Default", PinType::Vec4)
                .with_default(PinValue::Vec4([0.0, 0.0, 0.0, 0.0])),
            PinTemplate::output("value", "Value", PinType::Vec4),
        ]
    },
    color: CLR_PARAMETER,
};

pub static PARAM_BOOL: MaterialNodeDef = MaterialNodeDef {
    node_type: "param/bool",
    display_name: "Bool Parameter",
    category: CAT_PARAMETER,
    description: "Named bool parameter — material instances can override its value.",
    pins: || {
        vec![
            PinTemplate::input("name", "Name", PinType::String)
                .with_default(PinValue::String("BoolParam".to_string())),
            PinTemplate::input("default", "Default", PinType::Bool)
                .with_default(PinValue::Bool(false)),
            PinTemplate::output("value", "Value", PinType::Bool),
        ]
    },
    color: CLR_PARAMETER,
};
