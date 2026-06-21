//! Bevy-native (ember) Marketplace browser: a left column (account + credit
//! balance, Upload Asset, category list), a search/sort toolbar, and a card grid
//! with per-card Get / Preview actions and pagination.
//!
//! Cards download through a permissions-style confirm overlay
//! (`install_overlay`) that lets the user choose the destination folder. Theme
//! cards additionally offer a live **Preview** that applies the downloaded theme
//! into the editor's `ThemeManager` without installing it, restorable from a
//! banner. Background list/category/preview fetches arrive over crossbeam
//! channels.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use crossbeam_channel::{unbounded, Receiver};

use renzora_auth::marketplace::{AssetSummary, MarketplaceListResponse};
use renzora_auth::session::AuthSession;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{
    bind_bg, bind_display, bind_text, bind_with, keyed_list, Bound, KeyedSnapshot,
};
use renzora_ember::theme::*;
use renzora_ember::widgets::{dropdown, text_input, EmberTextInput};
use renzora::SplashState;
use renzora_theme::ThemeManager;

use crate::thumbs::HubThumbs;

const GREEN: (u8, u8, u8) = (52, 180, 96);
const RED: (u8, u8, u8) = (224, 80, 80);
const CARD_W: f32 = 176.0;
const THUMB_H: f32 = 150.0;

const SORTS: [(&str, &str); 4] = [
    ("newest", "Newest"),
    ("popular", "Popular"),
    ("price_asc", "Price: Low"),
    ("price_desc", "Price: High"),
];

/// True for theme-category assets, which get a live "Preview" action.
fn is_theme(category: &str) -> bool {
    category.to_lowercase().contains("theme")
}

/// Background category-fetch result: `(slug, display name)` pairs, or an error.
type CategoriesFetch = Result<Vec<(String, String)>, String>;

/// One cached page of store results, keyed by its query signature so navigating
/// back to a page (or re-applying a search/sort) reuses it instead of re-hitting
/// the network.
struct CachedPage {
    assets: Vec<AssetSummary>,
    total: i64,
    per_page: i64,
}

#[derive(Resource)]
struct HubStoreData {
    search: String,
    category: Option<String>,
    sort: String,
    page: u32,
    assets: Vec<AssetSummary>,
    total: i64,
    per_page: i64,
    categories: Vec<(String, String)>,
    loading: bool,
    error: Option<String>,
    asset_rx: Option<Receiver<Result<MarketplaceListResponse, String>>>,
    cat_rx: Option<Receiver<CategoriesFetch>>,
    initialized: bool,
    dirty: bool,
    /// Fetched pages keyed by `(search, category, sort, page)` hash. Persists for
    /// the session — paging back/forward is a cache hit, not a request.
    cache: std::collections::HashMap<u64, CachedPage>,
    /// Query signature of the request currently in flight, so its response lands
    /// in the right cache slot even if the user navigated on since.
    pending_sig: Option<u64>,
}

impl Default for HubStoreData {
    fn default() -> Self {
        Self {
            search: String::new(),
            category: None,
            sort: "popular".into(),
            page: 1,
            assets: Vec::new(),
            total: 0,
            per_page: 24,
            categories: Vec::new(),
            loading: false,
            error: None,
            asset_rx: None,
            cat_rx: None,
            initialized: false,
            dirty: false,
            cache: std::collections::HashMap::new(),
            pending_sig: None,
        }
    }
}

impl HubStoreData {
    /// Hash of the inputs that determine a page's contents.
    fn query_sig(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.search.hash(&mut h);
        self.category.hash(&mut h);
        self.sort.hash(&mut h);
        self.page.hash(&mut h);
        h.finish()
    }
    fn total_pages(&self) -> u32 {
        ((self.total as f32) / (self.per_page.max(1) as f32)).ceil() as u32
    }
}

/// Live theme-preview state: a theme applied into the editor's `ThemeManager`
/// without installing it. `saved` holds what to restore when the preview stops.
#[derive(Resource, Default)]
struct ThemePreview {
    /// Display name of the asset currently being previewed (drives the banner).
    previewing: Option<String>,
    /// The asset behind the active preview, so "Install Theme" can target it.
    asset: Option<AssetSummary>,
    /// The (name, theme) to restore when the preview stops.
    saved: Option<(String, renzora_theme::Theme)>,
    /// In-flight download/parse of the theme `.toml`.
    rx: Option<Receiver<Result<(String, renzora_theme::Theme), String>>>,
    error: Option<String>,
}

pub struct NativeHubStore;

impl Plugin for NativeHubStore {
    fn build(&self, app: &mut App) {
        app.init_resource::<HubStoreData>();
        app.init_resource::<ThemePreview>();
        app.register_panel_content("hub_store", false, build);
        crate::install_overlay::register(app);
        app.add_systems(
            Update,
            (
                poll_store,
                store_init,
                store_refetch,
                store_search_sync,
                store_search_click,
                store_sort_dropdown,
                store_category_click,
                store_page_click,
                store_install_click,
                store_preview_click,
                store_signin_click,
                store_topup_click,
                store_upload_click,
                request_store_thumbs,
            )
                .run_if(in_state(SplashState::Editor)),
        );
        app.add_systems(
            Update,
            (poll_preview, store_stop_preview_click, store_preview_install_click)
                .run_if(in_state(SplashState::Editor)),
        );
    }
}

#[derive(Component)]
struct StoreSearch;
#[derive(Component)]
struct StoreSearchBtn;
#[derive(Component)]
struct StoreSortDropdown;
#[derive(Component)]
struct StoreCatRow(Option<String>);
#[derive(Component)]
struct StorePageBtn(i32);
#[derive(Component)]
struct StoreInstallBtn(AssetSummary);
#[derive(Component)]
struct StorePreviewBtn(AssetSummary);
#[derive(Component)]
struct StoreSignInBtn;
#[derive(Component)]
struct StoreTopUpBtn;
#[derive(Component)]
struct StoreUploadBtn;
#[derive(Component)]
struct StopPreviewBtn;
#[derive(Component)]
struct PreviewInstallBtn;

fn signed_in(w: &World) -> bool {
    w.get_resource::<AuthSession>().map(|s| s.is_signed_in()).unwrap_or(false)
}

// ── Build ────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(6.0)),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();

    // Toolbar: search + search button + sort dropdown + total.
    let toolbar = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), flex_shrink: 0.0, ..default() })
        .id();
    let search = text_input(commands, &fonts.ui, "Search assets...", "");
    commands.entity(search).insert((
        StoreSearch,
        Node { flex_grow: 1.0, min_width: Val::Px(0.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), align_items: AlignItems::Center, border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
    ));
    let search_btn = chip_button(commands, fonts, "magnifying-glass", None, StoreSearchBtn);
    // Sort is a proper dropdown menu (was a cycling toggle button).
    let sort_labels: Vec<&str> = SORTS.iter().map(|(_, l)| *l).collect();
    // Default selection mirrors `HubStoreData::default().sort` (Popular).
    let default_sort = SORTS.iter().position(|(v, _)| *v == "popular").unwrap_or(0);
    let sort = dropdown(commands, fonts, &sort_labels, default_sort);
    commands.entity(sort).insert(StoreSortDropdown);
    let total = commands.spawn((Text::new(""), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_muted())))).id();
    bind_text(commands, total, |w| format!("{} assets", w.resource::<HubStoreData>().total));
    commands.entity(toolbar).add_children(&[search, search_btn, sort, total]);

    // Status / error.
    let status = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(RED)), Node { flex_shrink: 0.0, ..default() })).id();
    bind_text(commands, status, |w| w.resource::<HubStoreData>().error.clone().map(|e| format!("\u{26a0} {e}")).unwrap_or_default());
    bind_display(commands, status, |w| w.resource::<HubStoreData>().error.is_some());

    // Live theme-preview banner (visible only while previewing).
    let banner = build_preview_banner(commands, fonts);

    // Split: left column (account + upload + categories) + asset grid.
    let split = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(0.0), flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), ..default() })
        .id();
    let sidebar = build_sidebar(commands, fonts);

    let right = commands
        .spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() })
        .id();
    let grid = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, align_content: AlignContent::FlexStart, align_items: AlignItems::FlexStart, column_gap: Val::Px(10.0), row_gap: Val::Px(10.0), ..default() })
        .id();
    keyed_list(commands, grid, assets_snapshot);
    let grid_scroll = renzora_ember::widgets::scroll_view(commands, grid);
    let pager = build_pager(commands, fonts);
    commands.entity(right).add_children(&[grid_scroll, pager]);

    commands.entity(split).add_children(&[sidebar, right]);
    commands.entity(root).add_children(&[toolbar, status, banner, split]);
    root
}

/// The left column: account header (signed-in identity + credit balance, or a
/// Sign In button), an Upload Asset action, then the scrollable category list.
fn build_sidebar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = commands
        .spawn(Node { width: Val::Px(160.0), flex_shrink: 0.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() })
        .id();

    // ── Account block ──
    let account = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), padding: UiRect::all(Val::Px(8.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() },
            BackgroundColor(rgb(section_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();

    // Signed-in identity + balance.
    let signed = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() }).id();
    bind_display(commands, signed, signed_in);
    let who_row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(5.0), ..default() }).id();
    let who_icon = icon_text(commands, &fonts.phosphor, "user-circle", text_muted(), 14.0);
    let who_col = commands.spawn(Node { flex_direction: FlexDirection::Column, min_width: Val::Px(0.0), ..default() }).id();
    let who_caption = commands.spawn((Text::new("Signed in as"), ui_font(&fonts.ui, 8.5), TextColor(rgb(text_muted())))).id();
    let who_name = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.5), TextColor(rgb(text_primary())), bevy::text::TextLayout::no_wrap(), Node { overflow: Overflow::clip(), ..default() })).id();
    bind_text(commands, who_name, |w| {
        w.get_resource::<AuthSession>().and_then(|s| s.user.as_ref().map(|u| u.username.clone())).unwrap_or_default()
    });
    commands.entity(who_col).add_children(&[who_caption, who_name]);
    commands.entity(who_row).add_children(&[who_icon, who_col]);
    let bal_row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(5.0), ..default() }).id();
    let bal_icon = icon_text(commands, &fonts.phosphor, "coins", (230, 200, 110), 13.0);
    let bal_text = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb((230, 200, 110))))).id();
    bind_text(commands, bal_text, |w| {
        let n = w.get_resource::<AuthSession>().and_then(|s| s.user.as_ref().map(|u| u.credit_balance)).unwrap_or(0);
        format!("{n} credits")
    });
    let bal_gap = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    // Top-up: opens the website wallet to buy more credits.
    let topup = commands
        .spawn((
            Node { width: Val::Px(20.0), height: Val::Px(20.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(4.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(rgb(accent())),
            Interaction::default(),
            StoreTopUpBtn,
            Name::new("store-topup"),
        ))
        .id();
    let topup_icon = icon_text(commands, &fonts.phosphor, "plus", (255, 255, 255), 12.0);
    commands.entity(topup_icon).insert(FocusPolicy::Pass);
    commands.entity(topup).add_child(topup_icon);
    commands.entity(bal_row).add_children(&[bal_icon, bal_text, bal_gap, topup]);
    commands.entity(signed).add_children(&[who_row, bal_row]);

    // Signed-out: a Sign In button.
    let signin = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, column_gap: Val::Px(5.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(rgb(accent())),
            Interaction::default(),
            StoreSignInBtn,
            Name::new("store-signin"),
        ))
        .id();
    bind_display(commands, signin, |w| !signed_in(w));
    let si_icon = icon_text(commands, &fonts.phosphor, "sign-in", (255, 255, 255), 13.0);
    let si_txt = commands.spawn((Text::new("Sign In"), ui_font(&fonts.ui, 11.0), TextColor(rgb((255, 255, 255))), FocusPolicy::Pass)).id();
    commands.entity(signin).add_children(&[si_icon, si_txt]);
    commands.entity(account).add_children(&[signed, signin]);

    // ── Upload Asset (placeholder action) ──
    let upload = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, column_gap: Val::Px(5.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(rgb(hover_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            StoreUploadBtn,
            Name::new("store-upload"),
        ))
        .id();
    let up_icon = icon_text(commands, &fonts.phosphor, "upload-simple", text_primary(), 13.0);
    let up_txt = commands.spawn((Text::new("Upload Asset"), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), FocusPolicy::Pass)).id();
    commands.entity(upload).add_children(&[up_icon, up_txt]);

    // ── Categories ──
    let cat_caption = commands.spawn((Text::new("Categories"), ui_font(&fonts.ui, 9.5), TextColor(rgb(text_muted())), Node { margin: UiRect::top(Val::Px(2.0)), ..default() })).id();
    // Natural-height column (sums its rows) so the scroll viewport overflows and
    // scrolls; with flex_grow the rows would squash to fit instead.
    let cats = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() })
        .id();
    keyed_list(commands, cats, categories_snapshot);
    let cats_scroll = renzora_ember::widgets::scroll_view(commands, cats);

    commands.entity(col).add_children(&[account, upload, cat_caption, cats_scroll]);
    col
}

/// The theme-preview banner — shown while a theme is being previewed live.
fn build_preview_banner(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let banner = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(rgb(accent()).with_alpha(0.16)),
            BorderColor::all(rgb(accent())),
        ))
        .id();
    bind_display(commands, banner, |w| w.resource::<ThemePreview>().previewing.is_some());
    let eye = icon_text(commands, &fonts.phosphor, "eye", accent(), 13.0);
    let label = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), Node { flex_grow: 1.0, min_width: Val::Px(0.0), ..default() }, bevy::text::TextLayout::no_wrap())).id();
    bind_text(commands, label, |w| w.resource::<ThemePreview>().previewing.clone().map(|n| format!("Previewing theme: {n}")).unwrap_or_default());
    let install = pill_btn(commands, fonts, "Install Theme", rgb(GREEN), PreviewInstallBtn);
    let stop = pill_btn(commands, fonts, "Stop", rgb(hover_bg()), StopPreviewBtn);
    commands.entity(banner).add_children(&[eye, label, install, stop]);
    banner
}

fn pill_btn<M: Component>(commands: &mut Commands, fonts: &EmberFonts, label: &str, bg: Color, marker: M) -> Entity {
    let btn = commands
        .spawn((
            Node { height: Val::Px(22.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, padding: UiRect::horizontal(Val::Px(8.0)), border_radius: BorderRadius::all(Val::Px(3.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(bg),
            Interaction::default(),
            marker,
            Name::new("store-pill"),
        ))
        .id();
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 10.0), TextColor(rgb((255, 255, 255))), FocusPolicy::Pass)).id();
    commands.entity(btn).add_child(t);
    btn
}

fn chip_button<M: Component>(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: Option<&str>, marker: M) -> Entity {
    let btn = commands
        .spawn((
            Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, column_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), border_radius: BorderRadius::all(Val::Px(4.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(rgb(hover_bg())),
            Interaction::default(),
            marker,
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 13.0);
    let mut kids = vec![ic];
    if let Some(l) = label {
        kids.push(commands.spawn((Text::new(l.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())))).id());
    }
    commands.entity(btn).add_children(&kids);
    btn
}

fn build_pager(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let pager = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, column_gap: Val::Px(8.0), flex_shrink: 0.0, ..default() })
        .id();
    bind_display(commands, pager, |w| w.resource::<HubStoreData>().total_pages() > 1);
    let prev = chip_button(commands, fonts, "caret-left", Some("Prev"), StorePageBtn(-1));
    bind_display(commands, prev, |w| w.resource::<HubStoreData>().page > 1);
    let label = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(value_text())))).id();
    bind_text(commands, label, |w| { let d = w.resource::<HubStoreData>(); format!("{} / {}", d.page, d.total_pages()) });
    let next = chip_button(commands, fonts, "caret-right", Some("Next"), StorePageBtn(1));
    bind_display(commands, next, |w| { let d = w.resource::<HubStoreData>(); d.page < d.total_pages() });
    commands.entity(pager).add_children(&[prev, label, next]);
    pager
}

fn categories_snapshot(world: &World) -> KeyedSnapshot {
    let d = world.resource::<HubStoreData>();
    let mut rows: Vec<(Option<String>, String)> = vec![(None, "All".to_string())];
    rows.extend(d.categories.iter().map(|(slug, name)| (Some(slug.clone()), name.clone())));
    let sel = d.category.clone();
    use std::hash::{Hash, Hasher};
    let items: Vec<(u64, u64)> = rows
        .iter()
        .enumerate()
        .map(|(i, (slug, name))| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            (i, slug).hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (name, slug == &sel).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| category_row(c, f, rows[i].0.clone(), &rows[i].1)),
    }
}

fn category_row(commands: &mut Commands, fonts: &EmberFonts, slug: Option<String>, name: &str) -> Entity {
    let icon = if slug.is_none() { "squares-four" } else { "tag" };
    let row = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(22.0), flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), padding: UiRect::horizontal(Val::Px(8.0)), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            StoreCatRow(slug.clone()),
            Name::new("store-cat"),
        ))
        .id();
    {
        let slug = slug.clone();
        bind_bg(commands, row, move |w| {
            let d = w.resource::<HubStoreData>();
            if d.category == slug {
                rgb(accent()).with_alpha(0.18)
            } else if matches!(w.get::<Interaction>(row), Some(Interaction::Hovered) | Some(Interaction::Pressed)) {
                rgb(hover_bg())
            } else {
                Color::NONE
            }
        });
    }
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 11.0);
    let lbl = commands.spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())))).id();
    commands.entity(row).add_children(&[ic, lbl]);
    row
}

fn assets_snapshot(world: &World) -> KeyedSnapshot {
    let d = world.resource::<HubStoreData>();
    if d.loading {
        return note_snapshot("Loading assets...");
    }
    if d.assets.is_empty() {
        return note_snapshot("No assets found. Try a different search or category.");
    }
    let assets = d.assets.clone();
    use std::hash::{Hash, Hasher};
    let items: Vec<(u64, u64)> = assets
        .iter()
        .map(|a| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            a.slug.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (&a.name, &a.category, a.price_credits).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| asset_card(c, f, &assets[i])),
    }
}

fn asset_card(commands: &mut Commands, fonts: &EmberFonts, a: &AssetSummary) -> Entity {
    let card = commands
        .spawn((
            // Flex-grow from a CARD_W basis so each row stretches to fill the
            // panel width (no ragged right-edge gap); capped so a sparse last row
            // doesn't balloon. align_items:FlexStart on the grid keeps heights
            // content-sized (no vertical stretch gaps).
            Node { flex_grow: 1.0, flex_basis: Val::Px(CARD_W), min_width: Val::Px(CARD_W), max_width: Val::Px(CARD_W * 1.35), flex_direction: FlexDirection::Column, border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(6.0)), overflow: Overflow::clip(), ..default() },
            BackgroundColor(rgb(section_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            Name::new("store-card"),
        ))
        .id();
    // Thumbnail.
    let thumb = commands
        .spawn((Node { width: Val::Percent(100.0), height: Val::Px(THUMB_H), align_items: AlignItems::Center, justify_content: JustifyContent::Center, overflow: Overflow::clip(), ..default() }, BackgroundColor(rgb(hover_bg())))).id();
    if let Some(url) = a.thumbnail_url.clone() {
        let img = commands
            .spawn((ImageNode::default(), Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), display: Display::None, ..default() }))
            .id();
        bind_with(
            commands,
            img,
            move |w| w.get_resource::<HubThumbs>().and_then(|t| t.get(&url)),
            |w, e, h: &Option<Handle<Image>>| {
                if let Some(h) = h {
                    if let Some(mut n) = w.get_mut::<ImageNode>(e) {
                        if n.image != *h { n.image = h.clone(); }
                    }
                    if let Some(mut node) = w.get_mut::<Node>(e) { node.display = Display::Flex; }
                }
            },
        );
        commands.entity(thumb).add_child(img);
    }
    // Info.
    let info = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(5.0), padding: UiRect::all(Val::Px(8.0)), ..default() }).id();
    let name = commands.spawn((Text::new(a.name.clone()), ui_font(&fonts.ui, 12.5), TextColor(rgb(text_primary())), bevy::text::TextLayout::no_wrap(), Node { overflow: Overflow::clip(), ..default() })).id();

    // Category with a leading tag icon.
    let cat_row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), ..default() }).id();
    let tag_ic = icon_text(commands, &fonts.phosphor, "tag", text_muted(), 10.0);
    let cat_t = commands.spawn((Text::new(a.category.clone()), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())))).id();
    commands.entity(cat_row).add_children(&[tag_ic, cat_t]);

    // Actions: compact pills, right-aligned, side by side. Preview is icon-only;
    // the Get/Buy pill's label carries the price so no separate price text.
    let actions = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::FlexEnd, column_gap: Val::Px(5.0), margin: UiRect::top(Val::Px(2.0)), ..default() }).id();
    let mut action_kids = Vec::new();
    if is_theme(&a.category) {
        let preview = card_pill(commands, fonts, "eye", None, rgb(hover_bg()), text_primary());
        commands.entity(preview).insert(StorePreviewBtn(a.clone()));
        action_kids.push(preview);
    }
    let (get_label, get_bg) = if a.price_credits == 0 {
        ("Get for free".to_string(), rgb(GREEN))
    } else {
        (format!("Buy ({} credits)", a.price_credits), rgb(accent()))
    };
    let get = card_pill(commands, fonts, "download-simple", Some(&get_label), get_bg, (255, 255, 255));
    commands.entity(get).insert(StoreInstallBtn(a.clone()));
    action_kids.push(get);
    commands.entity(actions).add_children(&action_kids);

    commands.entity(info).add_children(&[name, cat_row, actions]);
    commands.entity(card).add_children(&[thumb, info]);
    card
}

/// A compact card action pill (icon + optional label) that lightens on hover.
/// Icon-only (a fixed square) when `label` is `None`. `fg` colors glyph + label.
fn card_pill(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: Option<&str>, bg: Color, fg: (u8, u8, u8)) -> Entity {
    let mut node = Node {
        height: Val::Px(24.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        column_gap: Val::Px(4.0),
        border_radius: BorderRadius::all(Val::Px(4.0)),
        flex_shrink: 0.0,
        ..default()
    };
    if label.is_some() {
        node.padding = UiRect::horizontal(Val::Px(10.0));
    } else {
        node.width = Val::Px(24.0);
    }
    let btn = commands
        .spawn((node, BackgroundColor(bg), Interaction::default(), Name::new("store-card-action")))
        .id();
    let hover = lighten(bg, 0.16);
    bind_bg(commands, btn, move |w| {
        if matches!(w.get::<Interaction>(btn), Some(Interaction::Hovered) | Some(Interaction::Pressed)) {
            hover
        } else {
            bg
        }
    });
    let ic = icon_text(commands, &fonts.phosphor, icon, fg, 12.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    commands.entity(btn).add_child(ic);
    if let Some(l) = label {
        let t = commands.spawn((Text::new(l.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(fg)), FocusPolicy::Pass)).id();
        commands.entity(btn).add_child(t);
    }
    btn
}

/// Mix `c` toward white by `amt` (0..1) for a lighter hover tint.
fn lighten(c: Color, amt: f32) -> Color {
    let s = c.to_srgba();
    Color::srgba(
        s.red + (1.0 - s.red) * amt,
        s.green + (1.0 - s.green) * amt,
        s.blue + (1.0 - s.blue) * amt,
        s.alpha,
    )
}

fn note_snapshot(text: &'static str) -> KeyedSnapshot {
    // Hash the message into the content key so a state change (e.g. Loading →
    // No assets found) re-runs the builder; a constant key would reuse the old
    // row and leave the stale "Loading..." text on screen.
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    KeyedSnapshot {
        items: vec![(u64::MAX, h.finish())],
        build: Box::new(move |c, f, _| {
            c.spawn((Text::new(text), ui_font(&f.ui, 11.0), TextColor(rgb(text_muted())), Node { margin: UiRect::all(Val::Px(16.0)), ..default() })).id()
        }),
    }
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn poll_store(mut data: ResMut<HubStoreData>) {
    if let Some(rx) = data.asset_rx.as_ref() {
        let mut got = Vec::new();
        while let Ok(r) = rx.try_recv() {
            got.push(r);
        }
        for r in got {
            match r {
                Ok(resp) => {
                    if let Some(sig) = data.pending_sig.take() {
                        data.cache.insert(
                            sig,
                            CachedPage { assets: resp.assets.clone(), total: resp.total, per_page: resp.per_page },
                        );
                    }
                    data.assets = resp.assets;
                    data.total = resp.total;
                    data.per_page = resp.per_page;
                    data.loading = false;
                }
                Err(e) => {
                    data.error = Some(e);
                    data.loading = false;
                }
            }
        }
    }
    if let Some(rx) = data.cat_rx.as_ref() {
        let mut got = Vec::new();
        while let Ok(r) = rx.try_recv() {
            got.push(r);
        }
        for r in got.into_iter().flatten() {
            data.categories = r;
        }
    }
}

fn store_init(mut data: ResMut<HubStoreData>) {
    if data.initialized {
        return;
    }
    data.initialized = true;
    fetch_categories(&mut data);
    fetch_assets(&mut data);
}

fn store_refetch(mut data: ResMut<HubStoreData>) {
    if data.dirty {
        data.dirty = false;
        fetch_assets(&mut data);
    }
}

fn store_search_sync(input: Query<&EmberTextInput, With<StoreSearch>>, mut data: ResMut<HubStoreData>) {
    for inp in &input {
        if data.search != inp.value {
            data.search = inp.value.clone();
        }
    }
}

fn store_search_click(q: Query<&Interaction, (With<StoreSearchBtn>, Changed<Interaction>)>, mut data: ResMut<HubStoreData>) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        data.page = 1;
        data.dirty = true;
    }
}

/// Sort dropdown selection → re-query. Skips the no-op change the dropdown emits
/// when it's first built (it lands on the current sort anyway).
#[allow(clippy::type_complexity)]
fn store_sort_dropdown(
    q: Query<&Bound<usize>, (With<StoreSortDropdown>, Changed<Bound<usize>>)>,
    mut data: ResMut<HubStoreData>,
) {
    for b in &q {
        if let Some((slug, _)) = SORTS.get(b.0) {
            if data.sort.as_str() != *slug {
                data.sort = (*slug).to_string();
                data.page = 1;
                data.dirty = true;
            }
        }
    }
}

fn store_category_click(q: Query<(&Interaction, &StoreCatRow), Changed<Interaction>>, mut data: ResMut<HubStoreData>) {
    for (interaction, row) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if data.category != row.0 {
            data.category = row.0.clone();
            data.page = 1;
            data.dirty = true;
        }
    }
}

fn store_page_click(q: Query<(&Interaction, &StorePageBtn), Changed<Interaction>>, mut data: ResMut<HubStoreData>) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let next = (data.page as i32 + btn.0).max(1) as u32;
        if next != data.page && next <= data.total_pages().max(1) {
            data.page = next;
            data.dirty = true;
        }
    }
}

/// Card "Get / Buy" → open the install confirm overlay. A paid asset for a
/// signed-out user instead opens the sign-in modal (purchase needs an account).
fn store_install_click(
    q: Query<(&Interaction, &StoreInstallBtn), Changed<Interaction>>,
    session: Option<Res<AuthSession>>,
    mut commands: Commands,
) {
    let signed = session.as_ref().map(|s| s.is_signed_in()).unwrap_or(false);
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let asset = btn.0.clone();
        if !signed && asset.price_credits > 0 {
            commands.insert_resource(renzora::core::AuthToggleWindowRequest);
            continue;
        }
        commands.queue(move |world: &mut World| crate::install_overlay::open(world, asset));
    }
}

/// Card "Preview" (theme) → download the theme `.toml` and apply it live.
fn store_preview_click(
    q: Query<(&Interaction, &StorePreviewBtn), Changed<Interaction>>,
    mut preview: ResMut<ThemePreview>,
) {
    if preview.rx.is_some() {
        return;
    }
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            start_preview_download(&mut preview, btn.0.clone());
            break;
        }
    }
}

fn store_signin_click(q: Query<&Interaction, (With<StoreSignInBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.insert_resource(renzora::core::AuthToggleWindowRequest);
    }
}

/// Credit "+" → open the website wallet to buy more credits.
fn store_topup_click(q: Query<&Interaction, (With<StoreTopUpBtn>, Changed<Interaction>)>) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        open_url("https://renzora.com/wallet");
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

fn store_upload_click(q: Query<&Interaction, (With<StoreUploadBtn>, Changed<Interaction>)>) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        renzora::core::console_log::console_info("Marketplace", "Asset upload is coming soon");
    }
}

/// Apply a downloaded preview theme into the editor's `ThemeManager` (saving the
/// current theme first so it can be restored), or surface a parse error.
fn poll_preview(mut preview: ResMut<ThemePreview>, manager: Option<ResMut<ThemeManager>>) {
    let Some(rx) = preview.rx.as_ref() else { return };
    let Ok(res) = rx.try_recv() else { return };
    preview.rx = None;
    let Some(mut manager) = manager else { return };
    match res {
        Ok((name, theme)) => {
            if preview.saved.is_none() {
                preview.saved = Some((manager.active_theme_name.clone(), manager.active_theme.clone()));
            }
            manager.active_theme = theme;
            manager.active_theme_name = format!("Preview \u{00b7} {name}");
            preview.previewing = Some(name);
            preview.error = None;
        }
        Err(e) => {
            preview.error = Some(e.clone());
            renzora::core::console_log::console_warn("Marketplace", format!("Theme preview failed: {e}"));
        }
    }
}

/// Banner "Stop" → restore the saved theme.
fn store_stop_preview_click(
    q: Query<&Interaction, (With<StopPreviewBtn>, Changed<Interaction>)>,
    mut preview: ResMut<ThemePreview>,
    manager: Option<ResMut<ThemeManager>>,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Some(mut manager) = manager else { return };
    if let Some((name, theme)) = preview.saved.take() {
        manager.active_theme = theme;
        manager.active_theme_name = name;
    }
    preview.previewing = None;
    preview.asset = None;
}

/// Banner "Install Theme" → open the install overlay for the previewed asset.
fn store_preview_install_click(
    q: Query<&Interaction, (With<PreviewInstallBtn>, Changed<Interaction>)>,
    preview: Res<ThemePreview>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        if let Some(asset) = preview.asset.clone() {
            commands.queue(move |world: &mut World| crate::install_overlay::open(world, asset));
        }
    }
}

fn request_store_thumbs(data: Res<HubStoreData>, mut thumbs: ResMut<HubThumbs>) {
    for a in &data.assets {
        if let Some(url) = &a.thumbnail_url {
            thumbs.request(url);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn start_preview_download(preview: &mut ThemePreview, asset: AssetSummary) {
    let (tx, rx) = unbounded();
    preview.rx = Some(rx);
    preview.asset = Some(asset.clone());
    preview.error = None;
    std::thread::spawn(move || {
        let result = (|| {
            let url = renzora_auth::marketplace::preview_file_url(&asset.id);
            let bytes = renzora_auth::marketplace::download_file(&url)?;
            let text = String::from_utf8(bytes).map_err(|e| format!("Theme file isn't valid UTF-8: {e}"))?;
            let theme: renzora_theme::Theme =
                toml::from_str(&text).map_err(|e| format!("Couldn't parse theme: {e}"))?;
            Ok::<_, String>((asset.name.clone(), theme))
        })();
        let _ = tx.send(result);
    });
}

#[cfg(target_arch = "wasm32")]
fn start_preview_download(_preview: &mut ThemePreview, _asset: AssetSummary) {}

#[cfg(not(target_arch = "wasm32"))]
fn fetch_assets(data: &mut HubStoreData) {
    let sig = data.query_sig();
    if let Some(page) = data.cache.get(&sig) {
        data.assets = page.assets.clone();
        data.total = page.total;
        data.per_page = page.per_page;
        data.loading = false;
        data.error = None;
        data.asset_rx = None;
        data.pending_sig = None;
        return;
    }

    let query = (!data.search.is_empty()).then(|| data.search.clone());
    let category = data.category.clone();
    let sort = data.sort.clone();
    let page = data.page;
    let (tx, rx) = unbounded();
    data.asset_rx = Some(rx);
    data.pending_sig = Some(sig);
    data.loading = true;
    std::thread::spawn(move || {
        let result = renzora_auth::marketplace::list_assets(query.as_deref(), category.as_deref(), Some(&sort), page);
        let _ = tx.send(result);
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch_categories(data: &mut HubStoreData) {
    let (tx, rx) = unbounded();
    data.cat_rx = Some(rx);
    std::thread::spawn(move || {
        let result = renzora_auth::marketplace::list_categories()
            .map(|cats| cats.into_iter().map(|c| (c.slug, c.name)).collect());
        let _ = tx.send(result);
    });
}

#[cfg(target_arch = "wasm32")]
fn fetch_assets(_data: &mut HubStoreData) {}
#[cfg(target_arch = "wasm32")]
fn fetch_categories(_data: &mut HubStoreData) {}
