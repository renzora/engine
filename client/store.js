import { proxy, ref } from 'valtio'
import { devtools } from 'valtio/utils'
import * as BABYLON from '@babylonjs/core'

// External Babylon.js scene reference (not in Valtio store to avoid performance issues)
export let babylonScene = { current: null }

// Helper to sync Babylon.js scene to lightweight store metadata
const syncSceneToStore = (scene) => {
  console.log('🔄 syncSceneToStore - Starting scene sync');
  if (!scene) {
    console.log('❌ syncSceneToStore - No scene provided');
    globalStore.editor.scene.isLoaded = false
    globalStore.editor.scene.objects.meshes = []
    globalStore.editor.scene.objects.transformNodes = []
    globalStore.editor.scene.objects.lights = []
    globalStore.editor.scene.objects.cameras = []
    globalStore.editor.scene.hierarchy = []
    return
  }

  console.log('🔍 syncSceneToStore - Scene analysis:', {
    totalMeshes: scene.meshes?.length || 0,
    totalTransformNodes: scene.transformNodes?.length || 0,
    totalLights: scene.lights?.length || 0,
    totalCameras: scene.cameras?.length || 0
  });

  // Extract lightweight mesh data with parent info (excluding internal meshes)
  // Include ALL meshes, even child meshes, so they can be individually selected and scripted
  const allMeshes = scene.meshes || [];
  console.log('🔍 syncSceneToStore - All meshes in scene:', allMeshes.map(m => ({
    name: m.name,
    isInternal: m._isInternalMesh,
    hasParent: !!m.parent,
    parentName: m.parent?.name,
    hasMaterial: !!m.material,
    materialName: m.material?.name
  })));

  const meshes = allMeshes
    .filter(mesh => {
      const isInternal = mesh._isInternalMesh;
      
      // System meshes (grid, gizmos, etc.) should be filtered regardless
      const isSystemMesh = mesh.name && (
        mesh.name.startsWith('__') && mesh.name !== '__root__' || // Keep __root__ but filter other system meshes
        mesh.name.includes('gizmo') ||
        mesh.name.includes('helper') ||
        mesh.name.includes('_internal_')
      );
      
      if (isSystemMesh) {
        console.log(`🚫 syncSceneToStore - Filtering out system mesh: ${mesh.name}`);
        return false;
      }
      
      // For internal meshes, only show them if their parent has been explicitly unpacked
      if (isInternal) {
        const parentId = mesh.parent ? (mesh.parent.uniqueId || mesh.parent.name) : null;
        const isUnpacked = parentId && globalStore.editor.scene.unpackedObjects?.includes(parentId);
        
        if (isUnpacked) {
          console.log(`✅ syncSceneToStore - Including unpacked internal mesh: ${mesh.name} (parent: ${parentId})`);
          return true;
        } else {
          console.log(`🚫 syncSceneToStore - Hiding internal mesh (parent not unpacked): ${mesh.name} (parent: ${parentId})`);
          return false;
        }
      }
      
      // Non-internal meshes are always shown
      return true;
    })
    .map(mesh => {
      const objectId = mesh.uniqueId || mesh.name || `mesh-${Math.random()}`;
      
      // Ensure default components exist for this mesh
      actions.editor.ensureDefaultComponents(objectId);
      
      return {
        id: objectId,
        name: mesh.name || 'Unnamed Mesh',
        type: 'mesh',
        visible: mesh.isVisible !== undefined ? mesh.isVisible : true,
        position: mesh.position ? [mesh.position.x, mesh.position.y, mesh.position.z] : [0, 0, 0],
        parentId: mesh.parent ? (mesh.parent.uniqueId || mesh.parent.name) : null,
        // Add material info for better debugging
        materialName: mesh.material ? mesh.material.name : null,
        materialType: mesh.material ? mesh.material.getClassName() : null
      };
    });

  console.log('✅ syncSceneToStore - Processed meshes:', meshes.map(m => ({
    name: m.name,
    parentId: m.parentId,
    hasMaterial: !!m.materialName
  })));

  // Extract transform nodes (these act as parent containers, excluding internal nodes)
  const allTransformNodes = scene.transformNodes || [];
  console.log('🔍 syncSceneToStore - All transform nodes in scene:', allTransformNodes.map(n => ({
    name: n.name,
    isInternal: n._isInternalNode,
    hasParent: !!n.parent,
    parentName: n.parent?.name,
    childMeshCount: n.getChildMeshes?.()?.length || 0
  })));

  const transformNodes = allTransformNodes
    .filter(node => {
      const isInternal = node._isInternalNode;
      
      // System nodes should be filtered regardless
      const isSystemNode = node.name && (
        node.name.startsWith('__') ||
        node.name.includes('gizmo') ||
        node.name.includes('helper') ||
        node.name.includes('_internal_')
      );
      
      if (isSystemNode) {
        console.log(`🚫 syncSceneToStore - Filtering out system transform node: ${node.name}`);
        return false;
      }
      
      // For internal transform nodes, only show them if their parent has been explicitly unpacked
      if (isInternal) {
        const parentId = node.parent ? (node.parent.uniqueId || node.parent.name) : null;
        const isUnpacked = parentId && globalStore.editor.scene.unpackedObjects?.includes(parentId);
        
        if (isUnpacked) {
          console.log(`✅ syncSceneToStore - Including unpacked internal transform node: ${node.name} (parent: ${parentId})`);
          return true;
        } else {
          console.log(`🚫 syncSceneToStore - Hiding internal transform node (parent not unpacked): ${node.name} (parent: ${parentId})`);
          return false;
        }
      }
      
      // Non-internal transform nodes are always shown
      return true;
    })
    .map(node => {
      const objectId = node.uniqueId || node.name || `transform-${Math.random()}`;
      
      // Ensure default components exist for this transform node
      actions.editor.ensureDefaultComponents(objectId);
      
      return {
        id: objectId,
        name: node.name || 'Unnamed Transform',
        type: 'mesh', // Show transform nodes as meshes in UI for simplicity
        visible: true,
        position: node.position ? [node.position.x, node.position.y, node.position.z] : [0, 0, 0],
        parentId: node.parent ? (node.parent.uniqueId || node.parent.name) : null
      };
    });

  console.log('✅ syncSceneToStore - Processed transform nodes:', transformNodes.map(n => ({
    name: n.name,
    parentId: n.parentId,
    id: n.id
  })));

  // Extract lightweight light data
  const lights = (scene.lights || []).map(light => {
    const objectId = light.uniqueId || light.name || `light-${Math.random()}`;
    
    // Ensure default components exist for this light
    actions.editor.ensureDefaultComponents(objectId);
    
    return {
      id: objectId,
      name: light.name || 'Unnamed Light',
      type: 'light',
      visible: light.isEnabled !== undefined ? light.isEnabled() : true,
      intensity: light.intensity !== undefined ? light.intensity : 1,
      parentId: light.parent ? (light.parent.uniqueId || light.parent.name) : null
    };
  })

  // Extract lightweight camera data
  const cameras = (scene.cameras || []).map(camera => {
    const objectId = camera.uniqueId || camera.name || `camera-${Math.random()}`;
    
    // Ensure default components exist for this camera
    actions.editor.ensureDefaultComponents(objectId);
    
    return {
      id: objectId,
      name: camera.name || 'Unnamed Camera',
      type: 'camera',
      visible: true, // Cameras are typically always visible in the hierarchy
      active: scene.activeCamera === camera,
      parentId: camera.parent ? (camera.parent.uniqueId || camera.parent.name) : null
    };
  })

  // Build hierarchical structure
  const allObjects = [...meshes, ...transformNodes, ...lights, ...cameras]
  const objectMap = new Map(allObjects.map(obj => [obj.id, obj]))
  
  // Function to build hierarchy recursively
  const buildHierarchyNode = (obj) => {
    const children = allObjects
      .filter(child => child.parentId === obj.id)
      .map(child => buildHierarchyNode(child))
    
    // Check if this object has internal child meshes (for unpack functionality)
    const scene = babylonScene?.current;
    let hasChildMeshes = false;
    if (scene) {
      const babylonObject = [...(scene.meshes || []), ...(scene.transformNodes || [])].find(bObj => 
        (bObj.uniqueId || bObj.name) === obj.id
      );
      if (babylonObject && babylonObject.getChildMeshes) {
        const childMeshes = babylonObject.getChildMeshes();
        hasChildMeshes = childMeshes.length > 0;
      }
    }
    
    // Check if this object is currently unpacked
    const isUnpacked = globalStore.editor.scene.unpackedObjects.includes(obj.id);
    
    // For imported models with many children, expand them by default to show individual parts
    const hasChildren = children.length > 0;
    const isModelContainer = hasChildren && children.length > 10; // Likely an imported model
    
    console.log(`🏗️ Building hierarchy node for ${obj.name}: ${children.length} children, ${hasChildMeshes ? 'has' : 'no'} child meshes, ${isUnpacked ? 'unpacked' : 'packed'}`);
    
    return {
      id: obj.id,
      name: obj.name,
      type: obj.type,
      visible: obj.visible,
      children: hasChildren ? children : undefined,
      expanded: isModelContainer || isUnpacked, // Expand model containers by default or when unpacked
      hasChildMeshes, // For context menu logic
      isUnpacked // For context menu logic
    }
  }
  
  // Get root level objects (no parent)
  const rootObjects = allObjects.filter(obj => !obj.parentId)
  
  const hierarchy = [
    {
      id: 'scene-root',
      name: globalStore.editor.scene.name,
      type: 'scene',
      expanded: true,
      children: rootObjects.map(obj => buildHierarchyNode(obj))
    }
  ]

  // Update store
  console.log('📝 syncSceneToStore - Final results:', {
    totalMeshes: meshes.length,
    totalTransformNodes: transformNodes.length,
    totalLights: lights.length,
    totalCameras: cameras.length,
    hierarchyRoots: hierarchy.length,
    hierarchyChildren: hierarchy[0]?.children?.length || 0
  });
  
  console.log('🗂️ syncSceneToStore - Hierarchy structure:', JSON.stringify(hierarchy, null, 2));
  
  globalStore.editor.scene.isLoaded = true
  globalStore.editor.scene.objects.meshes = meshes
  globalStore.editor.scene.objects.transformNodes = transformNodes
  globalStore.editor.scene.objects.lights = lights
  globalStore.editor.scene.objects.cameras = cameras
  globalStore.editor.scene.hierarchy = hierarchy
}

// Create the global store
export const globalStore = proxy({
  editor: {
    isOpen: false,
    mode: 'scene',
    
    // UI settings
    ui: {
      rightPanelWidth: 304,
      bottomPanelHeight: 256,
      scenePropertiesHeight: 450,
      selectedTool: 'scene',
      selectedBottomTab: 'assets',
      // Toolbar configuration
      toolbarTabOrder: [
        'scene', 'light', 'effects', 'folder', 'star', 'wifi', 'cloud', 'monitor'
      ],
      toolbarBottomTabOrder: [
        'add', 'settings', 'fullscreen'
      ],
      // Bottom tabs configuration
      bottomTabOrder: [
        'assets'
      ]
    },
    
    // Panel state
    panels: {
      isResizingPanels: false,
      isScenePanelOpen: true,
      isAssetPanelOpen: true
    },
    
    // Selection
    selection: {
      entity: null,
      object: null,
      transformMode: 'select'
    },
    
    // Scene state - lightweight metadata for UI reactivity
    scene: {
      isLoaded: false,
      name: 'Untitled Scene',
      selectedObjectId: null,
      unpackedObjects: [], // Array of object IDs that should show their internal meshes
      
      // Lightweight object lists for UI (not full Babylon.js objects)
      objects: {
        meshes: [
          // { id: 'mesh-1', name: 'Cube', type: 'mesh', visible: true, position: [0,0,0] }
        ],
        transformNodes: [
          // { id: 'transform-1', name: 'Model Root', type: 'transform', visible: true, position: [0,0,0] }
        ],
        lights: [
          // { id: 'light-1', name: 'Directional Light', type: 'light', intensity: 1 }
        ],
        cameras: [
          // { id: 'camera-1', name: 'Main Camera', type: 'camera', active: true }
        ]
      },
      
      // Scene hierarchy for tree view
      hierarchy: [
        // { id: 'scene-root', name: 'Scene', type: 'scene', children: ['mesh-1', 'light-1'] }
      ]
    },
    
    // Viewport state
    viewport: {
      showGrid: true,
      gridSnapping: false,
      renderMode: 'solid',
      // Multi-tab viewport system
      tabs: [
        {
          id: 'viewport-1',
          type: '3d-viewport',
          name: 'Scene 1',
          isPinned: false,
          hasUnsavedChanges: false
        }
      ],
      activeTabId: 'viewport-1',
      suspendedTabs: []
    },

    // Node Editor state
    nodeEditor: {
      // Node graphs by object ID
      graphs: {
        // [objectId]: {
        //   nodes: [...],
        //   connections: [...],
        //   viewTransform: { x: 0, y: 0, scale: 1 }
        // }
      }
    },

    // Object Properties state
    objectProperties: {
      // Properties by object ID
      objects: {
        // [objectId]: {
        //   transform: { position: [0,0,0], rotation: [0,0,0], scale: [1,1,1] },
        //   material: { baseColor: '#ffffff', roughness: 0.5, metallic: 0.0 },
        //   components: { scripting: { scriptFile: null }, physics: { enabled: false } },
        //   nodeBindings: { 'material.baseColor': 'nodeId.outputId' }
        // }
      }
    },
    
    // Settings
    settings: {
      viewport: {
        backgroundColor: '#1a202c',
        renderingEngine: 'webgl'
      },
      editor: {
        showStats: true,
        panelPosition: 'right' // 'left' or 'right'
      },
      grid: {
        enabled: true,
        size: 20, // Reduced from 100 to 20 meters for realistic room/studio size
        cellSize: 1, // 1 meter per cell
        unit: 'meters', // meters, centimeters, millimeters, feet, inches
        position: [0, 0, 0],
        cellColor: '#555555',
        sectionColor: '#888888',
        sectionSize: 10, // Every 10th line is a section line (10 meter sections)
        infiniteGrid: false
      },
      
      // Scene scale settings
      scene: {
        worldUnit: 'meters', // The unit that 1 Babylon unit represents
        floorSize: 20 // Floor plane size in world units (20x20 meters = realistic studio/room)
      }
    },
    
    // Viewport camera (editor navigation only)
    viewportCamera: {
      speed: 5,
      mouseSensitivity: 0.002,
      mode: 'orbit', // orbit, fly, fps
      position: [0, 0, 5],
      target: [0, 0, 0]
    },
    
    // Backward compatibility - points to viewportCamera
    get camera() {
      return globalStore.editor.viewportCamera
    },
    
    // Console state
    console: {
      contextMenuHandler: null,
      messages: []
    },

    // Asset cache state
    assets: {
      // Current project name for cache validation
      currentProject: null,
      
      // Folder tree cache
      folderTree: null,
      folderTreeTimestamp: null,
      
      // Asset categories cache (for type view)
      categories: null,
      categoriesTimestamp: null,
      
      // Assets by path cache (for folder view)
      assetsByPath: {
        // '': { assets: [...], timestamp: 1234567890 },
        // 'models': { assets: [...], timestamp: 1234567890 }
      },
      
      // Cache settings
      cacheExpiryMs: 5 * 60 * 1000 // 5 minutes
    }
  }
})

// Actions
export const actions = {
  editor: {
    toggleOpen: () => {
      globalStore.editor.isOpen = !globalStore.editor.isOpen
    },
    
    setSelectedTool: (tool) => {
      globalStore.editor.ui.selectedTool = tool
    },
    
    setSelectedEntity: (entityId) => {
      console.log('🏪 Store - setSelectedEntity called:', {
        'old value': globalStore.editor.selection.entity,
        'new value': entityId
      });
      globalStore.editor.selection.entity = entityId
      console.log('🏪 Store - setSelectedEntity completed, current value:', globalStore.editor.selection.entity);
    },

    // Complete object selection with gizmo, highlighting, and store updates
    selectObject: (objectId) => {
      console.log('🏪 Store - selectObject called with ID:', objectId);
      
      const scene = babylonScene?.current;
      if (!scene) {
        console.warn('🏪 Store - No Babylon scene available for selection');
        return;
      }

      if (objectId) {
        // Find the Babylon.js object
        const allObjects = [...(scene.meshes || []), ...(scene.transformNodes || []), ...(scene.lights || []), ...(scene.cameras || [])];
        const babylonObject = allObjects.find(obj => 
          (obj.uniqueId || obj.name) === objectId
        );

        if (babylonObject) {
          console.log('🏪 Store - Found Babylon object for selection:', babylonObject.name);
          
          // Clear previous highlight
          if (scene._highlightLayer) {
            console.log('🎨 Store - Clearing previous highlights');
            scene._highlightLayer.removeAllMeshes();
          }
          
          // Attach gizmo to the object
          if (scene._gizmoManager) {
            scene._gizmoManager.attachToMesh(babylonObject);
            
            // Ensure gizmo thickness is applied
            if (scene._ensureGizmoThickness) {
              scene._ensureGizmoThickness();
            }
          }
          
          // Add yellow outline to selected object
          if (scene._highlightLayer) {
            console.log('🎨 Store - Adding highlight to object:', babylonObject.getClassName(), babylonObject.name);
            try {
              // Handle different object types for highlighting
              if (babylonObject.getClassName() === 'TransformNode') {
                // For TransformNodes, highlight all child meshes
                const childMeshes = babylonObject.getChildMeshes();
                console.log('🎨 Store - TransformNode child meshes to highlight:', childMeshes.length);
                childMeshes.forEach((childMesh, index) => {
                  if (childMesh.getClassName() === 'Mesh') {
                    console.log(`🎨 Store - Highlighting child mesh ${index}:`, childMesh.name);
                    scene._highlightLayer.addMesh(childMesh, BABYLON.Color3.Yellow());
                  }
                });
              } else if (babylonObject.getClassName() === 'Mesh') {
                // Direct mesh highlighting
                console.log('🎨 Store - Highlighting direct mesh:', babylonObject.name);
                scene._highlightLayer.addMesh(babylonObject, BABYLON.Color3.Yellow());
              }
              console.log('✅ Store - Highlighting completed');
            } catch (highlightError) {
              console.error('❌ Store - Could not add highlight to selected object:', highlightError);
            }
          } else {
            console.warn('🎨 Store - No highlight layer available');
          }
          
          // Update selection in store
          globalStore.editor.selection.entity = objectId;
          globalStore.editor.scene.selectedObjectId = objectId;
          
          console.log('✅ Store - Object selection completed:', babylonObject.name, 'ID:', objectId);
        } else {
          console.warn('🏪 Store - Could not find Babylon.js object with ID:', objectId);
          // Still update store even if object not found in scene
          globalStore.editor.selection.entity = objectId;
          globalStore.editor.scene.selectedObjectId = objectId;
        }
      } else {
        // Clear selection
        console.log('🎨 Store - Clearing selection and highlights');
        if (scene._gizmoManager) {
          scene._gizmoManager.attachToMesh(null);
        }
        if (scene._highlightLayer) {
          console.log('🎨 Store - Removing all mesh highlights');
          scene._highlightLayer.removeAllMeshes();
        }
        globalStore.editor.selection.entity = null;
        globalStore.editor.scene.selectedObjectId = null;
        
        console.log('✅ Store - Selection cleared');
      }
    },
    
    updateViewportSettings: (settings) => {
      Object.assign(globalStore.editor.settings.viewport, settings)
    },
    
    setBabylonScene: (scene) => {
      // Update external reference (not in Valtio store)
      babylonScene.current = scene
      
      // Sync lightweight metadata to store for UI reactivity
      syncSceneToStore(scene)
      
      console.log('Babylon.js scene updated:', scene ? 'loaded' : 'cleared')
    },
    
    updateBabylonScene: (scene) => {
      // Same as setBabylonScene for consistency
      babylonScene.current = scene
      syncSceneToStore(scene)
    },
    
    // Panel actions
    setScenePanelOpen: (isOpen) => {
      globalStore.editor.panels.isScenePanelOpen = isOpen
    },
    
    setAssetPanelOpen: (isOpen) => {
      globalStore.editor.panels.isAssetPanelOpen = isOpen
    },
    
    setResizingPanels: (isResizing) => {
      globalStore.editor.panels.isResizingPanels = isResizing
    },

    setRightPanelWidth: (width) => {
      globalStore.editor.ui.rightPanelWidth = width
    },

    setBottomPanelHeight: (height) => {
      globalStore.editor.ui.bottomPanelHeight = height
    },
    
    setScenePropertiesHeight: (height) => {
      globalStore.editor.ui.scenePropertiesHeight = height
    },
    
    setContextMenuHandler: (handler) => {
      globalStore.editor.console.contextMenuHandler = handler
    },
    
    addConsoleMessage: (message, type = 'info') => {
      globalStore.editor.console.messages.push({
        message,
        type,
        timestamp: Date.now()
      })
    },
    
    setShowGrid: (show) => {
      globalStore.editor.viewport.showGrid = show
    },
    
    setGridSnapping: (snap) => {
      globalStore.editor.viewport.gridSnapping = snap
    },
    
    setRenderMode: (mode) => {
      globalStore.editor.viewport.renderMode = mode
    },
    
    setCameraSpeed: (speed) => {
      globalStore.editor.viewportCamera.speed = speed
    },
    
    setCameraSensitivity: (sensitivity) => {
      globalStore.editor.viewportCamera.mouseSensitivity = sensitivity
    },
    
    setViewportCameraMode: (mode) => {
      globalStore.editor.viewportCamera.mode = mode
    },
    
    updateViewportCamera: (settings) => {
      Object.assign(globalStore.editor.viewportCamera, settings)
    },
    
    setTransformMode: (mode) => {
      globalStore.editor.selection.transformMode = mode
    },
    
    updateGridSettings: (settings) => {
      Object.assign(globalStore.editor.settings.grid, settings)
    },
    
    setActiveViewportTab: (tabId) => {
      globalStore.editor.viewport.activeTabId = tabId
    },
    
    addViewportTab: (tab) => {
      globalStore.editor.viewport.tabs.push(tab)
    },
    
    createNodeEditorTab: (objectId, objectName) => {
      const tabId = `node-editor-${objectId}-${Date.now()}`;
      const tab = {
        id: tabId,
        type: 'node-editor',
        name: `${objectName || objectId} Nodes`,
        objectId: objectId,
        isPinned: false,
        isActive: true
      };
      
      const existingTab = globalStore.editor.viewport.tabs.find(
        t => t.type === 'node-editor' && t.objectId === objectId
      );
      
      if (existingTab) {
        globalStore.editor.viewport.activeTabId = existingTab.id;
        return existingTab;
      }
      
      globalStore.editor.viewport.tabs.push(tab);
      globalStore.editor.viewport.activeTabId = tabId;
      
      return tab;
    },
    
    removeViewportTab: (tabId) => {
      const tabs = globalStore.editor.viewport.tabs
      const index = tabs.findIndex(tab => tab.id === tabId)
      if (index !== -1) {
        tabs.splice(index, 1)
      }
    },

    getNodeGraph: (objectId) => {
      return globalStore.editor.nodeEditor.graphs[objectId] || null
    },

    setNodeGraph: (objectId, graph) => {
      globalStore.editor.nodeEditor.graphs[objectId] = graph
    },

    updateNodeGraph: (objectId, updates) => {
      const graph = globalStore.editor.nodeEditor.graphs[objectId]
      if (graph) {
        Object.assign(graph, updates)
      }
    },

    deleteNodeAndCleanupProperties: (objectId, nodeId) => {
      const graph = globalStore.editor.nodeEditor.graphs[objectId];
      if (!graph) return;

      console.log('Deleting node and cleaning up properties:', nodeId);

      const nodeToDelete = graph.nodes.find(n => n.id === nodeId);
      
      if (nodeToDelete?.type === 'output') {
        console.log('Deleting output node - cascading to connected nodes');
        
        const connectedInputNodes = graph.connections
          .filter(conn => conn.to.nodeId === nodeId)
          .map(conn => conn.from.nodeId);
        
        connectedInputNodes.forEach(inputNodeId => {
          actions.editor.deleteNodeAndCleanupProperties(objectId, inputNodeId);
        });
      }

      const connectionsToRemove = graph.connections.filter(c => 
        c.from.nodeId === nodeId || c.to.nodeId === nodeId
      );

      const updatedNodes = graph.nodes.filter(n => n.id !== nodeId);
      const updatedConnections = graph.connections.filter(c => 
        c.from.nodeId !== nodeId && c.to.nodeId !== nodeId
      );

      Object.assign(graph, {
        nodes: updatedNodes,
        connections: updatedConnections
      });

      const objectProps = globalStore.editor.objectProperties.objects[objectId];
      if (objectProps && nodeToDelete?.type === 'output') {
        if (nodeToDelete.title === 'Material Output' && objectProps.material) {
          console.log('Removing material properties - Material Output node deleted');
          delete objectProps.material;
        }
        
        if (nodeToDelete.title === 'Transform Output' && objectProps.transform) {
          console.log('Removing transform properties - Transform Output node deleted');
          delete objectProps.transform;
        }
      }

      if (objectProps && objectProps.nodeBindings) {
        const updatedBindings = {};
        Object.keys(objectProps.nodeBindings).forEach(propertyPath => {
          const binding = objectProps.nodeBindings[propertyPath];
          if (binding.nodeId !== nodeId) {
            updatedBindings[propertyPath] = binding;
          } else {
            console.log('Removing property binding for:', propertyPath);
          }
        });
        
        objectProps.nodeBindings = updatedBindings;
      }
    },

    addNodeToGraph: (objectId, node) => {
      if (!globalStore.editor.nodeEditor.graphs[objectId]) {
        globalStore.editor.nodeEditor.graphs[objectId] = {
          nodes: [],
          connections: [],
          viewTransform: { x: 0, y: 0, scale: 1 }
        }
      }
      globalStore.editor.nodeEditor.graphs[objectId].nodes.push(node)
    },

    removeNodeFromGraph: (objectId, nodeId) => {
      const graph = globalStore.editor.nodeEditor.graphs[objectId]
      if (graph) {
        graph.nodes = graph.nodes.filter(n => n.id !== nodeId)
        graph.connections = graph.connections.filter(c => 
          c.from.nodeId !== nodeId && c.to.nodeId !== nodeId
        )
      }
    },

    addConnectionToGraph: (objectId, connection) => {
      const graph = globalStore.editor.nodeEditor.graphs[objectId]
      if (graph) {
        graph.connections = graph.connections.filter(c => 
          !(c.to.nodeId === connection.to.nodeId && c.to.portId === connection.to.portId)
        )
        graph.connections.push(connection)
      }
    },

    addConnectionAndGenerateProperties: (objectId, connection) => {
      const graph = globalStore.editor.nodeEditor.graphs[objectId];
      if (!graph) return;

      actions.editor.addConnectionToGraph(objectId, connection);
      const fromNode = graph.nodes.find(n => n.id === connection.from.nodeId);
      const toNode = graph.nodes.find(n => n.id === connection.to.nodeId);

      console.log('Connection created between:', fromNode?.title, '->', toNode?.title);

      if (fromNode && toNode) {
        if (toNode.type === 'output' && toNode.title === 'Material Output') {
          console.log('Updating material properties for Material Output connection');
          
          let materialProps = actions.editor.initializeObjectProperties(objectId).material || {};
          
          const inputPort = toNode.inputs.find(input => input.id === connection.to.portId);
          if (inputPort) {
            switch (inputPort.name) {
              case 'Base Color':
                materialProps.baseColor = '#ffffff';
                break;
              case 'Roughness':
                materialProps.roughness = 0.5;
                break;
              case 'Metallic':
                materialProps.metallic = 0.0;
                break;
            }
          }
          
          actions.editor.addPropertySection(objectId, 'material', materialProps);
        }

        if (toNode.type === 'output' && toNode.title === 'Transform Output') {
          console.log('Updating transform properties for Transform Output connection');
          
          let transformProps = actions.editor.initializeObjectProperties(objectId).transform || {};
          
          const inputPort = toNode.inputs.find(input => input.id === connection.to.portId);
          if (inputPort) {
            switch (inputPort.name) {
              case 'Position':
                transformProps.position = [0, 0, 0];
                break;
              case 'Rotation':
                transformProps.rotation = [0, 0, 0];
                break;
              case 'Scale':
                transformProps.scale = [1, 1, 1];
                break;
            }
          }
          
          actions.editor.addPropertySection(objectId, 'transform', transformProps);
        }
      }

      actions.editor.updateBabylonObjectFromProperties(objectId);
    },

    removeConnectionFromGraph: (objectId, connectionId) => {
      const graph = globalStore.editor.nodeEditor.graphs[objectId]
      if (graph) {

        const connectionToRemove = graph.connections.find(c => c.id === connectionId);
        graph.connections = graph.connections.filter(c => c.id !== connectionId)

        if (connectionToRemove) {
          actions.editor.cleanupPropertiesAfterConnectionRemoval(objectId, connectionToRemove);
        }
      }
    },

    cleanupPropertiesAfterConnectionRemoval: (objectId, removedConnection) => {
      const graph = globalStore.editor.nodeEditor.graphs[objectId];
      const objectProps = globalStore.editor.objectProperties.objects[objectId];
      
      if (!graph || !objectProps) return;

      const fromNode = graph.nodes.find(n => n.id === removedConnection.from.nodeId);
      const toNode = graph.nodes.find(n => n.id === removedConnection.to.nodeId);

      if (!fromNode || !toNode) return;

      console.log('Cleaning up properties after connection removal:', fromNode?.title, '<-X->', toNode?.title);

      if (toNode.type === 'output') {
        const inputPort = toNode.inputs.find(input => input.id === removedConnection.to.portId);
        
        if (toNode.title === 'Material Output' && inputPort && objectProps.material) {
          switch (inputPort.name) {
            case 'Base Color':
              delete objectProps.material.baseColor;
              break;
            case 'Roughness':
              delete objectProps.material.roughness;
              break;
            case 'Metallic':
              delete objectProps.material.metallic;
              break;
          }
          
          if (Object.keys(objectProps.material).length === 0) {
            delete objectProps.material;
          }
        }
        
        if (toNode.title === 'Transform Output' && inputPort && objectProps.transform) {
          switch (inputPort.name) {
            case 'Position':
              delete objectProps.transform.position;
              break;
            case 'Rotation':
              delete objectProps.transform.rotation;
              break;
            case 'Scale':
              delete objectProps.transform.scale;
              break;
          }
          
          if (Object.keys(objectProps.transform).length === 0) {
            delete objectProps.transform;
          }
        }
      }

      actions.editor.updateBabylonObjectFromProperties(objectId);
    },

    initializeObjectProperties: (objectId) => {
      if (!globalStore.editor.objectProperties.objects[objectId]) {
        globalStore.editor.objectProperties.objects[objectId] = {
          nodeBindings: {}
        }
      }
      return globalStore.editor.objectProperties.objects[objectId]
    },

    ensureDefaultComponents: (objectId) => {
      const objectProps = actions.editor.initializeObjectProperties(objectId);
      
      if (!objectProps.transform) {
        objectProps.transform = {
          position: [0, 0, 0],
          rotation: [0, 0, 0], 
          scale: [1, 1, 1]
        };
      }
      
      if (!objectProps.components) {
        objectProps.components = {};
      }
      
      if (!objectProps.components.scripting) {
        objectProps.components.scripting = {
          enabled: false,
          scriptFiles: []
        };
      }
      
      console.log(`✅ ensureDefaultComponents - Added default components for object: ${objectId}`);
    },

    updateObjectProperty: (objectId, propertyPath, value) => {
      console.log('🎯 Store - updateObjectProperty called:', { objectId, propertyPath, value });
      const obj = actions.editor.initializeObjectProperties(objectId)
      const pathParts = propertyPath.split('.')
      
      let current = obj
      for (let i = 0; i < pathParts.length - 1; i++) {
        if (!current[pathParts[i]]) {
          current[pathParts[i]] = {}
        }
        current = current[pathParts[i]]
      }
      current[pathParts[pathParts.length - 1]] = value
      console.log('🎯 Store - Property updated in store. Full object now:', JSON.stringify(obj, null, 2));
    },

    addPropertySection: (objectId, sectionName, defaultValues) => {
      const obj = actions.editor.initializeObjectProperties(objectId)
      if (!obj[sectionName]) {
        obj[sectionName] = defaultValues
      }
    },

    bindNodeToProperty: (objectId, propertyPath, nodeId, outputId) => {
      const obj = actions.editor.initializeObjectProperties(objectId)
      obj.nodeBindings[propertyPath] = `${nodeId}.${outputId}`
    },

    unbindNodeFromProperty: (objectId, propertyPath) => {
      const obj = globalStore.editor.objectProperties.objects[objectId]
      if (obj && obj.nodeBindings) {
        delete obj.nodeBindings[propertyPath]
      }
    },

    getObjectProperties: (objectId) => {
      return globalStore.editor.objectProperties.objects[objectId] || null
    },
    
    setToolbarTabOrder: (order) => {
      globalStore.editor.ui.toolbarTabOrder = order
    },
    
    setToolbarBottomTabOrder: (order) => {
      globalStore.editor.ui.toolbarBottomTabOrder = order
    },
    
    setBottomTabOrder: (order) => {
      globalStore.editor.ui.bottomTabOrder = order
    },
    
    hydrateFromLocalStorage: () => {
      console.log('Hydrating from localStorage...')
    },
    
    toggleStats: () => {
      globalStore.editor.settings.editor.showStats = !globalStore.editor.settings.editor.showStats
    },
    
    updateEditorSettings: (settings) => {
      Object.assign(globalStore.editor.settings.editor, settings)
    },
    
    updateSceneMetadata: (metadata) => {
      Object.assign(globalStore.editor.scene, metadata)
    },
    
    setSceneName: (name) => {
      globalStore.editor.scene.name = name
    },
    
    selectSceneObject: (objectId) => {
      console.log('🏪 Store - selectSceneObject called:', {
        'old value': globalStore.editor.scene.selectedObjectId,
        'new value': objectId
      });
      globalStore.editor.scene.selectedObjectId = objectId
      console.log('🏪 Store - selectSceneObject completed, current value:', globalStore.editor.scene.selectedObjectId);
    },
    
    updateSceneObjectProperty: (objectId, property, value) => {
      const scene = babylonScene.current
      if (scene && property === 'visible') {
        const babylonObject = [...(scene.meshes || []), ...(scene.transformNodes || []), ...(scene.lights || []), ...(scene.cameras || [])]
          .find(obj => (obj.uniqueId || obj.name) === objectId)
        
        if (babylonObject) {
          if (babylonObject.getClassName() === 'TransformNode') {
            babylonObject.getChildMeshes().forEach(childMesh => {
              childMesh.isVisible = value
            })
          } else if ('isVisible' in babylonObject) {
            babylonObject.isVisible = value
          } else if ('setEnabled' in babylonObject) {
            babylonObject.setEnabled(value)
          }
        }
      }
      
      const meshes = globalStore.editor.scene.objects.meshes
      const transformNodes = globalStore.editor.scene.objects.transformNodes || []
      const lights = globalStore.editor.scene.objects.lights
      const cameras = globalStore.editor.scene.objects.cameras
      const allObjects = [...meshes, ...transformNodes, ...lights, ...cameras]
      const object = allObjects.find(obj => obj.id === objectId)
      
      if (object && property in object) {
        object[property] = value
        
        const updateHierarchyVisibility = (nodes) => {
          nodes.forEach(node => {
            if (node.id === objectId) {
              node.visible = value
            }
            if (node.children) {
              updateHierarchyVisibility(node.children)
            }
          })
        }
        
        updateHierarchyVisibility(globalStore.editor.scene.hierarchy)
      }
    },
    
    refreshSceneData: () => {
      console.log('🔄 Store - Refreshing scene data manually triggered');
      syncSceneToStore(babylonScene.current)
      console.log('✅ Store - Scene data refresh completed');
    },

    createFolder: (folderName, parentId = null) => {
      console.log(`📁 Store - Creating folder: ${folderName}, parent: ${parentId}`);
      
      const folderId = `folder-${Date.now()}`;
      const newFolder = {
        id: folderId,
        name: folderName,
        type: 'folder',
        visible: true,
        children: []
      };
      
      if (parentId) {
        const findAndAddToParent = (nodes) => {
          for (let node of nodes) {
            if (node.id === parentId) {
              if (!node.children) node.children = [];
              node.children.push(newFolder);
              return true;
            }
            if (node.children && findAndAddToParent(node.children)) {
              return true;
            }
          }
          return false;
        };
        
        if (!findAndAddToParent(globalStore.editor.scene.hierarchy)) {
          globalStore.editor.scene.hierarchy[0].children.push(newFolder);
        }
      } else {
        globalStore.editor.scene.hierarchy[0].children.push(newFolder);
      }
      
      globalStore.editor.scene.hierarchy = [...globalStore.editor.scene.hierarchy];
      
      console.log(`✅ Store - Created folder: ${folderName}`);
      return folderId;
    },

    moveObjectToFolder: (objectId, folderId) => {
      console.log(`📁 Store - Moving object ${objectId} to folder ${folderId}`);
      
      let movedItem = null;
      
      const findAndRemoveObject = (nodes) => {
        for (let i = 0; i < nodes.length; i++) {
          const node = nodes[i];
          if (node.id === objectId) {
            movedItem = nodes.splice(i, 1)[0];
            return true;
          }
          if (node.children && findAndRemoveObject(node.children)) {
            return true;
          }
        }
        return false;
      };
      
      const findAndAddToFolder = (nodes) => {
        for (let node of nodes) {
          if (node.id === folderId) {
            if (!node.children) node.children = [];
            node.children.push(movedItem);
            return true;
          }
          if (node.children && findAndAddToFolder(node.children)) {
            return true;
          }
        }
        return false;
      };
      
      if (findAndRemoveObject(globalStore.editor.scene.hierarchy) && movedItem) {
        if (findAndAddToFolder(globalStore.editor.scene.hierarchy)) {
          globalStore.editor.scene.hierarchy = [...globalStore.editor.scene.hierarchy];
          console.log(`✅ Store - Moved ${objectId} to folder ${folderId}`);
          return true;
        } else {
          console.error(`❌ Store - Could not find target folder ${folderId}`);
          return false;
        }
      } else {
        console.error(`❌ Store - Could not find object ${objectId} to move`);
        return false;
      }
    },

    renameObject: (objectId, newName) => {
      console.log(`✏️ Store - Renaming object: ${objectId} to ${newName}`);
      
      const findAndRename = (nodes) => {
        for (let node of nodes) {
          if (node.id === objectId) {
            node.name = newName;
            return true;
          }
          if (node.children && findAndRename(node.children)) {
            return true;
          }
        }
        return false;
      };
      
      const renamed = findAndRename(globalStore.editor.scene.hierarchy);
      
      if (renamed) {
        const scene = babylonScene?.current;
        if (scene) {
          const allObjects = [
            ...(scene.meshes || []),
            ...(scene.transformNodes || []),
            ...(scene.lights || []),
            ...(scene.cameras || [])
          ];
          
          const babylonObject = allObjects.find(obj => 
            (obj.uniqueId || obj.name) === objectId
          );
          
          if (babylonObject) {
            babylonObject.name = newName;
            console.log(`✅ Store - Renamed Babylon object: ${objectId} to ${newName}`);
          }
        }
        
        globalStore.editor.scene.hierarchy = [...globalStore.editor.scene.hierarchy];
        
        console.log(`✅ Store - Renamed object: ${objectId} to ${newName}`);
        return true;
      }
      
      console.warn(`❌ Store - Object not found for rename: ${objectId}`);
      return false;
    },

    reorderObjectInHierarchy: (draggedId, targetId, position) => {
      console.log(`🔄 Store - Reordering ${draggedId} ${position} ${targetId}`);
      
      try {
        const findObjectInHierarchy = (nodes, objectId) => {
          for (let i = 0; i < nodes.length; i++) {
            const node = nodes[i];
            if (node.id === objectId) {
              return { node, parent: nodes, index: i };
            }
            if (node.children) {
              const result = findObjectInHierarchy(node.children, objectId);
              if (result) return { ...result, parent: node.children };
            }
          }
          return null;
        };

        const draggedResult = findObjectInHierarchy(globalStore.editor.scene.hierarchy, draggedId);
        const targetResult = findObjectInHierarchy(globalStore.editor.scene.hierarchy, targetId);

        if (!draggedResult || !targetResult) {
          console.error('Could not find objects in hierarchy for reordering');
          return false;
        }

        if (position === 'inside') {
          if (targetResult.node.type !== 'folder') {
            console.warn('Cannot drop inside a non-folder object');
            return false;
          }
          return actions.editor.moveObjectToFolder(draggedId, targetId);
        }

        if (draggedResult.parent !== targetResult.parent) {
          console.log('Objects are at different hierarchy levels, moving to same parent first');
          
          const draggedNode = draggedResult.node;
          draggedResult.parent.splice(draggedResult.index, 1);
          const targetParent = targetResult.parent;
          const targetIndex = targetResult.index;
          
          let insertIndex = targetIndex;
          if (position === 'below') {
            insertIndex = targetIndex + 1;
          }
          
          targetParent.splice(insertIndex, 0, draggedNode);
        } else {
          const draggedNode = draggedResult.node;
          const parent = draggedResult.parent;
        
          parent.splice(draggedResult.index, 1);
          
          let newIndex = targetResult.index;
          if (draggedResult.index < targetResult.index) {
            newIndex--;
          }
          if (position === 'below') {
            newIndex++;
          }
          
          parent.splice(Math.max(0, newIndex), 0, draggedNode);
        }

        console.log(`✅ Store - Reordered ${draggedId} successfully`);
        
        globalStore.editor.scene.hierarchy = [...globalStore.editor.scene.hierarchy];
        
        return true;

      } catch (error) {
        console.error('Error reordering hierarchy:', error);
        return false;
      }
    },

    unpackMesh: (objectId) => {
      console.log('📦 Store - Unpacking mesh:', objectId);
      
      const scene = babylonScene?.current;
      if (!scene) return;
      
      const findLogicalContainers = (parentId, processedIds = new Set()) => {
        const containersToUnpack = [];
        const allObjects = [...(scene.meshes || []), ...(scene.transformNodes || [])];
        
        if (processedIds.has(parentId)) return containersToUnpack;
        processedIds.add(parentId);
        
        const parentObject = allObjects.find(obj => (obj.uniqueId || obj.name) === parentId);
        if (!parentObject) return containersToUnpack;
        
        const directChildren = allObjects.filter(obj => {
          const objParentId = obj.parent ? (obj.parent.uniqueId || obj.parent.name) : null;
          return objParentId === parentId;
        });
        
        console.log(`📦 Store - Found ${directChildren.length} direct children of ${parentId}`);
        
        directChildren.forEach(child => {
          const childId = child.uniqueId || child.name;
          const childMeshes = child.getChildMeshes ? child.getChildMeshes() : [];
          
          if (childMeshes.length > 0) {
            console.log(`📦 Store - Container ${childId} has ${childMeshes.length} child meshes`);
            
            const hasNestedContainers = directChildren.some(grandChild => {
              const grandChildParentId = grandChild.parent ? (grandChild.parent.uniqueId || grandChild.parent.name) : null;
              return grandChildParentId === childId && grandChild.getChildMeshes && grandChild.getChildMeshes().length > 0;
            });
            
            if (hasNestedContainers) {
              console.log(`📦 Store - Container ${childId} has nested containers - unpacking and recursing`);
              containersToUnpack.push(childId);
              const nestedContainers = findLogicalContainers(childId, processedIds);
              containersToUnpack.push(...nestedContainers);
            } else {
              console.log(`📦 Store - Container ${childId} has only final meshes - showing as logical group`);
              containersToUnpack.push(childId);
            }
          } else {
            console.log(`📦 Store - Object ${childId} has no child meshes - will not unpack`);
          }
        });
        
        return containersToUnpack;
      };
      
      const containersToUnpack = findLogicalContainers(objectId);
      console.log('📦 Store - Logical containers to unpack:', containersToUnpack);
      
      if (!globalStore.editor.scene.unpackedObjects.includes(objectId)) {
        globalStore.editor.scene.unpackedObjects.push(objectId);
      }
      
      containersToUnpack.forEach(id => {
        if (!globalStore.editor.scene.unpackedObjects.includes(id)) {
          globalStore.editor.scene.unpackedObjects.push(id);
        }
      });
      
      syncSceneToStore(babylonScene.current);
    },

    packMesh: (objectId) => {
      console.log('📦 Store - Packing mesh:', objectId);
      const scene = babylonScene?.current;
      if (!scene) return;
      
      const findRelatedContainers = (parentId) => {
        const containersToRemove = [parentId];
        const allObjects = [...(scene.meshes || []), ...(scene.transformNodes || [])];
    
        const directChildren = allObjects.filter(obj => {
          const objParentId = obj.parent ? (obj.parent.uniqueId || obj.parent.name) : null;
          return objParentId === parentId;
        });
        
        directChildren.forEach(child => {
          const childId = child.uniqueId || child.name;
          if (globalStore.editor.scene.unpackedObjects.includes(childId)) {
            containersToRemove.push(childId);
          }
        });
        
        return containersToRemove;
      };
      
      const containersToRemove = findRelatedContainers(objectId);
      console.log('📦 Store - Containers to pack:', containersToRemove);
      
      containersToRemove.forEach(id => {
        const index = globalStore.editor.scene.unpackedObjects.indexOf(id);
        if (index > -1) {
          globalStore.editor.scene.unpackedObjects.splice(index, 1);
        }
      });
      
      syncSceneToStore(babylonScene.current);
    },

    updateBabylonObjectFromProperties: (objectId) => {
      console.log('🔧 Store - updateBabylonObjectFromProperties called for:', objectId);
      
      const scene = babylonScene?.current;
      if (!scene) {
        console.warn('🔧 Store - No Babylon scene available');
        return;
      }

      const objectProps = globalStore.editor.objectProperties.objects[objectId];
      if (!objectProps) {
        console.warn('🔧 Store - No properties found for object:', objectId);
        return;
      }

      console.log('🔧 Store - Object properties to apply:', objectProps);

      const allObjects = [...(scene.meshes || []), ...(scene.transformNodes || []), ...(scene.lights || []), ...(scene.cameras || [])];
      const babylonObject = allObjects.find(obj => 
        (obj.uniqueId || obj.name) === objectId
      );

      if (!babylonObject) {
        console.warn('🔧 Store - Babylon object not found for ID:', objectId);
        console.log('🔧 Store - Available objects:', allObjects.map(obj => ({ id: obj.uniqueId || obj.name, name: obj.name, type: obj.getClassName() })));
        return;
      }

      console.log('🔧 Store - Found Babylon object to update:', {
        name: babylonObject.name,
        type: babylonObject.getClassName(),
        id: babylonObject.uniqueId || babylonObject.name
      });

      if (objectProps.transform) {
        console.log('🔧 Store - Updating transform properties:', objectProps.transform);
        
        if (objectProps.transform.position && babylonObject.position) {
          console.log('🔧 Store - Updating position from', [babylonObject.position.x, babylonObject.position.y, babylonObject.position.z], 'to', objectProps.transform.position);
          babylonObject.position.x = objectProps.transform.position[0];
          babylonObject.position.y = objectProps.transform.position[1];
          babylonObject.position.z = objectProps.transform.position[2];
        }
        if (objectProps.transform.rotation && babylonObject.rotation) {
          console.log('🔧 Store - Updating rotation from', [babylonObject.rotation.x, babylonObject.rotation.y, babylonObject.rotation.z], 'to', objectProps.transform.rotation);
          babylonObject.rotation.x = objectProps.transform.rotation[0];
          babylonObject.rotation.y = objectProps.transform.rotation[1];
          babylonObject.rotation.z = objectProps.transform.rotation[2];
        }
        if (objectProps.transform.scale && babylonObject.scaling) {
          console.log('🔧 Store - Updating scale from', [babylonObject.scaling.x, babylonObject.scaling.y, babylonObject.scaling.z], 'to', objectProps.transform.scale);
          babylonObject.scaling.x = objectProps.transform.scale[0];
          babylonObject.scaling.y = objectProps.transform.scale[1];
          babylonObject.scaling.z = objectProps.transform.scale[2];
        }
      }

      if (objectProps.material) {
        console.log('🎨 Store - Processing material properties:', objectProps.material);
        
        if (babylonObject.material) {
          console.log(`🎨 Store - Updating material for object "${babylonObject.name}" with material "${babylonObject.material.name}"`);

          if (objectProps.material.baseColor) {
            const hex = objectProps.material.baseColor.replace('#', '');
            const r = parseInt(hex.substr(0, 2), 16) / 255;
            const g = parseInt(hex.substr(2, 2), 16) / 255;
            const b = parseInt(hex.substr(4, 2), 16) / 255;
            
            console.log(`🎨 Store - Converting color ${objectProps.material.baseColor} to RGB:`, { r, g, b });
            
            if (babylonObject.material.albedoColor) {
              console.log(`🎨 Store - Applying to albedoColor (PBR material)`);
              babylonObject.material.albedoColor.r = r;
              babylonObject.material.albedoColor.g = g;
              babylonObject.material.albedoColor.b = b;
            } else if (babylonObject.material.diffuseColor) {
              console.log(`🎨 Store - Applying to diffuseColor (Standard material)`);
              babylonObject.material.diffuseColor.r = r;
              babylonObject.material.diffuseColor.g = g;
              babylonObject.material.diffuseColor.b = b;
            } else {
              console.log(`🎨 Store - Material type not recognized:`, babylonObject.material.getClassName());
            }
          }
          
          if (objectProps.material.roughness !== undefined && babylonObject.material.roughness !== undefined) {
            console.log(`🎨 Store - Updating roughness to:`, objectProps.material.roughness);
            babylonObject.material.roughness = objectProps.material.roughness;
          }
          
          if (objectProps.material.metallic !== undefined && babylonObject.material.metallic !== undefined) {
            console.log(`🎨 Store - Updating metallic to:`, objectProps.material.metallic);
            babylonObject.material.metallic = objectProps.material.metallic;
          }
        } else {
          console.log(`🎨 Store - Object "${babylonObject.name}" has no material to update`);
        }
      }
      
      console.log('✅ Store - updateBabylonObjectFromProperties completed successfully for:', objectId);
    },

    toggleHierarchyNode: (nodeId) => {
      const toggleNodeInHierarchy = (nodes) => {
        for (let node of nodes) {
          if (node.id === nodeId) {
            node.expanded = !node.expanded
            return true
          }
          if (node.children && toggleNodeInHierarchy(node.children)) {
            return true
          }
        }
        return false
      }
      
      toggleNodeInHierarchy(globalStore.editor.scene.hierarchy)
    },

    setAssetsProject: (projectName) => {
      if (globalStore.editor.assets.currentProject !== projectName) {
        globalStore.editor.assets.currentProject = projectName
        globalStore.editor.assets.folderTree = null
        globalStore.editor.assets.folderTreeTimestamp = null
        globalStore.editor.assets.categories = null
        globalStore.editor.assets.categoriesTimestamp = null
        globalStore.editor.assets.assetsByPath = {}
      }
    },

    isCacheValid: (timestamp) => {
      if (!timestamp) return false
      return (Date.now() - timestamp) < globalStore.editor.assets.cacheExpiryMs
    },

    setFolderTree: (tree) => {
      globalStore.editor.assets.folderTree = tree
      globalStore.editor.assets.folderTreeTimestamp = Date.now()
    },

    setAssetCategories: (categories) => {
      globalStore.editor.assets.categories = categories
      globalStore.editor.assets.categoriesTimestamp = Date.now()
    },

    setAssetsForPath: (path, assets) => {
      globalStore.editor.assets.assetsByPath[path] = {
        assets: assets,
        timestamp: Date.now()
      }
    },

    getAssetsForPath: (path) => {
      const cached = globalStore.editor.assets.assetsByPath[path]
      if (cached && actions.editor.isCacheValid(cached.timestamp)) {
        return cached.assets
      }
      return null
    },

    invalidateAssetPath: (path) => {
      delete globalStore.editor.assets.assetsByPath[path]
    },

    invalidateAssetPaths: (paths) => {
      paths.forEach(path => {
        delete globalStore.editor.assets.assetsByPath[path]
      })
    },

    invalidateFolderTree: () => {
      globalStore.editor.assets.folderTree = null
      globalStore.editor.assets.folderTreeTimestamp = null
    },

    invalidateCategories: () => {
      globalStore.editor.assets.categories = null
      globalStore.editor.assets.categoriesTimestamp = null
    },

    clearAllAssetCache: () => {
      globalStore.editor.assets.folderTree = null
      globalStore.editor.assets.folderTreeTimestamp = null
      globalStore.editor.assets.categories = null
      globalStore.editor.assets.categoriesTimestamp = null
      globalStore.editor.assets.assetsByPath = {}
    }
  }
}

if (typeof window !== 'undefined') {
  try {
    devtools(globalStore, {
      name: 'Global Store',
      enabled: true
    })
    console.log('✅ Valtio devtools enabled')
    
    window.globalStore = globalStore
    window.actions = actions
    
  } catch (error) {
    console.error('Failed to setup devtools:', error)
  }
}