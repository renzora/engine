//! Time nodes
//!
//! Nodes for time management, timers, and delays.

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

// =============================================================================
// TIME VALUES
// =============================================================================

/// Get delta time
pub static GET_DELTA_TIME: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/delta",
    display_name: "Get Delta Time",
    category: "Time",
    description: "Get time since last frame in seconds",
    create_pins: || vec![
        Pin::output("delta", "Delta", PinType::Float),
        Pin::output("delta_ms", "Delta (ms)", PinType::Float),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

/// Get elapsed time
pub static GET_ELAPSED_TIME: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/elapsed",
    display_name: "Get Elapsed Time",
    category: "Time",
    description: "Get total time since game started",
    create_pins: || vec![
        Pin::output("seconds", "Seconds", PinType::Float),
        Pin::output("milliseconds", "Milliseconds", PinType::Float),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

/// Get unscaled delta time
pub static GET_UNSCALED_DELTA: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/unscaled_delta",
    display_name: "Get Unscaled Delta",
    category: "Time",
    description: "Get delta time unaffected by time scale",
    create_pins: || vec![
        Pin::output("delta", "Delta", PinType::Float),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

/// Get unscaled elapsed time
pub static GET_UNSCALED_ELAPSED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/unscaled_elapsed",
    display_name: "Get Unscaled Elapsed",
    category: "Time",
    description: "Get elapsed time unaffected by time scale",
    create_pins: || vec![
        Pin::output("seconds", "Seconds", PinType::Float),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

/// Get frame count
pub static GET_FRAME_COUNT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/frame_count",
    display_name: "Get Frame Count",
    category: "Time",
    description: "Get the total number of frames since start",
    create_pins: || vec![
        Pin::output("frames", "Frames", PinType::Int),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// TIME SCALE
// =============================================================================

/// Get time scale
pub static GET_TIME_SCALE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/get_scale",
    display_name: "Get Time Scale",
    category: "Time",
    description: "Get the current time scale",
    create_pins: || vec![
        Pin::output("scale", "Scale", PinType::Float),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

/// Set time scale
pub static SET_TIME_SCALE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/set_scale",
    display_name: "Set Time Scale",
    category: "Time",
    description: "Set the time scale (1 = normal, 0 = paused, 2 = double speed)",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// TIMERS
// =============================================================================

/// Create timer
pub static CREATE_TIMER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/create_timer",
    display_name: "Create Timer",
    category: "Time",
    description: "Create a new timer",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("repeat", "Repeat", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("timer", "Timer", PinType::TimerHandle),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

/// Start timer
pub static START_TIMER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/start_timer",
    display_name: "Start Timer",
    category: "Time",
    description: "Start or restart a timer",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("timer", "Timer", PinType::TimerHandle),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

/// Stop timer
pub static STOP_TIMER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/stop_timer",
    display_name: "Stop Timer",
    category: "Time",
    description: "Stop a timer",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("timer", "Timer", PinType::TimerHandle),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

/// Pause timer
pub static PAUSE_TIMER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/pause_timer",
    display_name: "Pause Timer",
    category: "Time",
    description: "Pause a timer",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("timer", "Timer", PinType::TimerHandle),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

/// Resume timer
pub static RESUME_TIMER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/resume_timer",
    display_name: "Resume Timer",
    category: "Time",
    description: "Resume a paused timer",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("timer", "Timer", PinType::TimerHandle),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

/// Reset timer
pub static RESET_TIMER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/reset_timer",
    display_name: "Reset Timer",
    category: "Time",
    description: "Reset a timer to its initial duration",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("timer", "Timer", PinType::TimerHandle),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

/// Get timer progress
pub static GET_TIMER_PROGRESS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/timer_progress",
    display_name: "Get Timer Progress",
    category: "Time",
    description: "Get the progress of a timer (0-1)",
    create_pins: || vec![
        Pin::input("timer", "Timer", PinType::TimerHandle),
        Pin::output("progress", "Progress", PinType::Float),
        Pin::output("remaining", "Remaining", PinType::Float),
        Pin::output("elapsed", "Elapsed", PinType::Float),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

/// Is timer finished
pub static IS_TIMER_FINISHED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/timer_finished",
    display_name: "Is Timer Finished",
    category: "Time",
    description: "Check if a timer has finished",
    create_pins: || vec![
        Pin::input("timer", "Timer", PinType::TimerHandle),
        Pin::output("finished", "Finished", PinType::Bool),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

/// Is timer running
pub static IS_TIMER_RUNNING: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/timer_running",
    display_name: "Is Timer Running",
    category: "Time",
    description: "Check if a timer is currently running",
    create_pins: || vec![
        Pin::input("timer", "Timer", PinType::TimerHandle),
        Pin::output("running", "Running", PinType::Bool),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

/// On timer finished
pub static ON_TIMER_FINISHED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/on_timer_finished",
    display_name: "On Timer Finished",
    category: "Time Events",
    description: "Triggered when a timer finishes",
    create_pins: || vec![
        Pin::input("timer", "Timer", PinType::TimerHandle),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [150, 200, 180],
    is_event: true,
    is_comment: false,
};

// =============================================================================
// DELAYS
// =============================================================================

/// Delay
pub static DELAY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/delay",
    display_name: "Delay",
    category: "Time",
    description: "Wait for a duration before continuing",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

/// Delay frames
pub static DELAY_FRAMES: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/delay_frames",
    display_name: "Delay Frames",
    category: "Time",
    description: "Wait for a number of frames before continuing",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("frames", "Frames", PinType::Int).with_default(PinValue::Int(1)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

/// Wait until
pub static WAIT_UNTIL: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/wait_until",
    display_name: "Wait Until",
    category: "Time",
    description: "Wait until a condition becomes true",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("condition", "Condition", PinType::Bool),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

/// Retriggerable delay
pub static RETRIGGERABLE_DELAY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/retrigger_delay",
    display_name: "Retriggerable Delay",
    category: "Time",
    description: "Delay that restarts when triggered again",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// COOLDOWN
// =============================================================================

/// Cooldown
pub static COOLDOWN: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/cooldown",
    display_name: "Cooldown",
    category: "Time",
    description: "Execute with a cooldown period",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("on_cooldown", "On Cooldown", PinType::Bool),
        Pin::output("remaining", "Remaining", PinType::Float),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

/// Is on cooldown
pub static IS_ON_COOLDOWN: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/is_on_cooldown",
    display_name: "Is On Cooldown",
    category: "Time",
    description: "Check if still on cooldown",
    create_pins: || vec![
        Pin::input("cooldown_id", "Cooldown ID", PinType::String),
        Pin::output("on_cooldown", "On Cooldown", PinType::Bool),
        Pin::output("remaining", "Remaining", PinType::Float),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// PERIODIC
// =============================================================================

/// Every N seconds
pub static EVERY_N_SECONDS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/every_seconds",
    display_name: "Every N Seconds",
    category: "Time",
    description: "Execute periodically every N seconds",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("interval", "Interval", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

/// Every N frames
pub static EVERY_N_FRAMES: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/every_frames",
    display_name: "Every N Frames",
    category: "Time",
    description: "Execute every N frames",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("frames", "Frames", PinType::Int).with_default(PinValue::Int(10)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// REAL TIME
// =============================================================================

/// Get system time
pub static GET_SYSTEM_TIME: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/system_time",
    display_name: "Get System Time",
    category: "Time",
    description: "Get the current system/wall clock time",
    create_pins: || vec![
        Pin::output("hour", "Hour", PinType::Int),
        Pin::output("minute", "Minute", PinType::Int),
        Pin::output("second", "Second", PinType::Int),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

/// Get system date
pub static GET_SYSTEM_DATE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/system_date",
    display_name: "Get System Date",
    category: "Time",
    description: "Get the current system date",
    create_pins: || vec![
        Pin::output("year", "Year", PinType::Int),
        Pin::output("month", "Month", PinType::Int),
        Pin::output("day", "Day", PinType::Int),
        Pin::output("day_of_week", "Day of Week", PinType::Int),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};

/// Get timestamp
pub static GET_TIMESTAMP: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "time/timestamp",
    display_name: "Get Timestamp",
    category: "Time",
    description: "Get Unix timestamp in seconds",
    create_pins: || vec![
        Pin::output("timestamp", "Timestamp", PinType::Float),
    ],
    color: [150, 200, 180],
    is_event: false,
    is_comment: false,
};
