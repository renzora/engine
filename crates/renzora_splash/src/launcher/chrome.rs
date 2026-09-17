//! The overlay panel's header: the product mark, the dismiss button, and the
//! click handler shared by every external link on the dashboard.
//!
//! It was window chrome, and the module is named for what is left of it. The
//! splash used to be an undecorated window of its own, so it drew its own title
//! bar to drag by, its own minimize/maximize/close buttons and eight resize
//! zones around the edge. The dashboard is a panel inside the editor's window
//! now: the editor owns the window and draws all of that, and what remains here
//! is a header with a ✕ in it.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy::window::SystemCursorIcon;

use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::bind_bg;
// The mark is a file beside the executable, which the browser build has no
// notion of: there it falls back to the glyph, and the on-disk image cache is
// never named. See `build_mark`.
#[cfg(not(target_arch = "wasm32"))]
use renzora_ember::reactive::tracked::bind_with;
#[cfg(not(target_arch = "wasm32"))]
use renzora_ember::widgets::{FileImageWanted, FileImages};

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

// The drag handle, the three window buttons and the eight resize zones are gone
// with the window they operated: the splash was an undecorated window of its
// own, so it had to draw and drive its own chrome. It is a panel inside the
// editor's window now, and the editor's title bar owns all of that.

/// Anything that opens `url` in the system browser when pressed.
#[derive(Component, Clone)]
pub(crate) struct SplashUrl(pub String);

// ── Title bar ────────────────────────────────────────────────────────────────

/// The strip across the top of the overlay panel: the product mark on the left,
/// the dismiss button on the right.
///
/// It was the window's title bar, carrying the minimize/maximize/close controls
/// and acting as the drag handle for the whole undecorated splash window. None
/// of that is the panel's business now: the editor owns the window and draws its
/// own title bar, and this one closes a panel rather than an application.
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
            // Blocks so a press on the header is not also a press on the scrim,
            // which would dismiss the overlay.
            FocusPolicy::Block,
            Interaction::default(),
            Name::new("splash-panel-header"),
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

    // The same side on every platform, unlike the window controls this replaced:
    // a panel's dismiss button is not an OS window control, so the macOS
    // convention of leading with it does not apply.
    let close = build_close_button(commands, fonts);
    commands.entity(bar).add_children(&[brand, close]);
    bar
}

/// The ✕ that dismisses the overlay.
///
/// A second way out, for anyone who does not discover that pressing the dimmed
/// editor behind the panel works. Escape is the third.
fn build_close_button(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
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
            super::SplashClose,
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("splash-panel-close"),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        if is_hovered(w, btn) {
            ca(255, 255, 255, 34)
        } else {
            Color::NONE
        }
    });
    let glyph = icon_text(commands, &fonts.phosphor, "x", ICON_TEXT, 14.0);
    commands.entity(glyph).insert(FocusPolicy::Pass);
    commands.entity(btn).add_child(glyph);
    btn
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

// ── Interaction systems ──────────────────────────────────────────────────────

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
