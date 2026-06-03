//! System monitor status bar plugin — FPS counter and hardware info.

use std::sync::RwLock;

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use nvml_wrapper::Nvml;

use renzora::{
    RenderingMode, RenzoraShellExt, ResolvedRenderingMode, ShellStatusAlign, ShellStatusItem,
    ShellStatusSegment,
};
use renzora_editor::{AppEditorExt, SplashState, StatusBarAlignment, StatusBarItem};
use renzora_theme::ThemeManager;

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
// Status bar item
// ============================================================================

struct SystemMonitorStatusItem {
    state: RwLock<SystemMonitorState>,
}

impl Default for SystemMonitorStatusItem {
    fn default() -> Self {
        Self {
            state: RwLock::new(SystemMonitorState::default()),
        }
    }
}

impl StatusBarItem for SystemMonitorStatusItem {
    fn id(&self) -> &str {
        "system_monitor"
    }

    fn alignment(&self) -> StatusBarAlignment {
        StatusBarAlignment::Right
    }

    fn order(&self) -> i32 {
        100
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        // Sync from world resource
        if let Some(world_state) = world.get_resource::<SystemMonitorState>() {
            if let Ok(mut local) = self.state.write() {
                *local = world_state.clone();
            }
        }

        let theme = world
            .get_resource::<ThemeManager>()
            .map(|tm| tm.active_theme.clone())
            .unwrap_or_default();

        let text_color = theme.text.secondary.to_color32();
        let muted_color = theme.text.muted.to_color32();

        let state = match self.state.read() {
            Ok(s) => s,
            Err(_) => return,
        };

        let fps_color = if state.fps >= 55.0 {
            egui::Color32::from_rgb(100, 200, 100) // green
        } else if state.fps >= 30.0 {
            egui::Color32::from_rgb(220, 180, 50) // yellow
        } else {
            egui::Color32::from_rgb(220, 80, 80) // red
        };

        // GPU name
        if !state.gpu_name.is_empty() {
            ui.label(
                egui::RichText::new(format!("{} {}", regular::GRAPHICS_CARD, state.gpu_name))
                    .size(11.0)
                    .color(muted_color),
            );
        }

        // Rendering mode (Forward/Deferred), shown next to the renderer. Read
        // live every frame so the deferred-prepass race is observable: watch
        // whether a load resolves to "Deferred" (the crash-prone path) and
        // whether it differs between project loads. Never "Auto" — by now the
        // abstract preference is resolved to a concrete path.
        if let Some(mode) = world.get_resource::<ResolvedRenderingMode>() {
            let (label, color) = match mode.0 {
                RenderingMode::Deferred => {
                    // amber — the known-fragile path that triggers the crash
                    ("Deferred", egui::Color32::from_rgb(220, 180, 50))
                }
                _ => ("Forward", text_color),
            };
            ui.label(
                egui::RichText::new(format!("{} {}", regular::STACK, label))
                    .size(11.0)
                    .color(color),
            );
        }

        // GPU usage + VRAM
        if state.gpu_vram_total_gb > 0.0 {
            ui.label(
                egui::RichText::new(format!(
                    "{} {:.0}% {:.1}/{:.0} GB",
                    regular::GRAPHICS_CARD,
                    state.gpu_usage_pct,
                    state.gpu_vram_used_gb,
                    state.gpu_vram_total_gb,
                ))
                .size(11.0)
                .color(text_color),
            );
        }

        // RAM
        ui.label(
            egui::RichText::new(format!(
                "{} {:.1} / {:.0} GB",
                regular::MEMORY,
                state.used_ram_gb,
                state.total_ram_gb,
            ))
            .size(11.0)
            .color(text_color),
        );

        // FPS + frame time
        ui.label(
            egui::RichText::new(format!(
                "{} {:.0} FPS ({:.2}ms)",
                regular::SPEEDOMETER,
                state.fps,
                state.frame_time_ms,
            ))
            .size(11.0)
            .color(fps_color),
        );
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

        app.register_status_item(SystemMonitorStatusItem::default());
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
