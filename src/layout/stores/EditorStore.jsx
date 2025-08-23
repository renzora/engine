import { createStore } from 'solid-js/store'

const [editorStore, setEditorStore] = createStore({
  isOpen: false,
  
  selection: {
    entity: null,
    transformMode: 'select'
  },
  
  ui: {
    rightPanelWidth: 304,
    bottomPanelHeight: Math.floor(window.innerHeight * 0.3),
    scenePropertiesHeight: Math.floor(window.innerHeight * 0.7),
    assetsLibraryWidth: 250,
    selectedTool: 'scene',
    selectedBottomTab: 'assets',
    toolbarTabOrder: [
      'scene', 'light', 'effects', 'folder', 'star', 'wifi', 'cloud', 'monitor'
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
      backgroundColor: 'theme',
      renderingEngine: 'webgl'
    },
    editor: {
      showStats: true,
      panelPosition: 'right'
    },
    grid: {
      enabled: true,
      unit: 'meters',
      size: 20,
      cellSize: 1,
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
  
  selectEntity: (entityId) => {
    console.log('🏪 Editor Store - selectEntity called:', {
      'old value': editorStore.selection.entity,
      'new value': entityId
    })
    setEditorStore('selection', 'entity', entityId)
  },
  
  setScenePanelOpen: (isOpen) => {
    setEditorStore('panels', 'isScenePanelOpen', isOpen)
  },

  setAssetPanelOpen: (isOpen) => {
    setEditorStore('panels', 'isAssetPanelOpen', isOpen)
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
    setEditorStore('settings', 'viewport', settings)
  },
  
  toggleStats: () => {
    setEditorStore('settings', 'editor', 'showStats', !editorStore.settings.editor.showStats)
  },
  
  updateEditorSettings: (settings) => {
    setEditorStore('settings', 'editor', settings)
  },
  
  updateGridSettings: (settings) => {
    setEditorStore('settings', 'grid', settings)
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
  }
}

export { editorStore }

if (typeof window !== 'undefined') {
  window.editorStore = editorStore
  window.editorActions = editorActions
}
