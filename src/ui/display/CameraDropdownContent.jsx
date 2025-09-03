import { createSignal, createEffect } from 'solid-js';
import { viewportStore, viewportActions } from "@/layout/stores/ViewportStore";
import { IconGrid3x3, IconCube, IconPalette, IconSun, IconPointer, IconCamera, IconRotate360, IconEye, IconMove } from '@tabler/icons-solidjs';
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
    { id: 'wireframe', label: 'Wireframe', icon: Grid3x3 },
    { id: 'solid', label: 'Solid', icon: Cube },
    { id: 'material', label: 'Material', icon: Palette },
    { id: 'rendered', label: 'Rendered', icon: Sun }
  ];
  
  const speedPresets = [
    { value: 1, label: 'Slow' },
    { value: 2, label: 'Normal' },
    { value: 5, label: 'Fast' },
    { value: 10, label: 'Very Fast' }
  ];

  const cameraTypes = [
    { id: 'universal', label: 'Fly Camera', icon: Move, description: 'WASD + QE flight controls like Unreal Engine' },
    { id: 'arcrotate', label: 'Orbit Camera', icon: Eye, description: 'Orbit around a target point' }
  ];

  return (
    <div class="w-64 space-y-4 p-4 bg-base-200 text-base-content max-h-96 overflow-y-auto">
      <div>
        <label class="block font-medium text-base-content mb-2">
          Camera Type
        </label>
        <div class="grid grid-cols-1 gap-1">
          {cameraTypes.map((type) => (
            <button
              onClick={() => setCameraType(type.id)}
              class={`btn btn-sm flex items-center gap-2 justify-start ${
                cameraType() === type.id
                  ? 'btn-primary'
                  : 'btn-ghost'
              }`}
              title={type.description}
            >
              <type.icon class="w-3 h-3" />
              <span>{type.label}</span>
            </button>
          ))}
        </div>
      </div>
      
      <div>
        <label class="block font-medium text-base-content mb-2">
          Camera Speed: {cameraSpeed()}
        </label>
        <div class="grid grid-cols-2 gap-1 mb-2">
          {speedPresets.map((preset) => (
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
          ))}
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
          {renderModes.map((mode) => (
            <button
              onClick={() => setRenderMode(mode.id)}
              class={`btn btn-xs flex items-center gap-2 justify-start ${
                renderMode() === mode.id
                  ? 'btn-primary'
                  : 'btn-ghost'
              }`}
              title={mode.label}
            >
              <mode.icon class="w-3 h-3" />
              <span>{mode.label}</span>
            </button>
          ))}
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
