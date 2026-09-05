//! The **Changelog** page: what shipped, newest first, straight from the
//! project's GitHub releases.
//!
//! The list is a keyed list rather than a one-shot build, because the fetch is
//! in flight while the page is being looked at: opening the dashboard and
//! clicking Changelog immediately is the normal case, not a race to guard
//! against. The three states it can be in — fetching, failed, empty — are rows
//! of the same list, so there is one thing to keep consistent instead of three
//! nodes fighting over `bind_display`.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy::window::SystemCursorIcon;

use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_bg, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::widgets::{markdown_view, scroll_view};

use crate::releases::{ReleaseEntry, ReleaseFeed, RELEASES_URL};

use super::chrome::SplashUrl;
use super::style::*;

pub(crate) fn register(app: &mut App) {
    super::sections::register_splash_section(
        app,
        super::sections::SplashSection::new("changelog", "scroll", "Changelog", 80, build),
    );
}

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
            Name::new("splash-page-changelog"),
        ))
        .id();

    let top = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexEnd,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(12.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let header = page_header(
        commands,
        fonts,
        "Changelog",
        "Every release of the engine, newest first.",
    );
    let all = link_button(commands, fonts, "arrow-square-out", "All releases", RELEASES_URL);
    commands.entity(top).add_children(&[header, all]);

    let list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(12.0),
                padding: UiRect::right(Val::Px(6.0)),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    keyed_list(commands, list, releases_snapshot);
    // See `projects::build` — `scroll_view`'s own `Node` is load-bearing.
    let scroll = scroll_view(commands, list);

    commands.entity(page).add_children(&[top, scroll]);
    page
}

// ── Rows ─────────────────────────────────────────────────────────────────────

fn releases_snapshot(world: &Rx) -> KeyedSnapshot {
    let Some(feed) = world.get_resource::<ReleaseFeed>() else {
        return note_snapshot("The changelog is unavailable in this build.");
    };
    if !feed.loaded {
        return note_snapshot("Fetching releases…");
    }
    if let Some(err) = feed.error.clone() {
        return note_snapshot(&err);
    }
    if feed.entries.is_empty() {
        return note_snapshot("No releases published yet.");
    }

    use std::hash::{Hash, Hasher};
    let entries = feed.entries.clone();
    let items: Vec<(u64, u64)> = entries
        .iter()
        .map(|e| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            e.tag.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (&e.title, &e.body, &e.date).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| release_card(c, f, &entries[i])),
    }
}

/// A single centred line standing in for the whole list — loading, failed, or
/// empty. Keyed on its own text so swapping between the three replaces the row.
fn note_snapshot(text_value: &str) -> KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    let msg = text_value.to_string();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    msg.hash(&mut h);
    let key = h.finish();
    KeyedSnapshot {
        items: vec![(key, key)],
        build: Box::new(move |c, f, _| {
            c.spawn((
                Text::new(msg.clone()),
                ui_font(&f.ui, 12.0),
                TextColor(text_muted()),
                Node { margin: UiRect::top(Val::Px(6.0)), ..default() },
                FocusPolicy::Pass,
            ))
            .id()
        }),
    }
}

fn release_card(commands: &mut Commands, fonts: &EmberFonts, entry: &ReleaseEntry) -> Entity {
    let card = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(14.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(ca(16, 18, 28, 220)),
            BorderColor::all(border_soft()),
            FocusPolicy::Block,
            Name::new("splash-release-card"),
        ))
        .id();

    let head = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();

    let mut head_kids = vec![
        chip(commands, fonts, &entry.tag, accent(), ca(110, 150, 255, 34)),
    ];
    if is_this_build(&entry.tag) {
        head_kids.push(chip(commands, fonts, "This build", success(), ca(74, 200, 130, 34)));
    }
    if entry.prerelease {
        head_kids.push(chip(commands, fonts, "Prerelease", c(215, 175, 90), ca(215, 175, 90, 30)));
    }
    let title = commands
        .spawn((
            Text::new(entry.title.clone()),
            ui_font(&fonts.ui, 13.5),
            TextColor(text()),
            Node { flex_grow: 1.0, ..default() },
            FocusPolicy::Pass,
        ))
        .id();
    head_kids.push(title);
    if !entry.date.is_empty() {
        let date = commands
            .spawn((
                Text::new(entry.date.clone()),
                ui_font(&fonts.mono, 10.5),
                TextColor(text_muted()),
                FocusPolicy::Pass,
            ))
            .id();
        head_kids.push(date);
    }
    commands.entity(head).add_children(&head_kids);

    let mut kids = vec![head];
    // An untitled release with no notes is a tag someone pushed and nothing
    // else. Say that, rather than leaving a card that looks like it failed.
    let body = entry.body.trim();
    if body.is_empty() {
        kids.push(
            commands
                .spawn((
                    Text::new("No release notes.".to_string()),
                    ui_font(&fonts.ui, 11.5),
                    TextColor(text_muted()),
                    FocusPolicy::Pass,
                ))
                .id(),
        );
    } else {
        let (excerpt, truncated) = excerpt(body);
        let md = markdown_view(commands, fonts, &excerpt);
        commands.entity(md).insert(FocusPolicy::Pass);
        kids.push(md);
        if truncated {
            kids.push(
                commands
                    .spawn((
                        Text::new("…".to_string()),
                        ui_font(&fonts.ui, 11.5),
                        TextColor(text_muted()),
                        FocusPolicy::Pass,
                    ))
                    .id(),
            );
        }
    }
    kids.push(link_button(commands, fonts, "github-logo", "View on GitHub", &entry.url));

    commands.entity(card).add_children(&kids);
    card
}

/// Lines of a release body a card renders before it stops.
///
/// `markdown_view` spawns a node tree per block, and a generated release note can
/// be hundreds of lines of commit subjects. Twelve of those on one page is tens
/// of thousands of UI nodes for text nobody reads in a launcher — so the card
/// shows the top of the notes and **View on GitHub** carries the rest.
const EXCERPT_LINES: usize = 24;
/// Characters, for the same reason: a body can be long without being many lines.
const EXCERPT_CHARS: usize = 1200;

/// The part of `body` a card renders, and whether anything was left out.
///
/// Cuts on a line boundary so the markdown handed to the renderer is still whole
/// blocks — a cut mid-list or mid-fence renders as literal syntax.
fn excerpt(body: &str) -> (String, bool) {
    let mut out = String::new();
    // Tracked rather than re-measured with `out.chars().count()` each iteration,
    // which walked the whole accumulated string every line. The `+ 1` is the
    // newline pushed with each line, so the budget still counts what `out`
    // actually holds.
    let mut chars = 0usize;
    let mut truncated = false;
    for (taken, line) in body.lines().enumerate() {
        let len = line.chars().count();
        if taken >= EXCERPT_LINES || chars + len > EXCERPT_CHARS {
            truncated = true;
            break;
        }
        out.push_str(line);
        out.push('\n');
        chars += len + 1;
    }
    // A single line longer than the whole budget would otherwise leave the card
    // empty; show it rather than nothing.
    if out.trim().is_empty() {
        return (body.chars().take(EXCERPT_CHARS).collect(), body.chars().count() > EXCERPT_CHARS);
    }
    (out, truncated)
}

/// Is `tag` the release this binary was published as?
///
/// A dev build has no tag of its own, so it matches on the version it is *for* —
/// which is what someone reading the changelog from a checkout wants marked.
fn is_this_build(tag: &str) -> bool {
    match renzora::version::release_tag() {
        Some(t) => t == tag,
        None => tag == renzora::version::ENGINE_VERSION,
    }
}

fn chip(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    fg: Color,
    bg: Color,
) -> Entity {
    let chip = commands
        .spawn((
            Node {
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(7.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(bg),
            FocusPolicy::Pass,
        ))
        .id();
    let t = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.mono, 10.0),
            TextColor(fg),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(chip).add_child(t);
    chip
}

/// A small outline button that opens `url` in the browser.
pub(crate) fn link_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    url: &str,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                height: Val::Px(26.0),
                flex_shrink: 0.0,
                align_self: AlignSelf::FlexStart,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(btn_dark()),
            BorderColor::all(border_soft()),
            Interaction::default(),
            FocusPolicy::Block,
            SplashUrl(url.to_string()),
            HoverCursor(SystemCursorIcon::Pointer),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        if is_hovered(w, btn) { btn_dark_hover() } else { btn_dark() }
    });
    let ic = icon_text(commands, &fonts.phosphor, icon, ICON_MUTED, 12.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let t = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.5),
            TextColor(text()),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(btn).add_children(&[ic, t]);
    btn
}
