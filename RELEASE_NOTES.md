<!-- r1-alpha7 -->

## Unreleased
- fix(viewport): a viewport docked in the global bottom panel takes the camera
  and the tools, instead of leaving them on the workspace's viewport.
- feat(terrain): the sculpt, paint and foliage brush cursors are filled in, shaded
  by the brush's falloff, so you can see the brush and not just its outline.
- fix(terrain): the foliage brush cursor follows the ground and honours the shape
  and falloff its toolbar sets.
- fix(viewport): the toolbar's move / rotate / scale snap steps accept decimals
  again, so a 0.25 grid is typeable.
- fix(assets): New Folder in the toolbar opens the rename field, like the menu
  row already did.
- feat(assets): `project.toml` is hidden in the Assets panel.
- feat(project): a new project no longer gets an empty `plugins/` folder.
- feat(viewport): the axis gizmo's colours are more vibrant.
- fix(settings): View > Reset to Defaults puts autosave back to its default
  without needing a restart.
- feat(settings): View > Reset to Defaults can clear everything installed
  plugins have saved.
- fix(editor): moving the cursor into File > Recent Projects no longer closes the
  menu.
- feat(editor): a hovered menu row is filled with the theme's accent colour.
- feat(viewport): Reset View and Grid moved from the nav cluster to the foot of
  the tool shelf.
- feat(viewport): the Grid button tints its icon while the grid is on instead of
  filling its background.
- feat(viewport): the height ruler moved to the bottom left, and its bar fills
  upwards as the camera climbs.
- feat(editor): the Renzora mark leads the top bar, and opens About when clicked.
- feat(editor): About credits the engine's GitHub contributors.
- feat(editor): About is darker, leads with the Renzora mark, and lays the
  upstream projects out as a grid.
- feat(settings): every editor preference lives in one `~/.renzora/settings.toml`,
  and an older install's settings move across on first launch.
- fix(settings): the two thirds of the Settings pages that were forgotten on
  restart now persist, including every code-editor preference.
- fix(shortcuts): a rebound keyboard shortcut survives a restart.
- feat(settings): camera sensitivity, the grid, gizmos and snapping are per user
  instead of per project.
- feat(project): `project.toml` holds only what a shipped game needs; the scene
  and tabs you had open move to your own settings.
- feat(plugin-api): a standalone plugin can save and load its own settings
  (ABI MINOR 4.11).
- feat(editor): File > Recent Projects opens a recent project without leaving the
  editor.
- feat(editor): leaving a project for another one prompts to save unsaved changes
  first.
- fix(setup): closing the plugin-build window quits instead of reopening it.
- feat(inspector): drag a component section by its grip to reorder it, and the
  order sticks across selections and restarts.
- feat(splash): recent projects are a grid of cards, each showing the last
  snapshot of its main scene.
- feat(project): a new project records the engine version it was created with.
- feat(viewport): the shading buttons move off the scene and onto the toolbar,
  left of Maximize.
- feat(viewport): the select, move, rotate and scale tools move to the shelf down
  the viewport's left edge.
- feat(terrain): the sculpt, paint and foliage modes move to the shelf, above the
  brushes they open.
- fix(terrain): picking a gizmo leaves the terrain mode you were in, instead of
  the mode putting itself straight back.
- feat(editor): Reset to Defaults can reset the tutorial, and resetting settings
  puts the theme back to Dark.
- feat(assets): the Create Asset list leads the Add and right-click menus.
- feat(assets): Rust Script joins the create-asset list.
- feat(assets): New Folder opens its rename field straight away.
- fix(assets): renaming a folder from the grid no longer types into the tree.
- fix(assets): Reveal in Explorer opens the folder you picked instead of the one
  above it.
- feat(shell): the collapsed bottom panel shows its shortcut beside the caret,
  and the whole thing is clickable.
- feat(editor): View has a Reset to Defaults, with a switch per thing it can
  reset: workspaces, editor settings, viewport and camera, and shortcuts.
- fix(editor): resetting editor settings now resets the ones saved between
  sessions too, instead of reading them straight back off disk.
- feat(editor): the UI workspace sits next to Scene.
- feat(inspector): the empty Camera 3D section is gone.
- feat(inspector): UI components are only offered on entities inside a UI canvas.
- feat(shell): the Simulate play target is now called Scripts.
- feat(marketplace): the sign-in and register overlay is redesigned, and has a
  close button.
- fix(ember): the text caret sits on the first character in an input whose box
  has been restyled.
- feat(level-presets): the description block under the grid is gone; the
  description is on each card instead.
- fix(import): dropping a file on the dashboard no longer crashes the app.
- fix(editor): the editor UI no longer renders behind the dashboard, so dragging
  a file over it stops showing "Drop to import".
- fix(splash): picking a language on the dashboard switches the dashboard to it
  straight away.
- feat(splash): the dashboard is translated, in every shipped language.
- feat(splash): opening a project goes straight into it, with no iris transition
  or power-on reveal.
- fix(import): a spec-gloss glTF no longer imports as an untextured white model.
- fix(import): importing a texture, sound, font or script no longer opens the
  model inspector.
- fix(import): the import window closes itself once everything has been imported.
- fix(import): changing an import setting no longer reconverts the model on its
  own; a Reconvert button appears instead.
- fix(import): clicking a file in the Files tab stays on the Files tab.
- fix(import): the column dividers in the import window track the cursor.
- feat(import): the import window's file list is readable, and each row has a
  trash to discard just that file.
- feat(import): a queued file shows a progress bar while it is being converted,
  and the ones still waiting are greyed out.
- fix(import): a converted file no longer appears twice in the import window's
  file list.
- feat(import): the import window shows a progress bar while a preview loads.
- feat(import): the import window's tabs, its green **Import** button (was **Add
  to project**) and its Close all move into one header row, and conversion
  progress moves to the bottom of the preview.
- feat(import): the material preview gets the same environment, lights, grid,
  axis gizmo and zoom buttons as the model preview.
- feat(import): clicking a mesh in the Meshes tab shows that mesh alone, framed
  close up.
- fix(import): the Meshes and Materials tabs show names alone, so a long name
  has the row to itself.
- fix(editor): clicking a folder in a folder picker opens and closes it, not
  just the caret beside it.
- feat(import): the model preview is lit by an environment matching the editor
  viewport's sky and shows it behind the model, with Environment, Lights and
  Grid switches over the viewport.
- feat(import): the model preview has a grid, scaled to the model and placed at
  its feet.
- fix(import): the model preview's default view frames the model closely and
  from a little further above it.
- feat(import): the model preview has an axis gizmo in its top-right corner,
  zoom buttons on its right edge and the lighting switches bottom-right.
- fix(import): a long mesh or material name no longer wraps out of its row in
  the import window's lists.
- feat(import): the import window's Destination tree opens folded, without the
  bordered box around it.
- fix(import): an abandoned import no longer leaves its converted files in the
  project cache forever; opening a project clears them.
- feat(import): the import window's Destination tab uses the editor's folder
  picker, so a new folder can be made without leaving the window.
- fix(import): the Reconvert notice sits above the Import button instead of
  below the fold at the bottom of the settings scroll.
- fix(editor): the viewport can be moved and clicked again after opening a
  project from the File menu.
- fix(shell): the editor leaves when it is told to. Quitting, **Restart Editor**
  after installing a plugin, and the update handoff all went through
  `std::process::exit`, which is not "exit now": it runs libc's atexit chain and
  then every loaded shared object's destructors first. Measured from an AppImage
  build with a project open, that was **7.6 seconds** from the decision to the
  process actually being gone, against 0.4 s for the same teardown done by the
  kernel alone. Restart spawns the successor before that stall, which is why a
  restart put a second editor on screen beside the first one instead of
  replacing it. All three paths now exit immediately, and a restart from an
  AppImage relaunches the `.AppImage` rather than the executable inside its own
  disappearing mount.
- fix(shell): `Alt+F4`, the taskbar's Close and the window manager's × close the
  editor the way its own × does. Bevy answered those by despawning the window,
  which skipped both halves of the editor's quit: the unsaved-changes prompt
  (edits went silently) and the fast exit (the World unwound the slow way, with
  the window already gone).
- fix(export, settings): the plugin grid stops ending in one giant card. The
  cards shared out leftover row space, and flex shares it per row, so a last row
  holding one card handed it the full width. Four equal columns now, however
  many are on the final row.
- feat(setup): the plugin-building window that comes up before the editor now
  looks like the splash screen it precedes: the same title bar, with the icon,
  the product name, the version and the window controls, in place of the OS
  frame that said "setup". The "Setting up Renzora" heading is gone (the bar
  says the name), the progress bar is taller, the caption above it is bigger,
  and the build log fills the window instead of ending in a band of dead space.
- fix(export): the save-before-export prompt gets the padding, the sizing and
  the accent on "Save and export" that every other confirmation in the editor
  has. Its message sat flush against the card border, and the two buttons read
  as interchangeable.
- fix(physics): stairs, ramps, wedges and seventeen other building blocks get a
  collider shaped like the mesh instead of one shaped like its bounding box.
  `stairs` was a solid unit cuboid, so a flight scaled to 6 x 13.6 x 14 was a
  13.6 m vertical wall at the bottom step with a flat lid floating over the
  treads: it could be neither walked up nor jumped onto, and the parkour probe
  saw one ledge too high to mantle where six climbable ones were drawn. A ramp
  was a flat slab, a doorway was a solid panel filling its own opening, pipes
  and torus and funnel were solid, and a hemisphere's collider sank half a
  sphere below the floor it stood on. All now default to a trimesh. Existing
  scene objects keep whatever was saved with them; switch the shape to Mesh in
  the Physics inspector to pick up the fix.
- fix(scripting): a Rust script that fails to compile now says so in the
  Problems panel, against the file and on the line rustc named. It only ever
  reached the Console and the log before, so the panel went on reporting the
  file as clean while the editor quietly kept running the last good build:
  every save appeared to do nothing, with no surface anywhere saying why.
- fix(console): `read_console` returns what the Console panel is showing. It
  read the global log buffer, which is a one-frame handoff queue the Console
  drains empty on every frame, so it reported an empty console however much had
  been logged. The Console now mirrors what it ingests into a history that
  nothing drains, and that is what the tool reads.
- fix(parkour): climbing down a ladder no longer drops the character through
  the world. A ladder climb warps the character, which does no collision at
  all, and the only guard at the bottom was the probe's grounded flag — which
  the auto-attach re-grab defeated: stepping off re-mounted the same ladder on
  the very next frame (the cooldown for exactly this was written and never
  read), and one climb step then took the capsule from above a thin floor slab
  to penetrating it, where `ignore_origin_penetration` makes the grounded cast
  report nothing at all. From that frame the descent was unbounded; measured
  63 m below a 12 mm ground plane, still climbing. The climb is now bounded by
  the ladder's own collider, unioned over its subtree so an imported model
  counts, which needs no floor to be there at all: a ladder over a gap or a
  hatch ends where the ladder ends. Stepping off now also holds the
  `auto_attach` cooldown it always set.
- fix(parkour): a mantle now scales to what it is climbing. Both the duration
  and the arc were constants, so stepping onto a kerb took the same
  `mantle_duration` as hauling over a 2.3 m wall and lifted the character the
  same 0.35 m above the destination on the way up — on a low lip the overshoot
  alone could be most of the height being climbed, which reads as a slow floaty
  hop over something you should have stepped onto. Both now scale with the
  rise, floored at 30% so a shallow climb does not become a teleport and the
  arc still clears the lip.
- feat(parkour): the diagnostic gizmos now highlight the *surface* an action
  would use, not just the point the probe found it at. A vault or mantle
  hatches the top face it would clear or land on, a wall run hatches the
  stretch of wall it would ride (grey when the wall is only jumpable), a ladder
  is marked over its climbable height, and a rope anchor in range draws its
  swing arc. A cross told you the controller found *something*; these say which
  surface it resolved to, which is the question when a mantle aims at the wrong
  shelf or a `Parkour Ladder` sits on the wrong ancestor. Rope anchors are
  found by proximity rather than the forward probe, so they never appear in
  `ParkourProbe` and are queried directly.
- fix(scene): instancing a scene whose path is also the Boot Scene is no longer
  refused as a reference cycle. `spawn_scene_instance`'s guard compared the
  source against `main_scene` rather than the scene being edited, so dropping a
  character prefab into a level while that prefab was still set as Boot Scene
  looked like a self-reference and was declined — and a genuine cycle in the
  open scene went unnoticed for the same reason. Both now compare against
  `editor_last_scene`. Same root cause as the save fix below: `main_scene`
  answers "what does a game boot into", and three places were using it to mean
  "what am I editing".
- fix(scene): pressing Play no longer overwrites an unrelated scene file.
  `save_current_scene` — what the play-mode pre-save and the editor's
  `SaveCurrentScene` event both go through — wrote to `main_scene`
  unconditionally, never consulting the tabs. That is only correct while the
  boot scene and the open scene happen to be the same file. With `main_scene`
  pointing anywhere else, every press of Play serialized the whole scene being
  edited over the top of it: silently, once per press, into a file the user was
  not looking at. It now targets `editor_last_scene`, the active scene tab's
  path and the same source `Ctrl+S` already used, falling back to `main_scene`
  when there is no scene tab.
- fix(scripts): a Rust script outside `scripts/` rebuilds on save. The watcher
  polled `<project>/scripts/` flat while `collect_project_scripts` — the walk
  the project-open build and the exporter share — covered the whole tree, so a
  script kept beside the model it drives compiled at startup and shipped in an
  export but silently stopped hot-reloading: the edit did nothing, the previous
  image stayed loaded, and nothing was logged. The watcher now uses the same
  walk, and tracks mtimes by path rather than by file name so two folders may
  each hold a `player.rs`.
- feat(parkour): `ParkourInput` is reflected and registered, so the Inspector
  shows what a character is being *asked* to do — `move_dir`, `sprint` and the
  three buffered one-shots — beside the `ParkourReadState` that says what it
  did. A character that will not move looks the same whether the script never
  called `parkour_move()` or called it and the controller declined; this is the
  one place that distinction exists. It also lets a tool drive the controller
  without a keyboard.
- fix(animation): a side-loaded `.anim` now binds to its skeleton every time,
  instead of roughly half the time. A track is routed to a bone by name, but the
  importer wrote the source's spelling (`mixamorig:Hips`) while
  `enforce_entity_ids` renamed the live entity to its canonical form
  (`mixamorig_hips`) a frame or so later, unordered against the systems that
  tag bones. Tag first and every curve bound; rename first and none did, and
  `ensure_animation_targets` froze the loser because it only ever filled in
  *absent* ids. The symptom was a character stuck in its bind pose while the
  animator, the current clip and the timeline all reported the clip playing —
  none of which can see whether a curve found a bone. Both sides now go through
  `renzora_animation::bone_target`, which normalizes with the same `sanitize_id`
  the rename uses; it is idempotent, so the two spellings collapse onto one key
  and the ordering stops mattering. Retargeting is unaffected: the key is still
  the bone name and nothing else, which is what lets one `.anim` drive a
  skeleton imported from a different file.
- feat(assets): animation clips get their own icon and colour. A `.anim` shows
  the same runner glyph the hierarchy puts on an animated entity and a `.animsm`
  a node tree, both in teal, and an `animations/` folder takes that teal too.
  They previously fell through to the generic page icon and an "ANIM" label.
- feat(release): every nightly and release now refreshes the browser build at
  renzora.com/engine. The `website` job dispatches the tag to renzora/website,
  which pulls `web-wasm32.zip` onto the droplet and swaps it in behind a
  symlink. The wasm is downloaded rather than committed: the editor module is
  ~100 MB, against GitHub's 100 MiB per-file limit, and it changes nightly.
- fix(terminal): a one-row terminal no longer takes the editor down. vt100 sets
  `scroll_bottom = rows - 1`, so a single-row grid has its scroll region ending
  at row 0, and `col_wrap` then computes `prev_pos.row -= scrolled` as `0 - 1`.
  With overflow checks off that wraps to 65535 and the following
  `drawing_row_mut(..).unwrap()` panics on the compute pool, so the first line
  long enough to wrap killed the process. The derived grid is now floored at 2
  rows and 2 columns, the smallest size with a non-degenerate scroll region.
- fix(shell): the top bar's hamburger dropdown and its submenus paint on the top
  bar's own surface instead of the lighter popup surface, so the menu continues
  the chrome it drops out of rather than reading as a grey card on near-black.
  Every other menu in the editor keeps the popup lift, which is what separates a
  context menu from the panel it covers.
- fix(terminal): the terminal plugin crashed the editor on startup with Bevy's
  B0001. `keep_active_visible` takes `&mut ScrollPosition` filtered to the chip
  strip and `&ScrollPosition` unfiltered, which the conflict check reads as an
  alias and panics on. The two never touch the same entity — the second is only
  ever fetched for the strip's *parent* — so the filter now says so. It surfaced
  when a change to the contract crate restamped every native plugin and this one
  was rebuilt.
- fix(mesh_edit): the modeling shortcuts stand down while the viewport camera is
  being flown. Held right mouse is the fly gesture (`WASD` + `Q`/`E`), and `E` is
  also extrude and `A` select-all — so flying around a mesh in Edit mode extruded
  it and selected every face on the way past. The camera used to resolve this
  from its own side by surrendering `Q`/`E` whenever Edit mode was open, which
  cost vertical navigation for the whole of Edit mode *and did not fix the
  clash*: it gave the key up in the direction that kept it, so `E` while flying
  still extruded. Guarding the shortcuts on right-mouse instead restores vertical
  fly, fixes `A` for free, and matches what `switch_gizmo_mode` and
  `modal_transform_input_system` already do.
- fix(gizmo): the tool-switch shortcuts are Scene-mode only, like the modal
  transform ones already were. `GizmoRotate` defaults to `E` and `GizmoScale` to
  `R`, which are mesh-edit's extrude and (with Ctrl) loop cut — so `E` in Edit
  mode both extruded and threw the scene tool over to Rotate, which
  `enter_edit_mode` then stomped back to `None` a frame later. Any mode that is
  not Scene belongs to the plugin driving it.
- fix(mesh_edit): a selected **face** is visible again in Edit mode. The
  highlight traced the face's own boundary, which put it exactly on top of the
  white edge the overlay had already drawn there — same line, same depth, and
  the depth test is `LESS`, so the second draw lost every time and a selected
  face looked identical to an unselected one. It is now inset toward the
  centroid and lifted a hair along the normal, with faint spokes to the centre
  so it reads as a face rather than a ring. Insetting also settles an ambiguity
  the boundary version had even when it did draw: an outline *on* an edge
  belongs equally to the two faces sharing it, so it never said which side was
  selected.
- feat(viewport): a **shading switch** centred on the viewport's top edge:
  Wireframe / Solid / Material / Rendered. Solid is the matcap
  clay: form without materials or scene lighting, which is what you want while
  modeling; Material is your objects lit; Rendered adds the world around them.
  They are **presets over the existing switches**, not a fifth piece
  of state, and the highlight is derived by comparing the current settings back
  against each one. A stored mode would have gone stale the moment anyone
  touched the Display dropdown or Settings, which edit the same switches, and
  the viewport would have claimed to be in a mode it had left. Derived, a
  hand-tweaked combination lights nothing, which is the honest answer.
- feat(engine): **the blockout grid is a greybox texture now, not a tiled wall.**
  It was rounded tiles separated by dark grout, with a heavy rule around every
  four-by-four section and a cross through that section's middle: three line
  weights and a shape, none of which a greybox needs. The section rule and the
  cross drew a coarse second grid over the first, so a wall read as a pattern of
  large marked panels rather than as a ruled surface and the eye picked out the
  decoration instead of the measure. What is left is the measure: one line
  weight, one cell size, repeating, with a bright four-pointed star on every
  fourth intersection: the cross survives, inverted. It marks the same thing the
  heavy rule did, one section every four cells, but by brightening a single
  intersection rather than boxing in the whole section, so it is a mark you can
  count off to judge a distance without a second set of edges competing with the
  geometry's own. It sits *on* an intersection with its arms along the lines, so
  it thickens the grid where it lands instead of cutting across it. The lines are
  also **brighter** than the face they rule rather than darker, which is what a
  greybox wants, since a light rule carries at distance and at grazing angles
  where dark grout closes up into a smear. A multiply cannot brighten, so the
  line is the part that passes the tint through untouched and the field is held
  below it.
- feat(engine): **every built-in shape starts white.** Each entry in the shape
  registry carried a hue of its own — a red cube, a blue sphere, a green cylinder
  — which made a palette of thirty colours out of a set of parts whose whole job
  is to have no character yet: dropping five shapes into a blockout produced a
  colour chart. Reading a greybox is reading *form*, and colour nobody chose
  competes with that; the blockout grid also needs a neutral tint to multiply
  against, or its lines take the hue too. The tint is still per-shape and still
  yours, for what it is actually good for (colour-coding a route, marking what is
  walkable), and starting neutral is what makes any colour in the scene one you
  put there on purpose.
- fix(engine): the blockout grid is bold enough to see on a lit surface. Its
  field sat close enough to its lines that a flat swatch looked right and a
  sunlit floor did not: a lit face is pushed to the top of the range where
  everything compresses together, and the grid washed out to nothing exactly
  where it was needed. The contrast is sized for the lit case now, which is the
  only one anybody sees.
- fix(viewport): the shape-drag ghost wears the grid the dropped shape will
  wear. Blockout UVs are projected by a system that only looks at primitives
  already in the scene, and the ghost is not one yet, so it carried the shape
  registry's authored unwrap and the texture visibly changed the instant you let
  go. It projects its own now, at the scale it will land at.
- feat(engine): **the blockout grid is projected onto a shape now, not stretched
  across it.** The old system took the mesh's authored UVs and scaled them by the
  object's size, which handled the one case it was written for (dragging a cube
  out into a wall) and nothing else. Its assumption was that the unwrap stays put
  and only the object's size changes, and modeling breaks that immediately:
  extrude and inset build new faces, and an operator that has never heard of a
  blockout grid has no UVs to give them, so the grid smeared into stripes across
  exactly the faces you had just made. Every vertex is now projected along
  whichever axis its normal points down most, in world-scaled object space, so
  the tile size is a property of the world rather than of the mesh's history:
  scale a wall, extrude a ledge, inset a panel, and the grid stays square and the
  same size as the grid on everything else in the scene. Being idempotent is what
  lets it run on whatever the mesh editor last baked with no handshake between
  the two. This replaces `BlockoutTiling`, which existed only to remember what
  the old rescale had already applied.
- fix(mesh_edit): **the mesh editor's wires stopped being drawn twice.** Bevy's
  wireframe pipeline and the editor's own overlay were both drawing the same
  mesh, a fraction of a pixel apart, which reads as one thick smeared line, and
  they disagree about what an edge is: the overlay draws the quads you are
  editing, the pipeline draws the triangles underneath them, so every face gained
  a diagonal belonging to no edge you can select. The global pipeline stands down
  while the editor is open.
- fix(mesh_edit): **vertex and edge markers are drawn for the selection only.**
  Every vertex used to get one, which on anything past a plain cube is a dot on
  every line and several per face: the marker means "this is a thing you can
  click", and a mesh where everything says that says nothing. The wires already
  show where the vertices are, since they are the places wires meet, so the dot
  is spent on the one question the wires cannot answer.
- feat(splash): **start a project from a template.** The dashboard gets a
  **Templates** page and the Projects page a **New from Template** button beside
  New Project, which stays a folder dialog: the empty-project path is one click
  and never waits on the network.

  A starter template **is a project** -- not a description of one, and not a
  library entry to instantiate later. There is nothing to keep locally and
  nothing to install: the download is the finished thing, so it lands in the
  folder you choose and *is* a project from that moment, openable and in your
  recents. The folder is chosen after the template, because the template is the
  interesting decision and a modal file dialog is a bad place to still be making
  it. It refuses a folder that already holds a `project.toml`, and stages the
  extract beside the destination so a download that dies half way cannot leave a
  folder that looks like a project and will not open.

  Project settings come along for free, since the template's `project.toml` is
  the new project's: a template can hand you a resolution, a rendering mode or a
  mixer bus layout already set.

  The page lives in `renzora_marketplace` and registers through the section
  registry `renzora_splash` exposes, the same door the Plugins page uses --
  `renzora_splash` is a dependency of the *runtime*, and reaching for the
  catalogue client from there would put the marketplace's dependency tree in the
  shipped game binary. Unlike the Plugins page it does not filter the unprompted
  view to first-party listings: a plugin is code compiled into the editor
  process, a template is data you look at before committing to it, and the risk
  is not comparable.
- feat(marketplace): **a Starter Templates category.** None of the three that
  nearly fit do: a *prefab* is a piece you drop into a scene you already have, a
  *complete project* is a finished game you open to study, and an *asset pack*
  has no scene and no settings. A starter is the state a project begins in, and
  it is the one category that is a project rather than a part of one -- which is
  the practical difference as much as the taxonomic one, since it means it does
  not install into the open project the way everything else does.
- fix(marketplace): the category icons added since the last engine release all
  rendered as the generic package glyph -- Prefabs, Media, SVGs & Vectors and UI
  Templates. `category_icon` is a hardcoded match and deliberately ignores the
  `icon` column the server sends: the phosphor font is *subsetted* at build time
  from the icon names appearing in the source, so an icon named only in the
  database renders a blank box. The cost of that is an arm per new category, and
  four were missing.
- fix(viewport): **every mesh in the scene could go invisible after visiting
  Wireframe.** Wireframe blanks each `StandardMaterial` (base colour `NONE`,
  alpha-mask discard) and hands the screen to the wireframe pipeline; Solid then
  hands the same materials to the debug-material swap, which leaves them blanked
  because the swap is what renders. Material and Rendered are the *same render
  toggles* as Solid with the swap off, and `update_render_toggles` tracked only
  the toggles — so it read that transition as "nothing changed", returned before
  the restore, and left every mesh discarding its own fragments, with no way back
  short of touching an unrelated toggle. It tracks the visualization mode too
  now, which is the other half of what decides whether it owns the materials.
- fix(mesh_edit): **Tab in and straight back out smeared the texture.** Merging
  coplanar triangle pairs into quads threw away their corner UVs on the
  reasoning that the mesh gets re-baked from the shared vertices anyway — which
  is the problem, not the excuse: the bake falls back to the *welded* vertex UV,
  and welding collapses a cube's 24 render vertices onto 8, so each corner kept
  whichever one of its three UVs happened to win. Every quad then sampled the
  grid across a UV triangle belonging to no face. The merge now carries the two
  triangles' corner UVs into the quad's own perimeter order.
- feat(camera): **`F` frames the selection instead of only re-aiming at it.** It
  called `focus_on`, which sets the orbit's focus point and nothing else — so
  pressing it on a small object across the scene left it the same speck, seen
  from the same angle, and pressing it on an imported model centred the ground
  under the model, because that is where such a model's origin usually sits. It
  now measures the world-space bounds of the selection *and its descendants* (an
  imported model is a parent holding nothing but its parts, so measuring only the
  entity you clicked measures an empty point), centres on those bounds, sets the
  distance from their size against the viewport's own vertical fov, and drops the
  pitch to near the object's horizon — from above, height foreshortens to nothing
  and the silhouette you are usually checking is unreadable. A selection with no
  bounds at all, a light or an empty, still just re-centres: how far back you want
  to be for one of those is a question about the scene around it.
- fix(viewport): the shading switch's Material button was a paint brush, which is
  a tool. These four are *views*: an icon has to say what you would be looking
  at, never what you would be doing. It is a half-lit ball now, which is the
  shading the mode actually turns on.
- feat(viewport): **the shading switch is a ladder, and the world environment is
  the top rung.** Material was "everything except shadows", which made it and
  Rendered nearly the same picture: one soft difference, and the sky dominating
  both. Material now has shadows and no environment, so each button adds exactly
  one thing to the one below it — topology, form, materials and lighting, then
  the world — and Material earns a use Rendered does not cover: judging materials
  and lighting without a sky lighting and colouring them. The environment is
  therefore a switch these buttons *write*, not one derived from the render
  toggles afterwards, since the toggles cannot tell the last two apart. It also
  comes back on its own in play mode and returns to the mode you were in when you
  stop, because the switch hides itself there and could not otherwise be put back.
  The render toggles are saved with the project and the environment flag is not,
  so opening a project lines the two up once: a project last left in Wireframe
  would otherwise reopen showing bare wireframes against a full sky, with no
  button lit to say what mode that was.
- feat(viewport): **Wireframe, Solid and Material turn the world environment
  off**, and Rendered puts it back. Wireframe draws topology and nothing else, so
  a sky behind it is contrast working against the wires; Solid is the matcap,
  which ignores scene lighting by design, so a scene-lit sky behind it states the
  same contradiction twice. The sky is produced by four unrelated
  things that each reconcile every frame from their own source of truth — the
  skybox, the atmosphere, the secondary-viewport sky share, and the clouds
  plugin's dome — so nothing outside them can take it away by removing a
  component: whatever you strip, the next frame puts back. They now agree to
  stand down together, on one shared `EnvironmentSuppressed` flag in the contract
  crate. Nothing in the scene is edited, and the environment returns exactly as
  it was left.
- fix(atmosphere): turning the sky off left a lit ground behind it. The
  atmosphere draws a **ground disc** below the horizon as well as sky above it,
  and that disc is `ground_albedo` lit by the sun rather than anything the medium
  scatters — so routing to the zero-density "off" medium made it *brighter*, not
  dimmer: transmittance goes to 1 and the lit ground reaches the camera
  unattenuated. Wireframe shading showed it as a cream lower half that changed
  hue with the sun. Off now zeroes the albedo as well as the density. The
  authored value is untouched, so switching the sky back on restores it.
- feat(mesh_edit): **edge mode marks its edges.** It used to draw nothing but
  the wireframe, recoloured where an edge was selected, which gives an edge no
  presence of its own: the mesh reads as a cage of lines rather than a set of
  things you can click, and on a dense mesh the recolour is a hairline among
  hundreds. Each edge now gets a faint dot at its midpoint — the same affordance
  vertex mode had, in the place an edge can be aimed at. Drawn as two concentric
  camera-facing circles rather than `Gizmos::sphere`, whose three orthogonal
  rings read as a little wire cage at marker size and turn a mesh's worth of
  them into a thicket. Both these and the vertex dots are now sized from the
  distance to the camera so they stay constant on screen: the fixed 0.03 world
  radius the vertex dots used is invisible on a 60-unit ground plane and
  swallows a half-unit sphere whole.
- feat(mesh_edit): Edit mode has **its own gizmo** — an extrude stalk along the
  selection's normal and an inset ring at its base — instead of the transform
  handles. Three axis arrows are the right handles for an object, because moving
  it is all you can do to one; having selected a face, the thing you reach for is
  extrude, then inset. The transform handles are still one click away on the
  Inspector's Gizmo button, but they are no longer what Edit mode puts up: they
  offered operations a face selection rarely wants while sitting on top of the
  geometry you still needed to click. The hit test is screen-space with a 14 px
  radius and the handles are drawn off the surface along the normal, so anything
  further away falls straight through to vertex/edge/face picking.
- fix(mesh_edit): clicking a panel no longer counts as a click in the viewport.
  `pick_element` never checked `pointer_over_ui`, so pressing the Vertex / Edge /
  Face buttons in the Inspector took the "clicked empty space" path and released
  the mesh being edited — changing select mode deselected the thing you were
  changing it for. The sculpt stroke and the new modeling handles had the same
  hole and are guarded too.
- feat(editor): `renzora::GizmoTarget` — a plugin can **borrow the transform
  gizmo** and drive something that is not an entity's `Transform`. It says where
  the handles go and in which mode; the gizmo says what the drag did, as a delta
  measured from the start of the gesture; the plugin decides what that means and
  records its own undo. The alternative was every plugin drawing its own arrows,
  and the arrows are the easy part: the analytic hit test, the constant on-screen
  handle size per viewport, the Local/World basis, snap and the always-on-top
  material all already exist, and a second implementation gets a different look
  and a worse hit test. Deliberately *not* routed through the global `GizmoMode`,
  which would also re-engage click-picking and box selection — a borrowing plugin
  has switched those off with `ActiveTool::None` because it is doing its own
  picking. `ActiveTool` decides who owns the mouse; this decides who owns the
  handles, and they are not the same question.
- feat(mesh_edit): **transform handles on the mesh selection** in Edit mode,
  through that API. Move / Rotate / Scale, pivoting on the median of the selected
  vertices rather than the object's origin, cycled from a button in the
  Inspector's Mesh Edit section. Until now the only way to move a selection was
  `G` and then `X`/`Y`/`Z`, which is faster once you know it and invisible until
  you do — the only place it was written down was the cheatsheet in the
  Inspector. Off in Sculpt mode, where handles sit on the silhouette being judged
  and would undo the point of `hide_gizmos_while_sculpting`.
- feat(editor): a **Base Color** row in the Material drawer, so the colour of an
  untextured primitive can be changed. A built-in shape wears the blockout grid
  tinted by its `MeshColor`, which is why a spawned cube is red without anything
  being assigned to it — and there was no way to change that colour from the
  editor at all. The row sits directly under the material slot, because "No
  material" is the answer that raises the question, and it hides itself once a
  real `.material` is bound, since the material owns the surface from then on.
  Alpha drives the alpha mode with it.
- fix(engine): a change to `MeshColor` now reaches the material, in the editor
  as well as in a game. It was write-once: every path that *creates* a primitive read it (spawn, undo of a
  delete, scene load, the drag ghost) and baked it into a fresh
  `StandardMaterial`, and nothing watched it afterwards — so the component sat
  in the scene file, on the entity, registered for reflection, and setting it
  did nothing until the next reload. That blocked the new colour row, and it had
  been silently swallowing script edits too, since `ScriptCommand::Spawn`
  inserts a `MeshColor` that a later script could not usefully change. Registered
  in the unconditional `render_3d` block rather than beside `rehydrate_meshes`,
  which lives in the `!is_editor` game-boot branch — the one place a colour is
  edited by hand is the editor, so registering it there would have been a system
  that never ran anywhere it was needed.
- feat(terminal): a **Terminal** panel, as a native plugin. A real shell on a
  real pseudo-terminal (PowerShell on Windows, `$SHELL` elsewhere), so `vim`,
  `htop`, `git add -p`, a dev server and `claude` run full-screen inside the
  dock and behave exactly as they do in a standalone terminal. Full key
  coverage including application-cursor mode, the 256-colour palette over a
  theme-derived default, 10,000 lines of scrollback on the wheel, drag
  selection with Ctrl+Shift+C/V (plain Ctrl+C stays the interrupt), and a
  resize that reaches the kernel so a TUI re-flows. It is a **panel**, and only
  a panel: no command vocabulary and no allow-list, because a terminal that
  filters what may be typed into it is not one. Starts in the project
  directory; keeps running while its tab is hidden, which is required rather
  than polite, since a pty nobody reads fills its kernel buffer and blocks the
  program writing into it.
- feat(terminal): **tabs**. A strip of chips along the top of the panel, one per
  shell, with `+` to open and `x` on a chip to close; Ctrl+Shift+T /
  Ctrl+Shift+W, and
  Ctrl+Tab / Ctrl+Shift+Tab to cycle. Each tab is its own shell with its own
  history, scrollback and selection, and every one of them keeps running in the
  background for the same reason the panel does. Switching is a repaint rather
  than a rebuild: all tabs are the same size, so they are the same rows. Tabs
  are addressed by a never-reused serial rather than by index, because a click
  is read a frame after the chip that carried it was built and a close in
  between would shift every index after it onto the wrong tab. Closing the last
  tab opens a fresh one instead of leaving the panel empty.
- feat(terminal): a **scrollbar** down the right-hand edge of the grid, showing
  where in the history you are and dragging you through it. The whole track is
  the control rather than just the thumb, because a terminal scrollbar is thin
  by necessity and a 10,000-line buffer shrinks the thumb to a couple of pixels
  at the far end. Its gutter is reserved even with nothing to scroll: hiding it
  when unused would change the usable width, and the usable width *is* the
  column count, so the first line of output would resize the shell under a
  program mid-redraw. The history's length is read back from the emulator by
  asking for an impossible scrollback offset and seeing what it clamps to, which
  is one integer assignment either way and beats re-deriving when a line scrolls
  off from the escape sequences that scrolled it.
- feat(terminal): rename a tab by double-clicking it, or from its right-click
  menu (which also has New Tab, Close and Close Others). The chip's label
  becomes an ember text field with the old name selected, committing on Enter or
  on a click away and cancelling on Escape. A typed name survives a shell
  restart, where an automatic one is replaced: the automatic name is a
  placeholder and the typed one is a decision. The terminal's own key handling
  stands down while a rename is open, because the field reads the same
  `KeyboardInput` stream from its own cursor and not gating would deliver every
  keystroke to both, typing the new name into the shell as well.
- feat(terminal): a top strip's tabs **shrink before they clip**, browser-style,
  down to a floor of a few characters plus the close button, so a strip that
  showed four at full width shows eight. Past that floor the active tab is
  **scrolled into view** and a **`v`** menu lists every tab, with a tick on the
  current one and a New Tab row of its own (the `+` rides at the end of the tab
  list, so it has been scrolled out of sight by then). Overflow is decided by
  measuring the laid-out chips rather than counting them or estimating from
  their titles, which is what makes it agree with whatever the theme's font does
  to a chip's width. It cannot oscillate: while the chips have room to give they
  shrink to fill the box exactly and no button is raised, and once they have all
  bottomed out the sum is fixed and taking 20px away for the button cannot bring
  it back under. Scrolling is expressed as a delta against `UiGlobalTransform`,
  which already has the current scroll baked in, so "how far outside the box is
  this chip" is exactly "how far should the scroll move" and there is no content
  width to track or keep in step with a rebuild.
- fix(terminal): the strip's controls no longer disappear once there are enough
  tabs to fill it. A flex item's automatic minimum size is its *content's* size,
  so the box holding the chips had a floor equal to all of them added up: that
  beat its zero flex-basis, grew it past the strip, and pushed the `+`, the `v`
  and the layout toggles off the end at once. The same growth then hid the
  symptom's own cure, since a box grown to fit its content measures as content
  that exactly fits its container, so the overflow check never fired and no
  dropdown appeared to get the tabs back. Two `min: 0`s, the same pair the grid
  body already carried for the same reason.
- feat(terminal): tabs can sit **across the top or down the right**, on two
  toggles at the end of the strip. Side tabs stack, which is the layout that
  works when the panel is tall and narrow or when a row of chips would clip.
  Each side is a different tree rather than a restyle, since a top chip keeps its
  natural width and a side chip fills the column. The strip stays the panel
  root's first child either way and moves to the right by reversing the root's
  main axis, rather than by reordering children: a hierarchy edit that has to
  stay in step with a style change is one that eventually does not. `+` moves
  with the shape - end of the tab list across the top, in the header band beside
  the layout toggles down the side, where it stays put as tabs accumulate rather
  than being pushed further down the column.
- feat(contract): `InputFocusState::plugin_wants_keyboard`, a per-frame claim a
  plugin raises while it owns the keyboard, which the editor ORs into
  `ui_wants_keyboard` and then clears. A panel that reads raw keystrokes could
  not previously hold the editor's own shortcuts off: `ui_wants_keyboard` is
  recomputed every frame from a fixed list of sources (text fields, an editing
  drag value, a focused code editor, play mode) that a plugin cannot join, and
  writing it directly is a race with whichever system runs second. Making it a
  claim rather than a flag is what lets several plugins hold one at once without
  clearing each other's, and lets a plugin release the keyboard by having
  stopped running.
- fix(engine): the `pyramid` primitive is no longer inside-out. Its four side
  faces were built with `edge × to_apex`, which is the *inward* normal for a
  base wound counter-clockwise, and `build_mesh` runs `ensure_correct_winding`
  to rewind every triangle to agree with its stored normal — so the wrong normal
  did not merely shade badly, it turned the faces around. A spawned pyramid
  rendered as an open shell: the sides backface-culled from outside, visible
  from within. It survived because the only test asserted an apex and four base
  corners, which an inverted pyramid has; the new one checks that every face
  points away from an interior point, which geometry alone cannot tell you.
- feat(viewport): a **Matcap** visualization mode, for sculpting and for judging
  a form. Two things it does that lit shading cannot. Its lights are fixed to
  the camera rather than to the world, so orbiting a model no longer changes its
  brightness and everything that appears to move is the shape; and it shades by
  *curvature* as well as by direction, computed from the screen-space
  derivatives of the view normal, so a crease that barely moves the normal —
  which is every fine wrinkle, and therefore invisible under any number of
  lights — reads as a shadow. Analytic rather than a sampled matcap image: no
  asset to ship or fail to load, and the cavity term has to be computed in the
  shader either way.
- feat(mesh_edit): `MeshEditControl` gains a `visualization` field, so a script
  or an agent driving a sculpt can switch the viewport to Matcap before looking
  at what it made. Same reason the `wireframe` and `solid` fields are there:
  `ViewportSettings` is not reflectable from outside the process, and "what am I
  looking at while I work" is part of the same request as "what am I doing".
- feat(mesh_edit): a **sculpt mask**. Masked geometry holds still under every
  brush, which is what makes it possible to work on a limb without the dab
  dragging the torso it grows out of along with it. Paint it with the new Mask
  brush (Ctrl erases), or clear / invert / smooth the whole thing from the
  Inspector or with `Alt+M` / `Ctrl+I`. The value lives on the
  vertex rather than in an array beside the mesh, so dyntopo's splits
  interpolate it and its collapses average it for free, and an operator that has
  never heard of masking cannot desynchronise it. Drawn as an outline around the
  protected region rather than a tint over it: the mask should not sit between
  you and the form you are judging. Every brush goes through one weight
  function, because a mask that some brushes honour and others ignore is worse
  than none — you protect a region, change tool, and lose it without warning.
- feat(mesh_edit): scripted dabs honour X symmetry, via a `symmetry_x` field on
  `MeshEditControl` or the Inspector toggle the interactive brushes already
  read. Mirrored by running the whole brush a second
  time with the centre and direction flipped rather than reflecting the result:
  the mirrored dab has to snap to and refine the *other* side's surface, which
  stops being the reflection of this one as soon as either has been sculpted.
  Without it a caller building a symmetric model issued every dab twice by hand,
  and the two sides drifted apart the moment one list was edited and the other
  was not.
- feat(mesh_edit): a **Snake Hook** brush, plus **Clay**, **Crease** and
  **Scrape**. Snake Hook is what makes a limb: Grab's falloff moves the centre
  of the brush the full distance and its rim none, so walking it along a
  direction shears the same skirt of triangles over and over and an arm pulled
  out of a sphere comes out as a spike. Snake Hook drags the inner 55% of the
  brush rigidly and grades only the outer band, so the cross-section under the
  core is carried forward rather than stretched, and a weak pull toward the axis
  of travel stops the trailing surface webbing. Crease is the wrinkle brush — a
  narrow furrow, which Draw at a small radius cannot make because it pushes a
  round dome; Clay adds mass without lumps; Scrape cuts back only what stands
  above the local plane.
- feat(mesh_edit): **dyntopo in the interactive stroke**. *Dyntopo Detail* in
  the Inspector refines the surface under the brush before each dab and
  decimates what is finer, so a stroke keeps pulling instead of stretching the
  handful of triangles that happened to be there. Expressed as a fraction of the
  brush radius, because it is the ratio that decides what the brush can do and
  an absolute length would need re-tuning on every resize. Off by default: it
  rewrites topology on every dab, and a mode that silently starts remeshing an
  authored mesh the first time it is touched is a mode that loses work.
- fix(mesh_edit): a sculpt stroke with dyntopo on now records its undo step as a
  whole-mesh snapshot. The cheap step is a list of `(index, old, new)`, which is
  only sound while an index means the same thing at both ends of the stroke —
  and dyntopo's collapse pass welds vertices, after which `compact_verts`
  renumbers everything that survived. Undo would have scattered positions across
  the mesh rather than restoring it.
- feat(mesh_edit): a scripted dab snaps onto the nearest surface within twice
  the brush radius, and `last_result` says how far it moved. The interactive
  path raycasts the pointer so its dab is on the mesh by construction; a script
  names coordinates blind, and the surface has usually moved since it last
  looked because every earlier dab in the same stroke deformed it. Taken
  literally, a stroke that starts correctly walks off the surface a fraction at
  a time and the rest of it silently does nothing. The limit is what stops a
  snap from crossing a gap and carving the torso when the caller aimed at an arm.
- feat(mesh_edit): Sculpt mode hides the selection box and the collider
  wireframes, and restores whatever they were on the way out. A sculpt is a
  close read of a silhouette: the orange box cuts across the form being judged,
  and a collider cage sits on it as a bright green wireframe exactly where the
  brush is working.
- refactor(mesh_edit): the Modeling panel is gone; its tools are a **Mesh Edit**
  section in the Inspector, anchored on a component. Which object you are
  modeling is a property of that object, so it belongs where the rest of its
  properties are — and being a component means it round-trips through the scene
  like any other.
- fix(mcp): the MCP server's scene tools refuse to touch the editor's own UI.
  `list_entities` and the despawn/select paths walked every named entity, and
  the editor's panels, ribbon and status bar are named entities: a "clear the
  scene" loop deleted the interface out from under the user. Scene content is
  now anything without a `Node` (or a `UiCanvas`/`UiWidget`, so a game's own UI
  is still reachable), and anything marked `HideInHierarchy` is excluded
  outright. `list_entities` on the same scene went from 1,993 rows to 236.
- feat(viewport): `renzora::ViewportModeRegistry` — a plugin contributes its own
  viewport modes, per view, instead of the contract crate naming them all.
  `ViewportMode::for_view` keeps the built-ins and the registry adds to them, so
  the Mode dropdown and `sanitize_mode_for_view` both describe what this editor
  can actually do. `plugins/mesh_edit` registers `Sculpt` for the 3D view, which
  is what puts it back after it was deliberately dropped from the built-in list
  pending that plugin: hardcoding it would have offered a mode that does nothing
  in an editor without the plugin, and the sanitizer would have reset anything
  that tried to enter it.
- refactor(mesh_edit): the vertex/edge/face modeling and sculpting tools are a
  native plugin (`plugins/mesh_edit`) rather than a crate linked into the
  editor. Edit mode, the six sculpt brushes, loop cut, extrude, inset, array,
  mirror, bisect and subdivide all move out of the binary; **an editor without
  the plugin has no Modeling panel and no Edit/Sculpt viewport modes.** Nothing
  in the 5,600 lines needed a type it could not reach: `renzora_editor_framework`
  turned out to be a facade re-exporting `ActiveTool`, `EditorSelection`,
  `ToolEntry`/`ToolSection` and `SplashState` from the contract crate, and its
  `in_mode` condition was three lines over `ViewportSettings`.
- feat(undo): `renzora::undo::undo_once` / `redo_once`. The pop-run-push half
  moves to the contract crate so a plugin can roll back a command it just
  recorded — an operation that records a topology change and then lets the user
  drag the result has to undo that record when the drag is cancelled, or an
  escaped extrude leaves duplicated zero-length geometry behind. Everything they
  touch was already public there; only the functions sat on the far side of a
  crate a plugin cannot link. `renzora_undo` keeps thin wrappers that add the
  `UndoExhausted` message and the document-tab write, since both need types that
  cannot move.
- feat(shader): `renzora::MaterialNodeCatalog` — `renzora_shader` publishes a
  data-only copy of `ALL_NODES` (node type, category, description, pin names and
  types) into the contract crate at startup, so a consumer that cannot link the
  shader crate can still find out what nodes exist and what their pins are
  called. The definitions themselves cannot move: a `PinTemplate` carries a
  `PinValue`, which is the compiler's vocabulary, and following that chain into
  `renzora` would end with the shader compiler in the contract crate. Strings
  answer the question that was actually being asked. Without this, anything
  outside the crate had to keep its own copy of a 159-entry list and find out it
  had drifted when a graph failed to compile.
- feat(ember): `renzora_ember::workspace::WorkspaceSwitch` — the shell publishes
  its workspace list and active workspace into it every frame and drains a
  `requested` name back, so a plugin can drive the ribbon it cannot name.
  `ShellLayouts` stays `pub(crate)` in `renzora_shell`: a native plugin links
  only `bevy`, `renzora` and `renzora_ember`, and widening that to reach a layout
  list would put the editor's shell crate in every plugin's ABI. Same shape
  `PendingWorkspaces` already uses for registration, in the other direction. A
  request naming an unknown workspace is dropped with a warning rather than
  clamped to an index, because a stale name and an in-range index look identical
  to the caller and only one of them is safe to act on.
- feat(import): `renzora::ImportInPlaceQueue` — push a folder and the editor runs
  every model in it through the import pipeline. The pipeline takes `&mut World`
  and lives in `renzora_import`, so anything that cannot link that crate (a
  native plugin links only `bevy`, `renzora` and `renzora_ember`) previously had
  no way to ask for a real import, and a downloader that wrote a glTF into the
  project produced a model that loads untextured with no `.material` files. The
  walk itself moved out of the marketplace into
  `renzora_import::import_tree_in_place`, which now has three callers that have
  to agree rather than two copies that agree today.
- feat(ember): `register_workspace`, the counterpart to `register_panel_content`.
  A plugin can now contribute the arrangement its panels sit in, not just the
  panels, so a whole editor mode can arrive and leave with its plugin.
- refactor(debugger): the thirteen debug panels and the Debug workspace are a
  native plugin (`plugins/debugger`) rather than a crate linked into the editor.
  The perf tables they read moved to `renzora::diagnostics`, where a plugin can
  reach them, and `renzora_shader`/`renzora_scripting` now publish counts instead
  of the debugger reaching into `MaterialCache` and `ScriptEngine`. **An editor
  without the plugin installed has no debug panels and no Debug workspace.**
- refactor(system_monitor): the status-bar CPU/RAM/GPU readouts are a native
  plugin (`plugins/system_monitor`) rather than a crate linked into the editor.
  It already linked nothing but `bevy` and `renzora`, so the move needed no
  contract changes.
- security(marketplace): a plugin archive containing an absolute path (`/etc/…`,
  `C:\…`, or a bare leading `\`) escaped the install directory and could write
  anywhere the editor could. The guard tested for `..`, which those names do not
  contain, and `Path::join` discards its base when handed an absolute path. Since
  an installed plugin is compiled and loaded, this was a code-execution channel.
  Both extraction loops now use the zip crate's own `enclosed_name` check.
- fix(export): a quick export from an installed Linux editor ships the native
  plugins you actually installed. `editor_dir` was `current_exe().parent()`,
  which inside an AppImage is a read-only temporary mount, so the exporter read
  `plugins/` and `resources/` from the squashed copy rather than from beside the
  bundle where the marketplace installs them.
- fix(import): a malformed USD or Alembic file is reported instead of taking the
  editor down. Offsets in both formats are 64-bit values read straight out of the
  file, and the guards against them were written `if off + 8 > data.len()`, which
  wraps rather than catching anything (this profile has overflow-checks off). A
  crafted or truncated asset panicked inside a slice index, naming an offset
  nobody could connect back to the file. All 28 reads now go through checked
  helpers.
- fix(assets): dragging a file in the tree view no longer opens the rename field
  mid-drag. Pressing a selected item's name arms an explorer-style rename that
  fires 0.45s later, and nothing cancelled it when the press became a drag. It
  showed up in the tree because a row there is one full-width name label, so
  every press lands on it.
- fix(release): the Linux engine zips ship the plugin SDK again. `linux-x64` and
  `linux-arm64` carried the AppImage and nothing else, so an installed editor
  could run no Rust script and no native plugin, and had no way to say why. The
  packager tested for `sdk.tar.zst` with a relative path from inside a subshell
  that had already changed directory, found nothing, and left it out in silence.
  Both Linux zips now carry the whole staged tree, minus the AppDir the AppImage
  already is.
- refactor(audio): the audio engine moves from `plugins/audio` into
  `crates/renzora_audio_backend` and is linked into the binary. It registers
  through the same C-ABI backend contract, so a replacement backend still loads
  from `plugins/`; what changes is that the bundled mixer can no longer go
  missing, and is stripped by `renzora_runtime`'s `audio` feature instead of by
  deleting a file.

## Highlights

- **C-ABI plugins**: write plugins in Rust with **no Bevy dependency**, so one
  built by any rustc loads into any engine. Covers physics, HTTP, audio,
  textures, materials, render passes, input and file dialogs, with hot-swap and
  a `no_std` mode. **Native plugins** ship as Rust source and compile on the
  user's machine.
- **Rust scripts**: per-entity native code with full `World` access, running in
  exported games as well as the editor.
- **A 2D editor**: avian2d physics, tilemaps, sprite animation, 2D lighting and
  particles, and an independent 2D camera per viewport.
- **A UI workspace**: building a menu no longer takes over the scene view, with
  world-space UI on 3D quads and a full visual UI editor.
- **The web editor**: the editor itself compiles for `wasm32`, boots, renders
  and opens a real project.
- **Rendering**: 3D gaussian splatting, FFT ocean, volumetric clouds, GPU
  foliage, one world wind, Solari fixes, streaming and mesh LODs.
- **VR play mode**, with fully decorated eye cameras and a live in-headset
  environment.
- **Editor**: full undo/redo across every panel, floating windows and
  multi-monitor docking, mesh modeling and sculpting, Simulate mode, a global
  bottom panel, and localization in 20 languages.
- **Export**: lean web export, capability auto-detection, `.AppImage`/`.app`
  bundles, and modding on by default.
- **The engine publishes and updates itself**: nightlies, releases, and in-place
  updates from the editor.
- **Binaries are ~87% smaller**: `renzora.exe` 187 MB to 24.9 MB; the installed
  tree ~470 MB to ~77 MB.

## Breaking

- **Rebuild every prebuilt plugin against ABI 4.10.** The C ABI was introduced
  and revised twice this cycle; two of those were majors (3.0 repaired an
  `Interface` table whose functions had been inserted rather than appended, 4.0
  made the three crossing enums newtypes).
- **First-party plugins are no longer in this repository.** Install them from
  the marketplace; `cargo renzora plugin <name>` is gone.

## Contributors

- **Kassinity** ([#92](https://github.com/renzora/engine/pull/92)): shader and
  material compile errors attributed to the graph node that caused them,
  generated shaders validated, WGSL pulled through one `renzora::wgsl` seam
- **Lucas Mundim** ([#96](https://github.com/renzora/engine/pull/96),
  [#98](https://github.com/renzora/engine/pull/98),
  [#102](https://github.com/renzora/engine/pull/102)): unwind symbols stubbed so
  `no_std` plugins load again, a missing `SuppressShadowMaps` tolerated on
  Solari's first extract, exact civil-date crash timestamps
- **saki2fifty** ([#89](https://github.com/renzora/engine/pull/89),
  [#90](https://github.com/renzora/engine/pull/90)): snap settings honoured in
  modal G/R/S transforms, the rotate HUD readout snapped
- **dreamersilly** ([#76](https://github.com/renzora/engine/pull/76),
  [#78](https://github.com/renzora/engine/pull/78)): a green CI build after it
  had been red since 14 June 2026, deleted entities restored under their
  original parent
- **Umut Faruk** ([#87](https://github.com/renzora/engine/pull/87)): directory
  drag-and-drop import
