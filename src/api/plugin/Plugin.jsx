export function createPlugin(config) {
  const {
    id,
    name,
    version,
    description = 'No description provided',
    author = 'Unknown',
    onInit = async () => {},
    onStart = async () => {},
    onUpdate = () => {},
    onStop = async () => {},
    onDispose = async () => {}
  } = config;

  if (!id) throw new Error('Plugin must have an id');
  if (!name) throw new Error('Plugin must have a name');
  if (!version) throw new Error('Plugin must have a version');

  return function Plugin(api) {
    let initialized = false;
    let started = false;
    const updateCallbacks = [];

    const plugin = {
      getId: () => id,
      getName: () => name,
      getVersion: () => version,
      getDescription: () => description,
      getAuthor: () => author,

      async onInit() {
        if (initialized) return;
        // Initialize plugin
        await onInit(api);
        initialized = true;
      },

      async onStart() {
        if (!initialized) throw new Error('Plugin must be initialized before starting');
        if (started) return;
        // Start plugin
        await onStart(api);
        started = true;
      },

      onUpdate() {
        if (!started) return;
        updateCallbacks.forEach(callback => {
          try {
            callback();
          } catch (error) {
            console.error(`Plugin ${id} update callback error:`, error);
          }
        });
        onUpdate(api);
      },

      async onStop() {
        if (!started) return;
        // Stop plugin
        await onStop(api);
        started = false;
      },

      async onDispose() {
        if (started) await plugin.onStop();
        // Dispose plugin resources
        updateCallbacks.length = 0;
        await onDispose(api);
        initialized = false;
      },

      addUpdateCallback: (callback) => updateCallbacks.push(callback),
      removeUpdateCallback: (callback) => {
        const index = updateCallbacks.indexOf(callback);
        if (index > -1) updateCallbacks.splice(index, 1);
      },

      menu: (menuId, config) => 
        api.registerTopMenuItem(menuId, { ...config, plugin: id }),
      
      tab: (tabId, config) => 
        api.registerPropertyTab(tabId, { ...config, plugin: id }),
      
      panel: (tabId, config) => 
        api.registerBottomPanelTab(tabId, { ...config, plugin: id }),
      
      viewport: (typeId, config) => 
        api.registerViewportType(typeId, { ...config, plugin: id }),
      
      button: (buttonId, config) => 
        api.registerToolbarButton(buttonId, { ...config, plugin: id }),
      
      footer: (buttonId, config) => 
        api.registerFooterButton(buttonId, { ...config, plugin: id }),
      
      open: (typeId, options = {}) => 
        api.createViewportTab(typeId, options),

      emit: (eventType, data) => 
        api.emit(`${id}:${eventType}`, data),
      
      on: (eventType, callback) => 
        api.on(eventType, callback),
      
      onSelf: (eventType, callback) => 
        api.on(`${id}:${eventType}`, callback),

      getStatus: () => ({
        id,
        name,
        version,
        initialized,
        started,
        updateCallbacks: updateCallbacks.length
      })
    };

    return plugin;
  };
}

// Base Plugin class for class-based plugins
export class Plugin {
  constructor(engineAPI) {
    this.engineAPI = engineAPI;
    this.id = null;
    this.name = null;
    this.version = null;
    this.description = null;
  }

  async initialize() {
    // Override in subclasses
  }

  registerViewportType(id, config) {
    return this.engineAPI.registerViewportType(id, { ...config, plugin: this.id });
  }

  registerToolbarButton(id, config) {
    return this.engineAPI.registerToolbarButton(id, { ...config, plugin: this.id });
  }

  registerFooterButton(id, config) {
    return this.engineAPI.registerFooterButton(id, { ...config, plugin: this.id });
  }

  registerTopMenuItem(id, config) {
    return this.engineAPI.registerTopMenuItem(id, { ...config, plugin: this.id });
  }

  registerPropertyTab(id, config) {
    return this.engineAPI.registerPropertyTab(id, { ...config, plugin: this.id });
  }

  registerBottomPanelTab(id, config) {
    return this.engineAPI.registerBottomPanelTab(id, { ...config, plugin: this.id });
  }

  createViewportTab(typeId, options = {}) {
    return this.engineAPI.createViewportTab(typeId, options);
  }

  emit(eventType, data) {
    return this.engineAPI.emit(`${this.id}:${eventType}`, data);
  }

  on(eventType, callback) {
    return this.engineAPI.on(eventType, callback);
  }
}

