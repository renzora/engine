//! Hub Library panel — view purchased/owned assets and install them.

use std::sync::{mpsc, Mutex, RwLock};

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, RichText, Sense, StrokeKind, Vec2};
use renzora_auth::marketplace::AssetSummary;
use renzora_auth::session::AuthSession;
use renzora_editor::{EditorPanel, PanelLocation};
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
}

impl Default for HubLibraryPanel {
    fn default() -> Self {
        Self {
            state: RwLock::new(LibraryState::default()),
            receiver: Mutex::new(None),
            needs_refresh: RwLock::new(true),
        }
    }
}

impl HubLibraryPanel {
    fn poll_results(&self) {
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
                install::install_asset(
                    &project_path,
                    &category,
                    &asset_name,
                    &dl_resp.download_url,
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

        let _accent = theme.semantic.accent.to_color32();
        let _text_secondary = theme.text.secondary.to_color32();
        let text_muted = theme.text.muted.to_color32();

        if !session.is_signed_in() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    RichText::new(egui_phosphor::regular::USER)
                        .size(32.0)
                        .color(text_muted),
                );
                ui.add_space(8.0);
                ui.label(RichText::new("Sign in to view your library").color(text_muted));
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
        ui.horizontal(|ui| {
            let mut state = self.state.write().unwrap();
            ui.add(
                egui::TextEdit::singleline(&mut state.filter)
                    .desired_width(160.0)
                    .hint_text("Filter..."),
            );

            let btn = ui.button(RichText::new(egui_phosphor::regular::ARROW_CLOCKWISE).size(13.0));
            if btn.clicked() {
                do_refresh = true;
            }
        });

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

        // ── Asset list ──
        egui::ScrollArea::vertical()
            .id_salt("hub_library_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let state = self.state.read().unwrap();

                if state.loading {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.spinner();
                    });
                    return;
                }

                if state.assets.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.label(
                            RichText::new("No purchased assets yet")
                                .color(text_muted),
                        );
                    });
                    return;
                }

                let filter_lower = state.filter.to_lowercase();
                let panel_bg = theme.surfaces.panel.to_color32();
                let border = theme.widgets.border.to_color32();

                for asset in &state.assets {
                    // Apply filter
                    if !filter_lower.is_empty()
                        && !asset.name.to_lowercase().contains(&filter_lower)
                        && !asset.category.to_lowercase().contains(&filter_lower)
                    {
                        continue;
                    }

                    let install_dir = install::install_dir_for_category(&asset.category);
                    let is_installing = state.installing_id.as_deref() == Some(&asset.id);

                    let (rect, _resp) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), 52.0),
                        Sense::hover(),
                    );

                    ui.painter().rect(
                        rect,
                        CornerRadius::same(3),
                        panel_bg,
                        egui::Stroke::new(0.5, border),
                        StrokeKind::Outside,
                    );

                    // Name + category
                    ui.painter().text(
                        rect.min + egui::vec2(8.0, 8.0),
                        egui::Align2::LEFT_TOP,
                        &asset.name,
                        egui::FontId::proportional(12.0),
                        theme.text.primary.to_color32(),
                    );
                    ui.painter().text(
                        rect.min + egui::vec2(8.0, 24.0),
                        egui::Align2::LEFT_TOP,
                        format!("{} {} {}/", egui_phosphor::regular::FOLDER_OPEN, &asset.category, install_dir),
                        egui::FontId::proportional(10.0),
                        text_muted,
                    );

                    // Install button
                    let btn_rect = egui::Rect::from_min_size(
                        egui::Pos2::new(rect.right() - 78.0, rect.center().y - 11.0),
                        Vec2::new(70.0, 22.0),
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
                    ui.painter().rect_filled(btn_rect, CornerRadius::same(3), btn_bg);

                    let btn_text = if is_installing { "Installing" } else { "Install" };
                    ui.painter().text(
                        btn_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        btn_text,
                        egui::FontId::proportional(10.5),
                        Color32::WHITE,
                    );

                    if btn_resp.hovered() && !is_installing {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    if btn_resp.clicked() && !is_installing {
                        do_install = Some(asset.clone());
                    }

                    ui.add_space(2.0);
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
