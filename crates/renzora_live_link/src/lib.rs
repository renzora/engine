//! Live link: the editor and the game Play is running, kept in step.
//!
//! Play runs the game as its own process, so the editor's world and the game's
//! are two worlds. The live link joins them:
//!
//! - **Editor to game.** Edit a component in the inspector, move something with
//!   the gizmo, add or delete an entity, and the running game gets the change.
//! - **Game to editor.** What the game does (the player walking, a crate
//!   falling, a bullet spawned) shows up in the editor's own world, so the
//!   hierarchy, the inspector and the viewport show the game as it is, with no
//!   second set of panels to build. When the game stops, the editor puts back
//!   everything the game changed and keeps everything the user did.
//! - **Tools to game.** Keys, gamepad input and screenshots, so an agent can
//!   play the game it is helping to build. See [`renzora::GameLink`].
//!
//! # How the two worlds agree
//!
//! An `Entity` means nothing outside its world, so entities are matched by
//! [`renzora::SceneEntityId`], which a save gives every scene entity and which
//! the side that creates a new one assigns on the spot. Components travel by
//! reflection: the game is the same executable as the editor, so both share one
//! type registry, and any component a scene can save can cross, a user's own
//! included. [`tracker`] decides what changed; [`apply`] makes the other side
//! match.
//!
//! The editor listens on a loopback port, and Play passes the port and a
//! one-off token to the game on its command line. A game launched any other way
//! opens no connection.
//!
//! Not on the web, which has neither processes nor sockets.

use serde::{Deserialize, Serialize};

pub use renzora::GameInput;

#[cfg(not(target_arch = "wasm32"))]
pub mod apply;
#[cfg(not(target_arch = "wasm32"))]
mod b64;
#[cfg(not(target_arch = "wasm32"))]
mod client;
#[cfg(not(target_arch = "wasm32"))]
pub mod tracker;
#[cfg(test)]
mod tests;

#[cfg(not(target_arch = "wasm32"))]
pub use b64::{decode as decode_base64, encode as encode_base64};

/// One line on the wire, newline-delimited JSON.
///
/// JSON for the envelope because it is trivially line-framed and readable when
/// debugging with a socket tool; RON for component values because that is the
/// encoding the scene format already uses for them, so a value that saves is a
/// value that travels.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LinkMessage {
    /// The game's first line: the token it was launched with.
    Hello { token: String },
    /// A new scene entity, `parent` being its parent's id when it has one in
    /// the scene. Its components follow as [`Self::Set`]s.
    Spawn { id: u64, parent: Option<u64>, name: String },
    /// Entity `id` and its children are gone.
    Despawn { id: u64 },
    /// Insert or replace component `ty` (a reflected type path) on entity `id`
    /// with `value`, a RON-serialized reflected value.
    Set { id: u64, ty: String, value: String },
    /// Remove component `ty` from entity `id`.
    Remove { id: u64, ty: String },
    /// Editor to game: press or release something.
    Input(GameInput),
    /// Editor to game: capture the window.
    ScreenshotRequest { req: u64 },
    /// Game to editor: the capture, as base64 PNG, or why there is none.
    Screenshot { req: u64, png: Result<String, String> },
}

#[derive(Default)]
pub struct LiveLinkPlugin;

impl bevy::app::Plugin for LiveLinkPlugin {
    fn build(&self, _app: &mut bevy::app::App) {
        #[cfg(not(target_arch = "wasm32"))]
        client::install(_app);
    }
}

renzora::add!(LiveLinkPlugin, Runtime);
