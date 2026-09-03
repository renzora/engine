//! The **Publish** panel — an in-editor asset uploader that mirrors the
//! website's `/marketplace/upload` form field-for-field (see
//! `website/crates/web/src/pages/upload.rs`).
//!
//! One page, three sections, matching the website exactly: **Your file** (main
//! file, cover, screenshots, a video URL, and — for Music — audio previews),
//! **Basics** (name, category, the category's own detail fields, description,
//! price, tags, minimum engine version, licence, AI flag) and **Credit /
//! Attribution**. There is no content-type choice — the game store is gone — and
//! no version field: everything publishes at 1.0.0, because a version belongs to
//! an update rather than a first publish.
//!
//! Category-specific groups show/hide by [`bind_display`] on the selected
//! category, the same way the web form toggles its `data-show-for-category`
//! divs, so field widgets — and their two-way bindings to [`Uploader`] — survive
//! a category change. All form state lives in the [`Uploader`] resource, so a
//! dock move that rebuilds the panel content re-seeds every field from state
//! rather than losing it.
//!
//! Networking matches the rest of the hub: file reads + the multipart upload run
//! on a worker thread and post their result back over a `crossbeam_channel`,
//! drained in [`uploader_poll`]. Native file dialogs (`rfd`) also run on a worker
//! thread (they block), exactly like the profile avatar/cover upload.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use crossbeam_channel::{unbounded, Receiver, Sender};

use crate::auth::marketplace::Category;
use crate::auth::publish::{self, MediaUpload, PublishMeta, UploadFile, UploadedItem};
use crate::auth::session::AuthSession;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::{KeyedSnapshot};
use renzora_ember::reactive::Rx;
use renzora_ember::reactive::tracked::{bind_2way, bind_display, bind_text, keyed_list};
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_text_input, checkbox, dropdown, text_input, textarea, tint};
use renzora::SplashState;



/// Licence options, value-for-value the website's `<select id="w-license">` and
/// the server's `VALID_LICENCES` (see `LICENCE_VALUES`).
const LICENSES: &[&str] = &[
    "Standard Marketplace License",
    "Extended License",
    "MIT",
    "Apache 2.0",
    "GPL 3.0",
    "CC0 (Public Domain)",
];
const GENRES: &[&str] = &[
    "Select genre…",
    "Ambient",
    "Orchestral",
    "Electronic",
    "Retro / Chiptune",
    "Rock",
    "Cinematic",
    "Other",
];
const SCRIPT_LANGS: &[&str] = &["Select…", "Rust", "Lua", "Rhai", "WGSL (Shader)", "Visual Blueprint", "Other"];
/// Value sent for each `SCRIPT_LANGS` entry (index 0 sends nothing).
const SCRIPT_LANG_VALUES: &[&str] = &["", "rust", "lua", "rhai", "wgsl", "blueprint", "other"];
/// Genre slug per `GENRES` entry (index 0 sends nothing).
const GENRE_VALUES: &[&str] = &["", "ambient", "orchestral", "electronic", "retro", "rock", "cinematic", "other"];
/// Licence ids, matching the server's `VALID_LICENCES`.
const LICENCE_VALUES: &[&str] = &["standard", "extended", "mit", "apache2", "gpl3", "cc0"];
/// Supported engine versions, newest first. Mirrors `docs/_versions.json`;
/// nightlies are deliberately not offered as a support target.
const ENGINE_VERSIONS: &[&str] = &["Any version", "r1-alpha7", "r1-alpha6", "r1-alpha5"];

/// A file the user picked from a native dialog. Bytes are read lazily on the
/// upload worker thread (the main file can be hundreds of MB), so we hold only
/// the path + display metadata here.
#[derive(Clone)]
struct PickedFile {
    path: PathBuf,
    name: String,
    size: u64,
}

/// Which slot a completed file-pick fills (posted from the picker worker thread).
enum PickMsg {
    Main(PickedFile),
    Thumb(PickedFile),
    Screenshots(Vec<PickedFile>),
    Audio(Vec<PickedFile>),
}

/// All form state. Text fields two-way-bind here via [`bind_text_input`];
/// dropdowns/checkboxes via [`bind_2way`] on their `Bound<_>`. Every field here
/// is submitted — the detail fields ride along in `PublishMeta::metadata`.
#[derive(Resource)]
struct Uploader {
    // Categories.
    categories: Vec<Category>,
    cats_loading: bool,
    /// Set once the fetch has been kicked. Without it a failed load would leave
    /// the list empty and `ensure_categories` would respawn a worker every
    /// frame; the old flow could not hit that because the fetch was triggered
    /// by a click.
    cats_attempted: bool,
    cats_rx: Option<Receiver<Result<Vec<Category>, String>>>,
    /// Index into the category dropdown: 0 is the "Select a category…"
    /// placeholder, so a real category is `categories[category_index - 1]`.
    category_index: usize,

    // Basics.
    name: String,
    description: String,
    price: String,
    credit_name: String,
    credit_url: String,

    // Tags.
    tags: Vec<String>,
    tag_query: String,
    tag_suggestions: Vec<String>,
    tag_last_searched: String,
    tag_rx: Option<Receiver<Vec<String>>>,

    // Detail fields, submitted inside `PublishMeta::metadata`.
    ai_generated: bool,
    engine_version: usize,
    license: usize,
    bpm: String,
    genre: usize,
    loopable: bool,
    script_lang: usize,

    // Files & media.
    file: Option<PickedFile>,
    thumbnail: Option<PickedFile>,
    screenshots: Vec<PickedFile>,
    video_url: String,
    audio: Vec<PickedFile>,
    pick_tx: Sender<PickMsg>,
    pick_rx: Receiver<PickMsg>,

    // Submit.
    submitting: bool,
    submit_rx: Option<Receiver<Result<UploadedItem, String>>>,
    error: Option<String>,
    success: Option<String>,
    success_url: Option<String>,
}

impl Default for Uploader {
    fn default() -> Self {
        let (pick_tx, pick_rx) = unbounded();
        Self {
            categories: Vec::new(),
            cats_loading: false,
            cats_attempted: false,
            cats_rx: None,
            category_index: 0,
            name: String::new(),
            description: String::new(),
            price: "0".to_string(),
            credit_name: String::new(),
            credit_url: String::new(),
            tags: Vec::new(),
            tag_query: String::new(),
            tag_suggestions: Vec::new(),
            tag_last_searched: String::new(),
            tag_rx: None,
            ai_generated: false,
            engine_version: 0,
            license: 0,
            bpm: String::new(),
            genre: 0,
            loopable: false,
            script_lang: 0,
            file: None,
            thumbnail: None,
            screenshots: Vec::new(),
            video_url: String::new(),
            audio: Vec::new(),
            pick_tx,
            pick_rx,
            submitting: false,
            submit_rx: None,
            error: None,
            success: None,
            success_url: None,
        }
    }
}

impl Uploader {
    /// The selected category's slug, or "" while the placeholder is showing.
    fn category_slug(&self) -> &str {
        self.category_index
            .checked_sub(1)
            .and_then(|i| self.categories.get(i))
            .map(|c| c.slug.as_str())
            .unwrap_or("")
    }

    fn price_credits(&self) -> i64 {
        self.price.trim().parse::<i64>().unwrap_or(0).max(0)
    }

    /// Buyers download `<asset-title>.<ext>` rather than whatever the file was
    /// called on disk — the same rule the website applies.
    fn download_filename(&self) -> String {
        let mut slug = String::new();
        let mut dash = false;
        for c in self.name.trim().to_lowercase().chars() {
            if c.is_ascii_alphanumeric() {
                slug.push(c);
                dash = false;
            } else if !slug.is_empty() && !dash {
                slug.push('-');
                dash = true;
            }
        }
        let slug = slug.trim_end_matches('-').chars().take(64).collect::<String>();
        let stem = if slug.is_empty() { "asset".to_string() } else { slug };
        match self.file.as_ref().and_then(|f| f.name.rsplit_once('.')) {
            Some((_, ext)) => format!("{stem}.{ext}"),
            None => stem,
        }
    }
}

fn u<'w>(w: &Rx<'w>) -> Option<&'w Uploader> {
    w.get_resource::<Uploader>()
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub(crate) struct UploaderPanel;

impl Plugin for UploaderPanel {
    fn build(&self, app: &mut App) {
        app.init_resource::<Uploader>();
        // Not a dock panel any more — publishing is a view inside the
        // marketplace overlay (see `store::build`), which is where you already
        // are when you decide to sell something. The systems are registered
        // plainly rather than through `register_panel_content`, so they are not
        // gated on a panel being visible; the only one that touches the network
        // is kicked by a click (see `begin_publishing`).
        app.add_systems(
            Update,
            (
                uploader_poll,
                nav_click,
                pick_click,
                tag_click,
                tag_search,
                success_link_click,
            )
                .run_if(in_state(SplashState::Editor)),
        );
    }
}

// ── Marker components ───────────────────────────────────────────────────────────


#[derive(Component)]
struct PublishBtn;
#[derive(Component)]
struct PickMainBtn;
#[derive(Component)]
struct PickThumbBtn;
#[derive(Component)]
struct PickShotsBtn;
#[derive(Component)]
struct PickAudioBtn;
#[derive(Component)]
struct TagRemoveBtn(usize);
#[derive(Component)]
struct TagAddBtn(String);
#[derive(Component)]
struct SuccessLinkBtn;

// ── Build ───────────────────────────────────────────────────────────────────────

pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // The panel has two stages and shows exactly one. Both are built up front
    // and swapped by `bind_display`, which is how every other conditional
    // surface here works — the alternative, rebuilding the subtree when creator
    // status lands, would throw away in-progress form state on a background
    // refresh.
    //
    // This is the "Become a Creator" panel, folded in. Keeping them apart meant
    // the uploader's only useful message to a non-creator was to go and open the
    // other panel, which is a worse version of just showing them the wizard.
    let outer = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();

    let wizard = crate::onboarding::build(commands, fonts);
    bind_display(commands, wizard, |w| !crate::onboarding::can_publish(w));

    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            max_width: Val::Px(680.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(14.0),
            padding: UiRect::all(Val::Px(20.0)),
            margin: UiRect::horizontal(Val::Auto),
            ..default()
        })
        .id();
    bind_display(commands, root, crate::onboarding::can_publish);

    // Header.
    let title = commands
        .spawn((Text::new("Publish an Asset"), ui_font(&fonts.ui, 22.0), TextColor(rgb(text_primary()))))
        .id();
    let subtitle = commands
        .spawn((
            Text::new("3D models, scripts, audio, textures, plugins and more. Fields marked * are required."),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(text_muted())),
        ))
        .id();

    // Error / success banners.
    let error = banner(commands, fonts, "warning-circle", (224, 96, 96));
    bind_display(commands, error, |w| u(w).map(|s| s.error.is_some()).unwrap_or(false));
    bind_banner_text(commands, fonts, error, |w| u(w).and_then(|s| s.error.clone()).unwrap_or_default(), (224, 96, 96), "warning-circle");

    let success = success_banner(commands, fonts);

    let sections = [
        build_files_section(commands, fonts),
        build_basics_section(commands, fonts),
        build_credit_section(commands, fonts),
        build_publish_row(commands, fonts),
    ];

    commands.entity(root).add_children(&[title, subtitle, error, success]);
    for s in sections {
        commands.entity(root).add_child(s);
    }
    commands.entity(outer).add_children(&[wizard, root]);
    outer
}

fn banner(commands: &mut Commands, _fonts: &EmberFonts, _icon: &str, hue: (u8, u8, u8)) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(tint(hue, 26)),
            BorderColor::all(tint(hue, 60)),
        ))
        .id()
}

/// Give a banner an icon + bound text child.
fn bind_banner_text(
    commands: &mut Commands,
    fonts: &EmberFonts,
    banner: Entity,
    get: impl Fn(&Rx) -> String + Send + Sync + 'static,
    hue: (u8, u8, u8),
    icon: &str,
) {
    let ic = icon_text(commands, &fonts.phosphor, icon, hue, 15.0);
    let txt = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(hue))))
        .id();
    bind_text(commands, txt, get);
    commands.entity(banner).add_children(&[ic, txt]);
}

/// Success banner with a clickable "View your item" link.
fn success_banner(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let b = banner(commands, fonts, "check-circle", (52, 180, 96));
    bind_display(commands, b, |w| u(w).map(|s| s.success.is_some()).unwrap_or(false));
    let ic = icon_text(commands, &fonts.phosphor, "check-circle", (52, 180, 96), 15.0);
    let txt = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb((52, 180, 96)))))
        .id();
    bind_text(commands, txt, |w| u(w).and_then(|s| s.success.clone()).unwrap_or_default());
    let link = commands
        .spawn((
            Node { padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(tint((52, 180, 96), 40)),
            Interaction::default(),
            SuccessLinkBtn,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    let link_txt = commands
        .spawn((Text::new("View →"), ui_font(&fonts.ui, 11.0), TextColor(rgb((52, 180, 96))), FocusPolicy::Pass))
        .id();
    commands.entity(link).add_child(link_txt);
    commands.entity(b).add_children(&[ic, txt, link]);
    b
}

// ── Shared field helpers ────────────────────────────────────────────────────────

/// A rounded content section (matches the web wizard's card panels).
fn section(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(14.0),
                padding: UiRect::all(Val::Px(18.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id()
}

fn heading(commands: &mut Commands, fonts: &EmberFonts, icon: &str, text: &str) -> Entity {
    let row = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(7.0), ..default() })
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, accent(), 15.0);
    let t = commands
        .spawn((Text::new(text.to_string()), ui_font(&fonts.ui, 13.5), TextColor(rgb(text_primary()))))
        .id();
    commands.entity(row).add_children(&[ic, t]);
    row
}

fn field_label(commands: &mut Commands, fonts: &EmberFonts, text: &str, required: bool) -> Entity {
    let label = if required { format!("{text} *") } else { text.to_string() };
    commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::bottom(Val::Px(4.0)), ..default() },
        ))
        .id()
}

fn help_text(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text.to_string()),
            ui_font(&fonts.ui, 9.5),
            TextColor(rgb(placeholder())),
            Node { margin: UiRect::top(Val::Px(3.0)), ..default() },
        ))
        .id()
}

/// A single-line text field bound two-way to a `String` in [`Uploader`].
fn text_field(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    required: bool,
    placeholder: &str,
    get: impl Fn(&Uploader) -> String + Send + Sync + 'static,
    set: impl Fn(&mut Uploader, String) + Send + Sync + 'static,
) -> Entity {
    let col = commands.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).id();
    let lbl = field_label(commands, fonts, label, required);
    let input = text_input(commands, &fonts.ui, placeholder, "");
    style_input(commands, input);
    bind_text_input(
        commands,
        input,
        move |w| u(w).map(&get).unwrap_or_default(),
        move |w, v| {
            if let Some(mut s) = w.get_resource_mut::<Uploader>() {
                set(&mut s, v);
            }
        },
    );
    commands.entity(col).add_children(&[lbl, input]);
    col
}

fn style_input(commands: &mut Commands, input: Entity) {
    commands.entity(input).insert((
        Node {
            width: Val::Percent(100.0),
            min_width: Val::Px(0.0),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(rgb(popup_bg())),
        BorderColor::all(rgb(border())),
    ));
}

/// A dropdown bound two-way to a `usize` index in [`Uploader`].
fn dropdown_field(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    options: &[&str],
    get: impl Fn(&Uploader) -> usize + Send + Sync + 'static,
    set: impl Fn(&mut Uploader, usize) + Send + Sync + 'static,
) -> Entity {
    let col = commands.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).id();
    let lbl = field_label(commands, fonts, label, false);
    let dd = dropdown(commands, fonts, options, 0);
    bind_2way(
        commands,
        dd,
        move |w| u(w).map(&get).unwrap_or(0),
        move |w, v| {
            if let Some(mut s) = w.get_resource_mut::<Uploader>() {
                set(&mut s, *v);
            }
        },
    );
    commands.entity(col).add_children(&[lbl, dd]);
    col
}

/// A checkbox + label row bound two-way to a `bool` in [`Uploader`].
fn check_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    get: impl Fn(&Uploader) -> bool + Send + Sync + 'static,
    set: impl Fn(&mut Uploader, bool) + Send + Sync + 'static,
) -> Entity {
    let row = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() })
        .id();
    let cb = checkbox(commands, false);
    bind_2way(
        commands,
        cb,
        move |w| u(w).map(&get).unwrap_or(false),
        move |w, v| {
            if let Some(mut s) = w.get_resource_mut::<Uploader>() {
                set(&mut s, *v);
            }
        },
    );
    let t = commands
        .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), FocusPolicy::Pass))
        .id();
    commands.entity(row).add_children(&[cb, t]);
    row
}

/// A full-width accent primary button (Continue / Publish) with a trailing icon.
/// Built inline (rather than [`accent_button`]) so it can `flex_grow` to fill the
/// nav row like the web wizard's full-width buttons.
fn primary_button(commands: &mut Commands, fonts: &EmberFonts, label: &str, icon: &str) -> Entity {
    let btn = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(accent())),
            renzora_ember::widgets::HoverTint {
                base: rgb(accent()),
                hover: tint(accent(), 255),
                pressed: tint(accent(), 200),
            },
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    let t = commands
        .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.5), TextColor(rgb((255, 255, 255))), FocusPolicy::Pass))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, (255, 255, 255), 13.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    commands.entity(btn).add_children(&[t, ic]);
    btn
}

// ── Section 1 — your file ───────────────────────────────────────────────────────

/// Files lead the form: picking the file is what the seller came to do, and it
/// supplies the extension for the derived download name.
fn build_files_section(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let sec = section(commands);
    let head = heading(commands, fonts, "file-arrow-up", "Your file");
    let intro = help_text(commands, fonts, "Start here — everything else describes this file.");

    let main_lbl = field_label(commands, fonts, "File", true);
    let main_btn = file_pick_button(commands, fonts, PickMainBtn, "Choose a file to upload", |w| {
        u(w).and_then(|s| s.file.as_ref().map(|f| format!("{}  ({:.1} MB)", f.name, f.size as f64 / 1_048_576.0)))
    });
    let main_hint = help_text(commands, fonts, "Accepted formats vary by category — max 50 MB");

    // Mirrors the website's live "Buyers will download: …" hint.
    let dl_hint = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 9.5), TextColor(rgb(placeholder())), Node { margin: UiRect::top(Val::Px(3.0)), ..default() }))
        .id();
    bind_text(commands, dl_hint, |w| match u(w) {
        Some(s) if s.file.is_some() => format!("Buyers will download: {}", s.download_filename()),
        _ => String::new(),
    });

    let thumb_lbl = field_label(commands, fonts, "Cover Image", false);
    let thumb_hint = help_text(commands, fonts, "Recommended: 1280x720 (16:9). PNG or JPG.");
    let thumb_btn = file_pick_button(commands, fonts, PickThumbBtn, "Choose a cover image", |w| {
        u(w).and_then(|s| s.thumbnail.as_ref().map(|f| f.name.clone()))
    });

    let shots_lbl = field_label(commands, fonts, "Screenshots", false);
    let shots_btn = file_pick_button(commands, fonts, PickShotsBtn, "Choose screenshots…", |w| {
        u(w).map(|s| s.screenshots.len()).filter(|n| *n > 0).map(|n| format!("{n} screenshot{} selected", if n == 1 { "" } else { "s" }))
    });
    let shots_hint = help_text(commands, fonts, "Up to 10 images, shown in the gallery. PNG or JPG.");

    let video = text_field(
        commands, fonts, "Video Preview URL", false, "https://www.youtube.com/watch?v=… or .mp4 link",
        |s| s.video_url.clone(), |s, v| s.video_url = v,
    );

    // Audio previews only earn their place on Music.
    let audio_grp = group(commands, |w| cat_in(w, &["music"]));
    let audio_lbl = field_label(commands, fonts, "Audio Previews", false);
    let audio_btn = file_pick_button(commands, fonts, PickAudioBtn, "Choose audio previews…", |w| {
        u(w).map(|s| s.audio.len()).filter(|n| *n > 0).map(|n| format!("{n} audio file{} selected", if n == 1 { "" } else { "s" }))
    });
    let audio_hint = help_text(commands, fonts, "Let buyers listen before they buy. MP3, WAV, OGG or FLAC.");
    commands.entity(audio_grp).add_children(&[audio_lbl, audio_btn, audio_hint]);

    commands.entity(sec).add_children(&[
        head, intro, main_lbl, main_btn, main_hint, dl_hint,
        thumb_lbl, thumb_hint, thumb_btn, shots_lbl, shots_btn, shots_hint, video, audio_grp,
    ]);
    sec
}

// ── Section 2 — basics ──────────────────────────────────────────────────────────

fn build_basics_section(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let sec = section(commands);
    let head = heading(commands, fonts, "info", "Basics");

    let name = text_field(
        commands, fonts, "Name", true, "My Awesome Creation",
        |s| s.name.clone(), |s, v| s.name = v,
    );

    // Category picker — a grid of the fetched categories, as on the web form's
    // select. Selecting one reveals that category's detail group directly below.
    let cat_lbl = field_label(commands, fonts, "Category", true);
    let cat_grid = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() })
        .id();
    keyed_list(commands, cat_grid, category_snapshot);

    // Music details, directly under the category that reveals them.
    let music_grp = group(commands, |w| cat_in(w, &["music"]));
    let music_head = sub_head(commands, fonts, "Music Details");
    let bpm = text_field(commands, fonts, "BPM", false, "120", |s| s.bpm.clone(), |s, v| s.bpm = v);
    let genre = dropdown_field(commands, fonts, "Genre", GENRES, |s| s.genre, |s, v| s.genre = v);
    let loopable = check_row(commands, fonts, "Loop-friendly (seamless loop)", |s| s.loopable, |s, v| s.loopable = v);
    commands.entity(music_grp).add_children(&[music_head, bpm, genre, loopable]);

    let script_grp = group(commands, |w| cat_in(w, &["scripts", "plugins", "blueprints"]));
    let lang = dropdown_field(commands, fonts, "Scripting Language", SCRIPT_LANGS, |s| s.script_lang, |s, v| s.script_lang = v);
    commands.entity(script_grp).add_child(lang);

    // Description (textarea).
    let desc_col = commands.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).id();
    let desc_lbl = field_label(commands, fonts, "Description", true);
    let desc = textarea(commands, &fonts.ui, "Describe what this is, what's included, and how to use it…", "");
    commands.entity(desc).insert((
        Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(90.0),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(rgb(popup_bg())),
        BorderColor::all(rgb(border())),
    ));
    bind_text_input(
        commands, desc,
        |w| u(w).map(|s| s.description.clone()).unwrap_or_default(),
        |w, v| { if let Some(mut s) = w.get_resource_mut::<Uploader>() { s.description = v; } },
    );
    commands.entity(desc_col).add_children(&[desc_lbl, desc]);

    // Price.
    let price_col = commands.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).id();
    let price_field = text_field(
        commands, fonts, "Price (credits)", false, "0",
        |s| s.price.clone(), |s, v| s.price = v,
    );
    let price_hint = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 9.5), TextColor(rgb(placeholder())), Node { margin: UiRect::top(Val::Px(3.0)), ..default() }))
        .id();
    bind_text(commands, price_hint, |w| {
        let p = u(w).map(|s| s.price_credits()).unwrap_or(0);
        if p == 0 {
            "Free — anyone can download".to_string()
        } else {
            let usd = p as f64 * 0.10;
            let earn = (p as f64 * 0.8).floor() as i64;
            format!("{p} credits (${usd:.2}) — you earn {earn} credits")
        }
    });
    let earn_hint = help_text(commands, fonts, "You earn 80% of each sale. 1 credit = $0.10 USD.");
    commands.entity(price_col).add_children(&[price_field, price_hint, earn_hint]);

    let tags = build_tags_field(commands, fonts);

    let ev_lic = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, column_gap: Val::Px(12.0), ..default() })
        .id();
    let ev = dropdown_field(commands, fonts, "Minimum Engine Version", ENGINE_VERSIONS, |s| s.engine_version, |s, v| s.engine_version = v);
    let lic = dropdown_field(commands, fonts, "License", LICENSES, |s| s.license, |s, v| s.license = v);
    for e in [ev, lic] {
        commands.entity(e).insert(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, flex_basis: Val::Px(0.0), ..default() });
    }
    commands.entity(ev_lic).add_children(&[ev, lic]);
    let ai = check_row(commands, fonts, "This asset was created with AI assistance", |s| s.ai_generated, |s, v| s.ai_generated = v);

    commands.entity(sec).add_children(&[
        head, name, cat_lbl, cat_grid, music_grp, script_grp,
        desc_col, price_col, tags, ev_lic, ai,
    ]);
    sec
}

// ── Section 3 — credit / attribution ────────────────────────────────────────────

fn build_credit_section(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let sec = section(commands);
    let head = heading(commands, fonts, "heart", "Credit / Attribution");
    let intro = help_text(commands, fonts, "If this asset is from another creator, credit them here. Credited assets are automatically free.");
    let name = text_field(
        commands, fonts, "Original Creator Name", false, "e.g. KayKit, Kenney",
        |s| s.credit_name.clone(), |s, v| s.credit_name = v,
    );
    let url = text_field(
        commands, fonts, "Creator Website / Source Link", false, "https://kaykit.itch.io",
        |s| s.credit_url.clone(), |s, v| s.credit_url = v,
    );
    let notice = commands
        .spawn((
            Text::new("This asset will be published as free because it credits another creator."),
            ui_font(&fonts.ui, 9.5),
            TextColor(rgb((110, 231, 183))),
            Node { display: Display::None, ..default() },
        ))
        .id();
    bind_display(commands, notice, |w| u(w).map(|s| !s.credit_name.trim().is_empty()).unwrap_or(false));
    commands.entity(sec).add_children(&[head, intro, name, url, notice]);
    sec
}

// ── Publish ─────────────────────────────────────────────────────────────────────

fn build_publish_row(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() })
        .id();
    let btn = primary_button(commands, fonts, "Publish", "rocket-launch");
    commands.entity(btn).insert(PublishBtn);
    let note = help_text(commands, fonts, "Published at version 1.0.0. By publishing, you agree to the Renzora content guidelines.");
    commands.entity(col).add_children(&[btn, note]);
    col
}

/// One item — the category dropdown — rebuilt when the fetched list changes.
/// `dropdown` takes its options at build time, and the categories arrive from
/// the network, so the field is built inside a keyed list rather than up front.
fn category_snapshot(world: &Rx) -> KeyedSnapshot {
    let Some(state) = u(world) else {
        return note("");
    };
    if state.cats_loading && state.categories.is_empty() {
        return note("Loading categories…");
    }
    if state.categories.is_empty() {
        return note("No categories available.");
    }
    let cats = state.categories.clone();
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for c in &cats {
        (&c.slug, &c.name).hash(&mut h);
    }
    let sig = h.finish();
    KeyedSnapshot {
        items: vec![(0, sig)],
        build: Box::new(move |c, f, _| {
            let mut labels: Vec<&str> = vec!["Select a category…"];
            labels.extend(cats.iter().map(|x| x.name.as_str()));
            let dd = dropdown(c, f, &labels, 0);
            bind_2way(
                c,
                dd,
                |w| u(w).map(|s| s.category_index).unwrap_or(0),
                |w, v| {
                    if let Some(mut s) = w.get_resource_mut::<Uploader>() {
                        s.category_index = *v;
                        s.error = None;
                    }
                },
            );
            dd
        }),
    }
}





fn build_tags_field(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let wrap = commands.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).id();
    let lbl = field_label(commands, fonts, "Tags", false);
    // Pills row.
    let pills = commands
        .spawn(Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(6.0), row_gap: Val::Px(5.0), margin: UiRect::bottom(Val::Px(6.0)), ..default() })
        .id();
    keyed_list(commands, pills, tag_pills_snapshot);
    // Input.
    let input = text_input(commands, &fonts.ui, "Type a tag and press comma…", "");
    style_input(commands, input);
    bind_text_input(
        commands, input,
        |w| u(w).map(|s| s.tag_query.clone()).unwrap_or_default(),
        |w, v| { if let Some(mut s) = w.get_resource_mut::<Uploader>() { s.tag_query = v; } },
    );
    // Suggestions.
    let sugg = commands
        .spawn(Node { flex_direction: FlexDirection::Column, margin: UiRect::top(Val::Px(4.0)), ..default() })
        .id();
    keyed_list(commands, sugg, tag_suggestions_snapshot);
    let help = help_text(commands, fonts, "Add up to 5 tags. Press comma to add. New tags are submitted for review.");
    commands.entity(wrap).add_children(&[lbl, pills, input, sugg, help]);
    wrap
}

fn tag_pills_snapshot(world: &Rx) -> KeyedSnapshot {
    let Some(state) = u(world) else { return empty(); };
    if state.tags.is_empty() {
        return empty();
    }
    let tags = state.tags.clone();
    use std::hash::{Hash, Hasher};
    let items: Vec<(u64, u64)> = tags
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            (i, t).hash(&mut k);
            (k.finish(), k.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| tag_pill(c, f, &tags[i], i)),
    }
}

fn tag_pill(commands: &mut Commands, fonts: &EmberFonts, tag: &str, index: usize) -> Entity {
    let pill = commands
        .spawn((
            Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() },
            BackgroundColor(tint(accent(), 40)),
        ))
        .id();
    let t = commands
        .spawn((Text::new(tag.to_string()), ui_font(&fonts.ui, 10.5), TextColor(rgb(accent())), FocusPolicy::Pass))
        .id();
    let x = commands
        .spawn((
            Node { align_items: AlignItems::Center, ..default() },
            Interaction::default(),
            TagRemoveBtn(index),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    let xg = icon_text(commands, &fonts.phosphor, "x", (200, 200, 210), 10.0);
    commands.entity(xg).insert(FocusPolicy::Pass);
    commands.entity(x).add_child(xg);
    commands.entity(pill).add_children(&[t, x]);
    pill
}

fn tag_suggestions_snapshot(world: &Rx) -> KeyedSnapshot {
    let Some(state) = u(world) else { return empty(); };
    if state.tag_suggestions.is_empty() {
        return empty();
    }
    let sugg = state.tag_suggestions.clone();
    use std::hash::{Hash, Hasher};
    let items: Vec<(u64, u64)> = sugg
        .iter()
        .map(|t| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            t.hash(&mut h);
            (h.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| tag_suggest_row(c, f, &sugg[i])),
    }
}

fn tag_suggest_row(commands: &mut Commands, fonts: &EmberFonts, name: &str) -> Entity {
    let row = commands
        .spawn((
            Node { width: Val::Percent(100.0), padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(rgb(hover_bg())),
            Interaction::default(),
            TagAddBtn(name.to_string()),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    let t = commands
        .spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), FocusPolicy::Pass))
        .id();
    commands.entity(row).add_child(t);
    row
}

/// A step-4 detail group, gated on a category/content-type predicate.
fn group(commands: &mut Commands, pred: impl Fn(&Rx) -> bool + Send + Sync + 'static) -> Entity {
    let g = commands
        .spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(12.0), display: Display::None, ..default() })
        .id();
    bind_display(commands, g, pred);
    g
}

fn sub_head(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((Text::new(text.to_uppercase()), ui_font(&fonts.ui, 9.5), TextColor(rgb(text_muted()))))
        .id()
}

fn cat_in(w: &Rx, list: &[&str]) -> bool {
    u(w).map(|s| list.contains(&s.category_slug())).unwrap_or(false)
}

/// A dashed file-picker button whose label shows the current selection.
fn file_pick_button<M: Component>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    marker: M,
    empty_label: &str,
    picked: impl Fn(&Rx) -> Option<String> + Send + Sync + 'static,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(16.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            marker,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, "upload-simple", text_muted(), 15.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let empty = empty_label.to_string();
    let t = commands
        .spawn((Text::new(empty_label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), FocusPolicy::Pass))
        .id();
    bind_text(commands, t, move |w| picked(w).unwrap_or_else(|| empty.clone()));
    commands.entity(btn).add_children(&[ic, t]);
    btn
}

// ── Snapshots helpers ────────────────────────────────────────────────────────────

fn empty() -> KeyedSnapshot {
    KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _f, _i| c.spawn_empty().id()) }
}

fn note(text: &str) -> KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    let text = text.to_string();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    KeyedSnapshot {
        items: vec![(u64::MAX, h.finish())],
        build: Box::new(move |c, f, _| {
            c.spawn((Text::new(text.clone()), ui_font(&f.ui, 11.0), TextColor(rgb(text_muted())))).id()
        }),
    }
}

// ── Systems ──────────────────────────────────────────────────────────────────────

/// Drain all in-flight worker results into the resource.
fn uploader_poll(mut state: ResMut<Uploader>) {
    // Categories.
    if let Some(rx) = &state.cats_rx {
        if let Ok(res) = rx.try_recv() {
            state.cats_rx = None;
            state.cats_loading = false;
            match res {
                Ok(cats) => state.categories = cats,
                Err(e) => state.error = Some(e),
            }
        }
    }
    // Tag suggestions.
    if let Some(rx) = &state.tag_rx {
        if let Ok(sugg) = rx.try_recv() {
            state.tag_rx = None;
            state.tag_suggestions = sugg;
        }
    }
    // File picks.
    while let Ok(msg) = state.pick_rx.try_recv() {
        match msg {
            PickMsg::Main(f) => state.file = Some(f),
            PickMsg::Thumb(f) => state.thumbnail = Some(f),
            PickMsg::Screenshots(v) => state.screenshots = v.into_iter().take(10).collect(),
            PickMsg::Audio(v) => state.audio = v.into_iter().take(10).collect(),
        }
    }
    // Submit result.
    if let Some(rx) = &state.submit_rx {
        if let Ok(res) = rx.try_recv() {
            state.submit_rx = None;
            state.submitting = false;
            match res {
                Ok(item) => {
                    let base = crate::auth::client::api_base();
                    state.success = Some("Asset published!".to_string());
                    state.success_url = Some(format!("{base}/marketplace/asset/{}", item.slug));
                    state.error = None;
                }
                Err(e) => state.error = Some(e),
            }
        }
    }
}

/// Kick the one category fetch. Called when the publish view is opened, which
/// is the click that used to be "pick a content type". One-shot: a failed load
/// leaves the list empty, and retrying it per-frame would spawn a worker per
/// frame.
fn ensure_categories(state: &mut Uploader) {
    if state.cats_attempted {
        return;
    }
    state.cats_attempted = true;
    state.cats_loading = true;
    let (tx, rx) = unbounded();
    state.cats_rx = Some(rx);
    std::thread::spawn(move || {
        let _ = tx.send(crate::auth::marketplace::list_categories());
    });
}

/// Open the publish view: flip the store into it and start the category fetch.
pub(crate) fn begin_publishing(world: &mut World) {
    if let Some(mut store) = world.get_resource_mut::<crate::store::HubStoreData>() {
        store.publishing = true;
    }
    if let Some(mut state) = world.get_resource_mut::<Uploader>() {
        ensure_categories(&mut state);
    }
}



/// Publish.
fn nav_click(
    publish_q: Query<&Interaction, (With<PublishBtn>, Changed<Interaction>)>,
    session: Option<Res<AuthSession>>,
    mut state: ResMut<Uploader>,
) {
    if publish_q.iter().any(|i| *i == Interaction::Pressed) {
        start_publish(&mut state, session.as_deref());
    }
}

/// Step 5 — file pickers. Each opens a native dialog on a worker thread.
fn pick_click(
    main: Query<&Interaction, (With<PickMainBtn>, Changed<Interaction>)>,
    thumb: Query<&Interaction, (With<PickThumbBtn>, Changed<Interaction>)>,
    shots: Query<&Interaction, (With<PickShotsBtn>, Changed<Interaction>)>,
    audio: Query<&Interaction, (With<PickAudioBtn>, Changed<Interaction>)>,
    state: Res<Uploader>,
) {
    if main.iter().any(|i| *i == Interaction::Pressed) {
        spawn_pick(state.pick_tx.clone(), PickKind::Main);
    }
    if thumb.iter().any(|i| *i == Interaction::Pressed) {
        spawn_pick(state.pick_tx.clone(), PickKind::Thumb);
    }
    if shots.iter().any(|i| *i == Interaction::Pressed) {
        spawn_pick(state.pick_tx.clone(), PickKind::Screenshots);
    }
    if audio.iter().any(|i| *i == Interaction::Pressed) {
        spawn_pick(state.pick_tx.clone(), PickKind::Audio);
    }
}

enum PickKind {
    Main,
    Thumb,
    Screenshots,
    Audio,
}

fn spawn_pick(tx: Sender<PickMsg>, kind: PickKind) {
    std::thread::spawn(move || {
        let to_picked = |p: PathBuf| -> Option<PickedFile> {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
            let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            Some(PickedFile { path: p, name, size })
        };
        match kind {
            PickKind::Main => {
                if let Some(p) = rfd::FileDialog::new().pick_file() {
                    if let Some(f) = to_picked(p) {
                        let _ = tx.send(PickMsg::Main(f));
                    }
                }
            }
            PickKind::Thumb => {
                if let Some(p) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif"]).pick_file() {
                    if let Some(f) = to_picked(p) {
                        let _ = tx.send(PickMsg::Thumb(f));
                    }
                }
            }
            PickKind::Screenshots => {
                if let Some(paths) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif"]).pick_files() {
                    let v: Vec<PickedFile> = paths.into_iter().filter_map(to_picked).collect();
                    let _ = tx.send(PickMsg::Screenshots(v));
                }
            }
            PickKind::Audio => {
                if let Some(paths) = rfd::FileDialog::new().add_filter("Audio", &["mp3", "wav", "ogg", "flac"]).pick_files() {
                    let v: Vec<PickedFile> = paths.into_iter().filter_map(to_picked).collect();
                    let _ = tx.send(PickMsg::Audio(v));
                }
            }
        }
    });
}

/// Tag remove / add-suggestion clicks.
fn tag_click(
    remove: Query<(&Interaction, &TagRemoveBtn), Changed<Interaction>>,
    add: Query<(&Interaction, &TagAddBtn), Changed<Interaction>>,
    mut state: ResMut<Uploader>,
) {
    for (interaction, btn) in &remove {
        if *interaction == Interaction::Pressed && btn.0 < state.tags.len() {
            state.tags.remove(btn.0);
            break;
        }
    }
    for (interaction, btn) in &add {
        if *interaction == Interaction::Pressed {
            add_tag(&mut state, btn.0.clone());
            state.tag_query.clear();
            state.tag_suggestions.clear();
            break;
        }
    }
}

fn add_tag(state: &mut Uploader, raw: String) {
    let clean = raw.trim().to_lowercase();
    if clean.is_empty() || state.tags.len() >= 5 || state.tags.contains(&clean) {
        return;
    }
    state.tags.push(clean);
}

/// Watch the tag input: a comma commits a tag; otherwise a changed query kicks a
/// debounced-by-value autocomplete search.
fn tag_search(mut state: ResMut<Uploader>) {
    // Comma commits the tag before it.
    if state.tag_query.contains(',') {
        let query = state.tag_query.clone();
        let mut parts: Vec<&str> = query.split(',').collect();
        let remainder = parts.pop().unwrap_or("").to_string();
        for p in parts {
            if !p.trim().is_empty() {
                add_tag(&mut state, p.to_string());
            }
        }
        state.tag_query = remainder;
        state.tag_suggestions.clear();
        state.tag_last_searched.clear();
        return;
    }
    let q = state.tag_query.trim().to_string();
    if q == state.tag_last_searched {
        return;
    }
    state.tag_last_searched = q.clone();
    if q.is_empty() {
        state.tag_suggestions.clear();
        return;
    }
    let (tx, rx) = unbounded();
    state.tag_rx = Some(rx);
    std::thread::spawn(move || {
        let mut names: Vec<String> = publish::search_tags(&q)
            .unwrap_or_default()
            .into_iter()
            .map(|t| t.name)
            .collect();
        // Always offer to submit the typed query as a new tag if not present.
        if !names.iter().any(|n| n.eq_ignore_ascii_case(&q)) {
            names.push(q);
        }
        let _ = tx.send(names);
    });
}

/// Open the published item's page in the browser.
fn success_link_click(q: Query<&Interaction, (With<SuccessLinkBtn>, Changed<Interaction>)>, state: Res<Uploader>) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        if let Some(url) = &state.success_url {
            crate::store::open_url(url);
        }
    }
}

/// Kick off the publish: read files + upload + attach media, all on one worker
/// thread. Enforces the "credited asset → free" rule the website does.
fn start_publish(state: &mut Uploader, session: Option<&AuthSession>) {
    if state.submitting {
        return;
    }
    if state.name.trim().is_empty() {
        state.error = Some("Name is required.".to_string());
        return;
    }
    if state.category_slug().is_empty() {
        state.error = Some("Please choose a category.".to_string());
        return;
    }
    if state.description.trim().is_empty() {
        state.error = Some("Description is required.".to_string());
        return;
    }
    let Some(file) = state.file.clone() else {
        state.error = Some("Please select a file to upload.".to_string());
        return;
    };
    let Some(session) = session.filter(|s| s.is_signed_in()) else {
        state.error = Some("Please sign in first.".to_string());
        return;
    };
    let session = clone_session(session);

    // Build metadata exactly as the website's handleSubmit does.
    let credit_name = state.credit_name.trim().to_string();
    let credit_url = state.credit_url.trim().to_string();
    // Crediting another creator forces the asset free, as on the website.
    let price = if credit_name.is_empty() { state.price_credits() } else { 0 };

    // The per-category extras the server stores as a free-form object. Only
    // fields the selected category actually shows are sent.
    let mut extra = serde_json::Map::new();
    if state.engine_version > 0 {
        if let Some(v) = ENGINE_VERSIONS.get(state.engine_version) {
            extra.insert("min_engine_version".into(), (*v).into());
        }
    }
    if state.category_slug() == "music" {
        let bpm = state.bpm.trim();
        if !bpm.is_empty() {
            extra.insert("bpm".into(), bpm.into());
        }
        if let Some(g) = GENRE_VALUES.get(state.genre).filter(|g| !g.is_empty()) {
            extra.insert("genre".into(), (*g).into());
        }
        if state.loopable {
            extra.insert("loopable".into(), true.into());
        }
    }
    if ["scripts", "plugins", "blueprints"].contains(&state.category_slug()) {
        if let Some(l) = SCRIPT_LANG_VALUES.get(state.script_lang).filter(|l| !l.is_empty()) {
            extra.insert("script_language".into(), (*l).into());
        }
    }

    let meta = PublishMeta {
        name: state.name.trim().to_string(),
        description: state.description.trim().to_string(),
        category: state.category_slug().to_string(),
        price_credits: price,
        // Never asked for: a version belongs to an update, not a first publish.
        version: "1.0.0".to_string(),
        tags: Some(state.tags.clone()),
        download_filename: Some(state.download_filename()),
        credit_name: (!credit_name.is_empty()).then_some(credit_name),
        credit_url: (!credit_url.is_empty()).then_some(credit_url),
        licence: LICENCE_VALUES.get(state.license).copied().unwrap_or("standard").to_string(),
        ai_generated: state.ai_generated,
        metadata: serde_json::Value::Object(extra),
    };

    let thumb = state.thumbnail.clone();
    let screenshots = state.screenshots.clone();
    let video_url = state.video_url.trim().to_string();
    let audio = state.audio.clone();

    state.submitting = true;
    state.error = None;
    state.success = None;
    let (tx, rx) = unbounded();
    state.submit_rx = Some(rx);

    std::thread::spawn(move || {
        let result = (|| -> Result<UploadedItem, String> {
            let main = read_upload_file(&file)?;
            let thumb_up = thumb.as_ref().map(read_upload_file).transpose()?;
            let item = publish::upload_asset(&session, &meta, &main, thumb_up.as_ref())?;
            // Attach media (best-effort — failures don't fail the publish).
            for shot in &screenshots {
                if let Ok(f) = read_upload_file(shot) {
                    let _ = publish::add_asset_media(&session, &item.id, &MediaUpload::Image(f));
                }
            }
            {
                if !video_url.is_empty() {
                    let _ = publish::add_asset_media(&session, &item.id, &MediaUpload::Video(video_url.clone()));
                }
                for clip in &audio {
                    if let Ok(f) = read_upload_file(clip) {
                        let _ = publish::add_asset_media(&session, &item.id, &MediaUpload::Audio(f));
                    }
                }
            }
            Ok(item)
        })();
        let _ = tx.send(result);
    });
}

fn read_upload_file(f: &PickedFile) -> Result<UploadFile, String> {
    let bytes = std::fs::read(&f.path).map_err(|e| format!("Could not read {}: {e}", f.name))?;
    Ok(UploadFile {
        filename: f.name.clone(),
        content_type: guess_content_type(&f.name),
        bytes,
    })
}

fn guess_content_type(name: &str) -> String {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "zip" => "application/zip",
        "json" | "gltf" => "application/json",
        _ => "application/octet-stream",
    }
    .to_string()
}

/// `AuthSession` isn't `Clone`; copy its fields so the worker owns a session.
fn clone_session(s: &AuthSession) -> AuthSession {
    AuthSession {
        user: s.user.clone(),
        access_token: s.access_token.clone(),
        refresh_token: s.refresh_token.clone(),
    }
}
