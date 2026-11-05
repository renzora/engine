//! Running an export in the companion process.
//!
//! An export packs every asset, compresses the archive, and for a lean build
//! compiles the whole engine, for minutes. It already ran on a thread of its
//! own, but in the editor's process: the packing and compression took cores
//! and memory from the frame being edited, and cargo ran as the editor's
//! child at the editor's priority. The companion runs at lower priority, so
//! the OS lets the editor's frame go first.
//!
//! Nothing about the export window changes. Its thread still starts and still
//! reports through the same channel; it hands the export to the companion
//! with [`renzora::CompanionRemote::run_cancellable`] and turns each progress
//! note back into the line the window shows. Cancel cancels the job in the
//! companion, which stops cargo as before. With no companion connected, the
//! thread runs the export itself, as it always did.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

use renzora::core::{CurrentProject, ProjectConfig, WindowMode};
use serde::{Deserialize, Serialize};

use crate::build::LeanProfile;
use crate::overlay::{export_worker, ExportMsg, PackagingMode};
use crate::templates::Platform;

/// The companion job kind.
pub const KIND: &str = "export";

/// One native plugin chosen to ship, in a form that survives the trip.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PluginPick {
    pub id: String,
    /// `true` for a Runtime-scope plugin, `false` for Editor scope.
    pub runtime: bool,
    pub lib: PathBuf,
}

impl From<&renzora_native_plugin::InstalledNativePlugin> for PluginPick {
    fn from(p: &renzora_native_plugin::InstalledNativePlugin) -> Self {
        Self { id: p.id.clone(), runtime: p.scope == renzora::NativePluginScope::Runtime, lib: p.lib.clone() }
    }
}

impl From<PluginPick> for renzora_native_plugin::InstalledNativePlugin {
    fn from(p: PluginPick) -> Self {
        Self {
            id: p.id,
            scope: if p.runtime { renzora::NativePluginScope::Runtime } else { renzora::NativePluginScope::Editor },
            lib: p.lib,
        }
    }
}

/// Everything an export needs, settled in the editor before it starts.
///
/// The project's settings travel whole rather than being read back from
/// `project.toml` in the companion: settings changed in the editor and not
/// yet saved are what the user is exporting.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExportJob {
    pub project_path: PathBuf,
    pub project_config: ProjectConfig,
    pub project_name: String,
    pub platform: Platform,
    pub packaging_mode: PackagingMode,
    pub compression_level: i32,
    pub output_dir: PathBuf,
    pub window_mode: WindowMode,
    pub window_width: u32,
    pub window_height: u32,
    pub console_logging: bool,
    pub allow_profiling: bool,
    pub icon_path: Option<String>,
    pub binary_name_override: Option<String>,
    pub include_server: bool,
    pub bundle_app: bool,
    pub enable_modding: bool,
    pub mesh_simplify: bool,
    pub mesh_simplify_ratio: f32,
    pub mesh_quantize: bool,
    pub mesh_generate_lods: bool,
    pub mesh_lod_levels: u32,
    pub template_path: PathBuf,
    pub selected_plugins: Vec<PluginPick>,
    pub link_plugins_in: bool,
    pub runtime_dir: PathBuf,
    pub disabled_bevy_features: Vec<String>,
    pub disabled_runtime_features: Vec<String>,
    pub lean_profile: LeanProfile,
    pub upx_compress: bool,
    pub included_files: Option<Vec<String>>,
    pub dlss: bool,
}

impl ExportJob {
    /// Run the export on this thread, reporting into `tx`.
    pub(crate) fn run_here(self, tx: mpsc::Sender<ExportMsg>, cancel: Arc<AtomicBool>) {
        let project = CurrentProject { path: self.project_path, config: self.project_config };
        export_worker(
            tx,
            project,
            self.project_name,
            self.platform,
            self.packaging_mode,
            self.compression_level,
            self.output_dir,
            self.window_mode,
            self.window_width,
            self.window_height,
            self.console_logging,
            self.allow_profiling,
            self.icon_path,
            self.binary_name_override,
            self.include_server,
            self.bundle_app,
            self.enable_modding,
            self.mesh_simplify,
            self.mesh_simplify_ratio,
            self.mesh_quantize,
            self.mesh_generate_lods,
            self.mesh_lod_levels,
            self.template_path,
            self.selected_plugins.into_iter().map(Into::into).collect(),
            self.link_plugins_in,
            self.runtime_dir,
            self.disabled_bevy_features,
            self.disabled_runtime_features,
            self.lean_profile,
            self.upx_compress,
            self.included_files,
            self.dlss,
            cancel,
        );
    }
}

/// Start `job` on a thread of its own: in the companion when `remote` has one
/// connected, here otherwise. Either way its lines arrive on `tx`.
pub(crate) fn start(
    job: ExportJob,
    tx: mpsc::Sender<ExportMsg>,
    cancel: Arc<AtomicBool>,
    remote: Option<renzora::CompanionRemote>,
) {
    std::thread::spawn(move || {
        if let Some(remote) = remote {
            let lines = tx.clone();
            let mut forward = |_: Option<f32>, note: &str| {
                let _ = lines.send(ExportMsg::Progress(note.to_string()));
            };
            let target = job.project_name.clone();
            match remote.run_cancellable(KIND, target, &job, &mut forward, &cancel) {
                Some(Ok(done)) => {
                    let _ = tx.send(ExportMsg::Done(done));
                    return;
                }
                Some(Err(why)) => {
                    let why = if cancel.load(Ordering::Relaxed) { "Export cancelled".to_string() } else { why };
                    let _ = tx.send(ExportMsg::Error(why));
                    return;
                }
                // No companion after all: run it here.
                None => {}
            }
        }
        job.run_here(tx, cancel);
    });
}

/// The companion's side: run the export described by `payload`, passing each
/// line to `progress` and stopping when `cancelled` says so. Answers with the
/// export's closing message, or why it failed.
pub fn run_in_companion(
    payload: &str,
    progress: &mut dyn FnMut(&str),
    cancelled: &dyn Fn() -> bool,
) -> Result<String, String> {
    let job: ExportJob =
        serde_json::from_str(payload).map_err(|e| format!("could not read the export's settings: {e}"))?;
    let (tx, rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = cancel.clone();
    let worker = std::thread::spawn(move || job.run_here(tx, worker_cancel));
    loop {
        match rx.recv_timeout(std::time::Duration::from_millis(250)) {
            Ok(ExportMsg::Progress(line)) => progress(&line),
            Ok(ExportMsg::Done(done)) => {
                let _ = worker.join();
                return Ok(done);
            }
            Ok(ExportMsg::Error(why)) => {
                let _ = worker.join();
                return Err(why);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = worker.join();
                return Err("the export stopped without saying why".into());
            }
        }
        if cancelled() {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_export_job_survives_the_trip() {
        let job = ExportJob {
            project_path: "C:/games/demo".into(),
            project_config: ProjectConfig::default(),
            project_name: "Demo".into(),
            platform: Platform::WindowsX64,
            packaging_mode: PackagingMode::LeanSingleBinary,
            compression_level: 9,
            output_dir: "C:/out".into(),
            window_mode: WindowMode::Borderless,
            window_width: 1920,
            window_height: 1080,
            console_logging: true,
            allow_profiling: false,
            icon_path: Some("icon.png".into()),
            binary_name_override: None,
            include_server: true,
            bundle_app: false,
            enable_modding: false,
            mesh_simplify: true,
            mesh_simplify_ratio: 0.5,
            mesh_quantize: true,
            mesh_generate_lods: true,
            mesh_lod_levels: 3,
            template_path: "C:/t".into(),
            selected_plugins: vec![PluginPick { id: "spline".into(), runtime: true, lib: "spline.dll".into() }],
            link_plugins_in: true,
            runtime_dir: "C:/r".into(),
            disabled_bevy_features: vec!["bevy_audio".into()],
            disabled_runtime_features: vec![],
            lean_profile: LeanProfile { panic_abort: false, opt_level_z: true, codegen_units_one: false },
            upx_compress: false,
            included_files: Some(vec!["scenes/main.bsn".into()]),
            dlss: false,
        };
        let back: ExportJob = serde_json::from_str(&serde_json::to_string(&job).unwrap()).unwrap();
        assert_eq!(back.platform, job.platform);
        assert_eq!(back.packaging_mode, job.packaging_mode);
        assert_eq!(back.window_mode, job.window_mode);
        assert_eq!(back.lean_profile, job.lean_profile);
        assert_eq!(back.included_files, job.included_files);
        assert_eq!(back.selected_plugins[0].id, "spline");
        assert_eq!(back.project_config.main_scene, job.project_config.main_scene);
        let plugin: renzora_native_plugin::InstalledNativePlugin = back.selected_plugins[0].clone().into();
        assert_eq!(plugin.scope, renzora::NativePluginScope::Runtime);
    }
}
