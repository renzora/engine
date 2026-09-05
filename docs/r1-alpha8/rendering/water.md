# Water & Oceans

Renzora's water is a **spectral FFT ocean**, not a stack of hand-authored sine
waves. You describe a *sea state* — how hard the wind blows, how far it has
blown, how deep the water is — and the wave shapes follow from it. Whitecaps
appear where waves actually break, because the simulation knows where the
surface has folded over itself.

The technique is ported from
[GodotOceanWaves](https://github.com/2Retr0/GodotOceanWaves) (MIT), which
implements Tessendorf's *Simulating Ocean Water* with Horvath's directional
spectra and the water lighting model from the 2019 GDC talk *"Wakes, Explosions
and Lighting: Interactive Water Simulation in Atlas"*.

## Quick start

1. Create an empty entity and add the **Water Surface** component to it from the
   inspector (Rendering category). The mesh and material are generated for you.
2. Move the entity **up or down** to set the water level. In `Clipmap` mode its
   XZ position is driven by the camera, so only the height is yours to set.
3. Tune the sea in the inspector — start with **Cascade 1 Wind Speed** and
   **Cascade 1 Fetch**. Everything updates live.

The defaults are the reference project's three-cascade open ocean: a long swell
(88 m tile), a mid-scale wind sea (57 m), and a normals-only detail cascade
(16 m).

## How it works

Each frame the GPU:

1. Builds a **JONSWAP/TMA spectrum** with Hasselmann directional spreading —
   the statistical description of a sea driven by a given wind over a given
   fetch, in a given depth of water. This step only re-runs when a parameter
   changes.
2. **Propagates** that spectrum in time with the dispersion relation.
3. **Inverse-Fourier-transforms** it into a displacement map and a
   normal + foam map, using a Stockham FFT in compute shaders.
4. Grows **foam** wherever the Jacobian of the horizontal displacement goes
   negative — the mathematical signature of a wave breaking — and decays it
   everywhere else.

The water material then displaces its vertices by the displacement map and
shades using the normals, with foam mixed in on top.

### Cascades

One FFT tiles: run a single simulation over a 100 m square and you will see that
square repeat. Cascades fix this by layering several simulations with different
tile lengths, so their repeats never coincide within sight.

Each cascade is a full sea state of its own. Typical layering:

| Cascade | Tile | Role |
|---|---|---|
| 1 | 80–200 m | Swell. Carries most of the displacement. |
| 2 | 40–60 m | Wind sea / chop. |
| 3 | 10–20 m | Detail. Usually `Displacement Scale 0` — normals only. |

Setting a cascade's **Displacement Scale** to 0 keeps its detail in the shading
normal while contributing nothing to the geometry. That is the right call for
small tiles, where per-vertex displacement is below the mesh's resolution anyway
and only produces shimmer.

Note that foam is decided *before* `Displacement Scale` is applied — the Jacobian
test runs on the raw simulation output. A cascade with `Displacement Scale 0` and
a non-zero `Foam Amount` therefore still produces whitecaps, which is exactly how
the detail cascade contributes fine foam without adding geometry. The flip side
is that lowering a cascade's displacement does not reduce its foam; lower its
`Foam Amount` or raise its `Whitecap` instead.

## Parameters

### Appearance

| Field | Meaning |
|---|---|
| Water Color | Body colour of the water, **linear**. Deep water is much darker than it looks in an sRGB picker — the default is `(0.010, 0.020, 0.027)`, the linear form of sRGB `(0.1, 0.15, 0.18)`. Setting a "reasonable-looking" sRGB value here makes the ocean milky. |
| Foam Color | Colour of accumulated foam, also linear. |
| Roughness | Feeds the microfacet specular and the fresnel curve. 0.65 is a good open-ocean value. |
| Normal Strength | Global multiplier on the shading normal. 0 shades the surface flat without touching the geometry. |

### Sea state

| Field | Meaning |
|---|---|
| Sea Depth | Water depth in metres. Shallow water shortens and steepens waves (Kitaigorodskii attenuation). |
| Seed | Re-rolls the spectrum's random phases — a different sea with the same statistics. Each cascade derives its own seed from this one, so they stay independent (see below). |
| Cascades | How many cascades are live (1–8). |

Each cascade mixes its **index** into the seed. That matters: the random phases
come from a hash of the frequency-grid coordinate, so cascades sharing one seed
would draw the *identical* Gaussian field — different tile lengths and
amplitudes, but the same realization rescaled. Their crests would line up, which
is precisely the beat that varying the tile lengths is there to avoid.

### Per cascade

The inspector edits **one cascade at a time**. The `Cascade` field picks which;
every field below it reads and writes that cascade. `Cascades` (above) sets how
many exist.

| Field | Meaning |
|---|---|
| Tile Length X / Y | Size of this cascade's repeating tile, in metres. The dominant knob. Unequal axes stretch the sea along one direction. |
| Time Scale | How fast this cascade's clock runs relative to real time. 1 is real time, 0 freezes it, 2 doubles it. |
| Wind Speed | Average wind over the water, m/s. Steeper, choppier as it rises. |
| Wind Direction (deg) | Heading, in degrees. |
| Fetch Length (km) | Distance the wind has blown over. Longer fetch = steeper but more ordered waves. |
| Swell | Biases toward long, ordered waves travelling with the wind. |
| Detail | Attenuates high frequencies. Lower it if a cascade shimmers. |
| Spread | 0 follows the wind tightly; 1 is fully isotropic. |
| Whitecap | How steep a wave must get before foam accumulates. Lower = foam sooner. |
| Foam Amount | How much foam, and how long it lingers. Wispy foam = high Foam Amount, low Whitecap. |
| Displacement Scale | Contribution to vertex displacement. Reduce as you add cascades. |
| Normal Scale | Contribution to the shading normal. |

### Following the world wind

By default a surface scales its whole sea state with the scene's
[wind](wind.md), via **Follow World Wind** and **Wind Response** (how much of
the wind reaches this surface — a sheltered bay is ~0.4, open ocean 1.0).

The per-cascade values above become the sea's *shape* — the swell-to-wind-sea
balance and the relative bearings — and the world wind scales and rotates that
set as a whole. Turn the wind up and the same ocean gets harsher without
becoming a different ocean. Turn Follow World Wind off and the sea you authored
comes straight back.

It follows a heavily smoothed wind, so the sea takes tens of seconds to build
after the dial moves. That lag is deliberate twice over: Wind Speed is a
spectrum input, so every change re-bakes the cascade textures, and a real sea
takes hours to build to a new wind. Watching the swell arrive after the gust is
the effect working, not lag.

**Time Scale is not a wind knob.** It leaves the spectrum untouched and rescales
only the `exp(i·ω·t)` propagation, so the waves keep their shape and simply
travel faster or slower. Running a long-swell cascade slower than the wind sea
on top of it is the usual way to sell scale — real ocean swell moves far more
slowly than its wavelength leads the eye to expect. Because it does not touch
the spectrum, dragging it does not rebuild anything.

### Performance

| Field | Meaning |
|---|---|
| Wave Resolution | FFT grid per cascade: 128, 256, 512 or 1024. Cost scales with the **square**, so this is the main dial. 512 is the default; 256 halves the cost but visibly smooths the chop, and 1024 is what the reference project ships. |
| Updates / Second | How often the simulation advances (default 50; 0 = every frame). Lowering it cuts GPU time without changing how the waves move. |
| Enable Sea Spray | Emit spray particles where waves break. |

Memory is roughly `cascades × resolution² × 26 bytes`: about 5 MB per cascade at
256, 20 MB per cascade at 512.

## Mesh modes

**Clipmap** (default) — a dense block under the camera surrounded by rings of
progressively larger quads, re-centred on the camera every frame. This is what
makes the ocean reach the horizon at a near-constant on-screen triangle density.

`Wave Mesh Quality` picks the density:

| Quality | Centre quad | Rings | Reach |
|---|---|---|---|
| Low | 0.5 m | 4 | ±512 m |
| Medium | 0.35 m | 5 | ±896 m |
| High (default) | 0.25 m | 5 | ±1024 m |
| Custom | uses the three `Clipmap *` fields below | | |

High is roughly 620k triangles, deliberately in the same range as the reference
project's clipmap. Under **Custom**, `Clipmap Rings` doubles the extent each
time, `Clipmap Resolution` sets the quads per ring edge, and `Clipmap Quad Size`
sets the finest quad; switching to Custom seeds those three from whichever
preset was active, so it starts from what was on screen.

Mesh density matters as much as `Wave Resolution`: the cascade maps hold detail
down to a few centimetres, and geometry coarser than that simply cannot express
it. A metre-scale clipmap turns a choppy sea into smooth hills no matter how
good the simulation is. If the water is costing too much, step `Wave Mesh
Quality` down before dropping `Wave Resolution`.

The mesh follows the camera **snapped to the finest quad size**, and wave UVs
come from world position, so the waves stay still in the world while the mesh
slides under them.

**Grid** — a fixed `Grid Size` plane with `Grid Subdivisions` quads per edge,
centred on the entity. Use this for lakes, ponds, rivers and anything with a
shoreline to hide the edge. Presets that are not open ocean (Calm Lake, River,
Swamp) select it automatically.

Displacement, shading normals and foam all fade with distance, because far
vertices sit on coarse rings where displacing them produces aliasing that crawls
as the mesh slides under the camera. **Those fade distances scale with the
water's extent**: the reference's constants (fade displacement past 150 m,
flatten normals at 0.0175/m) are tuned for its fixed ±256 m ocean, and copying
them literally onto a kilometre-wide one flattens everything past a quarter of
the view into a dead plane. Water up to ±256 m uses the reference's values;
beyond that they stretch, up to 3×.

## Buoyancy

Add the **Buoyant** component to a rigid body and it floats, riding the waves
rather than a flat plane.

Because the simulation lives in VRAM, buoyancy does not read it back. Instead
the engine recomputes the same spectrum on the CPU at 64×64 per cascade, ~20
times a second, hashing the random phases by the *same* frequency-grid
coordinates the GPU uses. The CPU surface is therefore the GPU surface
low-pass-filtered — the swell matches exactly, the chop is missing. Buoyancy
does not care about the chop, and this is the only approach that also works on a
headless server (`--server`), which has no GPU at all.

| Field | Meaning |
|---|---|
| Force | Upward force when submerged. |
| Damping | Extra vertical damping — settles bobbing. |
| Submerge Depth | Depth at which buoyancy reaches full strength. |
| Wave Push | How strongly wave slopes carry the body along. |
| Drag | Resistance to motion through the water. |

The CPU field is only computed while at least one `Buoyant` entity exists.

## Presets

`Ocean`, `Stormy Ocean`, `Tropical`, `Arctic`, `Calm Lake`, `River` and `Swamp`
are expressed as cascade parameter sets — a storm is high wind, long fetch and a
low whitecap threshold, a lake is low wind with almost no fetch.

## Limits

- **One sea state per scene.** The cascade maps are global, so the *first*
  `WaterSurface` entity drives the simulation. Additional water entities render
  the same waves (with their own colours and mesh). Two genuinely different
  oceans would need a second full set of maps.
- **Water renders in the transparent phase** and does not write depth. An opaque
  material is also drawn into the depth prepass, and the prepass would use
  Bevy's stock vertex shader — the flat, undisplaced plane — so every wave would
  be depth-rejected. Fixing that needs a displaced prepass vertex shader.
- **No screen-space refraction, caustics or shoreline foam.** This is a faithful
  port of an open-ocean shader; those effects belonged to the previous Gerstner
  water and were not part of it.
- Resolution 1024 sits exactly on WebGPU's 16 KiB workgroup-memory limit. It
  works, but it is the ceiling — there is no 2048.
