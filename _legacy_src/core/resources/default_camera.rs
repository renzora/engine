//! Default camera tracking resource

use bevy::prelude::*;

/// Resource that tracks which camera entity is the default game camera
#[derive(Resource, Default)]
pub struct DefaultCameraEntity {
    pub entity: Option<Entity>,
}
