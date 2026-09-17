//! Every action a menu can perform, named once.
//!
//! The editor has two menus now. The in-app hamburger has always been there, and
//! on macOS a real menu bar sits in the system bar alongside it — see
//! [`crate::native_menu`]. Both offer the same commands, and that is exactly the
//! arrangement where they drift: someone adds a row to one, the other keeps
//! doing the old thing, and the bug report is "Save works from the menu bar but
//! not from the hamburger".
//!
//! So neither menu holds an action. They both hold a [`MenuCommand`], and
//! [`MenuCommand::run`] is the only place that says what one does. A command
//! that gains a row in one menu and not the other is then a missing *row*, which
//! is visible, rather than two implementations that disagree, which is not.
//!
//! # Why this is `&mut World` rather than events
//!
//! Almost every command is a one-line `insert_resource` picked up by whichever
//! system owns that concern — the importer, the save prompts, the camera. That
//! is the pattern the hamburger already used, and routing it through an event
//! would add a frame of latency and a second vocabulary for no gain. The two
//! that are not resource inserts (undo/redo, which call into
//! `EditorActionHooks`) need the world anyway.

use bevy::prelude::*;

use renzora_ember::dock::{Dock, DockDirty};

use crate::open_url;

/// One thing a menu can do.
///
/// Ordered as the menus present them — File, Edit, View, Help, Account — so the
/// enum reads as the menu does and a missing command is easy to spot.
/// `Clone` rather than `Copy`: [`MenuCommand::OpenRecent`] carries the path it
/// opens. Encoding an *index* into the recents list instead would keep this
/// `Copy` and be wrong — the list reorders as projects are opened, so an index
/// captured when the menu was built can name a different project by the time it
/// is clicked.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MenuCommand {
    // ── File ────────────────────────────────────────────────────────────────
    /// Both of these leave the project, so they ask
    /// `save_prompts::process_project_switch_request` for it rather than doing
    /// it: that closes every open document, and doing so on one click with
    /// unsaved edits in them is the loss the window's × has always prompted
    /// about.
    NewProject,
    /// File > Create Project: give the untitled scratch session a real home.
    ///
    /// Only offered while [`renzora::UntitledProject`] is present. It copies the
    /// scratch folder to a folder you pick and opens the copy, so the work
    /// carries over rather than starting again, which is the difference between
    /// this and [`Self::NewProject`].
    CreateProject,
    OpenProject,
    /// File > Import Bevy Project: open a hand-written Bevy crate.
    ///
    /// Separate from [`Self::OpenProject`] because it restarts the editor rather
    /// than transitioning it: a Bevy project's code is a plugin, and a plugin is
    /// installed while the `App` is being built. See
    /// `renzora_bevy_project`.
    ImportBevyProject,
    /// File > Reload Project Code: load the newest build of a Bevy project into
    /// the running editor, rebuilding the world its code makes.
    ///
    /// Not a project switch, so it goes straight to its own request rather than
    /// through `ProjectSwitchRequest`: the project is not changing, only the
    /// code behind it, and none of the save-prompt machinery applies.
    ReloadProjectCode,
    /// One specific project from the recents list.
    OpenRecent(std::path::PathBuf),
    NewScene,
    OpenScene,
    Save,
    SaveAs,
    /// Two commands because no OS dialog picks files and folders at once — see
    /// `renzora::core::ImportPick`.
    ImportFiles,
    ImportFolder,
    InstallPlugin,

    // ── Edit ────────────────────────────────────────────────────────────────
    Undo,
    Redo,
    /// The same palette the top bar's magnifier toggles — see
    /// `top_bar::CommandPaletteBtn`. It is the editor's search, so a menu row
    /// for it opens that rather than trying to be a second one.
    CommandPalette,

    // ── View ────────────────────────────────────────────────────────────────
    ZoomIn,
    ZoomOut,
    ResetZoom,
    FitAll,
    IsolationMode,
    ResetLayout,
    ResetWorkspace,
    ResetGlobalDocks,
    ResetDefaults,

    // ── Help ────────────────────────────────────────────────────────────────
    /// Help > Splash Screen: put the dashboard back over the editor.
    ///
    /// The splash appears on launch and is dismissed by pressing the editor
    /// behind it, so without a way back the projects list, the changelog and the
    /// account pages would be reachable exactly once per session.
    SplashScreen,
    Tutorial,
    Documentation,
    YouTube,
    Discord,
    GitHub,
    CheckUpdates,
    About,

    // ── Account ─────────────────────────────────────────────────────────────
    MyLibrary,
    SignIn,
    SignOut,

    // ── Application ─────────────────────────────────────────────────────────
    Settings,
}

impl MenuCommand {
    /// Perform it.
    ///
    /// Exhaustive on purpose — no `_ =>` arm — so adding a variant fails to
    /// compile until it has been given a behaviour, rather than silently doing
    /// nothing when a menu row is wired to it.
    pub fn run(self, world: &mut World) {
        use crate::save_prompts::{ProjectSwitch, ProjectSwitchRequest};
        use renzora::core::*;

        match self {
            // ── File ────────────────────────────────────────────────────────
            Self::NewProject => {
                world.insert_resource(ProjectSwitchRequest(ProjectSwitch::New));
            }
            Self::CreateProject => {
                world.insert_resource(ProjectSwitchRequest(ProjectSwitch::CreateFromUntitled));
            }
            Self::OpenProject => {
                world.insert_resource(ProjectSwitchRequest(ProjectSwitch::Pick));
            }
            Self::ImportBevyProject => {
                world.insert_resource(ProjectSwitchRequest(ProjectSwitch::ImportBevy));
            }
            Self::ReloadProjectCode => {
                world.insert_resource(renzora::RequestProjectReload);
            }
            Self::OpenRecent(path) => {
                world.insert_resource(ProjectSwitchRequest(ProjectSwitch::Recent(path)));
            }
            Self::NewScene => world.insert_resource(NewSceneRequested),
            Self::OpenScene => world.insert_resource(OpenSceneRequested),
            Self::Save => world.insert_resource(SaveSceneRequested),
            Self::SaveAs => world.insert_resource(SaveAsSceneRequested),
            Self::ImportFiles => world.insert_resource(ImportRequested(ImportPick::Files)),
            Self::ImportFolder => world.insert_resource(ImportRequested(ImportPick::Folder)),
            Self::InstallPlugin => crate::plugin_install::open_install_dialog(world),

            // ── Edit ────────────────────────────────────────────────────────
            // Read out of the hooks first and called after, because the hook
            // takes `&mut World` and holding the resource borrow across the call
            // would not compile.
            Self::Undo => {
                let f = world
                    .get_resource::<renzora_editor_framework::EditorActionHooks>()
                    .and_then(|h| h.undo);
                if let Some(f) = f {
                    f(world);
                }
            }
            Self::Redo => {
                let f = world
                    .get_resource::<renzora_editor_framework::EditorActionHooks>()
                    .and_then(|h| h.redo);
                if let Some(f) = f {
                    f(world);
                }
            }

            Self::CommandPalette => world.insert_resource(ToggleCommandPaletteRequested),

            // ── View ────────────────────────────────────────────────────────
            Self::ZoomIn => world.insert_resource(CameraViewRequest::ZoomIn),
            Self::ZoomOut => world.insert_resource(CameraViewRequest::ZoomOut),
            Self::ResetZoom => world.insert_resource(CameraViewRequest::ResetZoom),
            Self::FitAll => world.insert_resource(CameraViewRequest::FrameAll),
            Self::IsolationMode => {
                let mut iso = world.remove_resource::<IsolationMode>().unwrap_or_default();
                iso.active = !iso.active;
                world.insert_resource(iso);
            }
            Self::ResetLayout => crate::top_menu::reset_layout_action(world),
            Self::ResetWorkspace => crate::top_menu::reset_workspace_action(world),
            Self::ResetGlobalDocks => crate::top_menu::reset_global_docks_action(world),
            Self::ResetDefaults => crate::top_menu::reset_defaults_action(world),

            // ── Help ────────────────────────────────────────────────────────
            Self::SplashScreen => {
                if let Some(mut overlay) = world.get_resource_mut::<renzora::SplashOverlay>() {
                    overlay.open = true;
                }
            }
            Self::Tutorial => world.insert_resource(TutorialRequested),
            Self::Documentation => open_url("https://renzora.com/docs"),
            Self::YouTube => open_url("https://youtube.com/@renzoragame"),
            Self::Discord => open_url("https://discord.gg/9UHUGUyDJv"),
            Self::GitHub => open_url("https://github.com/renzora/engine"),
            Self::CheckUpdates => world.insert_resource(UpdateRequested),
            Self::About => world.insert_resource(crate::about::ShowAboutRequested),

            // ── Account ─────────────────────────────────────────────────────
            Self::MyLibrary => {
                if let Some(mut dock) = world.get_resource_mut::<Dock>() {
                    dock.tree.focus_or_add_panel("hub_library");
                }
                if let Some(mut d) = world.get_resource_mut::<DockDirty>() {
                    d.0 = true;
                }
            }
            Self::SignIn => world.insert_resource(AuthToggleWindowRequest),
            Self::SignOut => world.insert_resource(AuthSignOutRequest),

            // ── Application ─────────────────────────────────────────────────
            // A toggle rather than an open, matching the gear button this row
            // replaced: clicking Settings with the panel already up closes it.
            Self::Settings => {
                if let Some(mut s) =
                    world.get_resource_mut::<renzora_editor_framework::EditorSettings>()
                {
                    s.show_settings = !s.show_settings;
                }
            }
        }
    }
}

/// Commands raised outside a system, waiting to be run against the world.
///
/// The native menu bar delivers its events on a channel rather than as a Bevy
/// input, so there is no system running at the moment a menu item is clicked.
/// The poller pushes here and [`drain`] performs them, which also means every
/// command runs at one known point in the schedule instead of wherever the
/// channel happened to be read.
#[derive(Resource, Default)]
pub struct MenuCommandQueue(Vec<MenuCommand>);

impl MenuCommandQueue {
    pub fn push(&mut self, cmd: MenuCommand) {
        self.0.push(cmd);
    }
}

/// Run everything queued, in the order it arrived.
///
/// Exclusive, because [`MenuCommand::run`] takes the whole world — most commands
/// insert a resource some other system is waiting on.
pub fn drain(world: &mut World) {
    let queued: Vec<MenuCommand> = match world.get_resource_mut::<MenuCommandQueue>() {
        Some(mut q) if !q.0.is_empty() => std::mem::take(&mut q.0),
        _ => return,
    };
    for cmd in queued {
        cmd.run(world);
    }
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<MenuCommandQueue>();
    app.add_systems(Update, drain);
}
