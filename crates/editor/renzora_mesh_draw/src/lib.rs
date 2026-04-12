//! Mesh Draw — a Blender-style level-prototyping tool.
//!
//! Two draw tools and one mesh operation:
//!
//!   * **Draw Box**: click-drag a rectangle on the ground plane, release to
//!     lock, then move the cursor to extrude height, click to commit.
//!   * **Draw Polyline**: click to drop footprint points. Click the first
//!     point again (or press Enter) to close the polygon. Then extrude and
//!     click to commit.
//!   * **Join Selected**: merges the current 2+ mesh selection into a
//!     single mesh entity. Source meshes are despawned.
//!
//! Right-click / Escape cancels the in-progress gesture (tool stays on).
//! Activating any draw tool sets `ActiveTool::None` so built-in picking /
//! gizmo / box-select disengage.

use bevy::prelude::*;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::asset::RenderAssetUsages;
use bevy::window::PrimaryWindow;
use renzora::bevy_egui::egui::{self, Color32 as EColor, CursorIcon as ECursor};
use renzora::core::EditorCamera;
use renzora::core::viewport_types::ViewportState;
use renzora::editor::{
    ActiveTool, AppEditorExt, EditorCommands, EditorSelection, ShortcutEntry,
    ToolEntry, ToolSection, ViewportOverlayRegistry,
};
use renzora::core::keybindings::KeyBinding;
use egui_phosphor::regular::{CUBE, POLYGON, LINK};

// ── State ──────────────────────────────────────────────────────────────────

/// Which draw tool is currently bound to the mouse.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolMode {
    #[default]
    Box,
    Polyline,
}

#[derive(Default, Clone, Debug)]
pub enum DrawStage {
    #[default]
    Idle,
    /// Box tool: L held, rubber-banding a rectangle.
    DrawingFootprint { anchor: Vec3 },
    /// Polyline tool: accumulating footprint points.
    DrawingPolyline { points: Vec<Vec3> },
    /// Footprint is locked; cursor-vertical-plane hit drives height.
    Extruding { footprint: Footprint, height: f32 },
}

/// A footprint shape on the XZ plane.
#[derive(Clone, Debug, Reflect)]
pub enum Footprint {
    Box { min: Vec2, max: Vec2 },
    Polygon { points: Vec<Vec2> },
}

impl Footprint {
    fn center(&self) -> Vec2 {
        match self {
            Footprint::Box { min, max } => (*min + *max) * 0.5,
            Footprint::Polygon { points } => {
                let n = points.len().max(1) as f32;
                points.iter().copied().fold(Vec2::ZERO, |a, b| a + b) / n
            }
        }
    }
}

#[derive(Resource, Default, Debug)]
pub struct MeshDrawState {
    pub active: bool,
    pub tool_mode: ToolMode,
    pub stage: DrawStage,
    pub ray_origin: Option<Vec3>,
    pub ray_dir: Option<Vec3>,
    pub cursor_ground: Option<Vec3>,
}

/// Construction recipe saved on every drawn mesh (future edit passes regenerate).
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component)]
pub struct MeshDrawRecipe {
    pub footprint: Footprint,
    pub height: f32,
}

impl Default for MeshDrawRecipe {
    fn default() -> Self {
        Self {
            footprint: Footprint::Box { min: Vec2::ZERO, max: Vec2::ONE },
            height: 1.0,
        }
    }
}

/// Close the polyline when clicking within this world-space distance of the first point.
const POLY_CLOSE_RADIUS: f32 = 0.25;

// ── Plugin ─────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct MeshDrawPlugin;

impl Plugin for MeshDrawPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] MeshDrawPlugin");
        app.register_type::<MeshDrawRecipe>()
            .init_resource::<MeshDrawState>()
            .register_tool(
                ToolEntry::new(
                    "mesh_draw.box",
                    CUBE,
                    "Draw Box (click-drag, release, move to extrude, click to commit)",
                    ToolSection::Custom("mesh_draw"),
                )
                .order(0)
                .active_if(|w| is_active_with(w, ToolMode::Box))
                .on_activate(|w| toggle_tool(w, ToolMode::Box)),
            )
            .register_tool(
                ToolEntry::new(
                    "mesh_draw.polyline",
                    POLYGON,
                    "Draw Polyline (click to drop points, click first point or Enter to close)",
                    ToolSection::Custom("mesh_draw"),
                )
                .order(1)
                .active_if(|w| is_active_with(w, ToolMode::Polyline))
                .on_activate(|w| toggle_tool(w, ToolMode::Polyline)),
            )
            .register_tool(
                ToolEntry::new(
                    "mesh_draw.join",
                    LINK,
                    "Join Selected Meshes — Ctrl+J (merge 2+ selected into one)",
                    ToolSection::Custom("mesh_draw"),
                )
                .order(2)
                .visible_if(|w| selected_mesh_count(w) >= 2)
                .active_if(|_| false)
                .on_activate(join_selected_meshes),
            )
            .register_shortcut(ShortcutEntry::new(
                "mesh_draw.box",
                "Draw Box",
                "Mesh Draw",
                KeyBinding::new(KeyCode::KeyB),
                |w| toggle_tool(w, ToolMode::Box),
            ))
            .register_shortcut(ShortcutEntry::new(
                "mesh_draw.polyline",
                "Draw Polyline",
                "Mesh Draw",
                KeyBinding::new(KeyCode::KeyP),
                |w| toggle_tool(w, ToolMode::Polyline),
            ))
            .register_shortcut(ShortcutEntry::new(
                "mesh_draw.join",
                "Join Selected Meshes",
                "Mesh Draw",
                KeyBinding::new(KeyCode::KeyJ).ctrl(),
                join_selected_meshes,
            ))
            .add_systems(
                Update,
                (
                    update_cursor_state,
                    handle_mouse_input,
                    draw_preview_gizmos,
                    handle_keys,
                )
                    .chain(),
            );

        app.world_mut()
            .resource_mut::<ViewportOverlayRegistry>()
            .register(200, draw_cursor_overlay);
    }
}

renzora::add!(MeshDrawPlugin, Editor);

// ── Toolbar activation ─────────────────────────────────────────────────────

fn is_active_with(world: &World, mode: ToolMode) -> bool {
    world
        .get_resource::<MeshDrawState>()
        .map_or(false, |s| s.active && s.tool_mode == mode)
}

/// Force-deactivate the tool. Used after a commit so the user isn't left
/// in draw mode with a fresh cursor-press about to start a new shape.
fn deactivate_tool(world: &mut World, _mode: ToolMode) {
    {
        let mut s = world.resource_mut::<MeshDrawState>();
        s.active = false;
        s.stage = DrawStage::Idle;
    }
    world.insert_resource(ActiveTool::Select);
}

fn toggle_tool(world: &mut World, target: ToolMode) {
    let becoming_active = {
        let mut s = world.resource_mut::<MeshDrawState>();
        if s.active && s.tool_mode == target {
            // Deactivate
            s.active = false;
            s.stage = DrawStage::Idle;
            false
        } else {
            s.active = true;
            s.tool_mode = target;
            s.stage = DrawStage::Idle;
            true
        }
    };
    if becoming_active {
        world.insert_resource(ActiveTool::None);
    } else {
        world.insert_resource(ActiveTool::Select);
    }
}

fn selected_mesh_count(world: &World) -> usize {
    let Some(sel) = world.get_resource::<EditorSelection>() else { return 0 };
    sel.get_all()
        .into_iter()
        .filter(|e| world.get::<Mesh3d>(*e).is_some())
        .count()
}

// ── Cursor ray / ground projection ─────────────────────────────────────────

fn update_cursor_state(
    mut state: ResMut<MeshDrawState>,
    viewport: Option<Res<ViewportState>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
) {
    if !state.active {
        state.ray_origin = None;
        state.ray_dir = None;
        state.cursor_ground = None;
        return;
    }
    let clear = |s: &mut MeshDrawState| {
        s.ray_origin = None; s.ray_dir = None; s.cursor_ground = None;
    };
    let Some(viewport) = viewport else { return };
    let Ok(window) = window_q.single() else { return };
    let Some(cursor) = window.cursor_position() else { clear(&mut state); return };

    let vp_min = viewport.screen_position;
    let vp_max = vp_min + viewport.screen_size;
    if cursor.x < vp_min.x || cursor.y < vp_min.y || cursor.x > vp_max.x || cursor.y > vp_max.y {
        clear(&mut state);
        return;
    }
    let Some((camera, cam_xform)) = camera_q.iter().next() else { clear(&mut state); return };
    let viewport_pos = Vec2::new(
        (cursor.x - vp_min.x) / viewport.screen_size.x * viewport.current_size.x as f32,
        (cursor.y - vp_min.y) / viewport.screen_size.y * viewport.current_size.y as f32,
    );
    let Ok(ray) = camera.viewport_to_world(cam_xform, viewport_pos) else { clear(&mut state); return };

    let dir = ray.direction.as_vec3();
    state.ray_origin = Some(ray.origin);
    state.ray_dir = Some(dir);
    state.cursor_ground = ground_hit(ray.origin, dir);
}

fn ground_hit(origin: Vec3, dir: Vec3) -> Option<Vec3> {
    if dir.y.abs() <= 1e-6 { return None; }
    let t = -origin.y / dir.y;
    if t <= 0.0 || t > 10_000.0 { return None; }
    let hit = origin + dir * t;
    Some(Vec3::new(hit.x, 0.0, hit.z))
}

/// Intersect cursor ray with a vertical plane through `center_xz` whose
/// normal is the camera forward projected onto XZ.
fn vertical_plane_hit(
    state: &MeshDrawState,
    cam_xform: &GlobalTransform,
    center_xz: Vec2,
) -> Option<Vec3> {
    let origin = state.ray_origin?;
    let dir = state.ray_dir?;
    let fwd = cam_xform.forward().as_vec3();
    let mut n = Vec3::new(fwd.x, 0.0, fwd.z);
    if n.length_squared() < 1e-6 { n = Vec3::Z; }
    let n = n.normalize();
    let p0 = Vec3::new(center_xz.x, 0.0, center_xz.y);
    let denom = dir.dot(n);
    if denom.abs() < 1e-6 { return None; }
    let t = (p0 - origin).dot(n) / denom;
    if t <= 0.0 { return None; }
    Some(origin + dir * t)
}

// ── Input state machine ────────────────────────────────────────────────────

fn handle_mouse_input(
    mut state: ResMut<MeshDrawState>,
    buttons: Res<ButtonInput<MouseButton>>,
    camera_q: Query<&GlobalTransform, With<EditorCamera>>,
    cmds: Option<Res<EditorCommands>>,
) {
    if !state.active { return; }

    if buttons.just_pressed(MouseButton::Right) {
        state.stage = DrawStage::Idle;
        return;
    }

    match state.stage.clone() {
        DrawStage::Idle => {
            if !buttons.just_pressed(MouseButton::Left) { return; }
            let Some(p) = state.cursor_ground else { return };
            match state.tool_mode {
                ToolMode::Box => state.stage = DrawStage::DrawingFootprint { anchor: p },
                ToolMode::Polyline => state.stage = DrawStage::DrawingPolyline { points: vec![p] },
            }
        }

        DrawStage::DrawingFootprint { anchor } => {
            if !buttons.just_released(MouseButton::Left) { return; }
            let Some(cur) = state.cursor_ground else {
                state.stage = DrawStage::Idle;
                return;
            };
            let min = Vec2::new(anchor.x.min(cur.x), anchor.z.min(cur.z));
            let max = Vec2::new(anchor.x.max(cur.x), anchor.z.max(cur.z));
            let size = max - min;
            if size.x.abs() < 0.05 || size.y.abs() < 0.05 {
                state.stage = DrawStage::Idle;
                return;
            }
            state.stage = DrawStage::Extruding {
                footprint: Footprint::Box { min, max },
                height: 0.01,
            };
        }

        DrawStage::DrawingPolyline { mut points } => {
            if !buttons.just_pressed(MouseButton::Left) {
                state.stage = DrawStage::DrawingPolyline { points };
                return;
            }
            let Some(cur) = state.cursor_ground else {
                state.stage = DrawStage::DrawingPolyline { points };
                return;
            };
            // Close if clicking near first point
            let close_now = points.len() >= 3
                && cur.distance(points[0]) < POLY_CLOSE_RADIUS;
            if close_now {
                let pts2: Vec<Vec2> = points.iter().map(|p| Vec2::new(p.x, p.z)).collect();
                state.stage = DrawStage::Extruding {
                    footprint: Footprint::Polygon { points: pts2 },
                    height: 0.01,
                };
                return;
            }
            // Skip duplicate if too close to last
            if let Some(last) = points.last() {
                if last.distance(cur) < 0.02 {
                    state.stage = DrawStage::DrawingPolyline { points };
                    return;
                }
            }
            points.push(cur);
            state.stage = DrawStage::DrawingPolyline { points };
        }

        DrawStage::Extruding { footprint, .. } => {
            let center = footprint.center();
            let new_height = camera_q
                .iter()
                .next()
                .and_then(|cam_xform| vertical_plane_hit(&state, cam_xform, center))
                .map(|hit| hit.y.max(0.01))
                .unwrap_or(0.01);

            if buttons.just_pressed(MouseButton::Left) {
                let recipe = MeshDrawRecipe { footprint, height: new_height };
                let current_mode = state.tool_mode;
                if let Some(cmds) = cmds {
                    cmds.push(move |world: &mut World| {
                        renzora::undo::execute(
                            world,
                            renzora::undo::UndoContext::Scene,
                            Box::new(SpawnDrawnMeshCmd { recipe, entity: None }),
                        );
                        // Auto-deactivate after committing so the next click
                        // doesn't immediately start another shape.
                        deactivate_tool(world, current_mode);
                    });
                }
                state.stage = DrawStage::Idle;
                state.active = false;
            } else {
                state.stage = DrawStage::Extruding { footprint, height: new_height };
            }
        }
    }
}

fn handle_keys(world: &mut World) {
    let keys = world.resource::<ButtonInput<KeyCode>>();
    let escape = keys.just_pressed(KeyCode::Escape);
    let enter = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);
    if !escape && !enter { return; }

    let (active, stage_clone) = {
        let s = world.resource::<MeshDrawState>();
        (s.active, s.stage.clone())
    };
    if !active { return; }

    if escape {
        match stage_clone {
            DrawStage::Idle => {
                // Second Escape exits the tool.
                let mode = world.resource::<MeshDrawState>().tool_mode;
                toggle_tool(world, mode);
            }
            _ => {
                world.resource_mut::<MeshDrawState>().stage = DrawStage::Idle;
            }
        }
        return;
    }

    // Enter: finalize polyline if possible.
    if enter {
        if let DrawStage::DrawingPolyline { points } = stage_clone {
            if points.len() >= 3 {
                let pts2: Vec<Vec2> = points.iter().map(|p| Vec2::new(p.x, p.z)).collect();
                world.resource_mut::<MeshDrawState>().stage = DrawStage::Extruding {
                    footprint: Footprint::Polygon { points: pts2 },
                    height: 0.01,
                };
            }
        }
    }
}

// ── Preview (Bevy gizmos — pure outline, no occluder) ──────────────────────

fn draw_preview_gizmos(mut gizmos: Gizmos, state: Res<MeshDrawState>) {
    if !state.active { return; }
    let color = Color::srgb(0.35, 0.75, 1.0);
    let ghost = Color::srgba(0.35, 0.75, 1.0, 0.6);

    match &state.stage {
        DrawStage::Idle => {}
        DrawStage::DrawingFootprint { anchor } => {
            let Some(cur) = state.cursor_ground else { return };
            let min = Vec3::new(anchor.x.min(cur.x), 0.01, anchor.z.min(cur.z));
            let max = Vec3::new(anchor.x.max(cur.x), 0.01, anchor.z.max(cur.z));
            draw_rect_y(&mut gizmos, min, max, color);
        }
        DrawStage::DrawingPolyline { points } => {
            for pair in points.windows(2) {
                gizmos.line(pair[0], pair[1], color);
            }
            if let (Some(first), Some(last)) = (points.first(), points.last()) {
                if let Some(cur) = state.cursor_ground {
                    gizmos.line(*last, cur, ghost);
                    if points.len() >= 3 && cur.distance(*first) < POLY_CLOSE_RADIUS {
                        gizmos.line(cur, *first, ghost);
                    }
                }
                // First point marker
                let r = 0.08;
                gizmos.line(*first + Vec3::new(-r, 0.02, 0.0), *first + Vec3::new(r, 0.02, 0.0), color);
                gizmos.line(*first + Vec3::new(0.0, 0.02, -r), *first + Vec3::new(0.0, 0.02, r), color);
            }
        }
        DrawStage::Extruding { footprint, height } => {
            match footprint {
                Footprint::Box { min, max } => {
                    let lo = Vec3::new(min.x, 0.0, min.y);
                    let hi = Vec3::new(max.x, *height, max.y);
                    draw_wire_box(&mut gizmos, lo, hi, color);
                }
                Footprint::Polygon { points } => {
                    let n = points.len();
                    for i in 0..n {
                        let a2 = points[i];
                        let b2 = points[(i + 1) % n];
                        let a_lo = Vec3::new(a2.x, 0.0, a2.y);
                        let b_lo = Vec3::new(b2.x, 0.0, b2.y);
                        let a_hi = Vec3::new(a2.x, *height, a2.y);
                        let b_hi = Vec3::new(b2.x, *height, b2.y);
                        gizmos.line(a_lo, b_lo, color);
                        gizmos.line(a_hi, b_hi, color);
                        gizmos.line(a_lo, a_hi, color);
                    }
                }
            }
        }
    }
}

fn draw_rect_y(gizmos: &mut Gizmos, min: Vec3, max: Vec3, color: Color) {
    let y = min.y;
    let a = Vec3::new(min.x, y, min.z);
    let b = Vec3::new(max.x, y, min.z);
    let c = Vec3::new(max.x, y, max.z);
    let d = Vec3::new(min.x, y, max.z);
    gizmos.line(a, b, color);
    gizmos.line(b, c, color);
    gizmos.line(c, d, color);
    gizmos.line(d, a, color);
}

fn draw_wire_box(gizmos: &mut Gizmos, min: Vec3, max: Vec3, color: Color) {
    let b00 = Vec3::new(min.x, min.y, min.z);
    let b10 = Vec3::new(max.x, min.y, min.z);
    let b11 = Vec3::new(max.x, min.y, max.z);
    let b01 = Vec3::new(min.x, min.y, max.z);
    let t00 = Vec3::new(min.x, max.y, min.z);
    let t10 = Vec3::new(max.x, max.y, min.z);
    let t11 = Vec3::new(max.x, max.y, max.z);
    let t01 = Vec3::new(min.x, max.y, max.z);
    for (a, b) in [
        (b00, b10), (b10, b11), (b11, b01), (b01, b00),
        (t00, t10), (t10, t11), (t11, t01), (t01, t00),
        (b00, t00), (b10, t10), (b11, t11), (b01, t01),
    ] {
        gizmos.line(a, b, color);
    }
}

// ── Viewport overlay (cursor + border tint) ────────────────────────────────

fn draw_cursor_overlay(ui: &mut egui::Ui, world: &World, rect: egui::Rect) {
    let Some(state) = world.get_resource::<MeshDrawState>() else { return };
    if !state.active { return; }
    let pointer_in = ui.ctx().pointer_hover_pos().map_or(false, |p| rect.contains(p));
    if pointer_in {
        ui.ctx().set_cursor_icon(ECursor::Crosshair);
    }
    let stroke = egui::Stroke::new(1.0, EColor::from_rgba_unmultiplied(90, 190, 255, 180));
    ui.painter().rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);
}

// ── Commit drawn meshes ────────────────────────────────────────────────────

/// Spawn a mesh built from `recipe` and return its entity.
fn spawn_drawn_mesh(world: &mut World, recipe: &MeshDrawRecipe) -> Entity {
    let (mesh, pivot) = build_recipe_mesh(recipe);
    let is_polygon = matches!(recipe.footprint, Footprint::Polygon { .. });
    let mesh_handle = world.resource_mut::<Assets<Mesh>>().add(mesh);
    let material_handle = world.resource_mut::<Assets<StandardMaterial>>().add(
        StandardMaterial {
            base_color: Color::srgb(0.8, 0.75, 0.7),
            // Polygon footprints can end up with inverted winding depending
            // on the order the user clicked points. Render double-sided so
            // the shape always appears filled.
            cull_mode: if is_polygon { None } else { Some(bevy::render::render_resource::Face::Back) },
            ..default()
        }
    );
    world
        .spawn((
            Name::new("DrawnMesh"),
            Mesh3d(mesh_handle),
            MeshMaterial3d(material_handle),
            Transform::from_translation(pivot),
            Visibility::default(),
            recipe.clone(),
        ))
        .id()
}

/// Undoable spawn: `execute` creates the entity and remembers its id;
/// `undo` despawns it; `execute` runs again on redo and respawns with the
/// original recipe.
struct SpawnDrawnMeshCmd {
    recipe: MeshDrawRecipe,
    entity: Option<Entity>,
}

impl renzora::undo::UndoCommand for SpawnDrawnMeshCmd {
    fn label(&self) -> &str { "Draw Mesh" }

    fn execute(&mut self, world: &mut World) {
        self.entity = Some(spawn_drawn_mesh(world, &self.recipe));
    }

    fn undo(&mut self, world: &mut World) {
        if let Some(entity) = self.entity.take() {
            if let Ok(ent) = world.get_entity_mut(entity) {
                ent.despawn();
            }
        }
    }
}

/// Build a mesh from a recipe. Returns `(mesh, pivot)` where pivot is the
/// Transform translation to place the entity at so the mesh sits on the
/// ground. The mesh itself is centered at the origin (XZ) with y in [0, height].
fn build_recipe_mesh(recipe: &MeshDrawRecipe) -> (Mesh, Vec3) {
    match &recipe.footprint {
        Footprint::Box { min, max } => {
            let size = *max - *min;
            let center_xz = (*min + *max) * 0.5;
            let pivot = Vec3::new(center_xz.x, recipe.height * 0.5, center_xz.y);
            let mesh: Mesh = Cuboid::new(
                size.x.max(0.01),
                recipe.height.max(0.01),
                size.y.max(0.01),
            ).into();
            (mesh, pivot)
        }
        Footprint::Polygon { points } => {
            // Centroid-origin: shift footprint so its centroid is at origin.
            let centroid = {
                let n = points.len().max(1) as f32;
                points.iter().copied().fold(Vec2::ZERO, |a, b| a + b) / n
            };
            let local: Vec<Vec2> = points.iter().map(|p| *p - centroid).collect();
            let pivot = Vec3::new(centroid.x, 0.0, centroid.y);
            (build_prism_mesh(&local, recipe.height.max(0.01)), pivot)
        }
    }
}

/// Build an extruded prism from a simple polygon on the XZ plane.
/// Caps are triangulated via ear clipping so non-convex shapes fill correctly.
fn build_prism_mesh(footprint: &[Vec2], height: f32) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // Normalize to CCW so triangulation winding is predictable.
    let ccw = signed_area(footprint) > 0.0;
    let poly: Vec<Vec2> = if ccw {
        footprint.to_vec()
    } else {
        footprint.iter().rev().copied().collect()
    };
    let n = poly.len();
    if n < 3 {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new());
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, Vec::<[f32; 2]>::new());
        mesh.insert_indices(Indices::U32(Vec::new()));
        return mesh;
    }

    let cap_tris = triangulate_ear_clip(&poly);

    // Bottom cap — push ring, then triangles with reversed winding so normal points -Y.
    let bot_start = positions.len() as u32;
    for p in &poly {
        positions.push([p.x, 0.0, p.y]);
        normals.push([0.0, -1.0, 0.0]);
        uvs.push([p.x, p.y]);
    }
    for [a, b, c] in &cap_tris {
        indices.extend_from_slice(&[bot_start + *a, bot_start + *c, bot_start + *b]);
    }

    // Top cap — push ring, triangles keep CCW so normal points +Y.
    let top_start = positions.len() as u32;
    for p in &poly {
        positions.push([p.x, height, p.y]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([p.x, p.y]);
    }
    for [a, b, c] in &cap_tris {
        indices.extend_from_slice(&[top_start + *a, top_start + *b, top_start + *c]);
    }

    // Side quads — unique vertices per edge so normals are face-flat.
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let edge = Vec2::new(b.x - a.x, b.y - a.y);
        // Outward normal (polygon is CCW on XZ): rotate edge +90° around Y.
        let mut nrm = Vec3::new(edge.y, 0.0, -edge.x);
        nrm = nrm.normalize_or_zero();
        let base = positions.len() as u32;
        positions.push([a.x, 0.0, a.y]);
        positions.push([b.x, 0.0, b.y]);
        positions.push([b.x, height, b.y]);
        positions.push([a.x, height, a.y]);
        for _ in 0..4 { normals.push([nrm.x, nrm.y, nrm.z]); }
        uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn signed_area(poly: &[Vec2]) -> f32 {
    let n = poly.len();
    let mut s = 0.0f32;
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        s += a.x * b.y - b.x * a.y;
    }
    s * 0.5
}

/// Ear-clipping triangulation of a simple polygon (assumed CCW).
/// Returns a list of triangles as `[i, j, k]` indices into the input slice.
fn triangulate_ear_clip(poly: &[Vec2]) -> Vec<[u32; 3]> {
    let n = poly.len();
    let mut out: Vec<[u32; 3]> = Vec::with_capacity(n.saturating_sub(2));
    if n < 3 { return out; }

    let mut remaining: Vec<u32> = (0..n as u32).collect();
    // Safety bound: at most n iterations to guard against pathological input.
    let mut guard = n * n;
    while remaining.len() > 3 && guard > 0 {
        guard -= 1;
        let m = remaining.len();
        let mut found_ear = false;

        for i in 0..m {
            let ia = remaining[(i + m - 1) % m];
            let ib = remaining[i];
            let ic = remaining[(i + 1) % m];
            let a = poly[ia as usize];
            let b = poly[ib as usize];
            let c = poly[ic as usize];

            // Must be a convex corner (CCW triangle).
            if tri_signed_area(a, b, c) <= 1e-7 { continue; }

            // No other remaining vertex may lie strictly inside triangle abc.
            let mut any_inside = false;
            for &idx in &remaining {
                if idx == ia || idx == ib || idx == ic { continue; }
                if point_in_triangle(poly[idx as usize], a, b, c) {
                    any_inside = true;
                    break;
                }
            }
            if any_inside { continue; }

            out.push([ia, ib, ic]);
            remaining.remove(i);
            found_ear = true;
            break;
        }

        if !found_ear {
            // Degenerate / self-intersecting input — fall back to a fan so we
            // at least produce *some* geometry rather than hanging.
            let pivot = remaining[0];
            for i in 1..remaining.len() - 1 {
                out.push([pivot, remaining[i], remaining[i + 1]]);
            }
            return out;
        }
    }

    if remaining.len() == 3 {
        out.push([remaining[0], remaining[1], remaining[2]]);
    }
    out
}

fn tri_signed_area(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    ((b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y)) * 0.5
}

fn point_in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    // Barycentric sign test. Strict inside only — edges don't count as inside.
    let d1 = tri_signed_area(p, a, b);
    let d2 = tri_signed_area(p, b, c);
    let d3 = tri_signed_area(p, c, a);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}

// ── Join selected meshes ───────────────────────────────────────────────────

fn join_selected_meshes(world: &mut World) {
    let entities = {
        let Some(sel) = world.get_resource::<EditorSelection>() else { return };
        sel.get_all()
    };
    if entities.len() < 2 { return; }

    // Collect mesh handle + world transform per selected entity.
    let mut sources: Vec<(Entity, Handle<Mesh>, Transform)> = Vec::new();
    for e in &entities {
        let Some(mesh3d) = world.get::<Mesh3d>(*e) else { continue };
        let Some(gt) = world.get::<GlobalTransform>(*e) else { continue };
        sources.push((*e, mesh3d.0.clone(), gt.compute_transform()));
    }
    if sources.len() < 2 { return; }

    // Build merged mesh in world space.
    let merged = {
        let assets = world.resource::<Assets<Mesh>>();
        merge_meshes(assets, &sources)
    };

    let mesh_handle = world.resource_mut::<Assets<Mesh>>().add(merged);
    let material_handle = world.resource_mut::<Assets<StandardMaterial>>().add(
        StandardMaterial { base_color: Color::srgb(0.8, 0.75, 0.7), ..default() }
    );
    let new_entity = world.spawn((
        Name::new("JoinedMesh"),
        Mesh3d(mesh_handle),
        MeshMaterial3d(material_handle),
        Transform::IDENTITY,
        Visibility::default(),
    )).id();

    // Despawn originals, reselect the merged result.
    if let Some(sel) = world.get_resource::<EditorSelection>() {
        sel.set(Some(new_entity));
    }
    for (e, _, _) in sources {
        if let Ok(ent) = world.get_entity_mut(e) {
            ent.despawn();
        }
    }
}

fn merge_meshes(assets: &Assets<Mesh>, sources: &[(Entity, Handle<Mesh>, Transform)]) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for (_, handle, xform) in sources {
        let Some(mesh) = assets.get(handle) else { continue };
        if mesh.primitive_topology() != PrimitiveTopology::TriangleList { continue; }

        let Some(pos_vals) = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(|v| match v {
                VertexAttributeValues::Float32x3(v) => Some(v.clone()),
                _ => None,
            }) else { continue };

        let base = positions.len() as u32;
        let matrix = xform.to_matrix();

        for p in &pos_vals {
            let v = matrix.transform_point3(Vec3::from_array(*p));
            positions.push([v.x, v.y, v.z]);
        }

        let src_normals: Option<Vec<[f32; 3]>> = mesh
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .and_then(|v| match v {
                VertexAttributeValues::Float32x3(v) => Some(v.clone()),
                _ => None,
            });
        if let Some(ns) = src_normals {
            for n in &ns {
                let rotated = xform.rotation * Vec3::from_array(*n);
                let rn = rotated.normalize_or_zero();
                normals.push([rn.x, rn.y, rn.z]);
            }
        } else {
            for _ in 0..pos_vals.len() { normals.push([0.0, 1.0, 0.0]); }
        }

        let src_uvs: Option<Vec<[f32; 2]>> = mesh
            .attribute(Mesh::ATTRIBUTE_UV_0)
            .and_then(|v| match v {
                VertexAttributeValues::Float32x2(v) => Some(v.clone()),
                _ => None,
            });
        if let Some(us) = src_uvs {
            for u in &us { uvs.push(*u); }
        } else {
            for _ in 0..pos_vals.len() { uvs.push([0.0, 0.0]); }
        }

        match mesh.indices() {
            Some(Indices::U32(idxs)) => { for i in idxs { indices.push(base + i); } }
            Some(Indices::U16(idxs)) => { for i in idxs { indices.push(base + *i as u32); } }
            None => {
                for i in 0..pos_vals.len() as u32 { indices.push(base + i); }
            }
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}
