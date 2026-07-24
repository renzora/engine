//! Live WebSocket client for renzora.com (`/api/ws/live`).
//!
//! One blocking `tungstenite` worker thread per signed-in session:
//! reconnects with backoff, parses `{"event","data"}` frames into
//! [`SocialWsEvent`]s on the worker, and hands them to the main thread over a
//! crossbeam channel. `poll_ws_events` fans them out into panel resources.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver, Sender};
use renzora::core::{SocialBridge, SocialPanelRequest, SocialWsState};
use renzora_auth::social::NotificationRow;
use renzora_auth::AuthSession;

use crate::panels::friends::FriendsPanel;
use crate::toasts::{ToastQueue, Tone};

/// How long to wait before respawning the worker after an auth failure.
const AUTH_RETRY_SECS: f64 = 30.0;

/// A parsed live event (deserialized on the worker thread).
#[derive(Debug)]
pub(crate) enum SocialWsEvent {
    Connected,
    /// Connection dropped; the worker is retrying on its own.
    Disconnected,
    /// The server rejected the token — worker exited; respawn with a fresh token.
    AuthFailed,
    Notification(NotificationRow),
    NewMessage {
        conversation_id: String,
        message_id: String,
        sender_id: String,
        sender_username: String,
        body: String,
        reply_to_id: Option<String>,
        created_at: String,
    },
    MessageEdited { conversation_id: String, message_id: String, body: String },
    MessageDeleted { conversation_id: String, message_id: String },
    ReadReceipt,
    FriendOnline { user_id: String },
    FriendOffline { user_id: String },
    /// Global `new_post` — ambiguous server-side (feed post OR forum reply);
    /// kept raw and used only as a refresh hint.
    NewPost(serde_json::Value),
    NewThread,
    Announcement { title: String },
    /// Credits landed on the account (a top-up purchase completed in the
    /// browser, or a gift was received). `amount` is the delta added.
    CreditUpdate { amount: i64 },
    /// Anything unrecognized — ignored (forward compatibility).
    Unknown,
}

#[derive(Resource)]
pub(crate) struct WsConnection {
    tx: Sender<SocialWsEvent>,
    rx: Receiver<SocialWsEvent>,
    shutdown: Option<Arc<AtomicBool>>,
    /// The token the live worker was spawned with (respawn on change).
    spawned_token: Option<String>,
    /// Don't respawn before this time (after auth failures).
    retry_at: f64,
    /// True once a connection has succeeded — used to resync only on REconnects.
    ever_connected: bool,
}

impl Default for WsConnection {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self { tx, rx, shutdown: None, spawned_token: None, retry_at: 0.0, ever_connected: false }
    }
}

/// Spawn/replace/stop the worker as the session changes.
pub(crate) fn manage_ws_connection(
    mut conn: ResMut<WsConnection>,
    mut bridge: ResMut<SocialBridge>,
    session: Res<AuthSession>,
    time: Res<Time>,
) {
    let token = session.access_token.clone().filter(|_| session.is_signed_in());

    match (&token, &conn.spawned_token) {
        // Signed out (or token gone) — stop the worker.
        (None, Some(_)) => {
            if let Some(flag) = conn.shutdown.take() {
                flag.store(true, Ordering::Relaxed);
            }
            conn.spawned_token = None;
            bridge.ws_state = SocialWsState::Disconnected;
        }
        // Fresh sign-in or token change — (re)spawn.
        (Some(t), spawned) if spawned.as_deref() != Some(t.as_str()) => {
            if time.elapsed_secs_f64() < conn.retry_at {
                return;
            }
            if let Some(flag) = conn.shutdown.take() {
                flag.store(true, Ordering::Relaxed);
            }
            let flag = Arc::new(AtomicBool::new(false));
            spawn_worker(t.clone(), conn.tx.clone(), flag.clone());
            conn.shutdown = Some(flag);
            conn.spawned_token = Some(t.clone());
            bridge.ws_state = SocialWsState::Connecting;
        }
        _ => {}
    }
}

/// Drain live events and fan them out.
#[allow(clippy::too_many_arguments)]
pub(crate) fn poll_ws_events(
    mut conn: ResMut<WsConnection>,
    mut bridge: ResMut<SocialBridge>,
    mut friends: ResMut<FriendsPanel>,
    mut toasts: ResMut<ToastQueue>,
    mut notifications: ResMut<crate::panels::notifications::NotificationsPanel>,
    mut chat: ResMut<crate::panels::chat::ChatPanel>,
    mut feed: ResMut<crate::panels::feed::FeedPanel>,
    mut confetti: ResMut<crate::confetti::Confetti>,
    mut session: ResMut<AuthSession>,
    time: Res<Time>,
) {
    let mut got = Vec::new();
    while let Ok(e) = conn.rx.try_recv() {
        got.push(e);
    }
    for event in got {
        match event {
            SocialWsEvent::Connected => {
                bridge.ws_state = SocialWsState::Connected;
                // After a reconnect, events may have been missed while offline —
                // clear the baselines so each panel refetches once when viewed.
                // (Bounded: one resync per reconnection, no polling.)
                if conn.ever_connected {
                    friends.loaded_once = false;
                    notifications.loaded_once = false;
                    chat.loaded_once = false;
                }
                conn.ever_connected = true;
            }
            SocialWsEvent::Disconnected => bridge.ws_state = SocialWsState::Connecting,
            SocialWsEvent::AuthFailed => {
                bridge.ws_state = SocialWsState::Disconnected;
                conn.spawned_token = None;
                conn.shutdown = None;
                conn.retry_at = time.elapsed_secs_f64() + AUTH_RETRY_SECS;
            }
            SocialWsEvent::Notification(row) => {
                bridge.unread_notifications = bridge.unread_notifications.saturating_add(1);
                let action = crate::routing::route_notification(&row);
                toasts.push(Tone::Info, row.title.clone(), Some(action));
                notifications.push_live(row);
            }
            SocialWsEvent::NewMessage {
                conversation_id,
                message_id,
                sender_id,
                sender_username,
                body,
                reply_to_id,
                created_at,
            } => {
                let own = session.user.as_ref().map(|u| u.id == sender_id).unwrap_or(false);
                let viewing = chat.active.as_deref() == Some(conversation_id.as_str());
                chat.apply_incoming(
                    &conversation_id,
                    message_id,
                    sender_id,
                    sender_username.clone(),
                    body.clone(),
                    reply_to_id,
                    created_at,
                );
                if !own && !viewing {
                    bridge.unread_messages = bridge.unread_messages.saturating_add(1);
                    toasts.push(
                        Tone::Info,
                        format!("{sender_username}: {}", truncate(&body, 60)),
                        Some(SocialPanelRequest::Chat { conversation_id: Some(conversation_id) }),
                    );
                }
            }
            SocialWsEvent::MessageEdited { conversation_id, message_id, body } => {
                chat.apply_edit(&conversation_id, &message_id, body);
            }
            SocialWsEvent::MessageDeleted { conversation_id, message_id } => {
                chat.apply_delete(&conversation_id, &message_id);
            }
            SocialWsEvent::ReadReceipt => {}
            SocialWsEvent::FriendOnline { user_id } => {
                friends.online.insert(user_id, true);
                bridge.friends_online = friends.online_count();
                friends.bump();
            }
            SocialWsEvent::FriendOffline { user_id } => {
                friends.online.insert(user_id, false);
                bridge.friends_online = friends.online_count();
                friends.bump();
            }
            SocialWsEvent::NewPost(data) => {
                // Forum replies (which carried `thread_slug`) no longer have a
                // panel; only a real feed post marks the feed stale.
                if data.get("thread_slug").is_none() {
                    feed.stale = true;
                }
            }
            // The forum was replaced by feed channels — thread events are inert.
            SocialWsEvent::NewThread => {}
            SocialWsEvent::Announcement { title } => {
                toasts.push(Tone::Info, title, None);
            }
            SocialWsEvent::CreditUpdate { amount } => {
                // Reflect the new balance live (the Wallet header + any credit
                // display read `session.user.credit_balance`), and celebrate.
                if amount != 0 {
                    if let Some(u) = session.user.as_mut() {
                        u.credit_balance = (u.credit_balance + amount).max(0);
                    }
                }
                if amount > 0 {
                    toasts.push(Tone::Success, format!("+{amount} credits added"), None);
                    confetti.fire();
                }
            }
            SocialWsEvent::Unknown => {}
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…")
    }
}

// ── Worker ───────────────────────────────────────────────────────────────────

/// Install ring as the process-level rustls provider exactly once (no-op if
/// something else already installed one).
#[cfg(not(target_arch = "wasm32"))]
fn ensure_crypto_provider() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_worker(token: String, tx: Sender<SocialWsEvent>, shutdown: Arc<AtomicBool>) {
    ensure_crypto_provider();
    std::thread::spawn(move || {
        let base = renzora_auth::client::api_base();
        let ws_base = if let Some(rest) = base.strip_prefix("https://") {
            format!("wss://{rest}")
        } else if let Some(rest) = base.strip_prefix("http://") {
            format!("ws://{rest}")
        } else {
            format!("wss://{base}")
        };
        let url = format!("{ws_base}/api/ws/live?token={token}");

        let mut backoff = 1u64;
        loop {
            if shutdown.load(Ordering::Relaxed) {
                return;
            }
            let connected_at = std::time::Instant::now();
            let request = match build_request(&url) {
                Ok(req) => req,
                Err(e) => {
                    bevy::log::warn!("[social] live WebSocket URL rejected: {e}");
                    return;
                }
            };
            match tungstenite::connect(request) {
                Ok((mut socket, _resp)) => {
                    bevy::log::info!("[social] live WebSocket connected");
                    // Short read timeout so the shutdown flag is honored.
                    set_read_timeout(&mut socket, std::time::Duration::from_secs(1));
                    let _ = tx.send(SocialWsEvent::Connected);
                    let mut last_ping = std::time::Instant::now();

                    loop {
                        if shutdown.load(Ordering::Relaxed) {
                            let _ = socket.close(None);
                            return;
                        }
                        // Keep-alive through proxies.
                        if last_ping.elapsed().as_secs() >= 30 {
                            last_ping = std::time::Instant::now();
                            if socket.send(tungstenite::Message::Ping(Vec::new())).is_err() {
                                break;
                            }
                        }
                        match socket.read() {
                            Ok(tungstenite::Message::Text(text)) => {
                                let event = parse_event(&text);
                                if !matches!(event, SocialWsEvent::Unknown) && tx.send(event).is_err() {
                                    return;
                                }
                            }
                            Ok(tungstenite::Message::Ping(payload)) => {
                                let _ = socket.send(tungstenite::Message::Pong(payload));
                            }
                            Ok(tungstenite::Message::Close(_)) => break,
                            Ok(_) => {}
                            Err(tungstenite::Error::Io(e))
                                if e.kind() == std::io::ErrorKind::WouldBlock
                                    || e.kind() == std::io::ErrorKind::TimedOut =>
                            {
                                continue;
                            }
                            Err(_) => break,
                        }
                    }
                    let _ = tx.send(SocialWsEvent::Disconnected);
                    // A connection that survived a while resets the backoff.
                    if connected_at.elapsed().as_secs() > 60 {
                        backoff = 1;
                    }
                }
                Err(tungstenite::Error::Http(resp)) if resp.status() == 401 => {
                    bevy::log::warn!("[social] live WebSocket rejected: 401 (token expired?)");
                    let _ = tx.send(SocialWsEvent::AuthFailed);
                    return;
                }
                Err(e) => {
                    bevy::log::warn!("[social] live WebSocket connect failed: {e}");
                    let _ = tx.send(SocialWsEvent::Disconnected);
                }
            }

            // Backoff before reconnecting, still honoring shutdown.
            let wait = backoff.min(30);
            backoff = (backoff * 2).min(30);
            for _ in 0..wait * 10 {
                if shutdown.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    });
}

/// Build the handshake request, adding the `User-Agent` tungstenite omits.
///
/// renzora.com sits behind Cloudflare, whose managed WAF rules block requests
/// that arrive with no `User-Agent` at all — the handshake never reaches axum
/// and comes back as a 403 HTML error page instead of the 401 the route would
/// return for a bad token. Our `ureq` calls are fine because ureq sends its own
/// UA by default; tungstenite sends none, so we set one here.
#[cfg(not(target_arch = "wasm32"))]
fn build_request(url: &str) -> Result<tungstenite::http::Request<()>, tungstenite::Error> {
    use tungstenite::client::IntoClientRequest;
    let mut request = url.into_client_request()?;
    request.headers_mut().insert(
        tungstenite::http::header::USER_AGENT,
        tungstenite::http::HeaderValue::from_static("renzora-editor"),
    );
    Ok(request)
}

#[cfg(target_arch = "wasm32")]
fn spawn_worker(_token: String, _tx: Sender<SocialWsEvent>, _shutdown: Arc<AtomicBool>) {}

#[cfg(not(target_arch = "wasm32"))]
fn set_read_timeout(
    socket: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    timeout: std::time::Duration,
) {
    use tungstenite::stream::MaybeTlsStream;
    match socket.get_mut() {
        MaybeTlsStream::Plain(s) => {
            let _ = s.set_read_timeout(Some(timeout));
        }
        MaybeTlsStream::Rustls(s) => {
            let _ = s.get_ref().set_read_timeout(Some(timeout));
        }
        _ => {}
    }
}

/// Parse a `{"event": ..., "data": ...}` frame.
#[cfg(not(target_arch = "wasm32"))]
fn parse_event(text: &str) -> SocialWsEvent {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return SocialWsEvent::Unknown;
    };
    let event = value.get("event").and_then(|e| e.as_str()).unwrap_or("");
    let data = value.get("data").cloned().unwrap_or(serde_json::Value::Null);
    let s = |key: &str| data.get(key).and_then(|v| v.as_str()).map(|v| v.to_string());

    match event {
        "connected" => SocialWsEvent::Unknown, // welcome frame; Connected is sent on socket open
        "notification" => match serde_json::from_value::<NotificationRow>(data) {
            Ok(row) => SocialWsEvent::Notification(row),
            Err(_) => SocialWsEvent::Unknown,
        },
        "new_message" => SocialWsEvent::NewMessage {
            conversation_id: s("conversation_id").unwrap_or_default(),
            message_id: s("message_id").unwrap_or_default(),
            sender_id: s("sender_id").unwrap_or_default(),
            sender_username: s("sender_username").unwrap_or_default(),
            body: s("body").unwrap_or_default(),
            reply_to_id: s("reply_to_id"),
            created_at: s("created_at").unwrap_or_default(),
        },
        "message_edited" => SocialWsEvent::MessageEdited {
            conversation_id: s("conversation_id").unwrap_or_default(),
            message_id: s("message_id").unwrap_or_default(),
            body: s("body").unwrap_or_default(),
        },
        "message_deleted" => SocialWsEvent::MessageDeleted {
            conversation_id: s("conversation_id").unwrap_or_default(),
            message_id: s("message_id").unwrap_or_default(),
        },
        "read_receipt" => SocialWsEvent::ReadReceipt,
        "friend_online" => SocialWsEvent::FriendOnline { user_id: s("user_id").unwrap_or_default() },
        "friend_offline" => SocialWsEvent::FriendOffline { user_id: s("user_id").unwrap_or_default() },
        "new_post" => SocialWsEvent::NewPost(data),
        "new_thread" => SocialWsEvent::NewThread,
        "announcement" => SocialWsEvent::Announcement { title: s("title").unwrap_or_default() },
        "credit_update" => SocialWsEvent::CreditUpdate {
            amount: data.get("amount").and_then(|v| v.as_i64()).unwrap_or(0),
        },
        _ => SocialWsEvent::Unknown,
    }
}
