//! Feed panel — one community activity stream: your posts feed (likes,
//! reactions, comments, cursor-paginated), the forum's latest threads, and
//! fresh marketplace assets, with source toggles plus sort / time-frame /
//! audience filters. A "new posts" refresh pill is driven by the WebSocket.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use crossbeam_channel::{unbounded, Receiver, Sender};
use renzora::core::{RenzoraShellExt, SocialBridge, SocialPanelRequest};
use renzora::SplashState;
use renzora_auth::feed::{Channel, FeedComment, FeedPost};
use renzora_auth::marketplace::AssetSummary;
use renzora_auth::AuthSession;
use renzora_ember::dock::panel_active;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, keyed_list_tokened, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::reactive::Bound;
use renzora_ember::widgets::{
    dropdown, empty_state, text_input, textarea, EmberForm, EmberTextInput, HoverTooltip,
};

use crate::reaction_picker::{reaction_bar, ReactionTarget};

use crate::avatars::avatar_image;
use crate::toasts::{ToastQueue, Tone};
use crate::util::{self, hash64, session_clone, HUE_FEED};
use crate::PendingSocialRequest;

pub(crate) const PANEL_ID: &str = "social_feed";

const PAGE_SIZE: u32 = 30;
const RED: (u8, u8, u8) = (224, 80, 80);
const VISIBILITIES: [&str; 3] = ["public", "followers", "friends"];

pub(crate) enum FeedResult {
    /// (append, page)
    Page(bool, Result<Vec<FeedPost>, String>),
    Posted(Result<(), String>),
    /// Like toggles are optimistic; errors just trigger a refresh.
    LikeFailed,
    Comments(String, Result<Vec<FeedComment>, String>),
    Commented(String, Result<(), String>),
    Uploaded(Result<String, String>),
    /// A post delete finished; errors restore via refresh.
    Deleted(Result<(), String>),
    /// The live channel list (for the left rail + the composer target).
    Channels(Vec<Channel>),
    /// A channel suggestion finished.
    Suggested(Result<(), String>),
    /// A post was reported (bool = whether it just got auto-hidden).
    Reported(Result<bool, String>),
    /// Newest marketplace assets.
    Assets(Vec<AssetSummary>),
    /// Who the signed-in user follows / is friends with (usernames), for the
    /// audience filter. `None` for a list that failed to load.
    Audience(Option<HashSet<String>>, Option<HashSet<String>>),
}

/// The feed's filter state — which sources show, how they're ordered, how far
/// back, and whose. All client-side over the fetched pages (the API has no
/// sort/time/audience parameters).
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct FeedFilters {
    pub show_posts: bool,
    pub show_market: bool,
    /// 0 = Recent, 1 = Most popular.
    pub sort: usize,
    /// 0 = All time, 1 = Today, 2 = This week, 3 = This month.
    pub timeframe: usize,
    /// 0 = Everyone, 1 = Following, 2 = Friends.
    pub audience: usize,
}

impl Default for FeedFilters {
    fn default() -> Self {
        Self {
            show_posts: true,
            show_market: true,
            sort: 0,
            timeframe: 0,
            audience: 0,
        }
    }
}

pub(crate) const FEED_SORTS: [&str; 2] = ["Recent", "Most popular"];
pub(crate) const FEED_TIMES: [&str; 4] = ["All time", "Today", "This week", "This month"];
pub(crate) const FEED_AUDIENCES: [&str; 3] = ["Everyone", "Following", "Friends"];

#[derive(Resource)]
pub(crate) struct FeedPanel {
    /// Set by the WebSocket when new posts exist upstream.
    pub stale: bool,
    /// Live mode: when true, new posts (a `stale` flip from the WS) are pulled
    /// in automatically so the feed stays at the top. Toggled by the play/pause
    /// button; when false, the "New posts" pill lets you catch up manually.
    pub auto_follow: bool,
    pub posts: Vec<FeedPost>,
    /// Live channels (left rail + composer target). Fetched once.
    pub channels: Vec<Channel>,
    pub channels_loaded: bool,
    /// The channel the feed is filtered to (slug), and the composer posts into.
    /// `None` = the whole feed / no channel.
    pub active_channel: Option<String>,
    /// Whether the "suggest a channel" input is open in the rail.
    pub suggesting: bool,
    /// Newest marketplace assets (for the "new in the marketplace" strip).
    pub assets: Vec<AssetSummary>,
    pub filters: FeedFilters,
    /// Usernames the signed-in user follows / befriends — fetched lazily the
    /// first time the audience filter needs them. `None` = not loaded yet.
    pub following: Option<HashSet<String>>,
    pub friends: Option<HashSet<String>>,
    /// Guards the lazy audience fetch against re-dispatch while in flight.
    pub audience_loading: bool,
    /// Comments for the currently expanded post.
    pub comments: HashMap<String, Vec<FeedComment>>,
    pub expanded: Option<String>,
    /// Posts whose long body is expanded via "See more" (id set). Body clamping
    /// is client-side, so this state — not anything from the API — decides it.
    pub body_expanded: HashSet<String>,
    /// Uploaded image URLs waiting to be attached to the next post.
    pub pending_media: Vec<String>,
    /// Two-step delete: the post id whose trash chip was clicked once, plus
    /// when — a second click within the window actually deletes.
    pub pending_delete: Option<(String, std::time::Instant)>,
    pub visibility: usize,
    pub at_end: bool,
    pub loading: bool,
    pub loading_more: bool,
    pub error: Option<String>,
    pub version: u64,
    pub loaded_once: bool,
    pub tx: Sender<FeedResult>,
    rx: Receiver<FeedResult>,
}

impl Default for FeedPanel {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self {
            stale: false,
            auto_follow: true,
            posts: Vec::new(),
            channels: Vec::new(),
            channels_loaded: false,
            active_channel: None,
            suggesting: false,
            assets: Vec::new(),
            filters: FeedFilters::default(),
            following: None,
            friends: None,
            audience_loading: false,
            comments: HashMap::new(),
            expanded: None,
            body_expanded: HashSet::new(),
            pending_media: Vec::new(),
            pending_delete: None,
            visibility: 0,
            at_end: false,
            loading: false,
            loading_more: false,
            error: None,
            version: 0,
            loaded_once: false,
            tx,
            rx,
        }
    }
}

impl FeedPanel {
    pub fn bump(&mut self) {
        self.version = self.version.wrapping_add(1);
    }
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<FeedPanel>();
    app.register_shell_panel(PANEL_ID, "Feed", "newspaper", "Community");
    // scroll: false — the panel scrolls its columns itself so the channels rail
    // stays fixed while only the center stream scrolls (like the marketplace).
    app.register_panel_content(PANEL_ID, false, build);
    app.add_systems(
        Update,
        (
            poll_results,
            auto_refresh.run_if(panel_active(PANEL_ID)),
            auto_follow_new_posts.run_if(panel_active(PANEL_ID)),
            composer_clicks,
            clicks,
            see_more_clicks,
            filter_clicks,
            channel_clicks,
            consume_request,
        )
            .run_if(in_state(SplashState::Editor)),
    );
}

/// Live mode: pull new posts in automatically when the WS marks the feed stale,
/// so fresh posts appear at the top without a manual refresh. Gated on the feed
/// being the active panel — no point re-fetching a feed nobody's looking at.
fn auto_follow_new_posts(mut panel: ResMut<FeedPanel>, session: Res<AuthSession>) {
    if panel.auto_follow && panel.stale && !panel.loading && !panel.loading_more {
        refresh(&mut panel, &session);
    }
}

// ── Fetching ─────────────────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
fn spawn_thread(f: impl FnOnce() + Send + 'static) {
    std::thread::spawn(f);
}
#[cfg(target_arch = "wasm32")]
fn spawn_thread(_f: impl FnOnce() + Send + 'static) {}

/// One-shot baseline fetch; marks `loaded_once` at SPAWN time so a failure can
/// never auto-retry — new posts show the WS-driven refresh pill instead.
/// Also refreshes the other stream sources (forum threads, marketplace) so
/// toggling their filter chips is instant.
fn refresh(panel: &mut FeedPanel, session: &AuthSession) {
    if !session.is_signed_in() {
        return;
    }
    panel.loaded_once = true;
    panel.loading = true;
    panel.stale = false;
    panel.at_end = false;
    panel.error = None;
    let channel = panel.active_channel.clone();
    let tx = panel.tx.clone();
    let s = session_clone(session);
    spawn_thread(move || {
        let _ = tx.send(FeedResult::Page(false, renzora_auth::feed::get_feed(&s, None, PAGE_SIZE, channel.as_deref())));
    });
    // Channels: fetched once (the rail + composer target don't change per refresh).
    if !panel.channels_loaded {
        panel.channels_loaded = true;
        let tx = panel.tx.clone();
        let s = session_clone(session);
        spawn_thread(move || {
            let _ = tx.send(FeedResult::Channels(renzora_auth::feed::list_channels(&s).unwrap_or_default()));
        });
    }
    // Marketplace: newest first, page 1.
    let tx = panel.tx.clone();
    spawn_thread(move || {
        let assets = renzora_auth::marketplace::list_assets(None, None, Some("newest"), 1, None, None)
            .map(|r| r.assets)
            .unwrap_or_default();
        let _ = tx.send(FeedResult::Assets(assets));
    });
}

/// Lazily fetch the following/friends username sets the audience filter needs.
fn ensure_audience(panel: &mut FeedPanel, session: &AuthSession) {
    if panel.audience_loading || (panel.following.is_some() && panel.friends.is_some()) {
        return;
    }
    let Some(me) = session.user.as_ref().map(|u| u.username.clone()) else { return };
    panel.audience_loading = true;
    let tx = panel.tx.clone();
    let s = session_clone(session);
    spawn_thread(move || {
        let following = renzora_auth::social::get_following(Some(&s), &me)
            .ok()
            .map(|v| v.into_iter().map(|u| u.username).collect::<HashSet<_>>());
        let friends = renzora_auth::social::get_friends(&s)
            .ok()
            .map(|v| v.into_iter().map(|f| f.username).collect::<HashSet<_>>());
        let _ = tx.send(FeedResult::Audience(following, friends));
    });
}

fn load_more(panel: &mut FeedPanel, session: &AuthSession) {
    let Some(last) = panel.posts.last().map(|p| p.id.clone()) else { return };
    if panel.loading_more || panel.at_end {
        return;
    }
    panel.loading_more = true;
    let channel = panel.active_channel.clone();
    let tx = panel.tx.clone();
    let s = session_clone(session);
    spawn_thread(move || {
        let _ = tx.send(FeedResult::Page(true, renzora_auth::feed::get_feed(&s, Some(&last), PAGE_SIZE, channel.as_deref())));
    });
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn poll_results(
    mut panel: ResMut<FeedPanel>,
    session: Res<AuthSession>,
    mut toasts: ResMut<ToastQueue>,
    // Profile shows the same post cards; comment/like results re-render it too.
    mut profile: ResMut<crate::panels::profile::ProfilePanel>,
) {
    let mut got = Vec::new();
    while let Ok(r) = panel.rx.try_recv() {
        got.push(r);
    }
    for r in got {
        match r {
            FeedResult::Page(append, Ok(page)) => {
                if page.len() < PAGE_SIZE as usize {
                    panel.at_end = true;
                }
                if append {
                    panel.loading_more = false;
                    panel.posts.extend(page);
                } else {
                    panel.loading = false;
                    panel.loaded_once = true;
                    panel.posts = page;
                }
                panel.bump();
            }
            FeedResult::Page(append, Err(e)) => {
                panel.loading = false;
                if append {
                    panel.loading_more = false;
                }
                panel.error = Some(e);
                panel.bump();
            }
            FeedResult::Posted(Ok(())) => {
                toasts.push(Tone::Success, "Posted", None);
                refresh(&mut panel, &session);
            }
            FeedResult::Posted(Err(e)) => {
                toasts.push(Tone::Error, format!("Post failed: {e}"), None);
            }
            FeedResult::LikeFailed => refresh(&mut panel, &session),
            FeedResult::Comments(post_id, Ok(list)) => {
                panel.comments.insert(post_id, list);
                panel.bump();
                profile.bump();
            }
            FeedResult::Comments(_, Err(e)) => {
                toasts.push(Tone::Error, e, None);
            }
            FeedResult::Commented(post_id, Ok(())) => {
                // Refetch the thread + bump the count locally (in both the
                // feed's copy of the post and the profile's).
                if let Some(p) = panel.posts.iter_mut().find(|p| p.id == post_id) {
                    p.comment_count += 1;
                }
                if let Some(p) = profile.posts.iter_mut().find(|p| p.id == post_id) {
                    p.comment_count += 1;
                }
                let tx = panel.tx.clone();
                let s = session_clone(&session);
                spawn_thread(move || {
                    let _ = tx.send(FeedResult::Comments(
                        post_id.clone(),
                        renzora_auth::feed::get_comments(&s, &post_id, 50, 0),
                    ));
                });
                panel.bump();
                profile.bump();
            }
            FeedResult::Commented(_, Err(e)) => {
                toasts.push(Tone::Error, format!("Comment failed: {e}"), None);
            }
            FeedResult::Uploaded(Ok(url)) => {
                panel.pending_media.push(url);
                toasts.push(Tone::Success, "Image attached to your next post", None);
                panel.bump();
            }
            FeedResult::Uploaded(Err(e)) => {
                toasts.push(Tone::Error, format!("Upload failed: {e}"), None);
            }
            FeedResult::Deleted(Ok(())) => {
                toasts.push(Tone::Success, "Post deleted", None);
            }
            FeedResult::Deleted(Err(e)) => {
                // The optimistic removal was wrong — refetch to restore it.
                toasts.push(Tone::Error, format!("Delete failed: {e}"), None);
                refresh(&mut panel, &session);
            }
            FeedResult::Channels(channels) => {
                panel.channels = channels;
                panel.bump();
            }
            FeedResult::Suggested(Ok(())) => {
                panel.suggesting = false;
                toasts.push(Tone::Success, "Channel suggested — an admin will review it", None);
                panel.bump();
            }
            FeedResult::Suggested(Err(e)) => {
                toasts.push(Tone::Error, format!("Couldn't suggest channel: {e}"), None);
            }
            FeedResult::Reported(Ok(hidden)) => {
                if hidden {
                    toasts.push(Tone::Success, "Reported — this post is now hidden pending review", None);
                    // A hide changes the feed; refetch so it disappears.
                    refresh(&mut panel, &session);
                } else {
                    toasts.push(Tone::Success, "Thanks — this post has been reported", None);
                }
            }
            FeedResult::Reported(Err(e)) => {
                toasts.push(Tone::Error, format!("Report failed: {e}"), None);
            }
            FeedResult::Assets(assets) => {
                panel.assets = assets;
                panel.bump();
            }
            FeedResult::Audience(following, friends) => {
                panel.audience_loading = false;
                panel.following = Some(following.unwrap_or_default());
                panel.friends = Some(friends.unwrap_or_default());
                panel.bump();
            }
        }
    }
}

fn auto_refresh(mut panel: ResMut<FeedPanel>, session: Res<AuthSession>) {
    if session.is_signed_in() && !panel.loaded_once {
        refresh(&mut panel, &session);
    }
}

fn consume_request(
    mut pending: ResMut<PendingSocialRequest>,
    mut panel: ResMut<FeedPanel>,
    session: Res<AuthSession>,
) {
    if !matches!(pending.0, Some(SocialPanelRequest::Feed { .. })) {
        return;
    }
    let Some(SocialPanelRequest::Feed { post_id }) = pending.0.take() else {
        return;
    };
    // A mention/comment notification deep-links to one post: expand its
    // comments so the linked content is visible on arrival, and make sure the
    // feed is fresh enough to contain it.
    if let Some(id) = post_id {
        if !panel.comments.contains_key(&id) {
            let tx = panel.tx.clone();
            let s = session_clone(&session);
            let cid = id.clone();
            spawn_thread(move || {
                let _ = tx.send(FeedResult::Comments(
                    cid.clone(),
                    renzora_auth::feed::get_comments(&s, &cid, 50, 0),
                ));
            });
        }
        panel.expanded = Some(id.clone());
        if !panel.posts.iter().any(|p| p.id == id) {
            refresh(&mut panel, &session);
        }
        panel.bump();
    }
}

// ── Clicks ───────────────────────────────────────────────────────────────────

#[derive(Component)]
struct ComposerInput;
#[derive(Component)]
struct PostBtn;
#[derive(Component)]
struct AttachImageBtn;
#[derive(Component)]
struct VisibilityDropdown;
#[derive(Component)]
struct StalePill;
/// The Live/Paused toggle in the feed header.
#[derive(Component)]
struct PlayPauseBtn;
#[derive(Component)]
struct LikeBtn(String);
#[derive(Component)]
struct CommentsBtn(String);
/// "See more"/"See less" toggle on a long post body.
#[derive(Component)]
struct SeeMoreBtn(String);
#[derive(Component)]
struct CommentInput;
#[derive(Component)]
struct CommentSendBtn(String);
#[derive(Component)]
struct LoadMoreBtn;
#[derive(Component)]
struct UserBtn(String);
/// Trash chip on a post (own posts, or any post for moderators).
#[derive(Component)]
struct DeleteBtn(String);
/// Filter chips toggling a stream source: 0 = posts, 2 = marketplace.
#[derive(Component)]
struct SourceToggleBtn(u8);
#[derive(Component)]
struct SortDropdown;
#[derive(Component)]
struct TimeDropdown;
#[derive(Component)]
struct AudienceDropdown;
/// A channel row in the left rail. `None` = the "All" pseudo-channel.
#[derive(Component)]
struct ChannelBtn(Option<String>);
/// The "+ Suggest a channel" toggle in the rail.
#[derive(Component)]
struct SuggestBtn;
/// The suggest-a-channel name input.
#[derive(Component)]
struct SuggestInput;
/// Submit a channel suggestion.
#[derive(Component)]
struct SuggestSendBtn;
/// Report a post (flag chip).
#[derive(Component)]
struct ReportBtn(String);
/// Ask staff to review one of your own hidden posts.
#[derive(Component)]
struct RequestReviewBtn(String);
/// A marketplace card in the "new assets" strip — opens the store.
#[derive(Component)]
struct AssetOpenBtn;

/// Composer interactions: write a post, attach an image, pick visibility.
#[allow(clippy::type_complexity)]
fn composer_clicks(
    mut panel: ResMut<FeedPanel>,
    session: Res<AuthSession>,
    posts: Query<&Interaction, (With<PostBtn>, Changed<Interaction>)>,
    attaches: Query<&Interaction, (With<AttachImageBtn>, Changed<Interaction>)>,
    vis: Query<&Bound<usize>, (With<VisibilityDropdown>, Changed<Bound<usize>>)>,
    mut composer: Query<&mut EmberTextInput, (With<ComposerInput>, Without<CommentInput>)>,
) {
    let pressed = |i: &Interaction| *i == Interaction::Pressed;

    for i in &posts {
        if pressed(i) {
            if let Ok(mut input) = composer.single_mut() {
                let body = input.value.trim().to_string();
                if body.is_empty() {
                    continue;
                }
                input.value.clear();
                let visibility = VISIBILITIES[panel.visibility].to_string();
                let media = std::mem::take(&mut panel.pending_media);
                let channel = panel.active_channel.clone();
                let tx = panel.tx.clone();
                let s = session_clone(&session);
                spawn_thread(move || {
                    let r = renzora_auth::feed::create_post(&s, &body, &visibility, &media, channel.as_deref()).map(|_| ());
                    let _ = tx.send(FeedResult::Posted(r));
                });
            }
        }
    }
    for b in &vis {
        if b.0 < VISIBILITIES.len() {
            panel.visibility = b.0;
        }
    }
    for i in &attaches {
        if pressed(i) {
            let tx = panel.tx.clone();
            let s = session_clone(&session);
            spawn_thread(move || {
                let Some(path) = rfd::FileDialog::new()
                    .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif"])
                    .pick_file()
                else {
                    return;
                };
                let result = (|| -> Result<String, String> {
                    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
                    if bytes.len() > 5 * 1024 * 1024 {
                        return Err("Image is larger than 5 MB".into());
                    }
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("image.png").to_string();
                    let ct = match path.extension().and_then(|e| e.to_str()).unwrap_or("png") {
                        "jpg" | "jpeg" => "image/jpeg",
                        "webp" => "image/webp",
                        "gif" => "image/gif",
                        _ => "image/png",
                    };
                    renzora_auth::feed::upload_image(&s, &name, ct, &bytes).map(|r| r.url)
                })();
                let _ = tx.send(FeedResult::Uploaded(result));
            });
        }
    }
}

/// Post-card interactions (shared with the Profile panel, which renders the
/// same cards): like, expand comments, reply, open profile, delete. Every
/// optimistic mutation mirrors into `ProfilePanel.posts` too, since the same
/// post can be on screen in both panels at once.
#[allow(clippy::too_many_arguments)]
fn clicks(
    mut panel: ResMut<FeedPanel>,
    mut profile: ResMut<crate::panels::profile::ProfilePanel>,
    session: Res<AuthSession>,
    mut bridge: ResMut<SocialBridge>,
    stale: Query<&Interaction, (With<StalePill>, Changed<Interaction>)>,
    play_pause: Query<&Interaction, (With<PlayPauseBtn>, Changed<Interaction>)>,
    likes: Query<(&Interaction, &LikeBtn), Changed<Interaction>>,
    comments: Query<(&Interaction, &CommentsBtn), Changed<Interaction>>,
    comment_sends: Query<(&Interaction, &CommentSendBtn), Changed<Interaction>>,
    more: Query<&Interaction, (With<LoadMoreBtn>, Changed<Interaction>)>,
    users: Query<(&Interaction, &UserBtn), Changed<Interaction>>,
    deletes: Query<(&Interaction, &DeleteBtn), Changed<Interaction>>,
    reports: Query<(&Interaction, &ReportBtn), Changed<Interaction>>,
    reviews: Query<(&Interaction, &RequestReviewBtn), Changed<Interaction>>,
    mut toasts: ResMut<ToastQueue>,
    mut comment_input: Query<&mut EmberTextInput, (With<CommentInput>, Without<ComposerInput>)>,
) {
    let pressed = |i: &Interaction| *i == Interaction::Pressed;

    for i in &stale {
        if pressed(i) {
            refresh(&mut panel, &session);
        }
    }
    for i in &play_pause {
        if pressed(i) {
            panel.auto_follow = !panel.auto_follow;
            // Resuming Live with posts already waiting → pull them in now.
            if panel.auto_follow && panel.stale {
                refresh(&mut panel, &session);
            }
        }
    }
    for (i, b) in &likes {
        if pressed(i) {
            // Optimistic toggle, in every copy of the post.
            if let Some(p) = panel.posts.iter_mut().find(|p| p.id == b.0) {
                p.is_liked = !p.is_liked;
                p.like_count += if p.is_liked { 1 } else { -1 };
            }
            if let Some(p) = profile.posts.iter_mut().find(|p| p.id == b.0) {
                p.is_liked = !p.is_liked;
                p.like_count += if p.is_liked { 1 } else { -1 };
            }
            panel.bump();
            profile.bump();
            let tx = panel.tx.clone();
            let s = session_clone(&session);
            let id = b.0.clone();
            spawn_thread(move || {
                if renzora_auth::feed::like_post(&s, &id).is_err() {
                    let _ = tx.send(FeedResult::LikeFailed);
                }
            });
        }
    }
    for (i, b) in &comments {
        if pressed(i) {
            if panel.expanded.as_deref() == Some(b.0.as_str()) {
                panel.expanded = None;
            } else {
                panel.expanded = Some(b.0.clone());
                if !panel.comments.contains_key(&b.0) {
                    let tx = panel.tx.clone();
                    let s = session_clone(&session);
                    let id = b.0.clone();
                    spawn_thread(move || {
                        let _ = tx.send(FeedResult::Comments(
                            id.clone(),
                            renzora_auth::feed::get_comments(&s, &id, 50, 0),
                        ));
                    });
                }
            }
            panel.bump();
            profile.bump();
        }
    }
    for (i, b) in &comment_sends {
        if pressed(i) {
            // The same expanded post can have a composer in the feed AND the
            // profile — send from whichever one was typed into.
            let Some(mut input) = comment_input.iter_mut().find(|i| !i.value.trim().is_empty())
            else {
                continue;
            };
            let body = input.value.trim().to_string();
            input.value.clear();
            let tx = panel.tx.clone();
            let s = session_clone(&session);
            let id = b.0.clone();
            spawn_thread(move || {
                let r = renzora_auth::feed::post_comment(&s, &id, &body, None).map(|_| ());
                let _ = tx.send(FeedResult::Commented(id, r));
            });
        }
    }
    for i in &more {
        if pressed(i) {
            load_more(&mut panel, &session);
        }
    }
    for (i, b) in &users {
        if pressed(i) {
            bridge.open_panel_request = Some(SocialPanelRequest::Profile { username: Some(b.0.clone()) });
        }
    }
    for (i, b) in &deletes {
        if !pressed(i) {
            continue;
        }
        // Two-step delete: first click arms, second click (within 4s, same
        // post) actually deletes — no modal needed, no accidental deletes.
        let armed = panel
            .pending_delete
            .as_ref()
            .is_some_and(|(id, at)| *id == b.0 && at.elapsed().as_secs_f32() < 4.0);
        if !armed {
            panel.pending_delete = Some((b.0.clone(), std::time::Instant::now()));
            toasts.push(Tone::Warn, "Click delete again to confirm", None);
            continue;
        }
        panel.pending_delete = None;
        // Optimistic removal; a server error refetches (see Deleted arm).
        panel.posts.retain(|p| p.id != b.0);
        profile.posts.retain(|p| p.id != b.0);
        panel.bump();
        profile.bump();
        let tx = panel.tx.clone();
        let s = session_clone(&session);
        let id = b.0.clone();
        spawn_thread(move || {
            let r = renzora_auth::feed::delete_post(&s, &id).map(|_| ());
            let _ = tx.send(FeedResult::Deleted(r));
        });
    }
    // Report a post (network-wide moderation; enough reports auto-hide it).
    for (i, b) in &reports {
        if pressed(i) {
            let tx = panel.tx.clone();
            let s = session_clone(&session);
            let id = b.0.clone();
            spawn_thread(move || {
                let hidden = renzora_auth::feed::report_post(&s, &id, "")
                    .map(|v| v.get("hidden").and_then(|x| x.as_bool()).unwrap_or(false));
                let _ = tx.send(FeedResult::Reported(hidden));
            });
        }
    }
    // Ask staff to review your own hidden post.
    for (i, b) in &reviews {
        if pressed(i) {
            if let Some(p) = panel.posts.iter_mut().find(|p| p.id == b.0) {
                p.review_requested = true;
            }
            if let Some(p) = profile.posts.iter_mut().find(|p| p.id == b.0) {
                p.review_requested = true;
            }
            panel.bump();
            profile.bump();
            toasts.push(Tone::Success, "Review requested — staff will take a look", None);
            let s = session_clone(&session);
            let id = b.0.clone();
            spawn_thread(move || {
                let _ = renzora_auth::feed::request_review(&s, &id);
            });
        }
    }
}

/// "See more"/"See less" on a long post body: toggle that post's body
/// expansion. Its own system because `clicks` is already at Bevy's 16-param
/// tuple cap. Mirrors the bump into the profile, which renders the same cards.
fn see_more_clicks(
    mut panel: ResMut<FeedPanel>,
    mut profile: ResMut<crate::panels::profile::ProfilePanel>,
    toggles: Query<(&Interaction, &SeeMoreBtn), Changed<Interaction>>,
) {
    for (i, b) in &toggles {
        if *i == Interaction::Pressed {
            if !panel.body_expanded.remove(&b.0) {
                panel.body_expanded.insert(b.0.clone());
            }
            panel.bump();
            profile.bump();
        }
    }
}

/// Filter-bar interactions + opening stream items that live in other panels.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn filter_clicks(
    mut panel: ResMut<FeedPanel>,
    session: Res<AuthSession>,
    dock: Option<ResMut<renzora_ember::dock::Dock>>,
    dock_dirty: Option<ResMut<renzora_ember::dock::DockDirty>>,
    sources: Query<(&Interaction, &SourceToggleBtn), Changed<Interaction>>,
    sorts: Query<&Bound<usize>, (With<SortDropdown>, Changed<Bound<usize>>)>,
    times: Query<&Bound<usize>, (With<TimeDropdown>, Changed<Bound<usize>>)>,
    audiences: Query<&Bound<usize>, (With<AudienceDropdown>, Changed<Bound<usize>>)>,
    assets: Query<&Interaction, (With<AssetOpenBtn>, Changed<Interaction>)>,
) {
    let pressed = |i: &Interaction| *i == Interaction::Pressed;

    for (i, b) in &sources {
        if pressed(i) {
            let f = &mut panel.filters;
            match b.0 {
                0 => f.show_posts = !f.show_posts,
                _ => f.show_market = !f.show_market,
            }
            panel.bump();
        }
    }
    for b in &sorts {
        if b.0 < FEED_SORTS.len() && panel.filters.sort != b.0 {
            panel.filters.sort = b.0;
            panel.bump();
        }
    }
    for b in &times {
        if b.0 < FEED_TIMES.len() && panel.filters.timeframe != b.0 {
            panel.filters.timeframe = b.0;
            panel.bump();
        }
    }
    for b in &audiences {
        if b.0 < FEED_AUDIENCES.len() && panel.filters.audience != b.0 {
            panel.filters.audience = b.0;
            if b.0 != 0 {
                ensure_audience(&mut panel, &session);
            }
            panel.bump();
        }
    }
    if assets.iter().any(pressed) {
        if let Some(mut dock) = dock {
            dock.tree.focus_or_add_panel("hub_store");
            // Arm a rebuild — mutating the tree alone doesn't refresh the shell
            // (that was the "shows only after I change the theme" bug).
            if let Some(mut dirty) = dock_dirty {
                dirty.0 = true;
            }
        }
    }
}

/// Left-rail interactions: pick a channel to filter/post into, and the
/// suggest-a-channel flow.
#[allow(clippy::type_complexity)]
fn channel_clicks(
    mut panel: ResMut<FeedPanel>,
    session: Res<AuthSession>,
    channels: Query<(&Interaction, &ChannelBtn), Changed<Interaction>>,
    suggest_toggles: Query<&Interaction, (With<SuggestBtn>, Changed<Interaction>)>,
    suggest_sends: Query<&Interaction, (With<SuggestSendBtn>, Changed<Interaction>)>,
    mut suggest_input: Query<&mut EmberTextInput, With<SuggestInput>>,
) {
    let pressed = |i: &Interaction| *i == Interaction::Pressed;
    for (i, b) in &channels {
        if pressed(i) && panel.active_channel != b.0 {
            panel.active_channel = b.0.clone();
            refresh(&mut panel, &session);
            break;
        }
    }
    if suggest_toggles.iter().any(pressed) {
        panel.suggesting = !panel.suggesting;
        panel.bump();
    }
    if suggest_sends.iter().any(pressed) {
        if let Some(mut input) = suggest_input.iter_mut().next() {
            let name = input.value.trim().to_string();
            if !name.is_empty() {
                input.value.clear();
                let tx = panel.tx.clone();
                let s = session_clone(&session);
                spawn_thread(move || {
                    let r = renzora_auth::feed::suggest_channel(&s, &name, "", "ph-hash").map(|_| ());
                    let _ = tx.send(FeedResult::Suggested(r));
                });
            }
        }
    }
}

// ── Build ────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            // Fill the panel so the inner columns can scroll independently.
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        })
        .id();

    let signed_out = empty_state(
        commands,
        fonts,
        HUE_FEED,
        "newspaper",
        "Sign in to join the feed",
        Some("See what the community is building — and show off your own work"),
    );
    bind_display(commands, signed_out, |w| !util::signed_in(w));

    let body = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() })
        .id();
    bind_display(commands, body, util::signed_in);

    // Compact neutral header: title + refresh. (The old full-width tinted
    // banner flooded the panel with its hue — the content is the hero now.)
    let head = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            padding: UiRect::axes(Val::Px(2.0), Val::Px(2.0)),
            ..default()
        })
        .id();
    let head_icon = icon_text(commands, &fonts.phosphor, "newspaper", text_muted(), 15.0);
    let head_col = commands
        .spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, ..default() })
        .id();
    let head_title = commands
        .spawn((Text::new("Community Feed"), ui_font(&fonts.ui, 13.5), TextColor(rgb(text_primary()))))
        .id();
    let head_sub = commands
        .spawn((Text::new("What everyone's building right now"), ui_font(&fonts.ui, 9.5), TextColor(rgb(placeholder()))))
        .id();
    commands.entity(head_col).add_children(&[head_title, head_sub]);
    // Live / Paused toggle (replaces the manual refresh). Hand-rolled like the
    // source chips so its state stays live via bindings on a build-once header:
    // accent-tinted when live, and the icon swaps pause↔play by display-toggle.
    let toggle = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(9.0), Val::Px(4.5)),
                border_radius: BorderRadius::all(Val::Px(13.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            PlayPauseBtn,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            HoverTooltip::new("New posts appear live — click to pause".to_string()),
        ))
        .id();
    renzora_ember::reactive::bind_bg(commands, toggle, |w| {
        let live = w.get_resource::<FeedPanel>().map(|p| p.auto_follow).unwrap_or(true);
        if live { rgb(accent()).with_alpha(0.30) } else { rgba([255, 255, 255, 10]) }
    });
    let pause_ic = icon_text(commands, &fonts.phosphor, "pause", accent(), 13.0);
    bind_display(commands, pause_ic, |w| {
        w.get_resource::<FeedPanel>().map(|p| p.auto_follow).unwrap_or(true)
    });
    let play_ic = icon_text(commands, &fonts.phosphor, "play", text_primary(), 13.0);
    bind_display(commands, play_ic, |w| {
        !w.get_resource::<FeedPanel>().map(|p| p.auto_follow).unwrap_or(true)
    });
    let toggle_label = commands
        .spawn((Text::new("Live"), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted()))))
        .id();
    bind_text(commands, toggle_label, |w| {
        let live = w.get_resource::<FeedPanel>().map(|p| p.auto_follow).unwrap_or(true);
        if live { "Live".to_string() } else { "Paused".to_string() }
    });
    commands.entity(toggle).add_children(&[pause_ic, play_ic, toggle_label]);
    commands.entity(head).add_children(&[head_icon, head_col, toggle]);

    // Composer — the invitation to post. Matches the input surface so it
    // reads as one writing area; Enter makes a new line (textarea).
    let composer = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgba([255, 255, 255, 12])),
        ))
        .id();
    let input = textarea(commands, &fonts.ui, "Share what you're building... (@name to tag someone)", "");
    commands.entity(input).insert((ComposerInput, Node { width: Val::Percent(100.0), min_height: Val::Px(56.0), ..default() }));
    let controls = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() })
        .id();
    let attach = util::action_chip(commands, fonts, "image", None, false, Some("Attach an image".to_string()));
    commands.entity(attach).insert(AttachImageBtn);
    let media_note = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 9.5), TextColor(rgb(text_muted())), Node { flex_grow: 1.0, ..default() }))
        .id();
    bind_text(commands, media_note, |w| {
        let n = w.get_resource::<FeedPanel>().map(|p| p.pending_media.len()).unwrap_or(0);
        match n {
            0 => String::new(),
            1 => "1 image attached".to_string(),
            n => format!("{n} images attached"),
        }
    });
    let vis = dropdown(commands, fonts, &["Public", "Followers", "Friends"], 0);
    commands.entity(vis).insert(VisibilityDropdown);
    let post_btn = util::pill_button(commands, fonts, "Post", accent(), (255, 255, 255));
    commands.entity(post_btn).insert(PostBtn);
    commands.entity(controls).add_children(&[attach, media_note, vis, post_btn]);
    // Which channel the post goes into (follows the selected channel in the rail).
    let ch_hint = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 9.0), TextColor(rgb(placeholder()))))
        .id();
    bind_text(commands, ch_hint, |w| {
        match w.get_resource::<FeedPanel>().and_then(|p| p.active_channel.clone()) {
            Some(slug) => format!("Posting to #{slug}"),
            None => "Posting to the main feed".to_string(),
        }
    });
    commands.entity(composer).add_children(&[input, controls, ch_hint]);

    // ── Filter bar: source chips + sort / time / audience dropdowns. ──
    let filter_bar = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    for (idx, icon, label) in [
        (0u8, "newspaper", "Posts"),
        (2u8, "storefront", "Marketplace"),
    ] {
        // Hand-rolled chip (not `action_chip`) because the active state must
        // stay live via bind_bg — the bar is built once, not per snapshot.
        let chip = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(5.0),
                    padding: UiRect::axes(Val::Px(9.0), Val::Px(4.5)),
                    border_radius: BorderRadius::all(Val::Px(13.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                SourceToggleBtn(idx),
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                HoverTooltip::new(format!("Show or hide {label}")),
            ))
            .id();
        renzora_ember::reactive::bind_bg(commands, chip, move |w| {
            let on = w
                .get_resource::<FeedPanel>()
                .map(|p| match idx {
                    0 => p.filters.show_posts,
                    _ => p.filters.show_market,
                })
                .unwrap_or(true);
            if on {
                rgb(accent()).with_alpha(0.30)
            } else {
                rgba([255, 255, 255, 8])
            }
        });
        let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 12.0);
        let t = commands
            .spawn((Text::new(label), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_primary()))))
            .id();
        commands.entity(chip).add_children(&[ic, t]);
        commands.entity(filter_bar).add_child(chip);
    }
    let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let sort_dd = dropdown(commands, fonts, &FEED_SORTS, 0);
    commands.entity(sort_dd).insert(SortDropdown);
    let time_dd = dropdown(commands, fonts, &FEED_TIMES, 0);
    commands.entity(time_dd).insert(TimeDropdown);
    let aud_dd = dropdown(commands, fonts, &FEED_AUDIENCES, 0);
    commands.entity(aud_dd).insert(AudienceDropdown);
    commands.entity(filter_bar).add_children(&[spacer, sort_dd, time_dd, aud_dd]);

    // Stale pill — fresh posts are waiting upstream. Only shown when PAUSED;
    // in Live mode they're pulled in automatically (see `auto_follow_new_posts`).
    let pill = util::pill_button(commands, fonts, "New posts — see what's fresh", accent(), (255, 255, 255));
    commands.entity(pill).insert(StalePill);
    bind_display(commands, pill, |w| {
        w.get_resource::<FeedPanel>().map(|p| p.stale && !p.auto_follow).unwrap_or(false)
    });

    // Error line.
    let error = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.5), TextColor(rgb(RED))))
        .id();
    bind_text(commands, error, |w| {
        w.get_resource::<FeedPanel>().and_then(|p| p.error.clone()).unwrap_or_default()
    });
    bind_display(commands, error, |w| {
        w.get_resource::<FeedPanel>().map(|p| p.error.is_some()).unwrap_or(false)
    });

    // Posts.
    let list = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() })
        .id();
    keyed_list_tokened(
        commands,
        list,
        |w| w.get_resource::<FeedPanel>().map(|p| p.version).unwrap_or(0),
        snapshot,
    );

    // Load more.
    let more = util::action_chip(commands, fonts, "arrow-down", Some("Load more"), false, None);
    commands.entity(more).insert(LoadMoreBtn);
    bind_display(commands, more, |w| {
        w.get_resource::<FeedPanel>()
            .map(|p| !p.posts.is_empty() && !p.at_end)
            .unwrap_or(false)
    });

    // ── Two columns: fixed channels rail (left) + the scrolling stream (right). ──
    let rail = build_channels_rail(commands, fonts);
    // The center's natural-height content (composer → filters → posts), wrapped in
    // its own scroll view so only this column scrolls — the rail stays put.
    let center_content = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() })
        .id();
    commands.entity(center_content).add_children(&[composer, filter_bar, pill, error, list, more]);
    let center_scroll = renzora_ember::widgets::scroll_view(commands, center_content);
    let right = commands
        .spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), min_height: Val::Px(0.0), flex_direction: FlexDirection::Column, ..default() })
        .id();
    commands.entity(right).add_child(center_scroll);
    let cols = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(0.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Stretch, column_gap: Val::Px(8.0), ..default() })
        .id();
    commands.entity(cols).add_children(&[rail, right]);

    commands.entity(body).add_children(&[head, cols]);
    commands.entity(root).add_children(&[signed_out, body]);
    root
}

/// The left rail: a "Channels" header, the channel list (All + each channel),
/// and a "suggest a channel" toggle + input.
fn build_channels_rail(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // Narrower than before (was 150) so the center stream gets more width.
    let rail = commands
        .spawn(Node { width: Val::Px(132.0), flex_shrink: 0.0, min_height: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() })
        .id();
    let header = commands
        .spawn((Text::new("CHANNELS"), ui_font(&fonts.ui, 9.0), TextColor(rgb(text_muted())), Node { flex_shrink: 0.0, margin: UiRect::bottom(Val::Px(2.0)), ..default() }))
        .id();
    let list = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() })
        .id();
    keyed_list_tokened(
        commands,
        list,
        |w| w.get_resource::<FeedPanel>().map(|p| p.version).unwrap_or(0),
        channels_snapshot,
    );
    // The channel list scrolls within the rail if it's long; the rail itself
    // (and the panel around it) stays fixed.
    let list_scroll = renzora_ember::widgets::scroll_view(commands, list);

    // Suggest a channel: a toggle that reveals an inline name input + submit.
    let suggest_btn = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(5.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)), border_radius: BorderRadius::all(Val::Px(6.0)), margin: UiRect::top(Val::Px(4.0)), ..default() },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            SuggestBtn,
            renzora_ember::widgets::HoverTint::solid(Color::NONE, rgb(hover_bg()), rgb(hover_bg())),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    let plus = icon_text(commands, &fonts.phosphor, "plus", text_muted(), 11.0);
    commands.entity(plus).insert(FocusPolicy::Pass);
    let plus_t = commands
        .spawn((Text::new("Suggest a channel"), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())), FocusPolicy::Pass))
        .id();
    commands.entity(suggest_btn).add_children(&[plus, plus_t]);

    let suggest_row = commands
        .spawn((Node { width: Val::Percent(100.0), flex_shrink: 0.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), display: Display::None, ..default() },))
        .id();
    bind_display(commands, suggest_row, |w| w.get_resource::<FeedPanel>().map(|p| p.suggesting).unwrap_or(false));
    let sinput = text_input(commands, &fonts.ui, "Channel name…", "");
    commands.entity(sinput).insert((SuggestInput, Node { width: Val::Percent(100.0), ..default() }));
    let ssend = util::pill_button(commands, fonts, "Suggest", accent(), (255, 255, 255));
    commands.entity(ssend).insert(SuggestSendBtn);
    commands.entity(suggest_row).insert(EmberForm { submit: ssend });
    commands.entity(suggest_row).add_children(&[sinput, ssend]);

    commands.entity(rail).add_children(&[header, list_scroll, suggest_btn, suggest_row]);
    rail
}

/// The channel rail's keyed list: an "All" pseudo-channel then each live channel.
fn channels_snapshot(w: &World) -> KeyedSnapshot {
    let Some(panel) = w.get_resource::<FeedPanel>() else {
        return util::empty_snapshot();
    };
    let active = panel.active_channel.clone();
    let channels = panel.channels.clone();
    let mut items: Vec<(u64, u64)> = Vec::new();
    // (slug, name, icon) — None = the "All" row.
    let mut rows: Vec<Option<(String, String, String)>> = Vec::new();
    items.push((hash64(&"all"), hash64(&active.is_none())));
    rows.push(None);
    for c in &channels {
        let is_active = active.as_deref() == Some(c.slug.as_str());
        items.push((hash64(&c.slug), hash64(&is_active)));
        rows.push(Some((c.slug.clone(), c.name.clone(), c.icon.clone())));
    }
    KeyedSnapshot {
        items,
        build: Box::new(move |commands, fonts, i| channel_row(commands, fonts, i, &rows[i])),
    }
}

fn channel_row(commands: &mut Commands, fonts: &EmberFonts, idx: usize, row: &Option<(String, String, String)>) -> Entity {
    let (slug, label, icon) = match row {
        None => (None, "All".to_string(), "list-bullets".to_string()),
        Some((s, n, ic)) => (Some(s.clone()), n.clone(), clean_channel_icon(ic)),
    };
    // Fixed-height rows (like the marketplace category rail) so the list reads as
    // an even, uniform table rather than padded pills of varying height.
    let row_e = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(26.0), flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), padding: UiRect::horizontal(Val::Px(8.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            ChannelBtn(slug.clone()),
        ))
        .id();
    // Live background: active → accent tint, hovered → hover, else odd/even zebra
    // by row index — mirrors the marketplace category rail. Reading the resource
    // each frame keeps active/hover live without a rebuild.
    {
        let slug = slug.clone();
        renzora_ember::reactive::bind_bg(commands, row_e, move |w| {
            let is_active = w
                .get_resource::<FeedPanel>()
                .map(|p| p.active_channel == slug)
                .unwrap_or(false);
            if is_active {
                rgb(accent()).with_alpha(0.28)
            } else if matches!(w.get::<Interaction>(row_e), Some(Interaction::Hovered) | Some(Interaction::Pressed)) {
                rgb(hover_bg())
            } else if idx.is_multiple_of(2) {
                rgb(row_even())
            } else {
                rgb(row_odd())
            }
        });
    }
    // Colored icon + light label, like the marketplace category rail: the icon
    // carries a per-channel hue (accent for "All"), and the text stays light
    // (primary) regardless of selection rather than dimming when inactive.
    let icon_col = match &slug {
        None => accent(),
        Some(s) => channel_hue(s),
    };
    let ic = icon_text(commands, &fonts.phosphor, &icon, icon_col, 12.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let t = commands
        .spawn((Text::new(label), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_primary())), FocusPolicy::Pass, bevy::text::TextLayout::no_wrap(), Node { overflow: Overflow::clip(), ..default() }))
        .id();
    commands.entity(row_e).add_children(&[ic, t]);
    row_e
}

/// A stable, pleasant hue for a channel's icon — picked from a fixed palette by
/// hashing the slug, so each channel keeps the same color. Mirrors the
/// marketplace's colored category icons.
fn channel_hue(slug: &str) -> (u8, u8, u8) {
    const PALETTE: [(u8, u8, u8); 12] = [
        (91, 156, 245),  // blue
        (240, 140, 90),  // orange
        (80, 200, 190),  // teal
        (232, 182, 82),  // amber
        (240, 120, 160), // pink
        (120, 205, 120), // green
        (167, 130, 245), // violet
        (205, 130, 240), // magenta
        (100, 185, 250), // sky
        (130, 205, 165), // mint
        (150, 160, 250), // periwinkle
        (240, 165, 90),  // tangerine
    ];
    PALETTE[(hash64(&slug) % PALETTE.len() as u64) as usize]
}

/// Channel icons arrive as web classes (`ph-code`); the engine's glyph lookup
/// wants the bare kebab name (`code`).
fn clean_channel_icon(raw: &str) -> String {
    raw.split_whitespace().last().unwrap_or("hash").trim_start_matches("ph-").to_string()
}

// ── Snapshot / rows ──────────────────────────────────────────────────────────

/// The per-post keyed-list content hash: everything a rendered post card
/// displays that can change without the post id changing.
///
/// `fetched` must be part of it: a post with ZERO comments arrives as an
/// empty list, which leaves the count at 0 — without this bit the row never
/// rebuilds and "Loading comments..." sticks. Shared with the Profile panel's
/// activity list, which renders the same cards.
pub(crate) fn post_key(
    p: &FeedPost,
    expanded: Option<&str>,
    comments: &HashMap<String, Vec<FeedComment>>,
    body_open: bool,
) -> u64 {
    let is_open = expanded == Some(p.id.as_str());
    let fetched = comments.contains_key(&p.id);
    let n_comments = comments.get(&p.id).map(|c| c.len()).unwrap_or(0);
    let rx: i64 = p.reactions.iter().map(|r| r.count + r.reacted as i64).sum();
    hash64(&(&p.body, p.like_count, p.comment_count, p.is_liked, is_open, fetched, n_comments, rx, body_open))
}

/// One row of the unified stream, resolved by the snapshot's build closure.
enum StreamRow {
    /// A hint / empty-state line.
    Note(&'static str),
    /// Index into the cloned posts.
    Post(usize),
    /// The "new in the marketplace" strip.
    Market,
}

fn snapshot(w: &World) -> KeyedSnapshot {
    let Some(panel) = w.get_resource::<FeedPanel>() else {
        return util::empty_snapshot();
    };
    let f = panel.filters.clone();
    let posts = panel.posts.clone();
    let assets = panel.assets.clone();
    let expanded = panel.expanded.clone();
    let comments = panel.comments.clone();
    let body_expanded = panel.body_expanded.clone();
    // Who's looking decides which posts show a delete chip (own posts, or all
    // of them for site moderators).
    let (me, moderator) = w
        .get_resource::<AuthSession>()
        .map(|s| (s.user.as_ref().map(|u| u.username.clone()), util::is_moderator(s)))
        .unwrap_or((None, false));

    // ── Filter: time frame + audience (client-side; the API has no params). ──
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let cutoff = match f.timeframe {
        1 => now - 86_400,
        2 => now - 7 * 86_400,
        3 => now - 30 * 86_400,
        _ => i64::MIN,
    };
    let audience: Option<HashSet<String>> = match f.audience {
        1 => Some(panel.following.clone().unwrap_or_default()),
        2 => Some(panel.friends.clone().unwrap_or_default()),
        _ => None,
    };
    let audience_pending = f.audience != 0 && panel.audience_loading;

    // ── Merge posts + threads into one scored, timestamped list. ──
    let mut merged: Vec<(i64, i64, StreamRow)> = Vec::new();
    if f.show_posts {
        for (idx, p) in posts.iter().enumerate() {
            let ts = util::parse_timestamp(&p.created_at).unwrap_or(0);
            if ts < cutoff {
                continue;
            }
            if let Some(set) = &audience {
                if !set.contains(&p.username) {
                    continue;
                }
            }
            let rx: i64 = p.reactions.iter().map(|r| r.count).sum();
            merged.push((ts, p.like_count + p.comment_count * 2 + rx, StreamRow::Post(idx)));
        }
    }
    match f.sort {
        1 => merged.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0))),
        _ => merged.sort_by_key(|r| std::cmp::Reverse(r.0)),
    }

    // ── Assemble rows: marketplace strip first (assets carry no timestamps,
    // so they can't be interleaved honestly), then the merged stream. ──
    let mut rows: Vec<StreamRow> = Vec::new();
    let mut items: Vec<(u64, u64)> = Vec::new();
    if f.show_market && !assets.is_empty() {
        let ids: Vec<&str> = assets.iter().map(|a| a.id.as_str()).collect();
        items.push((hash64(&"market-strip"), hash64(&ids)));
        rows.push(StreamRow::Market);
    }
    if audience_pending {
        items.push((hash64(&"audience-note"), 0));
        rows.push(StreamRow::Note("Loading who you follow…"));
    }
    for (_, _, row) in merged {
        if let StreamRow::Post(i) = row {
            let body_open = body_expanded.contains(&posts[i].id);
            items.push((hash64(&posts[i].id), post_key(&posts[i], expanded.as_deref(), &comments, body_open)));
            rows.push(StreamRow::Post(i));
        }
    }
    if rows.is_empty() {
        let loading = panel.loading;
        return KeyedSnapshot {
            items: vec![(u64::MAX, loading as u64)],
            build: Box::new(move |commands, fonts, _| {
                if loading {
                    commands
                        .spawn((Text::new("Loading..."), ui_font(&fonts.ui, 11.0), TextColor(rgb(placeholder()))))
                        .id()
                } else {
                    empty_state(
                        commands,
                        fonts,
                        HUE_FEED,
                        "rocket-launch",
                        "Nothing here",
                        Some("Loosen the filters, follow more people — or be the first to post"),
                    )
                }
            }),
        };
    }

    KeyedSnapshot {
        items,
        build: Box::new(move |commands, fonts, i| match &rows[i] {
            StreamRow::Note(msg) => commands
                .spawn((Text::new(*msg), ui_font(&fonts.ui, 10.5), TextColor(rgb(placeholder()))))
                .id(),
            StreamRow::Market => market_strip(commands, fonts, &assets),
            StreamRow::Post(i) => {
                let p = &posts[*i];
                let is_open = expanded.as_deref() == Some(p.id.as_str());
                let body_open = body_expanded.contains(&p.id);
                post_card(commands, fonts, p, is_open, comments.get(&p.id), me.as_deref(), moderator, body_open)
            }
        }),
    }
}

/// The "new in the marketplace" strip: a wrapping row of compact asset cards.
fn market_strip(commands: &mut Commands, fonts: &EmberFonts, assets: &[AssetSummary]) -> Entity {
    let wrap = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(7.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(rgba([255, 255, 255, 10])),
        ))
        .id();
    let head = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() })
        .id();
    let ic = icon_text(commands, &fonts.phosphor, "storefront", text_muted(), 13.0);
    let title = commands
        .spawn((Text::new("New in the Marketplace"), ui_font(&fonts.ui, 11.5), TextColor(rgb(text_primary()))))
        .id();
    commands.entity(head).add_children(&[ic, title]);
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(7.0),
            row_gap: Val::Px(7.0),
            ..default()
        })
        .id();
    for a in assets.iter().take(6) {
        let card = commands
            .spawn((
                Node {
                    width: Val::Px(126.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    padding: UiRect::all(Val::Px(6.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(rgba([255, 255, 255, 6])),
                Interaction::default(),
                renzora_ember::widgets::HoverTint::solid(
                    rgba([255, 255, 255, 6]),
                    rgba([255, 255, 255, 16]),
                    rgba([255, 255, 255, 26]),
                ),
                AssetOpenBtn,
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                HoverTooltip::new(format!("{} — open the Marketplace", a.name)),
            ))
            .id();
        let thumb = crate::avatars::thumb_image(commands, fonts, a.thumbnail_url.as_deref(), 114.0, 72.0, "package");
        let name = commands
            .spawn((Text::new(a.name.clone()), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_primary()))))
            .id();
        let price = commands
            .spawn((
                Text::new(if a.price_credits == 0 { "Free".to_string() } else { format!("{} credits", a.price_credits) }),
                ui_font(&fonts.ui, 9.0),
                TextColor(rgb(text_muted())),
            ))
            .id();
        commands.entity(card).add_children(&[thumb, name, price]);
        commands.entity(row).add_child(card);
    }
    commands.entity(wrap).add_children(&[head, row]);
    wrap
}

/// One feed post as a card — also used verbatim by the Profile panel's
/// activity tab, so posts look and behave identically everywhere (the click
/// systems in this module handle the markers globally).
#[allow(clippy::too_many_arguments)]
pub(crate) fn post_card(
    commands: &mut Commands,
    fonts: &EmberFonts,
    p: &FeedPost,
    expanded: bool,
    comments: Option<&Vec<FeedComment>>,
    me: Option<&str>,
    moderator: bool,
    body_open: bool,
) -> Entity {
    // One neutral card: header (avatar · name · time), body, media, actions.
    let card = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(rgba([255, 255, 255, 10])),
        ))
        .id();

    // ── Header: avatar · [name + role, meta] ····· [delete] ──
    let header = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(9.0),
            ..default()
        })
        .id();
    let av = avatar_image(commands, fonts, p.avatar_url.as_deref(), 34.0);
    let id_col = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            ..default()
        })
        .id();
    let name_row = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(5.0), ..default() })
        .id();
    let name = commands
        .spawn((
            Text::new(p.username.clone()),
            ui_font(&fonts.ui, 12.5),
            TextColor(rgb(text_primary())),
            Interaction::default(),
            UserBtn(p.username.clone()),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            HoverTooltip::new("View profile"),
        ))
        .id();
    commands.entity(name_row).add_child(name);
    if let Some((icon, hue)) = util::role_icon(&p.role) {
        let ri = icon_text(commands, &fonts.phosphor, icon, hue, 11.0);
        commands.entity(name_row).add_child(ri);
    }
    // Channel chip (if the post is in a channel).
    if let Some(cn) = p.channel_name.clone().or_else(|| p.channel_slug.clone()) {
        let chip = commands
            .spawn((
                Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(2.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() },
                BackgroundColor(rgb(accent()).with_alpha(0.22)),
            ))
            .id();
        let hic = icon_text(commands, &fonts.phosphor, "hash", accent(), 9.0);
        commands.entity(hic).insert(FocusPolicy::Pass);
        let ht = commands
            .spawn((Text::new(cn), ui_font(&fonts.ui, 9.0), TextColor(rgb(text_primary())), FocusPolicy::Pass))
            .id();
        commands.entity(chip).add_children(&[hic, ht]);
        commands.entity(name_row).add_child(chip);
    }
    let meta = if p.visibility == "public" {
        util::relative_time(&p.created_at)
    } else {
        format!("{} · {}", util::relative_time(&p.created_at), p.visibility)
    };
    let meta_e = commands
        .spawn((Text::new(meta), ui_font(&fonts.ui, 9.5), TextColor(rgb(placeholder()))))
        .id();
    commands.entity(id_col).add_children(&[name_row, meta_e]);
    commands.entity(header).add_children(&[av, id_col]);
    // Delete — own posts, or any post when the viewer is a site moderator.
    if me == Some(p.username.as_str()) || moderator {
        let del = util::action_chip(
            commands,
            fonts,
            "trash",
            None,
            false,
            Some("Delete post (click twice)".to_string()),
        );
        commands.entity(del).insert(DeleteBtn(p.id.clone()));
        commands.entity(header).add_child(del);
    }

    // ── Body — long posts clamp behind a "See more" toggle. Clamping is
    // client-side (char count, on a char boundary so multi-byte text is safe);
    // `body_open` from the panel decides whether it's currently expanded. ──
    const BODY_CLAMP: usize = 500;
    let body_long = p.body.chars().count() > BODY_CLAMP;
    let body_col = if p.body.trim().is_empty() {
        None
    } else {
        let shown = if body_long && !body_open {
            let mut s: String = p.body.chars().take(BODY_CLAMP).collect();
            s.truncate(s.trim_end().len());
            format!("{s}…")
        } else {
            p.body.clone()
        };
        let col = commands
            .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() })
            .id();
        let text = commands
            .spawn((Text::new(shown), ui_font(&fonts.ui, 13.0), TextColor(rgb(value_text()))))
            .id();
        commands.entity(col).add_child(text);
        if body_long {
            let see = commands
                .spawn((
                    Text::new(if body_open { "See less" } else { "See more" }),
                    ui_font(&fonts.ui, 11.0),
                    TextColor(rgb(accent())),
                    Interaction::default(),
                    SeeMoreBtn(p.id.clone()),
                    renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                    HoverTooltip::new(if body_open { "Collapse".to_string() } else { "Show the full post".to_string() }),
                ))
                .id();
            commands.entity(col).add_child(see);
        }
        Some(col)
    };

    // ── Media — sized by how many share the row: one image gets hero size. ──
    let media_row = if p.media_urls.is_empty() {
        None
    } else {
        let (mw, mh) = match p.media_urls.len() {
            1 => (440.0, 290.0),
            2 => (260.0, 175.0),
            _ => (200.0, 135.0),
        };
        let row = commands
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(6.0),
                row_gap: Val::Px(6.0),
                ..default()
            })
            .id();
        for url in p.media_urls.iter().take(4) {
            let img = crate::avatars::thumb_image(commands, fonts, Some(url), mw, mh, "image");
            // Click to view full-size in the lightbox overlay.
            commands.entity(img).insert((
                Interaction::default(),
                crate::lightbox::LightboxImage(url.clone()),
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            ));
            commands.entity(row).add_child(img);
        }
        Some(row)
    };

    // ── Actions: ❤ like · reactions (+ picker) · 💬 comments — one chip style. ──
    let actions = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            align_items: AlignItems::Center,
            flex_wrap: FlexWrap::Wrap,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    let like_tip = match (p.like_count, p.is_liked) {
        (0, _) => "Be the first to upvote this".to_string(),
        (1, true) => "You upvoted this — click to remove".to_string(),
        (n, true) => format!("You and {} other{} upvoted this", n - 1, if n == 2 { "" } else { "s" }),
        (n, false) => format!("{n} upvote{}", if n == 1 { "" } else { "s" }),
    };
    let like_count = p.like_count.to_string();
    let like = util::action_chip(
        commands,
        fonts,
        if p.is_liked { "arrow-fat-up-fill" } else { "arrow-fat-up" },
        Some(&like_count),
        p.is_liked,
        Some(like_tip),
    );
    commands.entity(like).insert(LikeBtn(p.id.clone()));
    let reactions = reaction_bar(commands, fonts, ReactionTarget::FeedPost(p.id.clone()), &p.reactions);
    let comment_count = p.comment_count.to_string();
    let cbtn = util::action_chip(
        commands,
        fonts,
        "chat-circle",
        Some(&comment_count),
        expanded,
        Some(if expanded { "Hide comments".to_string() } else { "Show comments".to_string() }),
    );
    commands.entity(cbtn).insert(CommentsBtn(p.id.clone()));
    commands.entity(actions).add_children(&[like, reactions, cbtn]);
    // Report — for anyone but the author. Enough reports auto-hide a post.
    if me != Some(p.username.as_str()) {
        let report = util::action_chip(commands, fonts, "flag", None, false, Some("Report this post".to_string()));
        commands.entity(report).insert(ReportBtn(p.id.clone()));
        commands.entity(actions).add_child(report);
    }

    // Hidden banner (own hidden posts only — the feed only returns them to the
    // author): explain + offer a staff review.
    let hidden_banner = if p.hidden {
        let banner = commands
            .spawn((
                Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), padding: UiRect::all(Val::Px(8.0)), border_radius: BorderRadius::all(Val::Px(7.0)), ..default() },
                BackgroundColor(rgba([RED.0, RED.1, RED.2, 26])),
            ))
            .id();
        let ic = icon_text(commands, &fonts.phosphor, "eye-slash", RED, 12.0);
        commands.entity(ic).insert(FocusPolicy::Pass);
        let msg = if p.review_requested { "Hidden — a review has been requested" } else { "Hidden pending review" };
        let t = commands
            .spawn((Text::new(msg), ui_font(&fonts.ui, 10.0), TextColor(rgb(RED)), FocusPolicy::Pass, Node { flex_grow: 1.0, ..default() }))
            .id();
        commands.entity(banner).add_children(&[ic, t]);
        if !p.review_requested {
            let rr = util::pill_button(commands, fonts, "Request review", accent(), (255, 255, 255));
            commands.entity(rr).insert(RequestReviewBtn(p.id.clone()));
            commands.entity(banner).add_child(rr);
        }
        Some(banner)
    } else {
        None
    };

    // Content column — indented past the avatar so media, body, and actions line
    // up under the username/timestamp (avatar 34 + header gap 9 = 43), rather than
    // running the full card width under the avatar. `align_self: Stretch` (with no
    // explicit width) makes it fill the card minus that left margin.
    let content = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            align_self: AlignSelf::Stretch,
            margin: UiRect::left(Val::Px(43.0)),
            ..default()
        })
        .id();
    // Media leads the content (above the text), so images are the first thing
    // seen after the author line; the body — and its "See more" — follows.
    let mut content_kids: Vec<Entity> = Vec::new();
    content_kids.extend(media_row);
    content_kids.extend(body_col);
    content_kids.push(actions);

    let mut kids = vec![header];
    kids.extend(hidden_banner);
    kids.push(content);

    // ── Expanded comments — an inset panel with padded bubbles per comment. ──
    if expanded {
        let section = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(rgba([255, 255, 255, 5])),
            ))
            .id();
        match comments {
            Some(list) if !list.is_empty() => {
                for c in list {
                    let row = commands
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(8.0),
                            align_items: AlignItems::FlexStart,
                            ..default()
                        })
                        .id();
                    let cav = avatar_image(commands, fonts, c.avatar_url.as_deref(), 24.0);
                    let bubble = commands
                        .spawn((
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(2.0),
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                                border_radius: BorderRadius::all(Val::Px(8.0)),
                                flex_grow: 1.0,
                                min_width: Val::Px(0.0),
                                ..default()
                            },
                            BackgroundColor(rgba([255, 255, 255, 8])),
                        ))
                        .id();
                    let head = commands
                        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() })
                        .id();
                    let cname = commands
                        .spawn((
                            Text::new(c.username.clone()),
                            ui_font(&fonts.ui, 11.0),
                            TextColor(rgb(text_primary())),
                            Interaction::default(),
                            UserBtn(c.username.clone()),
                            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                        ))
                        .id();
                    let cwhen = commands
                        .spawn((Text::new(util::relative_time(&c.created_at)), ui_font(&fonts.ui, 9.0), TextColor(rgb(placeholder()))))
                        .id();
                    commands.entity(head).add_children(&[cname, cwhen]);
                    let b = commands
                        .spawn((Text::new(c.body.clone()), ui_font(&fonts.ui, 11.5), TextColor(rgb(value_text()))))
                        .id();
                    commands.entity(bubble).add_children(&[head, b]);
                    commands.entity(row).add_children(&[cav, bubble]);
                    commands.entity(section).add_child(row);
                }
            }
            Some(_) => {
                let none = commands
                    .spawn((Text::new("No comments yet — say something nice"), ui_font(&fonts.ui, 10.5), TextColor(rgb(placeholder()))))
                    .id();
                commands.entity(section).add_child(none);
            }
            None => {
                let loading = commands
                    .spawn((Text::new("Loading comments..."), ui_font(&fonts.ui, 10.5), TextColor(rgb(placeholder()))))
                    .id();
                commands.entity(section).add_child(loading);
            }
        }
        // Comment composer.
        let crow = commands
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Center,
                margin: UiRect::top(Val::Px(2.0)),
                ..default()
            })
            .id();
        let cinput = text_input(commands, &fonts.ui, "Write a comment...", "");
        commands.entity(cinput).insert((CommentInput, Node { flex_grow: 1.0, ..default() }));
        let csend = util::pill_button(commands, fonts, "Reply", accent(), (255, 255, 255));
        commands.entity(csend).insert(CommentSendBtn(p.id.clone()));
        // Enter in the comment field presses Reply.
        commands.entity(crow).insert(EmberForm { submit: csend });
        commands.entity(crow).add_children(&[cinput, csend]);
        commands.entity(section).add_child(crow);
        content_kids.push(section);
    }

    commands.entity(content).add_children(&content_kids);
    commands.entity(card).add_children(&kids);
    card
}
