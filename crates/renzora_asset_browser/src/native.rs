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

use bevy::prelude::*;

use renzora_editor::{MaterialThumbnailRegistry, ModelThumbnailRegistry, SplashState};
use renzora_ember::font::{icon_glyph, icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_bg, bind_with, keyed_list, KeyedSnapshot};
use renzora_ember::theme::{rgb, ACCENT_BLUE, TEXT_MUTED, TEXT_PRIMARY};
use renzora_ember::widgets::{scroll_view, text_input, EmberTextInput};

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

const TILE_W: f32 = 84.0;
const HOVER_BG: (u8, u8, u8) = (40, 40, 50);

/// Lean native state for the browser (independent of the egui panel's state).
#[derive(Resource, Default)]
pub(crate) struct NativeAssets {
    current: Option<PathBuf>,
    selected: Option<PathBuf>,
    search: String,
    /// Expanded folders in the left tree.
    expanded: HashSet<PathBuf>,
    /// The grid tile the cursor is currently over (for right-click targeting).
    hovered: Option<PathBuf>,
    /// The asset the open context menu acts on (`None` = menu closed).
    context: Option<PathBuf>,
}

#[derive(Clone, Copy)]
enum Action {
    Duplicate,
    Delete,
    Reveal,
}

#[derive(Component)]
struct AssetRoot;
#[derive(Component)]
struct AssetContextMenu;
#[derive(Component)]
struct ContextAction(Action);

#[derive(Component)]
struct AssetTile {
    path: PathBuf,
    is_dir: bool,
}
#[derive(Component)]
struct AssetBack;
#[derive(Component)]
struct AssetSearch;
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
            create_asset_click,
            import_click,
            crumb_click,
        )
            .run_if(in_state(SplashState::Editor)),
    );
    app.add_systems(
        Update,
        (track_hover, context_action_click, context_open)
            .chain()
            .run_if(in_state(SplashState::Editor)),
    );
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

fn context_action_click(
    q: Query<(&Interaction, &ContextAction), Changed<Interaction>>,
    mut state: ResMut<NativeAssets>,
    mut menu: Query<&mut Node, With<AssetContextMenu>>,
) {
    for (interaction, action) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(path) = state.context.clone() else {
            continue;
        };
        match action.0 {
            Action::Duplicate => duplicate_asset(&path),
            Action::Delete => {
                let _ = if path.is_dir() {
                    std::fs::remove_dir_all(&path)
                } else {
                    std::fs::remove_file(&path)
                };
                if state.selected.as_deref() == Some(path.as_path()) {
                    state.selected = None;
                }
            }
            Action::Reveal => reveal_in_explorer(&path),
        }
        state.context = None;
        for mut n in &mut menu {
            n.display = Display::None;
        }
    }
}

fn context_open(
    mouse: Res<ButtonInput<MouseButton>>,
    roots: Query<(&bevy::ui::RelativeCursorPosition, &bevy::ui::ComputedNode), With<AssetRoot>>,
    mut menu: Query<&mut Node, With<AssetContextMenu>>,
    mut state: ResMut<NativeAssets>,
) {
    let right = mouse.just_pressed(MouseButton::Right);
    let left = mouse.just_pressed(MouseButton::Left);
    if !right && !left {
        return;
    }
    for (rcp, computed) in &roots {
        if right && rcp.cursor_over {
            if let (Some(nrm), Some(path)) = (rcp.normalized, state.hovered.clone()) {
                let size = computed.size() * computed.inverse_scale_factor();
                state.context = Some(path);
                for mut n in &mut menu {
                    n.left = Val::Px((nrm.x + 0.5) * size.x);
                    n.top = Val::Px((nrm.y + 0.5) * size.y);
                    n.display = Display::Flex;
                }
            }
        } else if left && state.context.is_some() {
            state.context = None;
            for mut n in &mut menu {
                n.display = Display::None;
            }
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

fn icon_for(path: &Path, is_dir: bool) -> &'static str {
    if is_dir {
        return "folder";
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "ktx2" | "dds" | "bmp" | "tga" => "image",
        "glb" | "gltf" | "obj" | "fbx" => "cube",
        "material" => "palette",
        "lua" | "rhai" | "rs" | "wgsl" | "py" => "code",
        "scene" | "ron" | "scn" => "stack",
        "wav" | "ogg" | "mp3" | "flac" => "speaker-high",
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
                border: UiRect::right(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(rgb((48, 48, 58))),
        ))
        .id();
    commands.entity(tree_pane).add_child(tree_scroll);

    // ── Content (toolbar + grid, own scroll) ──
    let content = commands
        .spawn(Node {
            flex_grow: 1.0,
            flex_direction: FlexDirection::Column,
            min_width: Val::Px(0.0),
            min_height: Val::Px(0.0),
            ..default()
        })
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
            BackgroundColor(rgb((26, 26, 32))),
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
            BackgroundColor(rgb((42, 42, 52))),
            Interaction::default(),
            AssetBack,
            Name::new("assets-back"),
        ))
        .id();
    let back_icon = icon_text(commands, &fonts.phosphor, "arrow-left", TEXT_PRIMARY, 13.0);
    commands.entity(back).add_child(back_icon);

    let new_folder = toolbar_btn(commands, fonts, "folder-plus", "New Folder");
    commands.entity(new_folder).insert(NewAssetBtn(NewAsset::Folder));
    let add = add_menu(commands, fonts);

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
    let import = toolbar_btn(commands, fonts, "download-simple", "Import");
    commands.entity(import).insert(ImportBtn);
    let search = text_input(commands, &fonts.ui, "Search...", "");
    commands.entity(search).insert(AssetSearch);
    commands
        .entity(toolbar)
        .add_children(&[back, new_folder, add, crumbs, spacer, import, search]);

    // Grid.
    let grid = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            width: Val::Percent(100.0),
            align_items: AlignItems::FlexStart,
            align_content: AlignContent::FlexStart,
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            padding: UiRect::all(Val::Px(6.0)),
            ..default()
        })
        .id();
    keyed_list(commands, grid, grid_snapshot);
    let grid_scroll = scroll_view(commands, grid);

    commands.entity(content).add_children(&[toolbar, grid_scroll]);

    // Context menu (shared, repositioned on right-click).
    let menu = context_menu(commands, fonts);

    commands.entity(root).add_children(&[tree_pane, content, menu]);
    root
}

/// The shared right-click context menu (hidden until opened).
fn context_menu(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let menu = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                min_width: Val::Px(140.0),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb((30, 30, 38))),
            BorderColor::all(rgb((60, 60, 74))),
            GlobalZIndex(700),
            AssetContextMenu,
            Name::new("asset-context-menu"),
        ))
        .id();
    let items = [
        ("copy", "Duplicate", Action::Duplicate),
        ("folder-open", "Reveal in Explorer", Action::Reveal),
        ("trash", "Delete", Action::Delete),
    ];
    let rows: Vec<Entity> = items
        .iter()
        .map(|(icon, label, action)| {
            let row = commands
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    Interaction::default(),
                    ContextAction(*action),
                    Name::new("ctx-action"),
                ))
                .id();
            let danger = matches!(action, Action::Delete);
            let color = if danger { (220, 90, 80) } else { TEXT_MUTED };
            let ic = icon_text(commands, &fonts.phosphor, icon, color, 12.0);
            let t = commands
                .spawn((
                    Text::new(*label),
                    ui_font(&fonts.ui, 11.0),
                    TextColor(rgb(if danger { (220, 90, 80) } else { TEXT_PRIMARY })),
                ))
                .id();
            commands.entity(row).add_children(&[ic, t]);
            row
        })
        .collect();
    commands.entity(menu).add_children(&rows);
    menu
}

/// A framed icon+label toolbar button (caller inserts the marker component).
fn toolbar_btn(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str) -> Entity {
    let b = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb((42, 42, 52))),
            Interaction::default(),
            Name::new("toolbar-btn"),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, TEXT_MUTED, 12.0);
    let t = commands
        .spawn((Text::new(label), ui_font(&fonts.ui, 11.0), TextColor(rgb(TEXT_MUTED))))
        .id();
    commands.entity(b).add_children(&[ic, t]);
    b
}

/// The "Add" asset-creation popover (material/blueprint/lua/rhai/particle).
fn add_menu(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let content = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Px(150.0),
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    let items = [
        ("palette", "Material", NewAsset::Material),
        ("blueprint", "Blueprint", NewAsset::Blueprint),
        ("code", "Lua Script", NewAsset::Lua),
        ("code", "Rhai Script", NewAsset::Rhai),
        ("sparkle", "Particle", NewAsset::Particle),
    ];
    let rows: Vec<Entity> = items
        .iter()
        .map(|(icon, label, kind)| {
            let row = commands
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        padding: UiRect::axes(Val::Px(4.0), Val::Px(3.0)),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    Interaction::default(),
                    NewAssetBtn(*kind),
                    Name::new("add-asset"),
                ))
                .id();
            let ic = icon_text(commands, &fonts.phosphor, icon, TEXT_MUTED, 12.0);
            let t = commands
                .spawn((Text::new(*label), ui_font(&fonts.ui, 11.0), TextColor(rgb(TEXT_PRIMARY))))
                .id();
            commands.entity(row).add_children(&[ic, t]);
            row
        })
        .collect();
    commands.entity(content).add_children(&rows);
    renzora_ember::widgets::popover(commands, fonts, "Add", content)
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
        kids.push(icon_text(commands, &fonts.phosphor, "caret-right", TEXT_MUTED, 9.0));
    }
    let label = commands
        .spawn((
            Text::new(name.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(TEXT_PRIMARY)),
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
                    TextColor(rgb(TEXT_MUTED)),
                    Node { padding: UiRect::all(Val::Px(8.0)), ..default() },
                ))
                .id()
            }),
        };
    }
    let items: Vec<(u64, u64)> = entries
        .iter()
        .map(|e| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (&e.name, e.is_dir).hash(&mut h);
            let mut k = std::collections::hash_map::DefaultHasher::new();
            e.path.hash(&mut k);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| tile(c, f, &entries[i])),
    }
}

fn tile(commands: &mut Commands, fonts: &EmberFonts, entry: &Entry) -> Entity {
    let path = entry.path.clone();
    let col = commands
        .spawn((
            Node {
                width: Val::Px(TILE_W),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(3.0),
                padding: UiRect::axes(Val::Px(2.0), Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            AssetTile {
                path: entry.path.clone(),
                is_dir: entry.is_dir,
            },
            Name::new("asset-tile"),
        ))
        .id();
    bind_bg(commands, col, move |w| {
        let selected = w
            .get_resource::<NativeAssets>()
            .map(|s| s.selected.as_deref() == Some(path.as_path()))
            .unwrap_or(false);
        if selected {
            return rgb(ACCENT_BLUE).with_alpha(0.30);
        }
        match w.get::<Interaction>(col) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(HOVER_BG),
            _ => Color::NONE,
        }
    });
    // Preview box: an icon, with a thumbnail ImageNode layered on top that
    // reveals once the cached Handle<Image> is ready (image files only for now).
    let preview = commands
        .spawn(Node {
            width: Val::Px(64.0),
            height: Val::Px(64.0),
            position_type: PositionType::Relative,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            overflow: Overflow::clip(),
            ..default()
        })
        .id();
    let color = if entry.is_dir { (220, 200, 130) } else { TEXT_MUTED };
    let icon = icon_text(commands, &fonts.phosphor, icon_for(&entry.path, entry.is_dir), color, 30.0);
    commands.entity(preview).add_child(icon);

    if let Some(kind) = (!entry.is_dir).then(|| thumb_kind(&entry.name)).flatten() {
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
        commands.entity(preview).add_child(img);
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

    let name = commands
        .spawn((
            Text::new(entry.name.clone()),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(TEXT_PRIMARY)),
            bevy::text::TextLayout::new_with_no_wrap(),
            Node { max_width: Val::Px(TILE_W - 4.0), overflow: Overflow::clip(), ..default() },
        ))
        .id();
    commands.entity(col).add_children(&[preview, name]);
    col
}

// ── Folder tree ──────────────────────────────────────────────────────────────

struct TreeRow {
    path: PathBuf,
    name: String,
    depth: usize,
    expanded: bool,
    has_children: bool,
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

fn flatten_dirs(dir: &Path, depth: usize, expanded: &HashSet<PathBuf>, out: &mut Vec<TreeRow>) {
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
    for (path, name) in subs {
        let is_exp = expanded.contains(&path);
        let has = has_subdirs(&path);
        out.push(TreeRow {
            path: path.clone(),
            name,
            depth,
            expanded: is_exp,
            has_children: has,
        });
        if is_exp && has {
            flatten_dirs(&path, depth + 1, expanded, out);
        }
    }
}

fn tree_snapshot(world: &World) -> KeyedSnapshot {
    let Some(root) = project_root(world) else {
        return KeyedSnapshot {
            items: Vec::new(),
            build: Box::new(|_, _, _| Entity::PLACEHOLDER),
        };
    };
    let expanded = world
        .get_resource::<NativeAssets>()
        .map(|s| s.expanded.clone())
        .unwrap_or_default();
    let mut rows = Vec::new();
    flatten_dirs(&root, 0, &expanded, &mut rows);
    let items: Vec<(u64, u64)> = rows
        .iter()
        .map(|r| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            r.path.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (r.depth, r.expanded, r.has_children, &r.name).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| tree_row(c, f, &rows[i])),
    }
}

fn tree_row(commands: &mut Commands, fonts: &EmberFonts, r: &TreeRow) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                width: Val::Percent(100.0),
                height: Val::Px(20.0),
                padding: UiRect::left(Val::Px(r.depth as f32 * 12.0 + 2.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Name::new("tree-row"),
        ))
        .id();
    let sel_path = r.path.clone();
    bind_bg(commands, row, move |w| {
        if current_folder(w).as_deref() == Some(sel_path.as_path()) {
            rgb(ACCENT_BLUE).with_alpha(0.25)
        } else {
            Color::NONE
        }
    });

    // Caret (only when there are subfolders).
    let caret = commands
        .spawn(Node {
            width: Val::Px(14.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .id();
    if r.has_children {
        let glyph = icon_glyph(if r.expanded { "caret-down" } else { "caret-right" }).unwrap_or(' ');
        let g = commands
            .spawn((Text::new(glyph.to_string()), ui_font(&fonts.phosphor, 10.0), TextColor(rgb(TEXT_MUTED))))
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
                height: Val::Percent(100.0),
                column_gap: Val::Px(4.0),
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
        (220, 200, 130),
        13.0,
    );
    let name = commands
        .spawn((
            Text::new(r.name.clone()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(TEXT_PRIMARY)),
            bevy::text::TextLayout::new_with_no_wrap(),
            Node { overflow: Overflow::clip(), ..default() },
        ))
        .id();
    commands.entity(nav).add_children(&[folder_icon, name]);
    commands.entity(row).add_children(&[caret, nav]);
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
        }
    }
}

fn tile_click(
    q: Query<(&Interaction, &AssetTile), Changed<Interaction>>,
    mut state: ResMut<NativeAssets>,
) {
    for (interaction, tile) in &q {
        if *interaction == Interaction::Pressed {
            if tile.is_dir {
                state.current = Some(tile.path.clone());
                state.selected = None;
            } else {
                state.selected = Some(tile.path.clone());
            }
        }
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
