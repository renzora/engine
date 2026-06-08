//! `renzora_tutorial` — the interactive first-run onboarding plugin.
//!
//! An **editor-only** distribution plugin (`renzora::add!(_, Editor)`, shipped as
//! an rlib member of the `renzora_editor` bundle) that teaches engine basics as
//! *hands-on tasks*: the user orbits, zooms and flies the camera, clicks a
//! glowing target mesh to select it, then moves it with the gizmo. Each task
//! completes only when the user **actually performs the action** — detection
//! polls the real editor state (`OrbitCameraState`, `EditorSelection`, the
//! target's `Transform`) — and is rewarded with an ember confetti burst. The
//! card shows an animated mouse/keyboard hint for the gesture each step needs.
//!
//! It launches automatically the first time a project is opened (tracked in
//! `project.toml`'s editor prefs) and can be re-run any time from
//! **Help → Getting Started Tutorial**.
//!
//! Modules: [`steps`] (the task catalog), [`state`] (the state machine +
//! detection), [`overlay_ui`] (the floating card), [`hints`] (animated input
//! hints), [`confetti`] (the celebration), [`demo`] (the target mesh),
//! [`persistence`] (first-run tracking).

use bevy::prelude::*;
use renzora::SplashState;

mod confetti;
mod demo;
mod demo_panel;
mod highlight;
mod hints;
mod overlay_ui;
mod persistence;
mod state;
mod steps;

/// Installs the onboarding tutorial. Editor scope — never ships in exported games.
#[derive(Default)]
pub struct TutorialPlugin;

impl Plugin for TutorialPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] TutorialPlugin (interactive onboarding)");
        demo_panel::register(app);
        app.init_resource::<state::TutorialState>()
            .init_resource::<state::CamInput>()
            .add_systems(
            Update,
            (
                state::probe_cam_input,
                state::trigger,
                state::detect_and_advance,
                state::rebuild_body,
                state::fire_confetti,
                state::handle_buttons,
                confetti::tick,
                hints::tick_hints,
                highlight::update_highlight,
            )
                .chain()
                .run_if(in_state(SplashState::Editor)),
        );
    }
}

renzora::add!(TutorialPlugin, Editor);
