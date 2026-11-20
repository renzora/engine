//! Module containing logic for change detection.

use bevy::prelude::*;

use crate::{lights::PointLight2d, prelude::Occluder2d};

/// Component that stores whether an entity has changed or not.
#[derive(Component, Clone, Default)]
pub struct Changes(pub bool);

/// Plugin that handles change detection. Added automatically by [`FireflyPlugin`](crate::prelude::FireflyPlugin).
pub struct ChangePlugin;

impl Plugin for ChangePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, reset_changes);
        app.add_systems(Update, (changed_occluders, changed_lights));
    }
}

fn changed_occluders(
    mut occluders: Query<&mut Changes, Or<(Changed<GlobalTransform>, Changed<Occluder2d>)>>,
) {
    for mut changed in &mut occluders {
        changed.0 = true;
    }
}

fn changed_lights(
    mut lights: Query<&mut Changes, Or<(Changed<GlobalTransform>, Changed<PointLight2d>)>>,
) {
    for mut changed in &mut lights {
        changed.0 = true;
    }
}

fn reset_changes(mut entities: Query<&mut Changes>) {
    for mut changed in &mut entities {
        *changed = default();
    }
}
