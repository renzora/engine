import { onMount, onCleanup, createSignal, createEffect, onCleanup as solidOnCleanup } from 'solid-js';
import { Engine } from '@babylonjs/core/Engines/engine';
import { Scene } from '@babylonjs/core/scene';
import { UniversalCamera } from '@babylonjs/core/Cameras/universalCamera';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';
import { Ray } from '@babylonjs/core/Culling/ray';
import { Color4 } from '@babylonjs/core/Maths/math.color';
import { Color3 } from '@babylonjs/core/Maths/math.color';
import '@babylonjs/core/Cameras/Inputs/arcRotateCameraPointersInput';
import '@babylonjs/core/Cameras/Inputs/arcRotateCameraKeyboardMoveInput';
import '@babylonjs/core/Cameras/Inputs/arcRotateCameraMouseWheelInput';
import { GizmoManager } from '@babylonjs/core/Gizmos/gizmoManager';
import { UtilityLayerRenderer } from '@babylonjs/core/Rendering/utilityLayerRenderer';
import { HighlightLayer } from '@babylonjs/core/Layers/highlightLayer';
import '@babylonjs/core/Rendering/edgesRenderer';
import '@babylonjs/core/Layers/effectLayerSceneComponent';
import '@babylonjs/core/Materials/standardMaterial';
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial';
import { Mesh } from '@babylonjs/core/Meshes/mesh';
import { TransformNode } from '@babylonjs/core/Meshes/transformNode';
import { SceneLoader } from '@babylonjs/core/Loading/sceneLoader';
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
import Stats from 'stats.js';
import { pluginAPI } from '@/api/plugin';
import { viewportStore } from '@/layout/stores/ViewportStore.jsx';



const loadDefaultSceneContent = (scene, canvas) => {
  if (window.DEBUG_RENDER) console.log('🌟 Loading default scene content');
  
  // Camera will be created during scene loading from saved data
  // No default objects created here - everything comes from scene data
  
  if (window.DEBUG_RENDER) console.log('✅ Default scene content loaded - objects will be restored from scene data');
};

export default function BabylonRenderer(props) {
  let canvasRef;
  let renderLoopRunning = false; // Track render loop state for proper cleanup
  const [engine, setEngine] = createSignal(null);
  const [scene, setScene] = createSignal(null);
  const [stats, setStats] = createSignal(null);
  
  // Initialize asset loader
  const { loadingTooltip, handleDragOver, handleDragEnter, handleDragLeave, handleDrop, loadAssetIntoScene, isPositioning } = useAssetLoader(scene, () => canvasRef);

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
    // Focus on selected object
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
      
      // Position camera at a safe angle (45 degrees up and back) to avoid going through object
      const backDirection = new Vector3(1, 1, 1).normalize(); // Back, up, and to the side
      const newCameraPosition = center.add(backDirection.scale(focusDistance));
      
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
          console.log('✅ Camera focused on object');
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
    // Delete selected object
    deleteObject: () => {
      const selectedObject = renderStore.selectedObject;
      
      if (!selectedObject) {
        console.log('⚠️ No object selected to delete');
        return;
      }
      
      
      console.log('🗑️ Deleting object:', selectedObject.name);
      
      // Remove from render store (this also updates the hierarchy)
      renderActions.removeObject(selectedObject);
      
      console.log('✅ Object deleted successfully');
    },
    // Snap to ground
    snapToGround: () => {
      const selectedObject = renderStore.selectedObject;
      const currentScene = renderStore.scene;
      
      if (!selectedObject || !currentScene) {
        console.log('⚠️ No object selected or no scene available for snap to ground');
        return;
      }
      
      console.log('📍 Snapping object to ground:', selectedObject.name);
      
      // Cast ray downward from object position to find ground
      const ray = new Ray(selectedObject.position.add(new Vector3(0, 100, 0)), new Vector3(0, -1, 0));
      const hit = currentScene.pickWithRay(ray, (mesh) => {
        // Exclude the selected object itself and any gizmo/helper objects
        return mesh !== selectedObject && 
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
        
        // Add console message to editor
        editorActions.addConsoleMessage(`Snapped "${selectedObject.name}" to surface`, 'success');
      } else {
        // Fallback: snap to Y=0 (ground plane)
        const boundingInfo = selectedObject.getBoundingInfo();
        const yOffset = Math.abs(boundingInfo.minimum.y);
        selectedObject.position.y = yOffset;
        console.log('⬇️ No surface found, snapped to ground plane at Y:', yOffset);
        
        // Add console message to editor
        editorActions.addConsoleMessage(`Snapped "${selectedObject.name}" to ground plane`, 'info');
      }
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
      const defaultPosition = new Vector3(5, 3, -5);  // Back, up, and to the side
      const defaultTarget = Vector3.Zero();            // Look at origin
      
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
          if (pointerInfo.pickInfo?.hit && pointerInfo.pickInfo.pickedMesh) {
            let targetObject = pointerInfo.pickInfo.pickedMesh;
            console.log('🎯 Clicked on mesh:', targetObject.name, 'class:', targetObject.getClassName());
            
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
            
            // Use shared selection - this will update both render and editor stores
            console.log('🔗 Calling renderActions.selectObject with:', targetObject.name, 'ID:', targetObject.uniqueId);
            renderActions.selectObject(targetObject);
          } else {
            // Left click but no hit - deselect
            renderActions.selectObject(null);
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
        onContextMenu={props.onContextMenu}
        onDragOver={handleDragOver}
        onDragEnter={handleDragEnter}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
      />
      <GizmoManagerComponent />
      <LoadingTooltip loadingTooltip={loadingTooltip} />
      <AxisHelper />
      
      {/* Time Display */}
      <div 
        id="time-display"
        className="absolute top-4 right-4 bg-black/50 text-white px-3 py-1 rounded-lg font-mono text-sm backdrop-blur-sm"
      >
        12:00
      </div>
    </>
  );
}
