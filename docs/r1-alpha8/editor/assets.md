# Assets and Importing

The **Assets** panel is a file explorer for your project: a folder tree on the left, and a grid or list of the current folder on the right.

<!-- screenshot: assets_grid.png - the Assets panel in grid view, folder tree on the left, mixed asset tiles with previews on the right -->

## Getting around

| Action | How |
|---|---|
| Open a folder or file | Double-click it. |
| Select | Click. `Ctrl+Click` toggles, `Shift+Click` extends a range. |
| Select a folder and everything in it | `Shift+Click` the folder in the tree. Hold `Ctrl` too to add it to the selection. |
| Box select | Left-drag in empty space, in the grid or the tree. Drag near an edge and it scrolls. |
| Move a file | Drag it onto a folder. |
| Put a model in your scene | Drag it into the viewport. |
| Rename | Press `F2`, or click the name of something already selected. |
| Everything else | Right-click. |

Each tile shows its preview over its name and a short line saying what it is. Tiles show the asset rather than a generic icon. Textures show themselves, materials and models show a rendered preview, and a scene shows the snapshot taken when you last saved it. A folder shows a mosaic of up to four images found inside it, with a small folder badge in the corner. Anything without a preview shows its type icon.

File names are shown without their extension, because the icon and its colour already tell you the type. When you rename, the extension is put back for you, so renaming `rock.png` to `boulder` gives you `boulder.png`. Type an extension explicitly to change the type, or end the name with a dot to drop it.

Moving or renaming something here keeps everything that uses it working. The project's materials, scenes, particle effects, UI templates, animations and blueprints are updated to the new path, and the Console says how many files changed. So if a model lands in the wrong folder on import, drag its folder to where it belongs and its materials follow their textures. Scripts are left alone, because a path in code may be built or compared in ways the editor can't safely rewrite.

Do your moving in this panel rather than in Explorer or Finder. A file moved outside the editor looks like a deletion followed by a new file, so the references to it are not updated.

A `.material` changed on disk by anything other than the Material Editor, whether a text editor, a `git pull` or a move, reloads in the viewport straight away.

## Making new assets

The **Import** button on the toolbar lists the assets you can make under its import choices, and the right-click menu in empty space has the same list. Either creates the new file in the current folder.

| | |
|---|---|
| Material | A surface, edited in the [Material Editor](/docs/r1-alpha8/editor/materials). |
| Rust Script | A `.rs` behaviour script. See [Rust Scripts](/docs/r1-alpha8/scripting/rust-scripts). |
| Particle Effect | A `.vfx` effect from a template, edited in the [effect editor](/docs/r1-alpha8/editor/particles). |
| Template | An HTML markup file for [Game UI](/docs/r1-alpha8/scripting/game-ui). |
| Scene | An empty `.bsn` scene. |

<!-- screenshot: assets_new_menu.png - the Assets panel Import overlay open, showing the colour-coded new-asset entries -->

New files start with a small piece of boilerplate. Turn that off under **Settings > Interface > UI Workspace** if you would rather start from an empty file.

## Importing

**Import** on the toolbar opens a window that leads with three choices: **Files**, **Folder** and **Marketplace**, with the list of new assets you can make underneath. Files and Folder are separate because no operating-system dialog can select files and folders in the same pass.

- **Files** is a multi-select file picker, filtered to everything the importer accepts.
- **Folder** picks a directory, walks it for importable files, and recreates its subfolder tree under the destination. A `props/` folder with `crates/` and `barrels/` inside arrives with those subfolders intact.
- **Marketplace** opens the Marketplace to find something there instead.

Either one opens the import overlay pre-loaded with what you picked, and models start converting straight away. You can also drag files or folders straight in from your desktop.

<!-- screenshot: import_overlay.png - the import overlay mid-import with a model converting -->

The same two choices appear under **File** in the main menu, in the right-click menu, and in the command palette.

### What you can import

| Kind | Formats |
|---|---|
| Models | `glb`, `gltf`, `fbx`, `obj`, `stl`, `ply`, `dae`, `abc`, `bvh`, `blend`, `usd`, `usda`, `usdc`, `usdz` |
| Textures | `png`, `jpg`, `jpeg`, `bmp`, `tga`, `webp`, `hdr`, `exr`, `ktx2`, `dds`, `rmip` |
| Vector images | `svg`, `svgz` (see [SVG Images](/docs/r1-alpha8/engine-core/svg)) |
| Audio | `wav`, `ogg`, `mp3`, `flac` |
| Fonts | `ttf`, `otf` |
| Scenes | `bsn` |

Models are converted to `glb` on import. Everything else is copied in as it is.

## The toolbar

From left to right the toolbar has **Import**, **New Folder** (which makes a folder where you are and starts its rename), the path to the folder you are in, a search box, the sort order, and the grid and list views.

The path shows every folder from the project root down. Click one to go there, or click the arrow after it for a menu of that folder's subfolders; the arrow on the folder you are in lists its own. When the path is too long for the toolbar, the folders nearest the root fold into **…** one at a time, keeping the root and the folders closest to where you are, and **…** opens a menu of the folded ones. A long folder name ends in "…" and shows in full when you hover it. The back arrow at the start goes up one folder.

The search box searches the folder you are in, and says which ("Search in scripts"). It matches the real file name, so typing `png` finds every PNG even though the extensions are hidden.

Under the grid, the footer shows how many items the folder holds and how many are selected, with the tile size slider on the right.

The folder tree has a switch in its header to show the **Project** tree, your **Recent** files or your **Favourites**, one list at a time, so a long list of either scrolls on its own. The header says which one is showing.

Narrow the panel below about 480px and the grid goes away: the tree fills the panel and lists files as well as folders, with its own search box and Import button above it. That point is where the grid would no longer fit two tiles beside the tree.

<!-- screenshot: assets_list.png - the Assets panel in list view, the list switch over the folder tree -->

## Dragging assets into your scene

Dragging out of the panel is how most things get used.

- A **model** dropped in the viewport spawns at the drop point.
- A **material** dropped on an object applies to it.
- A **scene** dropped in the viewport or the Hierarchy is added as a nested scene instance.
- A **script** dropped on an entity attaches to it.
- An **image**, SVG included, dropped on a texture slot (a material, a particle emitter's Texture, a UI image's Source, a sprite's Add Sheet) uses it there, and dropped in the 2D viewport makes a sprite.

An SVG shows in the panel with the image icon and a thumbnail, labelled **Vector Image**. Edit it in another program and save, and every place it is used updates while the editor runs. Its optional settings file (`name.svg.toml`, for the output size and so on) sits beside it.

While you are mid-drag, hover over another panel's tab for a moment and that tab springs to the front, so you can reveal a drop target hidden behind another tab without letting go.

## Right-click menu

<!-- screenshot: assets_context_menu.png - the right-click menu on an asset tile -->

On an asset: Open, Rename, Duplicate, Favorite, Copy Path, Reveal in Explorer, then **New**, a submenu with the same colour-coded create-new list the **Import** button opens plus Folder, and **Delete** last, always in reach. On empty space the menu leads with the create-new list, then New Folder, Import and Reveal.
