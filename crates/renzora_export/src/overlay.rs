#![allow(deprecated)] // egui API rename pending; will migrate at next bevy_egui bump.

//! Export overlay UI — a modal dialog for configuring and running project exports.

use std::sync::{mpsc, Mutex};

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora::core::{CurrentProject, WindowMode};
use renzora_import::optimize::MeshOptSettings;
use renzora_rpak::{
    pack_project_filtered, pack_project_with_progress, RpakPacker, SERVER_EXTENSIONS,
};
use renzora_theme::ThemeManager;

use crate::download::{self, DownloadProgress, DownloadTask, ReleaseInfo};
use crate::templates::{Platform, TemplateManager};

/// Packaging mode for the exported build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackagingMode {
    /// Runtime binary + .rpak file side by side.
    SeparateFiles,
    /// .rpak appended to the binary — single executable.
    SingleBinary,
}

/// Export progress state.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportProgress {
    Idle,
    Working(String),
    Done(String),
    Error(String),
}

/// Messages sent from the background export thread.
enum ExportMsg {
    Progress(String),
    Done(String),
    Error(String),
}

/// Handle for a running background export.
struct ExportTask {
    rx: Mutex<mpsc::Receiver<ExportMsg>>,
}

/// Resource holding the export overlay state.
#[derive(Resource)]
pub struct ExportOverlayState {
    pub visible: bool,
    pub platform: Platform,
    pub packaging_mode: PackagingMode,
    pub window_mode: WindowMode,
    pub window_width: u32,
    pub window_height: u32,
    pub console_logging: bool,
    pub compression_level: i32,
    pub icon_path: Option<String>,
    pub include_server: bool,
    /// Optional override for the exported binary's filename (without extension).
    /// Empty = use the project name.
    pub binary_name: String,
    pub mesh_simplify: bool,
    pub mesh_simplify_ratio: f32,
    pub mesh_quantize: bool,
    pub mesh_generate_lods: bool,
    pub mesh_lod_levels: u32,
    pub output_dir: String,
    pub progress: ExportProgress,
    /// Background export task (if running).
    active_task: Option<ExportTask>,
    /// Available runtime-compatible plugins (scanned once).
    pub available_plugins: Vec<dynamic_plugin_loader::DynamicPluginInfo>,
    /// Which plugins are selected for export (by id).
    pub selected_plugins: std::collections::HashSet<String>,
    /// Whether plugins have been scanned yet.
    plugins_scanned: bool,
    /// Latest GitHub release info (for runtime downloads).
    pub release_info: Option<ReleaseInfo>,
    /// Background fetch of release manifest.
    release_fetch_rx: Option<Mutex<mpsc::Receiver<Result<ReleaseInfo, String>>>>,
    /// Whether release fetch has been kicked off.
    release_fetch_started: bool,
    /// Last error from release manifest fetch (if any).
    pub release_fetch_error: Option<String>,
    /// Active runtime download task.
    download_task: Option<DownloadTask>,
    /// Last download status (per platform shown in UI).
    pub download_status: Option<(Platform, DownloadProgress)>,
}

impl Default for ExportOverlayState {
    fn default() -> Self {
        Self {
            visible: false,
            platform: Platform::current().unwrap_or(Platform::WindowsX64),
            packaging_mode: PackagingMode::SeparateFiles,
            window_mode: WindowMode::Windowed,
            window_width: 1280,
            window_height: 720,
            console_logging: false,
            compression_level: 3,
            icon_path: None,
            include_server: false,
            binary_name: String::new(),
            mesh_simplify: false,
            mesh_simplify_ratio: 0.5,
            mesh_quantize: false,
            mesh_generate_lods: false,
            mesh_lod_levels: 3,
            output_dir: String::new(),
            progress: ExportProgress::Idle,
            active_task: None,
            available_plugins: Vec::new(),
            selected_plugins: std::collections::HashSet::new(),
            plugins_scanned: false,
            release_info: None,
            release_fetch_rx: None,
            release_fetch_started: false,
            release_fetch_error: None,
            download_task: None,
            download_status: None,
        }
    }
}

/// Drain progress messages from the background thread into overlay state.
fn poll_export_task(world: &mut World) {
    let has_task = world.resource::<ExportOverlayState>().active_task.is_some();
    if !has_task {
        return;
    }

    let mut finished = false;
    let mut updates: Vec<ExportMsg> = Vec::new();

    {
        let state = world.resource::<ExportOverlayState>();
        let task = state.active_task.as_ref().unwrap();
        let rx = task.rx.lock().unwrap();
        loop {
            match rx.try_recv() {
                Ok(msg) => {
                    let is_terminal = matches!(msg, ExportMsg::Done(_) | ExportMsg::Error(_));
                    updates.push(msg);
                    if is_terminal {
                        finished = true;
                        break;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    updates.push(ExportMsg::Error(
                        "Export thread terminated unexpectedly".into(),
                    ));
                    finished = true;
                    break;
                }
            }
        }
    }

    let mut state = world.resource_mut::<ExportOverlayState>();
    for msg in updates {
        match msg {
            ExportMsg::Progress(label) => {
                state.progress = ExportProgress::Working(label);
            }
            ExportMsg::Done(msg) => {
                state.progress = ExportProgress::Done(msg);
            }
            ExportMsg::Error(msg) => {
                state.progress = ExportProgress::Error(msg);
            }
        }
    }

    if finished {
        state.active_task = None;
    }
}

/// Kick off the GitHub release manifest fetch on first open.
// drop(state) ends the Mut<Resource> borrow early so `world` is free again;
// Mut isn't Drop so clippy flags it, but the lifetime-ending effect is intended.
#[allow(clippy::drop_non_drop)]
fn ensure_release_fetch(world: &mut World) {
    let mut state = world.resource_mut::<ExportOverlayState>();
    if state.release_fetch_started {
        return;
    }
    state.release_fetch_started = true;
    let (tx, rx) = mpsc::channel();
    state.release_fetch_rx = Some(Mutex::new(rx));
    drop(state);
    std::thread::spawn(move || {
        let _ = tx.send(download::fetch_release_info());
    });
}

/// Drain release manifest result if it has arrived.
fn poll_release_fetch(world: &mut World) {
    let mut state = world.resource_mut::<ExportOverlayState>();
    let Some(rx) = state.release_fetch_rx.as_ref() else {
        return;
    };
    let msg = rx.lock().ok().and_then(|rx| rx.try_recv().ok());
    if let Some(result) = msg {
        match result {
            Ok(info) => {
                state.release_info = Some(info);
                state.release_fetch_error = None;
            }
            Err(e) => {
                state.release_fetch_error = Some(e);
            }
        }
        state.release_fetch_rx = None;
    }
}

/// Drain progress messages from the runtime download thread.
fn poll_download_task(world: &mut World) {
    let has_task = world
        .resource::<ExportOverlayState>()
        .download_task
        .is_some();
    if !has_task {
        return;
    }

    let mut finished = false;
    let mut updates: Vec<DownloadProgress> = Vec::new();

    {
        let state = world.resource::<ExportOverlayState>();
        let task = state.download_task.as_ref().unwrap();
        let rx = task.rx.lock().unwrap();
        loop {
            match rx.try_recv() {
                Ok(msg) => {
                    let is_terminal =
                        matches!(msg, DownloadProgress::Done(_) | DownloadProgress::Error(_));
                    updates.push(msg);
                    if is_terminal {
                        finished = true;
                        break;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    updates.push(DownloadProgress::Error(
                        "Download thread terminated unexpectedly".into(),
                    ));
                    finished = true;
                    break;
                }
            }
        }
    }

    let platform = world
        .resource::<ExportOverlayState>()
        .download_task
        .as_ref()
        .map(|t| t.platform);

    {
        let mut state = world.resource_mut::<ExportOverlayState>();
        for msg in updates {
            if let Some(p) = platform {
                state.download_status = Some((p, msg));
            }
        }
        if finished {
            state.download_task = None;
        }
    }

    // After a download finishes, rescan templates so the newly installed
    // runtime gets picked up.
    if finished {
        world.resource_mut::<TemplateManager>().scan();
    }
}

// drop(export_state) ends the Mut<Resource> borrow early so `world` is free to
// re-borrow; Mut isn't Drop so clippy flags it, but the effect is intended.
#[allow(clippy::drop_non_drop)]
pub fn draw_export_overlay(world: &mut World, ctx: &egui::Context) {
    // Poll background tasks every frame
    poll_export_task(world);
    ensure_release_fetch(world);
    poll_release_fetch(world);
    poll_download_task(world);

    // Dim background
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("export_overlay_bg"),
    ));
    painter.rect_filled(screen, 0.0, egui::Color32::from_black_alpha(160));

    // Read theme
    let (panel_bg, text_primary, text_secondary, accent, error_color, border_color, surface_mid) = {
        let theme_mgr = world.resource::<ThemeManager>();
        let t = &theme_mgr.active_theme;
        (
            t.surfaces.panel.0,
            t.text.primary.0,
            t.text.secondary.0,
            t.semantic.accent.0,
            t.semantic.error.0,
            t.widgets.border.0,
            t.surfaces.faint.0,
        )
    };

    let has_project = world.get_resource::<CurrentProject>().is_some();

    let window_width = 760.0;
    let sidebar_width = 180.0;
    let window_height = (screen.height() * 0.85).clamp(520.0, 820.0);
    let window_id = egui::Id::new("export_overlay_window");

    egui::Area::new(window_id)
        .fixed_pos(egui::pos2(
            (screen.width() - window_width) / 2.0,
            (screen.height() - window_height) / 2.0,
        ))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let frame = egui::Frame::new()
                .fill(panel_bg)
                .stroke(egui::Stroke::new(1.0, border_color))
                .corner_radius(8.0)
                .inner_margin(egui::Margin::same(20));

            frame.show(ui, |ui| {
                ui.set_width(window_width);
                ui.set_height(window_height - 40.0);

                // Header
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{} Export Project", regular::PACKAGE))
                            .size(18.0)
                            .color(text_primary),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(regular::X)
                                        .size(16.0)
                                        .color(text_secondary),
                                )
                                .frame(false),
                            )
                            .clicked()
                        {
                            let mut s = world.resource_mut::<ExportOverlayState>();
                            s.visible = false;
                            s.active_task = None;
                        }
                    });
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(8.0);

                if !has_project {
                    ui.label(
                        egui::RichText::new("No project open. Open a project before exporting.")
                            .color(error_color),
                    );
                    return;
                }

                let project_name = world.resource::<CurrentProject>().config.name.clone();

                // ===== Two-column layout: platform sidebar + per-platform settings =====
                let selected_platform = world.resource::<ExportOverlayState>().platform;
                let is_desktop = matches!(
                    selected_platform,
                    Platform::WindowsX64
                        | Platform::LinuxX64
                        | Platform::MacOSX64
                        | Platform::MacOSArm64
                );
                let template_installed = world
                    .resource::<TemplateManager>()
                    .is_installed(selected_platform);

                // Reserve space at the bottom for Output + Progress + Export button.
                let footer_reserved = 120.0;
                let content_height = (ui.available_height() - footer_reserved).max(200.0);
                let right_pane_width = window_width - sidebar_width - 32.0;

                ui.allocate_ui_with_layout(
                    egui::vec2(window_width, content_height),
                    egui::Layout::left_to_right(egui::Align::Min),
                    |ui| {
                        // ----- LEFT RAIL: platform list -----
                        ui.allocate_ui_with_layout(
                            egui::vec2(sidebar_width, content_height),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.set_width(sidebar_width);

                                section_label(ui, regular::DESKTOP_TOWER, "Platform", text_primary);
                                ui.add_space(6.0);

                                // Snapshot release info / current selection so we can
                                // mutate ExportOverlayState below without holding refs
                                // across iterations.
                                let release_info =
                                    world.resource::<ExportOverlayState>().release_info.clone();
                                let release_fetching = world
                                    .resource::<ExportOverlayState>()
                                    .release_fetch_rx
                                    .is_some();
                                let release_error = world
                                    .resource::<ExportOverlayState>()
                                    .release_fetch_error
                                    .clone();
                                let current = world.resource::<ExportOverlayState>().platform;

                                for p in Platform::ALL {
                                    let installed =
                                        world.resource::<TemplateManager>().is_installed(*p);
                                    let available = release_info
                                        .as_ref()
                                        .map(|i| i.available_platforms.contains(p))
                                        .unwrap_or(false);
                                    let selected = current == *p;
                                    let resp = platform_sidebar_button(
                                        ui,
                                        *p,
                                        selected,
                                        installed,
                                        available,
                                        accent,
                                        text_primary,
                                        text_secondary,
                                        surface_mid,
                                        border_color,
                                    );
                                    if resp.clicked() && !selected {
                                        world.resource_mut::<ExportOverlayState>().platform = *p;
                                    }
                                }

                                ui.add_space(8.0);

                                if release_fetching {
                                    ui.horizontal(|ui| {
                                        ui.spinner();
                                        ui.label(
                                            egui::RichText::new("Loading release...")
                                                .size(11.0)
                                                .color(text_secondary),
                                        );
                                    });
                                } else if let Some(info) = &release_info {
                                    ui.label(
                                        egui::RichText::new(format!("Latest: {}", info.tag_name))
                                            .size(11.0)
                                            .color(text_secondary),
                                    );
                                } else if let Some(err) = &release_error {
                                    ui.label(
                                        egui::RichText::new(err.as_str())
                                            .size(10.0)
                                            .color(error_color),
                                    );
                                }
                            },
                        );

                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(8.0);

                        // ----- RIGHT PANE: settings for selected platform -----
                        ui.allocate_ui_with_layout(
                            egui::vec2(right_pane_width, content_height),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                // Platform header
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(platform_icon(selected_platform))
                                            .size(20.0)
                                            .color(text_primary),
                                    );
                                    ui.add_space(4.0);
                                    ui.vertical(|ui| {
                                        ui.label(
                                            egui::RichText::new(selected_platform.display_name())
                                                .size(15.0)
                                                .color(text_primary)
                                                .strong(),
                                        );
                                        ui.label(
                                            egui::RichText::new(
                                                selected_platform.supported_devices(),
                                            )
                                            .size(11.0)
                                            .color(text_secondary),
                                        );
                                    });
                                });

                                ui.add_space(8.0);

                                // Runtime template status + install/download buttons
                                draw_runtime_status(
                                    ui,
                                    world,
                                    selected_platform,
                                    template_installed,
                                    accent,
                                    text_primary,
                                    text_secondary,
                                    surface_mid,
                                    error_color,
                                );

                                ui.add_space(12.0);

                                egui::ScrollArea::vertical()
                                    .id_salt("export_settings_scroll")
                                    .max_height(420.0)
                                    .auto_shrink([false, true])
                                    .show(ui, |ui| {
                                        draw_settings_panel(
                                            ui,
                                            world,
                                            selected_platform,
                                            is_desktop,
                                            text_primary,
                                            text_secondary,
                                            surface_mid,
                                        );
                                    });
                            },
                        );
                    },
                );

                ui.add_space(12.0);

                // --- Output ---
                section_label(ui, regular::FOLDER_OPEN, "Output", text_primary);
                ui.add_space(4.0);

                let mut export_state = world.resource_mut::<ExportOverlayState>();

                let binary_hint = format!("Binary name (default: {})", project_name);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Name:")
                            .size(12.0)
                            .color(text_secondary),
                    );
                    let text_edit = egui::TextEdit::singleline(&mut export_state.binary_name)
                        .hint_text(binary_hint)
                        .desired_width(ui.available_width());
                    ui.add(text_edit);
                });

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    let text_edit = egui::TextEdit::singleline(&mut export_state.output_dir)
                        .hint_text("Export directory...")
                        .desired_width(ui.available_width() - 80.0);
                    ui.add(text_edit);

                    let output_dir_mut = &mut export_state.output_dir;
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(format!("{} Browse", regular::FOLDER))
                                    .size(11.0),
                            )
                            .fill(surface_mid),
                        )
                        .clicked()
                    {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Select output directory")
                            .pick_folder()
                        {
                            *output_dir_mut = path.to_string_lossy().to_string();
                        }
                    }
                });

                let progress = export_state.progress.clone();
                let can_export = template_installed
                    && !export_state.output_dir.is_empty()
                    && export_state.active_task.is_none()
                    && matches!(
                        progress,
                        ExportProgress::Idle | ExportProgress::Done(_) | ExportProgress::Error(_)
                    );

                drop(export_state);

                ui.add_space(16.0);

                // --- Progress / status ---
                match &progress {
                    ExportProgress::Idle => {}
                    ExportProgress::Working(label) => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(egui::RichText::new(label).color(text_secondary));
                        });
                    }
                    ExportProgress::Done(msg) => {
                        ui.label(
                            egui::RichText::new(format!("{} {}", regular::CHECK_CIRCLE, msg))
                                .color(egui::Color32::from_rgb(89, 191, 115)),
                        );
                    }
                    ExportProgress::Error(msg) => {
                        ui.label(
                            egui::RichText::new(format!("{} {}", regular::WARNING, msg))
                                .color(error_color),
                        );
                    }
                }

                ui.add_space(8.0);

                // --- Export button ---
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let button = egui::Button::new(
                            egui::RichText::new(format!("{} Export", regular::ROCKET_LAUNCH))
                                .size(14.0)
                                .color(egui::Color32::WHITE),
                        )
                        .fill(if can_export { accent } else { surface_mid })
                        .min_size(egui::vec2(100.0, 32.0));

                        if ui.add_enabled(can_export, button).clicked() {
                            run_export(world, &project_name);
                        }
                    });
                });
            });
        });
}

/// Phosphor icon character for a platform's sidebar entry.
fn platform_icon(platform: Platform) -> &'static str {
    match platform {
        Platform::WindowsX64 => regular::WINDOWS_LOGO,
        Platform::LinuxX64 => regular::LINUX_LOGO,
        Platform::MacOSX64 | Platform::MacOSArm64 => regular::APPLE_LOGO,
        Platform::IOSArm64 => regular::DEVICE_MOBILE,
        Platform::TvOSArm64 => regular::TELEVISION_SIMPLE,
        Platform::AndroidArm64 | Platform::AndroidX86_64 => regular::ANDROID_LOGO,
        Platform::FireTVArm64 => regular::TELEVISION,
        Platform::WebWasm32 => regular::GLOBE,
    }
}

/// Render one platform row in the left sidebar.
#[allow(clippy::too_many_arguments)]
fn platform_sidebar_button(
    ui: &mut egui::Ui,
    platform: Platform,
    selected: bool,
    installed: bool,
    available: bool,
    accent: egui::Color32,
    text_primary: egui::Color32,
    text_secondary: egui::Color32,
    surface_mid: egui::Color32,
    border_color: egui::Color32,
) -> egui::Response {
    let row_height = 40.0;
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), row_height),
        egui::Sense::click(),
    );

    if selected {
        ui.painter().rect_filled(rect, 4.0, surface_mid);
        ui.painter().rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0, accent),
            egui::StrokeKind::Inside,
        );
    } else if resp.hovered() {
        ui.painter()
            .rect_filled(rect, 4.0, surface_mid.gamma_multiply(0.5));
    } else {
        ui.painter().rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0, border_color.gamma_multiply(0.4)),
            egui::StrokeKind::Inside,
        );
    }

    let icon_color = if selected { accent } else { text_primary };
    let text_color = if selected { accent } else { text_primary };

    ui.painter().text(
        rect.left_center() + egui::vec2(10.0, 0.0),
        egui::Align2::LEFT_CENTER,
        platform_icon(platform),
        egui::FontId::proportional(18.0),
        icon_color,
    );

    ui.painter().text(
        rect.left_center() + egui::vec2(34.0, 0.0),
        egui::Align2::LEFT_CENTER,
        platform.display_name(),
        egui::FontId::proportional(12.5),
        text_color,
    );

    // Status dot on the right edge: green = installed, amber = available to download, gray = neither
    let dot_color = if installed {
        egui::Color32::from_rgb(89, 191, 115)
    } else if available {
        egui::Color32::from_rgb(242, 166, 64)
    } else {
        text_secondary.gamma_multiply(0.6)
    };
    ui.painter().circle_filled(
        egui::pos2(rect.right() - 10.0, rect.center().y),
        3.5,
        dot_color,
    );

    resp
}

/// Render the runtime template status block (installed / download / install-from-file).
#[allow(clippy::too_many_arguments)]
fn draw_runtime_status(
    ui: &mut egui::Ui,
    world: &mut World,
    selected_platform: Platform,
    template_installed: bool,
    accent: egui::Color32,
    text_primary: egui::Color32,
    text_secondary: egui::Color32,
    surface_mid: egui::Color32,
    error_color: egui::Color32,
) {
    let _ = text_primary;
    let runtime_dir = world.resource::<TemplateManager>().runtime_dir();

    let release_info = world.resource::<ExportOverlayState>().release_info.clone();
    let download_status = world
        .resource::<ExportOverlayState>()
        .download_status
        .clone();
    let download_active = world
        .resource::<ExportOverlayState>()
        .download_task
        .is_some();

    let asset_available = release_info
        .as_ref()
        .map(|info| info.available_platforms.contains(&selected_platform))
        .unwrap_or(false);

    // Status line
    ui.horizontal(|ui| {
        if template_installed {
            ui.label(
                egui::RichText::new(regular::CHECK_CIRCLE)
                    .color(egui::Color32::from_rgb(89, 191, 115)),
            );
            ui.label(
                egui::RichText::new("Runtime template installed")
                    .size(12.0)
                    .color(text_secondary),
            );
        } else {
            ui.label(
                egui::RichText::new(regular::WARNING).color(egui::Color32::from_rgb(242, 166, 64)),
            );
            ui.label(
                egui::RichText::new("Runtime template not installed")
                    .size(12.0)
                    .color(text_secondary),
            );
        }
    });

    ui.add_space(4.0);

    // Action buttons
    ui.horizontal(|ui| {
        let download_label = if asset_available {
            format!("{} Download from GitHub", regular::DOWNLOAD_SIMPLE)
        } else {
            format!("{} Not yet released", regular::DOWNLOAD_SIMPLE)
        };
        let download_button =
            egui::Button::new(egui::RichText::new(download_label).size(12.0).color(
                if asset_available {
                    egui::Color32::WHITE
                } else {
                    text_secondary
                },
            ))
            .fill(if asset_available { accent } else { surface_mid })
            .min_size(egui::vec2(180.0, 26.0));

        let download_enabled = asset_available && !download_active;
        if ui.add_enabled(download_enabled, download_button).clicked() {
            let task = download::spawn_download(selected_platform, runtime_dir.clone());
            let mut state = world.resource_mut::<ExportOverlayState>();
            state.download_task = Some(task);
            state.download_status = Some((
                selected_platform,
                DownloadProgress::Fetching("Starting download...".into()),
            ));
        }

        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new(format!("{} Install from file...", regular::FOLDER_OPEN))
                        .size(12.0),
                )
                .fill(surface_mid),
            )
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Select runtime template binary")
                .pick_file()
            {
                let mut mgr = world.resource_mut::<TemplateManager>();
                let _ = std::fs::create_dir_all(&runtime_dir);
                let dest = runtime_dir.join(selected_platform.runtime_binary_name());
                if let Err(e) = std::fs::copy(&path, &dest) {
                    warn!("Failed to install template: {}", e);
                }
                mgr.scan();
            }
        }
    });

    // Show ongoing download progress for the selected platform
    if let Some((p, status)) = &download_status {
        if *p == selected_platform {
            ui.add_space(4.0);
            match status {
                DownloadProgress::Fetching(msg) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            egui::RichText::new(msg.as_str())
                                .size(11.0)
                                .color(text_secondary),
                        );
                    });
                }
                DownloadProgress::Done(msg) => {
                    ui.label(
                        egui::RichText::new(format!("{} {}", regular::CHECK_CIRCLE, msg))
                            .size(11.0)
                            .color(egui::Color32::from_rgb(89, 191, 115)),
                    );
                }
                DownloadProgress::Error(msg) => {
                    ui.label(
                        egui::RichText::new(format!("{} {}", regular::WARNING, msg))
                            .size(11.0)
                            .color(error_color),
                    );
                }
            }
        }
    }
}

/// Render the per-platform export settings panel (packaging, mesh, window, options, plugins, icon).
// drop(export_state) ends the Mut<Resource> borrow early so `world` is free to
// re-borrow; Mut isn't Drop so clippy flags it, but the effect is intended.
#[allow(clippy::drop_non_drop)]
fn draw_settings_panel(
    ui: &mut egui::Ui,
    world: &mut World,
    selected_platform: Platform,
    is_desktop: bool,
    text_primary: egui::Color32,
    text_secondary: egui::Color32,
    surface_mid: egui::Color32,
) {
    // --- Packaging (desktop only) ---
    if is_desktop {
        section_label(ui, regular::FILE_ARCHIVE, "Packaging", text_primary);
        ui.add_space(4.0);

        let mut export_state = world.resource_mut::<ExportOverlayState>();

        ui.horizontal(|ui| {
            ui.radio_value(
                &mut export_state.packaging_mode,
                PackagingMode::SeparateFiles,
                egui::RichText::new("Binary + .rpak").color(text_primary),
            );
            ui.radio_value(
                &mut export_state.packaging_mode,
                PackagingMode::SingleBinary,
                egui::RichText::new("Single executable").color(text_primary),
            );
        });

        drop(export_state);
    }

    // --- Compression ---
    {
        if !is_desktop {
            section_label(ui, regular::FILE_ARCHIVE, "Packaging", text_primary);
            ui.add_space(4.0);
        } else {
            ui.add_space(4.0);
        }
        let mut export_state = world.resource_mut::<ExportOverlayState>();
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Compression:")
                    .size(12.0)
                    .color(text_secondary),
            );
            ui.add(
                egui::Slider::new(&mut export_state.compression_level, 1..=22).text("zstd level"),
            );
        });
        drop(export_state);
    }

    ui.add_space(12.0);

    // --- Mesh Optimization ---
    section_label(ui, regular::CUBE, "Mesh Optimization", text_primary);
    ui.add_space(4.0);

    {
        let mut export_state = world.resource_mut::<ExportOverlayState>();

        ui.checkbox(
            &mut export_state.mesh_simplify,
            egui::RichText::new("Simplify meshes").color(text_primary),
        );
        if export_state.mesh_simplify {
            ui.indent("mesh_simplify_ratio", |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Keep ratio:")
                            .size(12.0)
                            .color(text_secondary),
                    );
                    ui.add(
                        egui::Slider::new(&mut export_state.mesh_simplify_ratio, 0.1..=1.0)
                            .text("triangles"),
                    );
                });
            });
        }

        ui.checkbox(
            &mut export_state.mesh_quantize,
            egui::RichText::new("Quantize vertex attributes").color(text_primary),
        );

        ui.checkbox(
            &mut export_state.mesh_generate_lods,
            egui::RichText::new("Generate LODs").color(text_primary),
        );
        if export_state.mesh_generate_lods {
            ui.indent("mesh_lod_levels", |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Levels:")
                            .size(12.0)
                            .color(text_secondary),
                    );
                    ui.add(egui::Slider::new(&mut export_state.mesh_lod_levels, 1..=5));
                });
            });
        }

        drop(export_state);
    }

    ui.add_space(12.0);

    // --- Window Settings (desktop only) ---
    if is_desktop {
        section_label(ui, regular::MONITOR, "Window", text_primary);
        ui.add_space(4.0);

        let mut export_state = world.resource_mut::<ExportOverlayState>();

        ui.horizontal(|ui| {
            ui.radio_value(
                &mut export_state.window_mode,
                WindowMode::Windowed,
                egui::RichText::new("Windowed").color(text_primary),
            );
            ui.radio_value(
                &mut export_state.window_mode,
                WindowMode::Fullscreen,
                egui::RichText::new("Fullscreen").color(text_primary),
            );
            ui.radio_value(
                &mut export_state.window_mode,
                WindowMode::Borderless,
                egui::RichText::new("Borderless").color(text_primary),
            );
        });

        if export_state.window_mode == WindowMode::Windowed {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Size:")
                        .size(12.0)
                        .color(text_secondary),
                );
                ui.add(
                    egui::DragValue::new(&mut export_state.window_width)
                        .speed(1)
                        .range(320..=7680)
                        .suffix("w"),
                );
                ui.label(egui::RichText::new("x").color(text_secondary));
                ui.add(
                    egui::DragValue::new(&mut export_state.window_height)
                        .speed(1)
                        .range(240..=4320)
                        .suffix("h"),
                );
            });
        }

        drop(export_state);
        ui.add_space(12.0);
    }

    // --- Options ---
    section_label(ui, regular::GEAR, "Options", text_primary);
    ui.add_space(4.0);

    let mut export_state = world.resource_mut::<ExportOverlayState>();

    ui.checkbox(
        &mut export_state.console_logging,
        egui::RichText::new("Console logging").color(text_primary),
    );

    if is_desktop {
        let server_available = selected_platform.supports_dedicated_server();
        if server_available {
            ui.checkbox(
                &mut export_state.include_server,
                egui::RichText::new("Include dedicated server").color(text_primary),
            );

            if export_state.include_server {
                drop(export_state);
                // The dedicated server reuses the game binary (run with
                // `--server`), so it just needs the platform's template
                // installed — same as a normal export.
                let server_installed = world
                    .resource::<TemplateManager>()
                    .is_installed(selected_platform);
                ui.indent("server_template_status", |ui| {
                    if server_installed {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(regular::CHECK_CIRCLE)
                                    .color(egui::Color32::from_rgb(89, 191, 115)),
                            );
                            ui.label(
                                egui::RichText::new("Adds server.bat + server.rpak")
                                    .size(11.0)
                                    .color(text_secondary),
                            );
                        });
                    } else {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(regular::WARNING)
                                    .color(egui::Color32::from_rgb(242, 166, 64)),
                            );
                            ui.label(
                                egui::RichText::new("Runtime template not installed")
                                    .size(11.0)
                                    .color(text_secondary),
                            );
                        });
                    }
                });
                export_state = world.resource_mut::<ExportOverlayState>();
            }
        }
    }

    drop(export_state);

    // --- Plugins ---
    if !world.resource::<ExportOverlayState>().plugins_scanned {
        let plugins_dir = world.resource::<TemplateManager>().runtime_plugins_dir();
        let plugins = dynamic_plugin_loader::scan_plugins(&plugins_dir);
        let mut state = world.resource_mut::<ExportOverlayState>();
        for p in &plugins {
            state.selected_plugins.insert(p.id.clone());
        }
        state.available_plugins = plugins;
        state.plugins_scanned = true;
    }

    {
        let mut state = world.resource_mut::<ExportOverlayState>();
        if !state.available_plugins.is_empty() {
            ui.add_space(8.0);
            section_label(ui, regular::PUZZLE_PIECE, "Plugins", text_primary);
            ui.add_space(4.0);

            let plugins: Vec<_> = state
                .available_plugins
                .iter()
                .map(|p| (p.id.clone(), p.scope))
                .collect();
            egui::ScrollArea::vertical()
                .id_salt("export_plugin_scroll")
                .max_height(160.0)
                .show(ui, |ui| {
                    for (id, scope) in &plugins {
                        let mut checked = state.selected_plugins.contains(id.as_str());
                        let label = format!("{} ({:?})", id, scope);
                        if ui
                            .checkbox(
                                &mut checked,
                                egui::RichText::new(label).size(12.0).color(text_primary),
                            )
                            .changed()
                        {
                            if checked {
                                state.selected_plugins.insert(id.clone());
                            } else {
                                state.selected_plugins.remove(id.as_str());
                            }
                        }
                    }
                });
        }
    }

    // --- Icon ---
    {
        let mut export_state = world.resource_mut::<ExportOverlayState>();
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Icon:")
                    .size(12.0)
                    .color(text_secondary),
            );
            if let Some(ref icon) = export_state.icon_path {
                ui.label(
                    egui::RichText::new(icon.as_str())
                        .size(11.0)
                        .color(text_primary),
                );
                if ui
                    .add(egui::Button::new(egui::RichText::new(regular::X).size(12.0)).frame(false))
                    .clicked()
                {
                    export_state.icon_path = None;
                }
            } else {
                ui.label(egui::RichText::new("None").size(11.0).color(text_secondary));
            }
            let icon_path_mut = &mut export_state.icon_path;
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("{} Browse", regular::IMAGE)).size(11.0),
                    )
                    .fill(surface_mid),
                )
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Select icon")
                    .add_filter("Images", &["png", "ico", "svg"])
                    .pick_file()
                {
                    *icon_path_mut = Some(path.to_string_lossy().to_string());
                }
            }
        });
    }
}

/// Export for Android: copy the template APK and inject the rpak into its assets/ folder.
fn export_android_apk(
    template_path: &std::path::Path,
    output_dir: &std::path::Path,
    binary_name: &str,
    packer: RpakPacker,
    compression_level: i32,
) -> std::io::Result<()> {
    use std::io::{Read as _, Write as _};

    let rpak_bytes = packer.finish(compression_level)?;

    let apk_dest = output_dir.join(binary_name);

    // Read the template APK
    let template_data = std::fs::read(template_path)?;
    let cursor = std::io::Cursor::new(&template_data);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    // Create the output APK, copying all existing entries and adding the rpak
    let out_file = std::fs::File::create(&apk_dest)?;
    let mut writer = zip::ZipWriter::new(out_file);

    // Copy all existing entries from template
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let name = entry.name().to_string();

        // Android requires resources.arsc and native libs to be stored
        // uncompressed with 4-byte alignment (R+ / API 30+)
        let must_store = name == "resources.arsc" || name.ends_with(".so");

        let options = if must_store {
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored)
                .unix_permissions(entry.unix_mode().unwrap_or(0o644))
                .with_alignment(16384)
        } else {
            zip::write::SimpleFileOptions::default()
                .compression_method(entry.compression())
                .unix_permissions(entry.unix_mode().unwrap_or(0o644))
        };

        writer
            .start_file(name, options)
            .map_err(std::io::Error::other)?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        writer.write_all(&buf)?;
    }

    // Add the rpak as assets/game.rpak
    let rpak_options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    writer
        .start_file("assets/game.rpak", rpak_options)
        .map_err(std::io::Error::other)?;
    writer.write_all(&rpak_bytes)?;

    writer
        .finish()
        .map_err(std::io::Error::other)?;

    Ok(())
}

/// Export for iOS: extract template .app zip, inject game.rpak, re-zip.
///
/// The template is a zip containing `RenzoraRuntime.app/` (unsigned).
/// We inject `game.rpak` into the app bundle's root so the VFS can find it
/// via `CFBundleCopyResourceURL`.
fn export_ios_app(
    template_path: &std::path::Path,
    output_dir: &std::path::Path,
    project_name: &str,
    packer: RpakPacker,
    compression_level: i32,
) -> std::io::Result<()> {
    use std::io::{Read as _, Write as _};

    let rpak_bytes = packer.finish(compression_level)?;
    let output_zip = output_dir.join(format!("{}.ipa", project_name));

    // Read the template zip
    let template_data = std::fs::read(template_path)?;
    let cursor = std::io::Cursor::new(&template_data);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let out_file = std::fs::File::create(&output_zip)?;
    let mut writer = zip::ZipWriter::new(out_file);

    // Copy all existing entries from template
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let name = entry.name().to_string();

        let options = zip::write::SimpleFileOptions::default()
            .compression_method(entry.compression())
            .unix_permissions(entry.unix_mode().unwrap_or(0o644));

        if entry.is_dir() {
            writer
                .add_directory(&name, options)
                .map_err(std::io::Error::other)?;
        } else {
            writer
                .start_file(&name, options)
                .map_err(std::io::Error::other)?;
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            writer.write_all(&buf)?;
        }
    }

    // Add game.rpak inside the .app bundle
    // IPA structure: Payload/AppName.app/game.rpak
    // Template structure: RenzoraRuntime.app/game.rpak
    // Find the .app directory name from existing entries
    let app_prefix = archive
        .file_names()
        .find(|n| n.ends_with(".app/"))
        .map(|n| n.to_string())
        .unwrap_or_else(|| "Payload/RenzoraRuntime.app/".to_string());

    let rpak_path = format!("{}game.rpak", app_prefix);
    let rpak_options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    writer
        .start_file(&rpak_path, rpak_options)
        .map_err(std::io::Error::other)?;
    writer.write_all(&rpak_bytes)?;

    writer
        .finish()
        .map_err(std::io::Error::other)?;

    Ok(())
}

/// Export for Web/WASM: extract template zip, add rpak + index.html, write output zip.
///
/// The template is a zip file containing `renzora-runtime.js` and
/// `renzora-runtime_bg.wasm` (built by `makers build-web`).
fn export_wasm_zip(
    tx: &mpsc::Sender<ExportMsg>,
    template_zip_path: &std::path::Path,
    output_dir: &std::path::Path,
    project_name: &str,
    packer: RpakPacker,
    compression_level: i32,
) -> std::io::Result<()> {
    use std::io::{Read as _, Write as _};

    let _ = tx.send(ExportMsg::Progress("Packaging WASM build...".into()));

    let rpak_bytes = packer.finish(compression_level)?;
    let zip_path = output_dir.join(format!("{}-web.zip", project_name));

    // Read the template zip
    let template_data = std::fs::read(template_zip_path)?;
    let cursor = std::io::Cursor::new(&template_data);
    let mut template_archive = zip::ZipArchive::new(cursor)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let out_file = std::fs::File::create(&zip_path)?;
    let mut writer = zip::ZipWriter::new(out_file);

    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    // Copy all template entries (js + wasm) into the output zip
    for i in 0..template_archive.len() {
        let mut entry = template_archive
            .by_index(i)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let name = entry.name().to_string();

        let file_options = options;

        writer
            .start_file(&name, file_options)
            .map_err(std::io::Error::other)?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        writer.write_all(&buf)?;
    }

    // Add the rpak as game.rpak
    writer
        .start_file("game.rpak", stored)
        .map_err(std::io::Error::other)?;
    writer.write_all(&rpak_bytes)?;

    // Generate index.html
    let index_html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{title}</title>
    <style>
        html, body {{ margin: 0; padding: 0; overflow: hidden; background: #050410; }}
        canvas {{ display: block; }}
        #loading {{
            position: fixed; inset: 0; display: flex;
            align-items: center; justify-content: center;
            background: #050410; color: #888; font-family: monospace; font-size: 14px;
            z-index: 10;
        }}
        #loading.hidden {{ display: none; }}
    </style>
</head>
<body>
    <div id="loading">Loading {title}...</div>
    <script type="module">
        import init, {{ set_rpak, start }} from './renzora-runtime.js';

        async function run() {{
            const rpakResp = await fetch('./game.rpak');
            if (!rpakResp.ok) throw new Error('Failed to fetch game.rpak: ' + rpakResp.status);
            const rpakBytes = new Uint8Array(await rpakResp.arrayBuffer());

            await init();
            set_rpak(rpakBytes);
            start();

            document.getElementById('loading').classList.add('hidden');

            const canvas = document.querySelector('canvas');
            if (canvas) {{
                const resize = () => {{
                    canvas.width = window.innerWidth;
                    canvas.height = window.innerHeight;
                    canvas.style.width = window.innerWidth + 'px';
                    canvas.style.height = window.innerHeight + 'px';
                }};
                resize();
                window.addEventListener('resize', resize);
            }}
        }}

        run().catch(err => {{
            document.getElementById('loading').textContent = 'Failed to load: ' + err;
            console.error(err);
        }});
    </script>
</body>
</html>
"#,
        title = project_name,
    );

    writer
        .start_file("index.html", options)
        .map_err(std::io::Error::other)?;
    writer.write_all(index_html.as_bytes())?;

    writer
        .finish()
        .map_err(std::io::Error::other)?;

    info!("[export] WASM zip written to {}", zip_path.display());

    Ok(())
}

fn section_label(ui: &mut egui::Ui, icon: &str, label: &str, color: egui::Color32) {
    ui.label(
        egui::RichText::new(format!("{} {}", icon, label))
            .size(13.0)
            .color(color)
            .strong(),
    );
}

fn run_export(world: &mut World, project_name: &str) {
    let project = world.resource::<CurrentProject>().clone();
    let export_state = world.resource::<ExportOverlayState>();
    let platform = export_state.platform;
    let packaging_mode = export_state.packaging_mode;
    let compression_level = export_state.compression_level;
    let output_dir = std::path::PathBuf::from(&export_state.output_dir);
    let window_mode = export_state.window_mode;
    let window_width = export_state.window_width;
    let window_height = export_state.window_height;
    let console_logging = export_state.console_logging;
    let include_server = export_state.include_server;
    let icon_path = if export_state
        .icon_path
        .as_deref()
        .map(str::is_empty)
        .unwrap_or(true)
    {
        None
    } else {
        export_state.icon_path.clone()
    };
    let binary_name_override: Option<String> = {
        let trimmed = export_state.binary_name.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    };
    let mesh_simplify = export_state.mesh_simplify;
    let mesh_simplify_ratio = export_state.mesh_simplify_ratio;
    let mesh_quantize = export_state.mesh_quantize;
    let mesh_generate_lods = export_state.mesh_generate_lods;
    let mesh_lod_levels = export_state.mesh_lod_levels;
    let selected_plugins: Vec<std::path::PathBuf> = export_state
        .available_plugins
        .iter()
        .filter(|p| export_state.selected_plugins.contains(&p.id))
        .map(|p| p.path.clone())
        .collect();
    let project_name = project_name.to_string();

    // Get runtime directory for shared libs
    let runtime_dir = world.resource::<TemplateManager>().runtime_dir();

    // Get template path before spawning thread
    let template_path = match world.resource::<TemplateManager>().get(platform) {
        Some(t) => t.path.clone(),
        None => {
            world.resource_mut::<ExportOverlayState>().progress =
                ExportProgress::Error("No template installed for this platform".to_string());
            return;
        }
    };

    // The dedicated server reuses the game binary (run with `--server`), so
    // there's no separate server template to resolve here.

    let (tx, rx) = mpsc::channel();

    // Set initial progress and store task
    {
        let mut state = world.resource_mut::<ExportOverlayState>();
        state.progress = ExportProgress::Working("Packing assets...".into());
        state.active_task = Some(ExportTask { rx: Mutex::new(rx) });
    }

    // Spawn background thread
    std::thread::spawn(move || {
        export_worker(
            tx,
            project,
            project_name,
            platform,
            packaging_mode,
            compression_level,
            output_dir,
            window_mode,
            window_width,
            window_height,
            console_logging,
            icon_path,
            binary_name_override,
            include_server,
            mesh_simplify,
            mesh_simplify_ratio,
            mesh_quantize,
            mesh_generate_lods,
            mesh_lod_levels,
            template_path,
            selected_plugins,
            runtime_dir,
        );
    });
}

/// Background export worker — runs on a separate thread.
#[allow(clippy::too_many_arguments)]
fn export_worker(
    tx: mpsc::Sender<ExportMsg>,
    project: CurrentProject,
    project_name: String,
    platform: Platform,
    packaging_mode: PackagingMode,
    compression_level: i32,
    output_dir: std::path::PathBuf,
    window_mode: WindowMode,
    window_width: u32,
    window_height: u32,
    console_logging: bool,
    icon_path: Option<String>,
    binary_name_override: Option<String>,
    include_server: bool,
    mesh_simplify: bool,
    mesh_simplify_ratio: f32,
    mesh_quantize: bool,
    mesh_generate_lods: bool,
    mesh_lod_levels: u32,
    template_path: std::path::PathBuf,
    selected_plugins: Vec<std::path::PathBuf>,
    runtime_dir: std::path::PathBuf,
) {
    // Pack assets
    let _ = tx.send(ExportMsg::Progress("Scanning project assets...".into()));
    let tx_pack = tx.clone();
    let mut packer = match pack_project_with_progress(&project.path, None, |key| {
        let _ = tx_pack.send(ExportMsg::Progress(format!("Packing {}", key)));
    }) {
        Ok(p) => p,
        Err(e) => {
            let _ = tx.send(ExportMsg::Error(format!("Failed to pack assets: {}", e)));
            return;
        }
    };
    info!("[export] Packed {} referenced files", packer.len());

    // Strip editor-only components from scene files
    packer.strip_for_runtime();

    // Mesh optimization
    let mesh_settings = MeshOptSettings {
        vertex_cache: true,
        overdraw: true,
        vertex_fetch: true,
        simplify: mesh_simplify,
        simplify_ratio: mesh_simplify_ratio,
        quantize: mesh_quantize,
        generate_lods: false,
        lod_levels: mesh_lod_levels,
    };
    if mesh_settings.any_enabled() {
        let settings = mesh_settings.clone();
        let tx2 = tx.clone();
        packer.optimize_meshes_with_progress(
            |bytes| renzora_import::optimize_glb(bytes, &settings),
            |current, total, name| {
                let _ = tx2.send(ExportMsg::Progress(format!(
                    "Optimizing meshes ({}/{}) {}",
                    current, total, name
                )));
            },
        );
    }

    // LOD generation
    if mesh_generate_lods {
        let tx2 = tx.clone();
        packer.generate_mesh_lods_with_progress(
            mesh_lod_levels,
            |bytes, ratio| {
                let lod_settings = MeshOptSettings {
                    vertex_cache: true,
                    overdraw: true,
                    vertex_fetch: true,
                    simplify: true,
                    simplify_ratio: ratio,
                    ..Default::default()
                };
                renzora_import::optimize_glb(bytes, &lod_settings)
            },
            |current, total, name| {
                let _ = tx2.send(ExportMsg::Progress(format!(
                    "Generating LODs ({}/{}) {}",
                    current, total, name
                )));
            },
        );
    }

    // Build the runtime ProjectConfig from the editor's project.toml plus
    // the export-overlay overrides, then replace project.toml inside the
    // rpak so the runtime sees the chosen window mode / size / console flag.
    let mut export_config = project.config.clone();
    export_config.window.width = window_width;
    export_config.window.height = window_height;
    export_config.window.mode = window_mode;
    export_config.window.resizable = matches!(window_mode, WindowMode::Windowed);
    export_config.console_logging = console_logging;
    // Editor-only fields shouldn't ship in exported builds.
    export_config.editor = None;
    export_config.editor_last_scene = None;

    // If the user picked an icon, copy it into the rpak under `assets/icon.png`
    // and point project.toml at it. The runtime resolves icons through Vfs.
    if let Some(ref icon_src) = icon_path {
        match std::fs::read(icon_src) {
            Ok(bytes) => {
                let archive_path = "assets/icon.png".to_string();
                packer.add_file(&archive_path, bytes);
                export_config.icon = Some(archive_path);
            }
            Err(e) => {
                warn!("[export] Failed to read icon {}: {}", icon_src, e);
            }
        }
    }

    match toml::to_string_pretty(&export_config) {
        Ok(s) => packer.add_file("project.toml", s.into_bytes()),
        Err(e) => {
            let _ = tx.send(ExportMsg::Error(format!(
                "Failed to serialize project config: {}",
                e
            )));
            return;
        }
    }

    let file_count = packer.len();

    let _ = tx.send(ExportMsg::Progress("Writing output...".into()));

    // Create output directory: output_dir/project_name/
    let output_dir = output_dir.join(&project_name);
    if let Err(e) = std::fs::create_dir_all(&output_dir) {
        let _ = tx.send(ExportMsg::Error(format!(
            "Failed to create output dir: {}",
            e
        )));
        return;
    }

    // Stem the binary's filename uses (e.g. "MyGame" produces MyGame.exe / MyGame.apk).
    // Override falls back to the project name when blank.
    let binary_stem = binary_name_override
        .as_deref()
        .unwrap_or(project_name.as_str());
    let binary_name = platform.binary_name(binary_stem);
    let is_android = matches!(
        platform,
        Platform::AndroidArm64 | Platform::AndroidX86_64 | Platform::FireTVArm64
    );
    let is_ios = matches!(platform, Platform::IOSArm64 | Platform::TvOSArm64);
    let is_wasm = matches!(platform, Platform::WebWasm32);

    let result = if is_ios {
        export_ios_app(
            &template_path,
            &output_dir,
            binary_stem,
            packer,
            compression_level,
        )
    } else if is_wasm {
        export_wasm_zip(
            &tx,
            &template_path,
            &output_dir,
            binary_stem,
            packer,
            compression_level,
        )
    } else if is_android {
        export_android_apk(
            &template_path,
            &output_dir,
            &binary_name,
            packer,
            compression_level,
        )
        .and_then(|_| {
            let apk_path = output_dir.join(&binary_name);
            crate::apk_signer::sign_apk(&apk_path)
        })
    } else {
        match packaging_mode {
            PackagingMode::SeparateFiles => {
                let rpak_path = output_dir.join(format!("{}.rpak", binary_stem));
                let binary_dest = output_dir.join(&binary_name);

                packer
                    .write_to_file(&rpak_path, compression_level)
                    .and_then(|_| std::fs::copy(&template_path, &binary_dest).map(|_| ())).map(|_| ())
            }
            PackagingMode::SingleBinary => {
                let binary_dest = output_dir.join(&binary_name);
                packer
                    .append_to_binary(&template_path, &binary_dest, compression_level).map(|_| ())
            }
        }
    };

    match result {
        Ok(()) => {
            if !is_wasm {
                // Copy shared libraries from runtime build (bevy_dylib + std + SDK)
                let _ = tx.send(ExportMsg::Progress("Copying shared libraries...".into()));
                for entry in std::fs::read_dir(&runtime_dir)
                    .into_iter()
                    .flatten()
                    .flatten()
                {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if let Some(ext) = entry.path().extension() {
                        let ext = ext.to_string_lossy();
                        if ext == "dll" || ext == "so" || ext == "dylib" {
                            // Copy SDK + bevy_dylib + std (not plugins/ or binaries)
                            if name_str.starts_with("bevy_dylib")
                                || name_str.starts_with("libbevy_dylib")
                                || name_str.starts_with("std-")
                                || name_str.starts_with("libstd-")
                                || name_str.starts_with("renzora.")
                                || name_str.starts_with("librenzora.")
                            {
                                let _ = std::fs::copy(entry.path(), output_dir.join(&name));
                            }
                        }
                    }
                }

                // Copy selected plugins
                if !selected_plugins.is_empty() {
                    let _ = tx.send(ExportMsg::Progress("Copying plugins...".into()));
                    let plugins_out = output_dir.join("plugins");
                    let _ = std::fs::create_dir_all(&plugins_out);

                    for plugin_path in &selected_plugins {
                        if let Some(filename) = plugin_path.file_name() {
                            let dest = plugins_out.join(filename);
                            if let Err(e) = std::fs::copy(plugin_path, &dest) {
                                warn!("[export] Failed to copy plugin {:?}: {}", filename, e);
                            }
                        }
                    }
                    info!(
                        "[export] Copied {} plugins to output",
                        selected_plugins.len()
                    );
                }
            }

            // Server export
            if include_server {
                let server_result = export_server_standalone(
                    &tx,
                    &project,
                    binary_stem,
                    platform,
                    compression_level,
                    &output_dir,
                );
                match server_result {
                    Ok(server_files) => {
                        let _ = tx.send(ExportMsg::Done(format!(
                            "Exported {} files + server ({} files) to {}",
                            file_count,
                            server_files,
                            output_dir.display()
                        )));
                    }
                    Err(e) => {
                        let _ = tx.send(ExportMsg::Done(format!(
                            "Exported {} files (server failed: {}) to {}",
                            file_count,
                            e,
                            output_dir.display()
                        )));
                    }
                }
            } else {
                let _ = tx.send(ExportMsg::Done(format!(
                    "Exported {} files to {}",
                    file_count,
                    output_dir.display()
                )));
            }
        }
        Err(e) => {
            let _ = tx.send(ExportMsg::Error(format!("Export failed: {}", e)));
        }
    }
}

/// Write the dedicated-server data bundle and launcher alongside the game
/// export. The server reuses the **game binary** (run with `--server`) — no
/// separate server executable is produced. Output:
///   - `server.rpak` — project assets stripped for server use (no visuals).
///   - `server.bat` / `server.sh` — runs the game binary in server mode,
///     pointed at `server.rpak` via `--rpak`.
fn export_server_standalone(
    tx: &mpsc::Sender<ExportMsg>,
    project: &CurrentProject,
    binary_stem: &str,
    platform: Platform,
    compression_level: i32,
    output_dir: &std::path::Path,
) -> Result<usize, String> {
    let _ = tx.send(ExportMsg::Progress("Packing server assets...".into()));

    let mut server_packer = pack_project_filtered(&project.path, SERVER_EXTENSIONS)
        .map_err(|e| format!("Failed to pack server assets: {}", e))?;

    server_packer.strip_for_server();

    let server_file_count = server_packer.len();

    let _ = tx.send(ExportMsg::Progress("Writing server bundle...".into()));

    // Always a standalone `server.rpak`; the launcher points the game binary at
    // it with `--rpak`, so the client's packaging mode doesn't matter here.
    let rpak_path = output_dir.join("server.rpak");
    server_packer
        .write_to_file(&rpak_path, compression_level)
        .map_err(|e| format!("Failed to write server.rpak: {}", e))?;

    let game_binary = platform.binary_name(binary_stem);
    write_server_launcher(output_dir, &game_binary, platform)
        .map_err(|e| format!("Failed to write server launcher: {}", e))?;

    Ok(server_file_count)
}

/// Write a `server.bat` (Windows) / `server.sh` (Linux/macOS) launcher that runs
/// the game binary in dedicated-server mode against `server.rpak`.
fn write_server_launcher(
    output_dir: &std::path::Path,
    game_binary: &str,
    platform: Platform,
) -> std::io::Result<()> {
    match platform {
        Platform::WindowsX64 => {
            let path = output_dir.join("server.bat");
            std::fs::write(
                path,
                format!(
                    "@echo off\r\n\"%~dp0{}\" --server --rpak \"%~dp0server.rpak\" %*\r\n",
                    game_binary
                ),
            )?;
        }
        Platform::LinuxX64 | Platform::MacOSX64 | Platform::MacOSArm64 => {
            let path = output_dir.join("server.sh");
            std::fs::write(
                &path,
                format!(
                    "#!/bin/sh\ndir=\"$(dirname \"$0\")\"\nexec \"$dir/{}\" --server --rpak \"$dir/server.rpak\" \"$@\"\n",
                    game_binary
                ),
            )?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
            }
        }
        _ => {}
    }
    Ok(())
}
