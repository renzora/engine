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

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use renzora::bevy_egui::egui::{self, Color32, Pos2, RichText};
use renzora::bevy_egui::{EguiContexts, EguiPrimaryContextPass};
use renzora::core::keybindings::{EditorAction, KeyBindings};
use renzora::core::viewport_types::ViewportState;
use renzora::core::EditorCamera;
use renzora::editor::{ActiveTool, EditorSelection, SpawnRegistry, SplashState};
use renzora::theme::ThemeManager;

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
    /// Incremented each time the menu opens. Mixed into the egui Area id
    /// so each open gets a fresh layout — otherwise egui caches the last
    /// render's rect and constrains the next open to its height (visible
    /// as the Add menu being "stuck" at the smaller Entity menu height).
    pub open_counter: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum MenuKind {
    #[default]
    None,
    /// Spawn a preset at this world position.
    AddHere { world_pos: Vec3 },
    /// Act on the current `EditorSelection` (Duplicate, Delete, Focus,
    /// Deselect). Shown when at least one entity is selected.
    EntityActions,
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
                detect_right_click_tap.run_if(in_state(SplashState::Editor)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                render_context_menu.run_if(in_state(SplashState::Editor)),
            );
    }
}

renzora::add!(ContextMenuPlugin, Editor);

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
                ActiveTool::Select
                    | ActiveTool::Translate
                    | ActiveTool::Rotate
                    | ActiveTool::Scale
            )
        })
        .unwrap_or(true);

    let Ok(window) = window_q.single() else { return };
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
    let Some(viewport) = viewport.as_deref() else { return };
    let vp_min = viewport.screen_position;
    let vp_max = vp_min + viewport.screen_size;
    if cursor.x < vp_min.x || cursor.y < vp_min.y
        || cursor.x > vp_max.x || cursor.y > vp_max.y {
        return;
    }

    let Some((camera, cam_xform)) = camera_q.iter().next() else { return };
    let viewport_pos = Vec2::new(
        (cursor.x - vp_min.x) / viewport.screen_size.x * viewport.current_size.x as f32,
        (cursor.y - vp_min.y) / viewport.screen_size.y * viewport.current_size.y as f32,
    );
    let Ok(ray) = camera.viewport_to_world(cam_xform, viewport_pos) else { return };

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
    if dir.y.abs() <= 1e-6 { return None; }
    let t = -origin.y / dir.y;
    if t <= 0.0 || t > 10_000.0 { return None; }
    let hit = origin + dir * t;
    Some(Vec3::new(hit.x, 0.0, hit.z))
}

// ── Render ─────────────────────────────────────────────────────────────────

fn render_context_menu(world: &mut World) {
    let (open, screen_pos, kind, mut query, just_opened, open_counter) = {
        let s = world.resource::<ContextMenuState>();
        (s.open, s.screen_pos, s.kind, s.query.clone(), s.just_opened, s.open_counter)
    };
    if !open { return; }

    let theme = world.get_resource::<ThemeManager>().map(|m| m.active_theme.clone());
    let ctx = {
        let mut state: bevy::ecs::system::SystemState<EguiContexts> =
            bevy::ecs::system::SystemState::new(world);
        let mut ctxs = state.get_mut(world);
        let Ok(ctx) = ctxs.ctx_mut() else { return };
        ctx.clone()
    };

    let (bg, border, text_primary, text_muted, hover) = theme
        .as_ref()
        .map(|t| (
            t.surfaces.panel.to_color32(),
            t.widgets.border.to_color32(),
            t.text.primary.to_color32(),
            t.text.muted.to_color32(),
            t.widgets.hovered_bg.to_color32(),
        ))
        .unwrap_or((
            Color32::from_rgb(30, 30, 32),
            Color32::from_rgb(80, 80, 88),
            Color32::WHITE,
            Color32::from_gray(160),
            Color32::from_rgb(55, 55, 65),
        ));

    let mut close = false;
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) { close = true; }

    // Action flagged by clicked menu item — applied after the UI closure.
    let mut pending_action: Option<PendingAction> = None;

    let area_resp = egui::Area::new(egui::Id::new(("context_menu", open_counter)))
        .order(egui::Order::Foreground)
        .fixed_pos(Pos2::new(screen_pos.x, screen_pos.y))
        .show(&ctx, |ui| {
            let frame = egui::Frame::new()
                .fill(bg)
                .stroke(egui::Stroke::new(1.0, border))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::same(4));

            frame.show(ui, |ui| {
                ui.set_min_width(200.0);
                ui.set_max_width(200.0);

                // Search bar — type to filter entries. Auto-focused on open.
                let edit = egui::TextEdit::singleline(&mut query)
                    .hint_text("Search…")
                    .text_color(text_primary)
                    .font(egui::FontId::proportional(14.0))
                    .desired_width(f32::INFINITY);
                let edit_resp = ui.add(edit);
                if just_opened || (!edit_resp.has_focus() && query.is_empty()) {
                    edit_resp.request_focus();
                }
                // Pressing Enter in the search field picks the first visible
                // entry — flagged via `enter_pressed` and read in the renderers.
                let enter_pressed = ctx.input(|i| i.key_pressed(egui::Key::Enter));
                ui.add_space(4.0);

                match kind {
                    MenuKind::AddHere { world_pos } => {
                        render_add_menu(
                            ui, world, &query, enter_pressed,
                            text_primary, text_muted, hover,
                            |id| {
                                pending_action = Some(PendingAction::Spawn { id, world_pos });
                            },
                        );
                    }
                    MenuKind::EntityActions => {
                        render_entity_menu(
                            ui, &query, enter_pressed,
                            text_primary, text_muted, hover,
                            |action| {
                                pending_action = Some(PendingAction::Act { action });
                            },
                        );
                    }
                    MenuKind::None => {}
                }
            });
        });

    // Click outside the Area → close.
    if ctx.input(|i| i.pointer.any_pressed()) {
        let pos = ctx.input(|i| i.pointer.interact_pos()).unwrap_or_default();
        if !area_resp.response.rect.contains(pos) {
            close = true;
        }
    }

    if pending_action.is_some() {
        close = true;
    }

    // Commit query back + clear the `just_opened` flag after first render.
    {
        let mut s = world.resource_mut::<ContextMenuState>();
        s.query = query;
        s.just_opened = false;
        if close {
            s.open = false;
        }
    }

    if let Some(action) = pending_action {
        apply_action(world, action);
    }
}

/// Lowercase substring match — empty query matches everything.
fn matches_query(label: &str, query_lower: &str) -> bool {
    query_lower.is_empty() || label.to_lowercase().contains(query_lower)
}

enum PendingAction {
    Spawn { id: &'static str, world_pos: Vec3 },
    Act { action: EntityAction },
}

#[derive(Clone, Copy)]
enum EntityAction {
    Duplicate,
    Delete,
    Focus,
    Deselect,
}

// ── Add menu ───────────────────────────────────────────────────────────────

fn render_add_menu(
    ui: &mut egui::Ui,
    world: &World,
    query: &str,
    enter_pressed: bool,
    text_primary: Color32,
    text_muted: Color32,
    hover: Color32,
    mut on_pick: impl FnMut(&'static str),
) {
    let Some(registry) = world.get_resource::<SpawnRegistry>() else {
        ui.label(RichText::new("No SpawnRegistry").color(text_muted).size(14.0));
        return;
    };

    ui.label(RichText::new("Add").color(text_muted).size(14.0).monospace());
    ui.separator();

    let q = query.to_lowercase();
    let groups = group_presets(registry);
    if groups.is_empty() {
        ui.label(RichText::new("No presets registered").color(text_muted).size(14.0));
        return;
    }

    // Flatten with category filtering — only render categories that still
    // have visible rows after the query filter.
    let mut first_visible: Option<&'static str> = None;
    let mut any_visible = false;

    egui::ScrollArea::vertical()
        .max_height(320.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
        for (cat, entries) in &groups {
            let visible: Vec<&PresetRow> = entries
                .iter()
                .filter(|r| matches_query(r.display_name, &q) || matches_query(cat, &q))
                .collect();
            if visible.is_empty() { continue; }

            any_visible = true;
            if !cat.is_empty() {
                ui.label(RichText::new(*cat).color(text_muted).size(14.0).monospace());
            }
            for row in visible {
                if first_visible.is_none() {
                    first_visible = Some(row.id);
                }
                if menu_row(ui, row.icon, row.display_name, text_primary, hover) {
                    on_pick(row.id);
                }
            }
        }
        if !any_visible {
            ui.label(RichText::new("No matches").color(text_muted).size(14.0));
        }
    });

    if enter_pressed {
        if let Some(id) = first_visible {
            on_pick(id);
        }
    }
}

#[derive(Clone)]
struct PresetRow {
    id: &'static str,
    display_name: &'static str,
    icon: &'static str,
}

fn group_presets(registry: &SpawnRegistry) -> Vec<(&'static str, Vec<PresetRow>)> {
    let mut out: Vec<(&'static str, Vec<PresetRow>)> = Vec::new();
    for preset in registry.iter() {
        let row = PresetRow {
            id: preset.id,
            display_name: preset.display_name,
            icon: preset.icon,
        };
        if let Some(bucket) = out.iter_mut().find(|(c, _)| *c == preset.category) {
            bucket.1.push(row);
        } else {
            out.push((preset.category, vec![row]));
        }
    }
    out
}

// ── Entity menu ────────────────────────────────────────────────────────────

fn render_entity_menu(
    ui: &mut egui::Ui,
    query: &str,
    enter_pressed: bool,
    text_primary: Color32,
    text_muted: Color32,
    hover: Color32,
    mut on_pick: impl FnMut(EntityAction),
) {
    ui.label(RichText::new("Entity").color(text_muted).size(14.0).monospace());
    ui.separator();

    let q = query.to_lowercase();
    let entries: &[(&str, &str, EntityAction)] = &[
        ("\u{E02A}", "Focus (F)", EntityAction::Focus),
        ("\u{E170}", "Duplicate (Ctrl+D)", EntityAction::Duplicate),
        ("\u{E07A}", "Deselect (Esc)", EntityAction::Deselect),
        ("\u{E1A0}", "Delete (Del)", EntityAction::Delete),
    ];

    let visible: Vec<&(&str, &str, EntityAction)> = entries
        .iter()
        .filter(|(_, label, _)| matches_query(label, &q))
        .collect();

    if visible.is_empty() {
        ui.label(RichText::new("No matches").color(text_muted).size(14.0));
        return;
    }

    for (icon, label, action) in &visible {
        if menu_row(ui, icon, label, text_primary, hover) {
            on_pick(*action);
        }
    }

    if enter_pressed {
        if let Some((_, _, action)) = visible.first() {
            on_pick(*action);
        }
    }
}

// ── Shared row widget ──────────────────────────────────────────────────────

fn menu_row(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    text_primary: Color32,
    hover: Color32,
) -> bool {
    // Use the full width the parent `Ui` gave us so rows align with the
    // scrollbar edge instead of being a fixed 170-ish px.
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::Vec2::new(w, 24.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, egui::CornerRadius::same(3), hover);
    }
    ui.painter().text(
        rect.left_center() + egui::Vec2::new(6.0, 0.0),
        egui::Align2::LEFT_CENTER,
        format!("{}  {}", icon, label),
        egui::FontId::proportional(14.0),
        text_primary,
    );
    resp.clicked()
}

// ── Apply ──────────────────────────────────────────────────────────────────

fn apply_action(world: &mut World, action: PendingAction) {
    match action {
        PendingAction::Spawn { id, world_pos } => {
            spawn_preset_at(world, id, world_pos);
        }
        PendingAction::Act { action } => {
            // Operates on the current EditorSelection — dispatch through
            // KeyBindings so consumers react as if the bound key was pressed
            // (rebinds + existing handlers apply).
            if let Some(kb) = world.get_resource::<KeyBindings>() {
                match action {
                    EntityAction::Focus => kb.dispatch(EditorAction::FocusSelected),
                    EntityAction::Duplicate => kb.dispatch(EditorAction::Duplicate),
                    EntityAction::Delete => kb.dispatch(EditorAction::Delete),
                    EntityAction::Deselect => kb.dispatch(EditorAction::Deselect),
                }
            }
        }
    }
}

fn spawn_preset_at(world: &mut World, preset_id: &'static str, position: Vec3) {
    let spawn_fn: Option<fn(&mut World) -> Entity> = world
        .get_resource::<SpawnRegistry>()
        .and_then(|r| r.iter().find(|p| p.id == preset_id).map(|p| p.spawn_fn));
    let Some(spawn_fn) = spawn_fn else { return };

    let entity = spawn_fn(world);
    if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
        if let Some(mut xform) = entity_mut.get_mut::<Transform>() {
            xform.translation = position;
        }
    }
    if let Some(sel) = world.get_resource::<EditorSelection>() {
        sel.set(Some(entity));
    }
}
