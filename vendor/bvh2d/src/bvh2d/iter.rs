use crate::bvh2d::{BVH2d, BVH2dNode};
use crate::{ContainedBy, Point2};

#[allow(clippy::upper_case_acronyms)]
pub struct BVH2dTraverseIterator<'a> {
    bvh2d: &'a BVH2d,
    point: &'a Point2,
    stack: [usize; 32],
    node_index: usize,
    stack_size: usize,
    has_node: bool,
}

impl<'a> BVH2dTraverseIterator<'a> {
    pub(crate) fn new(bvh2d: &'a BVH2d, point: &'a Point2) -> Self {
        BVH2dTraverseIterator {
            bvh2d,
            point,
            stack: [0; 32],
            node_index: 0,
            stack_size: 0,
            has_node: true,
        }
    }

    fn is_stack_empty(&self) -> bool {
        self.stack_size == 0
    }

    fn stack_push(&mut self, node: usize) {
        self.stack[self.stack_size] = node;
        self.stack_size += 1;
    }

    fn stack_pop(&mut self) -> usize {
        self.stack_size -= 1;
        self.stack[self.stack_size]
    }

    fn move_left(&mut self) {
        match self.bvh2d.nodes[self.node_index] {
            BVH2dNode::Node {
                child_l_index,
                ref child_l_aabb,
                ..
            } => {
                if self.point.contained_by(child_l_aabb) {
                    self.node_index = child_l_index;
                    self.has_node = true;
                } else {
                    self.has_node = false;
                }
            }
            BVH2dNode::Leaf { .. } => {
                self.has_node = false;
            }
        }
    }

    fn move_right(&mut self) {
        match self.bvh2d.nodes[self.node_index] {
            BVH2dNode::Node {
                child_r_index,
                ref child_r_aabb,
                ..
            } => {
                if self.point.contained_by(child_r_aabb) {
                    self.node_index = child_r_index;
                    self.has_node = true;
                } else {
                    self.has_node = false;
                }
            }
            BVH2dNode::Leaf { .. } => {
                self.has_node = false;
            }
        }
    }
}

impl<'a> Iterator for BVH2dTraverseIterator<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.is_stack_empty() && !self.has_node {
                // Completed traversal.
                break;
            }
            if self.has_node {
                // If we have any node, save it and attempt to move to its left child.
                self.stack_push(self.node_index);
                self.move_left();
            } else {
                // Go back up the stack and see if a node or leaf was pushed.
                self.node_index = self.stack_pop();
                match self.bvh2d.nodes[self.node_index] {
                    BVH2dNode::Node { .. } => {
                        // If a node was pushed, now attempt to move to its right child.
                        self.move_right();
                    }
                    BVH2dNode::Leaf { shape_index, .. } => {
                        // We previously pushed a leaf node. This is the "visit" of the in-order traverse.
                        // Next time we call `next()` we try to pop the stack again.
                        self.has_node = false;
                        return Some(shape_index);
                    }
                }
            }
        }
        None
    }
}
