//! The marketplace's toasts, handed to ember's queue to be shown.
//!
//! This module used to draw its own stack of cards bottom-right. The editor's
//! toasts now all go through `renzora_ember::widgets::ToastQueue`, which owns
//! the one stack in that corner, so this keeps only the queue its call sites
//! already push to and forwards it on.

use std::collections::VecDeque;

use bevy::prelude::*;
pub(crate) use renzora_ember::widgets::Tone;

pub(crate) struct ToastRequest {
    pub tone: Tone,
    pub message: String,
}

/// Toasts raised by the marketplace this frame.
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

/// Hand this frame's toasts to ember, which shows them.
pub(crate) fn forward_toasts(
    mut queue: ResMut<ToastQueue>,
    mut shown: ResMut<renzora_ember::widgets::ToastQueue>,
) {
    for req in queue.pending.drain(..) {
        shown.push(req.tone, req.message);
    }
}
