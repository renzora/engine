//! Render the 3D viewport in a separate process (experimental).
//!
//! With [`ViewportSettings::separate_renderer`] on, the editor spawns
//! `renzora --render-host <file>` (see `renzora_runtime::render_host`), tells it
//! where the camera is every frame, and shows the frames it sends back in the
//! slot 0 viewport panel. The editor's own viewport cameras stay switched off
//! while that is working, so the editor only ever renders its UI and never
//! waits on the scene, however slow the scene is.
//!
//! # What phase 1 does not do yet
//!
//! - **Unsaved edits.** The render host reads the scene from disk and reloads
//!   it when the file changes, so it shows the last save.
//! - **Editor overlays.** Gizmos, the grid and selection outlines were drawn by
//!   the editor's own viewport camera, which is off.
//! - **Secondary viewports and 2D.** Only slot 0 in 3D view uses it; the rest
//!   render in-process as before.
//!
//! # Falling back
//!
//! Until the host's first frame arrives, and whenever it is not running, the
//! editor renders the viewport itself as usual. A host that crashes is
//! restarted after [`RETRY_SECS`], and the viewport keeps working meanwhile.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Sender};

use bevy::prelude::*;
use renzora::core::viewport_types::{ViewportSettings, Viewports};
use renzora::core::{CurrentProject, PrimaryViewportCamera};
use renzora_render_link::{Link, SceneMessage, ViewRequest, MAX_HEIGHT, MAX_WIDTH};

/// Seconds to wait before restarting a render host that exited on its own.
const RETRY_SECS: f64 = 5.0;

/// Whether this editor session renders through a separate process. Read once,
/// at startup, from the saved setting.
///
/// Latched rather than live because it decides how the project's meshes and
/// textures load (see [`keep_scene_assets_off_gpu`]): an asset already on the
/// GPU stays there until its last handle drops, so switching mid-session could
/// not take the editor's copy back off. Changing the setting takes effect on
/// the next launch.
#[derive(Resource, Clone, Copy, Default)]
pub struct SeparateRendererActive(pub bool);

pub fn latch_separate_renderer(mut commands: Commands, settings: Res<ViewportSettings>) {
    commands.insert_resource(SeparateRendererActive(settings.separate_renderer));
    if settings.separate_renderer {
        info!("[render-host] separate render process on for this session; the editor keeps the project's meshes and textures off the GPU");
    }
}

/// With a separate render process, keep the project's meshes and textures off
/// the editor's GPU.
///
/// The editor needs them on the CPU (picking, editing, saving) but never draws
/// the scene, so a GPU copy is pure waste, and it was not small: the editor and
/// the render process each holding the scene ran a 12 GB card out of memory.
/// Bevy uploads every asset whose usage includes the render world, whether or
/// anything draws it or not, so each one is switched to main-world-only in the
/// frame it arrives, before extraction can see it. `get_mut_untracked`, so the
/// switch itself is not a change that would re-extract it.
///
/// Only files inside the project are touched, and not its `.cache` folder:
/// the editor's own icons, fonts and thumbnails still have to draw.
pub fn keep_scene_assets_off_gpu(
    active: Option<Res<SeparateRendererActive>>,
    project: Option<Res<CurrentProject>>,
    asset_server: Res<AssetServer>,
    mut mesh_events: MessageReader<AssetEvent<Mesh>>,
    mut image_events: MessageReader<AssetEvent<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mesh_ids: Vec<AssetId<Mesh>> = mesh_events.read().filter_map(arrived).collect();
    let image_ids: Vec<AssetId<Image>> = image_events.read().filter_map(arrived).collect();
    if !active.is_some_and(|a| a.0) {
        return;
    }
    let Some(project) = project else { return };
    let in_project = |path: Option<bevy::asset::AssetPath>| {
        let Some(path) = path else { return false };
        if !matches!(path.source(), bevy::asset::io::AssetSourceId::Default) {
            return false;
        }
        let relative = path.path();
        !relative.starts_with(".cache") && project.path.join(relative).is_file()
    };
    for id in mesh_ids {
        if in_project(asset_server.get_path(id)) {
            if let Some(mesh) = meshes.get_mut_untracked(id) {
                mesh.asset_usage = bevy::asset::RenderAssetUsages::MAIN_WORLD;
            }
        }
    }
    for id in image_ids {
        if in_project(asset_server.get_path(id)) {
            if let Some(image) = images.get_mut_untracked(id) {
                image.asset_usage = bevy::asset::RenderAssetUsages::MAIN_WORLD;
            }
        }
    }
}

/// The running render host, if any, and the frames it has sent.
#[derive(Resource, Default)]
pub struct RenderHostClient {
    child: Option<Child>,
    /// Scene messages for the host, written to its stdin by a thread that owns
    /// the pipe, so a slow host never blocks an editor frame. Dropping this ends
    /// the thread, which closes the pipe: that is how the host learns the editor
    /// is gone, and a crash closes it just the same.
    outbox: Option<Sender<Vec<u8>>>,
    link: Option<Link>,
    link_path: Option<PathBuf>,
    /// The scene the host was started on; a different one means a restart.
    scene: Option<String>,
    last_frame: u64,
    frames_shown: u64,
    /// Size of the last frame copied into the viewport image.
    shown_size: UVec2,
    /// The panel's current size, as of the last `pull_frame`.
    panel_size: UVec2,
    frame: Vec<u8>,
    retry_at: f64,
    /// Exits in a row without a frame shown in between. See [`MAX_CRASHES`].
    crashes: u32,
}

/// Exits in a row after which the editor stops restarting the host. A host
/// that dies on startup every time (most often out of GPU memory, since it
/// holds a second copy of the scene) would otherwise restart forever.
const MAX_CRASHES: u32 = 3;

impl RenderHostClient {
    /// Whether the host's frames are what the viewport shows. Only then do the
    /// editor's own viewport cameras stand down.
    ///
    /// The newest frame must also be the panel's size. While a panel resizes
    /// the host is a frame or two behind, and the image already has the new
    /// size; showing the old frame there smeared its rows across the panel. The
    /// editor renders those few frames itself instead.
    pub fn is_showing(&self) -> bool {
        self.child.is_some() && self.frames_shown > 0 && self.shown_size == self.panel_size
    }

    /// Whether a host is running and accepting scene messages.
    pub fn is_connected(&self) -> bool {
        self.outbox.is_some()
    }

    /// Queue a scene message for the host. Dropped if no host is running.
    pub fn send(&self, message: &SceneMessage) {
        if let Some(outbox) = self.outbox.as_ref() {
            let _ = outbox.send(message.encode());
        }
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.outbox = None;
        self.link = None;
        if let Some(path) = self.link_path.take() {
            let _ = std::fs::remove_file(path);
        }
        self.scene = None;
        self.last_frame = 0;
        self.frames_shown = 0;
        self.shown_size = UVec2::ZERO;
    }
}

/// The editor's overlays (transform gizmo, selection box, light and collider
/// gizmos) drawn over the render process's frame.
///
/// The editor's own viewport camera is off in this mode, and it drew those. So
/// a small camera of its own draws only the overlay layers (1, and slot 0's
/// gizmo layer) into a transparent image, from the same pose, and the panel
/// layers that image over the frame. Scene meshes are on layer 0, which it does
/// not see, so nothing of the scene is drawn twice.
///
/// The floor grid is not drawn: it fades against the scene's depth, which the
/// editor does not have in this mode, and would cover the scene.
#[derive(Resource, Default)]
pub struct RenderHostOverlay {
    pub image: Option<Handle<Image>>,
    camera: Option<Entity>,
    size: UVec2,
}

impl RenderHostOverlay {
    /// The overlay image while it is in use, for the viewport panel.
    pub fn shown(&self, client: &RenderHostClient) -> Option<Handle<Image>> {
        client.is_showing().then(|| self.image.clone()).flatten()
    }
}

/// A transparent BGRA target the size of the viewport, for the overlay camera.
fn overlay_target(size: UVec2) -> Image {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
    // No CPU copy: only the GPU draws into or samples a render target, and
    // wgpu zero-fills one created without data (zero is transparent here).
    let mut image = Image::new_uninit(
        Extent3d { width: size.x.max(1), height: size.y.max(1), depth_or_array_layers: 1 },
        TextureDimension::D2,
        TextureFormat::Bgra8UnormSrgb,
        bevy::asset::RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    image
}

/// Keep the overlay camera on the editor camera's pose, at the panel's size,
/// and switched on only while the render process supplies the frame.
pub fn drive_overlay(
    mut commands: Commands,
    client: Res<RenderHostClient>,
    viewports: Res<Viewports>,
    mut overlay: ResMut<RenderHostOverlay>,
    mut images: ResMut<Assets<Image>>,
    primary: Query<(&GlobalTransform, &Projection), With<PrimaryViewportCamera>>,
    mut cameras: Query<(&mut Camera, &mut Transform, &mut GlobalTransform, &mut Projection), Without<PrimaryViewportCamera>>,
) {
    use bevy::camera::visibility::RenderLayers;
    use renzora::core::viewport_types::VIEWPORT_3D_GIZMO_LAYER_BASE;

    let showing = client.is_showing();
    if !showing {
        if let Some((mut camera, ..)) = overlay.camera.and_then(|e| cameras.get_mut(e).ok()) {
            if camera.is_active {
                camera.is_active = false;
            }
        }
        return;
    }
    let Ok((pose, projection)) = primary.single() else { return };
    let size = viewports.slots.first().map(|s| s.current_size).unwrap_or(UVec2::ONE);

    // The image, sized to the panel.
    match overlay.image.clone() {
        None => {
            overlay.image = Some(images.add(overlay_target(size)));
            overlay.size = size;
        }
        Some(handle) if overlay.size != size => {
            if let Some(mut image) = images.get_mut(&handle) {
                image.resize(bevy::render::render_resource::Extent3d {
                    width: size.x.max(1),
                    height: size.y.max(1),
                    depth_or_array_layers: 1,
                });
            }
            overlay.size = size;
        }
        Some(_) => {}
    }
    let Some(image) = overlay.image.clone() else { return };

    // The camera, spawned on first use.
    let Some(entity) = overlay.camera else {
        let entity = commands
            .spawn((
                Camera3d::default(),
                Camera {
                    // After the editor's other cameras, though it has a target
                    // of its own and no ordering against them really matters.
                    order: 10,
                    clear_color: ClearColorConfig::Custom(Color::NONE),
                    ..default()
                },
                bevy::camera::RenderTarget::Image(image.into()),
                RenderLayers::from_layers(&[1, VIEWPORT_3D_GIZMO_LAYER_BASE]),
                bevy::core_pipeline::tonemapping::Tonemapping::None,
                bevy::core_pipeline::tonemapping::DebandDither::Disabled,
                bevy::render::view::Msaa::Off,
                // Kept out of the effect routing, the skybox and contact
                // shadows, which all leave offscreen utility cameras alone.
                renzora::core::IsolatedCamera,
                renzora::core::HideInHierarchy,
                Name::new("Render Host Overlay Camera"),
                pose.compute_transform(),
                projection.clone(),
            ))
            .id();
        overlay.camera = Some(entity);
        return;
    };
    if let Ok((mut camera, mut transform, mut global, mut proj)) = cameras.get_mut(entity) {
        if !camera.is_active {
            camera.is_active = true;
        }
        // Both transforms, so the overlay is on this frame's pose rather than
        // a frame behind waiting for propagation.
        let wanted = pose.compute_transform();
        if *transform != wanted {
            *transform = wanted;
            *global = *pose;
        }
        if !matches!((&*proj, projection), (Projection::Perspective(a), Projection::Perspective(b)) if a.fov == b.fov && a.near == b.near && a.far == b.far)
        {
            *proj = projection.clone();
        }
    }
}

/// The asset an event says has just arrived or changed.
fn arrived<A: Asset>(event: &AssetEvent<A>) -> Option<AssetId<A>> {
    match event {
        AssetEvent::Added { id } | AssetEvent::Modified { id } => Some(*id),
        _ => None,
    }
}

/// The scene the editor has open, as the project-relative path a render host
/// takes with `--scene`.
fn open_scene(project: &CurrentProject) -> String {
    project
        .config
        .editor_last_scene
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| project.config.main_scene.clone())
}

/// Start, stop and restart the render host to match the setting and the open
/// scene.
pub fn manage_render_host(
    settings: Res<ViewportSettings>,
    active: Option<Res<SeparateRendererActive>>,
    project: Option<Res<CurrentProject>>,
    time: Res<Time<Real>>,
    mut client: ResMut<RenderHostClient>,
    mut told_restart: Local<Option<bool>>,
) {
    let active = active.is_some_and(|a| a.0);
    // The setting only takes effect at launch; say so once when it is changed.
    if settings.separate_renderer != active {
        if *told_restart != Some(settings.separate_renderer) {
            *told_restart = Some(settings.separate_renderer);
            info!(
                "[render-host] Separate Render Process turns {} the next time the editor starts",
                if settings.separate_renderer { "on" } else { "off" }
            );
        }
    } else {
        *told_restart = None;
    }
    let now = time.elapsed_secs_f64();

    // A host that died on its own: say so, and try again in a moment, unless
    // it keeps dying.
    if let Some(child) = client.child.as_mut() {
        if let Ok(Some(status)) = child.try_wait() {
            if client.frames_shown == 0 {
                client.crashes += 1;
            } else {
                client.crashes = 1;
            }
            client.stop();
            if client.crashes >= MAX_CRASHES {
                warn!("[render-host] the render process exited ({status}) {MAX_CRASHES} times in a row, so it will not be restarted. Its crash.log beside the editor has the error. The viewport will show the scene without its meshes and textures, which this mode keeps off the editor's GPU; restart the editor to try again, or turn Separate Render Process off and restart to edit without it.");
                client.retry_at = f64::INFINITY;
            } else {
                warn!("[render-host] the render process exited ({status}); the editor is rendering the viewport itself and will retry in {RETRY_SECS} s");
                client.retry_at = now + RETRY_SECS;
            }
        }
    }

    let wanted = active;
    let scene = project.as_deref().map(open_scene);
    let running = client.child.is_some();

    // Switching scenes needs no restart: the despawns and spawns reach the host
    // as an ordinary patch (`render_sync`). `scene` only picks what the host
    // loads from disk while it starts.
    if running && !wanted {
        client.stop();
        return;
    }
    if !wanted || running || now < client.retry_at {
        return;
    }
    let (Some(project), Some(scene)) = (project, scene) else {
        return;
    };

    match spawn(&project, &scene) {
        Ok((child, stdin, link, path)) => {
            info!("[render-host] started the render process for {scene}");
            client.child = Some(child);
            client.outbox = Some(pipe_writer(stdin));
            client.link = Some(link);
            client.link_path = Some(path);
            client.scene = Some(scene);
        }
        Err(e) => {
            warn!("[render-host] could not start the render process: {e}");
            client.retry_at = now + RETRY_SECS;
        }
    }
}

fn spawn(
    project: &CurrentProject,
    scene: &str,
) -> std::io::Result<(Child, ChildStdin, Link, PathBuf)> {
    let path = std::env::temp_dir().join(format!("renzora-render-{}.link", std::process::id()));
    let link = Link::create(&path)?;
    let binary = crate::external_runtime::find_runtime_binary()
        .ok_or_else(|| std::io::Error::other("no renzora binary to launch"))?;
    let mut command = Command::new(binary);
    command
        .arg("--no-editor")
        .arg("--project")
        .arg(&project.path)
        .arg("--scene")
        .arg(scene)
        .arg("--render-host")
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    if let Some(out) = child.stdout.take() {
        crate::external_runtime::forward_to_console(out, "Renderer");
    }
    if let Some(err) = child.stderr.take() {
        crate::external_runtime::forward_to_console(err, "Renderer");
    }
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| std::io::Error::other("render process has no stdin"))?;
    Ok((child, stdin, link, path))
}

/// The thread that writes queued messages to the host's stdin. It ends, closing
/// the pipe, when the sender is dropped or the host stops reading.
fn pipe_writer(mut stdin: ChildStdin) -> Sender<Vec<u8>> {
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        for bytes in rx {
            if stdin.write_all(&bytes).and_then(|_| stdin.flush()).is_err() {
                break;
            }
        }
    });
    tx
}

/// Tell the host where the camera is and how big the panel is. After
/// transform propagation, so the pose is this frame's.
pub fn push_view(
    client: Res<RenderHostClient>,
    viewports: Res<Viewports>,
    camera: Query<(&GlobalTransform, &Projection), With<PrimaryViewportCamera>>,
) {
    let Some(link) = client.link.as_ref() else { return };
    let Ok((transform, projection)) = camera.single() else { return };
    let size = viewports.slots.first().map(|s| s.current_size).unwrap_or(UVec2::ONE);
    let (_, rotation, translation) = transform.to_scale_rotation_translation();
    let mut view = ViewRequest {
        translation: translation.to_array(),
        rotation: rotation.to_array(),
        width: size.x,
        height: size.y,
        ..default()
    };
    match projection {
        Projection::Perspective(p) => {
            view.perspective = 1;
            view.fov_y = p.fov;
            view.near = p.near;
            view.far = p.far;
        }
        Projection::Orthographic(o) => {
            view.perspective = 0;
            view.ortho_scale = o.scale;
            view.near = o.near;
            view.far = o.far;
        }
        _ => return,
    }
    link.write_view(&view);
}

/// Copy the host's newest frame into the slot 0 viewport image.
pub fn pull_frame(
    mut client: ResMut<RenderHostClient>,
    viewports: Res<Viewports>,
    mut images: ResMut<Assets<Image>>,
) {
    let client = &mut *client;
    let Some(link) = client.link.as_ref() else { return };
    let Some(slot) = viewports.slots.first() else { return };
    let Some(handle) = slot.image.as_ref() else { return };
    client.panel_size = slot.current_size;
    // A panel bigger than the link carries stays in-process.
    if slot.current_size.x > MAX_WIDTH || slot.current_size.y > MAX_HEIGHT {
        client.frames_shown = 0;
        return;
    }
    let Some((number, width, height)) = link.read_frame(client.last_frame, &mut client.frame) else {
        return;
    };
    client.last_frame = number;
    // A frame rendered before the panel's last resize: the host is a frame or
    // two behind and the next one will fit.
    if UVec2::new(width, height) != slot.current_size {
        return;
    }
    let Some(mut image) = images.get_mut(handle) else { return };
    let size = image.texture_descriptor.size;
    if (size.width, size.height) != (width, height) {
        return;
    }
    // Swap rather than copy: the buffer comes back empty and refills from the
    // link next frame.
    image.data = Some(std::mem::take(&mut client.frame));
    if client.frames_shown == 0 {
        info!("[render-host] showing frames from the render process");
    }
    client.frames_shown += 1;
    client.shown_size = UVec2::new(width, height);
    client.crashes = 0;
}
