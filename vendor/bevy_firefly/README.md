# Firefly - 2D Lighting for Bevy 

[![Discord](https://img.shields.io/discord/805147867924267018?logo=discord&color=7289DA)](https://discord.com/channels/691052431525675048/1447681362722033816)
[![crates.io](https://img.shields.io/crates/v/bevy_firefly)](https://crates.io/crates/bevy_firefly)
[![docs](https://docs.rs/bevy_firefly/badge.svg)](https://docs.rs/bevy_firefly/)
[![downloads](https://img.shields.io/crates/d/bevy_firefly)](https://crates.io/crates/bevy_firefly)

[Firefly](https://crates.io/crates/bevy_firefly) is an open-source, **2d lighting** crate for the [Bevy game engine](https://bevy.org/). 

## Objective

When I was working on a Bevy project, I found myself needing features that other 2d lighting crates did not yet provide. I needed an accessible 2d lighting solution that offers the same capabilities as engines such as Unity and Godot. So, as part of my bachelor thesis, I began work on Firely.

My **main objectives** while making and maintaining Firefly are: 
- **Minimal Setup.** this crate should be extremely easy to plug into any existing bevy app, and the API should feel minimal and intuitive.
- **Consistent Maintenance.** I will keep this crate up-to-date with all new bevy versions and changes, until it is inevitably deprectated by another solution or upstreamed into Bevy itself.
- **Community Feedback.** I'm eagerly accepting any feature requests and bug reports. I do have my own backlog but any features requested by users will be prioritized.
- **Power.** While Bevy has never had a proper 2d lighting solution similar to other game engines, I am dedicated to changing that. Firefly should offer users all the features that those engines do and more!

## Showcases 
Here are some videos showcasing various features of Firefly. Credit for the character and assets goes to [Kimberly](https://github.com/Kaircha) and her upcoming game, Starlight!

Soft shadows and z-sorting.

https://github.com/user-attachments/assets/1984ef2a-0edd-4a40-93cb-a901057a9b74

Same scene but with light banding and hard shadows.

https://github.com/user-attachments/assets/6118f75e-b797-41bb-998e-381dc9d84cb9

Video of the [flashlight example](https://github.com/PVDoriginal/firefly/blob/main/examples/flashlight.rs).

https://github.com/user-attachments/assets/4c525c50-39a6-4604-904e-65f38676f4c7

Video of the [crates example](https://github.com/PVDoriginal/firefly/blob/main/examples/shapes.rs), showcasing normal maps and z-sorting.

https://github.com/user-attachments/assets/fd9453ba-e42a-4155-b96b-889bfdceea48

Video of the [stress example](https://github.com/PVDoriginal/firefly/blob/main/examples/stress.rs).

https://github.com/user-attachments/assets/c9b8c716-a0c4-4604-8fbb-50d6bbbe8aad

## Usage 
To use this crate, simply run `cargo add bevy_firefly` or add Firefly to your Cargo.toml file. 

You can see all the Firefly versions [here](https://crates.io/crates/bevy_firefly/versions). 

Here is a basic example of integrating Firefly into a bevy app: 

```Rs
use bevy::prelude::*;
use bevy_firefly::prelude::*;

fn main() {
  App:new()
    .add_plugins((DefaultPlugins, FireflyPlugin))
    .add_systems(Startup, setup)
    .run();
}

fn setup(mut commands: Commands) {
  commands.spawn((
    Camera2d,
    FireflyConfig::default()
  ));
     
  commands.spawn((
    PointLight2d {
      color: Color::srgb(1.0, 0.0, 0.0),
      range: 100.0,
      ..default()
    },
  ));
     
  commands.spawn((
    Occluder2d::circle(10.0),
    Transform::from_translation(vec3(0.0, 50.0, 0.0)),
  ));
}
```
Check out the [examples](examples/) and the [crate documentation](https://docs.rs/bevy_firefly/) to learn more about using it.

## Features 

Some of the existing features are:
  - Point lights
  - Round and polygonal occluders
  - Soft shadows
  - Occlusion z-sorting
  - Normal maps
  - Transparent & colored occluders
  - Light banding
  - Multiple Lightmaps
  - Render Layers 

Some of the currently planned features are: 
  - Occluders casting sprite-based shadows
  - Light textures

Check out my [milestone goals](https://github.com/users/PVDoriginal/projects/7/views/2) to see what features are currently planned based on the Bevy release cycle. 

Feel free to open an issue if you want to request any specific features or report any bugs!

Also you can ask any questions over on [discord](https://discord.com/channels/691052431525675048/1447681362722033816)! You can also follow the thread to be kept up-to-date with new features. 

## Bevy Compatibility 

| bevy | bevy_firefly  |
|------|---------------|
| 0.19 | 0.19          |
| 0.18 | 0.18          |
| 0.17 | 0.17          |
| 0.16 | 0.16          |

Each minor Firefly version is compatible with that specific Bevy version (e.g. Firefly 0.18.x should work with Bevy 0.18.0, 0.18.1, and so on). 

The Firefly patch version is unrelated to Bevy's (e.g. Firefly 0.18.1 is not related to Bevy 0.18.1 and still works with any Bevy 0.18 version).

Unlike Bevy, Firefly can have breaking API changes in every release. 

## Current Limitations 

Some notable limitations that Firefly currently has: 
  - WebGPU only. I've chosen to drop WebGL2 support in favor of WebGPU and the many possibilities it offers, which means Firefly might not run on specific browsers / devices yet (although WebGPU support is rapidly growing). I am willing to add and maintain a WebGL2 compatibility mode if there's a significant need for it though. 
  - Firefly was mostly designed to work with Sprites, which means there is limited compatibility with 2d Meshes. While lights can be cast over them, they don't yet support normal maps or occlusion z-sorting. This will be changed very soon as Bevy's Sprite backend is being migrated to the mesh one.
  - Occluder scaling isn't supported yet. 

These aren't hard limitations though, and can be overcome with some effort. I just didn't have the time yet. If you want me to prioritize fixing any of them, feel free to open an issue. 

## Alternatives

There are a few other 2d lighting solutions for Bevy that I'm aware of: 
- [**bevy_magic_light**](https://github.com/zaycev/bevy-magic-light-2d). 
- [**bevy_lit**](https://github.com/malbernaz/bevy_lit).  
- [**bevy_light_2d**](https://github.com/jgayfer/bevy_light_2d). 
