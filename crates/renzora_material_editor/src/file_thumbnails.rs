//! Offscreen capture of `.material` file thumbnails for the asset browser.
//!
//! Architecture: ephemeral, one-shot. A request produces a single capture:
//!
//! 1. If a PNG cache hit exists on disk → load it as an egui image, publish
//!    the `TextureId` to the registry, done.
//! 2. Miss → spawn a render-target `Image`, camera, sphere + lights on a
//!    dedicated render layer. Preload any texture assets the material binds,
//!    and wait until they're loaded so the sphere doesn't capture with
//!    fallback textures.
//! 3. Once assets are ready, attach a `Screenshot::image(handle)` component.
//!    Its observer writes the PNG to disk, registers the captured image with
//!    egui, publishes the `TextureId`, and despawns all capture entities.
//!
//! No persistent cameras or slots — cost is paid only when a thumbnail is
//! actually requested, and is reclaimed immediately after.

use std::marker::PhantomData;
use std::path::PathBuf;

use bevy::asset::LoadState;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy_egui::{EguiTextureHandle, EguiUserTextures};
use uuid::Uuid;

use renzora::core::{CurrentProject, EditorLocked, HideInHierarchy, IsolatedCamera};
use renzora_editor_framework::{material_thumb_path, MaterialThumbnailRegistry};
use renzora_shader::material::codegen::{self, CompileResult, TextureKind};
use renzora_shader::material::graph::{MaterialDomain, MaterialGraph};
use renzora_shader::material::runtime::{new_graph_material, FallbackTexture, GraphMaterial};
use renzora_splash::{LoadingTaskHandle, LoadingTasks, SplashState};

pub const MATERIAL_THUMBNAIL_LAYER: usize = 7;
/// Off-screen render target resolution. The asset browser displays
/// thumbnails at roughly 96px, but rendering at higher resolution gives
/// noticeably cleaner detail (normals, silhouettes, fine procedural noise)
/// when the browser scales the PNG down.
const THUMB_SIZE: u32 = 256;
const MAX_INTAKE_PER_FRAME: usize = 8;
/// Multiple captures run in parallel so their shader pipelines compile
/// concurrently during the warmup window. The 2 shared directional lights
/// (spawned once at startup) illuminate all of them on the thumbnail layer,
/// so the 10-directional-light Bevy limit isn't an issue.
const MAX_CONCURRENT_CAPTURES: usize = 8;
/// World-space spacing between simultaneous captures. Each armed capture
/// gets its own cell so cameras don't see each other's spheres — but the
/// cells stay close to origin to keep float precision comfortable.
const CAPTURE_CELL_SPACING: f32 = 40.0;
const ASSET_LOAD_TIMEOUT_FRAMES: u32 = 600; // ~10s at 60fps

struct CaptureJob {
    material_path: PathBuf,
    thumb_path: PathBuf,
    /// `None` when the graph failed to parse / compile / is a non-Surface
    /// domain — we still render a fallback grey sphere in that case so the
    /// asset browser gets *some* thumbnail rather than the default pink icon.
    compile: Option<CompileResult>,
    texture_handles: Vec<Handle<Image>>,
    waited_frames: u32,
}

#[derive(Resource, Default)]
struct PendingCaptures {
    jobs: Vec<CaptureJob>,
    active: usize,
}

/// Disk-cached PNGs whose `Handle<Image>` is still loading — we can only
/// register with egui once the GPU texture is ready.
#[derive(Resource, Default)]
struct PendingDiskLoads {
    entries: Vec<PendingDiskLoad>,
}

struct PendingDiskLoad {
    material_path: PathBuf,
    handle: Handle<Image>,
}

/// Tracks the `LoadingTasks` handle for the bulk-generate pass that runs
/// during `SplashState::Loading`. Present only while that pass is active.
#[derive(Resource)]
struct MaterialThumbnailLoadingJob {
    handle: LoadingTaskHandle,
}

/// Frames to wait between spawning a capture's camera/sphere/lights and
/// actually triggering the `Screenshot`.
///
/// When `shader_uuid` is set but the render-world pipeline hasn't finished
/// specializing + compiling the WGSL yet, `ExtendedMaterial` falls back to
/// the default StandardMaterial shader and the sphere renders as plain
/// white. Complex procedural shaders (fbm / voronoi / plasma / etc.) can
/// take well over half a second to compile, so we err generous here.
///
/// 60 frames ≈ 1 second at 60fps — long enough in practice, still cheap
/// because captures are serial and each material needs it only once per
/// session (subsequent sessions hit the PNG disk cache).
const CAPTURE_WARMUP_FRAMES: u32 = 60;

/// Tracks which capture cells (world-space slots) are in use so concurrent
/// captures don't collide on the same sphere position.
#[derive(Resource, Default)]
struct CaptureCells {
    /// Bit mask — bit `i` set ⇒ cell `i` is in use.
    busy_mask: u64,
}

impl CaptureCells {
    fn alloc(&mut self) -> Option<usize> {
        for i in 0..MAX_CONCURRENT_CAPTURES {
            let bit = 1u64 << i;
            if self.busy_mask & bit == 0 {
                self.busy_mask |= bit;
                return Some(i);
            }
        }
        None
    }
    fn free(&mut self, idx: usize) {
        if idx < 64 {
            self.busy_mask &= !(1u64 << idx);
        }
    }
    fn cell_center(idx: usize) -> Vec3 {
        Vec3::new(idx as f32 * CAPTURE_CELL_SPACING, 0.0, 0.0)
    }
}

/// A capture whose entities have been spawned but whose `Screenshot` has not
/// yet been dispatched — we're burning `frames_remaining` frames to give the
/// render pipeline a chance to compile the shader & upload textures.
struct ArmedCapture {
    material_path: PathBuf,
    thumb_path: PathBuf,
    render_image: Handle<Image>,
    capture_entities: Vec<Entity>,
    cell_idx: usize,
    frames_remaining: u32,
}

#[derive(Resource, Default)]
struct ArmedCaptures {
    list: Vec<ArmedCapture>,
}

/// Emit a structured line to stdout that the parent splash process parses
/// to drive its progress UI. Format: `[progress] thumbnails <done>/<total> <name>`.
fn emit_thumbnail_progress(done: u32, total: u32, name: &str) {
    use std::io::Write;
    println!("[progress] thumbnails {}/{} {}", done, total, name);
    let _ = std::io::stdout().flush();
}

fn intake_thumbnail_requests(
    mut registry: ResMut<MaterialThumbnailRegistry>,
    mut pending: ResMut<PendingCaptures>,
    mut disk_loads: ResMut<PendingDiskLoads>,
    mut tasks: ResMut<LoadingTasks>,
    asset_server: Res<AssetServer>,
    project: Option<Res<CurrentProject>>,
    job: Option<Res<MaterialThumbnailLoadingJob>>,
) {
    let advance_skip = |tasks: &mut LoadingTasks, job: &Option<Res<MaterialThumbnailLoadingJob>>, name: &str| {
        if let Some(j) = job.as_ref() {
            tasks.advance(j.handle, 1);
            if let Some(t) = tasks.tasks().iter().find(|(h, _)| *h == j.handle).map(|(_, t)| t) {
                emit_thumbnail_progress(t.completed, t.total, name);
            }
        }
    };

    for _ in 0..MAX_INTAKE_PER_FRAME {
        let Some(material_path) = registry.incoming_requests.pop_front() else {
            break;
        };

        let short_name = material_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let Some(project) = project.as_ref() else {
            registry.cancel(&material_path);
            advance_skip(&mut tasks, &job, &short_name);
            continue;
        };

        let thumb_path = material_thumb_path(&material_path, project);

        // Disk cache hit: kick off an async load; register with egui when ready.
        if thumb_path.exists() {
            if let Some(handle) = try_load_cached_thumb(&thumb_path, &asset_server, project) {
                disk_loads.entries.push(PendingDiskLoad {
                    material_path,
                    handle,
                });
                continue;
            }
        }

        // Miss — prepare a capture job. If anything goes wrong (bad JSON,
        // compile errors, non-Surface domain), we still push a job with
        // `compile: None`; the render path falls back to a plain default
        // material sphere so every `.material` gets *some* thumbnail.
        let compile_opt: Option<CompileResult> = match std::fs::read_to_string(&material_path) {
            Ok(content) => match serde_json::from_str::<MaterialGraph>(&content) {
                Ok(graph) if graph.domain == MaterialDomain::Surface => {
                    let result = codegen::compile(&graph);
                    if result.errors.is_empty() {
                        Some(result)
                    } else {
                        for err in &result.errors {
                            warn!(
                                "[material_thumbnails] Compile error in '{}': {} — using fallback",
                                material_path.display(),
                                err
                            );
                        }
                        None
                    }
                }
                Ok(graph) => {
                    info!(
                        "[material_thumbnails] '{}' is {:?} domain, using fallback",
                        material_path.display(),
                        graph.domain
                    );
                    None
                }
                Err(e) => {
                    warn!(
                        "[material_thumbnails] Parse error in '{}': {} — using fallback",
                        material_path.display(),
                        e
                    );
                    None
                }
            },
            Err(e) => {
                warn!(
                    "[material_thumbnails] Cannot read '{}': {}",
                    material_path.display(),
                    e
                );
                registry.cancel(&material_path);
                advance_skip(&mut tasks, &job, &short_name);
                continue;
            }
        };

        // Preload texture assets (only for compiled graphs). Without a
        // compiled shader there are no bindings anyway.
        let texture_handles: Vec<Handle<Image>> = compile_opt
            .as_ref()
            .map(|c| {
                c.texture_bindings
                    .iter()
                    .filter(|tb| !tb.asset_path.is_empty())
                    .map(|tb| asset_server.load(&tb.asset_path))
                    .collect()
            })
            .unwrap_or_default();

        info!(
            "[material_thumbnails] Queued '{}' ({})",
            material_path.display(),
            if compile_opt.is_some() { "compiled" } else { "fallback" }
        );

        pending.jobs.push(CaptureJob {
            material_path,
            thumb_path,
            compile: compile_opt,
            texture_handles,
            waited_frames: 0,
        });
    }
}

fn try_load_cached_thumb(
    thumb_path: &std::path::Path,
    asset_server: &AssetServer,
    project: &CurrentProject,
) -> Option<Handle<Image>> {
    // Use the asset server's reader so the image goes through the normal
    // pipeline (format conversion, etc.). Needs asset-relative path.
    let rel = project.make_asset_relative(thumb_path);
    // If make_asset_relative failed (path outside project), bail — we'd pass
    // an absolute path to asset_server.load which won't resolve.
    if std::path::Path::new(&rel).is_absolute() {
        return None;
    }
    Some(asset_server.load(rel))
}

fn run_pending_captures(
    mut commands: Commands,
    mut pending: ResMut<PendingCaptures>,
    mut armed: ResMut<ArmedCaptures>,
    mut cells: ResMut<CaptureCells>,
    mut materials: ResMut<Assets<GraphMaterial>>,
    mut shaders: ResMut<Assets<Shader>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut tasks: ResMut<LoadingTasks>,
    loading_job: Option<Res<MaterialThumbnailLoadingJob>>,
    asset_server: Res<AssetServer>,
    fallback: Res<FallbackTexture>,
) {
    // Loop to arm multiple captures per frame when slots are available.
    for _ in 0..MAX_CONCURRENT_CAPTURES {
        if pending.active >= MAX_CONCURRENT_CAPTURES {
            return;
        }
        arm_one_capture(
            &mut commands,
            &mut pending,
            &mut armed,
            &mut cells,
            &mut materials,
            &mut shaders,
            &mut meshes,
            &mut images,
            &mut tasks,
            &loading_job,
            &asset_server,
            &fallback,
        );
    }
}

fn arm_one_capture(
    commands: &mut Commands,
    pending: &mut PendingCaptures,
    armed: &mut ArmedCaptures,
    cells: &mut CaptureCells,
    materials: &mut Assets<GraphMaterial>,
    shaders: &mut Assets<Shader>,
    meshes: &mut Assets<Mesh>,
    images: &mut Assets<Image>,
    _tasks: &mut LoadingTasks,
    _loading_job: &Option<Res<MaterialThumbnailLoadingJob>>,
    asset_server: &AssetServer,
    fallback: &FallbackTexture,
) {
    if pending.active >= MAX_CONCURRENT_CAPTURES {
        return;
    }

    // Pull the next job that has all its textures loaded (or that has waited
    // long enough that we should capture anyway with fallbacks).
    let mut take_index: Option<usize> = None;
    for (i, job) in pending.jobs.iter_mut().enumerate() {
        let all_loaded = job.texture_handles.iter().all(|h| {
            matches!(
                asset_server.get_load_state(h),
                Some(LoadState::Loaded) | Some(LoadState::Failed(_))
            )
        });
        job.waited_frames += 1;
        if all_loaded || job.waited_frames > ASSET_LOAD_TIMEOUT_FRAMES {
            take_index = Some(i);
            break;
        }
    }

    let Some(i) = take_index else { return };

    // Allocate a unique world-space cell for this capture.
    let Some(cell_idx) = cells.alloc() else {
        // All cells busy — will try again next frame.
        return;
    };

    let job = pending.jobs.swap_remove(i);
    let cell_origin = CaptureCells::cell_center(cell_idx);

    // Build the render-target image that this capture renders into.
    let extent = Extent3d {
        width: THUMB_SIZE,
        height: THUMB_SIZE,
        depth_or_array_layers: 1,
    };
    let mut render_image = Image {
        data: Some(vec![0u8; (extent.width * extent.height * 4) as usize]),
        ..default()
    };
    render_image.texture_descriptor.size = extent;
    render_image.texture_descriptor.format = TextureFormat::Bgra8UnormSrgb;
    render_image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_DST
        | TextureUsages::COPY_SRC
        | TextureUsages::RENDER_ATTACHMENT;
    let render_image_handle = images.add(render_image);

    // Build the graph material. If the graph couldn't compile we skip the
    // shader + texture setup entirely — the material falls back to the
    // default SurfaceGraphExt shader (StandardMaterial pass-through), which
    // renders a plain lit sphere rather than a broken/black shader.
    let material_handle = {
        let mut mat = new_graph_material(fallback);
        let fb = &fallback.0;
        mat.extension.texture_0 = Some(fb.clone());
        mat.extension.texture_1 = Some(fb.clone());
        mat.extension.texture_2 = Some(fb.clone());
        mat.extension.texture_3 = Some(fb.clone());
        mat.extension.cube_0 = None;
        mat.extension.array_0 = None;
        mat.extension.volume_0 = None;

        if let Some(compile) = &job.compile {
            for tb in &compile.texture_bindings {
                if tb.asset_path.is_empty() {
                    continue;
                }
                let h: Handle<Image> = asset_server.load(&tb.asset_path);
                match (tb.kind, tb.binding) {
                    (TextureKind::D2, 0) => mat.extension.texture_0 = Some(h),
                    (TextureKind::D2, 1) => mat.extension.texture_1 = Some(h),
                    (TextureKind::D2, 2) => mat.extension.texture_2 = Some(h),
                    (TextureKind::D2, 3) => mat.extension.texture_3 = Some(h),
                    (TextureKind::Cube, 0) => mat.extension.cube_0 = Some(h),
                    (TextureKind::D2Array, 0) => mat.extension.array_0 = Some(h),
                    (TextureKind::D3, 0) => mat.extension.volume_0 = Some(h),
                    _ => {}
                }
            }

            let uuid = Uuid::new_v4();
            let shader_handle: Handle<Shader> = Handle::Uuid(uuid, PhantomData);
            let shader = Shader::from_wgsl(
                compile.fragment_shader.clone(),
                "graph_material://thumbnail",
            );
            let _ = shaders.insert(&shader_handle, shader);
            mat.extension.shader_uuid = Some(uuid);
        }

        materials.add(mat)
    };

    // All captures live at the world origin — MAX_CONCURRENT_CAPTURES = 1
    // means there's never another capture's sphere to catch accidentally,
    // and placing entities close to (0,0,0) avoids the float-precision
    // problems we'd see at huge world-space offsets.
    let sphere_mesh = meshes.add(Sphere::new(1.0).mesh().ico(5).unwrap());

    let sphere = commands
        .spawn((
            Mesh3d(sphere_mesh),
            MeshMaterial3d(material_handle),
            Transform::from_translation(cell_origin),
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
            ViewVisibility::default(),
            RenderLayers::layer(MATERIAL_THUMBNAIL_LAYER),
            HideInHierarchy,
            EditorLocked,
            Name::new(format!("Material Thumbnail Sphere ({})", job.material_path.display())),
        ))
        .id();

    let camera = commands
        .spawn((
            Camera3d::default(),
            // MSAA 4× — the thumbnail is a small 1-object render with high
            // contrast between the lit sphere and the flat background, so
            // silhouette aliasing is very visible at 128px.
            Msaa::Sample4,
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgba(0.08, 0.08, 0.1, 1.0)),
                order: -1000 - (cell_idx as isize),
                is_active: true,
                ..default()
            },
            RenderTarget::Image(render_image_handle.clone().into()),
            Transform::from_translation(cell_origin + Vec3::new(0.0, 1.0, 3.0))
                .looking_at(cell_origin, Vec3::Y),
            RenderLayers::layer(MATERIAL_THUMBNAIL_LAYER),
            IsolatedCamera,
            HideInHierarchy,
            EditorLocked,
            Name::new("Material Thumbnail Camera"),
        ))
        .id();

    pending.active += 1;
    info!(
        "[material_thumbnails] Armed capture for '{}' (cell {}) — warmup {} frames",
        job.material_path.display(),
        cell_idx,
        CAPTURE_WARMUP_FRAMES
    );

    // Defer the `Screenshot` spawn — let the render pipeline compile the
    // per-material shader and upload its textures first. `fire_armed_captures`
    // ticks these down each frame and dispatches the Screenshot when ready.
    armed.list.push(ArmedCapture {
        material_path: job.material_path.clone(),
        thumb_path: job.thumb_path.clone(),
        render_image: render_image_handle,
        capture_entities: vec![sphere, camera],
        cell_idx,
        frames_remaining: CAPTURE_WARMUP_FRAMES,
    });
}

fn fire_armed_captures(
    mut commands: Commands,
    mut armed: ResMut<ArmedCaptures>,
) {
    // Countdown, then pop-and-fire.
    for entry in armed.list.iter_mut() {
        if entry.frames_remaining > 0 {
            entry.frames_remaining -= 1;
        }
    }
    let mut i = 0;
    while i < armed.list.len() {
        if armed.list[i].frames_remaining > 0 {
            i += 1;
            continue;
        }
        let entry = armed.list.remove(i);
        let capture_entities = entry.capture_entities;
        let material_path = entry.material_path;
        let thumb_path = entry.thumb_path;
        let cell_idx = entry.cell_idx;

        info!(
            "[material_thumbnails] Triggering screenshot for '{}' (cell {})",
            material_path.display(),
            cell_idx
        );

        commands
            .spawn((
                Screenshot::image(entry.render_image),
                HideInHierarchy,
                EditorLocked,
                Name::new("Material Thumbnail Screenshot"),
            ))
            .observe(
                move |trigger: On<ScreenshotCaptured>,
                      mut cmds: Commands,
                      mut images: ResMut<Assets<Image>>,
                      mut user_textures: ResMut<EguiUserTextures>,
                      mut registry: ResMut<MaterialThumbnailRegistry>,
                      mut pending: ResMut<PendingCaptures>,
                      mut cells: ResMut<CaptureCells>,
                      mut tasks: ResMut<LoadingTasks>,
                      job: Option<Res<MaterialThumbnailLoadingJob>>| {
                    pending.active = pending.active.saturating_sub(1);
                    cells.free(cell_idx);

                    for e in &capture_entities {
                        cmds.entity(*e).despawn();
                    }

                let captured = trigger.image.clone();

                if let Some(parent) = thumb_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match captured.clone().try_into_dynamic() {
                    Ok(dyn_img) => {
                        let rgba = dyn_img.to_rgba8();
                        if let Err(e) = rgba.save(&thumb_path) {
                            warn!(
                                "[material_thumbnails] Failed to write {}: {}",
                                thumb_path.display(),
                                e
                            );
                        }
                    }
                    Err(e) => {
                        warn!(
                            "[material_thumbnails] Captured image has unsupported format: {}",
                            e
                        );
                    }
                }

                let captured_handle = images.add(captured);
                user_textures.add_image(EguiTextureHandle::Strong(captured_handle.clone()));
                match user_textures.image_id(captured_handle.id()) {
                    Some(tid) => registry.complete(material_path.clone(), tid),
                    None => registry.cancel(&material_path),
                }

                if let Some(job) = job.as_ref() {
                    tasks.advance(job.handle, 1);
                    let name = material_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();
                        if let Some(t) = tasks.tasks().iter().find(|(h, _)| *h == job.handle).map(|(_, t)| t) {
                            emit_thumbnail_progress(t.completed, t.total, &name);
                        }
                    }
                },
            );
    }
}

/// Walks the project's `assets/` directory looking for `.material` files.
fn scan_project_materials(project_root: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let assets_dir = project_root.join("assets");
    let start = if assets_dir.is_dir() {
        assets_dir
    } else {
        project_root.to_path_buf()
    };
    let mut stack = vec![start];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                // Skip our own cache folder so we don't recurse into it.
                if path.file_name().and_then(|n| n.to_str()) == Some(".thumbs") {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) == Some("material") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// On entry to [`SplashState::Loading`], scan the project for `.material`
/// files, register a loading task, and enqueue every file for capture.
fn queue_loading_thumbnails(
    mut commands: Commands,
    mut registry: ResMut<MaterialThumbnailRegistry>,
    mut tasks: ResMut<LoadingTasks>,
    project: Option<Res<CurrentProject>>,
) {
    let Some(project) = project else {
        // No project → nothing to do; still register a no-op task so the
        // loading screen has something to show and can transition.
        let handle = tasks.register("Material thumbnails", 0);
        commands.insert_resource(MaterialThumbnailLoadingJob { handle });
        return;
    };

    let materials = scan_project_materials(&project.path);
    let total = materials.len() as u32;
    let handle = tasks.register("Material thumbnails", total);
    commands.insert_resource(MaterialThumbnailLoadingJob { handle });
    emit_thumbnail_progress(0, total, "");

    for path in materials {
        registry.request(path);
    }
}

fn resolve_disk_loads(
    mut disk_loads: ResMut<PendingDiskLoads>,
    mut user_textures: ResMut<EguiUserTextures>,
    mut registry: ResMut<MaterialThumbnailRegistry>,
    mut tasks: ResMut<LoadingTasks>,
    job: Option<Res<MaterialThumbnailLoadingJob>>,
    asset_server: Res<AssetServer>,
) {
    let mut i = 0;
    while i < disk_loads.entries.len() {
        let state = asset_server.get_load_state(&disk_loads.entries[i].handle);
        match state {
            Some(LoadState::Loaded) => {
                let entry = disk_loads.entries.swap_remove(i);
                let name = entry
                    .material_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                user_textures.add_image(EguiTextureHandle::Strong(entry.handle.clone()));
                match user_textures.image_id(entry.handle.id()) {
                    Some(tid) => registry.complete(entry.material_path, tid),
                    None => registry.cancel(&entry.material_path),
                }
                if let Some(job) = job.as_ref() {
                    tasks.advance(job.handle, 1);
                    if let Some(t) = tasks.tasks().iter().find(|(h, _)| *h == job.handle).map(|(_, t)| t) {
                        emit_thumbnail_progress(t.completed, t.total, &name);
                    }
                }
            }
            Some(LoadState::Failed(_)) => {
                let entry = disk_loads.entries.swap_remove(i);
                let name = entry
                    .material_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                registry.cancel(&entry.material_path);
                if let Some(job) = job.as_ref() {
                    tasks.advance(job.handle, 1);
                    if let Some(t) = tasks.tasks().iter().find(|(h, _)| *h == job.handle).map(|(_, t)| t) {
                        emit_thumbnail_progress(t.completed, t.total, &name);
                    }
                }
            }
            _ => {
                i += 1;
            }
        }
    }
}

pub struct MaterialFileThumbnailPlugin;

impl Plugin for MaterialFileThumbnailPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] MaterialFileThumbnailPlugin");
        app.init_resource::<MaterialThumbnailRegistry>()
            .init_resource::<PendingCaptures>()
            .init_resource::<ArmedCaptures>()
            .init_resource::<CaptureCells>()
            .init_resource::<PendingDiskLoads>()
            .add_systems(PostStartup, setup_shared_thumbnail_lights)
            .add_systems(OnEnter(SplashState::Loading), queue_loading_thumbnails)
            .add_systems(OnExit(SplashState::Loading), clear_loading_job)
            .add_systems(
                Update,
                (
                    intake_thumbnail_requests,
                    run_pending_captures,
                    fire_armed_captures,
                    resolve_disk_loads,
                )
                    .chain(),
            );
    }
}

/// Spawns the two directional lights once on the thumbnail render layer.
/// Directional lights have no position — they illuminate every sphere on
/// layer 7 regardless of which cell the sphere sits in — so one pair is
/// enough for all concurrent captures. This also keeps us comfortably under
/// Bevy's 10-directional-light limit.
fn setup_shared_thumbnail_lights(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.98, 0.95),
            illuminance: 6000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.6, 0.4, 0.0)),
        RenderLayers::layer(MATERIAL_THUMBNAIL_LAYER),
        HideInHierarchy,
        EditorLocked,
        Name::new("Material Thumbnail Key Light"),
    ));
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.6, 0.7, 0.9),
            illuminance: 2000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, 0.3, -0.8, 0.0)),
        RenderLayers::layer(MATERIAL_THUMBNAIL_LAYER),
        HideInHierarchy,
        EditorLocked,
        Name::new("Material Thumbnail Fill Light"),
    ));
}

fn clear_loading_job(mut commands: Commands) {
    commands.remove_resource::<MaterialThumbnailLoadingJob>();
}
