Newer release notes have been moved to their own [folder](https://github.com/PVDoriginal/firefly/tree/main/release-notes).

# 0.18.0 
## Major 
  - Updated to Bevy 0.18.
## Minor 
  - A few minor performance optimizations. 

# 0.17.2 
## Major 
  - Fixed a severe bug that caused bad memory allocation when too many occluders were clustered in the same area. 

# 0.17.1 
Update mainly focused on performance improvements. 

## Major 
  - Moved most GPU data to global buffers that assign and reallocate resources to each entity, based on visibility and change detection. Previously the buffers were rewritten each frame, bottlenecking performance.
  - Implemented a custom hybrid of angular sweep and 2D BVH that allocates occluders to bins based on their polar intervals relative to the light sources. This has added a huge boost to performance.

## Minor 
  - Firefly no longer supports WebGL2. I've decided to use Storage Buffers which are only available on WebGPU which are considerably more flexible. This should hopefully not affect anyone.
  - Sprite exclusions were removed from occluders and are no longer a feature. They were unreliable and added considerable overhead. If you consider they were useful, consider creating an issue telling me to re-add them.
  - Frustrum culling has been greatly improved.
  - Lights now have an optional 'falloff intensity' that can be used to adjust them further.    

# 0.17.0 
## Major
  - Updated Bevy version to 0.17.

# 0.16.2 
## Major
  - Added support for normal maps.
  - Moved light rendering to a BinnerRenderPhase, significantly boosting performance. 
## Minor 
  - Small bugfixes and optimizations.
  - Offset field for lights and occluders. 

