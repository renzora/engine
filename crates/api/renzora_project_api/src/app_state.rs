//! App lifecycle state: the launcher / loading / editor phase, the launcher's
//! effects setting, and the requests that open or switch a project.
//!
//! Coordination contract used by both the splash screen UI and the editor
//! framework, so neither side has to depend on the other's implementation
//! crate. Moved here from `renzora`'s root; plugins still write
//! `renzora::SplashState` and the rest.

use bevy::prelude::{Resource, States};

/// Top-level app phase. The splash UI runs while `Splash`, a loading
/// overlay during `Loading`, and the full editor while `Editor`.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SplashState {
    #[default]
    Splash,
    Loading,
    Editor,
}

/// Whether the launcher and loading screen run their decorative effects: the
/// launcher's 3D Light Chamber cinematic and the loading screen's animated haze.
///
/// Both are full-window and both scale with the window's pixel count, so on a
/// very large or ultrawide display they cost the most for the least: a 49"
/// 5120x1440 panel has 3.5x the pixels of 1080p. `Auto` resolves to reduced on
/// such displays and to full elsewhere; the resolved answer is
/// [`SplashEffectsSetting::reduced`].
#[derive(
    serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum SplashEffects {
    #[default]
    Auto,
    Full,
    Reduced,
}

/// The [`SplashEffects`] choice, mirrored from the editor's settings so the
/// splash crate can read it without linking the editor framework.
#[derive(Resource, Default, Clone, Copy)]
pub struct SplashEffectsSetting(pub SplashEffects);

impl SplashEffectsSetting {
    /// Resolve against the display the launcher is on. `monitor` is its
    /// physical size, when known.
    ///
    /// "Large" is wider than 21:9 or more pixels than 4K: the displays where a
    /// full-window shader is the most expensive thing on screen.
    ///
    /// Always reduced on the web, whatever the setting: the cinematic held the
    /// browser editor's launcher to about 10 fps.
    pub fn reduced(&self, monitor: Option<(u32, u32)>) -> bool {
        if cfg!(target_arch = "wasm32") {
            return true;
        }
        match self.0 {
            SplashEffects::Full => false,
            SplashEffects::Reduced => true,
            SplashEffects::Auto => monitor.is_some_and(|(w, h)| {
                let h = h.max(1);
                w as f32 / h as f32 > 2.4 || (w as u64) * (h as u64) > 3840 * 2160
            }),
        }
    }
}

/// Marker request: open a different project. Inserted by the editor's File
/// menu; consumed by the splash plugin which shows the file dialog,
/// validates, updates recent projects, and transitions state.
#[derive(Resource)]
pub struct RequestOpenProject;

/// Request: open the project rooted at this path, with no file dialog.
///
/// [`RequestOpenProject`]'s sibling for a project the user has *already* named
/// (File > Recent Projects picks the path off [`RecentProjects`]), so asking
/// them to find the same `project.toml` in an OS dialog would be the whole
/// point of the list undone.
#[derive(Resource)]
pub struct RequestOpenProjectPath(pub std::path::PathBuf);

/// Request: switch to the project rooted at this path, the way File > Recent
/// Projects does, unsaved-changes prompt included.
///
/// [`RequestOpenProjectPath`] opens straight away and is what that prompt
/// inserts once it has been answered. This is the request for anything that
/// cannot see the document tabs, a plugin above all: it goes through the same
/// check the menu does, so it can never throw away an unsaved scene.
#[derive(Resource)]
pub struct RequestSwitchProject(pub std::path::PathBuf);

/// The recently-opened project roots, most recent first.
///
/// Mirrored out of the launcher's `AppConfig` by the splash plugin, which owns
/// the on-disk list. It lives here because the File menu is built in
/// `renzora_shell`, and the shell must not depend on the splash crate to read
/// one `Vec<PathBuf>`. Empty in a build with no splash plugin, which is exactly
/// what the menu should show there.
#[derive(Resource, Default, Clone)]
pub struct RecentProjects(pub Vec<std::path::PathBuf>);
