//! Module containing structs and functions relevant to Occluders.

use bevy::{
    camera::visibility::{RenderLayers, VisibilityClass, add_visibility_class},
    color::palettes::css::BLACK,
    math::bounding::{Aabb2d, BoundingVolume},
    prelude::*,
    reflect::std_traits::ReflectDefault,
    render::{render_resource::ShaderType, sync_world::SyncToRenderWorld},
};
use bytemuck::{NoUninit, Pod, Zeroable};
use core::f32;

use crate::visibility::{OccluderAabb, VisibilityTimer};
use crate::{buffers::BufferIndex, change::Changes};

/// An occluder that blocks light.
///
/// Can be semi-transparent, have a color, any polygonal shape
/// and a few other select shapes (capsule, circle, round_rectangle).
///
/// Can be moved around or rotated by their transform.
///
/// Only z-axis rotations are allowed, any other type of rotation can cause unexpected behavior and bugs.
#[derive(Debug, Component, Clone, Reflect, Default)]
// renzora: registered reflection data so the editor inspector and the
// reflection-driven scene serializer can insert/round-trip this component.
#[reflect(Component, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[require(
    SyncToRenderWorld,
    Occluder2dEnabled,
    Transform,
    VisibilityClass,
    ViewVisibility,
    VisibilityTimer,
    OccluderAabb,
    Changes,
    RenderLayers
)]
#[component(on_add = add_visibility_class::<Occluder2d>)]
pub struct Occluder2d {
    shape: Occluder2dShape,

    /// Color of the occluder. **Alpha is ignored**.
    pub color: Color,

    /// Opacity of the occluder.
    ///
    /// An occluder of opacity 0 won't block any light.
    /// An occluder of opacity 1 will completely both light (and cast a fully black shadow).
    ///
    /// Anything in-between will cast a colored shadow depending on how opaque it is.
    pub opacity: f32,

    /// If true, this occluder won't cast shadows over sprites with a higher z value.
    ///
    /// This does nothing if z_sorting is set to false in the [config](crate::prelude::FireflyConfig::z_sorting).
    pub z_sorting: bool,

    /// Offset to the position of the occluder.
    ///
    /// **Default**: [Vec3::ZERO].
    pub offset: Vec3,
}

#[derive(Debug, Component, Clone, Reflect)]
#[reflect(Component, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Occluder2dEnabled(pub bool);

impl Default for Occluder2dEnabled {
    fn default() -> Self {
        Self(true)
    }
}

impl Occluder2d {
    /// Get the occluder's **internal shape**.
    pub fn shape(&self) -> &Occluder2dShape {
        &self.shape
    }

    fn from_shape(shape: Occluder2dShape) -> Self {
        Self {
            shape,
            opacity: 1.,
            color: bevy::prelude::Color::Srgba(BLACK),
            z_sorting: true,
            offset: default(),
        }
    }

    /// Construct a new occluder with the specified [color](Occluder2d::color).
    pub fn with_color(&self, color: Color) -> Self {
        let mut res = self.clone();
        res.color = color;
        res
    }

    /// Construct a new occluder with the specified [opacity](Occluder2d::opacity).
    pub fn with_opacity(&self, opacity: f32) -> Self {
        let mut res = self.clone();
        res.opacity = opacity;
        res
    }

    /// Construct a new occluder with the specified [z-sorting](Occluder2d::z_sorting).
    pub fn with_z_sorting(&self, z_sorting: bool) -> Self {
        let mut res = self.clone();
        res.z_sorting = z_sorting;
        res
    }

    /// Construct a new occluder with the specified [offset](Occluder2d::offset).
    pub fn with_offset(&self, offset: Vec3) -> Self {
        let mut res = self.clone();
        res.offset = offset;
        res
    }

    /// Construct a polygonal occluder from the given points.
    ///
    /// The points can form a convex or concave polygon. However,
    /// having self-intersections can cause unexpected behavior.
    ///
    /// The points should be relative to the entity's translation.
    ///
    /// ## Vertex Ordering
    /// This method will perform an extra `O(N)` check to determine if
    /// the vertices are in clockwise or counter-clockwise order.
    ///
    /// If you wish to bypass this check, you can use the [`polygon_cc`](Occluder2d::polygon_cc)
    /// and [`polygon_ccw`](Occluder2d::polygon_ccw) methods.
    ///
    /// ## Failure
    /// This returns None if the provided list doesn't contain at least 2 vertices.
    pub fn polygon(vertices: impl Into<Vec<Vec2>>) -> Option<Self> {
        let vertices = vertices.into();

        if vertices.len() < 2 {
            return None;
        }

        Some(Self::from_shape(Occluder2dShape::Polygon {
            concave: is_concave(&vertices),
            vertices: normalize_vertices(vertices),
        }))
    }

    /// Construct a polygonal occluder from the given points.
    ///
    /// The points can form a convex or concave polygon. However,
    /// having self-intersections can cause unexpected behavior.
    ///
    /// The points should be relative to the entity's translation.
    ///
    /// ## Vertex Ordering
    /// Compared to [`polygon`](Occluder2d::polygon), this method assumed the vertices are in **clockwise** order.
    ///
    /// ## Failure
    /// This returns None if the provided list doesn't contain at least 2 vertices.
    pub fn polygon_cc(vertices: impl Into<Vec<Vec2>>) -> Option<Self> {
        let vertices = vertices.into();

        if vertices.len() < 2 {
            return None;
        }

        Some(Self::from_shape(Occluder2dShape::Polygon {
            concave: is_concave(&vertices),
            vertices,
        }))
    }

    /// Construct a polygonal occluder from the given points.
    ///
    /// The points can form a convex or concave polygon. However,
    /// having self-intersections can cause unexpected behavior.
    ///
    /// The points should be relative to the entity's translation.
    ///
    /// ## Vertex Ordering
    /// Compared to [`polygon`](Occluder2d::polygon), this method assumed the vertices are in **counter-clockwise** order.
    ///
    /// ## Failure
    /// This returns None if the provided list doesn't contain at least 2 vertices.
    pub fn polygon_ccw(vertices: impl Into<Vec<Vec2>>) -> Option<Self> {
        let mut vertices = vertices.into();

        if vertices.len() < 2 {
            return None;
        }
        vertices.reverse();

        Some(Self::from_shape(Occluder2dShape::Polygon {
            concave: is_concave(&vertices),
            vertices,
        }))
    }

    /// Construct a polyline occluder from the given points.
    ///
    /// Having self-intersections can cause unexpected behavior.
    ///
    /// The points should be relative to the entity's translation.
    ///
    /// # Failure
    /// This returns None if the provided list doesn't contain at least 2 vertices.
    pub fn polyline(vertices: impl Into<Vec<Vec2>>) -> Option<Self> {
        let mut vertices = vertices.into();

        if vertices.len() < 2 {
            return None;
        }

        let mut vertices_clone = vertices.clone();

        vertices_clone.reverse();
        vertices.extend_from_slice(&vertices_clone[1..vertices_clone.len() - 1]);
        Some(Self::from_shape(Occluder2dShape::Polyline { vertices }))
    }

    /// Construct a rectangle occluder from width and height.
    pub fn rectangle(width: f32, height: f32) -> Self {
        Self::round_rectangle(width, height, 0.)
    }

    /// Construct a round rectangle occluder from width, height and radius.
    ///
    /// The resulted occluder is esentially a rectangle with a radius-sized padding around it.
    ///  
    /// For instance, a circle is a round rectangle with no height or width, and a capsule
    /// is a round rectangle with only height or only width (and radius).
    pub fn round_rectangle(width: f32, height: f32, radius: f32) -> Self {
        Self::from_shape(Occluder2dShape::RoundRectangle {
            half_width: width * 0.5,
            half_height: height * 0.5,
            radius,
        })
    }

    /// Construct a circle occluder.
    pub fn circle(radius: f32) -> Self {
        Self::round_rectangle(0., 0., radius)
    }

    /// Construct a vertical capsule occluder.
    pub fn vertical_capsule(length: f32, radius: f32) -> Self {
        Self::round_rectangle(0., length, radius)
    }

    /// Construct a horizontal_capsule occluder.
    pub fn horizontal_capsule(length: f32, radius: f32) -> Self {
        Self::round_rectangle(length, 0., radius)
    }

    /// Construct a capsule occluder. This is vertical by default. For a horizontal capsule check [`Occluder2d::horizontal_capsule()`].
    pub fn capsule(length: f32, radius: f32) -> Self {
        Self::vertical_capsule(length, radius)
    }
}

/// Component with data extracted to the Render World from Occluders.
#[derive(Component, Clone)]
#[require(RoundOccluderIndex, PolyOccluderIndex)]
pub struct ExtractedOccluder {
    pub pos: Vec2,
    pub rot: f32,
    pub shape: Occluder2dShape,
    pub aabb: Aabb2d,
    pub z: f32,
    pub color: Color,
    pub opacity: f32,
    pub z_sorting: bool,
    pub changes: Changes,
    pub render_layers: RenderLayers,
}

impl PartialEq for ExtractedOccluder {
    fn eq(&self, other: &Self) -> bool {
        self.pos == other.pos && self.rot == other.rot && self.shape == other.shape
    }
}

impl ExtractedOccluder {
    /// Get the occluder's vertices. This will be an empty Vec if the occluder has no vertices.
    pub fn vertices(&self) -> Vec<Vec2> {
        self.shape.vertices(self.pos, Rot2::radians(self.rot))
    }
    /// Get an iterator of the occluder's vertices. This will panic if the occluder has no vertices.
    pub fn vertices_iter<'a>(&'a self) -> Box<dyn 'a + DoubleEndedIterator<Item = Vec2>> {
        self.shape
            .vertices_iter(self.pos, Rot2::radians(self.rot))
            .unwrap()
    }
}

/// Rotates vertices to be clockwise.
fn normalize_vertices(mut vertices: Vec<Vec2>) -> Vec<Vec2> {
    let mut sum = 0.0;

    for i in 0..vertices.len() {
        let j = (i + 1) % vertices.len();
        sum += (vertices[j].x - vertices[i].x) * (vertices[j].y + vertices[i].y);
    }

    if sum >= 0.0 {
        vertices
    } else {
        vertices.reverse();
        vertices
    }
}

fn is_concave(vertices: &Vec<Vec2>) -> bool {
    let n = vertices.len();
    let mut first_orientation = orientation(vertices[0], vertices[1 % n], vertices[2 % n]);

    for i in 1..n {
        let new_orientation = orientation(
            vertices[i % n],
            vertices[(i + 1) % n],
            vertices[(i + 2) % n],
        );

        if matches!(first_orientation, Orientation::Touch) {
            first_orientation = new_orientation;
        } else if first_orientation != new_orientation {
            return false;
        }
    }

    true
}

#[derive(PartialEq, Eq)]
enum Orientation {
    Touch,
    Left,
    Right,
}

fn orientation(a: Vec2, b: Vec2, p: Vec2) -> Orientation {
    let res = (b.x - a.x) * (p.y - a.y) - (p.x - a.x) * (b.y - a.y);
    if res < 0. {
        return Orientation::Right;
    }
    if res > 0. {
        return Orientation::Left;
    }
    Orientation::Touch
}

pub(crate) fn point_inside_poly(p: Vec2, poly: &Vec<Vec2>, aabb: Aabb2d, concave: bool) -> bool {
    if !aabb.contains(&Aabb2d { min: p, max: p }) {
        return false;
    }
    let n = poly.len();

    if !concave {
        let mut inside = false;

        for i in 0..n {
            let line = [poly[i % n], poly[(i + 1) % n]];

            if p.y > line[0].y.min(line[1].y)
                && p.y <= line[0].y.max(line[1].y)
                && p.x <= line[0].x.max(line[1].x)
            {
                let x_intersection = (p.y - line[0].y) * (line[1].x - line[0].x)
                    / (line[1].y - line[0].y)
                    + line[0].x;

                if line[0].x == line[1].x || p.x <= x_intersection {
                    inside = !inside;
                }
            }
        }
        inside
    } else {
        for i in 0..n {
            let ori = orientation(poly[i % n], poly[(i + 1) % n], p);
            if matches!(ori, Orientation::Left) {
                return false;
            }

            if matches!(ori, Orientation::Touch) {
                return true;
            }
        }

        true
    }
}

/// Plugin that adds general main-world behavior relating to occluders. This is mainly responsible for
/// change and visibility detection. It is added automatically by the [`FireflyPlugin`](crate::prelude::FireflyPlugin).   
pub struct OccluderPlugin;

impl Plugin for OccluderPlugin {
    fn build(&self, _app: &mut App) {}
}

/// Data that is transferred to the GPU to be read inside shaders.
#[repr(C)]
#[derive(ShaderType, Clone, Copy, Default, NoUninit)]
pub struct UniformOccluder {
    pub vertex_start: u32,
    pub n_vertices: u32,
    pub z: f32,
    pub opacity: f32,
    pub color: Vec4,
    pub z_sorting: u32,
    pub _pad1: [u32; 3],
}

/// Data that is transferred to the GPU to be read inside shaders.
#[repr(C)]
#[derive(ShaderType, Clone, Copy, Default, NoUninit)]
pub struct UniformRoundOccluder {
    pub pos: Vec2,
    pub rot: f32,
    pub half_width: f32,
    pub half_height: f32,
    pub radius: f32,
    pub z: f32,
    pub opacity: f32,
    pub color: Vec4,
    pub z_sorting: u32,
    pub _pad1: [u32; 3],
}

#[repr(C)]
#[derive(ShaderType, Clone, Copy, Zeroable, Pod, Default)]
pub(crate) struct UniformVertex {
    pub angle: f32,
    pub pos: Vec2,
}

/// The internal shape of an [`Occluder`](crate::prelude::Occluder2d). This is intended to be generated automatically through
/// the occluder's constructor methods and not added by hand.   
#[derive(Debug, Reflect, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Occluder2dShape {
    Polygon {
        vertices: Vec<Vec2>,
        concave: bool,
    },
    Polyline {
        vertices: Vec<Vec2>,
    },
    RoundRectangle {
        half_width: f32,
        half_height: f32,
        radius: f32,
    },
}

impl Default for Occluder2dShape {
    fn default() -> Self {
        Self::RoundRectangle {
            half_width: 5.,
            half_height: 5.,
            radius: 0.,
        }
    }
}

impl Occluder2dShape {
    pub(crate) fn n_vertices(&self) -> u32 {
        match &self {
            Self::Polygon { vertices, .. } => vertices.len() as u32,
            Self::Polyline { vertices } => vertices.len() as u32,
            Self::RoundRectangle { .. } => 0,
        }
    }

    pub(crate) fn vertices(&self, pos: Vec2, rot: Rot2) -> Vec<Vec2> {
        match &self {
            Self::Polygon { vertices, .. } => translate_vertices(vertices.to_vec(), pos, rot),
            Self::Polyline { vertices, .. } => translate_vertices(vertices.to_vec(), pos, rot),
            Self::RoundRectangle { .. } => default(),
        }
    }
    pub(crate) fn vertices_iter<'a>(
        &'a self,
        pos: Vec2,
        rot: Rot2,
    ) -> Option<Box<dyn 'a + DoubleEndedIterator<Item = Vec2>>> {
        match self {
            Self::Polygon { vertices, .. } => Some(translate_vertices_iter(
                Box::new(vertices.iter().copied()),
                pos,
                rot,
            )),
            Self::Polyline { vertices, .. } => Some(translate_vertices_iter(
                Box::new(vertices.iter().copied()),
                pos,
                rot,
            )),
            Self::RoundRectangle { .. } => None,
        }
    }

    pub(crate) fn is_concave(&self) -> bool {
        match self {
            Self::Polygon { concave, .. } => *concave,
            _ => false,
        }
    }
}

pub(crate) fn translate_vertices(vertices: Vec<Vec2>, pos: Vec2, rot: Rot2) -> Vec<Vec2> {
    vertices.iter().map(|v| rot * *v + pos).collect()
}

pub(crate) fn translate_vertices_iter<'a>(
    vertices: Box<dyn 'a + DoubleEndedIterator<Item = Vec2>>,
    pos: Vec2,
    rot: Rot2,
) -> Box<dyn 'a + DoubleEndedIterator<Item = Vec2>> {
    Box::new(vertices.map(move |v| rot * v + pos))
}

#[derive(Component, Clone, Copy, Default)]
pub struct RoundOccluderIndex(pub Option<BufferIndex>);

#[derive(Component, Clone, Copy, Default)]
pub struct PolyOccluderIndex {
    pub occluder: Option<BufferIndex>,
    pub vertices: Option<BufferIndex>,
}
