//! Building world partition stand-ins (HLOD): each cell's contents merged into
//! one coarse, vertex-coloured mesh, saved beside the cell as `x_z.hlod.glb`.
//! The engine shows a cell's stand-in whenever its full contents are not on
//! screen (`renzora_engine::world_partition`), so nothing in the world ever
//! disappears, however far or high the camera goes.
//!
//! Everything is read from files, never from the editor's world: a cell does
//! not have to be loaded to get a stand-in, which is the point, since a city
//! is never loaded all at once. Per cell:
//!
//! 1. The cell's `.bsn` gives each model's path and its transform in the cell.
//! 2. Each model's `.rvg` gives its meshes at the coarse cut virtual geometry
//!    already built for ray tracing, so there is no simplification to redo
//!    per mesh; its glTF scene places each mesh and names its material.
//! 3. A mesh is coloured with the average of its base colour texture, read
//!    from the texture's smallest mip level (one 4 x 4 block), so no texture
//!    is ever decoded in full.
//! 4. The cell's meshes are merged and simplified together. City data is
//!    hundreds of tiny meshes per block (one per wall), each already as coarse
//!    as it can get alone; merged, the whole block simplifies as one surface.
//! 5. The result is written flat-shaded (each triangle its own vertices and
//!    normal), which keeps building edges sharp at a distance.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use renzora_log_api::*;
use renzora_scene_api::{MeshColor, MeshInstanceData, MeshPrimitive, PartitionCell, SceneInstance};
use renzora_material_api::MaterialRef;
use renzora_project_api::CurrentProject;
use renzora_bsn::bsn::{BsnSerializer, SceneSerializer};
use renzora_engine::world_partition as wp;

/// Triangle budget for one cell's stand-in: a sixth of what the cell's
/// coarse cut holds, within these bounds. A dense block keeps its skyline;
/// a cell of a few houses is not left with a handful of triangles. Stand-ins
/// draw as ordinary meshes, one per cell, so a few million triangles across a
/// city is well within budget.
const MIN_TRIANGLES: usize = 600;
const MAX_TRIANGLES: usize = 15000;
const REDUCTION: usize = 6;

/// How far, in metres, simplification may move the surface, tried in turn
/// until a cell fits twice its budget. Seen from a cell away a few metres is
/// below a pixel.
///
/// Capped at 8 m. Steps of 20 and 50 m used to follow for dense cells, and
/// with pruning on they fit the budget by deleting every building smaller than
/// the step: whole streets of houses were missing from their stand-in and
/// vanished the moment their cell unloaded. A dense cell now keeps more
/// triangles instead, which ordinary meshes draw cheaply at this count.
const SHAPE_ERRORS_M: [f32; 2] = [3.0, 8.0];

/// Colour for a mesh with no texture or colour of its own: light concrete,
/// which is what untextured city blocks are.
const DEFAULT_COLOUR: [f32; 3] = [0.62, 0.62, 0.6];

/// Something in a cell that draws, as the cell file describes it.
enum Piece {
    /// An imported model, by its path.
    Model(String),
    /// A library shape (`MeshPrimitive`): the shape id, its `MeshColor` in
    /// linear RGB, and the material file it wears, if any.
    Shape { id: String, colour: Option<[f32; 3]>, material: Option<String> },
}

/// Every library shape's geometry, positions and triangle indices, built once
/// on the main thread from the shape registry's factories (they take the
/// world's mesh store, which the worker threads cannot reach).
type ShapeGeometry = HashMap<String, (Vec<[f32; 3]>, Vec<u32>)>;

fn shape_geometry(world: &World) -> ShapeGeometry {
    let Some(registry) = world.get_resource::<renzora_scene_api::ShapeRegistry>() else {
        return HashMap::new();
    };
    let mut meshes = Assets::<Mesh>::default();
    let mut out = HashMap::new();
    for entry in registry.iter() {
        let handle = (entry.create_mesh)(&mut meshes);
        let Some(mesh) = meshes.get(&handle) else { continue };
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            continue;
        };
        let indices: Vec<u32> = match mesh.indices() {
            Some(i) => i.iter().map(|i| i as u32).collect(),
            None => (0..positions.len() as u32).collect(),
        };
        out.insert(entry.id.to_string(), (positions.clone(), indices));
    }
    out
}

/// One cell to build.
struct Job {
    cell: Entity,
    coord: IVec2,
    source_abs: PathBuf,
    out_abs: PathBuf,
}

/// The bake in flight, shared with its worker threads.
#[derive(Default)]
struct Progress {
    total: u32,
    done: u32,
    /// Cells finished: (cell, built). `false` means it had nothing to build.
    finished: Vec<(Entity, bool)>,
    errors: Vec<String>,
    complete: bool,
}

/// The running bake, if any.
#[derive(Resource, Default)]
pub(crate) struct HlodBake {
    progress: Option<Arc<Mutex<Progress>>>,
}

impl HlodBake {
    pub(crate) fn running(&self) -> bool {
        self.progress.is_some()
    }
}

/// Start building stand-ins for every cell of the scene's partition, on
/// background threads. Loaded cells are written back first, so a stand-in
/// matches what is on screen rather than the last save.
pub(crate) fn start_hlod_bake(world: &mut World) {
    if world.resource::<HlodBake>().running() {
        return;
    }
    let Some(project) = world.get_resource::<CurrentProject>().map(|p| p.path.clone()) else {
        return;
    };
    wp::write_back_loaded_cells(world);
    let jobs: Vec<Job> = {
        let mut q = world.query::<(Entity, &PartitionCell, &SceneInstance)>();
        q.iter(world)
            .map(|(cell, pc, instance)| Job {
                cell,
                coord: pc.coord(),
                source_abs: project.join(&instance.source),
                out_abs: project.join(PartitionCell::hlod_path(&instance.source)),
            })
            .collect()
    };
    if jobs.is_empty() {
        console_warn("World Partition", "No cells to build distant LODs for: partition the scene first");
        return;
    }
    let registry = world.resource::<AppTypeRegistry>().clone();
    let shapes = Arc::new(shape_geometry(world));
    let progress = Arc::new(Mutex::new(Progress {
        total: jobs.len() as u32,
        ..Default::default()
    }));
    world.resource_mut::<HlodBake>().progress = Some(progress.clone());
    console_info(
        "World Partition",
        format!("Building distant LODs for {} cells", jobs.len()),
    );

    std::thread::spawn(move || {
        let colours = Mutex::new(HashMap::<PathBuf, [f32; 3]>::new());
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 12);
        let queue = Mutex::new(jobs);
        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|| loop {
                    let Some(job) = queue.lock().unwrap().pop() else { break };
                    let result = build_cell(&job, &project, &registry, &shapes, &colours);
                    let mut p = progress.lock().unwrap();
                    p.done += 1;
                    match result {
                        Ok(built) => p.finished.push((job.cell, built)),
                        Err(e) => p.errors.push(format!("cell {}_{}: {e}", job.coord.x, job.coord.y)),
                    }
                });
            }
        });
        progress.lock().unwrap().complete = true;
    });
}

/// Report a running bake on the viewport's progress card, and when it ends
/// mark the cells that got a stand-in and swap in the new files.
pub(crate) fn poll_hlod_bake(world: &mut World) {
    let Some(progress) = world.resource::<HlodBake>().progress.clone() else {
        return;
    };
    let (done, total, complete) = {
        let p = progress.lock().unwrap();
        (p.done, p.total, p.complete)
    };
    if let Some(mut work) = world.get_resource_mut::<renzora_viewport_api::ViewportWork>() {
        let next = (!complete).then(|| renzora_viewport_api::WorkItem {
            id: "world_partition_hlod",
            label: "Building distant LODs".into(),
            done,
            total,
        });
        if work.differs("world_partition_hlod", next.as_ref()) {
            work.set("world_partition_hlod", next);
        }
    }
    if !complete {
        return;
    }
    world.resource_mut::<HlodBake>().progress = None;
    let p = std::mem::take(&mut *progress.lock().unwrap());

    let mut built = 0usize;
    let mut changed = false;
    for (cell, ok) in &p.finished {
        let Ok(mut e) = world.get_entity_mut(*cell) else { continue };
        let Some(mut pc) = e.get_mut::<PartitionCell>() else { continue };
        if pc.has_hlod != *ok {
            pc.has_hlod = *ok;
            changed = true;
        }
        if *ok {
            built += 1;
        }
        // A rebuilt stand-in is respawned from its new file: drop the old one
        // (the engine forgets it and spawns a fresh one next frame) and make
        // the asset server read the file again rather than reuse the asset.
        if let Some(old) = world.get::<wp::CellHlod>(*cell).map(|h| h.0) {
            if world.get_entity(old).is_ok() {
                world.despawn(old);
            }
            world.entity_mut(*cell).remove::<wp::CellHlod>();
        }
        let path = world
            .get::<SceneInstance>(*cell)
            .map(|s| PartitionCell::hlod_path(&s.source));
        if let (true, Some(path)) = (*ok, path) {
            world.resource::<AssetServer>().reload(path);
        }
    }
    for e in &p.errors {
        console_warn("World Partition", e.clone());
    }
    console_success(
        "World Partition",
        format!(
            "Built distant LODs for {built} of {} cells{}",
            p.total,
            if p.errors.is_empty() { String::new() } else { format!(", {} failed", p.errors.len()) }
        ),
    );
    if changed {
        if let Some(mut tabs) = world.get_resource_mut::<renzora_ui::DocumentTabState>() {
            let active = tabs.active_tab;
            if let Some(tab) = tabs.tabs.get_mut(active) {
                tab.is_modified = true;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// One cell
// ---------------------------------------------------------------------------

/// Build one cell's stand-in. `Ok(false)` when the cell holds nothing to
/// draw (no file, or no models).
fn build_cell(
    job: &Job,
    project: &Path,
    registry: &AppTypeRegistry,
    shapes: &ShapeGeometry,
    colours: &Mutex<HashMap<PathBuf, [f32; 3]>>,
) -> Result<bool, String> {
    let Ok(text) = std::fs::read_to_string(&job.source_abs) else {
        return Ok(false);
    };
    let pieces = cell_models(&text, registry)?;
    if pieces.is_empty() {
        let _ = std::fs::remove_file(&job.out_abs);
        return Ok(false);
    }

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut vertex_colours: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    for (piece, to_cell) in &pieces {
        match piece {
            Piece::Model(model_path) => {
                let abs = project.join(model_path);
                append_model(&abs, *to_cell, colours, &mut positions, &mut vertex_colours, &mut indices);
            }
            Piece::Shape { id, colour, material } => {
                let Some((shape_positions, shape_indices)) = shapes.get(id) else { continue };
                let colour = material
                    .as_ref()
                    .and_then(|m| material_colour(&project.join(m), colours))
                    .or(*colour)
                    .unwrap_or(DEFAULT_COLOUR);
                let base = positions.len() as u32;
                for p in shape_positions {
                    positions.push(to_cell.transform_point3(Vec3::from(*p)).to_array());
                    vertex_colours.push(colour);
                }
                indices.extend(shape_indices.iter().map(|i| base + i));
            }
        }
    }
    if indices.len() < 3 {
        let _ = std::fs::remove_file(&job.out_abs);
        return Ok(false);
    }

    let triangles = indices.len() / 3;
    let target = (triangles / REDUCTION).clamp(MIN_TRIANGLES, MAX_TRIANGLES).min(triangles);
    let simplified = if target < triangles {
        let bytes: &[u8] = bytemuck_cast(&positions);
        let adapter = meshopt::VertexDataAdapter::new(bytes, 12, 0).map_err(|e| e.to_string())?;
        // Shape-preserving only: it keeps a flat wall flat, moves the surface
        // by no more than the allowed error, and with `Prune` drops parts
        // smaller than that error whole. A cell still over budget gets a
        // larger allowed error, so what goes is its smallest buildings,
        // cleanly, and never a shattered one.
        //
        // There is deliberately no sloppy fallback. It reached any budget by
        // welding vertices across neighbouring buildings with no regard for
        // the surfaces between them, which turned dense residential cells
        // (thousands of separate boxes, none of which can lose a face) into
        // fields of slivers. A dense cell keeping more triangles is cheap; a
        // shattered one is visible from every hill in the city.
        let mut shaped = Vec::new();
        for error in SHAPE_ERRORS_M {
            shaped = meshopt::simplify(
                &indices,
                &adapter,
                target * 3,
                error,
                meshopt::SimplifyOptions::ErrorAbsolute | meshopt::SimplifyOptions::Prune,
                None,
            );
            if shaped.len() / 3 <= target * 2 {
                break;
            }
        }
        shaped
    } else {
        indices
    };

    let glb = write_glb(&positions, &vertex_colours, &simplified);
    if let Some(parent) = job.out_abs.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&job.out_abs, glb).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Every model and library shape in a cell file, with the matrix taking its
/// own space into the cell's. Entities without a parent in the file are the
/// cell's direct children, so their transform is relative to the cell.
///
/// Shapes count: a level blocked out in the editor is nothing but shapes, and
/// a partition of one used to get no stand-ins at all ("0 of 241 cells").
fn cell_models(text: &str, registry: &AppTypeRegistry) -> Result<Vec<(Piece, Mat4)>, String> {
    let (scene, _skipped) = {
        let reg = registry.read();
        BsnSerializer
            .deserialize_lossy(text, &reg)
            .map_err(|e| format!("cannot read cell file: {e}"))?
    };
    let mut locals: HashMap<Entity, Mat4> = HashMap::new();
    let mut parents: HashMap<Entity, Entity> = HashMap::new();
    let mut models: Vec<(Entity, Piece)> = Vec::new();
    for entity in &scene.entities {
        let mut local = Mat4::IDENTITY;
        let mut shape: Option<String> = None;
        let mut colour: Option<[f32; 3]> = None;
        let mut material: Option<String> = None;
        for component in &entity.components {
            let Some(info) = component.get_represented_type_info() else { continue };
            let path = info.type_path();
            if path == Transform::type_path() {
                if let Some(t) = <Transform as FromReflect>::from_reflect(component.as_ref()) {
                    local = t.to_matrix();
                }
            } else if path == ChildOf::type_path() {
                if let Some(c) = <ChildOf as FromReflect>::from_reflect(component.as_ref()) {
                    parents.insert(entity.entity, c.parent());
                }
            } else if path == MeshInstanceData::type_path() {
                if let Some(Some(model)) =
                    <MeshInstanceData as FromReflect>::from_reflect(component.as_ref()).map(|m| m.model_path)
                {
                    models.push((entity.entity, Piece::Model(model)));
                }
            } else if path == MeshPrimitive::type_path() {
                shape = <MeshPrimitive as FromReflect>::from_reflect(component.as_ref()).map(|p| p.0);
            } else if path == MeshColor::type_path() {
                colour = <MeshColor as FromReflect>::from_reflect(component.as_ref()).map(|c| {
                    let c = c.0.to_linear();
                    [c.red, c.green, c.blue]
                });
            } else if path == MaterialRef::type_path() {
                material = <MaterialRef as FromReflect>::from_reflect(component.as_ref()).map(|m| m.0);
            }
        }
        if let Some(id) = shape {
            models.push((entity.entity, Piece::Shape { id, colour, material }));
        }
        locals.insert(entity.entity, local);
    }
    Ok(models
        .into_iter()
        .map(|(entity, path)| {
            let mut m = Mat4::IDENTITY;
            let mut cursor = Some(entity);
            let mut depth = 0;
            while let Some(e) = cursor.filter(|_| depth < 128) {
                m = locals.get(&e).copied().unwrap_or(Mat4::IDENTITY) * m;
                cursor = parents.get(&e).copied();
                depth += 1;
            }
            (path, m)
        })
        .collect())
}

/// Append one model's meshes, at their coarse cut, coloured and placed in the
/// cell. Models that cannot be read are skipped: a stand-in missing one
/// building is better than no stand-in.
fn append_model(
    path: &Path,
    to_cell: Mat4,
    colours: &Mutex<HashMap<PathBuf, [f32; 3]>>,
    positions: &mut Vec<[f32; 3]>,
    vertex_colours: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
) {
    let Ok(bytes) = std::fs::read(path) else { return };
    let Ok(file) = renzora_rvg::RvgFile::from_bytes(&bytes) else { return };
    let json = wp::model_json(&bytes, true);
    let placements = json.as_ref().map(wp::mesh_placements).unwrap_or_default();
    let model_dir = path.parent().unwrap_or(Path::new("."));

    for mesh in &file.meshes {
        let cut = mesh.proxy_indices();
        if cut.is_empty() {
            continue;
        }
        let colour = json
            .as_ref()
            .map(|j| mesh_colour(j, mesh.mesh_index as usize, mesh.primitive_index as usize, model_dir, colours))
            .unwrap_or(DEFAULT_COLOUR);
        let matrices = placements
            .get(&(mesh.mesh_index as usize))
            .cloned()
            .unwrap_or_else(|| vec![Mat4::IDENTITY]);
        for m in matrices {
            let to = to_cell * m;
            let base = positions.len() as u32;
            for p in &mesh.positions {
                positions.push(to.transform_point3(Vec3::from(*p)).to_array());
                vertex_colours.push(colour);
            }
            indices.extend(cut.iter().map(|i| base + i));
        }
    }
}

/// The colour a mesh shows from afar: the average of its material's base
/// colour texture (from the importer's `.material` file), else its glTF base
/// colour, else concrete. Linear RGB.
fn mesh_colour(
    json: &serde_json::Value,
    mesh_index: usize,
    primitive_index: usize,
    model_dir: &Path,
    colours: &Mutex<HashMap<PathBuf, [f32; 3]>>,
) -> [f32; 3] {
    let material = json
        .get("meshes")
        .and_then(|m| m.get(mesh_index))
        .and_then(|m| m.get("primitives"))
        .and_then(|p| p.get(primitive_index))
        .and_then(|p| p.get("material"))
        .and_then(|m| m.as_u64())
        .and_then(|i| json.get("materials")?.get(i as usize));
    let Some(material) = material else {
        return DEFAULT_COLOUR;
    };
    let factor = material
        .get("pbrMetallicRoughness")
        .and_then(|p| p.get("baseColorFactor"))
        .and_then(|f| f.as_array())
        .and_then(|f| {
            let v: Vec<f32> = f.iter().filter_map(|x| x.as_f64().map(|x| x as f32)).collect();
            (v.len() >= 3).then(|| [v[0], v[1], v[2]])
        });
    let texture = material
        .get("name")
        .and_then(|n| n.as_str())
        .and_then(|name| material_texture(model_dir, name));
    let tex_colour = texture.and_then(|t| {
        if let Some(c) = colours.lock().unwrap().get(&t) {
            return Some(*c);
        }
        let c = texture_average(&t)?;
        colours.lock().unwrap().insert(t, c);
        Some(c)
    });
    match (tex_colour, factor) {
        (Some(t), Some(f)) => [t[0] * f[0], t[1] * f[1], t[2] * f[2]],
        (Some(t), None) => t,
        (None, Some(f)) => f,
        (None, None) => DEFAULT_COLOUR,
    }
}

/// The colour a shape's `.material` shows from afar, linear RGB: the surface
/// output's base colour when it is set there, otherwise the first colour any
/// node of the graph holds (a graph that computes its colour usually takes it
/// as an input), otherwise the average of the first texture it samples.
/// `None` when the graph has none of these, and the shape's own colour is
/// used.
fn material_colour(path: &Path, cache: &Mutex<HashMap<PathBuf, [f32; 3]>>) -> Option<[f32; 3]> {
    if let Some(c) = cache.lock().unwrap().get(path) {
        return Some(*c);
    }
    let text = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let nodes = json.get("nodes")?.as_array()?;
    let colour_of = |v: &serde_json::Value| -> Option<[f32; 3]> {
        let c = v.get("Color")?.as_array()?;
        let c: Vec<f32> = c.iter().filter_map(|x| x.as_f64().map(|x| x as f32)).collect();
        (c.len() >= 3).then(|| [c[0], c[1], c[2]])
    };
    let output = nodes
        .iter()
        .find(|n| n.get("node_type").and_then(|t| t.as_str()).is_some_and(|t| t.starts_with("output/")))
        .and_then(|n| n.get("input_values")?.get("base_color"))
        .and_then(colour_of);
    let any = || {
        nodes.iter().find_map(|n| n.get("input_values")?.as_object()?.values().find_map(colour_of))
    };
    let texture = || {
        let rel = nodes.iter().find_map(|n| {
            let values = n.get("input_values")?.as_object()?;
            values.values().find_map(|v| v.get("TexturePath")?.as_str().map(str::to_string))
        })?;
        let mut dir = path.parent();
        while let Some(d) = dir {
            if d.join(&rel).exists() {
                return texture_average(&d.join(&rel));
            }
            dir = d.parent();
        }
        None
    };
    let colour = output.or_else(any).or_else(texture)?;
    cache.lock().unwrap().insert(path.to_path_buf(), colour);
    Some(colour)
}

/// The first texture a `.material` graph samples, as an absolute path. The
/// importer writes one `.material` per glTF material, named after it, into
/// `materials/` beside the model; texture paths in it are project-relative.
fn material_texture(model_dir: &Path, material_name: &str) -> Option<PathBuf> {
    let file = model_dir
        .join("materials")
        .join(format!("{}.material", sanitize(material_name)));
    let text = std::fs::read_to_string(file).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let texture = json.get("nodes")?.as_array()?.iter().find_map(|n| {
        let values = n.get("input_values")?.as_object()?;
        values.values().find_map(|v| v.get("TexturePath")?.as_str().map(str::to_string))
    })?;
    // Project-relative: the models folder sits under the project root, so
    // walk up from the model to the directory the path is relative to.
    let mut root = model_dir;
    while let Some(parent) = root.parent() {
        if parent.join(&texture).exists() {
            return Some(parent.join(&texture));
        }
        root = parent;
    }
    None
}

/// The importer's material file name rule, so the lookup finds what it wrote.
fn sanitize(name: &str) -> String {
    renzora_engine::material_binding::sanitize_material_name(name)
}

/// The average colour of an `.rmip`, from its smallest mip level, in linear
/// RGB. Block-compressed levels are one 4 x 4 block at that size.
fn texture_average(path: &Path) -> Option<[f32; 3]> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let mut header = [0u8; 24];
    file.read_exact(&mut header).ok()?;
    if &header[0..4] != b"RMIP" {
        return None;
    }
    let u = |o: usize| u32::from_le_bytes(header[o..o + 4].try_into().unwrap());
    let (width, height, mips, code) = (u(8), u(12), u(16), u(20));
    let format = renzora_rmip::RmipFormat::from_code(code)?;
    let last = mips.max(1) - 1;
    let offset: usize = (0..last)
        .map(|l| format.level_byte_size((width >> l).max(1), (height >> l).max(1)))
        .sum();
    let (w, h) = ((width >> last).max(1), (height >> last).max(1));
    let size = format.level_byte_size(w, h);
    file.seek(SeekFrom::Start(24 + offset as u64)).ok()?;
    let mut level = vec![0u8; size];
    file.read_exact(&mut level).ok()?;

    use renzora_rmip::RmipFormat as F;
    let mut rgba = [0u8; 64];
    let pixels: &[u8] = match format {
        F::Rgba8UnormSrgb | F::Rgba8Unorm => &level,
        F::Bc7RgbaUnormSrgb | F::Bc7RgbaUnorm => {
            bcdec_rs::bc7(level.get(..16)?, &mut rgba, 16);
            &rgba
        }
        F::Bc1RgbaUnormSrgb | F::Bc1RgbaUnorm => {
            bcdec_rs::bc1(level.get(..8)?, &mut rgba, 16);
            &rgba
        }
        F::Bc3RgbaUnormSrgb | F::Bc3RgbaUnorm => {
            bcdec_rs::bc3(level.get(..16)?, &mut rgba, 16);
            &rgba
        }
        // Normal maps and single channels are not colour.
        _ => return None,
    };
    // A level below 4 x 4 pads its block: average only the real pixels.
    let (pw, ph) = if format.is_block_compressed() { (w.min(4), h.min(4)) } else { (w, h) };
    let stride = if format.is_block_compressed() { 4 } else { w };
    let mut sum = [0f32; 3];
    let mut n = 0f32;
    for y in 0..ph {
        for x in 0..pw {
            let i = ((y * stride + x) * 4) as usize;
            let Some(px) = pixels.get(i..i + 3) else { continue };
            for c in 0..3 {
                sum[c] += px[c] as f32 / 255.0;
            }
            n += 1.0;
        }
    }
    if n == 0.0 {
        return None;
    }
    let avg = [sum[0] / n, sum[1] / n, sum[2] / n];
    Some(if format.is_srgb() { avg.map(srgb_to_linear) } else { avg })
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn bytemuck_cast(v: &[[f32; 3]]) -> &[u8] {
    // SAFETY: `[f32; 3]` is plain data with no padding; reading it as bytes
    // is how meshoptimizer's adapter takes vertex positions.
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
}

// ---------------------------------------------------------------------------
// Writing the stand-in
// ---------------------------------------------------------------------------

/// A GLB of flat-shaded triangles with vertex colours and one plain material.
/// Each triangle gets its own three vertices and its face normal, so building
/// edges stay sharp; degenerate triangles from the simplifier are dropped.
pub(crate) fn write_glb(positions: &[[f32; 3]], colours: &[[f32; 3]], indices: &[u32]) -> Vec<u8> {
    let mut pos: Vec<f32> = Vec::with_capacity(indices.len() * 3);
    let mut nrm: Vec<f32> = Vec::with_capacity(indices.len() * 3);
    let mut col: Vec<u8> = Vec::with_capacity(indices.len() * 4);
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for tri in indices.as_chunks::<3>().0 {
        let p = [0, 1, 2].map(|k| Vec3::from(positions[tri[k] as usize]));
        let n = (p[1] - p[0]).cross(p[2] - p[0]);
        if n.length_squared() < 1e-12 {
            continue;
        }
        let n = n.normalize();
        for k in 0..3 {
            pos.extend_from_slice(&p[k].to_array());
            nrm.extend_from_slice(&n.to_array());
            let c = colours[tri[k] as usize];
            col.extend_from_slice(&[
                (c[0].clamp(0.0, 1.0) * 255.0).round() as u8,
                (c[1].clamp(0.0, 1.0) * 255.0).round() as u8,
                (c[2].clamp(0.0, 1.0) * 255.0).round() as u8,
                255,
            ]);
            min = min.min(p[k]);
            max = max.max(p[k]);
        }
    }
    let count = pos.len() / 3;
    let mut bin: Vec<u8> = Vec::new();
    let pos_off = bin.len();
    bin.extend(pos.iter().flat_map(|f| f.to_le_bytes()));
    let nrm_off = bin.len();
    bin.extend(nrm.iter().flat_map(|f| f.to_le_bytes()));
    let col_off = bin.len();
    bin.extend_from_slice(&col);
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    if count == 0 {
        min = Vec3::ZERO;
        max = Vec3::ZERO;
    }

    let json = serde_json::json!({
        "asset": { "version": "2.0", "generator": "Renzora world partition HLOD" },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "mesh": 0, "name": "hlod" }],
        "meshes": [{ "primitives": [{
            "attributes": { "POSITION": 0, "NORMAL": 1, "COLOR_0": 2 },
            "material": 0,
            "mode": 4
        }]}],
        // Double-sided: city data does not wind its triangles consistently
        // (a PLATEAU block had a third of its roofs facing down), and a face
        // normal taken from a backwards triangle points into the building.
        // Lit one-sided, those faces came out nearly black. Double-sided, the
        // renderer turns each normal towards the camera, whatever the winding.
        "materials": [{
            "name": "hlod",
            "doubleSided": true,
            "pbrMetallicRoughness": {
                "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                "metallicFactor": 0.0,
                "roughnessFactor": 0.9
            }
        }],
        "buffers": [{ "byteLength": bin.len() }],
        "bufferViews": [
            { "buffer": 0, "byteOffset": pos_off, "byteLength": count * 12 },
            { "buffer": 0, "byteOffset": nrm_off, "byteLength": count * 12 },
            { "buffer": 0, "byteOffset": col_off, "byteLength": count * 4 }
        ],
        "accessors": [
            { "bufferView": 0, "componentType": 5126, "count": count, "type": "VEC3",
              "min": min.to_array(), "max": max.to_array() },
            { "bufferView": 1, "componentType": 5126, "count": count, "type": "VEC3" },
            { "bufferView": 2, "componentType": 5121, "normalized": true, "count": count, "type": "VEC4" }
        ]
    });
    let mut text = serde_json::to_vec(&json).unwrap_or_default();
    while !text.len().is_multiple_of(4) {
        text.push(b' ');
    }
    let total = 12 + 8 + text.len() + 8 + bin.len();
    let mut glb = Vec::with_capacity(total);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&(total as u32).to_le_bytes());
    glb.extend_from_slice(&(text.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(&text);
    glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"BIN\0");
    glb.extend_from_slice(&bin);
    glb
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A written stand-in must be a GLB the engine's own bounds reader can
    /// parse, with its bounds where the triangles are.
    #[test]
    fn written_stand_ins_are_valid_glb() {
        let positions = [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 5.0, 0.0], [0.0, 0.0, 0.0]];
        let colours = [[1.0, 0.0, 0.0]; 4];
        // The second triangle is degenerate and must be dropped.
        let glb = write_glb(&positions, &colours, &[0, 1, 2, 0, 0, 3]);
        assert_eq!(&glb[0..4], b"glTF");
        assert_eq!(u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize, glb.len());
        let dir = std::env::temp_dir().join(format!("renzora_hlod_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cell.glb");
        std::fs::write(&path, &glb).unwrap();
        let (min, max) = wp::model_file_bounds(&path).expect("bounds");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!((min, max), (Vec3::ZERO, Vec3::new(10.0, 5.0, 0.0)));
    }

    /// A cell holding only library shapes has something to draw: the shapes,
    /// placed by their transform and coloured by `MeshColor`.
    #[test]
    fn shapes_in_a_cell_file_are_found() {
        let registry = AppTypeRegistry::default();
        {
            let mut r = registry.write();
            r.register::<Transform>();
            r.register::<Name>();
            r.register::<MeshPrimitive>();
            r.register::<MeshColor>();
            r.register::<MaterialRef>();
        }
        let mut world = World::new();
        world.insert_resource(registry.clone());
        let e = world
            .spawn((
                Name::new("crate"),
                Transform::from_xyz(4.0, 0.0, 0.0),
                MeshPrimitive("cube".into()),
                MeshColor(Color::linear_rgb(1.0, 0.0, 0.0)),
            ))
            .id();
        let text = {
            let reg = registry.read();
            let scene = renzora_bsn::DynamicSceneBuilder::from_world(&world).extract_entity(e).build();
            BsnSerializer.serialize(&scene, &reg).expect("serialize")
        };
        let pieces = cell_models(&text, &registry).expect("read");
        assert_eq!(pieces.len(), 1);
        let (piece, m) = &pieces[0];
        match piece {
            Piece::Shape { id, colour, material } => {
                assert_eq!(id, "cube");
                assert_eq!(*colour, Some([1.0, 0.0, 0.0]));
                assert!(material.is_none());
            }
            Piece::Model(_) => panic!("a shape read as a model"),
        }
        assert_eq!(m.transform_point3(Vec3::ZERO), Vec3::new(4.0, 0.0, 0.0));
    }

    #[test]
    fn srgb_white_and_black_stay_put() {
        assert_eq!(srgb_to_linear(0.0), 0.0);
        assert!((srgb_to_linear(1.0) - 1.0).abs() < 1e-6);
        assert!(srgb_to_linear(0.5) < 0.25);
    }
}
