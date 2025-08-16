import { createSignal, createContext, useContext, onMount, onCleanup } from 'solid-js';
import { PluginLoader } from './PluginLoader.jsx';

const EngineAPIContext = createContext();

const [topMenuItems, setTopMenuItems] = createSignal(new Map());
const [propertyTabs, setPropertyTabs] = createSignal(new Map());
const [bottomPanelTabs, setBottomPanelTabs] = createSignal(new Map());
const [viewportTypes, setViewportTypes] = createSignal(new Map());
const [toolbarButtons, setToolbarButtons] = createSignal(new Map());
const [currentTheme, setCurrentTheme] = createSignal('dark');
const [availableThemes, setAvailableThemes] = createSignal(new Map());
const [registeredPlugins, setRegisteredPlugins] = createSignal(new Map());
const [propertiesPanelVisible, setPropertiesPanelVisible] = createSignal(true);
const [bottomPanelVisible, setBottomPanelVisible] = createSignal(true);
const [horizontalMenuButtonsEnabled, setHorizontalMenuButtonsEnabled] = createSignal(true);

export class EngineAPI {
  constructor() {
    this.id = 'engine-api';
    this.version = '1.0.0';
    this.pluginLoader = new PluginLoader(this);
    this.initialized = false;
  }

  async initialize() {
    if (this.initialized) return;
    
    console.log('[EngineAPI] Initializing Engine API...');
    
    try {
      await this.pluginLoader.loadAllPlugins();

      this.pluginLoader.startUpdateLoop();
      this.initialized = true;
      console.log('[EngineAPI] Engine API initialized successfully');
      
      this.emit('engine-initialized', {
        pluginStats: this.pluginLoader.getStats()
      });
      
    } catch (error) {
      console.error('[EngineAPI] Failed to initialize:', error);
      throw error;
    }
  }

  async dispose() {
    if (!this.initialized) return;
    
    console.log('[EngineAPI] Disposing Engine API...');
    this.pluginLoader.stopUpdateLoop();
    
    const plugins = this.pluginLoader.getAllPlugins();
    for (const plugin of plugins) {
      if (plugin.instance && typeof plugin.instance.dispose === 'function') {
        try {
          await plugin.instance.dispose();
        } catch (error) {
          console.error(`Failed to dispose plugin ${plugin.id}:`, error);
        }
      }
    }
    
    this.initialized = false;
    console.log('[EngineAPI] Engine API disposed');
  }

  registerTopMenuItem(id, config) {
    const menuItem = {
      id,
      label: config.label,
      onClick: config.onClick,
      icon: config.icon,
      order: config.order || 100,
      plugin: config.plugin || 'unknown'
    };

    setTopMenuItems(prev => new Map(prev.set(id, menuItem)));
    
    console.log(`[EngineAPI] Top menu item registered: ${id}`);
    return true;
  }

  getTopMenuItems() {
    return Array.from(topMenuItems().values())
      .sort((a, b) => a.order - b.order);
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
    console.log(`[EngineAPI] Property tab registered: ${id}`);
    return true;
  }

  setActivePropertyTab(id) {
    setActivePropertyTab(id);
    console.log(`[EngineAPI] Property tab activated: ${id}`);
  }

  getPropertyTabs() {
    return Array.from(propertyTabs().values())
      .sort((a, b) => a.order - b.order);
  }

  getActivePropertyTab() {
    return activePropertyTab();
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
    console.log(`[EngineAPI] Bottom panel tab registered: ${id}`);
    return true;
  }

  setActiveBottomPanelTab(id) {
    setActiveBottomPanelTab(id);
    console.log(`[EngineAPI] Bottom panel tab activated: ${id}`);
  }

  getBottomPanelTabs() {
    return Array.from(bottomPanelTabs().values())
      .sort((a, b) => a.order - b.order);
  }

  getActiveBottomPanelTab() {
    return activeBottomPanelTab();
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
    console.log(`[EngineAPI] Viewport type registered: ${id}`);
    return true;
  }

  getViewportTypes() {
    return Array.from(viewportTypes().values());
  }

  createViewportTab(typeId, options = {}) {
    console.log(`[EngineAPI] createViewportTab called for: ${typeId}`);
    const viewportType = viewportTypes().get(typeId);
    if (!viewportType) {
      console.error(`[EngineAPI] Viewport type not found: ${typeId}`);
      console.log(`[EngineAPI] Available viewport types:`, Array.from(viewportTypes().keys()));
      return false;
    }
    console.log(`[EngineAPI] Found viewport type:`, viewportType);

    try {
      import('@/plugins/editor/stores/ViewportStore.jsx').then(({ viewportActions }) => {
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

        console.log(`[EngineAPI] Creating viewport tab: ${newTabId} (${typeId})`);
        viewportActions.addViewportTab(newTab);
        
        if (options.setActive !== false) {
          viewportActions.setActiveViewportTab(newTabId);
        }
      }).catch(err => {
        console.error('[EngineAPI] Failed to create viewport tab:', err);
      });
      
      return true;
    } catch (error) {
      console.error('[EngineAPI] Failed to create viewport tab:', error);
      return false;
    }
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
    console.log(`[EngineAPI] Toolbar button registered: ${id}`);
    return true;
  }

  getToolbarButtons() {
    return Array.from(toolbarButtons().values());
  }

  registerTheme(id, theme) {
    const themeConfig = {
      id,
      name: theme.name,
      description: theme.description,
      colors: theme.colors || {},
      cssVariables: theme.cssVariables || {},
      plugin: theme.plugin || 'unknown'
    };

    setAvailableThemes(prev => new Map(prev.set(id, themeConfig)));
    console.log(`[EngineAPI] Theme registered: ${id}`);
    return true;
  }

  setTheme(id) {
    const theme = availableThemes().get(id);
    if (!theme) {
      console.error(`[EngineAPI] Theme not found: ${id}`);
      return false;
    }

    const root = document.documentElement;
    Object.entries(theme.cssVariables).forEach(([key, value]) => {
      root.style.setProperty(key, value);
    });

    setCurrentTheme(id);
    console.log(`[EngineAPI] Theme applied: ${id}`);
    return true;
  }

  getCurrentTheme() {
    return currentTheme();
  }

  getThemes() {
    return Array.from(availableThemes().values());
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
    console.log(`[EngineAPI] Plugin registered: ${id} v${plugin.version}`);
    return true;
  }

  getPlugin(id) {
    return registeredPlugins().get(id);
  }

  getPlugins() {
    return Array.from(registeredPlugins().values());
  }

  setPropertiesPanelVisible(visible) {
    setPropertiesPanelVisible(visible);
    console.log(`[EngineAPI] Properties panel visibility: ${visible}`);
  }

  getPropertiesPanelVisible() {
    return propertiesPanelVisible();
  }

  setBottomPanelVisible(visible) {
    setBottomPanelVisible(visible);
    console.log(`[EngineAPI] Bottom panel visibility: ${visible}`);
  }

  getBottomPanelVisible() {
    return bottomPanelVisible();
  }

  setHorizontalMenuButtonsEnabled(enabled) {
    setHorizontalMenuButtonsEnabled(enabled);
    console.log(`[EngineAPI] Horizontal menu buttons enabled: ${enabled}`);
  }
  
  getHorizontalMenuButtonsEnabled() {
    return horizontalMenuButtonsEnabled();
  }

  createSceneViewport(options = {}) {
    try {
      import('@/plugins/editor/stores/ViewportStore.jsx').then(({ viewportActions }) => {
        const newTabId = `viewport-${Date.now()}`;
        const newTab = {
          id: newTabId,
          type: '3d-viewport',
          name: options.name || 'Scene 1',
          isPinned: options.isPinned || false,
          hasUnsavedChanges: options.hasUnsavedChanges || false
        };

        console.log(`[EngineAPI] Creating 3D scene viewport: ${newTabId}`);
        viewportActions.addViewportTab(newTab);
        
        if (options.setActive !== false) {
          viewportActions.setActiveViewportTab(newTabId);
        }
      }).catch(err => {
        console.error('[EngineAPI] Failed to create scene viewport:', err);
      });
      
      return true;
    } catch (error) {
      console.error('[EngineAPI] Failed to create scene viewport:', error);
      return false;
    }
  }

  getPluginLoader() {
    return this.pluginLoader;
  }

  getPluginStats() {
    return this.pluginLoader.getStats();
  }

  emit(eventType, data) {
    const event = new CustomEvent(`engine:${eventType}`, { detail: data });
    document.dispatchEvent(event);
    console.log(`[EngineAPI] Event emitted: ${eventType}`, data);
  }

  on(eventType, callback) {
    const handler = (event) => callback(event.detail);
    document.addEventListener(`engine:${eventType}`, handler);
    return () => document.removeEventListener(`engine:${eventType}`, handler);
  }

  getInfo() {
    return {
      id: this.id,
      version: this.version,
      registeredTopMenuItems: topMenuItems().size,
      registeredPropertyTabs: propertyTabs().size,
      registeredBottomPanelTabs: bottomPanelTabs().size,
      registeredThemes: availableThemes().size,
      registeredPlugins: registeredPlugins().size,
      currentTheme: currentTheme()
    };
  }
}

export const engineAPI = new EngineAPI();

export function EngineAPIProvider(props) {
  return (
    <EngineAPIContext.Provider value={engineAPI}>
      {props.children}
    </EngineAPIContext.Provider>
  );
}

export function useEngineAPI() {
  const api = useContext(EngineAPIContext);
  if (!api) {
    throw new Error('useEngineAPI must be used within an EngineAPIProvider');
  }
  return api;
}

export {
  topMenuItems,
  propertyTabs,
  bottomPanelTabs,
  viewportTypes,
  toolbarButtons,
  currentTheme,
  availableThemes,
  registeredPlugins,
  propertiesPanelVisible,
  bottomPanelVisible,
  horizontalMenuButtonsEnabled
};