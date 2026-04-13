//! Lifecycle node type definitions and registry.

use renzora::{BlueprintNodeDef, PinTemplate, PinType, PinValue};

pub const CAT_LIFECYCLE: &str = "Lifecycle";

const CLR_LIFECYCLE: [u8; 3] = [90, 180, 100];

// ── Shared category colors ─────────────────────────────────────────────────
const CLR_FLOW: [u8; 3] = [140, 140, 160];
const CLR_MATH: [u8; 3] = [120, 120, 120];
const CLR_STRING: [u8; 3] = [180, 130, 200];
const CLR_CONVERT: [u8; 3] = [150, 150, 180];
const CLR_DEBUG: [u8; 3] = [180, 180, 80];

/// All lifecycle-specific node definitions.
pub static ALL_NODES: &[&BlueprintNodeDef] = &[
    // ── Events ──
    &ON_GAME_START,
    &ON_SCENE_LOADED,
    &ON_CONNECTED,
    &ON_DISCONNECTED,
    &ON_PLAYER_JOINED,
    &ON_PLAYER_LEFT,
    &ON_TIMER,
    &ON_MESSAGE,
    // ── Actions ──
    &LOAD_SCENE,
    &WAIT,
    &START_TIMER,
    &CONNECT,
    &DISCONNECT,
    &HOST_SERVER,
    &SEND_MESSAGE,
    &SPAWN_NETWORKED,
    &LOG,
    // ── Data queries ──
    &IS_SERVER,
    &IS_CONNECTED,
    &GET_SCENE_NAME,
    &GET_PLAYER_COUNT,
    &GET_VARIABLE,
    &SET_VARIABLE,
];

/// Shared nodes (flow, math, string, convert, debug) that lifecycle also supports.
static SHARED_NODES: &[&BlueprintNodeDef] = &[
    // Flow
    &BRANCH, &SEQUENCE, &DO_ONCE, &FLIP_FLOP, &GATE, &COUNTER,
    // Math
    &MATH_ADD, &MATH_SUBTRACT, &MATH_MULTIPLY, &MATH_DIVIDE,
    &MATH_NEGATE, &MATH_ABS, &MATH_CLAMP, &MATH_COMPARE,
    &MATH_AND, &MATH_OR, &MATH_NOT, &MATH_MIN, &MATH_MAX,
    // String
    &STRING_CONCAT, &STRING_FORMAT,
    // Convert
    &TO_STRING, &TO_FLOAT, &TO_INT, &TO_BOOL,
    // Debug
    &DEBUG_LOG,
];

// ── Event nodes ─────────────────────────────────────────────────────────────

static ON_GAME_START: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/on_game_start",
    display_name: "On Game Start",
    category: CAT_LIFECYCLE,
    description: "Fires once when the game starts",
    pins: || {
        vec![PinTemplate::exec_out("exec", "")]
    },
    color: CLR_LIFECYCLE,
};

static ON_SCENE_LOADED: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/on_scene_loaded",
    display_name: "On Scene Loaded",
    category: CAT_LIFECYCLE,
    description: "Fires when a scene finishes loading",
    pins: || {
        vec![
            PinTemplate::exec_out("exec", ""),
            PinTemplate::output("scene", "Scene", PinType::String),
        ]
    },
    color: CLR_LIFECYCLE,
};

static ON_CONNECTED: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/on_connected",
    display_name: "On Connected",
    category: CAT_LIFECYCLE,
    description: "Fires when connected to a server",
    pins: || vec![PinTemplate::exec_out("exec", "")],
    color: CLR_LIFECYCLE,
};

static ON_DISCONNECTED: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/on_disconnected",
    display_name: "On Disconnected",
    category: CAT_LIFECYCLE,
    description: "Fires when disconnected from a server",
    pins: || vec![PinTemplate::exec_out("exec", "")],
    color: CLR_LIFECYCLE,
};

static ON_PLAYER_JOINED: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/on_player_joined",
    display_name: "On Player Joined",
    category: CAT_LIFECYCLE,
    description: "Fires when a player joins (server only)",
    pins: || {
        vec![
            PinTemplate::exec_out("exec", ""),
            PinTemplate::output("player_id", "Player ID", PinType::Int),
        ]
    },
    color: CLR_LIFECYCLE,
};

static ON_PLAYER_LEFT: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/on_player_left",
    display_name: "On Player Left",
    category: CAT_LIFECYCLE,
    description: "Fires when a player leaves (server only)",
    pins: || {
        vec![
            PinTemplate::exec_out("exec", ""),
            PinTemplate::output("player_id", "Player ID", PinType::Int),
        ]
    },
    color: CLR_LIFECYCLE,
};

static ON_TIMER: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/on_timer",
    display_name: "On Timer",
    category: CAT_LIFECYCLE,
    description: "Fires when a named timer completes",
    pins: || {
        vec![
            PinTemplate::exec_out("exec", ""),
            PinTemplate::input("name", "Name", PinType::String),
        ]
    },
    color: CLR_LIFECYCLE,
};

static ON_MESSAGE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/on_message",
    display_name: "On Message",
    category: CAT_LIFECYCLE,
    description: "Fires when a network message is received on a channel",
    pins: || {
        vec![
            PinTemplate::exec_out("exec", ""),
            PinTemplate::input("channel", "Channel", PinType::String),
            PinTemplate::output("data", "Data", PinType::String),
        ]
    },
    color: CLR_LIFECYCLE,
};

// ── Action nodes ────────────────────────────────────────────────────────────

static LOAD_SCENE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/load_scene",
    display_name: "Load Scene",
    category: CAT_LIFECYCLE,
    description: "Load a scene by path",
    pins: || {
        vec![
            PinTemplate::exec_in("exec", ""),
            PinTemplate::input("path", "Path", PinType::String),
            PinTemplate::exec_out("success", "Success"),
            PinTemplate::exec_out("error", "Error"),
        ]
    },
    color: CLR_LIFECYCLE,
};

static WAIT: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/wait",
    display_name: "Wait",
    category: CAT_LIFECYCLE,
    description: "Wait for a duration before continuing",
    pins: || {
        vec![
            PinTemplate::exec_in("exec", ""),
            PinTemplate::input("seconds", "Seconds", PinType::Float)
                .with_default(PinValue::Float(1.0)),
            PinTemplate::exec_out("success", "Success"),
            PinTemplate::exec_out("error", "Error"),
        ]
    },
    color: CLR_LIFECYCLE,
};

static START_TIMER: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/start_timer",
    display_name: "Start Timer",
    category: CAT_LIFECYCLE,
    description: "Start a named timer",
    pins: || {
        vec![
            PinTemplate::exec_in("exec", ""),
            PinTemplate::input("name", "Name", PinType::String),
            PinTemplate::input("seconds", "Seconds", PinType::Float),
            PinTemplate::input("repeat", "Repeat", PinType::Bool),
            PinTemplate::exec_out("success", "Success"),
            PinTemplate::exec_out("error", "Error"),
        ]
    },
    color: CLR_LIFECYCLE,
};

static CONNECT: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/connect",
    display_name: "Connect",
    category: CAT_LIFECYCLE,
    description: "Connect to a server",
    pins: || {
        vec![
            PinTemplate::exec_in("exec", ""),
            PinTemplate::input("address", "Address", PinType::String),
            PinTemplate::input("port", "Port", PinType::Int)
                .with_default(PinValue::Int(7636)),
            PinTemplate::exec_out("success", "Success"),
            PinTemplate::exec_out("error", "Error"),
        ]
    },
    color: CLR_LIFECYCLE,
};

static DISCONNECT: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/disconnect",
    display_name: "Disconnect",
    category: CAT_LIFECYCLE,
    description: "Disconnect from the server",
    pins: || {
        vec![
            PinTemplate::exec_in("exec", ""),
            PinTemplate::exec_out("success", "Success"),
            PinTemplate::exec_out("error", "Error"),
        ]
    },
    color: CLR_LIFECYCLE,
};

static HOST_SERVER: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/host_server",
    display_name: "Host Server",
    category: CAT_LIFECYCLE,
    description: "Start hosting a server",
    pins: || {
        vec![
            PinTemplate::exec_in("exec", ""),
            PinTemplate::input("port", "Port", PinType::Int)
                .with_default(PinValue::Int(7636)),
            PinTemplate::input("max_clients", "Max Clients", PinType::Int)
                .with_default(PinValue::Int(32)),
            PinTemplate::exec_out("success", "Success"),
            PinTemplate::exec_out("error", "Error"),
        ]
    },
    color: CLR_LIFECYCLE,
};

static SEND_MESSAGE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/send_message",
    display_name: "Send Message",
    category: CAT_LIFECYCLE,
    description: "Send a network message on a channel",
    pins: || {
        vec![
            PinTemplate::exec_in("exec", ""),
            PinTemplate::input("channel", "Channel", PinType::String),
            PinTemplate::input("data", "Data", PinType::String),
            PinTemplate::exec_out("success", "Success"),
            PinTemplate::exec_out("error", "Error"),
        ]
    },
    color: CLR_LIFECYCLE,
};

static SPAWN_NETWORKED: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/spawn_networked",
    display_name: "Spawn Networked",
    category: CAT_LIFECYCLE,
    description: "Spawn a networked entity",
    pins: || {
        vec![
            PinTemplate::exec_in("exec", ""),
            PinTemplate::input("name", "Name", PinType::String),
            PinTemplate::input("position", "Position", PinType::Vec3),
            PinTemplate::exec_out("success", "Success"),
            PinTemplate::exec_out("error", "Error"),
        ]
    },
    color: CLR_LIFECYCLE,
};

static LOG: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/log",
    display_name: "Log",
    category: CAT_LIFECYCLE,
    description: "Log a message to the console",
    pins: || {
        vec![
            PinTemplate::exec_in("exec", ""),
            PinTemplate::input("message", "Message", PinType::String),
            PinTemplate::exec_out("success", "Success"),
            PinTemplate::exec_out("error", "Error"),
        ]
    },
    color: CLR_LIFECYCLE,
};

// ── Data query nodes ────────────────────────────────────────────────────────

static IS_SERVER: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/is_server",
    display_name: "Is Server",
    category: CAT_LIFECYCLE,
    description: "Returns true if running as server",
    pins: || vec![PinTemplate::output("value", "Value", PinType::Bool)],
    color: CLR_LIFECYCLE,
};

static IS_CONNECTED: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/is_connected",
    display_name: "Is Connected",
    category: CAT_LIFECYCLE,
    description: "Returns true if connected to a server",
    pins: || vec![PinTemplate::output("value", "Value", PinType::Bool)],
    color: CLR_LIFECYCLE,
};

static GET_SCENE_NAME: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/get_scene_name",
    display_name: "Get Scene Name",
    category: CAT_LIFECYCLE,
    description: "Returns the current scene name",
    pins: || vec![PinTemplate::output("name", "Name", PinType::String)],
    color: CLR_LIFECYCLE,
};

static GET_PLAYER_COUNT: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/get_player_count",
    display_name: "Get Player Count",
    category: CAT_LIFECYCLE,
    description: "Returns the number of connected players",
    pins: || vec![PinTemplate::output("count", "Count", PinType::Int)],
    color: CLR_LIFECYCLE,
};

static GET_VARIABLE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/get_variable",
    display_name: "Get Variable",
    category: CAT_LIFECYCLE,
    description: "Read a lifecycle variable by name",
    pins: || {
        vec![
            PinTemplate::input("name", "Name", PinType::String),
            PinTemplate::output("value", "Value", PinType::Any),
        ]
    },
    color: CLR_LIFECYCLE,
};

static SET_VARIABLE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "lifecycle/set_variable",
    display_name: "Set Variable",
    category: CAT_LIFECYCLE,
    description: "Write a lifecycle variable by name",
    pins: || {
        vec![
            PinTemplate::exec_in("exec", ""),
            PinTemplate::input("name", "Name", PinType::String),
            PinTemplate::input("value", "Value", PinType::Any),
            PinTemplate::exec_out("success", "Success"),
            PinTemplate::exec_out("error", "Error"),
        ]
    },
    color: CLR_LIFECYCLE,
};

// ═══════════════════════════════════════════════════════════════════════════
// Shared node definitions (duplicated from renzora_blueprint to avoid dep)
// ═══════════════════════════════════════════════════════════════════════════

// ── Flow ────────────────────────────────────────────────────────────────────

static BRANCH: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "flow/branch",
    display_name: "Branch",
    category: "Flow",
    description: "If/else — routes execution based on a condition",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("condition", "Condition", PinType::Bool)
            .with_default(PinValue::Bool(true)),
        PinTemplate::exec_out("true", "True"),
        PinTemplate::exec_out("false", "False"),
    ],
    color: CLR_FLOW,
};

static SEQUENCE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "flow/sequence",
    display_name: "Sequence",
    category: "Flow",
    description: "Executes outputs in order",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::exec_out("then_0", "Then 0"),
        PinTemplate::exec_out("then_1", "Then 1"),
        PinTemplate::exec_out("then_2", "Then 2"),
    ],
    color: CLR_FLOW,
};

static DO_ONCE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "flow/do_once",
    display_name: "Do Once",
    category: "Flow",
    description: "Executes only the first time",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::exec_in("reset", "Reset"),
        PinTemplate::exec_out("completed", "Completed"),
    ],
    color: CLR_FLOW,
};

static FLIP_FLOP: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "flow/flip_flop",
    display_name: "Flip Flop",
    category: "Flow",
    description: "Alternates between A and B each time triggered",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::exec_out("a", "A"),
        PinTemplate::exec_out("b", "B"),
        PinTemplate::output("is_a", "Is A", PinType::Bool),
    ],
    color: CLR_FLOW,
};

static GATE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "flow/gate",
    display_name: "Gate",
    category: "Flow",
    description: "Only passes execution when the gate is open",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::exec_in("open", "Open"),
        PinTemplate::exec_in("close", "Close"),
        PinTemplate::exec_in("toggle", "Toggle"),
        PinTemplate::input("start_open", "Start Open", PinType::Bool)
            .with_default(PinValue::Bool(true)),
        PinTemplate::exec_out("exit", "Exit"),
    ],
    color: CLR_FLOW,
};

static COUNTER: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "flow/counter",
    display_name: "Counter",
    category: "Flow",
    description: "Increments a value each time it executes",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("step", "Step", PinType::Float)
            .with_default(PinValue::Float(1.0)),
        PinTemplate::input("min", "Min", PinType::Float)
            .with_default(PinValue::Float(0.0)),
        PinTemplate::input("max", "Max", PinType::Float)
            .with_default(PinValue::Float(1.0)),
        PinTemplate::input("loop", "Loop", PinType::Bool)
            .with_default(PinValue::Bool(true)),
        PinTemplate::exec_out("then", ""),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_FLOW,
};

// ── Math ────────────────────────────────────────────────────────────────────

static MATH_ADD: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/add",
    display_name: "Add",
    category: "Math",
    description: "A + B",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

static MATH_SUBTRACT: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/subtract",
    display_name: "Subtract",
    category: "Math",
    description: "A - B",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

static MATH_MULTIPLY: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/multiply",
    display_name: "Multiply",
    category: "Math",
    description: "A * B",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

static MATH_DIVIDE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/divide",
    display_name: "Divide",
    category: "Math",
    description: "A / B (safe — returns 0 if B is 0)",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

static MATH_NEGATE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/negate",
    display_name: "Negate",
    category: "Math",
    description: "-Value",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

static MATH_ABS: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/abs",
    display_name: "Abs",
    category: "Math",
    description: "Absolute value",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

static MATH_CLAMP: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/clamp",
    display_name: "Clamp",
    category: "Math",
    description: "Clamp value between min and max",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::input("min", "Min", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("max", "Max", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

static MATH_COMPARE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/compare",
    display_name: "Compare",
    category: "Math",
    description: "A > B, A < B, A == B comparisons",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("greater", "A > B", PinType::Bool),
        PinTemplate::output("less", "A < B", PinType::Bool),
        PinTemplate::output("equal", "A == B", PinType::Bool),
    ],
    color: CLR_MATH,
};

static MATH_AND: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/and",
    display_name: "AND",
    category: "Math",
    description: "Logical AND",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Bool).with_default(PinValue::Bool(false)),
        PinTemplate::input("b", "B", PinType::Bool).with_default(PinValue::Bool(false)),
        PinTemplate::output("result", "Result", PinType::Bool),
    ],
    color: CLR_MATH,
};

static MATH_OR: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/or",
    display_name: "OR",
    category: "Math",
    description: "Logical OR",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Bool).with_default(PinValue::Bool(false)),
        PinTemplate::input("b", "B", PinType::Bool).with_default(PinValue::Bool(false)),
        PinTemplate::output("result", "Result", PinType::Bool),
    ],
    color: CLR_MATH,
};

static MATH_NOT: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/not",
    display_name: "NOT",
    category: "Math",
    description: "Logical NOT",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Bool).with_default(PinValue::Bool(false)),
        PinTemplate::output("result", "Result", PinType::Bool),
    ],
    color: CLR_MATH,
};

static MATH_MIN: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/min",
    display_name: "Min",
    category: "Math",
    description: "Return the smaller of two values",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

static MATH_MAX: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/max",
    display_name: "Max",
    category: "Math",
    description: "Return the larger of two values",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

// ── String ──────────────────────────────────────────────────────────────────

static STRING_CONCAT: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "string/concat",
    display_name: "Concat",
    category: "String",
    description: "Concatenate two strings",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::String).with_default(PinValue::String(String::new())),
        PinTemplate::input("b", "B", PinType::String).with_default(PinValue::String(String::new())),
        PinTemplate::output("result", "Result", PinType::String),
    ],
    color: CLR_STRING,
};

static STRING_FORMAT: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "string/format",
    display_name: "Format",
    category: "String",
    description: "Replace {0} in template with value",
    pins: || vec![
        PinTemplate::input("template", "Template", PinType::String)
            .with_default(PinValue::String("Value: {0}".into())),
        PinTemplate::input("value", "Value", PinType::Any),
        PinTemplate::output("result", "Result", PinType::String),
    ],
    color: CLR_STRING,
};

// ── Convert ─────────────────────────────────────────────────────────────────

static TO_STRING: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "convert/to_string",
    display_name: "To String",
    category: "Convert",
    description: "Convert any value to a string",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Any),
        PinTemplate::output("result", "Result", PinType::String),
    ],
    color: CLR_CONVERT,
};

static TO_FLOAT: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "convert/to_float",
    display_name: "To Float",
    category: "Convert",
    description: "Convert a value to float",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Any),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_CONVERT,
};

static TO_INT: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "convert/to_int",
    display_name: "To Int",
    category: "Convert",
    description: "Convert a value to integer",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Any),
        PinTemplate::output("result", "Result", PinType::Int),
    ],
    color: CLR_CONVERT,
};

static TO_BOOL: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "convert/to_bool",
    display_name: "To Bool",
    category: "Convert",
    description: "Convert a value to boolean",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Any),
        PinTemplate::output("result", "Result", PinType::Bool),
    ],
    color: CLR_CONVERT,
};

// ── Debug ───────────────────────────────────────────────────────────────────

static DEBUG_LOG: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "debug/log",
    display_name: "Log",
    category: "Debug",
    description: "Print a message to the console",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("message", "Message", PinType::String)
            .with_default(PinValue::String("Hello!".into())),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_DEBUG,
};

// ── Registry ────────────────────────────────────────────────────────────────

/// Look up a lifecycle node definition by type string.
pub fn node_def(node_type: &str) -> Option<&'static BlueprintNodeDef> {
    // Check lifecycle-specific nodes first
    for def in ALL_NODES {
        if def.node_type == node_type {
            return Some(def);
        }
    }
    // Fall back to shared nodes (flow, math, string, convert, debug)
    for def in SHARED_NODES {
        if def.node_type == node_type {
            return Some(def);
        }
    }
    None
}

/// Return all lifecycle-specific categories.
pub fn categories() -> Vec<&'static str> {
    vec![CAT_LIFECYCLE, "Flow", "Math", "String", "Convert", "Debug"]
}

/// Return all nodes in a given category.
pub fn nodes_in_category(category: &str) -> Vec<&'static BlueprintNodeDef> {
    if category == CAT_LIFECYCLE {
        return ALL_NODES.iter().copied().collect();
    }
    // Return shared nodes in the requested category
    SHARED_NODES
        .iter()
        .copied()
        .filter(|n| n.category == category)
        .collect()
}
