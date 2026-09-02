//! The editor shell's extension points: panels, status-bar items and top-bar
//! buttons a plugin can contribute without linking the shell.
//!
//! Every registry here follows the same seam — the shell owns the chrome, the
//! plugin owns the thing behind it, and what crosses between them is data plus
//! an id. No callback goes over the boundary, which is what lets a plugin that
//! links no shell code put a button in the shell.

use bevy::prelude::*;

/// Per-panel metadata for the bevy_ui editor shell, keyed by panel id.
///
/// `renzora_shell` seeds this with each panel's title/icon at startup (its
/// `PANEL_META` table); plugins can add or override entries via
/// [`RenzoraShellExt::register_shell_panel`].
#[derive(Resource, Default)]
pub struct ShellPanelRegistry {
    pub panels: bevy::platform::collections::HashMap<String, ShellPanelInfo>,
}

#[derive(Clone, Default)]
pub struct ShellPanelInfo {
    pub title: String,
    /// Phosphor icon NAME (kebab-case), resolved to a glyph via
    /// `renzora_ember::font::icon_glyph` (empty if none).
    pub icon: String,
    pub category: String,
}

/// A progress bar drawn inside a status-bar segment, after its text.
///
/// Two kinds, because the honest answer is often "we don't know". A download
/// whose server sent no `Content-Length` has no fraction, and inventing one —
/// a curve that creeps toward 90% and waits — is a lie the bar tells for the
/// whole of the wait. [`Busy`](Self::Busy) says "working" and animates, which
/// is the true statement; the segment's *text* carries whatever real number
/// there is (bytes so far, files written).
#[derive(Clone, Copy, PartialEq)]
pub enum ShellStatusBar {
    /// Work in progress with no known total: an animated sweep.
    Busy,
    /// A known fraction, 0..=1.
    Fraction(f32),
}

/// One drawn piece of a bevy_ui status-bar item: an optional phosphor icon
/// (name *or* raw glyph) + text + color, and optionally a progress bar.
#[derive(Clone)]
pub struct ShellStatusSegment {
    pub icon: String,
    pub text: String,
    pub color: [u8; 3],
    pub bar: Option<ShellStatusBar>,
}

impl ShellStatusSegment {
    pub fn new(icon: impl Into<String>, text: impl Into<String>, color: [u8; 3]) -> Self {
        Self {
            icon: icon.into(),
            text: text.into(),
            color,
            bar: None,
        }
    }

    /// Draw a progress bar after the text.
    pub fn bar(mut self, bar: ShellStatusBar) -> Self {
        self.bar = Some(bar);
        self
    }
}

/// Which side of the status bar an item sits on.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ShellStatusAlign {
    Left,
    Right,
}

/// A bevy_ui status-bar item contributed by a plugin (the native counterpart of
/// the egui `StatusBarItem`). `render` runs each frame with `&World` and returns
/// the current segments, so live metrics update without re-registering.
pub struct ShellStatusItem {
    pub id: &'static str,
    pub align: ShellStatusAlign,
    pub order: i32,
    pub render: fn(&bevy::prelude::World) -> Vec<ShellStatusSegment>,
}

/// Registry of bevy_ui status-bar items. Any renzora plugin can push to it; the
/// shell renders them (no egui dependency).
#[derive(Resource, Default)]
pub struct ShellStatusRegistry {
    pub items: Vec<ShellStatusItem>,
}

/// An icon button a plugin contributes to the top bar, to the right of the
/// built-in controls.
///
/// The shell owns the chrome and the plugin owns the thing the button opens, so
/// neither can hold both halves: this is the seam. The shell draws whatever is
/// registered and reports presses through [`ShellActionInvoked`]; the plugin
/// reads its own id there and does whatever it likes. No callback crosses the
/// boundary, which is what lets a plugin that links no shell code put a button
/// in the shell.
pub struct ShellActionItem {
    /// Stable, unique. This is the string a plugin looks for in
    /// [`ShellActionInvoked`].
    pub id: &'static str,
    /// Phosphor icon NAME (kebab-case), resolved by the shell.
    pub icon: &'static str,
    /// The visible label, as a function rather than a string: it is built when
    /// the bar is built, which may be long after registration and after the
    /// user has changed language. `None` for an icon-only button.
    pub label: Option<fn() -> String>,
    /// Tint for the icon and the button's fill, as `rgb`. `None` takes the
    /// shell's muted default — the quiet treatment every other top-bar icon
    /// gets. Give it a colour when the button is somewhere to *go* rather than
    /// a toggle, and pick one no other chip in the bar is using: two tinted
    /// pills of the same hue side by side read as one control in two halves.
    pub color: Option<[u8; 3]>,
    /// The tooltip, as a function rather than a string, for the same reason as
    /// `label`.
    pub tooltip: fn() -> String,
    /// Left-to-right order among the contributed buttons. Ties keep
    /// registration order.
    pub order: i32,
}

/// Registry of plugin-contributed top-bar buttons. Any plugin can push to it;
/// the shell renders them beside the gear.
#[derive(Resource, Default)]
pub struct ShellActionRegistry {
    pub items: Vec<ShellActionItem>,
}

/// A shell action was invoked.
///
/// The top bar's button writes this, but it is deliberately not *only* the top
/// bar's: anything that should open the same thing writes the same id, which is
/// how the asset browser's Import button reaches the Marketplace without either
/// crate knowing about the other. The plugin that registered the id reads the
/// message and does the work.
#[derive(bevy::prelude::Message, Clone, Copy)]
pub struct ShellActionInvoked(pub &'static str);

impl ShellActionInvoked {
    /// Invoke a shell action from an exclusive system.
    ///
    /// A no-op when nothing has registered the id — the message type is only
    /// added by `register_shell_action`, so an editor built without the plugin
    /// that owns this action simply has nowhere to send it, which is the right
    /// outcome and not an error.
    pub fn invoke(world: &mut bevy::prelude::World, id: &'static str) {
        if let Some(mut messages) = world.get_resource_mut::<bevy::ecs::message::Messages<Self>>() {
            messages.write(Self(id));
        }
    }
}

/// The Marketplace overlay's action id.
///
/// Here rather than in the plugin that owns it because the asset browser's
/// Import menu opens the same thing, and neither crate should depend on the
/// other to agree on a string.
pub const ACTION_MARKETPLACE: &str = "marketplace.open";

/// Relaunch this executable with the same arguments and exit.
///
/// Here in the contract crate because more than one thing needs it and none of
/// them should own it: first-run setup restarts after building plugins, and
/// installing a plugin from the marketplace has to restart to load it — plugins
/// are opened once, during `App` assembly, so a new one on disk is not a new one
/// in the process.
///
/// The replacement is spawned **detached** rather than waited on. This process
/// is finished, and holding it open to parent the new one would leave a
/// redundant entry in the task list for the whole of the next session. It also
/// matters on Windows, where a plugin file cannot be replaced while a process
/// holds it open: the successor starts as this one is leaving.
#[cfg(not(target_arch = "wasm32"))]
pub fn restart_process() -> ! {
    if let Ok(exe) = std::env::current_exe() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let _ = std::process::Command::new(exe).args(args).spawn();
    }
    std::process::exit(0)
}

/// Overrides the status bar's left-hand **"Ready"** label. The host owns the
/// status bar, so a plugin can't replace that label by registering a status item
/// (those only *append*). Instead it writes here: `label = Some(text)` swaps the
/// "Ready" text for `text` (in `color`, falling back to the muted default when
/// `None`); `label = None` restores "Ready". This is how the auto-save plugin
/// shows its "Auto save in Ns" countdown in place of "Ready".
#[derive(Resource, Default)]
pub struct ShellReadyStatus {
    pub label: Option<String>,
    pub color: Option<[u8; 3]>,
}

/// The bevy-native editor-extension API. A renzora plugin (full ECS access) uses
/// this to add panels + status-bar items to the bevy_ui shell directly — no
/// egui, no bridge — mirroring how `#[derive]` component macros let plugins add
/// their own data.
pub trait RenzoraShellExt {
    /// Register a panel's metadata (title/icon/category) for the dock + Add-Panel
    /// picker. The panel's *content* is registered separately via
    /// `renzora_ember`'s `register_panel_content`.
    fn register_shell_panel(
        &mut self,
        id: impl Into<String>,
        title: impl Into<String>,
        icon: impl Into<String>,
        category: impl Into<String>,
    ) -> &mut Self;

    /// Register a status-bar item.
    fn register_shell_status_item(&mut self, item: ShellStatusItem) -> &mut Self;

    /// Register a top-bar icon button. Pressing it writes a
    /// [`ShellActionInvoked`] carrying the id.
    fn register_shell_action(&mut self, item: ShellActionItem) -> &mut Self;
}

impl RenzoraShellExt for bevy::app::App {
    fn register_shell_panel(
        &mut self,
        id: impl Into<String>,
        title: impl Into<String>,
        icon: impl Into<String>,
        category: impl Into<String>,
    ) -> &mut Self {
        self.init_resource::<ShellPanelRegistry>();
        self.world_mut()
            .resource_mut::<ShellPanelRegistry>()
            .panels
            .insert(
                id.into(),
                ShellPanelInfo {
                    title: title.into(),
                    icon: icon.into(),
                    category: category.into(),
                },
            );
        self
    }

    fn register_shell_status_item(&mut self, item: ShellStatusItem) -> &mut Self {
        self.init_resource::<ShellStatusRegistry>();
        self.world_mut()
            .resource_mut::<ShellStatusRegistry>()
            .items
            .push(item);
        self
    }

    fn register_shell_action(&mut self, item: ShellActionItem) -> &mut Self {
        self.init_resource::<ShellActionRegistry>();
        self.add_message::<ShellActionInvoked>();
        self.world_mut()
            .resource_mut::<ShellActionRegistry>()
            .items
            .push(item);
        self
    }
}

/// Panel ids that have a **bevy-native** (ember) content renderer — i.e. their
/// own crate builds the panel into the dock leaf and keeps it in sync, instead
/// of the shell's placeholder/`content_dispatch`. The shell skips these ids so
/// the two never fight over the same `content` entity.
#[derive(Resource, Default)]
pub struct NativePanelIds(pub bevy::platform::collections::HashSet<String>);

/// Lets a panel crate declare it owns the bevy_ui rendering for an id.
pub trait NativePanelExt {
    /// Mark `id` as having a native ember content renderer (order-independent).
    fn register_native_panel(&mut self, id: &str) -> &mut Self;
}

impl NativePanelExt for App {
    fn register_native_panel(&mut self, id: &str) -> &mut Self {
        self.init_resource::<NativePanelIds>();
        if let Some(mut ids) = self.world_mut().get_resource_mut::<NativePanelIds>() {
            ids.0.insert(id.to_string());
        }
        self
    }
}
