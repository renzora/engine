//! Time & Timer API functions for Rhai scripts

use rhai::{Dynamic, Engine, Map, ImmutableString};

/// Register time functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Timers
    // ===================

    // start_timer(name, duration) - Start a one-shot timer
    engine.register_fn("start_timer", |name: ImmutableString, duration: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("start_timer"));
        m.insert("name".into(), Dynamic::from(name));
        m.insert("duration".into(), Dynamic::from(duration));
        m.insert("repeat".into(), Dynamic::from(false));
        m
    });

    // start_timer_repeating(name, duration) - Start a repeating timer
    engine.register_fn("start_timer_repeating", |name: ImmutableString, duration: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("start_timer"));
        m.insert("name".into(), Dynamic::from(name));
        m.insert("duration".into(), Dynamic::from(duration));
        m.insert("repeat".into(), Dynamic::from(true));
        m
    });

    // stop_timer(name) - Stop a timer
    engine.register_fn("stop_timer", |name: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("stop_timer"));
        m.insert("name".into(), Dynamic::from(name));
        m
    });

    // pause_timer(name) - Pause a timer
    engine.register_fn("pause_timer", |name: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("pause_timer"));
        m.insert("name".into(), Dynamic::from(name));
        m
    });

    // resume_timer(name) - Resume a paused timer
    engine.register_fn("resume_timer", |name: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("resume_timer"));
        m.insert("name".into(), Dynamic::from(name));
        m
    });

    // ===================
    // Timer State Queries (use scope variables)
    // ===================

    // timer_just_finished(timers_finished, name) - Check if timer just finished this frame
    // timers_finished is an array of timer names that just finished
    engine.register_fn("timer_just_finished", |timers_finished: rhai::Array, name: ImmutableString| -> bool {
        timers_finished.iter().any(|t| {
            t.clone().try_cast::<ImmutableString>()
                .map(|n| n.as_str() == name.as_str())
                .unwrap_or(false)
        })
    });

    // Old map-based API for backwards compatibility
    // timer_finished(timers_finished_map, name) - Check if timer just finished
    engine.register_fn("timer_finished", |timers_map: Map, name: ImmutableString| -> bool {
        timers_map.get(name.as_str())
            .and_then(|v| v.clone().try_cast::<bool>())
            .unwrap_or(false)
    });

    // timer_progress(timers_map, name) - Get timer progress (0.0 to 1.0)
    engine.register_fn("timer_progress", |timers_map: Map, name: ImmutableString| -> f64 {
        timers_map.get(name.as_str())
            .and_then(|v| v.clone().try_cast::<f64>())
            .unwrap_or(0.0)
    });

    // timer_remaining(timers_map, name) - Get remaining time in seconds
    engine.register_fn("timer_remaining", |timers_map: Map, name: ImmutableString| -> f64 {
        timers_map.get(name.as_str())
            .and_then(|v| v.clone().try_cast::<f64>())
            .unwrap_or(0.0)
    });

    // ===================
    // Delay/Wait (returns commands)
    // ===================

    // delay(seconds, callback_name) - Execute callback after delay
    // Note: This queues a timer internally
    engine.register_fn("delay", |seconds: f64, callback: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("start_timer"));
        m.insert("name".into(), Dynamic::from(format!("_delay_{}", callback)));
        m.insert("duration".into(), Dynamic::from(seconds));
        m.insert("repeat".into(), Dynamic::from(false));
        m.insert("callback".into(), Dynamic::from(callback));
        m
    });

    // ===================
    // Time Utilities
    // ===================

    // These are provided via scope variables:
    // - delta: frame delta time
    // - elapsed: total elapsed time
    // - frame: current frame number

    // format_time(seconds) - Format seconds as "MM:SS"
    engine.register_fn("format_time", |seconds: f64| -> String {
        let total_secs = seconds as i64;
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        format!("{:02}:{:02}", mins, secs)
    });

    // format_time_precise(seconds) - Format as "MM:SS.mmm"
    engine.register_fn("format_time_precise", |seconds: f64| -> String {
        let total_secs = seconds as i64;
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        let millis = ((seconds - seconds.floor()) * 1000.0) as i64;
        format!("{:02}:{:02}.{:03}", mins, secs, millis)
    });
}
