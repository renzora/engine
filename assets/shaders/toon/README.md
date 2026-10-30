# Toon

Cel shading: luminance is quantized into bands and an edge is drawn over the result.

It is **Posterize** and **Outline** in one pass rather than two, which is the point: doing it together keeps the edge aligned to the bands it is outlining.

**Use it:** add a **Post Process** component to the camera or the World Environment, then drag `toon.wesl` from the Assets panel onto it.

**Settings:** Levels, Edge Threshold, Edge Thickness, Saturation Boost
