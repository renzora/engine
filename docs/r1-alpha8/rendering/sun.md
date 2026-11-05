# Sun & Glare

The **Sun** section of a World Environment places the scene's sun and sets how
it looks in the sky.

| Field | What it does |
|---|---|
| **Azimuth**, **Elevation** | Where the sun is, in degrees. Below the horizon its light fades out. |
| **Color**, **Illuminance** | The light it casts, in lux. 40,000 is a clear day. |
| **Angular Diameter** | How big the disc looks, in degrees. Earth's sun is 0.53. |
| **Disk Intensity** | How bright the disc is. 0 hides it, 1 is physical. |
| **Glare** | Strength of the bright haze around the sun when it is in view. 0 turns it off; the default is 1.5. Raise **Glare Size** with care: the wide part of the glare grows with both, and together they soon veil the whole sky. |
| **Glare Size** | How far the glare spreads from the sun, in degrees. |
| **Shadows**, **Contact Shadows** | Whether the sun casts shadow maps and screen-space contact shadows. |

A new World Environment starts with the sun at mid-morning, azimuth 245.5°
and 38.5° up, with contact shadows on and screen-space reflections enabled:
high enough to light a street between buildings, low enough that walls are
modelled and shadows fall long.

## Glare

Looking toward the sun, the glare is what makes it blinding: a hot core around
the disc, a wide halo, and a faint lift across the sunward sky.

Bloom cannot make this on its own. It caps how much any single pixel adds to
the glow, so a sun disc many thousands of times brighter than the sky blooms
only as much as a bright cloud. The glare is drawn separately, before
tonemapping, so auto exposure and the camera's tone curve treat it like any
other light in the picture.

It fades by itself when something covers the sun. Every frame it compares the
disc with the sky just around it, so clouds drifting across, terrain, buildings
or the sun leaving the frame all dim it, with nothing to set up. The reading is
averaged over the last few frames, so a cloud edge or the noise of ray-traced
lighting cannot make it flicker; a cloud crossing the sun dims it within about
a tenth of a second. Its strength
follows the brightness of the sky around the sun, so it softens through haze
and at sunset along with everything else.

The glare needs the sky: a camera only draws it while the World Environment's
**Atmosphere** is on.
