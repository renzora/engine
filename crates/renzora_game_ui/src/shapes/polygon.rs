//! Regular polygon shape — SDF-based N-sided polygon.

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use serde::{Deserialize, Serialize};

/// A regular polygon with N sides.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct PolygonShape {
    pub color: Color,
    pub stroke_color: Color,
    pub stroke_width: f32,
    /// Number of sides (3–12).
    pub sides: u32,
    /// Rotation in degrees.
    pub rotation: f32,
}

impl Default for PolygonShape {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            stroke_color: Color::NONE,
            stroke_width: 0.0,
            sides: 6,
            rotation: 0.0,
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct PolygonMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(1)]
    pub stroke_color: LinearRgba,
    /// x = stroke_width (pixels), y = sides, z = rotation (radians)
    #[uniform(2)]
    pub params: Vec4,
}

impl PolygonMaterial {
    pub fn from_shape(shape: &PolygonShape) -> Self {
        Self {
            color: shape.color.to_linear(),
            stroke_color: shape.stroke_color.to_linear(),
            params: Vec4::new(
                shape.stroke_width,
                shape.sides as f32,
                shape.rotation.to_radians(),
                0.0,
            ),
        }
    }
}

impl UiMaterial for PolygonMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_game_ui/shapes/polygon.wgsl".into()
    }
}

pub fn sync_polygon_materials(
    query: Query<(&PolygonShape, &MaterialNode<PolygonMaterial>), Changed<PolygonShape>>,
    mut materials: ResMut<Assets<PolygonMaterial>>,
) {
    for (shape, mat_node) in &query {
        if let Some(mat) = materials.get_mut(mat_node.id()) {
            mat.color = shape.color.to_linear();
            mat.stroke_color = shape.stroke_color.to_linear();
            mat.params = Vec4::new(
                shape.stroke_width,
                shape.sides as f32,
                shape.rotation.to_radians(),
                0.0,
            );
        }
    }
}
