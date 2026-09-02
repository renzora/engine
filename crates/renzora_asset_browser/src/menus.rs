//! Every menu the browser opens: the Add button's create-asset list, the Import
//! two-row picker, the per-asset and background right-click menus, and the sort
//! dropdown — plus the hover tracking the right-click menus target off.

use std::path::PathBuf;

use bevy::prelude::*;

use renzora_ember::font::EmberFonts;
use renzora_ember::widgets::{
    menu_card, menu_header, menu_item, menu_item_styled, menu_sep, screen_menu,
    screen_menu_est_height, screen_menu_flip, screen_menu_under, trigger_rect,
};

use crate::interact::{open_action, open_from_menu};
use crate::ops::{
    create_asset, delete_asset, duplicate_asset, reveal_in_explorer, toggle_favorite,
};
use crate::rename::start_rename;
use crate::state::{
    unique_path, AddMenuBtn, AssetRoot, AssetTile, ImportBtn, NativeAssets, NewAsset, NewAssetBtn,
    SortMenuBtn, SortMode, TreeAddBtn, TreeNav,
};

pub(crate) fn create_asset_click(
    q: Query<(&Interaction, &NewAssetBtn), Changed<Interaction>>,
    mut state: ResMut<NativeAssets>,
    project: Option<Res<renzora::core::CurrentProject>>,
    settings: Option<Res<renzora_editor_framework::EditorSettings>>,
) {
    let boilerplate = settings.as_ref().is_none_or(|s| s.new_file_boilerplate);
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(folder) = state.current.clone().or_else(|| project.as_ref().map(|p| p.path.clone())) else {
            continue;
        };
        let kind = btn.0;
        let path = unique_path(&folder, kind.filename(), kind.is_folder());
        let ok = if kind.is_folder() {
            std::fs::create_dir_all(&path).is_ok()
        } else {
            std::fs::write(&path, kind.content(boilerplate)).is_ok()
        };
        if ok {
            state.selected = Some(path);
        }
    }
}

/// Click the toolbar's Import button → open a two-row menu asking *what* to
/// import, rather than going straight to a picker.
///
/// It used to insert [`ImportRequested`](renzora::core::ImportRequested) and let
/// the importer open the multi-select **file** dialog. That made folder import
/// unreachable and, worse, look broken: no OS dialog selects files and folders
/// in one pass, so clicking a folder in a file dialog just navigates into it —
/// which reads as "Import refuses to import a folder". One extra click buys a
/// choice that is actually visible.
pub(crate) fn import_click(
    q: Query<
        (
            &Interaction,
            &bevy::ui::RelativeCursorPosition,
            &bevy::ui::ComputedNode,
        ),
        (With<ImportBtn>, Changed<Interaction>),
    >,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else {
        return;
    };
    let Some((_, rcp, cn)) = q.iter().find(|(i, ..)| **i == Interaction::Pressed) else {
        return;
    };
    let Some((win_h, cursor)) = windows
        .iter()
        .find_map(|w| w.cursor_position().map(|c| (w.height(), c)))
    else {
        return;
    };
    // Hang the menu off the button's own box, flipping up only if the two rows
    // genuinely don't fit below. The window-half rule the Add menu uses is wrong
    // here: the Assets panel is docked at the bottom, so its toolbar is always in
    // the lower half and a short dropdown flipped up over the panel's own tabs
    // with the whole file grid free underneath it.
    let menu = screen_menu_under(
        &mut commands,
        trigger_rect(cursor, rcp, cn),
        win_h,
        screen_menu_est_height(2, 0),
    );
    let kids = import_menu_items(&mut commands, &fonts);
    commands.entity(menu).add_children(&kids);
}

/// The Files / Folder rows, shared by the Import button and both right-click
/// menus so no entry point can quietly offer only half the choice.
pub(crate) fn import_menu_items(commands: &mut Commands, fonts: &EmberFonts) -> Vec<Entity> {
    vec![
        menu_item(
            commands,
            fonts,
            "file",
            &renzora::lang::t("assets.import_files"),
            |w| request_import(w, renzora::core::ImportPick::Files),
        ),
        menu_item(
            commands,
            fonts,
            "folder-open",
            &renzora::lang::t("assets.import_folder"),
            |w| request_import(w, renzora::core::ImportPick::Folder),
        ),
        // The third way to get an asset into a project, beside the two local
        // ones. It belongs in this menu because "I need a tree" is the same
        // question whether the answer is on disk or in the store, and the store
        // was otherwise only reachable from the top-bar workspace switcher.
        //
        // Opens the marketplace rather than importing anything: what arrives
        // from it is chosen there and installed by its own overlay, which
        // already asks where to put it.
        //
        // Through the shell-action message, not a panel id — the Marketplace is
        // an overlay now, and this crate has never heard of the one that owns
        // it. The id is the only thing that crosses, and it lives in the
        // contract crate so both ends agree on the string.
        menu_item(
            commands,
            fonts,
            "storefront",
            &renzora::lang::t("assets.search_marketplace"),
            |w| renzora::ShellActionInvoked::invoke(w, renzora::ACTION_MARKETPLACE),
        ),
    ]
}

/// Queue an import targeting the browser's *current* folder — the one path
/// every Import row in this panel takes (menu rows run as world closures, not
/// as buttons a click system can see). The title bar's File menu and the
/// command palette insert the request themselves and set no target, so their
/// assets land in the importer's default folder instead.
fn request_import(world: &mut World, pick: renzora::core::ImportPick) {
    world.insert_resource(renzora::core::ImportRequested(pick));
    let Some(root) = world
        .get_resource::<renzora::core::CurrentProject>()
        .map(|p| p.path.clone())
    else {
        return;
    };
    let folder = world
        .get_resource::<NativeAssets>()
        .and_then(|s| s.current.clone())
        .unwrap_or_else(|| root.clone());
    // PROJECT-RELATIVE, forward-slashed — the overlay prefixes it with "assets/".
    let target_dir = folder
        .strip_prefix(&root)
        .ok()
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    if !target_dir.is_empty() {
        world.insert_resource(renzora::core::ImportTargetDir(target_dir));
    }
}

// ── Context menu ─────────────────────────────────────────────────────────────

pub(crate) fn track_hover(
    tiles: Query<(&Interaction, &AssetTile)>,
    tree: Query<(&Interaction, &TreeNav)>,
    mut state: ResMut<NativeAssets>,
) {
    let over = |i: &Interaction| matches!(i, Interaction::Hovered | Interaction::Pressed);
    // A hovered grid/list tile takes priority; otherwise a hovered tree folder
    // row — so right-click targeting (and the context-menu delete) works over the
    // folder tree too, not just the grid.
    let hovered = tiles
        .iter()
        .find(|(i, _)| over(i))
        .map(|(_, t)| t.path.clone())
        .or_else(|| tree.iter().find(|(i, _)| over(i)).map(|(_, n)| n.0.clone()));
    // Clear over empty space rather than letting the last hover stick. A sticky
    // path made a right-click on the empty grid open the per-asset menu (Rename /
    // Duplicate / Delete) for whatever the pointer happened to brush past on the
    // way there — and those items then acted on that asset. `None` is what tells
    // `asset_context_menu` to offer the folder-background menu instead.
    if state.hovered != hovered {
        state.hovered = hovered;
    }
}

/// Right-click a tile → open the shared ember menu (Favorite / Duplicate /
/// Reveal / Delete) for the hovered asset.
pub(crate) fn asset_context_menu(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    state: Res<NativeAssets>,
    roots: Query<&bevy::ui::RelativeCursorPosition, With<AssetRoot>>,
    mut commands: Commands,
) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    let Some(fonts) = fonts else {
        return;
    };
    if !roots.iter().any(|rcp| rcp.cursor_over) {
        return;
    }
    let Some(win) = windows.iter().find(|w| w.cursor_position().is_some()) else {
        return;
    };
    let Some(cursor) = win.cursor_position() else {
        return;
    };
    let win_h = win.height();
    // Nothing under the cursor → the click targets the *folder*, not an asset.
    let Some(path) = state.hovered.clone() else {
        background_context_menu(&mut commands, &fonts, &state, cursor, win_h);
        return;
    };
    let fav_label = if state.favorites.contains(&path) {
        renzora::lang::t("assets.context.unfavorite")
    } else {
        renzora::lang::t("assets.context.favorite")
    };
    // Flip the menu upward when the click lands low in the window so the (often
    // tall) "Create Asset" list grows up from the cursor instead of being clipped.
    let menu = screen_menu_flip(&mut commands, cursor.x, cursor.y, win_h);
    // Same color-coded "create new X" rows as the Add button, led by the
    // "Create Asset" header — at the TOP of the menu so a new asset can be made
    // straight from the right-click menu (lands in the current folder).
    let mut kids = new_asset_menu_items(&mut commands, &fonts);
    kids.push(menu_sep(&mut commands));
    // "Open in <Editor>" routes editor-backed assets to their panel/layout.
    if let Some((icon, label)) = open_action(&path) {
        kids.push(menu_item(&mut commands, &fonts, icon, &label, {
            let path = path.clone();
            move |w| open_from_menu(w, &path)
        }));
        kids.push(menu_sep(&mut commands));
    }
    kids.extend([
        menu_item(&mut commands, &fonts, "star", &fav_label, {
            let path = path.clone();
            move |w| toggle_favorite(w, &path)
        }),
        menu_item(&mut commands, &fonts, "pencil-simple", &renzora::lang::t("assets.context.rename"), {
            let path = path.clone();
            move |w| start_rename(w, &path)
        }),
        menu_item(&mut commands, &fonts, "copy", &renzora::lang::t("assets.context.duplicate"), {
            let path = path.clone();
            move |_| duplicate_asset(&path)
        }),
        menu_item(&mut commands, &fonts, "folder-open", &renzora::lang::t("assets.context.reveal"), {
            let path = path.clone();
            move |_| reveal_in_explorer(&path)
        }),
        menu_sep(&mut commands),
        menu_item_styled(&mut commands, &fonts, "trash", &renzora::lang::t("assets.context.delete"), (224, 96, 88), (224, 96, 88), {
            let path = path.clone();
            move |w| {
                // Right-clicking an item that's part of the current selection
                // deletes the whole selection; right-clicking an unselected item
                // deletes just it (mirrors the hierarchy's delete behaviour).
                let paths: Vec<PathBuf> = {
                    let state = w.resource::<NativeAssets>();
                    if state.is_selected(&path) {
                        let all: Vec<PathBuf> = state.selection.iter().cloned().collect();
                        if all.is_empty() { vec![path.clone()] } else { all }
                    } else {
                        vec![path.clone()]
                    }
                };
                for p in &paths {
                    delete_asset(w, p);
                }
            }
        }),
    ]);
    commands.entity(menu).add_children(&kids);
}

/// Right-click empty space → the menu for the folder you are *in*: New Folder,
/// Import, the create-asset list and Reveal. None of the per-asset actions
/// (Favorite / Rename / Duplicate / Delete) belong here, because there is no
/// asset for them to act on.
fn background_context_menu(
    commands: &mut Commands,
    fonts: &EmberFonts,
    state: &NativeAssets,
    cursor: Vec2,
    win_h: f32,
) {
    // Same upward flip as the per-asset menu: the create-asset list is tall
    // enough to be clipped by a click low in the window.
    let menu = screen_menu_flip(commands, cursor.x, cursor.y, win_h);
    let mut kids = vec![menu_item(
        commands,
        fonts,
        "folder-plus",
        &renzora::lang::t("assets.new_folder"),
        |w| create_asset(w, NewAsset::Folder),
    )];
    kids.extend(import_menu_items(commands, fonts));
    kids.push(menu_sep(commands));
    kids.extend(new_asset_menu_items(commands, fonts));
    if let Some(folder) = state.current.clone() {
        kids.push(menu_sep(commands));
        kids.push(menu_item(
            commands,
            fonts,
            "folder-open",
            &renzora::lang::t("assets.context.reveal"),
            move |_| reveal_in_explorer(&folder),
        ));
    }
    commands.entity(menu).add_children(&kids);
}

/// Click an "Add" button → open the shared ember menu of new-asset types at the
/// cursor. The tree strip's "+" ([`TreeAddBtn`]) leads that list with New Folder
/// and Import: in the tree-only layout it is the *only* action key on screen, so
/// the menu has to carry everything the hidden toolbar would have offered.
pub(crate) fn add_menu_open(
    q: Query<
        (
            &Interaction,
            &bevy::ui::RelativeCursorPosition,
            &bevy::ui::ComputedNode,
            Has<TreeAddBtn>,
        ),
        (With<AddMenuBtn>, Changed<Interaction>),
    >,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else {
        return;
    };
    let Some((_, rcp, cn, file_actions)) = q.iter().find(|(i, ..)| **i == Interaction::Pressed) else {
        return;
    };
    let Some((win_h, cursor)) = windows
        .iter()
        .find_map(|w| w.cursor_position().map(|c| (w.height(), c)))
    else {
        return;
    };
    // Anchor the menu to the button's bottom-left (stable, cursor-independent) —
    // except for the narrow layout's bottom action bar, where "below the button"
    // is off-screen. There, grow the menu up from the button's top edge instead.
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    let menu = if top_left.y > win_h * 0.5 {
        screen_menu_flip(&mut commands, top_left.x, top_left.y - 2.0, win_h)
    } else {
        screen_menu(&mut commands, top_left.x, top_left.y + size.y + 2.0)
    };
    let mut kids = Vec::new();
    if file_actions {
        kids.push(menu_item(
            &mut commands,
            &fonts,
            "folder-plus",
            &renzora::lang::t("assets.new_folder"),
            |w| create_asset(w, NewAsset::Folder),
        ));
        kids.extend(import_menu_items(&mut commands, &fonts));
        kids.push(menu_sep(&mut commands));
    }
    kids.extend(new_asset_menu_items(&mut commands, &fonts));
    commands.entity(menu).add_children(&kids);
}

/// The color-coded "create new X" rows shared by the Add button and the
/// right-click menu, led by an Unreal-style "Create Asset" section header. Each
/// row carries the type's accent color (icon + label) so the menu reads as the
/// same color language as the asset tiles.
fn new_asset_menu_items(commands: &mut Commands, fonts: &EmberFonts) -> Vec<Entity> {
    let mut kids = vec![menu_header(commands, fonts, &renzora::lang::t("assets.new.header"))];
    kids.extend(NewAsset::MENU.iter().map(|&kind| {
        menu_card(
            commands,
            fonts,
            kind.icon(),
            &kind.label(),
            &kind.subtitle(),
            kind.color(),
            move |w| create_asset(w, kind),
        )
    }));
    kids
}

/// Open the sort menu (modes + ascending/descending) anchored under the button.
pub(crate) fn sort_menu_open(
    q: Query<
        (
            &Interaction,
            &bevy::ui::RelativeCursorPosition,
            &bevy::ui::ComputedNode,
        ),
        (With<SortMenuBtn>, Changed<Interaction>),
    >,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    state: Option<Res<NativeAssets>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else {
        return;
    };
    let Some((_, rcp, cn)) = q.iter().find(|(i, _, _)| **i == Interaction::Pressed) else {
        return;
    };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else {
        return;
    };
    let (cur_sort, cur_desc) = state.map(|s| (s.sort, s.sort_desc)).unwrap_or((SortMode::Name, false));
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    let menu = screen_menu(&mut commands, top_left.x, top_left.y + size.y + 2.0);
    let mut kids: Vec<Entity> = SortMode::ALL
        .iter()
        .map(|&mode| {
            let icon = if mode == cur_sort { "check" } else { "dot" };
            menu_item(&mut commands, &fonts, icon, &mode.label(), move |w| {
                if let Some(mut s) = w.get_resource_mut::<NativeAssets>() {
                    s.sort = mode;
                }
            })
        })
        .collect();
    kids.push(menu_sep(&mut commands));
    kids.push(menu_item(
        &mut commands,
        &fonts,
        if cur_desc { "arrow-up" } else { "check" },
        &renzora::lang::t("assets.sort.ascending"),
        |w| {
            if let Some(mut s) = w.get_resource_mut::<NativeAssets>() {
                s.sort_desc = false;
            }
        },
    ));
    kids.push(menu_item(
        &mut commands,
        &fonts,
        if cur_desc { "check" } else { "arrow-down" },
        &renzora::lang::t("assets.sort.descending"),
        |w| {
            if let Some(mut s) = w.get_resource_mut::<NativeAssets>() {
                s.sort_desc = true;
            }
        },
    ));
    commands.entity(menu).add_children(&kids);
}
