//! Asking to install Rust when a project's scripts need compiling and this
//! machine has none.
//!
//! The splash offers the install first, to everyone. This is the fallback for
//! someone who opened a project without taking it: `renzora_rust_script` holds
//! the project's script builds and sets `scripts_waiting` on
//! [`RustToolchain::Missing`], and this asks, answering with
//! [`RustToolchainAnswer`]. Installing software is the user's decision about
//! their machine, so nothing is fetched until they press Install.
//!
//! Only once scripts are waiting, so a project without any is never asked about
//! a compiler it does not use, and only in the editor proper, so it never lands
//! on top of the splash's own offer.
//!
//! Dismissing (Escape, a backdrop click, the ×) counts as "Not now". Unlike the
//! scene-conflict prompt, which comes back because an unanswered conflict is
//! still a conflict, this one would otherwise reappear on every frame the
//! scripts are held, and a prompt that cannot be closed is worse than none.

use bevy::prelude::*;
use renzora::{RustToolchain, RustToolchainAnswer};
use renzora_ember::font::EmberFonts;
use renzora_ember::UiInteraction;

#[derive(Component)]
pub(crate) struct ToolchainPromptRoot;

#[derive(Component)]
pub(crate) struct ToolchainPromptLater;

#[derive(Component)]
pub(crate) struct ToolchainPromptInstall;

/// Raise the prompt when scripts are waiting on a compiler that is missing.
///
/// `shown` keeps it to once a session: an answer of "Not now" is not reversed
/// by opening a second project, and the splash still offers the install.
pub(crate) fn spawn_prompt(
    state: Res<RustToolchain>,
    fonts: Option<Res<EmberFonts>>,
    mut shown: Local<bool>,
    mut commands: Commands,
) {
    let RustToolchain::Missing { version, rustup_installed, scripts_waiting: true } = &*state
    else {
        return;
    };
    let Some(fonts) = fonts else { return };
    if *shown {
        return;
    }
    *shown = true;

    let body = if *rustup_installed {
        format!(
            "Rust scripts are compiled on your machine and need Rust {version}, \
             which rustup has not installed yet.\n\nInstall adds it through rustup \
             (about 400 MB). Your scripts compile when it finishes."
        )
    } else {
        format!(
            "Rust scripts are compiled on your machine, and Rust is not installed.\n\n\
             Install downloads the official installer from rust-lang.org and sets up \
             Rust {version} (about 400 MB in ~/.cargo and ~/.rustup). Your scripts \
             compile when it finishes."
        )
    };

    // "Install" last, so it takes the accent: every way of dismissing is
    // "Not now", which leaves the machine as it was. See `confirm_dialog`.
    let (root, buttons) = renzora_ember::widgets::confirm_dialog(
        &mut commands,
        &fonts,
        "Install Rust?",
        body,
        460.0,
        220.0,
        &["Not now", "Install"],
    );
    if let [later, install] = buttons[..] {
        commands.entity(later).insert(ToolchainPromptLater);
        commands.entity(install).insert(ToolchainPromptInstall);
    }
    commands.entity(root).insert(ToolchainPromptRoot);
}

/// Answer from the buttons, or "Not now" when the prompt was dismissed.
pub(crate) fn prompt_buttons(
    later: Query<&UiInteraction, (Changed<UiInteraction>, With<ToolchainPromptLater>)>,
    install: Query<&UiInteraction, (Changed<UiInteraction>, With<ToolchainPromptInstall>)>,
    roots: Query<Entity, With<ToolchainPromptRoot>>,
    mut removed: RemovedComponents<ToolchainPromptRoot>,
    state: Res<RustToolchain>,
    mut answers: MessageWriter<RustToolchainAnswer>,
    mut commands: Commands,
) {
    // A button press also despawns the root below, which the removal check sees
    // next frame. That second "Not now" is harmless: the builder only acts on an
    // answer while the state is still `Missing`, and the first answer moved it on.
    let answer = if install.iter().any(|i| *i == UiInteraction::Pressed) {
        Some(RustToolchainAnswer::Install)
    } else if later.iter().any(|i| *i == UiInteraction::Pressed) {
        Some(RustToolchainAnswer::NotNow)
    } else if removed.read().count() > 0 && matches!(*state, RustToolchain::Missing { .. }) {
        // Despawned by ember's generic dismiss, with no button pressed.
        Some(RustToolchainAnswer::NotNow)
    } else {
        None
    };
    let Some(answer) = answer else { return };
    answers.write(answer);
    for root in &roots {
        commands.entity(root).try_despawn();
    }
}
