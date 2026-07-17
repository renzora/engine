//! On-demand OpenXR session control for in-editor VR play.
//!
//! The OpenXR *device* must exist from boot (it's the wgpu device everything
//! renders with), but the *session* — what actually lights up the headset —
//! can start and stop freely. In game mode (`--vr`) `XrSessionPlugin`'s
//! auto-handler starts it immediately; in an XR-capable editor session this
//! module drives it from the [`renzora::VrPlayState`] contract instead:
//! the viewport play-mode code flips `requested` when the "VR Headset" play
//! target starts/stops, and the session follows.

use bevy::prelude::*;
use bevy_mod_xr::session::{
    XrBeginSessionMessage, XrCreateSessionMessage, XrDestroySessionMessage, XrEndSessionMessage,
    XrRequestExitMessage, XrState,
};

pub(crate) fn register(app: &mut App, auto_start: bool) {
    app.init_resource::<renzora::VrPlayState>();
    if auto_start {
        // Game mode: the auto-handler owns the lifecycle; just mirror state
        // (and keep `requested` truthful for anything reading the contract).
        app.add_systems(Update, mirror_state_game_mode);
    } else {
        app.add_systems(Update, drive_session_from_play_state);
    }
}

fn mirror_state_game_mode(state: Res<XrState>, mut play: ResMut<renzora::VrPlayState>) {
    play.requested = true;
    play.active = matches!(*state, XrState::Running);
}

/// Editor mode: walk the session state machine toward "running" while the
/// editor requests VR play, and back down when it stops. Mirrors
/// `auto_handle_session`'s transitions but gates session *creation* on the
/// request. Messages may repeat for a frame or two while a transition is in
/// flight — the backend ignores messages that don't apply to the current
/// state, so repeats are harmless.
fn drive_session_from_play_state(
    mut play: ResMut<renzora::VrPlayState>,
    state: Res<XrState>,
    mut create: MessageWriter<XrCreateSessionMessage>,
    mut begin: MessageWriter<XrBeginSessionMessage>,
    mut end: MessageWriter<XrEndSessionMessage>,
    mut destroy: MessageWriter<XrDestroySessionMessage>,
    mut exit: MessageWriter<XrRequestExitMessage>,
) {
    let active = matches!(*state, XrState::Running);
    if play.active != active {
        play.active = active;
    }

    match (*state, play.requested) {
        (XrState::Available, true) => {
            create.write_default();
        }
        // Begin as soon as the runtime is ready; end/destroy on the way down
        // regardless of `requested` (the runtime can also initiate an exit,
        // e.g. the user closes the session from the headset).
        (XrState::Ready, _) => {
            begin.write_default();
        }
        (XrState::Running, false) => {
            exit.write_default();
        }
        (XrState::Stopping, _) => {
            end.write_default();
        }
        (XrState::Exiting { .. }, _) => {
            destroy.write_default();
        }
        _ => {}
    }
}
