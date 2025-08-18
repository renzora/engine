import { viewportStore, viewportActions } from "@/layout/stores/ViewportStore";
import { Grid3x3, Cube, Palette, Sun, Pointer, Camera, Rotate360, Eye, Move } from '@/ui/icons';
import { Dynamic } from 'solid-js/web';

export default function CameraDropdownContent() {
  const { setCameraType, setCameraSpeed, setCameraSensitivity, setRenderMode } = viewportActions;
  
  const cameraSpeed = () => viewportStore.camera.speed || 5;
  const mouseSensitivity = () => viewportStore.camera.mouseSensitivity || 0.002;
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
    { value: 5, label: 'Normal' },
    { value: 10, label: 'Fast' },
    { value: 20, label: 'Very Fast' }
  ];

  const cameraTypes = [
    { id: 'universal', label: 'Fly Camera', icon: Move, description: 'WASD + QE flight controls like Unreal Engine' },
    { id: 'arcrotate', label: 'Orbit Camera', icon: Eye, description: 'Orbit around a target point' }
  ];

  return (
    <div class="w-64 space-y-4 p-4">
      <div>
        <label class="block font-medium text-gray-300 mb-2">
          Camera Type
        </label>
        <div class="grid grid-cols-1 gap-1">
          {cameraTypes.map((type) => (
            <button
              onClick={() => setCameraType(type.id)}
              class={`flex items-center gap-2 px-3 py-2 text-xs rounded transition-colors ${
                cameraType() === type.id
                  ? 'bg-blue-600 text-white'
                  : 'bg-gray-700 text-gray-300 hover:bg-gray-600'
              }`}
              title={type.description}
            >
              <Dynamic component={type.icon} class="w-3 h-3" />
              <span>{type.label}</span>
            </button>
          ))}
        </div>
      </div>
      
      <div>
        <label class="block font-medium text-gray-300 mb-2">
          Camera Speed: {cameraSpeed()}
        </label>
        <div class="grid grid-cols-2 gap-1 mb-2">
          {speedPresets.map((preset) => (
            <button
              onClick={() => setCameraSpeed(preset.value)}
              class={`px-2 py-1 text-xs rounded transition-colors ${
                cameraSpeed() === preset.value
                  ? 'bg-blue-600 text-white'
                  : 'bg-gray-700 text-gray-300 hover:bg-gray-600'
              }`}
            >
              {preset.label}
            </button>
          ))}
        </div>
        <input
          type="range"
          min="0.5"
          max="50"
          step="0.5"
          value={cameraSpeed()}
          onInput={(e) => setCameraSpeed(parseFloat(e.target.value))}
          class="w-full h-2 bg-gray-700 rounded-lg appearance-none cursor-pointer slider"
        />
      </div>
      
      <div>
        <label class="block font-medium text-gray-300 mb-2">
          Mouse Sensitivity: {(mouseSensitivity() * 1000).toFixed(1)}
        </label>
        <input
          type="range"
          min="0.001"
          max="0.01"
          step="0.0001"
          value={mouseSensitivity()}
          onInput={(e) => setCameraSensitivity(parseFloat(e.target.value))}
          class="w-full h-2 bg-gray-700 rounded-lg appearance-none cursor-pointer slider"
        />
      </div>
      
      <div>
        <label class="block font-medium text-gray-300 mb-2">
          Render Mode
        </label>
        <div class="grid grid-cols-2 gap-1">
          {renderModes.map((mode) => (
            <button
              onClick={() => setRenderMode(mode.id)}
              class={`flex items-center gap-2 px-2 py-2 text-xs rounded transition-colors ${
                renderMode() === mode.id
                  ? 'bg-blue-600 text-white'
                  : 'bg-gray-700 text-gray-300 hover:bg-gray-600'
              }`}
              title={mode.label}
            >
              <Dynamic component={mode.icon} class="w-3 h-3" />
              <span>{mode.label}</span>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
