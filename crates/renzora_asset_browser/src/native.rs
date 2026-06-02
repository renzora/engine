//! Bevy-native (ember) Asset Browser — Increment 1: a navigable tile grid.
//!
//! Toolbar (back + breadcrumb + search) over a wrapping grid of folder/file
//! tiles read from the current folder. Click a folder to navigate in, a file to
//! select. Thumbnails, the folder tree, drag-drop, rename/delete and the context
//! menu are later increments; the egui `AssetBrowserPanel` stays the source for
//! those until then.

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use bevy::prelude::*;

use renzora_editor::SplashState;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_bg, bind_text, bind_with, keyed_list, KeyedSnapshot};
use renzora_ember::theme::{rgb, ACCENT_BLUE, TEXT_MUTED, TEXT_PRIMARY};
use renzora_ember::widgets::{text_input, EmberTextInput};

use crate::thumbnails::{supports_thumbnail, ThumbnailCache};

const TILE_W: f32 = 84.0;
const HOVER_BG: (u8, u8, u8) = (40, 40, 50);

/// Lean native state for the browser (independent of the egui panel's state).
#[derive(Resource, Default)]
pub(crate) struct NativeAssets {
    current: Option<PathBuf>,
    selected: Option<PathBuf>,
    search: String,
}

#[derive(Component)]
struct AssetTile {
    path: PathBuf,
    is_dir: bool,
}
#[derive(Component)]
struct AssetBack;
#[derive(Component)]
struct AssetSearch;

pub fn register_native_asset_browser(app: &mut App) {
    app.init_resource::<NativeAssets>();
    app.register_panel_content("assets", true, build);
    app.add_systems(
        Update,
        (tile_click, back_click, search_sync, request_thumbnails).run_if(in_state(SplashState::Editor)),
    );
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
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
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
    let crumb = commands
        .spawn((Text::new(""), ui_font(&fonts.mono, 11.0), TextColor(rgb(TEXT_MUTED))))
        .id();
    bind_text(commands, crumb, breadcrumb);
    let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let search = text_input(commands, &fonts.ui, "Search...", "");
    commands.entity(search).insert(AssetSearch);
    commands.entity(toolbar).add_children(&[back, crumb, spacer, search]);

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

    commands.entity(root).add_children(&[toolbar, grid]);
    root
}

fn breadcrumb(w: &World) -> String {
    let (Some(cur), Some(root)) = (current_folder(w), project_root(w)) else {
        return String::new();
    };
    cur.strip_prefix(&root)
        .ok()
        .map(|rel| {
            let r = rel.to_string_lossy();
            if r.is_empty() {
                "assets".to_string()
            } else {
                format!("assets/{}", r.replace('\\', "/"))
            }
        })
        .unwrap_or_else(|| cur.to_string_lossy().to_string())
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

    if !entry.is_dir && supports_thumbnail(&entry.name) {
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
            move |w| w.get_resource::<ThumbnailCache>().and_then(|c| c.handle(&p)),
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

// ── Click systems ────────────────────────────────────────────────────────────

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

/// Kick off thumbnail loads for visible image tiles (the cache de-dupes).
fn request_thumbnails(
    tiles: Query<&AssetTile>,
    mut cache: ResMut<ThumbnailCache>,
    asset_server: Res<AssetServer>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
    let project = project.as_deref();
    for tile in &tiles {
        if tile.is_dir {
            continue;
        }
        if let Some(name) = tile.path.file_name().and_then(|n| n.to_str()) {
            if supports_thumbnail(name) {
                cache.request(tile.path.clone(), &asset_server, project);
            }
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
