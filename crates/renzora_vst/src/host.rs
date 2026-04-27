//! Live plugin host — owns the `clack-host` instances and their floating
//! editor windows.
//!
//! All entry points here run on the **main thread**. Per the CLAP spec,
//! plugin instantiation and the GUI extension must not be touched from
//! worker threads, so `PluginInstances` is a Bevy `NonSend` resource.
//!
//! Threading: each instance gets its own crossbeam channel. The plugin's
//! audio threads (and any worker threads) can call `request_callback()`
//! and `closed()` on `RenzoraHostShared` (which is `Send + Sync`); those
//! callbacks push messages onto the channel. The host's `pump` system
//! drains every channel each frame and reacts (running on-main-thread
//! callbacks, flipping editor_open, etc.).
//!
//! Crash safety: bundle loading and instantiation happen inside
//! `catch_unwind` in `host_impl::load_and_instantiate`. A malformed
//! plugin's `clap_entry::init` won't take down the editor.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bevy::prelude::*;

use crate::insert::BusInserts;

#[derive(Debug)]
pub enum HostOpOutcome {
    Ok,
    HostDisabled,
    Failed(String),
}

// ─── Per-slot live state ────────────────────────────────────────────────────
//
// Two shapes — with and without the clap-host feature. Same field names so
// public methods can read them uniformly.

#[cfg(not(feature = "clap-host"))]
struct LiveSlot {
    bundle_path: PathBuf,
    editor_open: bool,
    instantiated: bool,
}

#[cfg(feature = "clap-host")]
struct LiveSlot {
    bundle_path: PathBuf,
    editor_open: bool,
    instantiated: bool,
    /// `Some` once `load_and_instantiate` succeeded. Held by value here;
    /// `clack-host` manages the self-referential lifetimes internally.
    loaded: Option<crate::host_impl::LoadedPlugin>,
    gui: Option<crate::host_impl::Gui>,
}

/// Map of `(bus, local_id) → LiveSlot`. NonSend because `clack-host`
/// instances are not `Send` and must live on the main thread.
#[derive(Default)]
pub struct PluginInstances {
    slots: HashMap<(String, u64), LiveSlot>,
}

impl PluginInstances {
    /// Allocate a live plugin instance for a freshly-added insert slot.
    pub fn add(&mut self, bus: &str, local_id: u64, bundle_path: &Path) -> HostOpOutcome {
        let key = (bus.to_string(), local_id);

        #[cfg(not(feature = "clap-host"))]
        {
            self.slots.insert(
                key,
                LiveSlot {
                    bundle_path: bundle_path.to_path_buf(),
                    editor_open: false,
                    instantiated: false,
                },
            );
            warn!(
                "[vst] Add: {} on bus '{}' — clap-host feature off; no real instance created",
                bundle_path.display(),
                bus
            );
            HostOpOutcome::HostDisabled
        }

        #[cfg(feature = "clap-host")]
        {
            match crate::host_impl::load_and_instantiate(bundle_path, bus, local_id) {
                Ok(loaded) => {
                    self.slots.insert(
                        key,
                        LiveSlot {
                            bundle_path: bundle_path.to_path_buf(),
                            editor_open: false,
                            instantiated: true,
                            loaded: Some(loaded),
                            gui: None,
                        },
                    );
                    info!("[vst] Loaded {}", bundle_path.display());
                    HostOpOutcome::Ok
                }
                Err(e) => {
                    warn!("[vst] Load failed for {}: {}", bundle_path.display(), e);
                    self.slots.insert(
                        key,
                        LiveSlot {
                            bundle_path: bundle_path.to_path_buf(),
                            editor_open: false,
                            instantiated: false,
                            loaded: None,
                            gui: None,
                        },
                    );
                    HostOpOutcome::Failed(e)
                }
            }
        }
    }

    /// Free the live instance for a removed slot. Closes the editor first.
    pub fn remove(&mut self, bus: &str, local_id: u64) {
        let Some(mut slot) = self.slots.remove(&(bus.to_string(), local_id)) else {
            return;
        };

        #[cfg(feature = "clap-host")]
        {
            if let (Some(loaded), Some(gui)) = (slot.loaded.as_mut(), slot.gui.as_mut()) {
                gui.destroy(&mut loaded.instance.plugin_handle());
            }
            // Drop `loaded` (and the PluginInstance inside it) on this thread.
        }

        // Touch fields to silence unused warnings on the no-feature build.
        let _ = (slot.editor_open, slot.bundle_path);
    }

    /// Open the plugin's floating editor window. CLAP plugin GUIs cannot
    /// be drawn into an egui texture — they get their own OS window.
    pub fn open_editor(&mut self, bus: &str, local_id: u64) -> HostOpOutcome {
        let Some(slot) = self.slots.get_mut(&(bus.to_string(), local_id)) else {
            return HostOpOutcome::Failed(format!("no live slot for {}/{}", bus, local_id));
        };
        if slot.editor_open {
            return HostOpOutcome::Ok;
        }

        #[cfg(not(feature = "clap-host"))]
        {
            warn!(
                "[vst] OpenEditor: {} on bus '{}' — clap-host feature off, no window will appear",
                slot.bundle_path.display(),
                bus
            );
            slot.editor_open = true;
            HostOpOutcome::HostDisabled
        }

        #[cfg(feature = "clap-host")]
        {
            let Some(loaded) = slot.loaded.as_mut() else {
                return HostOpOutcome::Failed("plugin never loaded".into());
            };

            // Take the gui extension handle that MainThreadHandler
            // captured at init time. Construct a `Gui` wrapper if we
            // haven't already.
            if slot.gui.is_none() {
                let plugin_gui = loaded
                    .instance
                    .access_handler(|h| h.gui);
                let Some(plugin_gui) = plugin_gui else {
                    return HostOpOutcome::Failed("plugin has no gui extension".into());
                };
                let mut handle = loaded.instance.plugin_handle();
                let gui = match crate::host_impl::Gui::new(plugin_gui, &mut handle) {
                    Some(gui) => gui,
                    None => {
                        return HostOpOutcome::Failed(
                            "plugin doesn't support floating GUI on this platform".into(),
                        );
                    }
                };
                slot.gui = Some(gui);
            }

            let gui = slot.gui.as_mut().unwrap();
            let mut handle = loaded.instance.plugin_handle();
            match gui.open_floating(&mut handle) {
                Ok(()) => {
                    slot.editor_open = true;
                    HostOpOutcome::Ok
                }
                Err(e) => HostOpOutcome::Failed(format!("gui open failed: {e}")),
            }
        }
    }

    pub fn close_editor(&mut self, bus: &str, local_id: u64) -> HostOpOutcome {
        let Some(slot) = self.slots.get_mut(&(bus.to_string(), local_id)) else {
            return HostOpOutcome::Failed(format!("no live slot for {}/{}", bus, local_id));
        };
        if !slot.editor_open {
            return HostOpOutcome::Ok;
        }

        #[cfg(not(feature = "clap-host"))]
        {
            slot.editor_open = false;
            HostOpOutcome::HostDisabled
        }

        #[cfg(feature = "clap-host")]
        {
            if let (Some(loaded), Some(gui)) = (slot.loaded.as_mut(), slot.gui.as_mut()) {
                gui.destroy(&mut loaded.instance.plugin_handle());
            }
            slot.editor_open = false;
            HostOpOutcome::Ok
        }
    }

    pub fn is_loaded(&self, bus: &str, local_id: u64) -> bool {
        self.slots
            .get(&(bus.to_string(), local_id))
            .map(|s| s.instantiated)
            .unwrap_or(false)
    }

    pub fn is_editor_open(&self, bus: &str, local_id: u64) -> bool {
        self.slots
            .get(&(bus.to_string(), local_id))
            .map(|s| s.editor_open)
            .unwrap_or(false)
    }

    /// Drain main-thread messages from every active plugin's channel.
    /// Call once per frame from a `NonSend` system. Returns the keys of
    /// any slots whose user-closed-the-window event was observed so the
    /// caller can sync `BusInserts.editor_open` back to false.
    pub fn pump(&mut self) -> Vec<(String, u64)> {
        let mut closed: Vec<(String, u64)> = Vec::new();

        #[cfg(feature = "clap-host")]
        {
            for ((bus, local_id), slot) in self.slots.iter_mut() {
                let Some(loaded) = slot.loaded.as_mut() else { continue };
                while let Ok(msg) = loaded.receiver.try_recv() {
                    match msg {
                        crate::host_impl::MainThreadMessage::RunOnMainThread => {
                            loaded.instance.call_on_main_thread_callback();
                        }
                        crate::host_impl::MainThreadMessage::GuiClosed => {
                            slot.editor_open = false;
                            closed.push((bus.clone(), *local_id));
                        }
                    }
                }
            }
        }

        closed
    }
}

/// Bevy system: pump the channel each frame and reflect any user-closed
/// editor windows back into `BusInserts` so the FX popup updates the
/// Open/Close button state. Runs as a `NonSend` system because
/// `PluginInstances` is `NonSend`.
pub fn pump_plugin_messages(
    mut instances: NonSendMut<PluginInstances>,
    mut inserts: ResMut<BusInserts>,
) {
    let closed = instances.pump();
    for (bus, local_id) in closed {
        if let Some(slot) = inserts
            .entry(&bus)
            .slots
            .iter_mut()
            .find(|s| s.local_id == local_id)
        {
            slot.editor_open = false;
        }
    }
}
