//! Hot-reload probe: does state survive, and does exactly one build run?
//!
//! Rebuild this plugin while the editor is running and three things should be
//! visible in the log:
//!
//! 1. **`elapsed` keeps climbing.** It does not reset to zero. Plugin resources
//!    live in the host's ECS, so a reload never touches them — nothing is
//!    serialised or restored, the data simply never left.
//! 2. **`BUILD` changes.** Edit the constant below, rebuild, and the new value
//!    appears. That is the new code actually running.
//! 3. **One line per second, not two.** The previous build's system is still in
//!    Bevy's schedule — it cannot be removed — but its generation is now behind
//!    the slot's counter, so it returns immediately. Two lines per tick would mean
//!    the retirement gate is broken.
//!
//! Deliberately no panel: a panel is registered once at startup by the editor
//! side, so a reloaded one does not re-spawn yet. Systems are the part that
//! reloads today.

use renzora_plugin::prelude::*;

/// Change this, rebuild, and watch the log. Any short string will do — it exists
/// purely to make "is the new code live?" answerable at a glance.
const BUILD: &str = "first";

/// Survives reloads. The host owns the bytes; this plugin only describes them.
#[derive(Resource)]
#[repr(C)]
pub struct Ticker {
    /// Seconds since the plugin was FIRST loaded, across every reload since.
    pub elapsed: f32,
    /// How often to log. A field rather than a constant so the inspector's Plugin
    /// Resources panel can change it live without a rebuild.
    pub interval: f32,
    /// Time left until the next log line. Also reload-surviving, which is why a
    /// swap does not produce an extra tick.
    pub countdown: f32,
}

impl Default for Ticker {
    fn default() -> Self {
        Self {
            elapsed: 0.0,
            interval: 1.0,
            countdown: 1.0,
        }
    }
}

fn tick(mut ticker: ResMut<Ticker>, time: Res<Time>) {
    let dt = time.delta_secs();
    ticker.elapsed += dt;
    ticker.countdown -= dt;
    if ticker.countdown > 0.0 {
        return;
    }
    ticker.countdown = ticker.interval.max(0.05);
    info(&format!(
        "ticker[{BUILD}]: {:.1}s since first load",
        ticker.elapsed
    ));
}

pub struct TickerPlugin;

impl Plugin for TickerPlugin {
    fn build(&self, app: &mut App) {
        // `init_resource` on a reload is a no-op for the DATA: the host already has
        // a `Ticker` under this name and keeps it. Only the schema is re-read.
        app.init_resource::<Ticker>().add_systems(Update, tick);
    }
}

renzora_plugin::add!(TickerPlugin);
