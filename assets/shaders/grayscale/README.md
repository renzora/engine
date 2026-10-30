# Grayscale

Drains colour from the frame. `intensity` crossfades, so it doubles as a partial desaturation rather than only an on/off black and white.

The luminance weights are the Rec. 709 constants and are deliberately not exposed: they are values the shader reads every pixel, not three sliders worth having.

**Use it:** add a **Post Process** component to the camera or the World Environment, then drag `grayscale.wesl` from the Assets panel onto it.

**Settings:** Intensity
