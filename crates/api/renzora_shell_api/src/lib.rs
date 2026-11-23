//! The editor chrome's vocabulary: the panels, status-bar items and top-bar
//! buttons a plugin adds (`shell`), the key map (`keybindings`), the one-way
//! events the editor fires at other crates (`editor_events`), and the
//! Inspector's pages and categories (`inspector_pages`), plus the editor's
//! extension points plugins write to: undo/redo (`undo`), the create-file menus
//! (`create_menu`), and the Assets and Hierarchy right-click actions
//! (`asset_actions`, `entity_actions`).
//!
//! A layer of the contract hierarchy, moved out of `renzora`. Not gated and not
//! editor-only: ember builds its panels from these registries in shipped games
//! too. The editor contract proper (selection, inspector registry, the
//! `Inspectable` derive) is `renzora_editor_api`, which builds on this.
//!
//! Plugins reach every item at the path it always had (`renzora::ShellPanelRegistry`,
//! `renzora::core::keybindings::..`, `renzora::inspector_pages::..`) through the
//! re-exports; engine crates depend on this crate directly.

pub mod editor_events; // one-way events the editor fires at other crates
pub mod inspector_pages; // the Inspector's pages, the categories every editor surface groups by
pub mod keybindings; // the key map and shortcut definitions
pub mod shell; // panels, status items and top-bar buttons a plugin adds

pub use editor_events::*;
pub use shell::*;

// ── Editor extension points (formerly `renzora_editor_ext`) ─────────────────
// Ungated for the same reason as the shell: ember's markup writeback, terrain
// graph builds and World Partition's hierarchy actions use them in every build.

// Undo/redo core. `execute` and `record` are far too generic for a crate root,
// so callers say `undo::execute`. An editing tool is the obvious thing to ship
// as a plugin, and the one thing it must do is make its edits undoable: see the
// module doc.
pub mod undo;

// Plugin-contributed entries in the "create a new file" menus. A contract type
// because the two crates that need it are the editor and an installed plugin.
pub mod create_menu;
pub use create_menu::{CreateMenuItem, CreateMenuRegistry, RegisterCreateMenuItem};

// Plugin-contributed rows in the Assets panel's right-click menu, keyed on file
// extension. Here for the same reason as `create_menu`.
pub mod asset_actions;
pub use asset_actions::{AssetAction, AssetActionRegistry, AssetActionScope, RegisterAssetAction};

// Plugin-contributed rows in the Hierarchy's right-click menu, offered per
// entity. Here for the same reason as `asset_actions`.
pub mod entity_actions;
pub use entity_actions::{EntityAction, EntityActionRegistry, RegisterEntityAction};
