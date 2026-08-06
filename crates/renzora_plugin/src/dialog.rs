//! Native file and folder pickers.
//!
//! Opt in with `features = ["dialog"]`. A domain module like
//! [`anim`](crate::anim), [`physics`](crate::physics) and [`http`](crate::http):
//! it rides [`CommandKind::Service`](crate::sys::CommandKind::Service) going out
//! and [`ReplySource`](crate::sys::ReplySource) coming back, so it adds **no**
//! boundary surface and does not move [`sys::VERSION_MINOR`].
//!
//! That it costs nothing is the point of the generic reply channel. Before it,
//! every domain the host had to answer — meshes, images, HTTP — paid an ABI bump
//! for a source of its own. This is the first that did not.
//!
//! ## Why the host has to do it
//!
//! Nothing stops a plugin linking `rfd` itself. It would then own a second copy
//! of the platform's dialog stack, opened from a thread the editor knows nothing
//! about, parented to no window — which on Windows means a modal that can fall
//! behind the editor with no way back to it. Going through the host gets the
//! right parent window and the editor's own last-used directory.
//!
//! ## Asking, then collecting
//!
//! Fire-and-tag, like [`http`](crate::http), and for the same reason: the
//! boundary has no callbacks, and a function pointer handed over would have to
//! survive a hot reload — exactly what generation-gating prevents. A dialog
//! whose requester was swapped out is simply never collected.
//!
//! ```ignore
//! use renzora_plugin::dialog::{DialogCommands, Dialogs, DialogFilter};
//!
//! const PICK_DOCS: u64 = 1;
//!
//! fn browse(mut commands: Commands) {
//!     commands.pick_folder(PICK_DOCS, "Choose a docs folder");
//! }
//!
//! fn collect(dialogs: Dialogs, mut cfg: ResMut<Config>) {
//!     if let Some(outcome) = dialogs.poll(PICK_DOCS) {
//!         match outcome.path() {
//!             Some(path) => cfg.docs = path.into(),
//!             // Cancelling is an ordinary outcome, not an error. It arrives as
//!             // a reply so a plugin can re-enable whatever it disabled.
//!             None => info("cancelled"),
//!         }
//!     }
//! }
//! ```

use crate::ecs::{Commands, Replies};
use crate::sys;
use alloc::string::String;
use alloc::vec::Vec;

/// Identifies this service in the host's queue.
pub const SERVICE: u64 = sys::service_id("renzora.dialog");

/// Which picker to open.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct DialogOp(pub u32);

#[allow(non_upper_case_globals)]
impl DialogOp {
    /// Choose one existing file.
    pub const OpenFile: Self = Self(0);
    /// Choose one existing directory.
    pub const OpenFolder: Self = Self(1);
    /// Choose a destination, existing or not.
    pub const SaveFile: Self = Self(2);

    pub const fn is_known(self) -> bool {
        self.0 < 3
    }

    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "OpenFile",
            1 => "OpenFolder",
            2 => "SaveFile",
            _ => "?",
        }
    }
}

impl core::fmt::Debug for DialogOp {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

/// What came back.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DialogResult(pub u32);

#[allow(non_upper_case_globals)]
impl DialogResult {
    /// The user chose something; the payload is the path.
    pub const Picked: Self = Self(0);
    /// The user dismissed the dialog. Empty payload.
    pub const Cancelled: Self = Self(1);
    /// The host could not open a dialog at all — a headless build, or no
    /// windowing. The payload is a message. Distinct from `Cancelled` because a
    /// plugin may want to fall back to typing a path rather than silently doing
    /// nothing.
    pub const Unavailable: Self = Self(2);

    pub const fn is_known(self) -> bool {
        self.0 < 3
    }
}

impl core::fmt::Debug for DialogResult {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self.0 {
            0 => "Picked",
            1 => "Cancelled",
            2 => "Unavailable",
            _ => "?",
        })
    }
}

/// Header of a dialog request; the title, then the filter spec, follow it in the
/// same buffer.
///
/// Both are length-prefixed rather than the trailing one being "the remainder",
/// unlike [`panel`](crate::panel): a filter spec can legitimately be empty, and
/// "empty" and "absent" have to stay distinguishable from a title that happens
/// to end in a separator.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DialogHeader {
    /// The plugin's own identifier, echoed back on the reply.
    pub tag: u64,
    pub title_len: u32,
    pub filter_len: u32,
}

/// A file-type filter: a label and the extensions it accepts.
///
/// Crosses the boundary as text — `"Images:png,jpg,webp"` — rather than as a
/// struct, because the count is variable and a `Vec` of anything cannot cross.
/// The host splits it; a plugin builds it with [`DialogFilter`].
#[derive(Clone, Debug, Default)]
pub struct DialogFilter(String);

impl DialogFilter {
    pub fn new() -> Self {
        Self(String::new())
    }

    /// Add one filter — a label and the extensions it accepts, without dots.
    ///
    /// ```ignore
    /// DialogFilter::new().add("Images", &["png", "jpg"]).add("All", &["*"])
    /// ```
    pub fn add(mut self, label: &str, extensions: &[&str]) -> Self {
        if !self.0.is_empty() {
            self.0.push(';');
        }
        // A label containing the separators would re-split wrongly on the host
        // side. Dropping the offending characters keeps the encoding total
        // rather than making this fallible for something no caller does on
        // purpose.
        for c in label.chars().filter(|c| *c != ':' && *c != ';' && *c != ',') {
            self.0.push(c);
        }
        self.0.push(':');
        for (i, e) in extensions.iter().enumerate() {
            if i > 0 {
                self.0.push(',');
            }
            for c in e.chars().filter(|c| *c != ':' && *c != ';' && *c != ',') {
                self.0.push(c);
            }
        }
        self
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The answer to one dialog request.
#[derive(Clone, Debug)]
pub struct DialogOutcome {
    pub result: DialogResult,
    /// The chosen path for [`DialogResult::Picked`], a message for
    /// [`DialogResult::Unavailable`], empty otherwise.
    pub value: String,
}

impl DialogOutcome {
    /// The chosen path, or `None` if the user cancelled or no dialog was
    /// available. The common case, and it collapses the two non-answers a caller
    /// usually treats the same.
    pub fn path(&self) -> Option<&str> {
        (self.result == DialogResult::Picked).then_some(self.value.as_str())
    }

    pub fn was_cancelled(&self) -> bool {
        self.result == DialogResult::Cancelled
    }
}

/// Collects dialog answers. A thin reading of [`Replies`] in this domain's
/// vocabulary.
pub struct Dialogs<'a>(Replies<'a>);

impl Dialogs<'_> {
    /// Take the answer for `tag`, if the user has finished with the dialog.
    ///
    /// `None` is the normal state — a dialog is open for as long as someone is
    /// looking at it, which is many thousands of frames. Delivered exactly once.
    pub fn poll(&self, tag: u64) -> Option<DialogOutcome> {
        let (op, data) = self.0.poll(SERVICE, tag)?;
        Some(DialogOutcome {
            result: DialogResult(op),
            // Lossy rather than refusing: a path the OS gave us that is not
            // valid UTF-8 should not make the answer unreachable.
            value: String::from_utf8_lossy(&data).into_owned(),
        })
    }
}

unsafe impl crate::ecs::SystemParam for Dialogs<'_> {
    fn declare(ctx: &mut crate::ecs::InitCtx, b: &mut crate::ecs::SystemBuilder) {
        <Replies as crate::ecs::SystemParam>::declare(ctx, b);
    }
    unsafe fn fetch(call: *const sys::SystemCall, cursor: &mut usize) -> Self {
        Dialogs(<Replies as crate::ecs::SystemParam>::fetch(call, cursor))
    }
}

/// Dialog methods on [`Commands`].
pub trait DialogCommands {
    /// Open a picker. The others are wrappers.
    fn dialog(&mut self, op: DialogOp, tag: u64, title: &str, filter: &DialogFilter) -> &mut Self;

    /// Choose one existing file. Poll [`Dialogs`] for `tag`.
    fn pick_file(&mut self, tag: u64, title: &str, filter: &DialogFilter) -> &mut Self;
    /// Choose one existing directory. Poll [`Dialogs`] for `tag`.
    fn pick_folder(&mut self, tag: u64, title: &str) -> &mut Self;
    /// Choose a destination path. Poll [`Dialogs`] for `tag`.
    fn pick_save_path(&mut self, tag: u64, title: &str, filter: &DialogFilter) -> &mut Self;
}

impl DialogCommands for Commands<'_> {
    fn dialog(&mut self, op: DialogOp, tag: u64, title: &str, filter: &DialogFilter) -> &mut Self {
        let filter = filter.as_str();
        let header = DialogHeader {
            tag,
            title_len: title.len() as u32,
            filter_len: filter.len() as u32,
        };
        let mut payload = Vec::with_capacity(
            core::mem::size_of::<DialogHeader>() + title.len() + filter.len(),
        );
        // SAFETY: `#[repr(C)]`, no pointers, no `Drop`.
        payload.extend_from_slice(unsafe {
            core::slice::from_raw_parts(
                (&header as *const DialogHeader).cast::<u8>(),
                core::mem::size_of::<DialogHeader>(),
            )
        });
        payload.extend_from_slice(title.as_bytes());
        payload.extend_from_slice(filter.as_bytes());
        self.call_service(SERVICE, op.0, &payload)
    }

    fn pick_file(&mut self, tag: u64, title: &str, filter: &DialogFilter) -> &mut Self {
        self.dialog(DialogOp::OpenFile, tag, title, filter)
    }

    fn pick_folder(&mut self, tag: u64, title: &str) -> &mut Self {
        // A folder picker has nothing to filter by, so the argument is not on
        // the wrapper rather than being an empty one every caller has to pass.
        self.dialog(DialogOp::OpenFolder, tag, title, &DialogFilter::new())
    }

    fn pick_save_path(&mut self, tag: u64, title: &str, filter: &DialogFilter) -> &mut Self {
        self.dialog(DialogOp::SaveFile, tag, title, filter)
    }
}
