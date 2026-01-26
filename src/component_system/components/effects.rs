//! Visual effects component definitions

use bevy::prelude::*;
use bevy_egui::egui;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::component_system::{ComponentCategory, ComponentDefinition, ComponentRegistry};

use egui_phosphor::regular::{
    SPARKLE, PATH, USER_FOCUS, SELECTION_BACKGROUND, PAINT_BRUSH, DROP,
};

// ============================================================================
// Billboard Component - Always faces camera
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct BillboardData {
    pub lock_y_axis: bool,
    pub offset: [f32; 3],
}

impl Default for BillboardData {
    fn default() -> Self {
        Self {
            lock_y_axis: true,
            offset: [0.0, 0.0, 0.0],
        }
    }
}

pub static BILLBOARD: ComponentDefinition = ComponentDefinition {
    type_id: "billboard",
    display_name: "Billboard",
    category: ComponentCategory::Rendering,
    icon: USER_FOCUS,
    priority: 10,
    add_fn: add_billboard,
    remove_fn: remove_billboard,
    has_fn: has_billboard,
    serialize_fn: serialize_billboard,
    deserialize_fn: deserialize_billboard,
    inspector_fn: inspect_billboard,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Particle System Component
// ============================================================================

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum ParticleEmitShape {
    #[default]
    Point,
    Sphere,
    Box,
    Cone,
    Circle,
}

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct ParticleSystemData {
    pub emit_rate: f32,
    pub max_particles: u32,
    pub lifetime: f32,
    pub lifetime_variance: f32,
    pub start_speed: f32,
    pub speed_variance: f32,
    pub start_size: f32,
    pub end_size: f32,
    pub start_color: [f32; 4],
    pub end_color: [f32; 4],
    pub gravity_modifier: f32,
    pub emit_shape: ParticleEmitShape,
    pub shape_radius: f32,
    pub loop_: bool,
    pub play_on_start: bool,
}

impl Default for ParticleSystemData {
    fn default() -> Self {
        Self {
            emit_rate: 10.0,
            max_particles: 100,
            lifetime: 2.0,
            lifetime_variance: 0.5,
            start_speed: 5.0,
            speed_variance: 1.0,
            start_size: 0.5,
            end_size: 0.1,
            start_color: [1.0, 1.0, 1.0, 1.0],
            end_color: [1.0, 1.0, 1.0, 0.0],
            gravity_modifier: 0.0,
            emit_shape: ParticleEmitShape::Point,
            shape_radius: 1.0,
            loop_: true,
            play_on_start: true,
        }
    }
}

pub static PARTICLE_SYSTEM: ComponentDefinition = ComponentDefinition {
    type_id: "particle_system",
    display_name: "Particle System",
    category: ComponentCategory::Effects,
    icon: SPARKLE,
    priority: 0,
    add_fn: add_particle_system,
    remove_fn: remove_particle_system,
    has_fn: has_particle_system,
    serialize_fn: serialize_particle_system,
    deserialize_fn: deserialize_particle_system,
    inspector_fn: inspect_particle_system,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Trail Renderer Component
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct TrailRendererData {
    pub time: f32,
    pub start_width: f32,
    pub end_width: f32,
    pub start_color: [f32; 4],
    pub end_color: [f32; 4],
    pub min_vertex_distance: f32,
}

impl Default for TrailRendererData {
    fn default() -> Self {
        Self {
            time: 1.0,
            start_width: 0.5,
            end_width: 0.0,
            start_color: [1.0, 1.0, 1.0, 1.0],
            end_color: [1.0, 1.0, 1.0, 0.0],
            min_vertex_distance: 0.1,
        }
    }
}

pub static TRAIL_RENDERER: ComponentDefinition = ComponentDefinition {
    type_id: "trail_renderer",
    display_name: "Trail Renderer",
    category: ComponentCategory::Effects,
    icon: PATH,
    priority: 1,
    add_fn: add_trail_renderer,
    remove_fn: remove_trail_renderer,
    has_fn: has_trail_renderer,
    serialize_fn: serialize_trail_renderer,
    deserialize_fn: deserialize_trail_renderer,
    inspector_fn: inspect_trail_renderer,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Line Renderer Component
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct LineRendererData {
    pub points: Vec<[f32; 3]>,
    pub width: f32,
    pub color: [f32; 4],
    pub loop_: bool,
    pub use_world_space: bool,
}

impl Default for LineRendererData {
    fn default() -> Self {
        Self {
            points: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
            width: 0.1,
            color: [1.0, 1.0, 1.0, 1.0],
            loop_: false,
            use_world_space: false,
        }
    }
}

pub static LINE_RENDERER: ComponentDefinition = ComponentDefinition {
    type_id: "line_renderer",
    display_name: "Line Renderer",
    category: ComponentCategory::Effects,
    icon: SELECTION_BACKGROUND,
    priority: 2,
    add_fn: add_line_renderer,
    remove_fn: remove_line_renderer,
    has_fn: has_line_renderer,
    serialize_fn: serialize_line_renderer,
    deserialize_fn: deserialize_line_renderer,
    inspector_fn: inspect_line_renderer,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Decal Component
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct DecalData {
    pub texture_path: String,
    pub size: [f32; 2],
    pub depth: f32,
    pub angle: f32,
    pub color: [f32; 4],
    pub fade_distance: f32,
}

impl Default for DecalData {
    fn default() -> Self {
        Self {
            texture_path: String::new(),
            size: [1.0, 1.0],
            depth: 0.5,
            angle: 0.0,
            color: [1.0, 1.0, 1.0, 1.0],
            fade_distance: 10.0,
        }
    }
}

pub static DECAL: ComponentDefinition = ComponentDefinition {
    type_id: "decal",
    display_name: "Decal",
    category: ComponentCategory::Effects,
    icon: PAINT_BRUSH,
    priority: 3,
    add_fn: add_decal,
    remove_fn: remove_decal,
    has_fn: has_decal,
    serialize_fn: serialize_decal,
    deserialize_fn: deserialize_decal,
    inspector_fn: inspect_decal,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Fog Volume Component
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct FogVolumeData {
    pub density: f32,
    pub color: [f32; 3],
    pub height_falloff: f32,
    pub size: [f32; 3],
    pub blend_distance: f32,
}

impl Default for FogVolumeData {
    fn default() -> Self {
        Self {
            density: 0.1,
            color: [0.8, 0.85, 0.9],
            height_falloff: 0.5,
            size: [10.0, 10.0, 10.0],
            blend_distance: 2.0,
        }
    }
}

pub static FOG_VOLUME: ComponentDefinition = ComponentDefinition {
    type_id: "fog_volume",
    display_name: "Fog Volume",
    category: ComponentCategory::Effects,
    icon: DROP,
    priority: 4,
    add_fn: add_fog_volume,
    remove_fn: remove_fog_volume,
    has_fn: has_fog_volume,
    serialize_fn: serialize_fog_volume,
    deserialize_fn: deserialize_fog_volume,
    inspector_fn: inspect_fog_volume,
    conflicts_with: &[],
    requires: &[],
};

/// Register all effects components
pub fn register(registry: &mut ComponentRegistry) {
    registry.register(&BILLBOARD);
    registry.register(&PARTICLE_SYSTEM);
    registry.register(&TRAIL_RENDERER);
    registry.register(&LINE_RENDERER);
    registry.register(&DECAL);
    registry.register(&FOG_VOLUME);
}

// ============================================================================
// Billboard Implementation
// ============================================================================

fn add_billboard(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(BillboardData::default());
}

fn remove_billboard(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<BillboardData>();
}

fn has_billboard(world: &World, entity: Entity) -> bool {
    world.get::<BillboardData>(entity).is_some()
}

fn serialize_billboard(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<BillboardData>(entity)?;
    Some(json!({
        "lock_y_axis": data.lock_y_axis,
        "offset": data.offset,
    }))
}

fn deserialize_billboard(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let billboard_data = BillboardData {
        lock_y_axis: data.get("lock_y_axis").and_then(|v| v.as_bool()).unwrap_or(true),
        offset: data.get("offset").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([0.0, 0.0, 0.0]),
    };
    entity_commands.insert(billboard_data);
}

fn inspect_billboard(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<BillboardData>(entity) {
        if ui.checkbox(&mut data.lock_y_axis, "Lock Y Axis").changed() {
            changed = true;
        }
        ui.label("Offset:");
        ui.horizontal(|ui| {
            ui.label("X:");
            if ui.add(egui::DragValue::new(&mut data.offset[0]).speed(0.1)).changed() { changed = true; }
            ui.label("Y:");
            if ui.add(egui::DragValue::new(&mut data.offset[1]).speed(0.1)).changed() { changed = true; }
            ui.label("Z:");
            if ui.add(egui::DragValue::new(&mut data.offset[2]).speed(0.1)).changed() { changed = true; }
        });
    }
    changed
}

// ============================================================================
// Particle System Implementation
// ============================================================================

fn add_particle_system(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(ParticleSystemData::default());
}

fn remove_particle_system(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<ParticleSystemData>();
}

fn has_particle_system(world: &World, entity: Entity) -> bool {
    world.get::<ParticleSystemData>(entity).is_some()
}

fn serialize_particle_system(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<ParticleSystemData>(entity)?;
    Some(json!({
        "emit_rate": data.emit_rate,
        "max_particles": data.max_particles,
        "lifetime": data.lifetime,
        "lifetime_variance": data.lifetime_variance,
        "start_speed": data.start_speed,
        "speed_variance": data.speed_variance,
        "start_size": data.start_size,
        "end_size": data.end_size,
        "start_color": data.start_color,
        "end_color": data.end_color,
        "gravity_modifier": data.gravity_modifier,
        "emit_shape": data.emit_shape,
        "shape_radius": data.shape_radius,
        "loop": data.loop_,
        "play_on_start": data.play_on_start,
    }))
}

fn deserialize_particle_system(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let particle_data = ParticleSystemData {
        emit_rate: data.get("emit_rate").and_then(|v| v.as_f64()).unwrap_or(10.0) as f32,
        max_particles: data.get("max_particles").and_then(|v| v.as_u64()).unwrap_or(100) as u32,
        lifetime: data.get("lifetime").and_then(|v| v.as_f64()).unwrap_or(2.0) as f32,
        lifetime_variance: data.get("lifetime_variance").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32,
        start_speed: data.get("start_speed").and_then(|v| v.as_f64()).unwrap_or(5.0) as f32,
        speed_variance: data.get("speed_variance").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        start_size: data.get("start_size").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32,
        end_size: data.get("end_size").and_then(|v| v.as_f64()).unwrap_or(0.1) as f32,
        start_color: data.get("start_color").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([1.0, 1.0, 1.0, 1.0]),
        end_color: data.get("end_color").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([1.0, 1.0, 1.0, 0.0]),
        gravity_modifier: data.get("gravity_modifier").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        emit_shape: data.get("emit_shape").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
        shape_radius: data.get("shape_radius").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        loop_: data.get("loop").and_then(|v| v.as_bool()).unwrap_or(true),
        play_on_start: data.get("play_on_start").and_then(|v| v.as_bool()).unwrap_or(true),
    };
    entity_commands.insert(particle_data);
}

fn inspect_particle_system(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<ParticleSystemData>(entity) {
        ui.collapsing("Emission", |ui| {
            ui.horizontal(|ui| {
                ui.label("Rate:");
                if ui.add(egui::DragValue::new(&mut data.emit_rate).speed(0.5).range(0.1..=1000.0)).changed() { changed = true; }
            });
            ui.horizontal(|ui| {
                ui.label("Max Particles:");
                if ui.add(egui::DragValue::new(&mut data.max_particles).speed(1.0).range(1..=10000)).changed() { changed = true; }
            });

            ui.horizontal(|ui| {
                ui.label("Shape:");
                egui::ComboBox::from_id_salt("emit_shape")
                    .selected_text(match data.emit_shape {
                        ParticleEmitShape::Point => "Point",
                        ParticleEmitShape::Sphere => "Sphere",
                        ParticleEmitShape::Box => "Box",
                        ParticleEmitShape::Cone => "Cone",
                        ParticleEmitShape::Circle => "Circle",
                    })
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(data.emit_shape == ParticleEmitShape::Point, "Point").clicked() { data.emit_shape = ParticleEmitShape::Point; changed = true; }
                        if ui.selectable_label(data.emit_shape == ParticleEmitShape::Sphere, "Sphere").clicked() { data.emit_shape = ParticleEmitShape::Sphere; changed = true; }
                        if ui.selectable_label(data.emit_shape == ParticleEmitShape::Box, "Box").clicked() { data.emit_shape = ParticleEmitShape::Box; changed = true; }
                        if ui.selectable_label(data.emit_shape == ParticleEmitShape::Cone, "Cone").clicked() { data.emit_shape = ParticleEmitShape::Cone; changed = true; }
                        if ui.selectable_label(data.emit_shape == ParticleEmitShape::Circle, "Circle").clicked() { data.emit_shape = ParticleEmitShape::Circle; changed = true; }
                    });
            });

            if data.emit_shape != ParticleEmitShape::Point {
                ui.horizontal(|ui| {
                    ui.label("Radius:");
                    if ui.add(egui::DragValue::new(&mut data.shape_radius).speed(0.1).range(0.1..=100.0)).changed() { changed = true; }
                });
            }
        });

        ui.collapsing("Lifetime", |ui| {
            ui.horizontal(|ui| {
                ui.label("Lifetime:");
                if ui.add(egui::DragValue::new(&mut data.lifetime).speed(0.1).range(0.1..=60.0)).changed() { changed = true; }
            });
            ui.horizontal(|ui| {
                ui.label("Variance:");
                if ui.add(egui::DragValue::new(&mut data.lifetime_variance).speed(0.05).range(0.0..=30.0)).changed() { changed = true; }
            });
        });

        ui.collapsing("Velocity", |ui| {
            ui.horizontal(|ui| {
                ui.label("Start Speed:");
                if ui.add(egui::DragValue::new(&mut data.start_speed).speed(0.1).range(0.0..=100.0)).changed() { changed = true; }
            });
            ui.horizontal(|ui| {
                ui.label("Variance:");
                if ui.add(egui::DragValue::new(&mut data.speed_variance).speed(0.1).range(0.0..=50.0)).changed() { changed = true; }
            });
            ui.horizontal(|ui| {
                ui.label("Gravity:");
                if ui.add(egui::DragValue::new(&mut data.gravity_modifier).speed(0.1).range(-10.0..=10.0)).changed() { changed = true; }
            });
        });

        ui.collapsing("Size", |ui| {
            ui.horizontal(|ui| {
                ui.label("Start:");
                if ui.add(egui::DragValue::new(&mut data.start_size).speed(0.01).range(0.01..=10.0)).changed() { changed = true; }
            });
            ui.horizontal(|ui| {
                ui.label("End:");
                if ui.add(egui::DragValue::new(&mut data.end_size).speed(0.01).range(0.0..=10.0)).changed() { changed = true; }
            });
        });

        ui.collapsing("Color", |ui| {
            let mut start_rgba = egui::Rgba::from_rgba_premultiplied(data.start_color[0], data.start_color[1], data.start_color[2], data.start_color[3]);
            ui.horizontal(|ui| {
                ui.label("Start:");
                if egui::color_picker::color_edit_button_rgba(ui, &mut start_rgba, egui::color_picker::Alpha::OnlyBlend).changed() {
                    data.start_color = [start_rgba.r(), start_rgba.g(), start_rgba.b(), start_rgba.a()];
                    changed = true;
                }
            });

            let mut end_rgba = egui::Rgba::from_rgba_premultiplied(data.end_color[0], data.end_color[1], data.end_color[2], data.end_color[3]);
            ui.horizontal(|ui| {
                ui.label("End:");
                if egui::color_picker::color_edit_button_rgba(ui, &mut end_rgba, egui::color_picker::Alpha::OnlyBlend).changed() {
                    data.end_color = [end_rgba.r(), end_rgba.g(), end_rgba.b(), end_rgba.a()];
                    changed = true;
                }
            });
        });

        ui.separator();
        if ui.checkbox(&mut data.loop_, "Loop").changed() { changed = true; }
        if ui.checkbox(&mut data.play_on_start, "Play on Start").changed() { changed = true; }
    }
    changed
}

// ============================================================================
// Trail Renderer Implementation
// ============================================================================

fn add_trail_renderer(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(TrailRendererData::default());
}

fn remove_trail_renderer(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<TrailRendererData>();
}

fn has_trail_renderer(world: &World, entity: Entity) -> bool {
    world.get::<TrailRendererData>(entity).is_some()
}

fn serialize_trail_renderer(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<TrailRendererData>(entity)?;
    Some(json!({
        "time": data.time,
        "start_width": data.start_width,
        "end_width": data.end_width,
        "start_color": data.start_color,
        "end_color": data.end_color,
        "min_vertex_distance": data.min_vertex_distance,
    }))
}

fn deserialize_trail_renderer(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let trail_data = TrailRendererData {
        time: data.get("time").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        start_width: data.get("start_width").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32,
        end_width: data.get("end_width").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        start_color: data.get("start_color").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([1.0, 1.0, 1.0, 1.0]),
        end_color: data.get("end_color").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([1.0, 1.0, 1.0, 0.0]),
        min_vertex_distance: data.get("min_vertex_distance").and_then(|v| v.as_f64()).unwrap_or(0.1) as f32,
    };
    entity_commands.insert(trail_data);
}

fn inspect_trail_renderer(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<TrailRendererData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Trail Time:");
            if ui.add(egui::DragValue::new(&mut data.time).speed(0.1).range(0.1..=10.0)).changed() { changed = true; }
        });
        ui.horizontal(|ui| {
            ui.label("Start Width:");
            if ui.add(egui::DragValue::new(&mut data.start_width).speed(0.01).range(0.01..=5.0)).changed() { changed = true; }
        });
        ui.horizontal(|ui| {
            ui.label("End Width:");
            if ui.add(egui::DragValue::new(&mut data.end_width).speed(0.01).range(0.0..=5.0)).changed() { changed = true; }
        });
        ui.horizontal(|ui| {
            ui.label("Min Distance:");
            if ui.add(egui::DragValue::new(&mut data.min_vertex_distance).speed(0.01).range(0.01..=2.0)).changed() { changed = true; }
        });

        let mut start_rgba = egui::Rgba::from_rgba_premultiplied(data.start_color[0], data.start_color[1], data.start_color[2], data.start_color[3]);
        ui.horizontal(|ui| {
            ui.label("Start Color:");
            if egui::color_picker::color_edit_button_rgba(ui, &mut start_rgba, egui::color_picker::Alpha::OnlyBlend).changed() {
                data.start_color = [start_rgba.r(), start_rgba.g(), start_rgba.b(), start_rgba.a()];
                changed = true;
            }
        });

        let mut end_rgba = egui::Rgba::from_rgba_premultiplied(data.end_color[0], data.end_color[1], data.end_color[2], data.end_color[3]);
        ui.horizontal(|ui| {
            ui.label("End Color:");
            if egui::color_picker::color_edit_button_rgba(ui, &mut end_rgba, egui::color_picker::Alpha::OnlyBlend).changed() {
                data.end_color = [end_rgba.r(), end_rgba.g(), end_rgba.b(), end_rgba.a()];
                changed = true;
            }
        });
    }
    changed
}

// ============================================================================
// Line Renderer Implementation
// ============================================================================

fn add_line_renderer(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(LineRendererData::default());
}

fn remove_line_renderer(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<LineRendererData>();
}

fn has_line_renderer(world: &World, entity: Entity) -> bool {
    world.get::<LineRendererData>(entity).is_some()
}

fn serialize_line_renderer(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<LineRendererData>(entity)?;
    Some(json!({
        "points": data.points,
        "width": data.width,
        "color": data.color,
        "loop": data.loop_,
        "use_world_space": data.use_world_space,
    }))
}

fn deserialize_line_renderer(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let line_data = LineRendererData {
        points: data.get("points").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_else(|| vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]]),
        width: data.get("width").and_then(|v| v.as_f64()).unwrap_or(0.1) as f32,
        color: data.get("color").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([1.0, 1.0, 1.0, 1.0]),
        loop_: data.get("loop").and_then(|v| v.as_bool()).unwrap_or(false),
        use_world_space: data.get("use_world_space").and_then(|v| v.as_bool()).unwrap_or(false),
    };
    entity_commands.insert(line_data);
}

fn inspect_line_renderer(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<LineRendererData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Width:");
            if ui.add(egui::DragValue::new(&mut data.width).speed(0.01).range(0.01..=2.0)).changed() { changed = true; }
        });

        let mut rgba = egui::Rgba::from_rgba_premultiplied(data.color[0], data.color[1], data.color[2], data.color[3]);
        ui.horizontal(|ui| {
            ui.label("Color:");
            if egui::color_picker::color_edit_button_rgba(ui, &mut rgba, egui::color_picker::Alpha::OnlyBlend).changed() {
                data.color = [rgba.r(), rgba.g(), rgba.b(), rgba.a()];
                changed = true;
            }
        });

        if ui.checkbox(&mut data.loop_, "Loop").changed() { changed = true; }
        if ui.checkbox(&mut data.use_world_space, "World Space").changed() { changed = true; }

        ui.separator();
        ui.label(format!("Points: {}", data.points.len()));
    }
    changed
}

// ============================================================================
// Decal Implementation
// ============================================================================

fn add_decal(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(DecalData::default());
}

fn remove_decal(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<DecalData>();
}

fn has_decal(world: &World, entity: Entity) -> bool {
    world.get::<DecalData>(entity).is_some()
}

fn serialize_decal(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<DecalData>(entity)?;
    Some(json!({
        "texture_path": data.texture_path,
        "size": data.size,
        "depth": data.depth,
        "angle": data.angle,
        "color": data.color,
        "fade_distance": data.fade_distance,
    }))
}

fn deserialize_decal(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let decal_data = DecalData {
        texture_path: data.get("texture_path").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        size: data.get("size").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([1.0, 1.0]),
        depth: data.get("depth").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32,
        angle: data.get("angle").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        color: data.get("color").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([1.0, 1.0, 1.0, 1.0]),
        fade_distance: data.get("fade_distance").and_then(|v| v.as_f64()).unwrap_or(10.0) as f32,
    };
    entity_commands.insert(decal_data);
}

fn inspect_decal(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<DecalData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Texture:");
            if ui.text_edit_singleline(&mut data.texture_path).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Size:");
            if ui.add(egui::DragValue::new(&mut data.size[0]).speed(0.1).prefix("W: ")).changed() { changed = true; }
            if ui.add(egui::DragValue::new(&mut data.size[1]).speed(0.1).prefix("H: ")).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Depth:");
            if ui.add(egui::DragValue::new(&mut data.depth).speed(0.1).range(0.1..=10.0)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Angle:");
            if ui.add(egui::DragValue::new(&mut data.angle).speed(1.0).range(0.0..=360.0).suffix("°")).changed() { changed = true; }
        });

        let mut rgba = egui::Rgba::from_rgba_premultiplied(data.color[0], data.color[1], data.color[2], data.color[3]);
        ui.horizontal(|ui| {
            ui.label("Color:");
            if egui::color_picker::color_edit_button_rgba(ui, &mut rgba, egui::color_picker::Alpha::OnlyBlend).changed() {
                data.color = [rgba.r(), rgba.g(), rgba.b(), rgba.a()];
                changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Fade Distance:");
            if ui.add(egui::DragValue::new(&mut data.fade_distance).speed(0.5).range(1.0..=100.0)).changed() { changed = true; }
        });
    }
    changed
}

// ============================================================================
// Fog Volume Implementation
// ============================================================================

fn add_fog_volume(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(FogVolumeData::default());
}

fn remove_fog_volume(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<FogVolumeData>();
}

fn has_fog_volume(world: &World, entity: Entity) -> bool {
    world.get::<FogVolumeData>(entity).is_some()
}

fn serialize_fog_volume(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<FogVolumeData>(entity)?;
    Some(json!({
        "density": data.density,
        "color": data.color,
        "height_falloff": data.height_falloff,
        "size": data.size,
        "blend_distance": data.blend_distance,
    }))
}

fn deserialize_fog_volume(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let fog_data = FogVolumeData {
        density: data.get("density").and_then(|v| v.as_f64()).unwrap_or(0.1) as f32,
        color: data.get("color").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([0.8, 0.85, 0.9]),
        height_falloff: data.get("height_falloff").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32,
        size: data.get("size").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([10.0, 10.0, 10.0]),
        blend_distance: data.get("blend_distance").and_then(|v| v.as_f64()).unwrap_or(2.0) as f32,
    };
    entity_commands.insert(fog_data);
}

fn inspect_fog_volume(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<FogVolumeData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Density:");
            if ui.add(egui::Slider::new(&mut data.density, 0.0..=1.0)).changed() { changed = true; }
        });

        let mut rgb = [data.color[0], data.color[1], data.color[2]];
        ui.horizontal(|ui| {
            ui.label("Color:");
            if ui.color_edit_button_rgb(&mut rgb).changed() {
                data.color = rgb;
                changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Height Falloff:");
            if ui.add(egui::DragValue::new(&mut data.height_falloff).speed(0.05).range(0.0..=2.0)).changed() { changed = true; }
        });

        ui.label("Size:");
        ui.horizontal(|ui| {
            if ui.add(egui::DragValue::new(&mut data.size[0]).speed(0.5).prefix("X: ")).changed() { changed = true; }
            if ui.add(egui::DragValue::new(&mut data.size[1]).speed(0.5).prefix("Y: ")).changed() { changed = true; }
            if ui.add(egui::DragValue::new(&mut data.size[2]).speed(0.5).prefix("Z: ")).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Blend Distance:");
            if ui.add(egui::DragValue::new(&mut data.blend_distance).speed(0.1).range(0.1..=10.0)).changed() { changed = true; }
        });
    }
    changed
}
