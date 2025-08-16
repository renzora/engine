import { Plugin } from '@/plugins/core/engine/Plugin.jsx';
import { bridgeService } from '@/plugins/core/bridge';
import { 
  IconFolder, 
  IconFolderPlus, 
  IconHistory, 
  IconSettings,
  IconDatabase
} from '@tabler/icons-solidjs';
import { createSignal, onMount, For } from 'solid-js';

// Project Management Viewport Component
function ProjectManagementViewport() {
  const [projects, setProjects] = createSignal([]);
  const [loading, setLoading] = createSignal(true);
  const [currentProject, setCurrentProject] = createSignal(null);

  onMount(async () => {
    try {
      const projectList = await bridgeService.getProjects();
      setProjects(projectList || []);
      setCurrentProject(bridgeService.getCurrentProject());
    } catch (error) {
      console.error('Failed to load projects:', error);
    } finally {
      setLoading(false);
    }
  });

  const handleProjectSelect = async (project) => {
    try {
      const projectData = {
        name: project.name,
        path: project.path,
        loaded: new Date()
      };
      bridgeService.setCurrentProject(projectData);
      setCurrentProject(projectData);
      
      // Emit project selected event
      document.dispatchEvent(new CustomEvent('engine:project-selected', { 
        detail: { project: projectData } 
      }));
    } catch (error) {
      console.error('Failed to load project:', error);
    }
  };

  return (
    <div class="w-full h-full bg-slate-900 text-white p-6">
      <div class="max-w-4xl mx-auto">
        <h1 class="text-2xl font-bold mb-6 flex items-center gap-2">
          <IconFolder class="w-6 h-6 text-blue-400" />
          Project Management
        </h1>
        
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* Current Project */}
          <div class="bg-slate-800 p-4 rounded-lg">
            <h2 class="text-lg font-semibold mb-4 flex items-center gap-2">
              <IconDatabase class="w-5 h-5 text-green-400" />
              Current Project
            </h2>
            {currentProject() ? (
              <div class="space-y-2">
                <div class="text-white font-medium">{currentProject().name}</div>
                <div class="text-sm text-gray-400">{currentProject().path}</div>
                <div class="text-xs text-gray-500">
                  Loaded: {new Date(currentProject().loaded).toLocaleString()}
                </div>
              </div>
            ) : (
              <div class="text-gray-400">No project loaded</div>
            )}
          </div>

          {/* Create New Project */}
          <div class="bg-slate-800 p-4 rounded-lg">
            <h2 class="text-lg font-semibold mb-4 flex items-center gap-2">
              <IconFolderPlus class="w-5 h-5 text-green-400" />
              Create New Project
            </h2>
            <div class="space-y-3">
              <input 
                type="text" 
                placeholder="Project name"
                class="w-full px-3 py-2 bg-slate-700 border border-slate-600 rounded text-white"
              />
              <button class="w-full px-3 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded">
                Create Project
              </button>
            </div>
          </div>

          {/* Available Projects */}
          <div class="lg:col-span-2 bg-slate-800 p-4 rounded-lg">
            <h2 class="text-lg font-semibold mb-4 flex items-center gap-2">
              <IconHistory class="w-5 h-5 text-blue-400" />
              Available Projects
            </h2>
            
            {loading() ? (
              <div class="text-gray-400">Loading projects...</div>
            ) : projects().length > 0 ? (
              <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
                <For each={projects()}>
                  {(project) => (
                    <div 
                      class="bg-slate-700 p-3 rounded cursor-pointer hover:bg-slate-600 transition-colors"
                      onClick={() => handleProjectSelect(project)}
                    >
                      <div class="font-medium text-white">{project.name}</div>
                      <div class="text-sm text-gray-400">{project.path}</div>
                      {project.created && (
                        <div class="text-xs text-gray-500 mt-1">
                          Created: {new Date(project.created).toLocaleDateString()}
                        </div>
                      )}
                    </div>
                  )}
                </For>
              </div>
            ) : (
              <div class="text-gray-400">No projects found</div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

export default class ProjectPluginClass extends Plugin {
  constructor(engineAPI) {
    super(engineAPI);
    this.fileWatcherConnected = false;
    this.currentProject = null;
  }

  getId() {
    return 'project-plugin';
  }

  getName() {
    return 'Project Management Plugin';
  }

  getVersion() {
    return '1.0.0';
  }

  getDescription() {
    return 'Core project management and state for Renzora Engine';
  }

  getAuthor() {
    return 'Renzora Engine Team';
  }

  async onInit() {
    console.log('[ProjectPlugin] Initializing project management plugin...');
    
    // Register project theme
    this.registerTheme('project-theme', {
      name: 'Project Management Theme',
      description: 'Theme optimized for project management workflows',
      colors: {
        primary: '#2563eb',
        secondary: '#1d4ed8',
        accent: '#3b82f6'
      },
      cssVariables: {
        '--project-primary': '#2563eb',
        '--project-secondary': '#1d4ed8'
      }
    });

    // Set up project event listeners via the engine API
    this.on('project-selected', (data) => {
      this.currentProject = data.project;
      this.emit('project-loaded', { project: data.project });
      console.log(`[ProjectPlugin] Project loaded: ${data.project.name}`);
    });

    console.log('[ProjectPlugin] Project management plugin initialized');
  }

  async onStart() {
    console.log('[ProjectPlugin] Starting project management plugin...');
    
    // Register project top menu item
    this.registerTopMenuItem('project-manager', {
      label: 'Project',
      icon: IconFolder,
      order: 3,
      onClick: () => {
        console.log('[ProjectPlugin] Project menu clicked - could show project dialog');
        // Could open project management modal/dialog
      }
    });

    // Register project properties tab
    this.registerPropertyTab('project-properties', {
      title: 'Project',
      component: () => (
        <div class="p-4 space-y-4">
          <h3 class="font-semibold text-white">Project Properties</h3>
          
          {this.currentProject ? (
            <div class="space-y-3">
              <div>
                <label class="block text-sm text-gray-300 mb-1">Name</label>
                <input 
                  type="text" 
                  value={this.currentProject.name}
                  class="w-full px-3 py-2 bg-slate-700 border border-slate-600 rounded text-white text-sm"
                  readonly
                />
              </div>
              
              <div>
                <label class="block text-sm text-gray-300 mb-1">Path</label>
                <input 
                  type="text" 
                  value={this.currentProject.path}
                  class="w-full px-3 py-2 bg-slate-700 border border-slate-600 rounded text-white text-sm"
                  readonly
                />
              </div>

              <div class="flex gap-2">
                <button class="flex-1 px-3 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded text-sm">
                  Save Project
                </button>
                <button class="flex-1 px-3 py-2 bg-gray-600 hover:bg-gray-700 text-white rounded text-sm">
                  Close Project
                </button>
              </div>
            </div>
          ) : (
            <div class="text-gray-400 text-sm">No project loaded</div>
          )}
        </div>
      ),
      icon: IconDatabase,
      order: 1
    });

    // Register project console tab
    this.registerBottomPanelTab('project-console', {
      title: 'Project Log',
      component: () => (
        <div class="h-full flex flex-col bg-slate-900">
          <div class="flex items-center justify-between p-3 border-b border-slate-700">
            <h3 class="font-semibold text-white">Project Activity Log</h3>
            <button class="px-2 py-1 bg-gray-600 hover:bg-gray-700 text-white text-xs rounded">
              Clear
            </button>
          </div>
          <div class="flex-1 overflow-auto p-3 space-y-1">
            <div class="text-xs font-mono text-green-400">
              <span class="text-gray-500">{new Date().toLocaleTimeString()}</span>
              <span class="text-green-500 font-bold ml-2">[INFO]</span>
              <span class="ml-2">Project management system initialized</span>
            </div>
            {this.currentProject && (
              <div class="text-xs font-mono text-blue-400">
                <span class="text-gray-500">{new Date().toLocaleTimeString()}</span>
                <span class="text-blue-500 font-bold ml-2">[PROJECT]</span>
                <span class="ml-2">Loaded project: {this.currentProject.name}</span>
              </div>
            )}
          </div>
        </div>
      ),
      icon: IconHistory,
      order: 2,
      defaultHeight: 200
    });

    // Listen to splash plugin events
    this.on('project-selected', (data) => {
      console.log('[ProjectPlugin] Project selected event received:', data);
      this.currentProject = data.project;
    });

    // Start monitoring for file changes
    this.addUpdateCallback(() => {
      // Check file watcher status periodically
      if (!this.fileWatcherConnected) {
        // Try to establish file watcher connection
      }
    });

    console.log('[ProjectPlugin] Project management plugin started');
  }

  onUpdate() {
    // Handle real-time project updates, file watching, etc.
  }

  async onStop() {
    console.log('[ProjectPlugin] Stopping project management plugin...');
    // Clean up file watchers and connections
  }

  async onDispose() {
    console.log('[ProjectPlugin] Disposing project management plugin...');
    // Final cleanup - no legacy handlers to remove
  }
}