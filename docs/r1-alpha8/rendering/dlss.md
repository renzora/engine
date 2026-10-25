# DLSS

NVIDIA DLSS Ray Reconstruction denoises [Solari](./solari.md)'s ray-traced
lighting and upscales the frame in one pass. Renzora uses it wherever it can
run, and uses its own [denoiser](./solari.md#denoiser) everywhere else, so a
project looks right on every GPU that can ray trace.

## In the editor

On the **World Environment**, the **Solari Ray-Traced GI** component has:

| Setting | What it does |
|---|---|
| **DLSS** | Use DLSS Ray Reconstruction where it is available. On by default. |
| **DLSS Mode** | How far below the output resolution Solari renders before DLSS reconstructs it: **Auto**, **DLAA** (native resolution, no upscaling), **Quality** (about 67%), **Balanced** (about 58%), **Performance** (50%) or **Ultra Performance** (about 33%). |
| **Denoising With** | Which denoiser is actually running, and why when it is not DLSS. |

Turning **DLSS** off, or running where it is unavailable, hands the image back
to the built-in denoiser on the next frame. TAA is taken off a camera while DLSS
runs on it, since DLSS anti-aliases the image itself, and comes back when DLSS
stops.

## When DLSS runs

All of these, or the built-in denoiser runs instead:

- the engine was **built with the DLSS SDK** (below);
- an **NVIDIA RTX** GPU with a current driver;
- the **Vulkan** renderer (the default; DX12 has no DLSS in this engine);
- `nvngx_dlss.dll` and `nvngx_dlssd.dll` **beside the executable**
  (`cargo renzora` puts them there).

## Building with DLSS

DLSS is NVIDIA's proprietary SDK, which cannot be shipped in this repository,
so a build includes it only where the SDK is installed. `cargo renzora` checks
on every build and turns DLSS on when it finds everything; otherwise it prints
why and builds without it.

What it needs:

1. **The DLSS SDK, release 310.9.1** exactly. Clone
   [NVIDIA/DLSS at v310.9.1](https://github.com/NVIDIA/DLSS/tree/v310.9.1) and
   set the `DLSS_SDK` environment variable to its absolute path. A different
   release is detected and refused, since its libraries do not match the
   bindings the engine is built against.
2. **The Vulkan SDK**, with `VULKAN_SDK` set (the LunarG installer sets it).
3. **clang** (LLVM), which generates the bindings to the SDK.

Then build as usual:

```sh
cargo renzora dist
```

The build prints `DLSS: on (<path>)` or the reason it is off. It also copies
the two DLSS libraries and NVIDIA's licence (`DLSS_LICENSE.txt`) into
`dist/<platform>/`. Set `RENZORA_DLSS=0` to build without DLSS on a machine that
has the SDK.

The first build after DLSS switches on or off recompiles Bevy, and installed
native plugins rebuild against it on their next load.

To build the feature by hand instead, pass `--features dlss` to cargo with the
same environment.

## Shipping a game with DLSS

Export takes care of it for a game that uses Solari (its **Raytraced Lighting
(Solari)** feature is on), on Windows x64 and Linux x64, when the editor was
built with DLSS:

- **Copy-based exports** ship the editor build's own executable, which already
  has DLSS, and copy `nvngx_dlss.dll`, `nvngx_dlssd.dll` and
  `DLSS_LICENSE.txt` beside it.
- **Lean exports** compile DLSS into the single binary, which needs the same
  `DLSS_SDK` and `VULKAN_SDK` as `cargo renzora` in the editor's environment,
  and copy the same three files beside it. The libraries cannot go inside the
  binary: they hold DLSS's models, and NVIDIA's runtime loads them from beside
  the executable.

A game without Solari never carries NVIDIA's files. A game whose DLSS files go
missing still runs, on the built-in denoiser.

NVIDIA's terms also ask for the copyright and licence text from section 9.5 of
the DLSS Programming Guide (`$DLSS_SDK/doc/DLSS_Programming_Guide_Release.pdf`)
to be included with the game.
