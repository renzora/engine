use bevy::{
    camera::{
        primitives::Aabb,
        visibility::{NoFrustumCulling, VisibilityClass, VisibilitySystems, add_visibility_class},
    },
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    math::bounding::BoundingVolume,
    prelude::*,
};
use bevy_interleave::prelude::*;

use crate::gaussian::interface::CommonCloud;

#[derive(Default)]
pub struct CloudPlugin<R: PlanarSync> {
    _phantom: std::marker::PhantomData<R>,
}

pub struct CloudVisibilityClass;

fn add_planar_class(world: DeferredWorld, ctx: HookContext) {
    add_visibility_class::<CloudVisibilityClass>(world, ctx);
}

impl<R: PlanarSync + Reflect + TypePath> Plugin for CloudPlugin<R>
where
    R::PlanarType: CommonCloud,
    R::PlanarTypeHandle: FromReflect + bevy::reflect::Typed,
{
    fn build(&self, app: &mut App) {
        app.register_required_components::<R::PlanarTypeHandle, VisibilityClass>();
        app.world_mut()
            .register_component_hooks::<R::PlanarTypeHandle>()
            .on_add(add_planar_class);

        app.add_systems(
            PostUpdate,
            (calculate_bounds::<R>.in_set(VisibilitySystems::CalculateBounds),),
        );
    }
}

// TODO: handle aabb updates (e.g. gaussian particle movements)
#[allow(clippy::type_complexity)]
pub fn calculate_bounds<R: PlanarSync>(
    mut commands: Commands,
    gaussian_clouds: Res<Assets<R::PlanarType>>,
    without_aabb: Query<(Entity, &R::PlanarTypeHandle), (Without<Aabb>, Without<NoFrustumCulling>)>,
) where
    R::PlanarType: CommonCloud,
{
    for (entity, cloud_handle) in &without_aabb {
        if let Some(cloud) = gaussian_clouds.get(cloud_handle.handle())
            && let Some(aabb3d) = cloud.compute_aabb()
        {
            commands.entity(entity).try_insert(Aabb {
                center: aabb3d.center(),
                half_extents: aabb3d.half_size(),
            });
        }
    }
}
