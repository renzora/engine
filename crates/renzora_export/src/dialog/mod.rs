//! The export dialog — a two-column modal that turns the open project into a
//! shippable build.
//!
//! A preset sidebar on the left (each preset is a named configuration for one
//! platform, saved with the project) and a five-tab settings pane on the right:
//! Output, Packaging, Features, Plugins, Files. Once an export starts the whole
//! form is replaced by the log view, which streams the build's output and offers
//! Cancel.
//!
//! It edits [`ExportOverlayState`](crate::overlay::ExportOverlayState) and reuses
//! the worker (`run_export` plus the pollers).
//!
//! | Module | What it holds |
//! |---|---|
//! | [`lifecycle`] | Spawn/despawn, the Docker probe, the plugin and file scans |
//! | [`frame`] | The backdrop, the header, and the Export button |
//! | [`sidebar`] | The preset list and its add / duplicate / remove actions |
//! | [`settings`] | The right pane: platform header, the tab strip, tab sizing |
//! | [`output`] | Output tab — name, folder, icon, window, flags |
//! | [`packaging`] | Packaging tab — mode, runtime template, modding, compression |
//! | [`features`] | Features tab — the lean engine-feature strip |
//! | [`plugins`] | Plugins tab — link mode and the plugin card grid |
//! | [`files`] | Files tab — the project's files as a ticked tree |
//! | [`log`] | The build log view and its progress bar |
//! | [`interaction`] | Every click handler, and the save-before-export prompt |
//! | [`widgets`] | Small shared builders |

use bevy::prelude::*;

pub(crate) mod features;
pub(crate) mod files;
pub(crate) mod frame;
pub(crate) mod interaction;
pub(crate) mod lifecycle;
pub(crate) mod log;
pub(crate) mod output;
pub(crate) mod packaging;
pub(crate) mod plugins;
pub(crate) mod settings;
pub(crate) mod sidebar;
pub(crate) mod widgets;

pub(super) const GREEN: (u8, u8, u8) = (89, 191, 115);
pub(super) const AMBER: (u8, u8, u8) = (242, 166, 64);
pub(super) const RED: (u8, u8, u8) = (239, 68, 68);
/// The Export button. Deliberately not the theme accent: every selected tab and
/// switched-on control in this dialog is already accent-coloured, so the one
/// button that starts a build was competing with them. A fixed blue makes it the
/// only thing of its colour here, and stays legible under a theme whose accent
/// sits close to the panel background.
pub(super) const EXPORT_BLUE: (u8, u8, u8) = (56, 121, 232);
pub(super) const EXPORT_BLUE_HOT: (u8, u8, u8) = (78, 141, 246);

pub(crate) fn register(app: &mut App) {
    app.add_systems(
        Update,
        (
            lifecycle::manage_export_modal,
            settings::rebuild_right_pane,
            interaction::preset_click,
            interaction::preset_dup_click,
            interaction::preset_del_click,
            interaction::output_browse_click,
            interaction::icon_browse_click,
            interaction::icon_clear_click,
            interaction::download_click,
            interaction::source_download_click,
            interaction::install_click,
            interaction::export_click,
            interaction::save_prompt_click,
            interaction::cancel_or_back_click,
            interaction::copy_log_click,
            interaction::close_click,
            interaction::section_toggle_click,
        ),
    );
}

// ── Markers ──────────────────────────────────────────────────────────────────

#[derive(Component)]
pub(super) struct ExportRoot;
#[derive(Component)]
pub(super) struct RightPane {
    pub(super) sig: Option<u8>,
}
#[derive(Component, Clone, Copy)]
pub(super) struct PresetBtn(pub(super) usize);
#[derive(Component)]
pub(super) struct PresetDupBtn;
#[derive(Component)]
pub(super) struct PresetDelBtn;
#[derive(Component)]
pub(super) struct OutputBrowseBtn;
#[derive(Component)]
pub(super) struct IconBrowseBtn;
#[derive(Component)]
pub(super) struct IconClearBtn;
#[derive(Component)]
pub(super) struct DownloadBtn;
#[derive(Component)]
pub(super) struct SourceDownloadBtn;
#[derive(Component)]
pub(super) struct InstallBtn;
#[derive(Component)]
pub(super) struct ExportBtn;
#[derive(Component)]
pub(super) struct CloseBtn;
/// Cancel the running export, or (once finished) go back to the settings view.
#[derive(Component)]
pub(super) struct CancelOrBackBtn;
/// Copy the full build log to the system clipboard.
#[derive(Component)]
pub(super) struct CopyLogBtn;
/// The build log's scroll viewport, so [`lifecycle::follow_log_tail`] can find it.
#[derive(Component)]
pub(super) struct LogScroll;
/// The Files tab's panel, so [`lifecycle::resolve_included_files`] can tell when
/// it is on screen and only then pay for reading the project.
#[derive(Component)]
pub(super) struct FilesPanel;
/// A bulk action button in the Files tab.
#[derive(Component)]
pub(super) struct FilesBulk(pub(super) FilesAction);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum FilesAction {
    All,
    None,
    /// Re-run the automatic crawl and take its answer — the way back from a
    /// mistake, and it re-reads the project so it also picks up anything added
    /// since the dialog opened.
    Detected,
}

/// Features-tab section header — click folds/unfolds that section.
#[derive(Component)]
pub(super) struct SectionToggle(pub(super) &'static str);
