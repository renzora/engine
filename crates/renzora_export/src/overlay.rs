//! Export overlay state + background worker. The native (bevy_ui) modal in
//! `native.rs` renders the UI and reuses everything here.

use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc, Mutex};

use bevy::prelude::*;
use renzora::core::{CurrentProject, WindowMode};
use renzora_import::optimize::MeshOptSettings;
use renzora_rpak::{
    pack_project_filtered, pack_project_with_progress, RpakPacker, SERVER_EXTENSIONS,
};

use crate::download::{self, DownloadProgress, DownloadTask, ReleaseInfo};
use crate::templates::{Platform, TemplateManager};

/// Packaging mode for the exported build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PackagingMode {
    /// Copy the dev runtime binary + its dylibs, write a sibling .rpak.
    SeparateFiles,
    /// Copy the dev runtime binary + its dylibs, .rpak appended to the binary.
    SingleBinary,
    /// Recompile a lean, fully static, stripped binary from source and append
    /// the .rpak to it — one self-contained file, no sibling dylibs. Needs a
    /// Rust toolchain (auto-provisioned if absent). See `build`/`toolchain`.
    LeanSingleBinary,
}

/// How the game's C-ABI plugins reach the exported build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum PluginLinkMode {
    /// Copy each plugin's library into a `plugins/` folder beside the binary,
    /// where the host finds and loads it at startup. Works with every packaging
    /// mode, and leaves the set changeable after shipping — a player or a mod
    /// can add one.
    #[default]
    ShipFiles,
    /// Compile the plugins into the binary. One file to ship and nothing to load
    /// at boot, at the cost of the folder being editable afterwards.
    ///
    /// Only possible for [`PackagingMode::LeanSingleBinary`], which is the only
    /// mode that compiles anything — the other two copy an already-built runtime,
    /// and no amount of packaging can put new code inside it.
    LinkIn,
}

/// Which view the export modal shows: the settings form, or the live build log.
/// Clicking Export switches to [`ExportView::Log`]; finishing + Back returns to
/// [`ExportView::Settings`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExportView {
    #[default]
    Settings,
    Log,
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
    /// Set by the Cancel button; the worker (and the cargo child it spawned)
    /// watch this and abort the build.
    pub(crate) cancel: Arc<AtomicBool>,
}

/// Resource holding the export overlay state.
#[derive(Resource)]
pub struct ExportOverlayState {
    pub visible: bool,
    /// Settings form vs. live build-log view.
    pub view: ExportView,
    /// Accumulated build/export output lines for the terminal log (tail-capped).
    pub build_log: Vec<String>,
    /// Count of crates compiled so far (parsed from cargo's "Compiling …" lines),
    /// used to estimate the progress bar.
    pub build_compiled: u32,
    /// Whether the build reached "Finished" / Done (progress bar → full).
    pub build_finished: bool,
    /// Saved export configurations for the open project, in list order.
    ///
    /// The settings below are the *working copy* — whichever preset is selected
    /// has been applied into them, and edits land there rather than in the
    /// preset. [`ExportOverlayState::sync_active_preset`] copies them back, and
    /// is called before anything that would lose them (switching preset,
    /// closing, exporting).
    pub presets: Vec<crate::presets::ExportPreset>,
    /// Index into `presets`, or `None` when the project has none yet.
    pub active_preset: Option<usize>,
    /// Project the loaded presets belong to, so opening a different project
    /// reloads rather than showing the previous one's list.
    pub(crate) presets_loaded_for: Option<std::path::PathBuf>,
    /// Result of the last Docker probe, and the platform it was run for.
    ///
    /// Probing spawns a process, so it happens when the modal opens or the
    /// selected platform changes — never per frame.
    pub docker: Option<crate::docker::DockerStatus>,
    pub(crate) docker_probed_for: Option<Platform>,

    pub platform: Platform,
    pub packaging_mode: PackagingMode,
    pub window_mode: WindowMode,
    pub window_width: u32,
    pub window_height: u32,
    pub console_logging: bool,
    pub compression_level: i32,
    /// Pack the shipped executable (and any sibling libraries) with UPX.
    ///
    /// Off by default: it needs a `upx` on the machine, and a self-extracting
    /// binary is the one size lever with a runtime cost — a slower cold start and
    /// a standing risk of a heuristic antivirus flagging the packer stub. See
    /// [`crate::upx`].
    pub upx_compress: bool,
    pub icon_path: Option<String>,
    pub include_server: bool,
    /// Ship the plugin SDK so players can add native plugins and Rust scripts to
    /// the exported game.
    ///
    /// On by default. A moddable game is the norm for this engine — the plugin
    /// system is the same one the editor uses — and the cost of getting the
    /// default wrong points one way: a game shipped without it cannot be modded
    /// at all, while a game shipped with it is merely larger.
    pub enable_modding: bool,
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
    pub available_plugins: Vec<renzora_plugin::host::loader::PluginInfo>,
    /// Which plugins are selected for export (by id).
    pub selected_plugins: std::collections::HashSet<String>,
    /// Files beside the binary, or compiled into it. See [`PluginLinkMode`].
    pub plugin_link_mode: PluginLinkMode,
    /// Engine capability toggles (id → on). Off ⇒ its Bevy features are stripped
    /// from the lean build. Populated with defaults after the plugin scan.
    pub capabilities: std::collections::HashMap<String, bool>,
    /// Features-tab sections the user has folded shut, by section id.
    ///
    /// Collapse is reactive (`bind_display` per row) rather than a rebuild, so
    /// folding a section doesn't respawn 60 checkboxes and lose scroll position.
    /// Absent = expanded, so a fresh state shows everything.
    pub collapsed_sections: std::collections::HashSet<String>,
    /// Whether plugins have been scanned yet.
    pub(crate) plugins_scanned: bool,
    /// Platform the current plugin list was scanned for.
    ///
    /// The available plugins are per-platform now that cross-platform templates
    /// exist: a downloaded Linux template brings its own `.so` plugins, and
    /// listing the editor's `.dll`s while exporting for Linux would offer
    /// libraries the game cannot load. Changing platform re-scans.
    pub(crate) plugins_scanned_for: Option<Platform>,
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
            presets: Vec::new(),
            active_preset: None,
            presets_loaded_for: None,
            docker: None,
            docker_probed_for: None,
            view: ExportView::Settings,
            build_log: Vec::new(),
            build_compiled: 0,
            build_finished: false,
            platform: Platform::current().unwrap_or(Platform::WindowsX64),
            packaging_mode: PackagingMode::SeparateFiles,
            window_mode: WindowMode::Windowed,
            window_width: 1280,
            window_height: 720,
            console_logging: false,
            compression_level: 3,
            upx_compress: false,
            icon_path: None,
            include_server: false,
            enable_modding: true,
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
            plugin_link_mode: PluginLinkMode::default(),
            capabilities: std::collections::HashMap::new(),
            collapsed_sections: std::collections::HashSet::new(),
            plugins_scanned: false,
            plugins_scanned_for: None,
            release_info: None,
            release_fetch_rx: None,
            release_fetch_started: false,
            release_fetch_error: None,
            download_task: None,
            download_status: None,
        }
    }
}

impl ExportOverlayState {
    /// Load this project's presets, if they aren't loaded already.
    ///
    /// Selects the first preset and applies it, so opening the modal lands on a
    /// working configuration rather than on whatever the last session left in
    /// the fields. A project with no presets yet gets `active_preset: None`,
    /// which the UI renders as an empty state inviting one to be added.
    pub fn load_presets(&mut self, project_root: &std::path::Path) {
        if self.presets_loaded_for.as_deref() == Some(project_root) {
            return;
        }
        self.presets = crate::presets::load(project_root);
        self.presets_loaded_for = Some(project_root.to_path_buf());
        self.active_preset = if self.presets.is_empty() { None } else { Some(0) };
        if let Some(p) = self.active_preset.and_then(|i| self.presets.get(i)).cloned() {
            p.apply(self);
        }
    }

    /// Copy the working settings back into the selected preset.
    ///
    /// Every edit in the modal writes to the flat fields rather than into the
    /// preset — the widgets were built that way and rebinding all of them
    /// through an index would be a large change for no gain — so this is what
    /// makes an edit durable. Call it before anything that would otherwise lose
    /// the edits: switching preset, closing the modal, starting an export.
    pub fn sync_active_preset(&mut self) {
        let Some(i) = self.active_preset else { return };
        let Some(name) = self.presets.get(i).map(|p| p.name.clone()) else { return };
        let updated = crate::presets::ExportPreset::capture(name, self);
        self.presets[i] = updated;
    }

    /// Persist the presets for the open project. Best-effort: a failure is
    /// logged rather than surfaced, because it must not block an export that is
    /// otherwise fine.
    pub fn save_presets(&self) {
        let Some(root) = self.presets_loaded_for.as_deref() else { return };
        if let Err(e) = crate::presets::save(root, &self.presets) {
            warn!("could not save export presets: {e}");
        }
    }

    /// Select a preset by index, keeping the outgoing one's edits.
    pub fn select_preset(&mut self, index: usize) {
        if self.active_preset == Some(index) || index >= self.presets.len() {
            return;
        }
        self.sync_active_preset();
        self.save_presets();
        self.active_preset = Some(index);
        if let Some(p) = self.presets.get(index).cloned() {
            p.apply(self);
        }
    }
}

/// Append a line to the terminal log, keeping a bounded tail so a long build
/// (thousands of cargo lines) can't grow the buffer unbounded.
fn push_log(state: &mut ExportOverlayState, line: String) {
    state.build_log.push(line);
    if state.build_log.len() > 600 {
        state.build_log.drain(0..200);
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
                // Track compile progress from cargo's own output for the bar.
                let trimmed = label.trim_start();
                if trimmed.starts_with("Compiling ") {
                    state.build_compiled += 1;
                }
                if trimmed.starts_with("Finished") {
                    state.build_finished = true;
                }
                push_log(&mut state, label.clone());
                state.progress = ExportProgress::Working(label);
            }
            ExportMsg::Done(msg) => {
                state.build_finished = true;
                push_log(&mut state, msg.clone());
                state.progress = ExportProgress::Done(msg);
            }
            ExportMsg::Error(msg) => {
                push_log(&mut state, format!("error: {msg}"));
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
    let enable_modding = export_state.enable_modding;
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
    // Kept as full `PluginInfo`s rather than bare paths: linking a plugin in
    // needs its id (to find the source that builds it) and its scope (so the
    // generated list declares the same one the library would have reported).
    let selected_plugins: Vec<renzora_plugin::host::loader::PluginInfo> = export_state
        .available_plugins
        .iter()
        .filter(|p| export_state.selected_plugins.contains(&p.id))
        .cloned()
        .collect();
    // Linking in is only meaningful for the mode that compiles. Silently
    // downgrading elsewhere is right rather than an error: the packaging radio is
    // the stronger statement of intent, and the UI says so next to the toggle.
    let link_plugins_in = export_state.plugin_link_mode == PluginLinkMode::LinkIn
        && packaging_mode == PackagingMode::LeanSingleBinary;
    // Bevy + runtime-subsystem features to strip from the lean build (capabilities
    // the game has off).
    let disabled_bevy_features =
        crate::capabilities::disabled_bevy_features(&export_state.capabilities);
    let disabled_runtime_features =
        crate::capabilities::disabled_runtime_features(&export_state.capabilities);
    let lean_profile = crate::capabilities::lean_profile(&export_state.capabilities);
    // UPX is post-build, so unlike the profile knobs it applies to the copy-based
    // packaging modes too — but only where the packer supports the format.
    let upx_compress = export_state.upx_compress && crate::upx::supports(platform);
    let project_name = project_name.to_string();

    // The game binary is the already-built renzora(.exe) for this platform.
    // Operation Merge: the editor's own binary IS the game — copy it (and the
    // shared libs sitting next to it) to the export dir. No download.
    let template_path = match world.resource::<TemplateManager>().get(platform) {
        Some(t) => t.path.clone(),
        None => {
            world.resource_mut::<ExportOverlayState>().progress = ExportProgress::Error(format!(
                "No build found for {} — build it first (`renzora build {}`).",
                platform.display_name(),
                platform.dist_dir_name()
            ));
            return;
        }
    };
    // The shared libs (bevy_dylib, renzora.dll, std) sit next to the binary.
    // The copy filter in the worker ships those but NOT renzora_editor.dll (the
    // editor bundle) or the editor's *.exe tools — so the export is a clean game.
    let runtime_dir = template_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    // The dedicated server reuses the game binary (run with `--server`), so
    // there's no separate server template to resolve here.

    let (tx, rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));

    // Switch to the live log view, reset its state, and store the task.
    {
        let mut state = world.resource_mut::<ExportOverlayState>();
        state.view = ExportView::Log;
        state.build_log.clear();
        state.build_compiled = 0;
        state.build_finished = false;
        state.progress = ExportProgress::Working("Packing assets...".into());
        state.active_task = Some(ExportTask {
            rx: Mutex::new(rx),
            cancel: cancel.clone(),
        });
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
            enable_modding,
            mesh_simplify,
            mesh_simplify_ratio,
            mesh_quantize,
            mesh_generate_lods,
            mesh_lod_levels,
            template_path,
            selected_plugins,
            link_plugins_in,
            runtime_dir,
            disabled_bevy_features,
            disabled_runtime_features,
            lean_profile,
            upx_compress,
            cancel,
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
    // Ship the plugin SDK so the game can compile plugins a player adds.
    enable_modding: bool,
    mesh_simplify: bool,
    mesh_simplify_ratio: f32,
    mesh_quantize: bool,
    mesh_generate_lods: bool,
    mesh_lod_levels: u32,
    template_path: std::path::PathBuf,
    selected_plugins: Vec<renzora_plugin::host::loader::PluginInfo>,
    link_plugins_in: bool,
    runtime_dir: std::path::PathBuf,
    disabled_bevy_features: Vec<String>,
    disabled_runtime_features: Vec<String>,
    lean_profile: crate::build::LeanProfile,
    upx_compress: bool,
    cancel: Arc<AtomicBool>,
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
    export_config.editor_open_tabs = Vec::new();

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

    // Ids the lean build compiled INTO the binary, so the copy step below leaves
    // them out. Filled by the lean arm; empty everywhere else, including when
    // linking was asked for but a plugin's source could not be found — those
    // fall back to shipping as files, which is why this is the build's answer
    // rather than the user's request.
    let mut linked_ids: Vec<String> = Vec::new();

    // Resolve the packer once, up front: a missing UPX is a skipped step with a
    // note, never a failed export. The user asked for a smaller game, not for the
    // export to stop.
    let upx_tool = if upx_compress {
        let found = crate::upx::locate();
        if found.is_none() {
            let _ = tx.send(ExportMsg::Progress(crate::upx::missing_hint()));
        }
        found
    } else {
        None
    };

    // Pack the executable BEFORE anything is appended to it, working on a copy in
    // the output dir so neither the dev runtime nor cargo's build output is ever
    // modified in place. Returns the original path when compression is off or
    // fails, so every caller can use the result unconditionally.
    let compress_exe = |src: &std::path::Path,
                        tx: &mpsc::Sender<ExportMsg>|
     -> std::path::PathBuf {
        let Some(upx) = upx_tool.as_ref() else {
            return src.to_path_buf();
        };
        let tmp = output_dir.join(format!("{binary_name}.upx-tmp"));
        let _ = tx.send(ExportMsg::Progress("Compressing binary with UPX…".into()));
        match crate::upx::compress_to_temp(upx, src, &tmp) {
            Ok((packed, before, after)) => {
                let _ = tx.send(ExportMsg::Progress(crate::upx::savings_line(
                    &binary_name,
                    before,
                    after,
                )));
                packed
            }
            Err(e) => {
                let _ = tx.send(ExportMsg::Progress(format!(
                    "UPX could not compress the binary ({e}) — shipping it uncompressed"
                )));
                src.to_path_buf()
            }
        }
    };

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
        // Ship the project's compiled Rust scripts beside the game. Only for the
        // copy-based modes and only for this machine's own platform: these are
        // host-shaped libraries, and the template they sit beside carries the
        // shared images they were compiled against. A lean export takes the other
        // route entirely and compiles them into the binary.
        if matches!(packaging_mode, PackagingMode::SeparateFiles | PackagingMode::SingleBinary)
            && Platform::current() == Some(platform)
        {
            let tx_s = tx.clone();
            let mut sp = |m: String| {
                let _ = tx_s.send(ExportMsg::Progress(m));
            };
            let lib_ext = match platform {
                Platform::WindowsX64 | Platform::WindowsArm64 => "dll",
                Platform::MacOSX64 | Platform::MacOSArm64 => "dylib",
                _ => "so",
            };
            // Best-effort: a game that ships without its scripts is still a
            // playable game, and failing the whole export over one is a worse
            // trade than saying so.
            if let Err(e) =
                crate::build::stage_prebuilt_scripts(&project.path, &output_dir, lib_ext, &mut sp)
            {
                let _ = tx.send(ExportMsg::Progress(format!("WARN: {e}")));
            }
            // Same trade for native plugins: a `Runtime`-scope one belongs in
            // the game, and the library the editor built is the thing that
            // ships. Read from the editor's own `plugins/`, not the project's —
            // a native plugin extends the engine, not one game.
            if let Some(editor_dir) = crate::build::editor_dir() {
                // The picker lists native plugins alongside C-ABI ones now, so
                // the same tick-list decides both. Ids are unique across the two
                // kinds — they all come from one `plugins/` namespace — so a set
                // of every selected id filters the native staging correctly
                // without having to know which kind each id was.
                let native_selection: std::collections::HashSet<String> =
                    selected_plugins.iter().map(|p| p.id.clone()).collect();
                if let Err(e) = crate::build::stage_runtime_native_plugins(
                    &editor_dir,
                    &output_dir,
                    lib_ext,
                    Some(&native_selection),
                    &mut sp,
                ) {
                    let _ = tx.send(ExportMsg::Progress(format!("WARN: {e}")));
                }
                // The SDK, when the game is meant to be moddable. That is what
                // turns "loads the plugins we shipped" into "compiles the ones a
                // player writes", and it is the only piece a player needs that
                // the game cannot do without.
                if enable_modding {
                    if let Err(e) =
                        crate::build::stage_modding_sdk(&editor_dir, &output_dir, &mut sp)
                    {
                        let _ = tx.send(ExportMsg::Progress(format!("WARN: {e}")));
                    }
                }
            }
        }

        match packaging_mode {
            PackagingMode::SeparateFiles => {
                let rpak_path = output_dir.join(format!("{}.rpak", binary_stem));
                let binary_dest = output_dir.join(&binary_name);
                let src = compress_exe(&template_path, &tx);

                packer
                    .write_to_file(&rpak_path, compression_level)
                    .and_then(|_| std::fs::copy(&src, &binary_dest).map(|_| ())).map(|_| ())
            }
            PackagingMode::SingleBinary => {
                let binary_dest = output_dir.join(&binary_name);
                let src = compress_exe(&template_path, &tx);
                packer
                    .append_to_binary(&src, &binary_dest, compression_level).map(|_| ())
            }
            PackagingMode::LeanSingleBinary => {
                // Recompile a lean static binary from the project workspace,
                // then embed the rpak in it. No dev runtime/dylibs are copied.
                let binary_dest = output_dir.join(&binary_name);
                let tx_b = tx.clone();
                let mut progress = |m: String| {
                    let _ = tx_b.send(ExportMsg::Progress(m));
                };
                // Everything here keys off where the EDITOR lives, not off
                // `runtime_dir` — that is the target platform's template dir, and
                // for a cross-platform export it is the download store with no
                // engine source above it. A lean build ignores the template
                // entirely; it recompiles the engine from source.
                let editor_dir = crate::build::editor_dir()
                    .unwrap_or_else(|| runtime_dir.clone());
                // A local Rust toolchain is only needed for a SAME-OS build,
                // which compiles natively. A different OS compiles in the
                // platform's container, which carries the pinned toolchain
                // itself — so provisioning one here would download and install a
                // rustup that the build never invokes, on a machine whose owner
                // installed Docker precisely so they would not need it.
                let cross = Platform::current() != Some(platform);
                let toolchain = if cross {
                    Ok(None)
                } else {
                    crate::toolchain::ensure_rust(&editor_dir, &mut progress).map(Some)
                };
                let built = toolchain
                    .and_then(|toolchain| {
                        // A lean build recompiles the ENGINE (the project is just
                        // assets → rpak), so compile the engine source checkout the
                        // editor was built from, found by walking up from the
                        // editor's own dir (e.g. `<engine>/dist/windows-x64/`).
                        let engine_dir = crate::build::resolve_engine_source()
                            .ok_or_else(|| {
                                "No engine source to compile. A lean build recompiles \
                                 the engine, so it needs either a source checkout the \
                                 editor runs from, or the engine source downloaded for \
                                 this version (Packaging → Download engine source)."
                                    .to_string()
                            })?;
                        // Linking a plugin in means COMPILING it, so it needs the
                        // source that produced the library the UI listed. A
                        // plugin with no source here (a marketplace download, say)
                        // is reported and shipped as a file instead — refusing the
                        // whole export over one would be a poor trade.
                        let statics = if link_plugins_in {
                            let wanted: Vec<(String, bool)> = selected_plugins
                                .iter()
                                .map(|p| {
                                    (
                                        p.id.clone(),
                                        p.scope == renzora_plugin::sys::PluginScope::Editor,
                                    )
                                })
                                .collect();
                            let (found, missing) =
                                crate::build::resolve_static_plugins(&engine_dir, &wanted);
                            if !missing.is_empty() {
                                progress(format!(
                                    "No source found for {} — shipping as file(s) beside the binary",
                                    missing.join(", ")
                                ));
                            }
                            // Keyed on the library stem, which is what the
                            // selection below compares against — `id` is the
                            // crate name, and on Unix the two differ by `lib`.
                            linked_ids = found.iter().map(|p| p.library_stem.clone()).collect();
                            found
                        } else {
                            Vec::new()
                        };
                        progress(format!(
                            "Compiling lean binary in {} (this can take several minutes)…",
                            engine_dir.display()
                        ));
                        crate::build::build_lean(
                            &engine_dir,
                            &project.path,
                            platform,
                            toolchain.as_ref(),
                            &mut progress,
                            &disabled_bevy_features,
                            &disabled_runtime_features,
                            lean_profile,
                            &statics,
                            &cancel,
                        )
                    });
                match built {
                    Ok(bin) => {
                        let src = compress_exe(&bin, &tx);
                        packer
                            .append_to_binary(&src, &binary_dest, compression_level)
                            .map(|_| ())
                    }
                    Err(e) => Err(std::io::Error::other(e)),
                }
            }
        }
    };

    // The compressed copy has been consumed by the copy/append above (or was
    // never made). Removing it here rather than in `compress_exe` keeps it alive
    // for exactly as long as it is read, and covers the failure paths too — a
    // leftover 50 MB temp beside a failed export would be baffling.
    if upx_tool.is_some() {
        let _ = std::fs::remove_file(output_dir.join(format!("{binary_name}.upx-tmp")));
    }

    // The lean binary is statically linked, so it ships none of the dev
    // runtime's sibling dylibs. It DOES still ship plugins — see below.
    let is_lean = matches!(packaging_mode, PackagingMode::LeanSingleBinary);

    match result {
        Ok(()) => {
            if !is_wasm && !is_lean {
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
                            // ── The shared images, plus the Rust std the binary
                            // imports. Not plugins (they live in `plugins/`) and
                            // not the executables.
                            //
                            // Matched on `_dylib` rather than by name, and that
                            // is a repair as much as a simplification. The list
                            // used to read `renzora.` / `librenzora.`, from when
                            // the contract dylib was `renzora.dll`. It is
                            // `renzora_dylib.dll` now, and `renzora_ember_dylib`
                            // joined it — so the test matched NEITHER, and every
                            // copy-based export since the rename shipped a game
                            // that could not start:
                            //
                            //   The code execution cannot proceed because
                            //   renzora_ember_dylib.dll was not found.
                            //
                            // The pattern covers all three today and any shared
                            // image added later, so the next rename cannot
                            // silently drop one the same way. `bevy_dylib` also
                            // carries a `-<hash>` suffix, which is why this is a
                            // substring test and not an equality one.
                            //
                            // `openxr_loader` rides along for the same reason
                            // `package-release.sh` copies it into every runtime
                            // template by name: the binary imports it, so a game
                            // without it fails to start exactly as it does
                            // without a `_dylib` — the next error in the same
                            // dialog, found the same way.
                            let shared = name_str.contains("_dylib")
                                || name_str.starts_with("std-")
                                || name_str.starts_with("libstd-")
                                || name_str.starts_with("openxr_loader");
                            if shared {
                                let _ = std::fs::copy(entry.path(), output_dir.join(&name));
                            }
                        }
                    }
                }
            }

            // Plugins ship with EVERY packaging mode, lean included.
            //
            // They used to be skipped for a lean export on the reasoning that a
            // static binary cannot dlopen. That is not true, and it is the wrong
            // mechanism besides: these are C-ABI plugins (`renzora_plugin`),
            // which link no Bevy at all — the interface is passed in as a
            // function table — so there is nothing for them to share with the
            // host and nothing about static linking that stops the OS loading
            // them. The result was a lean game shipping zero plugins, silently:
            // no Lua, no post-process effects, and no error, because the host
            // simply found an empty `plugins/` directory.
            //
            // Anything the lean build compiled in is skipped here. Copying it as
            // well would not merely waste space: the host would initialise the
            // plugin twice, and every first-claim registration it makes — a
            // script backend's file extensions, a panel id — would log a
            // duplicate-registration error on the losing copy.
            let to_copy: Vec<&std::path::Path> = selected_plugins
                .iter()
                .filter(|p| !linked_ids.contains(&p.id))
                .map(|p| p.path.as_path())
                .collect();
            if !is_wasm && !to_copy.is_empty() {
                let _ = tx.send(ExportMsg::Progress("Copying plugins...".into()));
                let plugins_out = output_dir.join("plugins");
                let _ = std::fs::create_dir_all(&plugins_out);

                for plugin_path in &to_copy {
                    if let Some(filename) = plugin_path.file_name() {
                        let dest = plugins_out.join(filename);
                        if let Err(e) = std::fs::copy(plugin_path, &dest) {
                            warn!("[export] Failed to copy plugin {:?}: {}", filename, e);
                        }
                    }
                }
                info!("[export] Copied {} plugins to output", to_copy.len());
            }
            if !linked_ids.is_empty() {
                info!(
                    "[export] Linked {} plugins into the binary",
                    linked_ids.len()
                );
            }

            // Pack the shipped libraries too. For a copy-based export this is
            // where nearly all the size is — `bevy_dylib` alone dwarfs the game
            // binary — so compressing only the .exe would look like the toggle
            // did almost nothing. These are packed in place, which is safe in a
            // way the executable is not: nothing is appended to a library, so
            // there is no payload for UPX to lose.
            //
            // A library that will not pack is logged and shipped as-is: UPX
            // declines some inputs (an already-packed file, an unusual section
            // layout), and one such file is no reason to fail an export that has
            // otherwise succeeded.
            if let Some(upx) = upx_tool.as_ref() {
                let mut libs: Vec<std::path::PathBuf> = Vec::new();
                for entry in std::fs::read_dir(&output_dir)
                    .into_iter()
                    .flatten()
                    .flatten()
                {
                    let path = entry.path();
                    let is_lib = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| matches!(e, "dll" | "so" | "dylib"));
                    if is_lib {
                        libs.push(path);
                    }
                }
                for entry in std::fs::read_dir(output_dir.join("plugins"))
                    .into_iter()
                    .flatten()
                    .flatten()
                {
                    libs.push(entry.path());
                }
                if !libs.is_empty() {
                    let _ = tx.send(ExportMsg::Progress(format!(
                        "Compressing {} shipped librar{} with UPX…",
                        libs.len(),
                        if libs.len() == 1 { "y" } else { "ies" }
                    )));
                }
                for lib in libs {
                    let label = lib
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    match crate::upx::compress_in_place(upx, &lib) {
                        Ok((before, after)) => {
                            let _ = tx.send(ExportMsg::Progress(crate::upx::savings_line(
                                &label, before, after,
                            )));
                        }
                        Err(e) => {
                            let _ = tx.send(ExportMsg::Progress(format!(
                                "Skipped {label}: {e}"
                            )));
                        }
                    }
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
