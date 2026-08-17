//! Session state: who is connected, in which role, and the link pump that keeps
//! that true.
//!
//! The pump is a plain system and deliberately does **not** touch the scene. It
//! handles the transport-level half of the protocol — handshakes, presence, peer
//! bookkeeping — and everything that needs to read or write the world is pushed
//! into [`CollabInbox`] for the exclusive systems in [`crate::sync`] to drain.
//! Keeping those apart is what stops the whole editor from taking `&mut World`
//! once per frame just to notice that a peer's camera moved.

use std::collections::{BTreeMap, VecDeque};

use bevy::prelude::*;

use crate::identity::CollabIds;
use crate::link::{Acceptor, Link, LinkEvent};
use crate::protocol::{CamPose, CollabMsg, PROTOCOL_VERSION};
use crate::tcp;

/// Which side of the session this editor is on.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CollabRole {
    #[default]
    Offline,
    /// This editor owns the document; peers connect to it.
    Hosting,
    /// This editor is connected to someone else's document.
    Guest,
}

/// The host is always slot 0 of the id space; guests get their slot from the
/// `Welcome` that admitted them.
pub const HOST_SLOT: u16 = 0;

/// The peer id a guest files its single link under — the host's.
pub const HOST_PEER: u64 = 0;

/// What we know about someone else in the session.
#[derive(Clone, Debug)]
pub struct PeerInfo {
    pub id: u64,
    pub name: String,
    pub color: [u8; 3],
    pub camera: Option<CamPose>,
    /// Ids the peer currently has selected — drawn as their highlight, and the
    /// basis of the lease they are implicitly asking for.
    pub selection: Vec<u64>,
    /// Ids this peer holds an edit lease on.
    pub leases: Vec<u64>,
    pub last_seen: f64,
    /// True once the peer has completed the handshake. A connected-but-unwelcomed
    /// link is a stranger on an open port, and is sent nothing.
    pub ready: bool,
}

/// Messages the pump could not handle without world access.
#[derive(Resource, Default)]
pub struct CollabInbox {
    /// `(from_peer, message)` in arrival order. Order matters across kinds —
    /// see the protocol module — so this is one queue, not one per kind.
    pub queue: VecDeque<(u64, CollabMsg)>,
    /// Peers admitted this frame that still need the document sent to them.
    pub needs_scene: Vec<u64>,
}

/// The live session.
#[derive(Resource)]
pub struct CollabSession {
    pub role: CollabRole,
    /// One line describing the session, shown in the panel and the status bar.
    pub status: String,
    /// How this editor introduces itself.
    pub display_name: String,
    /// Panel field state. Kept as text because that is what the input widget
    /// binds to, and because an unparseable port should show the user what they
    /// typed rather than silently becoming 0.
    pub port_text: String,
    pub join_text: String,
    /// Host-only switch: whether guests may change the document, or only watch.
    /// Off is the safe default — "let me show you something" is a much more
    /// common invitation than "take the wheel", and the host can flip it the
    /// moment they mean the second one.
    pub allow_control: bool,
    /// This peer's slot in the id space.
    pub slot: u16,
    /// Whether the host has granted this guest edit control. Always true for a
    /// host, which cannot be denied its own document.
    pub granted_control: bool,
    pub peers: BTreeMap<u64, PeerInfo>,
    /// The LAN address to read out to a guest, if we could work one out.
    pub address_hint: Option<String>,
    /// The relay room code, when this session goes through renzora.com. This is
    /// what the host reads out instead of an IP address, and the whole reason
    /// the relay exists — an eight-character code works between two people on
    /// ordinary home connections, and an IP address does not.
    pub room_code: Option<String>,
    /// Panel field: a code being typed in to join with.
    pub code_text: String,
    /// Recent session events, newest last. Bounded — see [`Self::note`].
    pub log: VecDeque<String>,

    /// Where inbound peers come from while hosting — a port, or the relay.
    acceptor: Option<Acceptor>,
    /// Host: one entry per peer. Guest: exactly one, keyed [`HOST_PEER`].
    links: BTreeMap<u64, Link>,
    next_peer: u64,
}

impl Default for CollabSession {
    fn default() -> Self {
        Self {
            role: CollabRole::Offline,
            status: String::new(),
            display_name: default_display_name(),
            port_text: crate::DEFAULT_PORT.to_string(),
            join_text: String::new(),
            allow_control: false,
            slot: HOST_SLOT,
            granted_control: false,
            peers: BTreeMap::new(),
            address_hint: None,
            room_code: None,
            code_text: String::new(),
            log: VecDeque::new(),
            acceptor: None,
            links: BTreeMap::new(),
            next_peer: 1,
        }
    }
}

/// The machine's user name, falling back to something neutral. Only a default —
/// the panel lets the user set it, and it is purely cosmetic.
fn default_display_name() -> String {
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "Editor".to_string())
}

impl CollabSession {
    pub fn is_active(&self) -> bool {
        self.role != CollabRole::Offline
    }

    pub fn is_host(&self) -> bool {
        self.role == CollabRole::Hosting
    }

    pub fn is_guest(&self) -> bool {
        self.role == CollabRole::Guest
    }

    /// Whether this editor is allowed to change the document.
    ///
    /// A guest whose host has not granted control can still move its own camera
    /// and select things — it simply never sends an edit. The local edit is not
    /// blocked, because blocking every mutation path in the editor is neither
    /// possible nor desirable; it just does not leave the machine, and the next
    /// upstream change overwrites it.
    pub fn may_edit(&self) -> bool {
        match self.role {
            CollabRole::Offline | CollabRole::Hosting => true,
            CollabRole::Guest => self.granted_control,
        }
    }

    pub fn note(&mut self, line: impl Into<String>) {
        let line = line.into();
        log::info!("[collab] {line}");
        self.log.push_back(line);
        // Bounded: this is a session log shown in a small panel, and an unbounded
        // one is a slow leak in a session left open all day.
        while self.log.len() > 200 {
            self.log.pop_front();
        }
    }

    /// Send to every ready peer.
    pub fn broadcast(&self, msg: CollabMsg) {
        for (id, link) in &self.links {
            if self.peers.get(id).is_some_and(|p| p.ready) {
                link.send(msg.clone());
            }
        }
    }

    /// Send to every ready peer but one — how the host relays a guest's edit to
    /// the rest without echoing it back to its author.
    pub fn broadcast_except(&self, skip: u64, msg: CollabMsg) {
        for (id, link) in &self.links {
            if *id != skip && self.peers.get(id).is_some_and(|p| p.ready) {
                link.send(msg.clone());
            }
        }
    }

    pub fn send_to(&self, peer: u64, msg: CollabMsg) {
        if let Some(link) = self.links.get(&peer) {
            link.send(msg);
        }
    }

    /// A sender a worker thread can hold — see [`Link::sender`].
    pub fn sender_for(&self, peer: u64) -> Option<crossbeam_channel::Sender<CollabMsg>> {
        self.links.get(&peer).map(|l| l.sender())
    }

    /// Send upstream: to the host if we are a guest, otherwise to everyone.
    pub fn send_up(&self, msg: CollabMsg) {
        match self.role {
            CollabRole::Guest => self.send_to(HOST_PEER, msg),
            _ => self.broadcast(msg),
        }
    }

    /// Open the port and start accepting peers.
    pub fn start_hosting(&mut self, project: &str) {
        if self.is_active() {
            return;
        }
        let port: u16 = self.port_text.trim().parse().unwrap_or(crate::DEFAULT_PORT);
        match tcp::listen(port) {
            Ok(acceptor) => {
                // The bound port, not the requested one — asking for 0 means
                // "any free port", and the guest needs the answer.
                let port = acceptor.local_port.unwrap_or(port);
                self.port_text = port.to_string();
                self.address_hint = tcp::local_address_hint();
                let where_ = self
                    .address_hint
                    .as_deref()
                    .map(|ip| format!("{ip}:{port}"))
                    .unwrap_or_else(|| acceptor.origin.clone());
                self.acceptor = Some(acceptor);
                self.role = CollabRole::Hosting;
                self.slot = HOST_SLOT;
                self.next_peer = 1;
                self.status = format!("Hosting “{project}” on {where_}");
                self.note(format!("hosting {project} — peers connect to {where_}"));
            }
            Err(e) => {
                self.status = format!("Could not host on port {port}: {e}");
                self.note(self.status.clone());
            }
        }
    }

    /// Host through renzora.com rather than a port on this machine.
    ///
    /// The room has already been created over the REST API by the time this is
    /// called — see [`crate::online`] — because creating it is a network request
    /// and a network request must never happen in a system. All that is left
    /// here is opening the relay socket the room told us about.
    pub fn start_hosting_online(
        &mut self,
        project: &str,
        code: String,
        ws_url: String,
        token: String,
    ) {
        if self.is_active() {
            return;
        }
        self.acceptor = Some(crate::relay::host(ws_url, token, code.clone()));
        self.role = CollabRole::Hosting;
        self.slot = HOST_SLOT;
        self.next_peer = 1;
        self.address_hint = None;
        self.room_code = Some(code.clone());
        self.status = format!("Hosting “{project}” — share code {code}");
        self.note(format!("hosting {project} online — code {code}"));
    }

    /// Join a relayed session by code.
    pub fn join_online(&mut self, code: String, ws_url: String, token: String) {
        if self.is_active() {
            return;
        }
        let link = crate::relay::join(ws_url, token);
        self.links.insert(HOST_PEER, link);
        self.peers.insert(
            HOST_PEER,
            PeerInfo {
                id: HOST_PEER,
                name: "host".into(),
                color: peer_color(HOST_PEER),
                camera: None,
                selection: Vec::new(),
                leases: Vec::new(),
                last_seen: 0.0,
                ready: false,
            },
        );
        self.role = CollabRole::Guest;
        self.granted_control = false;
        self.room_code = Some(code.clone());
        self.status = format!("Joining session {code}…");
        self.note(format!("joining session {code}"));
    }

    /// Connect to a host. The connection itself completes on a worker thread.
    pub fn join(&mut self) {
        if self.is_active() {
            return;
        }
        let addr = self.join_text.trim().to_string();
        if addr.is_empty() {
            self.status = "Enter the host's address first".into();
            return;
        }
        let link = tcp::connect(addr.clone());
        self.links.insert(HOST_PEER, link);
        self.peers.insert(
            HOST_PEER,
            PeerInfo {
                id: HOST_PEER,
                name: "host".into(),
                color: peer_color(HOST_PEER),
                camera: None,
                selection: Vec::new(),
                leases: Vec::new(),
                last_seen: 0.0,
                ready: false,
            },
        );
        self.role = CollabRole::Guest;
        self.granted_control = false;
        self.status = format!("Connecting to {addr}…");
        self.note(format!("connecting to {addr}"));
    }

    /// Tear the session down. Dropping the listener and the links is what
    /// actually closes the sockets — each signals its workers on drop.
    pub fn leave(&mut self) {
        if self.is_active() {
            self.note("session ended");
        }
        self.acceptor = None;
        self.room_code = None;
        self.links.clear();
        self.peers.clear();
        self.role = CollabRole::Offline;
        self.granted_control = false;
        self.status = String::new();
    }
}

/// A stable colour per peer, so the same collaborator is the same colour in the
/// hierarchy, the viewport and the panel without anyone having to pick one.
pub fn peer_color(peer: u64) -> [u8; 3] {
    const PALETTE: [[u8; 3]; 6] = [
        [86, 156, 214],  // blue
        [206, 145, 120], // orange
        [181, 206, 168], // green
        [197, 134, 192], // purple
        [220, 220, 120], // yellow
        [120, 206, 200], // teal
    ];
    PALETTE[(peer as usize) % PALETTE.len()]
}

/// Accept connections, drain every link, and keep [`CollabSession::peers`] true.
pub fn pump_links(
    mut session: ResMut<CollabSession>,
    mut inbox: ResMut<CollabInbox>,
    mut ids: ResMut<CollabIds>,
    project: Option<Res<renzora::core::CurrentProject>>,
    time: Res<Time>,
) {
    if !session.is_active() {
        return;
    }
    let now = time.elapsed_secs_f64();
    let project_name = project
        .as_ref()
        .and_then(|p| p.path.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    accept_new_peers(&mut session, now);

    // Drain first, act second: the borrow checker will not let a link be read
    // while the session it lives in is mutated, and every handler below mutates.
    let mut events: Vec<(u64, LinkEvent)> = Vec::new();
    for (&peer, link) in session.links.iter_mut() {
        for event in link.drain() {
            events.push((peer, event));
        }
    }

    let mut dropped: Vec<u64> = Vec::new();
    for (peer, event) in events {
        match event {
            LinkEvent::Connected => {
                if session.is_guest() {
                    // The socket is up; introduce ourselves and wait to be let in.
                    let hello = CollabMsg::Hello {
                        protocol: PROTOCOL_VERSION,
                        display_name: session.display_name.clone(),
                        project: project_name.clone(),
                    };
                    session.send_to(peer, hello);
                    session.status = "Connected — waiting to be admitted…".into();
                }
            }
            LinkEvent::Closed(reason) => {
                dropped.push(peer);
                let who = session
                    .peers
                    .get(&peer)
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| format!("peer {peer}"));
                session.note(format!("{who} disconnected: {reason}"));
            }
            LinkEvent::Message(msg) => {
                handle_message(&mut session, &mut inbox, &mut ids, peer, msg, now, &project_name);
            }
        }
    }

    for peer in dropped {
        session.links.remove(&peer);
        session.peers.remove(&peer);
        if session.is_guest() {
            // The guest's only link is the host's; losing it ends the session.
            let status = std::mem::take(&mut session.status);
            session.leave();
            session.status = if status.starts_with("Could not") { status } else { "Disconnected".into() };
        } else {
            session.broadcast(CollabMsg::PeerLeft { peer });
        }
    }
}

/// Move freshly accepted sockets into the session as not-yet-ready peers.
fn accept_new_peers(session: &mut CollabSession, now: f64) {
    let mut fresh: Vec<Link> = Vec::new();
    if let Some(acceptor) = &session.acceptor {
        while let Ok(link) = acceptor.incoming.try_recv() {
            fresh.push(link);
        }
    }
    for link in fresh {
        let peer = session.next_peer;
        session.next_peer += 1;
        session.peers.insert(
            peer,
            PeerInfo {
                id: peer,
                name: link.addr.clone(),
                color: peer_color(peer),
                camera: None,
                selection: Vec::new(),
                leases: Vec::new(),
                last_seen: now,
                ready: false,
            },
        );
        session.links.insert(peer, link);
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_message(
    session: &mut CollabSession,
    inbox: &mut CollabInbox,
    ids: &mut CollabIds,
    peer: u64,
    msg: CollabMsg,
    now: f64,
    project_name: &str,
) {
    if let Some(info) = session.peers.get_mut(&peer) {
        info.last_seen = now;
    }

    match msg {
        CollabMsg::Hello { protocol, display_name, project } => {
            if !session.is_host() {
                return;
            }
            // Version before anything else. A peer that speaks a different
            // protocol cannot be reasoned with, and letting it half-participate
            // would corrupt the document rather than fail cleanly.
            if protocol != PROTOCOL_VERSION {
                let reason = format!(
                    "protocol {protocol} but this editor speaks {PROTOCOL_VERSION} — update the older one"
                );
                session.send_to(peer, CollabMsg::Rejected { reason: reason.clone() });
                session.note(format!("refused {display_name}: {reason}"));
                session.peers.remove(&peer);
                session.links.remove(&peer);
                return;
            }
            if !project.is_empty() && !project_name.is_empty() && project != project_name {
                session.note(format!(
                    "{display_name} has project “{project}” open, this is “{project_name}” — files will be synced"
                ));
            }
            if let Some(info) = session.peers.get_mut(&peer) {
                info.name = display_name.clone();
                info.ready = true;
            }
            session.send_to(
                peer,
                CollabMsg::Welcome {
                    protocol: PROTOCOL_VERSION,
                    peer_id: peer,
                    host_name: session.display_name.clone(),
                    project: project_name.to_string(),
                },
            );
            // Tell the others, and tell the newcomer who is already here.
            let color = session.peers.get(&peer).map(|p| p.color).unwrap_or([200, 200, 200]);
            session.broadcast_except(
                peer,
                CollabMsg::PeerJoined { peer, name: display_name.clone(), color },
            );
            let existing: Vec<CollabMsg> = session
                .peers
                .values()
                .filter(|p| p.id != peer && p.ready)
                .map(|p| CollabMsg::PeerJoined { peer: p.id, name: p.name.clone(), color: p.color })
                .collect();
            for m in existing {
                session.send_to(peer, m);
            }
            session.send_to(peer, CollabMsg::Control { allowed: session.allow_control });
            session.note(format!("{display_name} joined"));
            inbox.needs_scene.push(peer);
        }

        CollabMsg::Welcome { protocol, peer_id, host_name, project } => {
            if !session.is_guest() {
                return;
            }
            if protocol != PROTOCOL_VERSION {
                session.status =
                    format!("Host speaks protocol {protocol}, this editor speaks {PROTOCOL_VERSION}");
                session.leave();
                return;
            }
            // Claim our slice of the id space. Doing this before any scene
            // arrives matters: ids minted against the wrong slot would collide
            // with the host's for the rest of the session.
            session.slot = (peer_id as u16).max(1);
            ids.begin(session.slot);
            if let Some(info) = session.peers.get_mut(&HOST_PEER) {
                info.name = host_name.clone();
                info.ready = true;
            }
            session.status = format!("In {host_name}'s session — “{project}”");
            session.note(format!("joined {host_name}'s session"));
        }

        CollabMsg::Rejected { reason } => {
            session.status = format!("Refused: {reason}");
            session.note(format!("refused by host: {reason}"));
            session.leave();
        }

        CollabMsg::PeerJoined { peer: id, name, color } => {
            session.peers.entry(id).or_insert_with(|| PeerInfo {
                id,
                name: name.clone(),
                color,
                camera: None,
                selection: Vec::new(),
                leases: Vec::new(),
                last_seen: now,
                ready: true,
            });
            session.note(format!("{name} is here"));
        }

        CollabMsg::PeerLeft { peer: id } => {
            if let Some(info) = session.peers.remove(&id) {
                session.note(format!("{} left", info.name));
            }
        }

        CollabMsg::Presence { peer: from, camera, selection } => {
            // A host relays presence so guests can see each other; it is the only
            // party holding every link.
            if session.is_host() {
                session.broadcast_except(
                    peer,
                    CollabMsg::Presence { peer, camera, selection: selection.clone() },
                );
            }
            let id = if session.is_host() { peer } else { from };
            if let Some(info) = session.peers.get_mut(&id) {
                info.camera = camera;
                info.selection = selection;
                info.last_seen = now;
            }
        }

        CollabMsg::LeaseRequest { ids: wanted } => {
            // Only the host arbitrates, and it arbitrates by the simplest rule
            // that works: first come, first served, and a request for something
            // already held is silently trimmed rather than refused. A guest that
            // asked for five entities and got four should carry on with the
            // four, not stop and ask again.
            if !session.is_host() {
                return;
            }
            let taken: Vec<u64> = session
                .peers
                .values()
                .filter(|p| p.id != peer)
                .flat_map(|p| p.leases.iter().copied())
                .collect();
            let granted: Vec<u64> =
                wanted.into_iter().filter(|id| !taken.contains(id)).collect();
            if let Some(info) = session.peers.get_mut(&peer) {
                info.leases = granted.clone();
            }
            session.broadcast(CollabMsg::LeaseGrant { peer, ids: granted });
        }

        CollabMsg::LeaseRelease { ids: released } => {
            if let Some(info) = session.peers.get_mut(&peer) {
                info.leases.retain(|id| !released.contains(id));
            }
            if session.is_host() {
                let remaining =
                    session.peers.get(&peer).map(|p| p.leases.clone()).unwrap_or_default();
                session.broadcast(CollabMsg::LeaseGrant { peer, ids: remaining });
            }
        }

        CollabMsg::LeaseGrant { peer: owner, ids: granted } => {
            for info in session.peers.values_mut() {
                if info.id != owner {
                    info.leases.retain(|id| !granted.contains(id));
                }
            }
            if let Some(info) = session.peers.get_mut(&owner) {
                info.leases = granted;
            } else if owner == u64::MAX {
                // Reserved: a grant addressed to nobody releases the ids.
            }
        }

        CollabMsg::Control { allowed } => {
            if !session.is_guest() {
                return;
            }
            session.granted_control = allowed;
            session.note(if allowed {
                "the host has given you control — your edits are live"
            } else {
                "you are watching — your edits stay on this machine"
            });
        }

        CollabMsg::Ping => session.send_to(peer, CollabMsg::Pong),
        CollabMsg::Pong => {}

        // Everything that needs the world goes to the exclusive systems.
        other => inbox.queue.push_back((peer, other)),
    }
}
