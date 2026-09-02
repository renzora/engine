//! The events a Rust script can receive, beyond its per-frame `update`.
//!
//! # Why this exists
//!
//! Lua scripts get a lifecycle: `on_ready`, `on_ui`, `on_rpc`,
//! `on_scene_loaded`, `on_animation_event`, `on_http`, `on_player_joined` /
//! `on_player_left`. Rust scripts got one entry point, `update`, and nothing
//! else — not because the events were unavailable, but because of how they are
//! delivered.
//!
//! Every hook is dispatched through [`ScriptBackend`], whose methods take a
//! `ScriptContext` and return `ScriptCommand`s. That interface deliberately has
//! no `World`, which is the one thing a Rust script exists for. So the Rust
//! backend returned "no commands" for every hook and the real per-frame call
//! happened somewhere else entirely — an exclusive system with `&mut World` that
//! only ever knew how to call `update`.
//!
//! The consequence was not a missing convenience. A loading screen has to run
//! from a global scene and be told when the incoming scene arrived; with no
//! `on_scene_loaded` reaching Rust, and the event inbox drained by the Lua
//! dispatcher before a Rust script could look, that feature was Lua-only. So
//! were rebindable input and script props. Four separate "Lua can, Rust can't"
//! gaps, all of them this one boundary.
//!
//! # The shape
//!
//! A script optionally exports a second entry point beside `update`, and gets
//! the event as a typed value:
//!
//! ```ignore
//! fn update(ctx: &mut ScriptCtx) { /* every frame */ }
//!
//! fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
//!     match hook {
//!         ScriptHook::Ready => { /* first frame, once */ }
//!         ScriptHook::SceneLoaded { path, .. } => { /* a scene arrived */ }
//!         ScriptHook::Ui { name, .. } => { /* a button was pressed */ }
//!         _ => {}
//!     }
//! }
//!
//! renzora::script!(update, hooks = hooks);
//! ```
//!
//! One function rather than eight named exports: the payloads differ, so a
//! script that cares about two events writes two match arms instead of two
//! signatures, and adding a ninth event later is a new variant rather than a new
//! symbol every existing script has to grow.
//!
//! The payload types are the same ones the inboxes already carry
//! ([`UiCallback`](crate::UiCallback), [`IncomingRpc`](crate::IncomingRpc),
//! [`SceneEvent`](crate::SceneEvent)…), borrowed rather than copied, because
//! both sides of this boundary link the same contract crate.

use bevy::prelude::Entity;
use std::collections::HashMap;

use crate::core::ScriptActionValue;

/// An event delivered to a script's hook entry point.
///
/// `#[non_exhaustive]` so a new event is an additive change: a script matching
/// on this must carry a `_ => {}` arm, and gains nothing to fix when the engine
/// learns a new hook.
#[non_exhaustive]
#[derive(Debug)]
pub enum ScriptHook<'a> {
    /// First frame this script runs on this entity, before its first `update`.
    ///
    /// The Rust equivalent of Lua's `on_ready`. Without it every script grew a
    /// `started: bool` component and a branch to set it.
    Ready,

    /// A markup callback fired — `on_press="..."` and friends.
    Ui {
        /// The callback name from the markup attribute.
        name: &'a str,
        /// `tag:`-prefixed attributes on the node, decoded.
        args: &'a HashMap<String, ScriptActionValue>,
        /// The UI node that fired it, so a handler can target the widget.
        source: Entity,
    },

    /// A networked RPC arrived.
    Rpc {
        name: &'a str,
        args: &'a HashMap<String, ScriptActionValue>,
        /// Sender's peer id; 0 is the server or a local call.
        from: u64,
    },

    /// A scene finished loading, or failed to.
    ///
    /// Only scripts the load did **not** destroy hear this — in practice the
    /// ones in an autoload (global) scene, since everything in the outgoing
    /// scene is despawned partway through. That is what makes a loading screen
    /// possible, and it is the hook that was missing.
    SceneLoaded {
        /// Scene path as the load was requested, project-relative.
        path: &'a str,
        /// `None` on success; the failure reason otherwise.
        error: Option<&'a str>,
    },

    /// Animation playback crossed a clip marker.
    AnimationEvent {
        name: &'a str,
        /// The animator entity that fired it.
        source: Entity,
    },

    /// A background HTTP request completed.
    Http {
        callback: &'a str,
        status: u16,
        body: &'a str,
    },

    /// A peer connected. Server-authoritative: only the host sees these.
    PlayerJoined { id: u64 },
    /// A peer disconnected.
    PlayerLeft { id: u64 },

    /// Time to repaint this entity's draw surface.
    ///
    /// The Rust counterpart to Lua's `on_draw(g)`. Delivered only to entities
    /// that actually have a surface — a `<canvas>` node in the markup naming
    /// them — with the surface's current size, so a script can lay out against
    /// real pixels rather than guessing.
    ///
    /// Immediate mode: the list is rebuilt from nothing every frame, so a script
    /// draws its whole picture each time and never has to remove anything.
    Draw {
        /// Surface size in pixels.
        width: f32,
        height: f32,
    },
}

impl ScriptHook<'_> {
    /// A short name for logs and diagnostics.
    pub fn name(&self) -> &'static str {
        match self {
            ScriptHook::Ready => "ready",
            ScriptHook::Ui { .. } => "ui",
            ScriptHook::Rpc { .. } => "rpc",
            ScriptHook::SceneLoaded { .. } => "scene_loaded",
            ScriptHook::AnimationEvent { .. } => "animation_event",
            ScriptHook::Http { .. } => "http",
            ScriptHook::PlayerJoined { .. } => "player_joined",
            ScriptHook::PlayerLeft { .. } => "player_left",
            ScriptHook::Draw { .. } => "draw",
        }
    }

    /// Is this event broadcast to every live script, rather than aimed at one
    /// entity?
    ///
    /// [`Ready`](Self::Ready) and [`Draw`](Self::Draw) are per-entity; the rest
    /// are broadcast. The dispatcher uses this to decide whether to filter.
    pub fn is_broadcast(&self) -> bool {
        !matches!(self, ScriptHook::Ready | ScriptHook::Draw { .. })
    }
}
