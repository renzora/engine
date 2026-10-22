//! Put the editor's toasts on screen.
//!
//! Systems raise toasts through `renzora_ui::Toasts`, which is only a queue.
//! Its drawing went with egui and was never replaced, so every toast raised
//! since was dropped unseen. This hands each one to ember's queue, which draws
//! them (see `renzora_ember::widgets::ToastQueue`).

use bevy::prelude::*;
use renzora_ember::widgets::Tone;
use renzora_ui::toast::ToastLevel;

pub(crate) fn forward_toasts(
    toasts: Option<ResMut<renzora_ui::Toasts>>,
    mut shown: ResMut<renzora_ember::widgets::ToastQueue>,
) {
    let Some(mut toasts) = toasts else {
        return;
    };
    if toasts.is_empty() {
        return;
    }
    for t in toasts.drain() {
        let tone = match t.level {
            ToastLevel::Info => Tone::Info,
            ToastLevel::Success => Tone::Success,
            ToastLevel::Warning => Tone::Warn,
            ToastLevel::Error => Tone::Error,
        };
        shown.push_for(tone, t.message, t.duration);
    }
}
