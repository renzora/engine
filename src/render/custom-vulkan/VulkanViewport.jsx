import { createSignal, createEffect, onMount, onCleanup } from 'solid-js';

export function VulkanViewport(props) {
  const [canvasRef, setCanvasRef] = createSignal(null);
  const [initialized, setInitialized] = createSignal(false);
  const [tauriInvoke, setTauriInvoke] = createSignal(null);

  // Initialize Vulkan renderer
  const initializeRenderer = async () => {
    try {
      const canvas = canvasRef();
      if (!canvas) return;

      console.log('🔥 Initializing native Vulkan renderer...');
      
      const { invoke } = await import('@tauri-apps/api/core');
      setTauriInvoke(() => invoke);

      const result = await invoke('init_vulkan_renderer');
      console.log('🔥 Vulkan result:', result);
      setInitialized(true);

    } catch (error) {
      console.error('🔥 Failed to initialize Vulkan:', error);
    }
  };

  // Extract and send scene data
  const extractSceneData = () => {
    const invoke = tauriInvoke();
    if (!invoke || !initialized()) return;

    try {
      // Get scene from global state
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

      // Send to Vulkan
      invoke('vulkan_render_frame', JSON.stringify(sceneData))
        .catch(error => console.error('🔥 Vulkan render error:', error));

    } catch (error) {
      console.error('🔥 Scene extraction error:', error);
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
    const invoke = tauriInvoke();
    if (invoke) {
      try {
        await invoke('vulkan_cleanup');
      } catch (error) {
        console.warn('🔥 Cleanup error:', error);
      }
    }
  });

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
          background: '#0a0a0a'
        }}
        onContextMenu={props.onContextMenu}
      />
      
      <div style={{
        position: 'absolute',
        top: '10px',
        left: '10px',
        color: '#ff6b6b',
        'font-family': 'monospace',
        'font-size': '12px',
        'background': 'rgba(0,0,0,0.8)',
        padding: '4px 8px',
        'border-radius': '4px'
      }}>
        🔥 Vulkan {initialized() ? '(Active)' : '(Initializing...)'}
      </div>
    </div>
  );
}

export default VulkanViewport;