#![allow(dead_code)] // Public surface area kept for upcoming features.

//! Viewport context menu — right-click-tap to open.
//!
//! Two modes, chosen by what's under the cursor at press:
//!
//! * Empty space → **Add** menu populated from `SpawnRegistry`. Selected
//!   preset spawns at the ground-plane hit point and is selected.
//! * An entity → **Entity actions** menu (Duplicate, Delete, Focus,
//!   Deselect). Dispatches existing `EditorAction`s through `KeyBindings`
//!   so rebinds and existing consumers still apply.
//!
//! Right-click is shared with the camera fly / orbit gesture. We
//! distinguish by accumulating `MouseMotion` during the press — if any
//! motion happens, it's a drag (suppress menu). Pure taps open the menu.
//! `MouseMotion` is used instead of cursor position because camera
//! controls lock/grab the cursor while orbiting, which would otherwise
//! freeze the cursor-delta check.
//!
//! NOTE: the egui renderer + egui palette panel were removed during the
//! egui→bevy_ui migration. This crate now only detects the right-click tap
//! and records the resulting [`ContextMenuState`]; nothing renders it yet.
//! It compiles as an effectively no-op plugin pending a native renderer.

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use renzora::core::keybindings::{EditorAction, KeyBindings};
use renzora::core::viewport_types::ViewportState;
use renzora::core::EditorCamera;
use renzora_editor_framework::{
    ActiveTool, EditorSelection, InspectorRegistry, OpenAddComponentMenuRequest,
    SpawnRegistry, SplashState,
};

// ── State ──────────────────────────────────────────────────────────────────

#[derive(Resource, Default, Debug)]
pub struct RightClickTracker {
    pub pressed: bool,
    pub press_pos: Vec2,
    /// Total pixel motion (from MouseMotion events) accumulated while held.
    pub motion_magnitude: f32,
}

#[derive(Resource, Default, Debug)]
pub struct ContextMenuState {
    pub open: bool,
    pub screen_pos: Vec2,
    pub kind: MenuKind,
    /// Live substring filter applied to menu entries.
    pub query: String,
    /// True on the first render after opening — forces focus onto the
    /// search text input so the user can type immediately.
    pub just_opened: bool,
    /// Incremented each time the menu opens. Mixed into the render id so each
    /// open gets a fresh layout.
    pub open_counter: u64,
    /// Category to scroll to in the item list (set by sidebar click).
    pub scroll_to_category: Option<String>,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum MenuKind {
    #[default]
    None,
    /// Spawn a preset at this world position.
    AddHere { world_pos: Vec3 },
    /// Act on the current `EditorSelection` (Duplicate, Delete, Focus,
    /// Deselect, Add Component). Shown when at least one entity is selected.
    EntityActions,
    /// Component picker — reached from the EntityActions menu via "Add
    /// Component". Lists reflected components that can be attached to the
    /// current selection.
    AddComponent,
}

/// Drag threshold in pixels. Motion magnitude below this still counts as a tap.
const DRAG_THRESHOLD_PX: f32 = 4.0;

// ── Plugin ─────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct ContextMenuPlugin;

impl Plugin for ContextMenuPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ContextMenuPlugin");
        app.init_resource::<RightClickTracker>()
            .init_resource::<ContextMenuState>()
            .add_systems(
                Update,
                (detect_right_click_tap, consume_add_component_request)
                    .run_if(in_state(SplashState::Editor)),
            );
    }
}

/// Drain pending [`OpenAddComponentMenuRequest`] — set by hierarchy /
/// inspector / any panel that wants to trigger the component picker.
fn consume_add_component_request(
    mut commands: Commands,
    request: Option<Res<OpenAddComponentMenuRequest>>,
    mut menu: ResMut<ContextMenuState>,
) {
    let Some(req) = request else { return };
    menu.open = true;
    menu.screen_pos = req.screen_pos;
    menu.kind = MenuKind::AddComponent;
    menu.query.clear();
    menu.just_opened = true;
    menu.open_counter = menu.open_counter.wrapping_add(1);
    commands.remove_resource::<OpenAddComponentMenuRequest>();
}

// ── Right-click tap detection ──────────────────────────────────────────────

fn detect_right_click_tap(
    mut tracker: ResMut<RightClickTracker>,
    mut menu: ResMut<ContextMenuState>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<MouseMotion>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    viewport: Option<Res<ViewportState>>,
    active_tool: Option<Res<ActiveTool>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    selection: Option<Res<EditorSelection>>,
    dock: Option<Res<renzora_ember::dock::Dock>>,
) {
    // Accumulate motion every frame — works even when the camera has
    // grabbed the cursor. Only counts while the button is held.
    if tracker.pressed {
        for ev in motion.read() {
            tracker.motion_magnitude += ev.delta.length();
        }
    } else {
        motion.clear();
    }

    let tool_ok = active_tool
        .as_deref()
        .map(|t| {
            matches!(
                t,
                ActiveTool::Select | ActiveTool::Translate | ActiveTool::Rotate | ActiveTool::Scale
            )
        })
        .unwrap_or(true);

    let Ok(window) = window_q.single() else {
        return;
    };
    let cursor = window.cursor_position();

    if mouse.just_pressed(MouseButton::Right) {
        if let Some(c) = cursor {
            tracker.pressed = true;
            tracker.press_pos = c;
            tracker.motion_magnitude = 0.0;
        }
        return;
    }

    if !mouse.just_released(MouseButton::Right) {
        return;
    }

    let was_tap = tracker.pressed && tracker.motion_magnitude <= DRAG_THRESHOLD_PX;
    tracker.pressed = false;
    if !was_tap || !tool_ok {
        return;
    }

    let Some(cursor) = cursor else { return };
    let Some(viewport) = viewport.as_deref() else {
        return;
    };
    // Suppress the viewport context menu if the Viewport panel isn't the
    // currently-visible tab in its dock leaf. Otherwise a right-click in a
    // Blueprint / Material / other panel that shares the same dock slot
    // would spawn the scene's Add / EntityActions menu, because
    // ViewportState.screen_position is stale from when the Viewport tab
    // was last rendered.
    // Read from the live dock. This was `DockingState`, whose tree was seeded
    // once at startup and never updated by the bevy_ui dock, so the gate was
    // answering about a layout that had not been true since boot.
    if !dock
        .as_deref()
        .is_none_or(|d| d.tree.is_active_tab("viewport"))
    {
        return;
    }
    let vp_min = viewport.screen_position;
    let vp_max = vp_min + viewport.screen_size;
    if cursor.x < vp_min.x || cursor.y < vp_min.y || cursor.x > vp_max.x || cursor.y > vp_max.y {
        return;
    }

    let Some((camera, cam_xform)) = camera_q.iter().next() else {
        return;
    };
    let viewport_pos = Vec2::new(
        (cursor.x - vp_min.x) / viewport.screen_size.x * viewport.current_size.x as f32,
        (cursor.y - vp_min.y) / viewport.screen_size.y * viewport.current_size.y as f32,
    );
    let Ok(ray) = camera.viewport_to_world(cam_xform, viewport_pos) else {
        return;
    };

    // Entity actions only when there's already a selection; otherwise the
    // Add menu is the "normal" right-click behaviour.
    let has_selection = selection
        .as_deref()
        .map(|s| !s.get_all().is_empty())
        .unwrap_or(false);

    let kind = if has_selection {
        MenuKind::EntityActions
    } else {
        let dir = ray.direction.as_vec3();
        let world_pos = ground_hit(ray.origin, dir).unwrap_or_else(|| {
            let forward = cam_xform.forward().as_vec3();
            let p = cam_xform.translation() + forward * 5.0;
            Vec3::new(p.x, 0.0, p.z)
        });
        MenuKind::AddHere { world_pos }
    };

    menu.open = true;
    menu.screen_pos = cursor;
    menu.kind = kind;
    menu.query.clear();
    menu.just_opened = true;
    menu.open_counter = menu.open_counter.wrapping_add(1);
}

fn ground_hit(origin: Vec3, dir: Vec3) -> Option<Vec3> {
    if dir.y.abs() <= 1e-6 {
        return None;
    }
    let t = -origin.y / dir.y;
    if t <= 0.0 || t > 10_000.0 {
        return None;
    }
    let hit = origin + dir * t;
    Some(Vec3::new(hit.x, 0.0, hit.z))
}

#[cfg(test)]
mod ground_hit_tests {
    use super::ground_hit;
    use bevy::prelude::*;

    /// Where a right-click lands on the ground plane is where "Add Cube" spawns
    /// the cube. Every rejection below is a case where there is no sensible
    /// answer, and returning one anyway would drop an object somewhere the user
    /// did not click.
    #[test]
    fn a_ray_aimed_down_hits_the_ground_below_it() {
        let hit = ground_hit(Vec3::new(3.0, 10.0, -4.0), Vec3::NEG_Y).unwrap();
        assert_eq!(hit, Vec3::new(3.0, 0.0, -4.0));
    }

    #[test]
    fn the_hit_is_always_flattened_onto_the_plane() {
        let hit = ground_hit(Vec3::new(0.0, 5.0, 0.0), Vec3::new(1.0, -1.0, 1.0)).unwrap();
        assert_eq!(hit.y, 0.0);
        // 5 units down means 5 units along x and z too.
        assert!((hit.x - 5.0).abs() < 1e-4, "{hit:?}");
        assert!((hit.z - 5.0).abs() < 1e-4, "{hit:?}");
    }

    /// From below the plane, an upward ray still hits it — the editor camera can
    /// legitimately sit under the ground.
    #[test]
    fn a_ray_from_below_hits_the_plane_going_up() {
        let hit = ground_hit(Vec3::new(1.0, -8.0, 2.0), Vec3::Y).unwrap();
        assert_eq!(hit, Vec3::new(1.0, 0.0, 2.0));
    }

    /// A ray pointing away from the plane has its intersection *behind* the
    /// camera. Without the `t <= 0.0` guard the click would spawn an object
    /// behind the viewer, mirrored through the camera.
    #[test]
    fn a_ray_pointing_away_from_the_plane_misses() {
        assert!(ground_hit(Vec3::new(0.0, 10.0, 0.0), Vec3::Y).is_none());
        assert!(ground_hit(Vec3::new(0.0, -10.0, 0.0), Vec3::NEG_Y).is_none());
    }

    /// A ray parallel to the ground never meets it, and `-origin.y / dir.y`
    /// would be an infinity — which then multiplies into a NaN position.
    #[test]
    fn a_ray_parallel_to_the_ground_misses_rather_than_going_infinite() {
        for dir in [Vec3::X, Vec3::Z, Vec3::new(1.0, 0.0, 1.0), Vec3::new(1.0, 1e-9, 0.0)] {
            assert!(ground_hit(Vec3::new(0.0, 5.0, 0.0), dir).is_none(), "{dir:?}");
        }
    }

    /// A near-parallel ray from a camera far above the plane produces a hit
    /// kilometres away. Spawning there is worse than not spawning: the object is
    /// off-screen and the user has no idea where it went.
    #[test]
    fn an_absurdly_distant_hit_is_rejected() {
        // Grazing angle: 100 units up, dropping 0.001 per unit travelled.
        let far = ground_hit(Vec3::new(0.0, 100.0, 0.0), Vec3::new(1.0, -0.001, 0.0));
        assert!(far.is_none(), "expected the 100km hit to be rejected, got {far:?}");
    }

    #[test]
    fn a_hit_just_inside_the_distance_limit_is_kept() {
        // t == 5000, comfortably inside the 10,000 cap.
        let hit = ground_hit(Vec3::new(0.0, 5000.0, 0.0), Vec3::NEG_Y);
        assert!(hit.is_some());
    }

    /// A camera sitting exactly on the plane is degenerate but reachable, and
    /// `t` is then 0 — the guard rejects it rather than spawning at the camera.
    #[test]
    fn an_origin_already_on_the_plane_is_rejected() {
        assert!(ground_hit(Vec3::new(4.0, 0.0, 4.0), Vec3::NEG_Y).is_none());
    }

    #[test]
    fn a_hit_is_never_non_finite() {
        for origin in [Vec3::new(0.0, 1.0, 0.0), Vec3::new(-500.0, 900.0, 250.0)] {
            for dir in [Vec3::NEG_Y, Vec3::new(0.5, -0.5, 0.5), Vec3::new(-1.0, -2.0, 3.0)] {
                if let Some(hit) = ground_hit(origin, dir) {
                    assert!(hit.is_finite(), "{origin:?} {dir:?} -> {hit:?}");
                }
            }
        }
    }
}

// ── Actions ────────────────────────────────────────────────────────────────
//
// Helpers a native renderer will call once it is wired up. Kept here so the
// spawn / component-add logic survives the egui removal.

#[derive(Clone, Copy)]
pub enum EntityAction {
    Duplicate,
    Delete,
    Focus,
    Deselect,
}

/// Dispatch an entity action through the existing [`KeyBindings`] so rebinds
/// and other consumers still apply.
pub fn dispatch_entity_action(world: &World, action: EntityAction) {
    if let Some(kb) = world.get_resource::<KeyBindings>() {
        match action {
            EntityAction::Focus => kb.dispatch(EditorAction::FocusSelected),
            EntityAction::Duplicate => kb.dispatch(EditorAction::Duplicate),
            EntityAction::Delete => kb.dispatch(EditorAction::Delete),
            EntityAction::Deselect => kb.dispatch(EditorAction::Deselect),
        }
    }
}

/// Invoke the `InspectorEntry::add_fn` for `type_id` on every entity in
/// the current selection.
pub fn add_component_to_selection(world: &mut World, type_id: &'static str) {
    let entities: Vec<Entity> = world
        .get_resource::<EditorSelection>()
        .map(|s| s.get_all())
        .unwrap_or_default();
    if entities.is_empty() {
        return;
    }

    let add_fn = world.get_resource::<InspectorRegistry>().and_then(|r| {
        r.iter()
            .find(|e| e.type_id == type_id)
            .and_then(|e| e.add_fn)
    });
    let Some(add_fn) = add_fn else {
        warn!("[context_menu] '{}' has no add_fn registered", type_id);
        return;
    };

    for entity in entities {
        add_fn(world, entity);
    }
}

pub fn spawn_preset_at(world: &mut World, preset_id: &'static str, position: Vec3) {
    let spawn_fn: Option<fn(&mut World) -> Entity> = world
        .get_resource::<SpawnRegistry>()
        .and_then(|r| r.iter().find(|p| p.id == preset_id).map(|p| p.spawn_fn));
    let Some(spawn_fn) = spawn_fn else { return };

    let entity = spawn_fn(world);
    // Enforce the one-id-per-entity rule: replace whatever display-y name the
    // spawn_fn set with a unique, snake_case id derived from the preset id
    // (`directional_light`, `directional_light_2`, …). The preset id is already
    // clean, so this is really just the dedupe pass.
    let id = renzora::unique_entity_name(world, preset_id, entity);
    if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
        entity_mut.insert(Name::new(id));
        if let Some(mut xform) = entity_mut.get_mut::<Transform>() {
            xform.translation = position;
        }
    }
    if let Some(sel) = world.get_resource::<EditorSelection>() {
        sel.set(Some(entity));
    }
}

renzora::add!(ContextMenuPlugin, Editor);
