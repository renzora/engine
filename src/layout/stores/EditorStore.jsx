import { createStore } from 'solid-js/store';

// Load settings from localStorage
const loadSettings = () => {
  try {
    const saved = localStorage.getItem('editor-settings');
    return saved ? JSON.parse(saved) : {};
  } catch (e) {
    console.warn('Failed to load settings from localStorage:', e);
    return {};
  }
};

// Save settings to localStorage
const saveSettings = (settings) => {
  try {
    localStorage.setItem('editor-settings', JSON.stringify(settings));
  } catch (e) {
    console.warn('Failed to save settings to localStorage:', e);
  }
};

const savedSettings = loadSettings();

const [editorStore, setEditorStore] = createStore({
  isOpen: false,
  
  selection: {
    entity: null,
    transformMode: 'select'
  },
  
  ui: {
    rightPanelWidth: 250,
    bottomPanelHeight: Math.floor(window.innerHeight * 0.4),
    scenePropertiesHeight: Math.floor(window.innerHeight * 0.7),
    assetsLibraryWidth: 200,
    selectedTool: 'scene',
    selectedBottomTab: 'assets',
    toolbarTabOrder: [
      'object-properties', 'scripts', 'camera', 'environment', 'grid', 'settings'
    ],
    toolbarBottomTabOrder: [
      'add', 'settings', 'fullscreen'
    ],
    bottomTabOrder: [
      'assets'
    ]
  },
  
  panels: {
    isResizingPanels: false,
    isScenePanelOpen: true,
    isAssetPanelOpen: true
  },
  
  console: {
    contextMenuHandler: null,
    messages: []
  },
  
  settings: {
    viewport: {
      backgroundColor: savedSettings.viewport?.backgroundColor || 'theme',
      renderingEngine: savedSettings.viewport?.renderingEngine || 'webgl'
    },
    editor: {
      showStats: savedSettings.editor?.showStats !== undefined ? savedSettings.editor.showStats : true,
      panelPosition: savedSettings.editor?.panelPosition || 'right',
      scriptReloadDebounceMs: savedSettings.editor?.scriptReloadDebounceMs || 500,
      renderPaused: savedSettings.editor?.renderPaused || false
    },
    grid: {
      enabled: savedSettings.grid?.enabled !== undefined ? savedSettings.grid.enabled : true,
      unit: savedSettings.grid?.unit || 'centimeters',
      size: savedSettings.grid?.size || 20,
      cellSize: savedSettings.grid?.cellSize || 1,
      sectionSize: savedSettings.grid?.sectionSize || 10,
      infiniteGrid: savedSettings.grid?.infiniteGrid !== undefined ? savedSettings.grid.infiniteGrid : true,
      position: savedSettings.grid?.position || [0, 0, 0],
      cellColor: savedSettings.grid?.cellColor || '#334155',
      sectionColor: savedSettings.grid?.sectionColor || '#475569'
    }
  }
})

export const editorActions = {
  toggleOpen: () => {
    setEditorStore('isOpen', !editorStore.isOpen)
  },
  
  setSelectedTool: (tool) => {
    setEditorStore('ui', 'selectedTool', tool)
  },

  setSelectedBottomTab: (tab) => {
    setEditorStore('ui', 'selectedBottomTab', tab)
  },
  
  selectEntity: (entityId) => {
    console.log('🏪 Editor Store - selectEntity called:', {
      'old value': editorStore.selection.entity,
      'new value': entityId
    })
    setEditorStore('selection', 'entity', entityId)
  },
  
  setScenePanelOpen: (isOpen) => {
    setEditorStore('panels', 'isScenePanelOpen', isOpen);
    
    // Trigger Babylon engine resize to prevent viewport squishing
    const { renderStore } = require('@/render/store.jsx');
    if (renderStore.engine) {
      renderStore.engine.resize();
    }
  },

  setAssetPanelOpen: (isOpen) => {
    setEditorStore('panels', 'isAssetPanelOpen', isOpen);
    
    // Trigger Babylon engine resize to prevent viewport squishing
    const { renderStore } = require('@/render/store.jsx');
    if (renderStore.engine) {
      renderStore.engine.resize();
    }
  },
  
  setResizingPanels: (isResizing) => {
    setEditorStore('panels', 'isResizingPanels', isResizing)
  },

  setRightPanelWidth: (width) => {
    setEditorStore('ui', 'rightPanelWidth', width)
  },

  setBottomPanelHeight: (height) => {
    setEditorStore('ui', 'bottomPanelHeight', height)
  },
  
  setScenePropertiesHeight: (height) => {
    setEditorStore('ui', 'scenePropertiesHeight', height)
  },
  
  setContextMenuHandler: (handler) => {
    setEditorStore('console', 'contextMenuHandler', handler)
  },
  
  addConsoleMessage: (message, type = 'info') => {
    setEditorStore('console', 'messages', editorStore.console.messages.length, {
      message,
      type,
      timestamp: Date.now()
    })
  },
  
  setTransformMode: (mode) => {
    setEditorStore('selection', 'transformMode', mode)
  },
  
  updateViewportSettings: (settings) => {
    setEditorStore('settings', 'viewport', settings);
    saveSettings(editorStore.settings);
  },
  
  toggleStats: () => {
    setEditorStore('settings', 'editor', 'showStats', !editorStore.settings.editor.showStats)
  },
  
  updateEditorSettings: (settings) => {
    setEditorStore('settings', 'editor', settings);
    saveSettings(editorStore.settings);
  },
  
  updateGridSettings: (settings) => {
    setEditorStore('settings', 'grid', settings);
    saveSettings(editorStore.settings);
  },
  
  setToolbarTabOrder: (order) => {
    setEditorStore('ui', 'toolbarTabOrder', order)
  },
  
  setToolbarBottomTabOrder: (order) => {
    setEditorStore('ui', 'toolbarBottomTabOrder', order)
  },
  
  setBottomTabOrder: (order) => {
    setEditorStore('ui', 'bottomTabOrder', order)
  },

  setAssetsLibraryWidth: (width) => {
    setEditorStore('ui', 'assetsLibraryWidth', width)
  },

  updateBabylonObjectFromProperties: (entityId) => {
    // This will be used to sync changes from property panel to Babylon object
    // Note: This creates a circular dependency, so we'll implement this differently
    // by calling it from the Scene.jsx component where both stores are already imported
    console.log('updateBabylonObjectFromProperties called for:', entityId);
  }
}

export { editorStore }

if (typeof window !== 'undefined') {
  window.editorStore = editorStore
  window.editorActions = editorActions
}
