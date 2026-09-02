//! Function nodes — material subgraphs: the input and output points that live
//! *inside* a function, and the call node that invokes one.
//!
//! They carry `CAT_CONTROL` rather than a category of their own, so the palette
//! shows them alongside the branches they compose with.

use crate::material::graph::{PinTemplate, PinType, PinValue};

use super::{MaterialNodeDef, CAT_CONTROL, CLR_CONTROL};

pub static FUNCTION_INPUT_POINT: MaterialNodeDef = MaterialNodeDef {
    node_type: "function/input_point",
    display_name: "Function Inputs",
    category: CAT_CONTROL,
    description: "Inside a material function only: outputs in_0..in_3 (the function's call-site inputs as Vec4). Use split_vec* nodes to unpack scalars.",
    pins: || vec![
        PinTemplate::output("in_0", "In 0", PinType::Vec4),
        PinTemplate::output("in_1", "In 1", PinType::Vec4),
        PinTemplate::output("in_2", "In 2", PinType::Vec4),
        PinTemplate::output("in_3", "In 3", PinType::Vec4),
    ],
    color: CLR_CONTROL,
};

pub static FUNCTION_OUTPUT_POINT: MaterialNodeDef = MaterialNodeDef {
    node_type: "function/output_point",
    display_name: "Function Outputs",
    category: CAT_CONTROL,
    description: "Inside a material function only: receives out_0..out_3 (what the function returns at the call site).",
    pins: || vec![
        PinTemplate::input("out_0", "Out 0", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 0.0])),
        PinTemplate::input("out_1", "Out 1", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 0.0])),
        PinTemplate::input("out_2", "Out 2", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 0.0])),
        PinTemplate::input("out_3", "Out 3", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 0.0])),
    ],
    color: CLR_CONTROL,
};

pub static FUNCTION_CALL: MaterialNodeDef = MaterialNodeDef {
    node_type: "function/call",
    display_name: "Function Call",
    category: CAT_CONTROL,
    description: "Invoke a reusable material function by name. Set input_values[\"function\"] to the function's name (a file in assets/material_functions/).",
    pins: || vec![
        PinTemplate::input("in_0", "In 0", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 0.0])),
        PinTemplate::input("in_1", "In 1", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 0.0])),
        PinTemplate::input("in_2", "In 2", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 0.0])),
        PinTemplate::input("in_3", "In 3", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 0.0])),
        PinTemplate::output("out_0", "Out 0", PinType::Vec4),
        PinTemplate::output("out_1", "Out 1", PinType::Vec4),
        PinTemplate::output("out_2", "Out 2", PinType::Vec4),
        PinTemplate::output("out_3", "Out 3", PinType::Vec4),
    ],
    color: CLR_CONTROL,
};
