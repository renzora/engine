//! Hub Library panel — view purchased/owned assets and install them.

use std::sync::{mpsc, Mutex, RwLock};

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, RichText, Sense, StrokeKind, Vec2};
use renzora_auth::marketplace::AssetSummary;
use renzora_auth::session::AuthSession;
use renzora_editor::{EditorPanel, PanelLocation};

use crate::images::ImageCache;
use crate::install;

// ── Background result types ──

enum LibraryResult {
    Assets(Result<Vec<AssetSummary>, String>),
    Install(Result<String, String>),
}

// ── Panel state ──

struct LibraryState {
    assets: Vec<AssetSummary>,
    filter: String,
    loading: bool,
    error: Option<String>,
    status: Option<String>,
    installing_id: Option<String>,
}

impl Default for LibraryState {
    fn default() -> Self {
        Self {
            assets: Vec::new(),
            filter: String::new(),
            loading: false,
            error: None,
            status: None,
            installing_id: None,
        }
    }
}

// ── Panel ──

pub struct HubLibraryPanel {
    state: RwLock<LibraryState>,
    receiver: Mutex<Option<mpsc::Receiver<LibraryResult>>>,
    needs_refresh: RwLock<bool>,
    images: Mutex<ImageCache>,
}

impl Default for HubLibraryPanel {
    fn default() -> Self {
        Self {
            state: RwLock::new(LibraryState::default()),
            receiver: Mutex::new(None),
            needs_refresh: RwLock::new(true),
            images: Mutex::new(ImageCache::default()),
        }
    }
}

impl HubLibraryPanel {
    fn poll_results(&self, ctx: &egui::Context) {
        let rx_guard = self.receiver.lock().ok();
        let result = rx_guard
            .as_ref()
            .and_then(|opt| opt.as_ref())
            .and_then(|rx| rx.try_recv().ok());

        if let Some(result) = result {
            let mut state = self.state.write().unwrap();
            match result {
                LibraryResult::Assets(Ok(assets)) => {
                    state.assets = assets;
                    state.loading = false;
                }
                LibraryResult::Assets(Err(e)) => {
                    state.error = Some(e);
                    state.loading = false;
                }
                LibraryResult::Install(Ok(msg)) => {
                    state.status = Some(msg);
                    state.installing_id = None;
                }
                LibraryResult::Install(Err(e)) => {
                    state.error = Some(e);
                    state.installing_id = None;
                }
            }
        }

        if let Ok(mut images) = self.images.lock() {
            images.poll(ctx);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn fetch_my_assets(&self, session: &AuthSession) {
        let session_clone = AuthSession {
            user: session.user.clone(),
            access_token: session.access_token.clone(),
            refresh_token: session.refresh_token.clone(),
        };
        let (tx, rx) = mpsc::channel();
        *self.receiver.lock().unwrap() = Some(rx);
        self.state.write().unwrap().loading = true;

        std::thread::spawn(move || {
            let result = renzora_auth::marketplace::get_my_assets(&session_clone)
                .map(|r| r.assets);
            let _ = tx.send(LibraryResult::Assets(result));
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn install_asset(
        &self,
        session: &AuthSession,
        asset: &AssetSummary,
        project_path: std::path::PathBuf,
    ) {
        let session_clone = AuthSession {
            user: session.user.clone(),
            access_token: session.access_token.clone(),
            refresh_token: session.refresh_token.clone(),
        };
        let asset_id = asset.id.clone();
        let asset_name = asset.name.clone();
        let category = asset.category.clone();
        self.state.write().unwrap().installing_id = Some(asset_id.clone());

        let (tx, rx) = mpsc::channel();
        *self.receiver.lock().unwrap() = Some(rx);

        std::thread::spawn(move || {
            let result = (|| {
                let dl_resp =
                    renzora_auth::marketplace::download_asset(&session_clone, &asset_id)?;
                let data = renzora_auth::marketplace::download_file(&dl_resp.download_url)?;
                install::install_asset_with_filename(
                    &project_path,
                    &category,
                    &asset_name,
                    &dl_resp.download_url,
                    &dl_resp.download_filename,
                    &data,
                )?;
                Ok(format!("Installed \"{}\"", asset_name))
            })();
            let _ = tx.send(LibraryResult::Install(result));
        });
    }
}

impl EditorPanel for HubLibraryPanel {
    fn id(&self) -> &str {
        "hub_library"
    }

    fn title(&self) -> &str {
        "My Library"
    }

    fn icon(&self) -> Option<&str> {
        Some(egui_phosphor::regular::BOOKS)
    }

    fn closable(&self) -> bool {
        true
    }

    fn min_size(&self) -> [f32; 2] {
        [250.0, 150.0]
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Left
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
        let project_path = world
            .get_resource::<renzora::core::CurrentProject>()
            .map(|p| p.path.clone());

        let accent = theme.semantic.accent.to_color32();
        let text_primary = theme.text.primary.to_color32();
        let text_secondary = theme.text.secondary.to_color32();
        let text_muted = theme.text.muted.to_color32();
        let surface = theme.surfaces.panel.to_color32();
        let border = theme.widgets.border.to_color32();

        if !session.is_signed_in() {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                ui.label(
                    RichText::new(egui_phosphor::regular::USER)
                        .size(36.0)
                        .color(text_muted),
                );
                ui.add_space(10.0);
                ui.label(
                    RichText::new("Sign in to view your library")
                        .size(13.0)
                        .color(text_muted),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Purchased assets will appear here")
                        .size(11.0)
                        .color(darken(text_muted, 20)),
                );
            });
            return;
        }

        // Auto-refresh when panel becomes visible
        {
            let mut refresh = self.needs_refresh.write().unwrap();
            if *refresh {
                *refresh = false;
                #[cfg(not(target_arch = "wasm32"))]
                self.fetch_my_assets(&session);
            }
        }

        let mut do_install: Option<AssetSummary> = None;
        let mut do_refresh = false;

        // ── Toolbar ──
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;

            let mut state = self.state.write().unwrap();
            ui.add(
                egui::TextEdit::singleline(&mut state.filter)
                    .desired_width(ui.available_width() - 34.0)
                    .hint_text(format!(
                        "{} Filter assets...",
                        egui_phosphor::regular::FUNNEL
                    )),
            );

            let refresh_btn = ui.button(
                RichText::new(egui_phosphor::regular::ARROW_CLOCKWISE).size(14.0),
            );
            if refresh_btn.clicked() {
                do_refresh = true;
            }
        });

        // Status/error
        {
            let mut state = self.state.write().unwrap();
            if let Some(err) = state.error.take() {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{} {}", egui_phosphor::regular::WARNING, err))
                            .size(10.5)
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
                        .size(10.5)
                        .color(Color32::from_rgb(34, 197, 94)),
                    );
                });
            }
        }

        ui.separator();

        // ── Asset list ──
        egui::ScrollArea::vertical()
            .id_salt("hub_library_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let state = self.state.read().unwrap();

                if state.loading {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.spinner();
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Loading library...")
                                .size(12.0)
                                .color(text_muted),
                        );
                    });
                    return;
                }

                if state.assets.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            RichText::new(egui_phosphor::regular::BOOKS)
                                .size(36.0)
                                .color(text_muted),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("No purchased assets yet")
                                .size(12.0)
                                .color(text_muted),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("Browse the Store to find assets")
                                .size(10.5)
                                .color(darken(text_muted, 20)),
                        );
                    });
                    return;
                }

                let filter_lower = state.filter.to_lowercase();

                // Snapshot assets
                let assets_snapshot: Vec<_> = state
                    .assets
                    .iter()
                    .filter(|a| {
                        filter_lower.is_empty()
                            || a.name.to_lowercase().contains(&filter_lower)
                            || a.category.to_lowercase().contains(&filter_lower)
                    })
                    .cloned()
                    .collect();
                let installing_id = state.installing_id.clone();
                drop(state); // Release read lock before image cache lock

                if assets_snapshot.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.label(
                            RichText::new("No matching assets")
                                .size(11.0)
                                .color(text_muted),
                        );
                    });
                    return;
                }

                let row_height = 62.0;

                for asset in &assets_snapshot {
                    let is_installing = installing_id.as_deref() == Some(&asset.id);
                    let install_dir = install::install_dir_for_category(&asset.category);

                    let (rect, resp) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), row_height),
                        Sense::hover(),
                    );

                    let hovered = resp.hovered();
                    let row_bg = if hovered { brighten(surface, 8) } else { surface };
                    let row_border = if hovered { accent } else { border };

                    ui.painter().rect(
                        rect,
                        CornerRadius::same(4),
                        row_bg,
                        egui::Stroke::new(if hovered { 1.0 } else { 0.5 }, row_border),
                        StrokeKind::Outside,
                    );

                    // Thumbnail (small square on the left)
                    let thumb_size = 44.0;
                    let thumb_rect = egui::Rect::from_min_size(
                        rect.min + egui::vec2(8.0, (row_height - thumb_size) / 2.0),
                        Vec2::splat(thumb_size),
                    );

                    let mut drew_image = false;
                    if let Some(ref url) = asset.thumbnail_url {
                        if let Ok(mut images) = self.images.lock() {
                            if let Some(handle) = images.get(url) {
                                ui.painter().rect_filled(
                                    thumb_rect,
                                    CornerRadius::same(3),
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
                            CornerRadius::same(3),
                            brighten(surface, 8),
                        );
                        ui.painter().text(
                            thumb_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            egui_phosphor::regular::PACKAGE,
                            egui::FontId::proportional(16.0),
                            text_muted,
                        );
                    }

                    // Content to the right of thumbnail
                    let text_left = thumb_rect.right() + 10.0;

                    // Name
                    ui.painter().text(
                        egui::Pos2::new(text_left, rect.top() + 10.0),
                        egui::Align2::LEFT_TOP,
                        truncate_str(&asset.name, 20),
                        egui::FontId::proportional(12.0),
                        text_primary,
                    );

                    // Category pill + install path
                    let meta_y = rect.top() + 28.0;

                    // Category pill
                    let pill_galley = ui.painter().layout_no_wrap(
                        asset.category.clone(),
                        egui::FontId::proportional(9.0),
                        text_secondary,
                    );
                    let pill_w = pill_galley.rect.width() + 8.0;
                    let pill_h = 14.0;
                    let pill_rect = egui::Rect::from_min_size(
                        egui::Pos2::new(text_left, meta_y),
                        Vec2::new(pill_w, pill_h),
                    );
                    ui.painter().rect_filled(
                        pill_rect,
                        CornerRadius::same(2),
                        brighten(surface, 16),
                    );
                    ui.painter().text(
                        pill_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        &asset.category,
                        egui::FontId::proportional(9.0),
                        text_secondary,
                    );

                    // Install path
                    ui.painter().text(
                        egui::Pos2::new(pill_rect.right() + 6.0, meta_y + 2.0),
                        egui::Align2::LEFT_TOP,
                        format!("{}/", install_dir),
                        egui::FontId::proportional(9.0),
                        darken(text_muted, 10),
                    );

                    // Install button (right side)
                    let btn_w = 68.0;
                    let btn_h = 24.0;
                    let btn_rect = egui::Rect::from_min_size(
                        egui::Pos2::new(
                            rect.right() - btn_w - 8.0,
                            rect.center().y - btn_h / 2.0,
                        ),
                        Vec2::new(btn_w, btn_h),
                    );
                    let btn_id = ui.id().with(("lib_install", &asset.id));
                    let btn_resp = ui.interact(btn_rect, btn_id, Sense::click());

                    let btn_bg = if is_installing {
                        text_muted
                    } else if btn_resp.hovered() {
                        Color32::from_rgb(28, 175, 80)
                    } else {
                        Color32::from_rgb(34, 197, 94)
                    };
                    ui.painter()
                        .rect_filled(btn_rect, CornerRadius::same(3), btn_bg);

                    let btn_text = if is_installing {
                        format!("{}", egui_phosphor::regular::SPINNER)
                    } else {
                        format!("{} Install", egui_phosphor::regular::DOWNLOAD_SIMPLE)
                    };
                    ui.painter().text(
                        btn_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        &btn_text,
                        egui::FontId::proportional(10.0),
                        Color32::WHITE,
                    );

                    if btn_resp.hovered() && !is_installing {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    if btn_resp.clicked() && !is_installing {
                        do_install = Some(asset.clone());
                    }

                    ui.add_space(3.0);
                }
            });

        // ── Apply deferred actions ──
        if do_refresh {
            #[cfg(not(target_arch = "wasm32"))]
            self.fetch_my_assets(&session);
        }
        if let Some(asset) = do_install {
            if let Some(pp) = project_path {
                #[cfg(not(target_arch = "wasm32"))]
                self.install_asset(&session, &asset, pp);
            }
        }
    }
}

// ── Helpers ──

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
