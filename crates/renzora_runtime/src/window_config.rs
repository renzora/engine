//! `[window]` in `project.toml`: the game window's size, resizability, mode and
//! vsync, applied live.
//!
//! This used to run once at `Startup`, so the window settings were fixed for the
//! life of the game: an in-game settings menu that wrote
//! `CurrentProject.config.window` changed nothing, and neither did an edit made
//! while the game ran. `CurrentProject` is the runtime's live settings store, so
//! the game now applies a change whenever it lands there.
//!
//! **Only fields that changed are written.** A resizable window belongs to the
//! player once it is open: re-applying the whole config on every change would
//! snap a window they had dragged larger back to the configured size the
//! moment anything else in the project changed (a script setting `vsync`, say).
//! So the last applied config is remembered, and the configured size is written
//! only when the configured size (or the mode) itself moved.
//!
//! A game played from the editor opens in its own window like an exported one,
//! so every one of these applies there too.
//!
//! `max_fps` is not here: `frame_limit` reads it from the config every frame.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use renzora_project_api::{CurrentProject, WindowConfig};

pub fn register(app: &mut App) {
    app.init_resource::<AppliedWindowConfig>();
    // Startup, so the first frame already has the project's window. The
    // `Update` copy picks up every later change; on its first run it finds the
    // config it applied at Startup and writes nothing.
    app.add_systems(Startup, apply_window_config);
    app.add_systems(
        Update,
        apply_window_config.run_if(window_config_due),
    );
}

/// The config changed, or a size is waiting on last frame's mode change.
fn window_config_due(project: Option<Res<CurrentProject>>, applied: Res<AppliedWindowConfig>) -> bool {
    project.is_some_and(|p| p.is_changed()) || applied.resize_pending
}

/// What has been written to the window so far.
#[derive(Resource, Default)]
struct AppliedWindowConfig {
    /// The `[window]` config last written, so a later change writes only what
    /// differs from it. `None` until the first apply.
    config: Option<WindowConfig>,
    /// The size still has to be written, a frame after a mode change.
    ///
    /// Written in the same frame as the mode, the size is applied against the
    /// frame the window had before: leaving borderless put the title bar and
    /// borders back *inside* the configured size, and a 1024x640 window came
    /// back as 1008x601. A frame later the decorations exist and the size lands
    /// as configured.
    resize_pending: bool,
}

/// Which window settings differ between what was applied and what is wanted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct WindowChanges {
    size: bool,
    resizable: bool,
    mode: bool,
    vsync: bool,
}

impl WindowChanges {
    // Unread on the web, which applies nothing but the title.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    fn any(self) -> bool {
        self.size || self.resizable || self.mode || self.vsync
    }
}

/// Compare the applied config with the wanted one. With nothing applied yet
/// every field counts as changed.
///
/// A mode change also counts as a size change: leaving fullscreen should land
/// the window at its configured size, not at whatever the monitor was.
fn window_changes(applied: Option<&WindowConfig>, wanted: &WindowConfig) -> WindowChanges {
    let Some(applied) = applied else {
        return WindowChanges { size: true, resizable: true, mode: true, vsync: true };
    };
    let mode = applied.mode != wanted.mode;
    WindowChanges {
        size: mode || applied.width != wanted.width || applied.height != wanted.height,
        resizable: applied.resizable != wanted.resizable,
        mode,
        vsync: applied.vsync != wanted.vsync,
    }
}

fn apply_window_config(
    project: Option<Res<CurrentProject>>,
    mut applied: ResMut<AppliedWindowConfig>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let Some(project) = project else {
        if applied.config.is_none() {
            warn!("[runtime] No project loaded, window config not applied");
        }
        return;
    };

    if project.config.console_logging {
        // Idempotent: attaches at most once per process.
        crate::attach_console();
    }

    // The title is the one setting that means something everywhere: on the web
    // it becomes the document title rather than a title bar. Compare-first so
    // an unrelated project change does not mark the window changed.
    if window.title != project.config.name {
        window.title = project.config.name.clone();
    }

    // ── The web has no window for a game to configure ────────────────────────
    //
    // Everything below this point describes an OS window, and on wasm the
    // "window" IS the canvas: sized by the page, with no decorations, no
    // resizability of its own, and a present mode the browser owns (rAF).
    //
    // `resolution.set` is the one that actually broke something, and visibly:
    // the window is created with `fit_canvas_to_parent`, which negotiates the
    // canvas size against `#stage`, and then this ran at `Startup` and stomped
    // it with the project's configured size. Because the parent never changes
    // afterwards, the ResizeObserver has no reason to fire and nothing ever puts
    // it back, so an exported game rendered into a 1280x720 box in the corner of
    // a black page.
    //
    // Fullscreen is not merely inapplicable but impossible from here:
    // `requestFullscreen` is gated on a user gesture, so a game that wants it on
    // the web has to ask for it from an input handler.
    #[cfg(not(target_arch = "wasm32"))]
    {
        let cfg = &project.config.window;
        let mut changes = window_changes(applied.config.as_ref(), cfg);
        // The size follows a mode change a frame later; see `resize_pending`.
        let resize_now = std::mem::take(&mut applied.resize_pending);
        if changes.mode {
            applied.resize_pending = changes.size || resize_now;
            changes.size = false;
        } else if resize_now {
            changes.size = true;
        }
        if !changes.any() {
            return;
        }
        apply_changes(&mut window, cfg, changes);
        applied.config = Some(cfg.clone());
    }
    #[cfg(target_arch = "wasm32")]
    let _ = (&mut applied, window_changes);
}

#[cfg(not(target_arch = "wasm32"))]
fn apply_changes(window: &mut Window, cfg: &WindowConfig, changes: WindowChanges) {
    use bevy::window::{MonitorSelection, PresentMode, WindowMode as BevyWindowMode};
    use renzora_project_api::WindowMode;

    if changes.resizable {
        window.resizable = cfg.resizable;
    }
    if changes.vsync {
        // `AutoNoVsync` rather than `Immediate` so wgpu falls back to a
        // supported present mode on adapters that lack tear-free mailbox or
        // immediate. Uncapped is also what lets the true per-frame cost be
        // measured on a fast GPU.
        window.present_mode = if cfg.vsync { PresentMode::AutoVsync } else { PresentMode::AutoNoVsync };
    }
    // Mode before size: decorations change the inner area winit has to work
    // with, so the size is set against the frame the window will actually have.
    if changes.mode {
        match cfg.mode {
            WindowMode::Windowed => {
                window.decorations = true;
                window.mode = BevyWindowMode::Windowed;
            }
            WindowMode::Fullscreen => {
                window.decorations = false;
                window.mode = BevyWindowMode::BorderlessFullscreen(MonitorSelection::Current);
            }
            WindowMode::Borderless => {
                window.decorations = false;
                window.mode = BevyWindowMode::Windowed;
            }
        }
    }
    if changes.size {
        window.resolution.set(cfg.width as f32, cfg.height as f32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_project_api::WindowMode;

    #[test]
    fn first_apply_writes_everything() {
        let changes = window_changes(None, &WindowConfig::default());
        assert_eq!(changes, WindowChanges { size: true, resizable: true, mode: true, vsync: true });
    }

    #[test]
    fn unchanged_config_writes_nothing() {
        let cfg = WindowConfig::default();
        assert!(!window_changes(Some(&cfg), &cfg).any());
    }

    #[test]
    fn a_vsync_change_leaves_the_players_window_size_alone() {
        let applied = WindowConfig::default();
        let wanted = WindowConfig { vsync: false, ..applied.clone() };
        assert_eq!(window_changes(Some(&applied), &wanted), WindowChanges { vsync: true, ..default() });
    }

    /// A mode change writes the mode at once and the size a frame later, so the
    /// size is set against the window's new decorations rather than its old
    /// ones.
    #[test]
    fn the_size_follows_a_mode_change_a_frame_later() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).insert_resource(CurrentProject {
            path: "game".into(),
            config: Default::default(),
        });
        let window = app.world_mut().spawn((Window::default(), PrimaryWindow)).id();
        register(&mut app);
        app.update();

        {
            let mut p = app.world_mut().resource_mut::<CurrentProject>();
            p.config.window.mode = WindowMode::Borderless;
            p.config.window.width = 1024;
            p.config.window.height = 640;
        }
        app.update();
        let w = app.world().get::<Window>(window).unwrap();
        assert!(!w.decorations, "the mode waited");
        assert_ne!(w.resolution.width(), 1024.0, "the size went with the mode");

        app.update();
        let w = app.world().get::<Window>(window).unwrap();
        assert_eq!((w.resolution.width(), w.resolution.height()), (1024.0, 640.0));
    }

    #[test]
    fn max_fps_is_not_a_window_change() {
        let applied = WindowConfig::default();
        let wanted = WindowConfig { max_fps: 30, ..applied.clone() };
        assert!(!window_changes(Some(&applied), &wanted).any());
    }

    #[test]
    fn a_new_size_is_a_size_change_only() {
        let applied = WindowConfig::default();
        let wanted = WindowConfig { width: 1920, ..applied.clone() };
        assert_eq!(window_changes(Some(&applied), &wanted), WindowChanges { size: true, ..default() });
    }

    #[test]
    fn a_mode_change_also_restores_the_configured_size() {
        let applied = WindowConfig { mode: WindowMode::Fullscreen, ..WindowConfig::default() };
        let wanted = WindowConfig { mode: WindowMode::Windowed, ..applied.clone() };
        assert_eq!(
            window_changes(Some(&applied), &wanted),
            WindowChanges { size: true, mode: true, ..default() }
        );
    }
}
