# Clouds

Volumetric clouds, raymarched through a spherical deck that follows the camera.
A tileable noise map sets where clouds form, a 3D noise volume erodes their
edges, a height profile gives them flat bases and billowing tops, and a second
march toward the sun gives every sample its own shadow.

The deck reads the scene's atmosphere, so it relights with the sky and turns
warm as the sun goes down. It drifts with the world [wind](./wind.md) unless
told not to.

## Adding clouds

- A **World Environment** (from **+ Add Entity**, or the **New Environment**
  starter) comes with clouds already on it.
- On any other entity, **Add Component > Clouds** in the Inspector.

Only the first enabled Clouds component in the scene draws. The toggle in the
section header switches the deck off without losing its settings.

The **Low** graphics quality tier switches clouds off entirely: the two
per-pixel marches are the largest fixed raster cost on a weak GPU. If the sky
is empty and the Clouds header shows an amber warning, that is the tier, not a
fault (**Settings > Viewport > Performance**). Below **High**, the march takes
fewer steps.

## The controls

| Group | Field | What it does |
|---|---|---|
| Shape | **Coverage** | 0 is a clear sky, 1 is solid overcast. |
| | **Density** | How opaque the cloud material is. |
| | **Scale** | Size of the formations: lower means larger, broader systems. |
| | **Detail Scale**, **Detail Strength** | How fine the eroding detail is, and how much of the outline it eats. |
| | **Edge Softness** | Low gives crisp, cauliflower edges; high gives soft haze. |
| | **Base Softness** | How far up from the base the density fades in. |
| Deck | **Bottom Height**, **Top Height** | Altitude of the deck, in metres. |
| | **Planet Radius** | How sharply the deck curves toward the horizon. Earth's radius for a realistic sky, smaller for a storybook one. |
| Wind | **Wind Speed**, **Wind Direction** | Drift of the deck, used when it does not follow the world wind. Cloud features are kilometres across, so this is weather-system fast. |
| | **Morph Speed** | How fast the shapes change as they drift. 0 freezes them. |
| Lighting | **Color**, **Brightness** | Tint and level of the sunlit cloud, relative to the scene's sun. |
| | **Ambient Color**, **Shadow Color**, **Ambient** | Skylight on the top and the underside of the deck. Keep the shadow colour saturated: a grey sky under cloud is almost always a hue problem, not a brightness one. |
| | **Absorption** | How hard the contrast between the sunlit and shadowed sides is. |
| | **Forward Scattering**, **Backward Scattering**, **Scattering Blend** | The silver lining toward the sun, and the glow of clouds seen against it. |
| | **Powder Effect** | Darkens thin sunlit edges, which otherwise look like cut paper. |
| March | **Raymarch Steps**, **Shadow Steps** | Samples per view ray and per shadow ray. The main cost controls. |
| Atmosphere | **Atmosphere Lighting** | Light the deck from the scene's atmosphere. With it on, every colour above is a noon value that the sky's state modulates. |
| | **Horizon Color**, **Atmosphere** | The haze distant cloud fades into, and how strongly. |

## In exported games

Clouds are an engine crate, `renzora_clouds`, behind the runtime's `clouds`
feature. A game export keeps them when a scene uses clouds and leaves them out
otherwise; the **Clouds** toggle in the export's Sky section overrides that.
