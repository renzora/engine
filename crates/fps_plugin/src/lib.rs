//! FPS Counter Plugin for the Bevy Editor
//!
//! Displays real-time FPS (frames per second) statistics in the status bar.

use editor_plugin_api::prelude::*;
use editor_plugin_api::ffi::FfiEditorApi;
use egui_phosphor::regular::{ACTIVITY, CLOCK};

/// Number of samples to average for smooth FPS display
const FPS_SAMPLE_COUNT: usize = 60;

/// FPS counter plugin
pub struct FpsPlugin {
    /// Ring buffer of frame times for averaging
    frame_times: Vec<f32>,
    /// Current index in the ring buffer
    frame_index: usize,
    /// Cached FPS value (updated every few frames)
    cached_fps: f32,
    /// Cached frame time in ms
    cached_frame_time: f32,
    /// Min FPS seen
    min_fps: f32,
    /// Max FPS seen
    max_fps: f32,
    /// Frame counter for periodic updates
    update_counter: u32,
}

impl FpsPlugin {
    pub fn new() -> Self {
        Self {
            frame_times: vec![0.016; FPS_SAMPLE_COUNT],
            frame_index: 0,
            cached_fps: 60.0,
            cached_frame_time: 16.67,
            min_fps: f32::MAX,
            max_fps: 0.0,
            update_counter: 0,
        }
    }

    fn update_stats(&mut self, dt: f32) {
        self.frame_times[self.frame_index] = dt;
        self.frame_index = (self.frame_index + 1) % FPS_SAMPLE_COUNT;
        self.update_counter += 1;

        if self.update_counter >= 10 {
            self.update_counter = 0;
            let avg_frame_time: f32 = self.frame_times.iter().sum::<f32>() / FPS_SAMPLE_COUNT as f32;
            self.cached_frame_time = avg_frame_time * 1000.0;
            self.cached_fps = if avg_frame_time > 0.0 { 1.0 / avg_frame_time } else { 0.0 };

            if self.cached_fps > 0.0 && self.cached_fps < 10000.0 {
                self.min_fps = self.min_fps.min(self.cached_fps);
                self.max_fps = self.max_fps.max(self.cached_fps);
            }
        }
    }

    /// Get the plugin manifest
    pub fn manifest(&self) -> PluginManifest {
        PluginManifest::new("com.bevy-editor.fps-counter", "FPS Counter", "0.1.0")
            .author("Bevy Editor Team")
            .description("Displays real-time FPS statistics in the status bar")
    }

    /// Called when the plugin is loaded (FFI version)
    pub fn on_load_ffi(&mut self, api: &FfiEditorApi) -> Result<(), PluginError> {
        api.log_info("FPS Counter plugin loaded!");
        Ok(())
    }

    /// Called when the plugin is unloaded (FFI version)
    pub fn on_unload_ffi(&mut self, api: &FfiEditorApi) {
        api.remove_status_item("fps");
        api.remove_status_item("frame_time");
        api.log_info("FPS Counter plugin unloaded!");
    }

    /// Called every frame (FFI version)
    pub fn on_update_ffi(&mut self, api: &FfiEditorApi, dt: f32) {
        self.update_stats(dt);

        // Update FPS status item
        let fps_text = format!("{:.0} FPS", self.cached_fps);
        let fps_tooltip = format!(
            "Frame Rate\nCurrent: {:.0} FPS\nMin: {:.0} FPS\nMax: {:.0} FPS",
            self.cached_fps,
            if self.min_fps < f32::MAX { self.min_fps } else { 0.0 },
            self.max_fps
        );
        api.set_status_item("fps", &fps_text, Some(ACTIVITY), Some(&fps_tooltip), true, 100);

        // Update frame time status item
        let frame_time_text = format!("{:.2} ms", self.cached_frame_time);
        api.set_status_item("frame_time", &frame_time_text, Some(CLOCK), Some("Frame Time (milliseconds)"), true, 99);
    }
}

impl Default for FpsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

// Export the plugin entry point
declare_plugin!(FpsPlugin, FpsPlugin::new());
