//! Internal `clack-host` glue. Only compiled when the `clap-host` feature
//! is on. Public surface is intentionally tiny — `host.rs` is the only
//! caller.
//!
//! Architecture mirrors the cpal example in the clack repo
//! (`host/examples/cpal/src/host.rs` and `.../host/gui.rs`), adapted for
//! a multi-instance host (we hold many plugins at once, one per FX slot)
//! and Bevy's main-loop model (we pump messages from an Update system
//! instead of blocking on `for message in receiver`).

#![cfg(feature = "clap-host")]

use std::ffi::CString;
use std::path::Path;

use bevy::prelude::*;
use clack_extensions::gui::{
    GuiApiType, GuiConfiguration, GuiError, GuiSize, HostGuiImpl, PluginGui,
};
use clack_extensions::log::{HostLog, HostLogImpl, LogSeverity};
use clack_extensions::params::{
    HostParams, HostParamsImplMainThread, HostParamsImplShared, ParamClearFlags, ParamRescanFlags,
};
use clack_extensions::gui::HostGui;
use clack_host::prelude::*;
use crossbeam_channel::{Receiver, Sender, unbounded};

/// Messages that any plugin thread can send back to the main thread.
#[derive(Debug)]
pub enum MainThreadMessage {
    /// The plugin asked for `on_main_thread()` to be called soon.
    RunOnMainThread,
    /// The user closed the floating editor window from its title bar.
    GuiClosed,
}

// ─── Host trait impls ───────────────────────────────────────────────────────

/// Phantom type carrying the host trait impls. Never instantiated.
pub struct RenzoraHost;

impl HostHandlers for RenzoraHost {
    type Shared<'a> = RenzoraHostShared;
    type MainThread<'a> = RenzoraHostMainThread<'a>;
    /// We don't run the plugin's audio processor yet (Kira `Effect` impl
    /// is a separate task). Setting this to `()` means `PluginInstance`
    /// is created without an active audio processor.
    type AudioProcessor<'a> = ();

    fn declare_extensions(builder: &mut HostExtensions<Self>, _shared: &Self::Shared<'_>) {
        builder
            .register::<HostLog>()
            .register::<HostGui>()
            .register::<HostParams>();
    }
}

/// Cross-thread state for one plugin instance. `Sender` is the side of the
/// main-thread channel; we never receive from this thread — the host owner
/// keeps the receiver and pumps it each frame.
pub struct RenzoraHostShared {
    sender: Sender<MainThreadMessage>,
    bus: String,
    local_id: u64,
}

impl RenzoraHostShared {
    fn new(sender: Sender<MainThreadMessage>, bus: String, local_id: u64) -> Self {
        Self { sender, bus, local_id }
    }
}

impl<'a> SharedHandler<'a> for RenzoraHostShared {
    fn initializing(&self, _instance: InitializingPluginHandle<'a>) {}

    fn request_restart(&self) {}
    fn request_process(&self) {}

    fn request_callback(&self) {
        // Drop the message if the receiver hung up — host has gone away.
        let _ = self.sender.send(MainThreadMessage::RunOnMainThread);
    }
}

impl HostLogImpl for RenzoraHostShared {
    fn log(&self, severity: LogSeverity, message: &str) {
        // Plugins are noisy at Debug — gate to Info+ so we don't drown the
        // editor console on each frame.
        if severity <= LogSeverity::Debug {
            return;
        }
        info!("[plugin {}/{}] {}: {}", self.bus, self.local_id, severity, message);
    }
}

impl HostGuiImpl for RenzoraHostShared {
    fn resize_hints_changed(&self) {}

    fn request_resize(&self, _new_size: GuiSize) -> Result<(), HostError> {
        // Floating mode — the plugin owns its window, so we don't honour
        // resize requests. Returning Ok() so the plugin doesn't error.
        Ok(())
    }

    fn request_show(&self) -> Result<(), HostError> {
        Ok(())
    }

    fn request_hide(&self) -> Result<(), HostError> {
        Ok(())
    }

    fn closed(&self, _was_destroyed: bool) {
        // User clicked the X on the plugin window. Tell the host to flip
        // the editor_open flag back so the FX popup updates.
        let _ = self.sender.send(MainThreadMessage::GuiClosed);
    }
}

impl HostParamsImplShared for RenzoraHostShared {
    fn request_flush(&self) {
        // No audio thread yet; nothing to flush.
    }
}

/// Main-thread state for one plugin instance.
pub struct RenzoraHostMainThread<'a> {
    _shared: &'a RenzoraHostShared,
    /// The plugin's GUI extension handle, if it advertised one. Harvested
    /// after init via `MainThreadHandler::initialized`.
    pub gui: Option<PluginGui>,
}

impl<'a> RenzoraHostMainThread<'a> {
    fn new(shared: &'a RenzoraHostShared) -> Self {
        Self { _shared: shared, gui: None }
    }
}

impl<'a> MainThreadHandler<'a> for RenzoraHostMainThread<'a> {
    fn initialized(&mut self, instance: InitializedPluginHandle<'a>) {
        self.gui = instance.get_extension();
    }
}

impl HostParamsImplMainThread for RenzoraHostMainThread<'_> {
    fn rescan(&mut self, _flags: ParamRescanFlags) {}
    fn clear(&mut self, _param_id: ClapId, _flags: ParamClearFlags) {}
}

// ─── Loading + GUI helpers ──────────────────────────────────────────────────

/// Information about *us*, the host. Reported to plugins on init.
fn host_info() -> HostInfo {
    HostInfo::new(
        "Renzora",
        "Renzora",
        "https://github.com/your-org/renzora",
        env!("CARGO_PKG_VERSION"),
    )
    .expect("static HostInfo strings should be valid")
}

/// Outcome of `load_and_instantiate`. Held as an opaque box by
/// `PluginInstances` so the public API doesn't leak `clack_host` types.
pub struct LoadedPlugin {
    pub instance: PluginInstance<RenzoraHost>,
    pub receiver: Receiver<MainThreadMessage>,
}

/// dlopen the bundle, instantiate the first plugin in its factory, and
/// return both the instance and the receiver half of its main-thread
/// channel. All errors are stringified — the caller logs them.
pub fn load_and_instantiate(
    bundle_path: &Path,
    bus: &str,
    local_id: u64,
) -> Result<LoadedPlugin, String> {
    // On Windows, plugin bundles often ship sidecar DLLs alongside the
    // `.clap` itself. Add the bundle's directory to the loader search path
    // so those DLLs resolve.
    #[cfg(windows)]
    add_dll_directory(bundle_path);

    // SAFETY: load_from is unsafe because it dlopens arbitrary native code.
    // We catch_unwind around it to survive a malformed plugin's
    // `clap_entry::init` panicking.
    let entry = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        PluginEntry::load(bundle_path)
    }))
    .map_err(|_| "plugin entry init panicked".to_string())?
    .map_err(|e| format!("PluginEntry::load failed: {e}"))?;

    let factory = entry
        .get_plugin_factory()
        .ok_or_else(|| "bundle has no plugin factory".to_string())?;

    // Pick the first plugin in the bundle. Multi-plugin bundles will need
    // a UI flow to pick which one — out of scope for this MVP.
    let descriptor = factory
        .plugin_descriptors()
        .next()
        .ok_or_else(|| "plugin factory is empty".to_string())?;

    let id_cstr = descriptor
        .id()
        .ok_or_else(|| "plugin descriptor missing id".to_string())?;
    let plugin_id = CString::new(id_cstr.to_bytes()).map_err(|e| e.to_string())?;

    let (sender, receiver) = unbounded();
    let host_info = host_info();
    let bus_owned = bus.to_string();

    let instance = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        PluginInstance::<RenzoraHost>::new(
            move |_| RenzoraHostShared::new(sender.clone(), bus_owned.clone(), local_id),
            move |shared| RenzoraHostMainThread::new(shared),
            &entry,
            &plugin_id,
            &host_info,
        )
    }))
    .map_err(|_| "plugin instantiation panicked".to_string())?
    .map_err(|e| format!("PluginInstance::new failed: {e}"))?;

    Ok(LoadedPlugin { instance, receiver })
}

/// Wraps a `PluginGui` handle plus the negotiated configuration. Mirrors
/// the `Gui` struct from the cpal example, slimmed to floating-only.
pub struct Gui {
    plugin_gui: PluginGui,
    configuration: GuiConfiguration<'static>,
    pub is_open: bool,
}

impl Gui {
    /// Negotiate a floating configuration with the plugin. Returns `None`
    /// if the plugin doesn't support floating GUIs on this platform.
    pub fn new(plugin_gui: PluginGui, plugin: &mut PluginMainThreadHandle) -> Option<Self> {
        let api_type = GuiApiType::default_for_current_platform()?;
        let config = GuiConfiguration { api_type, is_floating: true };
        if !plugin_gui.is_api_supported(plugin, config) {
            return None;
        }
        Some(Self {
            plugin_gui,
            configuration: config,
            is_open: false,
        })
    }

    pub fn open_floating(
        &mut self,
        plugin: &mut PluginMainThreadHandle,
    ) -> Result<(), GuiError> {
        self.plugin_gui.create(plugin, self.configuration)?;
        self.plugin_gui.suggest_title(plugin, c"Renzora plugin");
        self.plugin_gui.show(plugin)?;
        self.is_open = true;
        Ok(())
    }

    pub fn destroy(&mut self, plugin: &mut PluginMainThreadHandle) {
        if self.is_open {
            let _ = self.plugin_gui.hide(plugin);
            self.plugin_gui.destroy(plugin);
            self.is_open = false;
        }
    }
}

#[cfg(windows)]
fn add_dll_directory(bundle_path: &Path) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::LibraryLoader::AddDllDirectory;

    // CLAP "bundles" on Windows are usually a single .clap file; the
    // sidecar DLL search path should be the directory containing it.
    let Some(dir) = bundle_path.parent() else { return };
    let mut wide: Vec<u16> = dir.as_os_str().encode_wide().collect();
    wide.push(0);
    // SAFETY: wide is a valid null-terminated UTF-16 string for the
    // duration of the call. AddDllDirectory copies it internally.
    let cookie = unsafe { AddDllDirectory(wide.as_ptr()) };
    if cookie.is_null() {
        warn!(
            "[vst] AddDllDirectory failed for {} — sidecar DLLs may not load",
            dir.display()
        );
    }
}
