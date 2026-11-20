# Outline

Screen-space outlines drawn along detected edges.

`mix_mode` crossfades between drawing the outline over the picture and replacing the picture with it, so one effect covers both the comic-book look and a plain line render.

**Use it:** add a **Post Process** component to the camera or the World Environment, then drag `outline.wesl` from the Assets panel onto it.

**Settings:** Thickness, Threshold, Mix Mode
