//! The math and the selection rules, pinned down without a GPU. The ECS side
//! is covered by `tests/app.rs` in a headless app.

use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{CameraProjection, PerspectiveProjection};
use bevy::math::{Affine3A, Mat4, Quat, Vec3, Vec4};
use bevy::prelude::{Entity, GlobalTransform, Transform};

use crate::component::{NormalAxis, PlanarReflection};
use crate::math::{
    apply_oblique_near_plane, mirror_camera_transform, plane_in_view_space, ObliqueProjection,
    ReflectionPlane,
};
use crate::select::{
    distance_to_obb, evaluate, is_editor_layer, scene_layers, select_active, update_due,
    Candidate, Skip, SurfaceView,
};

const EPS: f32 = 1e-4;

fn approx(a: Vec3, b: Vec3) -> bool {
    a.distance(b) < EPS
}

fn projection() -> PerspectiveProjection {
    PerspectiveProjection {
        aspect_ratio: 16.0 / 9.0,
        ..Default::default()
    }
}

/// Clip-space position of a world point.
fn clip(clip_from_view: Mat4, camera: &GlobalTransform, point: Vec3) -> Vec4 {
    let view_from_world = Mat4::from(camera.affine().inverse());
    clip_from_view * view_from_world * point.extend(1.0)
}

/// Whether a clip-space point survives clipping (reverse Z: depth 0..1).
fn visible(c: Vec4) -> bool {
    c.w > 0.0
        && c.x.abs() <= c.w
        && c.y.abs() <= c.w
        && c.z >= 0.0
        && c.z <= c.w
}

fn floor() -> ReflectionPlane {
    ReflectionPlane::from_transform(&GlobalTransform::IDENTITY, NormalAxis::Y).unwrap()
}

fn viewer() -> GlobalTransform {
    GlobalTransform::from(Transform::from_xyz(0.0, 2.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y))
}

// --- the plane --------------------------------------------------------------

#[test]
fn plane_from_identity_is_the_local_axis_through_the_origin() {
    for (axis, normal) in [
        (NormalAxis::X, Vec3::X),
        (NormalAxis::Y, Vec3::Y),
        (NormalAxis::Z, Vec3::Z),
    ] {
        let plane = ReflectionPlane::from_transform(&GlobalTransform::IDENTITY, axis).unwrap();
        assert!(approx(plane.normal, normal), "{axis:?}: {:?}", plane.normal);
        assert!(approx(plane.point, Vec3::ZERO));
    }
}

#[test]
fn plane_follows_rotation_and_translation() {
    // The engine's Plane primitive stood upright, as the Mirror preset does:
    // local Y turns to face world +Z.
    let t = GlobalTransform::from(
        Transform::from_xyz(1.0, 2.0, 3.0)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
    );
    let plane = ReflectionPlane::from_transform(&t, NormalAxis::Y).unwrap();
    assert!(approx(plane.normal, Vec3::Z), "{:?}", plane.normal);
    assert!(approx(plane.point, Vec3::new(1.0, 2.0, 3.0)));
}

#[test]
fn plane_normal_survives_non_uniform_scale() {
    // A plane tilted 45 degrees about Z, then squashed along local X. The
    // rotated axis would be the wrong normal; the inverse transpose is not.
    let t = GlobalTransform::from(
        Transform::from_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4))
            .with_scale(Vec3::new(4.0, 1.0, 1.0)),
    );
    let plane = ReflectionPlane::from_transform(&t, NormalAxis::Y).unwrap();
    // Two points in the mesh's plane (local y = 0) must lie on the plane.
    for local in [Vec3::new(1.0, 0.0, 0.0), Vec3::new(-0.5, 0.0, 2.0)] {
        let world = t.transform_point(local);
        assert!(plane.signed_distance(world).abs() < EPS, "{local:?} -> {world:?}");
    }
    assert!((plane.normal.length() - 1.0).abs() < EPS);
}

#[test]
fn degenerate_transform_has_no_plane() {
    let t = GlobalTransform::from(Transform::from_scale(Vec3::new(0.0, 1.0, 1.0)));
    assert!(ReflectionPlane::from_transform(&t, NormalAxis::Y).is_none());
}

#[test]
fn plane_vec4_is_positive_on_the_reflecting_side() {
    let plane = ReflectionPlane {
        normal: Vec3::Y,
        point: Vec3::new(0.0, 1.0, 0.0),
    };
    let v = plane.to_vec4();
    assert!(v.dot(Vec3::new(0.0, 3.0, 0.0).extend(1.0)) > 0.0);
    assert!(v.dot(Vec3::new(0.0, -3.0, 0.0).extend(1.0)) < 0.0);
    assert!((plane.offset(0.5).signed_distance(Vec3::new(0.0, 1.5, 0.0))).abs() < EPS);
}

// --- reflecting a transform -------------------------------------------------

#[test]
fn reflecting_twice_is_the_identity() {
    let plane = ReflectionPlane {
        normal: Vec3::new(1.0, 2.0, -0.5).normalize(),
        point: Vec3::new(0.3, -1.0, 2.0),
    };
    let p = Vec3::new(4.0, -2.0, 7.0);
    assert!(approx(plane.reflect_point(plane.reflect_point(p)), p));
    let v = Vec3::new(-1.0, 0.5, 2.0);
    assert!(approx(plane.reflect_vector(plane.reflect_vector(v)), v));
}

#[test]
fn mirror_camera_sits_below_the_floor_looking_up() {
    let mirror = mirror_camera_transform(&viewer(), &floor());
    assert!(approx(mirror.translation, Vec3::new(0.0, -2.0, 6.0)));
    // The viewer looks down at the origin; its reflection looks up at it.
    let forward = *mirror.forward();
    let expected = (Vec3::ZERO - mirror.translation).normalize();
    assert!(approx(forward, expected), "{forward:?}");
    // Proper rotation: a Transform cannot hold a reflection.
    assert!(mirror.rotation.is_normalized());
}

#[test]
fn mirror_image_is_the_reflection_flipped_horizontally() {
    // A point the viewer sees in the mirror lands at the same height and the
    // opposite x in the mirror camera's image, which is why the overlay
    // samples at 1 - u.
    let plane = ReflectionPlane {
        normal: Vec3::Z,
        point: Vec3::ZERO,
    };
    let viewer = GlobalTransform::from(
        Transform::from_xyz(1.0, 1.5, 4.0).looking_at(Vec3::new(-0.5, 1.0, 0.0), Vec3::Y),
    );
    let mirror = GlobalTransform::from(mirror_camera_transform(&viewer, &plane));
    let m = projection().get_clip_from_view();
    for real in [Vec3::new(0.3, 1.2, 1.5), Vec3::new(-0.8, 0.6, 2.5)] {
        // The viewer sees `real`'s reflection behind the mirror...
        let seen = clip(m, &viewer, plane.reflect_point(real));
        // ...and the mirror camera sees `real` itself.
        let captured = clip(m, &mirror, real);
        let (seen, captured) = (seen / seen.w, captured / captured.w);
        assert!((seen.x + captured.x).abs() < 1e-3, "{seen:?} vs {captured:?}");
        assert!((seen.y - captured.y).abs() < 1e-3, "{seen:?} vs {captured:?}");
    }
}

// --- the oblique projection -------------------------------------------------

/// The floor mirror's capture camera and its oblique projection.
fn floor_capture() -> (GlobalTransform, ObliqueProjection, PerspectiveProjection) {
    let plane = floor();
    let mirror = GlobalTransform::from(mirror_camera_transform(&viewer(), &plane));
    let clip_plane = plane_in_view_space(plane.offset(0.01).to_vec4(), &mirror.affine());
    (mirror, ObliqueProjection::new(projection(), Some(clip_plane)), projection())
}

#[test]
fn view_space_plane_keeps_the_mirror_camera_behind_it() {
    let (mirror, oblique, _) = floor_capture();
    let plane = oblique.clip_plane.expect("camera is behind the plane");
    // The camera is the view-space origin, so `w` is its signed distance.
    assert!(plane.w < 0.0, "{plane:?}");
    // And a world point on the reflecting side is positive in view space.
    let above = mirror.affine().inverse().transform_point3(Vec3::new(0.0, 0.5, 0.0));
    assert!(plane.dot(above.extend(1.0)) > 0.0);
}

#[test]
fn oblique_projection_keeps_the_reflected_side_and_clips_the_other() {
    let (mirror, oblique, plain) = floor_capture();
    let m = oblique.get_clip_from_view();
    let unclipped = plain.get_clip_from_view();

    // In front of the mirror: drawn into the reflection.
    for p in [Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.4, 1.5, -1.0), Vec3::new(0.0, 0.05, 1.0)] {
        assert!(visible(clip(m, &mirror, p)), "{p:?} should be visible");
    }
    // Behind the mirror (under the floor): the plain projection would draw
    // it, the oblique one must not.
    for p in [Vec3::new(0.0, -0.5, 0.0), Vec3::new(0.3, -0.2, -1.0)] {
        assert!(visible(clip(unclipped, &mirror, p)), "{p:?} is in the plain frustum");
        assert!(!visible(clip(m, &mirror, p)), "{p:?} should be clipped");
    }
    // Inside the clip offset counts as behind: the surface never reflects
    // itself.
    assert!(!visible(clip(m, &mirror, Vec3::new(0.0, 0.0, 0.0))));
}

#[test]
fn oblique_frustum_culls_whole_objects_behind_the_mirror() {
    let (mirror, oblique, _) = floor_capture();
    let frustum = oblique.compute_frustum(&mirror);
    let unit = Aabb::from_min_max(Vec3::splat(-0.2), Vec3::splat(0.2));
    let at = |p: Vec3| Affine3A::from_translation(p);
    assert!(frustum.intersects_obb(&unit, &at(Vec3::new(0.0, 1.0, 0.0)), true, true));
    assert!(!frustum.intersects_obb(&unit, &at(Vec3::new(0.0, -1.0, 0.0)), true, true));
}

#[test]
fn oblique_only_rewrites_the_depth_row() {
    let mut m = projection().get_clip_from_view();
    let before = m;
    apply_oblique_near_plane(&mut m, Vec4::new(0.0, 0.6, -0.8, -2.0));
    assert_eq!(m.row(0), before.row(0));
    assert_eq!(m.row(1), before.row(1));
    assert_eq!(m.row(3), before.row(3));
    assert_ne!(m.row(2), before.row(2));
}

#[test]
fn plane_through_the_camera_falls_back_to_plain_perspective() {
    let oblique = ObliqueProjection::new(projection(), Some(Vec4::new(0.0, 1.0, 0.0, 0.0)));
    assert!(oblique.clip_plane.is_none());
    assert_eq!(oblique.get_clip_from_view(), projection().get_clip_from_view());
}

// --- selection ----------------------------------------------------------------

fn surface(plane: ReflectionPlane, world_from_local: &Affine3A) -> SurfaceView<'_> {
    SurfaceView {
        plane,
        world_from_local,
        aabb: None,
        max_distance: None,
    }
}

#[test]
fn surfaces_facing_away_are_skipped() {
    let flat = Aabb::from_min_max(Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 0.0, 1.0));
    let identity = Affine3A::IDENTITY;
    let mut view = surface(floor(), &identity);
    view.aabb = Some(&flat);
    let frustum = projection().compute_frustum(&GlobalTransform::from(
        Transform::from_xyz(0.0, -2.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    assert_eq!(
        evaluate(&view, Vec3::new(0.0, -2.0, 6.0), &frustum),
        Err(Skip::FacingAway)
    );
    // Level with the plane is as good as behind it.
    assert_eq!(
        evaluate(&view, Vec3::new(0.0, 0.0, 6.0), &frustum),
        Err(Skip::FacingAway)
    );
}

#[test]
fn surfaces_off_screen_or_far_are_skipped_and_visible_ones_measured() {
    let flat = Aabb::from_min_max(Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 0.0, 1.0));
    let identity = Affine3A::IDENTITY;
    let camera = viewer();
    let frustum = projection().compute_frustum(&camera);
    let pos = camera.translation();

    let mut view = surface(floor(), &identity);
    // No mesh bounds yet: nothing to draw.
    assert_eq!(evaluate(&view, pos, &frustum), Err(Skip::OffScreen));

    view.aabb = Some(&flat);
    let d = evaluate(&view, pos, &frustum).expect("the floor is in view");
    // Nearest point of the 2x2 floor to (0, 2, 6) is (0, 0, 1).
    assert!((d - Vec3::new(0.0, 2.0, 5.0).length()).abs() < 1e-3, "{d}");

    view.max_distance = Some(3.0);
    assert_eq!(evaluate(&view, pos, &frustum), Err(Skip::TooFar));

    // Same floor, moved behind the camera.
    let behind = Affine3A::from_translation(Vec3::new(0.0, 0.0, 20.0));
    let mut moved = surface(
        ReflectionPlane {
            normal: Vec3::Y,
            point: Vec3::new(0.0, 0.0, 20.0),
        },
        &behind,
    );
    moved.aabb = Some(&flat);
    assert_eq!(evaluate(&moved, pos, &frustum), Err(Skip::OffScreen));
}

#[test]
fn distance_is_to_the_box_not_its_centre() {
    let big = Aabb::from_min_max(Vec3::new(-50.0, 0.0, -50.0), Vec3::new(50.0, 0.0, 50.0));
    let d = distance_to_obb(Vec3::new(10.0, 1.7, 10.0), &big, &Affine3A::IDENTITY);
    assert!((d - 1.7).abs() < EPS, "{d}");
    // And under scale: a unit box stretched 10x along X.
    let unit = Aabb::from_min_max(Vec3::splat(-0.5), Vec3::splat(0.5));
    let stretched = Affine3A::from_scale(Vec3::new(10.0, 1.0, 1.0));
    let d = distance_to_obb(Vec3::new(7.0, 0.0, 0.0), &unit, &stretched);
    assert!((d - 2.0).abs() < EPS, "{d}");
}

fn candidate(surface: u32, camera: u32, distance: f32) -> Candidate {
    Candidate {
        surface: Entity::from_raw_u32(surface).unwrap(),
        camera: Entity::from_raw_u32(camera).unwrap(),
        distance,
    }
}

#[test]
fn the_cap_keeps_the_nearest() {
    let picked = select_active(
        vec![
            candidate(1, 100, 9.0),
            candidate(2, 100, 1.0),
            candidate(3, 100, 5.0),
            candidate(4, 100, 3.0),
            candidate(5, 100, 7.0),
        ],
        3,
    );
    let order: Vec<f32> = picked.iter().map(|c| c.distance).collect();
    assert_eq!(order, vec![1.0, 3.0, 5.0]);
    assert!(select_active(vec![candidate(1, 100, 1.0)], 0).is_empty());
}

#[test]
fn ties_break_the_same_way_every_frame() {
    let a = vec![candidate(7, 100, 2.0), candidate(3, 100, 2.0), candidate(5, 101, 2.0)];
    let mut b = a.clone();
    b.reverse();
    assert_eq!(select_active(a, 2), select_active(b, 2));
}

#[test]
fn update_every_spreads_renders_across_frames() {
    assert!((0..10).all(|f| update_due(f, 1, 3)));
    let due: Vec<u64> = (0..6).filter(|&f| update_due(f, 3, 0)).collect();
    assert_eq!(due, vec![0, 3]);
    // Different staggers land on different frames.
    let a: Vec<u64> = (0..4).filter(|&f| update_due(f, 2, 0)).collect();
    let b: Vec<u64> = (0..4).filter(|&f| update_due(f, 2, 1)).collect();
    assert_eq!(a, vec![0, 2]);
    assert_eq!(b, vec![1, 3]);
    // Zero means every frame rather than a division by zero.
    assert!(update_due(5, 0, 0));
}

// --- layers and settings ------------------------------------------------------

#[test]
fn mirror_cameras_drop_editor_layers() {
    for editor in [1, 20, 23, 24, 27] {
        assert!(is_editor_layer(editor), "{editor}");
    }
    for scene in [0, 2, 19, 28, 31] {
        assert!(!is_editor_layer(scene), "{scene}");
    }
    let editor_camera = RenderLayers::from_layers(&[0, 1, 25]);
    assert_eq!(scene_layers(Some(&editor_camera)), RenderLayers::layer(0));
    let game_camera = RenderLayers::from_layers(&[0, 3]);
    assert_eq!(scene_layers(Some(&game_camera)), game_camera);
    assert_eq!(scene_layers(None), RenderLayers::layer(0));
}

#[test]
fn settings_are_clamped_where_they_are_read() {
    let wild = PlanarReflection {
        resolution_scale: 4.0,
        strength: -1.0,
        fresnel: f32::NAN,
        blur: 2.0,
        clip_offset: -1.0,
        max_distance: 0.0,
        update_every: 0,
        ..Default::default()
    };
    assert_eq!(wild.resolution_scale(), 1.0);
    assert_eq!(wild.strength(), 0.0);
    assert_eq!(wild.fresnel(), 0.0);
    assert_eq!(wild.blur(), 1.0);
    assert_eq!(wild.clip_offset(), 0.0);
    assert_eq!(wild.max_distance(), None);
    assert_eq!(wild.update_every(), 1);
    let mirror = PlanarReflection::mirror(NormalAxis::Z);
    assert_eq!((mirror.strength, mirror.fresnel), (1.0, 0.0));
}

/// The Bistro case: a window whose vertices sit metres from its entity's
/// origin. Fitted, the plane goes through the mesh, not the origin.
#[test]
fn fitted_plane_goes_through_the_mesh_not_the_origin() {
    let t = GlobalTransform::from(Transform::from_xyz(0.1, 0.0, 1.2));
    let aabb = Aabb::from_min_max(Vec3::new(4.0, 0.5, 2.0), Vec3::new(4.02, 2.5, 3.0));
    let fitted = PlanarReflection { normal_axis: NormalAxis::X, ..Default::default() };
    let plane = ReflectionPlane::for_surface(&t, &fitted, Some(&aabb), None).unwrap();
    assert!((plane.point.x - 4.11).abs() < 1e-4, "plane at x = {}", plane.point.x);
    let unfitted = PlanarReflection { fit_to_mesh: false, ..fitted.clone() };
    let origin = ReflectionPlane::for_surface(&t, &unfitted, Some(&aabb), None).unwrap();
    assert!((origin.point.x - 0.1).abs() < 1e-4);
}

#[test]
fn flip_turns_the_normal_and_offset_moves_along_it() {
    let t = GlobalTransform::IDENTITY;
    let r = PlanarReflection { flip: true, plane_offset: 0.5, ..Default::default() };
    let plane = ReflectionPlane::for_surface(&t, &r, None, None).unwrap();
    assert_eq!(plane.normal, Vec3::NEG_Y);
    // The offset follows the (flipped) normal.
    assert!((plane.point - Vec3::new(0.0, -0.5, 0.0)).length() < 1e-5);
}

#[test]
fn fit_without_bounds_falls_back_to_the_origin() {
    let t = GlobalTransform::from(Transform::from_xyz(1.0, 2.0, 3.0));
    let plane = ReflectionPlane::for_surface(&t, &PlanarReflection::default(), None, None).unwrap();
    assert!((plane.point - Vec3::new(1.0, 2.0, 3.0)).length() < 1e-5);
}

/// Two triangles of a quad in the plane `z = z`, spanning x0..x1 and y0..y1,
/// wound to face +Z (or -Z when `back`).
fn quad_z(z: f32, x0: f32, x1: f32, y0: f32, y1: f32, back: bool) -> Vec<[f32; 3]> {
    let (a, b, c, d) = ([x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]);
    if back {
        vec![a, c, b, a, d, c]
    } else {
        vec![a, b, c, a, c, d]
    }
}

fn mesh_of(positions: Vec<[f32; 3]>) -> bevy::mesh::Mesh {
    bevy::mesh::Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default())
        .with_inserted_attribute(bevy::mesh::Mesh::ATTRIBUTE_POSITION, positions)
}

/// One merged mesh with a long wall of glass and a short one, plus a side
/// wall facing X. Along Z the fit lands on the long wall, not between the two
/// as the bounds' middle does, and says the mesh is not one flat surface.
#[test]
fn fit_picks_the_biggest_sheet_of_a_merged_facade() {
    let mut p = quad_z(4.3, 0.8, 8.5, 0.5, 2.8, false);
    p.extend(quad_z(1.3, 0.8, 2.0, 0.5, 2.8, false));
    // A wall facing X, which must not count toward Z.
    p.extend([[0.8, 0.5, 1.3], [0.8, 0.5, 4.3], [0.8, 2.8, 4.3]]);
    let fit = crate::fit::fit_mesh(&mesh_of(p), NormalAxis::Z).unwrap();
    assert!((fit.point.z - 4.3).abs() < 1e-4, "fitted z = {}", fit.point.z);
    assert!((fit.normal - Vec3::Z).length() < 1e-4);
    assert!(!fit.flat);
}

/// The real Bistro glass: three panes on one wall turned 21.5 degrees off Z,
/// a couple of centimetres apart in depth. The fit takes the wall's own
/// normal, not Z, and calls it flat although its bounds are 3 m deep.
#[test]
fn fit_finds_a_wall_turned_off_the_axis() {
    let wall = Vec3::new(-0.3672, 0.0, 0.9301).normalize();
    let along = Vec3::Y.cross(wall);
    let mut p = Vec::new();
    for (i, depth) in [0.901_f32, 0.892, 0.8805].into_iter().enumerate() {
        let o = wall * depth + along * (i as f32 * 2.8);
        let (a, b, c, d) = (o, o + along * 2.3, o + along * 2.3 + Vec3::Y * 1.15, o + Vec3::Y * 1.15);
        p.extend([a, b, c, a, c, d].map(|v| v.to_array()));
    }
    let fit = crate::fit::fit_mesh(&mesh_of(p), NormalAxis::Z).unwrap();
    assert!(fit.normal.dot(wall) > 0.9999, "normal {:?}", fit.normal);
    assert!((fit.point.dot(wall) - 0.891).abs() < 0.01);
    assert!(fit.flat);
}

#[test]
fn fit_follows_which_way_the_glass_faces() {
    let back = mesh_of(quad_z(2.0, 0.0, 1.0, 0.0, 1.0, true));
    assert_eq!(crate::fit::fit_mesh(&back, NormalAxis::Z).unwrap().normal, Vec3::NEG_Z);
    // Double sided: the two faces cancel, so the chosen axis stands.
    let mut both = quad_z(2.0, 0.0, 1.0, 0.0, 1.0, false);
    both.extend(quad_z(2.0, 0.0, 1.0, 0.0, 1.0, true));
    assert_eq!(crate::fit::fit_mesh(&mesh_of(both), NormalAxis::Z).unwrap().normal, Vec3::Z);
    // Nothing faces Y.
    assert!(crate::fit::fit_mesh(&back, NormalAxis::Y).is_none());
}

#[test]
fn a_found_fit_overrides_the_bounds_and_turns_the_normal() {
    let t = GlobalTransform::from(Transform::from_xyz(1.0, 0.0, 0.0));
    let aabb = Aabb::from_min_max(Vec3::new(0.0, 0.0, 1.0), Vec3::new(8.0, 2.0, 4.0));
    let r = PlanarReflection { normal_axis: NormalAxis::Z, ..Default::default() };
    let fit = crate::fit::MeshPlaneFit {
        mesh: bevy::asset::AssetId::default(),
        axis: NormalAxis::Z,
        local_point: Vec3::new(4.0, 1.0, 4.0),
        local_normal: Vec3::NEG_Z,
        flat: true,
        found: true,
    };
    let plane = ReflectionPlane::for_surface(&t, &r, Some(&aabb), Some(&fit)).unwrap();
    assert!((plane.point.z - 4.0).abs() < 1e-5);
    assert!((plane.normal - Vec3::NEG_Z).length() < 1e-6);
    // A fit for another axis, or one that found nothing, is ignored.
    let stale = crate::fit::MeshPlaneFit { axis: NormalAxis::X, ..fit };
    assert!((ReflectionPlane::for_surface(&t, &r, Some(&aabb), Some(&stale)).unwrap().point.z - 2.5).abs() < 1e-5);
    let empty = crate::fit::MeshPlaneFit { found: false, ..fit };
    assert_eq!(ReflectionPlane::for_surface(&t, &r, Some(&aabb), Some(&empty)).unwrap().normal, Vec3::Z);
}

#[test]
fn normal_axis_labels_round_trip() {
    for axis in [NormalAxis::X, NormalAxis::Y, NormalAxis::Z] {
        assert_eq!(NormalAxis::from_label(axis.label()), axis);
    }
}
