//! Test extension — demonstrates how to add panels, layouts, and document tabs
//! to the Renzora editor from an external crate.

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor::{
    DockTree, DocumentTabState, EditorPanel, LayoutManager, PanelLocation, PanelRegistry, TabKind,
    WorkspaceLayout,
};
use renzora_theme::ThemeManager;

// ── Viewport panel ──────────────────────────────────────────────────────────

pub struct ViewportPanel;

impl EditorPanel for ViewportPanel {
    fn id(&self) -> &str {
        "viewport"
    }

    fn title(&self) -> &str {
        "Viewport"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::MONITOR)
    }

    fn ui(&self, ui: &mut egui::Ui, _world: &World) {
        let rect = ui.available_rect_before_wrap();

        // Dark background simulating a 3D viewport
        ui.painter()
            .rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 20, 25));

        // Center crosshair
        let center = rect.center();
        let cross_color = egui::Color32::from_white_alpha(40);
        ui.painter().line_segment(
            [
                egui::Pos2::new(center.x - 20.0, center.y),
                egui::Pos2::new(center.x + 20.0, center.y),
            ],
            egui::Stroke::new(1.0, cross_color),
        );
        ui.painter().line_segment(
            [
                egui::Pos2::new(center.x, center.y - 20.0),
                egui::Pos2::new(center.x, center.y + 20.0),
            ],
            egui::Stroke::new(1.0, cross_color),
        );

        // Info text
        ui.painter().text(
            egui::Pos2::new(rect.min.x + 12.0, rect.min.y + 12.0),
            egui::Align2::LEFT_TOP,
            "3D Viewport",
            egui::FontId::proportional(11.0),
            egui::Color32::from_white_alpha(80),
        );

        ui.painter().text(
            egui::Pos2::new(rect.max.x - 12.0, rect.min.y + 12.0),
            egui::Align2::RIGHT_TOP,
            format!("{:.0} x {:.0}", rect.width(), rect.height()),
            egui::FontId::proportional(11.0),
            egui::Color32::from_white_alpha(50),
        );
    }

    fn closable(&self) -> bool {
        false
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Center
    }
}

// ── Inspector panel ─────────────────────────────────────────────────────────

pub struct InspectorPanel;

impl EditorPanel for InspectorPanel {
    fn id(&self) -> &str {
        "inspector"
    }

    fn title(&self) -> &str {
        "Inspector"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::SLIDERS)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world
            .get_resource::<ThemeManager>()
            .map(|tm| &tm.active_theme);

        ui.add_space(4.0);

        if let Some(theme) = theme {
            renzora_editor::section_header(ui, "Properties", theme);

            for (i, prop) in ["Position", "Rotation", "Scale"].iter().enumerate() {
                renzora_editor::inline_property(ui, i, prop, theme, |ui| {
                    ui.label("0.0, 0.0, 0.0");
                });
            }

            ui.add_space(8.0);
            renzora_editor::section_header(ui, "Material", theme);

            for (i, prop) in ["Color", "Roughness", "Metallic"].iter().enumerate() {
                renzora_editor::inline_property(ui, i + 3, prop, theme, |ui| {
                    ui.label("default");
                });
            }
        } else {
            ui.label("No entity selected.");
        }
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Right
    }
}

// ── Console panel ───────────────────────────────────────────────────────────

pub struct ConsolePanel {
    lines: RwLock<Vec<(String, ConsoleLevel)>>,
}

#[derive(Clone)]
enum ConsoleLevel {
    Info,
    Warn,
    Error,
}

impl Default for ConsolePanel {
    fn default() -> Self {
        Self {
            lines: RwLock::new(vec![
                ("[info] Editor initialized".into(), ConsoleLevel::Info),
                ("[info] Scene loaded: Untitled Scene".into(), ConsoleLevel::Info),
                ("[warn] No skybox assigned".into(), ConsoleLevel::Warn),
                ("[info] 3 entities in scene".into(), ConsoleLevel::Info),
                ("[error] Shader compilation failed: missing uniform".into(), ConsoleLevel::Error),
                ("[info] Auto-save complete".into(), ConsoleLevel::Info),
            ]),
        }
    }
}

impl EditorPanel for ConsolePanel {
    fn id(&self) -> &str {
        "console"
    }

    fn title(&self) -> &str {
        "Console"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::TERMINAL)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world
            .get_resource::<ThemeManager>()
            .map(|tm| &tm.active_theme);

        let lines = self.lines.read().unwrap();

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (text, level) in lines.iter() {
                let color = match (level, theme) {
                    (ConsoleLevel::Info, Some(t)) => t.text.muted.to_color32(),
                    (ConsoleLevel::Warn, Some(t)) => t.semantic.warning.to_color32(),
                    (ConsoleLevel::Error, Some(t)) => t.semantic.error.to_color32(),
                    (ConsoleLevel::Info, None) => egui::Color32::GRAY,
                    (ConsoleLevel::Warn, None) => egui::Color32::YELLOW,
                    (ConsoleLevel::Error, None) => egui::Color32::RED,
                };
                ui.label(egui::RichText::new(text).font(egui::FontId::monospace(11.0)).color(color));
            }
        });
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Bottom
    }
}

// ── Assets panel ────────────────────────────────────────────────────────────

pub struct AssetsPanel;

impl EditorPanel for AssetsPanel {
    fn id(&self) -> &str {
        "assets"
    }

    fn title(&self) -> &str {
        "Assets"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::FOLDER_OPEN)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world
            .get_resource::<ThemeManager>()
            .map(|tm| &tm.active_theme);

        ui.add_space(4.0);

        if let Some(theme) = theme {
            renzora_editor::section_header(ui, "Project Files", theme);
        }

        let folders = ["meshes/", "textures/", "materials/", "scripts/", "scenes/", "audio/"];
        for folder in &folders {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(regular::FOLDER)
                        .font(egui::FontId::proportional(12.0))
                        .color(egui::Color32::from_rgb(220, 190, 100)),
                );
                ui.label(*folder);
            });
        }
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Bottom
    }
}

// ── Performance panel ───────────────────────────────────────────────────────

pub struct PerformancePanel;

impl EditorPanel for PerformancePanel {
    fn id(&self) -> &str {
        "performance"
    }

    fn title(&self) -> &str {
        "Performance"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::CHART_LINE)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world
            .get_resource::<ThemeManager>()
            .map(|tm| &tm.active_theme);

        ui.add_space(4.0);

        if let Some(theme) = theme {
            renzora_editor::section_header(ui, "Stats", theme);

            let stats = [
                ("FPS", "60.0"),
                ("Frame Time", "16.6ms"),
                ("Draw Calls", "128"),
                ("Triangles", "45,230"),
                ("Entities", "3"),
                ("GPU Memory", "256 MB"),
            ];
            for (i, (label, value)) in stats.iter().enumerate() {
                renzora_editor::inline_property(ui, i, label, theme, |ui| {
                    ui.label(*value);
                });
            }
        }
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Bottom
    }
}

// ── Code Editor panel ───────────────────────────────────────────────────────

pub struct CodeEditorPanel;

impl EditorPanel for CodeEditorPanel {
    fn id(&self) -> &str {
        "code_editor"
    }

    fn title(&self) -> &str {
        "Code Editor"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::CODE)
    }

    fn ui(&self, ui: &mut egui::Ui, _world: &World) {
        let code = r#"fn setup(mut commands: Commands) {
    // Spawn a 3D camera
    commands.spawn(Camera3d::default());

    // Spawn a light
    commands.spawn(DirectionalLight {
        illuminance: 10000.0,
        ..default()
    });

    // Spawn a cube
    commands.spawn(Mesh3d(meshes.add(Cuboid::default())));
}"#;

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut code.to_string())
                    .font(egui::FontId::monospace(12.0))
                    .desired_width(f32::INFINITY)
                    .frame(false),
            );
        });
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Center
    }
}

// ── Custom layouts ──────────────────────────────────────────────────────────

fn layout_content_creation() -> WorkspaceLayout {
    WorkspaceLayout {
        name: "Content".into(),
        tree: DockTree::horizontal(
            DockTree::leaf("assets"),
            DockTree::horizontal(
                DockTree::leaf("viewport"),
                DockTree::leaf("inspector"),
                0.7,
            ),
            0.2,
        ),
    }
}

fn layout_profiling() -> WorkspaceLayout {
    WorkspaceLayout {
        name: "Profiling".into(),
        tree: DockTree::horizontal(
            DockTree::vertical(
                DockTree::leaf("viewport"),
                DockTree::leaf("console"),
                0.65,
            ),
            DockTree::vertical(
                DockTree::leaf("performance"),
                DockTree::leaf("inspector"),
                0.5,
            ),
            0.65,
        ),
    }
}

// ── Plugin ──────────────────────────────────────────────────────────────────

/// Test extension plugin — registers demo panels, layouts, and a document tab.
pub struct TestExtensionPlugin;

impl Plugin for TestExtensionPlugin {
    fn build(&self, app: &mut App) {
        let world = app.world_mut();

        // Register panels
        let mut registry = world
            .remove_resource::<PanelRegistry>()
            .unwrap_or_default();

        registry.register(ViewportPanel);
        registry.register(InspectorPanel);
        registry.register(ConsolePanel::default());
        registry.register(AssetsPanel);
        registry.register(PerformancePanel);
        registry.register(CodeEditorPanel);

        world.insert_resource(registry);

        // Add custom layouts
        let mut layouts = world
            .remove_resource::<LayoutManager>()
            .unwrap_or_default();

        layouts.layouts.push(layout_content_creation());
        layouts.layouts.push(layout_profiling());

        world.insert_resource(layouts);

        // Add a default document tab
        let mut tabs = world
            .remove_resource::<DocumentTabState>()
            .unwrap_or_default();

        tabs.add_tab("player_controller.rhai".into(), TabKind::Script);

        world.insert_resource(tabs);
    }
}
