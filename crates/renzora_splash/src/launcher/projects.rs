//! The **Projects** page: create or open a project, and pick one out of the
//! recents.
//!
//! This is what the whole splash used to be, minus the social links and the
//! version line that now live in the status strip. It stays the default page
//! because opening a project is still the reason the window exists — the other
//! pages are things you do *while* you are here, not instead.

use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};
use bevy::window::SystemCursorIcon;

use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_bg, bind_display, keyed_list};
use renzora_ember::reactive::{react, KeyedSnapshot, Rx};
use renzora_ember::widgets::{bind_text_input, scroll_view, text_input, HoverTooltip};

use crate::config::AppConfig;
// Both are desktop-only paths: the browser opens a project through a directory
// handle (`renzora_webfs`), not a path, so neither has a caller on wasm.
#[cfg(not(target_arch = "wasm32"))]
use crate::project::{create_project, open_project};

use super::style::*;

pub(crate) const SECTION_ID: &str = "projects";

#[derive(Component)]
struct NewProjectBtn;
#[derive(Component)]
struct OpenProjectBtn;
/// A recent-project row — a spectral sheen travels around its border on hover.
#[derive(Component)]
struct RecentRow;
#[derive(Component, Clone)]
struct RecentOpen(std::path::PathBuf);
#[derive(Component, Clone)]
struct RecentRemove(std::path::PathBuf);

/// The recents search/filter text.
#[derive(Resource, Default)]
pub(crate) struct SplashFilter(String);

pub(crate) fn register(app: &mut App) {
    app.init_resource::<SplashFilter>();
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
            open_project_click,
            recent_open_click,
            recent_remove_click,
            animate_recent_borders,
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
        "Projects",
        "Open one you were working on, or start something new.",
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
    let new = pill_button(commands, fonts, "plus", "New Project", true);
    commands.entity(new).insert(NewProjectBtn);
    let open = pill_button(commands, fonts, "folder-open", "Open Project", false);
    commands.entity(open).insert(OpenProjectBtn);
    let search = build_search(commands, fonts);
    commands.entity(toolbar).add_children(&[new, open, search]);

    let heading = commands
        .spawn((
            Text::new("Recent Projects".to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(c(104, 112, 132)),
            FocusPolicy::Pass,
        ))
        .id();

    // The list fills whatever height is left; the scroll view is what stops a
    // long recents list from pushing the page off the bottom of the window.
    let list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::right(Val::Px(6.0)),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    keyed_list(commands, list, recents_snapshot);
    // `scroll_view` already returns a `flex_grow: 1` / `min_height: 0` viewport
    // that fills the column. Don't replace its `Node`: the scrollbar track is
    // positioned absolutely against it and the clip is what makes it scroll.
    let scroll = scroll_view(commands, list);

    let empty = commands
        .spawn((
            Text::new("No recent projects yet.".to_string()),
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
    let search = text_input(commands, &fonts.ui, "Search projects…", "");
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
    path: std::path::PathBuf,
    path_display: String,
    exists: bool,
}

fn all_rows(world: &Rx) -> Vec<RowData> {
    let Some(cfg) = world.get_resource::<AppConfig>() else {
        return Vec::new();
    };
    cfg.recent_projects
        .iter()
        .map(|p| {
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown Project")
                .to_string();
            let path_display = p.to_string_lossy().to_string();
            #[cfg(not(target_arch = "wasm32"))]
            let exists = p.join("project.toml").exists();
            #[cfg(target_arch = "wasm32")]
            let exists = true;
            RowData { name, path: p.clone(), path_display, exists }
        })
        .collect()
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
            (key, h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |commands, fonts, i| build_recent_row(commands, fonts, &rows[i])),
    }
}

fn build_recent_row(commands: &mut Commands, fonts: &EmberFonts, row: &RowData) -> Entity {
    let container = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(56.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(13.0),
                padding: UiRect::horizontal(Val::Px(14.0)),
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(ca(16, 18, 28, 220)),
            card_gradient(ca(22, 24, 36, 225), ca(11, 13, 21, 225)),
            BorderColor::all(border_soft()),
            Interaction::default(),
            // `cursor_over` — not `Interaction` — drives the row's hover sheen:
            // the ✕ blocks, so `Interaction` correctly drops to `None` the moment
            // the pointer crosses onto it, and keying the sheen off that would
            // make the card flatten out under your own cursor. Bevy fills
            // `RelativeCursorPosition` for every node containing the pointer
            // regardless of who captures the press, which is exactly the "is the
            // pointer anywhere over this row" signal the visual wants.
            RelativeCursorPosition::default(),
            FocusPolicy::Block,
            RecentRow,
        ))
        .id();
    if row.exists {
        commands
            .entity(container)
            .insert((RecentOpen(row.path.clone()), HoverCursor(SystemCursorIcon::Pointer)));
    }

    let icon = icon_text(
        commands,
        &fonts.phosphor,
        "folder",
        if row.exists { ICON_ACCENT } else { ICON_MUTED },
        21.0,
    );
    commands.entity(icon).insert(FocusPolicy::Pass);

    let info = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let name_txt = if row.exists {
        row.name.clone()
    } else {
        format!("{}  (missing)", row.name)
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
            Text::new(elide_path(&row.path_display, 70)),
            ui_font(&fonts.mono, 10.0),
            TextColor(text_muted()),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(info).add_children(&[name, path]);

    let remove = commands
        .spawn((
            Node {
                width: Val::Px(26.0),
                height: Val::Px(26.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            // Without this the press also reaches the row behind it, which opens
            // the project — the reported bug. It only *looked* correct for a
            // project whose folder had been deleted by hand, because a missing
            // project's row carries no `RecentOpen` for the press to land on.
            FocusPolicy::Block,
            RecentRemove(row.path.clone()),
            // The ✕ removes the entry from this list; it does not touch the
            // folder on disk. Say so — the reporter of #82 read it as "delete
            // project", which is a reasonable thing to read into a red ✕.
            HoverTooltip::new("Remove from recent projects"),
            HoverCursor(SystemCursorIcon::Pointer),
        ))
        .id();
    let rc = remove;
    bind_bg(commands, remove, move |w| {
        if is_hovered(w, rc) { ca(239, 68, 68, 40) } else { Color::NONE }
    });
    let rx = icon_text(commands, &fonts.phosphor, "x", ICON_MUTED, 13.0);
    commands.entity(rx).insert(FocusPolicy::Pass);
    bind_text_color_on_hover(commands, rx, remove);
    commands.entity(remove).add_child(rx);

    commands.entity(container).add_children(&[icon, info, remove]);
    container
}

fn bind_text_color_on_hover(commands: &mut Commands, text_e: Entity, btn: Entity) {
    react(commands, move |world: &mut World| {
        if world.get_entity(text_e).is_err() || world.get_entity(btn).is_err() {
            return false;
        }
        let col = if is_hovered(&Rx::new(&*world), btn) {
            error_color()
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
            .set_title("Open Project")
            .add_filter("Project File", &["toml"])
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
            .set_title("New Project — choose a folder")
            .pick_folder()
        {
            let name = folder
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "New Project".to_string());
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
