//! Flow control nodes
//!
//! Nodes for loops, conditionals, and execution flow control.

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

// =============================================================================
// LOOPS
// =============================================================================

/// For loop
pub static FOR_LOOP: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "flow/for",
    display_name: "For Loop",
    category: "Flow",
    description: "Execute a loop a fixed number of times",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("start", "Start", PinType::Int).with_default(PinValue::Int(0)),
        Pin::input("end", "End", PinType::Int).with_default(PinValue::Int(10)),
        Pin::input("step", "Step", PinType::Int).with_default(PinValue::Int(1)),
        Pin::output("loop", "Loop Body", PinType::Execution),
        Pin::output("index", "Index", PinType::Int),
        Pin::output("completed", "Completed", PinType::Execution),
    ],
    color: [200, 100, 150],
    is_event: false,
    is_comment: false,
};

/// For each loop
pub static FOR_EACH: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "flow/for_each",
    display_name: "For Each",
    category: "Flow",
    description: "Iterate over a collection",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("array", "Array", PinType::Any),
        Pin::output("loop", "Loop Body", PinType::Execution),
        Pin::output("element", "Element", PinType::Any),
        Pin::output("index", "Index", PinType::Int),
        Pin::output("completed", "Completed", PinType::Execution),
    ],
    color: [200, 100, 150],
    is_event: false,
    is_comment: false,
};

/// While loop
pub static WHILE_LOOP: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "flow/while",
    display_name: "While Loop",
    category: "Flow",
    description: "Execute while condition is true",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("condition", "Condition", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("loop", "Loop Body", PinType::Execution),
        Pin::output("completed", "Completed", PinType::Execution),
    ],
    color: [200, 100, 150],
    is_event: false,
    is_comment: false,
};

/// Do while loop
pub static DO_WHILE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "flow/do_while",
    display_name: "Do While",
    category: "Flow",
    description: "Execute at least once, then while condition is true",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("condition", "Condition", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("loop", "Loop Body", PinType::Execution),
        Pin::output("completed", "Completed", PinType::Execution),
    ],
    color: [200, 100, 150],
    is_event: false,
    is_comment: false,
};

/// Break
pub static BREAK: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "flow/break",
    display_name: "Break",
    category: "Flow",
    description: "Break out of current loop",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
    ],
    color: [200, 100, 150],
    is_event: false,
    is_comment: false,
};

/// Continue
pub static CONTINUE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "flow/continue",
    display_name: "Continue",
    category: "Flow",
    description: "Continue to next iteration of loop",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
    ],
    color: [200, 100, 150],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// CONDITIONALS
// =============================================================================

/// If
pub static IF: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "flow/if",
    display_name: "If",
    category: "Flow",
    description: "Branch based on a condition",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("condition", "Condition", PinType::Bool),
        Pin::output("true", "True", PinType::Execution),
        Pin::output("false", "False", PinType::Execution),
    ],
    color: [200, 100, 150],
    is_event: false,
    is_comment: false,
};

/// Switch on int
pub static SWITCH_INT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "flow/switch_int",
    display_name: "Switch (Int)",
    category: "Flow",
    description: "Branch based on an integer value",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("value", "Value", PinType::Int),
        Pin::output("case_0", "Case 0", PinType::Execution),
        Pin::output("case_1", "Case 1", PinType::Execution),
        Pin::output("case_2", "Case 2", PinType::Execution),
        Pin::output("case_3", "Case 3", PinType::Execution),
        Pin::output("default", "Default", PinType::Execution),
    ],
    color: [200, 100, 150],
    is_event: false,
    is_comment: false,
};

/// Switch on string
pub static SWITCH_STRING: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "flow/switch_string",
    display_name: "Switch (String)",
    category: "Flow",
    description: "Branch based on a string value",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("value", "Value", PinType::String),
        Pin::input("case_0", "Case 0", PinType::String),
        Pin::input("case_1", "Case 1", PinType::String),
        Pin::input("case_2", "Case 2", PinType::String),
        Pin::input("case_3", "Case 3", PinType::String),
        Pin::output("out_0", "Out 0", PinType::Execution),
        Pin::output("out_1", "Out 1", PinType::Execution),
        Pin::output("out_2", "Out 2", PinType::Execution),
        Pin::output("out_3", "Out 3", PinType::Execution),
        Pin::output("default", "Default", PinType::Execution),
    ],
    color: [200, 100, 150],
    is_event: false,
    is_comment: false,
};

/// Multi-gate
pub static MULTI_GATE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "flow/multi_gate",
    display_name: "Multi-Gate",
    category: "Flow",
    description: "Execute outputs in sequence",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("reset", "Reset", PinType::Execution),
        Pin::input("loop", "Loop", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::output("out_0", "Out 0", PinType::Execution),
        Pin::output("out_1", "Out 1", PinType::Execution),
        Pin::output("out_2", "Out 2", PinType::Execution),
        Pin::output("out_3", "Out 3", PinType::Execution),
    ],
    color: [200, 100, 150],
    is_event: false,
    is_comment: false,
};

/// Do once
pub static DO_ONCE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "flow/do_once",
    display_name: "Do Once",
    category: "Flow",
    description: "Execute only once until reset",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("reset", "Reset", PinType::Execution),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [200, 100, 150],
    is_event: false,
    is_comment: false,
};

/// Do N
pub static DO_N: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "flow/do_n",
    display_name: "Do N Times",
    category: "Flow",
    description: "Execute N times then stop",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("n", "N", PinType::Int).with_default(PinValue::Int(1)),
        Pin::input("reset", "Reset", PinType::Execution),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("count", "Count", PinType::Int),
    ],
    color: [200, 100, 150],
    is_event: false,
    is_comment: false,
};

/// Flip flop
pub static FLIP_FLOP: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "flow/flip_flop",
    display_name: "Flip Flop",
    category: "Flow",
    description: "Alternate between two outputs",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::output("a", "A", PinType::Execution),
        Pin::output("b", "B", PinType::Execution),
        Pin::output("is_a", "Is A", PinType::Bool),
    ],
    color: [200, 100, 150],
    is_event: false,
    is_comment: false,
};

/// Gate
pub static GATE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "flow/gate",
    display_name: "Gate",
    category: "Flow",
    description: "Control flow with open/close gate",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("open", "Open", PinType::Execution),
        Pin::input("close", "Close", PinType::Execution),
        Pin::input("toggle", "Toggle", PinType::Execution),
        Pin::input("start_open", "Start Open", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [200, 100, 150],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// SEQUENCE/PARALLEL
// =============================================================================

/// Sequence (already exists but including for completeness)
pub static SEQUENCE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "flow/sequence",
    display_name: "Sequence",
    category: "Flow",
    description: "Execute outputs in sequence",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::output("then_0", "Then 0", PinType::Execution),
        Pin::output("then_1", "Then 1", PinType::Execution),
        Pin::output("then_2", "Then 2", PinType::Execution),
        Pin::output("then_3", "Then 3", PinType::Execution),
    ],
    color: [200, 100, 150],
    is_event: false,
    is_comment: false,
};

/// Parallel
pub static PARALLEL: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "flow/parallel",
    display_name: "Parallel",
    category: "Flow",
    description: "Execute multiple paths in parallel",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::output("branch_0", "Branch 0", PinType::Execution),
        Pin::output("branch_1", "Branch 1", PinType::Execution),
        Pin::output("branch_2", "Branch 2", PinType::Execution),
        Pin::output("branch_3", "Branch 3", PinType::Execution),
    ],
    color: [200, 100, 150],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// SELECTION
// =============================================================================

/// Select int
pub static SELECT_INT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "flow/select_int",
    display_name: "Select (Int)",
    category: "Flow",
    description: "Select a value based on condition",
    create_pins: || vec![
        Pin::input("condition", "Condition", PinType::Bool),
        Pin::input("true_value", "True Value", PinType::Int),
        Pin::input("false_value", "False Value", PinType::Int),
        Pin::output("result", "Result", PinType::Int),
    ],
    color: [200, 100, 150],
    is_event: false,
    is_comment: false,
};

/// Select float
pub static SELECT_FLOAT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "flow/select_float",
    display_name: "Select (Float)",
    category: "Flow",
    description: "Select a value based on condition",
    create_pins: || vec![
        Pin::input("condition", "Condition", PinType::Bool),
        Pin::input("true_value", "True Value", PinType::Float),
        Pin::input("false_value", "False Value", PinType::Float),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [200, 100, 150],
    is_event: false,
    is_comment: false,
};

/// Select string
pub static SELECT_STRING: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "flow/select_string",
    display_name: "Select (String)",
    category: "Flow",
    description: "Select a value based on condition",
    create_pins: || vec![
        Pin::input("condition", "Condition", PinType::Bool),
        Pin::input("true_value", "True Value", PinType::String),
        Pin::input("false_value", "False Value", PinType::String),
        Pin::output("result", "Result", PinType::String),
    ],
    color: [200, 100, 150],
    is_event: false,
    is_comment: false,
};

/// Select vec3
pub static SELECT_VEC3: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "flow/select_vec3",
    display_name: "Select (Vec3)",
    category: "Flow",
    description: "Select a value based on condition",
    create_pins: || vec![
        Pin::input("condition", "Condition", PinType::Bool),
        Pin::input("true_value", "True Value", PinType::Vec3),
        Pin::input("false_value", "False Value", PinType::Vec3),
        Pin::output("result", "Result", PinType::Vec3),
    ],
    color: [200, 100, 150],
    is_event: false,
    is_comment: false,
};

/// Select entity
pub static SELECT_ENTITY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "flow/select_entity",
    display_name: "Select (Entity)",
    category: "Flow",
    description: "Select an entity based on condition",
    create_pins: || vec![
        Pin::input("condition", "Condition", PinType::Bool),
        Pin::input("true_value", "True Value", PinType::Entity),
        Pin::input("false_value", "False Value", PinType::Entity),
        Pin::output("result", "Result", PinType::Entity),
    ],
    color: [200, 100, 150],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// RETURN/EXIT
// =============================================================================

/// Return
pub static RETURN: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "flow/return",
    display_name: "Return",
    category: "Flow",
    description: "Return from current function/script",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("value", "Value", PinType::Any),
    ],
    color: [200, 100, 150],
    is_event: false,
    is_comment: false,
};
