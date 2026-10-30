# Sobel Edge

Sobel edge detection: the frame is replaced by its own gradient, so only edges survive.

For edges drawn over the picture rather than instead of it, use **Outline**; for glowing ones, **Edge Glow**.

**Use it:** add a **Post Process** component to the camera or the World Environment, then drag `sobel_edge.wesl` from the Assets panel onto it.

**Settings:** Intensity, Threshold
