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

use bevy::asset::{LoadState, RenderAssetUsages};
use bevy::camera::visibility::RenderLayers;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy::camera::Hdr;
use bevy::core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass, NormalPrepass};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use uuid::Uuid;

use renzora::core::{CurrentProject, EditorLocked, HideInHierarchy, IsolatedCamera};
use renzora_editor_framework::{
    material_thumb_path, migrate_legacy_thumbnail_cache, MaterialThumbnailRegistry,
};
use renzora_shader::material::codegen::{self, CompileResult, TextureKind};
use renzora_shader::material::graph::{MaterialDomain, MaterialGraph};
use renzora_shader::material::runtime::{new_graph_material, FallbackTexture, GraphMaterial};
use renzora_shader::material::sprite_material::{insert_sprite_shader, sprite_material_from_compiled, SpriteGraphExt};
use bevy::sprite_render::SpriteMaterial;
use renzora_splash::{LoadingTaskHandle, LoadingTasks, SplashState};

/// This capture rig's private render layer. Allocated in the contract crate
/// alongside every other offscreen rig's — see the registry there for why.
pub use renzora::core::viewport_types::MATERIAL_THUMBNAIL_LAYER;
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
    /// The parsed graph `compile` came from, kept to fill the base material
    /// the way the scene resolver does (see `arm_one_capture`).
    graph: Option<MaterialGraph>,
    /// A Sprite material: captured on the sample sprite by a 2D camera, with a
    /// "2D" tag in the corner, rather than on the sphere it cannot be drawn on.
    sprite: bool,
    /// Whether the captured PNG may be written to the disk cache. False when
    /// the `.material` failed to *parse* — that's usually a transient state
    /// (the importer is still writing the file), and persisting the fallback
    /// sphere would poison the cache until the material is next re-saved.
    cache_to_disk: bool,
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
    /// See [`CaptureJob::cache_to_disk`].
    cache_to_disk: bool,
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
    (jobs, mut companion): (Option<ResMut<renzora::CompanionJobs>>, ResMut<CompanionCaptures>),
) {
    let mut jobs = jobs;
    let advance_skip =
        |tasks: &mut LoadingTasks, job: &Option<Res<MaterialThumbnailLoadingJob>>, name: &str| {
            if let Some(j) = job.as_ref() {
                tasks.advance(j.handle, 1);
                if let Some(t) = tasks
                    .tasks()
                    .iter()
                    .find(|(h, _)| *h == j.handle)
                    .map(|(_, t)| t)
                {
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

        // Disk cache hit (and newer than the `.material` that produced it):
        // kick off an async load; published to the registry once decoded.
        if cached_thumb_is_fresh(&thumb_path, &material_path) {
            if let Some(handle) = try_load_cached_thumb(&thumb_path, &asset_server, project) {
                disk_loads.entries.push(PendingDiskLoad {
                    material_path,
                    handle,
                });
                continue;
            }
        }

        // Miss: the companion renders it when one is connected (see
        // `CompanionCaptures`).
        if !companion.local_only.contains(&material_path) {
            let args = renzora::job_args::MaterialThumbnail { project: project.path.clone(), material: material_path.clone() };
            if let Some(id) = jobs.as_mut().and_then(|j| j.submit("material_thumbnail", short_name.clone(), &args)) {
                companion.jobs.insert(id, (material_path, thumb_path));
                continue;
            }
        }

        // Here: prepare a capture job. If anything goes wrong (bad JSON,
        // compile errors, non-Surface domain), we still push a job with
        // `compile: None`; the render path falls back to a plain default
        // material sphere so every `.material` gets *some* thumbnail.
        let mut cache_to_disk = true;
        let mut sprite = false;
        let mut base_graph: Option<MaterialGraph> = None;
        let compile_opt: Option<CompileResult> = match read_material_source(&material_path) {
            Ok(content) => match serde_json::from_str::<MaterialGraph>(&content) {
                Ok(graph) if matches!(graph.domain, MaterialDomain::Surface | MaterialDomain::Sprite) => {
                    sprite = graph.domain == MaterialDomain::Sprite;
                    let result = codegen::compile(&graph);
                    if result.errors.is_empty() {
                        base_graph = Some(graph.clone());
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
                    // A parse failure is usually the importer still writing
                    // the file — show the fallback this session, but don't
                    // let it poison the disk cache.
                    cache_to_disk = false;
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
            if compile_opt.is_some() {
                "compiled"
            } else {
                "fallback"
            }
        );

        pending.jobs.push(CaptureJob {
            material_path,
            thumb_path,
            compile: compile_opt,
            graph: base_graph,
            sprite,
            cache_to_disk,
            texture_handles,
            waited_frames: 0,
        });
    }
}

/// True iff the cached thumbnail file is fresh — exists and its mtime is at
/// least as new as the `.material` that produced it. Mirrors the model and
/// texture caches' freshness checks; duplicated to keep this renderer
/// self-contained.
/// Read a `.material` file's JSON.
///
/// On the web this goes through the picked directory handle rather than
/// `std::fs`, which there returns "operation not supported on this platform"
/// for every file — a thumbnail pass over a model's material folder turned that
/// into hundreds of identical warnings.
///
/// The web read is cached and asynchronous: a miss returns `NotFound` for now
/// and fetches, so the thumbnail simply appears a frame or two later, which is
/// already how the disk-cache path behaves.
fn read_material_source(path: &std::path::Path) -> std::io::Result<String> {
    #[cfg(target_arch = "wasm32")]
    {
        renzora_webfs::read_text_cached(path).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "not read yet (web)")
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::read_to_string(path)
    }
}

fn cached_thumb_is_fresh(cache_path: &std::path::Path, source_path: &std::path::Path) -> bool {
    let Ok(cache_meta) = std::fs::metadata(cache_path) else {
        return false;
    };
    let Ok(source_meta) = std::fs::metadata(source_path) else {
        return false;
    };
    let (Ok(cache_mtime), Ok(source_mtime)) = (cache_meta.modified(), source_meta.modified())
    else {
        return false;
    };
    cache_mtime >= source_mtime
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
    mut sprite_kit: SpriteCaptureKit,
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
            &mut sprite_kit,
        );
    }
}

/// What a Sprite material's capture needs beyond the 3D path's resources.
#[derive(bevy::ecs::system::SystemParam)]
struct SpriteCaptureKit<'w, 's> {
    materials: ResMut<'w, Assets<SpriteGraphExt>>,
    fonts: Option<Res<'w, renzora_ember::font::EmberFonts>>,
    /// The sample sprite's image, made once and shared by every capture.
    sample: Local<'s, Option<Handle<Image>>>,
}

/// Distance between two 2D capture cells. A 2D camera sees `THUMB_SIZE`
/// units across, so cells far wider than that keep concurrent captures out of
/// each other's frame; the 3D cells' 40 would overlap.
const SPRITE_CELL_SPACING: f32 = 1000.0;

#[allow(clippy::too_many_arguments)]
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
    sprite_kit: &mut SpriteCaptureKit,
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

    if job.sprite {
        let capture_entities = spawn_sprite_capture(commands, &job, cell_idx, &render_image_handle, images, shaders, asset_server, fallback, sprite_kit);
        pending.active += 1;
        armed.list.push(ArmedCapture {
            material_path: job.material_path.clone(),
            thumb_path: job.thumb_path.clone(),
            render_image: render_image_handle,
            capture_entities,
            cell_idx,
            frames_remaining: CAPTURE_WARMUP_FRAMES,
            cache_to_disk: job.cache_to_disk,
        });
        return;
    }

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
            // A path of its own per shader. Bevy's shader cache is keyed by
            // module path, so every thumbnail sharing one path meant each new
            // capture replaced the last one's shader mid-warmup.
            let shader = renzora::shader_source::shader(
                compile.fragment_shader.clone(),
                format!("graph_material/thumbnail_{uuid}.wesl"),
            );
            let _ = shaders.insert(&shader_handle, shader);
            mat.extension.shader_uuid = Some(uuid);
            // Without this the deferred G-buffer pass (a project in deferred
            // mode puts every 3D camera there, thumbnail cameras included)
            // kept StandardMaterial's shader and drew the base: plain white.
            // The scene resolver sets it the same way.
            mat.extension.writes_gbuffer = codegen::supports_deferred(&compile.fragment_shader);
            if let Some(graph) = &job.graph {
                renzora_shader::material::standard_build::apply_base_approximation(&mut mat.base, graph, asset_server);
            }
        }

        materials.add(mat)
    };

    // All captures live at the world origin — MAX_CONCURRENT_CAPTURES = 1
    // means there's never another capture's sphere to catch accidentally,
    // and placing entities close to (0,0,0) avoids the float-precision
    // problems we'd see at huge world-space offsets.
    // UV sphere for the same straight-seam reason as the preview's.
    let sphere_mesh = meshes.add(crate::preview::preview_sphere());

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
            Name::new(format!(
                "Material Thumbnail Sphere ({})",
                job.material_path.display()
            )),
        ))
        .id();

    let camera = commands
        .spawn((
            Camera3d::default(),
            // Render HDR like the editor viewport cameras. Bevy's mesh pipeline
            // key only carries a coarse `hdr` bit, so a non-HDR offscreen camera
            // specializes `pbr_opaque_mesh_pipeline` for an LDR target — which,
            // under heavy thumbnail load, the HDR viewport pass can pick up,
            // crashing wgpu ("Rgba8UnormSrgb vs Rgba16Float"). Matching HDR keeps
            // a single pbr pipeline format across every camera. Tonemapping still
            // resolves down to the LDR thumbnail image on output.
            Hdr,
            NormalPrepass,
            DepthPrepass,
            MotionVectorPrepass,
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
        cache_to_disk: job.cache_to_disk,
    });
}

/// A Sprite material's capture rig: a 2D camera on the sample sprite the
/// Material Preview uses, wearing the material, with a "2D" tag in the
/// corner so a sprite material reads as one at a glance in the Assets panel.
/// A graph that failed to compile still gets the plain sample sprite and the
/// tag, as a 3D one still gets its grey sphere.
#[allow(clippy::too_many_arguments)]
fn spawn_sprite_capture(
    commands: &mut Commands,
    job: &CaptureJob,
    cell_idx: usize,
    target: &Handle<Image>,
    images: &mut Assets<Image>,
    shaders: &mut Assets<Shader>,
    asset_server: &AssetServer,
    fallback: &FallbackTexture,
    kit: &mut SpriteCaptureKit,
) -> Vec<Entity> {
    let origin = Vec3::new(cell_idx as f32 * SPRITE_CELL_SPACING, 10_000.0, 0.0);
    let layer = RenderLayers::layer(MATERIAL_THUMBNAIL_LAYER);
    let half = THUMB_SIZE as f32 / 2.0;
    let sample = kit
        .sample
        .get_or_insert_with(|| images.add(crate::preview_2d::sample_sprite_image()))
        .clone();

    let camera = commands
        .spawn((
            Camera2d,
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgba(0.08, 0.08, 0.1, 1.0)),
                order: -1000 - (cell_idx as isize),
                is_active: true,
                ..default()
            },
            RenderTarget::Image(target.clone().into()),
            Transform::from_translation(origin),
            layer.clone(),
            IsolatedCamera,
            HideInHierarchy,
            EditorLocked,
            Name::new("Sprite Material Thumbnail Camera"),
        ))
        .id();

    let mut sprite = commands.spawn((
        Sprite {
            image: sample,
            custom_size: Some(Vec2::splat(THUMB_SIZE as f32 * 0.8)),
            ..default()
        },
        Transform::from_translation(origin),
        layer.clone(),
        HideInHierarchy,
        EditorLocked,
        Name::new(format!("Sprite Material Thumbnail ({})", job.material_path.display())),
    ));
    if let Some(compile) = &job.compile {
        let label = job.material_path.to_string_lossy().to_string();
        let uuid = insert_sprite_shader(shaders, compile.fragment_shader.clone(), "graph_material/sprite_thumbnail.wesl".into());
        let material = sprite_material_from_compiled(&fallback.0, uuid, &compile.texture_bindings, &compile.parameters, &label, asset_server);
        sprite.insert(SpriteMaterial(kit.materials.add(material)));
    }
    let sprite = sprite.id();
    let mut entities = vec![camera, sprite];

    // The "2D" tag, bottom-right. Text needs the editor font, which is always
    // loaded by the time a thumbnail is asked for; without it the tag is left
    // off rather than drawn in a fallback face.
    if let Some(fonts) = kit.fonts.as_ref() {
        let chip_size = Vec2::new(54.0, 34.0);
        let chip_at = origin + Vec3::new(half - chip_size.x / 2.0 - 8.0, -half + chip_size.y / 2.0 + 8.0, 1.0);
        entities.push(
            commands
                .spawn((
                    Sprite::from_color(Color::srgba(0.05, 0.05, 0.07, 0.85), chip_size),
                    Transform::from_translation(chip_at),
                    layer.clone(),
                    HideInHierarchy,
                    EditorLocked,
                    Name::new("Sprite Material Thumbnail Tag"),
                ))
                .id(),
        );
        entities.push(
            commands
                .spawn((
                    Text2d::new("2D"),
                    TextFont { font: fonts.ui.clone(), font_size: bevy::text::FontSize::Px(24.0), ..default() },
                    TextColor(Color::srgb(0.55, 0.85, 1.0)),
                    Transform::from_translation(chip_at + Vec3::Z),
                    layer,
                    HideInHierarchy,
                    EditorLocked,
                    Name::new("Sprite Material Thumbnail Tag Text"),
                ))
                .id(),
        );
    }
    entities
}

fn fire_armed_captures(mut commands: Commands, mut armed: ResMut<ArmedCaptures>) {
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
        let cache_to_disk = entry.cache_to_disk;

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

                    let mut captured = trigger.image.clone();

                    if cache_to_disk {
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
                    }

                    // Publish the rendered thumbnail's `Handle<Image>` to the
                    // registry; the native (bevy_ui) browser/inspector display it
                    // via `ImageNode`.
                    //
                    // Bevy hands the readback back as `RenderAssetUsages::MAIN_WORLD`
                    // — the CPU pixels are there (which is why the PNG on disk is
                    // correct) but the image is never extracted into the render
                    // world, so it has no GPU texture and the `ImageNode` draws a
                    // black tile. Saving a material therefore looked like "the
                    // thumbnail wasn't generated": the freshly captured handle was
                    // unrenderable, and only the *next* session's disk-cache reload
                    // (which goes through the asset server, RENDER_WORLD included)
                    // showed anything. Opt the copy we keep into both worlds.
                    captured.asset_usage = RenderAssetUsages::default();
                    let captured_handle = images.add(captured);
                    registry.complete(material_path.clone(), captured_handle.clone());

                    if let Some(job) = job.as_ref() {
                        tasks.advance(job.handle, 1);
                        let name = material_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_string();
                        if let Some(t) = tasks
                            .tasks()
                            .iter()
                            .find(|(h, _)| *h == job.handle)
                            .map(|(_, t)| t)
                        {
                            emit_thumbnail_progress(t.completed, t.total, &name);
                        }
                    }
                },
            );
    }
}

/// On entry to [`SplashState::Loading`]: forget the previous project's
/// thumbnails and move a legacy cache into place. Nothing is rendered here.
///
/// This used to find every `.material` in the project and render the ones
/// without a cached thumbnail, as a task the loading screen waited for: a
/// project could not be opened until every material had been drawn. Now a
/// thumbnail is asked for when something shows it (the Assets panel, the
/// material drawer), and the companion renders whatever is missing in the
/// background once the project is open (see `renzora_companion`'s cache).
fn queue_loading_thumbnails(mut registry: ResMut<MaterialThumbnailRegistry>, project: Option<Res<CurrentProject>>) {
    // Drop every entry the previous session populated, so a thumbnail asked
    // for in the new project is not answered from the old one. The on-disk
    // PNG cache survives this; the first request reloads from there.
    registry.reset();

    // Best-effort move from the legacy `.thumbs/` directory to
    // `.cache/thumbnails/`, before any thumbnail is asked for, so cache hits
    // find their PNGs at the new path. Skipped if either the legacy path is
    // absent or the new path already exists.
    if let Some(p) = project.as_deref() {
        migrate_legacy_thumbnail_cache(p);
    }
}

fn resolve_disk_loads(
    mut disk_loads: ResMut<PendingDiskLoads>,
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
                registry.complete(entry.material_path, entry.handle.clone());
                if let Some(job) = job.as_ref() {
                    tasks.advance(job.handle, 1);
                    if let Some(t) = tasks
                        .tasks()
                        .iter()
                        .find(|(h, _)| *h == job.handle)
                        .map(|(_, t)| t)
                    {
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
                    if let Some(t) = tasks
                        .tasks()
                        .iter()
                        .find(|(h, _)| *h == job.handle)
                        .map(|(_, t)| t)
                    {
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
        renzora::init_companion_jobs(app);
        app.init_resource::<MaterialThumbnailRegistry>()
            .init_resource::<PendingCaptures>()
            .init_resource::<ArmedCaptures>()
            .init_resource::<CaptureCells>()
            .init_resource::<PendingDiskLoads>()
            .init_resource::<CompanionCaptures>()
            .init_resource::<LoadingTasks>()
            .add_systems(PostStartup, setup_shared_thumbnail_lights)
            .add_systems(OnEnter(SplashState::Loading), queue_loading_thumbnails)
            .add_systems(OnExit(SplashState::Loading), clear_loading_job)
            .add_systems(
                Update,
                (
                    intake_thumbnail_requests,
                    collect_companion_captures,
                    run_pending_captures,
                    fire_armed_captures,
                    resolve_disk_loads,
                )
                    .chain(),
            );
    }
}

/// Captures handed to the companion process, and the materials it failed on.
///
/// A capture compiles the material's shader and renders a sphere with it:
/// work that competes with the scene being edited. The companion runs this
/// same plugin and renders into the same cache file; the editor only loads the
/// PNG. With no companion connected, or for a material it could not render,
/// the capture happens here as it always did.
#[derive(Resource, Default)]
struct CompanionCaptures {
    /// Job id to the material and its cache file.
    jobs: std::collections::HashMap<renzora::CompanionJobId, (PathBuf, PathBuf)>,
    /// Materials the companion failed on: captured here from now on.
    local_only: std::collections::HashSet<PathBuf>,
}

/// Turn finished companion captures into cache loads, and failed ones back
/// into requests the editor captures itself.
fn collect_companion_captures(
    mut done: MessageReader<renzora::CompanionJobDone>,
    mut captures: ResMut<CompanionCaptures>,
    mut registry: ResMut<MaterialThumbnailRegistry>,
    mut disk_loads: ResMut<PendingDiskLoads>,
    asset_server: Res<AssetServer>,
    project: Option<Res<CurrentProject>>,
) {
    for answer in done.read() {
        let Some((material_path, thumb_path)) = captures.jobs.remove(&answer.id) else {
            continue;
        };
        let loaded = answer.result.is_ok()
            && project.as_ref().is_some_and(|project| match try_load_cached_thumb(&thumb_path, &asset_server, project) {
                Some(handle) => {
                    disk_loads.entries.push(PendingDiskLoad { material_path: material_path.clone(), handle });
                    true
                }
                None => false,
            });
        if !loaded {
            if let Err(why) = &answer.result {
                warn!(
                    "[material_thumbnails] the companion could not render {}: {why}; rendering it here",
                    material_path.display()
                );
            }
            captures.local_only.insert(material_path.clone());
            // Still in flight in the registry, so straight back to intake.
            registry.incoming_requests.push_back(material_path);
        }
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
            shadow_maps_enabled: false,
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
            shadow_maps_enabled: false,
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
