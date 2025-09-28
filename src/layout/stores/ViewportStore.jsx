import { createStore } from 'solid-js/store'

const [viewportStore, setViewportStore] = createStore({
  showGrid: true,
  gridSnapping: false,
  renderMode: 'solid',
  
  tabs: [],
  activeTabId: null,
  suspendedTabs: [],
  
  camera: {
    speed: 2,
    mouseSensitivity: 0.004,
    friction: 2,
    mode: 'orbit',
    type: 'universal',
    position: [0, 0, 5],
    target: [0, 0, 0]
  },
  
  lighting: {
    sunIntensity: 4.0,
    skyIntensity: 4.0,
    rimIntensity: 0.4,
    bounceIntensity: 0.3,
    moonIntensity: 15.0,
    nightTurbidity: 48,
    baseLuminance: 0.1,
    sunColor: [1.0, 0.98, 0.9],
    skyColor: [0.8, 0.9, 1.0],
    rimColor: [0.9, 0.7, 0.5],
    bounceColor: [0.4, 0.5, 0.7]
  }
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
  
  setCameraFriction: (friction) => {
    setViewportStore('camera', 'friction', friction)
  },
  
  setViewportCameraMode: (mode) => {
    setViewportStore('camera', 'mode', mode)
  },

  setCameraType: (type) => {
    setViewportStore('camera', 'type', type)
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
  
  
  removeViewportTab: (tabId) => {
    const tabs = viewportStore.tabs;
    const index = tabs.findIndex(tab => tab.id === tabId);
    
    if (index !== -1 && tabs.length > 1) {
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
  },
  
  setTabUnsavedChanges: (tabId, hasUnsavedChanges) => {
    const index = viewportStore.tabs.findIndex(tab => tab.id === tabId);
    if (index !== -1) {
      setViewportStore('tabs', index, 'hasUnsavedChanges', hasUnsavedChanges);
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
  },

  syncBabylonObjectProperties: (objectId) => {
    // Sync properties from Babylon object to the properties store
    // Note: This creates a circular dependency, so we'll implement this differently
    // by calling it from the Scene.jsx component where the render store is already imported
    console.log('syncBabylonObjectProperties called for:', objectId);
  }
}

export { viewportStore, objectPropertiesStore }

if (typeof window !== 'undefined') {
  window.viewportStore = viewportStore
  window.objectPropertiesStore = objectPropertiesStore
  window.viewportActions = viewportActions
  window.objectPropertiesActions = objectPropertiesActions
}
