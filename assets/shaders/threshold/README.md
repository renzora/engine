# Threshold

Cuts the frame to black and white at a brightness threshold.

`smoothness` is what keeps the cut from aliasing: at zero the split is a hard step and every edge in the picture stairsteps, so the default eases it over a narrow band either side.

**Use it:** add a **Post Process** component to the camera or the World Environment, then drag `threshold.wesl` from the Assets panel onto it.

**Settings:** Threshold, Smoothness
