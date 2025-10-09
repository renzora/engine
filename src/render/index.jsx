import { onMount, onCleanup, createSignal, createEffect } from 'solid-js';
import { Engine } from '@babylonjs/core/Engines/engine';
import { Scene } from '@babylonjs/core/scene';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';
import { Ray } from '@babylonjs/core/Culling/ray';
import { Color4 } from '@babylonjs/core/Maths/math.color';
import '@babylonjs/core/Cameras/Inputs/arcRotateCameraPointersInput';
import '@babylonjs/core/Cameras/Inputs/arcRotateCameraKeyboardMoveInput';
import '@babylonjs/core/Cameras/Inputs/arcRotateCameraMouseWheelInput';
import { HighlightLayer } from '@babylonjs/core/Layers/highlightLayer';
import '@babylonjs/core/Rendering/edgesRenderer';
import '@babylonjs/core/Layers/effectLayerSceneComponent';
import '@babylonjs/core/Materials/standardMaterial';
import '@babylonjs/loaders/glTF';
import { HavokPlugin } from '@babylonjs/core/Physics/v2/Plugins/havokPlugin';
import HavokPhysics from '@babylonjs/havok';
import '@babylonjs/core/Physics/physicsEngineComponent';
import { bridgeService } from '@/plugins/core/bridge';
import { renderStore, renderActions } from './store.jsx';
import { editorStore, editorActions } from '@/layout/stores/EditorStore';
import { grid } from './hooks/grid.jsx';
import { renderShortcuts } from './hooks/renderShortcuts';
import { useCameraController } from './hooks/cameraMovement.jsx';
import { GizmoManagerComponent } from './hooks/gizmo.jsx';
import { useAssetLoader } from './hooks/assetLoader.jsx';
import { LoadingTooltip } from './components/LoadingTooltip.jsx';
import { AxisHelper } from './components/AxisHelper.jsx';
import { pluginAPI } from '@/api/plugin';
import { viewportStore } from '@/layout/stores/ViewportStore.jsx';



const loadDefaultSceneContent = (scene, _canvas) => {
  if (window.DEBUG_RENDER) console.log('🌟 Loading default scene content');
  
  // Add default lighting if no lights exist
  if (scene.lights.length === 0) {
    // Import lighting classes
    import('@babylonjs/core/Lights/hemisphericLight').then(({ HemisphericLight }) => {
      import('@babylonjs/core/Lights/directionalLight').then(({ DirectionalLight }) => {
        import('@babylonjs/core/Maths/math.vector').then(({ Vector3 }) => {
          import('@babylonjs/core/Maths/math.color').then(({ Color3 }) => {
            // Add ambient hemispheric light for general illumination
            const hemisphericLight = new HemisphericLight('defaultAmbient', new Vector3(0, 1, 0), scene);
            hemisphericLight.intensity = 0.6;
            hemisphericLight.diffuse = new Color3(1, 1, 1);
            hemisphericLight.groundColor = new Color3(0.3, 0.3, 0.3);
            
            // Add directional light for proper shading
            const directionalLight = new DirectionalLight('defaultDirectional', new Vector3(-0.5, -1, -0.5), scene);
            directionalLight.intensity = 0.8;
            directionalLight.diffuse = new Color3(1, 0.95, 0.9);
            directionalLight.specular = new Color3(1, 1, 1);
            
            if (window.DEBUG_RENDER) console.log('✅ Default lighting added to scene');
          });
        });
      });
    });
  }
  
  // Camera will be created during scene loading from saved data
  // No default objects created here - everything comes from scene data
  
  if (window.DEBUG_RENDER) console.log('✅ Default scene content loaded - objects will be restored from scene data');
};

// Function to snap an object to the grid
const _snapObjectToGrid = (object) => {
  if (!object) return;
  
  // Get grid cell size from editor store, fallback to 1.0 if not available
  const gridSettings = editorStore.settings?.grid;
  const gridSize = gridSettings?.cellSize || 1.0;
  
  // Snap position to grid
  const snappedX = Math.round(object.position.x / gridSize) * gridSize;
  const snappedY = Math.round(object.position.y / gridSize) * gridSize;
  const snappedZ = Math.round(object.position.z / gridSize) * gridSize;
  
  object.position.x = snappedX;
  object.position.y = snappedY;
  object.position.z = snappedZ;
  
  console.log(`📍 Snapped "${object.name}" to grid: (${snappedX}, ${snappedY}, ${snappedZ}) with grid size ${gridSize}`);
  
  // Add console message to editor
  if (editorActions && editorActions.addConsoleMessage) {
    editorActions.addConsoleMessage(`Snapped "${object.name}" to grid (${gridSize}m)`, 'success');
  }
};

export default function BabylonRenderer(_props) {
  let canvasRef = null;
  let renderLoopRunning = false; // Track render loop state for proper cleanup
  const [engine, setEngine] = createSignal(null);
  const [scene, setScene] = createSignal(null);
  const [stats, _setStats] = createSignal(null);
  
  // Initialize asset loader
  const { loadingTooltip, handleDragOver, handleDragEnter, handleDragLeave, handleDrop, loadAssetIntoScene: _loadAssetIntoScene, isPositioning: _isPositioning } = useAssetLoader(scene, () => canvasRef);

  // Show UI panels when this 3D viewport is active
  createEffect(() => {
    const activeTabId = viewportStore.activeTabId;
    const activeTab = viewportStore.tabs.find(t => t.id === activeTabId);
    
    if (activeTab?.type === '3d-viewport') {
      pluginAPI.showPanel();
      pluginAPI.showProps();
      pluginAPI.showMenu();
      pluginAPI.showTabs();
    }
  });

  onMount(() => {
    initializeBabylon();
  });

  // Initialize grid system
  grid(scene);

  // Initialize camera controller
  useCameraController(() => scene()?.activeCamera, () => canvasRef, scene);

  // Initialize global shortcuts (no movement keys)
  renderShortcuts({
    // Snap mode checking
    checkSnapMode: () => viewportStore.gridSnapping,
    
    // Transform gizmos
    positionMode: () => {
      const gizmoManager = renderStore.gizmoManager;
      if (gizmoManager) {
        gizmoManager.positionGizmoEnabled = !gizmoManager.positionGizmoEnabled;
        gizmoManager.rotationGizmoEnabled = false;
        gizmoManager.scaleGizmoEnabled = false;
      }
    },
    rotateMode: () => {
      const gizmoManager = renderStore.gizmoManager;
      if (gizmoManager) {
        gizmoManager.rotationGizmoEnabled = !gizmoManager.rotationGizmoEnabled;
        gizmoManager.positionGizmoEnabled = false;
        gizmoManager.scaleGizmoEnabled = false;
      }
    },
    scaleMode: () => {
      const gizmoManager = renderStore.gizmoManager;
      if (gizmoManager) {
        gizmoManager.scaleGizmoEnabled = !gizmoManager.scaleGizmoEnabled;
        gizmoManager.positionGizmoEnabled = false;
        gizmoManager.rotationGizmoEnabled = false;
      }
    },
    // Focus on selected object based on current camera view
    focusObject: () => {
      const selectedObject = renderStore.selectedObject;
      const camera = renderStore.camera;
      
      if (!selectedObject || !camera) {
        console.log('⚠️ No object selected or no camera available for focus');
        return;
      }
      
      console.log('🎯 Focusing camera on object:', selectedObject.name);
      
      // Cancel any existing focus animation
      if (window._focusAnimationId) {
        cancelAnimationFrame(window._focusAnimationId);
        window._focusAnimationId = null;
      }
      
      // Get the bounding box of the selected object
      let boundingInfo, center, size;
      
      if (selectedObject.getClassName() === 'TransformNode') {
        // For TransformNode containers, use hierarchy bounding box
        const hierarchyBounds = selectedObject.getHierarchyBoundingVectors();
        center = hierarchyBounds.min.add(hierarchyBounds.max).scale(0.5);
        size = hierarchyBounds.max.subtract(hierarchyBounds.min);
      } else {
        // For regular meshes, use standard bounding box
        boundingInfo = selectedObject.getBoundingInfo();
        center = boundingInfo.boundingBox.centerWorld;
        size = boundingInfo.boundingBox.extendSizeWorld;
      }
      
      // Calculate distance to fit entire object in camera view
      const maxDimension = Math.max(size.x, size.y, size.z);
      const cameraFov = camera.fov || 0.8; // Default FOV in radians
      
      // Calculate distance needed to fit object in view
      // Using half FOV and some padding for better framing
      const padding = 1.2; // 20% padding around object
      const focusDistance = Math.max(1, (maxDimension * padding) / (2 * Math.tan(cameraFov / 2)));
      
      console.log(`Object size: ${maxDimension.toFixed(2)}, FOV: ${(cameraFov * 180/Math.PI).toFixed(1)}°, focus distance: ${focusDistance.toFixed(2)}`);
      
      // Calculate the current camera direction to maintain the viewing angle
      const cameraDirection = camera.getDirection ? 
        camera.getDirection(Vector3.Forward()).normalize() : 
        camera.getForwardRay().direction.normalize();
      
      // Position camera at the calculated distance in the current viewing direction
      const newCameraPosition = center.subtract(cameraDirection.scale(focusDistance));
      
      // Smooth animation to new position
      const startPosition = camera.position.clone();
      const animationDuration = 500; // ms
      const startTime = Date.now();
      
      const animate = () => {
        const elapsed = Date.now() - startTime;
        const progress = Math.min(elapsed / animationDuration, 1);
        
        // Smooth easing function (ease-out)
        const easedProgress = 1 - Math.pow(1 - progress, 3);
        
        // Interpolate position
        camera.position = Vector3.Lerp(startPosition, newCameraPosition, easedProgress);
        
        // Set target to object center
        if (camera.setTarget) {
          camera.setTarget(center);
        }
        
        if (progress < 1) {
          window._focusAnimationId = requestAnimationFrame(animate);
        } else {
          window._focusAnimationId = null;
          console.log('✅ Camera focused on object while maintaining current view direction');
          editorActions.addConsoleMessage(`Focused camera on "${selectedObject.name}"`, 'success');
        }
      };
      
      window._focusAnimationId = requestAnimationFrame(animate);
    },
    // Focus on mouse point
    focusOnMousePoint: () => {
      const camera = renderStore.camera;
      const currentScene = renderStore.scene;
      
      if (!camera || !currentScene) {
        console.log('⚠️ No camera or scene available for focus');
        return;
      }
      
      // Cancel any existing mouse focus animation
      if (window._mouseFocusAnimationId) {
        cancelAnimationFrame(window._mouseFocusAnimationId);
        window._mouseFocusAnimationId = null;
      }
      
      // Get current mouse position from last known coordinates
      const canvas = canvasRef;
      if (!canvas) {
        console.log('⚠️ No canvas available');
        return;
      }
      
      // Cast ray from camera through mouse position
      const pickInfo = currentScene.pick(
        currentScene.pointerX || canvas.width / 2,
        currentScene.pointerY || canvas.height / 2
      );
      
      if (pickInfo && pickInfo.hit && pickInfo.pickedPoint) {
        const targetPoint = pickInfo.pickedPoint;
        console.log('🎯 Focusing camera on point:', targetPoint);
        
        // Calculate camera position maintaining current distance but looking at picked point
        const currentDistance = Vector3.Distance(camera.position, camera.getTarget());
        const direction = camera.position.subtract(targetPoint).normalize();
        const newCameraPosition = targetPoint.add(direction.scale(currentDistance));
        
        // Smooth animation to new position
        const startPosition = camera.position.clone();
        const animationDuration = 500; // ms
        const startTime = Date.now();
        
        const animate = () => {
          const elapsed = Date.now() - startTime;
          const progress = Math.min(elapsed / animationDuration, 1);
          
          // Smooth easing function (ease-out)
          const easedProgress = 1 - Math.pow(1 - progress, 3);
          
          // Interpolate position
          camera.position = Vector3.Lerp(startPosition, newCameraPosition, easedProgress);
          
          // Set target to picked point
          if (camera.setTarget) {
            camera.setTarget(targetPoint);
          }
          
          if (progress < 1) {
            window._mouseFocusAnimationId = requestAnimationFrame(animate);
          } else {
            window._mouseFocusAnimationId = null;
            // Camera focused on mouse point
            editorActions.addConsoleMessage('Focused camera on mouse point', 'success');
          }
        };
        
        window._mouseFocusAnimationId = requestAnimationFrame(animate);
      } else {
        // No surface found at mouse position
        editorActions.addConsoleMessage('No surface found at mouse position', 'warning');
      }
    },
    // Delete selected objects
    deleteObject: () => {
      const selectedObjects = renderStore.selectedObjects;
      
      if (!selectedObjects || selectedObjects.length === 0) {
        console.log('⚠️ No objects selected to delete');
        return;
      }
      
      console.log(`🗑️ Deleting ${selectedObjects.length} object(s):`, selectedObjects.map(obj => obj.name));
      
      // Remove all selected objects from render store
      selectedObjects.forEach(obj => {
        renderActions.removeObject(obj);
      });
      
      console.log('✅ Objects deleted successfully');
    },
    // Duplicate selected objects
    duplicate: () => {
      const selectedObjects = renderStore.selectedObjects;
      
      if (!selectedObjects || selectedObjects.length === 0) {
        console.log('⚠️ No objects selected to duplicate');
        return;
      }
      
      console.log(`📋 Duplicating ${selectedObjects.length} object(s):`, selectedObjects.map(obj => obj.name));
      
      try {
        const duplicatedObjects = [];
        
        selectedObjects.forEach(selectedObject => {
          let newObject = selectedObject.clone(selectedObject.name + '_duplicate', null, false, true);
          
          if (newObject) {
            // Keep the duplicated object at the same position as original initially
            newObject.position.copyFrom(selectedObject.position);
            
            // Add object to hierarchy
            renderActions.addObject(newObject);
            duplicatedObjects.push(newObject);
          }
        });
        
        if (duplicatedObjects.length > 0) {
          // Select all duplicated objects
          renderActions.selectObject(null); // Clear current selection first
          duplicatedObjects.forEach((obj, index) => {
            renderActions.selectObject(obj, index > 0); // Multi-select for all but the first
          });
          
          // Trigger Blender-style grab mode (equivalent to pressing 'G')
          setTimeout(() => {
            // Call the transform function directly
            if (window.triggerBlenderTransform) {
              window.triggerBlenderTransform('move');
            }
          }, 50); // Small delay to ensure selection is complete
          
          console.log('✅ Objects duplicated successfully and grab mode will be activated');
        }
      } catch (error) {
        console.error('❌ Failed to duplicate objects:', error);
      }
    },
    // Snap to ground
    snapToGround: () => {
      const selectedObjects = renderStore.selectedObjects;
      const currentScene = renderStore.scene;
      
      if (!selectedObjects || selectedObjects.length === 0 || !currentScene) {
        console.log('⚠️ No objects selected or no scene available for snap to ground');
        return;
      }
      
      console.log(`📍 Snapping ${selectedObjects.length} object(s) to ground:`, selectedObjects.map(obj => obj.name));
      
      selectedObjects.forEach(selectedObject => {
        // Cast ray downward from object position to find ground
        const ray = new Ray(selectedObject.position.add(new Vector3(0, 100, 0)), new Vector3(0, -1, 0));
        const hit = currentScene.pickWithRay(ray, (mesh) => {
          // Exclude the selected objects and any gizmo/helper objects
          return !selectedObjects.includes(mesh) && 
                 !mesh.name.includes('gizmo') && 
                 !mesh.name.includes('helper') &&
                 !mesh.name.startsWith('__');
        });
        
        if (hit && hit.hit && hit.pickedPoint) {
          // Get the bounding box of the selected object to calculate proper offset
          const boundingInfo = selectedObject.getBoundingInfo();
          const yOffset = Math.abs(boundingInfo.minimum.y);
          
          // Snap to the surface with proper offset so object sits on top
          selectedObject.position.y = hit.pickedPoint.y + yOffset;
          console.log('✅ Object snapped to surface at Y:', hit.pickedPoint.y + yOffset);
        } else {
          // Fallback: snap to Y=0 (ground plane)
          const boundingInfo = selectedObject.getBoundingInfo();
          const yOffset = Math.abs(boundingInfo.minimum.y);
          selectedObject.position.y = yOffset;
          console.log('⬇️ No surface found, snapped to ground plane at Y:', yOffset);
        }
      });
      
      // Add console message to editor
      editorActions.addConsoleMessage(`Snapped ${selectedObjects.length} object(s) to ground`, 'success');
    },
    // Reset camera to default position
    resetCamera: () => {
      const camera = renderStore.camera;
      const currentScene = renderStore.scene;
      
      if (!camera || !currentScene) {
        console.log('⚠️ No camera or scene available for reset');
        return;
      }
      
      console.log('🏠 Resetting camera to default position');
      
      // Cancel any existing camera animation
      if (window._cameraResetAnimationId) {
        cancelAnimationFrame(window._cameraResetAnimationId);
        window._cameraResetAnimationId = null;
      }
      
      // Define default camera position and target
      const defaultPosition = new Vector3(7, 8, -7);  // Angled view with elevated position for horizon view
      const defaultTarget = new Vector3(0, 2, 0);     // Look slightly above origin to show horizon
      
      // Smooth animation to default position
      const startPosition = camera.position.clone();
      const startTarget = camera.getTarget ? camera.getTarget() : Vector3.Zero();
      const animationDuration = 200; // 0.2 seconds - super fast
      const startTime = Date.now();
      
      const animate = () => {
        const elapsed = Date.now() - startTime;
        const progress = Math.min(elapsed / animationDuration, 1);
        
        // Smooth easing function (ease-in-out)
        const easedProgress = progress < 0.5 
          ? 2 * progress * progress 
          : 1 - Math.pow(-2 * progress + 2, 3) / 2;
        
        // Interpolate position and target
        camera.position = Vector3.Lerp(startPosition, defaultPosition, easedProgress);
        
        // Set target to origin
        if (camera.setTarget) {
          const currentTarget = Vector3.Lerp(startTarget, defaultTarget, easedProgress);
          camera.setTarget(currentTarget);
        }
        
        if (progress < 1) {
          window._cameraResetAnimationId = requestAnimationFrame(animate);
        } else {
          window._cameraResetAnimationId = null;
          console.log('✅ Camera reset to default position');
          editorActions.addConsoleMessage('Camera reset to default position', 'success');
        }
      };
      
      window._cameraResetAnimationId = requestAnimationFrame(animate);
    },
    // Save project
    save: () => {
      console.log('💾 Save requested');
      try {
        // Trigger save through bridge service or plugin API
        if (bridgeService && bridgeService.saveProject) {
          bridgeService.saveProject();
        } else if (pluginAPI && pluginAPI.saveProject) {
          pluginAPI.saveProject();
        } else {
          console.log('⚠️ No save function available');
          editorActions.addConsoleMessage('Save not available', 'warning');
        }
      } catch (error) {
        console.error('❌ Save failed:', error);
        editorActions.addConsoleMessage('Save failed', 'error');
      }
    },
    // Toggle bottom panel
    toggleBottomPanel: () => {
      const currentVisible = pluginAPI.getBottomPanelVisible();
      pluginAPI.setBottomPanelVisible(!currentVisible);
      editorActions.addConsoleMessage(`Bottom panel ${!currentVisible ? 'shown' : 'hidden'}`, 'info');
      
      // Recalculate viewport after panel toggle
      setTimeout(() => {
        if (renderStore.engine) {
          renderStore.engine.resize();
          console.log('🔄 Viewport recalculated for bottom panel toggle');
        }
      }, 50);
    },
    // Toggle right panel (properties panel)
    toggleRightPanel: () => {
      const currentVisible = pluginAPI.getPropertiesPanelVisible();
      pluginAPI.setPropertiesPanelVisible(!currentVisible);
      editorActions.addConsoleMessage(`Properties panel ${!currentVisible ? 'shown' : 'hidden'}`, 'info');
      
      // Recalculate viewport after panel toggle
      setTimeout(() => {
        if (renderStore.engine) {
          renderStore.engine.resize();
          console.log('🔄 Viewport recalculated for right panel toggle');
        }
      }, 50);
    }
  });

  onCleanup(() => {
    cleanup();
  });

  // Watch for viewport settings changes
  createEffect(() => {
    const viewportSettings = editorStore.settings.viewport;
    if (scene() && viewportSettings.backgroundColor) {
      renderActions.updateSettings({
        backgroundColor: viewportSettings.backgroundColor
      });
    }
  });

  const initializeBabylon = async () => {
    if (!canvasRef) return;

    try {
      if (window.DEBUG_RENDER) console.log('🎮 Initializing Babylon.js engine only...');

      // Create engine only - no scene yet
      const babylonEngine = new Engine(canvasRef, true, {
        adaptToDeviceRatio: true,
        antialias: true
      });

      // Store engine but no scene yet
      renderActions.setEngine(babylonEngine);
      setEngine(babylonEngine);
      
      if (window.DEBUG_RENDER) console.log('✅ Babylon.js engine ready, waiting for project to create scene');
      
      // Set up resize handling
      window.addEventListener('resize', () => {
        babylonEngine.resize();
      });

      // Engine is ready - scene will be created when project loads

    } catch (error) {
      console.error('❌ Failed to initialize Babylon.js engine:', error);
    }
  };

  // Create Babylon scene after project is loaded
  const createBabylonScene = async () => {
    const babylonEngine = engine();
    if (!babylonEngine) {
      console.error('❌ Cannot create scene - no engine available');
      return null;
    }

    // If scene exists, dispose it properly without destroying the engine
    if (scene()) {
      console.warn('⚠️ Scene already exists, disposing first');
      const babylonScene = scene();
      
      // Stop render loop
      renderLoopRunning = false;
      
      // Clean up particle systems
      if (babylonScene) {
        const particleSystems = babylonScene.particleSystems?.slice() || [];
        particleSystems.forEach(system => {
          if (system && typeof system.dispose === 'function') {
            system.stop();
            system.dispose();
          }
        });
        
        // Unregister all before render callbacks
        babylonScene.unregisterBeforeRender();
        
        // Dispose scene but keep engine
        babylonScene.dispose();
      }
      
      // Clear scene signal
      setScene(null);
      
      // Use render store cleanup (but this might also dispose engine)
      renderActions.cleanup();
      
      // Small delay to ensure cleanup is complete
      await new Promise(resolve => setTimeout(resolve, 100));
    }

    try {
      if (window.DEBUG_RENDER) console.log('🎮 Creating Babylon.js scene...');

      // Ensure engine is still valid after cleanup
      const currentEngine = engine();
      if (!currentEngine || currentEngine.isDisposed) {
        console.error('❌ Engine was disposed during cleanup, cannot create scene');
        return null;
      }

      // Create scene
      const babylonScene = new Scene(currentEngine);
      babylonScene.clearColor = new Color4(0.1, 0.1, 0.1, 1.0);

      // Initialize Havok Physics v2
      try {
        if (window.DEBUG_RENDER) console.log('🌟 Initializing Havok Physics v2...');
        const havokInstance = await HavokPhysics();
        const hk = new HavokPlugin(true, havokInstance);
        const gravity = new Vector3(0, -9.81, 0);
        babylonScene.enablePhysics(gravity, hk);
        if (window.DEBUG_RENDER) console.log('✅ Havok Physics v2 initialized successfully');
      } catch (error) {
        console.warn('⚠️ Failed to initialize Havok Physics v2:', error);
        console.warn('Physics features will be disabled');
      }

      // Start render loop  
      renderLoopRunning = true;
      currentEngine.runRenderLoop(() => {
        if (renderLoopRunning && babylonScene && !babylonScene.isDisposed && babylonScene.activeCamera) {
          babylonScene.render();
        }
      });

      // Set up pointer click handling for object selection
      babylonScene.onPointerObservable.add((pointerInfo) => {
        // Only process left-click events (button 0) - let right-click pass through to camera controller
        if (pointerInfo.type === 1 && pointerInfo.event && pointerInfo.event.button === 0) { // LEFT CLICK only
          // Check if we're in transform mode - if so, don't process selection changes
          if (renderStore.isTransformActive) {
            console.log('🚫 Skipping selection - transform is active');
            return;
          }
          
          const isShiftPressed = pointerInfo.event.shiftKey;
          
          if (pointerInfo.pickInfo?.hit && pointerInfo.pickInfo.pickedMesh) {
            let targetObject = pointerInfo.pickInfo.pickedMesh;
            console.log('🎯 Clicked on mesh:', targetObject.name, 'class:', targetObject.getClassName(), 'shift:', isShiftPressed);
            
            // Walk up the hierarchy to find the top-level selectable object
            // Keep walking up until we reach a root object (no parent)
            console.log('🔍 Starting hierarchy walk from:', targetObject.name, 'ID:', targetObject.uniqueId);
            console.log('🔍 Initial parent check:', targetObject.parent?.name || 'none', 'ID:', targetObject.parent?.uniqueId || 'none');
            
            // Walk up until we reach a true root object (no parent) 
            // Don't stop at system objects if they have parents - keep going to reach our container
            let walkCount = 0;
            while (targetObject.parent && walkCount < 10) { // Safety limit
              walkCount++;
              
              console.log('⬆️ Walking up hierarchy from', targetObject.name, '(ID:', targetObject.uniqueId, ') to parent', targetObject.parent.name, '(ID:', targetObject.parent.uniqueId, ') parent class:', targetObject.parent.getClassName());
              targetObject = targetObject.parent;
              console.log('⬆️ Now at node:', targetObject.name, '(ID:', targetObject.uniqueId, ')');
              console.log('⬆️ Parent of current node:', targetObject.parent?.name || 'none', '(ID:', targetObject.parent?.uniqueId || 'none', ')');
              
              // Only stop if we encounter a system object that has no parent (true system root)
              const currentName = targetObject.name || '';
              const isSystemObject = currentName.startsWith('__') || 
                                   currentName.includes('gizmo') || 
                                   currentName.includes('helper');
              
              if (isSystemObject && !targetObject.parent) {
                console.log('🏁 Reached system root object:', currentName, '- stopping walk');
                break;
              }
            }
            
            if (walkCount >= 10) {
              console.log('⚠️ Hierarchy walk exceeded safety limit, stopping');
            }
            
            console.log('✅ Final selection target:', targetObject.name, '(ID:', targetObject.uniqueId, ') class:', targetObject.getClassName());
            
            // Use shared selection with multi-select parameter
            console.log('🔗 Calling renderActions.selectObject with:', targetObject.name, 'ID:', targetObject.uniqueId, 'multiSelect:', isShiftPressed);
            renderActions.selectObject(targetObject, isShiftPressed);
          } else {
            // Left click but no hit - deselect (only if deselection is allowed)
            if (!isShiftPressed && editorActions.canDeselect()) {
              renderActions.selectObject(null);
            }
          }
        }
      });

      // Make scene globally accessible
      window._cleanBabylonScene = babylonScene;
      
      // Load default scene content first
      loadDefaultSceneContent(babylonScene, canvasRef);
      
      // Set scene in store (this initializes hierarchy with the content)
      renderActions.setScene(babylonScene);
      setScene(babylonScene);
      
      // Initialize highlight layer for selection
      const highlightLayer = new HighlightLayer("selectionHighlight", babylonScene);
      highlightLayer.blurHorizontalSize = 1.0;
      highlightLayer.blurVerticalSize = 1.0;
      highlightLayer.outerGlow = true;
      highlightLayer.innerGlow = false;
      renderActions.setHighlightLayer(highlightLayer);

      if (window.DEBUG_RENDER) console.log('✅ Babylon.js scene created successfully');
      return babylonScene;

    } catch (error) {
      console.error('❌ Failed to create Babylon.js scene:', error);
      return null;
    }
  };

  // Expose scene creation function globally so splash plugin can call it
  window._createBabylonScene = createBabylonScene;

  const cleanup = () => {
    const babylonEngine = engine();
    const babylonScene = scene();
    const statsInstance = stats();
    
    renderLoopRunning = false;
    
    // Cancel any running animations
    if (window._focusAnimationId) {
      cancelAnimationFrame(window._focusAnimationId);
      window._focusAnimationId = null;
    }
    if (window._mouseFocusAnimationId) {
      cancelAnimationFrame(window._mouseFocusAnimationId);
      window._mouseFocusAnimationId = null;
    }
    
    // Clean up particle systems before disposing scene
    if (babylonScene) {
      const particleSystems = babylonScene.particleSystems?.slice() || [];
      particleSystems.forEach(system => {
        if (system && typeof system.dispose === 'function') {
          system.stop();
          system.dispose();
        }
      });
      
      // Unregister all before render callbacks to prevent memory leaks
      babylonScene.unregisterBeforeRender();
    }
    
    // Use render store for cleanup
    renderActions.cleanup();
    
    if (babylonEngine) {
      babylonEngine.stopRenderLoop();
      babylonEngine.dispose();
    }
    
    // Clean up stats
    if (statsInstance && statsInstance.dom && statsInstance.dom.parentElement) {
      statsInstance.dom.parentElement.removeChild(statsInstance.dom);
    }
    
    window._cleanBabylonScene = null;
    window._dayNightCycle = null;
    renderActions.setEngine(null);
    renderActions.setScene(null);
    renderActions.setCamera(null);
    
    if (window.DEBUG_RENDER) console.log('🗑️ Babylon.js cleaned up');
  };

  return (
    <>
      <canvas
        ref={canvasRef}
        className="w-full h-full outline-none"
        style={{ 'touch-action': 'none' }}
        onDragOver={handleDragOver}
        onDragEnter={handleDragEnter}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
      />
      <GizmoManagerComponent />
      <LoadingTooltip loadingTooltip={loadingTooltip} />
      <AxisHelper />
    </>
  );
}
