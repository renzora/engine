# Audio backends

The engine ships an audio **API** and no audio. What makes sound is a separate
C-ABI plugin, exactly the way a scripting language is — see
[Script backends](./script-backends.md), whose shape this mirrors deliberately.

Drop `audio.dll` (`.so`, `.dylib`) into `plugins/` and the game plays. Delete it
and the same binary runs silent: the mixer panel still shows a board, every
`play_sound()` still resolves, nothing panics. That is what "removable" means
here — not a feature flag, a file.

## Why audio is a plugin and not part of the engine

Two reasons, and only the second is about size.

**Different platforms need genuinely different implementations.** The native
backend is [cpal](https://crates.io/crates/cpal) plus a mixer we wrote. A browser
backend cannot be: cpal's wasm hosts implement output but return an error from
`build_input_stream_raw`, so a browser build has no microphone at all through
that path. It wants WebAudio instead, where the graph, the panner and the
decoders come free from the browser and nothing has to be compiled into the wasm
blob. Those two share a contract, not a line of code.

**A game that makes no sound should not carry a mixer.** With the plugin absent,
the binary contains no device layer, no decoders and no DSP.

## What each side owns

| | |
|---|---|
| **The engine** (`renzora_audio`) | the bus graph, the components scenes serialize, the command queue, the timeline, emitter bookkeeping, **and all file I/O** |
| **The backend** (`plugins/audio`) | decoding, mixing, panning, distance attenuation, effects, the device, capture |

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
use renzora_plugin::audio::*;
use renzora_plugin::prelude::*;

#[derive(Default)]
struct MyMixer { /* … */ }

impl Backend for MyMixer {
    const NAME: &'static str = "my_mixer";

    fn init(&mut self) -> Result<BackendInfo, String> { /* open a device */ }
    fn load_clip(&mut self, clip: u64, ext: &str, bytes: &[u8]) -> Result<ClipInfo, String> { … }
    fn play(&mut self, request: &PlayRequest) -> Result<(), String> { … }
    fn update(&mut self, request: &UpdateRequest) -> UpdateReply { … }
    // everything else has a default
}

renzora_plugin::audio_backend!(MyMixer);

pub struct MyAudioPlugin;
impl Plugin for MyAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_backend(audio_backend::desc());
    }
}
renzora_plugin::add!(MyAudioPlugin);
```

Four required methods. Capture, feeds, device enumeration and clip unloading all
have defaults, so a backend that only plays clips implements the four above and
reports the capabilities it actually has.

**One backend loads.** Two scripting languages coexist because a script picks one
by its file extension; there is no equivalent for audio, and a second backend
would open the same output device and mix over the first. The host keeps the
first registration and logs the second.

## Ops, not one function pointer per operation

A backend registers a single `extern "C"` entry point, and the operation is
selected by an `AudioOp` code. A struct of thirteen named function pointers would
make adding a fourteenth an ABI break — every prebuilt backend would need
rebuilding to add, say, a new send.

With an op code, a backend that does not recognise one returns
`AudioStatus::UnknownOp`, and the engine treats that exactly as it treats a
capability that backend never claimed. Appending an op is a `VERSION_MINOR` bump
and nothing stops working.

## The bundled backend

`plugins/audio` is the native one: cpal for the device, symphonia for decoding, and
our own mixer, spatialiser, reverb and delay. Its decoder support is **per-project**
— cargo features for `ogg`, `wav` (on by default), `mp3` and `flac` — so a game
that only ships `.ogg` does not carry the MP3 and FLAC decoders.

Two things worth knowing about it:

- **Everything is decoded up front.** A three-minute stereo track costs roughly
  60 MB resident as `f32`. It buys a mixer with no I/O in it — no decode thread,
  no underrun path — and it is the only shape that works unchanged on wasm, where
  there are no threads to decode on.
- **The mixer runs on the audio thread**, reached through a lock-free queue, and
  finished voices are handed *back* to be freed on the game thread. A mutex would
  have the device callback wait on a descheduled game thread, which is not a slow
  frame but an audible click.
