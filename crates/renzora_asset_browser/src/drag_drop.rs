//! Dragging assets out of the browser: the press→drag promotion, the cursor
//! ghost, dropping onto a folder, and the spring-loaded dock tabs that reveal a
//! drop target hidden behind another tab.
//!
//! None of these systems is gated on the Assets panel being the active tab. A
//! drag *starts* here but continues over other panels, and
//! [`asset_drag_tab_spring`] deliberately switches the Assets leaf to a sibling
//! mid-drag — a panel-gated `asset_drag` would freeze there, leaving the payload
//! uncleared and the ghost stranded on screen at release.

use std::path::{Path, PathBuf};

use bevy::picking::Pickable;
use bevy::prelude::*;

use renzora_editor_framework::{
    MaterialThumbnailRegistry, ModelThumbnailRegistry, SceneThumbnailRegistry,
};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::theme::{accent, popup_bg, rgb, text_primary};
use renzora_ember::widgets::{EmberScroll, ScrollbarBusy};

use crate::ops::{asset_type_info, icon_for};
use crate::state::{
    file_name_of, thumb_kind, AssetTile, DragGhost, GridArea, NativeAssets, ThumbKind, TreeNav,
};
use crate::thumbnails::ThumbnailCache;

/// Drag a tile out toward the viewport: records the press, and once the cursor
/// moves >5px inserts an `AssetDragPayload` (the viewport shows a live preview
/// while it exists and commits the spawn when it's removed on release). Mirrors
/// the egui drag lifecycle, which only runs in the egui pass.
pub(crate) fn asset_drag(
    tiles: Query<(&Interaction, &AssetTile)>,
    tree: Query<(&Interaction, &TreeNav)>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    scrollbar: Res<ScrollbarBusy>,
    mut state: ResMut<NativeAssets>,
    payload: Option<Res<renzora_editor_framework::AssetDragPayload>>,
    mut commands: Commands,
) {
    if mouse.just_released(MouseButton::Left) {
        // Dropped over a folder (a grid folder tile or a tree row) → move the
        // dragged file(s) into it instead of spawning into the viewport.
        if state.dragging {
            if let Some(payload) = payload.as_ref() {
                if let Some(target) = drop_folder(&tiles, &tree) {
                    let sources = payload.paths.clone();
                    commands.queue(move |w: &mut World| move_assets(w, &sources, &target));
                }
            }
        }
        // A plain click (no drag) on an already-multi-selected item collapses
        // the selection to just that item — applied here, on release, so the
        // multi-selection survived in case this had become a drag instead.
        if let Some(p) = state.pending_single_select.take() {
            if !state.dragging {
                state.selection.clear();
                state.selection.insert(p.clone());
                state.selected = Some(p.clone());
                state.selection_anchor = Some(p);
            }
        }
        state.drag_press = None;
        state.dragging = false;
        if payload.is_some() {
            commands.remove_resource::<renzora_editor_framework::AssetDragPayload>();
        }
        return;
    }
    let cursor = windows.iter().find_map(|w| w.cursor_position());
    if mouse.just_pressed(MouseButton::Left) && !scrollbar.active() {
        if let Some(c) = cursor {
            // A pressed grid/list tile (file or folder)…
            if let Some((_, tile)) = tiles.iter().find(|(i, _)| matches!(i, Interaction::Pressed)) {
                state.drag_press = Some((tile.path.clone(), tile.is_dir, c));
            // …or a pressed tree folder row (TreeNav targets are always folders).
            } else if let Some((_, nav)) = tree.iter().find(|(i, _)| matches!(i, Interaction::Pressed)) {
                state.drag_press = Some((nav.0.clone(), true, c));
            }
        }
    }
    if payload.is_none() {
        if let (Some((path, _is_dir, origin)), Some(c)) = (state.drag_press.clone(), cursor) {
            // Files *and* folders are draggable into folders.
            if c.distance(origin) > 5.0 {
                // Multi-drag: carry the whole selection if the dragged tile is
                // part of it, else just the dragged file.
                let paths: Vec<PathBuf> = if state.selection.contains(&path) && state.selection.len() > 1 {
                    state.selection.iter().cloned().collect()
                } else {
                    vec![path.clone()]
                };
                let count = paths.len();
                commands.insert_resource(renzora_editor_framework::AssetDragPayload {
                    name: file_name_of(&path),
                    paths,
                    icon: String::new(),
                    color: [170, 175, 190],
                    origin: Vec2::new(origin.x, origin.y),
                    is_detached: true,
                    drag_count: count,
                    path,
                });
                state.dragging = true;
                // The press that started this drag may also have armed an
                // explorer-style rename, if it landed on the name label of an
                // already-selected item. Moving 5px settles which gesture it
                // was, and it was not a rename, so retire the arm here rather
                // than let it open a text field 0.45s into the drag.
                state.rename_arm = None;
            }
        }
    }
}

/// Spring-loaded tabs: while an asset drag is in flight, dwelling the cursor over
/// a background dock tab brings that tab to the foreground — without releasing the
/// drag. This lets you reveal a drop target that's hidden behind another tab (drop
/// a mesh onto the viewport, a texture onto the inspector, …) the same way file
/// managers spring open a folder you hover over mid-drag.
///
/// A short dwell delay is deliberate: merely sweeping the cursor *across* a tab bar
/// on the way somewhere else shouldn't yank the layout around — only a pause on one
/// tab does. Not gated on `panel_active("assets")`, since the target tab usually
/// lives in a *different* leaf than the Assets panel and we must keep watching the
/// drag regardless of which panel is focused.
pub(crate) fn asset_drag_tab_spring(
    payload: Option<Res<renzora_editor_framework::AssetDragPayload>>,
    time: Res<Time>,
    dock: Res<renzora_ember::dock::Dock>,
    tabs: Query<(&bevy::ui::RelativeCursorPosition, &renzora_ember::dock::DockTab)>,
    mut focus: ResMut<renzora_ember::dock::FocusPanelRequest>,
    // (hovered tab id, seconds dwelled on it) — reset whenever the drag ends or
    // the hovered tab changes, so each tab earns its own fresh dwell.
    mut dwell: Local<Option<(String, f32)>>,
) {
    // No live drag (or the press hasn't crossed the drag threshold yet) → stand down.
    if payload.is_none_or(|p| !p.is_detached) {
        *dwell = None;
        return;
    }

    // The background tab under the cursor, if any. Tabs that are already the active
    // tab in their leaf have nothing to reveal, so skip them.
    let hovered = tabs.iter().find_map(|(rcp, tab)| {
        (rcp.cursor_over && !dock.tree.is_active_tab(&tab.id)).then(|| tab.id.clone())
    });
    let Some(id) = hovered else {
        *dwell = None;
        return;
    };

    // Accumulate dwell on the same tab; a change of tab restarts the timer.
    let elapsed = match dwell.as_mut() {
        Some((prev, secs)) if *prev == id => {
            *secs += time.delta_secs();
            *secs
        }
        _ => {
            *dwell = Some((id.clone(), 0.0));
            0.0
        }
    };
    if elapsed >= SPRING_DWELL_SECS {
        // Route through the same in-place switch a click performs. `apply_tab_switch`
        // recolors labels and shows the pane; the drop-target crates react to the
        // now-visible pane. Clear the dwell so we don't re-fire while still hovering.
        focus.0 = Some(id);
        *dwell = None;
    }
}

/// Hover-dwell before a spring tab-switch fires. Long enough that crossing a tab
/// bar in passing is ignored, short enough to feel responsive once you settle.
const SPRING_DWELL_SECS: f32 = 0.35;

/// The folder under the cursor to drop onto — a hovered grid folder tile, else a
/// hovered tree folder row.
fn drop_folder(
    tiles: &Query<(&Interaction, &AssetTile)>,
    tree: &Query<(&Interaction, &TreeNav)>,
) -> Option<PathBuf> {
    // Accept Hovered *or* Pressed: on the release frame the folder under the
    // cursor may still read Pressed (the button was down through the drag).
    let over = |i: &Interaction| matches!(i, Interaction::Hovered | Interaction::Pressed);
    if let Some((_, tile)) = tiles.iter().find(|(i, t)| t.is_dir && over(i)) {
        return Some(tile.path.clone());
    }
    tree.iter().find(|(i, _)| over(i)).map(|(_, nav)| nav.0.clone())
}

/// Move `sources` into `target` (drag-to-folder). Skips no-op / into-itself
/// moves and updates asset references via `emit_asset_path_change`, mirroring the
/// egui browser's `pending_move`. Navigates to the target on success.
fn move_assets(world: &mut World, sources: &[PathBuf], target: &Path) {
    let mut moved = 0usize;
    for source in sources {
        let Some(file_name) = source.file_name() else { continue };
        let dest = target.join(file_name);
        if *source == dest || source.as_path() == target {
            continue;
        }
        // Don't move a folder into itself / a descendant.
        if dest.starts_with(source) {
            continue;
        }
        let is_dir = source.is_dir();
        if std::fs::rename(source, &dest).is_ok() {
            moved += 1;
            crate::emit_asset_path_change(world, source, &dest, is_dir);
        }
    }
    if let Some(mut s) = world.get_resource_mut::<NativeAssets>() {
        s.selection.clear();
        s.selected = None;
        s.listing_dirty = true;
        if moved > 0 {
            s.current = Some(target.to_path_buf());
            s.expanded.insert(target.to_path_buf());
        }
    }
}

/// A floating ghost (icon + name) that follows the cursor while a tile is being
/// dragged out of the browser. Spawned as a top-level overlay so it isn't
/// clipped by the panel, and despawned the moment the drag ends.
pub(crate) fn drag_ghost(
    mut commands: Commands,
    state: Res<NativeAssets>,
    payload: Option<Res<renzora_editor_framework::AssetDragPayload>>,
    fonts: Option<Res<EmberFonts>>,
    windows: Query<&Window>,
    // The same thumbnail sources the tiles use, so the ghost carries the asset's
    // real rendered preview instead of a generic glyph.
    cache: Option<Res<ThumbnailCache>>,
    model: Option<Res<ModelThumbnailRegistry>>,
    material: Option<Res<MaterialThumbnailRegistry>>,
    scene: Option<Res<SceneThumbnailRegistry>>,
    mut ghosts: Query<(Entity, &mut Node), With<DragGhost>>,
) {
    let Some(payload) = payload.filter(|_| state.dragging) else {
        for (e, _) in &ghosts {
            commands.entity(e).despawn();
        }
        return;
    };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else {
        return;
    };
    // Reposition an existing ghost.
    if let Some((_, mut node)) = ghosts.iter_mut().next() {
        node.left = Val::Px(cursor.x + 12.0);
        node.top = Val::Px(cursor.y + 14.0);
        return;
    }
    // Otherwise spawn one (fonts may not be ready on the very first drag frame).
    let Some(fonts) = fonts else {
        return;
    };
    let color = asset_type_info(&payload.path).0;
    let ghost = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(cursor.x + 12.0),
                top: Val::Px(cursor.y + 14.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                max_width: Val::Px(240.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(popup_bg()).with_alpha(0.92)),
            BorderColor::all(rgb(accent())),
            GlobalZIndex(10_000),
            Pickable::IGNORE,
            DragGhost,
            Name::new("asset-drag-ghost"),
        ))
        .id();
    // Prefer the asset's real thumbnail (already rendered/cached — you had to see
    // its tile to grab it), and fall back to the type glyph if it isn't ready.
    let handle = thumb_kind(&payload.name).and_then(|kind| match kind {
        ThumbKind::Image => cache.as_ref().and_then(|c| c.handle(&payload.path)),
        ThumbKind::Model => model.as_ref().and_then(|r| r.handle(&payload.path)),
        ThumbKind::Material => material.as_ref().and_then(|r| r.handle(&payload.path)),
        ThumbKind::Scene => scene.as_ref().and_then(|r| r.handle(&payload.path)),
    });
    let lead = match handle {
        Some(image) => commands
            .spawn((
                ImageNode::new(image),
                Node {
                    width: Val::Px(GHOST_THUMB),
                    height: Val::Px(GHOST_THUMB),
                    flex_shrink: 0.0,
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .id(),
        None => icon_text(&mut commands, &fonts.phosphor, icon_for(&payload.path, false), color, 14.0),
    };
    // For a multi-select drag, name the count rather than a single file.
    let text = if payload.drag_count > 1 {
        format!("{} items", payload.drag_count)
    } else {
        payload.name.clone()
    };
    let lbl = commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(ghost).add_children(&[lead, lbl]);
}

/// Edge length of the thumbnail preview in the drag ghost.
const GHOST_THUMB: f32 = 32.0;

/// Consume [`AssetDropScrollRequest`](renzora::core::AssetDropScrollRequest) and
/// pin the grid's scroll view to the bottom for ~1.2 s afterwards. The window
/// (not a one-shot) matters because the dropped file isn't on disk / in the
/// listing yet the frame it's requested — `scroll_to(f32::MAX)` re-clamps to the
/// growing bottom each frame until the rescan has surfaced the new tile.
pub(crate) fn scroll_grid_on_drop(
    time: Res<Time>,
    mut req: ResMut<renzora::core::AssetDropScrollRequest>,
    mut until: Local<Option<f32>>,
    grid_area: Query<&Children, With<GridArea>>,
    mut scrolls: Query<&mut EmberScroll>,
) {
    let now = time.elapsed_secs();
    if req.0 {
        req.0 = false;
        *until = Some(now + 1.2);
    }
    let Some(deadline) = *until else { return };
    if now >= deadline {
        *until = None;
        return;
    }
    let Ok(kids) = grid_area.single() else { return };
    let Some(viewport) = kids.iter().find(|&e| scrolls.contains(e)) else { return };
    if let Ok(mut s) = scrolls.get_mut(viewport) {
        // f32::MAX is clamped to the scrollable range by `scroll_update`.
        s.scroll_to(f32::MAX);
    }
}
