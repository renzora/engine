//! Asset detail overlay — full-screen modal for viewing, purchasing, rating,
//! and commenting on marketplace assets. Mirrors the website's asset page.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, RwLock};

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, RichText, Sense, Vec2};
use renzora_auth::marketplace::{AssetComment, AssetDetail, AssetRating, CommentsResponse};
use renzora_auth::session::AuthSession;
use renzora_editor_framework::EditorCommands;


use crate::images::ImageCache;
use crate::install;
use crate::preview::{HubPreviewImage, HubShaderRequest};

/// Default fragment shader used to preview shader assets (simple lit gradient).
const DEFAULT_PREVIEW_SHADER: &str = r#"
#import bevy_pbr::forward_io::VertexOutput

struct ShaderUniforms {
    time: f32,
    delta_time: f32,
    resolution: vec2<f32>,
    mouse: vec4<f32>,
    frame: u32,
    _pad: vec3<f32>,
}

@group(3) @binding(0) var<uniform> uniforms: ShaderUniforms;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.8));
    let diffuse = max(dot(n, light_dir), 0.0);
    let ambient = 0.15;
    let t = uniforms.time * 0.3;
    let base = vec3<f32>(0.35 + 0.15 * sin(t), 0.45 + 0.1 * cos(t * 0.7), 0.7);
    let color = base * (ambient + diffuse * 0.85);
    return vec4<f32>(color, 1.0);
}
"#;

// ── Audio preview thread (native only) ──────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
enum AudioCmd {
    Play(Vec<u8>),
    Stop,
}

#[cfg(not(target_arch = "wasm32"))]
fn audio_preview_thread(rx: mpsc::Receiver<AudioCmd>, playing: Arc<AtomicBool>) {
    use kira::sound::static_sound::StaticSoundData;

    let Ok(mut manager) =
        kira::AudioManager::<kira::DefaultBackend>::new(kira::AudioManagerSettings::default())
    else {
        return;
    };

    let mut current: Option<kira::sound::static_sound::StaticSoundHandle> = None;

    while let Ok(cmd) = rx.recv() {
        match cmd {
            AudioCmd::Play(data) => {
                if let Some(ref mut h) = current {
                    let _ = h.stop(kira::Tween::default());
                }
                current = None;
                playing.store(false, Ordering::SeqCst);

                match StaticSoundData::from_cursor(std::io::Cursor::new(data)) {
                    Ok(sound) => {
                        if let Ok(handle) = manager.play(sound) {
                            current = Some(handle);
                            playing.store(true, Ordering::SeqCst);
                        }
                    }
                    Err(_) => {}
                }
            }
            AudioCmd::Stop => {
                if let Some(ref mut h) = current {
                    let _ = h.stop(kira::Tween::default());
                }
                current = None;
                playing.store(false, Ordering::SeqCst);
            }
        }
    }
}

// ── Background result types ─────────────────────────────────────────────────

enum OverlayResult {
    Detail(Result<AssetDetail, String>),
    Purchase(Result<String, String>),
    Download(Result<DownloadComplete, String>),
    Comments(Result<CommentsResponse, String>),
    Rating(Result<AssetRating, String>),
    PostComment(Result<AssetComment, String>),
    PostRating(Result<AssetRating, String>),
    AudioData(Result<Vec<u8>, String>),
    ShaderSource(Result<String, String>),
}

#[allow(dead_code)]
struct DownloadComplete {
    asset_name: String,
    category: String,
    file_url: String,
    data: Vec<u8>,
}

// ── Overlay state ───────────────────────────────────────────────────────────

struct OverlayState {
    open: bool,
    slug: String,

    detail: Option<AssetDetail>,
    loading: bool,
    error: Option<String>,
    status: Option<String>,
    installing: bool,

    comments: Vec<AssetComment>,
    comment_draft: String,
    posting_comment: bool,

    rating: Option<AssetRating>,
    hover_star: Option<i32>,

    audio_loading: bool,
    confirm_purchase: bool,
    shader_requested: bool,
    shader_source: Option<String>,
}

impl Default for OverlayState {
    fn default() -> Self {
        Self {
            open: false,
            slug: String::new(),
            detail: None,
            loading: false,
            error: None,
            status: None,
            installing: false,
            comments: Vec::new(),
            comment_draft: String::new(),
            posting_comment: false,
            rating: None,
            hover_star: None,
            audio_loading: false,
            confirm_purchase: false,
            shader_requested: false,
            shader_source: None,
        }
    }
}

// ── Overlay manager ─────────────────────────────────────────────────────────

pub struct AssetOverlay {
    state: RwLock<OverlayState>,
    result_tx: mpsc::Sender<OverlayResult>,
    result_rx: Mutex<mpsc::Receiver<OverlayResult>>,
    images: Mutex<ImageCache>,
    #[cfg(not(target_arch = "wasm32"))]
    audio_cmd_tx: mpsc::Sender<AudioCmd>,
    audio_playing: Arc<AtomicBool>,
}

impl Default for AssetOverlay {
    fn default() -> Self {
        let audio_playing = Arc::new(AtomicBool::new(false));
        let (result_tx, result_rx) = mpsc::channel();

        #[cfg(not(target_arch = "wasm32"))]
        let audio_cmd_tx = {
            let (tx, rx) = mpsc::channel();
            let playing = audio_playing.clone();
            std::thread::Builder::new()
                .name("hub-audio-preview".into())
                .spawn(move || audio_preview_thread(rx, playing))
                .ok();
            tx
        };

        Self {
            state: RwLock::new(OverlayState::default()),
            result_tx,
            result_rx: Mutex::new(result_rx),
            images: Mutex::new(ImageCache::default()),
            #[cfg(not(target_arch = "wasm32"))]
            audio_cmd_tx,
            audio_playing,
        }
    }
}

impl AssetOverlay {
    /// Open the overlay for a given asset slug.
    pub fn open(&self, slug: &str, session: Option<&AuthSession>) {
        let mut state = self.state.write().unwrap();
        state.open = true;
        state.slug = slug.to_string();
        state.detail = None;
        state.error = None;
        state.status = None;
        state.comments.clear();
        state.comment_draft.clear();
        state.rating = None;
        state.hover_star = None;
        state.loading = true;
        state.confirm_purchase = false;
        state.audio_loading = false;
        drop(state);

        self.stop_audio();

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.fetch_detail(slug);
            self.fetch_comments(slug);
            self.fetch_rating(slug, session);
        }
    }

    pub fn is_open(&self) -> bool {
        self.state.read().unwrap().open
    }

    fn stop_audio(&self) {
        self.audio_playing.store(false, Ordering::SeqCst);
        #[cfg(not(target_arch = "wasm32"))]
        let _ = self.audio_cmd_tx.send(AudioCmd::Stop);
    }

    fn close(&self) {
        self.stop_audio();
        let mut state = self.state.write().unwrap();
        state.open = false;
        state.detail = None;
        state.comments.clear();
    }

    // ── Poll ────────────────────────────────────────────────────────────

    fn poll(&self, ctx: &egui::Context) {
        if let Ok(rx) = self.result_rx.lock() {
            while let Ok(result) = rx.try_recv() {
                let mut state = self.state.write().unwrap();
                match result {
                    OverlayResult::Detail(Ok(detail)) => {
                        state.detail = Some(detail);
                        state.loading = false;
                    }
                    OverlayResult::Detail(Err(e)) => {
                        state.error = Some(e);
                        state.loading = false;
                    }
                    OverlayResult::Purchase(Ok(msg)) => {
                        state.status = Some(msg);
                        state.loading = false;
                        state.confirm_purchase = false;
                        if let Some(ref mut d) = state.detail {
                            d.owned = Some(true);
                        }
                    }
                    OverlayResult::Purchase(Err(e)) => {
                        state.error = Some(e);
                        state.loading = false;
                    }
                    OverlayResult::Download(Ok(dl)) => {
                        state.installing = false;
                        state.status = Some(format!("Installed \"{}\"", dl.asset_name));
                    }
                    OverlayResult::Download(Err(e)) => {
                        state.installing = false;
                        state.error = Some(e);
                    }
                    OverlayResult::Comments(Ok(resp)) => {
                        state.comments = resp.comments;
                    }
                    OverlayResult::Comments(Err(_)) => {}
                    OverlayResult::Rating(Ok(r)) => {
                        state.rating = Some(r);
                    }
                    OverlayResult::Rating(Err(_)) => {}
                    OverlayResult::PostComment(Ok(comment)) => {
                        state.posting_comment = false;
                        state.comment_draft.clear();
                        state.comments.insert(0, comment);
                    }
                    OverlayResult::PostComment(Err(e)) => {
                        state.posting_comment = false;
                        state.error = Some(e);
                    }
                    OverlayResult::PostRating(Ok(r)) => {
                        state.rating = Some(r);
                    }
                    OverlayResult::PostRating(Err(e)) => {
                        state.error = Some(e);
                    }
                    OverlayResult::AudioData(Ok(data)) => {
                        state.audio_loading = false;
                        #[cfg(not(target_arch = "wasm32"))]
                        let _ = self.audio_cmd_tx.send(AudioCmd::Play(data));
                    }
                    OverlayResult::AudioData(Err(e)) => {
                        state.audio_loading = false;
                        state.error = Some(format!("Audio: {e}"));
                    }
                    OverlayResult::ShaderSource(Ok(source)) => {
                        state.shader_source = Some(source);
                    }
                    OverlayResult::ShaderSource(Err(e)) => {
                        state.error = Some(format!("Shader: {e}"));
                    }
                }
            }
        }

        if let Ok(mut images) = self.images.lock() {
            images.poll(ctx);
        }
    }

    // ── Network helpers ─────────────────────────────────────────────────

    #[cfg(not(target_arch = "wasm32"))]
    fn fetch_detail(&self, slug: &str) {
        let slug = slug.to_string();
        let tx = self.result_tx.clone();
        std::thread::spawn(move || {
            let result = renzora_auth::marketplace::get_asset(&slug);
            let _ = tx.send(OverlayResult::Detail(result));
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn fetch_comments(&self, slug: &str) {
        let slug = slug.to_string();
        let tx = self.result_tx.clone();
        std::thread::spawn(move || {
            let result = renzora_auth::marketplace::get_comments(&slug);
            let _ = tx.send(OverlayResult::Comments(result));
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn fetch_rating(&self, slug: &str, session: Option<&AuthSession>) {
        let slug = slug.to_string();
        let session_clone = session.map(|s| AuthSession {
            user: s.user.clone(),
            access_token: s.access_token.clone(),
            refresh_token: s.refresh_token.clone(),
        });
        let tx = self.result_tx.clone();
        std::thread::spawn(move || {
            let result =
                renzora_auth::marketplace::get_rating(&slug, session_clone.as_ref());
            let _ = tx.send(OverlayResult::Rating(result));
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
        let tx = self.result_tx.clone();
        self.state.write().unwrap().loading = true;

        std::thread::spawn(move || {
            let result = renzora_auth::marketplace::purchase_asset(&session_clone, &asset_id);
            let _ = tx.send(OverlayResult::Purchase(result.map(|r| r.message)));
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
        let tx = self.result_tx.clone();
        self.state.write().unwrap().installing = true;

        std::thread::spawn(move || {
            let result = (|| {
                let dl_resp =
                    renzora_auth::marketplace::download_asset(&session_clone, &asset_id)
                        .map_err(|e| format!("Failed to get download URL: {e}"))?;
                let data = renzora_auth::marketplace::download_file(&dl_resp.download_url)
                    .map_err(|e| format!("Failed to download file: {e}"))?;

                install::install_asset_with_filename(
                    &project_path,
                    &category,
                    &asset_name,
                    &dl_resp.download_url,
                    &dl_resp.download_filename,
                    &data,
                )?;

                Ok(DownloadComplete {
                    asset_name,
                    category,
                    file_url: dl_resp.download_url,
                    data,
                })
            })();
            let _ = tx.send(OverlayResult::Download(result));
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn fetch_audio_preview(&self, session: &AuthSession, asset_id: &str) {
        let session_clone = AuthSession {
            user: session.user.clone(),
            access_token: session.access_token.clone(),
            refresh_token: session.refresh_token.clone(),
        };
        let asset_id = asset_id.to_string();
        let tx = self.result_tx.clone();
        self.state.write().unwrap().audio_loading = true;

        std::thread::spawn(move || {
            let result = (|| -> Result<Vec<u8>, String> {
                let dl_resp =
                    renzora_auth::marketplace::download_asset(&session_clone, &asset_id)
                        .map_err(|e| format!("{e}"))?;
                renzora_auth::marketplace::download_file(&dl_resp.download_url)
                    .map_err(|e| format!("{e}"))
            })();
            let _ = tx.send(OverlayResult::AudioData(result));
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn fetch_shader_source(&self, session: &AuthSession, asset_id: &str) {
        let session_clone = AuthSession {
            user: session.user.clone(),
            access_token: session.access_token.clone(),
            refresh_token: session.refresh_token.clone(),
        };
        let asset_id = asset_id.to_string();
        let tx = self.result_tx.clone();
        self.state.write().unwrap().shader_requested = true;

        std::thread::spawn(move || {
            let result = (|| -> Result<String, String> {
                let dl_resp =
                    renzora_auth::marketplace::download_asset(&session_clone, &asset_id)
                        .map_err(|e| format!("{e}"))?;
                let data = renzora_auth::marketplace::download_file(&dl_resp.download_url)
                    .map_err(|e| format!("{e}"))?;

                // Try as raw UTF-8 text first
                if let Ok(source) = String::from_utf8(data.clone()) {
                    return Ok(source);
                }

                // Try as zip archive — extract first shader file
                extract_shader_from_zip(&data)
            })();
            let _ = tx.send(OverlayResult::ShaderSource(result));
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn post_comment(&self, session: &AuthSession, slug: &str, content: &str) {
        let session_clone = AuthSession {
            user: session.user.clone(),
            access_token: session.access_token.clone(),
            refresh_token: session.refresh_token.clone(),
        };
        let slug = slug.to_string();
        let content = content.to_string();
        let tx = self.result_tx.clone();
        self.state.write().unwrap().posting_comment = true;

        std::thread::spawn(move || {
            let result =
                renzora_auth::marketplace::post_comment(&session_clone, &slug, &content);
            let _ = tx.send(OverlayResult::PostComment(result));
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn post_rating(&self, session: &AuthSession, slug: &str, rating: i32) {
        let session_clone = AuthSession {
            user: session.user.clone(),
            access_token: session.access_token.clone(),
            refresh_token: session.refresh_token.clone(),
        };
        let slug = slug.to_string();
        let tx = self.result_tx.clone();

        std::thread::spawn(move || {
            let result =
                renzora_auth::marketplace::post_rating(&session_clone, &slug, rating);
            let _ = tx.send(OverlayResult::PostRating(result));
        });
    }

    // ── Render ──────────────────────────────────────────────────────────

    pub fn show(&self, ctx: &egui::Context, world: &World) {
        self.poll(ctx);

        if !self.is_open() {
            return;
        }

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
        let bg = theme.surfaces.window.to_color32();

        // ── Read state snapshot ──
        let state = self.state.read().unwrap();
        let slug = state.slug.clone();
        let loading = state.loading;
        let detail = state.detail.clone();
        let error = state.error.clone();
        let status = state.status.clone();
        let installing = state.installing;
        let comments: Vec<_> = state.comments.clone();
        let comment_draft = state.comment_draft.clone();
        let posting_comment = state.posting_comment;
        let rating = state.rating.clone();
        let hover_star = state.hover_star;
        let audio_loading = state.audio_loading;
        let confirm_purchase = state.confirm_purchase;
        let shader_requested = state.shader_requested;
        let shader_source = state.shader_source.clone();
        drop(state);

        let audio_playing = self.audio_playing.load(Ordering::SeqCst);

        // Deferred actions
        let mut do_close = false;
        let mut do_purchase: Option<String> = None;
        let mut do_install: Option<(String, String, String)> = None;
        let mut do_audio_play: Option<String> = None;
        let mut do_audio_stop = false;
        let mut do_post_comment = false;
        let mut do_rate: Option<i32> = None;
        let mut do_confirm_show = false;
        let mut do_confirm_cancel = false;
        let mut new_comment_draft: Option<String> = None;
        let mut new_hover_star: Option<Option<i32>> = None;
        #[allow(unused_assignments)]
        let mut do_fetch_shader: Option<String> = None;

        // ── Dimmed background (painted at Middle layer, below Foreground modal) ──
        #[allow(deprecated)]
        let screen = ctx.screen_rect();
        let bg_layer =
            egui::LayerId::new(egui::Order::Middle, egui::Id::new("hub_overlay_dim"));
        ctx.layer_painter(bg_layer)
            .rect_filled(screen, 0.0, Color32::from_black_alpha(160));

        // ── Modal window ──
        let modal_w = (screen.width() * 0.75).min(900.0).max(500.0);
        let modal_h = (screen.height() * 0.85).min(700.0).max(400.0);

        egui::Window::new("##hub_asset_overlay")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([modal_w, modal_h])
            .frame(
                egui::Frame::new()
                    .fill(bg)
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(egui::Margin::same(0))
                    .stroke(egui::Stroke::new(1.0, surface)),
            )
            .show(ctx, |ui| {
                // ── Header bar ──
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(16, 10))
                    .fill(surface)
                    .corner_radius(CornerRadius {
                        nw: 10,
                        ne: 10,
                        sw: 0,
                        se: 0,
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let title = detail
                                .as_ref()
                                .map(|d| d.name.as_str())
                                .unwrap_or("Loading...");
                            ui.label(
                                RichText::new(title)
                                    .size(15.0)
                                    .strong()
                                    .color(text_primary),
                            );

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let close = ui.add(
                                        egui::Button::new(
                                            RichText::new(egui_phosphor::regular::X)
                                                .size(14.0)
                                                .color(text_muted),
                                        )
                                        .frame(false),
                                    );
                                    if close.clicked() {
                                        do_close = true;
                                    }
                                },
                            );
                        });
                    });

                // ── Body ──
                egui::ScrollArea::vertical()
                    .id_salt("hub_overlay_body")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        let pad = 16.0;

                        if loading && detail.is_none() {
                            ui.vertical_centered(|ui| {
                                ui.add_space(60.0);
                                ui.spinner();
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new("Loading...")
                                        .size(12.0)
                                        .color(text_muted),
                                );
                            });
                            return;
                        }

                        let Some(ref detail) = detail else {
                            ui.vertical_centered(|ui| {
                                ui.add_space(60.0);
                                ui.label(
                                    RichText::new("Asset not found")
                                        .size(13.0)
                                        .color(text_muted),
                                );
                            });
                            return;
                        };

                        let is_audio = is_audio_asset(&detail.category);
                        let is_shader = is_shader_asset(&detail.category);
                        let detail_owned = detail.owned.unwrap_or(false);

                        // Shader preview: download source if owned, compile & apply
                        if is_shader {
                            // Kick off download if not yet requested
                            if detail_owned && !shader_requested {
                                do_fetch_shader = Some(detail.id.clone());
                            }

                            // Apply shader to preview sphere:
                            // - Use downloaded source if available
                            // - Fall back to default preview shader
                            // Re-apply whenever shader_source changes (is Some)
                            let needs_apply = if let Some(req) = world.get_resource::<HubShaderRequest>() {
                                // Apply if not yet active, or if we just got real source
                                !req.active || shader_source.is_some()
                            } else {
                                false
                            };

                            if needs_apply {
                                let wgsl_to_use = shader_source.clone()
                                    .unwrap_or_else(|| DEFAULT_PREVIEW_SHADER.to_string());
                                if let Some(cmds) = world.get_resource::<EditorCommands>() {
                                    cmds.push(move |world: &mut World| {
                                        let final_wgsl = if let Some(registry) = world.get_resource::<renzora_shader::registry::ShaderBackendRegistry>() {
                                            let lang = renzora_shader::file::detect_language(&wgsl_to_use);
                                            match registry.transpile(lang, &wgsl_to_use) {
                                                Ok(compiled) => compiled,
                                                Err(_) => wgsl_to_use,
                                            }
                                        } else {
                                            wgsl_to_use
                                        };
                                        if let Some(mut req) = world.get_resource_mut::<HubShaderRequest>() {
                                            req.wgsl = Some(final_wgsl);
                                        }
                                    });
                                }
                                // Clear shader_source so we don't re-apply every frame
                                if shader_source.is_some() {
                                    self.state.write().unwrap().shader_source = None;
                                }
                            }
                        }

                        // ── Two-column layout: preview (left) + info (right) ──
                        ui.add_space(pad);

                        let avail = ui.available_width() - pad * 2.0;
                        let preview_w = (avail * 0.45).min(350.0);
                        let info_w = avail - preview_w - pad;

                        ui.horizontal(|ui| {
                            ui.add_space(pad);

                            // ── Left: Preview ──
                            ui.vertical(|ui| {
                                ui.set_width(preview_w);
                                let preview_h = preview_w;

                                let (rect, _) = ui.allocate_exact_size(
                                    Vec2::new(preview_w, preview_h),
                                    Sense::hover(),
                                );

                                // Dark background
                                ui.painter().rect_filled(
                                    rect,
                                    CornerRadius::same(8),
                                    brighten(surface, 3),
                                );

                                let mut drew = false;

                                // Shader preview from render-to-texture
                                if is_shader {
                                    if let Some(preview_img) =
                                        world.get_resource::<HubPreviewImage>()
                                    {
                                        if let Some(tex_id) = preview_img.texture_id {
                                            ui.painter().image(
                                                tex_id,
                                                rect,
                                                egui::Rect::from_min_max(
                                                    egui::pos2(0.0, 0.0),
                                                    egui::pos2(1.0, 1.0),
                                                ),
                                                Color32::WHITE,
                                            );
                                            drew = true;
                                        }
                                    }
                                }

                                // Image thumbnail (aspect-preserving)
                                if !drew {
                                    if let Some(ref url) = detail.thumbnail_url {
                                        if let Ok(mut images) = self.images.lock() {
                                            if let Some(handle) = images.get(url) {
                                                let tex_size = handle.size();
                                                let (iw, ih) = (
                                                    tex_size[0] as f32,
                                                    tex_size[1] as f32,
                                                );
                                                if iw > 0.0 && ih > 0.0 {
                                                    let aspect = iw / ih;
                                                    let container =
                                                        rect.width() / rect.height();
                                                    let (dw, dh) = if aspect > container
                                                    {
                                                        (
                                                            rect.width(),
                                                            rect.width() / aspect,
                                                        )
                                                    } else {
                                                        (
                                                            rect.height() * aspect,
                                                            rect.height(),
                                                        )
                                                    };
                                                    let draw_rect =
                                                        egui::Rect::from_center_size(
                                                            rect.center(),
                                                            Vec2::new(dw, dh),
                                                        );
                                                    ui.painter().image(
                                                        handle.id(),
                                                        draw_rect,
                                                        egui::Rect::from_min_max(
                                                            egui::pos2(0.0, 0.0),
                                                            egui::pos2(1.0, 1.0),
                                                        ),
                                                        Color32::WHITE,
                                                    );
                                                    drew = true;
                                                }
                                            }
                                        }
                                    }
                                }

                                if !drew {
                                    ui.painter().text(
                                        rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        egui_phosphor::regular::IMAGE,
                                        egui::FontId::proportional(36.0),
                                        text_muted,
                                    );
                                }

                                // Audio player (below preview)
                                if is_audio && detail_owned {
                                    ui.add_space(8.0);
                                    egui::Frame::new()
                                        .inner_margin(egui::Margin::symmetric(10, 8))
                                        .corner_radius(CornerRadius::same(6))
                                        .fill(brighten(surface, 6))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new(format!(
                                                        "{} Audio",
                                                        egui_phosphor::regular::WAVEFORM
                                                    ))
                                                    .size(10.5)
                                                    .color(text_secondary),
                                                );
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        if audio_loading {
                                                            ui.spinner();
                                                        } else if audio_playing {
                                                            if ui
                                                                .add(
                                                                    egui::Button::new(
                                                                        RichText::new(format!(
                                                                            "{} Stop",
                                                                            egui_phosphor::regular::STOP
                                                                        ))
                                                                        .size(10.0)
                                                                        .color(Color32::WHITE),
                                                                    )
                                                                    .fill(Color32::from_rgb(
                                                                        239, 68, 68,
                                                                    ))
                                                                    .corner_radius(
                                                                        CornerRadius::same(4),
                                                                    ),
                                                                )
                                                                .clicked()
                                                            {
                                                                do_audio_stop = true;
                                                            }
                                                        } else {
                                                            if ui
                                                                .add(
                                                                    egui::Button::new(
                                                                        RichText::new(format!(
                                                                            "{} Play",
                                                                            egui_phosphor::regular::PLAY
                                                                        ))
                                                                        .size(10.0)
                                                                        .color(Color32::WHITE),
                                                                    )
                                                                    .fill(accent)
                                                                    .corner_radius(
                                                                        CornerRadius::same(4),
                                                                    ),
                                                                )
                                                                .clicked()
                                                            {
                                                                do_audio_play =
                                                                    Some(detail.id.clone());
                                                            }
                                                        }
                                                    },
                                                );
                                            });
                                        });
                                }
                            });

                            ui.add_space(pad);

                            // ── Right: Info ──
                            ui.vertical(|ui| {
                                ui.set_width(info_w);

                                // Creator
                                if let Some(ref creator) = detail.creator_name {
                                    ui.label(
                                        RichText::new(format!("by {creator}"))
                                            .size(11.5)
                                            .color(text_muted),
                                    );
                                    ui.add_space(6.0);
                                }

                                // Metadata
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 10.0;

                                    egui::Frame::new()
                                        .inner_margin(egui::Margin::symmetric(6, 2))
                                        .corner_radius(CornerRadius::same(3))
                                        .fill(brighten(surface, 16))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new(format!(
                                                    "{} {}",
                                                    egui_phosphor::regular::TAG,
                                                    &detail.category
                                                ))
                                                .size(10.0)
                                                .color(text_secondary),
                                            );
                                        });

                                    ui.label(
                                        RichText::new(format!("v{}", &detail.version))
                                            .size(10.0)
                                            .color(text_muted),
                                    );
                                    ui.label(
                                        RichText::new(format!(
                                            "{} {}",
                                            egui_phosphor::regular::DOWNLOAD_SIMPLE,
                                            format_count(detail.downloads)
                                        ))
                                        .size(10.0)
                                        .color(text_muted),
                                    );
                                });

                                ui.add_space(6.0);

                                let install_dir =
                                    install::install_dir_for_category(&detail.category);
                                ui.label(
                                    RichText::new(format!(
                                        "{} Installs to: {}/",
                                        egui_phosphor::regular::FOLDER_OPEN,
                                        install_dir
                                    ))
                                    .size(10.0)
                                    .color(text_muted),
                                );

                                ui.add_space(10.0);

                                // Description
                                ui.label(
                                    RichText::new(&detail.description)
                                        .size(11.5)
                                        .color(text_secondary),
                                );

                                ui.add_space(16.0);

                                // ── Actions ──
                                if detail_owned {
                                    let install_text = if installing {
                                        format!(
                                            "{} Installing...",
                                            egui_phosphor::regular::SPINNER
                                        )
                                    } else {
                                        format!(
                                            "{} Install to Project",
                                            egui_phosphor::regular::DOWNLOAD_SIMPLE
                                        )
                                    };
                                    let btn = ui.add_sized(
                                        [160.0, 28.0],
                                        egui::Button::new(
                                            RichText::new(&install_text)
                                                .color(Color32::WHITE)
                                                .size(11.5),
                                        )
                                        .fill(Color32::from_rgb(34, 197, 94))
                                        .corner_radius(CornerRadius::same(4)),
                                    );
                                    if btn.clicked() && !installing {
                                        do_install = Some((
                                            detail.id.clone(),
                                            detail.name.clone(),
                                            detail.category.clone(),
                                        ));
                                    }
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(format!(
                                            "{} Owned",
                                            egui_phosphor::regular::CHECK_CIRCLE
                                        ))
                                        .size(10.5)
                                        .color(Color32::from_rgb(34, 197, 94)),
                                    );
                                } else if !session.is_signed_in() {
                                    ui.label(
                                        RichText::new(format!(
                                            "{} Sign in to purchase",
                                            egui_phosphor::regular::SIGN_IN
                                        ))
                                        .size(11.0)
                                        .color(text_muted),
                                    );
                                } else if confirm_purchase {
                                    // Inline confirmation
                                    let price_msg = if detail.price_credits == 0 {
                                        "Get this asset for free?".to_string()
                                    } else {
                                        format!(
                                            "Purchase for {} credits?",
                                            detail.price_credits
                                        )
                                    };
                                    ui.label(
                                        RichText::new(&price_msg)
                                            .size(11.5)
                                            .color(text_primary),
                                    );
                                    ui.add_space(6.0);
                                    ui.horizontal(|ui| {
                                        let confirm_label = if detail.price_credits == 0 {
                                            format!(
                                                "{} Get for Free",
                                                egui_phosphor::regular::GIFT
                                            )
                                        } else {
                                            format!(
                                                "{} Confirm",
                                                egui_phosphor::regular::CHECK
                                            )
                                        };
                                        if ui
                                            .add_sized(
                                                [120.0, 28.0],
                                                egui::Button::new(
                                                    RichText::new(&confirm_label)
                                                        .color(Color32::WHITE)
                                                        .size(11.0),
                                                )
                                                .fill(accent)
                                                .corner_radius(CornerRadius::same(4)),
                                            )
                                            .clicked()
                                        {
                                            do_purchase = Some(detail.id.clone());
                                        }
                                        if ui
                                            .add_sized(
                                                [70.0, 28.0],
                                                egui::Button::new(
                                                    RichText::new("Cancel").size(11.0),
                                                )
                                                .corner_radius(CornerRadius::same(4)),
                                            )
                                            .clicked()
                                        {
                                            do_confirm_cancel = true;
                                        }
                                    });
                                } else {
                                    let price_label = if detail.price_credits == 0 {
                                        format!(
                                            "{} Get for Free",
                                            egui_phosphor::regular::GIFT
                                        )
                                    } else {
                                        format!(
                                            "Buy for {} credits",
                                            detail.price_credits
                                        )
                                    };
                                    let btn = ui.add_sized(
                                        [160.0, 28.0],
                                        egui::Button::new(
                                            RichText::new(&price_label)
                                                .color(Color32::WHITE)
                                                .size(11.5),
                                        )
                                        .fill(accent)
                                        .corner_radius(CornerRadius::same(4)),
                                    );
                                    if btn.clicked() {
                                        do_confirm_show = true;
                                    }
                                    if btn.hovered() {
                                        ui.ctx()
                                            .set_cursor_icon(CursorIcon::PointingHand);
                                    }
                                }

                                // Error / status
                                if let Some(ref err) = error {
                                    ui.add_space(6.0);
                                    ui.label(
                                        RichText::new(format!(
                                            "{} {}",
                                            egui_phosphor::regular::WARNING,
                                            err
                                        ))
                                        .size(10.5)
                                        .color(Color32::from_rgb(239, 68, 68)),
                                    );
                                }
                                if let Some(ref msg) = status {
                                    ui.add_space(6.0);
                                    ui.label(
                                        RichText::new(format!(
                                            "{} {}",
                                            egui_phosphor::regular::CHECK_CIRCLE,
                                            msg
                                        ))
                                        .size(10.5)
                                        .color(Color32::from_rgb(34, 197, 94)),
                                    );
                                }
                            });
                        });

                        ui.add_space(pad);
                        ui.separator();

                        // ── Rating section ──
                        ui.add_space(8.0);
                        egui::Frame::new()
                            .inner_margin(egui::Margin::symmetric(pad as i8, 4))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    // Average rating display
                                    if let Some(ref r) = rating {
                                        // Filled stars
                                        for i in 1..=5 {
                                            let filled = (i as f32) <= r.average;
                                            let half =
                                                !filled && (i as f32 - 0.5) <= r.average;
                                            let icon = if filled || half {
                                                egui_phosphor::regular::STAR
                                            } else {
                                                egui_phosphor::regular::STAR
                                            };
                                            let color = if filled {
                                                Color32::from_rgb(250, 204, 21) // yellow
                                            } else if half {
                                                Color32::from_rgb(250, 204, 21)
                                            } else {
                                                darken(text_muted, 20)
                                            };
                                            ui.label(
                                                RichText::new(icon)
                                                    .size(14.0)
                                                    .color(color),
                                            );
                                        }
                                        ui.label(
                                            RichText::new(format!(
                                                "{:.1} ({} ratings)",
                                                r.average, r.count
                                            ))
                                            .size(10.5)
                                            .color(text_muted),
                                        );
                                    } else {
                                        ui.label(
                                            RichText::new("No ratings yet")
                                                .size(10.5)
                                                .color(text_muted),
                                        );
                                    }

                                    // User rating input
                                    if session.is_signed_in() {
                                        ui.add_space(20.0);
                                        ui.label(
                                            RichText::new("Rate:")
                                                .size(10.5)
                                                .color(text_secondary),
                                        );

                                        let user_rating = rating
                                            .as_ref()
                                            .and_then(|r| r.user_rating)
                                            .unwrap_or(0);

                                        for i in 1..=5 {
                                            let is_hovered = hover_star
                                                .map(|h| i <= h)
                                                .unwrap_or(false);
                                            let is_filled = i <= user_rating;
                                            let color = if is_hovered {
                                                Color32::from_rgb(250, 204, 21)
                                            } else if is_filled {
                                                Color32::from_rgb(234, 179, 8)
                                            } else {
                                                darken(text_muted, 10)
                                            };

                                            let star = ui.add(
                                                egui::Button::new(
                                                    RichText::new(
                                                        egui_phosphor::regular::STAR,
                                                    )
                                                    .size(15.0)
                                                    .color(color),
                                                )
                                                .frame(false),
                                            );
                                            if star.hovered() {
                                                new_hover_star = Some(Some(i));
                                                ui.ctx().set_cursor_icon(
                                                    CursorIcon::PointingHand,
                                                );
                                            }
                                            if star.clicked() {
                                                do_rate = Some(i);
                                            }
                                        }

                                        // Clear hover when not on any star
                                        if new_hover_star.is_none()
                                            && hover_star.is_some()
                                        {
                                            // Check if pointer is outside all stars
                                            if !ui.ui_contains_pointer() {
                                                new_hover_star = Some(None);
                                            }
                                        }
                                    }
                                });
                            });

                        ui.separator();

                        // ── Comments section ──
                        ui.add_space(8.0);
                        egui::Frame::new()
                            .inner_margin(egui::Margin::symmetric(pad as i8, 0))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "{} Comments ({})",
                                        egui_phosphor::regular::CHAT_TEXT,
                                        comments.len()
                                    ))
                                    .size(12.0)
                                    .color(text_primary),
                                );

                                ui.add_space(8.0);

                                // Post comment
                                if session.is_signed_in() {
                                    ui.horizontal(|ui| {
                                        let mut draft = comment_draft.clone();
                                        let resp = ui.add(
                                            egui::TextEdit::singleline(&mut draft)
                                                .desired_width(ui.available_width() - 70.0)
                                                .hint_text("Write a comment..."),
                                        );
                                        if draft != comment_draft {
                                            new_comment_draft = Some(draft.clone());
                                        }

                                        let can_post =
                                            !draft.trim().is_empty() && !posting_comment;
                                        let post_btn = ui.add_enabled(
                                            can_post,
                                            egui::Button::new(
                                                RichText::new(if posting_comment {
                                                    "Posting..."
                                                } else {
                                                    "Post"
                                                })
                                                .size(11.0)
                                                .color(if can_post {
                                                    Color32::WHITE
                                                } else {
                                                    text_muted
                                                }),
                                            )
                                            .fill(if can_post {
                                                accent
                                            } else {
                                                brighten(surface, 10)
                                            })
                                            .corner_radius(CornerRadius::same(4)),
                                        );
                                        if post_btn.clicked()
                                            || (resp.lost_focus()
                                                && ui.input(|i| {
                                                    i.key_pressed(egui::Key::Enter)
                                                })
                                                && can_post)
                                        {
                                            do_post_comment = true;
                                        }
                                    });
                                    ui.add_space(8.0);
                                }

                                // Comments list
                                if comments.is_empty() {
                                    ui.label(
                                        RichText::new("No comments yet. Be the first!")
                                            .size(10.5)
                                            .color(text_muted),
                                    );
                                } else {
                                    for comment in &comments {
                                        egui::Frame::new()
                                            .inner_margin(egui::Margin::symmetric(10, 6))
                                            .corner_radius(CornerRadius::same(4))
                                            .fill(brighten(surface, 4))
                                            .show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        RichText::new(&comment.user_name)
                                                            .size(10.5)
                                                            .strong()
                                                            .color(text_primary),
                                                    );
                                                    ui.label(
                                                        RichText::new(&comment.created_at)
                                                            .size(9.5)
                                                            .color(text_muted),
                                                    );
                                                });
                                                ui.label(
                                                    RichText::new(&comment.content)
                                                        .size(11.0)
                                                        .color(text_secondary),
                                                );
                                            });
                                        ui.add_space(4.0);
                                    }
                                }
                            });

                        ui.add_space(pad);
                    });
            });

        // ── Apply deferred actions ──
        if do_close {
            // Deactivate shader preview
            if let Some(cmds) = world.get_resource::<EditorCommands>() {
                cmds.push(|world: &mut World| {
                    if let Some(mut req) = world.get_resource_mut::<HubShaderRequest>() {
                        req.clear = true;
                    }
                });
            }
            self.close();
        }
        if do_confirm_show {
            self.state.write().unwrap().confirm_purchase = true;
        }
        if do_confirm_cancel {
            self.state.write().unwrap().confirm_purchase = false;
        }
        if let Some(asset_id) = do_purchase {
            #[cfg(not(target_arch = "wasm32"))]
            self.purchase(&session, &asset_id);
        }
        if let Some((asset_id, asset_name, category)) = do_install {
            if let Some(pp) = project_path {
                #[cfg(not(target_arch = "wasm32"))]
                self.download_and_install(
                    &session,
                    &asset_id,
                    &asset_name,
                    &category,
                    pp.clone(),
                );
            }
        }
        if let Some(asset_id) = do_audio_play {
            #[cfg(not(target_arch = "wasm32"))]
            self.fetch_audio_preview(&session, &asset_id);
        }
        if do_audio_stop {
            self.stop_audio();
        }
        if let Some(asset_id) = do_fetch_shader {
            #[cfg(not(target_arch = "wasm32"))]
            self.fetch_shader_source(&session, &asset_id);
        }
        if let Some(draft) = new_comment_draft {
            self.state.write().unwrap().comment_draft = draft;
        }
        if do_post_comment {
            let draft = self.state.read().unwrap().comment_draft.clone();
            if !draft.trim().is_empty() {
                #[cfg(not(target_arch = "wasm32"))]
                self.post_comment(&session, &slug, &draft);
            }
        }
        if let Some(rating_val) = do_rate {
            #[cfg(not(target_arch = "wasm32"))]
            self.post_rating(&session, &slug, rating_val);
        }
        if let Some(hs) = new_hover_star {
            self.state.write().unwrap().hover_star = hs;
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn is_audio_asset(category: &str) -> bool {
    let lower = category.to_lowercase();
    lower.contains("audio") || lower.contains("sound") || lower.contains("music") || lower.contains("sfx")
}

fn is_shader_asset(category: &str) -> bool {
    let lower = category.to_lowercase();
    lower.contains("shader") || lower.contains("material")
}

fn format_count(n: i64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
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

/// Shader file extensions we look for inside zip archives.
const SHADER_EXTENSIONS: &[&str] = &[".wgsl", ".glsl", ".frag", ".vert", ".hlsl", ".shader"];

/// Try to extract the first shader source file from a zip archive.
#[cfg(not(target_arch = "wasm32"))]
fn extract_shader_from_zip(data: &[u8]) -> Result<String, String> {
    use std::io::Read;

    let cursor = std::io::Cursor::new(data);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("Not a valid zip/text file: {e}"))?;

    // Find the first shader source file
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {e}"))?;

        if entry.is_dir() {
            continue;
        }

        let name = entry.name().to_lowercase();
        let is_shader = SHADER_EXTENSIONS.iter().any(|ext| name.ends_with(ext));
        if !is_shader {
            continue;
        }

        let mut buf = Vec::new();
        entry
            .read_to_end(&mut buf)
            .map_err(|e| format!("Failed to read {}: {e}", entry.name()))?;

        return String::from_utf8(buf)
            .map_err(|_| format!("Shader file {} is not valid UTF-8", entry.name()));
    }

    Err("No shader file found in archive".to_string())
}
