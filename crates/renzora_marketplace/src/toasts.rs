//! Global toast queue: ephemeral notification cards stacked bottom-right,
//! built on the ember `toast()` card. Click a toast to jump to the relevant
//! panel (when it carries an action); toasts expire after a few seconds.

use std::collections::VecDeque;

use bevy::prelude::*;
// A toast used to be able to carry an action — a deep link into whichever social
// panel the notification came from — and clicking it jumped there. Notifications
// are gone, every remaining caller passed `None`, and a toast here is now purely
// a message you dismiss.
use renzora_ember::font::EmberFonts;
use renzora_ember::widgets::toast;
pub(crate) use renzora_ember::widgets::Tone;

const MAX_VISIBLE: usize = 3;
const TTL_SECS: f64 = 5.0;

pub(crate) struct ToastRequest {
    pub tone: Tone,
    pub message: String,
}

/// Pending toasts, drained into the UI up to [`MAX_VISIBLE`] at a time.
#[derive(Resource, Default)]
pub(crate) struct ToastQueue {
    pending: VecDeque<ToastRequest>,
}

impl ToastQueue {
    /// The third argument is vestigial: it carried the panel a click should open
    /// and every caller passes `None`. Kept so the ~24 call sites did not all
    /// have to change in a commit that is about something else.
    pub fn push(&mut self, tone: Tone, message: impl Into<String>, _action: Option<()>) {
        self.pending.push_back(ToastRequest { tone, message: message.into() });
    }
}

/// A live toast card.
#[derive(Component)]
pub(crate) struct SocialToast {
    expires_at: f64,
}

/// The stacking container (bottom-right, above the status bar).
#[derive(Resource, Default)]
pub(crate) struct ToastUi {
    container: Option<Entity>,
}

/// Spawn pending toasts (respecting the visible cap), expire old ones.
pub(crate) fn drain_toasts(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    time: Res<Time>,
    mut queue: ResMut<ToastQueue>,
    mut ui: ResMut<ToastUi>,
    live: Query<(Entity, &SocialToast)>,
    containers: Query<Entity, With<Node>>,
) {
    let Some(fonts) = fonts else { return };
    let now = time.elapsed_secs_f64();

    // Expire old toasts. `try_despawn`: `toast_clicks` may have despawned the
    // same toast this frame (click on the frame it expires).
    for (e, t) in &live {
        if now > t.expires_at {
            commands.entity(e).try_despawn();
        }
    }

    if queue.pending.is_empty() {
        return;
    }

    // Ensure the container exists.
    let container = match ui.container.filter(|e| containers.get(*e).is_ok()) {
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
                    GlobalZIndex(900),
                    Name::new("social_toasts"),
                ))
                .id();
            ui.container = Some(e);
            e
        }
    };

    let visible = live.iter().count();
    let slots = MAX_VISIBLE.saturating_sub(visible);
    for _ in 0..slots {
        let Some(req) = queue.pending.pop_front() else { break };
        let card = toast(&mut commands, &fonts, req.tone, &req.message);
        commands.entity(card).insert((
            SocialToast { expires_at: now + TTL_SECS },
            Interaction::default(),
        ));
        commands.entity(container).add_child(card);
    }
}

/// Click a toast to dismiss it.
pub(crate) fn toast_clicks(
    mut commands: Commands,
    clicked: Query<(Entity, &Interaction), (Changed<Interaction>, With<SocialToast>)>,
) {
    for (e, interaction) in &clicked {
        if *interaction == Interaction::Pressed {
            // `try_despawn`: the expiry pass may despawn this toast the same
            // frame it's clicked.
            commands.entity(e).try_despawn();
        }
    }
}
