# Letterbox

Black bars top and bottom, for cutscenes and cinematic framing.

`aspect_ratio` at zero means "use `bar_height` directly". Give it a real ratio instead and the bars size themselves to crop the view to it, which is what you want for a cutscene that has to look the same on every display.

For a narrow picture on a wide display, use **Pillarbox**.

**Use it:** add a **Post Process** component to the camera or the World Environment, then drag `letterbox.wesl` from the Assets panel onto it.

**Settings:** Bar Height, Softness, Aspect Ratio
