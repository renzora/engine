# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## important information
- NEVER use server commands
- NEVER use git commands without permission
- All javascript in src directory should use .jsx extension

## Development Commands

### Primary Development Modes
- `bun run web` - Web development with hot reload (localhost:3000)
- `bun run app` - Desktop Tauri development mode
- `bun run bridge` - Start Rust bridge server (port 3001)

### Building
- `bun run build:web` - Build web application
- `bun run build:app` - Build desktop application  
- `bun run build:bridge` - Build Rust bridge server
- `bun run serve` - Production server with built bridge

### Maintenance
- `bun run clean` - Clean all build artifacts and stop processes
- `bun run kill` - Stop processes and clear ports 3000/3001
- `bun run lint` - Run oxlint for code quality


## Architecture Overview

### Core Components
- **Frontend**: SolidJS + TailwindCSS with Rspack bundling
- **3D Engine**: BabylonJS with custom rendering pipeline
- **Bridge**: Rust backend for file operations and project management
- **Desktop**: Tauri 2.0 for native desktop application

### Plugin System Architecture
The engine uses automatic plugin discovery from `src/plugins/`:
- **core/**: Core engine functionality (bridge, project management, rendering)
- **editor/**: Development environment (viewports, UI, stores)  
- **splash/**: Startup screen and project selection
- **menu/**: Application menus

Plugins are automatically discovered using patterns:
- `src/plugins/*/index.jsx` (preferred)
- `src/plugins/*/*/index.jsx` (preferred)

### Key Directories
- `src/api/`: Core APIs (bridge, plugin, script systems)
- `src/render/`: BabylonJS rendering and 3D scene management
- `src/layout/`: UI layout components and stores
- `src/pages/`: Main application pages (editor, node editor)
- `src/ui/`: Reusable UI components
- `bridge/`: Rust backend server
- `projects/`: Game project files and assets

### Script System (RenScript)
Custom scripting language with 580+ methods across 16 modules:
- Script files have `.ren` extension
- API documentation in `src/api/script/README.md`
- Compiled via `src/api/script/renscript/RenScriptCompiler.js`

### State Management
- **Render Store**: `src/render/store.jsx` - 3D scene state, camera, selection
- **Editor Store**: `src/layout/stores/EditorStore.jsx` - UI state, settings
- **Viewport Store**: `src/layout/stores/ViewportStore.jsx` - Viewport management
- **Asset Store**: `src/layout/stores/AssetStore.jsx` - Asset management

### Bridge Communication
Bridge server runs on port 3001 providing:
- File operations and project management
- Real-time file watching and synchronization
- Asset thumbnail generation
- Cross-platform file system access

## Plugin Development

### Creating Plugins
Plugins must export a class extending the base Plugin class:

```javascript
import { Plugin } from '@/api/plugin/Plugin.jsx';

class MyPlugin extends Plugin {
  constructor(engineAPI) {
    super(engineAPI);
    this.id = 'my-plugin';
    this.version = '1.0.0';
  }

  async initialize() {
    // Plugin initialization code
  }
}

export default MyPlugin;
```

### Plugin API Methods
- `this.registerTopMenuItem(id, config)` - Add menu items
- `this.registerViewportType(id, config)` - Add viewport types  
- `this.registerBottomPanelTab(id, config)` - Add bottom panel tabs
- `this.registerPropertyTab(id, config)` - Add property panel tabs
- `this.registerToolbarButton(id, config)` - Add toolbar buttons

## Important Notes

### Technology Stack
- Uses Bun as package manager and task runner
- Rspack for fast bundling (not Webpack)
- SolidJS for reactive UI (not React)
- DaisyUI component library with TailwindCSS
- Babylon.js for 3D rendering and physics

### File Operations
Always use the Bridge service for file operations:
```javascript
import { bridgeService } from '@/plugins/core/bridge';
await bridgeService.readFile('path/to/file');
```

### Project Structure
Projects are stored in `projects/` directory with structure:
- `assets/` - Game assets (models, textures, scripts, etc.)
- `scenes/` - Scene files
- `project.json` - Project configuration

### Development Workflow
1. Start bridge server: `bun run bridge`
2. Start web dev: `bun run web` OR desktop: `bun run app`
3. Bridge runs on port 3001, web dev on port 3000
4. Use `bun run kill` to stop all processes