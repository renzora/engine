use bevy::{math::bounding::Aabb3d, prelude::*};
use bevy_interleave::prelude::Planar;

#[cfg(feature = "sort_rayon")]
use rayon::prelude::*;

use crate::gaussian::iter::PositionIter;

pub trait CommonCloud
where
    Self: Planar,
{
    type PackedType;

    fn len_sqrt_ceil(&self) -> usize {
        (self.len() as f32).sqrt().ceil() as usize
    }
    fn square_len(&self) -> usize {
        self.len_sqrt_ceil().pow(2)
    }

    fn compute_aabb(&self) -> Option<Aabb3d> {
        if self.is_empty() {
            return None;
        }

        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);

        // TODO: find a more correct aabb bound derived from scalar max gaussian scale
        let max_scale = 0.1;

        #[cfg(feature = "sort_rayon")]
        {
            (min, max) = self
                .position_par_iter()
                .fold(
                    || (min, max),
                    |(curr_min, curr_max), position| {
                        let pos = Vec3::from(*position);
                        let offset = Vec3::splat(max_scale);
                        (curr_min.min(pos - offset), curr_max.max(pos + offset))
                    },
                )
                .reduce(
                    || (min, max),
                    |(a_min, a_max), (b_min, b_max)| (a_min.min(b_min), a_max.max(b_max)),
                );
        }

        #[cfg(not(feature = "sort_rayon"))]
        {
            for position in self.position_iter() {
                min = min.min(Vec3::from(*position) - Vec3::splat(max_scale));
                max = max.max(Vec3::from(*position) + Vec3::splat(max_scale));
            }
        }

        Some(Aabb3d {
            min: min.into(),
            max: max.into(),
        })
    }

    fn visibility(&self, index: usize) -> f32;
    fn visibility_mut(&mut self, index: usize) -> &mut f32;

    // TODO: type erasure for position iterators
    fn position_iter(&self) -> PositionIter<'_>;

    #[cfg(feature = "sort_rayon")]
    fn position_par_iter(&self) -> crate::gaussian::iter::PositionParIter<'_>;
}

pub trait TestCloud {
    fn test_model() -> Self;
}

// TODO: CloudSlice and CloudStream traits
