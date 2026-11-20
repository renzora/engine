# glamx - Glam eXtensions

Extensions for [glam](https://github.com/bitshifter/glam-rs): Pose2, Pose3, Rot2, SVD, eigendecomposition, and matrix utilities.

[![Crates.io](https://img.shields.io/crates/v/glamx.svg)](https://crates.io/crates/glamx)
[![Documentation](https://docs.rs/glamx/badge.svg)](https://docs.rs/glamx)
[![License](https://img.shields.io/crates/l/glamx.svg)](https://github.com/dimforge/glamx#license)

## Overview

`glamx` provides additional types commonly needed in game development, robotics, and computer graphics that aren't included in glam:

- **`Rot2` / `DRot2`**: 2D rotations represented as unit complex numbers
- **`Rot3` / `DRot3`**: 3D rotations (re-exports of glam's quaternions)
- **`Pose2` / `DPose2`**: 2D rigid body transformations (rotation + translation)
- **`Pose3` / `DPose3`**: 3D rigid body transformations (rotation + translation)
- **`MatExt`**: Extension trait for glam matrix types
- **`SymmetricEigen2` / `SymmetricEigen3`**: Eigendecomposition for symmetric matrices
- **`Svd2` / `Svd3`**: Singular Value Decomposition for 2x2 and 3x3 matrices

This crate re-exports all of glam's types, so you can use it as a drop-in replacement.

## Feature Flags

| Feature         | Description                            |
|-----------------|----------------------------------------|
| `std` (default) | Enables standard library support       |
| `f64` (default via `all-types`) | Enables `f64` glam types and the corresponding glamx `D*` types (`DRot2`, `DPose2`, `DSymmetricEigen2`, ...) |
| `serde`         | Enables serialization via serde        |
| `bytemuck`      | Enables bytemuck derives for `Rot2`/`DRot2` |
| `nalgebra`      | Enables conversions to/from nalgebra types |
| `libm`          | Uses libm for `no_std` math operations |
| `approx`        | Enables approximate comparison traits  |
| `rand`          | Enables random generation support      |
| `mint`          | Enables mint type conversions          |
| `encase`        | Enables encase encoding                |

All of glam's other type-gating features (`all-types`, `float-types`, `integer-types`, `size-types`, `i8`/`u8`/`i16`/`u16`/`i32`/`u32`/`i64`/`u64`/`isize`/`usize`) are also exposed as passthroughs to glam.

## `no_std` Support

`glamx` is `no_std` compatible. Disable default features and enable `libm` for math operations:

```toml
[dependencies]
glamx = { version = "0.1", default-features = false, features = ["libm"] }
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
