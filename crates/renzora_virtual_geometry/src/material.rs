//! Which material types virtual geometry can shade, and what it needs to know
//! about each material.
//!
//! The renderer is generic over material types: a type is registered with
//! [`VirtualGeometryAppExt::add_virtual_geometry_material`], giving a function
//! that classifies one material (supported or not, cut-out or not,
//! double-sided or not). A system per registered type keeps a type-erased
//! [`VgMaterial`] on every mesh using that type, and everything downstream
//! (attaching, extraction, pipelines) reads only that. This crate registers
//! `StandardMaterial`; the runtime registers the node-graph material, so the
//! two crates never depend on each other.
//!
//! A registered type's shaders must honour `MESHLET_MESH_MATERIAL_PASS` (see
//! `shaders/resolve.wesl`), and its bind group must start with
//! `StandardMaterial`'s own bindings: the masked raster pass and the prepass
//! read the base colour, cutoff and flags from there.

use std::any::TypeId;
use std::collections::HashSet;

use bevy::asset::UntypedAssetId;
use bevy::prelude::*;
use bevy::render::RenderApp;

use crate::attach::Checked;

/// What the renderer needs to know about a material it can shade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VgSurface {
    /// Alpha-masked: rasterized by the pass that cuts out by alpha.
    pub masked: bool,
    /// Rasterized without back-face culling.
    pub double_sided: bool,
    /// Blended (glass and the like): drawn in Bevy's transparent pass, sorted
    /// with every other transparent object, rather than through the
    /// visibility buffer.
    pub transparent: bool,
}

/// A mesh's material, as virtual geometry sees it. `surface` is `None` for a
/// material it cannot shade, which keeps the mesh ordinary geometry.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct VgMaterial {
    pub id: UntypedAssetId,
    pub surface: Option<VgSurface>,
    /// The material was taken off the mesh. Usually it is being swapped for
    /// another (an imported model's glTF material for its `.material`), and
    /// the new one's classification follows a frame later, so this is not
    /// reported as the mesh's material being unsupported.
    pub removed: bool,
}

/// The registered material types, in both worlds, and (main world) the
/// `MeshMaterial3d<M>` components that carry them: a mesh with none of those
/// is never classified, so never drawn as virtual geometry.
#[derive(Resource, Default, Clone)]
pub(crate) struct VgMaterialTypes(pub HashSet<TypeId>, pub HashSet<TypeId>);

#[derive(Resource)]
struct Classifier<M: Material>(fn(&M) -> Option<VgSurface>);

/// Ordering for the main-world systems: materials are classified before the
/// meshes using them are attached.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VgSystems {
    Classify,
    Attach,
}

pub trait VirtualGeometryAppExt {
    /// Let meshes using material type `M` render as virtual geometry, where
    /// `classify` says a given material can.
    fn add_virtual_geometry_material<M: Material>(
        &mut self,
        classify: fn(&M) -> Option<VgSurface>,
    ) -> &mut Self;
}

impl VirtualGeometryAppExt for App {
    fn add_virtual_geometry_material<M: Material>(
        &mut self,
        classify: fn(&M) -> Option<VgSurface>,
    ) -> &mut Self {
        let type_id = TypeId::of::<M>();
        let mut types = self.world_mut().get_resource_or_insert_with(VgMaterialTypes::default);
        types.0.insert(type_id);
        types.1.insert(TypeId::of::<MeshMaterial3d<M>>());
        if let Some(render_app) = self.get_sub_app_mut(RenderApp) {
            render_app
                .world_mut()
                .get_resource_or_insert_with(VgMaterialTypes::default)
                .0
                .insert(type_id);
        }
        self.insert_resource(Classifier::<M>(classify))
            .add_systems(PostUpdate, classify_meshes::<M>.in_set(VgSystems::Classify))
    }
}

/// The classification `StandardMaterial` gets: opaque, alpha-masked or blended,
/// and not transmissive.
///
/// Transmissive materials refract what is behind them, which Bevy renders in a
/// phase of its own that reads a copy of the opaque scene; they stay ordinary
/// geometry. Alpha to coverage needs MSAA, which views drawing virtual
/// geometry do not have.
pub fn classify_standard(material: &StandardMaterial) -> Option<VgSurface> {
    let (masked, transparent) = match material.alpha_mode {
        AlphaMode::Opaque => (false, false),
        AlphaMode::Mask(_) => (true, false),
        AlphaMode::Blend | AlphaMode::Premultiplied | AlphaMode::Add | AlphaMode::Multiply => (false, true),
        AlphaMode::AlphaToCoverage => return None,
    };
    if material.specular_transmission != 0.0 || material.diffuse_transmission != 0.0 {
        return None;
    }
    Some(VgSurface {
        masked,
        double_sided: material.double_sided || material.cull_mode.is_none(),
        transparent,
    })
}

/// Keep [`VgMaterial`] current on every mesh using `M`.
///
/// A material still loading gets no component yet, so the mesh is looked at
/// again once it has one. A change in what the material allows drops the
/// mesh's `Checked` mark, which is what lets `attach` convert a mesh whose
/// material became supported, and `detach` hand back one whose material
/// stopped being.
fn classify_meshes<M: Material>(
    mut commands: Commands,
    classifier: Res<Classifier<M>>,
    materials: Res<Assets<M>>,
    meshes: Query<(Entity, &MeshMaterial3d<M>, Option<&VgMaterial>)>,
    mut removed: RemovedComponents<MeshMaterial3d<M>>,
    current: Query<&VgMaterial>,
) {
    for (entity, material, existing) in &meshes {
        let Some(asset) = materials.get(&material.0) else {
            continue;
        };
        let info = VgMaterial {
            id: material.id().untyped(),
            surface: (classifier.0)(asset),
            removed: false,
        };
        if existing != Some(&info) {
            commands.entity(entity).try_insert(info).try_remove::<Checked>();
        }
    }
    for entity in removed.read() {
        // Marked unsupported rather than removed: a missing `VgMaterial`
        // means "not classified yet", which `detach` leaves alone. Only this
        // type's entry, so a material swapped for another registered type is
        // not undone (and if it was, its own system writes it again next
        // frame).
        if let Ok(existing) = current.get(entity) {
            if existing.id.type_id() == TypeId::of::<M>() {
                commands
                    .entity(entity)
                    .try_insert(VgMaterial {
                        id: existing.id,
                        surface: None,
                        removed: true,
                    })
                    .try_remove::<Checked>();
            }
        }
    }
}
