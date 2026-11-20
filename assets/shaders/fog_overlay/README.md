# Fog Overlay

A cheap height-fade over the finished picture. It is a screen-space overlay, not a depth-aware fog: it knows how far up the frame a pixel is, not how far away it is.

For fog that respects distance, use the engine's own **Distance Fog** on the World Environment instead.

**Use it:** add a **Post Process** component to the camera or the World Environment, then drag `fog_overlay.wesl` from the Assets panel onto it.

**Settings:** Density, Height
