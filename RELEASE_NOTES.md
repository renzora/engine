# Renzora Engine `r1-alpha8`

## 2026-10-30 16:06
- feat(clouds): volumetric clouds ship with the engine as a bundled plugin instead of a marketplace install

## 2026-10-30 16:03
- docs(physics): the physics docs no longer point to the Rapier plugin, which has been retired; avian is the physics engine

## 2026-10-30 15:09
- build: the engine now builds against Bevy `main`, carried in-tree in `bevy/`, with every Bevy-based dependency vendored in `vendor/`

## 2026-10-30 11:20
- fix(rendering): the editor no longer crashes with "Compaction is already being prepared" when ray-traced meshes are rebuilt quickly, such as terrain streaming in

## 2026-10-30 11:16
- fix(terrain): fog now reaches terrain that wears the blockout grid
- feat(editor): New Environment starts as a clear mid-morning with sky-lit shadows and distance haze, seen through a wider, filmic camera

## 2026-10-30 10:47
- feat(editor): New Environment's landscape is a broad flat floor in a basin of low hills, with the camera at eye height on the floor

## 2026-10-30 08:25
- fix(terrain): a large terrain fills in a few chunks a frame, nearest the camera first, instead of freezing the editor while every chunk builds
- feat(editor): a progress card at the top of the viewport counts down a terrain being shaped and built
- fix(editor): New Environment no longer freezes the editor for seconds, and starts the camera in the meadow
- fix(terrain): New Environment's terrain no longer shows lines along its chunk borders

## 2026-10-30 07:53
- feat(editor): the New Environment starter builds a sculpted landscape of meadow and mountains instead of a flat tile

## 2026-10-30 07:47
- feat(editor): the camera's orbit pivot shows as a ring with its distance while you orbit, pan or zoom
- fix(editor): Camera to Cursor (End) puts the pivot on the surface under the mouse instead of the ground plane below it

## 2026-10-30 07:30
- fix(editor): flying the viewport camera slows down near the ground and walls instead of keeping the speed of a distant orbit pivot

## 2026-10-30 07:24
- fix(editor): a selected terrain's outline follows the ground instead of lying flat at its lowest height
- fix(editor): selecting a large terrain no longer costs several milliseconds a frame

## 2026-10-29 17:07
- fix(terrain): Alpine Landscape's textures are mipmapped and compressed, so distant ground no longer looks grainy
- fix(terrain): generating noise over a large terrain runs on every core instead of freezing the editor
- fix(terrain): large terrains no longer lose several milliseconds a frame to their distant stand-ins
- feat(terrain): Alpine Landscape's graph is laid out in labelled bands, one per layer, with nothing overlapping

## 2026-10-29 15:45
- fix(terrain): growing a terrain to thousands of chunks no longer runs the GPU out of memory; only the chunks near the camera are built at full detail
- fix(terrain): the chunk map draws crisp cells instead of a blur
- fix(terrain): a terrain with no material no longer reads "Loading..." in its material slot

## 2026-10-29 14:41
- feat(terrain): the Terrain component is redesigned as a material slot over Manage, Sculpt, Paint and Foliage tabs, in real units throughout
- feat(terrain): the Manage tab has a clickable chunk map with streaming state, size and streaming settings with Apply for rebuilds, and the generator graph card
- feat(terrain): sculpt brushes are grouped, each shows only its own settings, and the falloff is a named curve with a drawn preview
- feat(terrain): Set Height can pick its target height from the ground
- feat(terrain): the Paint tab lists the material's layers with coverage, and exposes slope and height masks
- feat(foliage): foliage types are tiles with blade counts, and the brush size is in metres
- refactor(terrain): the Generate toolbar, the Resize tool and the Edit Terrain dialog are replaced by the Manage tab

## 2026-10-29 14:02
- feat(terrain): a terrain is textured by one node-graph landscape material, chosen in its new Landscape Material section
- feat(materials): Landscape nodes (Layer Coords, Layer Blend, Layer Sample, Slope) for building terrain materials
- feat(terrain): Alpine Landscape, the default landscape material, with grass, gravel, rock and snow placed by slope and height
- feat(terrain): the Paint tab paints the material's layers as weights that always add up to the whole, sized in metres, with a target weight
- feat(terrain): scenes with the old terrain Surface or painted overlay layers convert to a landscape material when opened in the editor
- feat(materials): a material can sample up to 16 different textures, and nodes reading the same texture share one slot
- feat(mcp): terrain tools to set a terrain's material, list its layers and paint them
- refactor(terrain): the overlay paint meshes, the splatmap material and the Terrain Layer material domain are gone

## 2026-10-29 13:44
- feat(terrain): terrain graphs (`.tgraph`), a node graph that builds a terrain's heights and named masks from primitives, shaping, erosion and data nodes
- feat(terrain): a Terrain workspace to edit terrain graphs, with a shaded preview on every node that updates in the background
- feat(terrain): a Terrain Generator component builds a terrain from a graph in the background and keeps its hand sculpting on a layer above
- feat(terrain): Highlands, Rolling Hills and Desert terrain graphs on the Assets panel's create menu
- feat(mcp): a `build_graph` terrain tool builds a terrain graph into a terrain

## 2026-10-29 13:12
- perf(terrain): terrain chunks collide as heightfields instead of triangle meshes, so streaming chunks in no longer stalls the frame
- feat(physics): a heightfield collider shape for the physics contract, with every backend building it
- fix(terrain): a terrain no longer disappears after its chunks stream back in
- fix(terrain): a streamed-out chunk no longer keeps or regrows a collider

## 2026-10-29 12:00
- feat(terrain): a new Eroded noise mode, now the Generate default, makes winding ridges, spurs and soft valleys
- fix(terrain): Erode no longer combs slopes with parallel grooves; gullies come out metres wide with soft sediment floors
- fix(terrain): Smooth and Relax work at a scale set by the brush size, so a big brush softens big features
- fix(terrain): Flatten, Set Height and Ramp leave sloped banks at the brush edge instead of vertical walls
- feat(terrain): Terrace cuts level treads with sloped risers and takes its step height in metres
- feat(terrain): the Cliff brush builds layered rock strata instead of vertical stripes
- fix(terrain): smoothing and levelling brushes work across chunk seams
- fix(terrain): noise no longer jumps at lattice lines below zero coordinates

## 2026-10-29 11:51
- feat(terrain): the default Surface uses new 2k grass, gravel, rock and snow textures with height maps
- feat(terrain): Surface textures are hex-tiled with random turns and shifts, so their repeat no longer shows
- feat(terrain): Surface layers meet along their height maps, so grass settles into rock cracks instead of cross-fading
- feat(terrain): the Surface adds dry and lush patches across the land and keeps distant ground from greying out
- fix(terrain): default snow only covers ground above 190 metres instead of 60
- fix(terrain): a Surface still on the first default textures moves to the new defaults when its scene loads

## 2026-10-29 11:44
- feat(terrain): terrain chunks stream in the editor around the viewport camera, and on by default for a new terrain
- fix(terrain): the Erode button rains in proportion to the terrain's area, so large terrains weather as much as small ones
- fix(terrain): a terrain Surface with anti-tiling on no longer draws black
- feat(terrain): a streamed-out chunk is drawn by a coarse stand-in instead of disappearing, and can still be sculpted
- fix(terrain): a new terrain wears the same blockout grid as a new shape
- fix(terrain): changing a terrain's streaming or detail settings no longer rebuilds every chunk
- fix(terrain): chunks streamed back in no longer show a seam line against their neighbours

## 2026-10-29 11:07
- fix(terrain): terrain drawn as virtual geometry no longer shows pinholes along chunk seams in the distance

## 2026-10-29 10:41
- feat(terrain): terrain is drawn as virtual geometry, with clusters the editor builds in the background as you sculpt and saves for exported games
- feat(export): an exported game ships the virtual geometry of its terrains
- fix(terrain): a new terrain shows the default checkerboard until a Surface is added
- fix(terrain): terrain drawn as virtual geometry is shaded with its Surface instead of one flat dark colour

## 2026-10-29 10:00
- fix(terrain): steep eroded slopes no longer show black streaks under Solari
- fix(solari): terrain is always part of the ray-traced scene, so a scene of only terrain no longer renders black
- fix(terrain): distant terrain switches to coarser detail twice as far away, so the change is much harder to see
- fix(terrain): chunk edges are lit the same on both sides, so seams no longer show as lines on hillsides
- fix(terrain): a sun on the default shadow cascades casts shadows across the whole terrain when zoomed out
- fix(terrain): the terrain inspector's action and tab buttons wrap instead of being cut off in a narrow panel

## 2026-10-29 09:55
- fix(materials): a `.glb` copied into a project without importing it keeps its own materials instead of filling the Console with missing `.material` errors
- fix(scene): placing a model with no scene, such as the virtual geometry `.rvg` beside a `.glb`, no longer leaves the loading overlay stuck
- fix(companion): the virtual geometry `.rvg` kept beside a `.glb` no longer gets a thumbnail of its own

## 2026-10-29 09:36
- fix(terrain): no gaps open between terrain chunks drawn at different detail, even on rough eroded ground
- fix(terrain): smooth, flatten and the other blending brushes no longer bake height layers into the sculpted ground
- fix(terrain): a scripted sculpt stroke moves the ground by the height asked for, and repeated passes add up
- fix(terrain): a scripted Ramp stroke slopes from the ground at its first point to the ground at its last

## 2026-10-29 03:39
- feat(terrain): the Erosion and Hydro brushes really erode, with simulated rain carving gullies and slopes settling, across chunk seams
- feat(terrain): an Erode button in the Generate panel weathers the whole terrain
- feat(terrain): plugins can query ground height, sculpt, generate and erode terrain through `renzora::terrain_tools`
- feat(engine): CC0 rocks and ground plants ship as model presets for dressing terrain
- feat(terrain): brush size, strength and falloff show their values and can be typed
- feat(terrain): a paint layer without a material paints dirt instead of placeholder green
- fix(terrain): painting a layer stays interactive on large terrains
- fix(terrain): foliage painting carries on across chunk seams
- fix(terrain): distant terrain no longer shows dark lines between chunks of different detail
- fix(terrain): terrain undo entries are named for the brush used
- fix(terrain): the Generate panel's controls wrap instead of being cut off in a narrow inspector

## 2026-10-29 02:51
- feat(terrain): distant terrain chunks are drawn with fewer triangles, set by Detail Levels and Detail Distance in Edit Terrain
- feat(terrain): the default Surface keeps grass on gentler slopes, with greener grass and greyer rock
- fix(terrain): terrain is part of Solari's ray-traced scene, so it casts shadows and bounces light

## 2026-10-29 02:32
- feat(terrain): new terrains are textured by rule with grass, gravel, rock and snow by slope and height, editable in the Surface section
- feat(terrain): the engine ships a default set of CC0 ground textures, copied into a project the first time a terrain uses them
- fix(terrain): having a large terrain selected no longer drops the editor's frame rate
- perf(editor): looking up a component by name for reflection no longer scans every registered type

## 2026-10-29 01:59
- fix(terrain): the Terrain inspector has Generate, Resize and Edit Terrain buttons again, so the terrain generator can be reached

## 2026-10-29 01:42
- feat(engine): the default shape grid is a glossier two-tone checker whose lines are fine grooves that glint in the sun
- fix(engine): the default shape grid is sharper up close

## 2026-10-29 01:07
- feat(editor): projects open without waiting for material thumbnails, which the companion renders after loading
- feat(companion): the companion renders material thumbnails and fills in every missing model and material thumbnail once a project opens

## 2026-10-29 00:43
- feat(engine): a project index lists every file in the open project, kept current by the file watcher, for panels to read instead of searching the disk
- feat(companion): the companion watches the project, renders thumbnails for models as they are added or changed, and deletes them with their model
- feat(companion): a Cache screen shows the thumbnail cache and can fill or clear it
- feat(editor): the document switcher shows the document's name without the project's, and its headings stand out

## 2026-10-29 00:23
- fix(editor): a scene's own entity keeps its name when a model in the scene has a node with the same name, instead of becoming `_1` after switching scene tabs
- fix(animation): the Animation workspace looks for model files on a background thread instead of stalling the editor every four seconds
- fix(companion): the profiler's graphs no longer grow the companion's GPU memory while they draw
- fix(companion): a profiled game that closes shows as closed rather than as a failed connection

## 2026-10-28 23:58
- fix(companion): the Debug and Jobs screens scroll with the mouse wheel when their content runs past the window

## 2026-10-28 23:41
- feat(companion): the debug panels are probes on the companion's Debug screen, and the editor's Debug workspace is gone
- feat(engine): a plugin can offer a debug probe the companion shows, without writing any UI
- feat(companion): model thumbnails render in the companion with the editor's own capture
- feat(companion): every companion job waits while a profile records, with Run anyway on the Jobs screen
- feat(companion): frames recorded while a companion job ran are greyed in the frame graph and counted in the profile summary

## 2026-10-28 22:41
- fix(companion): the Render screen's graph holds still while recording instead of rebuilding and flickering as passes swap places
- feat(companion): the profiler's timeline scrolls up and down through its threads with the wheel over their names, Ctrl + wheel, or a drag

## 2026-10-28 22:34
- fix(companion): the Render screen's wires join the nodes they connect instead of piling up to one side
- fix(companion): the profiler's thread lanes leave out the frame-wide wrapper bars, so more threads fit and the GPU lane stays on screen
- fix(companion): the Jobs screen only says jobs are waiting for a recording when one is

## 2026-10-28 22:04
- fix(engine): the editor no longer crashes with a GPU validation error when a blockout shape's grid first draws

## 2026-10-28 21:54
- feat(companion): the Render screen reads top to bottom, one row per group of passes that ran together, and opens at the top at a readable zoom
- fix(companion): the profiler's Slowest in view strip stays on screen, and overlapping span labels no longer print over each other
- feat(editor): a node graph view can open fitted to its width at the top

## 2026-10-28 21:35
- feat(companion): model imports convert, bake their textures and build their `.rvg` in the companion when it is connected
- feat(companion): the companion opens from a spanner beside the Settings gear instead of the status bar and the top-right chip
- feat(editor): a plugin can add an icon-only tool button beside the Settings gear
- fix(companion): the Render screen finds a camera's passes on every thread and lays them out by what ran at the same time
- feat(companion): the Profiler and Render screens are a single column, with frame details in the header

## 2026-10-28 21:14
- fix(companion): the timeline lists the main thread first, then the render thread, then the pools in number order
- fix(companion): the timeline's ruler counts from the start of the frame in view instead of repeating the same time
- fix(companion): long names in Slowest in view are shortened instead of wrapping, and render-world wrappers are left out

## 2026-10-28 21:02
- perf(profiler): recording costs the recorded process about a third less, and frames are encoded off its main thread
- fix(companion): a following timeline keeps its zoom and steps to the newest frame twice a second instead of jumping every redraw
- feat(companion): the timeline has a Follow button, and zooming no longer stops following
- feat(companion): the editor's MCP tools can start a recording and ask the companion for a summary and the causes of slow frames

## 2026-10-28 20:19
- feat(companion): the companion has a profiler with a zoomable per-thread timeline, a frame graph and GPU pass times
- feat(companion): the companion shows the render schedule as a node graph in the order its systems ran
- feat(companion): the companion finds games on the network, connects to them, and saves and opens captures
- feat(companion): texture thumbnails, Rust script compiles, virtual geometry bakes and marketplace uploads run in the companion when it is connected
- feat(export): an Allow profiling option lets the companion on this machine profile the exported game
- fix(scripting): a script without a given hook no longer costs a context build every frame for it

## 2026-10-28 19:16
- feat(engine): the default shape grid is now light cells ruled in thin dark lines, with a heavier dark line around each 4x4 section
- fix(engine): the default grid no longer smears into jagged lines on spheres and other curved shapes
- fix(engine): the default grid stays sharp when the camera is close to it

## 2026-10-28 19:10
- feat(editor): a plugin's status bar text can be clickable, and clicking Companion opens the companion's window in front

## 2026-10-28 18:47
- feat(profiler): every build can record how long each system takes, ready for the companion's profiler; installed plugins rebuild once after updating

## 2026-10-28 18:06
- feat(editor): the editor starts a companion process beside itself for background work, shown in the status bar and opened from the top bar

## 2026-10-28 17:12
- fix(lighting): soft shadows render smooth under TAA instead of staying grainy

## 2026-10-28 13:33
- feat(scripting): games can save named values to slots and load them back with `save_set`, `save_write` and `save_load`
- feat(lighting): lights can cast soft shadows that blur with distance from the object

## 2026-10-28 13:20
- feat(lighting): lights have shadow bias, contact shadow and near-plane fields, and directional lights have cascade count and shadow distance
- fix(lighting): toggling Shadows on the Sun's Directional Light section no longer reverts the next time the Sun changes

## 2026-10-28 13:15
- feat(scripting): scripts can read touch screen fingers, taps, and two-finger pinch and twist

## 2026-10-28 13:11
- feat(scripting): an `on_fixed_update` hook runs once per fixed step, and the physics forces it sets land before the next step

## 2026-10-28 09:48
- fix(materials): a material whose alpha mode or Double Sided was changed in its file recompiles instead of keeping its old shader

## 2026-10-28 09:36
- feat(materials): a Dithered alpha mode draws hair and fur as soft-edged cut-outs instead of a see-through veil

## 2026-10-28 01:14
- fix(solari): ray tracing uses a node-graph material's authored reflectance instead of the default

## 2026-10-28 01:03
- fix(import): spec-gloss models keep their authored reflectivity, so hair and fabric no longer mirror the sky

## 2026-10-28 00:55
- fix(import): hair, leaves and other see-through textures keep their colour when shrunk instead of turning into a dark, hazy veil

## 2026-10-28 00:44
- fix(solari): a glowing mesh with more than 65,535 triangles no longer resets the GPU when it enters a ray-traced scene
- fix(editor): crash reports show their whole message on the next launch, not only its first line

## 2026-10-28 00:31
- fix(editor): when the GPU stops responding, the editor saves a recovery copy of the open scene and restarts instead of failing with nothing saved

## 2026-10-28 00:18
- fix(editor): the Play and Marketplace chips have an even tint instead of fading out halfway across

## 2026-10-28 00:00
- fix(solari): a scene with nothing to ray trace no longer shows a smeared ghost of the previous scene under DLSS
- fix(solari): the Ray Tracing panel's rays-per-pixel figure no longer reads in the millions in a view of only sky
- security: rustls is updated to 0.23.46, which fixes four advisories

## 2026-10-27 23:45
- fix(solari): the Ray Tracing panel's path-traced reference no longer shows point-light stand-ins as glowing balls or alpha-masked leaves as solid cards
- fix(solari): the Solari component can be added by scripts and editor tools, not only from the Inspector

## 2026-10-27 23:22
- fix(solari): tinted glass and sheer curtains near light panels and lamps are no longer covered in bright specks

## 2026-10-27 21:26
- fix(solari): sunlit sheer curtains are no longer covered in white specks in scenes with lamps

## 2026-10-27 20:35
- fix(solari): virtual geometry models no longer show black patches or gaps in curtain folds and at grazing edges
- fix(solari): switching virtual geometry, the denoiser or DLSS no longer drops meshes or sheer curtains out of the ray-traced image

## 2026-10-27 18:37
- fix(rendering): virtual geometry streams a model's full detail in within a frame or two instead of a level at a time
- fix(solari): models no longer show bright and black patches for the first seconds after a scene loads

## 2026-10-27 17:54
- fix(solari): windows, curtains and other see-through surfaces in front of the sky are no longer painted over by it
- fix(solari): sheer curtains glow with the light coming through them from behind

## 2026-10-27 16:38
- fix(solari): large scenes no longer show dark patches of missing bounce light that crawl as the camera moves
- fix(solari): sheer curtains and frosted glass are lit by the sky and stay steady instead of flickering in and out
- fix(solari): the Ray Tracing panel hides the built-in denoiser's figures while DLSS denoises

## 2026-10-27 16:01
- fix(rendering): the back of a double-sided virtual geometry surface is lit like its front instead of dark, and no longer turns black with Solari on

## 2026-10-27 14:44
- feat(editor): Page Down drops each selected entity onto the surface beneath it
- fix(editor): Move to Cursor (V) and Paste land on the surface under the cursor by the model's bottom, and V can be undone
- fix(editor): the mouse pointer stays where it was when right-dragging or middle-dragging the viewport camera

## 2026-10-27 14:30
- fix(editor): scrolling the viewport camera follows the camera speed set with `[` / `]` and the toolbar, as flying does

## 2026-10-27 14:29
- feat(editor): scrolling in slows down as the viewport camera nears a surface instead of overshooting into it

## 2026-10-27 14:13
- fix(editor): getting close to a small model in the viewport no longer slices away the side nearest the camera

## 2026-10-27 14:04
- feat(solari): a Glass Reflections toggle on the Solari component turns ray-traced glass and clear coats on or off

## 2026-10-27 13:43
- fix(solari): bounce light stops adding cells to a crowded world cache, so large enclosed scenes fill it more slowly

## 2026-10-27 11:38
- fix(solari): glass in view no longer makes the rest of the scene's lighting blotchy while the camera moves

## 2026-10-27 11:20
- fix(solari): the back faces of single-sided glass are no longer drawn with Solari on, matching how it looks with Solari off

## 2026-10-27 11:07
- fix(solari): layered glass such as a headlight's lenses draws every layer instead of only the nearest
- feat(solari): the Ray Tracing panel shows the GPU time of the glass and see-through pass

## 2026-10-27 10:48
- fix(solari): reflections in glass and clear-coat surfaces are no longer over three times too bright
- feat(solari): see-through surfaces with a clear coat show its sharp reflection over their own

## 2026-10-27 10:10
- fix(solari): glass and clear-coat surfaces reflect the scene around them instead of only the sky
- fix(solari): meshes with no UVs or normals are ray traced instead of being left out

## 2026-10-27 09:37
- fix(solari): looking into a building from outside no longer shows its interior washed out behind bright volumetric fog

## 2026-10-27 09:09
- fix(solari): bright firefly specks no longer linger on glossy surfaces with the built-in denoiser
- fix(solari): thin edges and silhouettes keep their history with the built-in denoiser instead of fizzing

## 2026-10-27 08:51
- fix(solari): glossy reflections settle smooth with the built-in denoiser while the camera is still, instead of sparkling

## 2026-10-27 08:33
- fix(solari): the built-in denoiser keeps sharp sun-shadow edges and thin shadows instead of blurring a halo around them
- fix(solari): indirectly lit surfaces are no longer about a tenth too dark with the built-in denoiser
- feat(solari): the built-in denoiser keeps shadow edges sharper while the camera moves
- fix(solari): the Ray Tracing panel's path-traced reference is no longer too dark after a stall

## 2026-10-27 08:12
- feat(viewport): the Display, Gizmos, Snap and Camera dropdowns are now submenus of one options menu
- fix(ui): clicking inside a submenu of a toolbar popup no longer closes the popup

## 2026-10-27 07:56
- feat(solari): a ReSTIR toggle on the Solari component overrides the quality preset's choice

## 2026-10-27 07:29
- fix(ui): clicking a dropdown option no longer also presses the control underneath it
- fix(editor): an inspector row that stops applying hides instead of showing a default value

## 2026-10-27 06:51
- feat(solari): a Ray Tracing debug panel measures rays, denoiser history, grain, brightness and GPU time, and shows Solari's internals as viewport views
- feat(solari): a path-traced reference accumulates beside the live image to measure its error against
- fix(viewport): the editor viewport renders long enough for Solari's built-in denoiser to converge before it idles

## 2026-10-27 06:20
- feat(editor): undo and redo moved from the top bar to the foot of the viewport's tool shelf
- feat(editor): the Play and Marketplace buttons are squarer chips with a fading tint that blends into the top bar

## 2026-10-27 05:53
- feat(solari): the default Denoiser choice is labelled DLSS, and the Denoising With readout is removed

## 2026-10-27 05:42
- feat(theme): the Dark theme uses deep blue-tinted surfaces instead of grey, with clearer separation and one blue accent
- feat(theme): the Light theme is redone with distinct chrome, panels and inputs and darker, readable text
- fix(editor): inspector toggle switches use the theme accent instead of a fixed green
- fix(editor): status-bar items stay readable on the Light theme

## 2026-10-27 05:36
- feat(solari): one Denoiser choice (Auto, Built-in, Off) replaces the separate Denoiser and DLSS toggles, and Off shows the raw image in any mode

## 2026-10-27 04:38
- fix(solari): switching DLSS off now hands the image to the built-in denoiser instead of leaving it grainy

## 2026-10-27 01:01
- fix(solari): the whole view no longer turns black with Solari on, after the previous fix
- fix(viewport): only the 8 spot lights nearest the camera cast shadows, so scenes with many spot lights stay fast

## 2026-10-26 10:30
- fix(solari): virtual geometry no longer shows black patches with Solari on when seen from a distance or a low angle

## 2026-10-26 09:21
- fix(solari): `.rvg` models no longer show black patches with Solari on, as Solari now traces their full detail
- fix(solari): a mesh whose geometry is swapped is traced as its new geometry

## 2026-10-26 08:43
- fix(import): `.rvg` models no longer carry a second copy of their full-detail geometry, which made Bistro's 40% larger

## 2026-10-26 02:21
- feat(import): imported models are stored as one `.rvg` file holding the model and its virtual geometry, in place of a `.glb`
- feat(assets): `.rvg` models show in the asset browser as models with thumbnails and drag into the scene like any model
- feat(rendering): an `.rvg` model draws at full detail wherever virtual geometry is off or cannot draw it
- feat(rigging): rigging, weight painting and blend shape sculpting work on `.rvg` models
- feat(editor): Convert Models to RVG in the command palette turns a project's `.glb` models into `.rvg` and updates every scene that uses them
- feat(export): exports pack `.rvg` models with the textures and materials they use

## 2026-10-26 01:06
- fix(rendering): removing a model with glass in it no longer crashes the editor
- fix(rendering): every transparent virtual geometry mesh in view is drawn, not only one per material
- fix(rendering): removing the Standard Geometry component turns a model back into virtual geometry

## 2026-10-25 20:08
- feat(rendering): virtual geometry streams its detail in and out of GPU memory within `[rendering] virtual_geometry_budget_mb`
- feat(viewport): the Visualization menu's Virtual Geometry: Residency view shows where streamed detail is still coming in
- fix(rendering): converted models no longer re-upload all virtual geometry when another model goes off screen

## 2026-10-25 19:40
- feat(rendering): virtual geometry files are compressed to about a third of their size, and rebuild themselves once on the next project open

## 2026-10-25 19:29
- feat(rendering): glass and other blended materials render as virtual geometry, sorted with the rest of the scene's transparency

## 2026-10-25 18:36
- feat(rendering): skinned characters render as virtual geometry, animated on the GPU every frame
- fix(rendering): node-graph materials saved before virtual geometry existed now render as virtual geometry too
- feat(rendering): the Console says why a converted model's meshes stay ordinary geometry
- fix(rendering): models with a dot in their file name render as virtual geometry

## 2026-10-25 18:19
- feat(rendering): node-graph materials render as virtual geometry, graph and all
- feat(plugins): a plugin's material type can opt in to virtual geometry with `add_virtual_geometry_material`

## 2026-10-25 17:47
- feat(rendering): alpha-masked materials such as leaves and grass render as virtual geometry, keeping their outline at a distance

## 2026-10-25 17:15
- feat(viewport): the Visualization menu shows virtual geometry by cluster, triangle, level of detail or instance

## 2026-10-25 16:37
- feat(rendering): virtual geometry, which draws models with opaque standard materials as GPU-culled clusters with per-cluster level of detail, mixed freely with ordinary geometry
- feat(editor): every model in a project is converted to virtual geometry in the background, and again whenever it changes
- feat(export): exports build any missing virtual geometry and pack it beside each model
- feat(editor): a Standard Geometry component keeps a model as ordinary geometry
- feat(project): `[rendering] virtual_geometry` turns virtual geometry off for a whole project

## 2026-10-25 14:34
- fix(export): a model's `.material` files and `.vfx` particle effects are packed into exports
- fix(export): a lean export always links plugins into the binary, since it cannot load plugin files
- fix(game-ui): a shipped game no longer logs a validation error several times a second from the UI template reloader
- fix(solari): Play no longer crashes at start-up with DLSS on, while the Game panel is still tiny

## 2026-10-25 13:48
- feat(export): a Solari game exported from a DLSS build ships with DLSS, compiled into lean binaries and with NVIDIA's libraries beside the executable
- fix(plugins): a native plugin's dependencies are resolved afresh when its dependency list changes
- fix(export): a lean export whose icon file is missing uses the engine's icon instead of failing

## 2026-10-25 12:29
- feat(solari): DLSS Ray Reconstruction with a DLSS toggle and mode dropdown, falling back to the built-in denoiser where it is unavailable
- feat(build): `cargo renzora` builds with DLSS whenever the DLSS SDK is installed, and stages its libraries

## 2026-10-25 09:05
- fix(web): the web editor builds again
- security: rustls and the web template's vite are updated past their published advisories

## 2026-10-25 07:54
- fix(editor): an edit to something the running game keeps moving, like the player, now reaches the game instead of being overwritten
- fix(editor): visibility stays right while the game plays with no viewport on screen, and Stop no longer drops it

## 2026-10-25 07:26
- fix(physics): a body loaded from a saved scene reports its velocity to scripts again instead of always zero

## 2026-10-25 07:05
- fix(editor): menus, dropdowns and dialogs opened over the Game panel now show above the running game

## 2026-10-25 05:51
- fix(physics): a 3D collider with no body is solid again instead of letting bodies fall through it

## 2026-10-25 05:11
- feat(editor): while the game plays, the hierarchy, inspector and viewport show it as it runs, including what it spawns and removes
- feat(editor): Stop puts back what the game changed and keeps your edits, and Keep changes adopts the game's changes instead
- feat(editor): saving while the game plays waits until Stop, so gameplay never lands in the scene file
- fix(editor): a parent link is no longer sent to the running game, where it would point at the wrong entity
- feat(mcp): game_screenshot and game_input let an agent see and play the running game

## 2026-10-25 01:59
- feat(editor): component edits made while the game is playing apply to the running game
- feat(scene): saved entities carry a stable id, so the same entity can be found again in another process

## 2026-10-25 01:51
- fix(scripting): scripts now see this frame's physics velocities, so a script-driven character no longer falls at half speed
- feat(physics): a Mesh collider on an imported model's root makes the whole model solid, and saves with the scene

## 2026-10-24 23:29
- fix(export): an export no longer copies the plugin SDK unless Enable modding is switched on

## 2026-10-24 23:16
- fix(scripting): Rust scripts run in Play again

## 2026-10-24 22:42
- feat(import): the import window can be opened already loaded with given files, with no file dialog

## 2026-10-24 22:41
- feat(export): the export dialog is redesigned, with presets grouped by platform, option cards, feature search, and a summary rail listing what blocks the export with a fix for each
- fix(export): opening the export dialog on a large project no longer fails the runtime-template lookup with "the frame loop is not running"

## 2026-10-24 21:41
- refactor(editor): Blueprints visual scripting is removed: its workspace, graph editor, `.blueprint` files and export capability are gone

## 2026-10-24 21:09
- fix(animation): a stretched shape made bendable keeps its proportions and bends as a tube instead of shearing flat
- fix(animation): moving a control puts every bone the controls own at its pose, so stale clip keys no longer show on untouched face bones

## 2026-10-24 16:02
- feat(animation): Skin mode's Select tool picks a whole part or a box of vertices and binds it rigidly to a bone, sets its weight, or auto-weights it alone
- fix(plugin): plugins build again (system monitor, terminal, Lua, debugger, Rapier physics, 3D text), and staging the plugin SDK no longer takes minutes
- fix(animation): a face rig sends each tooth, gum and tongue with the jaw or the head as a whole, for mouths built as one mesh or several
- feat(animation): Save as Control makes a slider from any pose of bones and blend shapes, for any rig, saved beside the model as a .controls file
- feat(animation): the Animate Controls tab lists, edits and keys a model's controls, with the face expressions as a preset
- feat(animation): a Shapes stage mode sculpts blend shapes on any model with Draw, Smooth, Grab, Inflate, Flatten and Pinch brushes, mirrored across X
- feat(animation): sculpted, renamed and deleted blend shapes save into the model's .glb as morph targets and key like imported ones

## 2026-10-24 14:10
- fix(animation): a face rig's closed lids cover an eyeball that bulges past them instead of passing behind it
- fix(animation): a blink no longer tears the skin sideways past the corners of the eye
- fix(animation): opening a rigged mouth moves each tooth whole instead of stretching it between the jaw and the head
- feat(animation): a Chain rig template bends anything along its length: tails, ropes, tentacles, capsules
- feat(animation): library shapes can be made bendable, from the Chain template or the Shape Library's Bendable option, and curve smoothly along their length
- feat(animation): a Quadruped rig template builds a four-legged skeleton from thirteen points
- feat(animation): the skeleton editor adds, deletes, moves, renames and reparents bones, on a generated, imported or empty skeleton
- feat(animation): automatic skin weights are measured across the skin, so a bone no longer pulls skin it only touches through the air
- fix(animation): rig points land in the middle of the limb under the cursor instead of on its surface
- fix(editor): a model's bones keep their own names while the Animation workspace shows it
- feat(animation): Export with Clips writes a rigged model and its clips to one .glb for other tools
- feat(animation): bone keys have interpolation modes (Auto, Linear, Constant, Flat, Free, Broken) and newly keyed poses ease in and out by default
- feat(animation): the timeline's Curves view is an editable graph editor with key and tangent-handle dragging, channel toggles, pan, zoom and framing
- feat(animation): the dopesheet shows key interpolation by shape and sets a bone key's mode from its right-click menu or key card
- feat(animation): onion skinning on the Animation stage shows ghost poses before and after the playhead
- fix(animation): dragging, editing or undoing bone keys in the timeline now plays straight away instead of after autosave
- feat(animation): two-bone IK, look-at/aim and rotation limit components that adjust animated skeletons at runtime
- feat(animation): foot IK that plants feet on uneven ground through the physics query API, lowers the hips and tilts feet onto slopes
- feat(animation): copy rotation/position constraints with per-axis masks
- feat(animation): spring bones with sphere colliders for hair, tails and ears, independent of frame rate
- feat(animation): script functions to set IK and look-at targets and weights, and to blend or reset spring bones
- feat(physics): partial ragdolls on one bone's subtree, physics/animation blend weight, and a smooth get-up blend back to animation
- fix(animation): event markers now fire on every playing clip, including skeletal-only clips, layers and loops
- fix(animation): a state machine's entry state now plays when the game starts
- feat(animation): state machine states can play a 1D or 2D blend tree, editable in the State Machine panel
- feat(animation): layers support bone masks and additive blending, and play their current clip with crossfades
- feat(animation): play_layer_animation and stop_layer_animation script functions
- feat(animation): root motion moves the entity by the root bone's movement, with optional yaw and an extract-only mode
- feat(animation): clips can be retargeted from another rig with a .skel rest pose and a bone name map
- feat(animation): blend shapes can be keyed in clips and play, blend and layer with the skeleton
- feat(animation): the Face panel's blend shape sliders are keyable, with one timeline track per shape
- feat(import): glTF blend shape animation is imported as blend shape tracks
- feat(import): FBX blend shapes are imported as morph targets, with their animation
- feat(import): skinned models write a rest skeleton beside their clips, and clips from another model are retargeted through it

## 2026-10-24 13:00
- feat(animation): Generate Skeleton rigs the fingers, found from the shape of each hand

## 2026-10-24 12:31
- fix(animation): a blink no longer tears the skin across the bridge of the nose
- feat(animation): a face rig can be built on a head with painted-on eyes, such as a photo scan

## 2026-10-24 10:37
- fix(animation): a face rig's blink closes the eyes fully, measured from each head's own eye opening

## 2026-10-24 10:24
- fix(animation): a model whose bone names collide with another rigged model in the scene keeps playing its clips, on the stage and in the scene

## 2026-10-24 10:03
- fix(animation): a model's clips play again straight after rigging it, removing its rig or reloading its file, without restarting the editor

## 2026-10-24 09:53
- feat(animation): the Face Rig panel shows a head's meshes as one row per kind, with the names on hover, instead of a line per mesh

## 2026-10-24 09:49
- fix(animation): rig points can be placed on any model on the stage, not only on ones that happen to sit clear of the editor's hidden preview objects

## 2026-10-24 09:36
- fix(animation): a face rig's jaw opens the mouth instead of stretching the inside of the lips

## 2026-10-24 09:19
- fix(animation): a face rig splits the jaw's skin where the lips meet, wherever on the lips their points were placed

## 2026-10-24 09:02
- fix(editor): spin sliders (the Face panel expressions, the Pose tab's Turn X/Y/Z, the Material panel's Cutoff) now change the value they show
- fix(animation): posing a model on the stage reaches the real bones again after rigging it

## 2026-10-24 08:18
- fix(build): the plugin SDK now lists the crates inside the shared images, so an installed plugin can use a Bevy-based crate
- fix(build): `cargo renzora` says when it is staging the plugin SDK instead of sitting silent after the build

## 2026-10-24 08:09
- feat(export): an Avian physics engine toggle, off by default for a project that simulates with another physics plugin

## 2026-10-24 05:29
- feat(plugins): bundled plugins, ordinary plugins in the checkout's `plugins/` folder that `cargo renzora` compiles into the binary and that delete with their folder
- refactor(physics): avian is now the bundled `avian_physics` plugin rather than an engine crate
- feat(plugins): an installed plugin can depend on a Bevy-based crate such as a physics engine, built against the engine's own Bevy
- feat(build): `cargo renzora remove` also removes a bundled plugin
- feat(physics): the Console warns when no physics engine is installed

## 2026-10-23 12:38
- feat(materials): a Premultiplied alpha mode for glass, which keeps its reflections at full strength however clear it is
- feat(materials): the Material panel sets a material's alpha mode, mask cutoff and Double Sided
- fix(materials): the material preview shows a procedural material's alpha mode and Double Sided
- fix(editor): undoing the delete of a model no longer leaves the loading overlay stuck on Processing models
- feat(animation): the stage's model picker lists the scene's models and the project's model files, adding a file to the scene when it is picked
- feat(editor): the skeleton gizmo in the viewport draws bones as lines and rings coloured by side, the same as the Animation workspace's stage
- fix(editor): a rigged model's bones no longer fill the Hierarchy; anything attached to a bone still shows under the model

## 2026-10-23 11:38
- feat(physics): joints (fixed, ball, hinge, slider and distance) through the physics API, on any physics engine
- feat(physics): collision layers on colliders, with Layers and Collides With checkboxes in the inspector
- feat(physics): a collider can be turned relative to its entity, with a Rotation row in the inspector
- feat(physics): continuous collision detection for fast bodies, as a toggle on the Physics Body
- feat(physics): collision start and end events carry the contact point, normal and impulse
- feat(scripting): `apply_torque`, `apply_angular_impulse` and `set_angular_velocity`
- feat(physics): overlap queries, and ray and shape casts usable from any exclusive system
- feat(physics): ragdolls, parkour and navmesh obstacles run on whichever physics engine the project picks
- feat(physics): the log says which physics engine runs the 3D and 2D worlds once a project loads
- fix(physics): ragdoll and other joints hold together; avian was built without its joint solver
- fix(physics): removing a body or collider component at runtime takes it out of the simulation
- fix(physics): a collider offset on a 3D body is kept instead of being reset by avian
- refactor(plugins): the `renzora::avian3d` and `renzora::avian2d` re-exports are gone; plugins use `renzora::physics`

## 2026-10-23 10:25
- feat(animation): the Animation workspace is an isolated stage with Rig, Skin, Animate and Logic modes, and tool panels either side that change with the mode
- feat(animation): click a bone on the stage, turn or move it with the gizmo, and press K to key it into the clip
- feat(animation): pose tools: reset to rest, mirror and flip poses, copy and paste poses, two-bone IK for arms and legs, and a pose library saved in the project
- feat(animation): generate a Mixamo-compatible skeleton for a model that has none from seven points clicked on the stage, keeping the original file
- feat(animation): Skin mode: auto weights, a weight brush with heat-map view and mirroring, test bends and range of motion, saved into the model file
- feat(animation): build a face rig on a head from four points, pose it with expression sliders, and generate lip sync from a recording and blinks
- fix(animation): a bone track with a single key holds that pose instead of doing nothing
- fix(animation): a second copy of a rigged model plays its clips on every bone

## 2026-10-23 05:12
- feat(physics): Project Settings > Physics picks the physics engine for the 3D and 2D worlds, and switches the open scene straight away

## 2026-10-23 04:45
- feat(physics): a plugin can register its own physics engine as a backend, with avian as the bundled one
- fix(physics): `apply_force` from a script now pushes the body instead of doing nothing

## 2026-10-23 03:34
- feat(editor): floating panel windows on Windows have the native title bar and join Windows Snap like the main window
- feat(particles): a plasma storm effect in the effect library generator

## 2026-10-23 03:01
- feat(animation): a track height slider on the timeline toolbar
- feat(animation): a new property track appears at the top of the track list, selected and scrolled into view

## 2026-10-23 02:39
- fix(test): the planar reflection shader test no longer fails at random in CI

## 2026-10-23 02:24
- fix(build): `cargo renzora` prints its plugin build progress and no longer repeats every compiler warning
- fix(editor): the themed surfaces no longer log four shader-path warnings on every start
- fix(inspector): a component's label column is only as wide as its longest label, so the value boxes get the rest of the row

## 2026-10-23 01:52
- perf(build): `cargo renzora` compiles your installed plugins itself, so the editor no longer opens a setup window to build them on launch
- perf(build): `cargo renzora` builds incrementally, so a small edit rebuilds in seconds instead of recompiling its whole crate
- fix(editor): a long document name in the scene switcher ends in "…" instead of hiding its save and close buttons

## 2026-10-23 01:02
- fix(editor): closing a document from the scene switcher leaves the switcher open
- fix(2d): Shift-dragging a selected sprite's handle resizes it instead of starting a selection box
- perf(plugin): a Marketplace plugin is compiled during the install, so the restart goes straight into the editor
- perf(build): `cargo renzora` no longer relinks the engine twice per run, and a build with nothing to do takes seconds
- perf(build): switching between `cargo renzora` and a plain cargo build no longer rebuilds the TLS stack and the editor

## 2026-10-22 16:07
- fix(editor): toasts are shown again, above dialogs and the tutorial card
- fix(scene): Save Scene As no longer changes the boot scene
- fix(scene): Save Scene As restores a minimised editor and stays in front of it, so its replace prompt can no longer open hidden
- fix(scene): a remembered scene or tab whose file is gone is dropped on open, instead of becoming the target of every save
- fix(scene): opening a scene by a project-relative path works
- fix(play): the game closes when the editor closes, crashes or is ended
- fix(marketplace): the marketplace's toast stack is no longer saved into the open scene

## 2026-10-22 14:16
- feat(assets): renaming or moving an asset updates the paths written in scripts, including scripts open in the Code Editor

## 2026-10-22 12:42
- fix(assets): renaming or moving an asset updates every object in the open scene that uses it, so saving no longer brings the old path back
- feat(assets): a rename or move warns when a reference to the asset could not be updated

## 2026-10-22 11:45
- fix(editor): pressing Tab in a number field keeps the typed value and moves to the next field

## 2026-10-22 11:08
- fix(hierarchy): Duplicate in the hierarchy right-click menu copies every component, like Ctrl+D, instead of making an empty entity at the origin
- fix(physics): reopening a scene no longer resizes the colliders of rotated objects

## 2026-10-22 09:40
- feat(particles): the effect library generator gains a `popular` set (explosion, fireworks, confetti, healing aura, sparks, coins, hearts, petals, snow, shield, teleport)
- feat(particles): the effect library generator gains a `stress` set for profiling GPU simulation, ribbons, sub-emitters, mesh particles, lights, sorting and CPU collisions

## 2026-10-22 06:21
- fix(particles): GPU emitters no longer crash the editor with a pipeline validation error on the state buffer
- feat(particles): the effect library generator gains a showcase set and an `--only <category>` option

## 2026-10-22 05:34
- fix(svg): the bundled Noto Sans font ships with its SIL Open Font License text

## 2026-10-22 04:58
- feat(particles): solid particles write the deferred G-buffer on deferred cameras, with a per-emitter Render Method (Auto, Forward, Deferred)

## 2026-10-22 04:54
- feat(assets): `.svg` and `.svgz` files load as images everywhere an image is accepted, and hot-reload
- feat(assets): an SVG's output size, sRGB, mipmaps and alpha are set in an optional `name.svg.toml` beside it
- feat(assets): pictures embedded in an SVG load from `data:` URIs or project paths, and editing one redraws the SVG
- feat(assets): SVG text uses the bundled Noto Sans plus the project's own fonts, never system fonts
- feat(editor): SVGs show an image icon and thumbnail in the asset browser and drop into material, particle, sprite, tileset and UI image slots
- feat(export): an exported game ships each SVG with its settings file, embedded pictures and fonts
- fix(export): a reference that climbs out of the project with `../` is no longer packed
- refactor(assets): every image extension list now reads one shared `renzora::IMAGE_EXTENSIONS`

## 2026-10-22 04:02
- feat(particles): any 3D particle can wear a project `.material` (quads, ribbons and meshes) with the material's full PBR shader
- feat(particles): particles wearing a project material run on the GPU backend too
- feat(particles): solid particles cast shadows and write the depth, normal and motion-vector prepasses, on both backends
- feat(particles): a Cast Shadows setting turns shadows on for blended particles or off for solid ones
- feat(particles): Multiply and Alpha to Coverage blend modes, on both backends and in 2D
- feat(material): Particle Color, Particle Age, Particle Random and Particle Velocity nodes read the particle a material is drawn on
- feat(particles): converted Hanabi effects keep their multiply blending

## 2026-10-22 03:58
- feat(particles): sub-emitters run on the GPU backend, every trigger, on both sides of a rule
- feat(particles): ribbons of both kinds run on the GPU backend
- feat(particles): a ribbon can be a round tube, on both backends
- feat(particles): an emitter's Draw Order can be Index, Oldest First, Youngest First or View Depth
- feat(particles): a World emitter can inherit a share of its entity's velocity
- feat(particles): the Particle Output's Emission pin scales the colour's HDR brightness over a particle's life
- feat(particles): emitters joined by a sub-emitter rule share one backend, and Running On names the partner that keeps them on the CPU
- feat(particles): converted Hanabi effects carry their HDR intensity as the Emission pin

## 2026-10-21 20:31
- refactor(particles): the old bevy_hanabi particle system and its editor are removed; `.vfx` is the only particle system
- feat(particles): `.particle` files still play, converted to an effect in memory as they load
- feat(particles): scenes with old Hanabi effects load with a Particle System in their place
- feat(editor): opening a project writes a `.vfx` beside each `.particle` that has none and lists each one in the Console
- feat(editor): scene effects that use a `.particle` are pointed at its `.vfx` once one exists
- feat(editor): double-clicking a `.particle` opens its `.vfx` in the effect editor
- feat(preview): the marketplace preview plays `.vfx` effects and still plays old particle definitions
- fix(editor): the Assets panel and the hierarchy no longer create old-format `.particle` files

## 2026-10-21 19:13
- feat(particles): Gravity Well and Orbit forces, stable at any frame rate, and the Attractor no longer overshoots its point
- feat(particles): a Conform To Sphere force that draws particles onto a sphere and holds them there, as Hanabi's did
- feat(particles): a Collide Scene node bounces particles off the scene's physics colliders (CPU only)
- feat(scripting): `vfx_set_param` and friends set an effect's Parameter nodes by name while it plays
- feat(particles): converted Hanabi effects keep their conform to sphere, attractors and orbit instead of approximating them

## 2026-10-21 18:54
- feat(particles): emitters can draw each particle as a built-in or model mesh that tumbles, follows its velocity or turns about an axis, on the CPU and the GPU
- feat(particles): particles can be lit by the scene's lights, and mesh particles can wear a project `.material`
- feat(particles): Opaque and Alpha Mask blend modes write depth and draw with the scene's solid geometry, with no sort
- feat(particles): particle sizes can be set in screen pixels, so they stay the same size at any distance

## 2026-10-21 17:02
- fix(rendering): Fit to Mesh takes a planar reflection's angle from the glass itself, so a window wall turned off the mesh's axis reflects in the right place

## 2026-10-21 16:34
- fix(rendering): Fit to Mesh puts a planar reflection on the mesh's largest sheet of glass facing the chosen axis, and faces it the way the glass faces
- fix(editor): planar reflection warnings appear in the Console

## 2026-10-21 16:23
- fix(rendering): a planar reflection's plane goes through its mesh (Fit to Mesh) instead of the entity's origin, so imported windows reflect the right place
- feat(rendering): planar reflections gain Flip and Plane Offset, and the gizmo shows the plane actually used, in amber when the mesh is too deep to be one flat surface

## 2026-10-21 16:18
- feat(particles): large emitters simulate and draw on the GPU from the same graphs, with the CPU as the fallback
- feat(particles): an emitter Backend setting (Auto, CPU, GPU), with a Running On row explaining the choice
- feat(particles): emitters can sort their particles back to front
- feat(particles): the preview stats chip shows CPU or GPU
- feat(particles): a GPU Stress template with 200,000 sparks
- feat(materials): mirror and wet ground example materials for testing planar reflections (test project only)

## 2026-10-21 15:24
- feat(particles): curl noise takes octaves, lacunarity and persistence
- feat(particles): a Max Speed output pin and Speed Limit and Drag force nodes
- feat(particles): emitters can light the scene with point lights that follow their particles
- feat(particles): ribbon rendering, as per-particle trails or strips through the particles
- feat(particles): sub-emitters spawn another emitter's particles on birth, death or an interval
- feat(particles): plane, sphere and box collision nodes with bounce, friction and kill on hit
- feat(particles): the fire presets light their surroundings and their embers bounce off the ground
- feat(particles): the .particle converter now maps noise octaves, velocity limits, kill zones, ribbons and the effect light

## 2026-10-21 14:40
- feat(particles): effects can key parameters, bursts and emitter on/off over their own time
- feat(timeline): the Timeline shows and edits the selected particle effect's tracks beside its animation, on one clock
- feat(particles): right-click a value field in the effect graph to Animate it

## 2026-10-21 14:33
- fix(particles): the Particle System inspector shows one transport control (play, pause, stop, restart) with the current state, and the effect slot's edit button beside it
- feat(particles): particles react to the world wind through new Wind and Wind Force nodes
- feat(particles): procedural flame and smoke shading, a Blackbody node and a layered Realistic Fire preset

## 2026-10-21 14:10
- fix(particles): the effect graph's emitters are chips on one header strip with the graph actions, and the preview keeps only its dropdowns, Restart and an options menu over a dark grid by default

## 2026-10-21 13:46
- feat(particles): convert `.particle` effects to `.vfx` from the Assets panel, the Hanabi editor's banner, the Effect slot, or the `convert_particles` example
- fix(particles): opening a `.particle` shows the Hanabi editor in the Particles workspace again

## 2026-10-21 13:01
- fix(editor): asset slots in the Inspector no longer show a reset button that did the same as their clear button

## 2026-10-21 12:56
- feat(editor): a selected Planar Reflection shows its reflecting surface and normal in the viewport, in red with a console hint when the entity has no mesh to reflect on

## 2026-10-21 12:33
- feat(rendering): Planar Reflection component and Add Entity > Mirror, with true reflections that include particles and transparent objects
- feat(particles): a node-graph editor for `.vfx` effects, with live preview, emitter tabs, curve and gradient editing, undo and Save
- feat(particles): New Particle Effect opens a template gallery from the Assets panel and the editor
- feat(particles): `.vfx` files open in the Particles workspace from the Assets panel and can be dragged into the viewport

## 2026-10-21 11:30
- feat(editor): particle systems have a viewport gizmo (a sparkle icon, plus the emitter shape when selected) and can be clicked to select
- fix(editor): the selection box and transform gizmo on a particle system no longer jump around with the particles

## 2026-10-21 10:57
- feat(particles): new node-graph particle system for 2D and 3D (`.vfx`) with a Particle System preset, playback inspector and five built-in effects
- feat(scripting): `vfx_play`, `vfx_pause`, `vfx_stop`, `vfx_restart`, `vfx_burst` and `vfx_set_time_scale` control `.vfx` effects

## 2026-10-21 09:22
- fix(animation): picking another clip while previewing switches the model to it, including after clips were added to an animator one by one

## 2026-10-21 09:17
- feat(editor): the Shape Library gains 53 new shapes, including polyhedra, roofs, turning stairs, pipe fittings and a new Props group

## 2026-10-21 09:11
- feat(timeline): drag the ruler's loop band to move it, and its right edge to set the loop end
- fix(timeline): ruler time labels no longer disappear while dragging a handle
- feat(timeline): All tracks shows a diamond per key time; drag one to retime that moment on every track
- fix(timeline): thinner loop strip line in the ruler
- fix(timeline): the transport no longer shifts as the time readout changes
- feat(timeline): I/O set the loop range at the playhead, Home/End jump to it, Shift+Home/End to the clip ends

## 2026-10-21 08:51
- fix(editor): preview spheres stand pole-up with their UV seam behind them, so UV patterns no longer split across the middle
- fix(editor): Shape Library thumbnails are lit from the side with a rim light so shapes read in 3D instead of as silhouettes

## 2026-10-21 08:41
- fix(editor): opening a material with a rigged model selected no longer crashes the editor

## 2026-10-21 08:20
- feat(editor): the Shape Library shows lit 3D renders of each shape in categories, with a sidebar, foldable sections and a tile-size slider
- fix(editor): Backspace or Delete while typing in a node graph comment or number field no longer deletes the selected nodes

## 2026-10-21 07:52
- fix(editor): material, shader and asset preview spheres no longer show a jagged seam on UV-driven materials

## 2026-10-21 07:46
- feat(timeline): drag the ruler's start handle to set where preview looping restarts
- fix(timeline): dragging a scrollbar no longer moves the playhead
- feat(timeline): drag the playhead by its needle or time badge
- feat(timeline): right-click a track to delete or disable it
- fix(timeline): the All tracks row shows a stack icon instead of a caret that did nothing
- feat(editor): the asset right-click menu leads with its actions, keeps Delete last, and moves the create list into a New submenu
- fix(editor): clicking empty space in the Hierarchy no longer deselects
- fix(editor): a narrow Assets panel switches to the tree-only view sooner instead of squeezing one tile column
- feat(editor): scroll over a mixer pan knob to adjust it, Ctrl+scroll for fine steps
- fix(editor): search overlay section headers and labels are back to the original, smaller style
- fix(animation): adding or choosing a clip in the editor no longer starts the model animating while the timeline is stopped

## 2026-10-21 06:54
- fix(editor): procedural materials show in the Material Preview and asset thumbnails instead of a plain white mesh in deferred projects
- feat(editor): the timeline pans with a middle-drag, or a right-drag on empty space
- feat(editor): the timeline ruler marks the clip as a blue band between a start and an end handle, with more room before time zero
- fix(editor): the timeline shows the ordinary pointer over empty space and a grab hand only over keys
- fix(editor): animation panels keep their entity when the selection is cleared and restore the clip last open on each entity

## 2026-10-21 06:44
- feat(editor): the empty-scene starter in the Hierarchy is a grid of starter tiles with one-click common entities (Cube, Sphere, Plane, Light, Camera, Empty)
- feat(editor): node graphs have a Comment toolbar button and a Comment entry in the right-click menu, and C with nothing selected adds an empty comment at the cursor

## 2026-10-21 06:32
- fix(editor): number fields show an unlimited value such as the default attenuation distance as ∞ instead of 1e37, and accept ∞ or inf when typed

## 2026-10-21 06:26
- feat(editor): node graph nodes use a darker body so they stand out from the canvas

## 2026-10-21 06:15
- fix(import): a model imports into the folder picked in Destination, not the folder the asset browser was showing when it was dropped

## 2026-10-21 06:07
- fix(editor): the Assets breadcrumb folds a long path into one "…" instead of shortening every folder to "…"

## 2026-10-21 05:40
- fix(editor): switching workspace no longer rebuilds the dock a second time, which made the panels flicker
- fix(editor): dragging a cable out of an input pin and releasing it on the canvas opens the add-node menu
- feat(editor): the add-node menu resizes from its corner, and its category column can be dragged wider
- feat(editor): the scene switcher keeps its search and scenes fixed while open documents scroll, and sets the boot scene from a star on each scene
- feat(editor): the Mixer's pan knob is larger, easier to see, and shows its value in the middle
- feat(editor): graph nodes have a dark body, a shadowed edge and short pin labels, with values right after them
- feat(editor): the material graph's toolbar has more room, with the view controls at the right edge
- feat(editor): the Inspector's fixed header sits on a slightly lighter surface

## 2026-10-21 05:02
- feat(editor): the add-node menu is wider, keeps names on one line, shows each node's pins and category counts, and describes the node under the cursor

## 2026-10-21 04:51
- feat(editor): the scene switcher has a search box, project scenes beside open documents, and open documents grouped by type
- feat(editor): the Mixer's strips are wider cards with a full-width colour bar, a dB scale, the bus's inserts and full-width mute and solo
- feat(editor): graph canvases draw a square line grid
- feat(editor): an unconnected graph pin is drawn as a ring, a connected one filled
- feat(editor): the material graph's toolbar uses flat buttons, with Apply as the one filled action
- fix(editor): graph nodes size to their value fields and line them up, instead of letting them spill past the edge
- fix(editor): the code editor stops scrolling once the last line reaches the bottom

## 2026-10-21 04:12
- feat(editor): graph nodes are compact, with each value editor on its pin's row and inputs and outputs sharing rows
- feat(editor): graph node headers fade from the node's colour
- fix(editor): the Timeline, Console and Inspector no longer use flat grey fills

## 2026-10-21 04:01
- feat(editor): the Mixer has Mixer, Routing and Metering pages
- feat(editor): the Mixer's Routing page sets each bus's output and its reverb and delay sends
- feat(editor): the Mixer's Metering page shows peak hold and latched clip lights per bus
- feat(editor): the Mixer's top bar has labelled Compact / Large, Horizontal and Add bus controls
- feat(editor): a Large mixer strip shows a dB scale, and a routed strip shows where it feeds
- feat(ui): ember's segmented control can be two-way bound

## 2026-10-21 02:55
- feat(editor): the Console has a new flat layout with level pills that show counts, a category menu and a Follow button
- feat(editor): clicking a Console line opens its details, with Copy and clickable entity ids
- feat(editor): typing `/` in the Console lists matching commands, completed with Tab
- fix(editor): pressing Enter in the Console's command box runs the command again

## 2026-10-21 02:39
- feat(editor): the Assets tree box-selects files and folders by dragging in its empty space
- feat(editor): Shift+Click on a folder in the Assets tree selects it and everything inside it
- fix(editor): folders selected together in the Assets tree are highlighted
- feat(audio): a bus can be routed into another bus, and can send to the shared reverb and delay

## 2026-10-21 01:52
- fix(editor): selecting a clip in the Timeline frames it to fill the panel
- feat(editor): clips can be renamed from the Timeline's clip picker
- feat(editor): the Timeline's track list can be resized by dragging its edge
- fix(editor): scrolling the Timeline's track list no longer draws the top row over the filter box
- feat(editor): the Inspector's page strip has an All page showing every component, and opens on it
- feat(editor): the Timeline's track list resize bar is thicker
- fix(editor): the Inspector's page tooltips spell out the component count
- fix(editor): the Timeline's zoom readout no longer wraps, and its clip picker has no tooltip
- feat(editor): the Animator component adds clips through a drop area, several at once, and its clip cards are darker

## 2026-10-20 15:47
- fix(editor): the cursor returns to where you pressed after a right- or middle-drag in the viewport

## 2026-10-20 15:31
- feat(editor): the Timeline has horizontal and vertical scrollbars
- feat(editor): the Timeline zooms in much further, and its rows can be made taller from the ⋯ menu or with Alt+wheel
- fix(editor): the Timeline's clip picker has no grey background until hovered

## 2026-10-20 14:24
- feat(editor): the Timeline has a single-row toolbar, a nested track list with a filter, and key runs instead of walls of diamonds
- feat(editor): Timeline box-select, multi-key drag, copy/paste and a key popover for time, value and easing
- feat(editor): a Timeline Curves view of the selected track
- feat(editor): the Timeline's empty state offers Record, Key a property, or Use an existing clip
- fix(editor): Timeline lanes are flat, with no odd/even stripes, and the rows scroll vertically

## 2026-10-20 14:01
- feat(editor): Animator clips are tidy cards with a labelled Speed field and a loop toggle beside the name, and the file path on its own line

## 2026-10-20 13:54
- feat(editor): with nothing selected, the Inspector shows the scene: its world environment's settings, the boot scene and start camera, and entity, mesh and triangle counts
- fix(editor): the Inspector header's eye and lock sit on the name field's line, and the lock turns red while the Inspector is pinned
- fix(editor): Hierarchy label colours show as a soft line down the row's edge and a tinted icon, with no row fills or coloured names
- feat(editor): dialog and overlay close buttons share one darker style
- feat(editor): Add Component, Add Entity and Add Panel category headers sit on a soft band of their colour instead of a line
- fix(editor): the Add Component, Add Entity and Add Panel category sidebar scrolls when it has more categories than fit

## 2026-10-20 13:24
- feat(editor): the Inspector's pages are a strip of icon tabs under the header instead of a rail down its left edge, so the cards use the full panel width
- feat(editor): the Inspector header's name is an always-visible text field

## 2026-10-20 13:19
- fix(editor): panels at the left and right of the window sit as far from its edge as they do from each other
- feat(editor): the Assets breadcrumb shows no pill at rest and takes the toolbar buttons' hover colour

## 2026-10-20 12:59
- feat(editor): the category list in Add Component, Add Entity and Add Panel scrolls to a section instead of filtering, and section headers slide their section open and shut

## 2026-10-20 12:34
- feat(editor): Add Component, Add Entity and Add Panel share one compact picker, three items to a row with each icon beside its name

## 2026-10-20 11:54
- feat(editor): the Assets toolbar has one Import button, New Folder, the breadcrumb, a "Search in folder" box, sort and grid/list buttons, with item count and tile size in a footer
- feat(editor): the Assets breadcrumb folds long paths into "…" and every level opens a menu of its subfolders
- feat(editor): the Assets folder tree switches between Project, Recent and Favourites with a sliding switch instead of a tab row

## 2026-10-20 11:49
- feat(editor): the Inspector shows one page of components at a time, picked by category, with Info and Relations pages
- feat(editor): the Inspector header shows a large icon tile in the entity's colour, its name as click-to-edit text, and what it is and which model it came from
- feat(editor): clicking the Inspector's icon tile opens one popover for the label colour swatches and a searchable icon grid
- feat(editor): the Inspector toolbar leads with a solid Add button and its filter has the magnifier inside
- feat(editor): Add Component groups by the Inspector's pages, opens on the current page, and switches to the added component's page
- fix(editor): sky, fog, atmosphere, terrain and environment map components file under Environment, and SSAO and SSR under Post Process

## 2026-10-20 10:47
- feat(editor): the active document in the switcher's list has a save button, amber while it has unsaved changes
- feat(editor): opening an Inspector component scrolls the panel just far enough to show it
- feat(editor): Add Component and Add Entity tiles are flat until hovered, and long names wrap instead of being cut

## 2026-10-20 10:30
- feat(editor): Add Component and Add Entity show a grid of tiles coloured by category instead of a striped list
- fix(editor): a component registered twice no longer appears twice in Add Component

## 2026-10-20 10:23
- feat(editor): menu rows highlight with a soft accent pill instead of a solid block
- feat(editor): the main menu colour-codes its icons: create, open, save, import, export and Marketplace each have a colour

## 2026-10-20 09:39
- feat(editor): the top bar's switcher names the open document of any kind, such as `test / gold.material`, and lists every open document with a close button
- feat(editor): the document tab strip and its Settings option are removed; open documents live in the switcher

## 2026-10-20 08:44
- feat(editor): the Import overlay's tiles are tinted in their own colour and carry one-word names
- feat(editor): the scene's Saved or Unsaved state shows in the status bar beside Ready
- fix(editor): long values no longer run out of text inputs, so the Inspector's entity name stays inside its box
- feat(editor): the Inspector's entity header sits at the top, with the filter, Add Component and expand-all in one toolbar beneath it
- feat(editor): the Scripts card's "+" moves to its header and opens a searchable script picker overlay
- feat(editor): the Scripts drop zone has no background and outlines itself only while a script is dragged over it
- fix(editor): long names in the hierarchy and asset tree end in "…" instead of being cut mid-letter, with the full name on hover

## 2026-10-20 08:34
- feat(editor): Inspector cards and their headers have a faint dark edge
- feat(editor): number field value bars are rounded at both ends and mark the value with a brighter edge
- fix(editor): number, text and dropdown fields in every Inspector component stretch to the row's width
- fix(editor): the Inspector label column is wider, so longer field names are no longer cut off
- feat(editor): Inspector action buttons span the row instead of repeating their name in a label

## 2026-10-20 06:41
- feat(editor): tree rows in the hierarchy and asset tree indent one caret width per level, so a child's caret sits under its parent's icon
- feat(editor): the assets Import and Add buttons open a centred overlay with colour-coded rows
- feat(editor): the Play button matches the Marketplace button's style
- feat(editor): the top bar is a step lighter than the Windows title bar
- feat(editor): workspace tabs show a hover wash, and the active one is a blue pill instead of an underline

## 2026-10-20 06:05
- feat(editor): a scene switcher at the left of the top bar shows the current scene and its saved state, and opens any project scene
- feat(editor): Play moves to the right of the top bar, beside the Marketplace
- feat(editor): the Scenes panel is removed; the scene switcher does its job
- feat(editor): on Windows 11 the title bar takes the top bar's colour
- feat(editor): the hierarchy and asset tree drop their guide lines and use larger text, icons and rows
- feat(editor): the assets tree button is called Import, and create-asset menus show one line per type
- feat(editor): menus use larger text and roomier rows
- feat(editor): Inspector cards hold their header inset and fully rounded, on a neutral surface

## 2026-10-20 05:29
- feat(editor): each Inspector component is a card with its header inside, and its rows are no longer striped
- feat(editor): the hierarchy and asset tree drop their odd/even stripes and highlight the selected row with a rounded pill
- feat(editor): Inter is the default UI font, with Noto Sans still available in Settings

## 2026-10-20 04:04
- fix(editor): the Game panel registers its window placement through the panel scope, so the test suite passes again

## 2026-10-19 21:09
- fix(ci): the web build uses the wasm-bindgen version the lockfile asks for, so it builds again
- fix(ci): the macOS disk image is retried when the runner briefly holds the app bundle

## 2026-10-19 20:59
- fix(rendering): scenes with many point lights no longer flash wrong lighting for a few frames on load
- fix(rendering): Solari no longer warns about a missing sky light at night
- fix(editor): the striped loading bar's lighter bands are a darker blue

## 2026-10-19 14:18
- fix(editor): pressing Play opens the Game panel with the running game in it, instead of turning the viewport into a blurry square
- fix(editor): an embedded game keeps its place in the Game panel if its window is restyled

## 2026-10-19 14:02
- fix(rendering): the night sky is dark again instead of a flat grey overcast

## 2026-10-19 13:26
- fix(rendering): Solari's Suppress Shadow Maps works again, so lights no longer render shadow maps nothing reads while Solari is on
- perf(rendering): the Solari denoiser's blur passes read a compact per-pixel guide instead of decoding the G-buffer for every tap
- feat(rendering): Solari reports separate GPU timings for each lighting and denoiser stage

## 2026-10-19 12:23
- refactor(rendering): the opt-in DLSS Ray Reconstruction build is gone, since Solari has its own denoiser

## 2026-10-19 12:01
- fix(scene): a scene saved at night no longer loads with the sun shining at full strength from below the horizon

## 2026-10-19 11:19
- feat(rendering): Solari has a built-in denoiser that works on any ray-tracing GPU, with a Denoiser toggle in its inspector

## 2026-10-19 10:42
- fix(rendering): mirror-smooth surfaces such as puddles no longer render black under Solari
- fix(rendering): emissive surfaces are as bright under Solari as without it, so they bloom again

## 2026-10-19 10:21
- fix(rendering): moving the sun under Solari updates the lighting straight away instead of keeping the old sunlight until Solari is toggled

## 2026-10-19 09:33
- feat(import): spec-gloss models bake their glossiness into a roughness texture, so their materials import as plain StandardMaterials
- fix(import): spec-gloss surfaces with no specular, such as brick and plaster, import matte instead of shiny

## 2026-10-19 09:21
- fix(materials): node-graph materials render correctly under deferred rendering and Solari instead of falling back to an approximation
- fix(materials): imported spec-gloss materials no longer turn into mirrors in the deferred and shadow passes
- fix(rendering): Solari now traces meshes with node-graph materials, so they cast shadows, bounce light and appear in reflections

## 2026-10-19 08:38
- change(rendering): Solari ray-traced lighting is built into the engine and no longer a marketplace plugin; delete an installed `plugins/solari` copy
- fix(export): an exported game keeps Solari whenever one of its scenes uses the Solari Ray-Traced GI component

## 2026-10-19 08:12
- change(editor): Play always runs the game as its own process, and the editor stays usable while it runs
- feat(editor): a Game panel shows the running game inside the editor, with Embed Game in Editor to turn it off (Windows)
- change(editor): the play target dropdown, VR Headset play, Scripts (Simulate) mode and in-viewport play are removed
- fix(editor): animated models, cloth and navmesh agents no longer move while editing

## 2026-10-19 06:52
- feat(editor): a model can be dropped on the viewport before it has loaded; a box marks the spot and a loading card shows until it appears there

## 2026-10-19 06:25
- change(plugins): the `#[post_process]` macro, `PostProcessPlugin<T>` and `add_post_process` are removed; a fullscreen effect is an effect file on a Post Process component, and a plugin's own pass uses `add_render_pass`
- change(marketplace): the 53 post-process effects are shader assets in the Shaders category, published to their existing listings as 2.0.0

## 2026-10-19 05:38
- fix(editor): dropping a post-process effect file on the viewport no longer crashes the editor; it adds the effect to the scene's Post Process list
- feat(editor): dropping an effect file on a Hierarchy row adds it to that entity's Post Process list
- fix(materials): a scene that names an effect file as a mesh's material loads with a Console warning instead of crashing

## 2026-10-19 05:02
- feat(rendering): a post-process effect is a single `.wesl` file that declares its own sliders, and a Post Process component runs an ordered list of them on a camera
- feat(editor): drag an effect file onto the Post Process inspector to add it, then reorder, switch off or tune it there

## 2026-10-19 04:01
- fix(build): the engine compiles in a debug build again, which the particle system's Bevy 0.20 update had broken

## 2026-10-19 01:55
- feat(materials): the Material Preview shows a Sprite material live on a sample sprite instead of a sphere
- feat(materials): the add-node menu only offers nodes the material's domain can use, and a node it cannot use is marked red and takes no wires
- fix(materials): the Sprite Output's Color and Alpha pins no longer show a value box that would override the sprite
- feat(assets): a Sprite material's thumbnail is the sample sprite wearing it, tagged 2D
- feat(editor): dragging a material over something that cannot wear it shows a "not allowed" cursor and the drop does nothing
- feat(viewport): a Sprite material dropped on a sprite in the 2D view is applied to it
- feat(editor): dragged nodes in every node graph snap to the grid dots; hold Alt to place freely

## 2026-10-19 00:37
- fix(editor): the viewport stats, node-graph wires and grid, UI canvas rulers and scrollbars, 2D overlay, scale HUD and menu-bar logo spawn again instead of failing, which left the stats text piled in the window's corner
- feat(engine): `RENZORA_SHUFFLE_SEED=<n>` shuffles the order systems run in, to expose ordering bugs

## 2026-10-18 03:03
- refactor(ui): the editor and game UI track hover and press with Bevy's picking instead of the deprecated `Interaction`; plugins use `renzora_ember::UiInteraction` and `interaction::BLOCK` / `PASS_THROUGH`
- feat(ui): a row stays highlighted while the pointer is on a button inside it
- change(game-ui): a game's UI panel only stops a click reaching a 3D object behind it when it blocks

## 2026-10-18 02:24
- feat(materials): sprites can wear material graphs: Add ▸ Sprite Material in the Assets panel, then drag it onto a sprite in the Hierarchy
- feat(materials): a Sprite Color node reads a sprite's own texture at any UV, for waves, wobble and dissolves
- fix(materials): a Sprite material dropped on a 3D mesh, or a 3D material on a sprite, is refused with a Console line instead of failing to draw

## 2026-10-18 02:05
- refactor(materials): the material graph writes its shaders in WESL instead of the old dialect that had to be translated on every load
- fix(shaders): a `.wgsl` file written in WESL loads instead of failing as plain WGSL

## 2026-10-18 01:49
- feat(editor): a plugin or script system that panics is logged to the Console and the editor keeps running, instead of crashing

## 2026-10-17 19:03
- feat(editor): the window title reads "Renzora Engine" with the version and the open project's folder, replacing the version in the status bar
- feat(editor): the editor, launcher and setup windows show the Renzora icon in their title bar and taskbar entry
- fix(runtime): an exported game shows the window icon set in its project again

## 2026-10-17 17:21
- feat(editor): the editor and launcher windows use the operating system's own frame again, so they move, snap and resize like any other window

## 2026-10-17 16:08
- refactor(rendering): Lumen and screen-space GI are removed, leaving Solari as the engine's global illumination

## 2026-10-16 18:28
- feat(viewport): dropping a model that brought its own camera adds a default scene camera that rides it with the source's field of view, so Play shows the imported shot
- fix(import): FBX materials that share a name each keep their own material instead of rendering with the other's

## 2026-10-16 16:30
- feat(import): an FBX animated on its objects (doors, machines, a fly-through camera) imports with its moving parts intact and a clip that plays them
- feat(animation): a Follow Node component keeps an entity on a named node, so a scene camera can ride an imported camera

## 2026-10-16 13:35
- perf(viewport): the viewport renders only when the scene changes, so a heavy scene no longer slows the rest of the editor while nothing moves; Display > Render > Realtime renders every frame

## 2026-10-16 12:39
- fix(materials): imported materials render as plain PBR materials again instead of compiling a shader each, which also puts imported models back into Solari's ray tracing

## 2026-10-16 12:18
- feat(solari): `cargo renzora` builds in NVIDIA DLSS Ray Reconstruction whenever the DLSS SDK is set up, which denoises Solari on an RTX card under Vulkan
- fix(solari): switching Solari off and on again brings the ray-traced lighting back, and no longer switches a Deferred project to forward rendering

## 2026-10-16 11:32
- feat(solari): the Solari component has a Quality preset (Low, Medium, High, Ultra) that trades frame rate against noise
- fix(solari): Solari denoises again with ReSTIR, which Bevy 0.20 had switched off by default
- fix(solari): the Console warns when Solari has no sky light because the Environment Map is off
- docs(solari): the Solari page explains the sky light Bevy 0.20 added and the new Quality presets

## 2026-10-16 10:22
- fix(rendering): resizing the viewport with screen-space GI on no longer leaves the sky black until the camera moves

## 2026-10-15 21:36
- fix(scripting): opening a project no longer drops the editor to a few frames a second while its Rust scripts build
- fix(scripting): a Rust script that has not changed since the last launch loads without being recompiled

## 2026-10-15 21:12
- fix(rendering): sky lighting is no longer rebuilt every frame, which cost frame rate and left black patches in the environment

## 2026-10-15 20:15
- fix(editor): the status bar is back at the bottom of the window, and a floating dock window no longer grows past its own edge
- fix(materials): opening a project no longer freezes the editor while its materials are checked for shader errors

## 2026-10-15 14:53
- feat(engine): the engine runs on Bevy 0.20 and Rust 1.97.1
- feat(shaders): shaders are written in WESL, and `.wesl` files can be opened, edited, previewed and dropped onto meshes like `.wgsl`
- feat(shaders): a `.wesl` shader can import functions from another `.wesl` file in the project
- feat(shaders): shaders written for earlier versions (`#import`, `#ifdef`) keep working, including those in saved materials, themes and plugins
- fix(rendering): glass, water and Scene Color materials still refract the scene behind them
- fix(rendering): the "None" tonemapping mode still keeps colour grading and dithering

## 2026-10-15 13:35
- feat(viewport): a `.wgsl` shader can be dragged onto a mesh in the viewport or the Hierarchy to apply it

## 2026-10-15 06:31
- fix(assets): moving or renaming an asset updates the paths stored in the project's materials, scenes, particles and UI templates, so a moved model keeps its textures
- feat(materials): a `.material` changed on disk outside the Material Editor reloads in the viewport

## 2026-10-15 04:31
- feat(editor): a project opens in a centered 1600x900 window instead of maximizing
- feat(settings): Launcher Effects turns off the launcher cinematic and loading-screen haze, automatically on ultrawide and larger-than-4K displays
- feat(viewport): the log names the active cameras and the environment maps being filtered whenever either changes

## 2026-10-15 03:14
- perf(rendering): environment maps are only re-filtered while their sky, image or camera is changing, instead of every frame
- perf(viewport): the debug visualization and infinite grid no longer re-upload their GPU data every frame
- perf(ui): theme surface materials are only re-uploaded when their colors, image or animation actually change
- perf(editor): the material, shader, studio, particle and camera preview cameras stop rendering when their panel is not the visible tab

## 2026-10-15 02:22
- fix(settings): the renderer backend chosen in Settings now takes effect on restart instead of always falling back to Vulkan

## 2026-09-25 11:11
- fix(scripting): a Rust script that panics no longer brings up the crash dialog, and the Console names the script, the message and the line

## 2026-09-22 18:26
- docs(plugin): the editor API said a plugin cannot make its edits undoable, which stopped being true when the undo core moved to the contract crate

## 2026-09-21 19:17
- feat(camera): a plugin can move the viewport camera, framing an entity or looking from a point at a target

## 2026-09-18 16:27
- fix(game-ui): an `expand` canvas previewed over the viewport lays out like the shipped game, so edge-anchored UI reaches the edge
- fix(game-ui): a canvas root no longer keeps a preview scale that was saved into a scene, which shrank the UI in an exported game

## 2026-09-15 13:52
- fix(linux): the editor window carries an app id, so a dock can show its icon and pinning it works

## 2026-09-15 13:38
- feat(editor): asset context menus can copy project-relative paths

## 2026-09-15 13:16
- fix(plugins): deleting a plugin that is currently loaded works instead of failing on Windows, and the folder is reclaimed at the next start

## 2026-09-15 12:37
- fix(marketplace): updating an installed plugin works while the editor is open, instead of failing with "the directory is not empty"

## 2026-09-15 12:14
- feat(marketplace): an Updates view lists the installed plugins with a newer version published, and updates them
- feat(marketplace): a card says what you already have: grey Installed with a tick, or an amber Update when there is a newer version
- feat(update): the software update dialog lists plugin updates under the engine's own versions
- feat(settings): Settings ▸ Editor ▸ Plugins marks plugins with an update and counts them above the grid
- feat(settings): a Plugin Update Reminders switch decides whether the editor volunteers that a plugin is out of date
- feat(export): the exporter's Plugins tab marks any plugin that would ship out of date
- fix(marketplace): a category's card grid fills the width instead of leaving most of a card's worth of gap at the end of every row

## 2026-09-15 11:46
- feat(marketplace): a plugin states the engine each release was built for, so you are offered the newest version your editor can actually run instead of one that will not load
- feat(marketplace): the plugin store hides listings with no release for your engine, and shows the version you would get rather than the newest one published
- feat(marketplace): the update check now says when a newer version exists but needs a newer editor, instead of showing nothing
- feat(plugins): a plugin can add its own entry to the Assets and Attach create menus
- fix(assets): the Lua Script entry no longer appears when no Lua interpreter is installed
- fix(scripting): a plugin can implement a scripting language again, which fixes Lua
- fix(plugins): a plugin's artwork shows up in Settings and the exporter again, whatever image format it ships
- build(ci): every nightly now checks that the published plugins still compile against the engine it just built, on all six desktop platforms
- build: the macOS and Linux build artifacts no longer carry the application twice, halving them from 1.16 GB to about 660 MB
- feat(sdk): a plugin or Rust script that fails to build now writes the compiler's diagnostic to a `build.log` in its own build directory, so it can be read after the window that showed it has closed
- feat(inspector): a component the graphics quality tier has switched off now shows an amber warning in its header saying so, instead of looking broken
- fix(marketplace): installing a free asset without signing in now counts towards the creator's downloads, which it never did
- fix(marketplace): installing a multi-file asset counts one download instead of two
- feat(marketplace): opening an asset in the editor's store now counts a view, as opening its page on the website already did
- fix(particles): removing a particle effect, or pointing it at a different file, now works instead of leaving the old effect drawing or frozen
- fix(particles): clearing the effect file leaves the emitter empty rather than swapping in a default effect that keeps emitting
- fix(particles): editing a `.particle`'s spawn rate, burst count or timing now takes effect, which only its colours did before
- fix(particles): an effect's light is removed when its effect no longer has one
- fix(scene): a particle emitter no longer saves its live spawner timing into the scene file
- feat(scene): choose what happens when a scene is edited outside the editor while you have unsaved changes: ask, reload, or keep yours (Settings → Editor → Scenes)
- feat(scene): deleting a file the open scene still uses now warns and names the entities using it, instead of the mesh quietly vanishing
- feat(particles): a `.particle` edited outside the editor updates every entity using it
- feat(ui): a `.html` template edited outside the editor rebuilds the canvases using it
- fix(assets): a file saved by an editor that writes to a temporary name and renames it now hot-reloads, which covers most editors and previously reloaded nothing
- feat(engine): the editor watches the whole project, so a file edited outside it is picked up as soon as it is saved
- fix(engine): a file being saved is no longer reported while it is still half-written under a temporary name
- fix(assets): a texture, model or sound edited outside the editor reloads on save, which it never did before
- perf(scripting): a Rust script rebuilds when you save it rather than up to half a second later, and an idle project costs nothing to watch
- fix(scripting): opening a project no longer compiles every Rust script twice
- perf(editor): the Assets panel, the script picker and the material picker no longer re-read the project on a timer to notice files changed outside the editor
- perf(editor): the script and material pickers update from the file that changed instead of re-reading the project, so their cost no longer grows with project size
- perf(editor): the Scenes panel no longer reads the scenes folder off disk on every frame it is open
- perf(editor): the editor no longer stats the active theme's shader files every frame to notice a theme edit
- perf(editor): project fonts and language packs are picked up when they change rather than by re-reading their folders on a timer
- perf(editor): a folder tile in the Assets panel walks its contents once instead of every five seconds for as long as it is on screen
- feat(scene): a scene edited outside the editor reloads in the viewport, and says so instead of reloading when you have unsaved changes
- fix(scripting): the editor no longer stutters twice a second on a project with a lot of files, which the Rust script watcher was walking on the main thread
- fix(editor): the VR on/off line at startup is printed again, having been written before the logger existed and silently dropped
- feat(editor): every editor launch logs whether VR is on or off, and whether pipelined rendering is on
- feat(editor): the console names each viewport camera as it switches on or off, and says why
- fix(editor): the editor no longer loses a third of its frame rate just because an OpenXR runtime is installed; VR editing is opt-in with `--xr`
- feat(plugin): a plugin can use the engine's physics types, so physics plugins no longer carry their own copy
- feat(plugin): a plugin can declare script functions, which only engine crates could do before
- refactor(plugin): Solari, lens distortion, wind and water ship as installed plugins rather than in the binary
- refactor(plugin): there is one kind of installable plugin, and it is an ordinary Bevy plugin shipped as source
- refactor(scripting): a language backend is a plain Rust trait a plugin registers, rather than a C-ABI boundary
- feat(settings): Reset to Defaults no longer offers to clear plugin settings, which plugins now keep as ordinary resources
- refactor(net): the HTTP client is a plain Rust trait rather than a C-ABI boundary
- refactor(audio): the mixer is a plain Rust trait rather than a C-ABI boundary
- fix(net): a backend that panics now fails the requests already handed to it, instead of leaving them parked until their own timeouts
- perf(plugin): plugins build several at a time instead of one after another
- perf(plugin): the plugins that take longest to build start first, so one no longer trails after the rest have finished
- fix(plugin): a plugin's cargo build tree is deleted once it has been built, which was most of what an install kept on disk
- fix(plugin): the setup progress bar follows plugins finishing rather than starting, so it no longer jumps ahead and then stalls
- fix(plugin): a disabled plugin that has never been built now appears in Settings, so it can be turned back on
- fix(ci): the Windows build no longer fails its own MSVC runtime audit when there is nothing to report
- feat(ci): a manual build can target a single arch, such as `windows-x64`, instead of a whole operating system
- fix(windows): the engine, its plugins and exported games link the Visual C++ runtime in, so they start on a machine that has never installed the redistributable
- fix(macos): the editor no longer opens as "damaged" — the `.app` ships signed again
- feat(ci): macOS releases can be Developer ID signed and notarized when the signing secrets are set
