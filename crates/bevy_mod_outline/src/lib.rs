#![allow(dead_code)] // Workaround for bevy#20400
//! This crate provides a Bevy plugin, [`OutlinePlugin`], and associated
//! components for rendering outlines around meshes using the vertex extrusion
//! and jump flood methods.
//!
//! Outlines are rendered in a seperate pass following the main 3D pass and
//! using a separate depth buffer. This ensures that outlines are not clipped
//! by non-outline geometry.
//!
//! An outline consists of two parts, a volume and a stencil. The volume
//! will, by itself, cover the original object entirely with the outline
//! colour. The stencil prevents the body of an object from being filled in.
//! Stencils also allows other entities to occlude outlines, otherwise the
//! outline will be drawn on top of them. These parts are controlled by the
//! [`OutlineVolume`] and [`OutlineStencil`] components respectively.
//!
//! The [`OutlineMode`] component specifies the rendering method. Outlines may
//! be flattened into a plane in order to further avoid clipping, or left in
//! real space. The depth of flat outlines can be controlled using the
//! [`OutlinePlaneDepth`] component.
//!
//! Outlines can be inherited from the parent via the [`InheritOutline`]
//! component.
//!
//! Vertex extrusion works best with meshes that have smooth surfaces. To
//! avoid visual artefacts when outlining meshes with hard edges, see the
//! [`OutlineMeshExt::generate_outline_normals`] function and the
//! [`AutoGenerateOutlineNormalsPlugin`].
//!
//! Jump flood support is currently experimental and can be enabled by
//! adding the [`OutlineMode::FloodFlat`] component.

use std::any::TypeId;

use bevy::asset::load_internal_asset;
use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::pbr::{MeshInputUniform, MeshUniform};
use bevy::prelude::*;
use bevy::render::batching::gpu_preprocessing::{self, GpuPreprocessingSupport};
use bevy::render::batching::no_gpu_preprocessing::{
    clear_batched_cpu_instance_buffers, write_batched_instance_buffer, BatchedInstanceBuffer,
};
use bevy::render::extract_component::UniformComponentPlugin;
use bevy::mesh::MeshVertexAttribute;
use bevy::render::render_graph::{EmptyNode, RenderGraphExt, RenderLabel, ViewNodeRunner};
use bevy::render::render_phase::{
    sort_phase_system, AddRenderCommand, BinnedRenderPhasePlugin, DrawFunctions, PhaseItem,
    SortedRenderPhasePlugin,
};
use bevy::render::render_resource::{SpecializedMeshPipelines, VertexFormat};
use bevy::render::renderer::RenderDevice;
use bevy::render::sync_component::SyncComponentPlugin;
use bevy::camera::visibility::{RenderLayers, VisibilitySystems};
use bevy::render::{Render, RenderApp, RenderDebugFlags, RenderSystems};
use bevy::prelude::TransformSystems;
use uniforms::extract_outlines;
use uniforms::AlphaMaskBindGroups;
use uniforms::RenderOutlineInstances;

use crate::msaa::MsaaExtraWritebackNode;
use crate::node::{OpaqueOutline, OutlineNode, StencilOutline, TransparentOutline};
use crate::pipeline::{
    OutlinePipeline, COMMON_SHADER_HANDLE, FRAGMENT_SHADER_HANDLE, OUTLINE_SHADER_HANDLE,
};
use crate::pipeline_key::{compute_outline_key, ComputedOutlineKey};
use crate::queue::{
    check_outline_entities_changed, extract_outline_entities_changed, queue_outline_mesh,
    specialise_outlines, OutlineCache, OutlineEntitiesChanged,
};
use crate::render::DrawOutline;
use crate::uniforms::set_outline_visibility;
use crate::uniforms::{
    prepare_alpha_mask_bind_groups, prepare_outline_instance_bind_group, OutlineInstanceUniform,
};
use crate::view_uniforms::{
    extract_outline_view_uniforms, prepare_outline_view_bind_group, OutlineViewUniform,
};

mod computed;
mod generate;
mod msaa;
mod node;
mod pipeline;
mod pipeline_key;
mod queue;
mod render;
mod uniforms;
mod view_uniforms;

pub use computed::*;
pub use generate::*;

#[cfg(feature = "flood")]
mod flood;

#[cfg(feature = "scene")]
mod scene;
#[cfg(feature = "scene")]
pub use scene::*;

/// Legacy bundles.
#[deprecated(since = "0.9.0", note = "Use required components instead")]
pub mod bundles;

// See https://alexanderameye.github.io/notes/rendering-outlines/

/// The direction to extrude the vertex when rendering the outline.
pub const ATTRIBUTE_OUTLINE_NORMAL: MeshVertexAttribute =
    MeshVertexAttribute::new("Outline_Normal", 1585570526, VertexFormat::Float32x3);

/// Labels for render graph nodes which draw outlines.
#[derive(Copy, Clone, Debug, RenderLabel, Hash, PartialEq, Eq)]
#[non_exhaustive]
pub enum NodeOutline {
    /// This node writes back unsampled post-processing effects to the sampled attachment.
    MsaaExtraWritebackPass,
    /// This node runs the jump flood algorithm for outlines
    FloodPass,
    /// This node runs after the main 3D passes and before the UI pass.
    OutlinePass,
    /// This node marks the end of the outline passes.
    EndOutlinePasses,
}

/// A component for stenciling meshes during outline rendering.
///
/// Stencils are used both to prevent entities with outlines from being
/// covered by their own outline volumes and to allow entities to occlude
/// any outlines behind them.
#[derive(Clone, Component)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
#[cfg_attr(feature = "reflect", reflect(Component, Default))]
pub struct OutlineStencil {
    /// Enable rendering of the stencil
    pub enabled: bool,
    /// Offset of the stencil in logical pixels
    pub offset: f32,
}

impl Default for OutlineStencil {
    fn default() -> Self {
        OutlineStencil {
            enabled: true,
            offset: 0.0,
        }
    }
}

fn lerp_bool(this: bool, other: bool, scalar: f32) -> bool {
    if scalar <= 0.0 {
        this
    } else if scalar >= 1.0 {
        other
    } else {
        this | other
    }
}

macro_rules! impl_lerp {
    ($t:ty, $e:expr) => {
        impl Ease for $t {
            fn interpolating_curve_unbounded(start: Self, end: Self) -> impl Curve<Self> {
                FunctionCurve::new(Interval::UNIT, move |t| $e(&start, &end, t))
            }
        }

        #[cfg(feature = "interpolation")]
        impl interpolation::Lerp for $t {
            type Scalar = f32;

            fn lerp(&self, other: &Self, scalar: &Self::Scalar) -> Self {
                $e(self, other, *scalar)
            }
        }
    };
}

fn lerp_stencil(start: &OutlineStencil, end: &OutlineStencil, t: f32) -> OutlineStencil {
    OutlineStencil {
        enabled: lerp_bool(start.enabled, end.enabled, t),
        offset: start.offset.lerp(end.offset, t),
    }
}

impl_lerp!(OutlineStencil, lerp_stencil);

/// A component for rendering outlines around meshes.
#[derive(Clone, Component, Default)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
#[cfg_attr(feature = "reflect", reflect(Component, Default))]
pub struct OutlineVolume {
    /// Enable rendering of the outline
    pub visible: bool,
    /// Width of the outline in logical pixels
    pub width: f32,
    /// Colour of the outline
    pub colour: Color,
}

fn lerp_volume(start: &OutlineVolume, end: &OutlineVolume, t: f32) -> OutlineVolume {
    OutlineVolume {
        visible: lerp_bool(start.visible, end.visible, t),
        width: start.width.lerp(end.width, t),
        colour: start.colour.mix(&end.colour, t),
    }
}

impl_lerp!(OutlineVolume, lerp_volume);

/// A component for specifying what layer(s) the outline should be rendered for.
#[derive(Component, Clone, PartialEq, Eq, PartialOrd, Ord, Deref, DerefMut, Default)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
#[cfg_attr(feature = "reflect", reflect(Component, Default))]
pub struct OutlineRenderLayers(pub RenderLayers);

impl From<RenderLayers> for OutlineRenderLayers {
    fn from(value: RenderLayers) -> Self {
        OutlineRenderLayers(value)
    }
}

/// A component which specifies how the outline should be rendered.
#[derive(Clone, Component, Default)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
#[cfg_attr(feature = "reflect", reflect(Component, Default))]
#[non_exhaustive]
pub enum OutlineMode {
    /// Vertex extrusion flattened into a billboard. (default)
    #[default]
    ExtrudeFlat,
    /// Vertex extrusion flattened into a double-sided billboard.
    ExtrudeFlatDoubleSided,
    /// Vertex extrusion in real model-space.
    ExtrudeReal,
    // Jump-flood into a billboard.
    #[cfg(feature = "flood")]
    FloodFlat,
    // Jump-flood into a double-sided billboard.
    #[cfg(feature = "flood")]
    FloodFlatDoubleSided,
}

/// A component which controls the depth sorting of flat outlines and stencils.
///
/// By flattening an outline into a plane, we avoid it being partially clipped
/// by other outlines. However, naive positioning of the outline planes can
/// cause an outline to be drawn behind the outline of an object it is actually
/// in front of. This component allows you to control the point in an object's
/// model-space through which the outline plane passes.
///
/// The plane point in model-space is calculated by adding the `model_plane_origin`
/// coordinate to the `model_plane_offset` vector multiplied by the model-space
/// eye vector. The latter component allows you to move the plane towards or away
/// from the camera in a view independent manner.
///
/// This component only affects outlines which have a flat [`OutlineMode`].
/// The plane point will be calculated using the transform of the entity to
/// which this component is attached. When inherited, the already calculated
/// value in world-space will be used by the children so that the parent and
/// child will share the same outline plane.
#[derive(Clone, Component, Default)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
#[cfg_attr(feature = "reflect", reflect(Component, Default))]
pub struct OutlinePlaneDepth {
    /// The point in model-space through which the outline plane passes, before
    /// the view dependent offset is applied.
    pub model_plane_origin: Vec3,
    /// An offset to the plane point multiplied by the model-space eye vector.
    pub model_plane_offset: Vec3,
}

/// A component for inheriting outlines from the parent entity.
#[derive(Clone, Component, Default)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
#[cfg_attr(feature = "reflect", reflect(Component, Default))]
pub struct InheritOutline;

/// The channel of a texture.
#[derive(Copy, Clone, Default)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
#[cfg_attr(feature = "reflect", reflect(Default))]
pub enum TextureChannel {
    R,
    G,
    B,
    #[default]
    A,
}

/// A component for specifying an alpha mask texture.
///
/// The alpha mask is a UV-mapped texture that can be used to determine
/// whether a pixel is part of the shape of the object for outlining
/// purposes. If the value read from the texture is less than the threshold
/// value, the pixel is considered outside the shape and not part of the
/// stencil used to generate the outline.
///
/// The visual effect of this depends on the outline mode being used:
///
/// - For extrusion modes, any masked-off part of the stencil will be filled
///   entirely with the outline colour.
///
/// - For jump-flood modes, any masked-off part of the stencil will be
///   outlined identically to a boundary created with geometry.
#[derive(Clone, Component, Default)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
#[cfg_attr(feature = "reflect", reflect(Component, Default))]
pub struct OutlineAlphaMask {
    /// The texture to use as a mask.
    pub texture: Option<Handle<Image>>,
    /// The channel of the texture to use as a mask.
    pub channel: TextureChannel,
    /// The threshold value above which pixels will be included.
    pub threshold: f32,
}

/// A component for warming up different specialisations of the outline pipeline.
///
/// When animating a property which causes the required pipeline specialisation
/// to change, failure to warm up the required specialisaton in advance may
/// cause the outline to briefly disappear.
#[derive(Clone, Component, Default)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
#[cfg_attr(feature = "reflect", reflect(Component, Default))]
pub struct OutlineWarmUp {
    layers: RenderLayers,
    disabled_stencil: bool,
    disabled_volume: bool,
    transparency: bool,
    vertex_offsets: bool,
}

impl OutlineWarmUp {
    /// Warms up the shaders for the given render layers.
    pub fn with_layers(self, layers: RenderLayers) -> Self {
        let mut s = self.clone();
        s.layers = layers;
        s
    }

    /// Warms up the stencil shader even if the outline stencil is disabled.
    pub fn with_disabled_stencil(self, disabled_stencil: bool) -> Self {
        let mut s = self.clone();
        s.disabled_stencil = disabled_stencil;
        s
    }

    /// Warms up the volume shader even if the outline volume is disabled.
    pub fn with_disabled_volume(self, disabled_volume: bool) -> Self {
        let mut s = self.clone();
        s.disabled_volume = disabled_volume;
        s
    }

    /// Warms up both the opaque and transparent versions of the volume shader.
    pub fn with_transparency(self, transparency: bool) -> Self {
        let mut s = self.clone();
        s.transparency = transparency;
        s
    }

    /// Warms up both the zero and non-zero vertex offset versions of the volume and stencil
    /// shaders.
    pub fn with_vertex_offsets(self, vertex_offset_zero: bool) -> Self {
        let mut s = self.clone();
        s.vertex_offsets = vertex_offset_zero;
        self
    }
}

// This makes `SetMeshBindGroup` work with CPU drawn outlines when GPU pre-processing is enabled
pub(crate) fn add_dummy_phase_buffer<P: PhaseItem + 'static>(
    bibs: &mut gpu_preprocessing::BatchedInstanceBuffers<MeshUniform, MeshInputUniform>,
) {
    let phase_buffer = bibs
        .phase_instance_buffers
        .entry(TypeId::of::<P>())
        .or_default();
    if phase_buffer.data_buffer.is_empty() {
        // An empty buffer will not be bound
        phase_buffer.data_buffer.add();
    }
}

fn add_dummy_phase_buffers(
    mut bibs: ResMut<gpu_preprocessing::BatchedInstanceBuffers<MeshUniform, MeshInputUniform>>,
) {
    add_dummy_phase_buffer::<StencilOutline>(&mut bibs);
    add_dummy_phase_buffer::<OpaqueOutline>(&mut bibs);
    add_dummy_phase_buffer::<TransparentOutline>(&mut bibs);
}

/// Adds support for rendering outlines.
pub struct OutlinePlugin;

impl Plugin for OutlinePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, COMMON_SHADER_HANDLE, "common.wgsl", Shader::from_wgsl);
        load_internal_asset!(
            app,
            OUTLINE_SHADER_HANDLE,
            "outline.wgsl",
            Shader::from_wgsl
        );
        load_internal_asset!(
            app,
            FRAGMENT_SHADER_HANDLE,
            "fragment.wgsl",
            Shader::from_wgsl
        );

        app.add_plugins((
            SyncComponentPlugin::<ComputedOutline>::default(),
            SyncComponentPlugin::<ComputedOutlineKey>::default(),
            UniformComponentPlugin::<OutlineViewUniform>::default(),
            BinnedRenderPhasePlugin::<StencilOutline, OutlinePipeline>::new(
                RenderDebugFlags::empty(),
            ),
            BinnedRenderPhasePlugin::<OpaqueOutline, OutlinePipeline>::new(
                RenderDebugFlags::empty(),
            ),
            SortedRenderPhasePlugin::<TransparentOutline, OutlinePipeline>::new(
                RenderDebugFlags::empty(),
            ),
        ))
        .register_required_components::<OutlineStencil, ComputedOutline>()
        .register_required_components::<OutlineVolume, ComputedOutline>()
        .register_required_components::<InheritOutline, ComputedOutline>()
        .init_resource::<OutlineEntitiesChanged>()
        .add_systems(
            PostUpdate,
            (
                compute_outline
                    .after(TransformSystems::Propagate)
                    .after(VisibilitySystems::VisibilityPropagate),
                compute_outline_key
                    .after(compute_outline),
                check_outline_entities_changed.after(compute_outline_key),
                set_outline_visibility.in_set(VisibilitySystems::CheckVisibility),
            ),
        )
        .sub_app_mut(RenderApp)
        .init_resource::<DrawFunctions<StencilOutline>>()
        .init_resource::<DrawFunctions<OpaqueOutline>>()
        .init_resource::<DrawFunctions<TransparentOutline>>()
        .init_resource::<SpecializedMeshPipelines<OutlinePipeline>>()
        .add_render_command::<StencilOutline, DrawOutline>()
        .add_render_command::<OpaqueOutline, DrawOutline>()
        .add_render_command::<TransparentOutline, DrawOutline>()
        .add_systems(
            ExtractSchedule,
            (
                extract_outline_view_uniforms,
                extract_outlines,
                extract_outline_entities_changed,
            ),
        )
        .add_systems(
            Render,
            msaa::prepare_msaa_extra_writeback_pipelines.in_set(RenderSystems::Prepare),
        )
        .add_systems(
            Render,
            (
                prepare_outline_view_bind_group,
                prepare_outline_instance_bind_group,
                prepare_alpha_mask_bind_groups,
            )
                .in_set(RenderSystems::PrepareBindGroups),
        )
        .add_systems(Render, specialise_outlines.in_set(RenderSystems::PrepareMeshes))
        .add_systems(Render, queue_outline_mesh.in_set(RenderSystems::QueueMeshes))
        .add_systems(
            Render,
            sort_phase_system::<TransparentOutline>.in_set(RenderSystems::PhaseSort),
        )
        .add_systems(
            Render,
            clear_batched_cpu_instance_buffers::<OutlinePipeline>
                .in_set(RenderSystems::Cleanup)
                .after(RenderSystems::Render),
        )
        .add_render_graph_node::<ViewNodeRunner<MsaaExtraWritebackNode>>(
            Core3d,
            NodeOutline::MsaaExtraWritebackPass,
        )
        .add_render_graph_node::<ViewNodeRunner<OutlineNode>>(Core3d, NodeOutline::OutlinePass)
        .add_render_graph_node::<EmptyNode>(Core3d, NodeOutline::EndOutlinePasses)
        // Outlining occurs after tone-mapping...
        .add_render_graph_edges(
            Core3d,
            (
                Node3d::Tonemapping,
                NodeOutline::MsaaExtraWritebackPass,
                NodeOutline::OutlinePass,
                NodeOutline::EndOutlinePasses,
                Node3d::EndMainPassPostProcessing,
            ),
        )
        // ...and before any later anti-aliasing.
        .add_render_graph_edge(Core3d, NodeOutline::EndOutlinePasses, Node3d::Fxaa)
        .add_render_graph_edge(Core3d, NodeOutline::EndOutlinePasses, Node3d::Smaa);

        #[cfg(feature = "reflect")]
        app.register_type::<OutlineStencil>()
            .register_type::<OutlineVolume>()
            .register_type::<OutlineRenderLayers>()
            .register_type::<OutlineMode>()
            .register_type::<OutlineAlphaMask>()
            .register_type::<InheritOutline>();

        #[cfg(feature = "scene")]
        app.init_resource::<AsyncSceneInheritOutlineSystems>();

        #[cfg(feature = "flood")]
        app.add_plugins(flood::FloodPlugin);
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .init_resource::<RenderOutlineInstances>()
            .init_resource::<OutlineCache>()
            .init_resource::<OutlinePipeline>()
            .init_resource::<AlphaMaskBindGroups>();

        let render_device = render_app.world().resource::<RenderDevice>();
        let instance_buffer = BatchedInstanceBuffer::<OutlineInstanceUniform>::new(&render_device.limits());
        render_app.insert_resource(instance_buffer).add_systems(
            Render,
            write_batched_instance_buffer::<OutlinePipeline>
                .in_set(RenderSystems::PrepareResourcesFlush),
        );

        let gpu_preprocessing_support = render_app.world().resource::<GpuPreprocessingSupport>();
        if gpu_preprocessing_support.is_available() {
            render_app.add_systems(
                Render,
                add_dummy_phase_buffers.in_set(RenderSystems::PrepareResourcesCollectPhaseBuffers),
            );
        }
    }
}
