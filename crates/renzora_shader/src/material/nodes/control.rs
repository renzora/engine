//! Control nodes — the two branches (runtime `If`, compile-time `Static
//! Switch`), channel masking, and the comparison / boolean operators.

use crate::material::graph::{PinTemplate, PinType, PinValue};

use super::{MaterialNodeDef, CAT_CONTROL, CLR_CONTROL};

pub static IF_NODE: MaterialNodeDef = MaterialNodeDef {
    node_type: "control/if",
    display_name: "If",
    category: CAT_CONTROL,
    description: "Runtime branch: when `condition > threshold`, outputs `if_true`, else `if_false`. Both branches always execute (use Static Switch for permutation-style branching).",
    pins: || vec![
        PinTemplate::input("condition", "Condition", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("threshold", "Threshold", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::input("if_true",  "True",  PinType::Vec4).with_default(PinValue::Vec4([1.0, 1.0, 1.0, 1.0])),
        PinTemplate::input("if_false", "False", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 1.0])),
        PinTemplate::output("result", "Result", PinType::Vec4),
    ],
    color: CLR_CONTROL,
};

pub static STATIC_SWITCH: MaterialNodeDef = MaterialNodeDef {
    node_type: "control/static_switch",
    display_name: "Static Switch",
    category: CAT_CONTROL,
    description: "Compile-time branch. Only the selected input's nodes are emitted in the shader — the unused branch is stripped. Set `use_a` (Bool) in the node's input_values.",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Vec4).with_default(PinValue::Vec4([1.0, 1.0, 1.0, 1.0])),
        PinTemplate::input("b", "B", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 1.0])),
        PinTemplate::input("use_a", "Use A", PinType::Bool).with_default(PinValue::Bool(true)),
        PinTemplate::output("result", "Result", PinType::Vec4),
    ],
    color: CLR_CONTROL,
};

pub static COMPONENT_MASK: MaterialNodeDef = MaterialNodeDef {
    node_type: "control/component_mask",
    display_name: "Component Mask",
    category: CAT_CONTROL,
    description: "Zero specific channels of a vec4. Toggle R/G/B/A booleans to keep or drop each channel. Equivalent to Unreal's ComponentMask.",
    pins: || vec![
        PinTemplate::input("vector", "Vector", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 1.0])),
        PinTemplate::input("keep_r", "R", PinType::Bool).with_default(PinValue::Bool(true)),
        PinTemplate::input("keep_g", "G", PinType::Bool).with_default(PinValue::Bool(true)),
        PinTemplate::input("keep_b", "B", PinType::Bool).with_default(PinValue::Bool(true)),
        PinTemplate::input("keep_a", "A", PinType::Bool).with_default(PinValue::Bool(false)),
        PinTemplate::output("vector", "Vector", PinType::Vec4),
    ],
    color: CLR_CONTROL,
};

pub static GREATER_THAN: MaterialNodeDef = MaterialNodeDef {
    node_type: "control/greater_than",
    display_name: "Greater Than",
    category: CAT_CONTROL,
    description: "Returns 1.0 if A > B, else 0.0",
    pins: || {
        vec![
            PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_CONTROL,
};

pub static LESS_THAN: MaterialNodeDef = MaterialNodeDef {
    node_type: "control/less_than",
    display_name: "Less Than",
    category: CAT_CONTROL,
    description: "Returns 1.0 if A < B, else 0.0",
    pins: || {
        vec![
            PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_CONTROL,
};

pub static EQUAL: MaterialNodeDef = MaterialNodeDef {
    node_type: "control/equal",
    display_name: "Equal",
    category: CAT_CONTROL,
    description: "Returns 1.0 if |A - B| < epsilon, else 0.0",
    pins: || {
        vec![
            PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("epsilon", "Epsilon", PinType::Float)
                .with_default(PinValue::Float(0.001)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_CONTROL,
};

pub static NOT_EQUAL: MaterialNodeDef = MaterialNodeDef {
    node_type: "control/not_equal",
    display_name: "Not Equal",
    category: CAT_CONTROL,
    description: "Returns 1.0 if |A - B| >= epsilon, else 0.0",
    pins: || {
        vec![
            PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("epsilon", "Epsilon", PinType::Float)
                .with_default(PinValue::Float(0.001)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_CONTROL,
};

pub static AND_NODE: MaterialNodeDef = MaterialNodeDef {
    node_type: "control/and",
    display_name: "And",
    category: CAT_CONTROL,
    description:
        "Logical AND on float booleans: min(A, B) — returns 1.0 only if both A and B are 1.0",
    pins: || {
        vec![
            PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_CONTROL,
};

pub static OR_NODE: MaterialNodeDef = MaterialNodeDef {
    node_type: "control/or",
    display_name: "Or",
    category: CAT_CONTROL,
    description: "Logical OR on float booleans: max(A, B) — returns 1.0 if either A or B is 1.0",
    pins: || {
        vec![
            PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_CONTROL,
};

pub static NOT_NODE: MaterialNodeDef = MaterialNodeDef {
    node_type: "control/not",
    display_name: "Not",
    category: CAT_CONTROL,
    description: "Logical NOT on float boolean: 1 - value",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_CONTROL,
};
