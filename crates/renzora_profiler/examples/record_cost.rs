//! What recording costs, piece by piece, at the editor's real load: about
//! 11,000 span edges a frame over 2,000 systems on several threads.
//!
//! `cargo run --profile dist -p renzora_profiler --example record_cost`
//!
//! Measures the three things recording adds: the layer on every span enter and
//! exit (spread over the worker threads), and the main thread's once-a-frame
//! work of pairing the events into spans and encoding the frame for the
//! companion.

use std::time::Instant;

use bevy::log::tracing;
use bevy::log::tracing_subscriber::prelude::*;
use bevy::log::tracing_subscriber::Registry;
use renzora::profiling;
use renzora_profiler::{wire::GamePacket, ProfileFrame, SpanBuilder};

const SYSTEMS: usize = 2000;
const THREADS: usize = 8;
const EDGES_PER_FRAME: usize = 11_000;
const FRAMES: usize = 200;

fn main() {
    let subscriber = Registry::default().with(profiling::ProfileLayer);
    tracing::subscriber::set_global_default(subscriber).expect("no subscriber yet");

    // Spans made once, as Bevy makes a system's span at setup.
    let spans: Vec<tracing::Span> = (0..SYSTEMS)
        .map(|i| tracing::info_span!("system", name = format!("crate::module::system_{i}").as_str()))
        .collect();
    let per_thread = EDGES_PER_FRAME / 2 / THREADS;

    let run = |recording: bool| -> f64 {
        profiling::set_recording(recording);
        profiling::drain();
        let start = Instant::now();
        std::thread::scope(|s| {
            for t in 0..THREADS {
                let spans = &spans;
                s.spawn(move || {
                    for f in 0..FRAMES {
                        for i in 0..per_thread {
                            let _e = spans[(t * per_thread + i + f) % SYSTEMS].enter();
                        }
                    }
                });
            }
        });
        let elapsed = start.elapsed().as_secs_f64();
        profiling::set_recording(false);
        // Per frame, summed over every thread: what the workers spend in total.
        elapsed * THREADS as f64 / FRAMES as f64 * 1e3
    };
    // One thread alone, to tell per-edge work from threads getting in each
    // other's way.
    {
        let n = 200_000;
        let time = |recording: bool| {
            profiling::set_recording(recording);
            profiling::drain();
            let t0 = Instant::now();
            for i in 0..n {
                let _e = spans[i % SYSTEMS].enter();
            }
            profiling::set_recording(false);
            t0.elapsed().as_nanos() as f64 / (n * 2) as f64
        };
        let (off, on) = (time(false), time(true));
        println!("one thread: off {off:.0} ns, on {on:.0} ns per edge");
    }
    let off = run(false);
    let on = run(true);
    println!("layer, all threads, per frame: off {off:.3} ms, on {on:.3} ms (+{:.3} ms of CPU)", on - off);
    println!(
        "  = {:.0} ns per edge while recording",
        (on - off) * 1e6 / EDGES_PER_FRAME as f64
    );

    // The parts of one edge, alone, so the layer's total can be split.
    const N: u32 = 2_000_000;
    let t0 = Instant::now();
    for _ in 0..N {
        std::hint::black_box(profiling::now_ns());
    }
    println!("  of which, the clock:      {:.0} ns", t0.elapsed().as_nanos() as f64 / N as f64);
    let m = std::sync::Mutex::new(Vec::with_capacity(N as usize));
    let t0 = Instant::now();
    for i in 0..N {
        m.lock().unwrap().push(std::hint::black_box(i));
    }
    println!("  of which, lock and push:  {:.0} ns", t0.elapsed().as_nanos() as f64 / N as f64);

    // One frame's events, as the drain hands them over.
    profiling::set_recording(true);
    profiling::drain();
    std::thread::scope(|s| {
        for t in 0..THREADS {
            let spans = &spans;
            s.spawn(move || {
                for i in 0..per_thread {
                    let _e = spans[(t * per_thread + i) % SYSTEMS].enter();
                }
            });
        }
    });
    profiling::set_recording(false);

    let t0 = Instant::now();
    let drained = profiling::drain();
    let t_drain = t0.elapsed();

    let t0 = Instant::now();
    let mut builder = SpanBuilder::default();
    let mut out = Vec::new();
    for thread in &drained {
        builder.feed(thread.thread, &thread.events, &mut out);
    }
    let t_feed = t0.elapsed();
    let frame = ProfileFrame { index: 0, start_ns: 0, end_ns: 1, spans: out, gpu: Vec::new(), counters: Vec::new() };

    let t0 = Instant::now();
    let bytes = GamePacket::Frame(frame.clone()).encode();
    let t_encode = t0.elapsed();

    println!("main thread, per frame ({} spans):", frame.spans.len());
    println!("  drain  {:>8.3} ms", t_drain.as_secs_f64() * 1e3);
    println!("  pair   {:>8.3} ms", t_feed.as_secs_f64() * 1e3);
    println!("  encode {:>8.3} ms ({} KB on the wire)", t_encode.as_secs_f64() * 1e3, bytes.len() / 1024);
    println!(
        "memory: {} B a span, {:.1} MB for the editor's {} frames of history",
        std::mem::size_of::<renzora_profiler::Span>(),
        (std::mem::size_of::<renzora_profiler::Span>() * frame.spans.len() * renzora_profiler::HISTORY) as f64 / 1e6,
        renzora_profiler::HISTORY
    );
}
