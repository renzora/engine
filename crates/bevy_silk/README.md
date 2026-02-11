# Bevy Silk

[![workflow](https://github.com/ManevilleF/bevy_silk/actions/workflows/rust.yml/badge.svg)](https://github.com/ManevilleF/bevy_silk/actions/workflows/rust.yml)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)
[![Crates.io](https://img.shields.io/crates/v/bevy_silk.svg)](https://crates.io/crates/bevy_silk)
[![Docs.rs](https://docs.rs/bevy_silk/badge.svg)](https://docs.rs/bevy_silk)
[![dependency status](https://deps.rs/crate/bevy_silk/0.10.0/status.svg)](https://deps.rs/crate/bevy_silk)

<!-- cargo-sync-readme start -->

CPU driven Cloth engine for Bevy using Verlet integration.

## Get started

### Dependency

Add `bevy_silk` as a dependency in the `Cargo.toml`

`bevy_silk = "0.10"`

Or follow the main git branch

`bevy_silk = { git = "https://github.com/ManevilleF/bevy_silk" }`

### Supported Bevy Versions

| `bevy_silk` | `bevy` |
|-------------|--------|
| 0.1.0  | 0.7  |
| 0.2.0  | 0.7  |
| 0.3.0  | 0.8  |
| 0.4.0  | 0.9  |
| 0.5.0  | 0.10 |
| 0.6.0  | 0.11 |
| 0.7.0  | 0.12 |
| 0.8.0  | 0.13 |
| 0.9.0  | 0.14 |
| 0.10.0  | 0.17 |

### Plugin

Add the `ClothPlugin` to your bevy app

```rust no_run
use bevy::prelude::*;
use bevy_silk::prelude::*;

fn main() {
  App::new()
    .add_plugins((DefaultPlugins, ClothPlugin))
    // ... Add your resources and systems
    .run();
}
```

### Add cloth to a mesh

For a mesh to be used as cloth, add the `ClothBuilder` component to any
entity with a `Handle<Mesh>` component.

> Note: `Transform` and `GlobalTransform` are also required
> Note: The mesh render asset usage must allow for _main world_ edits

cloth data which will be populated automatically from the associated
`Handle<Mesh>`.

```rust
use bevy::prelude::*;
use bevy_silk::prelude::*;

fn spawn(mut commands: Commands) {
    commands.spawn((
        // Add your mesh, material and your custom PBR data
        // Mesh3d(...),
        // MeshMaterial3d(...),
        ClothBuilder::new()
            // Define pinned vertices ids using an Iterator
            .with_pinned_vertex_ids(0..9)
            // Define the stick generation mode
            .with_stick_generation(StickGeneration::Quads)
            // Defines the sticks target length option
            .with_stick_length(StickLen::Auto)
            // The cloth will compute flat mesh normals
            .with_flat_normals(),
    ));
}
```

#### Vertex anchoring

Specifying vertex anchors allows to pin some cloth vertices to various
entities. The `ClothBuilder` has multiple methods allowing to anchor
vertices through their id or color.

For example you can pin some cloth vertices to the cloth entity's
`GlobalTransform`:

```rust
use bevy::color::{palettes::css::*, Color};
use bevy_silk::prelude::*;

let cloth = ClothBuilder::new()
    // Adds pinned vertices ids using an Iterator
    .with_pinned_vertex_ids(0..9)
    // Adds a single pinned vertex id
    .with_pinned_vertex_id(10)
    // Adds pinned vertex colors using an Iterator
    .with_pinned_vertex_colors([Color::from(WHITE), Color::from(BLACK)].into_iter())
    // Adds a single pinned vertex color
    .with_pinned_vertex_color(Color::from(YELLOW))
    // Adds pinned vertex positions
    .with_pinned_vertex_positions(|pos| pos.x > 0.0 && pos.z <= 5.0);
```

For more anchoring options, for example to specify a custom entity to pin
the vertices to:

```rust
use bevy::{color::palettes::css::*, prelude::*};
use bevy_silk::prelude::*;

fn spawn(mut commands: Commands) {
    // Spawn an entity and get its id
    let entity_a = commands
        .spawn((
            // Add your components
            // ...
        ))
        .id();
    let anchor_to_a = VertexAnchor {
        // The anchor will pin the vertices to `entity_a`
        custom_target: Some(entity_a),
        // Specify an extra offset from the target's `GlobalTransform`
        custom_offset: Some(Vec3::new(1.0, 1.2, 0.0)),
        ..Default::default()
    };
    let anchor_to_self = VertexAnchor {
        // The anchor will pin the cloth entity
        custom_target: None,
        // Specify an extra offset from the target's `GlobalTransform`
        custom_offset: Some(Vec3::new(-1.0, 0.0, -0.1)),
        ..Default::default()
    };

    let cloth = ClothBuilder::new()
        // Adds pinned vertices ids using an Iterator
        .with_anchored_vertex_ids(0..9, anchor_to_a)
        // Adds a single pinned vertex id
        .with_anchored_vertex_id(10, anchor_to_self)
        // Adds pinned vertex colors using an Iterator
        .with_anchored_vertex_colors(
            [Color::from(WHITE), Color::from(BLACK)].into_iter(),
            anchor_to_a,
        )
        // Adds a single pinned vertex color
        .with_anchored_vertex_color(Color::from(YELLOW), anchor_to_self)
        // Adds pinned vertex positions
        .with_anchored_vertex_positions(|pos| pos.x > 0.0 && pos.z <= 5.0, anchor_to_self);
}
```

Custom anchoring allows to :

* pin vertices to various entities, like skeletal mesh joints
* define custom offsets to customize the distance between the anchored
  vertices an the target
* use world space pinning and ignore the target's rotation for example
* override the vertex positions, using only the offset

### Configuration

You can customize the global cloth physics by inserting the `ClothConfig`
resource to your app:

```rust no_run
use bevy::prelude::*;
use bevy_silk::prelude::*;

fn main() {
  App::new()
    .add_plugins((DefaultPlugins, ClothPlugin))
    .insert_resource(ClothConfig {
        gravity: Vec3::new(0.0, -9.81, 0.0),
        friction: 0.02,
        sticks_computation_depth: 5,
        acceleration_smoothing: AccelerationSmoothing::default()
    })
    // ... Add your resources and systems
    .run();
}
```

`ClothConfig` can also be used as a *component* to override the global
configuration.

## Wind

You may add wind forces to the simulation for a more dynamic clothing
effect, for each force you may choose from:

* `Wind::Constant` for constant wind force
* `Wind::SinWave` for a sin wave following wind intensity with custom force
  and frequency.

`Wind` forces can be added as a resource to your app through the `Winds`
container:

```rust no_run
use bevy::prelude::*;
use bevy_silk::prelude::*;

fn main() {
  App::new()
    .add_plugins((DefaultPlugins, ClothPlugin))
    .insert_resource(Winds {
        wind_forces: vec![Wind::SinWave {
            max_velocity: Vec3::new(10.0, 15.0, -5.0),
            frequency: 3.0,
            normalize: false,
            abs: false
        }]
    })
    // ... Add your resources and systems
    .run();
}
```

> Check the flag example for simple wind effect.

## Collisions

Both [`bevy_rapier`] and [`avian`] are supported for cloth interactions
with colliders. They can be enabled with the `rapier_collisions` and
`avian_collisions` features respectively.

> Note: Collision support is still experimental for now and is not suited
> for production use. Feedback is welcome!

### `bevy_rapier`

Add `bevy_rapier3d::RapierPhysicsPlugin` to your app and a `ClothCollider`
to your entity to enable collisions:

```rust
use bevy::prelude::*;
use bevy_silk::prelude::*;

fn spawn(mut commands: Commands) {
    commands.spawn((
        // Add your mesh, material and your custom PBR data
        // Mesh3d(...),
        // MeshMaterial3d(...),
        ClothBuilder::new(),
        ClothCollider::default(),
    ));
}
```

Three `bevy_rapier` components will be automatically inserted:

* a `RigidBody::KinematicPositionBased`
* a `Collider` which will be updated every frame to follow the cloth bounds
  (AABB)
* a `SolverGroup` set to 0 (`Group::NONE`) in everything, avoiding default
  collision solving.

You can customize what collisions will be checked by specifying
`CollisionGroups`. (See the [`bevy_rapier` docs](https://rapier.rs/docs/user_guides/bevy_plugin/colliders#collision-groups-and-solver-groups)).

### `avian` (previously `bevy_xpbd`)

Add `avian3d::PhysicsPlugins` to your app and a `ClothCollider`
to your entity to enable collisions:

```rust
use bevy::prelude::*;
use bevy_silk::prelude::*;

fn spawn(mut commands: Commands) {
    commands.spawn((
        // Add your mesh, material and your custom PBR data
        // Mesh3d(...),
        // MeshMaterial3d(...),
        ClothBuilder::new(),
        ClothCollider::default(),
    ));
}
```

Three `avian3d` components will be automatically inserted:

* a `RigidBody::Kinematic`
* a `Collider` which will be updated every frame to follow the cloth bounds
  (AABB)
* a `Sensor` used for avoiding default collision solving.

You can customize what collisions will be checked by specifying
`CollisionLayers`. (See the [`avian` docs](https://docs.rs/avian3d/latest/avian3d/collision/struct.CollisionLayers.html)).

## Mesh utils

`bevy_silk` provides a plane mesh generation function `rectangle_mesh`
useful for classic cloth uses like flags or capes

## Q&A

* `My mesh falls immediately and infinitely when I add a Cloth component,
  how to fix it?`

You probably didn't specify any *pinned points*, meaning there are no
vertices anchored to your entity's `GlobalTransform`.

* `My cloth jitters a lot/ suddenly falls down/ has strange sudden
  behaviour`

Gravity and winds are by default smoothed out by the framerate, if the
framerate drops suddenly gravity and wind get much stronger.
If your simulation suffers from this you can specify a custom smooth
value in `ClothConfig::acceleration_smoothing`.

[`bevy_rapier`]: https://github.com/dimforge/bevy_rapier
[`avian`]: https://github.com/Jondolf/avian

<!-- cargo-sync-readme end -->

## TODO list

* [ ] World and per cloth simulation pause/ filter out cloth update
* [ ] Cloth update batching
* [ ] Cloth cutting maybe?

## Examples

![Bevy Flag](docs/flag.gif)

1. Flag example

    run `cargo run --example flag`

2. Balloon example

    run `cargo run --example balloon`

3. Moving example

    run `cargo run --example moving`

4. [`bevy_rapier`] collision example

    run `cargo run --example rapier_collision --features rapier_collisions`

5. [`bevy_xpbd`] collision example

    run `cargo run --example xpbd_collision --features xpbd_collisions`

6. Anchors example

    run `cargo run --example anchors`
