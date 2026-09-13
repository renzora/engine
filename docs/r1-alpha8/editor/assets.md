# Assets and Importing

The **Assets** panel is a file explorer for your project: a folder tree on the left, and a grid or list of the current folder on the right.

<!-- screenshot: assets_grid.png - the Assets panel in grid view, folder tree on the left, mixed asset tiles with previews on the right -->

## Getting around

| Action | How |
|---|---|
| Open a folder or file | Double-click it. |
| Select | Click. `Ctrl+Click` toggles, `Shift+Click` extends a range. |
| Box select | Left-drag in empty space. Drag near an edge and the grid scrolls. |
| Move a file | Drag it onto a folder. |
| Put a model in your scene | Drag it into the viewport. |
| Rename | Press `F2`, or click the name of something already selected. |
| Everything else | Right-click. |

Tiles show the asset rather than a generic icon. Textures show themselves, materials and models show a rendered preview, and a scene shows the snapshot taken when you last saved it. A folder shows a mosaic of up to four images found inside it, with a small folder badge in the corner. Anything without a preview gets a type icon in that type's colour.

File names are shown without their extension, because the icon and its colour already tell you the type. When you rename, the extension is put back for you, so renaming `rock.png` to `boulder` gives you `boulder.png`. Type an extension explicitly to change the type, or end the name with a dot to drop it.

## Making new assets

The **Add** button on the toolbar, and the right-click menu in empty space, both create a new file in the current folder.

| | |
|---|---|
| Material | A surface, edited in the [Material Editor](/docs/r1-alpha8/editor/materials). |
| Rust Script | A `.rs` behaviour script. See [Rust Scripts](/docs/r1-alpha8/scripting/rust-scripts). |
| Particle | An effect, edited in the [Particle Editor](/docs/r1-alpha8/editor/particles). |
| Template | An HTML markup file for [Game UI](/docs/r1-alpha8/scripting/game-ui). |
| Scene | An empty `.bsn` scene. |

<!-- screenshot: assets_new_menu.png - the Assets panel Add menu open, showing the colour-coded new-asset entries -->

New files start with a small piece of boilerplate. Turn that off under **Settings > Interface > UI Workspace** if you would rather start from an empty file.

## Importing

**Import** on the toolbar opens two choices, because no operating-system dialog can select files and folders in the same pass.

- **Import Files** is a multi-select file picker, filtered to everything the importer accepts.
- **Import Folder** picks a directory, walks it for importable files, and recreates its subfolder tree under the destination. A `props/` folder with `crates/` and `barrels/` inside arrives with those subfolders intact.

Either one opens the import overlay pre-loaded with what you picked, and models start converting straight away. You can also drag files or folders straight in from your desktop.

<!-- screenshot: import_overlay.png - the import overlay mid-import with a model converting -->

The same two choices appear under **File** in the main menu, in the right-click menu, and in the command palette.

### What you can import

| Kind | Formats |
|---|---|
| Models | `glb`, `gltf`, `fbx`, `obj`, `stl`, `ply`, `dae`, `abc`, `bvh`, `blend`, `usd`, `usda`, `usdc`, `usdz` |
| Textures | `png`, `jpg`, `jpeg`, `bmp`, `tga`, `webp`, `hdr`, `exr`, `ktx2`, `dds` |
| Audio | `wav`, `ogg`, `mp3`, `flac` |
| Fonts | `ttf`, `otf` |
| Scenes | `bsn` |

Models are converted to `glb` on import. Everything else is copied in as it is.

## The toolbar

Beyond Add and Import, the toolbar has **New Folder**, a sort order, a grid and list toggle, a tile zoom, and a search box. Search matches the real file name, so typing `png` finds every PNG even though the extensions are hidden.

Down the left of the panel, type filters narrow the view to Scenes, Meshes, Textures, Materials, Audio, Scripts, Shaders or Prefabs.

<!-- screenshot: assets_list.png - the Assets panel in list view with the type filters visible -->

## Dragging assets into your scene

Dragging out of the panel is how most things get used.

- A **model** dropped in the viewport spawns at the drop point.
- A **material** dropped on an object applies to it.
- A **scene** dropped in the viewport or the Hierarchy is added as a nested scene instance.
- A **script** dropped on an entity attaches to it.

While you are mid-drag, hover over another panel's tab for a moment and that tab springs to the front, so you can reveal a drop target hidden behind another tab without letting go.

## Right-click menu

<!-- screenshot: assets_context_menu.png - the right-click menu on an asset tile -->

Open, Rename, Duplicate, Favorite, Reveal in Explorer, Delete, Reimport, and the same colour-coded create-new section the **Add** button opens.
