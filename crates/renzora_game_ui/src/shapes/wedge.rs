//! Wedge / Pie slice shape — a sector of a circle.

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use serde::{Deserialize, Serialize};

/// A pie-slice / wedge sector.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct WedgeShape {
    pub color: Color,
    /// Start angle in degrees (0 = top, clockwise).
    pub start_angle: f32,
    /// End angle in degrees.
    pub end_angle: f32,
    /// Inner radius as fraction (0.0 = full pie, 0.5 = donut slice).
    pub inner_radius: f32,
}

impl Default for WedgeShape {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            start_angle: 0.0,
            end_angle: 90.0,
            inner_radius: 0.0,
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct WedgeMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    /// x = start_angle (radians), y = end_angle (radians), z = inner_radius
    #[uniform(1)]
    pub params: Vec4,
}

impl WedgeMaterial {
    pub fn from_shape(shape: &WedgeShape) -> Self {
        Self {
            color: shape.color.to_linear(),
            params: Vec4::new(
                shape.start_angle.to_radians(),
                shape.end_angle.to_radians(),
                shape.inner_radius,
                0.0,
            ),
        }
    }
}

impl UiMaterial for WedgeMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_game_ui/shapes/wedge.wgsl".into()
    }
}

pub fn sync_wedge_materials(
    query: Query<(&WedgeShape, &MaterialNode<WedgeMaterial>), Changed<WedgeShape>>,
    mut materials: ResMut<Assets<WedgeMaterial>>,
) {
    for (shape, mat_node) in &query {
        if let Some(mat) = materials.get_mut(mat_node.id()) {
            mat.color = shape.color.to_linear();
            mat.params = Vec4::new(
                shape.start_angle.to_radians(),
                shape.end_angle.to_radians(),
                shape.inner_radius,
                0.0,
            );
        }
    }
}
