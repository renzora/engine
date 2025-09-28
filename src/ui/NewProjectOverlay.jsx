import { createSignal, Show } from 'solid-js';
import { IconPlus, IconX, IconRocket } from '@tabler/icons-solidjs';
import { createProject, getProjects } from '@/api/bridge/projects';

export default function NewProjectOverlay({ isOpen, onClose, onProjectCreated, onProjectSelect, reloadProjects }) {
  const [newProjectName, setNewProjectName] = createSignal('');
  const [creating, setCreating] = createSignal(false);
  const [creationProgress, setCreationProgress] = createSignal({ step: 0, message: '', total: 4 });
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

  const handleOverlayClick = (e) => {
    if (e.target === e.currentTarget) {
      onClose();
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
      
      // Step 3: Reload project list (if reloadProjects function provided)
      if (reloadProjects) {
        setCreationProgress({ step: 3, message: 'Refreshing project list...', total: 4 });
        await reloadProjects();
      } else {
        setCreationProgress({ step: 3, message: 'Project created successfully...', total: 4 });
        await new Promise(resolve => setTimeout(resolve, 200));
      }
      
      // Step 4: Select the new project (if onProjectSelect function provided)
      setCreationProgress({ step: 4, message: 'Opening project...', total: 4 });
      if (onProjectSelect) {
        // Find and select the new project
        const projectData = await getProjects();
        const newProject = projectData?.find(p => p.name === name);
        if (newProject) {
          onProjectSelect(newProject);
        }
      } else {
        await new Promise(resolve => setTimeout(resolve, 200));
      }
      
      // Notify parent about project creation
      if (onProjectCreated) {
        await onProjectCreated(name);
      }
      
      // Close dialog and reset
      onClose();
      setNewProjectName('');
    } catch (err) {
      console.error('Failed to create project:', err);
      alert('Failed to create project. Please try again.');
    } finally {
      setCreating(false);
      setCreationProgress({ step: 0, message: '', total: 4 });
    }
  };

  return (
    <Show when={isOpen()}>
      <div 
        class="fixed inset-0 bg-base-100/70 backdrop-blur-md flex items-center justify-center p-4 z-[200] animate-in fade-in duration-300"
        onClick={handleOverlayClick}
      >
        <div class="bg-base-200 backdrop-blur-xl rounded-2xl border border-base-content/30 p-8 w-full max-w-lg shadow-2xl animate-in zoom-in-95 duration-300">
          {/* Header */}
          <div class="flex items-center justify-between mb-6">
            <div class="flex items-center gap-3">
              <div class="w-10 h-10 bg-gradient-to-br from-primary to-secondary rounded-xl flex items-center justify-center">
                <IconPlus class="w-6 h-6 text-primary-content" />
              </div>
              <h2 class="text-2xl font-bold text-base-content">Create New Project</h2>
            </div>
            <button
              onClick={onClose}
              class="w-8 h-8 flex items-center justify-center text-base-content/60 hover:text-base-content hover:bg-base-300 rounded-lg transition-colors"
              disabled={creating()}
            >
              <IconX class="w-4 h-4" />
            </button>
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
              disabled={creating()}
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
                disabled={creating()}
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
                      disabled={creating()}
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
                disabled={creating()}
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
                  disabled={creating()}
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
                  disabled={creating()}
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
                onClose();
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
  );
}