//! Toast: a notification card (themed, with a tone icon, the message and a
//! close ×), and the one queue that puts them on screen.
//!
//! # One queue for the whole editor
//!
//! [`ToastQueue`] is where every toast goes, and [`drain_toasts`] is the only
//! thing that draws them: stacked bottom-right above the status bar, a few at a
//! time, each gone after its time or on a click. The editor shell forwards
//! `renzora_ui::Toasts` into it, and the marketplace its own notices.
//!
//! It lives here, beside the card, because the editor had a queue and no
//! renderer. `renzora_ui::Toasts` lost its drawing in the move off egui and
//! nothing replaced it, so every toast queued since ("Snapped camera to
//! viewport", "Reloaded script", a move's reference count) was silently
//! dropped, while the marketplace drew its own in the same corner. One renderer
//! means one stack in that corner, not two drawn over each other.

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::font::{icon_text, ui_font, EmberFonts};
use crate::style::{Role, Styled};
use crate::theme::*;
use crate::UiInteraction;

use super::tone::Tone;

/// How many toasts show at once; the rest wait their turn.
const MAX_VISIBLE: usize = 3;

/// Above every dialog (up to 9700) and menu (up to 9790), below only the
/// full-screen image lightbox (9900). The marketplace's stack sat at 900,
/// under the tutorial card (8100) that shares its corner, so a toast raised
/// during the tutorial was drawn and never seen.
const TOAST_Z: i32 = 9800;

/// How long a toast stays up when its sender gives no time.
pub const DEFAULT_TOAST_SECS: f64 = 5.0;

struct PendingToast {
    tone: Tone,
    message: String,
    secs: f64,
}

/// Toasts waiting to be shown. Push from anywhere; [`drain_toasts`] shows them.
#[derive(Resource, Default)]
pub struct ToastQueue {
    pending: VecDeque<PendingToast>,
}

impl ToastQueue {
    /// Queue a toast for [`DEFAULT_TOAST_SECS`].
    pub fn push(&mut self, tone: Tone, message: impl Into<String>) {
        self.push_for(tone, message, DEFAULT_TOAST_SECS);
    }

    /// Queue a toast that stays up for `secs` once shown.
    pub fn push_for(&mut self, tone: Tone, message: impl Into<String>, secs: f64) {
        self.pending.push_back(PendingToast { tone, message: message.into(), secs });
    }
}

/// A toast on screen, and when it goes.
#[derive(Component)]
pub(crate) struct LiveToast {
    expires_at: f64,
}

/// The column the toasts stack in.
#[derive(Resource, Default)]
pub(crate) struct ToastStack {
    container: Option<Entity>,
}

/// Show queued toasts up to [`MAX_VISIBLE`] and take down the expired ones.
pub(crate) fn drain_toasts(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    time: Res<Time>,
    mut queue: ResMut<ToastQueue>,
    mut stack: ResMut<ToastStack>,
    live: Query<(Entity, &LiveToast)>,
    nodes: Query<(), With<Node>>,
) {
    let Some(fonts) = fonts else { return };
    let now = time.elapsed_secs_f64();

    // `try_despawn`: a click may have taken the same toast down this frame.
    let mut visible = 0;
    for (e, t) in &live {
        if now > t.expires_at {
            commands.entity(e).try_despawn();
        } else {
            visible += 1;
        }
    }
    if queue.pending.is_empty() || visible >= MAX_VISIBLE {
        return;
    }

    // Root-spawned, so no panel's clipping can hide it, and recreated if a
    // layout rebuild took it. `HideInHierarchy` makes it editor chrome: the
    // marketplace's stack, named and root-spawned without it, was written into
    // the open scene by every save and came back as a scene entity on load.
    let container = match stack.container.filter(|e| nodes.contains(*e)) {
        Some(e) => e,
        None => {
            let e = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(12.0),
                        bottom: Val::Px(34.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        align_items: AlignItems::FlexEnd,
                        ..default()
                    },
                    GlobalZIndex(TOAST_Z),
                    Name::new("toasts"),
                    renzora::HideInHierarchy,
                ))
                .id();
            stack.container = Some(e);
            e
        }
    };

    for _ in visible..MAX_VISIBLE {
        let Some(t) = queue.pending.pop_front() else { break };
        let card = toast(&mut commands, &fonts, t.tone, &t.message);
        commands
            .entity(card)
            .insert((LiveToast { expires_at: now + t.secs }, UiInteraction::default()));
        commands.entity(container).add_child(card);
    }
}

/// Click a toast to dismiss it.
pub(crate) fn toast_clicks(
    mut commands: Commands,
    clicked: Query<(Entity, &UiInteraction), (Changed<UiInteraction>, With<LiveToast>)>,
) {
    for (e, interaction) in &clicked {
        if *interaction == UiInteraction::Pressed {
            commands.entity(e).try_despawn();
        }
    }
}

/// A toast notification (themed card + tone icon + message + close ×).
pub fn toast(commands: &mut Commands, fonts: &EmberFonts, tone: Tone, message: &str) -> Entity {
    let box_e = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                align_items: AlignItems::Center,
                min_width: Val::Px(220.0),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            BorderColor::all(rgb(tree_line())),
            Styled::new(Role::Toast),
            Name::new("toast"),
        ))
        .id();
    let icon = icon_text(commands, &fonts.phosphor, tone.icon(), tone.color(), 14.0);
    let msg = commands
        .spawn((
            Text::new(message),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
            Node {
                flex_grow: 1.0,
                ..default()
            },
        ))
        .id();
    let close = icon_text(commands, &fonts.phosphor, "x", text_muted(), 12.0);
    commands.entity(box_e).add_children(&[icon, msg, close]);
    box_e
}
