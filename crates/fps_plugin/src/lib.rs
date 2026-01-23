//! FPS Counter Plugin for the Bevy Editor
//!
//! Displays real-time FPS (frames per second) statistics.

use editor_plugin_api::prelude::*;

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
    /// Whether to show detailed stats
    show_details: bool,
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
            show_details: false,
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

    fn fps_color(&self) -> [f32; 4] {
        if self.cached_fps >= 55.0 {
            [0.4, 0.8, 0.4, 1.0] // Green - good
        } else if self.cached_fps >= 30.0 {
            [0.9, 0.7, 0.2, 1.0] // Yellow - okay
        } else {
            [0.9, 0.3, 0.3, 1.0] // Red - bad
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
            .description("Displays real-time FPS statistics")
            .capability(PluginCapability::Panel)
    }

    fn on_load(&mut self, api: &mut dyn EditorApi) -> Result<(), PluginError> {
        api.log_info("FPS Counter plugin loaded!");

        // Register a small floating panel
        api.register_panel(
            PanelDefinition::new("fps_panel", "FPS")
                .icon("📊")
                .location(PanelLocation::Floating)
                .min_size(140.0, 80.0)
        );

        Ok(())
    }

    fn on_unload(&mut self, api: &mut dyn EditorApi) {
        api.log_info("FPS Counter plugin unloaded!");
    }

    fn on_update(&mut self, api: &mut dyn EditorApi, dt: f32) {
        // Update statistics
        self.update_stats(dt);

        // Build panel content
        let mut content = vec![
            // Main FPS display
            Widget::Label {
                text: format!("{:.0} FPS", self.cached_fps),
                style: TextStyle::Heading1,
            },
            Widget::Label {
                text: format!("{:.2} ms", self.cached_frame_time),
                style: TextStyle::Caption,
            },
        ];

        // Toggle for details
        content.push(Widget::checkbox("Details", self.show_details, UiId::new(1)));

        if self.show_details {
            content.push(Widget::Separator);
            content.push(Widget::Label {
                text: format!("Min: {:.0} FPS", if self.min_fps < f32::MAX { self.min_fps } else { 0.0 }),
                style: TextStyle::Caption,
            });
            content.push(Widget::Label {
                text: format!("Max: {:.0} FPS", self.max_fps),
                style: TextStyle::Caption,
            });

            // Reset button
            content.push(Widget::button("Reset Stats", UiId::new(2)));
        }

        api.set_panel_content("fps_panel", content);
    }

    fn on_event(&mut self, api: &mut dyn EditorApi, event: &EditorEvent) {
        if let EditorEvent::UiEvent(ui_event) = event {
            match ui_event {
                UiEvent::CheckboxToggled { id, checked } if id.0 == 1 => {
                    self.show_details = *checked;
                }
                UiEvent::ButtonClicked(id) if id.0 == 2 => {
                    // Reset stats
                    self.min_fps = f32::MAX;
                    self.max_fps = 0.0;
                    api.log_info("FPS stats reset!");
                }
                _ => {}
            }
        }
    }
}

// Export the plugin entry point
declare_plugin!(FpsPlugin, FpsPlugin::new());
