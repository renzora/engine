// Rapier backend — placeholder until bevy_rapier3d supports Bevy 0.18.
//
// To use: enable the `rapier` feature and disable `avian`.
// Once bevy_rapier3d releases Bevy 0.18 support, implement the functions below
// mirroring the avian backend.

#[cfg(not(feature = "rapier"))]
compile_error!("The rapier backend requires the `rapier` feature");

use bevy::prelude::*;

use crate::data::*;
use crate::properties::*;

pub struct RapierBackendPlugin {
    pub start_paused: bool,
}

impl Plugin for RapierBackendPlugin {
    fn build(&self, _app: &mut App) {
        info!("[runtime] RapierBackendPlugin");
        // TODO: Add bevy_rapier3d plugins
        // app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default());
        // app.add_plugins(RapierDebugRenderPlugin::default());
        unimplemented!("Rapier backend not yet implemented — waiting for bevy_rapier3d Bevy 0.18 support");
    }
}

pub fn spawn_physics_body(_commands: &mut Commands, _entity: Entity, _body_data: &PhysicsBodyData) {
    unimplemented!("Rapier spawn_physics_body")
}

pub fn spawn_collision_shape(_commands: &mut Commands, _entity: Entity, _shape_data: &CollisionShapeData) {
    unimplemented!("Rapier spawn_collision_shape")
}

pub fn despawn_physics_components(_commands: &mut Commands, _entity: Entity) {
    unimplemented!("Rapier despawn_physics_components")
}

pub fn physics_pause(_: &mut bevy::prelude::ResMut<bevy::prelude::Time>) {
    unimplemented!("Rapier physics_pause")
}

pub fn physics_unpause(_: &mut bevy::prelude::ResMut<bevy::prelude::Time>) {
    unimplemented!("Rapier physics_unpause")
}

pub fn physics_is_paused(_: &bevy::prelude::Res<bevy::prelude::Time>) -> bool {
    unimplemented!("Rapier physics_is_paused")
}
