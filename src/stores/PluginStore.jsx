import { createSignal, createEffect } from 'solid-js';
import pluginsConfig from '@/api/plugin/plugins.json';

// Create reactive signals for plugin management
const [pluginConfigs, setPluginConfigs] = createSignal(new Map());
const [pluginInstances, setPluginInstances] = createSignal(new Map());
const [pluginStates, setPluginStates] = createSignal(new Map());
const [pluginErrors, setPluginErrors] = createSignal(new Map());

// Plugin states enum
export const PLUGIN_STATES = {
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

// Initialize plugin configurations from JSON and localStorage
const initializePluginConfigs = () => {
  const configs = new Map();
  
  // Start with JSON file configurations
  pluginsConfig.plugins.forEach(plugin => {
    configs.set(plugin.id, {
      ...plugin,
      enabled: plugin.enabled !== false // Default to true if not specified
    });
  });
  
  // Check localStorage for saved configurations
  try {
    const saved = localStorage.getItem('pluginConfigs');
    if (saved) {
      const savedConfigs = JSON.parse(saved);
      if (savedConfigs.plugins && Array.isArray(savedConfigs.plugins)) {
        // Merge saved configurations with JSON configurations
        savedConfigs.plugins.forEach(savedPlugin => {
          if (configs.has(savedPlugin.id)) {
            // Update enabled state from localStorage
            const existing = configs.get(savedPlugin.id);
            configs.set(savedPlugin.id, {
              ...existing,
              enabled: savedPlugin.enabled
            });
          }
        });
        console.log('[PluginStore] Merged configurations from localStorage');
      }
    }
  } catch (error) {
    console.warn('[PluginStore] Failed to load from localStorage:', error);
  }
  
  setPluginConfigs(configs);
};

// Initialize on import
initializePluginConfigs();

// Plugin Store API
export const pluginStore = {
  // Getters
  getPluginConfigs: () => pluginConfigs(),
  getPluginConfig: (id) => pluginConfigs().get(id),
  getPluginInstances: () => pluginInstances(),
  getPluginInstance: (id) => pluginInstances().get(id),
  getPluginStates: () => pluginStates(),
  getPluginState: (id) => pluginStates().get(id) || PLUGIN_STATES.DISCOVERED,
  getPluginErrors: () => pluginErrors(),
  getPluginError: (id) => pluginErrors().get(id),
  
  // Get all plugin information combined
  getAllPlugins: () => {
    const configs = pluginConfigs();
    const instances = pluginInstances();
    const states = pluginStates();
    const errors = pluginErrors();
    
    return Array.from(configs.values()).map(config => ({
      id: config.id,
      name: config.name || config.id.split('-').map(word => 
        word.charAt(0).toUpperCase() + word.slice(1)
      ).join(' ') + ' Plugin',
      description: config.description || `Plugin: ${config.id}`,
      version: config.version || '1.0.0',
      author: config.author || 'Renzora Engine Team',
      path: config.path,
      main: config.main,
      priority: config.priority || 1,
      enabled: config.enabled,
      state: states.get(config.id) || PLUGIN_STATES.DISCOVERED,
      instance: instances.get(config.id) || null,
      error: errors.get(config.id) || null,
      loadedAt: instances.get(config.id)?.loadedAt || null
    }));
  },
  
  // Get enabled plugins only
  getEnabledPlugins: () => {
    return pluginStore.getAllPlugins().filter(plugin => plugin.enabled);
  },
  
  // Get disabled plugins only
  getDisabledPlugins: () => {
    return pluginStore.getAllPlugins().filter(plugin => !plugin.enabled);
  },
  
  // Get running plugins only
  getRunningPlugins: () => {
    return pluginStore.getAllPlugins().filter(plugin => 
      plugin.state === PLUGIN_STATES.RUNNING
    );
  },
  
  // Statistics
  getStats: () => {
    const all = pluginStore.getAllPlugins();
    const states = {};
    
    Object.values(PLUGIN_STATES).forEach(state => {
      states[state] = all.filter(p => p.state === state).length;
    });
    
    return {
      total: all.length,
      enabled: all.filter(p => p.enabled).length,
      disabled: all.filter(p => !p.enabled).length,
      running: all.filter(p => p.state === PLUGIN_STATES.RUNNING).length,
      errors: all.filter(p => p.state === PLUGIN_STATES.ERROR).length,
      states
    };
  },
  
  // Setters for plugin instances and states
  setPluginInstance: (id, instance, module = null) => {
    setPluginInstances(prev => new Map(prev.set(id, {
      instance,
      module,
      loadedAt: Date.now()
    })));
  },
  
  setPluginState: (id, state) => {
    setPluginStates(prev => new Map(prev.set(id, state)));
  },
  
  setPluginError: (id, error) => {
    setPluginErrors(prev => new Map(prev.set(id, {
      error: error.message || error,
      stack: error.stack,
      timestamp: Date.now()
    })));
  },
  
  clearPluginError: (id) => {
    setPluginErrors(prev => {
      const newMap = new Map(prev);
      newMap.delete(id);
      return newMap;
    });
  },
  
  // Enable/Disable plugins with runtime loading/unloading
  setPluginEnabled: async (id, enabled) => {
    const currentConfig = pluginConfigs().get(id);
    if (!currentConfig) {
      console.error(`Plugin ${id} not found in configurations`);
      return false;
    }

    // Prevent disabling core plugins
    if (!enabled && currentConfig.path && currentConfig.path.includes('/src/plugins/core/')) {
      console.warn(`Cannot disable core plugin: ${id}`);
      return false;
    }

    // Update local state
    setPluginConfigs(prev => {
      const newConfigs = new Map(prev);
      newConfigs.set(id, { ...currentConfig, enabled });
      return newConfigs;
    });
    
    // Handle runtime loading/unloading
    try {
      if (enabled) {
        // Enable plugin - load it dynamically at runtime
        console.log(`[PluginStore] Enabling plugin: ${id}`);
        pluginStore.setPluginState(id, PLUGIN_STATES.LOADING);
        
        // Emit event that plugin is being enabled
        pluginStore.emit('plugin-enabling', { pluginId: id, config: currentConfig });
        
      } else {
        // Disable plugin - unload it from runtime
        console.log(`[PluginStore] Disabling plugin: ${id}`);
        pluginStore.setPluginState(id, PLUGIN_STATES.DISABLED);
        
        // Clean up plugin instance
        const instance = pluginStore.getPluginInstance(id);
        if (instance?.instance) {
          await pluginStore.unloadPlugin(id);
        }
        
        // Emit event that plugin has been disabled
        pluginStore.emit('plugin-disabled', { pluginId: id, config: currentConfig });
      }
      
      // Persist to JSON file via API call
      await pluginStore.saveConfigsToFile();
      
      // Emit general state change event
      pluginStore.emit('plugin-state-changed', { 
        pluginId: id, 
        enabled, 
        config: { ...currentConfig, enabled }
      });
      
      return true;
    } catch (error) {
      console.error(`Failed to ${enabled ? 'enable' : 'disable'} plugin ${id}:`, error);
      
      // Revert state on error
      setPluginConfigs(prev => {
        const newConfigs = new Map(prev);
        newConfigs.set(id, currentConfig);
        return newConfigs;
      });
      
      pluginStore.setPluginError(id, error);
      pluginStore.setPluginState(id, PLUGIN_STATES.ERROR);
      return false;
    }
  },
  
  // Unload a plugin and clean up its UI elements
  unloadPlugin: async (id) => {
    console.log(`[PluginStore] Unloading plugin: ${id}`);
    
    const instance = pluginStore.getPluginInstance(id);
    if (instance?.instance) {
      try {
        // Call plugin's cleanup method if it exists
        if (typeof instance.instance.onDispose === 'function') {
          await instance.instance.onDispose();
        }
        
        // Clean up SolidJS reactive context if it exists
        if (instance.instance._dispose) {
          instance.instance._dispose();
        }
        
        // Remove from instance registry
        setPluginInstances(prev => {
          const newMap = new Map(prev);
          newMap.delete(id);
          return newMap;
        });
        
        console.log(`[PluginStore] Plugin ${id} unloaded successfully`);
      } catch (error) {
        console.error(`[PluginStore] Error unloading plugin ${id}:`, error);
        throw error;
      }
    }
  },
  
  // Add new plugin configuration
  addPluginConfig: (config) => {
    setPluginConfigs(prev => new Map(prev.set(config.id, {
      ...config,
      enabled: config.enabled !== false
    })));
  },
  
  // Remove plugin configuration
  removePluginConfig: (id) => {
    const currentConfig = pluginConfigs().get(id);
    
    // Prevent removing core plugins
    if (currentConfig && currentConfig.path && currentConfig.path.includes('/src/plugins/core/')) {
      console.warn(`Cannot remove core plugin: ${id}`);
      return false;
    }
    
    setPluginConfigs(prev => {
      const newMap = new Map(prev);
      newMap.delete(id);
      return newMap;
    });
    
    // Clean up related data
    setPluginInstances(prev => {
      const newMap = new Map(prev);
      newMap.delete(id);
      return newMap;
    });
    
    setPluginStates(prev => {
      const newMap = new Map(prev);
      newMap.delete(id);
      return newMap;
    });
    
    setPluginErrors(prev => {
      const newMap = new Map(prev);
      newMap.delete(id);
      return newMap;
    });
  },
  
  // Save configurations to localStorage (file writing not supported for non-project files)
  saveConfigsToFile: async () => {
    const configs = Array.from(pluginConfigs().values());
    const jsonData = {
      plugins: configs.map(config => ({
        id: config.id,
        main: config.main,
        path: config.path,
        priority: config.priority,
        enabled: config.enabled
      }))
    };
    
    try {
      // Save to localStorage (primary persistence mechanism)
      localStorage.setItem('pluginConfigs', JSON.stringify(jsonData));
      console.log('[PluginStore] Plugin configurations saved to localStorage');
      
      // Note: File writing to src/api/plugin/plugins.json is not supported by the backend
      // as it only allows writing to project directories (projects/{project_name}/...)
      // Plugin states will be restored from localStorage on next app launch
      
      return true;
    } catch (error) {
      console.error('[PluginStore] Failed to save plugin configurations:', error);
      throw error;
    }
  },
  
  // Reload configurations from JSON file
  reloadConfigsFromFile: async () => {
    try {
      const response = await fetch('/src/api/plugin/plugins.json?' + Date.now());
      const updatedConfig = await response.json();
      
      const configs = new Map();
      updatedConfig.plugins.forEach(plugin => {
        configs.set(plugin.id, {
          ...plugin,
          enabled: plugin.enabled !== false
        });
      });
      
      setPluginConfigs(configs);
      console.log('[PluginStore] Plugin configurations reloaded successfully');
      return true;
    } catch (error) {
      console.error('[PluginStore] Failed to reload plugin configurations:', error);
      return false;
    }
  },
  
  // Event emitter for plugin changes
  emit: (eventType, data) => {
    const event = new CustomEvent(`plugin-store:${eventType}`, { detail: data });
    document.dispatchEvent(event);
  },
  
  on: (eventType, callback) => {
    const handler = (event) => callback(event.detail);
    document.addEventListener(`plugin-store:${eventType}`, handler);
    return () => document.removeEventListener(`plugin-store:${eventType}`, handler);
  }
};

// Auto-emit events when things change
createEffect(() => {
  const configs = pluginConfigs();
  pluginStore.emit('configs-changed', Array.from(configs.values()));
});

createEffect(() => {
  const states = pluginStates();
  pluginStore.emit('states-changed', Object.fromEntries(states));
});

// Export reactive signals for direct access if needed
export {
  pluginConfigs,
  pluginInstances,
  pluginStates,
  pluginErrors,
  setPluginConfigs,
  setPluginInstances,
  setPluginStates,
  setPluginErrors
};

export default pluginStore;