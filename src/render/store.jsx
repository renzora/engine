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
  },
  
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
        
        // Enable the appropriate gizmo only if an object is selected
        if (renderStore.selectedObject && mode !== 'select') {
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
          }
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
    // Track gizmo drag state to prevent camera movement during transformation
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
          // Position gizmo drag started
          this.setGizmoDragging(true);
        });
        
        posGizmo.onDragEndObservable.add(() => {
          // Position gizmo drag ended
          this.setGizmoDragging(false);
          
          // Mark transform as manually changed for physics sync
          if (renderStore.selectedObject) {
            renderStore.selectedObject._manualTransformChange = true;
          }
          
          // Mark scene as modified
          import('@/api/scene/SceneManager.js').then(({ sceneManager }) => {
            sceneManager.markAsModified();
          }).catch(err => {
            console.error('❌ Failed to mark scene as modified:', err);
          });
        });
      }
      
      // Rotation gizmo callbacks
      if (gizmoManager.gizmos.rotationGizmo) {
        const rotGizmo = gizmoManager.gizmos.rotationGizmo;
        rotGizmo.onDragStartObservable.clear(); // Clear existing callbacks
        rotGizmo.onDragEndObservable.clear();
        
        rotGizmo.onDragStartObservable.add(() => {
          // Rotation gizmo drag started
          this.setGizmoDragging(true);
        });
        
        rotGizmo.onDragEndObservable.add(() => {
          // Rotation gizmo drag ended
          this.setGizmoDragging(false);
          
          // Mark transform as manually changed for physics sync
          if (renderStore.selectedObject) {
            renderStore.selectedObject._manualTransformChange = true;
          }
          
          // Mark scene as modified
          import('@/api/scene/SceneManager.js').then(({ sceneManager }) => {
            sceneManager.markAsModified();
          }).catch(err => {
            console.error('❌ Failed to mark scene as modified:', err);
          });
        });
      }
      
      // Scale gizmo callbacks
      if (gizmoManager.gizmos.scaleGizmo) {
        const scaleGizmo = gizmoManager.gizmos.scaleGizmo;
        scaleGizmo.onDragStartObservable.clear(); // Clear existing callbacks
        scaleGizmo.onDragEndObservable.clear();
        
        scaleGizmo.onDragStartObservable.add(() => {
          // Scale gizmo drag started
          this.setGizmoDragging(true);
        });
        
        scaleGizmo.onDragEndObservable.add(() => {
          // Scale gizmo drag ended
          this.setGizmoDragging(false);
          
          // Mark scene as modified
          import('@/api/scene/SceneManager.js').then(({ sceneManager }) => {
            sceneManager.markAsModified();
          }).catch(err => {
            console.error('❌ Failed to mark scene as modified:', err);
          });
        });
      }
    }
  },

  selectObject(object) {
    // Update selected object in render store
    setRenderStore('selectedObject', object);
    
    // Also update editor store selection to keep UI in sync
    if (object) {
      const entityId = object.uniqueId || object.name;
      // Import editorActions dynamically to avoid circular imports
      import('@/layout/stores/EditorStore').then(({ editorActions }) => {
        editorActions.selectEntity(entityId);
      });
    } else {
      // Clear editor selection
      import('@/layout/stores/EditorStore').then(({ editorActions }) => {
        editorActions.selectEntity(null);
      });
    }
    
    // Handle gizmo attachment
    const gizmoManager = renderStore.gizmoManager;
    const highlightLayer = renderStore.highlightLayer;
    
    if (gizmoManager && highlightLayer) {
      // Clear previous selection
      highlightLayer.removeAllMeshes();
      
      if (object) {
        // Attach gizmo to selected object
        gizmoManager.attachToMesh(object);
        
        // Only show gizmo if not in select mode
        const currentMode = renderStore.transformMode;
        if (currentMode !== 'select') {
          try {
            gizmoManager.positionGizmoEnabled = currentMode === 'move';
            gizmoManager.rotationGizmoEnabled = currentMode === 'rotate';
            gizmoManager.scaleGizmoEnabled = currentMode === 'scale';
          } catch (e) {
            // Custom gizmo handles this internally
          }
        } else {
          // In select mode, hide all gizmos
          try {
            gizmoManager.positionGizmoEnabled = false;
            gizmoManager.rotationGizmoEnabled = false;
            gizmoManager.scaleGizmoEnabled = false;
          } catch (e) {
            // Custom gizmo handles this internally
          }
        }
        
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
    
    // Object added to scene
    
    // Mark scene as modified
    import('@/api/scene/SceneManager.js').then(({ sceneManager }) => {
      sceneManager.markAsModified();
    }).catch(err => {
      console.error('❌ Failed to mark scene as modified:', err);
    });
    
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
    
    // Object removed from scene
    
    // Mark scene as modified
    import('@/api/scene/SceneManager.js').then(({ sceneManager }) => {
      sceneManager.markAsModified();
    }).catch(err => {
      console.error('❌ Failed to mark scene as modified:', err);
    });
    
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

  // Select object by ID (used by scene tree)
  selectObjectById(objectId) {
    // Find and select object by ID from scene hierarchy
    if (!renderStore.scene) return false;
    
    // Find the Babylon object by ID in the hierarchy
    const findObjectById = (hierarchyItems) => {
      for (const item of hierarchyItems) {
        if (item.id === objectId && item.babylonObject) {
          return item.babylonObject;
        }
        if (item.children) {
          const found = findObjectById(item.children);
          if (found) return found;
        }
      }
      return null;
    };
    
    const babylonObject = findObjectById(renderStore.hierarchy);
    if (babylonObject) {
      this.selectObject(babylonObject);
      return true;
    }
    
    console.warn(`Could not find Babylon object for ID: ${objectId}`);
    return false;
  },

  // Hierarchy management functions
  // Cached hierarchy building with memoization
  _hierarchyCache: new Map(),
  
  buildHierarchyFromBabylon(babylonObject, depth = 0) {
    if (!babylonObject) return null;
    
    const objectId = babylonObject.uniqueId || babylonObject.name || `${babylonObject.getClassName()}-${Math.random()}`;
    
    // Check cache first to avoid rebuilding unchanged objects
    const cacheKey = `${objectId}-${depth}-${babylonObject.isVisible}-${babylonObject.getChildren?.()?.length || 0}`;
    if (this._hierarchyCache.has(cacheKey)) {
      return this._hierarchyCache.get(cacheKey);
    }
    
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
    
    // For imported asset containers, don't show children to keep hierarchy clean
    const isImportedAsset = babylonObject.getClassName() === 'TransformNode' && 
                           babylonObject.getChildren && 
                           babylonObject.getChildren().some(child => 
                             child.getClassName && child.getClassName().includes('Mesh')
                           );
    
    // Only build children for non-imported assets or when specifically requested
    if (babylonObject.getChildren && !isImportedAsset && depth < 3) { // Limit depth for performance
      const babylonChildren = babylonObject.getChildren();
      for (const child of babylonChildren) {
        if (child.name && !child.name.startsWith('__') && !child.name.includes('gizmo')) {
          children.push(this.buildHierarchyFromBabylon(child, depth + 1));
        }
      }
    }
    
    const result = {
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
    
    // Cache the result
    this._hierarchyCache.set(cacheKey, result);
    
    // Limit cache size to prevent memory leaks
    if (this._hierarchyCache.size > 100) {
      const firstKey = this._hierarchyCache.keys().next().value;
      this._hierarchyCache.delete(firstKey);
    }
    
    return result;
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
    
    // Separate lights, cameras, environment objects, and other objects for organization
    const lights = rootObjects.filter(obj => obj.getClassName && obj.getClassName().includes('Light'));
    const cameras = rootObjects.filter(obj => obj.getClassName && obj.getClassName().includes('Camera'));
    const environmentObjects = rootObjects.filter(obj => 
      obj.name && (obj.name.toLowerCase().includes('skybox') || 
      (obj.name.toLowerCase().includes('moon') && (!obj.getClassName || !obj.getClassName().includes('Light'))))
    );
    const otherObjects = rootObjects.filter(obj => 
      (!obj.getClassName || (!obj.getClassName().includes('Light') && !obj.getClassName().includes('Camera'))) &&
      (!obj.name || (!obj.name.toLowerCase().includes('skybox') && 
      !(obj.name.toLowerCase().includes('moon') && (!obj.getClassName || !obj.getClassName().includes('Light')))))
    );
    
    const hierarchyItems = [];
    
    // Add cameras first
    hierarchyItems.push(...cameras.map(obj => this.buildHierarchyFromBabylon(obj)));
    
    // Add other objects
    hierarchyItems.push(...otherObjects.map(obj => this.buildHierarchyFromBabylon(obj)));
    
    // Create virtual Environment folder if there are environment objects
    if (environmentObjects.length > 0) {
      // Check if folder already exists to prevent duplicates
      const existingEnvFolder = hierarchyItems.find(item => item.id === 'environment-folder');
      if (!existingEnvFolder) {
        const environmentFolder = {
          id: 'environment-folder',
          name: 'Environment',
          type: 'folder',
          visible: true,
          expanded: true,
          children: environmentObjects.map(obj => this.buildHierarchyFromBabylon(obj))
        };
        hierarchyItems.push(environmentFolder);
      }
    }
    
    // Create virtual Lighting folder if there are lights (add at end)
    if (lights.length > 0) {
      // Check if folder already exists to prevent duplicates
      const existingLightFolder = hierarchyItems.find(item => item.id === 'lighting-folder');
      if (!existingLightFolder) {
        const lightingFolder = {
          id: 'lighting-folder',
          name: 'Lighting',
          type: 'folder',
          visible: true,
          expanded: true,
          children: lights.map(light => this.buildHierarchyFromBabylon(light))
        };
        hierarchyItems.push(lightingFolder);
      }
    }
    
    const hierarchy = [{
      id: 'scene-root',
      name: 'New Scene',
      type: 'scene',
      expanded: true,
      children: hierarchyItems
    }];
    
    setRenderStore('hierarchy', hierarchy);
    // Scene hierarchy initialized
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

    // Object added to scene hierarchy
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

    // Object removed from scene hierarchy
  },


  updateObjectVisibility(objectId, visible) {
    setRenderStore('hierarchy', prev => {
      const updateVisibilityInNodes = (nodes) => {
        return nodes.map(node => {
          if (node.id === objectId) {
            return { ...node, visible: visible };
          }
          if (node.children) {
            return { ...node, children: updateVisibilityInNodes(node.children) };
          }
          return node;
        });
      };
      return updateVisibilityInNodes(prev);
    });
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
  },

  clearProjectData() {
    // Complete state reset for project switching - preserves engine but clears all project data
    console.log('🧹 RenderStore: Clearing all project data...');
    
    // Clear selection and transform state
    setRenderStore('selectedObject', null);
    setRenderStore('transformMode', 'select');
    setRenderStore('isGizmoDragging', false);
    
    // Clear hierarchy completely
    setRenderStore('hierarchy', []);
    
    // Clear hierarchy cache to prevent stale references
    this._hierarchyCache.clear();
    
    // Don't dispose engine - we want to keep it for new project
    // Don't dispose scene here - scene disposal is handled by render component
    
    // Reset camera reference (new scene will create new camera)
    setRenderStore('camera', null);
    
    // Dispose and clear gizmo manager (will be recreated with new scene)
    if (renderStore.gizmoManager) {
      try {
        renderStore.gizmoManager.dispose();
      } catch (error) {
        console.warn('Warning disposing gizmo manager:', error);
      }
      setRenderStore('gizmoManager', null);
    }
    
    // Dispose and clear highlight layer (will be recreated with new scene)
    if (renderStore.highlightLayer) {
      try {
        renderStore.highlightLayer.dispose();
      } catch (error) {
        console.warn('Warning disposing highlight layer:', error);
      }
      setRenderStore('highlightLayer', null);
    }
    
    // Reset settings to defaults (except engine which should persist)
    setRenderStore('settings', {
      backgroundColor: '#1a202c',
      enableGrid: true,
      gridSize: 10,
      renderingEngine: renderStore.settings.renderingEngine // Preserve engine choice
    });
    
    console.log('✅ RenderStore project data cleared');
  }
};