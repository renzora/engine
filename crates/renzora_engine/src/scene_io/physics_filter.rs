//! What a scene save leaves out of physics.
//!
//! Two things, neither of which this crate can name by type:
//!
//! - **A backend's own components.** Avian keeps its bodies as components
//!   (`avian3d::...`); they are rebuilt from `PhysicsBodyData` and
//!   `CollisionShapeData` on load, and a scene that carried them would load only
//!   where that backend is installed. Each backend declares its type-path
//!   prefixes (`PhysicsBackend::runtime_type_prefixes`), so this filter follows
//!   whatever backends the build has rather than a list of avian's types.
//! - **Generated physics data.** An entity marked `RuntimeOnlyPhysics` (a
//!   ragdoll bone) had its body and collider built at runtime; saved, they would
//!   come back as hand-placed overrides on every bone.

use std::collections::HashSet;

use bevy::prelude::*;
use bevy::reflect::TypePath;
use renzora::physics::{CollisionShapeData, PhysicsBackends, PhysicsBodyData, RuntimeOnlyPhysics};

/// Built once per save, before the scene is extracted.
pub(crate) struct PhysicsFilter {
    backends: Option<PhysicsBackends>,
    generated: HashSet<Entity>,
}

impl PhysicsFilter {
    pub(crate) fn new(world: &mut World) -> Self {
        let backends = world.get_resource::<PhysicsBackends>().cloned();
        let generated = world
            .query_filtered::<Entity, With<RuntimeOnlyPhysics>>()
            .iter(world)
            .collect();
        Self { backends, generated }
    }

    /// Whether `entity`'s component at `type_path` belongs in the saved scene.
    pub(crate) fn keep(&self, entity: Entity, type_path: &str) -> bool {
        if self.backends.as_ref().is_some_and(|b| b.is_runtime_type(type_path)) {
            return false;
        }
        if self.generated.contains(&entity)
            && (type_path == PhysicsBodyData::type_path() || type_path == CollisionShapeData::type_path())
        {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora::physics::{AppPhysicsBackendExt, CastBatch, Caps, PhysicsBackend};

    struct Fake;

    impl PhysicsBackend for Fake {
        fn name(&self) -> &str {
            "fake"
        }
        fn caps(&self) -> Caps {
            Caps::DIM_3D
        }
        fn cast(&self, _world: &World, _batch: &mut CastBatch) {}
        fn runtime_type_prefixes(&self) -> &[&'static str] {
            &["fake_engine::"]
        }
    }

    #[test]
    fn a_backends_components_and_generated_data_stay_out_of_saves() {
        let mut app = App::new();
        app.add_physics_backend(Fake);
        let authored = app.world_mut().spawn(PhysicsBodyData::default()).id();
        let bone = app.world_mut().spawn((PhysicsBodyData::default(), RuntimeOnlyPhysics)).id();
        let filter = PhysicsFilter::new(app.world_mut());

        assert!(!filter.keep(authored, "fake_engine::RigidBody"), "the backend's own type");
        assert!(filter.keep(authored, "bevy_transform::components::transform::Transform"));
        assert!(filter.keep(authored, PhysicsBodyData::type_path()), "authored data saves");
        assert!(!filter.keep(bone, PhysicsBodyData::type_path()), "generated data does not");
        assert!(!filter.keep(bone, CollisionShapeData::type_path()));
        assert!(filter.keep(bone, "bevy_ecs::name::Name"), "the rest of the bone still saves");
    }
}
