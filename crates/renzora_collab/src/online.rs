//! Talking to renzora.com about rooms.
//!
//! The relay socket needs a room to connect to, and a room is created over the
//! ordinary REST API. That is the only part of the online path that is a *request*
//! rather than a stream, and it is kept here, apart from the transport, for one
//! reason: **`renzora_net::fetch` blocks the calling thread and must never be
//! called from a system.** A system runs inside the frame that the network pump
//! needs in order to make progress, so blocking there waits on something that
//! cannot happen until the system returns.
//!
//! So every call here happens on a worker thread and posts its answer back
//! through a channel, which [`poll`] drains once per frame and hands to the
//! session. The panel's buttons ask for work; they never wait for it.

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver, Sender};
use renzora_auth::AuthSession;

use crate::session::CollabSession;

/// The answer to a room request.
enum OnlineEvent {
    /// A room was created and is ready for its host to connect.
    Hosting { project: String, code: String, ws_url: String, token: String },
    /// A room was found and can be joined.
    Joining { code: String, ws_url: String, token: String, host_username: String },
    Failed(String),
}

/// Pending online work.
#[derive(Resource)]
pub struct OnlineRequests {
    tx: Sender<OnlineEvent>,
    rx: Receiver<OnlineEvent>,
    /// True while a request is in flight, so the panel can disable its buttons
    /// rather than letting an impatient second click open two rooms.
    pub busy: bool,
}

impl Default for OnlineRequests {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self { tx, rx, busy: false }
    }
}

/// Whether the editor is signed in enough to use the relay at all.
///
/// The relay is not anonymous: every participant authenticates, which is what
/// lets a host see real usernames instead of IP addresses, and what keeps a
/// stranger who guesses a code from being nobody at all.
pub fn access_token(session: &AuthSession) -> Option<String> {
    session.access_token.clone().filter(|_| session.is_signed_in())
}

/// Ask the site for a new room. The reply arrives via [`poll`].
pub fn request_host(requests: &mut OnlineRequests, token: String, project: String) {
    if requests.busy {
        return;
    }
    requests.busy = true;
    let tx = requests.tx.clone();
    spawn("collab-online-host", move || {
        let url = format!("{}/api/collab/sessions", renzora_auth::client::api_base());
        let body = serde_json::json!({ "project": project });
        let event = match post_json(&url, &token, &body) {
            Ok(value) => match (value.get("code"), value.get("ws_url")) {
                (Some(code), Some(ws_url)) => OnlineEvent::Hosting {
                    project,
                    code: code.as_str().unwrap_or_default().to_string(),
                    ws_url: ws_url.as_str().unwrap_or_default().to_string(),
                    token,
                },
                _ => OnlineEvent::Failed("the site returned an unexpected reply".into()),
            },
            Err(e) => OnlineEvent::Failed(e),
        };
        let _ = tx.send(event);
    });
}

/// Look a code up before joining it.
///
/// Checked first rather than simply opening the socket, because a wrong code
/// should say "no session with that code" rather than failing as a WebSocket
/// handshake error — and because it lets the panel show whose session it is
/// before the guest's own scene is replaced by it.
pub fn request_join(requests: &mut OnlineRequests, token: String, code: String) {
    if requests.busy {
        return;
    }
    requests.busy = true;
    let tx = requests.tx.clone();
    spawn("collab-online-join", move || {
        let code = code.trim().to_ascii_uppercase();
        let url = format!("{}/api/collab/sessions/{code}", renzora_auth::client::api_base());
        let event = match get_json(&url, &token) {
            Ok(value) => {
                let ws_url = value.get("ws_url").and_then(|v| v.as_str()).unwrap_or_default();
                if ws_url.is_empty() {
                    OnlineEvent::Failed("the site returned an unexpected reply".into())
                } else {
                    OnlineEvent::Joining {
                        code,
                        ws_url: ws_url.to_string(),
                        token,
                        host_username: value
                            .get("host_username")
                            .and_then(|v| v.as_str())
                            .unwrap_or("someone")
                            .to_string(),
                    }
                }
            }
            Err(e) => OnlineEvent::Failed(e),
        };
        let _ = tx.send(event);
    });
}

/// Invite a friend to the open room.
pub fn request_invite(requests: &OnlineRequests, token: String, code: String, user_id: String) {
    let tx = requests.tx.clone();
    spawn("collab-online-invite", move || {
        let url =
            format!("{}/api/collab/sessions/{code}/invite", renzora_auth::client::api_base());
        let body = serde_json::json!({ "user_id": user_id });
        if let Err(e) = post_json(&url, &token, &body) {
            let _ = tx.send(OnlineEvent::Failed(e));
        }
    });
}

/// Apply whatever the workers came back with.
pub fn poll(
    mut requests: ResMut<OnlineRequests>,
    mut session: ResMut<CollabSession>,
) {
    let mut events = Vec::new();
    while let Ok(event) = requests.rx.try_recv() {
        events.push(event);
    }
    for event in events {
        requests.busy = false;
        match event {
            OnlineEvent::Hosting { project, code, ws_url, token } => {
                session.start_hosting_online(&project, code, ws_url, token);
            }
            OnlineEvent::Joining { code, ws_url, token, host_username } => {
                session.note(format!("joining {host_username}'s session"));
                session.join_online(code, ws_url, token);
            }
            OnlineEvent::Failed(reason) => {
                session.status = reason.clone();
                session.note(reason);
            }
        }
    }
}

// ── Requests ────────────────────────────────────────────────────────────────

fn spawn(name: &str, work: impl FnOnce() + Send + 'static) {
    if let Err(e) = std::thread::Builder::new().name(name.into()).spawn(work) {
        log::error!("[collab] could not spawn {name}: {e}");
    }
}

fn post_json(
    url: &str,
    token: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let response = renzora_net::Request::post(url)
        .bearer(token)
        .json(body)
        .send()
        .map_err(describe)?;
    parse(response)
}

fn get_json(url: &str, token: &str) -> Result<serde_json::Value, String> {
    let response = renzora_net::Request::get(url).bearer(token).send().map_err(describe)?;
    parse(response)
}

fn parse(response: renzora_net::Response) -> Result<serde_json::Value, String> {
    let status = response.status;
    let text = response.text();
    if !(200..300).contains(&status) {
        // The API answers failures as `{"error": "..."}`; surface that rather
        // than a bare status, because these are messages meant for the user
        // ("You can only invite friends to a session").
        let message = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
            .unwrap_or_else(|| match status {
                401 => "You need to sign in to renzora.com first".to_string(),
                404 => "No session with that code — it may have ended".to_string(),
                _ => format!("The site returned {status}"),
            });
        return Err(message);
    }
    serde_json::from_str(&text).map_err(|e| format!("could not read the site's reply: {e}"))
}

fn describe(error: renzora_net::Error) -> String {
    format!("could not reach renzora.com: {error}")
}
