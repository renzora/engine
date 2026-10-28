# Shadows

Every light can cast shadows. The settings are in the light's section of the Inspector: **Directional Light**, **Point Light** or **Spot Light**.

## Turning shadows on

| Field | Effect |
|---|---|
| **Shadows** | The light renders a shadow map and things block it. A shadowed spot light is one extra render of the scene and a point light six, one per cube face, so give shadows only to the lights where they show. |
| **Contact Shadows** | Fills in the small shadows where objects meet surfaces (a cup on a table, the gap under a door) that a shadow map is too coarse to catch. Traced in screen space, so it is cheap and only sees what is on screen. |

On the scene's Sun, both switches are the Sun's own: changing them in either the Sun or the Directional Light section changes the same setting.

## Fixing acne and floating shadows

A shadow map stores depth at a fixed resolution, and two artifacts come from that:

- **Shadow acne**: stripes or speckles of shadow on a lit surface, where the surface shadows itself.
- **Peter-panning**: a shadow that starts a little way from the object casting it, so the object seems to float.

The two bias fields push the comparison in opposite directions. Raise them to cure acne; lower them if shadows detach.

| Field | Default | Effect |
|---|---|---|
| **Shadow Depth Bias** | 0.02 directional and spot, 0.08 point | Pushes the stored depth away from the light. |
| **Shadow Normal Bias** | 1.8 directional and spot, 0.6 point | Pushes along the surface normal, scaled to the shadow map's texel size, so it adapts to how stretched the map is. Usually the better one to adjust first on steep or curved surfaces. |
| **Shadow Near Plane** | 0.1 | Point and spot only. Nothing closer to the light than this casts a shadow. Raising it sharpens shadows, so set it as high as the scene allows. |

## Cascades (directional lights)

A directional light covers everything from your feet to the horizon, so one shadow map would be blurry everywhere. Instead the view is split into slices called **cascades**, each with its own map: the nearest slice gets fine detail and the far ones get coarse detail where you cannot tell.

| Field | Default | Effect |
|---|---|---|
| **Shadow Cascades** | 4 | How many slices, 1 to 4. Fewer is cheaper; a game with a fixed top-down camera often looks fine with 1 or 2. |
| **Shadow Distance** | 150 | Beyond this distance from the camera nothing receives shadows. Lower it to spend the same resolution on a smaller area: a close third-person game gets much crisper shadows at 40 to 60. |
| **First Cascade Distance** | 10 | Where the sharpest slice ends. The rest are spread out from here to the shadow distance. |
| **Cascade Blend** | 0.2 | How much neighbouring slices overlap, so the change from one to the next is a soft blend instead of a visible line. |

## Soft shadows

A real shadow is sharp where an object touches the ground and grows blurrier the further it falls from the object, because a light is not a single point. Soft shadows reproduce that.

| Light | Field | Effect |
|---|---|---|
| Directional | **Soft Shadow Size** | 0 is off, giving hard shadows. Above 0 is the size of the light: larger gives blurrier shadows away from the caster. |
| Point, Spot | **Soft Shadows** | Turns them on. |
| Point, Spot | **Radius** | The size of the bulb. Sets how blurry the shadows get, and also how large the light's reflections look on shiny surfaces. |

Soft shadows cost noticeably more than hard ones, and on their own they look grainy. Add **Blue Noise Shadows** to the camera and keep TAA on, so the grain averages out over a few frames.

## Resolution and filtering

The shadow map's resolution is set per project by the graphics quality tier: 2048 on **High**, 1024 on **Medium**, 512 on **Low**, for every cascade.

Shadow edges are filtered with a fixed hardware filter by default. Add **Blue Noise Shadows** to the camera to filter them with temporally jittered blue noise instead, which hides banding on soft edges when TAA is on.

## Related

- [Camera System](./camera)
- [Solari Ray-Traced GI](./solari)
