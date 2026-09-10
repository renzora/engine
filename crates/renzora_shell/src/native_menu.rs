//! The real macOS menu bar.
//!
//! Every other piece of chrome in this editor is drawn by the editor — the title
//! bar, the buttons, the hamburger. The menu bar cannot be, because it does not
//! live in the window: it belongs to the application and sits at the top of the
//! screen, and only AppKit can put anything there. So this is the one place the
//! shell reaches out to a native widget rather than drawing its own.
//!
//! The in-app hamburger stays. A Mac user reaches for the menu bar; someone who
//! learned the editor on Windows reaches for the hamburger; and both are right.
//! What they must not do is disagree, which is why neither of them holds an
//! action — see [`crate::menu_command`], where each row's behaviour is named
//! once and both menus dispatch through it.
//!
//! # Main-thread discipline
//!
//! AppKit menus may only be touched from the main thread, and muda's handles are
//! `Rc`-based, so they are neither `Send` nor `Sync`. Both problems have the
//! same answer: the menu is held as a Bevy **non-send** resource, which the
//! scheduler guarantees is only ever accessed from the main thread, and building
//! it happens in an exclusive system for the same reason.
//!
//! # What is deliberately not here
//!
//! **Accelerators.** macOS would happily show `⌘S` beside Save, and it would
//! work — but the editor already binds its own shortcuts, and a menu accelerator
//! fires *in addition* to them rather than instead. A double save is untidy; a
//! double undo loses work. Wiring these properly means routing the editor's
//! bindings through the menu rather than adding a second source of the same key,
//! and that is a bigger change than this one.
//!
//! **A live Account menu.** The bar is built once at startup, so the Account
//! submenu offers Sign In and Sign Out together rather than the one that applies
//! right now. Rebuilding it on every auth change is the fix; offering a row that
//! is occasionally a no-op is the cost of not doing that yet.

use bevy::prelude::*;
use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

use crate::menu_command::{MenuCommand, MenuCommandQueue};

/// Keeps the menu alive for the life of the app.
///
/// Non-send because muda's handles are `Rc`s over AppKit objects. Dropping this
/// would tear the menu down, so it is held rather than used — nothing reads it
/// after construction.
struct NativeMenu {
    #[allow(dead_code)]
    menu: Menu,
}

pub(crate) fn register(app: &mut App) {
    // Exclusive, so it runs on the main thread — see the module docs. `Startup`
    // rather than `PreStartup` because winit has to have created the
    // `NSApplication` before there is an app menu to install into, and that
    // happens while the window is being opened.
    app.add_systems(Startup, build_menu);
    app.add_systems(Update, poll_menu_events);
}

/// One stable string per command, used as the muda item id.
///
/// Exhaustive on purpose: a new [`MenuCommand`] will not compile until it has an
/// id here, which is what keeps [`command_from_id`] able to name everything the
/// menu can raise.
fn command_id(cmd: MenuCommand) -> &'static str {
    use MenuCommand::*;
    match cmd {
        NewProject => "renzora.new_project",
        OpenProject => "renzora.open_project",
        NewScene => "renzora.new_scene",
        OpenScene => "renzora.open_scene",
        Save => "renzora.save",
        SaveAs => "renzora.save_as",
        ImportFiles => "renzora.import_files",
        ImportFolder => "renzora.import_folder",
        InstallPlugin => "renzora.install_plugin",
        Undo => "renzora.undo",
        Redo => "renzora.redo",
        ZoomIn => "renzora.zoom_in",
        ZoomOut => "renzora.zoom_out",
        ResetZoom => "renzora.reset_zoom",
        FitAll => "renzora.fit_all",
        IsolationMode => "renzora.isolation_mode",
        ResetLayout => "renzora.reset_layout",
        ResetWorkspace => "renzora.reset_workspace",
        ResetGlobalDocks => "renzora.reset_global_docks",
        ResetDefaults => "renzora.reset_defaults",
        Tutorial => "renzora.tutorial",
        Documentation => "renzora.documentation",
        YouTube => "renzora.youtube",
        Discord => "renzora.discord",
        GitHub => "renzora.github",
        CheckUpdates => "renzora.check_updates",
        About => "renzora.about",
        MyLibrary => "renzora.my_library",
        SignIn => "renzora.sign_in",
        SignOut => "renzora.sign_out",
    }
}

/// Every command the bar can offer, in menu order.
///
/// A command missing from here is simply absent from the menu bar — a missing
/// row, which someone notices — rather than a row that does nothing.
const ALL: &[MenuCommand] = {
    use MenuCommand::*;
    &[
        NewProject,
        OpenProject,
        NewScene,
        OpenScene,
        Save,
        SaveAs,
        ImportFiles,
        ImportFolder,
        InstallPlugin,
        Undo,
        Redo,
        ZoomIn,
        ZoomOut,
        ResetZoom,
        FitAll,
        IsolationMode,
        ResetLayout,
        ResetWorkspace,
        ResetGlobalDocks,
        ResetDefaults,
        Tutorial,
        Documentation,
        YouTube,
        Discord,
        GitHub,
        CheckUpdates,
        About,
        MyLibrary,
        SignIn,
        SignOut,
    ]
};

fn command_from_id(id: &str) -> Option<MenuCommand> {
    ALL.iter().copied().find(|c| command_id(*c) == id)
}

/// A clickable row for one command, labelled from the same translation key the
/// hamburger uses so the two menus read identically.
fn item(cmd: MenuCommand, label: String) -> MenuItem {
    MenuItem::with_id(command_id(cmd), label, true, None)
}

fn build_menu(world: &mut World) {
    let menu = Menu::new();

    // ── The application menu ────────────────────────────────────────────────
    // macOS titles the first submenu with the app name whatever it is called
    // here, and expects About / Settings / Services / Hide / Quit in it. About
    // is ours rather than muda's `PredefinedMenuItem::about`, because the editor
    // already has an About panel and two different ones would be odd.
    let app_menu = Submenu::with_items(
        "Renzora",
        true,
        &[
            &item(MenuCommand::About, renzora::lang::t_or("menu.help.about_engine", "About Renzora Engine")),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::services(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::separator(),
            // Predefined, not a `MenuCommand`: quitting is AppKit's job and it
            // does the termination dance properly. Routing it through the queue
            // would mean quitting one frame later for no benefit.
            &PredefinedMenuItem::quit(None),
        ],
    );

    let file = Submenu::with_items(
        "File",
        true,
        &[
            &item(MenuCommand::NewProject, renzora::lang::t("menu.file.new_project")),
            &item(MenuCommand::OpenProject, renzora::lang::t("menu.file.open_project")),
            &PredefinedMenuItem::separator(),
            &item(MenuCommand::NewScene, renzora::lang::t("menu.file.new_scene")),
            &item(MenuCommand::OpenScene, renzora::lang::t("menu.file.open_scene")),
            &PredefinedMenuItem::separator(),
            &item(MenuCommand::Save, renzora::lang::t("common.save")),
            &item(MenuCommand::SaveAs, renzora::lang::t_or("menu.file.save_as", "Save As…")),
            &PredefinedMenuItem::separator(),
            &item(MenuCommand::ImportFiles, renzora::lang::t("assets.import_files")),
            &item(MenuCommand::ImportFolder, renzora::lang::t("assets.import_folder")),
            &PredefinedMenuItem::separator(),
            &item(MenuCommand::InstallPlugin, renzora::lang::t_or("menu.file.install_plugin", "Install Plugin…")),
        ],
    );

    // The clipboard rows are predefined rather than ours: they dispatch through
    // the responder chain, so they act on whatever text field has focus. A
    // `MenuCommand` would have to know which one that is, and it does not.
    let edit = Submenu::with_items(
        "Edit",
        true,
        &[
            &item(MenuCommand::Undo, renzora::lang::t("common.undo")),
            &item(MenuCommand::Redo, renzora::lang::t("common.redo")),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::cut(None),
            &PredefinedMenuItem::copy(None),
            &PredefinedMenuItem::paste(None),
            &PredefinedMenuItem::select_all(None),
        ],
    );

    let view = Submenu::with_items(
        "View",
        true,
        &[
            &item(MenuCommand::ZoomIn, renzora::lang::t_or("menu.view.zoom_in", "Zoom In")),
            &item(MenuCommand::ZoomOut, renzora::lang::t_or("menu.view.zoom_out", "Zoom Out")),
            &item(MenuCommand::ResetZoom, renzora::lang::t_or("menu.view.reset_zoom", "Reset Zoom")),
            &PredefinedMenuItem::separator(),
            &item(MenuCommand::FitAll, renzora::lang::t_or("menu.view.fit_all", "Fit All")),
            &item(MenuCommand::IsolationMode, renzora::lang::t_or("menu.view.isolation_mode", "Isolation Mode")),
            &PredefinedMenuItem::separator(),
            &item(MenuCommand::ResetLayout, renzora::lang::t("menu.window.reset_layout")),
            &item(MenuCommand::ResetWorkspace, renzora::lang::t_or("menu.view.reset_workspace", "Reset Workspace")),
            &item(MenuCommand::ResetGlobalDocks, renzora::lang::t_or("menu.view.reset_global_docks", "Reset Global Docks")),
            &PredefinedMenuItem::separator(),
            &item(MenuCommand::ResetDefaults, renzora::lang::t_or("menu.view.reset_defaults", "Reset to Defaults")),
        ],
    );

    // Every Mac app has one, and its rows are window-manager actions rather than
    // application ones — so they are predefined, and they are the same three the
    // traffic lights perform.
    let window = Submenu::with_items(
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(None),
            &PredefinedMenuItem::fullscreen(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::close_window(None),
        ],
    );

    let account = Submenu::with_items(
        "Account",
        true,
        &[
            &item(MenuCommand::SignIn, renzora::lang::t("auth.sign_in")),
            &item(MenuCommand::SignOut, renzora::lang::t("auth.sign_out")),
            &PredefinedMenuItem::separator(),
            &item(MenuCommand::MyLibrary, renzora::lang::t("menu.account.my_library")),
        ],
    );

    let help = Submenu::with_items(
        "Help",
        true,
        &[
            &item(MenuCommand::Tutorial, renzora::lang::t_or("menu.help.tutorial", "Getting Started Tutorial")),
            &PredefinedMenuItem::separator(),
            &item(MenuCommand::Documentation, renzora::lang::t("menu.help.documentation")),
            &item(MenuCommand::YouTube, renzora::lang::t("menu.help.youtube")),
            &item(MenuCommand::Discord, renzora::lang::t("menu.help.discord")),
            &item(MenuCommand::GitHub, renzora::lang::t_or("menu.help.github", "GitHub")),
            &PredefinedMenuItem::separator(),
            &item(MenuCommand::CheckUpdates, renzora::lang::t("menu.help.check_updates")),
        ],
    );

    // Every step can fail — muda returns `Result` because AppKit can refuse —
    // and none of them is worth taking the editor down for. A missing menu bar
    // is a degraded editor; a panic here is no editor at all, and the hamburger
    // still offers every one of these commands.
    let submenus = [app_menu, file, edit, view, window, account, help];
    for sub in submenus.iter() {
        match sub {
            Ok(sub) => {
                if let Err(e) = menu.append(sub) {
                    warn!("[menu] could not append a submenu: {e}");
                }
            }
            Err(e) => warn!("[menu] could not build a submenu: {e}"),
        }
    }

    menu.init_for_nsapp();
    world.insert_non_send(NativeMenu { menu });
    info!("[menu] native macOS menu bar installed");
}

/// Move menu clicks onto the command queue.
///
/// muda delivers on a channel of its own rather than through Bevy, so this
/// drains it once a frame. `try_recv` in a loop rather than a blocking read —
/// this runs in `Update` and must never wait.
fn poll_menu_events(mut queue: ResMut<MenuCommandQueue>) {
    while let Ok(event) = MenuEvent::receiver().try_recv() {
        match command_from_id(event.id().as_ref()) {
            Some(cmd) => queue.push(cmd),
            // A predefined item (Quit, Copy, Minimize…) — AppKit already
            // performed it, and it has no command of ours to run.
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ids must round-trip, or a menu click would resolve to nothing — the
    /// failure mode where the bar looks right and does nothing when used.
    #[test]
    fn every_command_round_trips_through_its_id() {
        for cmd in ALL {
            assert_eq!(
                command_from_id(command_id(*cmd)),
                Some(*cmd),
                "{cmd:?} does not survive its own id"
            );
        }
    }

    /// Two commands sharing an id would make one of them unreachable, and
    /// `command_from_id` would silently return whichever came first.
    #[test]
    fn ids_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for cmd in ALL {
            assert!(seen.insert(command_id(*cmd)), "duplicate id for {cmd:?}");
        }
    }
}
