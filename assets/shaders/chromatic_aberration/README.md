# Chromatic Aberration

Splits the colour channels across the frame, the way a cheap lens does. The direction is a fixed horizontal split rather than a setting: a directional aberration reads as a camera fault, and the one people reach for is lateral.

For the radial version, where the split grows with distance from the centre, use **Chromatic Ring**.

**Use it:** add a **Post Process** component to the camera or the World Environment, then drag `chromatic_aberration.wesl` from the Assets panel onto it.

**Settings:** Intensity, Samples
