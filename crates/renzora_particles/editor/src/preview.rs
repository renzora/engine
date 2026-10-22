//! The Effect Preview's offscreen rig: the open effect playing on its own
//! render layer ([`VFX_PREVIEW_LAYER`]), drawn into an image the panel shows.
//!
//! The effect is an ordinary `ParticleSystem` entity with the document's
//! path, so it resolves to the same asset handle the editor pushes edits into
//! and rebuilds with them like every scene instance does; the preview is not a
//! second renderer that could disagree with the game. Its renderer children
//! inherit the layer (`renzora_particles::render` copies it), which is what
//! keeps the preview's particles out of the scene viewport.
//!
//! Two cameras share the image and only the one matching the effect's
//! dimension renders: a 3D orbit camera over a ground plane, and a 2D camera
//! whose units are pixels, as a 2D effect's are. Both render only while the
//! panel is the visible tab somewhere, as every other preview does.
//!
//! What the panel's controls drive lives in [`VfxPreviewSettings`]: the
//! backdrop, a speed multiplier (the entity's `ParticlePlayback::time_scale`),
//! the emitter's motion (the entity is moved along a circle or a figure 8, so
//! world-space trails are visible), looping a one-shot, and restarting after
//! an edit settles.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Hdr, RenderTarget};
use bevy::image::ImageSampler;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::ui::ComputedNode;

use renzora::core::viewport_types::VFX_PREVIEW_LAYER;
use renzora::core::{EditorLocked, HideInHierarchy, IsolatedCamera};
use renzora_ember::dock::{Dock, DockWindows, FixedDock};
use renzora_ember::UiInteraction;
use renzora_particles::asset::Dimension;
use renzora_particles::gpu::ActiveBackend;
use renzora_particles::{ParticlePlayback, ParticleRuntime, ParticleSystem};

use crate::state::VfxEditorState;

pub const PANEL: &str = "vfx_preview";

/// How long an edit must sit still before restart-on-edit fires, so a scrub
/// does not restart the effect on every frame of the drag.
const RESTART_SETTLE: f64 = 0.35;

/// The preview's surroundings. Grid is the default: the dark ground with its
/// grid lines gives an effect scale and a floor to rise from, where the plain
/// dark ground left particles floating in nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Backdrop {
    Dark,
    #[default]
    Grid,
    Sky,
}

impl Backdrop {
    pub const ALL: [Backdrop; 3] = [Backdrop::Dark, Backdrop::Grid, Backdrop::Sky];
    pub fn label(self) -> &'static str {
        match self {
            Backdrop::Dark => "Dark",
            Backdrop::Grid => "Grid",
            Backdrop::Sky => "Sky",
        }
    }
    fn clear(self) -> Color {
        match self {
            Backdrop::Dark | Backdrop::Grid => Color::srgb(0.045, 0.047, 0.06),
            Backdrop::Sky => Color::srgb(0.42, 0.6, 0.82),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Motion {
    #[default]
    Still,
    Circle,
    Figure8,
}

impl Motion {
    pub const ALL: [Motion; 3] = [Motion::Still, Motion::Circle, Motion::Figure8];
    pub fn label(self) -> &'static str {
        match self {
            Motion::Still => "Still",
            Motion::Circle => "Circle",
            Motion::Figure8 => "Figure 8",
        }
    }

    /// Where the emitter is at `phase` seconds, as a fraction of `radius`
    /// in the motion's plane (XZ for 3D, XY for 2D).
    pub fn offset(self, phase: f32) -> Vec2 {
        let a = phase * 1.6;
        match self {
            Motion::Still => Vec2::ZERO,
            Motion::Circle => Vec2::new(a.cos(), a.sin()),
            // A lemniscate: the crossing point is where it started.
            Motion::Figure8 => Vec2::new(a.sin(), (2.0 * a).sin() * 0.5),
        }
    }
}

pub const SPEEDS: [f32; 5] = [0.1, 0.25, 0.5, 1.0, 2.0];

#[derive(Resource)]
pub struct VfxPreviewSettings {
    pub backdrop: Backdrop,
    /// Index into [`SPEEDS`].
    pub speed: usize,
    pub motion: Motion,
    /// Replay a finished one-shot.
    pub looping: bool,
    pub restart_on_edit: bool,
}

impl Default for VfxPreviewSettings {
    fn default() -> Self {
        Self { backdrop: Backdrop::Grid, speed: 3, motion: Motion::Still, looping: true, restart_on_edit: false }
    }
}

/// Live numbers off the preview's runtime, for the panel's chips.
#[derive(Resource, Default, PartialEq, Clone)]
pub struct VfxPreviewStats {
    pub alive: u32,
    pub dropped: u64,
    pub time: f32,
    /// Per emitter of the document: (name, alive, capacity, compiled).
    pub emitters: Vec<(String, u32, u32, bool)>,
    /// Per emitter of the document: the backend the preview runs it on, and
    /// why the CPU when the GPU was not ruled out by the setting itself.
    /// `None` for an emitter that is not running (off, or failed to compile).
    pub backends: Vec<Option<(ActiveBackend, Option<String>)>>,
}

impl VfxPreviewStats {
    /// "GPU", "CPU", or "CPU + GPU" for an effect with both.
    pub fn backend_summary(&self) -> &'static str {
        let running = || self.backends.iter().flatten();
        let gpu = running().any(|(b, _)| *b == ActiveBackend::Gpu);
        let cpu = running().any(|(b, _)| *b == ActiveBackend::Cpu);
        match (cpu, gpu) {
            (true, true) => "CPU + GPU",
            (false, true) => "GPU",
            _ => "CPU",
        }
    }
}

#[derive(Resource)]
pub struct VfxPreviewImage {
    pub handle: Handle<Image>,
    requested: UVec2,
    current: UVec2,
}

#[derive(Resource, Clone, Copy)]
pub struct VfxPreviewOrbit {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: Vec3,
    pub pan_2d: Vec2,
    pub zoom_2d: f32,
}

impl Default for VfxPreviewOrbit {
    fn default() -> Self {
        Self { yaw: 0.5, pitch: 0.32, distance: 6.0, target: Vec3::new(0.0, 1.0, 0.0), pan_2d: Vec2::new(0.0, 120.0), zoom_2d: 1.0 }
    }
}

/// A one-shot "restart the preview" request from the panel's button.
#[derive(Resource)]
pub struct RestartPreview;

#[derive(Component)]
struct PreviewCamera3d;
#[derive(Component)]
struct PreviewCamera2d;
#[derive(Component)]
struct PreviewGround;
#[derive(Component)]
struct PreviewBackdrop2d;
/// The preview's effect entity. Excluded from the viewport gizmo.
#[derive(Component)]
pub struct PreviewEffect;
/// On the panel's image node: orbit input reads its interaction.
#[derive(Component)]
pub struct PreviewViewport;

#[derive(Resource)]
struct GroundMaterials {
    plain: Handle<StandardMaterial>,
    grid: Handle<StandardMaterial>,
    sky: Handle<StandardMaterial>,
    grid_2d: Handle<Image>,
}

pub struct VfxPreviewPlugin;

impl Plugin for VfxPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VfxPreviewSettings>()
            .init_resource::<VfxPreviewStats>()
            .init_resource::<VfxPreviewOrbit>()
            .add_systems(PostStartup, setup)
            // Camera activity has to be decided every frame, so closing the
            // panel switches the cameras off; the rest only while visible.
            .add_systems(Update, sync_cameras)
            .add_systems(
                Update,
                (report_size, resize, orbit_input, place_cameras, sync_effect, drive_effect, sync_backdrop, collect_stats)
                    .chain()
                    .run_if(renzora_ember::dock::panel_active(PANEL)),
            );
    }
}

fn rgba_image(w: u32, h: u32, data: Vec<u8>) -> Image {
    Image::new(
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// A grid texture: thin lines every `cell` texels on a dark ground, a
/// brighter one every eighth line, so scale reads at a glance.
fn grid_image(size: u32, cell: u32, base: [u8; 3], line: [u8; 3], major: [u8; 3]) -> Image {
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let on = |v: u32, every: u32| v.is_multiple_of(every);
            let c = if on(x, cell * 8) || on(y, cell * 8) {
                major
            } else if on(x, cell) || on(y, cell) {
                line
            } else {
                base
            };
            data.extend(c);
            data.push(255);
        }
    }
    let mut image = rgba_image(size, size, data);
    image.sampler = ImageSampler::linear();
    image
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let size = Extent3d { width: 640, height: 480, depth_or_array_layers: 1 };
    let mut target = Image { data: Some(vec![0u8; (size.width * size.height * 4) as usize]), ..default() };
    target.texture_descriptor.size = size;
    target.texture_descriptor.format = TextureFormat::Bgra8UnormSrgb;
    target.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    let handle = images.add(target);
    commands.insert_resource(VfxPreviewImage { handle: handle.clone(), requested: UVec2::new(640, 480), current: UVec2::new(640, 480) });

    let layer = RenderLayers::layer(VFX_PREVIEW_LAYER);
    // HDR so additive fire stacks past 1 the way it does in the scene.
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Camera { clear_color: ClearColorConfig::Custom(Backdrop::Dark.clear()), order: -6, is_active: false, ..default() },
        RenderTarget::Image(handle.clone().into()),
        Transform::from_xyz(0.0, 2.5, 6.0).looking_at(Vec3::Y, Vec3::Y),
        layer.clone(),
        PreviewCamera3d,
        IsolatedCamera,
        HideInHierarchy,
        EditorLocked,
        Name::new("VFX Preview Camera"),
    ));
    commands.spawn((
        Camera2d,
        Camera { clear_color: ClearColorConfig::Custom(Backdrop::Dark.clear()), order: -6, is_active: false, ..default() },
        RenderTarget::Image(handle.into()),
        layer.clone(),
        PreviewCamera2d,
        IsolatedCamera,
        HideInHierarchy,
        EditorLocked,
        Name::new("VFX Preview Camera 2D"),
    ));
    // The ground is the only lit thing here; particles are unlit.
    commands.spawn((
        DirectionalLight { illuminance: 3500.0, shadow_maps_enabled: false, ..default() },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, 0.4, 0.0)),
        layer.clone(),
        HideInHierarchy,
        EditorLocked,
        Name::new("VFX Preview Light"),
    ));

    let grid_tex = images.add(grid_image(512, 32, [22, 23, 29], [38, 40, 50], [58, 62, 78]));
    let sky_tex = images.add(grid_image(512, 32, [86, 94, 86], [100, 108, 100], [120, 128, 120]));
    let grid_2d = images.add(grid_image(512, 32, [14, 15, 19], [26, 28, 35], [42, 45, 58]));
    let mat = |tex: Option<Handle<Image>>, base: Color| StandardMaterial { base_color: base, base_color_texture: tex, perceptual_roughness: 0.95, reflectance: 0.05, ..default() };
    let plain = materials.add(mat(None, Color::srgb(0.07, 0.072, 0.09)));
    let grid = materials.add(mat(Some(grid_tex), Color::WHITE));
    let sky = materials.add(mat(Some(sky_tex), Color::WHITE));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(12.0)))),
        MeshMaterial3d(plain.clone()),
        Transform::default(),
        layer.clone(),
        PreviewGround,
        HideInHierarchy,
        EditorLocked,
        Name::new("VFX Preview Ground"),
    ));
    commands.spawn((
        Sprite { image: grid_2d.clone(), custom_size: Some(Vec2::splat(4096.0)), ..default() },
        Transform::from_xyz(0.0, 0.0, -500.0),
        Visibility::Hidden,
        layer,
        PreviewBackdrop2d,
        HideInHierarchy,
        EditorLocked,
        Name::new("VFX Preview Backdrop 2D"),
    ));
    commands.insert_resource(GroundMaterials { plain, grid, sky, grid_2d });
}

fn visible(dock: Option<&Dock>, fixed: Option<&FixedDock>, wins: Option<&DockWindows>) -> bool {
    renzora_ember::dock::panel_visible_anywhere(PANEL, dock, fixed, wins)
}

fn sync_cameras(
    state: Res<VfxEditorState>,
    dock: Option<Res<Dock>>,
    fixed: Option<Res<FixedDock>>,
    wins: Option<Res<DockWindows>>,
    mut cams3: Query<&mut Camera, (With<PreviewCamera3d>, Without<PreviewCamera2d>)>,
    mut cams2: Query<&mut Camera, (With<PreviewCamera2d>, Without<PreviewCamera3d>)>,
) {
    let on = visible(dock.as_deref(), fixed.as_deref(), wins.as_deref()) && state.is_open() && state.load_error.is_none();
    let two_d = state.doc.dimension == Dimension::TwoD;
    for mut c in &mut cams3 {
        let want = on && !two_d;
        if c.is_active != want {
            c.is_active = want;
        }
    }
    for mut c in &mut cams2 {
        let want = on && two_d;
        if c.is_active != want {
            c.is_active = want;
        }
    }
}

fn report_size(panel: Query<&ComputedNode, With<PreviewViewport>>, mut img: ResMut<VfxPreviewImage>) {
    if let Some(cn) = panel.iter().next() {
        let s = cn.size();
        let want = UVec2::new((s.x.round() as u32).max(1), (s.y.round() as u32).max(1));
        if img.requested != want {
            img.requested = want;
        }
    }
}

fn resize(mut img: ResMut<VfxPreviewImage>, mut images: ResMut<Assets<Image>>) {
    if img.requested == img.current {
        return;
    }
    let w = img.requested.x.clamp(64, 3840);
    let h = img.requested.y.clamp(64, 2160);
    if let Some(mut image) = images.get_mut(&img.handle) {
        image.resize(Extent3d { width: w, height: h, depth_or_array_layers: 1 });
        img.current = img.requested;
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Drag {
    Rotate,
    Pan,
}

/// Left-drag orbits (pans in 2D), right/middle-drag pans, the wheel zooms;
/// only while the press started over the preview image.
fn orbit_input(
    hover: Query<&UiInteraction, With<PreviewViewport>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    state: Res<VfxEditorState>,
    mut orbit: ResMut<VfxPreviewOrbit>,
    mut drag: Local<Option<Drag>>,
) {
    let over = hover.iter().any(|i| matches!(i, UiInteraction::Hovered | UiInteraction::Pressed));
    if drag.is_none() && over {
        if mouse.just_pressed(MouseButton::Left) {
            *drag = Some(Drag::Rotate);
        } else if mouse.just_pressed(MouseButton::Right) || mouse.just_pressed(MouseButton::Middle) {
            *drag = Some(Drag::Pan);
        }
    }
    if !mouse.pressed(MouseButton::Left) && !mouse.pressed(MouseButton::Right) && !mouse.pressed(MouseButton::Middle) {
        *drag = None;
    }
    let two_d = state.doc.dimension == Dimension::TwoD;
    let d = motion.delta;
    if let Some(mode) = *drag {
        if d != Vec2::ZERO {
            if two_d {
                let zoom = orbit.zoom_2d;
                orbit.pan_2d += Vec2::new(-d.x, d.y) * zoom;
            } else if mode == Drag::Rotate {
                orbit.yaw -= d.x * 0.01;
                orbit.pitch = (orbit.pitch + d.y * 0.01).clamp(-0.2, 1.45);
            } else {
                let right = Vec3::new(orbit.yaw.cos(), 0.0, -orbit.yaw.sin());
                let s = (orbit.distance * 0.0015).max(0.0005);
                orbit.target += right * (-d.x * s) + Vec3::Y * (d.y * s);
            }
        }
    }
    if over && scroll.delta.y != 0.0 {
        let k = 1.0 - scroll.delta.y * 0.1;
        if two_d {
            orbit.zoom_2d = (orbit.zoom_2d * k).clamp(0.1, 10.0);
        } else {
            orbit.distance = (orbit.distance * k).clamp(0.5, 60.0);
        }
    }
}

fn place_cameras(
    orbit: Res<VfxPreviewOrbit>,
    mut cams3: Query<&mut Transform, (With<PreviewCamera3d>, Without<PreviewCamera2d>)>,
    mut cams2: Query<(&mut Transform, &mut Projection), (With<PreviewCamera2d>, Without<PreviewCamera3d>)>,
) {
    if !orbit.is_changed() {
        return;
    }
    let o = *orbit;
    for mut t in &mut cams3 {
        let offset = Vec3::new(o.pitch.cos() * o.yaw.sin(), o.pitch.sin(), o.pitch.cos() * o.yaw.cos()) * o.distance;
        *t = Transform::from_translation(o.target + offset).looking_at(o.target, Vec3::Y);
    }
    for (mut t, mut p) in &mut cams2 {
        t.translation = Vec3::new(o.pan_2d.x, o.pan_2d.y, 900.0);
        if let Projection::Orthographic(ortho) = &mut *p {
            if (ortho.scale - o.zoom_2d).abs() > f32::EPSILON {
                ortho.scale = o.zoom_2d;
            }
        }
    }
}

/// Keep one preview effect entity, pointing at the open document's path.
fn sync_effect(mut commands: Commands, state: Res<VfxEditorState>, mut effects: Query<(Entity, &mut ParticleSystem), With<PreviewEffect>>) {
    let path = state.path.clone().filter(|_| state.load_error.is_none());
    match (path, effects.iter_mut().next()) {
        (Some(path), Some((_, mut system))) => {
            if system.path != path {
                system.path = path;
            }
        }
        (Some(path), None) => {
            let e = commands
                .spawn((
                    ParticleSystem::from_path(path),
                    Transform::default(),
                    RenderLayers::layer(VFX_PREVIEW_LAYER),
                    PreviewEffect,
                    HideInHierarchy,
                    EditorLocked,
                    Name::new("VFX Preview Effect"),
                ))
                .id();
            // Not pickable from the scene viewport: it is not scene content.
            commands.entity(e).remove::<renzora::PickProxy>();
        }
        (None, Some((e, _))) => commands.entity(e).despawn(),
        (None, None) => {}
    }
}

/// Speed, motion, loop and restart-on-edit, applied to the preview entity.
fn drive_effect(
    mut commands: Commands,
    time: Res<Time<Real>>,
    settings: Res<VfxPreviewSettings>,
    state: Res<VfxEditorState>,
    restart: Option<Res<RestartPreview>>,
    link: Option<Res<crate::timeline::TimelineLink>>,
    mut effects: Query<(Entity, &mut Transform, &mut ParticlePlayback, Option<&ParticleRuntime>), With<PreviewEffect>>,
    mut phase: Local<f32>,
    mut restarted_for: Local<Option<f64>>,
) {
    let Some((entity, mut transform, mut playback, runtime)) = effects.iter_mut().next() else { return };
    // The timeline's playhead holds this effect's clock: restarting it here
    // would only be undone by the timeline's next seek.
    let driven = link.is_some_and(|l| l.driving == Some(entity));
    let speed = SPEEDS.get(settings.speed).copied().unwrap_or(1.0);
    if playback.time_scale != speed {
        playback.time_scale = speed;
    }
    let mut restart_now = restart.is_some();
    if restart.is_some() {
        commands.remove_resource::<RestartPreview>();
    }
    if settings.restart_on_edit {
        if let Some(at) = state.last_edit {
            if *restarted_for != Some(at) && time.elapsed_secs_f64() - at > RESTART_SETTLE {
                *restarted_for = Some(at);
                restart_now = true;
            }
        }
    }
    if settings.looping && runtime.is_some_and(|r| r.sim.is_finished()) {
        restart_now = true;
    }
    if restart_now && !driven {
        playback.restart();
        *phase = 0.0;
    }
    *phase += time.delta_secs() * speed;
    let two_d = state.doc.dimension == Dimension::TwoD;
    let radius = if two_d { 160.0 } else { 1.6 };
    let o = settings.motion.offset(*phase) * radius;
    let want = if two_d { Vec3::new(o.x, o.y, 0.0) } else { Vec3::new(o.x, 0.0, o.y) };
    if transform.translation != want {
        transform.translation = want;
    }
}

fn sync_backdrop(
    settings: Res<VfxPreviewSettings>,
    state: Res<VfxEditorState>,
    mats: Option<Res<GroundMaterials>>,
    mut cams: Query<&mut Camera, Or<(With<PreviewCamera3d>, With<PreviewCamera2d>)>>,
    mut ground: Query<(&mut MeshMaterial3d<StandardMaterial>, &mut Visibility), (With<PreviewGround>, Without<PreviewBackdrop2d>)>,
    mut backdrop: Query<(&mut Sprite, &mut Visibility), (With<PreviewBackdrop2d>, Without<PreviewGround>)>,
) {
    if !settings.is_changed() && !state.is_changed() {
        return;
    }
    let Some(mats) = mats else { return };
    let b = settings.backdrop;
    for mut c in &mut cams {
        c.clear_color = ClearColorConfig::Custom(b.clear());
    }
    let two_d = state.doc.dimension == Dimension::TwoD;
    for (mut m, mut v) in &mut ground {
        let want = match b {
            Backdrop::Dark => mats.plain.clone(),
            Backdrop::Grid => mats.grid.clone(),
            Backdrop::Sky => mats.sky.clone(),
        };
        if m.0 != want {
            m.0 = want;
        }
        v.set_if_neq(if two_d { Visibility::Hidden } else { Visibility::Inherited });
    }
    for (mut s, mut v) in &mut backdrop {
        if s.image != mats.grid_2d {
            s.image = mats.grid_2d.clone();
        }
        v.set_if_neq(if two_d && b == Backdrop::Grid { Visibility::Inherited } else { Visibility::Hidden });
    }
}

fn collect_stats(
    state: Res<VfxEditorState>,
    effects: Query<Option<&ParticleRuntime>, With<PreviewEffect>>,
    mut stats: ResMut<VfxPreviewStats>,
) {
    let runtime = effects.iter().next().flatten();
    let mut next = VfxPreviewStats::default();
    if let Some(r) = runtime {
        next.alive = r.sim.alive() as u32;
        next.dropped = r.sim.dropped();
        next.time = r.sim.time;
    }
    next.emitters = state
        .doc
        .emitters
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let sim = runtime.and_then(|r| r.sim.emitters.iter().find(|s| s.emitter_index == i));
            (e.name.clone(), sim.map_or(0, |s| s.alive() as u32), e.capacity, sim.is_some())
        })
        .collect();
    next.backends = (0..state.doc.emitters.len())
        .map(|i| {
            let sim = runtime.and_then(|r| r.sim.emitters.iter().find(|s| s.emitter_index == i))?;
            Some((sim.backend.active, sim.backend.reason.clone()))
        })
        .collect();
    // Seconds tick every frame; only republish when the chips would change.
    let mut cmp = next.clone();
    cmp.time = stats.time;
    if *stats != cmp || (next.time - stats.time).abs() > 0.1 {
        *stats = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_motion_starts_at_the_origin_or_on_its_path() {
        assert_eq!(Motion::Still.offset(3.0), Vec2::ZERO);
        assert!((Motion::Circle.offset(0.0) - Vec2::X).length() < 1e-5);
        assert!(Motion::Figure8.offset(0.0).length() < 1e-5);
    }

    #[test]
    fn the_default_speed_is_real_time() {
        assert_eq!(SPEEDS[VfxPreviewSettings::default().speed], 1.0);
    }

    #[test]
    fn the_grid_texture_has_the_requested_size() {
        let img = grid_image(64, 8, [0; 3], [1; 3], [2; 3]);
        assert_eq!(img.texture_descriptor.size.width, 64);
        assert_eq!(img.data.as_ref().map(Vec::len), Some(64 * 64 * 4));
    }
}
