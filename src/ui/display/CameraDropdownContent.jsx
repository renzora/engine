import { viewportStore, viewportActions } from "@/layout/stores/ViewportStore";
import { Grid3x3, Cube, Palette, Sun, Pointer, Camera, Rotate360, Eye, Move } from '@/ui/icons';
import { Dynamic } from 'solid-js/web';

export default function CameraDropdownContent() {
  const { setCameraType, setCameraSpeed, setCameraSensitivity, setCameraFriction, setRenderMode } = viewportActions;
  
  const cameraSpeed = () => viewportStore.camera.speed || 2;
  const mouseSensitivity = () => viewportStore.camera.mouseSensitivity || 0.004;
  const cameraFriction = () => viewportStore.camera.friction || 2;
  const renderMode = () => viewportStore.renderMode || 'solid';
  const cameraType = () => viewportStore.camera.type || 'universal';
  
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
    <div class="w-64 space-y-4 p-4 bg-base-200 text-base-content">
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
    </div>
  );
}
