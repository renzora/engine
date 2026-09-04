//! Checking installed plugins against what the marketplace now publishes.
//!
//! Runs once, shortly after the editor is up, on a worker thread — an update
//! check is not worth a frame of the main thread and is never urgent. The
//! result is one toast summarising what is available, not a modal: nothing here
//! needs an answer, and a dialog on every start for something the user may not
//! act on for weeks is a tax rather than a service.
//!
//! Applying an update is the ordinary install path. That is the point of
//! keying installs on the asset id: reinstalling the same asset replaces its
//! directory, so "update" needs no separate machinery.

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver};

use crate::installed::{self, UpdateState};

/// One plugin with something newer published.
///
/// `slug` and `installed_version` are carried for the surface that will list
/// these — the toast only needs the count and a name — and kept here so the
/// check does not have to be re-run to build that list.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AvailableUpdate {
    pub name: String,
    pub slug: String,
    pub installed_version: String,
    pub state: UpdateState,
}

#[derive(Resource, Default)]
pub(crate) struct PluginUpdateCheck {
    /// Started already. One check per session: plugins are not installed often
    /// enough to justify polling, and an install refreshes this itself.
    started: bool,
    rx: Option<Receiver<Vec<AvailableUpdate>>>,
    /// What the last check found, for anything that wants to show it.
    pub(crate) available: Vec<AvailableUpdate>,
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<PluginUpdateCheck>();
    app.add_systems(
        Update,
        (start_check, poll_check).run_if(in_state(renzora::SplashState::Editor)),
    );
}

/// Kick the check once, if anything is installed to check.
#[cfg(not(target_arch = "wasm32"))]
fn start_check(mut state: ResMut<PluginUpdateCheck>) {
    if state.started {
        return;
    }
    state.started = true;

    let installed = installed::scan();
    if installed.is_empty() {
        return;
    }
    let ids: Vec<String> = installed.iter().map(|p| p.asset_id.clone()).collect();

    let (tx, rx) = unbounded();
    state.rx = Some(rx);
    std::thread::spawn(move || {
        let Ok(latest) = crate::auth::marketplace::plugin_updates(&ids) else {
            // A failed check is not worth reporting: the user did not ask, and
            // the editor works either way.
            let _ = tx.send(Vec::new());
            return;
        };
        let engine = renzora::version::ENGINE_VERSION;
        let out = installed
            .iter()
            .filter_map(|p| {
                let found = latest.iter().find(|l| l.id == p.asset_id)?;
                let state = installed::update_state(
                    &p.version,
                    found.published,
                    &found.version,
                    &found.min_engine_version,
                    engine,
                );
                matches!(
                    state,
                    UpdateState::Available { .. } | UpdateState::NeedsNewerEngine { .. }
                )
                .then(|| AvailableUpdate {
                    name: found.name.clone(),
                    slug: found.slug.clone(),
                    installed_version: p.version.clone(),
                    state,
                })
            })
            .collect();
        let _ = tx.send(out);
    });
}

#[cfg(target_arch = "wasm32")]
fn start_check(mut state: ResMut<PluginUpdateCheck>) {
    state.started = true;
}

fn poll_check(
    mut state: ResMut<PluginUpdateCheck>,
    mut toasts: ResMut<crate::toasts::ToastQueue>,
) {
    let Some(rx) = state.rx.as_ref() else { return };
    let Ok(found) = rx.try_recv() else { return };
    state.rx = None;
    if found.is_empty() {
        return;
    }

    // One line, whatever the count — a toast per plugin would stack up over
    // something nobody has to act on now.
    let blocked = found
        .iter()
        .filter(|u| matches!(u.state, UpdateState::NeedsNewerEngine { .. }))
        .count();
    let ready = found.len() - blocked;

    let message = match (ready, blocked) {
        (0, _) => format!(
            "{blocked} plugin update{} need{} a newer editor",
            plural(blocked),
            if blocked == 1 { "s" } else { "" }
        ),
        (_, 0) if ready == 1 => format!("Update available for {}", found[0].name),
        (_, 0) => format!("{ready} plugin updates available"),
        _ => format!("{ready} plugin update{} available, {blocked} need a newer editor", plural(ready)),
    };
    toasts.push(crate::toasts::Tone::Info, message, None);
    state.available = found;
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}
