<dl>
  <p align="center">
  <img width="150" height="150" src="https://i.imgur.com/tsOyrhG.png" title="Hazel Lockwood Is the Main Character of Renzora & Logo of Renzora Engine">
  </p>

  <h1>Renzora Engine [Alpha] 0.1.0</h1>

  <h3>Renzora Engine is a cross-platform Javascript game engine with support for BabylonJS Webgl & Webgpu</h3>
</dl>

![image](https://i.imgur.com/FIoVtMp.png)

# Notice
This is a very early version of the engine and is only meant for evaluation and not for actual usage. Please expect things to break or not work. I will be actively developing the engine and you can join the discord server to discuss features, bugs and commits here: https://discord.gg/9UHUGUyDJv

# Current Features
- Project creation
- Directory Sync
- 3D viewport render
- Scene Objects
- Scene Properties
- Node Editor

# Installation
`git clone https://github.com/renzora/engine`

`cd engine`

`npm install`

# Dev server
`npm run dev` then visit `https://localhost:3000`

# Electron dev server
`npm run electron:dev`

# Issues/bugs
- Cannot save projects or make levels yet
- Switching from webgl to webgpu will clear your scene (persistent state will be coming soon)
- Node editor doesn't do anything yet
- Some issues with file syncing not updating.
- Scene object tree has minor issues especially with dragging objects to folders
- webgpu should work, if it doesn't then run the .bat or ps1 tool to generate fake ssl certificate
- issues with packing and building to electron. dev server works fine
- some issues with camera movement
- Software update doesn't work yet
- Light's can't be positioned yet
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

