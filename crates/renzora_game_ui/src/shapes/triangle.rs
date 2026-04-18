//! Triangle shape — equilateral triangle with optional stroke.

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use serde::{Deserialize, Serialize};

/// An equilateral triangle.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct TriangleShape {
    pub color: Color,
    pub stroke_color: Color,
    pub stroke_width: f32,
    /// Rotation in degrees (0 = pointing up).
    pub rotation: f32,
}

impl Default for TriangleShape {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            stroke_color: Color::NONE,
            stroke_width: 0.0,
            rotation: 0.0,
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct TriangleMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(1)]
    pub stroke_color: LinearRgba,
    /// x = stroke_width (pixels), y = rotation (radians)
    #[uniform(2)]
    pub params: Vec4,
}

impl TriangleMaterial {
    pub fn from_shape(shape: &TriangleShape) -> Self {
        Self {
            color: shape.color.to_linear(),
            stroke_color: shape.stroke_color.to_linear(),
            params: Vec4::new(shape.stroke_width, shape.rotation.to_radians(), 0.0, 0.0),
        }
    }
}

impl UiMaterial for TriangleMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_game_ui/shapes/triangle.wgsl".into()
    }
}

pub fn sync_triangle_materials(
    query: Query<(&TriangleShape, &MaterialNode<TriangleMaterial>), Changed<TriangleShape>>,
    mut materials: ResMut<Assets<TriangleMaterial>>,
) {
    for (shape, mat_node) in &query {
        if let Some(mat) = materials.get_mut(mat_node.id()) {
            mat.color = shape.color.to_linear();
            mat.stroke_color = shape.stroke_color.to_linear();
            mat.params = Vec4::new(shape.stroke_width, shape.rotation.to_radians(), 0.0, 0.0);
        }
    }
}
