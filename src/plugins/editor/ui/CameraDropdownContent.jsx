import { editorStore, editorActions } from "@/plugins/editor/stores/EditorStore";
import { viewportStore, viewportActions } from "@/plugins/editor/stores/ViewportStore";
import { IconGrid3x3, IconCube, IconPalette, IconSun } from '@tabler/icons-solidjs';

export default function CameraDropdownContent() {
  const { setCameraSpeed, setCameraSensitivity, setRenderMode } = editorActions;
  
  const cameraSpeed = () => viewportStore.camera.speed || 5;
  const mouseSensitivity = () => viewportStore.camera.mouseSensitivity || 0.002;
  const renderMode = () => viewportStore.renderMode || 'solid';
  
  const renderModes = [
    { id: 'wireframe', label: 'Wireframe', icon: IconGrid3x3 },
    { id: 'solid', label: 'Solid', icon: IconCube },
    { id: 'material', label: 'Material', icon: IconPalette },
    { id: 'rendered', label: 'Rendered', icon: IconSun }
  ];
  
  const speedPresets = [
    { value: 1, label: 'Slow' },
    { value: 5, label: 'Normal' },
    { value: 10, label: 'Fast' },
    { value: 20, label: 'Very Fast' }
  ];

  return (
    <div class="w-64 space-y-4 p-4">
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
              <mode.icon class="w-3 h-3" />
              <span>{mode.label}</span>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}