use aabb::AABB;

pub const EPSILON: f32 = 0.00001;

pub type Point2 = glam::Vec2;

pub type Vector2 = glam::Vec2;

pub mod aabb;
pub mod axis;
pub mod bvh2d;
mod utils;

trait ContainedBy {
    fn contained_by(&self, aabb: &AABB) -> bool;
}

impl ContainedBy for Point2 {
    #[inline]
    fn contained_by(&self, aabb: &AABB) -> bool {
        (aabb.min.x..=aabb.max.x).contains(&self.x) && (aabb.min.y..=aabb.max.y).contains(&self.y)
    }
}
