# `bevy_heavy`

[![MIT/Apache 2.0](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/Jondolf/bevy_heavy#license)
[![ci](https://github.com/Jondolf/bevy_heavy/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/Jondolf/bevy_heavy/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/bevy_heavy?label=crates.io)](https://crates.io/crates/bevy_heavy)
[![docs.rs](https://img.shields.io/docsrs/bevy_heavy?label=docs.rs)](https://docs.rs/bevy_heavy)

`bevy_heavy` is a crate for computing mass properties (mass, angular inertia, and center of mass)
for the [geometric primitives] in the [Bevy game engine][Bevy]. This is typically required
for things like physics simulations.

[geometric primitives]: https://docs.rs/bevy/latest/bevy/math/primitives/index.html
[Bevy]: https://bevyengine.org

## Features

- `MassProperties2d` and `MassProperties3d` structs containing the mass, angular inertia, and local center of mass
- Mass property computation for all of Bevy's [geometric primitives]
- Eigensolver for symmetric 3x3 matrices
- Support for `bevy_reflect` and `serde` through the `bevy_reflect` and `serialize` feature flags

## Usage

Mass properties can be computed individually for shapes using the `mass`, `angular_inertia`,
and `center_of_mass` methods:

```rust
use bevy_heavy::{ComputeMassProperties2d, MassProperties2d};
use bevy_math::{primitives::Rectangle, Vec2};

let rectangle = Rectangle::new(2.0, 1.0);
let density = 2.0;

let mass = rectangle.mass(density);
let angular_inertia = rectangle.angular_inertia(mass);
let center_of_mass = rectangle.center_of_mass();
```

You can also compute all mass properties at once, returning `MassProperties2d` for 2D shapes,
or `MassProperties3d` for 3D shapes. This can be more efficient when more than one property is needed.

```rust
let mass_props = rectangle.mass_properties(density);
```

The mass property types have several helper methods for various transformations and operations:

```rust
let shifted_inertia = mass_props.shifted_angular_inertia(Vec2::new(-3.5, 1.0));
let global_center_of_mass = mass_props.global_center_of_mass(Vec2::new(5.0, 7.5));
```

You can also add and subtract mass properties:

```rust
let mass_props_2 = MassProperties2d::new(mass, angular_inertia, Vec2::new(0.0, 1.0));
let sum = mass_props + mass_props_2;
approx::assert_relative_eq!(sum - mass_props_2, mass_props);
```

To support mass property computation for custom shapes, implement `ComputeMassProperties2d`
or `ComputeMassProperties3d` for them.

## Supported `bevy_math` Versions

| `bevy_math` | `bevy_heavy` |
| ----------- | ------------ |
| 0.19-rc     | main         |
| 0.18        | 0.4          |
| 0.17        | 0.3          |
| 0.16        | 0.2          |
| 0.15        | 0.1          |

## License

`bevy_heavy` is free, open source, and permissively licensed! Except where noted (below and/or in individual files),
all code in this repository is dual-licensed under either:

- MIT License ([LICENSE-MIT](/LICENSE-MIT) or <http://opensource.org/licenses/MIT>)
- Apache License, Version 2.0 ([LICENSE-APACHE](/LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)

at your option. This dual-licensing approach is the de-facto standard in the Rust ecosystem,
and there are [very good reasons](https://github.com/bevyengine/bevy/issues/2373) to include both.
