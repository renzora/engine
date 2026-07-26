//! Unity-style world-space UI: emit the laid-out UI tree as batched 3D geometry.
//!
//! A texture-mode [`WorldUiPanel`] renders its UI to an offscreen image on a quad
//! (RTT — flat, fixed-resolution). Mesh mode instead walks the *already
//! laid-out* UI tree — bevy_ui has computed each node's [`ComputedNode`] rect and
//! [`UiGlobalTransform`] — and emits geometry directly into the 3D scene on the
//! panel's plane. That's how Unity's world-space Canvas turns UI into scene
//! geometry.
//!
//! - **Background rects** → a vertex-coloured quad mesh on the panel entity.
//! - **Text** → each text node is rendered by [`build_text_mesh`] — the *same*
//!   crisp SDF glyph-mesh generator the standalone 3D-Text entity uses — as a
//!   child mesh placed at the node's position. Reusing that proven builder (rather
//!   than re-deriving glyph geometry from bevy's atlas here) keeps world-UI text
//!   pixel-identical to 3D text and sidesteps a class of glyph-placement bugs.
//!
//! ## Only rebuild on change
//!
//! The walk is cheap; recreating meshes/materials/textures is not — and doing it
//! every frame both tanks the FPS and (worse) despawns text meshes while the
//! render world may still reference them, drawing freed vertex buffers as garbage
//! triangles. So each panel stores a hash of the geometry it last built; a frame
//! whose walk produces the same hash touches nothing. Geometry is rebuilt only
//! when the layout actually changes (text edited, theme swapped, panel resized).
//!
//! Milestone: rects + SDF text. Images, borders and rounded corners follow.

use std::hash::Hasher;

use bevy::asset::RenderAssetUsages;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::text::{
    Font, FontAtlasSet, FontCx, FontSize, LayoutCx, RemSize, ScaleCx, TextColor, TextFont,
    TextPipeline,
};
use bevy::ui::{BackgroundColor, ComputedNode, UiGlobalTransform};

use renzora_text_mesh::{build_text_mesh, SdfTextMaterial, WORLD_UNITS_PER_PX};

use super::components::UiCanvas;
use super::world_panel::{canvas_resolution, canvas_size, WorldUiPanelLive, WorldUiPanelOwner};

/// Marks a child entity holding emitted text geometry, so it can be torn down and
/// rebuilt when the panel's content changes.
#[derive(Component)]
pub struct WorldUiTextGeom;

/// The content hash of the geometry a panel last emitted. While the walk keeps
/// producing this same hash, the emitter leaves everything as-is — no per-frame
/// mesh/texture churn (see the module docs). `sync_world_ui_panels` removes it on
/// any panel change to force a clean rebuild (e.g. a live texture→mesh switch).
#[derive(Component)]
pub(crate) struct WorldUiMeshBuilt(u64);

pub(crate) fn register(app: &mut App) {
    // Register the shared SDF material + shader (idempotent — text3d may also call
    // it; the guard dedupes since both link the same rlib type).
    renzora_text_mesh::ensure_sdf_material(app);
    app.add_systems(Update, emit_world_ui_meshes);
}

/// An accumulating vertex buffer for one mesh (the background rects).
#[derive(Default)]
struct Buf {
    positions: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl Buf {
    /// Quad centred at `(cx, cy, z)`, half-extents `(hw, hh)`, colour `col`.
    fn quad(&mut self, cx: f32, cy: f32, z: f32, hw: f32, hh: f32, col: [f32; 4]) {
        let base = self.positions.len() as u32;
        self.positions.push([cx - hw, cy - hh, z]);
        self.positions.push([cx + hw, cy - hh, z]);
        self.positions.push([cx + hw, cy + hh, z]);
        self.positions.push([cx - hw, cy + hh, z]);
        for _ in 0..4 {
            self.colors.push(col);
            self.normals.push([0.0, 0.0, 1.0]);
            self.uvs.push([0.0, 0.0]);
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    fn into_mesh(self) -> Mesh {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}

/// A text node collected during the walk, built into a mesh only on rebuild.
struct TextNode {
    /// Panel-local centre of the node (world units, pre-child-scale).
    center: Vec2,
    text: String,
    font: bevy::text::FontSource,
    size_px: f32,
    color: LinearRgba,
}

/// Fold a float into the running content hash by its exact bit pattern.
fn hash_f32(h: &mut impl Hasher, f: f32) {
    h.write_u32(f.to_bits());
}

/// Bevy's text pipeline resources, bundled so the emitter stays under the 16
/// system-param cap. [`build_text_mesh`] drives all of these to lay out and
/// rasterize a string (see its signature).
#[derive(SystemParam)]
struct TextCtx<'w> {
    pipeline: ResMut<'w, TextPipeline>,
    fonts: Res<'w, Assets<Font>>,
    atlas_set: ResMut<'w, FontAtlasSet>,
    font_cx: ResMut<'w, FontCx>,
    layout_cx: ResMut<'w, LayoutCx>,
    scale_cx: ResMut<'w, ScaleCx>,
    rem: Res<'w, RemSize>,
}

#[allow(clippy::too_many_arguments)]
fn emit_world_ui_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut sdf_materials: ResMut<Assets<SdfTextMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut tcx: TextCtx,
    panels: Query<(
        Entity,
        &UiCanvas,
        Option<&WorldUiPanelLive>,
        Option<&WorldUiMeshBuilt>,
    )>,
    children: Query<&Children>,
    text_children: Query<(), With<WorldUiTextGeom>>,
    nodes: Query<(
        &ComputedNode,
        &UiGlobalTransform,
        Option<&BackgroundColor>,
        Option<&Text>,
        Option<&TextFont>,
        Option<&TextColor>,
    )>,
    mut rect_mat: Local<Option<Handle<StandardMaterial>>>,
) {
    for (entity, canvas, live, built) in &panels {
        if !canvas.is_world() || !canvas.is_mesh_mode() {
            continue;
        }
        // Mesh mode but no template/root (e.g. the template was cleared) → drop any
        // text this canvas had emitted so stale glyphs don't sit on the dark surface
        // `sync_world_ui_canvases` now shows. The canvas's own dark quad comes from
        // there; here we only clean up our child geometry.
        let Some(live) = live else {
            if let Ok(ch) = children.get(entity) {
                for c in ch.iter() {
                    if text_children.get(c).is_ok() {
                        commands.entity(c).despawn();
                    }
                }
            }
            if built.is_some() {
                commands.entity(entity).remove::<WorldUiMeshBuilt>();
            }
            continue;
        };
        let panel_size = canvas_size(canvas);
        let res = canvas_resolution(canvas).as_vec2();
        if res.x <= 0.0 || res.y <= 0.0 {
            continue;
        }
        let scale = panel_size / res; // world units per UI px
        // px (y-down, origin top-left) → panel-local world (centred, y-up).
        let to_local =
            |px: Vec2| Vec2::new((px.x - res.x * 0.5) * scale.x, -(px.y - res.y * 0.5) * scale.y);

        let mut rects = Buf::default();
        let mut texts: Vec<TextNode> = Vec::new();

        // Content hash accumulated over the DETERMINISTIC breadth-first walk, so
        // an unchanged layout hashes identically frame to frame.
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        hash_f32(&mut hasher, panel_size.x);
        hash_f32(&mut hasher, panel_size.y);
        hash_f32(&mut hasher, res.x);
        hash_f32(&mut hasher, res.y);

        // Breadth-first from the panel root: parents before children.
        let mut queue: std::collections::VecDeque<Entity> = std::collections::VecDeque::new();
        queue.push_back(live.ui_root);
        let mut order = 0u32;
        while let Some(n) = queue.pop_front() {
            if let Ok((cn, gt, bg, text, tf, tc)) = nodes.get(n) {
                let center = gt.translation;
                // Skip any node bevy hasn't finished laying out — a non-finite
                // transform would emit garbage triangles reaching to infinity.
                if center.is_finite() {
                    let c = to_local(center);
                    if let Some(bg) = bg {
                        let lin = bg.0.to_linear();
                        if lin.alpha > 0.0 && cn.size.x > 0.0 && cn.size.y > 0.0 {
                            let col = [lin.red, lin.green, lin.blue, lin.alpha];
                            rects.quad(
                                c.x,
                                c.y,
                                order as f32 * 0.0005,
                                cn.size.x * 0.5 * scale.x,
                                cn.size.y * 0.5 * scale.y,
                                col,
                            );
                            hasher.write_u32(order);
                            for v in [c.x, c.y, cn.size.x, cn.size.y] {
                                hash_f32(&mut hasher, v);
                            }
                            for v in col {
                                hash_f32(&mut hasher, v);
                            }
                        }
                    }
                    if let (Some(text), Some(tf)) = (text, tf) {
                        let s = text.0.trim();
                        if !s.is_empty() {
                            let size_px = match tf.font_size {
                                FontSize::Px(p) => p,
                                _ => tcx.rem.0,
                            };
                            let color = tc.map(|c| c.0.to_linear()).unwrap_or(LinearRgba::WHITE);
                            for b in s.as_bytes() {
                                hasher.write_u8(*b);
                            }
                            for v in [c.x, c.y, size_px, color.red, color.green, color.blue] {
                                hash_f32(&mut hasher, v);
                            }
                            texts.push(TextNode {
                                center: c,
                                text: s.to_string(),
                                font: tf.font.clone(),
                                size_px,
                                color,
                            });
                        }
                    }
                }
            }
            if let Ok(ch) = children.get(n) {
                for c in ch.iter() {
                    queue.push_back(c);
                }
            }
            order += 1;
        }

        // Nothing changed since the last build → leave all geometry untouched.
        let hash = hasher.finish();
        if built.map(|b| b.0) == Some(hash) {
            continue;
        }

        // ── Background mesh on the panel entity ──
        if !rects.is_empty() {
            if rect_mat.is_none() {
                *rect_mat = Some(materials.add(StandardMaterial {
                    base_color: Color::WHITE,
                    unlit: true,
                    // OPAQUE on purpose: the background is one mesh with a single
                    // transparent-sort position (the panel centre), so if it were
                    // blended, any label farther than that centre would sort behind
                    // it and get painted over — text vanished on the panel's far
                    // side at an angle. Opaque writes depth in the opaque pass; the
                    // blended text then depth-tests in front and always shows.
                    alpha_mode: AlphaMode::Opaque,
                    cull_mode: None,
                    ..default()
                }));
            }
            let h = meshes.add(rects.into_mesh());
            commands
                .entity(entity)
                .insert((Mesh3d(h), MeshMaterial3d(rect_mat.as_ref().unwrap().clone())));
        }

        // ── Rebuild text child meshes via the shared 3D-text builder ──
        if let Ok(ch) = children.get(entity) {
            for c in ch.iter() {
                if text_children.get(c).is_ok() {
                    commands.entity(c).despawn();
                }
            }
        }
        // `build_text_mesh` emits at a fixed WORLD_UNITS_PER_PX; the child's scale
        // maps that onto this panel's px→world scale (so a 24px label is 24px tall
        // on the panel, per axis). A font still loading yields None — leave the
        // hash unset so we retry next frame instead of freezing on missing text.
        let mut all_ready = true;
        let child_scale = Vec3::new(
            scale.x / WORLD_UNITS_PER_PX,
            scale.y / WORLD_UNITS_PER_PX,
            1.0,
        );
        // Put ALL text in a z-band above every rect (rects top out at
        // `order * 0.0005`), so a deep node's blended background can't sort in
        // front of a shallow node's label and make it vanish at some angles.
        let text_base_z = (order as f32 + 2.0) * 0.0005;
        for (i, t) in texts.iter().enumerate() {
            let built = build_text_mesh(
                &mut tcx.pipeline,
                &tcx.fonts,
                &mut tcx.atlas_set,
                &mut images,
                &mut tcx.font_cx,
                &mut tcx.layout_cx,
                &mut tcx.scale_cx,
                tcx.rem.0,
                t.font.clone(),
                &t.text,
                t.size_px,
            );
            let Some((mesh, strip)) = built else {
                all_ready = false;
                continue;
            };
            let mesh_h = meshes.add(mesh);
            let mat = sdf_materials.add(SdfTextMaterial {
                color: t.color,
                atlas: strip,
            });
            // Tiny per-label bump breaks ties between overlapping text runs.
            let z = text_base_z + i as f32 * 0.0001;
            commands.spawn((
                Mesh3d(mesh_h),
                MeshMaterial3d(mat),
                Transform::from_translation(Vec3::new(t.center.x, t.center.y, z))
                    .with_scale(child_scale),
                Visibility::default(),
                // Glyph meshes are tiny and the AABB of scaled generated geometry
                // is culling them in and out with the camera — just never cull.
                NoFrustumCulling,
                // Generated chrome: keep it out of the scene hierarchy AND out of
                // the selection outline (which would trace every glyph as stray
                // dotted lines — see renzora_gizmo::selection_visuals).
                renzora::HideInHierarchy,
                WorldUiTextGeom,
                WorldUiPanelOwner(entity),
                ChildOf(entity),
            ));
        }

        if all_ready {
            commands.entity(entity).insert(WorldUiMeshBuilt(hash));
        } else {
            // A font wasn't ready — force a rebuild next frame.
            commands.entity(entity).remove::<WorldUiMeshBuilt>();
        }
    }
}
