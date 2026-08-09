//! The engine's audio backend, implemented over this crate's mixer.
//!
//! Everything here is bookkeeping between two id spaces and one device. The
//! interesting code is in [`graph`](crate::graph) and [`device`](crate::device);
//! this module's whole job is to be the thin place where the boundary meets it.

extern crate alloc;

use std::collections::HashMap;
use std::string::String;
use std::vec::Vec;

use renzora_plugin::audio::{
    Backend, BackendInfo, BusState, Caps, CaptureInfo, ClipInfo, DeviceList, PlayRequest,
    StopRequest, StopTarget, UpdateReply, UpdateRequest,
};

use crate::capture::{self, Capture};
use crate::device::{AudioDevice, Command};
use crate::graph::{PlayParams, VoiceId};
use crate::pcm::PcmRef;
use crate::spatial::{Emitter, Listener, Rolloff};

/// The backend.
///
/// `Default` rather than a constructor because the boundary's `audio_backend!`
/// macro builds it lazily on the first call — the device is opened by
/// [`Backend::init`], not by existing.
#[derive(Default)]
pub struct RenzoraAudio {
    /// `None` until `init`, and again after `shutdown`. Every op tolerates its
    /// absence rather than asserting: the host can call `shutdown` and then take
    /// a frame to notice, and a dead device should make a call do nothing, not
    /// abort the process.
    device: Option<AudioDevice>,
    /// Decoded clips, by the handle the host assigned.
    clips: HashMap<u64, PcmRef>,
    /// Open capture streams, by host handle.
    captures: HashMap<u64, Capture>,
    /// Bus keys in the order the host last sent them. Meters are reported back
    /// in this order, and it is how a removed bus is detected: anything here and
    /// not in the new list is gone.
    buses: Vec<String>,
    /// Mirror of the *device's* bus order, which is not the host's.
    ///
    /// Meters come back by index — the shared readback is a fixed array of
    /// atomics, not a map — so something has to turn an index into a bus. It
    /// cannot be [`Self::buses`]: the mixer always has Master at index 0 whether
    /// or not the host mentioned it, so a host board that omits Master, or lists
    /// it anywhere but first, puts every meter on the wrong strip. Tracking what
    /// was actually sent is the only thing that stays true.
    device_buses: Vec<String>,
}

/// Push a whole board at a device: create every bus, then set every strip.
///
/// `order` mirrors the device's own bus list and is updated to match — see
/// [`RenzoraAudio::device_buses`] for why that has to be tracked separately.
fn send_board(device: &mut AudioDevice, buses: &[BusState], order: &mut Vec<String>) {
    for bus in buses {
        device.send(Command::AddBus(bus.key.clone()));
        device.send(Command::SetBus {
            key: bus.key.clone(),
            gain: bus.gain,
            pan: bus.pan,
            muted: bus.muted,
            soloed: bus.soloed,
        });
        // `add_bus` is idempotent and appends, so the mirror only grows for a
        // key the device did not already have.
        if !order.iter().any(|k| k == &bus.key) {
            order.push(bus.key.clone());
        }
    }
}

/// The mixer's starting bus list: Master, which always exists and is always
/// index 0.
fn alloc_master() -> Vec<String> {
    alloc::vec![String::from("Master")]
}

impl Backend for RenzoraAudio {
    const NAME: &'static str = "renzora_audio";

    fn init(&mut self) -> Result<BackendInfo, String> {
        let device = AudioDevice::open().map_err(|e| e.0)?;
        let sample_rate = device.sample_rate();
        // A fresh mixer starts with Master and nothing else.
        self.device_buses = alloc_master();
        let name = capture::default_output_device().unwrap_or_else(|| String::from("default"));
        self.device = Some(device);
        // A board described before the device opened, or carried across a
        // shutdown/init cycle, has to reach the fresh mixer — the host has no
        // reason to send it again just because we reopened.
        if !self.buses.is_empty() {
            let board: Vec<BusState> = self
                .buses
                .iter()
                .map(|key| BusState {
                    key: key.clone(),
                    // Levels are unknown here: only the keys survive, because
                    // that is all this backend keeps. The host pushes the real
                    // board on its next `SetBuses`, and unity until then is the
                    // right guess — silence would be a bug nobody could see.
                    gain: 1.0,
                    pan: 0.0,
                    muted: false,
                    soloed: false,
                })
                .collect();
            if let Some(device) = self.device.as_mut() {
                send_board(device, &board, &mut self.device_buses);
            }
        }
        // Everything is claimed because this backend does everything — the
        // capability answer earns its keep on the *other* backend, where a
        // browser cannot capture through cpal. Claiming honestly here is what
        // makes claiming honestly there meaningful.
        Ok(BackendInfo {
            sample_rate,
            caps: Caps::CAPTURE
                .union(Caps::SPATIAL)
                .union(Caps::FEEDS)
                .union(Caps::DEVICE_LIST),
            device: name,
        })
    }

    fn shutdown(&mut self) {
        // Order matters: dropping the device stops the stream, and dropping the
        // clips afterwards means nothing frees a buffer the mixer might still
        // be reading. The other order is a use-after-free that only shows up
        // under load.
        self.device = None;
        self.captures.clear();
        self.clips.clear();
        self.buses.clear();
        self.device_buses.clear();
    }

    fn set_buses(&mut self, buses: &[BusState]) {
        // The list is recorded whether or not a device is open. The host may
        // describe its board before `init`, or while a device is being replaced,
        // and forgetting it would leave the mixer with no buses once the device
        // came back — every sound routed to the master fallback with nothing to
        // say why. `init` replays whatever is here onto a fresh device.
        if let Some(device) = self.device.as_mut() {
            // Removals first, so a bus replaced by one with the same key is not
            // removed straight after being added. `add_bus` is idempotent on the
            // engine side, so re-sending an unchanged board costs nothing.
            for old in &self.buses {
                if !buses.iter().any(|b| &b.key == old) {
                    device.send(Command::RemoveBus(old.clone()));
                    // Master is never actually removed by the mixer, so it must
                    // not leave the mirror either — dropping it would shift
                    // every index by one.
                    if old != "Master" {
                        self.device_buses.retain(|k| k != old);
                    }
                }
            }
            send_board(device, buses, &mut self.device_buses);
        }
        self.buses = buses.iter().map(|b| b.key.clone()).collect();
    }

    fn load_clip(&mut self, clip: u64, extension: &str, bytes: &[u8]) -> Result<ClipInfo, String> {
        let pcm = crate::decode::decode(bytes.to_vec(), extension).map_err(|e| e.0)?;
        let info = ClipInfo {
            duration: pcm.duration(),
            sample_rate: pcm.sample_rate(),
        };
        self.clips.insert(clip, std::sync::Arc::new(pcm));
        Ok(info)
    }

    fn unload_clip(&mut self, clip: u64) {
        // Only this map's reference goes. Voices hold their own `Arc`, so a
        // sound already playing finishes rather than cutting — unloading is a
        // memory decision, not a playback one.
        self.clips.remove(&clip);
    }

    fn play(&mut self, request: &PlayRequest) -> Result<(), String> {
        let Some(source) = self.clips.get(&request.clip).cloned() else {
            return Err(std::format!("clip {} was never loaded", request.clip));
        };
        let Some(device) = self.device.as_mut() else {
            return Err(String::from("no audio device"));
        };
        device.play_as(
            VoiceId(request.voice),
            source,
            PlayParams {
                bus: request.bus.clone(),
                gain: request.gain,
                pan: request.pan,
                pitch: request.pitch,
                looping: request.looping,
                fade_in: request.fade_in,
                start: request.start,
                reverb_send: request.reverb_send,
                delay_send: request.delay_send,
                emitter: request.emitter.map(|e| Emitter {
                    position: e.position,
                    min_distance: e.min_distance,
                    max_distance: e.max_distance,
                    // Anything this build does not recognise is logarithmic —
                    // the default, and the one that sounds like distance.
                    rolloff: if e.rolloff == 1 {
                        Rolloff::Linear
                    } else {
                        Rolloff::Logarithmic
                    },
                }),
            },
        );
        Ok(())
    }

    fn stop(&mut self, request: &StopRequest) {
        let Some(device) = self.device.as_mut() else {
            return;
        };
        let command = match &request.target {
            StopTarget::Voice(id) => Command::Stop {
                id: VoiceId(*id),
                fade: request.fade,
            },
            StopTarget::Bus(key) => Command::StopBus {
                key: key.clone(),
                fade: request.fade,
            },
            StopTarget::All => Command::StopAll { fade: request.fade },
        };
        device.send(command);
    }

    fn update(&mut self, request: &UpdateRequest) -> UpdateReply {
        let Some(device) = self.device.as_mut() else {
            return UpdateReply::default();
        };
        if let Some(l) = request.listener {
            device.send(Command::SetListener(Listener {
                position: l.position,
                right: l.right,
            }));
        }
        for (voice, position) in &request.moved {
            device.send(Command::SetVoicePosition {
                id: VoiceId(*voice),
                position: *position,
            });
        }
        for (voice, gain) in &request.gains {
            device.send(Command::SetVoiceGain {
                id: VoiceId(*voice),
                gain: *gain,
            });
        }
        for (voice, pitch) in &request.pitches {
            device.send(Command::SetVoicePitch {
                id: VoiceId(*voice),
                pitch: *pitch,
            });
        }
        for (voice, paused) in &request.paused {
            device.send(Command::SetVoicePaused {
                id: VoiceId(*voice),
                paused: *paused,
            });
        }

        // Meters, reported in the host's bus order but looked up by the device's.
        // A bus the mixer has never heard of reads zero rather than reading
        // whatever happens to sit at that index.
        let peaks = self
            .buses
            .iter()
            .map(|key| {
                self.device_buses
                    .iter()
                    .position(|k| k == key)
                    .map(|i| device.peak(i))
                    .unwrap_or(0.0)
            })
            .collect();

        UpdateReply {
            peaks,
            finished: device
                .collect_finished()
                .into_iter()
                .map(|v| v.0)
                .collect(),
        }
    }

    fn open_capture(&mut self, capture: u64, device: Option<&str>) -> Result<CaptureInfo, String> {
        let stream = Capture::open(device).map_err(|e| e.0)?;
        let info = CaptureInfo {
            sample_rate: stream.sample_rate(),
            device: stream.device_name().to_string(),
        };
        self.captures.insert(capture, stream);
        Ok(info)
    }

    fn close_capture(&mut self, capture: u64) {
        self.captures.remove(&capture);
    }

    fn read_capture(&mut self, capture: u64) -> Vec<f32> {
        let mut out = Vec::new();
        if let Some(stream) = self.captures.get_mut(&capture) {
            stream.read(&mut out);
        }
        out
    }

    fn push_frames(&mut self, bus: &str, samples: &[f32]) {
        // Straight through with no resampling: the caller knows what rate its
        // samples are and whether they need converting, and a backend that
        // guessed would be wrong for exactly the case that matters — a caller
        // pushing samples it generated at the output rate on purpose.
        if let Some(device) = self.device.as_mut() {
            device.send(Command::PushFrames {
                bus: bus.to_string(),
                samples: samples.to_vec(),
            });
        }
    }

    fn list_devices(&mut self) -> DeviceList {
        DeviceList {
            inputs: capture::input_devices(),
            outputs: capture::output_devices(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Everything must tolerate a backend that has not been initialised — the
    /// host can call `shutdown` and take a frame to notice, and a dead device
    /// should make a call do nothing rather than abort the process.
    #[test]
    fn every_op_is_safe_before_init_and_after_shutdown() {
        let mut b = RenzoraAudio::default();

        b.set_buses(&[BusState {
            key: String::from("Sfx"),
            gain: 1.0,
            pan: 0.0,
            muted: false,
            soloed: false,
        }]);
        b.stop(&StopRequest {
            target: StopTarget::All,
            fade: 0.0,
        });
        b.push_frames("Sfx", &[0.0; 8]);
        assert_eq!(b.update(&UpdateRequest::default()), UpdateReply::default());
        assert!(b.read_capture(1).is_empty());
        b.close_capture(1);
        b.unload_clip(1);
        // Recorded even with no device, so it survives to reach one later.
        assert_eq!(b.buses, ["Sfx"]);
        b.shutdown();
        assert!(b.buses.is_empty());
    }

    /// A clip handle the host never loaded is an error with a message, not a
    /// silent no-op — the host needs to know an asset went missing.
    #[test]
    fn playing_an_unloaded_clip_says_so() {
        let mut b = RenzoraAudio::default();
        let err = b
            .play(&PlayRequest {
                voice: 1,
                clip: 42,
                bus: String::from("Sfx"),
                gain: 1.0,
                pan: 0.0,
                pitch: 1.0,
                looping: None,
                fade_in: 0.0,
                start: 0.0,
                emitter: None,
                reverb_send: 0.0,
                delay_send: 0.0,
            })
            .unwrap_err();
        assert!(err.contains("42"), "{err}");
    }

    /// Decoding does not need a device, so this works headless — which is what
    /// makes it testable in CI at all.
    #[test]
    fn a_clip_decodes_and_reports_its_duration() {
        let mut b = RenzoraAudio::default();
        // 8 frames of 16-bit stereo silence at 8 kHz = 1 ms.
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36u32 + 32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&8_000u32.to_le_bytes());
        wav.extend_from_slice(&32_000u32.to_le_bytes());
        wav.extend_from_slice(&4u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&32u32.to_le_bytes());
        wav.extend_from_slice(&[0u8; 32]);

        let info = b.load_clip(7, "wav", &wav).expect("should decode");
        assert_eq!(info.sample_rate, 8_000);
        assert!((info.duration - 0.001).abs() < 1e-6, "{}", info.duration);
        assert!(b.clips.contains_key(&7));

        b.unload_clip(7);
        assert!(!b.clips.contains_key(&7));
    }

    #[test]
    fn undecodable_bytes_are_an_error_rather_than_a_panic() {
        let mut b = RenzoraAudio::default();
        assert!(b.load_clip(1, "wav", &[0u8; 32]).is_err());
        assert!(b.load_clip(1, "", &[]).is_err());
    }

    /// The bus list is what turns a meter index back into a bus, so it has to
    /// track exactly what the host last sent.
    #[test]
    fn the_bus_list_follows_the_host() {
        let mut b = RenzoraAudio::default();
        let bus = |key: &str| BusState {
            key: String::from(key),
            gain: 1.0,
            pan: 0.0,
            muted: false,
            soloed: false,
        };
        b.set_buses(&[bus("Sfx"), bus("Music")]);
        assert_eq!(b.buses, ["Sfx", "Music"]);

        b.set_buses(&[bus("Music")]);
        assert_eq!(b.buses, ["Music"]);
    }

    /// The device always has Master at index 0, whether or not the host
    /// mentioned it. A host board that omits Master — or lists it anywhere but
    /// first — would otherwise put every meter on the wrong strip.
    #[test]
    fn the_device_bus_mirror_keeps_master_at_index_zero() {
        let mut b = RenzoraAudio::default();
        b.device_buses = alloc_master();
        let bus = |key: &str| BusState {
            key: String::from(key),
            gain: 1.0,
            pan: 0.0,
            muted: false,
            soloed: false,
        };

        // A host board that never mentions Master.
        let mut order = b.device_buses.clone();
        let board = [bus("Sfx"), bus("Music")];
        for b in &board {
            if !order.iter().any(|k| k == &b.key) {
                order.push(b.key.clone());
            }
        }
        assert_eq!(order, ["Master", "Sfx", "Music"]);
        assert_eq!(order.iter().position(|k| k == "Sfx"), Some(1));

        // And one that lists Master last: it must not move.
        let mut order = alloc_master();
        for key in ["Sfx", "Master"] {
            if !order.iter().any(|k| k == key) {
                order.push(String::from(key));
            }
        }
        assert_eq!(order, ["Master", "Sfx"]);
    }
}
