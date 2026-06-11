//! Import overlay UI — modal dialog for importing 3D models.

use std::path::PathBuf;
use std::sync::{mpsc, Mutex};

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
    pub pending_files: Vec<PathBuf>,
    pub target_directory: String,
    /// How imported files are organized under the destination folder.
    pub layout: ImportLayout,
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
            layout: ImportLayout::PerFileFolder,
            settings: ImportSettings::default(),
            progress: ImportProgress::Idle,
            log_entries: Vec::new(),
            active_task: None,
        }
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
                let base_stem = source_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("model");
                let (stem_owned, model_dir) = match layout {
                    ImportLayout::PerFileFolder => unique_model_dir(&dest, base_stem),
                    ImportLayout::Combined => (base_stem.to_string(), dest.clone()),
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
                        // root so consumers can resolve it. The model folder is
                        // `<target>/<stem>/` (PerFileFolder) or `<target>/`
                        // (Combined).
                        let prefix = match layout {
                            ImportLayout::PerFileFolder if target_dir.is_empty() => stem.to_string(),
                            ImportLayout::PerFileFolder => format!("{}/{}", target_dir, stem),
                            ImportLayout::Combined => target_dir.clone(),
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
                                emissive_texture: rewrite_uri(&mat.emissive_texture),
                                occlusion_texture: rewrite_uri(&mat.occlusion_texture),
                                specular_glossiness_texture: rewrite_uri(
                                    &mat.specular_glossiness_texture,
                                ),
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
                    ImportLayout::PerFileFolder => unique_model_dir(&dest, base_stem),
                    ImportLayout::Combined => (base_stem.to_string(), dest.clone()),
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
