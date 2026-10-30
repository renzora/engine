# Oil Painting

A simplified Kuwahara filter with luminance bucketing on top, which is what gives it visible brush clumps rather than the smooth flattening **Kuwahara** produces.

The sample loop is quadratic in `radius`, so that is the cost knob.

**Use it:** add a **Post Process** component to the camera or the World Environment, then drag `oil_painting.wesl` from the Assets panel onto it.

**Settings:** Radius, Levels
