//! The import window — a modal that converts what you queued, shows you the
//! result, and waits for a verdict.
//!
//! Two panes flank a viewport: a left list (the file queue before conversion,
//! the scene tree / mesh list / material list after), a centre showing the
//! staged model or the selected material, and a right rail carrying the import
//! settings and then the selected item's properties. The layout is deliberately
//! the same before and after conversion — only what each region holds changes —
//! so nothing jumps around when a file finishes converting.
//!
//! It edits [`ImportOverlayState`](crate::overlay::ImportOverlayState) and
//! reuses the worker (`run_import` / `poll_import_task`), which the rest of the
//! crate drives regardless of whether the window is up.
//!
//! | Module | What it holds |
//! |---|---|
//! | [`lifecycle`] | Spawn/despawn, the initial widget values, auto-convert, the settle timer |
//! | [`frame`] | The window chrome: scrim, title bar, tab bar, splitters |
//! | [`panes`] | The three regions and the destination picker |
//! | [`rows`] | The one row builder every list in the window uses |
//! | [`tree`] | The scene tree: what is visible, what survives a prune |
//! | [`lists`] | The keyed-list snapshots behind each pane |
//! | [`interaction`] | Every click handler, and the verdict |
//! | [`toast`] | The corner progress toast shown after the window closes |
//! | [`widgets`] | Small shared builders (rows, pills, settings accessors) |

use std::path::PathBuf;

use bevy::prelude::*;

pub(crate) mod frame;
pub(crate) mod interaction;
pub(crate) mod lifecycle;
pub(crate) mod lists;
pub(crate) mod panes;
pub(crate) mod rows;
pub(crate) mod toast;
pub(crate) mod tree;
pub(crate) mod widgets;

pub use interaction::{pick_and_queue_files, pick_and_queue_folder};

pub(super) const GREEN: (u8, u8, u8) = (89, 191, 115);
pub(super) const RED: (u8, u8, u8) = (239, 68, 68);
pub(super) const AMBER: (u8, u8, u8) = (223, 165, 74);

/// Which tab the window's left pane is showing.
///
/// `Files` is the pre-conversion state — the queue and the drop targets. The
/// other three describe a *converted* model and only exist while one is staged,
/// which is why the tab bar hides them until then.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ImportTab {
    #[default]
    Files,
    Scene,
    Meshes,
    Materials,
    Destination,
}

/// One row of the scene tree. A node's mesh hangs under it as a child, and the
/// mesh's surfaces under that, which is how a DCC tool and Godot both present
/// it — the mesh is a *resource the node points at*, not the node itself, and
/// showing them as one row hides which nodes share geometry.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TreeItem {
    Node(usize),
    Mesh(usize),
    /// `(mesh, primitive)` — one surface, i.e. one material's worth of it.
    Prim(usize, usize),
}

/// Everything the window remembers about what you are looking at: the tab, the
/// expanded tree rows, and the selection per tab.
///
/// Selection is kept per tab rather than as one shared index because the three
/// lists address different things — node 4, mesh 4 and material 4 are unrelated
/// — and switching tabs should not silently repoint the properties rail.
#[derive(Resource, Default)]
pub struct ImportNav {
    pub tab: ImportTab,
    pub expanded: std::collections::HashSet<TreeItem>,
    pub sel_item: Option<TreeItem>,
    pub sel_mesh: Option<usize>,
    pub sel_material: Option<usize>,
}

impl ImportNav {
    /// Clear everything tied to one staged file. Called when a verdict is given
    /// and when the next file stages, so indices from the previous model can
    /// never address the new one.
    pub(crate) fn reset_selection(&mut self) {
        self.expanded.clear();
        self.sel_item = None;
        self.sel_mesh = None;
        self.sel_material = None;
    }
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<ImportNav>()
        .init_resource::<ImportColumns>()
        // Split in two: a system tuple caps out at 20 elements.
        .add_systems(
            Update,
            (
                lifecycle::manage_import_modal,
                toast::manage_import_toast,
                interaction::file_browse_click,
                interaction::folder_browse_click,
                interaction::dest_folder_click,
                interaction::tab_click,
                frame::splitter_drag,
                interaction::staged_row_click,
                interaction::tree_row_click,
                interaction::tree_check_click,
                interaction::mesh_row_click,
                interaction::mat_row_click,
            ),
        )
        .add_systems(
            Update,
            (
                interaction::commit_click,
                interaction::skip_click,
                interaction::discard_all_click,
                lifecycle::settings_watch,
                crate::overlay::drive_reimport,
                lifecycle::auto_start_import,
                lifecycle::on_staged_changed,
                interaction::cancel_click,
                toast::toast_dismiss_click,
                interaction::remove_file_click,
            ),
        );
}

// ── Markers ──────────────────────────────────────────────────────────────────

#[derive(Component)]
pub(super) struct ImportRoot;

/// The editor grid's visibility from before the window opened, restored on
/// close. The grid's render pass is not confined to the main viewport's layer,
/// so it draws through the preview's own camera and cuts a lattice across
/// whatever is being inspected.
#[derive(Resource)]
pub(super) struct GridSuppressed(pub(super) bool);
#[derive(Component)]
pub(super) struct FileBrowseBtn;
#[derive(Component)]
pub(super) struct FolderBrowseBtn;
/// A clickable sidebar nav row; switches the active pane on press.
#[derive(Component, Clone, Copy)]
pub(super) struct TabBtn(pub(super) ImportTab);

/// Which edge a splitter drags.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Side {
    Left,
    Right,
}

#[derive(Component, Clone, Copy)]
pub(super) struct Splitter(pub(super) Side);

/// User-set column widths, in logical pixels. Lives outside the window so a
/// resize survives closing and reopening it.
#[derive(Resource)]
pub(crate) struct ImportColumns {
    pub(super) left: f32,
    pub(super) right: f32,
}

impl Default for ImportColumns {
    fn default() -> Self {
        Self {
            left: 310.0,
            right: 320.0,
        }
    }
}
/// A scene-tree row, carrying its node index.
#[derive(Component, Clone, Copy)]
pub(super) struct TreeRow(pub(super) TreeItem);
/// A row in the mesh list.
#[derive(Component, Clone, Copy)]
pub(super) struct MeshRow(pub(super) usize);
/// A row in the material list.
#[derive(Component, Clone, Copy)]
pub(super) struct MatRow(pub(super) usize);
/// Accept the staged file into the project.
#[derive(Component)]
pub(super) struct CommitBtn;
/// Discard this staged file, continue the queue.
#[derive(Component)]
pub(super) struct SkipBtn;
/// Discard this staged file and abandon the queue.
#[derive(Component)]
pub(super) struct DiscardAllBtn;
/// A row in the destination folder tree. Holds the project-relative path it
/// targets (forward-slashed, `""` = project root).
#[derive(Component, Clone)]
pub(super) struct DestFolderRow(pub(super) String);
#[derive(Component)]
pub(super) struct CancelBtn;
#[derive(Component, Clone)]
pub(super) struct RemoveFileBtn(pub(super) PathBuf);
/// A staged model in the Files list; clicking makes it the one on show.
#[derive(Component, Clone, Copy)]
pub(super) struct StagedRow(pub(super) usize);
/// The include-checkbox on a scene-tree row. Only ever inserted on a box the
/// user is allowed to click.
#[derive(Component, Clone, Copy)]
pub(super) struct TreeCheck(pub(super) TreeItem);
#[derive(Component)]
pub(super) struct FilesContainer;
#[derive(Component)]
pub(super) struct LogContainer;
/// Root of the corner progress toast shown after the modal closes on Import.
#[derive(Component)]
pub(super) struct ToastRoot;
/// The toast's close/dismiss button.
#[derive(Component)]
pub(super) struct ToastDismissBtn;
