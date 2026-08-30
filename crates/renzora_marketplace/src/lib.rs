//! The Renzora account — buy assets, manage what you own, and publish your own.
//!
//! Five surfaces, all of them about one thing: an account that exists to
//! purchase and publish assets.
//!
//! * **Marketplace** (`hub_store`) — browse and search the catalogue, preview an
//!   item before buying, and install into the project.
//! * **Library** (`hub_library`) — what you already own, and installing it again.
//! * **Publish** (`asset_uploader`) — becoming a creator *and* uploading, in one
//!   panel. They were two before, and separating them made no sense: the first
//!   thing the uploader had to tell a non-creator was to go open the other one.
//! * **Wallet** (`social_wallet`) — credits, purchases and payouts.
//! * **Account** — a Settings section, plus the sign-in modal.
//!
//! # Where this came from
//!
//! Three crates: `renzora_hub` (the storefront), `renzora_social` (a whole
//! community layer — feed, messages, friends, docs, profiles, teams,
//! notifications and a live WebSocket) and `renzora_auth` (the API client under
//! both). The community layer is **deleted**, not moved. What survived is the
//! commerce half: the store, the library, the uploader, the wallet, creator
//! onboarding, and the parts of the API client those five need.
//!
//! # No `cfg(target_arch = "wasm32")` anywhere
//!
//! All three predecessors carried a wasm arm that hollowed the plugin out,
//! because the whole thing talks to renzora.com over `renzora::net`, whose HTTP
//! client is blocking calls on worker threads — something a browser has neither
//! of. They needed those arms because they were *statically linked*: the
//! generated plugin list had to name their types on every target the editor
//! compiles for, so the type had to exist even where it could do nothing.
//!
//! A native plugin has no such obligation. It is never built for wasm at all, so
//! the arms are gone and the modules below are unconditional.
//!
//! # No `cfg(feature = "editor")` either
//!
//! A native plugin is compiled by a bare `rustc` with no cargo features, so such
//! a gate would be permanently false and the Settings section would silently
//! vanish. The editor contract is always present in the SDK's `renzora`.

use bevy::prelude::*;

pub mod auth;

mod account_settings;
mod avatars;
mod hub_lightbox;
pub mod install;
mod install_overlay;
mod item_overlay;
mod lightbox;
mod material_viewer;
mod model_viewer;
mod native_library;
mod native_store;
mod store_overlay;
mod onboarding;
mod thumbs;
mod toasts;
mod upload_panel;
mod util;
mod wallet;

/// The account + marketplace plugin.
#[derive(Default)]
pub struct MarketplacePlugin;

impl Plugin for MarketplacePlugin {
    fn build(&self, app: &mut App) {
        info!("[marketplace] native plugin");

        // Session, sign-in modal, and the `AuthBridge` the shell's title bar
        // reads. Everything else here needs a session, so this goes first.
        app.add_plugins(auth::AuthPlugin);

        // Relative image and link paths in catalogue markdown (an item's
        // description) resolve against the same server the client talks to.
        app.insert_resource(renzora_ember::widgets::MarkdownBaseUrl(
            auth::client::api_base().to_string(),
        ));

        // Shared caches and overlays: catalogue thumbnails, account avatars,
        // toasts, and the full-size image lightbox.
        app.init_resource::<thumbs::HubThumbs>()
            .init_resource::<avatars::AvatarCache>()
            .init_resource::<toasts::ToastQueue>()
            .init_resource::<toasts::ToastUi>()
            .init_resource::<lightbox::Lightbox>();

        app.add_systems(
            Update,
            (
                thumbs::poll_thumbs,
                avatars::poll_avatars,
                avatars::request_avatars,
                toasts::drain_toasts,
                toasts::toast_clicks,
                // Open after close, so clicking a NEW image while a lightbox is
                // up swaps images instead of the backdrop-close eating the press.
                (lightbox::close_clicks, lightbox::open_clicks).chain(),
                sign_out_cleanup,
            )
                .run_if(in_state(renzora::SplashState::Editor)),
        );

        // The panels.
        app.add_plugins(native_store::NativeHubStore);
        app.add_plugins(native_library::NativeHubLibrary);
        app.add_plugins(upload_panel::UploaderPanel);
        // Creator onboarding: state and systems only. Its UI is Publish's first
        // stage, not a panel of its own — see `onboarding::register`.
        onboarding::register(app);
        wallet::register(app);

        // Offscreen previews for catalogue items: a 3D turntable for models and
        // animations, a live material/shader preview with `@param` controls.
        app.add_plugins(model_viewer::ModelViewerPlugin);
        app.add_plugins(material_viewer::MaterialViewerPlugin);

        // Settings → Account.
        account_settings::register(app);
    }
}

/// When the session ends, clear the account-scoped panel state so the next
/// sign-in re-fetches rather than showing the previous account's library,
/// wallet balance or creator status.
fn sign_out_cleanup(
    session: Res<auth::AuthSession>,
    mut was_signed_in: Local<bool>,
    mut library: ResMut<native_library::HubLibraryData>,
    mut wallet: ResMut<wallet::WalletPanel>,
    mut onboarding: ResMut<onboarding::OnboardingPanel>,
) {
    let signed_in = session.is_signed_in();
    if *was_signed_in && !signed_in {
        *library = Default::default();
        *wallet = Default::default();
        // Creator status is account-scoped — a new sign-in must re-fetch it.
        *onboarding = Default::default();
    }
    *was_signed_in = signed_in;
}

// `add!`, not `plugin!` — this is an in-workspace crate the build generator
// links statically, not a native plugin. `add!` defaults to `Runtime`, so the
// scope has to be spelled out: nothing here runs in a shipped game, because a
// game does not sign in, browse a catalogue, or publish.
renzora::add!(MarketplacePlugin, Editor);
