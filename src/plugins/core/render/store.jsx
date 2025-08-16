import { createStore } from 'solid-js/store'
import { Color3 } from '@babylonjs/core/Maths/math.color'

export let babylonScene = { current: null }

const [sceneStore, setSceneStore] = createStore({
  isLoaded: false,
  name: 'Untitled Scene',
  selectedObjectId: null,
  unpackedObjects: [],
  
  objects: {
    meshes: [],
    transformNodes: [],
    lights: [],
    cameras: []
  },
  
  hierarchy: [],
  
  grid: {
    enabled: true,
    size: 20,
    cellSize: 1,
    unit: 'meters',
    position: [0, 0, 0],
    cellColor: '#555555',
    sectionColor: '#888888',
    sectionSize: 10,
    infiniteGrid: false
  },
  
  scene: {
    worldUnit: 'meters',
    floorSize: 20
  }
})

const syncSceneToStore = (scene) => {
  console.log('🔄 syncSceneToStore - Starting scene sync')
  if (!scene) {
    console.log('❌ syncSceneToStore - No scene provided')
    setSceneStore('isLoaded', false)
    setSceneStore('objects', 'meshes', [])
    setSceneStore('objects', 'transformNodes', [])
    setSceneStore('objects', 'lights', [])
    setSceneStore('objects', 'cameras', [])
    setSceneStore('hierarchy', [])
    return
  }

  console.log('🔍 syncSceneToStore - Scene analysis:', {
    totalMeshes: scene.meshes?.length || 0,
    totalTransformNodes: scene.transformNodes?.length || 0,
    totalLights: scene.lights?.length || 0,
    totalCameras: scene.cameras?.length || 0
  })

  const allMeshes = scene.meshes || []
  const meshes = allMeshes
    .filter(mesh => {
      const isInternal = mesh._isInternalMesh
      
      const isSystemMesh = mesh.name && (
        mesh.name.startsWith('__') && mesh.name !== '__root__' ||
        mesh.name.includes('gizmo') ||
        mesh.name.includes('helper') ||
        mesh.name.includes('_internal_')
      )
      
      if (isSystemMesh) {
        console.log(`🚫 syncSceneToStore - Filtering out system mesh: ${mesh.name}`)
        return false
      }
      
      if (isInternal) {
        const parentId = mesh.parent ? (mesh.parent.uniqueId || mesh.parent.name) : null
        const isUnpacked = parentId && sceneStore.unpackedObjects?.includes(parentId)
        
        if (isUnpacked) {
          console.log(`✅ syncSceneToStore - Including unpacked internal mesh: ${mesh.name} (parent: ${parentId})`)
          return true
        } else {
          console.log(`🚫 syncSceneToStore - Hiding internal mesh (parent not unpacked): ${mesh.name} (parent: ${parentId})`)
          return false
        }
      }
      
      return true
    })
    .map(mesh => {
      const objectId = mesh.uniqueId || mesh.name || `mesh-${Math.random()}`
      
      sceneActions.ensureDefaultComponents(objectId)
      
      return {
        id: objectId,
        name: mesh.name || 'Unnamed Mesh',
        type: 'mesh',
        visible: mesh.isVisible !== undefined ? mesh.isVisible : true,
        position: mesh.position ? [mesh.position.x, mesh.position.y, mesh.position.z] : [0, 0, 0],
        parentId: mesh.parent ? (mesh.parent.uniqueId || mesh.parent.name) : null,
        materialName: mesh.material ? mesh.material.name : null,
        materialType: mesh.material ? mesh.material.getClassName() : null
      }
    })

  const allTransformNodes = scene.transformNodes || []
  const transformNodes = allTransformNodes
    .filter(node => {
      const isInternal = node._isInternalNode
      
      const isSystemNode = node.name && (
        node.name.startsWith('__') ||
        node.name.includes('gizmo') ||
        node.name.includes('helper') ||
        node.name.includes('_internal_')
      )
      
      if (isSystemNode) {
        console.log(`🚫 syncSceneToStore - Filtering out system transform node: ${node.name}`)
        return false
      }
      
      if (isInternal) {
        const parentId = node.parent ? (node.parent.uniqueId || node.parent.name) : null
        const isUnpacked = parentId && sceneStore.unpackedObjects?.includes(parentId)
        
        if (isUnpacked) {
          console.log(`✅ syncSceneToStore - Including unpacked internal transform node: ${node.name} (parent: ${parentId})`)
          return true
        } else {
          console.log(`🚫 syncSceneToStore - Hiding internal transform node (parent not unpacked): ${node.name} (parent: ${parentId})`)
          return false
        }
      }
      
      return true
    })
    .map(node => {
      const objectId = node.uniqueId || node.name || `transform-${Math.random()}`
      sceneActions.ensureDefaultComponents(objectId)
      
      return {
        id: objectId,
        name: node.name || 'Unnamed Transform',
        type: 'mesh',
        visible: true,
        position: node.position ? [node.position.x, node.position.y, node.position.z] : [0, 0, 0],
        parentId: node.parent ? (node.parent.uniqueId || node.parent.name) : null
      }
    })

  const lights = (scene.lights || []).map(light => {
    const objectId = light.uniqueId || light.name || `light-${Math.random()}`
    sceneActions.ensureDefaultComponents(objectId)
    
    return {
      id: objectId,
      name: light.name || 'Unnamed Light',
      type: 'light',
      visible: light.isEnabled !== undefined ? light.isEnabled() : true,
      intensity: light.intensity !== undefined ? light.intensity : 1,
      parentId: light.parent ? (light.parent.uniqueId || light.parent.name) : null
    }
  })

  const cameras = (scene.cameras || []).map(camera => {
    const objectId = camera.uniqueId || camera.name || `camera-${Math.random()}`
    sceneActions.ensureDefaultComponents(objectId)
    
    return {
      id: objectId,
      name: camera.name || 'Unnamed Camera',
      type: 'camera',
      visible: true,
      active: scene.activeCamera === camera,
      parentId: camera.parent ? (camera.parent.uniqueId || camera.parent.name) : null
    }
  })

  const allObjects = [...meshes, ...transformNodes, ...lights, ...cameras]
  
  const buildHierarchyNode = (obj) => {
    const children = allObjects
      .filter(child => child.parentId === obj.id)
      .map(child => buildHierarchyNode(child))
    
    const scene = babylonScene?.current
    let hasChildMeshes = false
    if (scene) {
      const babylonObject = [...(scene.meshes || []), ...(scene.transformNodes || [])].find(bObj => 
        (bObj.uniqueId || bObj.name) === obj.id
      )
      if (babylonObject && babylonObject.getChildMeshes) {
        const childMeshes = babylonObject.getChildMeshes()
        hasChildMeshes = childMeshes.length > 0
      }
    }
    
    const isUnpacked = sceneStore.unpackedObjects.includes(obj.id)
    const hasChildren = children.length > 0
    const isModelContainer = hasChildren && children.length > 10
    
    return {
      id: obj.id,
      name: obj.name,
      type: obj.type,
      visible: obj.visible,
      children: hasChildren ? children : undefined,
      expanded: isModelContainer || isUnpacked,
      hasChildMeshes,
      isUnpacked
    }
  }
  
  const rootObjects = allObjects.filter(obj => !obj.parentId)
  
  const hierarchy = [
    {
      id: 'scene-root',
      name: sceneStore.name,
      type: 'scene',
      expanded: true,
      children: rootObjects.map(obj => buildHierarchyNode(obj))
    }
  ]

  console.log('📝 syncSceneToStore - Final results:', {
    totalMeshes: meshes.length,
    totalTransformNodes: transformNodes.length,
    totalLights: lights.length,
    totalCameras: cameras.length,
    hierarchyRoots: hierarchy.length,
    hierarchyChildren: hierarchy[0]?.children?.length || 0
  })
  
  setSceneStore('isLoaded', true)
  setSceneStore('objects', 'meshes', meshes)
  setSceneStore('objects', 'transformNodes', transformNodes)
  setSceneStore('objects', 'lights', lights)
  setSceneStore('objects', 'cameras', cameras)
  setSceneStore('hierarchy', hierarchy)
}

export const sceneActions = {
  setScene: (scene) => {
    babylonScene.current = scene
    syncSceneToStore(scene)
    console.log('Babylon.js scene updated:', scene ? 'loaded' : 'cleared')
  },
  
  updateScene: (scene) => {
    babylonScene.current = scene
    syncSceneToStore(scene)
  },
  
  updateBabylonScene: (scene) => {
    babylonScene.current = scene
    if (scene) {
      syncSceneToStore(scene)
    }
  },
  
  selectObject: async (objectId) => {
    console.log('🏪 Scene Store - selectObject called with ID:', objectId)
    
    const scene = babylonScene?.current
    if (!scene) {
      console.warn('🏪 Scene Store - No Babylon scene available for selection')
      return
    }

    if (objectId) {
      const allObjects = [...(scene.meshes || []), ...(scene.transformNodes || []), ...(scene.lights || []), ...(scene.cameras || [])]
      const babylonObject = allObjects.find(obj => 
        (obj.uniqueId || obj.name) === objectId
      )

      if (babylonObject) {
        console.log('🏪 Scene Store - Found Babylon object for selection:', babylonObject.name)
        
        if (scene._highlightLayer) {
          console.log('🎨 Scene Store - Clearing previous highlights')
          scene._highlightLayer.removeAllMeshes()
        }
        
        if (scene._gizmoManager) {
          scene._gizmoManager.attachToMesh(babylonObject)
          
          if (scene._ensureGizmoThickness) {
            scene._ensureGizmoThickness()
          }
        }
      
        if (scene._highlightLayer) {
          console.log('🎨 Scene Store - Adding highlight to object:', babylonObject.getClassName(), babylonObject.name)
          try {
            // Use imported Color3
            
            if (babylonObject.getClassName() === 'TransformNode') {
              const childMeshes = babylonObject.getChildMeshes()
              console.log('🎨 Scene Store - TransformNode child meshes to highlight:', childMeshes.length)
              childMeshes.forEach((childMesh, index) => {
                if (childMesh.getClassName() === 'Mesh') {
                  console.log(`🎨 Scene Store - Highlighting child mesh ${index}:`, childMesh.name)
                  scene._highlightLayer.addMesh(childMesh, Color3.Yellow())
                }
              })
            } else if (babylonObject.getClassName() === 'Mesh') {
              console.log('🎨 Scene Store - Highlighting direct mesh:', babylonObject.name)
              scene._highlightLayer.addMesh(babylonObject, Color3.Yellow())
            }
            console.log('✅ Scene Store - Highlighting completed')
          } catch (highlightError) {
            console.error('❌ Scene Store - Could not add highlight to selected object:', highlightError)
          }
        } else {
          console.warn('🎨 Scene Store - No highlight layer available')
        }
        
        setSceneStore('selectedObjectId', objectId)
        
        console.log('✅ Scene Store - Object selection completed:', babylonObject.name, 'ID:', objectId)
      } else {
        console.warn('🏪 Scene Store - Could not find Babylon.js object with ID:', objectId)
        setSceneStore('selectedObjectId', objectId)
      }
    } else {
      console.log('🎨 Scene Store - Clearing selection and highlights')
      if (scene._gizmoManager) {
        scene._gizmoManager.attachToMesh(null)
      }
      if (scene._highlightLayer) {
        console.log('🎨 Scene Store - Removing all mesh highlights')
        scene._highlightLayer.removeAllMeshes()
      }
      setSceneStore('selectedObjectId', null)
      
      console.log('✅ Scene Store - Selection cleared')
    }
  },
  
  selectSceneObject: (objectId) => {
    console.log('🏪 Scene Store - selectSceneObject called:', {
      'old value': sceneStore.selectedObjectId,
      'new value': objectId
    })
    setSceneStore('selectedObjectId', objectId)
  },
  
  refreshSceneData: () => {
    console.log('🔄 Scene Store - Refreshing scene data manually triggered')
    syncSceneToStore(babylonScene.current)
    console.log('✅ Scene Store - Scene data refresh completed')
  },

  toggleHierarchyNode: (nodeId) => {
    const toggleNodeInHierarchy = (nodes, path = []) => {
      for (let i = 0; i < nodes.length; i++) {
        const node = nodes[i]
        if (node.id === nodeId) {
          setSceneStore('hierarchy', ...path, i, 'expanded', !node.expanded)
          return true
        }
        if (node.children && toggleNodeInHierarchy(node.children, [...path, i, 'children'])) {
          return true
        }
      }
      return false
    }
    
    toggleNodeInHierarchy(sceneStore.hierarchy)
  },
  
  updateSceneMetadata: (metadata) => {
    setSceneStore(metadata)
  },
  
  setSceneName: (name) => {
    setSceneStore('name', name)
  },
  
  updateGridSettings: (settings) => {
    setSceneStore('grid', settings)
  },

  ensureDefaultComponents: (objectId) => {
    console.log(`✅ ensureDefaultComponents - Placeholder for object: ${objectId}`)
  }
}

export { sceneStore }

if (typeof window !== 'undefined') {
  window.sceneStore = sceneStore
  window.sceneActions = sceneActions
}