//! Microphone and line input.
//!
//! Capture hands samples *back to the caller* rather than wiring them straight
//! into the mixer, and that is the whole design. Monitoring a mic on a bus and
//! sending a mic over the network are the same operation up to the point where
//! the samples arrive; splitting there means one code path serves recording to
//! disk, voice chat, level analysis and a tuner, instead of four features that
//! each know about microphones.
//!
//! So: the device callback fills a ring, the host drains it once a frame, and
//! whatever the host wants to do with the samples — including pushing them to a
//! bus with [`Engine::push_frames`] — it does with the samples in hand. The cost
//! is one frame of monitoring latency, which is the right trade against the
//! alternative of a second consumer the mixer would have to own.
//!
//! [`Engine::push_frames`]: crate::graph::Engine::push_frames

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

use crate::device::DeviceError;

/// Samples buffered between host drains. Half a second of stereo at 48 kHz —
/// generous, because a host that stalls for a frame should not lose audio, and
/// a host that stalls for half a second has bigger problems than a gap.
const CAPTURE_CAPACITY: usize = 48_000;

/// A device's human-readable name.
///
/// `description()` rather than the deprecated `name()`: cpal 0.17 split the two,
/// with `id()` as the stable machine identifier and `description()` carrying the
/// name a person reads. We want the readable one — this string goes in the
/// mixer's device menu — and deliberately *not* the id, because a project must
/// not persist a machine-specific device handle (see the engine's `AudioConfig`,
/// which stores no devices at all for the same reason).
fn readable_name(device: &cpal::Device) -> Option<String> {
    device.description().ok().map(|d| d.name().into())
}

/// A live input stream, and the samples it has produced.
///
/// Dropping this closes the device.
pub struct Capture {
    _stream: cpal::Stream,
    samples: rtrb::Consumer<f32>,
    /// The device this was opened on, so a host can tell whether the user's
    /// selection changed and it needs to reopen.
    device_name: String,
    sample_rate: u32,
}

impl Capture {
    /// Open an input device by name, or the default when `name` is `None`.
    ///
    /// A named device that has since been unplugged is an error rather than a
    /// silent fallback to the default: capture is something a user explicitly
    /// asked for, and quietly recording from the wrong microphone is worse than
    /// not recording.
    pub fn open(name: Option<&str>) -> Result<Self, DeviceError> {
        let host = cpal::default_host();
        let device = match name {
            Some(wanted) => host
                .input_devices()
                .map_err(|e| DeviceError(format!("could not enumerate input devices: {e}")))?
                .find(|d| readable_name(d).as_deref() == Some(wanted))
                .ok_or_else(|| DeviceError(format!("input device not found: {wanted}")))?,
            None => host
                .default_input_device()
                .ok_or_else(|| DeviceError(String::from("no default input device")))?,
        };
        let device_name = readable_name(&device).unwrap_or_else(|| String::from("unknown"));
        let supported = device
            .default_input_config()
            .map_err(|e| DeviceError(format!("no usable input config: {e}")))?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let sample_rate = config.sample_rate;
        let channels = config.channels as usize;

        let (mut producer, consumer) = rtrb::RingBuffer::<f32>::new(CAPTURE_CAPACITY);
        // Widening to stereo happens here, in the callback, rather than on the
        // host side: it is two moves per frame, and doing it later would mean
        // every consumer of `read` having to know the device's channel count.
        let mut push = move |data: &[f32]| {
            match channels {
                0 => {}
                1 => {
                    for &s in data {
                        let _ = producer.push(s);
                        let _ = producer.push(s);
                    }
                }
                2 => {
                    for &s in data {
                        let _ = producer.push(s);
                    }
                }
                n => {
                    for frame in data.chunks_exact(n) {
                        let _ = producer.push(frame[0]);
                        let _ = producer.push(frame[1]);
                    }
                }
            }
        };

        let on_error = |_| {};
        let stream = match format {
            SampleFormat::F32 => {
                device.build_input_stream(&config, move |d: &[f32], _| push(d), on_error, None)
            }
            SampleFormat::I16 => device.build_input_stream(
                &config,
                move |d: &[i16], _| {
                    let converted: Vec<f32> =
                        d.iter().map(|s| *s as f32 / i16::MAX as f32).collect();
                    push(&converted);
                },
                on_error,
                None,
            ),
            SampleFormat::U16 => device.build_input_stream(
                &config,
                move |d: &[u16], _| {
                    // Unsigned formats centre silence at half scale.
                    let converted: Vec<f32> = d
                        .iter()
                        .map(|s| (*s as f32 / u16::MAX as f32) * 2.0 - 1.0)
                        .collect();
                    push(&converted);
                },
                on_error,
                None,
            ),
            other => {
                return Err(DeviceError(format!(
                    "unsupported input sample format: {other:?}"
                )))
            }
        };
        let stream =
            stream.map_err(|e| DeviceError(format!("could not open the input stream: {e}")))?;
        stream
            .play()
            .map_err(|e| DeviceError(format!("could not start the input stream: {e}")))?;

        Ok(Self {
            _stream: stream,
            samples: consumer,
            device_name,
            sample_rate,
        })
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// The device's rate. Not necessarily the output device's — a 44.1 kHz mic
    /// feeding a 48 kHz output needs resampling, which is the caller's problem
    /// because only the caller knows whether it is monitoring (resample) or
    /// recording to disk (keep the source rate).
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Take everything captured since the last call, appending interleaved
    /// stereo samples to `out`. Returns how many samples were appended.
    pub fn read(&mut self, out: &mut Vec<f32>) -> usize {
        let mut n = 0;
        while let Ok(sample) = self.samples.pop() {
            out.push(sample);
            n += 1;
        }
        n
    }

    /// Discard everything captured so far.
    ///
    /// For a host that has been monitoring, stopped, and is starting again: the
    /// ring still holds however much audio accumulated in between, and playing
    /// it out on resume is a burst of stale sound.
    pub fn flush(&mut self) -> usize {
        let mut n = 0;
        while self.samples.pop().is_ok() {
            n += 1;
        }
        n
    }
}

/// Input device names, for the mixer's device menu.
///
/// Returns an empty list rather than an error when the host has no input
/// support at all — a machine with no microphone is an ordinary machine, not a
/// failure to report.
pub fn input_devices() -> Vec<String> {
    let host = cpal::default_host();
    match host.input_devices() {
        Ok(devices) => devices.filter_map(|d| readable_name(&d)).collect(),
        Err(_) => Vec::new(),
    }
}

/// Output device names.
pub fn output_devices() -> Vec<String> {
    let host = cpal::default_host();
    match host.output_devices() {
        Ok(devices) => devices.filter_map(|d| readable_name(&d)).collect(),
        Err(_) => Vec::new(),
    }
}

/// Name of the default output device, if there is one.
pub fn default_output_device() -> Option<String> {
    cpal::default_host()
        .default_output_device()
        .and_then(|d| readable_name(&d))
}

/// Name of the default input device, if there is one.
pub fn default_input_device() -> Option<String> {
    cpal::default_host()
        .default_input_device()
        .and_then(|d| readable_name(&d))
}

/// Resample interleaved stereo from `from` Hz to `to` Hz, appending to `out`.
///
/// Needed because a capture device rarely runs at the output device's rate, and
/// pushing 44.1 kHz samples into a 48 kHz mixer plays them about 9% flat — which
/// on a voice is immediately obvious and on a monitor sounds like a fault.
///
/// Linear, for the same reason [`crate::pcm::Pcm::sample`] is: this runs on
/// every captured frame and the material is a live monitor, not a master.
pub fn resample_stereo(input: &[f32], from: u32, to: u32, out: &mut Vec<f32>) {
    let frames = input.len() / 2;
    if frames == 0 || from == 0 || to == 0 {
        return;
    }
    if from == to {
        out.extend_from_slice(&input[..frames * 2]);
        return;
    }
    let step = from as f64 / to as f64;
    let target = ((frames as f64) / step).floor() as usize;
    out.reserve(target * 2);
    for i in 0..target {
        let pos = i as f64 * step;
        let index = pos as usize;
        let t = (pos - index as f64) as f32;
        // The last source frame has no successor to interpolate toward; holding
        // it is correct and avoids reading past the slice.
        let next = (index + 1).min(frames - 1);
        for ch in 0..2 {
            let a = input[index * 2 + ch];
            let b = input[next * 2 + ch];
            out.push(a + (b - a) * t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn a_matching_rate_copies_rather_than_resampling() {
        let input = vec![0.1, 0.2, 0.3, 0.4];
        let mut out = Vec::new();
        resample_stereo(&input, 48_000, 48_000, &mut out);
        assert_eq!(out, input);
    }

    #[test]
    fn downsampling_produces_proportionally_fewer_frames() {
        let input: Vec<f32> = (0..96).map(|i| i as f32).collect();
        let mut out = Vec::new();
        resample_stereo(&input, 48_000, 24_000, &mut out);
        assert_eq!(out.len() / 2, 24);
    }

    #[test]
    fn upsampling_produces_proportionally_more_frames() {
        let input: Vec<f32> = (0..8).map(|i| i as f32).collect();
        let mut out = Vec::new();
        resample_stereo(&input, 22_050, 44_100, &mut out);
        assert_eq!(out.len() / 2, 8);
    }

    /// The last source frame has no successor; interpolating toward one would
    /// read past the slice.
    #[test]
    fn resampling_never_reads_past_the_input() {
        let input = vec![1.0, -1.0, 2.0, -2.0];
        let mut out = Vec::new();
        resample_stereo(&input, 8_000, 44_100, &mut out);
        assert!(!out.is_empty());
        for s in &out {
            assert!(s.is_finite());
        }
    }

    #[test]
    fn degenerate_rates_and_empty_input_are_no_ops() {
        let mut out = Vec::new();
        resample_stereo(&[], 48_000, 48_000, &mut out);
        resample_stereo(&[1.0, 1.0], 0, 48_000, &mut out);
        resample_stereo(&[1.0, 1.0], 48_000, 0, &mut out);
        assert!(out.is_empty());
    }

    /// Channel values must not bleed across: left stays left.
    #[test]
    fn resampling_keeps_the_channels_separate() {
        let input = vec![1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0];
        let mut out = Vec::new();
        resample_stereo(&input, 48_000, 24_000, &mut out);
        for frame in out.chunks_exact(2) {
            assert!(frame[0] > 0.0, "left should stay positive");
            assert!(frame[1] < 0.0, "right should stay negative");
        }
    }
}
