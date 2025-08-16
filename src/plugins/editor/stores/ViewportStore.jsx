import { createStore } from 'solid-js/store'

const [viewportStore, setViewportStore] = createStore({
  showGrid: true,
  gridSnapping: false,
  renderMode: 'solid',
  
  tabs: [],
  activeTabId: null,
  suspendedTabs: [],
  
  camera: {
    speed: 5,
    mouseSensitivity: 0.002,
    mode: 'orbit',
    position: [0, 0, 5],
    target: [0, 0, 0]
  }
})

const [nodeEditorStore, setNodeEditorStore] = createStore({
  graphs: {}
})

const [objectPropertiesStore, setObjectPropertiesStore] = createStore({
  objects: {}
})

export const viewportActions = {
  setShowGrid: (show) => {
    setViewportStore('showGrid', show)
  },
  
  setGridSnapping: (snap) => {
    setViewportStore('gridSnapping', snap)
  },
  
  setRenderMode: (mode) => {
    setViewportStore('renderMode', mode)
  },
  
  setCameraSpeed: (speed) => {
    setViewportStore('camera', 'speed', speed)
  },
  
  setCameraSensitivity: (sensitivity) => {
    setViewportStore('camera', 'mouseSensitivity', sensitivity)
  },
  
  setViewportCameraMode: (mode) => {
    setViewportStore('camera', 'mode', mode)
  },
  
  updateViewportCamera: (settings) => {
    setViewportStore('camera', settings)
  },
  
  setActiveViewportTab: (tabId) => {
    setViewportStore('activeTabId', tabId)
  },
  
  addViewportTab: (tab) => {
    setViewportStore('tabs', (tabs) => [...tabs, tab])
  },
  
  createNodeTab: (objectId, objectName) => {
    const tabId = `node-editor-${objectId}-${Date.now()}`
    const tab = {
      id: tabId,
      type: 'node-editor',
      name: `${objectName || objectId} Nodes`,
      objectId: objectId,
      isPinned: false,
      isActive: true
    }
    
    const existingTabIndex = viewportStore.tabs.findIndex(
      t => t.type === 'node-editor' && t.objectId === objectId
    )
    
    if (existingTabIndex !== -1) {
      setViewportStore('activeTabId', viewportStore.tabs[existingTabIndex].id)
      return viewportStore.tabs[existingTabIndex]
    }
    
    setViewportStore('tabs', viewportStore.tabs.length, tab)
    setViewportStore('activeTabId', tabId)
    
    return tab
  },
  
  removeViewportTab: (tabId) => {
    const tabs = viewportStore.tabs;
    const index = tabs.findIndex(tab => tab.id === tabId);
    
    if (index !== -1 && tabs.length > 1) {
      // If we're removing the active tab, switch to another one
      if (viewportStore.activeTabId === tabId) {
        const newActiveTab = tabs[index === 0 ? 1 : index - 1];
        setViewportStore('activeTabId', newActiveTab.id);
      }
      
      setViewportStore('tabs', tabs => tabs.filter(tab => tab.id !== tabId));
    }
  },
  
  closeViewportTab: (tabId) => {
    viewportActions.removeViewportTab(tabId);
  },
  
  pinViewportTab: (tabId) => {
    const index = viewportStore.tabs.findIndex(tab => tab.id === tabId);
    if (index !== -1) {
      setViewportStore('tabs', index, 'isPinned', (pinned) => !pinned);
    }
  },
  
  duplicateViewportTab: (tabId) => {
    const tab = viewportStore.tabs.find(t => t.id === tabId);
    if (tab) {
      const newTabId = `viewport-${Date.now()}`;
      const newTab = {
        ...tab,
        id: newTabId,
        name: `${tab.name} (Copy)`,
        isPinned: false
      };
      viewportActions.addViewportTab(newTab);
      viewportActions.setActiveViewportTab(newTabId);
    }
  },
  
  renameViewportTab: (tabId, newName) => {
    const index = viewportStore.tabs.findIndex(tab => tab.id === tabId);
    if (index !== -1) {
      setViewportStore('tabs', index, 'name', newName);
    }
  }
}

export const nodeEditorActions = {
  getNodeGraph: (objectId) => {
    return nodeEditorStore.graphs[objectId] || null
  },

  setNodeGraph: (objectId, graph) => {
    setNodeEditorStore('graphs', objectId, graph)
  },

  updateNodeGraph: (objectId, updates) => {
    if (nodeEditorStore.graphs[objectId]) {
      setNodeEditorStore('graphs', objectId, updates)
    }
  }
}

export const objectPropertiesActions = {
  ensureDefaultComponents: (objectId) => {
    if (!objectPropertiesStore.objects[objectId]) {
      setObjectPropertiesStore('objects', objectId, {
        transform: {
          position: [0, 0, 0],
          rotation: [0, 0, 0], 
          scale: [1, 1, 1]
        },
        components: {
          scripting: {
            enabled: false,
            scriptFiles: []
          }
        },
        nodeBindings: {}
      })
    }
    
    console.log(`✅ ensureDefaultComponents - Added default components for object: ${objectId}`)
  },

  initializeObjectProperties: (objectId) => {
    if (!objectPropertiesStore.objects[objectId]) {
      setObjectPropertiesStore('objects', objectId, {
        nodeBindings: {}
      })
    }
    return objectPropertiesStore.objects[objectId]
  },

  updateObjectProperty: (objectId, propertyPath, value) => {
    console.log('🎯 Object Properties Store - updateObjectProperty called:', { objectId, propertyPath, value })
    const pathParts = propertyPath.split('.')
    
    if (!objectPropertiesStore.objects[objectId]) {
      objectPropertiesActions.initializeObjectProperties(objectId)
    }
    
    let path = ['objects', objectId]
    pathParts.forEach(part => path.push(part))
    
    setObjectPropertiesStore(...path, value)
    console.log('🎯 Object Properties Store - Property updated in store')
  },

  addPropertySection: (objectId, sectionName, defaultValues) => {
    if (!objectPropertiesStore.objects[objectId]) {
      objectPropertiesActions.initializeObjectProperties(objectId)
    }
    if (!objectPropertiesStore.objects[objectId][sectionName]) {
      setObjectPropertiesStore('objects', objectId, sectionName, defaultValues)
    }
  },

  bindNodeToProperty: (objectId, propertyPath, nodeId, outputId) => {
    if (!objectPropertiesStore.objects[objectId]) {
      objectPropertiesActions.initializeObjectProperties(objectId)
    }
    setObjectPropertiesStore('objects', objectId, 'nodeBindings', propertyPath, `${nodeId}.${outputId}`)
  },

  unbindNodeFromProperty: (objectId, propertyPath) => {
    if (objectPropertiesStore.objects[objectId]?.nodeBindings) {
      setObjectPropertiesStore('objects', objectId, 'nodeBindings', propertyPath, undefined)
    }
  },

  getObjectProperties: (objectId) => {
    return objectPropertiesStore.objects[objectId] || null
  }
}

export { viewportStore, nodeEditorStore, objectPropertiesStore }

if (typeof window !== 'undefined') {
  window.viewportStore = viewportStore
  window.nodeEditorStore = nodeEditorStore
  window.objectPropertiesStore = objectPropertiesStore
  window.viewportActions = viewportActions
  window.nodeEditorActions = nodeEditorActions
  window.objectPropertiesActions = objectPropertiesActions
}