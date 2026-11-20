# CRT

Screen curvature, scanlines, a colour fringe and a vignette in one pass.

Each of those exists separately in this set, but a CRT wants them sharing a curved UV: the scanlines have to bend with the glass, and stacking four separate passes would leave them flat over a curved picture.

**Use it:** add a **Post Process** component to the camera or the World Environment, then drag `crt.wesl` from the Assets panel onto it.

**Settings:** Scanline Intensity, Curvature, Chromatic Amount, Vignette Amount
