//! Arc / Ring shape — SDF-based arc segment with configurable angles.

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use serde::{Deserialize, Serialize};

/// An arc (partial ring) shape.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct ArcShape {
    pub color: Color,
    /// Start angle in degrees (0 = right, counter-clockwise).
    pub start_angle: f32,
    /// End angle in degrees.
    pub end_angle: f32,
    /// Ring thickness as fraction of radius (0.0–1.0). 1.0 = full disc.
    pub thickness: f32,
}

impl Default for ArcShape {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            start_angle: 0.0,
            end_angle: 270.0,
            thickness: 0.2,
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct ArcMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    /// x = start_angle (radians), y = end_angle (radians), z = thickness, w = unused
    #[uniform(1)]
    pub params: Vec4,
}

impl ArcMaterial {
    pub fn from_shape(shape: &ArcShape) -> Self {
        Self {
            color: shape.color.to_linear(),
            params: Vec4::new(
                shape.start_angle.to_radians(),
                shape.end_angle.to_radians(),
                shape.thickness,
                0.0,
            ),
        }
    }
}

impl UiMaterial for ArcMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_game_ui/shapes/arc.wgsl".into()
    }
}

pub fn sync_arc_materials(
    query: Query<(&ArcShape, &MaterialNode<ArcMaterial>), Changed<ArcShape>>,
    mut materials: ResMut<Assets<ArcMaterial>>,
) {
    for (shape, mat_node) in &query {
        if let Some(mat) = materials.get_mut(mat_node.id()) {
            mat.color = shape.color.to_linear();
            mat.params = Vec4::new(
                shape.start_angle.to_radians(),
                shape.end_angle.to_radians(),
                shape.thickness,
                0.0,
            );
        }
    }
}
