//! System Monitor Plugin for the Bevy Editor
//!
//! Displays real-time CPU, RAM, and GPU usage in the status bar.

use editor_plugin_api::prelude::*;
use editor_plugin_api::ffi::FfiEditorApi;
use egui_phosphor::regular::{CPU, MEMORY, MONITOR};
use sysinfo::System;
use nvml_wrapper::Nvml;

/// Update interval in seconds
const UPDATE_INTERVAL: f32 = 1.0;

/// System monitor plugin
pub struct SystemMonitorPlugin {
    system: System,
    nvml: Option<Nvml>,
    time_since_update: f32,
    cpu_usage: f32,
    ram_used_gb: f32,
    ram_total_gb: f32,
    gpu_usage: Option<u32>,
    gpu_mem_used_gb: Option<f32>,
    gpu_mem_total_gb: Option<f32>,
    gpu_name: Option<String>,
}

impl SystemMonitorPlugin {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        let nvml = Nvml::init().ok();
        let gpu_name = nvml.as_ref().and_then(|n| {
            n.device_by_index(0).ok().and_then(|d| d.name().ok())
        });

        Self {
            system,
            nvml,
            time_since_update: UPDATE_INTERVAL,
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
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();

        let cpus = self.system.cpus();
        if !cpus.is_empty() {
            self.cpu_usage = cpus.iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / cpus.len() as f32;
        }

        let used_mem = self.system.used_memory();
        let total_mem = self.system.total_memory();
        self.ram_used_gb = used_mem as f32 / (1024.0 * 1024.0 * 1024.0);
        self.ram_total_gb = total_mem as f32 / (1024.0 * 1024.0 * 1024.0);

        if let Some(ref nvml) = self.nvml {
            if let Ok(device) = nvml.device_by_index(0) {
                if let Ok(util) = device.utilization_rates() {
                    self.gpu_usage = Some(util.gpu);
                }
                if let Ok(mem_info) = device.memory_info() {
                    self.gpu_mem_used_gb = Some(mem_info.used as f32 / (1024.0 * 1024.0 * 1024.0));
                    self.gpu_mem_total_gb = Some(mem_info.total as f32 / (1024.0 * 1024.0 * 1024.0));
                }
            }
        }
    }

    pub fn manifest(&self) -> PluginManifest {
        PluginManifest::new("com.bevy-editor.system-monitor", "System Monitor", "0.1.0")
            .author("Bevy Editor Team")
            .description("Displays CPU, RAM, and GPU usage in the status bar")
    }

    pub fn on_load_ffi(&mut self, api: &FfiEditorApi) -> Result<(), PluginError> {
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

    pub fn on_unload_ffi(&mut self, api: &FfiEditorApi) {
        api.remove_status_item("cpu");
        api.remove_status_item("ram");
        api.remove_status_item("gpu");
        api.log_info("System Monitor plugin unloaded!");
    }

    pub fn on_update_ffi(&mut self, api: &FfiEditorApi, dt: f32) {
        self.time_since_update += dt;

        if self.time_since_update >= UPDATE_INTERVAL {
            self.time_since_update = 0.0;
            self.update_stats();
        }

        // CPU status
        let cpu_text = format!("CPU {:.0}%", self.cpu_usage);
        let cpu_tooltip = format!("CPU Usage: {:.1}%\nCores: {}", self.cpu_usage, self.system.cpus().len());
        api.set_status_item("cpu", &cpu_text, Some(CPU), Some(&cpu_tooltip), true, 80);

        // RAM status
        let ram_text = format!("RAM {:.1}/{:.0} GB", self.ram_used_gb, self.ram_total_gb);
        let ram_percent = if self.ram_total_gb > 0.0 {
            (self.ram_used_gb / self.ram_total_gb) * 100.0
        } else {
            0.0
        };
        let ram_tooltip = format!(
            "Memory Usage: {:.1}%\nUsed: {:.2} GB\nTotal: {:.2} GB",
            ram_percent, self.ram_used_gb, self.ram_total_gb
        );
        api.set_status_item("ram", &ram_text, Some(MEMORY), Some(&ram_tooltip), true, 70);

        // GPU status
        if self.nvml.is_some() {
            let gpu_text = match self.gpu_usage {
                Some(usage) => format!("GPU {}%", usage),
                None => "GPU --".to_string(),
            };
            let tooltip = match (&self.gpu_name, self.gpu_usage, self.gpu_mem_used_gb, self.gpu_mem_total_gb) {
                (Some(name), Some(usage), Some(mem_used), Some(mem_total)) => {
                    format!("{}\nGPU Usage: {}%\nVRAM: {:.1}/{:.1} GB", name, usage, mem_used, mem_total)
                }
                (Some(name), _, _, _) => format!("{}\nUsage data unavailable", name),
                _ => "GPU monitoring unavailable".to_string(),
            };
            api.set_status_item("gpu", &gpu_text, Some(MONITOR), Some(&tooltip), true, 60);
        }
    }
}

impl Default for SystemMonitorPlugin {
    fn default() -> Self {
        Self::new()
    }
}

declare_plugin!(SystemMonitorPlugin, SystemMonitorPlugin::new());
