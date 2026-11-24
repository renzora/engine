//! The game's frame-rate limit: `[window] max_fps`, held to by waiting out the
//! rest of each frame.
//!
//! A different thing from vsync, and the two combine. Vsync makes a frame wait
//! for the monitor, so it allows whatever the display runs at (240 on a 240 Hz
//! screen); this holds the game to a chosen rate on any display, which is what
//! a game built for a steady 30 or 60 needs. With both on, a 60 limit on a
//! 240 Hz monitor gives a smooth 60.
//!
//! # Why it waits at the end of the frame
//!
//! The wait runs in `Last`, after everything the frame does. The window's
//! input is gathered between frames, so it is read after the wait rather than
//! before it, and a limited game shows the newest input it can rather than
//! input that sat through the wait. With pipelined rendering the main world is
//! what sets the pace, so holding it holds the whole frame.
//!
//! # Why it sleeps and then spins
//!
//! `thread::sleep` alone wakes up to about a millisecond late, which at 60 FPS
//! is visible as uneven frames. So it sleeps until shortly before the deadline
//! and spins for the remainder: a millisecond of one core per frame, for frame
//! times that land where they are meant to.
//!
//! Built on nothing but `std` and Bevy's schedule. A crates.io frame limiter is
//! built on its own Bevy, which this workspace must never link twice.
//!
//! Registered only in a game (the shipped one, and the Play child): the
//! editor's own window is never limited, so editing stays responsive whatever
//! the game targets.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use renzora_project_api::CurrentProject;

/// How long before the deadline the sleep ends and the spin takes over.
const SPIN: Duration = Duration::from_millis(1);

pub(crate) fn register(app: &mut App) {
    app.add_systems(Last, hold_frame_rate);
}

/// Wait out the rest of this frame, if a limit is set.
///
/// The limit is read straight from `CurrentProject.config.window.max_fps`
/// every frame, the one place it lives, so the editor, a script and an in-game
/// settings menu all change it the same way and the limiter follows at once.
/// (It was a separate resource seeded once at startup, which made a second
/// copy a script could change and the config could not.)
///
/// `next` is when the frame after this one may start. It advances from the
/// previous deadline rather than from "now", so the rate does not drift by the
/// wake-up error each frame.
fn hold_frame_rate(project: Option<Res<CurrentProject>>, mut next: Local<Option<Instant>>) {
    let max_fps = project.map_or(0, |p| p.config.window.max_fps);
    let Some(period) = frame_period(max_fps) else {
        // No limit: forget the schedule, so turning one on later starts fresh.
        *next = None;
        return;
    };
    let now = Instant::now();
    let Some(deadline) = *next else {
        *next = Some(now + period);
        return;
    };
    wait_until(deadline);
    *next = Some(next_deadline(deadline, Instant::now(), period));
}

/// The time one frame takes at `max_fps`, or `None` for no limit.
fn frame_period(max_fps: u32) -> Option<Duration> {
    (max_fps > 0).then(|| Duration::from_secs_f64(1.0 / max_fps as f64))
}

/// When the next frame may start, given this frame's deadline and when it
/// actually let go.
///
/// One period after the deadline, which keeps an even pace. Unless the frame
/// ran more than a whole period late (a hitch, a loading spike): then from now,
/// so the game does not race through several short frames to catch up.
fn next_deadline(deadline: Instant, released: Instant, period: Duration) -> Instant {
    if released > deadline + period {
        released + period
    } else {
        deadline + period
    }
}

fn wait_until(deadline: Instant) {
    let now = Instant::now();
    if deadline <= now {
        return;
    }
    let left = deadline - now;
    if left > SPIN {
        std::thread::sleep(left - SPIN);
    }
    while Instant::now() < deadline {
        std::hint::spin_loop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_no_limit_and_rates_become_periods() {
        assert_eq!(frame_period(0), None);
        assert_eq!(frame_period(60), Some(Duration::from_secs_f64(1.0 / 60.0)));
        assert_eq!(frame_period(30), Some(Duration::from_secs_f64(1.0 / 30.0)));
    }

    /// On time or a little late: the next frame is one period after the
    /// deadline, so small wake-up errors do not accumulate.
    #[test]
    fn the_pace_is_kept_from_the_deadline() {
        let start = Instant::now();
        let period = Duration::from_millis(16);
        let deadline = start + period;
        let slightly_late = deadline + Duration::from_millis(2);
        assert_eq!(next_deadline(deadline, deadline, period), deadline + period);
        assert_eq!(next_deadline(deadline, slightly_late, period), deadline + period);
    }

    /// More than a whole frame late: start again from now rather than running
    /// short frames to catch up.
    #[test]
    fn a_hitch_restarts_the_pace() {
        let start = Instant::now();
        let period = Duration::from_millis(16);
        let deadline = start + period;
        let hitch = deadline + Duration::from_millis(50);
        assert_eq!(next_deadline(deadline, hitch, period), hitch + period);
    }

    /// The limiter in a running app, driven by the project config: ten frames
    /// at 50 FPS take at least nine periods (the first frame only starts the
    /// schedule).
    #[test]
    fn an_app_is_held_to_the_limit() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        register(&mut app);
        let mut config = renzora_project_api::ProjectConfig::default();
        config.window.max_fps = 50;
        app.insert_resource(CurrentProject { path: std::path::PathBuf::from("."), config });
        let start = Instant::now();
        for _ in 0..10 {
            app.update();
        }
        assert!(start.elapsed() >= Duration::from_millis(9 * 20), "{:?}", start.elapsed());
    }
}
