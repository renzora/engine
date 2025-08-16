export class Plugin {
  constructor(engineAPI) {
    this.engineAPI = engineAPI;
    this.initialized = false;
    this.started = false;
    this.updateCallbacks = [];
  }

  getId() {
    throw new Error('Plugin must implement getId() method');
  }

  getName() {
    throw new Error('Plugin must implement getName() method');
  }

  getVersion() {
    throw new Error('Plugin must implement getVersion() method');
  }

  getDescription() {
    return 'No description provided';
  }

  getAuthor() {
    return 'Unknown';
  }

  async init() {
    if (this.initialized) return;
    
    console.log(`[Plugin:${this.getId()}] Initializing...`);
    
    await this.onInit();
    
    this.initialized = true;
    console.log(`[Plugin:${this.getId()}] Initialized`);
  }

  async start() {
    if (!this.initialized) {
      throw new Error('Plugin must be initialized before starting');
    }
    
    if (this.started) return;
    
    console.log(`[Plugin:${this.getId()}] Starting...`);
    await this.onStart();
    
    this.started = true;
    console.log(`[Plugin:${this.getId()}] Started`);
  }

  update() {
    if (!this.started) return;
    
    this.updateCallbacks.forEach(callback => {
      try {
        callback();
      } catch (error) {
        console.error(`[Plugin:${this.getId()}] Update callback error:`, error);
      }
    });
    
    this.onUpdate();
  }

  async stop() {
    if (!this.started) return;
    
    console.log(`[Plugin:${this.getId()}] Stopping...`);
    await this.onStop();
    
    this.started = false;
    console.log(`[Plugin:${this.getId()}] Stopped`);
  }

  async dispose() {
    if (this.started) {
      await this.stop();
    }
    
    console.log(`[Plugin:${this.getId()}] Disposing...`);
    this.updateCallbacks = [];
    
    await this.onDispose();
    
    this.initialized = false;
    console.log(`[Plugin:${this.getId()}] Disposed`);
  }

  async onInit() {}
  async onStart() {}
  onUpdate() {}
  async onStop() {}
  async onDispose() {}

  addUpdateCallback(callback) {
    this.updateCallbacks.push(callback);
  }

  removeUpdateCallback(callback) {
    const index = this.updateCallbacks.indexOf(callback);
    if (index > -1) {
      this.updateCallbacks.splice(index, 1);
    }
  }

  registerTopMenuItem(id, config) {
    return this.engineAPI.registerTopMenuItem(id, {
      ...config,
      plugin: this.getId()
    });
  }

  registerPropertyTab(id, config) {
    return this.engineAPI.registerPropertyTab(id, {
      ...config,
      plugin: this.getId()
    });
  }

  registerBottomPanelTab(id, config) {
    return this.engineAPI.registerBottomPanelTab(id, {
      ...config,
      plugin: this.getId()
    });
  }

  registerViewportType(id, config) {
    return this.engineAPI.registerViewportType(id, {
      ...config,
      plugin: this.getId()
    });
  }

  registerToolbarButton(id, config) {
    return this.engineAPI.registerToolbarButton(id, {
      ...config,
      plugin: this.getId()
    });
  }

  registerTheme(id, theme) {
    return this.engineAPI.registerTheme(id, {
      ...theme,
      plugin: this.getId()
    });
  }

  createViewportTab(typeId, options = {}) {
    return this.engineAPI.createViewportTab(typeId, options);
  }

  emit(eventType, data) {
    return this.engineAPI.emit(`${this.getId()}:${eventType}`, data);
  }

  on(eventType, callback) {
    return this.engineAPI.on(eventType, callback);
  }

  onSelf(eventType, callback) {
    return this.engineAPI.on(`${this.getId()}:${eventType}`, callback);
  }

  getStatus() {
    return {
      id: this.getId(),
      name: this.getName(),
      version: this.getVersion(),
      initialized: this.initialized,
      started: this.started,
      updateCallbacks: this.updateCallbacks.length
    };
  }
}