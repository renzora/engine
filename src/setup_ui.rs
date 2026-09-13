//! The setup window: a progress bar and a build log, shown before the editor
//! exists.
//!
//! A downloaded release has to unpack the SDK and compile its native plugins
//! before the editor can load them, and that takes a few seconds to a few
//! minutes. Doing it silently would look like a hang — the window simply would
//! not appear — so this puts up a small window that says what is happening.
//!
//! It is not only a *first* run. The same work is due after an engine update
//! (every plugin's stamp stops matching) and after editing a plugin's source,
//! so nothing here calls it one.
//!
//! # Why this is its own Bevy `App`
//!
//! The work has to finish before [`renzora_native_plugin::NativePluginLoader`]
//! runs, and that runs during the *editor's* `App` assembly. So there is no
//! editor `App` to draw into yet, and the splash — which is the natural place
//! for "getting ready" — comes later still.
//!
//! A second, tiny `App` is the way out: bare `DefaultPlugins`, one window, a
//! handful of nodes. It runs, finishes, and the process restarts into the
//! ordinary editor with everything in place. Nothing here uses `renzora_ember`,
//! deliberately — the theme, fonts and stylesheet all live behind engine plugins
//! this app does not have, and pulling them in would make setup depend on most
//! of the thing it exists to set up.
//!
//! # Why the chrome is hand-drawn
//!
//! The window is borderless and carries the splash screen's title bar: the
//! product mark, the name, the version, and the window controls on the right. It
//! is the first window of a launch that ends in the splash, so an OS title bar
//! saying "setup" in the middle of it announced a utility rather than the thing
//! that was starting.
//!
//! Every part of it is spelled out here rather than reused from
//! `renzora_splash::launcher::chrome`, for the same reason the rest of this file
//! avoids ember: that bar is built from ember fonts, the icon font and the
//! reactive bindings, all of which arrive with plugins this app does not run. So
//! the palette is copied, the icon is decoded straight off the disk, and the
//! control glyphs are drawn as plain nodes. There is no maximize control,
//! because the window is not resizable.
//!
//! # Why the work runs on a thread
//!
//! `prebuild::run` is blocking: it decompresses ~1.9 GB and then drives `rustc`
//! once per plugin. On the main thread the window would freeze for the whole
//! duration and Windows would grey it out as "not responding" — the exact
//! impression the bar exists to avoid. It runs on a plain `std::thread` and
//! publishes progress through a mutex the render loop samples each frame.
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::math::Rot2;
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, UiTransform};
use bevy::window::{PrimaryWindow, WindowPlugin, WindowResolution};
// Reached through `renzora_runtime` rather than as a direct dependency: this
// binary already depends on the runtime, and the re-export is what the editor
// executable used to provide before it became a loadable image.
use renzora_runtime::renzora_native_plugin::prebuild::{self, Progress};

/// How many log lines the window shows. The pane fills whatever is left under
/// the bar and the lines are a fixed set of `Text` nodes rewritten in place — a
/// log that spawned a node per line would churn the UI for the whole of a long
/// build. Sized to fill the window at [`WINDOW_H`]; a few spare rows at the
/// bottom cost nothing, a short pane would waste the space.
const LOG_LINES: usize = 21;

/// The title bar's height, matching the splash screen's `TITLEBAR_H`.
const TITLEBAR_H: f32 = 38.0;

const WINDOW_W: u32 = 760;
const WINDOW_H: u32 = 470;

/// Shared between the worker thread and the render loop.
#[derive(Default)]
struct Shared {
    latest: Option<Progress>,
    /// Set once the worker is finished, whatever the outcome.
    finished: bool,
    /// The build log, newest last. Bounded — a long build's output is not worth
    /// holding in full when only the last [`LOG_LINES`] are ever drawn.
    log: VecDeque<String>,
    /// Every [`Progress::Failed`] as it arrived, so the finished window can say
    /// what went wrong. The caption only ever holds the *latest* step, and a
    /// failure is usually followed by more steps that scroll it away.
    failures: Vec<String>,
    /// What the worker actually did, for the summary line.
    prepared: prebuild::Prepared,
    /// A toolchain the run needed and did not find. Drives the second button.
    gap: Option<prebuild::ToolchainGap>,
}

impl Shared {
    fn push_log(&mut self, line: String) {
        if self.log.len() >= LOG_LINES * 4 {
            self.log.pop_front();
        }
        self.log.push_back(line);
    }
}

#[derive(Resource, Clone)]
struct Work(Arc<Mutex<Shared>>);

#[derive(Component)]
struct BarFill;

#[derive(Component)]
struct StatusText;

/// One line of the build log pane, by row index (0 = oldest shown).
#[derive(Component)]
struct LogLine(usize);

/// Offers to install a missing toolchain. Shown only when one is missing AND
/// the editor can actually add it — see [`prebuild::ToolchainGap`].
#[derive(Component)]
struct InstallBtn;

/// The install button's caption, which names the version being fetched.
#[derive(Component)]
struct InstallLabel;

/// The progress bar's track, hidden once the worker finishes — a full bar under
/// a summary reads as "still going", and the button is the live thing then.
#[derive(Component)]
struct BarTrack;

/// The title bar, which drags the borderless window.
#[derive(Component)]
struct DragHandle;

/// One of the window controls, and which one.
#[derive(Component, Clone, Copy)]
enum WinBtn {
    Minimize,
    Close,
}

/// What to call the thing being set up.
///
/// The executable's own file name, not "Renzora". This window belongs to one
/// binary that is the editor when the editor image sits beside it and a shipped
/// game when it does not — so in a player's hands it says the name of the game
/// they launched, which is the only name that means anything to them. Saying
/// "Setting up Renzora" there names an engine they may never have heard of.
///
/// Falls back to the engine name when the executable cannot be resolved, which
/// is the editor's case anyway.
fn product_name() -> String {
    std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(std::path::Path::file_stem)
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .map(|s| {
            // `hey.exe` reads better as `Hey` in a window title. Only the first
            // character — a game called `myGame` keeps the rest as its author
            // wrote it.
            let mut c = s.chars();
            match c.next() {
                Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
                None => s,
            }
        })
        .unwrap_or_else(|| "Renzora".to_string())
}

/// How the setup window ended, which is the caller's whole decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The work finished (or failed and was reported) and the window closed
    /// itself. Restart into the editor.
    Finished,
    /// The user closed the window while it was still working.
    ///
    /// This has to be told apart from [`Finished`](Self::Finished), because the
    /// caller's answer to that one is to relaunch the process — and an
    /// interrupted build leaves `prebuild::needed()` true, so relaunching puts
    /// the same window straight back up. Closing the window looked like it
    /// spawned another one.
    Cancelled,
}

/// Set when the user closes the window, read once [`run`]'s `App` has ended.
///
/// An `Arc<AtomicBool>` rather than a resource read afterwards because
/// `App::run` consumes the app: by the time it returns there is no `World` left
/// to ask. `AppExit` cannot carry the answer either — the finish path writes the
/// same `AppExit::Success`.
#[derive(Resource, Clone)]
struct Cancelled(Arc<AtomicBool>);

/// Run setup with a window, returning how it ended.
///
/// The window closes itself when the work is done — see the note in [`tick`] —
/// so returning normally means "setup ran". The exception is the user closing
/// it, which is [`Outcome::Cancelled`] and means "stop", not "start over".
///
/// The caller restarts afterwards; this function does not, so that the decision
/// stays in `main` where the rest of the boot sequence is visible.
pub fn run() -> Outcome {
    let shared = Arc::new(Mutex::new(Shared::default()));
    let cancelled = Arc::new(AtomicBool::new(false));

    spawn_worker(shared.clone());

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                // Not "first run": this window is also how an update and an
                // edited plugin get rebuilt, and both are ordinary launches.
                // Only the taskbar reads this, the bar being drawn below.
                title: format!("{} setup", product_name()),
                resolution: WindowResolution::new(WINDOW_W, WINDOW_H),
                resizable: false,
                // The bar is ours, like the splash's. See the module docs.
                decorations: false,
                // Centred and alone: this is a modal moment, not a workspace.
                position: WindowPosition::Centered(MonitorSelection::Primary),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(WINDOW_BG))
        .insert_resource(Work(shared))
        .insert_resource(Cancelled(cancelled.clone()))
        .add_systems(Startup, spawn_ui)
        .add_systems(Update, (tick, chrome_input, cancel_on_os_close))
        .run();

    if cancelled.load(Ordering::Relaxed) {
        Outcome::Cancelled
    } else {
        Outcome::Finished
    }
}

/// Alt+F4, the taskbar's Close, the window manager's own × — the window has no
/// decorations, but every one of those still arrives, and each means what the
/// bar's × means.
///
/// Bevy's `close_when_requested` answers them by despawning the window, which
/// ends the app with the same `AppExit::Success` the finish path writes. Without
/// this the flag would be unset and the caller would relaunch.
fn cancel_on_os_close(
    mut closes: MessageReader<bevy::window::WindowCloseRequested>,
    cancelled: Res<Cancelled>,
) {
    if closes.read().next().is_some() {
        cancelled.0.store(true, Ordering::Relaxed);
    }
}

// The splash screen's palette. Copied rather than imported: `launcher::style`
// is `pub(crate)` inside a crate this app deliberately does not link (see the
// module docs), and a progress window needs this handful of its colors.
const WINDOW_BG: Color = Color::srgb(0.039, 0.047, 0.078);
const BAR_BG: Color = Color::srgb(0.027, 0.035, 0.059);
const LOG_BG: Color = Color::srgb(0.016, 0.020, 0.035);
const TEXT: Color = Color::srgb(0.878, 0.894, 0.941);
const MUTED: Color = Color::srgb(0.588, 0.620, 0.698);
const ACCENT: Color = Color::srgb(0.431, 0.588, 1.0);
/// The track under the fill: the log pane's ground, one step darker than the
/// window, so an empty bar reads as empty rather than as a filled grey one.
const TRACK_BG: Color = Color::srgb(0.078, 0.090, 0.133);
/// Hover fills for the window controls, straight from the splash's chrome.
const BTN_HOVER: Color = Color::srgba(1.0, 1.0, 1.0, 0.133);
const CLOSE_HOVER: Color = Color::srgb(0.910, 0.067, 0.137);

/// Run the setup pass on a worker thread, reporting into `shared`.
///
/// A function rather than an inline spawn because it runs **twice** in the one
/// case this window exists to rescue: install the missing toolchain, then do the
/// work that was waiting for it. Restarting the process to achieve the same
/// thing would drop the log the user is reading.
fn spawn_worker(worker: Arc<Mutex<Shared>>) {
    std::thread::spawn(move || {
        let prepared = prebuild::run(&mut |p| {
            // Also logged to stderr: the window shows the build, but a failure
            // needs to survive the window closing.
            if let Progress::Failed(e) = &p {
                eprintln!("[setup] {e}");
            }
            let mut s = worker.lock().expect("setup progress lock");
            match &p {
                // Unpacking reports continuously and has a real fraction — it
                // belongs on the bar, not as thousands of log lines.
                Progress::Unpacking { .. } => {}
                // No counter here: several plugins compile at once, so the
                // number this carries is where the build STARTED and would read
                // as the log jumping around. The count belongs on `Built`, which
                // is the line that means one is actually done.
                Progress::Building { name, .. } => {
                    s.push_log(format!("Compiling {name}"));
                }
                Progress::Built { name, done, total } => {
                    s.push_log(format!("Built {name}  ({done}/{total})"));
                }
                Progress::Compiling { line, .. } => {
                    let line = line.trim();
                    if !line.is_empty() {
                        s.push_log(format!("   {line}"));
                    }
                }
                Progress::Failed(e) => {
                    s.failures.push(e.clone());
                    s.push_log(format!("error: {e}"));
                }
            }
            s.latest = Some(p);
        });
        let mut s = worker.lock().expect("setup progress lock");
        s.prepared = prepared;
        s.gap = prebuild::toolchain_gap();
        s.finished = true;
    });
}

/// Install the pinned toolchain, then run the setup pass again.
///
/// The window stays open across both, which is the point: the plugins that were
/// waiting on the toolchain build in the same session that installed it, with
/// their output in the same log. Restarting the process between the two would
/// work and would throw away everything the user is reading.
///
/// Failure is reported into the same log and leaves the button offered again —
/// a network drop is worth a second press, and rustup's own message says more
/// about why than anything this could add.
fn start_install(shared: &Arc<Mutex<Shared>>, version: String) {
    {
        let mut s = shared.lock().expect("setup progress lock");
        s.finished = false;
        s.gap = None;
        // The previous run's failures were all downstream of the missing
        // toolchain. Keeping them would summarise a state that no longer exists.
        s.failures.clear();
        s.push_log(format!("Installing Rust {version} (rustup, minimal profile)…"));
    }
    let shared = shared.clone();
    std::thread::spawn(move || {
        match prebuild::install_toolchain(&version) {
            Ok(()) => {
                shared
                    .lock()
                    .expect("setup progress lock")
                    .push_log(format!("Installed Rust {version}"));
                // Straight on into the work it was blocking.
                spawn_worker(shared);
            }
            Err(e) => {
                eprintln!("[setup] rustup: {e}");
                let mut s = shared.lock().expect("setup progress lock");
                s.push_log(format!("error: rustup could not install {version}: {e}"));
                s.failures.push(format!("Rust {version} could not be installed: {e}"));
                s.gap = Some(prebuild::ToolchainGap::Installable { version });
                s.finished = true;
            }
        }
    });
}

fn spawn_ui(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn(Camera2d);

    let root = commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(WINDOW_BG),
            Name::new("SetupRoot"),
        ))
        .id();

    let bar = build_title_bar(&mut commands, &mut images);

    // Everything under the bar, filling the rest of the window. The log grows
    // into whatever is left rather than the column being centred with a fixed
    // pane: centred, the finished window carried a band of dead space under the
    // log, which read as something still to come.
    let body = commands
        .spawn((
            Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                flex_direction: FlexDirection::Column,
                row_gap: px(10),
                padding: UiRect::all(px(18)),
                ..default()
            },
            Name::new("SetupBody"),
        ))
        .id();

    let status = commands
        .spawn((
            Text::new("Preparing…"),
            TextFont::from_font_size(15.0),
            TextColor(TEXT),
            StatusText,
            // One line, always. Compiler output is arbitrarily long and
            // arbitrarily wide; wrapping it would grow this caption to several
            // lines and shove everything below it down the window as the text
            // changed.
            bevy::text::TextLayout::no_wrap(),
            Node { width: percent(100), overflow: Overflow::clip(), ..default() },
        ))
        .id();

    // The track. The fill is a child so its width can be a percentage of it
    // rather than of the window.
    let fill = commands
        .spawn((
            Node {
                width: percent(0),
                height: percent(100),
                border_radius: BorderRadius::all(px(7)),
                ..default()
            },
            BackgroundColor(ACCENT),
            BarFill,
        ))
        .id();
    let track = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(14),
                flex_shrink: 0.0,
                border_radius: BorderRadius::all(px(7)),
                ..default()
            },
            BackgroundColor(TRACK_BG),
            BarTrack,
        ))
        .id();
    commands.entity(track).add_child(fill);

    // The build log. Every plugin as it starts compiling, plus whatever the
    // compiler says — which is what you would be watching in a terminal, and
    // the reason this window exists instead of one.
    //
    // A plugin with no third-party dependencies produces no compiler output at
    // all until something goes wrong (rustc is invoked directly, so there are
    // no cargo "Compiling" lines for its dependencies), so on a clean run this
    // is one line per plugin. That is still the thing worth seeing: which
    // plugin is taking the time.
    let log = commands
        .spawn((
            Node {
                width: percent(100),
                // Takes the rest of the window rather than a height of its own,
                // so the pane ends where the window does.
                flex_grow: 1.0,
                min_height: px(0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(10)),
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(LOG_BG),
            Name::new("SetupLog"),
        ))
        .id();
    let lines: Vec<Entity> = (0..LOG_LINES)
        .map(|i| {
            commands
                .spawn((
                    Text::new(""),
                    TextFont::from_font_size(11.0),
                    TextColor(MUTED),
                    LogLine(i),
                    bevy::text::TextLayout::no_wrap(),
                    // Fixed rows: the pane now flex-grows, and a shrinkable row
                    // would let a full log squeeze its own lines instead of
                    // running off the bottom of the clip.
                    Node { height: px(15.0), flex_shrink: 0.0, overflow: Overflow::clip(), ..default() },
                ))
                .id()
        })
        .collect();
    commands.entity(log).add_children(&lines);

    // Offered rather than imposed, and only when it is ours to offer: rustup is
    // already installed, so this adds a toolchain to it rather than putting Rust
    // on someone's machine. The caption carries the version, which is not known
    // until the worker has looked.
    let install_label = commands
        .spawn((
            Text::new(""),
            TextFont::from_font_size(14.0),
            TextColor(TEXT),
            InstallLabel,
        ))
        .id();
    let install = commands
        .spawn((
            Button,
            Node {
                // A row wrapping this button would keep its place in the column
                // gap even while empty, which is the band of space that used to
                // sit under the log on every run that had nothing to offer. The
                // button is its own child of the body instead: `Display::None`
                // takes an item out of the layout, gap and all.
                display: Display::None,
                align_self: AlignSelf::FlexEnd,
                padding: UiRect::axes(px(22), px(9)),
                border_radius: BorderRadius::all(px(6)),
                border: UiRect::all(px(1)),
                ..default()
            },
            BorderColor::all(MUTED),
            BackgroundColor(Color::NONE),
            InstallBtn,
        ))
        .id();
    commands.entity(install).add_child(install_label);

    commands.entity(body).add_children(&[status, track, log, install]);
    commands.entity(root).add_children(&[bar, body]);
}

/// The splash screen's title bar: the mark, the product name and the engine
/// version on the left, the window controls on the right, everything between
/// them a drag handle.
fn build_title_bar(commands: &mut Commands, images: &mut Assets<Image>) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(TITLEBAR_H),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::left(px(14)),
                ..default()
            },
            BackgroundColor(BAR_BG),
            // Blocks so the press starts a window drag here rather than falling
            // through to whatever is behind the bar.
            FocusPolicy::Block,
            Interaction::default(),
            DragHandle,
            Name::new("SetupTitleBar"),
        ))
        .id();

    let brand = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(8),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let mark = build_mark(commands, images);
    let name = commands
        .spawn((
            Text::new(product_name()),
            TextFont::from_font_size(13.0),
            TextColor(TEXT),
            FocusPolicy::Pass,
        ))
        .id();
    // The separator the splash writes as "·". Drawn rather than typed: the only
    // font here is Bevy's embedded fallback, whose coverage past ASCII is not
    // something to bet the title bar on.
    let dot = commands
        .spawn((
            Node {
                width: px(3),
                height: px(3),
                border_radius: BorderRadius::all(px(1.5)),
                ..default()
            },
            BackgroundColor(MUTED),
            FocusPolicy::Pass,
        ))
        .id();
    let version = commands
        .spawn((
            Text::new(renzora_runtime::renzora::version::display()),
            TextFont::from_font_size(11.0),
            TextColor(MUTED),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(brand).add_children(&[mark, name, dot, version]);

    let controls = commands
        .spawn((
            Node {
                height: percent(100),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let minimize = win_button(commands, WinBtn::Minimize);
    let close = win_button(commands, WinBtn::Close);
    commands.entity(controls).add_children(&[minimize, close]);

    commands.entity(bar).add_children(&[brand, controls]);
    bar
}

/// The product mark: the real icon staged beside the executable, with a plain
/// accent tile standing in where the file is not there (a bare `cargo run`,
/// which stages nothing, chiefly).
///
/// Decoded here and now rather than requested from the `AssetServer`, which
/// resolves against an asset root this window has no reason to have set up. It
/// is one small PNG, once, on a frame nobody is waiting on.
fn build_mark(commands: &mut Commands, images: &mut Assets<Image>) -> Entity {
    const MARK: f32 = 20.0;

    let frame = commands
        .spawn((
            Node {
                width: px(MARK),
                height: px(MARK),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();

    let child = match brand_icon() {
        Some(image) => commands
            .spawn((
                ImageNode::new(images.add(image)),
                Node { width: percent(100), height: percent(100), ..default() },
                FocusPolicy::Pass,
            ))
            .id(),
        None => commands
            .spawn((
                Node {
                    width: px(13),
                    height: px(13),
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
                BackgroundColor(ACCENT),
                FocusPolicy::Pass,
            ))
            .id(),
    };
    commands.entity(frame).add_child(child);
    frame
}

/// The icon staged beside the executable as `resources/icon.png` (the same file
/// the splash's mark and the exporter's fallback use), decoded.
fn brand_icon() -> Option<Image> {
    let path: PathBuf = std::env::current_exe()
        .ok()?
        .parent()?
        .join("resources")
        .join("icon.png");
    let bytes = std::fs::read(path).ok()?;
    Image::from_buffer(
        &bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::Default,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .ok()
}

/// One window control: a 44px key with its glyph drawn from nodes.
fn win_button(commands: &mut Commands, kind: WinBtn) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: px(44),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            // Blocks, or the press reaches the bar underneath and starts a
            // window drag as well as pressing the button.
            FocusPolicy::Block,
            kind,
            Name::new("SetupWinBtn"),
        ))
        .id();

    match kind {
        WinBtn::Minimize => {
            let dash = commands
                .spawn((
                    Node { width: px(11), height: px(1.0), ..default() },
                    BackgroundColor(TEXT),
                    FocusPolicy::Pass,
                ))
                .id();
            commands.entity(btn).add_child(dash);
        }
        WinBtn::Close => {
            // Two bars crossed. The icon font the splash draws this from is
            // ember's, which this app does not load.
            let cross = commands
                .spawn((
                    Node { width: px(11), height: px(11), ..default() },
                    FocusPolicy::Pass,
                ))
                .id();
            for angle in [std::f32::consts::FRAC_PI_4, -std::f32::consts::FRAC_PI_4] {
                let bar = commands
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(0),
                            top: percent(50),
                            width: percent(100),
                            height: px(1.0),
                            // Half the bar's height, so it straddles the centre
                            // line rather than hanging off it.
                            margin: UiRect::top(px(-0.5)),
                            ..default()
                        },
                        UiTransform { rotation: Rot2::radians(angle), ..default() },
                        BackgroundColor(TEXT),
                        FocusPolicy::Pass,
                    ))
                    .id();
                commands.entity(cross).add_child(bar);
            }
            commands.entity(btn).add_child(cross);
        }
    }
    btn
}

/// Drag the window by its bar, and answer the two controls.
///
/// Closing ends the app AND records that the user asked for it, so `main` quits
/// instead of relaunching. It used to only write `AppExit`, and `main` restarted
/// either way — which, mid-build, meant the relaunched process found the work
/// still outstanding and opened this window again. Pressing × spawned a window.
fn chrome_input(
    drag: Query<&Interaction, (Changed<Interaction>, With<DragHandle>)>,
    mut buttons: Query<(&Interaction, &mut BackgroundColor, &WinBtn), Changed<Interaction>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    cancelled: Res<Cancelled>,
    mut exit: MessageWriter<AppExit>,
) {
    let mut handed_to_wm = false;

    if drag.iter().any(|i| *i == Interaction::Pressed) {
        if let Ok(mut window) = windows.single_mut() {
            window.start_drag_move();
            handed_to_wm = true;
        }
    }

    for (interaction, mut bg, kind) in &mut buttons {
        bg.0 = match (interaction, kind) {
            (Interaction::None, _) => Color::NONE,
            (_, WinBtn::Close) => CLOSE_HOVER,
            (_, WinBtn::Minimize) => BTN_HOVER,
        };
        if *interaction != Interaction::Pressed {
            continue;
        }
        match kind {
            WinBtn::Close => {
                cancelled.0.store(true, Ordering::Relaxed);
                exit.write(AppExit::Success);
            }
            WinBtn::Minimize => {
                if let Ok(mut window) = windows.single_mut() {
                    window.set_minimized(true);
                }
            }
        }
    }

    // The window manager owns the button now and never hands back the release,
    // so `Interaction` would stay `Pressed` and the next drag's
    // `Changed<Interaction>` would never fire. `renzora_ui::window_chrome` has
    // the long version of this; `reset` rather than `release` so no button under
    // the cursor sees a synthetic click.
    if handed_to_wm {
        mouse.reset(MouseButton::Left);
    }
}

fn tick(
    work: Res<Work>,
    // Every `&mut Node` query here must be provably disjoint from every other,
    // or Bevy refuses the system at init — `error[B0001]`, at run time, because
    // the check is on the resolved archetypes and no compiler can see it. Adding
    // a fourth button meant every earlier query needed to exclude it too, which
    // is the part that is easy to miss: the new query is not the one that breaks.
    mut fill: Query<
        &mut Node,
        (With<BarFill>, Without<BarTrack>, Without<InstallBtn>),
    >,
    mut track: Query<&mut Node, (With<BarTrack>, Without<InstallBtn>)>,
    mut install: Query<(&mut Node, &Interaction), With<InstallBtn>>,
    mut install_label: Query<&mut Text, (With<InstallLabel>, Without<StatusText>, Without<LogLine>)>,
    mut status: Query<&mut Text, (With<StatusText>, Without<LogLine>)>,
    mut log: Query<(&LogLine, &mut Text), Without<StatusText>>,
    mut writer: MessageWriter<AppExit>,
) {
    let (latest, finished, lines, summary, gap) = {
        let s = work.0.lock().expect("setup progress lock");
        // Only the tail is drawn; copying it under the lock keeps the lock held
        // for as long as it takes to clone ~14 short strings.
        let start = s.log.len().saturating_sub(LOG_LINES);
        let lines: Vec<String> = s.log.iter().skip(start).cloned().collect();
        let summary = s.finished.then(|| summarize(&s.prepared, &s.failures));
        (s.latest.clone(), s.finished, lines, summary, s.gap.clone())
    };

    for (slot, mut text) in &mut log {
        let want = lines.get(slot.0).map(String::as_str).unwrap_or("");
        if text.as_str() != want {
            **text = want.to_string();
        }
    }

    if finished {
        // Swap the bar for the button, and the step caption for a summary of
        // what happened. Done every frame rather than once: this system holds no
        // state, and writing the same values again costs a string compare.
        let summary = summary.unwrap_or_default();
        if let Ok(mut text) = status.single_mut() {
            if text.as_str() != summary {
                **text = summary;
            }
        }
        if let Ok(mut node) = track.single_mut() {
            node.display = Display::None;
        }

        // The offer, when there is one to make. `RustupMissing` deliberately
        // gets no button: the summary says where to get Rust, and installing it
        // is a decision about the user's machine rather than about this editor.
        let offer = match &gap {
            Some(prebuild::ToolchainGap::Installable { version }) => Some(version.clone()),
            _ => None,
        };
        if let Ok((mut node, interaction)) = install.single_mut() {
            match &offer {
                None => node.display = Display::None,
                Some(version) => {
                    node.display = Display::Flex;
                    if let Ok(mut text) = install_label.single_mut() {
                        let want = format!("Install Rust {version}");
                        if text.as_str() != want {
                            **text = want;
                        }
                    }
                    if *interaction == Interaction::Pressed {
                        start_install(&work.0, version.clone());
                        return;
                    }
                }
            }
        }

        // Nothing left to decide, so nothing to press: the editor starts.
        //
        // There used to be a Start Editor button here, and the reason was that
        // this window once closed the instant the worker finished — a plugin
        // that failed to compile was on screen for a single frame. That reason
        // is gone rather than overruled: a failure is written to stderr as it
        // happens, and the loader records it in the plugin inventory, so it is
        // waiting in Settings ▸ Editor ▸ Plugins when the editor comes up.
        // Holding a window shut until someone acknowledges a message they can
        // still read afterwards is a toll, not a safeguard.
        //
        // The exception is an offer, above. While one stands, the window is the
        // only place it can be taken, so closing would answer it by default —
        // and the default would be no.
        if offer.is_none() {
            writer.write(AppExit::Success);
        }
        return;
    }

    if let Some(p) = &latest {
        if let Ok(mut text) = status.single_mut() {
            let line = p.to_string();
            if text.as_str() != line {
                **text = line;
            }
        }
        // Unpacking has a real ratio; compiling does not (rustc reports nothing
        // until it is done), so plugin steps advance by whole plugins.
        //
        // A failure has no fraction, so it leaves the bar where it is — but it
        // must NOT return from this system. It used to, and the `finished` check
        // above is what starts the editor: one plugin that failed to compile
        // therefore left the setup window on screen forever, with the editor
        // never starting. A plugin the user is still writing is the single most
        // likely thing to fail here, so "does not compile" has to mean "is
        // skipped", never "nothing runs".
        let frac = match p {
            Progress::Unpacking { done, total } => Some(*done as f32 / (*total).max(1) as f32),
            // Completions, not starts. Eight plugins begin within milliseconds of
            // each other, so a bar driven by `Building` leapt to 8/52 and then
            // held still for the four seconds they all took — and reached the end
            // while eight were still compiling.
            Progress::Built { done, total, .. } => Some(*done as f32 / (*total).max(1) as f32),
            Progress::Building { .. } => None,
            // A compiler line reports what is happening, not how far along it
            // is — the bar stays where `Building` put it and only the caption
            // moves. Same reasoning as `Failed`: no fraction is not zero.
            Progress::Compiling { .. } | Progress::Failed(_) => None,
        };
        if let (Some(frac), Ok(mut node)) = (frac, fill.single_mut()) {
            node.width = percent(frac.clamp(0.0, 1.0) * 100.0);
        }
    }
}

/// One line describing how setup went, for the finished window.
///
/// Failures come first and win the line: a run that unpacked the SDK and built
/// nine plugins but lost the tenth is a run the user needs to know about, and
/// "Set up 9 plugins" would bury that. Only the first failure is named — the
/// rest are counted, because this is one un-wrapped line and the log pane above
/// it has them all.
fn summarize(prepared: &prebuild::Prepared, failures: &[String]) -> String {
    if let Some(first) = failures.first() {
        return match failures.len() {
            1 => first.clone(),
            n => format!("{first}  (+{} more, see the log above)", n - 1),
        };
    }
    match (prepared.unpacked_sdk, prepared.built) {
        (true, 0) => "Rust SDK unpacked. Ready.".to_string(),
        (true, 1) => "Rust SDK unpacked, 1 plugin built. Ready.".to_string(),
        (true, n) => format!("Rust SDK unpacked, {n} plugins built. Ready."),
        (false, 0) => "Nothing to do. Ready.".to_string(),
        (false, 1) => "1 plugin built. Ready.".to_string(),
        (false, n) => format!("{n} plugins built. Ready."),
    }
}
