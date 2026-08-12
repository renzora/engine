//! `renzora_tutorial` — the interactive first-run onboarding plugin.
//!
//! An **editor-only** distribution plugin (`renzora::add!(_, Editor)`, shipped as
//! an rlib member of the `renzora_editor` bundle) that teaches the engine as a
//! set of short, independent **chapters** — Getting Started, Building a Scene,
//! Scripting, Materials, Your Workspace, The Marketplace, Play Mode — each a
//! run of *hands-on tasks*. A task is satisfied by the user **actually
//! performing the action**: detection polls real editor state (`OrbitCameraState`,
//! `EditorSelection`, the dock tree, `PlayModeState`, files on disk) and is
//! rewarded with an ember confetti burst. The card shows an animated
//! mouse/keyboard hint for the gesture each step needs.
//!
//! Doing the action **arms** the step rather than advancing it: a green Continue
//! button appears and the user moves on when they're ready. That's also what
//! makes it possible to teach the parts of the editor that expose no observable
//! state (a marketplace purchase, a node dropped in the material graph) —
//! `StepKind::Info` steps have nothing to detect and offer Continue immediately,
//! instead of us pretending to detect them or reaching into another crate's
//! private state.
//!
//! The card is draggable by its whole header (ember's `DragHandle` widget),
//! because a step's target is sometimes exactly where the card is parked.
//!
//! A project's first run drops straight into Getting Started; **Help → Getting
//! Started Tutorial** re-opens at the chapter picker. Progress is tracked in
//! `project.toml`'s editor prefs.
//!
//! Modules: [`steps`] (step/chapter vocabulary), [`chapters`] (the content),
//! [`state`] (the state machine + detection), [`overlay_ui`] (the floating card,
//! picker and per-step body), [`hints`] (animated input hints), [`highlight`]
//! (the glow box + pointer arrow), [`confetti`] (the celebration), [`demo`] (the
//! target mesh), [`persistence`] (first-run + per-chapter tracking).

use bevy::prelude::*;
use renzora::SplashState;

mod chapters;
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
                // Detection only arms the step; Continue does the advancing.
                // Both run before `rebuild_body` so the card is rebuilt once,
                // against the settled state, rather than twice or a frame late.
                state::detect_step_done,
                state::handle_continue,
                state::handle_chapter_pick,
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
