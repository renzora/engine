# Kuwahara

Edge-preserving smoothing: flat areas are flattened further while edges stay sharp. That is the painterly look without the brush clumps **Oil Painting** adds.

`radius` is the cost knob as well as the look knob, since the sample loop grows with its square.

**Use it:** add a **Post Process** component to the camera or the World Environment, then drag `kuwahara.wesl` from the Assets panel onto it.

**Settings:** Radius
