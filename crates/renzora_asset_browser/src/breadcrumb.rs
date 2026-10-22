//! The breadcrumb in the toolbar: one chip per folder from the project root to
//! the folder on screen, a chevron after each that lists its subfolders, and a
//! "…" that stands in for the levels that do not fit.
//!
//! It used to have a row of its own under the toolbar. Moving it into the
//! toolbar gave the grid that row back, and the toolbar has the room: after
//! Import and New Folder there are hundreds of pixels before the search box on
//! any dock wide enough to show the grid at all.
//!
//! **Folding.** Every level shows while the path fits. When it would push into
//! the search box, levels fold into "…" from the root side, one at a time, so
//! the project root and the levels nearest the folder on screen stay readable
//! longest; at the very narrowest only the root, "…" and the current folder are
//! left. The fold is measured, not guessed from a character count: a wider dock
//! simply shows more. [`fit_breadcrumb`] works from the laid-out widths and only
//! runs when one of them moved, so a still panel costs nothing.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};
use bevy::window::SystemCursorIcon;

use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::bind_bg;
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::{rgb, text_muted, text_primary};
use renzora_ember::widgets::{
    ellipsize, menu_item, menu_item_styled, screen_menu_est_height, screen_menu_under,
    trigger_rect,
};
use renzora_ember::UiInteraction;

use crate::ops::{current_folder, folder_color, project_root};
use crate::state::{
    is_hidden_name, CrumbBar, CrumbEllipsisBtn, CrumbMenuBtn, CrumbNav, CrumbSeg, CrumbSpacer,
    NativeAssets,
};

/// A folder name's widest showing before it ends in "…". The folder on screen
/// gets more room: it is the one level you always need to read.
const SEG_MAX_W: f32 = 140.0;
const CURRENT_MAX_W: f32 = 220.0;

/// Gap between breadcrumb items; also what [`fit_breadcrumb`] adds between the
/// children it sums.
pub(crate) const CRUMB_GAP: f32 = 2.0;

/// Width the "…" button takes once it appears, for the first fold's budget.
const ELLIPSIS_W: f32 = 28.0;

/// The levels from the project root to the folder on screen: `(name, path)`.
fn levels(world: &Rx) -> Vec<(String, PathBuf)> {
    let Some(root) = project_root(world) else {
        return Vec::new();
    };
    let cur = current_folder(world).unwrap_or_else(|| root.clone());
    let root_name = root
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| renzora::lang::t("common.project"));
    let mut out = vec![(root_name, root.clone())];
    if let Ok(rel) = cur.strip_prefix(&root) {
        let mut acc = root.clone();
        for comp in rel.components() {
            acc = acc.join(comp);
            out.push((comp.as_os_str().to_string_lossy().to_string(), acc.clone()));
        }
    }
    out
}

/// How many levels may fold: everything between the root and the current
/// folder.
fn max_hidden(levels: usize) -> usize {
    levels.saturating_sub(2)
}

enum CrumbItem {
    Level { name: String, path: PathBuf, current: bool },
    Ellipsis(Vec<PathBuf>),
}

pub(crate) fn crumb_snapshot(world: &Rx) -> KeyedSnapshot {
    let levels = levels(world);
    if levels.is_empty() {
        return KeyedSnapshot {
            items: Vec::new(),
            build: Box::new(|_, _, _| Entity::PLACEHOLDER),
        };
    }
    let hidden = world
        .get_resource::<NativeAssets>()
        .map(|s| s.crumb_hidden)
        .unwrap_or(0)
        .min(max_hidden(levels.len()));
    let last = levels.len() - 1;
    let mut items: Vec<CrumbItem> = Vec::new();
    for (i, (name, path)) in levels.iter().enumerate() {
        if i >= 1 && i <= hidden {
            if i == 1 {
                let folded = levels[1..=hidden].iter().map(|(_, p)| p.clone()).collect();
                items.push(CrumbItem::Ellipsis(folded));
            }
            continue;
        }
        items.push(CrumbItem::Level { name: name.clone(), path: path.clone(), current: i == last });
    }
    let keys: Vec<(u64, u64)> = items
        .iter()
        .map(|item| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            let mut h = std::collections::hash_map::DefaultHasher::new();
            match item {
                CrumbItem::Level { name, path, current } => {
                    path.hash(&mut k);
                    (name, current).hash(&mut h);
                }
                CrumbItem::Ellipsis(folded) => {
                    "ellipsis".hash(&mut k);
                    folded.hash(&mut h);
                }
            }
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items: keys,
        build: Box::new(move |c, f, i| match &items[i] {
            CrumbItem::Level { name, path, current } => level(c, f, name, path, *current),
            CrumbItem::Ellipsis(folded) => ellipsis(c, f, folded),
        }),
    }
}

/// One level: a clickable chip (navigates there, and is a drop target) plus a
/// chevron that lists the level's subfolders. The folder on screen is lit and
/// ends in a caret instead, since there is no level after it to point at.
fn level(commands: &mut Commands, fonts: &EmberFonts, name: &str, path: &Path, current: bool) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(1.0),
                // Fixed size: the bar's overflow is how a fold is detected, so
                // its children must not shrink to hide it.
                flex_shrink: 0.0,
                ..default()
            },
            CrumbSeg(path.to_path_buf()),
            Name::new("crumb-level"),
        ))
        .id();
    let chip = commands
        .spawn((
            Node {
                height: Val::Px(26.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::horizontal(Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            CrumbNav(path.to_path_buf()),
            Name::new("crumb"),
        ))
        .id();
    // The folder on screen opens its subfolder menu from its own chip, the way
    // the other levels do from their chevron: clicking it would only navigate
    // to where you already are.
    if current {
        commands
            .entity(chip)
            .insert((CrumbMenuBtn(path.to_path_buf()), RelativeCursorPosition::default()));
    }
    // The drag tint binds on every chip, the current one included: it is the
    // folder you are looking at, and still a place to drop things.
    bind_bg(commands, chip, move |w| {
        if let Some(tint) = crate::drag_drop::drop_target_tint(w, chip, true) {
            return tint;
        }
        let hovered = matches!(
            w.get::<UiInteraction>(chip),
            Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed)
        );
        // No pill at rest, the current folder included: its brighter text and
        // caret already mark it, and the standing grey pill read as a stray
        // button. See [`crumb_hover`] for the hover colour.
        if hovered {
            crumb_hover(w)
        } else {
            Color::NONE
        }
    });
    let label = commands
        .spawn((
            Text::new(name.to_string()),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(if current { text_primary() } else { text_muted() })),
            Node {
                max_width: Val::Px(if current { CURRENT_MAX_W } else { SEG_MAX_W }),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    ellipsize(commands, label, name.to_string(), Some(chip));
    // The label must not shrink: only its `max_width` may shorten it. Left
    // shrinkable (as `ellipsize` makes it), layout squeezed every label before
    // the bar could overflow, so `fit_breadcrumb` never folded a level and
    // each label ellipsized itself down to a lone "...". Holding them at their
    // own width puts the pressure back on the bar, where the fold handles it.
    commands.entity(label).entry::<Node>().and_modify(|mut n| n.flex_shrink = 0.0);
    let mut kids = vec![label];
    if current {
        let caret = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 11.0);
        commands.entity(caret).insert(Pickable::IGNORE);
        kids.push(caret);
    }
    commands.entity(chip).add_children(&kids);

    let mut row_kids = vec![chip];
    if !current {
        row_kids.push(chevron(commands, fonts, path));
    }
    commands.entity(row).add_children(&row_kids);
    row
}

/// The hover fill for every breadcrumb button: the theme's button hover, the
/// same one the New Folder button beside the path takes, so the toolbar's
/// buttons and the path's segments answer a hover alike.
fn crumb_hover(w: &renzora_ember::reactive::Rx) -> Color {
    w.get_resource::<renzora_ember::style::Theme>()
        .map(|t| t.button.bg_hover.color())
        .unwrap_or_else(|| rgb(renzora_ember::theme::hover_bg()))
}

/// The separator after a level, which is also the button for that level's
/// subfolder menu.
fn chevron(commands: &mut Commands, fonts: &EmberFonts, path: &Path) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(16.0),
                height: Val::Px(26.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            RelativeCursorPosition::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            CrumbMenuBtn(path.to_path_buf()),
            Name::new("crumb-chevron"),
        ))
        .id();
    bind_bg(commands, btn, move |w| match w.get::<UiInteraction>(btn) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => crumb_hover(w),
        _ => Color::NONE,
    });
    let glyph = icon_text(commands, &fonts.phosphor, "caret-right", text_muted(), 10.0);
    commands.entity(glyph).insert(Pickable::IGNORE);
    commands.entity(btn).add_child(glyph);
    btn
}

/// The "…" standing in for the folded levels; opens them as a menu.
fn ellipsis(commands: &mut Commands, fonts: &EmberFonts, folded: &[PathBuf]) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(1.0),
                flex_shrink: 0.0,
                ..default()
            },
            Name::new("crumb-ellipsis-row"),
        ))
        .id();
    let btn = commands
        .spawn((
            Node {
                height: Val::Px(26.0),
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(7.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            RelativeCursorPosition::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(renzora::lang::t_or(
                "assets.crumb.hidden",
                "Show hidden folders",
            )),
            CrumbEllipsisBtn(folded.to_vec()),
            Name::new("crumb-ellipsis"),
        ))
        .id();
    bind_bg(commands, btn, move |w| match w.get::<UiInteraction>(btn) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => crumb_hover(w),
        _ => Color::NONE,
    });
    let dots = commands
        .spawn((
            Text::new("\u{2026}"),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_primary())),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(btn).add_child(dots);
    let sep = icon_text(commands, &fonts.phosphor, "caret-right", text_muted(), 10.0);
    commands.entity(sep).insert((Pickable::IGNORE, Node { width: Val::Px(16.0), justify_content: JustifyContent::Center, ..default() }));
    commands.entity(row).add_children(&[btn, sep]);
    row
}

/// Fold or unfold breadcrumb levels to fit the toolbar.
///
/// The bar may shrink and its children may not, so a path that does not fit
/// shows up as the children summing wider than the bar; the spacer after it is
/// the room left over when it does fit. A level folds when the bar overflows and
/// comes back when the spacer has room for its last measured width, so the two
/// rules cannot chase each other.
///
/// Gated on something having moved: the bar's or the spacer's size, a level
/// laid out for the first time, or a new path. Otherwise it returns at once.
pub(crate) fn fit_breadcrumb(
    bar: Query<(Ref<ComputedNode>, Option<&Children>), With<CrumbBar>>,
    spacer: Query<Ref<ComputedNode>, With<CrumbSpacer>>,
    segs: Query<(Ref<ComputedNode>, &CrumbSeg)>,
    nodes: Query<&ComputedNode>,
    project: Option<Res<renzora::core::CurrentProject>>,
    mut state: ResMut<NativeAssets>,
    mut widths: Local<HashMap<PathBuf, f32>>,
    mut last_path: Local<Option<PathBuf>>,
) {
    let Ok((bar_cn, children)) = bar.single() else {
        return;
    };
    let Ok(spacer_cn) = spacer.single() else {
        return;
    };
    let Some(root) = project.map(|p| p.path.clone()) else {
        return;
    };
    let current = state.current.clone().unwrap_or_else(|| root.clone());
    let path_changed = last_path.as_ref() != Some(&current);
    let segs_changed = segs.iter().any(|(cn, _)| cn.is_changed());
    if !(path_changed || segs_changed || bar_cn.is_changed() || spacer_cn.is_changed()) {
        return;
    }
    *last_path = Some(current.clone());

    let logical = |cn: &ComputedNode| cn.size().x * cn.inverse_scale_factor();
    for (cn, seg) in &segs {
        let w = logical(&cn);
        if w > 0.0 {
            widths.insert(seg.0.clone(), w);
        }
    }

    let n_levels = current
        .strip_prefix(&root)
        .map(|rel| rel.components().count() + 1)
        .unwrap_or(1);
    let max = max_hidden(n_levels);
    let mut hidden = state.crumb_hidden.min(max);
    if path_changed {
        // A new path starts unfolded and folds below from what it measures;
        // carrying the old count over would hide levels the new path may not
        // even have to hide.
        hidden = 0;
    }

    let bar_w = logical(&bar_cn);
    let kids: Vec<f32> = children
        .map(|c| c.iter().filter_map(|e| nodes.get(e).ok()).map(logical).collect())
        .unwrap_or_default();
    let content: f32 = kids.iter().sum::<f32>() + CRUMB_GAP * kids.len().saturating_sub(1) as f32;
    let level_path = |i: usize| -> PathBuf {
        let mut p = root.clone();
        if let Ok(rel) = current.strip_prefix(&root) {
            for comp in rel.components().take(i) {
                p = p.join(comp);
            }
        }
        p
    };
    // Nothing laid out yet (a fresh build): measure first, decide next frame.
    if bar_w <= 0.0 || kids.is_empty() || kids.iter().all(|w| *w <= 0.0) {
        if state.crumb_hidden != hidden {
            state.crumb_hidden = hidden;
        }
        return;
    }

    if content > bar_w + 1.0 && !path_changed {
        let mut need = content - bar_w;
        if hidden == 0 {
            need += ELLIPSIS_W;
        }
        while need > 0.0 && hidden < max {
            hidden += 1;
            need -= widths.get(&level_path(hidden)).copied().unwrap_or(80.0) + CRUMB_GAP;
        }
    } else if hidden > 0 && !path_changed {
        let free = logical(&spacer_cn) + if hidden == 1 { ELLIPSIS_W } else { 0.0 };
        let back = widths.get(&level_path(hidden)).copied().unwrap_or(f32::MAX);
        if free > back + 8.0 {
            hidden -= 1;
        }
    }
    if state.crumb_hidden != hidden {
        state.crumb_hidden = hidden;
    }
}

/// A folder's visible subfolders, sorted by name.
fn subfolders(folder: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(folder)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .filter(|e| !is_hidden_name(&e.file_name().to_string_lossy()))
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|p| crate::state::file_name_of(p).to_lowercase());
    out
}

fn navigate(world: &mut World, path: PathBuf) {
    if let Some(mut s) = world.get_resource_mut::<NativeAssets>() {
        s.current = Some(path);
        s.selected = None;
    }
}

/// A chevron (or the current folder's chip) pressed: list that folder's
/// subfolders under it. The one on the path to the folder on screen is ticked,
/// so the menu also says where you came through.
pub(crate) fn crumb_menu_open(
    q: Query<(&UiInteraction, &RelativeCursorPosition, &ComputedNode, &CrumbMenuBtn), Changed<UiInteraction>>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    state: Res<NativeAssets>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, rcp, cn, btn)) = q.iter().find(|(i, ..)| **i == UiInteraction::Pressed) else {
        return;
    };
    let Some((win_h, cursor)) = windows.iter().find_map(|w| w.cursor_position().map(|c| (w.height(), c))) else {
        return;
    };
    let current = state.current.clone();
    let folders = subfolders(&btn.0);
    let menu = screen_menu_under(
        &mut commands,
        trigger_rect(cursor, rcp, cn),
        win_h,
        screen_menu_est_height(folders.len().max(1), 0),
    );
    let mut kids = Vec::new();
    if folders.is_empty() {
        kids.push(menu_item_styled(
            &mut commands,
            &fonts,
            "folder-dashed",
            &renzora::lang::t_or("assets.crumb.no_subfolders", "No subfolders"),
            text_muted(),
            text_muted(),
            |_| {},
        ));
    }
    for folder in folders {
        let name = crate::state::file_name_of(&folder);
        let on_path = current.as_ref().is_some_and(|c| c.starts_with(&folder));
        let icon = if on_path { "check" } else { "folder" };
        let row = menu_item_styled(
            &mut commands,
            &fonts,
            icon,
            &name,
            if on_path { renzora_ember::theme::accent() } else { folder_color(&name) },
            text_primary(),
            move |w| navigate(w, folder.clone()),
        );
        kids.push(row);
    }
    commands.entity(menu).add_children(&kids);
}

/// "…" pressed: the folded levels as a menu, indented in path order, to jump
/// straight to any of them.
pub(crate) fn crumb_ellipsis_open(
    q: Query<(&UiInteraction, &RelativeCursorPosition, &ComputedNode, &CrumbEllipsisBtn), Changed<UiInteraction>>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, rcp, cn, btn)) = q.iter().find(|(i, ..)| **i == UiInteraction::Pressed) else {
        return;
    };
    let Some((win_h, cursor)) = windows.iter().find_map(|w| w.cursor_position().map(|c| (w.height(), c))) else {
        return;
    };
    let menu = screen_menu_under(
        &mut commands,
        trigger_rect(cursor, rcp, cn),
        win_h,
        screen_menu_est_height(btn.0.len(), 0),
    );
    let mut kids = Vec::new();
    for (depth, folder) in btn.0.iter().enumerate() {
        let name = crate::state::file_name_of(folder);
        let target = folder.clone();
        let row = menu_item(&mut commands, &fonts, "folder", &name, move |w| navigate(w, target.clone()));
        // Indented one step per level, so the list reads as the path it is.
        commands.entity(row).entry::<Node>().and_modify(move |mut n| {
            n.padding.left = Val::Px(8.0 + depth as f32 * 10.0);
        });
        kids.push(row);
    }
    commands.entity(menu).add_children(&kids);
}
