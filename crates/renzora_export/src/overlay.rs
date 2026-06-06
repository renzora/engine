//! Export overlay state + background worker. The native (bevy_ui) modal in
//! `native.rs` renders the UI and reuses everything here.

use std::sync::{mpsc, Mutex};

use bevy::prelude::*;
use renzora::core::{CurrentProject, WindowMode};
use renzora_import::optimize::MeshOptSettings;
use renzora_rpak::{
    pack_project_filtered, pack_project_with_progress, RpakPacker, SERVER_EXTENSIONS,
};

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
pub(crate) struct ExportTask {
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
    pub(crate) active_task: Option<ExportTask>,
    /// Available runtime-compatible plugins (scanned once).
    pub available_plugins: Vec<dynamic_plugin_loader::DynamicPluginInfo>,
    /// Which plugins are selected for export (by id).
    pub selected_plugins: std::collections::HashSet<String>,
    /// Whether plugins have been scanned yet.
    pub(crate) plugins_scanned: bool,
    /// Latest GitHub release info (for runtime downloads).
    pub release_info: Option<ReleaseInfo>,
    /// Background fetch of release manifest.
    release_fetch_rx: Option<Mutex<mpsc::Receiver<Result<ReleaseInfo, String>>>>,
    /// Whether release fetch has been kicked off.
    pub(crate) release_fetch_started: bool,
    /// Last error from release manifest fetch (if any).
    pub release_fetch_error: Option<String>,
    /// Active runtime download task.
    pub(crate) download_task: Option<DownloadTask>,
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
pub(crate) fn poll_export_task(world: &mut World) {
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
pub(crate) fn ensure_release_fetch(world: &mut World) {
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
pub(crate) fn poll_release_fetch(world: &mut World) {
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
pub(crate) fn poll_download_task(world: &mut World) {
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

pub(crate) fn run_export(world: &mut World, project_name: &str) {
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
