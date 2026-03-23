//! Lifecycle node type definitions and registry.

use renzora_blueprint::graph::{BlueprintNodeDef, PinTemplate, PinType, PinValue};

pub const CAT_LIFECYCLE: &str = "Lifecycle";

const CLR_LIFECYCLE: [u8; 3] = [90, 180, 100];

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

// ── Registry ────────────────────────────────────────────────────────────────

/// Look up a lifecycle node definition by type string.
pub fn node_def(node_type: &str) -> Option<&'static BlueprintNodeDef> {
    // Check lifecycle-specific nodes first
    for def in ALL_NODES {
        if def.node_type == node_type {
            return Some(def);
        }
    }
    // Fall back to shared blueprint nodes (flow, math, string, convert)
    renzora_blueprint::node_def(node_type)
}

/// Return all lifecycle-specific categories.
pub fn categories() -> Vec<&'static str> {
    vec![CAT_LIFECYCLE, "Flow", "Math", "String", "Convert"]
}

/// Return all nodes in a given category.
pub fn nodes_in_category(category: &str) -> Vec<&'static BlueprintNodeDef> {
    if category == CAT_LIFECYCLE {
        return ALL_NODES.iter().copied().collect();
    }
    // Delegate to blueprint for shared categories
    renzora_blueprint::nodes_in_category(category)
}
