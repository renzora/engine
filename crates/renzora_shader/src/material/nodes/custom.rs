//! The Custom Code node — the graph's escape hatch into hand-written WGSL.

use crate::material::graph::{PinTemplate, PinType, PinValue};

use super::{MaterialNodeDef, CAT_CUSTOM, CLR_CUSTOM};

/// The escape hatch: inline WGSL when no combination of nodes expresses what
/// you need. Mirrors Unreal's Custom (HLSL) expression. The snippet runs in a
/// generated helper function with the four `vec4` inputs `a`/`b`/`c`/`d` in
/// scope and must assign the `result` (`vec4<f32>`, pre-seeded to opaque
/// black). Outputs expose the result and its individual channels.
pub static CUSTOM_CODE: MaterialNodeDef = MaterialNodeDef {
    node_type: "custom/code",
    display_name: "Custom Code",
    category: CAT_CUSTOM,
    description: "Inline WGSL. Reads vec4 inputs a/b/c/d and assigns `result`.",
    pins: || {
        vec![
            PinTemplate::input("code", "WGSL", PinType::String)
                .with_default(PinValue::String("result = a;".to_string())),
            PinTemplate::input("a", "A", PinType::Vec4)
                .with_default(PinValue::Vec4([0.0, 0.0, 0.0, 1.0])),
            PinTemplate::input("b", "B", PinType::Vec4)
                .with_default(PinValue::Vec4([0.0, 0.0, 0.0, 0.0])),
            PinTemplate::input("c", "C", PinType::Vec4)
                .with_default(PinValue::Vec4([0.0, 0.0, 0.0, 0.0])),
            PinTemplate::input("d", "D", PinType::Vec4)
                .with_default(PinValue::Vec4([0.0, 0.0, 0.0, 0.0])),
            PinTemplate::output("result", "Result", PinType::Vec4),
            PinTemplate::output("rgb", "RGB", PinType::Vec3),
            PinTemplate::output("x", "X", PinType::Float),
            PinTemplate::output("y", "Y", PinType::Float),
            PinTemplate::output("z", "Z", PinType::Float),
            PinTemplate::output("w", "W", PinType::Float),
        ]
    },
    color: CLR_CUSTOM,
};
