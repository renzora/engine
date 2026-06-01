//! `renzora_shell` — the bevy_ui-native editor shell.
//!
//! The editor's layout (menu bar, ribbon, document tabs, dock splits, panel
//! tab-bars, status bar) drawn with **`bevy_ui`** instead of egui. This is the
//! host half of the egui → bevy_ui/HUI migration: the [`dock`] data model is
//! reused 1:1 from the egui editor; only the renderer changes from egui
//! immediate-mode to a bevy_ui reconcile.
//!
//! ## Coexistence during the migration
//! The shell is additive. [`renzora::EditorUiBackend`] selects which editor
//! renders — the legacy egui editor (`Egui`, default) or this shell (`BevyUi`)
//! — and the two are mutually exclusive (the egui `editor_ui_system` is gated
//! off when the backend is `BevyUi`). Press **F10** in the editor to toggle, so
//! the editor stays fully usable while the shell is built out panel by panel.
//!
//! ## Status
//! Phase 1: static render of the Scene layout + chrome, with **placeholder**
//! content per panel (panel title centered). No resize/tab-drag yet, no real
//! panel content. Colors are a local dark palette; theme unification with a
//! bevy-native theme comes later (the current `renzora_theme` is egui-coupled).

use bevy::prelude::*;

use renzora::EditorUiBackend;

pub mod dock;

use dock::{DockTree, DropZone, SplitDirection};

#[derive(Default)]
pub struct ShellPlugin;

impl Plugin for ShellPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ShellPlugin (bevy_ui editor shell)");
        app.init_resource::<EditorUiBackend>();
        let layouts = dock::workspace_layouts();
        app.insert_resource(ShellDock {
            tree: layouts[0].1.clone(),
        });
        app.insert_resource(ShellLayouts { layouts, active: 0 });
        app.init_resource::<renzora::ShellPanelRegistry>();
        app.init_resource::<DraggedDivider>();
        app.init_resource::<TabDrag>();
        app.init_resource::<DockDirty>();
        app.init_resource::<PendingSwitch>();
        app.add_systems(Startup, load_shell_assets);
        app.add_systems(
            Update,
            (
                toggle_backend,
                manage_shell_root,
                divider_drag,
                tab_drag,
                apply_tab_switch,
                apply_panel_meta,
                tab_hover,
                tab_close_hover,
                ribbon_switch,
                rebuild_dock,
            ),
        );
    }
}

renzora::add!(ShellPlugin, Editor);

/// The live dock layout. Divider drags write ratios back here so they persist
/// across shell rebuilds (e.g. F10 toggles) and layout switching.
#[derive(Resource)]
struct ShellDock {
    tree: DockTree,
}

/// The ribbon's workspace layouts and which one is active. Switching saves the
/// current dock tree back into the active slot (so per-layout edits persist)
/// and loads the chosen one into [`ShellDock`].
#[derive(Resource)]
struct ShellLayouts {
    layouts: Vec<(String, DockTree)>,
    active: usize,
}

/// A ribbon workspace button (Scene, Blueprints, …). Carries its layout index
/// and the entities to restyle when the active layout changes.
#[derive(Component)]
struct RibbonItem {
    index: usize,
    text: Entity,
    underline: Entity,
}

/// The active divider drag (latched on press, cleared on release) so dragging
/// continues even off the handle. Delta-based: the seam moves relative to where
/// it was grabbed, so a click never snaps it to the cursor.
#[derive(Resource, Default)]
struct DraggedDivider(Option<DividerDrag>);

struct DividerDrag {
    handle: Entity,
    start_cursor: Vec2,
    start_ratio: f32,
}

/// Tags a split's divider with everything its drag needs: the split container
/// (whose size converts a cursor delta into a ratio), the first child wrapper
/// to resize, the axis, and the tree path to persist the ratio. The same
/// component also drives the tab-bar resize handle (see [`ParentSplit`]).
#[derive(Component)]
struct Divider {
    container: Entity,
    first_wrap: Entity,
    horizontal: bool,
    path: Vec<bool>,
}

/// Passed to a leaf so its tab-bar empty area can act as a secondary resize
/// handle for the parent split's boundary — egui-style "more to grip". Carries
/// the same data the parent's [`Divider`] uses.
#[derive(Clone)]
struct ParentSplit {
    container: Entity,
    first_wrap: Entity,
    horizontal: bool,
    is_second: bool,
    path: Vec<bool>,
}

impl ParentSplit {
    /// The tab bar borders the resize boundary only for a vertical parent's
    /// bottom child (tab bar at the top edge) or a horizontal parent's left
    /// child (empty area at the right edge).
    fn aligned(&self) -> bool {
        (!self.horizontal && self.is_second) || (self.horizontal && !self.is_second)
    }
}

/// Marks the dock-area node so the dock subtree can be rebuilt in place after a
/// layout change (tab switch / re-dock).
#[derive(Component)]
struct DockArea;

/// A draggable panel tab. Click switches the active tab (in place); drag
/// re-docks. Holds its leaf + child entities so a switch can restyle without
/// rebuilding (which would blank the Phosphor icons for a frame).
#[derive(Component)]
struct DockTab {
    id: String,
    leaf: Entity,
    label: Entity,
    icon: Entity,
    /// The vertical insertion marker shown at this tab's edge during a drag.
    marker: Entity,
}

/// The vertical line shown between tabs to mark a drop insertion point.
#[derive(Component)]
struct InsertMarker;

/// A tab's close button — turns red on hover (via its `RelativeCursorPosition`,
/// so it doesn't steal the tab's hover state).
#[derive(Component)]
struct TabClose;

/// A dock leaf — the drop target for tab drags. `tabs` is its panel ids,
/// `overlay` its hidden drop-zone highlight, `content_text` the placeholder
/// text entity to swap on a tab switch.
#[derive(Component)]
struct DockLeaf {
    tabs: Vec<String>,
    overlay: Entity,
    content_text: Entity,
}

/// The translucent drop-zone highlight inside each leaf (hidden until a drag).
#[derive(Component)]
struct DropOverlay;

/// Links a leaf's tab-bar back to its leaf — dropping a tab on the bar adds it
/// as a tab (rather than splitting the top edge).
#[derive(Component)]
struct TabBarOf(Entity);

/// The floating preview that follows the cursor while a tab is dragged.
#[derive(Component)]
struct TabGhost;

/// An in-progress tab drag.
#[derive(Resource, Default)]
struct TabDrag(Option<TabDragState>);

struct TabDragState {
    /// Panel id being dragged.
    id: String,
    /// The leaf the dragged tab belongs to (captured at press).
    leaf: Entity,
    start_cursor: Vec2,
    /// True once the cursor has moved past the drag threshold (else it's a click).
    active: bool,
    /// What a release would do (split a leaf, or insert as a tab).
    action: Option<DropAction>,
    /// The floating preview entity (spawned once the drag activates).
    ghost: Option<Entity>,
    /// Currently-shown drop overlay / insertion marker, so they can be hidden
    /// when the target changes (avoids iterating every overlay each frame).
    shown_overlay: Option<Entity>,
    shown_marker: Option<Entity>,
}

/// What dropping the dragged tab will do.
enum DropAction {
    /// Split the leaf containing `rep` on the given side.
    Split { rep: String, zone: DropZone },
    /// Add the tab to `rep`'s leaf, before panel `before` (or at the end).
    Tab { rep: String, before: Option<String> },
}

/// Set when the dock tree *structure* changes (a re-dock) so [`rebuild_dock`]
/// rebuilds the subtree. Tab switches do NOT set this — they update in place.
#[derive(Resource, Default)]
struct DockDirty(bool);

/// A pending in-place tab switch: (leaf entity, panel id to activate).
#[derive(Resource, Default)]
struct PendingSwitch(Option<(Entity, String)>);

/// Fonts/handles the shell renders with. Loaded once at startup.
#[derive(Resource)]
struct ShellAssets {
    /// Proportional UI font — matches the egui editor's default (Noto Sans).
    ui_font: Handle<Font>,
}

/// Noto Sans, embedded so the font is available regardless of the running
/// editor's asset-root (which is the open *project's* folder, not the engine's
/// `assets/`). Mirrors how the egui editor and `renzora_hui` embed their fonts.
const NOTO_SANS: &[u8] = include_bytes!("../embedded/NotoSans-Regular.ttf");

fn load_shell_assets(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    let ui_font = match bevy::text::Font::try_from_bytes(NOTO_SANS.to_vec()) {
        Ok(font) => fonts.add(font),
        Err(e) => {
            error!("[shell] failed to load embedded Noto Sans: {e:?}");
            Handle::default()
        }
    };
    commands.insert_resource(ShellAssets { ui_font });
}

/// Global nudge so shell text matches the egui editor's slightly smaller text.
const TEXT_SCALE: f32 = 0.92;

/// A `TextFont` in the shell's UI font at the given (pre-scale) size.
fn ui_font(font: &Handle<Font>, size: f32) -> TextFont {
    TextFont {
        font: font.clone(),
        font_size: size * TEXT_SCALE,
        ..default()
    }
}

// ── Palette — the default theme colors from `renzora_theme` (`*::default()`).
// Hardcoded as bevy Colors so the shell stays egui-free; a bevy-native theme
// resource will replace these constants later, but the VALUES are the source of
// truth from `renzora_theme::{SurfaceColors, PanelColors, TextColors, ...}`.

// Calibrated to the egui editor's *appearance* (eyedropped against the swatch
// ladder). egui lifts the dark end, so these run a touch brighter than the raw
// `renzora_theme` bytes (window 11 / panel 26 / …); the blue undertone is kept.
const WINDOW_BG: (u8, u8, u8) = (24, 24, 30); // chrome (top bar/doc tabs/status/dock gaps)
const PANEL_BG: (u8, u8, u8) = (33, 33, 39); // leaf content (lighter than chrome)
const HEADER_BG: (u8, u8, u8) = (37, 37, 44); // doc tabs + panel tab headers
const TAB_ACTIVE_BG: (u8, u8, u8) = (50, 50, 62); // active tab
const TAB_HOVER_BG: (u8, u8, u8) = (42, 42, 52); // hovered (inactive) tab
const CLOSE_RED: (u8, u8, u8) = (216, 84, 84); // close × on hover
const DIVIDER: (u8, u8, u8) = (14, 14, 20); // split dividers (not black)
const TEXT_PRIMARY: (u8, u8, u8) = (230, 230, 240); // text.primary
const TEXT_MUTED: (u8, u8, u8) = (148, 148, 160); // text.muted
const PLACEHOLDER: (u8, u8, u8) = (110, 110, 122); // dim placeholder
const PLAY_GREEN: (u8, u8, u8) = (89, 191, 115); // semantic.success
const ACCENT_BLUE: (u8, u8, u8) = (80, 140, 255); // active ribbon underline

fn rgb((r, g, b): (u8, u8, u8)) -> Color {
    Color::srgb_u8(r, g, b)
}

/// Marks the shell's root UI entity so it can be despawned when the backend
/// switches back to egui.
#[derive(Component)]
struct ShellRoot;

// ── Systems ─────────────────────────────────────────────────────────────────

/// F10 flips the active editor UI backend between the legacy egui editor and
/// the bevy_ui shell.
fn toggle_backend(keys: Res<ButtonInput<KeyCode>>, mut backend: ResMut<EditorUiBackend>) {
    if keys.just_pressed(KeyCode::F10) {
        *backend = match *backend {
            EditorUiBackend::Egui => EditorUiBackend::BevyUi,
            EditorUiBackend::BevyUi => EditorUiBackend::Egui,
        };
        info!("[shell] editor UI backend -> {:?}", *backend);
    }
}

/// Spawn the shell when the backend is `BevyUi`; tear it down when it isn't.
fn manage_shell_root(
    mut commands: Commands,
    backend: Res<EditorUiBackend>,
    assets: Option<Res<ShellAssets>>,
    phosphor: Option<Res<renzora_hui::icons::PhosphorFont>>,
    dock: Res<ShellDock>,
    roots: Query<Entity, With<ShellRoot>>,
) {
    let want = backend.is_bevy_ui();
    let have = !roots.is_empty();
    if want && !have {
        // Wait for both fonts before building: Noto (text) so it doesn't flash
        // the fallback, and Phosphor (icons) so dock icons resolve immediately
        // — see `icon_text`.
        let (Some(assets), Some(phosphor)) = (assets, phosphor) else {
            return;
        };
        spawn_shell(&mut commands, &assets.ui_font, &phosphor.0, &dock.tree);
    } else if !want && have {
        for e in &roots {
            commands.entity(e).despawn();
        }
    }
}

/// A `Val::Percent(p)` as a 0..1 ratio.
fn as_ratio(v: Val) -> Option<f32> {
    if let Val::Percent(p) = v {
        Some(p / 100.0)
    } else {
        None
    }
}

/// Drag a split's divider handle to resize it. Latches the handle under the
/// cursor on press (so the drag continues off it), then moves the seam by the
/// cursor *delta* relative to the grab point — no snap-to-cursor jump. Resizes
/// the first child + the handle live and persists the ratio in [`ShellDock`].
fn divider_drag(
    mut dragged: ResMut<DraggedDivider>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    dividers: Query<(Entity, &Interaction, &Divider)>,
    computed: Query<&bevy::ui::ComputedNode>,
    mut nodes: Query<&mut Node>,
    mut dock: ResMut<ShellDock>,
) {
    if mouse.just_released(MouseButton::Left) {
        dragged.0 = None;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };

    // Latch the divider handle under the cursor on press.
    if dragged.0.is_none() {
        if !mouse.just_pressed(MouseButton::Left) {
            return;
        }
        for (entity, interaction, div) in &dividers {
            if *interaction == Interaction::Pressed {
                let start_ratio = nodes
                    .get(div.first_wrap)
                    .ok()
                    .and_then(|n| as_ratio(if div.horizontal { n.width } else { n.height }))
                    .unwrap_or(0.5);
                dragged.0 = Some(DividerDrag {
                    handle: entity,
                    start_cursor: cursor,
                    start_ratio,
                });
                break;
            }
        }
    }

    let Some(drag) = dragged.0.as_ref() else {
        return;
    };
    let Ok((_, _, div)) = dividers.get(drag.handle) else {
        dragged.0 = None;
        return;
    };
    let Ok(cn) = computed.get(div.container) else {
        return;
    };
    // Split extent in logical px (ComputedNode size is physical).
    let extent = if div.horizontal {
        cn.size().x
    } else {
        cn.size().y
    } * cn.inverse_scale_factor();
    if extent <= 0.0 {
        return;
    }
    let moved = if div.horizontal {
        cursor.x - drag.start_cursor.x
    } else {
        cursor.y - drag.start_cursor.y
    };
    let ratio = (drag.start_ratio + moved / extent).clamp(0.1, 0.9);

    let first_wrap = div.first_wrap;
    let handle = drag.handle;
    let horizontal = div.horizontal;
    let dpath = div.path.clone();

    // Resize the first child + move the grab handle so it stays on the seam.
    if let Ok(mut n) = nodes.get_mut(first_wrap) {
        if horizontal {
            n.width = Val::Percent(ratio * 100.0);
        } else {
            n.height = Val::Percent(ratio * 100.0);
        }
    }
    if let Ok(mut n) = nodes.get_mut(handle) {
        if horizontal {
            n.left = Val::Percent(ratio * 100.0);
        } else {
            n.top = Val::Percent(ratio * 100.0);
        }
    }
    dock.tree.update_ratio(&dpath, ratio);
}

/// Minimum cursor travel before a tab press becomes a drag (vs a click).
const TAB_DRAG_THRESHOLD: f32 = 6.0;

/// Drag a panel tab to re-dock it; a plain click switches the active tab.
/// While dragging past the threshold, the leaf under the cursor shows a
/// drop-zone highlight (center = tab into it, edges = split). On release the
/// dock tree is mutated and [`rebuild_dock`] rebuilds the subtree.
#[allow(clippy::too_many_arguments)]
fn tab_drag(
    mut drag: ResMut<TabDrag>,
    mut dirty: ResMut<DockDirty>,
    mut pending: ResMut<PendingSwitch>,
    mut commands: Commands,
    assets: Option<Res<ShellAssets>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    tabs: Query<(&Interaction, &DockTab, &bevy::ui::RelativeCursorPosition)>,
    tabbars: Query<(&TabBarOf, &bevy::ui::RelativeCursorPosition)>,
    leaves: Query<(Entity, &DockLeaf, &bevy::ui::RelativeCursorPosition)>,
    mut nodes: Query<&mut Node>,
    mut dock: ResMut<ShellDock>,
) {
    let cursor = windows.single().ok().and_then(|w| w.cursor_position());

    // Latch a tab on press.
    if drag.0.is_none() && mouse.just_pressed(MouseButton::Left) {
        if let Some(cur) = cursor {
            for (interaction, tab, _) in &tabs {
                if *interaction == Interaction::Pressed {
                    drag.0 = Some(TabDragState {
                        id: tab.id.clone(),
                        leaf: tab.leaf,
                        start_cursor: cur,
                        active: false,
                        action: None,
                        ghost: None,
                        shown_overlay: None,
                        shown_marker: None,
                    });
                    break;
                }
            }
        }
    }

    // Release → apply (drag) or switch tab (click).
    if mouse.just_released(MouseButton::Left) {
        if let Some(state) = drag.0.take() {
            if let Some(ghost) = state.ghost {
                commands.entity(ghost).despawn();
            }
            for e in [state.shown_overlay, state.shown_marker].into_iter().flatten() {
                if let Ok(mut n) = nodes.get_mut(e) {
                    n.display = Display::None;
                }
            }
            if state.active {
                if let Some(action) = state.action {
                    apply_action(&mut dock.tree, &state.id, action);
                    dirty.0 = true;
                }
            } else {
                // A plain click — switch the active tab in place (no rebuild).
                pending.0 = Some((state.leaf, state.id));
            }
        }
        return;
    }

    let Some(cur) = cursor else {
        return;
    };
    let Some(state) = drag.0.as_mut() else {
        return;
    };
    if !state.active {
        if cur.distance(state.start_cursor) > TAB_DRAG_THRESHOLD {
            state.active = true;
            if let Some(assets) = &assets {
                state.ghost = Some(spawn_ghost(&mut commands, &assets.ui_font, &state.id, cur));
            }
        } else {
            return;
        }
    }

    // Ghost follows the cursor.
    if let Some(ghost) = state.ghost {
        if let Ok(mut n) = nodes.get_mut(ghost) {
            n.left = Val::Px(cur.x + 12.0);
            n.top = Val::Px(cur.y + 12.0);
        }
    }

    // Decide what a drop here does + what to show. A tab-bar means precise tab
    // insertion (a vertical marker between tabs); a leaf body means split (edge)
    // or add-as-tab (center), shown as an outline.
    let mut action: Option<DropAction> = None;
    let mut new_overlay: Option<(Entity, DropZone)> = None;
    let mut new_marker: Option<(Entity, bool)> = None; // (marker, right-anchored)

    let over_bar = tabbars
        .iter()
        .find(|(_, rcp)| rcp.cursor_over)
        .map(|(bar, _)| bar.0);
    if let Some(leaf_ent) = over_bar {
        if let Some((_, ld, _)) = leaves.iter().find(|(e, _, _)| *e == leaf_ent) {
            // egui-style: insert before the first tab whose centre is right of
            // the cursor. `normalized.x < 0` means the cursor is left of that
            // tab's centre, and it's computed for every tab (gaps included), so
            // there's no dead zone between tabs.
            let mut before: Option<String> = None;
            let mut marker: Option<(Entity, bool)> = None;
            for id in ld.tabs.iter() {
                if let Some((_, tab, rcp)) = tabs.iter().find(|(_, t, _)| &t.id == id) {
                    let nx = rcp.normalized.map_or(f32::INFINITY, |n| n.x);
                    if nx < 0.0 {
                        before = Some(id.clone());
                        marker = Some((tab.marker, false));
                        break;
                    }
                }
            }
            // Cursor past every tab centre → insert at the end.
            if marker.is_none() {
                if let Some(last) = ld.tabs.last() {
                    marker = tabs
                        .iter()
                        .find(|(_, t, _)| &t.id == last)
                        .map(|(_, t, _)| (t.marker, true));
                }
                before = None;
            }
            if let Some(rep) = ld.tabs.iter().find(|t| **t != state.id).cloned() {
                action = Some(DropAction::Tab { rep, before });
                new_marker = marker;
            }
        }
    } else {
        for (_, ld, rcp) in &leaves {
            if rcp.cursor_over {
                if let Some(norm) = rcp.normalized {
                    // Bevy's `normalized` is centered: (-0.5,-0.5) top-left,
                    // (0.5,0.5) bottom-right. Shift to 0..1 for the zone math.
                    let (x, y) = (norm.x + 0.5, norm.y + 0.5);
                    let zone = pick_zone(x, y);
                    if let Some(rep) = ld.tabs.iter().find(|t| **t != state.id).cloned() {
                        action = Some(if matches!(zone, DropZone::Center) {
                            DropAction::Tab { rep, before: None }
                        } else {
                            DropAction::Split { rep, zone }
                        });
                        new_overlay = Some((ld.overlay, zone));
                    }
                }
                break;
            }
        }
    }
    state.action = action;

    // Reconcile the shown overlay/marker (hide the previous when it changes).
    let new_overlay_e = new_overlay.map(|(e, _)| e);
    if state.shown_overlay != new_overlay_e {
        if let Some(old) = state.shown_overlay {
            if let Ok(mut n) = nodes.get_mut(old) {
                n.display = Display::None;
            }
        }
        state.shown_overlay = new_overlay_e;
    }
    if let Some((e, zone)) = new_overlay {
        if let Ok(mut n) = nodes.get_mut(e) {
            n.display = Display::Flex;
            set_zone_rect(&mut n, zone);
        }
    }

    let new_marker_e = new_marker.map(|(e, _)| e);
    if state.shown_marker != new_marker_e {
        if let Some(old) = state.shown_marker {
            if let Ok(mut n) = nodes.get_mut(old) {
                n.display = Display::None;
            }
        }
        state.shown_marker = new_marker_e;
    }
    if let Some((e, right)) = new_marker {
        if let Ok(mut n) = nodes.get_mut(e) {
            n.display = Display::Flex;
            if right {
                n.left = Val::Auto;
                n.right = Val::Px(-2.0);
            } else {
                n.left = Val::Px(-2.0);
                n.right = Val::Auto;
            }
        }
    }
}

/// Spawn the floating drag preview (icon + title) at the cursor.
fn spawn_ghost(commands: &mut Commands, font: &Handle<Font>, id: &str, cursor: Vec2) -> Entity {
    let (title, icon) = panel_meta(id);
    let ghost = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(cursor.x + 12.0),
                top: Val::Px(cursor.y + 12.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(TAB_ACTIVE_BG)),
            bevy::ui::GlobalZIndex(1000),
            bevy::ui::FocusPolicy::Pass,
            TabGhost,
            renzora::HideInHierarchy,
            Name::new("tab-ghost"),
        ))
        .id();
    let gi = glyph(commands, icon, TEXT_PRIMARY, 13.0);
    let gl = commands
        .spawn((
            Text::new(title),
            ui_font(font, 12.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    commands.entity(ghost).add_children(&[gi, gl]);
    ghost
}

/// Pick a drop zone from the cursor's 0..1 position within a leaf.
fn pick_zone(x: f32, y: f32) -> DropZone {
    // Matches the egui dock's EDGE_FRACTION (outer 25% of each side splits).
    const EDGE: f32 = 0.25;
    if x < EDGE {
        DropZone::Left
    } else if x > 1.0 - EDGE {
        DropZone::Right
    } else if y < EDGE {
        DropZone::Top
    } else if y > 1.0 - EDGE {
        DropZone::Bottom
    } else {
        DropZone::Center
    }
}

/// Position a leaf's overlay over the given drop zone (percentages of the leaf).
fn set_zone_rect(n: &mut Node, zone: DropZone) {
    let (l, t, w, h) = match zone {
        DropZone::Center => (0.0, 0.0, 100.0, 100.0),
        DropZone::Left => (0.0, 0.0, 50.0, 100.0),
        DropZone::Right => (50.0, 0.0, 50.0, 100.0),
        DropZone::Top => (0.0, 0.0, 100.0, 50.0),
        DropZone::Bottom => (0.0, 50.0, 100.0, 50.0),
    };
    n.left = Val::Percent(l);
    n.top = Val::Percent(t);
    n.width = Val::Percent(w);
    n.height = Val::Percent(h);
}

/// Apply a tab drop to the dock tree.
fn apply_action(tree: &mut DockTree, dragged: &str, action: DropAction) {
    match action {
        DropAction::Split { rep, zone } => {
            if rep == dragged {
                return;
            }
            tree.remove_panel(dragged);
            tree.split_at(&rep, dragged.to_string(), zone);
        }
        DropAction::Tab { rep, before } => {
            tree.remove_panel(dragged);
            tree.add_tab_before(&rep, dragged.to_string(), before.as_deref());
        }
    }
}

/// Rebuild the dock subtree in place after a layout change. Despawns the old
/// tree under the dock area and rebuilds it from [`ShellDock`].
fn rebuild_dock(
    mut dirty: ResMut<DockDirty>,
    mut commands: Commands,
    assets: Option<Res<ShellAssets>>,
    phosphor: Option<Res<renzora_hui::icons::PhosphorFont>>,
    dock: Res<ShellDock>,
    area: Query<(Entity, Option<&Children>), With<DockArea>>,
) {
    if !dirty.0 {
        return;
    }
    let (Some(assets), Some(phosphor)) = (assets, phosphor) else {
        return;
    };
    let Ok((area_entity, children)) = area.single() else {
        return;
    };
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    let tree_root = build_tree(
        &mut commands,
        &assets.ui_font,
        &phosphor.0,
        None,
        Vec::new(),
        &dock.tree,
    );
    commands.entity(area_entity).add_child(tree_root);
    dirty.0 = false;
}

/// Apply a pending tab switch in place: restyle the leaf's tabs (background,
/// label + icon color, close-button visibility) and swap the content text —
/// no entity churn, so the Phosphor icons never blank out.
fn apply_tab_switch(
    mut pending: ResMut<PendingSwitch>,
    mut dock: ResMut<ShellDock>,
    tabs: Query<(Entity, &DockTab)>,
    leaves: Query<&DockLeaf>,
    mut colors: Query<&mut TextColor>,
    mut texts: Query<&mut Text>,
) {
    let Some((leaf, id)) = pending.0.take() else {
        return;
    };
    dock.tree.set_active_tab(&id);

    // Tab backgrounds are owned by `tab_hover`; here we just recolor the
    // label/icon and swap the content text.
    for (_tab_entity, tab) in &tabs {
        if tab.leaf != leaf {
            continue;
        }
        let fg = rgb(if tab.id == id { TEXT_PRIMARY } else { TEXT_MUTED });
        if let Ok(mut c) = colors.get_mut(tab.label) {
            c.0 = fg;
        }
        if let Ok(mut c) = colors.get_mut(tab.icon) {
            c.0 = fg;
        }
    }

    if let Ok(leaf_data) = leaves.get(leaf) {
        if let Ok(mut text) = texts.get_mut(leaf_data.content_text) {
            *text = Text::new(panel_meta(&id).0);
        }
    }
}

/// Apply real panel titles/icons from [`renzora::ShellPanelRegistry`] onto the
/// tabs (overriding the build-time `panel_meta` placeholders). Runs cheaply
/// each frame, guarded so it only writes when a value actually differs.
fn apply_panel_meta(
    registry: Res<renzora::ShellPanelRegistry>,
    tabs: Query<&DockTab>,
    mut texts: Query<&mut Text>,
) {
    if registry.panels.is_empty() {
        return;
    }
    for tab in &tabs {
        let Some(info) = registry.panels.get(&tab.id) else {
            continue;
        };
        if !info.title.is_empty() {
            if let Ok(mut t) = texts.get_mut(tab.label) {
                if t.0 != info.title {
                    t.0 = info.title.clone();
                }
            }
        }
        if !info.icon.is_empty() {
            if let Ok(mut t) = texts.get_mut(tab.icon) {
                if t.0 != info.icon {
                    t.0 = info.icon.clone();
                }
            }
        }
    }
}

/// Tab background follows hover + active state (active wins). Owns the tab
/// background so `apply_tab_switch` doesn't have to.
fn tab_hover(dock: Res<ShellDock>, mut tabs: Query<(&Interaction, &DockTab, &mut BackgroundColor)>) {
    for (interaction, tab, mut bg) in &mut tabs {
        let target = if dock.tree.is_active_tab(&tab.id) {
            rgb(TAB_ACTIVE_BG)
        } else if matches!(interaction, Interaction::Hovered | Interaction::Pressed) {
            rgb(TAB_HOVER_BG)
        } else {
            Color::NONE
        };
        if bg.0 != target {
            bg.0 = target;
        }
    }
}

/// A tab's close × reddens while the cursor is over it. Uses
/// `RelativeCursorPosition` (geometric) so it doesn't steal the tab's hover.
fn tab_close_hover(
    mut closes: Query<(&bevy::ui::RelativeCursorPosition, &mut TextColor), With<TabClose>>,
) {
    for (rcp, mut color) in &mut closes {
        let target = rgb(if rcp.cursor_over { CLOSE_RED } else { TEXT_MUTED });
        if color.0 != target {
            color.0 = target;
        }
    }
}

/// Clicking a ribbon workspace button switches the dock layout. The current
/// dock is saved back into its slot first (so per-layout edits persist), the
/// chosen layout is loaded, the dock is rebuilt, and the ribbon is restyled.
fn ribbon_switch(
    triggers: Query<(&RibbonItem, &Interaction), Changed<Interaction>>,
    items: Query<&RibbonItem>,
    mut layouts: ResMut<ShellLayouts>,
    mut dock: ResMut<ShellDock>,
    mut dirty: ResMut<DockDirty>,
    mut backgrounds: Query<&mut BackgroundColor>,
    mut colors: Query<&mut TextColor>,
) {
    let mut switch_to = None;
    for (item, interaction) in &triggers {
        if *interaction == Interaction::Pressed {
            switch_to = Some(item.index);
            break;
        }
    }
    let Some(index) = switch_to else {
        return;
    };
    if index == layouts.active || index >= layouts.layouts.len() {
        return;
    }

    // Save the current dock into the active slot, then load the chosen layout.
    let active = layouts.active;
    if let Some(slot) = layouts.layouts.get_mut(active) {
        slot.1 = dock.tree.clone();
    }
    dock.tree = layouts.layouts[index].1.clone();
    layouts.active = index;
    dirty.0 = true;

    // Restyle the ribbon — active item gets primary text + the blue underline.
    for item in &items {
        let is_active = item.index == index;
        if let Ok(mut c) = colors.get_mut(item.text) {
            c.0 = rgb(if is_active { TEXT_PRIMARY } else { TEXT_MUTED });
        }
        if let Ok(mut b) = backgrounds.get_mut(item.underline) {
            b.0 = if is_active {
                rgb(ACCENT_BLUE)
            } else {
                Color::NONE
            };
        }
    }
}

// ── Build the shell UI tree ─────────────────────────────────────────────────

fn spawn_shell(
    commands: &mut Commands,
    font: &Handle<Font>,
    phosphor: &Handle<Font>,
    tree: &DockTree,
) {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(rgb(WINDOW_BG)),
            ShellRoot,
            renzora::HideInHierarchy,
            Name::new("Renzora Shell"),
        ))
        .id();

    let top_bar = build_top_bar(commands, font);
    let doctabs = build_doc_tabs(commands, font);

    // Dock area fills the remaining vertical space.
    let dock_area = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                overflow: Overflow::clip(),
                ..default()
            },
            DockArea,
            Name::new("dock-area"),
        ))
        .id();
    let tree_root = build_tree(commands, font, phosphor, None, Vec::new(), tree);
    commands.entity(dock_area).add_child(tree_root);

    let statusbar = chrome_row(
        commands,
        font,
        "status-bar",
        22.0,
        rgb(WINDOW_BG),
        16.0,
        10.0,
        &[
            ("Ready", false),
            ("Dark", false),
            ("Vulkan", false),
            ("60 FPS", false),
        ],
    );

    commands
        .entity(root)
        .add_children(&[top_bar, doctabs, dock_area, statusbar]);
}

/// The top bar: File/Edit/View/Help on the left, the layout ribbon centered,
/// and action buttons (play, code, settings, sign-in, window controls) on the
/// right. The center group stays centered because the left and right zones
/// both grow equally.
fn build_top_bar(commands: &mut Commands, font: &Handle<Font>) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(34.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(8.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(WINDOW_BG)),
            Name::new("top-bar"),
        ))
        .id();

    // Left: application menus. Small gap so File/Edit/View/Help sit close
    // together like the egui menu bar.
    let left = zone(commands, "top-left", JustifyContent::FlexStart, 2.0, 1.0);
    let left_kids = text_items(
        commands,
        font,
        &[("File", false), ("Edit", false), ("View", false), ("Help", false)],
        14.0,
    );
    commands.entity(left).add_children(&left_kids);

    // Center: the layout ribbon (workspace switcher). `grow: 0` keeps it
    // content-sized so it sits centered between the two growing side zones.
    let center = zone(commands, "top-center", JustifyContent::Center, 2.0, 0.0);
    let mut center_kids = vec![glyph(commands, "magnifying-glass", TEXT_MUTED, 14.0)];
    // Workspace switcher buttons, in the same order as `dock::workspace_layouts`.
    for (i, label) in [
        "Scene",
        "Blueprints",
        "Scripting",
        "Animation",
        "Materials",
        "Particles",
        "Video",
        "Audio",
        "Debug",
    ]
    .into_iter()
    .enumerate()
    {
        center_kids.push(ribbon_item(commands, font, label, i, i == 0));
    }
    // "+" — add workspace (not a switcher; wired later).
    center_kids.push(text_item(commands, font, "+", TEXT_MUTED, 12.0));
    commands.entity(center).add_children(&center_kids);

    // Right: action buttons, account group, then window controls.
    let right = zone(commands, "top-right", JustifyContent::FlexEnd, 8.0, 1.0);
    let play = icon_item(commands, "play", PLAY_GREEN, 16.0);
    let code = icon_item(commands, "code", TEXT_MUTED, 16.0);
    let settings = icon_item(commands, "gear", TEXT_MUTED, 16.0);

    // Account: user icon + "Sign In", kept tight together.
    let account = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            },
            Name::new("account"),
        ))
        .id();
    let user = glyph(commands, "user", TEXT_MUTED, 14.0);
    let sign_in = commands
        .spawn((
            Text::new("Sign In"),
            ui_font(font, 12.0),
            TextColor(rgb(TEXT_MUTED)),
        ))
        .id();
    commands.entity(account).add_children(&[user, sign_in]);

    // Window controls, spaced away from the account group.
    let window = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                margin: UiRect::left(Val::Px(14.0)),
                ..default()
            },
            Name::new("window-buttons"),
        ))
        .id();
    let min = icon_item(commands, "minus", TEXT_MUTED, 14.0);
    let max = icon_item(commands, "square", TEXT_MUTED, 13.0);
    let close = icon_item(commands, "x", TEXT_MUTED, 14.0);
    commands.entity(window).add_children(&[min, max, close]);

    commands
        .entity(right)
        .add_children(&[play, code, settings, account, window]);

    commands.entity(bar).add_children(&[left, center, right]);
    bar
}

/// A top-bar ribbon entry (workspace switcher). Full height so the active
/// item's blue underline pins to the bottom edge of the top bar. Clicking it
/// switches to workspace `index` (see [`ribbon_switch`]).
fn ribbon_item(
    commands: &mut Commands,
    font: &Handle<Font>,
    label: &str,
    index: usize,
    active: bool,
) -> Entity {
    let item = commands
        .spawn((
            Node {
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            Interaction::default(),
            renzora_hui::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new(format!("ribbon:{label}")),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label),
            ui_font(font, 12.0),
            TextColor(rgb(if active { TEXT_PRIMARY } else { TEXT_MUTED })),
        ))
        .id();
    let text_wrap = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(Val::Px(7.0)),
                ..default()
            },
            Name::new("ribbon-label"),
        ))
        .id();
    commands.entity(text_wrap).add_child(text);
    let underline = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(if active { rgb(ACCENT_BLUE) } else { Color::NONE }),
            Name::new("ribbon-underline"),
        ))
        .id();
    commands.entity(item).insert(RibbonItem {
        index,
        text,
        underline,
    });
    commands.entity(item).add_children(&[text_wrap, underline]);
    item
}

/// The document tab strip: a button-styled active document tab (file icon +
/// name + close) and an add-tab button, with a bottom border separating it
/// from the dock below.
fn build_doc_tabs(commands: &mut Commands, font: &Handle<Font>) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(30.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(HEADER_BG)),
            BorderColor::all(rgb(DIVIDER)),
            Name::new("doc-tabs"),
        ))
        .id();
    let tab = commands
        .spawn((
            Node {
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                // Blue accent on the top edge of the active document tab.
                border: UiRect::top(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(rgb(TAB_ACTIVE_BG)),
            BorderColor::all(rgb(ACCENT_BLUE)),
            Name::new("doc:sponza"),
        ))
        .id();
    let ic = glyph(commands, "file", TEXT_PRIMARY, 13.0);
    let lbl = commands
        .spawn((
            Text::new("sponza"),
            ui_font(font, 12.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    let cl = glyph(commands, "x", TEXT_MUTED, 11.0);
    commands.entity(tab).add_children(&[ic, lbl, cl]);

    // Add-tab button (a small box, not a bare glyph).
    let plus = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(7.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(TAB_ACTIVE_BG)),
            Name::new("doc-add"),
        ))
        .id();
    let plus_icon = glyph(commands, "plus", TEXT_MUTED, 13.0);
    commands.entity(plus).add_child(plus_icon);
    commands.entity(bar).add_children(&[tab, plus]);
    bar
}

/// A full-height flex row used as a top-bar zone (left / center / right).
fn zone(
    commands: &mut Commands,
    name: &str,
    justify: JustifyContent,
    gap: f32,
    grow: f32,
) -> Entity {
    commands
        .spawn((
            Node {
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: justify,
                column_gap: Val::Px(gap),
                flex_grow: grow,
                ..default()
            },
            Name::new(name.to_string()),
        ))
        .id()
}

/// A padded text item (menu entry, ribbon tab). `active` → primary, else muted.
fn text_items(
    commands: &mut Commands,
    font: &Handle<Font>,
    items: &[(&str, bool)],
    size: f32,
) -> Vec<Entity> {
    items
        .iter()
        .map(|(label, active)| {
            let color = if *active { TEXT_PRIMARY } else { TEXT_MUTED };
            text_item(commands, font, label, color, size)
        })
        .collect()
}

fn text_item(
    commands: &mut Commands,
    font: &Handle<Font>,
    label: &str,
    color: (u8, u8, u8),
    size: f32,
) -> Entity {
    commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                align_items: AlignItems::Center,
                ..default()
            },
            Name::new(format!("item:{label}")),
        ))
        .with_children(|p| {
            p.spawn((Text::new(label), ui_font(font, size), TextColor(rgb(color))));
        })
        .id()
}

/// A Phosphor icon button. The HUI `Icon` component is resolved into a Text +
/// Phosphor-font glyph by `renzora_hui`'s `apply_icons` system (run globally by
/// HuiPlugin), so the editor's icon set renders straight into bevy_ui.
fn icon_item(commands: &mut Commands, name: &str, color: (u8, u8, u8), size: f32) -> Entity {
    commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                align_items: AlignItems::Center,
                ..default()
            },
            renzora_hui::icons::Icon::new(name.to_string(), size, Some(rgb(color))),
            Name::new(format!("icon:{name}")),
        ))
        .id()
}

/// An inline Phosphor glyph with no padding (for tab icons, close/add buttons).
/// Uses the deferred [`renzora_hui::icons::Icon`] (resolved a frame later) —
/// fine for chrome built once. Dock icons use [`icon_text`] instead so a
/// re-dock rebuild has no blank frame.
fn glyph(commands: &mut Commands, name: &str, color: (u8, u8, u8), size: f32) -> Entity {
    commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                ..default()
            },
            renzora_hui::icons::Icon::new(name.to_string(), size, Some(rgb(color))),
            Name::new(format!("glyph:{name}")),
        ))
        .id()
}

/// An inline Phosphor glyph resolved immediately (real glyph + Phosphor font),
/// so rebuilding the entity doesn't flash a blank frame like the deferred
/// `Icon` component does. Used for the dock, which rebuilds on re-dock.
fn icon_text(
    commands: &mut Commands,
    phosphor: &Handle<Font>,
    name: &str,
    color: (u8, u8, u8),
    size: f32,
) -> Entity {
    let ch = renzora_hui::phosphor_map::icon_glyph(name).unwrap_or('\u{E4C6}');
    commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                ..default()
            },
            Text::new(ch.to_string()),
            TextFont {
                font: phosphor.clone(),
                font_size: size,
                ..default()
            },
            TextColor(rgb(color)),
            Name::new(format!("icon:{name}")),
        ))
        .id()
}

/// Display title + Phosphor icon for a panel id. Mirrors each panel's
/// `EditorPanel::title()`/`icon()` (which the shell can't reach yet — those are
/// egui trait objects). A bevy-native panel registry will replace this map.
fn panel_meta(id: &str) -> (String, &'static str) {
    let (title, icon): (&str, &str) = match id {
        "viewport" => ("Viewport", "monitor"),
        "render_pipeline" => ("Render Pipeline", "git-fork"),
        "code_editor" => ("Code Editor", "code"),
        "assets" => ("Assets", "folder"),
        "hub_store" => ("Hub Store", "storefront"),
        "console" => ("Console", "terminal"),
        "mixer" => ("Mixer", "faders"),
        "sequencer" => ("Sequencer", "film-strip"),
        "timeline" => ("Timeline", "clock"),
        "record" => ("Record", "record"),
        "hierarchy" => ("Hierarchy", "tree-structure"),
        "scenes" => ("Scenes", "stack"),
        "shape_library" => ("Shapes", "shapes"),
        "inspector" => ("Inspector", "sliders-horizontal"),
        "gamepad" => ("Gamepad", "game-controller"),
        "history" => ("History", "clock-counter-clockwise"),
        _ => return (humanize(id), "circle"),
    };
    (title.to_string(), icon)
}

/// A horizontal strip of text items (menu bar, ribbon, doc tabs, status bar).
/// `active` items render in primary text color, the rest muted.
fn chrome_row(
    commands: &mut Commands,
    font: &Handle<Font>,
    name: &str,
    height: f32,
    bg: Color,
    gap: f32,
    pad: f32,
    items: &[(&str, bool)],
) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(height),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(gap),
                padding: UiRect::horizontal(Val::Px(pad)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(bg),
            Name::new(name.to_string()),
        ))
        .id();

    let kids: Vec<Entity> = items
        .iter()
        .map(|(label, active)| {
            let color = if *active { TEXT_PRIMARY } else { TEXT_MUTED };
            commands
                .spawn((
                    Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    Name::new(format!("item:{label}")),
                ))
                .with_children(|p| {
                    p.spawn((
                        Text::new(*label),
                        ui_font(font, 12.0),
                        TextColor(rgb(color)),
                    ));
                })
                .id()
        })
        .collect();
    commands.entity(row).add_children(&kids);
    row
}

/// Recursively convert a [`DockTree`] into a bevy_ui entity subtree. `path` is
/// the branch chain from the root (`false`/`true` = first/second child),
/// stamped on each divider so a drag can persist its split's ratio.
fn build_tree(
    commands: &mut Commands,
    font: &Handle<Font>,
    phosphor: &Handle<Font>,
    parent: Option<ParentSplit>,
    path: Vec<bool>,
    tree: &DockTree,
) -> Entity {
    match tree {
        DockTree::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let row = matches!(direction, SplitDirection::Horizontal);
            let container = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        flex_direction: if row {
                            FlexDirection::Row
                        } else {
                            FlexDirection::Column
                        },
                        position_type: PositionType::Relative,
                        ..default()
                    },
                    Name::new("split"),
                ))
                .id();

            let pct = ratio.clamp(0.1, 0.9) * 100.0;

            // First child: fixed fraction of the split.
            let mut wa = Node {
                overflow: Overflow::clip(),
                flex_shrink: 0.0,
                ..default()
            };
            if row {
                wa.width = Val::Percent(pct);
                wa.height = Val::Percent(100.0);
            } else {
                wa.height = Val::Percent(pct);
                wa.width = Val::Percent(100.0);
            }
            let wrap_a = commands.spawn((wa, Name::new("split-first"))).id();
            let mut path_a = path.clone();
            path_a.push(false);
            let info_a = ParentSplit {
                container,
                first_wrap: wrap_a,
                horizontal: row,
                is_second: false,
                path: path.clone(),
            };
            let child_a = build_tree(commands, font, phosphor, Some(info_a), path_a, first);
            commands.entity(wrap_a).add_child(child_a);

            // Visible divider line — thin and purely cosmetic (the grab area is
            // a separate, wider invisible overlay below).
            let mut dv = Node {
                flex_shrink: 0.0,
                ..default()
            };
            if row {
                dv.width = Val::Px(1.0);
                dv.height = Val::Percent(100.0);
            } else {
                dv.height = Val::Px(1.0);
                dv.width = Val::Percent(100.0);
            }
            let divider = commands
                .spawn((dv, BackgroundColor(rgb(DIVIDER)), Name::new("divider")))
                .id();

            // Second child: fills the remainder.
            let mut wb = Node {
                overflow: Overflow::clip(),
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                ..default()
            };
            if row {
                wb.height = Val::Percent(100.0);
            } else {
                wb.width = Val::Percent(100.0);
            }
            let wrap_b = commands.spawn((wb, Name::new("split-second"))).id();
            let mut path_b = path.clone();
            path_b.push(true);
            let info_b = ParentSplit {
                container,
                first_wrap: wrap_a,
                horizontal: row,
                is_second: true,
                path: path.clone(),
            };
            let child_b = build_tree(commands, font, phosphor, Some(info_b), path_b, second);
            commands.entity(wrap_b).add_child(child_b);

            // Invisible grab handle: a wide absolutely-positioned strip centered
            // on the seam, on top of everything (added last) so it's easy to
            // grab without widening the visible line. Carries the `Divider`.
            const GRAB: f32 = 11.0;
            let mut hit = Node {
                position_type: PositionType::Absolute,
                ..default()
            };
            if row {
                hit.left = Val::Percent(pct);
                hit.margin = UiRect::left(Val::Px(-GRAB / 2.0));
                hit.width = Val::Px(GRAB);
                hit.top = Val::Px(0.0);
                hit.height = Val::Percent(100.0);
            } else {
                hit.top = Val::Percent(pct);
                hit.margin = UiRect::top(Val::Px(-GRAB / 2.0));
                hit.height = Val::Px(GRAB);
                hit.left = Val::Px(0.0);
                hit.width = Val::Percent(100.0);
            }
            // Resize cursor on hover (HuiPlugin's cursor system applies it).
            let cursor = renzora_hui::cursor_icon::parse_cursor(if row {
                "ew-resize"
            } else {
                "ns-resize"
            })
            .unwrap();
            let handle = commands
                .spawn((
                    hit,
                    Interaction::default(),
                    renzora_hui::cursor_icon::HoverCursor(cursor),
                    Divider {
                        container,
                        first_wrap: wrap_a,
                        horizontal: row,
                        path: path.clone(),
                    },
                    Name::new("divider-handle"),
                ))
                .id();

            commands
                .entity(container)
                .add_children(&[wrap_a, divider, wrap_b, handle]);
            container
        }
        DockTree::Leaf { tabs, active_tab } => {
            build_leaf(commands, font, phosphor, parent, tabs, *active_tab)
        }
        DockTree::Empty => commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                Name::new("empty"),
            ))
            .id(),
    }
}

/// A dock leaf: a tab-bar over a content region (placeholder content for now).
fn build_leaf(
    commands: &mut Commands,
    font: &Handle<Font>,
    phosphor: &Handle<Font>,
    parent: Option<ParentSplit>,
    tabs: &[String],
    active: usize,
) -> Entity {
    let leaf = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                position_type: PositionType::Relative,
                ..default()
            },
            BackgroundColor(rgb(PANEL_BG)),
            // Cursor's 0..1 position in the leaf — used to pick the drop zone.
            bevy::ui::RelativeCursorPosition::default(),
            Name::new("leaf"),
        ))
        .id();
    populate_leaf(commands, font, phosphor, parent, leaf, tabs, active);
    leaf
}

/// Build a leaf's contents (tab-bar, content, drop overlay) onto `leaf`. Split
/// out so a tab switch can rebuild only this leaf — though switching actually
/// mutates in place ([`apply_tab_switch`]); this runs on first build / re-dock.
fn populate_leaf(
    commands: &mut Commands,
    font: &Handle<Font>,
    phosphor: &Handle<Font>,
    parent: Option<ParentSplit>,
    leaf: Entity,
    tabs: &[String],
    active: usize,
) {
    // Tab bar.
    let tabbar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(28.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                padding: UiRect::horizontal(Val::Px(4.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(HEADER_BG)),
            // Drop target: dragging a tab here adds it to this leaf.
            bevy::ui::RelativeCursorPosition::default(),
            TabBarOf(leaf),
            Name::new("tabbar"),
        ))
        .id();
    // Each tab: icon + label + a close × (always present; hidden when inactive
    // so a switch toggles display rather than adding/removing entities).
    let mut bar_kids: Vec<Entity> = Vec::new();
    for (i, id) in tabs.iter().enumerate() {
        let is_active = i == active;
        let fg = if is_active { TEXT_PRIMARY } else { TEXT_MUTED };
        let (title, icon) = panel_meta(id);
        let tab = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(5.0),
                    padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                    position_type: PositionType::Relative,
                    // Keep the tab at its label's natural width so the text
                    // never gets squeezed and wrapped.
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(if is_active {
                    rgb(TAB_ACTIVE_BG)
                } else {
                    Color::NONE
                }),
                Interaction::default(),
                // For computing the drop insertion point within the tab bar.
                bevy::ui::RelativeCursorPosition::default(),
                renzora_hui::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                Name::new(format!("tab:{id}")),
            ))
            .id();
        let tab_icon = icon_text(commands, phosphor, icon, fg, 13.0);
        let tab_label = commands
            .spawn((
                Text::new(title),
                ui_font(font, 12.0),
                TextColor(rgb(fg)),
                bevy::text::TextLayout::new_with_no_wrap(),
            ))
            .id();
        // Close button — shown on every tab; reddens on hover. `Pass` so it
        // doesn't block the tab's own hover state.
        let close = icon_text(commands, phosphor, "x", TEXT_MUTED, 11.0);
        commands.entity(close).insert((
            TabClose,
            bevy::ui::RelativeCursorPosition::default(),
            bevy::ui::FocusPolicy::Pass,
        ));
        // Vertical insertion marker at this tab's left edge (toggled during a
        // drag; re-anchored to the right edge for end-insertion).
        let marker = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(-2.0),
                    top: Val::Px(0.0),
                    height: Val::Percent(100.0),
                    width: Val::Px(2.0),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(rgb(ACCENT_BLUE)),
                bevy::ui::FocusPolicy::Pass,
                InsertMarker,
                Name::new("insert-marker"),
            ))
            .id();
        commands.entity(tab).insert(DockTab {
            id: id.clone(),
            leaf,
            label: tab_label,
            icon: tab_icon,
            marker,
        });
        commands
            .entity(tab)
            .add_children(&[tab_icon, tab_label, close, marker]);
        bar_kids.push(tab);
    }
    // Trailing add-tab button.
    bar_kids.push(icon_text(commands, phosphor, "plus", TEXT_MUTED, 13.0));
    // Empty area filler — also a secondary resize handle for the parent split
    // boundary when aligned (egui-style "more to grip"). Reuses `Divider` +
    // `divider_drag` so it resizes exactly like the adjacent divider.
    let filler = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Percent(100.0),
                ..default()
            },
            Name::new("tabbar-filler"),
        ))
        .id();
    if let Some(p) = parent.filter(|p| p.aligned()) {
        let cursor = renzora_hui::cursor_icon::parse_cursor(if p.horizontal {
            "ew-resize"
        } else {
            "ns-resize"
        })
        .unwrap();
        commands.entity(filler).insert((
            Interaction::default(),
            renzora_hui::cursor_icon::HoverCursor(cursor),
            Divider {
                container: p.container,
                first_wrap: p.first_wrap,
                horizontal: p.horizontal,
                path: p.path,
            },
        ));
    }
    bar_kids.push(filler);
    commands.entity(tabbar).add_children(&bar_kids);

    // Content region — placeholder showing the active panel's title.
    let content = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            Name::new("content"),
        ))
        .id();
    let title = tabs.get(active).map(|s| panel_meta(s).0).unwrap_or_default();
    let content_text = commands
        .spawn((
            Text::new(title),
            ui_font(font, 13.0),
            TextColor(rgb(PLACEHOLDER)),
        ))
        .id();
    commands.entity(content).add_child(content_text);

    // Drop-zone highlight — an accent outline (no fill), like the egui dock.
    // Hidden until a tab is dragged over this leaf.
    let overlay = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                border: UiRect::all(Val::Px(2.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(rgb(ACCENT_BLUE)),
            bevy::ui::FocusPolicy::Pass,
            DropOverlay,
            Name::new("drop-overlay"),
        ))
        .id();

    commands.entity(leaf).insert(DockLeaf {
        tabs: tabs.to_vec(),
        overlay,
        content_text,
    });
    commands
        .entity(leaf)
        .add_children(&[tabbar, content, overlay]);
}

/// `render_pipeline` → `Render Pipeline`, `code_editor` → `Code Editor`.
fn humanize(id: &str) -> String {
    id.split('_')
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
