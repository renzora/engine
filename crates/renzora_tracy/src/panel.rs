//! Tracy status panel — registered under Debug.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, RichText};
use renzora_editor::{AppEditorExt, EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;

pub(crate) fn register(app: &mut App) {
    app.register_panel(TracyPanel);
}

struct TracyPanel;

impl EditorPanel for TracyPanel {
    fn id(&self) -> &str {
        "tracy_profiler"
    }
    fn title(&self) -> &str {
        "Tracy Profiler"
    }
    fn icon(&self) -> Option<&str> {
        Some(egui_phosphor::regular::MAGNIFYING_GLASS)
    }
    fn category(&self) -> &str {
        "Debug"
    }
    fn default_location(&self) -> PanelLocation {
        PanelLocation::Bottom
    }
    fn min_size(&self) -> [f32; 2] {
        [240.0, 180.0]
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world
            .get_resource::<ThemeManager>()
            .map(|tm| tm.active_theme.clone())
            .unwrap_or_default();

        egui::Frame::NONE
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Tracy")
                            .size(16.0)
                            .color(theme.text.primary.to_color32())
                            .strong(),
                    );
                    ui.label(
                        RichText::new("READY (on-demand)")
                            .size(10.0)
                            .color(Color32::from_rgb(100, 200, 100))
                            .strong(),
                    );
                });

                ui.add_space(8.0);

                ui.label(
                    RichText::new(
                        "Tracy is compiled in but dormant. Launch Tracy GUI 0.11.x and \
                         connect to localhost — only then does the client start a thread \
                         and capture events. Bevy systems, render-graph nodes, and renzora \
                         spans (voxel.clear / inject / resolve, lumen.trace, geometry.*) \
                         appear as zones; renzora plots include entity_count and \
                         frame_time_ms.",
                    )
                    .size(10.0)
                    .color(theme.text.secondary.to_color32()),
                );

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                ui.label(
                    RichText::new("Tracy GUI")
                        .size(11.0)
                        .color(theme.text.muted.to_color32()),
                );
                ui.add_space(2.0);
                ui.label(
                    RichText::new(
                        "Download from github.com/wolfpld/tracy/releases — match the 0.11.x major.minor \
                         that ships in tracing-tracy 0.11.4 (currently Tracy 0.11.x).",
                    )
                    .size(10.0)
                    .color(theme.text.secondary.to_color32()),
                );
            });
    }
}
