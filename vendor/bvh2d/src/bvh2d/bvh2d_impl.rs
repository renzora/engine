use crate::aabb::{Bounded, AABB};
use crate::bvh2d::iter::BVH2dTraverseIterator;
use crate::utils::{concatenate_vectors, joint_aabb_of_shapes, Bucket};
use crate::Point2;
use crate::EPSILON;
use std::f32;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[allow(clippy::upper_case_acronyms)]
pub(crate) enum BVH2dNode {
    Leaf {
        shape_index: usize,
    },

    Node {
        child_l_index: usize,
        child_l_aabb: AABB,
        child_r_index: usize,
        child_r_aabb: AABB,
    },
}

impl BVH2dNode {
    const DUMMY: BVH2dNode = { BVH2dNode::Leaf { shape_index: 0 } };

    fn build<T: Bounded>(shapes: &[T], indices: &[usize], nodes: &mut Vec<BVH2dNode>) -> usize {
        // Helper function to accumulate the AABB joint and the centroids AABB
        #[inline]
        fn grow_convex_hull(convex_hull: (AABB, AABB), shape_aabb: &AABB) -> (AABB, AABB) {
            let center = &shape_aabb.center();
            let convex_hull_aabbs = &convex_hull.0;
            let convex_hull_centroids = &convex_hull.1;
            (
                convex_hull_aabbs.join(shape_aabb),
                convex_hull_centroids.grow(center),
            )
        }

        // If there is only one element left, don't split anymore
        if indices.len() == 1 {
            let shape_index = indices[0];
            let node_index = nodes.len();
            nodes.push(BVH2dNode::Leaf { shape_index });
            // Let the shape know the index of the node that represents it.
            return node_index;
        }

        let mut convex_hull = (AABB::EMPTY, AABB::EMPTY);
        for index in indices {
            convex_hull = grow_convex_hull(convex_hull, &shapes[*index].aabb());
        }
        let (aabb_bounds, centroid_bounds) = convex_hull;

        // From here on we handle the recursive case. This dummy is required, because the children
        // must know their parent, and it's easier to update one parent node than the child nodes.
        let node_index = nodes.len();
        nodes.push(BVH2dNode::DUMMY);

        // Find the axis along which the shapes are spread the most.
        let split_axis = centroid_bounds.largest_axis();
        let split_axis_size = centroid_bounds.max[split_axis] - centroid_bounds.min[split_axis];

        // The following `if` partitions `indices` for recursively calling `BVH2d::build`.
        let (child_l_index, child_l_aabb, child_r_index, child_r_aabb) = if split_axis_size
            < EPSILON
        {
            // In this branch the shapes lie too close together so that splitting them in a
            // sensible way is not possible. Instead we just split the list of shapes in half.
            let (child_l_indices, child_r_indices) = indices.split_at(indices.len() / 2);
            let child_l_aabb = joint_aabb_of_shapes(child_l_indices, shapes);
            let child_r_aabb = joint_aabb_of_shapes(child_r_indices, shapes);

            // Proceed recursively.
            let child_l_index = BVH2dNode::build(shapes, child_l_indices, nodes);
            let child_r_index = BVH2dNode::build(shapes, child_r_indices, nodes);
            (child_l_index, child_l_aabb, child_r_index, child_r_aabb)
        } else {
            const NUM_BUCKETS: usize = 4;
            const BUCKET_START_CAPACITY: usize = 20;
            let mut buckets = [Bucket::EMPTY; NUM_BUCKETS];
            let mut bucket_assignments: [Vec<usize>; NUM_BUCKETS] = [
                Vec::with_capacity(BUCKET_START_CAPACITY),
                Vec::with_capacity(BUCKET_START_CAPACITY),
                Vec::with_capacity(BUCKET_START_CAPACITY),
                Vec::with_capacity(BUCKET_START_CAPACITY),
            ];

            // In this branch the `split_axis_size` is large enough to perform meaningful splits.
            // We start by assigning the shapes to `Bucket`s.
            for idx in indices {
                let shape = &shapes[*idx];
                let shape_aabb = shape.aabb();
                let shape_center = shape_aabb.center();

                // Get the relative position of the shape centroid `[0.0..1.0]`.
                let bucket_num_relative =
                    (shape_center[split_axis] - centroid_bounds.min[split_axis]) / split_axis_size;

                // Convert that to the actual `Bucket` number.
                let bucket_num = (bucket_num_relative * (NUM_BUCKETS as f32 - 0.01)) as usize;

                // Extend the selected `Bucket` and add the index to the actual bucket.
                buckets[bucket_num].add_aabb(&shape_aabb);
                bucket_assignments[bucket_num].push(*idx);
            }

            // Compute the costs for each configuration and select the best configuration.
            let mut min_bucket = 0;
            let mut min_cost = f32::INFINITY;
            let mut child_l_aabb = AABB::EMPTY;
            let mut child_r_aabb = AABB::EMPTY;
            for i in 0..(NUM_BUCKETS - 1) {
                let (l_buckets, r_buckets) = buckets.split_at(i + 1);
                let child_l = l_buckets.iter().fold(Bucket::EMPTY, Bucket::join_bucket);
                let child_r = r_buckets.iter().fold(Bucket::EMPTY, Bucket::join_bucket);

                let cost = (child_l.size as f32 * child_l.aabb.surface_area()
                    + child_r.size as f32 * child_r.aabb.surface_area())
                    / aabb_bounds.surface_area();
                if cost < min_cost {
                    min_bucket = i;
                    min_cost = cost;
                    child_l_aabb = child_l.aabb;
                    child_r_aabb = child_r.aabb;
                }
            }

            // Join together all index buckets.
            let (l_assignments, r_assignments) = bucket_assignments.split_at_mut(min_bucket + 1);
            let child_l_indices = concatenate_vectors(l_assignments);
            let child_r_indices = concatenate_vectors(r_assignments);

            // Proceed recursively.
            let child_l_index = BVH2dNode::build(shapes, &child_l_indices, nodes);
            let child_r_index = BVH2dNode::build(shapes, &child_r_indices, nodes);
            (child_l_index, child_l_aabb, child_r_index, child_r_aabb)
        };

        // Construct the actual data structure and replace the dummy node.
        debug_assert!(!child_l_aabb.is_empty());
        debug_assert!(!child_r_aabb.is_empty());
        nodes[node_index] = BVH2dNode::Node {
            child_l_aabb,
            child_l_index,
            child_r_aabb,
            child_r_index,
        };

        node_index
    }
}

#[allow(clippy::upper_case_acronyms)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct BVH2d {
    pub(crate) nodes: Vec<BVH2dNode>,
}

impl BVH2d {
    pub fn build<Shape: Bounded>(shapes: &[Shape]) -> BVH2d {
        let indices = (0..shapes.len()).collect::<Vec<usize>>();
        let expected_node_count = shapes.len() * 2;
        let mut nodes = Vec::with_capacity(expected_node_count);
        BVH2dNode::build(shapes, &indices, &mut nodes);
        BVH2d { nodes }
    }

    pub fn contains_iterator<'a>(&'a self, point: &'a Point2) -> BVH2dTraverseIterator<'a> {
        BVH2dTraverseIterator::new(self, point)
    }
}
