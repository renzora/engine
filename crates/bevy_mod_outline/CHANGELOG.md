# Changelog

## bevy_mod_outline 0.10.3 (2025-08-24)

### Added
- Added OutlineWarmUp for warming up pipelines.

## bevy_mod_outline 0.10.2 (2025-06-19)

### Fixed
- Fixed origin of depth plane incorrectly calculated from eye vector offset.
- Fixed jump-flood outlines interpreting logical as physical pixels.
- Fixed one frame delay when first rendering an extruded outline.

## bevy_mod_outline 0.10.1 (2025-06-05)

### Fixed
- Fixed outlining skinned meshes when storage buffers enabled.
- Fixed flickering outlines when mesh not rendered with a material.

## bevy_mod_outline 0.10.0 (2025-05-15)

### Added
- Added picking example. (@Lethja)

### Changed
- Updated Bevy dependency to 0.16.

## bevy_mod_outline 0.9.2 (2025-05-06)

### Fixed
- Fixed Z ordering of transparent outlines.
- Fixed jump-flood outlines causing a Metal shader error.

### Changed
- Improved performance of jump-flood outlines.

## bevy_mod_outline 0.9.1 (2025-03-11)

### Added
- Added double-sided outline modes for non-manifold meshes.
- Added OutlineAlphaMask for outlining using a texture.
- Added alpha_mask example.

### Fixed
- Fixed panic if jump flood pipelines were not immediately ready.
- Fixed regression that NoAutomaticBatching was not respected.
- Fixed jump-flood outlines being visible when visible flag was false.

### Changed
- Removed bevy_image workaround and raised Bevy dependency to 0.15.3.

## bevy_mod_outline 0.9.0 (2024-12-12)

### Added
- Added support for the jump flood algorithm.
- Added OutlinePlaneDepth for better depth control.
- Added edge stretching setting to outline normal generator.
- Added feature flags for Reflect impls and scene utilities.
- Added Ease trait impls.

### Fixed
- Fixed outlines being distorted by the viewport aspect ratio.

### Changed
- Updated Bevy dependency to 0.15.
- Migrated from bundles to required components.
- Changed AsyncSceneInheritOutline to be reversible.
- Removed ill-conceived Mix impls.

## bevy_mod_outline 0.8.3 (2024-08-28)

### Fixed
- Fixed panic if a scene entity is despawned straight after spawning.

## bevy_mod_outline 0.8.2 (2024-08-04)

### Fixed
- Fixed outlining skinned meshes when motion vector prepass enabled.
- Fixed TAA jitter affecting outlines.
- Fixed depth precision issue on Metal platforms.

## bevy_mod_outline 0.8.1 (2024-07-21)

### Added
- Added support for NoAutomaticBatching.

### Fixed
- Fixed stencil shader with OutlineMode::RealVertex.
- Fixed using wrong entity data for skinned/morphed meshes.
- Fixed panic if entity despawned between extract and render.

## bevy_mod_outline 0.8.0 (2024-07-09)

### Added
- Added Mix trait impls.
- Added interpolation feature for Lerp trait impls.

### Changed
- Updated Bevy dependency to 0.14.
- Outlines are now drawn in batches.

## bevy_mod_outline 0.7.2 (2024-07-05)

### Added
- Added bloom example.

### Fixed
- Fixed missing reflect attributes and type registration.

## bevy_mod_outline 0.7.1 (2024-06-11)

### Added
- Added Reflect impls to public Components.

### Fixed
- Fixed post-processing effects being discarded when MSAA enabled.

### Changed
- Removed workaround from render_layers example.

## bevy_mod_outline 0.7.0 (2024-03-12)

### Added
- Added AsyncSceneInheritOutline(Plugin).
- Added ui_aa example.

### Changed
- Updated Bevy dependency to 0.13.
- Updated interpolation dependency to 0.3.
- Moved OutlineNode to after tone-mapping in render graph.
- Removed unneeded bevy_ui and interpolation_03 features.

## bevy_mod_outline 0.6.2 (2024-02-10)

### Added
- Added interpolation_03 feature.

## bevy_mod_outline 0.6.1 (2024-02-04)

### Fixed
- Fixed panic if prepasses are enabled.
- Fixed Visibility::Hidden did not affect outlines.

## bevy_mod_outline 0.6.0 (2023-11-08)

### Changed
- Updated Bevy dependency to 0.12.
- Changed outline and stencil properties to be inheritable.

## bevy_mod_outline 0.5.1 (2023-11-03)

### Added
- Added flying_objects example.

### Fixed
- Fixed Z-fighting between overlay and stencil more reliably.

### Changed
- Removed unnecessary extraction step.
- Removed unused vertex attributes bindings.

## bevy_mod_outline 0.5.0 (2023-08-14)

### Changed
- Updated Bevy dependency to 0.11. (@ramirezmike and @zainthemaynnn)
- Removed panic if specialising mesh pipeline fails. (@arjo129)

## bevy_mod_outline 0.4.3 (2023-11-06)

## Fixed
- Fixed Z-fighting between overlay and stencil more reliably (back-port from 0.5.1).

### Changed
- Removed unnecessary extraction step (back-port from 0.5.1).

## bevy_mod_outline 0.4.2 (2023-05-30)

### Fixed
- Fixed failures to propagate ComputedOutlineDepth when needed.
- Fixed Z-fighting between overlay and stencil with OpenGL wgpu back-end.

## bevy_mod_outline 0.4.1 (2023-04-11)

### Fixed
- Fixed panic if mesh has unused vertex indices.
- Fixed panic if the DepthMode hasn't propagated before rendering.

### Changed
- Changed normal weighting to use vertex rather than face normals if available.

## bevy_mod_outline 0.4.0 (2023-03-22)

### Added
- Added enabled flag to OutlineStencil.
- Added hollow example.

### Fixed
- Fixed outline depth propagating when inheritance not enabled.

### Changed
- Updated Bevy dependency to 0.10.
- Changed outline normal generator to use face normals.

## bevy_mod_outline 0.3.5 (2023-03-08)

### Fixed
- Fixed regression breaking SetOutlineDepth::Real.

## bevy_mod_outline 0.3.4 (2023-03-08)

### Fixed
- Fixed texture format error when HDR is enabled.
- Fixed bad clipping of outlines behind the camera.

## bevy_mod_outline 0.3.3 (2023-02-21)

### Fixed
- Fixed SetOutlineDepth being ignored in some cases (@mxgrey).
- Fixed defaulting to SetOutlineDepth::Real in some cases.
- Fixed missing examples from README.

## bevy_mod_outline 0.3.2 (2023-01-15)

### Added
- Added support for (Outline)RenderLayers components (@mxgrey).
- Added render_layers example.

## bevy_mod_outline 0.3.1 (2023-01-05)

### Added
- Added Lerp impls for OutlineVolume and OutlineStencil.
- Added Real mode to SetOutlineDepth enum.

## bevy_mod_outline 0.3.0 (2022-11-22)

### Added
- Added ComputedOutlineDepth, SetOutlineDepth, and InheritOutlineDepth.
- Added offset field to OutlineStencil.
- Added pieces example.

### Removed
- Removed align16 feature.

### Fixed
- Fixed errant debug println in AutoGenerateOutlineNormalsPlugin.

### Changed
- Updated Bevy dependency to 0.9.
- Renamed Outline component to OutlineVolume.

## bevy_mod_outline 0.2.5 (2023-01-14)

### Added
- Added support for (Outline)RenderLayers components (@mxgrey) (back-port from 0.3.2).
- Added render_layers example (back-port from 0.3.2).

### Fixed
- Fixed errant debug println in AutoGenerateOutlineNormalsPlugin (back-port from 0.3.0).

## bevy_mod_outline 0.2.4 (2022-10-12)

### Fixed
- Fixed adding outlines to jointed (skinned) meshes.

## bevy_mod_outline 0.2.3 (2022-08-28)

### Added
- Added AutoGenerateOutlineNormalsPlugin.

## bevy_mod_outline 0.2.2 (2022-08-23)

### Added
- Added a feature flag to control the dependency on bevy_ui (@Shatur).
- Added a feature flag to control uniform struct alignment.

### Fixed
- Fixed compilation on 32-bit platforms.
- Fixed insufficient alignment causing errors with WebGL.

### Changed
- Removed dependency on bevy's monolithic render feature flag (@Shatur).

## bevy_mod_outline 0.2.1 (2022-08-10)

### Added
- Added a constant to expose the outline pass node name.

### Fixed
- Fixed outlines causing UI to disappear when MSAA is enabled.
- Fixed bad derive allowing OutlineBundle to be inserted as a component.

## bevy_mod_outline 0.2.0 (2022-08-09)

### Added
- Added support for outline normals.

### Changed
- Updated Bevy dependency to 0.8.
- Changed from rendering in main pass to separate pass.
- Changed from using asset handles to plain components.

## bevy_mod_outline 0.1.0 (2022-06-14)

- Initial release
