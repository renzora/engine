# Screen-Space Indirect Lighting (SSIL)

Light does not stop at the first surface it hits. Sun on a red brick wall
throws a warm tint onto the pavement in front of it; a lit shop front brightens
the underside of its awning. Direct lighting does not do this, and neither does
the environment map, which only knows the sky.

**SSIL** adds that bounce. Every pixel looks at the surfaces around it on
screen, takes the light they showed last frame, and adds what reaches it onto
its own colour, while dimming the sky light those surfaces block. It is cheap
compared with ray-traced GI and runs on any GPU, at the cost of only knowing
about what is on screen.

## Quick start

A new **World Environment** comes with SSIL on, with defaults that suit a scene
built in metres. For a scene made before that, or for a camera, select it and
use **Add Component > Post Process > SSIL**.

SSIL draws only when all of these hold:

- the camera renders **Deferred**: the bounce is multiplied by each pixel's
  diffuse colour, which only the deferred G-buffer has;
- the **Graphics Quality** tier is **High** (see
  [the tier table](./pipeline.md#graphics-quality-tiers)); below that the
  section's header shows the amber gate warning;
- [Solari](./solari.md) is **not** lighting the camera. Solari traces its own
  bounce light, so SSIL stands aside while it is on and comes back when it is
  off, with its settings untouched. The section says so while it happens.

## Settings

Intensity, Occlusion and Radius are on the card. Samples, Sharpness and Normal
Rejection fold into a **Quality** group, and the two fade distances into
**Distance Fade**.

| Field | Default | What it does |
|---|---|---|
| Intensity | 1.0 | How strong the bounce light is. |
| Occlusion | 0.3 | How strongly the sky light blocked by nearby surfaces is taken away. 1 is the full amount; the default keeps darkening in corners without darkening the scene overall, since Bevy's own SSAO, if the camera has it, dims ambient light as well. 0 leaves only the bounce. |
| Radius | 5 m | How far light travels between surfaces. Larger reaches further but spreads the same taps thinner, so it gets noisier. |
| Samples | 12 | Taps per half-resolution pixel, 3 to 32, each with a mirrored twin. Cost grows linearly. |
| Sharpness | 0.98 | How firmly the blur stops at depth edges. Lower lets the bounce soften across silhouettes. |
| Normal Rejection | 1.0 | How strictly a surface must face a pixel to light it. At 0 the back of a wall lights what is in front of it. |
| Fade Start | 50 m | Distance at which the bounce begins to fade. |
| Fade End | 300 m | Distance beyond which there is none. Far away one pixel covers more than the radius, so the result would be noise. |

## Seeing what it does

The viewport's **Display ▸ Visualization ▸ SSIL** submenu shows one part of
SSIL in place of the frame, in editor viewports only:

| View | Shows |
|---|---|
| **Light** | The gathered bounce on its own, before it is multiplied by each surface's colour. |
| **Occlusion** | How much of the sky light each pixel loses: white is open, black blocked. |
| **History** | Last frame exactly as the taps read it, reprojected and blurred. If this is black or misaligned, nothing else can work. |

They draw only where SSIL is on, so with SSIL off or stood down the view shows
the normal frame. **None** at the top of Visualization goes back to it.

## What it can and cannot see

SSIL only knows what is on screen. A bright wall behind the camera, or one
hidden behind a pillar, sends nothing. Because it reads last frame, which
already holds last frame's bounce, light keeps bouncing over a few frames; a
fast camera cut starts from nothing for one frame.

The dimming needs the camera's environment light to know how much sky light a
pixel had. A camera without one still gets the bounce, but nothing is dimmed.

Metals receive no bounce, because they have no diffuse colour; their
reflections come from SSR and the environment map. Unlit materials show
exactly their own colour and receive nothing either.

## How it works

The pass runs after the main pass has lit the frame and before temporal
anti-aliasing, so TAA smooths it along with everything else.

1. **Gather**, at half resolution. Each pixel takes mirrored pairs of taps on
   a rotated disk within the radius and rebuilds each tap's position from the
   depth buffer. A tap counts by how much it rises above the pixel's surface
   (its obscurance), and sends light only if it faces the pixel. The light is
   read from where the tap was last frame, in a blurred 1/32-size copy of that
   frame, compressed by `c / (1 + luma)` before averaging so one blown-out
   pixel cannot dominate. The compression works in units of last frame's
   average brightness, so it behaves the same under physical light units as
   in a scene lit near 1. The gather also writes the obscurance and the depth
   edges.
2. **Blur**, four passes, two wide and two narrow, stopping at depth edges.
3. **Composite**, at full resolution: the sky light the obscurance blocks is
   taken off and the gathered light put on, both on the diffuse colour from
   the G-buffer.
4. **History**: the finished frame is kept, with its mips, for the next
   frame's gather.

The gather, edges and blur are Intel's ASSAO (MIT licence) as converted to
gather light.

## Exporting

SSIL is its own export capability, **SSIL**, under post-processing. It is on by
default and is detected automatically when a scene uses it.
