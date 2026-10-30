# Vibrance

Saturation weighted by how unsaturated a pixel already is, so it lifts muted colour without pushing already-vivid pixels further.

`intensity` goes negative, which desaturates the same way round.

**Use it:** add a **Post Process** component to the camera or the World Environment, then drag `vibrance.wesl` from the Assets panel onto it.

**Settings:** Intensity
