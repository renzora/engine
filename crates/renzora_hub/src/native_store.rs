//! Bevy-native (ember) port of the egui `HubStorePanel` ("Marketplace") browse
//! view: a search/sort toolbar over a category sidebar + a card grid with
//! pagination. Reuses the shared `HubThumbs` cache. Background list/category
//! fetches arrive over crossbeam channels. The asset-detail overlay modal
//! (purchase/ratings/comments) is a separate follow-up.

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver};

use renzora_auth::marketplace::{AssetSummary, MarketplaceListResponse};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_bg, bind_display, bind_text, bind_with, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{text_input, EmberTextInput};
use renzora::SplashState;

use crate::thumbs::HubThumbs;

const GREEN: (u8, u8, u8) = (52, 180, 96);
const RED: (u8, u8, u8) = (224, 80, 80);
const CARD_W: f32 = 120.0;
const THUMB_H: f32 = 120.0;

const SORTS: [(&str, &str); 4] = [
    ("newest", "Newest"),
    ("popular", "Popular"),
    ("price_asc", "Price: Low"),
    ("price_desc", "Price: High"),
];

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
    cat_rx: Option<Receiver<Result<Vec<(String, String)>, String>>>,
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
            sort: "newest".into(),
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
    fn sort_label(&self) -> &'static str {
        SORTS.iter().find(|(v, _)| *v == self.sort).map(|(_, l)| *l).unwrap_or("Newest")
    }
}

pub struct NativeHubStore;

impl Plugin for NativeHubStore {
    fn build(&self, app: &mut App) {
        app.init_resource::<HubStoreData>();
        app.register_panel_content("hub_store", false, build);
        app.add_systems(
            Update,
            (
                poll_store,
                store_init,
                store_refetch,
                store_search_sync,
                store_search_click,
                store_sort_click,
                store_category_click,
                store_page_click,
                request_store_thumbs,
            )
                .run_if(in_state(SplashState::Editor)),
        );
    }
}

#[derive(Component)]
struct StoreSearch;
#[derive(Component)]
struct StoreSearchBtn;
#[derive(Component)]
struct StoreSortBtn;
#[derive(Component)]
struct StoreCatRow(Option<String>);
#[derive(Component)]
struct StorePageBtn(i32);

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

    // Toolbar: search + search button + sort + total.
    let toolbar = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), flex_shrink: 0.0, ..default() })
        .id();
    let search = text_input(commands, &fonts.ui, "Search assets...", "");
    commands.entity(search).insert((
        StoreSearch,
        Node { flex_grow: 1.0, min_width: Val::Px(0.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), align_items: AlignItems::Center, border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
    ));
    let search_btn = chip_button(commands, fonts, "magnifying-glass", None, StoreSearchBtn);
    let sort_btn = commands
        .spawn((
            Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(rgb(section_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            StoreSortBtn,
            Name::new("store-sort"),
        ))
        .id();
    let sort_lbl = commands.spawn((Text::new("Newest"), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())))).id();
    bind_text(commands, sort_lbl, |w| w.resource::<HubStoreData>().sort_label().to_string());
    let sort_caret = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 9.0);
    commands.entity(sort_btn).add_children(&[sort_lbl, sort_caret]);
    let total = commands.spawn((Text::new(""), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_muted())))).id();
    bind_text(commands, total, |w| format!("{} assets", w.resource::<HubStoreData>().total));
    commands.entity(toolbar).add_children(&[search, search_btn, sort_btn, total]);

    // Status / error.
    let status = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(RED)), Node { flex_shrink: 0.0, ..default() })).id();
    bind_text(commands, status, |w| w.resource::<HubStoreData>().error.clone().map(|e| format!("\u{26a0} {e}")).unwrap_or_default());
    bind_display(commands, status, |w| w.resource::<HubStoreData>().error.is_some());

    // Split: category sidebar + asset grid (both scroll independently).
    let split = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(0.0), flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), ..default() })
        .id();
    let sidebar = commands
        .spawn(Node { width: Val::Px(140.0), flex_shrink: 0.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() })
        .id();
    keyed_list(commands, sidebar, categories_snapshot);
    let sidebar_scroll = renzora_ember::widgets::scroll_view(commands, sidebar);

    let right = commands
        .spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() })
        .id();
    let grid = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, align_content: AlignContent::FlexStart, column_gap: Val::Px(10.0), row_gap: Val::Px(10.0), ..default() })
        .id();
    keyed_list(commands, grid, assets_snapshot);
    let grid_scroll = renzora_ember::widgets::scroll_view(commands, grid);
    // Pagination.
    let pager = build_pager(commands, fonts);
    commands.entity(right).add_children(&[grid_scroll, pager]);

    commands.entity(split).add_children(&[sidebar_scroll, right]);
    commands.entity(root).add_children(&[toolbar, status, split]);
    root
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
            Node { width: Val::Percent(100.0), height: Val::Px(22.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), padding: UiRect::horizontal(Val::Px(8.0)), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
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
            Node { width: Val::Px(CARD_W), flex_direction: FlexDirection::Column, border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(6.0)), overflow: Overflow::clip(), ..default() },
            BackgroundColor(rgb(section_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            Name::new("store-card"),
        ))
        .id();
    // Thumbnail.
    let thumb = commands
        .spawn((Node { width: Val::Percent(100.0), height: Val::Px(THUMB_H), align_items: AlignItems::Center, justify_content: JustifyContent::Center, overflow: Overflow::clip(), ..default() }, BackgroundColor(rgb(hover_bg())))).id();
    let ph = icon_text(commands, &fonts.phosphor, "image", placeholder(), 22.0);
    commands.entity(thumb).add_child(ph);
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
    let info = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), padding: UiRect::all(Val::Px(6.0)), ..default() }).id();
    let name = commands.spawn((Text::new(a.name.clone()), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_primary())), bevy::text::TextLayout::new_with_no_wrap(), Node { overflow: Overflow::clip(), ..default() })).id();
    let bottom = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, ..default() }).id();
    let pill = commands.spawn((Node { padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(2.0)), ..default() }, BackgroundColor(rgb(card_bg())))).id();
    let pill_t = commands.spawn((Text::new(a.category.clone()), ui_font(&fonts.ui, 8.5), TextColor(rgb(value_text())))).id();
    commands.entity(pill).add_child(pill_t);
    let gap = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let (price_s, price_c) = if a.price_credits == 0 { ("Free".to_string(), GREEN) } else { (format!("{} cr", a.price_credits), accent()) };
    let price = commands.spawn((Text::new(price_s), ui_font(&fonts.ui, 9.0), TextColor(rgb(price_c)))).id();
    commands.entity(bottom).add_children(&[pill, gap, price]);
    commands.entity(info).add_children(&[name, bottom]);
    commands.entity(card).add_children(&[thumb, info]);
    card
}

fn note_snapshot(text: &'static str) -> KeyedSnapshot {
    KeyedSnapshot {
        items: vec![(u64::MAX, 0)],
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
                    // Stash the freshly-fetched page under the query it was
                    // requested for, so navigating back to it skips the network.
                    if let Some(sig) = data.pending_sig.take() {
                        data.cache.insert(
                            sig,
                            CachedPage {
                                assets: resp.assets.clone(),
                                total: resp.total,
                                per_page: resp.per_page,
                            },
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

fn store_sort_click(q: Query<&Interaction, (With<StoreSortBtn>, Changed<Interaction>)>, mut data: ResMut<HubStoreData>) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let cur = SORTS.iter().position(|(v, _)| *v == data.sort).unwrap_or(0);
    data.sort = SORTS[(cur + 1) % SORTS.len()].0.to_string();
    data.page = 1;
    data.dirty = true;
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

fn request_store_thumbs(data: Res<HubStoreData>, mut thumbs: ResMut<HubThumbs>) {
    for a in &data.assets {
        if let Some(url) = &a.thumbnail_url {
            thumbs.request(url);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch_assets(data: &mut HubStoreData) {
    let sig = data.query_sig();
    // Cache hit (e.g. paging back to a page already fetched) → apply instantly,
    // no network round-trip.
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
