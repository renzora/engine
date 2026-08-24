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

use bevy::prelude::*;
use bevy::ui::ZIndex;
use renzora::core::viewport_types::{ViewportSettings, ViewportState};
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