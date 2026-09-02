//! Close, install, rate, post, select — plus the one system that drains every
//! network channel and applies the result.

use bevy::prelude::*;
use crossbeam_channel::{unbounded, TryRecvError};

use crate::auth::session::AuthSession;
use crate::thumbs::HubThumbs;
use renzora_ember::widgets::EmberTextInput;

#[cfg(not(target_arch = "wasm32"))]
use renzora_audio::AudioLink;

use super::net::{fetch_comments, spawn_post_comment, spawn_post_rating};
use super::{
    clone_session, AudioTrackBtn, ItemBackdrop, ItemCloseBtn, ItemCommentInput, ItemInstallBtn,
    ItemOverlay, ItemPostBtn, MainImageBtn, StarBtn, StripThumbBtn, VideoBtn,
};

/// Close on a backdrop press, the X, or Escape. `try_despawn` because a layout
/// rebuild may already have torn the overlay down.
pub(super) fn item_close(
    keys: Res<ButtonInput<KeyCode>>,
    backdrop: Query<&Interaction, (With<ItemBackdrop>, Changed<Interaction>)>,
    close_btn: Query<&Interaction, (With<ItemCloseBtn>, Changed<Interaction>)>,
    lightbox: Res<crate::hub_lightbox::HubLightbox>,
    mut state: ResMut<ItemOverlay>,
    mut link: Option<ResMut<AudioLink>>,
    mut commands: Commands,
) {
    let Some(root) = state.root else {
        return;
    };
    let pressed = backdrop.iter().chain(close_btn.iter()).any(|i| *i == Interaction::Pressed);
    // When the full-image lightbox is up (stacked above), let Escape close *it*
    // first rather than tearing down the whole detail overlay underneath.
    let escape = keys.just_pressed(KeyCode::Escape) && lightbox.root.is_none();
    if pressed || escape {
        // Stop the clip first — resetting the resource drops the voice id, and a
        // dropped id keeps playing.
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(link) = link.as_mut() {
            super::audio::stop_audio_inner(&mut state.audio, link);
        }
        commands.entity(root).try_despawn();
        *state = ItemOverlay::default();
        // Despawn the 3D preview model + idle its camera so a closed overlay
        // costs nothing and never leaks a spinning scene.
        commands.queue(|world: &mut World| crate::model_viewer::close_model_preview(world));
        commands.queue(|world: &mut World| crate::material_viewer::close_material_preview(world));
    }
}

/// Overlay Install/Get → open the shared install confirm overlay (stacked above
/// this one). A paid asset for a signed-out user opens sign-in first, matching
/// the store card behavior.
pub(super) fn item_install_click(
    q: Query<&Interaction, (With<ItemInstallBtn>, Changed<Interaction>)>,
    state: Res<ItemOverlay>,
    session: Option<Res<AuthSession>>,
    mut commands: Commands,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Some(asset) = state.asset.clone() else {
        return;
    };
    let signed = session.as_ref().map(|s| s.is_signed_in()).unwrap_or(false);
    if !signed && asset.price_credits > 0 {
        commands.insert_resource(renzora::core::AuthToggleWindowRequest);
        return;
    }
    commands.queue(move |world: &mut World| crate::install_overlay::open(world, asset));
}

/// Click a star → submit that rating (1-5). No-op when signed out or a rating
/// post is already in flight.
pub(super) fn item_star_click(
    q: Query<(&Interaction, &StarBtn), Changed<Interaction>>,
    mut state: ResMut<ItemOverlay>,
) {
    if state.post_rating_rx.is_some() {
        return;
    }
    let Some(session) = state.session.as_ref().map(clone_session) else {
        return;
    };
    for (interaction, star) in &q {
        if *interaction == Interaction::Pressed {
            let rating = star.0;
            let asset_id = state.asset_id.clone();
            let (tx, rx) = unbounded();
            state.post_rating_rx = Some(rx);
            spawn_post_rating(session, asset_id, rating, tx);
            break;
        }
    }
}

/// "Post" → submit the composer's text as a comment. Debounced via `posting`.
pub(super) fn item_post_click(
    q: Query<&Interaction, (With<ItemPostBtn>, Changed<Interaction>)>,
    input: Query<&EmberTextInput, With<ItemCommentInput>>,
    mut state: ResMut<ItemOverlay>,
) {
    if state.posting {
        return;
    }
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Some(session) = state.session.as_ref().map(clone_session) else {
        return;
    };
    let Some(content) = input.iter().next().map(|i| i.value.trim().to_string()) else {
        return;
    };
    if content.is_empty() {
        return;
    }
    // Posting is keyed by asset id (same endpoint as the comments list).
    let asset_id = state.asset_id.clone();
    let (tx, rx) = unbounded();
    state.post_comment_rx = Some(rx);
    state.posting = true;
    spawn_post_comment(session, asset_id, content, tx);
}

/// Click a strip thumbnail → make it the main viewer's selected image.
pub(super) fn strip_thumb_click(
    q: Query<(&Interaction, &StripThumbBtn), Changed<Interaction>>,
    mut state: ResMut<ItemOverlay>,
) {
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            state.selected_media = btn.0;
            break;
        }
    }
}

/// Click the big preview image → open it full-size in the lightbox (like a feed
/// image), stacked above the item overlay.
pub(super) fn main_image_click(
    q: Query<&Interaction, (With<MainImageBtn>, Changed<Interaction>)>,
    state: Res<ItemOverlay>,
    mut commands: Commands,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    // What the viewer currently shows: the selected gallery image, else the
    // asset's own thumbnail.
    let imgs: Vec<String> = state
        .media
        .iter()
        .filter(|m| m.media_type == "image")
        .map(|m| m.url.clone())
        .collect();
    let url = imgs
        .get(state.selected_media)
        .or_else(|| imgs.first())
        .cloned()
        .or_else(|| state.asset.as_ref().and_then(|a| a.thumbnail_url.clone()));
    if let Some(url) = url {
        commands.queue(move |world: &mut World| crate::hub_lightbox::open(world, url));
    }
}

/// Click a video poster → open its URL in the browser (native video is out of
/// scope). `open_url` handles YouTube + direct links.
pub(super) fn video_thumb_click(q: Query<(&Interaction, &VideoBtn), Changed<Interaction>>) {
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            crate::store::open_url(&btn.0);
            break;
        }
    }
}

/// Click a track-selector row → switch the audio player to that track. Only the
/// index changes here; [`super::audio::sync_audio`] notices the divergence and
/// stops/reloads.
pub(super) fn audio_track_click(
    q: Query<(&Interaction, &AudioTrackBtn), Changed<Interaction>>,
    mut state: ResMut<ItemOverlay>,
) {
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            state.audio_selected = btn.0;
            break;
        }
    }
}

/// Drain the four network channels: apply comments/rating, and on a successful
/// post re-fetch the affected data (clearing the composer input on a comment).
pub(super) fn poll_item(
    mut state: ResMut<ItemOverlay>,
    mut input: Query<&mut EmberTextInput, With<ItemCommentInput>>,
    mut thumbs: ResMut<HubThumbs>,
) {
    // Preview media gallery → store it and request every image/video thumbnail.
    if let Some(rx) = state.media_rx.take() {
        match rx.try_recv() {
            Ok(res) => match res {
                Ok(media) => {
                    for m in &media {
                        match m.media_type.as_str() {
                            "image" => thumbs.request(&m.url),
                            "video" => {
                                if let Some(t) = &m.thumbnail_url {
                                    thumbs.request(t);
                                }
                            }
                            _ => {}
                        }
                    }
                    state.media = media;
                    state.selected_media = 0;
                    state.audio_selected = 0;
                    state.error = None;
                }
                Err(e) => state.error = Some(e),
            },
            Err(TryRecvError::Empty) => state.media_rx = Some(rx),
            Err(TryRecvError::Disconnected) => {}
        }
    }
    // Initial (or refreshed) comments.
    if let Some(rx) = state.comments_rx.take() {
        match rx.try_recv() {
            Ok(res) => {
                state.comments_loading = false;
                match res {
                    Ok(r) => {
                        state.comments = r.comments;
                        state.error = None;
                    }
                    Err(e) => state.error = Some(e),
                }
            }
            Err(TryRecvError::Empty) => state.comments_rx = Some(rx),
            Err(TryRecvError::Disconnected) => state.comments_loading = false,
        }
    }
    // Rating aggregate.
    if let Some(rx) = state.rating_rx.take() {
        match rx.try_recv() {
            Ok(Ok(r)) => state.rating = Some(r),
            Ok(Err(e)) => state.error = Some(e),
            Err(TryRecvError::Empty) => state.rating_rx = Some(rx),
            Err(TryRecvError::Disconnected) => {}
        }
    }
    // Comment post acknowledged → clear input + re-fetch the thread.
    if let Some(rx) = state.post_comment_rx.take() {
        match rx.try_recv() {
            Ok(res) => {
                state.posting = false;
                match res {
                    Ok(_) => {
                        for mut i in &mut input {
                            i.value.clear();
                            i.caret_index = 0;
                            i.sel_anchor = None;
                            i.select_all = false;
                        }
                        let asset_id = state.asset_id.clone();
                        fetch_comments(&mut state, &asset_id);
                    }
                    Err(e) => state.error = Some(e),
                }
            }
            Err(TryRecvError::Empty) => state.post_comment_rx = Some(rx),
            Err(TryRecvError::Disconnected) => state.posting = false,
        }
    }
    // Rating post acknowledged → the worker already re-read the aggregate and
    // stamped your vote, so apply it straight to the displayed rating.
    if let Some(rx) = state.post_rating_rx.take() {
        match rx.try_recv() {
            Ok(res) => match res {
                Ok(r) => state.rating = Some(r),
                Err(e) => state.error = Some(e),
            },
            Err(TryRecvError::Empty) => state.post_rating_rx = Some(rx),
            Err(TryRecvError::Disconnected) => {}
        }
    }
}
