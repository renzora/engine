//! Line shape — renders a line segment across the node.

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use serde::{Deserialize, Serialize};

/// A line segment rendered via SDF.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct LineShape {
    pub color: Color,
    /// Thickness in pixels.
    pub thickness: f32,
    /// Angle in degrees (0 = horizontal, 90 = vertical).
    pub angle: f32,
}

impl Default for LineShape {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            thickness: 2.0,
            angle: 0.0,
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct LineMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    /// x = thickness (pixels), y = angle (radians)
    #[uniform(1)]
    pub params: Vec4,
}

impl LineMaterial {
    pub fn from_shape(shape: &LineShape) -> Self {
        Self {
            color: shape.color.to_linear(),
            params: Vec4::new(shape.thickness, shape.angle.to_radians(), 0.0, 0.0),
        }
    }
}

impl UiMaterial for LineMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_game_ui/shapes/line.wgsl".into()
    }
}

pub fn sync_line_materials(
    query: Query<(&LineShape, &MaterialNode<LineMaterial>), Changed<LineShape>>,
    mut materials: ResMut<Assets<LineMaterial>>,
) {
    for (shape, mat_node) in &query {
        if let Some(mat) = materials.get_mut(mat_node.id()) {
            mat.color = shape.color.to_linear();
            mat.params = Vec4::new(shape.thickness, shape.angle.to_radians(), 0.0, 0.0);
        }
    }
}
