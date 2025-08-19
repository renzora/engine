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
    <div class="w-full h-full relative flex overflow-hidden bg-base-100">
      {/* Animated 3D Neon Grid Background */}
      <AnimatedBackground />
      
      {/* Left side - Brand/Welcome */}
      <div class="flex-1 relative z-10 flex flex-col justify-center items-center p-12">
        <div class="text-center mb-8">
          <div class="w-20 h-20 bg-gradient-to-br from-primary to-secondary rounded-2xl mx-auto mb-6 flex items-center justify-center shadow-2xl">
            <IconRocket class="w-10 h-10 text-primary-content" />
          </div>
          <h1 class="text-4xl font-bold text-base-content mb-3 tracking-tight">
            Renzora <span class="text-transparent bg-clip-text bg-gradient-to-r from-primary to-secondary">Engine</span> <span class="text-accent">r2</span>
          </h1>
          <p class="text-lg text-base-content/70 max-w-md mx-auto leading-relaxed mb-8">
            Open sourced and royalty free game engine to build console quality games for the web
          </p>
          
          {/* Create New Project Button */}
          <div class="w-full">
            <button
              onClick={() => setShowCreateDialog(true)}
              class="w-full p-5 border-2 border-dashed border-base-content/15 hover:border-primary hover:bg-gradient-to-br hover:from-primary/20 hover:to-secondary/20 rounded-xl transition-all duration-300 group bg-base-200 hover:shadow-lg"
            >
              <div class="flex flex-col items-center gap-3 text-base-content/70 group-hover:text-primary">
                <div class="w-12 h-12 bg-gradient-to-br from-primary/10 to-secondary/10 group-hover:from-primary/20 group-hover:to-secondary/20 rounded-xl flex items-center justify-center border border-base-content/10 group-hover:border-primary/30 transition-all">
                  <IconPlus class="w-6 h-6" />
                </div>
                <div class="text-center">
                  <div class="font-semibold text-base-content">Create New Project</div>
                  <div class="text-xs text-base-content/50 group-hover:text-base-content/60">Start building something amazing</div>
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
            <div class="flex flex-col h-full min-h-0">
              {/* Projects Grid */}
              <Show when={projects().length > 0}>
                <div class="flex-1 min-h-0 overflow-y-auto overflow-x-hidden scrollbar-thin">
                  <div class="text-xs font-semibold text-base-content/40 uppercase tracking-wider mb-6">Recent Projects</div>
                  <div class="grid grid-cols-3 gap-4 mr-2">
                    <For each={projects()}>
                      {(project) => (
                        <button
                          onClick={() => onProjectSelect(project)}
                          class="p-4 bg-gradient-to-br from-base-200 to-base-300 hover:from-base-200 hover:to-base-200 border border-primary/20 rounded-xl transition-all duration-300 text-left group shadow-lg hover:shadow-xl"
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
                      )}
                    </For>
                  </div>
                </div>
              </Show>

              <Show when={projects().length === 0}>
                <div class="text-center py-12 flex-1 flex flex-col items-center justify-center">
                  <div class="w-20 h-20 bg-gradient-to-br from-base-300 to-base-300 rounded-2xl flex items-center justify-center mx-auto mb-6 border border-base-content/10">
                    <IconFolderOpen class="w-10 h-10 text-base-content/60" />
                  </div>
                  <h3 class="text-base-content mb-2 font-semibold text-lg">No projects yet</h3>
                  <p class="text-base-content/60 mb-4 max-w-xs">Create your first project to start building amazing 3D experiences</p>
                  <button
                    onClick={() => setShowCreateDialog(true)}
                    class="btn btn-primary"
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
        <div class="fixed inset-0 bg-base-100/70 backdrop-blur-md flex items-center justify-center p-4 z-[100] animate-in fade-in duration-300">
          <div class="bg-base-200 backdrop-blur-xl rounded-2xl border border-base-content/30 p-8 w-full max-w-lg shadow-2xl animate-in zoom-in-95 duration-300">
            <div class="flex items-center gap-3 mb-6">
              <div class="w-10 h-10 bg-gradient-to-br from-primary to-secondary rounded-xl flex items-center justify-center">
                <IconPlus class="w-6 h-6 text-primary-content" />
              </div>
              <h2 class="text-2xl font-bold text-base-content">Create New Project</h2>
            </div>
            
            <div class="mb-8">
              <label class="block text-sm font-semibold text-base-content/80 mb-3">
                Project Name
              </label>
              <input
                type="text"
                value={newProjectName()}
                onInput={(e) => setNewProjectName(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && createProject()}
                placeholder="My Awesome Project"
                class="input input-bordered w-full text-lg placeholder:text-base-content/50"
                autofocus
              />
              <p class="text-xs text-base-content/50 mt-2">Choose a descriptive name for your project</p>
            </div>

            <div class="flex justify-end gap-4">
              <button
                onClick={() => {
                  setShowCreateDialog(false);
                  setNewProjectName('');
                }}
                disabled={creating()}
                class="btn btn-ghost"
              >
                Cancel
              </button>
              <button
                onClick={createProject}
                disabled={!newProjectName().trim() || creating()}
                class="btn btn-primary flex items-center gap-3"
              >
                <Show when={creating()}>
                  <div class="w-5 h-5 border-2 border-primary-content border-t-transparent rounded-full animate-spin"></div>
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