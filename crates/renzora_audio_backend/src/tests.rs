//! Graph-level tests: what comes out of [`Engine::render`] for a given board.
//!
//! Per-module tests live beside their code; these are the ones that need a whole
//! engine — routing, solo, bus removal, voice lifetime — because that is where
//! the behaviour the mixer panel promises actually gets decided.

use alloc::sync::Arc;
use alloc::vec;

use crate::graph::{Engine, PlayParams, VoiceId};
use crate::pcm::{Pcm, PcmRef};
use crate::spatial::Emitter;

const RATE: u32 = 48_000;

/// A clip of `frames` frames at full scale on both channels, at the device rate
/// — so one output frame consumes exactly one source frame and the arithmetic in
/// a test is the arithmetic in the mixer.
fn dc(frames: usize) -> PcmRef {
    Arc::new(Pcm::stereo(vec![1.0; frames * 2], RATE))
}

fn approx(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-4, "{a} != {b}");
}

/// A centre-panned voice at unity through a unity board.
///
/// Constant-power pan is applied once, at the voice, so full scale arrives at
/// 0.707. Buses and master use the *balance* law, which is unity at centre — so
/// this level must not depend on how many buses the signal passed through.
/// Pinning it here means a later change to either pan law can't quietly move
/// every level in the engine.
fn unity_centre(v: f32) -> f32 {
    v * core::f32::consts::FRAC_1_SQRT_2
}

#[test]
fn a_voice_reaches_the_output_through_its_bus() {
    let mut e = Engine::new(RATE);
    e.add_bus("Sfx");
    e.play(
        VoiceId(1),
        dc(64),
        &PlayParams {
            bus: "Sfx".into(),
            ..Default::default()
        },
    );

    let mut out = vec![0.0; 16];
    e.render(&mut out);
    approx(out[0], unity_centre(1.0));
    approx(out[1], unity_centre(1.0));
}

/// An emitter routed to a key nothing answers to must be audible and wrong,
/// not silent and invisible.
#[test]
fn an_unknown_bus_key_falls_back_to_master_rather_than_dropping_the_sound() {
    let mut e = Engine::new(RATE);
    e.play(
        VoiceId(1),
        dc(64),
        &PlayParams {
            bus: "Nonexistent".into(),
            ..Default::default()
        },
    );

    let mut out = vec![0.0; 16];
    e.render(&mut out);
    assert!(out[0] > 0.0);
}

#[test]
fn muting_a_bus_silences_it_and_its_meter() {
    let mut e = Engine::new(RATE);
    e.add_bus("Sfx");
    e.play(
        VoiceId(1),
        dc(64),
        &PlayParams {
            bus: "Sfx".into(),
            ..Default::default()
        },
    );
    e.bus_mut("Sfx").unwrap().muted = true;

    let mut out = vec![0.0; 16];
    e.render(&mut out);
    approx(out[0], 0.0);
    approx(e.bus_mut("Sfx").unwrap().peak, 0.0);
}

/// Solo is a property of the whole board — a bus can't know it's the soloed one.
#[test]
fn soloing_one_bus_silences_every_other_bus() {
    let mut e = Engine::new(RATE);
    e.add_bus("Sfx");
    e.add_bus("Music");
    e.play(
        VoiceId(1),
        dc(64),
        &PlayParams {
            bus: "Sfx".into(),
            ..Default::default()
        },
    );
    e.play(
        VoiceId(2),
        dc(64),
        &PlayParams {
            bus: "Music".into(),
            ..Default::default()
        },
    );
    e.bus_mut("Music").unwrap().soloed = true;

    let mut out = vec![0.0; 16];
    e.render(&mut out);
    approx(e.bus_mut("Sfx").unwrap().peak, 0.0);
    assert!(e.bus_mut("Music").unwrap().peak > 0.0);
}

/// A signal through Sfx→Master must arrive at the same level as one straight to
/// Master. It didn't, before buses got their own pan law: every extra stage cost
/// 3 dB, so a plain unity board came out at half amplitude.
#[test]
fn routing_through_a_bus_does_not_attenuate_a_centred_signal() {
    let level = |bus: &str| {
        let mut e = Engine::new(RATE);
        e.add_bus("Sfx");
        e.play(
            VoiceId(1),
            dc(64),
            &PlayParams {
                bus: bus.into(),
                ..Default::default()
            },
        );
        let mut out = vec![0.0; 16];
        e.render(&mut out);
        out[0]
    };
    approx(level("Sfx"), level("Master"));
    approx(level("Sfx"), unity_centre(1.0));
}

/// Master's gain has to apply to the summed mix, not to a bus in isolation.
#[test]
fn master_gain_scales_the_whole_mix() {
    let mut e = Engine::new(RATE);
    e.add_bus("Sfx");
    e.play(
        VoiceId(1),
        dc(64),
        &PlayParams {
            bus: "Sfx".into(),
            ..Default::default()
        },
    );
    e.bus_mut("Master").unwrap().gain = 0.5;

    let mut out = vec![0.0; 16];
    e.render(&mut out);
    approx(out[0], unity_centre(1.0) * 0.5);
}

#[test]
fn a_voice_is_dropped_once_it_runs_out_of_source() {
    let mut e = Engine::new(RATE);
    e.play(VoiceId(1), dc(4), &PlayParams::default());
    assert_eq!(e.voice_count(), 1);

    let mut out = vec![0.0; 16];
    e.render(&mut out);
    assert_eq!(e.voice_count(), 0);
    // Four frames of signal, then silence — not four frames then garbage.
    approx(out[8], 0.0);
}

#[test]
fn a_looping_voice_outlives_its_source_length() {
    let mut e = Engine::new(RATE);
    e.play(
        VoiceId(1),
        dc(4),
        &PlayParams {
            looping: Some((0.0, 4.0 / RATE as f64)),
            ..Default::default()
        },
    );

    let mut out = vec![0.0; 64];
    e.render(&mut out);
    assert_eq!(e.voice_count(), 1);
    assert!(out[62] != 0.0, "loop should still be producing signal");
}

/// A zero-or-negative loop region would spin the playhead without advancing it.
#[test]
fn a_degenerate_loop_region_falls_back_to_the_whole_clip() {
    let mut e = Engine::new(RATE);
    e.play(
        VoiceId(1),
        dc(8),
        &PlayParams {
            looping: Some((0.5, 0.1)),
            ..Default::default()
        },
    );

    let mut out = vec![0.0; 64];
    e.render(&mut out);
    assert_eq!(e.voice_count(), 1);
    assert!(out[62] != 0.0);
}

/// Cutting from full amplitude to zero is a step, and a step is broadband
/// noise. Stopping has to ramp even when asked for a zero fade.
#[test]
fn stopping_a_voice_ramps_rather_than_cutting() {
    let mut e = Engine::new(RATE);
    let id = VoiceId(1);
    e.play(id, dc(4096), &PlayParams::default());
    e.stop(id, 0.0);

    let mut out = vec![0.0; 16];
    e.render(&mut out);
    assert!(out[0].abs() > 0.0, "the first frame should still have signal");
}

#[test]
fn a_fade_in_starts_from_silence_and_climbs() {
    let mut e = Engine::new(RATE);
    e.play(
        VoiceId(1),
        dc(4096),
        &PlayParams {
            fade_in: 0.01,
            ..Default::default()
        },
    );

    let mut out = vec![0.0; 64];
    e.render(&mut out);
    approx(out[0], 0.0);
    assert!(out[62] > out[2], "fade should be climbing");
}

/// Removing a bus is an authoring action; silencing live sound as a side effect
/// is the more surprising outcome, so voices reseat onto master.
#[test]
fn removing_a_bus_reseats_its_voices_onto_master() {
    let mut e = Engine::new(RATE);
    e.add_bus("Sfx");
    e.play(
        VoiceId(1),
        dc(4096),
        &PlayParams {
            bus: "Sfx".into(),
            ..Default::default()
        },
    );
    assert!(e.remove_bus("Sfx"));

    let mut out = vec![0.0; 16];
    e.render(&mut out);
    assert!(out[0] > 0.0);
    approx(out[0], unity_centre(1.0));
}

/// Indices above a removed bus shift down; a voice pointing at one of them must
/// follow rather than addressing whatever moved into its slot.
#[test]
fn removing_a_bus_reindexes_the_buses_above_it() {
    let mut e = Engine::new(RATE);
    e.add_bus("Sfx");
    e.add_bus("Music");
    e.play(
        VoiceId(1),
        dc(4096),
        &PlayParams {
            bus: "Music".into(),
            ..Default::default()
        },
    );
    e.remove_bus("Sfx");
    e.bus_mut("Music").unwrap().muted = true;

    let mut out = vec![0.0; 16];
    e.render(&mut out);
    approx(out[0], 0.0);
}

#[test]
fn master_can_never_be_removed() {
    let mut e = Engine::new(RATE);
    assert!(!e.remove_bus("Master"));
    assert_eq!(e.buses().len(), 1);
}

/// The host re-sends its whole bus graph whenever the mixer changes; a
/// non-idempotent add would leak a bus per fader twitch.
#[test]
fn adding_an_existing_bus_returns_the_same_index() {
    let mut e = Engine::new(RATE);
    let a = e.add_bus("Sfx");
    let b = e.add_bus("Sfx");
    assert_eq!(a, b);
    assert_eq!(e.buses().len(), 2);
}

#[test]
fn a_distant_spatial_voice_is_quieter_than_a_near_one() {
    let level = |x: f32| {
        let mut e = Engine::new(RATE);
        e.play(
            VoiceId(1),
            dc(64),
            &PlayParams {
                emitter: Some(Emitter {
                    position: [x, 0.0, 0.0],
                    min_distance: 1.0,
                    max_distance: 100.0,
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        let mut out = vec![0.0; 16];
        e.render(&mut out);
        out[0].abs().max(out[1].abs())
    };
    assert!(level(50.0) < level(1.0));
}

/// A summed mix exceeds full scale trivially, and integer wrap-around at the
/// device is a far louder artefact than clipping.
#[test]
fn the_output_is_clipped_rather_than_allowed_to_wrap() {
    let mut e = Engine::new(RATE);
    for _ in 0..32 {
        e.play(VoiceId(1), dc(64), &PlayParams::default());
    }
    let mut out = vec![0.0; 16];
    e.render(&mut out);
    for s in &out {
        assert!(*s <= 1.0 && *s >= -1.0, "{s} escaped full scale");
    }
}

/// `render` runs in a device callback, where a panic is an abort.
#[test]
fn an_odd_length_output_buffer_does_not_panic() {
    let mut e = Engine::new(RATE);
    e.play(VoiceId(1), dc(64), &PlayParams::default());
    let mut out = vec![0.0; 15];
    e.render(&mut out);
    let mut empty: [f32; 0] = [];
    e.render(&mut empty);
}

/// Silence between blocks would be a stale scratch buffer leaking through.
#[test]
fn a_block_with_no_voices_renders_silence() {
    let mut e = Engine::new(RATE);
    e.add_bus("Sfx");
    e.play(
        VoiceId(1),
        dc(2),
        &PlayParams {
            bus: "Sfx".into(),
            ..Default::default()
        },
    );
    let mut out = vec![0.0; 16];
    e.render(&mut out);
    e.render(&mut out);
    for s in &out {
        approx(*s, 0.0);
    }
}

// ── Pushed feeds ─────────────────────────────────────────────────────────────

#[test]
fn pushed_frames_mix_into_their_bus() {
    let mut e = Engine::new(RATE);
    e.add_bus("Voice");
    e.push_frames("Voice", &[1.0; 8]);

    let mut out = vec![0.0; 8];
    e.render(&mut out);
    assert!(out[0] > 0.0);
}

/// A feed is consumed exactly once — two blocks playing the same samples is a
/// stutter, not a mix.
#[test]
fn a_feed_is_drained_rather_than_replayed() {
    let mut e = Engine::new(RATE);
    e.add_bus("Voice");
    e.push_frames("Voice", &[1.0; 8]);

    let mut out = vec![0.0; 8];
    e.render(&mut out);
    assert_eq!(e.feed_len("Voice"), 0);
    e.render(&mut out);
    for s in &out {
        approx(*s, 0.0);
    }
}

/// Unlike a clip, a feed is continuous: silently redirecting one to master would
/// be a mix problem that never stops.
#[test]
fn a_feed_to_an_unknown_bus_is_dropped_rather_than_rerouted() {
    let mut e = Engine::new(RATE);
    e.push_frames("Nonexistent", &[1.0; 8]);

    let mut out = vec![0.0; 8];
    e.render(&mut out);
    for s in &out {
        approx(*s, 0.0);
    }
}

/// A feed nobody drains is a leak, and unbounded latency on a live stream is
/// worse than a gap.
#[test]
fn an_undrained_feed_is_bounded_and_keeps_the_newest_samples() {
    let mut e = Engine::new(RATE);
    e.add_bus("Voice");
    for _ in 0..64 {
        e.push_frames("Voice", &[0.5; 4096]);
    }
    let len = e.feed_len("Voice");
    assert!(len <= 48_000 / 2, "feed grew unbounded: {len}");
    assert!(len > 0);
}

/// A feed shorter than the block must not leave the rest of the block reading
/// stale samples.
#[test]
fn a_partial_feed_fills_only_what_it_has() {
    let mut e = Engine::new(RATE);
    e.add_bus("Voice");
    e.push_frames("Voice", &[1.0; 4]);

    let mut out = vec![0.0; 32];
    e.render(&mut out);
    assert!(out[0] > 0.0);
    approx(out[30], 0.0);
}

// ── Effect sends ─────────────────────────────────────────────────────────────

/// Peak output over `frames`, ignoring the first `skip` frames.
///
/// Skipping matters: the source is 16 frames long and full scale, so the dry
/// signal dominates the peak of the whole block and an effect return — which is
/// quieter and arrives later — is invisible in it. What a send test wants to
/// know is whether anything is there *after* the voice has gone.
fn tail_peak_with_sends(reverb_send: f32, delay_send: f32, skip: usize, frames: usize) -> f32 {
    let mut e = Engine::new(RATE);
    e.play(
        VoiceId(1),
        dc(16),
        &PlayParams {
            reverb_send,
            delay_send,
            ..Default::default()
        },
    );
    let mut out = vec![0.0; frames * 2];
    e.render(&mut out);
    out[skip * 2..]
        .iter()
        .fold(0.0f32, |a, s| a.max(s.abs()))
}

/// A send at zero must be bit-identical to no effects at all — otherwise every
/// existing project's mix moves the day this ships.
#[test]
fn a_voice_with_no_sends_is_unaffected_by_the_effects() {
    let mut e = Engine::new(RATE);
    e.play(VoiceId(1), dc(64), &PlayParams::default());
    let mut out = vec![0.0; 32];
    e.render(&mut out);
    approx(out[0], unity_centre(1.0));
    approx(out[30], unity_centre(1.0));
}

#[test]
fn a_reverb_send_adds_signal_that_outlasts_the_voice() {
    let dry = tail_peak_with_sends(0.0, 0.0, 100, 8_000);
    let wet = tail_peak_with_sends(1.0, 0.0, 100, 8_000);
    approx(dry, 0.0);
    assert!(wet > dry, "reverb send added nothing: {dry} -> {wet}");
}

/// The window has to outlast the delay time — the default is 375 ms, which is
/// 18,000 frames at 48 kHz, so a shorter block would show silence and prove
/// nothing.
#[test]
fn a_delay_send_adds_signal_that_outlasts_the_voice() {
    let dry = tail_peak_with_sends(0.0, 0.0, 100, 24_000);
    let wet = tail_peak_with_sends(0.0, 1.0, 100, 24_000);
    approx(dry, 0.0);
    assert!(wet > dry, "delay send added nothing: {dry} -> {wet}");
}

/// A tail outlives its input by definition. Skipping the effect call on a silent
/// block would chop it off the instant the last voice stopped.
#[test]
fn a_reverb_tail_keeps_sounding_after_every_voice_has_finished() {
    let mut e = Engine::new(RATE);
    e.play(
        VoiceId(1),
        dc(16),
        &PlayParams {
            reverb_send: 1.0,
            ..Default::default()
        },
    );
    let mut out = vec![0.0; 512];
    e.render(&mut out);
    assert_eq!(e.voice_count(), 0, "the voice should have finished");

    let mut tail = 0.0f32;
    for _ in 0..64 {
        e.render(&mut out);
        tail = tail.max(out.iter().fold(0.0f32, |a, s| a.max(s.abs())));
    }
    assert!(tail > 0.0, "the tail was cut off with the voice");
}

/// Sends are post-fader and post-spatial, so a distant sound sends less reverb
/// than a near one — which is what makes distance read at all.
#[test]
fn a_distant_voice_sends_less_reverb_than_a_near_one() {
    let level = |x: f32| {
        let mut e = Engine::new(RATE);
        e.play(
            VoiceId(1),
            dc(16),
            &PlayParams {
                reverb_send: 1.0,
                emitter: Some(Emitter {
                    position: [x, 0.0, 0.0],
                    min_distance: 1.0,
                    max_distance: 100.0,
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        let mut out = vec![0.0; 8_000];
        e.render(&mut out);
        out.iter().fold(0.0f32, |a, s| a.max(s.abs()))
    };
    assert!(level(60.0) < level(1.0));
}

/// The whole point of a send: one shared reverb, not one per voice.
#[test]
fn every_voice_shares_one_reverb() {
    let mut e = Engine::new(RATE);
    for i in 1..=8 {
        e.play(
            VoiceId(i),
            dc(16),
            &PlayParams {
                reverb_send: 1.0,
                ..Default::default()
            },
        );
    }
    let mut out = vec![0.0; 4_000];
    e.render(&mut out);
    for s in &out {
        assert!(s.is_finite() && s.abs() <= 1.0, "{s} escaped full scale");
    }
}

// ── Pause and pitch ──────────────────────────────────────────────────────────

/// Silent while paused, and — the part that is easy to get wrong — resuming
/// carries on from where it stopped rather than from where it would have been.
#[test]
fn a_paused_voice_is_silent_and_does_not_advance() {
    let mut e = Engine::new(RATE);
    // A ramp, so the sample value tells us where the playhead is.
    let ramp: Vec<f32> = (0..64).flat_map(|i| [i as f32 / 64.0; 2]).collect();
    e.play(
        VoiceId(1),
        Arc::new(Pcm::stereo(ramp, RATE)),
        &PlayParams::default(),
    );

    let mut out = vec![0.0; 8];
    e.render(&mut out);
    let before = out[6];
    assert!(before > 0.0);

    e.set_voice_paused(VoiceId(1), true);
    e.render(&mut out);
    for s in &out {
        approx(*s, 0.0);
    }

    e.set_voice_paused(VoiceId(1), false);
    e.render(&mut out);
    // Frame 4 of the source follows frame 3 — it did not skip the paused block.
    assert!(out[0] > before, "resumed behind where it paused");
    assert!(out[0] < before * 3.0, "playhead ran on while paused");
}

#[test]
fn a_paused_voice_is_not_dropped_as_finished() {
    let mut e = Engine::new(RATE);
    e.play(VoiceId(1), dc(4), &PlayParams::default());
    e.set_voice_paused(VoiceId(1), true);

    let mut out = vec![0.0; 64];
    e.render(&mut out);
    e.render(&mut out);
    assert_eq!(e.voice_count(), 1, "a paused voice must survive");
}

/// Recomputed from the source rate rather than scaled from the current value,
/// so repeated changes cannot drift.
#[test]
fn setting_the_same_pitch_twice_is_idempotent() {
    let mut e = Engine::new(RATE);
    let source: Vec<f32> = (0..256).flat_map(|i| [i as f32 / 256.0; 2]).collect();
    e.play(
        VoiceId(1),
        Arc::new(Pcm::stereo(source.clone(), RATE)),
        &PlayParams::default(),
    );
    let mut out = vec![0.0; 16];
    e.render(&mut out);
    let baseline = out[14];

    let mut e2 = Engine::new(RATE);
    e2.play(
        VoiceId(1),
        Arc::new(Pcm::stereo(source, RATE)),
        &PlayParams::default(),
    );
    e2.set_voice_pitch(VoiceId(1), 1.0);
    e2.set_voice_pitch(VoiceId(1), 1.0);
    let mut out2 = vec![0.0; 16];
    e2.render(&mut out2);
    approx(out2[14], baseline);
}

#[test]
fn a_higher_pitch_consumes_the_source_faster() {
    let level_after = |pitch: f64| {
        let mut e = Engine::new(RATE);
        let ramp: Vec<f32> = (0..256).flat_map(|i| [i as f32 / 256.0; 2]).collect();
        e.play(
            VoiceId(1),
            Arc::new(Pcm::stereo(ramp, RATE)),
            &PlayParams::default(),
        );
        e.set_voice_pitch(VoiceId(1), pitch);
        let mut out = vec![0.0; 16];
        e.render(&mut out);
        out[14]
    };
    assert!(level_after(2.0) > level_after(1.0));
}
