//! Easing function nodes
//!
//! Nodes for various easing/interpolation functions commonly used in animations.

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

// =============================================================================
// QUADRATIC EASING
// =============================================================================

/// Ease in quadratic
pub static EASE_IN_QUAD: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/in_quad",
    display_name: "Ease In Quad",
    category: "Easing",
    description: "Quadratic ease-in: accelerating from zero velocity",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Ease out quadratic
pub static EASE_OUT_QUAD: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/out_quad",
    display_name: "Ease Out Quad",
    category: "Easing",
    description: "Quadratic ease-out: decelerating to zero velocity",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Ease in-out quadratic
pub static EASE_INOUT_QUAD: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/inout_quad",
    display_name: "Ease In-Out Quad",
    category: "Easing",
    description: "Quadratic ease-in-out: acceleration until halfway, then deceleration",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// CUBIC EASING
// =============================================================================

/// Ease in cubic
pub static EASE_IN_CUBIC: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/in_cubic",
    display_name: "Ease In Cubic",
    category: "Easing",
    description: "Cubic ease-in: accelerating from zero velocity",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Ease out cubic
pub static EASE_OUT_CUBIC: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/out_cubic",
    display_name: "Ease Out Cubic",
    category: "Easing",
    description: "Cubic ease-out: decelerating to zero velocity",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Ease in-out cubic
pub static EASE_INOUT_CUBIC: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/inout_cubic",
    display_name: "Ease In-Out Cubic",
    category: "Easing",
    description: "Cubic ease-in-out: acceleration until halfway, then deceleration",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// QUARTIC EASING
// =============================================================================

/// Ease in quartic
pub static EASE_IN_QUART: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/in_quart",
    display_name: "Ease In Quart",
    category: "Easing",
    description: "Quartic ease-in: accelerating from zero velocity",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Ease out quartic
pub static EASE_OUT_QUART: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/out_quart",
    display_name: "Ease Out Quart",
    category: "Easing",
    description: "Quartic ease-out: decelerating to zero velocity",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Ease in-out quartic
pub static EASE_INOUT_QUART: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/inout_quart",
    display_name: "Ease In-Out Quart",
    category: "Easing",
    description: "Quartic ease-in-out: acceleration until halfway, then deceleration",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// QUINTIC EASING
// =============================================================================

/// Ease in quintic
pub static EASE_IN_QUINT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/in_quint",
    display_name: "Ease In Quint",
    category: "Easing",
    description: "Quintic ease-in: accelerating from zero velocity",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Ease out quintic
pub static EASE_OUT_QUINT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/out_quint",
    display_name: "Ease Out Quint",
    category: "Easing",
    description: "Quintic ease-out: decelerating to zero velocity",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Ease in-out quintic
pub static EASE_INOUT_QUINT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/inout_quint",
    display_name: "Ease In-Out Quint",
    category: "Easing",
    description: "Quintic ease-in-out: acceleration until halfway, then deceleration",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// SINE EASING
// =============================================================================

/// Ease in sine
pub static EASE_IN_SINE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/in_sine",
    display_name: "Ease In Sine",
    category: "Easing",
    description: "Sinusoidal ease-in: accelerating from zero velocity",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Ease out sine
pub static EASE_OUT_SINE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/out_sine",
    display_name: "Ease Out Sine",
    category: "Easing",
    description: "Sinusoidal ease-out: decelerating to zero velocity",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Ease in-out sine
pub static EASE_INOUT_SINE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/inout_sine",
    display_name: "Ease In-Out Sine",
    category: "Easing",
    description: "Sinusoidal ease-in-out: acceleration until halfway, then deceleration",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// EXPONENTIAL EASING
// =============================================================================

/// Ease in exponential
pub static EASE_IN_EXPO: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/in_expo",
    display_name: "Ease In Expo",
    category: "Easing",
    description: "Exponential ease-in: accelerating from zero velocity",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Ease out exponential
pub static EASE_OUT_EXPO: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/out_expo",
    display_name: "Ease Out Expo",
    category: "Easing",
    description: "Exponential ease-out: decelerating to zero velocity",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Ease in-out exponential
pub static EASE_INOUT_EXPO: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/inout_expo",
    display_name: "Ease In-Out Expo",
    category: "Easing",
    description: "Exponential ease-in-out: acceleration until halfway, then deceleration",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// CIRCULAR EASING
// =============================================================================

/// Ease in circular
pub static EASE_IN_CIRC: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/in_circ",
    display_name: "Ease In Circ",
    category: "Easing",
    description: "Circular ease-in: accelerating from zero velocity",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Ease out circular
pub static EASE_OUT_CIRC: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/out_circ",
    display_name: "Ease Out Circ",
    category: "Easing",
    description: "Circular ease-out: decelerating to zero velocity",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Ease in-out circular
pub static EASE_INOUT_CIRC: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/inout_circ",
    display_name: "Ease In-Out Circ",
    category: "Easing",
    description: "Circular ease-in-out: acceleration until halfway, then deceleration",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// BACK EASING
// =============================================================================

/// Ease in back
pub static EASE_IN_BACK: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/in_back",
    display_name: "Ease In Back",
    category: "Easing",
    description: "Back ease-in: overshooting cubic ease-in",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("overshoot", "Overshoot", PinType::Float).with_default(PinValue::Float(1.70158)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Ease out back
pub static EASE_OUT_BACK: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/out_back",
    display_name: "Ease Out Back",
    category: "Easing",
    description: "Back ease-out: overshooting cubic ease-out",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("overshoot", "Overshoot", PinType::Float).with_default(PinValue::Float(1.70158)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Ease in-out back
pub static EASE_INOUT_BACK: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/inout_back",
    display_name: "Ease In-Out Back",
    category: "Easing",
    description: "Back ease-in-out: overshooting cubic ease-in-out",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("overshoot", "Overshoot", PinType::Float).with_default(PinValue::Float(1.70158)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// ELASTIC EASING
// =============================================================================

/// Ease in elastic
pub static EASE_IN_ELASTIC: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/in_elastic",
    display_name: "Ease In Elastic",
    category: "Easing",
    description: "Elastic ease-in: exponentially decaying sine wave",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Ease out elastic
pub static EASE_OUT_ELASTIC: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/out_elastic",
    display_name: "Ease Out Elastic",
    category: "Easing",
    description: "Elastic ease-out: exponentially decaying sine wave",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Ease in-out elastic
pub static EASE_INOUT_ELASTIC: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/inout_elastic",
    display_name: "Ease In-Out Elastic",
    category: "Easing",
    description: "Elastic ease-in-out: exponentially decaying sine wave",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// BOUNCE EASING
// =============================================================================

/// Ease in bounce
pub static EASE_IN_BOUNCE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/in_bounce",
    display_name: "Ease In Bounce",
    category: "Easing",
    description: "Bounce ease-in: exponentially decaying bounce",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Ease out bounce
pub static EASE_OUT_BOUNCE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/out_bounce",
    display_name: "Ease Out Bounce",
    category: "Easing",
    description: "Bounce ease-out: exponentially decaying bounce",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Ease in-out bounce
pub static EASE_INOUT_BOUNCE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/inout_bounce",
    display_name: "Ease In-Out Bounce",
    category: "Easing",
    description: "Bounce ease-in-out: exponentially decaying bounce",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// GENERIC/CONFIGURABLE
// =============================================================================

/// Linear interpolation (no easing)
pub static EASE_LINEAR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/linear",
    display_name: "Linear",
    category: "Easing",
    description: "Linear interpolation (no easing)",
    create_pins: || vec![
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Apply easing to value
pub static APPLY_EASING: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/apply",
    display_name: "Apply Easing",
    category: "Easing",
    description: "Apply an easing function to interpolate from A to B",
    create_pins: || vec![
        Pin::input("from", "From", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("to", "To", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("eased_t", "Eased T", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Inverse lerp (get T from value)
pub static INVERSE_LERP: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "easing/inverse_lerp",
    display_name: "Inverse Lerp",
    category: "Easing",
    description: "Find T such that lerp(from, to, t) = value",
    create_pins: || vec![
        Pin::input("from", "From", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("to", "To", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::output("t", "T", PinType::Float),
    ],
    color: [140, 180, 220],
    is_event: false,
    is_comment: false,
};
