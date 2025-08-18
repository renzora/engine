import { createPlugin } from '@/api/plugin';
import { bridgeService } from '@/plugins/core/bridge';
import { Folder, FolderPlus, History, Database, Camera, Grid3x3, Settings as SettingsIcon, Maximize } from '@/ui/icons';
import { createSignal, onMount, For } from 'solid-js';

// Import editor components  
import Scene from '@/pages/editor/Scene.jsx';
import SettingsComponent from '@/pages/editor/Settings.jsx';
import AssetLibrary from '@/pages/editor/AssetLibrary.jsx';
import CameraDropdownContent from '@/ui/display/CameraDropdownContent.jsx';
import GridDropdownContent from '@/ui/display/GridDropdownContent.jsx';

function SceneIcon() {
  return <div>🎬</div>;
}

function AssetsIcon() {
  return <div>📁</div>;
}

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
          <Folder class="w-6 h-6 text-blue-400" />
          Project Management
        </h1>
        
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
          <div class="bg-slate-800 p-4 rounded-lg">
            <h2 class="text-lg font-semibold mb-4 flex items-center gap-2">
              <Database class="w-5 h-5 text-green-400" />
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

          <div class="bg-slate-800 p-4 rounded-lg">
            <h2 class="text-lg font-semibold mb-4 flex items-center gap-2">
              <FolderPlus class="w-5 h-5 text-green-400" />
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

          <div class="lg:col-span-2 bg-slate-800 p-4 rounded-lg">
            <h2 class="text-lg font-semibold mb-4 flex items-center gap-2">
              <History class="w-5 h-5 text-blue-400" />
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

export default createPlugin({
  id: 'project-plugin',
  name: 'Project Management Plugin',
  version: '1.0.0',
  description: 'Core project management and state for Renzora Engine',
  author: 'Renzora Engine Team',

  async onInit(api) {
    console.log('[ProjectPlugin] Initializing project management plugin...');
    
    // Register project theme
    api.theme('project-theme', {
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

    console.log('[ProjectPlugin] Project management plugin initialized');
  },

  async onStart(api) {
    console.log('[ProjectPlugin] Starting project management plugin...');
    
    // Register editor UI components
    console.log('[ProjectPlugin] Registering editor components...');
    
    // Register property tabs (right panel)
    api.tab('scene', {
      title: 'Scene',
      component: Scene,
      icon: SceneIcon,
      order: 10
    });

    api.tab('settings', {
      title: 'Settings',
      component: SettingsComponent,
      icon: SettingsIcon,
      order: 20
    });

    // Register bottom panel tabs
    api.panel('assets', {
      title: 'Assets',
      component: AssetLibrary,
      icon: AssetsIcon,
      order: 10,
      defaultHeight: 300
    });
    
    // Register toolbar buttons (right side of horizontal toolbar)
    api.button('camera-helper', {
      title: 'Camera Options',
      icon: Camera,
      section: 'right',
      order: 10,
      hasDropdown: true,
      dropdownComponent: CameraDropdownContent,
      dropdownWidth: 256
    });
    
    api.button('grid-helper', {
      title: 'Grid Options',
      icon: Grid3x3,
      section: 'right',
      order: 20,
      hasDropdown: true,
      dropdownComponent: GridDropdownContent,
      dropdownWidth: 256
    });
    
    api.button('settings-button', {
      title: 'Settings',
      icon: SettingsIcon,
      section: 'right',
      order: 30,
      onClick: () => {
        console.log('[ProjectPlugin] Settings button clicked');
      }
    });
    
    api.button('fullscreen-button', {
      title: 'Toggle Fullscreen',
      icon: Maximize,
      section: 'right',
      order: 40,
      onClick: () => {
        if (!document.fullscreenElement) {
          document.documentElement.requestFullscreen().catch(err => {
            console.error('[ProjectPlugin] Error attempting to enable fullscreen:', err);
          });
        } else {
          document.exitFullscreen();
        }
      }
    });

    console.log('[ProjectPlugin] Editor components registered');
    
    // Listen to project events
    api.on('project-selected', (data) => {
      console.log('[ProjectPlugin] Project selected event received:', data);
      api.emit('project-loaded', { project: data.project });
    });

    console.log('[ProjectPlugin] Project management plugin started');
  },

  onUpdate() {
    // Handle real-time project updates, file watching, etc.
  },

  async onStop() {
    console.log('[ProjectPlugin] Stopping project management plugin...');
    // Clean up file watchers and connections
  },

  async onDispose() {
    console.log('[ProjectPlugin] Disposing project management plugin...');
    // Final cleanup
  }
});