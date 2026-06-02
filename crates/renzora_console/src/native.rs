//! Bevy-native (ember) console panel — a faithful bevy_ui port of the egui
//! `render_console_content`: toolbar (clear/copy, level filters, search/category,
//! count, timestamp/frame/auto-scroll toggles), category chips, the log list, and
//! the command input bar.
//!
//! Reactivity is Bevy change detection, not egui's per-frame redraw, and the log
//! list is **append-only**: the shell builds once into the leaf's `content`
//! entity, then each frame only the *new* entries (`ConsoleState::pushed` advanced)
//! are appended as rows and the front is trimmed past [`MAX_ROWS`]. A full rebuild
//! happens only when the filter set changes. (The egui panel virtualized to
//! visible rows; rebuilding the whole list every frame here would thrash layout.)
//! Toolbar controls write straight back to `ConsoleState` — no egui `Arc<Mutex>`.

use bevy::prelude::*;
use bevy::ui::ScrollPosition;

use renzora_ember::dock::{DockLeaf, PanelContent};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::theme::{
    rgb, ACCENT_BLUE, CLOSE_RED, PLACEHOLDER, PLAY_GREEN, TEXT_MUTED, TEXT_PRIMARY, WARN_AMBER,
};
use renzora_ember::widgets::{scroll_view, text_input, EmberTextInput};

use crate::state::{ConsoleState, LogEntry, LogLevel};

const PANEL_ID: &str = "console";
/// Max log rows kept in the DOM at once (older ones are trimmed from the front).
const MAX_ROWS: usize = 400;
const SEP: (u8, u8, u8) = (60, 60, 72);
const BTN_BG: (u8, u8, u8) = (42, 42, 52);
const CHIP_BG: (u8, u8, u8) = (60, 60, 60);
const HYPERLINK: (u8, u8, u8) = ACCENT_BLUE;
const DIM: (u8, u8, u8) = (120, 120, 120);

fn level_color(level: LogLevel) -> (u8, u8, u8) {
    match level {
        LogLevel::Info => ACCENT_BLUE,
        LogLevel::Success => PLAY_GREEN,
        LogLevel::Warning => WARN_AMBER,
        LogLevel::Error => CLOSE_RED,
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

/// On the leaf's `content` entity: references to the sub-parts the systems
/// update, the last-rendered filter signature, the append cursor, and the
/// current real-row count.
#[derive(Component)]
pub(crate) struct ConsoleView {
    log_list: Entity,
    scroll: Entity,
    count_label: Entity,
    chips_row: Entity,
    filter: FilterSig,
    rendered_pushed: u64,
    rows: usize,
}

/// Everything that forces a *full* log-list rebuild when it changes (level
/// filters, search/category text, hidden/seen categories, timestamp/frame
/// columns). New entries alone never change this — they're appended.
#[derive(Clone, PartialEq, Default)]
struct FilterSig {
    show_info: bool,
    show_success: bool,
    show_warnings: bool,
    show_errors: bool,
    search: String,
    category: String,
    hidden: usize,
    seen: usize,
    show_timestamps: bool,
    show_frame: bool,
}

impl FilterSig {
    fn of(s: &ConsoleState) -> Self {
        Self {
            show_info: s.show_info,
            show_success: s.show_success,
            show_warnings: s.show_warnings,
            show_errors: s.show_errors,
            search: s.search_filter.clone(),
            category: s.category_filter.clone(),
            hidden: s.hidden_categories.len(),
            seen: s.seen_categories.len(),
            show_timestamps: s.show_timestamps,
            show_frame: s.show_frame,
        }
    }
}

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

/// A toolbar button whose icon recolors with the toggle's on/off state.
#[derive(Component)]
pub(crate) struct ConsoleToggle {
    icon: Entity,
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
            BackgroundColor(rgb(SEP)),
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
            BackgroundColor(rgb(SEP)),
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
            BackgroundColor(rgb(BTN_BG)),
            Interaction::default(),
            btn,
            Name::new(format!("console-btn:{label}")),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, TEXT_MUTED, 12.0);
    let tx = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(TEXT_MUTED)),
        ))
        .id();
    commands.entity(b).add_children(&[ic, tx]);
    b
}

/// An icon-only toggle button whose glyph recolors with `on`/`off`.
fn toggle_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str, btn: ConsoleBtn, size: f32) -> Entity {
    let ic = icon_text(commands, &fonts.phosphor, icon, PLACEHOLDER, size);
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
            ConsoleToggle { icon: ic },
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

/// Build the whole console into a root node; returns `(root, ConsoleView)`.
fn build_console(commands: &mut Commands, fonts: &EmberFonts, state: &ConsoleState) -> (Entity, ConsoleView) {
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
    let mag = icon_text(commands, &fonts.phosphor, "magnifying-glass", TEXT_MUTED, 12.0);
    let search = text_input(commands, &fonts.ui, "Search...", &state.search_filter);
    commands.entity(search).insert(ConsoleField::Search);
    let funnel = icon_text(commands, &fonts.phosphor, "funnel", TEXT_MUTED, 12.0);
    let category = text_input(commands, &fonts.ui, "Category...", &state.category_filter);
    commands.entity(category).insert(ConsoleField::Category);

    let spacer = commands
        .spawn((Node {
            flex_grow: 1.0,
            ..default()
        },))
        .id();
    let count_label = label_text(commands, fonts, "0/0", TEXT_MUTED, 11.0);
    let t_frame = toggle_button(commands, fonts, "hash", ConsoleBtn::Frame, 13.0);
    let t_ts = toggle_button(commands, fonts, "clock", ConsoleBtn::Timestamps, 13.0);
    let auto_icon = icon_text(commands, &fonts.phosphor, "check-circle", PLACEHOLDER, 13.0);
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
            ConsoleToggle { icon: auto_icon },
            Name::new("console-autoscroll"),
        ))
        .id();
    let auto_label = label_text(commands, fonts, "Auto-scroll", TEXT_MUTED, 11.0);
    commands.entity(auto).add_children(&[auto_icon, auto_label]);

    commands.entity(toolbar).add_children(&[
        clear, copy, sep1, t_info, t_ok, t_warn, t_err, sep2, mag, search, funnel, category, spacer,
        count_label, t_frame, t_ts, auto,
    ]);

    // ── Category chips (filled by refresh) ──
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

    let div1 = hsep(commands);

    // ── Log list (scrollable, flex-fills) ──
    let log_list = commands
        .spawn((Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            ..default()
        },))
        .id();
    let scroll = scroll_view(commands, log_list);

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
    let chevron = icon_text(commands, &fonts.phosphor, "caret-right", ACCENT_BLUE, 14.0);
    let input = text_input(commands, &fonts.mono, "Type /help for commands...", "");
    commands.entity(input).insert(ConsoleField::Command);
    commands.entity(input).insert(Node {
        flex_grow: 1.0,
        padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
        align_items: AlignItems::Center,
        ..default()
    });
    let enter = icon_text(commands, &fonts.phosphor, "arrow-elbow-down-left", TEXT_MUTED, 12.0);
    commands.entity(input_row).add_children(&[chevron, input, enter]);

    commands
        .entity(root)
        .add_children(&[toolbar, chips_row, div1, scroll, div2, input_row]);

    let view = ConsoleView {
        log_list,
        scroll,
        count_label,
        chips_row,
        filter: FilterSig::default(),
        rendered_pushed: 0,
        rows: 0,
    };
    (root, view)
}

fn build_log_row(commands: &mut Commands, fonts: &EmberFonts, entry: &LogEntry, state: &ConsoleState) -> Entity {
    let color = level_color(entry.level);
    let row = commands
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        },))
        .id();
    let mut kids: Vec<Entity> = Vec::new();

    if state.show_timestamps {
        let secs = entry.timestamp;
        let mins = (secs / 60.0) as u64;
        let s = secs % 60.0;
        kids.push(mono_text(commands, fonts, &format!("{:02}:{:05.2}", mins, s), DIM, 10.0));
    }
    if state.show_frame && entry.frame > 0 {
        kids.push(mono_text(commands, fonts, &format!("f{}", entry.frame), DIM, 10.0));
    }
    kids.push(icon_text(commands, &fonts.phosphor, level_icon(entry.level), color, 12.0));
    if !entry.category.is_empty() {
        kids.push(label_text(commands, fonts, &format!("[{}]", entry.category), HYPERLINK, 11.0));
    }
    let is_repl = entry.category == "Input" || entry.category == "Output";
    if is_repl {
        kids.push(mono_text(commands, fonts, &entry.message, TEXT_PRIMARY, 12.0));
    } else {
        kids.push(label_text(commands, fonts, &entry.message, TEXT_PRIMARY, 12.0));
    }

    commands.entity(row).add_children(&kids);
    row
}

fn rebuild_chips(commands: &mut Commands, fonts: &EmberFonts, state: &ConsoleState, chips_row: Entity, children: &Query<&Children>) {
    if let Ok(kids) = children.get(chips_row) {
        for k in kids.iter() {
            commands.entity(k).despawn();
        }
    }
    if state.seen_categories.is_empty() {
        return;
    }
    let mut chips: Vec<Entity> = vec![icon_text(commands, &fonts.phosphor, "tag", TEXT_MUTED, 11.0)];
    for cat in &state.seen_categories {
        let hidden = state.hidden_categories.contains(cat);
        let (fg, bg) = if hidden {
            (PLACEHOLDER, Color::NONE)
        } else {
            (TEXT_MUTED, rgb(CHIP_BG))
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
                ConsoleChip(cat.clone()),
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
        chips.push(chip);
    }
    commands.entity(chips_row).add_children(&chips);
}

// ── Registration ────────────────────────────────────────────────────────────

/// Wire the bevy-native console into the editor (a single call from `ConsolePlugin`).
pub fn register_native_console(app: &mut App) {
    use renzora::NativePanelExt;
    use renzora_editor::SplashState;
    app.register_native_panel(PANEL_ID);
    app.add_systems(
        Update,
        (
            // Chained so the build's commands (despawn old subtree + insert the
            // fresh ConsoleView) apply via an auto-inserted sync point BEFORE the
            // refresh runs — otherwise the refresh can append to a log_list that
            // the build just despawned and panic at apply time.
            (console_content_system, console_log_refresh).chain(),
            console_buttons,
            console_toolbar_sync,
            console_chips,
            console_text_sync,
        )
            .run_if(in_state(SplashState::Editor)),
    );
}

// ── Systems ───────────────────────────────────────────────────────────────

/// Build the console shell once into the active leaf's `content` entity.
pub(crate) fn console_content_system(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    state: Res<ConsoleState>,
    leaves: Query<&DockLeaf>,
    content_q: Query<(Option<&PanelContent>, Option<&Children>)>,
) {
    let Some(fonts) = fonts else {
        return;
    };
    for leaf in &leaves {
        if leaf.active != PANEL_ID {
            continue;
        }
        let Ok((pc, children)) = content_q.get(leaf.content) else {
            continue;
        };
        if pc.map(|p| p.0.as_str()) == Some(PANEL_ID) {
            continue;
        }
        if let Some(kids) = children {
            for k in kids.iter() {
                commands.entity(k).despawn();
            }
        }
        let (root, view) = build_console(&mut commands, &fonts, &state);
        commands.entity(leaf.content).add_child(root);
        commands
            .entity(leaf.content)
            .insert((PanelContent(PANEL_ID.to_string()), view));
    }
}

/// Keep the log list in sync: full rebuild on filter change, otherwise append
/// only the new entries and trim the front past [`MAX_ROWS`].
pub(crate) fn console_log_refresh(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    state: Res<ConsoleState>,
    mut views: Query<&mut ConsoleView>,
    children: Query<&Children>,
    mut texts: Query<&mut Text>,
    mut scrolls: Query<&mut ScrollPosition>,
) {
    let Some(fonts) = fonts else {
        return;
    };
    let fsig = FilterSig::of(&state);
    for mut view in &mut views {
        // The log_list handle can go stale if a dock rebuild despawned and reused
        // its slot — skip rather than despawn/parent a now-different entity.
        if commands.get_entity(view.log_list).is_err() {
            continue;
        }
        let filter_changed = view.filter != fsig;
        // `pushed` ran backwards → the log was cleared; rebuild from scratch.
        let reset = view.rendered_pushed > state.pushed;
        let has_new = state.pushed > view.rendered_pushed;
        if !filter_changed && !reset && !has_new {
            continue;
        }

        if filter_changed || reset {
            view.filter = fsig.clone();
            if let Ok(kids) = children.get(view.log_list) {
                for k in kids.iter() {
                    commands.entity(k).despawn();
                }
            }
            let filtered: Vec<&LogEntry> = state.filtered_entries().collect();
            let start = filtered.len().saturating_sub(MAX_ROWS);
            if filtered.is_empty() {
                let empty = label_text(&mut commands, &fonts, "No log entries", TEXT_MUTED, 13.0);
                commands.entity(view.log_list).add_child(empty);
                view.rows = 0;
            } else {
                let rows: Vec<Entity> = filtered[start..]
                    .iter()
                    .map(|e| build_log_row(&mut commands, &fonts, e, &state))
                    .collect();
                view.rows = rows.len();
                commands.entity(view.log_list).add_children(&rows);
            }
            rebuild_chips(&mut commands, &fonts, &state, view.chips_row, &children);
        } else {
            // Append only the entries pushed since last frame (filtered).
            let new = (state.pushed - view.rendered_pushed).min(state.entries.len() as u64) as usize;
            let skip = state.entries.len() - new;
            let rows: Vec<Entity> = state
                .entries
                .iter()
                .skip(skip)
                .filter(|e| passes(&state, e))
                .map(|e| build_log_row(&mut commands, &fonts, e, &state))
                .collect();
            if !rows.is_empty() {
                if view.rows == 0 {
                    // Clear the "No log entries" placeholder.
                    if let Ok(kids) = children.get(view.log_list) {
                        for k in kids.iter() {
                            commands.entity(k).despawn();
                        }
                    }
                }
                let total = view.rows + rows.len();
                if total > MAX_ROWS {
                    let excess = total - MAX_ROWS;
                    if let Ok(kids) = children.get(view.log_list) {
                        for k in kids.iter().take(excess) {
                            commands.entity(k).despawn();
                        }
                    }
                    view.rows = MAX_ROWS;
                } else {
                    view.rows = total;
                }
                commands.entity(view.log_list).add_children(&rows);
            }
        }

        view.rendered_pushed = state.pushed;

        if let Ok(mut t) = texts.get_mut(view.count_label) {
            let filtered_count = state.filtered_entries().count();
            *t = Text::new(format!("{}/{}", filtered_count, state.entries.len()));
        }
        if state.auto_scroll {
            if let Ok(mut sp) = scrolls.get_mut(view.scroll) {
                sp.y = 1.0e6;
            }
        }
    }
}

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

/// Recolor toggle icons from the current flags (guarded so it never churns).
pub(crate) fn console_toolbar_sync(
    state: Res<ConsoleState>,
    toggles: Query<(&ConsoleBtn, &ConsoleToggle)>,
    mut colors: Query<&mut TextColor>,
) {
    for (btn, tog) in &toggles {
        let (active, on) = match btn {
            ConsoleBtn::Info => (state.show_info, ACCENT_BLUE),
            ConsoleBtn::Success => (state.show_success, PLAY_GREEN),
            ConsoleBtn::Warn => (state.show_warnings, WARN_AMBER),
            ConsoleBtn::Error => (state.show_errors, CLOSE_RED),
            ConsoleBtn::Timestamps => (state.show_timestamps, ACCENT_BLUE),
            ConsoleBtn::Frame => (state.show_frame, ACCENT_BLUE),
            ConsoleBtn::AutoScroll => (state.auto_scroll, ACCENT_BLUE),
            _ => continue,
        };
        let target = rgb(if active { on } else { PLACEHOLDER });
        if let Ok(mut c) = colors.get_mut(tog.icon) {
            if c.0 != target {
                c.0 = target;
            }
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
                        c.0 = rgb(TEXT_MUTED);
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
