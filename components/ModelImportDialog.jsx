import { createSignal, Show, For } from 'solid-js';
import { IconSparkles, IconGrid3x3, IconList, IconBox } from '@tabler/icons-solidjs';
import { IconX, IconAlertTriangle, IconChevronRight } from '@tabler/icons-solidjs';

const ModelImportDialog = ({ 
  isOpen, 
  onClose, 
  onImport, 
  modelName, 
  modelAnalysis = null 
}) => {
  const [importMode, setImportMode] = createSignal('smart');
  const [maxObjects, setMaxObjects] = createSignal(50);
  const [hierarchyDepth, setHierarchyDepth] = createSignal(3);
  const [importMaterials, setImportMaterials] = createSignal(true);
  const [importAnimations, setImportAnimations] = createSignal(true);
  const [importCameras, setImportCameras] = createSignal(false);
  const [importLights, setImportLights] = createSignal(false);
  const [showAdvanced, setShowAdvanced] = createSignal(false);

  const importModes = [
    {
      id: 'smart',
      name: 'Smart (Recommended)',
      description: 'Auto-group similar objects and limit hierarchy depth',
      icon: <IconSparkles class="w-5 h-5" />
    },
    {
      id: 'simplified',
      name: 'Simplified',
      description: 'Combine objects by material type',
      icon: <IconGrid3x3 class="w-5 h-5" />
    },
    {
      id: 'individual',
      name: 'Individual',
      description: 'Keep all objects separate (may create many objects)',
      icon: <IconList class="w-5 h-5" />
    },
    {
      id: 'single',
      name: 'Single Mesh',
      description: 'Merge everything into one object',
      icon: <IconBox class="w-5 h-5" />
    }
  ];

  const handleImport = () => {
    const importSettings = {
      mode: importMode(),
      maxObjects: maxObjects(),
      hierarchyDepth: hierarchyDepth(),
      importMaterials: importMaterials(),
      importAnimations: importAnimations(),
      importCameras: importCameras(),
      importLights: importLights()
    };
    
    onImport(importSettings);
  };

  if (!isOpen) return null;

  return (
    <div class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50">
      <div class="bg-slate-800 rounded-lg shadow-2xl max-w-md w-full mx-4 max-h-[90vh] overflow-hidden">
        <div class="flex items-center justify-between p-4 border-b border-slate-700">
          <div class="flex items-center gap-3">
            <IconBox class="w-5 h-5 text-blue-400" />
            <div>
              <h2 class="text-lg font-semibold text-white">Import Model</h2>
              <p class="text-sm text-gray-400">{modelName}</p>
            </div>
          </div>
          <button
            onClick={onClose}
            class="p-2 hover:bg-slate-700 rounded transition-colors"
          >
            <IconX class="w-4 h-4 text-gray-400" />
          </button>
        </div>

        <div class="p-4 space-y-4 max-h-[60vh] overflow-y-auto">
          <Show when={modelAnalysis}>
            <div class="bg-slate-900/50 rounded-lg p-3 text-sm">
              <div class="text-gray-300 mb-2">Model Analysis:</div>
              <div class="grid grid-cols-2 gap-2 text-xs">
                <div class="text-gray-400">Objects: <span class="text-white">{modelAnalysis.totalObjects}</span></div>
                <div class="text-gray-400">Materials: <span class="text-white">{modelAnalysis.totalMaterials}</span></div>
                <div class="text-gray-400">Meshes: <span class="text-white">{modelAnalysis.totalMeshes}</span></div>
                <div class="text-gray-400">Depth: <span class="text-white">{modelAnalysis.maxDepth} levels</span></div>
              </div>
              <Show when={modelAnalysis.totalObjects > 100}>
                <div class="mt-2 text-xs text-amber-400 flex items-center gap-1">
                  <IconAlertTriangle class="w-3 h-3" />
                  Complex model detected - Smart mode recommended
                </div>
              </Show>
            </div>
          </Show>

          <div>
            <label class="block text-sm font-medium text-gray-300 mb-2">Import Mode:</label>
            <div class="space-y-2">
              <For each={importModes}>
                {(mode) => (
                  <button
                    onClick={() => setImportMode(mode.id)}
                    class={`w-full p-3 rounded-lg border text-left transition-colors ${
                      importMode() === mode.id
                        ? 'border-blue-500 bg-blue-500/10 text-blue-300'
                        : 'border-slate-600 bg-slate-700/50 text-gray-300 hover:bg-slate-700'
                    }`}
                  >
                    <div class="flex items-start gap-3">
                      <div class={`mt-0.5 ${importMode() === mode.id ? 'text-blue-400' : 'text-gray-400'}`}>
                        {mode.icon}
                      </div>
                      <div>
                        <div class="font-medium">{mode.name}</div>
                        <div class="text-xs text-gray-400 mt-1">{mode.description}</div>
                      </div>
                    </div>
                  </button>
                )}
              </For>
            </div>
          </div>

          <Show when={importMode() === 'smart'}>
            <div class="space-y-3">
              <div>
                <label class="block text-sm font-medium text-gray-300 mb-2">
                  Hierarchy Depth: {hierarchyDepth()}
                </label>
                <input
                  type="range"
                  min="1"
                  max="5"
                  value={hierarchyDepth()}
                  onInput={(e) => setHierarchyDepth(parseInt(e.target.value))}
                  class="w-full h-2 bg-slate-600 rounded-lg appearance-none cursor-pointer"
                />
                <div class="flex justify-between text-xs text-gray-400 mt-1">
                  <span>Flat</span>
                  <span>Deep</span>
                </div>
              </div>

              <div>
                <label class="block text-sm font-medium text-gray-300 mb-2">
                  Max Objects: {maxObjects()}
                </label>
                <input
                  type="range"
                  min="10"
                  max="200"
                  value={maxObjects()}
                  onInput={(e) => setMaxObjects(parseInt(e.target.value))}
                  class="w-full h-2 bg-slate-600 rounded-lg appearance-none cursor-pointer"
                />
                <div class="flex justify-between text-xs text-gray-400 mt-1">
                  <span>Simple</span>
                  <span>Detailed</span>
                </div>
              </div>
            </div>
          </Show>

          <div>
            <label class="block text-sm font-medium text-gray-300 mb-2">Options:</label>
            <div class="space-y-2">
              <label class="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={importMaterials()}
                  onChange={(e) => setImportMaterials(e.target.checked)}
                  class="rounded border-slate-600 bg-slate-700 text-blue-500 focus:ring-blue-500/50"
                />
                <span class="text-gray-300">Import Materials</span>
              </label>
              <label class="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={importAnimations()}
                  onChange={(e) => setImportAnimations(e.target.checked)}
                  class="rounded border-slate-600 bg-slate-700 text-blue-500 focus:ring-blue-500/50"
                />
                <span class="text-gray-300">Import Animations</span>
              </label>
              <label class="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={importCameras()}
                  onChange={(e) => setImportCameras(e.target.checked)}
                  class="rounded border-slate-600 bg-slate-700 text-blue-500 focus:ring-blue-500/50"
                />
                <span class="text-gray-300">Import Cameras</span>
              </label>
              <label class="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={importLights()}
                  onChange={(e) => setImportLights(e.target.checked)}
                  class="rounded border-slate-600 bg-slate-700 text-blue-500 focus:ring-blue-500/50"
                />
                <span class="text-gray-300">Import Lights</span>
              </label>
            </div>
          </div>

          <button
            onClick={() => setShowAdvanced(!showAdvanced())}
            class="flex items-center gap-2 text-sm text-blue-400 hover:text-blue-300 transition-colors"
          >
            <IconChevronRight class={`w-3 h-3 transition-transform ${showAdvanced() ? 'rotate-90' : ''}`} />
            Advanced Options
          </button>

          <Show when={showAdvanced()}>
            <div class="bg-slate-900/30 rounded-lg p-3 space-y-2">
              <div class="text-xs text-gray-400">
                Advanced settings for fine-tuning import behavior
              </div>
            </div>
          </Show>
        </div>

        <div class="flex items-center justify-end gap-3 p-4 border-t border-slate-700">
          <button
            onClick={onClose}
            class="px-4 py-2 text-sm text-gray-300 hover:text-white transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleImport}
            class="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm rounded-lg transition-colors"
          >
            Import Model
          </button>
        </div>
      </div>
    </div>
  );
};

export default ModelImportDialog;
