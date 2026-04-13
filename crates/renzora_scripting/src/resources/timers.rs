use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct ScriptTimer {
    pub duration: f32,
    pub elapsed: f32,
    pub repeat: bool,
    pub paused: bool,
    pub just_finished: bool,
    pub times_finished: u32,
}

impl ScriptTimer {
    pub fn new(duration: f32, repeat: bool) -> Self {
        Self { duration, elapsed: 0.0, repeat, paused: false, just_finished: false, times_finished: 0 }
    }

    pub fn tick(&mut self, delta: f32) {
        if self.paused { return; }
        self.just_finished = false;
        self.elapsed += delta;
        if self.elapsed >= self.duration {
            self.just_finished = true;
            self.times_finished += 1;
            if self.repeat {
                self.elapsed -= self.duration;
            } else {
                self.elapsed = self.duration;
            }
        }
    }

    pub fn progress(&self) -> f32 {
        (self.elapsed / self.duration).min(1.0)
    }
}

#[derive(Resource, Default)]
pub struct ScriptTimers {
    timers: HashMap<String, ScriptTimer>,
}

impl ScriptTimers {
    pub fn start(&mut self, name: impl Into<String>, duration: f32, repeat: bool) {
        self.timers.insert(name.into(), ScriptTimer::new(duration, repeat));
    }

    pub fn stop(&mut self, name: &str) -> bool {
        self.timers.remove(name).is_some()
    }

    pub fn pause(&mut self, name: &str) {
        if let Some(t) = self.timers.get_mut(name) { t.paused = true; }
    }

    pub fn resume(&mut self, name: &str) {
        if let Some(t) = self.timers.get_mut(name) { t.paused = false; }
    }

    pub fn tick_all(&mut self, delta: f32) {
        for timer in self.timers.values_mut() { timer.tick(delta); }
    }

    pub fn get_just_finished(&self) -> Vec<String> {
        self.timers.iter()
            .filter(|(_, t)| t.just_finished)
            .map(|(n, _)| n.clone())
            .collect()
    }

    pub fn clear(&mut self) {
        self.timers.clear();
    }
}

/// System to tick all timers each frame
pub fn update_script_timers(time: Res<Time>, mut timers: ResMut<ScriptTimers>) {
    timers.tick_all(time.delta_secs());
}
