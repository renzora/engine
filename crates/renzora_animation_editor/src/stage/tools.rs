//! What the left mouse button does on the stage, and what is drawn over it.
//!
//! - **Bones**: a click near a bone selects it (Shift adds). A bone is drawn
//!   from its joint to its child's, and belongs to the joint it turns around,
//!   so clicking the forearm selects the forearm. The selection is the real
//!   model's bone, so the timeline, the Inspector and keying all see it.
//! - **The gizmo**: rings to turn the selected bone (E), arrows and a centre
//!   handle to move it (W). It edits the real bone's transform, which the pose
//!   layer then holds over the clip until it is keyed, exactly as the scene
//!   gizmo does; each drag is one undo step.
//! - **Rig points**: while a point is being placed, a click on the model drops
//!   it there ([`place_marker`]).
//!
//! Everything is drawn with the studio preview's gizmo group, which only the
//! stage camera sees.

use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings};
use bevy::prelude::*;

use super::clone::StageRig;
use super::view::{StageCursor, StageNav};
use super::{StageMode, StageUi, Tool};
use crate::rig::markers::{MarkerSet, Markers};
use crate::studio_preview::{StudioPreviewCamera, StudioPreviewGizmoGroup, StudioPreviewModel};

/// How near the cursor must come to a handle or a bone, in pixels.
const REACH: f32 = 9.0;

/// A part of the gizmo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Grip {
    /// Turn about an axis (0, 1, 2), or about the view (3).
    Ring(usize),
    /// Move along an axis.
    Arrow(usize),
    /// Move in the plane facing the camera.
    Centre,
}

struct Drag {
    grip: Grip,
    real: Entity,
    start: Transform,
    world_rotation: Quat,
    world_position: Vec3,
    parent: Mat4,
    /// The parent's world turn, decomposed rather than read off `parent`: a
    /// Mixamo armature is scaled to a hundredth, and a quaternion taken
    /// straight from a scaled matrix is not a rotation.
    parent_rotation: Quat,
    axis: Vec3,
    centre_px: Vec2,
    previous_angle: f32,
    turned: f32,
    cursor_start: Vec2,
    plane_start: Option<Vec3>,
    camera_forward: Vec3,
    radius: f32,
}

#[derive(Resource, Default)]
pub(crate) struct StageGizmo {
    pub(crate) hover: Option<Grip>,
    drag: Option<Drag>,
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<StageGizmo>().add_systems(
        Update,
        (pointer, place_marker, draw)
            .chain()
            .after(super::view::StageInputSet)
            .run_if(in_state(renzora_editor_framework::SplashState::Editor))
            .run_if(super::stage_mounted)
            .run_if(renzora::not_in_play_mode),
    );
}

/// The selected bone's gizmo, where it is and how it is turned.
struct Frame {
    real: Entity,
    centre: Vec3,
    rotation: Quat,
    axes: [Vec3; 3],
    radius: f32,
    camera: Camera,
    camera_gt: GlobalTransform,
}

fn camera(world: &mut World) -> Option<(Camera, GlobalTransform)> {
    let mut q = world.query_filtered::<(&Camera, &GlobalTransform), With<StudioPreviewCamera>>();
    q.iter(world).next().map(|(c, g)| (c.clone(), *g))
}

fn frame(world: &mut World) -> Option<Frame> {
    let (camera, camera_gt) = camera(world)?;
    let real = world.get_resource::<renzora_editor_framework::EditorSelection>()?.get()?;
    let copy = world.resource::<StageRig>().clone_of(real)?;
    let gt = *world.get::<GlobalTransform>(copy)?;
    let local = world.get_resource::<StageUi>().is_none_or(|u| u.local);
    let rotation = gt.rotation();
    let axes = if local { [rotation * Vec3::X, rotation * Vec3::Y, rotation * Vec3::Z] } else { [Vec3::X, Vec3::Y, Vec3::Z] };
    let centre = gt.translation();
    let radius = (camera_gt.translation().distance(centre) * 0.13).max(1e-4);
    Some(Frame { real, centre, rotation, axes, radius, camera, camera_gt })
}

fn to_px(f: &Frame, p: Vec3) -> Option<Vec2> {
    f.camera.world_to_viewport(&f.camera_gt, p).ok()
}

/// Points around a ring, for hit-testing and drawing.
fn ring_points(centre: Vec3, axis: Vec3, radius: f32, n: usize) -> Vec<Vec3> {
    let (u, v) = axis.normalize().any_orthonormal_pair();
    (0..=n).map(|i| {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        centre + (u * a.cos() + v * a.sin()) * radius
    }).collect()
}

fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() < 1e-6 { 0.0 } else { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) };
    p.distance(a + ab * t)
}

fn view_axis(f: &Frame) -> Vec3 {
    (f.camera_gt.translation() - f.centre).normalize_or(Vec3::Z)
}

/// Which grip the cursor is on, if any.
fn grip_under(f: &Frame, tool: Tool, cursor: Vec2) -> Option<Grip> {
    let mut best: Option<(f32, Grip)> = None;
    let mut consider = |d: f32, g: Grip| {
        if d < REACH && best.is_none_or(|(b, _)| d < b) {
            best = Some((d, g));
        }
    };
    match tool {
        Tool::Rotate => {
            for (i, axis) in f.axes.iter().chain(std::iter::once(&view_axis(f))).enumerate() {
                let r = if i == 3 { f.radius * 1.18 } else { f.radius };
                let pts: Vec<Vec2> = ring_points(f.centre, *axis, r, 48).into_iter().filter_map(|p| to_px(f, p)).collect();
                let d = pts.windows(2).map(|w| distance_to_segment(cursor, w[0], w[1])).fold(f32::MAX, f32::min);
                consider(d, Grip::Ring(i));
            }
        }
        Tool::Move => {
            let c = to_px(f, f.centre)?;
            for (i, axis) in f.axes.iter().enumerate() {
                if let Some(tip) = to_px(f, f.centre + *axis * f.radius * 1.1) {
                    consider(distance_to_segment(cursor, c, tip), Grip::Arrow(i));
                }
            }
            consider(cursor.distance(c) * 0.7, Grip::Centre);
        }
    }
    best.map(|(_, g)| g)
}

/// Angle of the cursor around the gizmo's centre on screen, anticlockwise.
fn screen_angle(centre: Vec2, p: Vec2) -> f32 {
    (-(p.y - centre.y)).atan2(p.x - centre.x)
}

fn intersect_plane(ray: Ray3d, point: Vec3, normal: Vec3) -> Option<Vec3> {
    let denom = ray.direction.dot(normal);
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = (point - ray.origin).dot(normal) / denom;
    (t >= 0.0).then(|| ray.origin + *ray.direction * t)
}

/// The left button on the stage: the gizmo first, then the bones.
fn pointer(world: &mut World) {
    let mode = super::mode(world);
    let (pressed, just_pressed, released) = {
        let m = world.resource::<ButtonInput<MouseButton>>();
        (m.pressed(MouseButton::Left), m.just_pressed(MouseButton::Left), m.just_released(MouseButton::Left))
    };
    let (shift, ctrl) = {
        let k = world.resource::<ButtonInput<KeyCode>>();
        (k.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]), k.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]))
    };
    let (over, cursor, ray) = {
        let c = world.resource::<StageCursor>();
        (c.over, c.px, c.ray)
    };
    let navigating = world.resource::<StageNav>().dragging.is_some();
    let tool = world.get_resource::<StageUi>().map(|u| u.tool).unwrap_or_default();
    let posing = matches!(mode, StageMode::Animate | StageMode::Logic);

    // A drag in progress.
    if world.resource::<StageGizmo>().drag.is_some() {
        if !pressed || released {
            finish_drag(world);
        } else if let Some(ray) = ray {
            continue_drag(world, cursor, ray);
        }
        return;
    }

    let f = if posing { frame(world) } else { None };
    let hover = f.as_ref().filter(|_| over && !navigating).and_then(|f| grip_under(f, tool, cursor));
    world.resource_mut::<StageGizmo>().hover = hover;

    if !just_pressed || !over || navigating {
        return;
    }
    if world.get_resource::<Markers>().is_some_and(|m| m.placing.is_some()) {
        return;
    }
    // Skin paints with the left button; Ctrl+click still picks a bone there,
    // which is how the bone to paint is chosen on the model.
    if mode == StageMode::Skin && !ctrl {
        return;
    }
    if let (Some(f), Some(grip)) = (f, hover) {
        start_drag(world, f, grip, cursor, ray);
        return;
    }
    pick_bone(world, cursor, shift);
}

fn start_drag(world: &mut World, f: Frame, grip: Grip, cursor: Vec2, ray: Option<Ray3d>) {
    let Some(start) = world.get::<Transform>(f.real).copied() else { return };
    let copy = world.resource::<StageRig>().clone_of(f.real);
    let parent_gt = copy.and_then(|c| world.get::<ChildOf>(c)).and_then(|p| world.get::<GlobalTransform>(p.parent())).copied();
    let parent = parent_gt.map(|g| g.to_matrix()).unwrap_or(Mat4::IDENTITY);
    let parent_rotation = parent_gt.map(|g| g.rotation()).unwrap_or(Quat::IDENTITY);
    let axis = match grip {
        Grip::Ring(3) => view_axis(&f),
        Grip::Ring(i) | Grip::Arrow(i) => f.axes[i],
        Grip::Centre => view_axis(&f),
    };
    let centre_px = to_px(&f, f.centre).unwrap_or(cursor);
    let camera_forward = (f.centre - f.camera_gt.translation()).normalize_or(Vec3::NEG_Z);
    let plane_start = ray.and_then(|r| intersect_plane(r, f.centre, camera_forward));
    world.resource_mut::<StageGizmo>().drag = Some(Drag {
        grip,
        real: f.real,
        start,
        world_rotation: f.rotation,
        world_position: f.centre,
        parent,
        parent_rotation,
        axis,
        centre_px,
        previous_angle: screen_angle(centre_px, cursor),
        turned: 0.0,
        cursor_start: cursor,
        plane_start,
        camera_forward,
        radius: f.radius,
    });
}

fn continue_drag(world: &mut World, cursor: Vec2, ray: Ray3d) {
    let Some((camera, camera_gt)) = camera(world) else { return };
    let new = {
        let mut g = world.resource_mut::<StageGizmo>();
        let Some(d) = g.drag.as_mut() else { return };
        let mut t = d.start;
        match d.grip {
            Grip::Ring(_) => {
                let angle = screen_angle(d.centre_px, cursor);
                let mut delta = angle - d.previous_angle;
                if delta > std::f32::consts::PI {
                    delta -= std::f32::consts::TAU;
                } else if delta < -std::f32::consts::PI {
                    delta += std::f32::consts::TAU;
                }
                d.previous_angle = angle;
                // Seen from the camera, a positive turn about an axis that
                // points at the camera goes anticlockwise.
                let facing = if d.axis.dot(-d.camera_forward) >= 0.0 { 1.0 } else { -1.0 };
                d.turned += delta * facing;
                let world_rotation = Quat::from_axis_angle(d.axis, d.turned) * d.world_rotation;
                t.rotation = (d.parent_rotation.inverse() * world_rotation).normalize();
            }
            Grip::Arrow(_) => {
                let (Ok(c), Ok(tip)) = (
                    camera.world_to_viewport(&camera_gt, d.world_position),
                    camera.world_to_viewport(&camera_gt, d.world_position + d.axis * d.radius),
                ) else {
                    return;
                };
                let dir = tip - c;
                if dir.length_squared() < 1.0 {
                    return;
                }
                let along = (cursor - d.cursor_start).dot(dir) / dir.length_squared();
                let target = d.world_position + d.axis * d.radius * along;
                t.translation = d.parent.inverse().transform_point3(target);
            }
            Grip::Centre => {
                let (Some(p0), Some(p)) = (d.plane_start, intersect_plane(ray, d.world_position, d.camera_forward)) else { return };
                let target = d.world_position + (p - p0);
                t.translation = d.parent.inverse().transform_point3(target);
            }
        }
        (d.real, t)
    };
    if let Some(mut tr) = world.get_mut::<Transform>(new.0) {
        *tr = new.1;
    }
}

fn finish_drag(world: &mut World) {
    let Some(d) = world.resource_mut::<StageGizmo>().drag.take() else { return };
    let Some(now) = world.get::<Transform>(d.real).copied() else { return };
    if now == d.start {
        return;
    }
    renzora_undo::record(world, renzora_undo::UndoContext::Scene, Box::new(renzora_undo::TransformCmd { entity: d.real, old: d.start, new: now }));
}

/// Select the bone nearest the cursor, if one is near enough.
fn pick_bone(world: &mut World, cursor: Vec2, add: bool) {
    let Some((camera, camera_gt)) = camera(world) else { return };
    let hit = {
        let rig = world.resource::<StageRig>();
        let px = |e: Entity| world.get::<GlobalTransform>(e).and_then(|g| camera.world_to_viewport(&camera_gt, g.translation()).ok());
        let mut best: Option<(f32, Entity)> = None;
        let mut consider = |d: f32, owner: Entity| {
            if d < REACH * 1.4 && best.is_none_or(|(b, _)| d < b) {
                best = Some((d, owner));
            }
        };
        let has_child: std::collections::HashSet<Entity> = rig.parent.values().copied().collect();
        for &j in &rig.joints {
            let Some(pj) = px(j) else { continue };
            match rig.parent.get(&j).and_then(|&p| Some((p, px(p)?))) {
                // The segment into this joint belongs to its parent.
                Some((p, pp)) => consider(distance_to_segment(cursor, pp, pj), p),
                None => consider(cursor.distance(pj), j),
            }
            if !has_child.contains(&j) {
                consider(cursor.distance(pj), j);
            }
        }
        best.and_then(|(_, owner)| rig.real_of(owner))
    };
    let Some(real) = hit else { return };
    if let Some(sel) = world.get_resource::<renzora_editor_framework::EditorSelection>() {
        if add {
            sel.toggle(real);
        } else {
            sel.set(Some(real));
        }
    }
}

/// While a rig point is being placed, a click on the model on the stage
/// drops it there, in the model's own space (the copy sits at the origin
/// unturned, so where it is hit is where the point is in the model), then
/// moves on to the next point still unplaced.
fn place_marker(
    mut markers: ResMut<Markers>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Res<StageCursor>,
    nav: Res<StageNav>,
    mut ray_cast: MeshRayCast,
    roots: Query<(), With<StudioPreviewModel>>,
    parents: Query<&ChildOf>,
) {
    let Some((set, index)) = markers.placing else { return };
    if !mouse.just_pressed(MouseButton::Left) || !cursor.over || nav.dragging.is_some() {
        return;
    }
    let Some(ray) = cursor.ray else { return };
    let on_copy = |mut e: Entity| loop {
        if roots.contains(e) {
            return true;
        }
        match parents.get(e) {
            Ok(p) => e = p.parent(),
            Err(_) => return false,
        }
    };
    // Filtered to the copy before testing, not after: the cast stops at its
    // first hit and ignores render layers, so the real model in the scene was
    // hit first wherever it stood between the stage camera and the copy. A
    // model sitting at the scene's origin encloses the copy completely, and
    // no click on the stage could place a point on it.
    let settings = MeshRayCastSettings::default()
        .with_visibility(bevy::picking::mesh_picking::ray_cast::RayCastVisibility::Any)
        .with_filter(&on_copy);
    let Some(point) = ray_cast.cast_ray(ray, &settings).first().map(|(_, h)| h.point) else { return };
    markers.set(set, index, point);
}

/// The skeleton, the selected bone's gizmo, and the rig points.
#[allow(clippy::too_many_arguments)]
fn draw(
    mut gizmos: Gizmos<StudioPreviewGizmoGroup>,
    ui: Res<StageUi>,
    rig: Res<StageRig>,
    gizmo: Res<StageGizmo>,
    markers: Res<Markers>,
    selection: Res<renzora_editor_framework::EditorSelection>,
    transforms: Query<&GlobalTransform>,
    cams: Query<&GlobalTransform, With<StudioPreviewCamera>>,
    skin: Option<Res<super::skin::SkinEdit>>,
) {
    let Ok(cam) = cams.single() else { return };
    let cam_pos = cam.translation();
    let selected: Vec<Entity> = selection.get_all().into_iter().filter_map(|r| rig.clone_of(r)).collect();
    let painting = skin.as_ref().and_then(|s| s.paint_bone.clone()).filter(|_| ui.mode == StageMode::Skin);

    // The same skeleton the scene viewport draws (`renzora_gizmo`'s skeleton
    // gizmo): a line per bone with a ring around it, in the shared palette.
    if ui.show_skeleton && ui.mode != StageMode::Logic {
        let has_child: std::collections::HashSet<Entity> = rig.parent.values().copied().collect();
        let colour = |e: Entity| {
            let name = rig.names.get(&e).map(String::as_str).unwrap_or("");
            let lit = selected.contains(&e) || painting.as_deref() == Some(name);
            let c = renzora::core::bone_style::bone_colour(name, lit, false);
            if lit { c } else { c.with_alpha(0.9) }
        };
        for &j in &rig.joints {
            let Ok(gj) = transforms.get(j) else { continue };
            let pj = gj.translation();
            if let Some(&p) = rig.parent.get(&j) {
                if let Ok(gp) = transforms.get(p) {
                    let pp = gp.translation();
                    let c = colour(p);
                    gizmos.line(pp, pj, c);
                    let len = pp.distance(pj);
                    if len > 1e-5 {
                        let dir = (pj - pp) / len;
                        let r = renzora::core::bone_style::ring_radius(len, cam_pos.distance(pp));
                        gizmos.circle(Isometry3d::new(pp + dir * len * 0.25, Quat::from_rotation_arc(Vec3::Z, dir)), r, c);
                    }
                }
            }
            if !has_child.contains(&j) {
                let r = (cam_pos.distance(pj) * 0.006).max(1e-4);
                gizmos.sphere(Isometry3d::from_translation(pj), r, colour(j));
            }
        }
    }

    // The gizmo on the selected bone.
    if matches!(ui.mode, StageMode::Animate | StageMode::Logic) {
        if let Some(&copy) = selected.first() {
            if let Ok(g) = transforms.get(copy) {
                let centre = g.translation();
                let radius = (cam_pos.distance(centre) * 0.13).max(1e-4);
                let rot = g.rotation();
                let axes = if ui.local { [rot * Vec3::X, rot * Vec3::Y, rot * Vec3::Z] } else { [Vec3::X, Vec3::Y, Vec3::Z] };
                let colours = [Color::srgb(0.9, 0.3, 0.3), Color::srgb(0.35, 0.8, 0.4), Color::srgb(0.32, 0.55, 1.0)];
                let hot = Color::srgb(1.0, 0.85, 0.3);
                match ui.tool {
                    Tool::Rotate => {
                        for i in 0..3 {
                            let c = if gizmo.hover == Some(Grip::Ring(i)) { hot } else { colours[i] };
                            gizmos.linestrip(ring_points(centre, axes[i], radius, 64), c);
                        }
                        let view = (cam_pos - centre).normalize_or(Vec3::Z);
                        let c = if gizmo.hover == Some(Grip::Ring(3)) { hot } else { Color::srgba(1.0, 1.0, 1.0, 0.6) };
                        gizmos.linestrip(ring_points(centre, view, radius * 1.18, 64), c);
                    }
                    Tool::Move => {
                        for i in 0..3 {
                            let c = if gizmo.hover == Some(Grip::Arrow(i)) { hot } else { colours[i] };
                            gizmos.arrow(centre, centre + axes[i] * radius * 1.1, c);
                        }
                        let c = if gizmo.hover == Some(Grip::Centre) { hot } else { Color::srgba(1.0, 1.0, 1.0, 0.7) };
                        let view = (cam_pos - centre).normalize_or(Vec3::Z);
                        gizmos.rect(Isometry3d::new(centre, Quat::from_rotation_arc(Vec3::Z, view)), Vec2::splat(radius * 0.22), c);
                    }
                }
            }
        }
    }

    // Rig points, the mirrored partner of the one-sided ones, and the
    // skeleton they will build once all are placed.
    if ui.mode == StageMode::Rig {
        let set = if ui.template == 1 { MarkerSet::Face } else { MarkerSet::Body };
        for (i, p) in markers.points(set).iter().enumerate() {
            let Some(p) = p else { continue };
            let r = (cam_pos.distance(*p) * 0.008).max(1e-5);
            let c = set.colour(i);
            gizmos.sphere(Isometry3d::from_translation(*p), r, c);
            if set.mirrored(i) {
                gizmos.sphere(Isometry3d::from_translation(Vec3::new(-p.x, p.y, p.z)), r, c.with_alpha(0.5));
            }
            if markers.placing == Some((set, i)) {
                gizmos.sphere(Isometry3d::from_translation(*p), r * 1.8, Color::WHITE);
            }
        }
        if set == MarkerSet::Body {
            if let Some(m) = markers.complete(MarkerSet::Body) {
                let body = renzora_rig::humanoid::BodyMarkers { chin: m[0], shoulder: m[1], elbow: m[2], wrist: m[3], groin: m[4], knee: m[5], ankle: m[6] };
                let bones = renzora_rig::humanoid::bones(&body);
                for b in &bones {
                    if let Some(p) = b.parent {
                        gizmos.line(bones[p].head, b.head, Color::srgba(0.95, 0.76, 0.29, 0.7));
                    }
                }
            }
        }
    }
}
