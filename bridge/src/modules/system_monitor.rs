use std::sync::{Arc, Mutex};
use std::time::Duration;
use sysinfo::System;
use serde::{Serialize, Deserialize};
use tokio::time::interval;
use log::info;

#[cfg(feature = "nvidia")]
use nvml_wrapper::Nvml;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStats {
    pub cpu_usage: f32,
    pub memory_usage: f32,
    pub memory_total: u64,
    pub memory_used: u64,
    pub gpu_usage: Option<f32>,
    pub gpu_memory_usage: Option<f32>,
    pub gpu_memory_total: Option<u64>,
    pub gpu_memory_used: Option<u64>,
    pub gpu_name: Option<String>,
    pub timestamp: u64,
}

impl Default for SystemStats {
    fn default() -> Self {
        Self {
            cpu_usage: 0.0,
            memory_usage: 0.0,
            memory_total: 0,
            memory_used: 0,
            gpu_usage: None,
            gpu_memory_usage: None,
            gpu_memory_total: None,
            gpu_memory_used: None,
            gpu_name: None,
            timestamp: 0,
        }
    }
}

#[derive(Debug)]
pub struct SystemMonitor {
    system: Arc<Mutex<System>>,
    stats: Arc<Mutex<SystemStats>>,
    #[cfg(feature = "nvidia")]
    nvml: Option<Arc<Nvml>>,
}

impl SystemMonitor {
    pub fn new() -> Self {
        let system = Arc::new(Mutex::new(System::new_all()));
        let stats = Arc::new(Mutex::new(SystemStats::default()));
        
        #[cfg(feature = "nvidia")]
        let nvml = match Nvml::init() {
            Ok(nvml) => {
                info!("🎮 NVIDIA GPU monitoring initialized");
                Some(Arc::new(nvml))
            }
            Err(_) => {
                info!("⚠️  NVIDIA GPU monitoring not available");
                None
            }
        };

        Self {
            system,
            stats,
            #[cfg(feature = "nvidia")]
            nvml,
        }
    }

    pub fn start_monitoring(&self) {
        let system = Arc::clone(&self.system);
        let stats = Arc::clone(&self.stats);
        
        #[cfg(feature = "nvidia")]
        let nvml = self.nvml.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(50)); // 50ms = 20fps  
            info!("📊 System monitoring started (50ms intervals for real-time)");

            loop {
                interval.tick().await;
                
                // Update system information
                if let Ok(mut sys) = system.lock() {
                    sys.refresh_cpu_usage();
                    sys.refresh_memory();
                }

                // Calculate stats
                if let Ok(sys) = system.lock() {
                    let cpu_usage = sys.global_cpu_info().cpu_usage();
                    let memory_total = sys.total_memory();
                    let memory_used = sys.used_memory();
                    let memory_usage = if memory_total > 0 {
                        (memory_used as f32 / memory_total as f32) * 100.0
                    } else {
                        0.0
                    };

                    #[cfg(feature = "nvidia")]
                    let mut gpu_usage = None;
                    #[cfg(not(feature = "nvidia"))]
                    let gpu_usage = None;
                    
                    #[cfg(feature = "nvidia")]
                    let mut gpu_memory_usage = None;
                    #[cfg(not(feature = "nvidia"))]
                    let gpu_memory_usage = None;
                    
                    #[cfg(feature = "nvidia")]
                    let mut gpu_memory_total = None;
                    #[cfg(not(feature = "nvidia"))]
                    let gpu_memory_total = None;
                    
                    #[cfg(feature = "nvidia")]
                    let mut gpu_memory_used = None;
                    #[cfg(not(feature = "nvidia"))]
                    let gpu_memory_used = None;
                    
                    #[cfg(feature = "nvidia")]
                    let mut gpu_name = None;
                    #[cfg(not(feature = "nvidia"))]
                    let gpu_name = None;

                    // Get GPU stats if available
                    #[cfg(feature = "nvidia")]
                    if let Some(ref nvml) = nvml {
                        if let Ok(device_count) = nvml.device_count() {
                            if device_count > 0 {
                                if let Ok(device) = nvml.device_by_index(0) {
                                    if let Ok(utilization) = device.utilization_rates() {
                                        gpu_usage = Some(utilization.gpu as f32);
                                    }
                                    
                                    if let Ok(memory_info) = device.memory_info() {
                                        gpu_memory_total = Some(memory_info.total);
                                        gpu_memory_used = Some(memory_info.used);
                                        gpu_memory_usage = Some((memory_info.used as f32 / memory_info.total as f32) * 100.0);
                                    }
                                    
                                    if let Ok(name) = device.name() {
                                        gpu_name = Some(name);
                                    }
                                }
                            }
                        }
                    }

                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or(Duration::ZERO)
                        .as_secs();

                    // Update shared stats
                    if let Ok(mut stats) = stats.lock() {
                        *stats = SystemStats {
                            cpu_usage,
                            memory_usage,
                            memory_total,
                            memory_used,
                            gpu_usage,
                            gpu_memory_usage,
                            gpu_memory_total,
                            gpu_memory_used,
                            gpu_name,
                            timestamp,
                        };
                    }
                }
            }
        });
    }

    pub fn get_stats(&self) -> SystemStats {
        if let Ok(stats) = self.stats.lock() {
            stats.clone()
        } else {
            SystemStats::default()
        }
    }
}

// Global monitor instance
use std::sync::OnceLock;
static SYSTEM_MONITOR: OnceLock<SystemMonitor> = OnceLock::new();

pub fn initialize_system_monitor() {
    let monitor = SystemMonitor::new();
    monitor.start_monitoring();
    SYSTEM_MONITOR.set(monitor).expect("Failed to set system monitor");
    info!("🖥️  System monitor initialized");
}

pub fn get_system_stats() -> SystemStats {
    SYSTEM_MONITOR
        .get()
        .map(|monitor| monitor.get_stats())
        .unwrap_or_default()
}