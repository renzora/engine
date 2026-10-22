//! Bevy-native (ember) console panel.
//!
//! One toolbar, one flat log, a details pane and a command bar:
//!
//! - **Toolbar:** a pill per level (Info, Success, Warn, Error) with its line
//!   count, which hides or shows that level; the message filter; a category
//!   menu; the shown/total count; Copy, Clear and Follow.
//! - **Log:** one line per entry, flat. No stripes and no tinted rows: the
//!   level dot and the message colour already say which lines are warnings and
//!   errors, and a banded list read as a grid competing with the text. A line
//!   is a fixed single row, so a long message is cut at the panel edge rather
//!   than wrapping into a wall; its whole text is one click away.
//! - **Details:** clicking a line opens it on the right: the full message, a
//!   Copy button, the entity ids it mentions as chips that select the entity,
//!   its frame and how often it repeated. Clicking the line again, or the x,
//!   closes it.
//! - **Command bar:** typing `/` lists the commands that match, above the bar.
//!   Up and Down choose, Tab or a click completes, Escape dismisses.
//!
//! The category chips row this replaces became the category menu: a row of
//! chips grew with every subsystem that ever logged, and on a short docked
//! panel it took a line from the log to say something you change rarely.
//!
//! The shell is built once; the log, the details and the suggestions are
//! `keyed_list`s, so only changed rows are touched.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text, bind_with, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::{
    accent, border, close_red, faint_bg, header_bg, hover_bg, mix, play_green, popup_bg, rgb,
    text_muted, text_primary, warn_amber,
};
use renzora_ember::widgets::{
    screen_menu_est_height, screen_menu_under, menu_item_styled, menu_sep, scroll_view,
    scroll_view_pinned, search_input, text_input, trigger_rect, EmberScroll, EmberTextInput,
    HoverTooltip,
};
use renzora_ember::UiInteraction;

use crate::state::{ConsoleState, LogEntry, LogLevel};

const PANEL_ID: &str = "console";
/// Max log rows kept in the DOM at once (older ones are trimmed from the front).
const MAX_ROWS: usize = 400;
const ROW_H: f32 = 22.0;
const DETAIL_W: f32 = 380.0;

const LEVELS: [LogLevel; 4] = [LogLevel::Info, LogLevel::Success, LogLevel::Warning, LogLevel::Error];

fn level_color(level: LogLevel) -> (u8, u8, u8) {
    match level {
        LogLevel::Info => accent(),
        LogLevel::Success => play_green(),
        LogLevel::Warning => warn_amber(),
        LogLevel::Error => close_red(),
    }
}

fn level_label(level: LogLevel) -> String {
    match level {
        LogLevel::Info => renzora::lang::t_or("console.filter.info", "Info"),
        LogLevel::Success => renzora::lang::t_or("console.filter.success", "Success"),
        LogLevel::Warning => renzora::lang::t_or("console.filter.warn", "Warn"),
        LogLevel::Error => renzora::lang::t_or("console.filter.error", "Error"),
    }
}

fn level_idx(level: LogLevel) -> u8 {
    match level {
        LogLevel::Info => 0,
        LogLevel::Success => 1,
        LogLevel::Warning => 2,
        LogLevel::Error => 3,
    }
}

fn level_shown(s: &ConsoleState, level: LogLevel) -> bool {
    match level {
        LogLevel::Info => s.show_info,
        LogLevel::Success => s.show_success,
        LogLevel::Warning => s.show_warnings,
        LogLevel::Error => s.show_errors,
    }
}

/// A message's text colour: warnings and errors are told apart by their text,
/// since the rows themselves are flat.
fn message_color(level: LogLevel) -> Color {
    match level {
        LogLevel::Warning => mix(text_primary(), warn_amber(), 0.45),
        LogLevel::Error => mix(text_primary(), close_red(), 0.45),
        _ => rgb(text_primary()),
    }
}

/// A category's colour, picked from a fixed set by its name, so a category
/// keeps its colour across sessions without anyone assigning one.
fn category_color(cat: &str) -> (u8, u8, u8) {
    const HUES: [(u8, u8, u8); 8] = [
        (115, 191, 242),
        (200, 140, 240),
        (60, 200, 160),
        (224, 160, 64),
        (230, 120, 90),
        (150, 180, 255),
        (200, 200, 120),
        (240, 130, 180),
    ];
    let mut h = std::collections::hash_map::DefaultHasher::new();
    cat.hash(&mut h);
    HUES[(h.finish() % HUES.len() as u64) as usize]
}

/// Elapsed seconds as `MM:SS.s`, or `H:MM:SS` past an hour.
pub(crate) fn fmt_time(secs: f64) -> String {
    let secs = secs.max(0.0);
    let whole = secs as u64;
    if whole >= 3600 {
        format!("{}:{:02}:{:02}", whole / 3600, (whole / 60) % 60, whole % 60)
    } else {
        format!("{:02}:{:04.1}", whole / 60, secs % 60.0)
    }
}

/// Every entity id (`1874v0`, as `Entity`'s `Debug` prints) in a message, in
/// order, without repeats.
pub(crate) fn entity_ids(message: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in message.split(|c: char| !c.is_ascii_alphanumeric()) {
        let Some((index, generation)) = word.split_once('v') else { continue };
        let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        if digits(index) && digits(generation) && !out.iter().any(|w| w == word) {
            out.push(word.to_string());
        }
    }
    out
}

/// Does this entry pass the current filters? (Mirrors `ConsoleState::filtered_entries`.)
fn passes(s: &ConsoleState, e: &LogEntry) -> bool {
    if !level_shown(s, e.level) {
        return false;
    }
    if !e.category.is_empty() && s.hidden_categories.contains(&e.category) {
        return false;
    }
    if !s.category_filter.is_empty()
        && !e.category.to_lowercase().contains(&s.category_filter.to_lowercase())
    {
        return false;
    }
    if !s.search_filter.is_empty()
        && !e.message.to_lowercase().contains(&s.search_filter.to_lowercase())
    {
        return false;
    }
    true
}

/// Absolute (stable) key of `entries[0]`: total pushed minus what's retained.
fn base_key(s: &ConsoleState) -> u64 {
    s.pushed.saturating_sub(s.entries.len() as u64)
}

/// The entry with absolute key `key`, if it is still in the buffer.
fn entry_at(s: &ConsoleState, key: u64) -> Option<&LogEntry> {
    let base = base_key(s);
    if key < base {
        return None;
    }
    s.entries.get((key - base) as usize)
}

// ── Commands ────────────────────────────────────────────────────────────────

/// One console command, as `/help` lists it and the suggestions show it.
pub(crate) struct CommandInfo {
    pub(crate) name: &'static str,
    pub(crate) args: &'static str,
    pub(crate) desc: &'static str,
}

/// Every command `execute_command` answers. One list, so the suggestions and
/// `/help` can never offer a command that does not exist.
pub(crate) const COMMANDS: &[CommandInfo] = &[
    CommandInfo { name: "/clear", args: "", desc: "Clear the console" },
    CommandInfo { name: "/help", args: "", desc: "List the commands" },
];

/// The commands matching what has been typed so far: only while a command name
/// is being typed (starts with `/`, no space yet), and not once the whole name
/// is there.
pub(crate) fn suggestions(input: &str) -> Vec<&'static CommandInfo> {
    let typed = input.trim_start();
    if !typed.starts_with('/') || typed.contains(char::is_whitespace) {
        return Vec::new();
    }
    let matches: Vec<&CommandInfo> = COMMANDS.iter().filter(|c| c.name.starts_with(typed)).collect();
    if matches.len() == 1 && matches[0].name == typed {
        return Vec::new();
    }
    matches
}

// ── Components & view state ─────────────────────────────────────────────────

#[derive(Component, Clone, Copy, PartialEq)]
pub(crate) enum ConsoleBtn {
    Clear,
    Copy,
    Level(LogLevel),
    Follow,
    CopyEntry,
    CloseDetail,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum ConsoleField {
    Search,
    Command,
}

/// A log line (its absolute key): click to open its details.
#[derive(Component)]
pub(crate) struct ConsoleRow(u64);

/// An entity id chip in the details pane: click to select that entity.
#[derive(Component)]
pub(crate) struct ConsoleEntityChip(String);

/// The category menu's trigger.
#[derive(Component)]
pub(crate) struct CategoryBtn;

/// A command suggestion row (the command name it completes to).
#[derive(Component)]
pub(crate) struct SuggestRow(&'static str);

/// The log's scroll view (its outer node), for Follow.
#[derive(Component)]
pub(crate) struct ConsoleLogScroll;

/// Panel-only state: which line is open and where the suggestion cursor is.
/// Kept off `ConsoleState`, which is the log itself and lives with or without
/// a panel.
#[derive(Resource, Default)]
pub(crate) struct ConsoleView {
    /// Absolute key of the line whose details are open.
    selected: Option<u64>,
    /// Highlighted suggestion.
    suggest_index: usize,
    /// The command text the suggestions were dismissed for (Escape); they come
    /// back as soon as the text changes.
    dismissed: Option<String>,
    /// The command text last seen, to reset the cursor when it changes.
    last_input: String,
}

fn view<'a>(rx: &'a Rx) -> Option<&'a ConsoleView> {
    rx.get_resource::<ConsoleView>()
}

// ── Build helpers ───────────────────────────────────────────────────────────

fn hover_pointer() -> renzora_ember::cursor_icon::HoverCursor {
    renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer)
}

fn vsep(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(1.0),
                height: Val::Px(18.0),
                margin: UiRect::horizontal(Val::Px(4.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(border())),
        ))
        .id()
}

fn spacer(commands: &mut Commands) -> Entity {
    commands.spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), ..default() }).id()
}

fn text(commands: &mut Commands, font: &bevy::text::FontSource, s: &str, color: Color, size: f32) -> Entity {
    commands
        .spawn((Text::new(s), ui_font(font, size), TextColor(color), bevy::text::TextLayout::no_wrap()))
        .id()
}

/// Hover wash on `e`, the one every clickable thing here wants.
fn hover_bg_of(commands: &mut Commands, e: Entity) {
    bind_bg(commands, e, move |w| match w.get::<UiInteraction>(e) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(hover_bg()),
        _ => Color::NONE,
    });
}

/// A square icon button with a tooltip.
fn icon_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str, tip: &str, btn: ConsoleBtn) -> Entity {
    let b = commands
        .spawn((
            Node {
                width: Val::Px(28.0),
                height: Val::Px(26.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            hover_pointer(),
            HoverTooltip::new(tip),
            btn,
        ))
        .id();
    hover_bg_of(commands, b);
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 14.0);
    commands.entity(ic).insert(PASS_THROUGH);
    commands.entity(b).add_child(ic);
    b
}

/// A rounded pill: optional icon, label. The caller binds its colours, and
/// adds what it does (a `ConsoleBtn`, or another marker).
fn pill(commands: &mut Commands, fonts: &EmberFonts, icon: Option<&str>, label: &str) -> (Entity, Entity) {
    let b = commands
        .spawn((
            Node {
                height: Val::Px(26.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(13.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::NONE),
            UiInteraction::default(),
            hover_pointer(),
        ))
        .id();
    let mut kids = Vec::new();
    if let Some(icon) = icon {
        let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 12.0);
        commands.entity(ic).insert(PASS_THROUGH);
        kids.push(ic);
    }
    let t = text(commands, &fonts.ui, label, rgb(text_primary()), 12.0);
    kids.push(t);
    commands.entity(b).add_children(&kids);
    (b, t)
}

/// A level pill: dot, name, line count. Lit while the level is shown.
fn level_pill(commands: &mut Commands, fonts: &EmberFonts, level: LogLevel) -> Entity {
    let (b, label) = pill(commands, fonts, None, &level_label(level));
    commands.entity(b).insert(ConsoleBtn::Level(level));
    let dot = commands
        .spawn((
            Node {
                width: Val::Px(7.0),
                height: Val::Px(7.0),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(level_color(level))),
            PASS_THROUGH,
        ))
        .id();
    let count = text(commands, &fonts.mono, "0", rgb(text_muted()), 11.0);
    bind_text(commands, count, move |w| {
        w.get_resource::<ConsoleState>()
            .map(|s| s.entries.iter().filter(|e| e.level == level).count().to_string())
            .unwrap_or_default()
    });
    commands.entity(b).insert_children(0, &[dot]);
    commands.entity(b).add_child(count);
    let on = move |w: &Rx| w.get_resource::<ConsoleState>().is_some_and(|s| level_shown(s, level));
    bind_bg(commands, b, move |w| if on(w) { rgb(hover_bg()) } else { Color::NONE });
    bind_with(
        commands,
        b,
        move |w: &Rx| on(w),
        |world: &mut World, e: Entity, on: &bool| {
            if let Some(mut bc) = world.get_mut::<BorderColor>(e) {
                *bc = BorderColor::all(if *on { rgb(border()) } else { rgb(border()).with_alpha(0.45) });
            }
        },
    );
    bind_with(
        commands,
        label,
        move |w: &Rx| on(w),
        |world: &mut World, e: Entity, on: &bool| {
            if let Some(mut c) = world.get_mut::<TextColor>(e) {
                c.0 = if *on { rgb(text_primary()) } else { rgb(text_muted()).with_alpha(0.6) };
            }
        },
    );
    b
}

/// The EmberScroll viewport inside the log's scroll view.
fn log_scroll_viewport(w: &Rx, outer: Entity) -> Option<Entity> {
    w.get::<Children>(outer)?.iter().find(|c| w.get::<EmberScroll>(*c).is_some())
}

/// Build the whole console into a root node and wire its reactive parts.
fn build_console(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::clip(),
                ..default()
            },
            Name::new("console-root"),
        ))
        .id();

    // ── Toolbar ──
    let toolbar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(38.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(10.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                flex_shrink: 0.0,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(header_bg())),
            BorderColor::all(rgb(border())),
            Name::new("console-toolbar"),
        ))
        .id();
    let mut bar: Vec<Entity> = LEVELS.iter().map(|l| level_pill(commands, fonts, *l)).collect();
    bar.push(vsep(commands));

    // Fields start empty; `console_text_sync` mirrors them into `ConsoleState`.
    let search = search_input(commands, fonts, &renzora::lang::t_or("console.search", "Filter messages"), "");
    commands.entity(search).insert(ConsoleField::Search);
    commands.entity(search).entry::<Node>().and_modify(|mut n| {
        n.width = Val::Px(300.0);
        n.min_width = Val::Px(80.0);
        n.flex_shrink = 1.0;
        n.height = Val::Px(26.0);
    });
    bar.push(search);

    let (cat_btn, cat_label) = pill(commands, fonts, Some("funnel"), "");
    // Not a ConsoleBtn: it opens a menu, so it gets its own marker instead.
    commands.entity(cat_btn).insert((CategoryBtn, RelativeCursorPosition::default()));
    commands.entity(cat_btn).entry::<Node>().and_modify(|mut n| n.border_radius = BorderRadius::all(Val::Px(6.0)));
    bind_bg(commands, cat_btn, move |w| match w.get::<UiInteraction>(cat_btn) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(hover_bg()),
        _ => rgb(faint_bg()),
    });
    bind_text(commands, cat_label, |w| {
        let hidden = w.get_resource::<ConsoleState>().map(|s| s.hidden_categories.len()).unwrap_or(0);
        if hidden == 0 {
            renzora::lang::t_or("console.all_categories", "All categories")
        } else {
            renzora::lang::t_or("console.categories_hidden", "{n} hidden").replace("{n}", &hidden.to_string())
        }
    });
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 10.0);
    commands.entity(caret).insert(PASS_THROUGH);
    commands.entity(cat_btn).add_child(caret);
    bar.push(cat_btn);

    bar.push(spacer(commands));
    let count_label = text(commands, &fonts.mono, "0 / 0", rgb(text_muted()), 11.0);
    bind_text(commands, count_label, |w| {
        let Some(s) = w.get_resource::<ConsoleState>() else {
            return "0 / 0".to_string();
        };
        format!("{} / {}", s.filtered_entries().count(), s.entries.len())
    });
    bar.push(count_label);
    bar.push(icon_button(commands, fonts, "copy", &renzora::lang::t_or("console.copy_visible", "Copy what is shown"), ConsoleBtn::Copy));
    bar.push(icon_button(commands, fonts, "trash", &renzora::lang::t("console.clear"), ConsoleBtn::Clear));

    // ── Log (scrollable, flex-fills, reactive) ──
    let log_list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::vertical(Val::Px(4.0)),
            ..default()
        })
        .id();
    keyed_list(commands, log_list, log_snapshot);
    let scroll = scroll_view_pinned(commands, log_list);
    commands.entity(scroll).insert(ConsoleLogScroll);
    commands.entity(scroll).entry::<Node>().and_modify(|mut n| {
        n.flex_grow = 1.0;
        n.min_width = Val::Px(0.0);
    });

    // Follow: lit while the log is following its newest line; pressing it
    // jumps back there after scrolling up to read.
    let (follow, follow_label) = pill(commands, fonts, Some("arrow-down"), &renzora::lang::t_or("console.follow", "Follow"));
    commands.entity(follow).insert(ConsoleBtn::Follow);
    commands.entity(follow).entry::<Node>().and_modify(|mut n| n.border_radius = BorderRadius::all(Val::Px(6.0)));
    let following = move |w: &Rx| {
        log_scroll_viewport(w, scroll)
            .and_then(|v| w.get::<EmberScroll>(v))
            .is_some_and(|s| s.following())
    };
    bind_bg(commands, follow, move |w| {
        if following(w) {
            rgb(accent()).with_alpha(0.18)
        } else {
            match w.get::<UiInteraction>(follow) {
                Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(hover_bg()),
                _ => Color::NONE,
            }
        }
    });
    bind_with(
        commands,
        follow_label,
        move |w: &Rx| following(w),
        |world: &mut World, e: Entity, on: &bool| {
            if let Some(mut c) = world.get_mut::<TextColor>(e) {
                c.0 = if *on { mix(accent(), text_primary(), 0.5) } else { rgb(text_muted()) };
            }
        },
    );
    bar.push(follow);
    commands.entity(toolbar).add_children(&bar);

    // ── Details pane ──
    let detail = commands
        .spawn((
            Node {
                width: Val::Px(DETAIL_W),
                height: Val::Percent(100.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                border: UiRect::left(Val::Px(1.0)),
                min_height: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            Name::new("console-detail"),
        ))
        .id();
    bind_display(commands, detail, |w| {
        let (Some(v), Some(s)) = (view(w), w.get_resource::<ConsoleState>()) else { return false };
        v.selected.is_some_and(|k| entry_at(s, k).is_some())
    });
    let detail_body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            padding: UiRect::axes(Val::Px(14.0), Val::Px(12.0)),
            ..default()
        })
        .id();
    keyed_list(commands, detail_body, detail_snapshot);
    let detail_scroll = scroll_view(commands, detail_body);
    commands.entity(detail).add_child(detail_scroll);

    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .id();
    commands.entity(body).add_children(&[scroll, detail]);

    // ── Command bar ──
    let input_row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(40.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::horizontal(Val::Px(12.0)),
                border: UiRect::top(Val::Px(1.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(header_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();
    let prompt = text(commands, &fonts.mono, ">", rgb(accent()), 13.0);
    let input = text_input(commands, &fonts.mono, &renzora::lang::t_or("console.command_hint", "Type / for commands"), "");
    commands.entity(input).insert(ConsoleField::Command);
    commands.entity(input).insert(Node {
        flex_grow: 1.0,
        min_width: Val::Px(0.0),
        padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
        align_items: AlignItems::Center,
        ..default()
    });
    let hint = text(commands, &fonts.ui, &renzora::lang::t_or("console.enter_to_run", "Enter to run"), rgb(text_muted()), 11.0);
    commands.entity(input_row).add_children(&[prompt, input, hint]);

    // ── Suggestions (over the log, above the bar) ──
    let suggest = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                bottom: Val::Px(44.0),
                width: Val::Px(520.0),
                max_width: Val::Percent(90.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            BoxShadow::new(Color::BLACK.with_alpha(0.45), Val::Px(0.0), Val::Px(-6.0), Val::Px(0.0), Val::Px(18.0)),
            Name::new("console-suggestions"),
        ))
        .id();
    bind_display(commands, suggest, move |w| suggest_open(w, input));
    let suggest_list = commands.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).id();
    keyed_list(commands, suggest_list, move |w| suggest_snapshot(w, input));
    let hints = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(14.0),
            padding: UiRect::new(Val::Px(10.0), Val::Px(10.0), Val::Px(6.0), Val::Px(3.0)),
            margin: UiRect::top(Val::Px(4.0)),
            border: UiRect::top(Val::Px(1.0)),
            ..default()
        })
        .id();
    commands.entity(hints).insert(BorderColor::all(rgb(border())));
    let h1 = text(commands, &fonts.ui, &renzora::lang::t_or("console.suggest_tab", "Tab to complete"), rgb(text_muted()), 11.0);
    let h2 = text(commands, &fonts.ui, &renzora::lang::t_or("console.suggest_arrows", "Up / Down to choose"), rgb(text_muted()), 11.0);
    let h3 = text(commands, &fonts.ui, &renzora::lang::t_or("console.suggest_esc", "Esc to close"), rgb(text_muted()), 11.0);
    commands.entity(hints).add_children(&[h1, h2, h3]);
    commands.entity(suggest).add_children(&[suggest_list, hints]);

    commands.entity(root).add_children(&[toolbar, body, input_row, suggest]);
    root
}

// ── Log rows ────────────────────────────────────────────────────────────────

/// Owned snapshot of one log entry's rendering inputs (so the keyed-list builder
/// can outlive the world borrow).
#[derive(Clone)]
struct RowData {
    key: u64,
    level: LogLevel,
    category: String,
    message: String,
    timestamp: f64,
    count: u32,
}

fn hash_row(r: &RowData) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (level_idx(r.level), &r.category, &r.message, r.timestamp.to_bits(), r.count).hash(&mut h);
    h.finish()
}

fn build_log_row(commands: &mut Commands, fonts: &EmberFonts, r: &RowData) -> Entity {
    let key = r.key;
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(ROW_H),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::horizontal(Val::Px(14.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            hover_pointer(),
            ConsoleRow(key),
        ))
        .id();
    // Flat: one surface for every line. Only the open line is lit, plus the
    // usual hover wash.
    bind_bg(commands, row, move |w| {
        if view(w).is_some_and(|v| v.selected == Some(key)) {
            return renzora_ember::inspector::list_row_selected();
        }
        match w.get::<UiInteraction>(row) {
            Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => renzora_ember::inspector::list_row_hover(),
            _ => Color::NONE,
        }
    });

    let time = text(commands, &fonts.mono, &fmt_time(r.timestamp), rgb(text_muted()).with_alpha(0.8), 11.0);
    commands.entity(time).insert((Node { width: Val::Px(52.0), flex_shrink: 0.0, ..default() }, PASS_THROUGH));
    let dot = commands
        .spawn((
            Node {
                width: Val::Px(8.0),
                height: Val::Px(8.0),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(level_color(r.level))),
            PASS_THROUGH,
        ))
        .id();
    let cat = text(commands, &fonts.mono, &r.category, rgb(category_color(&r.category)), 12.0);
    commands
        .entity(cat)
        .insert((Node { width: Val::Px(84.0), flex_shrink: 0.0, overflow: Overflow::clip(), ..default() }, PASS_THROUGH));
    // One line, cut at the edge: the details pane has the rest.
    let one_line = r.message.replace(['\n', '\r'], "  ");
    let msg = commands
        .spawn((
            Text::new(one_line),
            ui_font(&fonts.mono, 12.0),
            TextColor(message_color(r.level)),
            bevy::text::TextLayout::no_wrap(),
            Node { flex_grow: 1.0, min_width: Val::Px(0.0), overflow: Overflow::clip(), ..default() },
            PASS_THROUGH,
        ))
        .id();
    let mut kids = vec![time, dot, cat, msg];
    if r.count > 1 {
        kids.push(count_badge(commands, fonts, r.count));
    }
    commands.entity(row).add_children(&kids);
    row
}

/// A small rounded "×N" pill shown when a log line repeated.
fn count_badge(commands: &mut Commands, fonts: &EmberFonts, count: u32) -> Entity {
    let badge = commands
        .spawn((
            Node {
                height: Val::Px(18.0),
                padding: UiRect::horizontal(Val::Px(7.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(9.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
            PASS_THROUGH,
        ))
        .id();
    let label = text(commands, &fonts.mono, &format!("\u{00d7}{count}"), rgb(text_primary()), 11.0);
    commands.entity(badge).add_child(label);
    badge
}

/// Keyed-list snapshot for the log list: filtered entries (keyed by their
/// monotonic push index so appends/trims/filter-changes are granular), trimmed
/// to the last [`MAX_ROWS`], or a single "No messages" placeholder.
fn log_snapshot(world: &Rx) -> KeyedSnapshot {
    let Some(state) = world.get_resource::<ConsoleState>() else {
        return KeyedSnapshot {
            items: Vec::new(),
            build: Box::new(|_, _, _| Entity::PLACEHOLDER),
        };
    };
    let base = base_key(state);
    let mut visible: Vec<RowData> = state
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| passes(state, e))
        .map(|(i, e)| RowData {
            key: base + i as u64,
            level: e.level,
            category: e.category.clone(),
            message: e.message.clone(),
            timestamp: e.timestamp,
            count: e.count,
        })
        .collect();
    let start = visible.len().saturating_sub(MAX_ROWS);
    let rows = visible.split_off(start);

    if rows.is_empty() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 0)],
            build: Box::new(|c, f, _| {
                let t = text(c, &f.ui, &renzora::lang::t("console.empty"), rgb(text_muted()), 13.0);
                c.entity(t).insert(Node { margin: UiRect::all(Val::Px(14.0)), ..default() });
                t
            }),
        };
    }

    let items: Vec<(u64, u64)> = rows.iter().map(|r| (r.key, hash_row(r))).collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| build_log_row(c, f, &rows[i])),
    }
}

// ── Details pane ────────────────────────────────────────────────────────────

fn detail_snapshot(world: &Rx) -> KeyedSnapshot {
    let entry = view(world)
        .and_then(|v| v.selected)
        .and_then(|k| world.get_resource::<ConsoleState>().and_then(|s| entry_at(s, k)).map(|e| (k, e.clone())));
    let Some((key, e)) = entry else {
        return KeyedSnapshot { items: Vec::new(), build: Box::new(|_, _, _| Entity::PLACEHOLDER) };
    };
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (key, &e.message, e.count, e.frame, e.timestamp.to_bits()).hash(&mut h);
    KeyedSnapshot {
        items: vec![(key, h.finish())],
        build: Box::new(move |c, f, _| build_detail(c, f, &e)),
    }
}

fn build_detail(commands: &mut Commands, fonts: &EmberFonts, e: &LogEntry) -> Entity {
    let col = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            ..default()
        })
        .id();

    let head = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let dot = commands
        .spawn((
            Node {
                width: Val::Px(8.0),
                height: Val::Px(8.0),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(level_color(e.level))),
        ))
        .id();
    let mut head_kids = vec![dot];
    if !e.category.is_empty() {
        let mut font = ui_font(&fonts.ui, 12.0);
        font.weight = bevy::text::FontWeight::SEMIBOLD;
        let cat = commands
            .spawn((Text::new(e.category.clone()), font, TextColor(rgb(category_color(&e.category)))))
            .id();
        head_kids.push(cat);
    }
    let when = format!("{} at {}", level_label(e.level), fmt_time(e.timestamp));
    head_kids.push(text(commands, &fonts.ui, &when, rgb(text_muted()), 12.0));
    head_kids.push(spacer(commands));
    let (copy, _) = pill(commands, fonts, Some("copy"), &renzora::lang::t("console.copy"));
    commands.entity(copy).insert(ConsoleBtn::CopyEntry);
    commands.entity(copy).entry::<Node>().and_modify(|mut n| {
        n.height = Val::Px(24.0);
        n.border_radius = BorderRadius::all(Val::Px(5.0));
    });
    bind_bg(commands, copy, move |w| match w.get::<UiInteraction>(copy) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(hover_bg()),
        _ => rgb(faint_bg()),
    });
    head_kids.push(copy);
    head_kids.push(icon_button(commands, fonts, "x", &renzora::lang::t_or("console.close_details", "Close"), ConsoleBtn::CloseDetail));
    commands.entity(head).add_children(&head_kids);

    let msg_box = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(faint_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();
    let msg = commands
        .spawn((
            Text::new(e.message.clone()),
            ui_font(&fonts.mono, 12.0),
            TextColor(message_color(e.level)),
            // Paths and entity lists have no spaces to break at.
            bevy::text::TextLayout::linebreak(bevy::text::LineBreak::AnyCharacter),
        ))
        .id();
    commands.entity(msg_box).add_child(msg);
    let mut kids = vec![head, msg_box];

    let ids = entity_ids(&e.message);
    if !ids.is_empty() {
        let mut font = ui_font(&fonts.ui, 11.0);
        font.weight = bevy::text::FontWeight::SEMIBOLD;
        let heading = commands
            .spawn((
                Text::new(renzora::lang::t_or("console.entities", "ENTITIES")),
                font,
                TextColor(rgb(text_muted())),
            ))
            .id();
        let wrap = commands
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
                ..default()
            })
            .id();
        let chips: Vec<Entity> = ids
            .into_iter()
            .map(|id| {
                let chip = commands
                    .spawn((
                        Node {
                            height: Val::Px(22.0),
                            padding: UiRect::horizontal(Val::Px(8.0)),
                            align_items: AlignItems::Center,
                            border_radius: BorderRadius::all(Val::Px(11.0)),
                            ..default()
                        },
                        BackgroundColor(rgb(accent()).with_alpha(0.14)),
                        UiInteraction::default(),
                        hover_pointer(),
                        HoverTooltip::new(renzora::lang::t_or("console.select_entity", "Select this entity")),
                        ConsoleEntityChip(id.clone()),
                    ))
                    .id();
                bind_bg(commands, chip, move |w| match w.get::<UiInteraction>(chip) {
                    Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(accent()).with_alpha(0.28),
                    _ => rgb(accent()).with_alpha(0.14),
                });
                let t = text(commands, &fonts.mono, &id, mix(accent(), text_primary(), 0.5), 11.0);
                commands.entity(t).insert(PASS_THROUGH);
                commands.entity(chip).add_child(t);
                chip
            })
            .collect();
        commands.entity(wrap).add_children(&chips);
        kids.push(heading);
        kids.push(wrap);
    }

    let seen = if e.count > 1 {
        renzora::lang::t_or("console.seen_times", "Seen {n} times").replace("{n}", &e.count.to_string())
    } else {
        renzora::lang::t_or("console.seen_once", "Seen once")
    };
    let footer = format!("{seen}    {} {}", renzora::lang::t_or("console.frame", "Frame"), e.frame);
    kids.push(text(commands, &fonts.ui, &footer, rgb(text_muted()), 11.0));
    commands.entity(col).add_children(&kids);
    col
}

// ── Suggestions ─────────────────────────────────────────────────────────────

fn command_value(w: &Rx, input: Entity) -> String {
    w.get::<EmberTextInput>(input).map(|i| i.value.clone()).unwrap_or_default()
}

fn suggest_open(w: &Rx, input: Entity) -> bool {
    let Some(inp) = w.get::<EmberTextInput>(input) else { return false };
    if !inp.focused || suggestions(&inp.value).is_empty() {
        return false;
    }
    view(w).is_none_or(|v| v.dismissed.as_deref() != Some(inp.value.as_str()))
}

fn suggest_snapshot(w: &Rx, input: Entity) -> KeyedSnapshot {
    let value = command_value(w, input);
    let typed = value.trim_start().to_string();
    let list = suggestions(&value);
    let cursor = view(w).map(|v| v.suggest_index).unwrap_or(0).min(list.len().saturating_sub(1));
    let items: Vec<(u64, u64)> = list
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            c.name.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (&typed, i == cursor).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| build_suggestion(c, f, list[i], &typed, i == cursor)),
    }
}

fn build_suggestion(commands: &mut Commands, fonts: &EmberFonts, cmd: &'static CommandInfo, typed: &str, lit: bool) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(30.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                padding: UiRect::horizontal(Val::Px(10.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            hover_pointer(),
            SuggestRow(cmd.name),
        ))
        .id();
    bind_bg(commands, row, move |w| {
        if lit {
            return renzora_ember::theme::menu_hover_bg();
        }
        match w.get::<UiInteraction>(row) {
            Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(hover_bg()),
            _ => Color::NONE,
        }
    });
    // The part already typed in the accent, the rest plain: what Tab adds.
    let split = typed.len().min(cmd.name.len());
    let name = commands
        .spawn(Node { width: Val::Px(140.0), flex_shrink: 0.0, flex_direction: FlexDirection::Row, ..default() })
        .id();
    let a = text(commands, &fonts.mono, &cmd.name[..split], rgb(accent()), 12.0);
    let b = text(commands, &fonts.mono, &cmd.name[split..], rgb(text_primary()), 12.0);
    commands.entity(name).add_children(&[a, b]);
    let args = text(commands, &fonts.mono, cmd.args, rgb(text_muted()), 12.0);
    commands.entity(args).insert(Node { width: Val::Px(120.0), flex_shrink: 0.0, ..default() });
    let desc = text(commands, &fonts.ui, cmd.desc, rgb(text_muted()), 12.0);
    for e in [name, a, b, args, desc] {
        commands.entity(e).insert(PASS_THROUGH);
    }
    commands.entity(row).add_children(&[name, args, desc]);
    row
}

// ── Registration ────────────────────────────────────────────────────────────

/// Wire the console into the editor (a single call from `ConsolePlugin`).
pub fn register(app: &mut App) {
    use renzora::SplashState;
    app.init_resource::<ConsoleView>();
    // `scroll = false`: the console manages its own internal log scroll.
    app.register_panel_content(PANEL_ID, false, build_console);
    // View systems only — gated on visibility. Log *capture* lives in
    // `ConsolePlugin` and stays always-on so a hidden console keeps collecting.
    app.add_systems(
        Update,
        (
            console_buttons,
            console_rows,
            console_entity_chips,
            console_category_menu,
            console_suggest,
            console_text_sync,
        )
            .chain()
            .run_if(in_state(SplashState::Editor))
            .run_if(renzora_ember::dock::panel_active(PANEL_ID)),
    );
}

// ── Systems ───────────────────────────────────────────────────────────────

/// Toolbar and details-pane button clicks.
pub(crate) fn console_buttons(
    q: Query<(&UiInteraction, &ConsoleBtn), Changed<UiInteraction>>,
    mut state: ResMut<ConsoleState>,
    mut view: ResMut<ConsoleView>,
    scrolls: Query<&Children, With<ConsoleLogScroll>>,
    mut viewports: Query<&mut EmberScroll>,
) {
    for (interaction, btn) in &q {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        match btn {
            ConsoleBtn::Clear => {
                state.clear();
                view.selected = None;
            }
            // Copies what is on screen, not the whole buffer: the filters are
            // how someone narrows a log down to the part worth pasting into an
            // issue, and copying everything would discard that work. A
            // collapsed repeat keeps its count; losing "x412" would turn a
            // flood into what looks like a single line.
            ConsoleBtn::Copy => {
                let mut out = String::new();
                for e in state.filtered_entries() {
                    out.push_str(&format!("[{}] ", fmt_time(e.timestamp)));
                    if !e.category.is_empty() {
                        out.push_str(&format!("[{}] ", e.category));
                    }
                    out.push_str(&format!("{:?}: {}", e.level, e.message));
                    if e.count > 1 {
                        out.push_str(&format!(" (x{})", e.count));
                    }
                    out.push('\n');
                }
                renzora_ember::widgets::clipboard::set_text(&out);
            }
            ConsoleBtn::Level(level) => match level {
                LogLevel::Info => state.show_info = !state.show_info,
                LogLevel::Success => state.show_success = !state.show_success,
                LogLevel::Warning => state.show_warnings = !state.show_warnings,
                LogLevel::Error => state.show_errors = !state.show_errors,
            },
            ConsoleBtn::Follow => {
                for kids in &scrolls {
                    for k in kids.iter() {
                        if let Ok(mut s) = viewports.get_mut(k) {
                            s.stick_to_bottom();
                        }
                    }
                }
            }
            ConsoleBtn::CopyEntry => {
                if let Some(e) = view.selected.and_then(|k| entry_at(&state, k)) {
                    renzora_ember::widgets::clipboard::set_text(&e.message);
                }
            }
            ConsoleBtn::CloseDetail => view.selected = None,
        }
    }
}

/// A line click opens its details, or closes them if it is the open one. Also
/// drops a selection whose line has left the buffer (cleared, or trimmed off
/// the front).
pub(crate) fn console_rows(
    q: Query<(&UiInteraction, &ConsoleRow), Changed<UiInteraction>>,
    state: Res<ConsoleState>,
    mut view: ResMut<ConsoleView>,
) {
    for (interaction, row) in &q {
        if *interaction == UiInteraction::Pressed {
            view.selected = if view.selected == Some(row.0) { None } else { Some(row.0) };
        }
    }
    if let Some(k) = view.selected {
        if entry_at(&state, k).is_none() {
            view.selected = None;
        }
    }
}

/// An entity chip selects the entity it names, if it still exists.
pub(crate) fn console_entity_chips(
    q: Query<(&UiInteraction, &ConsoleEntityChip), Changed<UiInteraction>>,
    entities: Query<Entity>,
    selection: Option<Res<renzora::EditorSelection>>,
    mut state: ResMut<ConsoleState>,
) {
    let Some(selection) = selection else { return };
    for (interaction, chip) in &q {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        let found = entities.iter().find(|e| format!("{}v{}", e.index(), e.generation()) == chip.0);
        match found {
            Some(e) => selection.set(Some(e)),
            None => state.log(LogLevel::Warning, "Console", format!("Entity {} no longer exists", chip.0)),
        }
    }
}

/// The category button opens a menu of every category seen, each ticked while
/// shown; picking one shows or hides it.
type CategoryTrigger<'a> = (&'a UiInteraction, &'a RelativeCursorPosition, &'a ComputedNode);

pub(crate) fn console_category_menu(
    q: Query<CategoryTrigger, (With<CategoryBtn>, Changed<UiInteraction>)>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    state: Res<ConsoleState>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, rcp, cn)) = q.iter().find(|(i, _, _)| **i == UiInteraction::Pressed) else { return };
    let Some((cursor, win_h)) = windows.iter().find_map(|w| w.cursor_position().map(|c| (c, w.height()))) else {
        return;
    };
    let mut cats = state.seen_categories.clone();
    cats.sort_by_key(|c| c.to_lowercase());
    let menu = screen_menu_under(
        &mut commands,
        trigger_rect(cursor, rcp, cn),
        win_h,
        screen_menu_est_height(cats.len() + 1, 1),
    );
    let mut kids = vec![menu_item_styled(
        &mut commands,
        &fonts,
        "eye",
        &renzora::lang::t_or("console.show_all_categories", "Show all"),
        text_muted(),
        text_primary(),
        |w| {
            if let Some(mut s) = w.get_resource_mut::<ConsoleState>() {
                s.hidden_categories.clear();
            }
        },
    )];
    kids.push(menu_sep(&mut commands));
    for cat in cats {
        let shown = !state.hidden_categories.contains(&cat);
        let name = cat.clone();
        kids.push(menu_item_styled(
            &mut commands,
            &fonts,
            if shown { "check-square" } else { "square" },
            &cat,
            if shown { category_color(&cat) } else { text_muted() },
            if shown { text_primary() } else { text_muted() },
            move |w| {
                if let Some(mut s) = w.get_resource_mut::<ConsoleState>() {
                    if !s.hidden_categories.remove(&name) {
                        s.hidden_categories.insert(name.clone());
                    }
                }
            },
        ));
    }
    commands.entity(menu).add_children(&kids);
}

/// Drive the command suggestions: Up / Down move the highlight, Tab or a click
/// completes, Escape dismisses until the text changes.
pub(crate) fn console_suggest(
    keys: Res<ButtonInput<KeyCode>>,
    rows: Query<(&UiInteraction, &SuggestRow), Changed<UiInteraction>>,
    mut inputs: Query<(&mut EmberTextInput, &ConsoleField)>,
    mut view: ResMut<ConsoleView>,
) {
    let Some((mut inp, _)) = inputs.iter_mut().find(|(_, f)| matches!(f, ConsoleField::Command)) else {
        return;
    };
    if inp.value != view.last_input {
        view.last_input = inp.value.clone();
        view.suggest_index = 0;
        if view.dismissed.as_deref() != Some(inp.value.as_str()) {
            view.dismissed = None;
        }
    }
    let list = suggestions(&inp.value);
    let clicked = rows.iter().find(|(i, _)| **i == UiInteraction::Pressed).map(|(_, r)| r.0);
    let open = !list.is_empty() && view.dismissed.is_none();
    let complete = |inp: &mut EmberTextInput, name: &str| {
        inp.value = format!("{name} ");
        inp.caret_index = inp.value.chars().count();
        inp.sel_anchor = None;
        inp.select_all = false;
        inp.focused = true;
    };
    if let Some(name) = clicked {
        complete(&mut inp, name);
        return;
    }
    if !open || !inp.focused {
        return;
    }
    let n = list.len();
    if keys.just_pressed(KeyCode::ArrowDown) {
        view.suggest_index = (view.suggest_index + 1) % n;
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        view.suggest_index = (view.suggest_index + n - 1) % n;
    } else if keys.just_pressed(KeyCode::Tab) {
        let name = list[view.suggest_index.min(n - 1)].name;
        complete(&mut inp, name);
    } else if keys.just_pressed(KeyCode::Escape) {
        view.dismissed = Some(inp.value.clone());
    }
}

/// Mirror the search field into `ConsoleState` (guarded), and submit the
/// command field on Enter.
///
/// Enter is read off the keyboard, not off the field's value: ember's
/// single-line input leaves Enter to whoever owns the submit and never puts a
/// newline in `value`, so waiting for one (as this used to) meant a command
/// could be typed but never run.
pub(crate) fn console_text_sync(
    keys: Res<ButtonInput<KeyCode>>,
    mut q: Query<(&mut EmberTextInput, &ConsoleField)>,
    mut state: ResMut<ConsoleState>,
) {
    let enter = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);
    for (mut inp, field) in &mut q {
        match field {
            ConsoleField::Search => {
                if state.search_filter != inp.value {
                    state.search_filter = inp.value.clone();
                }
            }
            ConsoleField::Command => {
                if enter && inp.focused {
                    let command = inp.value.trim().to_string();
                    // The widget redraws itself (back to the placeholder) when
                    // its value changes.
                    inp.value.clear();
                    inp.caret_index = 0;
                    if !command.is_empty() {
                        submit_command(&mut state, &command);
                    }
                }
            }
        }
    }
}

fn submit_command(state: &mut ConsoleState, command: &str) {
    state.log(LogLevel::Info, "Input", format!("> {}", command));
    if command.starts_with('/') {
        execute_command(command, state);
    } else {
        state.log(LogLevel::Info, "Input", command);
    }
    state.command_history.push(command.to_string());
    state.history_index = None;
    state.saved_input.clear();
    state.auto_scroll = true;
}

fn execute_command(input: &str, state: &mut ConsoleState) {
    let trimmed = input.strip_prefix('/').unwrap_or(input);
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    let Some(&cmd) = parts.first() else {
        return;
    };
    match cmd {
        "clear" => {
            state.clear();
            state.log(LogLevel::Success, "Command", "Console cleared.");
        }
        "help" => {
            let mut help = String::from("Available commands:");
            for c in COMMANDS {
                help.push_str(&format!("\n  {:<10}{}", c.name, c.desc));
            }
            state.log(LogLevel::Success, "Command", help);
        }
        other => state.log(
            LogLevel::Error,
            "Command",
            format!("Unknown command: /{}. Type /help for a list of commands.", other),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_ids_are_found_once_each_in_order() {
        let ids = entity_ids("Multi-select: 3 entities [1871v0, 1872v0, 1871v0] in 2v nothing v3 12v4x");
        assert_eq!(ids, vec!["1871v0", "1872v0"]);
        assert_eq!(entity_ids("Selected 7633v31"), vec!["7633v31"]);
        assert!(entity_ids("Saved new_scene.bsn (11 entities)").is_empty());
    }

    #[test]
    fn suggestions_follow_the_typed_prefix() {
        assert_eq!(suggestions("/").len(), COMMANDS.len());
        assert_eq!(suggestions("/cl").iter().map(|c| c.name).collect::<Vec<_>>(), vec!["/clear"]);
        // Not while a command is complete, or once arguments start, or for
        // plain text.
        assert!(suggestions("/clear").is_empty());
        assert!(suggestions("/help me").is_empty());
        assert!(suggestions("hello").is_empty());
        assert!(suggestions("/zz").is_empty());
    }

    #[test]
    fn every_listed_command_is_answered() {
        for c in COMMANDS {
            let mut state = ConsoleState::default();
            state.entries.clear();
            execute_command(c.name, &mut state);
            assert!(
                !state.entries.iter().any(|e| e.message.starts_with("Unknown command")),
                "{} is listed but not handled",
                c.name
            );
        }
    }

    #[test]
    fn times_read_as_minutes_then_hours() {
        assert_eq!(fmt_time(5.25), "00:05.2");
        assert_eq!(fmt_time(125.0), "02:05.0");
        assert_eq!(fmt_time(3725.0), "1:02:05");
    }
}
