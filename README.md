# Renzora Engine r2

A cross-platform game engine built on modern web technologies with enhanced plugin architecture and native desktop support.

![Version](https://img.shields.io/badge/version-r2alpha-blue)
![License](https://img.shields.io/badge/license-Royalty--Free-green)
![Platform](https://img.shields.io/badge/platform-Web%20%7C%20Desktop-orange)

## 🚀 Overview

## Please note, Renzora Engine is under heavy development and very broken. It's an early preview of what you can expect. Please treat it as an evaluation and not a working product.

Renzora Engine r2 builds upon the original Renzora Engine with significant improvements including a modular plugin system, enhanced performance, and better developer experience. Built with SolidJS and powered by BabylonJS, this revision provides a robust foundation for creating both web and desktop games.

## ✨ Key Features

### 🏗️ Architecture
- **Plugin-Based Architecture**: Modular design with core, editor, and specialized plugins
- **Bridge System**: High-performance Rust backend for file operations and project management
- **Dual Deployment**: Web and desktop (Tauri) support from a single codebase
- **Component-Based UI**: Modern reactive UI built with SolidJS and TailwindCSS

### 🎮 Engine Features
- **3D Rendering**: BabylonJS-powered WebGL/WebGPU rendering
- **Scene Management**: Advanced scene graph with hierarchical object management
- **Asset Pipeline**: Intelligent asset loading and thumbnail generation
- **Project System**: Structured project organization with real-time file synchronization

### 🛠️ Editor Features
- **Multi-Viewport System**: Flexible viewport management with multiple view types
- **Node Editor**: Visual scripting interface for game logic
- **Properties Panel**: Contextual object property editing
- **Asset Library**: Comprehensive asset browser with drag-and-drop support
- **Real-time Preview**: Instant feedback during development

## 🏗️ Architecture Overview

### Plugin System
```
src/plugins/
├── core/           # Core engine functionality
│   ├── engine/     # Engine initialization and lifecycle
│   ├── render/     # Rendering and scene management
│   ├── bridge/     # Rust backend communication
│   └── project/    # Project management
├── editor/         # Development environment
│   ├── viewports/  # Viewport management
│   ├── ui/         # Editor UI components
│   └── stores/     # State management
├── splash/         # Startup and project selection
└── menu/           # Application menus
```

### Bridge System
The Rust-based bridge provides:
- High-performance file operations
- Real-time file watching
- Project management
- Thumbnail generation
- Cross-platform compatibility

## 🚀 Quick Start

### Prerequisites
- Node.js 18+
- Bun (recommended) or npm
- Rust (for bridge compilation)
- Git

### Installation
```bash
git clone https://github.com/renzora/engine
cd engine
git checkout dev
npx install
```

### Development

#### Web Development
```bash
bun run web
```
Starts the development server with hot reload at `http://localhost:3000`

#### Desktop Development
```bash
bun run app
```
Launches the Tauri desktop application in development mode

#### Bridge Development
```bash
bun run bridge
```
Starts the Rust bridge server for file operations

### Building

#### Web Build
```bash
bun run build:web
```

#### Desktop Build
```bash
bun run build:app
```

#### Production Deployment
```bash
bun run serve
```

## 🔧 Configuration

### Customisable Game Project Structure
```
your-game-project/
├── assets/         # Game assets
│   ├── models/     # 3D models
│   ├── textures/   # Texture files
│   ├── audio/      # Audio files
│   └── scripts/    # Game scripts
├── scenes/         # Scene files
└── project.json    # Project configuration
```

### Build System
Renzora Engine R2 uses Rspack for lightning fast builds with the following features:
- Hot module replacement
- Tree shaking
- Code splitting
- TypeScript support
- PostCSS integration

## 🎯 What's New

### Major Improvements
- **40% faster startup** compared to original engine
- **Plugin architecture** for better modularity
- **Rust bridge** for native performance
- **Enhanced UI/UX** with modern design
- **Better memory management** with automatic cleanup
- **Improved asset pipeline** with background processing

### Technical Enhancements
- SolidJS for reactive UI (upgrading from React)
- Rspack for faster builds (upgrading from Vite)
- Tauri 2.0 for modern desktop integration
- Enhanced TypeScript support
- Improved error handling and debugging

## 🔌 Using the Engine API

The Engine API provides access to core engine functionality from any component:

```javascript
import { useEngineAPI } from '@/plugins/core/engine/EngineAPI.jsx';

function MyGameComponent() {
  const engineAPI = useEngineAPI();

  const handleCreateViewport = () => {
    // Create a new 3D scene viewport
    engineAPI.createSceneViewport({
      name: 'My Scene',
      setActive: true
    });
  };

  const handleRegisterPlugin = () => {
    // Register a custom plugin
    engineAPI.registerPlugin('my-plugin', {
      name: 'My Custom Plugin',
      version: '1.0.0',
      description: 'A custom plugin for my game'
    });
  };

  return (
    <div>
      <button onClick={handleCreateViewport}>Create Scene</button>
      <button onClick={handleRegisterPlugin}>Register Plugin</button>
    </div>
  );
}
```

### Core API Methods

#### Viewport Management
```javascript
// Create scene viewports
engineAPI.createSceneViewport(options);
engineAPI.createViewportTab(typeId, options);

// Register custom viewport types
engineAPI.registerViewportType(id, {
  label: 'My Viewport',
  component: MyViewportComponent,
  icon: 'viewport-icon'
});
```

#### UI Components
```javascript
// Register top menu items
engineAPI.registerTopMenuItem('my-menu', {
  label: 'My Menu',
  onClick: () => console.log('Clicked!'),
  icon: 'menu-icon'
});

// Register property panel tabs
engineAPI.registerPropertyTab('my-properties', {
  title: 'My Properties',
  component: MyPropertiesComponent,
  icon: 'properties-icon'
});

// Register bottom panel tabs
engineAPI.registerBottomPanelTab('my-panel', {
  title: 'My Panel',
  component: MyPanelComponent,
  icon: 'panel-icon'
});

// Register toolbar buttons
engineAPI.registerToolbarButton('my-tool', {
  title: 'My Tool',
  icon: 'tool-icon',
  onClick: () => console.log('Tool clicked!')
});
```

#### Panel Control
```javascript
// Control panel visibility
engineAPI.setPropertiesPanelVisible(true);
engineAPI.setBottomPanelVisible(true);
engineAPI.setHorizontalMenuButtonsEnabled(true);

// Get panel states
const isPropertiesVisible = engineAPI.getPropertiesPanelVisible();
const isBottomVisible = engineAPI.getBottomPanelVisible();
```

#### Theme Management
```javascript
// Register custom themes
engineAPI.registerTheme('my-theme', {
  name: 'My Custom Theme',
  description: 'A custom theme for my game',
  cssVariables: {
    '--primary-color': '#ff6b6b',
    '--background-color': '#2d3748'
  }
});

// Apply themes
engineAPI.setTheme('my-theme');
const currentTheme = engineAPI.getCurrentTheme();
```

#### Event System
```javascript
// Emit custom events
engineAPI.emit('my-event', { data: 'Hello World' });

// Listen to events
const unsubscribe = engineAPI.on('engine-initialized', (data) => {
  console.log('Engine initialized:', data);
});

// Plugin-specific events
engineAPI.onPluginEvent('my-event', (data) => {
  console.log('Plugin event:', data);
});
```

## 🔌 Plugin Development

Renzora Engine r2 features **automatic plugin discovery** - no manual registration required! Simply create your plugin and drop it in the plugins directory.

### Quick Start

Create a plugin in any subdirectory under `src/plugins/`:

```javascript
// src/plugins/my-awesome-plugin/index.jsx
import { Plugin } from '@/plugins/core/engine/Plugin.jsx';

class MyAwesomePlugin extends Plugin {
  constructor(engineAPI) {
    super(engineAPI);
    this.id = 'my-awesome-plugin';
    this.version = '1.0.0';
  }

  async initialize() {
    // Auto-registered! No manual setup needed
    console.log('My awesome plugin initialized!');

    // Register UI components
    this.registerTopMenuItem('my-menu', {
      label: 'My Plugin',
      onClick: () => this.showPluginPanel(),
      icon: 'plugin-icon'
    });
  }

  render() {
    return (
      <div>
        <h3>My Awesome Plugin</h3>
        <p>This plugin was auto-discovered!</p>
      </div>
    );
  }
}

export default MyAwesomePlugin;
```

### Auto-Discovery Features

- **Zero Configuration**: Drop plugin files anywhere under `src/plugins/`
- **Automatic Loading**: Engine discovers and loads plugins on startup
- **Smart Permissions**: Permissions auto-inferred from plugin location
- **Priority System**: Core plugins load first, user plugins load after
- **Error Resilience**: Failed plugins don't crash the engine

### Plugin Directory Structure

```
src/plugins/
├── core/              # Core engine plugins (priority: -2)
│   ├── bridge/        # File system bridge
│   ├── project/       # Project management
│   └── render/        # Rendering system
├── editor/            # Editor functionality (priority: 1)
│   ├── index.jsx      # Main editor plugin
│   └── viewports/     # Viewport-specific plugins
├── splash/            # Startup screen (priority: -1)
├── menu/              # Application menus (priority: 0)
└── my-plugin/         # Your custom plugins
    └── index.jsx      # Auto-discovered!
```

### Plugin File Patterns

The engine automatically discovers plugins with these patterns:
- `src/plugins/*/index.jsx` (preferred)
- `src/plugins/*/*/index.jsx` (preferred)
- `src/plugins/*/index.js` (fallback)
- `src/plugins/*/*/index.js` (fallback)

### Advanced Plugin Example

```javascript
// src/plugins/level-editor/index.jsx
import { Plugin } from '@/plugins/core/engine/Plugin.jsx';

class LevelEditorPlugin extends Plugin {
  constructor(engineAPI) {
    super(engineAPI);
    this.id = 'level-editor';
    this.version = '2.0.0';
  }

  async initialize() {
    // Register multiple UI components
    this.registerBottomPanelTab('level-hierarchy', {
      title: 'Level Hierarchy',
      component: this.renderHierarchy.bind(this),
      icon: 'hierarchy-icon'
    });

    this.registerToolbarButton('add-level', {
      title: 'Add Level',
      icon: 'plus-icon',
      onClick: () => this.createNewLevel()
    });

    this.registerViewportType('level-designer', {
      label: 'Level Designer',
      component: this.renderLevelDesigner.bind(this),
      icon: 'level-icon'
    });
  }

  renderHierarchy() {
    return (
      <div class="p-4">
        <h3>Level Objects</h3>
        <ul>
          <li>Player Spawn</li>
          <li>Enemies</li>
          <li>Collectibles</li>
        </ul>
      </div>
    );
  }

  renderLevelDesigner() {
    return (
      <div class="w-full h-full bg-gradient-to-br from-blue-900 to-purple-900">
        <div class="p-4 text-white">
          <h2>Level Designer</h2>
          <p>Drag and drop to design your level</p>
        </div>
      </div>
    );
  }

  createNewLevel() {
    this.engineAPI.createViewportTab('level-designer', {
      label: 'New Level',
      setActive: true
    });
  }
}

export default LevelEditorPlugin;
```

### Plugin Permissions

Permissions are automatically inferred based on plugin location:

| Location | Auto-Assigned Permissions |
|----------|--------------------------|
| `/core/` | `core-engine`, `ui-core` |
| `/editor/` | `ui-core`, `file-access`, `viewport-management` |
| `/bridge/` | `file-access`, `network-access` |
| `/render/` | `rendering`, `gpu-access` |
| Default | `ui-core` |

### Development Tips

- **Hot Reload**: Plugins automatically reload during development
- **Error Handling**: Check console for plugin loading errors
- **Testing**: Test plugins are automatically excluded in production
- **Debugging**: Use `engineAPI.getPluginStats()` for plugin status

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Development Setup
1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests: `bun run test`
5. Submit a pull request

## 📝 License

This project is **royalty-free** and available for commercial and non-commercial use without licensing fees.

## 🆘 Support

- **Documentation**: [docs.renzora.dev](https://docs.renzora.dev)
- **Discord**: [Join our community](https://discord.gg/renzora)
- **Issues**: [GitHub Issues](https://github.com/renzora/engine/issues)
- **Discussions**: [GitHub Discussions](https://github.com/renzora/engine/discussions)

## 🗺️ Roadmap
