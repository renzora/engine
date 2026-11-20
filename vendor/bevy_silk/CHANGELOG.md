# Changelog

## [Unreleased]

## 0.10.0

* Bevy `0.17`

## 0.9.0

* Physics improvements (#25)
* Bevy `0.14`
* Migrated from `bevy_xpbd` to `avian`
* (**BREAKING**) Renamed `xpbd_collisions` feature to `avian_collisions`
* Renamed `xpbd_collision_example` to `avian_collision_example`

## 0.8.0

* Bump `bevy` to `0.13.x`
* Bump `bevy_rapier3d` to `0.25.x`
* Bump `bevy_xpbd3d` to `0.4.x`
* Bump `bevy_inspector_egui` to `0.23.x`
* Reworked example camera plugin

## 0.7.0

* Bump `bevy` to `0.12.x`
* Bump `bevy_rapier3d` to `0.23.x`
* Bump `bevy_inspector_egui` to `0.21.x`
* [bevy_xpbd](https://github.com/Jondolf/bevy_xpbd) collision support (#22):
  * Added `xpbd_collisions` feature
  * Added `xpbd_collision` example
  * Split `collisions` into `rapier` and `xpbd` sub-modules
* Collisions happen on fixed update schedule (#20)
* Fixed clippy warnings for rust 1.72.0 (#19)
* Added rustfmt config (#19)
* Added vertex colors in flag example (#19)

## 0.6.0

* Bump `bevy` to `0.11.x`
* Bump `bevy_rapier3d` to `0.22.x`

## 0.5.0

* Bump `bevy` to `0.10.x`
* Bump `bevy_rapier3d` to `0.21.x`
* `ClothBuilder` is now `Reflect` and registered
* Added vertex position anchoring for `ClothBuilder`:
  * `ClothBuilder::with_pinned_vertex_positions`
  * `ClothBuilder::with_anchored_vertex_positions`
* Removed unused `VertexAnchor::world_space` attribute
* Added `StickMode` enum with spring variants

### Fixes

* Fixed visibility issues with cloths. Now the `Aabb` is directly handled by
the cloth engine.

### bevy_inspector_egui

With the new version (See [Migration guide](https://github.com/jakobhellermann/bevy-inspector-egui/blob/main/docs/MIGRATION_GUIDE_0.15_0.16.md)),
the following changes were applied:

* (**BREAKING**) Removed `debug` feature
* Removed `bevy_inspector_egui` dependency, kept only as `dev-dependency` for examples

## 0.4.0

* Bump `bevy` to `0.9.x`
* Bump `bevy_rapier3d` to `0.19.x`
* Bump `bevy_inspector_egui` to `0.14.x`
* Modules refactoring

## 0.3.0

* Bump `bevy` to `0.8.x`
* Bump `bevy_rapier3d` to `0.18.x`
* Bump `bevy_inspector_egui` to `0.13.x`
* Removed `smooth-bevy-cameras` dependency

## 0.2.0

### Added

* [bevy_rapier](https://github.com/dimforge/bevy_rapier) collision support:
  * Added `rapier_collisions` feature
  * Added `rapier_collision` example
  * Added `ClothCollider` component
* Added `AccelerationSmoothing` enum, defining gravity/winds acceleration smoothing
  * Added related `acceleration_smoothing` field to `ClothConfig`
* Added custom anchor support with `VertexAnchor`

### API changes

* (**BREAKING**) Renamed `ClothBuilder::fixed_points` to `anchored_vertex_ids`
  * Added `ClothBuilder::with_pinned_vertex_ids` method
  * Added `ClothBuilder::with_pinned_vertex_id` method
  * Added `ClothBuilder::with_anchored_vertex_ids` method
  * Added `ClothBuilder::with_anchored_vertex_id` method
  * Deprecated `ClothBuilder::with_fixed_points` in favor of `ClothBuilder::with_pinned_vertex_ids`
* Added `ClothBuilder::anchored_vertex_colors` field:
  * Added `ClothBuilder::with_pinned_vertex_colors` method
  * Added `ClothBuilder::with_pinned_vertex_color` method
  * Added `ClothBuilder::with_anchored_vertex_colors` method
  * Added `ClothBuilder::with_anchored_vertex_color` method
* Added `ClothBuilder::with_flat_normals` method
  * Deprecated `ClothBuilder::with_flat_normal_computation` in favor of `ClothBuilder::with_flat_normals`
* Added `ClothBuilder::with_smooth_normals` method
  * Deprecated `ClothBuilder::with_smooth_normal_computation` in favor of `ClothBuilder::with_smooth_normals`

### Examples

* Added `rapier_collisions` example
* Added `anchors` example

## 0.1.0

First version
