//! The splash dashboard's **Plugins** page: browse the marketplace's plugin
//! listings and install one before a project is ever opened.
//!
//! # Why this page is here and not in the store
//!
//! A plugin does not install into a project. It extracts into the engine's own
//! `plugins/` directory, where `prebuild` compiles it and `NativePluginLoader`
//! loads it — both at process startup (see [`crate::install::engine_plugins_dir`]).
//! So installing one from inside the editor always ends the same way: a notice
//! saying it will be there next time you start. The splash is the one place in
//! the app where "next time you start" is a sentence away rather than a session
//! away, which makes it the right place to fit the engine out, not a convenience
//! copy of the store.
//!
//! It is deliberately not the store in miniature. There are no categories, no
//! filters, no pager and no item overlay: one category, one search, and the
//! single action a listing on this page can have. Anything richer belongs in the
//! Marketplace, which is a click away once a project is open.
//!
//! # Why this lives in this crate rather than in `renzora_splash`
//!
//! The catalogue client, the session and the plugin installer are all here, and
//! `renzora_splash` is a dependency of the *runtime* — a splash that reached for
//! them would put `rfd`, the import pipeline and the audio decoder in the
//! shipped game binary. So the page is registered from this side, through the
//! registry `renzora_splash` exposes for exactly this. See
//! `renzora_splash::launcher::sections`.

use std::collections::HashSet;

use bevy::prelude::*;
use bevy::ui::widget::NodeImageMode;
use bevy::ui::FocusPolicy;
use crossbeam_channel::{unbounded, Receiver};

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_display, bind_with, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::rgb;
use renzora_ember::widgets::{scroll_view, text_input, EmberTextInput};
use renzora_splash::{register_splash_section, SplashSection};

use crate::auth::marketplace::{AssetSummary, MarketplaceListResponse};
use crate::auth::session::AuthSession;
use crate::installed::{self, InstalledPlugin, UpdateState};
use crate::thumbs::HubThumbs;
use crate::util::{hash64, session_clone, signed_in};

/// The category slug the marketplace files plugins under. The server has
/// accepted both spellings historically — `install::is_plugin_category` is the
/// authority on reading one back — but a *query* has to pick one, and this is
/// the one the store's own sidebar sends.
const PLUGIN_CATEGORY: &str = "plugins";

/// The creator whose plugins the page shows **before you search**.
///
/// The default listing is a shelf, not a catalogue: it is what the launcher
/// offers unprompted, and offering an unprompted stranger's code — compiled
/// against the staged SDK and loaded into the editor process at startup — is a
/// different thing from answering a question the user asked. So the browse view
/// is first-party, and a search reaches the whole catalogue: typing a name is
/// asking for it by name.
///
/// Filtered client-side because the list endpoint takes no creator parameter,
/// which makes it a filter over one page of `popular` rather than a query. If
/// the first-party plugins ever fall off page one, this wants a server-side
/// `creator=` filter and not a bigger page.
const FIRST_PARTY: &str = "renzora";

// ── Palette ──────────────────────────────────────────────────────────────────
//
// The dashboard is not themed: it draws before a project (and therefore before a
// theme) is loaded, so it carries its own fixed palette. These match
// `renzora_splash::launcher::style` — keep them in step by eye; there is nothing
// to import, because the splash's palette is private to the crate that draws the
// rest of the window.

const TEXT: (u8, u8, u8) = (224, 228, 240);
const TEXT_MUTED: (u8, u8, u8) = (150, 158, 178);
const ACCENT: (u8, u8, u8) = (110, 150, 255);
const GREEN: (u8, u8, u8) = (74, 200, 130);
const GOLD: (u8, u8, u8) = (235, 195, 80);
const RED: (u8, u8, u8) = (239, 68, 68);

fn card_bg() -> Color {
    Color::srgba_u8(16, 18, 28, 220)
}
fn border_soft() -> Color {
    Color::srgb_u8(36, 40, 56)
}

// ── State ────────────────────────────────────────────────────────────────────

/// What a listing's one button does right now.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Action {
    Install,
    Update,
    /// Installed, at the version the marketplace publishes.
    UpToDate,
    /// Installed, and the newer version needs an engine this build is behind.
    NeedsNewerEngine,
    /// Paid, and there is no session to buy or download it with.
    SignIn,
    Busy,
}

#[derive(Resource, Default)]
struct PluginStore {
    search: String,
    assets: Vec<AssetSummary>,
    loading: bool,
    error: Option<String>,
    initialized: bool,
    /// Set by the search field; consumed by [`refetch`].
    dirty: bool,
    rx: Option<Receiver<Result<MarketplaceListResponse, String>>>,
    /// Whether the request now in flight carried a search term.
    ///
    /// `search` is not a substitute: it changes on every keystroke, so reading
    /// it when the response lands would decide "browse or search?" from what the
    /// user has typed *since*, and clearing the box mid-request would filter a
    /// set of search results down to the first-party ones. Only one request is
    /// ever outstanding — starting another replaces `rx` and drops the old one —
    /// so a single flag set at fetch time describes whatever arrives.
    pending_is_search: bool,
    /// Marketplace-installed plugins beside the editor, rescanned after every
    /// install so a card's button is right without relaunching.
    installed: Vec<InstalledPlugin>,
    /// Asset ids with an install in flight.
    installing: HashSet<String>,
    jobs: Vec<Job>,
    /// The most recent install's outcome, shown as a banner under the toolbar.
    notice: Option<Result<String, String>>,
    /// Something was installed this session, so the page offers the restart that
    /// actually loads it.
    needs_restart: bool,
}

struct Job {
    asset_id: String,
    rx: Receiver<Result<String, String>>,
}

impl PluginStore {
    /// What `asset`'s button should say.
    fn action_for(&self, asset: &AssetSummary, signed_in: bool) -> Action {
        if self.installing.contains(&asset.id) {
            return Action::Busy;
        }
        if let Some(existing) = self.installed.iter().find(|p| p.asset_id == asset.id) {
            // The listing is what the marketplace publishes *now*, so it stands
            // in for the update check's `latest_version` — this page is looking
            // at the catalogue, not at a cached copy of it.
            return match installed::update_state(
                &existing.version,
                true,
                &asset.version,
                "",
                renzora::version::ENGINE_VERSION,
            ) {
                UpdateState::UpToDate | UpdateState::Unavailable => Action::UpToDate,
                UpdateState::Available { .. } => Action::Update,
                UpdateState::NeedsNewerEngine { .. } => Action::NeedsNewerEngine,
            };
        }
        // A free plugin downloads through the public preview proxy, so it needs
        // no session. A paid one needs the authenticated endpoint, which also
        // enforces ownership — so "signed in" is the most this page can check,
        // and a listing the user has not bought fails with the server's own
        // message rather than a guess made here.
        if asset.price_credits > 0 && !signed_in {
            Action::SignIn
        } else {
            Action::Install
        }
    }
}

// ── Markers ──────────────────────────────────────────────────────────────────

#[derive(Component)]
struct PluginSearch;
#[derive(Component)]
struct PluginInstallBtn(AssetSummary);
#[derive(Component)]
struct PluginSignInBtn;
#[derive(Component)]
struct PluginRefreshBtn;
#[derive(Component)]
struct PluginRestartBtn;

// ── Registration ─────────────────────────────────────────────────────────────

pub(crate) fn register(app: &mut App) {
    app.init_resource::<PluginStore>();
    register_splash_section(
        app,
        SplashSection::new("plugins", "puzzle-piece", "Plugins", 40, build),
    );
    app.add_systems(
        Update,
        (
            init_fetch,
            poll_list,
            refetch,
            search_sync,
            search_enter,
            request_thumbs,
            install_click,
            sign_in_click,
            refresh_click,
            restart_click,
            poll_installs,
        )
            // Splash only. An install started here keeps writing on its worker
            // thread whatever the app does next — the files land either way; it
            // is only the notice that is dropped if the user opens a project
            // mid-install, and there is nowhere left to show it by then.
            .run_if(in_state(renzora::SplashState::Splash)),
    );
}

// ── Page ─────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let page = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                // Fills the host by growing into it, not by asking for 100% of
                // it — a percentage height here resolves against a row that has
                // not been sized yet, and a long list then grows the window's
                // whole column instead of scrolling inside it. See
                // `renzora_splash::launcher::sections::build_page_host`.
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(14.0),
                padding: UiRect::all(Val::Px(22.0)),
                ..default()
            },
            FocusPolicy::Pass,
            Name::new("splash-page-plugins"),
        ))
        .id();

    let header = header_block(
        commands,
        fonts,
        "Plugins",
        "Official plugins, installed before you open a project so they load at startup. Search reaches the whole catalogue.",
    );

    // Toolbar: search + refresh.
    let toolbar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let search = build_search(commands, fonts);
    let refresh = icon_pill(commands, fonts, "arrows-clockwise", "Refresh");
    commands.entity(refresh).insert(PluginRefreshBtn);
    commands.entity(toolbar).add_children(&[search, refresh]);

    // Signed-out prompt. Free plugins install without an account, so this asks
    // rather than blocks — it is only paid listings that need the session.
    let banner = sign_in_banner(commands, fonts);
    bind_display(commands, banner, |w| !signed_in(w));

    let notice = notice_banner(commands, fonts);
    bind_display(commands, notice, |w| {
        w.get_resource::<PluginStore>().is_some_and(|s| s.notice.is_some())
    });

    let restart = restart_banner(commands, fonts);
    bind_display(commands, restart, |w| {
        w.get_resource::<PluginStore>().is_some_and(|s| s.needs_restart)
    });

    let list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(9.0),
                padding: UiRect::right(Val::Px(6.0)),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    keyed_list(commands, list, listings_snapshot);
    // `scroll_view` returns a viewport that already grows to fill the column and
    // clips; replacing its `Node` would strip the clip and the relative
    // positioning its scrollbar track depends on.
    let scroll = scroll_view(commands, list);

    commands
        .entity(page)
        .add_children(&[header, toolbar, banner, notice, restart, scroll]);
    page
}

fn header_block(
    commands: &mut Commands,
    fonts: &EmberFonts,
    title: &str,
    subtitle: &str,
) -> Entity {
    let col = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let h = text_node(commands, fonts, title, 17.0, TEXT);
    let s = text_node(commands, fonts, subtitle, 11.5, TEXT_MUTED);
    commands.entity(col).add_children(&[h, s]);
    col
}

fn build_search(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                max_width: Val::Px(340.0),
                height: Val::Px(34.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::horizontal(Val::Px(11.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba_u8(10, 12, 20, 225)),
            BorderColor::all(border_soft()),
            FocusPolicy::Block,
        ))
        .id();
    let mag = icon_text(commands, &fonts.phosphor, "magnifying-glass", TEXT_MUTED, 14.0);
    commands.entity(mag).insert(FocusPolicy::Pass);
    let input = text_input(commands, &fonts.ui, "Search plugins…", "");
    commands.entity(input).insert((
        Node {
            flex_grow: 1.0,
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::NONE),
        BorderColor::all(Color::NONE),
        PluginSearch,
    ));
    commands.entity(row).add_children(&[mag, input]);
    row
}

// ── Banners ──────────────────────────────────────────────────────────────────

fn banner_row(commands: &mut Commands, tint: (u8, u8, u8)) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(9.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba_u8(tint.0, tint.1, tint.2, 26)),
            BorderColor::all(Color::srgba_u8(tint.0, tint.1, tint.2, 90)),
            FocusPolicy::Block,
        ))
        .id()
}

fn sign_in_banner(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = banner_row(commands, ACCENT);
    let icon = icon_text(commands, &fonts.phosphor, "user-circle", ACCENT, 17.0);
    commands.entity(icon).insert(FocusPolicy::Pass);
    let msg = commands
        .spawn((
            Text::new(
                "Free plugins install without an account. Sign in to install ones you have bought."
                    .to_string(),
            ),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(TEXT)),
            Node { flex_grow: 1.0, ..default() },
            FocusPolicy::Pass,
        ))
        .id();
    let btn = pill(commands, fonts, "Sign In", ACCENT, (255, 255, 255));
    commands.entity(btn).insert(PluginSignInBtn);
    commands.entity(row).add_children(&[icon, msg, btn]);
    row
}

fn notice_banner(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = banner_row(commands, GREEN);
    // One node for both outcomes: the tint and the glyph swap with the result,
    // which is cheaper and less to go wrong than two banners racing on
    // `bind_display`.
    bind_with(
        commands,
        row,
        |w| {
            w.get_resource::<PluginStore>()
                .and_then(|s| s.notice.as_ref().map(|n| n.is_ok()))
                .unwrap_or(true)
        },
        |w, e, ok: &bool| {
            let tint = if *ok { GREEN } else { RED };
            if let Some(mut bg) = w.get_mut::<BackgroundColor>(e) {
                bg.0 = Color::srgba_u8(tint.0, tint.1, tint.2, 26);
            }
            if let Some(mut b) = w.get_mut::<BorderColor>(e) {
                *b = BorderColor::all(Color::srgba_u8(tint.0, tint.1, tint.2, 90));
            }
        },
    );
    let icon = icon_text(commands, &fonts.phosphor, "check-circle", GREEN, 17.0);
    commands.entity(icon).insert(FocusPolicy::Pass);
    let msg = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(TEXT)),
            Node { flex_grow: 1.0, ..default() },
            FocusPolicy::Pass,
        ))
        .id();
    renzora_ember::reactive::tracked::bind_text(commands, msg, |w| {
        w.get_resource::<PluginStore>()
            .and_then(|s| s.notice.clone())
            .map(|n| match n {
                Ok(m) => m,
                Err(e) => e,
            })
            .unwrap_or_default()
    });
    commands.entity(row).add_children(&[icon, msg]);
    row
}

fn restart_banner(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = banner_row(commands, GOLD);
    let icon = icon_text(commands, &fonts.phosphor, "arrow-clockwise", GOLD, 17.0);
    commands.entity(icon).insert(FocusPolicy::Pass);
    let msg = commands
        .spawn((
            Text::new(
                "A plugin is staged. It is compiled and loaded on the next start.".to_string(),
            ),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(TEXT)),
            Node { flex_grow: 1.0, ..default() },
            FocusPolicy::Pass,
        ))
        .id();
    let btn = pill(commands, fonts, "Restart now", GOLD, (26, 22, 10));
    commands.entity(btn).insert(PluginRestartBtn);
    commands.entity(row).add_children(&[icon, msg, btn]);
    row
}

// ── Listings ─────────────────────────────────────────────────────────────────

fn listings_snapshot(world: &Rx) -> KeyedSnapshot {
    let Some(store) = world.get_resource::<PluginStore>() else {
        return note_snapshot("The marketplace is unavailable in this build.");
    };
    if store.loading && store.assets.is_empty() {
        return note_snapshot("Loading plugins…");
    }
    if let Some(err) = store.error.clone() {
        return note_snapshot(&err);
    }
    if store.assets.is_empty() {
        return note_snapshot(if store.pending_is_search {
            "No plugins match that search."
        } else {
            "No official plugins published yet. Search to browse the whole catalogue."
        });
    }

    let signed = signed_in(world);
    let rows: Vec<(AssetSummary, Action)> = store
        .assets
        .iter()
        .map(|a| (a.clone(), store.action_for(a, signed)))
        .collect();
    let items: Vec<(u64, u64)> = rows
        .iter()
        .map(|(a, action)| {
            // The action is part of the content hash, not just the key: a card
            // whose button went from Install to Installed has to be rebuilt, and
            // nothing about the listing itself changed to say so.
            (hash64(&a.id), hash64(&(&a.name, &a.version, a.price_credits, *action)))
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| listing_card(c, f, &rows[i].0, rows[i].1)),
    }
}

fn note_snapshot(message: &str) -> KeyedSnapshot {
    let msg = message.to_string();
    let key = hash64(&msg);
    KeyedSnapshot {
        items: vec![(key, key)],
        build: Box::new(move |c, f, _| {
            c.spawn((
                Text::new(msg.clone()),
                ui_font(&f.ui, 12.0),
                TextColor(rgb(TEXT_MUTED)),
                Node { margin: UiRect::top(Val::Px(6.0)), ..default() },
                FocusPolicy::Pass,
            ))
            .id()
        }),
    }
}

fn listing_card(
    commands: &mut Commands,
    fonts: &EmberFonts,
    asset: &AssetSummary,
    action: Action,
) -> Entity {
    let card = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(13.0),
                padding: UiRect::all(Val::Px(11.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(card_bg()),
            BorderColor::all(border_soft()),
            FocusPolicy::Block,
            Name::new("splash-plugin-card"),
        ))
        .id();

    let thumb = build_thumb(commands, fonts, asset);

    let info = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();

    let title_row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(7.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let name = text_node(commands, fonts, &elide(&asset.name, 46), 13.0, TEXT);
    let version = chip(commands, fonts, &format!("v{}", asset.version), ACCENT);
    let mut title_kids = vec![name, version];
    if matches!(action, Action::UpToDate | Action::Update | Action::NeedsNewerEngine) {
        title_kids.push(chip(commands, fonts, "Installed", GREEN));
    }
    commands.entity(title_row).add_children(&title_kids);

    let by = text_node(
        commands,
        fonts,
        &format!("by {}  ·  {} downloads", asset.creator_name, asset.downloads),
        10.0,
        TEXT_MUTED,
    );
    let desc = text_node(
        commands,
        fonts,
        &elide(first_line(&asset.description), 110),
        11.0,
        TEXT_MUTED,
    );
    commands.entity(info).add_children(&[title_row, by, desc]);

    let right = commands
        .spawn((
            Node {
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: Val::Px(6.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let price = if asset.price_credits == 0 {
        text_node(commands, fonts, "Free", 11.0, GREEN)
    } else {
        text_node(commands, fonts, &format!("{} credits", asset.price_credits), 11.0, GOLD)
    };
    let button = action_button(commands, fonts, asset, action);
    commands.entity(right).add_children(&[price, button]);

    commands.entity(card).add_children(&[thumb, info, right]);
    card
}

/// The card's one action. An already-installed listing still gets a button-shaped
/// node rather than a bare label, so the column does not jump about between rows.
fn action_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    asset: &AssetSummary,
    action: Action,
) -> Entity {
    let (label, bg, fg) = match action {
        Action::Install => ("Install", ACCENT, (255, 255, 255)),
        Action::Update => ("Update", GREEN, (10, 24, 16)),
        Action::UpToDate => ("Installed", (40, 44, 58), TEXT_MUTED),
        Action::NeedsNewerEngine => ("Needs newer engine", (40, 44, 58), TEXT_MUTED),
        Action::SignIn => ("Sign in", (40, 44, 58), TEXT),
        Action::Busy => ("Installing…", (40, 44, 58), TEXT_MUTED),
    };
    let btn = pill(commands, fonts, label, bg, fg);
    match action {
        Action::Install | Action::Update => {
            commands.entity(btn).insert(PluginInstallBtn(asset.clone()));
        }
        Action::SignIn => {
            commands.entity(btn).insert(PluginSignInBtn);
        }
        // Installed, unavailable or busy: nothing to press. No marker, so the
        // press falls on the floor rather than starting a second install.
        Action::UpToDate | Action::NeedsNewerEngine | Action::Busy => {}
    }
    btn
}

fn build_thumb(commands: &mut Commands, fonts: &EmberFonts, asset: &AssetSummary) -> Entity {
    let frame = commands
        .spawn((
            Node {
                width: Val::Px(52.0),
                height: Val::Px(52.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(Val::Px(9.0)),
                ..default()
            },
            BackgroundColor(Color::srgba_u8(28, 32, 46, 220)),
            FocusPolicy::Pass,
        ))
        .id();

    match asset.thumbnail_url.clone() {
        Some(url) => {
            let img = commands
                .spawn((
                    ImageNode { image_mode: NodeImageMode::Stretch, ..default() },
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        display: Display::None,
                        border_radius: BorderRadius::all(Val::Px(9.0)),
                        ..default()
                    },
                    FocusPolicy::Pass,
                ))
                .id();
            bind_with(
                commands,
                img,
                move |w| w.get_resource::<HubThumbs>().and_then(|t| t.get(&url)),
                |w, e, handle: &Option<Handle<Image>>| {
                    let Some(h) = handle.clone() else { return };
                    if let Some(mut node) = w.get_mut::<ImageNode>(e) {
                        node.image = h;
                    }
                    if let Some(mut n) = w.get_mut::<Node>(e) {
                        n.display = Display::Flex;
                    }
                },
            );
            commands.entity(frame).add_child(img);
        }
        None => {
            let glyph = icon_text(commands, &fonts.phosphor, "puzzle-piece", TEXT_MUTED, 22.0);
            commands.entity(glyph).insert(FocusPolicy::Pass);
            commands.entity(frame).add_child(glyph);
        }
    }
    frame
}

fn chip(commands: &mut Commands, fonts: &EmberFonts, label: &str, tint: (u8, u8, u8)) -> Entity {
    let chip = commands
        .spawn((
            Node {
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba_u8(tint.0, tint.1, tint.2, 34)),
            FocusPolicy::Pass,
        ))
        .id();
    let t = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.mono, 9.5),
            TextColor(rgb(tint)),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(chip).add_child(t);
    chip
}

/// [`crate::util::pill_button`] with the two things every clickable node on the
/// splash needs and the store's panels do not: an explicit `FocusPolicy::Block`
/// (see the launcher's module doc — a `Pass` node hands the press to every
/// ancestor under the cursor as well) and a pointer cursor.
fn pill(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    bg: (u8, u8, u8),
    fg: (u8, u8, u8),
) -> Entity {
    let btn = crate::util::pill_button(commands, fonts, label, bg, fg);
    commands.entity(btn).insert((
        FocusPolicy::Block,
        renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
    ));
    btn
}

fn icon_pill(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str) -> Entity {
    let btn = commands
        .spawn((
            Node {
                height: Val::Px(34.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba_u8(12, 14, 22, 235)),
            BorderColor::all(border_soft()),
            Interaction::default(),
            FocusPolicy::Block,
            renzora_ember::widgets::HoverTint::solid(
                Color::srgba_u8(12, 14, 22, 235),
                Color::srgba_u8(26, 30, 46, 245),
                Color::srgba_u8(34, 40, 60, 250),
            ),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, TEXT_MUTED, 13.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let t = text_node(commands, fonts, label, 11.5, TEXT);
    commands.entity(btn).add_children(&[ic, t]);
    btn
}

fn text_node(
    commands: &mut Commands,
    fonts: &EmberFonts,
    value: &str,
    size: f32,
    color: (u8, u8, u8),
) -> Entity {
    commands
        .spawn((
            Text::new(value.to_string()),
            ui_font(&fonts.ui, size),
            TextColor(rgb(color)),
            FocusPolicy::Pass,
        ))
        .id()
}

/// The first line of a description — catalogue descriptions are markdown, and a
/// card has room for the summary a seller opens with, not the document.
fn first_line(s: &str) -> &str {
    s.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("")
}

/// Counts `char`s rather than bytes: a listing's name and description are
/// user-entered and routinely carry accents, CJK or emoji, and slicing a
/// `String` by byte index in the middle of one of those panics.
fn elide(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        let head: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{head}…")
    } else {
        s.to_string()
    }
}

// ── Fetching ─────────────────────────────────────────────────────────────────

fn init_fetch(mut store: ResMut<PluginStore>) {
    if store.initialized {
        return;
    }
    store.initialized = true;
    store.installed = scan_installed();
    fetch(&mut store);
}

fn refetch(mut store: ResMut<PluginStore>) {
    if store.dirty {
        store.dirty = false;
        fetch(&mut store);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch(store: &mut PluginStore) {
    let query = (!store.search.trim().is_empty()).then(|| store.search.trim().to_string());
    let (tx, rx) = unbounded();
    store.pending_is_search = query.is_some();
    store.rx = Some(rx);
    store.loading = true;
    store.error = None;
    std::thread::spawn(move || {
        let result = crate::auth::marketplace::list_assets(
            query.as_deref(),
            Some(PLUGIN_CATEGORY),
            Some("popular"),
            1,
            None,
            None,
        );
        let _ = tx.send(result);
    });
}

#[cfg(target_arch = "wasm32")]
fn fetch(_store: &mut PluginStore) {}

fn poll_list(mut store: ResMut<PluginStore>) {
    let mut got = Vec::new();
    if let Some(rx) = store.rx.as_ref() {
        while let Ok(r) = rx.try_recv() {
            got.push(r);
        }
    }
    for r in got {
        store.loading = false;
        match r {
            Ok(resp) => {
                // The browse shelf is first-party; a search reaches the whole
                // catalogue. See `FIRST_PARTY`.
                //
                // Filtered on the way in rather than at draw time, so nothing
                // else — the thumbnail requests especially — ever sees a listing
                // this page will not show.
                store.assets = if store.pending_is_search {
                    resp.assets
                } else {
                    resp.assets
                        .into_iter()
                        .filter(|a| a.creator_name.eq_ignore_ascii_case(FIRST_PARTY))
                        .collect()
                };
                store.error = None;
            }
            Err(e) => store.error = Some(e),
        }
    }
}

fn request_thumbs(store: Res<PluginStore>, mut thumbs: ResMut<HubThumbs>) {
    for a in &store.assets {
        if let Some(url) = &a.thumbnail_url {
            thumbs.request(url);
        }
    }
}

fn search_sync(
    inputs: Query<&EmberTextInput, With<PluginSearch>>,
    mut store: ResMut<PluginStore>,
) {
    for inp in &inputs {
        if store.search != inp.value {
            store.search = inp.value.clone();
        }
    }
}

/// Enter in the search field runs the search. `text_input` deliberately leaves
/// Enter to whoever owns the field, and this is that owner.
fn search_enter(
    keys: Res<ButtonInput<KeyCode>>,
    inputs: Query<&EmberTextInput, With<PluginSearch>>,
    mut store: ResMut<PluginStore>,
) {
    if !keys.just_pressed(KeyCode::Enter) && !keys.just_pressed(KeyCode::NumpadEnter) {
        return;
    }
    if !inputs.iter().any(|i| i.focused) {
        return;
    }
    store.dirty = true;
}

fn refresh_click(
    q: Query<&Interaction, (With<PluginRefreshBtn>, Changed<Interaction>)>,
    mut store: ResMut<PluginStore>,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        store.installed = scan_installed();
        store.dirty = true;
    }
}

fn sign_in_click(
    q: Query<&Interaction, (With<PluginSignInBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        // The same modal the editor's title bar opens; its systems are ungated,
        // so it renders over the dashboard and the session it establishes is the
        // one the editor will already be holding.
        commands.insert_resource(renzora::core::AuthToggleWindowRequest);
    }
}

fn restart_click(q: Query<&Interaction, (With<PluginRestartBtn>, Changed<Interaction>)>) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        renzora::restart_process();
    }
}

// ── Installing ───────────────────────────────────────────────────────────────

fn install_click(
    q: Query<(&Interaction, &PluginInstallBtn), Changed<Interaction>>,
    session: Res<AuthSession>,
    mut store: ResMut<PluginStore>,
) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let asset = btn.0.clone();
        if store.installing.contains(&asset.id) {
            continue;
        }
        let session = session.is_signed_in().then(|| session_clone(&session));
        store.installing.insert(asset.id.clone());
        store.notice = None;
        let (tx, rx) = unbounded();
        store.jobs.push(Job { asset_id: asset.id.clone(), rx });
        spawn_install(session, asset, tx);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_install(
    session: Option<AuthSession>,
    asset: AssetSummary,
    tx: crossbeam_channel::Sender<Result<String, String>>,
) {
    std::thread::Builder::new()
        .name("renzora-splash-plugin-install".to_string())
        .spawn(move || {
            let _ = tx.send(run_install(session.as_ref(), &asset));
        })
        .ok();
}

#[cfg(target_arch = "wasm32")]
fn spawn_install(
    _session: Option<AuthSession>,
    _asset: AssetSummary,
    tx: crossbeam_channel::Sender<Result<String, String>>,
) {
    let _ = tx.send(Err("Installing isn't supported in the browser yet".into()));
}

/// Fetch the plugin's source archive and extract it into the engine's `plugins/`
/// directory.
///
/// The same two-door download as the store's installer: the authenticated
/// endpoint when there is a session (which is also what enforces ownership), and
/// the public preview proxy for a free listing when there is not. The write half
/// is [`crate::install::install_plugin_source`] — the one implementation, so a
/// plugin installed from the splash lands exactly where one installed from the
/// store does, with the same sidecar and the same clash handling.
#[cfg(not(target_arch = "wasm32"))]
fn run_install(session: Option<&AuthSession>, asset: &AssetSummary) -> Result<String, String> {
    use crate::auth::marketplace as mk;
    use crate::install;

    let mut ignore = |_: u64| {};
    let bytes = if let Some(s) = session.filter(|s| s.is_signed_in()) {
        let dl = mk::download_asset(s, &asset.id)?;
        // A plugin is one source archive. Unlike a model it never arrives as
        // several files, so there is no zip-of-everything case to handle here.
        mk::download_file_progress(&dl.download_url, &mut ignore)?
    } else if asset.price_credits == 0 {
        mk::download_file_progress(&mk::preview_file_url(&asset.id), &mut ignore)?
    } else {
        return Err("Sign in to install this plugin".into());
    };

    let done = install::install_plugin_source(&asset.id, &bytes)?;
    // The sidecar ties the installed source back to its listing, and it is also
    // what marks this directory as marketplace-owned: `xtask`'s `prune_orphans`
    // deletes any staged plugin directory without one, since that is how it
    // recognises a leftover copy of a repo plugin. Failing the install is better
    // than leaving a plugin the next `cargo renzora` would silently delete.
    let meta = install::PluginSidecar {
        asset_id: asset.id.clone(),
        name: asset.name.clone(),
        slug: asset.slug.clone(),
        version: asset.version.clone(),
        category: asset.category.clone(),
        crate_name: done.dir_name.clone(),
        ..Default::default()
    };
    if let Err(e) = install::write_plugin_sidecar(&done.path, &meta) {
        let _ = std::fs::remove_dir_all(&done.path);
        return Err(format!("Could not finish installing '{}': {e}", done.dir_name));
    }

    let verb = if done.updated { "Updated" } else { "Installed" };
    // A rename is not a footnote: the plugin builds and loads under the new
    // name, so anything the user does with it later uses that name.
    let renamed = match &done.renamed_from {
        Some(wanted) => format!(
            " Another plugin already uses '{wanted}', so this one installed as '{}'.",
            done.dir_name
        ),
        None => String::new(),
    };
    Ok(format!(
        "{verb} \"{}\" as plugin '{}'.{renamed}",
        asset.name, done.dir_name
    ))
}

fn poll_installs(mut store: ResMut<PluginStore>) {
    let mut finished: Vec<(String, Result<String, String>)> = Vec::new();
    store.jobs.retain(|job| match job.rx.try_recv() {
        Ok(outcome) => {
            finished.push((job.asset_id.clone(), outcome));
            false
        }
        Err(_) => true,
    });
    if finished.is_empty() {
        return;
    }
    for (asset_id, outcome) in finished {
        store.installing.remove(&asset_id);
        if let Ok(msg) = &outcome {
            renzora::core::console_log::console_info("Marketplace", msg.clone());
            store.needs_restart = true;
            // Rescan rather than patch the list: `install_plugin_source` decides
            // the directory name (and may rename around a clash), so what is on
            // disk is the only thing that knows what actually happened.
            store.installed = scan_installed();
        } else if let Err(e) = &outcome {
            renzora::core::console_log::console_error("Marketplace", e.clone());
        }
        store.notice = Some(outcome);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn scan_installed() -> Vec<InstalledPlugin> {
    installed::scan()
}

#[cfg(target_arch = "wasm32")]
fn scan_installed() -> Vec<InstalledPlugin> {
    Vec::new()
}
