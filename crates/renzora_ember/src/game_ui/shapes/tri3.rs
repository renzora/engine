//! Arbitrary 3-point triangle, filled via an SDF from three node-local vertices.
//!
//! Unlike [`TriangleShape`](super::TriangleShape) (a fixed equilateral that fills
//! the node), this takes explicit points — used by the script draw canvas for
//! `g.triangle` and, fanned, `g.poly`. Not authored/serialized; the canvas inserts
//! it fresh each frame, so no reflection/rehydrate on scene load is needed.

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

/// A filled triangle with three explicit vertices, in node-local pixels.
#[derive(Component, Clone, Debug)]
pub struct Tri3Shape {
    pub color: Color,
    pub a: Vec2,
    pub b: Vec2,
    pub c: Vec2,
}

impl Default for Tri3Shape {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            a: Vec2::ZERO,
            b: Vec2::new(1.0, 0.0),
            c: Vec2::new(0.0, 1.0),
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct Tri3Material {
    #[uniform(0)]
    pub color: LinearRgba,
    /// a.xy, b.xy in node-local px.
    #[uniform(1)]
    pub pts_ab: Vec4,
    /// c.xy in node-local px (z, w unused).
    #[uniform(2)]
    pub pts_c: Vec4,
}

impl Tri3Material {
    pub fn from_shape(s: &Tri3Shape) -> Self {
        Self {
            color: s.color.to_linear(),
            pts_ab: Vec4::new(s.a.x, s.a.y, s.b.x, s.b.y),
            pts_c: Vec4::new(s.c.x, s.c.y, 0.0, 0.0),
        }
    }
}

impl UiMaterial for Tri3Material {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_ember/game_ui/shapes/tri3.wgsl".into()
    }
}

/// Attach the material the frame a `Tri3Shape` is first inserted (the canvas
/// spawner inserts only the shape + `Node`).
pub fn init_tri3_materials(
    mut commands: Commands,
    q: Query<(Entity, &Tri3Shape), (Added<Tri3Shape>, Without<MaterialNode<Tri3Material>>)>,
    mut materials: ResMut<Assets<Tri3Material>>,
) {
    for (entity, shape) in &q {
        let handle = materials.add(Tri3Material::from_shape(shape));
        commands.entity(entity).try_insert(MaterialNode(handle));
    }
}

/// Repaint when the vertices/colour change.
pub fn sync_tri3_materials(
    query: Query<(&Tri3Shape, &MaterialNode<Tri3Material>), Changed<Tri3Shape>>,
    mut materials: ResMut<Assets<Tri3Material>>,
) {
    for (shape, node) in &query {
        if let Some(mut mat) = materials.get_mut(node.id()) {
            *mat = Tri3Material::from_shape(shape);
        }
    }
}
