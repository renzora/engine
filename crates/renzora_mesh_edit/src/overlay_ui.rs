//! Screen-space overlay for Edit-mode selection feedback.
//!
//! Vertex dots and the marquee rubber-band need true screen-pixel positioning,
//! so they can't use [`bevy_gizmos::Gizmos`] (whose `rect_2d` / `line_2d` go
//! through the 3D gizmo pipeline at world-space `(x, y, 0)` — confirmed in
//! `bevy_gizmos-0.19.1/src/gizmos.rs:804`, where `lineloop_2d` is literally
//! `lineloop(positions.map(|vec2| vec2.extend(0.)), color)`). With a
//! perspective viewport those "2-D" gizmos end up far off-scene and don't
//! appear where the user expects them.
//!
//! This module owns a persistent tree of UI Nodes that live under the
//! viewport's panel (sibling of the `ViewportImage`) and renders:
//!
//! * one absolute-positioned Node per vertex (filled square), colour-coded
//!   selected vs. unselected,
//! * one absolute-positioned border Node for the marquee rubber-band.
//!
//! Both are hidden when their mode (vertex / drag) isn't active, and the
//! pool of vertex dots is grown/shrunk to match the active `EditMesh`'s
//! vertex count each frame.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::ui::ZIndex;
use renzora::core::viewport_types::{ViewportMode, ViewportSettings, ViewportState};
use renzora::core::EditorCamera;
use renzora_viewport::ViewportImage;

use crate::edit_mesh::{EditMesh, VertexId};
use crate::selection::{MeshSelection, SelectMode};
use crate::systems::{viewport_cursor, MeshEditBoxSelect, MeshEditBoxSelectState};

/// Z-index layered just below the editor's modal HUD (`GlobalZIndex(9000)`)
/// and above the viewport's rulers / selection boxes (`Z_SELECTION = 90`).
/// High enough that vertex dots and the marquee never hide behind the
/// 3D-rendered scene or the panel chrome.
const OVERLAY_Z: i32 = 7000;

/// Marker for the overlay container — a UI Node parented under the viewport
/// panel, hosting the per-vertex dots and the marquee rect.
#[derive(Component)]
pub struct EditOverlayRoot;

/// Marker for one of the per-vertex UI Nodes.
#[derive(Component)]
pub struct EditVertexDot;

/// Marker for the marquee border Node.
#[derive(Component)]
pub struct EditMarqueeRect;

/// Owned by the mesh-edit plugin: holds the entities for the screen-space
/// overlay so systems can grow/shrink the pool without re-querying.
#[derive(Resource, Default)]
pub struct EditOverlayEntities {
    pub root: Option<Entity>,
    pub vertex_dots: Vec<Entity>,
    pub marquee: Option<Entity>,
    /// Translucent 3D mesh overlays parented to the edit target, one per
    /// currently-selected face. We spawn/despawn these every frame the
    /// face-selection set changes — typical meshes have a handful of
    /// selected faces, so the spawn cost is negligible.
    pub face_overlays: Vec<Entity>,
    /// Cached translucent material handle so we don't re-allocate a
    /// `StandardMaterial` asset every face-overlay spawn.
    pub face_overlay_material: Option<Handle<StandardMaterial>>,
    /// The mesh target these overlays were spawned for. Detects when the
    /// user switched to a different edit target so we can despawn the
    /// stale set.
    pub face_overlay_target: Option<Entity>,
}

/// The overlay container matches `ViewportImage`'s sizing so its `overflow:
/// clip` doesn't crop the absolutely-positioned children. With `width: 0,
/// height: 0` the clip area is zero and every child gets rendered into
/// nothing — which is exactly what was happening on the first build of
/// this commit (vertex dots and marquee invisible even though the box-
/// select state machine still worked, because the picker is a separate
/// system that doesn't touch the overlay tree).
fn overlay_root_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Percent(0.0),
        // Fill the panel so the clip area matches the viewport's screen
        // rect. Child vertex dots / marquee are positioned in panel pixel
        // coords (mapped via `viewport_to_screen`) and clip cleanly to
        // the panel bounds — nothing bleeds into adjacent dock leaves.
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        overflow: Overflow::clip(),
        ..default()
    }
}

/// Convert a viewport-render-target pixel position (`world_to_viewport`'s
/// output space) into a position relative to the overlay's parent Node,
/// suitable for `Val::Px` on a UI child.
///
/// `world_to_viewport` returns viewport pixels in the rendered image's
/// top-left origin (Y-down). To position a UI child, we need its offset
/// from the parent's top-left, NOT from the window's top-left — Bevy UI's
/// `top: Px(N)` and `left: Px(N)` measure from the parent, not the
/// window. Without the `- screen_position` adjustment, every child lands
/// `screen_position.y` pixels below its target (which is exactly the
/// "dots below the cube" / "marquee below the cursor" bug we shipped
/// before this fix).
///
/// Returns `None` if the viewport hasn't reported its rect yet (during
/// the first frame after a panel swap).
fn viewport_to_screen(vp_px: Vec2, viewport: &ViewportState) -> Option<Vec2> {
    if viewport.screen_size.x <= 0.0 || viewport.screen_size.y <= 0.0 {
        return None;
    }
    if viewport.current_size.x == 0 || viewport.current_size.y == 0 {
        return None;
    }
    let nx = vp_px.x / viewport.current_size.x as f32;
    let ny = vp_px.y / viewport.current_size.y as f32;
    Some(Vec2::new(
        nx * viewport.screen_size.x,
        ny * viewport.screen_size.y,
    ))
}

/// Build / refresh the overlay container, parented as a sibling of
/// [`ViewportImage`] under the same panel content Node. Cheap on the steady
/// state: only spawns when the root is missing or its entity has been
/// despawned.
pub fn ensure_overlay_root(
    mut commands: Commands,
    mut state: ResMut<EditOverlayEntities>,
    viewport_images: Query<&ChildOf, With<ViewportImage>>,
) {
    // The editor ships with four viewport slots
    // (`PANEL_IDS = ["viewport", "viewport-2", "viewport-3", "viewport-4"]`
    // in `renzora_viewport::native_viewport`), so there can be up to four
    // `ViewportImage` entities. The editor is currently testing with one
    // viewport, but on a multi-viewport setup `single()` would return
    // `Err` and the overlay root would never spawn.
    //
    // We attach the overlay to the FIRST viewport's content Node — the
    // other slots get nothing. When the user splits viewports, the
    // selected mesh shows up in only one of them, so picking is still
    // unambiguous. (See Commit C / D for the multi-viewport polish.)
    let Some(content) = viewport_images
        .iter()
        .next()
        .map(|c| c.parent())
    else {
        return;
    };

    // We own the root's lifecycle: spawned in this system, dropped only
    // when the resource is reset. No need to validate against the world
    // each frame.
    if state.root.is_some() {
        return;
    }

    let root = commands
        .spawn((
            overlay_root_node(),
            // `ZIndex` covers the local stacking inside this Node's
            // children.
            ZIndex(OVERLAY_Z),
            EditOverlayRoot,
            Name::new("mesh-edit-overlay-root"),
        ))
        .id();
    commands.entity(content).add_child(root);
    state.root = Some(root);
}

/// Update each vertex dot's UI Node to reflect the projected screen
/// position and selection state of its corresponding vertex. Dots are
/// shown only in `SelectMode::Vertex`; in edge/face modes they're hidden
/// via `Display::None`.
#[allow(clippy::too_many_arguments)]
pub fn update_vertex_dots(
    mut commands: Commands,
    mut overlay: ResMut<EditOverlayEntities>,
    mesh_selection: Res<MeshSelection>,
    edit_q: Query<(&EditMesh, &GlobalTransform)>,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    viewport: Option<Res<ViewportState>>,
    viewport_settings: Res<ViewportSettings>,
    mut dot_q: Query<
        (Entity, &mut Node, &mut BackgroundColor, &mut Visibility),
        With<EditVertexDot>,
    >,
) {
    let Some(root) = overlay.root else {
        return;
    };
    let Some(viewport) = viewport.as_ref() else {
        return;
    };

    // No edit target → hide every dot but leave the pool intact.
    let Some(target) = mesh_selection.target else {
        for (_, _, _, mut visibility) in &mut dot_q {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let Ok((edit, gt)) = edit_q.get(target) else {
        return;
    };
    let Ok((camera, cam_gt)) = camera_q.single() else {
        return;
    };

    let vertex_mode = mesh_selection.mode == SelectMode::Vertex;
    let px_unselected = f32::from(viewport_settings.mesh_edit_vert_size);
    let px_selected = f32::from(viewport_settings.mesh_edit_vert_size_selected);

    // Grow the pool to match the vertex count. Extra entities (from a
    // previous, larger mesh) are despawned to keep the bookkeeping simple.
    let needed = edit.vertices.len();
    if overlay.vertex_dots.len() < needed {
        for _ in overlay.vertex_dots.len()..needed {
            let dot = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Px(0.0),
                        height: Val::Px(0.0),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    Visibility::Hidden,
                    EditVertexDot,
                    ChildOf(root),
                    Name::new("mesh-edit-vertex-dot"),
                ))
                .id();
            overlay.vertex_dots.push(dot);
        }
    } else if overlay.vertex_dots.len() > needed {
        let extra: Vec<Entity> = overlay.vertex_dots[needed..].to_vec();
        for e in extra {
            commands.entity(e).despawn();
        }
        overlay.vertex_dots.truncate(needed);
    }

    if !vertex_mode {
        for (_, _, _, mut visibility) in &mut dot_q {
            *visibility = Visibility::Hidden;
        }
        return;
    }

    // Walk the active vertices and update each dot. We trust the resource
    // list to stay in sync with the live entities: spawn above appends in
    // order, despawn trims the tail.
    for (i, v) in edit.vertices.iter().enumerate() {
        let Some(&dot_entity) = overlay.vertex_dots.get(i) else {
            continue;
        };

        let selected = mesh_selection.verts.contains(&VertexId(i as u32));
        let px_size = if selected { px_selected } else { px_unselected };
        if px_size <= 0.0 {
            if let Ok((_, _, _, mut visibility)) = dot_q.get_mut(dot_entity) {
                *visibility = Visibility::Hidden;
            }
            continue;
        }

        let world_pos = gt.transform_point(v.position);
        let Ok(vp_px) = camera.world_to_viewport(cam_gt, world_pos) else {
            if let Ok((_, _, _, mut visibility)) = dot_q.get_mut(dot_entity) {
                *visibility = Visibility::Hidden;
            }
            continue;
        };
        let Some(screen_px) = viewport_to_screen(vp_px, viewport) else {
            if let Ok((_, _, _, mut visibility)) = dot_q.get_mut(dot_entity) {
                *visibility = Visibility::Hidden;
            }
            continue;
        };

        let half = px_size * 0.5;
        if let Ok((_, mut node, mut bg, mut vis)) = dot_q.get_mut(dot_entity) {
            node.left = Val::Px(screen_px.x - half);
            node.top = Val::Px(screen_px.y - half);
            // `mesh_edit_vert_size` is the full pixel side length of the
            // dot — an N-value setting produces an N×N square, matching
            // the user's request and Blender's `vertex_size` slider.
            node.width = Val::Px(px_size);
            node.height = Val::Px(px_size);
            let color = if selected {
                Color::srgb(1.0, 0.55, 0.1)
            } else {
                Color::srgb(0.15, 0.55, 1.0)
            };
            *bg = BackgroundColor(color);
            *vis = Visibility::Visible;
        }
    }
}

/// Mirror of `render_box_selection` from `renzora_gizmo`, but for the
/// Edit-mode marquee. Owned here because the box-select state machine for
/// mesh edit lives in `systems::MeshEditBoxSelect`, while the Scene-mode
/// drag lives in `renzora_gizmo::BoxSelectionState` — two different state
/// machines, two different overlay renderers.
pub fn update_marquee(
    mut commands: Commands,
    mut overlay: ResMut<EditOverlayEntities>,
    box_select: Res<MeshEditBoxSelect>,
    viewport: Option<Res<ViewportState>>,
    window_q: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut marquee_q: Query<(&mut Node, &mut Visibility), With<EditMarqueeRect>>,
) {
    let Some(root) = overlay.root else {
        return;
    };
    let Some(cursor_vp) = viewport_cursor(&viewport, &window_q) else {
        return;
    };
    let Some(viewport) = viewport.as_ref() else {
        return;
    };

    match box_select.state {
        MeshEditBoxSelectState::Idle | MeshEditBoxSelectState::Pressing { .. } => {
            // No active drag — hide the marquee (if any). A hidden Node
            // is free.
            for (_, mut v) in &mut marquee_q {
                *v = Visibility::Hidden;
            }
        }
        MeshEditBoxSelectState::Marqueeing { anchor_vp, .. } => {
            let Some(anchor_screen) = viewport_to_screen(anchor_vp, viewport) else {
                return;
            };
            let Some(cursor_screen) = viewport_to_screen(cursor_vp, viewport) else {
                return;
            };
            let min = Vec2::new(
                anchor_screen.x.min(cursor_screen.x),
                anchor_screen.y.min(cursor_screen.y),
            );
            let max = Vec2::new(
                anchor_screen.x.max(cursor_screen.x),
                anchor_screen.y.max(cursor_screen.y),
            );
            let w = (max.x - min.x).max(0.0);
            let h = (max.y - min.y).max(0.0);

            // Lazy-spawn the marquee Node the first time it becomes
            // visible. It stays alive across drags so subsequent drags
            // reuse the same entity.
            let marquee = match overlay.marquee {
                Some(e) => e,
                None => {
                    let e = commands
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(0.0),
                                top: Val::Px(0.0),
                                width: Val::Px(0.0),
                                height: Val::Px(0.0),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(
                                66.0 / 255.0,
                                150.0 / 255.0,
                                250.0,
                                0.157,
                            )),
                            BorderColor::all(Color::srgb_u8(66, 150, 250)),
                            Visibility::Hidden,
                            EditMarqueeRect,
                            ChildOf(root),
                            Name::new("mesh-edit-marquee-rect"),
                        ))
                        .id();
                    overlay.marquee = Some(e);
                    e
                }
            };

            if let Ok((mut node, mut vis)) = marquee_q.get_mut(marquee) {
                node.left = Val::Px(min.x);
                node.top = Val::Px(min.y);
                node.width = Val::Px(w);
                node.height = Val::Px(h);
                *vis = Visibility::Visible;
            }
        }
    }
}

/// Build a translucent 3D mesh overlay for every currently-selected face,
/// parented to the edit target so it inherits the entity's `GlobalTransform`.
/// Each overlay uses the **same triangulation** as `EditMesh::bake_to_mesh` —
/// an `n - 2` triangle fan anchored at the face's first perimeter vertex
/// (index 0 = `face.verts[0]`), with the remaining perimeter vertices
/// pushed in `face.verts` order. The geometry intentionally mirrors
/// `bake_to_mesh` so the GPU rasterizes both meshes with the same per-pixel
/// depth interpolation; see
/// `AgentFiles/Documentation/mesh-edit-face-overlay-centroid-fan.md` for the
/// reasoning (the earlier centroid-fan approach filled the visible gap but
/// introduced a triangulation mismatch with the underlying mesh that
/// `depth_bias` could only mask, not fix).
///
/// Spawned only in Face mode; outside Face mode (or with no target / no
/// selected faces) every overlay is despawned so the screen stays clean.
///
/// Replaces the old gizmo-line triangle-fan approach that read as a
/// sparse fan instead of a tinted fill. A real 3D mesh overlay gives the
/// Blender-style translucent-tint look.
///
/// Strategy: despawn everything each frame and respawn. Typical meshes
/// have a handful of selected faces, so the cost is small. Avoids a
/// per-face diff + asset mutation dance.
#[allow(clippy::too_many_arguments)]
pub fn update_face_overlays(
    mut commands: Commands,
    mut overlay: ResMut<EditOverlayEntities>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mesh_selection: Res<MeshSelection>,
    edit_q: Query<&EditMesh>,
) {
    // 1. Despawn every previous overlay. Doing this before any early
    //    `return` keeps the screen clean when the user changes mode /
    //    target — stale overlays would otherwise linger.
    let stale: Vec<Entity> = overlay.face_overlays.drain(..).collect();
    for e in stale {
        commands.entity(e).despawn();
    }
    overlay.face_overlay_target = None;

    // 2. Bail out if we shouldn't spawn anything this frame.
    let Some(target) = mesh_selection.target else {
        return;
    };
    if mesh_selection.mode != SelectMode::Face {
        return;
    }
    let Ok(edit) = edit_q.get(target) else {
        return;
    };

    // 3. Lazily create the shared translucent material. `unlit` skips PBR
    //    shading so the tint reads as a flat colour (matches Blender's
    //    `face_select` theme overlay). `AlphaMode::Blend` keeps the draw
    //    order independent of which face is in front.
    //
    //    `depth_bias: 1.0` pushes the overlay in front of the cube's
    //    geometry — without it, the overlay is at the exact same Z as
    //    the source face, and the GPU's depth buffer can't decide which
    //    is in front, producing the "shimmer as you orbit" artefacts the
    //    user reported. **Sign convention:** in Bevy 0.19, positive
    //    `depth_bias` values render closer to the camera and negative
    //    values render behind — the inverse of what the older wgpu
    //    docs imply. We want the overlay drawn in front, so the value
    //    is positive. The bias is a constant value picked by the
    //    `wgpu::DepthBiasState`; units depend on the depth-format, but
    //    `1.0` is well above the precision noise for any reasonable
    //    scene.
    let material_handle = if let Some(h) = overlay.face_overlay_material.clone() {
        h
    } else {
        let mat = materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.55, 0.1, 0.45),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            depth_bias: 1.0,
            ..default()
        });
        overlay.face_overlay_material = Some(mat.clone());
        mat
    };

    // 4. Spawn one overlay entity per selected face. Each is a child of
    //    the edit target so its world transform = edit target's transform.
    overlay.face_overlay_target = Some(target);
    for face_id in mesh_selection.faces.iter() {
        let Some(face) = edit.faces.get(face_id.0 as usize) else {
            continue;
        };
        let n = face.verts.len();
        if n < 3 {
            continue; // degenerate face — skip rather than render a degenerate tri
        }

        // Perimeter positions in `face.verts` order, mesh-local. No
        // separate centroid vertex — the overlay mirrors
        // `EditMesh::bake_to_mesh` so the GPU rasterizes both meshes
        // with the same per-pixel depth interpolation. (A separate
        // centroid vertex would triangulate the same planar surface
        // differently and the GPU's per-triangle depth interpolation
        // would no longer match — see the per-bug design doc.)
        let mut positions: Vec<Vec3> = Vec::with_capacity(n);
        for vid in &face.verts {
            let Some(v) = edit.vertices.get(vid.0 as usize) else {
                continue;
            };
            positions.push(v.position);
        }
        if positions.len() != n {
            continue; // missing vertex → skip rather than ship a broken mesh
        }

        // Vertex-anchored fan: `(v_0, v_i, v_{i+1})` for `i in 1..n-1`.
        // Identical to `EditMesh::bake_to_mesh`'s triangulation so the
        // overlay and the underlying cube rasterize the same surface
        // into the same triangles — that match is what eliminates the
        // fan-shaped flicker at the centroid-fan boundaries.
        let indices = face_overlay_indices(n);

        // Build the mesh asset and spawn the overlay entity. The mesh
        // owns its own indices + positions; we don't reuse across faces
        // because each face has different geometry.
        //
        // `MAIN_WORLD | RENDER_WORLD` is required: Bevy 0.19's
        // `Mesh::insert_attribute` rejects any mesh that doesn't carry the
        // `MAIN_WORLD` flag (it panics with
        // `ExtractedToRenderWorld` even on a freshly-created mesh — the flag
        // is a permission, not a state). Without `MAIN_WORLD` the editor
        // crashes on the first face-overlay spawn. `RENDER_WORLD` is the
        // half that lets the GPU upload the mesh for the actual draw.
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_indices(Indices::U32(indices));
        let mesh_handle = meshes.add(mesh);

        let e = commands
            .spawn((
                Mesh3d(mesh_handle),
                MeshMaterial3d(material_handle.clone()),
                Transform::IDENTITY,
                ChildOf(target),
                Name::new("mesh-edit-face-overlay"),
            ))
            .id();
        overlay.face_overlays.push(e);
    }
}

/// Generate the index buffer for a vertex-anchored triangle fan of an
/// `n`-sided polygon whose vertices are laid out as
/// `[v_0, v_1, v_2, ..., v_{n-1}]`.
///
/// This is the same fan `EditMesh::bake_to_mesh` produces — `(v_0, v_i,
/// v_{i+1})` for `i in 1..n-1`, which gives `n - 2` triangles. Mirroring
/// the underlying mesh's triangulation is essential: even though both
/// fans cover the same planar surface, the GPU interpolates depth per
/// triangle. Two different triangulations of the same polygon produce
/// different per-pixel depth, and the resulting mismatch shows up as a
/// fan-shaped flicker that `depth_bias` can only mask, not fix.
///
/// Returns an empty buffer for `n < 3` (a degenerate face has no
/// triangles).
pub(crate) fn face_overlay_indices(n: usize) -> Vec<u32> {
    if n < 3 {
        return Vec::new();
    }
    let mut indices = Vec::with_capacity((n - 2) * 3);
    for i in 1..n - 1 {
        indices.extend_from_slice(&[0, i as u32, (i + 1) as u32]);
    }
    indices
}

/// Always-on cleanup: when the viewport isn't in Edit mode, hide every
/// vertex dot and marquee, and despawn every face overlay. The
/// `update_vertex_dots` / `update_marquee` / `update_face_overlays`
/// systems — all gated by `in_mode(ViewportMode::Edit)` — never fire
/// after Edit mode exits, so without this the dots, marquee, and 3D-mesh
/// overlays linger on screen until something else re-triggers them
/// (re-entering Edit mode, switching selection, respawning the mesh).
/// The user-facing symptom was: enter Edit mode, press `Tab` to exit,
/// press `Tab` again to re-enter Scene mode — the blue vertex dots from
/// the prior Edit-mode session are still drawn on the cube.
pub fn cleanup_overlay_when_not_editing(
    mut commands: Commands,
    mut overlay: ResMut<EditOverlayEntities>,
    viewport_settings: Res<ViewportSettings>,
    mut vis_q: Query<&mut Visibility, Or<(With<EditVertexDot>, With<EditMarqueeRect>)>>,
) {
    if viewport_settings.viewport_mode == ViewportMode::Edit {
        return;
    }
    for mut vis in &mut vis_q {
        *vis = Visibility::Hidden;
    }
    let stale: Vec<Entity> = overlay.face_overlays.drain(..).collect();
    for e in stale {
        commands.entity(e).despawn();
    }
    overlay.face_overlay_target = None;
}

#[cfg(test)]
mod tests {
    use super::face_overlay_indices;

    /// Verify the index buffer is exactly `n - 2` triangles long and
    /// that every index stays within the `n` perimeter vertices.
    fn assert_fan_shape(indices: &[u32], n: usize) {
        assert_eq!(
            indices.len(),
            (n - 2) * 3,
            "expected {} triangles ({} indices) for an {}-gon, got {}",
            n - 2,
            (n - 2) * 3,
            n,
            indices.len()
        );
        for (i, idx) in indices.iter().enumerate() {
            assert!(
                *idx < n as u32,
                "index {i} (={idx}) is out of bounds for n={n}"
            );
        }
        // Each triangle's three indices.
        for tri in indices.chunks_exact(3) {
            // Anchor: the first perimeter vertex (index 0).
            assert_eq!(tri[0], 0, "every triangle anchors at v_0 (index 0)");
            assert_ne!(tri[1], tri[2], "degenerate triangle (same vertex)");
        }
    }

    #[test]
    fn face_overlay_triangle() {
        // Triangle (n=3): single triangle, (v0, v1, v2).
        let indices = face_overlay_indices(3);
        assert_eq!(indices, vec![0, 1, 2]);
        assert_fan_shape(&indices, 3);
    }

    #[test]
    fn face_overlay_quad() {
        // Quad (n=4): two triangles — `(v0,v1,v2)` and `(v0,v2,v3)`.
        // This matches `EditMesh::bake_to_mesh` exactly.
        let indices = face_overlay_indices(4);
        assert_eq!(indices, vec![0, 1, 2, 0, 2, 3]);
        assert_fan_shape(&indices, 4);
    }

    #[test]
    fn face_overlay_pentagon() {
        // Pentagon (n=5): three triangles — (v0,v1,v2), (v0,v2,v3),
        // (v0,v3,v4). Matches `bake_to_mesh`.
        let indices = face_overlay_indices(5);
        assert_eq!(indices, vec![0, 1, 2, 0, 2, 3, 0, 3, 4]);
        assert_fan_shape(&indices, 5);
    }

    #[test]
    fn face_overlay_matches_bake_to_mesh() {
        // For a range of polygon sizes, the overlay's index buffer
        // must equal the indices `bake_to_mesh` would emit for the
        // same perimeter. We can't import `bake_to_mesh` here without
        // a full mesh asset; the duplicated formula below is the
        // ground-truth reference. If this drifts from `bake_to_mesh`,
        // the user will see fan-shaped flicker on the selected face.
        fn bake_indices(n: usize) -> Vec<u32> {
            if n < 3 {
                return Vec::new();
            }
            let mut out = Vec::with_capacity((n - 2) * 3);
            for i in 1..n - 1 {
                out.extend_from_slice(&[0, i as u32, (i + 1) as u32]);
            }
            out
        }
        for n in 3..=10usize {
            assert_eq!(
                face_overlay_indices(n),
                bake_indices(n),
                "overlay triangulation drifted from bake_to_mesh at n={n}"
            );
        }
    }

    #[test]
    fn face_overlay_rejects_degenerate_polygons() {
        // n < 3 produces no triangles — a 0-vertex "polygon" has
        // nothing to fill, a 1- or 2-vertex "polygon" is degenerate
        // and would otherwise emit either zero or one triangle with
        // reused indices. An empty buffer is the safe choice and lets
        // `update_face_overlays` skip the face cleanly.
        for n in 0..3usize {
            assert!(
                face_overlay_indices(n).is_empty(),
                "n={n}: expected empty index buffer"
            );
        }
    }
}