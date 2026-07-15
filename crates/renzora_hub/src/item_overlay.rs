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
//! Networking mirrors `native_store`: every call is blocking on a worker thread,
//! its result posted over a `crossbeam_channel` and drained in [`poll_item`].
//! There is no async runtime here.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};
use crossbeam_channel::{unbounded, Receiver, TryRecvError};

use renzora_auth::marketplace::{
    AssetComment, AssetRating, AssetSummary, CommentsResponse, MediaItem,
};
use renzora_auth::session::AuthSession;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::{
    bind_display, bind_text, bind_text_color, bind_with, keyed_list, KeyedSnapshot,
};
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    accent_button, accent_chip, audio_player, scroll_area, text_input, tint,
    AudioPlayer as EmberAudioPlayer, EmberForm, EmberTextInput, HoverTint, OverlaySurface,
};
use renzora::SplashState;

// Native audio playback backend for the audio-preview widget. Gated because the
// Kira stack (and the whole `renzora_audio` native module) doesn't compile on
// wasm; the overlay still builds there, the audio player just stays silent.
#[cfg(not(target_arch = "wasm32"))]
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
#[cfg(not(target_arch = "wasm32"))]
use kira::sound::PlaybackState;
#[cfg(not(target_arch = "wasm32"))]
use kira::Tween;
#[cfg(not(target_arch = "wasm32"))]
use renzora_audio::{KiraAudioManager, MixerState};

use crate::thumbs::HubThumbs;

/// The marketplace identity hue — a warm amber/gold that reads as "store",
/// distinct from tool chrome. Used only as low-alpha tints + accents so it
/// adapts to any active theme.
const HUE_STORE: (u8, u8, u8) = (226, 168, 74);
/// Filled-star color (a slightly brighter gold than the hue for contrast).
const GOLD: (u8, u8, u8) = (236, 194, 92);
/// Free-asset "Get" accent (matches `native_store`'s free pill).
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
    /// The in-flight `/media` fetch, drained in [`poll_item`].
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
    /// The playing/paused Kira handle. Dropping it does NOT stop playback, so it
    /// must be `stop()`ped explicitly (see [`stop_audio_inner`]).
    handle: Option<StaticSoundHandle>,
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
/// The comment "Post" button (also the [`EmberForm`] submit target).
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
/// keeps [`sync_audio`] scoped to this overlay's player.
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
            (item_close, store_card_click).chain(),
            item_install_click,
            item_star_click,
            item_post_click,
            strip_thumb_click,
            main_image_click,
            video_thumb_click,
            audio_track_click,
            poll_item,
        )
            .run_if(in_state(SplashState::Editor)),
    );
    // The audio backend is native-only; on wasm the player renders but stays silent.
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(Update, sync_audio.run_if(in_state(SplashState::Editor)));
}

/// True when a user is signed in — gates rating/commenting.
fn signed_in(w: &World) -> bool {
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

// ── Open ───────────────────────────────────────────────────────────────────────

/// Card body click → open the detail overlay for that asset. One open per frame;
/// the pill children capture their own clicks (they're `Block`), so this only
/// fires for a genuine body/thumbnail press.
fn store_card_click(q: Query<(&Interaction, &StoreCardBtn), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let asset = btn.0.clone();
        commands.queue(move |world: &mut World| open(world, asset));
        break;
    }
}

/// Build and show the detail overlay for `asset`. Exclusive-world (queued from
/// the card click) so it can read `EmberFonts` / `AuthSession`, request the
/// preview thumbnail, and kick the comments/rating fetches in one shot.
fn open(world: &mut World, asset: AssetSummary) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };

    // Replace any overlay already up (defensive — the close system normally runs
    // first, but a queued open shouldn't leak a second backdrop either).
    if let Some(old) = world.get_resource::<ItemOverlay>().and_then(|s| s.root) {
        if let Ok(e) = world.get_entity_mut(old) {
            e.despawn();
        }
    }
    // Stop any clip from the previous overlay first: the state resource is about
    // to be overwritten, and dropping a Kira handle doesn't stop playback.
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(mut old) = world.get_resource_mut::<ItemOverlay>() {
        stop_audio_inner(&mut old.audio);
    }

    // Reuse the thumbnail the card already requested; request again in case this
    // asset scrolled out of the visible set that `request_store_thumbs` covers.
    if let (Some(mut thumbs), Some(url)) =
        (world.get_resource_mut::<HubThumbs>(), asset.thumbnail_url.clone())
    {
        thumbs.request(&url);
    }

    let session = world
        .get_resource::<AuthSession>()
        .filter(|s| s.is_signed_in())
        .map(clone_session);

    let mut queue = CommandQueue::default();
    let root = {
        let mut commands = Commands::new(&mut queue, world);
        build_overlay(&mut commands, &fonts, &asset)
    };
    queue.apply(world);

    // Seed state and fire the initial fetches.
    let mut state = ItemOverlay {
        root: Some(root),
        asset_id: asset.id.clone(),
        session,
        comments_loading: true,
        asset: Some(asset.clone()),
        ..default()
    };
    // Comments and ratings (reviews) are both keyed by asset id.
    fetch_comments(&mut state, &asset.id);
    fetch_rating(&mut state, &asset.id);
    fetch_media(&mut state, &asset.id);
    world.insert_resource(state);

    // Kick the 3D turntable for model/animation assets (a no-op that resets the
    // rig for anything else, so a prior model never lingers behind a new card).
    crate::model_viewer::open_model_preview(world, &asset);
    // Kick the live material/shader preview for material/shader assets (resets
    // the rig for anything else).
    crate::material_viewer::open_material_preview(world, &asset);
}

/// Spawn the backdrop + content card and return the backdrop root.
fn build_overlay(commands: &mut Commands, fonts: &EmberFonts, asset: &AssetSummary) -> Entity {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.82)),
            GlobalZIndex(9600),
            FocusPolicy::Block,
            // OverlaySurface + a cursor probe so ember counts this as a modal
            // surface and confines wheel/pointer to it (panels behind go inert).
            OverlaySurface,
            RelativeCursorPosition::default(),
            Interaction::default(),
            ItemBackdrop,
            Name::new("item-overlay"),
        ))
        .id();

    let card = commands
        .spawn((
            Node {
                width: Val::Percent(92.0),
                max_width: Val::Px(640.0),
                max_height: Val::Percent(86.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            // Block so clicks inside the card never reach the backdrop-close.
            FocusPolicy::Block,
            Name::new("item-overlay-card"),
        ))
        .id();

    let header = build_header(commands, fonts, asset);
    let body = build_body(commands, fonts, asset);
    // `scroll_area` (px-capped) sizes to content; `scroll_view` flex-grows to
    // fill a fixed-height parent, and this card is content-sized (only
    // `max_height`), so it would collapse the body to zero height.
    let body_scroll = scroll_area(commands, body, 520.0);

    commands.entity(card).add_children(&[header, body_scroll]);
    commands.entity(root).add_child(card);
    root
}

/// The pinned header: the large main-image viewer (the currently-selected
/// gallery image) with a floating close X.
fn build_header(commands: &mut Commands, fonts: &EmberFonts, _asset: &AssetSummary) -> Entity {
    let header = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(240.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
        ))
        .id();

    // ── Static image gallery (non-model assets, or a model whose 3D preview
    // failed to load). Wrapped so its whole visibility is gated by
    // `model_viewer::show_gallery`, while the inner image keeps its own
    // swap-on-ready display logic. ──
    let gallery_wrap = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            overflow: Overflow::clip(),
            display: Display::None,
            ..default()
        })
        .id();
    // Gallery shows when neither live preview is active: the model viewer says so
    // (non-model / failed) AND no material preview is on.
    bind_display(commands, gallery_wrap, |w| {
        crate::model_viewer::show_gallery(w) && !crate::material_viewer::material_active(w)
    });
    let img = commands
        .spawn((
            ImageNode::default(),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::None,
                ..default()
            },
            // Click the big preview to open it full-size in the lightbox.
            Interaction::default(),
            MainImageBtn,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    // Bind to the resolved handle of the *selected* image, not a fixed URL: the
    // value changes both when the selection moves and when a handle finishes
    // loading, so the same swap-on-ready path covers both.
    bind_with(
        commands,
        img,
        selected_image_handle,
        |w, e, h: &Option<Handle<Image>>| {
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
        },
    );
    commands.entity(gallery_wrap).add_child(img);
    commands.entity(header).add_child(gallery_wrap);

    // ── Live 3D turntable (model / animation assets). Shows the offscreen RTT
    // once the model is framed; a placeholder covers the load. Letterboxed
    // (height-fit, 16:9) so the model isn't stretched by the header's ratio. ──
    let model_img = commands
        .spawn((
            ImageNode::default(),
            Node {
                height: Val::Percent(100.0),
                aspect_ratio: Some(16.0 / 9.0),
                display: Display::None,
                ..default()
            },
        ))
        .id();
    bind_with(
        commands,
        model_img,
        crate::model_viewer::preview_image_handle,
        |w, e, h: &Option<Handle<Image>>| {
            if let Some(h) = h {
                if let Some(mut n) = w.get_mut::<ImageNode>(e) {
                    if n.image != *h {
                        n.image = h.clone();
                    }
                }
            }
        },
    );
    bind_display(commands, model_img, crate::model_viewer::model_ready);
    commands.entity(header).add_child(model_img);

    // Loading placeholder for the 3D preview (self-hides once ready/failed).
    let model_note = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                display: Display::None,
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let model_note_lbl = commands
        .spawn((
            Text::new("Loading 3D preview\u{2026}"),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(model_note).add_child(model_note_lbl);
    bind_display(commands, model_note, crate::model_viewer::model_loading);
    commands.entity(header).add_child(model_note);

    // ── Live material/shader preview (materials & shaders assets). Same
    // letterboxed RTT treatment as the model turntable. ──
    let mat_img = commands
        .spawn((
            ImageNode::default(),
            Node {
                height: Val::Percent(100.0),
                aspect_ratio: Some(16.0 / 9.0),
                display: Display::None,
                ..default()
            },
        ))
        .id();
    bind_with(
        commands,
        mat_img,
        crate::material_viewer::preview_image_handle,
        |w, e, h: &Option<Handle<Image>>| {
            if let Some(h) = h {
                if let Some(mut n) = w.get_mut::<ImageNode>(e) {
                    if n.image != *h {
                        n.image = h.clone();
                    }
                }
            }
        },
    );
    bind_display(commands, mat_img, crate::material_viewer::material_ready);
    commands.entity(header).add_child(mat_img);

    // Loading placeholder for the material preview.
    let mat_note = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                display: Display::None,
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let mat_note_lbl = commands
        .spawn((
            Text::new("Compiling shader preview\u{2026}"),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(mat_note).add_child(mat_note_lbl);
    bind_display(commands, mat_note, crate::material_viewer::material_loading);
    commands.entity(header).add_child(mat_note);

    // Floating close button (top-right), over the image.
    let close = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(8.0),
                right: Val::Px(8.0),
                width: Val::Px(26.0),
                height: Val::Px(26.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(13.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            HoverTint::solid(
                Color::srgba(0.0, 0.0, 0.0, 0.5),
                Color::srgba(0.0, 0.0, 0.0, 0.72),
                Color::srgba(0.0, 0.0, 0.0, 0.72),
            ),
            Interaction::default(),
            ItemCloseBtn,
            Name::new("item-overlay-close"),
        ))
        .id();
    let x = icon_text(commands, &fonts.phosphor, "x", (240, 240, 245), 13.0);
    commands.entity(x).insert(FocusPolicy::Pass);
    commands.entity(close).add_child(x);
    commands.entity(header).add_child(close);
    header
}

/// The scrollable body: title, meta, rating, install, description, comments.
fn build_body(commands: &mut Commands, fonts: &EmberFonts, asset: &AssetSummary) -> Entity {
    let col = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            padding: UiRect::all(Val::Px(16.0)),
            ..default()
        })
        .id();

    // Title.
    let name = commands
        .spawn((
            Text::new(asset.name.clone()),
            ui_font(&fonts.ui, 18.0),
            TextColor(rgb(text_primary())),
        ))
        .id();

    // Meta row: creator + category + downloads chips.
    let meta = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            flex_wrap: FlexWrap::Wrap,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    let by = commands
        .spawn((
            Text::new(format!("by {}", asset.creator_name)),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let cat_chip = accent_chip(commands, fonts, HUE_STORE, Some("tag"), &asset.category);
    let dl_chip = accent_chip(
        commands,
        fonts,
        HUE_STORE,
        Some("download-simple"),
        &format!("{} downloads", asset.downloads),
    );
    commands.entity(meta).add_children(&[by, cat_chip, dl_chip]);

    // Rating row: five interactive stars + average/count + the viewer's vote.
    let rating = build_rating_row(commands, fonts);

    // Install / Get action, price carried on its label.
    let (label, hue) = if asset.price_credits == 0 {
        ("Get for free".to_string(), GREEN)
    } else {
        (format!("Buy ({} credits)", asset.price_credits), HUE_STORE)
    };
    let actions = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let install = accent_button(commands, fonts, hue, &label);
    commands.entity(install).insert(ItemInstallBtn);
    commands.entity(actions).add_child(install);

    // Any network error, surfaced quietly under the actions.
    let error = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 10.5),
            TextColor(rgb((224, 96, 96))),
        ))
        .id();
    bind_text(commands, error, |w| {
        w.get_resource::<ItemOverlay>().and_then(|s| s.error.clone()).unwrap_or_default()
    });
    bind_display(commands, error, |w| {
        w.get_resource::<ItemOverlay>().map(|s| s.error.is_some()).unwrap_or(false)
    });

    // Description.
    let desc_label = section_label(commands, fonts, "About");
    let desc = commands
        .spawn((
            Text::new(if asset.description.trim().is_empty() {
                "No description provided.".to_string()
            } else {
                asset.description.clone()
            }),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(text_muted())),
        ))
        .id();

    // Comments.
    let comments_label = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 12.5), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, comments_label, |w| {
        let n = w.get_resource::<ItemOverlay>().map(|s| s.comments.len()).unwrap_or(0);
        if n == 0 {
            "Comments".to_string()
        } else {
            format!("Comments ({n})")
        }
    });
    let comments_list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    // Untokened: the modal is short-lived and the list small, so recomputing the
    // snapshot each frame is cheap and avoids any stale-token risk when comments
    // arrive or a new post lands.
    keyed_list(commands, comments_list, comments_snapshot);

    let composer = build_composer(commands, fonts);
    let div1 = divider(commands);
    let div2 = divider(commands);

    // Gallery + media previews. Each is self-hiding when its media type is
    // absent (a fresh asset with no `/media` shows just the fallback thumbnail).
    let strip = build_strip(commands);
    let audio = build_audio(commands, fonts);
    let video = build_video(commands, fonts);

    // Live material/shader controls — shape selector + auto-generated `@param`
    // sliders (self-hides unless the open asset is a material/shader).
    let mat_controls = crate::material_viewer::build_material_controls(commands, fonts);

    commands.entity(col).add_children(&[
        name,
        meta,
        strip,
        mat_controls,
        rating,
        actions,
        error,
        div1,
        desc_label,
        desc,
        audio,
        video,
        div2,
        comments_label,
        comments_list,
        composer,
    ]);
    col
}

// ── Gallery (image strip + main-viewer helpers) ─────────────────────────────────

/// The image URLs for the gallery: the `/media` images in order, or — if none
/// were returned — the asset's own single thumbnail as a one-item fallback.
fn image_urls(w: &World) -> Vec<String> {
    let Some(s) = w.get_resource::<ItemOverlay>() else {
        return Vec::new();
    };
    let imgs: Vec<String> = s
        .media
        .iter()
        .filter(|m| m.media_type == "image")
        .map(|m| m.url.clone())
        .collect();
    if !imgs.is_empty() {
        return imgs;
    }
    s.asset
        .as_ref()
        .and_then(|a| a.thumbnail_url.clone())
        .into_iter()
        .collect()
}

/// The loaded texture for the currently-selected gallery image, if ready.
fn selected_image_handle(w: &World) -> Option<Handle<Image>> {
    let urls = image_urls(w);
    if urls.is_empty() {
        return None;
    }
    let sel = w
        .get_resource::<ItemOverlay>()
        .map(|s| s.selected_media)
        .unwrap_or(0);
    let url = urls.get(sel).or_else(|| urls.first())?;
    w.get_resource::<HubThumbs>().and_then(|t| t.get(url))
}

/// All media of a given type, in sorted order.
fn media_by_type(w: &World, ty: &str) -> Vec<MediaItem> {
    w.get_resource::<ItemOverlay>()
        .map(|s| {
            s.media
                .iter()
                .filter(|m| m.media_type == ty)
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// The horizontal thumbnail strip beneath the main viewer. Shown only when the
/// gallery holds more than one image; each thumb selects its image on click.
fn build_strip(commands: &mut Commands) -> Entity {
    let strip = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(6.0),
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    keyed_list(commands, strip, strip_snapshot);
    strip
}

/// Keyed snapshot of the strip: one thumb per image, keyed by index, with the
/// selected flag folded into the content hash so selecting rebuilds just the
/// two affected thumbs (old + new) rather than the whole strip.
fn strip_snapshot(world: &World) -> KeyedSnapshot {
    let urls = image_urls(world);
    if urls.len() <= 1 {
        return empty_snapshot();
    }
    let sel = world
        .get_resource::<ItemOverlay>()
        .map(|s| s.selected_media)
        .unwrap_or(0);
    use std::hash::{Hash, Hasher};
    let items: Vec<(u64, u64)> = urls
        .iter()
        .enumerate()
        .map(|(i, u)| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            i.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (u, i == sel).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    let urls2 = urls;
    KeyedSnapshot {
        items,
        build: Box::new(move |c, _f, i| strip_thumb(c, &urls2[i], i, i == sel)),
    }
}

/// One strip thumbnail: a small clickable image with a selection border.
fn strip_thumb(commands: &mut Commands, url: &str, index: usize, selected: bool) -> Entity {
    let cell = commands
        .spawn((
            Node {
                width: Val::Px(64.0),
                height: Val::Px(42.0),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                border: UiRect::all(Val::Px(if selected { 2.0 } else { 1.0 })),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
            BorderColor::all(if selected { rgb(HUE_STORE) } else { rgb(border()) }),
            Interaction::default(),
            StripThumbBtn(index),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("gallery-thumb"),
        ))
        .id();
    let img = commands
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
        .id();
    let u = url.to_string();
    bind_with(
        commands,
        img,
        move |w| w.get_resource::<HubThumbs>().and_then(|t| t.get(&u)),
        |w, e, h: &Option<Handle<Image>>| {
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
        },
    );
    commands.entity(cell).add_child(img);
    cell
}

/// An always-empty keyed snapshot (self-hiding sections). The `build` closure is
/// never called since there are no items, but the API requires one.
fn empty_snapshot() -> KeyedSnapshot {
    KeyedSnapshot {
        items: Vec::new(),
        build: Box::new(|c, _f, _i| c.spawn_empty().id()),
    }
}

// ── Audio preview ──────────────────────────────────────────────────────────────

/// The audio section: a header, a track selector (only for multi-track assets),
/// and a single ember [`audio_player`] the hub drives via Kira. The whole section
/// hides when the asset has no audio media.
fn build_audio(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let wrap = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        })
        .id();
    bind_display(commands, wrap, |w| !media_by_type(w, "audio").is_empty());

    let label = section_label(commands, fonts, "Audio preview");

    // Track selector (rebuilt on selection — cheap; it's just labels).
    let selector = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            ..default()
        })
        .id();
    keyed_list(commands, selector, audio_selector_snapshot);

    // The single player, in its own keyed slot keyed only on "audio exists" so a
    // selection change never rebuilds (and tears down) the live player.
    let player_slot = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            ..default()
        })
        .id();
    keyed_list(commands, player_slot, audio_player_snapshot);

    commands
        .entity(wrap)
        .add_children(&[label, selector, player_slot]);
    wrap
}

/// A friendly label for an audio track: its file name, else `Track N`.
fn track_label(m: &MediaItem, index: usize) -> String {
    m.url
        .rsplit('/')
        .next()
        .map(|s| s.split('?').next().unwrap_or(s))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("Track {}", index + 1))
}

/// Keyed snapshot of the audio track selector — one row per track, highlighted
/// when selected. Empty (hidden) unless there's more than one track.
fn audio_selector_snapshot(world: &World) -> KeyedSnapshot {
    let tracks = media_by_type(world, "audio");
    if tracks.len() <= 1 {
        return empty_snapshot();
    }
    let sel = world
        .get_resource::<ItemOverlay>()
        .map(|s| s.audio_selected)
        .unwrap_or(0);
    use std::hash::{Hash, Hasher};
    let names: Vec<String> = tracks
        .iter()
        .enumerate()
        .map(|(i, m)| track_label(m, i))
        .collect();
    let items: Vec<(u64, u64)> = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            i.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (n, i == sel).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| audio_track_row(c, f, &names[i], i, i == sel)),
    }
}

/// One clickable track-selector row.
fn audio_track_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    name: &str,
    index: usize,
    selected: bool,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(if selected {
                tint(HUE_STORE, 26)
            } else {
                Color::NONE
            }),
            Interaction::default(),
            AudioTrackBtn(index),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(name.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(if selected { text_primary() } else { text_muted() })),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(row).add_child(text);
    row
}

/// Keyed snapshot of the one audio player. A constant key/hash means it's built
/// exactly once when audio first appears and never rebuilt thereafter, so the
/// live Kira binding survives image/track selection.
fn audio_player_snapshot(world: &World) -> KeyedSnapshot {
    if media_by_type(world, "audio").is_empty() {
        return empty_snapshot();
    }
    KeyedSnapshot {
        items: vec![(0, 0)],
        build: Box::new(|c, f, _i| {
            let p = audio_player(c, f);
            c.entity(p).insert(HubAudioPlayer);
            p
        }),
    }
}

// ── Video preview ────────────────────────────────────────────────────────────────

/// The video section: one poster card per video, opened in the browser on click
/// (native video decode is out of scope). Hidden when the asset has no video.
fn build_video(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let wrap = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        })
        .id();
    bind_display(commands, wrap, |w| !media_by_type(w, "video").is_empty());

    let label = section_label(commands, fonts, "Video");
    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    keyed_list(commands, list, video_snapshot);
    commands.entity(wrap).add_children(&[label, list]);
    wrap
}

/// Keyed snapshot of the video posters — one card per video, keyed by id.
fn video_snapshot(world: &World) -> KeyedSnapshot {
    let videos = media_by_type(world, "video");
    if videos.is_empty() {
        return empty_snapshot();
    }
    use std::hash::{Hash, Hasher};
    let items: Vec<(u64, u64)> = videos
        .iter()
        .map(|m| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            m.id.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (&m.url, &m.thumbnail_url).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| video_card(c, f, &videos[i])),
    }
}

/// One video poster: the thumbnail with a centered play triangle, a caption, and
/// a click that opens the video URL in the browser.
fn video_card(commands: &mut Commands, fonts: &EmberFonts, item: &MediaItem) -> Entity {
    let card = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(180.0),
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
            Interaction::default(),
            VideoBtn(item.url.clone()),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("video-card"),
        ))
        .id();

    if let Some(poster) = item.thumbnail_url.clone() {
        let img = commands
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
            .id();
        bind_with(
            commands,
            img,
            move |w| w.get_resource::<HubThumbs>().and_then(|t| t.get(&poster)),
            |w, e, h: &Option<Handle<Image>>| {
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
            },
        );
        commands.entity(card).add_child(img);
    }

    // Play-triangle badge (over the poster).
    let badge = commands
        .spawn((
            Node {
                width: Val::Px(52.0),
                height: Val::Px(52.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(26.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            FocusPolicy::Pass,
        ))
        .id();
    let tri = icon_text(commands, &fonts.phosphor, "play", (240, 240, 245), 22.0);
    commands.entity(tri).insert(FocusPolicy::Pass);
    commands.entity(badge).add_child(tri);
    commands.entity(card).add_child(badge);

    // "Opens in your browser" caption, bottom-left.
    let caption = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                bottom: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            FocusPolicy::Pass,
        ))
        .id();
    let caption_text = commands
        .spawn((
            Text::new("Opens in your browser"),
            ui_font(&fonts.ui, 9.5),
            TextColor(rgb((224, 224, 230))),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(caption).add_child(caption_text);
    commands.entity(card).add_child(caption);
    card
}

/// Five stars showing the average (hover previews your vote), plus the aggregate
/// text and the viewer's current rating.
fn build_rating_row(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();

    let stars_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(1.0),
            ..default()
        })
        .id();

    // Build the star containers first, then wire each glyph's color to the
    // hovered star (preview) or the rounded average (resting).
    let mut stars: Vec<(Entity, Entity)> = Vec::with_capacity(5);
    for i in 1..=5 {
        let star = commands
            .spawn((
                Node {
                    padding: UiRect::all(Val::Px(1.0)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                Interaction::default(),
                StarBtn(i),
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                Name::new("item-star"),
            ))
            .id();
        let glyph = icon_text(commands, &fonts.phosphor, "star", GOLD, 18.0);
        commands.entity(glyph).insert(FocusPolicy::Pass);
        commands.entity(star).add_child(glyph);
        commands.entity(stars_row).add_child(star);
        stars.push((star, glyph));
    }
    let containers: Vec<Entity> = stars.iter().map(|(s, _)| *s).collect();
    for (idx, (_, glyph)) in stars.iter().enumerate() {
        let containers = containers.clone();
        let value = idx as i32 + 1;
        bind_text_color(commands, *glyph, move |w| {
            let displayed = hovered_star(w, &containers).unwrap_or_else(|| {
                w.get_resource::<ItemOverlay>()
                    .and_then(|s| s.rating.as_ref())
                    .map(|r| r.average.round() as i32)
                    .unwrap_or(0)
            });
            if value <= displayed {
                rgb(GOLD)
            } else {
                rgb(text_muted())
            }
        });
    }

    let aggregate = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted()))))
        .id();
    bind_text(commands, aggregate, |w| {
        match w.get_resource::<ItemOverlay>().and_then(|s| s.rating.clone()) {
            Some(r) if r.count > 0 => format!("{:.1} ({})", r.average, r.count),
            _ => "No ratings yet".to_string(),
        }
    });

    let yours = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.5), TextColor(rgb(HUE_STORE))))
        .id();
    bind_text(commands, yours, |w| {
        w.get_resource::<ItemOverlay>()
            .and_then(|s| s.rating.as_ref().and_then(|r| r.user_rating))
            .map(|n| format!("\u{00b7} your rating: {n}/5"))
            .unwrap_or_default()
    });
    bind_display(commands, yours, |w| {
        w.get_resource::<ItemOverlay>()
            .and_then(|s| s.rating.as_ref().and_then(|r| r.user_rating))
            .is_some()
    });

    commands.entity(row).add_children(&[stars_row, aggregate, yours]);
    row
}

/// The comment composer: a text input + Post button wired as an [`EmberForm`]
/// (Enter submits), shown only when signed in; otherwise a sign-in prompt.
fn build_composer(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let wrap = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            margin: UiRect::top(Val::Px(2.0)),
            ..default()
        })
        .id();

    // Post button first — it's the form's submit target.
    let post = accent_button(commands, fonts, HUE_STORE, "Post");
    commands.entity(post).insert(ItemPostBtn);

    let form = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            EmberForm { submit: post },
        ))
        .id();
    let input = text_input(commands, &fonts.ui, "Add a comment...", "");
    commands.entity(input).insert((
        ItemCommentInput,
        Node {
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
    ));
    commands.entity(form).add_children(&[input, post]);
    bind_display(commands, form, signed_in);

    // Signed-out prompt (mutually exclusive with the form).
    let prompt = commands
        .spawn((
            Text::new("Sign in to rate and comment."),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_display(commands, prompt, |w| !signed_in(w));

    commands.entity(wrap).add_children(&[form, prompt]);
    wrap
}

/// One comment: author + timestamp header, then the body text.
fn comment_row(commands: &mut Commands, fonts: &EmberFonts, c: &AssetComment) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                padding: UiRect::all(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
        ))
        .id();
    let head = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let who = commands
        .spawn((
            Text::new(c.user_name.clone()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let when = commands
        .spawn((
            Text::new(c.created_at.clone()),
            ui_font(&fonts.ui, 9.5),
            TextColor(rgb(placeholder())),
        ))
        .id();
    commands.entity(head).add_children(&[who, when]);
    let body = commands
        .spawn((
            Text::new(c.content.clone()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(row).add_children(&[head, body]);
    row
}

/// Keyed snapshot of the comments list — a loading/empty note, or one row per
/// comment keyed by id (rebuilt only when a comment's content changes).
fn comments_snapshot(world: &World) -> KeyedSnapshot {
    let Some(state) = world.get_resource::<ItemOverlay>() else {
        return note_snapshot("");
    };
    if state.comments_loading && state.comments.is_empty() {
        return note_snapshot("Loading comments...");
    }
    if state.comments.is_empty() {
        return note_snapshot("No comments yet. Be the first to comment.");
    }
    let comments = state.comments.clone();
    use std::hash::{Hash, Hasher};
    let items: Vec<(u64, u64)> = comments
        .iter()
        .map(|c| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            c.id.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (&c.user_name, &c.content, &c.created_at).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| comment_row(c, f, &comments[i])),
    }
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

/// The highest 1-based star index currently hovered/pressed, if any.
fn hovered_star(w: &World, stars: &[Entity]) -> Option<i32> {
    stars.iter().enumerate().rev().find_map(|(i, &e)| {
        matches!(
            w.get::<Interaction>(e),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        )
        .then_some(i as i32 + 1)
    })
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

// ── Systems ──────────────────────────────────────────────────────────────────

/// Close on a backdrop press, the X, or Escape. `try_despawn` because a layout
/// rebuild may already have torn the overlay down.
fn item_close(
    keys: Res<ButtonInput<KeyCode>>,
    backdrop: Query<&Interaction, (With<ItemBackdrop>, Changed<Interaction>)>,
    close_btn: Query<&Interaction, (With<ItemCloseBtn>, Changed<Interaction>)>,
    lightbox: Res<crate::hub_lightbox::HubLightbox>,
    mut state: ResMut<ItemOverlay>,
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
        // Stop the clip first — resetting the resource drops the handle, and a
        // dropped Kira handle keeps playing.
        #[cfg(not(target_arch = "wasm32"))]
        stop_audio_inner(&mut state.audio);
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
fn item_install_click(
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
fn item_star_click(
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
fn item_post_click(
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
fn strip_thumb_click(
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
fn main_image_click(
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
fn video_thumb_click(q: Query<(&Interaction, &VideoBtn), Changed<Interaction>>) {
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            crate::native_store::open_url(&btn.0);
            break;
        }
    }
}

/// Click a track-selector row → switch the audio player to that track. Only the
/// index changes here; [`sync_audio`] notices the divergence and stops/reloads.
fn audio_track_click(
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
fn poll_item(
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

// ── Network (blocking, on worker threads) ──────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
fn fetch_comments(state: &mut ItemOverlay, asset_id: &str) {
    let (tx, rx) = unbounded();
    state.comments_rx = Some(rx);
    state.comments_loading = true;
    let asset_id = asset_id.to_string();
    std::thread::spawn(move || {
        let _ = tx.send(renzora_auth::marketplace::get_comments(&asset_id));
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch_rating(state: &mut ItemOverlay, asset_id: &str) {
    let (tx, rx) = unbounded();
    state.rating_rx = Some(rx);
    let asset_id = asset_id.to_string();
    let session = state.session.as_ref().map(clone_session);
    std::thread::spawn(move || {
        let _ = tx.send(renzora_auth::marketplace::get_rating(&asset_id, session.as_ref()));
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_post_comment(
    session: AuthSession,
    asset_id: String,
    content: String,
    tx: crossbeam_channel::Sender<Result<AssetComment, String>>,
) {
    std::thread::spawn(move || {
        let _ = tx.send(renzora_auth::marketplace::post_comment(&session, &asset_id, &content));
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_post_rating(
    session: AuthSession,
    asset_id: String,
    rating: i32,
    tx: crossbeam_channel::Sender<Result<AssetRating, String>>,
) {
    std::thread::spawn(move || {
        let _ = tx.send(renzora_auth::marketplace::post_rating(&session, &asset_id, rating));
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch_media(state: &mut ItemOverlay, asset_id: &str) {
    let (tx, rx) = unbounded();
    state.media_rx = Some(rx);
    let asset_id = asset_id.to_string();
    std::thread::spawn(move || {
        let _ = tx.send(renzora_auth::marketplace::get_media(&asset_id));
    });
}

// ── Audio playback (native, driving the ember AudioPlayer via Kira) ─────────────

/// The audio-track URLs (the `/media` audio subset), in order.
#[cfg(not(target_arch = "wasm32"))]
fn audio_urls(state: &ItemOverlay) -> Vec<String> {
    state
        .media
        .iter()
        .filter(|m| m.media_type == "audio")
        .map(|m| m.url.clone())
        .collect()
}

/// Stop the live clip and clear the playback state. Explicit `stop()` is
/// required — dropping a Kira handle doesn't halt the sound.
#[cfg(not(target_arch = "wasm32"))]
fn stop_audio_inner(audio: &mut AudioPlayback) {
    if let Some(mut h) = audio.handle.take() {
        h.stop(Tween::default());
    }
    audio.track = None;
    audio.rx = None;
    audio.loading = false;
    audio.duration = 0.0;
    audio.spectrum.clear();
    audio.levels.clear();
}

/// Kick off a background download of the clip bytes for `url`.
#[cfg(not(target_arch = "wasm32"))]
fn spawn_audio_download(audio: &mut AudioPlayback, url: &str) {
    let (tx, rx) = unbounded();
    audio.rx = Some(rx);
    audio.loading = true;
    let url = url.to_string();
    std::thread::spawn(move || {
        let _ = tx.send(renzora_auth::marketplace::download_file(&url));
    });
}

/// Precompute a spectrogram over the first [`PREVIEW_SECS`] of a clip: for each
/// of [`EQ_COLUMNS`] time slices, the energy in [`EQ_BANDS`] log-spaced frequency
/// bands (via a Goertzel single-frequency filter per band — cheap, no FFT crate).
/// The live EQ reads the column under the playhead so the bars bounce with the
/// music instead of showing one static envelope.
#[cfg(not(target_arch = "wasm32"))]
fn compute_spectrogram(data: &StaticSoundData) -> Vec<Vec<f32>> {
    use std::f32::consts::PI;
    let frames: &[kira::Frame] = &data.frames;
    let sr = data.sample_rate as f32;
    if frames.is_empty() || sr <= 0.0 {
        return Vec::new();
    }
    let cap = (((PREVIEW_SECS * sr) as usize).min(frames.len())).max(1);
    // Log-spaced band centers from 60 Hz up to just under Nyquist.
    let fmin = 60.0f32;
    let fmax = (sr * 0.45).clamp(fmin * 2.0, 14000.0);
    let centers: Vec<f32> = (0..EQ_BANDS)
        .map(|b| fmin * (fmax / fmin).powf(b as f32 / (EQ_BANDS.max(2) - 1) as f32))
        .collect();
    let win = 512usize.min(cap);
    let mut cols: Vec<Vec<f32>> = Vec::with_capacity(EQ_COLUMNS);
    for c in 0..EQ_COLUMNS {
        let center = ((c as f32 + 0.5) / EQ_COLUMNS as f32 * cap as f32) as usize;
        let start = center.saturating_sub(win / 2).min(cap.saturating_sub(win));
        let seg = &frames[start..(start + win).min(cap)];
        let mut col = vec![0.0f32; EQ_BANDS];
        for (bi, &f) in centers.iter().enumerate() {
            let coeff = 2.0 * (2.0 * PI * (f / sr)).cos();
            let (mut s1, mut s2) = (0.0f32, 0.0f32);
            for fr in seg {
                let x = (fr.left + fr.right) * 0.5;
                let s0 = x + coeff * s1 - s2;
                s2 = s1;
                s1 = s0;
            }
            col[bi] = (s1 * s1 + s2 * s2 - coeff * s1 * s2).max(0.0).sqrt();
        }
        cols.push(col);
    }
    // Global-normalize, then sqrt-lift so quiet bands stay visible.
    let maxv = cols.iter().flatten().copied().fold(0.0f32, f32::max);
    if maxv > 1e-6 {
        for col in &mut cols {
            for v in col {
                *v = (*v / maxv).sqrt().clamp(0.0, 1.0);
            }
        }
    }
    cols
}

/// Ease the EQ bar levels toward the spectrogram column under the playhead each
/// frame (fast attack, slower release), so the bars animate with the audio and
/// fall to zero when paused/stopped.
#[cfg(not(target_arch = "wasm32"))]
fn update_eq(audio: &mut AudioPlayback, playing: bool, position: f32) {
    if audio.levels.len() != EQ_BANDS {
        audio.levels = vec![0.0; EQ_BANDS];
    }
    let target: Vec<f32> = if playing && !audio.spectrum.is_empty() && audio.duration > 0.0 {
        let frac = (position / audio.duration).clamp(0.0, 1.0);
        let col = ((frac * audio.spectrum.len() as f32) as usize).min(audio.spectrum.len() - 1);
        audio.spectrum[col].clone()
    } else {
        vec![0.0; EQ_BANDS]
    };
    for (lvl, &t) in audio.levels.iter_mut().zip(target.iter()) {
        let rate = if t > *lvl { 0.6 } else { 0.18 };
        *lvl += (t - *lvl) * rate;
        // Snap tiny values to zero so idle (paused) frames stop re-baking.
        if lvl.abs() < 0.001 {
            *lvl = 0.0;
        }
    }
}

/// Bridge the on-screen ember [`AudioPlayer`] to the engine's Kira manager:
/// read the widget's `playing` / `seek_to` intent, drive the one live clip, and
/// push back `position` / `duration` / `amps`. Only one clip plays at a time.
#[cfg(not(target_arch = "wasm32"))]
fn sync_audio(
    mut state: ResMut<ItemOverlay>,
    manager: Option<NonSendMut<KiraAudioManager>>,
    mixer: Option<Res<MixerState>>,
    mut players: Query<&mut EmberAudioPlayer, With<HubAudioPlayer>>,
) {
    let Ok(mut ap) = players.single_mut() else {
        return; // no audio player on screen
    };
    let (Some(mut mgr), Some(mixer)) = (manager, mixer) else {
        return; // audio backend not up
    };
    let urls = audio_urls(&state);
    if urls.is_empty() {
        return;
    }
    let sel = state.audio_selected.min(urls.len() - 1);
    let cur_url = urls[sel].clone();

    // Selection moved away from the loaded track → stop it and reset the widget.
    if state.audio.track.is_some() && state.audio.track != Some(sel) {
        stop_audio_inner(&mut state.audio);
        ap.playing = false;
        ap.position = 0.0;
        ap.duration = 0.0;
        ap.amps.clear();
        ap.seek_to = None;
    }

    // A finished download → decode, publish peaks/duration, and play it.
    if let Some(rx) = state.audio.rx.take() {
        match rx.try_recv() {
            Ok(Ok(bytes)) => {
                state.audio.loading = false;
                match StaticSoundData::from_cursor(std::io::Cursor::new(bytes)) {
                    Ok(data) => {
                        // Cap the shown/scrubbable duration at 30s (the preview
                        // length); compute the EQ spectrogram before `data` moves.
                        state.audio.duration = data.duration().as_secs_f32().min(PREVIEW_SECS);
                        state.audio.spectrum = compute_spectrogram(&data);
                        state.audio.levels = vec![0.0; EQ_BANDS];
                        match mgr.play_on_bus(data, "Master", &mixer) {
                            Ok(handle) => {
                                state.audio.handle = Some(handle);
                                state.audio.track = Some(sel);
                                // Honor a pause requested while the clip loaded.
                                if !ap.playing {
                                    if let Some(h) = state.audio.handle.as_mut() {
                                        h.pause(Tween::default());
                                    }
                                }
                            }
                            Err(e) => state.error = Some(format!("Audio play failed: {e}")),
                        }
                    }
                    Err(e) => state.error = Some(format!("Audio decode failed: {e}")),
                }
            }
            Ok(Err(e)) => {
                state.audio.loading = false;
                state.error = Some(e);
            }
            Err(TryRecvError::Empty) => state.audio.rx = Some(rx),
            Err(TryRecvError::Disconnected) => state.audio.loading = false,
        }
    }

    // Play requested but nothing loaded/loading → start fetching the clip bytes.
    if ap.playing && state.audio.handle.is_none() && !state.audio.loading {
        spawn_audio_download(&mut state.audio, &cur_url);
    }

    // Apply intent to the live handle and read back its position.
    let mut finished = false;
    // Captured before the handle borrow so the 30s check below doesn't alias it.
    let cap_dur = state.audio.duration;
    if let Some(handle) = state.audio.handle.as_mut() {
        if let Some(t) = ap.seek_to.take() {
            handle.seek_to(t as f64);
        }
        let st = handle.state();
        if st == PlaybackState::Stopped {
            finished = true;
        } else if ap.playing && st != PlaybackState::Playing {
            handle.resume(Tween::default());
        } else if !ap.playing && st == PlaybackState::Playing {
            handle.pause(Tween::default());
        }
        ap.position = handle.position() as f32;
        // Enforce the 30s preview cap: stop the clip at the limit (dropping the
        // handle alone wouldn't halt Kira). `finished` then resets the widget.
        if cap_dur > 0.0 && ap.position >= cap_dur {
            handle.stop(Tween::default());
            finished = true;
        }
    }
    if finished {
        // Clip ran to the end (or hit the 30s cap): back to paused-at-zero.
        state.audio.handle = None;
        state.audio.track = None;
        ap.playing = false;
        ap.position = 0.0;
    }

    // Publish backend truth to the widget (only when it actually changed).
    let d = state.audio.duration;
    if (ap.duration - d).abs() > f32::EPSILON {
        ap.duration = d;
    }
    // Live EQ: ease the bar levels toward the spectrum column under the playhead
    // and push them to the waveform each frame (they fall to zero when paused).
    update_eq(&mut state.audio, ap.playing, ap.position);
    if ap.amps != state.audio.levels {
        ap.amps = state.audio.levels.clone();
    }
}

// The browser build has no blocking HTTP path; the overlay opens but stays inert.
#[cfg(target_arch = "wasm32")]
fn fetch_comments(state: &mut ItemOverlay, _slug: &str) {
    state.comments_loading = false;
}
#[cfg(target_arch = "wasm32")]
fn fetch_rating(_state: &mut ItemOverlay, _slug: &str) {}
#[cfg(target_arch = "wasm32")]
fn spawn_post_comment(
    _session: AuthSession,
    _slug: String,
    _content: String,
    _tx: crossbeam_channel::Sender<Result<AssetComment, String>>,
) {
}
#[cfg(target_arch = "wasm32")]
fn spawn_post_rating(
    _session: AuthSession,
    _slug: String,
    _rating: i32,
    _tx: crossbeam_channel::Sender<Result<AssetRating, String>>,
) {
}
#[cfg(target_arch = "wasm32")]
fn fetch_media(_state: &mut ItemOverlay, _asset_id: &str) {}
