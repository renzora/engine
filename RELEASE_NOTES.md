# Renzora Engine `r1-alpha8`

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
