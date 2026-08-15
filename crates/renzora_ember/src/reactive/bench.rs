//! A measurement harness for the dependency gate.
//!
//! Not a correctness test — [`super::tests`] does that. This exists because
//! "reactivity is faster now" is a claim that should come with a number, and
//! the editor cannot be launched headlessly to produce one. It builds a
//! binding population shaped like the real editor's and reports what the gate
//! costs and saves.
//!
//! Run it with:
//!
//! ```text
//! cargo test -p renzora_ember --lib reactive::bench -- --nocapture --ignored
//! ```
//!
//! `#[ignore]`d so a timing test never fails CI on a noisy machine.

// `bevy::platform::time::Instant`, never `std`'s — std's panics on wasm.
use bevy::platform::time::Instant;

use super::*;
use super::rx::{DepSet, Rx};

#[derive(Resource, Default)]
struct Hot(u32);

#[derive(Resource, Default)]
struct Cold(u32);

/// Roughly what a real `bind_text` closure costs: read a resource, format a
/// string, allocate. The allocation is the point — it is what the gate avoids.
fn work(n: u32, tag: usize) -> String {
    format!("{tag}: value is {n} ({:.2})", n as f32 * 1.5)
}

fn register(world: &mut World, f: impl FnOnce(&mut Commands)) {
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, world);
    f(&mut commands);
    queue.apply(world);
}

fn build_world(count: usize, tracked_bindings: bool) -> World {
    let mut world = World::new();
    world.init_resource::<ReactionRegistry>();
    world.init_resource::<ReactiveStats>();
    world.insert_resource(Hot(0));
    world.insert_resource(Cold(0));

    let targets: Vec<Entity> = (0..count)
        .map(|_| world.spawn(Text::new(String::new())).id())
        .collect();

    for (i, e) in targets.into_iter().enumerate() {
        // One binding in 50 watches the resource that actually changes, which
        // is the ~1-3% churn rate the editor was measured at.
        let hot = i % 50 == 0;
        register(&mut world, |c| {
            if tracked_bindings {
                tracked::bind_text(c, e, move |rx| {
                    let n = if hot {
                        rx.resource::<Hot>().0
                    } else {
                        rx.resource::<Cold>().0
                    };
                    work(n, i)
                });
            } else {
                bind_text(c, e, move |w: &World| {
                    let n = if hot {
                        w.resource::<Hot>().0
                    } else {
                        w.resource::<Cold>().0
                    };
                    work(n, i)
                });
            }
        });
    }
    world
}

/// Steady state: one resource ticks every frame, the rest of the world is
/// still. This is what an idle editor with panels open looks like.
fn measure(count: usize, frames: usize, tracked_bindings: bool) -> (f32, usize) {
    let mut world = build_world(count, tracked_bindings);
    // Warm up: seed dep sets and let the first-run allocations settle.
    for _ in 0..3 {
        run_reactions(&mut world);
    }

    let mut total = 0.0f32;
    let mut skipped_total = 0usize;
    for _ in 0..frames {
        world.increment_change_tick();
        world.resource_mut::<Hot>().0 += 1;
        let t0 = Instant::now();
        run_reactions(&mut world);
        total += t0.elapsed().as_secs_f32() * 1e3;
        skipped_total += world.resource::<ReactiveStats>().skipped_this_frame;
    }
    (total / frames as f32, skipped_total / frames)
}

#[test]
#[ignore = "timing measurement, not a pass/fail assertion"]
fn report_gate_cost_and_saving() {
    const FRAMES: usize = 200;
    println!();
    println!("  bindings |    legacy ms |   tracked ms |  skipped/frame |  saved");
    println!("  ---------+--------------+--------------+----------------+-------");
    for &count in &[100usize, 400, 900, 2000] {
        let (legacy_ms, _) = measure(count, FRAMES, false);
        let (tracked_ms, skipped) = measure(count, FRAMES, true);
        let saved = if legacy_ms > 0.0 {
            100.0 * (legacy_ms - tracked_ms) / legacy_ms
        } else {
            0.0
        };
        println!(
            "  {count:>8} | {legacy_ms:>11.3}  | {tracked_ms:>11.3}  | {skipped:>10} /{count:<4} | {saved:>4.0}%"
        );
    }
    println!();
    println!("  Steady state: 1 binding in 50 watches the resource that changes.");
    println!("  'legacy' is today's behaviour — every binding recomputes, the");
    println!("  PartialEq diff then discards ~98% of the results.");
    println!();
}

/// The shape the first version of this bench failed to model, and which
/// regressed the editor: a `keyed_list` snapshot that reads across every row.
///
/// A `bind_*` closure reads one or two slots; a snapshot reads hundreds, and
/// the dedup was O(n²) in that count. Live, this took keyed-list time from
/// 0.10 ms to 1.33 ms a frame. `DepSet::MAX_DEPS` is what bounds it — this
/// measures that the wide case is back to roughly the cost of the raw reads.
#[test]
#[ignore = "timing measurement, not a pass/fail assertion"]
fn report_wide_reader_cost() {
    #[derive(Component)]
    struct Row(u32);

    const ROWS: usize = 400;
    const ITERS: usize = 400;

    let mut world = World::new();
    let entities: Vec<Entity> = (0..ROWS).map(|i| world.spawn(Row(i as u32)).id()).collect();

    // Baseline: the same reads straight off the world, no tracking at all.
    let t0 = Instant::now();
    let mut sink = 0u64;
    for _ in 0..ITERS {
        for &e in &entities {
            sink += world.get::<Row>(e).map(|r| r.0 as u64).unwrap_or(0);
        }
    }
    let raw_us = t0.elapsed().as_secs_f32() * 1e6 / ITERS as f32;

    // Through an `Rx`, which bails out after `MAX_DEPS` distinct slots.
    let t0 = Instant::now();
    for _ in 0..ITERS {
        let rx = Rx::new(&world);
        for &e in &entities {
            sink += rx.get::<Row>(e).map(|r| r.0 as u64).unwrap_or(0);
        }
        let _ = rx.into_deps();
    }
    let rx_us = t0.elapsed().as_secs_f32() * 1e6 / ITERS as f32;

    println!();
    println!("  Wide reader — {ROWS} distinct component slots per pass:");
    println!("    raw &World reads : {raw_us:.1} µs");
    println!("    through Rx       : {rx_us:.1} µs");
    println!(
        "    tracking overhead: {:+.0}%  (cap = {} slots)",
        100.0 * (rx_us - raw_us) / raw_us,
        DepSet::MAX_DEPS
    );
    println!("  (sink {sink}, keeps the reads from being optimised away)");
    println!();
}

/// The gate's own overhead on the worst case for it: every binding dirty every
/// frame, so the tick check is pure cost and buys nothing. Guards against the
/// gate being a pessimisation for churny panels.
#[test]
#[ignore = "timing measurement, not a pass/fail assertion"]
fn report_worst_case_overhead() {
    const COUNT: usize = 900;
    const FRAMES: usize = 200;

    let mut tracked_world = build_world(COUNT, true);
    let mut legacy_world = build_world(COUNT, false);
    for _ in 0..3 {
        run_reactions(&mut tracked_world);
        run_reactions(&mut legacy_world);
    }

    let run = |world: &mut World| {
        let mut total = 0.0f32;
        for _ in 0..FRAMES {
            // Dirty BOTH resources, so every binding must re-run.
            world.increment_change_tick();
            world.resource_mut::<Hot>().0 += 1;
            world.resource_mut::<Cold>().0 += 1;
            let t0 = Instant::now();
            run_reactions(world);
            total += t0.elapsed().as_secs_f32() * 1e3;
        }
        total / FRAMES as f32
    };

    let legacy_ms = run(&mut legacy_world);
    let tracked_ms = run(&mut tracked_world);
    println!();
    println!("  Worst case — every binding dirty every frame ({COUNT} bindings):");
    println!("    legacy  {legacy_ms:.3} ms/frame");
    println!("    tracked {tracked_ms:.3} ms/frame");
    println!(
        "    gate overhead when it never fires: {:+.1}%",
        100.0 * (tracked_ms - legacy_ms) / legacy_ms
    );
    println!();
}
