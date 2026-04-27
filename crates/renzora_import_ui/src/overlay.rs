//! Import overlay UI — modal dialog for importing 3D models.

use std::path::PathBuf;
use std::sync::{mpsc, Mutex};

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora::core::CurrentProject;
use renzora_import::optimize::MeshOptSettings;
use renzora_import::settings::{ImportSettings, UpAxis};
use renzora_theme::ThemeManager;

/// Import progress state.
#[derive(Debug, Clone, PartialEq)]
pub enum ImportProgress {
    Idle,
    Working {
        current: usize,
        total: usize,
        label: String,
    },
    Done(String),
    Error(String),
}

/// Per-file import result for the output log.
#[derive(Debug, Clone)]
pub struct ImportLogEntry {
    pub file_name: String,
    pub success: bool,
    pub message: String,
}

/// Messages sent from the background import thread.
enum ImportMsg {
    Progress {
        current: usize,
        total: usize,
        label: String,
    },
    Log(ImportLogEntry),
    /// Marshal a material-extraction event across the thread boundary; the
    /// UI-side poller triggers it on `&mut World` so observers in other
    /// crates can write the `.material` file without us depending on them.
    MaterialExtracted(renzora::core::PbrMaterialExtracted),
    Done(String),
    Error(String),
}

/// Handle for a running background import.
pub(crate) struct ImportTask {
    rx: Mutex<mpsc::Receiver<ImportMsg>>,
}

/// Resource holding the import overlay state.
#[derive(Resource)]
pub struct ImportOverlayState {
    pub visible: bool,
    pub pending_files: Vec<PathBuf>,
    pub target_directory: String,
    pub settings: ImportSettings,
    pub progress: ImportProgress,
    /// Per-file import results shown in the output log.
    pub log_entries: Vec<ImportLogEntry>,
    /// Background import task (if running).
    pub(crate) active_task: Option<ImportTask>,
}

impl Default for ImportOverlayState {
    fn default() -> Self {
        Self {
            visible: false,
            pending_files: Vec::new(),
            target_directory: "models".to_string(),
            settings: ImportSettings::default(),
            progress: ImportProgress::Idle,
            log_entries: Vec::new(),
            active_task: None,
        }
    }
}

/// Drain progress messages from the background thread into overlay state.
pub(crate) fn poll_import_task(world: &mut World) {
    let has_task = world
        .resource::<ImportOverlayState>()
        .active_task
        .is_some();
    if !has_task {
        return;
    }

    let mut finished = false;
    let mut progress_updates: Vec<ImportMsg> = Vec::new();

    // Drain all pending messages
    {
        let state = world.resource::<ImportOverlayState>();
        let task = state.active_task.as_ref().unwrap();
        let rx = task.rx.lock().unwrap();
        loop {
            match rx.try_recv() {
                Ok(msg) => {
                    let is_terminal = matches!(msg, ImportMsg::Done(_) | ImportMsg::Error(_));
                    progress_updates.push(msg);
                    if is_terminal {
                        finished = true;
                        break;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    progress_updates
                        .push(ImportMsg::Error("Import thread terminated unexpectedly".into()));
                    finished = true;
                    break;
                }
            }
        }
    }

    // Apply messages. Material-extraction events need to fire on `&mut World`
    // (observers can't be triggered while an exclusive resource borrow is
    // held), so we split them out and deliver after releasing the state
    // borrow.
    let mut material_events: Vec<renzora::core::PbrMaterialExtracted> = Vec::new();
    {
        let mut state = world.resource_mut::<ImportOverlayState>();
        for msg in progress_updates {
            match msg {
                ImportMsg::Progress {
                    current,
                    total,
                    label,
                } => {
                    state.progress = ImportProgress::Working {
                        current,
                        total,
                        label,
                    };
                }
                ImportMsg::Log(entry) => {
                    state.log_entries.push(entry);
                }
                ImportMsg::MaterialExtracted(ev) => {
                    material_events.push(ev);
                }
                ImportMsg::Done(msg) => {
                    state.progress = ImportProgress::Done(msg);
                    state.pending_files.clear();
                }
                ImportMsg::Error(msg) => {
                    state.progress = ImportProgress::Error(msg);
                }
            }
        }

        if finished {
            state.active_task = None;
        }
    }

    for ev in material_events {
        world.trigger(ev);
    }
}

pub fn draw_import_overlay(world: &mut World, ctx: &egui::Context) {
    // Poll background task every frame
    poll_import_task(world);

    // Dim background
    let screen = ctx.input(|i| i.viewport_rect());
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("import_overlay_bg"),
    ));
    painter.rect_filled(screen, 0.0, egui::Color32::from_black_alpha(160));

    // Read theme
    let (panel_bg, text_primary, text_secondary, accent, error_color, border_color, surface_mid, success_color) = {
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
            egui::Color32::from_rgb(89, 191, 115),
        )
    };

    let has_project = world.get_resource::<CurrentProject>().is_some();

    let window_width = 520.0;
    let window_id = egui::Id::new("import_overlay_window");

    egui::Area::new(window_id)
        .fixed_pos(egui::pos2(
            (screen.width() - window_width) / 2.0,
            screen.height() * 0.1,
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

                // Header
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{} Import 3D Models", regular::CUBE))
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
                            close_overlay(world);
                        }
                    });
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(8.0);

                if !has_project {
                    ui.label(
                        egui::RichText::new("No project open. Open a project before importing.")
                            .color(error_color),
                    );
                    return;
                }

                // --- Files ---
                section_label(ui, regular::FILES, "Files", text_primary);
                ui.add_space(4.0);

                // File list
                let file_frame = egui::Frame::new()
                    .fill(surface_mid)
                    .stroke(egui::Stroke::new(1.0, border_color))
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin::same(8));

                file_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    let mut remove_idx = None;

                    let file_count = world.resource::<ImportOverlayState>().pending_files.len();

                    if file_count == 0 {
                        ui.label(
                            egui::RichText::new("No files added yet")
                                .size(11.0)
                                .color(text_secondary),
                        );
                    } else {
                        // Show scrollable file list (max 5 visible)
                        let max_height = 120.0;
                        egui::ScrollArea::vertical()
                            .id_salt("import_file_list")
                            .max_height(max_height)
                            .show(ui, |ui| {
                                let state = world.resource::<ImportOverlayState>();
                                for (i, path) in state.pending_files.iter().enumerate() {
                                    ui.horizontal(|ui| {
                                        let name = path
                                            .file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or("unknown");
                                        let ext = path
                                            .extension()
                                            .and_then(|e| e.to_str())
                                            .unwrap_or("")
                                            .to_uppercase();

                                        ui.label(
                                            egui::RichText::new(regular::CUBE)
                                                .size(12.0)
                                                .color(egui::Color32::from_rgb(255, 170, 100)),
                                        );
                                        ui.label(
                                            egui::RichText::new(name)
                                                .size(11.0)
                                                .color(text_primary),
                                        );
                                        ui.label(
                                            egui::RichText::new(format!("({})", ext))
                                                .size(10.0)
                                                .color(text_secondary),
                                        );

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui
                                                    .add(
                                                        egui::Button::new(
                                                            egui::RichText::new(regular::X)
                                                                .size(11.0)
                                                                .color(text_secondary),
                                                        )
                                                        .frame(false),
                                                    )
                                                    .clicked()
                                                {
                                                    remove_idx = Some(i);
                                                }
                                            },
                                        );
                                    });
                                }
                            });
                    }

                    if let Some(idx) = remove_idx {
                        world.resource_mut::<ImportOverlayState>().pending_files.remove(idx);
                    }

                    ui.add_space(4.0);

                    // Drop zone hint + browse button
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} Drop files here or",
                                regular::CLOUD_ARROW_DOWN
                            ))
                            .size(11.0)
                            .color(text_secondary),
                        );

                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(format!("{} Browse...", regular::FOLDER_OPEN))
                                        .size(11.0),
                                )
                                .fill(surface_mid),
                            )
                            .clicked()
                        {
                            let extensions: Vec<&str> =
                                renzora_import::supported_extensions().to_vec();
                            if let Some(paths) = rfd::FileDialog::new()
                                .set_title("Select 3D model files")
                                .add_filter("3D Models", &extensions)
                                .add_filter("All Files", &["*"])
                                .pick_files()
                            {
                                let mut state = world.resource_mut::<ImportOverlayState>();
                                let was_empty = state.pending_files.is_empty();
                                for path in &paths {
                                    if !state.pending_files.contains(path) {
                                        state.pending_files.push(path.clone());
                                    }
                                }
                                if was_empty && state.settings.scale == 1.0 {
                                    if let Some(scale) = renzora_import::units::detect_unit_scale(&paths[0]) {
                                        state.settings.scale = scale;
                                    }
                                }
                            }
                        }
                    });

                    // Handle drops inside the overlay
                    let dropped_in_zone: Vec<PathBuf> = ui.input(|i| {
                        i.raw
                            .dropped_files
                            .iter()
                            .filter_map(|f| f.path.clone())
                            .filter(|p| renzora_import::formats::is_supported(p))
                            .collect()
                    });
                    if !dropped_in_zone.is_empty() {
                        let mut state = world.resource_mut::<ImportOverlayState>();
                        let was_empty = state.pending_files.is_empty();
                        for path in &dropped_in_zone {
                            if !state.pending_files.contains(path) {
                                state.pending_files.push(path.clone());
                            }
                        }
                        if was_empty && state.settings.scale == 1.0 {
                            if let Some(scale) = renzora_import::units::detect_unit_scale(&dropped_in_zone[0]) {
                                state.settings.scale = scale;
                            }
                        }
                    }
                });

                ui.add_space(12.0);

                // --- Import Settings ---
                section_label(ui, regular::GEAR, "Import Settings", text_primary);
                ui.add_space(4.0);

                {
                    let mut state = world.resource_mut::<ImportOverlayState>();

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Scale:")
                                .size(12.0)
                                .color(text_secondary),
                        );
                        ui.add(
                            egui::DragValue::new(&mut state.settings.scale)
                                .speed(0.01)
                                .range(0.001..=1000.0)
                                .suffix("x"),
                        );
                    });

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Up Axis:")
                                .size(12.0)
                                .color(text_secondary),
                        );

                        let axis_label = match state.settings.up_axis {
                            UpAxis::Auto => "Auto",
                            UpAxis::YUp => "Y-Up",
                            UpAxis::ZUp => "Z-Up",
                        };

                        egui::ComboBox::from_id_salt("import_up_axis")
                            .selected_text(axis_label)
                            .width(100.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut state.settings.up_axis,
                                    UpAxis::Auto,
                                    "Auto",
                                );
                                ui.selectable_value(
                                    &mut state.settings.up_axis,
                                    UpAxis::YUp,
                                    "Y-Up (GLTF/Bevy)",
                                );
                                ui.selectable_value(
                                    &mut state.settings.up_axis,
                                    UpAxis::ZUp,
                                    "Z-Up (Blender/CAD)",
                                );
                            });
                    });

                    ui.checkbox(
                        &mut state.settings.flip_uvs,
                        egui::RichText::new("Flip UVs").color(text_primary),
                    );

                    ui.checkbox(
                        &mut state.settings.generate_normals,
                        egui::RichText::new("Generate normals if missing").color(text_primary),
                    );
                }

                ui.add_space(12.0);

                // --- Extract ---
                // Per-type toggles for what gets pulled out of the source file
                // and written next to the GLB. Mesh is always extracted; the
                // rest are opt-in/out so users can skip parts of an asset.
                section_label(ui, regular::PUZZLE_PIECE, "Extract", text_primary);
                ui.add_space(4.0);
                {
                    let mut state = world.resource_mut::<ImportOverlayState>();
                    ui.checkbox(
                        &mut state.settings.extract_skeleton,
                        egui::RichText::new("Skeleton + skin weights").color(text_primary),
                    );
                    ui.checkbox(
                        &mut state.settings.extract_animations,
                        egui::RichText::new("Animations (→ animations/*.anim)").color(text_primary),
                    );
                    ui.checkbox(
                        &mut state.settings.extract_textures,
                        egui::RichText::new("Textures (→ textures/*.png)").color(text_primary),
                    );
                    ui.checkbox(
                        &mut state.settings.extract_materials,
                        egui::RichText::new("Materials (→ materials/*.material)").color(text_primary),
                    );
                }

                ui.add_space(12.0);

                // --- Mesh Optimization ---
                section_label(ui, regular::CUBE, "Mesh Optimization", text_primary);
                ui.add_space(4.0);

                {
                    let mut state = world.resource_mut::<ImportOverlayState>();

                    ui.checkbox(
                        &mut state.settings.optimize_vertex_cache,
                        egui::RichText::new("Optimize vertex cache").color(text_primary),
                    );
                    ui.checkbox(
                        &mut state.settings.optimize_overdraw,
                        egui::RichText::new("Optimize overdraw").color(text_primary),
                    );
                    ui.checkbox(
                        &mut state.settings.optimize_vertex_fetch,
                        egui::RichText::new("Optimize vertex fetch").color(text_primary),
                    );
                }

                ui.add_space(12.0);

                // --- Destination ---
                section_label(ui, regular::FOLDER_OPEN, "Destination", text_primary);
                ui.add_space(4.0);

                {
                    let mut state = world.resource_mut::<ImportOverlayState>();
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("assets/")
                                .size(12.0)
                                .color(text_secondary),
                        );
                        let text_edit = egui::TextEdit::singleline(&mut state.target_directory)
                            .hint_text("e.g. models/imported")
                            .desired_width(ui.available_width() - 80.0);
                        ui.add(text_edit);
                    });
                }

                let browse_clicked = ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(format!("{} Browse", regular::FOLDER))
                                .size(11.0),
                        )
                        .fill(surface_mid),
                    )
                    .clicked();

                if browse_clicked {
                    let project = world.resource::<CurrentProject>();
                    let project_dir = &project.path;
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Select destination folder")
                        .set_directory(project_dir)
                        .pick_folder()
                    {
                        if let Ok(rel) = path.strip_prefix(project_dir) {
                            world.resource_mut::<ImportOverlayState>().target_directory =
                                rel.to_string_lossy().replace('\\', "/");
                        }
                    }
                }

                let (progress, can_import, log_entries) = {
                    let state = world.resource::<ImportOverlayState>();
                    let progress = state.progress.clone();
                    let can = !state.pending_files.is_empty()
                        && state.active_task.is_none()
                        && matches!(
                            progress,
                            ImportProgress::Idle | ImportProgress::Done(_) | ImportProgress::Error(_)
                        );
                    (progress, can, state.log_entries.clone())
                };

                ui.add_space(16.0);

                // --- Progress / status ---
                match &progress {
                    ImportProgress::Idle => {}
                    ImportProgress::Working {
                        current,
                        total,
                        label,
                    } => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(
                                egui::RichText::new(format!("[{}/{}] {}", current, total, label))
                                    .color(text_secondary),
                            );
                        });
                        let frac = *current as f32 / (*total as f32).max(1.0);
                        ui.add(egui::ProgressBar::new(frac).show_percentage());
                    }
                    ImportProgress::Done(msg) => {
                        ui.label(
                            egui::RichText::new(format!("{} {}", regular::CHECK_CIRCLE, msg))
                                .color(success_color),
                        );
                    }
                    ImportProgress::Error(msg) => {
                        ui.label(
                            egui::RichText::new(format!("{} {}", regular::WARNING, msg))
                                .color(error_color),
                        );
                    }
                }

                // --- Output log ---
                if !log_entries.is_empty() {
                    ui.add_space(8.0);
                    section_label(ui, regular::LIST_BULLETS, "Output", text_primary);
                    ui.add_space(4.0);

                    let log_frame = egui::Frame::new()
                        .fill(surface_mid)
                        .stroke(egui::Stroke::new(1.0, border_color))
                        .corner_radius(4.0)
                        .inner_margin(egui::Margin::same(8));

                    log_frame.show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        egui::ScrollArea::vertical()
                            .id_salt("import_output_log")
                            .max_height(120.0)
                            .show(ui, |ui| {
                                for entry in &log_entries {
                                    ui.horizontal(|ui| {
                                        if entry.success {
                                            ui.label(
                                                egui::RichText::new(regular::CHECK_CIRCLE)
                                                    .size(11.0)
                                                    .color(success_color),
                                            );
                                        } else {
                                            ui.label(
                                                egui::RichText::new(regular::WARNING)
                                                    .size(11.0)
                                                    .color(error_color),
                                            );
                                        }
                                        ui.label(
                                            egui::RichText::new(&entry.file_name)
                                                .size(11.0)
                                                .color(text_primary),
                                        );
                                        ui.label(
                                            egui::RichText::new(&entry.message)
                                                .size(11.0)
                                                .color(if entry.success { text_secondary } else { error_color }),
                                        );
                                    });
                                }
                            });
                    });
                }

                ui.add_space(8.0);

                // --- Buttons ---
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Import button
                        let button = egui::Button::new(
                            egui::RichText::new(format!("{} Import", regular::DOWNLOAD_SIMPLE))
                                .size(14.0)
                                .color(egui::Color32::WHITE),
                        )
                        .fill(if can_import { accent } else { surface_mid })
                        .min_size(egui::vec2(100.0, 32.0));

                        if ui.add_enabled(can_import, button).clicked() {
                            run_import(world);
                        }

                        // Cancel button
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("Cancel")
                                        .size(13.0)
                                        .color(text_primary),
                                )
                                .fill(surface_mid)
                                .min_size(egui::vec2(80.0, 32.0)),
                            )
                            .clicked()
                        {
                            close_overlay(world);
                        }
                    });
                });
            });
        });
}

fn section_label(ui: &mut egui::Ui, icon: &str, label: &str, color: egui::Color32) {
    ui.label(
        egui::RichText::new(format!("{} {}", icon, label))
            .size(13.0)
            .color(color)
            .strong(),
    );
}

fn close_overlay(world: &mut World) {
    let mut state = world.resource_mut::<ImportOverlayState>();
    state.visible = false;
    state.pending_files.clear();
    state.progress = ImportProgress::Idle;
    state.log_entries.clear();
    state.active_task = None;
}

pub(crate) fn run_import(world: &mut World) {
    let project = world.resource::<CurrentProject>().clone();
    let state = world.resource::<ImportOverlayState>();
    let files = state.pending_files.clone();
    let settings = state.settings.clone();
    let target_dir = state.target_directory.clone();

    let total = files.len();
    info!(
        "[import] Starting import of {} file(s) to assets/{}",
        total, target_dir
    );

    let (tx, rx) = mpsc::channel();

    // Set initial progress and store the task handle
    {
        let mut state = world.resource_mut::<ImportOverlayState>();
        state.log_entries.clear();
        state.progress = ImportProgress::Working {
            current: 0,
            total,
            label: "Starting...".into(),
        };
        state.active_task = Some(ImportTask {
            rx: Mutex::new(rx),
        });
    }

    // Spawn background thread
    std::thread::spawn(move || {
        import_worker(tx, project, files, settings, target_dir);
    });
}

/// Pick a model-folder name under `dest` that doesn't already exist.
/// Returns (`name`, `dest/name`). If `base` is taken, tries `base1`, `base2`, …
fn unique_model_dir(dest: &std::path::Path, base: &str) -> (String, PathBuf) {
    let candidate = dest.join(base);
    if !candidate.exists() {
        return (base.to_string(), candidate);
    }
    for i in 1..u32::MAX {
        let name = format!("{}{}", base, i);
        let path = dest.join(&name);
        if !path.exists() {
            return (name, path);
        }
    }
    (base.to_string(), dest.join(base))
}

/// Background import worker — runs on a separate thread.
fn import_worker(
    tx: mpsc::Sender<ImportMsg>,
    project: CurrentProject,
    files: Vec<PathBuf>,
    settings: ImportSettings,
    target_dir: String,
) {
    let total = files.len();
    let dest = project.path.join(&target_dir);

    if let Err(e) = std::fs::create_dir_all(&dest) {
        let _ = tx.send(ImportMsg::Error(format!(
            "Failed to create directory: {}",
            e
        )));
        return;
    }

    let mut imported = 0usize;
    let mut errors = Vec::new();
    let mut all_warnings = Vec::new();

    for (i, source_path) in files.iter().enumerate() {
        let file_name = source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // --- Phase: converting ---
        let _ = tx.send(ImportMsg::Progress {
            current: i + 1,
            total,
            label: format!("Converting {}", file_name),
        });

        match renzora_import::convert_to_glb(source_path, &settings) {
            Ok(result) => {
                // --- Phase: optimizing ---
                let opt_settings = MeshOptSettings {
                    vertex_cache: settings.optimize_vertex_cache,
                    overdraw: settings.optimize_overdraw,
                    vertex_fetch: settings.optimize_vertex_fetch,
                    ..Default::default()
                };

                let glb_bytes = if opt_settings.any_enabled() {
                    let _ = tx.send(ImportMsg::Progress {
                        current: i + 1,
                        total,
                        label: format!("Optimizing {}", file_name),
                    });

                    match renzora_import::optimize_glb(&result.glb_bytes, &opt_settings) {
                        Ok(optimized) => optimized,
                        Err(e) => {
                            warn!(
                                "[import] Mesh optimization failed for {}: {}",
                                file_name, e
                            );
                            result.glb_bytes.clone()
                        }
                    }
                } else {
                    result.glb_bytes.clone()
                };

                // --- Phase: writing ---
                // Each imported model gets its own folder so all derived
                // assets (animations, textures, materials) live together.
                let base_stem = source_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("model");
                let (stem_owned, model_dir) = unique_model_dir(&dest, base_stem);
                let stem: &str = &stem_owned;
                if let Err(e) = std::fs::create_dir_all(&model_dir) {
                    let msg = format!("failed to create model folder: {}", e);
                    errors.push(format!("{}: {}", file_name, msg));
                    let _ = tx.send(ImportMsg::Log(ImportLogEntry {
                        file_name: file_name.clone(),
                        success: false,
                        message: msg,
                    }));
                    continue;
                }
                let output_path = model_dir.join(format!("{}.glb", stem));

                let size_kb = glb_bytes.len() as f64 / 1024.0;
                let warn_count = result.warnings.len();

                // Materials: fire a PbrMaterialExtracted event per material.
                // Any crate observing it (e.g. renzora_material) writes the
                // `.material` file; this overlay stays oblivious to the
                // material graph format.
                if settings.extract_materials && !result.extracted_materials.is_empty() {
                    let mat_dir = model_dir.join("materials");
                    let rewrite_uri = |uri: &Option<String>| -> Option<String> {
                        // Textures live under the model folder, which itself
                        // lives under `<target>/<stem>/`. Prefix the relative
                        // URI so consumers can resolve it from the project
                        // root.
                        uri.as_ref().map(|u| {
                            if target_dir.is_empty() {
                                format!("{}/{}", stem, u)
                            } else {
                                format!("{}/{}/{}", target_dir, stem, u)
                            }
                        })
                    };
                    for mat in &result.extracted_materials {
                        let _ = tx.send(ImportMsg::MaterialExtracted(
                            renzora::core::PbrMaterialExtracted {
                                name: mat.name.clone(),
                                output_dir: mat_dir.clone(),
                                base_color: mat.base_color,
                                metallic: mat.metallic,
                                roughness: mat.roughness,
                                emissive: mat.emissive,
                                base_color_texture: rewrite_uri(&mat.base_color_texture),
                                normal_texture: rewrite_uri(&mat.normal_texture),
                                metallic_roughness_texture: rewrite_uri(&mat.metallic_roughness_texture),
                                emissive_texture: rewrite_uri(&mat.emissive_texture),
                                occlusion_texture: rewrite_uri(&mat.occlusion_texture),
                                specular_glossiness_texture: rewrite_uri(&mat.specular_glossiness_texture),
                                alpha_mode: match mat.alpha_mode {
                                    renzora_import::ExtractedAlphaMode::Opaque => renzora::core::PbrAlphaMode::Opaque,
                                    renzora_import::ExtractedAlphaMode::Mask => renzora::core::PbrAlphaMode::Mask,
                                    renzora_import::ExtractedAlphaMode::Blend => renzora::core::PbrAlphaMode::Blend,
                                },
                                alpha_cutoff: mat.alpha_cutoff,
                                double_sided: mat.double_sided,
                            },
                        ));
                    }
                }

                // Write any embedded textures the converter pulled out of the
                // source (e.g. textures bundled inside an FBX). Failures here
                // surface as warnings rather than aborting the import.
                if settings.extract_textures && !result.extracted_textures.is_empty() {
                    let tex_dir = model_dir.join("textures");
                    if let Err(e) = std::fs::create_dir_all(&tex_dir) {
                        all_warnings
                            .push(format!("textures dir: {}", e));
                    } else {
                        for tex in &result.extracted_textures {
                            let tex_path =
                                tex_dir.join(format!("{}.{}", tex.name, tex.extension));
                            if let Err(e) = std::fs::write(&tex_path, &tex.data) {
                                all_warnings.push(format!(
                                    "texture '{}': {}",
                                    tex.name, e
                                ));
                            }
                        }
                    }
                }

                match std::fs::write(&output_path, &glb_bytes) {
                    Ok(()) => {
                        imported += 1;
                        let msg = if warn_count > 0 {
                            format!("{:.1} KB ({} warnings)", size_kb, warn_count)
                        } else {
                            format!("{:.1} KB", size_kb)
                        };
                        let _ = tx.send(ImportMsg::Log(ImportLogEntry {
                            file_name: file_name.clone(),
                            success: true,
                            message: msg,
                        }));

                        // --- Phase: extracting animations ---
                        if !settings.extract_animations {
                            // Skip animation extraction entirely when the
                            // user has opted out.
                            continue;
                        }

                        let _ = tx.send(ImportMsg::Progress {
                            current: i + 1,
                            total,
                            label: format!("Extracting animations from {}", file_name),
                        });

                        let anim_dir = model_dir.join("animations");
                        match renzora_import::extract_animations_from_glb(&glb_bytes, &anim_dir) {
                            Ok(anim_result) => {
                                for anim_path in &anim_result.written_files {
                                    let _ = tx.send(ImportMsg::Log(ImportLogEntry {
                                        file_name: std::path::Path::new(anim_path)
                                            .file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or("animation")
                                            .to_string(),
                                        success: true,
                                        message: "animation extracted".to_string(),
                                    }));
                                }
                                for w in &anim_result.warnings {
                                    all_warnings.push(w.clone());
                                }
                            }
                            Err(e) => {
                                all_warnings.push(format!("animation extraction: {}", e));
                            }
                        }
                    }
                    Err(e) => {
                        let msg = format!("write failed: {}", e);
                        errors.push(format!("{}: {}", stem, msg));
                        let _ = tx.send(ImportMsg::Log(ImportLogEntry {
                            file_name: file_name.clone(),
                            success: false,
                            message: msg,
                        }));
                    }
                }

                for w in &result.warnings {
                    warn!("[import] {}: {}", file_name, w);
                }
                all_warnings.extend(result.warnings);
            }
            Err(e) => {
                // If geometry conversion failed for an FBX file, still try
                // extracting animations directly (animation-only FBX files
                // have no mesh geometry). Animation-only imports still get
                // their own per-stem folder so clips stay grouped.
                let ext_lower = source_path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                let base_stem = source_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("model");
                let (stem_owned, fallback_model_dir) = unique_model_dir(&dest, base_stem);
                let stem: &str = &stem_owned;
                let anim_dir = fallback_model_dir.join("animations");

                let anim_fallback_result: Option<Result<_, String>> =
                    if !settings.extract_animations {
                        None
                    } else {
                        match ext_lower.as_str() {
                            "fbx" => Some(renzora_import::extract_animations_from_fbx(source_path, &anim_dir)),
                            "usd" | "usda" | "usdc" | "usdz" => {
                                Some(renzora_import::extract_animations_from_usd(source_path, &anim_dir))
                            }
                            "bvh" => Some(renzora_import::extract_animations_from_bvh(source_path, &anim_dir)),
                            _ => None,
                        }
                    };

                let mut fallback_note: Option<String> = None;
                if let Some(fb) = anim_fallback_result {
                    match fb {
                        Ok(anim_result) => {
                            if !anim_result.written_files.is_empty() {
                                imported += 1;
                                for anim_path in &anim_result.written_files {
                                    let _ = tx.send(ImportMsg::Log(ImportLogEntry {
                                        file_name: std::path::Path::new(anim_path)
                                            .file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or("animation")
                                            .to_string(),
                                        success: true,
                                        message: "animation extracted".to_string(),
                                    }));
                                }
                                for w in &anim_result.warnings {
                                    all_warnings.push(w.clone());
                                }
                                continue;
                            }
                            if !anim_result.warnings.is_empty() {
                                fallback_note = Some(format!(
                                    "animation fallback: {}",
                                    anim_result.warnings.join("; ")
                                ));
                            } else {
                                fallback_note =
                                    Some("animation fallback: no animation data found".into());
                            }
                        }
                        Err(fb_err) => {
                            fallback_note =
                                Some(format!("animation fallback failed: {}", fb_err));
                        }
                    }
                }

                let msg = if let Some(note) = fallback_note {
                    format!("{} ({})", e, note)
                } else {
                    format!("{}", e)
                };
                errors.push(format!("{}: {}", file_name, msg));
                let _ = tx.send(ImportMsg::Log(ImportLogEntry {
                    file_name: file_name.clone(),
                    success: false,
                    message: msg,
                }));
            }
        }
    }

    // Final message
    if errors.is_empty() {
        let warn_suffix = if all_warnings.is_empty() {
            String::new()
        } else {
            format!(" ({} warnings)", all_warnings.len())
        };
        let _ = tx.send(ImportMsg::Done(format!(
            "Imported {} file{} to assets/{}{}",
            imported,
            if imported == 1 { "" } else { "s" },
            target_dir,
            warn_suffix,
        )));
    } else {
        let _ = tx.send(ImportMsg::Error(format!(
            "Imported {}/{} — {} error(s)",
            imported,
            total,
            errors.len()
        )));
    }
}
