//! Light gizmos.
//!
//! When a light is selected, draw 3D wireframes that visualise its falloff
//! (point radius sphere, spot inner/outer cone, sun direction arrow) using
//! Bevy's immediate-mode gizmos.
//!
//! Note: the always-visible phosphor scene-icon overlay was dropped in the
//! egui purge and is pending a native re-implementation. The
//! [`SceneIconCache`] icon-position bookkeeping is retained for that future
//! port.

use bevy::light::{LightProbe, ParallaxCorrection};
use bevy::prelude::*;

use renzora::SceneCamera;
use renzora_editor_framework::{EditorCamera, EditorSelection};
use renzora_lighting::Sun;

use crate::OverlayGizmoGroup;

/// Cached scene-icon positions so the overlay drawer (which only has
/// `&World`) doesn't need to run a query. Populated each frame by
/// [`update_scene_icon_cache`].
#[derive(Resource, Default)]
pub struct SceneIconCache {
    pub editor_camera: Option<Entity>,
    /// Editor 2D camera reference — same purpose as `editor_camera`, but
    /// for the orthographic 2D editor camera. Updated by a separate
    /// always-on system because the 3D icon-cache update only runs in 3D
    /// view, while 2D overlays need the 2D camera looked up in 2D view.
    pub editor_camera_2d: Option<Entity>,
    pub light_icons: Vec<(Vec3, &'static str)>,
    pub camera_icons: Vec<Vec3>,
}

/// Always-on updater that just refreshes `SceneIconCache.editor_camera_2d`.
/// Sibling of `update_scene_icon_cache`; lives outside that system so it
/// can run even when the viewport is in 2D view (where the 3D cache
/// updater is gated off).
pub fn update_editor_camera_2d_cache(
    mut cache: ResMut<SceneIconCache>,
    editor_camera_2d: Query<Entity, With<renzora::core::EditorCamera2d>>,
) {
    cache.editor_camera_2d = editor_camera_2d.single().ok();
}

#[allow(clippy::type_complexity)]
pub fn update_scene_icon_cache(
    mut cache: ResMut<SceneIconCache>,
    editor_camera: Query<Entity, With<EditorCamera>>,
    suns: Query<&GlobalTransform, With<Sun>>,
    dir_lights: Query<&GlobalTransform, (With<DirectionalLight>, Without<Sun>)>,
    point_lights: Query<&GlobalTransform, With<PointLight>>,
    spot_lights: Query<&GlobalTransform, With<SpotLight>>,
    // Restrict camera icons to entities marked `SceneCamera` — these are the
    // user-placed cameras visible in the hierarchy. GLB models routinely
    // bake `Camera3d` nodes deep inside their entity tree (one per imported
    // model variant), and showing an icon for each would clutter the
    // viewport with cameras the user never authored.
    scene_cameras: Query<&GlobalTransform, (With<SceneCamera>, Without<EditorCamera>)>,
) {
    cache.editor_camera = editor_camera.single().ok();

    cache.light_icons.clear();
    for gt in &suns {
        cache
            .light_icons
            .push((gt.translation(), "sun-horizon"));
    }
    for gt in &dir_lights {
        cache.light_icons.push((gt.translation(), "sun"));
    }
    for gt in &point_lights {
        cache.light_icons.push((gt.translation(), "lightbulb"));
    }
    for gt in &spot_lights {
        cache
            .light_icons
            .push((gt.translation(), "flashlight"));
    }

    cache.camera_icons.clear();
    for gt in &scene_cameras {
        cache.camera_icons.push(gt.translation());
    }
}

const SPOT_COLOR: Color = Color::srgb(1.0, 0.78, 0.35);
const POINT_COLOR: Color = Color::srgb(1.0, 0.85, 0.35);
const SUN_COLOR: Color = Color::srgb(1.0, 0.92, 0.55);
const AREA_COLOR: Color = Color::srgb(0.55, 0.8, 1.0);
const PROBE_COLOR: Color = Color::srgb(0.45, 0.95, 0.85);

// ── Selection-only 3D wireframe extras ──────────────────────────────────────

pub fn draw_light_gizmos(
    mut gizmos: Gizmos<OverlayGizmoGroup>,
    selection: Res<EditorSelection>,
    point_lights: Query<(Entity, &GlobalTransform, &PointLight)>,
    spot_lights: Query<(Entity, &GlobalTransform, &SpotLight)>,
    dir_lights: Query<(Entity, &GlobalTransform, &DirectionalLight)>,
    rect_lights: Query<(Entity, &GlobalTransform, &RectLight)>,
    probes: Query<(Entity, &GlobalTransform, Option<&ParallaxCorrection>), With<LightProbe>>,
) {
    let selected = selection.get();

    // Area lights have no mesh and no scene icon, so — unlike the other light
    // gizmos — draw the rectangle for *every* RectLight (dim), brightening the
    // selected one. The rect lies in the light's local XY plane and emits along
    // local -Z, mirroring bevy's own `rect_light_gizmo`.
    for (entity, gt, rect) in rect_lights.iter() {
        let (_, rotation, translation) = gt.to_scale_rotation_translation();
        let size = Vec2::new(rect.width.max(0.001), rect.height.max(0.001));
        let is_selected = Some(entity) == selected;
        let c = with_alpha(AREA_COLOR, if is_selected { 0.95 } else { 0.45 });
        gizmos.rect(Isometry3d::new(translation, rotation), size, c);
        // Emission-direction arrow (local -Z), short like bevy's.
        let dir = rotation * Vec3::NEG_Z;
        draw_arrow(&mut gizmos, translation, dir, 0.6, c);
        // Diagonals make the panel read clearly as a filled emitter when
        // selected, without cluttering every unselected light.
        if is_selected {
            let hx = rotation * (Vec3::X * size.x * 0.5);
            let hy = rotation * (Vec3::Y * size.y * 0.5);
            gizmos.line(translation - hx - hy, translation + hx + hy, c);
            gizmos.line(translation - hx + hy, translation + hx - hy, c);
        }
    }

    // Reflection probes have no mesh either: draw the parallax-correction box
    // for *every* probe (dim), brightening the selected one. The box is the
    // probe's unit cube under its Transform (Auto/None), or scaled to the
    // `Custom` half-extents (in probe space) — so it shows exactly the volume
    // the cubemap is corrected against.
    for (entity, gt, parallax) in probes.iter() {
        let half = match parallax {
            Some(ParallaxCorrection::Custom(v)) => *v,
            _ => Vec3::splat(0.5),
        };
        let mut t = gt.compute_transform();
        t.scale *= half * 2.0;
        let is_selected = Some(entity) == selected;
        gizmos.cube(t, with_alpha(PROBE_COLOR, if is_selected { 0.9 } else { 0.35 }));
    }

    let Some(selected) = selected else {
        return;
    };

    if let Ok((_, gt, light)) = point_lights.get(selected) {
        let pos = gt.translation();
        let r = light.range.max(0.01);
        let c = with_alpha(POINT_COLOR, 0.55);
        gizmos.circle(Isometry3d::new(pos, Quat::IDENTITY), r, c);
        gizmos.circle(
            Isometry3d::new(pos, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            r,
            c,
        );
        gizmos.circle(
            Isometry3d::new(pos, Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
            r,
            c,
        );
    }

    if let Ok((_, gt, light)) = spot_lights.get(selected) {
        let pos = gt.translation();
        let rot = gt.rotation();
        let range = light.range.max(0.01);
        let outer = light.outer_angle.clamp(0.0, std::f32::consts::FRAC_PI_2);
        let inner = light.inner_angle.clamp(0.0, outer);
        draw_spot_cone(
            &mut gizmos,
            pos,
            rot,
            range,
            outer,
            with_alpha(SPOT_COLOR, 0.65),
        );
        if inner > 0.001 && (outer - inner).abs() > 0.01 {
            draw_spot_cone(
                &mut gizmos,
                pos,
                rot,
                range,
                inner,
                with_alpha(SPOT_COLOR, 0.4),
            );
        }
    }

    if let Ok((_, gt, _)) = dir_lights.get(selected) {
        let pos = gt.translation();
        let dir = gt.rotation() * Vec3::NEG_Z;
        draw_arrow(&mut gizmos, pos, dir, 2.0, with_alpha(SUN_COLOR, 0.9));
    }
}

fn with_alpha(c: Color, a: f32) -> Color {
    let s = c.to_srgba();
    Color::srgba(s.red, s.green, s.blue, a)
}

fn draw_spot_cone(
    gizmos: &mut Gizmos<OverlayGizmoGroup>,
    pos: Vec3,
    rot: Quat,
    range: f32,
    half_angle: f32,
    color: Color,
) {
    let forward = rot * Vec3::NEG_Z;
    let right = rot * Vec3::X;
    let up = rot * Vec3::Y;
    let radius = range * half_angle.tan();
    let tip_center = pos + forward * range;
    let segments = 24;
    let mut prev = Vec3::ZERO;
    for i in 0..=segments {
        let a = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let p = tip_center + (right * a.cos() + up * a.sin()) * radius;
        if i > 0 {
            gizmos.line(prev, p, color);
        }
        prev = p;
        if i % 6 == 0 {
            gizmos.line(pos, p, color);
        }
    }
}

fn draw_arrow(
    gizmos: &mut Gizmos<OverlayGizmoGroup>,
    origin: Vec3,
    dir: Vec3,
    length: f32,
    color: Color,
) {
    let dir = dir.try_normalize().unwrap_or(Vec3::NEG_Z);
    let tip = origin + dir * length;
    gizmos.line(origin, tip, color);
    let any_up = if dir.y.abs() > 0.99 { Vec3::X } else { Vec3::Y };
    let side = dir.cross(any_up).normalize();
    let up = side.cross(dir).normalize();
    let head = length * 0.15;
    let back = tip - dir * head;
    gizmos.line(tip, back + side * head * 0.6, color);
    gizmos.line(tip, back - side * head * 0.6, color);
    gizmos.line(tip, back + up * head * 0.6, color);
    gizmos.line(tip, back - up * head * 0.6, color);
}

