# Dithering

Ordered (Bayer) dithering: colour is reduced to a small number of levels and the error is traded for a fixed crosshatch pattern, the way early 8-bit and 16-bit displays faked extra shades.

The threshold matrix is generated in the shader, so the effect ships no texture and the pattern stays pixel-exact at any render resolution.

**Use it:** add a **Post Process** component to the camera or the World Environment, then drag `dithering.wesl` from the Assets panel onto it.

**Settings:** Color Depth, Intensity
