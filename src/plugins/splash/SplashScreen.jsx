import { createSignal, createEffect, onMount, Show, For } from 'solid-js';
import { IconFolder, IconPlus, IconFolderOpen, IconSettings, IconCode, IconRocket, IconBox } from '@tabler/icons-solidjs';
import { bridgeService } from '@/plugins/core/bridge';
import AnimatedBackground from './AnimatedBackground';

export default function SplashScreen({ onProjectSelect }) {
  const [projects, setProjects] = createSignal([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal(null);
  const [showCreateDialog, setShowCreateDialog] = createSignal(false);
  const [newProjectName, setNewProjectName] = createSignal('');
  const [creating, setCreating] = createSignal(false);

  // Load projects from bridge
  const loadProjects = async () => {
    try {
      setLoading(true);
      setError(null);
      
      const response = await fetch('http://localhost:3001/projects');
      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }
      
      const projectData = await response.json();
      setProjects(projectData || []);
    } catch (err) {
      console.error('Failed to load projects:', err);
      setError('Failed to connect to project server. Make sure the bridge server is running.');
    } finally {
      setLoading(false);
    }
  };

  // Create a new project
  const createProject = async () => {
    const name = newProjectName().trim();
    if (!name) return;

    try {
      setCreating(true);
      
      // Create project directory structure via bridge
      const response = await fetch('http://localhost:3001/projects', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          name,
          template: 'basic' // Could be expanded to support different templates
        })
      });

      if (!response.ok) {
        throw new Error(`Failed to create project: ${response.status}`);
      }

      // Reload projects and select the new one
      await loadProjects();
      const newProject = projects().find(p => p.name === name);
      if (newProject) {
        onProjectSelect(newProject);
      }
      
      setShowCreateDialog(false);
      setNewProjectName('');
    } catch (err) {
      console.error('Failed to create project:', err);
      setError('Failed to create project. Please try again.');
    } finally {
      setCreating(false);
    }
  };

  // Load projects on mount
  onMount(() => {
    loadProjects();
  });

  return (
    <div class="w-full h-full relative flex overflow-hidden bg-black">
      {/* Animated 3D Neon Grid Background */}
      <AnimatedBackground />
      
      {/* Left side - Brand/Welcome */}
      <div class="flex-1 relative z-10 flex flex-col justify-center items-center p-12">
        <div class="text-center mb-8">
          <div class="w-20 h-20 bg-gradient-to-br from-blue-500 to-purple-600 rounded-2xl mx-auto mb-6 flex items-center justify-center shadow-2xl">
            <IconRocket class="w-10 h-10 text-white" />
          </div>
          <h1 class="text-4xl font-bold text-white mb-3 tracking-tight">
            Renzora <span class="text-transparent bg-clip-text bg-gradient-to-r from-blue-400 to-purple-400">Engine</span> <span class="text-orange-400">r2</span>
          </h1>
          <p class="text-lg text-gray-300 max-w-md mx-auto leading-relaxed mb-8">
            Open sourced and royalty free game engine to build console quality games for the web
          </p>
          
          {/* Create New Project Button */}
          <div class="w-full">
            <button
              onClick={() => setShowCreateDialog(true)}
              class="w-full p-5 border-2 border-dashed border-white/15 hover:border-blue-400 hover:bg-gradient-to-br hover:from-blue-500/20 hover:to-purple-500/20 rounded-xl transition-all duration-300 group bg-black/50 hover:shadow-lg"
            >
              <div class="flex flex-col items-center gap-3 text-gray-300 group-hover:text-blue-400">
                <div class="w-12 h-12 bg-gradient-to-br from-blue-500/10 to-purple-500/10 group-hover:from-blue-500/20 group-hover:to-purple-500/20 rounded-xl flex items-center justify-center border border-white/10 group-hover:border-blue-400/30 transition-all">
                  <IconPlus class="w-6 h-6" />
                </div>
                <div class="text-center">
                  <div class="font-semibold">Create New Project</div>
                  <div class="text-xs text-gray-500 group-hover:text-gray-400">Start building something amazing</div>
                </div>
              </div>
            </button>
          </div>
        </div>
      </div>

      {/* Right side - Project Panel */}
      <div class="w-[32rem] relative z-10 flex flex-col">
        <div class="flex-1 p-12 flex flex-col min-h-0">
          <Show when={loading()}>
            <div class="text-center py-8 flex-1 flex flex-col items-center justify-center">
              <div class="w-8 h-8 border-2 border-blue-400 border-t-transparent rounded-full animate-spin mx-auto mb-4"></div>
              <p class="text-gray-400">Loading projects...</p>
            </div>
          </Show>

          <Show when={error()}>
            <div class="text-center py-8 flex-1 flex flex-col items-center justify-center">
              <div class="w-12 h-12 bg-red-500/20 rounded-full flex items-center justify-center mx-auto mb-4">
                <IconSettings class="w-6 h-6 text-red-400" />
              </div>
              <p class="text-red-400 mb-4 text-sm">{error()}</p>
              <button
                onClick={loadProjects}
                class="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-lg transition-colors text-sm"
              >
                Retry
              </button>
            </div>
          </Show>

          <Show when={!loading() && !error()}>
            <div class="flex flex-col h-full min-h-0">
              {/* Projects Grid */}
              <Show when={projects().length > 0}>
                <div class="flex-1 min-h-0 overflow-y-auto overflow-x-hidden scrollbar-thin">
                  <div class="text-xs font-semibold text-gray-400 uppercase tracking-wider mb-6">Recent Projects</div>
                  <div class="grid grid-cols-3 gap-4 mr-2">
                    <For each={projects()}>
                      {(project) => (
                        <button
                          onClick={() => onProjectSelect(project)}
                          class="p-4 bg-gradient-to-br from-black/90 to-black/85 hover:from-black/95 hover:to-black/90 border border-blue-400/20 rounded-xl transition-all duration-300 text-left group shadow-lg hover:shadow-xl"
                        >
                          <div class="flex flex-col items-center text-center gap-3">
                            <div class="p-3 bg-gradient-to-br from-blue-500/20 to-purple-500/20 group-hover:from-blue-500/40 group-hover:to-purple-500/40 rounded-xl border border-white/10 group-hover:border-blue-400/30 transition-all duration-300">
                              <IconFolder class="w-6 h-6 text-blue-400 group-hover:text-blue-300" />
                            </div>
                            <div class="w-full">
                              <h3 class="font-bold text-white group-hover:text-blue-100 truncate mb-2">{project.name}</h3>
                              <p class="text-xs text-gray-400 group-hover:text-gray-300 truncate mb-2 font-mono">{project.path}</p>
                              <div class="flex items-center justify-center gap-1 text-xs text-gray-500 group-hover:text-gray-400">
                                <IconBox class="w-3 h-3" />
                                <span class="font-medium">{project.files?.length || 0} assets</span>
                              </div>
                            </div>
                          </div>
                        </button>
                      )}
                    </For>
                  </div>
                </div>
              </Show>

              <Show when={projects().length === 0}>
                <div class="text-center py-12 flex-1 flex flex-col items-center justify-center">
                  <div class="w-20 h-20 bg-gradient-to-br from-gray-600/20 to-gray-700/20 rounded-2xl flex items-center justify-center mx-auto mb-6 border border-white/10">
                    <IconFolderOpen class="w-10 h-10 text-gray-400" />
                  </div>
                  <h3 class="text-white mb-2 font-semibold text-lg">No projects yet</h3>
                  <p class="text-gray-400 mb-4 max-w-xs">Create your first project to start building amazing 3D experiences</p>
                  <button
                    onClick={() => setShowCreateDialog(true)}
                    class="px-6 py-3 bg-gradient-to-r from-blue-600 to-purple-600 hover:from-blue-700 hover:to-purple-700 text-white rounded-lg transition-all font-medium shadow-lg hover:shadow-xl transform hover:scale-105"
                  >
                    Get Started
                  </button>
                </div>
              </Show>
            </div>
          </Show>
        </div>
      </div>

      {/* Create Project Dialog */}
      <Show when={showCreateDialog()}>
        <div class="fixed inset-0 bg-black/70 backdrop-blur-md flex items-center justify-center p-4 z-[100] animate-in fade-in duration-300">
          <div class="bg-black/90 backdrop-blur-xl rounded-2xl border border-white/30 p-8 w-full max-w-lg shadow-2xl animate-in zoom-in-95 duration-300">
            <div class="flex items-center gap-3 mb-6">
              <div class="w-10 h-10 bg-gradient-to-br from-blue-500 to-purple-600 rounded-xl flex items-center justify-center">
                <IconPlus class="w-6 h-6 text-white" />
              </div>
              <h2 class="text-2xl font-bold text-white">Create New Project</h2>
            </div>
            
            <div class="mb-8">
              <label class="block text-sm font-semibold text-gray-300 mb-3">
                Project Name
              </label>
              <input
                type="text"
                value={newProjectName()}
                onInput={(e) => setNewProjectName(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && createProject()}
                placeholder="My Awesome Project"
                class="w-full px-5 py-4 bg-black/50 border border-white/30 rounded-xl text-white placeholder-gray-500 focus:outline-none focus:border-blue-400 focus:ring-2 focus:ring-blue-400/20 transition-all text-lg"
                autofocus
              />
              <p class="text-xs text-gray-500 mt-2">Choose a descriptive name for your project</p>
            </div>

            <div class="flex justify-end gap-4">
              <button
                onClick={() => {
                  setShowCreateDialog(false);
                  setNewProjectName('');
                }}
                disabled={creating()}
                class="px-6 py-3 text-gray-400 hover:text-white transition-all disabled:opacity-50 font-medium"
              >
                Cancel
              </button>
              <button
                onClick={createProject}
                disabled={!newProjectName().trim() || creating()}
                class="px-8 py-3 bg-gradient-to-r from-blue-600 to-purple-600 hover:from-blue-700 hover:to-purple-700 disabled:from-gray-600 disabled:to-gray-600 text-white rounded-xl transition-all flex items-center gap-3 font-semibold shadow-lg hover:shadow-xl transform hover:scale-105 disabled:transform-none"
              >
                <Show when={creating()}>
                  <div class="w-5 h-5 border-2 border-white border-t-transparent rounded-full animate-spin"></div>
                </Show>
                <Show when={!creating()}>
                  <IconRocket class="w-5 h-5" />
                </Show>
                {creating() ? 'Creating...' : 'Create Project'}
              </button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
}