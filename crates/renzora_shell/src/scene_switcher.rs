//! The top bar's scene switcher: `project / document ▾` at the head of the bar,
//! opening onto every open document and every scene you can go to, and the
//! saved indicator the status bar shows beside "Ready".
//!
//! It replaced three things. The **Scenes panel** listed the project's scenes in
//! a dock tab that shared a column with the Hierarchy, so it was usually behind
//! it, and it was the only place to set the boot scene. The **document tab
//! strip** spent a whole row of the window on what was, most of the time, one
//! or two tabs, and a Settings option offered a **document dropdown** in its
//! place. "What am I editing, and is it saved" is the question the editor
//! answers most often, so it sits where the eye starts, and the documents you
//! have open are one click from it.
//!
//! The label names any kind of document: `test / main` for a scene, `test /
//! gold.material` for a material. Picking a document in the menu activates it,
//! and the workspace follows as it always has (see
//! [`crate::documents::sync_workspace_to_active_doc`]).
//!
//! The menu is built each time it opens rather than kept live: the scene list
//! comes off the disk, and a menu nobody is looking at has no reason to re-read
//! a folder.

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora_ember::font::{glyph, icon_text, ui_font, EmberFonts};
use renzora_ember::interaction::{BLOCK, PASS_THROUGH};
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text, bind_text_color, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::{play_green, rgb, text_muted, text_primary, warn_amber};
use renzora_ember::widgets::{menu_item, menu_item_styled, trigger_rect, MenuAction};
use renzora_ember::UiInteraction;

use crate::documents::{activate_doc_tab, doc_closable, request_close_doc};

/// The trigger button.
#[derive(Component)]
pub(crate) struct SceneSwitcherBtn;

/// Glyph shown before a name whose icon does not resolve (matches `icon_text`).
const ICON_FALLBACK: char = '\u{E4C6}';

/// Build the switcher: the project's name, a slash, the active document's type
/// icon and name, and a caret.
pub(crate) fn build_scene_switcher(
    commands: &mut Commands,
    fonts: &EmberFonts,
    font: &bevy::text::FontSource,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                height: Val::Px(24.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                max_width: Val::Px(360.0),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            RelativeCursorPosition::default(),
            SceneSwitcherBtn,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(renzora::lang::t("scene_switcher.tooltip")),
            Name::new("scene-switcher"),
        ))
        .id();
    bind_bg(commands, btn, move |w| match w.get::<UiInteraction>(btn) {
        // The hamburger's hover, so the bar's two menus answer the same way.
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(renzora_ember::theme::hover_bg()),
        _ => Color::NONE,
    });

    let project = label(commands, font, text_muted());
    bind_text(commands, project, |w: &Rx| {
        w.get_resource::<renzora::CurrentProject>()
            .map(|p| p.config.name.clone())
            .unwrap_or_default()
    });
    let slash = label(commands, font, text_muted());
    commands.entity(slash).insert(Text::new("/"));

    // The document's type glyph in its type colour, the same mark the asset
    // browser gives that kind of file, so a material reads as a material before
    // its name is read.
    let kind = renzora_ui::DocTabKind::Scene;
    let icon = icon_text(commands, &fonts.phosphor, kind.icon(), kind.color(), 14.0);
    commands.entity(icon).insert(PASS_THROUGH);
    bind_text(commands, icon, |w: &Rx| {
        let name = active_doc(w).map(|t| t.kind.icon()).unwrap_or(renzora_ui::DocTabKind::Scene.icon());
        renzora_ember::phosphor_map::icon_glyph(name)
            .unwrap_or(ICON_FALLBACK)
            .to_string()
    });
    bind_text_color(commands, icon, |w: &Rx| {
        rgb(active_doc(w)
            .map(|t| t.kind.color())
            .unwrap_or_else(|| renzora_ui::DocTabKind::Scene.color()))
    });
    let doc = label(commands, font, text_primary());
    bind_text(commands, doc, |w: &Rx| active_doc(w).map(doc_label).unwrap_or_default());
    let caret = glyph(commands, "caret-down", text_muted(), 11.0);
    commands.entity(caret).insert(PASS_THROUGH);

    commands.entity(btn).add_children(&[project, slash, icon, doc, caret]);
    btn
}

/// The active document's saved state, as a dot and a word: green "Saved", amber
/// "Unsaved". It reads the document's `is_modified` flag.
///
/// It sits in the status bar beside "Ready", not in the top bar beside the
/// switcher: it is a passive fact about the session like the rest of that bar,
/// and next to the document name it read as part of a button you could click.
pub(crate) fn build_saved_badge(commands: &mut Commands, font: &bevy::text::FontSource) -> Entity {
    let badge = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                flex_shrink: 0.0,
                ..default()
            },
            Name::new("status-saved"),
        ))
        .id();
    let dot = commands
        .spawn((
            Node {
                width: Val::Px(6.0),
                height: Val::Px(6.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(play_green())),
            PASS_THROUGH,
        ))
        .id();
    bind_bg(commands, dot, |w| {
        if doc_modified(w) {
            rgb(warn_amber())
        } else {
            rgb(play_green())
        }
    });
    let state = commands
        .spawn((
            Text::new(String::new()),
            ui_font(font, 11.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
        ))
        .id();
    bind_text(commands, state, |w: &Rx| {
        renzora::lang::t(if doc_modified(w) {
            "scene_switcher.unsaved"
        } else {
            "scene_switcher.saved"
        })
    });
    bind_text_color(commands, state, |w: &Rx| {
        if doc_modified(w) {
            rgb(warn_amber())
        } else {
            rgb(text_muted())
        }
    });

    bind_display(commands, badge, |w| active_doc(w).is_some());
    commands.entity(badge).add_children(&[dot, state]);
    badge
}

/// A single-line label inside the trigger.
fn label(commands: &mut Commands, font: &bevy::text::FontSource, color: (u8, u8, u8)) -> Entity {
    commands
        .spawn((
            Text::new(String::new()),
            ui_font(font, 14.0),
            TextColor(rgb(color)),
            bevy::text::TextLayout::no_wrap(),
            PASS_THROUGH,
        ))
        .id()
}

/// The active document, of any kind.
fn active_doc<'w>(w: &Rx<'w>) -> Option<&'w renzora_ui::DocumentTab> {
    w.get_resource::<renzora_ui::DocumentTabState>()
        .and_then(|s| s.tabs.get(s.active_tab))
}

fn doc_modified(w: &Rx) -> bool {
    active_doc(w).is_some_and(|t| t.is_modified)
}

/// How a document is named in the switcher. A scene is its name (`main`): the
/// switcher is the scene switcher, and every scene is a `.bsn`. Anything else is
/// its file name with the extension (`gold.material`), because a material, a
/// script and a shader can share a stem and the extension is what tells them
/// apart. A document with no file yet falls back to its name.
fn doc_label(t: &renzora_ui::DocumentTab) -> String {
    if t.kind == renzora_ui::DocTabKind::Scene {
        return t.name.clone();
    }
    t.scene_path
        .as_deref()
        .and_then(|p| Path::new(p).file_name())
        .and_then(|n| n.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| t.name.clone())
}

/// The project's scene files, `<project>/scenes/*.bsn`, sorted by name.
fn project_scenes(project: &renzora::CurrentProject) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(project.resolve_path("scenes")) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = read
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("bsn"))
        .collect();
    out.sort_by_key(|p| file_stem(p).to_lowercase());
    out
}

fn file_stem(p: &Path) -> String {
    p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string()
}

/// One open document, as `menu_open_docs` lists it.
struct OpenDoc {
    id: u64,
    label: String,
    kind: renzora_ui::DocTabKind,
    active: bool,
    modified: bool,
    closable: bool,
}

/// Click the switcher: open the menu under it.
///
/// Three groups. **Open** lists every open document, of every kind, so going
/// back to one keeps its unsaved state; each closable one has an **x**.
/// **Project** lists every scene file in the project; the boot scene is
/// starred, and picking one opens it as a document of its own (or switches to
/// the one it already has). Then the actions: a new scene, opening one from
/// elsewhere, and, while a scene is the active document, making it the one the
/// game boots into.
pub(crate) fn scene_switcher_open(
    q: Query<(&UiInteraction, &RelativeCursorPosition, &ComputedNode), (With<SceneSwitcherBtn>, Changed<UiInteraction>)>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    project: Option<Res<renzora::CurrentProject>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, rcp, cn)) = q.iter().find(|(i, ..)| **i == UiInteraction::Pressed) else {
        return;
    };
    let Some((win_h, cursor)) = windows
        .iter()
        .find_map(|w| w.cursor_position().map(|c| (w.height(), c)))
    else {
        return;
    };

    let boot = project.as_deref().map(|p| p.main_scene_path());
    let scenes = project.as_deref().map(project_scenes).unwrap_or_default();

    // The scene list is read off the disk once, here, and kept for the menu's
    // life: the lists below are reactive (the search filters them as you
    // type), and a reactive list must not walk a folder every frame.
    commands.insert_resource(SwitcherView { query: String::new(), collapsed: Vec::new(), scenes, boot });

    // A fixed panel rather than ember's scrolling menu card: in the card the
    // whole menu scrolled as one, so the footer was only reachable by
    // scrolling past every open document. Here only the Open column scrolls.
    let rect = trigger_rect(cursor, rcp, cn);
    let height = SWITCHER_H.min((win_h - rect.max.y - 12.0).max(240.0));
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(rect.min.x),
                top: Val::Px(rect.max.y + 2.0),
                width: Val::Px(560.0),
                height: Val::Px(height),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::popup_bg())),
            BorderColor::all(rgb(renzora_ember::theme::border())),
            GlobalZIndex(9000),
            renzora_ember::widgets::ScreenMenu,
            renzora_ember::widgets::OverlaySurface,
            BLOCK,
            RelativeCursorPosition::default(),
            Name::new("scene-switcher-menu"),
        ))
        .id();
    let body = build_switcher_body(&mut commands, &fonts);
    commands.entity(root).add_child(body);
}

/// The switcher panel's height, when the window has room for it.
const SWITCHER_H: f32 = 440.0;

// ── The switcher's menu: search, two columns, footer ─────────────────────────

/// The open menu's state: what the search box says, which document groups are
/// folded, and the project's scenes as read when the menu opened.
#[derive(Resource, Default)]
pub(crate) struct SwitcherView {
    query: String,
    /// Folded groups, by the kind's persisted name.
    collapsed: Vec<&'static str>,
    scenes: Vec<PathBuf>,
    boot: Option<PathBuf>,
}

/// The menu's search field.
#[derive(Component)]
pub(crate) struct SwitcherSearch;

/// A scene row's star: makes that scene the boot scene. Not a `MenuAction`,
/// so the menu stays open and the star can be seen moving.
#[derive(Component)]
pub(crate) struct SceneBootBtn(PathBuf);

/// An Open row's x: closes that document. Not a `MenuAction`, which closes
/// the menu, so you can close several documents in a row and watch each one
/// leave the list.
#[derive(Component)]
pub(crate) struct SwitcherCloseDoc(u64);

/// A group header in the Open column (the kind it folds). Not a `MenuAction`:
/// folding a group must not close the menu.
#[derive(Component)]
pub(crate) struct SwitcherGroup(&'static str);

/// The kinds, in the order the Open column groups them.
const KIND_ORDER: [renzora_ui::DocTabKind; 8] = [
    renzora_ui::DocTabKind::Scene,
    renzora_ui::DocTabKind::Material,
    renzora_ui::DocTabKind::Shader,
    renzora_ui::DocTabKind::Script,
    renzora_ui::DocTabKind::Blueprint,
    renzora_ui::DocTabKind::Particle,
    renzora_ui::DocTabKind::Ui,
    renzora_ui::DocTabKind::Other,
];

fn kind_key(kind: renzora_ui::DocTabKind) -> &'static str {
    match kind {
        renzora_ui::DocTabKind::Scene => "scene",
        renzora_ui::DocTabKind::Material => "material",
        renzora_ui::DocTabKind::Particle => "particle",
        renzora_ui::DocTabKind::Blueprint => "blueprint",
        renzora_ui::DocTabKind::Script => "script",
        renzora_ui::DocTabKind::Shader => "shader",
        renzora_ui::DocTabKind::Ui => "ui",
        renzora_ui::DocTabKind::Other => "other",
    }
}

fn kind_title(kind: renzora_ui::DocTabKind) -> String {
    let (key, fallback) = match kind {
        renzora_ui::DocTabKind::Scene => ("scene_switcher.group.scenes", "Scenes"),
        renzora_ui::DocTabKind::Material => ("scene_switcher.group.materials", "Materials"),
        renzora_ui::DocTabKind::Particle => ("scene_switcher.group.particles", "Particles"),
        renzora_ui::DocTabKind::Blueprint => ("scene_switcher.group.blueprints", "Blueprints"),
        renzora_ui::DocTabKind::Script => ("scene_switcher.group.scripts", "Scripts"),
        renzora_ui::DocTabKind::Shader => ("scene_switcher.group.shaders", "Shaders"),
        renzora_ui::DocTabKind::Ui => ("scene_switcher.group.ui", "UI"),
        renzora_ui::DocTabKind::Other => ("scene_switcher.group.other", "Other"),
    };
    renzora::lang::t_or(key, fallback)
}

fn view<'w>(w: &Rx<'w>) -> Option<&'w SwitcherView> {
    w.get_resource::<SwitcherView>()
}

fn matches(query: &str, name: &str) -> bool {
    query.is_empty() || name.to_lowercase().contains(query)
}

/// The menu: a search box, then **project scenes** on the left and **open
/// documents** grouped by type on the right, then the actions.
///
/// It was one long list: every open document, then the project's scenes,
/// then the actions. With a few materials and shaders open, the scenes, the
/// thing a scene switcher is for, were pushed under the fold. Two columns
/// keep both in view; the search narrows both at once; grouping by type
/// (each group folds) keeps a pile of open files readable.
fn build_switcher_body(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let body = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() },
            Name::new("scene-switcher-body"),
        ))
        .id();

    let search_row = commands
        .spawn(Node { width: Val::Percent(100.0), padding: UiRect::all(Val::Px(6.0)), ..default() })
        .id();
    let search = renzora_ember::widgets::search_input(
        commands,
        fonts,
        &renzora::lang::t_or("scene_switcher.search", "Find a scene or document..."),
        "",
    );
    commands.entity(search).insert(SwitcherSearch);
    commands.entity(search).entry::<Node>().and_modify(|mut n| {
        n.width = Val::Percent(100.0);
        n.height = Val::Px(28.0);
    });
    // Focused on open, so typing straight after the click narrows the lists.
    commands.queue(move |w: &mut World| {
        if let Some(mut i) = w.get_mut::<renzora_ember::widgets::EmberTextInput>(search) {
            i.focused = true;
        }
    });
    commands.entity(search_row).add_child(search);

    let columns = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(0.0), flex_direction: FlexDirection::Row, ..default() })
        .id();

    // Left: the project's scenes.
    let left = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::right(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::faint_bg())),
            BorderColor::all(rgb(renzora_ember::theme::border())),
        ))
        .id();
    let left_head = column_header(commands, fonts, &renzora::lang::t("scene_switcher.project"), |w| {
        view(w).map(|v| v.scenes.iter().filter(|p| matches(&v.query, &file_stem(p))).count()).unwrap_or(0)
    });
    let scene_list = commands.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).id();
    keyed_list(commands, scene_list, scenes_snapshot);
    let spring = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let new_scene = menu_item(commands, fonts, "plus", &renzora::lang::t("menu.file.new_scene"), |w| {
        w.insert_resource(renzora::core::NewSceneRequested)
    });
    commands.entity(left).add_children(&[left_head, scene_list, spring, new_scene]);

    // Right: open documents, grouped by type.
    let right = commands
        .spawn(Node {
            flex_grow: 1.0,
            flex_basis: Val::Px(0.0),
            min_width: Val::Px(0.0),
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(4.0)),
            ..default()
        })
        .id();
    let right_head = column_header(commands, fonts, &renzora::lang::t("scene_switcher.open"), |w| {
        let q = view(w).map(|v| v.query.clone()).unwrap_or_default();
        w.get_resource::<renzora_ui::DocumentTabState>()
            .map(|s| s.tabs.iter().filter(|t| matches(&q, &doc_label(t))).count())
            .unwrap_or(0)
    });
    // The one part of the panel that scrolls: a long list of open files must
    // not push the scenes or the footer out of reach.
    let open_list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() }).id();
    keyed_list(commands, open_list, open_snapshot);
    let open_scroll = renzora_ember::widgets::scroll_view_bar(commands, open_list);
    commands.entity(open_scroll).entry::<Node>().and_modify(|mut n| {
        n.flex_grow = 1.0;
        n.min_height = Val::Px(0.0);
    });
    commands.entity(right).add_children(&[right_head, open_scroll]);
    commands.entity(columns).add_children(&[left, right]);

    // Footer: the rarer actions.
    let footer = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                border: UiRect::top(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(rgb(renzora_ember::theme::border())),
        ))
        .id();
    let open_file = menu_item(commands, fonts, "folder-open", &renzora::lang::t("scene_switcher.open_file"), |w| {
        w.insert_resource(renzora::core::OpenSceneRequested)
    });
    // Setting the boot scene lives on the scene rows now (their star), so the
    // footer is just the one action that needs a file dialog.
    commands.entity(footer).add_child(open_file);

    commands.entity(body).add_children(&[search_row, columns, footer]);
    body
}

/// A column's title with a live count on the right.
fn column_header(commands: &mut Commands, fonts: &EmberFonts, title: &str, count: impl Fn(&Rx) -> usize + Send + Sync + 'static) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            padding: UiRect::new(Val::Px(8.0), Val::Px(8.0), Val::Px(4.0), Val::Px(4.0)),
            ..default()
        })
        .id();
    let mut font = ui_font(&fonts.ui, 11.0);
    font.weight = bevy::text::FontWeight::SEMIBOLD;
    let t = commands
        .spawn((Text::new(title.to_uppercase()), font, TextColor(rgb(text_muted())), Node { flex_grow: 1.0, ..default() }))
        .id();
    let n = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
    bind_text(commands, n, move |w| count(w).to_string());
    commands.entity(row).add_children(&[t, n]);
    row
}

fn scenes_snapshot(w: &Rx) -> KeyedSnapshot {
    let Some(v) = view(w) else {
        return KeyedSnapshot { items: Vec::new(), build: Box::new(|_, _, _| Entity::PLACEHOLDER) };
    };
    let rows: Vec<(PathBuf, String, bool)> = v
        .scenes
        .iter()
        .filter(|p| matches(&v.query, &file_stem(p)))
        .map(|p| (p.clone(), file_stem(p), v.boot.as_deref().is_some_and(|b| same_file(b, p))))
        .collect();
    if rows.is_empty() {
        return KeyedSnapshot {
            items: vec![(0, 0)],
            build: Box::new(|c, f, _| muted_note(c, f, &renzora::lang::t_or("scene_switcher.no_scenes", "No matching scenes"))),
        };
    }
    let items = rows
        .iter()
        .map(|(p, _, boot)| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            p.hash(&mut k);
            (k.finish(), *boot as u64)
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (path, name, boot) = rows[i].clone();
            let path_for_boot = path.clone();
            let kind = renzora_ui::DocTabKind::Scene;
            let row = menu_item_styled(c, f, kind.icon(), &name, kind.color(), text_primary(), move |w| {
                w.insert_resource(renzora::core::OpenScenePathRequested(path.clone()));
            });
            // The star at the row's right edge: amber on the boot scene, and
            // on any other scene a button (shown while the row is hovered)
            // that makes it the boot scene. It replaced a "Set as boot"
            // footer action that only ever applied to the open document.
            let spring = c.spawn((Node { flex_grow: 1.0, ..default() }, PASS_THROUGH)).id();
            let btn = c
                .spawn((
                    Node {
                        width: Val::Px(20.0),
                        height: Val::Px(20.0),
                        flex_shrink: 0.0,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    UiInteraction::default(),
                    BLOCK,
                    SceneBootBtn(path_for_boot.clone()),
                    renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                    renzora_ember::widgets::HoverTooltip::new(if boot {
                        renzora::lang::t_or("scene_switcher.boot_tip", "Boot scene")
                    } else {
                        renzora::lang::t_or("scene_switcher.make_boot_tip", "Make this the boot scene")
                    }),
                ))
                .id();
            bind_bg(c, btn, move |w| match w.get::<UiInteraction>(btn) {
                Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(renzora_ember::theme::tab_active()),
                _ => Color::NONE,
            });
            let star = icon_text(c, &f.phosphor, "star", if boot { warn_amber() } else { text_muted() }, 12.0);
            c.entity(star).insert(PASS_THROUGH);
            if !boot {
                bind_text_color(c, star, move |w| {
                    let hot = |e| matches!(w.get::<UiInteraction>(e), Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed));
                    if hot(row) || hot(btn) { rgb(text_muted()) } else { Color::NONE }
                });
            }
            c.entity(btn).add_child(star);
            c.entity(row).add_children(&[spring, btn]);
            row
        }),
    }
}

fn open_snapshot(w: &Rx) -> KeyedSnapshot {
    let empty = || KeyedSnapshot { items: Vec::new(), build: Box::new(|_, _, _| Entity::PLACEHOLDER) };
    let (Some(v), Some(s)) = (view(w), w.get_resource::<renzora_ui::DocumentTabState>()) else { return empty() };
    enum Item {
        Group(renzora_ui::DocTabKind, usize, bool),
        Doc(OpenDoc),
    }
    let mut items: Vec<Item> = Vec::new();
    for kind in KIND_ORDER {
        let docs: Vec<OpenDoc> = s
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, t)| t.kind == kind && matches(&v.query, &doc_label(t)))
            .map(|(i, t)| OpenDoc {
                id: t.id,
                label: doc_label(t),
                kind: t.kind,
                active: i == s.active_tab,
                modified: t.is_modified,
                closable: doc_closable(s, t.id),
            })
            .collect();
        if docs.is_empty() {
            continue;
        }
        // A search opens every group, so a match is never hidden in a folded one.
        let folded = v.query.is_empty() && v.collapsed.contains(&kind_key(kind));
        items.push(Item::Group(kind, docs.len(), folded));
        if !folded {
            items.extend(docs.into_iter().map(Item::Doc));
        }
    }
    if items.is_empty() {
        return KeyedSnapshot {
            items: vec![(0, 0)],
            build: Box::new(|c, f, _| muted_note(c, f, &renzora::lang::t_or("scene_switcher.no_docs", "No matching documents"))),
        };
    }
    let keys = items
        .iter()
        .map(|it| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            let mut h = std::collections::hash_map::DefaultHasher::new();
            match it {
                Item::Group(kind, n, folded) => {
                    (0u8, kind_key(*kind)).hash(&mut k);
                    (n, folded).hash(&mut h);
                }
                Item::Doc(d) => {
                    (1u8, d.id).hash(&mut k);
                    (&d.label, d.active, d.modified, d.closable).hash(&mut h);
                }
            }
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items: keys,
        build: Box::new(move |c, f, i| match &items[i] {
            Item::Group(kind, n, folded) => group_header(c, f, *kind, *n, *folded),
            Item::Doc(d) => open_doc_row(c, f, d),
        }),
    }
}

fn muted_note(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::axes(Val::Px(8.0), Val::Px(6.0)), ..default() },
        ))
        .id()
}

/// A foldable group header: caret, type colour, name, count.
fn group_header(commands: &mut Commands, fonts: &EmberFonts, kind: renzora_ui::DocTabKind, count: usize, folded: bool) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(24.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(7.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                margin: UiRect::top(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            SwitcherGroup(kind_key(kind)),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    bind_bg(commands, row, move |w| match w.get::<UiInteraction>(row) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(renzora_ember::theme::hover_bg()),
        _ => Color::NONE,
    });
    let caret = icon_text(commands, &fonts.phosphor, if folded { "caret-right" } else { "caret-down" }, text_muted(), 10.0);
    let swatch = commands
        .spawn((
            Node { width: Val::Px(8.0), height: Val::Px(8.0), border_radius: BorderRadius::all(Val::Px(2.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(rgb(kind.color())),
        ))
        .id();
    let title = commands
        .spawn((Text::new(kind_title(kind)), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted())), Node { flex_grow: 1.0, ..default() }))
        .id();
    let n = commands.spawn((Text::new(count.to_string()), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted())))).id();
    for e in [caret, swatch, title, n] {
        commands.entity(e).insert(PASS_THROUGH);
    }
    commands.entity(row).add_children(&[caret, swatch, title, n]);
    row
}

/// Mirror the search field into the view, and fold or unfold a group on a
/// header click.
pub(crate) fn switcher_interact(
    inputs: Query<&renzora_ember::widgets::EmberTextInput, With<SwitcherSearch>>,
    groups: Query<(&UiInteraction, &SwitcherGroup), Changed<UiInteraction>>,
    boots: Query<(&UiInteraction, &SceneBootBtn), Changed<UiInteraction>>,
    closes: Query<(&UiInteraction, &SwitcherCloseDoc), Changed<UiInteraction>>,
    project: Option<ResMut<renzora::CurrentProject>>,
    view: Option<ResMut<SwitcherView>>,
    mut commands: Commands,
) {
    let Some(mut view) = view else { return };
    // Queued, because closing needs the whole world (an unsaved document asks
    // first). The keyed list then drops the row on its own.
    for (_, close) in closes.iter().filter(|(i, _)| **i == UiInteraction::Pressed) {
        let id = close.0;
        commands.queue(move |w: &mut World| request_close_doc(w, id));
    }
    if let (Some(mut project), Some((_, btn))) = (project, boots.iter().find(|(i, _)| **i == UiInteraction::Pressed)) {
        let rel = project.make_asset_relative(&btn.0);
        project.config.main_scene = rel.clone();
        match project.save_config() {
            Ok(()) => {
                renzora::core::console_log::console_success("Scene", format!("Boot scene set to {rel}"));
                view.boot = Some(btn.0.clone());
            }
            Err(e) => renzora::core::console_log::console_error("Scene", format!("Failed to save project config: {e}")),
        }
    }
    if let Some(input) = inputs.iter().next() {
        let q = input.value.trim().to_lowercase();
        if view.query != q {
            view.query = q;
        }
    }
    for (i, g) in &groups {
        if *i == UiInteraction::Pressed {
            if let Some(pos) = view.collapsed.iter().position(|k| *k == g.0) {
                view.collapsed.remove(pos);
            } else {
                view.collapsed.push(g.0);
            }
        }
    }
}

/// A row of the **Open** group: the document's type icon in its type colour and
/// its name, an amber dot while it has unsaved changes, a check on the active
/// one, and an **x** that closes it.
///
/// The x `BLOCK`s the press, so closing a document does not also activate it
/// (UI nodes start as pass-through, and the row behind it is a menu action),
/// and it is not a [`MenuAction`] itself, so the menu stays open. It is drawn only while the row is
/// hovered: a column of x's down every row competes with the names, and the
/// close is for the row you are already pointing at. Closing goes through
/// [`request_close_doc`], so a document with unsaved changes still asks first.
fn open_doc_row(commands: &mut Commands, fonts: &EmberFonts, doc: &OpenDoc) -> Entity {
    let id = doc.id;
    let text = if doc.active { text_primary() } else { text_muted() };
    let row = menu_item_styled(
        commands,
        fonts,
        doc.kind.icon(),
        &doc.label,
        doc.kind.color(),
        text,
        move |w| activate_doc_tab(w, id),
    );
    // A long name ends in "…" rather than pushing the row's buttons out past
    // the column's edge, where the clip hid them: a name with no spaces
    // (`anim_flowing_water.material`) cannot wrap, so the active row lost its
    // save button and every long row its x. The label is the row's second
    // child, after the icon; the full name shows as a tooltip while shortened.
    commands.queue(move |w: &mut World| {
        let Some(label) = w.get::<Children>(row).and_then(|c| c.get(1).copied()) else { return };
        let full = w.get::<Text>(label).map(|t| t.0.clone()).unwrap_or_default();
        renzora_ember::widgets::ellipsize(&mut w.commands(), label, full, Some(row));
        w.flush();
    });
    let mut kids = Vec::new();
    // The active row says its saved state with its save button (below), so
    // the dot is for the others.
    if doc.modified && !doc.active {
        kids.push(
            commands
                .spawn((
                    Node {
                        width: Val::Px(6.0),
                        height: Val::Px(6.0),
                        flex_shrink: 0.0,
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(rgb(warn_amber())),
                    PASS_THROUGH,
                ))
                .id(),
        );
    }
    // Takes the slack, so the check and the x sit at the row's right edge.
    kids.push(commands.spawn((Node { flex_grow: 1.0, ..default() }, PASS_THROUGH)).id());
    // The active document gets a save button where a tick used to mark it:
    // the row's own icon colour and text already say which one is active, and
    // a tick read as "saved". Amber while there is something to save.
    if doc.active {
        let save_color = if doc.modified { warn_amber() } else { text_muted() };
        let save = small_row_button(commands, "scene-switcher-save", renzora::lang::t("common.save"));
        commands
            .entity(save)
            .insert(MenuAction(Box::new(|w: &mut World| crate::menu_command::MenuCommand::Save.run(w))));
        let disk = icon_text(commands, &fonts.phosphor, "floppy-disk", save_color, 13.0);
        commands.entity(disk).insert(PASS_THROUGH);
        commands.entity(save).add_child(disk);
        kids.push(save);
    }
    if doc.closable {
        let close = small_row_button(commands, "scene-switcher-close", renzora::lang::t("scene_switcher.close"));
        commands.entity(close).insert(SwitcherCloseDoc(id));
        // `icon_text`, not the deferred `glyph`: a deferred icon writes its
        // own colour a frame after spawning, over the hidden colour this
        // binding had already set, and the binding does not run again until
        // a hover changes. That left an x showing on every row.
        let x = icon_text(commands, &fonts.phosphor, "x", text_primary(), 12.0);
        commands.entity(x).insert((PASS_THROUGH, TextColor(Color::NONE)));
        // The row's hover covers the x too, since the x is inside the row.
        renzora_ember::reactive::tracked::bind_text_color(commands, x, move |w| {
            let hovered = |e| {
                matches!(
                    w.get::<UiInteraction>(e),
                    Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed)
                )
            };
            if hovered(row) || hovered(close) {
                rgb(text_primary())
            } else {
                Color::NONE
            }
        });
        commands.entity(close).add_child(x);
        kids.push(close);
    }
    commands.entity(row).add_children(&kids);
    row
}

/// An 18px icon button inside an Open row (save, close). It `BLOCK`s the
/// press, so pressing it does not also activate the row behind it; the caller
/// says what a press does.
fn small_row_button(commands: &mut Commands, name: &'static str, tooltip: String) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(18.0),
                height: Val::Px(18.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            BLOCK,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(tooltip),
            Name::new(name),
        ))
        .id();
    bind_bg(commands, btn, move |w| match w.get::<UiInteraction>(btn) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(renzora_ember::theme::tab_active()),
        _ => Color::NONE,
    });
    btn
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}
