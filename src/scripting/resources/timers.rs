//! Script timer system
//!
//! Allows scripts to create named timers that can trigger callbacks.

use bevy::prelude::*;
use std::collections::HashMap;

/// State of a single timer
#[derive(Clone, Debug)]
pub struct ScriptTimer {
    /// Total duration of the timer
    pub duration: f32,
    /// Current elapsed time
    pub elapsed: f32,
    /// Whether the timer repeats after finishing
    pub repeat: bool,
    /// Whether the timer is currently paused
    pub paused: bool,
    /// True on the frame the timer finishes
    pub just_finished: bool,
    /// Number of times this timer has finished
    pub times_finished: u32,
}

impl ScriptTimer {
    pub fn new(duration: f32, repeat: bool) -> Self {
        Self {
            duration,
            elapsed: 0.0,
            repeat,
            paused: false,
            just_finished: false,
            times_finished: 0,
        }
    }

    /// Tick the timer by delta time
    pub fn tick(&mut self, delta: f32) {
        if self.paused {
            return;
        }

        // Clear just_finished flag from previous frame
        self.just_finished = false;

        self.elapsed += delta;

        if self.elapsed >= self.duration {
            self.just_finished = true;
            self.times_finished += 1;

            if self.repeat {
                // Carry over excess time
                self.elapsed -= self.duration;
            } else {
                self.elapsed = self.duration;
            }
        }
    }

    /// Check if timer has finished (at least once)
    pub fn finished(&self) -> bool {
        self.times_finished > 0
    }

    /// Get progress as a value from 0.0 to 1.0
    pub fn progress(&self) -> f32 {
        (self.elapsed / self.duration).min(1.0)
    }

    /// Reset the timer
    pub fn reset(&mut self) {
        self.elapsed = 0.0;
        self.just_finished = false;
    }
}

/// Resource that stores all script timers
#[derive(Resource, Default)]
pub struct ScriptTimers {
    /// Map of timer name -> timer state
    timers: HashMap<String, ScriptTimer>,
}

impl ScriptTimers {
    /// Create or replace a timer
    pub fn start(&mut self, name: impl Into<String>, duration: f32, repeat: bool) {
        let name = name.into();
        self.timers.insert(name, ScriptTimer::new(duration, repeat));
    }

    /// Stop and remove a timer
    pub fn stop(&mut self, name: &str) -> bool {
        self.timers.remove(name).is_some()
    }

    /// Pause a timer
    pub fn pause(&mut self, name: &str) -> bool {
        if let Some(timer) = self.timers.get_mut(name) {
            timer.paused = true;
            true
        } else {
            false
        }
    }

    /// Resume a paused timer
    pub fn resume(&mut self, name: &str) -> bool {
        if let Some(timer) = self.timers.get_mut(name) {
            timer.paused = false;
            true
        } else {
            false
        }
    }

    /// Check if a timer exists
    pub fn has(&self, name: &str) -> bool {
        self.timers.contains_key(name)
    }

    /// Get a timer by name
    pub fn get(&self, name: &str) -> Option<&ScriptTimer> {
        self.timers.get(name)
    }

    /// Get a mutable reference to a timer
    pub fn get_mut(&mut self, name: &str) -> Option<&mut ScriptTimer> {
        self.timers.get_mut(name)
    }

    /// Check if a timer just finished this frame
    pub fn just_finished(&self, name: &str) -> bool {
        self.timers.get(name).map_or(false, |t| t.just_finished)
    }

    /// Tick all timers
    pub fn tick_all(&mut self, delta: f32) {
        for timer in self.timers.values_mut() {
            timer.tick(delta);
        }
    }

    /// Get all timers that just finished (for providing to scripts)
    pub fn get_just_finished(&self) -> Vec<String> {
        self.timers
            .iter()
            .filter_map(|(name, timer)| {
                if timer.just_finished {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Clear all timers (on play mode stop)
    pub fn clear(&mut self) {
        self.timers.clear();
    }
}
