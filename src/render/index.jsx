import { onMount, onCleanup, createSignal, createEffect } from 'solid-js';
import { Engine } from '@babylonjs/core/Engines/engine';
import { Scene } from '@babylonjs/core/scene';
import { UniversalCamera } from '@babylonjs/core/Cameras/universalCamera';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';
import { Ray } from '@babylonjs/core/Culling/ray';
import { DirectionalLight } from '@babylonjs/core/Lights/directionalLight';
import { ShadowGenerator } from '@babylonjs/core/Lights/Shadows/shadowGenerator';
import '@babylonjs/core/Lights/Shadows/shadowGeneratorSceneComponent';
import { Color4 } from '@babylonjs/core/Maths/math.color';
import { CreateBox } from '@babylonjs/core/Meshes/Builders/boxBuilder';
import { CreateGround } from '@babylonjs/core/Meshes/Builders/groundBuilder';
import { CreateLines } from '@babylonjs/core/Meshes/Builders/linesBuilder';
import { CreateSphere } from '@babylonjs/core/Meshes/Builders/sphereBuilder';
import { CubeTexture } from '@babylonjs/core/Materials/Textures/cubeTexture';
import '@babylonjs/core/Materials/Textures/Loaders/envTextureLoader';
import { CreateSkyBox } from '@babylonjs/core/Helpers/environmentHelper';
import { HemisphericLight } from '@babylonjs/core/Lights/hemisphericLight';
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial';
import { Color3 } from '@babylonjs/core/Maths/math.color';
import '@babylonjs/core/Cameras/Inputs/arcRotateCameraPointersInput';
import '@babylonjs/core/Cameras/Inputs/arcRotateCameraKeyboardMoveInput';
import '@babylonjs/core/Cameras/Inputs/arcRotateCameraMouseWheelInput';
import { GizmoManager } from '@babylonjs/core/Gizmos/gizmoManager';
import { UtilityLayerRenderer } from '@babylonjs/core/Rendering/utilityLayerRenderer';
import { HighlightLayer } from '@babylonjs/core/Layers/highlightLayer';
import '@babylonjs/core/Layers/effectLayerSceneComponent';
import '@babylonjs/core/Materials/standardMaterial';
import { Mesh } from '@babylonjs/core/Meshes/mesh';
import { CreateCylinder } from '@babylonjs/core/Meshes/Builders/cylinderBuilder';
import { CreateTorus } from '@babylonjs/core/Meshes/Builders/torusBuilder';
import { DynamicTexture } from '@babylonjs/core/Materials/Textures/dynamicTexture';
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
import { pluginAPI } from '@/api/plugin';
import { viewportStore } from '@/layout/stores/ViewportStore.jsx';



const loadDefaultSceneContent = (scene, canvas) => {
  console.log('🌟 Loading default scene content');
  
  // Create default camera positioned diagonally to show X and Z axis intersection
  const camera = new UniversalCamera(
    'camera',
    new Vector3(7, 5, 7),
    scene
  );
  // Don't attach Babylon's native controls - we use custom camera controller
  camera.setTarget(Vector3.Zero());


  // Enhanced lighting setup with skybox and better shadows
  
  // Create a simple gradient skybox manually
  const skybox = CreateBox('skyBox', { size: 1000 }, scene);
  const skyboxMaterial = new StandardMaterial('skyBox', scene);
  skyboxMaterial.backFaceCulling = false;
  skyboxMaterial.diffuseColor = new Color3(0, 0, 0);
  skyboxMaterial.specularColor = new Color3(0, 0, 0);
  skyboxMaterial.emissiveColor = new Color3(0.5, 0.7, 1.0); // Sky blue emission
  skybox.material = skyboxMaterial;
  skybox.infiniteDistance = true;
  
  // Sky ambient light - reduced intensity since we have skybox
  const skyLight = new HemisphericLight('skyLight', new Vector3(0, 1, 0), scene);
  skyLight.intensity = 0.3;
  skyLight.diffuse = new Color3(0.6, 0.8, 1.0);
  skyLight.groundColor = new Color3(0.2, 0.3, 0.4);
  
  // Main sun light - stronger and more dramatic
  const sunLight = new DirectionalLight('sunLight', new Vector3(-0.4, -0.7, -0.6), scene);
  sunLight.intensity = 1.2;
  sunLight.diffuse = new Color3(1.0, 0.95, 0.85);
  sunLight.specular = new Color3(1.0, 0.9, 0.8);
  
  // Position sun higher for better shadow angles
  const sunElevation = 60 * Math.PI / 180; // 60 degrees elevation
  const sunAzimuth = 135 * Math.PI / 180; // Southeast direction
  sunLight.direction = new Vector3(
    Math.cos(sunElevation) * Math.cos(sunAzimuth),
    -Math.sin(sunElevation),
    Math.cos(sunElevation) * Math.sin(sunAzimuth)
  );
  
  // Fill light - simulates bounce light from environment
  const fillLight = new DirectionalLight('fillLight', new Vector3(0.3, -0.2, 0.8), scene);
  fillLight.intensity = 0.15;
  fillLight.diffuse = new Color3(0.7, 0.8, 1.0);
  fillLight.specular = new Color3(0.0, 0.0, 0.0);

  // Enhanced shadow generator with better quality
  const shadowGenerator = new ShadowGenerator(4096, sunLight); // Higher resolution
  shadowGenerator.usePercentageCloserFiltering = true;
  shadowGenerator.filteringQuality = ShadowGenerator.QUALITY_HIGH;
  shadowGenerator.darkness = 0.4; // More pronounced shadows
  shadowGenerator.bias = 0.0001; // Tighter bias for sharper shadows
  
  // Contact hardening for realistic shadow softness
  shadowGenerator.useContactHardeningShadow = true;
  shadowGenerator.contactHardeningLightSizeUVRatio = 0.075;
  
  // Set shadow map size and enable blur
  shadowGenerator.blurBoxOffset = 2.0;
  shadowGenerator.blurScale = 2.0;
  shadowGenerator.blurKernel = 32;
  
  // Store shadow generator for access by physics objects
  scene.shadowGenerator = shadowGenerator;

  // Set camera in render store
  renderActions.setCamera(camera);
  
  console.log('✅ Default scene content loaded with lighting');
};

export default function BabylonRenderer(props) {
  let canvasRef;
  const [engine, setEngine] = createSignal(null);
  const [scene, setScene] = createSignal(null);
  
  // Initialize asset loader
  const { loadingTooltip, handleDragOver, handleDrop, loadAssetIntoScene } = useAssetLoader(scene, () => canvasRef);

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
          requestAnimationFrame(animate);
        } else {
          console.log('✅ Camera focused on object');
          editorActions.addConsoleMessage(`Focused camera on "${selectedObject.name}"`, 'success');
        }
      };
      
      animate();
    },
    // Focus on mouse point
    focusOnMousePoint: () => {
      const camera = renderStore.camera;
      const currentScene = renderStore.scene;
      
      if (!camera || !currentScene) {
        console.log('⚠️ No camera or scene available for focus');
        return;
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
            requestAnimationFrame(animate);
          } else {
            console.log('✅ Camera focused on mouse point');
            editorActions.addConsoleMessage('Focused camera on mouse point', 'success');
          }
        };
        
        animate();
      } else {
        console.log('⚠️ No surface found at mouse position');
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
      console.log('🎮 Initializing Babylon.js...');

      // Create engine
      const babylonEngine = new Engine(canvasRef, true, {
        adaptToDeviceRatio: true,
        antialias: true
      });

      // Create scene
      const babylonScene = new Scene(babylonEngine);
      babylonScene.useRightHandedSystem = true;
      babylonScene.clearColor = new Color4(0.1, 0.1, 0.15, 1);
      // Lower overall exposure to avoid overly bright results
      babylonScene.imageProcessingConfiguration.exposure = 0.85;
      
      // Enable FXAA for better anti-aliasing on lines and edges
      babylonScene.imageProcessingConfiguration.fxaaEnabled = true;

      // Enable Havok physics for RenScript
      try {
        const havokInstance = await HavokPhysics();
        const hk = new HavokPlugin(true, havokInstance);
        const enableResult = babylonScene.enablePhysics(new Vector3(0, -9.81, 0), hk);
        console.log('✅ Havok physics enabled for RenScript, result:', enableResult);
      } catch (error) {
        console.warn('⚠️ Failed to enable Havok physics:', error);
      }

      // Scene content will be loaded separately
      console.log('🎮 Scene created, ready for content loading');

      // Create highlight layer for selection
      const highlightLayer = new HighlightLayer('highlight', babylonScene);
      
      // Store in render store (gizmo manager will be set by GizmoManagerComponent)
      renderActions.setHighlightLayer(highlightLayer);

      // Start render loop
      babylonEngine.runRenderLoop(() => {
        babylonScene.render();
      });

      // Handle resize
      window.addEventListener('resize', () => {
        babylonEngine.resize();
      });


      // Add object picking for selection - only on LEFT CLICK to avoid interfering with camera
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

      // Update store
      renderActions.setEngine(babylonEngine);
      
      // Load default scene content first
      loadDefaultSceneContent(babylonScene, canvasRef);
      
      // Set scene in store (this initializes hierarchy with the content)
      renderActions.setScene(babylonScene);

      setEngine(babylonEngine);
      setScene(babylonScene);

      console.log('✅ Babylon.js initialized successfully');

    } catch (error) {
      console.error('❌ Failed to initialize Babylon.js:', error);
    }
  };

  const cleanup = () => {
    const babylonEngine = engine();
    const babylonScene = scene();
    
    
    // Use render store for cleanup
    renderActions.cleanup();
    
    if (babylonEngine) {
      babylonEngine.stopRenderLoop();
      babylonEngine.dispose();
    }
    
    window._cleanBabylonScene = null;
    renderActions.setEngine(null);
    renderActions.setScene(null);
    renderActions.setCamera(null);
    
    console.log('🗑️ Babylon.js cleaned up');
  };

  return (
    <>
      <canvas
        ref={canvasRef}
        className="w-full h-full outline-none"
        style={{ 'touch-action': 'none' }}
        onContextMenu={props.onContextMenu}
        onDragOver={handleDragOver}
        onDrop={handleDrop}
      />
      <GizmoManagerComponent />
      <LoadingTooltip loadingTooltip={loadingTooltip} />
    </>
  );
}
