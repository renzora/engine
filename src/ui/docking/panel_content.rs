//! Panel content rendering for the docking system
//!
//! This module provides functions to render panel content into arbitrary rectangles,
//! allowing panels to be placed anywhere in the dock tree.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, Id, Pos2, Rect, Ui, Vec2};

use super::dock_tree::PanelId;
use super::renderer::TAB_BAR_HEIGHT;
use crate::theming::Theme;

/// Context for rendering a panel within a docked area
#[allow(dead_code)]
pub struct DockedPanelContext {
    /// The full rect allocated to this leaf (including tab bar)
    pub leaf_rect: Rect,
    /// The content rect (below tab bar)
    pub content_rect: Rect,
    /// The panel being rendered
    pub panel_id: PanelId,
    /// Whether this panel is the active tab
    pub is_active: bool,
    /// Whether this panel is being dragged (floating)
    pub is_floating: bool,
}

impl DockedPanelContext {
    pub fn new(leaf_rect: Rect, panel_id: PanelId, is_active: bool) -> Self {
        let content_rect = Rect::from_min_max(
            Pos2::new(leaf_rect.min.x, leaf_rect.min.y + TAB_BAR_HEIGHT),
            leaf_rect.max,
        );
        Self {
            leaf_rect,
            content_rect,
            panel_id,
            is_active,
            is_floating: false,
        }
    }

    pub fn new_floating(leaf_rect: Rect, panel_id: PanelId, is_active: bool) -> Self {
        let content_rect = Rect::from_min_max(
            Pos2::new(leaf_rect.min.x, leaf_rect.min.y + TAB_BAR_HEIGHT),
            leaf_rect.max,
        );
        Self {
            leaf_rect,
            content_rect,
            panel_id,
            is_active,
            is_floating: true,
        }
    }
}

/// Trait for panels that can be rendered in a docked context
#[allow(dead_code)]
pub trait DockablePanel {
    /// Render the panel content into the given UI
    fn render_content(&mut self, ui: &mut Ui, ctx: &DockedPanelContext);

    /// Get the panel's background color
    fn background_color(&self) -> Color32 {
        Color32::from_gray(30)
    }
}

/// Render a panel content area with proper clipping and background
pub fn render_panel_frame(
    ctx: &egui::Context,
    panel_ctx: &DockedPanelContext,
    theme: &Theme,
    add_contents: impl FnOnce(&mut Ui),
) {
    // Only render if this is the active panel
    if !panel_ctx.is_active {
        return;
    }

    // Use panel_id for stable ID - each panel should only appear once in tree
    let id_suffix = if panel_ctx.is_floating { "_floating" } else { "" };
    let id = Id::new(("docked_panel", format!("{:?}{}", panel_ctx.panel_id, id_suffix)));

    // Ensure content rect has valid non-negative dimensions
    let safe_size = egui::Vec2::new(
        panel_ctx.content_rect.width().max(1.0),
        panel_ctx.content_rect.height().max(1.0),
    );

    // Use higher order for floating panels so they appear on top
    let order = if panel_ctx.is_floating {
        egui::Order::Foreground
    } else {
        egui::Order::Middle
    };

    // For floating panels, draw a shadow first
    if panel_ctx.is_floating {
        let shadow_offset = Vec2::new(8.0, 8.0);
        let shadow_rect = panel_ctx.leaf_rect.translate(shadow_offset);
        egui::Area::new(Id::new(("panel_shadow", format!("{:?}", panel_ctx.panel_id))))
            .fixed_pos(shadow_rect.min)
            .order(egui::Order::Middle)
            .interactable(false)
            .show(ctx, |ui| {
                ui.painter().rect_filled(
                    Rect::from_min_size(Pos2::ZERO, shadow_rect.size()),
                    6.0,
                    Color32::from_rgba_unmultiplied(0, 0, 0, 80),
                );
            });
    }

    egui::Area::new(id)
        .fixed_pos(panel_ctx.content_rect.min)
        .order(order)
        .interactable(!panel_ctx.is_floating) // Floating panels shouldn't capture input
        .show(ctx, |ui| {
            ui.set_clip_rect(panel_ctx.content_rect);
            ui.set_min_size(safe_size);
            ui.set_max_size(safe_size);

            // Draw background using theme
            ui.painter().rect_filled(
                panel_ctx.content_rect,
                0.0,
                theme.surfaces.panel.to_color32(),
            );

            // Draw border for floating panels
            if panel_ctx.is_floating {
                let accent = theme.semantic.accent.to_color32();
                ui.painter().rect_stroke(
                    panel_ctx.content_rect,
                    0.0,
                    egui::Stroke::new(2.0, accent),
                    egui::StrokeKind::Inside,
                );
            }

            // Create a child UI for the content
            let mut child_rect = panel_ctx.content_rect;
            child_rect.min.x += 4.0;
            child_rect.min.y += 4.0;
            child_rect.max.x -= 4.0;
            child_rect.max.y -= 4.0;

            let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(child_rect));
            add_contents(&mut child_ui);
        });
}

/// Placeholder panel content for panels not yet implemented
#[allow(dead_code)]
pub fn render_placeholder_content(ui: &mut Ui, panel_id: &PanelId) {
    ui.vertical_centered(|ui| {
        ui.add_space(20.0);
        ui.label(
            egui::RichText::new(panel_id.icon())
                .size(32.0)
                .color(Color32::from_gray(80)),
        );
        ui.add_space(10.0);
        ui.label(
            egui::RichText::new(panel_id.title())
                .size(16.0)
                .color(Color32::from_gray(100)),
        );
        ui.add_space(5.0);
        ui.label(
            egui::RichText::new("Panel content")
                .size(12.0)
                .color(Color32::from_gray(60)),
        );
    });
}

/// Get the minimum size for a panel type
#[allow(dead_code)]
pub fn get_panel_min_size(panel_id: &PanelId) -> Vec2 {
    match panel_id {
        PanelId::Viewport => Vec2::new(200.0, 200.0),
        PanelId::Hierarchy => Vec2::new(150.0, 100.0),
        PanelId::Inspector => Vec2::new(250.0, 100.0),
        PanelId::Assets => Vec2::new(200.0, 80.0),
        PanelId::Console => Vec2::new(200.0, 60.0),
        PanelId::Animation => Vec2::new(200.0, 80.0),
        PanelId::Timeline => Vec2::new(400.0, 150.0),
        PanelId::ScriptEditor => Vec2::new(300.0, 200.0),
        PanelId::History => Vec2::new(150.0, 100.0),
        PanelId::Blueprint => Vec2::new(400.0, 300.0),
        PanelId::NodeLibrary => Vec2::new(180.0, 200.0),
        PanelId::MaterialPreview => Vec2::new(200.0, 200.0),
        PanelId::Settings => Vec2::new(300.0, 400.0),
        PanelId::Gamepad => Vec2::new(250.0, 300.0),
        PanelId::Performance => Vec2::new(250.0, 300.0),
        PanelId::RenderStats => Vec2::new(250.0, 300.0),
        PanelId::EcsStats => Vec2::new(280.0, 320.0),
        PanelId::MemoryProfiler => Vec2::new(260.0, 300.0),
        PanelId::PhysicsDebug => Vec2::new(280.0, 350.0),
        PanelId::CameraDebug => Vec2::new(280.0, 320.0),
        PanelId::SystemProfiler => Vec2::new(300.0, 280.0),
        PanelId::LevelTools => Vec2::new(200.0, 300.0),
        PanelId::StudioPreview => Vec2::new(300.0, 300.0),
        PanelId::NodeExplorer => Vec2::new(250.0, 300.0),
        PanelId::ImagePreview => Vec2::new(300.0, 300.0),
        PanelId::Plugin(_) => Vec2::new(100.0, 100.0),
    }
}
