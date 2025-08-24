import { createSignal, createEffect, onMount, onCleanup } from 'solid-js';

export const BabylonNativeViewport = (props) => {
  const [canvasRef, setCanvasRef] = createSignal(null);
  const [initialized, setInitialized] = createSignal(false);
  const [tauriInvoke, setTauriInvoke] = createSignal(null);
  const [babylonEngine, setBabylonEngine] = createSignal(null);
  const [babylonScene, setBabylonScene] = createSignal(null);
  const [frameCount, setFrameCount] = createSignal(0);
  const [animationEnabled, setAnimationEnabled] = createSignal(true);
  const [cameraAngle, setCameraAngle] = createSignal(0);
  const [cameraDistance, setCameraDistance] = createSignal(10);
  const [objectPosition, setObjectPosition] = createSignal([0, 0, 0]);

  // Initialize Babylon Native renderer with actual Babylon.js rendering
  const initializeRenderer = async () => {
    try {
      const canvas = canvasRef();
      if (!canvas) return;

      console.log('🏛️ Initializing Babylon Native renderer with actual rendering...');
      
      // Import Babylon.js for actual rendering
      const BABYLON = await import('@babylonjs/core');
      
      // Create high-performance engine
      const engine = new BABYLON.Engine(canvas, true, {
        antialias: true,
        powerPreference: 'high-performance',
        adaptToDeviceRatio: true
      });
      
      // Create scene
      const scene = new BABYLON.Scene(engine);
      scene.clearColor = new BABYLON.Color3(0.1, 0.1, 0.15);
      
      // Add default camera
      const camera = new BABYLON.ArcRotateCamera("camera", -Math.PI/2, Math.PI/2.5, 10, BABYLON.Vector3.Zero(), scene);
      camera.setTarget(BABYLON.Vector3.Zero());
      scene.setActiveCameraByName("camera");
      
      // Add default lighting
      const light = new BABYLON.HemisphericLight("light", new BABYLON.Vector3(0, 1, 0), scene);
      light.intensity = 0.7;
      
      // Add default cube
      const box = BABYLON.MeshBuilder.CreateBox("box", {size: 2}, scene);
      const material = new BABYLON.StandardMaterial("boxMaterial", scene);
      material.diffuseColor = new BABYLON.Color3(1, 0.3, 0.3);
      box.material = material;
      
      // Store globally for scene data extraction
      window.globalSceneInstance = () => scene;
      
      // Start render loop
      engine.runRenderLoop(() => {
        scene.render();
      });
      
      setBabylonEngine(engine);
      setBabylonScene(scene);
      
      // Initialize Tauri bridge
      const { invoke } = await import('@tauri-apps/api/core');
      setTauriInvoke(() => invoke);

      // Initialize both Babylon.js rendering and native C++ backend
      const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow');
      const currentWindow = getCurrentWebviewWindow();
      
      const nativeResult = await invoke('init_native_cpp_renderer', {
        window: currentWindow,
        width: canvas.width || 800,
        height: canvas.height || 600
      });

      const babylonResult = await invoke('init_babylon_native', {
        width: canvas.width || 800,
        height: canvas.height || 600
      });

      console.log('🏛️ Native C++ result:', nativeResult);
      console.log('🏛️ Babylon Native result:', babylonResult);
      setInitialized(true);

    } catch (error) {
      console.error('🏛️ Failed to initialize Babylon Native:', error);
    }
  };

  // Extract and send scene data
  const extractSceneData = () => {
    const invoke = tauriInvoke();
    if (!invoke || !initialized()) return;

    try {
      // Get scene from global state (same as VulkanViewport)
      const sceneInstance = window.globalSceneInstance;
      if (!sceneInstance) return null;

      const scene = sceneInstance();
      if (!scene) return null;

      // Extract camera data
      const camera = scene.activeCamera;
      const cameraData = {
        position: camera ? [camera.position.x, camera.position.y, camera.position.z] : [0, 0, 10],
        target: camera && camera.getTarget ? [camera.getTarget().x, camera.getTarget().y, camera.getTarget().z] : [0, 0, 0],
        fov: camera ? camera.fov : 0.8,
        near: camera ? camera.minZ : 0.1,
        far: camera ? camera.maxZ : 100
      };

      // Extract objects
      const objects = scene.meshes.map(mesh => ({
        name: mesh.name || 'unnamed',
        position: [mesh.position.x, mesh.position.y, mesh.position.z],
        rotation: [mesh.rotation.x, mesh.rotation.y, mesh.rotation.z],
        scaling: [mesh.scaling.x, mesh.scaling.y, mesh.scaling.z],
        is_visible: mesh.isEnabled() && mesh.isVisible,
        material: mesh.material ? {
          diffuse_color: mesh.material.diffuseColor ? 
            [mesh.material.diffuseColor.r, mesh.material.diffuseColor.g, mesh.material.diffuseColor.b] : 
            [1.0, 1.0, 1.0]
        } : null
      }));

      // Extract lights
      const lights = scene.lights.map(light => ({
        name: light.name || 'unnamed',
        light_type: light.getClassName ? light.getClassName().toLowerCase() : 'unknown',
        position: light.position ? [light.position.x, light.position.y, light.position.z] : null,
        direction: light.direction ? [light.direction.x, light.direction.y, light.direction.z] : null,
        intensity: light.intensity || 1.0,
        diffuse: light.diffuse ? [light.diffuse.r, light.diffuse.g, light.diffuse.b] : [1.0, 1.0, 1.0]
      }));

      const sceneData = {
        camera: cameraData,
        objects: objects,
        lights: lights,
        timestamp: performance.now()
      };

      // Send to both Babylon Native and native C++ renderer
      invoke('babylon_native_render', { sceneData: JSON.stringify(sceneData) })
        .catch(error => console.error('🏛️ Babylon Native render error:', error));
        
      invoke('native_cpp_render_frame', { sceneData: JSON.stringify(sceneData) })
        .then(result => {
          // Update frame count from C++ renderer
          const match = result.match(/Frame (\d+)/);
          if (match) {
            setFrameCount(parseInt(match[1]));
          }
        })
        .catch(error => console.error('🏛️ Native C++ render error:', error));

    } catch (error) {
      console.error('🏛️ Scene extraction error:', error);
    }
  };

  // Set up render loop
  createEffect(() => {
    if (!initialized()) return;

    const renderLoop = () => {
      extractSceneData();
      requestAnimationFrame(renderLoop);
    };

    const frameId = requestAnimationFrame(renderLoop);

    onCleanup(() => {
      cancelAnimationFrame(frameId);
    });
  });

  // Handle canvas mounting
  onMount(() => {
    const canvas = canvasRef();
    if (canvas) {
      initializeRenderer();
    }
  });

  // Cleanup on unmount
  onCleanup(async () => {
    const engine = babylonEngine();
    if (engine) {
      engine.dispose();
    }
    
    const invoke = tauriInvoke();
    if (invoke) {
      try {
        await invoke('babylon_native_cleanup');
      } catch (error) {
        console.warn('🏛️ Cleanup error:', error);
      }
    }
  });

  // Animation and camera controls
  const toggleAnimation = async () => {
    const invoke = tauriInvoke();
    if (!invoke) return;
    
    try {
      await invoke('native_cpp_toggle_animation');
      setAnimationEnabled(!animationEnabled());
    } catch (error) {
      console.error('Failed to toggle animation:', error);
    }
  };

  const updateCameraOrbit = async (angle, distance, height = 5) => {
    const invoke = tauriInvoke();
    if (!invoke) return;
    
    try {
      await invoke('native_cpp_set_camera_orbit', { angle, distance, height });
      setCameraAngle(angle);
      setCameraDistance(distance);
    } catch (error) {
      console.error('Failed to update camera:', error);
    }
  };

  const moveObject = async (index, x, y, z) => {
    const invoke = tauriInvoke();
    if (!invoke) return;
    
    try {
      await invoke('native_cpp_move_object', { index, x, y, z });
      setObjectPosition([x, y, z]);
    } catch (error) {
      console.error('Failed to move object:', error);
    }
  };

  return (
    <div style={props.style} class="relative w-full h-full">
      <canvas
        ref={setCanvasRef}
        width="800"
        height="600"
        style={{
          width: '100%',
          height: '100%',
          display: 'block',
          background: '#1a1a1a'
        }}
        onContextMenu={props.onContextMenu}
      />
      
      <div style={{
        position: 'absolute',
        top: '10px',
        left: '10px',
        color: '#ffffff',
        'font-family': 'monospace',
        'font-size': '12px',
        'background': 'rgba(0,0,0,0.7)',
        padding: '4px 8px',
        'border-radius': '4px'
      }}>
        🏛️ Babylon Native + C++ {initialized() ? `(Frame ${frameCount()})` : '(Initializing...)'}
      </div>
      
      {/* Animation Controls */}
      <div style={{
        position: 'absolute',
        top: '10px',
        right: '10px',
        'background': 'rgba(0,0,0,0.8)',
        padding: '12px',
        'border-radius': '8px',
        color: '#ffffff',
        'font-family': 'sans-serif',
        'font-size': '12px',
        'min-width': '200px'
      }}>
        <div style={{ 'margin-bottom': '8px', 'font-weight': 'bold' }}>C++ DirectX Controls</div>
        
        <div style={{ 'margin-bottom': '8px' }}>
          <button 
            onClick={toggleAnimation}
            style={{
              padding: '4px 8px',
              'background': animationEnabled() ? '#22c55e' : '#ef4444',
              color: 'white',
              border: 'none',
              'border-radius': '4px',
              cursor: 'pointer',
              'font-size': '11px'
            }}
          >
            {animationEnabled() ? '⏸️ Pause' : '▶️ Play'} Animation
          </button>
        </div>
        
        <div style={{ 'margin-bottom': '8px' }}>
          <label>Camera Angle: {cameraAngle().toFixed(1)}°</label>
          <input 
            type="range" 
            min="0" 
            max="360" 
            value={cameraAngle()}
            onInput={(e) => updateCameraOrbit(parseFloat(e.target.value), cameraDistance())}
            style={{ width: '100%', 'margin-top': '2px' }}
          />
        </div>
        
        <div style={{ 'margin-bottom': '8px' }}>
          <label>Camera Distance: {cameraDistance().toFixed(1)}</label>
          <input 
            type="range" 
            min="5" 
            max="20" 
            step="0.5"
            value={cameraDistance()}
            onInput={(e) => updateCameraOrbit(cameraAngle(), parseFloat(e.target.value))}
            style={{ width: '100%', 'margin-top': '2px' }}
          />
        </div>
        
        <div>
          <label>Object Position</label>
          <div style={{ display: 'flex', gap: '4px', 'margin-top': '2px' }}>
            <input 
              type="range" 
              min="-5" 
              max="5" 
              step="0.1"
              value={objectPosition()[0]}
              onInput={(e) => moveObject(0, parseFloat(e.target.value), objectPosition()[1], objectPosition()[2])}
              style={{ flex: 1 }}
              placeholder="X"
            />
            <input 
              type="range" 
              min="-5" 
              max="5" 
              step="0.1"
              value={objectPosition()[1]}
              onInput={(e) => moveObject(0, objectPosition()[0], parseFloat(e.target.value), objectPosition()[2])}
              style={{ flex: 1 }}
              placeholder="Y"
            />
            <input 
              type="range" 
              min="-5" 
              max="5" 
              step="0.1"
              value={objectPosition()[2]}
              onInput={(e) => moveObject(0, objectPosition()[0], objectPosition()[1], parseFloat(e.target.value))}
              style={{ flex: 1 }}
              placeholder="Z"
            />
          </div>
        </div>
      </div>
    </div>
  );
};