//! Queueing structural changes. Mirrors `bevy::Commands`.
//!
//! Deferred, like Bevy's: everything queued here is applied after the system
//! finishes, because spawning mid-iteration would invalidate the rows being
//! walked. The id from `spawn_empty` is nonetheless usable immediately — the
//! host reserves it up front and materialises the entity when commands apply.
//!
//! Every payload that crosses must be plain-old-data. The sink copies `data` as
//! bytes, so a pointer inside it would survive the copy as a pointer and be read
//! after the system returned, aimed at a stack frame that is gone.

use core::marker::PhantomData;

use crate::sys;

use super::component::{Component, Transform};
use super::init::component_id_of;
use super::system::push_service;

/// A group of components to insert together. Mirrors `bevy::Bundle`.
///
/// Implemented for any single component (emitted by `#[derive(Component)]`) and
/// for tuples of them, so `spawn((A, B, C))` reads exactly as it does in Bevy.
///
/// There is no blanket `impl<T: Component> Bundle for T` because it would
/// overlap with the tuple impls — Rust cannot prove a tuple is not a component.
/// The derive emitting it per-type sidesteps that.
pub trait Bundle {
    fn write(self, e: &mut EntityCommands);
}

/// A scene, ready to spawn. Produced by the [`bsn!`](crate::bsn) macro.
///
/// Named to match `bevy_scene::Scene`, and spawned the same way — through
/// [`Commands::spawn_scene`], not `spawn`. Bevy keeps them separate because a
/// scene is not a bundle: it describes a whole tree, and `spawn` takes the
/// components of one entity.
///
/// The source names components rather than carrying their bytes, which is why
/// one syntax reaches both the engine's components and the plugin's own: the
/// host resolves a name against the type registry first and the plugin schema
/// registry second, and the author never has to know which side a component is
/// on.
#[derive(Clone, Copy)]
pub struct Scene(pub &'static str);

impl Bundle for Scene {
    fn write(self, e: &mut EntityCommands) {
        if e.sink.is_null() {
            return;
        }
        let cmd = sys::Command {
            kind: sys::CommandKind::SpawnBsn,
            entity: e.id,
            component: sys::ComponentId::INVALID,
            data: self.0.as_ptr(),
            data_len: self.0.len(),
        };
        // SAFETY: the sink copies the bytes before returning, and the source is
        // `'static` regardless.
        unsafe { ((*e.sink).push)(e.sink, &cmd) };
    }
}

macro_rules! bundle_tuples {
    ($(($($p:ident),+))+) => {
        $(
            #[allow(non_snake_case)]
            impl<$($p: Bundle),+> Bundle for ($($p,)+) {
                fn write(self, e: &mut EntityCommands) {
                    let ($($p,)+) = self;
                    $($p.write(e);)+
                }
            }
        )+
    };
}

bundle_tuples! {
    (A)
    (A, B)
    (A, B, C)
    (A, B, C, D)
    (A, B, C, D, E)
    (A, B, C, D, E, F)
    (A, B, C, D, E, F, G)
    (A, B, C, D, E, F, G, H)
    (A, B, C, D, E, F, G, H, I)
    (A, B, C, D, E, F, G, H, I, J)
    (A, B, C, D, E, F, G, H, I, J, K)
    (A, B, C, D, E, F, G, H, I, J, K, L)
    (A, B, C, D, E, F, G, H, I, J, K, L, M)
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N)
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O)
}

/// Queue structural changes. Mirrors `bevy::Commands`.
///
/// Deferred, like Bevy's: everything queued here is applied after the system
/// finishes. Spawning mid-iteration would invalidate the rows being walked.
pub struct Commands<'a> {
    pub(crate) sink: *mut sys::CommandSink,
    pub(crate) _p: PhantomData<&'a ()>,
}

impl<'a> Commands<'a> {
    /// Spawn an entity carrying `bundle`. Mirrors `bevy::Commands::spawn`.
    pub fn spawn<B: Bundle>(&mut self, bundle: B) -> EntityCommands<'_> {
        let mut e = self.spawn_empty();
        bundle.write(&mut e);
        e
    }

    /// Spawn a visible entity: mesh, material and transform in one go.
    ///
    /// Returns [`EntityCommands`] so plugin components can be attached to it in
    /// the same breath.
    pub fn spawn_mesh(
        &mut self,
        mesh: sys::AssetHandle,
        material: sys::AssetHandle,
        transform: Transform,
    ) -> EntityCommands<'_> {
        let mut e = self.spawn_empty();
        e.make_renderable(mesh, material, transform);
        e
    }

    /// Spawn a scene. Mirrors `bevy::Commands::spawn_scene`.
    ///
    /// ```ignore
    /// commands.spawn_scene(bsn! {
    ///     #Light
    ///     Transform { translation: Vec3(0.0, 6.0, 0.0) }
    ///     PointLight { intensity: 400000.0 }
    ///     Children [
    ///         Marker,
    ///     ]
    /// });
    /// ```
    ///
    /// The returned id is the scene's root. It is valid immediately, even though
    /// the tree materialises when commands are applied — the same reservation
    /// `spawn_empty` uses.
    pub fn spawn_scene(&mut self, scene: Scene) -> EntityCommands<'_> {
        self.spawn(scene)
    }

    /// Reserve an entity with nothing on it. The id is usable immediately — for
    /// parenting, for storing in a component — even though the entity itself
    /// appears when commands are applied.
    pub fn spawn_empty(&mut self) -> EntityCommands<'_> {
        let id = if self.sink.is_null() {
            sys::Entity(u64::MAX)
        } else {
            unsafe { ((*self.sink).reserve_entity)(self.sink) }
        };
        EntityCommands {
            id,
            sink: self.sink,
            _p: PhantomData,
        }
    }

    /// Queue changes for an existing entity.
    pub fn entity(&mut self, entity: sys::Entity) -> EntityCommands<'_> {
        EntityCommands {
            id: entity,
            sink: self.sink,
            _p: PhantomData,
        }
    }
}

/// Queued changes for one entity. Mirrors `bevy::EntityCommands`.
pub struct EntityCommands<'a> {
    pub(crate) id: sys::Entity,
    pub(crate) sink: *mut sys::CommandSink,
    /// Ties this to its `Commands`, so it cannot outlive the sink it writes to.
    pub(crate) _p: PhantomData<&'a ()>,
}

impl<'a> EntityCommands<'a> {
    pub fn id(&self) -> sys::Entity {
        self.id
    }

    /// Insert a bundle. Mirrors `bevy::EntityCommands::insert`.
    pub fn insert<B: Bundle>(&mut self, bundle: B) -> &mut Self {
        bundle.write(self);
        self
    }

    /// Queue one component, copying its bytes.
    ///
    /// Takes the value by move and forgets it: the host now owns those bytes and
    /// will run the component's destructor if it has one. Dropping here too
    /// would free twice.
    pub fn insert_one<T: Component>(&mut self, value: T) -> &mut Self {
        if self.sink.is_null() {
            return self;
        }
        let bytes = &value as *const T as *const u8;
        let cmd = sys::Command {
            kind: sys::CommandKind::Insert,
            entity: self.id,
            component: sys::ComponentId::INVALID,
            data: bytes,
            data_len: core::mem::size_of::<T>(),
        };
        // The host resolves the component from the descriptor rather than an id,
        // because a plugin has no way to know the id it was assigned without
        // asking — and asking mid-system is exactly what the sink exists to
        // avoid.
        let mut cmd = cmd;
        cmd.component = component_id_of::<T>();
        unsafe { ((*self.sink).push)(self.sink, &cmd) };
        core::mem::forget(value);
        self
    }

    /// Attach mesh, material and transform. Used by [`Commands::spawn_mesh`].
    pub fn make_renderable(
        &mut self,
        mesh: sys::AssetHandle,
        material: sys::AssetHandle,
        transform: Transform,
    ) -> &mut Self {
        if self.sink.is_null() {
            return self;
        }
        let desc = sys::SpawnMeshDesc {
            mesh,
            material,
            transform,
        };
        let cmd = sys::Command {
            kind: sys::CommandKind::SpawnMesh,
            entity: self.id,
            component: sys::ComponentId::INVALID,
            data: (&desc as *const sys::SpawnMeshDesc).cast(),
            data_len: core::mem::size_of::<sys::SpawnMeshDesc>(),
        };
        unsafe { ((*self.sink).push)(self.sink, &cmd) };
        self
    }

    /// Replace this entity's material, keeping its mesh and transform.
    ///
    /// The counterpart to [`Self::make_renderable`], and the one to reach for
    /// when the geometry is not yours: an imported model, a shape the user
    /// authored, anything already in the scene.
    ///
    /// ```ignore
    /// fn shade(q: Query<Entity, (With<Glow>, With<Mesh3d>)>, mut commands: Commands) {
    ///     for e in &q {
    ///         commands.entity(e).set_material(handle);
    ///     }
    /// }
    /// ```
    ///
    /// Filter on [`Mesh3d`](super::Mesh3d) as above unless you know the entity
    /// has one. A material on an entity with no mesh is not an error and draws
    /// nothing — it just sits there, which is a confusing thing to debug.
    pub fn set_material(&mut self, material: sys::AssetHandle) -> &mut Self {
        if self.sink.is_null() {
            return self;
        }
        // Only `material` is read. Sharing the struct with `make_renderable`
        // keeps the two commands visibly the same shape, and the unread fields
        // cost six words on a path that runs once per entity, not per frame.
        let desc = sys::SpawnMeshDesc {
            mesh: sys::AssetHandle::INVALID,
            material,
            transform: Transform::default(),
        };
        let cmd = sys::Command {
            kind: sys::CommandKind::SetMaterial,
            entity: self.id,
            component: sys::ComponentId::INVALID,
            data: (&desc as *const sys::SpawnMeshDesc).cast(),
            data_len: core::mem::size_of::<sys::SpawnMeshDesc>(),
        };
        unsafe { ((*self.sink).push)(self.sink, &cmd) };
        self
    }

    pub fn remove<T: Component>(&mut self) -> &mut Self {
        if self.sink.is_null() {
            return self;
        }
        let cmd = sys::Command {
            kind: sys::CommandKind::Remove,
            entity: self.id,
            component: component_id_of::<T>(),
            data: core::ptr::null(),
            data_len: 0,
        };
        unsafe { ((*self.sink).push)(self.sink, &cmd) };
        self
    }

    /// Despawn this entity and its descendants.
    pub fn despawn(&mut self) {
        if self.sink.is_null() {
            return;
        }
        let cmd = sys::Command {
            kind: sys::CommandKind::Despawn,
            entity: self.id,
            component: sys::ComponentId::INVALID,
            data: core::ptr::null(),
            data_len: 0,
        };
        unsafe { ((*self.sink).push)(self.sink, &cmd) };
    }

    /// Call a host service — animation, audio, physics — with an opaque payload.
    ///
    /// The mechanism knows no service by name. `service` comes from
    /// [`sys::service_id`], `op` is that service's own numbering, and `payload`
    /// is whatever layout it and the draining engine crate agreed on. A service
    /// nothing drains is not an error: the call is discarded at end of frame.
    ///
    /// You normally do not call this directly — a domain crate wraps it in named
    /// methods (`renzora_anim` gives you `play_animation`). It is public because
    /// a domain crate is an ordinary dependency with no privileged access, which
    /// is the whole point: adding one changes nothing here.
    ///
    /// The payload must be plain-old-data. The sink copies `data` as bytes, so a
    /// pointer inside it would survive the copy as a pointer and be read after
    /// this system returned, pointing at a stack frame that is gone.
    pub fn call_service(&mut self, service: u64, op: u32, payload: &[u8]) -> &mut Self {
        if self.sink.is_null() {
            return self;
        }
        push_service(self.sink, self.id, service, op, payload);
        self
    }
}
