//! Radial progress — circular progress indicator (cooldowns, loading rings).

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use serde::{Deserialize, Serialize};

/// A circular progress indicator that fills clockwise from the top.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct RadialProgressShape {
    /// Progress value (0.0 = empty, 1.0 = full).
    pub value: f32,
    /// Fill color for the completed portion.
    pub color: Color,
    /// Background color for the uncompleted portion.
    pub bg_color: Color,
    /// Ring thickness as fraction of radius (0.0–1.0). 1.0 = full disc.
    pub thickness: f32,
}

impl Default for RadialProgressShape {
    fn default() -> Self {
        Self {
            value: 0.5,
            color: Color::srgba(0.3, 0.7, 1.0, 1.0),
            bg_color: Color::srgba(0.2, 0.2, 0.2, 0.5),
            thickness: 0.2,
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct RadialProgressMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(1)]
    pub bg_color: LinearRgba,
    /// x = value (0-1), y = thickness (0-1)
    #[uniform(2)]
    pub params: Vec4,
}

impl RadialProgressMaterial {
    pub fn from_shape(shape: &RadialProgressShape) -> Self {
        Self {
            color: shape.color.to_linear(),
            bg_color: shape.bg_color.to_linear(),
            params: Vec4::new(shape.value, shape.thickness, 0.0, 0.0),
        }
    }
}

impl UiMaterial for RadialProgressMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_game_ui/shapes/radial_progress.wgsl".into()
    }
}

pub fn sync_radial_progress_materials(
    query: Query<
        (&RadialProgressShape, &MaterialNode<RadialProgressMaterial>),
        Changed<RadialProgressShape>,
    >,
    mut materials: ResMut<Assets<RadialProgressMaterial>>,
) {
    for (shape, mat_node) in &query {
        if let Some(mat) = materials.get_mut(mat_node.id()) {
            mat.color = shape.color.to_linear();
            mat.bg_color = shape.bg_color.to_linear();
            mat.params = Vec4::new(shape.value, shape.thickness, 0.0, 0.0);
        }
    }
}
