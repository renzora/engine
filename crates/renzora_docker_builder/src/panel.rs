//! Egui panel rendering for the Docker builder.
//!
//! Panel reads [`DockerBuilderState`] read-only and pushes [`UiAction`]s into
//! the [`ActionBridge`]. The drain system handles mutations.

#![allow(deprecated)]

use std::sync::RwLock;

use bevy::prelude::World;
use bevy_egui::egui::{self, Color32, RichText, ScrollArea};
use egui_phosphor::regular::{
    BROOM, CUBE, ERASER, HAMMER, PLAY, SPINNER, STOP, WARNING_CIRCLE, CHECK_CIRCLE,
};
use renzora_theme::{Theme, ThemeManager};
use renzora_ui::{EditorPanel, PanelLocation};

use crate::state::{
    ActionBridge, BuildTarget, DockerBuilderState, PanelSettings, Stage, TargetStatus, UiAction,
};

pub struct DockerBuilderPanel {
    bridge: ActionBridge,
    snapshot: RwLock<PanelSnapshot>,
}

#[derive(Default, Clone)]
struct PanelSnapshot {
    targets: Vec<BuildTarget>,
    logs: Vec<String>,
    stage: Option<Stage>,
    running: bool,
}

impl DockerBuilderPanel {
    pub fn new(bridge: ActionBridge) -> Self {
        Self {
            bridge,
            snapshot: RwLock::new(PanelSnapshot::default()),
        }
    }

    fn push_action(&self, a: UiAction) {
        if let Ok(mut q) = self.bridge.actions.lock() {
            q.push(a);
        }
    }

    fn with_settings<R>(&self, f: impl FnOnce(&mut PanelSettings) -> R) -> Option<R> {
        self.bridge.settings.lock().ok().map(|mut s| f(&mut s))
    }

    fn read_settings(&self) -> PanelSettings {
        self.bridge
            .settings
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default()
    }
}

impl EditorPanel for DockerBuilderPanel {
    fn id(&self) -> &str {
        "docker_builder"
    }

    fn title(&self) -> &str {
        "Engine Builder"
    }

    fn icon(&self) -> Option<&str> {
        Some(HAMMER)
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Bottom
    }

    fn min_size(&self) -> [f32; 2] {
        [360.0, 200.0]
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        // Mirror authoritative state into our snapshot.
        if let Some(state) = world.get_resource::<DockerBuilderState>() {
            if let Ok(mut snap) = self.snapshot.write() {
                snap.targets = state.targets.clone();
                snap.logs = state.logs.iter().cloned().collect();
                snap.stage = Some(state.stage.clone());
                snap.running = state.running;
            }
        }

        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };

        let snap = self
            .snapshot
            .read()
            .map(|s| s.clone())
            .unwrap_or_default();

        let settings = self.read_settings();

        ui.add_space(4.0);
        self.render_toolbar(ui, &snap, &settings);
        ui.add_space(4.0);

        self.render_stage_line(ui, &snap, &theme);
        ui.separator();

        // Split: top = target grid, bottom = log view.
        let avail = ui.available_height();
        let grid_height = (avail * 0.5).clamp(120.0, 360.0);

        ScrollArea::vertical()
            .id_salt("docker_builder_targets")
            .max_height(grid_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.render_target_grid(ui, &snap, &theme);
            });

        ui.separator();
        self.render_log_view(ui, &snap, &theme, &settings);
    }
}

impl DockerBuilderPanel {
    fn render_toolbar(&self, ui: &mut egui::Ui, snap: &PanelSnapshot, settings: &PanelSettings) {
        ui.horizontal(|ui| {
            let running = snap.running;

            if ui
                .add_enabled(
                    !running,
                    egui::Button::new(format!("{CUBE} Build Image")),
                )
                .on_hover_text("docker build -f docker/engine-builder/Dockerfile -t renzora/engine .")
                .clicked()
            {
                self.push_action(UiAction::BuildImage);
            }

            if ui
                .add_enabled(!running, egui::Button::new(format!("{PLAY} Build All")))
                .on_hover_text("Run scripts/build-all.sh inside the persistent container")
                .clicked()
            {
                self.push_action(UiAction::BuildAll);
            }

            if ui
                .add_enabled(running, egui::Button::new(format!("{STOP} Stop")))
                .clicked()
            {
                self.push_action(UiAction::Stop);
            }

            ui.separator();

            if ui
                .add_enabled(!running, egui::Button::new(format!("{BROOM} Clean Cache")))
                .on_hover_text("rm -rf target inside the container")
                .clicked()
            {
                self.push_action(UiAction::CleanCache);
            }

            if ui.button(format!("{ERASER} Clear Logs")).clicked() {
                self.push_action(UiAction::ClearLogs);
            }

            ui.separator();

            let mut auto_scroll = settings.auto_scroll;
            if ui.checkbox(&mut auto_scroll, "Auto-scroll").changed() {
                self.with_settings(|s| s.auto_scroll = auto_scroll);
            }
        });
    }

    fn render_stage_line(&self, ui: &mut egui::Ui, snap: &PanelSnapshot, theme: &Theme) {
        let (label, color) = match &snap.stage {
            Some(Stage::Idle) | None => ("Idle".to_string(), theme.text.muted.to_color32()),
            Some(Stage::BuildingImage) => (
                format!("{SPINNER} Building docker image…"),
                theme.semantic.accent.to_color32(),
            ),
            Some(Stage::StartingContainer) => (
                format!("{SPINNER} Starting container…"),
                theme.semantic.accent.to_color32(),
            ),
            Some(Stage::Building) => (
                format!("{SPINNER} Building all platforms…"),
                theme.semantic.accent.to_color32(),
            ),
            Some(Stage::Cleaning) => (
                format!("{SPINNER} Cleaning build cache…"),
                theme.semantic.accent.to_color32(),
            ),
            Some(Stage::Done) => (
                format!("{CHECK_CIRCLE} Done"),
                theme.semantic.success.to_color32(),
            ),
            Some(Stage::Failed(msg)) => (
                format!("{WARNING_CIRCLE} Failed: {msg}"),
                theme.semantic.error.to_color32(),
            ),
        };
        ui.label(RichText::new(label).color(color));
    }

    fn render_target_grid(&self, ui: &mut egui::Ui, snap: &PanelSnapshot, theme: &Theme) {
        let card_width = 260.0_f32;
        let total = ui.available_width();
        let cols = ((total / (card_width + 8.0)).floor() as usize).max(1);

        let chunks: Vec<&[BuildTarget]> = snap.targets.chunks(cols).collect();

        egui::Grid::new("docker_builder_grid")
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                for row in chunks {
                    for target in row {
                        render_target_card(ui, target, theme, card_width);
                    }
                    ui.end_row();
                }
            });
    }

    fn render_log_view(
        &self,
        ui: &mut egui::Ui,
        snap: &PanelSnapshot,
        theme: &Theme,
        settings: &PanelSettings,
    ) {
        let muted = theme.text.muted.to_color32();
        let err_c = theme.semantic.error.to_color32();
        let warn_c = theme.semantic.warning.to_color32();
        let header_c = theme.semantic.accent.to_color32();

        ui.horizontal(|ui| {
            ui.label(RichText::new("Logs").color(muted));
            ui.label(
                RichText::new(format!("({} lines)", snap.logs.len()))
                    .color(theme.text.disabled.to_color32())
                    .small(),
            );
            ui.separator();
            let mut filter = settings.search_filter.clone();
            if ui
                .add(egui::TextEdit::singleline(&mut filter).hint_text("filter"))
                .changed()
            {
                self.with_settings(|s| s.search_filter = filter);
            }
        });

        ScrollArea::vertical()
            .id_salt("docker_builder_logs")
            .auto_shrink([false, false])
            .stick_to_bottom(settings.auto_scroll)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                let filter_lower = settings.search_filter.to_ascii_lowercase();
                for line in &snap.logs {
                    if !filter_lower.is_empty()
                        && !line.to_ascii_lowercase().contains(&filter_lower)
                    {
                        continue;
                    }
                    let color = classify_line_color(line, err_c, warn_c, header_c, Color32::LIGHT_GRAY);
                    ui.label(
                        RichText::new(line)
                            .monospace()
                            .color(color)
                            .size(11.0),
                    );
                }
            });
    }
}

fn classify_line_color(
    line: &str,
    err: Color32,
    warn: Color32,
    header: Color32,
    default: Color32,
) -> Color32 {
    let lower = line.to_ascii_lowercase();
    if line.starts_with("=== ") {
        header
    } else if lower.contains("error") || line.starts_with("error[") {
        err
    } else if lower.contains("warn") {
        warn
    } else {
        default
    }
}

fn render_target_card(ui: &mut egui::Ui, target: &BuildTarget, theme: &Theme, width: f32) {
    let (icon, tint) = match target.status {
        TargetStatus::Pending => ("○", theme.text.disabled.to_color32()),
        TargetStatus::InProgress => (SPINNER, theme.semantic.accent.to_color32()),
        TargetStatus::Done => (CHECK_CIRCLE, theme.semantic.success.to_color32()),
        TargetStatus::Failed => (WARNING_CIRCLE, theme.semantic.error.to_color32()),
    };

    egui::Frame::new()
        .fill(theme.surfaces.faint.to_color32())
        .stroke(egui::Stroke::new(1.0, theme.widgets.border_light.to_color32()))
        .inner_margin(6.0)
        .show(ui, |ui| {
            ui.set_width(width);
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon).color(tint).monospace());
                ui.label(
                    RichText::new(&target.platform)
                        .strong()
                        .color(theme.text.primary.to_color32()),
                );
                ui.label(
                    RichText::new(format!("({})", target.feature))
                        .color(theme.text.muted.to_color32()),
                );
            });

            let bar = egui::ProgressBar::new(target.progress())
                .desired_width(width - 12.0)
                .animate(matches!(target.status, TargetStatus::InProgress));
            ui.add(bar);

            if target.crates_compiled > 0 || !target.last_line.is_empty() {
                let sub = if target.crates_compiled > 0 {
                    format!("{} crates compiled", target.crates_compiled)
                } else {
                    target.last_line.clone()
                };
                ui.label(
                    RichText::new(sub)
                        .small()
                        .color(theme.text.muted.to_color32()),
                );
            }

            if let Some(err) = &target.error {
                ui.label(
                    RichText::new(err)
                        .small()
                        .color(theme.semantic.error.to_color32()),
                );
            }
        });
}
