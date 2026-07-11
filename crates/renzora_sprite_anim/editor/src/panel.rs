//! The **Sprite Anim** panel layout — a cell picker for creating clips.
//!
//! Header (the target sprite), a zoomable **palette** of its current sheet
//! (grid and cell selection — see [`crate::palette`]), and a **Create Clip**
//! row (fps + name). No grid inputs (the grid is the entity's `SpriteSheet`)
//! and no image switching (that's the Sprite Image inspector + the timeline).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora::core::{SpriteImagePath, SpriteImages};
use renzora::RenzoraShellExt;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, bind_text, keyed_list, KeyedSnapshot};
use renzora_ember::theme::{rgb, text_muted};
use renzora_ember::widgets::{
    drag_value, dropdown, icon_button, icon_label_button, label, scroll_view_xy, text_input,
};

use crate::{CurrentSheet, SheetFps, SpriteTarget};

// ── Markers ──────────────────────────────────────────────────────────────────

/// The sheet `ImageNode` — the zoom/pan + cell-selection target.
#[derive(Component)]
pub(crate) struct SheetImageNode;

/// A grid-overlay line (child of the sheet image).
#[derive(Component)]
pub(crate) struct GridLine;

/// A selected-cell mark (child of the sheet image, carries its order number).
#[derive(Component)]
pub(crate) struct SelCellMark;

/// Zoom step buttons.
#[derive(Component)]
pub(crate) struct ZoomInBtn;
#[derive(Component)]
pub(crate) struct ZoomOutBtn;

/// The new-clip name field + Create button.
#[derive(Component)]
pub(crate) struct ClipNameInput;
#[derive(Component)]
pub(crate) struct CreateClipButton;

// ── Registration + build ─────────────────────────────────────────────────────

pub(crate) fn register(app: &mut App) {
    app.register_shell_panel("sprite_anim", "Sprite Anim", "film-strip", "2D");
    app.register_panel_content("sprite_anim", false, build);
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        })
        .id();

    // Header: the target sprite's name (or a hint) + zoom controls.
    let header = row(commands);
    let name = label(commands, &fonts.ui, "Select a 2D sprite");
    bind_text(commands, name, |world: &World| {
        match world.get_resource::<SpriteTarget>().and_then(|t| t.0) {
            Some(e) => world
                .get::<Name>(e)
                .map(|n| n.as_str().to_string())
                .unwrap_or_else(|| "Sprite".to_string()),
            None => "Select a 2D sprite".to_string(),
        }
    });
    let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let zoom_out = icon_button(commands, fonts, "minus");
    commands.entity(zoom_out).insert(ZoomOutBtn);
    let zoom_in = icon_button(commands, fonts, "plus");
    commands.entity(zoom_in).insert(ZoomInBtn);
    commands.entity(header).add_children(&[name, spacer, zoom_out, zoom_in]);

    // Sheet dropdown: pick which sheet to author from (mirrors the inspector's
    // Image dropdown). Rebuilt reactively when the sheet list changes.
    let sheet_row = row(commands);
    let sheet_label = label(commands, &fonts.ui, "Sheet");
    let sheet_dd_host = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    keyed_list(commands, sheet_dd_host, sheet_dropdown_snapshot);
    commands.entity(sheet_row).add_children(&[sheet_label, sheet_dd_host]);

    // Palette.
    let palette_content = commands
        .spawn(Node { align_self: AlignSelf::Start, flex_shrink: 0.0, ..default() })
        .id();
    keyed_list(commands, palette_content, sheet_snapshot);
    let palette_scroll = scroll_view_xy(commands, palette_content);
    let palette_box = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            min_height: Val::Px(80.0),
            ..default()
        })
        .id();
    commands.entity(palette_box).add_child(palette_scroll);

    // Create row: FPS + name + Create Clip.
    let create = row(commands);
    let fps_label = label(commands, &fonts.ui, "FPS");
    let fps_dv = drag_value(commands, &fonts.ui, "", (150, 150, 160), 10.0, 1.0);
    bind_2way(
        commands,
        fps_dv,
        |world: &World| world.get_resource::<SheetFps>().map(|f| f.0).unwrap_or(10.0),
        |world: &mut World, v: &f32| {
            if let Some(mut f) = world.get_resource_mut::<SheetFps>() {
                f.0 = v.max(1.0);
            }
        },
    );
    let name_in = text_input(commands, &fonts.ui, "clip name (e.g. idle_n)", "");
    commands.entity(name_in).insert(ClipNameInput);
    let create_btn = icon_label_button(commands, fonts, "plus", "Create Clip");
    commands.entity(create_btn).insert(CreateClipButton);
    commands.entity(create).add_children(&[fps_label, fps_dv, name_in, create_btn]);

    // Footer note.
    let note = commands
        .spawn((
            Text::new(
                "Grid = the Sprite Sheet component (H/V Frames). Switch sheets in Sprite Image. \
                 Clips open in the Timeline.",
            ),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            bevy::picking::Pickable::IGNORE,
        ))
        .id();

    commands.entity(root).add_children(&[header, sheet_row, palette_box, create, note]);
    root
}

/// File name of an asset-relative path (for dropdown labels).
fn file_name(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

/// Reactive dropdown of the target sprite's sheet names. Switching it sets
/// `SpriteImages.index` — the active sheet the palette shows and that new clips
/// pin to. Keyed on the target + name list, so it rebuilds when a sheet is added
/// (same reason the inspector's Image dropdown folds its options into the sig).
fn sheet_dropdown_snapshot(world: &World) -> KeyedSnapshot {
    let target = world.get_resource::<SpriteTarget>().and_then(|t| t.0);
    let (names, selected): (Vec<String>, usize) = target
        .map(|e| {
            if let Some(imgs) = world.get::<SpriteImages>(e) {
                (imgs.images.iter().map(|p| file_name(p)).collect(), imgs.index as usize)
            } else if let Some(p) = world.get::<SpriteImagePath>(e) {
                if p.0.is_empty() {
                    (Vec::new(), 0)
                } else {
                    (vec![file_name(&p.0)], 0)
                }
            } else {
                (Vec::new(), 0)
            }
        })
        .unwrap_or((Vec::new(), 0));

    let mut items = Vec::new();
    if !names.is_empty() {
        let mut h = DefaultHasher::new();
        target.map(|e| e.to_bits()).hash(&mut h);
        names.hash(&mut h);
        items.push((0u64, h.finish()));
    }

    KeyedSnapshot {
        items,
        build: Box::new(move |c: &mut Commands, fonts: &EmberFonts, _i: usize| {
            let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
            let sel = selected.min(refs.len().saturating_sub(1));
            let dd = dropdown(c, fonts, &refs, sel);
            bind_2way(
                c,
                dd,
                |world: &World| {
                    world
                        .get_resource::<SpriteTarget>()
                        .and_then(|t| t.0)
                        .and_then(|e| world.get::<SpriteImages>(e))
                        .map(|s| s.index as usize)
                        .unwrap_or(0)
                },
                |world: &mut World, i: &usize| {
                    if let Some(e) = world.get_resource::<SpriteTarget>().and_then(|t| t.0) {
                        if let Some(mut s) = world.get_mut::<SpriteImages>(e) {
                            s.index = *i as u32;
                        }
                    }
                },
            );
            dd
        }),
    }
}

/// A left-aligned flex row with a small gap.
fn row(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id()
}

// ── Snapshot ─────────────────────────────────────────────────────────────────

/// Single-item snapshot: the current sheet image (or a hint), keyed on path +
/// pixel size. The palette systems attach grid + selection marks as children.
fn sheet_snapshot(world: &World) -> KeyedSnapshot {
    let path = world.get_resource::<CurrentSheet>().and_then(|s| s.0.clone());
    let mut items = Vec::new();
    let mut data: Option<Handle<Image>> = None;

    if let Some(path) = &path {
        let handle = world
            .get_resource::<AssetServer>()
            .map(|a| a.load::<Image>(path.clone()))
            .unwrap_or_default();
        let dims = world
            .get_resource::<Assets<Image>>()
            .and_then(|imgs| imgs.get(&handle))
            .map(|img| img.size())
            .unwrap_or_default();
        let mut h = DefaultHasher::new();
        path.hash(&mut h);
        (dims.x, dims.y).hash(&mut h);
        items.push((1u64, h.finish()));
        data = Some(handle);
    } else {
        items.push((2u64, 0));
    }

    KeyedSnapshot {
        items,
        build: Box::new(move |c: &mut Commands, fonts: &EmberFonts, _i: usize| {
            let Some(handle) = data.clone() else {
                return label(c, &fonts.ui, "Select a 2D sprite to pick frames from");
            };
            c.spawn((
                ImageNode::new(handle),
                Node { width: Val::Px(1.0), height: Val::Px(1.0), flex_shrink: 0.0, ..default() },
                SheetImageNode,
                RelativeCursorPosition::default(),
            ))
            .id()
        }),
    }
}
