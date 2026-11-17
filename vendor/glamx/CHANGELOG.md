# v0.3.1

- Add `From` conversions between `Pose2`/`Pose3` and their `(translation, rotation)` tuples.
  ([#3](https://github.com/dimforge/glamx/pull/3))
- Fix `Rot2::slerp` (and therefore `Pose2::lerp`) taking the long way around whenever the arc
  crosses the `-pi`/`pi` cut of `Rot2::angle`. It now interpolates along the shortest arc.

# v0.3.0

- Update to `glam 0.33`, `simba 0.10`, and `nalgebra 0.35`. ([#5](https://github.com/dimforge/glamx/pull/5))
- Mirror glam 0.33's type-gating features: `all-types` (default), `float-types`, `integer-types`, `size-types`, and individual `f64`/`i8`/`u8`/`i16`/`u16`/`i32`/`u32`/`i64`/`u64`/`isize`/`usize` passthroughs. Disabling `f64` now also removes the corresponding glamx `D*` types (`DRot2`, `DPose3`, `DSymmetricEigen3`, ...). ([#5](https://github.com/dimforge/glamx/pull/5))
- Bump MSRV to 1.89. ([#5](https://github.com/dimforge/glamx/pull/5))

# v0.2.0

- Add `padding` field to `Pose3` for better compatibility with `bytemuck` and `spirv`.
- Fix various internal implementation details that are incompatible with `spirv`/`naga`.
- Update to `glam 0.32`.

# v0.1.3

- Add `From`/`Into` conversions between f32 and f64 type variants.

# v0.1.2

- Add optional `rkyv 0.8` support.