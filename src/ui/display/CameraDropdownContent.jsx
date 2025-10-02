import { createSignal, createEffect, For } from 'solid-js';
import { viewportStore, viewportActions } from "@/layout/stores/ViewportStore";
import { IconGrid3x3, IconCube, IconPalette, IconSun, IconPointer, IconCamera, IconRotate360, IconEye, IconArrowsMove } from '@tabler/icons-solidjs';
import { Dynamic } from 'solid-js/web';

export default function CameraDropdownContent() {
  const { setCameraType, setCameraSpeed, setCameraSensitivity, setCameraFriction, setRenderMode } = viewportActions;
  
  // Camera settings
  const cameraSpeed = () => viewportStore.camera.speed || 2;
  const mouseSensitivity = () => viewportStore.camera.mouseSensitivity || 0.004;
  const cameraFriction = () => viewportStore.camera.friction || 2;
  const renderMode = () => viewportStore.renderMode || 'solid';
  const cameraType = () => viewportStore.camera.type || 'universal';
  
  // Vignette and FOV signals
  const [vignetteEnabled, setVignetteEnabled] = createSignal(false);
  const [vignetteAmount, setVignetteAmount] = createSignal(0.5);
  const [vignetteColor, setVignetteColor] = createSignal([0, 0, 0]); // RGB black
  const [cameraFOV, setCameraFOV] = createSignal(60); // Default FOV in degrees
  const [nightColor, setNightColor] = createSignal([0.1, 0.1, 0.15]); // Default night blue tint
  
  // Apply vignette and FOV changes to scene
  createEffect(() => {
    const scene = window._cleanBabylonScene;
    if (!scene || !scene._camera) return;
    
    const camera = scene._camera;
    
    // Update FOV
    camera.fov = (cameraFOV() * Math.PI) / 180; // Convert degrees to radians
    
    // Store camera settings globally
    if (!window._cameraSettings) window._cameraSettings = {};
    window._cameraSettings.nightColor = nightColor();
    window._cameraSettings.vignette = {
      enabled: vignetteEnabled(),
      amount: vignetteAmount(),
      color: vignetteColor()
    };
    
    // Update built-in vignette system
    if (window.updateVignetteSettings) {
      window.updateVignetteSettings();
    }
  });
  
  // Convert RGB array to hex for color input
  const rgbToHex = (rgb) => {
    const r = Math.round(rgb[0] * 255).toString(16).padStart(2, '0');
    const g = Math.round(rgb[1] * 255).toString(16).padStart(2, '0');
    const b = Math.round(rgb[2] * 255).toString(16).padStart(2, '0');
    return `#${r}${g}${b}`;
  };
  
  // Convert hex to RGB array
  const hexToRgb = (hex) => {
    const r = parseInt(hex.slice(1, 3), 16) / 255;
    const g = parseInt(hex.slice(3, 5), 16) / 255;
    const b = parseInt(hex.slice(5, 7), 16) / 255;
    return [r, g, b];
  };
  
  const renderModes = [
    { id: 'wireframe', label: 'Wireframe', icon: IconGrid3x3 },
    { id: 'solid', label: 'Solid', icon: IconCube },
    { id: 'material', label: 'Material', icon: IconPalette },
    { id: 'rendered', label: 'Rendered', icon: IconSun }
  ];
  
  const speedPresets = [
    { value: 1, label: 'Slow' },
    { value: 2, label: 'Normal' },
    { value: 5, label: 'Fast' },
    { value: 10, label: 'Very Fast' }
  ];

  const cameraTypes = [
    { id: 'universal', label: 'Fly Camera', icon: IconArrowsMove, description: 'WASD + QE flight controls like Unreal Engine' },
    { id: 'arcrotate', label: 'Orbit Camera', icon: IconEye, description: 'Orbit around a target point' }
  ];

  const cameraViews = [
    { id: 'front', label: 'Front', shortcut: '1' },
    { id: 'back', label: 'Back', shortcut: 'Ctrl+1' },
    { id: 'right', label: 'Right', shortcut: '3' },
    { id: 'left', label: 'Left', shortcut: 'Ctrl+3' },
    { id: 'top', label: 'Top', shortcut: '7' },
    { id: 'bottom', label: 'Bottom', shortcut: 'Ctrl+7' },
    { id: 'frontLeft', label: 'Front Left', shortcut: '8' },
    { id: 'frontRight', label: 'Front Right', shortcut: '6' }
  ];

  // Camera view functions (copied from Toolbar.jsx)
  const setCameraView = (viewType) => {
    // Import necessary functions
    const getCurrentScene = () => window._cleanBabylonScene;
    const { Vector3 } = window.BABYLON || {};
    
    if (!Vector3) {
      console.error('Babylon.js Vector3 not available');
      return;
    }

    const scene = getCurrentScene();
    if (!scene) {
      console.error('No active scene available');
      return;
    }

    const camera = scene.activeCamera || scene._camera || (scene.cameras && scene.cameras[0]);
    if (!camera) {
      console.error('No active camera available');
      return;
    }

    // Calculate current focus point
    let focusPoint = new Vector3(0, 0, 0);
    let currentDistance = 15;
    
    if (camera.getTarget && typeof camera.getTarget === 'function') {
      focusPoint = camera.getTarget();
      currentDistance = Vector3.Distance(camera.position, focusPoint);
    } else {
      const cameraDirection = camera.getDirection ? camera.getDirection(Vector3.Forward()) : new Vector3(0, 0, 1);
      currentDistance = Vector3.Distance(camera.position, focusPoint);
      focusPoint = camera.position.add(cameraDirection.scale(currentDistance));
    }

    // Define camera positions for each view
    const positions = {
      front: new Vector3(focusPoint.x, focusPoint.y, focusPoint.z + currentDistance),
      back: new Vector3(focusPoint.x, focusPoint.y, focusPoint.z - currentDistance),
      right: new Vector3(focusPoint.x + currentDistance, focusPoint.y, focusPoint.z),
      left: new Vector3(focusPoint.x - currentDistance, focusPoint.y, focusPoint.z),
      top: new Vector3(focusPoint.x, focusPoint.y + currentDistance, focusPoint.z),
      bottom: new Vector3(focusPoint.x, focusPoint.y - currentDistance, focusPoint.z),
      frontRight: new Vector3(focusPoint.x + currentDistance * 0.7, focusPoint.y + currentDistance * 0.5, focusPoint.z + currentDistance * 0.7),
      frontLeft: new Vector3(focusPoint.x - currentDistance * 0.7, focusPoint.y + currentDistance * 0.5, focusPoint.z + currentDistance * 0.7)
    };

    if (positions[viewType]) {
      const newPosition = positions[viewType];
      camera.position = newPosition;
      
      if (camera.setTarget && typeof camera.setTarget === 'function') {
        camera.setTarget(focusPoint);
      } else if (camera.rotation) {
        const direction = focusPoint.subtract(newPosition).normalize();
        camera.rotation.x = Math.asin(-direction.y);
        camera.rotation.y = Math.atan2(direction.x, direction.z);
        camera.rotation.z = 0;
      }

      // Update global state for helper button
      const viewNames = {
        front: "Front", back: "Back", right: "Right", left: "Left",
        top: "Top", bottom: "Bottom", frontLeft: "Front Left", frontRight: "Front Right"
      };
      const viewName = viewNames[viewType] || "Camera";
      window._currentCameraViewName = viewName;

      // Close dropdown by calling global close function
      if (window._closeHelperDropdowns) {
        window._closeHelperDropdowns();
      }
    }
  };

  return (
    <div class="w-64 space-y-4 p-4 bg-base-200 text-base-content max-h-96 overflow-y-auto">
      <div>
        <label class="block font-medium text-base-content mb-2">
          Camera Views
        </label>
        <div class="grid grid-cols-2 gap-1 mb-4">
          <For each={cameraViews}>
            {(view) => (
              <button
                onClick={() => setCameraView(view.id)}
                class="btn btn-xs btn-ghost flex items-center justify-between text-left"
                title={`${view.label} (${view.shortcut})`}
              >
                <span>{view.label}</span>
                <span class="text-xs text-base-content/60">{view.shortcut}</span>
              </button>
            )}
          </For>
        </div>
      </div>
      
      <div>
        <label class="block font-medium text-base-content mb-2">
          Camera Type
        </label>
        <div class="grid grid-cols-1 gap-1">
          <For each={cameraTypes}>
            {(type) => (
              <button
                onClick={() => setCameraType(type.id)}
                class={`btn btn-sm flex items-center gap-2 justify-start ${
                  cameraType() === type.id
                    ? 'btn-primary'
                    : 'btn-ghost'
                }`}
                title={type.description}
              >
                <Dynamic component={type.icon} class="w-3 h-3" />
                <span>{type.label}</span>
              </button>
            )}
          </For>
        </div>
      </div>
      
      <div>
        <label class="block font-medium text-base-content mb-2">
          Camera Speed: {cameraSpeed()}
        </label>
        <div class="grid grid-cols-2 gap-1 mb-2">
          <For each={speedPresets}>
            {(preset) => (
              <button
                onClick={() => setCameraSpeed(preset.value)}
                class={`btn btn-xs ${
                  cameraSpeed() === preset.value
                    ? 'btn-primary'
                    : 'btn-ghost'
                }`}
              >
                {preset.label}
              </button>
            )}
          </For>
        </div>
        <input
          type="range"
          min="0.1"
          max="10"
          step="0.1"
          value={cameraSpeed()}
          onInput={(e) => setCameraSpeed(parseFloat(e.target.value))}
          class="range range-primary w-full"
        />
      </div>
      
      <div>
        <label class="block font-medium text-base-content mb-2">
          Mouse Sensitivity: {(mouseSensitivity() * 1000).toFixed(1)}
        </label>
        <input
          type="range"
          min="0.001"
          max="0.01"
          step="0.0001"
          value={mouseSensitivity()}
          onInput={(e) => setCameraSensitivity(parseFloat(e.target.value))}
          class="range range-primary w-full"
        />
      </div>
      
      <div>
        <label class="block font-medium text-base-content mb-2">
          Movement Momentum: {cameraFriction()}
        </label>
        <input
          type="range"
          min="1"
          max="5"
          step="1"
          value={cameraFriction()}
          onInput={(e) => setCameraFriction(parseInt(e.target.value))}
          class="range range-primary w-full"
        />
        <div class="flex justify-between text-xs text-base-content/60 mt-1">
          <span>Quick Stop</span>
          <span>Smooth Drift</span>
        </div>
      </div>
      
      <div>
        <label class="block font-medium text-base-content mb-2">
          Render Mode
        </label>
        <div class="grid grid-cols-2 gap-1">
          <For each={renderModes}>
            {(mode) => (
              <button
                onClick={() => setRenderMode(mode.id)}
                class={`btn btn-xs flex items-center gap-2 justify-start ${
                  renderMode() === mode.id
                    ? 'btn-primary'
                    : 'btn-ghost'
                }`}
                title={mode.label}
              >
                <Dynamic component={mode.icon} class="w-3 h-3" />
                <span>{mode.label}</span>
              </button>
            )}
          </For>
        </div>
      </div>
      
      <div>
        <label class="block font-medium text-base-content mb-2">
          Field of View: {cameraFOV()}°
        </label>
        <input
          type="range"
          min="30"
          max="120"
          step="1"
          value={cameraFOV()}
          onInput={(e) => setCameraFOV(parseInt(e.target.value))}
          class="range range-primary w-full"
        />
        <div class="flex justify-between text-xs text-base-content/60 mt-1">
          <span>30°</span>
          <span>75°</span>
          <span>120°</span>
        </div>
      </div>
      
      <div>
        <div class="flex items-center justify-between mb-2">
          <label class="block font-medium text-base-content">
            Vignette
          </label>
          <input
            type="checkbox"
            checked={vignetteEnabled()}
            onChange={(e) => setVignetteEnabled(e.target.checked)}
            class="toggle toggle-sm toggle-primary"
          />
        </div>
        
        {vignetteEnabled() && (
          <div class="space-y-2">
            <div>
              <label class="block text-sm text-base-content/80 mb-1">
                Amount: {vignetteAmount().toFixed(2)}
              </label>
              <input
                type="range"
                min="0"
                max="1"
                step="0.01"
                value={vignetteAmount()}
                onInput={(e) => setVignetteAmount(parseFloat(e.target.value))}
                class="range range-primary w-full"
              />
            </div>
            
            <div>
              <label class="block text-sm text-base-content/80 mb-1">Color</label>
              <input
                type="color"
                value={rgbToHex(vignetteColor())}
                onInput={(e) => setVignetteColor(hexToRgb(e.target.value))}
                class="w-full h-8 rounded border border-base-300"
              />
            </div>
          </div>
        )}
      </div>
      
      <div>
        <label class="block font-medium text-base-content mb-2">
          Night Color Tint
        </label>
        <input
          type="color"
          value={rgbToHex(nightColor())}
          onInput={(e) => setNightColor(hexToRgb(e.target.value))}
          class="w-full h-8 rounded border border-base-300"
        />
        <div class="text-xs text-base-content/60 mt-1">
          Affects overall night lighting tint
        </div>
      </div>
    </div>
  );
}
