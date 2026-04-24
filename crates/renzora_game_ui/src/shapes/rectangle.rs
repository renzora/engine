//! Rectangle shape — SDF-based rounded rectangle with optional stroke.

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use serde::{Deserialize, Serialize};

/// A rectangle with optional rounded corners and stroke.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct RectangleShape {
    pub color: Color,
    pub stroke_color: Color,
    /// Stroke width in pixels. 0.0 = no stroke.
    pub stroke_width: f32,
    /// Corner radii in pixels, clockwise from top-left: [TL, TR, BR, BL].
    pub corner_radius: [f32; 4],
}

impl Default for RectangleShape {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            stroke_color: Color::NONE,
            stroke_width: 0.0,
            corner_radius: [0.0; 4],
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct RectangleMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(1)]
    pub stroke_color: LinearRgba,
    /// x = stroke_width (pixels)
    #[uniform(2)]
    pub params: Vec4,
    /// Corner radii. Shader expects (BR, TR, BL, TL) per IQ's convention;
    /// we remap from the component's [TL, TR, BR, BL] order in `from_shape`.
    #[uniform(3)]
    pub corners: Vec4,
}

impl RectangleMaterial {
    pub fn from_shape(shape: &RectangleShape) -> Self {
        let [tl, tr, br, bl] = shape.corner_radius;
        Self {
            color: shape.color.to_linear(),
            stroke_color: shape.stroke_color.to_linear(),
            params: Vec4::new(shape.stroke_width, 0.0, 0.0, 0.0),
            // Shader order: (BR, TR, BL, TL)
            corners: Vec4::new(br, tr, bl, tl),
        }
    }
}

impl UiMaterial for RectangleMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_game_ui/shapes/rectangle.wgsl".into()
    }
}

pub fn sync_rectangle_materials(
    query: Query<(&RectangleShape, &MaterialNode<RectangleMaterial>), Changed<RectangleShape>>,
    mut materials: ResMut<Assets<RectangleMaterial>>,
) {
    for (shape, mat_node) in &query {
        if let Some(mat) = materials.get_mut(mat_node.id()) {
            let new_mat = RectangleMaterial::from_shape(shape);
            mat.color = new_mat.color;
            mat.stroke_color = new_mat.stroke_color;
            mat.params = new_mat.params;
            mat.corners = new_mat.corners;
        }
    }
}
