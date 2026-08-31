//! The first-run setup window: a progress bar, shown before the editor exists.
//!
//! A downloaded release has to unpack the SDK and compile its native plugins
//! before the editor can load them, and that takes a few seconds. Doing it
//! silently would look like a hang — the window simply would not appear — so
//! this puts up a small window that says what is happening.
//!
//! # Why this is its own Bevy `App`
//!
//! The work has to finish before [`renzora_native_plugin::NativePluginLoader`]
//! runs, and that runs during the *editor's* `App` assembly. So there is no
//! editor `App` to draw into yet, and the splash — which is the natural place
//! for "getting ready" — comes later still.
//!
//! A second, tiny `App` is the way out: bare `DefaultPlugins`, one window, two
//! nodes. It runs, finishes, and the process restarts into the ordinary editor
//! with everything in place. Nothing here uses `renzora_ember`, deliberately —
//! the theme, fonts and stylesheet all live behind engine plugins this app does
//! not have, and pulling them in would make setup depend on most of the thing it
//! exists to set up.
//!
//! # Why the work runs on a thread
//!
//! `prebuild::run` is blocking: it decompresses ~1.9 GB and then drives `rustc`
//! once per plugin. On the main thread the window would freeze for the whole
//! duration and Windows would grey it out as "not responding" — the exact
//! impression the bar exists to avoid. It runs on a plain `std::thread` and
//! publishes progress through a mutex the render loop samples each frame.
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::window::{WindowPlugin, WindowResolution};
use renzora_native_plugin::prebuild::{self, Progress};

/// Shared between the worker thread and the render loop.
#[derive(Default)]
struct Shared {
    latest: Option<Progress>,
    /// Set once the worker is finished, whatever the outcome.
    finished: bool,
    /// Every [`Progress::Failed`] as it arrived, so the finished window can say
    /// what went wrong. The caption only ever holds the *latest* step, and a
    /// failure is usually followed by more steps that scroll it away.
    failures: Vec<String>,
    /// What the worker actually did, for the summary line.
    prepared: prebuild::Prepared,
}

#[derive(Resource, Clone)]
struct Work(Arc<Mutex<Shared>>);

#[derive(Component)]
struct BarFill;

#[derive(Component)]
struct StatusText;

/// The "Start Editor" button, hidden until the worker finishes.
#[derive(Component)]
struct StartBtn;

/// The progress bar, hidden once the worker finishes — a full bar under a
/// summary reads as "still going", and the button below is the live thing then.
#[derive(Component)]
struct BarTrack;

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
            // Also logged: the window shows the current step, but a build failure
            // needs to survive the window closing.
            if let Progress::Failed(e) = &p {
                eprintln!("[setup] {e}");
            }
            let mut s = worker.lock().expect("setup progress lock");
            if let Progress::Failed(e) = &p {
                s.failures.push(e.clone());
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
                title: "Installing plugins".into(),
                resolution: WindowResolution::new(520, 180),
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

fn spawn_ui(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(14),
                ..default()
            },
            children![
                (
                    Text::new("Setting up Renzora"),
                    TextFont::from_font_size(20.0),
                    TextColor(Color::srgb(0.92, 0.92, 0.95)),
                ),
                (
                    Text::new("Preparing…"),
                    TextFont::from_font_size(13.0),
                    TextColor(Color::srgb(0.62, 0.62, 0.68)),
                    StatusText,
                    // One line, always. Compiler output is arbitrarily long and
                    // arbitrarily wide; wrapping it would grow this caption to
                    // several lines and shove the progress bar down the window
                    // as the text changed.
                    bevy::text::TextLayout::no_wrap(),
                    Node { width: px(420), overflow: Overflow::clip(), ..default() },
                ),
                // The track. The fill is a child so its width can be a
                // percentage of it rather than of the window.
                (
                    Node {
                        width: px(400),
                        height: px(8),
                        border_radius: BorderRadius::all(px(4)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.18, 0.18, 0.22)),
                    BarTrack,
                    children![(
                        Node {
                            width: percent(0),
                            height: percent(100),
                            border_radius: BorderRadius::all(px(4)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.35, 0.62, 0.95)),
                        BarFill,
                    )],
                ),
                // Takes the bar's place when the work is done. Setup used to
                // close itself the instant the worker finished, which meant the
                // one thing worth reading — a plugin that failed to compile —
                // was on screen for a single frame. Ending on a button instead
                // holds the result until it has been seen, and makes starting
                // the editor something that was chosen rather than something
                // that happened.
                (
                    Button,
                    Node {
                        display: Display::None,
                        padding: UiRect::axes(px(22), px(9)),
                        border_radius: BorderRadius::all(px(6)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.35, 0.62, 0.95)),
                    StartBtn,
                    children![(
                        Text::new("Start Editor"),
                        TextFont::from_font_size(14.0),
                        TextColor(Color::srgb(1.0, 1.0, 1.0)),
                    )],
                ),
            ],
        ))
        .insert(Name::new("SetupRoot"));
}

fn tick(
    work: Res<Work>,
    mut fill: Query<&mut Node, (With<BarFill>, Without<BarTrack>, Without<StartBtn>)>,
    mut track: Query<&mut Node, (With<BarTrack>, Without<StartBtn>)>,
    mut start: Query<(&mut Node, &Interaction), With<StartBtn>>,
    mut status: Query<&mut Text, With<StatusText>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut writer: MessageWriter<AppExit>,
) {
    let (latest, finished) = {
        let s = work.0.lock().expect("setup progress lock");
        (s.latest.clone(), s.finished)
    };

    if finished {
        // Swap the bar for the button, and the step caption for a summary of
        // what happened. Done every frame rather than once: this system holds no
        // state, and writing the same values again costs a string compare.
        let summary = {
            let s = work.0.lock().expect("setup progress lock");
            summarize(&s.prepared, &s.failures)
        };
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
            **text = p.to_string();
        }
        // Unpacking has a real ratio; compiling does not (rustc reports nothing
        // until it is done), so plugin steps advance by whole plugins.
        //
        // A failure has no fraction, so it leaves the bar where it is — but it
        // must NOT return from this system. It used to, and the `finished` check
        // below is what restarts the editor: one plugin that failed to compile
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
/// rest are counted, because this is one un-wrapped line and the console has
/// them all.
fn summarize(prepared: &prebuild::Prepared, failures: &[String]) -> String {
    if let Some(first) = failures.first() {
        return match failures.len() {
            1 => first.clone(),
            n => format!("{first} (+{} more — see the console)", n - 1),
        };
    }
    match (prepared.unpacked_sdk, prepared.built) {
        (true, 0) => "Rust SDK unpacked. Renzora is ready.".to_string(),
        (true, 1) => "Rust SDK unpacked, 1 plugin built. Renzora is ready.".to_string(),
        (true, n) => format!("Rust SDK unpacked, {n} plugins built. Renzora is ready."),
        (false, 0) => "Nothing to do. Renzora is ready.".to_string(),
        (false, 1) => "1 plugin built. Renzora is ready.".to_string(),
        (false, n) => format!("{n} plugins built. Renzora is ready."),
    }
}
