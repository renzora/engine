# Sepia

Sepia tone mapping. The simplest effect in the set, and the one worth reading if you are writing your own.

The tone weights are a tuned constant rather than three sliders, so they stay in the struct and out of the Inspector.

**Use it:** add a **Post Process** component to the camera or the World Environment, then drag `sepia.wesl` from the Assets panel onto it.

**Settings:** Intensity
