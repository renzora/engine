use crate::aabb::{Bounded, AABB};

#[inline]
pub fn concatenate_vectors<T: Sized>(vectors: &mut [Vec<T>]) -> Vec<T> {
    let mut result = Vec::with_capacity(vectors.iter().map(|v| v.len()).sum());
    for vector in vectors.iter_mut() {
        result.append(vector);
    }
    result
}

#[derive(Copy, Clone)]
pub(crate) struct Bucket {
    pub(crate) size: usize,
    pub(crate) aabb: AABB,
}

impl Bucket {
    pub(crate) const EMPTY: Bucket = {
        Bucket {
            size: 0,
            aabb: AABB::EMPTY,
        }
    };

    #[inline]
    pub(crate) fn add_aabb(&mut self, aabb: &AABB) {
        self.size += 1;
        self.aabb = self.aabb.join(aabb);
    }

    #[inline]
    pub(crate) fn join_bucket(a: Bucket, b: &Bucket) -> Bucket {
        Bucket {
            size: a.size + b.size,
            aabb: a.aabb.join(&b.aabb),
        }
    }
}

#[inline]
pub(crate) fn joint_aabb_of_shapes<Shape: Bounded>(indices: &[usize], shapes: &[Shape]) -> AABB {
    let mut aabb = AABB::EMPTY;
    for index in indices {
        let shape = &shapes[*index];
        aabb.join_mut(&shape.aabb());
    }
    aabb
}
