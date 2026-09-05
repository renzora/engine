//! Encoded bytes in, [`Pcm`] out.
//!
//! The engine hands over the whole file as bytes and this turns them into
//! interleaved stereo `f32` at the source's own rate. It never opens a file —
//! see the crate doc for why that rule is load-bearing rather than tidy.
//!
//! Everything is decoded in full up front rather than streamed. That is a real
//! trade: a three-minute stereo Vorbis track costs about 60 MB resident as
//! `f32`. It buys a mixer with no I/O in it at all — no decode thread, no
//! underrun path, no partial-buffer state machine in the render loop — and it is
//! the only shape that works unchanged on wasm, where there are no threads to
//! decode on. Streaming, if it earns its place later, belongs behind this same
//! call as a second [`Source`] variant rather than as a change to the mixer.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::pcm::Pcm;

/// Why a clip could not be decoded.
///
/// One string rather than an enum of causes: every one of these ends the same
/// way — the host logs it against the asset path and the sound does not play —
/// and a caller that cannot act differently on the variants does not need them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError(pub String);

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Decode a whole encoded file into stereo PCM.
///
/// `extension` is a *hint* only — symphonia probes the bytes and will happily
/// decode an `.ogg` that is really a WAV. Pass what the asset path said and let
/// the prober disagree; the alternative is trusting a file extension, which is
/// how a mislabelled asset turns into a silent failure nobody can explain.
pub fn decode(bytes: Vec<u8>, extension: &str) -> Result<Pcm, DecodeError> {
    if bytes.is_empty() {
        return Err(DecodeError(String::from("empty file")));
    }

    let stream = MediaSourceStream::new(alloc::boxed::Box::new(std::io::Cursor::new(bytes)), Default::default());
    let mut hint = Hint::new();
    let ext = extension.trim_start_matches('.');
    if !ext.is_empty() {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions {
                // The mixer needs a frame count to seek and loop against, and a
                // Vorbis stream only reveals its length from the seek index.
                enable_gapless: true,
                ..Default::default()
            },
            &MetadataOptions::default(),
        )
        .map_err(|e| DecodeError(format!("unrecognised audio format: {e}")))?;

    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| DecodeError(String::from("file contains no audio track")))?;
    let track_id = track.id;
    let params = track.codec_params.clone();

    let mut decoder = symphonia::default::get_codecs()
        .make(&params, &DecoderOptions::default())
        .map_err(|e| DecodeError(format!("no decoder for this codec: {e}")))?;

    // The codec usually knows the rate up front; when it doesn't, the first
    // decoded packet does. Zero here would make every voice's playback rate a
    // division by zero, so it is checked once at the end rather than trusted.
    let mut sample_rate = params.sample_rate.unwrap_or(0);
    let mut samples: Vec<f32> = Vec::with_capacity(
        params
            .n_frames
            .map(|f| (f as usize).saturating_mul(2))
            .unwrap_or(0),
    );
    let mut buffer: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            // Symphonia signals a clean end of stream as an io error rather than
            // a variant of its own, so this is the normal exit, not a failure.
            Err(SymphoniaError::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break
            }
            Err(SymphoniaError::ResetRequired) => break,
            Err(e) => return Err(DecodeError(format!("read error: {e}"))),
        };
        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            // A corrupt packet mid-file should cost that packet, not the clip.
            // Games ship assets that were converted by something careless more
            // often than they ship silence on purpose.
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(DecodeError(format!("decode error: {e}"))),
        };

        let spec = *decoded.spec();
        if sample_rate == 0 {
            sample_rate = spec.rate;
        }
        let buf = buffer.get_or_insert_with(|| {
            SampleBuffer::<f32>::new(decoded.capacity() as u64, spec)
        });
        buf.copy_interleaved_ref(decoded);
        append_as_stereo(&mut samples, buf.samples(), spec.channels.count());
    }

    if sample_rate == 0 {
        return Err(DecodeError(String::from("stream declares no sample rate")));
    }
    if samples.is_empty() {
        return Err(DecodeError(String::from("stream decoded to no audio")));
    }
    Ok(Pcm::stereo(samples, sample_rate))
}

/// Fold `channels`-wide interleaved samples onto stereo, appending to `out`.
///
/// Mono widens. Stereo passes through. Anything wider keeps the first two
/// channels and discards the rest — which is the right call for game assets: a
/// 5.1 source in a project is almost always a mistake, and downmixing it
/// properly needs the channel *layout*, not just the count, to know which pair
/// is front-left/right.
fn append_as_stereo(out: &mut Vec<f32>, samples: &[f32], channels: usize) {
    match channels {
        0 => {}
        1 => {
            out.reserve(samples.len() * 2);
            for &s in samples {
                out.push(s);
                out.push(s);
            }
        }
        2 => out.extend_from_slice(samples),
        n => {
            out.reserve(samples.len() / n * 2);
            for frame in samples.chunks_exact(n) {
                out.push(frame[0]);
                out.push(frame[1]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// A minimal 16-bit PCM WAV, built by hand so the decoder is tested against
    /// bytes rather than against another part of this crate.
    fn wav(sample_rate: u32, channels: u16, frames: &[i16]) -> Vec<u8> {
        let data_len = (frames.len() * 2) as u32;
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36 + data_len).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
        v.extend_from_slice(&1u16.to_le_bytes()); // PCM
        v.extend_from_slice(&channels.to_le_bytes());
        v.extend_from_slice(&sample_rate.to_le_bytes());
        let block_align = channels * 2;
        v.extend_from_slice(&(sample_rate * block_align as u32).to_le_bytes());
        v.extend_from_slice(&block_align.to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
        v.extend_from_slice(b"data");
        v.extend_from_slice(&data_len.to_le_bytes());
        for s in frames {
            v.extend_from_slice(&s.to_le_bytes());
        }
        v
    }

    #[test]
    fn a_stereo_wav_round_trips_to_pcm() {
        let bytes = wav(44_100, 2, &[i16::MAX, i16::MIN, 0, 0]);
        let pcm = decode(bytes, "wav").expect("should decode");
        assert_eq!(pcm.sample_rate(), 44_100);
        assert_eq!(pcm.frames(), 2);
        let [l, r] = pcm.sample(0.0);
        assert!(l > 0.99, "{l}");
        assert!(r < -0.99, "{r}");
    }

    #[test]
    fn a_mono_wav_is_widened_to_both_channels() {
        let bytes = wav(22_050, 1, &[i16::MAX, 0]);
        let pcm = decode(bytes, "wav").expect("should decode");
        assert_eq!(pcm.sample_rate(), 22_050);
        assert_eq!(pcm.frames(), 2);
        let [l, r] = pcm.sample(0.0);
        assert_eq!(l, r);
    }

    /// The extension is a hint, not a promise — a mislabelled asset should play,
    /// not fail silently.
    #[test]
    fn the_bytes_win_when_the_extension_lies() {
        let bytes = wav(48_000, 2, &[0, 0, 0, 0]);
        assert!(decode(bytes.clone(), "ogg").is_ok());
        assert!(decode(bytes, "").is_ok());
    }

    #[test]
    fn garbage_is_an_error_rather_than_a_panic() {
        assert!(decode(vec![0u8; 64], "wav").is_err());
        assert!(decode(Vec::new(), "wav").is_err());
    }

    #[test]
    fn wider_than_stereo_keeps_the_first_pair() {
        let mut out = Vec::new();
        append_as_stereo(&mut out, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 6);
        assert_eq!(out, vec![1.0, 2.0]);
    }

    /// A zero-channel spec would divide by zero in `chunks_exact`.
    #[test]
    fn a_zero_channel_stream_appends_nothing() {
        let mut out = Vec::new();
        append_as_stereo(&mut out, &[1.0, 2.0], 0);
        assert!(out.is_empty());
    }
}
