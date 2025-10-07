import { createSignal, Show, For, createEffect } from 'solid-js';
import { IconX, IconFolder, IconFolderOpen, IconSettings, IconCode, IconBox } from '@tabler/icons-solidjs';
import { getProjects } from '@/api/bridge/projects';
import { sceneManager } from '@/api/scene/SceneManager.js';

export default function OpenProjectOverlay({ isOpen, onClose, onProjectSelect }) {
  const [projects, setProjects] = createSignal([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal(null);
  const [showSceneDialog, setShowSceneDialog] = createSignal(false);
  const [selectedProject, setSelectedProject] = createSignal(null);
  const [availableScenes, setAvailableScenes] = createSignal([]);
  const [loadingScenes, setLoadingScenes] = createSignal(false);

  const handleOverlayClick = (e) => {
    if (e.target === e.currentTarget) {
      onClose();
    }
  };

  // Load projects from bridge
  const loadProjects = async () => {
    try {
      console.log('OpenProjectOverlay: Starting to load projects...');
      setLoading(true);
      setError(null);
      
      const projectData = await getProjects();
      console.log('OpenProjectOverlay: Received project data:', projectData);
      setProjects(projectData || []);
    } catch (err) {
      console.error('OpenProjectOverlay: Failed to load projects:', err);
      setError('Failed to connect to project server. Make sure the bridge server is running.');
    } finally {
      console.log('OpenProjectOverlay: Finished loading, setting loading to false');
      setLoading(false);
    }
  };

  // Load scenes for a project
  const loadScenesForProject = async (project) => {
    try {
      setLoadingScenes(true);
      setSelectedProject(project);
      
      // Temporarily set current project for sceneManager
      const currentProject = { name: project.name };
      window.tempCurrentProject = currentProject;
      
      const scenes = await sceneManager.getAvailableScenes();
      setAvailableScenes(scenes);
      setShowSceneDialog(true);
    } catch (err) {
      console.error('Failed to load scenes:', err);
      setError('Failed to load scenes for project');
    } finally {
      setLoadingScenes(false);
    }
  };

  // Load a specific scene and open project
  const loadSceneAndProject = async (sceneName) => {
    try {
      const project = selectedProject();
      if (!project) return;
      
      // First open the project
      if (onProjectSelect) {
        await onProjectSelect(project);
      }
      
      // Wait a bit for project to load, then load the scene
      setTimeout(async () => {
        const result = await sceneManager.loadScene(sceneName);
        if (result.success) {
          // Scene loaded successfully
        } else {
          alert(`Failed to load scene: ${result.error}`);
        }
      }, 500);
      
      setShowSceneDialog(false);
      onClose(); // Close the open project overlay
    } catch (err) {
      console.error('Failed to load scene:', err);
      alert('Failed to load scene');
    }
  };

  // Handle direct project selection
  const handleProjectClick = async (project) => {
    if (onProjectSelect) {
      await onProjectSelect(project);
      onClose(); // Close the overlay after selecting project
    }
  };

  // Load projects whenever the overlay opens
  createEffect(() => {
    if (isOpen()) {
      console.log('OpenProjectOverlay: Loading projects...');
      loadProjects();
    }
  });

  return (
    <Show when={isOpen()}>
      <div 
        class="fixed inset-0 bg-base-100/70 backdrop-blur-md flex items-center justify-center p-4 z-[200] animate-in fade-in duration-300"
        onClick={handleOverlayClick}
      >
        <div class="bg-base-200 backdrop-blur-xl rounded-2xl border border-base-content/30 shadow-2xl max-w-4xl w-full max-h-[80vh] overflow-hidden animate-in zoom-in-95 duration-300">
          {/* Header */}
          <div class="flex items-center justify-between p-6 border-b border-base-300">
            <div class="flex items-center gap-3">
              <div class="w-10 h-10 bg-gradient-to-br from-primary to-secondary rounded-xl flex items-center justify-center">
                <IconFolderOpen class="w-6 h-6 text-primary-content" />
              </div>
              <h2 class="text-2xl font-bold text-base-content">Open Project</h2>
            </div>
            <button
              onClick={onClose}
              class="w-8 h-8 flex items-center justify-center text-base-content/60 hover:text-base-content hover:bg-base-300 rounded-lg transition-colors"
            >
              <IconX class="w-4 h-4" />
            </button>
          </div>

          {/* Content */}
          <div class="p-6 overflow-y-auto max-h-[60vh]">
            <Show when={loading()}>
              <div class="text-center py-8 flex-1 flex flex-col items-center justify-center">
                <div class="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin mx-auto mb-4"></div>
                <p class="text-base-content/60">Loading projects...</p>
              </div>
            </Show>

            <Show when={error()}>
              <div class="text-center py-8 flex-1 flex flex-col items-center justify-center">
                <div class="w-12 h-12 bg-error/20 rounded-full flex items-center justify-center mx-auto mb-4">
                  <IconSettings class="w-6 h-6 text-error" />
                </div>
                <p class="text-error mb-4 text-sm">{error()}</p>
                <button
                  onClick={loadProjects}
                  class="btn btn-primary btn-sm"
                >
                  Retry
                </button>
              </div>
            </Show>

            <Show when={!loading() && !error()}>
              <Show when={projects().length > 0}>
                <div class="text-xs font-semibold text-base-content/40 uppercase tracking-wider mb-6">Available Projects</div>
                <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                  <For each={projects()}>
                    {(project) => (
                      <div class="relative group">
                        <button
                          onClick={() => handleProjectClick(project)}
                          class="w-full p-4 bg-gradient-to-br from-base-200 to-base-300 hover:from-base-200 hover:to-base-200 border border-primary/20 rounded-xl transition-all duration-300 text-left group shadow-lg hover:shadow-xl"
                        >
                          <div class="flex flex-col items-center text-center gap-3">
                            <div class="p-3 bg-gradient-to-br from-primary/20 to-secondary/20 group-hover:from-primary/40 group-hover:to-secondary/40 rounded-xl border border-base-content/10 group-hover:border-primary/30 transition-all duration-300">
                              <IconFolder class="w-6 h-6 text-primary group-hover:text-primary/80" />
                            </div>
                            <div class="w-full">
                              <h3 class="font-bold text-base-content group-hover:text-primary truncate mb-2">{project.name}</h3>
                              <p class="text-xs text-base-content/60 group-hover:text-base-content/70 truncate mb-2 font-mono">{project.path}</p>
                              <div class="flex items-center justify-center gap-1 text-xs text-base-content/50 group-hover:text-base-content/60">
                                <IconBox class="w-3 h-3" />
                                <span class="font-medium">{project.files?.length || 0} assets</span>
                              </div>
                            </div>
                          </div>
                        </button>
                        {/* Load Scene Button */}
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            loadScenesForProject(project);
                          }}
                          class="absolute top-2 right-2 p-1 bg-base-300 hover:bg-primary/20 rounded-lg opacity-0 group-hover:opacity-100 transition-all duration-200"
                          title="Load Scene"
                        >
                          <IconCode class="w-4 h-4 text-base-content/70 hover:text-primary" />
                        </button>
                      </div>
                    )}
                  </For>
                </div>
              </Show>

              <Show when={projects().length === 0}>
                <div class="text-center py-12 flex-1 flex flex-col items-center justify-center">
                  <div class="w-20 h-20 bg-gradient-to-br from-base-300 to-base-300 rounded-2xl flex items-center justify-center mx-auto mb-6 border border-base-content/10">
                    <IconFolderOpen class="w-10 h-10 text-base-content/60" />
                  </div>
                  <h3 class="text-base-content mb-2 font-semibold text-lg">No projects found</h3>
                  <p class="text-base-content/60 mb-4 max-w-xs">Create your first project to start building amazing 3D experiences</p>
                  <button
                    onClick={onClose}
                    class="btn btn-primary"
                  >
                    Close
                  </button>
                </div>
              </Show>
            </Show>
          </div>
        </div>
      </div>

      {/* Scene Selection Dialog */}
      <Show when={showSceneDialog()}>
        <div class="fixed inset-0 bg-base-100/70 backdrop-blur-md flex items-center justify-center p-4 z-[250] animate-in fade-in duration-300">
          <div class="bg-base-200 backdrop-blur-xl rounded-2xl border border-base-content/30 p-8 w-full max-w-md shadow-2xl animate-in zoom-in-95 duration-300">
            <div class="flex items-center gap-3 mb-6">
              <div class="w-10 h-10 bg-gradient-to-br from-primary to-secondary rounded-xl flex items-center justify-center">
                <IconCode class="w-6 h-6 text-primary-content" />
              </div>
              <div>
                <h2 class="text-xl font-bold text-base-content">Load Scene</h2>
                <p class="text-sm text-base-content/60">{selectedProject()?.name}</p>
              </div>
            </div>
            
            <Show when={loadingScenes()}>
              <div class="text-center py-8">
                <div class="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin mx-auto mb-4"></div>
                <p class="text-base-content/60">Loading scenes...</p>
              </div>
            </Show>

            <Show when={!loadingScenes()}>
              <Show when={availableScenes().length === 0}>
                <div class="text-center py-8">
                  <p class="text-base-content/60 mb-4">No scenes found in this project</p>
                  <div class="flex gap-2 justify-center">
                    <button
                      onClick={() => setShowSceneDialog(false)}
                      class="btn btn-ghost btn-sm"
                    >
                      Cancel
                    </button>
                    <button
                      onClick={() => {
                        setShowSceneDialog(false);
                        handleProjectClick(selectedProject());
                      }}
                      class="btn btn-primary btn-sm"
                    >
                      Open Project
                    </button>
                  </div>
                </div>
              </Show>

              <Show when={availableScenes().length > 0}>
                <div class="mb-6">
                  <label class="block text-sm font-semibold text-base-content/80 mb-3">
                    Available Scenes
                  </label>
                  <div class="space-y-2 max-h-64 overflow-y-auto">
                    <For each={availableScenes()}>
                      {(sceneName) => (
                        <button
                          onClick={() => loadSceneAndProject(sceneName)}
                          class="w-full p-3 bg-base-100 hover:bg-primary/10 border border-base-300 hover:border-primary/30 rounded-lg transition-all duration-200 text-left group"
                        >
                          <div class="flex items-center gap-3">
                            <div class="w-8 h-8 bg-primary/10 group-hover:bg-primary/20 rounded-lg flex items-center justify-center">
                              <IconCode class="w-4 h-4 text-primary" />
                            </div>
                            <div class="flex-1">
                              <div class="font-medium text-base-content group-hover:text-primary">{sceneName}</div>
                              <div class="text-xs text-base-content/50">Scene file</div>
                            </div>
                          </div>
                        </button>
                      )}
                    </For>
                  </div>
                </div>

                <div class="flex justify-end gap-2">
                  <button
                    onClick={() => setShowSceneDialog(false)}
                    class="btn btn-ghost btn-sm"
                  >
                    Cancel
                  </button>
                  <button
                    onClick={() => {
                      setShowSceneDialog(false);
                      handleProjectClick(selectedProject());
                    }}
                    class="btn btn-secondary btn-sm"
                  >
                    Open Project Only
                  </button>
                </div>
              </Show>
            </Show>
          </div>
        </div>
      </Show>
    </Show>
  );
}