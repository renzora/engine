#![allow(dead_code)]

//! Skeleton gizmo — draws solid octahedral bone meshes for entities with
//! AnimatorComponent. Unlike the rest of the viewport gizmos (which are
//! line-based via `Gizmos<OverlayGizmoGroup>`), bones are rendered as real
//! `Mesh3d` entities using `GizmoMaterial` so they read as solid surfaces.
//!
//! Bones are re-spawned every frame in immediate-mode style: all existing
//! `BoneGizmoMesh` entities are despawned at the start of the system, then
//! one fresh mesh is spawned per bone. The mesh asset is shared; only the
//! per-entity Transform + material handle differ.
//!
//! Bone side (Left/Right/Center) is inferred from the bone's `Name` suffix
//! and used to pick a tinted material. Hover/selected bones override the
//! side tint with a distinct color.

use bevy::camera::visibility::RenderLayers;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::prelude::*;
use bevy::animation::AnimationTargetId;
use bevy::asset::RenderAssetUsages;

use renzora_editor::{EditorSelection, HideInHierarchy};

use crate::GizmoMaterial;

/// Resource tracking hovered/selected bone for gizmo interaction.
#[derive(Resource, Default)]
pub struct BoneSelection {
    pub selected_bone: Option<Entity>,
    pub hovered_bone: Option<Entity>,
}

/// Marker on spawned bone mesh entities so we can clear them each frame.
#[derive(Component)]
pub struct BoneGizmoMesh;

/// Shared mesh + tinted material handles. Built lazily on first use so we
/// don't need a separate startup system.
#[derive(Resource)]
pub struct BoneGizmoAssets {
    mesh: Handle<Mesh>,
    mat_left: Handle<GizmoMaterial>,
    mat_right: Handle<GizmoMaterial>,
    mat_center: Handle<GizmoMaterial>,
    mat_hovered: Handle<GizmoMaterial>,
    mat_selected: Handle<GizmoMaterial>,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum BoneSide {
    Left,
    Right,
    Center,
}

fn classify_bone_side(name: &str) -> BoneSide {
    // Match common rigging suffixes: .L / _L / .Left / _left / "Left" word,
    // same for R. Checked case-insensitively for the word forms.
    let n = name;
    if n.ends_with(".L") || n.ends_with("_L") || n.ends_with(".l") || n.ends_with("_l") {
        return BoneSide::Left;
    }
    if n.ends_with(".R") || n.ends_with("_R") || n.ends_with(".r") || n.ends_with("_r") {
        return BoneSide::Right;
    }
    let lower = n.to_ascii_lowercase();
    if lower.ends_with("left") || lower.ends_with(".left") || lower.ends_with("_left") {
        return BoneSide::Left;
    }
    if lower.ends_with("right") || lower.ends_with(".right") || lower.ends_with("_right") {
        return BoneSide::Right;
    }
    BoneSide::Center
}

/// Build a unit octahedral bone mesh: head at origin, tail at +Y=1, with a
/// waist ring at y=0.15 and radius=1 on the X/Z plane. Per-bone Transform
/// scales (x, y, z) to (radius, length, radius) so the mesh stretches
/// between head and tail correctly.
fn build_bone_mesh() -> Mesh {
    let head = [0.0, 0.0, 0.0];
    let tail = [0.0, 1.0, 0.0];
    let waist_y = 0.15;
    // 4 waist points — +X, +Z, -X, -Z
    let r = [
        [1.0, waist_y, 0.0],
        [0.0, waist_y, 1.0],
        [-1.0, waist_y, 0.0],
        [0.0, waist_y, -1.0],
    ];

    // Duplicate vertices per triangle so each face gets a flat normal.
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(24);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(24);
    let mut indices: Vec<u32> = Vec::with_capacity(24);

    let mut push_tri = |a: [f32; 3], b: [f32; 3], c: [f32; 3]| {
        let ab = Vec3::new(b[0] - a[0], b[1] - a[1], b[2] - a[2]);
        let ac = Vec3::new(c[0] - a[0], c[1] - a[1], c[2] - a[2]);
        let n = ab.cross(ac).normalize_or_zero().to_array();
        let base = positions.len() as u32;
        positions.push(a);
        positions.push(b);
        positions.push(c);
        normals.push(n);
        normals.push(n);
        normals.push(n);
        indices.push(base);
        indices.push(base + 1);
        indices.push(base + 2);
    };

    // 4 head-side faces (head → ring[i] → ring[i+1])
    for i in 0..4 {
        let a = head;
        let b = r[i];
        let c = r[(i + 1) % 4];
        push_tri(a, b, c);
    }
    // 4 tail-side faces (tail → ring[i+1] → ring[i]) — reversed for outward normal
    for i in 0..4 {
        let a = tail;
        let b = r[(i + 1) % 4];
        let c = r[i];
        push_tri(a, b, c);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn ensure_assets(
    commands: &mut Commands,
    assets: Option<Res<BoneGizmoAssets>>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<GizmoMaterial>,
) -> BoneGizmoAssets {
    if let Some(a) = assets {
        return BoneGizmoAssets {
            mesh: a.mesh.clone(),
            mat_left: a.mat_left.clone(),
            mat_right: a.mat_right.clone(),
            mat_center: a.mat_center.clone(),
            mat_hovered: a.mat_hovered.clone(),
            mat_selected: a.mat_selected.clone(),
        };
    }

    let mk = |m: &mut Assets<GizmoMaterial>, r: f32, g: f32, b: f32| {
        m.add(GizmoMaterial {
            base_color: LinearRgba::new(r, g, b, 1.0),
            emissive: LinearRgba::new(r, g, b, 1.0),
        })
    };

    // Values pushed hard so tonemapping doesn't wash them to grey. Matches
    // the axis-gizmo convention (one channel near 1.0, others near zero).
    let built = BoneGizmoAssets {
        mesh: meshes.add(build_bone_mesh()),
        mat_left: mk(materials, 1.0, 0.1, 0.2),
        mat_right: mk(materials, 0.1, 0.35, 1.0),
        mat_center: mk(materials, 1.0, 0.75, 0.1),
        mat_hovered: mk(materials, 1.0, 0.85, 0.2),
        mat_selected: mk(materials, 0.1, 1.0, 1.0),
    };

    let stored = BoneGizmoAssets {
        mesh: built.mesh.clone(),
        mat_left: built.mat_left.clone(),
        mat_right: built.mat_right.clone(),
        mat_center: built.mat_center.clone(),
        mat_hovered: built.mat_hovered.clone(),
        mat_selected: built.mat_selected.clone(),
    };
    commands.insert_resource(stored);
    built
}

/// Draw skeleton overlay for the selected entity as solid octahedral meshes.
pub fn draw_skeleton_gizmo(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GizmoMaterial>>,
    assets: Option<Res<BoneGizmoAssets>>,
    selection: Res<EditorSelection>,
    bone_selection: Res<BoneSelection>,
    global_transforms: Query<&GlobalTransform>,
    children_q: Query<&Children>,
    parent_q: Query<&ChildOf>,
    target_q: Query<(), With<AnimationTargetId>>,
    name_q: Query<&Name>,
    existing: Query<Entity, With<BoneGizmoMesh>>,
) {
    // Clear last frame's bone meshes first — immediate-mode.
    for e in &existing {
        commands.entity(e).despawn();
    }

    let Some(selected) = selection.get() else { return };

    let a = ensure_assets(&mut commands, assets, &mut meshes, &mut materials);

    let mut bones = Vec::new();
    collect_bones(selected, &children_q, &target_q, &mut bones);

    for &bone in &bones {
        let Ok(bone_gt) = global_transforms.get(bone) else { continue };
        let bone_pos = bone_gt.translation();

        // Pick material by hover/selection first, else side tint.
        let mat = if bone_selection.selected_bone == Some(bone) {
            a.mat_selected.clone()
        } else if bone_selection.hovered_bone == Some(bone) {
            a.mat_hovered.clone()
        } else {
            let side = name_q
                .get(bone)
                .ok()
                .map(|n| classify_bone_side(n.as_str()))
                .unwrap_or(BoneSide::Center);
            match side {
                BoneSide::Left => a.mat_left.clone(),
                BoneSide::Right => a.mat_right.clone(),
                BoneSide::Center => a.mat_center.clone(),
            }
        };

        let has_bone_child = children_q
            .get(bone)
            .ok()
            .map(|children| children.iter().any(|c| target_q.get(c).is_ok()))
            .unwrap_or(false);

        // Spawn a bone mesh between this joint and its parent joint.
        if let Ok(child_of) = parent_q.get(bone) {
            let parent = child_of.parent();
            if target_q.get(parent).is_ok() {
                if let Ok(parent_gt) = global_transforms.get(parent) {
                    spawn_bone_mesh(
                        &mut commands,
                        &a,
                        mat.clone(),
                        parent_gt.translation(),
                        bone_pos,
                    );
                }
            }
        }

        // Leaf joint marker so end-bones are still visible.
        if !has_bone_child {
            commands.spawn((
                Mesh3d(a.mesh.clone()),
                MeshMaterial3d(mat),
                Transform {
                    translation: bone_pos,
                    rotation: Quat::IDENTITY,
                    scale: Vec3::splat(0.015),
                },
                BoneGizmoMesh,
                HideInHierarchy,
                RenderLayers::layer(1),
            ));
        }
    }
}

fn spawn_bone_mesh(
    commands: &mut Commands,
    a: &BoneGizmoAssets,
    mat: Handle<GizmoMaterial>,
    head: Vec3,
    tail: Vec3,
) {
    let axis = tail - head;
    let length = axis.length();
    if length < 1e-5 {
        return;
    }
    let forward = axis / length;
    // `from_rotation_arc` panics when vectors point in opposite directions;
    // handle the 180° case manually by rotating around any perpendicular axis.
    let rotation = if forward.dot(Vec3::Y) < -0.9999 {
        Quat::from_rotation_x(std::f32::consts::PI)
    } else {
        Quat::from_rotation_arc(Vec3::Y, forward)
    };
    let radius = (length * 0.1).clamp(0.004, 0.03);

    commands.spawn((
        Mesh3d(a.mesh.clone()),
        MeshMaterial3d(mat),
        Transform {
            translation: head,
            rotation,
            scale: Vec3::new(radius, length, radius),
        },
        BoneGizmoMesh,
        HideInHierarchy,
        RenderLayers::layer(1),
    ));
}

fn collect_bones(
    entity: Entity,
    children_q: &Query<&Children>,
    target_q: &Query<(), With<AnimationTargetId>>,
    out: &mut Vec<Entity>,
) {
    if target_q.get(entity).is_ok() {
        out.push(entity);
    }
    if let Ok(children) = children_q.get(entity) {
        for child in children.iter() {
            collect_bones(child, children_q, target_q, out);
        }
    }
}
