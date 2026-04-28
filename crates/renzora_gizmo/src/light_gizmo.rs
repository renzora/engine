//! Light gizmos.
//!
//! Two layers:
//! 1. **Always-visible icons** — phosphor glyphs painted on top of the
//!    viewport at each light's projected screen position via the
//!    [`ViewportOverlayRegistry`]. These act as scene icons so lights are
//!    findable/clickable from any angle without lighting a wireframe sphere
//!    in the 3D view.
//! 2. **Selection extras** — when a light is selected, draw 3D wireframes
//!    that visualise its falloff (point radius sphere, spot inner/outer
//!    cone, sun direction arrow) using Bevy's immediate-mode gizmos.

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular as icons;

use renzora::SceneCamera;
use renzora_editor::{EditorCamera, EditorSelection};
use renzora_lighting::Sun;

use crate::OverlayGizmoGroup;

/// Cached scene-icon positions so the overlay drawer (which only has
/// `&World`) doesn't need to run a query. Populated each frame by
/// [`update_scene_icon_cache`].
#[derive(Resource, Default)]
pub struct SceneIconCache {
    pub editor_camera: Option<Entity>,
    pub light_icons: Vec<(Vec3, &'static str)>,
    pub camera_icons: Vec<Vec3>,
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
        cache.light_icons.push((gt.translation(), icons::SUN_HORIZON));
    }
    for gt in &dir_lights {
        cache.light_icons.push((gt.translation(), icons::SUN));
    }
    for gt in &point_lights {
        cache.light_icons.push((gt.translation(), icons::LIGHTBULB));
    }
    for gt in &spot_lights {
        cache.light_icons.push((gt.translation(), icons::FLASHLIGHT));
    }

    cache.camera_icons.clear();
    for gt in &scene_cameras {
        cache.camera_icons.push(gt.translation());
    }
}

const SPOT_COLOR: Color = Color::srgb(1.0, 0.78, 0.35);
const POINT_COLOR: Color = Color::srgb(1.0, 0.85, 0.35);
const SUN_COLOR: Color = Color::srgb(1.0, 0.92, 0.55);

/// Icon glyph point size — sized to read like Unreal's scene icons (large
/// and easy to click) rather than tiny annotations.
pub(crate) const ICON_FONT_SIZE: f32 = 44.0;
pub(crate) const ICON_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(255, 220, 130, 235);
pub(crate) const ICON_SHADOW: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 0, 0, 200);

// ── Selection-only 3D wireframe extras ──────────────────────────────────────

pub fn draw_light_gizmos(
    mut gizmos: Gizmos<OverlayGizmoGroup>,
    selection: Res<EditorSelection>,
    point_lights: Query<(Entity, &GlobalTransform, &PointLight)>,
    spot_lights: Query<(Entity, &GlobalTransform, &SpotLight)>,
    dir_lights: Query<(Entity, &GlobalTransform, &DirectionalLight)>,
) {
    let Some(selected) = selection.get() else { return };

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
        draw_spot_cone(&mut gizmos, pos, rot, range, outer, with_alpha(SPOT_COLOR, 0.65));
        if inner > 0.001 && (outer - inner).abs() > 0.01 {
            draw_spot_cone(&mut gizmos, pos, rot, range, inner, with_alpha(SPOT_COLOR, 0.4));
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

// ── Phosphor icon overlay (always-visible scene icons) ──────────────────────

/// Painter callback registered with [`ViewportOverlayRegistry`]. Iterates
/// every entity that carries a light component, projects its world position
/// through the editor camera, and paints a phosphor glyph at the resulting
/// screen position.
pub fn draw_light_icon_overlay(ui: &mut egui::Ui, world: &World, rect: egui::Rect) {
    if !icons_enabled(world) {
        return;
    }
    let Some(cache) = world.get_resource::<SceneIconCache>() else { return };
    let Some(cam_entity) = cache.editor_camera else { return };
    let Some(camera) = world.get::<Camera>(cam_entity) else { return };
    let Some(cam_gt) = world.get::<GlobalTransform>(cam_entity) else { return };
    let painter = ui.painter_at(rect);
    let font = egui::FontId::proportional(ICON_FONT_SIZE);

    for &(world_pos, glyph) in &cache.light_icons {
        let Some(pos) = project_world_to_rect(camera, cam_gt, world_pos, rect) else {
            continue;
        };
        painter.text(
            pos + egui::vec2(1.0, 1.0),
            egui::Align2::CENTER_CENTER,
            glyph,
            font.clone(),
            ICON_SHADOW,
        );
        painter.text(
            pos,
            egui::Align2::CENTER_CENTER,
            glyph,
            font.clone(),
            ICON_COLOR,
        );
    }
}

/// Whether scene icons should render. Read from `ViewportSettings` so the
/// Display dropdown checkbox controls both the light and camera overlays.
pub(crate) fn icons_enabled(world: &World) -> bool {
    world
        .get_resource::<renzora::core::viewport_types::ViewportSettings>()
        .map(|s| s.show_scene_icons)
        .unwrap_or(true)
}

/// Project a world-space point into egui rect coordinates. Returns `None`
/// if the point is behind the camera or outside the rect.
pub(crate) fn project_world_to_rect(
    camera: &Camera,
    cam_gt: &GlobalTransform,
    world_pos: Vec3,
    rect: egui::Rect,
) -> Option<egui::Pos2> {
    let ndc = camera.world_to_ndc(cam_gt, world_pos)?;
    if !(0.0..=1.0).contains(&ndc.z) {
        return None;
    }
    let x = rect.min.x + (ndc.x + 1.0) * 0.5 * rect.width();
    let y = rect.min.y + (1.0 - ndc.y) * 0.5 * rect.height();
    let pos = egui::pos2(x, y);
    if !rect.contains(pos) {
        return None;
    }
    Some(pos)
}
