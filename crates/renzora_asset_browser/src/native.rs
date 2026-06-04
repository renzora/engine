//! Bevy-native (ember) Asset Browser — Increment 1: a navigable tile grid.
//!
//! Toolbar (back + breadcrumb + search) over a wrapping grid of folder/file
//! tiles read from the current folder. Click a folder to navigate in, a file to
//! select. Thumbnails, the folder tree, drag-drop, rename/delete and the context
//! menu are later increments; the egui `AssetBrowserPanel` stays the source for
//! those until then.

use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use bevy::picking::Pickable;
use bevy::prelude::*;

use renzora_editor::{MaterialThumbnailRegistry, ModelThumbnailRegistry, SplashState};
use renzora_ember::font::{icon_glyph, icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::inspector_stripe;
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, bind_bg, bind_display, bind_with, keyed_list, KeyedSnapshot};
use renzora_ember::theme::{accent, panel_bg, popup_bg, rgb, text_muted, text_primary};
use renzora_ember::widgets::{
    icon_label_button, menu_item, menu_item_styled, menu_sep, screen_menu, scroll_view, slider,
    text_input, EmberTextInput,
};

use crate::thumbnails::{
    supports_material_thumbnail, supports_model_thumbnail, supports_thumbnail, ThumbnailCache,
};

/// Which thumbnail source a file uses.
#[derive(Clone, Copy, PartialEq)]
enum ThumbKind {
    Image,
    Model,
    Material,
}

fn thumb_kind(name: &str) -> Option<ThumbKind> {
    if supports_thumbnail(name) {
        Some(ThumbKind::Image)
    } else if supports_model_thumbnail(name) {
        Some(ThumbKind::Model)
    } else if supports_material_thumbnail(name) {
        Some(ThumbKind::Material)
    } else {
        None
    }
}

/// The ready thumbnail handle for `path`, from whichever registry owns `kind`.
fn handle_for(w: &World, kind: ThumbKind, path: &PathBuf) -> Option<Handle<Image>> {
    match kind {
        ThumbKind::Image => w.get_resource::<ThumbnailCache>().and_then(|c| c.handle(path)),
        ThumbKind::Model => w.get_resource::<ModelThumbnailRegistry>().and_then(|r| r.handle(path)),
        ThumbKind::Material => w.get_resource::<MaterialThumbnailRegistry>().and_then(|r| r.handle(path)),
    }
}

const TILE_W: f32 = 96.0;

// Content-area surfaces (Unreal-style: flat, dark).

/// Lean native state for the browser (independent of the egui panel's state).
#[derive(Resource)]
pub(crate) struct NativeAssets {
    current: Option<PathBuf>,
    selected: Option<PathBuf>,
    search: String,
    /// Expanded folders in the left tree.
    expanded: HashSet<PathBuf>,
    /// The grid tile the cursor is currently over (for right-click targeting).
    hovered: Option<PathBuf>,
    /// Last tile click (path, time) for double-click detection.
    last_click: Option<(PathBuf, f64)>,
    /// Grid tile zoom (0.5–1.5).
    zoom: f32,
    /// Folder-tree pane width (px).
    tree_width: f32,
    /// Active divider drag: `(start cursor x, start tree width)`. Persists the
    /// drag even when the cursor leaves the thin splitter (bevy_ui drops
    /// `Pressed` off-element).
    divider_drag: Option<(f32, f32)>,
    /// Favorited folders (persisted to `<root>/.editor/favorites`).
    favorites: Vec<PathBuf>,
    /// Recently opened files (persisted to `<root>/.editor/recent`).
    recent: Vec<PathBuf>,
    /// Whether favorites/recent have been loaded from disk this session.
    loaded: bool,
    /// A pending tile press `(path, is_dir, origin)` — promoted to a drag once
    /// the cursor moves >5px (for drag-to-viewport).
    drag_press: Option<(PathBuf, bool, Vec2)>,
    /// Whether the collapsible FAVORITES / RECENT tree sections are expanded
    /// (both collapsed by default).
    fav_open: bool,
    recent_open: bool,
    /// True while a tile is being dragged out (drives the cursor ghost).
    dragging: bool,
}

impl Default for NativeAssets {
    fn default() -> Self {
        Self {
            current: None,
            selected: None,
            search: String::new(),
            expanded: HashSet::new(),
            hovered: None,
            last_click: None,
            zoom: 1.0,
            tree_width: 180.0,
            divider_drag: None,
            favorites: Vec::new(),
            recent: Vec::new(),
            loaded: false,
            drag_press: None,
            fav_open: false,
            recent_open: false,
            dragging: false,
        }
    }
}

// ── Favorites / recent persistence (<root>/.editor/{favorites,recent}) ─────────

fn load_list(root: &Path, file: &str) -> Vec<PathBuf> {
    let path = root.join(".editor").join(file);
    std::fs::read_to_string(&path)
        .ok()
        .map(|c| {
            c.lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| root.join(l.trim()))
                .filter(|p| p.exists())
                .collect()
        })
        .unwrap_or_default()
}

fn save_list(root: &Path, file: &str, list: &[PathBuf]) {
    let dir = root.join(".editor");
    let _ = std::fs::create_dir_all(&dir);
    let content: String = list
        .iter()
        .filter_map(|p| p.strip_prefix(root).ok().map(|r| r.to_string_lossy().replace('\\', "/")))
        .collect::<Vec<_>>()
        .join("\n");
    let _ = std::fs::write(dir.join(file), content);
}

#[derive(Component)]
struct AssetRoot;
#[derive(Component)]
struct DragGhost;
/// The toolbar "Add" button — clicking it opens the new-asset menu.
#[derive(Component)]
struct AddMenuBtn;
#[derive(Component)]
struct Splitter;

#[derive(Component)]
struct AssetTile {
    path: PathBuf,
    is_dir: bool,
}
#[derive(Component)]
struct AssetBack;
#[derive(Component)]
struct AssetSearch;
/// A collapsible tree section header (FAVORITES / RECENT).
#[derive(Clone, Copy, PartialEq)]
enum Section {
    Favorites,
    Recent,
}

#[derive(Component)]
struct SectionToggle(Section);

#[derive(Component)]
struct TreeToggle(PathBuf);
#[derive(Component)]
struct TreeNav(PathBuf);
#[derive(Component)]
struct NewAssetBtn(NewAsset);
#[derive(Component)]
struct ImportBtn;
#[derive(Component)]
struct CrumbNav(PathBuf);
#[derive(Component)]
struct ShortcutClick {
    path: PathBuf,
    is_dir: bool,
}

/// What the New-Folder / Add menu creates.
#[derive(Clone, Copy)]
enum NewAsset {
    Folder,
    Material,
    Blueprint,
    Lua,
    Rhai,
    Particle,
}

impl NewAsset {
    fn filename(self) -> &'static str {
        match self {
            NewAsset::Folder => "New Folder",
            NewAsset::Material => "NewMaterial.material",
            NewAsset::Blueprint => "NewBlueprint.blueprint",
            NewAsset::Lua => "new_script.lua",
            NewAsset::Rhai => "new_script.rhai",
            NewAsset::Particle => "NewParticle.particle",
        }
    }
    fn content(self) -> &'static str {
        match self {
            NewAsset::Folder => "",
            NewAsset::Material | NewAsset::Blueprint => "{}",
            NewAsset::Lua => "-- New Lua script\n",
            NewAsset::Rhai => "// New Rhai script\n",
            NewAsset::Particle => "(name: \"New Particle\")",
        }
    }
    fn is_folder(self) -> bool {
        matches!(self, NewAsset::Folder)
    }
}

/// A free path in `folder` for `filename`, suffixing " 2", " 3"… on collision.
fn unique_path(folder: &Path, filename: &str, is_folder: bool) -> PathBuf {
    let candidate = folder.join(filename);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = if is_folder {
        (filename.to_string(), String::new())
    } else {
        let p = Path::new(filename);
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or(filename).to_string();
        let ext = p.extension().and_then(|e| e.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
        (stem, ext)
    };
    for n in 2..1000 {
        let cand = folder.join(format!("{stem} {n}{ext}"));
        if !cand.exists() {
            return cand;
        }
    }
    candidate
}

pub fn register_native_asset_browser(app: &mut App) {
    app.init_resource::<NativeAssets>();
    app.register_panel_content("assets", false, build);
    app.add_systems(
        Update,
        (
            tile_click,
            back_click,
            search_sync,
            request_thumbnails,
            tree_toggle_click,
            tree_nav_click,
            section_toggle_click,
            create_asset_click,
            import_click,
            crumb_click,
            load_persisted,
            shortcut_click,
            asset_drag,
            drag_ghost,
            add_menu_open,
        )
            .run_if(in_state(SplashState::Editor)),
    );
    app.add_systems(
        Update,
        (track_hover, asset_context_menu)
            .chain()
            .run_if(in_state(SplashState::Editor)),
    );
    app.add_systems(Update, splitter_drag.run_if(in_state(SplashState::Editor)));
    // Force the resize cursor while hovering/dragging the divider. In PostUpdate
    // so it wins over renzora_hui's Update cursor system (which would otherwise
    // reset to Default once the cursor leaves the thin splitter mid-drag).
    app.add_systems(PostUpdate, divider_cursor.run_if(in_state(SplashState::Editor)));
}

/// Drag the tree/content divider to resize the tree pane. The drag persists via
/// `divider_drag` (captured on press) so it keeps tracking even when the cursor
/// moves off the thin splitter — mirrors the dock's divider.
fn splitter_drag(
    splitter: Query<&Interaction, With<Splitter>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut state: ResMut<NativeAssets>,
) {
    if mouse.just_released(MouseButton::Left) {
        state.divider_drag = None;
    }
    let Some(cx) = windows.iter().next().and_then(|w| w.cursor_position()).map(|p| p.x) else {
        return;
    };
    if state.divider_drag.is_none()
        && mouse.just_pressed(MouseButton::Left)
        && splitter.iter().any(|i| *i == Interaction::Pressed)
    {
        state.divider_drag = Some((cx, state.tree_width));
    }
    if let Some((start_x, start_w)) = state.divider_drag {
        state.tree_width = (start_w + (cx - start_x)).clamp(120.0, 420.0);
    }
}

/// Show the ew-resize cursor whenever the divider is hovered or being dragged.
fn divider_cursor(
    splitter: Query<&Interaction, With<Splitter>>,
    state: Res<NativeAssets>,
    windows: Query<Entity, With<bevy::window::PrimaryWindow>>,
    mut commands: Commands,
    mut forcing: Local<bool>,
) {
    let want = state.divider_drag.is_some()
        || splitter.iter().any(|i| matches!(i, Interaction::Hovered | Interaction::Pressed));
    let Ok(win) = windows.single() else {
        return;
    };
    if want {
        commands
            .entity(win)
            .insert(bevy::window::CursorIcon::System(bevy::window::SystemCursorIcon::EwResize));
        *forcing = true;
    } else if *forcing {
        *forcing = false;
        commands
            .entity(win)
            .insert(bevy::window::CursorIcon::System(bevy::window::SystemCursorIcon::Default));
    }
}

fn create_asset_click(
    q: Query<(&Interaction, &NewAssetBtn), Changed<Interaction>>,
    mut state: ResMut<NativeAssets>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
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
            std::fs::write(&path, kind.content()).is_ok()
        };
        if ok {
            state.selected = Some(path);
        }
    }
}

fn import_click(
    q: Query<&Interaction, (With<ImportBtn>, Changed<Interaction>)>,
    mut commands: Commands,
    state: Res<NativeAssets>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    commands.insert_resource(renzora::core::ImportRequested);
    if let Some(folder) = state.current.clone().or_else(|| project.map(|p| p.path.clone())) {
        commands.insert_resource(renzora::core::ImportTargetDir(folder.to_string_lossy().to_string()));
    }
}

/// Load favorites + recent from disk once the project is known.
fn load_persisted(mut state: ResMut<NativeAssets>, project: Option<Res<renzora::core::CurrentProject>>) {
    if state.loaded {
        return;
    }
    let Some(root) = project.map(|p| p.path.clone()) else {
        return;
    };
    state.favorites = load_list(&root, "favorites");
    state.recent = load_list(&root, "recent");
    state.loaded = true;
}

/// Drag a tile out toward the viewport: records the press, and once the cursor
/// moves >5px inserts an `AssetDragPayload` (the viewport shows a live preview
/// while it exists and commits the spawn when it's removed on release). Mirrors
/// the egui drag lifecycle, which only runs in the egui pass.
fn asset_drag(
    tiles: Query<(&Interaction, &AssetTile)>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut state: ResMut<NativeAssets>,
    payload: Option<Res<renzora_editor::AssetDragPayload>>,
    mut commands: Commands,
) {
    if mouse.just_released(MouseButton::Left) {
        state.drag_press = None;
        state.dragging = false;
        if payload.is_some() {
            commands.remove_resource::<renzora_editor::AssetDragPayload>();
        }
        return;
    }
    let cursor = windows.iter().next().and_then(|w| w.cursor_position());
    if mouse.just_pressed(MouseButton::Left) {
        if let Some(c) = cursor {
            if let Some((_, tile)) = tiles.iter().find(|(i, _)| matches!(i, Interaction::Pressed)) {
                state.drag_press = Some((tile.path.clone(), tile.is_dir, c));
            }
        }
    }
    if payload.is_none() {
        if let (Some((path, is_dir, origin)), Some(c)) = (state.drag_press.clone(), cursor) {
            if !is_dir && c.distance(origin) > 5.0 {
                commands.insert_resource(renzora_editor::AssetDragPayload {
                    name: file_name_of(&path),
                    paths: vec![path.clone()],
                    icon: String::new(),
                    color: bevy_egui::egui::Color32::from_rgb(170, 175, 190),
                    origin: bevy_egui::egui::Pos2::new(origin.x, origin.y),
                    is_detached: true,
                    drag_count: 1,
                    path,
                });
                state.dragging = true;
            }
        }
    }
}

/// A floating ghost (icon + name) that follows the cursor while a tile is being
/// dragged out of the browser. Spawned as a top-level overlay so it isn't
/// clipped by the panel, and despawned the moment the drag ends.
fn drag_ghost(
    mut commands: Commands,
    state: Res<NativeAssets>,
    payload: Option<Res<renzora_editor::AssetDragPayload>>,
    fonts: Option<Res<EmberFonts>>,
    windows: Query<&Window>,
    mut ghosts: Query<(Entity, &mut Node), With<DragGhost>>,
) {
    let Some(payload) = payload.filter(|_| state.dragging) else {
        for (e, _) in &ghosts {
            commands.entity(e).despawn();
        }
        return;
    };
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else {
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
    let ic = icon_text(&mut commands, &fonts.phosphor, icon_for(&payload.path, false), color, 14.0);
    let lbl = commands
        .spawn((
            Text::new(payload.name.clone()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::new_with_no_wrap(),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(ghost).add_children(&[ic, lbl]);
}

/// A favorites/recent shortcut row: navigate (folder) or open (file).
fn shortcut_click(
    q: Query<(&Interaction, &ShortcutClick), Changed<Interaction>>,
    mut state: ResMut<NativeAssets>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
    for (interaction, shortcut) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if shortcut.is_dir {
            state.current = Some(shortcut.path.clone());
            state.selected = None;
        } else {
            os_open(&shortcut.path);
            let root = project.as_ref().map(|p| p.path.clone());
            track_recent(&mut state, &shortcut.path, root.as_deref());
        }
    }
}

/// Move `path` to the front of the recent list (max 20) + persist.
fn track_recent(state: &mut NativeAssets, path: &Path, root: Option<&Path>) {
    state.recent.retain(|p| p != path);
    state.recent.insert(0, path.to_path_buf());
    state.recent.truncate(20);
    if let Some(root) = root {
        save_list(root, "recent", &state.recent);
    }
}

fn crumb_click(
    q: Query<(&Interaction, &CrumbNav), Changed<Interaction>>,
    mut state: ResMut<NativeAssets>,
) {
    for (interaction, nav) in &q {
        if *interaction == Interaction::Pressed {
            state.current = Some(nav.0.clone());
            state.selected = None;
        }
    }
}

// ── Context menu ─────────────────────────────────────────────────────────────

fn track_hover(tiles: Query<(&Interaction, &AssetTile)>, mut state: ResMut<NativeAssets>) {
    for (interaction, tile) in &tiles {
        if matches!(interaction, Interaction::Hovered | Interaction::Pressed)
            && state.hovered.as_deref() != Some(tile.path.as_path())
        {
            state.hovered = Some(tile.path.clone());
        }
    }
}

/// Right-click a tile → open the shared ember menu (Favorite / Duplicate /
/// Reveal / Delete) for the hovered asset.
fn asset_context_menu(
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
    let Some(path) = state.hovered.clone() else {
        return;
    };
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else {
        return;
    };
    let fav_label = if state.favorites.contains(&path) {
        "Unfavorite"
    } else {
        "Favorite"
    };
    let menu = screen_menu(&mut commands, cursor.x, cursor.y);
    let kids = vec![
        menu_item(&mut commands, &fonts, "star", fav_label, {
            let path = path.clone();
            move |w| toggle_favorite(w, &path)
        }),
        menu_item(&mut commands, &fonts, "copy", "Duplicate", {
            let path = path.clone();
            move |_| duplicate_asset(&path)
        }),
        menu_item(&mut commands, &fonts, "folder-open", "Reveal in Explorer", {
            let path = path.clone();
            move |_| reveal_in_explorer(&path)
        }),
        menu_sep(&mut commands),
        menu_item_styled(&mut commands, &fonts, "trash", "Delete", (224, 96, 88), (224, 96, 88), {
            let path = path.clone();
            move |w| delete_asset(w, &path)
        }),
    ];
    commands.entity(menu).add_children(&kids);
}

/// Click the toolbar "Add" button → open the shared ember menu of new-asset
/// types at the cursor.
fn add_menu_open(
    q: Query<
        (
            &Interaction,
            &bevy::ui::RelativeCursorPosition,
            &bevy::ui::ComputedNode,
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
    let Some((_, rcp, cn)) = q.iter().find(|(i, _, _)| **i == Interaction::Pressed) else {
        return;
    };
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else {
        return;
    };
    // Anchor the menu to the button's bottom-left (stable, cursor-independent).
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    let menu = screen_menu(&mut commands, top_left.x, top_left.y + size.y + 2.0);
    let items = [
        ("palette", "Material", NewAsset::Material),
        ("blueprint", "Blueprint", NewAsset::Blueprint),
        ("code", "Lua Script", NewAsset::Lua),
        ("code", "Rhai Script", NewAsset::Rhai),
        ("sparkle", "Particle", NewAsset::Particle),
    ];
    let kids: Vec<Entity> = items
        .iter()
        .map(|&(icon, label, kind)| {
            menu_item(&mut commands, &fonts, icon, label, move |w| create_asset(w, kind))
        })
        .collect();
    commands.entity(menu).add_children(&kids);
}

/// Create a new asset (folder or file) in the current folder + select it.
fn create_asset(world: &mut World, kind: NewAsset) {
    let folder = world
        .get_resource::<NativeAssets>()
        .and_then(|s| s.current.clone())
        .or_else(|| {
            world
                .get_resource::<renzora::core::CurrentProject>()
                .map(|p| p.path.clone())
        });
    let Some(folder) = folder else {
        return;
    };
    let path = unique_path(&folder, kind.filename(), kind.is_folder());
    let ok = if kind.is_folder() {
        std::fs::create_dir_all(&path).is_ok()
    } else {
        std::fs::write(&path, kind.content()).is_ok()
    };
    if ok {
        if let Some(mut s) = world.get_resource_mut::<NativeAssets>() {
            s.selected = Some(path);
        }
    }
}

fn toggle_favorite(world: &mut World, path: &Path) {
    let root = world
        .get_resource::<renzora::core::CurrentProject>()
        .map(|p| p.path.clone());
    if let Some(mut s) = world.get_resource_mut::<NativeAssets>() {
        if let Some(i) = s.favorites.iter().position(|f| f == path) {
            s.favorites.remove(i);
        } else {
            s.favorites.push(path.to_path_buf());
        }
        if let Some(root) = root {
            save_list(&root, "favorites", &s.favorites);
        }
    }
}

fn delete_asset(world: &mut World, path: &Path) {
    let _ = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    if let Some(mut s) = world.get_resource_mut::<NativeAssets>() {
        if s.selected.as_deref() == Some(path) {
            s.selected = None;
        }
    }
}

/// Copy a file (or directory tree) next to itself with a " copy" suffix.
fn duplicate_asset(path: &Path) {
    let Some(parent) = path.parent() else {
        return;
    };
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("copy");
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    let is_dir = path.is_dir();
    let dest = unique_path(parent, &format!("{stem} copy{ext}"), is_dir);
    if is_dir {
        let _ = copy_dir_recursive(path, &dest);
    } else {
        let _ = std::fs::copy(path, &dest);
    }
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)?.flatten() {
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Open the OS file manager at `path` (selecting it where supported).
fn reveal_in_explorer(path: &Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg("/select,").arg(path).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let dir = path.parent().unwrap_or(path);
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    }
}

fn project_root(w: &World) -> Option<PathBuf> {
    w.get_resource::<renzora::core::CurrentProject>().map(|p| p.path.clone())
}

/// The folder being shown (the explicit nav target, else the project root).
fn current_folder(w: &World) -> Option<PathBuf> {
    w.get_resource::<NativeAssets>()
        .and_then(|s| s.current.clone())
        .or_else(|| project_root(w))
}

/// Accent color for a folder's icon, by well-known name (ported from the egui
/// browser's `folder_icon_color`).
fn folder_color(name: &str) -> (u8, u8, u8) {
    match name.to_lowercase().as_str() {
        "assets" => (255, 210, 100),
        "scenes" | "blueprints" => (100, 180, 255),
        "scripts" => (130, 230, 180),
        "materials" => (255, 130, 200),
        "textures" | "images" => (150, 230, 130),
        "models" | "meshes" => (255, 170, 100),
        "audio" | "sounds" | "music" => (200, 130, 230),
        "prefabs" => (130, 180, 255),
        "src" => (255, 130, 80),
        "shaders" => (180, 130, 255),
        _ => (170, 175, 190),
    }
}

/// Accent color + human-readable type label for a file, by extension. Drives the
/// tile's type subtitle and bottom accent strip (mirrors Unreal's "Blueprint
/// Class" subtitle + colored underline). Folders are handled separately.
fn asset_type_info(path: &Path) -> ((u8, u8, u8), &'static str) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "material" => ((0, 200, 130), "Material"),
        "blueprint" | "bp" => ((100, 180, 255), "Blueprint"),
        "lua" => ((120, 170, 255), "Lua Script"),
        "rhai" => ((130, 230, 180), "Rhai Script"),
        "wgsl" | "glsl" | "vert" | "frag" => ((220, 120, 255), "Shader"),
        "rs" => ((230, 140, 90), "Rust Source"),
        "png" | "jpg" | "jpeg" | "webp" | "ktx2" | "dds" | "bmp" | "tga" => {
            ((150, 210, 120), "Texture")
        }
        "glb" | "gltf" | "obj" | "fbx" => ((255, 170, 100), "Model"),
        "ron" | "scn" | "scene" => ((115, 191, 242), "Scene"),
        "particle" => ((230, 160, 90), "Particle"),
        "wav" | "ogg" | "mp3" | "flac" => ((200, 130, 230), "Audio"),
        "html" => ((230, 120, 90), "Template"),
        "" => ((150, 155, 170), "File"),
        other => ((150, 155, 170), uppercase_ext(other)),
    }
}

/// Leak a small set of uppercased extension labels for unknown types so the
/// subtitle can read e.g. "TXT" / "JSON". Bounded: only the handful of distinct
/// unknown extensions actually present in a project are ever leaked.
fn uppercase_ext(ext: &str) -> &'static str {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    let mut map = CACHE.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap();
    if let Some(s) = map.get(ext) {
        return s;
    }
    let leaked: &'static str = Box::leak(ext.to_uppercase().into_boxed_str());
    map.insert(ext.to_string(), leaked);
    leaked
}

fn icon_for(path: &Path, is_dir: bool) -> &'static str {
    if is_dir {
        return "folder";
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "ktx2" | "dds" | "bmp" | "tga" => "image",
        "glb" | "gltf" | "obj" | "fbx" => "cube",
        "material" => "palette",
        "wgsl" | "glsl" | "vert" | "frag" => "graphics-card",
        "lua" | "rhai" | "rs" | "py" | "js" | "ts" => "code",
        "scene" | "ron" | "scn" => "stack",
        "wav" | "ogg" | "mp3" | "flac" => "speaker-high",
        "particle" => "sparkle",
        "blueprint" | "bp" => "blueprint",
        "html" => "browser",
        "toml" => "brackets-curly",
        _ => "file",
    }
}

// ── Panel ────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                ..default()
            },
            bevy::ui::RelativeCursorPosition::default(),
            AssetRoot,
        ))
        .id();

    // ── Folder tree (left pane, own scroll) ──
    let tree_list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            padding: UiRect::vertical(Val::Px(4.0)),
            ..default()
        })
        .id();
    keyed_list(commands, tree_list, tree_snapshot);
    let tree_scroll = scroll_view(commands, tree_list);
    let tree_pane = commands
        .spawn((
            Node {
                width: Val::Px(180.0),
                height: Val::Percent(100.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                min_height: Val::Px(0.0),
                ..default()
            },
            // Match the hierarchy panel base so the shared odd/even stripe
            // (inspector_stripe) renders the exact same colors.
            BackgroundColor(rgb(panel_bg())),
        ))
        .id();
    commands.entity(tree_pane).add_child(tree_scroll);
    bind_with(
        commands,
        tree_pane,
        |w| w.get_resource::<NativeAssets>().map(|s| s.tree_width).unwrap_or(180.0),
        |w, e, width: &f32| {
            if let Some(mut n) = w.get_mut::<Node>(e) {
                n.width = Val::Px(*width);
            }
        },
    );

    // Draggable divider (highlights on hover/drag).
    let splitter = commands
        .spawn((
            Node {
                width: Val::Px(4.0),
                height: Val::Percent(100.0),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::border())),
            Interaction::default(),
            Splitter,
            Name::new("assets-splitter"),
        ))
        .id();
    bind_bg(commands, splitter, move |w| {
        let dragging = w.get_resource::<NativeAssets>().is_some_and(|s| s.divider_drag.is_some());
        let hovered = matches!(
            w.get::<Interaction>(splitter),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        );
        if dragging || hovered {
            rgb(accent())
        } else {
            rgb(renzora_ember::theme::border())
        }
    });

    // ── Content (toolbar + search + grid + footer) ──
    let content = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::panel_bg())),
        ))
        .id();

    // Toolbar.
    let toolbar = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                width: Val::Percent(100.0),
                column_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::header_bg())),
        ))
        .id();
    let back = commands
        .spawn((
            Node {
                width: Val::Px(22.0),
                height: Val::Px(20.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::hover_bg())),
            Interaction::default(),
            AssetBack,
            Name::new("assets-back"),
        ))
        .id();
    let back_icon = icon_text(commands, &fonts.phosphor, "arrow-left", text_primary(), 13.0);
    commands.entity(back).add_child(back_icon);
    // Hidden at the project root (nowhere to go up to).
    bind_display(commands, back, |w| current_folder(w) != project_root(w));

    let new_folder = icon_label_button(commands, fonts, "folder-plus", "New Folder");
    commands.entity(new_folder).insert(NewAssetBtn(NewAsset::Folder));
    let add = icon_label_button(commands, fonts, "plus", "Add");
    commands
        .entity(add)
        .insert((AddMenuBtn, bevy::ui::RelativeCursorPosition::default()));
    let import = icon_label_button(commands, fonts, "download-simple", "Import");
    commands.entity(import).insert(ImportBtn);

    // Breadcrumb in a dark inset box, with the back button to its left.
    let crumbs = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    keyed_list(commands, crumbs, crumb_snapshot);

    let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();

    // Zoom control (maps 0..1 → 0.5..1.5 tile scale) in a small framed box with
    // a magnifier glyph.
    let zoom = slider(commands, 0.5);
    bind_2way(
        commands,
        zoom,
        |w| w.get_resource::<NativeAssets>().map(|s| (s.zoom - 0.5).clamp(0.0, 1.0)).unwrap_or(0.5),
        |w, v| {
            if let Some(mut s) = w.get_resource_mut::<NativeAssets>() {
                s.zoom = 0.5 + *v;
            }
        },
    );
    commands.entity(zoom).insert(Node {
        width: Val::Px(70.0),
        height: Val::Px(14.0),
        position_type: PositionType::Relative,
        align_items: AlignItems::Center,
        ..default()
    });
    let zoom_icon = icon_text(commands, &fonts.phosphor, "magnifying-glass", text_muted(), 11.0);
    let zoom_box = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::row_even())),
            BorderColor::all(rgb(renzora_ember::theme::border())),
            Name::new("zoom-box"),
        ))
        .id();
    commands.entity(zoom_box).add_children(&[zoom_icon, zoom]);

    // Compact search field, placed just left of the zoom control.
    let search = text_input(commands, &fonts.ui, "Search...", "");
    commands.entity(search).insert((
        AssetSearch,
        Node {
            width: Val::Px(160.0),
            min_width: Val::Px(0.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
    ));

    commands
        .entity(toolbar)
        .add_children(&[add, import, new_folder, spacer, search, zoom_box]);

    // Grid.
    let grid = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            width: Val::Percent(100.0),
            align_items: AlignItems::FlexStart,
            align_content: AlignContent::FlexStart,
            column_gap: Val::Px(10.0),
            row_gap: Val::Px(12.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        })
        .id();
    keyed_list(commands, grid, grid_snapshot);
    let grid_scroll = scroll_view(commands, grid);

    // ── Footer: back + breadcrumb on the left, live item count on the right. ──
    let footer = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(26.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::top(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::window_bg())),
            BorderColor::all(rgb(renzora_ember::theme::border())),
        ))
        .id();
    let footer_spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let count = commands
        .spawn((
            Text::new("0 items"),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_with(
        commands,
        count,
        |w| list_entries(w).len(),
        |w, e, n: &usize| {
            if let Some(mut t) = w.get_mut::<Text>(e) {
                let s = if *n == 1 {
                    "1 item".to_string()
                } else {
                    format!("{n} items")
                };
                if t.0 != s {
                    t.0 = s;
                }
            }
        },
    );
    commands
        .entity(footer)
        .add_children(&[back, crumbs, footer_spacer, count]);

    commands
        .entity(content)
        .add_children(&[toolbar, grid_scroll, footer]);

    commands.entity(root).add_children(&[tree_pane, splitter, content]);
    root
}

/// Clickable breadcrumb segments (project root + each path component).
fn crumb_snapshot(world: &World) -> KeyedSnapshot {
    let Some(root) = project_root(world) else {
        return KeyedSnapshot {
            items: Vec::new(),
            build: Box::new(|_, _, _| Entity::PLACEHOLDER),
        };
    };
    let cur = current_folder(world).unwrap_or_else(|| root.clone());
    let root_name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Project")
        .to_string();
    let mut segs: Vec<(String, PathBuf)> = vec![(root_name, root.clone())];
    if let Ok(rel) = cur.strip_prefix(&root) {
        let mut acc = root.clone();
        for comp in rel.components() {
            acc = acc.join(comp);
            segs.push((comp.as_os_str().to_string_lossy().to_string(), acc.clone()));
        }
    }
    let items: Vec<(u64, u64)> = segs
        .iter()
        .enumerate()
        .map(|(i, (name, path))| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            (i, path).hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            name.hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| crumb_seg(c, f, i, &segs[i].0, &segs[i].1)),
    }
}

fn crumb_seg(commands: &mut Commands, fonts: &EmberFonts, idx: usize, name: &str, path: &Path) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    let mut kids = Vec::new();
    if idx > 0 {
        kids.push(icon_text(commands, &fonts.phosphor, "caret-right", text_muted(), 9.0));
    }
    let label = commands
        .spawn((
            Text::new(name.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            Interaction::default(),
            CrumbNav(path.to_path_buf()),
            Name::new("crumb"),
        ))
        .id();
    kids.push(label);
    commands.entity(row).add_children(&kids);
    row
}

// ── Grid ─────────────────────────────────────────────────────────────────────

struct Entry {
    path: PathBuf,
    name: String,
    is_dir: bool,
}

fn list_entries(w: &World) -> Vec<Entry> {
    let Some(folder) = current_folder(w) else {
        return Vec::new();
    };
    let search = w
        .get_resource::<NativeAssets>()
        .map(|s| s.search.to_lowercase())
        .unwrap_or_default();
    let mut entries: Vec<Entry> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&folder) {
        for e in rd.flatten() {
            let path = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if !search.is_empty() && !name.to_lowercase().contains(&search) {
                continue;
            }
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            entries.push(Entry { path, name, is_dir });
        }
    }
    // Folders first, then alphabetical.
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    entries
}

fn grid_snapshot(world: &World) -> KeyedSnapshot {
    let entries = list_entries(world);
    if entries.is_empty() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 0)],
            build: Box::new(|c, f, _| {
                c.spawn((
                    Text::new("Empty folder"),
                    ui_font(&f.ui, 11.0),
                    TextColor(rgb(text_muted())),
                    Node { padding: UiRect::all(Val::Px(8.0)), ..default() },
                ))
                .id()
            }),
        };
    }
    let zoom = world.get_resource::<NativeAssets>().map(|s| s.zoom).unwrap_or(1.0);
    let zoom_q = (zoom * 20.0).round() as u64;
    let favs: HashSet<PathBuf> = world
        .get_resource::<NativeAssets>()
        .map(|s| s.favorites.iter().cloned().collect())
        .unwrap_or_default();
    let items: Vec<(u64, u64)> = entries
        .iter()
        .map(|e| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (&e.name, e.is_dir, zoom_q, favs.contains(&e.path)).hash(&mut h);
            let mut k = std::collections::hash_map::DefaultHasher::new();
            e.path.hash(&mut k);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| tile(c, f, &entries[i], zoom, favs.contains(&entries[i].path))),
    }
}

fn tile(commands: &mut Commands, fonts: &EmberFonts, entry: &Entry, zoom: f32, fav: bool) -> Entity {
    let card_w = (TILE_W * zoom).round();
    let thumb_h = card_w; // square preview, Unreal-style
    let icon_sz = (card_w * 0.42).round();
    let is_dir = entry.is_dir;
    let (type_color, type_label) = if is_dir {
        (folder_color(&entry.name), "Folder")
    } else {
        asset_type_info(&entry.path)
    };

    // ── Card shell (folders and files share the same card) ──
    let col = commands
        .spawn((
            Node {
                width: Val::Px(card_w),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(rgb(renzora_ember::theme::border())),
            Interaction::default(),
            AssetTile {
                path: entry.path.clone(),
                is_dir,
            },
            Name::new("asset-tile"),
        ))
        .id();
    let path_bg = entry.path.clone();
    bind_bg(commands, col, move |w| {
        let selected = w
            .get_resource::<NativeAssets>()
            .map(|s| s.selected.as_deref() == Some(path_bg.as_path()))
            .unwrap_or(false);
        let hovered = matches!(
            w.get::<Interaction>(col),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        );
        let st = &w.resource::<renzora_ember::style::Theme>().asset_tile;
        if selected {
            st.card_selected.color()
        } else if hovered {
            st.card_hover.color()
        } else {
            st.card_bg.color()
        }
    });
    // Themed border — accent (border_selected) on select, else border.
    let path_bd = entry.path.clone();
    bind_with(
        commands,
        col,
        move |w| {
            w.get_resource::<NativeAssets>()
                .map(|s| s.selected.as_deref() == Some(path_bd.as_path()))
                .unwrap_or(false)
        },
        move |w, e, selected: &bool| {
            let (sel, norm) = {
                let st = &w.resource::<renzora_ember::style::Theme>().asset_tile;
                (st.border_selected.color(), st.border.color())
            };
            if let Some(mut bc) = w.get_mut::<BorderColor>(e) {
                *bc = BorderColor::all(if *selected { sel } else { norm });
            }
        },
    );

    // ── Thumbnail area (icon + optional rendered thumbnail overlay) ──
    let thumb = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(thumb_h),
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::popup_bg())),
        ))
        .id();
    bind_bg(commands, thumb, |w| {
        w.resource::<renzora_ember::style::Theme>().asset_tile.thumb_bg.color()
    });
    let icon = icon_text(commands, &fonts.phosphor, icon_for(&entry.path, is_dir), type_color, icon_sz);
    commands.entity(thumb).add_child(icon);

    // Star badge on favorited assets.
    if fav {
        let star = icon_text(commands, &fonts.phosphor, "star", (255, 200, 70), 12.0);
        renzora_ember::reactive::bind_text_color(commands, star, |w| {
            w.resource::<renzora_ember::style::Theme>().asset_tile.star.color()
        });
        commands.entity(star).insert(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(3.0),
            right: Val::Px(3.0),
            ..default()
        });
        commands.entity(thumb).add_child(star);
    }

    if let Some(kind) = (!is_dir).then(|| thumb_kind(&entry.name)).flatten() {
        let img = commands
            .spawn((
                ImageNode::default(),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    display: Display::None,
                    ..default()
                },
                Name::new("asset-thumb"),
            ))
            .id();
        commands.entity(thumb).add_child(img);
        let p = entry.path.clone();
        bind_with(
            commands,
            img,
            move |w| handle_for(w, kind, &p),
            move |w, e, h: &Option<Handle<Image>>| {
                let Some(handle) = h else { return };
                if let Some(mut n) = w.get_mut::<ImageNode>(e) {
                    n.image = handle.clone();
                }
                if let Some(mut node) = w.get_mut::<Node>(e) {
                    node.display = Display::Flex;
                }
                if let Some(mut node) = w.get_mut::<Node>(icon) {
                    node.display = Display::None;
                }
            },
        );
    }

    // ── Label area: name + type subtitle ──
    let info = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            row_gap: Val::Px(1.0),
            padding: UiRect::axes(Val::Px(5.0), Val::Px(4.0)),
            ..default()
        })
        .id();
    let name = commands
        .spawn((
            Text::new(entry.name.clone()),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_primary())),
            Node {
                width: Val::Percent(100.0),
                max_height: Val::Px(26.0),
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    let ty = commands
        .spawn((
            Text::new(type_label),
            ui_font(&fonts.ui, 9.0),
            TextColor(rgb(renzora_ember::theme::text_muted())),
            bevy::text::TextLayout::new_with_no_wrap(),
            Node {
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    commands.entity(info).add_children(&[name, ty]);

    // Colored bottom accent strip (the Unreal "type underline").
    let accent = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(rgb(type_color)),
        ))
        .id();
    commands.entity(col).add_children(&[thumb, info, accent]);
    col
}

// ── Folder tree ──────────────────────────────────────────────────────────────
//
// Tree indent guides match the hierarchy panel: 1.5px absolute line nodes, the
// same INDENT / LINE_OFFSET geometry, and `inspector_stripe` odd/even row bands.

const TREE_INDENT: f32 = 12.0;
const TREE_ROW_H: f32 = 20.0;
const TREE_BASE_X: f32 = 4.0;
const TREE_LINE_OFFSET: f32 = TREE_INDENT / 2.0 - 1.0; // 5.0
const TREE_CENTER_Y: f32 = TREE_ROW_H / 2.0;

struct TreeRow {
    path: PathBuf,
    name: String,
    depth: usize,
    expanded: bool,
    has_children: bool,
    /// Whether this is the last child of its parent (the elbow vs. tee join).
    is_last: bool,
    /// For each ancestor level, whether that ancestor has more siblings below
    /// (i.e. draw a pass-through vertical line at that level).
    parent_lines: Vec<bool>,
}

fn has_subdirs(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten().any(|e| {
                !e.file_name().to_string_lossy().starts_with('.')
                    && e.file_type().map(|t| t.is_dir()).unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// A 1.5px vertical guide line. `full` runs the whole row height; otherwise it
/// stops at `height` (used for the elbow on a last child).
fn flatten_dirs(
    dir: &Path,
    depth: usize,
    expanded: &HashSet<PathBuf>,
    ancestors: &mut Vec<bool>,
    out: &mut Vec<TreeRow>,
) {
    let mut subs: Vec<(PathBuf, String)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                subs.push((e.path(), name));
            }
        }
    }
    subs.sort_by_key(|a| a.1.to_lowercase());
    let last = subs.len().saturating_sub(1);
    for (idx, (path, name)) in subs.into_iter().enumerate() {
        let is_last = idx == last;
        let is_exp = expanded.contains(&path);
        let has = has_subdirs(&path);
        out.push(TreeRow {
            path: path.clone(),
            name,
            depth,
            expanded: is_exp,
            has_children: has,
            is_last,
            parent_lines: ancestors.clone(),
        });
        if is_exp && has {
            // Descendants draw a pass-through line at this level iff this node
            // has a sibling after it.
            ancestors.push(!is_last);
            flatten_dirs(&path, depth + 1, expanded, ancestors, out);
            ancestors.pop();
        }
    }
}

/// A row in the tree: a section header, a favorites/recent shortcut, or a folder.
enum TreeItem {
    /// A section header. `section` is `Some` for collapsible groups
    /// (FAVORITES / RECENT); `None` for the always-open FOLDERS header.
    Header {
        label: &'static str,
        section: Option<Section>,
        open: bool,
    },
    Shortcut { name: String, path: PathBuf, is_dir: bool },
    Folder(TreeRow),
}

fn file_name_of(p: &Path) -> String {
    p.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string()
}

fn tree_snapshot(world: &World) -> KeyedSnapshot {
    let Some(root) = project_root(world) else {
        return KeyedSnapshot {
            items: Vec::new(),
            build: Box::new(|_, _, _| Entity::PLACEHOLDER),
        };
    };
    let st = world.get_resource::<NativeAssets>();
    let expanded = st.map(|s| s.expanded.clone()).unwrap_or_default();
    let favorites = st.map(|s| s.favorites.clone()).unwrap_or_default();
    let recent = st.map(|s| s.recent.clone()).unwrap_or_default();
    let fav_open = st.map(|s| s.fav_open).unwrap_or(false);
    let recent_open = st.map(|s| s.recent_open).unwrap_or(false);

    let mut items: Vec<TreeItem> = Vec::new();
    if !favorites.is_empty() {
        items.push(TreeItem::Header {
            label: "FAVORITES",
            section: Some(Section::Favorites),
            open: fav_open,
        });
        if fav_open {
            for p in &favorites {
                items.push(TreeItem::Shortcut {
                    name: file_name_of(p),
                    path: p.clone(),
                    is_dir: p.is_dir(),
                });
            }
        }
    }
    if !recent.is_empty() {
        items.push(TreeItem::Header {
            label: "RECENT",
            section: Some(Section::Recent),
            open: recent_open,
        });
        if recent_open {
            for p in recent.iter().take(20) {
                items.push(TreeItem::Shortcut {
                    name: file_name_of(p),
                    path: p.clone(),
                    is_dir: false,
                });
            }
        }
    }
    items.push(TreeItem::Header {
        label: "FOLDERS",
        section: None,
        open: true,
    });
    let mut rows = Vec::new();
    flatten_dirs(&root, 0, &expanded, &mut Vec::new(), &mut rows);
    items.extend(rows.into_iter().map(TreeItem::Folder));

    let keyed: Vec<(u64, u64)> = items
        .iter()
        .map(|it| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            let mut h = std::collections::hash_map::DefaultHasher::new();
            match it {
                TreeItem::Header { label, open, .. } => {
                    (0u8, label).hash(&mut k);
                    (label, open).hash(&mut h);
                }
                TreeItem::Shortcut { name, path, is_dir } => {
                    (1u8, path).hash(&mut k);
                    (name, is_dir).hash(&mut h);
                }
                TreeItem::Folder(r) => {
                    (2u8, &r.path).hash(&mut k);
                    (r.depth, r.expanded, r.has_children, &r.name).hash(&mut h);
                    (r.is_last, &r.parent_lines).hash(&mut h);
                }
            }
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items: keyed,
        build: Box::new(move |c, f, i| match &items[i] {
            TreeItem::Header { label, section, open } => tree_header(c, f, label, *section, *open),
            TreeItem::Shortcut { name, path, is_dir } => shortcut_row(c, f, name, path, *is_dir),
            TreeItem::Folder(r) => tree_row(c, f, r, i),
        }),
    }
}

/// A section header. Collapsible groups (FAVORITES / RECENT) get a caret and a
/// click target; the FOLDERS header is a plain label.
fn tree_header(
    commands: &mut Commands,
    fonts: &EmberFonts,
    text: &str,
    section: Option<Section>,
    open: bool,
) -> Entity {
    // Collapsible sections (FAVORITES / RECENT) sit in a subtle inset box; the
    // plain FOLDERS header is borderless.
    let boxed = section.is_some();
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                width: Val::Percent(100.0),
                column_gap: Val::Px(3.0),
                margin: if boxed {
                    UiRect::new(Val::Px(4.0), Val::Px(4.0), Val::Px(4.0), Val::Px(0.0))
                } else {
                    UiRect::ZERO
                },
                padding: if boxed {
                    UiRect::axes(Val::Px(6.0), Val::Px(4.0))
                } else {
                    UiRect::new(Val::Px(6.0), Val::Px(0.0), Val::Px(6.0), Val::Px(2.0))
                },
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(if boxed { rgb(renzora_ember::theme::popup_bg()) } else { Color::NONE }),
            Name::new("tree-header"),
        ))
        .id();
    let mut kids = Vec::new();
    if section.is_some() {
        let caret = icon_text(
            commands,
            &fonts.phosphor,
            if open { "caret-down" } else { "caret-right" },
            text_muted(),
            9.0,
        );
        kids.push(caret);
    }
    let label = commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 9.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    kids.push(label);
    commands.entity(row).add_children(&kids);
    if let Some(sec) = section {
        commands
            .entity(row)
            .insert((Interaction::default(), SectionToggle(sec)));
    }
    row
}

fn shortcut_row(commands: &mut Commands, fonts: &EmberFonts, name: &str, path: &Path, is_dir: bool) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                width: Val::Percent(100.0),
                height: Val::Px(20.0),
                padding: UiRect::left(Val::Px(8.0)),
                column_gap: Val::Px(4.0),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            ShortcutClick {
                path: path.to_path_buf(),
                is_dir,
            },
            Name::new("tree-shortcut"),
        ))
        .id();
    bind_bg(commands, row, move |w| match w.get::<Interaction>(row) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(renzora_ember::theme::hover_bg()),
        _ => Color::NONE,
    });
    let (icon_name, icon_color) = if is_dir {
        ("folder", folder_color(name))
    } else {
        (icon_for(path, false), text_muted())
    };
    let ic = icon_text(commands, &fonts.phosphor, icon_name, icon_color, 12.0);
    let label = commands
        .spawn((
            Text::new(name.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::new_with_no_wrap(),
            Node {
                min_width: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    commands.entity(row).add_children(&[ic, label]);
    row
}

fn tree_row(commands: &mut Commands, fonts: &EmberFonts, r: &TreeRow, row_index: usize) -> Entity {
    let content_x = TREE_BASE_X + r.depth as f32 * TREE_INDENT;
    let stripe = inspector_stripe(row_index);

    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                width: Val::Percent(100.0),
                height: Val::Px(TREE_ROW_H),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                ..default()
            },
            Name::new("tree-row"),
        ))
        .id();

    // Caret (only when there are subfolders).
    let caret = commands
        .spawn(Node {
            width: Val::Px(16.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_shrink: 0.0,
            ..default()
        })
        .id();
    if r.has_children {
        let glyph = icon_glyph(if r.expanded { "caret-down" } else { "caret-right" }).unwrap_or(' ');
        let g = commands
            .spawn((
                Text::new(glyph.to_string()),
                ui_font(&fonts.phosphor, 10.0),
                TextColor(rgb(text_muted())),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(caret).insert((Interaction::default(), TreeToggle(r.path.clone())));
        commands.entity(caret).add_child(g);
    }

    // Nav zone (icon + name).
    let nav = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                height: Val::Percent(100.0),
                column_gap: Val::Px(4.0),
                overflow: Overflow::clip(),
                ..default()
            },
            Interaction::default(),
            TreeNav(r.path.clone()),
            Name::new("tree-nav"),
        ))
        .id();
    let folder_icon = icon_text(
        commands,
        &fonts.phosphor,
        if r.expanded { "folder-open" } else { "folder" },
        folder_color(&r.name),
        13.0,
    );
    let name = commands
        .spawn((
            Text::new(r.name.clone()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::new_with_no_wrap(),
            Pickable::IGNORE,
            Node {
                min_width: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    commands.entity(nav).add_children(&[folder_icon, name]);

    // Full-row background (odd/even stripe + selection + hover), lowest z.
    let bg_visual = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(stripe),
            Pickable::IGNORE,
            Name::new("tree-row-bg"),
        ))
        .id();
    let sel_path = r.path.clone();
    bind_bg(commands, bg_visual, move |w| {
        if current_folder(w).as_deref() == Some(sel_path.as_path()) {
            return rgb(accent()).with_alpha(0.30);
        }
        if matches!(
            w.get::<Interaction>(nav),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            return rgb(renzora_ember::theme::border());
        }
        stripe
    });

    // bg first (behind), then guide lines, then the indent spacer + content.
    let mut kids = vec![bg_visual];
    for (level, &has_more) in r.parent_lines.iter().enumerate() {
        if has_more {
            let x = TREE_BASE_X + level as f32 * TREE_INDENT + TREE_LINE_OFFSET;
            kids.push(renzora_ember::widgets::tree_vline(commands, x, 0.0, true, 0.0));
        }
    }
    if r.depth > 0 {
        let x = TREE_BASE_X + (r.depth - 1) as f32 * TREE_INDENT + TREE_LINE_OFFSET;
        kids.push(renzora_ember::widgets::tree_vline(commands, x, 0.0, !r.is_last, TREE_CENTER_Y));
        kids.push(renzora_ember::widgets::tree_hline(commands, x, TREE_CENTER_Y, 5.0));
    }
    // Indent spacer up to the caret column.
    kids.push(
        commands
            .spawn((
                Node {
                    width: Val::Px(content_x),
                    height: Val::Percent(100.0),
                    flex_shrink: 0.0,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .id(),
    );
    kids.push(caret);
    kids.push(nav);
    commands.entity(row).add_children(&kids);
    row
}

// ── Click systems ────────────────────────────────────────────────────────────

fn tree_toggle_click(
    q: Query<(&Interaction, &TreeToggle), Changed<Interaction>>,
    mut state: ResMut<NativeAssets>,
) {
    for (interaction, toggle) in &q {
        if *interaction == Interaction::Pressed {
            if state.expanded.contains(&toggle.0) {
                state.expanded.remove(&toggle.0);
            } else {
                state.expanded.insert(toggle.0.clone());
            }
        }
    }
}

fn tree_nav_click(
    q: Query<(&Interaction, &TreeNav), Changed<Interaction>>,
    mut state: ResMut<NativeAssets>,
) {
    for (interaction, nav) in &q {
        if *interaction == Interaction::Pressed {
            state.current = Some(nav.0.clone());
            state.selected = None;
            // Clicking anywhere on a folder row also toggles its expansion.
            if state.expanded.contains(&nav.0) {
                state.expanded.remove(&nav.0);
            } else {
                state.expanded.insert(nav.0.clone());
            }
        }
    }
}

fn section_toggle_click(
    q: Query<(&Interaction, &SectionToggle), Changed<Interaction>>,
    mut state: ResMut<NativeAssets>,
) {
    for (interaction, toggle) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match toggle.0 {
            Section::Favorites => state.fav_open = !state.fav_open,
            Section::Recent => state.recent_open = !state.recent_open,
        }
    }
}

fn tile_click(
    q: Query<(&Interaction, &AssetTile), Changed<Interaction>>,
    mut state: ResMut<NativeAssets>,
    time: Res<Time>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
    let now = time.elapsed_secs_f64();
    let root = project.as_ref().map(|p| p.path.clone());
    for (interaction, tile) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let double = state
            .last_click
            .as_ref()
            .is_some_and(|(p, t)| p == &tile.path && now - t < 0.4);
        if double {
            state.last_click = None;
            if tile.is_dir {
                state.current = Some(tile.path.clone());
                state.selected = None;
            } else {
                os_open(&tile.path);
                track_recent(&mut state, &tile.path, root.as_deref());
            }
        } else {
            // Single click selects; the next click within 0.4s opens/navigates.
            state.selected = Some(tile.path.clone());
            state.last_click = Some((tile.path.clone(), now));
        }
    }
}

/// Open a file with its OS default application.
fn os_open(path: &Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(path)
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}

fn back_click(
    q: Query<&Interaction, (With<AssetBack>, Changed<Interaction>)>,
    mut state: ResMut<NativeAssets>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Some(root) = project.map(|p| p.path.clone()) else {
        return;
    };
    let cur = state.current.clone().unwrap_or_else(|| root.clone());
    if cur == root {
        return;
    }
    if let Some(parent) = cur.parent() {
        // Don't navigate above the project root.
        if parent.starts_with(&root) || parent == root {
            state.current = Some(parent.to_path_buf());
            state.selected = None;
        }
    }
}

/// Kick off thumbnail loads for visible tiles (each registry de-dupes).
fn request_thumbnails(
    tiles: Query<&AssetTile>,
    mut cache: ResMut<ThumbnailCache>,
    mut model: Option<ResMut<ModelThumbnailRegistry>>,
    mut material: Option<ResMut<MaterialThumbnailRegistry>>,
    asset_server: Res<AssetServer>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
    let project = project.as_deref();
    for tile in &tiles {
        if tile.is_dir {
            continue;
        }
        let Some(name) = tile.path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        match thumb_kind(name) {
            Some(ThumbKind::Image) => {
                cache.request(tile.path.clone(), &asset_server, project);
            }
            Some(ThumbKind::Model) => {
                if let Some(model) = model.as_mut() {
                    model.request(tile.path.clone());
                }
            }
            Some(ThumbKind::Material) => {
                if let Some(material) = material.as_mut() {
                    material.request(tile.path.clone());
                }
            }
            None => {}
        }
    }
}

fn search_sync(input: Query<&EmberTextInput, With<AssetSearch>>, mut state: ResMut<NativeAssets>) {
    for inp in &input {
        if state.search != inp.value {
            state.search = inp.value.clone();
        }
    }
}
