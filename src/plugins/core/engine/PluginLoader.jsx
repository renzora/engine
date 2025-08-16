import { createSignal, createEffect } from 'solid-js';

const PLUGIN_STATES = {
  DISCOVERED: 'discovered',
  LOADING: 'loading', 
  LOADED: 'loaded',
  INITIALIZING: 'initializing',
  INITIALIZED: 'initialized',
  STARTING: 'starting',
  RUNNING: 'running',
  ERROR: 'error',
  DISABLED: 'disabled'
};

const [plugins, setPlugins] = createSignal(new Map());
const [pluginStates, setPluginStates] = createSignal(new Map());
const [pluginErrors, setPluginErrors] = createSignal(new Map());

export class PluginLoader {
  constructor(engineAPI) {
    this.engineAPI = engineAPI;
    this.updateInterval = null;
    this.pluginDirectories = [
      '/src/plugins/core',
      '/src/plugins/editor', 
      '/src/plugins/community'
    ];
  }

  async discoverPlugins() {
    console.log('[PluginLoader] Auto-discovering plugins...');
    const discovered = new Map();

    // Define plugin discovery patterns (prefer .jsx files)
    const pluginPatterns = [
      { pattern: '/src/plugins/*/index.jsx', type: 'jsx' },
      { pattern: '/src/plugins/*/*/index.jsx', type: 'jsx' },
      { pattern: '/src/plugins/*/index.js', type: 'js' },
      { pattern: '/src/plugins/*/*/index.js', type: 'js' }
    ];

    // Auto-discover plugins by scanning directories
    const autoDiscoveredPlugins = await this.scanForPlugins();
    
    autoDiscoveredPlugins.forEach(plugin => {
      discovered.set(plugin.id, plugin);
      this.setPluginState(plugin.id, PLUGIN_STATES.DISCOVERED);
    });

    console.log(`[PluginLoader] Auto-discovered ${discovered.size} plugins`);
    return discovered;
  }

  async scanForPlugins() {
    const plugins = [];
    
    // Known plugin locations based on file system scan
    const pluginLocations = [
      { path: '/src/plugins/splash', main: 'index.jsx', priority: -1 },
      { path: '/src/plugins/editor', main: 'index.jsx', priority: 1 },
      { path: '/src/plugins/menu', main: 'index.jsx', priority: 0 },
      { path: '/src/plugins/core/bridge', main: 'index.jsx', priority: -2 },
      { path: '/src/plugins/core/project', main: 'index.jsx', priority: -2 },
      { path: '/src/plugins/core/render', main: 'index.jsx', priority: -2 },
      { path: '/src/plugins/test copy', main: 'index.jsx', priority: 10 },
      { path: '/src/plugins/editor/viewports/nodeEditor', main: 'index.jsx', priority: 5 }
    ];

    for (const location of pluginLocations) {
      try {
        // Generate plugin ID from path
        const pathParts = location.path.split('/').filter(p => p && p !== 'src' && p !== 'plugins');
        const pluginId = pathParts.join('-') + '-plugin';
        
        // Skip test plugins in production
        if (pluginId.includes('test') && process.env.NODE_ENV === 'production') {
          continue;
        }

        // Generate plugin name from path
        const pluginName = pathParts
          .map(part => part.split(/[-_]/)
            .map(word => word.charAt(0).toUpperCase() + word.slice(1))
            .join(' '))
          .join(' ') + ' Plugin';

        const plugin = {
          id: pluginId,
          path: location.path,
          manifest: {
            name: pluginName,
            version: '1.0.0',
            description: `Auto-discovered plugin: ${pluginName}`,
            author: 'Renzora Engine Team',
            main: location.main,
            dependencies: [],
            permissions: this.inferPermissions(location.path),
            apiVersion: '1.0.0',
            priority: location.priority
          }
        };

        plugins.push(plugin);
        console.log(`[PluginLoader] Auto-discovered: ${pluginId} at ${location.path}`);
      } catch (error) {
        console.warn(`[PluginLoader] Failed to process plugin at ${location.path}:`, error);
      }
    }

    // Sort by priority (lower numbers load first)
    plugins.sort((a, b) => a.manifest.priority - b.manifest.priority);
    
    return plugins;
  }

  inferPermissions(pluginPath) {
    const permissions = [];
    
    if (pluginPath.includes('/core/')) {
      permissions.push('core-engine', 'ui-core');
    }
    if (pluginPath.includes('/editor')) {
      permissions.push('ui-core', 'file-access', 'viewport-management');
    }
    if (pluginPath.includes('/splash')) {
      permissions.push('ui-core', 'viewport-management');
    }
    if (pluginPath.includes('/menu')) {
      permissions.push('ui-core');
    }
    if (pluginPath.includes('/bridge')) {
      permissions.push('file-access', 'network-access');
    }
    if (pluginPath.includes('/render')) {
      permissions.push('rendering', 'gpu-access');
    }
    
    return permissions.length > 0 ? permissions : ['ui-core'];
  }

  async loadPlugin(pluginInfo) {
    const { id, path, manifest } = pluginInfo;
    
    try {
      this.setPluginState(id, PLUGIN_STATES.LOADING);
      console.log(`[PluginLoader] Loading plugin: ${id}`);

      let pluginModule;
      
      // Use static imports for known plugins to avoid webpack dynamic import issues
      try {
        switch (id) {
          case 'splash-plugin':
            pluginModule = await import(`@/plugins/splash/index.jsx`);
            break;
          case 'editor-plugin':
            const editorModule = await import(`@/plugins/editor/index.jsx`);
            pluginModule = { default: editorModule.EditorPluginClass };
            break;
          case 'menu-plugin':
            pluginModule = await import(`@/plugins/menu/index.jsx`);
            break;
          case 'core-bridge-plugin':
            pluginModule = await import(`@/plugins/core/bridge/index.jsx`);
            break;
          case 'core-project-plugin':
            const projectModule = await import(`@/plugins/core/project/index.jsx`);
            pluginModule = { default: projectModule.ProjectPluginClass };
            break;
          case 'core-render-plugin':
            pluginModule = await import(`@/plugins/core/render/index.jsx`);
            break;
          case 'editor-viewports-nodeEditor-plugin':
            pluginModule = await import(`@/plugins/editor/viewports/nodeEditor/index.jsx`);
            break;
          case 'test copy-plugin':
            pluginModule = await import(`@/plugins/test copy/index.jsx`);
            break;
          default:
            // For unknown plugins, try dynamic imports
            try {
              const mainPath = `${path}/${manifest.main}`;
              pluginModule = await import(mainPath);
            } catch {
              try {
                pluginModule = await import(`${path}/index.jsx`);
              } catch {
                pluginModule = await import(`${path}/index.js`);
              }
            }
        }
      } catch (importError) {
        console.warn(`[PluginLoader] Failed to import plugin ${id}:`, importError);
        throw new Error(`Could not load plugin from ${path}`);
      }

      if (!pluginModule.default && !pluginModule.Plugin) {
        throw new Error(`Plugin ${id} must export a default plugin class or Plugin class`);
      }

      const PluginClass = pluginModule.default || pluginModule.Plugin;
      const pluginInstance = new PluginClass(this.engineAPI);
      const requiredMethods = ['getId', 'getName', 'getVersion'];
      requiredMethods.forEach(method => {
        if (typeof pluginInstance[method] !== 'function') {
          console.warn(`[PluginLoader] Plugin ${id} missing method: ${method}`);
        }
      });

      setPlugins(prev => new Map(prev.set(id, {
        ...pluginInfo,
        instance: pluginInstance,
        module: pluginModule,
        loadedAt: Date.now()
      })));

      this.setPluginState(id, PLUGIN_STATES.LOADED);
      console.log(`[PluginLoader] Plugin loaded: ${id}`);
      
      return pluginInstance;
    } catch (error) {
      console.error(`[PluginLoader] Failed to load plugin ${id}:`, error);
      this.setPluginError(id, error);
      this.setPluginState(id, PLUGIN_STATES.ERROR);
      throw error;
    }
  }

  async initializePlugin(pluginId) {
    const plugin = plugins().get(pluginId);
    if (!plugin || !plugin.instance) {
      throw new Error(`Plugin ${pluginId} not loaded`);
    }

    try {
      this.setPluginState(pluginId, PLUGIN_STATES.INITIALIZING);
      console.log(`[PluginLoader] Initializing plugin: ${pluginId}`);

      if (typeof plugin.instance.init === 'function') {
        await plugin.instance.init();
      }

      this.engineAPI.registerPlugin(pluginId, {
        name: plugin.manifest.name,
        version: plugin.manifest.version,
        description: plugin.manifest.description,
        author: plugin.manifest.author,
        instance: plugin.instance
      });

      this.setPluginState(pluginId, PLUGIN_STATES.INITIALIZED);
      console.log(`[PluginLoader] Plugin initialized: ${pluginId}`);
    } catch (error) {
      console.error(`[PluginLoader] Failed to initialize plugin ${pluginId}:`, error);
      this.setPluginError(pluginId, error);
      this.setPluginState(pluginId, PLUGIN_STATES.ERROR);
      throw error;
    }
  }

  async startPlugin(pluginId) {
    const plugin = plugins().get(pluginId);
    if (!plugin || !plugin.instance) {
      throw new Error(`Plugin ${pluginId} not loaded`);
    }

    try {
      this.setPluginState(pluginId, PLUGIN_STATES.STARTING);
      console.log(`[PluginLoader] Starting plugin: ${pluginId}`);

      if (typeof plugin.instance.start === 'function') {
        await plugin.instance.start();
      }

      this.setPluginState(pluginId, PLUGIN_STATES.RUNNING);
      console.log(`[PluginLoader] Plugin started: ${pluginId}`);
    } catch (error) {
      console.error(`[PluginLoader] Failed to start plugin ${pluginId}:`, error);
      this.setPluginError(pluginId, error);
      this.setPluginState(pluginId, PLUGIN_STATES.ERROR);
      throw error;
    }
  }

  async loadAllPlugins() {
    console.log('[PluginLoader] Loading all plugins...');
    
    const discovered = await this.discoverPlugins();
    const loadPromises = [];

    for (const [id, pluginInfo] of discovered) {
      loadPromises.push(
        this.loadPlugin(pluginInfo).catch(error => {
          console.error(`Failed to load plugin ${id}:`, error);
          return null;
        })
      );
    }

    await Promise.all(loadPromises);

    const initPromises = [];
    for (const [id] of plugins()) {
      if (this.getPluginState(id) === PLUGIN_STATES.LOADED) {
        initPromises.push(
          this.initializePlugin(id).catch(error => {
            console.error(`Failed to initialize plugin ${id}:`, error);
            return null;
          })
        );
      }
    }

    await Promise.all(initPromises);

    const startPromises = [];
    for (const [id] of plugins()) {
      if (this.getPluginState(id) === PLUGIN_STATES.INITIALIZED) {
        startPromises.push(
          this.startPlugin(id).catch(error => {
            console.error(`Failed to start plugin ${id}:`, error);
            return null;
          })
        );
      }
    }

    await Promise.all(startPromises);

    console.log(`[PluginLoader] Plugin loading complete. Running plugins: ${this.getRunningPlugins().length}`);
  }

  startUpdateLoop() {
    if (this.updateInterval) return;

    console.log('[PluginLoader] Starting plugin update loop...');
    this.updateInterval = setInterval(() => {
      this.updatePlugins();
    }, 1000 / 60);
  }

  stopUpdateLoop() {
    if (this.updateInterval) {
      clearInterval(this.updateInterval);
      this.updateInterval = null;
      console.log('[PluginLoader] Plugin update loop stopped');
    }
  }

  updatePlugins() {
    const runningPlugins = this.getRunningPlugins();
    
    runningPlugins.forEach(plugin => {
      try {
        if (typeof plugin.instance.update === 'function') {
          plugin.instance.update();
        }
      } catch (error) {
        console.error(`[PluginLoader] Plugin ${plugin.id} update error:`, error);
        this.setPluginError(plugin.id, error);
      }
    });
  }

  getRunningPlugins() {
    return Array.from(plugins().values()).filter(plugin => 
      this.getPluginState(plugin.id) === PLUGIN_STATES.RUNNING
    );
  }

  getPluginState(pluginId) {
    return pluginStates().get(pluginId) || PLUGIN_STATES.DISCOVERED;
  }

  setPluginState(pluginId, state) {
    setPluginStates(prev => new Map(prev.set(pluginId, state)));
    
    this.engineAPI.emit('plugin-state-changed', {
      pluginId,
      state,
      timestamp: Date.now()
    });
  }

  setPluginError(pluginId, error) {
    setPluginErrors(prev => new Map(prev.set(pluginId, {
      error: error.message,
      stack: error.stack,
      timestamp: Date.now()
    })));
  }

  getPluginInfo(pluginId) {
    return plugins().get(pluginId);
  }

  getAllPlugins() {
    return Array.from(plugins().values());
  }

  getStats() {
    const allPlugins = this.getAllPlugins();
    const states = {};
    
    Object.values(PLUGIN_STATES).forEach(state => {
      states[state] = allPlugins.filter(p => this.getPluginState(p.id) === state).length;
    });

    return {
      total: allPlugins.length,
      states,
      errors: pluginErrors().size
    };
  }
}

export { plugins, pluginStates, pluginErrors, PLUGIN_STATES };