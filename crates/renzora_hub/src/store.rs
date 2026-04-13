//! Hub Marketplace panel — browse grid of marketplace assets with category sidebar.
//!
//! Clicking an asset opens the [`AssetOverlay`] modal for full detail,
//! purchase, ratings, and comments.

use std::sync::{mpsc, Mutex, RwLock};

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, RichText, Sense, Stroke, StrokeKind, Vec2};
use renzora_auth::marketplace::{AssetSummary, Category, MarketplaceListResponse};
use renzora_auth::session::AuthSession;
use renzora_editor_framework::{EditorPanel, PanelLocation};
use renzora_theme::Theme;

use crate::images::ImageCache;
use crate::overlay::AssetOverlay;

// ── Panel state ─────────────────────────────────────────────────────────────

struct StoreState {
    search_query: String,
    selected_category: Option<String>,
    sort: String,
    page: u32,

    assets: Vec<AssetSummary>,
    total: i64,
    per_page: i64,
    categories: Vec<Category>,

    loading: bool,
    error: Option<String>,
    status: Option<String>,

    sidebar_width: f32,
}

impl Default for StoreState {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            selected_category: None,
            sort: "newest".into(),
            page: 1,
            assets: Vec::new(),
            total: 0,
            per_page: 24,
            categories: Vec::new(),
            loading: false,
            error: None,
            status: None,
            sidebar_width: 150.0,
        }
    }
}

// ── Panel ───────────────────────────────────────────────────────────────────

pub struct HubStorePanel {
    state: RwLock<StoreState>,
    asset_rx: Mutex<Option<mpsc::Receiver<Result<MarketplaceListResponse, String>>>>,
    category_rx: Mutex<Option<mpsc::Receiver<Result<Vec<Category>, String>>>>,
    initialized: RwLock<bool>,
    images: Mutex<ImageCache>,
    pub(crate) overlay: AssetOverlay,
}

impl Default for HubStorePanel {
    fn default() -> Self {
        Self {
            state: RwLock::new(StoreState::default()),
            asset_rx: Mutex::new(None),
            category_rx: Mutex::new(None),
            initialized: RwLock::new(false),
            images: Mutex::new(ImageCache::default()),
            overlay: AssetOverlay::default(),
        }
    }
}

impl HubStorePanel {
    fn poll_results(&self, ctx: &egui::Context) {
        // Poll asset results
        if let Ok(guard) = self.asset_rx.lock() {
            if let Some(rx) = guard.as_ref() {
                if let Ok(result) = rx.try_recv() {
                    let mut state = self.state.write().unwrap();
                    match result {
                        Ok(resp) => {
                            state.assets = resp.assets;
                            state.total = resp.total;
                            state.per_page = resp.per_page;
                            state.loading = false;
                        }
                        Err(e) => {
                            state.error = Some(e);
                            state.loading = false;
                        }
                    }
                }
            }
        }

        // Poll category results
        if let Ok(guard) = self.category_rx.lock() {
            if let Some(rx) = guard.as_ref() {
                if let Ok(result) = rx.try_recv() {
                    if let Ok(cats) = result {
                        self.state.write().unwrap().categories = cats;
                    }
                }
            }
        }

        if let Ok(mut images) = self.images.lock() {
            images.poll(ctx);
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
        *self.asset_rx.lock().unwrap() = Some(rx);
        self.state.write().unwrap().loading = true;

        std::thread::spawn(move || {
            let result = renzora_auth::marketplace::list_assets(
                query.as_deref(),
                category.as_deref(),
                Some(&sort),
                page,
            );
            let _ = tx.send(result);
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn fetch_categories(&self) {
        let (tx, rx) = mpsc::channel();
        *self.category_rx.lock().unwrap() = Some(rx);

        std::thread::spawn(move || {
            let result = renzora_auth::marketplace::list_categories();
            let _ = tx.send(result);
        });
    }
}

impl EditorPanel for HubStorePanel {
    fn id(&self) -> &str {
        "hub_store"
    }

    fn title(&self) -> &str {
        "Marketplace"
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
        self.poll_results(ui.ctx());

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

        self.render_browse_view(ui, &theme, &session);

        // Overlay renders on top of everything
        self.overlay.show(ui.ctx(), world);
    }
}

// ── Constants ───────────────────────────────────────────────────────────────

const CARD_WIDTH: f32 = 120.0;
const CARD_HEIGHT: f32 = 170.0;
const THUMB_HEIGHT: f32 = 120.0;
const CARD_RADIUS: f32 = 6.0;
const CARD_SPACING: f32 = 10.0;

impl HubStorePanel {
    fn render_browse_view(&self, ui: &mut egui::Ui, theme: &Theme, session: &AuthSession) {
        let accent = theme.semantic.accent.to_color32();
        let text_primary = theme.text.primary.to_color32();
        let text_secondary = theme.text.secondary.to_color32();
        let text_muted = theme.text.muted.to_color32();
        let surface = theme.surfaces.panel.to_color32();
        let border = theme.widgets.border.to_color32();

        let mut should_search = false;
        let mut navigate_to: Option<String> = None;
        let mut page_change: Option<u32> = None;

        // ── Header + Search Toolbar ──
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;

            let mut state = self.state.write().unwrap();
            let resp = ui.add(
                egui::TextEdit::singleline(&mut state.search_query)
                    .desired_width(180.0)
                    .hint_text(format!(
                        "{} Search assets...",
                        egui_phosphor::regular::MAGNIFYING_GLASS
                    )),
            );
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                state.page = 1;
                should_search = true;
            }

            // Sort
            egui::ComboBox::from_id_salt("hub_sort")
                .selected_text(match state.sort.as_str() {
                    "newest" => "Newest",
                    "popular" => "Popular",
                    "price_asc" => "Price: Low",
                    "price_desc" => "Price: High",
                    _ => "Newest",
                })
                .width(100.0)
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

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{} assets", state.total))
                        .size(10.5)
                        .color(text_muted),
                );
            });
        });

        ui.add_space(2.0);

        // Status/error bar
        {
            let mut state = self.state.write().unwrap();
            if let Some(err) = state.error.take() {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{} {}", egui_phosphor::regular::WARNING, err))
                            .size(11.0)
                            .color(Color32::from_rgb(239, 68, 68)),
                    );
                });
            }
            if let Some(msg) = state.status.take() {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} {}",
                            egui_phosphor::regular::CHECK_CIRCLE,
                            msg
                        ))
                        .size(11.0)
                        .color(Color32::from_rgb(34, 197, 94)),
                    );
                });
            }
        }

        ui.separator();

        // ── Split layout: category sidebar (left) + asset grid (right) ──
        let content_rect = ui.available_rect_before_wrap();
        if content_rect.height() < 10.0 {
            return;
        }

        let sidebar_width = self
            .state
            .read()
            .unwrap()
            .sidebar_width
            .clamp(100.0, (content_rect.width() - 200.0).max(100.0));
        let splitter_width = 4.0;

        let sidebar_rect = egui::Rect::from_min_max(
            content_rect.min,
            egui::pos2(content_rect.min.x + sidebar_width, content_rect.max.y),
        );
        let splitter_rect = egui::Rect::from_min_max(
            egui::pos2(sidebar_rect.max.x, content_rect.min.y),
            egui::pos2(sidebar_rect.max.x + splitter_width, content_rect.max.y),
        );
        let grid_rect = egui::Rect::from_min_max(
            egui::pos2(splitter_rect.max.x, content_rect.min.y),
            content_rect.max,
        );

        // Draggable splitter
        let splitter_id = ui.id().with("marketplace_splitter");
        let splitter_resp =
            ui.interact(splitter_rect, splitter_id, egui::Sense::click_and_drag());
        if splitter_resp.dragged() {
            let new_w = (sidebar_width + splitter_resp.drag_delta().x)
                .clamp(100.0, (content_rect.width() - 200.0).max(100.0));
            self.state.write().unwrap().sidebar_width = new_w;
        }
        if splitter_resp.hovered() || splitter_resp.dragged() {
            ui.ctx()
                .set_cursor_icon(CursorIcon::ResizeHorizontal);
        }
        let splitter_color = if splitter_resp.hovered() || splitter_resp.dragged() {
            accent
        } else {
            border
        };
        ui.painter().vline(
            splitter_rect.center().x,
            splitter_rect.y_range(),
            Stroke::new(1.0, splitter_color),
        );

        // ── Category sidebar ──
        let mut sidebar_ui = ui.new_child(egui::UiBuilder::new().max_rect(sidebar_rect));
        egui::ScrollArea::vertical()
            .id_salt("hub_category_scroll")
            .auto_shrink([false, false])
            .show(&mut sidebar_ui, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Categories")
                            .size(10.0)
                            .color(text_muted),
                    );
                });
                ui.add_space(4.0);

                let row_height = 22.0;
                let mut state = self.state.write().unwrap();
                let current_cat = state.selected_category.clone();
                let categories_snapshot: Vec<_> = state
                    .categories
                    .iter()
                    .map(|c| (c.slug.clone(), c.name.clone()))
                    .collect();

                // "All" row
                {
                    let selected = current_cat.is_none();
                    let (rect, resp) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), row_height),
                        Sense::click(),
                    );
                    let bg = if selected {
                        accent.linear_multiply(0.15)
                    } else if resp.hovered() {
                        brighten(surface, 8)
                    } else {
                        Color32::TRANSPARENT
                    };
                    ui.painter()
                        .rect_filled(rect, CornerRadius::same(3), bg);
                    if selected {
                        ui.painter().rect_stroke(
                            rect,
                            CornerRadius::same(3),
                            Stroke::new(1.0, accent.linear_multiply(0.4)),
                            StrokeKind::Inside,
                        );
                    }
                    let text_color = if selected { accent } else { text_primary };
                    ui.painter().text(
                        egui::pos2(rect.min.x + 10.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        format!("{}  All", egui_phosphor::regular::SQUARES_FOUR),
                        egui::FontId::proportional(11.0),
                        text_color,
                    );
                    if resp.clicked() && !selected {
                        state.selected_category = None;
                        state.page = 1;
                        should_search = true;
                    }
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                }

                // Category rows
                for (slug, name) in &categories_snapshot {
                    let selected = current_cat.as_deref() == Some(slug.as_str());
                    let (rect, resp) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), row_height),
                        Sense::click(),
                    );
                    let bg = if selected {
                        accent.linear_multiply(0.15)
                    } else if resp.hovered() {
                        brighten(surface, 8)
                    } else {
                        Color32::TRANSPARENT
                    };
                    ui.painter()
                        .rect_filled(rect, CornerRadius::same(3), bg);
                    if selected {
                        ui.painter().rect_stroke(
                            rect,
                            CornerRadius::same(3),
                            Stroke::new(1.0, accent.linear_multiply(0.4)),
                            StrokeKind::Inside,
                        );
                    }
                    let text_color = if selected { accent } else { text_primary };
                    ui.painter().text(
                        egui::pos2(rect.min.x + 10.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        format!("{}  {}", egui_phosphor::regular::TAG, name),
                        egui::FontId::proportional(11.0),
                        text_color,
                    );
                    if resp.clicked() && !selected {
                        state.selected_category = Some(slug.clone());
                        state.page = 1;
                        should_search = true;
                    }
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                }
            });

        // ── Asset grid (right pane) ──
        let mut grid_child = ui.new_child(egui::UiBuilder::new().max_rect(grid_rect));
        egui::ScrollArea::vertical()
            .id_salt("hub_store_scroll")
            .auto_shrink([false, false])
            .show(&mut grid_child, |ui| {
                let state = self.state.read().unwrap();

                if state.loading {
                    ui.vertical_centered(|ui| {
                        ui.add_space(60.0);
                        ui.spinner();
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Loading assets...").size(12.0).color(text_muted),
                        );
                    });
                    return;
                }

                if state.assets.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(60.0);
                        ui.label(
                            RichText::new(egui_phosphor::regular::PACKAGE)
                                .size(40.0)
                                .color(text_muted),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("No assets found").size(13.0).color(text_muted),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("Try a different search or category")
                                .size(11.0)
                                .color(text_muted),
                        );
                    });
                    return;
                }

                // Grid layout — centered
                let available_width = ui.available_width();
                let cols = ((available_width + CARD_SPACING) / (CARD_WIDTH + CARD_SPACING))
                    .floor()
                    .max(1.0) as usize;
                let total_grid_width =
                    cols as f32 * CARD_WIDTH + (cols.saturating_sub(1)) as f32 * CARD_SPACING;
                let left_pad = ((available_width - total_grid_width) / 2.0).max(0.0);

                let assets_snapshot: Vec<_> = state
                    .assets
                    .iter()
                    .map(|a| {
                        (
                            a.slug.clone(),
                            a.name.clone(),
                            a.category.clone(),
                            a.price_credits,
                            a.thumbnail_url.clone(),
                        )
                    })
                    .collect();
                let page = state.page;
                let total = state.total;
                let per_page = state.per_page;
                drop(state);

                for row in assets_snapshot.chunks(cols) {
                    ui.horizontal(|ui| {
                        ui.add_space(left_pad);
                        ui.spacing_mut().item_spacing.x = CARD_SPACING;

                        for (slug, name, category, price, thumb_url) in row {
                            let (rect, response) = ui.allocate_exact_size(
                                Vec2::new(CARD_WIDTH, CARD_HEIGHT),
                                Sense::click(),
                            );

                            let hovered = response.hovered();

                            // Card background
                            let card_bg = if hovered { brighten(surface, 10) } else { surface };
                            let card_border = if hovered { accent } else { border };
                            ui.painter().rect(
                                rect,
                                CornerRadius::same(CARD_RADIUS as u8),
                                card_bg,
                                egui::Stroke::new(
                                    if hovered { 1.5 } else { 1.0 },
                                    card_border,
                                ),
                                StrokeKind::Outside,
                            );

                            // Square thumbnail
                            let thumb_rect = egui::Rect::from_min_size(
                                rect.min,
                                Vec2::new(CARD_WIDTH, THUMB_HEIGHT),
                            );
                            let thumb_radius = CornerRadius {
                                nw: CARD_RADIUS as u8,
                                ne: CARD_RADIUS as u8,
                                sw: 0,
                                se: 0,
                            };

                            let mut drew_image = false;
                            if let Some(url) = thumb_url {
                                if let Ok(mut images) = self.images.lock() {
                                    if let Some(handle) = images.get(url) {
                                        ui.painter().rect_filled(
                                            thumb_rect,
                                            thumb_radius,
                                            Color32::TRANSPARENT,
                                        );
                                        ui.painter().image(
                                            handle.id(),
                                            thumb_rect,
                                            egui::Rect::from_min_max(
                                                egui::pos2(0.0, 0.0),
                                                egui::pos2(1.0, 1.0),
                                            ),
                                            Color32::WHITE,
                                        );
                                        drew_image = true;
                                    }
                                }
                            }

                            if !drew_image {
                                ui.painter().rect_filled(
                                    thumb_rect,
                                    thumb_radius,
                                    brighten(surface, 5),
                                );
                                ui.painter().text(
                                    thumb_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    egui_phosphor::regular::IMAGE,
                                    egui::FontId::proportional(22.0),
                                    darken(text_muted, 30),
                                );
                            }

                            // Content below thumbnail
                            let content_left = rect.left() + 6.0;
                            let content_right = rect.right() - 6.0;
                            let content_top = thumb_rect.bottom() + 4.0;

                            // Asset name
                            ui.painter().text(
                                egui::Pos2::new(content_left, content_top),
                                egui::Align2::LEFT_TOP,
                                truncate_str(name, 16),
                                egui::FontId::proportional(10.5),
                                text_primary,
                            );

                            // Bottom row: category + price
                            let bottom_y = rect.bottom() - 16.0;

                            // Category pill
                            let pill_text = truncate_str(category, 8);
                            let pill_galley = ui.painter().layout_no_wrap(
                                pill_text.to_string(),
                                egui::FontId::proportional(8.5),
                                text_secondary,
                            );
                            let pill_w = pill_galley.rect.width() + 6.0;
                            let pill_rect = egui::Rect::from_min_size(
                                egui::Pos2::new(content_left, bottom_y),
                                Vec2::new(pill_w, 14.0),
                            );
                            ui.painter().rect_filled(
                                pill_rect,
                                CornerRadius::same(2),
                                brighten(surface, 16),
                            );
                            ui.painter().text(
                                pill_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                pill_text,
                                egui::FontId::proportional(8.5),
                                text_secondary,
                            );

                            // Price
                            let (price_text, price_color) = if *price == 0 {
                                ("Free".to_string(), Color32::from_rgb(34, 197, 94))
                            } else {
                                (format!("{} credits", price), accent)
                            };
                            ui.painter().text(
                                egui::Pos2::new(content_right, bottom_y + 7.0),
                                egui::Align2::RIGHT_CENTER,
                                &price_text,
                                egui::FontId::proportional(9.0),
                                price_color,
                            );

                            if hovered {
                                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                            }
                            if response.clicked() {
                                navigate_to = Some(slug.clone());
                            }
                        }
                    });
                    ui.add_space(CARD_SPACING);
                }

                // ── Pagination ──
                let total_pages = ((total as f32) / (per_page.max(1) as f32)).ceil() as u32;
                if total_pages > 1 {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let pad =
                            ((ui.available_width() - 200.0) / 2.0).max(0.0);
                        ui.add_space(pad);

                        if page > 1 {
                            if ui
                                .button(
                                    RichText::new(format!(
                                        "{} Prev",
                                        egui_phosphor::regular::CARET_LEFT
                                    ))
                                    .size(11.0),
                                )
                                .clicked()
                            {
                                page_change = Some(page - 1);
                            }
                        }
                        ui.label(
                            RichText::new(format!("{} / {}", page, total_pages))
                                .size(11.0)
                                .color(text_secondary),
                        );
                        if page < total_pages {
                            if ui
                                .button(
                                    RichText::new(format!(
                                        "Next {}",
                                        egui_phosphor::regular::CARET_RIGHT
                                    ))
                                    .size(11.0),
                                )
                                .clicked()
                            {
                                page_change = Some(page + 1);
                            }
                        }
                    });
                    ui.add_space(8.0);
                }
            });

        // ── Apply deferred actions ──
        if should_search {
            #[cfg(not(target_arch = "wasm32"))]
            self.fetch_assets();
        }
        if let Some(slug) = navigate_to {
            self.overlay.open(&slug, Some(session));
        }
        if let Some(page) = page_change {
            self.state.write().unwrap().page = page;
            #[cfg(not(target_arch = "wasm32"))]
            self.fetch_assets();
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

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

fn darken(c: Color32, delta: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(
        c.r().saturating_sub(delta),
        c.g().saturating_sub(delta),
        c.b().saturating_sub(delta),
        c.a(),
    )
}
