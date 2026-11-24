//! Which components replicate.
//!
//! By type path rather than by Rust type, for two reasons. A Rust script is a
//! separate library that cannot reach `App` to register anything, but it can
//! reach this resource through `&mut World`. And the server sends a component
//! by naming its type path, so the path is the identity on the wire anyway.
//!
//! A component replicates when it is registered here, reflected with
//! `#[reflect(Component)]`, and present on a [`crate::Networked`] entity. The
//! server compares each one's serialized value every tick and sends only what
//! changed.

use bevy::prelude::*;

/// The component types every networked entity replicates, by type path.
#[derive(Resource, Debug, Clone)]
pub struct ReplicationRegistry {
    paths: Vec<String>,
}

impl Default for ReplicationRegistry {
    /// The engine's own: what an entity is called, who owns it, what it looks
    /// like and how it moves. Without these a client would receive a bare
    /// position with nothing to draw at it.
    fn default() -> Self {
        let mut r = Self { paths: Vec::new() };
        r.register::<Name>();
        r.register::<crate::NetworkOwner>();
        r.register::<crate::NetworkPlayer>();
        r.register::<crate::NetworkPrefab>();
        r.register::<crate::NetworkTransform>();
        r
    }
}

impl ReplicationRegistry {
    pub fn register<T: TypePath>(&mut self) -> &mut Self {
        self.register_path(T::type_path())
    }

    pub fn register_path(&mut self, path: impl Into<String>) -> &mut Self {
        let path = path.into();
        if !self.paths.contains(&path) {
            self.paths.push(path);
        }
        self
    }

    pub fn paths(&self) -> &[String] {
        &self.paths
    }

    pub fn contains(&self, path: &str) -> bool {
        self.paths.iter().any(|p| p == path)
    }
}

/// `app.replicate::<Health>()`, for a plugin. A script writes
/// `world.resource_mut::<ReplicationRegistry>().register::<Health>()` instead.
pub trait AppReplicateExt {
    fn replicate<T: TypePath>(&mut self) -> &mut Self;
}

impl AppReplicateExt for App {
    fn replicate<T: TypePath>(&mut self) -> &mut Self {
        self.world_mut()
            .get_resource_or_insert_with(ReplicationRegistry::default)
            .register::<T>();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_cover_what_a_client_needs_to_draw_an_avatar() {
        let r = ReplicationRegistry::default();
        assert!(r.contains(<Name as TypePath>::type_path()));
        assert!(r.contains("renzora_network::components::NetworkPrefab"));
    }

    #[test]
    fn registering_twice_keeps_one() {
        let mut r = ReplicationRegistry::default();
        let n = r.paths().len();
        r.register::<Name>();
        assert_eq!(r.paths().len(), n);
    }
}
