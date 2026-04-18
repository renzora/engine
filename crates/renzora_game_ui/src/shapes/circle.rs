//! Circle shape — SDF-based circle with optional stroke.

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use serde::{Deserialize, Serialize};

/// A circle shape rendered via SDF.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct CircleShape {
    pub color: Color,
    pub stroke_color: Color,
    /// Stroke width in pixels. 0.0 = no stroke.
    pub stroke_width: f32,
}

impl Default for CircleShape {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            stroke_color: Color::NONE,
            stroke_width: 0.0,
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct CircleMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(1)]
    pub stroke_color: LinearRgba,
    /// x = stroke_width (normalized 0..1 relative to radius)
    #[uniform(2)]
    pub params: Vec4,
}

impl CircleMaterial {
    pub fn from_shape(shape: &CircleShape) -> Self {
        Self {
            color: shape.color.to_linear(),
            stroke_color: shape.stroke_color.to_linear(),
            params: Vec4::new(shape.stroke_width, 0.0, 0.0, 0.0),
        }
    }
}

impl UiMaterial for CircleMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_game_ui/shapes/circle.wgsl".into()
    }
}

pub fn sync_circle_materials(
    query: Query<(&CircleShape, &MaterialNode<CircleMaterial>), Changed<CircleShape>>,
    mut materials: ResMut<Assets<CircleMaterial>>,
) {
    for (shape, mat_node) in &query {
        if let Some(mat) = materials.get_mut(mat_node.id()) {
            mat.color = shape.color.to_linear();
            mat.stroke_color = shape.stroke_color.to_linear();
            mat.params.x = shape.stroke_width;
        }
    }
}
