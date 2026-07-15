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
use bevy::ui::widget::NodeImageMode;
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
use renzora_ember::widgets::{dropdown, text_input, tint, EmberTextInput};
use renzora::SplashState;
use renzora_theme::ThemeManager;

use crate::thumbs::HubThumbs;

const GREEN: (u8, u8, u8) = (52, 180, 96);
const RED: (u8, u8, u8) = (224, 80, 80);
/// Warm gold for the credit price — reads as "store currency".
const GOLD: (u8, u8, u8) = (238, 184, 82);
const CARD_W: f32 = 200.0;
const THUMB_H: f32 = 142.0;
/// How many top-popular assets rotate through the home hero slider.
const FEATURED_CAP: usize = 6;
/// How many cards a home category shelf shows before "See all".
const SECTION_CAP: usize = 6;
/// Fixed hero height — wide/tall enough to read as a storefront banner.
const HERO_H: f32 = 200.0;

const SORTS: [(&str, &str); 5] = [
    ("popular", "Most Downloaded"),
    ("top_rated", "Top Rated"),
    ("newest", "Newest"),
    ("price_asc", "Price: Low"),
    ("price_desc", "Price: High"),
];

/// Minimum-rating filter options: `(min_rating, label)`. `0` = no filter. Maps to
/// the backend's `min_rating` query param.
const RATINGS: [(i32, &str); 5] = [
    (0, "Any rating"),
    (4, "4★ & up"),
    (3, "3★ & up"),
    (2, "2★ & up"),
    (5, "5★ only"),
];

/// Price filter options: `(max_price, label)`. `None` = no filter, `Some(0)` =
/// free only. Maps to the backend's `max_price` query param.
const PRICES: [(Option<i64>, &str); 5] = [
    (None, "Any price"),
    (Some(0), "Free"),
    (Some(100), "≤ 100 cr"),
    (Some(500), "≤ 500 cr"),
    (Some(1000), "≤ 1000 cr"),
];

/// True for theme-category assets, which get a live "Preview" action.
fn is_theme(category: &str) -> bool {
    category.to_lowercase().contains("theme")
}

/// True for 3D model / animation assets, whose thumbnails are transparent renders
/// (a framed model on nothing). We skip the stretched backdrop for these — a
/// blurred copy of a transparent render behind itself looks wrong.
fn is_3d_thumb(category: &str) -> bool {
    let c = category.to_lowercase();
    c.contains("model") || c.contains("3d") || c.contains("anim")
}

/// Swap a thumbnail `ImageNode`'s texture to `h` once it's loaded and reveal it.
/// Shared by every card/hero image + blurred-backdrop binding.
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

/// One category's home-page shelf: its display `name`, its `slug` (so "See all"
/// can switch the browse query to it), and up to [`SECTION_CAP`] top assets.
struct HomeSection {
    name: String,
    slug: String,
    assets: Vec<AssetSummary>,
}

/// A background home-data result. The featured slider and every category shelf
/// each fetch on their own worker thread and post back over one shared channel,
/// so `poll_store` drains them all through a single receiver.
enum HomeMsg {
    /// Top-popular assets for the hero slider.
    Featured(Result<Vec<AssetSummary>, String>),
    /// A category shelf: `(slug, display name, assets)`.
    Section(String, String, Result<Vec<AssetSummary>, String>),
}

#[derive(Resource)]
struct HubStoreData {
    search: String,
    category: Option<String>,
    sort: String,
    /// Minimum-rating filter (0 = any); sent as `min_rating`.
    min_rating: i32,
    /// Max-price filter in credits (`None` = any, `Some(0)` = free); sent as `max_price`.
    max_price: Option<i64>,
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
    /// Top-popular assets shown in the home hero slider (≤ [`FEATURED_CAP`]).
    featured: Vec<AssetSummary>,
    /// Which featured slide is currently on show.
    featured_index: usize,
    /// Per-category home shelves in category order (empty ones are dropped).
    sections: Vec<HomeSection>,
    /// Guard so the home data (featured + shelves) is fetched exactly once.
    home_loaded: bool,
    /// Bumped whenever the featured set/shelves change or the slide advances, so
    /// the home keyed lists rebuild (the hero rebuilds on every bump).
    home_version: u64,
    /// Single receiver for all home-data worker threads (see [`HomeMsg`]).
    home_rx: Option<Receiver<HomeMsg>>,
}

impl Default for HubStoreData {
    fn default() -> Self {
        Self {
            search: String::new(),
            category: None,
            sort: "popular".into(),
            min_rating: 0,
            max_price: None,
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
            featured: Vec::new(),
            featured_index: 0,
            sections: Vec::new(),
            home_loaded: false,
            home_version: 0,
            home_rx: None,
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
        self.min_rating.hash(&mut h);
        self.max_price.hash(&mut h);
        self.page.hash(&mut h);
        h.finish()
    }
    fn total_pages(&self) -> u32 {
        ((self.total as f32) / (self.per_page.max(1) as f32)).ceil() as u32
    }
    /// Home mode (featured slider + category shelves) versus flat browse
    /// (grid + pager). Home shows only when nothing narrows the view: no search
    /// text and no specific category ("All"). A search or a chosen category —
    /// including a shelf's "See all" — flips to browse.
    fn is_home(&self) -> bool {
        self.search.is_empty() && self.category.is_none()
    }
}

/// Drives the hero's ~6s auto-advance. Manual arrows still work; this just keeps
/// the banner rotating so it reads as alive.
#[derive(Resource)]
struct HomeCarousel(Timer);

impl Default for HomeCarousel {
    fn default() -> Self {
        Self(Timer::from_seconds(9.0, TimerMode::Repeating))
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
        app.init_resource::<HomeCarousel>();
        app.register_panel_content("hub_store", false, build);
        crate::install_overlay::register(app);
        crate::item_overlay::register(app);
        app.add_systems(
            Update,
            (
                poll_store,
                store_init,
                store_home_init,
                store_refetch,
                store_search_sync,
                store_search_click,
                // Nested to keep the outer tuple within Bevy's 20-system cap.
                (store_sort_dropdown, store_rating_dropdown, store_price_dropdown),
                store_category_click,
                store_page_click,
                store_see_all_click,
                (store_hero_arrow_click, store_hero_dot_click),
                store_home_autoadvance,
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
struct StoreRatingDropdown;
#[derive(Component)]
struct StorePriceDropdown;
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
/// Hero slider "‹" (previous slide).
#[derive(Component)]
struct HeroPrevBtn;
/// Hero slider "›" (next slide).
#[derive(Component)]
struct HeroNextBtn;
/// A hero slider dot — carries the slide index it jumps to on click.
#[derive(Component)]
struct HeroDotBtn(usize);
/// A home shelf's header / "See all" — carries the category slug to browse.
#[derive(Component)]
struct StoreSeeAllBtn(String);

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

    // Featured hero slider — pinned above the toolbar so the search sits right
    // under the slideshow. Home mode only (hidden while browsing).
    let hero = build_hero_slot(commands);

    // Toolbar: a large, prominent search bar (the primary way to shop the store)
    // + search button + sort dropdown + total.
    let toolbar = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, align_items: AlignItems::Center, column_gap: Val::Px(8.0), row_gap: Val::Px(6.0), flex_shrink: 0.0, padding: UiRect::vertical(Val::Px(4.0)), ..default() })
        .id();
    let search = text_input(commands, &fonts.ui, "Search assets...", "");
    commands.entity(search).insert((
        StoreSearch,
        Node { flex_grow: 1.0, min_width: Val::Px(140.0), height: Val::Px(38.0), padding: UiRect::axes(Val::Px(14.0), Val::Px(9.0)), align_items: AlignItems::Center, border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(9.0)), ..default() },
        // Lighter surface than the default popup bg so the primary search field
        // stands out from the panel.
        BackgroundColor(rgba([255, 255, 255, 20])),
        BorderColor::all(rgba([255, 255, 255, 34])),
    ));
    let search_btn = chip_button(commands, fonts, "magnifying-glass", None, StoreSearchBtn);
    // Sort + filters (backend: `sort`, `min_rating`, `max_price`).
    let sort_labels: Vec<&str> = SORTS.iter().map(|(_, l)| *l).collect();
    // Default selection mirrors `HubStoreData::default().sort` (popular).
    let default_sort = SORTS.iter().position(|(v, _)| *v == "popular").unwrap_or(0);
    let sort = dropdown(commands, fonts, &sort_labels, default_sort);
    commands.entity(sort).insert(StoreSortDropdown);
    let rating_labels: Vec<&str> = RATINGS.iter().map(|(_, l)| *l).collect();
    let rating = dropdown(commands, fonts, &rating_labels, 0);
    commands.entity(rating).insert(StoreRatingDropdown);
    let price_labels: Vec<&str> = PRICES.iter().map(|(_, l)| *l).collect();
    let price = dropdown(commands, fonts, &price_labels, 0);
    commands.entity(price).insert(StorePriceDropdown);
    let total = commands.spawn((Text::new(""), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_muted())))).id();
    bind_text(commands, total, |w| format!("{} assets", w.resource::<HubStoreData>().total));
    commands.entity(toolbar).add_children(&[search, search_btn, sort, rating, price, total]);

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
        .spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), min_height: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)), ..default() })
        .id();

    // Home: featured slider + category shelves, shown only in home mode.
    let home = build_home(commands);

    // Browse: the flat grid + pager, shown for a search or a chosen category.
    // Wrapped so a single `bind_display` toggles both together.
    let grid = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, align_content: AlignContent::FlexStart, align_items: AlignItems::FlexStart, column_gap: Val::Px(12.0), row_gap: Val::Px(14.0), padding: UiRect::right(Val::Px(4.0)), ..default() })
        .id();
    keyed_list(commands, grid, assets_snapshot);
    let grid_scroll = renzora_ember::widgets::scroll_view(commands, grid);
    let pager = build_pager(commands, fonts);
    let browse = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() })
        .id();
    bind_display(commands, browse, |w| !w.resource::<HubStoreData>().is_home());
    commands.entity(browse).add_children(&[grid_scroll, pager]);

    commands.entity(right).add_children(&[home, browse]);

    commands.entity(split).add_children(&[sidebar, right]);
    commands.entity(root).add_children(&[hero, toolbar, status, banner, split]);
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

    // ── Upload Asset (opens the Publish uploader panel) ──
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
        build: Box::new(move |c, f, i| category_row(c, f, i, rows[i].0.clone(), &rows[i].1)),
    }
}

fn category_row(commands: &mut Commands, fonts: &EmberFonts, idx: usize, slug: Option<String>, name: &str) -> Entity {
    // "All" gets the accent; real categories get their category color + icon.
    let (icon, icon_col) = if slug.is_none() {
        ("squares-four", accent())
    } else {
        (category_icon(name), category_hue(name))
    };
    let row = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(24.0), flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), padding: UiRect::horizontal(Val::Px(8.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
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
            } else if idx.is_multiple_of(2) {
                rgb(row_even())
            } else {
                rgb(row_odd())
            }
        });
    }
    let ic = icon_text(commands, &fonts.phosphor, icon, icon_col, 11.0);
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
    let base = rgb(section_bg());
    let hover = lighten(base, 0.12);
    let card = commands
        .spawn((
            // Flex-grow from a CARD_W basis so each row stretches to fill the
            // panel width (no ragged right-edge gap); capped so a sparse last row
            // doesn't balloon. align_items:FlexStart on the grid keeps heights
            // content-sized.
            Node { flex_grow: 1.0, flex_basis: Val::Px(CARD_W), min_width: Val::Px(CARD_W), max_width: Val::Px(CARD_W * 1.4), flex_direction: FlexDirection::Column, border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(10.0)), overflow: Overflow::clip(), ..default() },
            BackgroundColor(base),
            BorderColor::all(rgba([255, 255, 255, 12])),
            Interaction::default(),
            // Clicking the card opens the item-detail overlay (install/buy live
            // there). Passive children are `FocusPolicy::Pass` so any click but
            // the preview button falls through to here.
            crate::item_overlay::StoreCardBtn(a.clone()),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("store-card"),
        ))
        .id();
    // Hover: lift the surface and accent the border so the whole card reads as
    // clickable (asset-store cards live and die on their hover feedback).
    bind_bg(commands, card, move |w| {
        if matches!(w.get::<Interaction>(card), Some(Interaction::Hovered) | Some(Interaction::Pressed)) {
            hover
        } else {
            base
        }
    });
    bind_with(
        commands,
        card,
        move |w| matches!(w.get::<Interaction>(card), Some(Interaction::Hovered) | Some(Interaction::Pressed)),
        |w, e, hov: &bool| {
            if let Some(mut b) = w.get_mut::<BorderColor>(e) {
                let a = accent();
                *b = BorderColor::all(if *hov { rgba([a.0, a.1, a.2, 150]) } else { rgba([255, 255, 255, 12]) });
            }
        },
    );

    // ── Thumbnail (relative so badges can overlay it) ──
    let thumb = commands
        .spawn((Node { width: Val::Percent(100.0), height: Val::Px(THUMB_H), position_type: PositionType::Relative, align_items: AlignItems::Center, justify_content: JustifyContent::Center, overflow: Overflow::clip(), ..default() }, BackgroundColor(rgb(hover_bg())), FocusPolicy::Pass))
        .id();
    // Only show a category glyph when there's NO thumbnail — otherwise it bled
    // through transparent (3D-render) thumbnails as a cube floating over the art.
    if a.thumbnail_url.is_none() {
        let ph = icon_text(commands, &fonts.phosphor, category_icon(&a.category), placeholder(), 34.0);
        commands.entity(ph).insert(FocusPolicy::Pass);
        commands.entity(thumb).add_child(ph);
    }
    if let Some(url) = a.thumbnail_url.clone() {
        // Backdrop: a BLURRED, darkened copy STRETCHED to fill the whole thumbnail
        // as a soft gradient with no grey bars around the crisp centered art.
        // Skipped for 3D models/animations (transparent renders).
        if !is_3d_thumb(&a.category) {
            let bg = commands
                .spawn((
                    ImageNode { color: Color::srgb(0.30, 0.30, 0.33), image_mode: NodeImageMode::Stretch, ..default() },
                    FocusPolicy::Pass,
                    Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), display: Display::None, ..default() },
                ))
                .id();
            let burl = url.clone();
            bind_with(commands, bg, move |w| w.get_resource::<HubThumbs>().and_then(|t| t.get_blurred(&burl)), apply_thumb);
            commands.entity(thumb).add_child(bg);
        }
        // Foreground: the full artwork, aspect-preserved, over the backdrop.
        let img = commands
            .spawn((ImageNode::default(), FocusPolicy::Pass, Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), display: Display::None, ..default() }))
            .id();
        bind_with(commands, img, move |w| w.get_resource::<HubThumbs>().and_then(|t| t.get(&url)), apply_thumb);
        commands.entity(thumb).add_child(img);
    }
    // Price badge (top-right).
    let badge = price_badge(commands, fonts, a.price_credits);
    commands.entity(thumb).add_child(badge);
    // Live-preview control for themes (top-left) — a labeled "Preview" pill, an
    // engine-only feature. A clear, wide `Block` target so clicking it previews
    // the theme in place and can't be mistaken for a card tap (which would open
    // the detail overlay).
    if is_theme(&a.category) {
        let preview = commands
            .spawn((
                Node { position_type: PositionType::Absolute, top: Val::Px(8.0), left: Val::Px(8.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), border_radius: BorderRadius::all(Val::Px(11.0)), ..default() },
                BackgroundColor(rgba([0, 0, 0, 165])),
                Interaction::default(),
                StorePreviewBtn(a.clone()),
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                renzora_ember::widgets::HoverTooltip::new("Preview this theme live".to_string()),
            ))
            .id();
        let ic = icon_text(commands, &fonts.phosphor, "eye", (235, 235, 240), 11.0);
        commands.entity(ic).insert(FocusPolicy::Pass);
        let label = commands
            .spawn((Text::new("Preview"), ui_font(&fonts.ui, 9.5), TextColor(rgb((235, 235, 240))), FocusPolicy::Pass))
            .id();
        commands.entity(preview).add_children(&[ic, label]);
        commands.entity(thumb).add_child(preview);
    }

    // ── Info ──
    let info = commands
        .spawn((Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), padding: UiRect::all(Val::Px(9.0)), ..default() }, FocusPolicy::Pass))
        .id();
    let name = commands
        .spawn((Text::new(a.name.clone()), ui_font(&fonts.ui, 12.5), TextColor(rgb(text_primary())), bevy::text::TextLayout::no_wrap(), FocusPolicy::Pass, Node { overflow: Overflow::clip(), ..default() }))
        .id();
    commands.entity(info).add_child(name);
    if !a.creator_name.is_empty() {
        let by = commands
            .spawn((Text::new(format!("by {}", a.creator_name)), ui_font(&fonts.ui, 9.5), TextColor(rgb(text_muted())), bevy::text::TextLayout::no_wrap(), FocusPolicy::Pass, Node { overflow: Overflow::clip(), ..default() }))
            .id();
        commands.entity(info).add_child(by);
    }
    // Meta row: category chip on the left, download count on the right.
    let meta = commands
        .spawn((Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), margin: UiRect::top(Val::Px(3.0)), ..default() }, FocusPolicy::Pass))
        .id();
    // Colored per-category chip — the main splash of color in the grid.
    let chue = category_hue(&a.category);
    let cat_chip = commands
        .spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(3.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() }, BackgroundColor(tint(chue, 34)), FocusPolicy::Pass))
        .id();
    let tag_ic = icon_text(commands, &fonts.phosphor, category_icon(&a.category), chue, 8.5);
    commands.entity(tag_ic).insert(FocusPolicy::Pass);
    let cat_t = commands.spawn((Text::new(a.category.clone()), ui_font(&fonts.ui, 9.0), TextColor(rgb(chue)), FocusPolicy::Pass)).id();
    commands.entity(cat_chip).add_children(&[tag_ic, cat_t]);
    let spacer = commands.spawn((Node { flex_grow: 1.0, ..default() }, FocusPolicy::Pass)).id();
    let dl_ic = icon_text(commands, &fonts.phosphor, "download-simple", placeholder(), 9.5);
    commands.entity(dl_ic).insert(FocusPolicy::Pass);
    let dl_t = commands.spawn((Text::new(fmt_count(a.downloads)), ui_font(&fonts.ui, 9.0), TextColor(rgb(placeholder())), FocusPolicy::Pass)).id();
    commands.entity(meta).add_children(&[cat_chip, spacer, dl_ic, dl_t]);
    commands.entity(info).add_child(meta);

    commands.entity(card).add_children(&[thumb, info]);
    card
}

// ── Home (storefront) ──────────────────────────────────────────────────────────

/// The featured hero slider, pinned at the top of the panel (above the search
/// toolbar) so the search bar reads as sitting *under the slideshow*. A
/// fixed-height frame whose single child (the current slide) is rebuilt by a
/// keyed list when `home_version` bumps (arrow / auto-advance). Shown only in
/// home mode.
fn build_hero_slot(commands: &mut Commands) -> Entity {
    let wrap = commands
        .spawn(Node { width: Val::Percent(100.0), flex_shrink: 0.0, ..default() })
        .id();
    let hero = commands
        .spawn(Node { width: Val::Percent(100.0), height: Val::Px(HERO_H), flex_shrink: 0.0, ..default() })
        .id();
    keyed_list(commands, hero, hero_snapshot);
    commands.entity(wrap).add_child(hero);
    bind_display(commands, wrap, |w| w.resource::<HubStoreData>().is_home());
    wrap
}

/// The storefront "home" body: a scrollable column of per-category shelves
/// (the hero now lives at the panel top — see [`build_hero_slot`]). Toggled
/// against the browse grid by `bind_display` on [`HubStoreData::is_home`].
fn build_home(commands: &mut Commands) -> Entity {
    // Natural-height column so the scroll viewport overflows and scrolls (a
    // `flex_grow` column would squash the shelves to fit instead).
    let col = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(16.0), padding: UiRect::right(Val::Px(4.0)), ..default() })
        .id();

    // Category shelves, keyed on `home_version` via their content hashes.
    let sections = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(18.0), ..default() })
        .id();
    keyed_list(commands, sections, sections_snapshot);

    commands.entity(col).add_child(sections);
    let scroll = renzora_ember::widgets::scroll_view(commands, col);
    bind_display(commands, scroll, |w| w.resource::<HubStoreData>().is_home());
    scroll
}

/// One-row keyed snapshot of the hero: the current featured slide, or a subtle
/// placeholder while the featured set loads. The content hash folds in the slide
/// index + `home_version` so advancing rebuilds it.
fn hero_snapshot(world: &World) -> KeyedSnapshot {
    let d = world.resource::<HubStoreData>();
    if d.featured.is_empty() {
        return hero_placeholder_snapshot();
    }
    let idx = d.featured_index.min(d.featured.len() - 1);
    let asset = d.featured[idx].clone();
    let total = d.featured.len();
    let ver = d.home_version;
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (&asset.slug, idx, total, ver).hash(&mut h);
    KeyedSnapshot {
        items: vec![(1, h.finish())],
        build: Box::new(move |c, f, _| build_hero(c, f, &asset, idx, total)),
    }
}

/// A muted "loading" hero shown until the featured set arrives. Same key as the
/// real hero (`1`) with a distinct hash, so it's replaced in place once loaded.
fn hero_placeholder_snapshot() -> KeyedSnapshot {
    KeyedSnapshot {
        items: vec![(1, u64::MAX)],
        build: Box::new(|c, f, _| {
            let frame = c
                .spawn((
                    Node { width: Val::Percent(100.0), height: Val::Percent(100.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(12.0)), ..default() },
                    BackgroundColor(rgb(hover_bg())),
                ))
                .id();
            let t = c.spawn((Text::new("Loading featured assets..."), ui_font(&f.ui, 12.0), TextColor(rgb(text_muted())))).id();
            c.entity(frame).add_child(t);
            frame
        }),
    }
}

/// Build one hero slide: a full-bleed thumbnail with a bottom scrim carrying the
/// name / creator / price, prev/next arrows, and dot indicators. The body is the
/// click target that opens the item-detail overlay — like a store card, passive
/// children are `FocusPolicy::Pass` while the arrows `Block` their own clicks.
fn build_hero(commands: &mut Commands, fonts: &EmberFonts, a: &AssetSummary, index: usize, total: usize) -> Entity {
    let chue = category_hue(&a.category);
    let hero = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), position_type: PositionType::Relative, flex_direction: FlexDirection::Row, overflow: Overflow::clip(), border_radius: BorderRadius::all(Val::Px(12.0)), ..default() },
            BackgroundColor(rgb(section_bg())),
            Interaction::default(),
            crate::item_overlay::StoreCardBtn(a.clone()),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("store-hero"),
        ))
        .id();

    // ── Left: the artwork (fixed ~44%) ──
    let img_box = commands
        .spawn((Node { width: Val::Percent(44.0), height: Val::Percent(100.0), position_type: PositionType::Relative, align_items: AlignItems::Center, justify_content: JustifyContent::Center, overflow: Overflow::clip(), flex_shrink: 0.0, ..default() }, BackgroundColor(rgb(hover_bg())), FocusPolicy::Pass))
        .id();
    if let Some(url) = a.thumbnail_url.clone() {
        // Ambient cover backdrop: a BLURRED, darkened copy STRETCHED to fill the
        // whole box (`NodeImageMode::Stretch` ignores aspect ratio), so any
        // thumbnail fills edge-to-edge as a soft gradient with no grey letterbox
        // bars. Skipped for 3D models/animations (transparent renders).
        if !is_3d_thumb(&a.category) {
            let bg = commands
                .spawn((
                    ImageNode { color: Color::srgb(0.30, 0.30, 0.33), image_mode: NodeImageMode::Stretch, ..default() },
                    FocusPolicy::Pass,
                    Node { position_type: PositionType::Absolute, top: Val::Px(0.0), left: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), display: Display::None, ..default() },
                ))
                .id();
            let burl = url.clone();
            bind_with(commands, bg, move |w| w.get_resource::<HubThumbs>().and_then(|t| t.get_blurred(&burl)), apply_thumb);
            commands.entity(img_box).add_child(bg);
        }
        // Foreground: the full artwork, over the backdrop.
        let img = commands
            .spawn((ImageNode::default(), FocusPolicy::Pass, Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), display: Display::None, ..default() }))
            .id();
        bind_with(commands, img, move |w| w.get_resource::<HubThumbs>().and_then(|t| t.get(&url)), apply_thumb);
        commands.entity(img_box).add_child(img);
    }
    commands.entity(hero).add_child(img_box);

    // ── Right: details fill the rest ── (a faint category-hue wash for color)
    let info = commands
        .spawn((Node { flex_grow: 1.0, height: Val::Percent(100.0), min_width: Val::Px(0.0), flex_direction: FlexDirection::Column, justify_content: JustifyContent::Center, row_gap: Val::Px(6.0), padding: UiRect { left: Val::Px(22.0), right: Val::Px(46.0), top: Val::Px(16.0), bottom: Val::Px(16.0) }, overflow: Overflow::clip(), ..default() }, BackgroundColor(tint(chue, 13)), FocusPolicy::Pass))
        .id();
    let cat_chip = commands
        .spawn((Node { align_self: AlignSelf::FlexStart, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)), border_radius: BorderRadius::all(Val::Px(7.0)), ..default() }, BackgroundColor(tint(chue, 40)), FocusPolicy::Pass))
        .id();
    let cc_ic = icon_text(commands, &fonts.phosphor, category_icon(&a.category), chue, 10.0);
    commands.entity(cc_ic).insert(FocusPolicy::Pass);
    let cc_t = commands.spawn((Text::new(a.category.clone()), ui_font(&fonts.ui, 9.5), TextColor(rgb(chue)), FocusPolicy::Pass)).id();
    commands.entity(cat_chip).add_children(&[cc_ic, cc_t]);
    let name = commands
        .spawn((Text::new(a.name.clone()), ui_font(&fonts.ui, 22.0), TextColor(rgb((245, 245, 248))), bevy::text::TextLayout::no_wrap(), FocusPolicy::Pass, Node { overflow: Overflow::clip(), ..default() }))
        .id();
    let mut info_kids = vec![cat_chip, name];
    if !a.creator_name.is_empty() {
        let by = commands
            .spawn((Text::new(format!("by {}", a.creator_name)), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), FocusPolicy::Pass))
            .id();
        info_kids.push(by);
    }
    if !a.description.trim().is_empty() {
        let mut d: String = a.description.chars().take(150).collect();
        if a.description.chars().count() > 150 {
            d.push('…');
        }
        let desc = commands
            .spawn((Text::new(d), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_muted())), FocusPolicy::Pass, Node { max_width: Val::Percent(100.0), ..default() }))
            .id();
        info_kids.push(desc);
    }
    // View + downloads.
    let actions = commands
        .spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(10.0), margin: UiRect::top(Val::Px(4.0)), ..default() }, FocusPolicy::Pass))
        .id();
    let view = commands
        .spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(5.0), padding: UiRect::axes(Val::Px(13.0), Val::Px(6.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() }, BackgroundColor(rgba([GOLD.0, GOLD.1, GOLD.2, 240])), FocusPolicy::Pass))
        .id();
    let view_ic = icon_text(commands, &fonts.phosphor, "eye", (40, 30, 8), 12.0);
    commands.entity(view_ic).insert(FocusPolicy::Pass);
    let view_t = commands.spawn((Text::new("View"), ui_font(&fonts.ui, 11.0), TextColor(rgb((40, 30, 8))), FocusPolicy::Pass)).id();
    commands.entity(view).add_children(&[view_ic, view_t]);
    // Price pill, right next to View.
    let price = commands
        .spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(3.0), padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() }, BackgroundColor(if a.price_credits == 0 { rgba([GREEN.0, GREEN.1, GREEN.2, 235]) } else { tint(GOLD, 36) }), FocusPolicy::Pass))
        .id();
    if a.price_credits == 0 {
        let t = commands.spawn((Text::new("Free"), ui_font(&fonts.ui, 10.5), TextColor(rgb((255, 255, 255))), FocusPolicy::Pass)).id();
        commands.entity(price).add_child(t);
    } else {
        let ic = icon_text(commands, &fonts.phosphor, "coins", GOLD, 11.0);
        commands.entity(ic).insert(FocusPolicy::Pass);
        let t = commands.spawn((Text::new(format!("{} credits", a.price_credits)), ui_font(&fonts.ui, 10.5), TextColor(rgb(GOLD)), FocusPolicy::Pass)).id();
        commands.entity(price).add_children(&[ic, t]);
    }
    let dl_ic = icon_text(commands, &fonts.phosphor, "download-simple", text_muted(), 11.0);
    commands.entity(dl_ic).insert(FocusPolicy::Pass);
    let dl_t = commands
        .spawn((Text::new(format!("{} downloads", fmt_count(a.downloads))), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())), FocusPolicy::Pass))
        .id();
    commands.entity(actions).add_children(&[view, price, dl_ic, dl_t]);
    info_kids.push(actions);
    commands.entity(info).add_children(&info_kids);
    commands.entity(hero).add_child(info);

    // Prev/next arrows + dots only make sense with more than one slide.
    if total > 1 {
        let prev = hero_arrow(commands, fonts, "caret-left", true, HeroPrevBtn);
        let next = hero_arrow(commands, fonts, "caret-right", false, HeroNextBtn);
        commands.entity(hero).add_children(&[prev, next]);

        // Dots centered along the bottom of the whole hero, in a subtle pill so
        // they stay legible over either the artwork or the info panel.
        let dots = commands
            .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(0.0), right: Val::Px(0.0), bottom: Val::Px(8.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() }, FocusPolicy::Pass))
            .id();
        let dots_pill = commands
            .spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(1.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)), border_radius: BorderRadius::all(Val::Px(10.0)), ..default() }, BackgroundColor(rgba([0, 0, 0, 110])), FocusPolicy::Pass))
            .id();
        for i in 0..total {
            let on = i == index;
            let d = if on { 7.0 } else { 5.0 };
            // Each dot is a padded, `Block`ing hit-target so clicking it jumps to
            // that slide instead of falling through to the hero (which would open
            // the item overlay). The padding makes the tiny dot easy to hit.
            let cell = commands
                .spawn((
                    Node { padding: UiRect::all(Val::Px(4.0)), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() },
                    Interaction::default(),
                    HeroDotBtn(i),
                    renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                ))
                .id();
            let dot = commands
                .spawn((Node { width: Val::Px(d), height: Val::Px(d), border_radius: BorderRadius::all(Val::Px(d / 2.0)), ..default() }, BackgroundColor(if on { rgba([GOLD.0, GOLD.1, GOLD.2, 255]) } else { rgba([255, 255, 255, 130]) }), FocusPolicy::Pass))
                .id();
            commands.entity(cell).add_child(dot);
            commands.entity(dots_pill).add_child(cell);
        }
        commands.entity(dots).add_child(dots_pill);
        commands.entity(hero).add_child(dots);
    }

    hero
}

/// A circular hero navigation arrow pinned to the left or right edge, centered
/// vertically. `Block`s its own click (default focus policy) so it navigates
/// rather than opening the overlay.
fn hero_arrow<M: Component>(commands: &mut Commands, fonts: &EmberFonts, icon: &str, left: bool, marker: M) -> Entity {
    let btn = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(50.0),
                // Pull up by half the height to sit on the vertical centerline.
                margin: UiRect::top(Val::Px(-16.0)),
                left: if left { Val::Px(8.0) } else { Val::Auto },
                right: if left { Val::Auto } else { Val::Px(8.0) },
                width: Val::Px(32.0),
                height: Val::Px(32.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(rgba([0, 0, 0, 150])),
            Interaction::default(),
            marker,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("store-hero-arrow"),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        if matches!(w.get::<Interaction>(btn), Some(Interaction::Hovered) | Some(Interaction::Pressed)) {
            rgba([0, 0, 0, 205])
        } else {
            rgba([0, 0, 0, 150])
        }
    });
    let ic = icon_text(commands, &fonts.phosphor, icon, (235, 235, 240), 16.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    commands.entity(btn).add_child(ic);
    btn
}

/// Keyed snapshot of the home shelves: one row per non-empty category section,
/// keyed by slug and rebuilt when the shelf's assets change.
fn sections_snapshot(world: &World) -> KeyedSnapshot {
    let d = world.resource::<HubStoreData>();
    let sections: Vec<(String, String, Vec<AssetSummary>)> =
        d.sections.iter().map(|s| (s.slug.clone(), s.name.clone(), s.assets.clone())).collect();
    use std::hash::{Hash, Hasher};
    let items: Vec<(u64, u64)> = sections
        .iter()
        .map(|(slug, name, assets)| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            slug.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            name.hash(&mut h);
            for a in assets {
                (&a.slug, &a.name, a.price_credits).hash(&mut h);
            }
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (slug, name, assets) = &sections[i];
            build_section(c, f, slug, name, assets)
        }),
    }
}

/// One category shelf: a clickable header ("See all →") over a wrapping row of
/// the same `asset_card`s the browse grid uses.
fn build_section(commands: &mut Commands, fonts: &EmberFonts, slug: &str, name: &str, assets: &[AssetSummary]) -> Entity {
    let col = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() })
        .id();

    // The whole header row is the "See all" target (its children are `Pass`), so
    // clicking anywhere on it — title or the affordance — enters browse mode.
    let header = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), padding: UiRect::vertical(Val::Px(2.0)), ..default() },
            Interaction::default(),
            StoreSeeAllBtn(slug.to_string()),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("store-section-header"),
        ))
        .id();
    let title = commands.spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 13.5), TextColor(rgb(text_primary())), FocusPolicy::Pass)).id();
    let spacer = commands.spawn((Node { flex_grow: 1.0, ..default() }, FocusPolicy::Pass)).id();
    let see_t = commands.spawn((Text::new("See all"), ui_font(&fonts.ui, 10.5), TextColor(rgb(accent())), FocusPolicy::Pass)).id();
    let see_ic = icon_text(commands, &fonts.phosphor, "arrow-right", accent(), 11.0);
    commands.entity(see_ic).insert(FocusPolicy::Pass);
    commands.entity(header).add_children(&[title, spacer, see_t, see_ic]);

    // A single non-wrapping shelf row (like a store's category rail) — the cards
    // flex-grow to fill the width, so there's never a half-empty second row.
    // "See all" opens the full category.
    let row = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::NoWrap, align_items: AlignItems::FlexStart, column_gap: Val::Px(12.0), overflow: Overflow::clip(), ..default() })
        .id();
    let cards: Vec<Entity> = assets.iter().take(6).map(|a| asset_card(commands, fonts, a)).collect();
    commands.entity(row).add_children(&cards);

    commands.entity(col).add_children(&[header, row]);
    col
}

/// The price pill overlaid on a card thumbnail: a green "Free", or a gold
/// coins + credit count.
fn price_badge(commands: &mut Commands, fonts: &EmberFonts, price: i64) -> Entity {
    let badge = commands
        .spawn((
            Node { position_type: PositionType::Absolute, top: Val::Px(8.0), right: Val::Px(8.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(3.0), padding: UiRect::axes(Val::Px(7.0), Val::Px(3.0)), border_radius: BorderRadius::all(Val::Px(11.0)), ..default() },
            BackgroundColor(if price == 0 { rgba([GREEN.0, GREEN.1, GREEN.2, 235]) } else { rgba([GOLD.0, GOLD.1, GOLD.2, 240]) }),
            FocusPolicy::Pass,
        ))
        .id();
    if price == 0 {
        let t = commands.spawn((Text::new("Free"), ui_font(&fonts.ui, 10.0), TextColor(rgb((255, 255, 255))), FocusPolicy::Pass)).id();
        commands.entity(badge).add_child(t);
    } else {
        let ic = icon_text(commands, &fonts.phosphor, "coins", (40, 30, 8), 10.0);
        commands.entity(ic).insert(FocusPolicy::Pass);
        let t = commands.spawn((Text::new(format!("{price} credits")), ui_font(&fonts.ui, 10.5), TextColor(rgb((40, 30, 8))), FocusPolicy::Pass)).id();
        commands.entity(badge).add_children(&[ic, t]);
    }
    badge
}

/// A distinct accent color per marketplace category — brings color to the
/// otherwise-grey grid (category chips, sidebar icons, the hero).
fn category_hue(category: &str) -> (u8, u8, u8) {
    let c = category.to_lowercase();
    if c.contains("theme") {
        (167, 130, 245) // violet
    } else if c.contains("model") || c.contains("3d") {
        (91, 156, 245) // blue
    } else if c.contains("anim") {
        (240, 140, 90) // orange
    } else if c.contains("material") || c.contains("shader") {
        (80, 200, 190) // teal
    } else if c.contains("texture") || c.contains("hdri") {
        (232, 182, 82) // amber
    } else if c.contains("2d") || c.contains("sprite") {
        (240, 120, 160) // pink
    } else if c.contains("particle") {
        (120, 205, 120) // green
    } else if c.contains("sound") || c.contains("sfx") {
        (205, 130, 240) // magenta
    } else if c.contains("music") {
        (100, 185, 250) // sky
    } else if c.contains("plugin") {
        (240, 165, 90) // tangerine
    } else if c.contains("script") {
        (130, 205, 165) // mint
    } else if c.contains("blueprint") {
        (150, 160, 250) // periwinkle
    } else if c.contains("project") {
        (230, 160, 110)
    } else if c.contains("font") {
        (185, 185, 205)
    } else {
        (150, 160, 185)
    }
}

/// A representative phosphor icon for a marketplace category — the thumbnail
/// placeholder and a hint of what the asset is.
fn category_icon(category: &str) -> &'static str {
    let c = category.to_lowercase();
    if c.contains("theme") {
        "palette"
    } else if c.contains("model") || c.contains("3d") {
        "cube"
    } else if c.contains("anim") {
        "person-simple-run"
    } else if c.contains("material") || c.contains("shader") {
        "sphere"
    } else if c.contains("texture") || c.contains("hdri") {
        "image"
    } else if c.contains("2d") || c.contains("sprite") {
        "image-square"
    } else if c.contains("particle") {
        "sparkle"
    } else if c.contains("sound") || c.contains("sfx") {
        "speaker-high"
    } else if c.contains("music") {
        "music-notes"
    } else if c.contains("plugin") {
        "plug"
    } else if c.contains("script") {
        "code"
    } else if c.contains("blueprint") {
        "tree-structure"
    } else if c.contains("project") {
        "folder-open"
    } else if c.contains("font") {
        "text-aa"
    } else {
        "package"
    }
}

/// Compact count for card meta: `950`, `1.2k`, `13k`.
fn fmt_count(n: i64) -> String {
    if n < 1000 {
        n.to_string()
    } else if n < 10_000 {
        format!("{:.1}k", n as f32 / 1000.0)
    } else {
        format!("{}k", n / 1000)
    }
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
    if let Some(rx) = data.home_rx.as_ref() {
        let mut got = Vec::new();
        while let Ok(m) = rx.try_recv() {
            got.push(m);
        }
        for m in got {
            match m {
                HomeMsg::Featured(Ok(mut assets)) => {
                    assets.truncate(FEATURED_CAP);
                    data.featured = assets;
                    if data.featured_index >= data.featured.len() {
                        data.featured_index = 0;
                    }
                    data.home_version += 1;
                }
                HomeMsg::Section(slug, name, Ok(mut assets)) => {
                    assets.truncate(SECTION_CAP);
                    // Skip empty shelves — an empty category shouldn't take up a row.
                    if !assets.is_empty() {
                        data.sections.push(HomeSection { name, slug, assets });
                        // Threads finish out of order; re-sort so shelves keep the
                        // category list's order regardless of who returned first.
                        let order: std::collections::HashMap<String, usize> =
                            data.categories.iter().enumerate().map(|(i, (s, _))| (s.clone(), i)).collect();
                        data.sections.sort_by_key(|s| order.get(&s.slug).copied().unwrap_or(usize::MAX));
                        data.home_version += 1;
                    }
                }
                // A failed home fetch just leaves that slider/shelf absent.
                HomeMsg::Featured(Err(_)) | HomeMsg::Section(_, _, Err(_)) => {}
            }
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

/// Fetch the storefront home once: the featured slider (top-popular, category
/// agnostic) plus one popular-sorted shelf per category. Waits for the category
/// list (kicked in [`store_init`]) to arrive first, since the shelves are keyed
/// off it. Every query runs on its own worker thread and streams back over one
/// shared channel drained in [`poll_store`], so the UI fills in as results land.
#[cfg(not(target_arch = "wasm32"))]
fn store_home_init(mut data: ResMut<HubStoreData>) {
    // The shelves need categories; wait for the async category fetch to land.
    if data.home_loaded || data.categories.is_empty() {
        return;
    }
    data.home_loaded = true;
    let (tx, rx) = unbounded();
    data.home_rx = Some(rx);

    // Featured = top popular, no category filter.
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let r = renzora_auth::marketplace::list_assets(None, None, Some("popular"), 1, None, None);
            let _ = tx.send(HomeMsg::Featured(r.map(|resp| resp.assets)));
        });
    }
    // One shelf per category, each on its own thread.
    for (slug, name) in data.categories.clone() {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let r = renzora_auth::marketplace::list_assets(None, Some(&slug), Some("popular"), 1, None, None);
            let _ = tx.send(HomeMsg::Section(slug, name, r.map(|resp| resp.assets)));
        });
    }
}

#[cfg(target_arch = "wasm32")]
fn store_home_init(_data: ResMut<HubStoreData>) {}

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

/// Rating filter dropdown → set `min_rating` and re-query.
#[allow(clippy::type_complexity)]
fn store_rating_dropdown(
    q: Query<&Bound<usize>, (With<StoreRatingDropdown>, Changed<Bound<usize>>)>,
    mut data: ResMut<HubStoreData>,
) {
    for b in &q {
        if let Some((r, _)) = RATINGS.get(b.0) {
            if data.min_rating != *r {
                data.min_rating = *r;
                data.page = 1;
                data.dirty = true;
            }
        }
    }
}

/// Price filter dropdown → set `max_price` and re-query.
#[allow(clippy::type_complexity)]
fn store_price_dropdown(
    q: Query<&Bound<usize>, (With<StorePriceDropdown>, Changed<Bound<usize>>)>,
    mut data: ResMut<HubStoreData>,
) {
    for b in &q {
        if let Some((p, _)) = PRICES.get(b.0) {
            if data.max_price != *p {
                data.max_price = *p;
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
        select_category(&mut data, row.0.clone());
    }
}

/// Switch the browse query to `category` (`None` = "All", back to home) and
/// refetch from page 1. Shared by the sidebar rows and the home "See all"
/// buttons so both take the exact same path into browse mode.
fn select_category(data: &mut HubStoreData, category: Option<String>) {
    if data.category != category {
        data.category = category;
        data.page = 1;
        data.dirty = true;
    }
}

/// A shelf header / "See all" → select that category, entering browse mode.
fn store_see_all_click(q: Query<(&Interaction, &StoreSeeAllBtn), Changed<Interaction>>, mut data: ResMut<HubStoreData>) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        select_category(&mut data, Some(btn.0.clone()));
        break;
    }
}

/// Hero ‹ / › → advance the featured slide (wrapping) and bump `home_version`
/// so the slider rebuilds on the new slide.
#[allow(clippy::type_complexity)]
fn store_hero_arrow_click(
    prev: Query<&Interaction, (With<HeroPrevBtn>, Changed<Interaction>)>,
    next: Query<&Interaction, (With<HeroNextBtn>, Changed<Interaction>)>,
    mut data: ResMut<HubStoreData>,
    mut carousel: ResMut<HomeCarousel>,
) {
    let n = data.featured.len();
    if n == 0 {
        return;
    }
    let mut moved = false;
    if prev.iter().any(|i| *i == Interaction::Pressed) {
        data.featured_index = (data.featured_index + n - 1) % n;
        moved = true;
    }
    if next.iter().any(|i| *i == Interaction::Pressed) {
        data.featured_index = (data.featured_index + 1) % n;
        moved = true;
    }
    if moved {
        data.home_version += 1;
        // Manual navigation restarts the auto-advance countdown.
        carousel.0.reset();
    }
}

/// Hero dot → jump straight to that slide (and restart the auto-advance timer).
fn store_hero_dot_click(
    q: Query<(&Interaction, &HeroDotBtn), Changed<Interaction>>,
    mut data: ResMut<HubStoreData>,
    mut carousel: ResMut<HomeCarousel>,
) {
    for (interaction, dot) in &q {
        if *interaction == Interaction::Pressed
            && dot.0 < data.featured.len()
            && data.featured_index != dot.0
        {
            data.featured_index = dot.0;
            data.home_version += 1;
            carousel.0.reset();
            break;
        }
    }
}

/// Advance the hero every ~6s while home is visible and more than one slide
/// exists. Purely a nicety on top of the manual arrows.
fn store_home_autoadvance(time: Res<Time>, mut carousel: ResMut<HomeCarousel>, mut data: ResMut<HubStoreData>) {
    let n = data.featured.len();
    if n < 2 || !data.is_home() {
        return;
    }
    if carousel.0.tick(time.delta()).just_finished() {
        data.featured_index = (data.featured_index + 1) % n;
        data.home_version += 1;
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
pub(crate) fn open_url(url: &str) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

fn store_upload_click(
    q: Query<&Interaction, (With<StoreUploadBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        // Open in the ember dock model the shell actually renders (+ arm a
        // rebuild). Using `DockingState` alone left the panel invisible until a
        // theme switch forced a refresh.
        commands.queue(|world: &mut World| {
            renzora_ember::dock::open_or_focus_panel(world, crate::upload_panel::PANEL_ID);
        });
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
    // The browse grid, the hero slider, and every home shelf all draw thumbnails,
    // so all three sets need requesting — not just the grid.
    let assets = data
        .assets
        .iter()
        .chain(data.featured.iter())
        .chain(data.sections.iter().flat_map(|s| s.assets.iter()));
    for a in assets {
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
    let min_rating = (data.min_rating > 0).then_some(data.min_rating);
    let max_price = data.max_price;
    let (tx, rx) = unbounded();
    data.asset_rx = Some(rx);
    data.pending_sig = Some(sig);
    data.loading = true;
    std::thread::spawn(move || {
        let result = renzora_auth::marketplace::list_assets(
            query.as_deref(),
            category.as_deref(),
            Some(&sort),
            page,
            min_rating,
            max_price,
        );
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
