//! `register_workspace` — let a plugin contribute a named workspace layout.
//!
//! The counterpart to [`register_panel_content`](crate::panel::RegisterPanelContent).
//! That call says "here is a panel and how to build it"; this one says "here is
//! an arrangement of panels worth offering as a workspace", and the two together
//! are what a plugin needs to add a whole editor mode rather than a single tab.
//!
//! ```ignore
//! app.register_workspace("Debug", DockTree::Split {
//!     direction: SplitDirection::Horizontal,
//!     ratio: 0.2,
//!     first: Box::new(DockTree::leaf("performance")),
//!     second: Box::new(DockTree::leaf("ecs_stats")),
//! });
//! ```
//!
//! # Why this is a queue and not a direct write
//!
//! The obvious implementation is to reach for the editor's layout list and push
//! into it. A plugin cannot: that list is `renzora_shell`'s `ShellLayouts`, and
//! a native plugin links `bevy`, `renzora` and `renzora_ember` and nothing else
//! (see `Externs` in `renzora_plugin_build`). Widening that list to a fourth
//! shared image would make every plugin's ABI depend on the editor's shell
//! crate, which is a large price for one registration.
//!
//! So this crate holds the request and `renzora_shell` drains it into the dock
//! it draws. That is the same shape `PluginPanels` and `PluginAudioBackend`
//! already use for the C-ABI boundary: the guest describes what it wants in
//! vocabulary it can reach, and the crate that owns the real structure performs
//! it.
//!
//! It also means registration order stops mattering. A plugin's `build` runs
//! whenever the loader gets to it, which may be before or after the shell has
//! restored its saved layouts from disk; a queue drained every frame is correct
//! either way, where a direct write would land in a resource about to be
//! overwritten.
//!
//! # Replacing rather than duplicating
//!
//! A workspace is keyed by name. Registering "Debug" twice leaves one "Debug",
//! the later registration winning, because the alternative is a layout switcher
//! that grows a duplicate entry every time a plugin reloads. Native plugins are
//! rebuilt and re-initialised whenever their source changes, so that is the
//! common case rather than the exotic one.

use bevy::prelude::*;

use crate::dock::DockTree;

/// One workspace a plugin has asked the editor to offer.
pub struct WorkspaceRequest {
    /// Shown in the title-bar layout switcher, and the key the request is
    /// deduplicated on.
    pub name: String,
    /// The arrangement, in this crate's vocabulary, which is also the dock's:
    /// `renzora_shell` installs it as-is.
    pub tree: DockTree,
    /// Hidden workspaces do not appear in the switcher. For asset-mode variants
    /// the editor activates on its own; a plugin adding an ordinary workspace
    /// wants `false`.
    pub hidden: bool,
}

/// Workspaces registered but not yet installed into the editor's ribbon.
///
/// Drained by `renzora_shell`. Left in place (rather than removed) when nothing
/// drains it, which is what keeps this crate usable in a build with no editor
/// shell: the requests simply accumulate and nothing reads them.
#[derive(Resource, Default)]
pub struct PendingWorkspaces(pub Vec<WorkspaceRequest>);

impl PendingWorkspaces {
    /// Take everything queued, leaving the resource empty.
    pub fn drain(&mut self) -> Vec<WorkspaceRequest> {
        std::mem::take(&mut self.0)
    }
}

/// App extension: offer a named workspace layout.
pub trait RegisterWorkspace {
    /// Register `name` as a workspace laid out by `tree`.
    ///
    /// The panels named in the tree do not have to exist yet. A leaf naming an
    /// unknown panel id renders as an empty tab rather than an error, which is
    /// what lets a plugin register its workspace and its panels in either order.
    fn register_workspace(&mut self, name: &str, tree: DockTree) -> &mut Self;

    /// As [`Self::register_workspace`], but kept out of the layout switcher.
    fn register_hidden_workspace(&mut self, name: &str, tree: DockTree) -> &mut Self;
}

impl RegisterWorkspace for App {
    fn register_workspace(&mut self, name: &str, tree: DockTree) -> &mut Self {
        queue(self, name, tree, false)
    }

    fn register_hidden_workspace(&mut self, name: &str, tree: DockTree) -> &mut Self {
        queue(self, name, tree, true)
    }
}

fn queue<'a>(app: &'a mut App, name: &str, tree: DockTree, hidden: bool) -> &'a mut App {
    if !app.world().contains_resource::<PendingWorkspaces>() {
        app.init_resource::<PendingWorkspaces>();
    }
    let mut pending = app.world_mut().resource_mut::<PendingWorkspaces>();
    // Last registration of a name wins, both here and in the drain: a plugin
    // that reloads must not leave its previous workspace behind.
    pending.0.retain(|w| w.name != name);
    pending.0.push(WorkspaceRequest {
        name: name.to_string(),
        tree,
        hidden,
    });
    app
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dock::SplitDirection;

    fn tree() -> DockTree {
        DockTree::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(DockTree::Leaf {
                tabs: vec!["a".into()],
                active_tab: 0,
            }),
            second: Box::new(DockTree::Leaf {
                tabs: vec!["b".into()],
                active_tab: 0,
            }),
        }
    }

    #[test]
    fn a_registration_is_queued_for_the_shell_to_drain() {
        let mut app = App::new();
        app.register_workspace("Debug", tree());
        let pending = app.world().resource::<PendingWorkspaces>();
        assert_eq!(pending.0.len(), 1);
        assert_eq!(pending.0[0].name, "Debug");
        assert!(!pending.0[0].hidden);
    }

    /// A native plugin is rebuilt and re-initialised whenever its source moves,
    /// so re-registering the same name is the ordinary case. It must not grow a
    /// second entry in the layout switcher each time.
    #[test]
    fn registering_a_name_twice_replaces_rather_than_duplicates() {
        let mut app = App::new();
        app.register_workspace("Debug", tree());
        app.register_hidden_workspace("Debug", tree());
        let pending = app.world().resource::<PendingWorkspaces>();
        assert_eq!(pending.0.len(), 1, "the second registration should replace the first");
        assert!(pending.0[0].hidden, "the later registration should win");
    }

    #[test]
    fn draining_empties_the_queue() {
        let mut app = App::new();
        app.register_workspace("Debug", tree());
        app.register_workspace("Profiling", tree());
        let mut pending = app.world_mut().resource_mut::<PendingWorkspaces>();
        assert_eq!(pending.drain().len(), 2);
        assert!(pending.0.is_empty());
    }
}
