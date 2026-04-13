//! Context-sensitive viewport-header options for terrain brush tools.
//!
//! Registered with the `ToolOptionsRegistry` — these replace the default
//! inline header content (textures/snap/grid toggles) while a brush tool
//! is active, mirroring Photoshop's tool options bar.

use bevy::prelude::*;
use bevy_egui::egui::{self, RichText};
use renzora_editor_framework::EditorCommands;
use egui_phosphor::regular::*;
use renzora_theme::ThemeManager;
use renzora_terrain::data::{BrushFalloffType, BrushShape, TerrainSettings};
use renzora_terrain::paint::SurfacePaintSettings;

pub fn draw_sculpt_options(ui: &mut egui::Ui, world: &World) {
    let Some(settings) = world.get_resource::<TerrainSettings>() else { return };
    let Some(cmds) = world.get_resource::<EditorCommands>() else { return };
    let muted = world.get_resource::<ThemeManager>()
        .map(|t| t.active_theme.text.muted.to_color32())
        .unwrap_or(egui::Color32::GRAY);

    ui.label(RichText::new(MOUNTAINS).size(14.0).color(muted));
    ui.label(RichText::new("Sculpt").size(12.0).color(muted));
    ui.separator();

    // Brush radius
    ui.label(RichText::new("Radius").size(11.0).color(muted));
    let mut radius = settings.brush_radius;
    if ui.add(egui::DragValue::new(&mut radius).range(0.5..=200.0).speed(0.2).max_decimals(1)).changed() {
        cmds.push(move |w: &mut World| { if let Some(mut s) = w.get_resource_mut::<TerrainSettings>() { s.brush_radius = radius; } });
    }

    // Brush strength
    ui.label(RichText::new("Strength").size(11.0).color(muted));
    let mut strength = settings.brush_strength;
    if ui.add(egui::DragValue::new(&mut strength).range(0.001..=1.0).speed(0.01).max_decimals(3)).changed() {
        cmds.push(move |w: &mut World| { if let Some(mut s) = w.get_resource_mut::<TerrainSettings>() { s.brush_strength = strength; } });
    }

    // Falloff
    ui.label(RichText::new("Falloff").size(11.0).color(muted));
    let mut falloff = settings.falloff;
    if ui.add(egui::DragValue::new(&mut falloff).range(0.0..=1.0).speed(0.01).max_decimals(2)).changed() {
        cmds.push(move |w: &mut World| { if let Some(mut s) = w.get_resource_mut::<TerrainSettings>() { s.falloff = falloff; } });
    }

    ui.separator();

    // Shape picker
    ui.label(RichText::new("Shape").size(11.0).color(muted));
    shape_picker(ui, settings.brush_shape, cmds, |s, v| {
        if let Some(mut t) = s.get_resource_mut::<TerrainSettings>() { t.brush_shape = v; }
    });

    // Falloff curve picker
    ui.label(RichText::new("Curve").size(11.0).color(muted));
    falloff_picker(ui, settings.falloff_type, cmds, |s, v| {
        if let Some(mut t) = s.get_resource_mut::<TerrainSettings>() { t.falloff_type = v; }
    });
}

pub fn draw_paint_options(ui: &mut egui::Ui, world: &World) {
    let Some(settings) = world.get_resource::<SurfacePaintSettings>() else { return };
    let Some(cmds) = world.get_resource::<EditorCommands>() else { return };
    let muted = world.get_resource::<ThemeManager>()
        .map(|t| t.active_theme.text.muted.to_color32())
        .unwrap_or(egui::Color32::GRAY);

    ui.label(RichText::new(PAINT_BRUSH).size(14.0).color(muted));
    ui.label(RichText::new("Paint").size(12.0).color(muted));
    ui.separator();

    ui.label(RichText::new("Layer").size(11.0).color(muted));
    let mut layer = settings.active_layer as i32;
    if ui.add(egui::DragValue::new(&mut layer).range(0..=7).speed(0.1)).changed() {
        let l = layer.max(0) as usize;
        cmds.push(move |w: &mut World| { if let Some(mut s) = w.get_resource_mut::<SurfacePaintSettings>() { s.active_layer = l; } });
    }

    ui.label(RichText::new("Radius").size(11.0).color(muted));
    let mut radius = settings.brush_radius;
    if ui.add(egui::DragValue::new(&mut radius).range(0.01..=2.0).speed(0.01).max_decimals(3)).changed() {
        cmds.push(move |w: &mut World| { if let Some(mut s) = w.get_resource_mut::<SurfacePaintSettings>() { s.brush_radius = radius; } });
    }

    ui.label(RichText::new("Strength").size(11.0).color(muted));
    let mut strength = settings.brush_strength;
    if ui.add(egui::DragValue::new(&mut strength).range(0.001..=1.0).speed(0.01).max_decimals(3)).changed() {
        cmds.push(move |w: &mut World| { if let Some(mut s) = w.get_resource_mut::<SurfacePaintSettings>() { s.brush_strength = strength; } });
    }

    ui.label(RichText::new("Falloff").size(11.0).color(muted));
    let mut fo = settings.brush_falloff;
    if ui.add(egui::DragValue::new(&mut fo).range(0.0..=1.0).speed(0.01).max_decimals(2)).changed() {
        cmds.push(move |w: &mut World| { if let Some(mut s) = w.get_resource_mut::<SurfacePaintSettings>() { s.brush_falloff = fo; } });
    }

    ui.separator();
    ui.label(RichText::new("Shape").size(11.0).color(muted));
    shape_picker(ui, settings.brush_shape, cmds, |s, v| {
        if let Some(mut t) = s.get_resource_mut::<SurfacePaintSettings>() { t.brush_shape = v; }
    });
}

fn pill_button(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    let accent = ui.visuals().selection.bg_fill;
    let inactive = ui.visuals().widgets.inactive.bg_fill;
    let fill = if selected { accent } else { inactive };
    let btn = egui::Button::new(RichText::new(label).size(11.0))
        .fill(fill)
        .min_size(egui::Vec2::new(0.0, 20.0));
    ui.add(btn)
}

fn shape_picker(
    ui: &mut egui::Ui, current: BrushShape, cmds: &EditorCommands,
    apply: fn(&mut World, BrushShape),
) {
    let shapes = [(BrushShape::Circle, "Circle"), (BrushShape::Square, "Square"), (BrushShape::Diamond, "Diamond")];
    for (sh, name) in shapes {
        if pill_button(ui, name, current == sh).clicked() {
            cmds.push(move |w: &mut World| apply(w, sh));
        }
    }
}

fn falloff_picker(
    ui: &mut egui::Ui, current: BrushFalloffType, cmds: &EditorCommands,
    apply: fn(&mut World, BrushFalloffType),
) {
    let items = [
        (BrushFalloffType::Smooth, "Smooth"),
        (BrushFalloffType::Linear, "Linear"),
        (BrushFalloffType::Spherical, "Spherical"),
        (BrushFalloffType::Tip, "Tip"),
        (BrushFalloffType::Flat, "Flat"),
    ];
    for (ft, name) in items {
        if pill_button(ui, name, current == ft).clicked() {
            cmds.push(move |w: &mut World| apply(w, ft));
        }
    }
}
