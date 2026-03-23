//! Hub Store panel — browse, search, and purchase marketplace assets.

use std::sync::{mpsc, Mutex, RwLock};

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, RichText, Sense, StrokeKind, Vec2};
use renzora_auth::marketplace::{AssetDetail, AssetSummary, Category, MarketplaceListResponse};
use renzora_auth::session::AuthSession;
use renzora_editor::{EditorPanel, PanelLocation};
use renzora_theme::Theme;

use crate::install;

// ── Background result types ──

enum StoreResult {
    Assets(Result<MarketplaceListResponse, String>),
    Categories(Result<Vec<Category>, String>),
    Detail(Result<AssetDetail, String>),
    Purchase(Result<String, String>),
    Download(Result<DownloadComplete, String>),
}

#[allow(dead_code)]
struct DownloadComplete {
    asset_name: String,
    category: String,
    file_url: String,
    data: Vec<u8>,
}

// ── Panel state ──

#[derive(Default)]
enum StoreView {
    #[default]
    Browse,
    Detail(String), // asset slug
}

struct StoreState {
    view: StoreView,
    search_query: String,
    selected_category: Option<String>,
    sort: String,
    page: u32,

    // Cached data
    assets: Vec<AssetSummary>,
    total: i64,
    per_page: i64,
    categories: Vec<Category>,
    detail: Option<AssetDetail>,

    // Status
    loading: bool,
    error: Option<String>,
    status: Option<String>,
    installing: bool,
}

impl Default for StoreState {
    fn default() -> Self {
        Self {
            view: StoreView::Browse,
            search_query: String::new(),
            selected_category: None,
            sort: "newest".into(),
            page: 1,
            assets: Vec::new(),
            total: 0,
            per_page: 24,
            categories: Vec::new(),
            detail: None,
            loading: false,
            error: None,
            status: None,
            installing: false,
        }
    }
}

// ── Panel ──

pub struct HubStorePanel {
    state: RwLock<StoreState>,
    receiver: Mutex<Option<mpsc::Receiver<StoreResult>>>,
    initialized: RwLock<bool>,
}

impl Default for HubStorePanel {
    fn default() -> Self {
        Self {
            state: RwLock::new(StoreState::default()),
            receiver: Mutex::new(None),
            initialized: RwLock::new(false),
        }
    }
}

impl HubStorePanel {
    fn poll_results(&self) {
        let rx_guard = self.receiver.lock().ok();
        let result = rx_guard
            .as_ref()
            .and_then(|opt| opt.as_ref())
            .and_then(|rx| rx.try_recv().ok());

        if let Some(result) = result {
            let mut state = self.state.write().unwrap();
            match result {
                StoreResult::Assets(Ok(resp)) => {
                    state.assets = resp.assets;
                    state.total = resp.total;
                    state.per_page = resp.per_page;
                    state.loading = false;
                }
                StoreResult::Assets(Err(e)) => {
                    state.error = Some(e);
                    state.loading = false;
                }
                StoreResult::Categories(Ok(cats)) => {
                    state.categories = cats;
                }
                StoreResult::Categories(Err(_)) => {}
                StoreResult::Detail(Ok(detail)) => {
                    state.detail = Some(detail);
                    state.loading = false;
                }
                StoreResult::Detail(Err(e)) => {
                    state.error = Some(e);
                    state.loading = false;
                }
                StoreResult::Purchase(Ok(msg)) => {
                    state.status = Some(msg);
                    state.loading = false;
                    // Mark as owned in detail view
                    if let Some(ref mut d) = state.detail {
                        d.owned = Some(true);
                    }
                }
                StoreResult::Purchase(Err(e)) => {
                    state.error = Some(e);
                    state.loading = false;
                }
                StoreResult::Download(Ok(dl)) => {
                    state.installing = false;
                    state.status = Some(format!("Installed \"{}\"", dl.asset_name));
                    // Actual file install happens via the stored project path
                    // (handled in the caller that set up the download).
                }
                StoreResult::Download(Err(e)) => {
                    state.installing = false;
                    state.error = Some(e);
                }
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn fetch_assets(&self) {
        let state = self.state.read().unwrap();
        let query = if state.search_query.is_empty() {
            None
        } else {
            Some(state.search_query.clone())
        };
        let category = state.selected_category.clone();
        let sort = state.sort.clone();
        let page = state.page;
        drop(state);

        let (tx, rx) = mpsc::channel();
        *self.receiver.lock().unwrap() = Some(rx);
        self.state.write().unwrap().loading = true;

        std::thread::spawn(move || {
            let result = renzora_auth::marketplace::list_assets(
                query.as_deref(),
                category.as_deref(),
                Some(&sort),
                page,
            );
            let _ = tx.send(StoreResult::Assets(result));
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn fetch_categories(&self) {
        let (tx, rx) = mpsc::channel();
        // Multiplex onto the same receiver — categories come back first
        *self.receiver.lock().unwrap() = Some(rx);

        std::thread::spawn(move || {
            let result = renzora_auth::marketplace::list_categories();
            let _ = tx.send(StoreResult::Categories(result));
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn fetch_detail(&self, slug: &str) {
        let slug = slug.to_string();
        let (tx, rx) = mpsc::channel();
        *self.receiver.lock().unwrap() = Some(rx);
        self.state.write().unwrap().loading = true;

        std::thread::spawn(move || {
            let result = renzora_auth::marketplace::get_asset(&slug);
            let _ = tx.send(StoreResult::Detail(result));
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn purchase(&self, session: &AuthSession, asset_id: &str) {
        let session_clone = AuthSession {
            user: session.user.clone(),
            access_token: session.access_token.clone(),
            refresh_token: session.refresh_token.clone(),
        };
        let asset_id = asset_id.to_string();
        let (tx, rx) = mpsc::channel();
        *self.receiver.lock().unwrap() = Some(rx);
        self.state.write().unwrap().loading = true;

        std::thread::spawn(move || {
            let result = renzora_auth::marketplace::purchase_asset(&session_clone, &asset_id);
            let _ = tx.send(StoreResult::Purchase(result.map(|r| r.message)));
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn download_and_install(
        &self,
        session: &AuthSession,
        asset_id: &str,
        asset_name: &str,
        category: &str,
        project_path: std::path::PathBuf,
    ) {
        let session_clone = AuthSession {
            user: session.user.clone(),
            access_token: session.access_token.clone(),
            refresh_token: session.refresh_token.clone(),
        };
        let asset_id = asset_id.to_string();
        let asset_name = asset_name.to_string();
        let category = category.to_string();
        let (tx, rx) = mpsc::channel();
        *self.receiver.lock().unwrap() = Some(rx);
        self.state.write().unwrap().installing = true;

        std::thread::spawn(move || {
            let result = (|| {
                let dl_resp =
                    renzora_auth::marketplace::download_asset(&session_clone, &asset_id)
                        .map_err(|e| format!("Failed to get download URL: {e}"))?;
                let data = renzora_auth::marketplace::download_file(&dl_resp.download_url)
                    .map_err(|e| format!("Failed to download file: {e}"))?;

                install::install_asset(
                    &project_path,
                    &category,
                    &asset_name,
                    &dl_resp.download_url,
                    &data,
                )?;

                Ok(DownloadComplete {
                    asset_name,
                    category,
                    file_url: dl_resp.download_url,
                    data,
                })
            })();
            let _ = tx.send(StoreResult::Download(result));
        });
    }
}

impl EditorPanel for HubStorePanel {
    fn id(&self) -> &str {
        "hub_store"
    }

    fn title(&self) -> &str {
        "Store"
    }

    fn icon(&self) -> Option<&str> {
        Some(egui_phosphor::regular::STOREFRONT)
    }

    fn closable(&self) -> bool {
        true
    }

    fn min_size(&self) -> [f32; 2] {
        [300.0, 200.0]
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Center
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        self.poll_results();

        let theme = match world.get_resource::<renzora_theme::ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };
        let session = world
            .get_resource::<AuthSession>()
            .map(|s| AuthSession {
                user: s.user.clone(),
                access_token: s.access_token.clone(),
                refresh_token: s.refresh_token.clone(),
            })
            .unwrap_or_default();
        let project_path = world
            .get_resource::<renzora_core::CurrentProject>()
            .map(|p| p.path.clone());

        // Initialize on first frame
        {
            let mut init = self.initialized.write().unwrap();
            if !*init {
                *init = true;
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.fetch_categories();
                    self.fetch_assets();
                }
            }
        }

        let accent = theme.semantic.accent.to_color32();
        let text_secondary = theme.text.secondary.to_color32();
        let text_muted = theme.text.muted.to_color32();

        // Check current view
        let current_view = {
            let state = self.state.read().unwrap();
            match &state.view {
                StoreView::Browse => None,
                StoreView::Detail(slug) => Some(slug.clone()),
            }
        };

        if let Some(_slug) = current_view {
            self.render_detail_view(ui, &theme, &session, project_path.as_ref(), accent, text_secondary, text_muted);
        } else {
            self.render_browse_view(ui, &theme, &session, accent, text_secondary, text_muted);
        }
    }
}

impl HubStorePanel {
    fn render_browse_view(
        &self,
        ui: &mut egui::Ui,
        theme: &Theme,
        _session: &AuthSession,
        accent: Color32,
        text_secondary: Color32,
        text_muted: Color32,
    ) {
        let mut should_search = false;
        let mut navigate_to: Option<String> = None;
        let mut page_change: Option<u32> = None;

        // ── Toolbar ──
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;

            // Search box
            let mut state = self.state.write().unwrap();
            let resp = ui.add(
                egui::TextEdit::singleline(&mut state.search_query)
                    .desired_width(200.0)
                    .hint_text("Search assets..."),
            );
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                state.page = 1;
                should_search = true;
            }

            let btn = ui.button(RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS).size(13.0));
            if btn.clicked() {
                state.page = 1;
                should_search = true;
            }

            ui.add_space(8.0);

            // Category filter
            let cat_label = state
                .selected_category
                .as_deref()
                .unwrap_or("All Categories")
                .to_string();
            let categories_snapshot: Vec<_> = state
                .categories
                .iter()
                .map(|c| (c.slug.clone(), c.name.clone()))
                .collect();
            let current_cat = state.selected_category.clone();
            egui::ComboBox::from_id_salt("hub_category")
                .selected_text(&cat_label)
                .width(120.0)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(current_cat.is_none(), "All Categories").clicked() {
                        state.selected_category = None;
                        state.page = 1;
                        should_search = true;
                    }
                    for (slug, name) in &categories_snapshot {
                        if ui.selectable_label(
                            current_cat.as_deref() == Some(slug.as_str()),
                            name,
                        ).clicked() {
                            state.selected_category = Some(slug.clone());
                            state.page = 1;
                            should_search = true;
                        }
                    }
                });

            // Sort
            egui::ComboBox::from_id_salt("hub_sort")
                .selected_text(match state.sort.as_str() {
                    "newest" => "Newest",
                    "popular" => "Popular",
                    "price_asc" => "Price: Low to High",
                    "price_desc" => "Price: High to Low",
                    _ => "Newest",
                })
                .width(120.0)
                .show_ui(ui, |ui| {
                    for (val, label) in [
                        ("newest", "Newest"),
                        ("popular", "Popular"),
                        ("price_asc", "Price: Low to High"),
                        ("price_desc", "Price: High to Low"),
                    ] {
                        if ui.selectable_label(state.sort == val, label).clicked() {
                            state.sort = val.to_string();
                            state.page = 1;
                            should_search = true;
                        }
                    }
                });
        });

        ui.add_space(4.0);

        // Status/error
        {
            let state = self.state.read().unwrap();
            if let Some(err) = &state.error {
                ui.label(RichText::new(err).size(11.0).color(Color32::from_rgb(239, 68, 68)));
            }
            if let Some(msg) = &state.status {
                ui.label(RichText::new(msg).size(11.0).color(Color32::from_rgb(34, 197, 94)));
            }
        }

        ui.separator();

        // ── Asset grid ──
        egui::ScrollArea::vertical()
            .id_salt("hub_store_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let state = self.state.read().unwrap();

                if state.loading {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.spinner();
                        ui.label(RichText::new("Loading...").color(text_muted));
                    });
                    return;
                }

                if state.assets.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            RichText::new(egui_phosphor::regular::PACKAGE)
                                .size(32.0)
                                .color(text_muted),
                        );
                        ui.label(RichText::new("No assets found").color(text_muted));
                    });
                    return;
                }

                // Grid of asset cards
                let available_width = ui.available_width();
                let card_width = 200.0_f32;
                let spacing = 8.0_f32;
                let cols = ((available_width + spacing) / (card_width + spacing))
                    .floor()
                    .max(1.0) as usize;

                let panel_bg = theme.surfaces.panel.to_color32();
                let border = theme.widgets.border.to_color32();

                for row in state.assets.chunks(cols) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = spacing;
                        for asset in row {
                            let (rect, response) = ui.allocate_exact_size(
                                Vec2::new(card_width, 140.0),
                                Sense::click(),
                            );

                            // Card background
                            let bg = if response.hovered() {
                                brighten(panel_bg, 12)
                            } else {
                                panel_bg
                            };
                            ui.painter().rect(
                                rect,
                                CornerRadius::same(4),
                                bg,
                                egui::Stroke::new(1.0, border),
                                StrokeKind::Outside,
                            );

                            // Thumbnail placeholder
                            let thumb_rect = egui::Rect::from_min_size(
                                rect.min + egui::vec2(8.0, 8.0),
                                Vec2::new(card_width - 16.0, 60.0),
                            );
                            ui.painter().rect_filled(
                                thumb_rect,
                                CornerRadius::same(2),
                                brighten(panel_bg, 6),
                            );
                            ui.painter().text(
                                thumb_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                egui_phosphor::regular::IMAGE,
                                egui::FontId::proportional(20.0),
                                text_muted,
                            );

                            // Name
                            let name_pos = egui::Pos2::new(rect.left() + 8.0, thumb_rect.bottom() + 6.0);
                            ui.painter().text(
                                name_pos,
                                egui::Align2::LEFT_TOP,
                                truncate_str(&asset.name, 24),
                                egui::FontId::proportional(11.5),
                                theme.text.primary.to_color32(),
                            );

                            // Creator
                            let creator_pos = egui::Pos2::new(rect.left() + 8.0, name_pos.y + 15.0);
                            ui.painter().text(
                                creator_pos,
                                egui::Align2::LEFT_TOP,
                                &asset.creator_name,
                                egui::FontId::proportional(10.0),
                                text_muted,
                            );

                            // Price
                            let price_text = if asset.price_credits == 0 {
                                "Free".to_string()
                            } else {
                                format!("{} credits", asset.price_credits)
                            };
                            let price_color = if asset.price_credits == 0 {
                                Color32::from_rgb(34, 197, 94)
                            } else {
                                accent
                            };
                            ui.painter().text(
                                egui::Pos2::new(rect.right() - 8.0, creator_pos.y),
                                egui::Align2::RIGHT_TOP,
                                &price_text,
                                egui::FontId::proportional(10.0),
                                price_color,
                            );

                            // Downloads
                            let dl_pos = egui::Pos2::new(rect.left() + 8.0, rect.bottom() - 10.0);
                            ui.painter().text(
                                dl_pos,
                                egui::Align2::LEFT_BOTTOM,
                                format!("{} {}", egui_phosphor::regular::DOWNLOAD_SIMPLE, asset.downloads),
                                egui::FontId::proportional(9.5),
                                text_muted,
                            );

                            if response.hovered() {
                                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                            }

                            if response.clicked() {
                                navigate_to = Some(asset.slug.clone());
                            }
                        }
                    });
                    ui.add_space(spacing);
                }

                // ── Pagination ──
                let total_pages = ((state.total as f32) / (state.per_page.max(1) as f32)).ceil() as u32;
                if total_pages > 1 {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if state.page > 1 {
                            if ui.button(RichText::new(egui_phosphor::regular::CARET_LEFT).size(12.0)).clicked() {
                                page_change = Some(state.page - 1);
                            }
                        }
                        ui.label(
                            RichText::new(format!("Page {} of {}", state.page, total_pages))
                                .size(11.0)
                                .color(text_secondary),
                        );
                        if state.page < total_pages {
                            if ui.button(RichText::new(egui_phosphor::regular::CARET_RIGHT).size(12.0)).clicked() {
                                page_change = Some(state.page + 1);
                            }
                        }
                    });
                }
            });

        // ── Apply deferred actions ──
        if should_search {
            #[cfg(not(target_arch = "wasm32"))]
            self.fetch_assets();
        }
        if let Some(slug) = navigate_to {
            self.state.write().unwrap().view = StoreView::Detail(slug.clone());
            self.state.write().unwrap().detail = None;
            self.state.write().unwrap().error = None;
            self.state.write().unwrap().status = None;
            #[cfg(not(target_arch = "wasm32"))]
            self.fetch_detail(&slug);
        }
        if let Some(page) = page_change {
            self.state.write().unwrap().page = page;
            #[cfg(not(target_arch = "wasm32"))]
            self.fetch_assets();
        }
    }

    fn render_detail_view(
        &self,
        ui: &mut egui::Ui,
        _theme: &Theme,
        session: &AuthSession,
        project_path: Option<&std::path::PathBuf>,
        accent: Color32,
        text_secondary: Color32,
        text_muted: Color32,
    ) {
        let mut go_back = false;
        let mut do_purchase: Option<String> = None;
        let mut do_install: Option<(String, String, String)> = None; // (id, name, category)

        // Back button
        ui.horizontal(|ui| {
            let back = ui.button(
                RichText::new(format!("{} Back", egui_phosphor::regular::ARROW_LEFT)).size(11.0),
            );
            if back.clicked() {
                go_back = true;
            }
        });

        ui.separator();

        let state = self.state.read().unwrap();

        if state.loading {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.spinner();
            });
        } else if let Some(detail) = &state.detail {
            egui::ScrollArea::vertical()
                .id_salt("hub_detail_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_space(8.0);

                    // Name
                    ui.label(
                        RichText::new(&detail.name)
                            .size(18.0)
                            .strong(),
                    );
                    ui.add_space(4.0);

                    // Category + Version
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{} {}", egui_phosphor::regular::TAG, &detail.category))
                                .size(11.0)
                                .color(text_secondary),
                        );
                        ui.label(
                            RichText::new(format!("v{}", &detail.version))
                                .size(11.0)
                                .color(text_muted),
                        );
                        ui.label(
                            RichText::new(format!("{} {} downloads", egui_phosphor::regular::DOWNLOAD_SIMPLE, detail.downloads))
                                .size(11.0)
                                .color(text_muted),
                        );
                    });

                    ui.add_space(12.0);

                    // Install directory info
                    let install_dir = install::install_dir_for_category(&detail.category);
                    ui.label(
                        RichText::new(format!("Installs to: {}/", install_dir))
                            .size(10.5)
                            .color(text_muted),
                    );

                    ui.add_space(12.0);

                    // Description
                    ui.label(
                        RichText::new(&detail.description)
                            .size(12.0)
                            .color(text_secondary),
                    );

                    ui.add_space(16.0);

                    // Price + Action buttons
                    let is_owned = detail.owned.unwrap_or(false);
                    let is_free = detail.price_credits == 0;

                    ui.horizontal(|ui| {
                        if is_owned {
                            // Already owned — show install button
                            let install_text = if state.installing {
                                "Installing..."
                            } else {
                                "Install to Project"
                            };
                            let btn = ui.add_sized(
                                [160.0, 28.0],
                                egui::Button::new(
                                    RichText::new(format!("{} {}", egui_phosphor::regular::DOWNLOAD_SIMPLE, install_text))
                                        .color(Color32::WHITE)
                                        .size(12.0),
                                )
                                .fill(Color32::from_rgb(34, 197, 94))
                                .corner_radius(CornerRadius::same(4)),
                            );
                            if btn.clicked() && !state.installing {
                                do_install = Some((
                                    detail.id.clone(),
                                    detail.name.clone(),
                                    detail.category.clone(),
                                ));
                            }
                            if btn.hovered() && !state.installing {
                                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                            }

                            ui.label(
                                RichText::new("Owned")
                                    .size(11.0)
                                    .color(Color32::from_rgb(34, 197, 94)),
                            );
                        } else if !session.is_signed_in() {
                            ui.label(
                                RichText::new("Sign in to purchase")
                                    .size(11.0)
                                    .color(text_muted),
                            );
                        } else {
                            // Purchase button
                            let price_label = if is_free {
                                "Get for Free".to_string()
                            } else {
                                format!("Buy for {} credits", detail.price_credits)
                            };
                            let btn = ui.add_sized(
                                [160.0, 28.0],
                                egui::Button::new(
                                    RichText::new(&price_label)
                                        .color(Color32::WHITE)
                                        .size(12.0),
                                )
                                .fill(accent)
                                .corner_radius(CornerRadius::same(4)),
                            );
                            if btn.clicked() && !state.loading {
                                do_purchase = Some(detail.id.clone());
                            }
                            if btn.hovered() && !state.loading {
                                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                            }
                        }
                    });

                    // Status/error
                    if let Some(err) = &state.error {
                        ui.add_space(8.0);
                        ui.label(RichText::new(err).size(11.0).color(Color32::from_rgb(239, 68, 68)));
                    }
                    if let Some(msg) = &state.status {
                        ui.add_space(8.0);
                        ui.label(RichText::new(msg).size(11.0).color(Color32::from_rgb(34, 197, 94)));
                    }
                });
        } else {
            ui.label(RichText::new("Asset not found").color(text_muted));
        }

        // ── Apply deferred actions ──
        if go_back {
            let mut state = self.state.write().unwrap();
            state.view = StoreView::Browse;
            state.detail = None;
            state.error = None;
            state.status = None;
        }
        if let Some(asset_id) = do_purchase {
            #[cfg(not(target_arch = "wasm32"))]
            self.purchase(session, &asset_id);
        }
        if let Some((asset_id, asset_name, category)) = do_install {
            if let Some(pp) = project_path {
                #[cfg(not(target_arch = "wasm32"))]
                self.download_and_install(session, &asset_id, &asset_name, &category, pp.clone());
            }
        }
    }
}

fn truncate_str(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len())]
    }
}

fn brighten(c: Color32, delta: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(
        c.r().saturating_add(delta),
        c.g().saturating_add(delta),
        c.b().saturating_add(delta),
        c.a(),
    )
}
