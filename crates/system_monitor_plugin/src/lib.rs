//! System Monitor Plugin for the Bevy Editor
//!
//! Displays real-time CPU, RAM, and GPU usage in the status bar.

use editor_plugin_api::prelude::*;
use egui_phosphor::regular::{CPU, MEMORY, MONITOR};
use sysinfo::System;
use nvml_wrapper::Nvml;

/// Update interval in seconds (to avoid performance overhead)
const UPDATE_INTERVAL: f32 = 1.0;

/// System monitor plugin
pub struct SystemMonitorPlugin {
    /// System info instance for CPU/RAM
    system: System,
    /// NVML instance for GPU (optional - only works with NVIDIA)
    nvml: Option<Nvml>,
    /// Time since last update
    time_since_update: f32,
    /// Cached CPU usage
    cpu_usage: f32,
    /// Cached RAM usage (used / total in GB)
    ram_used_gb: f32,
    ram_total_gb: f32,
    /// Cached GPU usage (percentage)
    gpu_usage: Option<u32>,
    /// Cached GPU memory (used / total in GB)
    gpu_mem_used_gb: Option<f32>,
    gpu_mem_total_gb: Option<f32>,
    /// GPU name
    gpu_name: Option<String>,
}

impl SystemMonitorPlugin {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        // Try to initialize NVML for GPU monitoring
        let nvml = Nvml::init().ok();
        let gpu_name = nvml.as_ref().and_then(|n| {
            n.device_by_index(0).ok().and_then(|d| d.name().ok())
        });

        Self {
            system,
            nvml,
            time_since_update: UPDATE_INTERVAL, // Force immediate update
            cpu_usage: 0.0,
            ram_used_gb: 0.0,
            ram_total_gb: 0.0,
            gpu_usage: None,
            gpu_mem_used_gb: None,
            gpu_mem_total_gb: None,
            gpu_name,
        }
    }

    fn update_stats(&mut self) {
        // Refresh CPU and memory
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();

        // Calculate average CPU usage across all cores
        let cpus = self.system.cpus();
        if !cpus.is_empty() {
            self.cpu_usage = cpus.iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / cpus.len() as f32;
        }

        // RAM usage
        let used_mem = self.system.used_memory();
        let total_mem = self.system.total_memory();
        self.ram_used_gb = used_mem as f32 / (1024.0 * 1024.0 * 1024.0);
        self.ram_total_gb = total_mem as f32 / (1024.0 * 1024.0 * 1024.0);

        // GPU usage (NVIDIA only via NVML)
        if let Some(ref nvml) = self.nvml {
            if let Ok(device) = nvml.device_by_index(0) {
                // GPU utilization
                if let Ok(util) = device.utilization_rates() {
                    self.gpu_usage = Some(util.gpu);
                }

                // GPU memory
                if let Ok(mem_info) = device.memory_info() {
                    self.gpu_mem_used_gb = Some(mem_info.used as f32 / (1024.0 * 1024.0 * 1024.0));
                    self.gpu_mem_total_gb = Some(mem_info.total as f32 / (1024.0 * 1024.0 * 1024.0));
                }
            }
        }
    }

}

impl Default for SystemMonitorPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorPlugin for SystemMonitorPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest::new("com.bevy-editor.system-monitor", "System Monitor", "0.1.0")
            .author("Bevy Editor Team")
            .description("Displays CPU, RAM, and GPU usage in the status bar")
    }

    fn on_load(&mut self, api: &mut dyn EditorApi) -> Result<(), PluginError> {
        api.log_info("System Monitor plugin loaded!");
        if self.nvml.is_some() {
            if let Some(ref name) = self.gpu_name {
                api.log_info(&format!("GPU detected: {}", name));
            }
        } else {
            api.log_info("No NVIDIA GPU detected - GPU monitoring disabled");
        }
        Ok(())
    }

    fn on_unload(&mut self, api: &mut dyn EditorApi) {
        api.remove_status_item("cpu");
        api.remove_status_item("ram");
        api.remove_status_item("gpu");
        api.log_info("System Monitor plugin unloaded!");
    }

    fn on_update(&mut self, api: &mut dyn EditorApi, dt: f32) {
        self.time_since_update += dt;

        // Only update stats periodically to avoid performance overhead
        if self.time_since_update >= UPDATE_INTERVAL {
            self.time_since_update = 0.0;
            self.update_stats();
        }

        // CPU status
        api.set_status_item(
            StatusBarItem::new("cpu", format!("CPU {:.0}%", self.cpu_usage))
                .icon(CPU)
                .tooltip(format!(
                    "CPU Usage: {:.1}%\nCores: {}",
                    self.cpu_usage,
                    self.system.cpus().len()
                ))
                .align_right()
                .priority(80)
        );

        // RAM status
        let ram_percent = if self.ram_total_gb > 0.0 {
            (self.ram_used_gb / self.ram_total_gb) * 100.0
        } else {
            0.0
        };
        api.set_status_item(
            StatusBarItem::new("ram", format!("RAM {:.1}/{:.0} GB", self.ram_used_gb, self.ram_total_gb))
                .icon(MEMORY)
                .tooltip(format!(
                    "Memory Usage: {:.1}%\nUsed: {:.2} GB\nTotal: {:.2} GB\nAvailable: {:.2} GB",
                    ram_percent,
                    self.ram_used_gb,
                    self.ram_total_gb,
                    self.ram_total_gb - self.ram_used_gb
                ))
                .align_right()
                .priority(70)
        );

        // GPU status (only if NVML is available)
        if self.nvml.is_some() {
            let gpu_text = match self.gpu_usage {
                Some(usage) => format!("GPU {}%", usage),
                None => "GPU --".to_string(),
            };

            let tooltip = match (&self.gpu_name, self.gpu_usage, self.gpu_mem_used_gb, self.gpu_mem_total_gb) {
                (Some(name), Some(usage), Some(mem_used), Some(mem_total)) => {
                    format!(
                        "{}\nGPU Usage: {}%\nVRAM: {:.1}/{:.1} GB ({:.0}%)",
                        name,
                        usage,
                        mem_used,
                        mem_total,
                        (mem_used / mem_total) * 100.0
                    )
                }
                (Some(name), _, _, _) => format!("{}\nUsage data unavailable", name),
                _ => "GPU monitoring unavailable".to_string(),
            };

            api.set_status_item(
                StatusBarItem::new("gpu", gpu_text)
                    .icon(MONITOR)
                    .tooltip(tooltip)
                    .align_right()
                    .priority(60)
            );
        }
    }

    fn on_event(&mut self, _api: &mut dyn EditorApi, _event: &EditorEvent) {
        // No events to handle
    }
}

// Export the plugin entry point
declare_plugin!(SystemMonitorPlugin, SystemMonitorPlugin::new());
