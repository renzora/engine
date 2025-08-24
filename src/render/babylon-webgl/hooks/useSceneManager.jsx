import { createSignal, createEffect } from 'solid-js'
import { Scene } from '@babylonjs/core/scene'
import { Color3 } from '@babylonjs/core/Maths/math.color'
import { Vector3 } from '@babylonjs/core/Maths/math.vector'
import { UniversalCamera } from '@babylonjs/core/Cameras/universalCamera'
import { ArcRotateCamera } from '@babylonjs/core/Cameras/arcRotateCamera'
import { MeshBuilder } from '@babylonjs/core/Meshes/meshBuilder'
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial'
import { DirectionalLight } from '@babylonjs/core/Lights/directionalLight'
import { HemisphericLight } from '@babylonjs/core/Lights/hemisphericLight'
import '@babylonjs/core/Layers/effectLayerSceneComponent'
import '@babylonjs/core/Meshes/Builders/sphereBuilder'
import '@babylonjs/core/Meshes/Builders/groundBuilder'
import { viewportStore } from '@/layout/stores/ViewportStore'
import { initializeScriptRuntime } from '@/api/script'
window._cleanBabylonScene = null;

export const useSceneManager = () => {
  const [sceneInstance, setSceneInstance] = createSignal(null)
  
  const createCameraByType = (type, scene) => {
    let camera;
    
    if (type === 'arcrotate') {
      // Orbit camera that rotates around a target
      camera = new ArcRotateCamera(
        "camera",
        Math.PI / 2,  // alpha (horizontal rotation)
        Math.PI / 3,  // beta (vertical rotation) 
        10,           // radius (distance from target)
        Vector3.Zero(), // target
        scene
      );
      camera.setTarget(Vector3.Zero());
      camera.wheelPrecision = 50;
      camera.pinchPrecision = 100;
    } else {
      // Universal camera for FPS-style fly controls (default)
      camera = new UniversalCamera(
        "camera",
        new Vector3(5, 3, -10),  // Starting position
        scene
      );
      camera.setTarget(Vector3.Zero());
      
      // Game development optimized settings for smooth flying
      camera.speed = 0.5;                // Base movement speed
      camera.angularSensibility = 2000;  // Mouse look sensitivity  
      camera.inertia = 0.8;             // Movement smoothness
    }
    
    camera.fov = Math.PI / 3;         // Field of view
    return camera;
  };
  
  const createScene = async (engine) => {
    // CLEAN SLATE - Basic scene with only essentials
    const scene = new Scene(engine)
    scene.clearColor = new Color3(0.05, 0.05, 0.05) // Dark background
    console.log('🎬 Clean Scene: Created fresh scene')

    // Create default camera - this will be the actual scene camera
    const camera = createCameraByType('universal', scene);
    camera.name = "MainCamera"; // Give it a proper name for identification
    scene._camera = camera
    
    // Ensure the camera is in the scene's cameras array and active
    if (!scene.cameras.includes(camera)) {
      scene.addCamera(camera);
    }
    scene.activeCamera = camera
    
    // Basic lighting
    const sunLight = new DirectionalLight("sunLight", new Vector3(-1, -1, -1), scene)
    sunLight.diffuse = new Color3(1, 0.95, 0.8)
    sunLight.intensity = 2
    
    const ambientLight = new HemisphericLight("ambientLight", new Vector3(0, 1, 0), scene)
    ambientLight.diffuse = new Color3(0.4, 0.6, 1)
    ambientLight.intensity = 0.3
    
    // ONLY THE HARDCODED CUBE - nothing else
    console.log('🧪 Creating ONLY hardcoded test cube...');
    const testCube = MeshBuilder.CreateBox("hardcoded_test_cube", { size: 3 }, scene);
    testCube.position = new Vector3(0, 1.5, 0);
    
    const testMaterial = new StandardMaterial("hardcoded_test_material", scene);
    testMaterial.diffuseColor = new Color3(1, 0, 0); // Bright red
    testMaterial.emissiveColor = new Color3(0.5, 0, 0); // Red glow
    testMaterial.backFaceCulling = false;
    testCube.material = testMaterial;
    
    console.log('🧪 Hardcoded cube created in clean scene:', {
      name: testCube.name,
      position: testCube.position,
      meshCount: scene.meshes.length
    });
    
    setSceneInstance(scene)
    // CLEAN SCENE: Set global scene reference
    window._cleanBabylonScene = scene;
    
    // Initialize script runtime
    try {
      const runtime = initializeScriptRuntime(scene);
      console.log('🔧 Script runtime initialized successfully');
      console.log('🔧 Script runtime stats:', runtime.getStats());
      
      // Make runtime globally accessible for debugging
      window._scriptRuntime = runtime;
    } catch (error) {
      console.error('🔧 Failed to initialize script runtime:', error);
    }
    
    console.log('✅ Clean scene created with only hardcoded cube')
    
    return scene
  }

  // Function to ensure at least one camera exists
  const ensureActiveCamera = (scene) => {
    if (!scene) return;
    
    // Check if there are any cameras in the scene
    if (scene.cameras.length === 0) {
      console.warn('⚠️ No cameras in scene! Creating default camera...');
      const defaultCamera = createCameraByType('universal', scene);
      defaultCamera.name = "DefaultCamera";
      scene.addCamera(defaultCamera);
      scene.activeCamera = defaultCamera;
      scene._camera = defaultCamera;
      
      // Dispatch event for UI update
      window.dispatchEvent(new CustomEvent('babylonSceneChanged', { 
        detail: { type: 'camera-created' } 
      }));
    } else if (!scene.activeCamera) {
      // If there are cameras but no active one, activate the first
      scene.activeCamera = scene.cameras[0];
      scene._camera = scene.cameras[0];
    }
  };

  const switchCameraType = (newType) => {
    const scene = sceneInstance();
    if (!scene) return;

    // Store current camera position and target for smooth transition
    const currentCamera = scene._camera;
    const currentPosition = currentCamera.position.clone();
    const currentTarget = currentCamera.getTarget ? currentCamera.getTarget() : Vector3.Zero();

    // Dispose current camera
    if (currentCamera) {
      currentCamera.dispose();
    }

    // Create new camera of requested type
    const newCamera = createCameraByType(newType, scene);
    
    // Try to preserve position when switching
    if (newType === 'arcrotate') {
      // For orbit camera, calculate alpha/beta from current position
      const distance = Vector3.Distance(currentPosition, currentTarget);
      newCamera.setTarget(currentTarget);
      newCamera.radius = Math.max(distance, 3); // Minimum distance of 3 units
      
      // Calculate spherical coordinates
      const direction = currentPosition.subtract(currentTarget).normalize();
      const alpha = Math.atan2(direction.x, direction.z);
      const beta = Math.acos(direction.y);
      
      newCamera.alpha = alpha;
      newCamera.beta = beta;
    } else {
      // For universal camera, just use the position directly
      newCamera.position = currentPosition;
      newCamera.setTarget(currentTarget);
    }

    // Attach controls to canvas if available
    const canvas = scene.getEngine().getRenderingCanvas();
    if (canvas) {
      try {
        if (typeof newCamera.attachControls === 'function') {
          newCamera.attachControls(canvas);
        }
      } catch (error) {
        console.warn('Could not attach camera controls:', error);
      }
    }

    scene._camera = newCamera;
    console.log(`🎥 Camera switched to ${newType}:`, newCamera.constructor.name);
  };

  // Watch for camera type changes in viewport store
  createEffect(() => {
    const scene = sceneInstance();
    const currentType = viewportStore.camera.type;
    
    if (scene && scene._camera) {
      const currentCameraType = scene._camera instanceof ArcRotateCamera ? 'arcrotate' : 'universal';
      
      if (currentType !== currentCameraType) {
        console.log(`🔄 Camera type change detected: ${currentCameraType} → ${currentType}`);
        switchCameraType(currentType);
      }
    }
  });
  
  const disposeScene = () => {
    const scene = sceneInstance()
    if (scene && !scene.isDisposed) {
      try {
        scene.dispose()
      } catch (e) {
        console.warn('Error disposing scene:', e)
      }
    }
    setSceneInstance(null)
  }
  
  // Switch to a specific camera by name or index
  const switchToCamera = (cameraIdentifier) => {
    const scene = sceneInstance();
    if (!scene) return false;
    
    let targetCamera = null;
    
    if (typeof cameraIdentifier === 'string') {
      // Find by name
      targetCamera = scene.cameras.find(cam => cam.name === cameraIdentifier);
    } else if (typeof cameraIdentifier === 'number') {
      // Find by index
      targetCamera = scene.cameras[cameraIdentifier];
    }
    
    if (targetCamera) {
      scene.activeCamera = targetCamera;
      scene._camera = targetCamera;
      targetCamera.attachControl(scene.getEngine().getRenderingCanvas(), true);
      console.log('📷 Switched to camera:', targetCamera.name);
      return true;
    }
    
    console.warn('📷 Camera not found:', cameraIdentifier);
    return false;
  };
  
  // Get list of all cameras
  const getCameraList = () => {
    const scene = sceneInstance();
    if (!scene) return [];
    
    return scene.cameras.map((cam, index) => ({
      name: cam.name,
      index: index,
      isActive: cam === scene.activeCamera,
      type: cam.getClassName()
    }));
  };
  
  return {
    sceneInstance,
    createScene,
    disposeScene,
    ensureActiveCamera,
    switchToCamera,
    getCameraList
  }
}