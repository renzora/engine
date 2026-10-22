//! Every menu the browser opens: the Import overlay (import tiles over the
//! create-asset list), the per-asset and background right-click menus, and the
//! sort dropdown, plus the hover tracking the right-click menus target off.

use std::path::PathBuf;

use bevy::prelude::*;

use renzora_ember::font::EmberFonts;
use renzora_ember::widgets::{
    clipboard, menu_card, menu_header, menu_item, menu_item_styled, menu_sep, menu_submenu, overlay_val_parts,
    screen_menu, screen_menu_flip, MenuAction, ScreenMenu,
};
use renzora_ember::UiInteraction;

use crate::interact::{open_action, open_from_menu};
use crate::ops::{
    create_asset, delete_asset, duplicate_asset, reveal_in_explorer, toggle_favorite,
};
use crate::rename::start_rename;
use crate::state::{
    unique_path, AddMenuBtn, AssetRoot, AssetTile, NativeAssets, NewAsset, NewAssetBtn,
    RenameSurface, SortMenuBtn, SortMode, TreeAddBtn, TreeNav,
};

pub(crate) fn create_asset_click(
    q: Query<(&UiInteraction, &NewAssetBtn), Changed<UiInteraction>>,
    mut state: ResMut<NativeAssets>,
    project: Option<Res<renzora::core::CurrentProject>>,
    settings: Option<Res<renzora_editor_framework::EditorSettings>>,
) {
    let boilerplate = settings.as_ref().is_none_or(|s| s.new_file_boilerplate);
    for (interaction, btn) in &q {
        if *interaction != UiInteraction::Pressed {
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
            // Force the rescan rather than waiting on the half-second throttle:
            // the tile has to exist before the keyed-list rebuild can put a
            // rename field in it, and a New Folder that appears half a second
            // after the click reads as the click having missed.
            state.listing_dirty = true;
            // Straight into the rename, matching the menu row that creates the
            // same thing ([`crate::ops::create_asset`]) — this button was the
            // one entry point that only selected the new folder, so New Folder
            // from the toolbar meant finding it again and pressing F2.
            //
            // The narrow layout has no grid to draw the field on (the tree
            // shows files there instead), the same exception `rename_shortcut`
            // makes for F2.
            if kind.is_folder() {
                let surface = if state.narrow {
                    RenameSurface::Tree
                } else {
                    RenameSurface::Grid
                };
                state.begin_rename(&path, surface);
            }
        }
    }
}

/// The Files / Folder / Marketplace rows, shared by both right-click menus so
/// no entry point can quietly offer only half the choice. The Import button's
/// overlay offers the same three as tiles (see [`add_menu_open`]).
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
    tiles: Query<(&UiInteraction, &AssetTile)>,
    tree: Query<(&UiInteraction, &TreeNav)>,
    mut state: ResMut<NativeAssets>,
) {
    let over = |i: &UiInteraction| matches!(i, UiInteraction::Hovered | UiInteraction::Pressed);
    // A hovered grid/list tile takes priority; otherwise a hovered tree folder
    // row — so right-click targeting (and the context-menu delete) works over the
    // folder tree too, not just the grid.
    // The surface is recorded alongside the path: a right-click Rename has to
    // open its field where the thing you right-clicked is drawn, and by the time
    // the menu item fires there is nothing left to work that out from.
    //
    // A hovered tile is a *tree* row in the narrow layout: that is where the
    // browser draws its files once the grid is hidden, and a `Grid` rename there
    // opens a focused field behind a `Display::None` node.
    let mut surface = if state.narrow {
        RenameSurface::Tree
    } else {
        RenameSurface::Grid
    };
    let hovered = tiles
        .iter()
        .find(|(i, _)| over(i))
        .map(|(_, t)| t.path.clone())
        .or_else(|| {
            tree.iter().find(|(i, _)| over(i)).map(|(_, n)| {
                surface = RenameSurface::Tree;
                n.0.clone()
            })
        });
    if state.hovered_surface != surface {
        state.hovered_surface = surface;
    }
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
    registry: Option<Res<renzora::CreateMenuRegistry>>,
    actions: Option<Res<renzora::AssetActionRegistry>>,
    project: Option<Res<renzora::core::CurrentProject>>,
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
        let folder = state.current.clone().or_else(|| project.as_ref().map(|p| p.path.clone()));
        background_context_menu(&mut commands, &fonts, &state, cursor, win_h, registry.as_deref(), actions.as_deref(), folder);
        return;
    };
    let fav_label = if state.favorites.contains(&path) {
        renzora::lang::t("assets.context.unfavorite")
    } else {
        renzora::lang::t("assets.context.favorite")
    };
    // Flip the menu upward when the click lands low in the window, so it grows
    // up from the cursor instead of being clipped.
    let menu = screen_menu_flip(&mut commands, cursor.x, cursor.y, win_h);
    // Actions on the asset first, Delete last, and "make something new" folded
    // into one New submenu. The create list used to lead this menu, nine
    // tall cards of it, which pushed Delete below the menu's height cap: the
    // thing you right-clicked an asset to do was behind a scroll.
    let mut kids = Vec::new();
    // "Open in <Editor>" routes editor-backed assets to their panel/layout.
    if let Some((icon, label)) = open_action(&path) {
        kids.push(menu_item(&mut commands, &fonts, icon, &label, {
            let path = path.clone();
            move |w| open_from_menu(w, &path)
        }));
    }
    // Rows other crates registered for this kind of file (or folder), right
    // under Open: they are about what this asset *is*, like Open, rather than
    // the file housekeeping below.
    kids.extend(registered_action_items(&mut commands, &fonts, actions.as_deref(), &path, path.is_dir()));
    kids.extend([
        menu_item(&mut commands, &fonts, "pencil-simple", &renzora::lang::t("assets.context.rename"), {
            let path = path.clone();
            let surface = state.hovered_surface;
            move |w| start_rename(w, &path, surface)
        }),
        menu_item(&mut commands, &fonts, "copy", &renzora::lang::t("assets.context.duplicate"), {
            let path = path.clone();
            move |_| duplicate_asset(&path)
        }),
        menu_item(&mut commands, &fonts, "star", &fav_label, {
            let path = path.clone();
            move |w| toggle_favorite(w, &path)
        }),
        menu_sep(&mut commands),
        menu_item(
            &mut commands,
            &fonts,
            "clipboard",
            &renzora::lang::t("assets.context.copy_path"),
            {
                let path = path.clone();
                move |world| {
                    if let Some(relative) = world
                        .get_resource::<renzora::core::CurrentProject>()
                        .and_then(|project| project.make_relative(&path))
                    {
                        clipboard::set_text(&relative);
                    }
                }
            },
        ),
        menu_item(&mut commands, &fonts, "folder-open", &renzora::lang::t("assets.context.reveal"), {
            let path = path.clone();
            move |_| reveal_in_explorer(&path)
        }),
        menu_sep(&mut commands),
    ]);
    // New ▸ : the same colour-coded create rows as the Add button (landing in
    // the current folder), then Folder, in a hover panel of their own.
    let (new_row, new_content) =
        menu_submenu(&mut commands, &fonts, "plus", &renzora::lang::t_or("assets.context.new", "New"));
    let mut new_kids = new_asset_rows(&mut commands, &fonts, registry.as_deref());
    new_kids.push(menu_sep(&mut commands));
    new_kids.push(menu_item(
        &mut commands,
        &fonts,
        "folder-plus",
        &renzora::lang::t("assets.new_folder"),
        |w| create_asset(w, NewAsset::Folder),
    ));
    commands.entity(new_content).add_children(&new_kids);
    kids.extend([
        new_row,
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

/// Right-click empty space → the menu for the folder you are *in*: the
/// create-asset list, then New Folder, Import and Reveal. None of the per-asset
/// actions (Favorite / Rename / Duplicate / Delete) belong here, because there
/// is no asset for them to act on.
///
/// Create Asset leads, here and in [`add_menu_open`], because it is what the
/// menu is *for*. Making something is the common case and New Folder is the
/// housekeeping around it, but the housekeeping is one line each and the
/// create list is a block of cards, so putting the short rows first buried the
/// answer under the question every time.
#[allow(clippy::too_many_arguments)]
fn background_context_menu(
    commands: &mut Commands,
    fonts: &EmberFonts,
    state: &NativeAssets,
    cursor: Vec2,
    win_h: f32,
    registry: Option<&renzora::CreateMenuRegistry>,
    actions: Option<&renzora::AssetActionRegistry>,
    folder: Option<PathBuf>,
) {
    // Same upward flip as the per-asset menu: the create-asset list is tall
    // enough to be clipped by a click low in the window.
    let menu = screen_menu_flip(commands, cursor.x, cursor.y, win_h);
    let mut kids = new_asset_menu_items(commands, fonts, registry);
    // Actions on the folder being shown ("Convert all .particle files here").
    if let Some(folder) = folder {
        let rows = registered_action_items(commands, fonts, actions, &folder, true);
        if !rows.is_empty() {
            kids.push(menu_sep(commands));
            kids.extend(rows);
        }
    }
    kids.push(menu_sep(commands));
    kids.push(menu_item(
        commands,
        fonts,
        "folder-plus",
        &renzora::lang::t("assets.new_folder"),
        |w| create_asset(w, NewAsset::Folder),
    ));
    kids.extend(import_menu_items(commands, fonts));
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

/// Menu rows for the actions other crates registered against `path`: its file
/// actions, or (for a folder) the folder actions that have a matching file in
/// it. See [`renzora::asset_actions`].
fn registered_action_items(
    commands: &mut Commands,
    fonts: &EmberFonts,
    actions: Option<&renzora::AssetActionRegistry>,
    path: &std::path::Path,
    is_dir: bool,
) -> Vec<Entity> {
    let Some(actions) = actions else {
        return Vec::new();
    };
    let matching: Vec<&renzora::AssetAction> = if is_dir {
        actions.for_folder(path)
    } else {
        actions.for_file(path).collect()
    };
    matching
        .into_iter()
        .map(|action| {
            let run = action.run;
            let path = path.to_path_buf();
            menu_item(commands, fonts, &action.icon, &action.label_text(), move |w| run(w, &path))
        })
        .collect()
}

/// Click an Import button → open the Import overlay, centred. A button marked
/// [`TreeAddBtn`] (both Import keys are) leads it with the ways to bring files
/// in, then the list of assets to create; an [`AddMenuBtn`] without the mark
/// opens on the create list alone.
///
/// An overlay rather than a menu hung off the button: the Assets panel is
/// usually docked at the bottom, so a list this long had no room below its
/// button and opened as a tall drop-up over the panels above, reading as
/// something that had gone wrong. Centred, it has the whole window.
///
/// The overlay root also carries [`ScreenMenu`], which is what makes picking a
/// row close it: `menu_action_run` runs the row's action and despawns every
/// screen menu. It has no `RelativeCursorPosition`, so the screen-menu
/// click-outside rule (which would need one) leaves it to the overlay's own
/// dismissal, and its full-window box is already inside the screen clamp.
pub(crate) fn add_menu_open(
    registry: Option<Res<renzora::CreateMenuRegistry>>,
    q: Query<(&UiInteraction, Has<TreeAddBtn>), (With<AddMenuBtn>, Changed<UiInteraction>)>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else {
        return;
    };
    let Some((_, file_actions)) = q.iter().find(|(i, ..)| **i == UiInteraction::Pressed) else {
        return;
    };
    let title = if file_actions {
        renzora::lang::t("assets.import")
    } else {
        renzora::lang::t("assets.new.header")
    };
    let parts = overlay_val_parts(&mut commands, &fonts, &title, Val::Px(440.0), Val::Auto, true);
    let (root, content) = (parts.root, parts.content);
    commands.entity(root).insert(ScreenMenu);
    // The title bar a step darker than the overlay's default, so it sits back
    // behind the colourful tiles rather than competing with them.
    commands
        .entity(parts.titlebar)
        .insert(BackgroundColor(renzora_ember::theme::rgb(renzora_ember::theme::faint_bg())));
    commands.entity(content).insert(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(1.0),
        padding: UiRect::all(Val::Px(8.0)),
        ..default()
    });

    let mut kids = Vec::new();
    if file_actions {
        kids.push(import_grid(&mut commands, &fonts));
        kids.push(menu_sep(&mut commands));
    }
    kids.extend(new_asset_menu_items(&mut commands, &fonts, registry.as_deref()));
    commands.entity(content).add_children(&kids);
}

/// The ways to bring files in, as a row of equal tiles along the top of the
/// overlay: Files, Folder and the Marketplace, each a coloured icon tile over
/// its name and a line saying what it does.
///
/// A grid rather than more rows: these are three answers to one question
/// ("where is it coming from?"), so side by side they read as a choice, and the
/// list of asset types to create underneath stays visibly a separate thing.
/// New Folder is not here: it makes something rather than bringing it in, and
/// it has its own key on the toolbar and in the right-click menu.
fn import_grid(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let grid = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                padding: UiRect::bottom(Val::Px(4.0)),
                ..default()
            },
            Name::new("import-grid"),
        ))
        .id();
    let tiles = [
        import_tile(
            commands,
            fonts,
            "file",
            &renzora::lang::t_or("assets.import_files_tile", "Files"),
            &renzora::lang::t_or("assets.import_files_sub", "From disk"),
            (96, 165, 250),
            |w| request_import(w, renzora::core::ImportPick::Files),
        ),
        import_tile(
            commands,
            fonts,
            "folder-open",
            &renzora::lang::t_or("assets.import_folder_tile", "Folder"),
            &renzora::lang::t_or("assets.import_folder_sub", "A whole folder"),
            (240, 180, 80),
            |w| request_import(w, renzora::core::ImportPick::Folder),
        ),
        import_tile(
            commands,
            fonts,
            "storefront",
            &renzora::lang::t_or("assets.search_marketplace_tile", "Marketplace"),
            &renzora::lang::t_or("assets.search_marketplace_sub", "Browse assets"),
            // The Marketplace's own top-bar hue, so it reads as the same place.
            (167, 130, 245),
            |w| renzora::ShellActionInvoked::invoke(w, renzora::ACTION_MARKETPLACE),
        ),
    ];
    commands.entity(grid).add_children(&tiles);
    grid
}

/// One tile of [`import_grid`]. Carries a [`MenuAction`], so a press runs it and
/// closes the overlay exactly as a menu row would (see [`add_menu_open`]).
fn import_tile(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    title: &str,
    subtitle: &str,
    hue: (u8, u8, u8),
    on_click: impl Fn(&mut World) + Send + Sync + 'static,
) -> Entity {
    use renzora_ember::font::{icon_text, ui_font};
    use renzora_ember::theme::{rgb, text_muted, text_primary};
    let tile = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            // Tinted with the tile's own hue rather than a neutral grey: grey
            // tiles read as disabled buttons, and the hue already says which
            // source each one is.
            BackgroundColor(rgb(hue).with_alpha(0.07)),
            BorderColor::all(rgb(hue).with_alpha(0.18)),
            UiInteraction::default(),
            MenuAction(Box::new(on_click)),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("import-tile"),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_bg(commands, tile, move |w| match w.get::<UiInteraction>(tile) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(hue).with_alpha(0.16),
        _ => rgb(hue).with_alpha(0.07),
    });
    let glyph = icon_text(commands, &fonts.phosphor, icon, hue, 20.0);
    commands.entity(glyph).insert(renzora_ember::interaction::PASS_THROUGH);
    let badge = commands
        .spawn((
            Node {
                width: Val::Px(38.0),
                height: Val::Px(38.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(9.0)),
                ..default()
            },
            BackgroundColor(rgb(hue).with_alpha(0.14)),
            renzora_ember::interaction::PASS_THROUGH,
        ))
        .id();
    commands.entity(badge).add_child(glyph);
    let label = |commands: &mut Commands, text: &str, size: f32, color: (u8, u8, u8)| {
        commands
            .spawn((
                Text::new(text),
                ui_font(&fonts.ui, size),
                TextColor(rgb(color)),
                bevy::text::TextLayout::no_wrap(),
                renzora_ember::interaction::PASS_THROUGH,
            ))
            .id()
    };
    // The tiles take one-word names of their own ("Files", not the menu row's
    // "Import Files..."): the overlay's title already says Import, and the
    // longer names overflowed a third of a 440px card.
    let name = label(commands, title, 14.0, text_primary());
    let sub = label(commands, subtitle, 11.0, text_muted());
    commands.entity(tile).add_children(&[badge, name, sub]);
    tile
}

/// The color-coded "create new X" rows shared by the Add button and the
/// right-click menu, led by an Unreal-style "Create Asset" section header. Each
/// row carries the type's accent color (icon + label) so the menu reads as the
/// same color language as the asset tiles.
fn new_asset_menu_items(
    commands: &mut Commands,
    fonts: &EmberFonts,
    registry: Option<&renzora::CreateMenuRegistry>,
) -> Vec<Entity> {
    let mut kids = vec![menu_header(commands, fonts, &renzora::lang::t("assets.new.header"))];
    kids.extend(new_asset_rows(commands, fonts, registry));
    kids
}

/// The create rows alone, without the header: the asset menu's New submenu is
/// already labelled by the row that opens it.
fn new_asset_rows(
    commands: &mut Commands,
    fonts: &EmberFonts,
    registry: Option<&renzora::CreateMenuRegistry>,
) -> Vec<Entity> {
    let mut kids: Vec<Entity> = NewAsset::MENU.iter().map(|&kind| {
        menu_card(
            commands,
            fonts,
            kind.icon(),
            &kind.label(),
            &kind.subtitle(),
            kind.color(),
            move |w| create_asset(w, kind),
        )
    }).collect();
    kids.extend(registered_menu_items(registry, commands, fonts));
    kids
}

/// The same rows, for file types a PLUGIN contributed.
///
/// Appended rather than merged into `NewAsset::MENU`, which stays the engine's
/// own list. The split is the point: an entry is here exactly when something
/// registered it, so the menu stops offering a Lua script to an editor with no
/// Lua backend installed.
///
/// Takes the world because the registry is a resource and the caller is
/// building UI; `None` when nothing has registered, which is a normal state and
/// draws no extra rows.
fn registered_menu_items(
    registry: Option<&renzora::CreateMenuRegistry>,
    commands: &mut Commands,
    fonts: &EmberFonts,
) -> Vec<Entity> {
    let Some(registry) = registry else {
        return Vec::new();
    };
    registry
        .items()
        .iter()
        .map(|item| {
            let id = item.id.clone();
            menu_card(
                commands,
                fonts,
                &item.icon,
                &item.label_text(),
                &item.subtitle_text(),
                // No per-type accent: the engine's palette is keyed to types it
                // knows, and inventing one per plugin would read as meaning
                // something it does not.
                renzora_ember::theme::text_primary(),
                move |w| crate::ops::create_registered_asset(w, &id),
            )
        })
        .collect()
}

/// Open the sort menu (modes + ascending/descending) anchored under the button.
pub(crate) fn sort_menu_open(
    q: Query<
        (
            &UiInteraction,
            &bevy::ui::RelativeCursorPosition,
            &bevy::ui::ComputedNode,
        ),
        (With<SortMenuBtn>, Changed<UiInteraction>),
    >,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    state: Option<Res<NativeAssets>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else {
        return;
    };
    let Some((_, rcp, cn)) = q.iter().find(|(i, _, _)| **i == UiInteraction::Pressed) else {
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
