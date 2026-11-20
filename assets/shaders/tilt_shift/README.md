# Tilt Shift

Blurs the top and bottom of the frame, leaving a sharp band in the middle, which reads as a miniature.

A screen-space fake rather than a real depth of field: the focus band is a horizontal strip at `focus_position`, so it costs nothing to sample the depth buffer and works on a 2D scene that has none. For real depth of field, use the engine's own **Depth of Field**.

**Use it:** add a **Post Process** component to the camera or the World Environment, then drag `tilt_shift.wesl` from the Assets panel onto it.

**Settings:** Blur Amount, Focus Position, Focus Width, Focus Falloff
