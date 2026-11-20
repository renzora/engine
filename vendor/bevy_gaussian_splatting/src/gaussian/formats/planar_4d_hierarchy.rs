// TODO: gaussian cloud 4d with temporal hierarchy
use crate::gaussian::formats::planar_4d::PlanarGaussian4dHandle;

pub struct TemporalGaussianLevel {
    pub instance_count: usize,
    // TODO: swap buffer slicing
}

// TODO: make this an asset
pub struct TemporalGaussianHierarchy {
    pub flat_cloud: PlanarGaussian4dHandle,
    pub levels: Vec<TemporalGaussianLevel>,
    // TODO: level descriptor validation
}

// TODO: implement level streaming utilities in src/stream/hierarchy.rs
// TODO: implement GPU slice utilities in src/stream/slice.rs
