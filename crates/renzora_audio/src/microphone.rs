//! Live microphone input as a Kira [`Sound`].
//!
//! A cpal input stream pushes interleaved samples into the producer side of
//! an rtrb SPSC ringbuffer; a [`MicrophoneSound`] running on Kira's audio
//! thread pops [`Frame`]s from the consumer side. The cpal `Stream` is
//! `!Send` on most platforms, so it is parked on the NonSend
//! [`KiraAudioManager`] in [`crate::manager::ActiveInputStream`] and torn
//! down explicitly when a bus's input binding changes.
//!
//! Sample-rate / resampling note: this implementation treats the cpal
//! device's native rate as authoritative and feeds samples through 1:1.
//! Kira's audio renderer typically runs at the output device's rate; if the
//! input device's rate differs, playback pitch will be off. Most consumer
//! mics report 44.1k or 48k matching the system output, which is the only
//! configuration we currently expect to encounter. Proper resampling can be
//! added by routing samples through `rubato` before they hit the ringbuffer.
//!
//! Channel-layout note: mono inputs are duplicated to L=R; multi-channel
//! inputs are downmixed by taking the first two channels and discarding
//! the rest.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use kira::{
    info::Info,
    sound::{Sound, SoundData},
    Frame,
};
use rtrb::{Consumer, Producer, RingBuffer};

/// Failure modes when starting a mic capture.
#[derive(Debug)]
pub enum MicError {
    NoInputDevice,
    DeviceNotFound(String),
    DefaultConfig(String),
    UnsupportedFormat(cpal::SampleFormat),
    BuildStream(String),
    StreamPlay(String),
}

impl std::fmt::Display for MicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoInputDevice => write!(f, "no default input device"),
            Self::DeviceNotFound(name) => write!(f, "input device not found: {}", name),
            Self::DefaultConfig(s) => write!(f, "could not query default config: {}", s),
            Self::UnsupportedFormat(fmt) => write!(f, "unsupported sample format: {:?}", fmt),
            Self::BuildStream(s) => write!(f, "failed to build input stream: {}", s),
            Self::StreamPlay(s) => write!(f, "failed to start input stream: {}", s),
        }
    }
}

impl std::error::Error for MicError {}

/// Ringbuffer capacity in stereo frames. ~50 ms at 48 kHz — large enough to
/// absorb scheduling jitter between cpal's input thread and Kira's audio
/// thread without lengthening perceived latency. Smaller = lower latency
/// but more dropouts if either thread stalls.
const RING_FRAME_CAPACITY: usize = 2400;

/// Handle returned by `play(MicrophoneSoundData)`. Drop it (or call `stop`)
/// to ask Kira to release the [`MicrophoneSound`] on its next housekeeping
/// pass — the cpal stream is owned separately and must be dropped by its
/// holder to actually stop capture.
pub struct MicrophoneSoundHandle {
    stop_flag: Arc<AtomicBool>,
}

impl MicrophoneSoundHandle {
    /// Signal the [`MicrophoneSound`] to report `finished()`. Idempotent.
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }
}

impl Drop for MicrophoneSoundHandle {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }
}

/// `SoundData` for a live mic. Owns the consumer end of the ringbuffer so
/// it can be moved into Kira's audio thread by [`SoundData::into_sound`].
pub struct MicrophoneSoundData {
    consumer: Consumer<Frame>,
    stop_flag: Arc<AtomicBool>,
}

impl SoundData for MicrophoneSoundData {
    type Error = std::convert::Infallible;
    type Handle = MicrophoneSoundHandle;

    fn into_sound(self) -> Result<(Box<dyn Sound>, Self::Handle), Self::Error> {
        let handle = MicrophoneSoundHandle {
            stop_flag: self.stop_flag.clone(),
        };
        let sound = MicrophoneSound {
            consumer: self.consumer,
            stop_flag: self.stop_flag,
        };
        Ok((Box::new(sound), handle))
    }
}

struct MicrophoneSound {
    consumer: Consumer<Frame>,
    stop_flag: Arc<AtomicBool>,
}

impl Sound for MicrophoneSound {
    fn process(&mut self, out: &mut [Frame], _dt: f64, _info: &Info) {
        for frame in out.iter_mut() {
            *frame = self.consumer.pop().unwrap_or(Frame::ZERO);
        }
    }

    fn finished(&self) -> bool {
        self.stop_flag.load(Ordering::Relaxed)
    }
}

/// Result of opening a mic device. The caller hands `data` to a Kira track
/// via `play()` and parks `stream` somewhere that won't drop until capture
/// should end (typically the NonSend `KiraAudioManager`).
pub struct OpenedMicrophone {
    pub stream: cpal::Stream,
    pub data: MicrophoneSoundData,
    pub config: cpal::StreamConfig,
}

/// List input device names available on the default cpal host. Devices
/// whose name lookup fails are silently skipped (rare, mostly virtual
/// devices on Windows).
pub fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    host.input_devices()
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|d| d.name().ok())
        .collect()
}

/// List output device names available on the default cpal host.
pub fn list_output_devices() -> Vec<String> {
    let host = cpal::default_host();
    host.output_devices()
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|d| d.name().ok())
        .collect()
}

/// Open the named input device and start a cpal stream feeding a fresh
/// ringbuffer. The returned [`OpenedMicrophone::data`] should be `play()`'d
/// on a Kira track to start consuming samples; [`OpenedMicrophone::stream`]
/// must be kept alive to keep capture running.
pub fn open_microphone(device_name: &str) -> Result<OpenedMicrophone, MicError> {
    let host = cpal::default_host();
    let device = host
        .input_devices()
        .map_err(|_| MicError::NoInputDevice)?
        .find(|d| d.name().map(|n| n == device_name).unwrap_or(false))
        .ok_or_else(|| MicError::DeviceNotFound(device_name.to_string()))?;

    let supported = device
        .default_input_config()
        .map_err(|e| MicError::DefaultConfig(e.to_string()))?;
    let channels = supported.channels();
    let sample_format = supported.sample_format();
    let config: cpal::StreamConfig = supported.config();

    let (producer, consumer) = RingBuffer::<Frame>::new(RING_FRAME_CAPACITY);
    let stop_flag = Arc::new(AtomicBool::new(false));

    let stream = match sample_format {
        cpal::SampleFormat::F32 => build_input_stream::<f32>(&device, &config, channels, producer)?,
        cpal::SampleFormat::I16 => build_input_stream::<i16>(&device, &config, channels, producer)?,
        cpal::SampleFormat::U16 => build_input_stream::<u16>(&device, &config, channels, producer)?,
        other => return Err(MicError::UnsupportedFormat(other)),
    };

    stream
        .play()
        .map_err(|e| MicError::StreamPlay(e.to_string()))?;

    Ok(OpenedMicrophone {
        stream,
        data: MicrophoneSoundData { consumer, stop_flag },
        config,
    })
}

/// Adapter from a cpal native sample type to a normalised f32 in `[-1, 1]`.
trait ToF32: Copy {
    fn to_f32(self) -> f32;
}

impl ToF32 for f32 {
    fn to_f32(self) -> f32 {
        self
    }
}

impl ToF32 for i16 {
    fn to_f32(self) -> f32 {
        // i16::MIN / 32768.0 = -1.0; i16::MAX / 32768.0 ≈ 0.99997.
        self as f32 * (1.0 / 32768.0)
    }
}

impl ToF32 for u16 {
    fn to_f32(self) -> f32 {
        (self as i32 - 32768) as f32 * (1.0 / 32768.0)
    }
}

// ─── Mixer ↔ cpal stream reconciliation ─────────────────────────────────────

use std::collections::HashMap;

use bevy::prelude::*;

use crate::manager::{ActiveInputStream, KiraAudioManager};
use crate::mixer::{ChannelStrip, MixerState};

/// Bus names used for the four built-in strips. Must match the names
/// `KiraAudioManager::play_on_bus` checks against.
const BUILT_IN_BUS_NAMES: [&str; 4] = ["Master", "Sfx", "Music", "Ambient"];

/// System: when [`MixerState`] changes, open / close cpal input streams so
/// the set of active mic captures matches the buses' `input_device` fields.
///
/// Always-on while the binding is set: streams aren't gated on whether the
/// user is recording. That way the mixer's level meters work continuously
/// and the user can dial in mic gain before pressing record.
pub fn sync_microphone_inputs(
    mixer: Res<MixerState>,
    audio: Option<NonSendMut<KiraAudioManager>>,
) {
    let Some(mut audio) = audio else { return };
    if !mixer.is_changed() {
        return;
    }

    let desired = collect_bus_inputs(&mixer);

    // Tear down streams whose bus no longer wants input or whose device has
    // changed. Done first so the second loop sees a clean slate when
    // reopening on the same bus.
    let to_close: Vec<String> = audio
        .input_streams
        .iter()
        .filter_map(|(bus, active)| match desired.get(bus) {
            Some(dev) if dev == &active.device_name => None,
            _ => Some(bus.clone()),
        })
        .collect();
    for bus in to_close {
        if audio.input_streams.remove(&bus).is_some() {
            info!("[MicCapture] Closed input on bus '{}'", bus);
        }
    }

    // Open streams for bindings that don't have an active stream yet.
    let to_open: Vec<(String, String)> = desired
        .into_iter()
        .filter(|(bus, _)| !audio.input_streams.contains_key(bus))
        .collect();
    for (bus, dev) in to_open {
        match open_microphone(&dev) {
            Ok(opened) => {
                let OpenedMicrophone { stream, data, .. } = opened;
                match audio.play_on_bus(data, &bus, &mixer) {
                    Ok(handle) => {
                        audio.input_streams.insert(
                            bus.clone(),
                            ActiveInputStream {
                                device_name: dev.clone(),
                                stream,
                                sound: handle,
                            },
                        );
                        info!(
                            "[MicCapture] Opened input '{}' → bus '{}'",
                            dev, bus
                        );
                    }
                    Err(e) => warn!(
                        "[MicCapture] play '{}' on bus '{}' failed: {:?}",
                        dev, bus, e
                    ),
                }
            }
            Err(e) => warn!("[MicCapture] open '{}' for bus '{}' failed: {}", dev, bus, e),
        }
    }
}

/// Collect every bus's `input_device` binding into a `bus_name → device_name`
/// map. Buses with no binding are omitted.
fn collect_bus_inputs(mixer: &MixerState) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let built_ins: [(&str, &ChannelStrip); 4] = [
        (BUILT_IN_BUS_NAMES[0], &mixer.master),
        (BUILT_IN_BUS_NAMES[1], &mixer.sfx),
        (BUILT_IN_BUS_NAMES[2], &mixer.music),
        (BUILT_IN_BUS_NAMES[3], &mixer.ambient),
    ];
    for (name, strip) in built_ins {
        if let Some(dev) = strip.input_device.clone() {
            out.insert(name.to_string(), dev);
        }
    }
    for (name, strip) in &mixer.custom_buses {
        if let Some(dev) = strip.input_device.clone() {
            out.insert(name.clone(), dev);
        }
    }
    out
}

fn build_input_stream<S>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: u16,
    mut producer: Producer<Frame>,
) -> Result<cpal::Stream, MicError>
where
    S: cpal::SizedSample + ToF32 + Send + 'static,
{
    let stride = channels.max(1) as usize;
    let err_fn = |e: cpal::StreamError| {
        bevy::log::warn!("[MicCapture] cpal stream error: {}", e);
    };
    let stream = device
        .build_input_stream::<S, _, _>(
            config,
            move |data: &[S], _: &cpal::InputCallbackInfo| {
                if stride == 1 {
                    for sample in data.iter() {
                        let v = sample.to_f32();
                        // Drop samples on overflow rather than block the
                        // input callback — better an audible glitch than a
                        // stalled audio thread.
                        let _ = producer.push(Frame::new(v, v));
                    }
                } else {
                    for chunk in data.chunks_exact(stride) {
                        let l = chunk[0].to_f32();
                        let r = chunk[1].to_f32();
                        let _ = producer.push(Frame::new(l, r));
                    }
                }
            },
            err_fn,
            None,
        )
        .map_err(|e| MicError::BuildStream(e.to_string()))?;
    Ok(stream)
}
