//! Which meshes render as virtual geometry.
//!
//! A mesh does when it came out of a GLB that has an `.rvg` beside it, that
//! file has an entry for its primitive, and its material is one the current
//! renderer can shade. Everything else renders exactly as before, so a
//! project with no `.rvg` files at all is unaffected.
//!
//! The mesh keeps its `Mesh3d`. Physics builds colliders from it, picking casts
//! against it, Solari traces it and Bevy's shadow passes still draw it. What
//! changes is its visibility class: the `Mesh3d` entry is swapped for
//! [`VirtualGeometry`], which takes the entity out of Bevy's camera passes
//! (opaque, prepass, deferred) and leaves it to this crate's renderer, while
//! every other system that looks at the mesh still sees it. Removing the
//! component swaps the class back, so falling back is one `remove`.

use std::any::TypeId;
use std::collections::HashMap;

use bevy::asset::io::AssetSourceId;
use bevy::asset::AssetPath;
use bevy::camera::visibility::{VisibilityClass, VisibilityRange};
use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::mesh::morph::MeshMorphWeights;
use bevy::prelude::*;
use bevy::tasks::{block_on, futures_lite::future, IoTaskPool, Task};
use renzora::TracedGeometry;
use renzora_rvg::RvgFile;

use crate::material::VgMaterial;

/// A mesh drawn by the virtual geometry renderer: entry `mesh` of `file`.
#[derive(Component, Clone, Debug)]
#[component(on_add = take_from_camera_passes, on_remove = return_to_camera_passes)]
pub struct VirtualGeometry {
    pub file: Handle<RvgFile>,
    pub mesh: u32,
    /// The `Mesh3d` this stands in for. A different handle means the mesh was
    /// replaced (edited, swapped by a script) and this no longer matches it.
    pub source: AssetId<Mesh>,
}

pub use renzora::StandardGeometry;

/// Global switch, read every frame. Off reverts every converted mesh to
/// ordinary geometry; on converts them back.
#[derive(Resource, Clone, Debug)]
pub struct VirtualGeometrySettings {
    pub enabled: bool,
    /// How far, in pixels, a simplified cluster may deviate from the full
    /// detail surface before a finer one is drawn. One pixel is visually
    /// lossless; raising it trades detail for speed.
    pub lod_error_pixels: f32,
    /// GPU memory, in megabytes, the streamed cluster data may take. From the
    /// project's `[rendering] virtual_geometry_budget_mb`.
    pub budget_mb: u32,
}

/// The streaming budget when the project does not set one.
pub const DEFAULT_BUDGET_MB: u32 = 512;

impl Default for VirtualGeometrySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            lod_error_pixels: 1.0,
            budget_mb: DEFAULT_BUDGET_MB,
        }
    }
}

fn take_from_camera_passes(mut world: DeferredWorld, ctx: HookContext) {
    if let Some(mut class) = world.get_mut::<VisibilityClass>(ctx.entity) {
        for id in class.iter_mut() {
            if *id == TypeId::of::<Mesh3d>() {
                *id = TypeId::of::<VirtualGeometry>();
            }
        }
    }
}

fn return_to_camera_passes(mut world: DeferredWorld, ctx: HookContext) {
    if let Some(mut class) = world.get_mut::<VisibilityClass>(ctx.entity) {
        for id in class.iter_mut() {
            if *id == TypeId::of::<VirtualGeometry>() {
                *id = TypeId::of::<Mesh3d>();
            }
        }
    }
}

/// Marks a mesh entity as looked at, so the per-frame scan skips it. Holds the
/// mesh it looked at: a new `Mesh3d` handle is looked at again.
#[derive(Component)]
pub(crate) struct Checked(AssetId<Mesh>);

/// What is known about the `.rvg` beside one GLB.
enum FileState {
    /// Asking the asset source whether the file exists. Asked before loading
    /// it, because loading a file that is not there logs an error, and most
    /// models in most projects have no `.rvg`.
    Probing(Task<bool>),
    Loading(Handle<RvgFile>),
    Ready(Handle<RvgFile>),
    Missing,
}

/// One entry per GLB that any mesh came from, keyed by its asset path.
#[derive(Resource, Default)]
pub(crate) struct VgFiles {
    by_model: HashMap<AssetPath<'static>, FileState>,
}

impl VgFiles {
    /// Forget everything about a model, so its `.rvg` is looked for again: the
    /// converter calls this after writing a new one.
    pub fn forget(&mut self, model: &AssetPath<'static>) {
        self.by_model.remove(model);
    }
}

/// The model file and glTF primitive a mesh handle was loaded as, from its
/// label (`Mesh3/Primitive0`). A model is a `.glb`/`.gltf` (with an `.rvg`
/// beside it) or an `.rvg` itself, whose scene carries the same labels.
fn primitive_of(path: &AssetPath) -> Option<(AssetPath<'static>, u32, u32)> {
    let label = path.label()?;
    let rest = label.strip_prefix("Mesh")?;
    let (mesh, primitive) = rest.split_once("/Primitive")?;
    let model = path.without_label().into_owned();
    // The last extension, not `get_full_extension`: that one starts at the
    // first dot, and `Vanguard By T. Choonyung.glb` would read as
    // " Choonyung.glb".
    let ext = model.path().extension()?.to_str()?;
    if !["glb", "gltf", "rvg"].iter().any(|e| ext.eq_ignore_ascii_case(e)) {
        return None;
    }
    Some((model, mesh.parse().ok()?, primitive.parse().ok()?))
}

/// The `.rvg` file holding a model's clusters: beside a `.glb`, or the model
/// itself.
fn rvg_file(model: &AssetPath<'static>) -> AssetPath<'static> {
    let path = model.path().with_extension("rvg");
    AssetPath::from_path_buf(path).with_source(model.source().clone_owned())
}

/// The clusters themselves: every `.rvg` loads as a model, with its virtual
/// geometry as the `Clusters` sub-asset (`renzora_rvg::model`).
fn rvg_path(model: &AssetPath<'static>) -> AssetPath<'static> {
    rvg_file(model).with_label(renzora_rvg::CLUSTERS_LABEL)
}

/// Advance every model's file state by what has happened since last frame.
pub(crate) fn poll_files(
    mut files: ResMut<VgFiles>,
    asset_server: Res<AssetServer>,
) {
    for (model, state) in files.by_model.iter_mut() {
        match state {
            FileState::Probing(task) => {
                if let Some(exists) = block_on(future::poll_once(task)) {
                    *state = if exists {
                        FileState::Loading(asset_server.load(rvg_path(model)))
                    } else {
                        FileState::Missing
                    };
                }
            }
            FileState::Loading(handle) => {
                if asset_server.is_loaded_with_dependencies(handle.id()) {
                    *state = FileState::Ready(handle.clone());
                } else if asset_server.load_state(handle.id()).is_failed() {
                    warn!(
                        "[virtual geometry] {} could not be read; the model renders as \
                         ordinary geometry. Rebuild its virtual geometry to fix this",
                        rvg_path(model)
                    );
                    *state = FileState::Missing;
                }
            }
            FileState::Ready(_) | FileState::Missing => {}
        }
    }
}

fn probe(asset_server: &AssetServer, model: &AssetPath<'static>) -> FileState {
    let server = asset_server.clone();
    let rvg = rvg_file(model);
    FileState::Probing(IoTaskPool::get().spawn(async move {
        let source: AssetSourceId = rvg.source().clone_owned();
        let Ok(source) = server.get_source(source) else {
            return false;
        };
        source.reader().read(rvg.path()).await.is_ok()
    }))
}

/// Convert newly seen meshes whose `.rvg` is ready. Which materials qualify is
/// decided per material type (see `material.rs`); a mesh whose material type
/// is not registered has no `VgMaterial` and is never looked at.
#[allow(clippy::type_complexity)]
pub(crate) fn attach(
    mut commands: Commands,
    settings: Res<VirtualGeometrySettings>,
    mut files: ResMut<VgFiles>,
    asset_server: Res<AssetServer>,
    rvg_files: Res<Assets<RvgFile>>,
    candidates: Query<
        (Entity, &Mesh3d, &VgMaterial, Option<&Checked>, Option<&Name>, Option<&FullDetail>),
        (
            Without<VirtualGeometry>,
            // Blend shapes (a face rig's) move vertices the cluster data has
            // no record of. Skinning is fine: `skin.wesl` deforms clusters.
            Without<MeshMorphWeights>,
            // A mesh in a distance-LOD band belongs to the older LOD system,
            // which swaps whole meshes by distance. Converting one would draw
            // it at every distance on top of its LOD siblings.
            Without<VisibilityRange>,
        ),
    >,
    opted_out: Query<(), With<StandardGeometry>>,
    parents: Query<&ChildOf>,
) {
    if !settings.enabled {
        return;
    }
    let mut kept: HashMap<(AssetPath<'static>, &'static str), (u32, Option<String>)> = HashMap::new();
    for (entity, mesh, material, checked, name, full_detail) in &candidates {
        // A mesh drawn at full detail in the ordinary way (`full_detail`) is
        // still its proxy as far as converting goes: that is the mesh the
        // model names, and the one put back when virtual geometry takes over.
        let identity = full_detail.map_or(mesh.id(), |f| f.proxy.id());
        if checked.is_some_and(|c| c.0 == identity) {
            continue;
        }
        let Some((model, mesh_index, primitive_index)) =
            asset_server.get_path(identity).as_ref().and_then(primitive_of)
        else {
            commands.entity(entity).try_insert(Checked(identity));
            continue;
        };
        let state = files
            .by_model
            .entry(model.clone())
            .or_insert_with(|| probe(&asset_server, &model));
        let handle = match state {
            // Not known yet: look again next frame, without marking it checked.
            FileState::Probing(_) | FileState::Loading(_) => continue,
            FileState::Missing => {
                commands.entity(entity).try_insert(Checked(identity));
                continue;
            }
            FileState::Ready(handle) => handle.clone(),
        };
        let opted = std::iter::once(entity)
            .chain(parents.iter_ancestors(entity))
            .any(|e| opted_out.contains(e));
        let found = rvg_files
            .get(&handle)
            .and_then(|file| file.find(mesh_index, primitive_index));
        commands.entity(entity).try_insert(Checked(identity));
        if opted {
            continue;
        }
        let reason = match (found, material.surface) {
            (Some(found), Some(_)) => {
                if let Some(full_detail) = full_detail {
                    commands
                        .entity(entity)
                        .try_insert(Mesh3d(full_detail.proxy.clone()))
                        .try_remove::<FullDetail>();
                }
                commands.entity(entity).try_insert(VirtualGeometry {
                    file: handle,
                    mesh: found as u32,
                    source: identity,
                });
                // A model stored as `.rvg` has only its proxy as `Mesh3d`, so a
                // ray tracer must trace the full detail instead (see
                // `traced_geometry`). A `.glb`'s own mesh is its full detail.
                if is_rvg_model(&model) {
                    commands.entity(entity).try_insert(TracedGeometry::default());
                }
                continue;
            }
            (None, _) => NOT_IN_FILE,
            // Mid-swap, not unsupported: the new material is classified next
            // frame, and that look is the one worth reporting.
            (Some(_), None) if material.removed => continue,
            (Some(_), None) => MATERIAL_UNSUPPORTED,
        };
        let entry = kept.entry((model, reason)).or_insert((0, None));
        entry.0 += 1;
        if entry.1.is_none() {
            entry.1 = Some(name.map_or_else(|| format!("{entity}"), |n| n.as_str().to_string()));
        }
    }
    // A model that has an `.rvg` is meant to be virtual geometry, so its
    // meshes that are not say why in the Console, one line per model and
    // reason: otherwise the only sign is a debug view that leaves them grey.
    for ((model, reason), (count, example)) in kept {
        let example = example.unwrap_or_default();
        renzora::console_log::console_info(
            "Virtual Geometry",
            format!("{model}: {count} mesh(es) stay ordinary geometry ({example} among them): {reason}"),
        );
    }
}

const NOT_IN_FILE: &str = "the .rvg has no entry for them (points, lines or morph targets)";
const MATERIAL_UNSUPPORTED: &str = "their material cannot be drawn as virtual geometry \
    (transmissive, alpha to coverage, a particle graph, or a plugin material type not registered for it)";

/// Hand meshes back to ordinary rendering when they stop qualifying: the
/// mesh or material changed under them, the model was opted out, or virtual
/// geometry was switched off.
#[allow(clippy::type_complexity)]
pub(crate) fn detach(
    mut commands: Commands,
    settings: Res<VirtualGeometrySettings>,
    converted: Query<(
        Entity,
        &VirtualGeometry,
        Option<&Mesh3d>,
        Option<&VgMaterial>,
        Has<MeshMorphWeights>,
        Has<VisibilityRange>,
    )>,
    opted_out: Query<(), With<StandardGeometry>>,
    parents: Query<&ChildOf>,
) {
    for (entity, vg, mesh, material, morphed, lod_band) in &converted {
        let still = settings.enabled
            && !morphed
            && !lod_band
            && mesh.is_some_and(|m| m.id() == vg.source)
            // Not classified yet (its first frame) counts as still fine; a
            // material that was removed or swapped is classified unsupported.
            && material.is_none_or(|m| m.surface.is_some())
            && !std::iter::once(entity)
                .chain(parents.iter_ancestors(entity))
                .any(|e| opted_out.contains(e));
        if !still {
            // Dropping `Checked` lets `attach` look at it again, which is what
            // makes switching the setting back on, or restoring a material,
            // convert it again.
            commands
                .entity(entity)
                .try_remove::<(VirtualGeometry, Checked)>();
        }
    }
}

/// Sent after an `.rvg` is written for a model, by the editor's converter.
///
/// A model whose `.rvg` was missing when its meshes were first seen is
/// remembered as missing, and a file appearing later is not an asset event
/// (nothing had loaded it), so this is how the renderer hears about it.
#[derive(Message, Clone, Debug)]
pub struct RvgUpdated {
    /// The model's asset path: `models/bistro/Bistro.glb`.
    pub model: AssetPath<'static>,
}

/// Follow the project's `[rendering] virtual_geometry` switch.
pub(crate) fn follow_project(
    project: Option<Res<renzora::core::CurrentProject>>,
    mut settings: ResMut<VirtualGeometrySettings>,
) {
    let Some(project) = project.filter(|p| p.is_changed()) else {
        return;
    };
    let wanted = project.config.rendering.virtual_geometry && !cfg!(target_arch = "wasm32");
    if settings.enabled != wanted {
        settings.enabled = wanted;
    }
    let budget = project.config.rendering.virtual_geometry_budget_mb;
    if settings.budget_mb != budget {
        settings.budget_mb = budget;
    }
}

/// Re-examine meshes after a model's `.rvg` changed, or the setting turned on.
///
/// Only what the change touches is redone. A mesh converted from a file that
/// was rebuilt goes back through `attach`, since the new file may number its
/// primitives differently; a mesh that found no file before is looked at again
/// in case it now has one. Every other converted mesh is left alone, which is
/// what keeps a scene from flickering while its models finish loading one by
/// one.
#[allow(clippy::type_complexity)]
pub(crate) fn react_to_changes(
    mut commands: Commands,
    settings: Res<VirtualGeometrySettings>,
    mut files: ResMut<VgFiles>,
    asset_server: Res<AssetServer>,
    mut asset_events: MessageReader<AssetEvent<RvgFile>>,
    mut updated: MessageReader<RvgUpdated>,
    meshes: Query<(Entity, Option<&VirtualGeometry>), Or<(With<Checked>, With<VirtualGeometry>)>>,
) {
    let reloaded: Vec<AssetId<RvgFile>> = asset_events
        .read()
        .filter_map(|e| match e {
            AssetEvent::Modified { id } => Some(*id),
            _ => None,
        })
        .collect();
    let mut rewritten: Vec<AssetPath<'static>> = Vec::new();
    for event in updated.read() {
        files.forget(&event.model);
        rewritten.push(rvg_path(&event.model));
    }
    let turned_on = settings.is_changed() && settings.enabled;
    if reloaded.is_empty() && rewritten.is_empty() && !turned_on {
        return;
    }
    for (entity, converted) in &meshes {
        match converted {
            Some(vg) => {
                let stale = reloaded.contains(&vg.file.id())
                    || asset_server
                        .get_path(vg.file.id())
                        .is_some_and(|p| rewritten.contains(&p));
                if stale {
                    commands
                        .entity(entity)
                        .try_remove::<(VirtualGeometry, Checked)>();
                }
            }
            None => {
                commands.entity(entity).try_remove::<Checked>();
            }
        }
    }
}

/// The renderer is installed: virtual geometry can draw meshes in this app.
#[derive(Resource)]
pub(crate) struct RendererActive;

/// A mesh of a model stored as `.rvg`, drawn the ordinary way at full detail.
///
/// Such a model's scene holds only each converted primitive's proxy (see
/// `renzora_rvg::bake::build_model`): the full detail is the clusters. While
/// virtual geometry draws the mesh, the proxy is right, being what shadows,
/// physics and picking read. When it does not (switched off, opted out, a
/// material it cannot shade, no renderer at all), the mesh would be drawn
/// coarse, so its `Mesh3d` is swapped for the full detail rebuilt from the
/// clusters, and `proxy` keeps the original for when virtual geometry takes
/// it back.
#[derive(Component)]
pub(crate) struct FullDetail {
    pub proxy: Handle<Mesh>,
}

/// Full-detail meshes already rebuilt, by clusters file and mesh, so a model
/// placed many times rebuilds each mesh once.
#[derive(Resource, Default)]
pub(crate) struct FullDetailMeshes(HashMap<(AssetId<RvgFile>, u32), Handle<Mesh>>);

/// Draw the full detail of every `.rvg` model mesh virtual geometry will not.
///
/// Virtual geometry has decided not to draw a mesh when it is switched off or
/// not installed, when it looked at the mesh and declined (`Checked` without
/// `VirtualGeometry`), or when the mesh's material type was never registered
/// for it (no `MeshMaterial3d` of a registered type, so it is never looked at).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn full_detail(
    mut commands: Commands,
    settings: Res<VirtualGeometrySettings>,
    active: Option<Res<RendererActive>>,
    types: Option<Res<crate::material::VgMaterialTypes>>,
    asset_server: Res<AssetServer>,
    rvg_files: Res<Assets<RvgFile>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut built: ResMut<FullDetailMeshes>,
    components: &bevy::ecs::component::Components,
    candidates: Query<
        (Entity, &Mesh3d, Has<Checked>, &bevy::ecs::archetype::Archetype),
        (Without<VirtualGeometry>, Without<FullDetail>),
    >,
) {
    // The registered `MeshMaterial3d<M>` components, by id. Read from each
    // entity's archetype rather than its components: Bevy keeps resources as
    // components too, so a query reading every component would conflict with
    // the mesh assets written here.
    let registered: Vec<bevy::ecs::component::ComponentId> = types
        .as_ref()
        .map(|t| t.1.iter().filter_map(|&type_id| components.get_id(type_id)).collect())
        .unwrap_or_default();
    for (entity, mesh, checked, archetype) in &candidates {
        let Some((model, mesh_index, primitive_index)) =
            asset_server.get_path(mesh.id()).as_ref().and_then(primitive_of)
        else {
            continue;
        };
        // Only a model stored as `.rvg` has a proxy for a mesh; a `.glb`
        // beside an `.rvg` holds its own full detail.
        if !is_rvg_model(&model) {
            continue;
        }
        let registered = registered.iter().any(|&id| archetype.contains(id));
        let declined = active.is_none() || !settings.enabled || checked || !registered;
        if !declined {
            continue;
        }
        let clusters: Handle<RvgFile> = asset_server.load(rvg_path(&model));
        let Some(file) = rvg_files.get(&clusters) else {
            continue;
        };
        let proxy = mesh.0.clone();
        let Some(index) = file.find(mesh_index, primitive_index) else {
            // Not converted (points, lines, morph targets): its scene
            // geometry is already its full detail.
            commands.entity(entity).try_insert(FullDetail { proxy });
            continue;
        };
        let full = built
            .0
            .entry((clusters.id(), index as u32))
            .or_insert_with(|| meshes.add(full_detail_mesh(&file.meshes[index])))
            .clone();
        commands.entity(entity).try_insert((FullDetail { proxy }, Mesh3d(full)));
    }
}

fn is_rvg_model(model: &AssetPath) -> bool {
    model.path().extension().is_some_and(|e| e.eq_ignore_ascii_case("rvg"))
}

/// Full-detail meshes built for a ray tracer, kept only while one wants them.
#[derive(Resource, Default)]
pub(crate) struct TracedMeshes(HashMap<(AssetId<RvgFile>, u32), Handle<Mesh>>);

/// Fill in [`TracedGeometry`] on `.rvg` model meshes drawn as virtual
/// geometry, while a ray tracer asks for it, and empty it again after.
///
/// Built only on request because a full-detail mesh costs what the model's
/// ordinary meshes would have, in system and GPU memory, and nothing but the
/// ray tracer needs it while virtual geometry draws the detail. A mesh that
/// stops being virtual geometry loses its `TracedGeometry`: `full_detail` then
/// makes its `Mesh3d` the full detail, which is traced like any mesh.
#[allow(clippy::type_complexity)]
pub(crate) fn traced_geometry(
    mut commands: Commands,
    wanted: Option<Res<renzora::TracedGeometryWanted>>,
    rvg_files: Res<Assets<RvgFile>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut traced: ResMut<TracedMeshes>,
    full_detail: Res<FullDetailMeshes>,
    mut marked: Query<(Entity, &mut TracedGeometry, Option<&VirtualGeometry>)>,
) {
    for (entity, mut geometry, vg) in &mut marked {
        let Some(vg) = vg else {
            commands.entity(entity).try_remove::<TracedGeometry>();
            continue;
        };
        if wanted.is_none() {
            if geometry.0.is_some() {
                geometry.0 = None;
            }
            continue;
        }
        if geometry.0.is_some() {
            continue;
        }
        let Some(file) = rvg_files.get(&vg.file) else { continue };
        let Some(mesh) = file.meshes.get(vg.mesh as usize) else { continue };
        let key = (vg.file.id(), vg.mesh);
        // The same mesh may already exist for drawing (`full_detail`).
        let handle = match full_detail.0.get(&key) {
            Some(handle) => handle.clone(),
            None => traced.0.entry(key).or_insert_with(|| meshes.add(full_detail_mesh(mesh))).clone(),
        };
        geometry.0 = Some(handle);
    }
    if wanted.is_none() && !traced.0.is_empty() {
        traced.0.clear();
    }
}

/// The full-detail triangles of a converted mesh as an ordinary mesh, with
/// every attribute the clusters keep.
fn full_detail_mesh(m: &renzora_rvg::RvgMesh) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
    use renzora_rvg::Attributes;
    let a = m.attributes;
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, m.positions.clone());
    if a.has(Attributes::NORMAL) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals.clone());
    }
    if a.has(Attributes::UV0) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, m.uv0.clone());
    }
    if a.has(Attributes::TANGENT) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, m.tangents.clone());
    }
    if a.has(Attributes::UV1) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, m.uv1.clone());
    }
    if a.has(Attributes::COLOR) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, m.colors.clone());
    }
    if a.has(Attributes::SKIN) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_INDEX, VertexAttributeValues::Uint16x4(m.joints.clone()));
        mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, m.weights.clone());
    }
    mesh.insert_indices(Indices::U32(m.cut_indices(0.0)));
    mesh
}

/// Look again at everything under a removed `StandardGeometry`.
///
/// `attach` marks an opted-out mesh as looked at, like any other, so the
/// per-frame scan skips it; nothing about the mesh itself changes when the
/// opt-out goes, so without this it stayed ordinary geometry until the model
/// reloaded.
pub(crate) fn follow_opt_out(
    mut commands: Commands,
    mut removed: RemovedComponents<StandardGeometry>,
    children: Query<&Children>,
    checked: Query<(), With<Checked>>,
) {
    for root in removed.read() {
        for entity in std::iter::once(root).chain(children.iter_descendants(root)) {
            if checked.contains(entity) {
                commands.entity(entity).try_remove::<Checked>();
            }
        }
    }
}

/// Turn MSAA off on 3D cameras while any mesh is virtual geometry.
///
/// The visibility buffer holds one sample per pixel, so a multisampled view
/// cannot draw converted meshes, and they would be missing from it. Every
/// editor viewport and game camera already runs without MSAA (deferred
/// shading and the atmosphere both require that); this catches the few
/// preview and thumbnail cameras that ask for it. The same trade the engine
/// makes in deferred mode.
pub(crate) fn disable_msaa(
    converted: Query<(), With<VirtualGeometry>>,
    mut cameras: Query<&mut Msaa, With<Camera3d>>,
) {
    if converted.is_empty() {
        return;
    }
    for mut msaa in &mut cameras {
        if *msaa != Msaa::Off {
            *msaa = Msaa::Off;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// A flat grid, big enough to have a proxy coarser than itself.
    fn grid_file(n: u32) -> RvgFile {
        let mut input = renzora_rvg::bake::MeshInput::default();
        for y in 0..=n {
            for x in 0..=n {
                let h = ((x as f32 * 0.7).sin() + (y as f32 * 0.4).cos()) * 0.1;
                input.positions.push([x as f32 / n as f32, h, y as f32 / n as f32]);
                input.normals.push([0.0, 1.0, 0.0]);
            }
        }
        for y in 0..n {
            for x in 0..n {
                let a = y * (n + 1) + x;
                input.indices.extend([a, a + n + 1, a + 1, a + 1, a + n + 1, a + n + 2]);
            }
        }
        RvgFile {
            meshes: vec![std::sync::Arc::new(renzora_rvg::bake::build_mesh(&input).unwrap())],
            ..default()
        }
    }

    /// The full detail a ray tracer traces in place of an `.rvg` model's
    /// proxy exists only while one asks for it, and is every triangle.
    #[test]
    fn full_detail_is_traced_only_while_wanted() {
        let mut world = World::new();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Assets<RvgFile>>();
        world.init_resource::<TracedMeshes>();
        world.init_resource::<FullDetailMeshes>();
        let file = world.resource_mut::<Assets<RvgFile>>().add(grid_file(64));
        let triangles = world.resource::<Assets<RvgFile>>().get(&file).unwrap().meshes[0].original_triangle_count();
        let entity = world
            .spawn((
                VirtualGeometry { file, mesh: 0, source: AssetId::default() },
                TracedGeometry::default(),
            ))
            .id();
        let traced = |world: &World| world.get::<TracedGeometry>(entity).and_then(|t| t.0.clone());

        world.run_system_once(traced_geometry).unwrap();
        assert!(traced(&world).is_none(), "built with nothing asking for it");

        world.insert_resource(renzora::TracedGeometryWanted);
        world.run_system_once(traced_geometry).unwrap();
        let mesh = traced(&world).expect("no full detail while a ray tracer wants it");
        let indices = world.resource::<Assets<Mesh>>().get(&mesh).unwrap().indices().unwrap().len();
        assert_eq!(indices, triangles * 3, "not the full detail");

        world.remove_resource::<renzora::TracedGeometryWanted>();
        world.run_system_once(traced_geometry).unwrap();
        assert!(traced(&world).is_none(), "kept after the ray tracer stopped");
        assert!(world.resource::<TracedMeshes>().0.is_empty());

        // No longer virtual geometry: its `Mesh3d` is the full detail again.
        world.entity_mut(entity).remove::<VirtualGeometry>();
        world.run_system_once(traced_geometry).unwrap();
        world.flush();
        assert!(world.get::<TracedGeometry>(entity).is_none());
    }

    #[test]
    fn reads_the_primitive_from_a_gltf_label() {
        let path = AssetPath::parse("models/bistro/Bistro.glb#Mesh12/Primitive3");
        let (model, mesh, primitive) = primitive_of(&path).unwrap();
        assert_eq!(model, AssetPath::parse("models/bistro/Bistro.glb"));
        assert_eq!((mesh, primitive), (12, 3));
        assert_eq!(rvg_file(&model), AssetPath::parse("models/bistro/Bistro.rvg"));
        assert_eq!(rvg_path(&model), AssetPath::parse("models/bistro/Bistro.rvg#Clusters"));
    }

    /// Taking `StandardGeometry` off a model lets its meshes convert again:
    /// the ones under it lose their looked-at mark.
    #[test]
    fn removing_the_opt_out_looks_again() {
        let mut world = World::new();
        let root = world.spawn(StandardGeometry).id();
        let mesh = world.spawn((ChildOf(root), Checked(AssetId::default()))).id();
        let elsewhere = world.spawn(Checked(AssetId::default())).id();
        let mut schedule = bevy::ecs::schedule::Schedule::default();
        schedule.add_systems(follow_opt_out);
        schedule.run(&mut world);
        world.entity_mut(root).remove::<StandardGeometry>();
        schedule.run(&mut world);
        assert!(!world.entity(mesh).contains::<Checked>(), "the mesh under it was not looked at again");
        assert!(world.entity(elsewhere).contains::<Checked>(), "an unrelated mesh was disturbed");
    }

    #[test]
    fn a_dot_in_the_model_name_is_not_its_extension() {
        let path = AssetPath::parse("models/Vanguard By T. Choonyung/Vanguard By T. Choonyung.glb#Mesh0/Primitive1");
        let (model, mesh, primitive) = primitive_of(&path).expect("a .glb with a dot in its name");
        assert_eq!((mesh, primitive), (0, 1));
        assert_eq!(
            rvg_file(&model),
            AssetPath::parse("models/Vanguard By T. Choonyung/Vanguard By T. Choonyung.rvg")
        );
    }

    /// A model stored as `.rvg` names its primitives the way a GLB does, and
    /// holds its own clusters.
    #[test]
    fn an_rvg_model_holds_its_own_clusters() {
        let path = AssetPath::parse("models/statue/statue.rvg#Mesh2/Primitive1");
        let (model, mesh, primitive) = primitive_of(&path).expect("an .rvg model's primitive");
        assert_eq!((mesh, primitive), (2, 1));
        assert_eq!(rvg_path(&model), AssetPath::parse("models/statue/statue.rvg#Clusters"));
    }

    #[test]
    fn ignores_meshes_not_from_a_model() {
        assert!(primitive_of(&AssetPath::parse("models/a.glb#Material0")).is_none());
        assert!(primitive_of(&AssetPath::parse("shapes/cube.mesh#Mesh0/Primitive0")).is_none());
        assert!(primitive_of(&AssetPath::parse("models/a.glb")).is_none());
    }
}
