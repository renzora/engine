<dl>
  <p align="center">
  <img width="150" height="150" src="https://i.imgur.com/tsOyrhG.png" title="Hazel Lockwood Is the Main Character of Renzora & Logo of Renzora Engine">
  </p>

  <h1>Renzora Engine [Alpha] 0.1.0</h1>

  <h3>Renzora Engine is a cross-platform Javascript game engine with support for BabylonJS Webgl & Webgpu</h3>
</dl>

![image](https://i.imgur.com/t2XufWb.png)

# Notice
This is a very early version of the engine and is only meant for evaluation and not for actual usage. Please expect things to break or not work. I will be actively developing the engine and you can join the discord server to discuss features, bugs and commits here: https://discord.gg/9UHUGUyDJv

# Installation
`git clone https://github.com/renzora/engine`

`cd engine`

`npm install`

# Dev server
`npm run dev` then visit `https://localhost:3000`

# Electron dev server
`npm run electron:dev`

# Current Features
- Project creation
- Directory Sync
- 3D viewport render
- Scene Objects
- Scene Properties
- Node Editor

# Features in development
- 2D support
- Physics Engine (Havok)
- collisions
- Terrain Editor
- Scripting Api
- Plugin/themes importing
- Adding new levels
- Full Node editor
- ui designer
- Migrating from React to Preact
- Code Editor
- Cloud based assets/sync (google drive, dropbox)
- Instancing/optimisations
- Adding support for Tauri
- Fbx to gltf/glb conversion
- Draco Compression
- Animation/rigging
- Level Designer
- Lighting System
- Particle Editor
- Shader Support
- ThreeJS support
- Post Processing
- backend authentication
- websocket management
- auto updating

# Issues/bugs
- undo/redo not implemented yet
- Cannot save projects or make levels yet
- Switching from webgl to webgpu will clear your scene (persistent state will be coming soon)
- Node editor doesn't do anything yet
- Some issues with file syncing not updating.
- Scene object tree has minor issues especially with dragging objects to folders
- webgpu should work, if it doesn't then run the .bat or ps1 tool to generate fake ssl certificate
- issues with packing and building to electron. dev server works fine
- some issues with camera movement
- Software update doesn't work yet
- Lights can't be positioned yet
- Scene background cannot be changed yet
- Scripting API not implemented yet
- Top Menu items don't work
- Click event issues with context menu (some context menu items don't work)

# Keyboard shortcuts
- `R` Rotate Object
  
- `S` Scale Object
  
- `G` Move Object

- `End` Snap object to nearest surface (can be buggy)

- `tab` Node Editor

- `Right click + W` camera forward

- `Right click + A` Camera Left

-  `Right click + S` Camera Back

-  `Right click + D` Camera Right

-  `Right click + Q` Camera Up

-  `Right click + E` Camera Down

-  `Right click + C` Camera FOV Back

-  `Right click + Z` Camera FOV Zoom

# Mouse
`Right click + drag` 360 pan

`Scroll` Zoom

`Left click` Object Selection

