import { createStore } from 'solid-js/store';

const [editorStore, setEditorStore] = createStore({
  isOpen: false,
  
  selection: {
    entity: null,
    entities: [], // Array of selected entity IDs for multi-selection
    transformMode: 'select'
  },
  
  ui: {
    rightPanelWidth: 300,
    bottomPanelHeight: 300,
    scenePropertiesHeight: Math.floor(window.innerHeight * 0.7),
    assetsLibraryWidth: 200,
    selectedTool: 'scene',
    selectedBottomTab: 'assets',
    currentMode: 'standard',
    toolbarTabOrder: [
      'scripts', 'camera', 'grid', 'settings'
    ],
    toolbarBottomTabOrder: [
      'add', 'settings', 'fullscreen'
    ],
    bottomTabOrder: [
      'assets'
    ]
  },

  // Centralized editor behavior controls
  controls: {
    // Selection system
    selection: {
      enabled: true,
      allowDeselection: true,
      multiSelectEnabled: true,
      highlightEnabled: true
    },
    
    // Camera system
    camera: {
      leftClickPanEnabled: true,
      rightClickPanEnabled: true,
      middleClickOrbitEnabled: true,
      rightClickOrbitEnabled: true,
      zoomEnabled: true,
      keyboardNavigationEnabled: true,
      focusEnabled: true,
      resetEnabled: true
    },
    
    // Transform system
    transform: {
      gizmosEnabled: true,
      positionEnabled: true,
      rotationEnabled: true,
      scaleEnabled: true,
      gridSnappingEnabled: false,
      constraintsEnabled: true
    },
    
    // Interaction system
    interaction: {
      objectPickingEnabled: true,
      groundPickingEnabled: true,
      dragDropEnabled: true,
      contextMenuEnabled: true,
      keyboardShortcutsEnabled: true,
      mouseWheelEnabled: true
    },
    
    // Viewport system
    viewport: {
      renderingEnabled: true,
      animationsEnabled: true,
      physicsEnabled: true,
      postProcessingEnabled: true,
      wireframeOverride: false,
      debugVisualizationEnabled: false
    },
    
    // Mode-specific overrides
    overrides: {
      // When in sculpting mode, certain controls are disabled
      sculpting: {
        selection: { allowDeselection: false },
        camera: { leftClickPanEnabled: false }, // Only disable left-click panning
        interaction: { objectPickingEnabled: false },
        transform: { gizmosEnabled: false }
      },
      
      // When in animation mode, certain features are enhanced
      animation: {
        viewport: { animationsEnabled: true },
        transform: { constraintsEnabled: false }
      },
      
      // When in level editor mode, snapping is enabled
      levelPrototyping: {
        transform: { gridSnappingEnabled: true },
        interaction: { groundPickingEnabled: true }
      }
    }
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
  
  scripts: {
    isPlaying: true
  },

  theme: 'dark',

  settings: {
    viewport: {
      backgroundColor: '#1a202c',
      renderingEngine: 'webgl'
    },
    editor: {
      showStats: true,
      panelPosition: 'right',
      scriptReloadDebounceMs: 500,
      renderPaused: false
    },
    grid: {
      enabled: true,
      unit: 'centimeters',
      size: 20,
      cellSize: 100,
      sectionSize: 10,
      infiniteGrid: true,
      position: [0, 0, 0],
      cellColor: '#334155',
      sectionColor: '#475569'
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
  
  selectEntity: (entityId, entityIds = null) => {
    setEditorStore('selection', 'entity', entityId);
    if (entityIds !== null) {
      setEditorStore('selection', 'entities', entityIds);
    } else {
      setEditorStore('selection', 'entities', entityId ? [entityId] : []);
    }
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
    editorActions.saveToProject();
  },
  
  toggleStats: () => {
    setEditorStore('settings', 'editor', 'showStats', !editorStore.settings.editor.showStats)
  },
  
  updateEditorSettings: (settings) => {
    setEditorStore('settings', 'editor', settings);
    editorActions.saveToProject();
  },
  
  updateGridSettings: (settings) => {
    setEditorStore('settings', 'grid', settings);
    editorActions.saveToProject();
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

  updateBabylonObjectFromProperties: () => {
    // This will be used to sync changes from property panel to Babylon object
    // Note: This creates a circular dependency, so we'll implement this differently
    // by calling it from the Scene.jsx component where both stores are already imported
    // Update Babylon object properties
  },

  toggleScriptExecution: () => {
    const newState = !editorStore.scripts.isPlaying;
    setEditorStore('scripts', 'isPlaying', newState);
    editorActions.saveToProject();
    
    // Trigger script manager state change
    const event = new CustomEvent('engine:script-execution-toggle', {
      detail: { isPlaying: newState }
    });
    document.dispatchEvent(event);
  },

  setScriptExecution: (isPlaying) => {
    setEditorStore('scripts', 'isPlaying', isPlaying);
    editorActions.saveToProject();
    
    // Trigger script manager state change
    const event = new CustomEvent('engine:script-execution-toggle', {
      detail: { isPlaying }
    });
    document.dispatchEvent(event);
  },

  setCurrentMode: (mode) => {
    setEditorStore('ui', 'currentMode', mode);
    editorActions.saveToProject();
    
    // Trigger mode change event
    const event = new CustomEvent('engine:mode-change', {
      detail: { mode, previousMode: editorStore.ui.currentMode }
    });
    document.dispatchEvent(event);
  },

  setTheme: (theme) => {
    setEditorStore('theme', theme);
    editorActions.saveToProject();
    
    // Apply theme to DOM
    document.documentElement.setAttribute('data-theme', theme);
  },

  // Save settings to project file instead of localStorage
  saveToProject: () => {
    // Import SceneManager dynamically to avoid circular dependencies
    import('@/api/scene/SceneManager.js').then(({ sceneManager }) => {
      sceneManager.markAsModified();
    }).catch(err => {
      console.warn('Failed to mark project as modified:', err);
    });
  },

  // Load settings from project data
  loadFromProject: (projectSettings) => {
    if (!projectSettings) return;
    
    // Load all settings from project
    if (projectSettings.settings) {
      setEditorStore('settings', projectSettings.settings);
    }
    
    if (projectSettings.ui) {
      setEditorStore('ui', 'currentMode', projectSettings.ui.currentMode || 'standard');
    }
    
    if (projectSettings.scripts) {
      setEditorStore('scripts', projectSettings.scripts);
    }
    
    if (projectSettings.theme) {
      setEditorStore('theme', projectSettings.theme);
      // Apply theme to DOM
      document.documentElement.setAttribute('data-theme', projectSettings.theme);
    }
  },

  // Control system actions
  setControlEnabled: (category, control, enabled) => {
    setEditorStore('controls', category, control, enabled);
  },

  setControlsEnabled: (category, controls) => {
    Object.entries(controls).forEach(([control, enabled]) => {
      setEditorStore('controls', category, control, enabled);
    });
  },

  // Utility to get effective control state (base + mode overrides)
  getEffectiveControls: () => {
    const currentMode = editorStore.ui.currentMode;
    const baseControls = editorStore.controls;
    const modeOverrides = baseControls.overrides[currentMode] || {};
    
    // Deep merge base controls with mode-specific overrides
    const effectiveControls = JSON.parse(JSON.stringify(baseControls));
    
    Object.entries(modeOverrides).forEach(([category, overrides]) => {
      if (effectiveControls[category]) {
        Object.assign(effectiveControls[category], overrides);
      }
    });
    
    return effectiveControls;
  },

  // Convenient getters for specific control states
  canDeselect: () => {
    const controls = editorActions.getEffectiveControls();
    return controls.selection.enabled && controls.selection.allowDeselection;
  },

  canPanCamera: (button = 0) => {
    const controls = editorActions.getEffectiveControls();
    if (button === 0) return controls.camera.leftClickPanEnabled;
    if (button === 2) return controls.camera.rightClickPanEnabled;
    return controls.camera.leftClickPanEnabled || controls.camera.rightClickPanEnabled;
  },

  canOrbitCamera: (button = 1) => {
    const controls = editorActions.getEffectiveControls();
    if (button === 1) return controls.camera.middleClickOrbitEnabled;
    if (button === 2) return controls.camera.rightClickOrbitEnabled;
    return controls.camera.middleClickOrbitEnabled || controls.camera.rightClickOrbitEnabled;
  },

  canZoomCamera: () => {
    const controls = editorActions.getEffectiveControls();
    return controls.camera.zoomEnabled;
  },

  canPickObjects: () => {
    const controls = editorActions.getEffectiveControls();
    return controls.interaction.objectPickingEnabled;
  },

  canUseKeyboardShortcuts: () => {
    const controls = editorActions.getEffectiveControls();
    return controls.interaction.keyboardShortcutsEnabled;
  },

  canUseMouseWheel: () => {
    const controls = editorActions.getEffectiveControls();
    return controls.interaction.mouseWheelEnabled;
  },

  canShowGizmos: () => {
    const controls = editorActions.getEffectiveControls();
    return controls.transform.gizmosEnabled;
  },

  canTransform: (type = 'position') => {
    const controls = editorActions.getEffectiveControls();
    return controls.transform.gizmosEnabled && controls.transform[`${type}Enabled`];
  }
}

export { editorStore }

if (typeof window !== 'undefined') {
  window.editorStore = editorStore
  window.editorActions = editorActions
}
