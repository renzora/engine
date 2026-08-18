# Volumetric Clouds

Renzora's clouds are **raymarched through a real volume**, not painted onto a
sphere. Every pixel of sky walks a ray through a cloud deck wrapped around a
virtual planet, and every sample along that ray takes a second, shorter walk
toward the sun to find out how much light reaches it. That second march is why
clouds have bright sunward faces and dark undersides, why one cloud shadows the
one behind it, and why the whole deck curves down and compresses into the
horizon on its own.

The model is the one from *"The Real-Time Volumetric Cloudscapes of Horizon Zero
Dawn"* (Andrew Schneider and Nathan Vos, Guerrilla Games), with the scattering
integration and dual-lobe phase function from *"Physically Based Sky, Atmosphere
and Cloud Rendering in Frostbite"* (Sébastien Hillaire). The implementation
follows [bevy-volumetric-clouds](https://github.com/evroon/bevy-volumetric-clouds)
(MIT) by Erik Vroon.

## Quick start

1. Select an entity — usually the same one carrying the scene's environment
   settings — and add the **Clouds** component from the inspector (Rendering
   category).
2. Drag **Coverage** to decide how much sky is cloud, from clear to overcast.
3. Drag **Scale** to decide how big the formations are: *lower* means larger,
   broader weather systems.

Everything else updates live. The defaults are a fair-weather cumulus deck from
2200 m to 4200 m, drifting at 40 m/s — cloud features are kilometres across, so
wind has to be weather-system fast before the sky reads as moving at all.

The clouds pick up the scene's sun automatically — direction, colour,
elevation and illuminance all come off the brightest `DirectionalLight`, so this
works in any scene that has a sun at all, with or without a **Sun** component.
The deck relights as the day advances and fades out below the horizon. It also
reads the scene's **atmosphere**, so sunlight reaching it reddens and dims
exactly as the sky does; see [Atmosphere coupling](#atmosphere-coupling).

## How it works

Two GPU noise fields are baked once, at startup, and then never touched again:

* a **1024² base map**, sampled by world XZ. Its red channel is Perlin FBM
  multiplied by Worley noise — the Perlin gives connected, wispy structure and
  the Worley carves the puffy cauliflower edges neither produces alone. The
  other two channels widen or narrow the coverage threshold per region, and
  modulate the height profile.
* a **32³ detail volume** of high-frequency Worley, which erodes the base
  silhouette into wisps.

Both tile seamlessly, which is what lets a ray travel any distance and the wind
offset grow without bound while still landing inside them.

Per pixel, the dome shader then:

1. Solves where the view ray enters and leaves the cloud shell. All three cases
   are handled — under the deck, inside it, above it — so a camera can **fly
   through the deck** and come out on top of it. See
   [Flying through](#flying-through).
2. Marches that span, sampling the base map, applying a **height profile** that
   flattens bases and billows tops, and eroding with the detail volume.
3. At every sample with density, marches again **toward the sun** with
   geometrically growing steps to get that sample's optical depth.
4. Accumulates radiance with Hillaire's energy-conserving integration, a
   **dual-lobe Henyey-Greenstein phase** — a tight forward lobe for the silver
   lining, a wide backward one for clouds lit from behind — and three
   **multiple-scattering octaves**, each carrying less light, extincting less,
   and scattering more isotropically than the last.
5. Fades the result into the horizon haze by **distance**, and outputs
   premultiplied colour that composites over whatever sky and stars are already
   there.

Two of those exist to fix things a naive port gets wrong, and are worth knowing
about because their symptoms are distinctive:

* **Multiple scattering.** A single dual-lobe phase spans a 400:1 range across
  the sky, so everything more than about 60° off the sun ends up lit by the
  ambient term alone — flat, grey, and unresponsive to where the sun is. The
  octaves reach deeper into the cloud with a progressively more isotropic phase,
  which is what keeps a shaded cloud body lit.
* **A weather map.** The base atlas repeats every ~13 km of world at the default
  scale, which along the horizon is close enough together to read as wallpaper.
  The same atlas sampled an order of magnitude wider, and rotated so the two
  lattices never line up, modulates coverage per region: every repeat of the
  silhouette then carries a different amount of cloud, and the pair only comes
  back into phase after a thousand km.

The erosion detail also fades out with distance. Its volume is 32 cells
repeating every few hundred metres — close up that reads as wisps, but a few km
out it is below a pixel and all that survives of it is the repeat, tiled across
the horizon.

## Morphing

Wind only *translates* the deck. A cloud whose silhouette never changes reads as
a cutout sliding across the sky however fast it moves, so **Morph Speed** runs
shape evolution on its own clock, crossing the wind rather than following it:

* A **warp field** — a smooth vector field at a third of the silhouette's
  frequency — displaces where the base map is sampled, and scrolls along its own
  axis. Because the field moves, the displacement at any fixed point in the
  world keeps changing, so clouds stretch and fold in place rather than passing
  through unaltered. One cloud spans several times its own width of the field,
  which is what makes it deform rather than merely shift.
* The **detail volume** walks its third axis over time, so the erosion evolves
  instead of being carried along rigidly. This is free — the volume is 3D
  whether or not anything moves through it.

Set it to 0 for a frozen sky that only drifts.

The mesh is a dome centred on the camera, but only as a way to get one fragment
per sky pixel — nothing about the shading uses its surface. It is sized from the
camera's far plane so scene geometry occludes it correctly.

## Atmosphere coupling

Leave **Atmosphere Lighting** on and every colour in the Lighting and Atmosphere
sections becomes a *noon* value that the scene's atmosphere then modulates. Drop
the sun toward the horizon and the light reaching the deck reddens and dims on
its own, the skylight filling the shadows cools and darkens, and the haze splits
into a warm half toward the sun and a cool half away from it. Nothing is
keyframed — it falls out of the same scattering medium the sky itself is
rendered from, so changing the atmosphere (or swapping Earth's medium for Mars')
changes the clouds with it.

The sun's height changes their *shape* as well as their colour, because the sun
march lengthens as it drops: light from overhead crosses the deck by the
shortest path there is, while light from 20° up crosses three times as much
cloud to reach the same point. That is the difference between a bright, flat
midday sky and a deep, modelled evening one, and it happens on its own.

Be aware the coupling is a *relative* model, so between 90° and 45° of elevation
it multiplies by 0.97–0.86 — deliberately almost nothing. The sun has to get
below about 20° before it is obvious, and below 10° it is dramatic.

Two situations look like "no atmosphere" and neither switches this off: a scene
that never spawned one because it is lit by a skybox or an HDRI, and a scene
that turned its procedural sky off. Both still have a sun in them, so both
measure Earth's medium instead — the alternative is a deck that stays noon-white
while everything underneath it goes to dusk, which is the single most
conspicuous way for a sky to look wrong.

The only thing that hands the authored colours straight through untouched is
turning **Atmosphere Lighting** off, which is what you want for a stylised sky
that should not care how high the sun is.

Mechanically this is evaluated once per frame on the CPU rather than sampled
per pixel, because Bevy's atmosphere LUTs live in the render world where a
material cannot reach them. That costs almost nothing and loses almost nothing:
the two terms that matter most to a cloud — the sunlight surviving to the deck,
and the colour of the sky filling its shadows — depend on the sun's elevation
and the deck's altitude, not on which pixel is asking. Only the haze genuinely
varies across the sky, and that is handled by evaluating the horizon twice, once
toward the sun and once away, and blending between them by bearing.

Two things it does not model: multiple scattering, so twilight reads slightly
darker here than in the sky behind it; and per-pixel aerial perspective, so the
haze follows a smooth falloff toward the horizon rather than the exact depth of
each cloud.

## Settings

### Shape

| Field | What it does |
|---|---|
| **Coverage** | 0 = clear, 1 = solid overcast. |
| **Density** | How opaque the cloud material is. |
| **Scale** | Size of the formations. Lower = larger systems. |
| **Detail Scale** | Frequency of the erosion detail relative to the base shape. |
| **Detail Strength** | How much that detail eats into the silhouette. |
| **Edge Softness** | Width of the coverage threshold. Low = crisp cauliflower, high = soft haze. |
| **Base Softness** | Fraction of the deck's depth over which density fades in from the base. |

### Deck geometry

| Field | What it does |
|---|---|
| **Bottom Height** | Altitude of the base of the deck, in metres. |
| **Top Height** | Altitude of the top of the deck, in metres. |
| **Planet Radius** | Radius of the virtual planet the deck wraps, in metres. This is what sets how sharply clouds compress toward the horizon — shrink it for a small, storybook world; leave it at Earth's 6 371 000 for a realistic sky. |

### Wind

| Field | What it does |
|---|---|
| **Wind Speed** | Metres per second. Weather-system speeds, not breezes — the default 40 m/s moves the deck about one cloud every thirty seconds. |
| **Wind Direction** | Degrees, 0–360. |
| **Morph Speed** | How fast shapes evolve, independent of the wind carrying them. 0 freezes them into pure drift. |

Wind moves the *sample* position through the noise, not the geometry, so the
deck drifts without the horizon moving with it. See
[Morphing](#morphing) for why drift alone is not enough.

### Lighting

With **Atmosphere Lighting** on, every colour here is the value at noon; the
atmosphere shifts it from there.

| Field | What it does |
|---|---|
| **Color** | Tint of the sunlight scattering out of the cloud. |
| **Brightness** | Overall level of the deck. A *trim* on the scene sun, not a substitute for it — both the sunlight and the skylight scale with the `Sun` component's illuminance first, so dimming the scene's sun dims the clouds with everything else, and this only says how bright the deck sits within that. |
| **Ambient Color** | Skylight filling the top of the deck. |
| **Shadow Color** | Skylight filling the base — scattered blue sky, which is what actually lights a real cloud's underside. Keep it **saturated**, not merely dark: grey cloud is almost never a brightness problem, it is a warm direct term summed with a near-neutral fill, and the only way out of neutral is for the two to disagree about hue. |
| **Ambient** | Multiplier on both ambient colours. |
| **Absorption** | How fast light is absorbed inside the cloud, and therefore how hard the lit/shadowed contrast is. |
| **Forward Scattering** | Eccentricity of the forward lobe — the silver lining. |
| **Backward Scattering** | Eccentricity of the backward lobe. Negative by convention. |
| **Scattering Blend** | Mix between the two lobes. 0 = all forward, 1 = all backward. |
| **Powder Effect** | Darkens thin sunlit edges, which have scattered little light back toward the eye yet. Without it, rims look like cut paper. |

### March

| Field | What it does |
|---|---|
| **Raymarch Steps** | Samples along each view ray. The dominant cost knob. |
| **Shadow Steps** | Samples along each sun-shadow ray. Set to 0 to drop self-shadowing entirely — much cheaper, and much flatter. |

### Atmosphere

| Field | What it does |
|---|---|
| **Atmosphere Lighting** | Drive the lighting from the scene's atmosphere. See above. |
| **Horizon Color** | Colour distant cloud fades into, at noon. |
| **Atmosphere** | How much haze the deck picks up at distance. |

Haze accumulates with the **distance** the ray travelled before it met cloud,
not with how low in the sky the pixel is — depth of air is what the eye reads as
depth in a sky, and keying it that way also does the work of hiding the base
map's repeat, which only becomes visible at the distances haze is thickest at.

## Flying through

The march starts at the camera and solves its span against the real shell, so
all three positions work: under the deck, inside it, and above looking down. Fly
up into it and you are enveloped; keep going and you come out on a cloud tops
view. Nothing needs switching on for this.

Two details make it hold up rather than merely not crash. The view march steps
**geometrically** rather than uniformly — a uniform step has to choose between
resolving the cloud you are inside and reaching the one on the horizon, and
sized for the horizon it would put two samples between you and the far side of
the cloud you are in, turning the envelope into a flat wall. And the dome is
centred using the camera's **global** transform, so a camera parented to a
flying rig still gets the deck put where its eye actually is.

## Day and night

The deck fades out below the horizon, between −2° and −12° of sun elevation. The
window sits entirely below the horizon on purpose: golden hour is the best the
clouds ever look, and a fade centred on 0° would have them half-transparent
through all of it. Above −2° they are fully solid; by −12° they are gone, so
night skies show stars rather than an unlit silhouette punched through them.

## Performance

Both marches run per pixel, so clouds are a fullscreen, resolution-bound cost
that is the same for an empty scene as for a full level. The
[graphics quality tier](pipeline.md#graphics-quality-tiers) is the main control:

* **High** — your configured step counts.
* **Medium** — capped at 16 view steps and 4 shadow steps.
* **Low** — the dome is despawned entirely.

Below that, in rough order of what buys the most frame time back:

* **Raymarch Steps** is close to linear in cost. 32 is comfortable; 16 still
  reads well with TAA on.
* **Shadow Steps** multiplies the cost of every sample that has density in it.
* **3D render scale** in the viewport settings cuts pixel count directly.

Ray starts are dithered with a hash of the view direction rather than of time or
screen position, so the dither is pinned to the sky: it does not crawl as the
camera turns or flicker frame to frame, which leaves it in a form TAA resolves
cleanly. If you run with TAA off, raise **Raymarch Steps**.
