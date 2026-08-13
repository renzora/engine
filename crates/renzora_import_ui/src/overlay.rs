//! Import overlay UI — modal dialog for importing 3D models.

use std::path::PathBuf;
use std::sync::{mpsc, Mutex};

use crate::kinds::QueuedAsset;
use bevy::prelude::*;
use renzora::core::CurrentProject;
use renzora_import::optimize::MeshOptSettings;
use renzora_import::settings::ImportSettings;

/// How imported files are laid out on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportLayout {
    /// Each source file gets its own `<stem>/` folder (default). Keeps a
    /// model's GLB plus its derived animations/, textures/ and materials/
    /// grouped together, isolated from other imports.
    PerFileFolder,
    /// All source files share the destination folder directly. Derived assets
    /// merge into single sibling `animations/`, `textures/` and `materials/`
    /// folders — handy when importing a batch of animation clips for one
    /// character (e.g. a folder of Mixamo FBX takes).
    Combined,
}

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
    // Boxed: PbrMaterialExtracted is much larger than the other variants.
    MaterialExtracted(Box<renzora::core::PbrMaterialExtracted>),
    /// Absolute path of a `.glb` that was just successfully written into
    /// the project. The poller forwards these to
    /// [`ModelThumbnailRegistry::request`] so the offscreen capture
    /// kicks off immediately — by the time the user closes the overlay
    /// and opens the asset browser, the PNG is already on disk.
    Imported(std::path::PathBuf),
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
    pub pending_files: Vec<QueuedAsset>,
    pub target_directory: String,
    /// How imported files are organized under the destination folder.
    pub layout: ImportLayout,
    pub settings: ImportSettings,
    pub progress: ImportProgress,
    /// Per-file import results shown in the output log.
    pub log_entries: Vec<ImportLogEntry>,
    /// Background import task (if running).
    pub(crate) active_task: Option<ImportTask>,
    /// True once an import has been launched from the overlay and the modal has
    /// been dismissed into the corner progress toast. While set, the toast
    /// system owns polling + lifecycle (so the silent drag-drop auto-import path
    /// in `lib.rs` stays out of the way).
    pub(crate) toast_active: bool,
    /// Wall-clock time (`Time::elapsed_secs_f64`) at which a finished toast
    /// should auto-dismiss. Set when the import reaches a terminal state.
    pub(crate) toast_dismiss_at: Option<f64>,
}

impl Default for ImportOverlayState {
    fn default() -> Self {
        Self {
            visible: false,
            pending_files: Vec::new(),
            target_directory: "models".to_string(),
            layout: ImportLayout::PerFileFolder,
            settings: ImportSettings::default(),
            progress: ImportProgress::Idle,
            log_entries: Vec::new(),
            active_task: None,
            toast_active: false,
            toast_dismiss_at: None,
        }
    }
}

impl ImportOverlayState {
    /// Append `assets` to the queue, skipping any whose source path is already
    /// queued, and auto-detect the unit scale when the queue starts empty.
    /// Returns whether anything new was added.
    ///
    /// Both entry points (the drop handler in `lib.rs` and the overlay's own
    /// Browse buttons in `native.rs`) funnel through here so they can't drift
    /// apart — they previously had two copies of this that disagreed about
    /// which file the scale is read from.
    ///
    /// The de-dup uses a set rather than a linear scan per item: a folder
    /// import can queue thousands of files at once, and the quadratic version
    /// spent that as several million `PathBuf` comparisons on the main thread.
    pub(crate) fn enqueue(&mut self, assets: &[QueuedAsset]) -> bool {
        if assets.is_empty() {
            return false;
        }
        let was_empty = self.pending_files.is_empty();
        let mut seen: std::collections::HashSet<&std::path::Path> =
            self.pending_files.iter().map(|q| q.path.as_path()).collect();
        let mut added = Vec::new();
        for asset in assets {
            if seen.insert(asset.path.as_path()) {
                added.push(asset.clone());
            }
        }
        if added.is_empty() {
            return false;
        }
        // Auto-detect the unit scale from the first *model* in a fresh queue.
        // Scanning for one matters for folder imports: the queue is sorted by
        // path, so entry zero is usually a texture, and `detect_unit_scale`
        // returns None for every non-model.
        if was_empty && self.settings.scale == 1.0 {
            if let Some(scale) = added
                .iter()
                .find_map(|q| renzora_import::units::detect_unit_scale(&q.path))
            {
                self.settings.scale = scale;
            }
        }
        // Clear a stale "No importable files in …" once the queue actually has
        // something, so the message line doesn't contradict the list under it.
        // Only while no toast is up: `manage_import_toast` waits on a terminal
        // progress state to auto-dismiss, and resetting it out from under a
        // live toast would strand it on screen.
        if !self.toast_active && matches!(self.progress, ImportProgress::Error(_)) {
            self.progress = ImportProgress::Idle;
        }
        self.pending_files.extend(added);
        true
    }
}

/// Drain progress messages from the background thread into overlay state.
pub(crate) fn poll_import_task(world: &mut World) {
    let has_task = world.resource::<ImportOverlayState>().active_task.is_some();
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
                    progress_updates.push(ImportMsg::Error(
                        "Import thread terminated unexpectedly".into(),
                    ));
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
    let mut imported_models: Vec<std::path::PathBuf> = Vec::new();
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
                    material_events.push(*ev);
                }
                ImportMsg::Imported(path) => {
                    imported_models.push(path);
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

    // Hand each freshly-imported `.glb` to the model thumbnail registry
    // so its offscreen capture pipeline starts immediately. By the time
    // the user closes the import overlay and opens the asset browser,
    // the cached PNG is already on disk.
    if !imported_models.is_empty() {
        if let Some(mut registry) =
            world.get_resource_mut::<renzora_editor_framework::ModelThumbnailRegistry>()
        {
            for path in imported_models {
                registry.request(path);
            }
        }
    }
}

pub(crate) fn close_overlay(world: &mut World) {
    let mut state = world.resource_mut::<ImportOverlayState>();
    state.visible = false;
    state.pending_files.clear();
    state.progress = ImportProgress::Idle;
    state.log_entries.clear();
    state.active_task = None;
    state.toast_active = false;
    state.toast_dismiss_at = None;
}

pub(crate) fn run_import(world: &mut World) {
    let project = world.resource::<CurrentProject>().clone();
    let state = world.resource::<ImportOverlayState>();
    let files = state.pending_files.clone();
    let settings = state.settings.clone();
    let target_dir = state.target_directory.clone();
    let layout = state.layout;

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
        state.active_task = Some(ImportTask { rx: Mutex::new(rx) });
    }

    // Spawn background thread
    std::thread::spawn(move || {
        import_worker(tx, project, files, settings, target_dir, layout);
    });
}

/// Join `dest` with a forward-slashed relative directory from a folder import.
///
/// `.` and `..` segments are dropped rather than walked. A filesystem walk
/// can't produce them, but `QueuedAsset::relative_dir` is a public field on a
/// public resource, so anything in the editor can push an entry — and joining
/// a `..` here would write outside the project.
fn dest_with_rel(dest: &std::path::Path, relative_dir: &str) -> PathBuf {
    if relative_dir.is_empty() {
        return dest.to_path_buf();
    }
    relative_dir
        .split('/')
        .filter(|s| !s.is_empty() && *s != "." && *s != "..")
        .fold(dest.to_path_buf(), |acc, seg| acc.join(seg))
}

/// Project-relative forward-slashed prefix for a file landing under `target_dir`
/// (plus an optional mirrored source subdirectory from a folder import).
fn project_prefix(target_dir: &str, relative_dir: &str) -> String {
    match (target_dir.is_empty(), relative_dir.is_empty()) {
        (true, true) => String::new(),
        (true, false) => relative_dir.to_string(),
        (false, true) => target_dir.to_string(),
        (false, false) => format!("{target_dir}/{relative_dir}"),
    }
}

/// Pick a file path under `dest` that doesn't collide. If `name` is taken,
/// inserts `1`, `2`, … before the extension (`tex.png` → `tex1.png`). Used by
/// the copy path for non-model assets, which land directly in the destination
/// folder rather than in a per-model `<stem>/` subfolder.
fn unique_file(dest: &std::path::Path, name: &str) -> PathBuf {
    let candidate = dest.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let path = std::path::Path::new(name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let ext = path.extension().and_then(|e| e.to_str());
    for i in 1..u32::MAX {
        let cand = match ext {
            Some(e) => format!("{}{}.{}", stem, i, e),
            None => format!("{}{}", stem, i),
        };
        let p = dest.join(cand);
        if !p.exists() {
            return p;
        }
    }
    dest.join(name)
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
    files: Vec<QueuedAsset>,
    settings: ImportSettings,
    target_dir: String,
    layout: ImportLayout,
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

    for (i, item) in files.iter().enumerate() {
        let source_path = &item.path;
        let file_name = source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Folder imports recreate the source tree under `dest`; single-file
        // picks land flat in `dest` (relative_dir empty).
        let file_dest = dest_with_rel(&dest, &item.relative_dir);
        if let Err(e) = std::fs::create_dir_all(&file_dest) {
            let msg = format!("failed to create folder: {}", e);
            errors.push(format!("{}: {}", file_name, msg));
            let _ = tx.send(ImportMsg::Log(ImportLogEntry {
                file_name: file_name.clone(),
                success: false,
                message: msg,
            }));
            continue;
        }
        let target_prefix = project_prefix(&target_dir, &item.relative_dir);

        // Non-model assets have no conversion step. "Importing" one just copies
        // it verbatim into the destination folder the user picked — images,
        // audio, `.bsn`, `.particle`, `.material`, fonts and scripts all take
        // this path. The layout / extract / optimize options are model-only, so
        // copies always land directly in `file_dest` (no per-stem subfolder).
        if renzora_import::formats::detect_format(source_path).is_none() {
            let _ = tx.send(ImportMsg::Progress {
                current: i + 1,
                total,
                label: format!("Copying {}", file_name),
            });
            let out = unique_file(&file_dest, &file_name);
            match std::fs::copy(source_path, &out) {
                Ok(bytes) => {
                    imported += 1;
                    let _ = tx.send(ImportMsg::Log(ImportLogEntry {
                        file_name: file_name.clone(),
                        success: true,
                        message: format!("{:.1} KB", bytes as f64 / 1024.0),
                    }));
                }
                Err(e) => {
                    let msg = format!("copy failed: {}", e);
                    errors.push(format!("{}: {}", file_name, msg));
                    let _ = tx.send(ImportMsg::Log(ImportLogEntry {
                        file_name: file_name.clone(),
                        success: false,
                        message: msg,
                    }));
                }
            }
            continue;
        }

        // --- Phase: converting ---
        let _ = tx.send(ImportMsg::Progress {
            current: i + 1,
            total,
            label: format!("Converting {}", file_name),
        });

        // Per-texture progress: texture baking (decode + mip + BC compression)
        // dominates import time for texture-heavy models, so surface it as a
        // moving "[done/total] Compressing textures: …" bar rather than
        // letting the file-level bar sit at 100% for minutes. Textures bake in
        // parallel, so the callback is invoked from multiple threads — wrap the
        // sender in a Mutex to make the closure `Sync`.
        let tex_tx = Mutex::new(tx.clone());
        let on_texture = move |done: usize, tex_total: usize, name: &str| {
            if let Ok(sender) = tex_tx.lock() {
                let _ = sender.send(ImportMsg::Progress {
                    current: done,
                    total: tex_total,
                    label: format!("Compressing textures: {}", name),
                });
            }
        };

        match renzora_import::convert_to_glb_with_progress(source_path, &settings, &on_texture) {
            Ok(result) => {
                // --- Phase: optimizing ---
                let opt_settings = MeshOptSettings {
                    vertex_cache: settings.optimize_vertex_cache,
                    overdraw: settings.optimize_overdraw,
                    vertex_fetch: settings.optimize_vertex_fetch,
                    ..Default::default()
                };

                let mut glb_bytes = if opt_settings.any_enabled() {
                    let _ = tx.send(ImportMsg::Progress {
                        current: i + 1,
                        total,
                        label: format!("Optimizing {}", file_name),
                    });

                    match renzora_import::optimize_glb(&result.glb_bytes, &opt_settings) {
                        Ok(optimized) => optimized,
                        Err(e) => {
                            warn!("[import] Mesh optimization failed for {}: {}", file_name, e);
                            result.glb_bytes.clone()
                        }
                    }
                } else {
                    result.glb_bytes.clone()
                };

                // --- Phase: writing ---
                // Where the GLB and its derived assets (animations, textures,
                // materials) land depends on the chosen layout:
                //   PerFileFolder — each model gets its own `<stem>/` folder so
                //     its assets stay isolated from other imports.
                //   Combined — every file writes straight into the destination,
                //     so derived assets merge into shared sibling folders.
                // Both are rooted at `file_dest` (mirrors folder imports).
                let base_stem = source_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("model");
                let (stem_owned, model_dir) = match layout {
                    ImportLayout::PerFileFolder => unique_model_dir(&file_dest, base_stem),
                    ImportLayout::Combined => (base_stem.to_string(), file_dest.clone()),
                };
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

                let warn_count = result.warnings.len();

                // Materials: fire a PbrMaterialExtracted event per material.
                // The observer in renzora_shader::material writes the
                // `.material` file; this overlay stays oblivious to the
                // material graph format.
                if settings.extract_materials && !result.extracted_materials.is_empty() {
                    let mat_dir = model_dir.join("materials");
                    let rewrite_uri = |uri: &Option<String>| -> Option<String> {
                        // Textures live under the model folder. Prefix the
                        // relative URI with that folder's path from the project
                        // root so consumers can resolve it.
                        let prefix = match layout {
                            ImportLayout::PerFileFolder if target_prefix.is_empty() => {
                                stem.to_string()
                            }
                            ImportLayout::PerFileFolder => {
                                format!("{}/{}", target_prefix, stem)
                            }
                            ImportLayout::Combined => target_prefix.clone(),
                        };
                        uri.as_ref().map(|u| {
                            if prefix.is_empty() {
                                u.clone()
                            } else {
                                format!("{}/{}", prefix, u)
                            }
                        })
                    };
                    for mat in &result.extracted_materials {
                        let _ = tx.send(ImportMsg::MaterialExtracted(Box::new(
                            renzora::core::PbrMaterialExtracted {
                                name: mat.name.clone(),
                                output_dir: mat_dir.clone(),
                                project_root: project.path.clone(),
                                base_color: mat.base_color,
                                metallic: mat.metallic,
                                roughness: mat.roughness,
                                emissive: mat.emissive,
                                base_color_texture: rewrite_uri(&mat.base_color_texture),
                                normal_texture: rewrite_uri(&mat.normal_texture),
                                metallic_roughness_texture: rewrite_uri(
                                    &mat.metallic_roughness_texture,
                                ),
                                roughness_texture: rewrite_uri(&mat.roughness_texture),
                                metallic_texture: rewrite_uri(&mat.metallic_texture),
                                emissive_texture: rewrite_uri(&mat.emissive_texture),
                                occlusion_texture: rewrite_uri(&mat.occlusion_texture),
                                specular_glossiness_texture: rewrite_uri(
                                    &mat.specular_glossiness_texture,
                                ),
                                opacity_texture: rewrite_uri(&mat.opacity_texture),
                                specular_texture: rewrite_uri(&mat.specular_texture),
                                advanced: mat.advanced.rewrite_textures(rewrite_uri),
                                alpha_mode: match mat.alpha_mode {
                                    renzora_import::ExtractedAlphaMode::Opaque => {
                                        renzora::core::PbrAlphaMode::Opaque
                                    }
                                    renzora_import::ExtractedAlphaMode::Mask => {
                                        renzora::core::PbrAlphaMode::Mask
                                    }
                                    renzora_import::ExtractedAlphaMode::Blend => {
                                        renzora::core::PbrAlphaMode::Blend
                                    }
                                },
                                alpha_cutoff: mat.alpha_cutoff,
                                double_sided: mat.double_sided,
                            },
                        )));
                    }
                }

                // Write any embedded textures the converter pulled out of the
                // source (e.g. textures bundled inside an FBX). Failures here
                // surface as warnings rather than aborting the import.
                if settings.extract_textures && !result.extracted_textures.is_empty() {
                    let tex_dir = model_dir.join("textures");
                    if let Err(e) = std::fs::create_dir_all(&tex_dir) {
                        all_warnings.push(format!("textures dir: {}", e));
                    } else {
                        for tex in &result.extracted_textures {
                            let tex_path = tex_dir.join(format!("{}.{}", tex.name, tex.extension));
                            if let Err(e) = std::fs::write(&tex_path, &tex.data) {
                                all_warnings.push(format!("texture '{}': {}", tex.name, e));
                            }
                        }
                    }
                }

                // --- Phase: extract animations, then compact the GLB ---
                // Conversion is additive — it leaves the source's embedded
                // animation keyframes and (after texture extraction) the now-
                // orphaned image bytes dead inside the GLB's binary chunk. Pull
                // animations out to `.anim` first (the runtime plays those, not
                // the GLB's embedded clips), then garbage-collect the buffer so
                // the on-disk model carries only live geometry/skins.
                if settings.extract_animations {
                    let _ = tx.send(ImportMsg::Progress {
                        current: i + 1,
                        total,
                        label: format!("Extracting animations from {}", file_name),
                    });
                    let anim_dir = model_dir.join("animations");
                    // Where the keyframes live depends on the source format:
                    // glTF/GLB conversion is passthrough, so its animations are
                    // still embedded in the GLB we just built — but the FBX/USD
                    // converters only emit geometry, skins and materials, so
                    // their animations exist *only* in the source file and must
                    // be sampled from there.
                    let src_ext = source_path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    let anim_result = match src_ext.as_str() {
                        "fbx" => renzora_import::extract_animations_from_fbx(
                            source_path,
                            &anim_dir,
                            &settings,
                        ),
                        "usd" | "usda" | "usdc" | "usdz" => {
                            renzora_import::extract_animations_from_usd(source_path, &anim_dir)
                        }
                        _ => renzora_import::extract_animations_from_glb(&glb_bytes, &anim_dir),
                    };
                    match anim_result {
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

                // Reclaim the dead bytes: orphaned embedded textures (their
                // pixels now live in external `textures/*.png`) and, when split
                // out above, animation keyframes. Skipped entirely when the user
                // extracts neither, so a passthrough import stays byte-for-byte.
                if settings.extract_textures || settings.extract_animations {
                    match renzora_import::compact_glb(&glb_bytes, settings.extract_animations) {
                        Ok(compacted) => glb_bytes = compacted,
                        Err(e) => {
                            warn!("[import] GLB compaction failed for {}: {}", file_name, e);
                            all_warnings.push(format!("GLB compaction: {}", e));
                        }
                    }
                }

                let size_kb = glb_bytes.len() as f64 / 1024.0;

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
                        // Kick off the thumbnail capture as soon as the
                        // GLB is on disk. The model isn't yet in the
                        // asset browser — the registry will load + spawn
                        // it offscreen, capture, and write the PNG cache.
                        let _ = tx.send(ImportMsg::Imported(output_path.clone()));
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
                let (stem_owned, fallback_model_dir) = match layout {
                    ImportLayout::PerFileFolder => unique_model_dir(&file_dest, base_stem),
                    ImportLayout::Combined => (base_stem.to_string(), file_dest.clone()),
                };
                let _stem: &str = &stem_owned;
                let anim_dir = fallback_model_dir.join("animations");

                let anim_fallback_result: Option<Result<_, String>> =
                    if !settings.extract_animations {
                        None
                    } else {
                        match ext_lower.as_str() {
                            "fbx" => Some(renzora_import::extract_animations_from_fbx(
                                source_path,
                                &anim_dir,
                                &settings,
                            )),
                            "usd" | "usda" | "usdc" | "usdz" => Some(
                                renzora_import::extract_animations_from_usd(source_path, &anim_dir),
                            ),
                            "bvh" => Some(renzora_import::extract_animations_from_bvh(
                                source_path,
                                &anim_dir,
                            )),
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
                            fallback_note = Some(format!("animation fallback failed: {}", fb_err));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dest_with_rel_mirrors_the_source_tree() {
        let dest = std::path::Path::new("proj").join("models");
        assert_eq!(dest_with_rel(&dest, ""), dest);
        assert_eq!(
            dest_with_rel(&dest, "Pack/textures"),
            dest.join("Pack").join("textures")
        );
        // Empty and traversal segments are dropped, never walked.
        assert_eq!(dest_with_rel(&dest, "Pack//textures"), dest.join("Pack").join("textures"));
        assert_eq!(dest_with_rel(&dest, "../../etc"), dest.join("etc"));
        assert_eq!(dest_with_rel(&dest, "./Pack"), dest.join("Pack"));
    }

    #[test]
    fn project_prefix_covers_every_target_relative_combination() {
        // Project root + single-file pick: no prefix at all.
        assert_eq!(project_prefix("", ""), "");
        // Project root + folder import: the mirrored subtree is the prefix.
        assert_eq!(project_prefix("", "Pack/textures"), "Pack/textures");
        // Chosen target + single-file pick: the old (pre-folder) behaviour.
        assert_eq!(project_prefix("models", ""), "models");
        // Both — the case that decides where extracted material textures
        // resolve from, so it's the one worth pinning down.
        assert_eq!(project_prefix("models", "Pack/textures"), "models/Pack/textures");
    }
}
