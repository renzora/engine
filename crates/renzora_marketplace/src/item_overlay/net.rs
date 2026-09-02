//! The overlay's network calls — blocking requests on worker threads, each
//! posting its result over a `crossbeam_channel` that [`super::systems::poll_item`]
//! drains. There is no async runtime here.
//!
//! The browser build has no blocking HTTP path, so the wasm half of this file
//! is a set of no-ops: the overlay opens but stays inert.

use crate::auth::marketplace::{AssetComment, AssetRating};
use crate::auth::session::AuthSession;

use super::ItemOverlay;
#[cfg(not(target_arch = "wasm32"))]
use crossbeam_channel::unbounded;
#[cfg(not(target_arch = "wasm32"))]
use super::clone_session;

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn fetch_comments(state: &mut ItemOverlay, asset_id: &str) {
    let (tx, rx) = unbounded();
    state.comments_rx = Some(rx);
    state.comments_loading = true;
    let asset_id = asset_id.to_string();
    std::thread::spawn(move || {
        let _ = tx.send(crate::auth::marketplace::get_comments(&asset_id));
    });
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn fetch_rating(state: &mut ItemOverlay, asset_id: &str) {
    let (tx, rx) = unbounded();
    state.rating_rx = Some(rx);
    let asset_id = asset_id.to_string();
    let session = state.session.as_ref().map(clone_session);
    std::thread::spawn(move || {
        let _ = tx.send(crate::auth::marketplace::get_rating(&asset_id, session.as_ref()));
    });
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn spawn_post_comment(
    session: AuthSession,
    asset_id: String,
    content: String,
    tx: crossbeam_channel::Sender<Result<AssetComment, String>>,
) {
    std::thread::spawn(move || {
        let _ = tx.send(crate::auth::marketplace::post_comment(&session, &asset_id, &content));
    });
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn spawn_post_rating(
    session: AuthSession,
    asset_id: String,
    rating: i32,
    tx: crossbeam_channel::Sender<Result<AssetRating, String>>,
) {
    std::thread::spawn(move || {
        let _ = tx.send(crate::auth::marketplace::post_rating(&session, &asset_id, rating));
    });
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn fetch_media(state: &mut ItemOverlay, asset_id: &str) {
    let (tx, rx) = unbounded();
    state.media_rx = Some(rx);
    let asset_id = asset_id.to_string();
    std::thread::spawn(move || {
        let _ = tx.send(crate::auth::marketplace::get_media(&asset_id));
    });
}

#[cfg(target_arch = "wasm32")]
pub(super) fn fetch_comments(state: &mut ItemOverlay, _slug: &str) {
    state.comments_loading = false;
}
#[cfg(target_arch = "wasm32")]
pub(super) fn fetch_rating(_state: &mut ItemOverlay, _slug: &str) {}
#[cfg(target_arch = "wasm32")]
pub(super) fn spawn_post_comment(
    _session: AuthSession,
    _slug: String,
    _content: String,
    _tx: crossbeam_channel::Sender<Result<AssetComment, String>>,
) {
}
#[cfg(target_arch = "wasm32")]
pub(super) fn spawn_post_rating(
    _session: AuthSession,
    _slug: String,
    _rating: i32,
    _tx: crossbeam_channel::Sender<Result<AssetRating, String>>,
) {
}
#[cfg(target_arch = "wasm32")]
pub(super) fn fetch_media(_state: &mut ItemOverlay, _asset_id: &str) {}
