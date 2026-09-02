//! Vector nodes — splitting and combining components, the geometric products,
//! and swizzling.

use crate::material::graph::{PinTemplate, PinType, PinValue};

use super::{MaterialNodeDef, CAT_VECTOR, CLR_VECTOR};

pub static SPLIT_VEC2: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/split_vec2",
    display_name: "Split Vec2",
    category: CAT_VECTOR,
    description: "Split Vec2 into components",
    pins: || {
        vec![
            PinTemplate::input("vector", "Vector", PinType::Vec2)
                .with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::output("x", "X", PinType::Float),
            PinTemplate::output("y", "Y", PinType::Float),
        ]
    },
    color: CLR_VECTOR,
};

pub static SPLIT_VEC3: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/split_vec3",
    display_name: "Split Vec3",
    category: CAT_VECTOR,
    description: "Split Vec3 into components",
    pins: || {
        vec![
            PinTemplate::input("vector", "Vector", PinType::Vec3)
                .with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
            PinTemplate::output("x", "X", PinType::Float),
            PinTemplate::output("y", "Y", PinType::Float),
            PinTemplate::output("z", "Z", PinType::Float),
        ]
    },
    color: CLR_VECTOR,
};

pub static COMBINE_VEC2: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/combine_vec2",
    display_name: "Combine Vec2",
    category: CAT_VECTOR,
    description: "Create Vec2 from components",
    pins: || {
        vec![
            PinTemplate::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("vector", "Vector", PinType::Vec2),
        ]
    },
    color: CLR_VECTOR,
};

pub static COMBINE_VEC3: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/combine_vec3",
    display_name: "Combine Vec3",
    category: CAT_VECTOR,
    description: "Create Vec3 from components",
    pins: || {
        vec![
            PinTemplate::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("z", "Z", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("vector", "Vector", PinType::Vec3),
        ]
    },
    color: CLR_VECTOR,
};

pub static COMBINE_VEC4: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/combine_vec4",
    display_name: "Combine Vec4",
    category: CAT_VECTOR,
    description: "Create Vec4 from components",
    pins: || {
        vec![
            PinTemplate::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("z", "Z", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("w", "W", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::output("vector", "Vector", PinType::Vec4),
        ]
    },
    color: CLR_VECTOR,
};

pub static DOT: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/dot",
    display_name: "Dot Product",
    category: CAT_VECTOR,
    description: "Dot product of two vectors",
    pins: || {
        vec![
            PinTemplate::input("a", "A", PinType::Vec3)
                .with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
            PinTemplate::input("b", "B", PinType::Vec3)
                .with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_VECTOR,
};

pub static CROSS: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/cross",
    display_name: "Cross Product",
    category: CAT_VECTOR,
    description: "Cross product of two Vec3",
    pins: || {
        vec![
            PinTemplate::input("a", "A", PinType::Vec3)
                .with_default(PinValue::Vec3([1.0, 0.0, 0.0])),
            PinTemplate::input("b", "B", PinType::Vec3)
                .with_default(PinValue::Vec3([0.0, 1.0, 0.0])),
            PinTemplate::output("result", "Result", PinType::Vec3),
        ]
    },
    color: CLR_VECTOR,
};

pub static NORMALIZE: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/normalize",
    display_name: "Normalize",
    category: CAT_VECTOR,
    description: "Normalize vector to unit length",
    pins: || {
        vec![
            PinTemplate::input("vector", "Vector", PinType::Vec3)
                .with_default(PinValue::Vec3([1.0, 0.0, 0.0])),
            PinTemplate::output("result", "Result", PinType::Vec3),
        ]
    },
    color: CLR_VECTOR,
};

pub static DISTANCE: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/distance",
    display_name: "Distance",
    category: CAT_VECTOR,
    description: "Distance between two points",
    pins: || {
        vec![
            PinTemplate::input("a", "A", PinType::Vec3)
                .with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
            PinTemplate::input("b", "B", PinType::Vec3)
                .with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_VECTOR,
};

pub static LENGTH: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/length",
    display_name: "Length",
    category: CAT_VECTOR,
    description: "Vector magnitude",
    pins: || {
        vec![
            PinTemplate::input("vector", "Vector", PinType::Vec3)
                .with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_VECTOR,
};

pub static REFLECT: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/reflect",
    display_name: "Reflect",
    category: CAT_VECTOR,
    description: "Reflect vector about normal",
    pins: || {
        vec![
            PinTemplate::input("incident", "Incident", PinType::Vec3),
            PinTemplate::input("normal", "Normal", PinType::Vec3),
            PinTemplate::output("result", "Result", PinType::Vec3),
        ]
    },
    color: CLR_VECTOR,
};

pub static REFRACT: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/refract",
    display_name: "Refract",
    category: CAT_VECTOR,
    description: "Refract incident vector through surface with index-of-refraction ratio",
    pins: || {
        vec![
            PinTemplate::input("incident", "Incident", PinType::Vec3),
            PinTemplate::input("normal", "Normal", PinType::Vec3)
                .with_default(PinValue::Vec3([0.0, 1.0, 0.0])),
            PinTemplate::input("eta", "IOR Ratio", PinType::Float)
                .with_default(PinValue::Float(1.0)),
            PinTemplate::output("result", "Result", PinType::Vec3),
        ]
    },
    color: CLR_VECTOR,
};

pub static SWIZZLE: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/swizzle",
    display_name: "Swizzle",
    category: CAT_VECTOR,
    description:
        "Rearrange vec4 components. Pick 0=X, 1=Y, 2=Z, 3=W, 4=zero, 5=one for each output channel.",
    pins: || {
        vec![
            PinTemplate::input("vector", "Vector", PinType::Vec4)
                .with_default(PinValue::Vec4([0.0, 0.0, 0.0, 1.0])),
            PinTemplate::input("out_x", "Out X", PinType::Float).with_default(PinValue::Int(0)),
            PinTemplate::input("out_y", "Out Y", PinType::Float).with_default(PinValue::Int(1)),
            PinTemplate::input("out_z", "Out Z", PinType::Float).with_default(PinValue::Int(2)),
            PinTemplate::input("out_w", "Out W", PinType::Float).with_default(PinValue::Int(3)),
            PinTemplate::output("vector", "Vector", PinType::Vec4),
        ]
    },
    color: CLR_VECTOR,
};
