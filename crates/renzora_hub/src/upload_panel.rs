//! The **Publish** panel — an in-editor asset/game uploader that mirrors the
//! website's `/marketplace/upload` wizard field-for-field (see
//! `website/crates/web/src/pages/upload.rs`).
//!
//! Six steps, identical to the web wizard: **1** content type (Marketplace Asset
//! vs Game), **2** category, **3** basic info (name, description, version, price,
//! and — for assets — tags, download filename, credit/attribution), **4**
//! adaptive type/category detail fields, **5** files & media (main file, cover,
//! screenshots, and — for assets — a video URL + audio previews), **6** review &
//! publish.
//!
//! The whole wizard is built once; steps show/hide by [`bind_display`] on the
//! current step (the web version hides `.wizard-step` divs the same way) so field
//! widgets — and their two-way bindings to [`Uploader`] — survive navigation. All
//! form state lives in the [`Uploader`] resource, so a dock move that rebuilds the
//! panel content re-seeds every field from state rather than losing it.
//!
//! Networking matches the rest of the hub: file reads + the multipart upload run
//! on a worker thread and post their result back over a `crossbeam_channel`,
//! drained in [`uploader_poll`]. Native file dialogs (`rfd`) also run on a worker
//! thread (they block), exactly like the profile avatar/cover upload.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use crossbeam_channel::{unbounded, Receiver, Sender};

use renzora_auth::marketplace::Category;
use renzora_auth::publish::{
    self, ContentType, MediaUpload, PublishMeta, UploadFile, UploadedItem,
};
use renzora_auth::session::AuthSession;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, bind_bg, bind_display, bind_text, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_text_input, checkbox, dropdown, text_input, textarea, tint};
use renzora::SplashState;

/// Panel id — matches the `PANEL_META` entry in `renzora_shell` and the
/// `focus_or_add_panel` call the Upload-Asset button makes.
pub(crate) const PANEL_ID: &str = "asset_uploader";

/// License options for the step-4 select (label pairs). Value-for-value the web
/// wizard's `<select id="w-license">`. Cosmetic parity: like the website, the
/// submitted metadata does not carry the license (the server defaults it).
const LICENSES: &[&str] = &[
    "Standard Marketplace License",
    "MIT",
    "Apache 2.0",
    "GPL 3.0",
    "CC BY 4.0",
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
const SCRIPT_LANGS: &[&str] = &["Select…", "Lua", "Rhai", "WGSL (Shader)", "Visual Blueprint", "Other"];
const TEX_RES: &[&str] = &["Select…", "512x512", "1024x1024", "2048x2048", "4096x4096"];
const PIPELINES: &[&str] = &["Select…", "PBR (Physically Based)", "Unlit", "Toon / Cel-Shaded", "Custom WGSL"];

const STEP_LABELS: [&str; 6] = [
    "Content Type",
    "Category",
    "Basic Information",
    "Additional Details",
    "Files & Media",
    "Review & Publish",
];

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

/// All wizard state. Text fields two-way-bind here via [`bind_text_input`];
/// dropdowns/checkboxes via [`bind_2way`] on their `Bound<_>`. Step-4 detail
/// fields are stored for persistence but — matching the website — are not sent.
#[derive(Resource)]
struct Uploader {
    step: usize,
    content_type: Option<ContentType>,

    // Categories (fetched per content type in step 2).
    categories: Vec<Category>,
    cats_loading: bool,
    cats_rx: Option<Receiver<Result<Vec<Category>, String>>>,
    category: String,
    category_name: String,

    // Step 3 — basic info.
    name: String,
    description: String,
    version: String,
    price: String,
    download_filename: String,
    credit_name: String,
    credit_url: String,

    // Tags (asset only).
    tags: Vec<String>,
    tag_query: String,
    tag_suggestions: Vec<String>,
    tag_last_searched: String,
    tag_rx: Option<Receiver<Vec<String>>>,

    // Step 4 — detail fields (cosmetic parity; not submitted).
    ai_generated: bool,
    engine_versions: String,
    license: usize,
    bpm: String,
    genre: usize,
    loopable: bool,
    script_lang: usize,
    dependencies: String,
    polycount: String,
    texres: usize,
    resolution: String,
    tileable: bool,
    pipeline: usize,
    mat_texres: usize,
    plat_windows: bool,
    plat_mac: bool,
    plat_linux: bool,
    plat_web: bool,
    sysreq: String,

    // Step 5 — files & media.
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
            step: 1,
            content_type: None,
            categories: Vec::new(),
            cats_loading: false,
            cats_rx: None,
            category: String::new(),
            category_name: String::new(),
            name: String::new(),
            description: String::new(),
            version: "1.0.0".to_string(),
            price: "0".to_string(),
            download_filename: String::new(),
            credit_name: String::new(),
            credit_url: String::new(),
            tags: Vec::new(),
            tag_query: String::new(),
            tag_suggestions: Vec::new(),
            tag_last_searched: String::new(),
            tag_rx: None,
            ai_generated: false,
            engine_versions: String::new(),
            license: 0,
            bpm: String::new(),
            genre: 0,
            loopable: false,
            script_lang: 0,
            dependencies: String::new(),
            polycount: String::new(),
            texres: 0,
            resolution: String::new(),
            tileable: false,
            pipeline: 0,
            mat_texres: 0,
            plat_windows: true,
            plat_mac: false,
            plat_linux: false,
            plat_web: false,
            sysreq: String::new(),
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
    fn is_asset(&self) -> bool {
        self.content_type == Some(ContentType::Asset)
    }
    fn price_credits(&self) -> i64 {
        self.price.trim().parse::<i64>().unwrap_or(0).max(0)
    }
}

fn u(w: &World) -> Option<&Uploader> {
    w.get_resource::<Uploader>()
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub(crate) struct UploaderPanel;

impl Plugin for UploaderPanel {
    fn build(&self, app: &mut App) {
        app.init_resource::<Uploader>();
        app.register_panel_content(PANEL_ID, true, build);
        app.add_systems(
            Update,
            (
                uploader_poll,
                ct_click,
                cat_click,
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
struct CtAssetBtn;
#[derive(Component)]
struct CtGameBtn;
#[derive(Component)]
struct CatBtn {
    slug: String,
    name: String,
}
#[derive(Component)]
struct NextBtn;
#[derive(Component)]
struct BackBtn;
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

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
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

    // Header.
    let title = commands
        .spawn((Text::new("Publish Content"), ui_font(&fonts.ui, 22.0), TextColor(rgb(text_primary()))))
        .id();
    let subtitle = commands
        .spawn((
            Text::new("Share your creation with the Renzora community."),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(text_muted())),
        ))
        .id();

    let progress = build_progress(commands);
    let step_label = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted()))))
        .id();
    bind_text(commands, step_label, |w| {
        let s = u(w).map(|s| s.step).unwrap_or(1).clamp(1, 6);
        format!("STEP {} OF 6 — {}", s, STEP_LABELS[s - 1])
    });

    // Error / success banners.
    let error = banner(commands, fonts, "warning-circle", (224, 96, 96));
    bind_display(commands, error, |w| u(w).map(|s| s.error.is_some()).unwrap_or(false));
    bind_banner_text(commands, fonts, error, |w| u(w).and_then(|s| s.error.clone()).unwrap_or_default(), (224, 96, 96), "warning-circle");

    let success = success_banner(commands, fonts);

    let steps = [
        build_step1(commands, fonts),
        build_step2(commands, fonts),
        build_step3(commands, fonts),
        build_step4(commands, fonts),
        build_step5(commands, fonts),
        build_step6(commands, fonts),
    ];

    commands
        .entity(root)
        .add_children(&[title, subtitle, progress, step_label, error, success]);
    for s in steps {
        commands.entity(root).add_child(s);
    }
    root
}

/// The 6-dot / 5-bar progress rail, each segment colored by the current step.
fn build_progress(commands: &mut Commands) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    for i in 1..=6usize {
        let dot = commands
            .spawn((
                Node {
                    width: Val::Px(11.0),
                    height: Val::Px(11.0),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(rgb(border())),
            ))
            .id();
        bind_bg(commands, dot, move |w| {
            let s = u(w).map(|s| s.step).unwrap_or(1);
            if i <= s { rgb(accent()) } else { rgb(border()) }
        });
        commands.entity(row).add_child(dot);
        if i < 6 {
            let bar = commands
                .spawn((
                    Node { width: Val::Px(38.0), height: Val::Px(2.0), ..default() },
                    BackgroundColor(rgb(border())),
                ))
                .id();
            bind_bg(commands, bar, move |w| {
                let s = u(w).map(|s| s.step).unwrap_or(1);
                if i < s { rgb(accent()) } else { rgb(border()) }
            });
            commands.entity(row).add_child(bar);
        }
    }
    row
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
    get: impl Fn(&World) -> String + Send + Sync + 'static,
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

/// A step wrapper: the rounded card body, shown only on step `n`.
fn step_card(commands: &mut Commands, n: usize) -> Entity {
    let card = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(14.0),
                display: Display::None,
                ..default()
            },
        ))
        .id();
    bind_display(commands, card, move |w| u(w).map(|s| s.step == n).unwrap_or(false));
    card
}

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

/// The Back / Continue (or Publish) navigation row for a step.
fn nav_row(commands: &mut Commands, fonts: &EmberFonts, has_back: bool, next: NavNext) -> Entity {
    let row = commands
        .spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(10.0), margin: UiRect::top(Val::Px(4.0)), ..default() })
        .id();
    if has_back {
        let back = ghost_button(commands, fonts, "Back");
        commands.entity(back).insert(BackBtn);
        commands.entity(row).add_child(back);
    }
    match next {
        NavNext::Continue => {
            let cont = primary_button(commands, fonts, "Continue", "arrow-right");
            commands.entity(cont).insert(NextBtn);
            commands.entity(row).add_child(cont);
        }
        NavNext::Publish => {
            let pub_btn = primary_button(commands, fonts, "Publish", "rocket-launch");
            commands.entity(pub_btn).insert(PublishBtn);
            commands.entity(row).add_child(pub_btn);
        }
        NavNext::None => {}
    }
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

enum NavNext {
    Continue,
    Publish,
    None,
}

fn ghost_button(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let b = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(16.0), Val::Px(9.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    let t = commands
        .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), FocusPolicy::Pass))
        .id();
    commands.entity(b).add_child(t);
    b
}

// ── Step 1 — content type ───────────────────────────────────────────────────────

fn build_step1(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let card = step_card(commands, 1);
    let grid = commands
        .spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(12.0), ..default() })
        .id();
    let asset = choice_card(commands, fonts, "package", "Marketplace Asset", "3D models, scripts, audio, textures, plugins, and more.");
    commands.entity(asset).insert(CtAssetBtn);
    let game = choice_card(commands, fonts, "game-controller", "Game", "Publish a playable game for the Renzora community.");
    commands.entity(game).insert(CtGameBtn);
    commands.entity(grid).add_children(&[asset, game]);
    commands.entity(card).add_child(grid);
    card
}

fn choice_card(commands: &mut Commands, fonts: &EmberFonts, icon: &str, title: &str, desc: &str) -> Entity {
    let card = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(22.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(14.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, accent(), 22.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let t = commands
        .spawn((Text::new(title.to_string()), ui_font(&fonts.ui, 15.0), TextColor(rgb(text_primary())), FocusPolicy::Pass))
        .id();
    let d = commands
        .spawn((Text::new(desc.to_string()), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_muted())), FocusPolicy::Pass))
        .id();
    commands.entity(card).add_children(&[ic, t, d]);
    card
}

// ── Step 2 — category ───────────────────────────────────────────────────────────

fn build_step2(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let card = step_card(commands, 2);
    let sec = section(commands);
    let head = commands
        .spawn((Text::new("Choose a category"), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary()))))
        .id();
    let sub = commands
        .spawn((Text::new("This helps buyers find your content."), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_muted()))))
        .id();
    let grid = commands
        .spawn(Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(8.0), row_gap: Val::Px(8.0), ..default() })
        .id();
    keyed_list(commands, grid, category_snapshot);
    commands.entity(sec).add_children(&[head, sub, grid]);
    let nav = nav_row(commands, fonts, true, NavNext::None);
    commands.entity(card).add_children(&[sec, nav]);
    card
}

fn category_snapshot(world: &World) -> KeyedSnapshot {
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
    let items: Vec<(u64, u64)> = cats
        .iter()
        .map(|c| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            c.slug.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (&c.slug, &c.name).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| category_button(c, f, &cats[i])),
    }
}

fn category_button(commands: &mut Commands, fonts: &EmberFonts, cat: &Category) -> Entity {
    let btn = commands
        .spawn((
            Node {
                flex_basis: Val::Px(190.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(9.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            CatBtn { slug: cat.slug.clone(), name: cat.name.clone() },
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    let icon = clean_icon(&cat.icon);
    let ic = icon_text(commands, &fonts.phosphor, &icon, accent(), 15.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let t = commands
        .spawn((Text::new(cat.name.clone()), ui_font(&fonts.ui, 11.5), TextColor(rgb(text_primary())), FocusPolicy::Pass))
        .id();
    commands.entity(btn).add_children(&[ic, t]);
    btn
}

/// Category icons arrive as web classes like `"ph ph-cube"`; the engine's glyph
/// lookup wants the bare kebab name (`"cube"`).
fn clean_icon(raw: &str) -> String {
    raw.split_whitespace()
        .last()
        .unwrap_or("folder")
        .trim_start_matches("ph-")
        .to_string()
}

// ── Step 3 — basic info ─────────────────────────────────────────────────────────

fn build_step3(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let card = step_card(commands, 3);
    let sec = section(commands);
    let head = heading(commands, fonts, "info", "Basic Information");

    let name = text_field(
        commands, fonts, "Name", true, "My Awesome Creation",
        |s| s.name.clone(), |s, v| s.name = v,
    );

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

    // Version + price row.
    let vp_row = commands
        .spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(12.0), ..default() })
        .id();
    let version_field = text_field(
        commands, fonts, "Version", false, "1.0.0",
        |s| s.version.clone(), |s, v| s.version = v,
    );
    let version = wide(commands, version_field);
    let price_col = commands.spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, flex_basis: Val::Px(0.0), ..default() }).id();
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
    commands.entity(price_col).add_children(&[price_field, price_hint]);
    commands.entity(vp_row).add_children(&[version, price_col]);

    commands.entity(sec).add_children(&[head, name, desc_col, vp_row]);

    // Asset-only: tags, download filename, credit.
    let tags = build_tags_field(commands, fonts);
    let dl_field = text_field(
        commands, fonts, "Download Filename", false, "my-asset.zip",
        |s| s.download_filename.clone(), |s, v| s.download_filename = v,
    );
    let dl = wrap_asset(commands, dl_field);
    let credit = build_credit_field(commands, fonts);
    commands.entity(sec).add_children(&[tags, dl, credit]);

    let nav = nav_row(commands, fonts, true, NavNext::Continue);
    commands.entity(card).add_children(&[sec, nav]);
    card
}

fn wide(commands: &mut Commands, e: Entity) -> Entity {
    commands.entity(e).insert(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, flex_basis: Val::Px(0.0), ..default() });
    e
}

/// Wrap a field so it's only visible for the Asset content type.
fn wrap_asset(commands: &mut Commands, child: Entity) -> Entity {
    let wrap = commands.spawn(Node { flex_direction: FlexDirection::Column, display: Display::None, ..default() }).id();
    bind_display(commands, wrap, |w| u(w).map(|s| s.is_asset()).unwrap_or(false));
    commands.entity(wrap).add_child(child);
    wrap
}

fn build_tags_field(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let wrap = commands.spawn(Node { flex_direction: FlexDirection::Column, display: Display::None, ..default() }).id();
    bind_display(commands, wrap, |w| u(w).map(|s| s.is_asset()).unwrap_or(false));
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

fn tag_pills_snapshot(world: &World) -> KeyedSnapshot {
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

fn tag_suggestions_snapshot(world: &World) -> KeyedSnapshot {
    let Some(state) = u(world) else { return empty(); };
    if !state.is_asset() || state.tag_suggestions.is_empty() {
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

fn build_credit_field(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let wrap = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(14.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();
    bind_display(commands, wrap, |w| u(w).map(|s| s.is_asset()).unwrap_or(false));
    let head = commands
        .spawn((Text::new("CREDIT / ATTRIBUTION"), ui_font(&fonts.ui, 9.5), TextColor(rgb(text_muted()))))
        .id();
    let note = commands
        .spawn((Text::new("If this asset is from another creator, credit them here. Credited assets are automatically free."), ui_font(&fonts.ui, 10.0), TextColor(rgb(placeholder()))))
        .id();
    let cname = text_field(commands, fonts, "Original Creator Name", false, "e.g. KayKit, Kenney", |s| s.credit_name.clone(), |s, v| s.credit_name = v);
    let curl = text_field(commands, fonts, "Creator Website / Source Link", false, "https://kaykit.itch.io", |s| s.credit_url.clone(), |s, v| s.credit_url = v);
    let free = commands
        .spawn((
            Node { padding: UiRect::all(Val::Px(9.0)), border_radius: BorderRadius::all(Val::Px(6.0)), display: Display::None, ..default() },
            BackgroundColor(tint((52, 180, 96), 22)),
        ))
        .id();
    bind_display(commands, free, |w| u(w).map(|s| !s.credit_name.trim().is_empty()).unwrap_or(false));
    let free_txt = commands
        .spawn((Text::new("This asset will be published as free because it credits another creator."), ui_font(&fonts.ui, 10.0), TextColor(rgb((52, 180, 96))), FocusPolicy::Pass))
        .id();
    commands.entity(free).add_child(free_txt);
    commands.entity(wrap).add_children(&[head, note, cname, curl, free]);
    wrap
}

// ── Step 4 — details ────────────────────────────────────────────────────────────

fn build_step4(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let card = step_card(commands, 4);
    let sec = section(commands);
    let head = heading(commands, fonts, "sliders-horizontal", "Additional Details");
    commands.entity(sec).add_child(head);

    // Asset-wide.
    let asset_grp = group(commands, |w| u(w).map(|s| s.is_asset()).unwrap_or(false));
    let ai = check_row(commands, fonts, "This asset was created with AI assistance", |s| s.ai_generated, |s, v| s.ai_generated = v);
    let ev = text_field(commands, fonts, "Supported Engine Versions", false, "r1-alpha7+", |s| s.engine_versions.clone(), |s, v| s.engine_versions = v);
    let lic = dropdown_field(commands, fonts, "License", LICENSES, |s| s.license, |s, v| s.license = v);
    commands.entity(asset_grp).add_children(&[ai, ev, lic]);

    // Audio (sfx, music).
    let audio_grp = group(commands, |w| cat_in(w, &["sfx", "music"]));
    let audio_head = sub_head(commands, fonts, "Audio Details");
    let bpm = text_field(commands, fonts, "BPM", false, "120", |s| s.bpm.clone(), |s, v| s.bpm = v);
    let genre = dropdown_field(commands, fonts, "Genre", GENRES, |s| s.genre, |s, v| s.genre = v);
    let loopable = check_row(commands, fonts, "Loop-friendly (seamless loop)", |s| s.loopable, |s, v| s.loopable = v);
    commands.entity(audio_grp).add_children(&[audio_head, bpm, genre, loopable]);

    // Script (scripts, plugins, blueprints).
    let script_grp = group(commands, |w| cat_in(w, &["scripts", "plugins", "blueprints"]));
    let script_head = sub_head(commands, fonts, "Script Details");
    let lang = dropdown_field(commands, fonts, "Scripting Language", SCRIPT_LANGS, |s| s.script_lang, |s, v| s.script_lang = v);
    let deps = text_field(commands, fonts, "Dependencies", false, "e.g. physics-plugin, networking-core", |s| s.dependencies.clone(), |s, v| s.dependencies = v);
    commands.entity(script_grp).add_children(&[script_head, lang, deps]);

    // 3D (3d-models, animations).
    let d3_grp = group(commands, |w| cat_in(w, &["3d-models", "animations"]));
    let d3_head = sub_head(commands, fonts, "3D Details");
    let poly = text_field(commands, fonts, "Polygon Count", false, "e.g. 12,500 tris", |s| s.polycount.clone(), |s, v| s.polycount = v);
    let texres = dropdown_field(commands, fonts, "Texture Resolution", TEX_RES, |s| s.texres, |s, v| s.texres = v);
    commands.entity(d3_grp).add_children(&[d3_head, poly, texres]);

    // 2D (2d-art, textures, particles).
    let d2_grp = group(commands, |w| cat_in(w, &["2d-art", "textures", "particles"]));
    let d2_head = sub_head(commands, fonts, "2D / Texture Details");
    let res = text_field(commands, fonts, "Resolution", false, "e.g. 1024x1024", |s| s.resolution.clone(), |s, v| s.resolution = v);
    let tileable = check_row(commands, fonts, "Seamlessly tileable", |s| s.tileable, |s, v| s.tileable = v);
    commands.entity(d2_grp).add_children(&[d2_head, res, tileable]);

    // Materials.
    let mat_grp = group(commands, |w| cat_in(w, &["materials"]));
    let mat_head = sub_head(commands, fonts, "Material Details");
    let pipe = dropdown_field(commands, fonts, "Render Pipeline", PIPELINES, |s| s.pipeline, |s, v| s.pipeline = v);
    let mtex = dropdown_field(commands, fonts, "Texture Resolution", TEX_RES, |s| s.mat_texres, |s, v| s.mat_texres = v);
    commands.entity(mat_grp).add_children(&[mat_head, pipe, mtex]);

    // Game.
    let game_grp = group(commands, |w| u(w).map(|s| s.content_type == Some(ContentType::Game)).unwrap_or(false));
    let plat_lbl = field_label(commands, fonts, "Platforms", false);
    let plats = commands.spawn(Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(14.0), row_gap: Val::Px(6.0), ..default() }).id();
    let pw = check_row(commands, fonts, "Windows", |s| s.plat_windows, |s, v| s.plat_windows = v);
    let pm = check_row(commands, fonts, "macOS", |s| s.plat_mac, |s, v| s.plat_mac = v);
    let pl = check_row(commands, fonts, "Linux", |s| s.plat_linux, |s, v| s.plat_linux = v);
    let pweb = check_row(commands, fonts, "Web", |s| s.plat_web, |s, v| s.plat_web = v);
    commands.entity(plats).add_children(&[pw, pm, pl, pweb]);
    let sysreq = text_field(commands, fonts, "Minimum System Requirements", false, "OS: Windows 10+, RAM: 4GB, GPU: OpenGL 3.3+", |s| s.sysreq.clone(), |s, v| s.sysreq = v);
    commands.entity(game_grp).add_children(&[plat_lbl, plats, sysreq]);

    // Empty state — shown only if nothing above is visible.
    let empty_note = commands
        .spawn((Text::new("No additional details needed for this category."), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), Node { display: Display::None, ..default() }))
        .id();
    bind_display(commands, empty_note, |w| !any_step4_visible(w));

    commands.entity(sec).add_children(&[asset_grp, audio_grp, script_grp, d3_grp, d2_grp, mat_grp, game_grp, empty_note]);
    let nav = nav_row(commands, fonts, true, NavNext::Continue);
    commands.entity(card).add_children(&[sec, nav]);
    card
}

/// A step-4 detail group, gated on a category/content-type predicate.
fn group(commands: &mut Commands, pred: impl Fn(&World) -> bool + Send + Sync + 'static) -> Entity {
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

fn cat_in(w: &World, list: &[&str]) -> bool {
    u(w).map(|s| s.is_asset() && list.contains(&s.category.as_str())).unwrap_or(false)
}

fn any_step4_visible(w: &World) -> bool {
    let Some(s) = u(w) else { return true; };
    match s.content_type {
        Some(ContentType::Asset) => true, // asset-wide group is always shown
        Some(ContentType::Game) => true,  // game group is always shown
        None => false,
    }
}

// ── Step 5 — files & media ──────────────────────────────────────────────────────

fn build_step5(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let card = step_card(commands, 5);

    let files_sec = section(commands);
    let files_head = heading(commands, fonts, "file-arrow-up", "Files");
    let main_lbl = field_label(commands, fonts, "File", true);
    let main_btn = file_pick_button(commands, fonts, PickMainBtn, "Choose a file to upload", |w| {
        u(w).and_then(|s| s.file.as_ref().map(|f| format!("{}  ({:.1} MB)", f.name, f.size as f64 / 1_048_576.0)))
    });
    let main_hint = help_text(commands, fonts, "ZIP, model, script, image, or audio — Max 50 MB");
    let thumb_lbl = field_label(commands, fonts, "Cover Image", false);
    let thumb_btn = file_pick_button(commands, fonts, PickThumbBtn, "Choose a cover image (1280×720)", |w| {
        u(w).and_then(|s| s.thumbnail.as_ref().map(|f| f.name.clone()))
    });
    commands.entity(files_sec).add_children(&[files_head, main_lbl, main_btn, main_hint, thumb_lbl, thumb_btn]);

    let media_sec = section(commands);
    let media_head = heading(commands, fonts, "images", "Screenshots & Media");
    let shots_hint = commands
        .spawn((Text::new("Add up to 10 screenshots. These appear in the gallery."), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted()))))
        .id();
    let shots_btn = file_pick_button(commands, fonts, PickShotsBtn, "Choose screenshots…", |w| {
        u(w).map(|s| s.screenshots.len()).filter(|n| *n > 0).map(|n| format!("{n} screenshot{} selected", if n == 1 { "" } else { "s" }))
    });
    commands.entity(media_sec).add_children(&[media_head, shots_hint, shots_btn]);

    // Asset-only: video url + audio previews.
    let extras = commands.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(14.0), display: Display::None, ..default() }).id();
    bind_display(commands, extras, |w| u(w).map(|s| s.is_asset()).unwrap_or(false));
    let video = text_field(commands, fonts, "Video Preview URL (optional)", false, "https://www.youtube.com/watch?v=… or .mp4 link", |s| s.video_url.clone(), |s, v| s.video_url = v);
    let audio_lbl = field_label(commands, fonts, "Audio Previews (optional)", false);
    let audio_btn = file_pick_button(commands, fonts, PickAudioBtn, "Choose audio previews…", |w| {
        u(w).map(|s| s.audio.len()).filter(|n| *n > 0).map(|n| format!("{n} audio file{} selected", if n == 1 { "" } else { "s" }))
    });
    commands.entity(extras).add_children(&[video, audio_lbl, audio_btn]);
    commands.entity(media_sec).add_child(extras);

    let nav = nav_row(commands, fonts, true, NavNext::Continue);
    commands.entity(card).add_children(&[files_sec, media_sec, nav]);
    card
}

/// A dashed file-picker button whose label shows the current selection.
fn file_pick_button<M: Component>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    marker: M,
    empty_label: &str,
    picked: impl Fn(&World) -> Option<String> + Send + Sync + 'static,
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

// ── Step 6 — review ─────────────────────────────────────────────────────────────

fn build_step6(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let card = step_card(commands, 6);
    let sec = section(commands);
    let head = heading(commands, fonts, "check-circle", "Review & Publish");
    let sub = commands
        .spawn((Text::new("Review your submission before uploading."), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(sec).add_children(&[head, sub]);

    // Review rows, each bound to a computed value; hidden when empty.
    review_row(commands, sec, fonts, "Type", |s| Some(s.content_type.map(|c| c.label().to_string()).unwrap_or_default()));
    review_row(commands, sec, fonts, "Category", |s| Some(if s.category_name.is_empty() { s.category.clone() } else { s.category_name.clone() }));
    review_row(commands, sec, fonts, "Name", |s| Some(s.name.clone()));
    review_row(commands, sec, fonts, "Version", |s| Some(s.version.clone()));
    review_row(commands, sec, fonts, "Price", |s| {
        Some(if !s.credit_name.trim().is_empty() {
            "Free (credited asset)".to_string()
        } else if s.price_credits() == 0 {
            "Free".to_string()
        } else {
            format!("{} credits (${:.2})", s.price_credits(), s.price_credits() as f64 * 0.10)
        })
    });
    review_row(commands, sec, fonts, "Tags", |s| {
        if s.is_asset() && !s.tags.is_empty() { Some(s.tags.join(", ")) } else { None }
    });
    review_row(commands, sec, fonts, "Credit", |s| {
        if s.is_asset() && !s.credit_name.trim().is_empty() { Some(s.credit_name.clone()) } else { None }
    });
    review_row(commands, sec, fonts, "File", |s| {
        s.file.as_ref().map(|f| format!("{} ({:.1} MB)", f.name, f.size as f64 / 1_048_576.0))
    });
    review_row(commands, sec, fonts, "Cover Image", |s| s.thumbnail.as_ref().map(|f| f.name.clone()));
    review_row(commands, sec, fonts, "Screenshots", |s| {
        (!s.screenshots.is_empty()).then(|| format!("{} image{}", s.screenshots.len(), if s.screenshots.len() == 1 { "" } else { "s" }))
    });

    let guidelines = commands
        .spawn((Text::new("By publishing, you agree to the Renzora content guidelines."), ui_font(&fonts.ui, 9.5), TextColor(rgb(placeholder())), Node { margin: UiRect::top(Val::Px(6.0)), ..default() }))
        .id();
    commands.entity(sec).add_child(guidelines);

    let nav = nav_row(commands, fonts, true, NavNext::Publish);
    commands.entity(card).add_children(&[sec, nav]);
    card
}

fn review_row(
    commands: &mut Commands,
    parent: Entity,
    fonts: &EmberFonts,
    label: &str,
    get: impl Fn(&Uploader) -> Option<String> + Send + Sync + 'static,
) {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                padding: UiRect::vertical(Val::Px(6.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(rgb(border())),
        ))
        .id();
    let getter = std::sync::Arc::new(get);
    let g1 = getter.clone();
    bind_display(commands, row, move |w| u(w).and_then(|s| g1(s)).map(|v| !v.trim().is_empty()).unwrap_or(false));
    let l = commands
        .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_muted())), FocusPolicy::Pass))
        .id();
    let v = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_primary())), FocusPolicy::Pass))
        .id();
    let g2 = getter.clone();
    bind_text(commands, v, move |w| u(w).and_then(|s| g2(s)).unwrap_or_default());
    commands.entity(row).add_children(&[l, v]);
    commands.entity(parent).add_child(row);
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
            PickMsg::Main(f) => {
                if state.download_filename.trim().is_empty() {
                    state.download_filename = f.name.clone();
                }
                state.file = Some(f);
            }
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
                    let base = renzora_auth::client::api_base();
                    let path = if state.is_asset() { "marketplace/asset" } else { "games" };
                    state.success = Some(format!("{} published!", state.content_type.map(|c| c.label()).unwrap_or("Item")));
                    state.success_url = Some(format!("{base}/{path}/{}", item.slug));
                    state.error = None;
                }
                Err(e) => state.error = Some(e),
            }
        }
    }
}

/// Step 1 — content type selection.
fn ct_click(
    asset: Query<&Interaction, (With<CtAssetBtn>, Changed<Interaction>)>,
    game: Query<&Interaction, (With<CtGameBtn>, Changed<Interaction>)>,
    mut state: ResMut<Uploader>,
) {
    if asset.iter().any(|i| *i == Interaction::Pressed) {
        select_content_type(&mut state, ContentType::Asset);
    }
    if game.iter().any(|i| *i == Interaction::Pressed) {
        select_content_type(&mut state, ContentType::Game);
    }
}

fn select_content_type(state: &mut Uploader, ct: ContentType) {
    state.content_type = Some(ct);
    state.error = None;
    // Fetch categories for this content type.
    state.categories.clear();
    state.cats_loading = true;
    let (tx, rx) = unbounded();
    state.cats_rx = Some(rx);
    std::thread::spawn(move || {
        let res = match ct {
            ContentType::Asset => renzora_auth::marketplace::list_categories(),
            ContentType::Game => publish::list_game_categories(),
        };
        let _ = tx.send(res);
    });
    state.step = 2;
}

/// Step 2 — category selection.
fn cat_click(q: Query<(&Interaction, &CatBtn), Changed<Interaction>>, mut state: ResMut<Uploader>) {
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            state.category = btn.slug.clone();
            state.category_name = btn.name.clone();
            state.error = None;
            state.step = 3;
            break;
        }
    }
}

/// Back / Continue / Publish.
fn nav_click(
    back: Query<&Interaction, (With<BackBtn>, Changed<Interaction>)>,
    next: Query<&Interaction, (With<NextBtn>, Changed<Interaction>)>,
    publish_q: Query<&Interaction, (With<PublishBtn>, Changed<Interaction>)>,
    session: Option<Res<AuthSession>>,
    mut state: ResMut<Uploader>,
) {
    if back.iter().any(|i| *i == Interaction::Pressed) {
        state.error = None;
        state.step = state.step.saturating_sub(1).max(1);
        return;
    }
    if next.iter().any(|i| *i == Interaction::Pressed) {
        if let Some(err) = validate_step(&state) {
            state.error = Some(err);
        } else {
            state.error = None;
            state.step = (state.step + 1).min(6);
        }
        return;
    }
    if publish_q.iter().any(|i| *i == Interaction::Pressed) {
        start_publish(&mut state, session.as_deref());
    }
}

fn validate_step(state: &Uploader) -> Option<String> {
    if state.step == 3 {
        if state.name.trim().is_empty() {
            return Some("Name is required.".to_string());
        }
        if state.description.trim().is_empty() {
            return Some("Description is required.".to_string());
        }
    } else if state.step == 5 && state.file.is_none() {
        return Some("Please choose a file to upload.".to_string());
    }
    None
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
    if !state.is_asset() {
        return;
    }
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
            crate::native_store::open_url(url);
        }
    }
}

/// Kick off the publish: read files + upload + attach media, all on one worker
/// thread. Enforces the "credited asset → free" rule the website does.
fn start_publish(state: &mut Uploader, session: Option<&AuthSession>) {
    if state.submitting {
        return;
    }
    let Some(file) = state.file.clone() else {
        state.error = Some("Please choose a file to upload.".to_string());
        return;
    };
    let Some(session) = session.filter(|s| s.is_signed_in()) else {
        state.error = Some("Please sign in first.".to_string());
        return;
    };
    let session = clone_session(session);
    let is_asset = state.is_asset();

    // Build metadata exactly as the web wizard does.
    let credit_name = state.credit_name.trim().to_string();
    let credit_url = state.credit_url.trim().to_string();
    let price = if is_asset && !credit_name.is_empty() { 0 } else { state.price_credits() };
    let meta = PublishMeta {
        name: state.name.trim().to_string(),
        description: state.description.trim().to_string(),
        category: state.category.clone(),
        price_credits: price,
        version: {
            let v = state.version.trim();
            if v.is_empty() { "1.0.0".to_string() } else { v.to_string() }
        },
        tags: is_asset.then(|| state.tags.clone()),
        download_filename: is_asset.then(|| state.download_filename.trim().to_string()),
        credit_name: (is_asset && !credit_name.is_empty()).then_some(credit_name),
        credit_url: (is_asset && !credit_url.is_empty()).then_some(credit_url),
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
            let item = if is_asset {
                publish::upload_asset(&session, &meta, &main, thumb_up.as_ref())?
            } else {
                publish::upload_game(&session, &meta, &main, thumb_up.as_ref())?
            };
            // Attach media (best-effort — failures don't fail the publish).
            for (i, shot) in screenshots.iter().enumerate() {
                if let Ok(f) = read_upload_file(shot) {
                    if is_asset {
                        let _ = publish::add_asset_media(&session, &item.id, &MediaUpload::Image(f));
                    } else {
                        let _ = publish::add_game_media(&session, &item.id, i, &f);
                    }
                }
            }
            if is_asset {
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
