import { createSignal, onMount, Show, For } from 'solid-js';
import { IconFolder, IconPlus, IconFolderOpen, IconSettings, IconRocket, IconBox, IconTrash } from '@tabler/icons-solidjs';
import { getProjects, deleteProject } from '@/api/bridge/projects';
import { loadProjectDirect } from '@/api/bridge/directProjectLoader.js';
import NewProjectOverlay from '@/ui/NewProjectOverlay.jsx';

export default function ProjectSelection({ onProjectSelect }) {
  const [projects, setProjects] = createSignal([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal(null);
  const [showCreateDialog, setShowCreateDialog] = createSignal(false);
  const [showDeleteDialog, setShowDeleteDialog] = createSignal(false);
  const [projectToDelete, setProjectToDelete] = createSignal(null);
  const [deletingProject, setDeletingProject] = createSignal(false);

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

  // Handle project selection
  const handleProjectSelect = async (project) => {
    try {
      // Use direct project loader - this will load the project immediately and handle indexing in background
      await loadProjectDirect(project, onProjectSelect);
    } catch (error) {
      console.error('Failed to load project:', error);
      setError(`Failed to load project: ${error.message || error}`);
    }
  };

  // Show delete confirmation dialog
  const showDeleteConfirmation = (project) => {
    setProjectToDelete(project);
    setShowDeleteDialog(true);
  };

  // Delete project
  const handleDeleteProject = async () => {
    const project = projectToDelete();
    if (!project) return;

    try {
      setDeletingProject(true);
      setError(null);
      
      await deleteProject(project.name);
      
      // Reload projects list
      await loadProjects();
      
      // Close dialog
      setShowDeleteDialog(false);
      setProjectToDelete(null);
    } catch (err) {
      console.error('Failed to delete project:', err);
      setError(`Failed to delete project: ${err.message || err}`);
    } finally {
      setDeletingProject(false);
    }
  };

  // Load projects on mount
  onMount(() => {
    loadProjects();
  });

  return (
    <div class="h-full flex flex-col bg-base-100">
      {/* Header */}
      <div class="bg-base-200/50 border-b border-base-300/50 p-4">
        <div class="flex items-center gap-3 mb-2">
          <div class="w-8 h-8 bg-gradient-to-br from-primary to-secondary rounded-xl flex items-center justify-center">
            <IconRocket class="w-5 h-5 text-primary-content" />
          </div>
          <div>
            <h2 class="text-lg font-bold text-base-content">Welcome to Renzora Engine</h2>
            <p class="text-sm text-base-content/70">Select or create a project to get started</p>
          </div>
        </div>
        
        {/* Create New Project Button */}
        <button
          onClick={() => setShowCreateDialog(true)}
          class="w-full p-3 border border-base-content/15 hover:border-primary hover:bg-gradient-to-br hover:from-primary/10 hover:to-secondary/10 rounded-xl transition-all duration-300 group bg-base-200 hover:shadow-lg"
        >
          <div class="flex items-center gap-3 text-base-content/70 group-hover:text-primary">
            <div class="w-8 h-8 bg-gradient-to-br from-primary/10 to-secondary/10 group-hover:from-primary/20 group-hover:to-secondary/20 rounded-xl flex items-center justify-center border border-base-content/10 group-hover:border-primary/30 transition-all">
              <IconPlus class="w-4 h-4" />
            </div>
            <div class="text-left">
              <div class="font-semibold text-base-content">Create New Project</div>
              <div class="text-xs text-base-content/50 group-hover:text-base-content/60">Start building something amazing</div>
            </div>
          </div>
        </button>
      </div>

      {/* Content */}
      <div class="flex-1 p-4 overflow-y-auto">
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
            <div class="text-xs font-semibold text-base-content/40 uppercase tracking-wider mb-4">Available Projects</div>
            <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
              <For each={projects()}>
                {(project) => (
                  <div class="relative group">
                    <button
                      onClick={() => handleProjectSelect(project)}
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
                    
                    {/* Delete Project Button */}
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        showDeleteConfirmation(project);
                      }}
                      class="absolute top-2 right-2 p-1 bg-base-300 hover:bg-error/20 rounded-lg opacity-0 group-hover:opacity-100 transition-all duration-200"
                      title="Delete Project"
                    >
                      <IconTrash class="w-4 h-4 text-base-content/70 hover:text-error" />
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
        </Show>
      </div>

      {/* Create Project Dialog */}
      <NewProjectOverlay 
        isOpen={showCreateDialog}
        onClose={() => setShowCreateDialog(false)}
        onProjectSelect={onProjectSelect}
        reloadProjects={loadProjects}
      />

      {/* Delete Project Confirmation Dialog */}
      <Show when={showDeleteDialog()}>
        <div class="fixed inset-0 bg-base-100/70 backdrop-blur-md flex items-center justify-center p-4 z-[100] animate-in fade-in duration-300">
          <div class="bg-base-200 backdrop-blur-xl rounded-2xl border border-base-content/30 p-8 w-full max-w-md shadow-2xl animate-in zoom-in-95 duration-300">
            <div class="flex items-center gap-3 mb-6">
              <div class="w-10 h-10 bg-gradient-to-br from-error to-error/80 rounded-xl flex items-center justify-center">
                <IconTrash class="w-6 h-6 text-error-content" />
              </div>
              <div>
                <h2 class="text-xl font-bold text-base-content">Delete Project</h2>
                <p class="text-sm text-base-content/60">This action cannot be undone</p>
              </div>
            </div>
            
            <div class="mb-6">
              <p class="text-base-content mb-4">
                Are you sure you want to delete the project 
                <span class="font-semibold text-primary">"{projectToDelete()?.name}"</span>?
              </p>
              <p class="text-sm text-base-content/60">
                This will permanently delete all project files, assets, and scenes.
              </p>
            </div>

            <Show when={error()}>
              <div class="mb-4 p-3 bg-error/20 border border-error/30 rounded-lg">
                <p class="text-error text-sm">{error()}</p>
              </div>
            </Show>

            <div class="flex justify-end gap-3">
              <button
                onClick={() => {
                  setShowDeleteDialog(false);
                  setProjectToDelete(null);
                  setError(null);
                }}
                class="btn btn-ghost"
                disabled={deletingProject()}
              >
                Cancel
              </button>
              <button
                onClick={handleDeleteProject}
                class="btn btn-error"
                disabled={deletingProject()}
              >
                <Show when={deletingProject()}>
                  <div class="w-4 h-4 border-2 border-error-content border-t-transparent rounded-full animate-spin mr-2"></div>
                </Show>
                Delete Project
              </button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
}