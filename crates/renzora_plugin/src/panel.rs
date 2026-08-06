//! Replacing a panel's contents after registration.
//!
//! [`App::add_panel`](crate::ecs::App::add_panel) takes the markup once, at
//! init, which is enough for a panel that is a fixed arrangement of widgets and
//! not enough for anything that has to *change*: a list that grows, a reply that
//! streams in, a form whose fields depend on what is selected. This module is
//! the other half — a running system can hand the host new BSN for a panel it
//! already registered, and the panel redraws.
//!
//! ## Why this is not a new ABI function
//!
//! It rides [`CommandKind::Service`](crate::sys::CommandKind::Service), the
//! generic channel, exactly as [`anim`](crate::anim), [`physics`](crate::physics)
//! and [`http`](crate::http) do. That is the established split and it is worth
//! restating: the boundary carries opaque bytes, a *domain* gives them meaning,
//! and adding a domain moves this crate's semver rather than
//! [`sys::VERSION_MINOR`]. A panel plugin should not have to declare a minimum
//! ABI that also encodes animation's history.
//!
//! ## Why it redraws without new host machinery
//!
//! The host already re-renders a panel whose markup no longer matches what is on
//! screen — that is how hot reload redraws a plugin's panel after a rebuild. So
//! the engine side of this is only "write the new markup where that comparison
//! will see it"; the diff, the BSN re-parse, the action-thunk rebind and the
//! respawn are the path that already existed. A malformed BSN is refused and the
//! old panel kept, for the same reason it is on reload: a half-built string is a
//! normal intermediate state, and blanking a panel on every bad frame would be
//! worse than showing a stale one.
//!
//! ```ignore
//! use renzora_plugin::panel::PanelCommands;
//!
//! fn redraw(mut commands: Commands, log: Res<ChatLog>) {
//!     let mut markup = String::from("Node { flex_direction: Column } Children [");
//!     for line in &log.lines {
//!         markup.push_str(&format!("Text({:?}),", line));
//!     }
//!     markup.push(']');
//!     commands.set_panel_content("mychat", &markup);
//! }
//! ```

use crate::ecs::Commands;
use crate::sys;
use alloc::vec::Vec;

/// Identifies this service in the host's queue.
pub const SERVICE: u64 = sys::service_id("renzora.panel");

/// Which panel operation a call carries.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PanelOp(pub u32);

#[allow(non_upper_case_globals)]
impl PanelOp {
    /// Replace a registered panel's markup.
    pub const SetContent: Self = Self(0);

    pub const fn is_known(self) -> bool {
        self.0 < 1
    }

    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "SetContent",
            _ => "?",
        }
    }
}

impl core::fmt::Debug for PanelOp {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

/// Header of a panel service payload; the id bytes then the markup bytes follow
/// it in the same buffer.
///
/// Only the id is length-prefixed — the markup is "whatever is left", so a panel
/// whose BSN happens to contain the id string cannot be mis-split. Same shape as
/// [`HttpHeader`](crate::http::HttpHeader), for the same reason: both fields are
/// genuinely variable, unlike an animation clip name where a cap is reasonable.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PanelContentHeader {
    pub id_len: u32,
}

/// Panel methods on [`Commands`].
pub trait PanelCommands {
    /// Replace the contents of the panel registered under `id`.
    ///
    /// `markup` is BSN source, the same text
    /// [`bsn!`](crate::bsn) produces — but a `&str` rather than a
    /// [`Scene`](crate::ecs::Scene), because `Scene` holds a `&'static str` and
    /// the whole point here is content built at run time.
    ///
    /// Applied at the end of the frame, like every other command. Sending the
    /// markup a panel already has is cheap: the host compares before it parses,
    /// so an unchanged string costs a string comparison and nothing else — which
    /// means a system may call this unconditionally every frame rather than
    /// tracking dirtiness itself.
    ///
    /// An `id` that was never registered, or BSN that does not parse, is
    /// reported by the host and leaves the panel as it was.
    fn set_panel_content(&mut self, id: &str, markup: &str) -> &mut Self;
}

impl PanelCommands for Commands<'_> {
    fn set_panel_content(&mut self, id: &str, markup: &str) -> &mut Self {
        let header = PanelContentHeader {
            id_len: id.len() as u32,
        };
        let mut payload = Vec::with_capacity(
            core::mem::size_of::<PanelContentHeader>() + id.len() + markup.len(),
        );
        // SAFETY: `#[repr(C)]`, no pointers, no `Drop`.
        payload.extend_from_slice(unsafe {
            core::slice::from_raw_parts(
                (&header as *const PanelContentHeader).cast::<u8>(),
                core::mem::size_of::<PanelContentHeader>(),
            )
        });
        payload.extend_from_slice(id.as_bytes());
        payload.extend_from_slice(markup.as_bytes());
        self.call_service(SERVICE, PanelOp::SetContent.0, &payload)
    }
}
