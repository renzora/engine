//! The profiler's recorder: when every span starts and ends, on which thread.
//!
//! Bevy wraps each system run in a span (the `trace` feature, on in every
//! build), so a `tracing` layer sees the whole frame without the engine being
//! instrumented by hand. [`ProfileLayer`] is that layer. It is installed with
//! the log subscriber at boot and does nothing but one atomic load until
//! [`set_recording`] turns it on; then each span enter and exit becomes one
//! [`SpanEvent`] pushed into a buffer owned by the thread it happened on.
//! `renzora_profiler` [`drain`]s the buffers once a frame and turns the events
//! into spans.
//!
//! # Why here
//!
//! The layer is installed by the executable (`LogPlugin` is built before any
//! other plugin), and the recording switch and buffers are read by the
//! profiler plugin and by whatever turns recording on. Statics in the shared
//! contract image are one per process; anywhere else they could be duplicated
//! into a second image and the reader would watch an empty buffer, silently.
//!
//! # Cost
//!
//! A span is named once, when it is created (Bevy creates a system's span when
//! the system is set up, not per run), so the per-run path is: check the
//! switch, look the span's name up in [`NameTable`], take a timestamp, push.
//! About 45 ns an enter or exit while recording, most of it the timestamp
//! (`Instant::now`, about 30 ns on Windows), and 7 ns while not. The editor
//! makes about 11,000 a frame, so recording costs it about 0.5 ms of CPU a
//! frame spread over its threads. Measure with
//! `cargo run --profile dist -p renzora_profiler --example record_cost`, on an
//! otherwise idle machine: a running editor beside it doubles the numbers.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use bevy::log::tracing::field::{Field, Visit};
use bevy::log::tracing::span::{Attributes, Id};
use bevy::log::tracing::Subscriber;
use bevy::log::tracing_subscriber::layer::{Context, Layer};
use bevy::log::tracing_subscriber::registry::LookupSpan;
// `bevy::platform::time::Instant`, never `std`'s: std's panics on wasm.
use bevy::platform::time::Instant;

static RECORDING: AtomicBool = AtomicBool::new(false);
static DROPPED: AtomicU64 = AtomicU64::new(0);

/// Events a thread may hold between drains. A recorder left on with nothing
/// draining it must not grow without bound; past this, events are counted in
/// [`dropped`] and discarded. A million is many seconds of a busy thread.
const MAX_PENDING: usize = 1 << 20;

/// Start or stop recording. Off by default.
pub fn set_recording(on: bool) {
    RECORDING.store(on, Ordering::Relaxed);
}

/// Whether span events are being recorded.
pub fn is_recording() -> bool {
    RECORDING.load(Ordering::Relaxed)
}

/// Events discarded because a thread's buffer was full, since the process
/// started. Non-zero means nothing drained for a long time.
pub fn dropped() -> u64 {
    DROPPED.load(Ordering::Relaxed)
}

/// Nanoseconds since the recorder's epoch, the clock every [`SpanEvent`] uses.
/// Monotonic, and shared by every thread, so events from different threads
/// line up.
pub fn now_ns() -> u64 {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    EPOCH.get_or_init(Instant::now).elapsed().as_nanos() as u64
}

/// Whether a span started or ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Enter,
    Exit,
}

/// One span starting or ending.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpanEvent {
    /// [`now_ns`] when it happened.
    pub at_ns: u64,
    /// The span's name, from [`intern`]; [`name`] turns it back into text.
    pub name: u32,
    pub edge: Edge,
}

/// Everything one thread recorded since the last [`drain`], in order.
#[derive(Debug)]
pub struct ThreadEvents {
    /// Stable for the thread's life; [`thread_name`] gives its name.
    pub thread: u32,
    pub events: Vec<SpanEvent>,
}

// ── Names ───────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Names {
    ids: HashMap<String, u32>,
    texts: Vec<String>,
}

fn names() -> &'static Mutex<Names> {
    static NAMES: OnceLock<Mutex<Names>> = OnceLock::new();
    NAMES.get_or_init(Default::default)
}

/// The id for `text`, the same one every time. Span names, GPU pass names and
/// counter names share the table, so a stream sends each name once.
pub fn intern(text: &str) -> u32 {
    let mut names = names().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&id) = names.ids.get(text) {
        return id;
    }
    let id = names.texts.len() as u32;
    names.texts.push(text.to_string());
    names.ids.insert(text.to_string(), id);
    id
}

/// The text of an interned name.
pub fn name(id: u32) -> Option<String> {
    let names = names().lock().unwrap_or_else(|e| e.into_inner());
    names.texts.get(id as usize).cloned()
}

/// How many names are interned. With [`names_from`], what a stream uses to
/// send only the names it has not sent yet.
pub fn name_count() -> u32 {
    names().lock().unwrap_or_else(|e| e.into_inner()).texts.len() as u32
}

/// Every name with an id of `first` or more, in id order.
pub fn names_from(first: u32) -> Vec<(u32, String)> {
    let names = names().lock().unwrap_or_else(|e| e.into_inner());
    names
        .texts
        .iter()
        .enumerate()
        .skip(first as usize)
        .map(|(id, text)| (id as u32, text.clone()))
        .collect()
}

// ── Per-thread buffers ──────────────────────────────────────────────────────

struct ThreadBuffer {
    id: u32,
    name: String,
    /// Locked once per event by its own thread and once per frame by the
    /// drain, so it is all but never contended.
    pending: Mutex<Vec<SpanEvent>>,
}

fn threads() -> &'static Mutex<Vec<Arc<ThreadBuffer>>> {
    static THREADS: OnceLock<Mutex<Vec<Arc<ThreadBuffer>>>> = OnceLock::new();
    THREADS.get_or_init(Default::default)
}

thread_local! {
    static LOCAL: RefCell<Option<Arc<ThreadBuffer>>> = const { RefCell::new(None) };
}

fn push(event: SpanEvent) {
    LOCAL.with(|local| {
        let mut local = local.borrow_mut();
        let buffer = local.get_or_insert_with(|| {
            let mut all = threads().lock().unwrap_or_else(|e| e.into_inner());
            let current = std::thread::current();
            let buffer = Arc::new(ThreadBuffer {
                id: all.len() as u32,
                name: current
                    .name()
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{:?}", current.id())),
                pending: Mutex::new(Vec::new()),
            });
            all.push(buffer.clone());
            buffer
        });
        let mut pending = buffer.pending.lock().unwrap_or_else(|e| e.into_inner());
        if pending.len() < MAX_PENDING {
            pending.push(event);
        } else {
            DROPPED.fetch_add(1, Ordering::Relaxed);
        }
    });
}

/// Take every event recorded since the last call, per thread. Threads with
/// nothing new are left out.
pub fn drain() -> Vec<ThreadEvents> {
    let all = threads().lock().unwrap_or_else(|e| e.into_inner()).clone();
    all.iter()
        .filter_map(|buffer| {
            let mut pending = buffer.pending.lock().unwrap_or_else(|e| e.into_inner());
            // Left with room for as many as it just held: a thread records
            // about the same each frame, and an empty Vec would regrow through
            // a dozen reallocations every frame, on the recording thread.
            let room = pending.len();
            let events = std::mem::replace(&mut *pending, Vec::with_capacity(room));
            drop(pending);
            (!events.is_empty()).then_some(ThreadEvents {
                thread: buffer.id,
                events,
            })
        })
        .collect()
}

/// How many threads have recorded anything; ids run from 0 to this.
pub fn thread_count() -> u32 {
    threads().lock().unwrap_or_else(|e| e.into_inner()).len() as u32
}

// ── Who may connect ─────────────────────────────────────────────────────────

/// Present when this process may be profiled from outside: it then announces
/// itself and accepts a companion holding `key`.
///
/// Inserted from the command line (`--profiling-key <key>`, which is how Play
/// and a profiling export get it) or by the editor for itself. Absent in a
/// normal shipped game, which then never opens a port or announces anything.
#[derive(bevy::prelude::Resource, Clone, Debug, PartialEq, Eq)]
pub struct ProfilingAccess {
    /// What a companion must present.
    pub key: String,
    /// Accept connections from the network, not only from this machine. A
    /// game on a phone or headset needs it; the editor and Play do not.
    pub lan: bool,
    /// How the game names itself in the companion's list.
    pub name: String,
}

impl ProfilingAccess {
    /// From `--profiling-key <key>` (and `--profiling-lan`), if given.
    pub fn from_args(name: impl Into<String>) -> Option<Self> {
        let args: Vec<String> = std::env::args().collect();
        let key = args
            .iter()
            .position(|a| a == "--profiling-key")
            .and_then(|i| args.get(i + 1))?
            .clone();
        Some(Self {
            key,
            lan: args.iter().any(|a| a == "--profiling-lan"),
            name: name.into(),
        })
    }
}

/// This machine's pairing key, made the first time it is asked for and kept
/// in `~/.renzora/profiling_key`.
///
/// The editor hands it to its companion and to every game it launches, and
/// bakes it into an export with profiling allowed; so a game only ever talks
/// to the companion of the editor that built it. Deleting the file makes a new
/// key, which is how builds that left your hands stop being able to pair.
#[cfg(not(target_arch = "wasm32"))]
pub fn machine_key() -> String {
    let path = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|home| std::path::PathBuf::from(home).join(".renzora").join("profiling_key"));
    if let Some(existing) = path
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
    {
        return existing;
    }
    let key = format!(
        "rz-{:016x}{:016x}",
        crate::SceneEntityId::new_random().0,
        crate::SceneEntityId::new_random().0
    );
    if let Some(path) = path {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, &key);
    }
    key
}

/// The name of a thread [`drain`] reported, e.g. `Compute Task Pool (2)`.
pub fn thread_name(thread: u32) -> Option<String> {
    let all = threads().lock().unwrap_or_else(|e| e.into_inner());
    all.get(thread as usize).map(|b| b.name.clone())
}

// ── Span id to name ─────────────────────────────────────────────────────────

/// Which name each live span has, by span id, readable without a lock.
///
/// Finding a span's name through the subscriber (`ctx.span(id)`, then its
/// extensions) is a sharded-slab lookup plus a lock, on every enter and every
/// exit. A probe here is a hash and two atomic loads. Measured with
/// `renzora_profiler`'s `record_cost` example on one thread: 70 ns an edge
/// through the subscriber, 45 ns with this.
///
/// Open addressing over a fixed table. A span's slot is written when the span
/// is created and cleared when it closes. Ids are never 0 (`tracing` uses
/// `NonZeroU64`), so 0 marks a slot never used; a cleared slot gets
/// [`GONE`] so a probe walks past it. A span that finds no room within
/// [`PROBES`] slots is simply not in the table, and the layer falls back to
/// the subscriber for it, so a full table is slower, never wrong.
struct NameTable {
    ids: Box<[AtomicU64]>,
    names: Box<[AtomicU32]>,
}

const SLOTS: usize = 1 << 16;
const PROBES: usize = 32;
const EMPTY: u64 = 0;
const GONE: u64 = u64::MAX;
/// Claimed by a writer that has not stored the name yet. Readers walk past it.
const CLAIMED: u64 = u64::MAX - 1;

fn table() -> &'static NameTable {
    static TABLE: OnceLock<NameTable> = OnceLock::new();
    TABLE.get_or_init(|| NameTable {
        ids: (0..SLOTS).map(|_| AtomicU64::new(EMPTY)).collect(),
        names: (0..SLOTS).map(|_| AtomicU32::new(0)).collect(),
    })
}

fn home(id: u64) -> usize {
    // Fibonacci hashing: slab ids are dense and sequential, which clusters
    // badly under a plain mask.
    (id.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> (64 - SLOTS.trailing_zeros())) as usize
}

impl NameTable {
    fn insert(&self, id: u64, name: u32) {
        let start = home(id);
        for i in 0..PROBES {
            let slot = (start + i) & (SLOTS - 1);
            let current = self.ids[slot].load(Ordering::Relaxed);
            if current != EMPTY && current != GONE {
                continue;
            }
            if self.ids[slot]
                .compare_exchange(current, CLAIMED, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                self.names[slot].store(name, Ordering::Relaxed);
                // Release: a reader that sees the id sees the name with it.
                self.ids[slot].store(id, Ordering::Release);
                return;
            }
        }
    }

    fn get(&self, id: u64) -> Option<u32> {
        let start = home(id);
        for i in 0..PROBES {
            let slot = (start + i) & (SLOTS - 1);
            match self.ids[slot].load(Ordering::Acquire) {
                found if found == id => return Some(self.names[slot].load(Ordering::Relaxed)),
                EMPTY => return None,
                _ => {}
            }
        }
        None
    }

    fn remove(&self, id: u64) {
        let start = home(id);
        for i in 0..PROBES {
            let slot = (start + i) & (SLOTS - 1);
            match self.ids[slot].load(Ordering::Relaxed) {
                found if found == id => {
                    self.ids[slot].store(GONE, Ordering::Release);
                    return;
                }
                EMPTY => return,
                _ => {}
            }
        }
    }
}

// ── The layer ───────────────────────────────────────────────────────────────

/// The interned name, stored on the span when it is created. Read back only
/// for a span the [`NameTable`] had no room for.
struct SpanName(u32);

/// Pulls a span's `name` field out, which is where Bevy puts the system name.
#[derive(Default)]
struct NameField(Option<String>);

impl Visit for NameField {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "name" {
            self.0 = Some(value.to_string());
        }
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "name" && self.0.is_none() {
            self.0 = Some(format!("{value:?}").trim_matches('"').to_string());
        }
    }
}

/// What to call a span. Bevy's system spans are all named `system` and carry
/// the system in a field, so the field is the name; the span that applies a
/// system's commands gets a suffix so the two can be told apart.
fn display_name(span: &str, field: Option<String>) -> String {
    match (span, field) {
        ("system", Some(system)) => system,
        ("system_commands", Some(system)) => format!("{system} (commands)"),
        (span, Some(detail)) => format!("{span}: {detail}"),
        (span, None) => span.to_string(),
    }
}

/// The recorder. See the module docs.
pub struct ProfileLayer;

impl<S> Layer<S> for ProfileLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut field = NameField::default();
        attrs.record(&mut field);
        let name = intern(&display_name(attrs.metadata().name(), field.0));
        table().insert(id.into_u64(), name);
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(SpanName(name));
        }
    }

    fn on_close(&self, id: Id, _ctx: Context<'_, S>) {
        // The subscriber reuses the id for a later span, which must not be
        // recorded under this one's name.
        table().remove(id.into_u64());
    }

    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        if is_recording() {
            record(id, &ctx, Edge::Enter);
        }
    }

    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        if is_recording() {
            record(id, &ctx, Edge::Exit);
        }
    }
}

fn record<S>(id: &Id, ctx: &Context<'_, S>, edge: Edge)
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let name = match table().get(id.into_u64()) {
        Some(name) => name,
        None => {
            let Some(span) = ctx.span(id) else { return };
            let Some(name) = span.extensions().get::<SpanName>().map(|n| n.0) else {
                return;
            };
            name
        }
    };
    push(SpanEvent {
        at_ns: now_ns(),
        name,
        edge,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::log::tracing;
    use bevy::log::tracing_subscriber::prelude::*;
    use bevy::log::tracing_subscriber::Registry;

    /// The recorder's state is process-wide, so tests that switch it take
    /// turns.
    static SERIAL: Mutex<()> = Mutex::new(());

    fn events_for_this_thread(drained: Vec<ThreadEvents>) -> Vec<SpanEvent> {
        let me = LOCAL.with(|l| l.borrow().as_ref().map(|b| b.id));
        drained
            .into_iter()
            .filter(|t| Some(t.thread) == me)
            .flat_map(|t| t.events)
            .collect()
    }

    #[test]
    fn nested_spans_are_recorded_in_order_with_their_names() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let subscriber = Registry::default().with(ProfileLayer);
        tracing::subscriber::with_default(subscriber, || {
            set_recording(true);
            drain();
            let outer = tracing::info_span!("system", name = "physics::step");
            let inner = tracing::info_span!("narrow_phase");
            {
                let _o = outer.enter();
                let _i = inner.enter();
            }
            set_recording(false);
        });
        let events = events_for_this_thread(drain());
        let shape: Vec<(String, Edge)> = events
            .iter()
            .map(|e| (name(e.name).unwrap(), e.edge))
            .collect();
        assert_eq!(
            shape,
            vec![
                ("physics::step".to_string(), Edge::Enter),
                ("narrow_phase".to_string(), Edge::Enter),
                ("narrow_phase".to_string(), Edge::Exit),
                ("physics::step".to_string(), Edge::Exit),
            ]
        );
        assert!(events.windows(2).all(|w| w[0].at_ns <= w[1].at_ns));
    }

    /// The subscriber hands a closed span's id to the next span. The name
    /// table must then give the new span's name, not the closed one's.
    #[test]
    fn a_reused_span_id_records_the_new_name() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let subscriber = Registry::default().with(ProfileLayer);
        tracing::subscriber::with_default(subscriber, || {
            set_recording(true);
            drain();
            for round in 0..50 {
                let name = format!("transient_{round}");
                let span = tracing::info_span!("system", name = name.as_str());
                let _e = span.enter();
            }
            set_recording(false);
            let names: Vec<String> = events_for_this_thread(drain())
                .into_iter()
                .filter(|e| e.edge == Edge::Enter)
                .filter_map(|e| name(e.name))
                .collect();
            let expected: Vec<String> = (0..50).map(|r| format!("transient_{r}")).collect();
            assert_eq!(names, expected);
        });
    }

    #[test]
    fn nothing_is_recorded_while_off() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let subscriber = Registry::default().with(ProfileLayer);
        tracing::subscriber::with_default(subscriber, || {
            set_recording(false);
            drain();
            let span = tracing::info_span!("system", name = "idle");
            let _e = span.enter();
        });
        assert!(events_for_this_thread(drain()).is_empty());
    }

    #[test]
    fn system_spans_are_named_for_their_system() {
        assert_eq!(display_name("system", Some("a::b".into())), "a::b");
        assert_eq!(display_name("system_commands", Some("a::b".into())), "a::b (commands)");
        assert_eq!(display_name("check_conditions", Some("x".into())), "check_conditions: x");
        assert_eq!(display_name("multithreaded executor", None), "multithreaded executor");
    }

    #[test]
    fn a_name_is_interned_once() {
        let a = intern("tests::same_name");
        assert_eq!(intern("tests::same_name"), a);
        assert_eq!(name(a).as_deref(), Some("tests::same_name"));
        assert!(names_from(a).iter().any(|(id, t)| *id == a && t == "tests::same_name"));
    }
}
