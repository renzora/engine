//! System monitor status bar plugin — FPS counter and hardware info.

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use nvml_wrapper::Nvml;

use renzora::{
    RenderingMode, RenzoraShellExt, ResolvedRenderingMode, ShellStatusAlign, ShellStatusItem,
    ShellStatusSegment,
};
use renzora_editor::SplashState;

// ============================================================================
// State
// ============================================================================

#[derive(Resource, Clone, Default)]
struct SystemMonitorState {
    fps: f64,
    frame_time_ms: f64,
    cpu_name: String,
    gpu_name: String,
    total_ram_gb: f64,
    used_ram_gb: f64,
    gpu_usage_pct: f64,
    gpu_vram_used_gb: f64,
    gpu_vram_total_gb: f64,
    accum_secs: f32,
}

#[derive(Resource)]
struct NvmlHandle(Option<Nvml>);

impl Default for NvmlHandle {
    fn default() -> Self {
        Self(Nvml::init().ok())
    }
}

const DISPLAY_REFRESH_SECS: f32 = 0.5;

fn update_system_monitor(
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,
    mut state: ResMut<SystemMonitorState>,
) {
    state.accum_secs += time.delta_secs();
    if state.accum_secs < DISPLAY_REFRESH_SECS {
        return;
    }
    state.accum_secs = 0.0;

    if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
        if let Some(val) = fps.average() {
            state.fps = val;
        }
    }
    if let Some(ft) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FRAME_TIME) {
        if let Some(val) = ft.average() {
            state.frame_time_ms = val;
        }
    }
}

fn init_hardware_info(mut state: ResMut<SystemMonitorState>) {
    use sysinfo::System;

    let mut sys = System::new();
    sys.refresh_memory();

    state.total_ram_gb = sys.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0);

    state.cpu_name = System::cpu_arch();

    // GPU name from Bevy's renderer adapter isn't available here easily,
    // so we leave it for the render world to fill in.
    state.gpu_name = String::new();
}

fn update_memory_info(mut state: ResMut<SystemMonitorState>) {
    use sysinfo::System;

    let mut sys = System::new();
    sys.refresh_memory();
    state.used_ram_gb = sys.used_memory() as f64 / (1024.0 * 1024.0 * 1024.0);
}

fn update_gpu_stats(nvml: Res<NvmlHandle>, mut state: ResMut<SystemMonitorState>) {
    let Some(nvml) = nvml.0.as_ref() else { return };
    let Ok(device) = nvml.device_by_index(0) else {
        return;
    };

    if let Ok(util) = device.utilization_rates() {
        state.gpu_usage_pct = util.gpu as f64;
    }
    if let Ok(mem) = device.memory_info() {
        let gb = 1024.0 * 1024.0 * 1024.0;
        state.gpu_vram_used_gb = mem.used as f64 / gb;
        state.gpu_vram_total_gb = mem.total as f64 / gb;
    }
}

// ============================================================================
// GPU name extraction from Bevy's render adapter
// ============================================================================

fn extract_gpu_name(
    adapter: Option<Res<bevy::render::renderer::RenderAdapterInfo>>,
    mut state: ResMut<SystemMonitorState>,
) {
    if state.gpu_name.is_empty() {
        if let Some(info) = adapter {
            state.gpu_name = info.name.clone();
        }
    }
}

// ============================================================================
// Plugin
// ============================================================================

#[derive(Default)]
pub struct SystemMonitorPlugin;

impl Plugin for SystemMonitorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] SystemMonitorPlugin");

        if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        }

        app.init_resource::<SystemMonitorState>();
        app.init_resource::<NvmlHandle>();

        app.add_systems(Startup, init_hardware_info);
        app.add_systems(
            Update,
            (
                update_system_monitor,
                update_memory_info,
                update_gpu_stats,
                extract_gpu_name,
            )
                .run_if(in_state(SplashState::Editor)),
        );

        // bevy_ui shell status item (the egui-free path).
        app.register_shell_status_item(ShellStatusItem {
            id: "system_monitor",
            align: ShellStatusAlign::Right,
            order: 0,
            render: monitor_status_segments,
        });
    }
}

/// The bevy_ui status segments — same values + order as the egui status item.
fn monitor_status_segments(world: &World) -> Vec<ShellStatusSegment> {
    let Some(s) = world.get_resource::<SystemMonitorState>() else {
        return Vec::new();
    };
    const SECONDARY: [u8; 3] = [200, 200, 210];
    const MUTED: [u8; 3] = [150, 150, 164];
    const AMBER: [u8; 3] = [220, 180, 50];

    let mut out = Vec::new();
    let fps_color = if s.fps >= 55.0 {
        [100, 200, 100]
    } else if s.fps >= 30.0 {
        AMBER
    } else {
        [220, 80, 80]
    };
    out.push(ShellStatusSegment::new(
        "speedometer",
        format!("{:.0} FPS ({:.2}ms)", s.fps, s.frame_time_ms),
        fps_color,
    ));
    out.push(ShellStatusSegment::new(
        "memory",
        format!("{:.1} / {:.0} GB", s.used_ram_gb, s.total_ram_gb),
        SECONDARY,
    ));
    if s.gpu_vram_total_gb > 0.0 {
        out.push(ShellStatusSegment::new(
            "graphics-card",
            format!(
                "{:.0}% {:.1}/{:.0} GB",
                s.gpu_usage_pct, s.gpu_vram_used_gb, s.gpu_vram_total_gb
            ),
            SECONDARY,
        ));
    }
    if let Some(mode) = world.get_resource::<ResolvedRenderingMode>() {
        let (label, color) = match mode.0 {
            RenderingMode::Deferred => ("Deferred", AMBER),
            _ => ("Forward", SECONDARY),
        };
        out.push(ShellStatusSegment::new("stack", label, color));
    }
    if !s.gpu_name.is_empty() {
        out.push(ShellStatusSegment::new("graphics-card", s.gpu_name.clone(), MUTED));
    }
    out
}

renzora::add!(SystemMonitorPlugin, Editor);
