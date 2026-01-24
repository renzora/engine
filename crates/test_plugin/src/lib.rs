//! Test Plugin for the Bevy Editor
//!
//! A simple test plugin that demonstrates the FFI plugin API.

use editor_plugin_api::prelude::*;
use editor_plugin_api::ffi::FfiEditorApi;
use editor_plugin_api::egui_phosphor::regular::FLASK;

/// A test plugin that demonstrates the plugin API
pub struct TestPlugin {
    /// Elapsed time since plugin load
    elapsed: f32,
    /// Update counter
    updates: u64,
}

impl TestPlugin {
    pub fn new() -> Self {
        Self {
            elapsed: 0.0,
            updates: 0,
        }
    }

    pub fn manifest(&self) -> PluginManifest {
        PluginManifest::new("com.bevy-editor.test-plugin", "Test Plugin", "0.1.0")
            .author("Bevy Editor Team")
            .description("A test plugin demonstrating the editor plugin API")
    }

    pub fn on_load_ffi(&mut self, api: &FfiEditorApi) -> Result<(), PluginError> {
        api.log_info("Test Plugin loaded!");
        Ok(())
    }

    pub fn on_unload_ffi(&mut self, api: &FfiEditorApi) {
        api.remove_status_item("test_uptime");
        api.log_info("Test Plugin unloaded!");
    }

    pub fn on_update_ffi(&mut self, api: &FfiEditorApi, dt: f32) {
        self.elapsed += dt;
        self.updates += 1;

        // Update status bar every ~30 frames to reduce overhead
        if self.updates % 30 == 0 {
            let mins = (self.elapsed / 60.0) as u32;
            let secs = (self.elapsed % 60.0) as u32;
            let text = format!("{}:{:02}", mins, secs);
            let tooltip = format!("Test Plugin Uptime\nRunning for {}m {}s\nUpdates: {}", mins, secs, self.updates);
            api.set_status_item("test_uptime", &text, Some(FLASK), Some(&tooltip), true, 50);
        }
    }
}

impl Default for TestPlugin {
    fn default() -> Self {
        Self::new()
    }
}

declare_plugin!(TestPlugin, TestPlugin::new());
