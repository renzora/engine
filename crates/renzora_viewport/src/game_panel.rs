//! The Game panel: where the running game appears when it is embedded.
//!
//! The game is always its own process (see `play_mode`). With **Embed Game in
//! Editor** on, its real OS window is made a child of the editor's window (the
//! game does that itself, see `renzora_runtime::embed`) and this module keeps
//! it sitting exactly over this panel: moved and resized with it, hidden when
//! the panel is behind another tab. With the setting off, or on a platform
//! without embedding, the game opens in its own window and this panel says so.
//!
//! **Why a native child window and not frames copied into the panel.** A
//! child window gets the mouse and keyboard directly, renders at the game's
//! own frame rate with its own GPU device, and costs the editor nothing per
//! frame. Copying frames across would need shared GPU textures per platform,
//! forwarding every input event, and an extra frame of latency.
//!
//! **Editor popups over the game.** A native window is drawn above everything
//! the editor draws in its area, so a menu, dropdown or dialog opened over the
//! Game panel used to be hidden behind the running game, or half drawn through
//! it. Every floating surface the editor shows carries `OverlaySurface` or
//! `ModalSurface`, so the rectangles of the visible ones are cut out of the
//! game window's region and the editor shows through the holes. A modal's
//! surface is its full-window backdrop, so the game is hidden while one is up,
//! which matches the modal dimming everything else.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};

use renzora_editor_framework::EditorSettings;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::tracked::{bind_display, bind_text};
use renzora_ember::theme::{rgb, text_muted};
use renzora_ember::widgets::{ModalSurface, OverlaySurface};

use crate::external_runtime::{ExternalRuntime, RuntimePhase};

/// Dock panel id.
pub const GAME_PANEL_ID: &str = "game";

/// The node the game's window is placed over: the whole panel body.
#[derive(Component)]
pub struct GameSurface;

/// Where the embedded game's window currently is, so it is only moved when
/// something actually changed.
#[derive(Resource, Default)]
pub struct GameEmbedState {
    /// The game's window, once found among the editor window's children.
    pub hwnd: Option<isize>,
    /// The last rect and visibility handed to Windows.
    placed: Option<(IRect, bool)>,
    /// The editor surfaces last cut out of the game's window, in its own
    /// client pixels, and the window size the region was built for: a region
    /// is a fixed shape, so a resize with holes open needs a new one.
    holes: (Vec<IRect>, IVec2),
}

impl GameEmbedState {
    /// Whether the game is on screen in the panel right now.
    pub fn showing(&self) -> bool {
        self.placed.is_some_and(|(_, shown)| shown)
    }
}

/// Whether this build can put the game in the panel at all.
pub const EMBED_SUPPORTED: bool = cfg!(target_os = "windows");

pub fn register(app: &mut App) {
    use renzora_editor_framework::SplashState;
    app.init_resource::<GameEmbedState>();
    app.register_panel_content(GAME_PANEL_ID, false, build)
        // Always, not only while the panel shows: hiding the game's window
        // when another tab covers the panel is half of this system's job, and
        // a native window is not hidden by the editor's UI. Gated on the
        // panel, the game would stay drawn over whichever tab replaced it.
        .always(
            Update,
            place_game_window.run_if(in_state(SplashState::Editor)),
        );
}

/// The editor window's native handle, for the game to parent its window to.
/// `None` when embedding is off, unsupported, or the window is not up.
pub fn embed_parent(world: &mut World) -> Option<isize> {
    if !EMBED_SUPPORTED
        || !world
            .get_resource::<EditorSettings>()
            .is_some_and(|s| s.embed_game)
    {
        return None;
    }
    let mut q = world.query_filtered::<&bevy::window::RawHandleWrapper, With<bevy::window::PrimaryWindow>>();
    let handle = q.single(world).ok()?.get_window_handle();
    match handle {
        raw_window_handle::RawWindowHandle::Win32(win) => Some(win.hwnd.get()),
        _ => None,
    }
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.0),
                ..default()
            },
            // Black, like a game's own window before its first frame, so the
            // moment between Play and the game appearing doesn't flash.
            BackgroundColor(Color::BLACK),
            GameSurface,
            Name::new("game-panel"),
        ))
        .id();

    let label = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 14.0),
            TextColor(rgb(text_muted())),
            TextLayout::justify(Justify::Center),
        ))
        .id();
    bind_text(commands, label, |w| {
        let phase = w
            .get_resource::<ExternalRuntime>()
            .map(|r| r.phase())
            .unwrap_or_default();
        let embed = w.get_resource::<EditorSettings>().is_some_and(|s| s.embed_game);
        let key = match (phase, embed && EMBED_SUPPORTED) {
            (RuntimePhase::Idle, true) => "game_panel.idle",
            (RuntimePhase::Idle, false) if !EMBED_SUPPORTED => "game_panel.unsupported",
            (RuntimePhase::Idle, false) => "game_panel.embed_off",
            (RuntimePhase::Preparing, _) => "game_panel.starting",
            (RuntimePhase::Running, true) => "game_panel.hidden",
            (RuntimePhase::Running, false) => "game_panel.own_window",
        };
        renzora::lang::t(key)
    });

    let bar = crate::load_overlay::striped_bar(commands);
    bind_display(commands, bar, |w| {
        w.get_resource::<ExternalRuntime>()
            .is_some_and(|r| r.phase() == RuntimePhase::Preparing)
    });

    commands.entity(root).add_children(&[label, bar]);
    root
}

/// A floating editor surface's box and whether it is on screen.
type EditorSurface = (
    &'static ComputedNode,
    &'static UiGlobalTransform,
    &'static InheritedVisibility,
    &'static Node,
);
type Floating = Or<(With<OverlaySurface>, With<ModalSurface>)>;

/// Keep the embedded game's window over the Game panel: find it once the game
/// has made it a child of the editor's window, then move, resize, show and
/// hide it to match the panel.
#[allow(clippy::too_many_arguments)]
fn place_game_window(
    mut runtime: ResMut<ExternalRuntime>,
    mut state: ResMut<GameEmbedState>,
    settings: Option<Res<EditorSettings>>,
    dock: Option<Res<renzora_ember::dock::Dock>>,
    fixed: Option<Res<renzora_ember::dock::FixedDock>>,
    surfaces: Query<(&ComputedNode, &UiGlobalTransform, &InheritedVisibility), With<GameSurface>>,
    overlays: Query<EditorSurface, Floating>,
    windows: Query<&bevy::window::RawHandleWrapper, With<bevy::window::PrimaryWindow>>,
) {
    let Some(pid) = runtime.pid() else {
        *state = GameEmbedState::default();
        return;
    };
    if !EMBED_SUPPORTED || !settings.is_some_and(|s| s.embed_game) {
        return;
    }
    let Ok(editor) = windows.single() else { return };
    let raw_window_handle::RawWindowHandle::Win32(editor) = editor.get_window_handle() else {
        return;
    };
    let editor = editor.hwnd.get();

    let hwnd = match state.hwnd {
        Some(hwnd) if win32::is_window(hwnd) => hwnd,
        _ => {
            state.placed = None;
            let Some(hwnd) = win32::find_child_of_process(editor, pid) else {
                state.hwnd = None;
                return;
            };
            state.hwnd = Some(hwnd);
            state.holes = Default::default();
            runtime.mark_running();
            hwnd
        }
    };

    // Only the primary window's docks: a panel floated into a window of its
    // own lives in a different OS window than the one the game is parented
    // to, so the game stays hidden until the panel is docked again.
    let tab_shown = dock.is_some_and(|d| d.tree.is_active_tab(GAME_PANEL_ID))
        || fixed.is_some_and(|f| f.tree.is_active_tab(GAME_PANEL_ID));
    let surface = surfaces
        .iter()
        .find(|(node, _, vis)| vis.get() && node.size().min_element() > 1.0);
    let (rect, shown) = match surface {
        Some((node, transform, _)) if tab_shown => {
            let size = node.size();
            let min = transform.translation - size * 0.5;
            (
                IRect::from_corners(min.round().as_ivec2(), (min + size).round().as_ivec2()),
                true,
            )
        }
        _ => (state.placed.map_or(IRect::default(), |(r, _)| r), false),
    };
    if state.placed != Some((rect, shown)) {
        state.placed = Some((rect, shown));
        win32::place(hwnd, rect, shown);
    }

    let mut holes: Vec<IRect> = if shown {
        overlays
            .iter()
            .filter(|(node, _, vis, style)| {
                vis.get() && style.display != Display::None && node.size().min_element() > 1.0
            })
            .map(|(node, transform, ..)| {
                let min = transform.translation - node.size() * 0.5;
                IRect::from_corners(min.round().as_ivec2(), (min + node.size()).round().as_ivec2())
                    .intersect(rect)
            })
            .filter(|hole| !hole.is_empty())
            .map(|hole| IRect::from_corners(hole.min - rect.min, hole.max - rect.min))
            .collect()
    } else {
        Vec::new()
    };
    holes.sort_by_key(|r| (r.min.x, r.min.y, r.max.x, r.max.y));
    let size = if holes.is_empty() { IVec2::ZERO } else { rect.size() };
    if (&holes, size) != (&state.holes.0, state.holes.1) {
        win32::set_holes(hwnd, size, holes.clone());
        state.holes = (holes, size);
    }
}

#[cfg(target_os = "windows")]
mod win32 {
    use bevy::math::IRect;

    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_SHOWWINDOW: u32 = 0x0040;
    const SWP_HIDEWINDOW: u32 = 0x0080;
    /// Don't wait for the game's thread to handle the move. Without it a game
    /// stuck in a long frame would stall the editor's frame along with it,
    /// which is the very thing running it in its own process is for.
    const SWP_ASYNCWINDOWPOS: u32 = 0x4000;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumChildWindows(
            parent: isize,
            callback: unsafe extern "system" fn(isize, isize) -> i32,
            lparam: isize,
        ) -> i32;
        fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
        fn GetParent(hwnd: isize) -> isize;
        fn IsWindow(hwnd: isize) -> i32;
        fn SetWindowPos(hwnd: isize, after: isize, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
        fn SetWindowRgn(hwnd: isize, region: isize, redraw: i32) -> i32;
    }

    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn CreateRectRgn(left: i32, top: i32, right: i32, bottom: i32) -> isize;
        fn CombineRgn(dest: isize, a: isize, b: isize, mode: i32) -> i32;
        fn DeleteObject(object: isize) -> i32;
    }

    const RGN_DIFF: i32 = 4;

    /// Show the game's window everywhere except `holes` (its own client
    /// pixels, within `size`), or everywhere when there are none.
    ///
    /// On a thread of its own: setting a window's region waits for the
    /// window's thread to take it, and the game's thread is in another process
    /// that may be stuck in a long frame. `place` avoids the same wait with
    /// `SWP_ASYNCWINDOWPOS`; there is no asynchronous form of this call.
    pub fn set_holes(hwnd: isize, size: bevy::math::IVec2, holes: Vec<IRect>) {
        std::thread::spawn(move || {
            // SAFETY: a stale handle makes the calls fail, nothing more. A
            // region handed to `SetWindowRgn` belongs to the system after a
            // successful call, so only the per-hole regions are deleted here,
            // and the combined one only when the call failed.
            unsafe {
                if holes.is_empty() {
                    SetWindowRgn(hwnd, 0, 1);
                    return;
                }
                let shape = CreateRectRgn(0, 0, size.x, size.y);
                for hole in &holes {
                    let cut = CreateRectRgn(hole.min.x, hole.min.y, hole.max.x, hole.max.y);
                    CombineRgn(shape, shape, cut, RGN_DIFF);
                    DeleteObject(cut);
                }
                if SetWindowRgn(hwnd, shape, 1) == 0 {
                    DeleteObject(shape);
                }
            }
        });
    }

    struct Search {
        parent: isize,
        pid: u32,
        found: isize,
    }

    unsafe extern "system" fn visit(hwnd: isize, lparam: isize) -> i32 {
        // SAFETY: `lparam` is the `&mut Search` passed to `EnumChildWindows`
        // below, alive for the whole enumeration.
        let search = unsafe { &mut *(lparam as *mut Search) };
        let mut pid = 0u32;
        // SAFETY: `hwnd` comes from the enumeration and `pid` is a valid out
        // pointer.
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        if pid == search.pid && unsafe { GetParent(hwnd) } == search.parent {
            search.found = hwnd;
            return 0;
        }
        1
    }

    /// The direct child of `parent` that belongs to process `pid`.
    pub fn find_child_of_process(parent: isize, pid: u32) -> Option<isize> {
        let mut search = Search {
            parent,
            pid,
            found: 0,
        };
        // SAFETY: the callback only reads through the pointer while this
        // frame keeps `search` alive.
        unsafe { EnumChildWindows(parent, visit, &mut search as *mut Search as isize) };
        (search.found != 0).then_some(search.found)
    }

    pub fn is_window(hwnd: isize) -> bool {
        // SAFETY: `IsWindow` accepts any value and only reports on it.
        unsafe { IsWindow(hwnd) != 0 }
    }

    /// Move the game's window to `rect` (editor client pixels) and show or
    /// hide it.
    pub fn place(hwnd: isize, rect: IRect, shown: bool) {
        let visibility = if shown { SWP_SHOWWINDOW } else { SWP_HIDEWINDOW };
        let size = rect.size().max(bevy::math::IVec2::ONE);
        // SAFETY: a stale handle makes the call fail, nothing more.
        unsafe {
            SetWindowPos(
                hwnd,
                0,
                rect.min.x,
                rect.min.y,
                size.x,
                size.y,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS | visibility,
            )
        };
    }
}

/// No embedding outside Windows yet; `EMBED_SUPPORTED` keeps these unreached.
#[cfg(not(target_os = "windows"))]
mod win32 {
    use bevy::math::IRect;

    pub fn find_child_of_process(_parent: isize, _pid: u32) -> Option<isize> {
        None
    }
    pub fn is_window(_hwnd: isize) -> bool {
        false
    }
    pub fn place(_hwnd: isize, _rect: IRect, _shown: bool) {}
    pub fn set_holes(_hwnd: isize, _size: bevy::math::IVec2, _holes: Vec<IRect>) {}
}
