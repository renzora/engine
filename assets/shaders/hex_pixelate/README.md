# Hex Pixelate

Pixelation on a hexagonal grid rather than a square one.

The grid is computed in axial coordinates in the shader, so `hex_size` is a pixel width rather than a cell count: the pattern stays the same size whatever the render resolution.

**Use it:** add a **Post Process** component to the camera or the World Environment, then drag `hex_pixelate.wesl` from the Assets panel onto it.

**Settings:** Hex Size
