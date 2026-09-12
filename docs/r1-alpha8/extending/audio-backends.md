# Audio backends

The engine ships an audio **API** and no audio. What makes sound is a backend
behind a Rust trait, exactly the way a scripting language is. See
[Script backends](./script-backends.md), whose shape this mirrors deliberately.

The bundled backend, `renzora_audio_backend`, is **linked into the binary**. You
do not install it and you cannot delete it: it is present whenever the engine was
built with its `audio` feature, and absent from the build entirely when it was
not. The trait it registers through is public, so a backend for a platform we
have not written one for can be installed as a plugin instead.

## Why the backend is a separate crate

Two reasons, and only the second is about size.

**Different platforms need genuinely different implementations.** The native
backend is [cpal](https://crates.io/crates/cpal) plus a mixer we wrote. A browser
backend cannot be: cpal's wasm hosts implement output but return an error from
`build_input_stream_raw`, so a browser build has no microphone at all through
that path. It wants WebAudio instead, where the graph, the panner and the
decoders come free from the browser and nothing has to be compiled into the wasm
blob. Those two share a contract, not a line of code.

**A game that makes no sound should not carry a mixer.** Build with
`renzora_runtime`'s `audio` feature off and the binary contains no device layer,
no decoders and no DSP.

### Why the bundled one is linked in rather than installed

It shipped as a loose `audio.dll` (`.so`, `.dylib`) for a while, and the size
argument above was the reason. What that form could not do is be reliably
*present*: every moving part of the API is inert without a backend, so a game
exported without the library beside it, or a player who deleted one file, got a
binary that ran perfectly and made no noise, with no error to explain it. A cargo
feature strips the mixer just as completely as a missing file did, and it cannot
go missing by accident.

## What each side owns

| | |
|---|---|
| **The engine** (`renzora_audio`) | the bus graph, the components scenes serialize, the command queue, the timeline, emitter bookkeeping, **and all file I/O** |
| **The backend** (`renzora_audio_backend`) | decoding, mixing, panning, distance attenuation, effects, the device, capture |

The split is the point: the backend speaks in handles, samples and bus keys, and
knows nothing about entities, asset paths, transforms or the editor.

### The host keeps file I/O, deliberately

A backend never opens a path. It is handed the bytes and an extension *hint*.

This is not tidiness. Exported and Android builds read assets out of an `.rpak`
archive through a loader the engine owns, so a backend calling `std::fs` would
work perfectly in the editor and fail in every shipped game — the worst possible
place for that difference to appear. It is the identical trap script backends
avoid for identical reasons.

### Capabilities are answered, not assumed

`Backend::init` returns a `Caps` bitfield: `CAPTURE`, `SPATIAL`, `FEEDS`,
`DEVICE_LIST`. The engine will not ask a backend for something it did not claim.

Claiming honestly matters. A backend that says it captures and then does nothing
produces a game that is silently wrong, which is worse than one that reports a
missing feature.

## Writing one

```rust
use bevy::prelude::*;
use renzora::audio_backend::*;

#[derive(Default)]
struct MyMixer { /* … */ }

impl Backend for MyMixer {
    fn name(&self) -> &str { "my_mixer" }

    fn init(&mut self) -> Result<BackendInfo, String> { /* open a device */ }
    fn load_clip(&mut self, clip: u64, extension: &str, bytes: &[u8]) -> Result<ClipInfo, String> { … }
    fn play(&mut self, request: &PlayRequest) -> Result<(), String> { … }
    fn update(&mut self, request: &UpdateRequest) -> UpdateReply { … }
    // everything else has a default
}

pub struct MyAudioPlugin;

impl Plugin for MyAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_backend(MyMixer::default());
    }
}

renzora::plugin!(MyAudioPlugin, Runtime);
```

Five required methods. Capture, feeds, device enumeration, bus updates and clip
unloading all have defaults, so a backend that only plays clips implements the
above and reports the capabilities it actually has.

`Backend` is `Send` but not `Sync`: a mixer usually holds a lock-free producer
that is not shareable across threads, and nothing needs it to be. The engine
reaches it through `&mut`, so there is no lock on the path.

**One backend loads.** Two scripting languages coexist because a script picks one
by its file extension; there is no equivalent for audio, and a second backend
would open the same output device and mix over the first. The engine keeps the
first registration and logs an error on the second. Since the bundled backend is
linked in, it is the one already holding that slot on any build with `audio` on:
replacing it means building with the feature off, not adding a file.

## The bundled backend

`crates/renzora_audio_backend` is the native one: cpal for the device, symphonia
for decoding, and our own mixer, spatialiser, reverb and delay. Its decoder
support is **per-project**, through cargo features for `ogg`, `wav` (on by
default), `mp3` and `flac`, so a game that only ships `.ogg` does not carry the
MP3 and FLAC decoders.

Two things worth knowing about it:

- **Everything is decoded up front.** A three-minute stereo track costs roughly
  60 MB resident as `f32`. It buys a mixer with no I/O in it — no decode thread,
  no underrun path — and it is the only shape that works unchanged on wasm, where
  there are no threads to decode on.
- **The mixer runs on the audio thread**, reached through a lock-free queue, and
  finished voices are handed *back* to be freed on the game thread. A mutex would
  have the device callback wait on a descheduled game thread, which is not a slow
  frame but an audible click.
