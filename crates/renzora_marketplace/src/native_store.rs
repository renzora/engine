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

use crate::auth::marketplace::{AssetSummary, MarketplaceListResponse};
use crate::auth::session::AuthSession;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::{Bound, KeyedSnapshot};
use renzora_ember::reactive::Rx;
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text, bind_with, keyed_list};
use renzora_ember::theme::*;
use renzora_ember::widgets::{dropdown, text_input, EmberTextInput};
use renzora::SplashState;
use renzora_theme::ThemeManager;

use crate::thumbs::HubThumbs;

const GREEN: (u8, u8, u8) = (52, 180, 96);
const RED: (u8, u8, u8) = (224, 80, 80);
/// Warm gold for the credit price — reads as "store currency".
const GOLD: (u8, u8, u8) = (238, 184, 82);
/// Tile width. Narrow on purpose: an app store shows a *lot* of icons at once,
/// and the tile is an icon plus two short lines — at 168 it was a card with an
/// icon in it, and a shelf fitted four.
const CARD_W: f32 = 124.0;
/// Corner rounding on the square icon. Matches the proportion a store icon is
/// usually drawn with (~13% of the side), so artwork that already has its own
/// rounded corners lines up instead of showing a sliver of card behind it.
/// Corner radius of the artwork square, and of the card that holds it.
///
/// They are one constant and a sum, not two numbers picked separately. A
/// rounded box inside a rounded box only looks right when the inner radius plus
/// the padding equals the outer one — otherwise the two curves run at different
/// rates and the gap between them pinches at the corners. It was 18 inside a
/// card of 11, so the artwork was *rounder than the thing containing it*.
const ICON_RADIUS: f32 = 9.0;
/// Padding between the artwork and the card edge.
const CARD_PAD: f32 = 7.0;
/// = `ICON_RADIUS + CARD_PAD`. See [`ICON_RADIUS`].
const CARD_RADIUS: f32 = ICON_RADIUS + CARD_PAD;
/// Characters of an asset name a card shows before eliding. Derived from
/// `CARD_W` minus its padding at the name's 12.5px size — an estimate, backed by
/// a hard clip (see `asset_card`).
const NAME_CHARS: usize = 17;

/// Shorten `s` to at most `max` characters, ending in an ellipsis.
///
/// Counts `char`s rather than bytes: an asset name is user-entered and routinely
/// carries accents, CJK or emoji, and slicing a `String` by byte index in the
/// middle of one of those panics.
fn elide(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    // Trailing space before the ellipsis reads as a typo.
    while out.ends_with(' ') {
        out.pop();
    }
    out.push('…');
    out
}
/// How many cards a home category shelf shows before "See all".
///
/// Ten rather than six because the cards are a fixed width now: six of them
/// filled a rail that stretched to the panel, but they fill only about half a
/// line of a maximised window at their real size, and a shelf that is mostly
/// empty space reads as a category with nothing in it.
const SECTION_CAP: usize = 10;

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

/// A background home-data result. Every category shelf fetches on its own worker
/// thread and posts back over one shared channel, so `poll_store` drains them all
/// through a single receiver.
enum HomeMsg {
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
    /// Per-category home shelves in category order (empty ones are dropped).
    sections: Vec<HomeSection>,
    /// Guard so the home shelves are fetched exactly once.
    home_loaded: bool,
    /// Bumped whenever the shelves change, so the home keyed list rebuilds.
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
    /// Home mode (category shelves) versus flat browse
    /// (grid + pager). Home shows only when nothing narrows the view: no search
    /// text and no specific category ("All"). A search or a chosen category —
    /// including a shelf's "See all" — flips to browse.
    fn is_home(&self) -> bool {
        self.search.is_empty() && self.category.is_none()
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
        // The Marketplace is an overlay now, not a docked panel — see
        // `store_overlay` for why. It is deliberately not registered with
        // `register_shell_panel` any more: leaving it in the Add-Panel picker
        // would offer a second, worse way into the same thing.
        crate::store_overlay::register(app);
        crate::install_overlay::register(app);
        crate::item_overlay::register(app);
        // panel-systems-ungated: poll_store drains in-flight async marketplace requests
        app.add_systems(
            Update,
            (
                poll_store,
                store_init,
                store_home_init,
                store_refetch,
                store_search_sync,
                store_search_enter,
                // Nested to keep the outer tuple within Bevy's 20-system cap.
                (store_sort_dropdown, store_rating_dropdown, store_price_dropdown),
                store_category_click,
                store_page_click,
                store_see_all_click,
                store_install_click,
                store_preview_click,
                store_signin_click,
                store_topup_click,
                store_upload_click,
                request_store_thumbs,
            )
                .run_if(in_state(SplashState::Editor)),
        );
        // panel-systems-ungated: async store work must continue while the tab is hidden
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
/// A home shelf's header / "See all" — carries the category slug to browse.
#[derive(Component)]
struct StoreSeeAllBtn(String);

fn signed_in(w: &Rx) -> bool {
    w.get_resource::<AuthSession>().map(|s| s.is_signed_in()).unwrap_or(false)
}

// ── Build ────────────────────────────────────────────────────────────────────

/// Build the store's content. Used both by the dock panel registration and by
/// the overlay (`store_overlay`), which is the same tree in a different
/// container — there is nothing panel-shaped about it.
pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
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

    // ── Toolbar: search, and who you are ─────────────────────────────────────
    //
    // It sits at the top of the *right* column, not across the whole panel. A
    // full-width search bar spanning the sidebar as well implied it searched
    // that too, and stretched a pill to a length no control wants to be; over
    // the grid, it is exactly as wide as the thing it searches.
    //
    // The sort and both filter dropdowns used to be up here too. They are in
    // the sidebar now, with the categories — everything that narrows the list
    // in one column, the list in the other.
    let toolbar = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, align_items: AlignItems::Center, column_gap: Val::Px(10.0), row_gap: Val::Px(6.0), flex_shrink: 0.0, padding: UiRect::vertical(Val::Px(4.0)), ..default() })
        .id();

    // The magnifier is *inside* the field, not a button beside it. A separate
    // search button next to a search box is a second thing to aim at for what
    // Enter already does — and Enter did not do it before, which is the actual
    // reason the button existed. `store_search_enter` fixes that.
    let search_row = commands
        .spawn((
            // 8, not the 18 that made it a full pill. A pill is a *button*
            // shape — it says "press me and something happens once"; a field you
            // type into wants corners closer to the square it actually is, and
            // 8 is what the rest of the panel's boxes use.
            Node { flex_grow: 1.0, min_width: Val::Px(160.0), height: Val::Px(36.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), padding: UiRect::horizontal(Val::Px(12.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(8.0)), ..default() },
            BackgroundColor(rgba([255, 255, 255, 16])),
            BorderColor::all(rgba([255, 255, 255, 28])),
        ))
        .id();
    let search_ic = icon_text(commands, &fonts.phosphor, "magnifying-glass", text_muted(), 13.0);
    commands.entity(search_ic).insert(FocusPolicy::Pass);
    let search = text_input(commands, &fonts.ui, "Search assets...", "");
    commands.entity(search).insert((
        StoreSearch,
        Node { flex_grow: 1.0, min_width: Val::Px(0.0), align_items: AlignItems::Center, ..default() },
        BackgroundColor(Color::NONE),
    ));
    commands.entity(search_row).add_children(&[search_ic, search]);

    // Account + Upload.
    let account_bar = build_account_bar(commands, fonts);
    commands
        .entity(toolbar)
        .add_children(&[search_row, account_bar]);

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

    commands.entity(right).add_children(&[toolbar, home, browse]);

    commands.entity(split).add_children(&[sidebar, right]);
    commands.entity(root).add_children(&[status, banner, split]);
    root
}

/// The account cluster for the **toolbar**: signed-in identity + credit balance,
/// or a Sign In button, plus Upload Asset.
///
/// These lived at the top of the category sidebar, which is where the store's
/// own artwork now starts and where a shopper is looking for genres rather than
/// for their account. Every store puts identity and "sell something" in the top
/// bar; moving them there also gives the sidebar back to the category list,
/// which is what the larger category type is for.
///
/// Returns one row, laid out horizontally rather than the sidebar's column.
fn build_account_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            flex_shrink: 0.0,
            ..default()
        })
        .id();

    // ── Account block ──
    let account = commands
        .spawn((
            Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() },
            BackgroundColor(rgb(section_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();

    // Signed-in identity + balance.
    // A row now, not a column: in the toolbar the identity and the balance sit
    // side by side rather than stacked, so the bar keeps one line's height.
    let signed = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(10.0), ..default() }).id();
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
            Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, column_gap: Val::Px(5.0), padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), flex_shrink: 0.0, ..default() },
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

    commands.entity(col).add_children(&[upload, account]);
    col
}

/// The left column: nothing but the category list now.
///
/// The account header and Upload Asset used to sit above it — see
/// [`build_account_bar`] for why they are in the toolbar instead.
fn build_sidebar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // A surface, not a floating column of text.
    //
    // With the stripes and the caption gone the list had nothing left holding it
    // together — eleven labels adrift against the same background as the grid
    // beside them, with no edge to say where navigation stopped and content
    // started. A panel tint plus a hairline on its right edge is what a sidebar
    // is; the stripes were standing in for it badly.
    //
    // 200 wide because "Materials & Shaders" still wrapped at 180 — the font is
    // wider than the estimate that picked that number, and the honest fix is to
    // measure by looking rather than guess again.
    let col = commands
        .spawn((
            Node { width: Val::Px(200.0), flex_shrink: 0.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(8.0)), border: UiRect::right(Val::Px(1.0)), ..default() },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();

    // Natural-height column (sums its rows) so the scroll viewport overflows and
    // scrolls; with flex_grow the rows would squash to fit instead.
    let cats = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(1.0), ..default() })
        .id();
    keyed_list(commands, cats, categories_snapshot);
    let cats_scroll = renzora_ember::widgets::scroll_view(commands, cats);
    // The category list takes whatever height is left; the sort and filters
    // below it are fixed. Otherwise eleven categories push the filters off the
    // bottom of a short window and there is no way to reach them.
    commands.entity(cats_scroll).entry::<Node>().and_modify(|mut n| {
        n.flex_grow = 1.0;
        n.min_height = Val::Px(0.0);
    });

    // ── Sort and filters ─────────────────────────────────────────────────────
    //
    // Down here with the categories rather than across the top, because they are
    // the same kind of thing: everything that narrows the list in one column,
    // the list itself in the other. Full-width stacked, so three controls of
    // different label lengths line up instead of making a ragged row.
    let sep = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(1.0), flex_shrink: 0.0, margin: UiRect::vertical(Val::Px(6.0)), ..default() },
            BackgroundColor(rgb(border())),
        ))
        .id();

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
    for d in [sort, rating, price] {
        commands.entity(d).entry::<Node>().and_modify(|mut n| {
            n.width = Val::Percent(100.0);
            n.flex_shrink = 0.0;
        });
    }

    let filters = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), flex_shrink: 0.0, ..default() })
        .id();
    let count = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())), Node { margin: UiRect::top(Val::Px(2.0)), ..default() }))
        .id();
    // The count is what the filters produced, so it lives with them.
    bind_text(commands, count, |w| {
        let d = w.resource::<HubStoreData>();
        if d.is_home() { String::new() } else { format!("{} assets", d.total) }
    });
    let sort_l = section_label(commands, fonts, "Sort");
    let rating_l = section_label(commands, fonts, "Rating");
    let price_l = section_label(commands, fonts, "Price");
    commands
        .entity(filters)
        .add_children(&[sort_l, sort, rating_l, rating, price_l, price, count]);

    let cats_label = section_label(commands, fonts, "Categories");
    commands
        .entity(col)
        .add_children(&[cats_label, cats_scroll, sep, filters]);
    col
}

/// A small muted heading over one block of the filter column.
fn section_label(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text.to_string()),
            ui_font(&fonts.ui, 9.5),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::left(Val::Px(2.0)), flex_shrink: 0.0, ..default() },
        ))
        .id()
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

fn categories_snapshot(world: &Rx) -> KeyedSnapshot {
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

/// No `idx` any more: it existed only to pick an odd/even stripe colour, and
/// the stripes are gone.
fn category_row(commands: &mut Commands, fonts: &EmberFonts, slug: Option<String>, name: &str) -> Entity {
    // "All" gets the accent; real categories get their category color + icon.
    let (icon, icon_col) = if slug.is_none() {
        ("squares-four", accent())
    } else {
        (category_icon(name), category_hue(name))
    };
    let row = commands
        .spawn((
            // `min_height` + padding, not a fixed `height`. At 28px fixed, the
            // three two-word categories — "Materials & Shaders", "Textures &
            // HDRIs", "Complete Projects" — wrapped to a second line inside a
            // box that could not grow, and their overflow drew straight over the
            // row beneath. A row that can grow is worth more than rows that are
            // all exactly the same height.
            Node { width: Val::Percent(100.0), min_height: Val::Px(28.0), flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(7.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), border: UiRect::left(Val::Px(2.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::NONE),
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            StoreCatRow(slug.clone()),
            Name::new("store-cat"),
        ))
        .id();
    {
        let slug = slug.clone();
        // No zebra stripes. They are worth having in a dense table of values you
        // read across; on eleven navigation rows they are eleven bands of
        // alternating grey competing with the one row that actually means
        // something — the selected one. Selection and hover are the only two
        // states this list has, so they are the only two it shows.
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
    // A solid accent bar down the selected row's left edge. The 18%-alpha fill
    // alone is easy to lose against the panel at a glance; the bar is what you
    // find when you look for "where am I".
    {
        let slug = slug.clone();
        bind_with(
            commands,
            row,
            move |w| w.resource::<HubStoreData>().category == slug,
            |w, e, sel: &bool| {
                let c = if *sel { rgb(accent()) } else { Color::NONE };
                if let Some(mut b) = w.get_mut::<BorderColor>(e) {
                    // Only the left edge is drawn — the row's `border` is
                    // `UiRect::left`, so the other three have no width to fill.
                    b.left = c;
                }
            },
        );
    }
    // Larger than the 11.0 the rest of the panel uses. The category list is the
    // sidebar's whole job now that the account controls have moved to the
    // toolbar, and at 11 it read as a caption beside the artwork rather than as
    // the primary way to move around the store.
    let ic = icon_text(commands, &fonts.phosphor, icon, icon_col, 13.0);
    let lbl = commands.spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary())))).id();
    commands.entity(row).add_children(&[ic, lbl]);
    row
}

fn assets_snapshot(world: &Rx) -> KeyedSnapshot {
    let d = world.resource::<HubStoreData>();
    if d.loading {
        return note_snapshot("Loading assets...");
    }
    if d.assets.is_empty() {
        return note_snapshot("No assets found. Try a different search or category.");
    }
    let assets = d.assets.clone();
    // A search is the one grid whose results are genuinely mixed, so it is the
    // one grid where a card has to say which category it is from. A category
    // grid already says so in the highlighted sidebar row.
    let show_category = !d.search.trim().is_empty();
    use std::hash::{Hash, Hasher};
    let items: Vec<(u64, u64)> = assets
        .iter()
        .map(|a| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            a.slug.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            // `show_category` is in the hash: it changes what a card draws, so a
            // card left over from a search must rebuild when the search clears.
            (&a.name, &a.category, a.price_credits, a.downloads, show_category).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| asset_card(c, f, &assets[i], show_category)),
    }
}

/// One store tile.
///
/// `show_category` is for search results only, where the grid genuinely holds
/// several categories at once. Everywhere else the answer is already on screen —
/// a shelf has a header naming it, a browse grid has a highlighted sidebar row —
/// and repeating it on forty cards spends the card's one line of context on
/// something the user has just clicked.
fn asset_card(
    commands: &mut Commands,
    fonts: &EmberFonts,
    a: &AssetSummary,
    show_category: bool,
) -> Entity {
    let base = rgb(section_bg());
    let hover = lighten(base, 0.12);
    let card = commands
        .spawn((
            // One fixed width, everywhere.
            //
            // The card used to flex-grow from a `CARD_W` basis so a row filled
            // the panel with no ragged right-edge gap. The cost was that a card
            // had no size of its own — it had the size of whatever slack its row
            // happened to have. A browse grid of forty cards leaves each of them
            // almost none, so they sat near 124px; a home shelf of six cards on
            // the same screen left each of them plenty, so they hit the 1.3 cap
            // at 161. Same builder, same panel, visibly different store,
            // depending only on how many results came back.
            //
            // A fixed width costs a trailing gap of less than one card. That is
            // what every asset store looks like, and it is a much smaller
            // problem than tiles that change size when you click a category.
            //
            // Padded, so the icon reads as an *icon* sitting on the card rather
            // than a banner bleeding to its edges — which is the difference
            // between a store tile and the old landscape card.
            Node { width: Val::Px(CARD_W), flex_shrink: 0.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), padding: UiRect::all(Val::Px(CARD_PAD)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(CARD_RADIUS)), ..default() },
            BackgroundColor(base),
            BorderColor::all(rgba([255, 255, 255, 12])),
            Interaction::default(),
            // Clicking the card opens the item-detail overlay (install/buy live
            // there). Passive children are `FocusPolicy::Pass` so any click but
            // the preview button falls through to here.
            crate::item_overlay::StoreCardBtn(a.clone()),
            // Geometric, unlike `Interaction`: the Get pill is `FocusPolicy::
            // Block`, so once it appears it takes the hover and the card behind
            // it stops being `Hovered` — a pill bound to the card's `Interaction`
            // would hide itself the instant the cursor reached it, and flicker.
            // `cursor_over` is a rect test and does not care what is on top.
            bevy::ui::RelativeCursorPosition::default(),
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

    // ── Icon: a 1:1 square, like an app store ──
    //
    // `aspect_ratio` rather than a fixed height, because the card flex-grows to
    // fill the row — a pixel height would stretch the box wider than tall at any
    // width but one, and put the letterboxing straight back.
    let thumb = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                aspect_ratio: Some(1.0),
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(Val::Px(ICON_RADIUS)),
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
            FocusPolicy::Pass,
        ))
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
            // The radius is repeated on the image itself. bevy_ui's
            // `Overflow::clip` clips to the node's RECT, not to its corner
            // radius, so a square image inside a rounded frame simply covers the
            // corners up — which is exactly what it looked like: rounded cards
            // with square pictures in them.
            let bg = commands
                .spawn((
                    ImageNode { color: Color::srgb(0.30, 0.30, 0.33), image_mode: NodeImageMode::Stretch, ..default() },
                    FocusPolicy::Pass,
                    Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), display: Display::None, border_radius: BorderRadius::all(Val::Px(ICON_RADIUS)), ..default() },
                ))
                .id();
            let burl = url.clone();
            bind_with(commands, bg, move |w| w.get_resource::<HubThumbs>().and_then(|t| t.get_blurred(&burl)), apply_thumb);
            commands.entity(thumb).add_child(bg);
        }
        // Foreground: the full artwork, aspect-preserved, over the backdrop.
        let img = commands
            .spawn((ImageNode::default(), FocusPolicy::Pass, Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), display: Display::None, border_radius: BorderRadius::all(Val::Px(ICON_RADIUS)), ..default() }))
            .id();
        bind_with(commands, img, move |w| w.get_resource::<HubThumbs>().and_then(|t| t.get(&url)), apply_thumb);
        commands.entity(thumb).add_child(img);
    }
    // The price used to be a badge floating on the top-right of the artwork. It
    // is a GET pill at the foot of the card now — the square icons are drawn to
    // be looked at, and a badge sat on top of them.
    //
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
                // The comment above says this blocks; nothing was making it. On
                // 0.19's `Pass` default the press carried on to the card behind,
                // so previewing a theme opened the detail overlay as well.
                FocusPolicy::Block,
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

    // ── Info: name, then one line of context ──
    //
    // The old card stacked name, "by creator", a coloured category chip and a
    // download count — four rows of chrome under a picture. A store tile gets one
    // line of context, so the creator is left to the detail overlay.
    let info = commands
        .spawn((Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }, FocusPolicy::Pass))
        .id();
    // Two mechanisms, because neither is enough on its own.
    //
    // The ellipsis is what you actually see: `elide` cuts the string and adds
    // "…", so a long name ends in a way that looks deliberate. The clipping
    // *wrapper* is the backstop — `Overflow::clip` clips a node's CHILDREN, not
    // the glyphs the node draws itself, so a `Text` with `no_wrap` and `clip`
    // still painted straight over its neighbours (which is exactly what the
    // Music shelf looked like). Putting the text inside a clipping box makes it
    // a child, and then the clip applies.
    //
    // The budget is a character count, not a measurement: the font is
    // proportional, so this cuts a line of capitals early and a line of
    // lowercase late. That is fine — the clip catches anything the estimate
    // lets through, and the tooltip has the whole name.
    let name_box = commands
        .spawn((
            Node { width: Val::Percent(100.0), overflow: Overflow::clip(), ..default() },
            FocusPolicy::Pass,
            renzora_ember::widgets::HoverTooltip::new(a.name.clone()),
        ))
        .id();
    let name = commands
        .spawn((
            Text::new(elide(&a.name, NAME_CHARS)),
            ui_font(&fonts.ui, 12.5),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(name_box).add_child(name);

    // The meta line: what it costs on the left, how many people took it on the
    // right. Both are facts you compare cards by, which is what earns a
    // permanent line; the category is not, because every card in a shelf sits
    // under a header naming it and every card in a browse grid sits beside a
    // highlighted sidebar row saying the same. It comes back only for a search,
    // where the results genuinely are mixed — see `show_category`.
    let sub = commands
        .spawn((Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(5.0), ..default() }, FocusPolicy::Pass))
        .id();
    let mut sub_kids: Vec<Entity> = Vec::new();
    if show_category {
        let chue = category_hue(&a.category);
        sub_kids.push(
            commands
                .spawn((Text::new(a.category.clone()), ui_font(&fonts.ui, 9.5), TextColor(rgb(chue)), bevy::text::TextLayout::no_wrap(), FocusPolicy::Pass, Node { overflow: Overflow::clip(), ..default() }))
                .id(),
        );
    } else {
        // Price as text, not as a filled button. `Free` in green and a credit
        // count in gold carry the same information the pill did at a fraction of
        // its weight — a grid of forty saturated pills was the loudest thing on
        // the page, and the artwork is what the page is for.
        let free = a.price_credits == 0;
        let (label, colour) = if free {
            ("Free".to_string(), GREEN)
        } else {
            (format!("{} credits", a.price_credits), GOLD)
        };
        sub_kids.push(
            commands
                .spawn((Text::new(label), ui_font(&fonts.ui, 10.0), TextColor(rgb(colour)), bevy::text::TextLayout::no_wrap(), FocusPolicy::Pass))
                .id(),
        );
    }
    sub_kids.push(commands.spawn((Node { flex_grow: 1.0, ..default() }, FocusPolicy::Pass)).id());
    let dl_ic = icon_text(commands, &fonts.phosphor, "download-simple", placeholder(), 9.5);
    commands.entity(dl_ic).insert(FocusPolicy::Pass);
    sub_kids.push(dl_ic);
    sub_kids.push(
        commands
            .spawn((Text::new(fmt_count(a.downloads)), ui_font(&fonts.ui, 9.5), TextColor(rgb(placeholder())), bevy::text::TextLayout::no_wrap(), FocusPolicy::Pass))
            .id(),
    );
    commands.entity(sub).add_children(&sub_kids);
    commands.entity(info).add_children(&[name_box, sub]);

    // The action sits *on* the artwork and appears under the cursor, rather than
    // occupying a permanent row on every card. One-click install survives for
    // anyone who wants it; everyone else gets a page of assets instead of a page
    // of buttons, and the card itself still opens the detail overlay where the
    // full-size Get button lives.
    let get = get_pill(commands, fonts, a, card);
    commands.entity(thumb).add_child(get);
    commands.entity(card).add_children(&[thumb, info]);
    card
}

// ── Home (storefront) ──────────────────────────────────────────────────────────

// A featured hero slider sat above the toolbar: a fixed-height banner rotating
// through the top-popular assets on a timer, with arrows and dots. It is gone —
// the search bar is the top of the panel now, and the category shelves below are
// what the storefront leads with.

/// The storefront "home" body: a scrollable column of per-category shelves
/// (the hero now lives at the panel top — see [`build_hero_slot`]). Toggled
/// against the browse grid by `bind_display` on [`HubStoreData::is_home`].
fn build_home(commands: &mut Commands) -> Entity {
    // Natural-height column so the scroll viewport overflows and scrolls (a
    // `flex_grow` column would squash the shelves to fit instead).
    let col = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(28.0), padding: UiRect::right(Val::Px(4.0)), ..default() })
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

/// Keyed snapshot of the home shelves: one row per non-empty category section,
/// keyed by slug and rebuilt when the shelf's assets change.
fn sections_snapshot(world: &Rx) -> KeyedSnapshot {
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
    // 15, and the shelf column below carries the space. The headers were only a
    // point above the card names, so a page of shelves read as one long list
    // with occasional bold rows in it rather than as sections.
    let title = commands.spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 15.0), TextColor(rgb(text_primary())), FocusPolicy::Pass)).id();
    // A filled pill, not accent-coloured text. As text it was the same weight
    // and nearly the same colour as a link in a body of prose, floating at the
    // right end of a header with nothing to say it was pressable; the pill
    // reads as a control at a glance and the white text on accent is the only
    // pairing in this panel that has real contrast.
    let see = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                height: Val::Px(24.0),
                padding: UiRect::horizontal(Val::Px(11.0)),
                border_radius: BorderRadius::all(Val::Px(12.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(accent())),
            // `Pass`: the whole header is the target, so the pill must not eat
            // the press — it is the affordance for the row, not a second button.
            FocusPolicy::Pass,
        ))
        .id();
    let see_t = commands.spawn((Text::new("See all"), ui_font(&fonts.ui, 10.5), TextColor(Color::WHITE), FocusPolicy::Pass)).id();
    let see_ic = icon_text(commands, &fonts.phosphor, "arrow-right", (255, 255, 255), 11.0);
    commands.entity(see_ic).insert(FocusPolicy::Pass);
    commands.entity(see).add_children(&[see_t, see_ic]);
    // Beside the title, not banished to the far right of the row. It belongs to
    // *that* category — at the opposite end of a full-width header it read as a
    // control for the panel, and on a wide window it was a hundred millimetres
    // of empty space away from the word it acts on.
    let after = commands.spawn((Node { flex_grow: 1.0, ..default() }, FocusPolicy::Pass)).id();
    commands.entity(header).add_children(&[title, see, after]);

    // The same wrapping grid the browse view uses, with the same gaps — because
    // it *is* the browse view, showing one category's top few. It was a
    // non-wrapping rail that clipped, which meant a narrow window silently hid
    // cards with no way to reach them but "See all".
    let row = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, align_content: AlignContent::FlexStart, align_items: AlignItems::FlexStart, column_gap: Val::Px(12.0), row_gap: Val::Px(14.0), ..default() })
        .id();
    // `SECTION_CAP`, not a second hardcoded 6 — they had drifted apart already
    // in waiting.
    // `false`: the header directly above these cards is the category.
    let cards: Vec<Entity> = assets
        .iter()
        .take(SECTION_CAP)
        .map(|a| asset_card(commands, fonts, a, false))
        .collect();
    commands.entity(row).add_children(&cards);

    commands.entity(col).add_children(&[header, row]);
    col
}

/// The store tile's action: **Download** when free, the credit price when not.
///
/// `Block`ing and carrying [`StoreInstallBtn`], so pressing it goes straight to
/// install or purchase rather than opening the detail overlay the rest of the
/// card opens.
///
/// # Why it hides
///
/// It used to be a permanent full-width pill under every card. Forty saturated
/// green rectangles were the loudest thing in the window, and a marketplace's
/// job is to show you the work — the grid read as a page of buttons with some
/// pictures above them. It sits on the artwork now and appears only while the
/// cursor is over its card, which costs nothing: the card opens the detail
/// overlay, where the same action is a full-size button that is always there.
/// The price it used to carry moved to the meta line as text, so nothing is
/// hidden — only the *button* is.
///
/// Bound to `card`'s [`RelativeCursorPosition`] rather than its `Interaction`,
/// because this pill blocks focus and would otherwise steal the hover it depends
/// on the moment the cursor arrived.
fn get_pill(commands: &mut Commands, fonts: &EmberFonts, a: &AssetSummary, card: Entity) -> Entity {
    let free = a.price_credits == 0;
    let base = if free { rgba([GREEN.0, GREEN.1, GREEN.2, 235]) } else { rgba([GOLD.0, GOLD.1, GOLD.2, 240]) };
    let hot = if free { rgba([GREEN.0, GREEN.1, GREEN.2, 255]) } else { rgba([GOLD.0, GOLD.1, GOLD.2, 255]) };
    let fg = if free { (255, 255, 255) } else { (40, 30, 8) };

    let pill = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                right: Val::Px(8.0),
                bottom: Val::Px(8.0),
                height: Val::Px(26.0),
                display: Display::None,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(4.0),
                border_radius: BorderRadius::all(Val::Px(13.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(base),
            Interaction::default(),
            StoreInstallBtn(a.clone()),
            // REQUIRED, not belt-and-braces. Bevy 0.19 made `Node` require
            // `FocusPolicy` and defaults it to `Pass`, so `ui_focus_system` marks
            // every node under the cursor as pressed and only stops at a `Block`.
            // Without this the pill downloads AND the press carries on to the
            // card's `StoreCardBtn` behind it, opening the detail overlay too.
            FocusPolicy::Block,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    bind_bg(commands, pill, move |w| {
        if matches!(w.get::<Interaction>(pill), Some(Interaction::Hovered) | Some(Interaction::Pressed)) {
            hot
        } else {
            base
        }
    });
    bind_display(commands, pill, move |w| {
        w.get::<bevy::ui::RelativeCursorPosition>(card)
            .is_some_and(|r| r.cursor_over)
    });

    if free {
        let ic = icon_text(commands, &fonts.phosphor, "download-simple", fg, 10.5);
        commands.entity(ic).insert(FocusPolicy::Pass);
        let t = commands
            .spawn((Text::new("Download"), ui_font(&fonts.ui, 10.5), TextColor(rgb(fg)), FocusPolicy::Pass))
            .id();
        commands.entity(pill).add_children(&[ic, t]);
    } else {
        let ic = icon_text(commands, &fonts.phosphor, "coins", fg, 10.5);
        commands.entity(ic).insert(FocusPolicy::Pass);
        let t = commands
            .spawn((Text::new(format!("{}", a.price_credits)), ui_font(&fonts.ui, 10.5), TextColor(rgb(fg)), FocusPolicy::Pass))
            .id();
        commands.entity(pill).add_children(&[ic, t]);
    }
    pill
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
                // A failed home fetch just leaves that shelf absent.
                HomeMsg::Section(_, _, Err(_)) => {}
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

/// Fetch the storefront home once: one popular-sorted shelf per category. Waits
/// for the category list (kicked in [`store_init`]) to arrive first, since the
/// shelves are keyed off it. Every query runs on its own worker thread and
/// streams back over one shared channel drained in [`poll_store`], so the UI
/// fills in as results land.
#[cfg(not(target_arch = "wasm32"))]
fn store_home_init(mut data: ResMut<HubStoreData>) {
    // The shelves need categories; wait for the async category fetch to land.
    if data.home_loaded || data.categories.is_empty() {
        return;
    }
    data.home_loaded = true;
    let (tx, rx) = unbounded();
    data.home_rx = Some(rx);

    // One shelf per category, each on its own thread.
    for (slug, name) in data.categories.clone() {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let r = crate::auth::marketplace::list_assets(None, Some(&slug), Some("popular"), 1, None, None);
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

/// Enter in the search field runs the search.
///
/// This is what the magnifier button beside the box was for. Typing only ever
/// updated `data.search`; nothing queried until that button was pressed, so a
/// user who typed and hit Enter — which is everyone — got nothing and had to go
/// find a second target for the gesture they had already made. `text_input`
/// deliberately leaves Enter to whoever owns the field (see its `Key::Enter`
/// arm), and this is that owner.
fn store_search_enter(
    keys: Res<ButtonInput<KeyCode>>,
    inputs: Query<&EmberTextInput, With<StoreSearch>>,
    mut data: ResMut<HubStoreData>,
) {
    if !keys.just_pressed(KeyCode::Enter) && !keys.just_pressed(KeyCode::NumpadEnter) {
        return;
    }
    if !inputs.iter().any(|i| i.focused) {
        return;
    }
    data.page = 1;
    data.dirty = true;
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
    // The browse grid and every home shelf both draw thumbnails, so both sets
    // need requesting — not just the grid.
    let assets = data
        .assets
        .iter()
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
            let url = crate::auth::marketplace::preview_file_url(&asset.id);
            let bytes = crate::auth::marketplace::download_file(&url)?;
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
        let result = crate::auth::marketplace::list_assets(
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
        let result = crate::auth::marketplace::list_categories()
            .map(|cats| cats.into_iter().map(|c| (c.slug, c.name)).collect());
        let _ = tx.send(result);
    });
}

#[cfg(target_arch = "wasm32")]
fn fetch_assets(_data: &mut HubStoreData) {}
#[cfg(target_arch = "wasm32")]
fn fetch_categories(_data: &mut HubStoreData) {}
