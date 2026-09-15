//! Checking installed plugins against what the marketplace now publishes.
//!
//! Runs once, shortly after the editor is up, on a worker thread — an update
//! check is not worth a frame of the main thread and is never urgent. The
//! result lands in [`renzora::PluginUpdates`], which is where every surface that
//! draws it reads from: the updater overlay, the store's Updates view, the
//! Settings plugin grid and the exporter's plugin picker. None of those can
//! depend on this crate, which is why the answer goes to the contract crate
//! rather than staying here.
//!
//! The *nudge* is one toast summarising what is available, not a modal: nothing
//! here needs an answer, and a dialog on every start for something the user may
//! not act on for weeks is a tax rather than a service. It is also the only part
//! Settings ▸ Editor ▸ Plugin update reminders silences: turning it off stops
//! the editor volunteering the news, and changes nothing about what the four
//! surfaces say when you go and look.
//!
//! Applying an update is the ordinary install path. That is the point of
//! keying installs on the asset id: reinstalling the same asset replaces its
//! directory, so "update" needs no separate machinery.

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver};

use crate::installed::{self, UpdateState};

/// One plugin with something newer published, as the worker found it.
///
/// Mirrors [`renzora::PluginUpdate`] rather than being it, because the worker
/// thread builds this before there is a `World` to write into and the contract
/// type carries the shape the UI wants rather than the shape the check produces.
struct Found {
    id: String,
    asset_id: String,
    name: String,
    slug: String,
    installed_version: String,
    state: UpdateState,
}

#[derive(Resource, Default)]
pub(crate) struct PluginUpdateCheck {
    /// Started already. One check per session: plugins are not installed often
    /// enough to justify polling, and an install refreshes this itself.
    started: bool,
    rx: Option<Receiver<Vec<Found>>>,
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<PluginUpdateCheck>();
    app.init_resource::<renzora::PluginUpdates>();
    // Read off disk once, here, rather than by the toggle's getter every frame.
    app.insert_resource(renzora::PluginUpdateReminders(
        renzora::core::load_plugin_update_reminders(),
    ));
    app.add_systems(
        Update,
        (start_check, poll_check).run_if(in_state(renzora::SplashState::Editor)),
    );
}

/// Kick the check once, if anything is installed to check.
///
/// The installed list is published straight away, before the network is asked.
/// It is what the store grid needs to say **Installed** on a card, and that
/// answer does not depend on the check succeeding, and an offline editor should
/// still know what it has.
#[cfg(not(target_arch = "wasm32"))]
fn start_check(
    mut state: ResMut<PluginUpdateCheck>,
    mut updates: ResMut<renzora::PluginUpdates>,
) {
    if state.started {
        return;
    }
    state.started = true;

    let installed = installed::scan();
    updates.installed = installed
        .iter()
        .map(|p| renzora::InstalledPluginAsset {
            id: p.dir_name.clone(),
            asset_id: p.asset_id.clone(),
            version: p.version.clone(),
        })
        .collect();
    if installed.is_empty() {
        // Nothing to ask about, and the answer is final: say so, or every
        // surface sits on "still checking" for the life of the session.
        updates.checked = true;
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
                    &found.latest_version,
                    &found.latest_min_engine_version,
                    engine,
                );
                matches!(
                    state,
                    UpdateState::Available { .. } | UpdateState::NeedsNewerEngine { .. }
                )
                .then(|| Found {
                    id: p.dir_name.clone(),
                    asset_id: p.asset_id.clone(),
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
fn start_check(
    mut state: ResMut<PluginUpdateCheck>,
    mut updates: ResMut<renzora::PluginUpdates>,
) {
    state.started = true;
    updates.checked = true;
}

fn poll_check(
    mut state: ResMut<PluginUpdateCheck>,
    mut updates: ResMut<renzora::PluginUpdates>,
    reminders: Res<renzora::PluginUpdateReminders>,
    mut toasts: ResMut<crate::toasts::ToastQueue>,
) {
    let Some(rx) = state.rx.as_ref() else { return };
    let Ok(found) = rx.try_recv() else { return };
    state.rx = None;
    updates.checked = true;

    updates.entries = found
        .iter()
        .map(|f| {
            let (available_version, needs_newer_engine, requires_engine) = match &f.state {
                UpdateState::Available { version } => (version.clone(), false, String::new()),
                UpdateState::NeedsNewerEngine { version, requires } => {
                    (version.clone(), true, requires.clone())
                }
                // Neither reaches here, because the worker filters to the two
                // states above, but a `match` that says so is cheaper than an
                // `unreachable!` in a path that only ever runs once.
                _ => (f.installed_version.clone(), false, String::new()),
            };
            renzora::PluginUpdate {
                id: f.id.clone(),
                asset_id: f.asset_id.clone(),
                slug: f.slug.clone(),
                name: f.name.clone(),
                installed_version: f.installed_version.clone(),
                available_version,
                needs_newer_engine,
                requires_engine,
            }
        })
        .collect();

    if updates.entries.is_empty() {
        return;
    }
    // The only thing the preference silences. Everything above has already
    // happened, so the Updates view and the Settings grid are as informed as
    // they would be either way.
    if !reminders.0 {
        return;
    }

    // One line, whatever the count — a toast per plugin would stack up over
    // something nobody has to act on now. Clicking it opens the Updates view,
    // which is the one place that can act on it.
    let blocked = updates.blocked();
    let ready = updates.ready();
    let first = updates.entries.first().map(|u| u.name.clone()).unwrap_or_default();

    let message = match (ready, blocked) {
        (0, _) => format!(
            "{blocked} plugin update{} need{} a newer editor",
            plural(blocked),
            if blocked == 1 { "s" } else { "" }
        ),
        (_, 0) if ready == 1 => format!("Update available for {first}"),
        (_, 0) => format!("{ready} plugin updates available"),
        _ => format!("{ready} plugin update{} available, {blocked} need a newer editor", plural(ready)),
    };
    toasts.push(crate::toasts::Tone::Info, message, None);
}

/// Bring the published answer back in step after a plugin is installed.
///
/// No network: an install fetches whatever the marketplace last offered, so the
/// copy on disk is current by construction and the entry that sent the user here
/// is spent. Re-asking would be a request to learn nothing.
///
/// The installed list is re-scanned rather than patched, because an install can
/// land in a *new* directory (a name clash disambiguated to `vignette_2`) and
/// that is the one thing this cannot work out from the entry it is retiring.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn record_install(world: &mut World) {
    let installed = installed::scan();
    let Some(mut updates) = world.get_resource_mut::<renzora::PluginUpdates>() else {
        return;
    };
    updates.installed = installed
        .iter()
        .map(|p| renzora::InstalledPluginAsset {
            id: p.dir_name.clone(),
            asset_id: p.asset_id.clone(),
            version: p.version.clone(),
        })
        .collect();
    // Keep only the plugins still behind what was published. Comparing against
    // the version now on disk rather than dropping by asset id covers the
    // blocked case too: installing something else does not resolve an update
    // that is waiting on a newer editor.
    updates.entries.retain(|u| {
        installed
            .iter()
            .find(|p| p.asset_id == u.asset_id)
            .is_none_or(|p| p.version.trim() != u.available_version.trim())
    });
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}
