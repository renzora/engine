//! Collaborative editing — two editors, one project, live.
//!
//! One person hosts the session; anyone they invite connects to it and sees the
//! scene as it is being built, with the host able to hand over control so their
//! collaborator can build alongside them from their own editor.
//!
//! ## The shape of it
//!
//! | Module | Job |
//! |---|---|
//! | [`protocol`] | The wire vocabulary and its framing |
//! | [`link`] | One connection, abstracted away from the transport under it |
//! | [`tcp`] | Direct connection — a port on the host's machine |
//! | [`relay`] | Connection through renzora.com, for hosts nobody can reach |
//! | [`session`] | Who is connected, in which role |
//! | [`identity`] | Names for entities that mean the same thing on both machines |
//! | [`sync`] | Noticing what changed, describing it, applying it |
//! | [`files`] | Getting the project itself to someone who has never had it |
//! | [`lease`] | Keeping two people off the same object |
//! | [`presence`] | Cameras and selections, so it feels like a room |
//! | [`online`] | Creating and finding relay rooms over the site's REST API |
//!
//! ## Three decisions worth knowing before changing anything here
//!
//! **The host is the authority.** Not because peer-to-peer merging is
//! impossible, but because the alternative is a distributed consensus problem
//! sitting underneath a level editor. One machine owns the document; everyone
//! else's edits are proposals it applies and relays. It also gives the feature
//! an honest answer to "whose version is saved": the host's, because the host is
//! the one with the project open.
//!
//! **State is replicated, not operations.** [`sync`] explains this at length. In
//! short: a tool written next year replicates without knowing this module
//! exists, which is the only version of this feature that survives the editor
//! continuing to grow.
//!
//! **The transport is replaceable, and there are two.** [`tcp`] connects two
//! machines directly, which needs the host to be reachable and so only really
//! works on a LAN. [`relay`] sends both editors *outward* to renzora.com, which
//! works between any two people and is what makes this "invite a friend" rather
//! than "invite a colleague on your subnet". Everything above [`link`] is
//! written in messages rather than sockets, which is why the second one was a
//! new module rather than a rewrite — [`session`] cannot tell them apart.
//!
//! ## What this does not do yet
//!
//! Worth knowing before relying on it:
//!
//! - **Only the scene replicates.** Terrain sculpts, tilemap paint, material and
//!   blueprint graphs, and script buffers are not synced; they carry data that
//!   does not fit an entity snapshot and each needs its own channel.
//! - **Conflicts are last-writer-wins.** [`lease`] keeps people apart rather
//!   than merging what happens when they aren't.
//! - **Undo is local.** Ctrl+Z undoes your own actions, and the result
//!   replicates as an ordinary edit. Nobody can undo someone else's work, which
//!   is the safe half of the behaviour; a shared timeline is not implemented.
//! - **A direct session is not encrypted.** Plain TCP on a local network — do not
//!   forward the port to the open internet and expect privacy. A relayed session
//!   is WSS end to end, at the cost of passing through a third machine.

// Everything below the panel needs a socket and worker threads, neither of
// which a browser tab has. The *plugin* still exists on wasm — the generated
// plugin list names it unconditionally — it simply installs nothing, which is
// the same shape `renzora_update` uses for the same reason.
#[cfg(not(target_arch = "wasm32"))]
pub mod files;
#[cfg(not(target_arch = "wasm32"))]
pub mod identity;
#[cfg(not(target_arch = "wasm32"))]
pub mod lease;
#[cfg(not(target_arch = "wasm32"))]
pub mod link;
#[cfg(not(target_arch = "wasm32"))]
pub mod online;
#[cfg(not(target_arch = "wasm32"))]
pub mod panel;
#[cfg(not(target_arch = "wasm32"))]
pub mod presence;
pub mod protocol;
#[cfg(not(target_arch = "wasm32"))]
pub mod relay;
#[cfg(not(target_arch = "wasm32"))]
pub mod session;
#[cfg(not(target_arch = "wasm32"))]
pub mod sync;
#[cfg(not(target_arch = "wasm32"))]
pub mod tcp;

use bevy::prelude::*;

/// The port a session listens on unless the user changes it. Chosen to sit well
/// clear of the game networking's own default so an editor session and a
/// playtest can run on one machine without a collision.
pub const DEFAULT_PORT: u16 = 7700;

#[derive(Default)]
pub struct CollabPlugin;

impl Plugin for CollabPlugin {
    #[cfg(target_arch = "wasm32")]
    fn build(&self, _app: &mut App) {
        info!("[editor] CollabPlugin — no session support on the web");
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn build(&self, app: &mut App) {
        info!("[editor] CollabPlugin");

        app.init_resource::<session::CollabSession>()
            .init_resource::<session::CollabInbox>()
            .init_resource::<identity::CollabIds>()
            .init_resource::<sync::SyncTracker>()
            .init_resource::<files::FileSync>()
            .init_resource::<lease::ClaimedSelection>()
            .init_resource::<presence::PresenceTimer>()
            .init_resource::<online::OnlineRequests>()
            .add_plugins(panel::CollabPanel);

        // Ordered deliberately, because each step depends on the last having run
        // this frame: the pump turns socket traffic into a queue, the apply
        // drains that queue into the world, and only then is it meaningful to
        // ask what changed — scanning first would send a peer their own edit
        // back to them a frame later.
        // Gated on an active session, not merely early-returning inside each
        // system. Two of these take `&mut World`, and an exclusive system is a
        // scheduler sync point whether or not its body does anything — an editor
        // that never opens a session should not pay one every frame for a
        // feature it is not using. The panel's own systems are registered
        // separately (through `PanelScope`) and stay ungated, because the button
        // that *starts* a session obviously cannot require one.
        app.add_systems(
            Update,
            (
                session::pump_links,
                sync::apply_inbox,
                sync::scan_and_send,
                files::poll_compare,
                lease::claim_selection,
                presence::broadcast_presence,
            )
                .chain()
                .run_if(in_session),
        );

        // Ungated: this is how a session *starts*, so it cannot require one.
        // Cheap — an empty channel drain — and it is the only always-on system
        // this plugin adds.
        app.add_systems(Update, online::poll);

        app.add_systems(PostUpdate, presence::draw_peers.run_if(in_session));
    }
}

/// Whether a session is running at all.
#[cfg(not(target_arch = "wasm32"))]
fn in_session(session: Option<Res<session::CollabSession>>) -> bool {
    session.is_some_and(|s| s.is_active())
}

// Editor scope: a shipped game has no document to collaborate on, and the panel
// this registers has no shell to live in.
renzora::add!(CollabPlugin, Editor);
