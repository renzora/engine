//! `renzora_autosave` — periodic scene auto-save.
//!
//! An **editor-only** plugin (`renzora::add!(_, Editor)`, shipped as an rlib
//! member of the `renzora_editor` bundle) that re-saves the open scene at a
//! user-set interval so an editor crash never costs more than that interval's
//! worth of work.
//!
//! It deliberately reuses the **exact Ctrl+S path**: when the countdown reaches
//! zero it inserts [`renzora::SaveSceneRequested`], which `renzora_scene`'s
//! `save_scene_system` consumes — so auto-save inherits all of that path's
//! safety (it won't write scene RON over a focused `.material`/asset tab, it
//! redirects an unsaved scene to Save-As, etc.).
//!
//! The countdown is surfaced in the status bar by writing
//! [`renzora::ShellReadyStatus`]: while counting it replaces the left-hand
//! "Ready" label with `Auto save in Ns`; on reaching zero it saves and lets the
//! label fall back to "Ready" for a short beat before the next cycle begins.
//!
//! Everything it touches is a `renzora` contract type, so the dylib is its only
//! dependency.

use bevy::prelude::*;

/// Status-bar accent for the countdown label (a soft amber = "save pending").
const ACCENT: [u8; 3] = [235, 185, 95];

/// Only surface the countdown in the status bar for this many seconds before the
/// save. The rest of the interval the bar just reads "Ready" — the timer is a
/// quiet background thing, the "Auto save in Ns" is only a heads-up that a write
/// is imminent.
const COUNTDOWN_VISIBLE: f32 = 5.0;

/// After the countdown hits zero (and a save fires) the label falls back to
/// "Ready" for this many seconds — the visible "back to Ready" beat the user
/// asked for — before the next interval starts counting down.
const READY_HOLD: f32 = 2.0;

/// Runtime countdown state (distinct from the persisted [`renzora::AutoSaveSettings`]).
#[derive(Resource)]
struct AutoSaveCountdown {
    /// Seconds until the next save. Counts down through zero into `-READY_HOLD`
    /// (the "Ready" beat), then resets to the interval.
    remaining: f32,
    /// True once this cycle's save has been requested, so we fire it exactly once
    /// per cycle even though `remaining` lingers below zero during the beat.
    saved: bool,
}

impl Default for AutoSaveCountdown {
    fn default() -> Self {
        // Seeded high; the idle branch reseeds it to the live interval every
        // frame until a project is open, so launch never triggers an instant save.
        Self {
            remaining: f32::MAX,
            saved: false,
        }
    }
}

#[derive(Default)]
pub struct AutoSavePlugin;

impl Plugin for AutoSavePlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] AutoSavePlugin");
        app.insert_resource(renzora::load_autosave());
        app.init_resource::<AutoSaveCountdown>();
        app.init_resource::<renzora::ShellReadyStatus>();
        app.add_systems(
            Update,
            tick.run_if(in_state(renzora::SplashState::Editor)),
        );
    }
}

fn tick(
    time: Res<Time>,
    cfg: Option<Res<renzora::AutoSaveSettings>>,
    project: Option<Res<renzora::CurrentProject>>,
    play: Option<Res<renzora::PlayModeState>>,
    mut cd: ResMut<AutoSaveCountdown>,
    mut ready: ResMut<renzora::ShellReadyStatus>,
    mut commands: Commands,
) {
    let cfg = cfg.map(|c| *c).unwrap_or_default();
    let interval = cfg.interval_secs.max(1) as f32;
    let in_play = play.as_deref().is_some_and(|p| p.is_in_play_mode());

    // Idle: disabled, no project, or playing → park the countdown and hand the
    // "Ready" label back to the host.
    if !cfg.enabled || project.is_none() || in_play {
        cd.remaining = interval;
        cd.saved = false;
        if ready.label.is_some() {
            ready.label = None;
            ready.color = None;
        }
        return;
    }

    // A live interval change (settings edit) shouldn't leave us counting down
    // from a now-larger value.
    if cd.remaining > interval {
        cd.remaining = interval;
    }

    cd.remaining -= time.delta_secs();

    if cd.remaining <= 0.0 && !cd.saved {
        cd.saved = true;
        commands.insert_resource(renzora::SaveSceneRequested);
    }

    // Past zero: hold the "Ready" beat, then restart the cycle.
    if cd.remaining <= -READY_HOLD {
        cd.remaining = interval;
        cd.saved = false;
    }

    // Show the countdown only in the final few seconds; otherwise (and during
    // the post-save beat) the bar reads the default "Ready".
    if cd.remaining > 0.0 && cd.remaining <= COUNTDOWN_VISIBLE {
        let secs = cd.remaining.ceil() as i64;
        ready.label = Some(format!("Auto save in {secs}s"));
        ready.color = Some(ACCENT);
    } else if ready.label.is_some() {
        ready.label = None;
        ready.color = None;
    }
}

renzora::add!(AutoSavePlugin, Editor);
