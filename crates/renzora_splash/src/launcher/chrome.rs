//! Window chrome: the title bar the borderless window drags by, the
//! minimize/maximize/close buttons, the eight resize zones around the edge, and
//! the click handler shared by every external link on the dashboard.
//!
//! The drag handle used to be the splash root — the whole background — because
//! the launcher had no chrome of its own to grab. The dashboard does, so
//! dragging is now the title bar's job alone; a press on a page's empty
//! background no longer picks the window up mid-scroll.

use bevy::math::CompassOctant;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy::window::SystemCursorIcon;

use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_bg, bind_text};
// The mark is a file beside the executable, which the browser build has no
// notion of — there it falls back to the glyph, and the on-disk image cache is
// never named. See `build_mark`.
#[cfg(not(target_arch = "wasm32"))]
use renzora_ember::reactive::tracked::bind_with;
#[cfg(not(target_arch = "wasm32"))]
use renzora_ember::widgets::{FileImageWanted, FileImages};
use renzora_ui::window_chrome::{WindowAction, WindowActionQueue};

use super::style::*;

pub(crate) const WEBSITE_URL: &str = "https://renzora.com";
pub(crate) const YOUTUBE_URL: &str = "https://youtube.com/@renzoragame";
pub(crate) const DISCORD_URL: &str = "https://discord.gg/9UHUGUyDJv";
pub(crate) const GITHUB_URL: &str = "https://github.com/renzora/engine";

// The ABI hash and its link to the release commit that froze it used to live in
// the status strip. Both are gone: the canonical record is `releases.json` at
// the repo root, the About dialog still reports the build, and the strip was
// spending its whole left side on a hex string that reads as an error code to
// everyone who is not writing a prebuilt plugin.

#[derive(Component)]
pub(crate) struct SplashDragHandle;

#[derive(Component, Clone, Copy)]
pub(crate) enum WinBtn {
    Min,
    Max,
    Close,
}

#[derive(Component)]
pub(crate) struct SplashWinBtn(pub WinBtn);

#[derive(Component)]
pub(crate) struct SplashResizeZone(pub CompassOctant);

/// Anything that opens `url` in the system browser when pressed.
#[derive(Component, Clone)]
pub(crate) struct SplashUrl(pub String);

// ── Title bar ────────────────────────────────────────────────────────────────

/// The strip across the top: the product mark on the left, the window controls
/// on the right, and the whole thing a drag handle in between.
pub(crate) fn build_title_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(TITLEBAR_H),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::left(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(rail_bg()),
            // Blocks so the press starts a window drag here rather than falling
            // through to whatever is behind the bar.
            FocusPolicy::Block,
            Interaction::default(),
            SplashDragHandle,
            Name::new("splash-title-bar"),
        ))
        .id();

    let brand = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let mark = build_mark(commands, fonts);
    let name = commands
        .spawn((
            Text::new("Renzora Engine".to_string()),
            ui_font(&fonts.ui, 12.5),
            TextColor(text()),
            FocusPolicy::Pass,
        ))
        .id();
    // The version reads as part of the product's name, not as a diagnostic — it
    // is the first thing anyone is asked for in a bug report, and in the status
    // strip it sat among frame rate and an ABI hash, which is where numbers go
    // to be ignored.
    let dot = commands
        .spawn((
            Text::new("·".to_string()),
            ui_font(&fonts.ui, 12.0),
            TextColor(text_muted()),
            FocusPolicy::Pass,
        ))
        .id();
    let version = commands
        .spawn((
            Text::new(renzora::version::display()),
            ui_font(&fonts.mono, 11.0),
            TextColor(text_muted()),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(brand).add_children(&[mark, name, dot, version]);

    let controls = build_window_controls(commands, fonts);
    commands.entity(bar).add_children(&[brand, controls]);
    bar
}

/// The Renzora mark in the title bar: the real icon, with a glyph standing in
/// where the file is not there.
///
/// Loaded through ember's on-disk image cache rather than the `AssetServer`,
/// because the asset root is the *project*'s and the splash exists precisely
/// when there is no project. The icon is staged beside the executable as
/// `resources/icon.png` (see `xtask`'s staging step, which puts it there for the
/// exporter to fall back on), so it is present in every staged and downloaded
/// build — and absent from a bare `cargo run`, which is what the glyph is for.
fn build_mark(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    const MARK: f32 = 20.0;

    let frame = commands
        .spawn((
            Node {
                width: Val::Px(MARK),
                height: Val::Px(MARK),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            FocusPolicy::Pass,
            Name::new("splash-brand-mark"),
        ))
        .id();

    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = brand_icon_path() {
        let img = commands
            .spawn((
                ImageNode::default(),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    // Revealed by the binding below once the decode lands, so a
                    // blank `ImageNode` never flashes as a white square.
                    display: Display::None,
                    ..default()
                },
                FocusPolicy::Pass,
                FileImageWanted(path.clone()),
            ))
            .id();
        bind_with(
            commands,
            img,
            move |w| w.get_resource::<FileImages>().and_then(|c| c.get(&path)),
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
        return frame;
    }

    let glyph = icon_text(commands, &fonts.phosphor, "cube", ICON_ACCENT, 15.0);
    commands.entity(glyph).insert(FocusPolicy::Pass);
    commands.entity(frame).add_child(glyph);
    frame
}

/// Absolute path of the icon staged beside the executable, if this build has one.
#[cfg(not(target_arch = "wasm32"))]
fn brand_icon_path() -> Option<std::path::PathBuf> {
    let path = std::env::current_exe()
        .ok()?
        .parent()?
        .join("resources")
        .join("icon.png");
    path.is_file().then_some(path)
}

fn build_window_controls(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn((
            Node {
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            FocusPolicy::Pass,
            Name::new("splash-window-controls"),
        ))
        .id();
    // Same as the editor shell's title bar: a browser tab has no OS window to
    // minimize, maximize or close, so the controls are left off rather than
    // rendered as three buttons that do nothing.
    #[cfg(not(target_arch = "wasm32"))]
    {
        let min = win_button(commands, fonts, WinBtn::Min, "minus", false);
        let max = win_button(commands, fonts, WinBtn::Max, "square", false);
        let close = win_button(commands, fonts, WinBtn::Close, "x", true);
        commands.entity(row).add_children(&[min, max, close]);
    }
    #[cfg(target_arch = "wasm32")]
    let _ = fonts;
    row
}

fn win_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    kind: WinBtn,
    icon: &str,
    is_close: bool,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(44.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            FocusPolicy::Block,
            SplashWinBtn(kind),
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("splash-win-btn"),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        if is_hovered(w, btn) {
            if is_close {
                c(232, 17, 35)
            } else {
                ca(255, 255, 255, 34)
            }
        } else {
            Color::NONE
        }
    });
    let glyph = icon_text(commands, &fonts.phosphor, icon, ICON_TEXT, 14.0);
    commands.entity(glyph).insert(FocusPolicy::Pass);
    if matches!(kind, WinBtn::Max) {
        let square = renzora_ember::font::icon_glyph("square").unwrap_or('\u{E4C6}');
        let restore = renzora_ember::font::icon_glyph("arrows-in-simple").unwrap_or('\u{E4C6}');
        bind_text(commands, glyph, move |w| {
            let maxed = w
                .get_resource::<WindowActionQueue>()
                .map(|q| q.maximized)
                .unwrap_or(false);
            (if maxed { restore } else { square }).to_string()
        });
    }
    commands.entity(btn).add_child(glyph);
    btn
}

// ── Resize zones ─────────────────────────────────────────────────────────────

pub(crate) fn build_resize_zones(commands: &mut Commands, root: Entity) {
    let t = Val::Px(8.0);
    let cz = Val::Px(16.0);
    let edges: [(CompassOctant, Edge); 8] = [
        (CompassOctant::North, Edge::horiz_top(t)),
        (CompassOctant::South, Edge::horiz_bottom(t)),
        (CompassOctant::West, Edge::vert_left(t)),
        (CompassOctant::East, Edge::vert_right(t)),
        (CompassOctant::NorthWest, Edge::corner(true, true, cz)),
        (CompassOctant::NorthEast, Edge::corner(false, true, cz)),
        (CompassOctant::SouthWest, Edge::corner(true, false, cz)),
        (CompassOctant::SouthEast, Edge::corner(false, false, cz)),
    ];
    for (octant, e) in edges {
        let cursor = resize_cursor(octant);
        let zone = commands
            .spawn((
                e.into_node(),
                BackgroundColor(Color::NONE),
                GlobalZIndex(560),
                Interaction::default(),
                // Or a drag from an edge starts an OS *move* as well as a resize.
                FocusPolicy::Block,
                SplashResizeZone(octant),
                HoverCursor(cursor),
                Name::new("splash-resize"),
            ))
            .id();
        commands.entity(root).add_child(zone);
    }
}

struct Edge {
    left: Val,
    right: Val,
    top: Val,
    bottom: Val,
    width: Val,
    height: Val,
}
impl Edge {
    fn horiz_top(t: Val) -> Self {
        Self { left: Val::Px(16.0), right: Val::Px(16.0), top: Val::Px(0.0), bottom: Val::Auto, width: Val::Auto, height: t }
    }
    fn horiz_bottom(t: Val) -> Self {
        Self { left: Val::Px(16.0), right: Val::Px(16.0), top: Val::Auto, bottom: Val::Px(0.0), width: Val::Auto, height: t }
    }
    fn vert_left(t: Val) -> Self {
        Self { left: Val::Px(0.0), right: Val::Auto, top: Val::Px(16.0), bottom: Val::Px(16.0), width: t, height: Val::Auto }
    }
    fn vert_right(t: Val) -> Self {
        Self { left: Val::Auto, right: Val::Px(0.0), top: Val::Px(16.0), bottom: Val::Px(16.0), width: t, height: Val::Auto }
    }
    fn corner(left_side: bool, top_side: bool, cz: Val) -> Self {
        Self {
            left: if left_side { Val::Px(0.0) } else { Val::Auto },
            right: if left_side { Val::Auto } else { Val::Px(0.0) },
            top: if top_side { Val::Px(0.0) } else { Val::Auto },
            bottom: if top_side { Val::Auto } else { Val::Px(0.0) },
            width: cz,
            height: cz,
        }
    }
    fn into_node(self) -> Node {
        Node {
            position_type: PositionType::Absolute,
            left: self.left,
            right: self.right,
            top: self.top,
            bottom: self.bottom,
            width: self.width,
            height: self.height,
            ..default()
        }
    }
}

fn resize_cursor(octant: CompassOctant) -> SystemCursorIcon {
    match octant {
        CompassOctant::North | CompassOctant::South => SystemCursorIcon::NsResize,
        CompassOctant::East | CompassOctant::West => SystemCursorIcon::EwResize,
        CompassOctant::NorthWest | CompassOctant::SouthEast => SystemCursorIcon::NwseResize,
        CompassOctant::NorthEast | CompassOctant::SouthWest => SystemCursorIcon::NeswResize,
    }
}

// ── Interaction systems ──────────────────────────────────────────────────────

pub(crate) fn window_btn_click(
    q: Query<(&Interaction, &SplashWinBtn), Changed<Interaction>>,
    queue: Option<ResMut<WindowActionQueue>>,
) {
    let Some(mut queue) = queue else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        queue.push(match btn.0 {
            WinBtn::Min => WindowAction::Minimize,
            WinBtn::Max => WindowAction::ToggleMaximize,
            WinBtn::Close => WindowAction::Close,
        });
    }
}

pub(crate) fn drag_handle(
    q: Query<&Interaction, (With<SplashDragHandle>, Changed<Interaction>)>,
    queue: Option<ResMut<WindowActionQueue>>,
) {
    let Some(mut queue) = queue else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        queue.push(WindowAction::StartDrag);
    }
}

pub(crate) fn resize_zone_click(
    q: Query<(&Interaction, &SplashResizeZone), Changed<Interaction>>,
    queue: Option<ResMut<WindowActionQueue>>,
) {
    let Some(mut queue) = queue else { return };
    for (interaction, zone) in &q {
        if *interaction == Interaction::Pressed {
            queue.push(WindowAction::StartResize(zone.0));
        }
    }
}

pub(crate) fn url_click(q: Query<(&Interaction, &SplashUrl), Changed<Interaction>>) {
    for (interaction, url) in &q {
        if *interaction == Interaction::Pressed {
            open_url(&url.0);
        }
    }
}

pub(crate) fn open_url(url: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(url).spawn();
        #[cfg(all(unix, not(target_os = "macos")))]
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = url;
    }
}
