//! The mixer graph: voices sum into buses, buses sum into master.
//!
//! This is the layer kira used to be. It is deliberately one level deep — a
//! voice picks a bus, a bus goes to master, and that is the whole topology —
//! because that is exactly what the engine's mixer panel exposes and nothing in
//! the editor can author anything deeper. An arbitrary DAG would be more
//! general and would earn nothing today.
//!
//! Everything here is real-time-safe in the sense that matters: [`Engine::render`]
//! allocates nothing and takes no locks. It reuses per-bus scratch buffers sized
//! on the first block and grown only when the host hands over a longer one, so
//! the audio callback never waits on the allocator while a deadline is running.

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;

use crate::effects::{Delay, Reverb};
use crate::pcm::PcmRef;
use crate::spatial::{balance_gains, pan_gains, Emitter, Listener};

/// Identifies a playing voice for later control (stop, move, retune).
///
/// A monotonic counter rather than a slot index: voices are recycled, and an
/// index alone would let a stale handle from a finished sound address whatever
/// took its place — silencing the wrong thing, which is the kind of bug that
/// only shows up under load.
///
/// Allocated by whoever *drives* the engine, not by the engine. Once the mixer
/// is running on a device thread, the caller needs the id the instant it asks
/// for a sound — long before the audio thread has seen the request — so the
/// counter has to live on the caller's side of the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VoiceId(pub u64);

/// Where a voice is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VoiceState {
    Playing,
    /// Fading out; removed when the fade completes. Stopping a voice outright
    /// would click — a discontinuity from full amplitude to zero is a step, and
    /// a step is broadband noise.
    Stopping,
    Finished,
}

/// One voice's contribution for a frame: what goes to its bus, and what goes to
/// each shared effect.
///
/// Returned as a struct rather than written into three buffers by the voice
/// itself, so the voice never learns which bus it is on or that the effects
/// exist — the mixer routes, the voice only sounds.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Frame {
    dry: [f32; 2],
    reverb: [f32; 2],
    delay: [f32; 2],
}

impl Frame {
    const SILENT: Self = Self {
        dry: [0.0, 0.0],
        reverb: [0.0, 0.0],
        delay: [0.0, 0.0],
    };
}

/// One playing sound.
///
/// Public but opaque: the device layer hands finished voices back across a
/// queue to be freed off the audio thread, so it needs to name the type without
/// needing anything inside it.
pub struct Voice {
    id: VoiceId,
    /// Index into [`Engine::buses`]. Resolved when the voice starts, so a bus
    /// removed mid-playback can't dangle — see [`Engine::remove_bus`].
    bus: usize,
    source: PcmRef,
    /// Playhead, in source frames. Fractional because the source rate and the
    /// device rate rarely match and pitch shifts it further.
    position: f64,
    /// Frames of source consumed per output frame: pitch × (source_rate /
    /// device_rate). Resampling and pitch are the same operation, so they are
    /// the same number.
    rate: f64,
    gain: f32,
    pan: f32,
    /// `(start, end)` in source frames. `None` plays through once.
    looping: Option<(f64, f64)>,
    /// Spatial parameters, or `None` for a 2D voice.
    emitter: Option<Emitter>,
    /// Proportion of this voice sent to the shared reverb and delay. Post-fader
    /// and post-spatial, so a distant sound sends less reverb than a near one —
    /// which is the behaviour that makes distance read at all.
    reverb_send: f32,
    delay_send: f32,
    /// Current fade multiplier and the per-frame step toward its target. Fades
    /// are linear in amplitude; over the tens of milliseconds they last, the
    /// difference from an equal-power curve is not audible and the arithmetic
    /// is one add per frame.
    fade: f32,
    fade_step: f32,
    /// Held silent with its playhead where it is. Distinct from a zero fade,
    /// which keeps advancing — resuming a paused sound has to carry on from
    /// where it stopped, not from where it would have been.
    paused: bool,
    state: VoiceState,
}

impl Voice {
    /// Which voice this is. The device layer needs it to tell the host which
    /// voices finished, since the host's bookkeeping is keyed by id and it has
    /// no other way to learn a sound ended on its own.
    pub fn id(&self) -> VoiceId {
        self.id
    }

    /// Advance one output frame, returning its dry contribution and what it
    /// sends to the two effect buses.
    fn next_frame(&mut self, listener: &Listener) -> Frame {
        // Silent *and* not advancing: a paused voice must resume where it
        // stopped. Returning silence while still stepping the playhead is the
        // classic version of this bug — the sound comes back mid-word.
        if self.state == VoiceState::Finished || self.paused {
            return Frame::SILENT;
        }

        let (spatial_gain, pan) = match &self.emitter {
            Some(e) => {
                let (g, p) = e.gain_and_pan(listener);
                // The voice's own pan biases the spatial result rather than
                // replacing it, so an author can push a positioned sound to one
                // side without losing its position.
                (g, (p + self.pan).clamp(-1.0, 1.0))
            }
            None => (1.0, self.pan),
        };

        let frame = self.source.sample(self.position);
        let [gl, gr] = pan_gains(pan);
        let amp = self.gain * spatial_gain * self.fade;
        let dry = [frame[0] * gl * amp, frame[1] * gr * amp];
        let out = Frame {
            dry,
            reverb: [dry[0] * self.reverb_send, dry[1] * self.reverb_send],
            delay: [dry[0] * self.delay_send, dry[1] * self.delay_send],
        };

        self.position += self.rate;
        if let Some((start, end)) = self.looping {
            if self.position >= end && end > start {
                // Wrap by the loop length rather than snapping to `start`, or a
                // rate above 1.0 would quantise the loop point to block edges
                // and drift the tempo of anything rhythmic.
                let length = end - start;
                self.position = start + (self.position - start) % length;
            }
        } else if self.position >= self.source.frames() as f64 {
            self.state = VoiceState::Finished;
        }

        if self.fade_step != 0.0 {
            self.fade += self.fade_step;
            if self.fade >= 1.0 {
                self.fade = 1.0;
                self.fade_step = 0.0;
            } else if self.fade <= 0.0 {
                self.fade = 0.0;
                self.fade_step = 0.0;
                if self.state == VoiceState::Stopping {
                    self.state = VoiceState::Finished;
                }
            }
        }
        out
    }

    fn is_finished(&self) -> bool {
        self.state == VoiceState::Finished
    }
}

/// One mixer bus.
pub struct Bus {
    /// The routing key. Matches `renzora_audio`'s `Bus::key` — the permanent
    /// identifier an `AudioPlayer` stores, never the display name.
    pub key: String,
    pub gain: f32,
    pub pan: f32,
    pub muted: bool,
    pub soloed: bool,
    /// Peak amplitude seen in the last rendered block, for the mixer's VU
    /// meters. Written by [`Engine::render`], read by the host between blocks.
    pub peak: f32,
    /// Interleaved stereo accumulation for the current block. Kept on the bus
    /// so rendering allocates nothing.
    scratch: Vec<f32>,
    /// Interleaved stereo samples pushed in from outside — a microphone being
    /// monitored, a voice-chat stream, a synth, anything generating audio at
    /// run time rather than loading it.
    ///
    /// This is the third kind of source a bus accepts, alongside clips and
    /// nothing. Keeping it a plain sample queue rather than a "voice chat"
    /// feature is what makes those cases the same code: whoever has samples
    /// pushes them, and the mixer does not care where they came from.
    feed: VecDeque<f32>,
}

/// Samples a bus will hold from [`Engine::push_frames`] before discarding the
/// oldest — a quarter second of stereo at 48 kHz.
///
/// Bounded because a feed nobody drains is a leak, and unbounded latency on a
/// live stream is worse than a gap: a monitor that runs half a second behind is
/// unusable, while one that drops a few milliseconds is merely imperfect. When
/// it overflows the *oldest* samples go, because live audio wants the newest.
const FEED_CAPACITY: usize = 48_000 / 2;

impl Bus {
    fn new(key: String) -> Self {
        Self {
            key,
            gain: 1.0,
            pan: 0.0,
            muted: false,
            soloed: false,
            peak: 0.0,
            scratch: Vec::new(),
            feed: VecDeque::with_capacity(FEED_CAPACITY),
        }
    }

    /// Amplitude after mute and solo. Mirrors the engine's
    /// `ChannelStrip::effective_volume` exactly — the two must agree, or the
    /// mixer panel shows one thing and the speakers do another.
    fn effective_gain(&self, any_solo: bool) -> f32 {
        if self.muted || (any_solo && !self.soloed) {
            return 0.0;
        }
        self.gain
    }
}

/// The mixer. Owns every bus and every playing voice.
pub struct Engine {
    /// Output rate, fixed by the device at startup. Every voice's `rate` is
    /// computed against it.
    sample_rate: u32,
    /// Master is index 0 and always exists; the host's buses follow.
    buses: Vec<Bus>,
    voices: Vec<Voice>,
    /// Voices that finished during the last block, waiting to be handed back.
    ///
    /// They are not dropped here on purpose. Dropping a `Voice` releases its
    /// `PcmRef`, and if that was the last reference the deallocation happens
    /// wherever the drop happened — which, once the mixer runs on a device
    /// thread, is the middle of a callback with a deadline. [`drain_finished`]
    /// lets the caller take them somewhere it is safe to free them.
    ///
    /// [`drain_finished`]: Engine::drain_finished
    finished: Vec<Voice>,
    listener: Listener,
    reverb: Reverb,
    delay: Delay,
    /// Interleaved stereo sums of everything sent to each effect this block.
    /// Kept on the engine so rendering allocates nothing.
    reverb_bus: Vec<f32>,
    delay_bus: Vec<f32>,
}

/// How a voice should start.
#[derive(Debug, Clone)]
pub struct PlayParams {
    /// Routing key of the target bus. An unknown key routes to master rather
    /// than dropping the sound — a mis-keyed emitter should be audible and
    /// wrong, not silent and invisible.
    pub bus: String,
    pub gain: f32,
    pub pan: f32,
    /// Playback speed multiplier; 1.0 is the source's own pitch.
    pub pitch: f64,
    /// Loop region in seconds. `None` plays once.
    pub looping: Option<(f64, f64)>,
    pub fade_in: f32,
    pub emitter: Option<Emitter>,
    /// Where to begin, in seconds.
    pub start: f64,
    /// How much of this voice reaches the shared reverb and delay, 0..1. These
    /// are the engine's `AudioPlayer::reverb_send` / `delay_send` verbatim.
    pub reverb_send: f32,
    pub delay_send: f32,
}

impl Default for PlayParams {
    fn default() -> Self {
        Self {
            bus: String::new(),
            gain: 1.0,
            pan: 0.0,
            pitch: 1.0,
            looping: None,
            fade_in: 0.0,
            emitter: None,
            start: 0.0,
            reverb_send: 0.0,
            delay_send: 0.0,
        }
    }
}

impl Engine {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate: sample_rate.max(1),
            buses: alloc::vec![Bus::new(String::from("Master"))],
            voices: Vec::new(),
            finished: Vec::new(),
            listener: Listener::default(),
            reverb: Reverb::new(sample_rate.max(1)),
            delay: Delay::new(sample_rate.max(1)),
            reverb_bus: Vec::new(),
            delay_bus: Vec::new(),
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn set_listener(&mut self, listener: Listener) {
        self.listener = listener;
    }

    /// Add a bus, or return the existing one's index if the key is taken.
    ///
    /// Idempotent because the host re-sends its whole bus graph whenever the
    /// mixer changes, and a graph sync that duplicated buses on every fader
    /// twitch would leak one bus per frame.
    pub fn add_bus(&mut self, key: &str) -> usize {
        if let Some(i) = self.bus_index(key) {
            return i;
        }
        self.buses.push(Bus::new(String::from(key)));
        self.buses.len() - 1
    }

    /// Remove a bus and reseat every voice that was playing on it.
    ///
    /// Master (index 0) is never removed. Voices move to master rather than
    /// being stopped: a bus disappearing is an authoring action, and silencing
    /// live sound as a side effect of an edit is the more surprising outcome.
    pub fn remove_bus(&mut self, key: &str) -> bool {
        let Some(index) = self.bus_index(key).filter(|i| *i != 0) else {
            return false;
        };
        self.buses.remove(index);
        // Indices above the hole shift down by one; anything that pointed *at*
        // the hole falls back to master.
        for voice in &mut self.voices {
            if voice.bus == index {
                voice.bus = 0;
            } else if voice.bus > index {
                voice.bus -= 1;
            }
        }
        true
    }

    pub fn bus_index(&self, key: &str) -> Option<usize> {
        self.buses.iter().position(|b| b.key == key)
    }

    pub fn bus_mut(&mut self, key: &str) -> Option<&mut Bus> {
        self.bus_index(key).map(|i| &mut self.buses[i])
    }

    pub fn buses(&self) -> &[Bus] {
        &self.buses
    }

    /// Number of voices that have not finished. The host uses it to decide
    /// whether an entity's sound is still playing.
    pub fn voice_count(&self) -> usize {
        self.voices.len()
    }

    /// Hand every voice that finished since the last call to `sink`.
    ///
    /// The point is *where* the resulting drop happens: see [`Engine::finished`].
    /// A caller that ignores this leaks nothing — the vector is emptied either
    /// way — it just frees on whichever thread called.
    pub fn drain_finished(&mut self, mut sink: impl FnMut(Voice)) {
        for voice in self.finished.drain(..) {
            sink(voice);
        }
    }

    /// Push interleaved stereo samples onto a bus, to be mixed in as they are
    /// consumed.
    ///
    /// The generic "audio from somewhere that isn't a file" path: mic
    /// monitoring, a remote player's voice, a synth, a decoded network stream.
    /// Unknown bus keys are ignored rather than falling back to master — unlike
    /// a clip, a feed is a continuous stream, and silently redirecting one to
    /// the wrong bus would be a mix problem that never stops.
    pub fn push_frames(&mut self, key: &str, samples: &[f32]) {
        let Some(index) = self.bus_index(key) else {
            return;
        };
        let feed = &mut self.buses[index].feed;
        // Drop from the front first, so the push below stays inside the
        // capacity the queue was built with and never reallocates.
        let overflow = (feed.len() + samples.len()).saturating_sub(FEED_CAPACITY);
        for _ in 0..overflow.min(feed.len()) {
            feed.pop_front();
        }
        for &s in samples.iter().take(FEED_CAPACITY) {
            feed.push_back(s);
        }
    }

    /// Samples currently queued on a bus's feed. A host draining a capture into
    /// a monitor uses this to notice it is falling behind.
    pub fn feed_len(&self, key: &str) -> usize {
        self.bus_index(key)
            .map(|i| self.buses[i].feed.len())
            .unwrap_or(0)
    }

    /// Start a voice under a caller-supplied id.
    pub fn play(&mut self, id: VoiceId, source: PcmRef, params: &PlayParams) {
        let source_rate = source.sample_rate() as f64;
        let bus = self.bus_index(&params.bus).unwrap_or(0);
        // Seconds → source frames. Loop points and the start offset are authored
        // against the *source*, not the device, so both convert by the source's
        // own rate.
        let to_frames = |secs: f64| secs * source_rate;
        let frames = source.frames() as f64;
        // An empty source can't loop: with no region to wrap into, the voice
        // would never reach the end-of-source check either, and would sit in the
        // mix forever contributing silence.
        let looping = if frames <= 0.0 {
            None
        } else {
            params.looping.map(|(a, b)| {
                let (start, end) = (to_frames(a), to_frames(b));
                // Inverted, empty, or entirely past the end of the clip — none
                // of which we can loop. Fall back to the whole clip rather than
                // stranding the playhead on a zero-length region, which spins
                // without advancing and leaks a voice per call.
                if end <= start || start >= frames {
                    (0.0, frames)
                } else {
                    (start.max(0.0), end.min(frames))
                }
            })
        };

        let fade_frames = params.fade_in * self.sample_rate as f32;
        let (fade, fade_step) = if fade_frames >= 1.0 {
            (0.0, 1.0 / fade_frames)
        } else {
            (1.0, 0.0)
        };

        self.voices.push(Voice {
            id,
            bus,
            position: to_frames(params.start).clamp(0.0, frames),
            rate: params.pitch.max(0.0) * source_rate / self.sample_rate as f64,
            gain: params.gain,
            pan: params.pan,
            looping,
            emitter: params.emitter,
            reverb_send: params.reverb_send.clamp(0.0, 1.0),
            delay_send: params.delay_send.clamp(0.0, 1.0),
            fade,
            fade_step,
            paused: false,
            state: VoiceState::Playing,
            source,
        });
    }

    /// Fade a voice out over `fade` seconds, then drop it. A zero fade still
    /// takes one block to fall to silence rather than cutting mid-waveform.
    pub fn stop(&mut self, id: VoiceId, fade: f32) {
        if let Some(v) = self.voices.iter_mut().find(|v| v.id == id) {
            v.state = VoiceState::Stopping;
            let frames = (fade * self.sample_rate as f32).max(1.0);
            v.fade_step = -v.fade / frames;
        }
    }

    /// Stop every voice on a bus — the backend half of "stop all sounds".
    pub fn stop_bus(&mut self, key: &str, fade: f32) {
        let Some(index) = self.bus_index(key) else {
            return;
        };
        let ids: Vec<VoiceId> = self
            .voices
            .iter()
            .filter(|v| v.bus == index)
            .map(|v| v.id)
            .collect();
        for id in ids {
            self.stop(id, fade);
        }
    }

    pub fn stop_all(&mut self, fade: f32) {
        let ids: Vec<VoiceId> = self.voices.iter().map(|v| v.id).collect();
        for id in ids {
            self.stop(id, fade);
        }
    }

    pub fn is_playing(&self, id: VoiceId) -> bool {
        self.voices
            .iter()
            .any(|v| v.id == id && v.state != VoiceState::Finished)
    }

    /// Move a positioned voice. No-op for a 2D voice — a caller that sets a
    /// position on a non-spatial sound has made a mistake, but making the sound
    /// vanish is not a useful way to report it.
    pub fn set_voice_position(&mut self, id: VoiceId, position: [f32; 3]) {
        if let Some(v) = self.voices.iter_mut().find(|v| v.id == id) {
            if let Some(e) = &mut v.emitter {
                e.position = position;
            }
        }
    }

    pub fn set_voice_gain(&mut self, id: VoiceId, gain: f32) {
        if let Some(v) = self.voices.iter_mut().find(|v| v.id == id) {
            v.gain = gain;
        }
    }

    /// Hold a voice silent, or let it carry on.
    pub fn set_voice_paused(&mut self, id: VoiceId, paused: bool) {
        if let Some(v) = self.voices.iter_mut().find(|v| v.id == id) {
            v.paused = paused;
        }
    }

    /// Retune a playing voice.
    ///
    /// Recomputed against the *source's* rate rather than scaled from the
    /// current value, so repeated changes cannot drift: setting 1.0 twice must
    /// leave a clip at its own pitch, not at its own pitch times whatever the
    /// resampling ratio happened to be.
    pub fn set_voice_pitch(&mut self, id: VoiceId, pitch: f64) {
        let device_rate = self.sample_rate as f64;
        if let Some(v) = self.voices.iter_mut().find(|v| v.id == id) {
            v.rate = pitch.max(0.0) * v.source.sample_rate() as f64 / device_rate;
        }
    }

    /// Render one block into `out` (interleaved stereo), replacing its contents.
    ///
    /// Allocation-free after the first block of a given size: the only `Vec`s
    /// touched are the per-bus scratch buffers, which are resized here and
    /// reused thereafter. `out.len()` must be even; a trailing half-frame is
    /// ignored rather than panicking, because this runs in a device callback
    /// where a panic is an abort.
    pub fn render(&mut self, out: &mut [f32]) {
        let frames = out.len() / 2;
        for s in out.iter_mut() {
            *s = 0.0;
        }
        if frames == 0 {
            return;
        }

        for bus in &mut self.buses {
            if bus.scratch.len() < frames * 2 {
                bus.scratch.resize(frames * 2, 0.0);
            }
            for s in bus.scratch[..frames * 2].iter_mut() {
                *s = 0.0;
            }
            bus.peak = 0.0;
        }
        for send in [&mut self.reverb_bus, &mut self.delay_bus] {
            if send.len() < frames * 2 {
                send.resize(frames * 2, 0.0);
            }
            for s in send[..frames * 2].iter_mut() {
                *s = 0.0;
            }
        }

        // Voices → their bus's scratch, and proportionally into the two shared
        // effect buses.
        let listener = self.listener;
        for voice in &mut self.voices {
            let scratch = match self.buses.get_mut(voice.bus) {
                Some(b) => &mut b.scratch,
                None => continue,
            };
            for f in 0..frames {
                let frame = voice.next_frame(&listener);
                scratch[f * 2] += frame.dry[0];
                scratch[f * 2 + 1] += frame.dry[1];
                self.reverb_bus[f * 2] += frame.reverb[0];
                self.reverb_bus[f * 2 + 1] += frame.reverb[1];
                self.delay_bus[f * 2] += frame.delay[0];
                self.delay_bus[f * 2 + 1] += frame.delay[1];
            }
        }
        // Moved, not dropped — see `Engine::finished`.
        let mut i = 0;
        while i < self.voices.len() {
            if self.voices[i].is_finished() {
                let voice = self.voices.swap_remove(i);
                self.finished.push(voice);
            } else {
                i += 1;
            }
        }

        // Feeds → their own bus's scratch, on the same footing as voices. Drained
        // rather than read, because a feed is consumed exactly once: two blocks
        // playing the same samples is a stutter, not a mix.
        for bus in &mut self.buses {
            let take = (bus.feed.len() / 2).min(frames);
            for f in 0..take {
                bus.scratch[f * 2] += bus.feed.pop_front().unwrap_or(0.0);
                bus.scratch[f * 2 + 1] += bus.feed.pop_front().unwrap_or(0.0);
            }
        }

        // Solo is a property of the whole board, so it has to be decided before
        // any bus is summed — a bus cannot know whether it is the soloed one.
        let any_solo = self.buses.iter().skip(1).any(|b| b.soloed);

        // Buses → master. Master is index 0 and is summed last, after everything
        // has been folded into it, so its own gain applies to the whole mix.
        let (master, rest) = self.buses.split_at_mut(1);
        let master = &mut master[0];
        for bus in rest.iter_mut() {
            let gain = bus.effective_gain(any_solo);
            let [gl, gr] = balance_gains(bus.pan);
            let mut peak = 0.0f32;
            for f in 0..frames {
                let l = bus.scratch[f * 2] * gain * gl;
                let r = bus.scratch[f * 2 + 1] * gain * gr;
                master.scratch[f * 2] += l;
                master.scratch[f * 2 + 1] += r;
                peak = peak.max(l.abs()).max(r.abs());
            }
            // Metered post-fader: the meter should answer "what is this bus
            // contributing", so muting a bus must take its meter to zero.
            bus.peak = peak;
        }

        // The two effect returns land on master, after the dry buses. They are
        // deliberately not routed through a bus of their own: a reverb return
        // that could be muted or soloed would let the board reach a state where
        // a sound is audible but its tail is not, which is not a mix anyone
        // wants and is a support question nobody can answer.
        //
        // Processed even when nothing was sent this block, because a reverb tail
        // outlives its input by design — skipping the call on a silent input
        // would chop the tail off the moment the last voice stopped.
        for f in 0..frames {
            let wet = self
                .reverb
                .process([self.reverb_bus[f * 2], self.reverb_bus[f * 2 + 1]]);
            let echo = self
                .delay
                .process([self.delay_bus[f * 2], self.delay_bus[f * 2 + 1]]);
            master.scratch[f * 2] += wet[0] + echo[0];
            master.scratch[f * 2 + 1] += wet[1] + echo[1];
        }

        let master_gain = if master.muted { 0.0 } else { master.gain };
        let [ml, mr] = balance_gains(master.pan);
        let mut peak = 0.0f32;
        for f in 0..frames {
            let l = master.scratch[f * 2] * master_gain * ml;
            let r = master.scratch[f * 2 + 1] * master_gain * mr;
            // Hard-clip at the output rather than letting the device wrap. A
            // summed mix can exceed full scale trivially — ten voices at unity
            // will — and integer wrap-around is a much louder artefact than
            // clipping is.
            out[f * 2] = l.clamp(-1.0, 1.0);
            out[f * 2 + 1] = r.clamp(-1.0, 1.0);
            peak = peak.max(l.abs()).max(r.abs());
        }
        master.peak = peak;
    }
}
