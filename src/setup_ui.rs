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
//! # Why the work runs on a thread
//!
//! `prebuild::run` is blocking: it decompresses ~1.9 GB and then drives `rustc`
//! once per plugin. On the main thread the window would freeze for the whole
//! duration and Windows would grey it out as "not responding" — the exact
//! impression the bar exists to avoid. It runs on a plain `std::thread` and
//! publishes progress through a mutex the render loop samples each frame.
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::window::{WindowPlugin, WindowResolution};
// Reached through `renzora_runtime` rather than as a direct dependency: this
// binary already depends on the runtime, and the re-export is what the editor
// executable used to provide before it became a loadable image.
use renzora_runtime::renzora_native_plugin::prebuild::{self, Progress};

/// How many log lines the window shows. The pane is fixed-height and the lines
/// are a fixed set of `Text` nodes rewritten in place — a log that spawned a
/// node per line would churn the UI for the whole of a long build.
const LOG_LINES: usize = 14;

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

/// The "Start Editor" button, hidden until the worker finishes.
#[derive(Component)]
struct StartBtn;

/// The progress bar's track, hidden once the worker finishes — a full bar under
/// a summary reads as "still going", and the button is the live thing then.
#[derive(Component)]
struct BarTrack;

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

/// Run setup with a window, returning once the user starts the editor.
///
/// Not once the *work* is complete: the window ends on a **Start Editor**
/// button, so a build failure stays readable instead of flashing past on the
/// frame before the restart. Closing the window is the same as pressing it.
///
/// The caller restarts afterwards; this function does not, so that the decision
/// stays in `main` where the rest of the boot sequence is visible.
pub fn run() {
    let shared = Arc::new(Mutex::new(Shared::default()));

    let worker = shared.clone();
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
                Progress::Building { name, index, total } => {
                    s.push_log(format!("Compiling {name}  ({index}/{total})"));
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
        s.finished = true;
    });

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                // Not "first run": this window is also how an update and an
                // edited plugin get rebuilt, and both are ordinary launches.
                title: format!("{} — setup", product_name()),
                resolution: WindowResolution::new(760, 470),
                resizable: false,
                // Centred and alone: this is a modal moment, not a workspace.
                position: WindowPosition::Centered(MonitorSelection::Primary),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.09, 0.09, 0.11)))
        .insert_resource(Work(shared))
        .add_systems(Startup, spawn_ui)
        .add_systems(Update, tick)
        .run();
}

const TEXT: Color = Color::srgb(0.92, 0.92, 0.95);
const MUTED: Color = Color::srgb(0.62, 0.62, 0.68);
const ACCENT: Color = Color::srgb(0.35, 0.62, 0.95);

fn spawn_ui(mut commands: Commands) {
    commands.spawn(Camera2d);

    let root = commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(12),
                padding: UiRect::all(px(22)),
                ..default()
            },
            Name::new("SetupRoot"),
        ))
        .id();

    let title = commands
        .spawn((
            Text::new(format!("Setting up {}", product_name())),
            TextFont::from_font_size(20.0),
            TextColor(TEXT),
        ))
        .id();

    let status = commands
        .spawn((
            Text::new("Preparing…"),
            TextFont::from_font_size(13.0),
            TextColor(MUTED),
            StatusText,
            // One line, always. Compiler output is arbitrarily long and
            // arbitrarily wide; wrapping it would grow this caption to several
            // lines and shove everything below it down the window as the text
            // changed.
            bevy::text::TextLayout::no_wrap(),
            Node { width: px(680), overflow: Overflow::clip(), ..default() },
        ))
        .id();

    // The track. The fill is a child so its width can be a percentage of it
    // rather than of the window.
    let fill = commands
        .spawn((
            Node {
                width: percent(0),
                height: percent(100),
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            BackgroundColor(ACCENT),
            BarFill,
        ))
        .id();
    let track = commands
        .spawn((
            Node {
                width: px(680),
                height: px(8),
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.18, 0.18, 0.22)),
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
                width: px(680),
                height: px(LOG_LINES as f32 * 15.0 + 16.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(8)),
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.06, 0.06, 0.08)),
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
                    Node { height: px(15.0), overflow: Overflow::clip(), ..default() },
                ))
                .id()
        })
        .collect();
    commands.entity(log).add_children(&lines);

    // Takes the bar's place when the work is done. Setup used to close itself
    // the instant the worker finished, which meant the one thing worth reading
    // — a plugin that failed to compile — was on screen for a single frame.
    // Ending on a button holds the result until it has been seen, and makes
    // starting the editor something that was chosen rather than something that
    // happened.
    let start_label = commands
        .spawn((
            Text::new("Start Editor"),
            TextFont::from_font_size(14.0),
            TextColor(Color::WHITE),
        ))
        .id();
    let start = commands
        .spawn((
            Button,
            Node {
                display: Display::None,
                padding: UiRect::axes(px(22), px(9)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(ACCENT),
            StartBtn,
        ))
        .id();
    commands.entity(start).add_child(start_label);

    commands
        .entity(root)
        .add_children(&[title, status, track, log, start]);
}

fn tick(
    work: Res<Work>,
    mut fill: Query<&mut Node, (With<BarFill>, Without<BarTrack>, Without<StartBtn>)>,
    mut track: Query<&mut Node, (With<BarTrack>, Without<StartBtn>)>,
    mut start: Query<(&mut Node, &Interaction), With<StartBtn>>,
    mut status: Query<&mut Text, (With<StatusText>, Without<LogLine>)>,
    mut log: Query<(&LogLine, &mut Text), Without<StatusText>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut writer: MessageWriter<AppExit>,
) {
    let (latest, finished, lines, summary) = {
        let s = work.0.lock().expect("setup progress lock");
        // Only the tail is drawn; copying it under the lock keeps the lock held
        // for as long as it takes to clone ~14 short strings.
        let start = s.log.len().saturating_sub(LOG_LINES);
        let lines: Vec<String> = s.log.iter().skip(start).cloned().collect();
        let summary = s.finished.then(|| summarize(&s.prepared, &s.failures));
        (s.latest.clone(), s.finished, lines, summary)
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
        if let Ok((mut node, interaction)) = start.single_mut() {
            node.display = Display::Flex;
            // Enter as well as a click: this is the only control in the window,
            // so the keyboard should reach it without a pointer.
            if *interaction == Interaction::Pressed
                || keys.just_pressed(KeyCode::Enter)
                || keys.just_pressed(KeyCode::NumpadEnter)
            {
                writer.write(AppExit::Success);
            }
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
            Progress::Building { index, total, .. } => {
                Some(*index as f32 / (*total).max(1) as f32)
            }
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
