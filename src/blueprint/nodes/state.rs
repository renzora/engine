//! State nodes
//!
//! Nodes for application state management and game states.

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

// =============================================================================
// APP STATES
// =============================================================================

/// Get current state
pub static GET_CURRENT_STATE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/get_current",
    display_name: "Get Current State",
    category: "State",
    description: "Get the current application state",
    create_pins: || vec![
        Pin::output("state", "State", PinType::String),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Set state
pub static SET_STATE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/set",
    display_name: "Set State",
    category: "State",
    description: "Change the application state",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("state", "State", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Push state
pub static PUSH_STATE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/push",
    display_name: "Push State",
    category: "State",
    description: "Push a new state onto the state stack",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("state", "State", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Pop state
pub static POP_STATE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/pop",
    display_name: "Pop State",
    category: "State",
    description: "Pop the current state from the state stack",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("popped", "Popped State", PinType::String),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// On state enter
pub static ON_STATE_ENTER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/on_enter",
    display_name: "On State Enter",
    category: "State Events",
    description: "Triggered when entering a specific state",
    create_pins: || vec![
        Pin::input("state", "State", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [160, 120, 200],
    is_event: true,
    is_comment: false,
};

/// On state exit
pub static ON_STATE_EXIT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/on_exit",
    display_name: "On State Exit",
    category: "State Events",
    description: "Triggered when exiting a specific state",
    create_pins: || vec![
        Pin::input("state", "State", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [160, 120, 200],
    is_event: true,
    is_comment: false,
};

/// On state transition
pub static ON_STATE_TRANSITION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/on_transition",
    display_name: "On State Transition",
    category: "State Events",
    description: "Triggered when transitioning between states",
    create_pins: || vec![
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("from", "From State", PinType::String),
        Pin::output("to", "To State", PinType::String),
    ],
    color: [160, 120, 200],
    is_event: true,
    is_comment: false,
};

/// Is in state
pub static IS_IN_STATE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/is_in",
    display_name: "Is In State",
    category: "State",
    description: "Check if currently in a specific state",
    create_pins: || vec![
        Pin::input("state", "State", PinType::String),
        Pin::output("in_state", "In State", PinType::Bool),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// GAME PAUSE
// =============================================================================

/// Pause game
pub static PAUSE_GAME: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/pause",
    display_name: "Pause Game",
    category: "State",
    description: "Pause the game",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Resume game
pub static RESUME_GAME: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/resume",
    display_name: "Resume Game",
    category: "State",
    description: "Resume the game from pause",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Toggle pause
pub static TOGGLE_PAUSE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/toggle_pause",
    display_name: "Toggle Pause",
    category: "State",
    description: "Toggle game pause state",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("paused", "Is Paused", PinType::Bool),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Is paused
pub static IS_PAUSED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/is_paused",
    display_name: "Is Paused",
    category: "State",
    description: "Check if the game is paused",
    create_pins: || vec![
        Pin::output("paused", "Is Paused", PinType::Bool),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// On pause
pub static ON_PAUSE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/on_pause",
    display_name: "On Pause",
    category: "State Events",
    description: "Triggered when game is paused",
    create_pins: || vec![
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [160, 120, 200],
    is_event: true,
    is_comment: false,
};

/// On resume
pub static ON_RESUME: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/on_resume",
    display_name: "On Resume",
    category: "State Events",
    description: "Triggered when game is resumed",
    create_pins: || vec![
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [160, 120, 200],
    is_event: true,
    is_comment: false,
};

// =============================================================================
// GAME VARIABLES (GLOBAL STATE)
// =============================================================================

/// Set global variable
pub static SET_GLOBAL_VAR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/set_global",
    display_name: "Set Global Variable",
    category: "State",
    description: "Set a global game variable",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("name", "Name", PinType::String),
        Pin::input("value", "Value", PinType::Any),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Get global variable
pub static GET_GLOBAL_VAR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/get_global",
    display_name: "Get Global Variable",
    category: "State",
    description: "Get a global game variable",
    create_pins: || vec![
        Pin::input("name", "Name", PinType::String),
        Pin::output("value", "Value", PinType::Any),
        Pin::output("exists", "Exists", PinType::Bool),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Has global variable
pub static HAS_GLOBAL_VAR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/has_global",
    display_name: "Has Global Variable",
    category: "State",
    description: "Check if a global variable exists",
    create_pins: || vec![
        Pin::input("name", "Name", PinType::String),
        Pin::output("exists", "Exists", PinType::Bool),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Remove global variable
pub static REMOVE_GLOBAL_VAR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/remove_global",
    display_name: "Remove Global Variable",
    category: "State",
    description: "Remove a global variable",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("name", "Name", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// PERSISTENCE
// =============================================================================

/// Save game data
pub static SAVE_GAME_DATA: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/save_data",
    display_name: "Save Game Data",
    category: "State",
    description: "Save game data to persistent storage",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("slot", "Slot", PinType::String).with_default(PinValue::String("save1".into())),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("success", "Success", PinType::Bool),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Load game data
pub static LOAD_GAME_DATA: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/load_data",
    display_name: "Load Game Data",
    category: "State",
    description: "Load game data from persistent storage",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("slot", "Slot", PinType::String).with_default(PinValue::String("save1".into())),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("success", "Success", PinType::Bool),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Delete save data
pub static DELETE_SAVE_DATA: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/delete_save",
    display_name: "Delete Save Data",
    category: "State",
    description: "Delete a save slot",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("slot", "Slot", PinType::String).with_default(PinValue::String("save1".into())),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Has save data
pub static HAS_SAVE_DATA: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/has_save",
    display_name: "Has Save Data",
    category: "State",
    description: "Check if a save slot exists",
    create_pins: || vec![
        Pin::input("slot", "Slot", PinType::String).with_default(PinValue::String("save1".into())),
        Pin::output("exists", "Exists", PinType::Bool),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Get all save slots
pub static GET_SAVE_SLOTS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/get_saves",
    display_name: "Get Save Slots",
    category: "State",
    description: "Get all available save slots",
    create_pins: || vec![
        Pin::output("slots", "Slots", PinType::StringArray),
        Pin::output("count", "Count", PinType::Int),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// GAME LIFECYCLE
// =============================================================================

/// Quit game
pub static QUIT_GAME: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/quit",
    display_name: "Quit Game",
    category: "State",
    description: "Exit the game",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Restart game
pub static RESTART_GAME: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/restart",
    display_name: "Restart Game",
    category: "State",
    description: "Restart the game from the beginning",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// On quit requested
pub static ON_QUIT_REQUESTED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "state/on_quit_requested",
    display_name: "On Quit Requested",
    category: "State Events",
    description: "Triggered when quit is requested (can cancel)",
    create_pins: || vec![
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("allow_quit", "Allow Quit", PinType::Bool),
    ],
    color: [160, 120, 200],
    is_event: true,
    is_comment: false,
};
