# Volumetric Fog & Light Shafts

Volumetric fog is fog with depth: light passes through it, so the sun cuts
bright shafts through it wherever buildings, trees or arches let it through,
and a lamp glows in a halo. Ordinary distance fog (the **Fog** section of the
World Environment) only fades things with distance; it has no shafts.

## Quick start

1. **Add Entity > Environment > Fog Volume.** A box of fog 40 m across and
   20 m tall appears, sitting on the ground at the origin.
2. Move it over the area that should be hazy and resize it with its
   **Transform** scale. Fog exists only inside the box.
3. Make sure a light is **volumetric**: the World Environment's sun already is;
   for another light, **Add Component > Volumetric Light**. The Fog Volume's
   card says so when no light is.

Look toward the sun through a gap (an arch, a street between buildings) and
the shafts show. They come mostly from light scattering forward, so with the
sun behind you the fog mostly reads as haze.

## How it is put together

Three things have to meet, and with any one missing nothing renders:

| Piece | What it is | Where it comes from |
|---|---|---|
| A **Fog Volume** | The box of fog | Add Entity; one per hazy area |
| **Volumetric fog on the camera** | The pass that marches through the boxes | On by itself while the scene holds a Fog Volume; set on the Fog Volume's card |
| A **Volumetric Light** | A light that casts shafts | On the World Environment's sun; Add Component for others |

The camera pass is on by itself while any Fog Volume exists and off again when
the last one goes, so a scene without fog never pays for it. How it draws (the
sky light inside the fog, the step count) is set on the Fog Volume's own card,
so everything about the fog is in one place. With several Fog Volumes, the
oldest one's settings are used.

A scene saved before then may show a separate **Volumetric Fog** card on its
World Environment; while it is there its settings win. Delete it to use the
Fog Volume's.

## Fog Volume

| Field | Default | What it does |
|---|---|---|
| **Density** | 0.02 | How thick the fog is. Over a box tens of metres deep a little goes a long way: 0.1 (a small, dense cloud) turns a street dark. |
| **Color** | white | The fog's own colour. |
| **Scattering** | 0.3 | How much light the fog throws back toward the camera: what makes shafts and halos bright. |
| **Absorption** | 0.1 | How much light the fog swallows. Higher darkens everything behind it without adding any glow. |
| **Forward Scatter** | 0.6 | How strongly light carries on in the direction it was going. Higher makes shafts bright looking toward the light and faint looking away. |
| **Light Tint**, **Light Intensity** | white, 1 | Tint and strength of the light the fog picks up. |

Scattering, absorption and the rest are folded into the card's **Light** group.
Two more groups set how the fog is drawn:

| Group | Field | Default | What it does |
|---|---|---|---|
| Sky Light | **Sky Color**, **Sky Brightness** | pale blue, 0.4 | The light the fog gets from the sky, which is what makes it read as bright haze rather than a dark veil. Brightness is in on-screen terms: about 1 is white however bright the scene is. Lower it for murky fog, raise it for a white-out. |
| Quality | **Step Count** | 64 | Samples along each view ray. Fewer is cheaper and shows banding. |
| Quality | **Jitter** | 0.5 | Moves the samples each frame so TAA can smooth out the banding. |

## Cost

The pass costs a few milliseconds of GPU time: about 5 ms at 1706 × 1408 on an
RTX 3060 with one large volume. It is the most expensive screen effect after
ray-traced GI, which is why it only runs while a Fog Volume exists.

## Exporting

Volumetric fog is its own export capability, under post-processing, and is
detected from a scene's Fog Volume or Volumetric Fog component.
