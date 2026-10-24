//! Reading and writing a skeleton's pose between animation and propagation.
//!
//! The constraints run after Bevy's `AnimationSystems` and before
//! `TransformSystems::Propagate`, so every `GlobalTransform` is last frame's.
//! World poses are therefore composed here from the local `Transform`s up the
//! hierarchy, and every write updates the local `Transform` so a later
//! constraint in the same frame composes the already-constrained pose.
//!
//! ## Why every constrained bone carries a [`PoseCache`]
//!
//! A bone the animation player drives gets a fresh pose every frame, so a
//! constraint always starts from the animation. A bone *nothing* animates
//! keeps whatever was last written, which would be the constraint's own output,
//! and solving again from there turns a 0.5 weight into a slow creep to 1 and
//! a spring bone into a chain that springs toward its own last frame. The cache
//! tells the two apart: if the `Transform` still holds exactly what the
//! constraints wrote last frame, nothing has posed it since, and the input it
//! was solved from is put back before solving again.

use bevy::prelude::*;

/// The pose a bone had before the constraints ran (`input`) and the one they
/// left it in (`output`), last frame.
#[derive(Component, Clone, Copy, Debug)]
pub struct PoseCache {
    input: Transform,
    output: Transform,
}

impl PoseCache {
    pub fn new(current: Transform) -> Self {
        Self { input: current, output: current }
    }
}

/// First in the chain: give unanimated bones back their input pose, and take
/// the fresh input of animated ones.
pub fn restore_inputs(mut bones: Query<(&mut Transform, &mut PoseCache)>) {
    for (mut transform, mut cache) in &mut bones {
        if *transform == cache.output {
            if *transform != cache.input {
                *transform = cache.input;
            }
        } else {
            cache.input = *transform;
        }
    }
}

/// Last in the chain: remember what the constraints left.
pub fn record_outputs(mut bones: Query<(&Transform, &mut PoseCache)>) {
    for (transform, mut cache) in &mut bones {
        cache.output = *transform;
    }
}

/// When the game stops (the editor leaving Play), hand every unanimated bone
/// its input pose back and forget the caches, so what is left in the scene is
/// what the animation or the author put there, not a solved pose.
pub fn release_on_stop(mut commands: Commands, mut bones: Query<(Entity, &mut Transform, &PoseCache)>) {
    for (entity, mut transform, cache) in &mut bones {
        if *transform == cache.output {
            *transform = cache.input;
        }
        commands.entity(entity).remove::<PoseCache>();
    }
}

/// Make sure `entity` is cached from next frame on. Its first constrained
/// frame goes uncached, which is harmless: it starts from the current pose.
pub fn track(commands: &mut Commands, cached: &Query<(), With<PoseCache>>, entity: Entity, current: Transform) {
    if cached.get(entity).is_err() {
        commands.entity(entity).try_insert(PoseCache::new(current));
    }
}

/// `entity`'s world transform, composed from local transforms up to its root.
/// `local` reads one entity's `Transform`.
pub fn world_of(entity: Entity, local: &impl Fn(Entity) -> Option<Transform>, parents: &Query<&ChildOf>) -> Option<Transform> {
    let mut world = local(entity)?;
    let mut current = entity;
    for _ in 0..1024 {
        let Ok(parent) = parents.get(current) else {
            break;
        };
        let parent = parent.parent();
        let Some(parent_local) = local(parent) else {
            break;
        };
        world = parent_local.mul_transform(world);
        current = parent;
    }
    Some(world)
}

/// The world transform of `entity`'s parent, identity for a root.
pub fn parent_world(entity: Entity, local: &impl Fn(Entity) -> Option<Transform>, parents: &Query<&ChildOf>) -> Transform {
    parents
        .get(entity)
        .ok()
        .and_then(|p| world_of(p.parent(), local, parents))
        .unwrap_or(Transform::IDENTITY)
}

/// A world-space view over a mutable transform query.
///
/// Generic over every lifetime of both queries because each system parameter
/// gets its own, and `&mut Query` (like `Query` in its data type) is
/// invariant, so none of them could be unified with another.
pub struct Pose<'a, 'w, 's, 'd, 'pw, 'ps, 'pd> {
    pub transforms: &'a mut Query<'w, 's, &'d mut Transform>,
    pub parents: &'a Query<'pw, 'ps, &'pd ChildOf>,
}

impl Pose<'_, '_, '_, '_, '_, '_, '_> {
    pub fn local(&self, entity: Entity) -> Option<Transform> {
        self.transforms.get(entity).ok().copied()
    }

    pub fn world(&self, entity: Entity) -> Option<Transform> {
        let read = |e: Entity| self.transforms.get(e).ok().copied();
        world_of(entity, &read, self.parents)
    }

    pub fn parent_world(&self, entity: Entity) -> Transform {
        let read = |e: Entity| self.transforms.get(e).ok().copied();
        parent_world(entity, &read, self.parents)
    }

    pub fn parent(&self, entity: Entity) -> Option<Entity> {
        self.parents.get(entity).ok().map(|p| p.parent())
    }

    /// Give `entity` the world rotation `rotation`, keeping its position.
    pub fn set_world_rotation(&mut self, entity: Entity, rotation: Quat) {
        let parent = self.parent_world(entity);
        if let Ok(mut t) = self.transforms.get_mut(entity) {
            t.rotation = (parent.rotation.inverse() * rotation).normalize();
        }
    }

    /// Move `entity` to the world position `position`.
    pub fn set_world_translation(&mut self, entity: Entity, position: Vec3) {
        let parent = self.parent_world(entity);
        let inverse = parent.compute_affine().inverse();
        if let Ok(mut t) = self.transforms.get_mut(entity) {
            t.translation = inverse.transform_point3(position);
        }
    }
}
