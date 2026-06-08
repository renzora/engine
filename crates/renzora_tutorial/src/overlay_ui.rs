//! The floating tutorial card and its per-step body.
//!
//! This is deliberately NOT ember's `overlay()` modal: that draws a full-screen,
//! click-blocking, auto-dismissing backdrop — fatal for a tutorial whose steps
//! require clicking the viewport. Instead this is a small, non-blocking card
//! pinned to the bottom-right. It carries `OverlaySurface` (so scroll/clicks over
//! the card itself don't bleed to panels behind it) but covers none of the
//! viewport, leaving the 3D scene fully interactive.

use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};

use renzora::core::{EditorLocked, HideInHierarchy};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::theme::{
    accent, border, header_bg, play_green, popup_bg, rgb, text_muted, text_primary,
};
use renzora_ember::widgets::OverlaySurface;

use crate::hints;
use crate::steps::STEPS;

/// The "Skip tutorial" button.
#[derive(Component)]
pub struct TutorialSkipButton;

/// The "Finish" button on the completion card.
#[derive(Component)]
pub struct TutorialFinishButton;

/// The progress-bar fill node (width is set to the % of steps done).
#[derive(Component)]
pub struct TutorialProgressFill;

/// Entities the state machine needs to keep referring to after build.
pub struct OverlayEntities {
    pub root: Entity,
    pub body: Entity,
    pub fill: Entity,
}

/// Build the persistent card shell (header + progress bar + empty body). The body
/// is filled per-step by [`build_step_body`].
pub fn build_overlay(commands: &mut Commands, fonts: &EmberFonts) -> OverlayEntities {
    let card = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(22.0),
                bottom: Val::Px(22.0),
                width: Val::Px(372.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(12.0),
                padding: UiRect::all(Val::Px(16.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
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

    // ── Header: graduation-cap + title + spacer + Skip ──
    let header = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let cap = icon_text(commands, &fonts.phosphor, "graduation-cap", accent(), 18.0);
    let title = commands
        .spawn((
            Text::new("Getting Started"),
            ui_font(&fonts.ui, 15.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let skip = small_button(commands, fonts, "Skip", false);
    commands.entity(skip).insert(TutorialSkipButton);
    commands.entity(header).add_children(&[cap, title, spacer, skip]);

    // ── Progress bar ──
    let track = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(6.0),
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(Val::Px(3.0)),
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
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(accent())),
            TutorialProgressFill,
        ))
        .id();
    commands.entity(track).add_child(fill);

    // ── Body (refilled each step) ──
    let body = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();

    commands.entity(card).add_children(&[header, track, body]);
    OverlayEntities { root: card, body, fill }
}

/// Fill `body` with the content for step `current` (or the completion card when
/// `current == total`). The caller clears `body`'s previous children first.
pub fn build_step_body(
    commands: &mut Commands,
    fonts: &EmberFonts,
    body: Entity,
    current: usize,
    total: usize,
) {
    if current < total {
        let step = &STEPS[current];

        let badge_row = commands
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            })
            .id();
        let badge = icon_text(commands, &fonts.phosphor, step.badge, accent(), 20.0);
        let counter = commands
            .spawn((
                Text::new(format!("STEP {} OF {}", current + 1, total)),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(text_muted())),
            ))
            .id();
        commands.entity(badge_row).add_children(&[badge, counter]);

        let title = commands
            .spawn((
                Text::new(step.title),
                ui_font(&fonts.ui, 17.0),
                TextColor(rgb(text_primary())),
            ))
            .id();
        let body_text = commands
            .spawn((
                Text::new(step.body),
                ui_font(&fonts.ui, 13.0),
                TextColor(rgb(text_muted())),
                Node { max_width: Val::Px(330.0), ..default() },
            ))
            .id();

        commands.entity(body).add_children(&[badge_row, title, body_text]);
        hints::build_hint(commands, fonts, body, &step.hint);
    } else {
        let icon = icon_text(commands, &fonts.phosphor, "confetti", play_green(), 40.0);
        let title = commands
            .spawn((
                Text::new("You're all set!"),
                ui_font(&fonts.ui, 18.0),
                TextColor(rgb(text_primary())),
            ))
            .id();
        let msg = commands
            .spawn((
                Text::new(
                    "That's the basics — orbiting, zooming, flying, selecting and moving objects. \
                     Dive in and start building your world.",
                ),
                ui_font(&fonts.ui, 13.0),
                TextColor(rgb(text_muted())),
                Node { max_width: Val::Px(330.0), ..default() },
            ))
            .id();
        let finish = small_button(commands, fonts, "Finish", true);
        commands.entity(finish).insert(TutorialFinishButton);
        commands.entity(body).add_children(&[icon, title, msg, finish]);
    }
}

/// A compact button: `filled` = accent background (primary), else a subtle
/// outline (secondary). Carries `Interaction` so a marker + change-detection
/// query can read presses.
fn small_button(commands: &mut Commands, fonts: &EmberFonts, label: &str, filled: bool) -> Entity {
    let (bg, fg, bc) = if filled {
        (rgb(accent()), Color::WHITE, rgb(accent()))
    } else {
        (Color::NONE, rgb(text_muted()), rgb(border()))
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
