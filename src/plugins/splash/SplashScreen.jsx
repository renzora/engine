import { createSignal, createEffect, onMount, Show, For } from 'solid-js';
import { IconFolder, IconPlus, IconFolderOpen, IconSettings, IconCode, IconRocket, IconBox } from '@tabler/icons-solidjs';
import { getProjects, createProject } from '@/api/bridge/projects';
import { sceneManager } from '@/api/scene/SceneManager.js';
import AnimatedBackground from './AnimatedBackground';

export default function SplashScreen({ onProjectSelect }) {
  const [projects, setProjects] = createSignal([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal(null);
  const [showCreateDialog, setShowCreateDialog] = createSignal(false);
  const [newProjectName, setNewProjectName] = createSignal('');
  const [creating, setCreating] = createSignal(false);
  const [creationProgress, setCreationProgress] = createSignal({ step: 0, message: '', total: 4 });
  const [showSceneDialog, setShowSceneDialog] = createSignal(false);
  const [selectedProject, setSelectedProject] = createSignal(null);
  const [availableScenes, setAvailableScenes] = createSignal([]);
  const [loadingScenes, setLoadingScenes] = createSignal(false);
  const [projectSettings, setProjectSettings] = createSignal({
    template: 'basic',
    folders: {
      models: true,
      textures: true,
      materials: true,
      scripts: true,
      audio: true,
      video: false,
      images: false
    },
    physics: true,
    resolution: { width: 1920, height: 1080 }
  });

  // Load projects from bridge
  const loadProjects = async () => {
    try {
      setLoading(true);
      setError(null);
      
      const projectData = await getProjects();
      setProjects(projectData || []);
    } catch (err) {
      console.error('Failed to load projects:', err);
      setError('Failed to connect to project server. Make sure the bridge server is running.');
    } finally {
      setLoading(false);
    }
  };

  // Create a new project
  const createNewProject = async () => {
    const name = newProjectName().trim();
    if (!name) return;

    try {
      setCreating(true);
      
      // Step 1: Initialize project structure
      setCreationProgress({ step: 1, message: 'Creating project directory...', total: 4 });
      await new Promise(resolve => setTimeout(resolve, 300)); // Small delay for UX
      
      // Step 2: Create project via bridge API
      setCreationProgress({ step: 2, message: 'Setting up project files...', total: 4 });
      await createProject(name, projectSettings().template, projectSettings());
      
      // Step 3: Reload project list
      setCreationProgress({ step: 3, message: 'Refreshing project list...', total: 4 });
      await loadProjects();
      
      // Step 4: Select the new project
      setCreationProgress({ step: 4, message: 'Opening project...', total: 4 });
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
      setCreationProgress({ step: 0, message: '', total: 4 });
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
      onProjectSelect(project);
      
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
    } catch (err) {
      console.error('Failed to load scene:', err);
      alert('Failed to load scene');
    }
  };

  // Load projects on mount
  onMount(() => {
    loadProjects();
  });

  return (
    <div class="w-full h-full relative flex overflow-hidden bg-base-100">
      <style>{`
        @keyframes gradient-x {
          0%, 100% { 
            background-position: 0% 50%;
          }
          50% { 
            background-position: 100% 50%;
          }
        }
        
        .animate-gradient-x {
          background-size: 400% 400%;
          animation: gradient-x 3s ease infinite;
        }
        
        @keyframes float {
          0%, 100% { 
            transform: translateY(0px);
          }
          50% { 
            transform: translateY(-10px);
          }
        }
        
        .animate-float {
          animation: float 3s ease-in-out infinite;
        }
        
        @keyframes glow-pulse {
          0%, 100% { 
            box-shadow: 0 0 20px rgba(59, 130, 246, 0.3), 0 0 40px rgba(59, 130, 246, 0.2);
          }
          50% { 
            box-shadow: 0 0 30px rgba(59, 130, 246, 0.5), 0 0 60px rgba(59, 130, 246, 0.3);
          }
        }
        
        .animate-glow-pulse {
          animation: glow-pulse 2s ease-in-out infinite;
        }
        
        @keyframes shimmer {
          0% { 
            transform: translateX(-100%);
          }
          100% { 
            transform: translateX(100%);
          }
        }
        
        .animate-shimmer {
          animation: shimmer 2s infinite;
        }
        
        @keyframes reveal {
          0% { 
            opacity: 0;
            transform: scale(0.8) translateY(20px);
          }
          100% { 
            opacity: 1;
            transform: scale(1) translateY(0);
          }
        }
        
        .animate-reveal {
          animation: reveal 1s ease-out forwards;
        }
      `}</style>
      
      {/* Animated 3D Neon Grid Background */}
      <AnimatedBackground />
      
      {/* Left side - Brand/Welcome */}
      <div class="flex-1 relative z-10 flex flex-col justify-center items-center p-12">
        <div class="text-center mb-8">
          <div class="w-20 h-20 bg-gradient-to-br from-primary to-secondary rounded-2xl mx-auto mb-6 flex items-center justify-center shadow-2xl">
            <IconRocket class="w-10 h-10 text-primary-content" />
          </div>
          <h1 class="text-4xl font-bold text-base-content mb-3 tracking-tight">
            Renzora <span class="text-transparent bg-clip-text bg-gradient-to-r from-primary to-secondary">Engine</span> <span class="text-accent">r3-broken-af</span>
          </h1>
          <p class="text-lg text-base-content/70 max-w-md mx-auto leading-relaxed mb-8">
            Open sourced and royalty free game engine to build console quality games for the web
          </p>
          
          {/* Create New Project Button */}
          <div class="w-full">
            <button
              onClick={() => setShowCreateDialog(true)}
              class="w-full p-5 border border-base-content/15 hover:border-primary hover:bg-gradient-to-br hover:from-primary/20 hover:to-secondary/20 rounded-xl transition-all duration-300 group bg-base-200 hover:shadow-lg"
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
                        <div class="relative group">
                          <button
                            onClick={() => onProjectSelect(project)}
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
            
            <div class="mb-6">
              <label class="block text-sm font-semibold text-base-content/80 mb-3">
                Project Name
              </label>
              <input
                type="text"
                value={newProjectName()}
                onInput={(e) => setNewProjectName(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && createNewProject()}
                placeholder="My Awesome Project"
                class="input input-bordered w-full text-lg placeholder:text-base-content/50"
                autofocus
              />
              <p class="text-xs text-base-content/50 mt-2">Choose a descriptive name for your project</p>
            </div>

            {/* Project Settings */}
            <div class="space-y-4 mb-6">
              <h3 class="text-sm font-semibold text-base-content/80">Project Settings</h3>
              
              {/* Template Selection */}
              <div>
                <label class="block text-xs font-medium text-base-content/70 mb-2">Template</label>
                <select 
                  class="select select-bordered select-sm w-full"
                  value={projectSettings().template}
                  onChange={(e) => setProjectSettings(prev => ({ ...prev, template: e.target.value }))}
                >
                  <option value="basic">Basic (Standard folders)</option>
                  <option value="minimal">Minimal (Essential folders only)</option>
                  <option value="game">Game (Full folder structure)</option>
                </select>
              </div>

              {/* Asset Folders */}
              <div>
                <label class="block text-xs font-medium text-base-content/70 mb-2">Asset Folders</label>
                <div class="grid grid-cols-2 gap-2 text-xs">
                  {Object.entries(projectSettings().folders).map(([folder, enabled]) => (
                    <label class="flex items-center gap-2 cursor-pointer">
                      <input 
                        type="checkbox" 
                        class="checkbox checkbox-xs" 
                        checked={enabled}
                        onChange={(e) => setProjectSettings(prev => ({
                          ...prev,
                          folders: { ...prev.folders, [folder]: e.target.checked }
                        }))}
                      />
                      <span class="capitalize">{folder}</span>
                    </label>
                  ))}
                </div>
              </div>

              {/* Physics Setting */}
              <div class="flex items-center justify-between">
                <label class="text-xs font-medium text-base-content/70">Enable Physics</label>
                <input 
                  type="checkbox" 
                  class="checkbox checkbox-sm" 
                  checked={projectSettings().physics}
                  onChange={(e) => setProjectSettings(prev => ({ ...prev, physics: e.target.checked }))}
                />
              </div>

              {/* Resolution Setting */}
              <div>
                <label class="block text-xs font-medium text-base-content/70 mb-2">Default Resolution</label>
                <div class="flex gap-2">
                  <input 
                    type="number" 
                    class="input input-bordered input-xs flex-1" 
                    value={projectSettings().resolution.width}
                    onChange={(e) => setProjectSettings(prev => ({
                      ...prev,
                      resolution: { ...prev.resolution, width: parseInt(e.target.value) || 1920 }
                    }))}
                  />
                  <span class="text-xs text-base-content/50 flex items-center">×</span>
                  <input 
                    type="number" 
                    class="input input-bordered input-xs flex-1" 
                    value={projectSettings().resolution.height}
                    onChange={(e) => setProjectSettings(prev => ({
                      ...prev,
                      resolution: { ...prev.resolution, height: parseInt(e.target.value) || 1080 }
                    }))}
                  />
                </div>
              </div>
            </div>

            {/* Progress Bar */}
            <Show when={creating()}>
              <div class="mb-6">
                <div class="w-full bg-base-300 rounded-full h-2">
                  <div 
                    class="bg-gradient-to-r from-primary to-secondary h-2 rounded-full transition-all duration-500 ease-out"
                    style={{width: `${(creationProgress().step / creationProgress().total) * 100}%`}}
                  ></div>
                </div>
              </div>
            </Show>

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
                onClick={createNewProject}
                disabled={!newProjectName().trim() || creating()}
                class="btn btn-primary flex items-center gap-3"
              >
                <Show when={creating()}>
                  <div class="w-5 h-5 border-2 border-primary-content border-t-transparent rounded-full animate-spin"></div>
                </Show>
                <Show when={!creating()}>
                  <IconRocket class="w-5 h-5" />
                </Show>
                {creating() ? (
                  <div class="flex flex-col items-start">
                    <span class="text-sm font-semibold">{creationProgress().message}</span>
                    <span class="text-xs opacity-75">Step {creationProgress().step} of {creationProgress().total}</span>
                  </div>
                ) : 'Create Project'}
              </button>
            </div>
          </div>
        </div>
      </Show>

      {/* Scene Selection Dialog */}
      <Show when={showSceneDialog()}>
        <div class="fixed inset-0 bg-base-100/70 backdrop-blur-md flex items-center justify-center p-4 z-[100] animate-in fade-in duration-300">
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
                        onProjectSelect(selectedProject());
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
                      onProjectSelect(selectedProject());
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

    </div>
  );
}