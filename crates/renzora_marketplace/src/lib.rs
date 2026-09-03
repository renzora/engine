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
//! # The `cfg(target_arch = "wasm32")` arm
//!
//! All three predecessors carried a wasm arm that hollowed the plugin out,
//! because the whole thing talks to renzora.com over `renzora::net`, whose HTTP
//! client is blocking calls on worker threads — something a browser has neither
//! of. They needed those arms because they were *statically linked*: the
//! generated plugin list had to name their types on every target the editor
//! compiles for, so the type had to exist even where it could do nothing.
//!
//! This crate is statically linked too — see the `add!` at the bottom of the
//! file — so the same obligation applies and the arm below is that arm.
//!
//! It was briefly absent. When the marketplace landed as a workspace crate the
//! docs here described it as a *native* plugin, which genuinely has no such
//! obligation because it is never built for wasm at all; on that premise the
//! arms were deleted and the modules made unconditional. The premise was wrong.
//! `renzora_editor` depends on this crate and the editor's wasm bundle
//! (`renzora_editor_app --features wasm`) compiles it like any other target, so
//! the nightly web lane broke on 40 errors — `rfd` has no browser equivalent,
//! `renzora_audio` gates `AudioLink` out of wasm, and every `auth::` API call is
//! native-only blocking HTTP.
//!
//! So: on wasm the modules do not exist and `build` installs nothing. The type
//! survives because the generated list names it on every target, which is the
//! whole reason the arm has to exist rather than the dependency being dropped.
//! Browsing a catalogue you cannot install into, from a build that can neither
//! open a file dialog nor play an audio preview, is not a web feature worth
//! keeping compiling.
//!
//! # No `cfg(feature = "editor")` either
//!
//! A native plugin is compiled by a bare `rustc` with no cargo features, so such
//! a gate would be permanently false and the Settings section would silently
//! vanish. The editor contract is always present in the SDK's `renzora`.

use bevy::prelude::*;

// Every module below is native-only; see the wasm arm in the module docs.
#[cfg(not(target_arch = "wasm32"))]
pub mod auth;

#[cfg(not(target_arch = "wasm32"))]
mod account_settings;
#[cfg(not(target_arch = "wasm32"))]
mod avatars;
#[cfg(not(target_arch = "wasm32"))]
mod hub_lightbox;
#[cfg(not(target_arch = "wasm32"))]
pub mod install;
#[cfg(not(target_arch = "wasm32"))]
mod install_overlay;
#[cfg(not(target_arch = "wasm32"))]
mod item_overlay;
#[cfg(not(target_arch = "wasm32"))]
mod lightbox;
#[cfg(not(target_arch = "wasm32"))]
mod material_viewer;
#[cfg(not(target_arch = "wasm32"))]
mod model_viewer;
#[cfg(not(target_arch = "wasm32"))]
mod library;
#[cfg(not(target_arch = "wasm32"))]
mod store;
#[cfg(not(target_arch = "wasm32"))]
mod store_overlay;
#[cfg(not(target_arch = "wasm32"))]
mod onboarding;
#[cfg(not(target_arch = "wasm32"))]
mod thumbs;
#[cfg(not(target_arch = "wasm32"))]
mod toasts;
#[cfg(not(target_arch = "wasm32"))]
mod docs_panel;
#[cfg(not(target_arch = "wasm32"))]
mod upload_panel;
#[cfg(not(target_arch = "wasm32"))]
mod util;
#[cfg(not(target_arch = "wasm32"))]
mod wallet;

/// The account + marketplace plugin.
#[derive(Default)]
pub struct MarketplacePlugin;

#[cfg(target_arch = "wasm32")]
impl Plugin for MarketplacePlugin {
    /// Nothing to install: there is no account, catalogue or installer on the
    /// web build. The type exists only so the generated Editor plugin list —
    /// which names every plugin on every target — still compiles.
    fn build(&self, _app: &mut App) {}
}

#[cfg(not(target_arch = "wasm32"))]
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
        app.add_plugins(store::StorePanel);
        app.add_plugins(library::LibraryPanel);
        app.add_plugins(upload_panel::UploaderPanel);
        // Docs: the renzora.com portal in a panel. Nothing to do with the
        // marketplace, but it lives here because its API client does.
        docs_panel::register(app);
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
#[cfg(not(target_arch = "wasm32"))]
fn sign_out_cleanup(
    session: Res<auth::AuthSession>,
    mut was_signed_in: Local<bool>,
    mut library: ResMut<library::HubLibraryData>,
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
