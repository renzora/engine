# Thermal Vision

A thermal camera look: luminance is mapped to a heat palette, from cold blues through to white-hot. `cold_threshold` sets where the palette starts.

It reads brightness, not temperature, so a bright cold surface still reads as hot.

**Use it:** add a **Post Process** component to the camera or the World Environment, then drag `thermal.wesl` from the Assets panel onto it.

**Settings:** Intensity, Contrast, Cold Threshold
