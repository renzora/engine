//! The marketplace **item-detail overlay**: a store "product page" that opens
//! when a card is clicked. It shows a large preview, the asset's name / creator /
//! category / downloads / price, the full description, a star rating (view the
//! average and cast your own), an Install button, and a comments thread you can
//! read and post to.
//!
//! Why a bespoke backdrop instead of ember's [`overlay`](renzora_ember::widgets::overlay):
//! ember's shared `overlay_dismiss` despawns *every* `Overlay`-marked backdrop on
//! any outside click, which would fight this modal's own lifecycle (and any
//! install confirm stacked above it). So this owns its surface — a full-screen
//! [`OverlaySurface`] that swallows pointer/scroll from the panels behind — and
//! its own close on backdrop-press / Escape / the X. The close system is chained
//! *before* the card-open system so clicking one card while another detail is
//! open swaps cleanly rather than the close eating the press.
//!
//! Networking mirrors `store`: every call is blocking on a worker thread,
//! its result posted over a `crossbeam_channel` and drained in
//! [`systems::poll_item`]. There is no async runtime here.
//!
//! The overlay is one screen made of several independent sections, so each gets
//! its own module: [`open`] builds the shell, [`gallery`] the image strip and
//! main viewer, [`audio`] and [`video`] the media previews, [`comments`] the
//! rating row and thread, [`systems`] the click handlers and the channel drain,
//! and [`net`] the worker-thread calls.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use crossbeam_channel::Receiver;

use crate::auth::marketplace::{AssetComment, AssetRating, AssetSummary, CommentsResponse, MediaItem};
use crate::auth::session::AuthSession;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::KeyedSnapshot;
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::tint;
use renzora::SplashState;

// Native audio playback backend for the audio-preview widget. Gated because the
// Kira stack (and the whole `renzora_audio` native module) doesn't compile on
// wasm; the overlay still builds there, the audio player just stays silent.
#[cfg(not(target_arch = "wasm32"))]
use renzora_audio::VoiceId;

pub(crate) mod audio;
pub(crate) mod comments;
pub(crate) mod gallery;
pub(crate) mod net;
pub(crate) mod open;
pub(crate) mod systems;
pub(crate) mod video;

/// The marketplace identity hue — a warm amber/gold that reads as "store",
/// distinct from tool chrome. Used only as low-alpha tints + accents so it
/// adapts to any active theme.
const HUE_STORE: (u8, u8, u8) = (226, 168, 74);
/// Filled-star color (a slightly brighter gold than the hue for contrast).
const GOLD: (u8, u8, u8) = (236, 194, 92);
/// Free-asset "Get" accent (matches `store`'s free pill).
const GREEN: (u8, u8, u8) = (52, 180, 96);

/// Audio previews are capped at 30 seconds — a teaser, not the full track.
#[cfg(not(target_arch = "wasm32"))]
const PREVIEW_SECS: f32 = 30.0;
/// Frequency bands in the live EQ (bars across the spectrum).
#[cfg(not(target_arch = "wasm32"))]
const EQ_BANDS: usize = 24;
/// Precomputed spectrogram time-columns spanning the 30s preview.
#[cfg(not(target_arch = "wasm32"))]
const EQ_COLUMNS: usize = 480;

/// Live state for the open detail overlay. Holds the asset being viewed, the
/// in-flight network channels, and the fetched comments/rating. Reset to default
/// on close. A single overlay is open at a time.
#[derive(Resource, Default)]
struct ItemOverlay {
    /// The backdrop root entity, or `None` when closed. Despawning it tears down
    /// the whole overlay (its bindings/lists auto-drop with their targets).
    root: Option<Entity>,
    /// The asset on show — drives the Install action and titles.
    asset: Option<AssetSummary>,
    /// The asset id, cached for the comments/rating (review) endpoints, which
    /// are keyed by id.
    asset_id: String,
    /// Cloned signed-in session (if any) so worker threads can authenticate
    /// posts without touching the live resource.
    session: Option<AuthSession>,
    /// Fetched comments, newest-first as the API returns them.
    comments: Vec<AssetComment>,
    /// True while the initial comments fetch is outstanding (drives a spinner
    /// note instead of an empty state).
    comments_loading: bool,
    /// Fetched rating aggregate + the viewer's own vote, or `None` until loaded.
    rating: Option<AssetRating>,
    /// Last network error, surfaced in a small line under the actions.
    error: Option<String>,
    /// True between a comment post and its acknowledgement (debounces the button).
    posting: bool,
    /// The asset's preview-media gallery (images / video / audio), once fetched.
    media: Vec<MediaItem>,
    /// The in-flight `/media` fetch, drained in [`systems::poll_item`].
    media_rx: Option<Receiver<Result<Vec<MediaItem>, String>>>,
    /// Which image (index into the image-only subset) the main viewer shows.
    selected_media: usize,
    /// Which audio track (index into the audio-only subset) the single player
    /// controls; only meaningful when the asset ships more than one.
    audio_selected: usize,
    /// Native audio playback backing the one on-screen audio player.
    #[cfg(not(target_arch = "wasm32"))]
    audio: AudioPlayback,
    comments_rx: Option<Receiver<Result<CommentsResponse, String>>>,
    rating_rx: Option<Receiver<Result<AssetRating, String>>>,
    post_comment_rx: Option<Receiver<Result<AssetComment, String>>>,
    post_rating_rx: Option<Receiver<Result<AssetRating, String>>>,
}

/// The engine-audio state behind the marketplace audio preview. Holds the one
/// live Kira handle (only one clip plays at a time), the clip's decoded duration
/// and waveform peaks, and any in-flight byte download. Reset when the overlay
/// closes, when the selected track changes, or when a clip finishes.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct AudioPlayback {
    /// The audio-subset index currently loaded into `handle`, if any.
    track: Option<usize>,
    /// The playing/paused voice. Dropping the id does NOT stop playback, so it
    /// must be stopped explicitly (see [`audio::stop_audio_inner`]).
    voice: Option<VoiceId>,
    /// The decoded sound in the backend, so a seek can replay it without
    /// downloading or decoding again.
    sound: u64,
    /// Whether the widget was playing last frame, so a pause/resume is sent once
    /// on the edge rather than every frame.
    was_playing: bool,
    /// Playhead, advanced from wall time while playing.
    ///
    /// Tracked here rather than read back from the backend because the boundary
    /// has no position op — and deliberately so. A 30-second preview scrubber
    /// does not need sample accuracy, and an op per frame per player to learn a
    /// number this side can already compute would be a poor trade.
    position: f32,
    duration: f32,
    /// Precomputed spectrogram: one [`EQ_BANDS`]-long column per time slice. The
    /// live EQ reads the column under the playhead each frame.
    spectrum: Vec<Vec<f32>>,
    /// Current (smoothed) EQ bar levels pushed to the waveform each frame.
    levels: Vec<f32>,
    /// The clip's byte download in flight, and whether one is outstanding.
    rx: Option<Receiver<Result<Vec<u8>, String>>>,
    loading: bool,
}

/// Marks a store card's body as the click target that opens this overlay. Lives
/// on the card container in `native_store::asset_card`; the passive card children
/// are `FocusPolicy::Pass` so a body click falls through to the card, while the
/// Get/Preview pills stay `Block` and capture their own presses — so a pill click
/// never also opens the detail.
#[derive(Component)]
pub(crate) struct StoreCardBtn(pub AssetSummary);

/// The dim full-screen backdrop; a press on it (outside the content card) closes.
#[derive(Component)]
struct ItemBackdrop;
/// The titlebar X close button.
#[derive(Component)]
struct ItemCloseBtn;
/// The overlay's Install/Get button.
#[derive(Component)]
struct ItemInstallBtn;
/// A clickable rating star, carrying its 1-based value.
#[derive(Component)]
struct StarBtn(i32);
/// The comment composer input (read to submit, cleared on success).
#[derive(Component)]
struct ItemCommentInput;
/// The comment "Post" button (also the [`EmberForm`](renzora_ember::widgets::EmberForm)
/// submit target).
#[derive(Component)]
struct ItemPostBtn;
/// A gallery strip thumbnail, carrying the image index it selects on click.
#[derive(Component)]
struct StripThumbBtn(usize);
/// A video poster card, carrying the URL to open in the browser on click.
#[derive(Component)]
struct VideoBtn(String);
/// A track-selector row for a multi-track asset, carrying the audio index it
/// selects on click.
#[derive(Component)]
struct AudioTrackBtn(usize);
/// The single on-screen ember audio player the hub drives via Kira. Marking it
/// keeps [`audio::sync_audio`] scoped to this overlay's player.
#[derive(Component)]
struct HubAudioPlayer;
/// The big preview image — click to open it full-size in the lightbox.
#[derive(Component)]
struct MainImageBtn;

pub(crate) fn register(app: &mut App) {
    app.init_resource::<ItemOverlay>();
    crate::hub_lightbox::register(app);
    app.add_systems(
        Update,
        (
            // Close before open so clicking a new card while one is open swaps
            // rather than the backdrop-close consuming the press.
            (systems::item_close, open::store_card_click).chain(),
            systems::item_install_click,
            systems::item_star_click,
            systems::item_post_click,
            systems::strip_thumb_click,
            systems::main_image_click,
            systems::video_thumb_click,
            systems::audio_track_click,
            systems::poll_item,
        )
            .run_if(in_state(SplashState::Editor)),
    );
    // The audio backend is native-only; on wasm the player renders but stays silent.
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(
        Update,
        audio::sync_audio.run_if(in_state(SplashState::Editor)),
    );
}

/// True when a user is signed in — gates rating/commenting.
fn signed_in(w: &Rx) -> bool {
    w.get_resource::<AuthSession>().map(|s| s.is_signed_in()).unwrap_or(false)
}

/// `AuthSession` isn't `Clone`; clone its fields so a worker thread owns a copy.
fn clone_session(s: &AuthSession) -> AuthSession {
    AuthSession {
        user: s.user.clone(),
        access_token: s.access_token.clone(),
        refresh_token: s.refresh_token.clone(),
    }
}

/// An always-empty keyed snapshot (self-hiding sections). The `build` closure is
/// never called since there are no items, but the API requires one.
fn empty_snapshot() -> KeyedSnapshot {
    KeyedSnapshot {
        items: Vec::new(),
        build: Box::new(|c, _f, _i| c.spawn_empty().id()),
    }
}

fn section_label(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text.to_string()),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(text_primary())),
        ))
        .id()
}

fn divider(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(1.0),
                margin: UiRect::vertical(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(tint(HUE_STORE, 26)),
        ))
        .id()
}

/// A single muted note row (loading / empty states), keyed on its text so a
/// state change rebuilds it rather than reusing the stale message.
fn note_snapshot(text: &str) -> KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    let text = text.to_string();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    let key = h.finish();
    KeyedSnapshot {
        items: vec![(u64::MAX, key)],
        build: Box::new(move |c, f, _| {
            c.spawn((
                Text::new(text.clone()),
                ui_font(&f.ui, 11.0),
                TextColor(rgb(text_muted())),
                Node { margin: UiRect::vertical(Val::Px(8.0)), ..default() },
            ))
            .id()
        }),
    }
}

/// A thumbnail that swaps itself in the moment its texture resolves. Every
/// preview image in the overlay loads asynchronously, so they all start hidden
/// and reveal on the same binding rather than flashing an empty frame.
fn apply_thumb(w: &mut World, e: Entity, h: &Option<Handle<Image>>) {
    if let Some(h) = h {
        if let Some(mut n) = w.get_mut::<ImageNode>(e) {
            if n.image != *h {
                n.image = h.clone();
            }
        }
        if let Some(mut node) = w.get_mut::<Node>(e) {
            node.display = Display::Flex;
        }
    }
}

/// A hidden `ImageNode` that fills its parent, ready for [`apply_thumb`].
fn thumb_image(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            ImageNode::default(),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::None,
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id()
}
