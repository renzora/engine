import { Plugin } from '@/plugins/core/engine/Plugin.jsx';
import SplashScreen from './SplashScreen.jsx';
import { ProjectProvider, useProject } from './ProjectStore.jsx';
import { IconHome, IconFolder, IconSettings } from '@tabler/icons-solidjs';
import { createSignal, Show } from 'solid-js';
import { useEngineAPI } from '@/plugins/core/engine/EngineAPI';

// Splash viewport component designed for viewport tab system
function SplashViewport() {
  const { currentProject, isProjectLoaded, setCurrentProject } = useProject();
  const engineAPI = useEngineAPI();

  const handleProjectSelect = (project) => {
    console.log('🎯 Project selected from splash viewport:', project.name);
    setCurrentProject(project);
    
    // Enable UI elements when project is loaded
    engineAPI.setPropertiesPanelVisible(true);
    engineAPI.setBottomPanelVisible(true);
    engineAPI.setHorizontalMenuButtonsEnabled(true);
    
    // Create the default scene viewport and make it active
    engineAPI.createSceneViewport({
      name: 'Scene 1',
      setActive: true
    });
    
    // Close the splash viewport after a brief delay to ensure scene viewport is created
    setTimeout(() => {
      // Import viewport actions dynamically to close splash viewport
      import('@/plugins/editor/stores/ViewportStore.jsx').then(({ viewportActions, viewportStore }) => {
        // Find and close the splash viewport tab
        const splashTab = viewportStore.tabs.find(tab => tab.type === 'splash-viewport');
        if (splashTab) {
          console.log('🎯 Closing splash viewport tab:', splashTab.id);
          viewportActions.removeViewportTab(splashTab.id);
        }
      }).catch(err => {
        console.error('Failed to close splash viewport:', err);
      });
    }, 100);
    
    // Emit event for other plugins
    document.dispatchEvent(new CustomEvent('engine:project-selected', { 
      detail: { project } 
    }));
  };

  return (
    <Show when={isProjectLoaded() && currentProject()} fallback={
      <SplashScreen onProjectSelect={handleProjectSelect} />
    }>
      <div class="w-full h-full bg-slate-900 text-white flex items-center justify-center">
        <div class="text-center max-w-md mx-auto p-6">
          <div class="mb-4">
            <div class="p-3 bg-green-600/20 rounded-full w-16 h-16 flex items-center justify-center mx-auto mb-4">
              <IconFolder class="w-8 h-8 text-green-400" />
            </div>
            <h1 class="text-xl font-bold mb-2">Project Ready</h1>
            <p class="text-lg text-green-400 font-semibold">{currentProject()?.name}</p>
          </div>
          <p class="text-gray-400 text-sm">
            Your project is loaded and ready to use. 
            Switch to other viewport tabs to start working with your 3D scenes.
          </p>
        </div>
      </div>
    </Show>
  );
}

export default class SplashPluginClass extends Plugin {
  constructor(engineAPI) {
    super(engineAPI);
    this.projectState = createSignal(null);
  }

  getId() {
    return 'splash-plugin';
  }

  getName() {
    return 'Splash Screen Plugin';
  }

  getVersion() {
    return '1.0.0';
  }

  getDescription() {
    return 'Project selection and startup screen for Renzora Engine';
  }

  getAuthor() {
    return 'Renzora Engine Team';
  }

  async onInit() {
    console.log('[SplashPlugin] Initializing splash screen plugin...');
    
    // Register splash theme
    this.registerTheme('splash-theme', {
      name: 'Splash Theme',
      description: 'Clean theme for project selection',
      colors: {
        primary: '#3b82f6',
        secondary: '#1e40af',
        accent: '#60a5fa'
      },
      cssVariables: {
        '--splash-primary': '#3b82f6',
        '--splash-bg': '#0f172a'
      }
    });

    console.log('[SplashPlugin] Splash plugin initialized');
  }

  async onStart() {
    console.log('[SplashPlugin] Starting splash screen plugin...');
    
    // Register splash screen as viewport type
    console.log('[SplashPlugin] Registering splash viewport type...');
    this.registerViewportType('splash-viewport', {
      label: 'Project Home',
      component: SplashViewport,
      icon: IconHome,
      description: 'Project selection and management screen'
    });
    console.log('[SplashPlugin] Splash viewport type registered');

    // Configure UI for splash screen state
    this.engineAPI.setPropertiesPanelVisible(false);
    this.engineAPI.setBottomPanelVisible(false);
    this.engineAPI.setHorizontalMenuButtonsEnabled(false);
    
    // Auto-create splash viewport tab on app load
    setTimeout(() => {
      console.log('[SplashPlugin] Creating splash viewport tab...');
      this.createViewportTab('splash-viewport', {
        label: 'Project Home',
        setActive: true
      });
    }, 500); // Increased delay to ensure viewport system is ready
    
    // Note: Removed home menu item as splash is now a viewport tab
    // Users can access splash screen by creating a new splash viewport tab

    // Register project management property tab
    this.registerPropertyTab('project-management', {
      title: 'Projects',
      component: () => (
        <div class="p-4 space-y-4">
          <h3 class="font-semibold text-white">Project Management</h3>
          
          <div class="space-y-3">
            <button class="w-full px-3 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded text-sm flex items-center gap-2">
              <IconFolder class="w-4 h-4" />
              New Project
            </button>
            
            <button class="w-full px-3 py-2 bg-gray-600 hover:bg-gray-700 text-white rounded text-sm flex items-center gap-2">
              <IconFolder class="w-4 h-4" />
              Open Project
            </button>
            
            <div class="border-t border-slate-600 pt-3">
              <h4 class="text-sm font-medium text-gray-300 mb-2">Recent Projects</h4>
              <div class="space-y-1">
                <div class="text-xs text-gray-400 hover:text-white cursor-pointer p-2 hover:bg-slate-700 rounded">
                  Sample Project
                </div>
                <div class="text-xs text-gray-400 hover:text-white cursor-pointer p-2 hover:bg-slate-700 rounded">
                  Demo Scene
                </div>
              </div>
            </div>
          </div>
        </div>
      ),
      icon: IconFolder,
      order: 0 // First tab
    });

    // Register splash settings tab
    this.registerBottomPanelTab('splash-settings', {
      title: 'Startup',
      component: () => (
        <div class="h-full flex flex-col bg-slate-900 p-4">
          <h3 class="font-semibold text-white mb-4">Startup Settings</h3>
          
          <div class="space-y-3">
            <div>
              <label class="flex items-center gap-2 text-sm text-gray-300">
                <input type="checkbox" class="rounded" checked />
                Show splash screen on startup
              </label>
            </div>
            
            <div>
              <label class="flex items-center gap-2 text-sm text-gray-300">
                <input type="checkbox" class="rounded" />
                Auto-load last project
              </label>
            </div>
            
            <div>
              <label class="block text-sm text-gray-300 mb-1">Default project directory</label>
              <input 
                type="text" 
                value="./projects"
                class="w-full px-3 py-2 bg-slate-700 border border-slate-600 rounded text-white text-sm"
              />
            </div>
          </div>
        </div>
      ),
      icon: IconSettings,
      order: 1,
      defaultHeight: 200
    });

    // Listen for project events
    this.on('project-created', (data) => {
      console.log('[SplashPlugin] Project created event received:', data);
    });

    this.on('project-opened', (data) => {
      console.log('[SplashPlugin] Project opened event received:', data);
    });

    // Listen for project selection (no longer auto-opens viewports)
    this.on('project-selected', (data) => {
      console.log('[SplashPlugin] Project selected:', data.project.name);
      // Project selection now just loads the fixed editor layout
    });

    console.log('[SplashPlugin] Splash screen plugin started');
  }

  onUpdate() {
    // Handle any real-time updates for project loading, etc.
  }

  async onStop() {
    console.log('[SplashPlugin] Stopping splash screen plugin...');
  }

  async onDispose() {
    console.log('[SplashPlugin] Disposing splash screen plugin...');
  }
}