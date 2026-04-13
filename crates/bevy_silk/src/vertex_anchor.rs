use bevy::{
    math::Vec3,
    prelude::{Entity, GlobalTransform},
    reflect::Reflect,
};

/// Defines a cloth vertex anchor through a `target` and `offset`
///
/// The default anchor will link the cloth vertices to the cloth entity's
/// `GlobalTransform`, you can anchor them to a specific entity by defining a
/// `custom_target`.
#[derive(Debug, Copy, Clone, Default, Reflect)]
#[must_use]
pub struct VertexAnchor {
    /// Optional custom anchor target entity. If not set, the cloth entity will
    /// be used
    pub custom_target: Option<Entity>,
    /// optional custom offset. If not set the base vertex position will be used
    pub custom_offset: Option<Vec3>,
    /// If set to true, the base vertex position will be ignored.
    /// If [`Self::custom_offset`] is defined, it will then override the vertex
    /// position
    pub ignore_vertex_position: bool,
}

impl VertexAnchor {
    /// Retrieves the anchor world space position.
    ///
    /// # Arguments
    ///
    /// * `original_pos` - the original local space vertex position
    /// * `self_transform` - the `GlobalTransform` associated with the cloth
    ///   entity used without a custom target entity
    /// * `transform_query` - ECS query used in case of a set
    ///   [`Self::custom_target`]
    #[must_use]
    pub fn get_position<'a>(
        &self,
        original_pos: Vec3,
        self_transform: &GlobalTransform,
        query: &impl Fn(Entity) -> Option<&'a GlobalTransform>,
    ) -> Vec3 {
        let transform = self.custom_target.and_then(query).unwrap_or(self_transform);
        let local_pos = if self.ignore_vertex_position {
            Vec3::ZERO
        } else {
            original_pos
        } + self.custom_offset.unwrap_or(Vec3::ZERO);
        transform.transform_point(local_pos)
    }
}
