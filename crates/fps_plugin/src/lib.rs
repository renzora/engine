//! FPS Counter Plugin for the Bevy Editor
//!
//! Displays real-time FPS (frames per second) statistics in the status bar.

use editor_plugin_api::prelude::*;
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
            frame_times: vec![0.016; FPS_SAMPLE_COUNT], // Initialize with ~60fps
            frame_index: 0,
            cached_fps: 60.0,
            cached_frame_time: 16.67,
            min_fps: f32::MAX,
            max_fps: 0.0,
            update_counter: 0,
        }
    }

    fn update_stats(&mut self, dt: f32) {
        // Store frame time in ring buffer
        self.frame_times[self.frame_index] = dt;
        self.frame_index = (self.frame_index + 1) % FPS_SAMPLE_COUNT;

        self.update_counter += 1;

        // Update cached values every 10 frames for stability
        if self.update_counter >= 10 {
            self.update_counter = 0;

            // Calculate average frame time
            let avg_frame_time: f32 = self.frame_times.iter().sum::<f32>() / FPS_SAMPLE_COUNT as f32;
            self.cached_frame_time = avg_frame_time * 1000.0; // Convert to ms
            self.cached_fps = if avg_frame_time > 0.0 { 1.0 / avg_frame_time } else { 0.0 };

            // Track min/max
            if self.cached_fps > 0.0 && self.cached_fps < 10000.0 {
                self.min_fps = self.min_fps.min(self.cached_fps);
                self.max_fps = self.max_fps.max(self.cached_fps);
            }
        }
    }

}

impl Default for FpsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorPlugin for FpsPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest::new("com.bevy-editor.fps-counter", "FPS Counter", "0.1.0")
            .author("Bevy Editor Team")
            .description("Displays real-time FPS statistics in the status bar")
    }

    fn on_load(&mut self, api: &mut dyn EditorApi) -> Result<(), PluginError> {
        api.log_info("FPS Counter plugin loaded!");
        Ok(())
    }

    fn on_unload(&mut self, api: &mut dyn EditorApi) {
        api.remove_status_item("fps");
        api.remove_status_item("frame_time");
        api.log_info("FPS Counter plugin unloaded!");
    }

    fn on_update(&mut self, api: &mut dyn EditorApi, dt: f32) {
        // Update statistics
        self.update_stats(dt);

        // Update status bar items
        api.set_status_item(
            StatusBarItem::new("fps", format!("{:.0} FPS", self.cached_fps))
                .icon(ACTIVITY)
                .tooltip(format!(
                    "Frame Rate\nCurrent: {:.0} FPS\nMin: {:.0} FPS\nMax: {:.0} FPS",
                    self.cached_fps,
                    if self.min_fps < f32::MAX { self.min_fps } else { 0.0 },
                    self.max_fps
                ))
                .align_right()
                .priority(100)
        );

        api.set_status_item(
            StatusBarItem::new("frame_time", format!("{:.2} ms", self.cached_frame_time))
                .icon(CLOCK)
                .tooltip("Frame Time (milliseconds)")
                .align_right()
                .priority(99)
        );
    }

    fn on_event(&mut self, _api: &mut dyn EditorApi, _event: &EditorEvent) {
        // No UI events to handle
    }
}

// Export the plugin entry point
declare_plugin!(FpsPlugin, FpsPlugin::new());
