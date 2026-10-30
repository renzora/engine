# Clouds

Volumetric clouds, raymarched through a spherical deck that follows the camera. The model is the Horizon Zero Dawn one (Schneider/Vos) with Frostbite's scattering integration: a tileable Perlin-Worley base map defines the silhouette, a 3D Worley volume erodes it, a height profile turns a flat map into flat-bottomed billowing cumulus, and a second march toward the sun gives every sample its own shadow.

The deck reads the scene's atmosphere, so it relights with the sky and follows the sun down at dusk. Wind comes from the world wind unless the deck opts out.

**Add it:** `+ Add Entity → Rendering → Clouds`.

**Note:** the **Low graphics quality tier switches clouds off entirely** (the two per-pixel marches are the largest scene-independent raster cost on a weak GPU). If the sky is empty and the Clouds header shows an amber warning, that is the tier, not a fault: Settings → Viewport → Performance.

**Files:** `noise.rs` bakes the two noise fields once on the GPU, `sky.rs` reads the atmosphere, `material.rs` and `clouds.wgsl` do the march.

**Scope:** Runtime (editor viewport and shipped game).
