//! The sound card, and the queue that reaches it.
//!
//! [`Engine`] does not live here — it lives *on the audio thread*, moved into
//! cpal's callback and never touched from anywhere else. Everything the host
//! wants to do arrives as a [`Command`] through a lock-free SPSC ring, and
//! everything it wants to know comes back through atomics.
//!
//! ## Why not just wrap the engine in a mutex
//!
//! Because the audio callback has a deadline measured in milliseconds and no
//! way to miss it gracefully. A `Mutex<Engine>` would have the callback wait on
//! a lock held by a game thread that has been descheduled, and the result is not
//! a slow frame — it is an underrun, which is an audible click. Worse, the
//! blocking is unbounded: the OS can preempt the lock holder for as long as it
//! likes. The queue makes the callback's worst case "drain N commands", which is
//! bounded and allocation-free.
//!
//! The same reasoning is why finished voices go *back* through a second ring
//! rather than being dropped in the callback. Dropping a voice releases its
//! `PcmRef`, and the last release calls `free` — an unbounded operation on most
//! allocators. See [`Engine::drain_finished`].

use alloc::string::String;
use alloc::sync::Arc;
use alloc::{format, vec};
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};

use crate::graph::{Engine, PlayParams, Voice, VoiceId};
use crate::pcm::PcmRef;
use crate::spatial::Listener;

/// How many buses publish a meter reading. Buses past this still mix; they just
/// have no VU in the panel, which is a far better failure than a heap-allocated
/// meter array the audio thread has to synchronise on.
pub const MAX_METERED_BUSES: usize = 64;

/// Commands buffered between host frames. At 60 Hz this is ~17 ms of headroom;
/// a game that overruns it is spawning thousands of sounds a frame and has a
/// problem the audio system cannot solve for it.
const COMMAND_CAPACITY: usize = 1024;

/// Finished voices buffered for the host to free.
const GARBAGE_CAPACITY: usize = 512;

/// Something the host asks the mixer to do.
///
/// Every variant is *owned* — `String`, `PcmRef` — so the audio thread never
/// dereferences host memory that might have gone away. The allocations all
/// happen on the host side, building the command; the callback only moves them.
pub enum Command {
    AddBus(String),
    RemoveBus(String),
    /// The whole strip at once. The mixer panel changes several of these
    /// together (a solo click alters every bus's effective gain), and sending
    /// them as one command keeps a block from ever rendering a half-applied
    /// board.
    SetBus {
        key: String,
        gain: f32,
        pan: f32,
        muted: bool,
        soloed: bool,
    },
    Play {
        id: VoiceId,
        source: PcmRef,
        params: PlayParams,
    },
    Stop {
        id: VoiceId,
        fade: f32,
    },
    StopBus {
        key: String,
        fade: f32,
    },
    StopAll {
        fade: f32,
    },
    SetListener(Listener),
    SetVoicePosition {
        id: VoiceId,
        position: [f32; 3],
    },
    SetVoiceGain {
        id: VoiceId,
        gain: f32,
    },
    SetVoiceBus {
        id: VoiceId,
        key: String,
    },
    SetVoicePan {
        id: VoiceId,
        pan: f32,
    },
    SetVoiceEmitter {
        id: VoiceId,
        emitter: crate::spatial::Emitter,
    },
    SetVoicePaused {
        id: VoiceId,
        paused: bool,
    },
    SetVoicePitch {
        id: VoiceId,
        pitch: f64,
    },
    /// Interleaved stereo samples to mix into a bus — a mic being monitored, a
    /// remote player's voice, a synth. Owned like every other variant, so the
    /// audio thread never reads host memory that may have gone away.
    PushFrames {
        bus: String,
        samples: alloc::vec::Vec<f32>,
    },
}

/// Readback from the audio thread, written every block and read whenever the
/// host likes.
///
/// Peaks are `AtomicU32` holding `f32::to_bits`, because there is no
/// `AtomicF32`. `Relaxed` throughout is correct here and not a shortcut: these
/// are independent scalars for a meter, nothing is ordered against them, and a
/// reading one block stale is invisible at 60 Hz.
struct Shared {
    peaks: [AtomicU32; MAX_METERED_BUSES],
    bus_count: AtomicUsize,
    voice_count: AtomicUsize,
    /// Commands the ring had no room for. Surfaced so a host that is overrunning
    /// finds out, rather than wondering why one sound in a hundred is missing.
    dropped_commands: AtomicUsize,
}

impl Shared {
    fn new() -> Self {
        Self {
            peaks: [const { AtomicU32::new(0) }; MAX_METERED_BUSES],
            bus_count: AtomicUsize::new(0),
            voice_count: AtomicUsize::new(0),
            dropped_commands: AtomicUsize::new(0),
        }
    }
}

/// Why the device could not be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceError(pub String);

impl core::fmt::Display for DeviceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The host-side handle: a live output stream plus the queue into it.
///
/// Dropping this stops the stream and takes the mixer with it.
pub struct AudioDevice {
    /// Held only to keep the stream alive — cpal stops it on drop.
    _stream: cpal::Stream,
    commands: rtrb::Producer<Command>,
    /// Finished voices coming back to be freed on this thread. Drained by
    /// [`AudioDevice::collect_garbage`].
    garbage: rtrb::Consumer<Voice>,
    shared: Arc<Shared>,
    sample_rate: u32,
    /// Voice ids are handed out here rather than by the engine, because a caller
    /// needs the id the moment it asks for a sound — long before the audio
    /// thread has seen the request.
    next_voice: u64,
}

impl AudioDevice {
    /// Open the default output device and start mixing.
    pub fn open() -> Result<Self, DeviceError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| DeviceError(String::from("no default output device")))?;
        let supported = device
            .default_output_config()
            .map_err(|e| DeviceError(format!("no usable output config: {e}")))?;
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let sample_rate = config.sample_rate;
        let channels = config.channels as usize;

        let (producer, consumer) = rtrb::RingBuffer::<Command>::new(COMMAND_CAPACITY);
        let (garbage_tx, garbage_rx) = rtrb::RingBuffer::<Voice>::new(GARBAGE_CAPACITY);
        let shared = Arc::new(Shared::new());

        let stream = build_stream(
            &device,
            &config,
            sample_format,
            channels,
            Engine::new(sample_rate),
            consumer,
            garbage_tx,
            Arc::clone(&shared),
        )?;
        stream
            .play()
            .map_err(|e| DeviceError(format!("could not start the stream: {e}")))?;

        Ok(Self {
            _stream: stream,
            commands: producer,
            garbage: garbage_rx,
            shared,
            sample_rate,
            next_voice: 1,
        })
    }

    /// The rate the device negotiated. Voices resample to it, so the host needs
    /// it for nothing except reporting — but a backend that silently ran at
    /// 44.1 kHz when asked for 48 is worth being able to see.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Post a command. Returns `false` if the ring was full, which also bumps
    /// [`AudioDevice::dropped_commands`].
    ///
    /// Dropping rather than blocking is deliberate: the alternative is a game
    /// thread waiting on an audio callback, which is the deadlock the whole
    /// design exists to avoid.
    pub fn send(&mut self, command: Command) -> bool {
        if self.commands.push(command).is_err() {
            self.shared.dropped_commands.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        true
    }

    /// Start a sound, returning the id to control it by.
    ///
    /// The id is valid even if the command was dropped — stopping a voice that
    /// never started is a no-op, and a caller that has to check would have to
    /// handle a case it can do nothing about.
    pub fn play(&mut self, source: PcmRef, params: PlayParams) -> VoiceId {
        let id = VoiceId(self.next_voice);
        self.next_voice += 1;
        self.play_as(id, source, params);
        id
    }

    /// [`Self::play`] under an id the caller already has.
    ///
    /// For a host that assigns its own handles — which the plugin boundary does,
    /// because the engine needs an id the instant it asks for a sound. Sharing
    /// the caller's numbering means no translation table between the two, and no
    /// way for the two to disagree about which voice is which.
    pub fn play_as(&mut self, id: VoiceId, source: PcmRef, params: PlayParams) {
        self.next_voice = self.next_voice.max(id.0 + 1);
        self.send(Command::Play { id, source, params });
    }

    /// Free everything the audio thread finished with, returning the ids.
    ///
    /// Two jobs in one drain, because they are the same drain. This is where a
    /// clip's memory is actually released — see the module doc — and it is also
    /// the only way a caller learns that a sound ended on its own rather than
    /// being stopped. Call once a frame.
    pub fn collect_finished(&mut self) -> Vec<VoiceId> {
        let mut out = Vec::new();
        while let Ok(voice) = self.garbage.pop() {
            out.push(voice.id());
            // `voice` drops here, on this thread, which is the entire point.
        }
        out
    }

    /// Peak level of the bus at `index`, in the order buses were added (0 is
    /// master). `0.0` for an index past what the mixer is publishing.
    pub fn peak(&self, index: usize) -> f32 {
        self.shared
            .peaks
            .get(index)
            .map(|p| f32::from_bits(p.load(Ordering::Relaxed)))
            .unwrap_or(0.0)
    }

    pub fn bus_count(&self) -> usize {
        self.shared.bus_count.load(Ordering::Relaxed)
    }

    /// How many voices are currently sounding. The host answers "is this entity
    /// still playing" from its own bookkeeping, but this is the truth about
    /// whether anything at all is.
    pub fn voice_count(&self) -> usize {
        self.shared.voice_count.load(Ordering::Relaxed)
    }

    /// Commands lost to a full ring since startup. Non-zero means the host is
    /// posting faster than the device drains, and sounds are going missing.
    pub fn dropped_commands(&self) -> usize {
        self.shared.dropped_commands.load(Ordering::Relaxed)
    }
}

/// Apply one command to the mixer. Split out so the callback body stays about
/// the audio and this stays about the vocabulary.
fn apply(engine: &mut Engine, command: Command) {
    match command {
        Command::AddBus(key) => {
            engine.add_bus(&key);
        }
        Command::RemoveBus(key) => {
            engine.remove_bus(&key);
        }
        Command::SetBus {
            key,
            gain,
            pan,
            muted,
            soloed,
        } => {
            if let Some(bus) = engine.bus_mut(&key) {
                bus.gain = gain;
                bus.pan = pan;
                bus.muted = muted;
                bus.soloed = soloed;
            }
        }
        Command::Play { id, source, params } => engine.play(id, source, &params),
        Command::Stop { id, fade } => engine.stop(id, fade),
        Command::StopBus { key, fade } => engine.stop_bus(&key, fade),
        Command::StopAll { fade } => engine.stop_all(fade),
        Command::SetListener(l) => engine.set_listener(l),
        Command::SetVoicePosition { id, position } => engine.set_voice_position(id, position),
        Command::SetVoiceGain { id, gain } => engine.set_voice_gain(id, gain),
        Command::SetVoiceBus { id, key } => engine.set_voice_bus(id, &key),
        Command::SetVoicePan { id, pan } => engine.set_voice_pan(id, pan),
        Command::SetVoiceEmitter { id, emitter } => engine.set_voice_emitter(id, emitter),
        Command::SetVoicePaused { id, paused } => engine.set_voice_paused(id, paused),
        Command::SetVoicePitch { id, pitch } => engine.set_voice_pitch(id, pitch),
        Command::PushFrames { bus, samples } => engine.push_frames(&bus, &samples),
    }
}

/// The per-block work, shared by every sample format.
///
/// Order matters: commands first so this block reflects what the host asked for,
/// then render, then publish. Rendering before draining would put every change
/// one block late for no reason.
fn tick(
    engine: &mut Engine,
    commands: &mut rtrb::Consumer<Command>,
    garbage: &mut rtrb::Producer<Voice>,
    shared: &Shared,
    scratch: &mut alloc::vec::Vec<f32>,
    frames: usize,
) {
    while let Ok(command) = commands.pop() {
        apply(engine, command);
    }

    if scratch.len() < frames * 2 {
        // Only ever on the first block of a given size. cpal can hand over a
        // larger buffer than the last one, and a short scratch would silently
        // render fewer frames than the device asked for.
        scratch.resize(frames * 2, 0.0);
    }
    engine.render(&mut scratch[..frames * 2]);

    // Back to the host to be freed. A full ring means the host has not called
    // `collect_garbage` in a while; dropping here is the degraded path, not the
    // normal one, and it costs a `free` in the callback rather than a leak.
    engine.drain_finished(|voice| {
        let _ = garbage.push(voice);
    });

    let buses = engine.buses();
    shared.bus_count.store(buses.len(), Ordering::Relaxed);
    shared.voice_count.store(engine.voice_count(), Ordering::Relaxed);
    for (i, bus) in buses.iter().take(MAX_METERED_BUSES).enumerate() {
        shared.peaks[i].store(bus.peak.to_bits(), Ordering::Relaxed);
    }
}

/// Spread the mixer's stereo block across however many channels the device has.
///
/// Mono devices get the two channels summed and halved rather than the left one:
/// taking one side would silence anything hard-panned the other way, which on a
/// mono laptop speaker is a bug report about "some sounds don't play".
fn spread(scratch: &[f32], out: &mut [f32], channels: usize) {
    match channels {
        0 => {}
        1 => {
            for (i, o) in out.iter_mut().enumerate() {
                *o = (scratch[i * 2] + scratch[i * 2 + 1]) * 0.5;
            }
        }
        2 => out.copy_from_slice(&scratch[..out.len()]),
        n => {
            for (f, frame) in out.chunks_exact_mut(n).enumerate() {
                frame[0] = scratch[f * 2];
                frame[1] = scratch[f * 2 + 1];
                for s in &mut frame[2..] {
                    *s = 0.0;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    format: SampleFormat,
    channels: usize,
    mut engine: Engine,
    mut commands: rtrb::Consumer<Command>,
    mut garbage: rtrb::Producer<Voice>,
    shared: Arc<Shared>,
) -> Result<cpal::Stream, DeviceError> {
    let mut scratch: alloc::vec::Vec<f32> = vec![0.0; 2048];
    // A device error is not recoverable from inside the callback, and the stream
    // is already dead by the time it fires. Swallowing it keeps the process
    // alive and silent, which is the right outcome for a game.
    let on_error = |_| {};

    let stream = match format {
        SampleFormat::F32 => device.build_output_stream(
            config,
            move |out: &mut [f32], _| {
                let frames = out.len() / channels.max(1);
                tick(
                    &mut engine,
                    &mut commands,
                    &mut garbage,
                    &shared,
                    &mut scratch,
                    frames,
                );
                spread(&scratch, out, channels);
            },
            on_error,
            None,
        ),
        SampleFormat::I16 => device.build_output_stream(
            config,
            move |out: &mut [i16], _| {
                let frames = out.len() / channels.max(1);
                tick(
                    &mut engine,
                    &mut commands,
                    &mut garbage,
                    &shared,
                    &mut scratch,
                    frames,
                );
                for (i, o) in out.iter_mut().enumerate() {
                    let f = mix_channel(&scratch, i, channels);
                    *o = (f * i16::MAX as f32) as i16;
                }
            },
            on_error,
            None,
        ),
        SampleFormat::U16 => device.build_output_stream(
            config,
            move |out: &mut [u16], _| {
                let frames = out.len() / channels.max(1);
                tick(
                    &mut engine,
                    &mut commands,
                    &mut garbage,
                    &shared,
                    &mut scratch,
                    frames,
                );
                for (i, o) in out.iter_mut().enumerate() {
                    let f = mix_channel(&scratch, i, channels);
                    // Unsigned formats put silence at half scale, not zero.
                    *o = ((f * 0.5 + 0.5) * u16::MAX as f32) as u16;
                }
            },
            on_error,
            None,
        ),
        other => {
            return Err(DeviceError(format!(
                "unsupported device sample format: {other:?}"
            )))
        }
    };
    stream.map_err(|e| DeviceError(format!("could not open the output stream: {e}")))
}

/// The sample for output index `i`, applying the same channel spread as
/// [`spread`] but one sample at a time — the integer paths convert as they go
/// rather than writing an `f32` buffer they would immediately re-read.
fn mix_channel(scratch: &[f32], i: usize, channels: usize) -> f32 {
    match channels {
        0 => 0.0,
        1 => (scratch[i * 2] + scratch[i * 2 + 1]) * 0.5,
        2 => scratch[i],
        n => {
            let (frame, ch) = (i / n, i % n);
            if ch < 2 {
                scratch[frame * 2 + ch]
            } else {
                0.0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn stereo_passes_straight_through() {
        let scratch = [0.1, 0.2, 0.3, 0.4];
        let mut out = [0.0; 4];
        spread(&scratch, &mut out, 2);
        assert_eq!(out, scratch);
    }

    /// Taking one side would silence anything hard-panned the other way, which
    /// on a mono laptop speaker reads as "some sounds don't play".
    #[test]
    fn mono_sums_both_sides_rather_than_taking_one() {
        let scratch = [1.0, 0.0, 0.0, 1.0];
        let mut out = [0.0; 2];
        spread(&scratch, &mut out, 1);
        assert_eq!(out, [0.5, 0.5]);
    }

    #[test]
    fn surround_fills_the_first_pair_and_silences_the_rest() {
        let scratch = [0.5, -0.5, 0.25, -0.25];
        let mut out = [9.0; 12];
        spread(&scratch, &mut out, 6);
        assert_eq!(out[0], 0.5);
        assert_eq!(out[1], -0.5);
        assert_eq!(&out[2..6], &[0.0; 4]);
        assert_eq!(out[6], 0.25);
        assert_eq!(out[7], -0.25);
    }

    /// The integer output paths convert as they go, so they must agree sample
    /// for sample with the f32 path they bypass.
    #[test]
    fn per_sample_mixing_matches_the_block_spread() {
        let scratch: Vec<f32> = (0..16).map(|i| i as f32 / 16.0).collect();
        for channels in [1usize, 2, 6] {
            let frames = 4;
            let mut block = vec![0.0; frames * channels];
            spread(&scratch, &mut block, channels);
            for (i, expected) in block.iter().enumerate() {
                let got = mix_channel(&scratch, i, channels);
                assert!(
                    (got - expected).abs() < 1e-6,
                    "channels={channels} i={i}: {got} != {expected}"
                );
            }
        }
    }

    #[test]
    fn a_zero_channel_device_does_not_divide_by_zero() {
        let mut out = [1.0; 4];
        spread(&[0.0; 8], &mut out, 0);
        assert_eq!(mix_channel(&[0.0; 8], 0, 0), 0.0);
    }
}
