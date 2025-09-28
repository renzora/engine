import { createSignal, createEffect, Show, For } from 'solid-js';
import { IconX, IconPlus, IconFolder, IconChairDirector, IconTrash } from '@tabler/icons-solidjs';
import { sceneManager } from '@/api/scene/SceneManager.js';

export default function SceneSelectionOverlay({ isOpen, onClose, onSceneSelect, onCreateScene }) {
  const [scenes, setScenes] = createSignal([]);
  const [isLoading, setIsLoading] = createSignal(false);
  const [showCreateForm, setShowCreateForm] = createSignal(false);
  const [newSceneName, setNewSceneName] = createSignal('');
  const [error, setError] = createSignal('');

  // Load scenes when overlay opens
  createEffect(() => {
    if (isOpen()) {
      loadScenes();
    }
  });

  const loadScenes = async () => {
    setIsLoading(true);
    setError('');
    try {
      const sceneList = await sceneManager.getAvailableScenes();
      setScenes(sceneList);
    } catch (err) {
      console.error('Failed to load scenes:', err);
      setError('Failed to load scenes');
    } finally {
      setIsLoading(false);
    }
  };

  const handleSceneSelect = (sceneName) => {
    onSceneSelect(sceneName);
    onClose();
  };

  const handleCreateNewScene = () => {
    setShowCreateForm(true);
    setNewSceneName('');
    setError('');
  };

  const handleCreateScene = async () => {
    const name = newSceneName().trim();
    if (!name) {
      setError('Scene name cannot be empty');
      return;
    }

    if (scenes().includes(name)) {
      setError('A scene with this name already exists');
      return;
    }

    try {
      setIsLoading(true);
      const result = await sceneManager.createNewScene(name);
      if (result.success) {
        onCreateScene(name);
        onClose();
      } else {
        setError(result.error || 'Failed to create scene');
      }
    } catch (err) {
      console.error('Failed to create scene:', err);
      setError('Failed to create scene');
    } finally {
      setIsLoading(false);
    }
  };

  const handleKeyDown = (e) => {
    if (e.key === 'Enter' && showCreateForm()) {
      handleCreateScene();
    } else if (e.key === 'Escape') {
      if (showCreateForm()) {
        setShowCreateForm(false);
        setError('');
      } else {
        onClose();
      }
    }
  };

  return (
    <Show when={isOpen()}>
      <div 
        class="fixed inset-0 bg-black/80 backdrop-blur-sm z-50 flex items-center justify-center p-4"
        onClick={(e) => e.target === e.currentTarget && onClose()}
        onKeyDown={handleKeyDown}
        tabindex="-1"
      >
        <div class="bg-slate-900 rounded-xl border border-slate-700 w-full max-w-2xl max-h-[80vh] flex flex-col shadow-2xl">
          {/* Header */}
          <div class="flex items-center justify-between p-6 border-b border-slate-700">
            <div class="flex items-center gap-3">
              <div class="p-2 bg-blue-600/20 rounded-lg">
                <IconChairDirector class="w-6 h-6 text-blue-400" />
              </div>
              <div>
                <h2 class="text-xl font-bold text-white">Select Scene</h2>
                <p class="text-sm text-gray-400">Choose an existing scene or create a new one</p>
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
          <div class="flex-1 overflow-hidden">
            <Show when={!showCreateForm()} fallback={
              /* Create New Scene Form */
              <div class="p-6">
                <div class="mb-6">
                  <h3 class="text-lg font-semibold text-white mb-2">Create New Scene</h3>
                  <p class="text-sm text-gray-400">Enter a name for your new scene</p>
                </div>

                <div class="space-y-4">
                  <div>
                    <label class="block text-sm font-medium text-gray-300 mb-2">
                      Scene Name
                    </label>
                    <input
                      type="text"
                      value={newSceneName()}
                      onInput={(e) => setNewSceneName(e.target.value)}
                      onKeyDown={handleKeyDown}
                      placeholder="e.g., Level1, MainMenu, Credits"
                      class="w-full px-4 py-3 bg-slate-800 border border-slate-600 rounded-lg text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                      autofocus
                    />
                  </div>

                  <Show when={error()}>
                    <div class="text-red-400 text-sm bg-red-900/20 p-3 rounded-lg border border-red-800">
                      {error()}
                    </div>
                  </Show>

                  <div class="flex gap-3 pt-2">
                    <button
                      onClick={handleCreateScene}
                      disabled={isLoading() || !newSceneName().trim()}
                      class="flex-1 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-600 disabled:cursor-not-allowed text-white font-medium py-3 px-4 rounded-lg transition-colors flex items-center justify-center gap-2"
                    >
                      <Show when={isLoading()} fallback={<IconPlus class="w-4 h-4" />}>
                        <div class="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
                      </Show>
                      {isLoading() ? 'Creating...' : 'Create Scene'}
                    </button>
                    <button
                      onClick={() => {
                        setShowCreateForm(false);
                        setError('');
                      }}
                      class="px-6 py-3 text-gray-400 hover:text-white transition-colors"
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              </div>
            }>
              {/* Scene List */}
              <div class="p-6">
                <div class="mb-6">
                  <button
                    onClick={handleCreateNewScene}
                    class="w-full bg-blue-600 hover:bg-blue-700 text-white font-medium py-3 px-4 rounded-lg transition-colors flex items-center justify-center gap-2"
                  >
                    <IconPlus class="w-4 h-4" />
                    Create New Scene
                  </button>
                </div>

                <Show when={error()}>
                  <div class="text-red-400 text-sm bg-red-900/20 p-3 rounded-lg border border-red-800 mb-4">
                    {error()}
                  </div>
                </Show>

                <Show when={isLoading()} fallback={
                  <Show when={scenes().length > 0} fallback={
                    <div class="text-center py-12">
                      <IconFolder class="w-12 h-12 text-gray-500 mx-auto mb-4" />
                      <p class="text-gray-400">No scenes found</p>
                      <p class="text-gray-500 text-sm">Create your first scene to get started</p>
                    </div>
                  }>
                    <div>
                      <h3 class="text-sm font-medium text-gray-400 uppercase tracking-wider mb-3">
                        Available Scenes ({scenes().length})
                      </h3>
                      <div class="space-y-2 max-h-96 overflow-y-auto">
                        <For each={scenes()}>
                          {(sceneName) => (
                            <div class="bg-slate-800 hover:bg-slate-750 border border-slate-700 hover:border-slate-600 rounded-lg p-4 cursor-pointer transition-all group">
                              <div 
                                class="flex items-center gap-3"
                                onClick={() => handleSceneSelect(sceneName)}
                              >
                                <div class="p-2 bg-purple-600/20 rounded-lg group-hover:bg-purple-600/30 transition-colors">
                                  <IconChairDirector class="w-5 h-5 text-purple-400" />
                                </div>
                                <div class="flex-1">
                                  <h4 class="font-medium text-white group-hover:text-blue-400 transition-colors">
                                    {sceneName}
                                  </h4>
                                  <p class="text-sm text-gray-400">Scene file</p>
                                </div>
                                <div class="opacity-0 group-hover:opacity-100 transition-opacity">
                                  <span class="text-xs text-blue-400 font-medium">Load Scene</span>
                                </div>
                              </div>
                            </div>
                          )}
                        </For>
                      </div>
                    </div>
                  </Show>
                }>
                  <div class="text-center py-12">
                    <div class="w-8 h-8 border-2 border-blue-500 border-t-transparent rounded-full animate-spin mx-auto mb-4" />
                    <p class="text-gray-400">Loading scenes...</p>
                  </div>
                </Show>
              </div>
            </Show>
          </div>
        </div>
      </div>
    </Show>
  );
}