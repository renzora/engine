//! The **Projects** page: create or open a project, and pick one out of the
//! recents.
//!
//! This is what the whole splash used to be, minus the social links and the
//! version line that now live in the status strip. It stays the default page
//! because opening a project is still the reason the window exists — the other
//! pages are things you do *while* you are here, not instead.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition, RepeatedGridTrack};
use bevy::window::SystemCursorIcon;

use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_with, keyed_list};
use renzora_ember::reactive::{react, KeyedSnapshot, Rx};
use renzora_ember::widgets::{
    bind_text_input, scroll_view, text_input, FileImageWanted, FileImages, HoverTooltip,
};

use crate::config::AppConfig;
// Desktop-only: the browser opens a project through a directory handle
// (`renzora_webfs`), not a path, so this has no caller on wasm. Creating a
// project moved to the New Project page, which owns the starter choice.
#[cfg(not(target_arch = "wasm32"))]
use crate::project::{create_project, open_project};

use super::style::*;

pub(crate) const SECTION_ID: &str = "projects";

/// Never fewer than two cards across. One column is a list wearing a grid's
/// clothes, and the window cannot get narrow enough to make one card readable
/// but two not.
const MIN_COLUMNS: u16 = 2;
/// Widest a card is allowed to get before the grid takes another column. A
/// snapshot is 16:9, so this is also what fixes the tile's height; much past it
/// and four recents fill the page.
const COLUMN_TARGET: f32 = 250.0;
/// Corner radius of a card, and of the snapshot's top two corners with it. Bevy
/// clips overflow to a rectangle, so the image has to round its own corners or
/// it squares off the card it sits in.
const CARD_RADIUS: f32 = 10.0;

#[derive(Component)]
struct NewProjectBtn;
#[derive(Component)]
struct NewFromTemplateBtn;
#[derive(Component)]
struct OpenProjectBtn;
/// A recent-project card — a spectral sheen travels around its border on hover.
#[derive(Component)]
struct RecentRow;
/// The grid the cards are laid out in. Marked so [`size_recent_grid`] can pick
/// a column count from the width the page actually got.
#[derive(Component)]
struct RecentGrid;
#[derive(Component, Clone)]
struct RecentOpen(PathBuf);
#[derive(Component, Clone)]
struct RecentRemove(PathBuf);

/// The recents search/filter text.
#[derive(Resource, Default)]
pub(crate) struct SplashFilter(String);

/// Project root → the cached snapshot of its **main scene**, as written by the
/// last save inside the editor.
///
/// Resolving one means reading that project's `project.toml` for `main_scene`,
/// which is a file read and a TOML parse per project. The card builder runs
/// inside a reactive list that rebuilds whenever anything it hashes changes, and
/// the empty-state binding re-runs the same query *every frame*, so doing the
/// resolve there would re-read ten `project.toml`s per frame. It is done once
/// per change to the recents list instead, by [`refresh_recent_thumbs`].
///
/// A path in here is not a promise the PNG exists: whether the file loads is
/// left to `FileImages`, which already remembers its failures, so a project that
/// has never been saved simply keeps the folder glyph.
#[derive(Resource, Default)]
pub(crate) struct RecentThumbs(HashMap<PathBuf, PathBuf>);

pub(crate) fn register(app: &mut App) {
    app.init_resource::<SplashFilter>();
    app.init_resource::<RecentThumbs>();
    super::sections::register_splash_section(
        app,
        super::sections::SplashSection::new("projects", "folders", "Projects", 0, build),
    );
}

/// Systems this page owns. Registered by `launcher::register` alongside the
/// rest, so the whole splash still has one system-set.
pub(crate) fn systems(app: &mut App) {
    app.add_systems(
        Update,
        (
            new_project_click,
            new_from_template_click,
            open_project_click,
            recent_open_click,
            recent_remove_click,
            animate_recent_borders,
            refresh_recent_thumbs,
            size_recent_grid,
        ),
    );
}

// ── Page ─────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let page = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                // Fills the host by growing into it, not by asking for 100% of
                // it — see `sections::build_page_host`.
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(16.0),
                padding: UiRect::all(Val::Px(PAGE_PAD)),
                ..default()
            },
            FocusPolicy::Pass,
            Name::new("splash-page-projects"),
        ))
        .id();

    let header = page_header(
        commands,
        fonts,
        &renzora::lang::t("splash.section.projects"),
        &renzora::lang::t("splash.projects_subtitle"),
    );

    // Actions + search share a line: they are the two ways to get to a project,
    // and stacking them pushed the recents list below the fold on a small window.
    let toolbar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let new = pill_button(commands, fonts, "plus", &renzora::lang::t("splash.new_project"), true);
    commands.entity(new).insert(NewProjectBtn);
    let template =
        pill_button(commands, fonts, "blueprint", &renzora::lang::t("splash.new_from_template"), false);
    commands.entity(template).insert(NewFromTemplateBtn);
    // Hidden when nothing registered the Templates page — a build without the
    // marketplace has no way to get a template, and a button that switches to a
    // page that does not exist would land on an empty host.
    bind_display(commands, template, |w| {
        w.get_resource::<super::SplashSections>()
            .is_some_and(|s| s.get(crate::TEMPLATES_SECTION_ID).is_some())
    });
    let open =
        pill_button(commands, fonts, "folder-open", &renzora::lang::t("splash.open_project"), false);
    commands.entity(open).insert(OpenProjectBtn);
    let search = build_search(commands, fonts);
    commands.entity(toolbar).add_children(&[new, template, open, search]);

    let heading = commands
        .spawn((
            Text::new(renzora::lang::t("splash.recent")),
            ui_font(&fonts.ui, 11.0),
            TextColor(c(104, 112, 132)),
            FocusPolicy::Pass,
        ))
        .id();

    // A grid of cards, not a list of rows: every recent project now carries a
    // picture of itself, and a picture is what you recognise a project by long
    // before you have read its path. Rows would have had to shrink the snapshot
    // to a strip to keep ten of them on screen.
    //
    // Equal `flex` tracks rather than fixed-width tiles that wrap, so the grid
    // always meets both edges of the page whatever column count the window
    // works out to — `size_recent_grid` picks the count.
    let list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                display: Display::Grid,
                grid_template_columns: vec![RepeatedGridTrack::flex(MIN_COLUMNS, 1.0)],
                align_content: AlignContent::FlexStart,
                row_gap: Val::Px(12.0),
                column_gap: Val::Px(12.0),
                padding: UiRect::right(Val::Px(6.0)),
                ..default()
            },
            FocusPolicy::Pass,
            RecentGrid,
        ))
        .id();
    keyed_list(commands, list, recents_snapshot);
    // `scroll_view` already returns a `flex_grow: 1` / `min_height: 0` viewport
    // that fills the column. Don't replace its `Node`: the scrollbar track is
    // positioned absolutely against it and the clip is what makes it scroll.
    let scroll = scroll_view(commands, list);

    let empty = commands
        .spawn((
            Text::new(renzora::lang::t("splash.no_recent")),
            ui_font(&fonts.ui, 12.0),
            TextColor(text_muted()),
            Node { margin: UiRect::top(Val::Px(6.0)), ..default() },
            FocusPolicy::Pass,
        ))
        .id();
    bind_display(commands, empty, |w| filtered_rows(w).is_empty());

    commands.entity(page).add_children(&[header, toolbar, heading, scroll, empty]);
    page
}

fn build_search(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                max_width: Val::Px(320.0),
                height: Val::Px(34.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::horizontal(Val::Px(11.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(ca(10, 12, 20, 225)),
            BorderColor::all(border_soft()),
            // The field itself is an ember widget with its own `Interaction`;
            // blocking on the frame around it is what keeps a click *into* the
            // field from reaching whatever is underneath mid-focus.
            FocusPolicy::Block,
        ))
        .id();
    let mag = icon_text(commands, &fonts.phosphor, "magnifying-glass", ICON_MUTED, 14.0);
    commands.entity(mag).insert(FocusPolicy::Pass);
    let search = text_input(commands, &fonts.ui, &renzora::lang::t("splash.search_projects"), "");
    commands.entity(search).insert(Node {
        flex_grow: 1.0,
        height: Val::Percent(100.0),
        align_items: AlignItems::Center,
        ..default()
    });
    commands
        .entity(search)
        .insert((BackgroundColor(Color::NONE), BorderColor::all(Color::NONE)));
    bind_text_input(commands, search, g_filter, s_filter);
    commands.entity(row).add_children(&[mag, search]);
    row
}

/// Icon + label action button (New / Open).
fn pill_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label_txt: &str,
    primary: bool,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                height: Val::Px(34.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(7.0),
                padding: UiRect::horizontal(Val::Px(14.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(if primary { accent() } else { btn_dark() }),
            Interaction::default(),
            FocusPolicy::Block,
            HoverCursor(SystemCursorIcon::Pointer),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        let hov = is_hovered(w, btn);
        if primary {
            if hov { accent_hover() } else { accent() }
        } else if hov {
            btn_dark_hover()
        } else {
            btn_dark()
        }
    });
    let ic = icon_text(
        commands,
        &fonts.phosphor,
        icon,
        if primary { (255, 255, 255) } else { ICON_TEXT },
        14.0,
    );
    commands.entity(ic).insert(FocusPolicy::Pass);
    let t = commands
        .spawn((
            Text::new(label_txt.to_string()),
            ui_font(&fonts.ui, 12.5),
            TextColor(if primary { white() } else { text() }),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(btn).add_children(&[ic, t]);
    btn
}

// ── Recents ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct RowData {
    name: String,
    path: PathBuf,
    path_display: String,
    exists: bool,
    /// The main scene's cached snapshot, from [`RecentThumbs`]. `None` until it
    /// has been resolved, and for a project whose folder is gone.
    thumb: Option<PathBuf>,
}

fn all_rows(world: &Rx) -> Vec<RowData> {
    let Some(cfg) = world.get_resource::<AppConfig>() else {
        return Vec::new();
    };
    let thumbs = world.get_resource::<RecentThumbs>();
    cfg.recent_projects
        .iter()
        .map(|p| {
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| renzora::lang::t("splash.unknown_project"));
            let path_display = p.to_string_lossy().to_string();
            #[cfg(not(target_arch = "wasm32"))]
            let exists = p.join("project.toml").exists();
            #[cfg(target_arch = "wasm32")]
            let exists = true;
            let thumb = thumbs.and_then(|t| t.0.get(p).cloned());
            RowData { name, path: p.clone(), path_display, exists, thumb }
        })
        .collect()
}

/// Resolve each recent project's main-scene snapshot, once per change to the
/// recents list. See [`RecentThumbs`] for why this is not done in the builder.
///
/// Entries the map already holds are kept: the resolve depends only on
/// `main_scene`, and re-reading every `project.toml` because one entry was
/// removed would put a file read on the frame a user clicked the ✕.
fn refresh_recent_thumbs(cfg: Res<AppConfig>, mut thumbs: ResMut<RecentThumbs>) {
    if !cfg.is_changed() {
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    for root in &cfg.recent_projects {
        if thumbs.0.contains_key(root) {
            continue;
        }
        if let Some(path) = main_scene_thumb(root) {
            thumbs.0.insert(root.clone(), path);
        }
    }
    // The browser never reaches this loop: a recent entry there is a folder
    // *name* the directory handle is looked up by, not a path, and the handle
    // needs the user's permission before anything under it can be read. Web
    // keeps the glyph.
    thumbs.0.retain(|root, _| cfg.recent_projects.contains(root));
}

/// Where `root`'s main scene keeps its snapshot, read out of its `project.toml`.
///
/// Parsed as a loose `toml::Value` rather than a `ProjectConfig` on purpose: the
/// only key that matters here is `main_scene`, and a strict parse would throw
/// away a perfectly good thumbnail because some *other* section of the file was
/// written by a newer editor.
#[cfg(not(target_arch = "wasm32"))]
fn main_scene_thumb(root: &Path) -> Option<PathBuf> {
    let src = std::fs::read_to_string(root.join("project.toml")).ok()?;
    let main_scene = src
        .parse::<toml::Value>()
        .ok()
        .and_then(|v| v.get("main_scene")?.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "scenes/main.bsn".to_string());
    Some(renzora::core::scene_thumbnail_path(root, &main_scene))
}

fn filtered_rows(world: &Rx) -> Vec<RowData> {
    let filter = world
        .get_resource::<SplashFilter>()
        .map(|f| f.0.to_lowercase())
        .unwrap_or_default();
    let filter = filter.trim();
    let rows = all_rows(world);
    if filter.is_empty() {
        return rows;
    }
    rows.into_iter()
        .filter(|r| {
            r.name.to_lowercase().contains(filter) || r.path_display.to_lowercase().contains(filter)
        })
        .collect()
}

fn recents_snapshot(world: &Rx) -> KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    let rows = filtered_rows(world);
    let items: Vec<(u64, u64)> = rows
        .iter()
        .map(|r| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            r.path.hash(&mut k);
            let key = k.finish();
            let mut h = std::collections::hash_map::DefaultHasher::new();
            r.name.hash(&mut h);
            r.exists.hash(&mut h);
            // Hashed so a card built before `refresh_recent_thumbs` had resolved
            // the snapshot is rebuilt once it has, rather than keeping the glyph
            // until something else happens to touch the list.
            r.thumb.hash(&mut h);
            (key, h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |commands, fonts, i| build_recent_card(commands, fonts, &rows[i])),
    }
}

/// One project in the grid: the main scene's snapshot over its name and path.
fn build_recent_card(commands: &mut Commands, fonts: &EmberFonts, row: &RowData) -> Entity {
    let card = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(CARD_RADIUS)),
                overflow: Overflow::clip(),
                // A grid item's automatic minimum size is its *content's*, so
                // without this the longest path in the recents decides how wide
                // every column is and a deep folder pushes the grid off the
                // right edge of the page. Explicit zero, then clip.
                min_width: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(ca(16, 18, 28, 220)),
            card_gradient(ca(22, 24, 36, 225), ca(11, 13, 21, 225)),
            BorderColor::all(border_soft()),
            Interaction::default(),
            // `cursor_over` — not `Interaction` — drives the card's hover sheen:
            // the ✕ blocks, so `Interaction` correctly drops to `None` the moment
            // the pointer crosses onto it, and keying the sheen off that would
            // make the card flatten out under your own cursor. Bevy fills
            // `RelativeCursorPosition` for every node containing the pointer
            // regardless of who captures the press, which is exactly the "is the
            // pointer anywhere over this card" signal the visual wants.
            RelativeCursorPosition::default(),
            FocusPolicy::Block,
            RecentRow,
        ))
        .id();
    if row.exists {
        commands
            .entity(card)
            .insert((RecentOpen(row.path.clone()), HoverCursor(SystemCursorIcon::Pointer)));
    }

    let thumb = build_card_thumb(commands, fonts, row);

    let info = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::axes(Val::Px(11.0), Val::Px(9.0)),
                min_width: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let name_txt = if row.exists {
        elide(&row.name, 28)
    } else {
        format!("{}  (missing)", elide(&row.name, 20))
    };
    let name = commands
        .spawn((
            Text::new(name_txt),
            ui_font(&fonts.ui, 13.5),
            TextColor(if row.exists { text() } else { text_muted() }),
            FocusPolicy::Pass,
        ))
        .id();
    let path = commands
        .spawn((
            Text::new(elide_path(&row.path_display, 34)),
            ui_font(&fonts.mono, 9.5),
            TextColor(text_muted()),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(info).add_children(&[name, path]);

    commands.entity(card).add_children(&[thumb, info]);
    card
}

/// The card's picture: the main scene's snapshot if one has ever been saved, a
/// folder glyph otherwise.
///
/// The glyph is always spawned and the image layered over it, rather than one
/// being swapped for the other, because the decode lands some frames after the
/// card is built — a card that started empty and filled in would make the whole
/// grid jump on the first paint.
fn build_card_thumb(commands: &mut Commands, fonts: &EmberFonts, row: &RowData) -> Entity {
    let frame = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                // A viewport snapshot is centre-cropped to a square when it is
                // written, so this shows the middle of one. 16:9 keeps the card
                // the shape of the window the scene was framed in.
                aspect_ratio: Some(16.0 / 9.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                border_radius: BorderRadius {
                    top_left: Val::Px(CARD_RADIUS - 1.5),
                    top_right: Val::Px(CARD_RADIUS - 1.5),
                    bottom_left: Val::Px(0.0),
                    bottom_right: Val::Px(0.0),
                },
                ..default()
            },
            BackgroundColor(ca(8, 9, 15, 220)),
            FocusPolicy::Pass,
        ))
        .id();

    let glyph = icon_text(
        commands,
        &fonts.phosphor,
        "folder",
        if row.exists { ICON_ACCENT } else { ICON_MUTED },
        30.0,
    );
    commands.entity(glyph).insert(FocusPolicy::Pass);
    commands.entity(frame).add_child(glyph);

    if let Some(thumb_path) = row.thumb.clone().filter(|_| row.exists) {
        let img = commands
            .spawn((
                ImageNode::default().with_mode(bevy::ui::widget::NodeImageMode::Stretch),
                Node {
                    position_type: PositionType::Absolute,
                    // A square image cropped to a 16:9 frame, not squashed into
                    // one: the snapshot on disk is always a centre-crop square
                    // (`renzora_scene::thumbnail`), and stretching it to 16:9
                    // would make every object in the shot 78% too wide.
                    //
                    // Full width, square, and pulled up by half the overhang so
                    // the middle band shows and the frame clips the rest. The
                    // offset is a percentage of the frame's *height*: with
                    // width W the image is W tall against a W*9/16 frame, so
                    // (H - W)/2 comes to -7/18 of H.
                    left: Val::Px(0.0),
                    top: Val::Percent(-100.0 * 7.0 / 18.0),
                    width: Val::Percent(100.0),
                    aspect_ratio: Some(1.0),
                    // Revealed by the binding below once the decode lands, so a
                    // blank `ImageNode` never flashes as a white rectangle.
                    display: Display::None,
                    ..default()
                },
                FocusPolicy::Pass,
                FileImageWanted(thumb_path.clone()),
            ))
            .id();
        bind_with(
            commands,
            img,
            move |w| w.get_resource::<FileImages>().and_then(|c| c.get(&thumb_path)),
            |w, e, handle: &Option<Handle<Image>>| {
                let Some(h) = handle else { return };
                if let Some(mut n) = w.get_mut::<ImageNode>(e) {
                    if n.image != *h {
                        n.image = h.clone();
                    }
                }
                if let Some(mut node) = w.get_mut::<Node>(e) {
                    node.display = Display::Flex;
                }
            },
        );
        commands.entity(frame).add_child(img);
    }

    let remove = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(6.0),
                right: Val::Px(6.0),
                width: Val::Px(24.0),
                height: Val::Px(24.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(ca(6, 7, 12, 170)),
            Interaction::default(),
            // Without this the press also reaches the card behind it, which opens
            // the project — the reported bug. It only *looked* correct for a
            // project whose folder had been deleted by hand, because a missing
            // project's card carries no `RecentOpen` for the press to land on.
            FocusPolicy::Block,
            RecentRemove(row.path.clone()),
            // The ✕ removes the entry from this list; it does not touch the
            // folder on disk. Say so — the reporter of #82 read it as "delete
            // project", which is a reasonable thing to read into a red ✕.
            HoverTooltip::new(renzora::lang::t("splash.remove_recent")),
            HoverCursor(SystemCursorIcon::Pointer),
        ))
        .id();
    let rc = remove;
    // A plate, not a tint: the ✕ sits over the snapshot now, and a 40-alpha wash
    // over an arbitrary frame of someone's game is not reliably visible.
    bind_bg(commands, remove, move |w| {
        if is_hovered(w, rc) { ca(220, 50, 50, 225) } else { ca(6, 7, 12, 170) }
    });
    let rx = icon_text(commands, &fonts.phosphor, "x", ICON_MUTED, 12.0);
    commands.entity(rx).insert(FocusPolicy::Pass);
    bind_text_color_on_hover(commands, rx, remove, white());
    commands.entity(remove).add_child(rx);
    commands.entity(frame).add_child(remove);

    frame
}

/// Pick the grid's column count from the width the page actually has, so the
/// cards keep roughly [`COLUMN_TARGET`] and the grid still meets both edges.
fn size_recent_grid(mut grid: Query<(&ComputedNode, &mut Node), With<RecentGrid>>) {
    for (cn, mut node) in &mut grid {
        let width = cn.size().x * cn.inverse_scale_factor();
        if width <= 0.0 {
            continue;
        }
        let n = ((width / COLUMN_TARGET).floor() as u16).clamp(MIN_COLUMNS, 5);
        let want = vec![RepeatedGridTrack::flex(n, 1.0)];
        if node.grid_template_columns != want {
            node.grid_template_columns = want;
        }
    }
}

fn bind_text_color_on_hover(commands: &mut Commands, text_e: Entity, btn: Entity, hover: Color) {
    react(commands, move |world: &mut World| {
        if world.get_entity(text_e).is_err() || world.get_entity(btn).is_err() {
            return false;
        }
        let col = if is_hovered(&Rx::new(&*world), btn) {
            hover
        } else {
            text_muted()
        };
        if let Some(mut c) = world.get_mut::<TextColor>(text_e) {
            c.0 = col;
        }
        true
    });
}

/// While a recent-project row is hovered, run a thin-film sheen around its border
/// and lift the card; restore the soft border otherwise.
///
/// Each edge is a different point on the spectrum and the whole set rotates, so the
/// colour appears to travel around the row the way it travels along a shaft in the
/// cinematic behind it. This replaced a glitch/colour-tearing effect that belonged
/// to the previous CRT-flavoured splash — nothing in this theme tears or blinks.
fn animate_recent_borders(
    time: Res<Time>,
    mut rows: Query<
        (&RelativeCursorPosition, &mut BorderColor, &mut bevy::ui::BackgroundGradient),
        With<RecentRow>,
    >,
) {
    let t = time.elapsed_secs();
    for (cursor, mut border, mut grad) in &mut rows {
        if !cursor.cursor_over {
            *border = BorderColor::all(border_soft());
            *grad = card_gradient(ca(22, 24, 36, 225), ca(11, 13, 21, 225));
            continue;
        }

        // ~9s for the sheen to travel all the way around — slow enough to read as a
        // material property rather than as an animation demanding attention.
        let hue = (t * 40.0).rem_euclid(360.0);
        let edge = |offset: f32| Color::hsl((hue + offset).rem_euclid(360.0), 0.72, 0.66);
        *border = BorderColor {
            top: edge(0.0),
            right: edge(28.0),
            bottom: edge(56.0),
            left: edge(84.0),
        };
        *grad = card_gradient(panel_hover(), ca(20, 22, 40, 250));
    }
}

// ── Field accessors ──────────────────────────────────────────────────────────

fn g_filter(w: &Rx) -> String {
    w.get_resource::<SplashFilter>().map(|f| f.0.clone()).unwrap_or_default()
}
fn s_filter(w: &mut World, v: String) {
    if let Some(mut f) = w.get_resource_mut::<SplashFilter>() {
        f.0 = v;
    }
}

// ── Interaction ──────────────────────────────────────────────────────────────

fn new_project_click(
    q: Query<&Interaction, (With<NewProjectBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(do_new_project);
    }
}

/// New from Template goes to the Templates page rather than a folder dialog.
///
/// A template is downloaded, so the choice is a browse, not a modal: it needs
/// thumbnails, descriptions and a search box, none of which fit in a file
/// dialog. Blank stays on the button next to it, so the fast path is still one
/// click and never waits on the network.
fn new_from_template_click(
    q: Query<&Interaction, (With<NewFromTemplateBtn>, Changed<Interaction>)>,
    mut active: ResMut<super::ActiveSection>,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        active.0 = crate::TEMPLATES_SECTION_ID.to_string();
    }
}

fn open_project_click(
    q: Query<&Interaction, (With<OpenProjectBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(do_open_project);
    }
}

fn recent_open_click(
    q: Query<(&Interaction, &RecentOpen), Changed<Interaction>>,
    mut commands: Commands,
) {
    for (interaction, open) in &q {
        if *interaction == Interaction::Pressed {
            let path = open.0.clone();
            commands.queue(move |world: &mut World| do_open_recent(world, &path));
        }
    }
}

fn recent_remove_click(
    q: Query<(&Interaction, &RecentRemove), Changed<Interaction>>,
    mut commands: Commands,
) {
    for (interaction, rm) in &q {
        if *interaction == Interaction::Pressed {
            let path = rm.0.clone();
            commands.queue(move |world: &mut World| {
                if let Some(mut cfg) = world.get_resource_mut::<AppConfig>() {
                    cfg.recent_projects.retain(|p| p != &path);
                    let _ = cfg.save();
                }
            });
        }
    }
}

// ── Project actions ──────────────────────────────────────────────────────────

fn do_open_recent(world: &mut World, path: &std::path::Path) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let toml = path.join("project.toml");
        match open_project(&toml) {
            Ok(p) => super::enter_project(world, p),
            Err(e) => error!("Failed to open project: {e}"),
        }
    }
    // Web: a recent entry is the folder's NAME, because the browser discloses
    // no path — so reopening goes through the directory handle stored in
    // IndexedDB when the project was first picked, and asks the user to
    // re-grant permission. Declining, or a folder that has since moved, fails
    // and leaves them to pick it again.
    #[cfg(target_arch = "wasm32")]
    {
        let _ = world;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        renzora_webfs::reopen_project(name);
    }
}

fn do_open_project(world: &mut World) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(file) = rfd::FileDialog::new()
            .set_title(renzora::lang::t("splash.open_project"))
            .add_filter(renzora::lang::t("splash.project_file"), &["toml"])
            .pick_file()
        {
            match open_project(&file) {
                Ok(p) => super::enter_project(world, p),
                Err(e) => error!("Failed to open project: {e}"),
            }
        }
    }
    // Web: the browser's directory picker reaches the same real folder the
    // desktop editor would open — `showDirectoryPicker` returns a handle with
    // read/write on whatever the user chooses, so one project works on both.
    //
    // The pick only starts here; `collect_web_project_pick` finishes it once
    // the browser resolves. `false` = the folder must already be a project.
    #[cfg(target_arch = "wasm32")]
    {
        let _ = world;
        renzora_webfs::pick_directory(false);
    }
}

/// New Project = pick (or create) a folder in the OS dialog; that folder becomes
/// the project root, named after the folder.
fn do_new_project(world: &mut World) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(folder) = rfd::FileDialog::new()
            .set_title(renzora::lang::t("splash.new_project_pick_folder"))
            .pick_folder()
        {
            let name = folder
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| renzora::lang::t("splash.new_project"));
            match create_project(&folder, &name) {
                Ok(p) => super::enter_project(world, p),
                Err(e) => error!("Failed to create project: {e}"),
            }
        }
    }
    // Web: the same picker, but `true` — the chosen folder is allowed to have
    // no project.toml, and `collect_web_project_pick` writes the skeleton into
    // it. Picking a folder that IS already a project opens it rather than
    // overwriting, which is the only safe reading of "New Project" landing on
    // someone's existing work.
    #[cfg(target_arch = "wasm32")]
    {
        let _ = world;
        renzora_webfs::pick_directory(true);
    }
}
