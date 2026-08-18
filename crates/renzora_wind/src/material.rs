//! `WindSwayMaterial` — StandardMaterial plus a wind-animated vertex stage.
//!
//! Bevy's `StandardMaterial` has no vertex hook, so a mesh using it is rigid by
//! construction. This rides on top of it as a `MaterialExtension`, replacing
//! only the vertex stage (forward *and* prepass, which is also what shadows go
//! through) and leaving the entire PBR fragment path — alpha masking, IBL,
//! shadows, fog, transmission — exactly as it was. A leaf card keeps its cutout
//! and its lighting and gains motion.
//!
//! # Why the parameters are per-material and not global
//!
//! The obvious design is one global wind uniform every shader reads. There is
//! nowhere to put it: the mesh-view bind group is Bevy's, and adding a binding
//! to it restructures a layout shared by every PBR draw in the engine (the
//! failure mode this codebase has already been bitten by twice). So the wind
//! rides in the material's own bind group instead.
//!
//! That would be expensive if it meant rewriting every material every frame —
//! `Assets::get_mut` marks the asset modified, which rebuilds its bind group.
//! It doesn't, because the *animation* is driven by `globals.time` inside the
//! shader. The uniform only carries the slowly-varying authored wind, so it is
//! written when the wind actually changes and not otherwise: a still scene
//! costs zero uploads while still moving on screen.

use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{
    ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

use renzora::WindState;

use crate::WindSway;

pub(crate) const FORWARD_VERTEX: &str = "embedded://renzora_wind/wind_sway.wgsl";
pub(crate) const PREPASS_VERTEX: &str = "embedded://renzora_wind/wind_sway_prepass.wgsl";

/// The wind uniform, mirrored 1:1 by `WindParams` in `wind_common.wgsl`.
///
/// Packed into `vec4`s rather than named scalars deliberately: the WGSL side is
/// hand-written, and `std140` scalar padding is the classic place for a silent
/// layout drift that shows up as garbage in one field on one backend. Four-wide
/// members have no padding to get wrong.
#[derive(ShaderType, Reflect, Debug, Clone, Default, PartialEq)]
pub struct WindParams {
    /// xy = unit travel direction, z = sustained strength (1.0 at reference
    /// wind speed), w = gust depth.
    pub dir_strength: Vec4,
    /// x = gusts/sec, y = turbulence, z = mesh response, w = mesh flutter.
    pub gust_turb: Vec4,
    /// x = sway amplitude (m), y = fallback pivot height (m), zw unused.
    pub misc: Vec4,
}

impl WindParams {
    /// Combine the world wind with one mesh's response curve.
    pub fn build(wind: &WindState, sway: &WindSway) -> Self {
        let enabled = sway.enabled;
        Self {
            dir_strength: Vec4::new(
                wind.direction.x,
                wind.direction.y,
                if enabled { wind.strength01() } else { 0.0 },
                wind.gust_strength,
            ),
            gust_turb: Vec4::new(
                wind.gust_frequency,
                wind.turbulence,
                sway.response,
                sway.flutter,
            ),
            misc: Vec4::new(sway.amplitude, sway.pivot_height, 0.0, 0.0),
        }
    }
}

/// Extension half of [`WindSwayMaterial`]. Binding 100 — StandardMaterial
/// reserves 0–99 by Bevy convention.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct WindSwayExt {
    #[uniform(100)]
    pub wind: WindParams,
}

impl MaterialExtension for WindSwayExt {
    fn vertex_shader() -> ShaderRef {
        FORWARD_VERTEX.into()
    }

    /// Also the shadow pass: Bevy renders shadow maps through `PrepassPipeline`,
    /// so this is what makes a swaying tree cast a swaying shadow.
    fn prepass_vertex_shader() -> ShaderRef {
        PREPASS_VERTEX.into()
    }

    /// The deferred path shares the prepass vertex layout, so it gets the same
    /// shader — otherwise a deferred camera would render the geometry rigid
    /// while a forward one swayed it.
    fn deferred_vertex_shader() -> ShaderRef {
        PREPASS_VERTEX.into()
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        _descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Nothing to specialize, and in particular the vertex buffer layout is
        // left strictly alone — overriding it here breaks the prepass, which
        // builds a different attribute set (uv_b at location 2 rather than 3)
        // than the forward pass does.
        Ok(())
    }
}

/// A `StandardMaterial` whose vertices are displaced by the world wind.
pub type WindSwayMaterial = ExtendedMaterial<StandardMaterial, WindSwayExt>;
