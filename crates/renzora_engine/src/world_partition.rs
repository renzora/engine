//! World partition: a scene split into a grid of square cells that stream in
//! and out around the camera.
//!
//! A cell is an ordinary streamed [`SceneInstance`] with a [`PartitionCell`]
//! on it, its contents saved in their own `.bsn` beside the scene. That reuse
//! is deliberate: a shipped game and editor Play already stream such
//! instances through `scene_stream` (off-thread parse, spawning under a frame
//! budget), so a partitioned scene streams there with no new runtime path.
//!
//! What a partition adds is in this module and in the editor's driver
//! (`renzora_scene::world_partition`):
//!
//! - **Streaming while editing.** Ordinary streamed instances stay expanded in
//!   editor edit mode so they can be authored. A partition exists because a
//!   world can be too big for that, so its cells stream in the editor too,
//!   from the viewport camera. [`partition_streams_in_editor`] is the one
//!   answer every system asks, so the loader that expands instances eagerly
//!   and the driver that streams them can never both claim a cell.
//! - **Ground distance.** Cells are measured on X and Z, like terrain chunks,
//!   so flying high over a city does not unload the street below.
//! - **Assignment by bounds** ([`partition_entities`]). A model is placed in
//!   the cell its *bounds* centre falls in, read from the model file itself
//!   ([`model_file_bounds`]) so partitioning a world does not first require
//!   loading all of it, which is the thing a partition exists to avoid. That
//!   also handles models whose real position is baked into their vertices
//!   with the entity sitting at the origin, the shape city data arrives in.

use bevy::prelude::*;
use renzora_log_api::*;
use renzora_scene_api::{MeshInstanceData, PartitionCell, SceneInstance, WorldPartition};
use renzora_project_api::CurrentProject;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The viewport camera cells stream from while editing, re-exported so the
/// editor's cell driver measures from the same camera terrain streaming does.
pub use renzora_platform_api::editor_camera_pos;

/// The scene's partition settings: the first [`WorldPartition`] found.
pub fn partition_settings(world: &mut World) -> Option<WorldPartition> {
    let mut q = world.query::<&WorldPartition>();
    q.iter(world).next().cloned()
}

/// Whether partition cells stream in the editor right now: the editor is
/// open, not playing, and the scene's partition has `stream_in_editor` on.
///
/// Outside that case cells follow the ordinary streamed-instance rule
/// (stream in a running game, stay expanded while editing). Both the eager
/// expander and the editor's cell driver ask this, so a cell is owned by
/// exactly one of them at a time.
pub fn partition_streams_in_editor(world: &mut World) -> bool {
    if !renzora_platform_api::editor_editing(world) {
        return false;
    }
    partition_settings(world).is_some_and(|p| p.stream_in_editor)
}

/// Run-condition form of [`partition_streams_in_editor`], for systems that
/// should cost nothing in a scene without a partition.
pub fn partition_streams_in_editor_condition(
    session: Option<Res<renzora_project_api::EditorSession>>,
    play: Option<Res<renzora_project_api::PlayModeState>>,
    partitions: Query<&WorldPartition>,
) -> bool {
    let editing = session.is_some_and(|s| s.is_editor())
        && play.is_none_or(|p| p.is_editing());
    editing && partitions.iter().next().is_some_and(|p| p.stream_in_editor)
}

/// Ground-plane distance between two points, the measure cells stream by.
pub fn ground_distance(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

/// Keep every cell's streaming radii equal to the partition's. Cells carry
/// their own copy because a running game streams them as plain
/// [`SceneInstance`]s and reads the radii from there; this keeps the copy
/// from going stale when the partition is edited.
pub fn sync_partition_cell_radii(
    partitions: Query<&WorldPartition, Changed<WorldPartition>>,
    mut cells: Query<&mut SceneInstance, With<PartitionCell>>,
) {
    let Some(partition) = partitions.iter().next() else {
        return;
    };
    let load = partition.load_radius.max(0.0);
    let unload = partition.effective_unload_radius();
    for mut instance in &mut cells {
        if instance.load_radius != load || instance.unload_radius != unload || !instance.streamed {
            instance.load_radius = load;
            instance.unload_radius = unload;
            instance.streamed = true;
        }
    }
}

// ---------------------------------------------------------------------------
// Revealing a cell when it is ready
// ---------------------------------------------------------------------------
//
// A cell's entities arrive in a few frames, but what they draw does not: the
// models instantiate, bind their `.material` files, and then every texture
// loads from disk and uploads to the GPU. Shown at once, a district appears
// as black geometry for seconds while hundreds of textures upload inside a
// handful of frames, and those frames take a few hundred milliseconds each.
//
// So a cell's contents arrive hidden, and are shown when every model under
// them has instantiated, every mesh has its material, and every texture those
// materials sample is loaded. While any cell is waiting, uploads to the GPU
// are spread across frames (`RenderAssetBytesPerFrame`), and the reveal waits
// the few frames that budget needs to finish the cell's textures.

/// GPU upload budget while cells stream in: enough for a dense city block's
/// textures in well under a second, small enough that no single frame stalls.
const STREAM_UPLOAD_BYTES_PER_FRAME: usize = 48 * 1024 * 1024;

/// However long a cell has waited, it is shown after this. A texture that
/// never loads (a missing file) must not keep a district invisible for good.
const REVEAL_TIMEOUT_SECS: f64 = 20.0;

/// On each root of a cell's contents while they wait to be shown. Not
/// reflected, so never saved; the `Visibility` it replaced is restored, and is
/// what gets saved if the cell is written while still waiting
/// ([`reveal_pending_under`]).
#[derive(Component, Clone, Copy, Debug)]
pub struct RevealPending {
    original: Option<Visibility>,
    since: f64,
    /// Frames still to wait once everything is loaded, for the GPU upload
    /// budget to get through this cell's textures. `None` until then.
    upload_frames_left: Option<u32>,
}

/// Hide freshly streamed cell contents until they are ready to be seen.
/// Called by the stream when it reparents a cell's roots under the cell.
pub fn hold_until_ready(world: &mut World, roots: &[Entity]) {
    let now = world
        .get_resource::<Time<Real>>()
        .map_or(0.0, |t| t.elapsed_secs_f64());
    for &root in roots {
        let original = world.get::<Visibility>(root).copied();
        world.entity_mut(root).insert((
            RevealPending { original, since: now, upload_frames_left: None },
            Visibility::Hidden,
        ));
    }
}

fn reveal(world: &mut World, root: Entity) {
    let Some(pending) = world.get::<RevealPending>(root).copied() else {
        return;
    };
    let mut e = world.entity_mut(root);
    e.remove::<RevealPending>();
    match pending.original {
        Some(v) => {
            e.insert(v);
        }
        None => {
            e.remove::<Visibility>();
        }
    }
}

/// Show anything under `entity` still waiting, at once. For writing a cell
/// to its file: what is saved must be the cell as authored, not hidden.
pub fn reveal_pending_under(world: &mut World, entity: Entity) {
    let mut stack = vec![entity];
    while let Some(e) = stack.pop() {
        if world.get::<RevealPending>(e).is_some() {
            reveal(world, e);
            continue;
        }
        if let Some(children) = world.get::<Children>(e) {
            stack.extend(children.iter());
        }
    }
}

/// Run condition: some cell contents are waiting to be shown.
pub fn reveals_pending(pending: Query<(), With<RevealPending>>) -> bool {
    !pending.is_empty()
}

/// What is still keeping a subtree from being shown, and how many bytes of
/// textures it holds (for the upload wait).
#[derive(Default)]
struct Readiness {
    waiting: bool,
    texture_bytes: usize,
}

fn subtree_readiness(world: &World, root: Entity) -> Readiness {
    // Only models and meshes can keep a subtree waiting; a build with neither
    // never writes to `r`.
    #[cfg_attr(not(any(feature = "gltf", feature = "render_3d")), allow(unused_mut))]
    let mut r = Readiness::default();
    #[cfg(feature = "render_3d")]
    let mut seen_images: std::collections::HashSet<bevy::asset::AssetId<Image>> = Default::default();
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        #[cfg(feature = "gltf")]
        {
            // A model is ready once its glTF has instantiated (the binder marks
            // the root `ImportedRoot`) or has failed to load.
            if let Some(m) = world.get::<MeshInstanceData>(e) {
                let failed = world.get::<crate::scene_io::MeshInstanceLoadFailed>(e).is_some();
                let instanced = world.get::<crate::material_binding::ImportedRoot>(e).is_some();
                if m.model_path.is_some() && !failed && !instanced {
                    r.waiting = true;
                    return r;
                }
            }
        }
        #[cfg(feature = "render_3d")]
        {
            if world.get::<Mesh3d>(e).is_some() {
                // Bound to its `.material` file, and that file resolved.
                #[cfg(feature = "gltf")]
                if world.get::<crate::material_binding::MaterialBindingDone>(e).is_none()
                    && world.get::<MeshMaterial3d<StandardMaterial>>(e).is_some()
                {
                    r.waiting = true;
                    return r;
                }
                if world.get::<renzora_material_api::MaterialRef>(e).is_some()
                    && world
                        .get::<renzora_material_api::material_ref::MaterialResolved>(e)
                        .is_none()
                {
                    r.waiting = true;
                    return r;
                }
                if let Some(mat) = world.get::<MeshMaterial3d<StandardMaterial>>(e) {
                    let materials = world.resource::<Assets<StandardMaterial>>();
                    let images = world.resource::<Assets<Image>>();
                    let Some(m) = materials.get(&mat.0) else {
                        r.waiting = true;
                        return r;
                    };
                    for h in [
                        m.base_color_texture.as_ref(),
                        m.normal_map_texture.as_ref(),
                        m.metallic_roughness_texture.as_ref(),
                        m.emissive_texture.as_ref(),
                        m.occlusion_texture.as_ref(),
                    ]
                    .into_iter()
                    .flatten()
                    {
                        match images.get(h) {
                            None => {
                                r.waiting = true;
                                return r;
                            }
                            Some(img) => {
                                if seen_images.insert(h.id()) {
                                    r.texture_bytes += img.data.as_ref().map_or_else(
                                        || {
                                            let s = img.texture_descriptor.size;
                                            (s.width * s.height * 4) as usize
                                        },
                                        |d| d.len(),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Some(children) = world.get::<Children>(e) {
            stack.extend(children.iter());
        }
    }
    r
}

/// Show waiting cell contents that are ready, and keep the upload budget on
/// while any are waiting. Checked a few times a second: a readiness walk over
/// a dense block is a few thousand entities.
pub fn reveal_ready_cells(world: &mut World, mut last_check: Local<f64>) {
    let now = world
        .get_resource::<Time<Real>>()
        .map_or(0.0, |t| t.elapsed_secs_f64());
    let pending: Vec<(Entity, RevealPending)> = {
        let mut q = world.query::<(Entity, &RevealPending)>();
        q.iter(world).map(|(e, p)| (e, *p)).collect()
    };

    // Frames counting down after readiness tick every frame.
    for (root, p) in &pending {
        if let Some(left) = p.upload_frames_left {
            if left == 0 {
                reveal(world, *root);
            } else if let Some(mut rp) = world.get_mut::<RevealPending>(*root) {
                rp.upload_frames_left = Some(left - 1);
            }
        }
    }
    if now - *last_check < 0.2 {
        return;
    }
    *last_check = now;

    for (root, p) in pending {
        if p.upload_frames_left.is_some() || world.get_entity(root).is_err() {
            continue;
        }
        if now - p.since > REVEAL_TIMEOUT_SECS {
            reveal(world, root);
            continue;
        }
        let r = subtree_readiness(world, root);
        if !r.waiting {
            let frames = r.texture_bytes.div_ceil(STREAM_UPLOAD_BYTES_PER_FRAME) as u32 + 2;
            if let Some(mut rp) = world.get_mut::<RevealPending>(root) {
                rp.upload_frames_left = Some(frames);
            }
        }
    }
}

/// Spread GPU uploads across frames while cells stream in, and lift the limit
/// again once they are done, so a scene opening behind the loading screen
/// still uploads at full speed. Leaves a budget it did not set alone.
#[cfg(feature = "render_3d")]
pub fn manage_stream_upload_budget(
    pending: Query<(), With<RevealPending>>,
    budget: Option<ResMut<bevy::render::render_asset::RenderAssetBytesPerFrame>>,
    mut ours: Local<bool>,
) {
    let Some(mut budget) = budget else { return };
    let want = !pending.is_empty();
    if want && budget.max_bytes.is_none() {
        budget.max_bytes = Some(STREAM_UPLOAD_BYTES_PER_FRAME);
        *ours = true;
    } else if !want && *ours {
        budget.max_bytes = None;
        *ours = false;
    }
}

// ---------------------------------------------------------------------------
// Distant stand-ins (HLOD)
// ---------------------------------------------------------------------------
//
// Streaming alone empties the world beyond the unload radius, which from any
// height means districts vanishing behind the camera. Each cell can carry a
// stand-in: its whole contents merged into one coarse, vertex-coloured mesh,
// built by the editor (`renzora_scene::world_partition::hlod`) and kept beside
// the cell's file. Stand-ins are small, always resident, and drawn whenever
// the cell's full contents are not: hidden the frame the cell is revealed
// (which already waits for its textures), shown again the frame it unloads.
// The skyline is therefore never empty, from any distance or direction.
//
// Stand-ins are spawned here at run time, not saved: the scene records only
// that a cell has one (`PartitionCell::has_hlod`), so rebuilding them changes
// files, never the scene.

/// On a cell's stand-in: the cell it stands in for.
#[derive(Component, Clone, Copy, Debug)]
pub struct PartitionHlod {
    pub cell: Entity,
}

/// On a cell with a spawned stand-in: that stand-in.
#[derive(Component, Clone, Copy, Debug)]
pub struct CellHlod(pub Entity);

/// Spawn the stand-in of every cell that has one built and none spawned.
#[allow(clippy::type_complexity)]
pub fn spawn_cell_hlods(
    mut commands: Commands,
    cells: Query<(Entity, &PartitionCell, &SceneInstance, &Transform, Option<&ChildOf>), Without<CellHlod>>,
) {
    for (cell, pc, instance, transform, parent) in &cells {
        if !pc.has_hlod {
            continue;
        }
        let mut proxy = commands
            .spawn((
                Name::new(format!("HLOD {}_{}", pc.x, pc.z)),
                *transform,
                Visibility::Inherited,
                MeshInstanceData {
                    model_path: Some(PartitionCell::hlod_path(&instance.source)),
                },
                // Not scene content: out of the hierarchy, out of saves, and
                // out of the editor's per-tab asset pins.
                renzora_scene_api::HideInHierarchy,
                renzora_scene_api::StreamedContent,
                // Drawn as ordinary geometry. The editor builds an `.rvg`
                // beside every model, stand-ins included, and the virtual
                // geometry renderer does not shade vertex colours, which are
                // all a stand-in's colour is: taken over by it, every
                // stand-in rendered plain white.
                renzora_scene_api::StandardGeometry,
                PartitionHlod { cell },
            ));
        // The stand-in copies the cell's transform, which is local to the
        // cell's folder, so it has to sit in the same folder to land where
        // the cell is.
        if let Some(parent) = parent {
            proxy.insert(ChildOf(parent.parent()));
        }
        let proxy = proxy.id();
        commands.entity(cell).insert(CellHlod(proxy));
    }
}

/// Show each stand-in exactly when its cell's full contents are not on
/// screen: unloaded, still loading, or loaded but held hidden until ready.
pub fn swap_cell_hlods(
    cells: Query<(&CellHlod, Option<&Children>)>,
    pending: Query<(), With<RevealPending>>,
    mut proxies: Query<&mut Visibility, With<PartitionHlod>>,
) {
    for (hlod, children) in &cells {
        let full_shown = children.is_some_and(|c| !c.is_empty() && c.iter().all(|e| !pending.contains(e)));
        let want = if full_shown { Visibility::Hidden } else { Visibility::Inherited };
        if let Ok(mut v) = proxies.get_mut(hlod.0) {
            if *v != want {
                *v = want;
            }
        }
    }
}

/// Report every moment a cell's full detail stops being drawn while nothing
/// drawable stands in for it: the cases where buildings vanish outright when
/// the camera pulls back. Reports the reason, so the gap can be fixed for a
/// known cause: no stand-in built for the cell, a stand-in whose model has
/// not finished loading, or one that loaded with nothing to draw.
///
/// Watches transitions only (full detail drawn, then not), so it costs a
/// lookup per cell per frame and logs once per gap.
pub fn report_stand_in_gaps(
    cells: Query<(Entity, &PartitionCell, Option<&CellHlod>, Option<&Children>)>,
    pending: Query<(), With<RevealPending>>,
    children_q: Query<&Children>,
    meshes: Query<(), With<Mesh3d>>,
    mut was_shown: Local<HashMap<Entity, bool>>,
) {
    for (cell, pc, hlod, children) in &cells {
        let shown = children.is_some_and(|c| !c.is_empty() && c.iter().all(|e| !pending.contains(e)));
        let before = was_shown.insert(cell, shown).unwrap_or(false);
        if !before || shown {
            continue;
        }
        let why = match hlod {
            None if !pc.has_hlod => Some("it has no stand-in (none was built for it)"),
            None => Some("its stand-in has not been spawned"),
            Some(h) => {
                let mut stack = vec![h.0];
                let mut drawable = false;
                while let Some(e) = stack.pop() {
                    if meshes.contains(e) {
                        drawable = true;
                        break;
                    }
                    if let Ok(c) = children_q.get(e) {
                        stack.extend(c.iter());
                    }
                }
                (!drawable).then_some("its stand-in has no mesh yet (still loading, or empty)")
            }
        };
        if let Some(why) = why {
            console_warn(
                "World Partition",
                format!("Cell {}_{} went out of view with nothing in its place: {why}", pc.x, pc.z),
            );
        }
    }
    was_shown.retain(|e, _| cells.contains(*e));
}

/// Remove stand-ins whose cell is gone (scene closed, cell deleted) or no
/// longer has one, and forget a stand-in that was despawned (a rebuild).
pub fn clean_up_cell_hlods(
    mut commands: Commands,
    proxies: Query<(Entity, &PartitionHlod)>,
    cells: Query<(Entity, &PartitionCell, Option<&CellHlod>)>,
) {
    for (proxy, p) in &proxies {
        let keep = cells
            .get(p.cell)
            .is_ok_and(|(_, pc, h)| pc.has_hlod && h.is_some_and(|h| h.0 == proxy));
        if !keep {
            commands.entity(proxy).despawn();
        }
    }
    for (cell, pc, h) in &cells {
        if let Some(h) = h {
            if !pc.has_hlod || proxies.get(h.0).is_err() {
                commands.entity(cell).remove::<CellHlod>();
            }
        }
    }
}

/// Per glTF mesh index, the model-space matrices of every node in the
/// default scene that places it. For building stand-ins from a model's file.
#[cfg(feature = "gltf")]
pub fn mesh_placements(json: &serde_json::Value) -> HashMap<usize, Vec<Mat4>> {
    let mut out: HashMap<usize, Vec<Mat4>> = HashMap::new();
    let Some(nodes) = json.get("nodes").and_then(|n| n.as_array()) else {
        return out;
    };
    let scene_index = json.get("scene").and_then(|s| s.as_u64()).unwrap_or(0) as usize;
    let roots: Vec<usize> = json
        .get("scenes")
        .and_then(|s| s.as_array())
        .and_then(|s| s.get(scene_index))
        .and_then(|s| s.get("nodes"))
        .and_then(|n| n.as_array())
        .map(|n| n.iter().filter_map(|v| v.as_u64().map(|i| i as usize)).collect())
        .unwrap_or_else(|| (0..nodes.len()).collect());
    let mut stack: Vec<(usize, Mat4, u32)> = roots.into_iter().map(|r| (r, Mat4::IDENTITY, 0)).collect();
    while let Some((index, parent, depth)) = stack.pop() {
        if depth > 64 {
            continue;
        }
        let Some(node) = nodes.get(index) else { continue };
        let world = parent * node_matrix(node);
        if let Some(mesh) = node.get("mesh").and_then(|m| m.as_u64()) {
            out.entry(mesh as usize).or_default().push(world);
        }
        if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
            for child in children.iter().filter_map(|c| c.as_u64()) {
                stack.push((child as usize, world, depth + 1));
            }
        }
    }
    out
}

/// The parsed glTF JSON of a model file (`.glb` or a model `.rvg`'s scene).
#[cfg(feature = "gltf")]
pub fn model_json(bytes: &[u8], is_rvg: bool) -> Option<serde_json::Value> {
    if is_rvg {
        renzora_rvg::scene_json(bytes).and_then(|s| serde_json::from_str(&s).ok())
    } else {
        glb_json(bytes)
    }
}

// ---------------------------------------------------------------------------
// Model bounds, read from the file
// ---------------------------------------------------------------------------

/// The model-space bounds of a `.glb` or `.rvg` model file, without loading it
/// as an asset: the min and max corner over every mesh the file's default
/// scene places, with each node's transform applied.
///
/// glTF requires every `POSITION` accessor to carry its `min` and `max`, so a
/// `.glb` is answered from its JSON alone. An `.rvg` keeps its vertex
/// positions in its own geometry, which is read instead (the accessors of the
/// scene it carries may be empty).
#[cfg(feature = "gltf")]
pub fn model_file_bounds(path: &Path) -> Option<(Vec3, Vec3)> {
    let bytes = std::fs::read(path).ok()?;
    let is_rvg = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("rvg"));
    if is_rvg {
        let file = renzora_rvg::RvgFile::from_bytes(&bytes).ok()?;
        let mut per_mesh: HashMap<usize, (Vec3, Vec3)> = HashMap::new();
        for mesh in &file.meshes {
            if let Some(b) = bounds_of_points(mesh.positions.iter().map(|p| Vec3::from(*p))) {
                let entry = per_mesh.entry(mesh.mesh_index as usize).or_insert(b);
                *entry = union(*entry, b);
            }
        }
        let glb = file.scene.as_deref();
        let json = glb.and_then(|g| glb_json(g));
        return match json {
            Some(json) => scene_bounds(&json, &per_mesh),
            // An `.rvg` built beside a `.glb` has no scene of its own: its
            // meshes are placed by that GLB. Untransformed bounds are the
            // best answer available without it.
            None => per_mesh.values().copied().reduce(union),
        };
    }
    let json = glb_json(&bytes)?;
    let per_mesh = accessor_mesh_bounds(&json);
    scene_bounds(&json, &per_mesh)
}

/// The JSON chunk of a GLB, parsed.
#[cfg(feature = "gltf")]
fn glb_json(glb: &[u8]) -> Option<serde_json::Value> {
    if glb.get(0..4)? != b"glTF" {
        return None;
    }
    let len = u32::from_le_bytes(glb.get(12..16)?.try_into().ok()?) as usize;
    if glb.get(16..20)? != b"JSON" {
        return None;
    }
    serde_json::from_slice(glb.get(20..20 + len)?).ok()
}

/// Per-mesh bounds from the `min`/`max` of each primitive's `POSITION`.
#[cfg(feature = "gltf")]
fn accessor_mesh_bounds(json: &serde_json::Value) -> HashMap<usize, (Vec3, Vec3)> {
    let mut out = HashMap::new();
    let accessors = json.get("accessors").and_then(|a| a.as_array());
    let meshes = json.get("meshes").and_then(|m| m.as_array());
    let (Some(accessors), Some(meshes)) = (accessors, meshes) else {
        return out;
    };
    for (mesh_index, mesh) in meshes.iter().enumerate() {
        let Some(primitives) = mesh.get("primitives").and_then(|p| p.as_array()) else {
            continue;
        };
        for primitive in primitives {
            let Some(index) = primitive
                .get("attributes")
                .and_then(|a| a.get("POSITION"))
                .and_then(|p| p.as_u64())
            else {
                continue;
            };
            let Some(accessor) = accessors.get(index as usize) else {
                continue;
            };
            let (Some(min), Some(max)) = (vec3_field(accessor, "min"), vec3_field(accessor, "max"))
            else {
                continue;
            };
            let entry = out.entry(mesh_index).or_insert((min, max));
            *entry = union(*entry, (min, max));
        }
    }
    out
}

/// Bounds over the default scene, applying each node's transform to the
/// bounds of the mesh it places.
#[cfg(feature = "gltf")]
fn scene_bounds(
    json: &serde_json::Value,
    per_mesh: &HashMap<usize, (Vec3, Vec3)>,
) -> Option<(Vec3, Vec3)> {
    let nodes = json.get("nodes").and_then(|n| n.as_array())?;
    let scene_index = json.get("scene").and_then(|s| s.as_u64()).unwrap_or(0) as usize;
    let roots: Vec<usize> = json
        .get("scenes")
        .and_then(|s| s.as_array())
        .and_then(|s| s.get(scene_index))
        .and_then(|s| s.get("nodes"))
        .and_then(|n| n.as_array())
        .map(|n| n.iter().filter_map(|v| v.as_u64().map(|i| i as usize)).collect())
        .unwrap_or_else(|| (0..nodes.len()).collect());

    let mut result: Option<(Vec3, Vec3)> = None;
    // Depth guard: a malformed file with a node cycle must not hang the editor.
    let mut stack: Vec<(usize, Mat4, u32)> = roots.into_iter().map(|r| (r, Mat4::IDENTITY, 0)).collect();
    while let Some((index, parent, depth)) = stack.pop() {
        if depth > 64 {
            continue;
        }
        let Some(node) = nodes.get(index) else { continue };
        let world = parent * node_matrix(node);
        if let Some(mesh) = node.get("mesh").and_then(|m| m.as_u64()) {
            if let Some(&(min, max)) = per_mesh.get(&(mesh as usize)) {
                let b = transform_bounds(world, min, max);
                result = Some(result.map_or(b, |r| union(r, b)));
            }
        }
        if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
            for child in children.iter().filter_map(|c| c.as_u64()) {
                stack.push((child as usize, world, depth + 1));
            }
        }
    }
    result
}

#[cfg(feature = "gltf")]
fn node_matrix(node: &serde_json::Value) -> Mat4 {
    if let Some(m) = node.get("matrix").and_then(|m| m.as_array()) {
        let v: Vec<f32> = m.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect();
        if v.len() == 16 {
            return Mat4::from_cols_slice(&v);
        }
    }
    let t = vec3_field(node, "translation").unwrap_or(Vec3::ZERO);
    let s = vec3_field(node, "scale").unwrap_or(Vec3::ONE);
    let r = node
        .get("rotation")
        .and_then(|r| r.as_array())
        .and_then(|r| {
            let v: Vec<f32> = r.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect();
            (v.len() == 4).then(|| Quat::from_xyzw(v[0], v[1], v[2], v[3]).normalize())
        })
        .unwrap_or(Quat::IDENTITY);
    Mat4::from_scale_rotation_translation(s, r, t)
}

#[cfg(feature = "gltf")]
fn vec3_field(value: &serde_json::Value, key: &str) -> Option<Vec3> {
    let a = value.get(key)?.as_array()?;
    if a.len() < 3 {
        return None;
    }
    Some(Vec3::new(
        a[0].as_f64()? as f32,
        a[1].as_f64()? as f32,
        a[2].as_f64()? as f32,
    ))
}

fn bounds_of_points(points: impl Iterator<Item = Vec3>) -> Option<(Vec3, Vec3)> {
    points.fold(None, |acc, p| match acc {
        None => Some((p, p)),
        Some((min, max)) => Some((min.min(p), max.max(p))),
    })
}

#[cfg(feature = "gltf")]
fn union(a: (Vec3, Vec3), b: (Vec3, Vec3)) -> (Vec3, Vec3) {
    (a.0.min(b.0), a.1.max(b.1))
}

/// The axis-aligned bounds of a box after an affine transform: its eight
/// corners transformed and re-bounded.
fn transform_bounds(m: Mat4, min: Vec3, max: Vec3) -> (Vec3, Vec3) {
    let corners = (0..8).map(|i| {
        m.transform_point3(Vec3::new(
            if i & 1 == 0 { min.x } else { max.x },
            if i & 2 == 0 { min.y } else { max.y },
            if i & 4 == 0 { min.z } else { max.z },
        ))
    });
    bounds_of_points(corners).unwrap_or((min, max))
}

// ---------------------------------------------------------------------------
// Partitioning a scene
// ---------------------------------------------------------------------------

/// What [`partition_entities`] did.
#[derive(Debug, Default, Clone)]
pub struct PartitionReport {
    /// Entities moved into a cell.
    pub placed: usize,
    /// Cells created or added to.
    pub cells: usize,
    /// Models whose file bounds could not be read, placed by their origin.
    pub unreadable_models: usize,
    /// The model paths that moved into cells. Anything that pinned these for
    /// the session (the editor's tab cache) should let go of them now, or the
    /// cells they went into can never free them by streaming out.
    pub moved_models: Vec<String>,
}

/// The folder a scene's cell files live in: `scenes/main.bsn` keeps its cells
/// in `scenes/main.cells/`, beside it, so moving or deleting a scene in a file
/// browser takes its cells along visibly.
pub fn cells_dir_for(scene_path: &Path) -> PathBuf {
    let stem = scene_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("scene");
    scene_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{stem}.cells"))
}

/// The file one cell's contents are saved in.
pub fn cell_file(cells_dir: &Path, cell: IVec2) -> PathBuf {
    cells_dir.join(format!("{}_{}.bsn", cell.x, cell.y))
}

/// The scene being edited, as an absolute path: the active scene tab, the
/// same answer the save path and the instance cycle guard use.
fn host_scene_path(world: &World) -> Option<PathBuf> {
    let project = world.get_resource::<CurrentProject>()?;
    Some(
        project
            .config
            .editor_last_scene
            .as_deref()
            .map(|scene| project.resolve_path(scene))
            .unwrap_or_else(|| project.main_scene_path()),
    )
}

/// Move the content under `roots` into the scene's partition cells.
///
/// Placeable entities are models (`MeshInstanceData`) and anything with no
/// children. A group (children, no model) is not placed itself: its
/// descendants are, and the group's name is recreated inside each cell that
/// receives one of them, so `Buildings/tile` stays `Buildings/tile` in every
/// cell and a category can still be found and hidden. Groups left empty are
/// removed.
///
/// Every moved entity keeps its exact world transform: its new local
/// transform is solved against the cell, and recreated groups carry the
/// identity. Touched cells are written to their files at once, so the cells
/// can stream out the moment this returns without losing anything.
///
/// Creates the scene's [`WorldPartition`] with defaults if it has none.
pub fn partition_entities(world: &mut World, roots: &[Entity]) -> Result<PartitionReport, String> {
    let (folder, partition) = match partition_root(world) {
        Some(found) => found,
        None => {
            let e = world
                .spawn((Name::new("World Partition"), WorldPartition::default(), Transform::default(), Visibility::default()))
                .id();
            (e, WorldPartition::default())
        }
    };
    let project = world
        .get_resource::<CurrentProject>()
        .cloned()
        .ok_or("no project is open")?;
    let scene_path = host_scene_path(world).ok_or("no scene is open")?;
    let cells_dir = cells_dir_for(&scene_path);

    // 1. Collect placeables, remembering the chain of group names above each.
    struct Placeable {
        entity: Entity,
        groups: Vec<String>,
        model: Option<String>,
    }
    let mut placeables: Vec<Placeable> = Vec::new();
    let mut emptied_groups: Vec<Entity> = Vec::new();
    let mut skipped_scene_wide = 0usize;
    let mut stack: Vec<(Entity, Vec<String>)> = roots.iter().map(|&e| (e, Vec::new())).collect();
    while let Some((entity, groups)) = stack.pop() {
        if world.get_entity(entity).is_err()
            || world.get::<PartitionCell>(entity).is_some()
            || world.get::<WorldPartition>(entity).is_some()
        {
            continue;
        }
        // Scene-wide things never go in a cell: in one, they vanish whenever
        // that cell unloads. The world environment sits at the origin, so
        // partitioning it took the sky and the sun with it the moment the
        // camera left the origin's cell.
        if is_scene_wide(world, entity) {
            skipped_scene_wide += 1;
            continue;
        }
        let model = world
            .get::<MeshInstanceData>(entity)
            .map(|m| m.model_path.clone());
        let children: Vec<Entity> = world
            .get::<Children>(entity)
            .map(|c| c.iter().filter(|&c| world.get::<Name>(c).is_some()).collect())
            .unwrap_or_default();
        if model.is_some() || children.is_empty() {
            placeables.push(Placeable { entity, groups, model: model.flatten() });
        } else {
            let name = world
                .get::<Name>(entity)
                .map(|n| n.as_str().to_string())
                .unwrap_or_else(|| "Group".to_string());
            let mut inner = groups.clone();
            inner.push(name);
            for child in children {
                stack.push((child, inner.clone()));
            }
            emptied_groups.push(entity);
        }
    }
    if placeables.is_empty() {
        return Err(if skipped_scene_wide > 0 {
            "nothing to partition: cameras, lights and the world environment stay out of cells. \
             Select the content to split into cells"
                .into()
        } else {
            "nothing to partition: select the entities or groups to split into cells".into()
        });
    }

    // 2. Each placeable's world-space centre. Model files are read in
    // parallel: a city is hundreds of files and some are large.
    let globals: Vec<GlobalTransform> = placeables
        .iter()
        .map(|p| world_transform(world, p.entity))
        .collect();
    let model_bounds = read_model_bounds(
        &project.path,
        &placeables.iter().map(|p| p.model.clone()).collect::<Vec<_>>(),
    );
    let mut unreadable = 0usize;
    let centers: Vec<Vec3> = placeables
        .iter()
        .zip(&globals)
        .zip(&model_bounds)
        .map(|((p, global), bounds)| match bounds {
            Some((min, max)) => {
                let (wmin, wmax) = transform_bounds(global.to_matrix(), *min, *max);
                (wmin + wmax) * 0.5
            }
            None => {
                if p.model.is_some() {
                    unreadable += 1;
                }
                global.translation()
            }
        })
        .collect();

    // 3. Find or create the cells, loading any existing cell that is not
    // resident so its file is extended rather than overwritten.
    let mut by_cell: HashMap<IVec2, Vec<usize>> = HashMap::new();
    for (i, c) in centers.iter().enumerate() {
        by_cell.entry(partition.cell_of(*c)).or_default().push(i);
    }
    let mut existing: HashMap<IVec2, Entity> = {
        let mut q = world.query::<(Entity, &PartitionCell)>();
        q.iter(world).map(|(e, c)| (c.coord(), e)).collect()
    };
    let load = partition.load_radius.max(0.0);
    let unload = partition.effective_unload_radius();

    let mut touched: Vec<Entity> = Vec::new();
    for (coord, members) in &by_cell {
        let cell = match existing.get(coord) {
            Some(&cell) => {
                expand_cell_now(world, cell);
                cell
            }
            None => {
                let source_abs = cell_file(&cells_dir, *coord);
                let source = project
                    .make_relative(&source_abs)
                    .ok_or("the scene is outside the project")?;
                // Filed under the World Partition entity, so hundreds of
                // cells fold away in the hierarchy instead of burying the
                // scene's own entities. Placed so its world position is the
                // cell's centre whatever the folder's transform.
                let local = GlobalTransform::from_translation(partition.cell_center(*coord))
                    .reparented_to(&world_transform(world, folder));
                let cell = world
                    .spawn((
                        Name::new(format!("Cell {}_{}", coord.x, coord.y)),
                        local,
                        Visibility::default(),
                        SceneInstance {
                            source,
                            streamed: true,
                            load_radius: load,
                            unload_radius: unload,
                        },
                        PartitionCell::new(coord.x, coord.y),
                        ChildOf(folder),
                    ))
                    .id();
                existing.insert(*coord, cell);
                cell
            }
        };
        let cell_global = GlobalTransform::from(Transform::from_translation(partition.cell_center(*coord)));

        // Recreated groups, by their name path, so two tiles from `Buildings`
        // land under one `Buildings` in the cell.
        let mut groups: HashMap<Vec<String>, Entity> = HashMap::new();
        for &i in members {
            let p = &placeables[i];
            let mut parent = cell;
            for depth in 1..=p.groups.len() {
                let key = p.groups[..depth].to_vec();
                parent = match groups.get(&key) {
                    Some(&g) => g,
                    None => {
                        let g = find_child_named(world, parent, &p.groups[depth - 1]).unwrap_or_else(|| {
                            world
                                .spawn((
                                    Name::new(p.groups[depth - 1].clone()),
                                    Transform::IDENTITY,
                                    Visibility::default(),
                                    ChildOf(parent),
                                ))
                                .id()
                        });
                        groups.insert(key, g);
                        g
                    }
                };
            }
            let local = globals[i].reparented_to(&cell_global);
            // Now cell content, so marked as a stream would have marked it.
            world
                .entity_mut(p.entity)
                .insert((local, ChildOf(parent), renzora_scene_api::StreamedContent));
        }
        for &g in groups.values() {
            world.entity_mut(g).insert(renzora_scene_api::StreamedContent);
        }
        touched.push(cell);
    }

    // 4. Groups whose content all moved are now empty shells: remove them,
    // deepest first so a parent sees its emptied child already gone.
    for group in emptied_groups.into_iter().rev() {
        let empty = world.get_entity(group).is_ok()
            && world
                .get::<Children>(group)
                .is_none_or(|c| c.iter().all(|c| world.get::<Name>(c).is_none()));
        if empty {
            world.despawn(group);
        }
    }

    // 5. Write every touched cell now. Until its file holds its contents a
    // cell that streams out would come back empty.
    for &cell in &touched {
        let Some(source) = world.get::<SceneInstance>(cell).map(|s| s.source.clone()) else {
            continue;
        };
        let path = project.path.join(&source);
        if let Err(e) = crate::scene_io::save_prefab_source(world, cell, &path) {
            return Err(format!("could not write {}: {e}", path.display()));
        }
    }

    let mut moved_models: Vec<String> = placeables.iter().filter_map(|p| p.model.clone()).collect();
    moved_models.sort();
    moved_models.dedup();
    let report = PartitionReport {
        placed: placeables.len(),
        cells: touched.len(),
        unreadable_models: unreadable,
        moved_models,
    };
    console_success(
        "World Partition",
        format!(
            "Partitioned {} entities into {} cells ({} m)",
            report.placed, report.cells, partition.cell_size
        ),
    );
    if unreadable > 0 {
        console_warn(
            "World Partition",
            format!("{unreadable} models had no readable bounds and were placed by their origin"),
        );
    }
    Ok(report)
}

/// Read the bounds of every model path, in parallel, keeping order.
fn read_model_bounds(project_root: &Path, models: &[Option<String>]) -> Vec<Option<(Vec3, Vec3)>> {
    #[cfg(all(feature = "gltf", not(target_arch = "wasm32")))]
    {
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 16);
        let chunk = models.len().div_ceil(threads).max(1);
        let mut out: Vec<Option<(Vec3, Vec3)>> = vec![None; models.len()];
        std::thread::scope(|scope| {
            for (models, out) in models.chunks(chunk).zip(out.chunks_mut(chunk)) {
                scope.spawn(move || {
                    for (model, slot) in models.iter().zip(out.iter_mut()) {
                        *slot = model
                            .as_ref()
                            .and_then(|m| model_file_bounds(&project_root.join(m)));
                    }
                });
            }
        });
        out
    }
    #[cfg(not(all(feature = "gltf", not(target_arch = "wasm32"))))]
    {
        let _ = project_root;
        vec![None; models.len()]
    }
}

/// Whether an entity is scene-wide rather than content: the world
/// environment, a camera, or a directional light. These light or view the
/// whole world, so they must stay resident wherever the camera is.
fn is_scene_wide(world: &World, entity: Entity) -> bool {
    world
        .get::<renzora_world_api::world_environment::WorldEnvironment>(entity)
        .is_some()
        || world.get::<Camera>(entity).is_some()
        || world.get::<renzora_scene_api::SceneCamera>(entity).is_some()
        || world.get::<DirectionalLight>(entity).is_some()
}

/// The entity holding the scene's [`WorldPartition`], with its settings.
/// It doubles as the folder every cell is filed under.
fn partition_root(world: &mut World) -> Option<(Entity, WorldPartition)> {
    let mut q = world.query::<(Entity, &WorldPartition)>();
    q.iter(world).next().map(|(e, p)| (e, p.clone()))
}

/// File every cell sitting at the top of the hierarchy under the World
/// Partition entity, keeping it where it is in the world.
///
/// New cells are created there already; this catches scenes partitioned
/// before cells had a folder, whose hundreds of cells otherwise sit at the
/// root and bury everything else. Gated on [`has_loose_cells`], so a tidy
/// scene never pays for an exclusive system.
pub fn file_cells_under_partition(world: &mut World) {
    let loose: Vec<Entity> = {
        let mut q = world.query_filtered::<Entity, (With<PartitionCell>, Without<ChildOf>)>();
        q.iter(world).collect()
    };
    if loose.is_empty() {
        return;
    }
    let Some((folder, _)) = partition_root(world) else {
        return;
    };
    let folder_global = world_transform(world, folder);
    for cell in loose {
        let local = world_transform(world, cell).reparented_to(&folder_global);
        world.entity_mut(cell).insert((local, ChildOf(folder)));
    }
}

/// Whether any cell sits at the top of the hierarchy, outside its folder.
pub fn has_loose_cells(cells: Query<(), (With<PartitionCell>, Without<ChildOf>)>) -> bool {
    !cells.is_empty()
}

fn find_child_named(world: &World, parent: Entity, name: &str) -> Option<Entity> {
    world
        .get::<Children>(parent)?
        .iter()
        .find(|&c| world.get::<Name>(c).is_some_and(|n| n.as_str() == name))
}

/// An entity's world transform, composed from its `Transform` and every
/// ancestor's. Computed rather than read from `GlobalTransform`, which lags a
/// frame behind anything spawned, moved or reparented since propagation ran,
/// and the partition action itself reparents as it goes.
fn world_transform(world: &World, entity: Entity) -> GlobalTransform {
    let mut chain: Vec<Transform> = Vec::new();
    let mut current = Some(entity);
    // Depth guard against a corrupt hierarchy.
    while let Some(e) = current.filter(|_| chain.len() < 256) {
        chain.push(world.get::<Transform>(e).copied().unwrap_or_default());
        current = world.get::<ChildOf>(e).map(|c| c.parent());
    }
    chain
        .into_iter()
        .rev()
        .fold(GlobalTransform::IDENTITY, |parent, local| parent.mul_transform(local))
}

/// Load one cell's contents synchronously, if it is not already resident.
/// For the partition action, which must extend a cell's file rather than
/// replace it; streaming goes through `scene_stream` instead.
pub fn expand_cell_now(world: &mut World, cell: Entity) {
    if world.get::<Children>(cell).is_some_and(|c| c.iter().count() > 0) {
        return;
    }
    let Some(source) = world.get::<SceneInstance>(cell).map(|s| s.source.clone()) else {
        return;
    };
    let Some(root) = world.get_resource::<CurrentProject>().map(|p| p.path.clone()) else {
        return;
    };
    let path = root.join(&source);
    if !path.exists() {
        return;
    }
    if world.contains_resource::<crate::scene_stream::SceneStreams>() {
        crate::scene_stream::cancel_streams_under(world, cell);
    }
    let before: std::collections::HashSet<Entity> = {
        let mut q = world.query_filtered::<Entity, (With<Name>, Without<ChildOf>)>();
        q.iter(world).collect()
    };
    crate::scene_io::load_scene(world, &path);
    let new_roots: Vec<Entity> = {
        let mut q = world.query_filtered::<Entity, (With<Name>, Without<ChildOf>)>();
        q.iter(world).filter(|e| !before.contains(e) && *e != cell).collect()
    };
    for root in new_roots {
        world.entity_mut(root).insert(ChildOf(cell));
    }
}

/// Whether a cell's contents are resident and finished arriving. Contents
/// are reparented under the cell only when a stream completes, so children
/// mean a whole cell, never a half-spawned one.
pub fn cell_is_loaded(world: &World, cell: Entity) -> bool {
    world
        .get::<Children>(cell)
        .is_some_and(|c| c.iter().count() > 0)
}

/// Write every loaded cell's contents back to its file. The editor calls this
/// before anything despawns loaded cells (a cell streaming out, a tab
/// switch), since a cell's edits live only in the world until written.
pub fn write_back_loaded_cells(world: &mut World) {
    let Some(root) = world.get_resource::<CurrentProject>().map(|p| p.path.clone()) else {
        return;
    };
    let cells: Vec<(Entity, String)> = {
        let mut q = world.query_filtered::<(Entity, &SceneInstance), With<PartitionCell>>();
        q.iter(world).map(|(e, s)| (e, s.source.clone())).collect()
    };
    for (cell, source) in cells {
        if cell_is_loaded(world, cell) {
            write_back_cell(world, cell, &root.join(source));
        }
    }
}

/// Write one loaded cell back to its file, reporting a failure.
pub fn write_back_cell(world: &mut World, cell: Entity, path: &Path) {
    if let Err(e) = crate::scene_io::save_prefab_source(world, cell, path) {
        console_error(
            "World Partition",
            format!("Failed to save cell {}: {e}", path.display()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_follow_the_ground_grid_from_the_world_origin() {
        let p = WorldPartition { cell_size: 100.0, ..Default::default() };
        assert_eq!(p.cell_of(Vec3::new(0.0, 500.0, 0.0)), IVec2::new(0, 0));
        assert_eq!(p.cell_of(Vec3::new(99.9, 0.0, 99.9)), IVec2::new(0, 0));
        assert_eq!(p.cell_of(Vec3::new(100.0, 0.0, 0.0)), IVec2::new(1, 0));
        // Negative space floors away from zero, so cell 0 is not twice as wide.
        assert_eq!(p.cell_of(Vec3::new(-0.1, 0.0, -150.0)), IVec2::new(-1, -2));
        assert_eq!(p.cell_center(IVec2::new(-1, 2)), Vec3::new(-50.0, 0.0, 250.0));
    }

    #[test]
    fn unload_radius_keeps_a_gap_above_load_radius() {
        let p = WorldPartition {
            cell_size: 1000.0,
            load_radius: 1500.0,
            unload_radius: 1000.0,
            ..Default::default()
        };
        assert!(p.effective_unload_radius() >= 1750.0);
    }

    #[test]
    fn ground_distance_ignores_height() {
        assert_eq!(ground_distance(Vec3::new(0.0, 900.0, 0.0), Vec3::new(3.0, 0.0, 4.0)), 5.0);
    }

    #[test]
    fn transformed_bounds_cover_the_rotated_box() {
        let m = Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let (min, max) = transform_bounds(m, Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 1.0, 1.0));
        assert!((min - Vec3::new(0.0, 0.0, -2.0)).length() < 1e-4, "{min}");
        assert!((max - Vec3::new(1.0, 1.0, 0.0)).length() < 1e-4, "{max}");
    }

    #[test]
    fn cell_files_live_beside_their_scene() {
        let dir = cells_dir_for(Path::new("proj/scenes/main.bsn"));
        assert_eq!(dir, Path::new("proj/scenes/main.cells"));
        assert_eq!(cell_file(&dir, IVec2::new(-3, 12)), Path::new("proj/scenes/main.cells/-3_12.bsn"));
    }

    /// A world with an open project in a fresh temp folder, and the types the
    /// scene writer reflects.
    fn project_world(tag: &str) -> (World, PathBuf) {
        let dir = std::env::temp_dir().join(format!("renzora_wp_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scenes")).unwrap();
        let mut world = World::new();
        let registry = AppTypeRegistry::default();
        {
            let mut r = registry.write();
            r.register::<Name>();
            r.register::<Transform>();
            r.register::<Visibility>();
            r.register::<ChildOf>();
            r.register::<SceneInstance>();
            r.register::<PartitionCell>();
            r.register::<WorldPartition>();
        }
        world.insert_resource(registry);
        let mut config = renzora_project_api::ProjectConfig::default();
        config.main_scene = "scenes/main.bsn".into();
        config.editor_last_scene = Some("scenes/main.bsn".into());
        world.insert_resource(CurrentProject { path: dir.clone(), config });
        (world, dir)
    }

    fn global(world: &World, e: Entity) -> Vec3 {
        world_transform(world, e).translation()
    }

    /// The core promise: content lands in the cell its position falls in, and
    /// not one object moves in the world doing it, even under a scaled parent
    /// (the shape city data arrives in).
    #[test]
    fn partitioning_keeps_every_object_where_it_was() {
        let (mut world, dir) = project_world("keep");
        world.spawn((WorldPartition { cell_size: 100.0, ..Default::default() }, Name::new("World Partition")));
        let city = world
            .spawn((Name::new("city"), Transform::from_scale(Vec3::splat(10.0)), Visibility::default()))
            .id();
        let buildings = world
            .spawn((Name::new("Buildings"), Transform::IDENTITY, Visibility::default(), ChildOf(city)))
            .id();
        // World x = 10 * 3 = 30 (cell 0) and 10 * 25 = 250 (cell 2).
        let a = world
            .spawn((Name::new("a"), Transform::from_xyz(3.0, 1.0, 3.0), ChildOf(buildings)))
            .id();
        let b = world
            .spawn((Name::new("b"), Transform::from_xyz(25.0, 0.0, -1.0), ChildOf(buildings)))
            .id();
        let before = (global(&world, a), global(&world, b));

        let report = partition_entities(&mut world, &[city]).expect("partition");
        assert_eq!((report.placed, report.cells), (2, 2));

        let after = (global(&world, a), global(&world, b));
        assert!((before.0 - after.0).length() < 1e-3, "{before:?} -> {after:?}");
        assert!((before.1 - after.1).length() < 1e-3, "{before:?} -> {after:?}");

        // Each sits under `Cell x_z / city / Buildings`, the group path it
        // came from recreated, in the cell its position is in.
        let cell_of = |world: &World, e: Entity| {
            let buildings = world.get::<ChildOf>(e).unwrap().parent();
            assert_eq!(world.get::<Name>(buildings).unwrap().as_str(), "Buildings");
            let city = world.get::<ChildOf>(buildings).unwrap().parent();
            assert_eq!(world.get::<Name>(city).unwrap().as_str(), "city");
            let cell = world.get::<ChildOf>(city).unwrap().parent();
            world.get::<PartitionCell>(cell).copied().unwrap().coord()
        };
        assert_eq!(cell_of(&world, a), IVec2::new(0, 0));
        assert_eq!(cell_of(&world, b), IVec2::new(2, -1));

        // Every cell is filed under the World Partition entity, not left at
        // the top of the hierarchy.
        let mut cells = world.query_filtered::<&ChildOf, With<PartitionCell>>();
        let parents: Vec<Entity> = cells.iter(&world).map(|c| c.parent()).collect();
        assert_eq!(parents.len(), 2);
        assert!(parents.iter().all(|&p| world.get::<WorldPartition>(p).is_some()));

        // The emptied groups are gone; the cell files exist.
        assert!(world.get_entity(buildings).is_err());
        assert!(world.get_entity(city).is_err());
        let cells = dir.join("scenes").join("main.cells");
        assert!(cells.join("0_0.bsn").exists());
        assert!(cells.join("2_-1.bsn").exists());
        let text = std::fs::read_to_string(cells.join("2_-1.bsn")).unwrap();
        assert!(text.contains("\"b\""), "cell file should hold its content:\n{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A scene partitioned before cells had a folder has its cells at the
    /// root. They are filed under the World Partition entity without moving in
    /// the world, even when that entity is not at the origin; and a cell
    /// created under a moved folder still lands on its grid square.
    #[test]
    fn loose_cells_are_filed_under_the_partition_without_moving() {
        let (mut world, dir) = project_world("folder");
        let folder = world
            .spawn((
                WorldPartition { cell_size: 100.0, ..Default::default() },
                Name::new("World Partition"),
                Transform::from_xyz(7.0, 0.0, -3.0),
            ))
            .id();
        let loose = world
            .spawn((Name::new("Cell 4_2"), Transform::from_xyz(450.0, 0.0, 250.0), PartitionCell::new(4, 2)))
            .id();
        file_cells_under_partition(&mut world);
        assert_eq!(world.get::<ChildOf>(loose).map(|c| c.parent()), Some(folder));
        assert!((global(&world, loose) - Vec3::new(450.0, 0.0, 250.0)).length() < 1e-3);

        let thing = world.spawn((Name::new("thing"), Transform::from_xyz(150.0, 0.0, 50.0))).id();
        partition_entities(&mut world, &[thing]).expect("partition");
        let cell = world.get::<ChildOf>(thing).unwrap().parent();
        assert_eq!(world.get::<ChildOf>(cell).map(|c| c.parent()), Some(folder));
        assert!((global(&world, cell) - Vec3::new(150.0, 0.0, 50.0)).length() < 1e-3);
        assert!((global(&world, thing) - Vec3::new(150.0, 0.0, 50.0)).length() < 1e-3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Partitioning into a cell that already exists but is not loaded must
    /// add to its file, never replace what is already in it.
    #[test]
    fn partitioning_into_an_unloaded_cell_keeps_its_contents() {
        let (mut world, dir) = project_world("extend");
        world.spawn((WorldPartition { cell_size: 100.0, ..Default::default() }, Name::new("World Partition")));
        let first = world.spawn((Name::new("first"), Transform::from_xyz(10.0, 0.0, 10.0))).id();
        partition_entities(&mut world, &[first]).expect("first partition");

        // Unload the cell, as streaming would.
        let cell = {
            let mut q = world.query_filtered::<Entity, With<PartitionCell>>();
            q.single(&world).unwrap()
        };
        let kids: Vec<Entity> = world.get::<Children>(cell).unwrap().iter().collect();
        for k in kids {
            world.despawn(k);
        }
        assert!(!cell_is_loaded(&world, cell));

        let second = world.spawn((Name::new("second"), Transform::from_xyz(20.0, 0.0, 20.0))).id();
        partition_entities(&mut world, &[second]).expect("second partition");
        let text = std::fs::read_to_string(dir.join("scenes/main.cells/0_0.bsn")).unwrap();
        assert!(text.contains("\"first\"") && text.contains("\"second\""), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The world environment, cameras and suns are scene-wide: partitioning a
    /// selection that includes them must leave them where they are, and a
    /// selection of nothing else must do nothing at all.
    #[test]
    fn scene_wide_entities_stay_out_of_cells() {
        let (mut world, dir) = project_world("scenewide");
        world.spawn((WorldPartition { cell_size: 100.0, ..Default::default() }, Name::new("World Partition")));
        let env = world
            .spawn((
                Name::new("world_environment"),
                renzora_world_api::world_environment::WorldEnvironment::default(),
                Transform::default(),
            ))
            .id();
        let camera = world.spawn((Name::new("camera"), Camera::default(), Transform::default())).id();
        assert!(partition_entities(&mut world, &[env, camera]).is_err());
        assert!(world.get::<ChildOf>(env).is_none() && world.get::<ChildOf>(camera).is_none());

        let thing = world.spawn((Name::new("thing"), Transform::from_xyz(5.0, 0.0, 5.0))).id();
        let report = partition_entities(&mut world, &[env, camera, thing]).expect("partition");
        assert_eq!(report.placed, 1);
        assert!(world.get::<ChildOf>(env).is_none(), "the world environment went into a cell");
        assert!(world.get::<ChildOf>(thing).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Held-hidden contents must never reach a cell file hidden: writing a
    /// cell shows its waiting contents first, restoring what they had.
    #[test]
    fn a_cell_written_while_waiting_saves_its_contents_visible() {
        let (mut world, dir) = project_world("reveal");
        world.spawn((WorldPartition { cell_size: 100.0, ..Default::default() }, Name::new("World Partition")));
        let thing = world
            .spawn((Name::new("thing"), Transform::from_xyz(5.0, 0.0, 5.0), Visibility::Visible))
            .id();
        partition_entities(&mut world, &[thing]).expect("partition");
        let cell = {
            let mut q = world.query_filtered::<Entity, With<PartitionCell>>();
            q.single(&world).unwrap()
        };

        hold_until_ready(&mut world, &[thing]);
        assert_eq!(world.get::<Visibility>(thing), Some(&Visibility::Hidden));

        let path = dir.join("scenes/main.cells/0_0.bsn");
        write_back_cell(&mut world, cell, &path);
        assert_eq!(world.get::<Visibility>(thing), Some(&Visibility::Visible));
        assert!(world.get::<RevealPending>(thing).is_none());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("Hidden"), "a waiting cell was saved hidden:\n{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// In edit mode with a partition streaming in the editor, the eager
    /// expander must leave cells to the editor's driver, or opening the scene
    /// would load the whole world. Without editor streaming it expands them as
    /// it always has.
    #[test]
    fn the_eager_expander_leaves_cells_to_the_editor_driver() {
        let (mut world, dir) = project_world("expand");
        world.insert_resource(renzora_project_api::EditorSession(true));
        world.insert_resource(renzora_project_api::PlayModeState::default());
        let partition = world
            .spawn((WorldPartition { cell_size: 100.0, ..Default::default() }, Name::new("World Partition")))
            .id();
        let thing = world.spawn((Name::new("thing"), Transform::from_xyz(5.0, 0.0, 5.0))).id();
        partition_entities(&mut world, &[thing]).expect("partition");
        let cell = {
            let mut q = world.query_filtered::<Entity, With<PartitionCell>>();
            q.single(&world).unwrap()
        };
        let kids: Vec<Entity> = world.get::<Children>(cell).unwrap().iter().collect();
        for k in kids {
            world.despawn(k);
        }

        assert!(partition_streams_in_editor(&mut world));
        crate::scene_io::expand_scene_instances(&mut world);
        assert!(!cell_is_loaded(&world, cell), "cell expanded while the editor streams it");

        world.get_mut::<WorldPartition>(partition).unwrap().stream_in_editor = false;
        assert!(!partition_streams_in_editor(&mut world));
        crate::scene_io::expand_scene_instances(&mut world);
        assert!(cell_is_loaded(&world, cell), "with editor streaming off, cells expand as instances do");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A GLB whose single node scales and moves a unit box: the bounds must
    /// come back in the node's space, read from the JSON alone.
    #[cfg(feature = "gltf")]
    #[test]
    fn glb_bounds_apply_node_transforms() {
        let json = serde_json::json!({
            "scene": 0,
            "scenes": [{ "nodes": [0] }],
            "nodes": [{ "mesh": 0, "translation": [10.0, 0.0, -5.0], "scale": [2.0, 2.0, 2.0] }],
            "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 } }] }],
            "accessors": [{ "min": [-1.0, 0.0, -1.0], "max": [1.0, 3.0, 1.0] }]
        });
        let mut text = serde_json::to_vec(&json).unwrap();
        while text.len() % 4 != 0 {
            text.push(b' ');
        }
        let mut glb = Vec::new();
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&((20 + text.len()) as u32).to_le_bytes());
        glb.extend_from_slice(&(text.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"JSON");
        glb.extend_from_slice(&text);

        let dir = std::env::temp_dir().join(format!("renzora_wp_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("box.glb");
        std::fs::write(&path, &glb).unwrap();
        let (min, max) = model_file_bounds(&path).expect("bounds");
        let _ = std::fs::remove_dir_all(&dir);
        assert!((min - Vec3::new(8.0, 0.0, -7.0)).length() < 1e-4, "{min}");
        assert!((max - Vec3::new(12.0, 6.0, -3.0)).length() < 1e-4, "{max}");
    }
}
