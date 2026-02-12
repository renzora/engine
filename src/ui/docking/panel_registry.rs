//! Panel registry for the docking system
//!
//! Manages registered panels and provides a unified interface for rendering panel content.

use super::dock_tree::PanelId;
use bevy_egui::egui::{self, Rect, Ui};
use std::collections::HashSet;

/// Minimum size constraints for panels
#[allow(dead_code)]
pub struct PanelConstraints {
    pub min_width: f32,
    pub min_height: f32,
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
}

impl Default for PanelConstraints {
    fn default() -> Self {
        Self {
            min_width: 100.0,
            min_height: 100.0,
            max_width: None,
            max_height: None,
        }
    }
}

impl PanelConstraints {
    #[allow(dead_code)]
    pub fn with_min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }

    #[allow(dead_code)]
    pub fn with_min_height(mut self, height: f32) -> Self {
        self.min_height = height;
        self
    }
}

/// Tracks which panels are available (not closed)
#[derive(Debug, Clone, Default)]
pub struct PanelAvailability {
    /// Panels that have been closed and can be re-opened
    pub closed_panels: HashSet<PanelId>,
}

impl PanelAvailability {
    /// Check if a panel is currently available (not closed)
    #[allow(dead_code)]
    pub fn is_available(&self, panel: &PanelId) -> bool {
        !self.closed_panels.contains(panel)
    }

    /// Close a panel
    pub fn close_panel(&mut self, panel: PanelId) {
        self.closed_panels.insert(panel);
    }

    /// Re-open a previously closed panel
    pub fn open_panel(&mut self, panel: &PanelId) {
        self.closed_panels.remove(panel);
    }

    /// Get list of closed panels
    #[allow(dead_code)]
    pub fn get_closed_panels(&self) -> Vec<PanelId> {
        self.closed_panels.iter().cloned().collect()
    }
}

/// Get constraints for a specific panel type
#[allow(dead_code)]
pub fn get_panel_constraints(panel: &PanelId) -> PanelConstraints {
    match panel {
        PanelId::Hierarchy => PanelConstraints::default().with_min_width(150.0),
        PanelId::Inspector => PanelConstraints::default().with_min_width(250.0),
        PanelId::Assets => PanelConstraints::default().with_min_height(100.0),
        PanelId::Console => PanelConstraints::default().with_min_height(80.0),
        PanelId::Viewport => PanelConstraints::default().with_min_width(200.0).with_min_height(200.0),
        PanelId::Animation => PanelConstraints::default().with_min_height(100.0),
        PanelId::Timeline => PanelConstraints::default().with_min_width(400.0).with_min_height(150.0),
        PanelId::CodeEditor => PanelConstraints::default().with_min_width(300.0).with_min_height(200.0),
        PanelId::ShaderPreview => PanelConstraints::default().with_min_width(300.0).with_min_height(300.0),
        PanelId::History => PanelConstraints::default().with_min_width(200.0),
        PanelId::Blueprint => PanelConstraints::default().with_min_width(400.0).with_min_height(300.0),
        PanelId::NodeLibrary => PanelConstraints::default().with_min_width(180.0).with_min_height(200.0),
        PanelId::MaterialPreview => PanelConstraints::default().with_min_width(200.0).with_min_height(200.0),
        PanelId::Settings => PanelConstraints::default().with_min_width(300.0).with_min_height(400.0),
        PanelId::Gamepad => PanelConstraints::default().with_min_width(250.0).with_min_height(300.0),
        PanelId::Performance => PanelConstraints::default().with_min_width(250.0).with_min_height(300.0),
        PanelId::RenderStats => PanelConstraints::default().with_min_width(250.0).with_min_height(300.0),
        PanelId::EcsStats => PanelConstraints::default().with_min_width(280.0).with_min_height(320.0),
        PanelId::MemoryProfiler => PanelConstraints::default().with_min_width(260.0).with_min_height(300.0),
        PanelId::PhysicsDebug => PanelConstraints::default().with_min_width(280.0).with_min_height(350.0),
        PanelId::CameraDebug => PanelConstraints::default().with_min_width(280.0).with_min_height(320.0),
        PanelId::SystemProfiler => PanelConstraints::default().with_min_width(300.0).with_min_height(280.0),
        PanelId::LevelTools => PanelConstraints::default().with_min_width(200.0).with_min_height(300.0),
        PanelId::StudioPreview => PanelConstraints::default().with_min_width(300.0).with_min_height(300.0),
        PanelId::NodeExplorer => PanelConstraints::default().with_min_width(250.0).with_min_height(300.0),
        PanelId::ImagePreview => PanelConstraints::default().with_min_width(300.0).with_min_height(300.0),
        PanelId::VideoEditor => PanelConstraints::default().with_min_width(400.0).with_min_height(300.0),
        PanelId::DAW => PanelConstraints::default().with_min_width(400.0).with_min_height(300.0),
        PanelId::ParticleEditor => PanelConstraints::default().with_min_width(350.0).with_min_height(400.0),
        PanelId::ParticlePreview => PanelConstraints::default().with_min_width(300.0).with_min_height(300.0),
        PanelId::TextureEditor => PanelConstraints::default().with_min_width(350.0).with_min_height(350.0),
        PanelId::ScriptVariables => PanelConstraints::default().with_min_width(220.0).with_min_height(200.0),
        PanelId::PixelCanvas => PanelConstraints::default().with_min_width(300.0).with_min_height(300.0),
        PanelId::PixelLayers => PanelConstraints::default().with_min_width(150.0).with_min_height(200.0),
        PanelId::PixelPalette => PanelConstraints::default().with_min_width(150.0).with_min_height(200.0),
        PanelId::PixelTools => PanelConstraints::default().with_min_width(100.0).with_min_height(200.0),
        PanelId::PixelTimeline => PanelConstraints::default().with_min_width(400.0).with_min_height(80.0),
        PanelId::PixelBrushSettings => PanelConstraints::default().with_min_width(100.0).with_min_height(150.0),
        PanelId::PhysicsPlayground => PanelConstraints::default().with_min_width(250.0).with_min_height(300.0),
        PanelId::PhysicsProperties => PanelConstraints::default().with_min_width(250.0).with_min_height(280.0),
        PanelId::PhysicsForces => PanelConstraints::default().with_min_width(250.0).with_min_height(300.0),
        PanelId::PhysicsMetrics => PanelConstraints::default().with_min_width(250.0).with_min_height(280.0),
        PanelId::PhysicsScenarios => PanelConstraints::default().with_min_width(250.0).with_min_height(300.0),
        PanelId::CollisionViz => PanelConstraints::default().with_min_width(250.0).with_min_height(300.0),
        PanelId::MovementTrails => PanelConstraints::default().with_min_width(250.0).with_min_height(250.0),
        PanelId::StressTest => PanelConstraints::default().with_min_width(280.0).with_min_height(350.0),
        PanelId::StateRecorder => PanelConstraints::default().with_min_width(250.0).with_min_height(300.0),
        PanelId::ArenaPresets => PanelConstraints::default().with_min_width(250.0).with_min_height(300.0),
        PanelId::RenderPipeline => PanelConstraints::default().with_min_width(400.0).with_min_height(300.0),
        PanelId::ShapeLibrary => PanelConstraints::default().with_min_width(200.0).with_min_height(300.0),
        PanelId::Plugin(_) => PanelConstraints::default(),
    }
}

/// All built-in panel types
#[allow(dead_code)]
pub fn all_builtin_panels() -> Vec<PanelId> {
    vec![
        PanelId::Hierarchy,
        PanelId::Inspector,
        PanelId::Assets,
        PanelId::Console,
        PanelId::Viewport,
        PanelId::Animation,
        PanelId::Timeline,
        PanelId::CodeEditor,
        PanelId::ShaderPreview,
        PanelId::History,
        PanelId::Blueprint,
        PanelId::NodeLibrary,
        PanelId::MaterialPreview,
        PanelId::Settings,
        PanelId::Gamepad,
        PanelId::Performance,
        PanelId::RenderStats,
        PanelId::EcsStats,
        PanelId::MemoryProfiler,
        PanelId::PhysicsDebug,
        PanelId::CameraDebug,
        PanelId::SystemProfiler,
        PanelId::LevelTools,
        PanelId::StudioPreview,
        PanelId::NodeExplorer,
        PanelId::ImagePreview,
        PanelId::VideoEditor,
        PanelId::DAW,
        PanelId::ParticleEditor,
        PanelId::ParticlePreview,
        PanelId::TextureEditor,
        PanelId::ScriptVariables,
        PanelId::PixelCanvas,
        PanelId::PixelLayers,
        PanelId::PixelPalette,
        PanelId::PixelTools,
        PanelId::PixelTimeline,
        PanelId::PixelBrushSettings,
        PanelId::PhysicsPlayground,
        PanelId::PhysicsProperties,
        PanelId::PhysicsForces,
        PanelId::PhysicsMetrics,
        PanelId::PhysicsScenarios,
        PanelId::CollisionViz,
        PanelId::MovementTrails,
        PanelId::StressTest,
        PanelId::StateRecorder,
        PanelId::ArenaPresets,
        PanelId::RenderPipeline,
        PanelId::ShapeLibrary,
    ]
}

/// Render a placeholder for an unknown panel type
#[allow(dead_code)]
pub fn render_placeholder_panel(ui: &mut Ui, panel: &PanelId, rect: Rect) {
    let _response = ui.allocate_rect(rect, egui::Sense::hover());

    ui.painter().rect_filled(
        rect,
        0.0,
        egui::Color32::from_gray(40),
    );

    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("{} (placeholder)", panel.title()),
        egui::FontId::default(),
        egui::Color32::from_gray(120),
    );
}
