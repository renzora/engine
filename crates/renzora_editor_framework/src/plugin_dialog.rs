//! The file-dialog half of the standalone-plugin boundary.
//!
//! Same construction as the animation, physics and HTTP bridges:
//! `renzora_plugin::sys` carries opaque bytes, `renzora_plugin::dialog` is the
//! Bevy-free vocabulary both sides compile, and this module is the engine side
//! that claims the service.
//!
//! It lives here rather than in `renzora_plugin` for the reason every bridge
//! does — that crate cannot depend on an engine crate, and it certainly cannot
//! depend on `rfd`. It lives in *this* crate specifically because a native
//! picker is editor furniture: it needs a parent window and a desktop, neither
//! of which a shipped game running headless has. A runtime-scope plugin that
//! asks for one in a build without this bridge has its call parked and never
//! answered, which is the same outcome HTTP has without a client.
//!
//! ## Blocking, deliberately
//!
//! The picker runs on the main thread and stalls the frame while it is open,
//! matching `renzora_import_ui::pick_importable_files` and every other dialog in
//! the editor. Native dialogs genuinely prefer the UI thread — on Windows the
//! modal is parented to it, and running one off-thread is how you get a dialog
//! that falls behind the editor with no way back. The frame is stalled either
//! way from the user's point of view: they are looking at a file picker.
//!
//! The plugin still sees an asynchronous API, because the reply lands in the
//! queue and is collected on a later frame. That is not a pretence — a plugin
//! must not assume the answer arrives in any particular frame, and going through
//! the queue is what keeps it honest.

use bevy::prelude::*;

use renzora_plugin::dialog::{DialogHeader, DialogOp, DialogResult};
use renzora_plugin::host::{PluginServiceCalls, PluginServiceReplies, ServiceReply};

/// Decode parked dialog calls, run the picker, and queue the answer.
fn drain_plugin_dialogs(
    mut parked: ResMut<PluginServiceCalls>,
    mut replies: ResMut<PluginServiceReplies>,
) {
    let calls = parked.take(renzora_plugin::dialog::SERVICE);
    for call in calls {
        let hdr_len = size_of::<DialogHeader>();
        if call.payload.len() < hdr_len {
            warn!("[dialog] plugin sent {} bytes for a header", call.payload.len());
            continue;
        }
        // SAFETY: length checked, and `DialogHeader` is `#[repr(C)]` plain data.
        let hdr = unsafe { call.payload.as_ptr().cast::<DialogHeader>().read_unaligned() };

        // Untrusted lengths — they crossed from another compilation unit, and a
        // bad pair would slice past the end. Exact, not "within": trailing bytes
        // mean the sender and this bridge disagree about the payload's shape.
        let title_end = hdr_len.saturating_add(hdr.title_len as usize);
        let filter_end = title_end.saturating_add(hdr.filter_len as usize);
        if filter_end != call.payload.len() {
            warn!(
                "[dialog] plugin request claims {} + {} bytes but sent {}",
                hdr.title_len,
                hdr.filter_len,
                call.payload.len() - hdr_len
            );
            continue;
        }

        let title = String::from_utf8_lossy(&call.payload[hdr_len..title_end]).into_owned();
        let filter = String::from_utf8_lossy(&call.payload[title_end..filter_end]).into_owned();

        let op = DialogOp(call.op);
        if !op.is_known() {
            warn!("[dialog] plugin asked for picker {}, which this build has not got", call.op);
            continue;
        }

        let (result, value) = run_picker(op, &title, &filter);
        replies.0.push(ServiceReply {
            service: renzora_plugin::dialog::SERVICE,
            tag: hdr.tag,
            op: result.0,
            payload: value.into_bytes(),
        });
    }
}

/// Open the picker and wait for it. `(result, path-or-message)`.
#[cfg(not(target_arch = "wasm32"))]
fn run_picker(op: DialogOp, title: &str, filter: &str) -> (DialogResult, String) {
    let mut dialog = rfd::FileDialog::new();
    if !title.is_empty() {
        dialog = dialog.set_title(title);
    }
    // `"Images:png,jpg;All:*"` — see `renzora_plugin::dialog::DialogFilter`, which
    // strips the separators out of labels and extensions so this cannot mis-split.
    for spec in filter.split(';').filter(|s| !s.is_empty()) {
        let Some((label, exts)) = spec.split_once(':') else {
            continue;
        };
        let exts: Vec<&str> = exts.split(',').filter(|e| !e.is_empty()).collect();
        if !exts.is_empty() {
            dialog = dialog.add_filter(label, &exts);
        }
    }

    let picked = match op {
        DialogOp::OpenFolder => dialog.pick_folder(),
        DialogOp::SaveFile => dialog.save_file(),
        // `OpenFile` and anything `is_known` let through that is not one of the
        // two above. A new op reaching here means this match and `is_known`
        // disagree, and opening a file picker is the safe reading.
        _ => dialog.pick_file(),
    };

    match picked {
        Some(path) => (DialogResult::Picked, path.to_string_lossy().into_owned()),
        // Cancelling is an ordinary outcome and still gets a reply — a plugin
        // that disabled a button while the dialog was open needs to hear back.
        None => (DialogResult::Cancelled, String::new()),
    }
}

/// wasm has no native picker. Answering `Unavailable` rather than `Cancelled`
/// lets a plugin fall back to typing a path instead of concluding the user said
/// no.
#[cfg(target_arch = "wasm32")]
fn run_picker(_op: DialogOp, _title: &str, _filter: &str) -> (DialogResult, String) {
    (
        DialogResult::Unavailable,
        "no native file dialog on this platform".into(),
    )
}

/// Installs the bridge.
pub struct PluginDialogBridge;

impl Plugin for PluginDialogBridge {
    fn build(&self, app: &mut App) {
        // The generic reply queue. `PluginServiceCalls` is created lazily by the
        // host the first time a plugin calls anything, but nothing creates this
        // one — the poller treats its absence as "no replies", so a bridge that
        // produces them has to put it there.
        app.init_resource::<PluginServiceReplies>();
        app.add_systems(Update, drain_plugin_dialogs);
    }
}
