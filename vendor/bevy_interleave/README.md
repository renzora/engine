# bevy_interleave 🧩
[![test](https://github.com/mosure/bevy_interleave/workflows/test/badge.svg)](https://github.com/Mosure/bevy_interleave/actions?query=workflow%3Atest)
[![GitHub License](https://img.shields.io/github/license/mosure/bevy_interleave)](https://raw.githubusercontent.com/mosure/bevy_interleave/main/LICENSE)
[![GitHub Releases](https://img.shields.io/github/v/release/mosure/bevy_interleave?include_prereleases&sort=semver)](https://github.com/mosure/bevy_interleave/releases)
[![GitHub Issues](https://img.shields.io/github/issues/mosure/bevy_interleave)](https://github.com/mosure/bevy_interleave/issues)
[![crates.io](https://img.shields.io/crates/v/bevy_interleave.svg)](https://crates.io/crates/bevy_interleave)

bevy static bind group api (e.g. statically typed meshes)


## features

- [x] storage/texture bind group automation
- [x] packed -> planar main world representation /w serialization
- [x] packed -> planar storage/texture GPU representation
- [x] derive macro automation

## minimal example

```rust
use bevy::prelude::*;
use bevy_interleave::prelude::*;


#[derive(
    Clone,
    Debug,
    Default,
    Planar,
    Reflect,
    ReflectInterleaved,
    StorageBindings,
    TextureBindings,
)]
pub struct MyStruct {
    #[texture_format(TextureFormat::R32Sint)]
    pub field: i32,

    #[texture_format(TextureFormat::R32Uint)]
    pub field2: u32,

    #[texture_format(TextureFormat::R8Unorm)]
    pub bool_field: bool,

    #[texture_format(TextureFormat::Rgba32Uint)]
    pub array: [u32; 4],
}


fn main() {
    let interleaved = vec![
        MyStruct { field: 0, field2: 1_u32, bool_field: true, array: [0, 1, 2, 3] },
        MyStruct { field: 2, field2: 3_u32, bool_field: false, array: [4, 5, 6, 7] },
        MyStruct { field: 4, field2: 5_u32, bool_field: true, array: [8, 9, 10, 11] },
    ];

    let planar = PlanarMyStruct::from_interleaved(interleaved);

    println!("{:?}", planar.field);
    println!("{:?}", planar.field2);
    println!("{:?}", planar.bool_field);
    println!("{:?}", planar.array);

    // Prints:
    // [0, 2, 4]
    // [1, 3, 5]
    // [true, false, true]
    // [[0, 1, 2, 3], [4, 5, 6, 7], [8, 9, 10, 11]]

    println!("\n\n{:?}", MyStruct::min_binding_sizes());
    println!("{:?}", MyStruct::ordered_field_names());

    // Prints:
    // [4, 4, 1, 16]
    // ["field", "field2", "bool_field", "array"]
}


// TODO: gpu node binding example, see bevy_gaussian_splatting
```


## why bevy?

`bevy_interleave` simplifies bind group creation within `bevy`. `Planar` derives can be used in conjunction with `ShaderType`'s to support both packed and planar data render pipelines.


## compatible bevy versions

| `bevy_interleave` | `bevy` |
| :--               | :--    |
| `0.9`             | `0.18` |
| `0.8`             | `0.17` |
| `0.7`             | `0.16` |
| `0.3`             | `0.15` |
| `0.2`             | `0.13` |
| `0.1`             | `0.12` |
