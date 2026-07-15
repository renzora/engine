//! Notifications data layer — the newest 50 notifications, kept live over the
//! WebSocket with a one-shot baseline fetch to seed the badge.
//!
//! There is **no notifications panel** anymore: the top-bar bell + its dropdown
//! ([`crate::notify_dropdown`]) are the whole surface, so a full docked panel
//! was redundant. This module keeps only the shared state and the fetch /
//! mark-read helpers the dropdown (and the WS handler) drive.

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver, Sender};
use renzora::core::SocialBridge;
use renzora::SplashState;
use renzora_auth::social::{NotificationRow, NotificationsResponse};
use renzora_auth::AuthSession;

use crate::util::session_clone;

pub(crate) enum NotifResult {
    List(Result<NotificationsResponse, String>),
    /// mark-read / mark-all-read finished (errors are silent; a resync heals).
    Marked,
}

#[derive(Resource)]
pub(crate) struct NotificationsPanel {
    pub items: Vec<NotificationRow>,
    pub loading: bool,
    pub error: Option<String>,
    pub version: u64,
    pub loaded_once: bool,
    pub tx: Sender<NotifResult>,
    rx: Receiver<NotifResult>,
}

impl Default for NotificationsPanel {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self {
            items: Vec::new(),
            loading: false,
            error: None,
            version: 0,
            loaded_once: false,
            tx,
            rx,
        }
    }
}

impl NotificationsPanel {
    pub fn bump(&mut self) {
        self.version = self.version.wrapping_add(1);
    }

    /// Prepend a live notification from the WebSocket.
    pub fn push_live(&mut self, row: NotificationRow) {
        self.items.retain(|n| n.id != row.id);
        self.items.insert(0, row);
        self.items.truncate(50);
        self.bump();
    }
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<NotificationsPanel>();
    // Only the data systems survive — no shell panel, no content builder.
    // `auto_refresh` runs ungated (not behind `panel_active`) so the bell badge
    // is correct from launch, before the dropdown is ever opened.
    app.add_systems(
        Update,
        (poll_results, auto_refresh).run_if(in_state(SplashState::Editor)),
    );
}

// ── Fetching ─────────────────────────────────────────────────────────────────

/// One-shot baseline fetch; marks `loaded_once` at SPAWN time so a failure can
/// never auto-retry — live updates arrive over the WebSocket.
pub(crate) fn refresh(panel: &mut NotificationsPanel, session: &AuthSession) {
    if !session.is_signed_in() {
        return;
    }
    panel.loaded_once = true;
    panel.loading = true;
    panel.error = None;
    let tx = panel.tx.clone();
    let session = session_clone(session);
    spawn_thread(move || {
        let _ = tx.send(NotifResult::List(renzora_auth::social::get_notifications(&session)));
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_thread(f: impl FnOnce() + Send + 'static) {
    std::thread::spawn(f);
}
#[cfg(target_arch = "wasm32")]
fn spawn_thread(_f: impl FnOnce() + Send + 'static) {}

// ── Systems ──────────────────────────────────────────────────────────────────

fn poll_results(mut panel: ResMut<NotificationsPanel>, mut bridge: ResMut<SocialBridge>) {
    let mut got = Vec::new();
    while let Ok(r) = panel.rx.try_recv() {
        got.push(r);
    }
    for r in got {
        match r {
            NotifResult::List(Ok(resp)) => {
                panel.items = resp.notifications;
                panel.loading = false;
                panel.loaded_once = true;
                bridge.unread_notifications = resp.unread.max(0) as u32;
                panel.bump();
            }
            NotifResult::List(Err(e)) => {
                panel.loading = false;
                panel.error = Some(e);
                panel.bump();
            }
            NotifResult::Marked => {}
        }
    }
}

/// One-shot baseline fetch on sign-in; the WebSocket keeps it live and a WS
/// reconnect clears `loaded_once` to resync. No polling.
fn auto_refresh(mut panel: ResMut<NotificationsPanel>, session: Res<AuthSession>) {
    if session.is_signed_in() && !panel.loaded_once {
        refresh(&mut panel, &session);
    }
}

// ── Mark-read helpers (driven by the bell dropdown) ──────────────────────────

/// Optimistically mark one notification read (locally + on the server).
pub(crate) fn mark_read_optimistic(
    panel: &mut NotificationsPanel,
    bridge: &mut SocialBridge,
    session: &AuthSession,
    id: &str,
) {
    if let Some(item) = panel.items.iter_mut().find(|x| x.id == id) {
        if item.read {
            return;
        }
        item.read = true;
    }
    bridge.unread_notifications = bridge.unread_notifications.saturating_sub(1);
    let tx = panel.tx.clone();
    let session = session_clone(session);
    let id = id.to_string();
    spawn_thread(move || {
        let _ = renzora_auth::social::mark_notification_read(&session, &id);
        let _ = tx.send(NotifResult::Marked);
    });
    panel.bump();
}

/// Optimistically mark every notification read (locally + on the server).
pub(crate) fn mark_all_optimistic(
    panel: &mut NotificationsPanel,
    bridge: &mut SocialBridge,
    session: &AuthSession,
) {
    if bridge.unread_notifications == 0 && panel.items.iter().all(|n| n.read) {
        return;
    }
    for item in panel.items.iter_mut() {
        item.read = true;
    }
    bridge.unread_notifications = 0;
    let tx = panel.tx.clone();
    let session = session_clone(session);
    spawn_thread(move || {
        let _ = renzora_auth::social::mark_all_notifications_read(&session);
        let _ = tx.send(NotifResult::Marked);
    });
    panel.bump();
}
