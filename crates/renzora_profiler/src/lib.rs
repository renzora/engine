//! Per-frame profiles from the recorder, for the companion's profiler.
//!
//! `renzora::profiling` records when each span starts and ends, per thread.
//! Once a frame, while recording is on, [`collect_frame`] drains those events
//! and turns them into a [`ProfileFrame`]: the spans that finished, each render
//! pass's GPU time and a few counters. The last [`HISTORY`] frames are kept in
//! [`Profiler`]; the companion's stream reads them from there.
//!
//! In every build and cheap while off: one atomic load a frame, and nothing
//! else, until something calls `renzora::profiling::set_recording(true)`.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::ecs::entity::Entities;
use bevy::prelude::*;
use renzora::profiling;

mod frame;
#[cfg(not(target_arch = "wasm32"))]
pub mod server;
pub mod wire;

pub use frame::{Counter, GpuPass, ProfileFrame, Span, SpanBuilder};

/// Frames kept for whoever reads them. About two seconds at 60 fps.
///
/// Short on purpose. Each companion is streamed every frame as it is made and
/// is never sent older ones, so this only has to cover a companion's writer
/// falling briefly behind. At the editor's load a frame is about 5,500 spans
/// (176 KB): six hundred of them were 105 MB of editor memory nothing read.
pub const HISTORY: usize = 120;

/// The recent frames, newest last, and the state that builds them.
#[derive(Resource, Default)]
pub struct Profiler {
    /// Shared, so a frame goes to every companion's writer thread without
    /// being copied.
    frames: VecDeque<Arc<ProfileFrame>>,
    builder: SpanBuilder,
    /// When the frame being recorded began; `None` until the first mark after
    /// recording starts.
    mark_ns: Option<u64>,
    next_index: u64,
}

impl Profiler {
    /// The recorded frames, oldest first.
    pub fn frames(&self) -> impl DoubleEndedIterator<Item = &ProfileFrame> {
        self.frames.iter().map(|f| &**f)
    }

    /// Frames with an index of `index` or more, for a reader catching up.
    pub fn frames_since(&self, index: u64) -> impl Iterator<Item = &ProfileFrame> {
        self.frames().filter(move |f| f.index >= index)
    }

    /// [`Self::frames_since`], as shared handles to hand to another thread.
    /// Newest first in the search, since a reader is almost always one frame
    /// behind: this is called every frame and should not walk the history.
    pub(crate) fn shared_since(&self, index: u64) -> impl Iterator<Item = &Arc<ProfileFrame>> {
        let behind = self.frames.iter().rev().take_while(|f| f.index >= index).count();
        self.frames.iter().skip(self.frames.len() - behind)
    }
}

#[derive(Default)]
pub struct ProfilerPlugin;

impl Plugin for ProfilerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Profiler>();

        let editor = app
            .world()
            .get_resource::<renzora::EditorSession>()
            .is_some_and(|s| s.0);
        // A game started with a key (Play, a profiling export) may be profiled.
        // The editor is given its access by the companion, which owns the key.
        #[cfg(not(target_arch = "wasm32"))]
        if !editor {
            let name = app
                .world()
                .get_resource::<renzora::CurrentProject>()
                .map(|p| p.config.name.clone())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| "Game".to_string());
            // From the command line (Play), or baked in by an export with
            // "Allow profiling" on, which also opens it to the network: a
            // profiling build is usually on a phone or headset.
            let baked = app
                .world()
                .get_resource::<renzora::CurrentProject>()
                .and_then(|p| p.config.profiling_key.clone());
            let access = renzora::profiling::ProfilingAccess::from_args(name.clone()).or_else(|| {
                baked.map(|key| renzora::profiling::ProfilingAccess { key, lan: true, name })
            });
            if let Some(access) = access {
                app.insert_resource(access);
            }
        }
        let profilable = editor || app.world().contains_resource::<renzora::profiling::ProfilingAccess>();

        // GPU pass times come from Bevy's render diagnostics, which time every
        // pass every frame once added and cannot be added later. So they are on
        // where profiling is possible and off in a normal shipped game.
        if profilable && !app.is_plugin_added::<bevy::render::diagnostic::RenderDiagnosticsPlugin>() {
            app.add_plugins(bevy::render::diagnostic::RenderDiagnosticsPlugin);
        }

        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(
            First,
            (collect_frame, server::start_server, server::stream).chain(),
        );
        // Probes' reports, in `Last` so a report sees the frame's finished
        // state, and apart from the frame stream: a companion can watch
        // probes without recording.
        app.init_resource::<renzora::probes::ProbeRegistry>()
            .init_resource::<renzora::probes::WatchedProbes>();
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(Last, server::serve_probes);
        #[cfg(target_arch = "wasm32")]
        app.add_systems(First, collect_frame);
    }
}

/// Close the frame that just ended, if recording.
///
/// In `First`, so the mark sits between one frame's last system and the next
/// frame's first. The render world runs on its own thread and its spans land
/// in whichever frame they finish in, with their real times.
pub fn collect_frame(
    mut profiler: ResMut<Profiler>,
    diagnostics: Option<Res<DiagnosticsStore>>,
    entities: &Entities,
) {
    if !profiling::is_recording() {
        if profiler.mark_ns.is_some() {
            // Stopped: forget half-open spans so the next recording starts clean.
            profiler.mark_ns = None;
            profiler.builder.reset();
            profiling::drain();
        }
        return;
    }
    let now = profiling::now_ns();
    let Some(start_ns) = profiler.mark_ns.replace(now) else {
        // The first mark only starts the clock; events before it belong to a
        // frame that was already half over when recording began.
        profiling::drain();
        return;
    };

    let mut spans = Vec::new();
    for thread in profiling::drain() {
        profiler.builder.feed(thread.thread, &thread.events, &mut spans);
    }
    let gpu = diagnostics.as_deref().map(gpu_passes).unwrap_or_default();
    let mut counters = vec![Counter {
        name: profiling::intern("Entities"),
        value: entities.count_spawned() as f64,
    }];
    if let Some(ms) = diagnostics
        .as_deref()
        .and_then(|d| d.get(&FrameTimeDiagnosticsPlugin::FRAME_TIME))
        .and_then(|d| d.value())
    {
        counters.push(Counter {
            name: profiling::intern("Frame time (ms)"),
            value: ms,
        });
    }

    let index = profiler.next_index;
    profiler.next_index += 1;
    if profiler.frames.len() == HISTORY {
        profiler.frames.pop_front();
    }
    profiler.frames.push_back(Arc::new(ProfileFrame {
        index,
        start_ns,
        end_ns: now,
        spans,
        gpu,
        counters,
    }));
}

/// Every render pass Bevy's render diagnostics timed recently, at every depth.
///
/// Empty unless `RenderDiagnosticsPlugin` is in the app and the GPU supports
/// timestamp queries. A pass that stopped running keeps its last value in the
/// store forever, so anything measured well before the newest pass is left
/// out rather than reported as still costing time.
fn gpu_passes(diagnostics: &DiagnosticsStore) -> Vec<GpuPass> {
    const STALE_AFTER: Duration = Duration::from_millis(500);

    let mut found = Vec::new();
    let mut newest = None;
    for diagnostic in diagnostics.iter() {
        let Some(pass) = diagnostic.path().as_str().strip_suffix("/elapsed_gpu") else {
            continue;
        };
        let Some(measurement) = diagnostic.measurement() else {
            continue;
        };
        let pass = pass.strip_prefix("render/").unwrap_or(pass);
        newest = Some(newest.map_or(measurement.time, |n: bevy::platform::time::Instant| {
            n.max(measurement.time)
        }));
        found.push((pass.to_string(), measurement.value as f32, measurement.time));
    }
    let Some(newest) = newest else {
        return Vec::new();
    };
    found
        .into_iter()
        .filter(|(_, _, time)| newest.saturating_duration_since(*time) < STALE_AFTER)
        .map(|(pass, ms, _)| GpuPass {
            depth: pass.matches('/').count().min(u8::MAX as usize) as u8,
            name: profiling::intern(&pass),
            ms,
        })
        .collect()
}

renzora::add!(ProfilerPlugin);

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// The process-wide recorder is switched by these tests, so they take turns.
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(ProfilerPlugin);
        app
    }

    #[test]
    fn nothing_is_kept_while_not_recording() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        profiling::set_recording(false);
        let mut app = app();
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(app.world().resource::<Profiler>().frames().count(), 0);
    }

    #[test]
    fn each_frame_after_the_first_mark_is_kept_in_order() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let mut app = app();
        profiling::set_recording(true);
        for _ in 0..4 {
            app.update();
        }
        profiling::set_recording(false);
        let profiler = app.world().resource::<Profiler>();
        let frames: Vec<&ProfileFrame> = profiler.frames().collect();
        assert_eq!(frames.len(), 3, "the first update only starts the clock");
        assert!(frames.windows(2).all(|w| w[0].index + 1 == w[1].index && w[0].end_ns == w[1].start_ns));
        let entities = profiling::intern("Entities");
        assert!(frames.iter().all(|f| f.counters.iter().any(|c| c.name == entities)));
    }

    /// The whole chain: Bevy's own system spans (the `trace` feature), through
    /// the recorder layer, out as named spans in a frame.
    #[test]
    fn a_bevy_system_shows_up_as_a_span_named_for_it() {
        use bevy::ecs::schedule::SingleThreadedExecutor;
        use bevy::log::tracing_subscriber::prelude::*;

        fn busy_work() {
            std::hint::black_box((0..1000u64).sum::<u64>());
        }

        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let subscriber = bevy::log::tracing_subscriber::Registry::default()
            .with(profiling::ProfileLayer);
        bevy::log::tracing::subscriber::with_default(subscriber, || {
            let mut app = app();
            // The test's subscriber is this thread's only, so keep every
            // system on this thread.
            app.edit_schedule(Update, |s| {
                s.set_executor(SingleThreadedExecutor::new());
            });
            app.edit_schedule(First, |s| {
                s.set_executor(SingleThreadedExecutor::new());
            });
            app.add_systems(Update, busy_work);
            profiling::set_recording(true);
            for _ in 0..3 {
                app.update();
            }
            profiling::set_recording(false);

            let profiler = app.world().resource::<Profiler>();
            let names: Vec<String> = profiler
                .frames()
                .flat_map(|f| f.spans.iter())
                .filter_map(|s| profiling::name(s.name))
                .collect();
            assert!(
                names.iter().any(|n| n.ends_with("busy_work")),
                "no busy_work span among {names:?}"
            );
            let span = profiler
                .frames()
                .flat_map(|f| f.spans.iter())
                .find(|s| profiling::name(s.name).is_some_and(|n| n.ends_with("busy_work")))
                .unwrap();
            assert!(span.end_ns >= span.start_ns);
        });
    }

    /// The game's server over a real socket: a wrong key is turned away; the
    /// right one gets a hello, and once it asks to record, names, threads and
    /// frames.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_companion_with_the_key_receives_frames() {
        use crate::wire::{ControlPacket, GamePacket};
        use std::net::TcpStream;
        use std::time::Duration;

        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        profiling::set_recording(false);
        let mut app = app();
        app.insert_resource(profiling::ProfilingAccess {
            key: "test-key".into(),
            lan: false,
            name: "probe".into(),
        });
        app.update();
        let port = app.world().resource::<server::Server>().port;
        assert_ne!(port, 0);

        // The wrong key: the server closes on it without a word.
        let mut wrong = TcpStream::connect(("127.0.0.1", port)).unwrap();
        wrong.write_all(&ControlPacket::Hello { version: wire::VERSION, key: "guess".into() }.encode()).unwrap();
        wrong.set_read_timeout(Some(Duration::from_secs(6))).unwrap();
        assert!(!matches!(GamePacket::read(&mut wrong), Ok(Some(_))));

        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        stream
            .write_all(&ControlPacket::Hello { version: wire::VERSION, key: "test-key".into() }.encode())
            .unwrap();
        // The server hands the connection over on its next frame.
        let mut hello = None;
        for _ in 0..50 {
            app.update();
            if hello.is_none() {
                stream.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
                if let Ok(Some(packet)) = GamePacket::read(&mut stream) {
                    hello = Some(packet);
                    break;
                }
            }
        }
        assert!(matches!(hello, Some(GamePacket::Hello { ref name, .. }) if name == "probe"));

        stream.write_all(&ControlPacket::Record(true).encode()).unwrap();
        stream.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
        let (mut names, mut frames) = (false, 0);
        for _ in 0..200 {
            app.update();
            while let Ok(Some(packet)) = GamePacket::read(&mut stream) {
                match packet {
                    GamePacket::Names(_) => names = true,
                    GamePacket::Frame(_) => frames += 1,
                    _ => {}
                }
            }
            if names && frames >= 3 {
                break;
            }
        }
        assert!(profiling::is_recording(), "a recording companion turns the recorder on");
        assert!(names, "names arrive before frames that use them");
        assert!(frames >= 3, "got {frames} frames");

        drop(stream);
        for _ in 0..50 {
            app.update();
            if !profiling::is_recording() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!profiling::is_recording(), "recording stops when the companion leaves");
    }

    #[test]
    fn history_is_bounded() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let mut app = app();
        profiling::set_recording(true);
        for _ in 0..(HISTORY + 20) {
            app.update();
        }
        profiling::set_recording(false);
        let profiler = app.world().resource::<Profiler>();
        assert_eq!(profiler.frames().count(), HISTORY);
        assert_eq!(profiler.frames_since(HISTORY as u64 + 10).count(), 9);
    }
}
