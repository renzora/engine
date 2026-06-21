use crate::Point2;
use std::ops::Index;

#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum Axis {
    X = 0,
    Y = 1,
}

impl Index<Axis> for Point2 {
    type Output = f32;

    #[inline]
    fn index(&self, axis: Axis) -> &f32 {
        match axis {
            Axis::X => &self.x,
            Axis::Y => &self.y,
        }
    }
}
