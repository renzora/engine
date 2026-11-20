use static_assertions::assert_cfg;

pub mod cloud;
pub mod covariance;
pub mod f16;
pub mod f32;
pub mod formats;
pub mod interface;
pub mod iter;
pub mod settings;

assert_cfg!(
    any(feature = "packed", feature = "planar",),
    "specify one of the following features: packed, planar",
);
