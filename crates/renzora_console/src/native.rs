//! Bevy-native (ember) console panel — a faithful bevy_ui port of the egui
//! `render_console_content`: toolbar (clear/copy, level filters, search/category,
//! count, timestamp/frame/auto-scroll toggles), category chips, the log list, and
//! the command input bar.
//!
//! The shell is built once into the leaf's `content` entity; everything live is
//! reactive: the log list and chips are `keyed_list`s (only changed/added/removed
//! rows are touched — never a full respawn, which is what used to freeze the
//! editor), the count label and toggle icons are value-diffed bindings. Toolbar
//! clicks + text fields write straight back to `ConsoleState`.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_text, bind_text_color, keyed_list, KeyedSnapshot};
use renzora_ember::theme::{
    accent, close_red, divider, placeholder, play_green, rgb, section_bg, tab_hover, text_muted,
    text_primary, warn_amber,
};
use renzora_ember::widgets::{scroll_view_pinned, text_input, EmberTextInput};

use crate::state::{ConsoleState, LogEntry, LogLevel};

const PANEL_ID: &str = "console";
/// Max log rows kept in the DOM at once (older ones are trimmed from the front).
const MAX_ROWS: usize = 400;

fn level_color(level: LogLevel) -> (u8, u8, u8) {
    match level {
        LogLevel::Info => accent(),
        LogLevel::Success => play_green(),
        LogLevel::Warning => warn_amber(),
        LogLevel::Error => close_red(),
    }
}

fn level_icon(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Info => "info",
        LogLevel::Success => "check-circle",
        LogLevel::Warning => "warning",
        LogLevel::Error => "x-circle",
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

/// Does this entry pass the current filters? (Mirrors `ConsoleState::filtered_entries`.)
fn passes(s: &ConsoleState, e: &LogEntry) -> bool {
    let level_ok = match e.level {
        LogLevel::Info => s.show_info,
        LogLevel::Success => s.show_success,
        LogLevel::Warning => s.show_warnings,
        LogLevel::Error => s.show_errors,
    };
    if !level_ok {
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

// ── Components ──────────────────────────────────────────────────────────────

#[derive(Component, Clone, Copy, PartialEq)]
pub(crate) enum ConsoleBtn {
    Clear,
    Copy,
    Info,
    Success,
    Warn,
    Error,
    Timestamps,
    Frame,
    AutoScroll,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum ConsoleField {
    Search,
    Category,
    Command,
}

/// A category filter chip (click to hide/show that category).
#[derive(Component)]
pub(crate) struct ConsoleChip(String);

// ── Build helpers ───────────────────────────────────────────────────────────

fn vsep(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(1.0),
                height: Val::Px(16.0),
                margin: UiRect::horizontal(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(divider())),
        ))
        .id()
}

fn hsep(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(1.0),
                margin: UiRect::vertical(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(rgb(divider())),
        ))
        .id()
}

/// A text+icon toolbar button (clear, copy).
fn text_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str, btn: ConsoleBtn) -> Entity {
    let b = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(tab_hover())),
            Interaction::default(),
            btn,
            Name::new(format!("console-btn:{label}")),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 12.0);
    let tx = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(b).add_children(&[ic, tx]);
    b
}

/// Current color of a toggle's glyph from the matching `ConsoleState` flag.
fn toggle_color(world: &World, btn: ConsoleBtn) -> Color {
    let Some(s) = world.get_resource::<ConsoleState>() else {
        return rgb(placeholder());
    };
    let (active, on) = match btn {
        ConsoleBtn::Info => (s.show_info, accent()),
        ConsoleBtn::Success => (s.show_success, play_green()),
        ConsoleBtn::Warn => (s.show_warnings, warn_amber()),
        ConsoleBtn::Error => (s.show_errors, close_red()),
        ConsoleBtn::Timestamps => (s.show_timestamps, accent()),
        ConsoleBtn::Frame => (s.show_frame, accent()),
        ConsoleBtn::AutoScroll => (s.auto_scroll, accent()),
        _ => return rgb(placeholder()),
    };
    rgb(if active { on } else { placeholder() })
}

/// An icon-only toggle button whose glyph recolors with its `ConsoleState` flag.
fn toggle_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str, btn: ConsoleBtn, size: f32) -> Entity {
    let ic = icon_text(commands, &fonts.phosphor, icon, placeholder(), size);
    bind_text_color(commands, ic, move |w| toggle_color(w, btn));
    let b = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                flex_shrink: 0.0,
                ..default()
            },
            Interaction::default(),
            btn,
            Name::new("console-toggle"),
        ))
        .id();
    commands.entity(b).add_child(ic);
    b
}

fn label_text(commands: &mut Commands, fonts: &EmberFonts, text: &str, color: (u8, u8, u8), size: f32) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, size),
            TextColor(rgb(color)),
        ))
        .id()
}

fn mono_text(commands: &mut Commands, fonts: &EmberFonts, text: &str, color: (u8, u8, u8), size: f32) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.mono, size),
            TextColor(rgb(color)),
        ))
        .id()
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
                padding: UiRect::all(Val::Px(4.0)),
                ..default()
            },
            Name::new("console-root"),
        ))
        .id();

    // ── Toolbar ──
    let toolbar = commands
        .spawn((Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            padding: UiRect::vertical(Val::Px(4.0)),
            flex_shrink: 0.0,
            ..default()
        },))
        .id();

    let clear = text_button(commands, fonts, "trash", "Clear", ConsoleBtn::Clear);
    let copy = text_button(commands, fonts, "clipboard", "Copy", ConsoleBtn::Copy);
    let sep1 = vsep(commands);
    let t_info = toggle_button(commands, fonts, "info", ConsoleBtn::Info, 14.0);
    let t_ok = toggle_button(commands, fonts, "check-circle", ConsoleBtn::Success, 14.0);
    let t_warn = toggle_button(commands, fonts, "warning", ConsoleBtn::Warn, 14.0);
    let t_err = toggle_button(commands, fonts, "x-circle", ConsoleBtn::Error, 14.0);
    let sep2 = vsep(commands);
    let mag = icon_text(commands, &fonts.phosphor, "magnifying-glass", text_muted(), 12.0);
    // Fields start empty; `console_text_sync` mirrors them into `ConsoleState`.
    let search = text_input(commands, &fonts.ui, "Search...", "");
    commands.entity(search).insert(ConsoleField::Search);
    let funnel = icon_text(commands, &fonts.phosphor, "funnel", text_muted(), 12.0);
    let category = text_input(commands, &fonts.ui, "Category...", "");
    commands.entity(category).insert(ConsoleField::Category);

    let spacer = commands
        .spawn((Node {
            flex_grow: 1.0,
            ..default()
        },))
        .id();
    let count_label = label_text(commands, fonts, "0/0", text_muted(), 11.0);
    bind_text(commands, count_label, |w| {
        let Some(s) = w.get_resource::<ConsoleState>() else {
            return "0/0".to_string();
        };
        format!("{}/{}", s.filtered_entries().count(), s.entries.len())
    });
    let t_frame = toggle_button(commands, fonts, "hash", ConsoleBtn::Frame, 13.0);
    let t_ts = toggle_button(commands, fonts, "clock", ConsoleBtn::Timestamps, 13.0);
    let auto_icon = icon_text(commands, &fonts.phosphor, "check-circle", placeholder(), 13.0);
    bind_text_color(commands, auto_icon, |w| toggle_color(w, ConsoleBtn::AutoScroll));
    let auto = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(3.0),
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                flex_shrink: 0.0,
                ..default()
            },
            Interaction::default(),
            ConsoleBtn::AutoScroll,
            Name::new("console-autoscroll"),
        ))
        .id();
    let auto_label = label_text(commands, fonts, "Auto-scroll", text_muted(), 11.0);
    commands.entity(auto).add_children(&[auto_icon, auto_label]);

    commands.entity(toolbar).add_children(&[
        clear, copy, sep1, t_info, t_ok, t_warn, t_err, sep2, mag, search, funnel, category, spacer,
        count_label, t_frame, t_ts, auto,
    ]);

    // ── Category chips (reactive list) ──
    let chips_row = commands
        .spawn((Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(3.0),
            flex_shrink: 0.0,
            ..default()
        },))
        .id();
    keyed_list(commands, chips_row, chips_snapshot);

    let div1 = hsep(commands);

    // ── Log list (scrollable, flex-fills, reactive) ──
    let log_list = commands
        .spawn((Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            ..default()
        },))
        .id();
    keyed_list(commands, log_list, log_snapshot);
    let scroll = scroll_view_pinned(commands, log_list);

    let div2 = hsep(commands);

    // ── Input bar ──
    let input_row = commands
        .spawn((Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
            flex_shrink: 0.0,
            ..default()
        },))
        .id();
    let chevron = icon_text(commands, &fonts.phosphor, "caret-right", accent(), 14.0);
    let input = text_input(commands, &fonts.mono, "Type /help for commands...", "");
    commands.entity(input).insert(ConsoleField::Command);
    commands.entity(input).insert(Node {
        flex_grow: 1.0,
        padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
        align_items: AlignItems::Center,
        ..default()
    });
    let enter = icon_text(commands, &fonts.phosphor, "arrow-elbow-down-left", text_muted(), 12.0);
    commands.entity(input_row).add_children(&[chevron, input, enter]);

    commands
        .entity(root)
        .add_children(&[toolbar, chips_row, div1, scroll, div2, input_row]);

    root
}

/// Owned snapshot of one log entry's rendering inputs (so the keyed-list builder
/// can outlive the world borrow).
#[derive(Clone)]
struct RowData {
    level: LogLevel,
    category: String,
    message: String,
    timestamp: f64,
    frame: u64,
    show_ts: bool,
    show_frame: bool,
}

fn hash_row(r: &RowData) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (
        level_idx(r.level),
        &r.category,
        &r.message,
        r.timestamp.to_bits(),
        r.frame,
        r.show_ts,
        r.show_frame,
    )
        .hash(&mut h);
    h.finish()
}

fn build_log_row(commands: &mut Commands, fonts: &EmberFonts, r: &RowData) -> Entity {
    let color = level_color(r.level);
    let row = commands
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        },))
        .id();
    let mut kids: Vec<Entity> = Vec::new();

    if r.show_ts {
        let secs = r.timestamp;
        let mins = (secs / 60.0) as u64;
        let s = secs % 60.0;
        kids.push(mono_text(commands, fonts, &format!("{:02}:{:05.2}", mins, s), text_muted(), 10.0));
    }
    if r.show_frame && r.frame > 0 {
        kids.push(mono_text(commands, fonts, &format!("f{}", r.frame), text_muted(), 10.0));
    }
    kids.push(icon_text(commands, &fonts.phosphor, level_icon(r.level), color, 12.0));
    if !r.category.is_empty() {
        kids.push(label_text(commands, fonts, &format!("[{}]", r.category), accent(), 11.0));
    }
    let is_repl = r.category == "Input" || r.category == "Output";
    if is_repl {
        kids.push(mono_text(commands, fonts, &r.message, text_primary(), 12.0));
    } else {
        kids.push(label_text(commands, fonts, &r.message, text_primary(), 12.0));
    }

    commands.entity(row).add_children(&kids);
    row
}

/// Keyed-list snapshot for the log list: filtered entries (keyed by their
/// monotonic push index so appends/trims/filter-changes are granular), trimmed
/// to the last [`MAX_ROWS`], or a single "No log entries" placeholder.
fn log_snapshot(world: &World) -> KeyedSnapshot {
    let Some(state) = world.get_resource::<ConsoleState>() else {
        return KeyedSnapshot {
            items: Vec::new(),
            build: Box::new(|_, _, _| Entity::PLACEHOLDER),
        };
    };
    let show_ts = state.show_timestamps;
    let show_frame = state.show_frame;
    // Absolute (stable) index of `entries[0]`: total pushed minus what's retained.
    let base = state.pushed.saturating_sub(state.entries.len() as u64);
    let mut visible: Vec<(u64, RowData)> = state
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| passes(state, e))
        .map(|(i, e)| {
            (
                base + i as u64,
                RowData {
                    level: e.level,
                    category: e.category.clone(),
                    message: e.message.clone(),
                    timestamp: e.timestamp,
                    frame: e.frame,
                    show_ts,
                    show_frame,
                },
            )
        })
        .collect();
    let start = visible.len().saturating_sub(MAX_ROWS);
    let visible = visible.split_off(start);

    if visible.is_empty() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 0)],
            build: Box::new(|c, f, _| label_text(c, f, "No log entries", text_muted(), 13.0)),
        };
    }

    let items: Vec<(u64, u64)> = visible.iter().map(|(k, r)| (*k, hash_row(r))).collect();
    let rows: Vec<RowData> = visible.into_iter().map(|(_, r)| r).collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| build_log_row(c, f, &rows[i])),
    }
}

/// One chip in the category-filter list.
enum ChipItem {
    Tag,
    Chip { cat: String, hidden: bool },
}

fn build_chip(commands: &mut Commands, fonts: &EmberFonts, cat: &str, hidden: bool) -> Entity {
    let (fg, bg) = if hidden {
        (placeholder(), Color::NONE)
    } else {
        (text_muted(), rgb(section_bg()))
    };
    let chip = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(7.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(bg),
            Interaction::default(),
            ConsoleChip(cat.to_string()),
            Name::new("console-chip"),
        ))
        .id();
    let txt = commands
        .spawn((
            Text::new(cat),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(fg)),
        ))
        .id();
    commands.entity(chip).add_child(txt);
    chip
}

/// Keyed-list snapshot for the category chips (tag icon + one chip per seen
/// category, keyed by name, rebuilt only when its hidden state flips).
fn chips_snapshot(world: &World) -> KeyedSnapshot {
    let mut data: Vec<ChipItem> = Vec::new();
    let mut items: Vec<(u64, u64)> = Vec::new();
    if let Some(state) = world.get_resource::<ConsoleState>() {
        if !state.seen_categories.is_empty() {
            data.push(ChipItem::Tag);
            items.push((u64::MAX, 0));
            for cat in &state.seen_categories {
                let hidden = state.hidden_categories.contains(cat);
                let mut h = std::collections::hash_map::DefaultHasher::new();
                cat.hash(&mut h);
                // Odd, and strictly below the Tag's sentinel key.
                let key = (h.finish() >> 1) | 1;
                data.push(ChipItem::Chip {
                    cat: cat.clone(),
                    hidden,
                });
                items.push((key, hidden as u64));
            }
        }
    }
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| match &data[i] {
            ChipItem::Tag => icon_text(c, &f.phosphor, "tag", text_muted(), 11.0),
            ChipItem::Chip { cat, hidden } => build_chip(c, f, cat, *hidden),
        }),
    }
}

// ── Registration ────────────────────────────────────────────────────────────

/// Wire the bevy-native console into the editor (a single call from `ConsolePlugin`).
pub fn register_native_console(app: &mut App) {
    use renzora::SplashState;
    // `scroll = false`: the console manages its own internal log scroll.
    app.register_panel_content(PANEL_ID, false, build_console);
    app.add_systems(
        Update,
        (console_buttons, console_chips, console_text_sync).run_if(in_state(SplashState::Editor)),
    );
}

// ── Systems ───────────────────────────────────────────────────────────────

/// Toolbar button clicks → mutate `ConsoleState`.
pub(crate) fn console_buttons(
    q: Query<(&Interaction, &ConsoleBtn), Changed<Interaction>>,
    mut state: ResMut<ConsoleState>,
) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match btn {
            ConsoleBtn::Clear => state.clear(),
            ConsoleBtn::Copy => {}
            ConsoleBtn::Info => state.show_info = !state.show_info,
            ConsoleBtn::Success => state.show_success = !state.show_success,
            ConsoleBtn::Warn => state.show_warnings = !state.show_warnings,
            ConsoleBtn::Error => state.show_errors = !state.show_errors,
            ConsoleBtn::Timestamps => state.show_timestamps = !state.show_timestamps,
            ConsoleBtn::Frame => state.show_frame = !state.show_frame,
            ConsoleBtn::AutoScroll => state.auto_scroll = !state.auto_scroll,
        }
    }
}

/// Category chip clicks → toggle that category's hidden state.
pub(crate) fn console_chips(
    q: Query<(&Interaction, &ConsoleChip), Changed<Interaction>>,
    mut state: ResMut<ConsoleState>,
) {
    for (interaction, chip) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if state.hidden_categories.contains(&chip.0) {
            state.hidden_categories.remove(&chip.0);
        } else {
            state.hidden_categories.insert(chip.0.clone());
        }
    }
}

/// Mirror the search/category text fields into `ConsoleState` (guarded), and
/// submit the command field on Enter.
pub(crate) fn console_text_sync(
    mut q: Query<(&mut EmberTextInput, &ConsoleField)>,
    mut state: ResMut<ConsoleState>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
) {
    for (mut inp, field) in &mut q {
        match field {
            ConsoleField::Search => {
                if state.search_filter != inp.value {
                    state.search_filter = inp.value.clone();
                }
            }
            ConsoleField::Category => {
                if state.category_filter != inp.value {
                    state.category_filter = inp.value.clone();
                }
            }
            ConsoleField::Command => {
                if inp.value.contains('\n') {
                    let command = inp.value.split('\n').next().unwrap_or("").trim().to_string();
                    let (text_e, ph) = (inp.text_entity, inp.placeholder.clone());
                    inp.value.clear();
                    if let Ok((mut t, mut c)) = texts.get_mut(text_e) {
                        *t = Text::new(ph);
                        c.0 = rgb(text_muted());
                    }
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
            let help = "Available commands:\n  /clear   Clear console output\n  /help    List all commands";
            state.log(LogLevel::Success, "Command", help);
        }
        other => state.log(
            LogLevel::Error,
            "Command",
            format!("Unknown command: /{}. Type /help for a list of commands.", other),
        ),
    }
}
