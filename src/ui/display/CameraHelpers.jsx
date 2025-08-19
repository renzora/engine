import { createSignal, createEffect, onCleanup } from 'solid-js';
import { IconGrid3x3, IconCube, IconPalette, IconSun, IconVideo } from '@tabler/icons-solidjs';
import { editorStore, editorActions } from "@/layout/stores/EditorStore";
import { viewportStore, viewportActions } from "@/layout/stores/ViewportStore";

export default function CameraHelpers() {
  const [isExpanded, setIsExpanded] = createSignal(false);
  let cameraRef;
  const store = editorStore;
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

  createEffect(() => {
    const handleClickOutside = (event) => {
      if (cameraRef && !cameraRef.contains(event.target)) {
        setIsExpanded(false);
      }
    };

    if (isExpanded()) {
      document.addEventListener('mousedown', handleClickOutside);
      onCleanup(() => {
        document.removeEventListener('mousedown', handleClickOutside);
      });
    }
  });

  return (
    <div class="relative group" ref={cameraRef}>
      <button
        class={`pl-2 pr-1 py-1 text-xs rounded transition-colors cursor-pointer ${
          isExpanded()
            ? 'bg-primary text-primary-content'
            : 'text-base-content/60 hover:text-base-content hover:bg-base-300'
        }`}
        onClick={() => setIsExpanded(!isExpanded())}
        title="Camera Settings"
      >
        <div class="flex items-center gap-0.5">
          <IconVideo class="w-4 h-4" />
          <svg class="w-2 h-2" fill="currentColor" viewBox="0 0 20 20">
            <path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd" />
          </svg>
        </div>
        
        <div class="absolute right-full mr-2 top-1/2 transform -translate-y-1/2 bg-base-300/95 text-base-content text-xs px-2 py-1 rounded whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50">
          Camera Settings
          <div class="absolute left-full top-1/2 transform -translate-y-1/2 border-4 border-transparent border-l-base-300/95" />
        </div>
      </button>
      
      {isExpanded() && (
        <div class="absolute top-full right-0 mt-2 w-64 bg-base-300/95 backdrop-blur-sm border border-base-300 rounded-lg shadow-xl space-y-4 text-base-content text-xs pointer-events-auto z-50 p-4">
          <div>
            <label class="block font-medium text-base-content/80 mb-2">
              Camera Speed: {cameraSpeed()}
            </label>
            <div class="grid grid-cols-2 gap-1 mb-2">
              {speedPresets.map((preset) => (
                <button
                  onClick={() => setCameraSpeed(preset.value)}
                  class={`px-2 py-1 text-xs rounded transition-colors ${
                    cameraSpeed() === preset.value
                      ? 'bg-primary text-primary-content'
                      : 'bg-base-200 text-base-content/70 hover:bg-base-300'
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
              class="w-full h-2 bg-base-200 rounded-lg appearance-none cursor-pointer slider"
            />
          </div>
          
          <div>
            <label class="block font-medium text-base-content/80 mb-2">
              Mouse Sensitivity: {(mouseSensitivity() * 1000).toFixed(1)}
            </label>
            <input
              type="range"
              min="0.001"
              max="0.01"
              step="0.0001"
              value={mouseSensitivity()}
              onInput={(e) => setCameraSensitivity(parseFloat(e.target.value))}
              class="w-full h-2 bg-base-200 rounded-lg appearance-none cursor-pointer slider"
            />
          </div>
          
          <div>
            <label class="block font-medium text-base-content/80 mb-2">
              Render Mode
            </label>
            <div class="grid grid-cols-2 gap-1">
              {renderModes.map((mode) => (
                <button
                  onClick={() => setRenderMode(mode.id)}
                  class={`flex items-center gap-2 px-2 py-2 text-xs rounded transition-colors ${
                    renderMode() === mode.id
                      ? 'bg-primary text-primary-content'
                      : 'bg-base-200 text-base-content/70 hover:bg-base-300'
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
      )}
    </div>
  );
}
