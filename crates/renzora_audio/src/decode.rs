//! Decoding audio to PCM, for looking at rather than listening to.
//!
//! Behind the `decode` feature and off by default, because the runtime does not
//! want it: playback decoding happens in the audio *backend*, on the far side of
//! the plugin boundary, and the whole point of that arrangement is that the
//! shipped binary carries no decoders.
//!
//! What still needs one on this side is the editor — the DAW's waveform
//! overview and the marketplace preview's spectrogram both draw a picture of a
//! file the mixer may never play. Those crates are editor-only and stripped from
//! exports, so the cost lands where it is used.
//!
//! Two decoders in the tree is a real duplication, and it is the right one: the
//! alternative is either symphonia in every shipped game, or an op that ships
//! whole decoded files back across the boundary to draw a thumbnail with.

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Interleaved stereo samples plus the rate they were decoded at.
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl DecodedAudio {
    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }

    pub fn duration(&self) -> f64 {
        if self.sample_rate == 0 {
            return 0.0;
        }
        self.frames() as f64 / self.sample_rate as f64
    }

    /// The frame at `index`, or silence past the end.
    pub fn frame(&self, index: usize) -> [f32; 2] {
        match self.samples.get(index * 2..index * 2 + 2) {
            Some([l, r]) => [*l, *r],
            _ => [0.0, 0.0],
        }
    }
}

/// Decode a whole file to interleaved stereo.
///
/// `extension` is a hint only — the bytes are probed and win, so a mislabelled
/// asset draws a waveform instead of nothing.
pub fn decode(bytes: Vec<u8>, extension: &str) -> Result<DecodedAudio, String> {
    if bytes.is_empty() {
        return Err(String::from("empty file"));
    }
    let stream = MediaSourceStream::new(
        Box::new(std::io::Cursor::new(bytes)),
        Default::default(),
    );
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
                enable_gapless: true,
                ..Default::default()
            },
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("unrecognised audio format: {e}"))?;

    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| String::from("file contains no audio track"))?;
    let track_id = track.id;
    let params = track.codec_params.clone();

    let mut decoder = symphonia::default::get_codecs()
        .make(&params, &DecoderOptions::default())
        .map_err(|e| format!("no decoder for this codec: {e}"))?;

    let mut sample_rate = params.sample_rate.unwrap_or(0);
    let mut samples: Vec<f32> = Vec::new();
    let mut buffer: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            // Symphonia reports a clean end of stream as an io error, so this is
            // the normal exit rather than a failure.
            Err(SymphoniaError::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break
            }
            Err(SymphoniaError::ResetRequired) => break,
            Err(e) => return Err(format!("read error: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            // A corrupt packet costs that packet, not the whole picture.
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(format!("decode error: {e}")),
        };
        let spec = *decoded.spec();
        if sample_rate == 0 {
            sample_rate = spec.rate;
        }
        let buf = buffer.get_or_insert_with(|| {
            SampleBuffer::<f32>::new(decoded.capacity() as u64, spec)
        });
        buf.copy_interleaved_ref(decoded);
        append_stereo(&mut samples, buf.samples(), spec.channels.count());
    }

    if sample_rate == 0 || samples.is_empty() {
        return Err(String::from("stream decoded to no audio"));
    }
    Ok(DecodedAudio {
        samples,
        sample_rate,
    })
}

/// Fold `channels`-wide interleaved samples onto stereo.
fn append_stereo(out: &mut Vec<f32>, samples: &[f32], channels: usize) {
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

    #[test]
    fn garbage_is_an_error_rather_than_a_panic() {
        assert!(decode(vec![0u8; 64], "wav").is_err());
        assert!(decode(Vec::new(), "wav").is_err());
    }

    #[test]
    fn a_zero_channel_stream_appends_nothing() {
        let mut out = Vec::new();
        append_stereo(&mut out, &[1.0, 2.0], 0);
        assert!(out.is_empty());
    }

    #[test]
    fn wider_than_stereo_keeps_the_first_pair() {
        let mut out = Vec::new();
        append_stereo(&mut out, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 6);
        assert_eq!(out, vec![1.0, 2.0]);
    }

    #[test]
    fn reading_past_the_end_gives_silence() {
        let audio = DecodedAudio {
            samples: vec![1.0, -1.0],
            sample_rate: 48_000,
        };
        assert_eq!(audio.frame(0), [1.0, -1.0]);
        assert_eq!(audio.frame(99), [0.0, 0.0]);
    }
}
