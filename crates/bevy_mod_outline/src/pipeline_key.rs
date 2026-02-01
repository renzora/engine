use std::ops::BitOr;

use bevy::{math::Vec3, pbr::MeshPipelineKey, prelude::*, render::view::Msaa};
use bitfield::{bitfield_bitrange, bitfield_fields};
use wgpu_types::PrimitiveTopology;

use crate::{uniforms::DepthMode, ComputedOutline, TextureChannel};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum PassType {
    Stencil = 1,
    Volume = 2,
    #[cfg(feature = "flood")]
    FloodInit = 3,
}

#[derive(Copy, Clone, Default, PartialEq, Eq, Hash)]
pub(crate) struct RawPipelineKey(u32);
bitfield_bitrange! {struct RawPipelineKey(u32)}

impl RawPipelineKey {
    bitfield_fields! {
        u32;
        // View parameters (0-7)
        msaa_samples_minus_one, set_msaa_samples_minus_one: 2, 0;
        pub hdr_format, set_hdr_format: 3;
        pub motion_vector_prepass, set_motion_vector_prepass: 4;
        // Mesh parameters (8:15)
        primitive_topology_int, set_primitive_topology_int: 10, 8;
        pub morph_targets, set_morph_targets: 11;
        // Entity parameters (16:29)
        depth_mode_int, set_depth_mode_int: 17, 16;
        pub transparent, set_transparent: 18;
        pub vertex_offset_zero, set_vertex_offset_zero: 19;
        pub stencil_vertex_offset_zero, set_stencil_vertex_offset_zero: 20;
        pub plane_offset_zero, set_plane_offset_zero: 21;
        pub double_sided, set_double_sided: 22;
        pub alpha_mask_texture, set_alpha_mask_texture: 23;
        pub alpha_mask_channel_int, set_alpha_mask_channel_int: 25, 24;
        // Derived parameters (30:31)
        pass_type_int, set_pass_type_int: 31, 30;
    }

    pub(crate) fn new() -> Self {
        RawPipelineKey(0)
    }

    pub(crate) fn with_pass_type(mut self, pass_type: PassType) -> Self {
        self.set_pass_type_int(pass_type as u32);
        self
    }

    pub(crate) fn msaa(&self) -> Msaa {
        match self.msaa_samples_minus_one() + 1 {
            x if x == Msaa::Off as u32 => Msaa::Off,
            x if x == Msaa::Sample2 as u32 => Msaa::Sample2,
            x if x == Msaa::Sample4 as u32 => Msaa::Sample4,
            x if x == Msaa::Sample8 as u32 => Msaa::Sample8,
            x => panic!("Invalid value for Msaa: {x}"),
        }
    }

    pub(crate) fn primitive_topology(&self) -> PrimitiveTopology {
        match self.primitive_topology_int() {
            x if x == PrimitiveTopology::PointList as u32 => PrimitiveTopology::PointList,
            x if x == PrimitiveTopology::LineList as u32 => PrimitiveTopology::LineList,
            x if x == PrimitiveTopology::LineStrip as u32 => PrimitiveTopology::LineStrip,
            x if x == PrimitiveTopology::TriangleList as u32 => PrimitiveTopology::TriangleList,
            x if x == PrimitiveTopology::TriangleStrip as u32 => PrimitiveTopology::TriangleStrip,
            x => panic!("Invalid value for PrimitiveTopology: {x}"),
        }
    }

    pub(crate) fn pass_type(&self) -> PassType {
        match self.pass_type_int() {
            x if x == PassType::Stencil as u32 => PassType::Stencil,
            x if x == PassType::Volume as u32 => PassType::Volume,
            #[cfg(feature = "flood")]
            x if x == PassType::FloodInit as u32 => PassType::FloodInit,
            x => panic!("Invalid value for PassType: {x}"),
        }
    }

    pub(crate) fn depth_mode(&self) -> DepthMode {
        match self.depth_mode_int() {
            x if x == DepthMode::Flat as u32 => DepthMode::Flat,
            x if x == DepthMode::Real as u32 => DepthMode::Real,
            x => panic!("Invalid value for DepthMode: {x}"),
        }
    }
}

impl BitOr for RawPipelineKey {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

#[derive(Copy, Clone, Default, PartialEq, Eq, Hash, Deref)]
pub(crate) struct ViewPipelineKey(RawPipelineKey);

impl ViewPipelineKey {
    pub(crate) fn new() -> Self {
        ViewPipelineKey(RawPipelineKey::new())
    }

    pub(crate) fn with_msaa(mut self, msaa: Msaa) -> Self {
        self.0.set_msaa_samples_minus_one(msaa as u32 - 1);
        self
    }

    pub(crate) fn with_hdr_format(mut self, hdr_format: bool) -> Self {
        self.0.set_hdr_format(hdr_format);
        self
    }

    pub(crate) fn with_motion_vector_prepass(mut self, motion_vector_prepass: bool) -> Self {
        self.0.set_motion_vector_prepass(motion_vector_prepass);
        self
    }
}

#[derive(Copy, Clone, Default, PartialEq, Eq, Hash, Deref)]
pub(crate) struct EntityPipelineKey(RawPipelineKey);

impl EntityPipelineKey {
    pub(crate) fn new() -> Self {
        EntityPipelineKey(RawPipelineKey::new())
    }

    pub(crate) fn with_primitive_topology(mut self, primitive_topology: PrimitiveTopology) -> Self {
        self.0.set_primitive_topology_int(primitive_topology as u32);
        self
    }

    pub(crate) fn with_morph_targets(mut self, morph_targets: bool) -> Self {
        self.0.set_morph_targets(morph_targets);
        self
    }

    pub(crate) fn with_transparent(mut self, transparent: bool) -> Self {
        self.0.set_transparent(transparent);
        self
    }

    pub(crate) fn with_depth_mode(mut self, depth_mode: DepthMode) -> Self {
        self.0.set_depth_mode_int(depth_mode as u32);
        self
    }

    pub(crate) fn with_vertex_offset_zero(mut self, vertex_offset_zero: bool) -> Self {
        self.0.set_vertex_offset_zero(vertex_offset_zero);
        self
    }

    pub(crate) fn with_plane_offset_zero(mut self, plane_offset_zero: bool) -> Self {
        self.0.set_plane_offset_zero(plane_offset_zero);
        self
    }

    pub(crate) fn with_stencil_vertex_offset_zero(
        mut self,
        stencil_vertex_offset_zero: bool,
    ) -> Self {
        self.0
            .set_stencil_vertex_offset_zero(stencil_vertex_offset_zero);
        self
    }

    pub(crate) fn with_double_sided(mut self, double_sided: bool) -> Self {
        self.0.set_double_sided(double_sided);
        self
    }

    pub(crate) fn with_alpha_mask_texture(mut self, alpha_mask_texture: bool) -> Self {
        self.0.set_alpha_mask_texture(alpha_mask_texture);
        self
    }

    pub(crate) fn with_alpha_mask_channel(mut self, channel: TextureChannel) -> Self {
        let channel_int = match channel {
            TextureChannel::R => 0,
            TextureChannel::G => 1,
            TextureChannel::B => 2,
            TextureChannel::A => 3,
        };
        self.0.set_alpha_mask_channel_int(channel_int);
        self
    }
}

#[derive(Copy, Clone, Default, PartialEq, Eq, Hash, Deref)]
pub(crate) struct DerivedPipelineKey(RawPipelineKey);

impl DerivedPipelineKey {
    pub(crate) fn new(
        view_key: ViewPipelineKey,
        entity_key: EntityPipelineKey,
        pass_type: PassType,
    ) -> Self {
        DerivedPipelineKey(
            match pass_type {
                PassType::Stencil => {
                    view_key
                        .with_hdr_format(false)
                        .with_motion_vector_prepass(false)
                        .0
                        | entity_key
                            .with_transparent(false)
                            .with_vertex_offset_zero(entity_key.stencil_vertex_offset_zero())
                            .with_stencil_vertex_offset_zero(false)
                            .0
                }
                PassType::Volume => {
                    view_key.0
                        | entity_key
                            .with_alpha_mask_texture(false)
                            .with_alpha_mask_channel(TextureChannel::A)
                            .with_stencil_vertex_offset_zero(false)
                            .0
                }
                #[cfg(feature = "flood")]
                PassType::FloodInit => {
                    entity_key
                        .with_transparent(false)
                        .with_vertex_offset_zero(true)
                        .with_stencil_vertex_offset_zero(false)
                        .with_plane_offset_zero(true)
                        .0
                }
            }
            .with_pass_type(pass_type),
        )
    }
}

impl From<DerivedPipelineKey> for MeshPipelineKey {
    fn from(key: DerivedPipelineKey) -> Self {
        let mut mesh_key = MeshPipelineKey::empty();
        if key.morph_targets() {
            mesh_key |= MeshPipelineKey::MORPH_TARGETS;
        }
        if key.motion_vector_prepass() {
            mesh_key |= MeshPipelineKey::MOTION_VECTOR_PREPASS;
        }
        mesh_key
    }
}

#[derive(Clone, Component, Default)]
pub(crate) struct ComputedOutlineKey(pub(crate) EntityPipelineKey);

#[allow(clippy::type_complexity)]
pub(crate) fn compute_outline_key(
    mut query: Query<
        (&ComputedOutline, &Mesh3d, &mut ComputedOutlineKey),
        Or<(
            Changed<ComputedOutline>,
            Changed<Mesh3d>,
            AssetChanged<Mesh3d>,
        )>,
    >,
    meshes: Res<Assets<Mesh>>,
) {
    for (outline, mesh, mut key) in query.iter_mut() {
        let Some(outline) = outline.0.as_ref() else {
            continue;
        };

        let Some(mesh) = meshes.get(mesh) else {
            continue;
        };
        key.0 = EntityPipelineKey::new()
            .with_primitive_topology(mesh.primitive_topology())
            .with_morph_targets(mesh.morph_targets().is_some())
            .with_transparent(!outline.volume.value.colour.is_fully_opaque())
            .with_depth_mode(outline.mode.value.depth_mode)
            .with_vertex_offset_zero(outline.volume.value.offset == 0.0)
            .with_stencil_vertex_offset_zero(outline.stencil.value.offset == 0.0)
            .with_plane_offset_zero(outline.depth.value.world_plane_offset == Vec3::ZERO)
            .with_double_sided(outline.mode.value.double_sided)
            .with_alpha_mask_texture(outline.alpha_mask.value.texture.is_some())
            .with_alpha_mask_channel(outline.alpha_mask.value.channel);
    }
}
