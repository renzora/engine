use crate::prelude::*;
use bevy::{
    color::{Color, ColorToComponents, ColorToPacked, Srgba},
    ecs::prelude::Component,
    log,
    math::Vec3,
    mesh::{Mesh, VertexAttributeValues},
    platform::collections::HashMap,
    reflect::Reflect,
};
use std::sync::Arc;

type PinnedPosCondition = dyn Fn(Vec3) -> bool + Send + Sync;

/// Builder component for cloth behaviour, defines every available option for
/// cloth generation and rendering.
///
/// Add this component to an entity with at least a `GlobalTransform` and a
/// `Handle<Mesh>`
///
/// ## Note
///
/// The associated `Mesh` must have its [`RenderAssetUsages`] set to
/// `MAIN_WORLD` for the cloth engine to access it
///
/// [`RenderAssetUsages`]: bevy::asset::RenderAssetUsages
#[derive(Clone, Reflect, Default, Component)]
#[must_use]
pub struct ClothBuilder {
    /// cloth vertex ids unaffected by physics and following the attached
    /// `GlobalTransform`.
    pub anchored_vertex_ids: HashMap<usize, VertexAnchor>,
    /// cloth vertex colors unaffected by physics and following the attached
    /// `GlobalTransform`.
    pub anchored_vertex_colors: HashMap<[u8; 4], VertexAnchor>,
    /// Optional condition to apply on vertex positions. If the condition
    /// returns `true` the vertex will be anchored, and therefore unaffected
    /// by physics and following the attached `GlobalTransform`
    #[reflect(ignore)]
    pub anchored_position_conditions: Vec<(Arc<PinnedPosCondition>, VertexAnchor)>,
    /// How cloth sticks get generated
    pub stick_generation: StickGeneration,
    /// Define cloth sticks target length
    pub stick_length: StickLen,
    /// Defines the cloth computation mode of vertex normals
    pub normals_computing: NormalComputing,
    /// Default behaviour for cloth sticks
    pub default_stick_mode: StickMode,
}

#[allow(clippy::missing_const_for_fn)]
impl ClothBuilder {
    /// Instantiates a new `ClothBuilder`
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds pinned points for the cloth
    ///
    /// # Arguments
    ///
    /// * `fixed_points` - Iterator on the vertex indexes that should be
    ///   attached to the associated `GlobalTransform`
    #[inline]
    #[doc(hidden)]
    #[deprecated(note = "Use `with_pinned_vertex_ids` instead")]
    pub fn with_fixed_points(mut self, fixed_points: impl Iterator<Item = usize>) -> Self {
        self.anchored_vertex_ids
            .extend(fixed_points.map(|id| (id, VertexAnchor::default())));
        self
    }

    /// Adds pinned vertex ids for the cloth. The vertices will be pinned to the
    /// associated `GlobalTransform`
    ///
    /// # Arguments
    ///
    /// * `pinned_ids` - Iterator on the vertex indexes that should be pinned to
    ///   the associated `GlobalTransform`
    #[inline]
    pub fn with_pinned_vertex_ids(mut self, pinned_ids: impl Iterator<Item = usize>) -> Self {
        self.anchored_vertex_ids
            .extend(pinned_ids.map(|id| (id, VertexAnchor::default())));
        self
    }

    /// Adds pinned a vertex id for the cloth. The vertex will be pinned to the
    /// associated `GlobalTransform`
    ///
    /// # Arguments
    ///
    /// * `pinned_id` - Vertex index that should be pinned to the associated
    ///   `GlobalTransform`
    #[inline]
    pub fn with_pinned_vertex_id(mut self, pinned_id: usize) -> Self {
        self.anchored_vertex_ids
            .insert(pinned_id, VertexAnchor::default());
        self
    }

    /// Adds custom anchored vertex ids for the cloth
    ///
    /// # Arguments
    ///
    /// * `vertex_ids` - Iterator on the vertex indexes that should be anchored
    /// * `vertex_anchor` - Vertex anchor definition
    #[inline]
    pub fn with_anchored_vertex_ids(
        mut self,
        vertex_ids: impl Iterator<Item = usize>,
        vertex_anchor: VertexAnchor,
    ) -> Self {
        self.anchored_vertex_ids
            .extend(vertex_ids.map(|id| (id, vertex_anchor)));
        self
    }

    /// Adds a custom anchored vertex id for the cloth
    ///
    /// # Arguments
    ///
    /// * `vertex_id` - vertex index that should be anchored
    /// * `vertex_anchor` - Vertex anchor definition
    #[inline]
    pub fn with_anchored_vertex_id(
        mut self,
        vertex_id: usize,
        vertex_anchor: VertexAnchor,
    ) -> Self {
        self.anchored_vertex_ids.insert(vertex_id, vertex_anchor);
        self
    }

    /// Adds pinned vertex colors for the cloth
    ///
    /// # Arguments
    ///
    /// * `vertex_colors` - Iterator on the vertex colors that should be pinned
    ///   to the associated `GlobalTransform`
    #[inline]
    pub fn with_pinned_vertex_colors(mut self, vertex_colors: impl Iterator<Item = Color>) -> Self {
        self.anchored_vertex_colors
            .extend(vertex_colors.map(|c| (c.to_srgba().to_u8_array(), VertexAnchor::default())));
        self
    }

    /// Adds a pinned vertex color for the cloth
    ///
    /// # Arguments
    ///
    /// * `vertex_color` - Vertex colors that should be pinned to the associated
    ///   `GlobalTransform`
    #[inline]
    pub fn with_pinned_vertex_color(mut self, vertex_color: Color) -> Self {
        self.anchored_vertex_colors.insert(
            vertex_color.to_srgba().to_u8_array(),
            VertexAnchor::default(),
        );
        self
    }

    /// Adds custom anchored vertex colors the cloth
    ///
    /// # Arguments
    ///
    /// * `vertex_colors` - Iterator on the vertex colors that should be
    ///   anchored
    /// * `vertex_anchor` - Vertex anchor definition
    #[inline]
    pub fn with_anchored_vertex_colors(
        mut self,
        vertex_colors: impl Iterator<Item = Color>,
        vertex_anchor: VertexAnchor,
    ) -> Self {
        self.anchored_vertex_colors
            .extend(vertex_colors.map(|c| (c.to_srgba().to_u8_array(), vertex_anchor)));
        self
    }

    /// Adds a custom anchored vertex color the cloth
    ///
    /// # Arguments
    ///
    /// * `vertex_color` - vertex color that should be anchored
    /// * `vertex_anchor` - Vertex anchor definition
    #[inline]
    pub fn with_anchored_vertex_color(
        mut self,
        vertex_color: Color,
        vertex_anchor: VertexAnchor,
    ) -> Self {
        self.anchored_vertex_colors
            .insert(vertex_color.to_srgba().to_u8_array(), vertex_anchor);
        self
    }

    /// Adds pinned vertex positions for the cloth
    ///
    /// # Arguments
    ///
    /// * `condition` - a function determining if a given position ([`Vec3`]) is
    ///   pinned to the associated `GlobalTransform`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use bevy_silk::prelude::*;
    ///
    /// let builder = ClothBuilder::new().with_pinned_vertex_positions(|pos| pos.x > 0.0);
    /// ```
    #[inline]
    pub fn with_pinned_vertex_positions(self, condition: fn(Vec3) -> bool) -> Self {
        self.with_anchored_vertex_positions(condition, Default::default())
    }

    /// Adds anchored vertex positions for the cloth
    ///
    /// # Arguments
    ///
    /// * `condition` - a function determining if a given position ([`Vec3`])
    ///   should be anchored
    /// * `vertex_anchor` - Vertex anchor definition
    ///
    /// # Example
    ///
    /// ```rust
    /// # use bevy_silk::prelude::*;
    ///
    /// let anchor = VertexAnchor::default();
    /// let builder = ClothBuilder::new().with_anchored_vertex_positions(|pos| pos.x > 0.0, anchor);
    /// ```
    #[inline]
    pub fn with_anchored_vertex_positions(
        mut self,
        condition: fn(Vec3) -> bool,
        vertex_anchor: VertexAnchor,
    ) -> Self {
        self.anchored_position_conditions
            .push((Arc::new(condition), vertex_anchor));
        self
    }

    /// Sets the stick generation option for the cloth
    ///
    /// # Arguments
    ///
    /// * `stick_generation` - Cloth sticks generation mode
    #[inline]
    pub fn with_stick_generation(mut self, stick_generation: StickGeneration) -> Self {
        self.stick_generation = stick_generation;
        self
    }

    /// Sets the default stick mode option for the cloth
    ///
    /// # Arguments
    ///
    /// * `stick_mode` - Default cloth sticks behavhiour
    #[inline]
    pub fn with_stick_mode(mut self, stick_mode: StickMode) -> Self {
        self.default_stick_mode = stick_mode;
        self
    }

    /// Sets the sticks target length option for the cloth
    ///
    /// # Arguments
    ///
    /// * `stick_len` - Cloth sticks target length option
    #[inline]
    pub fn with_stick_length(mut self, stick_len: StickLen) -> Self {
        self.stick_length = stick_len;
        self
    }

    /// The cloth won't re-compute the mesh normals. It's the fastest option but
    /// lighting will become inconsistent
    #[inline]
    pub fn without_normal_computation(mut self) -> Self {
        self.normals_computing = NormalComputing::None;
        self
    }

    /// The cloth will compute smooth vertex normals
    #[deprecated(note = "Use `with_smooth_normals` instead")]
    #[doc(hidden)]
    #[inline]
    pub fn with_smooth_normal_computation(mut self) -> Self {
        self.normals_computing = NormalComputing::SmoothNormals;
        self
    }

    /// The cloth will compute smooth vertex normals
    #[inline]
    pub fn with_smooth_normals(mut self) -> Self {
        self.normals_computing = NormalComputing::SmoothNormals;
        self
    }

    /// The cloth will compute flat vertex normals and duplicate shared vertices
    #[deprecated(note = "Use `with_flat_normals` instead")]
    #[doc(hidden)]
    #[inline]
    pub fn with_flat_normal_computation(mut self) -> Self {
        self.normals_computing = NormalComputing::FlatNormals;
        self
    }

    /// The cloth will compute flat vertex normals and duplicate shared vertices
    #[inline]
    pub fn with_flat_normals(mut self) -> Self {
        self.normals_computing = NormalComputing::FlatNormals;
        self
    }

    /// Retrieves all anchored vertex ids using:
    /// - [`Self::anchored_vertex_ids`] explicit ids
    /// - [`Self::anchored_vertex_colors`] to find every vertex id in `mesh`
    ///   matching a pinned color
    ///
    /// Note: anchored vertex colors are ignored if the given `mesh` doesn't
    /// have vertex colors
    #[must_use]
    pub fn anchored_vertex_ids(&self, mesh: &Mesh) -> HashMap<usize, VertexAnchor> {
        let mut res = self.anchored_vertex_ids.clone();
        if !self.anchored_vertex_colors.is_empty() {
            let vertex_colors: Option<Vec<[u8; 4]>> = mesh
                .attribute(Mesh::ATTRIBUTE_COLOR)
                .and_then(|attr| match attr {
                    VertexAttributeValues::Float32x3(v) => Some(
                        v.iter()
                            .copied()
                            .map(|c| Srgba::from_f32_array_no_alpha(c).to_u8_array())
                            .collect(),
                    ),
                    VertexAttributeValues::Float32x4(v) => Some(
                        v.iter()
                            .copied()
                            .map(|c| Srgba::from_f32_array(c).to_u8_array())
                            .collect(),
                    ),
                    VertexAttributeValues::Uint8x4(v) => Some(v.clone()),
                    _ => None,
                });
            #[allow(clippy::option_if_let_else)]
            match vertex_colors {
                Some(colors) => {
                    res.extend(colors.into_iter().enumerate().filter_map(|(i, color)| {
                        self.anchored_vertex_colors
                            .get(&color)
                            .map(|anchor| (i, *anchor))
                    }));
                }
                None => {
                    log::warn!(
                        "ClothBuilder has anchored vertex colors but the associated mesh doesn't \
                         have a valid Vertex_Color attribute"
                    );
                }
            };
        }
        if !self.anchored_position_conditions.is_empty() {
            let vertex_positions: Option<Vec<Vec3>> = mesh
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .and_then(|attr| match attr {
                    VertexAttributeValues::Float32x3(v) => {
                        Some(v.iter().copied().map(Vec3::from).collect())
                    }
                    _ => None,
                });
            #[allow(clippy::option_if_let_else)]
            match vertex_positions {
                Some(positions) => {
                    res.extend(positions.into_iter().enumerate().flat_map(|(i, pos)| {
                        self.anchored_position_conditions
                            .iter()
                            .filter_map(move |(c, anchor)| c(pos).then_some((i, *anchor)))
                    }));
                }
                None => {
                    log::warn!(
                        "ClothBuilder has anchored vertex positions but the associated mesh \
                         doesn't have a valid Vertex_Position attribute"
                    );
                }
            };
        }
        res
    }
}
