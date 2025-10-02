import { createSignal, createEffect, onCleanup } from 'solid-js';
import { renderStore } from '@/render/store.jsx';
import { IconVideo } from '@tabler/icons-solidjs';

function CameraNameDisplay() {
  const [cameraName, setCameraName] = createSignal('No Camera');

  // Update camera name when scene or camera changes
  createEffect(() => {
    const updateCameraName = () => {
      const scene = renderStore.scene;
      if (!scene) {
        setCameraName('No Scene');
        return;
      }

      const activeCamera = scene.activeCamera;
      if (!activeCamera) {
        setCameraName('No Camera');
        return;
      }

      // Get camera name, fallback to ID or type
      const name = activeCamera.name || 
                  activeCamera.id || 
                  activeCamera.getClassName?.() || 
                  'Camera';
      
      setCameraName(name);
    };

    // Initial update
    updateCameraName();

    // Watch for scene changes
    if (renderStore.scene) {
      const scene = renderStore.scene;
      
      // Listen for camera changes (when activeCamera is set)
      const interval = setInterval(updateCameraName, 500); // Check every 500ms
      
      onCleanup(() => {
        clearInterval(interval);
      });
    }
  });

  return (
    <div class="flex items-center gap-1.5 px-2 py-1 text-xs text-base-content/70 bg-base-200/50 rounded border border-base-300/50">
      <IconVideo class="w-3 h-3" />
      <span class="font-medium truncate max-w-24" title={cameraName()}>
        {cameraName()}
      </span>
    </div>
  );
}

export default CameraNameDisplay;