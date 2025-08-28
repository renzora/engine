import { createStore } from 'solid-js/store';
import { Color4, Color3 } from '@babylonjs/core/Maths/math.color';
import { initializeScriptRuntime } from '@/api/script';

// Render Store for managing Babylon.js state
export const [renderStore, setRenderStore] = createStore({
  engine: null,
  scene: null,
  camera: null,
  selectedObject: null,
  gizmoManager: null,
  highlightLayer: null,
  transformMode: 'select', // 'select', 'move', 'rotate', 'scale'
  isGizmoDragging: false, // Track when gizmo is being dragged
  isInitialized: false,
  hierarchy: [], // Scene hierarchy tree for UI
  settings: {
    backgroundColor: '#1a202c',
    enableGrid: true,
    gridSize: 10,
    renderingEngine: 'webgl'
  }
});

// Actions for the render store
export const renderActions = {
  setEngine(engine) {
    setRenderStore('engine', engine);
    setRenderStore('isInitialized', !!engine);
  },

  setScene(scene) {
    setRenderStore('scene', scene);
    // Initialize hierarchy when scene is set
    if (scene) {
      this.initializeHierarchy();
      // Initialize script runtime
      initializeScriptRuntime(scene);
    }
  },

  setCamera(camera) {
    setRenderStore('camera', camera);
  },

  setGizmoManager(gizmoManager) {
    setRenderStore('gizmoManager', gizmoManager);
  },

  setHighlightLayer(highlightLayer) {
    setRenderStore('highlightLayer', highlightLayer);
  },

  setTransformMode(mode) {
    setRenderStore('transformMode', mode);
    
    // Update gizmos based on transform mode
    const gizmoManager = renderStore.gizmoManager;
    if (gizmoManager) {
      try {
        // Disable all gizmos first
        gizmoManager.positionGizmoEnabled = false;
        gizmoManager.rotationGizmoEnabled = false;
        gizmoManager.scaleGizmoEnabled = false;
        
        // Enable the appropriate gizmo
        switch (mode) {
          case 'move':
            gizmoManager.positionGizmoEnabled = true;
            break;
          case 'rotate':
            gizmoManager.rotationGizmoEnabled = true;
            break;
          case 'scale':
            gizmoManager.scaleGizmoEnabled = true;
            break;
          case 'select':
          default:
            // No gizmos enabled for select mode
            break;
        }
      } catch (e) {
        // Custom gizmo handles this internally
      }
      
      // Apply gizmo improvements
      renderActions.ensureGizmoThickness();
      
      // Ensure callbacks are attached when switching gizmo modes
      renderActions.attachGizmoCallbacks(gizmoManager);
    }
  },

  setGizmoDragging(isDragging) {
    console.log('🎯 setGizmoDragging called:', isDragging);
    setRenderStore('isGizmoDragging', isDragging);
  },

  ensureGizmoThickness() {
    const gizmoManager = renderStore.gizmoManager;
    if (gizmoManager && gizmoManager.gizmos.positionGizmo) {
      // Use more precise control for Unreal-style gizmos
      gizmoManager.gizmos.positionGizmo.xGizmo.dragBehavior.dragDeltaRatio = 0.2;
      gizmoManager.gizmos.positionGizmo.yGizmo.dragBehavior.dragDeltaRatio = 0.2;
      gizmoManager.gizmos.positionGizmo.zGizmo.dragBehavior.dragDeltaRatio = 0.2;
    }
  },

  attachGizmoCallbacks(gizmoManager) {
    // Attach drag start/end callbacks to prevent camera movement during gizmo drag
    if (gizmoManager && gizmoManager.gizmos) {
      // Position gizmo callbacks
      if (gizmoManager.gizmos.positionGizmo) {
        const posGizmo = gizmoManager.gizmos.positionGizmo;
        posGizmo.onDragStartObservable.clear(); // Clear existing callbacks
        posGizmo.onDragEndObservable.clear();
        
        posGizmo.onDragStartObservable.add(() => {
          console.log('🔧 Position gizmo drag started');
          this.setGizmoDragging(true);
        });
        
        posGizmo.onDragEndObservable.add(() => {
          console.log('🔧 Position gizmo drag ended');
          this.setGizmoDragging(false);
        });
      }
      
      // Rotation gizmo callbacks
      if (gizmoManager.gizmos.rotationGizmo) {
        const rotGizmo = gizmoManager.gizmos.rotationGizmo;
        rotGizmo.onDragStartObservable.clear(); // Clear existing callbacks
        rotGizmo.onDragEndObservable.clear();
        
        rotGizmo.onDragStartObservable.add(() => {
          console.log('🔧 Rotation gizmo drag started');
          this.setGizmoDragging(true);
        });
        
        rotGizmo.onDragEndObservable.add(() => {
          console.log('🔧 Rotation gizmo drag ended');
          this.setGizmoDragging(false);
        });
      }
      
      // Scale gizmo callbacks
      if (gizmoManager.gizmos.scaleGizmo) {
        const scaleGizmo = gizmoManager.gizmos.scaleGizmo;
        scaleGizmo.onDragStartObservable.clear(); // Clear existing callbacks
        scaleGizmo.onDragEndObservable.clear();
        
        scaleGizmo.onDragStartObservable.add(() => {
          console.log('🔧 Scale gizmo drag started');
          this.setGizmoDragging(true);
        });
        
        scaleGizmo.onDragEndObservable.add(() => {
          console.log('🔧 Scale gizmo drag ended');
          this.setGizmoDragging(false);
        });
      }
    }
  },

  selectObject(object) {
    setRenderStore('selectedObject', object);
    
    // Handle gizmo attachment
    const gizmoManager = renderStore.gizmoManager;
    const highlightLayer = renderStore.highlightLayer;
    
    if (gizmoManager && highlightLayer) {
      // Clear previous selection
      highlightLayer.removeAllMeshes();
      
      if (object) {
        // Attach gizmo to selected object
        gizmoManager.attachToMesh(object);
        
        // Automatically show move gizmo when object is selected
        try {
          gizmoManager.positionGizmoEnabled = true;
          gizmoManager.rotationGizmoEnabled = false;
          gizmoManager.scaleGizmoEnabled = false;
        } catch (e) {
          // Custom gizmo handles this internally
        }
        setRenderStore('transformMode', 'move');
        
        // Add drag callbacks to the existing gizmo manager
        this.attachGizmoCallbacks(gizmoManager);
        
        // Add highlight to selected object
        try {
          if (object.getChildMeshes) {
            const childMeshes = object.getChildMeshes();
            childMeshes.forEach(childMesh => {
              if (childMesh.getClassName() === 'Mesh') {
                highlightLayer.addMesh(childMesh, Color3.Yellow());
              }
            });
          } else {
            highlightLayer.addMesh(object, Color3.Yellow());
          }
        } catch (error) {
          console.warn('Could not add highlight to object:', error);
        }
      } else {
        // No object selected, detach gizmo and hide all gizmos
        gizmoManager.attachToMesh(null);
        try {
          gizmoManager.positionGizmoEnabled = false;
          gizmoManager.rotationGizmoEnabled = false;
          gizmoManager.scaleGizmoEnabled = false;
        } catch (e) {
          // Custom gizmo handles this internally
        }
        setRenderStore('transformMode', 'select');
      }
    }
    
    // Dispatch selection event
    if (typeof window !== 'undefined') {
      const event = new CustomEvent('babylonObjectSelected', {
        detail: { object, scene: renderStore.scene }
      });
      window.dispatchEvent(event);
    }
  },

  updateSettings(newSettings) {
    setRenderStore('settings', (prev) => ({ ...prev, ...newSettings }));
    
    // Apply settings to scene if available
    if (renderStore.scene && newSettings.backgroundColor) {
      const color = newSettings.backgroundColor;
      if (color !== 'theme') {
        const hex = color.replace('#', '');
        const r = parseInt(hex.substr(0, 2), 16) / 255;
        const g = parseInt(hex.substr(2, 2), 16) / 255;
        const b = parseInt(hex.substr(4, 2), 16) / 255;
        
        renderStore.scene.clearColor = new Color4(r, g, b, 1);
      }
    }
  },

  addObject(mesh) {
    if (!renderStore.scene || !mesh) return;
    
    // Ensure mesh is in the scene
    if (mesh.getScene() !== renderStore.scene) {
      mesh.setParent(null);
      mesh._scene = renderStore.scene;
    }
    
    // Update hierarchy directly
    this.addObjectToHierarchy(mesh);
    
    console.log('➕ Object added to scene:', mesh.name);
    
    // Dispatch scene change event
    if (typeof window !== 'undefined') {
      const event = new CustomEvent('babylonSceneChanged', {
        detail: { type: 'objectAdded', object: mesh }
      });
      window.dispatchEvent(event);
    }
  },

  removeObject(mesh) {
    if (!mesh) return;
    
    const objectId = mesh.uniqueId || mesh.name;
    
    if (renderStore.selectedObject === mesh) {
      this.selectObject(null);
    }
    
    // Update hierarchy before disposal
    this.removeObjectFromHierarchy(objectId);
    
    mesh.dispose();
    
    console.log('➖ Object removed from scene:', mesh.name);
    
    // Dispatch scene change event
    if (typeof window !== 'undefined') {
      const event = new CustomEvent('babylonSceneChanged', {
        detail: { type: 'objectRemoved', object: mesh }
      });
      window.dispatchEvent(event);
    }
  },

  // Utility methods
  getScene() {
    return renderStore.scene;
  },

  getEngine() {
    return renderStore.engine;
  },

  getCamera() {
    return renderStore.camera;
  },

  getSelectedObject() {
    return renderStore.selectedObject;
  },

  isReady() {
    return renderStore.isInitialized && renderStore.scene && renderStore.engine;
  },

  // Hierarchy management functions
  buildHierarchyFromBabylon(babylonObject, depth = 0) {
    if (!babylonObject) return null;
    
    const objectId = babylonObject.uniqueId || babylonObject.name || `${babylonObject.getClassName()}-${Math.random()}`;
    
    let type = 'mesh';
    let lightType = null;
    
    const className = babylonObject.getClassName();
    if (className.includes('Light')) {
      type = 'light';
      lightType = className.toLowerCase().replace('light', '');
    } else if (className.includes('Camera')) {
      type = 'camera';
    } else if (className === 'TransformNode') {
      // Check if this is an imported asset container (has mesh children)
      const hasMeshChildren = babylonObject.getChildren && 
        babylonObject.getChildren().some(child => 
          child.getClassName && (
            child.getClassName().includes('Mesh') || 
            child.getClassName().includes('InstancedMesh')
          )
        );
      type = hasMeshChildren ? 'mesh' : 'folder';
    }
    
    const children = [];
    if (babylonObject.getChildren) {
      const babylonChildren = babylonObject.getChildren();
      babylonChildren.forEach(child => {
        if (child.name && !child.name.startsWith('__') && !child.name.includes('gizmo')) {
          children.push(this.buildHierarchyFromBabylon(child, depth + 1));
        }
      });
    }
    
    return {
      id: objectId,
      name: babylonObject.name || `Unnamed ${className}`,
      type: type,
      lightType: lightType,
      visible: babylonObject.isVisible !== undefined ? babylonObject.isVisible : 
               (babylonObject.isEnabled ? babylonObject.isEnabled() : true),
      children: children.length > 0 ? children : undefined,
      expanded: depth < 2,
      babylonObject: babylonObject
    };
  },

  initializeHierarchy() {
    const scene = renderStore.scene;
    if (!scene) {
      setRenderStore('hierarchy', []);
      return;
    }
    
    const allObjects = [
      ...(scene.meshes || []),
      ...(scene.transformNodes || []),
      ...(scene.lights || []),
      ...(scene.cameras || [])
    ];
    
    const rootObjects = allObjects.filter(obj => {
      const isSystemObject = obj.name && (
        obj.name.startsWith('__') ||
        obj.name.includes('gizmo') ||
        obj.name.includes('helper') ||
        obj.name.includes('_internal_')
      );
      
      return !isSystemObject && !obj.parent;
    });
    
    const hierarchyItems = rootObjects.map(obj => this.buildHierarchyFromBabylon(obj));
    
    const hierarchy = [{
      id: 'scene-root',
      name: 'Clean Scene',
      type: 'scene',
      expanded: true,
      children: hierarchyItems
    }];
    
    setRenderStore('hierarchy', hierarchy);
    console.log('🌳 Scene Tree: Hierarchy initialized with', hierarchyItems.length, 'root objects');
  },

  addObjectToHierarchy(babylonObject) {
    const newItem = this.buildHierarchyFromBabylon(babylonObject);
    if (!newItem) return;

    setRenderStore('hierarchy', prev => {
      const findAndAddToParent = (nodes, parentId, item) => {
        return nodes.map(node => {
          if (node.id === parentId) {
            return {
              ...node,
              children: [...(node.children || []), item]
            };
          } else if (node.children) {
            return {
              ...node,
              children: findAndAddToParent(node.children, parentId, item)
            };
          }
          return node;
        });
      };

      if (babylonObject.parent) {
        const parentId = babylonObject.parent.uniqueId || babylonObject.parent.name;
        return findAndAddToParent(prev, parentId, newItem);
      } else {
        // Add to scene root
        return prev.map(node => {
          if (node.id === 'scene-root') {
            return {
              ...node,
              children: [...(node.children || []), newItem]
            };
          }
          return node;
        });
      }
    });

    console.log('🌳 Scene Tree: Added object to hierarchy:', newItem.name);
  },

  removeObjectFromHierarchy(objectId) {
    setRenderStore('hierarchy', prev => {
      const removeFromNodes = (nodes) => {
        return nodes.map(node => ({
          ...node,
          children: node.children ? removeFromNodes(node.children).filter(child => child.id !== objectId) : undefined
        })).filter(node => node.id !== objectId);
      };
      return removeFromNodes(prev);
    });

    console.log('🌳 Scene Tree: Removed object from hierarchy:', objectId);
  },

  cleanup() {
    // Dispose of gizmo manager and highlight layer
    if (renderStore.gizmoManager) {
      renderStore.gizmoManager.dispose();
      setRenderStore('gizmoManager', null);
    }
    
    if (renderStore.highlightLayer) {
      renderStore.highlightLayer.dispose();
      setRenderStore('highlightLayer', null);
    }

    // Clear other references
    setRenderStore('selectedObject', null);
    setRenderStore('transformMode', 'select');
    setRenderStore('hierarchy', []);
  }
};