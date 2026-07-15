//! Profile viewer — a user's public renzora.com profile with follow / friend /
//! message / block actions and an About / Posts split.
//!
//! It's a **shared overlay**, not a panel: clicking a username anywhere (feed,
//! forum, friends, chat, a notification, the command palette) raises the same
//! centered modal over whatever you were doing, following the image-lightbox
//! pattern (full-screen dim backdrop, `GlobalZIndex`, `OverlaySurface`,
//! backdrop-click / Esc to close).

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use crossbeam_channel::{unbounded, Receiver, Sender};
use renzora::core::{SocialBridge, SocialPanelRequest};
use renzora::SplashState;
use std::collections::{HashMap, HashSet};

use renzora_auth::account::{self, ProfileUpdate, SocialConnection, SOCIAL_PLATFORMS};
use renzora_auth::feed::{FeedComment, FeedPost};
use renzora_auth::social::{ProfileAsset, PublicProfile, UserForumPost, UserRef};
use renzora_auth::AuthSession;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::{bind_display, bind_text, keyed_list_tokened, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    accent_button, accent_chip, accent_ghost, accent_icon_button, empty_state, scroll_area,
    text_input, textarea, tint, EmberForm, EmberTextInput, OverlaySurface,
};

use crate::avatars::avatar_image;
use crate::toasts::{ToastQueue, Tone};
use crate::util::{self, hash64, session_clone, HUE_CHAT, HUE_FRIENDS};
use crate::PendingSocialRequest;

const RED: (u8, u8, u8) = (224, 80, 80);
/// Profile hue — a friendly indigo for the overlay chrome/accents.
const HUE_PROFILE: (u8, u8, u8) = (129, 140, 248);

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ProfileTab {
    #[default]
    Activity,
    Assets,
}

/// Which people list is open (followers / following / friends), if any.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PeopleKind {
    Followers,
    Following,
    Friends,
}

impl PeopleKind {
    fn label(self) -> &'static str {
        match self {
            PeopleKind::Followers => "Followers",
            PeopleKind::Following => "Following",
            PeopleKind::Friends => "Friends",
        }
    }
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum ProfileResult {
    Profile(Result<PublicProfile, String>),
    Posts(Result<Vec<FeedPost>, String>),
    Assets(Result<Vec<ProfileAsset>, String>),
    ForumPosts(Result<Vec<UserForumPost>, String>),
    People(Result<Vec<UserRef>, String>),
    Action(Result<String, String>),
    OpenDm(Result<String, String>),
    /// Own-profile edits (avatar/cover uploads, field save) — the `Ok` string is
    /// a success toast; all of them refetch the profile so the change shows.
    Edited(Result<String, String>),
}

#[derive(Resource)]
pub(crate) struct ProfilePanel {
    pub username: Option<String>,
    pub profile: Option<PublicProfile>,
    pub posts: Vec<FeedPost>,
    pub assets: Vec<ProfileAsset>,
    pub forum_posts: Vec<UserForumPost>,
    pub people: Vec<UserRef>,
    pub people_kind: Option<PeopleKind>,
    pub people_note: Option<String>,
    pub tab: ProfileTab,
    /// When viewing your own profile: whether the inline edit form is showing.
    pub editing: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub version: u64,
    pub tx: Sender<ProfileResult>,
    rx: Receiver<ProfileResult>,
}

impl Default for ProfilePanel {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self {
            username: None,
            profile: None,
            posts: Vec::new(),
            assets: Vec::new(),
            forum_posts: Vec::new(),
            people: Vec::new(),
            people_kind: None,
            people_note: None,
            tab: ProfileTab::default(),
            editing: false,
            loading: false,
            error: None,
            version: 0,
            tx,
            rx,
        }
    }
}

impl ProfilePanel {
    pub fn bump(&mut self) {
        self.version = self.version.wrapping_add(1);
    }

    pub fn open(&mut self, session: &AuthSession, username: String) {
        self.username = Some(username.clone());
        self.profile = None;
        self.posts.clear();
        self.assets.clear();
        self.forum_posts.clear();
        self.people.clear();
        self.people_kind = None;
        self.people_note = None;
        self.tab = ProfileTab::Activity;
        // A fresh load always drops back to the read-only view.
        self.editing = false;
        self.loading = true;
        self.error = None;
        self.bump();
        let tx = self.tx.clone();
        let session = session_clone(session);
        spawn_thread(move || {
            let _ = tx.send(ProfileResult::Profile(renzora_auth::social::view_profile(
                &username,
                Some(&session),
            )));
            let _ = tx.send(ProfileResult::Posts(renzora_auth::feed::get_user_posts(
                &session, &username, None, 20,
            )));
            let _ = tx.send(ProfileResult::Assets(
                renzora_auth::social::get_user_assets(&username, 1).map(|r| r.assets),
            ));
            let _ = tx.send(ProfileResult::ForumPosts(
                renzora_auth::social::get_user_forum_posts(&username, 1).map(|r| r.posts),
            ));
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_thread(f: impl FnOnce() + Send + 'static) {
    std::thread::spawn(f);
}
#[cfg(target_arch = "wasm32")]
fn spawn_thread(_f: impl FnOnce() + Send + 'static) {}

/// The open profile overlay, if any.
#[derive(Resource, Default)]
pub(crate) struct ProfileOverlay {
    root: Option<Entity>,
    /// Set when an action navigated away (opened a DM) — the overlay should
    /// close so the destination panel isn't trapped behind the modal.
    close_requested: bool,
}

#[derive(Component)]
struct ProfileOverlayRoot;
#[derive(Component)]
struct ProfileOverlayCloseBtn;

pub(crate) fn register(app: &mut App) {
    app.init_resource::<ProfilePanel>();
    app.init_resource::<ProfileOverlay>();
    app.add_systems(
        Update,
        (
            poll_results,
            clicks,
            edit_clicks,
            social_link_click,
            // Close before open so clicking a new username while the overlay is
            // up swaps profiles instead of the backdrop-close eating the press.
            (close_overlay, open_overlay).chain(),
        )
            .run_if(in_state(SplashState::Editor)),
    );
}

/// Click a left-rail social link → open its platform URL in the browser.
fn social_link_click(q: Query<(&Interaction, &SocialLinkBtn), Changed<Interaction>>) {
    for (i, b) in &q {
        if *i == Interaction::Pressed && !b.0.trim().is_empty() {
            open_url(&b.0);
            break;
        }
    }
}

/// Open `url` in the user's default browser (best effort, per platform).
fn open_url(url: &str) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

/// Raise (or reuse) the profile overlay for a stashed `Profile` request.
fn open_overlay(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    mut pending: ResMut<PendingSocialRequest>,
    mut panel: ResMut<ProfilePanel>,
    mut overlay: ResMut<ProfileOverlay>,
    session: Res<AuthSession>,
    nodes: Query<(), With<Node>>,
) {
    let Some(SocialPanelRequest::Profile { username }) = &pending.0 else {
        return;
    };
    let Some(fonts) = fonts else { return };
    // `None` username = my own profile (needs sign-in); a named profile is public.
    let target = username.clone().or_else(|| session.user.as_ref().map(|u| u.username.clone()));
    pending.0 = None;
    let Some(target) = target else { return };

    if panel.username.as_deref() != Some(target.as_str()) || panel.profile.is_none() {
        panel.open(&session, target);
    }
    // Reuse a live overlay (just swap the profile data); otherwise spawn one.
    if overlay.root.is_some_and(|r| nodes.contains(r)) {
        return;
    }
    overlay.close_requested = false;
    overlay.root = Some(spawn_overlay(&mut commands, &fonts));
}

/// Close on backdrop click, Esc, the close button, or a navigate-away action.
fn close_overlay(
    mut commands: Commands,
    mut overlay: ResMut<ProfileOverlay>,
    keys: Res<ButtonInput<KeyCode>>,
    backdrops: Query<&Interaction, (With<ProfileOverlayRoot>, Changed<Interaction>)>,
    closes: Query<&Interaction, (With<ProfileOverlayCloseBtn>, Changed<Interaction>)>,
) {
    let Some(root) = overlay.root else { return };
    let backdrop_hit = backdrops.iter().any(|i| *i == Interaction::Pressed);
    let close_hit = closes.iter().any(|i| *i == Interaction::Pressed);
    if backdrop_hit || close_hit || overlay.close_requested || keys.just_pressed(KeyCode::Escape) {
        // `try_despawn`: a layout/theme rebuild may have torn it down already.
        commands.entity(root).try_despawn();
        overlay.root = None;
        overlay.close_requested = false;
    }
}

/// Full-screen dim backdrop + centered card hosting the profile content.
fn spawn_overlay(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let backdrop = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::all(Val::Px(24.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.66)),
            GlobalZIndex(9550),
            FocusPolicy::Block,
            Interaction::default(),
            OverlaySurface,
            ProfileOverlayRoot,
            Name::new("profile-overlay"),
        ))
        .id();

    // The card: clicks inside must NOT reach the backdrop-close, hence its own
    // FocusPolicy::Block. Capped size, its own scroll for tall profiles.
    let card = commands
        .spawn((
            Node {
                width: Val::Px(560.0),
                max_width: Val::Percent(96.0),
                max_height: Val::Percent(92.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(12.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(tint(HUE_PROFILE, 70)),
            FocusPolicy::Block,
        ))
        .id();

    // Slim title bar with a close button.
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(tint(HUE_PROFILE, 20)),
            BorderColor::all(rgba([255, 255, 255, 12])),
        ))
        .id();
    let title = commands
        .spawn((
            Text::new("Profile"),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(text_muted())),
            Node { flex_grow: 1.0, ..default() },
        ))
        .id();
    let close = accent_icon_button(commands, fonts, HUE_PROFILE, "x");
    commands.entity(close).insert(ProfileOverlayCloseBtn);
    commands.entity(bar).add_children(&[title, close]);

    // The existing profile content column, scrollable.
    // `scroll_area` (capped at a px height) sizes to content up to the cap — the
    // right choice for a content-sized modal card. `scroll_view` instead flex-
    // grows to fill a fixed-height parent, and this card has none, so it would
    // collapse to zero height (the "only the title bar shows" bug).
    let content = build(commands, fonts);
    let scroll = scroll_area(commands, content, 620.0);

    commands.entity(card).add_children(&[bar, scroll]);
    commands.entity(backdrop).add_child(card);
    backdrop
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn poll_results(
    mut panel: ResMut<ProfilePanel>,
    session: Res<AuthSession>,
    mut bridge: ResMut<SocialBridge>,
    mut overlay: ResMut<ProfileOverlay>,
    mut toasts: ResMut<ToastQueue>,
) {
    let mut got = Vec::new();
    while let Ok(r) = panel.rx.try_recv() {
        got.push(r);
    }
    for r in got {
        match r {
            ProfileResult::Profile(Ok(p)) => {
                panel.profile = Some(p);
                panel.loading = false;
                panel.bump();
            }
            ProfileResult::Profile(Err(e)) => {
                panel.loading = false;
                panel.error = Some(e);
                panel.bump();
            }
            ProfileResult::Posts(Ok(list)) => {
                panel.posts = list;
                panel.bump();
            }
            ProfileResult::Posts(Err(_)) => {}
            ProfileResult::Assets(Ok(list)) => {
                panel.assets = list;
                panel.bump();
            }
            ProfileResult::Assets(Err(_)) => {}
            ProfileResult::ForumPosts(Ok(list)) => {
                panel.forum_posts = list;
                panel.bump();
            }
            ProfileResult::ForumPosts(Err(_)) => {}
            ProfileResult::People(Ok(list)) => {
                panel.people = list;
                panel.people_note = None;
                panel.bump();
            }
            ProfileResult::People(Err(e)) => {
                panel.people = Vec::new();
                panel.people_note = Some(if e.contains("Unauthorized") || e.contains("401") {
                    "This user keeps their connections private".to_string()
                } else {
                    e
                });
                panel.bump();
            }
            ProfileResult::Action(Ok(msg)) => {
                toasts.push(Tone::Success, msg, None);
                if let Some(username) = panel.username.clone() {
                    panel.open(&session, username);
                }
            }
            ProfileResult::Action(Err(e)) => {
                toasts.push(Tone::Error, e, None);
            }
            ProfileResult::OpenDm(Ok(conversation_id)) => {
                bridge.open_panel_request = Some(SocialPanelRequest::Chat {
                    conversation_id: Some(conversation_id),
                });
                // Don't trap the chat panel behind the modal.
                overlay.close_requested = true;
            }
            ProfileResult::OpenDm(Err(e)) => {
                toasts.push(Tone::Error, e, None);
            }
            ProfileResult::Edited(Ok(msg)) => {
                toasts.push(Tone::Success, msg, None);
                // Refetch (which also drops edit mode) so the new avatar/cover/
                // fields render.
                if let Some(username) = panel.username.clone() {
                    panel.open(&session, username);
                }
            }
            ProfileResult::Edited(Err(e)) => {
                toasts.push(Tone::Error, e, None);
            }
        }
    }
}

// ── Clicks ───────────────────────────────────────────────────────────────────

#[derive(Component)]
struct ProfileTabBtn(ProfileTab);
#[derive(Component)]
struct FollowBtn(String);
#[derive(Component)]
struct FriendBtn(String);
#[derive(Component)]
struct MessageBtn(String);
#[derive(Component)]
struct BlockBtn(String);
#[derive(Component)]
struct PeopleBtn(PeopleKind);
#[derive(Component)]
struct PeopleCloseBtn;
#[derive(Component)]
struct PeopleRowBtn(String);
#[derive(Component)]
struct ForumPostRowBtn(String);
/// The private-note editor field on someone's profile.
#[derive(Component)]
struct NoteInput;
/// Saves the private note for this username.
#[derive(Component)]
struct NoteSaveBtn(String);
/// A social-link row in the left rail — opens its platform URL in the browser.
#[derive(Component)]
struct SocialLinkBtn(String);

// Own-profile editing.
#[derive(Component)]
struct EditProfileBtn;
#[derive(Component)]
struct CancelEditBtn;
#[derive(Component)]
struct SaveEditBtn;
#[derive(Component)]
struct ChangeAvatarBtn;
#[derive(Component)]
struct ChangeCoverBtn;
#[derive(Component)]
struct RemoveCoverBtn;
#[derive(Component)]
struct EditBioInput;
#[derive(Component)]
struct EditLocationInput;
#[derive(Component)]
struct EditWebsiteInput;
#[derive(Component)]
struct EditProfileColorInput;
#[derive(Component)]
struct EditBannerColorInput;

#[allow(clippy::too_many_arguments)]
fn clicks(
    mut panel: ResMut<ProfilePanel>,
    session: Res<AuthSession>,
    tabs: Query<(&Interaction, &ProfileTabBtn), Changed<Interaction>>,
    follows: Query<(&Interaction, &FollowBtn), Changed<Interaction>>,
    friends: Query<(&Interaction, &FriendBtn), Changed<Interaction>>,
    messages: Query<(&Interaction, &MessageBtn), Changed<Interaction>>,
    blocks: Query<(&Interaction, &BlockBtn), Changed<Interaction>>,
    people: Query<(&Interaction, &PeopleBtn), Changed<Interaction>>,
    people_close: Query<&Interaction, (With<PeopleCloseBtn>, Changed<Interaction>)>,
    people_rows: Query<(&Interaction, &PeopleRowBtn), Changed<Interaction>>,
    forum_rows: Query<(&Interaction, &ForumPostRowBtn), Changed<Interaction>>,
    note_saves: Query<(&Interaction, &NoteSaveBtn), Changed<Interaction>>,
    note_inputs: Query<&EmberTextInput, With<NoteInput>>,
    mut toasts: ResMut<ToastQueue>,
    mut bridge: ResMut<SocialBridge>,
) {
    let pressed = |i: &Interaction| *i == Interaction::Pressed;

    for (i, b) in &note_saves {
        if pressed(i) {
            if let Ok(input) = note_inputs.single() {
                util::save_profile_note(&b.0, &input.value);
                toasts.push(
                    Tone::Success,
                    if input.value.trim().is_empty() { "Note cleared" } else { "Note saved" },
                    None,
                );
            }
        }
    }

    for (i, b) in &tabs {
        if pressed(i) && panel.tab != b.0 {
            panel.tab = b.0;
            panel.bump();
        }
    }
    for (i, b) in &people {
        if pressed(i) {
            let kind = b.0;
            if panel.people_kind == Some(kind) {
                panel.people_kind = None;
                panel.bump();
                continue;
            }
            panel.people_kind = Some(kind);
            panel.people.clear();
            panel.people_note = Some("Loading...".to_string());
            panel.bump();
            let Some(username) = panel.username.clone() else { continue };
            let tx = panel.tx.clone();
            let s = session_clone(&session);
            spawn_thread(move || {
                let r = match kind {
                    PeopleKind::Followers => renzora_auth::social::get_followers(Some(&s), &username),
                    PeopleKind::Following => renzora_auth::social::get_following(Some(&s), &username),
                    PeopleKind::Friends => renzora_auth::social::get_friends_of(Some(&s), &username),
                };
                let _ = tx.send(ProfileResult::People(r));
            });
        }
    }
    for i in &people_close {
        if pressed(i) {
            panel.people_kind = None;
            panel.bump();
        }
    }
    for (i, b) in &people_rows {
        if pressed(i) {
            let target = b.0.clone();
            panel.open(&session, target);
        }
    }
    for (i, b) in &forum_rows {
        if pressed(i) {
            bridge.open_panel_request = Some(SocialPanelRequest::Forum { thread_slug: Some(b.0.clone()) });
        }
    }
    for (i, b) in &follows {
        if pressed(i) {
            let tx = panel.tx.clone();
            let s = session_clone(&session);
            let username = b.0.clone();
            spawn_thread(move || {
                let r = renzora_auth::social::toggle_follow(&s, &username).map(|f| {
                    if f.following { format!("Following {username}") } else { format!("Unfollowed {username}") }
                });
                let _ = tx.send(ProfileResult::Action(r));
            });
        }
    }
    for (i, b) in &friends {
        if pressed(i) {
            let tx = panel.tx.clone();
            let s = session_clone(&session);
            let username = b.0.clone();
            spawn_thread(move || {
                let r = renzora_auth::social::view_profile(&username, Some(&s))
                    .and_then(|p| renzora_auth::social::friend_add(&s, &p.id))
                    .map(|_| format!("Friend request sent to {username}"));
                let _ = tx.send(ProfileResult::Action(r));
            });
        }
    }
    for (i, b) in &messages {
        if pressed(i) {
            let tx = panel.tx.clone();
            let s = session_clone(&session);
            let user_id = b.0.clone();
            spawn_thread(move || {
                let r = renzora_auth::messages::open_dm(&s, &user_id).map(|r| r.conversation_id);
                let _ = tx.send(ProfileResult::OpenDm(r));
            });
        }
    }
    for (i, b) in &blocks {
        if pressed(i) {
            let tx = panel.tx.clone();
            let s = session_clone(&session);
            let username = b.0.clone();
            spawn_thread(move || {
                let r = renzora_auth::social::block_user(&s, &username)
                    .map(|_| format!("Blocked {username}"));
                let _ = tx.send(ProfileResult::Action(r));
            });
        }
    }
}

// ── Own-profile editing ──────────────────────────────────────────────────────

/// Which own-profile image an rfd upload targets (the two differ only in size
/// cap, endpoint, and toast wording).
enum ImageKind {
    Avatar,
    Banner,
}

/// Blocking file-pick + validate + upload, reporting the outcome as an
/// [`ProfileResult::Edited`]. Runs on a worker thread (opens a native dialog and
/// does network I/O). Mirrors the feed composer's attach-image flow.
fn upload_image_pick(session: &AuthSession, tx: &Sender<ProfileResult>, kind: ImageKind) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif"])
        .pick_file()
    else {
        return;
    };
    let cap = match kind {
        ImageKind::Avatar => 2,
        ImageKind::Banner => 4,
    } * 1024
        * 1024;
    let result = (|| -> Result<String, String> {
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        if bytes.len() > cap {
            return Err(match kind {
                ImageKind::Avatar => "Avatar is larger than 2 MB",
                ImageKind::Banner => "Cover photo is larger than 4 MB",
            }
            .to_string());
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("image.png").to_string();
        let ct = match path.extension().and_then(|e| e.to_str()).unwrap_or("png") {
            "jpg" | "jpeg" => "image/jpeg",
            "webp" => "image/webp",
            "gif" => "image/gif",
            _ => "image/png",
        };
        match kind {
            ImageKind::Avatar => {
                account::upload_avatar(session, &name, ct, &bytes).map(|_| "Avatar updated".to_string())
            }
            ImageKind::Banner => {
                account::upload_banner(session, &name, ct, &bytes).map(|_| "Cover photo updated".to_string())
            }
        }
    })();
    let _ = tx.send(ProfileResult::Edited(result));
}

/// Interactions specific to editing your own profile (toggle edit mode, avatar/
/// cover uploads, field save/cancel). Split from [`clicks`] to stay under the
/// system-param limit.
#[allow(clippy::too_many_arguments)]
fn edit_clicks(
    mut panel: ResMut<ProfilePanel>,
    session: Res<AuthSession>,
    edit_toggles: Query<&Interaction, (With<EditProfileBtn>, Changed<Interaction>)>,
    cancel_edits: Query<&Interaction, (With<CancelEditBtn>, Changed<Interaction>)>,
    save_edits: Query<&Interaction, (With<SaveEditBtn>, Changed<Interaction>)>,
    avatar_btns: Query<&Interaction, (With<ChangeAvatarBtn>, Changed<Interaction>)>,
    cover_btns: Query<&Interaction, (With<ChangeCoverBtn>, Changed<Interaction>)>,
    remove_covers: Query<&Interaction, (With<RemoveCoverBtn>, Changed<Interaction>)>,
    bio_inputs: Query<&EmberTextInput, With<EditBioInput>>,
    location_inputs: Query<&EmberTextInput, With<EditLocationInput>>,
    website_inputs: Query<&EmberTextInput, With<EditWebsiteInput>>,
    profile_color_inputs: Query<&EmberTextInput, With<EditProfileColorInput>>,
    banner_color_inputs: Query<&EmberTextInput, With<EditBannerColorInput>>,
) {
    let pressed = |i: &Interaction| *i == Interaction::Pressed;

    for i in &edit_toggles {
        if pressed(i) {
            panel.editing = !panel.editing;
            panel.bump();
        }
    }
    for i in &cancel_edits {
        if pressed(i) {
            panel.editing = false;
            panel.bump();
        }
    }
    for i in &avatar_btns {
        if pressed(i) {
            let tx = panel.tx.clone();
            let s = session_clone(&session);
            spawn_thread(move || upload_image_pick(&s, &tx, ImageKind::Avatar));
        }
    }
    for i in &cover_btns {
        if pressed(i) {
            let tx = panel.tx.clone();
            let s = session_clone(&session);
            spawn_thread(move || upload_image_pick(&s, &tx, ImageKind::Banner));
        }
    }
    for i in &remove_covers {
        if pressed(i) {
            let tx = panel.tx.clone();
            let s = session_clone(&session);
            spawn_thread(move || {
                let r = account::remove_banner(&s).map(|_| "Cover photo removed".to_string());
                let _ = tx.send(ProfileResult::Edited(r));
            });
        }
    }
    for i in &save_edits {
        if pressed(i) {
            // Non-empty → Some (empty color fields leave the color unchanged
            // rather than blanking it).
            let opt = |s: String| {
                let t = s.trim().to_string();
                if t.is_empty() {
                    None
                } else {
                    Some(t)
                }
            };
            let bio = bio_inputs.single().map(|x| x.value.trim().to_string()).unwrap_or_default();
            let location =
                location_inputs.single().map(|x| x.value.trim().to_string()).unwrap_or_default();
            let website =
                website_inputs.single().map(|x| x.value.trim().to_string()).unwrap_or_default();
            let profile_color =
                profile_color_inputs.single().map(|x| x.value.clone()).unwrap_or_default();
            let banner_color =
                banner_color_inputs.single().map(|x| x.value.clone()).unwrap_or_default();
            let update = ProfileUpdate {
                bio: Some(bio),
                location: Some(location),
                website: Some(website),
                profile_color: opt(profile_color),
                banner_color: opt(banner_color),
                ..Default::default()
            };
            let tx = panel.tx.clone();
            let s = session_clone(&session);
            spawn_thread(move || {
                let r = account::update_profile(&s, &update).map(|_| "Profile saved".to_string());
                let _ = tx.send(ProfileResult::Edited(r));
            });
        }
    }
}

// ── Build ────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        })
        .id();

    let error = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.5), TextColor(rgb(RED))))
        .id();
    bind_text(commands, error, |w| {
        w.get_resource::<ProfilePanel>().and_then(|p| p.error.clone()).unwrap_or_default()
    });
    bind_display(commands, error, |w| {
        w.get_resource::<ProfilePanel>().map(|p| p.error.is_some()).unwrap_or(false)
    });

    let content = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() })
        .id();
    keyed_list_tokened(
        commands,
        content,
        // Activity posts render through the feed's shared card, whose comment
        // threads / expansion live in FeedPanel — its version must retrigger
        // this panel too (mixed so the two counters can't cancel out).
        |w| {
            let p = w.get_resource::<ProfilePanel>().map(|p| p.version).unwrap_or(0);
            let f = w
                .get_resource::<crate::panels::feed::FeedPanel>()
                .map(|f| f.version)
                .unwrap_or(0);
            p ^ f.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        },
        snapshot,
    );

    commands.entity(root).add_children(&[error, content]);
    root
}

// ── Snapshot / views ─────────────────────────────────────────────────────────

fn snapshot(w: &World) -> KeyedSnapshot {
    let Some(panel) = w.get_resource::<ProfilePanel>() else {
        return util::empty_snapshot();
    };
    let Some(profile) = panel.profile.clone() else {
        let msg = if panel.loading { "Loading..." } else { "Open a profile from Friends, Feed, or Chat" };
        return KeyedSnapshot {
            items: vec![(u64::MAX, hash64(msg))],
            build: Box::new(move |commands, fonts, _| {
                empty_state(
                    commands,
                    fonts,
                    HUE_FRIENDS,
                    "identification-card",
                    msg,
                    Some("Click a name anywhere in Community to open their profile"),
                )
            }),
        };
    };
    let me = w
        .get_resource::<AuthSession>()
        .and_then(|s| s.user.as_ref().map(|u| u.username.clone()))
        .unwrap_or_default();
    let moderator = w
        .get_resource::<AuthSession>()
        .is_some_and(util::is_moderator);
    let posts = panel.posts.clone();
    let assets = panel.assets.clone();
    let forum_posts = panel.forum_posts.clone();
    let people = panel.people.clone();
    let people_kind = panel.people_kind;
    let people_note = panel.people_note.clone();
    let tab = panel.tab;
    let editing = panel.editing;
    // Comment expansion/threads live in the FeedPanel (the shared post cards
    // are the feed's); fold their state into the key so interactions re-render.
    let (expanded, comments, body_expanded) = w
        .get_resource::<crate::panels::feed::FeedPanel>()
        .map(|f| (f.expanded.clone(), f.comments.clone(), f.body_expanded.clone()))
        .unwrap_or_default();
    let posts_digest = posts
        .iter()
        .map(|p| crate::panels::feed::post_key(p, expanded.as_deref(), &comments, body_expanded.contains(&p.id)))
        .fold(0u64, |a, b| a.rotate_left(1) ^ b);
    let key = hash64(&(
        &profile.username,
        profile.is_following,
        posts.len(),
        assets.len(),
        forum_posts.len(),
        people.len(),
        people_kind.map(|k| k as u8),
        &people_note,
        tab as u8,
        editing,
        posts_digest,
    ));
    KeyedSnapshot {
        items: vec![(hash64(&profile.id), key)],
        build: Box::new(move |commands, fonts, _| {
            profile_view(
                commands,
                fonts,
                &profile,
                &posts,
                &assets,
                &forum_posts,
                people_kind.map(|k| (k, people.clone(), people_note.clone())),
                tab,
                profile.username == me,
                editing,
                PostCardCtx {
                    me: me.clone(),
                    moderator,
                    expanded: expanded.clone(),
                    comments: comments.clone(),
                    body_expanded: body_expanded.clone(),
                },
            )
        }),
    }
}

/// Viewer/interaction context threaded to the feed's shared post cards.
struct PostCardCtx {
    me: String,
    moderator: bool,
    expanded: Option<String>,
    comments: HashMap<String, Vec<FeedComment>>,
    body_expanded: HashSet<String>,
}

fn parse_hex(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some((r, g, b))
}


#[allow(clippy::too_many_arguments)]
fn profile_view(
    commands: &mut Commands,
    fonts: &EmberFonts,
    p: &PublicProfile,
    posts: &[FeedPost],
    assets: &[ProfileAsset],
    forum_posts: &[UserForumPost],
    people: Option<(PeopleKind, Vec<UserRef>, Option<String>)>,
    tab: ProfileTab,
    is_me: bool,
    editing: bool,
    ctx: PostCardCtx,
) -> Entity {
    let wrap = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(10.0), ..default() })
        .id();
    let mut kids: Vec<Entity> = Vec::new();

    let hue = p
        .banner_color
        .as_deref()
        .or(p.profile_color.as_deref())
        .and_then(parse_hex)
        .unwrap_or(accent());

    // ── Hero: the cover photo fills a banner with the avatar + identity overlaid
    // at the bottom (over a legibility scrim). ──
    kids.push(hero_banner(commands, fonts, p, hue));

    // ── Edit mode: the whole body becomes the edit form. ──
    if is_me && editing {
        kids.push(edit_form(commands, fonts, p, hue));
        commands.entity(wrap).add_children(&kids);
        return wrap;
    }

    // ── Two columns: a fixed identity/stats rail (left) + flexible content (right). ──
    let cols = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::FlexStart, column_gap: Val::Px(12.0), ..default() })
        .id();
    let left = commands
        .spawn(Node { width: Val::Px(178.0), flex_shrink: 0.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(9.0), ..default() })
        .id();
    let right = commands
        .spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() })
        .id();

    // ---- LEFT RAIL ----
    // Actions.
    if !is_me {
        let follow = rail_button(commands, fonts, if p.is_following { "Unfollow" } else { "Follow" }, hue, !p.is_following);
        commands.entity(follow).insert(FollowBtn(p.username.clone()));
        commands.entity(left).add_child(follow);
        let act_row = commands
            .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), ..default() })
            .id();
        let friend = renzora_ember::widgets::accent_icon_button(commands, fonts, HUE_FRIENDS, "handshake");
        commands.entity(friend).insert(FriendBtn(p.username.clone()));
        let message = renzora_ember::widgets::accent_icon_button(commands, fonts, HUE_CHAT, "chat-circle");
        commands.entity(message).insert(MessageBtn(p.id.clone()));
        let block = renzora_ember::widgets::accent_icon_button(commands, fonts, RED, "prohibit");
        commands.entity(block).insert(BlockBtn(p.username.clone()));
        commands.entity(act_row).add_children(&[friend, message, block]);
        commands.entity(left).add_child(act_row);
    } else {
        let edit = rail_button(commands, fonts, "Edit profile", hue, true);
        commands.entity(edit).insert(EditProfileBtn);
        commands.entity(left).add_child(edit);
    }

    // XP progress toward the next level (real thresholds: `level*(level-1)*50`).
    let xp = xp_card(commands, fonts, p, hue);
    commands.entity(left).add_child(xp);

    // Stats — clickable people lists.
    let f1 = stat_row(commands, fonts, hue, "users", &p.follower_count.to_string(), "Followers", PeopleKind::Followers);
    let f2 = stat_row(commands, fonts, hue, "user-plus", &p.following_count.to_string(), "Following", PeopleKind::Following);
    let f3 = stat_row(commands, fonts, HUE_FRIENDS, "handshake", "", "Friends", PeopleKind::Friends);
    for e in [f1, f2, f3] {
        commands.entity(left).add_child(e);
    }

    // Social links as text.
    if !p.connections.is_empty() {
        let h = rail_header(commands, fonts, "Links");
        let s = socials_text(commands, fonts, &p.connections);
        commands.entity(left).add_children(&[h, s]);
    }

    // Badges.
    if !p.badges.is_empty() {
        let h = rail_header(commands, fonts, "Badges");
        let brow = commands
            .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(5.0), row_gap: Val::Px(4.0), ..default() })
            .id();
        for b in &p.badges {
            let color = parse_hex(&b.color).unwrap_or(accent());
            let chip = accent_chip(commands, fonts, color, Some(if b.icon.is_empty() { "medal" } else { &b.icon }), &b.name);
            commands.entity(brow).add_child(chip);
        }
        commands.entity(left).add_children(&[h, brow]);
    }

    // About info.
    let ah = rail_header(commands, fonts, "About");
    let ai = about_info(commands, fonts, p);
    commands.entity(left).add_children(&[ah, ai]);

    // Private note (moderation) — local-only, other people's profiles only.
    if !is_me {
        let note = private_note(commands, fonts, p);
        commands.entity(left).add_child(note);
    }

    // ---- RIGHT CONTENT ----
    // People list (when a stat is open).
    if let Some((kind, list, note)) = &people {
        let panel_e = people_panel(commands, fonts, hue, *kind, list, note.as_deref());
        commands.entity(right).add_child(panel_e);
    }

    // Photos (images from the user's posts) → open full-size in the lightbox.
    if let Some(photos) = photos_section(commands, fonts, posts) {
        commands.entity(right).add_child(photos);
    }

    // Tabs (Activity / Assets). About info now lives in the left rail.
    let tabs = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, column_gap: Val::Px(4.0), ..default() })
        .id();
    let mut tab_defs = vec![(ProfileTab::Activity, "Activity".to_string())];
    if p.asset_count > 0 || !assets.is_empty() {
        tab_defs.push((ProfileTab::Assets, format!("Assets ({})", p.asset_count.max(assets.len() as i64))));
    }
    for (t, label) in tab_defs {
        let active = t == tab;
        let btn = util::pill_button(commands, fonts, &label, if active { hue } else { hover_bg() }, if active { (255, 255, 255) } else { text_primary() });
        commands.entity(btn).insert(ProfileTabBtn(t));
        commands.entity(tabs).add_child(btn);
    }
    commands.entity(right).add_child(tabs);
    let content = match tab {
        ProfileTab::Assets => assets_list(commands, fonts, assets),
        _ => activity_list(commands, fonts, p, posts, forum_posts, &ctx),
    };
    commands.entity(right).add_child(content);

    commands.entity(cols).add_children(&[left, right]);
    kids.push(cols);
    commands.entity(wrap).add_children(&kids);
    wrap
}

/// The cover-photo hero: the banner image (cover-filled) with the avatar +
/// name / level / role overlaid at the bottom over a dark scrim for legibility.
fn hero_banner(commands: &mut Commands, fonts: &EmberFonts, p: &PublicProfile, hue: (u8, u8, u8)) -> Entity {
    let hero = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(122.0),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(tint(hue, 165)),
        ))
        .id();
    if let Some(url) = p.banner_url.as_deref().filter(|u| !u.is_empty()) {
        crate::avatars::fill_image(commands, hero, url);
    }
    // Dark bottom scrim so the identity text stays legible over any cover.
    let scrim = commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(0.0), right: Val::Px(0.0), bottom: Val::Px(0.0), height: Val::Px(74.0), ..default() },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(hero).add_child(scrim);
    // Identity row, bottom-left.
    let content = commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(14.0), right: Val::Px(14.0), bottom: Val::Px(11.0), flex_direction: FlexDirection::Row, align_items: AlignItems::FlexEnd, column_gap: Val::Px(11.0), ..default() },
            FocusPolicy::Pass,
        ))
        .id();
    let ring = commands
        .spawn((
            Node { width: Val::Px(66.0), height: Val::Px(66.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border: UiRect::all(Val::Px(2.5)), border_radius: BorderRadius::all(Val::Px(33.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(tint(hue, 255)),
        ))
        .id();
    let av = avatar_image(commands, fonts, p.avatar_url.as_deref(), 58.0);
    commands.entity(ring).add_child(av);
    let idcol = commands
        .spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), flex_grow: 1.0, min_width: Val::Px(0.0), padding: UiRect::bottom(Val::Px(3.0)), ..default() })
        .id();
    let name_row = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), flex_wrap: FlexWrap::Wrap, row_gap: Val::Px(3.0), ..default() })
        .id();
    let name = commands
        .spawn((Text::new(p.username.clone()), ui_font(&fonts.ui, 18.0), TextColor(rgb((245, 245, 248)))))
        .id();
    let mut name_kids = vec![name];
    if let Some((icon, role_hue)) = util::role_icon(&p.role) {
        name_kids.push(icon_text(commands, &fonts.phosphor, icon, role_hue, 14.0));
    }
    name_kids.push(accent_chip(commands, fonts, (230, 180, 80), Some("star"), &format!("Level {}", p.level)));
    if p.seller_level > 0 {
        name_kids.push(accent_chip(commands, fonts, (70, 190, 190), Some("storefront"), &format!("Seller {}", p.seller_level)));
    }
    commands.entity(name_row).add_children(&name_kids);
    commands.entity(idcol).add_child(name_row);
    if let Some(bio) = p.bio.as_deref().filter(|b| !b.is_empty()) {
        let short: String = bio.chars().take(84).collect();
        let text = if bio.chars().count() > 84 { format!("{short}…") } else { short };
        let b = commands
            .spawn((Text::new(text), ui_font(&fonts.ui, 10.0), TextColor(rgb((214, 214, 222))), bevy::text::TextLayout::no_wrap(), Node { max_width: Val::Percent(100.0), overflow: Overflow::clip(), ..default() }))
            .id();
        commands.entity(idcol).add_child(b);
    }
    commands.entity(content).add_children(&[ring, idcol]);
    commands.entity(hero).add_child(content);
    hero
}

/// A muted uppercase section label for the left rail.
fn rail_header(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text.to_uppercase()),
            ui_font(&fonts.ui, 9.0),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::top(Val::Px(2.0)), ..default() },
        ))
        .id()
}

/// A full-width rail button (filled = primary, else a hue-tinted ghost). Built
/// inline so it can be `width: 100%` without clobbering an ember button's own
/// `Node`.
fn rail_button(commands: &mut Commands, fonts: &EmberFonts, label: &str, hue: (u8, u8, u8), filled: bool) -> Entity {
    let bg = if filled { rgb(hue) } else { Color::NONE };
    let btn = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(bg),
            BorderColor::all(if filled { rgb(hue) } else { tint(hue, 90) }),
            Interaction::default(),
            renzora_ember::widgets::HoverTint::solid(
                bg,
                if filled { tint(hue, 255) } else { tint(hue, 34) },
                if filled { tint(hue, 200) } else { tint(hue, 60) },
            ),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    let t = commands
        .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(if filled { (255, 255, 255) } else { hue })), FocusPolicy::Pass))
        .id();
    commands.entity(btn).add_child(t);
    btn
}

/// A full-width clickable stat row (icon + count + label) that opens a people
/// list. `count` empty → just the label (e.g. Friends).
fn stat_row(commands: &mut Commands, fonts: &EmberFonts, hue: (u8, u8, u8), icon: &str, count: &str, label: &str, kind: PeopleKind) -> Entity {
    let base = rgba([255, 255, 255, 8]);
    let row = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(7.0), padding: UiRect::axes(Val::Px(9.0), Val::Px(6.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() },
            BackgroundColor(base),
            Interaction::default(),
            renzora_ember::widgets::HoverTint::solid(base, rgb(hover_bg()), tint(hue, 40)),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            PeopleBtn(kind),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, hue, 12.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let mut row_kids = vec![ic];
    if !count.is_empty() {
        let c = commands
            .spawn((Text::new(count.to_string()), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())), FocusPolicy::Pass))
            .id();
        row_kids.push(c);
    }
    let l = commands
        .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_muted())), FocusPolicy::Pass, Node { flex_grow: 1.0, ..default() }))
        .id();
    row_kids.push(l);
    commands.entity(row).add_children(&row_kids);
    row
}

/// The XP progress card: level, a fill bar toward the next level, and the XP
/// remaining. Thresholds match the backend (`xp_for_level = level*(level-1)*50`).
fn xp_card(commands: &mut Commands, fonts: &EmberFonts, p: &PublicProfile, hue: (u8, u8, u8)) -> Entity {
    let lvl = p.level.max(0);
    let cur = (lvl * (lvl - 1) * 50).max(0);
    let next = ((lvl + 1) * lvl * 50).max(cur + 1);
    let frac = ((p.total_xp - cur) as f32 / (next - cur) as f32).clamp(0.0, 1.0);
    let to_next = (next - p.total_xp).max(0);

    let card = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(5.0), ..default() })
        .id();
    let head = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(5.0), ..default() })
        .id();
    let star = icon_text(commands, &fonts.phosphor, "lightning", (230, 180, 80), 11.0);
    let lbl = commands
        .spawn((Text::new(format!("Level {}", p.level)), ui_font(&fonts.ui, 11.5), TextColor(rgb(text_primary())), Node { flex_grow: 1.0, ..default() }))
        .id();
    let xp_lbl = commands
        .spawn((Text::new(format!("{} XP", p.total_xp)), ui_font(&fonts.ui, 9.5), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(head).add_children(&[star, lbl, xp_lbl]);
    let track = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(7.0), border_radius: BorderRadius::all(Val::Px(4.0)), overflow: Overflow::clip(), ..default() },
            BackgroundColor(rgb(hover_bg())),
        ))
        .id();
    let fill = commands
        .spawn((
            Node { width: Val::Percent(frac * 100.0), height: Val::Percent(100.0), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(tint(hue, 255)),
        ))
        .id();
    commands.entity(track).add_child(fill);
    let foot = commands
        .spawn((Text::new(format!("{to_next} XP to level {}", p.level + 1)), ui_font(&fonts.ui, 9.0), TextColor(rgb(placeholder()))))
        .id();
    commands.entity(card).add_children(&[head, track, foot]);
    card
}

/// The linked social accounts as clickable text rows (icon + username), opening
/// the platform URL in the browser.
fn socials_text(commands: &mut Commands, fonts: &EmberFonts, connections: &[SocialConnection]) -> Entity {
    let col = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() })
        .id();
    for c in connections {
        let (label, icon) = SOCIAL_PLATFORMS
            .iter()
            .find(|(v, _, _)| *v == c.platform)
            .map(|(_, l, ic)| (*l, *ic))
            .unwrap_or((c.platform.as_str(), "link"));
        let url = c.platform_url.clone().unwrap_or_default();
        let base = Color::NONE;
        let row = commands
            .spawn((
                Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), padding: UiRect::axes(Val::Px(4.0), Val::Px(3.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
                BackgroundColor(base),
                Interaction::default(),
                renzora_ember::widgets::HoverTint::solid(base, rgba([255, 255, 255, 14]), rgba([255, 255, 255, 22])),
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                SocialLinkBtn(url),
            ))
            .id();
        let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 12.0);
        commands.entity(ic).insert(FocusPolicy::Pass);
        let text = if c.platform_username.is_empty() { label.to_string() } else { c.platform_username.clone() };
        let t = commands
            .spawn((Text::new(text), ui_font(&fonts.ui, 10.5), TextColor(rgb(value_text())), FocusPolicy::Pass, bevy::text::TextLayout::no_wrap(), Node { flex_grow: 1.0, overflow: Overflow::clip(), ..default() }))
            .id();
        commands.entity(row).add_children(&[ic, t]);
        commands.entity(col).add_child(row);
    }
    col
}

/// The About info rows (joined / location / website / posts / seller) for the
/// left rail.
fn about_info(commands: &mut Commands, fonts: &EmberFonts, p: &PublicProfile) -> Entity {
    let col = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() })
        .id();
    let mut lines: Vec<(String, &str)> = vec![(format!("Joined {}", util::relative_time(&p.created_at)), "cake")];
    if let Some(v) = p.location.as_deref().filter(|v| !v.is_empty()) {
        lines.push((v.to_string(), "map-pin"));
    }
    if let Some(v) = p.website.as_deref().filter(|v| !v.is_empty()) {
        lines.push((v.to_string(), "link"));
    }
    lines.push((format!("{} posts", p.post_count), "newspaper"));
    if p.seller_level > 0 {
        lines.push((format!("Seller level {}", p.seller_level), "storefront"));
    }
    for (text, icon) in lines {
        let row = commands
            .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() })
            .id();
        let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 11.0);
        let short: String = text.chars().take(26).collect();
        let display = if text.chars().count() > 26 { format!("{short}…") } else { text };
        let t = commands
            .spawn((Text::new(display), ui_font(&fonts.ui, 10.0), TextColor(rgb(value_text())), bevy::text::TextLayout::no_wrap(), Node { overflow: Overflow::clip(), ..default() }))
            .id();
        commands.entity(row).add_children(&[ic, t]);
        commands.entity(col).add_child(row);
    }
    col
}

/// A grid of the user's post images; each opens full-size in the shared
/// lightbox. `None` when the user has posted no images.
fn photos_section(commands: &mut Commands, fonts: &EmberFonts, posts: &[FeedPost]) -> Option<Entity> {
    let mut urls: Vec<String> = Vec::new();
    for post in posts {
        for u in &post.media_urls {
            if !u.is_empty() && !urls.contains(u) {
                urls.push(u.clone());
            }
        }
    }
    if urls.is_empty() {
        return None;
    }
    urls.truncate(9);
    let wrap = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(5.0), ..default() })
        .id();
    let head = rail_header(commands, fonts, "Photos");
    let grid = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(5.0), row_gap: Val::Px(5.0), ..default() })
        .id();
    for url in &urls {
        let cell = crate::avatars::thumb_image(commands, fonts, Some(url), 84.0, 84.0, "image");
        commands.entity(cell).insert((
            Interaction::default(),
            crate::lightbox::LightboxImage(url.clone()),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ));
        commands.entity(grid).add_child(cell);
    }
    commands.entity(wrap).add_children(&[head, grid]);
    Some(wrap)
}

/// The followers/following/friends list panel (shown when a stat row is open).
fn people_panel(commands: &mut Commands, fonts: &EmberFonts, hue: (u8, u8, u8), kind: PeopleKind, list: &[UserRef], note: Option<&str>) -> Entity {
    let panel_e = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(tint(hue, 60)),
        ))
        .id();
    let head = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, ..default() })
        .id();
    let title = commands
        .spawn((Text::new(kind.label().to_string()), ui_font(&fonts.ui, 11.5), TextColor(rgb(text_primary())), Node { flex_grow: 1.0, ..default() }))
        .id();
    let close = renzora_ember::widgets::accent_icon_button(commands, fonts, (170, 170, 182), "x");
    commands.entity(close).insert(PeopleCloseBtn);
    commands.entity(head).add_children(&[title, close]);
    commands.entity(panel_e).add_child(head);
    if let Some(note) = note {
        let n = commands
            .spawn((Text::new(note.to_string()), ui_font(&fonts.ui, 10.0), TextColor(rgb(placeholder()))))
            .id();
        commands.entity(panel_e).add_child(n);
    } else if list.is_empty() {
        let n = commands
            .spawn((Text::new("Nobody here yet"), ui_font(&fonts.ui, 10.0), TextColor(rgb(placeholder()))))
            .id();
        commands.entity(panel_e).add_child(n);
    }
    for (ui_i, u) in list.iter().enumerate() {
        let base = rgb(if ui_i % 2 == 0 { row_even() } else { row_odd() });
        let row = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(7.0),
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(base),
                Interaction::default(),
                renzora_ember::widgets::HoverTint::solid(base, rgb(hover_bg()), tint(hue, 50)),
                PeopleRowBtn(u.username.clone()),
            ))
            .id();
        let pav = avatar_image(commands, fonts, u.avatar_url.as_deref(), 20.0);
        let pname = commands
            .spawn((Text::new(u.username.clone()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), Node { flex_grow: 1.0, ..default() }))
            .id();
        let mut row_kids = vec![pav, pname];
        if let Some((icon, role_hue)) = util::role_icon(&u.role) {
            row_kids.push(icon_text(commands, &fonts.phosphor, icon, role_hue, 11.0));
        }
        commands.entity(row).add_children(&row_kids);
        commands.entity(panel_e).add_child(row);
    }
    panel_e
}

/// The private moderation note (local-only) input + save.
fn private_note(commands: &mut Commands, fonts: &EmberFonts, p: &PublicProfile) -> Entity {
    let note_wrap = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(rgba([255, 255, 255, 5])),
        ))
        .id();
    let note_head = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(5.0), ..default() })
        .id();
    let note_icon = icon_text(commands, &fonts.phosphor, "note-pencil", text_muted(), 11.0);
    let note_label = commands
        .spawn((Text::new("Private note"), ui_font(&fonts.ui, 9.5), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(note_head).add_children(&[note_icon, note_label]);
    let existing = util::load_profile_notes().get(&p.username).cloned().unwrap_or_default();
    let note_row = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(5.0), ..default() })
        .id();
    let note_in = text_input(commands, &fonts.ui, "Add a note…", &existing);
    commands.entity(note_in).insert((NoteInput, Node { width: Val::Percent(100.0), ..default() }));
    let note_save = util::pill_button(commands, fonts, "Save", accent(), (255, 255, 255));
    commands.entity(note_save).insert(NoteSaveBtn(p.username.clone()));
    commands.entity(note_row).insert(EmberForm { submit: note_save });
    commands.entity(note_row).add_children(&[note_in, note_save]);
    commands.entity(note_wrap).add_children(&[note_head, note_row]);
    note_wrap
}

/// A muted field label above an edit control.
fn field_label(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text.to_string()),
            ui_font(&fonts.ui, 9.5),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::top(Val::Px(4.0)), ..default() },
        ))
        .id()
}

/// The inline edit form for your own profile: avatar & cover uploads plus the
/// editable text fields. Rendered in place of the read-only body while editing;
/// each control is seeded from the current profile so Save round-trips cleanly.
fn edit_form(commands: &mut Commands, fonts: &EmberFonts, p: &PublicProfile, hue: (u8, u8, u8)) -> Entity {
    let form = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(tint(hue, 60)),
        ))
        .id();
    let mut kids: Vec<Entity> = Vec::new();

    // Avatar.
    kids.push(field_label(commands, fonts, "Avatar"));
    let av_row = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(10.0), ..default() })
        .id();
    let av = avatar_image(commands, fonts, p.avatar_url.as_deref(), 48.0);
    let av_btn = accent_ghost(commands, fonts, hue, "Change avatar");
    commands.entity(av_btn).insert(ChangeAvatarBtn);
    commands.entity(av_row).add_children(&[av, av_btn]);
    kids.push(av_row);

    // Cover photo (banner).
    kids.push(field_label(commands, fonts, "Cover photo"));
    let cover_row = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(10.0), ..default() })
        .id();
    let cover_preview = crate::avatars::thumb_image(commands, fonts, p.banner_url.as_deref(), 120.0, 40.0, "image");
    let cover_change = accent_ghost(commands, fonts, hue, "Change cover");
    commands.entity(cover_change).insert(ChangeCoverBtn);
    let cover_remove = accent_ghost(commands, fonts, RED, "Remove");
    commands.entity(cover_remove).insert(RemoveCoverBtn);
    commands.entity(cover_row).add_children(&[cover_preview, cover_change, cover_remove]);
    kids.push(cover_row);

    // Bio.
    kids.push(field_label(commands, fonts, "Bio"));
    let bio_in = textarea(commands, &fonts.ui, "Tell people about yourself", p.bio.as_deref().unwrap_or(""));
    commands.entity(bio_in).insert((EditBioInput, Node { width: Val::Percent(100.0), min_height: Val::Px(56.0), ..default() }));
    kids.push(bio_in);

    // Location.
    kids.push(field_label(commands, fonts, "Location"));
    let loc_in = text_input(commands, &fonts.ui, "Where are you?", p.location.as_deref().unwrap_or(""));
    commands.entity(loc_in).insert(EditLocationInput);
    kids.push(loc_in);

    // Website.
    kids.push(field_label(commands, fonts, "Website"));
    let web_in = text_input(commands, &fonts.ui, "https://example.com", p.website.as_deref().unwrap_or(""));
    commands.entity(web_in).insert(EditWebsiteInput);
    kids.push(web_in);

    // Colors (hex — no native color picker widget, so a #rrggbb text field).
    kids.push(field_label(commands, fonts, "Profile color (#rrggbb)"));
    let pc_in = text_input(commands, &fonts.ui, "#5b9cf5", p.profile_color.as_deref().unwrap_or(""));
    commands.entity(pc_in).insert(EditProfileColorInput);
    kids.push(pc_in);

    kids.push(field_label(commands, fonts, "Banner color (#rrggbb)"));
    let bc_in = text_input(commands, &fonts.ui, "#5b9cf5", p.banner_color.as_deref().unwrap_or(""));
    commands.entity(bc_in).insert(EditBannerColorInput);
    kids.push(bc_in);

    // Save / Cancel.
    let btn_row = commands
        .spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), margin: UiRect::top(Val::Px(8.0)), ..default() })
        .id();
    let save = accent_button(commands, fonts, hue, "Save");
    commands.entity(save).insert(SaveEditBtn);
    let cancel = accent_ghost(commands, fonts, (150, 150, 162), "Cancel");
    commands.entity(cancel).insert(CancelEditBtn);
    commands.entity(btn_row).add_children(&[save, cancel]);
    kids.push(btn_row);

    commands.entity(form).add_children(&kids);
    form
}

/// The merged Activity timeline: feed posts + forum posts, newest first.
/// Feed posts render through the feed panel's shared [`post_card`], so they
/// look and behave exactly like the feed (likes, reactions, comments, images).
fn activity_list(
    commands: &mut Commands,
    fonts: &EmberFonts,
    p: &PublicProfile,
    posts: &[FeedPost],
    forum_posts: &[UserForumPost],
    ctx: &PostCardCtx,
) -> Entity {
    let list = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() })
        .id();

    enum Item<'a> {
        Post(&'a FeedPost),
        Forum(&'a UserForumPost),
    }
    let mut items: Vec<(i64, Item)> = posts
        .iter()
        .map(|x| (util::parse_timestamp(&x.created_at).unwrap_or(0), Item::Post(x)))
        .chain(forum_posts.iter().map(|x| (util::parse_timestamp(&x.created_at).unwrap_or(0), Item::Forum(x))))
        .collect();
    items.sort_by_key(|(t, _)| -*t);

    if items.is_empty() {
        let none = commands
            .spawn((Text::new("No activity yet"), ui_font(&fonts.ui, 10.5), TextColor(rgb(placeholder()))))
            .id();
        commands.entity(list).add_child(none);
    }

    for (_, item) in items {
        match item {
            Item::Post(post) => {
                let is_open = ctx.expanded.as_deref() == Some(post.id.as_str());
                let card = crate::panels::feed::post_card(
                    commands,
                    fonts,
                    post,
                    is_open,
                    ctx.comments.get(&post.id),
                    (!ctx.me.is_empty()).then_some(ctx.me.as_str()),
                    ctx.moderator,
                    ctx.body_expanded.contains(&post.id),
                );
                commands.entity(list).add_child(card);
            }
            Item::Forum(fp) => {
                let card = commands
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.0),
                            padding: UiRect::all(Val::Px(10.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(rgb(card_bg())),
                        BorderColor::all(rgba([255, 255, 255, 12])),
                        Interaction::default(),
                        renzora_ember::widgets::HoverTint::solid(rgb(card_bg()), rgb(hover_bg()), renzora_ember::widgets::tint(crate::util::HUE_FORUM, 40)),
                        ForumPostRowBtn(fp.thread_slug.clone()),
                    ))
                    .id();
                let head = commands
                    .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() })
                    .id();
                let hchip = accent_chip(commands, fonts, crate::util::HUE_FORUM, Some("chats-circle"), "Forum");
                let title = commands
                    .spawn((Text::new(fp.thread_title.clone()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), Node { flex_grow: 1.0, ..default() }))
                    .id();
                let htime = commands
                    .spawn((Text::new(util::relative_time(&fp.created_at)), ui_font(&fonts.ui, 9.0), TextColor(rgb(placeholder()))))
                    .id();
                commands.entity(head).add_children(&[hchip, title, htime]);
                let body = commands
                    .spawn((Text::new(fp.content.clone()), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_muted()))))
                    .id();
                commands.entity(card).add_children(&[head, body]);
                commands.entity(list).add_child(card);
            }
        }
    }
    let _ = p;
    list
}

/// Published marketplace assets.
fn assets_list(commands: &mut Commands, fonts: &EmberFonts, assets: &[ProfileAsset]) -> Entity {
    let list = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(5.0), ..default() })
        .id();
    if assets.is_empty() {
        let none = commands
            .spawn((Text::new("No published assets"), ui_font(&fonts.ui, 10.5), TextColor(rgb(placeholder()))))
            .id();
        commands.entity(list).add_child(none);
    }
    for a in assets {
        let row = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(10.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(rgb(card_bg())),
                BorderColor::all(rgba([255, 255, 255, 12])),
            ))
            .id();
        let thumb = crate::avatars::thumb_image(commands, fonts, a.thumbnail_url.as_deref(), 56.0, 40.0, "package");
        let col = commands
            .spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), flex_grow: 1.0, ..default() })
            .id();
        let name = commands
            .spawn((Text::new(a.name.clone()), ui_font(&fonts.ui, 11.5), TextColor(rgb(text_primary()))))
            .id();
        let meta = commands
            .spawn((
                Text::new(format!("{} · {} downloads", a.category, a.downloads)),
                ui_font(&fonts.ui, 9.5),
                TextColor(rgb(text_muted())),
            ))
            .id();
        commands.entity(col).add_children(&[name, meta]);
        let price = commands
            .spawn((
                Text::new(if a.price_credits == 0 { "Free".to_string() } else { format!("{} cr", a.price_credits) }),
                ui_font(&fonts.ui, 10.5),
                TextColor(rgb(if a.price_credits == 0 { (82, 196, 120) } else { (235, 180, 80) })),
            ))
            .id();
        commands.entity(row).add_children(&[thumb, col, price]);
        commands.entity(list).add_child(row);
    }
    list
}
