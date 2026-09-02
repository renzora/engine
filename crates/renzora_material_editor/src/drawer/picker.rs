//! The picker tray: a search box over a wrapping grid of material previews.
//!
//! It's an ordinary in-flow node that starts hidden, **not** a `Popup` — an
//! overlay would have to float above the drawer, and ember's `popup_position`
//! pins a panel with `top: 100%`, which on a node that isn't absolutely
//! positioned offsets it by its own height instead of anchoring it. Opening the
//! tray simply makes the drawer taller and slides the texture slots down.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use bevy::prelude::*;
use bevy::ui::FlexWrap;

use renzora_editor_framework::MaterialThumbnailRegistry;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::keyed_list_tokened;
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::{accent, border, hover_bg, popup_bg, rgb, text_muted, text_primary};
use renzora_ember::widgets::{text_input, HoverTint, HoverTooltip};

use super::drop::bind_material;
use super::index::MaterialIndex;
use super::slot::{bind_preview, preview_square};
use super::textures::{TexSlotZone, TexSlotsToggle};
use super::{material_path, MatPickerFilter};

/// Tile metrics. A `.material` is a *picture*, so the picker shows pictures: a
/// wrapping grid of preview tiles instead of the old text rows, which packed an
/// 11px name and a 9px folder into a 26px row and collided.
///
/// The tile is a fixed width and the grid wraps, so the layout re-flows with the
/// inspector — a wide dock simply fits more per row.
const TILE_W: f32 = 78.0;
const TILE_GAP: f32 = 6.0;
/// 3px padding + 72px preview + 3px gap + 13px label + 3px padding.
const TILE_H: f32 = 94.0;

/// Most tiles the tray will ever show.
///
/// This is a *hard* cap, not a window: the tray has no scroll area of its own,
/// so what it builds is what it is tall enough for. That is the point — the tray
/// lives inside the inspector, which already scrolls, and nesting a second
/// scrollbar a few pixels from the panel's own read as a mistake before it read
/// as a control. Twelve is four rows at the usual three columns: enough to
/// recognise a material by sight, small enough that the drawer below stays
/// reachable. Anything past it is reached by typing, and [`picker_note`] says so
/// rather than letting the rest vanish silently.
const PICKER_MAX_ROWS: usize = 12;

/// Marks a picker tray. Purely a marker — the tiles are a keyed list that
/// captures its inspected entity directly, so nothing needs to look it up off
/// the tray. Kept because `refresh_material_index` gates on its presence (no
/// tray built → never walk the project) and [`close_pickers`] finds trays by it.
#[derive(Component)]
pub(crate) struct MatPickerPanel;

/// The field that slides its picker tray open, and the two things it drives.
///
/// Deliberately not ember's [`Popup`](renzora_ember::widgets::Popup): that's for
/// panels that *float*, and its positioning system pins one with `top: 100%`,
/// which on an in-flow node offsets it by its own height rather than anchoring
/// it under the trigger.
#[derive(Component)]
pub(super) struct MatPickerToggle {
    /// The inspected entity, so opening the tray can hide *its* texture rows and
    /// not another drawer's.
    pub(super) entity: Entity,
    pub(super) panel: Entity,
    pub(super) caret: Entity,
}

#[derive(Component)]
pub(super) struct MatPickerItem {
    entity: Entity,
    rel: String,
}

/// Build the picker tray.
///
/// It carries no surface of its own. A filled, bordered tray inside the
/// inspector's own filled, bordered panel was a box in a box; the search field
/// is the only thing here that needs an edge, so it's the only thing that has
/// one, and the tiles sit directly on the drawer.
///
/// Built **once**, with the slot — never refilled per keystroke. The tiles are
/// registered on the inner `grid` node rather than on the tray, so reconciling
/// them can never touch the search box: `run_keyed_lists` calls
/// `replace_children` on its container, which would otherwise blow the input away
/// (and with it the focus and the half-typed query) on every keystroke.
pub(super) fn build_picker_panel(commands: &mut Commands, fonts: &EmberFonts, entity: Entity) -> Entity {
    let panel = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                margin: UiRect::top(Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                display: Display::None,
                ..default()
            },
            bevy::ui::FocusPolicy::Block,
            MatPickerPanel,
            Name::new("material-picker-tray"),
        ))
        .id();

    // The search row *is* the search box: the glyph sits inside the same
    // bordered surface as the text, so there's one edge here rather than an
    // input box nested in a header strip drawing a second one beside it.
    let header = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(26.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            Name::new("material-picker-search"),
        ))
        .id();
    let glass = icon_text(commands, &fonts.phosphor, "magnifying-glass", text_muted(), 12.0);
    commands.entity(glass).insert(bevy::ui::FocusPolicy::Pass);
    let search = text_input(commands, &fonts.ui, "Search materials…", "");
    commands
        .entity(search)
        .insert((BackgroundColor(Color::NONE), BorderColor::all(Color::NONE)))
        .entry::<Node>()
        .and_modify(|mut n| {
            n.flex_grow = 1.0;
            n.min_width = Val::Px(0.0);
        });
    bind_search(commands, search);
    commands.entity(header).add_children(&[glass, search]);

    // Wrapping grid, sitting straight on the drawer. The vertical gap is the
    // tile's own bottom margin rather than the container's `row_gap` so the last
    // row doesn't leave a hanging gap above the note under it.
    let grid = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_content: AlignContent::FlexStart,
            column_gap: Val::Px(TILE_GAP),
            ..default()
        })
        .id();
    register_picker_rows(commands, grid, entity);
    commands.entity(panel).add_children(&[header, grid]);
    panel
}

/// Register the tiles as a keyed list.
///
/// Plain rather than virtualized: [`PICKER_MAX_ROWS`] is the whole list now, and
/// windowing twelve tiles would be bookkeeping in exchange for nothing.
fn register_picker_rows(commands: &mut Commands, list: Entity, entity: Entity) {
    keyed_list_tokened(
        commands,
        list,
        // Dirty token: re-snapshot only when the query text, the cached index, or
        // this entity's assigned material actually changes.
        move |w: &Rx| {
            let mut h = DefaultHasher::new();
            w.get_resource::<MatPickerFilter>()
                .map(|f| f.text.as_str())
                .unwrap_or("")
                .hash(&mut h);
            w.get_resource::<MaterialIndex>().map(|i| i.generation).unwrap_or(0).hash(&mut h);
            material_path(&Rx::new(w.untracked()), entity).hash(&mut h);
            h.finish()
        },
        move |w: &Rx| picker_snapshot(&Rx::new(w.untracked()), entity),
    );
}

/// This frame's filtered row set. Cheap: an `Arc` clone plus a substring test per
/// candidate; no filesystem access (see [`MaterialIndex`]).
fn picker_snapshot(w: &Rx, entity: Entity) -> KeyedSnapshot {
    let query = w.get_resource::<MatPickerFilter>().map(|f| f.text.clone()).unwrap_or_default();
    let current_path = material_path(w, entity);
    let materials = w
        .get_resource::<MaterialIndex>()
        .map(|i| i.materials.clone())
        .unwrap_or_default();
    let lower = query.trim().to_ascii_lowercase();
    // Count every match, then keep only the first [`PICKER_MAX_ROWS`]: the total
    // is what the truncation note reports, and without it a cap of twelve would
    // quietly claim the project has twelve materials.
    let matched: Vec<&(String, String)> = materials
        .iter()
        .filter(|(rel, _)| lower.is_empty() || rel.to_ascii_lowercase().contains(&lower))
        .collect();
    let total = matched.len();
    let rows: Vec<(String, String, bool)> = matched
        .into_iter()
        .take(PICKER_MAX_ROWS)
        .map(|(rel, abs)| {
            let is_current = rel.as_str() == current_path.as_str();
            (rel.clone(), abs.clone(), is_current)
        })
        .collect();

    if rows.is_empty() {
        let mut k = DefaultHasher::new();
        "\u{0}<no-matches>".hash(&mut k);
        return KeyedSnapshot {
            items: vec![(k.finish(), 0)],
            build: Box::new(|c: &mut Commands, f: &EmberFonts, _| {
                picker_note(c, f, "No materials match".to_string(), 48.0)
            }),
        };
    }

    // Key = the project-relative path: stable identity that survives filtering, so
    // narrowing the search keeps surviving rows AND their thumbnail bindings.
    // Hash = only what is baked into the row at build time. The thumbnail
    // `Handle<Image>` is deliberately excluded — it arrives via the row's own
    // `bind_with`, and hashing it would make every thumbnail that resolves
    // despawn and rebuild its row.
    let mut items: Vec<(u64, u64)> = rows
        .iter()
        .map(|(rel, abs, is_current)| {
            let mut k = DefaultHasher::new();
            rel.hash(&mut k);
            let mut h = DefaultHasher::new();
            abs.hash(&mut h);
            is_current.hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();

    // One more "row" when the cap bit, hashed on the count so it re-renders as
    // typing narrows the field.
    let shown = rows.len();
    let truncated = total > shown;
    if truncated {
        let mut k = DefaultHasher::new();
        "\u{0}<truncated>".hash(&mut k);
        let mut h = DefaultHasher::new();
        total.hash(&mut h);
        items.push((k.finish(), h.finish()));
    }

    KeyedSnapshot {
        items,
        build: Box::new(move |c: &mut Commands, f: &EmberFonts, i: usize| {
            match rows.get(i) {
                Some((rel, abs, is_current)) => picker_tile(c, f, entity, rel, abs, *is_current),
                None => picker_note(
                    c,
                    f,
                    format!("Showing {shown} of {total} — type to narrow"),
                    22.0,
                ),
            }
        }),
    }
}

/// A full-width line in the grid — the empty state and the truncation note.
///
/// Full width so it takes a row of its own and centres, rather than landing in
/// the first tile's column.
fn picker_note(commands: &mut Commands, fonts: &EmberFonts, message: String, height: f32) -> Entity {
    let wrap = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(height),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .id();
    let text = commands
        .spawn((
            Text::new(message),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(wrap).add_child(text);
    wrap
}

/// One grid tile: a preview square over a clipped name.
///
/// The folder lives in the tooltip rather than on a second line — it only
/// matters when two materials share a name, and paying every tile a line of 9px
/// grey for that case is what made the old rows unreadable.
fn picker_tile(commands: &mut Commands, fonts: &EmberFonts, entity: Entity, rel: &str, abs: &str, is_current: bool) -> Entity {
    let path = std::path::Path::new(rel);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(rel).to_string();
    let parent = path
        .parent()
        .and_then(|p| p.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("project root");

    // The current material is marked three ways — tinted tile, accented preview
    // border, accented name — because at 78px one of them alone reads as noise.
    let base = if is_current { rgb(accent()).with_alpha(0.20) } else { Color::NONE };
    let tile = commands
        .spawn((
            Node {
                width: Val::Px(TILE_W),
                height: Val::Px(TILE_H),
                margin: UiRect::bottom(Val::Px(TILE_GAP)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(base),
            HoverTint::solid(base, rgb(hover_bg()), rgb(accent()).with_alpha(0.32)),
            Interaction::default(),
            bevy::ui::FocusPolicy::Block,
            HoverTooltip::new(format!("{stem}  ·  {parent}")),
            MatPickerItem { entity, rel: rel.to_string() },
            Name::new("material-picker-tile"),
        ))
        .id();

    let (preview, glyph) = preview_square(commands, fonts, TILE_W - 6.0, 4.0, 22.0);
    if is_current {
        commands.entity(preview).insert(BorderColor::all(rgb(accent())));
    }
    let abs_pb = PathBuf::from(abs);
    // Ask for the thumbnail from the tile's own build. A tile is only built when
    // it scrolls into the window, so opening the picker on a project with
    // hundreds of materials queues renders for the dozen actually on screen
    // rather than for all of them; `request` is a no-op once a path is cached or
    // in flight, so scrolling back over one costs nothing.
    let wanted = abs_pb.clone();
    commands.queue(move |w: &mut World| {
        if let Some(mut reg) = w.get_resource_mut::<MaterialThumbnailRegistry>() {
            reg.request(wanted);
        }
    });
    bind_preview(commands, preview, glyph, move |w| {
        w.get_resource::<MaterialThumbnailRegistry>().and_then(|r| r.handle(&abs_pb))
    });

    let name = commands
        .spawn((
            Text::new(stem),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(if is_current { accent() } else { text_primary() })),
            bevy::text::TextLayout::no_wrap(),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    // Clip wrapper again: `Overflow::clip` clips a node's *children*, so a long
    // name has to sit inside something rather than carry the clip itself.
    let name_clip = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(13.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.entity(name_clip).add_child(name);
    commands.entity(tile).add_children(&[preview, name_clip]);
    tile
}

fn bind_search(commands: &mut Commands, input: Entity) {
    use renzora_ember::widgets::bind_text_input;
    bind_text_input(
        commands,
        input,
        move |w| w.get_resource::<MatPickerFilter>().map(|f| f.text.clone()).unwrap_or_default(),
        move |w, s: String| {
            if let Some(mut f) = w.get_resource_mut::<MatPickerFilter>() {
                f.text = s;
            }
        },
    );
}

pub(super) fn mat_picker_select(q: Query<(&Interaction, &MatPickerItem), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, item) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (e, rel) = (item.entity, item.rel.clone());
        commands.queue(move |w: &mut World| {
            bind_material(w, e, rel);
            close_pickers(w);
        });
    }
}

/// The picker field's surface and border for the two tray states.
///
/// Accent-tinted while open, so the field reads as the thing the grid below
/// belongs to rather than as an unrelated control that happens to sit above it.
fn field_colors(open: bool) -> (Color, Color) {
    if open {
        (rgb(accent()).with_alpha(0.22), rgb(accent()))
    } else {
        (rgb(popup_bg()), rgb(border()))
    }
}

/// Click the field → slide its picker tray open or shut.
///
/// Opening it also folds the texture-slot rows away. They're the tallest thing
/// in the drawer and they're about the material you're in the middle of
/// *replacing*, so leaving them there pushed the grid off the bottom of the
/// panel and asked you to scroll past six rows that were on their way out.
pub(super) fn mat_picker_toggle(
    mut q: Query<
        (&Interaction, &MatPickerToggle, &mut HoverTint, &mut BackgroundColor, &mut BorderColor),
        Changed<Interaction>,
    >,
    tex_rows: Query<(Entity, &TexSlotZone)>,
    tex_toggles: Query<(Entity, &TexSlotsToggle)>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
) {
    for (interaction, toggle, mut tint, mut bg, mut bc) in &mut q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let open = nodes.get(toggle.panel).is_ok_and(|n| n.display == Display::None);
        if let Ok(mut node) = nodes.get_mut(toggle.panel) {
            node.display = if open { Display::Flex } else { Display::None };
        }
        set_caret(&mut texts, toggle.caret, open);
        set_texture_rows(&mut nodes, &tex_rows, &tex_toggles, Some(toggle.entity), !open);

        // `HoverTint.base` too, not just the background: ember's `hover_tint`
        // writes `base` back the moment the pointer leaves, so painting only the
        // background would hold the active colour exactly until you moved the
        // mouse off the field.
        let (fill, edge) = field_colors(open);
        tint.base = fill;
        bg.0 = fill;
        *bc = BorderColor::all(edge);
    }
}

/// Show or hide texture-slot rows — every one when `entity` is `None`, otherwise
/// only the rows belonging to that inspected entity.
fn set_texture_rows(
    nodes: &mut Query<&mut Node>,
    tex_rows: &Query<(Entity, &TexSlotZone)>,
    toggles: &Query<(Entity, &TexSlotsToggle)>,
    entity: Option<Entity>,
    visible: bool,
) {
    let want = if visible { Display::Flex } else { Display::None };
    // The expand footer goes with the rows it belongs to — left behind, it sits
    // under the open tray offering to reveal rows that aren't there.
    let rows = tex_rows
        .iter()
        .map(|(row, zone)| (row, zone.entity))
        .chain(toggles.iter().map(|(row, t)| (row, t.entity)));
    for (row, owner) in rows {
        if entity.is_some_and(|e| e != owner) {
            continue;
        }
        if let Ok(mut node) = nodes.get_mut(row) {
            if node.display != want {
                node.display = want;
            }
        }
    }
}

/// Point a field's caret at the tray's state.
fn set_caret(texts: &mut Query<&mut Text>, caret: Entity, open: bool) {
    set_glyph(texts, caret, if open { "caret-up" } else { "caret-down" });
}

/// Repoint an existing icon entity at another phosphor glyph, by name.
fn set_glyph(texts: &mut Query<&mut Text>, icon: Entity, name: &str) {
    let Some(glyph) = renzora_ember::phosphor_map::icon_glyph(name) else { return };
    if let Ok(mut text) = texts.get_mut(icon) {
        let want = glyph.to_string();
        if text.0 != want {
            *text = Text::new(want);
        }
    }
}

/// Shut every open picker tray and reset its search.
///
/// Picking used to leave the list sitting there — it only closed on an outside
/// click, so choosing a material took two clicks: one to choose, one to get the
/// grid off the drawer. Now that the tray is in flow that second click also cost
/// the texture slots their position on screen, which makes closing it on select
/// non-negotiable rather than merely tidy.
pub(super) fn close_pickers(world: &mut World) {
    let toggles: Vec<(Entity, Entity, Entity)> = world
        .query::<(Entity, &MatPickerToggle)>()
        .iter(world)
        .map(|(field, t)| (field, t.panel, t.caret))
        .collect();
    let (fill, edge) = field_colors(false);
    for (field, panel, caret) in toggles {
        if let Some(mut node) = world.get_mut::<Node>(panel) {
            if node.display == Display::None {
                continue;
            }
            node.display = Display::None;
        }
        if let Some(glyph) = renzora_ember::phosphor_map::icon_glyph("caret-down") {
            if let Some(mut text) = world.get_mut::<Text>(caret) {
                *text = Text::new(glyph.to_string());
            }
        }
        if let Some(mut tint) = world.get_mut::<HoverTint>(field) {
            tint.base = fill;
        }
        if let Some(mut bg) = world.get_mut::<BackgroundColor>(field) {
            bg.0 = fill;
        }
        if let Some(mut bc) = world.get_mut::<BorderColor>(field) {
            *bc = BorderColor::all(edge);
        }
    }
    // Every tray is shut now, so every texture row is due back — no need to
    // match them up per entity. The rebuild that follows a *changed* material
    // respawns them visible anyway; this covers the case where the pick landed
    // on the material already bound, which changes the drawer's signature not at
    // all and so rebuilds nothing.
    let rows: Vec<Entity> = world
        .query_filtered::<Entity, With<TexSlotZone>>()
        .iter(world)
        .collect();
    for row in rows {
        if let Some(mut node) = world.get_mut::<Node>(row) {
            if node.display != Display::Flex {
                node.display = Display::Flex;
            }
        }
    }
    // Reopening pre-filtered by a query you typed a minute ago reads as "there
    // are only two materials in this project".
    if let Some(mut filter) = world.get_resource_mut::<MatPickerFilter>() {
        if !filter.text.is_empty() {
            filter.text.clear();
        }
    }
}
