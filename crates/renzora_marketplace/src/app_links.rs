//! `renzora://marketplace/<slug>` links: the website's download buttons, when
//! the engine is installed, open the listing here instead of downloading it.
//!
//! The link itself arrives through `renzora_platform_api::app_link` (from the
//! command line, or forwarded by a later launch). This module turns its slug
//! into a listing and decides where to show it:
//!
//! * **A project is open:** the Marketplace, with the item's overlay on top.
//! * **The launcher, and the item is a plugin:** the launcher's Plugins page,
//!   searched for it. A plugin installs into the engine rather than a project,
//!   and that page is where the launcher does it.
//! * **The launcher, anything else:** it waits until a project is open, since
//!   an asset installs into one and there is nowhere to put it yet.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use crossbeam_channel::{unbounded, Receiver, Sender};

use renzora_platform_api::app_link::{self, AppLink};
use renzora_project_api::SplashState;

use crate::auth::marketplace::{AssetDetail, AssetSummary};
use crate::toasts::{ToastQueue, Tone};

type Fetched = (String, Result<AssetDetail, String>);

#[derive(Resource)]
struct LinkState {
    tx: Sender<Fetched>,
    rx: Receiver<Fetched>,
    /// Fetches started and not yet answered.
    in_flight: usize,
    /// A listing fetched and waiting for somewhere to show it.
    waiting: Option<AssetSummary>,
}

impl Default for LinkState {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self { tx, rx, in_flight: 0, waiting: None }
    }
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<LinkState>();
    app.add_systems(
        Update,
        (
            take_links.run_if(app_link::pending),
            poll_fetches.run_if(|s: Res<LinkState>| s.in_flight > 0),
            show_waiting.run_if(|s: Res<LinkState>| s.waiting.is_some()),
        )
            .chain(),
    );
}

/// Start fetching each new link's listing, and bring the window forward when
/// the link came from another launch.
fn take_links(
    mut state: ResMut<LinkState>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if app_link::take_focus_request() {
        if let Ok(mut window) = windows.single_mut() {
            window.set_minimized(false);
            window.focused = true;
        }
    }
    for link in app_link::take() {
        match link {
            AppLink::MarketplaceItem { slug } => {
                state.in_flight += 1;
                let tx = state.tx.clone();
                std::thread::spawn(move || {
                    let result =
                        crate::auth::marketplace::get_asset(&app_link::percent_encode(&slug));
                    let _ = tx.send((slug, result));
                });
            }
        }
    }
}

fn poll_fetches(
    mut state: ResMut<LinkState>,
    splash: Res<State<SplashState>>,
    mut toasts: ResMut<ToastQueue>,
    mut commands: Commands,
) {
    while let Ok((slug, result)) = state.rx.try_recv() {
        state.in_flight = state.in_flight.saturating_sub(1);
        match result {
            // The newest link wins: two clicks in a row mean the second item.
            Ok(detail) => state.waiting = Some(summary(detail)),
            Err(e) => {
                let message = format!("Couldn't open \"{slug}\" from the website: {e}");
                warn!("[link] {message}");
                if *splash.get() == SplashState::Splash {
                    commands.queue(move |world: &mut World| {
                        crate::splash_plugins::show_from_link(world, Err(message))
                    });
                } else {
                    toasts.push(Tone::Error, message, None);
                }
            }
        }
    }
}

/// Show the waiting listing wherever the app now is; see the module doc.
fn show_waiting(
    mut state: ResMut<LinkState>,
    splash: Res<State<SplashState>>,
    store_open: Query<(), With<crate::store_overlay::StoreOverlayRoot>>,
    mut commands: Commands,
) {
    match splash.get() {
        SplashState::Editor => {
            let Some(asset) = state.waiting.take() else {
                return;
            };
            let open_store = store_open.is_empty();
            commands.queue(move |world: &mut World| {
                if open_store {
                    renzora_shell_api::ShellActionInvoked::invoke(world, crate::store_overlay::ACTION_ID);
                }
                crate::item_overlay::open::open(world, asset);
            });
        }
        SplashState::Splash => {
            let is_plugin = state
                .waiting
                .as_ref()
                .is_some_and(|a| crate::install::is_plugin_category(&a.category));
            if !is_plugin {
                return;
            }
            let Some(asset) = state.waiting.take() else {
                return;
            };
            commands.queue(move |world: &mut World| {
                crate::splash_plugins::show_from_link(world, Ok(asset.name))
            });
        }
        // A project is on its way; show it when it is open.
        SplashState::Loading => {}
    }
}

fn summary(d: AssetDetail) -> AssetSummary {
    AssetSummary {
        id: d.id,
        name: d.name,
        slug: d.slug,
        description: d.description,
        category: d.category,
        price_credits: d.price_credits,
        thumbnail_url: d.thumbnail_url,
        version: d.version,
        downloads: d.downloads,
        creator_name: d.creator_name.unwrap_or_default(),
    }
}
