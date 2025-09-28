import { createSignal, createEffect, Show } from 'solid-js';
import { IconX, IconDeviceFloppy } from '@tabler/icons-solidjs';
import { sceneManager } from '@/api/scene/SceneManager.js';

export default function SaveAsOverlay({ isOpen, onClose, onSave, currentSceneName }) {
  const [sceneName, setSceneName] = createSignal('');
  const [isLoading, setIsLoading] = createSignal(false);
  const [error, setError] = createSignal('');
  const [availableScenes, setAvailableScenes] = createSignal([]);

  // Initialize with current scene name when overlay opens
  createEffect(() => {
    if (isOpen()) {
      setSceneName(currentSceneName || '');
      setError('');
      loadAvailableScenes();
    }
  });

  const loadAvailableScenes = async () => {
    try {
      const scenes = await sceneManager.getAvailableScenes();
      setAvailableScenes(scenes);
    } catch (err) {
      console.warn('Failed to load available scenes:', err);
      setAvailableScenes([]);
    }
  };

  const handleSave = async () => {
    const name = sceneName().trim();
    if (!name) {
      setError('Scene name cannot be empty');
      return;
    }

    // Check if scene already exists and warn user
    if (availableScenes().includes(name) && name !== currentSceneName) {
      const shouldOverwrite = confirm(
        `A scene named "${name}" already exists. Do you want to overwrite it?`
      );
      if (!shouldOverwrite) {
        return;
      }
    }

    try {
      setIsLoading(true);
      setError('');
      await onSave(name);
      onClose();
    } catch (err) {
      console.error('Failed to save scene:', err);
      setError(err.message || 'Failed to save scene');
    } finally {
      setIsLoading(false);
    }
  };

  const handleKeyDown = (e) => {
    if (e.key === 'Enter') {
      handleSave();
    } else if (e.key === 'Escape') {
      onClose();
    }
  };

  const isNameValid = () => {
    const name = sceneName().trim();
    return name.length > 0 && /^[a-zA-Z0-9_-]+$/.test(name);
  };

  const getNameValidationMessage = () => {
    const name = sceneName().trim();
    if (!name) return '';
    if (!/^[a-zA-Z0-9_-]+$/.test(name)) {
      return 'Scene name can only contain letters, numbers, hyphens, and underscores';
    }
    return '';
  };

  return (
    <Show when={isOpen()}>
      <div 
        class="fixed inset-0 bg-black/80 backdrop-blur-sm z-50 flex items-center justify-center p-4"
        onClick={(e) => e.target === e.currentTarget && onClose()}
        onKeyDown={handleKeyDown}
        tabindex="-1"
      >
        <div class="bg-slate-900 rounded-xl border border-slate-700 w-full max-w-md shadow-2xl">
          {/* Header */}
          <div class="flex items-center justify-between p-6 border-b border-slate-700">
            <div class="flex items-center gap-3">
              <div class="p-2 bg-green-600/20 rounded-lg">
                <IconDeviceFloppy class="w-6 h-6 text-green-400" />
              </div>
              <div>
                <h2 class="text-xl font-bold text-white">Save Scene As</h2>
                <p class="text-sm text-gray-400">Enter a name for your scene</p>
              </div>
            </div>
            <button
              onClick={onClose}
              class="p-2 hover:bg-slate-800 rounded-lg transition-colors"
            >
              <IconX class="w-5 h-5 text-gray-400" />
            </button>
          </div>

          {/* Content */}
          <div class="p-6">
            <div class="space-y-4">
              <div>
                <label class="block text-sm font-medium text-gray-300 mb-2">
                  Scene Name
                </label>
                <input
                  type="text"
                  value={sceneName()}
                  onInput={(e) => setSceneName(e.target.value)}
                  onKeyDown={handleKeyDown}
                  placeholder="e.g., Level1, MainMenu, Credits"
                  class="w-full px-4 py-3 bg-slate-800 border border-slate-600 rounded-lg text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-green-500 focus:border-transparent"
                  autofocus
                />
                <Show when={getNameValidationMessage()}>
                  <div class="text-yellow-400 text-sm mt-2">
                    {getNameValidationMessage()}
                  </div>
                </Show>
              </div>

              <Show when={availableScenes().includes(sceneName().trim()) && sceneName().trim() !== currentSceneName}>
                <div class="text-orange-400 text-sm bg-orange-900/20 p-3 rounded-lg border border-orange-800">
                  ⚠️ A scene with this name already exists and will be overwritten
                </div>
              </Show>

              <Show when={error()}>
                <div class="text-red-400 text-sm bg-red-900/20 p-3 rounded-lg border border-red-800">
                  {error()}
                </div>
              </Show>

              <div class="flex gap-3 pt-2">
                <button
                  onClick={handleSave}
                  disabled={isLoading() || !isNameValid()}
                  class="flex-1 bg-green-600 hover:bg-green-700 disabled:bg-gray-600 disabled:cursor-not-allowed text-white font-medium py-3 px-4 rounded-lg transition-colors flex items-center justify-center gap-2"
                >
                  <Show when={isLoading()} fallback={<IconDeviceFloppy class="w-4 h-4" />}>
                    <div class="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
                  </Show>
                  {isLoading() ? 'Saving...' : 'Save Scene'}
                </button>
                <button
                  onClick={onClose}
                  class="px-6 py-3 text-gray-400 hover:text-white transition-colors"
                >
                  Cancel
                </button>
              </div>
            </div>

            {/* Current scene info */}
            <Show when={currentSceneName}>
              <div class="mt-4 pt-4 border-t border-slate-700">
                <p class="text-xs text-gray-500">
                  Currently editing: <span class="text-gray-400 font-medium">{currentSceneName}</span>
                </p>
              </div>
            </Show>
          </div>
        </div>
      </div>
    </Show>
  );
}