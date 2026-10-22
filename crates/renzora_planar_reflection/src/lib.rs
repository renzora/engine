//! Planar reflections: mirrors, windows, polished floors, still water.
//!
//! The engine's equivalent of Unreal's Planar Reflection actor. Put a
//! [`PlanarReflection`] on an entity with a flat mesh and the scene is rendered
//! a second time, from the viewing camera mirrored across the surface, and laid
//! over the surface's own material.
//!
//! # How
//!
//! - [`math`]: the plane from the entity's transform, the mirrored camera pose,
//!   and [`math::ObliqueProjection`], a custom camera projection whose near
//!   plane *is* the mirror (Lengyel's oblique frustum), so nothing behind the
//!   mirror reaches the capture and frustum culling drops it before it costs a
//!   draw.
//! - [`select`]: which (surface, camera) pairs are worth a render this frame:
//!   facing the camera, on screen, within `max_distance`, and among the
//!   nearest [`PlanarReflectionSettings::max_active`].
//! - [`systems`]: one capture rig per pair, a mirror camera rendering HDR into
//!   its own image and an overlay child drawing the same mesh with
//!   [`material::ReflectionOverlayMaterial`], which samples that image in screen
//!   space. See its module docs for which camera counts as "the" camera, and
//!   for why reflections cannot recurse.
//!
//! # Why a second render and not SSR
//!
//! SSR reflects only what is already on screen, and only opaque geometry
//! that wrote depth: turn toward a mirror and the room behind you is missing,
//! and particles, glass and anything transparent never appear. A planar
//! reflection is an ordinary render of the scene, so it has all of those. The
//! price is that render. See `docs/r1-alpha8/rendering/planar-reflections.md`
//! for the numbers.
//!
//! # Cost
//!
//! Per active pair, every `update_every` frames: one scene render at
//! `resolution_scale` of the viewing camera's resolution (0.5 is a quarter of
//! its pixels), including a depth/normal prepass and its own directional-light
//! shadow cascades, which Bevy fits per view. Point and spot shadow maps are
//! per light and shared. A surface that is off screen, behind the camera or out
//! of range costs nothing. The overlay itself is one transparent pass over the
//! surface's pixels, plus twelve extra texture taps where `blur > 0`.
//!
//! The editor half (inspector entry, Add Entity > Mirror) is the nested
//! `renzora_planar_reflection_editor` crate.

pub mod component;
pub mod fit;
pub mod math;
pub mod select;

#[cfg(feature = "render_3d")]
pub mod material;
#[cfg(feature = "render_3d")]
pub mod systems;

#[cfg(test)]
mod tests;

pub use component::{NormalAxis, PlanarReflection, PlanarReflectionSettings};

use bevy::prelude::*;

/// Ordering handle for the planar-reflection pass in `PostUpdate`.
///
/// After transform propagation, because a mirror camera is posed from the
/// viewing camera's final world transform; before Bevy's camera update and
/// frustum pass, so the projection and frustum Bevy computes this frame are
/// the ones this pass just wrote; and before visibility propagation, so an
/// overlay shown or hidden here is shown or hidden this frame.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlanarReflectionSystems;

#[derive(Default)]
pub struct PlanarReflectionPlugin;

impl Plugin for PlanarReflectionPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] PlanarReflectionPlugin");

        // Registered everywhere, a dedicated server included, so a scene that
        // carries a mirror deserializes the same on every build.
        app.register_type::<PlanarReflection>()
            .register_type::<NormalAxis>()
            .init_resource::<PlanarReflectionSettings>();

        if app.world().contains_resource::<renzora::DedicatedServer>() {
            return;
        }

        #[cfg(feature = "render_3d")]
        {
            use bevy::camera::visibility::VisibilitySystems;
            use bevy::transform::TransformSystems;

            bevy::asset::embedded_asset!(app, "shaders/reflection_overlay.wesl");
            app.add_plugins(bevy::pbr::MaterialPlugin::<material::ReflectionOverlayMaterial>::default())
                .init_resource::<systems::ReflectionRigs>()
                .init_resource::<systems::ReflectionPlan>()
                .configure_sets(
                    PostUpdate,
                    PlanarReflectionSystems
                        .after(TransformSystems::Propagate)
                        .before(bevy::camera::CameraUpdateSystems)
                        .before(VisibilitySystems::UpdateFrusta)
                        .before(VisibilitySystems::VisibilityPropagate),
                )
                .add_systems(
                    PostUpdate,
                    (
                        systems::plan_reflections,
                        systems::maintain_rigs,
                        systems::drive_rigs,
                    )
                        .chain()
                        .in_set(PlanarReflectionSystems),
                )
                // Before the set, so a surface whose mesh just loaded is
                // planned against its fitted sheet rather than its bounds.
                .add_systems(PostUpdate, fit::update_plane_fits.before(PlanarReflectionSystems));
        }
    }
}

renzora::add!(PlanarReflectionPlugin, Runtime);
