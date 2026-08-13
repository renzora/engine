//! The floating tutorial card: its shell, the chapter picker, and the per-step
//! body.
//!
//! This is deliberately NOT ember's `overlay()` modal: that draws a full-screen,
//! click-blocking, auto-dismissing backdrop — fatal for a tutorial whose steps
//! require clicking the viewport. Instead this is a small, non-blocking card
//! pinned to the bottom-right. It carries `OverlaySurface` (so scroll/clicks over
//! the card itself don't bleed to panels behind it) but covers none of the
//! viewport, leaving the 3D scene fully interactive.
//!
//! The whole header is a drag handle (ember's `DragHandle` widget, which moves a
//! *target* rather than itself — see `renzora_ember::widgets::drag_grip`), and
//! the body scrolls inside a capped card height so a long chapter list can't push
//! Skip off the top of the window.

use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};

use renzora::core::{EditorLocked, HideInHierarchy};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::theme::{
    accent, border, header_bg, play_green, rgb, text_muted, text_primary, window_bg,
};
use renzora_ember::widgets::{rich_text_sized, scroll_area, DragHandle, OverlaySurface};

/// How tall the card's body may get before it scrolls. Sized so the tallest
/// step still fits without a bar, and the seven-chapter picker scrolls.
const BODY_MAX_H: f32 = 300.0;

use crate::hints;
use crate::state::TutorialState;
use crate::steps::{StepKind, CHAPTERS};

/// The "Skip" button — moves past the current step without doing it. Hidden on
/// the picker and the completion card, where there's no step to skip.
#[derive(Component)]
pub struct TutorialSkipButton;

/// The header's X — closes the tutorial entirely.
#[derive(Component)]
pub struct TutorialCloseButton;

/// The "Finish" button on a chapter's completion card.
#[derive(Component)]
pub struct TutorialFinishButton;

/// The green "Continue" button, shown once the current step's action is done.
#[derive(Component)]
pub struct TutorialContinueButton;

/// A chapter row on the picker; payload is the index into [`CHAPTERS`].
#[derive(Component)]
pub struct TutorialChapterButton(pub usize);

/// The progress-bar fill node (width is set to the % of steps done).
#[derive(Component)]
pub struct TutorialProgressFill;

/// The card's title text, retargeted per chapter.
#[derive(Component)]
pub struct TutorialCardTitle;

/// Entities the state machine needs to keep referring to after build.
pub struct OverlayEntities {
    pub root: Entity,
    pub body: Entity,
    pub fill: Entity,
}

/// Build the persistent card shell (header + progress bar + empty body). The body
/// is filled per-step by [`build_body`].
pub fn build_overlay(commands: &mut Commands, fonts: &EmberFonts) -> OverlayEntities {
    let card = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(18.0),
                bottom: Val::Px(18.0),
                width: Val::Px(320.0),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::clip(),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(9.0)),
                ..default()
            },
            // The card sits on top of panels that are themselves `popup_bg`, so
            // it takes the darker `window_bg` and an accent border to read as a
            // distinct surface rather than another panel.
            BackgroundColor(rgb(window_bg())),
            BorderColor::all(rgb(accent())),
            GlobalZIndex(8100),
            // Block on the card itself (so its own clicks/scroll don't bleed),
            // but the card covers none of the viewport.
            FocusPolicy::Block,
            RelativeCursorPosition::default(),
            OverlaySurface,
            HideInHierarchy,
            EditorLocked,
            Name::new("tutorial-card"),
        ))
        .id();

    // ── Header: grip + graduation-cap + title + spacer + Skip ──
    //
    // The whole strip is the drag handle, not just the grip — a 16px glyph is a
    // fussy target for something you reach for precisely when the card is in
    // your way. Skip and the X therefore have to *capture* their press, which
    // is what the `FocusPolicy::Block` below buys: since Bevy 0.19, `Node`
    // requires `FocusPolicy` and its default is `Pass`, so an unmarked button
    // hands its press down to every node behind it — here the header — and
    // pressing Skip would start dragging the card as well.
    let header = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(7.0),
                padding: UiRect::axes(Val::Px(11.0), Val::Px(8.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(header_bg())),
            Interaction::default(),
            DragHandle::new(card),
            Name::new("tutorial-card-header"),
        ))
        .id();
    let grip = icon_text(commands, &fonts.phosphor, "dots-six-vertical", text_muted(), 13.0);
    let cap = icon_text(commands, &fonts.phosphor, "graduation-cap", accent(), 15.0);
    let title = commands
        .spawn((
            Text::new(CHAPTERS[0].title),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_primary())),
            TutorialCardTitle,
        ))
        .id();
    let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    // Skip moves past the *current step* — for the one you can't do right now
    // (no model to import, no marketplace account). Closing the whole tutorial
    // is the X beside it, so neither action can be taken for the other.
    let skip = small_button(commands, fonts, "Skip", ButtonStyle::Outline);
    commands.entity(skip).insert((TutorialSkipButton, FocusPolicy::Block));
    let close = icon_button(commands, fonts, "x");
    commands.entity(close).insert((TutorialCloseButton, FocusPolicy::Block));
    commands
        .entity(header)
        .add_children(&[grip, cap, title, spacer, skip, close]);

    // ── Progress bar ──
    let track = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(3.0),
                flex_shrink: 0.0,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(header_bg())),
        ))
        .id();
    let fill = commands
        .spawn((
            Node {
                width: Val::Percent(0.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(rgb(accent())),
            TutorialProgressFill,
        ))
        .id();
    commands.entity(track).add_child(fill);

    // ── Body (refilled each step), inside a scroller ──
    let body = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(7.0),
            padding: UiRect::axes(Val::Px(11.0), Val::Px(10.0)),
            ..default()
        })
        .id();
    // `scroll_area` (content-height, capped) rather than the keyed/flex variants:
    // those are `flex_grow: 1; flex_basis: 0`, which contributes nothing to an
    // auto-height parent — the card would collapse to just its header. Capping
    // here also keeps the card exactly as tall as the step needs, and only the
    // long chapter list actually scrolls. Not keyed on purpose: each step should
    // open at the top, not wherever the previous one was scrolled to.
    let scroller = scroll_area(commands, body, BODY_MAX_H);

    commands.entity(card).add_children(&[header, track, scroller]);
    OverlayEntities { root: card, body, fill }
}

/// Fill `body` with whatever the tutorial should be showing: the chapter picker,
/// the current step, or the chapter-complete card. The caller clears `body`'s
/// previous children first.
///
/// `done` carries one flag per [`CHAPTERS`] entry — has this project already
/// finished that chapter — which the picker renders as a tick.
pub fn build_body(
    commands: &mut Commands,
    fonts: &EmberFonts,
    body: Entity,
    state: &TutorialState,
    done: &[bool],
) {
    if state.show_picker {
        build_picker(commands, fonts, body, done);
    } else if let Some(step) = state.step() {
        build_step(commands, fonts, body, state, step);
    } else {
        build_complete(commands, fonts, body, state);
    }
}

/// Is chapter `i` unlocked? The first always is; every other one opens when its
/// predecessor is finished.
///
/// Sequential on purpose: the chapters build on each other (Scripting assumes you
/// can select an entity; Materials assumes you can find a panel), and a plain
/// list of seven gives a new user no idea which to open. Unlocking one at a time
/// makes the order the tutorial's own answer to that.
pub fn is_unlocked(index: usize, done: &[bool]) -> bool {
    index == 0 || done.get(index - 1).copied().unwrap_or(false)
}

/// The chapter list — one row per [`CHAPTERS`] entry: finished ones ticked,
/// locked ones dimmed and inert.
fn build_picker(commands: &mut Commands, fonts: &EmberFonts, body: Entity, done: &[bool]) {
    let cleared = done.iter().filter(|d| **d).count();
    let title = commands
        .spawn((
            Text::new("Chapters"),
            ui_font(&fonts.ui, 15.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let blurb = commands
        .spawn((
            Text::new(format!(
                "{cleared} of {} complete. Finish one to unlock the next.",
                CHAPTERS.len()
            )),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
            Node { max_width: Val::Px(276.0), ..default() },
        ))
        .id();
    commands.entity(body).add_children(&[title, blurb]);

    for (i, chapter) in CHAPTERS.iter().enumerate() {
        let finished = done.get(i).copied().unwrap_or(false);
        let unlocked = is_unlocked(i, done);
        let row = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(10.0),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(7.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                BorderColor::all(rgb(border())),
                Interaction::default(),
                TutorialChapterButton(i),
            ))
            .id();
        // Three states, three glyphs: locked (padlock), cleared (green tick),
        // available (the chapter's own icon).
        let (glyph, tint) = if !unlocked {
            ("lock-simple", text_muted())
        } else if finished {
            ("check-circle", play_green())
        } else {
            (chapter.icon, accent())
        };
        let icon = icon_text(commands, &fonts.phosphor, glyph, tint, 18.0);
        let text_col = commands
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            })
            .id();
        let name = commands
            .spawn((
                Text::new(chapter.title),
                ui_font(&fonts.ui, 13.0),
                // A locked row is greyed rather than hidden — seeing what's ahead
                // is half the point of a chapter list.
                TextColor(rgb(if unlocked { text_primary() } else { text_muted() })),
            ))
            .id();
        let summary = commands
            .spawn((
                Text::new(if unlocked {
                    chapter.summary.to_string()
                } else {
                    format!("Finish {} first", CHAPTERS[i - 1].title)
                }),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(text_muted())),
                Node { max_width: Val::Px(214.0), ..default() },
            ))
            .id();
        commands.entity(text_col).add_children(&[name, summary]);
        commands.entity(row).add_children(&[icon, text_col]);
        commands.entity(body).add_child(row);
    }
}

/// One step: badge row, title, instructions, animated hint, and — once the
/// action is done — the green Continue button.
fn build_step(
    commands: &mut Commands,
    fonts: &EmberFonts,
    body: Entity,
    state: &TutorialState,
    step: &'static crate::steps::Step,
) {
    let total = state.steps().len();
    // An `Info` step is arm-on-arrival — it never "succeeds", it's just read, so
    // it keeps its instructions right up to Continue.
    let succeeded = state.step_done && !matches!(step.kind, StepKind::Info);

    let badge_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let (badge_glyph, badge_tint) = if succeeded {
        ("check-circle", play_green())
    } else {
        (step.badge, accent())
    };
    let badge = icon_text(commands, &fonts.phosphor, badge_glyph, badge_tint, 17.0);
    let counter = commands
        .spawn((
            Text::new(format!("STEP {} OF {}", state.current + 1, total)),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(badge_row).add_children(&[badge, counter]);

    let title = commands
        .spawn((
            Text::new(step.title),
            ui_font(&fonts.ui, 15.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    commands.entity(body).add_children(&[badge_row, title]);

    if succeeded {
        // Instructions and the input hint have done their job — leaving them up
        // reads as "still to do", which is exactly what the user just finished.
        // They're replaced by the confirmation, not stacked under it.
        let msg = commands
            .spawn((
                Text::new(success_line(state.current)),
                ui_font(&fonts.ui, 12.0),
                TextColor(rgb(play_green())),
                Node { max_width: Val::Px(276.0), ..default() },
            ))
            .id();
        commands.entity(body).add_child(msg);
    } else {
        let runs = keyword_runs(step.body);
        let body_text = rich_text_sized(commands, &fonts.ui, &runs, 12.0);
        commands
            .entity(body_text)
            .insert(Node { max_width: Val::Px(276.0), ..default() });
        commands.entity(body).add_child(body_text);
        // An `Info` step has no gesture to illustrate, so drawing an empty hint
        // row would just add a gap.
        if !step.hint.icons.is_empty() || !step.hint.keys.is_empty() {
            hints::build_hint(commands, fonts, body, &step.hint);
        }
    }

    if state.step_done {
        let row = commands
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                margin: UiRect::top(Val::Px(2.0)),
                ..default()
            })
            .id();
        let btn = small_button(commands, fonts, "Continue", ButtonStyle::Green);
        commands.entity(btn).insert(TutorialContinueButton);
        commands.entity(row).add_child(btn);
        commands.entity(body).add_child(row);
    }
}

/// Split a step body into coloured runs, so the names of things the user has to
/// find on screen stand out from the prose around them.
///
/// The markup is `*asterisks*`, and it only ever produces two colours — accent
/// for the marked runs, muted for everything else. Keeping it to one distinction
/// is the point: a body where four different things are emphasised emphasises
/// nothing, so the marks go on the literal UI labels ("*Move Speed*",
/// "*Add Entity*") and nothing else. An unclosed `*` just renders literally.
fn keyword_runs(body: &str) -> Vec<(&str, (u8, u8, u8))> {
    let mut runs = Vec::new();
    let mut rest = body;
    while let Some(open) = rest.find('*') {
        let Some(close) = rest[open + 1..].find('*') else {
            break; // unmatched — the remainder is plain text
        };
        let close = open + 1 + close;
        if open > 0 {
            runs.push((&rest[..open], text_muted()));
        }
        runs.push((&rest[open + 1..close], accent()));
        rest = &rest[close + 1..];
    }
    if !rest.is_empty() {
        runs.push((rest, text_muted()));
    }
    runs
}

/// Varied praise, picked by step index so consecutive steps don't repeat and the
/// same step always says the same thing (no reshuffling on a card rebuild).
fn success_line(step_index: usize) -> &'static str {
    const LINES: &[&str] = &[
        "Nice — that's it.",
        "Got it. That's the one.",
        "Perfect.",
        "That's exactly right.",
        "Done — nicely handled.",
    ];
    LINES[step_index % LINES.len()]
}

/// The chapter-complete card. Finish returns to the picker (see
/// `state::handle_buttons`), so the label promises exactly that.
fn build_complete(
    commands: &mut Commands,
    fonts: &EmberFonts,
    body: Entity,
    state: &TutorialState,
) {
    let chapter = &CHAPTERS[state.chapter.min(CHAPTERS.len() - 1)];
    let icon = icon_text(commands, &fonts.phosphor, "confetti", play_green(), 32.0);
    let title = commands
        .spawn((
            Text::new(format!("{} — done!", chapter.title)),
            ui_font(&fonts.ui, 15.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let msg = commands
        .spawn((
            Text::new(chapter.outro),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
            Node { max_width: Val::Px(276.0), ..default() },
        ))
        .id();
    let finish = small_button(commands, fonts, "Back to chapters", ButtonStyle::Green);
    commands.entity(finish).insert(TutorialFinishButton);
    commands.entity(body).add_children(&[icon, title, msg, finish]);
}

/// How a [`small_button`] is painted.
enum ButtonStyle {
    /// Subtle outline — secondary actions (Skip).
    Outline,
    /// Green fill — "go on, you've earned it" (Continue / Finish).
    Green,
}

/// A borderless square icon button — the header's X.
fn icon_button(commands: &mut Commands, fonts: &EmberFonts, glyph: &str) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(20.0),
                height: Val::Px(20.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
        ))
        .id();
    let icon = icon_text(commands, &fonts.phosphor, glyph, text_muted(), 13.0);
    commands.entity(btn).add_child(icon);
    btn
}

/// A compact button. Carries `Interaction` so a marker + change-detection query
/// can read presses.
fn small_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    style: ButtonStyle,
) -> Entity {
    let (bg, fg, bc) = match style {
        ButtonStyle::Outline => (Color::NONE, rgb(text_muted()), rgb(border())),
        ButtonStyle::Green => (rgb(play_green()), Color::WHITE, rgb(play_green())),
    };
    let btn = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(7.0)),
                border: UiRect::all(Val::Px(1.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                align_self: AlignSelf::FlexStart,
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(bg),
            BorderColor::all(bc),
            Interaction::default(),
        ))
        .id();
    let t = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 13.0),
            TextColor(fg),
        ))
        .id();
    commands.entity(btn).add_child(t);
    btn
}
