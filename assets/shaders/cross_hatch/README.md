# Cross Hatch

Cross-hatch shading, as a pen-and-ink drawing shades with overlapping strokes: the darker the pixel, the more layers of hatching it gets.

`angle` is in radians and tops out at a quarter turn, because past that the hatching is the same set of lines again, mirrored.

**Use it:** add a **Post Process** component to the camera or the World Environment, then drag `cross_hatch.wesl` from the Assets panel onto it.

**Settings:** Density, Thickness, Angle, Brightness
