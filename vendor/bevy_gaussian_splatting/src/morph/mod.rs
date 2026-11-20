use bevy::prelude::*;
use bevy_interleave::prelude::*;

#[cfg(feature = "morph_interpolate")]
pub mod interpolate;

#[cfg(feature = "morph_particles")]
pub mod particle;

pub struct MorphPlugin<R: PlanarSync> {
    _phantom: std::marker::PhantomData<R>,
}
impl<R: PlanarSync> Default for MorphPlugin<R> {
    fn default() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<R> Plugin for MorphPlugin<R>
where
    R: PlanarSync,
    R::GpuPlanarType: GpuPlanarStorage,
    <R::GpuPlanarType as GpuPlanar>::PackedType: ReflectInterleaved,
{
    fn build(&self, app: &mut App) {
        #[cfg(feature = "morph_interpolate")]
        {
            app.add_plugins(interpolate::InterpolatePlugin::<R>::default());
        }

        #[cfg(feature = "morph_particles")]
        {
            app.add_plugins(particle::ParticleBehaviorPlugin::<R>::default());
        }

        #[cfg(not(any(feature = "morph_interpolate", feature = "morph_particles")))]
        let _ = app;
    }
}
