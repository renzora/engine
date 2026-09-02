//! Assets a plugin asked the host to create, and the ABI calls that make them.
//!
//! A plugin never holds a real `Handle` — it gets an index into [`PluginAssets`].
//! That keeps `Handle`'s layout (and `Assets<T>`'s existence) entirely on this
//! side of the boundary, and means an unloaded plugin's assets are still
//! reachable for cleanup by [`super::reload::retire_slot`].
//!
//! Every length arriving here is treated as untrusted. These are raw pointers
//! out of another compilation unit: a bad index is not a soft failure downstream
//! but a GPU read past the vertex buffer that faults the process, and a short
//! image buffer would be uploaded as a full texture, reading past the plugin's
//! heap straight into a transfer.

use bevy::prelude::*;

use crate::sys;

use super::reload::{guard_host, HostCtx};

/// Assets a plugin asked the host to create.
///
/// A plugin never holds a real `Handle` — it gets an index into these. That
/// keeps `Handle`'s layout (and `Assets<T>`'s existence) entirely on this side
/// of the boundary, and means an unloaded plugin's assets are still reachable
/// for cleanup.
#[derive(Resource, Default)]
pub struct PluginAssets {
    /// Images a plugin created, by the handle index it was given.
    pub images: Vec<(usize, Handle<Image>)>,
    /// `(owning slot, handle)`. The owner is what lets a reload drop only its own
    /// meshes — the strong handle here is usually the only one, so dropping it is
    /// what actually frees the GPU memory.
    pub meshes: Vec<(usize, Handle<Mesh>)>,
    pub materials: Vec<(usize, MaterialSlot)>,
}

/// What a plugin's material handle actually refers to.
///
/// Two kinds share one index space so a plugin can pass a handle to `spawn_mesh`
/// without caring which it holds.
#[derive(Clone)]
pub enum MaterialSlot {
    /// Built by `add_material` — a plain PBR material this crate can name.
    ///
    /// Absent without `render_3d`: `StandardMaterial` comes from `bevy_pbr`,
    /// which a 2D-only export strips. The `Custom` arm still works, so a plugin
    /// shipping its own material is unaffected.
    #[cfg(feature = "render_3d")]
    Standard(Handle<StandardMaterial>),
    /// Built by `add_material_shader`. The asset type lives in the render
    /// bridge, which this crate cannot depend on, so applying it goes through
    /// [`CustomMaterialApplier`] — the same indirection `BsnSpawner` uses.
    Custom,
}

/// Attaches a custom plugin material to an entity.
///
/// Registered by the render bridge, because the material's Rust type lives
/// there. Absent in a build with no renderer, in which case a spawn naming a
/// custom material gets no material rather than the wrong one.
#[derive(Resource, Clone, Copy)]
pub struct CustomMaterialApplier(pub fn(&mut World, Entity, usize));

/// Put a resolved [`MaterialSlot`] on an entity.
///
/// Shared by `SpawnMesh` and `SetMaterial` so the two cannot drift — the custom
/// branch in particular is easy to get subtly wrong, and having it written twice
/// is how one of them ends up missing the applier check.
///
/// `what` names the calling command, so the error says which one a plugin got
/// wrong rather than leaving the author to guess.
pub(crate) fn attach_material(
    world: &mut World,
    entity: Entity,
    slot: MaterialSlot,
    index: usize,
    what: &str,
) {
    match slot {
        #[cfg(feature = "render_3d")]
        MaterialSlot::Standard(handle) => {
            if let Ok(mut e) = world.get_entity_mut(entity) {
                e.insert(MeshMaterial3d(handle));
            }
        }
        // The asset's Rust type lives in the render bridge, so attaching it goes
        // back out through the applier the bridge registered. Absent in a build
        // with no renderer, where the entity ends up unmaterialed rather than
        // wrong.
        MaterialSlot::Custom => match world.get_resource::<CustomMaterialApplier>().copied() {
            Some(apply) => (apply.0)(world, entity, index),
            None => error!(
                "[plugin] {what} used a custom material but nothing registered a \
                 `CustomMaterialApplier`"
            ),
        },
    }
}

pub(crate) unsafe extern "C" fn add_mesh(
    host: *mut sys::Host,
    desc: *const sys::MeshDesc,
) -> sys::AssetHandle {
    guard_host("add_mesh", sys::AssetHandle::INVALID, || {
        let ctx = &mut *(host as *mut HostCtx);
        let d = &*desc;
        let s = d.size;
        let mesh: Mesh = match d.primitive {
            sys::Primitive::Cuboid => Cuboid::new(s.x, s.y, s.z).into(),
            sys::Primitive::Sphere => Sphere::new(s.x).into(),
            sys::Primitive::Plane => {
                Plane3d::default().mesh().size(s.x, s.z).into()
            }
            sys::Primitive::Cylinder => Cylinder::new(s.x, s.y).into(),
            sys::Primitive::Capsule => Capsule3d::new(s.x, s.y).into(),
            // The ABI documents `x` = major radius and `y` = minor radius, but
            // `Torus::new` takes (inner, outer). Passing (y, x) made bevy derive
            // major = (x+y)/2 and minor = (x-y)/2, so a plugin got a different
            // torus from the one it asked for. inner = major - minor and
            // outer = major + minor invert bevy's arithmetic exactly.
            sys::Primitive::Torus => Torus::new(s.x - s.y, s.x + s.y).into(),
            // A shape this build cannot make. A visible cube beats a missing
            // mesh, which reads as "the spawn silently failed".
            other => {
                warn!("plugin asked for primitive {} which this build does not have", other.0);
                Cuboid::new(s.x, s.y, s.z).into()
            }
        };
        let Some(mut meshes) = ctx.world.get_resource_mut::<Assets<Mesh>>() else {
            warn!("[plugin] add_mesh ignored — this build has no renderer");
            return sys::AssetHandle::INVALID;
        };
        let handle = meshes.add(mesh);
        let owner = ctx.slot;
        let mut store = ctx
            .world
            .get_resource_or_insert_with(PluginAssets::default);
        store.meshes.push((owner, handle));
        sys::AssetHandle((store.meshes.len() - 1) as u64)
    })
}

/// Validate a plugin image descriptor and turn it into pixel bytes.
///
/// The length check is the whole point: a buffer shorter than the dimensions
/// claim would be uploaded as a full texture, reading past the plugin's heap
/// straight into a GPU transfer. Refused rather than padded.
unsafe fn image_bytes(d: &sys::ImageDesc) -> Option<(Vec<u8>, bevy::render::render_resource::TextureFormat)> {
    use bevy::render::render_resource::TextureFormat;
    if !d.format.is_known() {
        error!("[plugin] image format {} is not one this build has", d.format.0);
        return None;
    }
    if d.width == 0 || d.height == 0 {
        error!("[plugin] image is {}x{}", d.width, d.height);
        return None;
    }
    let expected = d.width as usize * d.height as usize * d.format.bytes_per_pixel();
    if d.data.is_null() || d.data_len != expected {
        error!(
            "[plugin] image is {}x{} {:?}, which needs {expected} bytes; got {}",
            d.width, d.height, d.format, d.data_len
        );
        return None;
    }
    let format = match d.format {
        sys::ImageFormat::Rgba8Srgb => TextureFormat::Rgba8UnormSrgb,
        sys::ImageFormat::Rgba8 => TextureFormat::Rgba8Unorm,
        _ => TextureFormat::R32Float,
    };
    Some((std::slice::from_raw_parts(d.data, d.data_len).to_vec(), format))
}

pub(crate) unsafe extern "C" fn add_image(
    host: *mut sys::Host,
    desc: *const sys::ImageDesc,
) -> sys::AssetHandle {
    guard_host("add_image", sys::AssetHandle::INVALID, || {
        use bevy::image::Image;
        use bevy::render::render_resource::{Extent3d, TextureDimension};
        let ctx = &mut *(host as *mut HostCtx);
        let d = &*desc;
        let Some((data, format)) = image_bytes(d) else {
            return sys::AssetHandle::INVALID;
        };
        let image = Image::new(
            Extent3d {
                width: d.width,
                height: d.height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            format,
            bevy::asset::RenderAssetUsages::default(),
        );
        let Some(mut images) = ctx.world.get_resource_mut::<Assets<Image>>() else {
            warn!("[plugin] add_image ignored — this build has no renderer");
            return sys::AssetHandle::INVALID;
        };
        let handle = images.add(image);
        let owner = ctx.slot;
        let mut store = ctx.world.get_resource_or_insert_with(PluginAssets::default);
        store.images.push((owner, handle));
        sys::AssetHandle((store.images.len() - 1) as u64)
    })
}

/// Validate plugin-supplied geometry and build a `Mesh`.
///
/// Shared by `add_mesh_data` (init) and `MeshSource::write` (per frame) so the
/// two cannot drift — a rule enforced on one path and not the other is worse
/// than no rule, because it makes the failure depend on which call you used.
///
/// Every length is treated as untrusted: these are raw pointers out of another
/// compilation unit, and a bad one is a read off the end of the plugin's heap.
pub(crate) unsafe fn build_mesh_from_desc(
    d: &sys::MeshDataDesc,
    colors: Option<&sys::MeshColors>,
) -> Option<Mesh> {
    if d.positions.is_null() || d.position_count == 0 {
        error!("[plugin] mesh data with no positions");
        return None;
    }
    let positions: Vec<[f32; 3]> = std::slice::from_raw_parts(d.positions, d.position_count)
        .iter()
        .map(|v| [v.x, v.y, v.z])
        .collect();

    // The index bound check is the one that matters. An out-of-range index is
    // not a soft failure downstream — wgpu reads past the vertex buffer and
    // faults the process, taking the editor with it.
    let indices: Option<Vec<u32>> = if d.indices.is_null() || d.index_count == 0 {
        None
    } else {
        let raw = std::slice::from_raw_parts(d.indices, d.index_count);
        if let Some(&bad) = raw.iter().find(|&&i| i as usize >= positions.len()) {
            error!(
                "[plugin] mesh index {bad} is out of range for {} vertices — refusing rather                  than letting the GPU read past the buffer",
                positions.len()
            );
            return None;
        }
        if raw.len() % 3 != 0 {
            error!("[plugin] {} indices is not a whole number of triangles", raw.len());
            return None;
        }
        Some(raw.to_vec())
    };
    if indices.is_none() && !positions.len().is_multiple_of(3) {
        error!(
            "[plugin] {} unindexed positions is not a whole number of triangles",
            positions.len()
        );
        return None;
    }

    // A short attribute array is refused rather than padded. Padding renders
    // with silently wrong shading or UVs on the tail vertices, which is harder
    // to notice than getting nothing.
    let normals: Option<Vec<[f32; 3]>> = if d.normals.is_null() || d.normal_count == 0 {
        None
    } else if d.normal_count != positions.len() {
        error!(
            "[plugin] {} normals for {} vertices",
            d.normal_count,
            positions.len()
        );
        return None;
    } else {
        Some(
            std::slice::from_raw_parts(d.normals, d.normal_count)
                .iter()
                .map(|v| [v.x, v.y, v.z])
                .collect(),
        )
    };
    let uvs: Option<Vec<[f32; 2]>> = if d.uvs.is_null() || d.uv_count == 0 {
        None
    } else if d.uv_count != positions.len() {
        error!("[plugin] {} uvs for {} vertices", d.uv_count, positions.len());
        return None;
    } else {
        Some(std::slice::from_raw_parts(d.uvs, d.uv_count).to_vec())
    };
    let vertex_colors: Option<Vec<[f32; 4]>> = match colors {
        Some(c) if !c.colors.is_null() && c.color_count > 0 => {
            if c.color_count != positions.len() {
                error!(
                    "[plugin] {} vertex colors for {} vertices",
                    c.color_count,
                    positions.len()
                );
                return None;
            }
            Some(std::slice::from_raw_parts(c.colors, c.color_count).to_vec())
        }
        _ => None,
    };

    let mut mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    // UVs before normals: `compute_normals` needs the indices in place but not
    // the UVs, and inserting them first keeps the attribute set complete
    // whichever branch runs below.
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        uvs.unwrap_or_else(|| vec![[0.0, 0.0]; mesh.count_vertices()]),
    );
    if let Some(c) = vertex_colors {
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, c);
    }
    if let Some(indices) = indices {
        mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));
    }
    match normals {
        Some(n) => mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, n),
        // Bevy's own derivation, so a plugin that skips normals gets the same
        // shading an engine crate would have produced by hand.
        None => mesh.compute_normals(),
    }
    Some(mesh)
}

pub(crate) unsafe extern "C" fn add_mesh_data(
    host: *mut sys::Host,
    desc: *const sys::MeshDataDesc,
) -> sys::AssetHandle {
    guard_host("add_mesh_data", sys::AssetHandle::INVALID, || {
        let ctx = &mut *(host as *mut HostCtx);
        let Some(mesh) = build_mesh_from_desc(&*desc, None) else {
            return sys::AssetHandle::INVALID;
        };
        let Some(mut meshes) = ctx.world.get_resource_mut::<Assets<Mesh>>() else {
            warn!("[plugin] add_mesh_data ignored — this build has no renderer");
            return sys::AssetHandle::INVALID;
        };
        let handle = meshes.add(mesh);
        let owner = ctx.slot;
        let mut store = ctx.world.get_resource_or_insert_with(PluginAssets::default);
        store.meshes.push((owner, handle));
        sys::AssetHandle((store.meshes.len() - 1) as u64)
    })
}

pub(crate) unsafe extern "C" fn add_material(
    host: *mut sys::Host,
    desc: *const sys::MaterialDesc,
) -> sys::AssetHandle {
    guard_host("add_material", sys::AssetHandle::INVALID, || {
        // Without bevy_pbr there is no `StandardMaterial` to build. Same shape as
        // the missing-renderer path below: refuse with a warning rather than
        // hand back a handle to something that was never created.
        #[cfg(not(feature = "render_3d"))]
        {
            let _ = (host, desc);
            warn!("[plugin] add_material ignored — this build has no 3D renderer");
            return sys::AssetHandle::INVALID;
        }
        #[cfg(feature = "render_3d")]
        {
        let ctx = &mut *(host as *mut HostCtx);
        let d = &*desc;
        let material = StandardMaterial {
            base_color: Color::linear_rgba(d.color[0], d.color[1], d.color[2], d.color[3]),
            metallic: d.metallic,
            perceptual_roughness: d.roughness,
            emissive: LinearRgba::new(d.emissive[0], d.emissive[1], d.emissive[2], d.emissive[3]),
            ..default()
        };
        let Some(mut materials) = ctx.world.get_resource_mut::<Assets<StandardMaterial>>() else {
            warn!("[plugin] add_material ignored — this build has no renderer");
            return sys::AssetHandle::INVALID;
        };
        let handle = materials.add(material);
        let owner = ctx.slot;
        let mut store = ctx
            .world
            .get_resource_or_insert_with(PluginAssets::default);
        store.materials.push((owner, MaterialSlot::Standard(handle)));
        sys::AssetHandle((store.materials.len() - 1) as u64)
        }
    })
}
