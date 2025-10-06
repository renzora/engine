import { createSignal, createContext, useContext, onMount, onCleanup } from 'solid-js';

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

const PluginAPIContext = createContext();

const [topMenuItems, setTopMenuItems] = createSignal(new Map());
const [propertyTabs, setPropertyTabs] = createSignal(new Map());
const [bottomPanelTabs, setBottomPanelTabs] = createSignal(new Map());
const [viewportTypes, setViewportTypes] = createSignal(new Map());
const [toolbarButtons, setToolbarButtons] = createSignal(new Map());
const [footerButtons, setFooterButtons] = createSignal(new Map());
const [registeredPlugins, setRegisteredPlugins] = createSignal(new Map());
const [propertiesPanelVisible, setPropertiesPanelVisible] = createSignal(true);
const [bottomPanelVisible, setBottomPanelVisible] = createSignal(true);
const [horizontalMenuButtonsEnabled, setHorizontalMenuButtonsEnabled] = createSignal(true);
const [footerVisible, setFooterVisible] = createSignal(true);
const [viewportTabsVisible, setViewportTabsVisible] = createSignal(true);
const [toolbarVisible, setToolbarVisible] = createSignal(true);
const [helperVisible, setHelperVisible] = createSignal(true);
const [layoutComponents, setLayoutComponents] = createSignal(new Map());
const [plugins, setPlugins] = createSignal(new Map());
const [pluginStates, setPluginStates] = createSignal(new Map());
const [pluginErrors, setPluginErrors] = createSignal(new Map());

class PluginLoader {
  constructor(PluginAPI) {
    this.PluginAPI = PluginAPI;
    this.updateInterval = null;
    this.pluginDirectories = [
      '/src/plugins/core',
      '/src/plugins/editor', 
      '/src/plugins/community'
    ];
  }

  async discoverPlugins() {
    // Auto-discovering plugins
    const discovered = new Map();

    const autoDiscoveredPlugins = await this.scanForPlugins();
    
    autoDiscoveredPlugins.forEach(plugin => {
      discovered.set(plugin.id, plugin);
      this.setPluginState(plugin.id, PLUGIN_STATES.DISCOVERED);
    });

    // Auto-discovery completed
    return discovered;
  }

  async scanForPlugins() {
    const plugins = [];
    
    const pluginLocations = [
      { path: '/src/plugins/splash', main: 'index.jsx', priority: -1 },
      { path: '/src/plugins/menu', main: 'index.jsx', priority: 0 },
      { path: '/src/plugins/core/bridge', main: 'index.jsx', priority: -2 },
      { path: '/src/plugins/bridge', main: 'index.jsx', priority: 1 },
      { path: '/src/plugins/scripts', main: 'index.jsx', priority: 1 },
      { path: '/src/plugins/render', main: 'index.jsx', priority: 1 },
      { path: '/src/plugins/material', main: 'index.jsx', priority: 1 },
      { path: '/src/plugins/light', main: 'index.jsx', priority: 1 },
      { path: '/src/plugins/web-browser', main: 'index.jsx', priority: 1 },
      { path: '/src/plugins/materials-viewport', main: 'index.jsx', priority: 1 },
      { path: '/src/plugins/code-editor-viewport', main: 'index.jsx', priority: 1 },
      { path: '/src/plugins/camera', main: 'index.jsx', priority: 3 },
      { path: '/src/plugins/grid', main: 'index.jsx', priority: 4 },
      { path: '/src/plugins/terrain', main: 'index.jsx', priority: 2 },
      { path: '/src/plugins/environment', main: 'index.jsx', priority: 2 }
    ];

    for (const location of pluginLocations) {
      try {
        const pathParts = location.path.split('/').filter(p => p && p !== 'src' && p !== 'plugins' && p !== 'ui');
        const pluginId = pathParts.join('-') + '-plugin';
        
        if (pluginId.includes('test') && process.env.NODE_ENV === 'production') {
          continue;
        }

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
        // Auto-discovered plugin: pluginId at location.path
      } catch (error) {
        console.warn(`[PluginLoader] Failed to process plugin at ${location.path}:`, error);
      }
    }

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
      // Loading plugin: id

      let pluginModule;
      
      try {
        switch (id) {
          case 'splash-plugin':
            pluginModule = await import(`@/plugins/splash/index.jsx`);
            break;
          case 'menu-plugin':
            pluginModule = await import(`@/plugins/menu/index.jsx`);
            break;
          case 'core-bridge-plugin':
            pluginModule = await import(`@/plugins/core/bridge/BridgePluginClass.jsx`);
            break;
          case 'bridge-plugin':
            pluginModule = await import(`@/plugins/bridge/index.jsx`);
            break;
          case 'camera-plugin':
            pluginModule = await import(`@/plugins/camera/index.jsx`);
            break;
          case 'grid-plugin':
            pluginModule = await import(`@/plugins/grid/index.jsx`);
            break;
          case 'scripts-plugin':
            pluginModule = await import(`@/plugins/scripts/index.jsx`);
            break;
          case 'render-plugin':
            pluginModule = await import(`@/plugins/render/index.jsx`);
            break;
          case 'material-plugin':
            pluginModule = await import(`@/plugins/material/index.jsx`);
            break;
          case 'light-plugin':
            pluginModule = await import(`@/plugins/light/index.jsx`);
            break;
          case 'web-browser-plugin':
            pluginModule = await import(`@/plugins/web-browser/index.jsx`);
            break;
          case 'materials-viewport-plugin':
            pluginModule = await import(`@/plugins/materials-viewport/index.jsx`);
            break;
          case 'code-editor-viewport-plugin':
            pluginModule = await import(`@/plugins/code-editor-viewport/index.jsx`);
            break;
          case 'terrain-plugin':
            pluginModule = await import(`@/plugins/terrain/index.jsx`);
            break;
          case 'environment-plugin':
            pluginModule = await import(`@/plugins/environment/index.jsx`);
            break;
          default:
            try {
              const mainPath = `${path}/${manifest.main}`;
              pluginModule = await import(/* webpackIgnore: true */ mainPath);
            } catch {
              try {
                pluginModule = await import(/* webpackIgnore: true */ `${path}/index.jsx`);
              } catch {
                pluginModule = await import(/* webpackIgnore: true */ `${path}/index.js`);
              }
            }
        }
      } catch (importError) {
        console.warn(`[PluginLoader] Failed to import plugin ${id}:`, importError);
        throw new Error(`Could not load plugin from ${path}`);
      }

      if (!pluginModule.default && !pluginModule.Plugin) {
        throw new Error(`Plugin ${id} must export a default plugin function`);
      }

      const PluginFactory = pluginModule.default || pluginModule.Plugin;
      let pluginInstance;
      
      // Handle class-based plugins
      if (PluginFactory.prototype && PluginFactory.prototype.constructor) {
        pluginInstance = new PluginFactory(this.PluginAPI);
        // Add required methods for class-based plugins
        if (!pluginInstance.getId) {
          pluginInstance.getId = () => pluginInstance.id;
        }
        if (!pluginInstance.getName) {
          pluginInstance.getName = () => pluginInstance.name;
        }
        if (!pluginInstance.getVersion) {
          pluginInstance.getVersion = () => pluginInstance.version;
        }
        if (!pluginInstance.onInit) {
          pluginInstance.onInit = () => pluginInstance.initialize();
        }
      } else {
        // Handle function-based plugins
        pluginInstance = PluginFactory(this.PluginAPI);
      }
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
      // Plugin loaded successfully
      
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
      // Initializing plugin

      if (typeof plugin.instance.onInit === 'function') {
        await plugin.instance.onInit();
      }

      this.PluginAPI.registerPlugin(pluginId, {
        name: plugin.manifest.name,
        version: plugin.manifest.version,
        description: plugin.manifest.description,
        author: plugin.manifest.author,
        instance: plugin.instance
      });

      this.setPluginState(pluginId, PLUGIN_STATES.INITIALIZED);
      // Plugin initialized successfully
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
      // Starting plugin

      if (typeof plugin.instance.onStart === 'function') {
        await plugin.instance.onStart();
      }

      this.setPluginState(pluginId, PLUGIN_STATES.RUNNING);
      // Plugin started successfully
    } catch (error) {
      console.error(`[PluginLoader] Failed to start plugin ${pluginId}:`, error);
      this.setPluginError(pluginId, error);
      this.setPluginState(pluginId, PLUGIN_STATES.ERROR);
      throw error;
    }
  }

  async loadAllPlugins() {
    // Loading all plugins
    
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

    // Plugin loading completed
  }

  startUpdateLoop() {
    if (this.updateInterval) return;

    // Starting plugin update loop
    this.updateInterval = setInterval(() => {
      this.updatePlugins();
    }, 1000 / 60);
  }

  stopUpdateLoop() {
    if (this.updateInterval) {
      clearInterval(this.updateInterval);
      this.updateInterval = null;
      // Plugin update loop stopped
    }
  }

  updatePlugins() {
    const runningPlugins = this.getRunningPlugins();
    
    runningPlugins.forEach(plugin => {
      try {
        if (typeof plugin.instance.onUpdate === 'function') {
          plugin.instance.onUpdate();
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
    
    this.PluginAPI.emit('plugin-state-changed', {
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

export class PluginAPI {
  constructor() {
    this.id = 'plugin-api';
    this.version = '1.0.0';
    this.pluginLoader = new PluginLoader(this);
    this.initialized = false;
  }

  async initialize() {
    if (this.initialized) return;
    
    // Initializing Plugin API
    
    try {
      await this.pluginLoader.loadAllPlugins();
      this.pluginLoader.startUpdateLoop();
      this.initialized = true;
      console.log('[PluginAPI] Plugin API initialized successfully');
      
      this.emit('api-initialized', {
        pluginStats: this.pluginLoader.getStats()
      });
      
    } catch (error) {
      console.error('[PluginAPI] Failed to initialize:', error);
      throw error;
    }
  }

  async dispose() {
    if (!this.initialized) return;
    
    // Disposing Plugin API
    this.pluginLoader.stopUpdateLoop();
    
    const plugins = this.pluginLoader.getAllPlugins();
    for (const plugin of plugins) {
      if (plugin.instance && typeof plugin.instance.onDispose === 'function') {
        try {
          await plugin.instance.onDispose();
        } catch (error) {
          console.error(`Failed to dispose plugin ${plugin.id}:`, error);
        }
      }
    }
    
    this.initialized = false;
    // Plugin API disposed
  }

  registerTopMenuItem(id, config) {
    const menuItem = {
      id,
      label: config.label,
      onClick: config.onClick,
      icon: config.icon,
      submenu: config.submenu,
      order: config.order || 100,
      plugin: config.plugin || 'unknown'
    };

    setTopMenuItems(prev => new Map(prev.set(id, menuItem)));
    // Top menu item registered
    return true;
  }

  registerPropertyTab(id, config) {
    const tab = {
      id,
      title: config.title,
      component: config.component,
      icon: config.icon,
      order: config.order || 100,
      condition: config.condition || (() => true),
      plugin: config.plugin || 'unknown'
    };

    setPropertyTabs(prev => new Map(prev.set(id, tab)));
    // Property tab registered
    return true;
  }

  registerBottomPanelTab(id, config) {
    const tab = {
      id,
      title: config.title,
      component: config.component,
      icon: config.icon,
      order: config.order || 100,
      defaultHeight: config.defaultHeight || 300,
      plugin: config.plugin || 'unknown'
    };

    setBottomPanelTabs(prev => new Map(prev.set(id, tab)));
    // Bottom panel tab registered
    return true;
  }

  registerViewportType(id, config) {
    const viewportType = {
      id,
      label: config.label,
      component: config.component,
      icon: config.icon,
      description: config.description || `${config.label} viewport`,
      plugin: config.plugin || 'unknown'
    };

    setViewportTypes(prev => new Map(prev.set(id, viewportType)));
    // Viewport type registered
    return true;
  }

  registerToolbarButton(id, config) {
    const button = {
      id,
      title: config.title,
      icon: config.icon,
      onClick: config.onClick,
      order: config.order || 100,
      section: config.section || 'main',
      plugin: config.plugin || 'unknown',
      hasDropdown: config.hasDropdown || false,
      dropdownComponent: config.dropdownComponent || null,
      dropdownWidth: config.dropdownWidth || 192,
      isCustomComponent: config.isCustomComponent || false,
      customComponent: config.customComponent || null
    };

    setToolbarButtons(prev => new Map(prev.set(id, button)));
    // Toolbar button registered
    return true;
  }

  registerHelperButton(id, config) {
    const button = {
      id,
      title: config.title,
      icon: config.icon,
      onClick: config.onClick,
      order: config.order || 100,
      section: 'helper',
      plugin: config.plugin || 'unknown',
      hasDropdown: config.hasDropdown || false,
      dropdownComponent: config.dropdownComponent || null,
      dropdownWidth: config.dropdownWidth || 192,
      isCustomComponent: config.isCustomComponent || false,
      customComponent: config.customComponent || null
    };

    setToolbarButtons(prev => new Map(prev.set(id, button)));
    // Helper button registered
    return true;
  }

  registerFooterButton(id, config) {
    const button = {
      id,
      component: config.component,
      order: config.order || 100,
      priority: config.priority || 100,
      section: config.section || 'status',
      plugin: config.plugin || 'unknown'
    };

    setFooterButtons(prev => new Map(prev.set(id, button)));
    // Footer button registered
    return true;
  }


  registerLayoutComponent(region, component) {
    setLayoutComponents(prev => new Map(prev.set(region, component)));
    // Layout component registered
    return true;
  }

  unregisterLayoutComponent(region) {
    setLayoutComponents(prev => {
      const newMap = new Map(prev);
      newMap.delete(region);
      return newMap;
    });
    // Layout component unregistered
    return true;
  }

  getLayoutComponent(region) {
    return layoutComponents().get(region);
  }

  getLayoutComponents() {
    return layoutComponents();
  }

  registerPlugin(id, plugin) {
    const pluginConfig = {
      id,
      name: plugin.name,
      version: plugin.version,
      description: plugin.description,
      author: plugin.author,
      api: plugin.api || {},
      registeredAt: Date.now()
    };

    setRegisteredPlugins(prev => new Map(prev.set(id, pluginConfig)));
    // Plugin registered
    return true;
  }

  menu(id, config) { return this.registerTopMenuItem(id, config); }
  tab(id, config) { return this.registerPropertyTab(id, config); }
  panel(id, config) { return this.registerBottomPanelTab(id, config); }
  viewport(id, config) { return this.registerViewportType(id, config); }
  button(id, config) { return this.registerToolbarButton(id, config); }
  helper(id, config) { return this.registerHelperButton(id, config); }
  footer(id, config) { return this.registerFooterButton(id, config); }
  open(typeId, options) { return this.createViewportTab(typeId, options); }

  createViewportTab(typeId, options = {}) {
    // Creating viewport tab for typeId
    const viewportType = viewportTypes().get(typeId);
    if (!viewportType) {
      console.error(`[PluginAPI] Viewport type not found: ${typeId}`);
      return false;
    }

    try {
      import('@/layout/stores/ViewportStore.jsx').then(({ viewportActions }) => {
        const newTabId = `${typeId}_${Date.now()}`;
        const newTab = {
          id: newTabId,
          name: options.label || viewportType.label,
          label: options.label || viewportType.label,
          type: typeId,
          icon: viewportType.icon,
          component: viewportType.component,
          ...options
        };

        // Creating viewport tab with new ID
        viewportActions.addViewportTab(newTab);
        
        if (options.setActive !== false) {
          viewportActions.setActiveViewportTab(newTabId);
        }
      }).catch(err => {
        console.error('[PluginAPI] Failed to create viewport tab:', err);
      });
      
      return true;
    } catch (error) {
      console.error('[PluginAPI] Failed to create viewport tab:', error);
      return false;
    }
  }

  createSceneViewport(options = {}) {
    try {
      import('@/layout/stores/ViewportStore.jsx').then(({ viewportActions }) => {
        const newTabId = `viewport-${Date.now()}`;
        const newTab = {
          id: newTabId,
          type: '3d-viewport',
          name: options.name || 'Scene 1',
          isPinned: options.isPinned || false,
          hasUnsavedChanges: options.hasUnsavedChanges || false
        };

        // Creating 3D scene viewport with new ID
        viewportActions.addViewportTab(newTab);
        
        if (options.setActive !== false) {
          viewportActions.setActiveViewportTab(newTabId);
        }
      }).catch(err => {
        console.error('[PluginAPI] Failed to create scene viewport:', err);
      });
      
      return true;
    } catch (error) {
      console.error('[PluginAPI] Failed to create scene viewport:', error);
      return false;
    }
  }

  setPropertiesPanelVisible(visible) {
    setPropertiesPanelVisible(visible);
    // Properties panel visibility changed
  }
  
  showProps(visible = true) { return this.setPropertiesPanelVisible(visible); }
  hideProps() { return this.setPropertiesPanelVisible(false); }

  setBottomPanelVisible(visible) {
    setBottomPanelVisible(visible);
    // Bottom panel visibility changed
  }
  
  showPanel(visible = true) { return this.setBottomPanelVisible(visible); }
  hidePanel() { return this.setBottomPanelVisible(false); }

  setHorizontalMenuButtonsEnabled(enabled) {
    setHorizontalMenuButtonsEnabled(enabled);
    // Horizontal menu buttons toggled
  }
  
  showMenu(enabled = true) { return this.setHorizontalMenuButtonsEnabled(enabled); }
  hideMenu() { return this.setHorizontalMenuButtonsEnabled(false); }

  setFooterVisible(visible) {
    setFooterVisible(visible);
    // Footer visibility changed
  }
  
  showFooter(visible = true) { return this.setFooterVisible(visible); }
  hideFooter() { return this.setFooterVisible(false); }

  setViewportTabsVisible(visible) {
    setViewportTabsVisible(visible);
    // Viewport tabs visibility changed
  }
  
  showTabs(visible = true) { return this.setViewportTabsVisible(visible); }
  hideTabs() { return this.setViewportTabsVisible(false); }

  setToolbarVisible(visible) {
    setToolbarVisible(visible);
    // Toolbar visibility changed
  }
  
  showToolbar(visible = true) { return this.setToolbarVisible(visible); }
  hideToolbar() { return this.setToolbarVisible(false); }

  setHelperVisible(visible) {
    setHelperVisible(visible);
    // Helper visibility changed
  }
  
  showHelper(visible = true) { return this.setHelperVisible(visible); }
  hideHelper() { return this.setHelperVisible(false); }


  getTopMenuItems() {
    return Array.from(topMenuItems().values()).sort((a, b) => a.order - b.order);
  }

  getPropertyTabs() {
    return Array.from(propertyTabs().values()).sort((a, b) => a.order - b.order);
  }

  getBottomPanelTabs() {
    return Array.from(bottomPanelTabs().values()).sort((a, b) => a.order - b.order);
  }

  getViewportTypes() {
    return Array.from(viewportTypes().values());
  }

  getToolbarButtons() {
    return Array.from(toolbarButtons().values());
  }

  getFooterButtons() {
    return Array.from(footerButtons().values()).sort((a, b) => a.order - b.order);
  }


  getPlugins() {
    return Array.from(registeredPlugins().values());
  }

  getPlugin(id) {
    return registeredPlugins().get(id);
  }

  getPropertiesPanelVisible() {
    return propertiesPanelVisible();
  }

  getBottomPanelVisible() {
    return bottomPanelVisible();
  }

  getHorizontalMenuButtonsEnabled() {
    return horizontalMenuButtonsEnabled();
  }


  getPluginLoader() {
    return this.pluginLoader;
  }

  getPluginStats() {
    return this.pluginLoader.getStats();
  }

  emit(eventType, data) {
    const event = new CustomEvent(`plugin:${eventType}`, { detail: data });
    document.dispatchEvent(event);
    // Event emitted: eventType
  }

  on(eventType, callback) {
    const handler = (event) => callback(event.detail);
    document.addEventListener(`plugin:${eventType}`, handler);
    return () => document.removeEventListener(`plugin:${eventType}`, handler);
  }

  getInfo() {
    return {
      id: this.id,
      version: this.version,
      registeredTopMenuItems: topMenuItems().size,
      registeredPropertyTabs: propertyTabs().size,
      registeredBottomPanelTabs: bottomPanelTabs().size,
      registeredPlugins: registeredPlugins().size
    };
  }
}

export const pluginAPI = new PluginAPI();

export function PluginAPIProvider(props) {
  return (
    <PluginAPIContext.Provider value={pluginAPI}>
      {props.children}
    </PluginAPIContext.Provider>
  );
}

export function usePluginAPI() {
  const api = useContext(PluginAPIContext);
  if (!api) {
    throw new Error('usePluginAPI must be used within a PluginAPIProvider');
  }
  return api;
}

export function Engine(props) {
  onMount(async () => {
    // Starting Renzora Engine
    try {
      await pluginAPI.initialize();
      // Renzora Engine started successfully
    } catch (error) {
      console.error('[Engine] Failed to start Renzora Engine:', error);
    }
  });

  onCleanup(async () => {
    // Shutting down Renzora Engine
    try {
      await pluginAPI.dispose();
      // Renzora Engine shut down successfully
    } catch (error) {
      console.error('[Engine] Error during shutdown:', error);
    }
  });

  return (
    <PluginAPIProvider>
      {props.children}
    </PluginAPIProvider>
  );
}

export { createPlugin } from './Plugin.jsx';

export {
  topMenuItems,
  propertyTabs,
  bottomPanelTabs,
  viewportTypes,
  toolbarButtons,
  footerButtons,
  registeredPlugins,
  propertiesPanelVisible,
  bottomPanelVisible,
  horizontalMenuButtonsEnabled,
  footerVisible,
  viewportTabsVisible,
  toolbarVisible,
  helperVisible,
  layoutComponents,
  plugins,
  pluginStates,
  pluginErrors,
  PLUGIN_STATES
};

