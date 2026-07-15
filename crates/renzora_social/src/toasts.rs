//! Global toast queue: ephemeral notification cards stacked bottom-right,
//! built on the ember `toast()` card. Click a toast to jump to the relevant
//! panel (when it carries an action); toasts expire after a few seconds.

use std::collections::VecDeque;

use bevy::prelude::*;
use renzora::core::{SocialBridge, SocialPanelRequest};
use renzora_ember::font::EmberFonts;
use renzora_ember::widgets::toast;
pub(crate) use renzora_ember::widgets::Tone;

const MAX_VISIBLE: usize = 3;
const TTL_SECS: f64 = 5.0;

pub(crate) struct ToastRequest {
    pub tone: Tone,
    pub message: String,
    /// Clicking the toast opens this panel.
    pub action: Option<SocialPanelRequest>,
}

/// Pending toasts, drained into the UI up to [`MAX_VISIBLE`] at a time.
#[derive(Resource, Default)]
pub(crate) struct ToastQueue {
    pending: VecDeque<ToastRequest>,
}

impl ToastQueue {
    pub fn push(&mut self, tone: Tone, message: impl Into<String>, action: Option<SocialPanelRequest>) {
        self.pending.push_back(ToastRequest { tone, message: message.into(), action });
    }
}

/// A live toast card.
#[derive(Component)]
pub(crate) struct SocialToast {
    expires_at: f64,
    action: Option<SocialPanelRequest>,
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

    // Collapse floods into a single summary toast.
    if queue.pending.len() > 6 {
        let n = queue.pending.len();
        queue.pending.clear();
        queue.pending.push_back(ToastRequest {
            tone: Tone::Info,
            message: format!("{n} new notifications"),
            action: Some(SocialPanelRequest::Notifications),
        });
    }

    let visible = live.iter().count();
    let slots = MAX_VISIBLE.saturating_sub(visible);
    for _ in 0..slots {
        let Some(req) = queue.pending.pop_front() else { break };
        let card = toast(&mut commands, &fonts, req.tone, &req.message);
        commands.entity(card).insert((
            SocialToast { expires_at: now + TTL_SECS, action: req.action },
            Interaction::default(),
        ));
        commands.entity(container).add_child(card);
    }
}

/// Click a toast → jump to its panel (or just dismiss it).
pub(crate) fn toast_clicks(
    mut commands: Commands,
    mut bridge: ResMut<SocialBridge>,
    clicked: Query<(Entity, &Interaction, &SocialToast), Changed<Interaction>>,
) {
    for (e, interaction, t) in &clicked {
        if *interaction == Interaction::Pressed {
            if let Some(action) = &t.action {
                bridge.open_panel_request = Some(action.clone());
            }
            // `try_despawn`: the expiry pass may despawn this toast the same
            // frame it's clicked.
            commands.entity(e).try_despawn();
        }
    }
}
