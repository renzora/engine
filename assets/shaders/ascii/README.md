# ASCII

ASCII art post-process effect. The character shapes are generated in the shader from luminance rather than sampled from a font atlas, so there is no texture to ship and the cell size can be any number rather than a multiple of a glyph.

**Use it:** add a **Post Process** component to the camera or the World Environment, then drag `ascii.wesl` from the Assets panel onto it.

**Settings:** Char Size, Color Mix, Contrast
