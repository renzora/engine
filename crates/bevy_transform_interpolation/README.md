# `bevy_transform_interpolation`

[![MIT/Apache 2.0](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/Jondolf/bevy_transform_interpolation#license)
[![ci](https://github.com/Jondolf/bevy_transform_interpolation/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/Jondolf/bevy_transform_interpolation/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/bevy_transform_interpolation?label=crates.io)](https://crates.io/crates/bevy_transform_interpolation)
[![docs.rs](https://img.shields.io/docsrs/bevy_transform_interpolation?label=docs.rs)](https://docs.rs/bevy_transform_interpolation)

A drop-in `Transform` interpolation solution for fixed timesteps for the [Bevy game engine](https://bevyengine.org).

## What Is This For?

A lot of gameplay logic and movement systems typically use a fixed timestep to produce consistent and stable behavior
regardless of the frame rate. Notable examples include physics simulation and character movement.

However, this can make movement appear choppy, especially on displays with a high refresh rate.
To achieve visually smooth movement while using a fixed timestep, the visual transform must be smoothed
independently of the "true" gameplay transform.

The most common way to do this is to use **transform interpolation**, which interpolates movement from the previous
state to the current state. This could be done by storing the current and old gameplay positions in their own components
and interpolating `Transform` using them:

```rust
use bevy::prelude::*;

#[derive(Component, Deref, DerefMut)]
struct Position(Vec3);

#[derive(Component, Deref, DerefMut)]
struct OldPosition(Vec3);

fn interpolate_transforms(
    mut query: Query<(&mut Transform, &Position, &OldPosition)>,
    fixed_time: Res<Time<Fixed>>
) {
    // How much of a "partial timestep" has accumulated since the last fixed timestep run.
    // Between `0.0` and `1.0`.
    let overstep = fixed_time.overstep_fraction();

    for (mut transform, position, old_position) in &mut query {
        // Linearly interpolate the translation from the old position to the current one.
        transform.translation = old_position.lerp(position.0, overstep);
    }
}
```

In fact, you could simply plug the above implementation into your own application if you wanted to!

However, it requires you to use `Position` for gameplay logic, and to manage `OldPosition` somewhere.
This can be annoying, and is incompatible with third party libraries that expect to be able to modify
the transform directly.

`bevy_transform_interpolation` aims to be a drop-in solution that allows easy and efficient transform interpolation,
while still allowing the usage of `Transform` for gameplay logic. It should be automatically compatible with physics engines
such as [Avian](https://github.com/Jondolf/avian) and [`bevy_rapier`](https://github.com/dimforge/bevy_rapier), as long as
the simulation is run in `FixedUpdate` or `FixedPostUpdate`.

## Getting Started

First, add `bevy_transform_interpolation` to your dependencies in `Cargo.toml`:

```toml
[dependencies]
bevy_transform_interpolation = "0.4"
```

To enable `Transform` interpolation, add the `TransformInterpolationPlugin` to your app:

```rust
use bevy::prelude::*;
use bevy_transform_interpolation::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, TransformInterpolationPlugin::default()))
        // ...other plugins, resources, and systems
        .run();
}
```

By default, interpolation is only performed for entities with the `TransformInterpolation` component:

```rust
fn setup(mut commands: Commands) {
    // Interpolate the entire transform: translation, rotation, and scale.
    commands.spawn((
        Transform::default(),
        TransformInterpolation,
    ));
}
```

Now, any changes made to the `Transform` of the entity in `FixedPreUpdate`, `FixedUpdate`, or `FixedPostUpdate`
will automatically be interpolated in between fixed timesteps.

If you want _all_ entities with a `Transform` to be interpolated by default, you can use
`TransformInterpolationPlugin::interpolate_all()`:

```rust
fn main() {
    App::new()
        .add_plugins(TransformInterpolationPlugin::interpolate_all())
        // ...
        .run();
}
```

## Advanced Usage

For a lot of applications, the functionality shown in the Getting Started guide might be all you need!
However, `bevy_transform_interpolation` has a lot more to offer:

- Granularly ease individual properties of the transform with `TranslationInterpolation`, `RotationInterpolation`, and `ScaleInterpolation`.
- Opt out of transform easing for individual entities with `NoTranslationEasing`, `NoRotationEasing`, and `NoScaleEasing`.
- Use extrapolation instead of interpolation with the `TransformExtrapolationPlugin` and its related components.
- Use Hermite interpolation for more natural and accurate movement with the `TransformHermiteEasingPlugin`.
- Implement custom easing backends for your specific needs.

## How Does It Work?

Internally, `bevy_transform_interpolation` simply maintains components that store the `start` and `end` of the interpolation.
For example, translation uses the following component for easing the movement:

```rust
pub struct TranslationEasingState {
    pub start: Option<Vec3>,
    pub end: Option<Vec3>,
}
```

The states are updated by the `TransformInterpolationPlugin` or `TransformExtrapolationPlugin`
depending on whether the entity has `TransformInterpolation` or `TransformExtrapolation` components.

If interpolation is used:

- In `FixedFirst`, `start` is set to the current `Transform`.
- In `FixedLast`, `end` is set to the current `Transform`.

If extrapolation is used:

- In `FixedLast`, `start` is set to the current `Transform`, and `end` is set to the `Transform` predicted based on velocity.

At the start of the `FixedFirst` schedule, the states are reset to `None`. If the `Transform` is detected to have changed
since the last easing run but _outside_ of the fixed timestep schedules, the easing is also reset to `None` to prevent overwriting the change.

The actual easing is performed in `RunFixedMainLoop`, right after `FixedMain`, before `Update`.
By default, linear interpolation (`lerp`) is used for translation and scale, and spherical linear interpolation (`slerp`)
is used for rotation.

However, thanks to the modular and flexible architecture, other easing methods can also be used.
The `TransformHermiteEasingPlugin` provides an easing backend using Hermite interpolation,
overwriting the linear interpolation for specific entities with the `NonlinearTranslationEasing`
and `NonlinearRotationEasing` marker components. Custom easing solutions can be implemented using the same pattern.

## Supported Bevy Versions

| `bevy`  | `bevy_transform_interpolation` |
| ------- | ------------------------------ |
| 0.19-rc | main                           |
| 0.18    | 0.4                            |
| 0.17    | 0.3                            |
| 0.16    | 0.2                            |
| 0.15    | 0.1                            |

## License

`bevy_transform_interpolation` is free and open source. All code in this repository is dual-licensed under either:

- MIT License ([LICENSE-MIT](/LICENSE-MIT) or <http://opensource.org/licenses/MIT>)
- Apache License, Version 2.0 ([LICENSE-APACHE](/LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)

at your option.
