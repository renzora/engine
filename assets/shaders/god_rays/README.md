# God Rays

Volumetric light shafts radiating from a point on screen: the frame is marched outward from the light position, accumulating brightness along the way.

The light position is authored in screen space (`light_pos_x` / `light_pos_y`, 0 to 1), so point it at wherever your sun lands in frame. `num_samples` is the cost knob.

**Use it:** add a **Post Process** component to the camera or the World Environment, then drag `god_rays.wesl` from the Assets panel onto it.

**Settings:** Intensity, Decay, Density, Num Samples, Light Pos X, Light Pos Y
