# Terrain textures

The default ground textures a new terrain is painted with. Each folder holds a
2048 px colour map (`albedo.jpg`), a 2048 px OpenGL-convention normal map
(`normal.jpg`), a 1024 px packed ambient occlusion / roughness / metallic map
(`arm.jpg`, R = AO, G = roughness, B = metallic) and a 1024 px greyscale height
map (`height.jpg`, white is high).

All four are from [Poly Haven](https://polyhaven.com) and are **CC0** (public
domain): no attribution is required, and they may be used, changed and shipped
in anything.

| Folder | Poly Haven asset |
|---|---|
| `grass` | [leafy_grass](https://polyhaven.com/a/leafy_grass) |
| `dirt` | [brown_mud_rocks_01](https://polyhaven.com/a/brown_mud_rocks_01) |
| `rock` | [dry_riverbed_rock](https://polyhaven.com/a/dry_riverbed_rock) |
| `snow` | [snow_02](https://polyhaven.com/a/snow_02) |

They were downloaded at 2k and re-saved as JPEG at quality 88 (colour, ARM)
and 90 (normals, height) to keep the editor small. The rock colour map was
desaturated and given a little more contrast, so its strata read as grey stone;
the dirt colour map was given a little more contrast; the snow colour map was
brightened, since the photo is exposed grey.
