//! Time & Timer API functions for Rhai scripts

use rhai::{Engine, Map, ImmutableString};
use super::super::rhai_commands::RhaiCommand;

/// Register time functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Timers
    // ===================

    engine.register_fn("start_timer", |name: ImmutableString, duration: f64| {
        super::push_command(RhaiCommand::StartTimer { name: name.to_string(), duration: duration as f32, repeat: false });
    });

    engine.register_fn("start_timer_repeating", |name: ImmutableString, duration: f64| {
        super::push_command(RhaiCommand::StartTimer { name: name.to_string(), duration: duration as f32, repeat: true });
    });

    engine.register_fn("stop_timer", |name: ImmutableString| {
        super::push_command(RhaiCommand::StopTimer { name: name.to_string() });
    });

    engine.register_fn("pause_timer", |name: ImmutableString| {
        super::push_command(RhaiCommand::PauseTimer { name: name.to_string() });
    });

    engine.register_fn("resume_timer", |name: ImmutableString| {
        super::push_command(RhaiCommand::ResumeTimer { name: name.to_string() });
    });

    // ===================
    // Timer State Queries
    // ===================

    engine.register_fn("timer_just_finished", |timers_finished: rhai::Array, name: ImmutableString| -> bool {
        timers_finished.iter().any(|t| {
            t.clone().try_cast::<ImmutableString>()
                .map(|n| n.as_str() == name.as_str())
                .unwrap_or(false)
        })
    });

    engine.register_fn("timer_finished", |timers_map: Map, name: ImmutableString| -> bool {
        timers_map.get(name.as_str())
            .and_then(|v| v.clone().try_cast::<bool>())
            .unwrap_or(false)
    });

    engine.register_fn("timer_progress", |timers_map: Map, name: ImmutableString| -> f64 {
        timers_map.get(name.as_str())
            .and_then(|v| v.clone().try_cast::<f64>())
            .unwrap_or(0.0)
    });

    engine.register_fn("timer_remaining", |timers_map: Map, name: ImmutableString| -> f64 {
        timers_map.get(name.as_str())
            .and_then(|v| v.clone().try_cast::<f64>())
            .unwrap_or(0.0)
    });

    // ===================
    // Delay/Wait
    // ===================

    engine.register_fn("delay", |seconds: f64, callback: ImmutableString| {
        super::push_command(RhaiCommand::StartTimer { name: format!("_delay_{}", callback), duration: seconds as f32, repeat: false });
    });

    // ===================
    // Time Utilities
    // ===================

    engine.register_fn("format_time", |seconds: f64| -> String {
        let total_secs = seconds as i64;
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        format!("{:02}:{:02}", mins, secs)
    });

    engine.register_fn("format_time_precise", |seconds: f64| -> String {
        let total_secs = seconds as i64;
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        let millis = ((seconds - seconds.floor()) * 1000.0) as i64;
        format!("{:02}:{:02}.{:03}", mins, secs, millis)
    });
}
